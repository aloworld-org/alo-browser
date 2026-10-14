/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Borders whose sides differ, with rounded corners: the case that closes
//! queue item 592.
//!
//! `cases/borders-side-by-side` places sixteen boxes 44 by 40, each with a
//! border whose sides differ in width, colour or both, most of them with
//! rounded corners: one colour on sides of two widths and of four, a side
//! with no width, two and four colours, an ellipse, corners of four radii,
//! a radius smaller than the border so the inside is square, translucent
//! colours, an `inset` and a `double` border, and two boxes laid out at
//! tenths of a pixel.
//!
//! Chrome 154 draws a solid border of one colour as one ring — the border
//! box's shape less the padding box's — and any other side by side, each
//! side the part of the ring on its own side of the mitre through the
//! border box's corner and the padding box's, carried on into a rounded
//! corner as far as its inner curve: measured on these boxes at one, two
//! and eight, not read from its source. This engine drew a solid border of
//! differing sides as four rectangles clipped to the box's outer curve,
//! which gave one side the whole of a square corner and left white inside
//! every rounded one, where the padding box's curve bulges into the
//! corner: 654 pixels at one and 2,987 at two were a different plain
//! colour from Chrome's.
//!
//! `chrome.png` and `chrome-at-2.png` beside the case are Chrome 154's
//! screenshots of the same bytes, 500 by 200 CSS pixels, headless, started
//! with `--force-device-scale-factor=1` and `=2`, without `--disable-gpu`,
//! taken 2026-10-14.
//!
//! `render-at-2.png` is this engine's picture at two, beside `render.png`
//! and never in its place (ADR 0057 § 5).

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_paint::Canvas;
use alo_paint::display::DisplayItem;
use alo_paint::path::{Path, Segment};

/// How a border is drawn: one ring of one colour, or its sides inside a
/// clip to the ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drawn {
    Ring,
    Sides,
}

/// Each box's border box as drawn at one and at two, as left, top, width
/// and height; its border's widths, top, right, bottom and left; and how it
/// is drawn — worked out by hand from the style and ADR 0057's snapping.
type Border = (&'static str, [f32; 4], [f32; 4], [f32; 4], Drawn);

const BORDERS: [Border; 16] = [
    (
        "one-colour",
        [15.0, 15.0, 44.0, 40.0],
        [15.0, 15.0, 44.0, 40.0],
        [4.0, 10.0, 4.0, 10.0],
        Drawn::Ring,
    ),
    (
        "uneven",
        [75.0, 15.0, 44.0, 40.0],
        [75.0, 15.0, 44.0, 40.0],
        [8.0, 2.0, 12.0, 4.0],
        Drawn::Ring,
    ),
    (
        "two-colours",
        [135.0, 15.0, 44.0, 40.0],
        [135.0, 15.0, 44.0, 40.0],
        [4.0, 10.0, 4.0, 10.0],
        Drawn::Sides,
    ),
    (
        "square",
        [195.0, 15.0, 44.0, 40.0],
        [195.0, 15.0, 44.0, 40.0],
        [4.0, 6.0, 8.0, 10.0],
        Drawn::Sides,
    ),
    (
        "four",
        [255.0, 15.0, 44.0, 40.0],
        [255.0, 15.0, 44.0, 40.0],
        [4.0, 6.0, 8.0, 10.0],
        Drawn::Sides,
    ),
    (
        "ellipse",
        [315.0, 15.0, 44.0, 40.0],
        [315.0, 15.0, 44.0, 40.0],
        [6.0, 3.0, 6.0, 3.0],
        Drawn::Sides,
    ),
    (
        "no-left",
        [375.0, 15.0, 44.0, 40.0],
        [375.0, 15.0, 44.0, 40.0],
        [4.0, 10.0, 4.0, 0.0],
        Drawn::Ring,
    ),
    (
        "thick",
        [435.0, 15.0, 44.0, 40.0],
        [435.0, 15.0, 44.0, 40.0],
        [12.0, 4.0, 12.0, 4.0],
        Drawn::Sides,
    ),
    (
        "inset",
        [15.0, 75.0, 44.0, 40.0],
        [15.0, 75.0, 44.0, 40.0],
        [4.0, 10.0, 4.0, 10.0],
        Drawn::Sides,
    ),
    (
        "double",
        [75.0, 75.0, 44.0, 40.0],
        [75.0, 75.0, 44.0, 40.0],
        [6.0, 12.0, 6.0, 12.0],
        Drawn::Sides,
    ),
    (
        "squared-inside",
        [135.0, 75.0, 44.0, 40.0],
        [135.0, 75.0, 44.0, 40.0],
        [10.0, 2.0, 10.0, 2.0],
        Drawn::Sides,
    ),
    (
        "corners",
        [195.0, 75.0, 44.0, 40.0],
        [195.0, 75.0, 44.0, 40.0],
        [2.0, 5.0, 7.0, 3.0],
        Drawn::Sides,
    ),
    (
        "translucent",
        [255.0, 75.0, 44.0, 40.0],
        [255.0, 75.0, 44.0, 40.0],
        [4.0, 10.0, 4.0, 10.0],
        Drawn::Ring,
    ),
    (
        "translucent-two",
        [315.0, 75.0, 44.0, 40.0],
        [315.0, 75.0, 44.0, 40.0],
        [4.0, 10.0, 4.0, 10.0],
        Drawn::Sides,
    ),
    // Laid out at 15.296875 and 135.59375, on the grid of a sixty-fourth
    // and of a hundred-and-twenty-eighth alike, as Chrome lays it out; each
    // edge rounded to the nearest device pixel.
    (
        "tenths",
        [15.0, 136.0, 44.0, 40.0],
        [15.5, 135.5, 44.0, 40.0],
        [4.0, 10.0, 4.0, 10.0],
        Drawn::Sides,
    ),
    // At 75.6875 and 135.1875.
    (
        "tenths-one",
        [76.0, 135.0, 44.0, 40.0],
        [75.5, 135.0, 44.0, 40.0],
        [3.0, 7.0, 5.0, 9.0],
        Drawn::Ring,
    ),
];

/// The case's plain colours: the page's white; the borders' blue, red,
/// green and black; the translucent blue and red over white; and an
/// `inset` grey's two tones, as Chrome's screenshot has them.
const PLAIN: [(u8, u8, u8); 9] = [
    (255, 255, 255),
    (0x1d, 0x4e, 0xd8),
    (0xc0, 0x39, 0x2b),
    (0x16, 0xa3, 0x4a),
    (0, 0, 0),
    (142, 166, 235),
    (223, 156, 149),
    (44, 44, 44),
    (212, 212, 212),
];

/// How far a blended pixel may be from Chrome's, of 255 in any channel, at
/// each scale, where either picture has a blend. Measured on this case:
/// the worst at one, (438, 25), and at two, (947, 78), are on `thick`'s
/// outer curve, which a ring and four rectangles both drew as they draw
/// now — Chrome's own coarse smoothing of a curve at one, and of a 20 px
/// radius at two (`rounded-clips-inside-borders` bounds its curves alike).
const EDGE: [(f32, u8); 2] = [(1.0, 95), (2.0, 99)];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("borders-side-by-side"))
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

