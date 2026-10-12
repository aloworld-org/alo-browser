/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A button whose page gave it a background, drawn in Chrome's two tones of
//! black: the case that closes queue item 508.
//!
//! A control whose page writes a background loses its own picture (item
//! 505), and what is drawn is the user-agent sheet's border: `2px outset
//! black` for a button and a submit, `2px inset #767676` for a field. Until
//! item 508 black's two tones were black and a grey of 85. Chrome 154 draws
//! a colour that dark lighter instead: 168 on the top and left and 84 on the
//! bottom and right (`alo_paint`'s `tone.rs`).
//!
//! `chrome.png` beside the case is Chrome's screenshot of the same bytes,
//! with the corpus's font, at one device pixel to one and without
//! `--disable-gpu`. Every box is on whole pixels, here and there.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;
use alo_paint::Canvas;
use alo_value::Rgba;

/// Each bordered box in the case, in document order: its border box as
/// Chrome 154's `getBoundingClientRect()` gave it, and its border's width.
fn boxes() -> [(&'static str, Rect, u32); 5] {
    [
        ("button", Rect::new(8.0, 8.0, 96.0, 21.0), 2),
        ("submit", Rect::new(8.0, 37.0, 96.0, 21.0), 2),
        ("field", Rect::new(8.0, 66.0, 96.0, 21.0), 2),
        ("black groove", Rect::new(8.0, 95.0, 32.0, 24.0), 4),
        ("dark ridge", Rect::new(8.0, 127.0, 32.0, 24.0), 4),
    ]
}

/// The case, its rendering and Chrome's screenshot of it.
fn case() -> Option<(Rendering, Canvas)> {
    let directory = cases_directory().join("a-button-with-its-own-background");
    let case = Case::read(&directory)?;
    let rendering = Rendering::of(&case).ok()?;
    let bytes = std::fs::read(directory.join("chrome.png")).ok()?;
    let chrome = alo_paint::from_png(&bytes).ok()?;
    Some((rendering, chrome))
}

/// The largest difference in any channel between two pixels.
fn difference(a: Option<Rgba>, b: Option<Rgba>) -> Option<u8> {
    let (ar, ag, ab, _) = a?.to_rgba8();
    let (br, bg, bb, _) = b?.to_rgba8();
    [ar.abs_diff(br), ag.abs_diff(bg), ab.abs_diff(bb)]
        .into_iter()
        .max()
}

/// Every pixel of a box's border and one pixel outside it, as the
/// difference between the two pictures, split into the pixels in a corner
/// (where two sides meet on a mitre) and the rest.
fn band(ours: &Canvas, chrome: &Canvas, area: Rect, width: u32) -> Option<(Vec<u8>, Vec<u8>)> {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the case's boxes are on whole pixels, inside the page"
    )]
    let (left, top, right, bottom) = (
        area.origin.x as u32,
        area.origin.y as u32,
        area.right() as u32,
        area.bottom() as u32,
    );
    let (mut sides, mut corners) = (Vec::new(), Vec::new());
    for y in top - 1..=bottom {
        for x in left - 1..=right {
            let across = x < left + width || x >= right - width;
            let down = y < top + width || y >= bottom - width;
            if !across && !down {
                continue;
            }
            let difference = difference(ours.at(x, y), chrome.at(x, y))?;
            if across && down {
                corners.push(difference);
            } else {
                sides.push(difference);
            }
        }
    }
    Some((sides, corners))
}

#[test]
fn every_box_is_chromes() {
    let Some((rendering, _)) = case() else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case draws");
    };
    // The body's flex items, in order, are the five boxes.
    let laid_out: Vec<Rect> = drawing
        .layout
        .iter()
        .map(|(_, held)| held.border_box)
        .filter(|rect| (rect.origin.x - 8.0).abs() < 1.0 / 64.0)
        .filter(|rect| rect.size.width > 30.0)
        .collect();
    for (name, chrome, _) in boxes() {
        assert!(
            laid_out.iter().any(|rect| {
                (rect.origin.y - chrome.origin.y).abs() < 1.0 / 64.0
                    && (rect.size.width - chrome.size.width).abs() < 1.0 / 64.0
                    && (rect.size.height - chrome.size.height).abs() < 1.0 / 64.0
            }),
            "the {name} is Chrome's {chrome:?}: {laid_out:?}"
        );
    }
}

#[test]
fn every_side_is_drawn_in_chromes_tones() {
    // Measured: every pixel of every side, and one outside it, is Chrome's
    // exactly, the black button's 168 and 84 included. With black's tones
    // as they were before item 508 the button's sides are 84 off at worst.
    let Some((rendering, chrome)) = case() else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case draws");
    };
    for (name, area, width) in boxes() {
        let Some((sides, _)) = band(&drawing.canvas, &chrome, area, width) else {
            panic!("the {name}'s border is on both pictures");
        };
        let worst = sides.iter().copied().max().unwrap_or_default();
        assert_eq!(worst, 0, "the {name}'s sides are {worst} off at worst");
    }
}

#[test]
fn a_corners_mitre_is_within_the_measured_bound() {
    // The tones are Chrome's in the corners too. What differs is the one
    // pixel on each mitre's diagonal, which Chrome draws as the two tones'
    // average and this engine as one wedge over the other: 141 against
    // Chrome's 126 for the button, and 49 at worst for the field. That is
    // queue item 518's to close, and until then this is the bound.
    let Some((rendering, chrome)) = case() else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case draws");
    };
    for (name, area, width) in boxes() {
        let Some((_, corners)) = band(&drawing.canvas, &chrome, area, width) else {
            panic!("the {name}'s border is on both pictures");
        };
        let worst = corners.iter().copied().max().unwrap_or_default();
        assert!(worst <= 49, "the {name}'s corners are {worst} off at worst");
    }
}
