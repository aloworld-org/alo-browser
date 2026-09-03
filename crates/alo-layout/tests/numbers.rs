/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Layout asserted in numbers.
//!
//! `CLAUDE.md`: *"A layout is a tree with numbers in it; assert on the
//! numbers, not on a screenshot somebody eyeballed."* This is that assertion,
//! and it is the whole tree rather than one rectangle — a change that moves a
//! box says which box and by how much, which an assertion on a single number
//! does not.
//!
//! Text is measured with a deliberately fake measurer: eight pixels a
//! character, sixteen tall. Item 6 brings a real one. Saying so here means the
//! numbers below are about the boxes and not about a font nobody has chosen.

use alo_box::{BoxId, BoxTree, build};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{BlockFont, LayoutTree, NoText, Rect, Size, compute};
use alo_style::{Origin, SourcedSheet, StyleTree, USER_AGENT_STYLE_SHEET, resolve};

/// Equal to within far less than a pixel. A layout assertion is about the
/// number, not about whether two floats happen to be bit-identical.
fn close(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.0001
}

fn lay_out(html: &str, css: &str, viewport: Size) -> (BoxTree, LayoutTree) {
    let document = parse_document(html);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    // Every test here is about where flex, grid and `calc` put a box, and the
    // user-agent sheet's `body { margin: 8px }` would move every one of them by
    // the same eight pixels — which would say nothing about flex and would hide
    // the number that does. So these tests start from a page with no margin,
    // deliberately and in one place. The margin itself is asserted in the
    // corpus, where it belongs, against a page that did not ask for it.
    let author = parse_stylesheet(&format!("body {{ margin: 0 }}\n{css}"));
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles: StyleTree = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build(&document, &styles);
    let layout = compute(&boxes, &styles, viewport, &BlockFont);
    (boxes, layout)
}

/// The rectangle of the first box whose element carries this `id`, or a
/// rectangle no layout produces so that the assertion which asked reports it.
fn rect_of(boxes: &BoxTree, layout: &LayoutTree, wanted: &str, document_html: &str) -> Rect {
    const NOT_FOUND: Rect = Rect {
        origin: alo_layout::Point {
            x: f32::NAN,
            y: f32::NAN,
        },
        size: Size {
            width: f32::NAN,
            height: f32::NAN,
        },
    };
    let document = parse_document(document_html);
    let Some(node) = document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.attr("id") == Some(wanted))
    }) else {
        return NOT_FOUND;
    };
    let Some(root) = boxes.root() else {
        return NOT_FOUND;
    };
    let found: Option<BoxId> = core::iter::once(root)
        .chain(boxes.descendants(root))
        .find(|id| boxes.get(*id).and_then(|held| held.kind.node()) == Some(node));
    found
        .and_then(|id| layout.border_box(id))
        .unwrap_or(NOT_FOUND)
}

#[test]
fn three_blocks_stack_and_fill_the_width() {
    let html = "<body><div id=a></div><div id=b></div><div id=c></div></body>";
    let css = "div { height: 20px }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));

    assert_eq!(
        rect_of(&boxes, &layout, "a", html),
        Rect::new(0.0, 0.0, 400.0, 20.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "b", html),
        Rect::new(0.0, 20.0, 400.0, 20.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "c", html),
        Rect::new(0.0, 40.0, 400.0, 20.0)
    );
}

#[test]
fn the_box_model_adds_up_the_way_css_says_it_does() {
    let html = "<body><div id=a></div></body>";
    let css = "div { width: 100px; height: 50px; padding: 10px; \
               border-top-width: 2px; border-right-width: 2px; \
               border-bottom-width: 2px; border-left-width: 2px; margin: 8px }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));

    // `content-box` is the initial value: `width` is the content, and padding
    // and border are added around it.
    let rect = rect_of(&boxes, &layout, "a", html);
    assert_eq!(rect, Rect::new(8.0, 8.0, 124.0, 74.0));

    let root = boxes.root().expect("a root");
    let geometry = core::iter::once(root)
        .chain(boxes.descendants(root))
        .find_map(|id| {
            let held = boxes.get(id)?;
            held.kind.node()?;
            let geometry = layout.get(id)?;
            (geometry.border_box == rect).then_some(geometry)
        })
        .expect("the div's geometry");
    assert_eq!(geometry.content_box(), Rect::new(20.0, 20.0, 100.0, 50.0));
    assert_eq!(geometry.padding_box(), Rect::new(10.0, 10.0, 120.0, 70.0));
}

