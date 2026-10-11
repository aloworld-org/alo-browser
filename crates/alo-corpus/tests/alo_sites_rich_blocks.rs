/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Sites' rich blocks, in numbers: the page that opened queue item 410.
//!
//! `cases/alo-sites-rich-blocks` pins the whole page as files. This says out
//! loud what item 410 is closed by. The page has no style sheet, so its code
//! blocks are `<pre>` as the user-agent sheet draws them, `white-space: pre`.
//! Until item 410 a run of spaces at the start of a line took no room
//! whatever `white-space` said, so the Rust block's `println!` stood flush
//! with the `fn` above it, its four spaces of indentation gone.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_paint::DisplayItem;

/// The case, read and rendered.
fn rich_blocks() -> Option<Rendering> {
    let case = Case::read(&cases_directory().join("alo-sites-rich-blocks"))?;
    Rendering::of(&case).ok()
}

/// Where the pen starts for each run drawn, as `(text, x, baseline)`.
fn runs(rendering: &Rendering) -> Vec<(String, f32, f32)> {
    let Some(drawing) = rendering.drawing() else {
        return Vec::new();
    };
    drawing
        .display
        .items()
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Text { text, origin, .. } => Some((text.clone(), origin.0, origin.1)),
            _ => None,
        })
        .collect()
}

/// Equal to within a thousandth of a pixel.
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

/// Four spaces of `DejaVu Sans` at 16 px: its space is 651/2048 of an em.
const INDENT: f32 = 4.0 * 651.0 / 2048.0 * 16.0;

#[test]
fn a_line_of_code_keeps_its_indentation() {
    let Some(rendering) = rich_blocks() else {
        panic!("the case renders");
    };
    let runs = runs(&rendering);
    let at = |wanted: &str| {
        runs.iter()
            .find(|(text, ..)| text == wanted)
            .map(|(_, x, y)| (*x, *y))
    };
    let (Some(open), Some(body), Some(close)) =
        (at("fn main() {"), at("println!(\"alo & sites\");"), at("}"))
    else {
        panic!("each line of the Rust block is drawn: {runs:?}");
    };
    // The body's 8 pixels of margin, and the block's own none.
    assert!(near(open.0, 8.0), "{open:?}");
    assert!(near(close.0, 8.0), "{close:?}");
    assert!(
        near(body.0, 8.0 + INDENT),
        "println! stands four spaces in, at {}: {body:?}",
        8.0 + INDENT,
    );
    // Each on its own line, 18.625 apart: three lines, not one.
    assert!(near(body.1 - open.1, 18.625) && near(close.1 - body.1, 18.625));

    // A line with no indentation is not moved: the TypeScript block's two
    // lines both start at the body's edge.
    for line in ["const greeting = \"<hello>\";", "console.log(greeting);"] {
        let Some((x, _)) = at(line) else {
            panic!("{line} is drawn: {runs:?}");
        };
        assert!(near(x, 8.0), "{line} at {x}");
    }
}

#[test]
fn the_indentation_is_room_the_block_is_measured_with() {
    let Some(rendering) = rich_blocks() else {
        panic!("the case renders");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case is drawn");
    };
    // The Rust block's text is as wide as its widest line, and that line is
    // the indented one: four spaces, then `println!("alo & sites");`, whose
    // letters alone are 175.10938 wide. Without the spaces the box was that.
    let text = drawing
        .boxes
        .ids()
        .filter(|id| {
            drawing.boxes.get(*id).is_some_and(|node| {
                matches!(&node.kind, alo_box::BoxKind::Text { text, .. } if text.starts_with("fn main"))
            })
        })
        .find_map(|id| drawing.layout.get(id).map(|layout| layout.border_box));
    let Some(text) = text else {
        panic!("the Rust block's text has a box");
    };
    assert!(near(text.origin.x, 8.0), "{text:?}");
    assert!(near(text.size.width, INDENT + 175.109_38), "{text:?}");
    assert!(near(text.size.height, 3.0 * 18.625), "{text:?}");
}
