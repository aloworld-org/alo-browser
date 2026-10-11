/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The three things httpbin's pizza form found (queue item 131), each on the
//! smallest page that shows it: a field inside its label, a `<textarea>`, and
//! a `time` field. `alo-corpus`' `an_agent_on_somebody_elses_page.rs` drives
//! the frozen page itself.

use alo_agent::{AgentTree, Change, Outcome, Refusal, Target, Verb, apply, perform};
use alo_box::build as build_boxes;
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::{Document, parse_document};
use alo_layout::{Size, compute};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{Font, FontDatabase, Slant, TextMeasurer, Weight};

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
    database.map_generic("serif", "DejaVu Sans");
    database
}

/// Lay `$document` out and read it as `$tree`, with the user-agent sheet
/// alone, as both pages the form came from are.
macro_rules! read {
    ($document:expr, $boxes:ident, $layout:ident, $tree:ident) => {
        let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let sheets = [SourcedSheet::new(Origin::UserAgent, &agent)];
        let styles = resolve($document, &sheets, &MediaContext::default());
        let $boxes = build_boxes($document, &styles);
        let database = fonts();
        let measurer = TextMeasurer::new(&database);
        let $layout = compute(&$boxes, &styles, Size::new(400.0, 200.0), &measurer);
        let $tree = AgentTree::new($document, &$boxes, &$layout);
    };
}

/// The page read as an outline.
fn outline(document: &Document) -> String {
    read!(document, boxes, layout, tree);
    tree.to_outline()
}

/// Put `text` into whatever is called `name`, carry it into the document,
/// and say what that changed.
fn put(document: &mut Document, name: &str, text: &str) -> Result<Vec<Change>, Refusal> {
    let (outcome, boxes) = {
        read!(&*document, boxes, layout, tree);
        let outcome = perform(
            &tree,
            &Target::Named(name.to_owned()),
            &Verb::PutText(text.to_owned()),
        )?;
        (outcome, boxes)
    };
    Ok(apply(document, &boxes, &outcome))
}

#[test]
fn a_fields_text_inside_its_label_is_read_as_the_fields() {
    // The label's words name the field and are read once, as its name. The
    // field's own text is what it holds, and is read inside it.
    let document = parse_document("<p><label>Customer name: <input value='Ada'></label></p>");
    let read = outline(&document);
    assert!(
        read.contains("textbox \"Customer name:\"") && read.contains("text \"Ada\""),
        "{read}"
    );
    assert!(!read.contains("text \"Customer name:\""), "{read}");
}

#[test]
fn a_held_date_inside_its_label_is_read_too() {
    let document = parse_document("<label>Day <input type=date value=2026-10-20></label>");
    let read = outline(&document);
    assert!(read.contains("text \"2026-10-20\""), "{read}");
}

#[test]
fn text_put_into_a_textarea_is_its_content_and_is_read_back() {
    let mut document =
        parse_document("<label>Delivery instructions: <textarea></textarea></label>");
    let Ok(changes) = put(&mut document, "Delivery instructions:", "Ring twice") else {
        panic!("a textarea takes text");
    };
    assert!(
        matches!(changes.as_slice(), [Change::Content { text, .. }] if text == "Ring twice"),
        "{changes:?}"
    );
    let read = outline(&document);
    assert!(read.contains("text \"Ring twice\""), "{read}");

    // Replaced whole, and emptied by the empty text.
    assert!(
        put(
            &mut document,
            "Delivery instructions:",
            "Leave it at the door"
        )
        .is_ok()
    );
    let read = outline(&document);
    assert!(
        read.contains("text \"Leave it at the door\"") && !read.contains("Ring twice"),
        "{read}"
    );
    assert!(put(&mut document, "Delivery instructions:", "").is_ok());
    let read = outline(&document);
    assert!(!read.contains("text \""), "{read}");
}

#[test]
fn a_field_whose_text_is_not_held_yet_refuses_it_by_name() {
    for kind in ["time", "month", "week", "datetime-local", "TIME"] {
        let html = format!("<label>When <input type={kind}></label>");
        let mut document = parse_document(&html);
        let before = document.change_count();
        let refused = put(&mut document, "When", "12:15");
        assert!(
            matches!(&refused, Err(Refusal::Unbuilt { kind: said, .. })
                if said.eq_ignore_ascii_case(kind)),
            "{kind}: {refused:?}"
        );
        assert_eq!(document.change_count(), before, "{kind}: nothing changed");
    }
    // A date field is built, and still takes a date.
    let mut document = parse_document("<label>When <input type=date></label>");
    let put_in = put(&mut document, "When", "2026-10-20");
    assert!(
        matches!(&put_in, Ok(changes) if matches!(changes.as_slice(), [Change::Set { .. }])),
        "{put_in:?}"
    );
}

#[test]
fn the_record_says_the_text_was_put_where_it_now_is() {
    let document = parse_document("<label>Notes <textarea>old</textarea></label>");
    read!(&document, boxes, layout, tree);
    let outcome = perform(
        &tree,
        &Target::Named("Notes".to_owned()),
        &Verb::PutText("new".to_owned()),
    );
    assert!(
        matches!(&outcome, Ok(Outcome::TextPut { text, .. }) if text == "new"),
        "{outcome:?}"
    );
}
