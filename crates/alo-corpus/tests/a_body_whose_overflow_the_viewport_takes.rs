/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A body whose `overflow` the viewport takes is `visible` in use, for its
//! lines and its baseline too (queue item 664, CSS Overflow 3 § 3.3).
//!
//! `cases/a-body-whose-overflow-the-viewport-takes-ends-no-line` is a body
//! 61 wide of `overflow: hidden; text-overflow: ellipsis; white-space:
//! nowrap` under a root of `visible`, holding `aaaa bbbbbbbbb`. Chrome 156
//! draws the whole line past the body's edge and no "…"; this engine read
//! the body's computed `overflow` and ended the line `aaaa …`.
//!
//! `cases/a-body-whose-overflow-the-viewport-takes-stands-on-its-last-line`
//! is a body of `display: inline-block; overflow: hidden` and two lines 20
//! tall, in a root of `line-height: 40px`. Chrome 156 stands it on its last
//! line, making the root 50 tall (`getBoundingClientRect`, two runs alike);
//! under a root of `auto` it stands on its bottom margin edge and the root
//! is 55. This engine stood it on its bottom margin edge here as well.
//!
//! `chrome.png` beside each case is Chrome 156.0.8078.73's screenshot of the
//! same bytes, its sheet inlined after the doctype and every element set in
//! `DejaVu Sans` by `@font-face` as the corpus's fonts are, 200 by 60 CSS
//! pixels, headless, `--force-device-scale-factor=1 --hide-scrollbars`,
//! taken 2026-10-29 and saved as RGBA. Chrome on macOS draws its letters
//! heavier than this engine does, so the pictures are compared by where
//! their ink is rather than pixel for pixel.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;
use alo_paint::Canvas;

fn case(name: &str) -> Option<Case> {
    Case::read(&cases_directory().join(format!("a-body-whose-overflow-the-viewport-takes-{name}")))
}

/// The leftmost, rightmost, top and bottom columns and rows holding ink —
/// a pixel darker than mid-grey — or `None` for a picture with none.
fn ink(canvas: &Canvas) -> Option<(u32, u32, u32, u32)> {
    let dark: Vec<(u32, u32)> = (0..canvas.height())
        .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            canvas.at(x, y).is_some_and(|pixel| {
                let (red, green, blue, _) = pixel.to_rgba8();
                u32::from(red) + u32::from(green) + u32::from(blue) < 300
            })
        })
        .collect();
    Some((
        dark.iter().map(|&(x, _)| x).min()?,
        dark.iter().map(|&(x, _)| x).max()?,
        dark.iter().map(|&(_, y)| y).min()?,
        dark.iter().map(|&(_, y)| y).max()?,
    ))
}

/// Chrome's picture of `case`, beside it.
fn chromes_picture(case: &Case) -> Option<Canvas> {
    let bytes = std::fs::read(case.expectation("chrome.png")).ok()?;
    alo_paint::from_png(&bytes).ok()
}

#[test]
fn the_bodys_line_is_not_ended_and_runs_on_past_it() {
    let case = case("ends-no-line").expect("the case reads");
    let rendering = Rendering::of(&case).expect("the case renders");
    let drawing = rendering.drawing().expect("it drew");
    let body = drawing.boxes.body().expect("the body has a box");
    assert!(drawing.layout.gives_its_overflow_to_the_viewport(body));
    assert_eq!(
        drawing.layout.border_box(body),
        Some(Rect::new(0.0, 0.0, 61.0, 22.0))
    );
    assert!(
        drawing
            .boxes
            .ids()
            .all(|id| drawing.layout.ellipses(id).is_empty()),
        "no box ends a line"
    );
    let outline = drawing.display.to_outline();
    assert!(
        outline.contains("\"aaaa bbbbbbbbb\"")
            && !outline.contains('…')
            && !outline.contains("clip"),
        "the whole line, unclipped, and no \"…\":\n{outline}"
    );
    // Chrome's ink runs from column 1 to 134, past the body's 61.
    let ours = &drawing.canvas;
    let chrome = chromes_picture(&case).expect("chrome.png is beside it");
    assert_eq!(
        (ours.width(), ours.height()),
        (chrome.width(), chrome.height())
    );
    let (ours, chromes) = (ink(ours), ink(&chrome));
    assert_eq!(chromes, Some((1, 134, 3, 14)));
    assert_eq!(ours, chromes);
}

#[test]
fn the_inline_block_body_stands_on_its_last_line() {
    let case = case("stands-on-its-last-line").expect("the case reads");
    let rendering = Rendering::of(&case).expect("the case renders");
    let drawing = rendering.drawing().expect("it drew");
    let body = drawing.boxes.body().expect("the body has a box");
    let root = drawing.boxes.root().expect("the root has a box");
    assert!(drawing.layout.gives_its_overflow_to_the_viewport(body));
    assert_eq!(
        drawing.layout.border_box(body),
        Some(Rect::new(0.0, 0.0, 29.78125, 40.0))
    );
    // On its bottom margin edge, the root would be 55, as Chrome makes it
    // under a root of `auto`.
    assert_eq!(
        drawing.layout.border_box(root),
        Some(Rect::new(0.0, 0.0, 200.0, 50.0))
    );
    let ours = &drawing.canvas;
    let chrome = chromes_picture(&case).expect("chrome.png is beside it");
    assert_eq!(
        (ours.width(), ours.height()),
        (chrome.width(), chrome.height())
    );
    let (ours, chromes) = (ink(ours), ink(&chrome));
    assert_eq!(chromes, Some((1, 28, 6, 34)));
    assert_eq!(ours, chromes);
}
