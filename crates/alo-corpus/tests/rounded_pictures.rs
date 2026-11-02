/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Pictures rounded by their own `border-radius`: the case that closes
//! queue item 769.
//!
//! `cases/rounded-pictures` places eleven `<img>`s of one solid colour,
//! each with its own radius — 4 pixels, 10 inside a blue border, 20
//! inside a red padding, 4 inside a padding of 5 (`squared`), 50 %, an
//! ellipse, `12px 4px` inside a border and a padding, 100 on a box 60 ×
//! 30, four different corners, and two laid out at fractions of a pixel.
//!
//! A picture is trimmed to its content edge's curve: each radius resolved
//! against the border box, fitted to it, then less the border and the
//! padding beside it, never below square (CSS Backgrounds 3 § 5.3). Chrome
//! 156.0.8078.76 does, in 27 of 27 probes at eight to within 0.09 of a
//! pixel's coverage, measured, not read from its source
//! (`alo-paint`'s `content_curve.rs`). This engine drew every one square.
//!
//! `chrome.png` and `chrome-at-2.png` beside the case are Chrome 156's
//! screenshots of the same bytes, 500 by 130 CSS pixels, headless, with
//! `--force-device-scale-factor=1` and `=2`, without `--disable-gpu`, taken
//! 2026-11-01. Wherever both pictures hold a plain colour it is the same
//! one at both scales. On the curves' edge pixels the two differ by at
//! most [`WHOLE_EDGE`], and by `tenths` and `tenths-ringed`, laid out at
//! fractions of a pixel, by at most [`EDGE`]: since queue item 783 their
//! curve is laid on the snapped border box less the border and the padding
//! as laid out, as Chrome's is, not on the picture's own snapped box —
//! which made `tenths`' top left look like a radius of 2.5, not 5.25, at
//! one (`rounded_pictures_at_fractions.rs` holds 48 more).
//!
//! `render-at-2.png` is this engine's picture at two, beside `render.png`
//! and never in its place (ADR 0057 § 5).

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_layout::{Point, Rect, Size};
use alo_paint::display::DisplayItem;
use alo_paint::path::{Path, Segment};
use alo_paint::{Canvas, Corners, rounded_rectangle};

