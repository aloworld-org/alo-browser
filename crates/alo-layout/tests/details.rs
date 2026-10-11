/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `<details>`, closed and open — ADR 0048 § 6, in numbers.
//!
//! A `details` is its summary, laid out first wherever it is written, then
//! one content box holding the rest. Closed, the content box is there and
//! holds nothing, so it takes no room; a flex or grid `details` still has a
//! gap before it. Each case below was measured in Chrome 154, headless, with
//! the summary 20 tall: the summary's height and the `details`' own padding
//! and border are what decide the numbers, not a font.
//!
//! Text is measured with [`BlockFont`], eight pixels a character and sixteen
//! tall; every box that holds text here is given a height so that it does
//! not matter.

use alo_box::{BoxId, BoxTree, build};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::{Document, parse_document};
use alo_layout::{BlockFont, LayoutTree, Rect, Size, compute};
use alo_style::{Origin, SourcedSheet, StyleTree, USER_AGENT_STYLE_SHEET, resolve};

/// Equal to within far less than a pixel.
fn close(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.0001
}

/// A page laid out in an 800 × 600 viewport with no margin on its body.
struct Laid {
    document: Document,
    boxes: BoxTree,
    layout: LayoutTree,
}

impl Laid {
    fn of(body: &str, css: &str) -> Self {
        let document = parse_document(&format!("<body>{body}</body>"));
        let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let author = parse_stylesheet(&format!("body {{ margin: 0 }} {css}"));
        let sheets = [
            SourcedSheet::new(Origin::UserAgent, &agent),
            SourcedSheet::new(Origin::Author, &author),
        ];
        let styles: StyleTree = resolve(&document, &sheets, &MediaContext::default());
        let boxes = build(&document, &styles);
        let layout = compute(&boxes, &styles, Size::new(800.0, 600.0), &BlockFont);
        Self {
            document,
            boxes,
            layout,
        }
    }

    /// The border box of the box made for the element with this `id`, or
    /// [`None`] when it made none.
    fn rect(&self, wanted: &str) -> Option<Rect> {
        let node = self.document.descendants(self.document.root()).find(|id| {
            self.document
                .element(*id)
                .is_some_and(|element| element.attr("id") == Some(wanted))
        })?;
        let root = self.boxes.root()?;
        let found: BoxId = core::iter::once(root)
            .chain(self.boxes.descendants(root))
            .find(|id| self.boxes.get(*id).and_then(|held| held.kind.node()) == Some(node))?;
        self.layout.border_box(found)
    }

    /// How tall the box for `wanted` is, or NaN — which no assertion
    /// accepts — when it made none.
    fn height(&self, wanted: &str) -> f32 {
        self.rect(wanted).map_or(f32::NAN, |rect| rect.size.height)
    }

    /// Where the box for `wanted` starts down the page, or NaN.
    fn top(&self, wanted: &str) -> f32 {
        self.rect(wanted).map_or(f32::NAN, |rect| rect.origin.y)
    }
}

/// alo Sites' FAQ rules, with the summary and the answer each 20 tall.
const FAQ: &str = "details { border: 1px solid; padding: 12px 16px; margin-bottom: 12px } \
                   summary, p { height: 20px } \
                   details p { margin: 12px 0 0 } \
                   #after { height: 10px }";

#[test]
fn a_closed_details_is_its_summary_and_its_padding_and_border() {
    // Chrome 154, the frozen FAQ's rules: 1 + 12 + the summary + 12 + 1.
    let laid = Laid::of(
        "<details id=d><summary id=s>Q</summary><p id=a>A</p></details><div id=after></div>",
        FAQ,
    );
    assert!(close(laid.height("d"), 46.0), "{}", laid.height("d"));
    assert!(close(laid.top("s"), 13.0), "{}", laid.top("s"));
    assert_eq!(laid.rect("a"), None, "the answer makes no box");
    assert!(
        close(laid.top("after"), 58.0),
        "what follows is under it and its margin: {}",
        laid.top("after")
    );
}

#[test]
fn an_open_details_holds_its_answer_under_the_summary() {
    // Chrome 154: the answer 12 below the summary, and the `details` 12 + 20
    // taller than closed.
    let laid = Laid::of(
        "<details id=d open><summary id=s>Q</summary><p id=a>A</p></details><div id=after></div>",
        FAQ,
    );
    assert!(close(laid.height("d"), 78.0), "{}", laid.height("d"));
    assert!(close(laid.top("a"), 45.0), "{}", laid.top("a"));
    assert!(close(laid.top("after"), 90.0), "{}", laid.top("after"));
}

