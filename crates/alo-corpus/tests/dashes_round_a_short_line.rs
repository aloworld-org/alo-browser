/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Dashed borders round lines too short for two dashes and two gaps: the
//! case that closes queue item 615.
//!
//! `cases/dashes-round-a-short-line` places ten small boxes with dashed,
//! fully rounded borders: circles with borders 1, 2, 3, 5 and 10 px wide,
//! an oblong with straight sides between its curves, an ellipse, and a
//! 30 px circle just long enough for the pattern's own spacing.
//!
//! The line through the middle of each of the first nine is longer than
//! two dashes but shorter than two dashes and two gaps. Chrome draws two
//! dashes there, a third of the line each, with a sixth between: the
//! pattern's own two widths and one (three and two under three device
//! pixels), shortened together. This engine drew two dashes of the full
//! two widths with what was left as gaps, so the 24 px box's dashes went
//! nearly half way round where Chrome's go a third: 115 plain pixels were
//! a different colour from Chrome's at one and 555 at two, 84 and 354 of
//! them on that box. Then 3 and 24 were; since the dashes are fitted to the
//! line's length in whole device pixels (queue item 617) none are at one
//! and 19 at two, all in [`NOT_YET`]. Measured at one, two and eight on 133
//! circles 1 to 16 px wide, not read from its source.
//!
//! `chrome.png` and `chrome-at-2.png` beside the case are Chrome 155's
//! (`155.0.8059.86`) screenshots of the same bytes, 200 by 100 CSS pixels,
//! headless, started with `--force-device-scale-factor=1` and `=2`,
//! without `--disable-gpu`, taken 2026-10-24.
//!
//! `render-at-2.png` is this engine's picture at two, beside `render.png`
//! and never in its place (ADR 0057 § 5).

use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_paint::Canvas;

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("dashes-round-a-short-line"))
}

/// Where a box's dashes cross a row three quarters of the way down it, in
/// CSS pixels, as Chrome 155's picture at eight has them: the box, the
/// row, the stretch of it read, and each run of the border's colour that
/// covers more than half of a device pixel. On the right of the row the
/// gap between the first dash's end and the second's start is open, where
/// a dash of two full widths covered it.
type Row = (&'static str, f32, (f32, f32), &'static [(f32, f32)]);

const ROWS: [Row; 10] = [
    ("ten", 28.0, (9.0, 35.0), &[(11.625, 22.0)]),
    ("five", 20.5, (49.0, 65.0), &[(51.0, 57.0)]),
    ("three", 15.625, (79.0, 88.5), &[(80.5, 83.75)]),
    ("three-wider", 16.375, (99.0, 109.5), &[(100.625, 104.25)]),
    ("two", 13.75, (119.0, 126.0), &[(120.375, 122.5)]),
    ("two-wider", 14.125, (139.0, 146.5), &[(140.375, 142.75)]),
    ("one", 12.625, (159.0, 164.5), &[(161.25, 163.125)]),
    ("oblong", 62.0, (9.0, 31.0), &[(11.125, 22.0)]),
    (
        "ellipse",
        65.75,
        (49.0, 78.0),
        &[(51.875, 63.5), (72.375, 75.125)],
    ),
    ("longer", 72.5, (99.0, 131.0), &[(102.125, 115.0)]),
];

/// How far, in CSS pixels, a run's end may be from Chrome's at eight: two
/// device pixels. Where a dash ends on a curve tighter than half the
/// stroke, Chrome's stroker leaves a sliver of a different shape (queue
/// item 604); `ellipse`'s second run ends a quarter of a pixel short of
/// Chrome's for it, and every other run is within an eighth.
const WITHIN: f32 = 0.25;

/// The case's plain colours: the page's white and the borders' blue.
const PLAIN: [(u8, u8, u8); 2] = [(255, 255, 255), (0x1d, 0x4e, 0xd8)];

/// The pixels at each scale, in device pixels, whose plain colour is not
/// Chrome's: each at the end of a dash round a curve tighter than half the
/// stroke, which Chrome's own stroker covers otherwise (queue item 604) —
/// at two on `ten`, along the edges of its dashes' ends near the hole, and
/// on `oblong` and `ellipse`, along the edges of dash ends near the curve's
/// middle. None at one: the three there were the first dash's start, which
/// item 617's whole-pixel fit put where Chrome's is.
const NOT_YET: [(f32, &[(u32, u32)]); 2] = [
    (1.0, &[]),
    (
        2.0,
        &[
            (31, 36),
            (33, 37),
            (35, 38),
            (37, 39),
            (39, 40),
            (48, 47),
            (50, 48),
            (52, 49),
            (116, 117),
            (44, 118),
            (117, 118),
            (119, 118),
            (118, 119),
            (135, 122),
            (134, 123),
            (136, 123),
            (137, 124),
            (138, 125),
            (139, 126),
        ],
    ),
];

/// Runs of the border's colour, more than half covering a pixel, along a
/// row from `from` to `to` CSS pixels, in CSS pixels, at `scale`.
fn runs(canvas: &Canvas, scale: f32, row: f32, (from, to): (f32, f32)) -> Vec<(f32, f32)> {
    let device = |at: f32| {
        // A device pixel's index, from a CSS pixel on this page.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a pixel of a 200 × 100 page at eight"
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

/// Where each box's dashes cross its row at eight, within [`WITHIN`] of
/// Chrome's: two dashes of a third of the line each, a sixth apart.
#[test]
fn two_dashes_go_round_a_short_line_as_chromes_do() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let Some(drawing) = Rendering::at_scale(&case, 8.0)
        .ok()
        .and_then(|rendering| rendering.drawing())
    else {
        panic!("the case is drawn at eight");
    };
    for (class, row, stretch, chrome) in ROWS {
        let found = runs(&drawing.canvas, 8.0, row, stretch);
        assert_eq!(found.len(), chrome.len(), "{class}: {found:?}");
        for ((from, to), (theirs_from, theirs_to)) in found.iter().zip(chrome) {
            assert!(
                (from - theirs_from).abs() <= WITHIN && (to - theirs_to).abs() <= WITHIN,
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

/// Wherever both pictures have a plain colour it is the same one, at one
/// and at two, but for the pixels in [`NOT_YET`] — which are all still
/// different, so the list shrinks as 604 is built.
#[test]
fn every_plain_pixel_is_chromes() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, excepted) in NOT_YET {
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
        let mut compared = 0_u32;
        for y in 0..ours.height() {
            for x in 0..ours.width() {
                if let (Some(a), Some(b)) = (plain(ours, x, y), plain(&chrome, x, y)) {
                    compared += 1;
                    if a != b {
                        differing.push((x, y));
                    }
                }
            }
        }
        assert!(compared > 0, "at {scale} nothing was compared");
        let mut expected = excepted.to_vec();
        expected.sort_unstable_by_key(|&(x, y)| (y, x));
        assert_eq!(
            differing, expected,
            "at {scale}, the pixels whose colour is not Chrome's"
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
