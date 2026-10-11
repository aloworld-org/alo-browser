/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A calculated track in an auto repetition, against its plain twin and
//! against Chrome — ADR 0047 § 6, in numbers.
//!
//! alo Sites' blog grid is `repeat(auto-fit, minmax(min(100%, 18rem),
//! 1fr))`. `taffy` 0.14 discards any auto repetition with a calculated
//! track, so `alo-layout`'s `auto_repeat.rs` resolves the calculation
//! against the grid's content box where that is definite and hands `taffy`
//! the length. Each case below was measured in Chrome 154, headless, with
//! `gap: 24px` and items 10 tall, and the track `minmax(min(100%, 288px),
//! 1fr)` unless it is the *plain* twin, `minmax(288px, 1fr)`.
//!
//! **Agreement (A–D):** the calculated grid lays out exactly as its plain
//! twin, which `taffy` counts by itself, and both as Chrome did. These are
//! what notice the day `taffy`'s rule or ours drifts — and they still pass
//! the day `auto_repeat.rs` is deleted because `taffy` learned to count a
//! calculation, which is how that day is noticed.
//!
//! **Refusals (E–G):** where the grid's width is not definite, Chrome's
//! answer differs from the plain twin's in every case, so the template is
//! left to `taffy`, which discards it — today's layout, the same as no
//! template at all — and one issue says so. Queue item 424 is the rest.

use alo_box::{BoxId, BoxTree, build};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{BlockFont, LayoutTree, Rect, Size, compute};
use alo_style::{Origin, SourcedSheet, StyleTree, USER_AGENT_STYLE_SHEET, resolve};

/// The track Chrome was measured with.
const CALCULATED: &str = "minmax(min(100%, 288px), 1fr)";

/// Its plain twin: the same track with the length the calculation comes to
/// in that grid — 288 wherever the content box is wider, and the content box
/// where it is not.
fn plain(length: f32) -> String {
    format!("minmax({length}px, 1fr)")
}

/// Equal to within far less than a pixel.
fn close(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.0001
}

/// A grid laid out in an 800 × 600 viewport with nothing around it: the
/// container's own declarations, its template, and an item a width each
/// (`auto` for none).
struct Grid {
    rects: Vec<Rect>,
    issues: Vec<String>,
}

fn lay_out(display: &str, container: &str, template: &str, items: &[&str]) -> Grid {
    let html = format!(
        "<body><div id=grid>{}</div></body>",
        items
            .iter()
            .map(|_| "<div class=item></div>")
            .collect::<String>()
    );
    let mut css = format!(
        "body {{ margin: 0 }} \
         #grid {{ display: {display}; gap: 24px; grid-template-columns: {template}; {container} }} \
         .item {{ height: 10px }}"
    );
    for (index, width) in items.iter().enumerate() {
        let nth = index + 1;
        css.push_str(" .item:nth-child(");
        css.push_str(&nth.to_string());
        css.push_str(") { width: ");
        css.push_str(width);
        css.push_str(" }");
    }
    let document = parse_document(&html);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(&css);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles: StyleTree = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build(&document, &styles);
    let layout = compute(&boxes, &styles, Size::new(800.0, 600.0), &BlockFont);
    if std::env::var_os("DBG").is_some() {
        eprintln!("{css}\n{}", layout.to_outline(&boxes));
    }
    Grid {
        rects: items_of(&document, &boxes, &layout),
        issues: layout.issues().iter().map(ToString::to_string).collect(),
    }
}

/// Every item's border box, in document order.
fn items_of(document: &alo_dom::Document, boxes: &BoxTree, layout: &LayoutTree) -> Vec<Rect> {
    let Some(root) = boxes.root() else {
        return Vec::new();
    };
    core::iter::once(root)
        .chain(boxes.descendants(root))
        .filter(|id: &BoxId| {
            boxes
                .get(*id)
                .and_then(|held| held.kind.node())
                .and_then(|node| document.element(node))
                .is_some_and(|element| element.attr("class") == Some("item"))
        })
        .filter_map(|id| layout.border_box(id))
        .collect()
}

/// Each item's left edge and width, which is where its column is.
fn columns(grid: &Grid) -> Vec<(f32, f32)> {
    grid.rects
        .iter()
        .map(|rect| (rect.origin.x, rect.size.width))
        .collect()
}

fn same_columns(left: &Grid, right: &Grid) -> bool {
    left.rects.len() == right.rects.len()
        && left.rects.iter().zip(&right.rects).all(|(a, b)| {
            close(a.origin.x, b.origin.x)
                && close(a.origin.y, b.origin.y)
                && close(a.size.width, b.size.width)
                && close(a.size.height, b.size.height)
        })
}

fn as_chrome(grid: &Grid, wanted: &[(f32, f32)]) -> bool {
    let found = columns(grid);
    found.len() == wanted.len()
        && found
            .iter()
            .zip(wanted)
            .all(|(a, b)| close(a.0, b.0) && close(a.1, b.1))
}

