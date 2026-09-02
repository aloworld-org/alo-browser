//! A style sheet and a document, end to end.
//!
//! Item 1 gave us a tree; item 2 gives us rules and a way to ask which of them
//! apply to which node. This is the first test in the repository that uses
//! both, and it is the shape everything after it takes: a document, a sheet, a
//! device, and an answer in terms of the tree rather than an image.
//!
//! It does **not** cascade. Which declaration wins is queue item 3; what is
//! asserted here is which rules match, and how specific each match was.

use alo_css::{ColorScheme, MatchContext, MediaContext, Specificity, parse_stylesheet};
use alo_dom::{Document, NodeId, parse_document};

const PAGE: &str = "<!DOCTYPE html>\
<html lang='en'>\
<body>\
  <main id='main'>\
    <h1 id='title' class='heading'>Invoices</h1>\
    <ul id='list' class='rows'>\
      <li id='row-1' class='row'>One</li>\
      <li id='row-2' class='row selected' aria-selected='true'>Two</li>\
      <li id='row-3' class='row'><a id='link' href='/three'>Three</a></li>\
    </ul>\
    <form id='form'>\
      <input id='name' name='name' required>\
      <input id='locked' name='locked' disabled>\
      <input id='agree' type='checkbox' checked>\
    </form>\
  </main>\
</body>\
</html>";

const SHEET: &str = "
:root { --gap: 8px }
.row { padding: var(--gap) }
.row.selected { background: #eee }
li:first-child { border-top: 0 }
#list > .row a { color: blue }
input:required { border-color: red }
input:disabled { opacity: 0.5 }
input:checked { outline: 1px solid }
li:hover { background: #fafafa }
.row::before { content: '' }
@media (prefers-color-scheme: dark) { .row { background: #101014 } }
@media (min-width: 900px) { .row { padding: 16px } }
";

/// A narrow light window: neither of the sheet's `@media` rules applies, so a
/// test about selectors is not quietly also a test about media queries.
const NARROW_LIGHT: MediaContext = MediaContext {
    width: 600.0,
    color_scheme: ColorScheme::Light,
};

/// The element with this `id` attribute.
fn node(document: &Document, wanted: &str) -> Option<NodeId> {
    document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.attr("id") == Some(wanted))
    })
}

/// Every selector in the sheet that matches this element, as written.
fn matching_selectors(wanted: &str, context: MediaContext) -> Vec<String> {
    let document = parse_document(PAGE);
    let sheet = parse_stylesheet(SHEET);
    let Some(id) = node(&document, wanted) else {
        // Reported rather than raised, so the test that asked says what it
        // wanted rather than the helper saying what it could not find.
        return vec![format!("no element with id={wanted}")];
    };
    let mut matcher = MatchContext::new(&document);
    sheet
        .style_rules_for(&context)
        .iter()
        .flat_map(|rule| rule.selectors.iter())
        .filter(|selector| matcher.matches(selector, id))
        .map(ToString::to_string)
        .collect()
}

#[test]
fn a_plain_row_matches_the_rules_written_for_it_and_no_others() {
    assert_eq!(
        matching_selectors("row-1", NARROW_LIGHT),
        vec![".row", "li:first-child"],
    );
}

#[test]
fn a_selected_row_matches_one_more() {
    assert_eq!(
        matching_selectors("row-2", NARROW_LIGHT),
        vec![".row", ".row.selected"],
    );
}

#[test]
fn a_descendant_selector_walks_the_whole_way_up() {
    assert_eq!(
        matching_selectors("link", NARROW_LIGHT),
        vec!["#list > .row a"],
    );
}

#[test]
fn state_selectors_read_the_markup_rather_than_a_guess() {
    let device = NARROW_LIGHT;
    assert_eq!(matching_selectors("name", device), vec!["input:required"]);
    assert_eq!(matching_selectors("locked", device), vec!["input:disabled"],);
    assert_eq!(matching_selectors("agree", device), vec!["input:checked"]);
}

#[test]
fn nothing_hovers_and_no_pseudo_element_is_produced() {
    for row in ["row-1", "row-2", "row-3"] {
        let matched = matching_selectors(row, NARROW_LIGHT);
        assert!(!matched.iter().any(|s| s.contains(":hover")), "{row}");
        assert!(!matched.iter().any(|s| s.contains("::before")), "{row}");
    }
}

#[test]
fn the_device_decides_which_media_rules_are_even_considered() {
    let wide_dark = MediaContext::new(1200.0, ColorScheme::Dark);

    assert_eq!(
        matching_selectors("row-1", NARROW_LIGHT),
        vec![".row", "li:first-child"],
    );
    assert_eq!(
        matching_selectors("row-1", wide_dark),
        vec![".row", "li:first-child", ".row", ".row"],
        "the dark rule and the wide rule both apply, and both are `.row`",
    );
}

#[test]
fn the_most_specific_matching_selector_is_the_one_the_cascade_will_want() {
    let document = parse_document(PAGE);
    let sheet = parse_stylesheet("h1, #title, .heading { color: red }");
    let mut matcher = MatchContext::new(&document);
    let rules = sheet.style_rules_for(&MediaContext::default());
    let rule = rules.first().expect("one rule");

    let title = node(&document, "title").expect("the heading");
    let winner = matcher
        .most_specific_match(&rule.selectors, title)
        .expect("three selectors match this element");
    assert_eq!(winner.to_string(), "#title");
    assert_eq!(
        winner.specificity(),
        Specificity {
            ids: 1,
            classes: 0,
            elements: 0,
        },
    );

    let list = node(&document, "list").expect("the list");
    assert!(
        matcher.most_specific_match(&rule.selectors, list).is_none(),
        "the list is not a heading by any of the three names",
    );
}

#[test]
fn specificity_is_reported_per_selector_not_per_rule() {
    let document = parse_document(PAGE);
    let sheet = parse_stylesheet(SHEET);
    let mut matcher = MatchContext::new(&document);
    let row = node(&document, "row-2").expect("the selected row");

    let found: Vec<(String, Specificity)> = sheet
        .style_rules_for(&NARROW_LIGHT)
        .iter()
        .flat_map(|rule| rule.selectors.iter())
        .filter(|selector| matcher.matches(selector, row))
        .map(|selector| (selector.to_string(), selector.specificity()))
        .collect();

    assert_eq!(
        found,
        vec![
            (
                ".row".to_owned(),
                Specificity {
                    ids: 0,
                    classes: 1,
                    elements: 0
                }
            ),
            (
                ".row.selected".to_owned(),
                Specificity {
                    ids: 0,
                    classes: 2,
                    elements: 0
                },
            ),
        ],
    );
}

#[test]
fn a_document_with_none_of_the_markup_matches_only_what_every_document_has() {
    let document = parse_document("<p>nothing here</p>");
    let sheet = parse_stylesheet(SHEET);
    let mut matcher = MatchContext::new(&document);
    let applying: Vec<String> = sheet
        .style_rules_for(&NARROW_LIGHT)
        .iter()
        .flat_map(|rule| rule.selectors.iter())
        .filter(|selector| {
            document
                .descendants(document.root())
                .any(|id| matcher.matches(selector, id))
        })
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        applying,
        vec![":root"],
        "every document has a root element, and nothing else in this sheet is there",
    );
}
