/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page drawn for a screen of two device pixels to the CSS pixel: the
//! case that closes queue item 561 (ADR 0057 § 4).
//!
//! `cases/checkbox-beside-text` lays nine checkboxes and radios out at
//! fractions of a pixel. At a scale of one each is drawn on the whole
//! pixels Chrome draws it on (`a_control_on_whole_pixels.rs`); at two it is
//! drawn on whole **device** pixels, which are halves of CSS ones, so a
//! control at x 23.9609375 is drawn from 24 rather than 24 again by luck:
//! from device pixel 48, `floor(23.9609375 · 2 + 0.5)`.
//!
//! `chrome-at-2.png` beside the case is Chrome 154's screenshot of the same
//! bytes, 600 by 400, headless, started with `--force-device-scale-factor=2`
//! (never the emulated scale of Chrome's developer tools, which lays out as at one), each
//! element set in the corpus's `DejaVu` face for its computed family, without
//! `--disable-gpu`, taken 2026-10-13. Chrome's controls there are where
//! `getBoundingClientRect()` at two put them, each edge rounded as
//! `floor(x·2 + 0.5)`: the device-pixel boxes below.
//!
//! `render-at-2.png` is this engine's picture at two, a reference beside
//! `render.png` and never in its place (ADR 0057 § 5), and the case's
//! `render.png` is still what this engine draws at one.

use alo_corpus::{Case, cases_directory, check_picture, corpus_fonts};
use alo_layout::Size;
use alo_paint::Canvas;
use alo_renderer::{Drawing, Screen, render_at};
use alo_value::Rgba;
use std::rc::Rc;

/// Where Chrome 154 draws each control at a scale of two, in document
/// order, in device pixels: left, top, right, bottom. Read from
/// `chrome-at-2.png`, and each is `floor(x·2 + 0.5)` of the control's
/// `getBoundingClientRect()` at two.
const CHROME_AT_TWO: [(u32, u32, u32, u32); 9] = [
    (48, 6, 74, 32),
    (50, 46, 76, 72),
    (8, 86, 34, 112),
    (48, 127, 74, 153),
    (128, 127, 154, 153),
    (88, 199, 114, 225),
    (40, 244, 66, 270),
    (40, 282, 66, 308),
    (48, 322, 88, 362),
];

/// The colour of an unchecked checkbox's edge, Chrome's and ours.
const EDGE: (u8, u8, u8) = (118, 118, 118);

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("checkbox-beside-text"))
}

/// The case drawn on a screen at `scale`.
fn drawn(case: &Case, scale: f32) -> Rc<Drawing> {
    let screen = Screen::new(Size::new(case.size.0, case.size.1)).at_scale(scale);
    render_at(
        &case.html,
        &case.css,
        screen,
        &corpus_fonts(),
        &case.linked,
        &case.resources,
    )
    .drawing
}

fn chrome(case: &Case) -> Option<Canvas> {
    let bytes = std::fs::read(case.expectation("chrome-at-2.png")).ok()?;
    alo_paint::from_png(&bytes).ok()
}

/// The case, and Chrome's screenshot of it at two.
fn case_and_chrome() -> Option<(Case, Canvas)> {
    let case = case()?;
    let chrome = chrome(&case)?;
    Some((case, chrome))
}

/// Where this engine drew each control at two: the whole device pixels of
/// its snapped border box.
fn controls(drawing: &Drawing) -> Vec<(u32, u32, u32, u32)> {
    let boxes = &drawing.boxes;
    boxes
        .ids()
        .filter(|id| boxes.toggle(*id).is_some())
        .filter_map(|id| {
            let snapped =
                alo_paint::snap::to_device_pixels(drawing.layout.get(id)?.border_box, 2.0);
            Some((
                device(snapped.left()),
                device(snapped.top()),
                device(snapped.right()),
                device(snapped.bottom()),
            ))
        })
        .collect()
}

/// A whole number of device pixels at two, from CSS pixels.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a whole, positive coordinate on a 600 × 400 canvas"
)]
fn device(value: f32) -> u32 {
    (value * 2.0).round().max(0.0) as u32
}

/// The largest difference in any channel between two pixels.
fn difference(a: Option<Rgba>, b: Option<Rgba>) -> Option<u8> {
    let (ar, ag, ab, _) = a?.to_rgba8();
    let (br, bg, bb, _) = b?.to_rgba8();
    [ar.abs_diff(br), ag.abs_diff(bg), ab.abs_diff(bb)]
        .into_iter()
        .max()
}