#[test]
fn border_box_sizing_puts_the_padding_and_border_inside_the_width() {
    let html = "<body><div id=a></div></body>";
    let css = "div { box-sizing: border-box; width: 100px; height: 50px; \
               padding: 10px; border-left-width: 5px; border-right-width: 5px }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));
    assert_eq!(
        rect_of(&boxes, &layout, "a", html).size,
        Size::new(100.0, 50.0),
        "the hundred includes everything, which is why one writes border-box",
    );
}

#[test]
fn a_flex_row_shares_the_space_the_way_the_grow_factors_say() {
    let html = "<body><div id=row><div id=one></div><div id=two></div></div></body>";
    let css = "#row { display: flex } #one { flex-grow: 1 } #two { flex-grow: 3 } \
               #row > div { height: 30px }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));

    assert_eq!(
        rect_of(&boxes, &layout, "one", html),
        Rect::new(0.0, 0.0, 100.0, 30.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "two", html),
        Rect::new(100.0, 0.0, 300.0, 30.0)
    );
}

#[test]
fn a_gap_takes_room_out_of_what_is_shared() {
    let html = "<body><div id=row><div id=one></div><div id=two></div></div></body>";
    let css = "#row { display: flex; gap: 20px } #row > div { flex-grow: 1; height: 10px }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));

    assert!(close(
        rect_of(&boxes, &layout, "one", html).size.width,
        190.0
    ));
    assert_eq!(
        rect_of(&boxes, &layout, "two", html),
        Rect::new(210.0, 0.0, 190.0, 10.0)
    );
}

#[test]
fn a_grid_of_three_equal_columns_is_three_equal_columns() {
    let html = "<body><div id=grid><div id=a></div><div id=b></div><div id=c></div></div></body>";
    let css = "#grid { display: grid; grid-template-columns: repeat(3, 1fr) } \
               #grid > div { height: 40px }";
    let (boxes, layout) = lay_out(html, css, Size::new(300.0, 300.0));

    assert_eq!(
        rect_of(&boxes, &layout, "a", html),
        Rect::new(0.0, 0.0, 100.0, 40.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "b", html),
        Rect::new(100.0, 0.0, 100.0, 40.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "c", html),
        Rect::new(200.0, 0.0, 100.0, 40.0)
    );
}

#[test]
fn a_grid_item_goes_where_it_is_placed_and_covers_what_it_spans() {
    let html = "<body><div id=grid><div id=wide></div><div id=small></div></div></body>";
    let css = "#grid { display: grid; grid-template-columns: 100px 100px 100px } \
               #wide { grid-column: 1 / span 2; height: 20px } \
               #small { grid-column: 3; height: 20px }";
    let (boxes, layout) = lay_out(html, css, Size::new(300.0, 300.0));

    assert_eq!(
        rect_of(&boxes, &layout, "wide", html),
        Rect::new(0.0, 0.0, 200.0, 20.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "small", html),
        Rect::new(200.0, 0.0, 100.0, 20.0)
    );
}

#[test]
fn minmax_holds_a_track_between_its_two_ends() {
    let html = "<body><div id=grid><div id=a></div><div id=b></div></div></body>";
    let css = "#grid { display: grid; grid-template-columns: minmax(50px, 100px) 1fr } \
               #grid > div { height: 10px }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));

    assert!(close(rect_of(&boxes, &layout, "a", html).size.width, 100.0));
    assert_eq!(
        rect_of(&boxes, &layout, "b", html),
        Rect::new(100.0, 0.0, 300.0, 10.0)
    );
}

#[test]
fn a_percentage_width_is_of_the_containing_block() {
    let html = "<body><div id=outer><div id=inner></div></div></body>";
    let css = "#outer { width: 200px; height: 100px } #inner { width: 50%; height: 25% }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));

    assert_eq!(
        rect_of(&boxes, &layout, "inner", html).size,
        Size::new(100.0, 25.0)
    );
}

#[test]
fn an_em_length_is_of_the_font_that_element_ended_up_with() {
    let html = "<body><div id=outer><div id=inner></div></div></body>";
    let css = "#outer { font-size: 20px } #inner { width: 3em; height: 1em }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));

    assert_eq!(
        rect_of(&boxes, &layout, "inner", html).size,
        Size::new(60.0, 20.0),
        "the cascade said twenty pixels and layout used it",
    );
}

