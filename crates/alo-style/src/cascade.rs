/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which declaration wins.
//!
//! For one element and one property, several declarations may apply. CSS
//! resolves that by asking five questions in order — which origin, whether it
//! is `!important`, whether it is attached to the element by its `style`
//! attribute, how specific the selector was, and which came last — and this
//! asks them in that order and no other. Asking them in a different order
//! is a bug that looks like a rendering bug: the page is wrong and every
//! individual rule is right.
//!
//! What is *not* here is what a winning value means. `var()`, `inherit` and
//! the rest are [`crate::computed`]'s business, because they need the parent's
//! answer and the cascade does not.

use crate::origin::{CascadeLevel, Origin};
use alo_css::{
    Declaration, Location, MatchContext, MediaContext, PropertyName, PseudoElement, SelectorList,
    Specificity, Stylesheet,
};
use alo_dom::NodeId;
use std::collections::BTreeMap;

/// A style sheet and where it came from.
#[derive(Debug, Clone, Copy)]
pub struct SourcedSheet<'a> {
    /// Who wrote it.
    pub origin: Origin,
    /// The sheet.
    pub sheet: &'a Stylesheet,
}

impl<'a> SourcedSheet<'a> {
    /// A sheet from an origin.
    pub fn new(origin: Origin, sheet: &'a Stylesheet) -> Self {
        Self { origin, sheet }
    }
}

/// One declaration that applied, and everything needed to order it against the
/// others.
#[derive(Debug, Clone, Copy)]
pub struct Contender<'a> {
    /// The declaration itself.
    pub declaration: &'a Declaration,
    /// Who wrote it.
    pub origin: Origin,
    /// Origin and importance together.
    pub level: CascadeLevel,
    /// Whether it was attached to the element by its `style` attribute
    /// rather than reached through a selector.
    ///
    /// CSS Cascade 4 § 6.1 asks this after origin and importance and before
    /// specificity (ADR 0033 § 1). It is a question of its own rather than a
    /// very large specificity, and the difference shows: an important sheet
    /// declaration still beats a normal attached one, and an important
    /// attached one beats an important sheet declaration with an id.
    pub attached: bool,
    /// How specific the selector that matched was — the selector, not the
    /// list it was written in. Zero for an attached declaration, which has
    /// no selector.
    pub specificity: Specificity,
    /// Where it was in the document, counting every declaration of every rule
    /// that applied, in order.
    pub order: usize,
    /// Where the rule it came from was written, for a diagnostic.
    pub at: Location,
}

impl Contender<'_> {
    /// The key CSS orders declarations by, greatest first.
    fn key(&self) -> (CascadeLevel, bool, Specificity, usize) {
        (self.level, self.attached, self.specificity, self.order)
    }
}

/// Every declaration that applies to one element, grouped by property and
/// ordered so that the last of each group is the one that wins.
#[derive(Debug, Default)]
pub struct Applicable<'a> {
    by_property: BTreeMap<PropertyName, Vec<Contender<'a>>>,
}

