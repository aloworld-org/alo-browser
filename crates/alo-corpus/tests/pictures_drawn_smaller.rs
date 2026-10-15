/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Pictures drawn smaller than they are, through Chrome's mipmap levels: a
//! case of queue item 179.
//!
//! `cases/pictures-drawn-smaller` draws seven files of random colours,
//! absolutely placed on whole pixels, so that every texel a pixel takes
//! from the wrong level, at the wrong share or rounded the wrong way shows:
//! a copy uploaded a level down and mixed with the next (`a-first`, 10 × 10
//! of 32) and two levels down at its own size (`c-first`); the picture's
//! own levels mixed at 53, 136, 158 and 241 256ths and a whole level and 241
//! down (`d`); a draw smaller on one axis only that is mixed all the same
//! because an earlier draw of the same file was (`b-later`), and one that is
//! not, because none was (`f`); a copy a level down of a 48 × 24 picture
//! (`e`); and one too little smaller to mix (`g`). At two most of them are
//! drawn larger than they are, and bilinear.
//!
//! This engine drew every picture smaller than it is one texel a pixel.
//!
//! `chrome.png` and `chrome-at-2.png` beside the case are Chrome 155's
//! (`155.0.8059.41`) screenshots of the same bytes, 500 by 60 CSS pixels,
//! headless, started with `--force-device-scale-factor=1` and `=2`, without
//! `--disable-gpu`, taken 2026-10-15, the style sheet put inside the page
//! and the pictures named beside it.
//!
//! `render-at-2.png` is this engine's picture at two, beside `render.png`
//! and never in its place (ADR 0057 § 5).

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_paint::Canvas;

/// Each picture's class and its border box in CSS pixels, as the sheet
/// places it: left, top, width and height.
const PLACED: [(&str, [f32; 4]); 10] = [
    ("a-first", [2.0, 2.0, 10.0, 10.0]),
    ("a-later", [14.0, 2.0, 20.0, 8.0]),
    ("b-first", [36.0, 2.0, 20.0, 20.0]),
    ("b-later", [58.0, 2.0, 40.0, 10.0]),
    ("c-first", [100.0, 2.0, 16.0, 4.0]),
    ("c-later", [118.0, 2.0, 30.0, 10.0]),
    ("d", [2.0, 26.0, 6.0, 22.0]),
    ("e", [10.0, 26.0, 9.0, 7.0]),
    ("f", [21.0, 26.0, 32.0, 10.0]),
    ("g", [55.0, 26.0, 24.0, 24.0]),
];

/// The pixels whose mix lies within a thirtieth of a sixteenth of a level
/// from half way, where Chrome's weights, worked out in floats, round it the
/// other way: scale, across and down. Each is a level from Chrome's in one
/// channel. `b-first`'s blue, 1,335.49 sixteenths here; `b-later`'s,
/// 2,039.51; `a-first`'s red at two, 2,520.50; `e`'s green, 2,552.53.
const ROUNDED_APART: [(u8, u32, u32); 4] = [(1, 49, 12), (1, 91, 7), (2, 7, 21), (2, 22, 55)];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("pictures-drawn-smaller"))
}

/// The box of the element with `class`.
fn box_of(rendering: &Rendering, class: &str) -> Option<BoxId> {
    let (document, drawing) = (rendering.document()?, rendering.drawing()?);
    let node = document.descendants(document.root()).find(|id| {
        document.element(*id).is_some_and(|element| {
            element
                .attr("class")
                .is_some_and(|classes| classes.split_whitespace().any(|name| name == class))
        })
    })?;
    let boxes = &drawing.boxes;
    boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|box_node| &box_node.kind),
            Some(BoxKind::Element { node: of, .. }) if *of == node
        )
    })
}

/// Chrome's screenshot at `scale`.
fn chrome(case: &Case, scale: u8) -> Option<Canvas> {
    let name = if scale == 1 {
        "chrome.png"
    } else {
        "chrome-at-2.png"
    };
    let bytes = std::fs::read(case.expectation(name)).ok()?;
    alo_paint::from_png(&bytes).ok()
}

/// Every picture's border box is where the sheet puts it, in CSS pixels, at
/// one and at two.
#[test]
fn every_picture_is_where_the_sheet_puts_it() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for scale in [1.0, 2.0] {
        let rendering = match Rendering::at_scale(&case, scale) {
            Ok(rendering) => rendering,
            Err(why) => panic!("at {scale}: {why}"),
        };
        let Some(drawing) = rendering.drawing() else {
            panic!("at {scale}: no drawing");
        };
        for (class, [left, top, width, height]) in PLACED {
            let Some(id) = box_of(&rendering, class) else {
                panic!("{class} has a box at {scale}");
            };
            let Some(laid) = drawing.layout.get(id) else {
                panic!("{class} is laid out at {scale}");
            };
            let border = laid.border_box;
            let found = [
                border.origin.x,
                border.origin.y,
                border.size.width,
                border.size.height,
            ];
            assert!(
                found
                    .iter()
                    .zip([left, top, width, height])
                    .all(|(ours, sheet)| (ours - sheet).abs() < 1.0 / 128.0),
                "{class} at {scale}: {found:?}"
            );
        }
    }
}

/// Every device pixel of every picture is Chrome's exactly, at one and at
/// two, and so is the page around them — but for the four
/// [`ROUNDED_APART`], each a level from Chrome's in one channel.
#[test]
fn every_pixel_is_chromes() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for scale in [1_u8, 2] {
        let Some(drawing) = Rendering::at_scale(&case, f32::from(scale))
            .ok()
            .and_then(|rendering| rendering.drawing())
        else {
            panic!("the case is drawn at {scale}");
        };
        let Some(chrome) = chrome(&case, scale) else {
            panic!("Chrome's picture at {scale} is beside the case");
        };
        let ours = &drawing.canvas;
        assert_eq!(
            (ours.width(), ours.height()),
            (chrome.width(), chrome.height()),
            "at {scale}"
        );
        let mut wrong = Vec::new();
        for y in 0..ours.height() {
            for x in 0..ours.width() {
                let (mine, theirs) = (
                    ours.at(x, y).map(alo_value::Rgba::to_rgba8),
                    chrome.at(x, y).map(alo_value::Rgba::to_rgba8),
                );
                if mine == theirs {
                    continue;
                }
                let apart = ROUNDED_APART.contains(&(scale, x, y))
                    && mine.zip(theirs).is_some_and(|(a, b)| {
                        let channels = [(a.0, b.0), (a.1, b.1), (a.2, b.2), (a.3, b.3)];
                        channels.iter().filter(|(a, b)| a != b).count() == 1
                            && channels.iter().all(|(a, b)| a.abs_diff(*b) <= 1)
                    });
                if !apart {
                    wrong.push(format!(
                        "({x}, {y}): Chrome {theirs:?}, this engine {mine:?}"
                    ));
                }
            }
        }
        assert!(
            wrong.is_empty(),
            "at {scale}, {} pixels:\n{}",
            wrong.len(),
            wrong
                .iter()
                .take(40)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

#[test]
fn the_picture_at_two_is_its_reference() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let Some(drawing) = Rendering::at_scale(&case, 2.0)
        .ok()
        .and_then(|rendering| rendering.drawing())
    else {
        panic!("the case is drawn at two");
    };
    let difference = check_picture(&case, "render-at-2.png", &drawing.canvas);
    assert!(difference.is_none(), "{difference:?}");
}
