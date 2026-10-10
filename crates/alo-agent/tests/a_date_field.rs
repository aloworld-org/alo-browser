/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ★ A date field, read and filled by an agent (ADR 0042 § 4, queue item
//! 385).
//!
//! An agent reads `<input type=date>` as a `date` — the platforms' role, since
//! ARIA has none — with the date it holds in the value's own form,
//! `yyyy-mm-dd`, and an empty one as holding nothing: the format the field
//! draws is how a date is written, not a date. It puts a date in as text in
//! that form, and anything else is refused by name rather than silently
//! emptied, because an agent whose text vanished would believe it had chosen
//! a day.

use alo_agent::{AgentTree, Outcome, Refusal, Target, Verb, apply, perform};
use alo_box::{BoxTree, KnownRole, Role, build as build_boxes};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::{Document, parse_document};
use alo_layout::{LayoutTree, Size, compute};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{Font, FontDatabase, Slant, TextMeasurer, Weight};

/// The booking form's field as alo Sites writes it, and a filled one beside.
const PAGE: &str = "<!DOCTYPE html><html><body><form>\
<p><label for=day>Choose a day</label><input id=day name=date type=date required></p>\
<p><label for=held>Held</label><input id=held type=date value=2026-10-12></p>\
<p><label for=junk>Junk</label><input id=junk type=date value=12/10/2026></p>\
<p><label for=fixed>Fixed</label><input id=fixed type=date readonly></p>\
</form></body></html>";

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
    database.map_generic("sans-serif", "DejaVu Sans");
    database
}

/// The document's boxes and their layout, as a renderer would draw them.
fn drawn(document: &Document) -> (BoxTree, LayoutTree) {
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet("body { margin: 0; font: 13px sans-serif }");
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(document, &sheets, &MediaContext::default());
    let boxes = build_boxes(document, &styles);
    let database = fonts();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, Size::new(400.0, 400.0), &measurer);
    (boxes, layout)
}

/// The outline's lines with their positions taken off, which are layout's
/// to assert, not this file's.
fn read(document: &Document, boxes: &BoxTree, layout: &LayoutTree) -> Vec<String> {
    AgentTree::new(document, boxes, layout)
        .to_outline()
        .lines()
        .map(|line| line.split(" at (").next().unwrap_or(line).to_owned())
        .collect()
}

/// The date field called `name`.
fn the_day(name: &str) -> Target {
    Target::NamedOfRole {
        role: Role::Known(KnownRole::Date),
        name: name.to_owned(),
    }
}

/// Put `text` into the field called `name`, carry it into the document if
/// it was put, and answer with what the verb said.
fn put(document: &mut Document, name: &str, text: &str) -> Result<Outcome, Refusal> {
    let (boxes, layout) = drawn(document);
    let decided = perform(
        &AgentTree::new(document, &boxes, &layout),
        &the_day(name),
        &Verb::PutText(text.to_owned()),
    );
    if let Ok(outcome) = &decided {
        apply(document, &boxes, outcome);
    }
    decided
}

/// The `value` the field with this `id` holds.
fn value_of(document: &Document, id: &str) -> Option<String> {
    document
        .descendants(document.root())
        .filter_map(|node| document.element(node))
        .find(|element| element.attr("id") == Some(id))
        .and_then(|element| element.attr("value"))
        .map(str::to_owned)
}

#[test]
fn an_empty_date_field_is_a_date_with_no_value() {
    let document = parse_document(PAGE);
    let (boxes, layout) = drawn(&document);
    let outline = read(&document, &boxes, &layout);
    let at = |name: &str| {
        outline
            .iter()
            .position(|line| line.trim_start().starts_with(name))
            .unwrap_or_else(|| panic!("no {name} in {outline:#?}"))
    };
    // The booking field: a date, named by its label, required, and nothing
    // beneath it — the `yyyy-mm-dd` it draws is not read as its text.
    let day = at("date \"Choose a day\"");
    assert_eq!(
        outline[day].trim_start(),
        "date \"Choose a day\" [required]"
    );
    assert!(
        !outline[day + 1].trim_start().starts_with("text"),
        "{outline:#?}"
    );
    assert!(
        outline.iter().all(|line| !line.contains("yyyy-mm-dd")),
        "the format leaked into what an agent reads: {outline:#?}"
    );
    // A held date is read as itself, in its value's form.
    let held = at("date \"Held\"");
    assert_eq!(outline[held + 1].trim_start(), "text \"2026-10-12\"");
    // A value that is not a date is sanitised to nothing.
    let junk = at("date \"Junk\"");
    assert!(
        !outline[junk + 1].trim_start().starts_with("text"),
        "{outline:#?}"
    );
}

