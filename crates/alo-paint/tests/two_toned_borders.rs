/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `inset`, `outset`, `groove` and `ridge`, in pixels.
//!
//! The committed picture is `alo-corpus`'s `border-styles`. What is here is
//! what a picture can only differ about: **which** side is the dark one, that
//! a groove's halves are the other way round from each other, that a corner
//! is split on the diagonal, and that the line between a groove's halves
//! follows a rounded corner's curve.
//!
//! Every box is 40 by 20 inside an 8px border of `#808080`, at (10, 10), so
//! its border box runs from 10 to 66 across and 10 to 46 down. The border's
//! two tones are about `#2b2b2b` and `#d5d5d5`.

use alo_box::build as build_boxes;
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_paint::{Canvas, PaintContext, build, render};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{FontDatabase, TextMeasurer};
use alo_value::Rgba;

const WIDTH: u32 = 80;
const HEIGHT: u32 = 60;

fn draw(css: &str) -> Canvas {
    let document = parse_document("<!DOCTYPE html><html><body><div id=b></div></body></html>");
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(&format!(
        "body {{ margin: 10px; background: #ffffff }}
         #b {{ width: 40px; height: 20px; {css} }}"
    ));
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build_boxes(&document, &styles);
    let database = FontDatabase::new();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, Size::new(80.0, 60.0), &measurer);
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
    canvas
}

/// What a pixel is, as `D` (the darker tone), `L` (the lighter one), `W`
/// (the page) or `?` (anything else, which no assertion here expects).
fn tone(canvas: &Canvas, x: u32, y: u32) -> char {
    match canvas.at(x, y).map(Rgba::to_rgba8) {
        Some((red, green, blue, _)) if red == green && green == blue => match red {
            0..=60 => 'D',
            190..=230 => 'L',
            255 => 'W',
            _ => '?',
        },
        _ => '?',
    }
}

/// The four sides' tones, outer half then inner half, top, right, bottom,
/// left — sampled in the middle of each side.
fn sides(canvas: &Canvas) -> [(char, char); 4] {
    [
        (tone(canvas, 38, 11), tone(canvas, 38, 16)),
        (tone(canvas, 64, 28), tone(canvas, 59, 28)),
        (tone(canvas, 38, 44), tone(canvas, 38, 39)),
        (tone(canvas, 11, 28), tone(canvas, 16, 28)),
    ]
}

#[test]
fn inset_is_dark_at_the_top_and_left_and_light_at_the_bottom_and_right() {
    let canvas = draw("border: 8px inset #808080");
    assert_eq!(
        sides(&canvas),
        [('D', 'D'), ('L', 'L'), ('L', 'L'), ('D', 'D')]
    );
}

#[test]
fn outset_is_the_other_way_round() {
    let canvas = draw("border: 8px outset #808080");
    assert_eq!(
        sides(&canvas),
        [('L', 'L'), ('D', 'D'), ('D', 'D'), ('L', 'L')]
    );
}

#[test]
fn a_groove_is_inset_outside_and_outset_inside() {
    let canvas = draw("border: 8px groove #808080");
    assert_eq!(
        sides(&canvas),
        [('D', 'L'), ('L', 'D'), ('L', 'D'), ('D', 'L')]
    );
}

#[test]
fn a_ridge_is_a_groove_turned_inside_out() {
    let canvas = draw("border: 8px ridge #808080");
    assert_eq!(
        sides(&canvas),
        [('L', 'D'), ('D', 'L'), ('D', 'L'), ('L', 'D')]
    );
}

/// A picture as bytes, which compare exactly.
type Pixels = Vec<(u8, u8, u8, u8)>;

