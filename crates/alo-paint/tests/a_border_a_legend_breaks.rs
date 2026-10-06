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
//! the border comes in, and where they stop — for a `solid` border, from the
//! display list; for the `groove` the user-agent sheet gives a fieldset, in
//! pixels, in both its tones; and for `dashed`, `dotted` and `double`, in
//! pixels against the same box with no legend.

use alo_box::build as build_boxes;
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_paint::{Canvas, DisplayList, PaintContext, build, render};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{Font, FontDatabase, Slant, TextMeasurer, Weight};
use alo_value::Rgba;

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

/// A solid border, which is drawn as rectangles that can be counted.
const SOLID: &str = "fieldset { border-style: solid }";

fn draw(html: &str) -> DisplayList {
    draw_with(html, SOLID, Size::new(300.0, 200.0))
}

fn draw_with(html: &str, css: &str, viewport: Size) -> DisplayList {
    let document = parse_document(html);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(css);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build_boxes(&document, &styles);
    let database = fonts();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, viewport, &measurer);
    build::build(
        &boxes,
        &layout,
        &styles,
        PaintContext {
            fonts: &database,
            pictures: &std::collections::BTreeMap::new(),
            drawings: &std::collections::BTreeMap::new(),
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

/// A fieldset 96 wide whose 8px groove of `#808080` has a 40 by 16 legend in
/// it, with no words to get in the way of a pixel.
///
/// The border box runs from 12 across (the body's 10 and the fieldset's own
/// 2) to 108, and from 10 down. The band is the legend's 16 tall, so the
/// stroke is 4 below the top: rows 14 to 22, the outer half 14 to 18 and the
/// inner half 18 to 22. The legend starts at 32 (12, the 8px border and 12px
/// of padding) and ends at 72. The groove's tones are about `#2b2b2b` and
/// `#d5d5d5`.
fn draw_groove(legend: &str) -> Canvas {
    let list = draw_with(
        "<body><fieldset><legend></legend><div></div></fieldset></body>",
        &format!(
            "body {{ margin: 10px }}
             fieldset {{ border-width: 8px; border-color: #808080; padding: 0 12px 4px }}
             legend {{ width: 40px; height: 16px; padding: 0; {legend} }}
             div {{ height: 20px }}"
        ),
        Size::new(120.0, 70.0),
    );
    let mut canvas = Canvas::new(120, 70, Rgba::WHITE);
    render(&list, &mut canvas);
    canvas
}

/// What a pixel is: `D` (the darker tone), `L` (the lighter one), `W` (the
/// page) or `?` (anything else, which no assertion here expects).
fn tone(canvas: &Canvas, x: u32, y: u32) -> char {
    match canvas.at(x, y).map(Rgba::to_rgba8) {
        Some((red, green, blue, _)) if red == green && green == blue => match red {
            0..=60 => 'D',
            190..=230 => 'L',
            255 => 'W',
            _ => '?',
        },
        _ => '?',
    }
}

/// The block-start border at one point across: its outer half, then its
/// inner half.
fn across(canvas: &Canvas, x: u32) -> (char, char) {
    (tone(canvas, x, 15), tone(canvas, x, 20))
}

#[test]
fn a_groove_is_drawn_in_both_tones_either_side_of_the_legend() {
    let canvas = draw_groove("");
    // Cut in: dark outside and light inside, before the legend and after it.
    assert_eq!(across(&canvas, 25), ('D', 'L'), "before the legend");
    assert_eq!(across(&canvas, 90), ('D', 'L'), "after it");
    // And nothing where the legend is, across the stroke's whole depth.
    for x in [33, 50, 70] {
        assert_eq!(across(&canvas, x), ('W', 'W'), "in the gap at {x}");
    }
}

#[test]
fn the_gap_ends_exactly_where_the_legend_does() {
    let canvas = draw_groove("");
    assert_eq!(across(&canvas, 31), ('D', 'L'), "the last pixel before it");
    assert_eq!(across(&canvas, 32), ('W', 'W'), "the legend's first");
    assert_eq!(across(&canvas, 71), ('W', 'W'), "the legend's last");
    assert_eq!(across(&canvas, 72), ('D', 'L'), "the first pixel after it");
}

#[test]
fn the_other_three_sides_are_a_groove_from_the_line_down() {
    let canvas = draw_groove("");
    // The left is dark outside and light inside, like the top; the right and
    // the bottom are the other way round, which is what makes it a groove.
    assert_eq!((tone(&canvas, 14, 40), tone(&canvas, 18, 40)), ('D', 'L'));
    assert_eq!((tone(&canvas, 106, 40), tone(&canvas, 102, 40)), ('L', 'D'));
    assert_eq!((tone(&canvas, 60, 56), tone(&canvas, 60, 52)), ('L', 'D'));
    // Starting at the line through the legend, not at the top of the box.
    assert_eq!(tone(&canvas, 14, 12), 'W', "above the line");
    assert_eq!(tone(&canvas, 106, 12), 'W', "above the line");
}

#[test]
fn a_legend_wider_than_the_box_leaves_the_far_corner_drawn() {
    // 120 wide from 32 is past the fieldset's right edge at 108, over the
    // whole of the right border. The border's corner stays; the legend takes
    // only the top side's part of the line.
    let canvas = draw_groove("width: 120px");
    assert_eq!(across(&canvas, 90), ('W', 'W'), "the legend's part");
    assert_eq!(across(&canvas, 99), ('W', 'W'), "up to the right border");
    assert_eq!(
        tone(&canvas, 101, 15),
        'D',
        "the corner, the top's outer half inside the right border"
    );
    assert_eq!(
        tone(&canvas, 106, 21),
        'L',
        "and the right's outer half below it"
    );
}

#[test]
fn the_user_agent_sheet_gives_a_fieldset_a_groove() {
    // `2px groove #c0c0c0`: two tones of that grey, and never the grey itself,
    // which is what the border was drawn in while it was a stand-in `solid`.
    let outline = draw_with(
        "<body><fieldset><legend>Size</legend><p>one</p></fieldset></body>",
        "",
        Size::new(300.0, 200.0),
    )
    .to_outline();
    let fills: Vec<&str> = outline
        .lines()
        .filter(|line| line.starts_with("fill "))
        .collect();
    assert!(
        fills.iter().any(|line| line.contains("rgb(107 107 107)")),
        "the darker tone: {fills:?}",
    );
    assert!(
        fills.iter().any(|line| line.contains("rgb(255 255 255)")),
        "the lighter tone: {fills:?}",
    );
    assert!(
        !outline.contains("rgb(192 192 192)"),
        "and not the grey itself: {outline}",
    );
}

/// A fieldset like [`draw_groove`]'s with a 6px border of `#333333` in
/// `style`, with or without its 40 by 16 legend.
///
/// The border box runs from 12 across to 108 and from 10 down. With the
/// legend the stroke is 5 below the top, rows 15 to 21, and the legend is 30
/// (12, the 6px border and 12px of padding) to 70 across; without it the
/// stroke is along the top, rows 10 to 16.
fn draw_patterned(style: &str, legend: bool) -> Canvas {
    let list = draw_with(
        if legend {
            "<body><fieldset><legend></legend><div></div></fieldset></body>"
        } else {
            "<body><fieldset><div></div></fieldset></body>"
        },
        &format!(
            "body {{ margin: 10px }}
             fieldset {{ border: 6px {style} #333333; padding: 0 12px 4px }}
             legend {{ width: 40px; height: 16px; padding: 0 }}
             div {{ height: 20px }}"
        ),
        Size::new(120.0, 70.0),
    );
    let mut canvas = Canvas::new(120, 70, Rgba::WHITE);
    render(&list, &mut canvas);
    canvas
}

/// Whether a pixel has any of the border in it.
fn inked(canvas: &Canvas, x: u32, y: u32) -> bool {
    canvas.at(x, y).map(Rgba::to_rgba8) != Some((255, 255, 255, 255))
}

#[test]
fn a_pattern_is_the_whole_sides_cut_where_the_legend_is() {
    // The block-start side beside the legend is the side the same box draws
    // with no legend at all, to the pixel: its dashes, dots or lines spaced
    // on the whole side, from corner to corner. Only where the legend is,
    // across the stroke's whole depth, is there nothing.
    for style in ["dashed", "dotted", "double"] {
        let broken = draw_patterned(style, true);
        let whole = draw_patterned(style, false);
        for depth in 0..6 {
            for x in 0..120 {
                let here = broken.at(x, 15 + depth).map(Rgba::to_rgba8);
                if (30..70).contains(&x) {
                    assert_eq!(
                        here,
                        Some((255, 255, 255, 255)),
                        "{style}: nothing where the legend is, at {x}, {depth} down",
                    );
                } else {
                    assert_eq!(
                        here,
                        whole.at(x, 10 + depth).map(Rgba::to_rgba8),
                        "{style}: the whole side's pattern at {x}, {depth} down",
                    );
                }
            }
        }
    }
}

#[test]
fn a_patterned_border_is_drawn_either_side_of_the_legend() {
    // Not left undrawn, which it was while only `solid` and the two-toned
    // styles were cut round a legend.
    for style in ["dashed", "dotted", "double"] {
        let canvas = draw_patterned(style, true);
        let inked_in = |columns: core::ops::Range<u32>| {
            columns
                .flat_map(|x| (15..21).map(move |y| (x, y)))
                .any(|(x, y)| inked(&canvas, x, y))
        };
        assert!(inked_in(18..30), "{style}: before the legend");
        assert!(inked_in(70..102), "{style}: after it");
        // And the other three sides, from the line down.
        assert!(inked_in(12..18), "{style}: the corner");
        assert!(
            (21..56).any(|y| inked(&canvas, 14, y)),
            "{style}: the left side"
        );
        assert!(
            (12..108).any(|x| inked(&canvas, x, 53)),
            "{style}: the bottom"
        );
    }
}

#[test]
fn a_dash_or_dot_the_legends_edge_falls_on_is_cut_there() {
    // With this box's spacing a dot runs from 68 to 74 across: it is cut at
    // the legend's end, 70, rather than moved clear of it or dropped.
    let whole = draw_patterned("dotted", false);
    let broken = draw_patterned("dotted", true);
    assert!(inked(&whole, 69, 13), "the dot, with no legend");
    assert!(!inked(&broken, 69, 18), "the legend's part of it");
    assert!(inked(&broken, 71, 18), "and the rest");
}
