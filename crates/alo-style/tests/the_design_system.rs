//! A design system built from custom properties, cascaded over a real tree.
//!
//! `docs/autonomy/QUEUE.md` item 3 names four tests: specificity order,
//! inheritance through a gap, a variable defined on `:root` and used four
//! levels down, and a cycle refused rather than looped. They are all here, on
//! one document and one sheet rather than four contrived ones, because the way
//! these go wrong is in combination.

use alo_css::{ColorScheme, IssueKind, MediaContext, parse_stylesheet};
use alo_dom::{Document, NodeId, parse_document};
use alo_style::{ComputedStyle, Origin, SourcedSheet, StyleTree, resolve};
use std::borrow::Cow;

const PAGE: &str = "<!DOCTYPE html>\
<html lang='en'>\
<body>\
  <main id='main'>\
    <section id='panel'>\
      <ul id='list'>\
        <li id='row' class='row'><span id='label'>Invoice 12</span></li>\
      </ul>\
    </section>\
  </main>\
</body>\
</html>";

const SHEET: &str = "
:root {
  --ink: #101014;
  --surface: #ffffff;
  --gap: 8px;
  --gap-large: calc(var(--gap) * 2);
  color: var(--ink);
  font-family: Inter, system-ui, sans-serif;
}

@media (prefers-color-scheme: dark) {
  :root { --ink: #f4f4f5; --surface: #101014 }
}

#panel { background: var(--surface); padding: var(--gap-large) }
.row { color: red }
li.row { color: green }
#row { color: blue }
#label { margin: var(--nowhere) }
";

fn tree_for(scheme: ColorScheme) -> (Document, StyleTree) {
    let document = parse_document(PAGE);
    let sheet = parse_stylesheet(SHEET);
    let sheets = [SourcedSheet::new(Origin::Author, &sheet)];
    let tree = resolve(&document, &sheets, &MediaContext::new(1280.0, scheme));
    (document, tree)
}

fn node(document: &Document, wanted: &str) -> Option<NodeId> {
    document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.attr("id") == Some(wanted))
    })
}

/// The computed style of the element with this `id`, or an empty style if the
/// document has no such element — reported by the assertion that asked rather
/// than by this helper, which does not know what the caller wanted.
fn style<'a>(document: &Document, tree: &'a StyleTree, wanted: &str) -> Cow<'a, ComputedStyle> {
    node(document, wanted)
        .and_then(|id| tree.get(id))
        .map_or_else(|| Cow::Owned(ComputedStyle::new()), Cow::Borrowed)
}

#[test]
fn specificity_decides_between_three_rules_that_all_match() {
    let (document, tree) = tree_for(ColorScheme::Light);
    assert_eq!(
        style(&document, &tree, "row").get("color"),
        Some("blue"),
        "#row beats li.row beats .row, whatever order they were written in",
    );
}

#[test]
fn inheritance_reaches_through_elements_that_say_nothing() {
    let (document, tree) = tree_for(ColorScheme::Light);
    // `:root` sets colour; `main`, `panel` and `list` set nothing.
    for gap in ["main", "panel", "list"] {
        assert_eq!(
            style(&document, &tree, gap).get("color"),
            Some("#101014"),
            "{gap} inherits from the root through the elements above it",
        );
    }
    assert_eq!(
        style(&document, &tree, "label").get("color"),
        Some("blue"),
        "and the span inherits the row's colour rather than the root's",
    );
}

#[test]
fn a_variable_defined_on_root_is_used_four_levels_down() {
    let (document, tree) = tree_for(ColorScheme::Light);
    let label = style(&document, &tree, "label");
    assert_eq!(label.variables().get("--gap"), Some("8px"));
    assert_eq!(label.variables().get("--ink"), Some("#101014"));
    assert_eq!(
        label.variables().get("--gap-large"),
        Some("calc(8px * 2)"),
        "a variable built from another resolves once, at the root",
    );
}

#[test]
fn a_variable_that_names_another_variable_resolves_wherever_it_is_used() {
    let (document, tree) = tree_for(ColorScheme::Light);
    assert_eq!(
        style(&document, &tree, "panel").get("padding"),
        Some("calc(8px * 2)"),
    );
    assert_eq!(
        style(&document, &tree, "panel").get("background"),
        Some("#ffffff"),
    );
}

