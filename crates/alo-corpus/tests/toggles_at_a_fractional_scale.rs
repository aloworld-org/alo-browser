/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Checkboxes and radios on a screen whose device pixels do not make whole
//! CSS pixels: the cases that close queue item 597.
//!
//! Chrome 154 draws a checkbox or a radio in its border box snapped to
//! whole device pixels and then cut to whole CSS pixels from the snapped
//! corner (`alo_paint::toggle::picture_box`). This engine drew it in the
//! snapped box itself, which at 1.5 is a half CSS pixel wider than Chrome's
//! picture or a whole one: `cases/toggles-at-fractional-scales` had 677
//! pixels of a different plain colour from Chrome's at 1.5 and 1,248 at
//! two, and `cases/checkbox-beside-text` 74 at 1.5. Measured, not read from
//! Chrome's source.
//!
//! `chrome.png`, `chrome-at-1.5.png` and `chrome-at-2.png` beside
//! `toggles-at-fractional-scales` are Chrome 154's screenshots of the same
//! bytes, 224 by 830 CSS pixels, headless, started with
//! `--force-device-scale-factor=1`, `=1.5` and `=2`, without
//! `--disable-gpu`, taken 2026-10-14. `chrome-at-1.5.png` beside
//! `checkbox-beside-text` is the same at 1.5, 300 by 200, each element set
//! in the corpus's `DejaVu` face for its computed family, as its
//! `chrome-at-2.png` was.
//!
//! `render-at-1.5.png` and `render-at-2.png` are this engine's pictures at
//! those scales, beside `render.png` and never in its place (ADR 0057 § 5).

use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_paint::Canvas;
use alo_renderer::Drawing;
use std::rc::Rc;

/// The three scales each case is drawn and compared at, and the name of
/// Chrome's picture at each.
const SCALES: [(f32, &str); 3] = [
    (1.0, "chrome.png"),
    (1.5, "chrome-at-1.5.png"),
    (2.0, "chrome-at-2.png"),
];

/// How many controls each of `toggles-at-fractional-scales`' four groups
/// holds: nine sizes at six offsets.
const GROUP: usize = 54;

fn case(name: &str) -> Option<Case> {
    Case::read(&cases_directory().join(name))
}

fn drawn(case: &Case, scale: f32) -> Option<Rc<Drawing>> {
    Rendering::at_scale(case, scale)
        .ok()
        .and_then(|rendering| rendering.drawing())
}

fn chrome(case: &Case, name: &str) -> Option<Canvas> {
    std::fs::read(case.expectation(name))
        .ok()
        .and_then(|bytes| alo_paint::from_png(&bytes).ok())
}

/// A case, drawn at `scale`, and Chrome's picture `name` beside it.
fn drawn_beside_chrome(case: &Case, scale: f32, name: &str) -> Option<(Rc<Drawing>, Canvas)> {
    Some((drawn(case, scale)?, chrome(case, name)?))
}

/// A coordinate in whole device pixels, for a value already on or near
/// one; anything off the canvas is its edge.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a whole, positive coordinate on a canvas of a few hundred pixels"
)]
fn pixel(device: f32) -> u32 {
    device.max(0.0) as u32
}

/// The device pixels a control's picture touches at `scale`: the largest
/// square in its picture box, centred, its edges taken outwards to whole
/// device pixels.
fn drawn_square(border_box: alo_layout::Rect, scale: f32) -> (u32, u32, u32, u32) {
    let snapped = alo_paint::snap::to_device_pixels(border_box, scale);
    let picture = alo_paint::toggle::picture_box(snapped, scale);
    let side = picture.size.width.min(picture.size.height);
    let left = picture.left() + (picture.size.width - side) / 2.0;
    let top = picture.top() + (picture.size.height - side) / 2.0;
    // A float step either way is not a pixel of ink.
    let slack = 1.0 / 1024.0;
    (
        pixel((left * scale + slack).floor()),
        pixel((top * scale + slack).floor()),
        pixel(((left + side) * scale - slack).ceil()),
        pixel(((top + side) * scale - slack).ceil()),
    )
}