#[test]
fn a_relative_box_moves_and_an_absolute_one_leaves_the_flow() {
    let html = "<body><div id=a></div><div id=b></div><div id=c></div></body>";
    let css = "div { height: 20px } \
               #b { position: relative; top: 5px; left: 10px } ";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));

    assert_eq!(
        rect_of(&boxes, &layout, "b", html),
        Rect::new(10.0, 25.0, 400.0, 20.0)
    );
    assert!(
        close(rect_of(&boxes, &layout, "c", html).origin.y, 40.0),
        "and nothing else moved, which is what relative means",
    );

    let absolute = "div { height: 20px } #b { position: absolute; top: 100px; left: 50px }";
    let (boxes, layout) = lay_out(html, absolute, Size::new(400.0, 300.0));
    assert_eq!(
        rect_of(&boxes, &layout, "b", html).origin,
        alo_layout::Point::new(50.0, 100.0)
    );
    assert!(
        close(rect_of(&boxes, &layout, "c", html).origin.y, 20.0),
        "and the flow closed up behind it",
    );
}

#[test]
fn text_wraps_where_a_line_may_break_and_nowhere_else() {
    let html = "<body><div id=a>abcd efgh</div></body>";

    let narrow = lay_out(html, "#a { width: 40px }", Size::new(400.0, 300.0));
    assert_eq!(
        rect_of(&narrow.0, &narrow.1, "a", html).size,
        Size::new(40.0, 32.0),
        "two words, one to a line, two lines of sixteen",
    );

    let wide = lay_out(html, "#a { width: 200px }", Size::new(400.0, 300.0));
    assert_eq!(
        rect_of(&wide.0, &wide.1, "a", html).size,
        Size::new(200.0, 16.0),
    );

    // A word with nowhere to break overflows rather than being cut in half.
    let unbreakable = "<body><div id=a>abcdefgh</div></body>";
    let tight = lay_out(unbreakable, "#a { width: 32px }", Size::new(400.0, 300.0));
    assert_eq!(
        rect_of(&tight.0, &tight.1, "a", unbreakable).size,
        Size::new(32.0, 16.0),
        "one line, and the text sticks out of it",
    );
}

/// Every rectangle whose element carries this `id`, in tree order.
///
/// An inline box broken around a block is two boxes from one element, so
/// asking for "the" rectangle of one would answer about half of it.
fn rects_of(boxes: &BoxTree, layout: &LayoutTree, wanted: &str, document_html: &str) -> Vec<Rect> {
    let document = parse_document(document_html);
    let Some(node) = document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.attr("id") == Some(wanted))
    }) else {
        return Vec::new();
    };
    let Some(root) = boxes.root() else {
        return Vec::new();
    };
    core::iter::once(root)
        .chain(boxes.descendants(root))
        .filter(|id| boxes.get(*id).and_then(|held| held.kind.node()) == Some(node))
        .filter_map(|id| layout.border_box(id))
        .collect()
}

#[test]
fn an_inline_broken_around_a_block_lays_out_in_three_bands() {
    let html = "<body><div id=w><span id=a>xx<p id=b>yy</p>zz</span></div></body>";
    let (boxes, layout) = lay_out(
        html,
        "#w { width: 100px } p { margin: 0 }",
        Size::new(200.0, 200.0),
    );

    let pieces = rects_of(&boxes, &layout, "a", html);
    assert_eq!(pieces.len(), 2, "the span is in two pieces: {pieces:?}");

    let first = pieces.first().copied().expect("a first piece");
    let block = rect_of(&boxes, &layout, "b", html);
    let second = pieces.get(1).copied().expect("a second piece");

    // Three bands, sixteen pixels each: the text before, the block, the text
    // after. The block is a *sibling* of the anonymous blocks the pieces sit
    // in, so it starts at the container's left edge and fills its width.
    assert!(close(first.origin.y, 0.0), "{first:?}");
    assert!(close(first.size.height, 16.0), "{first:?}");
    assert!(close(block.origin.x, 0.0), "{block:?}");
    assert!(close(block.origin.y, 16.0), "{block:?}");
    assert!(
        close(block.size.width, 100.0),
        "the block fills the width: {block:?}"
    );
    assert!(close(second.origin.y, 32.0), "{second:?}");

    // And the whole thing is three lines tall, not one.
    assert!(close(rect_of(&boxes, &layout, "w", html).size.height, 48.0));
}

