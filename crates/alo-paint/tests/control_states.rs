/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a control draws for its own state, asserted rather than pictured.
//!
//! The committed pictures are `alo-corpus`'s `control-states` case, which is
//! where a reference render belongs, and `toggle-artwork`, which is compared
//! with Chrome's screenshot pixel by pixel. What is here is the half a
//! picture cannot say: *why* it is right. An image can only tell you that a
//! checkbox changed; these say the ink inside it is the accent colour, that
//! a radio's is round and a checkbox's is not, and that a disabled control
//! draws its state in a colour that says it is not live.
//!
//! Every one of these was true and invisible before queue item 182: the agent
//! tree said `checkbox "Remember me" [checked=true]` and the picture showed an
//! empty square.

use alo_box::build as build_boxes;
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_paint::control::DEFAULT_ACCENT;
use alo_paint::toggle::{DISABLED_EDGE, DISABLED_INSIDE, EDGE};
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

/// How large a page one control is drawn on. Wide enough for the widest
/// control here and no wider, so a stray mark has nowhere to hide.
const WIDTH: u16 = 60;
const HEIGHT: u16 = 40;

fn draw(html: &str, css: &str) -> (DisplayList, Canvas) {
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
    let layout = compute(
        &boxes,
        &styles,
        Size::new(f32::from(WIDTH), f32::from(HEIGHT)),
        &measurer,
    );
    let list = build::build(
        &boxes,
        &layout,
        &styles,
        PaintContext {
            fonts: &database,
            pictures: &std::collections::BTreeMap::new(),
            drawings: &std::collections::BTreeMap::new(),
        },
    );

    let mut canvas = Canvas::new(u32::from(WIDTH), u32::from(HEIGHT), Rgba::WHITE);
    render(&list, &mut canvas);
    (list, canvas)
}

/// The page a single control is put on. No background of its own, so that
/// every fill in the list belongs to the control. Its margin is the page's,
/// so that it stands at the body's corner, 16×16, whatever the user-agent
/// sheet gives a checkbox (queue item 443).
const PAGE: &str = "body { margin: 8px }\n#c { width: 16px; height: 16px; margin: 0 }";

/// One control on an otherwise empty page, at a known place.
fn control(attributes: &str) -> (DisplayList, Canvas) {
    draw(
        &format!("<!DOCTYPE html><html><body><input id=c {attributes}></body></html>"),
        PAGE,
    )
}

/// What is inside a checkbox's or a radio's one-pixel edge: the 16×16 the
/// page asked for, at the body's margin, less the edge (queue item 454).
const INSIDE: (u32, u32, u32, u32) = (9, 9, 23, 23);

/// The top third of it, which a tick reaches and a dash does not.
const UPPER: (u32, u32, u32, u32) = (9, 9, 23, 13);

/// What a translucent colour reads as over the white page.
fn over_white(colour: Rgba) -> Rgba {
    let blend = |channel: f32| channel * colour.alpha + (1.0 - colour.alpha);
    Rgba::new(
        blend(colour.red),
        blend(colour.green),
        blend(colour.blue),
        1.0,
    )
}

/// How many things the list fills, which is the count that changes when a
/// control starts drawing its state.
fn fills(list: &DisplayList) -> usize {
    list.to_outline()
        .lines()
        .filter(|line| line.trim_start().starts_with("fill "))
        .count()
}

/// How many pixels in an area are a given colour.
///
/// A count rather than a sample, because the shape of a mark is the thing
/// being tested and a single pixel would pin it to one arbitrary point of it.
fn pixels(canvas: &Canvas, area: (u32, u32, u32, u32), colour: Rgba) -> usize {
    let (left, top, right, bottom) = area;
    (left..right)
        .flat_map(|x| (top..bottom).map(move |y| (x, y)))
        .filter(|(x, y)| canvas.at(*x, *y).is_some_and(|pixel| close(pixel, colour)))
        .count()
}

