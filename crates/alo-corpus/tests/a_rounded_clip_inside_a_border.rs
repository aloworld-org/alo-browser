/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Bordered, rounded boxes that clip, clipping to their padding edge's
//! curve: the case that closes queue item 588.
//!
//! `cases/rounded-clips-inside-borders` places seventeen boxes 44 by 40,
//! each with a border, rounded corners and `overflow: hidden`, each holding
//! a red child that reaches 20 px past it on every side, so what is drawn
//! of the child is the clip. Eight have blue borders — seven as wide on
//! every side, whose radii fit as written, and `sides`, whose sides are
//! not, which was transparent until this engine filled the inside of such
//! a border's rounded corners (queue item 592); nine have transparent
//! ones, with sides of unequal widths, radii CSS scales down to fit,
//! percentages, ellipses and four different corners; two are laid out at
//! tenths of a pixel.
//!
//! Chrome 154 clips to the padding box rounded by each radius resolved
//! against the border box, fitted to it, and then less the border beside
//! it, never below square — CSS Backgrounds 3's rule, and 60 of 60 seeded
//! boxes at one, two and eight, measured, not read from its source. This
//! engine rounded the padding box by the border box's radii, resolved
//! against the padding box, which left a sliver of white between a blue
//! border's inner curve and the red it clips.
//!
//! `chrome.png` and `chrome-at-2.png` beside the case are Chrome 154's
//! screenshots of the same bytes, 500 by 200 CSS pixels, headless, started
//! with `--force-device-scale-factor=1` and `=2`, without `--disable-gpu`,
//! taken 2026-10-14, and again that day when `sides` was made blue: the
//! second pair differs from the first inside `sides` and in 2 curve pixels
//! elsewhere at one and 8 at two, Chrome's own run-to-run smoothing.
//!
//! `render-at-2.png` is this engine's picture at two, beside `render.png`
//! and never in its place (ADR 0057 § 5).

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_paint::Canvas;
use alo_paint::display::DisplayItem;
use alo_paint::path::{Path, Segment};

