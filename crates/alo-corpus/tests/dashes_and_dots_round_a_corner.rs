/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Dashed and dotted borders with rounded corners: the case that closes
//! queue item 596.
//!
//! `cases/dashes-and-dots-round-a-corner` places fifteen boxes, most 44 by
//! 40, with dashed and dotted borders round rounded corners: sides of one
//! width and of two, one colour and four, solid sides beside dashed ones,
//! circles and an ellipse, sides 2 px wide whose dots are square at one and
//! round at two, a 10 px border round a corner of 20, three small boxes,
//! and two boxes laid out at tenths of a pixel.
//!
//! Chrome 154 lays a rounded border's dashes and dots along the line
//! through the middle of the border, round the whole box from where the
//! top-left curve meets the top, in each side's own pattern, measuring the
//! curves coarsely; this engine placed them on each straight side and let
//! the curve clip them: 2,112 pixels at one and 10,707 at two were a
//! different plain colour from Chrome's (287 at one on the 44 × 40 box with
//! sides of 4 and 10 that the item names). Measured at one, two and eight,
//! not read from its source.
//!
//! `chrome.png` and `chrome-at-2.png` beside the case are Chrome 154's
//! screenshots of the same bytes, 500 by 200 CSS pixels, headless, started
//! with `--force-device-scale-factor=1` and `=2`, without `--disable-gpu`,
//! taken 2026-10-14.
//!
//! `render-at-2.png` is this engine's picture at two, beside `render.png`
//! and never in its place (ADR 0057 § 5).

use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_paint::Canvas;

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("dashes-and-dots-round-a-corner"))
}

/// Where the dashes cross the line through the middle of a box's top, in
/// CSS pixels, as Chrome 154's picture at eight has them: the box, the
/// row, the stretch of it read, and each run of the border's colour that
/// covers more than half of a device pixel. The runs at either end are the
/// top's pattern carried into the corners.
type Top = (&'static str, f32, (f32, f32), &'static [(f32, f32)]);

const TOPS: [Top; 5] = [
    (
        "uneven-dashed",
        17.0,
        (15.0, 59.0),
        &[(21.625, 24.375), (29.0, 37.0), (41.25, 49.875)],
    ),
    (
        "thin-dashed",
        16.0,
        (195.0, 239.0),
        &[
            (202.125, 205.125),
            (207.0, 211.0),
            (212.875, 216.875),
            (218.75, 222.75),
            (224.625, 228.625),
            (230.625, 231.875),
        ],
    ),
    (
        "ellipse",
        17.0,
        (375.0, 419.0),
        &[(382.875, 390.875), (395.0, 403.0), (407.25, 411.0)],
    ),
    ("thick", 20.0, (435.0, 479.0), &[(454.875, 472.25)]),
    (
        "four",
        78.0,
        (15.0, 59.0),
        &[(30.0, 36.0), (41.625, 47.5), (53.25, 57.25)],
    ),
];

/// The case's plain colours: the page's white and the borders' blue, red,
/// green and black.
const PLAIN: [(u8, u8, u8); 5] = [
    (255, 255, 255),
    (0x1d, 0x4e, 0xd8),
    (0xc0, 0x39, 0x2b),
    (0x16, 0xa3, 0x4a),
    (0, 0, 0),
];

/// The one pixel at each scale, in the order of [`BLENDS`], whose plain
/// colour is not Chrome's, in device pixels: on `uneven-dashed`, just before the top's first dash,
/// which Chrome's stroke — 2.2 times the 10 px sides beside it, round a
/// curve tighter than half that — reaches back into. Queue item 604.
const NOT_YET: [(f32, (u32, u32)); 2] = [(1.0, (28, 16)), (2.0, (57, 30))];

/// How far blended pixels may be from Chrome's, at each scale: on average,
/// and how many may be more than half the way from one plain colour to the
/// other, of the 2,747 and 5,654 measured. Chrome draws a shape's edge at
/// one in quarters of coverage and this engine finer, so where a dash ends
/// mid-pixel the two can differ by up to three quarters of a step; the
/// worst at one, 224, is at `thick`'s dash ends.
const BLENDS: [(f32, f32, usize); 2] = [(1.0, 40.0, 140), (2.0, 36.0, 110)];

