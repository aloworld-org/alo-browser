/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A disabled button and a disabled field, in Chrome's colours: the case
//! that closes queue item 509.
//!
//! Chrome 154 computes a disabled button's text as `rgba(16, 16, 16, 0.3)`
//! and a field's as `rgb(84, 84, 84)`, and edges and tints both in
//! translucent greys. The first two controls draw their own pictures
//! (`alo_paint`'s `face.rs`), so their text is what the sheet's colours
//! change. The last two have a page's background, so the sheet's border is
//! drawn as CSS, in two tones of a translucent grey, and the page's
//! background must beat the sheet's `background-color`.
//!
//! `chrome.png` beside the case is Chrome's screenshot of the same bytes,
//! with the corpus's font, at one device pixel to one and without
//! `--disable-gpu`. Every box is on whole pixels, here and there.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;
use alo_paint::Canvas;
use alo_value::Rgba;

/// Each control in the case, in document order, as Chrome 154's
/// `getBoundingClientRect()` gave it.
fn boxes() -> [(&'static str, Rect); 4] {
    [
        ("button", Rect::new(8.0, 8.0, 96.0, 21.0)),
        ("field", Rect::new(8.0, 37.0, 96.0, 21.0)),
        ("button with a background", Rect::new(8.0, 66.0, 96.0, 21.0)),
        ("field with a background", Rect::new(8.0, 95.0, 96.0, 21.0)),
    ]
}

/// The case, its rendering and Chrome's screenshot of it.
fn case() -> Option<(Rendering, Canvas)> {
    let directory = cases_directory().join("a-disabled-control");
    let case = Case::read(&directory)?;
    let rendering = Rendering::of(&case).ok()?;
    let bytes = std::fs::read(directory.join("chrome.png")).ok()?;
    let chrome = alo_paint::from_png(&bytes).ok()?;
    Some((rendering, chrome))
}

/// A pixel's channels, as bytes.
fn rgb(pixel: Option<Rgba>) -> Option<[u8; 3]> {
    let (red, green, blue, _) = pixel?.to_rgba8();
    Some([red, green, blue])
}

/// The largest difference in any channel between two pixels.
fn difference(a: [u8; 3], b: [u8; 3]) -> u8 {
    a.into_iter()
        .zip(b)
        .map(|(a, b)| a.abs_diff(b))
        .max()
        .unwrap_or_default()
}

/// A box's edges as whole pixels.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the case's boxes are on whole pixels, inside the page"
)]
fn pixels(area: Rect) -> (u32, u32, u32, u32) {
    (
        area.origin.x as u32,
        area.origin.y as u32,
        area.right() as u32,
        area.bottom() as u32,
    )
}

/// Every pixel of a box's two-pixel border and one pixel outside it, as
/// the difference between the two pictures: the worst on a side and the
/// worst in a corner.
fn band(ours: &Canvas, chrome: &Canvas, area: Rect) -> Option<(u8, u8)> {
    let (left, top, right, bottom) = pixels(area);
    let (mut sides, mut corners) = (0, 0);
    for y in top - 1..=bottom {
        for x in left - 1..=right {
            let across = x < left + 2 || x >= right - 2;
            let down = y < top + 2 || y >= bottom - 2;
            let difference = difference(rgb(ours.at(x, y))?, rgb(chrome.at(x, y))?);
            if across && down {
                corners = corners.max(difference);
            } else if across || down {
                sides = sides.max(difference);
            }
        }
    }
    Some((sides, corners))
}

/// The darkest pixel inside a box's border and padding: its text's ink at
/// its fullest.
fn darkest(canvas: &Canvas, area: Rect) -> Option<[u8; 3]> {
    let (left, top, right, bottom) = pixels(area);
    let mut found: Option<[u8; 3]> = None;
    for y in top + 3..bottom - 3 {
        for x in left + 3..right - 3 {
            let pixel = rgb(canvas.at(x, y))?;
            let sum = |p: [u8; 3]| p.iter().map(|c| u32::from(*c)).sum::<u32>();
            if found.is_none_or(|was| sum(pixel) < sum(was)) {
                found = Some(pixel);
            }
        }
    }
    found
}

#[test]
fn every_box_is_chromes() {
    let Some((rendering, _)) = case() else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case draws");
    };
    let laid_out: Vec<Rect> = drawing
        .layout
        .iter()
        .map(|(_, held)| held.border_box)
        .filter(|rect| (rect.origin.x - 8.0).abs() < 1.0 / 64.0)
        .collect();
    for (name, chrome) in boxes() {
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
fn the_text_is_chromes_grey() {
    // Measured: the darkest pixel of each control's text is Chrome's, 171
    // on a button (a near-black at three tenths over its fill) and 84 in a
    // field, and a level off at most over the page's yellow. The letters'
    // edges differ by more, which is queue item 462's. With the sheet's
    // old `#6d6d6d` the button's is 109, 62 off.
    let Some((rendering, chrome)) = case() else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case draws");
    };
    for (name, area) in boxes() {
        let (Some(ours), Some(theirs)) = (darkest(&drawing.canvas, area), darkest(&chrome, area))
        else {
            panic!("the {name} is on both pictures");
        };
        let off = difference(ours, theirs);
        assert!(
            off <= 1,
            "the {name}'s text is {ours:?}, Chrome's {theirs:?}"
        );
    }
}

#[test]
fn the_fill_and_the_edge_are_chromes() {
    // Measured: inside each control, clear of its text, the fill is
    // Chrome's exactly, the page's yellow included where the engine's
    // sheet used to win. The sides are within 5 of Chrome's (the pictures'
    // edges, as item 505 left them) and the corners within 3, the mitre's
    // diagonal pixel (item 518).
    let Some((rendering, chrome)) = case() else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case draws");
    };
    for (name, area) in boxes() {
        let (left, top, _, _) = pixels(area);
        let (Some(ours), Some(theirs)) = (
            rgb(drawing.canvas.at(left + 4, top + 4)),
            rgb(chrome.at(left + 4, top + 4)),
        ) else {
            panic!("the {name}'s fill is on both pictures");
        };
        assert_eq!(ours, theirs, "the {name}'s fill");
        let Some((sides, corners)) = band(&drawing.canvas, &chrome, area) else {
            panic!("the {name}'s border is on both pictures");
        };
        assert!(sides <= 5, "the {name}'s sides are {sides} off at worst");
        assert!(
            corners <= 3,
            "the {name}'s corners are {corners} off at worst"
        );
    }
}
