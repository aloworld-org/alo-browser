/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Letters against Chrome's, by how much of each pixel they cover: the case
//! that closes queue item 462.
//!
//! `cases/letters-at-three-sizes` is `DejaVu Sans`, black on white, at 12,
//! 13 and 16 px, two lines at each, every baseline on a whole pixel. Beside
//! it are two screenshots of the same bytes from Chrome 154 at one device
//! pixel to one: `chrome.png`, as Chrome draws a page, and
//! `chrome-antialiased.png`, the same with `-webkit-font-smoothing:
//! antialiased`, which on macOS turns off the thickening Chrome otherwise
//! gives every letter.
//!
//! A letter's ink is `255` less its grey. Each line is the 24 rows it
//! stands in, and is scored over the pixels inked in either picture: the
//! mean of the difference in ink, and the ratio of the line's whole ink to
//! Chrome's.
//!
//! **What was measured, and what the bounds are.** Letters sampled four by
//! four, as the rented rasteriser samples, and sixteen by sixteen where an
//! edge is, as every other shape here is (`alo_paint::refine`):
//!
//! | line | rented, plain | fine, plain | rented, antialiased | fine, antialiased |
//! |------|---------------|-------------|---------------------|-------------------|
//! | 12 a | 25.37 | 23.45 | 23.57 | 20.20 |
//! | 12 b | 24.44 | 22.70 | 22.72 | 19.78 |
//! | 13 a | 42.14 | 38.89 | 41.04 | 37.08 |
//! | 13 b | 37.89 | 36.11 | 37.83 | 35.26 |
//! | 16 a | 22.18 | 21.20 | 17.12 | 15.39 |
//! | 16 b | 22.61 | 22.22 | 20.02 | 16.60 |
//!
//! Every line is nearer with the finer sampling, so letters are sampled as
//! shapes are. The bounds below are those measurements, rounded up by at
//! most a half, and the four-by-four sampling fails every size of them.
//!
//! **What is left is not sampling.** Against `chrome.png` this engine's
//! letters carry 12 to 16 per cent less ink at every size: Chrome on macOS
//! thickens them (item 576). Against `chrome-antialiased.png` the ink is
//! within 2 per cent at 12 and 16 px, and the differences that remain are
//! at the tops of letters: Chrome pulls an `x`'s top and a capital's up to
//! a whole pixel where this engine draws the outline where the font puts
//! it, which at 13 px is most of a row (item 577).

use alo_corpus::{Case, Rendering, cases_directory};
use alo_paint::Canvas;
use alo_value::Rgba;

/// Rows each line stands in.
const LINE: u32 = 24;

/// The case's picture here, and one of Chrome's beside it.
fn pictures(chrome: &str) -> Option<(Canvas, Canvas)> {
    let directory = cases_directory().join("letters-at-three-sizes");
    let case = Case::read(&directory)?;
    let rendering = Rendering::of(&case).ok()?;
    let ours = rendering.drawing()?.canvas.clone();
    let theirs = alo_paint::from_png(&std::fs::read(directory.join(chrome)).ok()?).ok()?;
    Some((ours, theirs))
}

/// How much ink a pixel holds: `255` less its red, the page being grey.
fn ink(canvas: &Canvas, x: u32, y: u32) -> f64 {
    canvas
        .at(x, y)
        .map_or(0.0, |pixel| 255.0 - f64::from(Rgba::to_rgba8(pixel).0))
}

/// One line's mean difference in ink over the pixels either picture inks,
/// and its whole ink over Chrome's.
fn score(ours: &Canvas, chrome: &Canvas, line: u32) -> (f64, f64) {
    let (mut difference, mut inked, mut here, mut there) = (0.0, 0u32, 0.0, 0.0);
    for y in line * LINE..(line + 1) * LINE {
        for x in 0..ours.width() {
            let (a, b) = (ink(ours, x, y), ink(chrome, x, y));
            if a > 0.0 || b > 0.0 {
                difference += (a - b).abs();
                inked += 1;
                here += a;
                there += b;
            }
        }
    }
    (difference / f64::from(inked.max(1)), here / there.max(1.0))
}

/// Score every line against one of Chrome's pictures and hold each to its
/// size's bound: what is wrong, with every line's numbers, or nothing.
fn within(chrome: &str, bounds: [f64; 3], ink: (f64, f64)) -> Result<(), String> {
    let (ours, theirs) =
        pictures(chrome).ok_or(format!("the case renders and Chrome's {chrome} reads"))?;
    if (ours.width(), ours.height()) != (theirs.width(), theirs.height()) {
        return Err(format!("{chrome} is not the case's size"));
    }
    let scores: Vec<(f64, f64)> = (0..6).map(|line| score(&ours, &theirs, line)).collect();
    for (line, &(difference, ratio)) in scores.iter().enumerate() {
        let bound = bounds.get(line / 2).copied().unwrap_or(0.0);
        if difference > bound {
            return Err(format!(
                "line {line} against {chrome}: {difference:.2} past {bound}; every line {scores:?}"
            ));
        }
        if !(ink.0..=ink.1).contains(&ratio) {
            return Err(format!(
                "line {line}'s ink against {chrome}: {ratio:.3} outside {ink:?}; every line {scores:?}"
            ));
        }
    }
    Ok(())
}

#[test]
fn letters_are_within_the_measured_bound_of_chromes_unthickened_letters() {
    assert_eq!(
        within("chrome-antialiased.png", [20.5, 37.5, 17.0], (0.93, 1.0)),
        Ok(())
    );
}

#[test]
fn letters_are_within_the_measured_bound_of_chromes_as_it_draws_a_page() {
    // A twelfth to a sixth less ink than Chrome's thickened letters, at
    // every size (item 576).
    assert_eq!(
        within("chrome.png", [23.75, 39.25, 22.5], (0.83, 0.9)),
        Ok(())
    );
}
