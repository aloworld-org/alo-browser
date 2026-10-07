/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A background of several layers, in pixels (queue item 313).
//!
//! `cases/background-layers` pins the picture. What is here is what a picture
//! can only differ about: the colour is beneath every layer, the first layer
//! written is on top, a layer that cannot be drawn is recorded once per box
//! while the others are drawn, and a list that cannot be read draws nothing
//! and says so.

use alo_box::build as build_boxes;
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_paint::{Canvas, DisplayList, PaintContext, build, render};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{Font, FontDatabase, Slant, TextMeasurer, Weight};
use alo_value::Rgba;

const WIDTH: u32 = 140;
const HEIGHT: u32 = 100;

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
    let layout = compute(&boxes, &styles, Size::new(140.0, 100.0), &measurer);
    let list = build::build(
        &boxes,
        &layout,
        &styles,
        PaintContext {
            fonts: &database,
            pictures: &std::collections::BTreeMap::new(),
            drawings: &std::collections::BTreeMap::new(),
        },
    );

    let mut canvas = Canvas::new(WIDTH, HEIGHT, Rgba::WHITE);
    render(&list, &mut canvas);
    (list, canvas)
}

/// The pixel at `(x, y)`, as bytes.
fn pixel(canvas: &Canvas, x: u32, y: u32) -> (u8, u8, u8) {
    let (red, green, blue, _) = canvas.at(x, y).map_or((0, 0, 0, 0), Rgba::to_rgba8);
    (red, green, blue)
}

/// Each channel within one of what was wanted.
fn near(found: (u8, u8, u8), wanted: (u8, u8, u8)) -> bool {
    found.0.abs_diff(wanted.0) <= 1
        && found.1.abs_diff(wanted.1) <= 1
        && found.2.abs_diff(wanted.2) <= 1
}

/// One 100 × 40 box at (20, 20), with `background` set to `layers`.
fn one_box(layers: &str) -> (DisplayList, Canvas) {
    draw(
        "<!DOCTYPE html><html><body><div id=box></div></body></html>",
        &format!(
            "body {{ margin: 20px; background: #ffffff }}
             #box {{ width: 100px; height: 40px; background: {layers} }}"
        ),
    )
}

#[test]
fn a_gradient_is_painted_over_the_colour_beneath_it() {
    let (_, canvas) = one_box("linear-gradient(to right, #000000, transparent), #ff0000");
    assert!(near(pixel(&canvas, 20, 40), (0, 0, 0)), "black on the left");
    assert!(
        near(pixel(&canvas, 119, 40), (255, 0, 0)),
        "the red beneath on the right"
    );
    // Half way: half black over red.
    let middle = pixel(&canvas, 70, 40);
    assert!(
        near(middle, (127, 0, 0)) || near(middle, (128, 0, 0)),
        "{middle:?}"
    );
}

#[test]
fn the_first_layer_written_is_on_top() {
    let (_, canvas) =
        one_box("linear-gradient(#0000ff, #0000ff), linear-gradient(#00ff00, #00ff00)");
    assert!(near(pixel(&canvas, 70, 40), (0, 0, 255)), "blue over green");

    let (_, canvas) = one_box(
        "linear-gradient(to right, #0000ff, transparent), linear-gradient(#00ff00, #00ff00)",
    );
    assert!(
        near(pixel(&canvas, 20, 40), (0, 0, 255)),
        "blue where it is opaque"
    );
    assert!(
        near(pixel(&canvas, 119, 40), (0, 255, 0)),
        "green where it is not"
    );
}

#[test]
fn a_picture_is_recorded_and_the_layers_around_it_are_drawn() {
    let (list, canvas) = one_box("linear-gradient(#0000ff, #0000ff), url(a.png), #ff0000");
    assert!(near(pixel(&canvas, 70, 40), (0, 0, 255)));
    let said: Vec<String> = list.issues().iter().map(ToString::to_string).collect();
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said.iter()
            .all(|line| line.contains("layer not drawn") && line.contains("url(a.png)"))
    );
}

#[test]
fn a_list_that_cannot_be_read_draws_nothing_and_says_so() {
    let (list, canvas) = one_box("linear-gradient(#0000ff, #0000ff) repeat-x, #ff0000");
    assert!(
        near(pixel(&canvas, 70, 40), (255, 255, 255)),
        "the page shows through"
    );
    let said: Vec<String> = list.issues().iter().map(ToString::to_string).collect();
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said.iter().all(|line| line.contains("repeat-x")));
}

#[test]
fn a_box_in_several_pieces_is_recorded_once() {
    let (list, _) = draw(
        "<!DOCTYPE html><html><body><p><span>one two three four five six seven eight \
         nine ten eleven twelve</span></p></body></html>",
        "body { margin: 0; font-family: system-ui; font-size: 13px }
         p { width: 60px }
         span { background: url(a.png), #ff0000 }",
    );
    let fills = list
        .to_outline()
        .lines()
        .filter(|line| line.starts_with("fill") && line.contains("rgb(255 0 0)"))
        .count();
    assert!(
        fills > 1,
        "the span is in several pieces:\n{}",
        list.to_outline()
    );
    assert_eq!(list.issues().len(), 1, "and its picture is recorded once");
}
