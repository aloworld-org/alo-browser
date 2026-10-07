/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `<br>` where a stranger put it, rendered with real fonts (queue item
//! 319).
//!
//! A `<br>` now ends the line it is on, and a page can put one anywhere
//! markup lets it and as many as it likes. Wherever it lands — a flex item,
//! an inline-block, a button, inside an `<svg>`, a `<pre>` — the page is
//! drawn rather than the renderer falling over, and a page of nothing but
//! breaks is as many lines tall as it has breaks.

use alo_corpus::{corpus_fonts, render};
use alo_layout::Size;

fn render_page(html: &str, css: &str) -> alo_corpus::Rendered {
    render(html, css, Size::new(200.0, 100.0), &corpus_fonts())
}

/// The border box of the first `<div>`.
fn first_div(rendered: &alo_corpus::Rendered) -> Option<alo_layout::Rect> {
    let boxes = &rendered.drawing.boxes;
    let id = boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|node| &node.kind),
            Some(alo_box::BoxKind::Element { node, .. })
                if rendered
                    .document
                    .element(*node)
                    .is_some_and(|element| element.name.is_html("div"))
        )
    })?;
    Some(rendered.drawing.layout.get(id)?.border_box)
}

#[test]
fn a_page_of_nothing_but_breaks_is_that_many_lines_tall() {
    let count = 5000;
    let html = format!("<div>{}</div>", "<br>".repeat(count));
    let rendered = render_page(&html, "div { line-height: 10px }");
    let Some(div) = first_div(&rendered) else {
        panic!("the div is laid out");
    };
    // Every break is a line of its own, and nothing follows the last one.
    let tall = 10.0 * 5000.0;
    assert!(
        (div.size.height - tall).abs() < 0.5,
        "{} breaks are {} tall",
        count,
        div.size.height,
    );
}

#[test]
fn a_closing_br_is_a_break_too() {
    // HTML's parser reads `</br>` as `<br>`, so a page that writes it gets one.
    let open = render_page("<div>a<br>b</div>", "div { line-height: 10px }");
    let closed = render_page("<div>a</br>b</div>", "div { line-height: 10px }");
    let heights: Vec<Option<f32>> = [&open, &closed]
        .iter()
        .map(|rendered| first_div(rendered).map(|div| div.size.height))
        .collect();
    assert_eq!(heights, [Some(20.0), Some(20.0)]);
}

#[test]
fn a_break_anywhere_markup_puts_it_is_drawn_without_falling_over() {
    for (html, css) in [
        ("<div style=\"display: flex\">a<br>b</div>", ""),
        ("<div style=\"display: grid\"><br><br></div>", ""),
        (
            "<div><span style=\"display: inline-block\">a<br>b</span></div>",
            "",
        ),
        ("<div><button>Save<br>now</button></div>", ""),
        ("<div><svg><br></svg>after</div>", ""),
        ("<div><pre>a\n<br>\nb</pre></div>", ""),
        ("<div><a href=x>a<br><p>block</p><br>b</a></div>", ""),
        ("<div>a<br>b</div>", "br { display: contents }"),
        ("<div>a<br>b</div>", "br { display: block }"),
        (
            "<div>a<br>b</div>",
            "br { display: inline-block; width: 1e38px }",
        ),
        ("<div>a<br>b</div>", "br { line-height: 3e38px }"),
        (
            "<div><span style=\"padding: 0 1e38px\"><br></span></div>",
            "",
        ),
        ("<div style=\"width: 0\">a<br>b</div>", ""),
        ("<div style=\"white-space: nowrap\">a<br>b</div>", ""),
    ] {
        let rendered = render_page(html, css);
        assert!(
            first_div(&rendered).is_some(),
            "{html} with {css:?}: the page is laid out",
        );
    }
}
