/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `line-height` a stranger wrote, rendered with real fonts (queue item
//! 316).
//!
//! `line-height` is read from a page's stylesheet and now sets how tall every
//! line is, so a page can hand layout any number it likes. One that cannot
//! be used — negative, infinite, not a number — is `normal`. One that is
//! merely enormous is a line that tall, as an enormous `height` is a box that
//! tall, and either way the page is drawn rather than the renderer falling
//! over.

use alo_corpus::{corpus_fonts, render};
use alo_layout::Size;
use alo_paint::DisplayItem;

/// The page, set in `value`: a paragraph that wraps, with a bordered span in
/// it, and a paragraph after it.
fn render_at(value: &str) -> alo_corpus::Rendered {
    let css = format!(
        "div {{ width: 40px; line-height: {value}; background: red }} \
         span {{ border: 1px solid green }}"
    );
    let html = "<body><div><span>ab cd ef</span> gh ij</div><p>after</p></body>";
    render(html, &css, Size::new(200.0, 100.0), &corpus_fonts())
}

/// Where the paragraph after the block starts, from its text.
fn after_top(rendered: &alo_corpus::Rendered) -> Option<f32> {
    rendered
        .drawing
        .display
        .items()
        .iter()
        .find_map(|item| match item {
            DisplayItem::Text { text, origin, .. } if text == "after" => Some(origin.1),
            _ => None,
        })
}

#[test]
fn a_line_height_that_cannot_be_used_is_normal() {
    let normal = render_at("normal");
    let Some(expected) = after_top(&normal) else {
        panic!("the paragraph after is drawn");
    };
    for value in [
        "-4px",
        "-2",
        "1e39px",
        "1e39",
        "NaN",
        "inf",
        "calc(1px / 0)",
        "2 3",
        "twelve",
    ] {
        let rendered = render_at(value);
        assert_eq!(after_top(&rendered), Some(expected), "{value}");
    }
}

#[test]
fn an_enormous_line_height_is_drawn_without_falling_over() {
    for value in ["3e38px", "1e37px", "1e30", "1e38%", "0", "0.0001px"] {
        let rendered = render_at(value);
        assert!(after_top(&rendered).is_some(), "{value}: the page is drawn");
    }
}
