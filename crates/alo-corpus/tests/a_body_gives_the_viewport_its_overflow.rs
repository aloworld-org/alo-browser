/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A body whose `overflow` the viewport takes clips nothing (queue item
//! 662, CSS Overflow 3 § 3.3).
//!
//! `cases/a-body-gives-the-viewport-its-overflow` is a body of
//! `overflow: auto` and 50 pixels under a root of `visible`, holding a blue
//! block 130 tall and a red one below it. The viewport takes the body's
//! `overflow`, so the body's own is `visible`: the blue runs down past the
//! body's yellow and the red is drawn at 130. This engine clipped both at
//! the body's 50 pixels.
//!
//! `chrome.png` beside the case is Chrome 155.0.8059.86's screenshot of the
//! same bytes, its sheet inlined after the doctype, 240 by 160 CSS pixels,
//! headless, `--force-device-scale-factor=1 --hide-scrollbars`, taken
//! 2026-10-25 and saved as RGBA.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;
use alo_paint::Canvas;

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("a-body-gives-the-viewport-its-overflow"))
}

/// The border box of the element whose `id` is `id`, as laid out.
fn border_box_of(rendering: &Rendering, id: &str) -> Option<Rect> {
    let document = rendering.document()?;
    let drawing = rendering.drawing()?;
    let element = document
        .descendants(document.root())
        .find(|&each| document.attribute_by_name(each, "id") == Some(id))?;
    drawing
        .boxes
        .ids()
        .find(|&each| drawing.boxes.get(each).and_then(|held| held.kind.node()) == Some(element))
        .and_then(|each| drawing.layout.border_box(each))
}

#[test]
fn the_body_is_fifty_tall_and_what_it_holds_is_laid_out_past_it() {
    let case = case().expect("the case reads");
    let rendering = Rendering::of(&case).expect("the case renders");
    let drawing = rendering.drawing().expect("it drew");
    let body = drawing
        .boxes
        .body()
        .and_then(|id| drawing.layout.border_box(id))
        .expect("the body was laid out");
    assert_eq!(body, Rect::new(0.0, 0.0, 240.0, 50.0));
    assert_eq!(
        border_box_of(&rendering, "tall"),
        Some(Rect::new(0.0, 0.0, 60.0, 130.0))
    );
    assert_eq!(
        border_box_of(&rendering, "below"),
        Some(Rect::new(100.0, 130.0, 30.0, 20.0))
    );
    let body_id = drawing.boxes.body().expect("the body has a box");
    assert!(drawing.layout.gives_its_overflow_to_the_viewport(body_id));
    assert!(
        !drawing.display.to_outline().contains("clip"),
        "nothing is clipped:\n{}",
        drawing.display.to_outline()
    );
}

#[test]
fn it_is_drawn_as_chrome_draws_it() {
    let case = case().expect("the case reads");
    let rendering = Rendering::of(&case).expect("the case renders");
    let ours = &rendering.drawing().expect("it drew").canvas;
    let bytes = std::fs::read(case.expectation("chrome.png")).expect("chrome.png is beside it");
    let chrome: Canvas = alo_paint::from_png(&bytes).expect("chrome.png reads");
    assert_eq!(
        (ours.width(), ours.height()),
        (chrome.width(), chrome.height())
    );
    let differing: Vec<(u32, u32)> = (0..chrome.height())
        .flat_map(|y| (0..chrome.width()).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            ours.at(x, y).map(alo_value::Rgba::to_rgba8)
                != chrome.at(x, y).map(alo_value::Rgba::to_rgba8)
        })
        .collect();
    assert!(
        differing.is_empty(),
        "{} pixels differ from Chrome's, first {:?}",
        differing.len(),
        differing.first()
    );
}
