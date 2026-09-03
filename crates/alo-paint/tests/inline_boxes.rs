/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An inline box has a box of its own, and it is one box per line.
//!
//! CSS puts a `<span>`'s border and padding in a particular place: the
//! horizontal ones take room on the line, once at its start and once at its
//! end; the vertical ones draw without changing the height of the line. And a
//! `<span>` that wraps is **two rectangles**, not one rectangle with the gap
//! between the lines painted over.

use alo_box::build as build_boxes;
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_paint::{Canvas, DisplayList, PaintContext, build, render};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{Font, FontDatabase, Slant, TextMeasurer, Weight};
use alo_value::Rgba;

const WIDTH: u32 = 160;
const HEIGHT: u32 = 120;

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

fn draw(html: &str, css: &str) -> (DisplayList, Canvas) {
    let document = parse_document(html);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(css);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build_boxes(&document, &styles);

    let database = fonts();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, Size::new(160.0, 120.0), &measurer);
    let list = build::build(
        &boxes,
        &layout,
        &styles,
        PaintContext {
            fonts: &database,
            pictures: &std::collections::BTreeMap::new(),
        },
    );

    let mut canvas = Canvas::new(WIDTH, HEIGHT, Rgba::WHITE);
    render(&list, &mut canvas);
    (list, canvas)
}

/// How dark a pixel is, from nothing to one. Off the canvas reads as less than
/// nothing, so a test that asked about one fails rather than passing quietly.
fn darkness(canvas: &Canvas, x: u32, y: u32) -> f32 {
    canvas.at(x, y).map_or(-1.0, |pixel| 1.0 - pixel.red)
}

/// How many rectangles the display list fills for a box.
fn fills(list: &DisplayList, marker: &str) -> usize {
    list.to_outline()
        .lines()
        .filter(|line| line.starts_with("fill") && line.contains(marker))
        .count()
}

const SHEET: &str = "body { margin: 10px; background: #ffffff;
                            font-family: system-ui; font-size: 12px }";

#[test]
fn an_inline_boxs_padding_takes_room_on_the_line() {
    let html = "<!DOCTYPE html><html><body><p>a<span id=s>b</span>c</p></body></html>";
    let (_, without) = draw(html, &format!("{SHEET} p {{ margin: 0 }}"));
    let (_, with) = draw(
        html,
        &format!("{SHEET} p {{ margin: 0 }} #s {{ padding-left: 20px }}"),
    );

    // The "c" after the span moves right by the padding, so a column that was
    // blank in one is inked in the other.
    let ink = |canvas: &Canvas, x: u32| (0..30).map(|y| darkness(canvas, x, y)).fold(0.0, f32::max);
    let moved = (0..80)
        .filter(|x| (ink(&without, *x) > 0.3) != (ink(&with, *x) > 0.3))
        .count();
    assert!(moved > 0, "the padding pushed the rest of the line along");
}

#[test]
fn an_inline_boxs_border_is_drawn_where_its_padding_put_it() {
    let html = "<!DOCTYPE html><html><body><p><span id=s>x</span></p></body></html>";
    let (list, canvas) = draw(
        html,
        &format!(
            "{SHEET} p {{ margin: 0 }}
             #s {{ padding: 4px; background: #ffffff;
                   border-top-width: 3px; border-right-width: 3px;
                   border-bottom-width: 3px; border-left-width: 3px;
                   border-top-style: solid; border-right-style: solid;
                   border-bottom-style: solid; border-left-style: solid;
                   border-top-color: #000000; border-right-color: #000000;
                   border-bottom-color: #000000; border-left-color: #000000 }}"
        ),
    );
    assert!(
        list.to_outline()
            .lines()
            .any(|line| line.starts_with("fill")),
        "the border reaches the display list:\n{}",
        list.to_outline(),
    );
    // The left border sits at the very start of the line, ten pixels in.
    assert!(
        darkness(&canvas, 11, 18) > 0.5,
        "a border on a span is drawn:\n{}",
        list.to_outline(),
    );
}

#[test]
fn an_inline_boxs_vertical_padding_does_not_push_the_lines_apart() {
    let html = "<!DOCTYPE html><html><body><p id=p>one<span id=s>two</span></p>\
                </body></html>";
    let plain = draw(html, &format!("{SHEET} p {{ margin: 0 }}"))
        .0
        .to_outline();
    let padded = draw(
        html,
        &format!("{SHEET} p {{ margin: 0 }} #s {{ padding-top: 10px; padding-bottom: 10px }}"),
    )
    .0
    .to_outline();

    let baseline = |outline: &str| {
        outline
            .lines()
            .find(|line| line.contains("\"one\""))
            .map(ToString::to_string)
    };
    assert_eq!(
        baseline(&plain),
        baseline(&padded),
        "vertical padding on an inline box changes nothing about the line",
    );
}

#[test]
fn a_span_that_wraps_is_two_rectangles_rather_than_one_over_the_gap() {
    let html = "<!DOCTYPE html><html><body><p id=p>\
<span id=s>one two three four five six</span></p></body></html>";
    let (list, canvas) = draw(
        html,
        &format!("{SHEET} p {{ margin: 0; width: 80px }} #s {{ background: #000000 }}"),
    );
    assert_eq!(
        fills(&list, "rgb(0 0 0)"),
        3,
        "one filled rectangle per line the span is on:\n{}",
        list.to_outline(),
    );

    // Between the first line and the second, at a column past the end of the
    // first line's text, the page is still white.
    let first_line_bottom = 10 + 14;
    assert!(
        darkness(&canvas, 78, first_line_bottom) < 0.2,
        "the gap past the end of a line is not painted over",
    );
}

#[test]
fn a_span_broken_across_lines_has_a_border_at_each_end_and_not_in_the_middle() {
    let html = "<!DOCTYPE html><html><body><p id=p>\
<span id=s>one two three four five six</span></p></body></html>";
    let (list, _) = draw(
        html,
        &format!(
            "{SHEET} p {{ margin: 0; width: 80px }}
             #s {{ border-left-width: 4px; border-right-width: 4px;
                   border-left-style: solid; border-right-style: solid;
                   border-left-color: #000000; border-right-color: #000000 }}"
        ),
    );
    // Three lines, so three pieces — and exactly two vertical border strips:
    // the start of the first piece and the end of the last.
    let strips = list
        .to_outline()
        .lines()
        .filter(|line| line.starts_with("fill") && line.contains("rgb(0 0 0)"))
        .count();
    assert_eq!(strips, 2, "{}", list.to_outline());
}
