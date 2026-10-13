/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page told the scale of the screen it is shown on: the case that closes
//! queue item 567 (ADR 0057 § 3).
//!
//! `cases/a-page-told-its-scale` reads `devicePixelRatio` and puts a
//! floating box on whole device pixels with it, as floating-ui's
//! `roundByDPR` does in alo's workspace bundle; it writes the ratio into its
//! first paragraph; and its sheet draws a card's edge as a hairline under
//! `(min-resolution: 2dppx)`, shows a band under `(min-resolution: 192dpi)`
//! and greys the card under `(max-resolution: 1x)`.
//!
//! `chrome-at-1.txt` and `chrome-at-2.txt` beside it are Chrome 154's boxes
//! for every element, headless, started with `--force-device-scale-factor`
//! at one and at two (never the emulated scale of Chrome's developer tools),
//! each element set in the corpus's `DejaVu` face for its computed family,
//! measured 2026-10-13. `chrome-at-2.png` is Chrome's picture of it at two,
//! from the same run. Every box this engine lays out is Chrome's, at both
//! scales, to the 1/128 px grid; and at two the hairline, the band and the
//! floating box's first column and row are on Chrome's device pixels.
//!
//! `render-at-2.png` is this engine's picture at two, beside `render.png`
//! and never in its place (ADR 0057 § 5).

use std::collections::BTreeMap;

use alo_corpus::{Case, Rendering, cases_directory, check_picture};
use alo_dom::{Document, NodeId};
use alo_paint::Canvas;
use alo_renderer::Drawing;

/// An element's border box in sixty-fourths of a CSS pixel: left, top,
/// width, height.
type Units = [f32; 4];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("a-page-told-its-scale"))
}

/// The case, loaded by a renderer told `scale` with the page: its document
/// and its last drawing.
fn loaded(case: &Case, scale: f32) -> Result<(Document, std::rc::Rc<Drawing>), String> {
    let rendering = Rendering::at_scale(case, scale)?;
    if !matches!(rendering, Rendering::Loaded(..)) {
        return Err("a page whose script runs is loaded, and this one was not".to_owned());
    }
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
/// Chrome files name (`/html0/body1/main0` is the first element child of
/// the second element child of the first).
fn ours(document: &Document, drawing: &Drawing) -> BTreeMap<String, Units> {
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
        .filter_map(|(path, node)| {
            let border = drawing.layout.get(*first.get(&node)?)?.border_box;
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

/// The paragraph's text, as the page's script left it.
fn ratio_text(document: &Document) -> Vec<String> {
    document
        .descendants(document.root())
        .filter_map(|id| document.get(id)?.text().map(ToOwned::to_owned))
        .filter(|text| text.starts_with("Shown at") || text.starts_with("Not told"))
        .collect()
}

#[test]
fn the_page_reads_the_scale_it_is_laid_out_at() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for (scale, said) in [(1.0, "Shown at 1x"), (2.0, "Shown at 2x")] {
        match loaded(&case, scale) {
            Ok((document, _)) => assert_eq!(ratio_text(&document), [said]),
            Err(why) => panic!("at {scale}: {why}"),
        }
    }
}

#[test]
fn every_box_is_where_chrome_puts_it_at_one_and_at_two() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    for scale in [1_u8, 2] {
        let (document, drawing) = match loaded(&case, f32::from(scale)) {
            Ok(both) => both,
            Err(why) => panic!("at {scale}: {why}"),
        };
        let ours = ours(&document, &drawing);
        let Some(chrome) = chrome(&case, scale) else {
            panic!("Chrome's boxes at {scale} are beside the case");
        };
        let mut compared = 0;
        for (path, theirs) in &chrome {
            if path.starts_with("/html0/head") || path.ends_with("script4") {
                continue;
            }
            // `display: none` has no box here; Chrome reports it as nothing.
            let Some(mine) = ours.get(path) else {
                assert!(
                    theirs.iter().all(|unit| unit.abs() < 0.25),
                    "{path} at {scale} has no box here and Chrome's is {theirs:?}"
                );
                continue;
            };
            assert!(
                mine.iter().zip(theirs).all(|(a, b)| (a - b).abs() < 0.25),
                "{path} at {scale}: Chrome {theirs:?}, this engine {mine:?}"
            );
            compared += 1;
        }
        // At one the band is not shown, so it is not compared.
        assert_eq!(compared, if scale == 1 { 6 } else { 7 }, "at {scale}");
    }
}

/// A pixel's colour, without its alpha.
fn rgb(canvas: &Canvas, x: u32, y: u32) -> Option<(u8, u8, u8)> {
    let (red, green, blue, _) = canvas.at(x, y)?.to_rgba8();
    Some((red, green, blue))
}

#[test]
fn at_two_the_hairline_the_band_and_the_floating_box_are_on_chromes_device_pixels() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let Some(chrome) = std::fs::read(case.expectation("chrome-at-2.png"))
        .ok()
        .and_then(|bytes| alo_paint::from_png(&bytes).ok())
    else {
        panic!("Chrome's picture at two is beside the case");
    };
    let drawing = match loaded(&case, 2.0) {
        Ok((_, drawing)) => drawing,
        Err(why) => panic!("{why}"),
    };
    let ours = &drawing.canvas;
    assert_eq!((ours.width(), ours.height()), (320, 160));
    let edge = Some((113, 113, 122));
    let white = Some((255, 255, 255));
    let green = Some((22, 163, 74));
    let blue = Some((29, 78, 216));
    for (x, y, colour) in [
        // The card's top edge: one device pixel, half a CSS pixel.
        (200, 45, white),
        (200, 46, edge),
        (200, 47, white),
        // Its left edge.
        (16, 60, edge),
        (17, 60, white),
        // Its bottom edge, then the band from 44 to 50 CSS pixels.
        (200, 87, edge),
        (200, 88, green),
        (200, 99, green),
        (200, 100, white),
        // The floating box from (60.5, 30.5): device pixel (121, 61).
        (120, 80, white),
        (121, 80, blue),
        (150, 60, white),
        (150, 61, blue),
    ] {
        assert_eq!(rgb(&chrome, x, y), colour, "Chrome's at ({x}, {y})");
        assert_eq!(rgb(ours, x, y), colour, "ours at ({x}, {y})");
    }
}

#[test]
fn the_picture_at_two_is_its_reference() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let drawing = match loaded(&case, 2.0) {
        Ok((_, drawing)) => drawing,
        Err(why) => panic!("{why}"),
    };
    let difference = check_picture(&case, "render-at-2.png", &drawing.canvas);
    assert!(difference.is_none(), "{difference:?}");
}