#[test]
fn a_broken_inline_starts_each_piece_at_the_left_again() {
    // The point of breaking rather than stretching: the second piece is a box
    // of its own, so its background starts where its own text does.
    let html = "<body><div id=w><span id=a>xxxx<p>y</p>zz</span></div></body>";
    let (boxes, layout) = lay_out(
        html,
        "#w { width: 100px } p { margin: 0 }",
        Size::new(200.0, 200.0),
    );
    let pieces = rects_of(&boxes, &layout, "a", html);
    let first = pieces.first().copied().expect("a first piece");
    let second = pieces.get(1).copied().expect("a second piece");
    assert!(close(first.origin.x, 0.0), "{first:?}");
    assert!(close(second.origin.x, 0.0), "{second:?}");
    assert!(
        second.size.width < first.size.width,
        "each piece is as wide as its own text: {first:?} then {second:?}",
    );
}

#[test]
fn with_no_font_text_has_no_size_and_the_boxes_still_lay_out() {
    let document = parse_document("<body><div id=a>text</div></body>");
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet("#a { width: 100px }");
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build(&document, &styles);
    let layout = compute(&boxes, &styles, Size::new(400.0, 300.0), &NoText);

    let root = boxes.root().expect("a root");
    assert!(layout.get(root).is_some());
    assert_eq!(layout.len(), boxes.len(), "every box got a rectangle");
}

#[test]
fn the_whole_layout_of_a_small_interface_is_what_it_should_be() {
    let html = "<body><main id=main>\
        <h1 id=title>Invoices</h1>\
        <ul id=rows><li>One</li><li>Two</li></ul>\
        </main></body>";
    let css = "main { padding: 8px } h1 { height: 24px; margin: 0 } \
               ul { display: flex; gap: 4px; margin: 0; padding: 0 } \
               li { width: 60px; height: 20px }";
    let (boxes, layout) = lay_out(html, css, Size::new(200.0, 200.0));

    let expected = "\
block flow · document → 200×60 at (0, 0)
  block flow · generic → 200×60 at (0, 0)
    block flow · main → 200×60 at (0, 0)
      block flow · heading [level=1] → 184×24 at (8, 8)
        text \"Invoices\" → 64×16 at (8, 8)
      block flex · list → 184×20 at (8, 32)
        block flow list-item · listitem → 60×20 at (8, 32)
          text \"One\" → 24×16 at (8, 32)
        block flow list-item · listitem → 60×20 at (72, 32)
          text \"Two\" → 24×16 at (72, 32)
";
    assert_eq!(layout.to_outline(&boxes), expected);
}

#[test]
fn a_calc_of_lengths_is_a_number_before_layout_ever_sees_it() {
    let html = "<body><div id=a></div></body>";
    let (boxes, layout) = lay_out(
        html,
        "#a { width: calc(4px * 5); height: 10px }",
        Size::new(400.0, 300.0),
    );
    assert!(close(rect_of(&boxes, &layout, "a", html).size.width, 20.0));
    assert!(layout.issues().is_empty());
}

#[test]
fn a_calc_with_a_percentage_is_resolved_against_the_containing_block() {
    // The value ADR 0004 was written for: a full-width thing with a gutter.
    let html = "<body><div id=w><div id=a></div></div></body>";
    let (boxes, layout) = lay_out(
        html,
        "#w { width: 400px } #a { width: calc(100% - 20px); height: 10px }",
        Size::new(600.0, 300.0),
    );
    assert!(
        close(rect_of(&boxes, &layout, "a", html).size.width, 380.0),
        "{:?}",
        rect_of(&boxes, &layout, "a", html),
    );
    assert!(layout.issues().is_empty(), "{:?}", layout.issues());
}

#[test]
fn a_calc_with_a_percentage_works_in_every_property_that_takes_one() {
    let html = "<body><div id=w><div id=a></div></div></body>";
    let (boxes, layout) = lay_out(
        html,
        "#w { width: 400px; height: 200px }
         #a { width: calc(50% + 10px); height: 20px;
              margin-left: calc(25% - 20px);
              padding-left: calc(10% + 5px);
              min-width: calc(10% + 1px) }",
        Size::new(600.0, 300.0),
    );
    let rect = rect_of(&boxes, &layout, "a", html);
    // A content box of half four hundred and ten more, plus a padding of a
    // tenth and five more: 210 + 45. The margin is a quarter less twenty.
    assert!(close(rect.size.width, 255.0), "{rect:?}");
    assert!(close(rect.origin.x, 80.0), "{rect:?}");
    assert!(layout.issues().is_empty(), "{:?}", layout.issues());
}