/// How many pixels in an area are nearer to one colour than to another.
///
/// The mark is thin — two and a half pixels across at the size a page usually
/// asks for — so most of it is anti-aliased and no pixel of it is exactly the
/// mark colour. Counting which of two colours a pixel is *closer to* is what
/// measures a shape rather than a flat fill.
fn nearer(canvas: &Canvas, area: (u32, u32, u32, u32), this: Rgba, than: Rgba) -> usize {
    let (left, top, right, bottom) = area;
    (left..right)
        .flat_map(|x| (top..bottom).map(move |y| (x, y)))
        .filter(|(x, y)| {
            canvas
                .at(*x, *y)
                .is_some_and(|pixel| distance(pixel, this) < distance(pixel, than))
        })
        .count()
}

fn distance(a: Rgba, b: Rgba) -> f32 {
    (a.red - b.red).powi(2) + (a.green - b.green).powi(2) + (a.blue - b.blue).powi(2)
}

fn close(a: Rgba, b: Rgba) -> bool {
    (a.red - b.red).abs() < 0.02
        && (a.green - b.green).abs() < 0.02
        && (a.blue - b.blue).abs() < 0.02
}

#[test]
fn an_unchecked_checkbox_draws_nothing_inside_itself() {
    let (list, canvas) = control("type=checkbox");
    // Its white inside and its grey edge, and nothing else: no accent, no
    // mark.
    assert_eq!(fills(&list), 2, "{}", list.to_outline());
    assert_eq!(
        pixels(&canvas, INSIDE, DEFAULT_ACCENT),
        0,
        "an unchecked box has no accent in it",
    );
    assert!(
        pixels(&canvas, INSIDE, Rgba::WHITE) >= 14 * 14 - 4,
        "it is white all the way across, but for where its inner corners round",
    );
    let edge = canvas
        .at(8, 16)
        .expect("the middle of the control's left edge");
    assert!(close(edge, EDGE), "inside a grey edge: {edge}");
}

#[test]
fn a_checked_checkbox_fills_with_the_accent_and_marks_it() {
    let (list, canvas) = control("type=checkbox checked");
    assert_eq!(
        fills(&list),
        2,
        "the accent and the tick:\n{}",
        list.to_outline(),
    );
    let accent = pixels(&canvas, INSIDE, DEFAULT_ACCENT);
    let mark = nearer(&canvas, INSIDE, Rgba::WHITE, DEFAULT_ACCENT);
    assert!(accent > 100, "most of it is the accent, found {accent}");
    assert!(mark > 20, "and a tick is drawn on it, found {mark} pixels");
}

#[test]
fn the_three_states_of_a_checkbox_are_three_different_pictures() {
    // The closing condition of queue item 182, as pixels: a person can tell
    // which of the three a box is in without being told.
    let (_, off) = control("type=checkbox");
    let (_, on) = control("type=checkbox checked");
    let (_, mixed) = control("type=checkbox aria-checked=mixed");

    assert_eq!(
        pixels(&off, INSIDE, DEFAULT_ACCENT),
        0,
        "an unchecked box is empty",
    );
    // What tells `on` from `mixed` is the shape of the mark: a tick rises into
    // the top of the box and a dash stays on its middle line.
    let tick_above = nearer(&on, UPPER, Rgba::WHITE, DEFAULT_ACCENT);
    let dash_above = nearer(&mixed, UPPER, Rgba::WHITE, DEFAULT_ACCENT);
    assert!(tick_above > 3, "a tick reaches upwards, found {tick_above}");
    assert_eq!(
        dash_above, 0,
        "a dash does not, so the two are different pictures",
    );
    // Chrome's bar is 0.62 of the control across and 0.15 of it down: at
    // 16 pixels, about ten by two and a half.
    assert!(
        nearer(&mixed, INSIDE, Rgba::WHITE, DEFAULT_ACCENT) > 12,
        "and the dash is still drawn",
    );
}