/// Runs of the border's colour, more than half covering a pixel, along a
/// row from `from` to `to` CSS pixels, in CSS pixels, at `scale`.
fn runs(canvas: &Canvas, scale: f32, row: f32, (from, to): (f32, f32)) -> Vec<(f32, f32)> {
    let device = |at: f32| {
        // A device pixel's index, from a CSS pixel on this page.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a pixel of a 500 × 200 page at eight"
        )]
        let index = (at * scale) as u32;
        index
    };
    let y = device(row);
    let mut found: Vec<(f32, f32)> = Vec::new();
    let mut open: Option<u32> = None;
    for x in device(from)..=device(to) {
        let on = canvas.at(x, y).is_some_and(|pixel| {
            let (red, ..) = pixel.to_rgba8();
            // Blue's red is 29; more than half way there from white.
            red < 142
        });
        let css = |index: u32| f32::from(u16::try_from(index).unwrap_or(u16::MAX)) / scale;
        match (on, open) {
            (true, None) => open = Some(x),
            (false, Some(start)) => {
                found.push((css(start), css(x)));
                open = None;
            }
            _ => {}
        }
    }
    found
}

/// Where the dashes of five boxes cross their top's middle line at eight,
/// within a device pixel of Chrome's: which is to say each dash is as long
/// as Chrome's and starts where Chrome's does, the corners' too.
#[test]
fn every_dash_is_where_chromes_is_along_the_top() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let Some(drawing) = Rendering::at_scale(&case, 8.0)
        .ok()
        .and_then(|rendering| rendering.drawing())
    else {
        panic!("the case is drawn at eight");
    };
    for (class, row, stretch, chrome) in TOPS {
        let found = runs(&drawing.canvas, 8.0, row, stretch);
        assert_eq!(found.len(), chrome.len(), "{class}: {found:?}");
        for ((from, to), (theirs_from, theirs_to)) in found.iter().zip(chrome) {
            assert!(
                (from - theirs_from).abs() <= 0.125 && (to - theirs_to).abs() <= 0.125,
                "{class}: ({from}, {to}) where Chrome's is ({theirs_from}, {theirs_to})",
            );
        }
    }
}

/// The picture Chrome took at `scale`, beside the case.
fn chrome(case: &Case, scale: f32) -> Option<Canvas> {
    let name = if scale > 1.0 {
        "chrome-at-2.png"
    } else {
        "chrome.png"
    };
    let bytes = std::fs::read(case.expectation(name)).ok()?;
    alo_paint::from_png(&bytes).ok()
}

/// Which plain colour a pixel is, if it is one.
fn plain(canvas: &Canvas, x: u32, y: u32) -> Option<usize> {
    let (red, green, blue, _) = canvas.at(x, y)?.to_rgba8();
    PLAIN.iter().position(|&(r, g, b)| {
        red.abs_diff(r) <= 1 && green.abs_diff(g) <= 1 && blue.abs_diff(b) <= 1
    })
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

/// Wherever both pictures have a plain colour it is the same one, at one
/// and at two, but for the one pixel each in [`NOT_YET`]: every dash and
/// dot round every curve where Chrome's is. Where either is a blend, the
/// two are within [`BLENDS`].
#[test]
fn every_dash_and_dot_is_drawn_where_chromes_is() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for ((scale, mean, past_half), (_, excepted)) in BLENDS.into_iter().zip(NOT_YET) {
        let rendering = Rendering::at_scale(&case, scale).ok();
        let (Some(drawing), Some(chrome)) = (
            rendering.as_ref().and_then(Rendering::drawing),
            chrome(&case, scale),
        ) else {
            panic!("the case is drawn at {scale}, and Chrome's picture is beside it");
        };
        let ours = &drawing.canvas;
        assert_eq!(
            (ours.width(), ours.height()),
            (chrome.width(), chrome.height()),
            "at {scale}"
        );
        let mut differing = Vec::new();
        let (mut blends, mut total, mut far) = (0_u32, 0_u32, 0_usize);
        for y in 0..ours.height() {
            for x in 0..ours.width() {
                if let (Some(a), Some(b)) = (plain(ours, x, y), plain(&chrome, x, y)) {
                    if a != b {
                        differing.push((x, y));
                    }
                    continue;
                }
                let found = difference(ours, &chrome, x, y);
                blends += 1;
                total += u32::from(found);
                if found > 128 {
                    far += 1;
                }
            }
        }
        assert_eq!(
            differing,
            [excepted],
            "at {scale}, a pixel is a different colour from Chrome's"
        );
        assert!(blends > 0, "at {scale} no edge was compared");
        let average = f64::from(total) / f64::from(blends);
        assert!(
            average <= f64::from(mean),
            "at {scale} blends are {average} from Chrome's on average"
        );
        assert!(
            far <= past_half,
            "at {scale}, {far} blends are past half a step"
        );
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
