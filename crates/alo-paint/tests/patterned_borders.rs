/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `dashed`, `dotted` and `double`, in pixels.
//!
//! The committed picture is `alo-corpus`'s `border-patterns`. What is here is
//! what a picture can only differ about: where the gaps are, that a dot is
//! round, that a double border's lines are a third of its width each, and
//! that none of the three is drawn as a solid border.
//!
//! Every box is 60 by 30 inside a 6px black border, at (10, 10), so its
//! border box runs from 10 to 82 across and 10 to 52 down.
//!
//! - **Dashed**: the top is 72 long, which is three dashes and two gaps of
//!   14.4 each — dashes from 10 to 24.4, 38.8 to 53.2 and 67.6 to 82. The
//!   left is 42 long: two dashes and a gap of 14, the gap from 24 to 38.
//! - **Dotted**: the top's dots are centred 3 down, at 13, 24, 35, 46, 57, 68
//!   and 79 across, each 6 wide.
//! - **Double**: the lines are 10 to 12 and 14 to 16 deep on the top.

use alo_box::build as build_boxes;
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_paint::{Canvas, PaintContext, build, render};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{FontDatabase, TextMeasurer};
use alo_value::Rgba;

const WIDTH: u32 = 100;
const HEIGHT: u32 = 70;

fn draw(css: &str) -> Canvas {
    let document = parse_document("<!DOCTYPE html><html><body><div id=b></div></body></html>");
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(&format!(
        "body {{ margin: 10px; background: #ffffff }}
         #b {{ width: 60px; height: 30px; {css} }}"
    ));
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build_boxes(&document, &styles);
    let database = FontDatabase::new();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, Size::new(100.0, 70.0), &measurer);
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
    canvas
}

/// What a pixel is: `#` the border's black (or nearly: at most a sixth of
/// it is the page), `.` the page's white (at most a sixth of it border), or
/// `?` anything between, which is an edge.
fn ink(canvas: &Canvas, x: u32, y: u32) -> char {
    match canvas.at(x, y).map(Rgba::to_rgba8) {
        Some((red, green, blue, 255)) if red == green && green == blue => match red {
            0..=42 => '#',
            213..=255 => '.',
            _ => '?',
        },
        _ => '?',
    }
}

/// A picture as bytes, which compare exactly.
type Pixels = Vec<(u8, u8, u8, u8)>;

fn pixels(canvas: &Canvas) -> Pixels {
    canvas
        .pixels()
        .iter()
        .map(|pixel| pixel.to_rgba8())
        .collect()
}

#[test]
fn a_dashed_side_has_gaps_and_starts_and_ends_on_a_dash() {
    let canvas = draw("border: 6px dashed #000000");
    let top: String = [12, 17, 31, 46, 60, 75, 80]
        .into_iter()
        .map(|x| ink(&canvas, x, 12))
        .collect();
    assert_eq!(top, "##.#.##", "along the top");
    let left: String = [12, 17, 31, 45, 50]
        .into_iter()
        .map(|y| ink(&canvas, 12, y))
        .collect();
    assert_eq!(left, "##.##", "down the left");
    // A dash is the border's whole depth, and stops where the padding box
    // begins.
    assert_eq!(ink(&canvas, 46, 15), '#');
    assert_eq!(ink(&canvas, 46, 16), '.');
}

#[test]
fn a_dashed_corner_has_no_seam() {
    // The top right corner's dash is the top's last and the right's first,
    // meeting on the mitre from (82, 10) to (76, 16). One colour, so one
    // shape: solid along the diagonal, where two pieces cut off at different
    // points on it once left a line of grey there.
    let canvas = draw("border: 6px dashed #000000");
    for step in 0..6 {
        assert_eq!(ink(&canvas, 81 - step, 10 + step), '#', "at {step}");
        assert_eq!(
            ink(&canvas, 10 + step, 10 + step),
            '#',
            "top left at {step}"
        );
        assert_eq!(
            ink(&canvas, 81 - step, 51 - step),
            '#',
            "bottom right at {step}"
        );
    }
}

#[test]
fn a_dotted_sides_dots_are_round_and_apart() {
    let canvas = draw("border: 6px dotted #000000");
    // The middle of every dot along the top, and between each pair.
    for centre in [13, 24, 35, 46, 57, 68, 79] {
        assert_eq!(ink(&canvas, centre, 13), '#', "dot at {centre}");
    }
    for between in [18, 29, 40, 51, 62, 73] {
        assert_eq!(ink(&canvas, between, 13), '.', "gap at {between}");
    }
    // Round: the pixel in a dot's square corner is nearly all outside the
    // circle — its centre is 3.5 from the dot's — where a square dot would
    // have filled it, and the pixels beside and above the centre, at the
    // same distance across or down, are filled.
    assert_eq!(ink(&canvas, 32, 10), '.', "the square's corner is empty");
    assert_eq!(ink(&canvas, 32, 13), '#', "beside the centre is filled");
    assert_eq!(ink(&canvas, 35, 10), '#', "above the centre is filled");
}