#[test]
fn a_checked_radio_is_round_where_a_checked_checkbox_is_not() {
    let (_, radio) = control("type=radio checked");
    let (_, checkbox) = control("type=checkbox checked");
    // A pixel in from the very corner of the control: a radio is a circle
    // and the page shows past it, a checkbox is a square rounded only two
    // pixels and it does not.
    let corner = |canvas: &Canvas| canvas.at(9, 9).expect("the control's top-left corner");
    assert!(
        corner(&radio).red > 0.8,
        "a radio's corner is still the page: {}",
        corner(&radio),
    );
    assert!(
        corner(&checkbox).red < 0.6,
        "a checkbox's corner is the control: {}",
        corner(&checkbox),
    );
    // And the dot is drawn in the middle of it, with white around it.
    let middle = radio.at(16, 16).expect("the middle of the radio");
    assert!(close(middle, DEFAULT_ACCENT), "the dot is there: {middle}");
    let around = radio
        .at(16, 10)
        .expect("between the radio's edge and its dot");
    assert!(close(around, Rgba::WHITE), "inside a white ring: {around}");
}

#[test]
fn a_disabled_control_still_says_what_state_it_is_in() {
    let (list, canvas) = control("type=checkbox checked disabled");
    assert_eq!(
        fills(&list),
        3,
        "a disabled control draws its state like any other, inside its edge:\n{}",
        list.to_outline(),
    );
    let grey = over_white(DISABLED_EDGE.over(DISABLED_INSIDE));
    assert!(
        pixels(&canvas, INSIDE, grey) > 100,
        "in the colour that says it is not live",
    );
    assert_eq!(
        pixels(&canvas, INSIDE, DEFAULT_ACCENT),
        0,
        "and not in the live one, which a page cannot make it use",
    );
    assert!(
        nearer(&canvas, UPPER, Rgba::WHITE, grey) > 3,
        "with the same tick on it, because it is still checked",
    );
}

#[test]
fn a_disabled_control_that_is_off_is_not_the_same_picture_as_a_live_one() {
    // "You cannot change this" and "this is off" are different things to be
    // told. The mark cannot say the first, because an unchecked control has
    // none — so the edge does, in Chrome's paler, translucent grey.
    let (_, live) = control("type=checkbox");
    let (_, dead) = control("type=checkbox disabled");
    let edge = |canvas: &Canvas| canvas.at(14, 8).expect("the control's top edge");
    assert!(
        edge(&dead).red > edge(&live).red + 0.1,
        "a disabled control's edge is paler: {} against {}",
        edge(&dead),
        edge(&live),
    );
}

/// How much of the page one pixel of a control hides, from that pixel over
/// a white page and over a black one: a colour of alpha `a` lets through
/// `1 − a` of the page, which is the whole of the difference between the two.
fn opacity(over_white: &Canvas, over_black: &Canvas, x: u32, y: u32) -> Option<f32> {
    Some(1.0 - (over_white.at(x, y)?.red - over_black.at(x, y)?.red))
}

#[test]
fn a_disabled_control_never_lets_the_page_through_between_its_pieces() {
    // Queues 463 and 578. A disabled control's inside and edge are
    // translucent. Drawn side by side, each smoothed on its own, a pixel half
    // covered by each lets the page through both halves, and over black the
    // seam showed as a darker ring inside the edge. Chrome draws the edge
    // over the inside, so where they cross they hide more of the page than
    // the inside alone (0.79 against 0.68), and past them only the edge's
    // 0.30. So from the middle outwards no pixel hides less of the page than
    // the inside does until the last that hides as much: nothing dips there
    // and recovers. A checked checkbox is left out: it is filled edge to
    // edge, and its tick hides more than the fill it crosses.
    for attributes in [
        "type=radio disabled",
        "type=radio checked disabled",
        "type=checkbox disabled",
    ] {
        let (_, white) = control(attributes);
        let (_, black) = draw(
            &format!("<!DOCTYPE html><html><body><input id=c {attributes}></body></html>"),
            &format!("html {{ background: #000000 }}\n{PAGE}"),
        );
        // The control is 16 by 16 at (8, 8); its middle is between 15 and 16.
        let rays = (8..24u32).flat_map(|line| {
            [
                (15..24u32).map(move |x| (x, line)).collect::<Vec<_>>(),
                (8..=16u32).rev().map(move |x| (x, line)).collect(),
                (15..24u32).map(move |y| (line, y)).collect(),
                (8..=16u32).rev().map(move |y| (line, y)).collect(),
            ]
        });
        let inside = DISABLED_INSIDE.alpha - 2.0 / 255.0;
        for ray in rays {
            let hidden = ray
                .iter()
                .map(|&(x, y)| {
                    let Some(here) = opacity(&white, &black, x, y) else {
                        panic!("({x}, {y}) is on the page");
                    };
                    here
                })
                .collect::<Vec<_>>();
            let Some(last) = hidden.iter().rposition(|&here| here >= inside) else {
                continue;
            };
            for (&(x, y), &here) in ray.iter().zip(&hidden).take(last) {
                assert!(
                    here >= inside,
                    "{attributes}: ({x}, {y}) hides {here} of the page, less than the inside's {}, inside pixels that hide that much",
                    DISABLED_INSIDE.alpha
                );
            }
        }
    }
}

