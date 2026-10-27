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

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("the-applications-icon"))
}

/// The page laid out and drawn in a window `side` pixels a side.
fn drawn(case: &Case, side: u32) -> Rc<Drawing> {
    let side = f32::from(u16::try_from(side).unwrap_or(u16::MAX));
    render_at(
        &case.html,
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
fn the_tile_stands_on_apples_grid_at_every_size() {
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
        let name = box_of(&case, &drawing, "name").expect("the name has a box");
        let (centre, middle) = (
            name.left() + name.size.width / 2.0,
            name.top() + name.size.height / 2.0,
        );
        assert!(
            (centre - 512.0 * unit).abs() < 0.01 && (middle - 512.0 * unit).abs() < 0.01,
            "at {side}: the name at {name:?} is not centred on the tile"
        );
        assert!(
            name.size.width < width,
            "at {side}: the name is {} wide, wider than its tile",
            name.size.width
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
