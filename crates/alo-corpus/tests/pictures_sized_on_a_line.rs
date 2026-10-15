/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Pictures on a line with their width and their height both set: the case
//! that closes queue item 590.
//!
//! `cases/pictures-sized-on-a-line` puts four lines of `<img>`s beside
//! text: a square picture at 30 by 20, at tenths, with a border, with a
//! padding of fractions and `border-box`; a 2:1 picture made narrow and
//! tall; one with a height alone and one with a width alone, which still
//! take the other from the picture's ratio; at `middle`, `top` and a
//! length; inside an inline block; one held by a `max-height`; and a line
//! of CSS 2 § 10.4's table, nothing fixed and each limit in turn. Then the
//! same pictures as flex items, in a column, as blocks and positioned.
//!
//! This engine handed a picture's ratio to `taffy`, which holds a box at
//! least its width over its ratio tall whatever its style fixed — so
//! `both` was 30 by 30, on a line and as a block — and carries a
//! `max-height` across the ratio into a width, so `capped`, `width: 60px`,
//! was 24.4 wide. A replaced box is sized by `alo-layout`'s `replaced.rs`
//! now, and `taffy` is handed only a ratio an author wrote.
//!
//! `chrome-at-1.txt` and `chrome-at-2.txt` beside the case are Chrome 154's
//! `getBoundingClientRect()` for every element, headless, started with
//! `--force-device-scale-factor=1` and `=2` (never the emulated scale of
//! Chrome's developer tools), the page set in the `dejavu` crate's `DejaVu
//! Sans`, measured 2026-10-14. `chrome.png` and `chrome-at-2.png` are its
//! screenshots of the same bytes in the same runs.
//!
//! `render-at-2.png` is this engine's picture at two, beside `render.png`
//! and never in its place (ADR 0057 § 5).

use std::collections::{BTreeMap, BTreeSet};

use alo_box::BoxId;
use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_dom::{Document, NodeId};
use alo_paint::display::DisplayItem;
use alo_renderer::Drawing;
use alo_value::Rgba;

/// An element's border box in sixty-fourths of a CSS pixel: left, top,
/// width, height.
type Units = [f32; 4];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("pictures-sized-on-a-line"))
}

/// The case drawn at `scale`: its document and its drawing.
fn drawn(case: &Case, scale: f32) -> Result<(Document, std::rc::Rc<Drawing>), String> {
    let rendering = Rendering::at_scale(case, scale)?;
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        return Err(format!("no document or no drawing at {scale}"));
    };
    Ok((document.clone(), drawing))
}

/// Chrome's boxes at `scale`, by path.
fn chrome(case: &Case, scale: u8) -> Option<BTreeMap<String, Units>> {
    let text = std::fs::read_to_string(case.expectation(&format!("chrome-at-{scale}.txt"))).ok()?;
    Some(
        text.lines()
            .filter(|line| !line.starts_with('#'))
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                let path = parts.next()?.to_owned();
                let mut units = [0.0; 4];
                for unit in &mut units {
                    *unit = parts.next()?.parse().ok()?;
                }
                Some((path, units))
            })
            .collect(),
    )
}

/// Every element's border box in this engine's drawing, by the path the
/// Chrome files name (`/html0/body1/div0` is the first element child of the
/// second element child of the first).
fn ours(document: &Document, drawing: &Drawing) -> BTreeMap<String, Units> {
    boxes(document, drawing)
        .into_iter()
        .filter_map(|(path, id)| {
            let border = drawing.layout.get(id)?.border_box;
            Some((
                path,
                [
                    border.origin.x,
                    border.origin.y,
                    border.size.width,
                    border.size.height,
                ]
                .map(|v| v * 64.0),
            ))
        })
        .collect()
}

/// Every element's first box, by its path.
fn boxes(document: &Document, drawing: &Drawing) -> Vec<(String, BoxId)> {
    let mut first = BTreeMap::new();
    for id in drawing.boxes.ids() {
        if let Some(alo_box::BoxKind::Element { node, .. }) =
            drawing.boxes.get(id).map(|found| &found.kind)
        {
            first.entry(*node).or_insert(id);
        }
    }
    let mut paths = Vec::new();
    walk(document, document.root(), "", &mut paths);
    paths
        .into_iter()
        .filter_map(|(path, node)| Some((path, *first.get(&node)?)))
        .collect()
}

/// The pictures drawn smaller than they are in device pixels at `scale`:
/// those smaller on one axis only, which Chrome draws bilinear unless an
/// earlier draw of the file went through its levels, and those smaller on
/// both, which go through them (queue item 179).
fn drawn_smaller(
    document: &Document,
    drawing: &Drawing,
    scale: f32,
) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut one_axis = BTreeSet::new();
    let mut both = BTreeSet::new();
    for item in drawing.display.items() {
        let DisplayItem::Picture {
            box_id,
            rect,
            picture,
        } = item
        else {
            continue;
        };
        let (across, down) = (rect.size.width * scale, rect.size.height * scale);
        let (Ok(wide), Ok(tall)) = (
            u16::try_from(picture.width()),
            u16::try_from(picture.height()),
        ) else {
            continue;
        };
        let narrower = across.round() < f32::from(wide);
        let shorter = down.round() < f32::from(tall);
        if narrower && shorter {
            both.insert(*box_id);
        } else if narrower || shorter {
            one_axis.insert(*box_id);
        }
    }
    let named = |ids: &BTreeSet<BoxId>| {
        boxes(document, drawing)
            .into_iter()
            .filter(|(_, id)| ids.contains(id))
            .map(|(path, _)| path)
            .collect()
    };
    (named(&one_axis), named(&both))
}

