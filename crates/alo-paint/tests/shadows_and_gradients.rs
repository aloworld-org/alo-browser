//! What a shadow and a gradient do to the pixels, asserted rather than
//! pictured.
//!
//! The committed pictures live in `alo-corpus`. What is here is the half a
//! picture cannot say: a shadow is *softer further out* and a gradient is
//! *monotone from one end to the other*, and those are properties an image can
//! only differ about.

use alo_box::build as build_boxes;
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_paint::{Canvas, DisplayList, PaintContext, build, render};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{Font, FontDatabase, Slant, TextMeasurer, Weight};
use alo_value::Rgba;

const WIDTH: u32 = 140;
const HEIGHT: u32 = 100;

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
    let layout = compute(&boxes, &styles, Size::new(140.0, 100.0), &measurer);
    let list = build::build(
        &boxes,
        &layout,
        &styles,
        PaintContext {
            fonts: &database,
            pictures: &std::collections::BTreeMap::new(),
        },
    );

    let mut canvas = Canvas::new(WIDTH, HEIGHT, Rgba::WHITE);
    render(&list, &mut canvas);
    (list, canvas)
}

/// How dark a pixel is, from nothing to one.
///
/// A pixel off the canvas reads as less than nothing, so a test that asked
/// about one fails on the assertion rather than passing quietly.
fn darkness(canvas: &Canvas, x: u32, y: u32) -> f32 {
    canvas.at(x, y).map_or(-1.0, |pixel| 1.0 - pixel.red)
}

const CARD: &str = "<!DOCTYPE html><html><body><div id=card></div></body></html>";

#[test]
fn a_shadow_is_darkest_at_the_shape_and_fades_outwards() {
    let (list, canvas) = draw(
        CARD,
        "body { margin: 20px; background: #ffffff }
         #card { width: 60px; height: 40px; background: #ffffff;
                 box-shadow: 0 0 10px #000000 }",
    );
    assert!(
        list.to_outline().contains("shadow"),
        "the list says there is a shadow:\n{}",
        list.to_outline(),
    );

    // Straight up from the middle of the card's top edge: just outside it is
    // dark, and it fades to nothing further out.
    let x = 50;
    let against = darkness(&canvas, x, 19);
    let a_little_out = darkness(&canvas, x, 14);
    let far_out = darkness(&canvas, x, 6);
    assert!(
        against > a_little_out && a_little_out > far_out,
        "a shadow fades outwards: {against}, then {a_little_out}, then {far_out}",
    );
    assert!(against > 0.2, "and it is visible against the shape");
    assert!(far_out < 0.05, "and gone by the time it is ten pixels out");
}

#[test]
fn a_shadow_with_no_blur_has_an_edge() {
    let (_, canvas) = draw(
        CARD,
        "body { margin: 20px; background: #ffffff }
         #card { width: 60px; height: 40px; background: #ffffff;
                 box-shadow: 10px 0 0 #000000 }",
    );
    // The card is at (20, 20), 60 wide; the shadow is the same rectangle ten
    // pixels to the right, so the strip past the card's right edge is solid
    // and the pixel past the shadow's is not.
    assert!(darkness(&canvas, 85, 40) > 0.95, "a hard shadow is solid");
    assert!(darkness(&canvas, 91, 40) < 0.05, "and stops where it stops");
}

#[test]
fn an_offset_puts_the_shadow_where_it_says_and_not_on_the_other_side() {
    let (_, canvas) = draw(
        CARD,
        "body { margin: 20px; background: #ffffff }
         #card { width: 60px; height: 40px; background: #ffffff;
                 box-shadow: 0 6px 4px #000000 }",
    );
    let below = darkness(&canvas, 50, 63);
    let above = darkness(&canvas, 50, 17);
    assert!(below > 0.3, "a shadow six pixels down is below the card");
    assert!(above < 0.05, "and not above it: {above}");
}

#[test]
fn a_shadow_is_behind_the_box_that_casts_it() {
    let (_, canvas) = draw(
        CARD,
        "body { margin: 20px; background: #ffffff }
         #card { width: 60px; height: 40px; background: #ffffff;
                 box-shadow: 0 0 12px #000000 }",
    );
    assert!(
        darkness(&canvas, 50, 40) < 0.01,
        "the card's own white covers its shadow",
    );
}

#[test]
fn an_inset_shadow_is_inside_the_box_and_nowhere_else() {
    let (_, canvas) = draw(
        CARD,
        "body { margin: 20px; background: #ffffff }
         #card { width: 60px; height: 40px; background: #ffffff;
                 box-shadow: inset 0 0 8px #000000 }",
    );
    let just_inside = darkness(&canvas, 22, 40);
    let middle = darkness(&canvas, 50, 40);
    let just_outside = darkness(&canvas, 17, 40);
    assert!(
        just_inside > middle,
        "an inset shadow hugs the inside edge: {just_inside} against {middle}",
    );
    assert!(just_inside > 0.2, "and is visible: {just_inside}");
    assert!(just_outside < 0.05, "and stays inside the box");
}

