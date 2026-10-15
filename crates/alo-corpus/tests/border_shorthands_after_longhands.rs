/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A border shorthand written after one of its longhands: the case that
//! closes queue item 603.
//!
//! `cases/border-shorthands-after-longhands` places twelve boxes, each 24
//! by 20 inside its border, where a `border` or `border-<side>` shorthand
//! and a longhand it sets compete: the shorthand written after the
//! longhand in one block, in a later rule, from the `style` attribute; the
//! longhand written after the shorthand; and the longhand winning by
//! specificity or by `!important` though the shorthand came later.
//!
//! Both are held whole beside each other, and every reader takes the
//! longhand first, so the cascade must take out a longhand its shorthand
//! beat. `border` did; a side shorthand did not, so `border-left-style:
//! none; border-left: 5px solid` drew no left border here where Chrome 155
//! computes `5px solid` (`getComputedStyle` and
//! `getBoundingClientRect()`, measured, the same at one and at two).
//!
//! `chrome.png` and `chrome-at-2.png` beside the case are Chrome 155's
//! screenshots of the same bytes, 500 by 140 CSS pixels, headless, started
//! with `--force-device-scale-factor=1` and `=2`, without `--disable-gpu`,
//! taken 2026-10-15.
//!
//! `render-at-2.png` is this engine's picture at two, beside `render.png`
//! and never in its place (ADR 0057 § 5).

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_paint::Canvas;

/// Each box's border box as Chrome lays it out, at one and at two alike,
/// as left, top, width and height in 128ths of a pixel; and its border's
/// widths as Chrome computes them, top, right, bottom and left.
type Measured = (&'static str, [f32; 4], [f32; 4]);

const MEASURED: [Measured; 12] = [
    (
        "side-after-style",
        [1920.0, 1920.0, 3712.0, 2560.0],
        [0.0, 0.0, 0.0, 5.0],
    ),
    (
        "all-after-style",
        [9600.0, 1920.0, 4352.0, 3840.0],
        [5.0; 4],
    ),
    (
        "all-after-width",
        [17280.0, 1920.0, 4096.0, 3584.0],
        [4.0; 4],
    ),
    (
        "side-after-color",
        [24960.0, 1920.0, 4096.0, 3840.0],
        [4.0, 4.0, 6.0, 4.0],
    ),
    (
        "longhand-after-side",
        [32640.0, 1920.0, 3328.0, 2560.0],
        [0.0, 0.0, 0.0, 2.0],
    ),
    (
        "across-rules",
        [40320.0, 1920.0, 3072.0, 3328.0],
        [6.0, 0.0, 0.0, 0.0],
    ),
    (
        "specific-longhand",
        [48000.0, 1920.0, 3200.0, 2560.0],
        [0.0, 1.0, 0.0, 0.0],
    ),
    (
        "important-longhand",
        [55680.0, 1920.0, 3840.0, 2560.0],
        [0.0, 0.0, 0.0, 6.0],
    ),
    (
        "attached",
        [1920.0, 9600.0, 3072.0, 3200.0],
        [5.0, 0.0, 0.0, 0.0],
    ),
    ("all-after-side", [9600.0, 9600.0, 3840.0, 3328.0], [3.0; 4]),
    (
        "side-over-all",
        [17280.0, 9600.0, 3584.0, 3328.0],
        [4.0, 2.0, 2.0, 2.0],
    ),
    (
        "tenths",
        [24998.0, 9676.0, 3712.0, 2560.0],
        [0.0, 5.0, 0.0, 0.0],
    ),
];

/// The case's plain colours: the page's white, the borders' blue and red,
/// and the boxes' yellow.
const PLAIN: [(u8, u8, u8); 4] = [
    (255, 255, 255),
    (0x1d, 0x4e, 0xd8),
    (0xc0, 0x39, 0x2b),
    (0xfd, 0xe6, 0x8a),
];

/// How far a blended pixel may be from Chrome's, of 255 in any channel, at
/// each scale, where either picture has a blend: a mitre between two
/// colours, or a box at tenths of a pixel. Measured on this case: the
/// worst at both scales, 76, is on `side-after-color`'s mitre between its
/// red 6px bottom and blue 4px sides, and the next, 60, on
/// `side-over-all`'s between its red 4px top and blue 2px sides, every
/// step of which Chrome draws as an even mix where this engine alternates
/// (queue item 518). Every other blend is Chrome's exactly.
const EDGE: [(f32, u8); 2] = [(1.0, 76), (2.0, 76)];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("border-shorthands-after-longhands"))
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