#[test]
fn a_calc_with_a_percentage_sizes_a_grid_track() {
    let html = "<body><div id=w><div id=a></div></div></body>";
    let (boxes, layout) = lay_out(
        html,
        "#w { display: grid; width: 400px;
              grid-template-columns: calc(50% - 20px) 1fr }
         #a { height: 10px }",
        Size::new(600.0, 300.0),
    );
    assert!(
        close(rect_of(&boxes, &layout, "a", html).size.width, 180.0),
        "{:?}",
        rect_of(&boxes, &layout, "a", html),
    );
    assert!(layout.issues().is_empty(), "{:?}", layout.issues());
}

#[test]
fn an_inline_boxs_horizontal_border_and_padding_take_room_on_the_line() {
    let html = "<body><p id=p><span id=s>abcd</span></p></body>";
    let css = "p { margin: 0; width: 200px }
               #s { padding-left: 10px; padding-right: 6px;
                    border-left-width: 2px; border-right-width: 2px;
                    border-left-style: solid; border-right-style: solid }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));

    // Four characters of eight, with ten and six of padding and two of border
    // on each side: 2 + 10 + 32 + 6 + 2.
    let span = rect_of(&boxes, &layout, "s", html);
    assert!(close(span.size.width, 52.0), "{span:?}");
    assert!(close(span.origin.x, 0.0), "{span:?}");
}

#[test]
fn an_inline_boxs_vertical_padding_draws_without_changing_the_line() {
    let html = "<body><p id=p><span id=s>abcd</span></p></body>";
    let plain = lay_out(
        html,
        "p { margin: 0; width: 200px }",
        Size::new(400.0, 300.0),
    );
    let padded = lay_out(
        html,
        "p { margin: 0; width: 200px }
         #s { padding-top: 9px; padding-bottom: 9px }",
        Size::new(400.0, 300.0),
    );

    // The paragraph is the same height either way — CSS's rule, and what stops
    // a padded `<em>` pushing a paragraph's lines apart.
    assert!(close(
        rect_of(&plain.0, &plain.1, "p", html).size.height,
        rect_of(&padded.0, &padded.1, "p", html).size.height,
    ));
    // And the span itself is taller by the padding, because it is drawn.
    let grown = rect_of(&padded.0, &padded.1, "s", html).size.height
        - rect_of(&plain.0, &plain.1, "s", html).size.height;
    assert!(close(grown, 18.0), "{grown}");
}

#[test]
fn an_inline_box_that_wraps_has_one_rectangle_per_line() {
    let html = "<body><p id=p><span id=s>abcd efgh</span></p></body>";
    let (boxes, layout) = lay_out(
        html,
        "p { margin: 0; width: 40px } #s { padding-left: 4px }",
        Size::new(400.0, 300.0),
    );
    let root = boxes.root().expect("a root");
    let span = core::iter::once(root)
        .chain(boxes.descendants(root))
        .find(|id| {
            boxes
                .get(*id)
                .and_then(|node| node.kind.node())
                .is_some_and(|source| {
                    parse_document(html)
                        .element(source)
                        .is_some_and(|element| element.attr("id") == Some("s"))
                })
        })
        .expect("the span");

    let pieces = layout.fragments(span);
    assert_eq!(pieces.len(), 2, "one piece per line: {pieces:?}");
    // The first piece carries the start padding; the second starts at the
    // left edge with none, because it has already had it.
    assert!(close(pieces[0].rect.origin.x, 0.0));
    assert!(close(pieces[0].rect.size.width, 36.0), "{:?}", pieces[0]);
    assert!(close(pieces[1].rect.origin.x, 0.0));
    assert!(close(pieces[1].rect.size.width, 32.0), "{:?}", pieces[1]);
}

#[test]
fn an_empty_piece_of_a_broken_inline_costs_no_height_when_it_has_no_border() {
    // CSS keeps the piece — "even if either side is empty" — and then says a
    // line box holding only empty inline boxes with no border and no padding
    // is zero-height and treated as not existing. Both together: the piece is
    // there, and it costs nothing.
    let broken = "<body><div id=w><span>abcd<p id=b>e</p></span></div></body>";
    let plain = "<body><div id=w><span>abcd</span><p id=b>e</p></div></body>";
    let css = "p { margin: 0 } #w { width: 100px }";

    let with = lay_out(broken, css, Size::new(400.0, 300.0));
    let without = lay_out(plain, css, Size::new(400.0, 300.0));
    assert!(close(
        rect_of(&with.0, &with.1, "w", broken).size.height,
        rect_of(&without.0, &without.1, "w", plain).size.height,
    ));
}