/// An agreement case: the calculated grid, its plain twin, and Chrome.
fn agrees(container: &str, count: &str, comes_to: f32, items: &[&str], chrome: &[(f32, f32)]) {
    let calculated = lay_out(
        "grid",
        container,
        &format!("repeat({count}, {CALCULATED})"),
        items,
    );
    let plain = lay_out(
        "grid",
        container,
        &format!("repeat({count}, {})", plain(comes_to)),
        items,
    );
    assert!(
        same_columns(&calculated, &plain),
        "calculated {:?}\nplain {:?}",
        columns(&calculated),
        columns(&plain)
    );
    assert!(
        as_chrome(&calculated, chrome),
        "{:?}, Chrome {chrome:?}",
        columns(&calculated)
    );
    assert!(calculated.issues.is_empty(), "{:?}", calculated.issues);
}

/// A refusal: one issue naming the template, and the layout it comes to.
///
/// `taffy` sizes a grid out of flow in two calls: it measures the grid with
/// no definite width — the refusal, the template discarded — and then lays
/// it out in the width that measurement gave, which is handed to it and so
/// definite by ADR 0047 § 1, and lowered there. Where the measured width
/// is no wider than one track, that is the same layout as no template at
/// all, which is what E and F assert; G's minimum makes it 700 wide, and
/// counted against 700 it is Chrome's two columns (ADR 0047 § 6 expected
/// today's single column there, and the build found otherwise).
///
/// Chrome was measured with an `inline-grid`. These are a grid out of flow,
/// which is sized to fit its content the same way and has no definite width
/// either, because an `inline-grid` holding blocks is split around them by
/// the box tree today, as an inline box would be (queue item 426).
fn refused(container: &str, items: &[&str]) -> Grid {
    let container = format!("position: absolute; {container}");
    let calculated = lay_out(
        "grid",
        &container,
        &format!("repeat(auto-fit, {CALCULATED})"),
        items,
    );
    let without = lay_out("grid", &container, "none", items);
    assert!(without.issues.is_empty(), "{:?}", without.issues);
    let said: Vec<&String> = calculated
        .issues
        .iter()
        .filter(|issue| issue.contains("grid-template-columns: repeat(auto-fit"))
        .collect();
    assert_eq!(said.len(), 1, "{:?}", calculated.issues);
    assert!(
        said.iter().all(|issue| issue.contains("definite width")),
        "{said:?}"
    );
    calculated
}

/// The layout a refused grid would have with no template at all.
fn as_without_a_template(grid: &Grid, container: &str, items: &[&str]) -> bool {
    let without = lay_out(
        "grid",
        &format!("position: absolute; {container}"),
        "none",
        items,
    );
    same_columns(grid, &without)
}

const THREE: [&str; 3] = ["auto", "auto", "auto"];

#[test]
fn a_a_definite_grid_fits_two_columns_as_its_plain_twin_does() {
    agrees(
        "width: 760px",
        "auto-fit",
        288.0,
        &THREE,
        &[(0.0, 368.0), (392.0, 368.0), (0.0, 368.0)],
    );
}

#[test]
fn b_one_item_under_auto_fit_takes_the_collapsed_tracks_room() {
    agrees(
        "width: 760px",
        "auto-fit",
        288.0,
        &["auto"],
        &[(0.0, 760.0)],
    );
}

#[test]
fn b_one_item_under_auto_fill_keeps_its_empty_track() {
    agrees(
        "width: 760px",
        "auto-fill",
        288.0,
        &["auto"],
        &[(0.0, 368.0)],
    );
}

#[test]
fn c_a_narrow_grid_takes_the_hundred_per_cent_of_the_min() {
    agrees(
        "width: 200px",
        "auto-fit",
        200.0,
        &THREE,
        &[(0.0, 200.0), (0.0, 200.0), (0.0, 200.0)],
    );
}

#[test]
fn d_the_percentage_is_of_the_content_box() {
    agrees(
        "width: 800px; padding: 20px; border: 3px solid; box-sizing: border-box",
        "auto-fit",
        288.0,
        &THREE,
        &[(23.0, 365.0), (412.0, 365.0), (23.0, 365.0)],
    );
}

#[test]
fn e_a_grid_sized_by_its_content_is_refused_and_says_so() {
    let items = ["50px", "50px", "0px"];
    let grid = refused("", &items);
    assert!(
        as_without_a_template(&grid, "", &items),
        "{:?}",
        columns(&grid)
    );
    // One column, 50 wide: Chrome's too, by the coincidence that the widest
    // item is narrower than 288.
    assert!(
        as_chrome(&grid, &[(0.0, 50.0), (0.0, 50.0), (0.0, 0.0)]),
        "{:?}",
        columns(&grid)
    );
}