#[test]
fn a_dot_in_a_corner_is_one_dot_without_a_seam() {
    // The top left dot is centred on the corner's mitre at (13, 13), and the
    // top and the left both draw it: it is one dot, solid along the
    // diagonal where two halves would each have been anti-aliased.
    let canvas = draw("border: 6px dotted #000000");
    for step in 1..5 {
        assert_eq!(ink(&canvas, 10 + step, 10 + step), '#', "at {step}");
    }
}

#[test]
fn a_double_border_is_two_lines_a_third_of_its_width_each() {
    let canvas = draw("border: 6px double #000000");
    let down: String = (10..17).map(|y| ink(&canvas, 46, y)).collect();
    assert_eq!(down, "##..##.", "across the top");
    let across: String = (10..17).map(|x| ink(&canvas, x, 31)).collect();
    assert_eq!(across, "##..##.", "across the left");
    // Both lines turn the corner.
    assert_eq!(ink(&canvas, 10, 10), '#');
    assert_eq!(ink(&canvas, 12, 12), '.');
    assert_eq!(ink(&canvas, 15, 15), '#');
}

#[test]
fn a_rounded_double_borders_lines_follow_the_curve() {
    // A radius of 15: the outer line runs between radius 15 and 13 about
    // the corner's centre at (25, 25), the inner between 11 and 9. On the
    // diagonal, 14 from the centre is (15.1, 15.1) and 10 is (17.9, 17.9).
    let canvas = draw("border: 6px double #000000; border-radius: 15px");
    assert_eq!(ink(&canvas, 10, 10), '.', "outside the curve");
    assert_eq!(ink(&canvas, 15, 15), '#', "the outer line");
    assert_eq!(ink(&canvas, 17, 17), '#', "the inner line");
    assert_eq!(ink(&canvas, 20, 20), '.', "inside the border");
}

#[test]
fn none_of_the_three_is_drawn_as_solid_or_as_each_other() {
    let solid = pixels(&draw("border: 6px solid #000000"));
    let drawn: Vec<(&str, Pixels)> = ["dashed", "dotted", "double"]
        .into_iter()
        .map(|style| {
            (
                style,
                pixels(&draw(&format!("border: 6px {style} #000000"))),
            )
        })
        .collect();
    for (index, (style, picture)) in drawn.iter().enumerate() {
        assert_ne!(&solid, picture, "{style} drew a solid border");
        let blank = vec![(255, 255, 255, 255); picture.len()];
        assert_ne!(&blank, picture, "{style} drew nothing");
        for (other, second) in drawn.iter().skip(index + 1) {
            assert_ne!(picture, second, "{style} and {other} drew the same pixels");
        }
    }
}

#[test]
fn a_patterned_side_beside_a_solid_one_is_mitred() {
    // A solid top and a dashed right: the corner is split on its mitre,
    // from (82, 10) to (76, 16), rather than the solid top's rectangle
    // taking all of it. The right's first dash is in the corner, so below
    // the diagonal is the right's, in its own colour.
    let canvas =
        draw("border: 6px dashed #000000; border-top-style: solid; border-top-color: #ff0000");
    assert_eq!(
        canvas.at(77, 11).map(Rgba::to_rgba8),
        Some((255, 0, 0, 255)),
        "the solid top, above the diagonal",
    );
    assert_eq!(ink(&canvas, 80, 14), '#', "the dashed right, below it");
}

#[test]
fn a_hair_thin_dotted_border_on_a_huge_box_is_drawn_and_bounded() {
    // A page that asks for a tenth of a pixel of dots round a box a hundred
    // thousand pixels tall: the dots are bounded in number and the picture
    // is still drawn rather than refused or run out of memory.
    let canvas = draw("height: 100000px; border: 0.1px dotted #000000");
    assert_eq!(canvas.width(), WIDTH);
}

#[test]
fn a_hair_thin_dashed_border_on_a_huge_box_is_drawn_and_bounded() {
    // The same, dashed, and with sides of very different widths, so that
    // the mitres run deep and many dashes are cut off on them.
    for css in [
        "width: 100000px; height: 100000px; border: 0.01px dashed #000000",
        "width: 2000px; height: 2000px; border-style: dashed;
         border-width: 0.01px 1000px; border-color: #000000",
    ] {
        let canvas = draw(css);
        assert_eq!(canvas.width(), WIDTH, "{css}");
    }
}