#[test]
fn a_valid_date_string_fills_it_and_is_read_back_in_that_form() {
    let mut document = parse_document(PAGE);
    let outcome = put(&mut document, "Choose a day", "2026-10-12");
    assert!(
        matches!(&outcome, Ok(Outcome::TextPut { text, .. }) if text == "2026-10-12"),
        "{outcome:?}"
    );
    assert_eq!(value_of(&document, "day").as_deref(), Some("2026-10-12"));
    let (boxes, layout) = drawn(&document);
    let outline = read(&document, &boxes, &layout);
    let Some(day) = outline
        .iter()
        .position(|line| line.trim_start() == "date \"Choose a day\" [required]")
    else {
        panic!("the field is still a date: {outline:#?}");
    };
    assert_eq!(outline[day + 1].trim_start(), "text \"2026-10-12\"");

    // The empty text is how a date is cleared, and the field reads empty.
    assert!(put(&mut document, "Choose a day", "").is_ok());
    assert_eq!(value_of(&document, "day").as_deref(), Some(""));
}

#[test]
fn anything_else_is_refused_by_name_and_leaves_the_field_as_it_was() {
    let mut document = parse_document(PAGE);
    let refused = put(&mut document, "Choose a day", "12/10/2026");
    let Err(refusal) = refused else {
        panic!("a day written another way was put in: {refused:?}");
    };
    assert!(
        matches!(&refusal, Refusal::NotADate { text, .. } if text == "12/10/2026"),
        "{refusal:?}"
    );
    assert!(refusal.to_string().contains("is not a date"), "{refusal}");
    assert_eq!(value_of(&document, "day"), None);

    // A held date is not emptied by a text that is not one.
    assert!(put(&mut document, "Held", "tomorrow").is_err());
    assert_eq!(value_of(&document, "held").as_deref(), Some("2026-10-12"));
}

#[test]
fn hostile_texts_are_refused_without_a_panic() {
    let mut document = parse_document(PAGE);
    let long_year = format!("{}-01-01", "9".repeat(100_000));
    for hostile in [
        "2026-02-29".to_owned(),
        "2026-13-01".to_owned(),
        "2026-04-31".to_owned(),
        "0000-01-01".to_owned(),
        "-01-01".to_owned(),
        "2026-10".to_owned(),
        "2026-10-12\u{0}".to_owned(),
        "２０２６-10-12".to_owned(),
        "-".repeat(100_000),
        format!("2026-10-{}", "1".repeat(100_000)),
    ] {
        let outcome = put(&mut document, "Choose a day", &hostile);
        assert!(
            matches!(outcome, Err(Refusal::NotADate { .. })),
            "{:?} was not refused",
            hostile.get(..40).unwrap_or(&hostile)
        );
    }
    assert_eq!(value_of(&document, "day"), None);
    // HTML puts no bound on a year's digits, so a very long one is a date.
    assert!(put(&mut document, "Choose a day", &long_year).is_ok());
    assert_eq!(value_of(&document, "day"), Some(long_year));
}

#[test]
fn a_read_only_date_field_is_refused_as_read_only_before_its_text_is_judged() {
    let mut document = parse_document(PAGE);
    let refused = put(&mut document, "Fixed", "nonsense");
    assert!(
        matches!(refused, Err(Refusal::ReadOnly { .. })),
        "{refused:?}"
    );
}
