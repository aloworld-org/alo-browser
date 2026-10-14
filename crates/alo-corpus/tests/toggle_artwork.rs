/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A checkbox's and a radio's own picture against Chrome's, pixel by pixel:
//! the case that closes queue item 454.
//!
//! `cases/toggle-artwork` holds 32 controls, each on a whole pixel, and
//! `chrome.png` beside it is Chrome 154's screenshot of the same bytes at one
//! device pixel to one, taken without `--disable-gpu`. Every pixel of every
//! control is compared.
//!
//! **Not pixel for pixel, and why.** The shapes were fitted at eight device
//! pixels to one, where they agree with Chrome's to well under 1% of a
//! pixel's coverage. At one to one Chrome quantises an edge's coverage —
//! its tick's edge pixels are 0, 64, 128, 191 or 255 and nothing between —
//! and this engine's rasteriser does not. So the pixels inside a shape are
//! Chrome's to within a unit, and the pixels on an edge differ by the two
//! ways of smoothing it: measured, by at most 61 of 255 and by 2.47 on
//! average over all 6,264 (64 and 3.0 before queue item 459 sampled an
//! edge's pixels sixteen by sixteen). A disabled control's translucent edge
//! and inside used to meet as two shapes each smoothed on its own, which
//! let the page through the seam: up to 44 of 255 over black, where Chrome's
//! seam is the two pieces mixed. They are drawn as one picture now (queue
//! item 463) and differ by at most 24, all of it where Chrome's thin ring
//! at one to one runs further in than this engine's on the diagonal (queue
//! item 578).
//!
//! The two controls with `aria-checked="mixed"` are left out: this engine
//! draws a bar across them and Chrome does not (queue item 456).

use alo_box::{Checked, Toggle};
use alo_corpus::{Case, Rendering, cases_directory};
use alo_paint::Canvas;
use alo_value::Rgba;

/// The case's picture here, and Chrome's, with where each control's picture
/// is: the whole pixels of the square it is drawn in.
struct Pictures {
    ours: Canvas,
    chrome: Canvas,
    controls: Vec<(u32, u32, u32)>,
    /// Which of the controls are radios: round, so the same either side.
    radios: Vec<(u32, u32, u32)>,
    /// Which are disabled: translucent, so over black every seam shows.
    disabled: Vec<(u32, u32, u32)>,
}

fn pictures() -> Option<Pictures> {
    let directory = cases_directory().join("toggle-artwork");
    let case = Case::read(&directory)?;
    let rendering = Rendering::of(&case).ok()?;
    let drawing = rendering.drawing()?;
    let chrome = alo_paint::from_png(&std::fs::read(directory.join("chrome.png")).ok()?).ok()?;
    let boxes = &drawing.boxes;
    let squares = boxes
        .ids()
        .filter(|id| boxes.toggle(*id).is_some())
        .filter(|id| {
            boxes
                .get(*id)
                .is_some_and(|node| node.semantics.states.checked != Some(Checked::Mixed))
        })
        .filter_map(|id| {
            let rect = drawing.layout.get(id)?.border_box;
            let side = rect.size.width.min(rect.size.height);
            let left = rect.origin.x + (rect.size.width - side) / 2.0;
            let top = rect.origin.y + (rect.size.height - side) / 2.0;
            // A square that starts half way through a pixel covers one more.
            let span = (left + side).ceil() - left.floor();
            let square = (pixel(left.floor()), pixel(top.floor()), pixel(span));
            let disabled = boxes
                .get(id)
                .is_some_and(|node| node.semantics.states.disabled);
            Some((square, boxes.toggle(id) == Some(Toggle::Radio), disabled))
        })
        .collect::<Vec<_>>();
    Some(Pictures {
        ours: drawing.canvas.clone(),
        chrome,
        controls: squares.iter().map(|(square, ..)| *square).collect(),
        radios: squares
            .iter()
            .filter(|(_, radio, _)| *radio)
            .map(|(square, ..)| *square)
            .collect(),
        disabled: squares
            .iter()
            .filter(|(.., disabled)| *disabled)
            .map(|(square, ..)| *square)
            .collect(),
    })
}

/// A whole number of pixels on the page.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a whole, positive coordinate on a 240 × 120 page"
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
fn every_control_is_chromes_picture_within_the_measured_bound() {
    let Some(pictures) = pictures() else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    assert_eq!(pictures.controls.len(), 30, "32 controls, less the 2 mixed");
    let mut total = 0u32;
    let mut counted = 0u32;
    for &(left, top, span) in &pictures.controls {
        let mut worst = 0u8;
        let mut sum = 0u32;
        let mut pixels = 0u32;
        for y in top..top + span {
            for x in left..left + span {
                let Some(d) = difference(pictures.ours.at(x, y), pictures.chrome.at(x, y)) else {
                    panic!("({x}, {y}) is on both pictures");
                };
                worst = worst.max(d);
                sum += u32::from(d);
                pixels += 1;
            }
        }
        // Measured: 61 at worst, on a tick's edge (64 before a curve's edge
        // was sampled finely, queue item 459).
        assert!(
            worst <= 61,
            "the control at ({left}, {top}) differs from Chrome's by {worst}"
        );
        // Measured: 4.85 at worst, a live checked checkbox's tick. A disabled
        // radio over black was 9.22 until its pieces were drawn as one
        // (queue item 463), and is 2.96.
        assert!(
            sum * 100 <= 490 * pixels,
            "the control at ({left}, {top}) differs from Chrome's by {} on average",
            f64::from(sum) / f64::from(pixels)
        );
        total += sum;
        counted += pixels;
    }
    // Measured: 2.47 (3.0 before queue item 459).
    assert!(
        total * 100 <= 250 * counted,
        "the controls differ from Chrome's by {} on average",
        f64::from(total) / f64::from(counted)
    );
}

