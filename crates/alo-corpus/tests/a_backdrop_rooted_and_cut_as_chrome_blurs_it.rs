/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A box blurring what is behind it as the backdrop root of what it holds,
//! and one cut by the page's edge, drawn as Chrome draws them: queue item
//! 887 (ADR 0104 amended).
//!
//! `cases/backdrops-rooted-and-cut` is `backdrops-blurred`'s page of 4 px
//! cells, and over it: a box off the page's top left corner, rounded 16,
//! `blur(4px)` under `rgba(255, 255, 255, 0.5)` — its crop cut at the page's
//! edge and mirrored there; a box `blur(3px)` under `rgba(255, 255, 255,
//! 0.3)` holding one `blur(5px)` that reaches past its side — which blurs
//! only what its holder drew, the white, never the page nor its holder's
//! own blur; and a box `blur(3px)` with no background holding one
//! `blur(6px)`, which so draws nothing at all.
//!
//! `chrome.png`, `chrome-at-2.png` and `chrome-at-8.png` beside it are
//! Google Chrome 157.0.8103.38's screenshots of the same bytes, headless on
//! its GPU path, 500 by 300 with `--force-device-scale-factor` 1, 2 and 8,
//! cropped to the page and made RGBA, taken 2026-11-09 (the session
//! scratchpad's `m887/`).
//!
//! Every pixel is compared, as `a_backdrop_blurred_as_chrome_blurs_it.rs`
//! compares them: **outside** every box, **inside** one two device pixels
//! and more from every edge, and on a **rim**. The bounds are what this
//! engine drew, measured, so a change that moves a blur says so. Measured
//! 2026-11-09: outside, every pixel Chrome's at each scale; inside, at most
//! 2 levels, 0.57–0.63 on average; on the rims at most 11, 12 and 13 at
//! one, two and eight, about 0.4 on average — the rounded corners, as in
//! `backdrops-blurred`. Before this item, the box held inside another was
//! up to 58 levels from Chrome's at one and two.
//!
//! `render-at-2.png` beside `render.png` is this engine's picture at two.

use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_paint::Canvas;

/// Each box's border box, in CSS pixels, and its corners' radius: the one
/// cut by the page's edge, the holder and what it holds, and the bare
/// holder and what it holds.
const BOXES: [(f32, f32, f32, f32, f32); 5] = [
    (-20.0, -14.0, 72.0, 56.0, 16.0),
    (70.0, 12.0, 88.0, 76.0, 12.0),
    (122.0, 28.0, 48.0, 40.0, 0.0),
    (178.0, 30.0, 56.0, 60.0, 0.0),
    (186.0, 38.0, 32.0, 30.0, 0.0),
];

/// At each scale, the most and the mean a channel may differ from
/// Chrome's, of 255: outside, inside, and on the rim.
struct Bounds {
    scale: f32,
    outside: (u8, f32),
    inside: (u8, f32),
    rim: (u8, f32),
}

const BOUNDS: [Bounds; 3] = [
    Bounds {
        scale: 1.0,
        outside: (0, 0.0),
        inside: (2, 0.58),
        rim: (11, 0.40),
    },
    Bounds {
        scale: 2.0,
        outside: (0, 0.0),
        inside: (2, 0.60),
        rim: (12, 0.43),
    },
    Bounds {
        scale: 8.0,
        outside: (0, 0.0),
        inside: (2, 0.64),
        rim: (13, 0.45),
    },
];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("backdrops-rooted-and-cut"))
}

/// The picture Chrome took at `scale`, beside the case.
fn chrome(case: &Case, scale: f32) -> Option<Canvas> {
    let name = match scale {
        1.0 => "chrome.png",
        2.0 => "chrome-at-2.png",
        _ => "chrome-at-8.png",
    };
    let bytes = std::fs::read(case.expectation(name)).ok()?;
    alo_paint::from_png(&bytes).ok()
}

/// Where a device pixel's middle is, at `scale`: outside every box, inside
/// one at least two device pixels from its edge, or on its rim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    Outside,
    Inside,
    Rim,
}

/// How far inside a rounded rectangle a point is, in its own units: less
/// than nothing outside it.
fn depth((x, y): (f32, f32), (left, top, width, height, radius): (f32, f32, f32, f32, f32)) -> f32 {
    let (right, bottom) = (left + width, top + height);
    let straight = (x - left).min(right - x).min(y - top).min(bottom - y);
    if radius <= 0.0 {
        return straight;
    }
    let cx = x.clamp(left + radius, right - radius);
    let cy = y.clamp(top + radius, bottom - radius);
    if (cx - x).abs() > 0.0 && (cy - y).abs() > 0.0 {
        return radius - ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
    }
    straight
}

fn place(x: u32, y: u32, scale: f32) -> Place {
    #[expect(clippy::cast_precision_loss, reason = "a pixel of a small page")]
    let middle = ((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
    let rim = 2.0 / scale;
    // Near any box's edge is a rim, whatever box it is inside.
    let depths = BOXES.map(|shape| depth(middle, shape));
    if depths.iter().any(|inside| inside.abs() < rim) {
        return Place::Rim;
    }
    if depths.iter().any(|inside| *inside >= rim) {
        return Place::Inside;
    }
    Place::Outside
}

/// The largest difference in any channel between two pixels.
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

/// The most and the mean difference in one place.
#[derive(Debug, Default)]
struct Tally {
    most: u8,
    sum: u64,
    count: u64,
    worst: Option<(u32, u32)>,
}

impl Tally {
    fn add(&mut self, differs: u8, at: (u32, u32)) {
        self.sum += u64::from(differs);
        self.count += 1;
        if differs > self.most {
            self.most = differs;
            self.worst = Some(at);
        }
    }

    fn mean(&self) -> f32 {
        #[expect(clippy::cast_precision_loss, reason = "a sum of levels over a page")]
        let mean = self.sum as f32 / self.count.max(1) as f32;
        mean
    }
}

/// Each place's tally: outside, inside, on the rim.
#[derive(Debug, Default)]
struct Found {
    outside: Tally,
    inside: Tally,
    rim: Tally,
}

fn compared(ours: &Canvas, chrome: &Canvas, scale: f32) -> Found {
    let mut found = Found::default();
    for y in 0..ours.height() {
        for x in 0..ours.width() {
            let tally = match place(x, y, scale) {
                Place::Outside => &mut found.outside,
                Place::Inside => &mut found.inside,
                Place::Rim => &mut found.rim,
            };
            tally.add(difference(ours, chrome, x, y), (x, y));
        }
    }
    found
}

#[test]
fn every_pixel_is_as_near_chromes_as_measured() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for bounds in BOUNDS {
        let scale = bounds.scale;
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
        let found = compared(ours, &chrome, scale);
        for (tally, (most, mean), name) in [
            (&found.outside, bounds.outside, "outside"),
            (&found.inside, bounds.inside, "inside"),
            (&found.rim, bounds.rim, "on the rim"),
        ] {
            assert!(tally.count > 0, "at {scale} nothing {name}");
            assert!(
                tally.most <= most && tally.mean() <= mean,
                "at {scale} {name}: {} of 255 at most, {:.3} on average, at {:?}",
                tally.most,
                tally.mean(),
                tally.worst,
            );
        }
    }
}

#[test]
fn the_picture_at_two_is_its_reference() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let Some(rendering) = Rendering::at_scale(&case, 2.0).ok() else {
        panic!("the case is drawn at two");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case is drawn at two");
    };
    let difference = check_picture(&case, "render-at-2.png", &drawing.canvas);
    assert!(difference.is_none(), "{difference:?}");
}
