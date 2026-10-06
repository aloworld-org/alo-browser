/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The walk from an `<svg>` to a drawing, end to end: in numbers, and against
//! input written to make it spend. Strokes are at the end.

use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::{Document, NodeId, parse_document};
use alo_paint::{DrawingItem, FillRule};
use alo_style::{Origin, SourcedSheet, StyleTree, USER_AGENT_STYLE_SHEET};
use alo_svg::bounds::{
    DEEPEST, MOST_DASH_LENGTHS, MOST_DASHES, MOST_ELEMENTS, MOST_GROUPS_DEEP, MOST_PATH_SEGMENTS,
    MOST_SEGMENTS,
};
use alo_svg::walk::MOST_GROUPS;
use alo_svg::{Drawn, draw};
use alo_value::Rgba;

fn styled(markup: &str, css: &str) -> (Document, StyleTree) {
    let document = parse_document(markup);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(css);
    let styles = alo_style::resolve(
        &document,
        &[
            SourcedSheet::new(Origin::UserAgent, &agent),
            SourcedSheet::new(Origin::Author, &author),
        ],
        &MediaContext::default(),
    );
    (document, styles)
}

/// The first `<svg>`, if the markup has one.
fn outermost(document: &Document) -> Option<NodeId> {
    document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| &*element.name.local == "svg")
    })
}

/// What the first `<svg>` draws — nothing at all, if there is none, which
/// every test below would notice.
fn drawn_with(markup: &str, css: &str, size: (f32, f32)) -> Drawn {
    let (document, styles) = styled(markup, css);
    outermost(&document).map_or_else(Drawn::default, |svg| draw(&document, &styles, svg, size))
}

fn drawn(markup: &str, size: (f32, f32)) -> Drawn {
    drawn_with(markup, "", size)
}

/// A fill as a test reads it: its bounds, its colour and its rule.
type Seen = ((f32, f32, f32, f32), Rgba, FillRule);

/// Each fill's bounds and colour, in order.
fn fills(drawn: &Drawn) -> Vec<Seen> {
    drawn
        .drawing
        .items()
        .iter()
        .filter_map(|item| match item {
            DrawingItem::Fill { path, color, rule } => {
                Some((path.bounds().unwrap_or_default(), *color, *rule))
            }
            _ => None,
        })
        .collect()
}

