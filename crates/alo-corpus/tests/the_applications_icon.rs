/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The macOS application's icon, drawn by this engine at every size an
//! `.icns` holds (queue item 689; ADR 0067 § 6).
//!
//! `cases/the-applications-icon` is one page whose every length is in
//! `vmin`, laid out in a window as wide as it is tall at each size, 16 to
//! 1,024 pixels a side. Each drawing is painted onto a **transparent**
//! picture rather than the white a page is rendered on, because nothing is
//! behind the tile and an icon's corners are the Finder's to show through.
//!
//! The drawing is the owner's, `docs/brand/alo-browser-icon.svg`, written
//! into the page as it is (queue item 913): a cream tile, a navy globe and
//! a teal swoosh filled by a `<linearGradient>` (queue item 275).
//!
//! Under 64 pixels a side the owner's thin arcs fade, so there the page
//! draws `docs/brand/alo-browser-icon-small.svg` instead — the same paths
//! and colours, each also stroked in its own paint (queue item 914), chosen
//! by a `max-width` media query on the window, which is the picture.
//!
//! The pictures, `icon-16.png` to `icon-1024.png` beside the case, are the
//! references compared here — and the bytes `alo-bundle` packs into the
//! application's `.icns`, so the icon a person sees is the one this test
//! holds.

use alo_corpus::{Case, Rendering, cases_directory, check_picture, corpus_fonts};
use alo_layout::{Rect, Size};
use alo_paint::Canvas;
use alo_renderer::{Drawing, Screen, render_at};
use alo_value::Rgba;
use std::rc::Rc;

/// Each side an `.icns` holds a picture of, in pixels.
const SIDES: [u32; 7] = [16, 32, 64, 128, 256, 512, 1024];

/// The smallest side the owner's drawing is drawn at; under it the small
/// drawing is.
const OWNERS_FROM: u32 = 64;

/// The class of the drawing shown at `side`.
fn shown_at(side: u32) -> &'static str {
    if side < OWNERS_FROM { "small" } else { "icon" }
}

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("the-applications-icon"))
}

/// The page laid out and drawn in a window `side` pixels a side.
fn drawn(case: &Case, side: u32) -> Rc<Drawing> {
    drawn_from(case, &case.html, side)
}

/// `html`, with the case's sheets and resources, laid out and drawn in a
/// window `side` pixels a side.
fn drawn_from(case: &Case, html: &str, side: u32) -> Rc<Drawing> {
    let side = f32::from(u16::try_from(side).unwrap_or(u16::MAX));
    render_at(
        html,
        &case.css,
        Screen::new(Size::new(side, side)),
        &corpus_fonts(),
        &case.linked,
        &case.resources,
    )
    .drawing
}

/// What `drawing` paints, onto nothing, as its PNG holds it.
///
/// Through the PNG and back because that is the icon a person sees, and
/// because it settles the one thing a picture drawn onto nothing has that a
/// file does not: the colour of a pixel no paint reached enough to show. The
/// shadow's farthest reach leaves its navy at an alpha that rounds to none;
/// a PNG read back holds such a pixel as nothing at all, which is what it is.
fn icon(drawing: &Drawing, side: u32) -> Option<Canvas> {
    let mut canvas = Canvas::new(side, side, Rgba::TRANSPARENT);
    alo_paint::render(&drawing.display, &mut canvas);
    alo_paint::from_png(&alo_paint::to_png(&canvas).ok()?).ok()
}

/// The border box of the element whose `class` is `class`.
fn box_of(case: &Case, drawing: &Drawing, class: &str) -> Option<Rect> {
    let document = alo_dom::parse_document(&case.html);
    let boxes = &drawing.boxes;
    let id = boxes.ids().find(|id| {
        boxes
            .get(*id)
            .and_then(|node| node.kind.node())
            .and_then(|node| document.attribute_by_name(node, "class"))
            == Some(class)
    })?;
    drawing.layout.border_box(id)
}

/// Two lengths the same to a thousandth of a pixel.
fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001
}

