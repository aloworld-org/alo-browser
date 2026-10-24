/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Dashed rounded borders fitted to their length in whole device pixels:
//! the case that closes queue item 617.
//!
//! `cases/dashes-fitted-to-whole-pixels` places four dashed circles with
//! 10 px borders, 34 to 60 px across, whose curves are wider than half the
//! stroke, and four 300 × 40 boxes with dashed, rounded borders 4, 6 and
//! 10 px wide, one with corners clamped to its height.
//!
//! Chrome fits a dashed line's gaps to the line's length truncated to
//! whole device pixels, and lays the pattern along the whole line, so the
//! last gap ends up to a device pixel before the line does and what is
//! left is the start of one more dash, run on into the first. So the first
//! dash starts up to a device pixel before the line's start, where the
//! top-left curve meets the top. This engine fitted the gaps to the whole
//! length and started the first dash at the line's start: 13 plain pixels
//! were a different colour from Chrome's at one and 39 at two, nearly all
//! in the column just before a line's start. Now none are. Measured at
//! one, two and eight on eleven 300 × 40 boxes and eight circles, not read
//! from its source.
//!
//! `chrome.png` and `chrome-at-2.png` beside the case are Chrome 155's
//! (`155.0.8059.86`) screenshots of the same bytes, 330 by 280 CSS pixels,
//! headless, started with `--force-device-scale-factor=1` and `=2`,
//! without `--disable-gpu`, taken 2026-10-24.
//!
//! `render-at-2.png` is this engine's picture at two, beside `render.png`
//! and never in its place (ADR 0057 § 5).

use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_paint::Canvas;

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("dashes-fitted-to-whole-pixels"))
}

/// The case's plain colours: the page's white and the borders' blue.
const PLAIN: [(u8, u8, u8); 2] = [(255, 255, 255), (0x1d, 0x4e, 0xd8)];

/// A long box's top: its name, the row through the middle of its top
/// border and where its line starts on that row, in CSS pixels.
type Top = (&'static str, f32, f32);

const TOPS: [Top; 4] = [
    ("four", 82.0, 26.0),
    ("six", 133.0, 19.0),
    ("ten", 185.0, 21.0),
    ("flat", 235.0, 30.0),
];

/// How much of the device pixel just before each long box's line start,
/// on the row through its top, the first dash covers in Chrome 155's
/// pictures, at one and at two, in the order of [`TOPS`]. Chrome draws
/// these edges in quarters of coverage; the dash reaches back past the
/// line's start by what truncating the line's length left, under a device
/// pixel.
const BEFORE_THE_START: [(f32, [f32; 4]); 2] = [
    (1.0, [0.25, 0.75, 0.75, 0.5]),
    (2.0, [0.75, 0.75, 0.75, 0.25]),
];

/// How far this engine's share of a pixel may be from Chrome's: a quarter,
/// Chrome's own step.
const SHARE_WITHIN: f32 = 0.25;

/// Where the dashes cross each long box's top row in Chrome 155's picture
/// at eight, in CSS pixels: the first three runs of the border's colour
/// covering more than half a device pixel, and the last two. `flat`'s
/// first run is the corner's sliver, then its first dash from 29.875,
/// before its line's start at 30.
const RUNS: [(&str, [(f32, f32); 5]); 4] = [
    (
        "four",
        [
            (18.25, 21.875),
            (26.0, 34.0),
            (38.125, 46.0),
            (279.5, 287.5),
            (291.625, 299.875),
        ],
    ),
    (
        "six",
        [
            (19.0, 31.0),
            (37.0, 48.875),
            (54.875, 66.875),
            (270.125, 282.0),
            (288.0, 300.0),
        ],
    ),
    (
        "ten",
        [
            (21.0, 41.0),
            (51.0, 71.0),
            (81.0, 101.0),
            (230.875, 250.875),
            (260.875, 280.875),
        ],
    ),
    (
        "flat",
        [
            (16.75, 16.875),
            (29.875, 50.0),
            (60.75, 80.75),
            (245.0, 264.875),
            (275.625, 295.875),
        ],
    ),
];

/// How many runs Chrome 155's picture at eight has along each top, in the
/// order of [`RUNS`].
const COUNTS: [usize; 4] = [24, 16, 9, 10];

/// The stretch of each top row read at eight, in CSS pixels.
const STRETCH: [(f32, f32); 4] = [(12.0, 300.0), (13.0, 302.0), (15.0, 300.0), (15.0, 300.0)];

/// How far a run's ends may be from Chrome's at eight: a device pixel.
const RUN_WITHIN: f32 = 0.125;

/// A device pixel's index on this page, from a CSS pixel at `scale`.
fn device(at: f32, scale: f32) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a pixel of a 330 × 280 page at eight"
    )]
    let index = (at * scale) as u32;
    index
}

