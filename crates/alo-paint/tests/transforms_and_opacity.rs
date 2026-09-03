//! What a transform and an opacity do to the pixels, asserted rather than
//! pictured.
//!
//! The committed pictures live in `alo-corpus`. What is here is the half a
//! picture cannot say: that a rotated box is *the same box somewhere else*,
//! that a group is faded **once** rather than box by box, and that a transform
//! changes what is drawn without changing what is laid out — which is what CSS
//! says and what an agent reading the tree depends on.

use alo_box::build as build_boxes;
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_paint::{Canvas, DisplayList, PaintContext, build, render};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{Font, FontDatabase, Slant, TextMeasurer, Weight};
use alo_value::Rgba;

const WIDTH: u32 = 120;
const HEIGHT: u32 = 120;

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
    let layout = compute(&boxes, &styles, Size::new(120.0, 120.0), &measurer);
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

/// How much of the page is not white.
fn inked(canvas: &Canvas) -> f32 {
    let mut total = 0.0;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            total += darkness(canvas, x, y);
        }
    }
    total
}

const BOX: &str = "<!DOCTYPE html><html><body><div id=a></div></body></html>";

/// A twenty-pixel black square at (20, 20), before anything moves it.
const SQUARE: &str = "body { margin: 20px; background: #ffffff }
                      #a { width: 20px; height: 20px; background: #000000 }";

#[test]
fn a_box_with_no_transform_is_left_exactly_where_it_was() {
    let (list, canvas) = draw(BOX, SQUARE);
    assert!(
        !list.to_outline().contains("transform"),
        "{}",
        list.to_outline()
    );
    assert!(darkness(&canvas, 25, 25) > 0.99);
    assert!(darkness(&canvas, 55, 25) < 0.01);
}

#[test]
fn a_translation_moves_the_box_and_nothing_else() {
    let (list, canvas) = draw(
        BOX,
        &format!("{SQUARE} #a {{ transform: translate(30px, 10px) }}"),
    );
    assert!(
        list.to_outline().contains("transform"),
        "{}",
        list.to_outline()
    );
    assert!(darkness(&canvas, 25, 25) < 0.01, "it left where it was");
    assert!(
        darkness(&canvas, 55, 35) > 0.99,
        "and arrived where it went"
    );
}

#[test]
fn a_scale_grows_the_box_about_its_middle() {
    let (_, canvas) = draw(BOX, &format!("{SQUARE} #a {{ transform: scale(2) }}"));
    // The square was 20..40 in both directions; doubled about (30, 30) it is
    // 10..50, so its old corner is inside it and the new one is on its edge.
    assert!(darkness(&canvas, 12, 12) > 0.99, "it grew outwards");
    assert!(darkness(&canvas, 30, 30) > 0.99, "and its middle held");
    assert!(
        darkness(&canvas, 55, 55) < 0.01,
        "and stopped where it stops"
    );
}

#[test]
fn a_rotation_turns_the_box_about_its_middle_and_keeps_its_area() {
    let (_, straight) = draw(BOX, SQUARE);
    let (_, turned) = draw(BOX, &format!("{SQUARE} #a {{ transform: rotate(45deg) }}"));

    // A square turned a quarter of a right angle has its corners where its
    // edges were: the middle of each old edge is now inside nothing.
    assert!(darkness(&turned, 30, 30) > 0.99, "the middle holds");
    assert!(
        darkness(&turned, 21, 21) < 0.5,
        "the old corner is outside it"
    );
    assert!(
        (inked(&turned) - inked(&straight)).abs() / inked(&straight) < 0.06,
        "a rotation moves ink about rather than making it: {} against {}",
        inked(&turned),
        inked(&straight),
    );
}

#[test]
fn a_transform_origin_says_what_the_box_turns_about() {
    let (_, canvas) = draw(
        BOX,
        &format!("{SQUARE} #a {{ transform: rotate(90deg); transform-origin: left top }}"),
    );
    // Turned about its own top-left corner at (20, 20), the square swings from
    // 20..40 across into 20..40 down and 0..20 across — off the left of where
    // it was, and the page's own left edge cuts part of it off.
    assert!(darkness(&canvas, 10, 30) > 0.99, "it swung left");
    assert!(darkness(&canvas, 30, 30) < 0.01, "and is not where it was");
}

#[test]
fn a_transform_carries_what_is_inside_the_box_with_it() {
    let (_, canvas) = draw(
        "<!DOCTYPE html><html><body><div id=a><div id=b></div></div></body></html>",
        "body { margin: 20px; background: #ffffff }
         #a { width: 40px; height: 40px; background: #dddddd;
              transform: translate(40px, 0) }
         #b { width: 10px; height: 10px; background: #000000 }",
    );
    assert!(
        darkness(&canvas, 65, 25) > 0.99,
        "the child moved with its parent",
    );
    assert!(darkness(&canvas, 25, 25) < 0.05, "and not without it");
}

