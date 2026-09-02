//! The style every element ends up with.
//!
//! This is where the three halves of queue item 3 meet: the cascade says which
//! declaration wins, inheritance says what a child gets when nothing wins for
//! it, and `var()` says what a winning value actually reads. They have to
//! happen in that order and in one pass down the tree, because a child's
//! `var(--surface)` resolves against the map its parent ended up with.
//!
//! # What a computed style holds, and what absence means
//!
//! Only what was set. **A property that is not here is at its initial value**,
//! and the reader knows what that is for the property it is asking about. That
//! is not a shortcut: CSS says "nobody set this" and "somebody set this to its
//! initial value" are the same state, and an engine that carried a table of
//! initial values would have a second place for them to be wrong.
//!
//! Values are the text they were written with — `16px` is still four
//! characters — and [`ComputedStyle::px`] is how a caller gets a number out of
//! one, using the font this element ended up with.
//!
//! **`font-size` and `line-height` are the two exceptions**, and they are
//! exceptions for a reason rather than by accident: they inherit as *computed*
//! values. A child that inherited the text `2em` would resolve it again
//! against its own font, so `2em` inside `2em` would be four times the
//! grandparent rather than twice the parent. They are therefore always present
//! and always already resolved, and [`ComputedStyle::metrics`] is the same
//! answer in the form a length wants.

use crate::cascade::{Applicable, SourcedSheet};
use crate::inheritance::inherits;
use crate::keyword::{Resolution, WideKeyword};
use crate::metrics::{DEFAULT_FONT_SIZE, metrics_for, resolve_font_size, resolve_line_height};
use crate::origin::Origin;
use crate::variables::{Resolved, Variables, resolve_variables, substitute};
use alo_css::{IssueKind, Location, MatchContext, MediaContext, PropertyName, StyleIssue};
use alo_dom::{Document, NodeId};
use alo_value::{FontMetrics, LengthPercentage};
use std::collections::BTreeMap;

/// What one element ended up with.
#[derive(Debug, Clone, PartialEq)]
pub struct ComputedStyle {
    properties: BTreeMap<PropertyName, String>,
    variables: Variables,
    metrics: FontMetrics,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            properties: BTreeMap::new(),
            variables: Variables::new(),
            metrics: FontMetrics::default(),
        }
    }
}

impl ComputedStyle {
    /// A style with nothing set — every property at its initial value.
    pub fn new() -> Self {
        Self::default()
    }

    /// The value of a property, or [`None`] if it is at its initial value.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.properties
            .get(&PropertyName::parse(name))
            .map(String::as_str)
    }

    /// The custom properties in force, already resolved.
    pub fn variables(&self) -> &Variables {
        &self.variables
    }

    /// Every property that is set, by name, in a stable order.
    pub fn iter(&self) -> impl Iterator<Item = (&PropertyName, &str)> {
        self.properties.iter().map(|(name, value)| (name, &**value))
    }

    /// How many properties are set.
    pub fn len(&self) -> usize {
        self.properties.len()
    }

    /// Whether nothing at all is set.
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }

    fn set(&mut self, name: &PropertyName, value: String) {
        self.properties.insert(name.clone(), value);
    }

    /// The font in force on this element, ready to turn a length into a
    /// number.
    ///
    /// This is the answer queue item 12 exists to give: `em` and `rem` cannot
    /// be numbers until the cascade has said what the font size is, and it is
    /// resolved here, once, on the way down the tree.
    pub fn metrics(&self) -> FontMetrics {
        self.metrics
    }

    /// This element's font size, in CSS pixels.
    pub fn font_size(&self) -> f32 {
        self.metrics.font_size
    }

    /// This element's line height, in CSS pixels.
    pub fn line_height(&self) -> f32 {
        self.metrics.line_height
    }

    /// A property's value as a length or a percentage, or [`None`] when it is
    /// absent or is something else — `auto`, a keyword, a value this engine
    /// cannot read. All of those mean "at its initial value" to a caller.
    pub fn length(&self, name: &str) -> Option<LengthPercentage> {
        crate::metrics::length_of(self.get(name))
    }

    /// A property's value in CSS pixels, given what a percentage in it would
    /// be a percentage of.
    pub fn px(&self, name: &str, basis: f32) -> Option<f32> {
        Some(self.length(name)?.to_px(self.metrics, basis))
    }

    /// A property's value as a plain number — `line-height: 1.5`,
    /// `flex-grow: 2`.
    pub fn number(&self, name: &str) -> Option<f32> {
        alo_value::parse_number(self.get(name)?)
    }

    fn take_inherited_from(parent: &ComputedStyle) -> Self {
        Self {
            properties: parent
                .properties
                .iter()
                .filter(|(name, _)| inherits(name))
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect(),
            variables: parent.variables.clone(),
            // Replaced once this element's own declarations are known; until
            // then the parent's is the right answer, because font size
            // inherits.
            metrics: parent.metrics,
        }
    }
}

