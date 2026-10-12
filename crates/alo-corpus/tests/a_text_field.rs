/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A text field is as wide as its `size` says: the cases that close queue
//! item 511.
//!
//! Chrome 154 makes a field of `n` characters `ceil(c × n + a − c)` wide
//! inside its padding and border, where `c` is the advance of the face's
//! `x`, rounded to a whole pixel when that makes it wider, and `a` is the
//! face's ascent in whole pixels (`alo_layout`'s `field_width`). It was
//! fitted to 3,000 fields in three faces and gives 2,997 of them. The
//! other three are 38 to 40 characters of `DejaVu Sans Mono` at 13.33 px,
//! a pixel narrower in Chrome, which sets a face at its size cut to
//! hundredths (queue item 514). It gives all 288 more in four faces and
//! nine sizes it was not fitted to.
//!
//! `a-text-field` is nine fields: twenty characters, one, five, a search
//! field and a number field of five, a password of seven, and ten at 16
//! px in `DejaVu Sans` and in `DejaVu Sans Mono`, and thirty at 21 px.
//! `web-a-form` is the HTML specification's pizza form, whose fields
//! were 20 of the font's zeros wide. Each case's `chrome.png` is Chrome's
//! screenshot of the same bytes, with the corpus's fonts standing for the
//! families Chrome computes, at one device pixel to one and without
//! `--disable-gpu`.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;
use alo_paint::Canvas;

/// The named case, its rendering and Chrome's screenshot of it.
fn case(name: &str) -> Option<(Rendering, Canvas)> {
    let directory = cases_directory().join(name);
    let case = Case::read(&directory)?;
    let rendering = Rendering::of(&case).ok()?;
    let bytes = std::fs::read(directory.join("chrome.png")).ok()?;
    let chrome = alo_paint::from_png(&bytes).ok()?;
    Some((rendering, chrome))
}

/// The border box of each `<input>`, in document order.
fn fields(rendering: &Rendering) -> Vec<Rect> {
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        return Vec::new();
    };
    let boxes = &drawing.boxes;
    document
        .descendants(document.root())
        .filter(|id| {
            document
                .element(*id)
                .is_some_and(|element| element.name.is_html("input"))
        })
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

/// The largest difference in any channel between two pictures' pixels at
/// `(x, y)`, or [`None`] when either has no pixel there.
fn difference(ours: &Canvas, chrome: &Canvas, x: u32, y: u32) -> Option<u8> {
    let (red, green, blue, _) = ours.at(x, y)?.to_rgba8();
    let (chrome_red, chrome_green, chrome_blue, _) = chrome.at(x, y)?.to_rgba8();
    Some(
        red.abs_diff(chrome_red)
            .max(green.abs_diff(chrome_green))
            .max(blue.abs_diff(chrome_blue)),
    )
}

/// A box's edges on whole pixels, each rounded on its own, a half up, as
/// a control's picture is drawn.
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

/// Whether a pixel is in one of the two-by-two squares at a box's corners,
/// where a rounded edge's inner pixel is 9 off Chrome's (queue item 524).
fn in_a_corner((left, top, right, bottom): (u32, u32, u32, u32), x: u32, y: u32) -> bool {
    let across = x < left + 2 || x + 2 >= right;
    let down = y < top + 2 || y + 2 >= bottom;
    across && down && x >= left && x < right && y >= top && y < bottom
}

#[test]
fn every_field_is_chromes_box() {
    let Some((rendering, _)) = case("a-text-field") else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    let fields = fields(&rendering);
    // Measured in Chrome 154, in its layout unit of 1/64 px: x, y, width
    // and height. A number field is twenty characters whatever its `size`
    // says, and a search field is as wide as a text field.
    let chrome: [(f32, f32, f32, f32); 9] = [
        (512.0, 512.0, 11008.0, 1344.0),
        (512.0, 2368.0, 1280.0, 1344.0),
        (512.0, 4224.0, 3328.0, 1344.0),
        (512.0, 6080.0, 3328.0, 1344.0),
        (512.0, 7936.0, 11008.0, 1344.0),
        (512.0, 9792.0, 4352.0, 1344.0),
        (512.0, 11648.0, 6976.0, 1600.0),
        (512.0, 13760.0, 7232.0, 1600.0),
        (512.0, 15872.0, 24832.0, 1920.0),
    ];
    assert_eq!(fields.len(), chrome.len(), "{fields:?}");
    let near = |ours: f32, units: f32| (ours - units / 64.0).abs() < 1.0 / 64.0;
    for (field, (x, y, width, height)) in fields.iter().zip(chrome) {
        assert!(
            near(field.origin.x, x)
                && near(field.origin.y, y)
                && near(field.size.width, width)
                && near(field.size.height, height),
            "{field:?} is Chrome's {x}, {y}, {width} by {height} units"
        );
    }
}

#[test]
fn every_field_is_drawn_as_chromes() {
    // Measured: every pixel of the page is within 9 levels of Chrome's
    // screenshot, and all but the corners' are within 2.
    let Some((rendering, chrome)) = case("a-text-field") else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case draws");
    };
    let corners: Vec<_> = fields(&rendering).into_iter().map(pixels).collect();
    assert_eq!(corners.len(), 9);
    let (mut anywhere, mut off_the_corners) = (0, 0);
    for y in 0..286 {
        for x in 0..400 {
            let Some(off) = difference(&drawing.canvas, &chrome, x, y) else {
                panic!("both pictures have a pixel at {x}, {y}");
            };
            anywhere = anywhere.max(off);
            if !corners.iter().any(|field| in_a_corner(*field, x, y)) {
                off_the_corners = off_the_corners.max(off);
            }
        }
    }
    assert!(anywhere <= 9, "a pixel is {anywhere} off");
    assert!(
        off_the_corners <= 2,
        "a pixel off the corners is {off_the_corners} off"
    );
}

#[test]
fn a_pizza_forms_fields_are_drawn_as_chromes() {
    // They were 177.66 wide, 20 zeros, where Chrome's are 172, so each
    // was 56 pixels from Chrome's screenshot, 137 levels at worst. Now
    // they are Chrome's width and stand where Chrome's do, and only their
    // corners' inner pixels are more than 2 off.
    let Some((rendering, chrome)) = case("web-a-form") else {
        panic!("the case renders and Chrome's screenshot reads");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case draws");
    };
    let text_fields: Vec<_> = fields(&rendering).into_iter().take(3).collect();
    assert_eq!(text_fields.len(), 3);
    for field in text_fields {
        let at = pixels(field);
        let (left, top, right, bottom) = at;
        let (mut anywhere, mut off_the_corners) = (0, 0);
        // From a pixel outside the box to a pixel outside it.
        for y in top - 1..=bottom {
            for x in left - 1..=right {
                let Some(off) = difference(&drawing.canvas, &chrome, x, y) else {
                    panic!("both pictures have a pixel at {x}, {y}");
                };
                anywhere = anywhere.max(off);
                if !in_a_corner(at, x, y) {
                    off_the_corners = off_the_corners.max(off);
                }
            }
        }
        assert!(anywhere <= 9, "{field:?}: a pixel is {anywhere} off");
        assert!(
            off_the_corners <= 2,
            "{field:?}: a pixel off the corners is {off_the_corners} off"
        );
    }
}