/// Each box's clip: its padding box at one and at two, as left, top, width
/// and height, and its corners' radii, across and down, from the top left
/// clockwise — the rule Chrome was fitted to, worked out by hand.
type Clip = (&'static str, [f32; 4], [f32; 4], [(f32, f32); 4]);

const CLIPS: [Clip; 17] = [
    (
        "ring-6-12",
        [21.0, 21.0, 32.0, 28.0],
        [21.0, 21.0, 32.0, 28.0],
        [(6.0, 6.0), (6.0, 6.0), (6.0, 6.0), (6.0, 6.0)],
    ),
    (
        "ring-2-12",
        [77.0, 17.0, 40.0, 36.0],
        [77.0, 17.0, 40.0, 36.0],
        [(10.0, 10.0), (10.0, 10.0), (10.0, 10.0), (10.0, 10.0)],
    ),
    (
        "ring-10-8",
        [145.0, 25.0, 24.0, 20.0],
        [145.0, 25.0, 24.0, 20.0],
        [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
    ),
    (
        "sides",
        [205.0, 19.0, 24.0, 32.0],
        [205.0, 19.0, 24.0, 32.0],
        [(4.0, 10.0), (4.0, 10.0), (4.0, 10.0), (4.0, 10.0)],
    ),
    (
        "ring-ellipse",
        [261.0, 21.0, 32.0, 28.0],
        [261.0, 21.0, 32.0, 28.0],
        [(14.0, 4.0), (14.0, 4.0), (14.0, 4.0), (14.0, 4.0)],
    ),
    (
        "ring-percent",
        [321.0, 21.0, 32.0, 28.0],
        [321.0, 21.0, 32.0, 28.0],
        [(7.2, 6.0), (7.2, 6.0), (7.2, 6.0), (7.2, 6.0)],
    ),
    (
        "ring-1-4",
        [376.0, 16.0, 42.0, 38.0],
        [376.0, 16.0, 42.0, 38.0],
        [(3.0, 3.0), (3.0, 3.0), (3.0, 3.0), (3.0, 3.0)],
    ),
    (
        "pill",
        [18.0, 78.0, 38.0, 34.0],
        [18.0, 78.0, 38.0, 34.0],
        [(17.0, 17.0), (17.0, 17.0), (17.0, 17.0), (17.0, 17.0)],
    ),
    (
        "uneven",
        [79.0, 83.0, 38.0, 20.0],
        [79.0, 83.0, 38.0, 20.0],
        [(12.0, 8.0), (14.0, 8.0), (14.0, 4.0), (12.0, 4.0)],
    ),
    (
        "half",
        [141.0, 81.0, 32.0, 28.0],
        [141.0, 81.0, 32.0, 28.0],
        [(16.0, 14.0), (16.0, 14.0), (16.0, 14.0), (16.0, 14.0)],
    ),
    (
        "diagonals",
        [207.0, 87.0, 28.0, 24.0],
        [207.0, 87.0, 28.0, 24.0],
        [(6.0, 6.0), (2.0, 0.0), (14.0, 14.0), (0.0, 2.0)],
    ),
    (
        "fitted",
        [258.0, 85.0, 31.0, 17.0],
        [258.0, 85.0, 31.0, 17.0],
        [(17.0, 10.0), (10.0, 10.0), (10.0, 7.0), (17.0, 7.0)],
    ),
    (
        "fitted-percent",
        [319.0, 79.0, 30.0, 32.0],
        [319.0, 79.0, 30.0, 32.0],
        [(18.0, 16.0), (12.0, 16.0), (12.0, 16.0), (18.0, 16.0)],
    ),
    (
        "fitted-uneven",
        [388.0, 79.0, 28.0, 26.0],
        [388.0, 79.0, 28.0, 26.0],
        [(7.0, 16.0), (17.0, 16.0), (17.0, 10.0), (7.0, 10.0)],
    ),
    (
        "corners",
        [436.0, 77.0, 42.0, 34.0],
        [436.0, 77.0, 42.0, 34.0],
        [(15.0, 14.0), (14.0, 13.0), (21.0, 18.0), (11.0, 8.0)],
    ),
    (
        "tenths",
        [18.0, 139.0, 38.0, 34.0],
        [18.5, 138.5, 38.0, 34.0],
        [(7.0, 7.0), (7.0, 7.0), (7.0, 7.0), (7.0, 7.0)],
    ),
    (
        "tenths-wide",
        [82.0, 141.0, 32.0, 28.0],
        [81.5, 141.0, 32.0, 28.0],
        [(6.0, 6.0), (6.0, 6.0), (6.0, 6.0), (6.0, 6.0)],
    ),
];

/// The case's plain colours: the page's white, the borders' blue and the
/// clipped child's red.
const PLAIN: [(u8, u8, u8); 3] = [(255, 255, 255), (0x1d, 0x4e, 0xd8), (0xc0, 0x39, 0x2b)];

/// How far a curve's edge pixel may be from Chrome's, of 255 in any
/// channel, at each scale, where either picture has a blend. Measured on
/// this case. The worst at one, (260, 18), is on `ring-ellipse`'s outer
/// curve, which is the border's and not the clip's, drawn as it was before
/// item 588; the next, 79 at (84, 142), is on `tenths-wide`'s clip, where
/// the pixel's area under the curve is 0.83, Chrome's screenshot at eight
/// covers it by 0.77 and its screenshot at one by 0.46, which is Chrome's
/// own coarse smoothing at one rather than a different curve. The worst at
/// two, (753, 34), is inside `ring-1-4`'s one-pixel border, where the
/// border's inner curve and the clip's meet.
const EDGE: [(f32, u8); 2] = [(1.0, 82), (2.0, 75)];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("rounded-clips-inside-borders"))
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

/// The shape the box with `class` clips what is inside it to: its last
/// clip, after any its own border pushed to draw its sides.
fn clip_of(rendering: &Rendering, class: &str) -> Option<Path> {
    let id = box_of(rendering, class)?;
    rendering
        .drawing()?
        .display
        .items()
        .iter()
        .rev()
        .find_map(|item| match item {
            DisplayItem::PushClip { box_id, path } if *box_id == id => Some(path.clone()),
            _ => None,
        })
}

/// Where each segment of a path ends.
fn ends(path: &Path) -> Vec<(f32, f32)> {
    path.segments()
        .iter()
        .filter_map(|segment| match *segment {
            Segment::MoveTo(to)
            | Segment::LineTo(to)
            | Segment::QuadTo(_, to)
            | Segment::CubicTo(_, _, to) => Some((to.x, to.y)),
            Segment::Close => None,
        })
        .collect()
}

/// A rounded rectangle's corners, read back from where its lines and curves
/// end: it starts after the top-left curve and goes clockwise, a line and
/// then a curve per side. A plain rectangle, four points, has none.
fn radii(path: &Path) -> Option<[(f32, f32); 4]> {
    let (left, top, right, bottom) = path.bounds()?;
    let points = ends(path);
    if points.len() == 4 {
        return Some([(0.0, 0.0); 4]);
    }
    let at = |index: usize| points.get(index).copied();
    let (start, top_right, right_top) = (at(0)?, at(1)?, at(2)?);
    let (right_bottom, bottom_right, bottom_left) = (at(3)?, at(4)?, at(5)?);
    let (left_bottom, left_top) = (at(6)?, at(7)?);
    Some([
        (start.0 - left, left_top.1 - top),
        (right - top_right.0, right_top.1 - top),
        (right - bottom_right.0, bottom - right_bottom.1),
        (bottom_left.0 - left, bottom - left_bottom.1),
    ])
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001
}

#[test]
fn every_clip_is_the_padding_box_rounded_by_its_edges_curve() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, pick) in [(1.0, 0), (2.0, 1)] {
        let Ok(rendering) = Rendering::at_scale(&case, scale) else {
            panic!("the case renders at {scale}");
        };
        for (class, at_one, at_two, corners) in CLIPS {
            let Some(path) = clip_of(&rendering, class) else {
                panic!("{class} clips at {scale}");
            };
            let [left, top, width, height] = if pick == 0 { at_one } else { at_two };
            let Some(bounds) = path.bounds() else {
                panic!("{class}'s clip has an extent");
            };
            let (l, t, r, b) = bounds;
            assert!(
                close(l, left) && close(t, top) && close(r - l, width) && close(b - t, height),
                "{class} at {scale} clips to {bounds:?}",
            );
            let Some(found) = radii(&path) else {
                panic!("{class}'s clip is a rounded rectangle");
            };
            for (corner, (drawn, asked)) in found.iter().zip(corners).enumerate() {
                assert!(
                    close(drawn.0, asked.0) && close(drawn.1, asked.1),
                    "{class} at {scale}, corner {corner}: {drawn:?} where Chrome's is {asked:?}",
                );
            }
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
/// and at two: no white between a border and what it clips where Chrome
/// has none, and no red where Chrome's corner is clipped away. Where either
/// is a blend, a curve's edge, the two are within `EDGE`.
#[test]
fn every_corner_is_clipped_where_chromes_is() {
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
            "at {scale} a curve's edge is {} from Chrome's at {:?}",
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
