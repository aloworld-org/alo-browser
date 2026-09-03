//! What a control draws for its own state, asserted rather than pictured.
//!
//! The committed picture is `alo-corpus`'s `control-states` case, which is
//! where a reference render belongs. What is here is the half a picture cannot
//! say: *why* it is right. An image can only tell you that a checkbox changed;
//! these say the ink inside it is the accent colour, that a radio's is round
//! and a checkbox's is not, and that a disabled control draws its state in a
//! colour that says it is not live.
//!
//! Every one of these was true and invisible before queue item 182: the agent
//! tree said `checkbox "Remember me" [checked=true]` and the picture showed an
//! empty square.

use alo_box::build as build_boxes;
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_paint::control::{DEFAULT_ACCENT, DISABLED_ACCENT};
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
        },
    );

    let mut canvas = Canvas::new(u32::from(WIDTH), u32::from(HEIGHT), Rgba::WHITE);
    render(&list, &mut canvas);
    (list, canvas)
}

/// The page a single control is put on. No background of its own, so that
/// every fill in the list belongs to the control.
const PAGE: &str = "body { margin: 8px }\n#c { width: 16px; height: 16px }";

/// One control on an otherwise empty page, at a known place.
fn control(attributes: &str) -> (DisplayList, Canvas) {
    draw(
        &format!("<!DOCTYPE html><html><body><input id=c {attributes}></body></html>"),
        PAGE,
    )
}

/// The control's **padding box**, which is where a mark is drawn: the 16×16
/// the page asked for, inside the one-pixel border, at the body's margin.
const INSIDE: (u32, u32, u32, u32) = (9, 9, 25, 25);

/// The top third of it, which a tick reaches and a dash does not.
const UPPER: (u32, u32, u32, u32) = (9, 9, 25, 14);

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
    // The border, and nothing else: no accent, no mark.
    assert_eq!(fills(&list), 1, "{}", list.to_outline());
    assert_eq!(
        pixels(&canvas, INSIDE, DEFAULT_ACCENT),
        0,
        "an unchecked box has no accent in it",
    );
    assert_eq!(
        pixels(&canvas, INSIDE, Rgba::WHITE),
        16 * 16,
        "it is the page all the way across",
    );
}

#[test]
fn a_checked_checkbox_fills_with_the_accent_and_marks_it() {
    let (list, canvas) = control("type=checkbox checked");
    assert_eq!(
        fills(&list),
        3,
        "the border, the accent and the tick:\n{}",
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
    assert!(
        nearer(&mixed, INSIDE, Rgba::WHITE, DEFAULT_ACCENT) > 20,
        "and the dash is still drawn",
    );
}

#[test]
fn a_checked_radio_is_round_where_a_checked_checkbox_is_not() {
    let (_, radio) = control("type=radio checked");
    let (_, checkbox) = control("type=checkbox checked");
    // The very corner of the control: a radio is a circle and the page shows
    // past it, a checkbox is a square and it does not.
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
    // And the dot is drawn in the middle of it either way round.
    let middle = radio.at(16, 16).expect("the middle of the radio");
    assert!(close(middle, Rgba::WHITE), "the dot is there: {middle}");
}

#[test]
fn a_disabled_control_still_says_what_state_it_is_in() {
    let (list, canvas) = control("type=checkbox checked disabled");
    assert_eq!(
        fills(&list),
        3,
        "a disabled control draws its state like any other:\n{}",
        list.to_outline(),
    );
    assert!(
        pixels(&canvas, INSIDE, DISABLED_ACCENT) > 100,
        "in the colour that says it is not live",
    );
    assert_eq!(
        pixels(&canvas, INSIDE, DEFAULT_ACCENT),
        0,
        "and not in the live one, which a page cannot make it use",
    );
    assert!(
        nearer(&canvas, UPPER, Rgba::WHITE, DISABLED_ACCENT) > 3,
        "with the same tick on it, because it is still checked",
    );
}

#[test]
fn a_disabled_control_that_is_off_is_not_the_same_picture_as_a_live_one() {
    // "You cannot change this" and "this is off" are different things to be
    // told. The mark cannot say the first, because an unchecked control has
    // none — so the border does, from the user-agent sheet.
    let (_, live) = control("type=checkbox");
    let (_, dead) = control("type=checkbox disabled");
    let edge = |canvas: &Canvas| canvas.at(14, 8).expect("the control's top edge");
    assert!(
        edge(&dead).red > edge(&live).red + 0.1,
        "a disabled control's border is paler: {} against {}",
        edge(&dead),
        edge(&live),
    );
}

#[test]
fn a_page_may_choose_the_colour_and_the_mark_stays_readable() {
    // `accent-color` is the property CSS has for this, and a pale one turns
    // the mark black — a fixed white tick would have vanished into it.
    let pale = Rgba::new(0.722, 0.941, 0.0, 1.0);
    let (_, canvas) = draw(
        "<!DOCTYPE html><html><body><input id=c type=checkbox checked></body></html>",
        "body { margin: 8px }\n\
         #c { width: 16px; height: 16px; accent-color: #b8f000 }",
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
    // A stretched tick is not the same symbol. The mark is drawn in the
    // largest square that fits, so a wide checkbox has wide empty margins
    // rather than a wide tick.
    let (_, canvas) = draw(
        "<!DOCTYPE html><html><body><input id=c type=checkbox checked></body></html>",
        "body { margin: 8px }\n#c { width: 40px; height: 16px }",
    );
    // The control's padding box runs from x=9 to x=49; its mark sits in the
    // 16-wide square in the middle of that, so both ends are bare accent.
    let ends = (9, 9, 20, 25);
    assert_eq!(
        pixels(&canvas, ends, Rgba::WHITE),
        0,
        "no mark near the left end of a wide control",
    );
    assert!(
        pixels(&canvas, ends, DEFAULT_ACCENT) > 100,
        "which is accent rather than page: a wide control is still filled",
    );
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
