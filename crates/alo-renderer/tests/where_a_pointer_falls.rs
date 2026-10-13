/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which element a person's pointer is over, through the whole pipeline
//! (ADR 0024 § 5, queue item 298).
//!
//! Every page here was put to Chrome 154 at 400 × 300 with the `dejavu`
//! crate's sans, and every expected element is the one its
//! `elementFromPoint` answered; mouse events sent over the `DevTools`
//! protocol found the same. Beside each, the boxes the points depend on are
//! asserted in numbers, so a pointer that finds the wrong element because
//! layout moved says so as a layout failure rather than as a hit.

use alo_dom::{Document, NodeId};
use alo_layout::Rect;
use alo_layout::geometry::Size;
use alo_paint::Point;
use alo_renderer::hit::element_at;
use alo_renderer::pipeline::{Rendered, render};
use alo_text::{Font, FontDatabase, Slant, Weight};

/// The `dejavu` crate's sans, as the generic sans-serif.
fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    if let Some(font) = Font::load(
        "DejaVu Sans",
        Weight::NORMAL,
        Slant::Normal,
        dejavu::sans::regular().to_vec(),
    ) {
        database.add(font);
    }
    database.map_generic("sans-serif", "DejaVu Sans");
    database
}

/// `body` as Chrome was given it: no margin unless the page says otherwise,
/// in the `dejavu` crate's sans, 400 × 300.
fn page(body: &str) -> Rendered {
    let html = format!(
        "<!doctype html><html><head><style>body{{margin:0;font-family:'DejaVu Sans'}}\
         </style></head><body>{body}</body></html>"
    );
    render(&html, "", Size::new(400.0, 300.0), &fonts())
}

/// What a test calls an element: its `id`, or else its tag.
fn name(document: &Document, node: NodeId) -> String {
    let Some(element) = document.element(node) else {
        return String::from("not an element");
    };
    element
        .attr("id")
        .map_or_else(|| element.name.local.to_string(), str::to_string)
}

/// The element found at each point, by name, beside the one Chrome found.
fn found(page: &Rendered, points: &[(f32, f32, &str)]) -> Vec<String> {
    points
        .iter()
        .filter_map(|&(x, y, wanted)| {
            let at = element_at(&page.drawing, &page.document, Point::new(x, y));
            let got = at.map_or_else(
                || String::from("nothing"),
                |node| name(&page.document, node),
            );
            (got != wanted).then(|| format!("({x}, {y}): {got}, where Chrome found {wanted}"))
        })
        .collect()
}

/// The element named `id`.
fn by_id(document: &Document, id: &str) -> Option<NodeId> {
    document
        .descendants(document.root())
        .find(|node| document.element(*node).and_then(|e| e.attr("id")) == Some(id))
}

/// Where each piece of the element named `id` was laid out: its border box,
/// or one rectangle per line it sits on.
fn pieces(page: &Rendered, id: &str) -> Vec<(f32, f32, f32, f32)> {
    let Some(node) = by_id(&page.document, id) else {
        return Vec::new();
    };
    let drawing = &page.drawing;
    let Some(first) = drawing
        .boxes
        .ids()
        .find(|box_id| drawing.boxes.get(*box_id).and_then(|b| b.kind.node()) == Some(node))
    else {
        return Vec::new();
    };
    let rects: Vec<Rect> = match drawing.layout.fragments(first) {
        [] => drawing
            .layout
            .get(first)
            .map(|geometry| vec![geometry.border_box])
            .unwrap_or_default(),
        some => some.iter().map(|fragment| fragment.rect).collect(),
    };
    rects
        .into_iter()
        .map(|rect| (rect.left(), rect.top(), rect.size.width, rect.size.height))
        .collect()
}

/// Whether two sets of rectangles agree to within a 64th of a pixel.
fn same(got: &[(f32, f32, f32, f32)], wanted: &[(f32, f32, f32, f32)]) -> bool {
    let near = |a: f32, b: f32| (a - b).abs() < 1.0 / 64.0;
    got.len() == wanted.len()
        && got
            .iter()
            .zip(wanted)
            .all(|(g, w)| near(g.0, w.0) && near(g.1, w.1) && near(g.2, w.2) && near(g.3, w.3))
}

