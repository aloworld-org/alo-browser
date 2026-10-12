/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A box is laid out on a grid of 1/64 px (ADR 0054), in numbers.
//!
//! Every number below is Chrome 154's, as ADR 0054 records it, in
//! sixty-fourths of a pixel: the probes measured `getBoundingClientRect()`
//! and multiplied by 64. Each test is one row or one table of that ADR, so a
//! change that moves a box off the grid says which rule it broke.
//!
//! What ADR 0054 measured and this engine does not yet give is not asserted
//! here as if it did: flexible items' exact shares and a grid that sizes its
//! columns twice are queue item 499's, and the only thing said about them
//! below is that their edges are on the grid.

use alo_box::{BoxId, BoxTree, build};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{LayoutTree, MeasureText, NoText, Rect, ScaledFont, Size, compute};
use alo_style::{Origin, SourcedSheet, StyleTree, USER_AGENT_STYLE_SHEET, resolve};

/// A box's left, top, width and height, in sixty-fourths of a pixel.
type Units = [f32; 4];

fn lay_out(html: &str, css: &str, measure: &impl MeasureText) -> (BoxTree, LayoutTree) {
    let document = parse_document(html);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    // The probes put every arrangement in a stage at (0, 0); a page with no
    // margin is the same thing.
    let author = parse_stylesheet(&format!("body {{ margin: 0 }}\n{css}"));
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles: StyleTree = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build(&document, &styles);
    let layout = compute(&boxes, &styles, Size::new(800.0, 600.0), measure);
    (boxes, layout)
}

/// The border box of the element with this `id`, in units, or `None`.
fn units_of(html: &str, css: &str, wanted: &str) -> Option<Units> {
    units_measured(html, css, wanted, &NoText)
}

fn units_measured(
    html: &str,
    css: &str,
    wanted: &str,
    measure: &impl MeasureText,
) -> Option<Units> {
    let (boxes, layout) = lay_out(html, css, measure);
    let rect = rect_of(&boxes, &layout, wanted, html)?;
    Some([
        rect.origin.x * 64.0,
        rect.origin.y * 64.0,
        rect.size.width * 64.0,
        rect.size.height * 64.0,
    ])
}

fn rect_of(boxes: &BoxTree, layout: &LayoutTree, wanted: &str, html: &str) -> Option<Rect> {
    let document = parse_document(html);
    let node = document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.attr("id") == Some(wanted))
    })?;
    let root = boxes.root()?;
    let found: BoxId = core::iter::once(root)
        .chain(boxes.descendants(root))
        .find(|id| boxes.get(*id).and_then(|held| held.kind.node()) == Some(node))?;
    layout.border_box(found)
}

/// Whether two numbers of units are the same: every number here is a whole
/// number of units, which an `f32` holds exactly at these sizes.
fn same(left: f32, right: f32) -> bool {
    left.to_bits() == right.to_bits()
}

#[test]
fn a_padding_and_a_margin_are_truncated_toward_zero() {
    // ADR 0054 § 1: `0.3px` is 19 units, `-0.3px` is −19, `0.6rem` is 614.
    let html = "<div id=o><div id=i></div></div>";
    let inner = units_of(
        html,
        "#o { padding-left: 0.3px } #i { height: 0.6rem }",
        "i",
    );
    assert!(
        inner.is_some_and(|[x, _, _, height]| same(x, 19.0) && same(height, 614.0)),
        "{inner:?}"
    );
    let pushed = units_of(html, "#o { margin-left: -0.3px }", "o");
    assert!(pushed.is_some_and(|[x, ..]| same(x, -19.0)), "{pushed:?}");
    // `19.92px` is 1,274.88 and becomes 1,274; half a unit over 10 px is 640.
    let tall = units_of(html, "#i { height: 19.92px; width: 10.0078125px }", "i");
    assert!(
        tall.is_some_and(|[_, _, width, height]| same(width, 640.0) && same(height, 1274.0)),
        "{tall:?}"
    );
}