/// Each picture's trim: its picture's rectangle at one and at two, as
/// left, top, width and height, and its corners' radii, across and down,
/// from the top left clockwise — the rule Chrome was fitted to, worked out
/// by hand from the case's style sheet. Square corners are no trim.
type Trim = (&'static str, [f32; 4], [f32; 4], [(f32, f32); 4]);

const TRIMS: [Trim; 11] = [
    (
        "r4",
        [10.0, 10.0, 40.0, 40.0],
        [10.0, 10.0, 40.0, 40.0],
        [(4.0, 4.0); 4],
    ),
    (
        "ringed",
        [73.0, 13.0, 40.0, 40.0],
        [73.0, 13.0, 40.0, 40.0],
        [(7.0, 7.0); 4],
    ),
    (
        "padded",
        [135.0, 15.0, 40.0, 40.0],
        [135.0, 15.0, 40.0, 40.0],
        [(15.0, 15.0); 4],
    ),
    (
        "squared",
        [205.0, 15.0, 40.0, 40.0],
        [205.0, 15.0, 40.0, 40.0],
        [(0.0, 0.0); 4],
    ),
    (
        "circle",
        [270.0, 10.0, 40.0, 40.0],
        [270.0, 10.0, 40.0, 40.0],
        [(20.0, 20.0); 4],
    ),
    (
        "ellipse",
        [330.0, 10.0, 40.0, 40.0],
        [330.0, 10.0, 40.0, 40.0],
        [(30.0, 10.0); 4],
    ),
    (
        "uneven",
        [395.0, 15.0, 40.0, 40.0],
        [395.0, 15.0, 40.0, 40.0],
        [(7.0, 7.0), (0.0, 0.0), (7.0, 7.0), (0.0, 0.0)],
    ),
    (
        "fitted",
        [10.0, 70.0, 60.0, 30.0],
        [10.0, 70.0, 60.0, 30.0],
        [(15.0, 15.0); 4],
    ),
    (
        "corners",
        [90.0, 70.0, 60.0, 44.0],
        [90.0, 70.0, 60.0, 44.0],
        [(4.0, 4.0), (12.0, 12.0), (20.0, 20.0), (8.0, 8.0)],
    ),
    (
        "tenths",
        [173.0, 73.0, 41.0, 33.0],
        [172.5, 72.5, 41.5, 33.5],
        [(5.25, 5.25); 4],
    ),
    (
        "tenths-ringed",
        [243.0, 73.0, 40.0, 40.0],
        [242.5, 72.5, 40.5, 40.5],
        [(7.5, 7.5); 4],
    ),
];

/// The rectangle a picture's curve rounds, where it is not the picture's
/// own: the snapped border box less the border and the padding as laid out
/// (queue item 783), at one and at two. `tenths`' border box, laid out at
/// (170.3, 70.4) 46 × 37.75, snaps to 170–216 by 70–108 at one and to
/// 170.5–216.5 by 70.5–108 at two; less its padding of 2.25 it is not the
/// picture's own box, 173–214 by 73–106 at one and 172.5–214 by 72.5–106 at
/// two. `tenths-ringed`'s, less its whole border, is its picture's.
const CURVED_ELSEWHERE: [(&str, [f32; 4], [f32; 4]); 1] = [(
    "tenths",
    [172.25, 72.25, 41.5, 33.5],
    [172.75, 72.75, 41.5, 33.0],
)];

/// The case's plain colours: the page's white, the picture's ink, the
/// borders' blue and the paddings' red.
const PLAIN: [(u8, u8, u8); 4] = [
    (255, 255, 255),
    (23, 33, 43),
    (0x1d, 0x4e, 0xd8),
    (0xc0, 0x39, 0x2b),
];

/// How far a curve's edge pixel of the two pictures laid out at fractions
/// of a pixel may be from Chrome's, of 255 in any channel, at each scale,
/// where either picture has a blend. Measured on this case since queue item
/// 783: 8 at one, at (172, 71), and 17 at two, at (565, 223) — where
/// before it, with the curve laid on the picture's own snapped box, they
/// were 131 at (175, 73) and 117 at (350, 146), on `tenths`' top left.
const EDGE: [(f32, u8); 2] = [(1.0, 8), (2.0, 17)];

/// How far a curve's edge pixel away from the pictures at fractions of a
/// pixel may be from Chrome's, at each scale: Chrome's smoothing against
/// this engine's. Measured: 53 at one, at (431, 59) on `uneven`'s bottom
/// right, where its picture's curve, its padding's and its border's meet;
/// 53 at two, at (791, 23), on `uneven`'s border's own outer curve.
const WHOLE_EDGE: [(f32, u8); 2] = [(1.0, 53), (2.0, 53)];

/// Where the pictures laid out at fractions of a pixel are, at one, as
/// left, top, right and bottom, a pixel of margin round each.
const FRACTIONS: [[u32; 4]; 2] = [[169, 69, 217, 110], [239, 69, 286, 116]];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("rounded-pictures"))
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

/// What the box with `class` draws its picture into, and the clip it is
/// drawn under, if any: the clip pushed for that box just before it.
fn picture_of(rendering: &Rendering, class: &str) -> Option<(Rect, Option<Path>)> {
    let id = box_of(rendering, class)?;
    let drawing = rendering.drawing()?;
    let items = drawing.display.items();
    let at = items
        .iter()
        .position(|item| matches!(item, DisplayItem::Picture { box_id, .. } if *box_id == id))?;
    let DisplayItem::Picture { rect, .. } = items.get(at)? else {
        return None;
    };
    let clip = at
        .checked_sub(1)
        .and_then(|before| items.get(before))
        .and_then(|item| match item {
            DisplayItem::PushClip { box_id, path } if *box_id == id => Some(path.clone()),
            _ => None,
        });
    Some((*rect, clip))
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001
}

/// A segment as its kind and the points it names.
fn points(segment: &Segment) -> (u8, Vec<alo_paint::Point>) {
    match *segment {
        Segment::MoveTo(to) => (0, vec![to]),
        Segment::LineTo(to) => (1, vec![to]),
        Segment::QuadTo(control, to) => (2, vec![control, to]),
        Segment::CubicTo(first, second, to) => (3, vec![first, second, to]),
        Segment::Close => (4, Vec::new()),
    }
}