#[test]
fn a_transform_moves_what_is_drawn_and_not_what_is_laid_out() {
    let plain = draw(BOX, SQUARE).0.to_outline();
    let moved = draw(
        BOX,
        &format!("{SQUARE} #a {{ transform: translate(30px, 0) }}"),
    )
    .0
    .to_outline();
    // The fill is at the same place in both lists; only the transform around
    // it differs. That is what lets an agent read a position from the layout.
    assert!(plain.contains("fill box#2"), "{plain}");
    assert!(
        moved.contains(
            plain
                .lines()
                .find(|line| line.contains("fill box#2"))
                .expect("a fill")
        ),
        "the box is drawn from the same place:\n{moved}",
    );
}

#[test]
fn a_transform_this_engine_cannot_read_moves_nothing() {
    let (list, canvas) = draw(
        BOX,
        &format!("{SQUARE} #a {{ transform: translateX(30px) rotate3d(1, 1, 1, 45deg) }}"),
    );
    assert!(
        !list.to_outline().contains("transform"),
        "{}",
        list.to_outline()
    );
    assert!(
        darkness(&canvas, 25, 25) > 0.99,
        "half a transform is worse than none",
    );
}

#[test]
fn opacity_fades_a_box() {
    let (list, canvas) = draw(BOX, &format!("{SQUARE} #a {{ opacity: 0.5 }}"));
    assert!(list.to_outline().contains("group"), "{}", list.to_outline());
    let faded = darkness(&canvas, 25, 25);
    assert!(
        (faded - 0.5).abs() < 0.02,
        "half a black square on white paper is a mid grey: {faded}",
    );
}

#[test]
fn a_group_is_faded_once_rather_than_box_by_box() {
    // Two black squares exactly on top of one another, at half opacity. Faded
    // as a group they are one mid grey; faded separately the second would show
    // through the first and come out three quarters dark.
    let (_, canvas) = draw(
        "<!DOCTYPE html><html><body><div id=a><div id=b></div></div></body></html>",
        "body { margin: 20px; background: #ffffff }
         #a { width: 20px; height: 20px; background: #000000; opacity: 0.5 }
         #b { width: 20px; height: 20px; background: #000000 }",
    );
    let grey = darkness(&canvas, 25, 25);
    assert!(
        (grey - 0.5).abs() < 0.02,
        "a group is composited once: {grey}",
    );
}

#[test]
fn opacity_of_one_needs_no_group_at_all() {
    let (list, _) = draw(BOX, &format!("{SQUARE} #a {{ opacity: 1 }}"));
    assert!(
        !list.to_outline().contains("group"),
        "an opaque box is not worth a surface:\n{}",
        list.to_outline(),
    );
}

#[test]
fn opacity_of_nothing_draws_nothing() {
    let (_, canvas) = draw(BOX, &format!("{SQUARE} #a {{ opacity: 0 }}"));
    assert!(darkness(&canvas, 25, 25) < 0.01);
}

#[test]
fn a_percentage_is_an_opacity_too() {
    let (_, canvas) = draw(BOX, &format!("{SQUARE} #a {{ opacity: 50% }}"));
    assert!((darkness(&canvas, 25, 25) - 0.5).abs() < 0.02);
}

#[test]
fn a_gradient_under_a_transform_turns_with_the_box() {
    let (_, canvas) = draw(
        BOX,
        "body { margin: 20px; background: #ffffff }
         #a { width: 40px; height: 40px; transform: rotate(90deg);
              background: linear-gradient(to right, #000000, #ffffff) }",
    );
    // Turned a quarter clockwise, "to right" points down the page.
    assert!(
        darkness(&canvas, 40, 22) > 0.9,
        "the dark end is at the top"
    );
    assert!(
        darkness(&canvas, 40, 58) < 0.1,
        "and the light end below it"
    );
}

#[test]
fn a_negative_z_index_puts_a_box_behind_its_parents_content() {
    let (_, canvas) = draw(
        "<!DOCTYPE html><html><body><div id=a><div id=b></div><div id=c></div>\
         </div></body></html>",
        "body { margin: 20px; background: #ffffff }
         #a { position: relative; z-index: 0; width: 40px; height: 40px }
         #b { position: absolute; z-index: -1; width: 40px; height: 40px;
              background: #000000 }
         #c { width: 40px; height: 20px; background: #ffffff }",
    );
    // A negative `z-index` goes behind its parent's *content* but in front of
    // its background — the one part of stacking that surprises people.
    assert!(
        darkness(&canvas, 30, 25) < 0.01,
        "the in-flow child is painted over it",
    );
    assert!(
        darkness(&canvas, 30, 50) > 0.99,
        "and where nothing covers it, it shows",
    );
}

#[test]
fn a_positioned_box_inside_a_transform_is_transformed_with_it() {
    let (_, canvas) = draw(
        "<!DOCTYPE html><html><body><div id=a><div id=b></div></div></body></html>",
        "body { margin: 20px; background: #ffffff }
         #a { width: 20px; height: 20px; transform: translate(40px, 0) }
         #b { position: absolute; width: 20px; height: 20px; background: #000000 }",
    );
    // Without the escape being caught by the transform's own stacking context
    // this would have been painted over the whole page instead.
    assert!(
        darkness(&canvas, 65, 25) > 0.99,
        "it moved with the transform"
    );
    assert!(darkness(&canvas, 25, 25) < 0.01);
}