/// How the box with `class` drew its border, and the ring it drew it in:
/// the fill itself for one ring, the first clip for sides.
fn border_of(rendering: &Rendering, class: &str) -> Option<(Drawn, Path)> {
    let id = box_of(rendering, class)?;
    let drawing = rendering.drawing()?;
    let own: Vec<&DisplayItem> = drawing
        .display
        .items()
        .iter()
        .filter(|item| match item {
            DisplayItem::PushClip { box_id, .. } | DisplayItem::Fill { box_id, .. } => {
                *box_id == id
            }
            _ => false,
        })
        .collect();
    match own.as_slice() {
        [DisplayItem::Fill { path, .. }] => Some((Drawn::Ring, path.clone())),
        [DisplayItem::PushClip { path, .. }, ..] => Some((Drawn::Sides, path.clone())),
        _ => None,
    }
}

/// Each outline of a path as how far it reaches: left, top, right and
/// bottom, from where its lines and curves end — which, for a rounded
/// rectangle, are its extremes.
fn outlines(path: &Path) -> Vec<[f32; 4]> {
    let mut found: Vec<[f32; 4]> = Vec::new();
    for segment in path.segments() {
        match *segment {
            Segment::MoveTo(to) => found.push([to.x, to.y, to.x, to.y]),
            Segment::LineTo(to) | Segment::QuadTo(_, to) | Segment::CubicTo(_, _, to) => {
                if let Some(reach) = found.last_mut() {
                    *reach = [
                        reach[0].min(to.x),
                        reach[1].min(to.y),
                        reach[2].max(to.x),
                        reach[3].max(to.y),
                    ];
                }
            }
            Segment::Close => {}
        }
    }
    found
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001
}

/// Every border is a ring from its border box to its padding box — drawn
/// as that ring when it is one colour, and as its sides inside it when
/// they differ — at one and at two.
#[test]
fn every_border_is_the_ring_from_its_border_box_to_its_padding_box() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for scale in [1.0, 2.0] {
        let Ok(rendering) = Rendering::at_scale(&case, scale) else {
            panic!("the case renders at {scale}");
        };
        for (class, at_one, at_two, [top, right, bottom, left], drawn) in BORDERS {
            let Some((how, ring)) = border_of(&rendering, class) else {
                panic!("{class} draws a border at {scale}");
            };
            assert_eq!(how, drawn, "{class} at {scale}");
            let [x, y, width, height] = if scale > 1.0 { at_two } else { at_one };
            let outer = [x, y, x + width, y + height];
            let inner = [x + left, y + top, x + width - right, y + height - bottom];
            // A `double` border's clip is its two lines, each a ring a third
            // as thick: the outermost outline and the innermost are still
            // the border box and the padding box.
            let found = outlines(&ring);
            let rings = if class == "double" { 4 } else { 2 };
            assert_eq!(found.len(), rings, "{class} at {scale}: {found:?}");
            let ends = [found.first(), found.last()];
            for (reach, expected) in ends.into_iter().flatten().zip([outer, inner]) {
                assert!(
                    reach.iter().zip(expected).all(|(a, b)| close(*a, b)),
                    "{class} at {scale}: {reach:?} where {expected:?} is drawn",
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
/// and at two: no white inside a rounded corner where Chrome draws the
/// border, and every corner split between two colours where Chrome splits
/// it. Where either is a blend — a curve's edge, or the mitre between two
/// colours — the two are within `EDGE`.
#[test]
fn every_corner_is_filled_and_split_where_chromes_is() {
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