#[test]
fn no_two_of_the_four_are_the_same_picture() {
    let pictures: Vec<(&str, Pixels)> = ["inset", "outset", "groove", "ridge"]
        .into_iter()
        .map(|style| {
            let canvas = draw(&format!("border: 8px {style} #808080"));
            (
                style,
                canvas
                    .pixels()
                    .iter()
                    .map(|pixel| pixel.to_rgba8())
                    .collect(),
            )
        })
        .collect();
    for (index, (first, one)) in pictures.iter().enumerate() {
        for (second, other) in pictures.iter().skip(index + 1) {
            assert_ne!(one, other, "{first} and {second} drew the same pixels");
        }
    }
    let solid = draw("border: 8px solid #808080");
    let solid: Pixels = solid
        .pixels()
        .iter()
        .map(|pixel| pixel.to_rgba8())
        .collect();
    for (style, picture) in &pictures {
        assert_ne!(&solid, picture, "{style} drew a solid border");
    }
}

#[test]
fn a_corner_is_split_on_the_diagonal_where_the_tones_change() {
    // The top right corner of an inset border: the dark top and the light
    // right meet on the line from (66, 10) to (58, 18). Above that line is
    // the top's; below it, the right's. Four rectangles would have given the
    // whole corner to one of them.
    let canvas = draw("border: 8px inset #808080");
    assert_eq!(tone(&canvas, 61, 11), 'D', "above the diagonal");
    assert_eq!(tone(&canvas, 64, 14), 'L', "below it");
    // And the bottom left, the other way: the dark left, the light bottom.
    assert_eq!(tone(&canvas, 11, 40), 'D', "left of the diagonal");
    assert_eq!(tone(&canvas, 14, 43), 'L', "right of it");
}

#[test]
fn where_both_sides_are_one_tone_the_corner_has_no_seam() {
    // The inset border's top left, where the top and the left are both dark:
    // the pixels on the diagonal are as dark as either side, rather than
    // showing the page through two anti-aliased edges.
    let canvas = draw("border: 8px inset #808080");
    for step in 0..8 {
        assert_eq!(tone(&canvas, 10 + step, 10 + step), 'D', "at {step}");
    }
}

#[test]
fn a_rounded_grooves_halves_meet_on_a_curve() {
    // 24px tall rather than 20, so the border box is 56 by 40 and a radius of
    // 20 fits without being scaled down. Centred at (30, 30): the halves meet
    // at radius 16 and the border ends at radius 12. On the corner's diagonal, at (17.5, 17.5) —
    // 17.7 from the centre — is the outer half; at (20.5, 20.5) — 13.4 — the
    // inner half. Halves cut by straight lines would have put both inside.
    let canvas = draw("height: 24px; border: 8px groove #808080; border-radius: 20px");
    assert_eq!(tone(&canvas, 17, 17), 'D', "the outer half");
    assert_eq!(tone(&canvas, 20, 20), 'L', "the inner half");
    // And outside the curve, nothing: the corner is rounded.
    assert_eq!(tone(&canvas, 11, 11), 'W', "outside the corner");
    // Inside the border, nothing either: the padding box.
    assert_eq!(tone(&canvas, 23, 23), 'W', "inside the border");
}

#[test]
fn a_solid_side_beside_a_two_toned_one_is_mitred_too() {
    // A solid top and an inset right: the corner is split rather than the
    // solid side's rectangle taking it.
    let canvas =
        draw("border: 8px inset #808080; border-top-style: solid; border-top-color: #000000");
    assert_eq!(
        canvas.at(61, 11).map(Rgba::to_rgba8),
        Some((0, 0, 0, 255)),
        "the solid top, above the diagonal",
    );
    assert_eq!(tone(&canvas, 64, 14), 'L', "the inset right, below it");
}

#[test]
fn a_side_whose_style_is_not_drawn_leaves_its_place_empty() {
    // A hidden bottom draws nothing, and is not drawn as anything else; its
    // neighbours still stop where it begins.
    let canvas = draw("border: 8px inset #808080; border-bottom-style: hidden");
    assert_eq!(tone(&canvas, 38, 44), 'W', "no bottom");
    assert_eq!(tone(&canvas, 11, 28), 'D', "the left is still there");
    // The bottom left corner's diagonal runs from (10, 46) to (18, 38): the
    // left side is above it and the empty bottom below it.
    assert_eq!(tone(&canvas, 11, 41), 'D', "the left reaches the diagonal");
    assert_eq!(tone(&canvas, 14, 44), 'W', "and stops there");
}
