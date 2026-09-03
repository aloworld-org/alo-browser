//! What clipping does to the pixels, asserted rather than pictured.
//!
//! The committed pictures live in `alo-corpus`, which is where a reference
//! render belongs — one place, one runner, one way to update them. What is
//! here is the half a picture cannot say: *why* the picture is right. A card's
//! corner is the page's background **because it was clipped away**, and an
//! assertion can say that where an image can only differ.

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

fn draw(html: &str, css: &str, width: f32, height: f32) -> (DisplayList, Canvas) {
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
    let layout = compute(&boxes, &styles, Size::new(width, height), &measurer);
    let list = build::build(
        &boxes,
        &layout,
        &styles,
        PaintContext {
            fonts: &database,
            pictures: &std::collections::BTreeMap::new(),
        },
    );

    let mut canvas = Canvas::new(120, 60, Rgba::WHITE);
    render(&list, &mut canvas);
    (list, canvas)
}

const CARD: &str = "<!DOCTYPE html><html><body><div id=card>\
<div id=banner></div></div></body></html>";

const ROUNDED: &str = "body { margin: 8px; background: #f4f4f5 }
#card { width: 100px; height: 40px; border-radius: 12px; overflow: hidden;
        background: #ffffff }
#banner { height: 40px; background: #000000 }";

#[test]
fn a_rounded_box_clips_its_content_to_its_own_corners() {
    let (list, canvas) = draw(CARD, ROUNDED, 120.0, 60.0);
    assert!(
        list.to_outline().contains("clip"),
        "the card asked to clip:\n{}",
        list.to_outline(),
    );

    let corner = canvas.at(9, 9).expect("just inside the card's corner");
    assert!(
        corner.red > 0.8,
        "the corner is clipped away, so it is still the page: {corner}",
    );
    let middle = canvas.at(60, 28).expect("the middle of the banner");
    assert!(middle.red < 0.2, "and the middle is the banner: {middle}");
}

#[test]
fn a_square_box_does_not_clip_its_corner_away() {
    let square = ROUNDED.replace("border-radius: 12px;", "border-radius: 0;");
    let (_, canvas) = draw(CARD, &square, 120.0, 60.0);
    let corner = canvas.at(9, 9).expect("just inside the card's corner");
    assert!(
        corner.red < 0.2,
        "with square corners the banner reaches the corner: {corner}",
    );
}

#[test]
fn a_box_that_does_not_clip_lets_its_content_out() {
    let unclipped = ROUNDED.replace("overflow: hidden;", "");
    let (list, _) = draw(CARD, &unclipped, 120.0, 60.0);
    assert!(
        !list.to_outline().contains("clip"),
        "`overflow: visible` is the initial value and does not clip",
    );
}

#[test]
fn the_edge_of_a_rounded_clip_is_smooth_rather_than_stepped() {
    let (_, canvas) = draw(CARD, ROUNDED, 120.0, 60.0);
    // Along the card's top-left curve there should be pixels that are neither
    // the page nor the banner — a clip multiplies coverage rather than
    // switching it on and off.
    let partly = (8..30)
        .flat_map(|x| (8..30).map(move |y| (x, y)))
        .filter(|(x, y)| {
            canvas
                .at(*x, *y)
                .is_some_and(|pixel| pixel.red > 0.1 && pixel.red < 0.9)
        })
        .count();
    assert!(
        partly > 5,
        "a curved clip has partly-covered pixels along it, found {partly}",
    );
}
