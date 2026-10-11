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
//!
//! It also says what item 412 is closed by. The user-agent sheet gives
//! `pre` and `code` `font-family: monospace`, and until item 412 the corpus
//! mapped no face to that generic, so all three blocks were drawn in
//! `DejaVu Sans` and an issue said so. They are drawn in `DejaVu Sans Mono`,
//! where every character is one advance wide.

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

/// One character of `DejaVu Sans Mono` at 16 px: every advance in the face
/// is 1233/2048 of an em.
const ADVANCE: f32 = 1233.0 / 2048.0 * 16.0;

/// Four spaces of it.
const INDENT: f32 = 4.0 * ADVANCE;

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
    // 24 letters alone are 24 advances wide. Without the spaces the box was
    // that.
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
    assert!(near(text.size.width, INDENT + 24.0 * ADVANCE), "{text:?}");
    assert!(near(text.size.height, 3.0 * 18.625), "{text:?}");
}

#[test]
fn code_is_drawn_in_the_monospace_face() {
    let Some(rendering) = rich_blocks() else {
        panic!("the case renders");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case is drawn");
    };
    let drawn: Vec<(&str, &str, f32)> = drawing
        .display
        .items()
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Text {
                text, font, size, ..
            } => Some((text.as_str(), font.family(), *size)),
            _ => None,
        })
        .collect();
    let face = |wanted: &str| {
        drawn
            .iter()
            .find(|(text, ..)| *text == wanted)
            .map(|(_, family, size)| (*family, *size))
    };
    // Every line of the two `<pre>` blocks and the equation's `<code>`, at
    // the 16 px they inherit: the generic's size is not changed (item 415).
    for line in [
        "const greeting = \"<hello>\";",
        "console.log(greeting);",
        "fn main() {",
        "println!(\"alo & sites\");",
        "}",
        "E = mc^2 & x < y",
    ] {
        assert_eq!(
            face(line),
            Some(("DejaVu Sans Mono", 16.0)),
            "{line}: {drawn:?}"
        );
    }
    // The caption is not code, and keeps the sans-serif face.
    assert_eq!(
        face("Ideas, captured & shared"),
        Some(("DejaVu Sans", 16.0)),
        "{drawn:?}"
    );
    // And each line is as many advances wide as it has characters: the
    // TypeScript block's first, 27 of them, is its widest.
    let widths: Vec<f32> = drawing
        .boxes
        .ids()
        .filter(|id| {
            drawing.boxes.get(*id).is_some_and(|node| {
                matches!(&node.kind, alo_box::BoxKind::Text { text, .. }
                    if text.starts_with("const") || text.starts_with("E = "))
            })
        })
        .filter_map(|id| Some(drawing.layout.get(id)?.border_box.size.width))
        .collect();
    let [typescript, equation] = widths.as_slice() else {
        panic!("two text boxes: {widths:?}");
    };
    assert!(near(*typescript, 27.0 * ADVANCE), "{typescript}");
    assert!(near(*equation, 16.0 * ADVANCE), "{equation}");
}
