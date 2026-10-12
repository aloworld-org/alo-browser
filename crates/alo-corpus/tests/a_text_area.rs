/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A text area draws a field's picture: the cases that close queue item
//! 510.
//!
//! Chrome 154 computes a `1px solid rgb(118, 118, 118)` border for a
//! `<textarea>` and does not draw it while the page leaves its border and
//! background alone. It draws a field's picture in that pixel instead: a
//! one-pixel rounded edge over a white fill, translucent when disabled.
//! Measured with the grip off, its textarea and a field of the same size
//! are the same pixels at one and at eight device pixels to one, live,
//! disabled and read-only, over black, white and a green.
//!
//! `a-text-area` is a live and a disabled text area, a field the same
//! size between them, one with a page's background and one that can be
//! resized. `web-a-form` is the HTML specification's pizza form, whose
//! text area closes the item. Each case's `chrome.png` is Chrome's
//! screenshot of the same bytes, with the corpus's fonts standing for the
//! families Chrome computes, at one device pixel to one and without
//! `--disable-gpu`.
//!
//! Chrome's text area also draws a grip in its corner while it can be
//! resized, which is `resize`'s and queue item 522's, so the comparisons
//! leave the grip's corner out where the grip is drawn.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;
use alo_paint::Canvas;
use alo_value::Rgba;

/// How far into a corner the grip reaches, in pixels, from its box's
/// right and bottom edges. Measured on Chrome's screenshot: its strokes
/// lie between 3 and 12 pixels in.
const GRIP: u32 = 14;

/// The named case, its rendering and Chrome's screenshot of it.
fn case(name: &str) -> Option<(Rendering, Canvas)> {
    let directory = cases_directory().join(name);
    let case = Case::read(&directory)?;
    let rendering = Rendering::of(&case).ok()?;
    let bytes = std::fs::read(directory.join("chrome.png")).ok()?;
    let chrome = alo_paint::from_png(&bytes).ok()?;
    Some((rendering, chrome))
}

/// The border box of each element called `name`, in document order.
fn boxes_of(rendering: &Rendering, name: &str) -> Vec<Rect> {
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        return Vec::new();
    };
    let boxes = &drawing.boxes;
    document
        .descendants(document.root())
        .filter(|id| document.element(*id).is_some_and(|e| e.name.is_html(name)))
        .filter_map(|node| {
            let id = boxes.ids().find(|id| {
                matches!(
                    boxes.get(*id).map(|held| &held.kind),
                    Some(alo_box::BoxKind::Element { node: of, .. }) if *of == node
                )
            })?;
            drawing.layout.get(id).map(|held| held.border_box)
        })
        .collect()
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

/// A box's edges moved onto whole pixels, as Chrome and this engine both
/// draw a control's picture: each edge rounded on its own, a half up.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the cases' boxes are inside their pages"
)]
fn pixels(area: Rect) -> (u32, u32, u32, u32) {
    let whole = |edge: f32| (edge + 0.5).floor() as u32;
    (
        whole(area.origin.x),
        whole(area.origin.y),
        whole(area.right()),
        whole(area.bottom()),
    )
}

/// Where the pixel `along` a side and `into` the box from it is, for each
/// of a box's four sides, counted from the side's first corner; `into`
/// starts a pixel outside the box.
fn on_side(
    side: usize,
    (left, top, right, bottom): (u32, u32, u32, u32),
    along: u32,
    into: u32,
) -> (u32, u32) {
    match side {
        0 => (left + along, top + into - 1),
        1 => (left + along, bottom - into),
        2 => (left + into - 1, top + along),
        _ => (right - into, top + along),
    }
}

/// The worst difference between two pictures of one control, each at its
/// own box, which may differ in size: on its sides, from a pixel outside
/// to three in, along all but four pixels at each end and `clear` at the
/// last; and in each corner, a square from a pixel outside to three in.
fn compared(
    ours: &Canvas,
    our_box: Rect,
    chrome: &Canvas,
    chrome_box: Rect,
    clear: u32,
) -> Option<(u8, u8)> {
    let (ours_at, chrome_at) = (pixels(our_box), pixels(chrome_box));
    let width = (ours_at.2 - ours_at.0).min(chrome_at.2 - chrome_at.0);
    let height = (ours_at.3 - ours_at.1).min(chrome_at.3 - chrome_at.1);
    let (mut sides, mut corners) = (0, 0);
    for side in 0..4 {
        let length = if side < 2 { width } else { height };
        for along in 4..length.saturating_sub(clear.max(4)) {
            for into in 0..4 {
                let (x, y) = on_side(side, ours_at, along, into);
                let (cx, cy) = on_side(side, chrome_at, along, into);
                sides = sides.max(difference(rgb(ours.at(x, y))?, rgb(chrome.at(cx, cy))?));
            }
        }
    }
    for corner in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        for down in 0..5 {
            for across in 0..5 {
                let at = |(left, top, right, bottom): (u32, u32, u32, u32)| {
                    let x = if corner.0 == 0 {
                        left + across - 1
                    } else {
                        right - across
                    };
                    let y = if corner.1 == 0 {
                        top + down - 1
                    } else {
                        bottom - down
                    };
                    (x, y)
                };
                let ((x, y), (cx, cy)) = (at(ours_at), at(chrome_at));
                corners = corners.max(difference(rgb(ours.at(x, y))?, rgb(chrome.at(cx, cy))?));
            }
        }
    }
    Some((sides, corners))
}

