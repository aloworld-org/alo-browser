/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An `<svg>`'s drawing and an SVG picture rounded by their own
//! `border-radius`: the case that closes queue item 784.
//!
//! `cases/rounded-svgs` places five `<svg>`s, each holding a black square
//! that reaches 20 past its viewport on every side, and four `<img>`s of a
//! black SVG square. CSS trims every replaced box's content to its content
//! edge's curve, and so does Chrome 156.0.8078.76 for both kinds, as it
//! does a raster picture's (items 769 and 783): the curve each radius less
//! the border and the padding beside it, laid on the snapped border box
//! less them as laid out — measured, not read from its source, headless,
//! without `--disable-gpu`, at one, two and eight. An `<svg>` that is
//! `overflow: visible` is not trimmed (`s4`). Before this item this engine
//! drew all eight square, 255 of 255 from Chrome at their corners.
//!
//! Chrome smooths an SVG picture's curve in quarters of a pixel (its edge
//! pixels are 64, 128 or 191 from white, at eight too), where this
//! engine's coverage is the curve's area: so a curve's edge pixel differs
//! by up to [`BOUND`] — the worst on `i4`, an SVG picture at fractions of
//! a pixel. Every pixel that is white or black in both pictures is the
//! same in both.
//!
//! `chrome.png` and `chrome-at-2.png` beside the case are Chrome's
//! screenshots of the same bytes, 320 by 130 CSS pixels, at
//! `--force-device-scale-factor` 1 and 2, taken 2026-11-02;
//! `render-at-2.png` is this engine's picture at two.

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_layout::Rect;
use alo_paint::display::DisplayItem;
use alo_paint::path::Path;
use alo_paint::{Canvas, Corners, rounded_rectangle};

/// How far any pixel may be from Chrome's, of 255, at one and at two: the
/// `<svg>`s in the top row, and the pictures under them. Measured: 25 at
/// one and 20 at two on `s5`'s circle; 58 and 60 on `i4`'s curves.
const BOUND: [(f32, &str, u8, u8); 2] = [
    (1.0, "chrome.png", 25, 58),
    (2.0, "chrome-at-2.png", 20, 60),
];

/// The row of `<svg>`s ends here, and the pictures' starts.
const ROW: u32 = 65;

/// Each box's clips, in the order pushed, at one and at two: a rectangle,
/// then any curve — the rectangle as left, top, width and height, and the
/// radius on every corner (none for a square clip). Worked out by hand
/// from the case's style sheet.
///
/// `s3` is laid out at (130.3, 10.6) with a padding of 2.25 and a radius of
/// 9.5: its content box snapped edge by edge is 133–173 by 13–53 at one
/// and 132.5–172.5 by 13–53 at two, and its curve, 7.25, is laid on its
/// border box snapped — 130–175 by 11–55 at one, 130.5–175 by 10.5–55 at
/// two — less 2.25.
type Clips = (&'static str, Vec<([f32; 4], f32)>, Vec<([f32; 4], f32)>);

fn clips() -> [Clips; 4] {
    [
        (
            "s1",
            vec![
                ([10.0, 10.0, 40.0, 40.0], 0.0),
                ([10.0, 10.0, 40.0, 40.0], 10.0),
            ],
            vec![
                ([10.0, 10.0, 40.0, 40.0], 0.0),
                ([10.0, 10.0, 40.0, 40.0], 10.0),
            ],
        ),
        (
            "s3",
            vec![
                ([133.0, 13.0, 40.0, 40.0], 0.0),
                ([132.25, 13.25, 40.5, 39.5], 7.25),
            ],
            vec![
                ([132.5, 13.0, 40.0, 40.0], 0.0),
                ([132.75, 12.75, 40.0, 40.0], 7.25),
            ],
        ),
        // Not clipped at all.
        ("s4", Vec::new(), Vec::new()),
        (
            "i1",
            vec![
                ([10.0, 70.0, 40.0, 40.0], 0.0),
                ([10.0, 70.0, 40.0, 40.0], 10.0),
            ],
            vec![
                ([10.0, 70.0, 40.0, 40.0], 0.0),
                ([10.0, 70.0, 40.0, 40.0], 10.0),
            ],
        ),
    ]
}

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("rounded-svgs"))
}

/// The box of the element with `class`.
fn box_of(rendering: &Rendering, class: &str) -> Option<BoxId> {
    let (document, drawing) = (rendering.document()?, rendering.drawing()?);
    let node = document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.attr("class") == Some(class))
    })?;
    let boxes = &drawing.boxes;
    boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|box_node| &box_node.kind),
            Some(BoxKind::Element { node: of, .. }) if *of == node
        )
    })
}

