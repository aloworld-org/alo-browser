//! Reference renders: the same document, drawn twice, compared.
//!
//! `CLAUDE.md` asks for **a reference render for anything visual** — a small
//! deterministic raster compared against a committed reference, so that a
//! change which moves a pixel says so. This is that, and it is the first one
//! the repository has, because it is the first item that can draw.
//!
//! # How a failure reads
//!
//! Two ways, deliberately. The **display list** is compared first, in words —
//! "fill box#7 rgb(238 238 238) at (8, 32) 184×20" — so that a difference says
//! *what* changed. Then the pixels, which catch everything the list cannot
//! describe: anti-aliasing, glyph shapes, compositing.
//!
//! # Regenerating
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-paint` rewrites the committed
//! pictures. Read the diff before committing it: that is the whole point of
//! them being committed.

use alo_box::build as build_boxes;
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_paint::{Canvas, DisplayList, PaintContext, display, from_png, render, to_png};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{Font, FontDatabase, Slant, TextMeasurer, Weight};
use alo_value::Rgba;
use std::path::PathBuf;

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for (family, weight, slant, data) in [
        (
            "DejaVu Sans",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::sans::regular(),
        ),
        (
            "DejaVu Sans",
            Weight::BOLD,
            Slant::Normal,
            dejavu::sans::bold(),
        ),
    ] {
        if let Some(font) = Font::load(family, weight, slant, data.to_vec()) {
            database.add(font);
        }
    }
    database.map_generic("sans-serif", "DejaVu Sans");
    database.map_generic("system-ui", "DejaVu Sans");
    database
}

/// Render a document, and give back what was drawn and the picture of it.
fn draw(html: &str, css: &str, size: Size) -> (DisplayList, Canvas) {
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
    let layout = compute(&boxes, &styles, size, &measurer);

    let list = display::build(&boxes, &layout, &styles, PaintContext { fonts: &database });
    let mut canvas = Canvas::new(pixels(size.width), pixels(size.height), Rgba::WHITE);
    render(&list, &mut canvas);
    (list, canvas)
}

/// A size in whole pixels.
fn pixels(value: f32) -> u32 {
    let clamped = value.round().clamp(0.0, 8192.0);
    let mut whole = 0u32;
    // A loop rather than a cast: the sizes here are small and known, and this
    // needs no reasoning about what a float-to-integer cast does at the edges.
    while f32::from(u16::try_from(whole).unwrap_or(u16::MAX)) + 1.0 <= clamped {
        whole += 1;
    }
    whole
}

fn reference_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(format!("{name}.png"))
}

/// Compare a canvas against the committed picture of the same name.
///
/// Returns what is wrong rather than raising it, so that the test which asked
/// reports it — this helper does not know which picture the caller meant.
fn compare(name: &str, canvas: &Canvas) -> Result<(), String> {
    let path = reference_path(name);
    let drawn = to_png(canvas).map_err(|error| error.to_string())?;

    if std::env::var_os("ALO_UPDATE_REFERENCES").is_some() {
        return std::fs::write(&path, &drawn).map_err(|error| error.to_string());
    }

    let Ok(committed) = std::fs::read(&path) else {
        return Err(format!(
            "no reference at {}. Run with ALO_UPDATE_REFERENCES=1 to make one, \
             and read the picture before committing it.",
            path.display(),
        ));
    };
    let expected = from_png(&committed).map_err(|error| error.to_string())?;

    if (expected.width(), expected.height()) != (canvas.width(), canvas.height()) {
        return Err(format!(
            "the picture changed size: was {}×{}, now {}×{}",
            expected.width(),
            expected.height(),
            canvas.width(),
            canvas.height(),
        ));
    }
    let mut differing = Vec::new();
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            let (Some(was), Some(now)) = (expected.at(x, y), canvas.at(x, y)) else {
                continue;
            };
            if was.to_rgba8() != now.to_rgba8() {
                differing.push((x, y, was.to_rgba8(), now.to_rgba8()));
            }
        }
    }
    if differing.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{} pixels differ from {}. The first few: {:?}\n\
         If the change is intended, ALO_UPDATE_REFERENCES=1 rewrites it — \
         and read the diff before committing.",
        differing.len(),
        path.display(),
        differing.iter().take(5).collect::<Vec<_>>(),
    ))
}

const PAGE: &str = "<!DOCTYPE html><html><body><main id=main>\
<h1 id=title>Invoices</h1>\
<ul id=rows>\
<li class=row>Invoice 11</li>\
<li class=row id=selected>Invoice 12</li>\
<li class=row>Invoice 13</li>\
</ul>\
</main></body></html>";

const SHEET: &str = "
:root { --ink: #101014; --surface: #ffffff; --line: #d4d4d8; --chosen: #e4e4e7 }
body { margin: 0; background: var(--surface); color: var(--ink);
       font-family: system-ui; font-size: 14px }
main { padding: 8px }
h1 { font-size: 20px; margin: 0 }
ul { margin: 8px 0 0 0; padding: 0 }
li { padding: 4px; border-bottom-width: 1px; border-bottom-style: solid;
     border-bottom-color: var(--line) }
#selected { background: var(--chosen) }
";

#[test]
fn a_list_of_invoices_draws_the_same_things_it_did_before() {
    let (list, _) = draw(PAGE, SHEET, Size::new(200.0, 120.0));
    let expected = "\
fill box#1 rgb(255 255 255) at (0, 0) 200×123
text box#4 \"Invoices\" rgb(16 16 20) 20px at (8, 26.564453)
fill box#6 rgb(212 212 216) at (8, 64) 184×1
text box#7 \"Invoice 11\" rgb(16 16 20) 14px at (12, 55.995117)
fill box#8 rgb(228 228 231) at (8, 64) 184×25
fill box#8 rgb(212 212 216) at (8, 88) 184×1
text box#9 \"Invoice 12\" rgb(16 16 20) 14px at (12, 80.99512)
fill box#10 rgb(212 212 216) at (8, 114) 184×1
text box#11 \"Invoice 13\" rgb(16 16 20) 14px at (12, 106.99512)
";
    assert_eq!(list.to_outline(), expected);
}

#[test]
fn a_list_of_invoices_draws_the_same_pixels_it_did_before() {
    let (_, canvas) = draw(PAGE, SHEET, Size::new(200.0, 120.0));
    compare("invoices", &canvas).expect("the committed picture of the invoice list");
}

#[test]
fn text_is_actually_drawn_rather_than_left_as_empty_boxes() {
    let (_, canvas) = draw(PAGE, SHEET, Size::new(200.0, 120.0));
    let inked = (0..canvas.height())
        .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
        .filter(|(x, y)| {
            canvas
                .at(*x, *y)
                .is_some_and(|pixel| pixel.red < 0.5 && pixel.green < 0.5)
        })
        .count();
    assert!(
        inked > 200,
        "the page should have a few hundred dark pixels of text, found {inked}",
    );
}

#[test]
fn a_page_of_nothing_is_a_page_of_its_background() {
    let (list, canvas) = draw(
        "<html><body></body></html>",
        "body { margin: 0; background: #ffffff }",
        Size::new(8.0, 4.0),
    );
    assert_eq!(list.len(), 1, "just the body's background");
    for y in 0..4 {
        for x in 0..8 {
            assert_eq!(
                canvas.at(x, y).map(Rgba::to_rgba8),
                Some((255, 255, 255, 255))
            );
        }
    }
}

#[test]
fn the_same_document_drawn_twice_is_the_same_picture() {
    // Determinism is the whole reason a reference render is worth committing.
    let first = to_png(&draw(PAGE, SHEET, Size::new(200.0, 120.0)).1).expect("a picture");
    let second = to_png(&draw(PAGE, SHEET, Size::new(200.0, 120.0)).1).expect("a picture");
    assert_eq!(first, second);
}