#[test]
fn the_colour_scheme_changes_every_value_the_variable_reaches() {
    let (document, tree) = tree_for(ColorScheme::Dark);
    assert_eq!(
        style(&document, &tree, "panel").get("background"),
        Some("#101014")
    );
    assert_eq!(
        style(&document, &tree, "main").get("color"),
        Some("#f4f4f5"),
        "one variable, redefined behind a media query, repaints the document",
    );
}

#[test]
fn a_reference_to_something_that_is_not_set_is_recorded_rather_than_silently_dropped() {
    let (document, tree) = tree_for(ColorScheme::Light);
    assert_eq!(
        style(&document, &tree, "label").get("margin"),
        None,
        "margin does not inherit, so an invalid declaration leaves it unset",
    );
    assert!(
        tree.issues()
            .iter()
            .any(|issue| issue.kind == IssueKind::InvalidAtComputedValueTime
                && issue.source.contains("--nowhere")),
        "and the sheet is told: {:?}",
        tree.issues()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
    );
}

#[test]
fn a_cycle_is_refused_rather_than_looped() {
    let document = parse_document(PAGE);
    let sheet = parse_stylesheet(
        ":root { --a: var(--b); --b: var(--c); --c: var(--a); --fine: 4px }\
         #row { padding: var(--a, 0); margin: var(--fine) }",
    );
    let sheets = [SourcedSheet::new(Origin::Author, &sheet)];
    let tree = resolve(&document, &sheets, &MediaContext::default());

    let row = style(&document, &tree, "row");
    for refused in ["--a", "--b", "--c"] {
        assert_eq!(
            row.variables().get(refused),
            None,
            "{refused} is in the ring"
        );
    }
    assert_eq!(row.variables().get("--fine"), Some("4px"));
    assert_eq!(
        row.get("padding"),
        Some("0"),
        "the fallback is used instead"
    );
    assert_eq!(row.get("margin"), Some("4px"));

    let cycles = tree
        .issues()
        .iter()
        .filter(|issue| issue.kind == IssueKind::VariableCycle)
        .count();
    assert!(
        cycles >= 3,
        "every property in the ring is refused, on every element that inherits it",
    );
}

#[test]
fn every_element_gets_a_style_and_the_root_is_where_the_system_starts() {
    let (document, tree) = tree_for(ColorScheme::Light);
    let elements = document
        .descendants(document.root())
        .filter(|id| document.element(*id).is_some())
        .count();
    assert_eq!(
        tree.len(),
        elements,
        "one style per element, no more, no fewer"
    );

    let root = node(&document, "main")
        .and_then(|id| document.parent(id))
        .and_then(|body| document.parent(body))
        .expect("the html element");
    assert_eq!(
        tree.get(root).and_then(|style| style.get("font-family")),
        Some("Inter, system-ui, sans-serif"),
        "a font stack survives the cascade with its commas and spaces intact",
    );
}

// --- the font in force, and the lengths that depend on it -------------------

#[test]
fn the_root_font_size_is_sixteen_pixels_until_something_says_otherwise() {
    let (document, tree) = tree_for(ColorScheme::Light);
    let style = style(&document, &tree, "main");
    assert!((style.font_size() - 16.0).abs() < 0.0001);
    assert!((style.line_height() - 19.2).abs() < 0.0001);
}

#[test]
fn em_compounds_down_the_tree_and_rem_does_not() {
    let document = parse_document(
        "<html><body><div id=outer><div id=inner><span id=deep>t</span></div></div></body></html>",
    );
    let sheet = parse_stylesheet(
        "html { font-size: 20px } #outer { font-size: 2em } #inner { font-size: 2em } \
         #deep { width: 1em; height: 1rem }",
    );
    let sheets = [SourcedSheet::new(Origin::Author, &sheet)];
    let tree = resolve(&document, &sheets, &MediaContext::default());

    assert!((style(&document, &tree, "outer").font_size() - 40.0).abs() < 0.0001);
    assert!(
        (style(&document, &tree, "inner").font_size() - 80.0).abs() < 0.0001,
        "em compounds: two of forty",
    );

    let deep = style(&document, &tree, "deep");
    assert!(
        (deep.px("width", 0.0).expect("a width") - 80.0).abs() < 0.0001,
        "1em is this element's font, which it inherited",
    );
    assert!(
        (deep.px("height", 0.0).expect("a height") - 20.0).abs() < 0.0001,
        "1rem is the root's, however deep it is",
    );
}

