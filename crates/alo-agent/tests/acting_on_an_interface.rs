//! ★ Acting on an interface by name, and being refused by reason.
//!
//! ADR 0002: *"activate this element, put this text in that field, scroll this
//! list. **No verb takes a coordinate**."* This test does all three on a page
//! the engine rendered, and then does the things that should be refused —
//! because a verb surface is only as good as what it declines.

use alo_agent::{AgentTree, Outcome, Refusal, ScrollBy, Target, Verb, perform};
use alo_box::{KnownRole, Role, build as build_boxes};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{Font, FontDatabase, Slant, TextMeasurer, Weight};

const PAGE: &str = "<!DOCTYPE html><html><body><main>\
<h1>Invoices</h1>\
<a id=link href='/invoices/12'>Open invoice 12</a>\
<ul id=list>\
<li>Invoice 11</li><li>Invoice 12</li><li>Invoice 13</li>\
<li>Invoice 14</li><li>Invoice 15</li><li>Invoice 16</li>\
</ul>\
<form>\
<label for='amount'>Amount</label><input id='amount'>\
<label for='locked'>Reference</label><input id='locked' readonly>\
<button id='save'>Save</button>\
<button id='cancel' disabled>Cancel</button>\
</form>\
<p id=prose>Nothing to press here.</p>\
</main></body></html>";

const SHEET: &str = "body { margin: 0; font-family: system-ui; font-size: 13px }
main { padding: 4px } h1 { font-size: 18px; margin: 0 }
#list { margin: 0; padding: 0; height: 40px; overflow: auto }
li { padding: 2px }";

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    if let Some(font) = Font::load(
        "DejaVu Sans",
        Weight::NORMAL,
        Slant::Normal,
        dejavu::sans::regular().to_vec(),
    ) {
        database.add(font);
    }
    database.map_generic("system-ui", "DejaVu Sans");
    database
}

macro_rules! read {
    ($document:ident, $boxes:ident, $layout:ident, $tree:ident) => {
        let $document = parse_document(PAGE);
        let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let author = parse_stylesheet(SHEET);
        let sheets = [
            SourcedSheet::new(Origin::UserAgent, &agent),
            SourcedSheet::new(Origin::Author, &author),
        ];
        let styles = resolve(&$document, &sheets, &MediaContext::default());
        let $boxes = build_boxes(&$document, &styles);
        let database = fonts();
        let measurer = TextMeasurer::new(&database);
        let $layout = compute(&$boxes, &styles, Size::new(220.0, 220.0), &measurer);
        let $tree = AgentTree::new(&$document, &$boxes, &$layout);
    };
}

#[test]
fn a_button_is_pressed_by_the_name_written_on_it() {
    read!(document, boxes, layout, tree);
    let outcome = perform(&tree, &Target::Named("Save".to_owned()), &Verb::Activate)
        .expect("a button called Save");
    match outcome {
        Outcome::Activated { name, .. } => assert_eq!(name.as_deref(), Some("Save")),
        other => panic!("expected an activation, got {other}"),
    }
}

#[test]
fn following_a_link_says_where_it_goes() {
    read!(document, boxes, layout, tree);
    let outcome = perform(
        &tree,
        &Target::Named("Open invoice 12".to_owned()),
        &Verb::Activate,
    )
    .expect("the link");
    match outcome {
        Outcome::Followed { to, .. } => assert_eq!(to, "/invoices/12"),
        other => panic!("expected a link to be followed, got {other}"),
    }
}

#[test]
fn text_goes_into_the_field_the_label_names() {
    read!(document, boxes, layout, tree);
    let outcome = perform(
        &tree,
        &Target::NamedOfRole {
            role: Role::Known(KnownRole::TextBox),
            name: "Amount".to_owned(),
        },
        &Verb::PutText("12.50".to_owned()),
    )
    .expect("the amount field");
    match outcome {
        Outcome::TextPut { text, .. } => assert_eq!(text, "12.50"),
        other => panic!("expected text to be put, got {other}"),
    }
}

#[test]
fn a_list_with_more_rows_than_room_can_be_scrolled() {
    read!(document, boxes, layout, tree);
    let list = tree
        .with_role(&Role::Known(KnownRole::List))
        .into_iter()
        .next()
        .expect("the list");
    assert!(list.scrolls(), "six rows in forty pixels overflow");

    let outcome = perform(
        &tree,
        &Target::Node(list.id()),
        &Verb::Scroll(ScrollBy::Pixels(40.0)),
    )
    .expect("a list that scrolls");
    assert!(matches!(outcome, Outcome::Scrolled { .. }));
}

