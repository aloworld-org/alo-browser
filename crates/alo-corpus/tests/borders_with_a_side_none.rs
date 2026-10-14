/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Borders with a side whose style is `none` or `hidden`: the case that
//! closes queue item 595.
//!
//! `cases/borders-with-a-side-none` places ten boxes, each 24 by 20 inside
//! its border, whose border has a side styled `none` or `hidden` — or no
//! style at all — written every way a sheet writes one: a longhand beside
//! `border-width` and `border-style`, `hidden` beside `border`, a width
//! with no style, a side shorthand naming no style after `border`,
//! `border-style: none` after `border`, a longhand width beside a longhand
//! `none`, an `inset` border, a two-value style list, and two boxes at
//! tenths of a pixel.
//!
//! Chrome 154 computes such a side's width as 0px however its width was
//! written, and lays the box out and draws it so (`getComputedStyle` and
//! `getBoundingClientRect()`, measured). This engine zeroed a width only
//! when its style came in the same shorthand, so `border-width: 4px 10px;
//! border-left-style: none` was laid out with a 10 px left border nothing
//! drew, its top and bottom mitred against it.
//!
//! `chrome.png` and `chrome-at-2.png` beside the case are Chrome 154's
//! screenshots of the same bytes, 500 by 140 CSS pixels, headless, started
//! with `--force-device-scale-factor=1` and `=2`, without `--disable-gpu`,
//! taken 2026-10-14.
//!
//! `render-at-2.png` is this engine's picture at two, beside `render.png`
//! and never in its place (ADR 0057 § 5).

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_paint::Canvas;

/// Each box's border box as Chrome lays it out at one and at two, as left,
/// top, width and height in 128ths of a pixel; and its border's widths as
/// Chrome computes them, top, right, bottom and left. Only `two-none`
/// differs between the scales, by a 128th, on the finer grid of two.
type Measured = (&'static str, [f32; 4], [f32; 4], [f32; 4]);

const MEASURED: [Measured; 10] = [
    (
        "left-none",
        [1920.0, 1920.0, 4352.0, 3584.0],
        [1920.0, 1920.0, 4352.0, 3584.0],
        [4.0, 10.0, 4.0, 0.0],
    ),
    (
        "top-hidden",
        [9600.0, 1920.0, 4608.0, 3328.0],
        [9600.0, 1920.0, 4608.0, 3328.0],
        [0.0, 6.0, 6.0, 6.0],
    ),
    (
        "no-style",
        [17280.0, 1920.0, 3072.0, 2560.0],
        [17280.0, 1920.0, 3072.0, 2560.0],
        [0.0; 4],
    ),
    (
        "side-without-style",
        [24960.0, 1920.0, 3584.0, 3584.0],
        [24960.0, 1920.0, 3584.0, 3584.0],
        [4.0, 4.0, 4.0, 0.0],
    ),
    (
        "right-none-two-colours",
        [32640.0, 1920.0, 3840.0, 4096.0],
        [32640.0, 1920.0, 3840.0, 4096.0],
        [6.0, 0.0, 6.0, 6.0],
    ),
    (
        "all-none",
        [40320.0, 1920.0, 3072.0, 2560.0],
        [40320.0, 1920.0, 3072.0, 2560.0],
        [0.0; 4],
    ),
    (
        "longhand-width",
        [48000.0, 1920.0, 3456.0, 3328.0],
        [48000.0, 1920.0, 3456.0, 3328.0],
        [3.0, 3.0, 3.0, 0.0],
    ),
    (
        "inset-left-none",
        [55680.0, 1920.0, 3584.0, 3584.0],
        [55680.0, 1920.0, 3584.0, 3584.0],
        [4.0, 4.0, 4.0, 0.0],
    ),
    (
        "tenths",
        [1958.0, 9676.0, 4864.0, 2944.0],
        [1958.0, 9676.0, 4864.0, 2944.0],
        [3.0, 7.0, 0.0, 7.0],
    ),
    (
        "two-none",
        [9688.0, 9624.0, 3072.0, 4096.0],
        [9689.0, 9625.0, 3072.0, 4096.0],
        [5.0, 0.0, 7.0, 0.0],
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
/// each scale, where either picture has a blend: a rounded corner's edge,
/// or a box at tenths of a pixel. Measured on this case: the worst at one,
/// (284, 37), is on `right-none-two-colours`' rounded corner, Chrome's own
/// coarse smoothing of a curve at one (`borders-side-by-side` bounds its
/// curves alike), and at two, (36, 156), on `tenths`' corner.
const EDGE: [(f32, u8); 2] = [(1.0, 92), (2.0, 80)];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("borders-with-a-side-none"))
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
/// Chrome computes it on each side — none at all on a side styled `none`
/// or `hidden`, or given no style — at one and at two.
#[test]
fn a_side_styled_none_has_no_width_however_it_was_written() {
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
/// and at two: the yellow of a box reaches its edge where a side has no
/// width, and a rounded corner beside such a side is the crescent Chrome
/// draws. Where either is a blend the two are within `EDGE`.
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
