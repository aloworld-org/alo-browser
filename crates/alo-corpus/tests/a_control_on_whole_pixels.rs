/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A checkbox and a radio laid out at fractions of a pixel, drawn on whole
//! ones as Chrome draws them: the case that closes queue item 457.
//!
//! `cases/checkbox-beside-text` lays its controls out at y 22.77, 42.55,
//! 99.25 and the like. Until item 457 each was drawn there, its edges
//! smeared across two rows, and its picture differed from Chrome's by 16.3
//! of 255 on average. Chrome 154 rounds each edge of the box to a whole
//! pixel and draws the picture in that (`alo_paint::snap`).
//!
//! `chrome.png` beside the case is Chrome's screenshot of the same bytes,
//! with the corpus's font, at one device pixel to one and without
//! `--disable-gpu`. Each control's square is compared with Chrome's square
//! **where each engine put it**. The first five stand on the same whole
//! pixels in both; the last four stand one or two pixels lower in Chrome,
//! whose rows are whole pixels tall because it rounds the font's ascent and
//! descent (queue item 460), which is where a control stands, not what it
//! draws. The bound is `toggle-artwork`'s, measured on whole pixels: 64 of
//! 255 at worst, 9 on average for any one control, 3.0 over all of them.
//! Here, measured: 64, 6.7 and 2.1.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_paint::Canvas;
use alo_value::Rgba;

/// Where Chrome 154 draws each control in the case, in document order: its
/// `getBoundingClientRect()` rounded to whole pixels, and its side.
const CHROME: [(u32, u32, u32); 9] = [
    (24, 3, 13),
    (25, 23, 13),
    (4, 43, 13),
    (24, 63, 13),
    (64, 63, 13),
    (44, 100, 13),
    (20, 123, 13),
    (20, 142, 13),
    (24, 162, 20),
];

/// The case's picture here, Chrome's, and where this engine drew each
/// control: the whole pixels of its snapped border box, which in this case
/// is its square.
struct Pictures {
    ours: Canvas,
    chrome: Canvas,
    controls: Vec<(u32, u32, u32)>,
}

fn pictures() -> Option<Pictures> {
    let directory = cases_directory().join("checkbox-beside-text");
    let case = Case::read(&directory)?;
    let rendering = Rendering::of(&case).ok()?;
    let drawing = rendering.drawing()?;
    let chrome = alo_paint::from_png(&std::fs::read(directory.join("chrome.png")).ok()?).ok()?;
    let boxes = &drawing.boxes;
    let controls = boxes
        .ids()
        .filter(|id| boxes.toggle(*id).is_some())
        .filter_map(|id| {
            let snapped = alo_paint::snap::to_whole_pixels(drawing.layout.get(id)?.border_box);
            Some((
                pixel(snapped.origin.x),
                pixel(snapped.origin.y),
                pixel(snapped.size.width),
            ))
        })
        .collect();
    Some(Pictures {
        ours: drawing.canvas.clone(),
        chrome,
        controls,
    })
}

/// A whole number of pixels on the page.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a whole, positive coordinate on a 300 × 200 page"
)]
fn pixel(value: f32) -> u32 {
    value.max(0.0) as u32
}

/// The largest difference in any channel between two pixels.
fn difference(a: Option<Rgba>, b: Option<Rgba>) -> Option<u8> {
    let (ar, ag, ab, _) = a?.to_rgba8();
    let (br, bg, bb, _) = b?.to_rgba8();
    [ar.abs_diff(br), ag.abs_diff(bg), ab.abs_diff(bb)]
        .into_iter()
        .max()
}

#[test]
fn each_control_is_drawn_on_the_whole_pixels_chrome_draws_it_on() {
    let Some(pictures) = pictures() else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    // Laid out at fractions; drawn from whole pixels. The first five are
    // Chrome's pixels exactly; the rest are lower in Chrome by its rows.
    assert_eq!(
        pictures.controls,
        vec![
            (24, 3, 13),
            (25, 23, 13),
            (4, 43, 13),
            (24, 63, 13),
            (64, 63, 13),
            (44, 99, 13),
            (20, 122, 13),
            (20, 140, 13),
            (24, 160, 20),
        ]
    );
    // An unchecked checkbox's edge fills its first column and row, and the
    // pixel outside it is the row's background, with nothing smeared over.
    let edge = Rgba::new(118.0 / 255.0, 118.0 / 255.0, 118.0 / 255.0, 1.0);
    for &(left, top, side) in [pictures.controls.first(), pictures.controls.get(2)]
        .into_iter()
        .flatten()
    {
        let middle = top + side / 2;
        assert_eq!(
            difference(pictures.ours.at(left, middle), Some(edge)),
            Some(0),
            "the edge at ({left}, {middle})"
        );
        assert_eq!(
            difference(pictures.ours.at(left + side / 2, top), Some(edge)),
            Some(0),
            "the edge at ({}, {top})",
            left + side / 2
        );
        assert_eq!(
            difference(
                pictures.ours.at(left - 1, middle),
                pictures.chrome.at(left - 1, middle)
            ),
            Some(0),
            "beside the edge at ({}, {middle})",
            left - 1
        );
    }
}

#[test]
fn each_controls_picture_is_chromes_within_toggle_artworks_bound() {
    let Some(pictures) = pictures() else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    assert_eq!(pictures.controls.len(), CHROME.len());
    let mut total = 0u32;
    let mut counted = 0u32;
    for (&(left, top, side), &(chrome_left, chrome_top, chrome_side)) in
        pictures.controls.iter().zip(&CHROME)
    {
        assert_eq!(side, chrome_side, "the control at ({left}, {top})");
        let mut worst = 0u8;
        let mut sum = 0u32;
        for down in 0..side {
            for across in 0..side {
                let here = pictures.ours.at(left + across, top + down);
                let there = pictures.chrome.at(chrome_left + across, chrome_top + down);
                let Some(d) = difference(here, there) else {
                    panic!("the control at ({left}, {top}) is on both pictures");
                };
                worst = worst.max(d);
                sum += u32::from(d);
            }
        }
        let pixels = side * side;
        assert!(
            worst <= 64,
            "the control at ({left}, {top}) differs from Chrome's by {worst}"
        );
        assert!(
            sum <= 9 * pixels,
            "the control at ({left}, {top}) differs from Chrome's by {} on average",
            f64::from(sum) / f64::from(pixels)
        );
        total += sum;
        counted += pixels;
    }
    // `toggle-artwork`'s 3.0 on whole pixels; measured here, 2.1.
    assert!(
        total * 10 <= 31 * counted,
        "the controls differ from Chrome's by {} on average",
        f64::from(total) / f64::from(counted)
    );
}