#[test]
fn each_length_is_truncated_on_its_own() {
    // Three nested paddings of 0.01 px put the innermost box 0 units in.
    let html = "<div id=a><div id=b><div id=c><div id=d></div></div></div></div>";
    let nested = units_of(html, "#a, #b, #c { padding-left: 0.01px }", "d");
    assert!(nested.is_some_and(|[x, ..]| same(x, 0.0)), "{nested:?}");
    // A box as tall as three children each 10.3 px is three times 659.
    let html = "<div id=s><div></div><div></div><div></div></div>";
    let stacked = units_of(html, "#s > div { height: 10.3px }", "s");
    assert!(
        stacked.is_some_and(|[.., height]| same(height, 1977.0)),
        "{stacked:?}"
    );
}

#[test]
fn a_percentage_is_truncated_after_it_is_resolved_against_a_basis_on_the_grid() {
    // ADR 0054 § 2: 50% of a container written 100.3 px is half of 6,419
    // units, 3,209.5, and Chrome gives 3,209.
    let html = "<div id=o><div id=i></div></div>";
    let half = units_of(html, "#o { width: 100.3px } #i { width: 50% }", "i");
    assert!(
        half.is_some_and(|[_, _, width, _]| same(width, 3209.0)),
        "{half:?}"
    );
    let third = units_of(html, "#o { width: 201px } #i { width: 33.3333% }", "i");
    assert!(
        third.is_some_and(|[_, _, width, _]| same(width, 4287.0)),
        "{third:?}"
    );
    let back = units_of(
        html,
        "#o { width: 100px } #i { margin-left: -33.3333% }",
        "i",
    );
    assert!(back.is_some_and(|[x, ..]| same(x, -2133.0)), "{back:?}");
    // A percentage of a percentage: 2,133 of 33.3333%, 710.99, is 710.
    let html = "<div id=o><div id=m><div id=i></div></div></div>";
    let nested = units_of(html, "#o { width: 100px } #m, #i { width: 33.3333% }", "i");
    assert!(
        nested.is_some_and(|[_, _, width, _]| same(width, 710.0)),
        "{nested:?}"
    );
}

#[test]
fn a_calculation_is_resolved_whole_and_truncated_once() {
    // ADR 0054 § 2: three hundredths of a pixel are 1.92 units together, and
    // 1 truncated; truncated alone they would be none.
    let html = "<div id=o><div id=i></div></div>";
    let lengths = units_of(html, "#i { width: calc(0.01px + 0.01px + 0.01px) }", "i");
    assert!(
        lengths.is_some_and(|[_, _, width, _]| same(width, 1.0)),
        "{lengths:?}"
    );
    let mixed = units_of(
        html,
        "#o { width: 100px } #i { width: calc(33.3333% + 0.01px + 0.01px) }",
        "i",
    );
    assert!(
        mixed.is_some_and(|[_, _, width, _]| same(width, 2134.0)),
        "{mixed:?}"
    );
}

#[test]
fn centring_truncates_each_share_toward_zero() {
    // ADR 0054 § 3.
    let html = "<div id=o><div id=i></div></div>";
    let across = "#o { display: flex; justify-content: center; width: 100.3px } \
                  #i { width: 10px; height: 10px; flex-shrink: 0 }";
    let centred = units_of(html, across, "i");
    assert!(
        centred.is_some_and(|[x, ..]| same(x, 2889.0)),
        "{centred:?}"
    );
    let over = "#o { display: flex; justify-content: center; width: 10.3px } \
                #i { width: 20px; height: 10px; flex-shrink: 0 }";
    let overflowing = units_of(html, over, "i");
    assert!(
        overflowing.is_some_and(|[x, _, width, _]| same(x, -310.0) && same(width, 1280.0)),
        "toward zero, and still 20 px wide: {overflowing:?}"
    );
    let down = "#o { display: flex; align-items: center; height: 31.3px } \
                #i { width: 10px; height: 10px }";
    let middle = units_of(html, down, "i");
    assert!(
        middle.is_some_and(|[_, y, ..]| same(y, 681.0)),
        "{middle:?}"
    );
    let margins = units_of(
        html,
        "#o { width: 101px } #i { width: 10px; margin: 0 auto }",
        "i",
    );
    assert!(
        margins.is_some_and(|[x, ..]| same(x, 2912.0)),
        "{margins:?}"
    );
}