#[test]
fn the_tile_and_its_drawing_stand_on_apples_grid_at_every_size() {
    let case = case().expect("the case is in the corpus");
    for side in SIDES {
        let drawing = drawn(&case, side);
        let tile = box_of(&case, &drawing, "tile").expect("the tile has a box");
        let unit = f32::from(u16::try_from(side).unwrap_or(u16::MAX)) / 1024.0;
        let (inset, width) = (100.0 * unit, 824.0 * unit);
        assert!(
            near(tile.left(), inset)
                && near(tile.top(), inset)
                && near(tile.size.width, width)
                && near(tile.size.height, width),
            "at {side}: {tile:?}, not {inset} in and {width} wide"
        );
        // The drawing shown stands exactly on the box that casts its
        // shadow, so the shadow's corners are the drawing's; the other has
        // no box at all.
        let shown = shown_at(side);
        let hidden = if shown == "small" { "icon" } else { "small" };
        assert!(
            box_of(&case, &drawing, hidden).is_none(),
            "at {side}: the {hidden} drawing has a box too"
        );
        let drawn = box_of(&case, &drawing, shown).expect("the drawing has a box");
        assert!(
            near(drawn.left(), tile.left())
                && near(drawn.top(), tile.top())
                && near(drawn.size.width, tile.size.width)
                && near(drawn.size.height, tile.size.height),
            "at {side}: the drawing at {drawn:?} is not on its tile at {tile:?}"
        );
    }
}

#[test]
fn the_icon_is_drawn_onto_nothing() {
    let case = case().expect("the case is in the corpus");
    for side in SIDES {
        let picture = icon(&drawn(&case, side), side).expect("the icon is a PNG");
        let middle = side / 2;
        assert_eq!(
            picture.at(0, 0).map(Rgba::to_rgba8).map(|(_, _, _, a)| a),
            Some(0),
            "at {side}: the corner outside the tile is drawn"
        );
        assert_eq!(
            picture
                .at(middle, side / 8 + 1)
                .map(Rgba::to_rgba8)
                .map(|(_, _, _, a)| a),
            Some(255),
            "at {side}: the tile's top middle is not opaque"
        );
        assert_eq!(
            picture.at(side - 1, side - 1).map(Rgba::to_rgba8),
            Some((0, 0, 0, 0)),
            "at {side}: the far corner is drawn"
        );
    }
}

#[test]
fn every_size_of_the_icon_is_the_one_committed() {
    let case = case().expect("the case is in the corpus");
    let differences: Vec<String> = SIDES
        .into_iter()
        .filter_map(|side| {
            let Some(picture) = icon(&drawn(&case, side), side) else {
                return Some(format!("icon-{side}.png could not be made a PNG"));
            };
            check_picture(&case, &format!("icon-{side}.png"), &picture)
                .map(|difference| difference.to_string())
        })
        .collect();
    assert!(
        differences.is_empty(),
        "{differences:#?}\nALO_UPDATE_REFERENCES=1 rewrites them, and the diff is then the review"
    );
}

#[test]
fn the_icon_at_1024_is_the_case_as_the_corpus_renders_it() {
    // The corpus draws the page on white at its size, 1,024; the icon is the
    // same list on nothing. Where the tile is opaque the two agree, so the
    // icon is the page and not a second drawing of it.
    let case = case().expect("the case is in the corpus");
    let rendering = Rendering::of(&case).expect("the case renders");
    let page = rendering.drawing().expect("it drew");
    let picture = icon(&page, 1024).expect("the icon is a PNG");
    let mut compared = 0;
    for (on_white, on_nothing) in page.canvas.pixels().iter().zip(picture.pixels()) {
        let (r, g, b, a) = on_nothing.to_rgba8();
        if a == 255 {
            assert_eq!(on_white.to_rgba8(), (r, g, b, 255));
            compared += 1;
        }
    }
    assert!(
        compared > 500_000,
        "only {compared} opaque pixels to compare"
    );
}