impl<'a> Applicable<'a> {
    /// Gather every declaration that applies to one element.
    ///
    /// Sheets are consulted in the order given, and the order a declaration
    /// was written in is counted across all of them — so a later sheet beats an
    /// earlier one at equal specificity, which is what linking two style sheets
    /// means.
    pub fn gather(
        sheets: &[SourcedSheet<'a>],
        device: &MediaContext,
        matcher: &mut MatchContext<'_>,
        id: NodeId,
    ) -> Self {
        Self::gather_with_hints(sheets, device, matcher, id, &[])
    }

    /// The same, with an element's presentation attributes as well
    /// ([`crate::presentation`]).
    ///
    /// They are author declarations of specificity zero, counted **before**
    /// every sheet, so that any author rule at all beats them on order where
    /// it does not already on specificity. The engine's own sheet still loses
    /// to them, by origin, which is what SVG 2 asks: `fill="none"` on a path
    /// is the author speaking.
    pub fn gather_with_hints(
        sheets: &[SourcedSheet<'a>],
        device: &MediaContext,
        matcher: &mut MatchContext<'_>,
        id: NodeId,
        hints: &'a [Declaration],
    ) -> Self {
        Self::gather_attached(sheets, device, matcher, id, hints, &[])
    }

    /// The same, with the declarations of the element's `style` attribute
    /// ([`crate::attached`]) as well.
    ///
    /// They are author declarations that answer the cascade's third question
    /// yes, so they beat every declaration of the same origin and importance
    /// that a selector reached, whatever its specificity, and among
    /// themselves the last written wins.
    pub fn gather_attached(
        sheets: &[SourcedSheet<'a>],
        device: &MediaContext,
        matcher: &mut MatchContext<'_>,
        id: NodeId,
        hints: &'a [Declaration],
        attached: &'a [Declaration],
    ) -> Self {
        let mut by_property: BTreeMap<PropertyName, Vec<Contender<'a>>> = BTreeMap::new();
        let mut order = 0usize;

        for declaration in hints {
            by_property
                .entry(declaration.name.clone())
                .or_default()
                .push(Contender {
                    declaration,
                    origin: Origin::Author,
                    level: CascadeLevel::of(Origin::Author, declaration.importance),
                    attached: false,
                    specificity: Specificity::default(),
                    order,
                    at: Location { line: 0, column: 0 },
                });
            order += 1;
        }

        gather_rules(sheets, device, &mut by_property, &mut order, |list| {
            matcher
                .most_specific_match(list, id)
                .map(alo_css::Selector::specificity)
        });

        for declaration in attached {
            by_property
                .entry(declaration.name.clone())
                .or_default()
                .push(Contender {
                    declaration,
                    origin: Origin::Author,
                    level: CascadeLevel::of(Origin::Author, declaration.importance),
                    attached: true,
                    specificity: Specificity::default(),
                    order,
                    at: Location { line: 0, column: 0 },
                });
            order += 1;
        }

        for contenders in by_property.values_mut() {
            contenders.sort_by_key(Contender::key);
        }
        Self { by_property }
    }

    /// Every declaration that applies to the pseudo-element `pseudo` of one
    /// element (ADR 0043 § 1): those of the rules whose selector names it and
    /// whose compound before it matches the element.
    ///
    /// They compete with each other by the same five questions and never
    /// with the element's own. There are no presentation hints and no
    /// attached declarations: neither has a selector, so neither can name a
    /// pseudo-element.
    pub fn gather_pseudo(
        sheets: &[SourcedSheet<'a>],
        device: &MediaContext,
        matcher: &mut MatchContext<'_>,
        id: NodeId,
        pseudo: PseudoElement,
    ) -> Self {
        let mut by_property: BTreeMap<PropertyName, Vec<Contender<'a>>> = BTreeMap::new();
        let mut order = 0usize;
        gather_rules(sheets, device, &mut by_property, &mut order, |list| {
            matcher
                .most_specific_pseudo_match(list, id, pseudo)
                .map(alo_css::Selector::specificity)
        });
        for contenders in by_property.values_mut() {
            contenders.sort_by_key(Contender::key);
        }
        Self { by_property }
    }

    /// Keep only the properties `keep` says yes to, handing every one it
    /// refuses, with what won for it, to `refused`.
    ///
    /// For a pseudo-element that reads only some of what applies to it
    /// ([`crate::pseudo`]): a property taken out here was never declared, as
    /// far as the computed style knows, and inherits from the element.
    pub fn retain(
        &mut self,
        mut keep: impl FnMut(&PropertyName) -> bool,
        mut refused: impl FnMut(&PropertyName, Contender<'a>),
    ) {
        self.by_property.retain(|name, contenders| {
            if keep(name) {
                return true;
            }
            if let Some(winner) = contenders.last().copied() {
                refused(name, winner);
            }
            false
        });
    }

    /// The declaration that wins for a property, if any does.
    pub fn winner(&self, name: &PropertyName) -> Option<Contender<'a>> {
        self.by_property.get(name)?.last().copied()
    }

    /// The declaration that would have won if a given origin had said nothing.
    ///
    /// This is what `revert` asks for. It is not "the second place": a whole
    /// origin steps aside, both its normal and its important declarations, and
    /// whatever is left is the answer.
    pub fn winner_without(&self, name: &PropertyName, without: Origin) -> Option<Contender<'a>> {
        self.by_property
            .get(name)?
            .iter()
            .rev()
            .find(|contender| contender.origin != without)
            .copied()
    }

    /// Every property that anything set, in a stable order.
    pub fn properties(&self) -> impl Iterator<Item = &PropertyName> {
        self.by_property.keys()
    }

    /// How many properties were set by anything at all.
    pub fn len(&self) -> usize {
        self.by_property.len()
    }

    /// Whether nothing applied.
    pub fn is_empty(&self) -> bool {
        self.by_property.is_empty()
    }
}

/// Every declaration of every style rule in `sheets` whose selector list
/// `specificity_of` matches, counted on from `order`.
///
/// `specificity_of` answers with the specificity of the most specific
/// selector that matched, which is what the declaration competes at: for an
/// element, or for one of its pseudo-elements.
fn gather_rules<'a>(
    sheets: &[SourcedSheet<'a>],
    device: &MediaContext,
    by_property: &mut BTreeMap<PropertyName, Vec<Contender<'a>>>,
    order: &mut usize,
    mut specificity_of: impl FnMut(&SelectorList) -> Option<Specificity>,
) {
    for sourced in sheets {
        for rule in sourced.sheet.style_rules_for(device) {
            let Some(specificity) = specificity_of(&rule.selectors) else {
                continue;
            };
            for declaration in &rule.declarations {
                by_property
                    .entry(declaration.name.clone())
                    .or_default()
                    .push(Contender {
                        declaration,
                        origin: sourced.origin,
                        level: CascadeLevel::of(sourced.origin, declaration.importance),
                        attached: false,
                        specificity,
                        order: *order,
                        at: rule.at,
                    });
                *order += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_css::parse_stylesheet;
    use alo_dom::{Document, parse_document};

    struct Fixture {
        document: Document,
        author: Stylesheet,
        agent: Stylesheet,
    }

    fn fixture(html: &str, author: &str, agent: &str) -> Fixture {
        Fixture {
            document: parse_document(html),
            author: parse_stylesheet(author),
            agent: parse_stylesheet(agent),
        }
    }

    /// The winning value for a property on the element with this id.
    fn winner(fixture: &Fixture, wanted: &str, property: &str) -> Option<String> {
        let id = fixture
            .document
            .descendants(fixture.document.root())
            .find(|id| {
                fixture
                    .document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some(wanted))
            })?;
        let sheets = [
            SourcedSheet::new(Origin::UserAgent, &fixture.agent),
            SourcedSheet::new(Origin::Author, &fixture.author),
        ];
        let mut matcher = MatchContext::new(&fixture.document);
        let applicable = Applicable::gather(&sheets, &MediaContext::default(), &mut matcher, id);
        applicable
            .winner(&PropertyName::parse(property))
            .map(|contender| contender.declaration.value.clone())
    }

    #[test]
    fn a_more_specific_selector_wins() {
        let fixture = fixture(
            "<p id=x class=lead>text</p>",
            "p { color: red } .lead { color: blue }",
            "",
        );
        assert_eq!(winner(&fixture, "x", "color").as_deref(), Some("blue"));
    }

    #[test]
    fn at_equal_specificity_the_later_declaration_wins() {
        let fixture = fixture(
            "<p id=x class=lead>text</p>",
            ".lead { color: red } .lead { color: blue }",
            "",
        );
        assert_eq!(winner(&fixture, "x", "color").as_deref(), Some("blue"));
    }

    #[test]
    fn a_later_declaration_in_the_same_block_wins_too() {
        let fixture = fixture("<p id=x>t</p>", "p { color: red; color: blue }", "");
        assert_eq!(winner(&fixture, "x", "color").as_deref(), Some("blue"));
    }

    #[test]
    fn specificity_beats_order_however_it_was_written() {
        let fixture = fixture(
            "<p id=x class=lead>t</p>",
            "#x { color: green } .lead { color: blue } p { color: red }",
            "",
        );
        assert_eq!(winner(&fixture, "x", "color").as_deref(), Some("green"));
    }

    #[test]
    fn important_beats_specificity() {
        let fixture = fixture(
            "<p id=x class=lead>t</p>",
            "#x { color: green } .lead { color: blue !important }",
            "",
        );
        assert_eq!(winner(&fixture, "x", "color").as_deref(), Some("blue"));
    }

    #[test]
    fn an_author_rule_beats_the_engines_own_and_important_reverses_that() {
        let plain = fixture("<p id=x>t</p>", "p { color: blue }", "p { color: black }");
        assert_eq!(winner(&plain, "x", "color").as_deref(), Some("blue"));

        let insisted = fixture(
            "<p id=x>t</p>",
            "p { color: blue !important }",
            "p { color: black !important }",
        );
        assert_eq!(winner(&insisted, "x", "color").as_deref(), Some("black"));
    }

    #[test]
    fn the_specificity_counted_is_of_the_selector_that_matched() {
        // `h1, #x` matches this element twice; the id is what should count.
        let fixture = fixture(
            "<h1 id=x>t</h1>",
            "h1, #x { color: green } .other, h1 { color: red }",
            "",
        );
        assert_eq!(winner(&fixture, "x", "color").as_deref(), Some("green"));
    }

    #[test]
    fn a_property_nothing_set_has_no_winner() {
        let fixture = fixture("<p id=x>t</p>", "p { color: red }", "");
        assert_eq!(winner(&fixture, "x", "margin"), None);
    }

    #[test]
    fn reverting_an_origin_takes_out_all_of_it_not_just_the_top_declaration() {
        let fixture = fixture(
            "<p id=x class=lead>t</p>",
            "p { color: red } .lead { color: blue !important }",
            "p { color: black }",
        );
        let id = fixture
            .document
            .descendants(fixture.document.root())
            .find(|id| {
                fixture
                    .document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some("x"))
            })
            .expect("the paragraph");
        let sheets = [
            SourcedSheet::new(Origin::UserAgent, &fixture.agent),
            SourcedSheet::new(Origin::Author, &fixture.author),
        ];
        let mut matcher = MatchContext::new(&fixture.document);
        let applicable = Applicable::gather(&sheets, &MediaContext::default(), &mut matcher, id);
        let color = PropertyName::parse("color");

        assert_eq!(
            applicable
                .winner(&color)
                .map(|contender| contender.declaration.value.clone())
                .as_deref(),
            Some("blue"),
        );
        assert_eq!(
            applicable
                .winner_without(&color, Origin::Author)
                .map(|contender| contender.declaration.value.clone())
                .as_deref(),
            Some("black"),
            "both author declarations step aside, not only the important one",
        );
        assert!(
            applicable
                .winner_without(&color, Origin::UserAgent)
                .is_some(),
            "and taking the engine's own origin out leaves the author's",
        );
    }

    #[test]
    fn every_property_anything_set_is_listed_once() {
        let fixture = fixture(
            "<p id=x>t</p>",
            "p { color: red; margin: 0 } p { color: blue; --gap: 8px }",
            "",
        );
        let id = fixture
            .document
            .descendants(fixture.document.root())
            .find(|id| {
                fixture
                    .document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some("x"))
            })
            .expect("the paragraph");
        let sheets = [SourcedSheet::new(Origin::Author, &fixture.author)];
        let mut matcher = MatchContext::new(&fixture.document);
        let applicable = Applicable::gather(&sheets, &MediaContext::default(), &mut matcher, id);

        // Seven, not three: a `margin` shorthand is written down as its four
        // longhands as well, so that an author's shorthand and a user agent's
        // longhand compete as the same property (see `alo_css`'s expansion).
        assert_eq!(applicable.len(), 7);
        assert!(!applicable.is_empty());
        assert_eq!(
            applicable
                .properties()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec![
                "--gap",
                "color",
                "margin",
                "margin-bottom",
                "margin-left",
                "margin-right",
                "margin-top"
            ],
        );
    }

    #[test]
    fn an_element_nothing_matches_gathers_nothing() {
        let fixture = fixture("<p id=x>t</p>", "div { color: red }", "");
        let id = fixture
            .document
            .descendants(fixture.document.root())
            .find(|id| {
                fixture
                    .document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some("x"))
            })
            .expect("the paragraph");
        let sheets = [SourcedSheet::new(Origin::Author, &fixture.author)];
        let mut matcher = MatchContext::new(&fixture.document);
        let applicable = Applicable::gather(&sheets, &MediaContext::default(), &mut matcher, id);
        assert!(applicable.is_empty());
    }

    #[test]
    fn a_presentation_attribute_beats_the_engines_sheet_and_loses_to_any_author_rule() {
        let fixture = fixture(
            r#"<svg><rect id=x fill="red"/></svg>"#,
            "",
            "rect { fill: black }",
        );
        let unopposed = fixture_winner_with_hints(&fixture, "fill");
        assert_eq!(
            unopposed.as_deref(),
            Some("red"),
            "the engine's own sheet loses"
        );

        let opposed = self::fixture(
            r#"<svg><rect id=x fill="red"/></svg>"#,
            "* { fill: blue }",
            "",
        );
        assert_eq!(
            fixture_winner_with_hints(&opposed, "fill").as_deref(),
            Some("blue"),
            "even the least specific author rule wins, by coming after it",
        );
    }

    /// The winner with everything an element carries — its presentation
    /// attributes and its `style` attribute — gathered as the resolver
    /// gathers them.
    fn attached_winner(html: &str, author: &str, agent: &str, property: &str) -> Option<String> {
        let fixture = fixture(html, author, agent);
        let id = fixture
            .document
            .descendants(fixture.document.root())
            .find(|id| {
                fixture
                    .document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some("x"))
            })?;
        let element = fixture.document.element(id)?;
        let mut issues = Vec::new();
        let hints = crate::presentation::hints(element, &mut issues);
        let attached = crate::attached::declarations(element, &mut issues);
        let sheets = [
            SourcedSheet::new(Origin::UserAgent, &fixture.agent),
            SourcedSheet::new(Origin::Author, &fixture.author),
        ];
        let mut matcher = MatchContext::new(&fixture.document);
        let applicable = Applicable::gather_attached(
            &sheets,
            &MediaContext::default(),
            &mut matcher,
            id,
            &hints,
            attached.as_slice(),
        );
        applicable
            .winner(&PropertyName::parse(property))
            .map(|contender| contender.declaration.value.clone())
    }

    #[test]
    fn a_normal_style_attribute_beats_an_id_selector() {
        assert_eq!(
            attached_winner(
                r#"<p id=x class=lead style="color: red">t</p>"#,
                "#x.lead { color: blue } p#x { color: green }",
                "",
                "color",
            )
            .as_deref(),
            Some("red"),
        );
    }

    #[test]
    fn an_important_sheet_declaration_beats_a_normal_style_attribute() {
        assert_eq!(
            attached_winner(
                r#"<p id=x style="color: red">t</p>"#,
                "p { color: blue !important }",
                "",
                "color",
            )
            .as_deref(),
            Some("blue"),
            "the attribute is its own step, not a specificity above every other",
        );
    }

    #[test]
    fn an_important_style_attribute_beats_an_important_id_selector() {
        assert_eq!(
            attached_winner(
                r#"<p id=x style="color: red !important">t</p>"#,
                "#x { color: blue !important }",
                "",
                "color",
            )
            .as_deref(),
            Some("red"),
        );
    }

    #[test]
    fn the_engines_important_declaration_still_beats_an_important_style_attribute() {
        assert_eq!(
            attached_winner(
                r#"<p id=x style="color: red !important">t</p>"#,
                "",
                "p { color: black !important }",
                "color",
            )
            .as_deref(),
            Some("black"),
        );
    }

    #[test]
    fn a_style_attribute_beats_a_presentation_attribute() {
        assert_eq!(
            attached_winner(
                r#"<svg><rect id=x fill="blue" style="fill: red"/></svg>"#,
                "",
                "",
                "fill",
            )
            .as_deref(),
            Some("red"),
        );
    }

    #[test]
    fn within_a_style_attribute_the_last_declaration_wins() {
        assert_eq!(
            attached_winner(
                r#"<p id=x style="color: red; color: blue; padding: 1px; padding-left: 0">t</p>"#,
                "",
                "",
                "color",
            )
            .as_deref(),
            Some("blue"),
        );
        assert_eq!(
            attached_winner(
                r#"<p id=x style="padding: 1px; padding-left: 0">t</p>"#,
                "#x { padding-left: 9px }",
                "",
                "padding-left",
            )
            .as_deref(),
            Some("0"),
            "a shorthand's longhands are attached as it is",
        );
    }

    fn fixture_winner_with_hints(fixture: &Fixture, property: &str) -> Option<String> {
        let id = fixture
            .document
            .descendants(fixture.document.root())
            .find(|id| {
                fixture
                    .document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some("x"))
            })?;
        let mut issues = Vec::new();
        let hints = crate::presentation::hints(fixture.document.element(id)?, &mut issues);
        let sheets = [
            SourcedSheet::new(Origin::UserAgent, &fixture.agent),
            SourcedSheet::new(Origin::Author, &fixture.author),
        ];
        let mut matcher = MatchContext::new(&fixture.document);
        let applicable = Applicable::gather_with_hints(
            &sheets,
            &MediaContext::default(),
            &mut matcher,
            id,
            &hints,
        );
        applicable
            .winner(&PropertyName::parse(property))
            .map(|contender| contender.declaration.value.clone())
    }
}