fn walk(document: &Document, node: NodeId, path: &str, out: &mut Vec<(String, NodeId)>) {
    let mut index = 0;
    for child in document.children(node) {
        let Some(element) = document.element(child) else {
            continue;
        };
        let here = format!("{path}/{}{index}", element.name.local);
        index += 1;
        out.push((here.clone(), child));
        walk(document, child, &here, out);
    }
}

/// Every box — each picture, each line and the inline block — is Chrome's
/// to within a quarter of a sixty-fourth, at one and at two. `both` was 30
/// by 30 here, its height its ratio's, and `capped` 30 high.
#[test]
fn every_box_is_where_chrome_puts_it_at_one_and_at_two() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for scale in [1_u8, 2] {
        let (document, drawing) = match drawn(&case, f32::from(scale)) {
            Ok(both) => both,
            Err(why) => panic!("at {scale}: {why}"),
        };
        let ours = ours(&document, &drawing);
        let Some(chrome) = chrome(&case, scale) else {
            panic!("Chrome's boxes at {scale} are beside the case");
        };
        let mut compared = 0;
        let mut wrong = Vec::new();
        for (path, theirs) in &chrome {
            if path.starts_with("/html0/head") {
                continue;
            }
            let Some(mine) = ours.get(path) else {
                panic!("{path} at {scale} has no box here; Chrome's is {theirs:?}");
            };
            if !mine.iter().zip(theirs).all(|(a, b)| (a - b).abs() < 0.25) {
                wrong.push(format!("{path}: Chrome {theirs:?}, this engine {mine:?}"));
            }
            compared += 1;
        }
        assert!(wrong.is_empty(), "at {scale}:\n{}", wrong.join("\n"));
        // The root, the body, seven lines and rows, an inline block and
        // twenty-five pictures.
        assert_eq!(compared, 35, "at {scale}");
    }
}

/// Every device pixel inside the border box of every picture, across and
/// down, is Chrome's exactly, at one and at two: the picture smoothed across
/// its texels as Chrome smooths it (queue item 594), through Chrome's mipmap
/// levels where it is drawn smaller on both axes (179), the seams where
/// `wide.png`'s halves meet with it, and its border and padding, in the
/// place and at the size Chrome lays them out. At one, five pictures are
/// smaller on both axes: `narrowed`, 25 × 12.5, mixed from two levels,
/// `boxed-in`, 20 × 10, level one's own texels, and three too little
/// smaller to mix. `wide.png`'s pictures after `narrowed`, though larger
/// across — `floored-and-capped` and the four 60 × 12.2 — are mixed from
/// its levels too, because Chrome holds one copy of the file and its first
/// draw through levels gave that copy levels; `wide-tall`, drawn before it,
/// is bilinear. At two none is smaller on both. The letters between
/// pictures are left out — Chrome thickens them on macOS (item 576).
#[test]
fn every_picture_is_drawn_as_chrome_draws_it() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, name) in [(1_u8, "chrome.png"), (2, "chrome-at-2.png")] {
        let (document, drawing) = match drawn(&case, f32::from(scale)) {
            Ok(both) => both,
            Err(why) => panic!("at {scale}: {why}"),
        };
        let Some(chrome) = std::fs::read(case.expectation(name))
            .ok()
            .and_then(|bytes| alo_paint::from_png(&bytes).ok())
        else {
            panic!("Chrome's picture at {scale} is beside the case");
        };
        let ours = &drawing.canvas;
        assert_eq!(
            (ours.width(), ours.height()),
            (chrome.width(), chrome.height()),
            "at {scale}"
        );
        let pictures = self::ours(&document, &drawing)
            .into_iter()
            .filter(|(path, _)| path.contains("/img"))
            .collect::<Vec<_>>();
        assert_eq!(pictures.len(), 25, "at {scale}");
        let (one_axis, both) = drawn_smaller(&document, &drawing, f32::from(scale));
        assert_eq!(
            (one_axis.len(), both.len()),
            if scale == 1 { (15, 5) } else { (4, 0) },
            "at {scale}: smaller on one axis {one_axis:?}, on both {both:?}"
        );
        let per_unit = f32::from(scale) / 64.0;
        let mut wrong = Vec::new();
        for (path, [left, top, width, height]) in pictures {
            // The device pixels wholly inside the border box: an edge a
            // fraction into a pixel is a blend of the picture and the page.
            let (x0, y0) = ((left * per_unit).ceil(), (top * per_unit).ceil());
            let (x1, y1) = (
                ((left + width) * per_unit).floor(),
                ((top + height) * per_unit).floor(),
            );
            for y in pixels(y0, y1) {
                for x in pixels(x0, x1) {
                    let (mine, theirs) = (
                        ours.at(x, y).map(Rgba::to_rgba8),
                        chrome.at(x, y).map(Rgba::to_rgba8),
                    );
                    if mine != theirs {
                        wrong.push(format!(
                            "{path} ({x}, {y}): Chrome {theirs:?}, this engine {mine:?}"
                        ));
                    }
                }
            }
        }
        assert!(wrong.is_empty(), "at {scale}:\n{}", wrong.join("\n"));
    }
}

/// The device pixels from `start` up to `end`, each a whole number.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "whole numbers on a 1000 × 280 canvas, never negative"
)]
fn pixels(start: f32, end: f32) -> std::ops::Range<u32> {
    (start.max(0.0) as u32)..(end.max(0.0) as u32)
}

#[test]
fn the_picture_at_two_is_its_reference() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let drawing = match drawn(&case, 2.0) {
        Ok((_, drawing)) => drawing,
        Err(why) => panic!("{why}"),
    };
    let difference = check_picture(&case, "render-at-2.png", &drawing.canvas);
    assert!(difference.is_none(), "{difference:?}");
}
