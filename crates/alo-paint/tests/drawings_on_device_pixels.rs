/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a drawing is clipped and placed, in numbers (queue item 589).
//!
//! Chrome 154 clips an `<svg>`'s drawing to its content box with each edge
//! on a whole device pixel, and draws it from its border box's corner on a
//! whole device pixel plus its border and padding as laid out; an SVG file
//! shown by an `<img>` is drawn into the rounded content box, grown to fill
//! it. The frozen page and Chrome's pictures of it are `alo-corpus`'
//! `svgs-on-device-pixels`; what is here is the rule on one box, at one and
//! at two.

use alo_box::{BoxId, BoxKind, build as build_boxes};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_paint::{
    DisplayItem, DisplayList, Drawing, DrawingItem, FillRule, PaintContext, Path, build,
};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{FontDatabase, TextMeasurer};
use alo_value::Rgba;

/// An `<svg>` with a padding of fractions inside a whole-pixel border, at
/// quarters of a pixel: its content box is laid out from (13, 13) to
/// (29.5, 29.25), at one and at two.
const PAGE: &str = "<!DOCTYPE html><html><body><svg></svg></body></html>";
const SHEET: &str = "body { margin: 0 }
svg { position: absolute; left: 10.25px; top: 10.25px; width: 16.5px; height: 16.25px;
      border: 2px solid #000000; padding: 0.75px 0 0.2px 0.75px }";

/// A square from (2, 2) to (7, 7) in the drawing's own coordinates.
fn square(confined: bool) -> Drawing {
    let mut drawing = Drawing::new();
    drawing.push(DrawingItem::Fill {
        path: Path::rectangle(2.0, 2.0, 5.0, 5.0),
        color: Rgba::BLACK,
        rule: FillRule::NonZero,
    });
    if confined {
        drawing.confined()
    } else {
        drawing
    }
}

/// The page's display list at `scale`, its `<svg>` holding `drawing`, or
/// nothing if the `<svg>` has no box.
fn drawn(scale: f32, drawing: Drawing) -> Option<DisplayList> {
    let document = parse_document(PAGE);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(SHEET);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default().at_scale(scale));
    let boxes = build_boxes(&document, &styles);
    let svg = document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| &*element.name.local == "svg")
    });
    let id: Option<BoxId> = boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|node| &node.kind),
            Some(BoxKind::Element { node, .. }) if Some(*node) == svg
        )
    });
    let id = id?;
    let fonts = FontDatabase::new();
    let measurer = TextMeasurer::new(&fonts);
    let layout = compute(&boxes, &styles, Size::new(100.0, 60.0), &measurer);
    let drawings = std::collections::BTreeMap::from([(id, drawing)]);
    Some(build::build(
        &boxes,
        &layout,
        &styles,
        PaintContext {
            fonts: &fonts,
            pictures: &std::collections::BTreeMap::new(),
            drawings: &drawings,
        },
    ))
}

/// `(left, top, right, bottom)` of the first clip and of the last fill.
fn clip_and_shape(list: Option<DisplayList>) -> (Option<[f32; 4]>, Option<[f32; 4]>) {
    let Some(list) = list else {
        return (None, None);
    };
    let edges = |path: &Path| path.bounds().map(|(l, t, r, b)| [l, t, r, b]);
    let clip = list.items().iter().find_map(|item| match item {
        DisplayItem::PushClip { path, .. } => edges(path),
        _ => None,
    });
    let shape = list.items().iter().rev().find_map(|item| match item {
        DisplayItem::Fill { path, .. } => edges(path),
        _ => None,
    });
    (clip, shape)
}

fn near(found: Option<[f32; 4]>, wanted: [f32; 4]) -> bool {
    found.is_some_and(|found| {
        found
            .iter()
            .zip(wanted)
            .all(|(found, wanted)| (found - wanted).abs() < 0.002)
    })
}

/// The clip is the content box, each edge rounded on its own; the square is
/// drawn from the border box's corner rounded, (10, 10) at one and
/// (10.5, 10.5) at two, plus the border and the padding as laid out — a
/// fraction into a pixel, where the content box rounded would put it on
/// whole ones.
#[test]
fn an_svg_is_clipped_on_device_pixels_and_drawn_from_its_rounded_corner() {
    let (clip, shape) = clip_and_shape(drawn(1.0, square(false)));
    assert!(near(clip, [13.0, 13.0, 30.0, 29.0]), "{clip:?}");
    assert!(near(shape, [14.75, 14.75, 19.75, 19.75]), "{shape:?}");
    let (clip, shape) = clip_and_shape(drawn(2.0, square(false)));
    assert!(near(clip, [13.0, 13.0, 29.5, 29.5]), "{clip:?}");
    assert!(near(shape, [15.25, 15.25, 20.25, 20.25]), "{shape:?}");
}

/// An SVG file's drawing is a picture: drawn into the rounded content box,
/// grown from the 16.5 by 16.25 it was made at to the 17 by 16 it is drawn
/// at, at one, and to 16.5 by 16.5 at two.
#[test]
fn a_pictures_drawing_fills_its_rounded_content_box() {
    let (clip, shape) = clip_and_shape(drawn(1.0, square(true)));
    assert!(near(clip, [13.0, 13.0, 30.0, 29.0]), "{clip:?}");
    let (x, y) = (17.0 / 16.5, 16.0 / 16.25);
    let wanted = [
        13.0 + 2.0 * x,
        13.0 + 2.0 * y,
        13.0 + 7.0 * x,
        13.0 + 7.0 * y,
    ];
    assert!(near(shape, wanted), "{shape:?}");
    let (clip, shape) = clip_and_shape(drawn(2.0, square(true)));
    assert!(near(clip, [13.0, 13.0, 29.5, 29.5]), "{clip:?}");
    let y = 16.5 / 16.25;
    let wanted = [15.0, 13.0 + 2.0 * y, 20.0, 13.0 + 7.0 * y];
    assert!(near(shape, wanted), "{shape:?}");
}