/// The device pixels that are not white around a control's snapped box in
/// Chrome's picture: where Chrome drew it.
fn ink(chrome: &Canvas, around: (u32, u32, u32, u32)) -> Option<(u32, u32, u32, u32)> {
    let (left, top, right, bottom) = around;
    let mut found: Option<(u32, u32, u32, u32)> = None;
    for y in top.saturating_sub(3)..bottom + 3 {
        for x in left.saturating_sub(3)..right + 3 {
            let (red, green, blue, _) = chrome.at(x, y)?.to_rgba8();
            if (red, green, blue) != (255, 255, 255) {
                found = Some(found.map_or((x, y, x + 1, y + 1), |(l, t, r, b)| {
                    (l.min(x), t.min(y), r.max(x + 1), b.max(y + 1))
                }));
            }
        }
    }
    found
}

/// Every unchecked checkbox and radio's picture covers the device pixels
/// Chrome's does, at one, two and 1.5: the picture box in numbers, against
/// Chrome's screenshots. Unchecked, because their edge is grey on white to
/// its last pixel; a checked one is the same box filled.
#[test]
fn each_unchecked_control_is_drawn_on_the_device_pixels_chromes_is() {
    let Some(case) = case("toggles-at-fractional-scales") else {
        panic!("the case reads");
    };
    for (scale, name) in SCALES {
        let Some((drawing, chrome)) = drawn_beside_chrome(&case, scale, name) else {
            panic!("the case is drawn at {scale}, and Chrome's {name} is beside it");
        };
        let toggles: Vec<alo_box::BoxId> = drawing
            .boxes
            .ids()
            .filter(|id| drawing.boxes.toggle(*id).is_some())
            .collect();
        assert_eq!(toggles.len(), 4 * GROUP, "at {scale}");
        let unchecked = toggles
            .iter()
            .take(GROUP)
            .chain(toggles.iter().skip(2 * GROUP).take(GROUP));
        for &id in unchecked {
            let Some(geometry) = drawing.layout.get(id) else {
                panic!("the control is laid out");
            };
            let ours = drawn_square(geometry.border_box, scale);
            let snapped = alo_paint::snap::to_device_pixels(geometry.border_box, scale);
            let around = (
                pixel((snapped.left() * scale).round()),
                pixel((snapped.top() * scale).round()),
                pixel((snapped.right() * scale).round()),
                pixel((snapped.bottom() * scale).round()),
            );
            assert_eq!(
                Some(ours),
                ink(&chrome, around),
                "at {scale}, the control laid out at {:?}",
                geometry.border_box
            );
        }
    }
}

/// `toggles-at-fractional-scales`' plain colours: the page, a live edge
/// and the accent.
const PLAIN: [(u8, u8, u8); 3] = [(255, 255, 255), (118, 118, 118), (0, 117, 255)];

/// How far the pixels that are a blend in either picture may be from
/// Chrome's, at each scale: on average, and at most. Measured: 6.99, 5.74
/// and 6.49 on average, and 78, 91 and 65 at most — the checked checkbox's
/// tick, whose edge Chrome draws its own way at every scale, as
/// `toggle_artwork.rs` and `a_page_drawn_at_a_scale_of_two.rs` hold at one
/// and two.
const BLENDS: [(f32, u8); 3] = [(7.5, 80), (6.25, 95), (7.0, 70)];

