/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Text laid out on the grid of 1/64 of a device pixel: the case that
//! closes queue item 574 (ADR 0054 § 4, `alo-layout`'s `text_run.rs`).
//!
//! `cases/text-on-the-grid` puts text items at tenths of a pixel: a padded
//! `<span>` holding one `&nbsp;`, one holding two, one holding `x&nbsp;x`,
//! a line split into two items by an empty `<b>`, the same letters as one
//! item with a space, `i,.m ima W.xai` (91.8125 px as one item, a unit
//! more rounded a word at a time, at one and at two), two padded spans
//! with a no-break space between them,
//! and a wrapped paragraph with a span on its second line. Chrome 154 rounds
//! each item's advance up onto the grid and starts what follows there, so
//! the `<span>` after `x<b></b>xx` stands at `ceil(x) + ceil(xx)` and the
//! one after `x xx` at `ceil(x xx)`; a word at a time, or the whole line at
//! once, is a unit off somewhere here.
//!
//! The numbers are Chrome 154's `getBoundingClientRect()` of every element,
//! in 1/128 px, headless, started with `--force-device-scale-factor=1` and
//! `=2` (never the emulated scale of Chrome's developer tools), the page set
//! in the corpus's `DejaVu Sans`, measured 2026-10-14, the source not read.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_dom::{Document, NodeId};

/// Chrome's border box of each element at one and at two, in 1/128 px:
/// left, top, width, height, by its path from the root.
const CHROME: [(&str, [i32; 4], [i32; 4]); 20] = [
    (
        "/html0/body1/div0/span0",
        [1324, 1330, 5726, 1920],
        [1324, 1331, 5725, 1920],
    ),
    (
        "/html0/body1/div0",
        [1324, 1330, 5726, 1920],
        [1324, 1331, 5725, 1920],
    ),
    (
        "/html0/body1/div1/span0",
        [1324, 3890, 6254, 1920],
        [1324, 3891, 6254, 1920],
    ),
    (
        "/html0/body1/div1/span1",
        [7578, 3890, 986, 1920],
        [7578, 3891, 985, 1920],
    ),
    (
        "/html0/body1/div1",
        [1324, 3890, 7240, 1920],
        [1324, 3891, 7239, 1920],
    ),
    (
        "/html0/body1/div2/span0",
        [1324, 6450, 7696, 1920],
        [1324, 6451, 7695, 1920],
    ),
    (
        "/html0/body1/div2/span1",
        [9020, 6450, 986, 1920],
        [9019, 6451, 985, 1920],
    ),
    (
        "/html0/body1/div2",
        [1324, 6450, 8682, 1920],
        [1324, 6451, 8680, 1920],
    ),
    (
        "/html0/body1/div3/b0",
        [2310, 9010, 0, 1920],
        [2309, 9011, 0, 1920],
    ),
    (
        "/html0/body1/div3/span1",
        [4280, 9010, 986, 1920],
        [4279, 9011, 985, 1920],
    ),
    (
        "/html0/body1/div3",
        [1324, 9010, 3942, 1920],
        [1324, 9011, 3940, 1920],
    ),
    (
        "/html0/body1/div4/span0",
        [4808, 11570, 986, 1920],
        [4808, 11571, 985, 1920],
    ),
    (
        "/html0/body1/div4",
        [1324, 11570, 4470, 1920],
        [1324, 11571, 4469, 1920],
    ),
    (
        "/html0/body1/div5/span0",
        [13076, 16690, 986, 1920],
        [13076, 16691, 985, 1920],
    ),
    (
        "/html0/body1/div5",
        [1324, 16690, 12738, 1920],
        [1324, 16691, 12737, 1920],
    ),
    (
        "/html0/body1/div6/span0",
        [1324, 14130, 6122, 1920],
        [1324, 14131, 6121, 1920],
    ),
    (
        "/html0/body1/div6/span1",
        [7976, 14130, 6122, 1920],
        [7974, 14131, 6121, 1920],
    ),
    (
        "/html0/body1/div6",
        [1324, 14130, 12774, 1920],
        [1324, 14131, 12771, 1920],
    ),
    (
        "/html0/body1/p7/span0",
        [23074, 3250, 986, 1920],
        [23074, 3251, 985, 1920],
    ),
    (
        "/html0/body1/p7",
        [20504, 1330, 7680, 3840],
        [20505, 1331, 7680, 3840],
    ),
];

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("text-on-the-grid"))
}

/// Every element below `node`, with its path from the root (`/html0/body1`
/// is the second element child of the first), the path Chrome's are named
/// by.
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

/// Each element's first box's border box in 1/128 px, by path.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a box on a 240 × 140 page, in 1/128 px, is far inside an i32"
)]
fn boxes(rendering: &Rendering) -> Vec<(String, [i32; 4])> {
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    walk(document, document.root(), "", &mut paths);
    let mut found = Vec::new();
    for (path, node) in paths {
        let first = drawing.boxes.ids().find(|id| {
            matches!(
                drawing.boxes.get(*id).map(|box_node| &box_node.kind),
                Some(alo_box::BoxKind::Element { node: of, .. }) if *of == node
            )
        });
        let Some(rect) = first
            .and_then(|id| drawing.layout.get(id))
            .map(|at| at.border_box)
        else {
            continue;
        };
        let units = [rect.left(), rect.top(), rect.size.width, rect.size.height]
            .map(|value| (value * 128.0).round() as i32);
        found.push((path, units));
    }
    found
}

/// Where this engine and Chrome differ at `scale`, one line each.
fn differences(scale: f32) -> Vec<String> {
    let Some(case) = case() else {
        return vec!["the case is missing".to_owned()];
    };
    let rendering = match Rendering::at_scale(&case, scale) {
        Ok(rendering) => rendering,
        Err(why) => return vec![why],
    };
    let ours = boxes(&rendering);
    let mut wrong = Vec::new();
    for (path, at_one, at_two) in CHROME {
        let chrome = if scale > 1.0 { at_two } else { at_one };
        let found = ours
            .iter()
            .find(|(at, _)| at == path)
            .map(|(_, units)| *units);
        if found != Some(chrome) {
            wrong.push(format!("{path} at {scale}: {found:?}, Chrome {chrome:?}"));
        }
    }
    wrong
}

#[test]
fn every_box_is_where_chrome_puts_it_at_one() {
    let wrong = differences(1.0);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn every_box_is_where_chrome_puts_it_at_two() {
    let wrong = differences(2.0);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