#[test]
fn an_open_details_lays_out_as_a_block_holding_the_same_children() {
    // The content box is a block its children's margins collapse through,
    // so an open `details` with no padding or border is laid out exactly as
    // a `div` holding the same summary and paragraphs — margins and all.
    let css = "summary, p { height: 20px } p { margin: 10px 0 } #after { height: 10px }";
    let details = Laid::of(
        "<details id=d open><summary id=s>Q</summary><p id=a>A</p><p id=b>B</p></details>\
         <div id=after></div>",
        css,
    );
    let div = Laid::of(
        "<div id=d><summary id=s>Q</summary><p id=a>A</p><p id=b>B</p></div>\
         <div id=after></div>",
        css,
    );
    for id in ["d", "s", "a", "b", "after"] {
        assert_eq!(details.rect(id), div.rect(id), "{id}");
    }
    assert!(close(details.height("d"), 80.0), "{}", details.height("d"));
}

#[test]
fn a_closed_flex_or_grid_details_keeps_the_gap_before_its_content() {
    // Chrome 154: `display: flex; flex-direction: column; gap: 10px`, and
    // `display: grid; row-gap: 10px`, are both 30 tall closed with a summary
    // 20 tall — the content box is an item of no size, after a gap.
    for css in [
        "details { display: flex; flex-direction: column; gap: 10px } summary { height: 20px }",
        "details { display: grid; row-gap: 10px } summary { height: 20px }",
    ] {
        let laid = Laid::of(
            "<details id=d><summary>Q</summary><p id=a>A</p></details>",
            css,
        );
        assert!(close(laid.height("d"), 30.0), "{css}: {}", laid.height("d"));
        assert_eq!(laid.rect("a"), None, "{css}");
    }
}

#[test]
fn a_summary_written_last_is_laid_out_first() {
    // Chrome 154: closed, the `details` is the summary's 20 with it at the
    // top; open, the paragraph written before it comes after it.
    let css = "summary, p { height: 20px } p { margin: 0 }";
    let closed = Laid::of(
        "<details id=d><p id=a>A</p><summary id=s>Q</summary></details>",
        css,
    );
    assert!(close(closed.height("d"), 20.0), "{}", closed.height("d"));
    assert!(close(closed.top("s"), 0.0), "{}", closed.top("s"));
    let open = Laid::of(
        "<details id=d open><p id=a>A</p><summary id=s>Q</summary></details>",
        css,
    );
    assert!(close(open.top("s"), 0.0), "{}", open.top("s"));
    assert!(close(open.top("a"), 20.0), "{}", open.top("a"));
}

#[test]
fn a_second_summary_is_an_ordinary_block_in_the_content() {
    // Chrome 154: a second `summary` in an open `details` is `display:
    // block`, under the first; closed, it is not laid out at all.
    let css = "summary { height: 20px }";
    let open = Laid::of(
        "<details id=d open><summary id=s>Q</summary><summary id=t>R</summary></details>",
        css,
    );
    assert!(close(open.top("t"), 20.0), "{}", open.top("t"));
    assert!(close(open.height("d"), 40.0), "{}", open.height("d"));
    let closed = Laid::of(
        "<details id=d><summary id=s>Q</summary><summary id=t>R</summary></details>",
        css,
    );
    assert_eq!(closed.rect("t"), None);
    assert!(close(closed.height("d"), 20.0), "{}", closed.height("d"));
}

#[test]
fn a_closed_details_with_no_summary_is_its_padding_and_border_and_says_why() {
    // Chrome 154 makes it a summary ("Details") and is as tall as with one;
    // that summary is queue item 433's, so here the slot is empty, the
    // `details` is 1 + 12 + 12 + 1, and the box tree records it.
    let laid = Laid::of("<details id=d><p id=a>A</p></details>", FAQ);
    assert!(close(laid.height("d"), 26.0), "{}", laid.height("d"));
    assert_eq!(laid.rect("a"), None);
    let issues: Vec<String> = laid
        .boxes
        .issues()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("<details> with no <summary>")),
        "{issues:?}"
    );
}