#[test]
fn a_disabled_control_over_black_is_chromes_without_a_seam() {
    // Queue item 463: over black, a seam that lets the page through is as
    // dark as the page, and Chrome's has none. Which controls stand on
    // black is read from Chrome's picture: the pixel left of the square.
    let Some(pictures) = pictures() else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    let over_black = pictures
        .disabled
        .iter()
        .filter(|(left, top, _)| {
            pictures
                .chrome
                .at(left.saturating_sub(1), *top)
                .is_some_and(|pixel| pixel.to_rgba8() == (0, 0, 0, 255))
        })
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(over_black.len(), 5, "the dark row's five disabled controls");
    for (left, top, span) in over_black {
        let mut worst = 0u8;
        let mut sum = 0u32;
        let mut pixels = 0u32;
        for y in top..top + span {
            for x in left..left + span {
                let Some(d) = difference(pictures.ours.at(x, y), pictures.chrome.at(x, y)) else {
                    panic!("({x}, {y}) is on both pictures");
                };
                worst = worst.max(d);
                sum += u32::from(d);
                pixels += 1;
            }
        }
        // Measured: 24 at worst and 2.96 on average, a disabled radio, where
        // the seam let 44 and 9.22 through. What is left is where Chrome's
        // ring runs on the diagonal at one to one (queue item 578).
        assert!(
            worst <= 24,
            "the disabled control at ({left}, {top}) differs from Chrome's by {worst} over black"
        );
        assert!(
            sum * 100 <= 300 * pixels,
            "the disabled control at ({left}, {top}) differs from Chrome's by {} over black",
            f64::from(sum) / f64::from(pixels)
        );
    }
}

#[test]
fn a_radio_is_drawn_alike_on_both_sides() {
    // Queue item 459: a dot's left edge was drawn 159 and its right 191,
    // where Chrome draws both 153, because the rented rasteriser's sampling
    // leans a small curve to one side. Every radio here is on whole pixels,
    // so each pixel is its mirror image's across the middle; Chrome's are to
    // within a unit.
    let Some(pictures) = pictures() else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    assert_eq!(pictures.radios.len(), 13, "the case's thirteen radios");
    for &(left, top, span) in &pictures.radios {
        for y in top..top + span {
            for x in left..left + span {
                let mirrored = 2 * left + span - 1 - x;
                let d = difference(pictures.ours.at(x, y), pictures.ours.at(mirrored, y));
                assert!(
                    d.is_some_and(|d| d <= 2),
                    "the radio at ({left}, {top}): ({x}, {y}) and ({mirrored}, {y}) differ by {d:?}"
                );
            }
        }
    }
}

#[test]
fn inside_each_shape_the_colour_is_chromes() {
    let Some(pictures) = pictures() else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    // A pixel inside each kind of shape, away from every edge, which Chrome
    // fills flat: what the colours in `alo_paint::toggle` were measured from.
    // A disabled checked checkbox's is read four rows up from where it was
    // first: two in and one up from its corner is a fiftieth inside the
    // edge's inner curve, which the rented sampling used to miss (459).
    for (x, y, what) in [
        (10, 10, "a live checkbox's white inside"),
        (5, 10, "its #767676 edge"),
        (27, 15, "a checked checkbox's #0075ff"),
        (73, 10, "a checked radio's dot"),
        (68, 10, "the white ring around it"),
        (67, 10, "its accent edge"),
        (94, 10, "a disabled inside over white"),
        (88, 10, "a disabled edge over white"),
        (111, 15, "a disabled fill over white"),
        (10, 52, "a disabled inside over black"),
        (4, 52, "a disabled edge over black"),
        (27, 50, "a disabled fill over black"),
        (73, 52, "a disabled dot over black"),
        (90, 57, "a checked checkbox's `accent-color`"),
        (111, 50, "a disabled one, whatever its accent"),
        (14, 77, "a 20-pixel checkbox's inside"),
    ] {
        let d = difference(pictures.ours.at(x, y), pictures.chrome.at(x, y));
        assert!(
            d.is_some_and(|d| d <= 1),
            "{what} at ({x}, {y}): {:?} here, {:?} in Chrome",
            pictures.ours.at(x, y).map(Rgba::to_rgba8),
            pictures.chrome.at(x, y).map(Rgba::to_rgba8)
        );
    }
}

#[test]
fn outside_the_controls_the_page_is_chromes() {
    let Some(pictures) = pictures() else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    // The mixed pair are in their own row's last 52 pixels across.
    let near_a_control = |x: u32, y: u32| {
        pictures.controls.iter().any(|&(left, top, span)| {
            x + 1 >= left && x <= left + span && y + 1 >= top && y <= top + span
        }) || (95..115).contains(&y) && x < 53
    };
    for y in 0..pictures.chrome.height() {
        for x in 0..pictures.chrome.width() {
            if near_a_control(x, y) {
                continue;
            }
            assert_eq!(
                difference(pictures.ours.at(x, y), pictures.chrome.at(x, y)),
                Some(0),
                "({x}, {y})"
            );
        }
    }
}
