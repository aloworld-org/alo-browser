/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's backgrounds beside its controls, drawn on Chrome's pixels
//! exactly: the case that closes queue item 461.
//!
//! `cases/checkbox-beside-text` is nine rows, each a `div` with a
//! background, holding text and checkboxes and radios laid out at fractions
//! of a pixel across. Chrome 154 draws each background crisply on whole
//! pixels. Until item 457 this engine smoothed edges across two pixels;
//! until item 460 the rows themselves stood at fractions down, so two
//! rows' backgrounds met at y 157.x with a blended row between them, and
//! once item 459 sampled edges finely the page showed two levels through
//! that seam. Item 460 put every row on whole pixels and item 565 drew
//! every box's own area on whole device pixels (`alo_paint::snap`).
//!
//! `chrome.png` beside the case is Chrome's screenshot of the same bytes,
//! with the corpus's font, at one device pixel to one and without
//! `--disable-gpu`. Every pixel outside a control and a run of text — the
//! whole pixels that cover each one's box — is Chrome's exactly: 49,525 of
//! the page's 60,000. The controls are `a_control_on_whole_pixels.rs`'
//! and the letters are item 462's.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;
use alo_paint::Canvas;
use alo_value::Rgba;

/// The case's picture here, Chrome's, and the whole pixels each control and
/// run of text here covers, as left, top, right and bottom.
struct Pictures {
    ours: Canvas,
    chrome: Canvas,
    covered: Vec<(u32, u32, u32, u32)>,
}

fn pictures() -> Option<Pictures> {
    let directory = cases_directory().join("checkbox-beside-text");
    let case = Case::read(&directory)?;
    let rendering = Rendering::of(&case).ok()?;
    let drawing = rendering.drawing()?;
    let chrome = alo_paint::from_png(&std::fs::read(directory.join("chrome.png")).ok()?).ok()?;
    let boxes = &drawing.boxes;
    let covered = boxes
        .ids()
        .filter(|id| {
            boxes.toggle(*id).is_some()
                || matches!(
                    boxes.get(*id).map(|node| &node.kind),
                    Some(alo_box::BoxKind::Text { .. })
                )
        })
        .filter_map(|id| Some(covering(drawing.layout.get(id)?.border_box)))
        .collect();
    Some(Pictures {
        ours: drawing.canvas.clone(),
        chrome,
        covered,
    })
}

/// The whole pixels a box touches, any part of them.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "whole, positive coordinates on a 300 × 200 page"
)]
fn covering(rect: Rect) -> (u32, u32, u32, u32) {
    let whole = |value: f32| value.max(0.0) as u32;
    (
        whole(rect.origin.x.floor()),
        whole(rect.origin.y.floor()),
        whole((rect.origin.x + rect.size.width).ceil()),
        whole((rect.origin.y + rect.size.height).ceil()),
    )
}

#[test]
fn every_pixel_outside_the_controls_and_text_is_chromes() {
    let Some(pictures) = pictures() else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    // Nine controls and sixteen runs of text.
    assert_eq!(pictures.covered.len(), 25);
    assert_eq!(
        (pictures.ours.width(), pictures.ours.height()),
        (pictures.chrome.width(), pictures.chrome.height())
    );
    let mut compared = 0u32;
    let mut differing = Vec::new();
    for y in 0..pictures.ours.height() {
        for x in 0..pictures.ours.width() {
            let inside = pictures.covered.iter().any(|&(left, top, right, bottom)| {
                (left..right).contains(&x) && (top..bottom).contains(&y)
            });
            if inside {
                continue;
            }
            compared += 1;
            let here = pictures.ours.at(x, y).map(Rgba::to_rgba8);
            let there = pictures.chrome.at(x, y).map(Rgba::to_rgba8);
            if here != there {
                differing.push((x, y, here, there));
            }
        }
    }
    // The page less the pixels covered; a box that grew would shrink it.
    assert_eq!(compared, 49_525);
    assert!(
        differing.is_empty(),
        "{} pixels differ from Chrome's, the first {:?}",
        differing.len(),
        differing.first()
    );
}
