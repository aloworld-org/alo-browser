/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 991: the frame's band, drawn by the strip's renderer as alo
//! OS's window band — the page's title at 22 across and 18 down, 19 tall, in
//! Manrope semibold 14; the application's name 24 after it and 20 down, 15
//! tall, in Manrope 11 — the title giving way first, with an ellipsis, the
//! name keeping its room, and the name said once when the title is it.
//!
//! Each box in numbers, at one and at two, as the layout holds them in CSS
//! pixels; what the composed frame looks like is `alo-window`'s
//! `the_window_composed.rs`'s references.

use alo_layout::{Rect, Size};
use alo_paint::DisplayItem;
use alo_renderer::band::{APPLICATION, Band};
use alo_renderer::typefaces::MANROPE;
use alo_renderer::{Frame, FromRenderer, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

/// The interface's typefaces, as the frame's strip's renderer is handed
/// them, and the corpus's sans-serif for the generics after them.
fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for font in alo_renderer::typefaces::fonts() {
        database.add(font);
    }
    if let Some(font) = Font::load(
        "DejaVu Sans",
        Weight::NORMAL,
        Slant::Normal,
        dejavu::sans::regular().to_vec(),
    ) {
        database.add(font);
    }
    database.map_generic("system-ui", "DejaVu Sans");
    database.map_generic("sans-serif", "DejaVu Sans");
    database
}

/// The band's height, in CSS pixels: alo OS's window band.
const HEIGHT: f32 = 54.0;

/// What a renderer holding a band paints, and the renderer.
fn drawn(title: &str, width: f32, scale: f32) -> (Renderer, Option<Frame>) {
    let mut renderer = Renderer::new(fonts());
    let band = Band::new(title, "https://example.com/", Size::new(width, HEIGHT)).at_scale(scale);
    let answer = renderer.handle(ToRenderer::Band(Box::new(band)));
    assert!(
        matches!(&answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{answer:?}"
    );
    let frame = match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Some(frame),
        _ => None,
    };
    (renderer, frame)
}

/// The border box of the element of `class`, in CSS pixels.
fn box_of(renderer: &Renderer, class: &str) -> Option<Rect> {
    let (drawing, document) = (renderer.rendered()?, renderer.document()?);
    drawing.boxes.ids().find_map(|id| {
        let made = drawing.boxes.get(id)?;
        if matches!(made.kind, alo_box::BoxKind::Text { .. }) {
            return None;
        }
        let node = made.kind.node()?;
        (document.attribute_by_name(node, "class") == Some(class))
            .then(|| drawing.layout.get(id).map(|at| at.border_box))
            .flatten()
    })
}

