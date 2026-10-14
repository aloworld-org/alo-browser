/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Lines centred in odd leftover room and boxes aligned `middle`, put on
//! the layout grid as Chrome puts them: the case that closes queue item 598.
//!
//! Chrome 154 halves the leftover room of a centred line and truncates the
//! half onto its grid of a sixty-fourth of a device pixel; it raises a box
//! aligned `middle` by half its parent's x-height rounded to that grid, a
//! half going up, less half the box's own height truncated. This engine
//! left each half where it fell, so `cases/centred-and-middle-on-the-grid`
//! had 40, 34 and 32 of its 118 boxes off Chrome's at one, 1.5 and two.
//! Measured, not read from Chrome's source.
//!
//! `chrome-at-1.txt`, `chrome-at-1.5.txt` and `chrome-at-2.txt` beside the
//! case are Chrome's boxes, and `chrome.png`, `chrome-at-1.5.png` and
//! `chrome-at-2.png` its screenshots of the same bytes, 320 by 832 CSS
//! pixels, headless, started with `--force-device-scale-factor=1`, `=1.5`
//! and `=2`, without `--disable-gpu`, taken 2026-10-14, every element set in
//! the corpus's `DejaVu Sans`. The text is clear, so the pictures are the
//! boxes and their line boxes alone. `render-at-1.5.png` and
//! `render-at-2.png` are this engine's pictures at those scales, beside
//! `render.png` and never in its place (ADR 0057 § 5).

use std::collections::BTreeMap;
use std::rc::Rc;

use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_dom::{Document, NodeId};
use alo_paint::Canvas;
use alo_renderer::Drawing;

const NAME: &str = "centred-and-middle-on-the-grid";

/// The three scales the case is laid out, drawn and compared at, and the
/// names of Chrome's boxes and picture at each.
const SCALES: [(f32, &str, &str); 3] = [
    (1.0, "chrome-at-1.txt", "chrome.png"),
    (1.5, "chrome-at-1.5.txt", "chrome-at-1.5.png"),
    (2.0, "chrome-at-2.txt", "chrome-at-2.png"),
];

/// How many elements of the case Chrome's files hold, past its head.
const ELEMENTS: usize = 118;

/// How far from Chrome's a number may be, in sixty-fourths of a device
/// pixel: a twentieth of one, which no number at 1.5 a float's error from
/// Chrome's whole sixty-fourths comes near, and a half the old rules left
/// off the grid is ten times.
const SLACK: f32 = 0.05;

/// An element's border box in sixty-fourths of a CSS pixel: left, top,
/// width, height.
type Units = [f32; 4];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join(NAME))
}

fn drawn(case: &Case, scale: f32) -> Option<Rc<Drawing>> {
    Rendering::at_scale(case, scale)
        .ok()
        .and_then(|rendering| rendering.drawing())
}

/// Every element's border box by its path from the root (`/html0/body1/div0`
/// is the first element child of the second element child of the first),
/// the path Chrome's files name.
fn boxes_by_path(document: &Document, drawing: &Drawing) -> BTreeMap<String, Units> {
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
    let mut found = BTreeMap::new();
    for (path, node) in paths {
        let Some(geometry) = first.get(&node).and_then(|id| drawing.layout.get(*id)) else {
            continue;
        };
        let rect = geometry.border_box;
        found.insert(
            path,
            [
                rect.origin.x * 64.0,
                rect.origin.y * 64.0,
                rect.size.width * 64.0,
                rect.size.height * 64.0,
            ],
        );
    }
    found
}

fn walk(document: &Document, node: NodeId, path: &str, out: &mut Vec<(String, NodeId)>) {
    let mut index = 0;
    for child in document.children(node) {
        if let Some(element) = document.element(child) {
            let here = format!("{path}/{}{index}", element.name.local);
            index += 1;
            walk(document, child, &here, out);
            out.push((here, child));
        }
    }
}

/// Chrome's boxes in the file `name` beside the case, past its head, or
/// [`None`] when a line is not a path and four numbers.
fn chrome_boxes(case: &Case, name: &str) -> Option<BTreeMap<String, Units>> {
    let text = std::fs::read_to_string(case.expectation(name)).ok()?;
    let mut found = BTreeMap::new();
    for line in text.lines().filter(|line| !line.starts_with('#')) {
        let mut words = line.split_whitespace();
        let path = words.next()?;
        let numbers: Vec<f32> = words.map(str::parse).collect::<Result<_, _>>().ok()?;
        let [left, top, width, height] = numbers.as_slice() else {
            return None;
        };
        if !path.starts_with("/html0/head") {
            found.insert(path.to_owned(), [*left, *top, *width, *height]);
        }
    }
    Some(found)
}