fn rgb(pixel: Option<Rgba>) -> Option<(u8, u8, u8)> {
    let (red, green, blue, _) = pixel?.to_rgba8();
    Some((red, green, blue))
}

#[test]
fn the_canvas_is_device_pixels() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let drawing = drawn(&case, 2.0);
    assert_eq!(
        (drawing.canvas.width(), drawing.canvas.height()),
        (600, 400)
    );
    assert_eq!(drawing.display.scale().to_bits(), 2.0_f32.to_bits());
}

#[test]
fn each_control_is_drawn_on_the_device_pixels_chrome_draws_it_on() {
    let Some((case, chrome)) = case_and_chrome() else {
        panic!("the case reads, and Chrome's screenshot at two beside it");
    };
    let drawing = drawn(&case, 2.0);
    assert_eq!(controls(&drawing), CHROME_AT_TWO);
    // The five unchecked checkboxes: each edge is two device pixels of the
    // edge's colour, exactly where Chrome's is, and the device pixel outside
    // it is Chrome's too, with nothing smeared over it.
    let unchecked = [0, 2, 5, 6, 8];
    for &(left, top, right, bottom) in unchecked.iter().filter_map(|at| CHROME_AT_TWO.get(*at)) {
        let middle = u32::midpoint(top, bottom);
        let centre = u32::midpoint(left, right);
        for (x, y) in [
            (left, middle),
            (left + 1, middle),
            (right - 1, middle),
            (centre, top),
            (centre, bottom - 1),
        ] {
            assert_eq!(
                rgb(drawing.canvas.at(x, y)),
                Some(EDGE),
                "ours at ({x}, {y})"
            );
            assert_eq!(rgb(chrome.at(x, y)), Some(EDGE), "Chrome's at ({x}, {y})");
        }
        for (x, y) in [
            (left - 1, middle),
            (left + 2, middle),
            (right, middle),
            (centre, top - 1),
        ] {
            assert_eq!(
                difference(drawing.canvas.at(x, y), chrome.at(x, y)),
                Some(0),
                "beside the edge at ({x}, {y})"
            );
        }
    }
}

/// Each control's picture against Chrome's, pixel by pixel, where both put
/// it. Measured: 65 of 255 at worst (the checked checkbox's tick, whose
/// curve Chrome draws its own way, as at one), 2.4 on average for any one
/// control, and 0.79 over all of them — under `toggle-artwork`'s bound at
/// one (64, 9 and 3.0), but for a unit at the tick's edge.
#[test]
fn each_controls_picture_is_chromes_within_a_measured_bound() {
    let Some((case, chrome)) = case_and_chrome() else {
        panic!("the case reads, and Chrome's screenshot at two beside it");
    };
    let drawing = drawn(&case, 2.0);
    let mut total = 0u32;
    let mut counted = 0u32;
    for &(left, top, right, bottom) in &CHROME_AT_TWO {
        let mut worst = 0u8;
        let mut sum = 0u32;
        for y in top..bottom {
            for x in left..right {
                let Some(d) = difference(drawing.canvas.at(x, y), chrome.at(x, y)) else {
                    panic!("the control at ({left}, {top}) is on both pictures");
                };
                worst = worst.max(d);
                sum += u32::from(d);
            }
        }
        let pixels = (right - left) * (bottom - top);
        assert!(
            worst <= 65,
            "the control at ({left}, {top}) differs from Chrome's by {worst}"
        );
        assert!(
            sum * 10 <= 24 * pixels,
            "the control at ({left}, {top}) differs from Chrome's by {} on average",
            f64::from(sum) / f64::from(pixels)
        );
        total += sum;
        counted += pixels;
    }
    assert!(
        total * 100 <= 80 * counted,
        "the controls differ from Chrome's by {} on average",
        f64::from(total) / f64::from(counted)
    );
}

#[test]
fn the_picture_at_two_is_its_reference_and_the_picture_at_one_is_unchanged() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let at_two = drawn(&case, 2.0);
    let difference = check_picture(&case, "render-at-2.png", &at_two.canvas);
    assert!(difference.is_none(), "{difference:?}");
    let at_one = drawn(&case, 1.0);
    let difference = check_picture(&case, "render.png", &at_one.canvas);
    assert!(difference.is_none(), "{difference:?}");
}