#[test]
fn an_empty_piece_with_a_border_keeps_its_line_and_draws_it() {
    let html = "<body><div id=w><span id=s>abcd<p id=b>e</p></span></div></body>";
    let bare = lay_out(
        html,
        "p { margin: 0 } #w { width: 100px }",
        Size::new(400.0, 300.0),
    );
    let bordered = lay_out(
        html,
        "p { margin: 0 } #w { width: 100px }
         #s { border-left-width: 3px; border-left-style: solid }",
        Size::new(400.0, 300.0),
    );
    assert!(
        rect_of(&bordered.0, &bordered.1, "w", html).size.height
            > rect_of(&bare.0, &bare.1, "w", html).size.height,
        "an empty inline with a border is a line, and a line has a height",
    );
}

#[test]
fn whitespace_a_person_did_not_mean_to_write_is_collapsed() {
    // Markup is indented for people. Before this, an indented paragraph was
    // drawn with its indentation in it, because the shaper was handed whatever
    // bytes the parser produced.
    let html = "<body><div id=a>one   two</div></body>";
    let (boxes, layout) = lay_out(html, "#a { width: 200px }", Size::new(400.0, 300.0));
    // Eight characters and one space at eight pixels each.
    assert!(close(rect_of(&boxes, &layout, "a", html).size.height, 16.0,));

    let spread = "<body><div id=a>one\n\n   two</div></body>";
    let (boxes, layout) = lay_out(spread, "#a { width: 200px }", Size::new(400.0, 300.0));
    assert!(
        close(rect_of(&boxes, &layout, "a", spread).size.height, 16.0),
        "a newline in the source is a space, not a line",
    );
}

#[test]
fn pre_line_keeps_the_newlines_and_makes_a_line_of_each() {
    // alo's own headline is one string with newlines in it. Three lines, and
    // the box is three lines tall.
    let html = "<body><div id=a>Your workspace.\nYour servers.\nYour rules.</div></body>";
    let (boxes, layout) = lay_out(
        html,
        "#a { width: 400px; white-space: pre-line }",
        Size::new(400.0, 300.0),
    );
    assert!(
        close(rect_of(&boxes, &layout, "a", html).size.height, 48.0),
        "three lines of sixteen: {:?}",
        rect_of(&boxes, &layout, "a", html),
    );

    // The same string without the rule is one line, because a newline is a
    // space.
    let (boxes, layout) = lay_out(html, "#a { width: 400px }", Size::new(400.0, 300.0));
    assert!(close(rect_of(&boxes, &layout, "a", html).size.height, 16.0));
}

#[test]
fn a_line_that_may_not_wrap_overflows_instead() {
    let html = "<body><div id=a>one two three four five</div></body>";
    let (boxes, layout) = lay_out(
        html,
        "#a { width: 40px; white-space: nowrap }",
        Size::new(400.0, 300.0),
    );
    assert!(
        close(rect_of(&boxes, &layout, "a", html).size.height, 16.0),
        "nowrap is one line however long it is: {:?}",
        rect_of(&boxes, &layout, "a", html),
    );
}

#[test]
fn pre_keeps_every_space_and_every_line() {
    let html = "<body><pre id=a>one   two\nthree</pre></body>";
    let (boxes, layout) = lay_out(html, "#a { margin: 0 }", Size::new(400.0, 300.0));
    // Two lines, from the user-agent sheet's own `pre { white-space: pre }` —
    // which was there before anything read it.
    assert!(
        close(rect_of(&boxes, &layout, "a", html).size.height, 32.0),
        "{:?}",
        rect_of(&boxes, &layout, "a", html),
    );
}

#[test]
fn letter_spacing_changes_what_a_run_measures_and_so_where_it_breaks() {
    // The test font is eight pixels a character. Five characters with two
    // pixels after each is fifty, not forty — and a box of forty-five then
    // holds four of them rather than five.
    let html = "<body><div id=a>ab cd</div></body>";
    let tight = lay_out(html, "#a { width: 45px }", Size::new(400.0, 300.0));
    assert!(close(
        rect_of(&tight.0, &tight.1, "a", html).size.height,
        16.0
    ));

    let spaced = lay_out(
        html,
        "#a { width: 45px; letter-spacing: 2px }",
        Size::new(400.0, 300.0),
    );
    assert!(
        rect_of(&spaced.0, &spaced.1, "a", html).size.height > 16.0,
        "spacing pushed it onto a second line: {:?}",
        rect_of(&spaced.0, &spaced.1, "a", html),
    );
}

