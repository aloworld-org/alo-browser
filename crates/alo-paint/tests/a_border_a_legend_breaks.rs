/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `<fieldset>`'s border, and the hole its legend leaves in it.
//!
//! Every other border in CSS goes all the way round. A fieldset showing a
//! legend has one that does not: the legend sits **in** the block-start border
//! rather than above it, and the border is not drawn behind it. That is what
//! makes a group of controls look like a group with a name written into the
//! line around it, and it is the only reason a fieldset is worth using.
//!
//! The numbers are asserted in `alo-layout`'s `numbers.rs`, which is where a
//! layout assertion belongs. This is about what is **drawn**: how many pieces
//! the border comes in, and where they stop.

use alo_box::build as build_boxes;
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_paint::{DisplayList, PaintContext, build};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{Font, FontDatabase, Slant, TextMeasurer, Weight};

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
    database.map_generic("system-ui", "DejaVu Sans");
    database
}

fn draw(html: &str) -> DisplayList {
    let document = parse_document(html);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet("");
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build_boxes(&document, &styles);
    let database = fonts();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, Size::new(300.0, 200.0), &measurer);
    build::build(
        &boxes,
        &layout,
        &styles,
        PaintContext {
            fonts: &database,
            pictures: &std::collections::BTreeMap::new(),
        },
    )
}

/// Every rectangle filled in the colour the user-agent sheet gives a fieldset,
/// as `x y width height`.
fn border_pieces(list: &DisplayList) -> Vec<(f32, f32, f32, f32)> {
    list.to_outline()
        .lines()
        .filter(|line| line.starts_with("fill") && line.contains("rgb(192 192 192)"))
        .filter_map(|line| {
            let (_, at) = line.split_once(" at (")?;
            let (origin, size) = at.split_once(") ")?;
            let (x, y) = origin.split_once(", ")?;
            let (width, height) = size.split_once('×')?;
            Some((
                x.parse().ok()?,
                y.parse().ok()?,
                width.parse().ok()?,
                height.parse().ok()?,
            ))
        })
        .collect()
}

/// The pieces of the block-start border, left to right.
///
/// The rectangles that are the border's own thickness *tall* — which is what
/// tells a horizontal side from a vertical one — and highest on the page,
/// which is what tells the block-start border from the block-end one.
fn across_the_top(pieces: &[(f32, f32, f32, f32)]) -> Vec<(f32, f32, f32, f32)> {
    let horizontal: Vec<(f32, f32, f32, f32)> = pieces
        .iter()
        .copied()
        .filter(|(_, _, _, height)| (*height - 2.0).abs() < 0.001)
        .collect();
    let top = horizontal
        .iter()
        .map(|(_, y, _, _)| *y)
        .fold(f32::INFINITY, f32::min);
    let mut across: Vec<(f32, f32, f32, f32)> = horizontal
        .into_iter()
        .filter(|(_, y, _, _)| (*y - top).abs() < 0.001)
        .collect();
    across.sort_by(|left, right| left.0.total_cmp(&right.0));
    across
}

#[test]
fn the_border_is_drawn_in_the_two_pieces_the_legend_leaves() {
    let list = draw("<body><fieldset><legend>Size</legend><p>one</p></fieldset></body>");
    let pieces = border_pieces(&list);
    assert_eq!(
        pieces.len(),
        5,
        "three whole sides and a block-start border in two pieces: {pieces:?}",
    );

    let across = across_the_top(&pieces);
    let [
        (left_x, _, left_width, left_height),
        (right_x, _, _, right_height),
    ] = across[..]
    else {
        panic!("the block-start border is in two pieces: {across:?}");
    };
    assert!(
        (left_height - 2.0).abs() < 0.001 && (right_height - 2.0).abs() < 0.001,
        "each as thick as the border the sheet asked for: {across:?}",
    );
    assert!(
        right_x > left_x + left_width,
        "with a gap between them, which is where the legend is: {across:?}",
    );
}

#[test]
fn the_gap_is_exactly_where_the_legend_is() {
    let short = border_pieces(&draw(
        "<body><fieldset><legend>Size</legend><p>one</p></fieldset></body>",
    ));
    let long = border_pieces(&draw(
        "<body><fieldset><legend>Size of the pizza</legend><p>one</p></fieldset></body>",
    ));
    let gap_of = |pieces: &[(f32, f32, f32, f32)]| {
        let across = across_the_top(pieces);
        across[1].0 - (across[0].0 + across[0].2)
    };
    assert!(
        gap_of(&long) > gap_of(&short) + 50.0,
        "a longer legend leaves a longer hole: {} against {}",
        gap_of(&long),
        gap_of(&short),
    );
}

#[test]
fn a_fieldset_with_no_legend_has_an_unbroken_border() {
    let list = draw("<body><fieldset><p>one</p></fieldset></body>");
    let pieces = border_pieces(&list);
    assert_eq!(
        pieces.len(),
        1,
        "one width and one colour all the way round is a ring, and a ring is \
         one shape: {pieces:?}",
    );
}

#[test]
fn the_border_is_drawn_through_the_middle_of_the_legend() {
    // Not along the top of it, which is where an ordinary block-start border
    // would be — the line goes through the legend's words, and the legend is
    // what hides the part of it that would cross them.
    let list = draw("<body><fieldset><legend>Size</legend><p>one</p></fieldset></body>");
    let pieces = border_pieces(&list);
    let top = pieces
        .iter()
        .filter(|(_, _, _, height)| (*height - 2.0).abs() < 0.001)
        .map(|(_, y, _, _)| *y)
        .fold(f32::INFINITY, f32::min);
    let legend_top = 8.0; // the fieldset's own top: `body { margin: 8px }`.
    assert!(
        top > legend_top + 4.0,
        "the border is well below the top of the legend: {top}",
    );
    let text = list
        .to_outline()
        .lines()
        .find(|line| line.contains("\"Size\""))
        .map(ToOwned::to_owned)
        .expect("the legend's text is drawn");
    assert!(
        text.contains("at (") && !text.is_empty(),
        "and the legend is drawn over it: {text}",
    );
}
