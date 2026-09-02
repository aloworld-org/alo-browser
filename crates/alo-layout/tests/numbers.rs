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
    let author = parse_stylesheet(css);
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
fn a_calc_of_lengths_works_and_a_calc_of_percentages_is_refused_and_recorded() {
    let html = "<body><div id=a></div></body>";
    let (boxes, layout) = lay_out(
        html,
        "#a { width: calc(4px * 5); height: 10px }",
        Size::new(400.0, 300.0),
    );
    assert!(close(rect_of(&boxes, &layout, "a", html).size.width, 20.0));
    assert!(layout.issues().is_empty());

    let (_, mixed) = lay_out(
        html,
        "#a { width: calc(100% - 20px); height: 10px }",
        Size::new(400.0, 300.0),
    );
    assert!(
        mixed
            .issues()
            .iter()
            .any(|issue| issue.source.contains("queue item 15")),
        "refused and said so rather than becoming something else: {:?}",
        mixed
            .issues()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
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