#[test]
fn negative_letter_spacing_pulls_a_line_back_together() {
    let html = "<body><div id=a>ab cd</div></body>";
    let wide = lay_out(html, "#a { width: 36px }", Size::new(400.0, 300.0));
    assert!(
        rect_of(&wide.0, &wide.1, "a", html).size.height > 16.0,
        "forty pixels of text does not fit in thirty-six",
    );

    let tightened = lay_out(
        html,
        "#a { width: 36px; letter-spacing: -1px }",
        Size::new(400.0, 300.0),
    );
    assert!(
        close(
            rect_of(&tightened.0, &tightened.1, "a", html).size.height,
            16.0
        ),
        "and thirty-five does: {:?}",
        rect_of(&tightened.0, &tightened.1, "a", html),
    );
}

#[test]
fn letter_spacing_of_normal_is_no_spacing_at_all() {
    let html = "<body><div id=a>ab cd</div></body>";
    let plain = lay_out(html, "#a { width: 45px }", Size::new(400.0, 300.0));
    let normal = lay_out(
        html,
        "#a { width: 45px; letter-spacing: normal }",
        Size::new(400.0, 300.0),
    );
    assert_eq!(plain.1.to_outline(&plain.0), normal.1.to_outline(&normal.0),);
}

#[test]
fn an_empty_field_is_still_one_line_tall() {
    // Not from a height in the user-agent sheet — a fixed height would be too
    // short for a field with something in it, which is exactly what happened.
    // It comes from the box the control holds its text in.
    let html = "<body><input id=a></body>";
    let (boxes, layout) = lay_out(html, "", Size::new(400.0, 300.0));
    let field = rect_of(&boxes, &layout, "a", html);
    // A line is 19.2 — one and a fifth of the sixteen-pixel default font —
    // plus a pixel of padding and a pixel of border on each side.
    assert!(close(field.size.height, 23.2), "{field:?}");
}

#[test]
fn a_field_with_something_in_it_is_tall_enough_for_it() {
    let html = "<body><input id=a value='typed'></body>";
    let (boxes, layout) = lay_out(
        html,
        "#a { padding: 8px; border-top-width: 1px; border-right-width: 1px;
              border-bottom-width: 1px; border-left-width: 1px;
              border-top-style: solid; border-right-style: solid;
              border-bottom-style: solid; border-left-style: solid }",
        Size::new(400.0, 300.0),
    );
    let field = rect_of(&boxes, &layout, "a", html);
    // A line of 19.2, sixteen of padding, two of border. The line rather than
    // the text: a line box is as tall as its line height, which is what stops
    // a field's own text touching its border.
    assert!(close(field.size.height, 37.2), "{field:?}");
}

#[test]
fn a_tall_buttons_label_sits_in_the_middle_of_it() {
    let html = "<body><button id=a>Save</button></body>";
    let (boxes, layout) = lay_out(
        html,
        "#a { width: 100px; height: 46px; padding: 0; border: 0 }",
        Size::new(400.0, 300.0),
    );
    let root = boxes.root().expect("a root");
    let label = boxes
        .descendants(root)
        .into_iter()
        .find(|id| {
            boxes
                .get(*id)
                .and_then(|node| node.text().map(str::to_owned))
                .is_some_and(|text| text.trim() == "Save")
        })
        .expect("the label");
    let piece = layout.border_box(label).expect("a rectangle");

    let button = rect_of(&boxes, &layout, "a", html);
    // Four characters of eight, centred across a hundred; sixteen of line,
    // centred down forty-six.
    assert!(close(piece.origin.x, button.origin.x + 34.0), "{piece:?}");
    assert!(close(piece.origin.y, button.origin.y + 15.0), "{piece:?}");
}