#[test]
fn every_box_is_chromes() {
    let Some((rendering, _)) = case("a-text-area") else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    let mut boxes = boxes_of(&rendering, "textarea");
    boxes.extend(boxes_of(&rendering, "input"));
    boxes.sort_by(|a, b| a.origin.y.total_cmp(&b.origin.y));
    // Measured: each is 96 by 40 at x 8, every 48 pixels down from 8.
    assert_eq!(boxes.len(), 5, "{boxes:?}");
    for (area, top) in boxes.iter().zip([8.0, 56.0, 104.0, 152.0, 200.0]) {
        let expected = Rect::new(8.0, top, 96.0, 40.0);
        let near = |a: f32, b: f32| (a - b).abs() < 1.0 / 64.0;
        assert!(
            near(area.origin.x, expected.origin.x)
                && near(area.origin.y, expected.origin.y)
                && near(area.size.width, expected.size.width)
                && near(area.size.height, expected.size.height),
            "{area:?} is Chrome's {expected:?}"
        );
    }
}

#[test]
fn a_text_area_is_drawn_as_a_field_of_its_size_is() {
    // Chrome's two are the same pixels, and so are this engine's.
    let Some((rendering, chrome)) = case("a-text-area") else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case draws");
    };
    let (Some(area), Some(field)) = (
        boxes_of(&rendering, "textarea").first().copied(),
        boxes_of(&rendering, "input").first().copied(),
    ) else {
        panic!("the case has a text area and a field");
    };
    for (picture, name) in [(&drawing.canvas, "here"), (&chrome, "in Chrome")] {
        let Some((sides, corners)) = compared(picture, area, picture, field, 0) else {
            panic!("both controls are on the picture {name}");
        };
        assert_eq!((sides, corners), (0, 0), "the two differ {name}");
    }
}

#[test]
fn each_text_area_is_chromes_picture() {
    // Measured: the fill is Chrome's exactly, and the edge is Chrome's
    // within a field's own bound, the same numbers the field between them
    // gives: none on a live side and 9 on a corner's inner pixel (191 here,
    // Chrome's 199), 5 and 5 disabled, and none at all where the page's
    // background has the sheet's square border drawn. The grip makes the
    // last 38 off on its sides, until its corner is left out.
    let Some((rendering, chrome)) = case("a-text-area") else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case draws");
    };
    let areas = boxes_of(&rendering, "textarea");
    assert_eq!(areas.len(), 4, "{areas:?}");
    for (area, (name, worst_side, worst_corner, clear)) in areas.iter().zip([
        ("live", 0, 9, 0),
        ("disabled", 5, 5, 0),
        ("with a background", 0, 0, 0),
        ("that can be resized", 0, 9, GRIP),
    ]) {
        let (left, top, _, _) = pixels(*area);
        assert_eq!(
            rgb(drawing.canvas.at(left + 4, top + 4)),
            rgb(chrome.at(left + 4, top + 4)),
            "the {name} text area's fill"
        );
        let Some((sides, corners)) = compared(&drawing.canvas, *area, &chrome, *area, clear) else {
            panic!("the {name} text area is on both pictures");
        };
        assert!(
            sides <= worst_side,
            "the {name} text area's sides are {sides} off"
        );
        assert!(
            corners <= worst_corner,
            "the {name} text area's corners are {corners} off"
        );
    }
}

#[test]
fn a_pizza_forms_text_area_is_drawn_as_chromes() {
    // Its box is not Chrome's: 166.55 by 46 at y 537.19 here, `20ch` and
    // `3em`, and 183 by 36 at 540.19 in Chrome, which sizes it by `cols`
    // and `rows` (queue item 523). So each picture is compared at its own
    // box. Measured: the sides are Chrome's exactly and the corners within
    // a field's 9, where the square solid pixel drawn before was 43 off on
    // a side and 107 in a corner.
    let Some((rendering, chrome)) = case("web-a-form") else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case draws");
    };
    let Some(area) = boxes_of(&rendering, "textarea").first().copied() else {
        panic!("the form has a text area");
    };
    let chromes = Rect::new(11755.0 / 64.0, 34572.0 / 64.0, 183.0, 36.0);
    let Some((sides, corners)) = compared(&drawing.canvas, area, &chrome, chromes, GRIP) else {
        panic!("the text area is on both pictures");
    };
    assert_eq!(sides, 0, "its sides are {sides} off");
    assert!(corners <= 9, "its corners are {corners} off");
}
