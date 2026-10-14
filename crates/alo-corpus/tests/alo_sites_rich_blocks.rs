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
//!
//! And what item 415 is closed by (ADR 0045 § 7). `font-family: monospace`
//! alone has a `medium` of 13 px, and the code inherits the body's `medium`,
//! so it is read again at 13 — as every browser draws this page — and so is
//! every `em` of the `<pre>`s' margins.
//!
//! And what item 413 is closed by (ADR 0046 § 7). The figure's `<img>` is a
//! `data:` URL of 24 bytes that are no picture, so it is its `alt`, "Orange
//! notebook <front>", drawn as text at the start of the figure's first line,
//! and an agent reads it as a broken image with that name.

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

/// A text item's width as a box: its advances rounded up onto the grid of
/// 1/64 px, as Chrome rounds one (ADR 0054 § 4, item 574).
fn on_the_grid(width: f32) -> f32 {
    alo_layout::layout_unit::rounded_up(width, 1.0)
}

/// The size code is drawn at: `monospace`'s own `medium` (ADR 0045 § 3).
const CODE: f32 = 13.0;

/// One character of `DejaVu Sans Mono` at 13 px: every advance in the face
/// is 1233/2048 of an em.
const ADVANCE: f32 = 1233.0 / 2048.0 * CODE;

/// One line of it: `DejaVu Sans Mono`'s ascent and descent are 1901 and 483
/// of 2048, as `DejaVu Sans`' are, and a line takes each in whole pixels
/// (ADR 0050 § 1): at 13 px 12.07 and 3.07, so 12 and 3.
const LINE: f32 = 15.0;

/// `DejaVu Sans`' `normal` line at 16 px: 14.85 and 3.77, so 15 and 4.
const SANS_LINE: f32 = 19.0;

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
    // Each on its own line, one 13 px line apart: three lines, not one.
    assert!(near(body.1 - open.1, LINE) && near(close.1 - body.1, LINE));

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
    assert!(
        near(text.size.width, on_the_grid(INDENT + 24.0 * ADVANCE)),
        "{text:?}"
    );
    assert!(near(text.size.height, 3.0 * LINE), "{text:?}");
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
    // 13 px: the body's `medium`, read again in `monospace` (item 415).
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
            Some(("DejaVu Sans Mono", CODE)),
            "{line}: {drawn:?}"
        );
    }
    // The caption is not code, and keeps the sans-serif face and 16 px.
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
    assert!(
        near(*typescript, on_the_grid(27.0 * ADVANCE)),
        "{typescript}"
    );
    assert!(near(*equation, on_the_grid(16.0 * ADVANCE)), "{equation}");
}

#[test]
fn a_pres_margins_are_its_own_ems() {
    let Some(rendering) = rich_blocks() else {
        panic!("the case renders");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case is drawn");
    };
    // The block boxes of the two `<pre>`s and the equation, top to bottom,
    // as (top, height).
    let blocks: Vec<(f32, f32)> = drawing
        .boxes
        .ids()
        .filter(|id| {
            drawing.boxes.get(*id).is_some_and(|node| {
                matches!(&node.kind, alo_box::BoxKind::Text { text, .. }
                    if text.starts_with("const") || text.starts_with("fn ") || text.starts_with("E = "))
            })
        })
        .filter_map(|id| {
            // The text's block: its parent's parent, the `<code>` between.
            let code = drawing.boxes.get(id)?.parent?;
            let block = drawing.boxes.get(code)?.parent?;
            let border = drawing.layout.get(block)?.border_box;
            Some((border.origin.y, border.size.height))
        })
        .collect();
    let [typescript, rust, equation] = blocks.as_slice() else {
        panic!("three blocks: {blocks:?}");
    };
    // Two lines, then three, of 13 px code.
    assert!(near(typescript.1, 2.0 * LINE), "{typescript:?}");
    assert!(near(rust.1, 3.0 * LINE), "{rust:?}");
    // The first `pre`'s 13 px top margin collapses into the figure's 16:
    // the figure, two lines of 16 px from 16, ends at 54.
    assert!(
        near(typescript.0, 16.0 + 2.0 * SANS_LINE + 16.0),
        "{typescript:?}"
    );
    // Between the two, each one's 1em margin is 13 and they collapse to 13.
    assert!(
        near(rust.0 - (typescript.0 + typescript.1), CODE),
        "{typescript:?} {rust:?}"
    );
    // And the second's bottom margin is 13 above the equation, which has
    // none of its own.
    assert!(
        near(equation.0 - (rust.0 + rust.1), CODE),
        "{rust:?} {equation:?}"
    );
}

/// "Orange notebook <front>" in `DejaVu Sans` at 16 px, as HarfBuzz's own
/// `hb-shape` shapes it: 26 779 font units of 2048, kerning included.
const ALT: f32 = 26_779.0 / 2048.0 * 16.0;

/// `DejaVu Sans`' ascent, 1901 of 2048, at the figure's 16 px: 14.85, which
/// a line takes as 15 (ADR 0050 § 1).
const ASCENT: f32 = 15.0;

#[test]
fn the_broken_picture_is_its_alt_on_the_figures_first_line() {
    let Some(rendering) = rich_blocks() else {
        panic!("the case renders");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("the case is drawn");
    };
    let Some(document) = rendering.document() else {
        panic!("the case has a document");
    };
    let img = drawing.boxes.ids().find(|id| {
        matches!(
            drawing.boxes.get(*id).map(|node| &node.kind),
            Some(alo_box::BoxKind::Element { node, .. })
                if document.element(*node).is_some_and(|element| element.name.is_html("img"))
        )
    });
    let Some(img) = img else {
        panic!("the figure's img has a box");
    };
    let Some(border) = drawing.layout.get(img).map(|layout| layout.border_box) else {
        panic!("the img is laid out");
    };
    // The figure starts at (48, 16): the body's 8 collapsed into its 16, and
    // 40 in. Its first line is the alt and nothing else, one 16 px line.
    assert!(near(border.origin.x, 48.0), "{border:?}");
    assert!(near(border.origin.y, 16.0), "{border:?}");
    assert!(near(border.size.width, on_the_grid(ALT)), "{border:?}");
    assert!(near(border.size.height, SANS_LINE), "{border:?}");
    let runs = runs(&rendering);
    let Some((_, x, baseline)) = runs
        .iter()
        .find(|(text, ..)| text == "Orange notebook <front>")
        .cloned()
    else {
        panic!("the alt is drawn: {runs:?}");
    };
    assert!(near(x, 48.0), "{x}");
    assert!(near(baseline, 16.0 + ASCENT), "{baseline}");
    // The caption is on the line after it, where it was.
    assert!(
        runs.iter()
            .any(|(text, _, line)| text.starts_with("Ideas")
                && near(*line, 16.0 + SANS_LINE + ASCENT)),
        "{runs:?}"
    );
    let agent = alo_agent::AgentTree::new(document, &drawing.boxes, &drawing.layout).to_outline();
    assert!(
        agent.contains("image \"Orange notebook <front>\" [broken] at (48, 16)"),
        "{agent}"
    );
    assert_eq!(agent.matches("Orange notebook").count(), 1, "{agent}");
}
