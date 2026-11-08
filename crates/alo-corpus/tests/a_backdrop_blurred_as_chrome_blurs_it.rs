/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `backdrop-filter: blur()` drawn as Chrome draws it: the case that closes
//! queue item 885 (ADR 0104).
//!
//! `cases/backdrops-blurred` is a page of 4 px cells in five colours, and
//! over it three boxes 64 by 56: `blur(4px)` with nothing else — the
//! cleanest read of the kernel and of the crop's mirrored edge — then
//! `blur(4px)` under `rgba(255, 255, 255, 0.7)` rounded 16, and
//! `blur(8px)` under `rgba(27, 36, 51, 0.32)`, *Settings*' own backdrop.
//! At one, two and eight that is a standard deviation of 4, 8 and 32
//! device pixels for the first two — blurred whole, halved once, halved
//! three times — and of 8, 16 and 64 for the third.
//!
//! `chrome.png`, `chrome-at-2.png` and `chrome-at-8.png` beside it are
//! Google Chrome 157.0.8103.38's screenshots of the same bytes, headless on
//! its GPU path (no `--disable-gpu`), 500 by 300 with
//! `--force-device-scale-factor` 1, 2 and 8, cropped to the page and made
//! RGBA, taken 2026-11-08 (the session scratchpad's `m885b/`).
//!
//! Every pixel is compared, in three places: **outside** every box, where
//! nothing is blurred and both are the page; **inside** a box, two device
//! pixels and more in from its edge and its corners' curves, which is the
//! blur; and its **rim**, the pixels between, where the box's soft edge
//! meets the page. The bounds are what this engine drew, measured, so a
//! change that moves a blur says so. Measured 2026-11-08: outside, every
//! pixel Chrome's at each scale; inside, at most 1, 2 and 2 levels from it
//! at one, two and eight, 0.55–0.61 on average; on the rim at most 15, 10
//! and 19, about 0.4 on average — the most at the rounded box's corners,
//! where Chrome covers an edge pixel otherwise than by its area (ADR 0104).
//!
//! `render-at-2.png` beside `render.png` is this engine's picture at two.

use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_paint::Canvas;

/// Each box's border box, in CSS pixels, and its corners' radius.
const BOXES: [(f32, f32, f32, f32, f32); 3] = [
    (8.0, 8.0, 64.0, 56.0, 0.0),
    (88.0, 8.0, 64.0, 56.0, 16.0),
    (168.0, 8.0, 64.0, 56.0, 0.0),
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
        inside: (1, 0.56),
        rim: (15, 0.41),
    },
    Bounds {
        scale: 2.0,
        outside: (0, 0.0),
        inside: (2, 0.60),
        rim: (10, 0.40),
    },
    Bounds {
        scale: 8.0,
        outside: (0, 0.0),
        inside: (2, 0.61),
        rim: (19, 0.44),
    },
];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("backdrops-blurred"))
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
    for shape in BOXES {
        let inside = depth(middle, shape);
        if inside >= rim {
            return Place::Inside;
        }
        if inside > -rim {
            return Place::Rim;
        }
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