#[test]
fn the_drawings_are_the_owners_and_its_small_variant_as_they_are() {
    // Every element of each file is in the page, attribute for attribute:
    // the page adds a class and a name to each <svg> and nothing else.
    let case = case().expect("the case is in the corpus");
    for (name, file) in [
        (
            "alo-browser-icon.svg",
            include_str!("../../../docs/brand/alo-browser-icon.svg"),
        ),
        (
            "alo-browser-icon-small.svg",
            include_str!("../../../docs/brand/alo-browser-icon-small.svg"),
        ),
    ] {
        let body = file
            .split_once('>')
            .map(|(_, rest)| rest.trim())
            .expect("the file opens with its <svg>");
        assert!(
            case.html.contains(&body.replace('\n', "\n    ")),
            "the page's drawing is not {name}"
        );
    }
}

/// How many of a picture's opaque pixels are darker than half, and the
/// darkest of them, by the mean of their channels.
fn darkness(picture: &Canvas) -> (usize, u8) {
    let mut dark = 0;
    let mut darkest = u8::MAX;
    for pixel in picture.pixels() {
        let (r, g, b, a) = pixel.to_rgba8();
        if a == 255 {
            let mean = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;
            let mean = u8::try_from(mean).unwrap_or(u8::MAX);
            darkest = darkest.min(mean);
            if mean < 128 {
                dark += 1;
            }
        }
    }
    (dark, darkest)
}

#[test]
fn the_small_pictures_are_the_heavier_drawing_in_numbers() {
    // The same page with its media query never matching draws the owner's
    // drawing at every size: what the 16- and 32-pixel pictures were
    // before item 914. The small drawing covers several times as many dark
    // pixels, and at 16 its darkest is nearer the navy's 61.
    let case = case().expect("the case is in the corpus");
    let owners_only = case.html.replace("(max-width: 63px)", "(max-width: 0px)");
    assert_ne!(owners_only, case.html, "the page has its media query");
    let mut measured = Vec::new();
    for side in [16, 32] {
        let small = icon(&drawn(&case, side), side).expect("the icon is a PNG");
        let owners = icon(&drawn_from(&case, &owners_only, side), side).expect("the icon is a PNG");
        measured.push((side, darkness(&small), darkness(&owners)));
    }
    // Measured: at 16, 27 dark pixels to the owner's 4, darkest 71 to 107;
    // at 32, 147 to 49.
    for (side, (dark, darkest), (owners_dark, owners_darkest)) in &measured {
        assert!(
            *dark >= owners_dark * 2 + 10 && darkest <= owners_darkest,
            "at {side}: {measured:?}"
        );
    }
    assert!(
        measured
            .first()
            .is_some_and(|(_, (_, darkest), _)| *darkest <= 80),
        "{measured:?}"
    );
    // From 64 up the page is the owner's drawing alone.
    for side in [64, 128] {
        let page = icon(&drawn(&case, side), side).expect("the icon is a PNG");
        let owners = icon(&drawn_from(&case, &owners_only, side), side).expect("the icon is a PNG");
        let colours = |picture: &Canvas| -> Vec<(u8, u8, u8, u8)> {
            picture
                .pixels()
                .iter()
                .map(|pixel| pixel.to_rgba8())
                .collect()
        };
        assert!(colours(&page) == colours(&owners), "at {side}");
    }
}

#[test]
fn the_swoosh_is_painted_by_its_gradient_at_every_size() {
    // Filled at every size; under 64 also stroked in it, which is the small
    // drawing's second ramp.
    let case = case().expect("the case is in the corpus");
    for side in SIDES {
        let drawing = drawn(&case, side);
        let ramps = drawing
            .display
            .items()
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    alo_paint::DisplayItem::Fill {
                        paint: alo_paint::Paint::Ramp(_),
                        ..
                    }
                )
            })
            .count();
        let expected = if side < OWNERS_FROM { 2 } else { 1 };
        assert_eq!(ramps, expected, "at {side}");
    }
}