#[test]
fn f_a_grid_with_only_a_maximum_is_refused_and_says_so() {
    let grid = refused("max-width: 700px", &THREE);
    // Its empty items measure nothing, so it is 0 wide, as with no template.
    // Chrome's is 198: still wrong, and said (queue item 424).
    assert!(
        as_without_a_template(&grid, "max-width: 700px", &THREE),
        "{:?}",
        columns(&grid)
    );
    assert!(
        as_chrome(&grid, &[(0.0, 0.0), (0.0, 0.0), (0.0, 0.0)]),
        "{:?}",
        columns(&grid)
    );
}

#[test]
fn g_a_grid_with_only_a_minimum_is_refused_and_says_so() {
    let grid = refused("min-width: 700px", &THREE);
    // Measured without its template, then laid out 700 wide and counted
    // against that: two columns of 338, which is Chrome's answer.
    assert!(
        as_chrome(&grid, &[(0.0, 338.0), (362.0, 338.0), (0.0, 338.0)]),
        "{:?}",
        columns(&grid)
    );
}

#[test]
fn a_grid_of_grids_each_resolves_against_its_own_width() {
    // The outer grid's two columns are 368; each holds a grid of the same
    // template, whose own content box is 368 — so each inner grid is one
    // column of 368 (two of 288 and a gap of 24 do not fit), held while the
    // outer one is held. The outer grid measures each inner one for its
    // content with no width, which is a refusal and said, once each.
    let html = "<body><div id=outer><div class=inner><div class=item></div><div class=item></div></div>\
                <div class=inner><div class=item></div></div></div></body>";
    let css = format!(
        "body {{ margin: 0 }} \
         #outer, .inner {{ display: grid; gap: 24px; grid-template-columns: repeat(auto-fit, {CALCULATED}) }} \
         #outer {{ width: 760px }} \
         .item {{ height: 10px }}"
    );
    let document = parse_document(html);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(&css);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles: StyleTree = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build(&document, &styles);
    let layout = compute(&boxes, &styles, Size::new(800.0, 600.0), &BlockFont);
    let found: Vec<(f32, f32)> = items_of(&document, &boxes, &layout)
        .iter()
        .map(|rect| (rect.origin.x, rect.size.width))
        .collect();
    let wanted = [(0.0, 368.0), (0.0, 368.0), (392.0, 368.0)];
    assert!(
        found.len() == 3
            && found
                .iter()
                .zip(wanted)
                .all(|(a, b)| close(a.0, b.0) && close(a.1, b.1)),
        "{found:?}"
    );
    let issues: Vec<String> = layout.issues().iter().map(ToString::to_string).collect();
    assert_eq!(issues.len(), 2, "{issues:?}");
    assert!(
        issues
            .iter()
            .all(|issue| issue.contains("grid-template-columns: repeat(auto-fit")),
        "{issues:?}"
    );
}

/// A deep nest of grids each lowering its own template, each held while
/// the ones around it are, laid out on a thread with the 8 MiB stack
/// `alo-render`'s main thread has. How deep any nest of grids may go before
/// the recursion through `taffy` runs out of stack is not this file's: it
/// is the same depth with a plain template or none (queue item 427).
#[test]
fn a_deep_nest_of_calculated_grids_lays_out_without_a_panic() {
    let depth = 200;
    let template = format!("repeat(auto-fill, {CALCULATED})");
    let laid_out = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            let html = format!(
                "<body>{}<div class=item></div>{}</body>",
                "<div class=g>".repeat(depth),
                "</div>".repeat(depth)
            );
            let css = format!(
                "body {{ margin: 0 }} \
                 .g {{ display: grid; width: 100%; grid-template-columns: {template} }} \
                 .item {{ height: 10px }}"
            );
            let document = parse_document(&html);
            let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
            let author = parse_stylesheet(&css);
            let sheets = [
                SourcedSheet::new(Origin::UserAgent, &agent),
                SourcedSheet::new(Origin::Author, &author),
            ];
            let styles: StyleTree = resolve(&document, &sheets, &MediaContext::default());
            let boxes = build(&document, &styles);
            let layout = compute(&boxes, &styles, Size::new(800.0, 600.0), &BlockFont);
            items_of(&document, &boxes, &layout)
                .iter()
                .map(|rect| rect.size.width)
                .collect::<Vec<f32>>()
        })
        .map(std::thread::JoinHandle::join);
    let Ok(Ok(widths)) = laid_out else {
        panic!("the layout thread did not finish");
    };
    // The outermost grid fits two tracks of 288 in 800, each 400 by its
    // `1fr`; every grid inside is 100 % of the 400 it is in, fits one, and
    // so the item is 400 wide.
    assert!(
        widths.len() == 1 && widths.iter().all(|width| close(*width, 400.0)),
        "{widths:?}"
    );
}