#[test]
fn a_page_may_choose_the_colour_and_the_mark_stays_readable() {
    // `accent-color` is the property CSS has for this, and a pale one turns
    // the mark black — a fixed white tick would have vanished into it.
    let pale = Rgba::new(0.722, 0.941, 0.0, 1.0);
    let (_, canvas) = draw(
        "<!DOCTYPE html><html><body><input id=c type=checkbox checked></body></html>",
        "body { margin: 8px }\n\
         #c { width: 16px; height: 16px; margin: 0; accent-color: #b8f000 }",
    );
    assert!(
        pixels(&canvas, INSIDE, pale) > 100,
        "the page's own accent is used",
    );
    assert!(
        nearer(&canvas, INSIDE, Rgba::BLACK, pale) > 20,
        "and the mark is black against it rather than invisible",
    );
    assert_eq!(
        pixels(&canvas, INSIDE, Rgba::WHITE),
        0,
        "which is the whole point: a white tick would have vanished",
    );
}

#[test]
fn a_control_the_page_made_large_still_draws_a_mark_it_can_hold() {
    // A stretched tick is not the same symbol. The control is drawn in the
    // largest square that fits, as Chrome draws it, so a wide checkbox is a
    // 16-pixel checkbox in the middle of a box with the page either side.
    let (_, canvas) = draw(
        "<!DOCTYPE html><html><body><input id=c type=checkbox checked></body></html>",
        "body { margin: 8px }\n#c { width: 40px; height: 16px; margin: 0 }",
    );
    // The box runs from x=8 to x=48, and its square from 20 to 36.
    let ends = (8, 8, 20, 24);
    assert_eq!(
        pixels(&canvas, ends, DEFAULT_ACCENT),
        0,
        "no accent near the left end of a wide control",
    );
    assert_eq!(
        pixels(&canvas, ends, Rgba::WHITE),
        12 * 16,
        "which is the page",
    );
    assert!(
        pixels(&canvas, (21, 9, 35, 23), DEFAULT_ACCENT) > 100,
        "and the square in the middle is the checkbox",
    );
}

#[test]
fn a_box_an_author_made_a_checkbox_marks_its_state_inside_its_own_border() {
    // Not an `<input>`: it has no picture of its own, so its state is drawn
    // inside the border its author gave it, over its padding box.
    let (list, canvas) = draw(
        "<!DOCTYPE html><html><body><div id=c role=checkbox aria-checked=true></div></body></html>",
        "body { margin: 8px }\n#c { width: 16px; height: 16px; border: 1px solid #767676 }",
    );
    assert_eq!(
        fills(&list),
        3,
        "the author's border, the accent and the tick:\n{}",
        list.to_outline(),
    );
    let edge = canvas.at(8, 16).expect("the box's left border");
    assert!(close(edge, EDGE), "the author's border stays: {edge}");
    assert!(pixels(&canvas, (9, 9, 25, 25), DEFAULT_ACCENT) > 100);
}

#[test]
fn something_that_is_not_a_control_draws_no_mark() {
    let (list, _) = draw(
        "<!DOCTYPE html><html><body><div id=c>text</div></body></html>",
        "body { margin: 8px }\n#c { width: 16px; height: 16px; background: #ffffff }",
    );
    assert_eq!(
        fills(&list),
        1,
        "just its background:\n{}",
        list.to_outline()
    );
}
