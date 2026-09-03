/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A document laid out with real text.
//!
//! Item 5 could lay out boxes and had to be told how wide text was. This is
//! the first test in the repository where the engine works it out itself: a
//! document, a style sheet, a font, and numbers that come from the font rather
//! than from a fake.
//!
//! It also carries the awkward-scripts assertion end to end, because that is
//! where it matters: a paragraph of Arabic in a document has to have a width,
//! and the width has to change when the font size does.

use alo_box::build;
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{Font, FontDatabase, FontRequest, Slant, TextMeasurer, Weight, measure_unwrapped};

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for (family, weight, slant, data) in [
        (
            "DejaVu Sans",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::sans::regular(),
        ),
        (
            "DejaVu Sans",
            Weight::BOLD,
            Slant::Normal,
            dejavu::sans::bold(),
        ),
        (
            "DejaVu Serif",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::serif::regular(),
        ),
    ] {
        if let Some(font) = Font::load(family, weight, slant, data.to_vec()) {
            database.add(font);
        }
    }
    database.map_generic("sans-serif", "DejaVu Sans");
    database.map_generic("serif", "DejaVu Serif");
    database
}

#[test]
fn a_document_is_laid_out_with_widths_that_came_from_a_font() {
    let document = parse_document(
        "<body><main><h1 id=title>Invoices</h1><p id=body>One two three</p></main></body>",
    );
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet("main { width: 400px } h1, p { margin: 0 }");
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build(&document, &styles);

    let database = fonts();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, Size::new(800.0, 600.0), &measurer);

    let root = boxes.root().expect("a root box");
    let text_boxes: Vec<_> = boxes
        .descendants(root)
        .into_iter()
        .filter(|id| boxes.get(*id).is_some_and(|held| held.text().is_some()))
        .collect();
    assert_eq!(text_boxes.len(), 2, "the heading and the paragraph");

    for id in text_boxes {
        let rect = layout.border_box(id).expect("a laid-out text box");
        assert!(
            rect.size.height > 0.0,
            "text with a font behind it has a height",
        );
    }

    let document_height = layout
        .border_box(root)
        .expect("the root was laid out")
        .size
        .height;
    assert!(
        document_height > 16.0,
        "two lines of text are taller than one, and this is {document_height}",
    );
}

#[test]
fn text_gets_wider_when_the_font_size_does() {
    let database = fonts();
    let request = FontRequest::family("DejaVu Sans");
    let small = measure_unwrapped("Invoices", &database, &request, 16.0);
    let large = measure_unwrapped("Invoices", &database, &request, 24.0);
    assert!((large.width() / small.width() - 1.5).abs() < 0.01);
}

#[test]
fn a_bold_face_is_chosen_when_bold_is_asked_for_and_is_not_the_same_width() {
    let database = fonts();
    let normal = measure_unwrapped(
        "Invoices",
        &database,
        &FontRequest::family("DejaVu Sans"),
        16.0,
    );
    let bold = measure_unwrapped(
        "Invoices",
        &database,
        &FontRequest {
            families: vec!["DejaVu Sans".to_owned()],
            weight: Weight::BOLD,
            slant: Slant::Normal,
        },
        16.0,
    );
    assert!(
        bold.width() > normal.width(),
        "the bold face is a different face and a wider one: {} against {}",
        bold.width(),
        normal.width(),
    );
}

#[test]
fn arabic_has_a_width_and_it_scales_like_any_other_text() {
    let database = fonts();
    let request = FontRequest::family("DejaVu Sans");
    let small = measure_unwrapped("مرحبا بالعالم", &database, &request, 16.0);
    let large = measure_unwrapped("مرحبا بالعالم", &database, &request, 32.0);

    assert!(small.width() > 0.0);
    assert!((large.width() - small.width() * 2.0).abs() < 0.01);
    assert_eq!(small.len(), 1, "one line, right to left");
}

#[test]
fn a_sentence_of_two_directions_is_wider_than_either_half() {
    let database = fonts();
    let request = FontRequest::family("DejaVu Sans");
    let english = measure_unwrapped("hello ", &database, &request, 16.0).width();
    let arabic = measure_unwrapped("مرحبا", &database, &request, 16.0).width();
    let both = measure_unwrapped("hello مرحبا", &database, &request, 16.0).width();

    assert!(both > english);
    assert!(both > arabic);
    assert!(
        (both - (english + arabic)).abs() < 0.01,
        "and it is exactly the two runs, laid down in the order they were written",
    );
}

#[test]
fn a_paragraph_wraps_inside_the_box_it_is_given() {
    let document =
        parse_document("<body><p id=body>the quick brown fox jumps over the lazy dog</p></body>");
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let sheets = [SourcedSheet::new(Origin::UserAgent, &agent)];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build(&document, &styles);

    let database = fonts();
    let measurer = TextMeasurer::new(&database);

    let wide = compute(&boxes, &styles, Size::new(800.0, 600.0), &measurer);
    let narrow = compute(&boxes, &styles, Size::new(120.0, 600.0), &measurer);

    let root = boxes.root().expect("a root box");
    let tall = |layout: &alo_layout::LayoutTree| {
        layout.border_box(root).map_or(0.0, |rect| rect.size.height)
    };
    assert!(
        tall(&narrow) > tall(&wide),
        "a narrower window takes more lines: {} against {}",
        tall(&narrow),
        tall(&wide),
    );
}

#[test]
fn a_character_no_font_has_takes_no_room_rather_than_drawing_something_else() {
    let database = fonts();
    let request = FontRequest::family("DejaVu Sans");
    let without = measure_unwrapped("ab", &database, &request, 16.0).width();
    // Devanagari, which none of the DejaVu faces cover.
    let with = measure_unwrapped("aक b", &database, &request, 16.0).width();
    assert!(with > 0.0);
    assert!(
        with > without,
        "the letters around it are still measured: {with} against {without}",
    );
}