#[test]
fn a_box_is_found_from_the_pixel_before_its_start_to_its_end() {
    let page = page(
        r#"<div id=a style="position:absolute;left:10px;top:10px;width:100px;height:50px"></div>"#,
    );
    assert!(same(&pieces(&page, "a"), &[(10.0, 10.0, 100.0, 50.0)]));
    let wrong = found(
        &page,
        &[
            (10.0, 10.0, "a"),
            (9.99, 10.0, "a"),
            (9.5, 10.0, "a"),
            (9.05, 30.0, "a"),
            (9.0, 30.0, "html"),
            (10.0, 9.99, "a"),
            (60.0, 9.05, "a"),
            (60.0, 9.0, "html"),
            (109.99, 59.99, "a"),
            (110.0, 30.0, "html"),
            (109.5, 30.0, "a"),
            (60.0, 60.0, "html"),
            (60.0, 59.5, "a"),
            (200.0, 200.0, "html"),
        ],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_box_at_fractions_of_a_pixel_is_found_at_fractions() {
    let page = page(
        r#"<div id=a style="position:absolute;left:10.25px;top:10.75px;width:20.5px;height:20.5px"></div>"#,
    );
    assert!(same(&pieces(&page, "a"), &[(10.25, 10.75, 20.5, 20.5)]));
    let wrong = found(
        &page,
        &[
            (9.25, 15.0, "html"),
            (9.3, 15.0, "a"),
            (10.24, 15.0, "a"),
            (30.74, 15.0, "a"),
            (30.75, 15.0, "html"),
            (15.0, 9.75, "html"),
            (15.0, 9.8, "a"),
            (15.0, 31.24, "a"),
            (15.0, 31.25, "html"),
        ],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn of_two_boxes_side_by_side_the_later_takes_the_pixel_they_share() {
    let page = page(
        r#"<div id=a style="position:absolute;left:0;top:0;width:10.25px;height:20px"></div>
           <div id=b style="position:absolute;left:10.25px;top:0;width:10px;height:20px"></div>"#,
    );
    let wrong = found(&page, &[(9.5, 5.0, "b"), (9.2, 5.0, "a"), (11.0, 5.0, "b")]);
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn the_box_painted_last_is_the_one_found() {
    let page = page(
        r#"<div id=a style="position:absolute;left:0;top:0;width:100px;height:100px"></div>
           <div id=b style="position:absolute;left:50px;top:50px;width:100px;height:100px"></div>
           <div id=c style="position:absolute;left:200px;top:0;width:100px;height:100px;z-index:2"></div>
           <div id=d style="position:absolute;left:250px;top:50px;width:100px;height:100px;z-index:1"></div>
           <div id=e style="position:absolute;left:0;top:200px;width:100px;height:50px;z-index:-1"></div>
           <div id=f style="margin-top:220px;height:50px;width:50px"></div>"#,
    );
    assert!(same(&pieces(&page, "f"), &[(0.0, 220.0, 50.0, 50.0)]));
    let wrong = found(
        &page,
        &[
            (75.0, 75.0, "b"),
            (25.0, 25.0, "a"),
            (125.0, 125.0, "b"),
            (275.0, 75.0, "c"),
            (325.0, 125.0, "d"),
            (20.0, 230.0, "f"),
            // The body is drawn over a negative `z-index`, background or not.
            (80.0, 230.0, "body"),
            (80.0, 210.0, "e"),
        ],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_block_is_found_in_its_border_and_padding_and_not_its_margin() {
    let page = page(
        r#"<style>body{margin:8px}</style><div id=outer style="margin:10px;padding:10px;border:5px solid;width:100px">
             <div id=inner style="height:20px"></div></div>"#,
    );
    assert!(same(&pieces(&page, "outer"), &[(18.0, 10.0, 130.0, 50.0)]));
    assert!(same(&pieces(&page, "inner"), &[(33.0, 25.0, 100.0, 20.0)]));
    let wrong = found(
        &page,
        &[
            (2.0, 2.0, "html"),
            (10.0, 10.0, "body"),
            (19.0, 19.0, "outer"),
            (20.0, 20.0, "outer"),
            (26.0, 26.0, "outer"),
            (36.0, 36.0, "inner"),
            (36.0, 50.0, "outer"),
            (200.0, 36.0, "body"),
            (200.0, 150.0, "html"),
        ],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_turned_box_is_found_where_it_was_turned_to() {
    let page = page(
        r#"<div id=a style="position:absolute;left:100px;top:100px;width:100px;height:100px;transform:rotate(45deg)"></div>"#,
    );
    assert!(same(&pieces(&page, "a"), &[(100.0, 100.0, 100.0, 100.0)]));
    let wrong = found(
        &page,
        &[
            (101.0, 101.0, "html"),
            (150.0, 150.0, "a"),
            (150.0, 85.0, "a"),
            (150.0, 78.0, "html"),
            (215.0, 150.0, "a"),
            (222.0, 150.0, "html"),
        ],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_moved_box_and_what_it_holds_are_found_where_they_moved_to() {
    let page = page(
        r#"<div id=a style="position:absolute;left:0;top:0;width:50px;height:50px;transform:translate(100px,20px)"><div id=b style="width:10px;height:10px"></div></div>"#,
    );
    assert!(same(&pieces(&page, "a"), &[(0.0, 0.0, 50.0, 50.0)]));
    let wrong = found(
        &page,
        &[
            (10.0, 10.0, "html"),
            (101.0, 21.0, "b"),
            (111.0, 31.0, "a"),
            (149.0, 69.0, "a"),
            (150.0, 69.0, "html"),
        ],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_scaled_box_is_found_at_its_scaled_edges() {
    let page = page(
        r#"<div id=a style="position:absolute;left:100px;top:100px;width:50px;height:50px;transform:scale(2)"></div>"#,
    );
    let wrong = found(
        &page,
        &[
            (76.0, 76.0, "a"),
            (74.0, 74.0, "html"),
            (174.0, 174.0, "a"),
            (176.0, 176.0, "html"),
        ],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn what_a_box_clips_is_not_found_outside_its_padding_box() {
    let page = page(
        r#"<div id=q style="position:absolute;left:0;top:0;width:200px;height:50px;overflow:hidden">
             <div id=p style="width:100px;height:100px;overflow:hidden;border:10px solid"><div id=c style="width:300px;height:300px"></div></div></div>"#,
    );
    assert!(same(&pieces(&page, "p"), &[(0.0, 0.0, 120.0, 120.0)]));
    assert!(same(&pieces(&page, "c"), &[(10.0, 10.0, 300.0, 300.0)]));
    let wrong = found(
        &page,
        &[
            (115.0, 30.0, "p"),
            (115.0, 55.0, "html"),
            (50.0, 49.0, "c"),
            (50.0, 50.0, "html"),
        ],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_clip_with_rounded_corners_hides_its_corners() {
    let page = page(
        r#"<div id=p style="position:absolute;left:0;top:0;width:100px;height:100px;overflow:hidden;border-radius:50px">
             <div id=c style="width:100px;height:100px"></div></div>"#,
    );
    let wrong = found(
        &page,
        &[
            (50.0, 50.0, "c"),
            (5.0, 5.0, "html"),
            (15.0, 15.0, "c"),
            (50.0, 1.0, "c"),
            (95.0, 95.0, "html"),
        ],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_boxs_own_rounded_corners_are_not_there() {
    let page = page(
        r#"<div id=a style="position:absolute;left:0;top:0;width:100px;height:100px;border-radius:50px"></div>"#,
    );
    let wrong = found(
        &page,
        &[
            (5.0, 5.0, "html"),
            (15.0, 15.0, "a"),
            (50.0, 1.0, "a"),
            (99.0, 50.0, "a"),
            (95.0, 95.0, "html"),
            (50.0, 50.0, "a"),
        ],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_turned_clip_hides_what_is_outside_it() {
    let page = page(
        r#"<div id=p style="position:absolute;left:100px;top:100px;width:100px;height:100px;overflow:hidden;transform:rotate(45deg)">
             <div id=c style="width:300px;height:300px"></div></div>"#,
    );
    let wrong = found(
        &page,
        &[
            (150.0, 150.0, "c"),
            (101.0, 101.0, "html"),
            (150.0, 85.0, "c"),
            (250.0, 150.0, "html"),
        ],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn an_absolute_box_placed_outside_a_clip_is_found_outside_it() {
    // Its containing block is the viewport, not the box that clips, so the
    // clip does not hold it. Placed in a positioned box that clips, Chrome
    // finds it over that box and only inside it; this engine draws it under
    // that box and outside the clip, and the pointer finds what is drawn
    // (queue item 538).
    let page = page(
        r#"<div id=p style="width:100px;height:100px;overflow:hidden">
             <div id=c style="position:absolute;left:50px;top:50px;width:100px;height:100px"></div></div>"#,
    );
    assert!(same(&pieces(&page, "c"), &[(50.0, 50.0, 100.0, 100.0)]));
    let wrong = found(
        &page,
        &[(75.0, 75.0, "c"), (125.0, 75.0, "c"), (25.0, 25.0, "p")],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_box_nobody_can_see_is_still_there() {
    let page = page(
        r#"<div id=a style="position:absolute;left:0;top:0;width:100px;height:100px;opacity:0"></div>"#,
    );
    let wrong = found(&page, &[(50.0, 50.0, "a")]);
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_later_block_over_an_earlier_one_is_found_and_a_positioned_one_over_both() {
    let later = page(
        r#"<div id=a style="height:50px"></div><div id=b style="height:50px;margin-top:-25px"></div>"#,
    );
    let wrong = found(
        &later,
        &[(10.0, 10.0, "a"), (10.0, 30.0, "b"), (10.0, 60.0, "b")],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
    let positioned = page(
        r#"<div id=a style="height:50px;position:relative"></div><div id=b style="height:50px;margin-top:-25px"></div>"#,
    );
    let wrong = found(&positioned, &[(10.0, 30.0, "a")]);
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn an_inline_box_is_found_on_its_own_piece_of_the_line() {
    let page = page(
        r##"<p id=p style="margin:0;font-size:20px;line-height:40px;width:300px">Hello <span id=s>world</span> <a id=l href="#">go</a></p>"##,
    );
    // Chrome's `getClientRects()`.
    assert!(same(
        &pieces(&page, "s"),
        &[(57.0625, 8.0, 55.078_125, 24.0)]
    ));
    // The link starts 0.023 px before Chrome's, because the space before it
    // is 6.336 wide here and 6.359 there (queue item 537).
    let link = pieces(&page, "l");
    assert!(
        link.len() == 1
            && link.iter().all(|&(left, top, width, height)| {
                (left - 118.5).abs() < 0.03
                    && (top - 8.0).abs() < 1.0 / 64.0
                    && (width - 24.9375).abs() < 1.0 / 64.0
                    && (height - 24.0).abs() < 1.0 / 64.0
            }),
        "{link:?}"
    );
    let wrong = found(
        &page,
        &[
            (5.0, 20.0, "p"),
            (5.0, 2.0, "p"),
            (5.0, 38.0, "p"),
            (250.0, 20.0, "p"),
            (70.0, 20.0, "s"),
            (70.0, 2.0, "p"),
            (70.0, 9.0, "s"),
            (70.0, 10.0, "s"),
            (70.0, 31.0, "s"),
            (70.0, 32.0, "p"),
            (70.0, 38.0, "p"),
            (125.0, 20.0, "l"),
        ],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn an_inline_box_is_found_on_whole_pixels() {
    let page = page(
        r#"<p id=p style="margin:0;font-size:20px;line-height:80px;width:300px">Hi <span id=s style="padding:15px;border:2px solid">world</span></p>"#,
    );
    assert!(same(
        &pieces(&page, "s"),
        &[(26.953_125, 11.0, 89.078_125, 58.0)]
    ));
    let wrong = found(
        &page,
        &[
            // The span starts at 26.95, drawn from 27, found from 26.05.
            (26.0, 40.0, "p"),
            (26.05, 40.0, "s"),
            (40.0, 12.0, "s"),
            (115.95, 40.0, "s"),
            (116.0, 40.0, "p"),
            (40.0, 9.95, "p"),
            (40.0, 10.05, "s"),
        ],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn text_is_found_as_the_element_it_is_in() {
    let block =
        page(r#"<div id=d style="font-size:20px;line-height:40px;width:300px">Hello</div>"#);
    let wrong = found(
        &block,
        &[
            (5.0, 20.0, "d"),
            (5.0, 2.0, "d"),
            (200.0, 20.0, "d"),
            (5.0, 41.0, "html"),
        ],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
    // A `display: contents` element has no box, and is what its text finds.
    let contents = page(
        r#"<div id=d style="font-size:20px;line-height:40px;width:300px"><span id=c style="display:contents">Hello</span></div>"#,
    );
    let wrong = found(&contents, &[(5.0, 20.0, "c")]);
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_fields_text_and_its_placeholder_are_the_field() {
    let page = page(
        r#"<input id=f value="hello" style="position:absolute;left:0;top:0;width:200px">
           <input id=h placeholder="hint" style="position:absolute;left:0;top:40px;width:200px">"#,
    );
    assert!(
        same(&pieces(&page, "f"), &[(0.0, 0.0, 208.0, 21.0)]),
        "{:?}",
        pieces(&page, "f")
    );
    assert!(
        same(&pieces(&page, "h"), &[(0.0, 40.0, 208.0, 21.0)]),
        "{:?}",
        pieces(&page, "h")
    );
    let wrong = found(&page, &[(10.0, 10.0, "f"), (10.0, 50.0, "h")]);
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn an_inline_block_is_found_over_the_whole_of_itself() {
    let page = page(
        r#"<div id=d style="font-size:20px;line-height:40px">A<span id=ib style="display:inline-block;width:50px;height:30px"></span>B</div>"#,
    );
    assert!(same(&pieces(&page, "ib"), &[(13.6875, 0.0, 50.0, 30.0)]));
    let wrong = found(
        &page,
        &[(20.0, 20.0, "ib"), (20.0, 2.0, "ib"), (5.0, 20.0, "d")],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn below_and_beside_every_box_is_the_root_and_outside_the_window_nothing() {
    let page = page(
        r#"<style>html{height:100px} body{margin:0;height:50px}</style><div id=a style="height:20px"></div>"#,
    );
    let wrong = found(
        &page,
        &[
            (10.0, 10.0, "a"),
            (10.0, 30.0, "body"),
            (10.0, 70.0, "html"),
            (10.0, 200.0, "html"),
            (-1.0, 10.0, "nothing"),
            (10.0, 300.0, "nothing"),
            (400.0, 10.0, "nothing"),
            (f32::NAN, 10.0, "nothing"),
        ],
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}