/// The style of every element in a document.
#[derive(Debug, Clone, Default)]
pub struct StyleTree {
    styles: BTreeMap<NodeId, ComputedStyle>,
    issues: Vec<StyleIssue>,
}

impl StyleTree {
    /// The style of one element, or [`None`] for a node that is not an element
    /// or is not in this document.
    pub fn get(&self, id: NodeId) -> Option<&ComputedStyle> {
        self.styles.get(&id)
    }

    /// Everything the cascade refused, with the text that caused it.
    pub fn issues(&self) -> &[StyleIssue] {
        &self.issues
    }

    /// How many elements were styled.
    pub fn len(&self) -> usize {
        self.styles.len()
    }

    /// Whether the document had no elements at all.
    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }
}

/// Compute the style of every element in a document.
///
/// Elements are visited in document order, so a parent's style is finished
/// before any child asks for it. That is not an optimisation — inheritance and
/// `var()` both need it, and doing it in any other order would mean resolving
/// the same element twice with different answers.
pub fn resolve(
    document: &Document,
    sheets: &[SourcedSheet<'_>],
    device: &MediaContext,
) -> StyleTree {
    let mut tree = StyleTree::default();
    let mut matcher = MatchContext::new(document);
    let root_style = ComputedStyle::new();
    // The first element styled is the root, and `rem` is relative to it — so
    // its own metrics have to be settled before any descendant asks.
    let mut root_metrics: Option<FontMetrics> = None;

    for id in document.descendants(document.root()) {
        if document.element(id).is_none() {
            continue;
        }
        let parent = document
            .parent(id)
            .and_then(|parent| tree.styles.get(&parent))
            .cloned()
            .unwrap_or_else(|| root_style.clone());

        let applicable = Applicable::gather(sheets, device, &mut matcher, id);
        let mut style = compute_one(&applicable, &parent, &mut tree.issues);
        style.metrics = resolve_metrics(&style, &parent, root_metrics);
        record_computed_font(&mut style);
        if root_metrics.is_none() {
            root_metrics = Some(style.metrics);
        }
        tree.styles.insert(id, style);
    }
    tree
}

/// Work out the font in force on an element, now that its declarations are
/// known.
///
/// The root is the case worth naming: `rem` on the root element is relative to
/// the root's *own* font size, so it is resolved against the default rather
/// than against something that does not exist yet.
fn resolve_metrics(
    style: &ComputedStyle,
    parent: &ComputedStyle,
    root: Option<FontMetrics>,
) -> FontMetrics {
    let root_font_size = root.map_or(DEFAULT_FONT_SIZE, |metrics| metrics.font_size);
    let font_size = resolve_font_size(
        style.get("font-size"),
        parent.metrics.font_size,
        root_font_size,
    );
    let line_height = resolve_line_height(style.get("line-height"), font_size, root_font_size);
    let root_line_height = root.map_or(line_height, |metrics| metrics.line_height);
    metrics_for(font_size, root_font_size, line_height, root_line_height)
}

/// Replace the font properties' specified text with what they computed to.
///
/// This is not tidying. `font-size` and `line-height` **inherit as computed
/// values**, and a child that inherited the text `2em` would resolve it again
/// against its own font — so `2em` inside `2em` would be four times the
/// grandparent rather than twice the parent. Writing the computed value back
/// is what makes inheritance mean what CSS says it means.
///
/// `line-height` keeps its number when it was written as one, because a number
/// is what it computes to: a child with a larger font then gets a
/// proportionally larger line, which is the whole reason to write
/// `line-height: 1.5` rather than `line-height: 24px`.
fn record_computed_font(style: &mut ComputedStyle) {
    let font_size = PropertyName::parse("font-size");
    style
        .properties
        .insert(font_size, format!("{}px", style.metrics.font_size));

    let line_height = PropertyName::parse("line-height");
    let computed = match style.properties.get(&line_height).map(String::as_str) {
        Some(text) if alo_value::parse_number(text).is_some_and(|number| number >= 0.0) => {
            text.trim().to_owned()
        }
        Some(text) if !text.trim().is_empty() && !text.eq_ignore_ascii_case("normal") => {
            format!("{}px", style.metrics.line_height)
        }
        // Nothing said anything, so it stays `normal` — which is a computed
        // value in its own right, and one that means something different at
        // every font size it is inherited into.
        _ => "normal".to_owned(),
    };
    style.properties.insert(line_height, computed);
}

/// Turn what applied to one element into what it ends up with.
///
/// The parent's **whole** style is needed, not only the part of it that
/// inherits: `margin: inherit` asks for the parent's margin, and margin is
/// exactly a property that would not have arrived on its own.
fn compute_one(
    applicable: &Applicable<'_>,
    parent: &ComputedStyle,
    issues: &mut Vec<StyleIssue>,
) -> ComputedStyle {
    let mut style = ComputedStyle::take_inherited_from(parent);
    let inherited_variables = style.variables.clone();

    // Custom properties first, and all of them together: a declaration may use
    // another declared on the same element, so they resolve as a group rather
    // than in the order the cascade happened to visit them.
    let mut declared: Vec<(Box<str>, String)> = Vec::new();
    let mut at = Location { line: 1, column: 1 };
    for name in applicable.properties() {
        if !name.is_custom() {
            continue;
        }
        if let Some(contender) = winning_contender(applicable, name) {
            at = contender.at;
            declared.push((name.as_str().into(), contender.value));
        }
    }
    style.variables = resolve_variables(&inherited_variables, &declared, at, issues);
    for (name, value) in style.variables.iter() {
        style
            .properties
            .insert(PropertyName::Custom(name.into()), value.to_owned());
    }
    // A custom property that was refused — a cycle — must not keep the value
    // it inherited under a name the cascade has just invalidated.
    let live: Vec<PropertyName> = style
        .properties
        .keys()
        .filter(|name| name.is_custom() && style.variables.get(name.as_str()).is_none())
        .cloned()
        .collect();
    for name in live {
        style.properties.remove(&name);
    }

    for name in applicable.properties() {
        if name.is_custom() {
            continue;
        }
        let Some(contender) = winning_contender(applicable, name) else {
            continue;
        };
        apply_one(&mut style, parent, name, &contender, issues);
    }
    style
}

/// The value that won for a property, with `revert` already followed back
/// through the origins.
struct Winner {
    value: String,
    at: Location,
}

fn winning_contender(applicable: &Applicable<'_>, name: &PropertyName) -> Option<Winner> {
    let mut contender = applicable.winner(name)?;
    // `revert` may itself be reverted, and there are only ever as many origins
    // as there are, so this cannot run away.
    let mut reverted: Vec<Origin> = Vec::new();
    while WideKeyword::parse(&contender.declaration.value) == Some(WideKeyword::Revert) {
        let origin = contender.origin;
        if reverted.contains(&origin) {
            break;
        }
        reverted.push(origin);
        match applicable.winner_without(name, origin) {
            Some(earlier) => contender = earlier,
            // No earlier origin said anything, so `revert` is `unset`.
            None => {
                return Some(Winner {
                    value: WideKeyword::Unset.as_str().to_owned(),
                    at: contender.at,
                });
            }
        }
    }
    Some(Winner {
        value: contender.declaration.value.clone(),
        at: contender.at,
    })
}

/// Put one property's winning value into the style, following whatever the
/// value asks for.
fn apply_one(
    style: &mut ComputedStyle,
    parent: &ComputedStyle,
    name: &PropertyName,
    winner: &Winner,
    issues: &mut Vec<StyleIssue>,
) {
    let property_inherits = inherits(name);

    if let Some(keyword) = WideKeyword::parse(&winner.value) {
        match keyword.resolve(property_inherits) {
            // `revert` reaches here only when no earlier origin said anything,
            // which the specification makes the same as `unset`.
            Resolution::FromParent | Resolution::PreviousOrigin => {
                take_from_parent(style, parent, name);
            }
            Resolution::Initial => {
                style.properties.remove(name);
            }
        }
        return;
    }

    match substitute(&winner.value, &style.variables) {
        Resolved::Value(value) => style.set(name, value),
        Resolved::InvalidAtComputedValueTime => {
            issues.push(StyleIssue {
                kind: IssueKind::InvalidAtComputedValueTime,
                source: format!("{name}: {}", winner.value),
                at: winner.at,
            });
            // CSS says such a declaration behaves as `unset`.
            if property_inherits {
                take_from_parent(style, parent, name);
            } else {
                style.properties.remove(name);
            }
        }
    }
}

/// Take whatever the parent ended up with for a property, which for a property
/// the parent did not set means taking nothing.
fn take_from_parent(style: &mut ComputedStyle, parent: &ComputedStyle, name: &PropertyName) {
    match parent.properties.get(name) {
        Some(value) => style.set(name, value.clone()),
        None => {
            style.properties.remove(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_css::parse_stylesheet;
    use alo_dom::parse_document;

    /// The computed style of the element with this `id` attribute.
    fn style_of(html: &str, css: &str, wanted: &str) -> ComputedStyle {
        let document = parse_document(html);
        let sheet = parse_stylesheet(css);
        let sheets = [SourcedSheet::new(Origin::Author, &sheet)];
        let tree = resolve(&document, &sheets, &MediaContext::default());
        let id = document
            .descendants(document.root())
            .find(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some(wanted))
            })
            .unwrap_or_else(|| panic!("no element with id={wanted}"));
        tree.get(id).cloned().unwrap_or_default()
    }

    #[test]
    fn a_property_nobody_set_is_absent_which_is_its_initial_value() {
        let style = style_of("<p id=x>t</p>", "p { color: red }", "x");
        assert_eq!(style.get("color"), Some("red"));
        assert_eq!(style.get("margin"), None);
        assert!(!style.is_empty());
        assert!(ComputedStyle::new().is_empty());
    }

    #[test]
    fn the_font_is_always_there_because_it_inherits_as_a_number() {
        let style = style_of("<p id=x>t</p>", "p { color: red }", "x");
        assert_eq!(
            style.get("font-size"),
            Some("16px"),
            "resolved, not the text somebody wrote — see the module docs",
        );
        assert_eq!(style.get("line-height"), Some("normal"));
        assert!((style.font_size() - 16.0).abs() < 0.0001);
        assert!((style.line_height() - 19.2).abs() < 0.0001);
    }

    #[test]
    fn a_line_height_written_as_a_number_stays_a_number_so_it_scales() {
        let style = style_of(
            "<div id=parent><p id=child>t</p></div>",
            "#parent { line-height: 1.5 } #child { font-size: 40px }",
            "child",
        );
        assert_eq!(style.get("line-height"), Some("1.5"));
        assert!(
            (style.line_height() - 60.0).abs() < 0.0001,
            "one and a half of the child's own font, not of the parent's",
        );
    }

    #[test]
    fn a_line_height_written_as_a_length_inherits_as_that_length() {
        let style = style_of(
            "<div id=parent><p id=child>t</p></div>",
            "#parent { line-height: 24px } #child { font-size: 40px }",
            "child",
        );
        assert_eq!(style.get("line-height"), Some("24px"));
        assert!((style.line_height() - 24.0).abs() < 0.0001);
    }

    #[test]
    fn an_inherited_property_reaches_a_child_that_does_not_set_it() {
        let style = style_of(
            "<div id=parent><p id=child>t</p></div>",
            "#parent { color: red; margin: 10px }",
            "child",
        );
        assert_eq!(style.get("color"), Some("red"));
        assert_eq!(style.get("margin"), None, "margin does not inherit");
    }

    #[test]
    fn inheritance_passes_through_an_element_that_says_nothing() {
        let style = style_of(
            "<div id=top><section><article><p id=deep>t</p></article></section></div>",
            "#top { color: red }",
            "deep",
        );
        assert_eq!(style.get("color"), Some("red"));
    }

    #[test]
    fn a_child_that_sets_the_property_overrides_what_it_inherited() {
        let style = style_of(
            "<div id=parent><p id=child>t</p></div>",
            "#parent { color: red } #child { color: blue }",
            "child",
        );
        assert_eq!(style.get("color"), Some("blue"));
    }

    #[test]
    fn inherit_takes_the_parents_value_for_a_property_that_would_not() {
        let style = style_of(
            "<div id=parent><p id=child>t</p></div>",
            "#parent { margin: 10px } #child { margin: inherit }",
            "child",
        );
        assert_eq!(style.get("margin"), Some("10px"));
    }

    #[test]
    fn initial_removes_what_was_inherited() {
        let style = style_of(
            "<div id=parent><p id=child>t</p></div>",
            "#parent { color: red } #child { color: initial }",
            "child",
        );
        assert_eq!(style.get("color"), None);
    }

    #[test]
    fn unset_is_inherit_for_one_kind_of_property_and_initial_for_the_other() {
        let inherited = style_of(
            "<div id=parent><p id=child>t</p></div>",
            "#parent { color: red } #child { color: green } #child { color: unset }",
            "child",
        );
        assert_eq!(inherited.get("color"), Some("red"));

        let not_inherited = style_of(
            "<div id=parent><p id=child>t</p></div>",
            "#parent { margin: 10px } #child { margin: 2px } #child { margin: unset }",
            "child",
        );
        assert_eq!(not_inherited.get("margin"), None);
    }

    #[test]
    fn a_variable_on_root_reaches_four_levels_down() {
        let style = style_of(
            "<html><body><main><section><p id=deep>t</p></section></main></body></html>",
            ":root { --ink: #101014 } #deep { color: var(--ink) }",
            "deep",
        );
        assert_eq!(style.get("color"), Some("#101014"));
        assert_eq!(style.variables().get("--ink"), Some("#101014"));
    }

    #[test]
    fn a_variable_can_be_redefined_part_way_down_and_only_affects_below() {
        let css = ":root { --ink: black } #middle { --ink: blue }";
        let html = "<html><body><div id=above><span id=sibling></span></div>\
             <div id=middle><span id=below></span></div></body></html>";
        assert_eq!(
            style_of(html, css, "sibling").variables().get("--ink"),
            Some("black"),
        );
        assert_eq!(
            style_of(html, css, "below").variables().get("--ink"),
            Some("blue"),
        );
    }

    #[test]
    fn a_variable_cycle_is_refused_and_leaves_nothing_behind() {
        let document = parse_document("<p id=x>t</p>");
        let sheet = parse_stylesheet("p { --a: var(--b); --b: var(--a); color: var(--a, red) }");
        let sheets = [SourcedSheet::new(Origin::Author, &sheet)];
        let tree = resolve(&document, &sheets, &MediaContext::default());
        let id = document
            .descendants(document.root())
            .find(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some("x"))
            })
            .expect("the paragraph");
        let style = tree.get(id).expect("a style");

        assert_eq!(style.variables().get("--a"), None);
        assert_eq!(style.get("--a"), None);
        assert_eq!(
            style.get("color"),
            Some("red"),
            "a reference to a refused property falls back",
        );
        assert!(
            tree.issues()
                .iter()
                .any(|issue| issue.kind == IssueKind::VariableCycle),
        );
    }

    #[test]
    fn a_reference_to_nothing_with_no_fallback_is_recorded_and_the_property_is_unset() {
        let document = parse_document("<div id=p><span id=c></span></div>");
        let sheet = parse_stylesheet("#p { color: red } #c { color: var(--nowhere) }");
        let sheets = [SourcedSheet::new(Origin::Author, &sheet)];
        let tree = resolve(&document, &sheets, &MediaContext::default());
        let child = document
            .descendants(document.root())
            .find(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some("c"))
            })
            .expect("the span");

        assert_eq!(
            tree.get(child).and_then(|style| style.get("color")),
            Some("red"),
            "unset on an inherited property is the parent's value",
        );
        assert!(
            tree.issues()
                .iter()
                .any(|issue| issue.kind == IssueKind::InvalidAtComputedValueTime),
        );
    }

    #[test]
    fn a_variable_used_inside_a_function_keeps_the_rest_of_the_value() {
        let style = style_of(
            "<p id=x>t</p>",
            ":root { --gap: 8px } p { padding: calc(var(--gap) * 2) }",
            "x",
        );
        assert_eq!(style.get("padding"), Some("calc(8px * 2)"));
    }

    #[test]
    fn a_document_with_no_style_sheet_still_produces_a_style_for_every_element() {
        let document = parse_document("<div><p>a</p><p>b</p></div>");
        let tree = resolve(&document, &[], &MediaContext::default());
        assert_eq!(
            tree.len(),
            6,
            "html, head, body, the div, and two paragraphs"
        );
        assert!(!tree.is_empty());
        assert!(tree.issues().is_empty());
        assert!(
            tree.styles.values().all(|style| {
                style
                    .iter()
                    .all(|(name, _)| matches!(name.as_str(), "font-size" | "line-height"))
            }),
            "and every one of them holds only the font, which is always resolved",
        );
    }

    #[test]
    fn a_style_lists_what_it_holds_in_a_stable_order() {
        let style = style_of(
            "<p id=x>t</p>",
            "p { margin: 0; color: red; --gap: 8px }",
            "x",
        );
        assert_eq!(
            style
                .iter()
                .map(|(name, value)| format!("{name}: {value}"))
                .collect::<Vec<_>>(),
            vec![
                "--gap: 8px",
                "color: red",
                "font-size: 16px",
                "line-height: normal",
                "margin: 0",
            ],
        );
    }
}