fn rounded(bounds: (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
    let round = |value: f32| (value * 1000.0).round() / 1000.0;
    (
        round(bounds.0),
        round(bounds.1),
        round(bounds.2),
        round(bounds.3),
    )
}

#[test]
fn a_viewbox_scales_user_space_into_the_box() {
    // A 24-unit viewBox in a 48-pixel box: everything doubles.
    let drawn = drawn(
        r#"<svg viewBox="0 0 24 24"><rect x="2" y="4" width="10" height="6"/></svg>"#,
        (48.0, 48.0),
    );
    assert_eq!(
        fills(&drawn),
        vec![((4.0, 8.0, 24.0, 20.0), Rgba::BLACK, FillRule::NonZero)]
    );
    assert!(drawn.issues.is_empty(), "{:?}", drawn.issues);
}

#[test]
fn transforms_compose_from_the_shape_out_to_the_viewbox() {
    // The rect is moved by its own transform, then by its group's, then
    // doubled by the viewBox.
    let moved = drawn(
        r#"<svg viewBox="0 0 50 50"><g transform="translate(10 0)"><rect transform="translate(0 5)" width="5" height="5"/></g></svg>"#,
        (100.0, 100.0),
    );
    assert_eq!(
        fills(&moved).first().map(|fill| fill.0),
        Some((20.0, 10.0, 30.0, 20.0))
    );

    let turned = drawn(
        r#"<svg><rect transform="rotate(90)" width="10" height="2"/></svg>"#,
        (20.0, 20.0),
    );
    assert_eq!(
        fills(&turned).first().map(|fill| rounded(fill.0)),
        Some((-2.0, 0.0, 0.0, 10.0)),
        "turned clockwise about the origin",
    );
}

#[test]
fn shapes_are_drawn_in_document_order_with_their_own_fills() {
    let drawn = drawn(
        r##"<svg>
            <circle cx="5" cy="5" r="5" fill="#ff0000"/>
            <ellipse cx="20" cy="5" rx="5" ry="3" fill="rgb(0 0 255)" fill-opacity="0.5"/>
            <polygon points="0,20 10,20 5,30" fill-rule="evenodd"/>
            <polyline points="20,20 30,20 25,30" fill="none"/>
            <line x1="0" y1="0" x2="30" y2="30"/>
        </svg>"##,
        (40.0, 40.0),
    );
    let fills = fills(&drawn);
    assert_eq!(fills.len(), 3, "a fill of none and a line draw nothing");
    assert_eq!(
        fills.first().map(|fill| fill.1),
        Some(Rgba::from_rgba8(255, 0, 0, 255))
    );
    assert_eq!(
        fills.get(1).map(|fill| (fill.0, fill.1.alpha)),
        Some(((15.0, 2.0, 25.0, 8.0), 0.5))
    );
    assert_eq!(fills.get(2).map(|fill| fill.2), Some(FillRule::EvenOdd));
}

#[test]
fn opacity_on_a_group_is_a_group_and_on_a_shape_is_its_alpha() {
    let drawn = drawn(
        r#"<svg><g opacity="0.5"><rect width="4" height="4"/><rect x="2" width="4" height="4"/></g><rect y="8" width="4" height="4" opacity="25%"/></svg>"#,
        (20.0, 20.0),
    );
    let kinds: Vec<String> = drawn
        .drawing
        .items()
        .iter()
        .map(|item| match item {
            DrawingItem::PushGroup { opacity } => format!("group {opacity}"),
            DrawingItem::PopGroup => "ungroup".to_owned(),
            DrawingItem::Fill { color, .. } => format!("fill {}", color.alpha),
        })
        .collect();
    assert_eq!(
        kinds,
        vec!["group 0.5", "fill 1", "fill 1", "ungroup", "fill 0.25"],
        "two overlapping shapes in a faded group are faded together, once",
    );
}

#[test]
fn an_empty_or_invisible_group_costs_nothing() {
    let drawn = drawn(
        r#"<svg><g opacity="0.5"><g opacity="0.5"></g></g><g opacity="0"><rect width="4" height="4"/></g></svg>"#,
        (20.0, 20.0),
    );
    assert!(drawn.drawing.is_empty(), "{:?}", drawn.drawing);
}

#[test]
fn display_none_hides_a_subtree_and_visibility_hides_only_what_says_so() {
    let drawn = drawn(
        r#"<svg>
            <g display="none"><rect width="4" height="4"/></g>
            <g visibility="hidden"><rect width="4" height="4"/><rect x="5" width="4" height="4" visibility="visible"/></g>
        </svg>"#,
        (20.0, 20.0),
    );
    assert_eq!(
        fills(&drawn).iter().map(|fill| fill.0).collect::<Vec<_>>(),
        vec![(5.0, 0.0, 9.0, 4.0)]
    );
}

#[test]
fn a_stylesheet_and_current_color_reach_inside_the_svg() {
    let drawn = drawn_with(
        r#"<p><svg class=icon><rect width="4" height="4" fill="red"/><rect class=accent x="5" width="4" height="4"/></svg></p>"#,
        "p { color: rgb(0 128 0) } .icon { fill: currentColor } rect { fill: rgb(1 2 3) } .accent { fill: inherit }",
        (20.0, 20.0),
    );
    assert_eq!(
        fills(&drawn).iter().map(|fill| fill.1).collect::<Vec<_>>(),
        vec![
            Rgba::from_rgba8(1, 2, 3, 255),
            Rgba::from_rgba8(0, 128, 0, 255)
        ],
        "the sheet beats the attribute; `inherit` takes the <svg>'s currentColor",
    );
}

#[test]
fn what_is_not_drawn_yet_is_recorded_and_what_never_draws_is_not() {
    let drawn = drawn(
        r#"<svg><title>Name</title><desc>About</desc><defs><rect width="9" height="9"/></defs>
            <path d="M0 0h4v4z"/><use href="x"/><text>hi</text><svg></svg><image/><foreignObject/>
            <animate/><blink/><rect width="1" height="1" clip-path="url(x)" transform="spin(3)"/></svg>"#,
        (20.0, 20.0),
    );
    assert_eq!(
        fills(&drawn).len(),
        2,
        "the path, and the last rect untransformed"
    );
    let said = drawn.issues.join("\n");
    for expected in [
        "item 274",
        "item 276",
        "item 278",
        "a picture inside a picture",
        "HTML inside SVG",
        "SMIL",
        "<blink>",
        "clip-path",
        "spin(3)",
    ] {
        assert!(said.contains(expected), "{expected} missing from:\n{said}");
    }
    assert_eq!(drawn.issues.len(), 9, "{said}");
}

#[test]
fn a_path_is_filled_in_the_boxs_coordinates_by_its_rule() {
    // A 10-unit square with a 4-unit square hole, in a viewBox drawn doubled.
    let markup = r#"<svg viewBox="0 0 20 20"><path fill-rule="evenodd" fill="rgb(1 2 3)"
        d="M 2 2 h 10 v 10 h -10 z m 3 3 h 4 v 4 h -4 z"/></svg>"#;
    let drawn = drawn(markup, (40.0, 40.0));
    assert_eq!(
        fills(&drawn)
            .into_iter()
            .map(|(bounds, color, rule)| (rounded(bounds), color, rule))
            .collect::<Vec<_>>(),
        vec![(
            (4.0, 4.0, 24.0, 24.0),
            Rgba::from_rgba8(1, 2, 3, 255),
            FillRule::EvenOdd
        )]
    );
    assert!(drawn.issues.is_empty(), "{:?}", drawn.issues);
}

#[test]
fn a_path_is_drawn_up_to_its_first_error_and_says_so() {
    let markup = r#"<svg><path d="M 0 0 L 10 0 L 10 10 L oops 0"/></svg>"#;
    let cut = drawn(markup, (20.0, 20.0));
    assert_eq!(
        fills(&cut)
            .into_iter()
            .map(|fill| rounded(fill.0))
            .collect::<Vec<_>>(),
        vec![(0.0, 0.0, 10.0, 10.0)]
    );
    assert_eq!(cut.issues.len(), 1);
    assert!(
        cut.issues
            .iter()
            .any(|issue| issue.contains("first error, at byte 23")),
        "{:?}",
        cut.issues
    );

    // Nothing drawn before the error, or nothing but moves, fills nothing.
    for nothing in [
        r#"<path d="L 10 10"/>"#,
        r#"<path d="M 1 1 M 2 2 Z"/>"#,
        r#"<path d="none"/>"#,
        "<path/>",
    ] {
        let empty = drawn(&format!("<svg>{nothing}</svg>"), (20.0, 20.0));
        assert!(empty.drawing.is_empty(), "{nothing}");
    }
}

#[test]
fn path_data_past_its_bound_refuses_the_drawing_and_data_at_it_does_not() {
    let at = format!(
        r#"<svg><path d="M0 0{}"/></svg>"#,
        " 1 1".repeat(MOST_PATH_SEGMENTS - 1)
    );
    assert_eq!(fills(&drawn(&at, (20.0, 20.0))).len(), 1);

    let past = format!(
        r#"<svg><rect width="5" height="5"/><path d="M0 0{}"/></svg>"#,
        " 1 1".repeat(MOST_PATH_SEGMENTS)
    );
    let drawn = drawn(&past, (20.0, 20.0));
    assert!(drawn.drawing.is_empty(), "refused whole, not cut short");
    assert!(
        drawn
            .issues
            .iter()
            .any(|issue| issue.contains("was not drawn at all")),
        "{:?}",
        drawn.issues
    );
}

#[test]
fn paths_count_toward_the_drawings_own_bound() {
    // Four paths each at their own bound pass the drawing's.
    let path = format!(
        r#"<path d="M0 0{}"/>"#,
        " 1 1".repeat(MOST_PATH_SEGMENTS - 1)
    );
    let under = MOST_SEGMENTS / MOST_PATH_SEGMENTS;
    let fits = format!("<svg>{}</svg>", path.repeat(under));
    assert_eq!(fills(&drawn(&fits, (20.0, 20.0))).len(), under);
    let over = format!("<svg>{}</svg>", path.repeat(under + 1));
    assert!(drawn(&over, (20.0, 20.0)).drawing.is_empty());
}

#[test]
fn a_viewbox_with_no_area_and_a_box_with_no_size_draw_nothing() {
    let markup = r#"<svg viewBox="0 0 0 10"><rect width="4" height="4"/></svg>"#;
    assert!(drawn(markup, (20.0, 20.0)).drawing.is_empty());
    let markup = r#"<svg><rect width="4" height="4"/></svg>"#;
    assert!(drawn(markup, (0.0, 20.0)).drawing.is_empty());
    assert!(drawn(markup, (f32::NAN, 20.0)).drawing.is_empty());
}

#[test]
fn a_bad_preserve_aspect_ratio_is_the_default_and_says_so() {
    let drawn = drawn(
        r#"<svg viewBox="0 0 10 10" preserveAspectRatio="sideways"><rect width="10" height="10"/></svg>"#,
        (40.0, 20.0),
    );
    assert_eq!(
        fills(&drawn).first().map(|fill| fill.0),
        Some((10.0, 0.0, 30.0, 20.0)),
        "xMidYMid meet: centred across",
    );
    assert_eq!(drawn.issues.len(), 1);
}

fn refused(drawn: &Drawn, why: &str) {
    assert!(drawn.drawing.is_empty(), "refused whole, never cut short");
    assert!(
        drawn
            .issues
            .last()
            .is_some_and(|issue| issue.contains("not drawn at all") && issue.contains(why)),
        "{:?}",
        drawn.issues,
    );
}

#[test]
fn elements_are_bounded_at_their_edge() {
    let at = format!("<svg>{}</svg>", "<g></g>".repeat(MOST_ELEMENTS));
    assert!(drawn(&at, (10.0, 10.0)).issues.is_empty());
    let past = format!(
        r#"<svg><rect width="1" height="1"/>{}</svg>"#,
        "<g></g>".repeat(MOST_ELEMENTS)
    );
    refused(&drawn(&past, (10.0, 10.0)), "elements");
}

#[test]
fn nesting_is_bounded_at_its_edge() {
    // The outermost <svg> is depth zero, so DEEPEST groups fit inside it.
    let at = format!(
        r#"<svg>{}<rect width="1" height="1"/>{}</svg>"#,
        "<g>".repeat(DEEPEST - 1),
        "</g>".repeat(DEEPEST - 1)
    );
    let fine = drawn(&at, (10.0, 10.0));
    assert_eq!(fills(&fine).len(), 1, "{:?}", fine.issues);
    let past = format!(
        r#"<svg>{}<rect width="1" height="1"/>{}</svg>"#,
        "<g>".repeat(DEEPEST),
        "</g>".repeat(DEEPEST)
    );
    refused(&drawn(&past, (10.0, 10.0)), "nested");
}

#[test]
fn groups_are_bounded_in_depth_and_in_number() {
    let deep = |levels: usize| {
        format!(
            r#"<svg>{}<rect width="1" height="1"/>{}</svg>"#,
            r#"<g opacity="0.9">"#.repeat(levels),
            "</g>".repeat(levels)
        )
    };
    assert_eq!(
        fills(&drawn(&deep(MOST_GROUPS_DEEP), (10.0, 10.0))).len(),
        1
    );
    refused(&drawn(&deep(MOST_GROUPS_DEEP + 1), (10.0, 10.0)), "deep");

    let many = |count: usize| {
        format!(
            "<svg>{}</svg>",
            r#"<g opacity="0.5"><rect width="1" height="1"/></g>"#.repeat(count)
        )
    };
    assert_eq!(
        fills(&drawn(&many(MOST_GROUPS), (10.0, 10.0))).len(),
        MOST_GROUPS
    );
    refused(&drawn(&many(MOST_GROUPS + 1), (10.0, 10.0)), "groups");
}

#[test]
fn segments_are_bounded_over_the_whole_drawing() {
    // Four polylines of a quarter of the bound each: exactly at it. (A bound
    // reached with rects would meet the element bound first.)
    let points = "0 0 1 1 ".repeat(MOST_SEGMENTS / 8);
    let one = format!(r#"<polyline points="{points}"/>"#);
    let at = format!("<svg>{}</svg>", one.repeat(4));
    assert_eq!(fills(&drawn(&at, (10.0, 10.0))).len(), 4);
    let past = format!(
        r#"<svg>{}<rect width="1" height="1"/></svg>"#,
        one.repeat(4)
    );
    refused(&drawn(&past, (10.0, 10.0)), "segments");
}

#[test]
fn hostile_numbers_never_panic() {
    for markup in [
        r#"<svg viewBox="0 0 1e-38 1e-38"><rect width="1e38" height="1e38"/></svg>"#,
        r#"<svg><rect width="3.4e38" height="3.4e38" rx="3.4e38"/></svg>"#,
        r#"<svg><circle r="1e99999"/></svg>"#,
        r#"<svg><polygon points="1e38,1e38 -1e38,-1e38 1e38,-1e38"/></svg>"#,
        r#"<svg><g transform="scale(1e38) scale(1e38)"><rect width="1" height="1"/></g></svg>"#,
        r#"<svg><g transform="matrix(0 0 0 0 0 0)"><rect width="1" height="1"/></g></svg>"#,
        r#"<svg><rect width="-0" height="NaN"/></svg>"#,
        r#"<svg><ellipse rx="auto" ry="-5"/></svg>"#,
        r#"<svg preserveAspectRatio=""><rect width="1" height="1"/></svg>"#,
    ] {
        let _ = drawn(markup, (10.0, 10.0));
    }
}

// Strokes (item 273).

/// The offline screen's terracotta, `#e76f51`.
fn terracotta() -> Rgba {
    Rgba::from_rgba8(0xe7, 0x6f, 0x51, 255)
}

#[test]
fn a_stroke_is_drawn_after_the_fill_as_the_outline_it_covers() {
    let drawn = drawn(
        r#"<svg><rect x="4" y="4" width="10" height="10" fill="blue" stroke="red" stroke-width="2"/></svg>"#,
        (20.0, 20.0),
    );
    assert_eq!(
        fills(&drawn),
        vec![
            (
                (4.0, 4.0, 14.0, 14.0),
                Rgba::from_rgba8(0, 0, 255, 255),
                FillRule::NonZero
            ),
            (
                (3.0, 3.0, 15.0, 15.0),
                Rgba::from_rgba8(255, 0, 0, 255),
                FillRule::NonZero
            ),
        ],
        "the fill, then a ring one unit either side of its edge",
    );
    assert!(drawn.issues.is_empty(), "{:?}", drawn.issues);
}

#[test]
fn a_line_is_stroked_and_never_filled() {
    let drawn = drawn(
        r#"<svg><line x1="2" y1="5" x2="18" y2="5" stroke="black" stroke-width="4" stroke-linecap="square"/></svg>"#,
        (20.0, 20.0),
    );
    let seen = fills(&drawn);
    assert_eq!(seen.len(), 1, "only the stroke");
    assert_eq!(rounded(seen[0].0), (0.0, 3.0, 20.0, 7.0));
}

#[test]
fn a_stroke_is_as_wide_as_the_viewbox_scales_it() {
    // The offline screen's arithmetic: two units of a 24-unit viewBox drawn
    // at 48 pixels is four pixels.
    let drawn = drawn(
        r#"<svg viewBox="0 0 24 24"><path d="M4 12H20" fill="none" stroke="black" stroke-width="2"/></svg>"#,
        (48.0, 48.0),
    );
    assert_eq!(
        fills(&drawn)
            .into_iter()
            .map(|fill| rounded(fill.0))
            .collect::<Vec<_>>(),
        vec![(8.0, 22.0, 40.0, 26.0)],
    );
}

#[test]
fn a_stroke_is_outlined_before_it_is_transformed_so_a_squash_squashes_it() {
    // Scaled twice across and not down: a horizontal stroke keeps its height
    // and a vertical one doubles its width.
    let drawn = drawn(
        r#"<svg><g transform="scale(2 1)" fill="none" stroke="black" stroke-width="2">
            <path d="M1 2H5"/><path d="M8 4V10"/></g></svg>"#,
        (40.0, 20.0),
    );
    assert_eq!(
        fills(&drawn)
            .into_iter()
            .map(|fill| rounded(fill.0))
            .collect::<Vec<_>>(),
        vec![(2.0, 1.0, 10.0, 3.0), (14.0, 4.0, 18.0, 10.0)],
    );
}

#[test]
fn a_shapes_opacity_over_a_fill_and_a_stroke_is_one_group() {
    let drawn = drawn(
        r#"<svg><rect width="10" height="10" fill="blue" stroke="red" opacity="0.5"/>
            <rect width="10" height="10" fill="none" stroke="red" stroke-opacity="0.5" opacity="0.5"/></svg>"#,
        (20.0, 20.0),
    );
    let items = drawn.drawing.items();
    assert_eq!(
        items.first(),
        Some(&DrawingItem::PushGroup { opacity: 0.5 })
    );
    assert!(matches!(items.get(3), Some(DrawingItem::PopGroup)));
    // Inside the group, both at full strength; after it, a stroke alone
    // folds both opacities into its colour.
    let alphas: Vec<f32> = fills(&drawn).into_iter().map(|fill| fill.1.alpha).collect();
    assert_eq!(alphas, vec![1.0, 1.0, 0.25]);
    assert_eq!(items.len(), 5);
}

#[test]
fn a_subpath_of_no_length_is_a_dot_under_a_round_cap_and_nothing_under_a_butt() {
    let dot = drawn(
        r#"<svg><path d="M5 5 Z" stroke="black" stroke-width="4" stroke-linecap="round"/></svg>"#,
        (20.0, 20.0),
    );
    assert_eq!(
        fills(&dot)
            .into_iter()
            .map(|fill| rounded(fill.0))
            .collect::<Vec<_>>(),
        vec![(3.0, 3.0, 7.0, 7.0)],
    );
    let nothing = drawn(
        r#"<svg><path d="M5 5 Z" stroke="black" stroke-width="4"/></svg>"#,
        (20.0, 20.0),
    );
    assert!(nothing.drawing.is_empty());
}

#[test]
fn the_offline_screens_hand_is_four_terracotta_strokes_inside_its_box() {
    // alo-workplace/web/public/offline.html's <svg>, as it writes it.
    let drawn = drawn(
        r##"<svg viewBox="0 0 24 24" fill="none" stroke="#e76f51" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M18 11V6a2 2 0 0 0-2-2a2 2 0 0 0-2 2" />
        <path d="M14 10V4a2 2 0 0 0-2-2a2 2 0 0 0-2 2v2" />
        <path d="M10 10.5V6a2 2 0 0 0-2-2a2 2 0 0 0-2 2v8" />
        <path d="M18 8a2 2 0 1 1 4 0v6a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.86-5.99-2.34l-3.6-3.6a2 2 0 0 1 2.83-2.82L7 15" />
      </svg>"##,
        (56.0, 56.0),
    );
    assert!(drawn.issues.is_empty(), "{:?}", drawn.issues);
    let seen = fills(&drawn);
    assert_eq!(seen.len(), 4, "four strokes and no fill");
    for (bounds, color, rule) in &seen {
        assert_eq!((*color, *rule), (terracotta(), FillRule::NonZero));
        assert!(
            bounds.0 >= 0.0 && bounds.1 >= 0.0 && bounds.2 <= 56.0 && bounds.3 <= 56.0,
            "{bounds:?}"
        );
    }
    // The first finger: from x = 14 − 1 to 18 + 1 units and y = 2 − 1… (the
    // top of its arc is y = 4, less a unit of stroke) to 11 + 1, at 56/24 a
    // unit. Round caps reach a unit past each end.
    let unit = 56.0 / 24.0;
    let first = seen[0].0;
    for (found, wanted) in [
        (first.0, 13.0 * unit),
        (first.1, 3.0 * unit),
        (first.2, 19.0 * unit),
        (first.3, 12.0 * unit),
    ] {
        assert!((found - wanted).abs() < 0.05, "{first:?}");
    }
}

#[test]
fn dashes_cut_the_stroke_and_are_counted_before_they_are_cut() {
    let dashed = drawn(
        r#"<svg><path d="M0 5H20" fill="none" stroke="black" stroke-dasharray="4 2"/></svg>"#,
        (20.0, 10.0),
    );
    assert_eq!(fills(&dashed).len(), 1, "one outline holding every dash");
    let subpaths = |drawn: &Drawn| match drawn.drawing.items().first() {
        Some(DrawingItem::Fill { path, .. }) => path
            .segments()
            .iter()
            .filter(|segment| matches!(segment, alo_paint::Segment::MoveTo(_)))
            .count(),
        _ => 0,
    };
    assert_eq!(subpaths(&dashed), 4, "dashes at 0, 6, 12 and 18");
}

#[test]
fn dashes_past_their_bound_refuse_the_drawing_and_dashes_at_it_do_not() {
    // A line of 2n units with dashes of one on, one off is counted as n + 1.
    let line = |length: usize| {
        format!(
            r#"<svg fill="none"><path d="M0 0H{length}" stroke="black" stroke-dasharray="1"/></svg>"#
        )
    };
    let at = drawn(&line(2 * (MOST_DASHES - 1)), (10.0, 10.0));
    assert_eq!(fills(&at).len(), 1, "{:?}", at.issues);
    refused(&drawn(&line(2 * MOST_DASHES), (10.0, 10.0)), "dashes");
    // And a dash too small to see along a short path is millions of dashes.
    refused(
        &drawn(
            r#"<svg><path d="M0 0H100" stroke="black" stroke-dasharray="0.00001"/></svg>"#,
            (10.0, 10.0),
        ),
        "dashes",
    );
}

#[test]
fn a_dash_list_longer_than_its_bound_refuses_the_drawing() {
    let at = vec!["1"; MOST_DASH_LENGTHS].join(",");
    let fine = drawn(
        &format!(
            r#"<svg fill="none"><path d="M0 0H10" stroke="black" stroke-dasharray="{at}"/></svg>"#
        ),
        (10.0, 10.0),
    );
    assert_eq!(fills(&fine).len(), 1);
    let past = vec!["1"; MOST_DASH_LENGTHS + 1].join(",");
    refused(
        &drawn(
            &format!(r#"<svg><path d="M0 0H10" stroke="black" stroke-dasharray="{past}"/></svg>"#),
            (10.0, 10.0),
        ),
        "stroke-dasharray",
    );
}

#[test]
fn an_outline_counts_toward_the_drawings_bound() {
    // Four paths at their own bound fill the drawing's exactly when filled;
    // stroking one of them as well adds its outline, which is past it.
    let path = format!(
        r#"<path d="M0 0{}"/>"#,
        " 1 1".repeat(MOST_PATH_SEGMENTS - 1)
    );
    let under = MOST_SEGMENTS / MOST_PATH_SEGMENTS;
    let fits = format!("<svg>{}</svg>", path.repeat(under));
    assert_eq!(fills(&drawn(&fits, (20.0, 20.0))).len(), under);
    let stroked = format!(r#"<svg stroke="black">{}</svg>"#, path.repeat(under));
    refused(&drawn(&stroked, (20.0, 20.0)), "segments");
}

#[test]
fn a_stroke_value_in_error_is_its_initial_value_and_says_so() {
    let drawn = drawn_with(
        r#"<svg fill="none"><path d="M2 5H18" stroke="black"/></svg>"#,
        "path { stroke-width: -4; stroke-linecap: flat }",
        (20.0, 10.0),
    );
    assert_eq!(
        fills(&drawn)
            .into_iter()
            .map(|fill| rounded(fill.0))
            .collect::<Vec<_>>(),
        vec![(2.0, 4.5, 18.0, 5.5)],
        "one unit wide, butt ends",
    );
    assert_eq!(drawn.issues.len(), 2, "{:?}", drawn.issues);
}

#[test]
fn hostile_strokes_never_panic() {
    for markup in [
        r#"<svg><path d="M0 0H10" stroke="black" stroke-width="3e38"/></svg>"#,
        r#"<svg><path d="M0 0L1e38 1e38" stroke="black" stroke-linecap="round"/></svg>"#,
        r#"<svg><path d="M0 0H10" stroke="black" stroke-width="1e-38"/></svg>"#,
        r#"<svg><path d="M0 0H10V10" stroke="black" stroke-miterlimit="3e38"/></svg>"#,
        r#"<svg><path d="M0 0H10" stroke="black" stroke-dasharray="1" stroke-dashoffset="3e38"/></svg>"#,
        r#"<svg><path d="M0 0H10" stroke="black" stroke-dasharray="1" stroke-dashoffset="-3e38"/></svg>"#,
        r#"<svg><path d="M0 0H10" stroke="black" stroke-dasharray="3e38 3e38"/></svg>"#,
        r#"<svg><path d="M0 0H10" stroke="black" stroke-dasharray="1e-45 1e-45"/></svg>"#,
        r#"<svg><path d="M0 0H10" stroke="black" stroke-dasharray="0 1" stroke-linecap="round"/></svg>"#,
        r#"<svg><g transform="scale(1e38)"><path d="M0 0H10" stroke="black"/></g></svg>"#,
        r#"<svg><g transform="scale(1e-38)"><path d="M0 0H10" stroke="black"/></g></svg>"#,
        r#"<svg><g transform="matrix(0 0 0 0 0 0)"><path d="M0 0H10" stroke="black"/></g></svg>"#,
        r#"<svg><path d="M0 0 Z M0 0 Z M0 0 Z" stroke="black" stroke-linecap="square"/></svg>"#,
        r#"<svg><path d="M0 0 A 1e38 1e38 0 1 1 1 1" stroke="black"/></svg>"#,
        r#"<svg><line x2="1e99999" stroke="black"/></svg>"#,
    ] {
        let _ = drawn(markup, (10.0, 10.0));
    }
}