#[test]
fn a_length_built_from_a_variable_becomes_a_number() {
    let (document, tree) = tree_for(ColorScheme::Light);
    let panel = style(&document, &tree, "panel");
    assert_eq!(panel.get("padding"), Some("calc(8px * 2)"));
    assert!(
        (panel.px("padding", 0.0).expect("a padding") - 16.0).abs() < 0.0001,
        "the cascade substituted the variable and the value layer evaluated it",
    );
}

#[test]
fn a_percentage_stays_a_percentage_until_a_basis_is_supplied() {
    let document = parse_document("<html><body><div id=half>t</div></body></html>");
    let sheet = parse_stylesheet("#half { width: 50% }");
    let sheets = [SourcedSheet::new(Origin::Author, &sheet)];
    let tree = resolve(&document, &sheets, &MediaContext::default());
    let half = style(&document, &tree, "half");

    let width = half.length("width").expect("a width");
    assert!(width.is_percentage());
    assert!((half.px("width", 400.0).expect("a width") - 200.0).abs() < 0.0001);
    assert!((half.px("width", 800.0).expect("a width") - 400.0).abs() < 0.0001);
}

#[test]
fn a_value_the_engine_cannot_read_is_absent_rather_than_guessed_at() {
    let document = parse_document("<html><body><div id=odd>t</div></body></html>");
    let sheet = parse_stylesheet("#odd { width: auto; height: 50vw; margin: banana }");
    let sheets = [SourcedSheet::new(Origin::Author, &sheet)];
    let tree = resolve(&document, &sheets, &MediaContext::default());
    let odd = style(&document, &tree, "odd");

    assert_eq!(odd.get("width"), Some("auto"), "the text is still there");
    assert_eq!(odd.px("width", 400.0), None, "and it is not a length");
    assert_eq!(odd.px("height", 400.0), None, "nor is a unit we refuse");
    assert_eq!(odd.px("margin", 400.0), None);
}

// --- colours ----------------------------------------------------------------

#[test]
fn a_colour_from_a_variable_arrives_as_channels() {
    let (document, tree) = tree_for(ColorScheme::Light);
    let main = style(&document, &tree, "main");
    assert_eq!(
        main.current_color().to_rgba8(),
        (16, 16, 20, 255),
        "the `--ink` the sheet set, resolved and inherited",
    );

    let (document, dark) = tree_for(ColorScheme::Dark);
    assert_eq!(
        style(&document, &dark, "main").current_color().to_rgba8(),
        (244, 244, 245, 255),
        "and the dark theme's, from the same variable",
    );
}

#[test]
fn current_colour_is_whatever_colour_is_on_that_element() {
    let document =
        parse_document("<html><body><div id=outer><div id=inner>t</div></div></body></html>");
    let sheet = parse_stylesheet(
        "#outer { color: #ff0000; border-top-color: currentColor } \
         #inner { border-top-color: currentColor }",
    );
    let sheets = [SourcedSheet::new(Origin::Author, &sheet)];
    let tree = resolve(&document, &sheets, &MediaContext::default());

    assert_eq!(
        style(&document, &tree, "outer")
            .color("border-top-color")
            .map(alo_value::Rgba::to_rgba8),
        Some((255, 0, 0, 255)),
    );
    assert_eq!(
        style(&document, &tree, "inner")
            .color("border-top-color")
            .map(alo_value::Rgba::to_rgba8),
        Some((255, 0, 0, 255)),
        "the child inherited the colour, so its currentColor is the same",
    );
}

#[test]
fn a_colour_this_engine_cannot_read_leaves_the_one_that_was_inherited() {
    let document = parse_document("<html><body><p id=odd>t</p></body></html>");
    let sheet = parse_stylesheet("html { color: #112233 } #odd { color: oklch(70% 0.1 200) }");
    let sheets = [SourcedSheet::new(Origin::Author, &sheet)];
    let tree = resolve(&document, &sheets, &MediaContext::default());

    assert_eq!(
        style(&document, &tree, "odd").current_color().to_rgba8(),
        (17, 34, 51, 255),
        "a colour space we do not have is an invalid declaration, not black",
    );
}