#[test]
fn a_button_an_author_made_a_flex_container_is_theirs_to_align() {
    // The reason a button's label is centred by a box in the tree rather than
    // by a rule in the user-agent sheet: a rule would centre this too, and an
    // author cannot override a rule they cannot see.
    let html = "<body><button id=a><span>Left</span></button></body>";
    let (boxes, layout) = lay_out(
        html,
        "#a { display: flex; width: 100px; padding: 0; border: 0 }",
        Size::new(400.0, 300.0),
    );
    let root = boxes.root().expect("a root");
    let span = boxes
        .descendants(root)
        .into_iter()
        .find(|id| {
            boxes
                .get(*id)
                .and_then(|node| node.text().map(str::to_owned))
                .is_some_and(|text| text.trim() == "Left")
        })
        .expect("the label");
    let piece = layout.border_box(span).expect("a rectangle");
    let button = rect_of(&boxes, &layout, "a", html);
    assert!(
        close(piece.origin.x, button.origin.x),
        "a flex container starts its items at the start: {piece:?}",
    );
}

#[test]
fn a_document_that_generates_no_boxes_lays_out_nothing_and_does_not_mind() {
    let (boxes, layout) = lay_out(
        "<p>t</p>",
        "html { display: none }",
        Size::new(400.0, 300.0),
    );
    assert!(boxes.root().is_none());
    assert!(layout.is_empty());
    assert_eq!(layout.viewport(), Size::new(400.0, 300.0));
    assert_eq!(layout.to_outline(&boxes), "");
}

// --- what a line box does that a row of boxes cannot ------------------------

#[test]
fn a_sentence_breaks_between_two_inline_boxes_and_not_only_around_them() {
    let html = "<body><p id=p>the <em id=em>quick brown</em> fox</p></body>";
    let (boxes, layout) = lay_out(
        html,
        "#p { width: 80px } p { margin: 0 }",
        Size::new(400.0, 300.0),
    );

    let paragraph = rect_of(&boxes, &layout, "p", html);
    assert!(
        paragraph.size.height > 16.0,
        "it wrapped: {}",
        paragraph.size.height,
    );

    // The `<em>` is one sentence with what surrounds it, so the break can land
    // inside it — which a row of three boxes could never do.
    let em = rect_of(&boxes, &layout, "em", html);
    assert!(em.size.width <= 80.001, "and it stays inside the paragraph");
}

#[test]
fn a_box_that_wraps_is_drawn_in_one_piece_per_line() {
    let html = "<body><p id=p><a id=link>one two three four</a></p></body>";
    let (boxes, layout) = lay_out(
        html,
        "#p { width: 60px } p { margin: 0 }",
        Size::new(400.0, 300.0),
    );

    let document = parse_document(html);
    let node = document
        .descendants(document.root())
        .find(|id| {
            document
                .element(*id)
                .is_some_and(|element| element.attr("id") == Some("link"))
        })
        .expect("the link");
    let root = boxes.root().expect("a root");
    let link = core::iter::once(root)
        .chain(boxes.descendants(root))
        .find(|id| boxes.get(*id).and_then(|held| held.kind.node()) == Some(node))
        .expect("the link's box");

    // The link's text is inside it, and that is what was fragmented.
    let text_box = boxes
        .descendants(link)
        .into_iter()
        .find(|id| boxes.get(*id).is_some_and(|held| held.text().is_some()))
        .expect("the link's text");

    assert!(
        layout.is_fragmented(text_box),
        "four words in sixty pixels is more than one line",
    );
    let pieces = layout.fragments(text_box);
    assert!(pieces.len() > 1);
    for pair in pieces.windows(2) {
        assert!(
            pair[1].rect.top() >= pair[0].rect.bottom() - 0.001,
            "and each piece is below the one before it rather than beside it",
        );
    }

    let union = layout.border_box(text_box).expect("a union rectangle");
    assert!(
        union.size.height > pieces[0].rect.size.height,
        "the union covers the gap between the lines, which is why paint uses the pieces",
    );
}

#[test]
fn things_of_different_heights_on_one_line_sit_on_one_baseline() {
    let html = "<body><p id=p>x<img id=tall>y</p></body>";
    let css = "p { margin: 0; width: 300px } #tall { width: 20px; height: 40px }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));

    let image = rect_of(&boxes, &layout, "tall", html);
    let paragraph = rect_of(&boxes, &layout, "p", html);

    assert_eq!(
        image.size,
        Size::new(20.0, 40.0),
        "the image keeps its size"
    );
    assert!(
        paragraph.size.height >= 40.0,
        "and the line grew to hold it rather than clipping it: {}",
        paragraph.size.height,
    );
    assert!(
        close(image.top(), paragraph.top()),
        "the tallest thing sets the baseline, so it starts at the top of the line",
    );
}