#[test]
fn a_grids_distribution_truncates_the_share_in_front() {
    // Three 10 px tracks in 100.3 px, 4,499 units free (ADR 0054 § 3).
    // Chrome truncates each share and adds them up: `space-around` is 749 in
    // front, then 1,499 between each, so its tracks start at 749, 2,888 and
    // 5,027. The boundary sees only where `taffy` put each track, which is
    // its exact shares added up, 749.8, 2,889.5 and 5,029.2, and truncates
    // that: 749, 2,889 and 5,029. The first share is Chrome's, and every
    // later track is within a unit for each share before it, on the grid.
    // Building it exactly needs `taffy`'s own numbers, which is item 499's.
    let html = "<div id=g><div id=a></div><div id=b></div><div id=c></div></div>";
    let grid = |distribution: &str| {
        format!(
            "#g {{ display: grid; width: 100.3px; height: 10px; \
                   grid-template-columns: 10px 10px 10px; justify-content: {distribution} }}"
        )
    };
    let lefts = |distribution: &str| {
        ["a", "b", "c"].map(|id| units_of(html, &grid(distribution), id).map(|[x, ..]| x))
    };
    for (distribution, chrome) in [
        ("space-around", [749.0, 2888.0, 5027.0]),
        ("space-evenly", [1124.0, 2888.0, 4652.0]),
        ("space-between", [0.0, 2889.0, 5778.0]),
    ] {
        let ours = lefts(distribution);
        assert_eq!(ours[0], Some(chrome[0]), "{distribution}: {ours:?}");
        for (track, (left, wanted)) in ours.iter().zip(chrome).enumerate() {
            let Some(left) = left else {
                panic!("{distribution}: track {track} was laid out");
            };
            assert!(
                same(*left, left.trunc()),
                "{distribution}: {left} is on the grid"
            );
            #[expect(clippy::cast_precision_loss, reason = "one of three tracks")]
            let shares_before = track as f32;
            assert!(
                (*left - wanted).abs() <= shares_before,
                "{distribution}: track {track} at {left}, Chrome's at {wanted}"
            );
        }
    }
}

#[test]
fn flexible_tracks_end_at_their_ideal_edges_floored() {
    // Seven `1fr` in 201 px: Chrome's edges are 0, 1,837, 3,675, 5,513,
    // 7,350, 9,188 and 11,026 for the first seven lines (ADR 0054 § 3).
    let html = "<div id=g><i id=t0></i><i id=t1></i><i id=t2></i><i id=t3></i>\
                <i id=t4></i><i id=t5></i><i id=t6></i></div>";
    let css = "#g { display: grid; width: 201px; grid-template-columns: repeat(7, 1fr) } \
               i { display: block; height: 10px }";
    let edges = [0.0, 1837.0, 3675.0, 5513.0, 7350.0, 9188.0, 11026.0];
    for (track, edge) in edges.iter().enumerate() {
        let placed = units_of(html, css, &format!("t{track}"));
        assert!(
            placed.is_some_and(|[x, ..]| same(x, *edge)),
            "{track}: {placed:?}"
        );
    }
    // Three `1fr` with a 0.3 px gap in 100 px: 2,120, 2,121 and 2,121, with
    // gaps of 19.
    let html = "<div id=g><i id=a></i><i id=b></i><i id=c></i></div>";
    let css = "#g { display: grid; width: 100px; column-gap: 0.3px; \
                    grid-template-columns: 1fr 1fr 1fr } i { display: block; height: 10px }";
    let tracks = ["a", "b", "c"].map(|id| units_of(html, css, id));
    assert_eq!(
        tracks.map(|found| found.map(|[x, _, width, _]| (x, width))),
        [
            Some((0.0, 2120.0)),
            Some((2139.0, 2121.0)),
            Some((4279.0, 2121.0))
        ]
    );
}

