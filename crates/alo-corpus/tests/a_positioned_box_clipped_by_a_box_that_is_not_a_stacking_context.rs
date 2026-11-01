/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A positioned box clipped by an `overflow` box that is not a stacking
//! context (queue item 753, CSS Overflow 3 § 3).
//!
//! `cases/a-positioned-box-clipped-by-a-box-that-is-not-a-stacking-context`
//! is an unpositioned box 120 × 80 at (20, 20) with `overflow: hidden`,
//! holding a relatively positioned box moved 80 right and 10 down, past its
//! right edge; a relatively positioned holder with an absolutely positioned
//! box inside it, past its right and bottom edges; and an absolutely
//! positioned box whose containing block is the page's, across its left and
//! bottom edges. A positioned box is painted with the stacking context it
//! belongs to — here the page's — and this engine painted all three there,
//! past the clip: the first two are clipped, as their containing blocks are
//! inside the clipping box, and the third is not.
//!
//! `chrome.png` beside the case is Chrome 156.0.8078.76's screenshot of the
//! same bytes, its sheet inlined after the doctype, 240 by 200 CSS pixels,
//! headless, `--force-device-scale-factor=1 --hide-scrollbars`, taken
//! 2026-11-01 and saved as RGBA. Every element a point finds below is the
//! one its `elementFromPoint` found (headless, its viewport 113 tall, so
//! points under 113 only).

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;
use alo_paint::{Canvas, Point};

fn case() -> Option<Case> {
    Case::read(
        &cases_directory().join("a-positioned-box-clipped-by-a-box-that-is-not-a-stacking-context"),
    )
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
fn the_boxes_are_where_chrome_lays_them_out() {
    let case = case().expect("the case reads");
    let rendering = Rendering::of(&case).expect("the case renders");
    // Chrome's `getBoundingClientRect` of each.
    for (id, rect) in [
        ("clip", Rect::new(20.0, 20.0, 120.0, 80.0)),
        ("relative", Rect::new(100.0, 30.0, 80.0, 30.0)),
        ("holder", Rect::new(20.0, 50.0, 120.0, 10.0)),
        ("held", Rect::new(110.0, 70.0, 60.0, 40.0)),
        ("free", Rect::new(10.0, 90.0, 100.0, 40.0)),
    ] {
        assert_eq!(border_box_of(&rendering, id), Some(rect), "{id}");
    }
}

#[test]
fn the_held_two_are_clipped_and_the_free_one_is_not() {
    let case = case().expect("the case reads");
    let rendering = Rendering::of(&case).expect("the case renders");
    let canvas = &rendering.drawing().expect("it drew").canvas;
    let at = |x: u32, y: u32| {
        canvas
            .at(x, y)
            .map(alo_value::Rgba::to_rgba8)
            .map(|(r, g, b, a)| [r, g, b, a])
    };
    let blue = Some([0x1d, 0x4e, 0xd8, 0xff]);
    let green = Some([0x2f, 0x6b, 0x5e, 0xff]);
    let red = Some([0xe6, 0x39, 0x46, 0xff]);
    let white = Some([0xff, 0xff, 0xff, 0xff]);
    // The relative box inside the clip, and nothing of it past 140.
    assert_eq!(at(139, 40), blue);
    assert_eq!(at(140, 40), white);
    // The held box inside the clip, and nothing of it past 140 or 100.
    assert_eq!(at(139, 99), green);
    assert_eq!(at(140, 75), white);
    assert_eq!(at(120, 100), white);
    // The free box across the clip's left and bottom edges, whole.
    assert_eq!(at(10, 90), red);
    assert_eq!(at(15, 129), red);
    assert_eq!(at(105, 125), red);
}

#[test]
fn a_pointer_finds_nothing_clipped_away_as_chrome_finds() {
    let case = case().expect("the case reads");
    let rendering = Rendering::of(&case).expect("the case renders");
    let document = rendering.document().expect("a document");
    let drawing = rendering.drawing().expect("it drew");
    let found = |x: f32, y: f32| {
        alo_paint::box_at(&drawing.display, Point::new(x, y))
            .and_then(|id| drawing.boxes.get(id))
            .and_then(|held| held.kind.node())
            .map(|node| {
                document.attribute_by_name(node, "id").map_or_else(
                    || {
                        document
                            .element(node)
                            .map_or_else(String::new, |element| element.name.local.to_uppercase())
                    },
                    str::to_owned,
                )
            })
    };
    // Chrome's `elementFromPoint` at each: past the clip, the box behind.
    for (x, y, chrome) in [
        (130.0, 40.0, "relative"),
        (150.0, 40.0, "BODY"),
        (130.0, 55.0, "holder"),
        (120.0, 75.0, "held"),
        (150.0, 75.0, "BODY"),
        (120.0, 99.0, "held"),
        (15.0, 95.0, "free"),
        (30.0, 25.0, "clip"),
    ] {
        assert_eq!(found(x, y).as_deref(), Some(chrome), "({x}, {y})");
    }
    // Under the body, where the held box was clipped away: the root, as
    // Chrome finds it.
    assert_eq!(found(120.0, 101.0).as_deref(), Some("HTML"));
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
    let differing = (0..chrome.height())
        .flat_map(|y| (0..chrome.width()).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            ours.at(x, y).map(alo_value::Rgba::to_rgba8)
                != chrome.at(x, y).map(alo_value::Rgba::to_rgba8)
        })
        .count();
    assert_eq!(differing, 0, "pixels that differ from Chrome's");
}