/// Every text drawn, with its font's family and size.
fn texts(renderer: &Renderer) -> Vec<(String, String, String)> {
    renderer
        .rendered()
        .map(|drawing| {
            drawing
                .display
                .items()
                .iter()
                .filter_map(|item| match item {
                    DisplayItem::Text {
                        text, font, size, ..
                    } => Some((text.clone(), font.family().to_owned(), format!("{size}"))),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A rectangle as the outline writes it: layout holds whole 1/64 px, so two
/// rectangles that write alike are the same, compared without comparing
/// floats.
fn units(rect: Option<Rect>) -> String {
    rect.map_or_else(
        || "none".to_owned(),
        |rect| {
            format!(
                "({}, {}) {}×{}",
                rect.left(),
                rect.top(),
                rect.size.width,
                rect.size.height
            )
        },
    )
}

#[test]
fn the_title_at_22_18_and_the_name_24_after_it_at_one_and_two() {
    for scale in [1.0, 2.0] {
        let (renderer, frame) = drawn("Invoices", 600.0, scale);
        let title = box_of(&renderer, "title").unwrap_or_else(|| panic!("no title at {scale}"));
        let name = box_of(&renderer, "name").unwrap_or_else(|| panic!("no name at {scale}"));
        // 22 across and 18 down, on a 19 line.
        let origin = format!(
            "({}, {}) h {}",
            title.left(),
            title.top(),
            title.size.height
        );
        assert_eq!(origin, "(22, 18) h 19", "the title at {scale}");
        // 24 after the title's end, 20 down, on a 15 line.
        let after = format!(
            "({}, {}) h {}",
            name.left() - title.right(),
            name.top(),
            name.size.height
        );
        assert_eq!(after, "(24, 20) h 15", "the name at {scale}");
        // Neither is cut: there is room for both.
        assert!(name.right() < 600.0, "{}", units(Some(name)));
        // Set in Manrope, the title at 14 and the name at 11.
        let said = texts(&renderer);
        assert_eq!(
            said,
            vec![
                ("Invoices".to_owned(), MANROPE.to_owned(), "14".to_owned()),
                (APPLICATION.to_owned(), MANROPE.to_owned(), "11".to_owned()),
            ],
            "at {scale}"
        );
        // Painted at the scale, the band's whole size in device pixels.
        let frame = frame.unwrap_or_else(|| panic!("nothing painted at {scale}"));
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "1 or 2"
        )]
        let factor = scale as u32;
        assert_eq!((frame.width, frame.height), (600 * factor, 54 * factor));
        // The band's ground is white.
        assert_eq!(frame.pixels.get(..4), Some(&[0xff, 0xff, 0xff, 0xff][..]));
    }
}

#[test]
fn a_long_title_gives_way_first_and_the_name_keeps_its_room() {
    let long = "Quarterly invoices for every client this organisation has ever had, \
                sorted by the date each was last paid";
    for scale in [1.0, 2.0] {
        let (short, _) = drawn("Invoices", 400.0, scale);
        let (renderer, _) = drawn(long, 400.0, scale);
        let title = box_of(&renderer, "title").unwrap_or_else(|| panic!("no title at {scale}"));
        let name = box_of(&renderer, "name").unwrap_or_else(|| panic!("no name at {scale}"));
        let kept = box_of(&short, "name").unwrap_or_else(|| panic!("no name at {scale}"));
        // The name as wide as it is beside a short title, at the band's
        // right edge, 24 after the cut title.
        assert_eq!(
            format!("{}", name.size.width),
            format!("{}", kept.size.width),
            "at {scale}"
        );
        assert_eq!(format!("{}", name.right()), "400", "at {scale}");
        assert_eq!(format!("{}", name.left() - title.right()), "24");
        assert_eq!(
            format!("{}", title.left()),
            "22",
            "the title still starts at 22"
        );
        // And the title is cut, an ellipsis drawn after what is kept, in
        // the title's face; the name whole after both.
        let said: Vec<(String, String)> = texts(&renderer)
            .into_iter()
            .map(|(text, _, size)| (text, size))
            .collect();
        let [(kept, at_14), (ellipsis, also_14), (named, at_11)] = said.as_slice() else {
            panic!("the cut title, its ellipsis and the name: {said:?}");
        };
        assert!(long.starts_with(kept.as_str()) && kept.len() < long.len());
        assert_eq!(
            (ellipsis.as_str(), named.as_str()),
            ("\u{2026}", APPLICATION)
        );
        assert_eq!(
            (at_14.as_str(), also_14.as_str(), at_11.as_str()),
            ("14", "14", "11")
        );
    }
}

#[test]
fn a_title_that_is_the_name_is_said_once() {
    let (renderer, _) = drawn(APPLICATION, 600.0, 1.0);
    assert_eq!(units(box_of(&renderer, "name")), "none");
    let title = box_of(&renderer, "title");
    assert_eq!(units(title).split(' ').next(), Some("(22,"));
    let said: Vec<String> = texts(&renderer)
        .into_iter()
        .map(|(text, ..)| text)
        .collect();
    assert_eq!(said, vec![APPLICATION.to_owned()]);
}

#[test]
fn a_title_holding_markup_is_drawn_as_the_characters_it_is() {
    let (renderer, _) = drawn("Invoices <b>2026</b>", 600.0, 1.0);
    let said: Vec<String> = texts(&renderer)
        .into_iter()
        .map(|(text, ..)| text)
        .collect();
    assert_eq!(
        said,
        vec!["Invoices <b>2026</b>".to_owned(), APPLICATION.to_owned()]
    );
}

#[test]
fn a_band_is_never_pressed() {
    let (mut renderer, _) = drawn("Invoices", 600.0, 1.0);
    let answer = renderer.handle(ToRenderer::StripPress(alo_paint::Point::new(30.0, 20.0)));
    assert!(
        matches!(
            answer,
            FromRenderer::Failed(alo_renderer::message::Failure::NotTheStrip)
        ),
        "{answer:?}"
    );
}
