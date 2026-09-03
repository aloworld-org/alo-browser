//! Which declaration wins.
//!
//! For one element and one property, several declarations may apply. CSS
//! resolves that by asking four questions in order — which origin, whether it
//! is `!important`, how specific the selector was, and which came last — and
//! this asks them in that order and no other. Asking them in a different order
//! is a bug that looks like a rendering bug: the page is wrong and every
//! individual rule is right.
//!
//! What is *not* here is what a winning value means. `var()`, `inherit` and
//! the rest are [`crate::computed`]'s business, because they need the parent's
//! answer and the cascade does not.

use crate::origin::{CascadeLevel, Origin};
use alo_css::{
    Declaration, Location, MatchContext, MediaContext, PropertyName, Specificity, Stylesheet,
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
    /// How specific the selector that matched was — the selector, not the
    /// list it was written in.
    pub specificity: Specificity,
    /// Where it was in the document, counting every declaration of every rule
    /// that applied, in order.
    pub order: usize,
    /// Where the rule it came from was written, for a diagnostic.
    pub at: Location,
}

impl Contender<'_> {
    /// The key CSS orders declarations by, greatest first.
    fn key(&self) -> (CascadeLevel, Specificity, usize) {
        (self.level, self.specificity, self.order)
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
        let mut by_property: BTreeMap<PropertyName, Vec<Contender<'a>>> = BTreeMap::new();
        let mut order = 0usize;

        for sourced in sheets {
            for rule in sourced.sheet.style_rules_for(device) {
                let Some(selector) = matcher.most_specific_match(&rule.selectors, id) else {
                    continue;
                };
                let specificity = selector.specificity();
                for declaration in &rule.declarations {
                    by_property
                        .entry(declaration.name.clone())
                        .or_default()
                        .push(Contender {
                            declaration,
                            origin: sourced.origin,
                            level: CascadeLevel::of(sourced.origin, declaration.importance),
                            specificity,
                            order,
                            at: rule.at,
                        });
                    order += 1;
                }
            }
        }

        for contenders in by_property.values_mut() {
            contenders.sort_by_key(Contender::key);
        }
        Self { by_property }
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
}