/// Every element of the case is where Chrome puts it and as big, at one,
/// 1.5 and two: each centred line's pieces, each `middle` box, and each line
/// box whose height a `middle` box decides.
#[test]
fn every_box_is_where_chrome_puts_it_at_one_one_and_a_half_and_two() {
    let Some(case) = case() else {
        panic!("{NAME} reads");
    };
    for (scale, boxes, _) in SCALES {
        let Some(chrome) = chrome_boxes(&case, boxes) else {
            panic!("{boxes} is beside {NAME}");
        };
        assert_eq!(chrome.len(), ELEMENTS, "{boxes}");
        let Ok(rendering) = Rendering::at_scale(&case, scale) else {
            panic!("{NAME} renders at {scale}");
        };
        let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
            panic!("{NAME} is drawn at {scale}");
        };
        let ours = boxes_by_path(document, &drawing);
        let wrong: Vec<String> = chrome
            .iter()
            .filter_map(|(path, theirs)| {
                let mine = ours.get(path);
                let same = mine.is_some_and(|mine| {
                    mine.iter()
                        .zip(theirs)
                        .all(|(a, b)| ((a - b) * scale).abs() <= SLACK)
                });
                (!same).then(|| format!("{path}: Chrome {theirs:?}, this engine {mine:?}"))
            })
            .collect();
        assert!(wrong.is_empty(), "at {scale}:\n{}", wrong.join("\n"));
    }
}

/// The case's plain colours: the page, a line box, a centred box and a
/// `middle` box.
const PLAIN: [(u8, u8, u8); 4] = [
    (255, 255, 255),
    (228, 232, 240),
    (32, 80, 208),
    (208, 48, 32),
];

/// Which plain colour a pixel is, if it is one.
fn plain_of(canvas: &Canvas, x: u32, y: u32) -> Option<usize> {
    let (red, green, blue, _) = canvas.at(x, y)?.to_rgba8();
    PLAIN.iter().position(|&(r, g, b)| {
        red.abs_diff(r) <= 1 && green.abs_diff(g) <= 1 && blue.abs_diff(b) <= 1
    })
}

/// The pixels where the two pictures are each a plain colour and not the
/// same one, and how many pixels are a blend in either.
fn compare(ours: &Canvas, chrome: &Canvas) -> (Vec<(u32, u32)>, usize) {
    let mut differing = Vec::new();
    let mut blends = 0_usize;
    for y in 0..ours.height() {
        for x in 0..ours.width() {
            match (plain_of(ours, x, y), plain_of(chrome, x, y)) {
                (Some(a), Some(b)) if a != b => differing.push((x, y)),
                (Some(_), Some(_)) => {}
                _ => blends += 1,
            }
        }
    }
    (differing, blends)
}

/// Wherever both pictures are a plain colour it is the same one, at one,
/// 1.5 and two, and there are no blends at all: every box and line box here
/// is on whole device pixels in both once it is snapped.
#[test]
fn every_pixel_is_chromes_at_one_one_and_a_half_and_two() {
    let Some(case) = case() else {
        panic!("{NAME} reads");
    };
    for (scale, _, picture) in SCALES {
        let (Some(drawing), Some(chrome)) = (
            drawn(&case, scale),
            std::fs::read(case.expectation(picture))
                .ok()
                .and_then(|bytes| alo_paint::from_png(&bytes).ok()),
        ) else {
            panic!("{NAME} is drawn at {scale}, and Chrome's {picture} is beside it");
        };
        assert_eq!(
            (drawing.canvas.width(), drawing.canvas.height()),
            (chrome.width(), chrome.height()),
            "at {scale}"
        );
        let (differing, blends) = compare(&drawing.canvas, &chrome);
        assert!(
            differing.is_empty(),
            "at {scale}, {} plain pixels differ, first {:?}",
            differing.len(),
            differing.first()
        );
        assert_eq!(blends, 0, "at {scale}");
    }
}

#[test]
fn the_pictures_at_one_and_a_half_and_two_are_their_references() {
    let Some(case) = case() else {
        panic!("{NAME} reads");
    };
    for (scale, reference) in [(1.5, "render-at-1.5.png"), (2.0, "render-at-2.png")] {
        let Some(drawing) = drawn(&case, scale) else {
            panic!("{NAME} is drawn at {scale}");
        };
        let difference = check_picture(&case, reference, &drawing.canvas);
        assert!(difference.is_none(), "{difference:?}");
    }
}