/// Every clip pushed for the box with `class`, in order.
fn clips_of(rendering: &Rendering, class: &str) -> Option<Vec<Path>> {
    let id = box_of(rendering, class)?;
    let drawing = rendering.drawing()?;
    Some(
        drawing
            .display
            .items()
            .iter()
            .filter_map(|item| match item {
                DisplayItem::PushClip { box_id, path } if *box_id == id => Some(path.clone()),
                _ => None,
            })
            .collect(),
    )
}

#[test]
fn each_drawing_is_clipped_square_then_trimmed_to_its_curve() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, pick) in [(1.0_f32, 0), (2.0, 1)] {
        let Ok(rendering) = Rendering::at_scale(&case, scale) else {
            panic!("the case renders at {scale}");
        };
        for (class, at_one, at_two) in clips() {
            let Some(found) = clips_of(&rendering, class) else {
                panic!("{class} has a box at {scale}");
            };
            let expected = if pick == 0 { at_one } else { at_two };
            assert_eq!(found.len(), expected.len(), "{class} at {scale}");
            for (clip, ([left, top, width, height], radius)) in found.iter().zip(expected) {
                let wanted =
                    rounded_rectangle(Rect::new(left, top, width, height), Corners::all(radius));
                // A square rectangle's path may be written as plain lines,
                // so a square clip is compared by its bounds alone.
                let close = |a: f32, b: f32| (a - b).abs() < 0.001;
                let bounds = clip.bounds().zip(wanted.bounds());
                assert!(
                    bounds.is_some_and(|(a, b)| close(a.0, b.0)
                        && close(a.1, b.1)
                        && close(a.2, b.2)
                        && close(a.3, b.3)),
                    "{class} at {scale} is clipped to {:?}, not {:?}",
                    clip.bounds(),
                    wanted.bounds()
                );
                if radius > 0.0 {
                    assert_eq!(
                        format!("{:?}", clip.segments()),
                        format!("{:?}", wanted.segments()),
                        "{class}'s curve at {scale}"
                    );
                }
            }
        }
    }
}

/// The largest difference in any channel between two pictures' pixels.
fn difference(ours: &Canvas, chrome: &Canvas, x: u32, y: u32) -> u8 {
    let (Some(a), Some(b)) = (ours.at(x, y), chrome.at(x, y)) else {
        return u8::MAX;
    };
    let ((ar, ag, ab, _), (br, bg, bb, _)) = (a.to_rgba8(), b.to_rgba8());
    [ar.abs_diff(br), ag.abs_diff(bg), ab.abs_diff(bb)]
        .into_iter()
        .max()
        .unwrap_or(u8::MAX)
}

/// Whether a pixel is white or black.
fn plain(canvas: &Canvas, x: u32, y: u32) -> Option<bool> {
    let (red, green, blue, _) = canvas.at(x, y)?.to_rgba8();
    match (red, green, blue) {
        (255, 255, 255) => Some(true),
        (0, 0, 0) => Some(false),
        _ => None,
    }
}

#[test]
fn every_plain_pixel_is_chromes_and_every_edge_within_its_smoothing() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, name, drawings, pictures) in BOUND {
        let drawing = Rendering::at_scale(&case, scale)
            .ok()
            .and_then(|rendering| rendering.drawing());
        let chrome = std::fs::read(case.expectation(name))
            .ok()
            .and_then(|bytes| alo_paint::from_png(&bytes).ok());
        let (Some(drawing), Some(chrome)) = (drawing, chrome) else {
            panic!("the case is drawn at {scale}, and {name} is beside it");
        };
        let ours = &drawing.canvas;
        assert_eq!(
            (ours.width(), ours.height()),
            (chrome.width(), chrome.height()),
            "at {scale}"
        );
        let mut plain_compared = 0;
        for y in 0..ours.height() {
            for x in 0..ours.width() {
                if let (Some(a), Some(b)) = (plain(ours, x, y), plain(&chrome, x, y)) {
                    assert_eq!(a, b, "at {scale}, ({x}, {y}) is a different colour");
                    plain_compared += 1;
                    continue;
                }
                let row = if scale > 1.0 { ROW * 2 } else { ROW };
                let bound = if y < row { drawings } else { pictures };
                let found = difference(ours, &chrome, x, y);
                assert!(
                    found <= bound,
                    "at {scale} ({x}, {y}) is {found} from Chrome's"
                );
            }
        }
        assert!(plain_compared > 30_000, "{plain_compared} at {scale}");
    }
}

#[test]
fn the_picture_at_two_is_its_reference() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let Some(drawing) = Rendering::at_scale(&case, 2.0)
        .ok()
        .and_then(|rendering| rendering.drawing())
    else {
        panic!("the case is drawn at two");
    };
    let difference = check_picture(&case, "render-at-2.png", &drawing.canvas);
    assert!(difference.is_none(), "{difference:?}");
}
