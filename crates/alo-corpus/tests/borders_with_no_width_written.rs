/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Borders given a style and no width: the case that closes queue item
//! 602.
//!
//! `cases/borders-with-no-width-written` places twelve boxes, each 24 by
//! 20 inside its border, whose border is given a style and — on some side
//! — no width, written every way a sheet writes one: `border-style`
//! alone, `border` naming a style and a colour, a side shorthand naming no
//! width after `border`, one side's style longhand alone, one side's
//! shorthand alone, `thin`, `medium` and `thick` in the width longhands,
//! an `inset` border, rounded corners, a two-value style list, and two
//! boxes at tenths of a pixel.
//!
//! CSS's initial border width is `medium`, and Chrome 155 computes it as
//! 3px, a side shorthand that names no width resetting its side to it
//! (`getComputedStyle` and `getBoundingClientRect()`, measured). This
//! engine gave such a side no width at all, and refused a named width in a
//! longhand, so all but `side-resets`' three written sides were drawn as
//! bare yellow boxes.
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

/// Each box's border box as Chrome lays it out at one and at two, as left,
/// top, width and height in 128ths of a pixel; and its border's widths as
/// Chrome computes them, top, right, bottom and left. Only `two-styles`
/// differs between the scales, by a 128th, on the finer grid of two.
type Measured = (&'static str, [f32; 4], [f32; 4], [f32; 4]);

const MEASURED: [Measured; 12] = [
    (
        "style-alone",
        [1920.0, 1920.0, 3840.0, 3328.0],
        [1920.0, 1920.0, 3840.0, 3328.0],
        [3.0; 4],
    ),
    (
        "border-no-width",
        [9600.0, 1920.0, 3840.0, 3328.0],
        [9600.0, 1920.0, 3840.0, 3328.0],
        [3.0; 4],
    ),
    (
        "side-resets",
        [17280.0, 1920.0, 4224.0, 4096.0],
        [17280.0, 1920.0, 4224.0, 4096.0],
        [6.0, 6.0, 6.0, 3.0],
    ),
    (
        "one-longhand",
        [24960.0, 1920.0, 3072.0, 2944.0],
        [24960.0, 1920.0, 3072.0, 2944.0],
        [3.0, 0.0, 0.0, 0.0],
    ),
    (
        "side-alone",
        [32640.0, 1920.0, 3072.0, 2944.0],
        [32640.0, 1920.0, 3072.0, 2944.0],
        [0.0, 0.0, 3.0, 0.0],
    ),
    (
        "named-widths",
        [40320.0, 1920.0, 3840.0, 3328.0],
        [40320.0, 1920.0, 3840.0, 3328.0],
        [1.0, 3.0, 5.0, 3.0],
    ),
    (
        "named-side",
        [48000.0, 1920.0, 4096.0, 3328.0],
        [48000.0, 1920.0, 4096.0, 3328.0],
        [3.0, 3.0, 3.0, 5.0],
    ),
    (
        "inset-medium",
        [55680.0, 1920.0, 3840.0, 3328.0],
        [55680.0, 1920.0, 3840.0, 3328.0],
        [3.0; 4],
    ),
    (
        "rounded",
        [1920.0, 9600.0, 3840.0, 3328.0],
        [1920.0, 9600.0, 3840.0, 3328.0],
        [3.0; 4],
    ),
    (
        "tenths",
        [9638.0, 9676.0, 3840.0, 3328.0],
        [9638.0, 9676.0, 3840.0, 3328.0],
        [3.0; 4],
    ),
    (
        "two-styles",
        [17368.0, 9624.0, 3072.0, 3328.0],
        [17369.0, 9625.0, 3072.0, 3328.0],
        [3.0, 0.0, 3.0, 0.0],
    ),
    (
        "medium-written",
        [24960.0, 9600.0, 3840.0, 3328.0],
        [24960.0, 9600.0, 3840.0, 3328.0],
        [3.0; 4],
    ),
];

/// The case's plain colours: the page's white; the borders' blue and red;
/// the boxes' yellow; and an `inset` grey's two tones, as Chrome's
/// screenshot has them.
const PLAIN: [(u8, u8, u8); 6] = [
    (255, 255, 255),
    (0x1d, 0x4e, 0xd8),
    (0xc0, 0x39, 0x2b),
    (0xfd, 0xe6, 0x8a),
    (44, 44, 44),
    (212, 212, 212),
];

/// How far a blended pixel may be from Chrome's, of 255 in any channel, at
/// each scale, where either picture has a blend: a mitre between two
/// colours, a rounded corner's edge, or a box at tenths of a pixel.
/// Measured on this case: the worst at both scales, 82, is on the mitre
/// between `side-resets`' red 3px left and its blue 6px bottom, whose
/// every step Chrome draws as an even mix of the two where this engine
/// alternates (queue item 518); away from it the worst is 47, on
/// `inset-medium`'s two-toned mitre, and every rounded corner is nearer.
const EDGE: [(f32, u8); 2] = [(1.0, 82), (2.0, 82)];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("borders-with-no-width-written"))
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
/// Chrome computes it on each side — `medium`, 3px, on a side given a
/// style and no width — at one and at two.
#[test]
fn a_style_with_no_width_written_is_medium() {
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
        for (class, at_one, at_two, widths) in MEASURED {
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
            let chromes = if scale > 1.0 { at_two } else { at_one };
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
/// and at two: each side given a style and no width is a 3px band of its
/// colour round the yellow. Where either is a blend the two are within
/// `EDGE`.
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