/// Which plain colour a pixel is, if it is one.
fn plain_of(canvas: &Canvas, x: u32, y: u32, plain: &[(u8, u8, u8)]) -> Option<usize> {
    let (red, green, blue, _) = canvas.at(x, y)?.to_rgba8();
    plain.iter().position(|&(r, g, b)| {
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

/// The pixels where both pictures have a plain colour and it is not the
/// same one, and over the rest the mean and largest difference.
fn compare(ours: &Canvas, chrome: &Canvas, plain: &[(u8, u8, u8)]) -> (Vec<(u32, u32)>, f64, u8) {
    let mut differing = Vec::new();
    let (mut blends, mut total, mut worst) = (0_u32, 0_u32, 0_u8);
    for y in 0..ours.height() {
        for x in 0..ours.width() {
            if let (Some(a), Some(b)) = (plain_of(ours, x, y, plain), plain_of(chrome, x, y, plain))
            {
                if a != b {
                    differing.push((x, y));
                }
                continue;
            }
            let found = difference(ours, chrome, x, y);
            blends += 1;
            total += u32::from(found);
            worst = worst.max(found);
        }
    }
    let mean = if blends == 0 {
        0.0
    } else {
        f64::from(total) / f64::from(blends)
    };
    (differing, mean, worst)
}

/// Wherever both pictures of `toggles-at-fractional-scales` have a plain
/// colour it is the same one, at one, 1.5 and two; where either is a
/// blend, the two are within [`BLENDS`].
#[test]
fn every_plain_pixel_is_chromes_at_one_one_and_a_half_and_two() {
    let Some(case) = case("toggles-at-fractional-scales") else {
        panic!("the case reads");
    };
    for ((scale, name), (mean, most)) in SCALES.into_iter().zip(BLENDS) {
        let Some((drawing, chrome)) = drawn_beside_chrome(&case, scale, name) else {
            panic!("the case is drawn at {scale}, and Chrome's {name} is beside it");
        };
        assert_eq!(
            (drawing.canvas.width(), drawing.canvas.height()),
            (chrome.width(), chrome.height()),
            "at {scale}"
        );
        let (differing, average, worst) = compare(&drawing.canvas, &chrome, &PLAIN);
        assert!(
            differing.is_empty(),
            "at {scale}, {} plain pixels differ, first {:?}",
            differing.len(),
            differing.first()
        );
        assert!(
            average <= f64::from(mean),
            "at {scale} blends are {average} from Chrome's on average"
        );
        assert!(worst <= most, "at {scale} a blend is {worst} from Chrome's");
    }
}

/// `checkbox-beside-text`'s plain colours: the page, the rows, the text,
/// a live edge and the accent.
const PLAIN_BESIDE_TEXT: [(u8, u8, u8); 5] = [
    (255, 255, 255),
    (221, 226, 238),
    (0, 0, 0),
    (118, 118, 118),
    (0, 117, 255),
];

/// The pixels of `checkbox-beside-text` at 1.5 that are still a different
/// plain colour from Chrome's, and none is a control's: the foot of the
/// last row's letters, which Chrome draws a device pixel lower where this
/// engine's baseline is the one layout gives (queue item 607).
const NOT_YET: [(u32, u32); 4] = [(11, 271), (17, 271), (18, 271), (95, 271)];

#[test]
fn checkbox_beside_text_at_one_and_a_half_is_chromes_but_for_a_row_of_letters() {
    let Some((drawing, chrome)) = case("checkbox-beside-text")
        .and_then(|case| drawn_beside_chrome(&case, 1.5, "chrome-at-1.5.png"))
    else {
        panic!("the case is drawn at 1.5, and Chrome's picture is beside it");
    };
    assert_eq!(
        (drawing.canvas.width(), drawing.canvas.height()),
        (chrome.width(), chrome.height())
    );
    let (differing, _, _) = compare(&drawing.canvas, &chrome, &PLAIN_BESIDE_TEXT);
    assert_eq!(differing, NOT_YET);
}

#[test]
fn the_pictures_at_one_and_a_half_and_two_are_their_references() {
    for (name, scales) in [
        ("toggles-at-fractional-scales", &[1.5_f32, 2.0][..]),
        ("checkbox-beside-text", &[1.5][..]),
    ] {
        let Some(case) = case(name) else {
            panic!("{name} reads");
        };
        for &scale in scales {
            let reference = if scale > 1.5 {
                "render-at-2.png"
            } else {
                "render-at-1.5.png"
            };
            let Some(drawing) = drawn(&case, scale) else {
                panic!("{name} is drawn at {scale}");
            };
            let difference = check_picture(&case, reference, &drawing.canvas);
            assert!(difference.is_none(), "{name}: {difference:?}");
        }
    }
}