/// Whether two paths are the same segments at the same points.
fn same(one: &Path, other: &Path) -> bool {
    one.segments().len() == other.segments().len()
        && one
            .segments()
            .iter()
            .zip(other.segments())
            .all(|(mine, theirs)| {
                let ((kind, at), (their_kind, their_at)) = (points(mine), points(theirs));
                kind == their_kind
                    && at
                        .iter()
                        .zip(&their_at)
                        .all(|(p, q)| close(p.x, q.x) && close(p.y, q.y))
            })
}

#[test]
fn every_picture_is_trimmed_to_its_content_edges_curve() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, pick) in [(1.0, 0), (2.0, 1)] {
        let Ok(rendering) = Rendering::at_scale(&case, scale) else {
            panic!("the case renders at {scale}");
        };
        for (class, at_one, at_two, radii) in TRIMS {
            let Some((rect, clip)) = picture_of(&rendering, class) else {
                panic!("{class} draws a picture at {scale}");
            };
            let [left, top, width, height] = if pick == 0 { at_one } else { at_two };
            let expected = Rect {
                origin: Point { x: left, y: top },
                size: Size { width, height },
            };
            assert!(
                close(rect.origin.x, left)
                    && close(rect.origin.y, top)
                    && close(rect.size.width, width)
                    && close(rect.size.height, height),
                "{class} at {scale} is drawn into {rect:?}"
            );
            let corners = Corners {
                top_left: radii[0],
                top_right: radii[1],
                bottom_right: radii[2],
                bottom_left: radii[3],
            };
            if corners.are_square() {
                assert!(clip.is_none(), "{class} at {scale} is trimmed: {clip:?}");
                continue;
            }
            let Some(clip) = clip else {
                panic!("{class} at {scale} is drawn square");
            };
            let curved = CURVED_ELSEWHERE
                .iter()
                .find(|(name, _, _)| *name == class)
                .map_or(expected, |(_, one, two)| {
                    let [left, top, width, height] = if pick == 0 { *one } else { *two };
                    Rect {
                        origin: Point { x: left, y: top },
                        size: Size { width, height },
                    }
                });
            assert!(
                same(&clip, &rounded_rectangle(curved, corners)),
                "{class} at {scale} is trimmed to {:?}, not {corners:?}",
                clip.segments()
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

/// Whether the pixel at `(x, y)` of a picture at `scale`, one or two, is
/// by a picture laid out at fractions of a pixel.
fn by_a_fraction(x: u32, y: u32, scale: u32) -> bool {
    FRACTIONS.iter().any(|[left, top, right, bottom]| {
        (left * scale..right * scale).contains(&x) && (top * scale..bottom * scale).contains(&y)
    })
}

/// Wherever both pictures have a plain colour it is the same one, at one
/// and at two: no square corner of ink where Chrome's is trimmed, and no
/// white or red where Chrome's picture is. Where either is a blend, a
/// curve's edge, the two are within `EDGE` by the two pictures at
/// fractions of a pixel, and within `WHOLE_EDGE` away from them.
#[test]
fn every_picture_is_trimmed_where_chromes_is() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for ((scale, edge), (_, whole)) in EDGE.into_iter().zip(WHOLE_EDGE) {
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
        let mut worst_whole = (0, None);
        let mut blends = 0;
        for y in 0..ours.height() {
            for x in 0..ours.width() {
                if let (Some(a), Some(b)) = (plain(ours, x, y), plain(&chrome, x, y)) {
                    assert_eq!(a, b, "at {scale}, ({x}, {y}) is a different colour");
                    continue;
                }
                blends += 1;
                let found = difference(ours, &chrome, x, y);
                if by_a_fraction(x, y, if scale > 1.0 { 2 } else { 1 }) {
                    if found > worst.0 {
                        worst = (found, Some((x, y)));
                    }
                } else if found > worst_whole.0 {
                    worst_whole = (found, Some((x, y)));
                }
            }
        }
        assert!(blends > 0, "at {scale} no curve was compared");
        assert!(
            worst.0 <= edge,
            "at {scale} a fractional picture's edge is {} from Chrome's at {:?}",
            worst.0,
            worst.1,
        );
        assert!(
            worst_whole.0 <= whole,
            "at {scale} a whole-pixel picture's edge is {} from Chrome's at {:?}",
            worst_whole.0,
            worst_whole.1,
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