fn within_a_128th(ours: f32, chromes: f32) -> bool {
    (ours - chromes / 128.0).abs() < 1.0 / 128.0
}

/// Every box is laid out where Chrome lays it out, its border as wide as
/// Chrome computes it on each side — whichever of a shorthand and its
/// longhand won the cascade deciding it — at one and at two.
#[test]
fn a_shorthand_and_its_longhand_compete_as_one_property() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for scale in [1.0, 2.0] {
        let rendering = Rendering::at_scale(&case, scale).ok();
        let Some(drawing) = rendering.as_ref().and_then(Rendering::drawing) else {
            panic!("the case renders at {scale}");
        };
        let Some(rendering) = rendering.as_ref() else {
            panic!("the case renders at {scale}");
        };
        for (class, chromes, widths) in MEASURED {
            let Some(laid_out) = box_of(rendering, class).and_then(|id| drawing.layout.get(id))
            else {
                panic!("{class} is laid out at {scale}");
            };
            let border = laid_out.border;
            let found = [border.top, border.right, border.bottom, border.left];
            assert!(
                found
                    .iter()
                    .zip(widths)
                    .all(|(a, b)| (a - b).abs() < 1.0 / 128.0),
                "{class} at {scale}: a border of {found:?} where Chrome's is {widths:?}",
            );
            let rect = laid_out.border_box;
            let ours = [
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                rect.size.height,
            ];
            assert!(
                ours.iter().zip(chromes).all(|(a, b)| within_a_128th(*a, b)),
                "{class} at {scale}: {ours:?} where Chrome's is {chromes:?} 128ths",
            );
        }
    }
}

/// The picture Chrome took at `scale`, beside the case.
fn chrome(case: &Case, scale: f32) -> Option<Canvas> {
    let name = if scale > 1.0 {
        "chrome-at-2.png"
    } else {
        "chrome.png"
    };
    let bytes = std::fs::read(case.expectation(name)).ok()?;
    alo_paint::from_png(&bytes).ok()
}

/// Which plain colour a pixel is, if it is one.
fn plain(canvas: &Canvas, x: u32, y: u32) -> Option<usize> {
    let (red, green, blue, _) = canvas.at(x, y)?.to_rgba8();
    PLAIN.iter().position(|&(r, g, b)| {
        red.abs_diff(r) <= 1 && green.abs_diff(g) <= 1 && blue.abs_diff(b) <= 1
    })
}

/// The largest difference in any channel between two pictures' pixels.
fn difference(ours: &Canvas, chrome: &Canvas, x: u32, y: u32) -> u8 {
    let (Some(a), Some(b)) = (ours.at(x, y), chrome.at(x, y)) else {
        return u8::MAX;
    };
    let ((ar, ag, ab, _), (br, bg, bb, _)) = (a.to_rgba8(), b.to_rgba8());
    [ar.abs_diff(br), ag.abs_diff(bg), ab.abs_diff(bb)]
        .into_iter()
        .max()
        .unwrap_or(u8::MAX)
}

/// Wherever both pictures have a plain colour it is the same one, at one
/// and at two: each side is drawn in the colour and style of whichever
/// declaration won it. Where either is a blend the two are within `EDGE`.
#[test]
fn every_box_is_drawn_as_chromes_is() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, edge) in EDGE {
        let rendering = Rendering::at_scale(&case, scale).ok();
        let (Some(drawing), Some(chrome)) = (
            rendering.as_ref().and_then(Rendering::drawing),
            chrome(&case, scale),
        ) else {
            panic!("the case is drawn at {scale}, and Chrome's picture is beside it");
        };
        let ours = &drawing.canvas;
        assert_eq!(
            (ours.width(), ours.height()),
            (chrome.width(), chrome.height()),
            "at {scale}"
        );
        let mut worst = (0, None);
        let mut blends = 0;
        for y in 0..ours.height() {
            for x in 0..ours.width() {
                if let (Some(a), Some(b)) = (plain(ours, x, y), plain(&chrome, x, y)) {
                    assert_eq!(a, b, "at {scale}, ({x}, {y}) is a different colour");
                    continue;
                }
                blends += 1;
                let found = difference(ours, &chrome, x, y);
                if found > worst.0 {
                    worst = (found, Some((x, y)));
                }
            }
        }
        assert!(blends > 0, "at {scale} no curve was compared");
        assert!(
            worst.0 <= edge,
            "at {scale} a blend is {} from Chrome's at {:?}",
            worst.0,
            worst.1,
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