/// How much of a pixel the borders' blue covers, from its red: 29 is all
/// of it and 255 none.
fn share(canvas: &Canvas, x: u32, y: u32) -> Option<f32> {
    let (red, ..) = canvas.at(x, y)?.to_rgba8();
    Some((255.0 - f32::from(red)) / (255.0 - 29.0))
}

/// Runs of the border's colour, more than half covering a pixel, along a
/// row from `from` to `to` CSS pixels, in CSS pixels, at `scale`.
fn runs(canvas: &Canvas, scale: f32, row: f32, (from, to): (f32, f32)) -> Vec<(f32, f32)> {
    let y = device(row, scale);
    let mut found: Vec<(f32, f32)> = Vec::new();
    let mut open: Option<u32> = None;
    for x in device(from, scale)..=device(to, scale) {
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

/// The case drawn at `scale`.
fn drawn(case: &Case, scale: f32) -> Option<Canvas> {
    let drawing = Rendering::at_scale(case, scale).ok()?.drawing()?;
    Some(drawing.canvas.clone())
}

/// The first dash reaches back over the device pixel before its line's
/// start as far as Chrome's does, within a quarter of it, at one and at two;
/// fitted to the whole length it reached back over none of it.
#[test]
fn the_first_dash_starts_before_its_line_as_chromes_does() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, chrome) in BEFORE_THE_START {
        let Some(canvas) = drawn(&case, scale) else {
            panic!("the case is drawn at {scale}");
        };
        for ((name, row, start), theirs) in TOPS.into_iter().zip(chrome) {
            let (x, y) = (device(start, scale).saturating_sub(1), device(row, scale));
            let Some(ours) = share(&canvas, x, y) else {
                panic!("{name}'s pixel ({x}, {y}) is on the page at {scale}");
            };
            assert!(
                ours > 0.0 && (ours - theirs).abs() <= SHARE_WITHIN,
                "{name} at {scale}: {ours} of ({x}, {y}) where Chrome's is {theirs}"
            );
        }
    }
}

/// Where the dashes cross each long box's top at eight, within a device
/// pixel of Chrome's: as many, as long, and as far apart.
#[test]
fn every_dash_along_the_tops_is_where_chromes_is() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let Some(canvas) = drawn(&case, 8.0) else {
        panic!("the case is drawn at eight");
    };
    for (((name, row, _), (_, chrome)), (count, stretch)) in TOPS
        .into_iter()
        .zip(RUNS)
        .zip(COUNTS.into_iter().zip(STRETCH))
    {
        let found = runs(&canvas, 8.0, row, stretch);
        assert_eq!(found.len(), count, "{name}: {found:?}");
        let ends = found.iter().take(3).chain(found.iter().skip(count - 2));
        for ((from, to), (theirs_from, theirs_to)) in ends.zip(chrome) {
            assert!(
                (from - theirs_from).abs() <= RUN_WITHIN && (to - theirs_to).abs() <= RUN_WITHIN,
                "{name}: ({from}, {to}) where Chrome's is ({theirs_from}, {theirs_to})",
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
/// and at two.
#[test]
fn every_plain_pixel_is_chromes() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for scale in [1.0, 2.0] {
        let (Some(ours), Some(chrome)) = (drawn(&case, scale), chrome(&case, scale)) else {
            panic!("the case is drawn at {scale}, and Chrome's picture is beside it");
        };
        assert_eq!(
            (ours.width(), ours.height()),
            (chrome.width(), chrome.height()),
            "at {scale}"
        );
        let mut differing = Vec::new();
        let mut compared = 0_u32;
        for y in 0..ours.height() {
            for x in 0..ours.width() {
                if let (Some(a), Some(b)) = (plain(&ours, x, y), plain(&chrome, x, y)) {
                    compared += 1;
                    if a != b {
                        differing.push((x, y));
                    }
                }
            }
        }
        assert!(compared > 0, "at {scale} nothing was compared");
        assert_eq!(
            differing,
            Vec::<(u32, u32)>::new(),
            "at {scale}, the pixels whose colour is not Chrome's"
        );
    }
}

#[test]
fn the_picture_at_two_is_its_reference() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let Some(canvas) = drawn(&case, 2.0) else {
        panic!("the case is drawn at two");
    };
    let difference = check_picture(&case, "render-at-2.png", &canvas);
    assert!(difference.is_none(), "{difference:?}");
}