#[test]
fn an_aspect_ratio_truncates_the_size_it_derives() {
    // 10 px wide at `aspect-ratio: 3` is 213.33 units tall, and 213.
    let html = "<div id=o><div id=i></div></div>";
    let derived = units_of(html, "#i { width: 10px; aspect-ratio: 3 }", "i");
    assert!(
        derived.is_some_and(|[.., height]| same(height, 213.0)),
        "{derived:?}"
    );
}

#[test]
fn a_box_that_shrinks_to_its_text_is_its_width_rounded_up() {
    // `ScaledFont` gives a character half its size: one at 13.3 px is 6.65
    // px, 425.6 units, and the box that shrinks to it is 426 (ADR 0054 § 4).
    let html = "<div id=o><span id=i>W</span></div>";
    for css in [
        "#o { font-size: 13.3px } #i { display: inline-block }",
        "#o { font-size: 13.3px; display: flex } #i { display: block }",
        "#o { font-size: 13.3px } #i { position: absolute }",
    ] {
        let fitted = units_measured(html, css, "i", &ScaledFont);
        assert!(
            fitted.is_some_and(|[_, _, width, _]| same(width, 426.0)),
            "{css}: {fitted:?}"
        );
    }
    // A width already on the grid stays: one character at 16 px is 8 px.
    let html = "<div id=o><span id=i>W</span></div>";
    let whole = units_measured(html, "#i { display: inline-block }", "i", &ScaledFont);
    assert!(
        whole.is_some_and(|[_, _, width, _]| same(width, 512.0)),
        "{whole:?}"
    );
}

#[test]
fn a_length_past_the_grids_range_saturates() {
    // ADR 0054 § 5: both read 2,147,483,392 units in Chrome.
    let html = "<div id=o><div id=i></div></div>";
    for width in ["33554431.99px", "1000000000px"] {
        let wide = units_of(html, &format!("#i {{ width: {width}; height: 1px }}"), "i");
        assert!(
            wide.is_some_and(|[_, _, width, _]| same(width, 2_147_483_392.0)),
            "{width}: {wide:?}"
        );
    }
}

#[test]
fn every_edge_of_a_page_of_shares_is_on_the_grid() {
    // Flexible items' exact shares are queue item 499's; what this engine
    // already promises is that every edge it hands back is on the grid.
    let html = "<div id=f><i></i><i></i><i></i><i></i><i></i><i></i><i></i></div>\
                <div id=c><b>x</b></div>";
    let css = "#f { display: flex; width: 201px; gap: 0.3px } i { flex: 1; height: 7.7px } \
               #c { display: flex; justify-content: space-evenly; align-items: center; \
                    width: 100.3px; height: 33.3px; padding: 0.37em }";
    let (boxes, layout) = lay_out(html, css, &ScaledFont);
    let Some(root) = boxes.root() else {
        panic!("a tree");
    };
    let mut seen = 0;
    for id in core::iter::once(root).chain(boxes.descendants(root)) {
        let Some(rect) = layout.border_box(id) else {
            continue;
        };
        for number in [
            rect.origin.x,
            rect.origin.y,
            rect.size.width,
            rect.size.height,
        ] {
            let units = number * 64.0;
            assert!(same(units, units.trunc()), "{id:?}: {rect:?}");
        }
        seen += 1;
    }
    assert!(seen > 10, "{seen}");
}
