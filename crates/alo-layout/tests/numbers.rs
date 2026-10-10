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
use alo_layout::{BlockFont, LayoutTree, MeasureText, NoText, Rect, ScaledFont, Size, compute};
use alo_style::{
    ComputedStyle, FaceUnits, MeasureFace, Origin, SourcedSheet, StyleTree, USER_AGENT_STYLE_SHEET,
    resolve, resolve_measured,
};

/// Equal to within far less than a pixel. A layout assertion is about the
/// number, not about whether two floats happen to be bit-identical.
fn close(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.0001
}

fn lay_out(html: &str, css: &str, viewport: Size) -> (BoxTree, LayoutTree) {
    lay_out_measured(html, css, viewport, &BlockFont)
}

/// The same, measuring text with a given measurer — for the tests where the
/// font's size has to matter, which [`BlockFont`] deliberately ignores.
fn lay_out_measured(
    html: &str,
    css: &str,
    viewport: Size,
    measure: &impl MeasureText,
) -> (BoxTree, LayoutTree) {
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
    let layout = compute(&boxes, &styles, viewport, measure);
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

/// Everything about where a box ended up, rather than only its rectangle —
/// for the assertions that are about a border or a band rather than a size.
fn geometry_of(
    boxes: &BoxTree,
    layout: &LayoutTree,
    wanted: &str,
    document_html: &str,
) -> alo_layout::BoxGeometry {
    let rect = rect_of(boxes, layout, wanted, document_html);
    let Some(root) = boxes.root() else {
        return alo_layout::BoxGeometry::default();
    };
    core::iter::once(root)
        .chain(boxes.descendants(root))
        .filter_map(|id| layout.get(id))
        .find(|geometry| geometry.border_box == rect)
        .unwrap_or_default()
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

/// `place-items` is `align-items` and then `justify-items`: one value both
/// ways, two values block axis first. A 100×40 item in one 300×200 cell.
#[test]
fn place_items_aligns_an_item_in_its_cell_both_ways() {
    let html = "<body><div id=grid><div id=item></div></div></body>";
    for (place, x, y) in [
        ("center", 100.0, 80.0),
        ("end start", 0.0, 160.0),
        ("start end", 200.0, 0.0),
        ("center; justify-items: start", 0.0, 80.0),
    ] {
        let css = format!(
            "#grid {{ display: grid; width: 300px; height: 200px; place-items: {place} }} \
             #item {{ width: 100px; height: 40px }}"
        );
        let (boxes, layout) = lay_out(html, &css, Size::new(400.0, 300.0));
        assert_eq!(
            rect_of(&boxes, &layout, "item", html),
            Rect::new(x, y, 100.0, 40.0),
            "place-items: {place}",
        );
    }
}

/// `place-self` on the item overrides the container's `place-items`.
#[test]
fn place_self_overrides_the_containers_place_items() {
    let html = "<body><div id=grid><div id=item></div></div></body>";
    let css = "#grid { display: grid; width: 300px; height: 200px; place-items: start } \
               #item { width: 100px; height: 40px; place-self: end center }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));
    assert_eq!(
        rect_of(&boxes, &layout, "item", html),
        Rect::new(100.0, 160.0, 100.0, 40.0)
    );
}

/// `place-content` moves the tracks, not the item in its cell: a 100×40
/// track in a 300×200 grid.
#[test]
fn place_content_moves_the_grids_tracks_both_ways() {
    let html = "<body><div id=grid><div id=item></div></div></body>";
    for (place, x, y) in [("center", 100.0, 80.0), ("end start", 0.0, 160.0)] {
        let css = format!(
            "#grid {{ display: grid; width: 300px; height: 200px; \
             grid-template-columns: 100px; grid-template-rows: 40px; place-content: {place} }}"
        );
        let (boxes, layout) = lay_out(html, &css, Size::new(400.0, 300.0));
        assert_eq!(
            rect_of(&boxes, &layout, "item", html),
            Rect::new(x, y, 100.0, 40.0),
            "place-content: {place}",
        );
    }
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

/// `max-width: none` undoes a limit set further up the sheet, as alo Sites'
/// banner sections write it, and is read rather than refused. `auto` is not a
/// maximum's value and is recorded.
#[test]
fn a_maximum_of_none_undoes_a_limit_and_says_nothing_about_it() {
    let html = "<body><div id=card class=card></div><div id=banner class='card banner'></div>\
                <div id=tall class=tall><div id=fill></div></div></body>";
    let css = ".card { max-width: 100px; height: 10px } .card.banner { max-width: none }
               .tall { max-height: 20px } #tall.tall { max-height: none } #fill { height: 50px }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));

    assert_eq!(
        rect_of(&boxes, &layout, "card", html),
        Rect::new(0.0, 0.0, 100.0, 10.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "banner", html),
        Rect::new(0.0, 10.0, 400.0, 10.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "tall", html),
        Rect::new(0.0, 20.0, 400.0, 50.0)
    );
    assert!(layout.issues().is_empty(), "{:?}", layout.issues());

    let css = ".card { max-width: auto; height: 10px }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));
    assert_eq!(
        rect_of(&boxes, &layout, "card", html).size,
        Size::new(400.0, 10.0)
    );
    let refused: Vec<_> = layout
        .issues()
        .iter()
        .map(|issue| issue.source.as_str())
        .collect();
    assert_eq!(refused, ["max-width: auto", "max-width: auto"]);
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

/// Queue item 352, from alo Sites' skip link: an inline with `position:
/// absolute` is blockified and placed out of flow, as a block is, so it is
/// at its offsets, as wide as its text and its padding, and the block after
/// it starts at the top.
#[test]
fn an_absolutely_positioned_inline_leaves_the_flow_and_its_line() {
    let html = "<body><a id=skip href=#m>Skip</a><main id=m></main></body>";
    let css = "main { height: 40px } \
               #skip { position: absolute; left: -999rem; top: 0; \
                       padding: 8px 16px }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));

    let skip = rect_of(&boxes, &layout, "skip", html);
    assert_eq!(skip.origin, alo_layout::Point::new(-999.0 * 16.0, 0.0));
    // Four characters of eight pixels, and sixteen of padding each side:
    // shrunk to what it holds, not stretched across the page as an
    // in-flow block would be.
    assert!(close(skip.size.width, 4.0 * 8.0 + 2.0 * 16.0), "{skip:?}");
    assert_eq!(
        rect_of(&boxes, &layout, "m", html),
        Rect::new(0.0, 0.0, 400.0, 40.0),
        "nothing is left in flow above the block after it",
    );
}

/// Queue item 355: an absolutely positioned box is placed against the
/// padding box of its nearest positioned ancestor (CSS 2 § 10.1), not its
/// parent. The section between them is 30 further right; the span is not.
#[test]
fn an_absolute_box_is_placed_against_its_nearest_positioned_ancestor() {
    let html = "<body><div id=p><section><span id=t>x</span></section></div></body>";
    let css = "#p { position: relative; margin-left: 50px; width: 200px; height: 100px; \
                    padding: 10px; border: 2px solid } \
               section { margin-left: 30px; width: 100px } \
               #t { position: absolute; left: 0; top: 0; width: 50% }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));

    let t = rect_of(&boxes, &layout, "t", html);
    // The padding box's corner: 50 of margin and 2 of border across, 2 of
    // border down; and half of the padding box's 220, not of the section's
    // 100.
    assert_eq!(t.origin, alo_layout::Point::new(52.0, 2.0), "{t:?}");
    assert!(close(t.size.width, 110.0), "{t:?}");
    // `bottom` and `right` are measured from the same box's far edges.
    let css = css.replace(
        "left: 0; top: 0; width: 50%",
        "right: 0; bottom: 0; width: 20px; height: 10px",
    );
    let (boxes, layout) = lay_out(html, &css, Size::new(400.0, 300.0));
    let t = rect_of(&boxes, &layout, "t", html);
    // 52 + 220 - 20 across; 2 + 120 - 10 down.
    assert_eq!(t, Rect::new(252.0, 112.0, 20.0, 10.0));
}

/// Queue item 355, from alo Sites' footer: with nothing positioned above it,
/// a box is placed against the initial containing block, the viewport at the
/// top left of the page, even when its parent has been pushed down by a
/// margin it shares with its first child.
#[test]
fn with_nothing_positioned_a_box_is_placed_against_the_viewport() {
    let html = "<body><a id=skip href=#m>Skip</a><a id=corner>c</a><main id=m></main></body>";
    let css = "main { margin-top: 48px; height: 40px } \
               #skip { position: absolute; left: -999rem; top: 0 } \
               #corner { position: absolute; right: 0; bottom: 0; width: 10px; height: 10px }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));

    assert!(
        close(rect_of(&boxes, &layout, "m", html).origin.y, 48.0),
        "the margin collapsed through the body, which is 48 down",
    );
    assert_eq!(
        rect_of(&boxes, &layout, "skip", html).origin,
        alo_layout::Point::new(-999.0 * 16.0, 0.0),
        "the top of the page, not of the body",
    );
    assert_eq!(
        rect_of(&boxes, &layout, "corner", html),
        Rect::new(390.0, 290.0, 10.0, 10.0),
        "the viewport's corner, not the body's, which ends at 88",
    );
}

/// An axis whose insets are both `auto` keeps the box where it would have
/// been in its parent (CSS 2 § 10.3.7), even when the box is placed against
/// an ancestor further out; an axis with an inset is measured from that
/// ancestor.
#[test]
fn an_axis_with_no_inset_keeps_its_static_position() {
    let html = "<body><div id=p><section><div id=before></div><span id=t>x</span>\
                </section></div></body>";
    let css = "#p { position: relative; margin-left: 40px; padding: 4px } \
               section { margin-left: 30px; padding: 6px 0 0 8px } \
               #before { height: 20px } \
               #t { position: absolute; left: 0; margin-top: 3px }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));

    let t = rect_of(&boxes, &layout, "t", html);
    // Across, `left: 0` against `#p`'s padding box: 40. Down, no inset, so
    // where it would have been: 4 of `#p`'s padding, 6 of the section's, the
    // 20 before it, and its own 3 of margin.
    assert_eq!(t.origin, alo_layout::Point::new(40.0, 33.0), "{t:?}");

    let both = css.replace("left: 0; ", "");
    let (boxes, layout) = lay_out(html, &both, Size::new(400.0, 300.0));
    let t = rect_of(&boxes, &layout, "t", html);
    // Across too: 40, 4 of padding, 30 of the section's margin and 8 of its
    // padding.
    assert_eq!(t.origin, alo_layout::Point::new(82.0, 33.0), "{t:?}");
    assert!(layout.issues().is_empty(), "{:?}", layout.issues());
}

/// CSS Transforms 1 § 2: a transformed box contains its absolutely
/// positioned descendants, positioned or not.
#[test]
fn a_transformed_ancestor_contains_an_absolute_box() {
    let html = "<body><div id=p><section><span id=t>x</span></section></div></body>";
    let css = "#p { transform: translateX(0); margin-left: 40px; margin-top: 7px } \
               section { margin-left: 30px } \
               #t { position: absolute; left: 0; top: 0 }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));
    assert_eq!(
        rect_of(&boxes, &layout, "t", html).origin,
        alo_layout::Point::new(40.0, 7.0)
    );
}

/// A box placed against the initial containing block counts in the page's
/// scrolling area — the viewport's, which is made of the root's reach — and
/// one beyond the left of the page does not.
#[test]
fn a_box_placed_against_the_page_reaches_as_far_as_it_goes() {
    let html = "<body><div id=far></div><div id=left></div></body>";
    let css = "#far { position: absolute; top: 2000px; left: 10px; width: 600px; height: 50px } \
               #left { position: absolute; left: -500px; top: 0; width: 10px; height: 10px }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));
    let root = boxes
        .root()
        .and_then(|root| layout.get(root))
        .expect("the root is laid out");
    assert!(close(root.reach.width, 610.0), "{root:?}");
    assert!(close(root.reach.height, 2050.0), "{root:?}");
}

/// In a flex or grid container that is not its containing block, an axis
/// with no inset takes its static position from an empty stand-in, which a
/// container aligning by size would place differently; that is said.
#[test]
fn a_static_position_in_a_flex_container_further_in_is_recorded() {
    let html = "<body><div id=p><section><span id=t>x</span></section></div></body>";
    let css = "#p { position: relative } section { display: flex } \
               #t { position: absolute; left: 0 }";
    let (_, layout) = lay_out(html, css, Size::new(400.0, 300.0));
    assert!(
        layout
            .issues()
            .iter()
            .any(|issue| issue.source.contains("its static position is taken")),
        "{:?}",
        layout.issues()
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

/// Text's narrowest is its widest unbreakable piece, not its whole line
/// (CSS Sizing 3 § 5.1), so a `1fr` column holding a sentence is still half
/// of the grid and the sentence wraps in it — alo Sites' closed booking
/// section, whose notice took 366 of 686 until item 394. Text that may not
/// wrap is as narrow as its line.
#[test]
fn text_is_as_narrow_as_its_widest_word_and_no_narrower() {
    let html = "<body><div id=grid><p id=short>ab</p>\
                <p id=long>abcd efgh ijkl mnop qrst uvwx</p></div>\
                <div id=word>abcd efghij</div><div id=line>abcd efghij</div></body>";
    let css =
        "p { margin: 0 } #grid { display: grid; grid-template-columns: 1fr 1fr; width: 400px }
               #word, #line { width: min-content } #line { white-space: nowrap }";
    let (boxes, layout) = lay_out(html, css, Size::new(800.0, 300.0));

    assert_eq!(
        rect_of(&boxes, &layout, "short", html),
        Rect::new(0.0, 0.0, 200.0, 32.0),
        "half the grid, stretched to the row's two lines",
    );
    assert_eq!(
        rect_of(&boxes, &layout, "long", html),
        Rect::new(200.0, 0.0, 200.0, 32.0),
        "29 characters are 232 wide and wrap after the fifth word in 200",
    );
    assert_eq!(
        rect_of(&boxes, &layout, "word", html),
        Rect::new(0.0, 32.0, 48.0, 32.0),
        "efghij is the widest word, six characters, and each word has a line",
    );
    assert_eq!(
        rect_of(&boxes, &layout, "line", html),
        Rect::new(0.0, 64.0, 88.0, 16.0),
        "nowrap leaves nowhere to break, so the narrowest is the whole line",
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

#[test]
fn a_fieldsets_legend_sits_in_its_border_rather_than_under_it() {
    // The user-agent sheet's own numbers: a 2px border, 0.35em of padding
    // above and 0.625em below, 0.75em either side, and a legend as wide as its
    // words. "Size" is four characters of the eight-pixel font, and the
    // legend's own padding adds two either side.
    let html = "<body><fieldset id=f><legend id=l>Size</legend><p id=p>one</p></fieldset></body>";
    let (boxes, layout) = lay_out(html, "p { margin: 0 }", Size::new(400.0, 300.0));

    let legend = rect_of(&boxes, &layout, "l", html);
    let fieldset = rect_of(&boxes, &layout, "f", html);
    assert!(
        close(legend.top(), fieldset.top()),
        "the legend is *in* the border rather than under it, so it starts \
         where the fieldset starts: {legend:?} against {fieldset:?}",
    );
    assert_eq!(legend, Rect::new(16.0, 0.0, 36.0, 16.0));

    assert!(
        close(rect_of(&boxes, &layout, "p", html).top(), 21.6),
        "and what the fieldset holds starts below the legend, then the \
         padding: {:?}",
        rect_of(&boxes, &layout, "p", html),
    );
    assert!(
        close(fieldset.size.height, 49.6),
        "the band **replaces** the block-start border rather than adding to \
         it — sixteen of legend, 0.35em of padding, a line, 0.625em, and the \
         2px border along the bottom: {fieldset:?}",
    );
}

#[test]
fn a_fieldsets_band_says_where_its_border_is_drawn_and_where_it_is_not() {
    let html = "<body><fieldset id=f><legend id=l>Size</legend><p id=p>one</p></fieldset></body>";
    let (boxes, layout) = lay_out(html, "p { margin: 0 }", Size::new(400.0, 300.0));
    let band = geometry_of(&boxes, &layout, "f", html)
        .band
        .expect("a fieldset showing a legend has a band");

    assert!(close(band.height, 16.0), "as tall as the legend: {band:?}");
    assert!(
        close(band.stroke, 2.0),
        "the border the style asked for, which the layout run was not given: {band:?}",
    );
    assert!(
        close(band.inset(), 7.0),
        "drawn through the middle: {band:?}"
    );
    // The legend's margin box, as distances from the fieldset's left edge:
    // it starts 12px of padding and 2px of border in, and is 36 wide.
    assert!(
        close(band.gap.0, 14.0) && close(band.gap.1, 50.0),
        "the gap is exactly the legend: {band:?}",
    );
    assert!(
        close(geometry_of(&boxes, &layout, "f", html).border.top, 0.0),
        "and the border itself takes no room, because the band is the border",
    );
}

#[test]
fn a_fieldset_with_no_legend_has_an_ordinary_border() {
    let html = "<body><fieldset id=f><p id=p>one</p></fieldset></body>";
    let (boxes, layout) = lay_out(html, "p { margin: 0 }", Size::new(400.0, 300.0));
    let fieldset = geometry_of(&boxes, &layout, "f", html);
    assert_eq!(fieldset.band, None);
    assert!(close(fieldset.border.top, 2.0));
    assert!(
        close(rect_of(&boxes, &layout, "p", html).top(), 7.6),
        "the border, and then the padding",
    );
    assert!(
        close(fieldset.border_box.size.height, 35.6),
        "which is the same fieldset without a legend's sixteen pixels, and \
         with its border back: {fieldset:?}",
    );
}

/// The `left` of the element `i` inside a 200-pixel container `w`, the
/// container aligned as `align` says.
fn left_of_i_aligned(html: &str, align: &str) -> f32 {
    let css = format!(
        "#w {{ width: 200px; text-align: {align} }} p {{ margin: 0 }} \
         #i {{ display: inline-block; width: 40px; height: 20px }}"
    );
    let (boxes, layout) = lay_out(html, &css, Size::new(400.0, 300.0));
    rect_of(&boxes, &layout, "i", html).left()
}

#[test]
fn text_align_moves_an_atomic_inline_in_a_line_nobody_wrote() {
    // The `<p>` beside it means the inline-block's line is an anonymous block,
    // which has no style of its own and inherits the container's `text-align`.
    // alo's offline screen is exactly this, and its picture and its button
    // were drawn against the left edge.
    let html = "<body><div id=w><span id=i></span><p>x</p></div></body>";
    for (align, left) in [
        ("start", 0.0),
        ("left", 0.0),
        ("center", 80.0),
        ("end", 160.0),
        ("right", 160.0),
    ] {
        let found = left_of_i_aligned(html, align);
        assert!(close(found, left), "{align}: at {found}, not {left}");
    }
}

#[test]
fn a_line_of_text_and_an_inline_block_move_together() {
    // "ab" is sixteen pixels and the box forty: a fifty-six pixel line, and
    // aligning it moves the whole line rather than each piece on its own.
    let html = "<body><div id=w>ab<span id=i></span><p>x</p></div></body>";
    assert!(close(left_of_i_aligned(html, "start"), 16.0));
    assert!(close(left_of_i_aligned(html, "center"), 72.0 + 16.0));
    assert!(close(left_of_i_aligned(html, "right"), 144.0 + 16.0));
}

#[test]
fn an_inline_blocks_margins_count_in_its_lines_height() {
    // alo's offline screen: a picture with `margin-bottom: 20px` alone on its
    // line, and a heading after it. The margin box is what sits on the line,
    // and the strut's descent hangs below its baseline, so the line is
    // 6 + 20 + 20 + 4 tall and the next block starts under the margin *and*
    // the descent rather than straight against the picture.
    let html = "<body><div id=w><span id=i></span><p id=after>x</p></div></body>";
    let css = "p { margin: 0 } \
               #i { display: inline-block; width: 40px; height: 20px; \
                    margin: 6px 0 20px 12px }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));

    let inline_block = rect_of(&boxes, &layout, "i", html);
    assert_eq!(
        inline_block,
        Rect::new(12.0, 6.0, 40.0, 20.0),
        "the border box, inside its top and left margins",
    );
    let after = rect_of(&boxes, &layout, "after", html);
    assert!(
        close(after.top(), 50.0),
        "the next block starts below the bottom margin and the descent: {}",
        after.top(),
    );
}

#[test]
fn every_line_starts_as_tall_as_its_containers_font() {
    // The strut is the font of the block holding the lines, not the default
    // and not whatever is on the line. Half the font size a character, three
    // quarters of it above the baseline.
    let css = "#w { font-size: 40px } #s { font-size: 10px } \
               #i { display: inline-block; width: 20px; height: 20px }";

    // Small text in a 40-pixel block: the line is the large font's 40, and
    // the text stands on its baseline at 30 — so its top is at 22.5.
    let html = "<body><div id=w><span id=s>ab</span></div></body>";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    let block = rect_of(&boxes, &layout, "w", html);
    assert!(close(block.size.height, 40.0), "{block:?}");
    let small = rect_of(&boxes, &layout, "s", html);
    assert_eq!(small, Rect::new(0.0, 22.5, 10.0, 10.0));

    // A 20-pixel picture alone in the same block: its bottom edge on the
    // baseline at 30, and the font's 10-pixel descent below.
    let html = "<body><div id=w><span id=i></span></div></body>";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    let block = rect_of(&boxes, &layout, "w", html);
    assert!(close(block.size.height, 40.0), "{block:?}");
    let picture = rect_of(&boxes, &layout, "i", html);
    assert_eq!(picture, Rect::new(0.0, 10.0, 20.0, 20.0));

    // In a 16-pixel block the same picture sets the ascent: 20, and 4 below.
    let html = "<body><div><span id=i></span><p id=after>x</p></div></body>";
    let css = "p { margin: 0 } #i { display: inline-block; width: 20px; height: 20px }";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    let after = rect_of(&boxes, &layout, "after", html);
    assert!(close(after.top(), 24.0), "{after:?}");

    // And a block with nothing worth a line in it is no taller for its font.
    let html = "<body><div id=w><span id=s></span></div></body>";
    let (boxes, layout) = lay_out_measured(
        html,
        "#w { font-size: 40px }",
        Size::new(400.0, 300.0),
        &ScaledFont,
    );
    let block = rect_of(&boxes, &layout, "w", html);
    assert!(close(block.size.height, 0.0), "{block:?}");
}

#[test]
fn an_empty_field_stands_where_its_text_will() {
    // A field with nothing typed in it stands on the baseline of the line it
    // would hold, so that it does not drop when somebody types. Half the font
    // size a character, three quarters of it above the baseline: at 16 px and
    // a 20 px line, the strut reaches 14 above and 6 below.
    let css = "#w, input { font-size: 16px; line-height: 20px } \
               input { padding: 5px; border: 0; width: 40px }";

    // Empty and filled lay the line out alike. The field is 30 tall — a line
    // and ten of padding — and its baseline is 19 down, five of padding and
    // the strut's 14. That sets the line's ascent; its 11 below the baseline
    // covers the strut's 6. So the line is the field's own 30, the field at
    // its top, and the text beside it stands on 19: its top at 7.
    for field in ["<input id=f>", "<input id=f value=cd>"] {
        let html = format!("<body><div id=w><span id=s>ab</span>{field}</div></body>");
        let (boxes, layout) = lay_out_measured(&html, css, Size::new(400.0, 300.0), &ScaledFont);
        let block = rect_of(&boxes, &layout, "w", &html);
        assert!(close(block.size.height, 30.0), "{field}: {block:?}");
        let text = rect_of(&boxes, &layout, "s", &html);
        assert_eq!(text, Rect::new(0.0, 7.0, 16.0, 16.0), "{field}");
        let input = rect_of(&boxes, &layout, "f", &html);
        assert_eq!(input, Rect::new(16.0, 0.0, 50.0, 30.0), "{field}");
    }

    // An empty field alone in a block makes a line of its own height.
    let html = "<body><div id=w><input id=f></div></body>";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    assert!(close(rect_of(&boxes, &layout, "w", html).size.height, 30.0));

    // A `<textarea>` and a button with nothing in them have no line, and
    // stand on their bottom margin edge: 30 above the baseline, and the
    // strut's 6 below it, so the line is 36 and the text's top at 18.
    let css = "#w, textarea, button { font-size: 16px; line-height: 20px } \
               textarea, button { padding: 5px; border: 0; width: 40px; height: auto }";
    for control in ["<textarea id=f></textarea>", "<button id=f></button>"] {
        let html = format!("<body><div id=w><span id=s>ab</span>{control}</div></body>");
        let (boxes, layout) = lay_out_measured(&html, css, Size::new(400.0, 300.0), &ScaledFont);
        let block = rect_of(&boxes, &layout, "w", &html);
        assert!(close(block.size.height, 36.0), "{control}: {block:?}");
        let text = rect_of(&boxes, &layout, "s", &html);
        assert_eq!(text, Rect::new(0.0, 18.0, 16.0, 16.0), "{control}");
    }
}

/// The rectangle of the first text box `pick` chooses, or a rectangle no
/// layout produces so that the assertion which asked reports it.
fn text_box_rect(boxes: &BoxTree, layout: &LayoutTree, pick: impl Fn(BoxId) -> bool) -> Rect {
    boxes
        .ids()
        .filter(|id| boxes.get(*id).and_then(alo_box::BoxNode::text).is_some())
        .find(|id| pick(*id))
        .and_then(|id| layout.border_box(id))
        .unwrap_or(Rect::new(f32::NAN, f32::NAN, f32::NAN, f32::NAN))
}

#[test]
fn a_placeholder_adds_nothing_to_its_field_and_stands_where_its_text_will() {
    // ADR 0043 § 3. An auto-width field is sized without its hint — here
    // as narrow as its padding, which is what this engine gives a field with
    // no `size` and nothing in it — so it is as wide with one as without,
    // and the line around it does not move.
    let css = "#w, input { font-size: 16px; line-height: 20px } \
               input { padding: 5px; border: 0; width: auto }";
    let bare = "<body><div id=w><span id=s>ab</span><input id=f></div></body>";
    let hinted = "<body><div id=w><span id=s>ab</span><input id=f placeholder='you@company.eu'></div></body>";
    let (bare_boxes, bare_layout) =
        lay_out_measured(bare, css, Size::new(400.0, 300.0), &ScaledFont);
    let (boxes, layout) = lay_out_measured(hinted, css, Size::new(400.0, 300.0), &ScaledFont);
    for id in ["w", "s", "f"] {
        assert_eq!(
            rect_of(&boxes, &layout, id, hinted),
            rect_of(&bare_boxes, &bare_layout, id, bare),
            "{id}"
        );
    }
    assert_eq!(
        rect_of(&boxes, &layout, "f", hinted),
        Rect::new(16.0, 0.0, 10.0, 30.0)
    );

    // The hint is a run in the field's line, where a value is: its pen at the
    // content edge, 5 in, and the line's top 5 down. Fourteen characters at
    // eight a character, on one line, past the narrow field's edge.
    let hint = text_box_rect(&boxes, &layout, |id| boxes.is_placeholder(id));
    assert_eq!(hint, Rect::new(21.0, 7.0, 112.0, 16.0));
    let valued = "<body><div id=w><span id=s>ab</span><input id=f value=cd></div></body>";
    let fixed = format!("{css} input {{ width: 40px }}");
    let (value_boxes, value_layout) =
        lay_out_measured(valued, &fixed, Size::new(400.0, 300.0), &ScaledFont);
    let value = text_box_rect(&value_boxes, &value_layout, |id| {
        value_boxes.get(id).and_then(alo_box::BoxNode::text) == Some("cd")
    });
    assert_eq!(
        value.origin, hint.origin,
        "a value stands where the hint did"
    );

    // A hint with spaces in it is still one line in a field too narrow for
    // it: it was given no room, and a wrapped hint would be lines the field
    // is not tall enough for.
    let spaced = "<body><input id=f placeholder='a b c d e f'></body>";
    let fixed =
        "input { font-size: 16px; line-height: 20px; padding: 5px; border: 0; width: 20px }";
    let (boxes, layout) = lay_out_measured(spaced, fixed, Size::new(400.0, 300.0), &ScaledFont);
    let field = rect_of(&boxes, &layout, "f", spaced);
    assert_eq!(field.size, Size::new(30.0, 30.0));
    let Some(hint) = boxes.ids().find(|id| boxes.is_placeholder(*id)) else {
        panic!("the field shows its hint");
    };
    assert_eq!(layout.fragments(hint).len(), 1);
}

#[test]
fn a_field_taller_than_its_line_holds_its_text_in_the_middle() {
    // As browsers draw a one-line field: alo's sign-in fields are 46 tall
    // with no padding (queue item 389). Here 50, a 20 line: 15 above it and
    // 15 below, the text's 16 two into it, so its top at 17.
    let css = "#w, input { font-size: 16px; line-height: 20px } \
               input { padding: 0; border: 0; width: 40px; height: 50px }";
    let filled = "<body><div id=w><span id=s>ab</span><input id=f value=cd></div></body>";
    let (boxes, layout) = lay_out_measured(filled, css, Size::new(400.0, 300.0), &ScaledFont);
    let value = text_box_rect(&boxes, &layout, |id| {
        boxes.get(id).and_then(alo_box::BoxNode::text) == Some("cd")
    });
    // The field stands on its text's baseline, 15 + 14 down, so the line's
    // ascent is 29 and the field at its top: the span's text, on the same
    // baseline in the same font, is level with the field's at 17.
    assert_eq!(value, Rect::new(16.0, 17.0, 16.0, 16.0));
    assert_eq!(
        rect_of(&boxes, &layout, "s", filled),
        Rect::new(0.0, 17.0, 16.0, 16.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "f", filled),
        Rect::new(16.0, 0.0, 40.0, 50.0)
    );

    // Empty, it stands on the same line, so nothing beside it moves.
    let empty = "<body><div id=w><span id=s>ab</span><input id=f></div></body>";
    let (empty_boxes, empty_layout) =
        lay_out_measured(empty, css, Size::new(400.0, 300.0), &ScaledFont);
    for id in ["w", "s", "f"] {
        assert_eq!(
            rect_of(&empty_boxes, &empty_layout, id, empty),
            rect_of(&boxes, &layout, id, filled),
            "{id}"
        );
    }

    // A `<textarea>` starts its lines at its top.
    let area = "<body><textarea id=f>cd</textarea></body>";
    let css = "textarea { font-size: 16px; line-height: 20px; padding: 0; border: 0; \
               width: 40px; height: 50px }";
    let (boxes, layout) = lay_out_measured(area, css, Size::new(400.0, 300.0), &ScaledFont);
    let text = text_box_rect(&boxes, &layout, |id| {
        boxes.get(id).and_then(alo_box::BoxNode::text) == Some("cd")
    });
    assert!(close(
        text.origin.y - rect_of(&boxes, &layout, "f", area).origin.y,
        2.0
    ));
}

#[test]
fn line_height_is_room_split_evenly_above_and_below_the_font() {
    // A line as tall as its `line-height`, with half of what that leaves over
    // the font above the letters and half below. Half the font size a
    // character, three quarters of it above the baseline.
    let html = "<body><div id=w><span id=s>ab</span></div><p id=after>x</p></body>";

    // 16 px at 1.5 is 24: the text is 16 of it, four over and four under,
    // so its baseline is at 16 rather than 12.
    let css = "p { margin: 0 } #w { font-size: 16px; line-height: 1.5 }";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    let block = rect_of(&boxes, &layout, "w", html);
    assert!(close(block.size.height, 24.0), "{block:?}");
    let text = rect_of(&boxes, &layout, "s", html);
    assert_eq!(text, Rect::new(0.0, 4.0, 16.0, 16.0));
    let after = rect_of(&boxes, &layout, "after", html);
    assert!(close(after.top(), 24.0), "{after:?}");

    // A number inherits as the number: a 32 px span in that block takes 48,
    // so the line is 48 and the span's text sits 8 down.
    let css = "#w { font-size: 16px; line-height: 1.5 } #s { font-size: 32px }";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    assert!(close(rect_of(&boxes, &layout, "w", html).size.height, 48.0));
    assert_eq!(
        rect_of(&boxes, &layout, "s", html),
        Rect::new(0.0, 8.0, 32.0, 32.0)
    );

    // Smaller than the font is a negative leading: 8 of room for 16 of
    // letters, which reach four past it on each side.
    let css = "#w { font-size: 16px; line-height: 0.5 }";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    assert!(close(rect_of(&boxes, &layout, "w", html).size.height, 8.0));
    assert_eq!(
        rect_of(&boxes, &layout, "s", html),
        Rect::new(0.0, -4.0, 16.0, 16.0)
    );

    // `normal` adds nothing: the line is the font's own 16.
    let (boxes, layout) = lay_out_measured(
        html,
        "#w { font-size: 16px }",
        Size::new(400.0, 300.0),
        &ScaledFont,
    );
    assert!(close(rect_of(&boxes, &layout, "w", html).size.height, 16.0));
}

/// The box of the first piece of text that reads `wanted`, once trimmed.
fn text_box(boxes: &BoxTree, wanted: &str) -> Option<BoxId> {
    let root = boxes.root()?;
    boxes.descendants(root).into_iter().find(|id| {
        boxes
            .get(*id)
            .and_then(|node| node.text())
            .is_some_and(|text| text.trim() == wanted)
    })
}

#[test]
fn text_straight_inside_a_flex_or_grid_container_takes_its_line_height() {
    // CSS wraps such text in an anonymous block, whose one line is as tall as
    // its `line-height` like any other block's. Measured bare, it was only
    // as tall as its font: alo's download buttons were 42.6 where a browser
    // makes them 48.8.
    let html = "<body><a id=b>Get it</a><div id=g>Grid text</div></body>";
    let css = "#b { display: inline-flex; font-size: 16px; line-height: 1.5; padding: 10px } \
               #g { display: grid; font-size: 20px; line-height: 40px }";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);

    // 16 px at 1.5 is a 24 px line, and ten of padding each side: 44.
    let button = rect_of(&boxes, &layout, "b", html);
    assert!(close(button.size.height, 44.0), "{button:?}");
    assert!(
        close(button.size.width, 68.0),
        "six characters of 8 and 20: {button:?}"
    );
    // The letters are 16 of the 24, four under the top of the line.
    let label = text_box(&boxes, "Get it").and_then(|id| layout.border_box(id));
    assert_eq!(
        label,
        Some(Rect::new(
            button.origin.x + 10.0,
            button.origin.y + 14.0,
            48.0,
            16.0
        )),
    );

    // A grid item the same: a 40 px line, 20 px letters ten down in it.
    let grid = rect_of(&boxes, &layout, "g", html);
    assert!(close(grid.size.height, 40.0), "{grid:?}");
    let text = text_box(&boxes, "Grid text").and_then(|id| layout.border_box(id));
    assert_eq!(
        text,
        Some(Rect::new(grid.origin.x, grid.origin.y + 10.0, 90.0, 20.0)),
    );
}

#[test]
fn a_mixed_line_is_as_tall_as_its_leadings_reach() {
    // Three half-leadings on one line. The strut, 16 px at 24, reaches 16
    // above the baseline and 8 below. A 32 px span at `line-height: 1` has
    // none and reaches 24 and 8. An 8 px span at 40 px has 16 each side and
    // reaches 22 and 18. The line is the furthest each way: 24 and 18, 42.
    let html = "<body><div id=w><span id=big>a</span><span id=small>b</span></div></body>";
    let css = "#w { font-size: 16px; line-height: 1.5 } \
               #big { font-size: 32px; line-height: 1 } \
               #small { font-size: 8px; line-height: 40px }";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    let block = rect_of(&boxes, &layout, "w", html);
    assert!(close(block.size.height, 42.0), "{block:?}");
    // Both stand on the baseline at 24: the large letters from the top of
    // the line, the small ones 6 above it.
    assert_eq!(
        rect_of(&boxes, &layout, "big", html),
        Rect::new(0.0, 0.0, 16.0, 32.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "small", html),
        Rect::new(16.0, 18.0, 4.0, 8.0)
    );
}

#[test]
fn an_inline_box_is_aligned_by_its_line_height_not_its_font() {
    // `text-top` puts the top of the box's line height, not of its letters,
    // at the top of its parent's font. An 8 px span at 40 px reaches 22 above
    // its baseline, and the block's 16 px font 12: the span's baseline is 10
    // under the line's, which is at 12, so its letters start at 22 - 6 = 16
    // and the 18 its leading reaches below them make the line 40.
    let html = "<body><div id=w><span id=t>b</span></div></body>";
    let css = "#w { font-size: 16px } \
               #t { font-size: 8px; line-height: 40px; vertical-align: text-top }";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    assert!(close(rect_of(&boxes, &layout, "w", html).size.height, 40.0));
    assert_eq!(
        rect_of(&boxes, &layout, "t", html),
        Rect::new(0.0, 16.0, 4.0, 8.0)
    );
}

#[test]
fn an_inline_block_keeps_its_width_inside_its_own_margins() {
    // "ab cd" at eight pixels a character: one line, 40 wide, whatever its
    // margins are. The box is laid out a second time in the room the line
    // gave it, and that room is its margin box — given only its border box,
    // it would take its margins out again, have 16 pixels for its text, and
    // come back as two lines.
    let html = "<body><div id=w><span id=i>ab cd</span><p>x</p></div></body>";
    let css = "p { margin: 0 } #i { display: inline-block; margin: 0 12px }";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));
    let inline_block = rect_of(&boxes, &layout, "i", html);
    assert_eq!(inline_block, Rect::new(12.0, 0.0, 40.0, 16.0));
}

#[test]
fn aligning_a_line_moves_an_inline_blocks_margin_box() {
    // A 40-wide box with a 20-pixel left margin is 60 on the line: centred in
    // 200 the margin box starts at 70, and the box 20 after it.
    let css = "#w { width: 200px; text-align: center } p { margin: 0 } \
               #i { display: inline-block; width: 40px; height: 20px; \
                    margin-left: 20px }";
    let html = "<body><div id=w><span id=i></span><p>x</p></div></body>";
    let (boxes, layout) = lay_out(html, css, Size::new(400.0, 300.0));
    let found = rect_of(&boxes, &layout, "i", html).left();
    assert!(close(found, 90.0), "at {found}");
}

#[test]
fn an_inline_block_of_text_stands_on_its_last_lines_baseline() {
    // Sixteen-pixel text at eight pixels a character, twelve of it above the
    // baseline and four below, inside and outside the box alike.
    let css = "#i { display: inline-block } p { margin: 0 }";

    // Alone in a block of the same font, a line of text in an inline-block
    // makes a line exactly as tall as the inline-block: its baseline is its
    // text's, and the strut's descent is already under it.
    let html = "<body><div id=w><span id=i>cd</span></div></body>";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    assert_eq!(
        rect_of(&boxes, &layout, "i", html),
        Rect::new(0.0, 0.0, 16.0, 16.0)
    );
    let block = rect_of(&boxes, &layout, "w", html);
    assert!(close(block.size.height, 16.0), "{block:?}");

    // Beside text, it stands on the text's baseline: both at the top of a
    // 16-pixel line rather than the text a descent below the box.
    let html = "<body><div id=w><span id=t>ab </span><span id=i>cd</span></div></body>";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    assert_eq!(
        rect_of(&boxes, &layout, "t", html),
        Rect::new(0.0, 0.0, 24.0, 16.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "i", html),
        Rect::new(24.0, 0.0, 16.0, 16.0)
    );
    assert!(close(rect_of(&boxes, &layout, "w", html).size.height, 16.0));

    // Two lines in it: the *last* one is on the outer line's baseline, 28
    // down, so the text beside it is at 16 and the line is 32.
    let narrow = format!("{css} #i {{ width: 16px }}");
    let html = "<body><div id=w><span id=t>ab </span><span id=i>cd ef</span></div></body>";
    let (boxes, layout) = lay_out_measured(html, &narrow, Size::new(400.0, 300.0), &ScaledFont);
    assert_eq!(
        rect_of(&boxes, &layout, "i", html),
        Rect::new(24.0, 0.0, 16.0, 32.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "t", html),
        Rect::new(0.0, 16.0, 24.0, 16.0)
    );
    assert!(close(rect_of(&boxes, &layout, "w", html).size.height, 32.0));

    // Its margins: the baseline is 6 + 12 below the top of the margin box,
    // and 4 + 10 of it hang below — a 32-pixel line, the box 6 down.
    let spaced = format!("{css} #i {{ margin: 6px 0 10px }}");
    let html = "<body><div id=w><span id=t>ab </span><span id=i>cd</span></div></body>";
    let (boxes, layout) = lay_out_measured(html, &spaced, Size::new(400.0, 300.0), &ScaledFont);
    assert_eq!(
        rect_of(&boxes, &layout, "i", html),
        Rect::new(24.0, 6.0, 16.0, 16.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "t", html),
        Rect::new(0.0, 6.0, 24.0, 16.0)
    );
    assert!(close(rect_of(&boxes, &layout, "w", html).size.height, 32.0));
}

#[test]
fn an_inline_block_with_no_line_or_hidden_overflow_stands_on_its_bottom_edge() {
    // With text and `overflow: hidden`, the box's bottom margin edge is its
    // baseline: 16 down, so the text beside it sits 4 lower and the line is
    // 16 + 4.
    let css = "#i { display: inline-block; overflow: hidden }";
    let html = "<body><div id=w><span id=t>ab </span><span id=i>cd</span></div></body>";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    assert_eq!(
        rect_of(&boxes, &layout, "i", html),
        Rect::new(24.0, 0.0, 16.0, 16.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "t", html),
        Rect::new(0.0, 4.0, 24.0, 16.0)
    );
    assert!(close(rect_of(&boxes, &layout, "w", html).size.height, 20.0));

    // An empty one has no line to stand on at all.
    let css = "#i { display: inline-block; width: 20px; height: 20px }";
    let html = "<body><div id=w><span id=t>ab </span><span id=i></span></div></body>";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    assert_eq!(
        rect_of(&boxes, &layout, "i", html),
        Rect::new(24.0, 0.0, 20.0, 20.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "t", html),
        Rect::new(0.0, 8.0, 24.0, 16.0)
    );
    assert!(close(rect_of(&boxes, &layout, "w", html).size.height, 24.0));
}

/// Item 395: an `inline-flex` box stands on its first item's first baseline
/// (CSS Flexbox 1 § 8.5), not on its bottom edge. alo Sites' menu links are
/// `inline-flex` and 44 tall with their text centred, and standing on their
/// bottom edges put the strut's descent under every one of them.
#[test]
fn an_inline_flex_box_stands_on_its_first_items_baseline() {
    // Sixteen-pixel text, twelve of it above the baseline and four below.
    let html = "<body><div id=w><span id=t>ab </span><a id=i>cd</a></div></body>";

    // Its text centred in 40: the item's line is 12 down, its baseline 24.
    // That is lower in the box than the text's beside it is in its line, so
    // the box's top is the line's, the text beside it 12 down, and the line
    // exactly the box. On its bottom edge the text was at 28 and the line 44.
    let css = "#i { display: inline-flex; height: 40px; align-items: center }";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    assert_eq!(
        rect_of(&boxes, &layout, "i", html),
        Rect::new(24.0, 0.0, 16.0, 40.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "t", html),
        Rect::new(0.0, 12.0, 24.0, 16.0)
    );
    assert!(close(rect_of(&boxes, &layout, "w", html).size.height, 40.0));

    // A column takes its *first* item's baseline, 12 down, not its last
    // line's at 28 as an `inline-block` would: the two boxes start level and
    // the second line hangs below, 32 in all. On its bottom edge: 36.
    let css = "#i { display: inline-flex; flex-direction: column }";
    let html_column =
        "<body><div id=w><span id=t>ab </span><a id=i><b>cd</b><b>ef</b></a></div></body>";
    let (boxes, layout) = lay_out_measured(html_column, css, Size::new(400.0, 300.0), &ScaledFont);
    assert_eq!(
        rect_of(&boxes, &layout, "i", html_column),
        Rect::new(24.0, 0.0, 16.0, 32.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "t", html_column),
        Rect::new(0.0, 0.0, 24.0, 16.0)
    );
    assert!(close(
        rect_of(&boxes, &layout, "w", html_column).size.height,
        32.0
    ));

    // A first item with no line in it has a baseline made from its border
    // box, along its bottom edge: 30 down, though the text after it has one
    // at 12 and the box's own bottom is at 50. The text beside the box
    // stands at 18, and the line is the box.
    let css = "#i { display: inline-flex; height: 50px; align-items: flex-start } \
               #e { width: 10px; height: 30px }";
    let html_empty = "<body><div id=w><span id=t>ab </span><a id=i><b id=e></b>cd</a></div></body>";
    let (boxes, layout) = lay_out_measured(html_empty, css, Size::new(400.0, 300.0), &ScaledFont);
    assert_eq!(
        rect_of(&boxes, &layout, "t", html_empty),
        Rect::new(0.0, 18.0, 24.0, 16.0)
    );
    assert!(close(
        rect_of(&boxes, &layout, "w", html_empty).size.height,
        50.0
    ));

    // One item of two lines: its *first* line's baseline, 12, not its
    // second's at 28, which is where an `inline-block` of the same text
    // stands. (The text is in an element: bare text in a flex container is
    // an item that does not shrink to its widest word yet, item 398.)
    let css = "#i { display: inline-flex; width: 16px }";
    let html_wrapped = "<body><div id=w><span id=t>ab </span><a id=i><b>cd ef</b></a></div></body>";
    let (boxes, layout) = lay_out_measured(html_wrapped, css, Size::new(400.0, 300.0), &ScaledFont);
    assert_eq!(
        rect_of(&boxes, &layout, "i", html_wrapped),
        Rect::new(24.0, 0.0, 16.0, 32.0)
    );
    assert_eq!(
        rect_of(&boxes, &layout, "t", html_wrapped),
        Rect::new(0.0, 0.0, 24.0, 16.0)
    );

    // A first item that is a flex container itself gives its own first
    // item's: the same 12 as the text straight inside. (An inner `inline-flex`
    // is made a block as a flex item; a `display: flex` would be broken
    // around by the box tree, which is item 286.)
    let css = "#i { display: inline-flex } b { display: inline-flex; height: 40px }";
    let html_nested = "<body><div id=w><span id=t>ab </span><a id=i><b>cd</b></a></div></body>";
    let (boxes, layout) = lay_out_measured(html_nested, css, Size::new(400.0, 300.0), &ScaledFont);
    assert_eq!(
        rect_of(&boxes, &layout, "t", html_nested),
        Rect::new(0.0, 0.0, 24.0, 16.0)
    );
    assert!(close(
        rect_of(&boxes, &layout, "w", html_nested).size.height,
        40.0
    ));
}

/// What item 395 does not answer stands where every atomic box used to, on
/// its bottom margin edge: an `inline-flex` box with no item, a row whose
/// items line up on their baselines (item 396), a scroll container, and an
/// `inline-grid`.
#[test]
fn an_inline_flex_box_with_no_item_or_a_baseline_row_stands_on_its_bottom_edge() {
    let html = "<body><div id=w><span id=t>ab </span><a id=i>cd</a></div></body>";
    for css in [
        "#i { display: inline-flex; align-items: baseline }",
        "#i { display: inline-flex; overflow: hidden }",
        "#i { display: inline-grid }",
    ] {
        let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
        assert_eq!(
            rect_of(&boxes, &layout, "t", html),
            Rect::new(0.0, 4.0, 24.0, 16.0),
            "{css}"
        );
        assert!(
            close(rect_of(&boxes, &layout, "w", html).size.height, 20.0),
            "{css}"
        );
    }
    // An item's own `align-self: baseline` is the same refusal.
    let css = "#i { display: inline-flex } b { align-self: baseline }";
    let html_self = "<body><div id=w><span id=t>ab </span><a id=i><b>cd</b></a></div></body>";
    let (boxes, layout) = lay_out_measured(html_self, css, Size::new(400.0, 300.0), &ScaledFont);
    assert_eq!(
        rect_of(&boxes, &layout, "t", html_self),
        Rect::new(0.0, 4.0, 24.0, 16.0)
    );

    // Empty, it has no baseline at all.
    let css = "#i { display: inline-flex; width: 20px; height: 20px }";
    let html_empty = "<body><div id=w><span id=t>ab </span><a id=i></a></div></body>";
    let (boxes, layout) = lay_out_measured(html_empty, css, Size::new(400.0, 300.0), &ScaledFont);
    assert_eq!(
        rect_of(&boxes, &layout, "t", html_empty),
        Rect::new(0.0, 8.0, 24.0, 16.0)
    );
    assert!(close(
        rect_of(&boxes, &layout, "w", html_empty).size.height,
        24.0
    ));
}

#[test]
fn a_button_stands_on_its_labels_baseline() {
    // A button with no edges is its label's line: beside text, both at the
    // top, and alone in a block it adds no descent of the strut's under it.
    // Its label is in a block nobody wrote, so this is also the search for a
    // line going down through a block. (An author's `inline-block` holding
    // blocks cannot show that yet: the box tree breaks it around them as if
    // it were a `<span>`, which is queue item 286.)
    let css = "#b { padding: 0; border: 0; margin: 0 }";
    let html = "<body><div id=w><span id=t>ab </span><button id=b>Go</button></div></body>";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    // The button is one `normal` line tall, 19.2, with its 16-pixel label
    // centred 1.6 down — so the label's baseline is 13.6 down, and the text
    // beside it stands there too. Its bottom edge would have put the text at
    // 7.2 and the line at 23.2.
    assert_eq!(
        rect_of(&boxes, &layout, "b", html),
        Rect::new(24.0, 0.0, 16.0, 19.2)
    );
    let text = rect_of(&boxes, &layout, "t", html);
    assert!(close(text.top(), 1.6), "{text:?}");
    let block = rect_of(&boxes, &layout, "w", html);
    assert!(close(block.size.height, 19.2), "{block:?}");
}

/// Item 279: an outermost `<svg>`'s `width` and `height` attributes are
/// presentation attributes, so a per cent is a share of the containing block
/// and an `em` is the `<svg>`'s own font size — through the cascade, not
/// guessed by the box.
///
/// The `<svg>`s sized by a per cent are made blocks. An inline-level one is
/// an atomic inline, and a per-cent width on one of those is resolved twice —
/// reserved at 200 and drawn at 100 — which is item 284's, not this one's:
/// it is the same for a stylesheet's `width: 50%` on an `inline-block`.
#[test]
fn an_svgs_relative_size_attributes_size_its_box() {
    let css = "#d { width: 400px } svg { font-size: 10px; display: block }";
    let html = "<body><div id=d><svg id=s width=50% height=2em></svg></div></body>";
    let (boxes, layout) = lay_out(html, css, Size::new(800.0, 600.0));
    assert_eq!(
        rect_of(&boxes, &layout, "s", html).size,
        Size::new(200.0, 20.0)
    );

    // A per-cent width and a `viewBox`'s shape: the height follows.
    let html = "<body><div id=d><svg id=s width=50% viewBox='0 0 4 1'></svg></div></body>";
    let (boxes, layout) = lay_out(html, css, Size::new(800.0, 600.0));
    assert_eq!(
        rect_of(&boxes, &layout, "s", html).size,
        Size::new(200.0, 50.0)
    );

    // Inline, as the user-agent sheet leaves it: an `em` on each axis, and
    // the `<svg>`'s own font size rather than its parent's.
    let html = "<body><p id=d><svg id=s width=3em height=2em></svg></p></body>";
    let (boxes, layout) = lay_out(
        html,
        "p { font-size: 20px } svg { font-size: 10px }",
        Size::new(800.0, 600.0),
    );
    assert_eq!(
        rect_of(&boxes, &layout, "s", html).size,
        Size::new(30.0, 20.0)
    );

    // A plain number is still pixels, and an absolute unit converts.
    let html = "<body><svg id=s width=48 height=0.25in></svg></body>";
    let (boxes, layout) = lay_out(html, "", Size::new(800.0, 600.0));
    assert_eq!(
        rect_of(&boxes, &layout, "s", html).size,
        Size::new(48.0, 24.0)
    );
}

/// A presentation attribute is an author declaration of specificity zero, so
/// any stylesheet rule beats it — even `*`, and even for one axis alone.
#[test]
fn a_stylesheet_beats_an_svgs_size_attributes() {
    let html = "<body><div id=d><svg id=s width=50% height=2em></svg></div></body>";
    let sizes = [
        ("* { width: 30px }", Size::new(30.0, 20.0)),
        ("svg { height: 7px }", Size::new(200.0, 7.0)),
        ("#s { width: 25%; height: 3em }", Size::new(100.0, 30.0)),
    ];
    for (rule, expected) in sizes {
        let css = format!("#d {{ width: 400px }} svg {{ font-size: 10px; display: block }} {rule}");
        let (boxes, layout) = lay_out(html, &css, Size::new(800.0, 600.0));
        assert_eq!(rect_of(&boxes, &layout, "s", html).size, expected, "{rule}");
    }
}
/// A size attribute that is not a size is ignored, as an invalid declaration
/// is, and the box is what it would have been without it.
#[test]
fn a_hostile_svg_size_attribute_is_as_though_unwritten() {
    for bad in ["-10", "1e39px", "twelve", "NaN"] {
        let html = format!("<body><svg id=s width='{bad}' height=20></svg></body>");
        let (boxes, layout) = lay_out(&html, "", Size::new(800.0, 600.0));
        assert_eq!(
            rect_of(&boxes, &layout, "s", &html).size,
            Size::new(300.0, 20.0),
            "{bad:?}",
        );
    }
}

/// Queue item 312: a 20-pixel box beside 13-pixel text, under each value of
/// `vertical-align`. The font is [`ScaledFont`], so the text reaches 9.75
/// above its baseline and 3.25 below, and its x-height is 6.5.
///
/// Each row is how far the box's top is above the text's baseline, and how
/// tall the line ends up: everything that moved still counts towards it.
#[test]
fn vertical_align_puts_a_box_where_each_keyword_says() {
    let html = "<body><p id=p><span id=t>x</span><i id=b></i></p></body>";
    let rows = [
        // On the baseline: its bottom edge, 20 over it, with the text's
        // descent below.
        ("baseline", 20.0, 23.25),
        // Its middle at half the x-height over the baseline: 10 + 3.25.
        ("middle", 13.25, 20.0),
        // Its top with the font's top, and its bottom with the font's bottom.
        ("text-top", 9.75, 20.0),
        ("text-bottom", 20.0 - 3.25, 20.0),
        // A fifth of the font size and a pixel down; a third and a pixel up.
        ("sub", 20.0 - 3.6, 20.0),
        (
            "super",
            20.0 + 13.0 / 3.0 + 1.0,
            20.0 + 13.0 / 3.0 + 1.0 + 3.25,
        ),
        // Against the line box's top: the line is the box's 20, and the text,
        // shorter than it, stands at its own ascent from the top.
        ("top", 9.75, 20.0),
        // Against its bottom: the text's descent is the bottom 3.25.
        ("bottom", 20.0 - 3.25, 20.0),
        // Up by a length, down by a negative one, and a percentage of the
        // box's own line height, which is 20.
        ("5px", 25.0, 28.25),
        ("-2px", 18.0, 18.0 + 3.25),
        ("50%", 30.0, 33.25),
    ];
    for (value, top_above_baseline, line) in rows {
        let css = format!(
            "#p {{ font-size: 13px; width: 300px }}
             #b {{ display: inline-block; width: 20px; height: 20px;
                   line-height: 20px; vertical-align: {value} }}"
        );
        let (boxes, layout) = lay_out_measured(html, &css, Size::new(400.0, 300.0), &ScaledFont);
        let text = rect_of(&boxes, &layout, "t", html);
        let placed = rect_of(&boxes, &layout, "b", html);
        let baseline = text.top() + 9.75;
        assert_eq!(placed.size, Size::new(20.0, 20.0), "{value}");
        assert!(
            close(baseline - placed.top(), top_above_baseline),
            "{value}: the box's top is {} above the baseline",
            baseline - placed.top(),
        );
        let height = rect_of(&boxes, &layout, "p", html).size.height;
        assert!(close(height, line), "{value}: a line of {height}");
        assert!(layout.issues().is_empty(), "{value}: {:?}", layout.issues());
    }

    // The midpoint is the point of `middle`: half the x-height over the
    // baseline, exactly.
    let css = "#p { font-size: 13px } #b { display: inline-block; width: 20px; height: 20px;
               vertical-align: middle }";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    let baseline = rect_of(&boxes, &layout, "t", html).top() + 9.75;
    let placed = rect_of(&boxes, &layout, "b", html);
    assert!(close(
        placed.top() + placed.size.height / 2.0,
        baseline - 6.5 / 2.0
    ));
}

/// An inline box's `vertical-align` moves everything inside it, and its own
/// pieces with it; and a value this engine cannot read is recorded and the
/// box stands on the baseline.
#[test]
fn vertical_align_on_an_inline_box_moves_what_is_in_it() {
    let html = "<body><p id=p><span id=a>x</span><sup id=s>2<b id=n>3</b></sup></p></body>";
    let css = "#p { font-size: 16px } #s { font-size: 10px; vertical-align: super }";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    let baseline = rect_of(&boxes, &layout, "a", html).top() + 12.0;
    // Raised a third of the parent's 16 and a pixel: 6.333 over it, and the
    // 10-pixel text reaches 7.5 above its own baseline.
    let raised = baseline - (16.0 / 3.0 + 1.0);
    for inside in ["s", "n"] {
        let rect = rect_of(&boxes, &layout, inside, html);
        assert!(
            close(rect.top(), raised - 7.5),
            "{inside} at {} rather than {}",
            rect.top(),
            raised - 7.5,
        );
    }
    // And the line grows above to hold it: 6.333 + 7.5 is more than 12.
    let height = rect_of(&boxes, &layout, "p", html).size.height;
    assert!(close(height, 16.0 / 3.0 + 1.0 + 7.5 + 4.0), "{height}");

    let css = "#p { font-size: 16px } #s { font-size: 10px; vertical-align: centre }";
    let (boxes, layout) = lay_out_measured(html, css, Size::new(400.0, 300.0), &ScaledFont);
    let baseline = rect_of(&boxes, &layout, "a", html).top() + 12.0;
    assert!(close(
        rect_of(&boxes, &layout, "s", html).top(),
        baseline - 7.5
    ));
    assert!(
        layout
            .issues()
            .iter()
            .any(|issue| issue.source.contains("vertical-align")),
        "{:?}",
        layout.issues(),
    );
}

/// A face whose `x` is 0.53 of its size and whose `0` is 0.6 — unlike half an
/// em, so a test can tell the face was asked — except in the family `Blank`,
/// which has neither.
struct FixedFace;

impl MeasureFace for FixedFace {
    fn face_units(&self, style: &ComputedStyle) -> Option<FaceUnits> {
        if style.get("font-family") == Some("Blank") {
            return None;
        }
        Some(FaceUnits {
            x_height: style.font_size() * 0.53,
            zero_width: style.font_size() * 0.6,
        })
    }
}

/// `ex` and `ch` are the face's `x` and `0` (queue item 320), at the size
/// the element's text is set in; a font size written in them is the
/// parent's, and a face nobody could measure is half an em.
#[test]
fn ex_and_ch_are_the_measured_faces() {
    let html = "<body><div id=a><div id=b></div></div><div id=c></div></body>";
    let css = "body { margin: 0 } \
               #a { font-size: 20px; width: 30ch; height: 10ex } \
               #b { font-size: 2ex; width: 10ch; height: 5ex } \
               #c { font-family: Blank; font-size: 20px; width: 30ch; height: 10ex }";
    let document = parse_document(html);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(css);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve_measured(&document, &sheets, &MediaContext::default(), &FixedFace);
    let boxes = build(&document, &styles);
    let layout = compute(&boxes, &styles, Size::new(800.0, 600.0), &BlockFont);

    let a = rect_of(&boxes, &layout, "a", html).size;
    assert!(close(a.width, 30.0 * 12.0), "{}", a.width);
    assert!(close(a.height, 10.0 * 10.6), "{}", a.height);
    // Two of the parent's `x` is 21.2 px, and its own lengths are of that.
    let b = rect_of(&boxes, &layout, "b", html).size;
    assert!(close(b.width, 10.0 * 21.2 * 0.6), "{}", b.width);
    assert!(close(b.height, 5.0 * 21.2 * 0.53), "{}", b.height);
    let c = rect_of(&boxes, &layout, "c", html).size;
    assert!(close(c.width, 300.0) && close(c.height, 100.0), "{c:?}");
}

/// ADR 0038 § 4: a box's scrolling area is its padding box extended toward
/// its end edges by what its content reaches, and what spills out to the
/// left or above counts nothing.
#[test]
fn a_scrolling_area_counts_what_reaches_right_and_down_and_not_left_or_up() {
    let html = "<div id=b><div id=wide></div><div id=left></div><div id=up></div></div>";
    let css = "#b { position: relative; width: 100px; height: 50px; padding: 10px; } \
               #wide { width: 300px; height: 80px; } \
               #left { position: absolute; left: -500px; top: 0; width: 40px; height: 10px; } \
               #up { position: absolute; left: 0; top: -900px; width: 10px; height: 10px; }";
    let (boxes, layout) = lay_out(html, css, Size::new(800.0, 600.0));
    let geometry = geometry_of(&boxes, &layout, "b", html);
    assert!(
        close(geometry.padding_box().size.width, 120.0),
        "{geometry:?}"
    );
    // `#wide` starts at the padding box's left plus its 10 px of padding and
    // is 300 wide; 80 tall below 10 px of padding. Nothing reaches further.
    assert!(close(geometry.reach.width, 310.0), "{geometry:?}");
    assert!(close(geometry.reach.height, 90.0), "{geometry:?}");
    let area = geometry.scrolling_area();
    assert!(close(area.width, 310.0), "{area:?}");
    assert!(close(area.height, 90.0), "{area:?}");
    // The absolutely positioned boxes far to the left and far above are in
    // `scrollable`, which a scroll cannot reach, and not in the area.
    assert!(geometry.scrollable.width > 800.0, "{geometry:?}");
    assert!(geometry.scrollable.height > 900.0, "{geometry:?}");
}