// --- the refusals -----------------------------------------------------------

#[test]
fn something_that_is_not_on_the_page_is_refused_rather_than_guessed_at() {
    read!(document, boxes, layout, tree);
    let refusal = perform(
        &tree,
        &Target::Named("Delete everything".to_owned()),
        &Verb::Activate,
    )
    .expect_err("nothing is called that");
    assert!(matches!(refusal, Refusal::NotFound { .. }));
    assert!(refusal.to_string().contains("nothing on the page"));
}

#[test]
fn two_things_of_the_same_kind_are_refused_with_both_of_them_named() {
    read!(document, boxes, layout, tree);
    let refusal = perform(
        &tree,
        &Target::OfRole(Role::Known(KnownRole::Button)),
        &Verb::Activate,
    )
    .expect_err("there are two buttons");
    match refusal {
        Refusal::Ambiguous { candidates, .. } => {
            assert_eq!(candidates.len(), 2, "Save and Cancel");
        }
        other => panic!("expected an ambiguity, got {other}"),
    }
}

#[test]
fn narrowing_an_ambiguous_request_by_name_resolves_it() {
    read!(document, boxes, layout, tree);
    let outcome = perform(
        &tree,
        &Target::NamedOfRole {
            role: Role::Known(KnownRole::Button),
            name: "Save".to_owned(),
        },
        &Verb::Activate,
    )
    .expect("one button called Save");
    assert!(matches!(outcome, Outcome::Activated { .. }));
}

#[test]
fn a_disabled_control_is_not_operated_even_though_nothing_prevents_it() {
    read!(document, boxes, layout, tree);
    let refusal = perform(&tree, &Target::Named("Cancel".to_owned()), &Verb::Activate)
        .expect_err("Cancel is disabled");
    assert!(matches!(refusal, Refusal::Disabled { .. }));
    assert!(refusal.to_string().contains("disabled"));
}

#[test]
fn a_paragraph_is_not_a_thing_to_press() {
    read!(document, boxes, layout, tree);
    let refusal = perform(
        &tree,
        &Target::OfRole(Role::Known(KnownRole::Paragraph)),
        &Verb::Activate,
    )
    .expect_err("a paragraph is not operable");
    assert!(matches!(refusal, Refusal::NotOperable { .. }));
}

#[test]
fn text_does_not_go_into_a_button() {
    read!(document, boxes, layout, tree);
    let refusal = perform(
        &tree,
        &Target::Named("Save".to_owned()),
        &Verb::PutText("hello".to_owned()),
    )
    .expect_err("a button is not a field");
    assert!(matches!(refusal, Refusal::NotAField { .. }));
}

#[test]
fn text_does_not_go_into_a_field_that_cannot_be_typed_into() {
    read!(document, boxes, layout, tree);
    let refusal = perform(
        &tree,
        &Target::NamedOfRole {
            role: Role::Known(KnownRole::TextBox),
            name: "Reference".to_owned(),
        },
        &Verb::PutText("hello".to_owned()),
    )
    .expect_err("the reference field is read-only");
    assert!(matches!(refusal, Refusal::ReadOnly { .. }));
}

#[test]
fn something_with_nothing_to_scroll_is_not_scrolled() {
    read!(document, boxes, layout, tree);
    let heading = tree
        .with_role(&Role::Known(KnownRole::Heading))
        .into_iter()
        .next()
        .expect("the heading");
    let refusal = perform(
        &tree,
        &Target::Node(heading.id()),
        &Verb::Scroll(ScrollBy::ToEnd),
    )
    .expect_err("a heading does not scroll");
    assert!(matches!(refusal, Refusal::DoesNotScroll { .. }));
}

#[test]
fn a_node_that_is_no_longer_there_is_refused_rather_than_hitting_another_one() {
    read!(document, boxes, layout, tree);
    // An id far beyond anything this page made. ADR 0003: ids are never
    // reused, so a stale one names nothing rather than something else.
    let stale = alo_box::BoxId::from_index_for_tests(10_000);
    let refusal =
        perform(&tree, &Target::Node(stale), &Verb::Activate).expect_err("that box does not exist");
    assert!(matches!(refusal, Refusal::NotFound { .. }));
}

#[test]
fn every_outcome_is_a_record_of_what_was_asked_for() {
    read!(document, boxes, layout, tree);
    let outcome =
        perform(&tree, &Target::Named("Save".to_owned()), &Verb::Activate).expect("the button");
    let record = outcome.to_string();
    assert!(record.starts_with("activated box#"), "{record}");
    assert!(
        record.contains("\"Save\""),
        "a record says what was operated, not only which box: {record}",
    );
}