#[test]
fn a_gradient_background_runs_from_one_colour_to_the_other() {
    let (list, canvas) = draw(
        CARD,
        "body { margin: 20px; background: #ffffff }
         #card { width: 60px; height: 40px;
                 background: linear-gradient(#000000, #ffffff) }",
    );
    assert!(
        list.to_outline().contains("linear-gradient"),
        "the list says what it is filled with:\n{}",
        list.to_outline(),
    );
    let top = darkness(&canvas, 50, 21);
    let middle = darkness(&canvas, 50, 40);
    let bottom = darkness(&canvas, 50, 58);
    assert!(top > 0.9, "the top is the first colour: {top}");
    assert!(bottom < 0.1, "the bottom is the last: {bottom}");
    assert!(
        middle < top && middle > bottom,
        "and it runs between them: {middle}",
    );
    // Across the gradient nothing changes, because it runs down the page.
    assert!((darkness(&canvas, 25, 40) - darkness(&canvas, 70, 40)).abs() < 0.02);
}

#[test]
fn a_gradient_takes_the_direction_it_was_given() {
    let (_, canvas) = draw(
        CARD,
        "body { margin: 20px; background: #ffffff }
         #card { width: 60px; height: 40px;
                 background: linear-gradient(to right, #000000, #ffffff) }",
    );
    assert!(darkness(&canvas, 22, 40) > 0.9, "black on the left");
    assert!(darkness(&canvas, 78, 40) < 0.1, "white on the right");
    assert!((darkness(&canvas, 50, 24) - darkness(&canvas, 50, 56)).abs() < 0.02);
}

#[test]
fn a_radial_gradient_is_a_ring_around_the_middle() {
    let (_, canvas) = draw(
        CARD,
        "body { margin: 20px; background: #ffffff }
         #card { width: 60px; height: 40px;
                 background: radial-gradient(#000000, #ffffff) }",
    );
    let middle = darkness(&canvas, 50, 40);
    let out = darkness(&canvas, 50, 24);
    let corner = darkness(&canvas, 22, 22);
    assert!(middle > 0.95, "the middle is the first colour: {middle}");
    assert!(middle > out && out > corner, "and it fades outwards");
    // The same distance in every direction is the same colour, which is what
    // makes it a circle in a square box and an oval in a wide one. The card's
    // middle is at y 40, so these two pixel centres are both 15.5 out.
    assert!((darkness(&canvas, 50, 24) - darkness(&canvas, 50, 55)).abs() < 0.02);
}

#[test]
fn a_gradient_is_painted_over_the_colour_beneath_it() {
    let (list, _) = draw(
        CARD,
        "#card { width: 60px; height: 40px; background-color: #ff0000;
                 background-image: linear-gradient(#000000, #ffffff) }",
    );
    let outline = list.to_outline();
    let colour = outline.find("rgb(255 0 0)").expect("the colour is drawn");
    let gradient = outline
        .find("linear-gradient")
        .expect("and so is the gradient");
    assert!(
        colour < gradient,
        "the colour first, the image over it:\n{outline}"
    );
}

#[test]
fn a_shadow_this_engine_cannot_read_draws_nothing_rather_than_guessing() {
    let (list, canvas) = draw(
        CARD,
        "body { margin: 20px; background: #ffffff }
         #card { width: 60px; height: 40px; background: #ffffff;
                 box-shadow: 3px }",
    );
    assert!(
        !list.to_outline().contains("shadow"),
        "{}",
        list.to_outline()
    );
    assert!(
        darkness(&canvas, 50, 14) < 0.01,
        "and the page is untouched"
    );
}

#[test]
fn text_casts_a_shadow_behind_the_letters() {
    let (list, canvas) = draw(
        "<!DOCTYPE html><html><body><p id=t>Hello</p></body></html>",
        "body { margin: 20px; background: #ffffff }
         #t { font: 32px system-ui; color: #ffffff;
              text-shadow: 0 0 6px #000000 }",
    );
    assert!(
        list.to_outline().contains("+ shadow"),
        "the list says the text casts one:\n{}",
        list.to_outline(),
    );
    // The letters are white on white; everything visible is the shadow.
    let mut darkest = 0.0f32;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            darkest = darkest.max(darkness(&canvas, x, y));
        }
    }
    assert!(
        darkest > 0.2,
        "white text on white paper is legible by its shadow alone: {darkest}",
    );
}
