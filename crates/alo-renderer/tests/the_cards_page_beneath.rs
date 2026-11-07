/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 869 (ADR 0100): the live card's page drawn **beneath** the
//! first screen rather than in it, so that a frame of the page never paints
//! the first screen again — and drawn so that a person cannot tell.
//!
//! The first screen with its card's picture beneath leaves the picture's
//! place clear ([`alo_paint::beneath`]) and says where
//! ([`ToRenderer::PictureBeneath`]); a renderer of its own draws Chrome's
//! frame at that place's size ([`ToRenderer::CardPicture`]); the window
//! puts the picture under the first screen's frame. Asserted here, each a
//! test below:
//!
//! - the place, in numbers: the card's page box, 81, 145, 878 × 520 at one
//!   and 162, 290, 1756 × 1040 at two, its every pixel clear and the card's
//!   bar and border around it not;
//! - **composed, it is the card drawn with its picture in it**: at one,
//!   against the committed reference `card-holding-a-frame.png` (the
//!   picture in the card, `a_page_as_a_card.rs`); at two, against the same
//!   card drawn with its picture in it then — within one level a channel,
//!   which is a rounded corner's soft edge written as bytes once more;
//! - a frame with translucent pixels — the Chromium alo carries draws a
//!   page with no background of its own clear — shows the card's surface
//!   through them, as it did in the card;
//! - an agent reads the same tree either way: the page as an image named
//!   [`PICTURED`].

use alo_paint::{Canvas, from_png};
use alo_renderer::beneath::Beneath;
use alo_renderer::card::Card;
use alo_renderer::card_document::PICTURED;
use alo_renderer::card_picture::CardPicture;
use alo_renderer::start::Start;
use alo_renderer::turn::Turn;
use alo_renderer::{Frame, FromRenderer, Renderer, Snapshot, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_value::Rgba;
use std::path::{Path, PathBuf};

/// The corpus's faces, as `a_page_as_a_card.rs` gives them, and the
/// design's typefaces.
fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for (family, weight, bytes) in [
        ("DejaVu Sans", Weight::NORMAL, dejavu::sans::regular()),
        ("DejaVu Sans", Weight::BOLD, dejavu::sans::bold()),
        ("DejaVu Serif", Weight::NORMAL, dejavu::serif::regular()),
    ] {
        if let Some(font) = Font::load(family, weight, Slant::Normal, bytes.to_vec()) {
            database.add(font);
        }
    }
    for (generic, family) in [
        ("system-ui", "DejaVu Sans"),
        ("sans-serif", "DejaVu Sans"),
        ("serif", "DejaVu Serif"),
    ] {
        database.map_generic(generic, family);
    }
    for font in alo_renderer::typefaces::fonts() {
        database.add(font);
    }
    database
}

fn reference(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name)
}

/// The frame Chrome streamed of `a_page_as_a_card.rs`'s page, at one.
fn chromes_frame() -> Result<Vec<u8>, String> {
    let path = reference("card-frame.png");
    std::fs::read(&path).map_err(|why| format!("{} is not there: {why}", path.display()))
}

/// The first screen of the design's window at `scale` with a card for
/// `card.html` holding `frame`, its picture in the card or beneath it.
fn with_a_card(frame: Vec<u8>, scale: f32, beneath: bool) -> Start {
    let url = alo_url::parse("http://127.0.0.1:8765/card.html")
        .unwrap_or_else(|_| alo_url::Url::about_blank());
    let card = Card::new("127.0.0.1:8765/card.html", url).loaded();
    let card = if beneath {
        card.pictured_beneath()
    } else {
        card.holding(frame)
    };
    Start {
        greeting: "Hello.".to_owned(),
        viewport: alo_layout::Size::new(1040.0, 820.0),
        scale,
        turns: vec![Turn::Page(card)],
        ..Start::default()
    }
}

/// What a renderer holding `start` painted, where it left a place clear,
/// and what an agent reads.
fn drawn(start: Start) -> Result<(Frame, Option<Beneath>, Snapshot), String> {
    let mut renderer = Renderer::new(fonts());
    match renderer.handle(ToRenderer::Start(Box::new(start))) {
        FromRenderer::Loaded { issues, .. } if issues.is_empty() => {}
        other => {
            return Err(format!(
                "the first screen was not loaded cleanly: {other:?}"
            ));
        }
    }
    let FromRenderer::Painted(frame) = renderer.handle(ToRenderer::Paint) else {
        return Err("nothing was painted".to_owned());
    };
    let FromRenderer::PictureBeneath(place) = renderer.handle(ToRenderer::PictureBeneath) else {
        return Err("no place was said".to_owned());
    };
    let FromRenderer::Tree(tree) = renderer.handle(ToRenderer::ReadTree) else {
        return Err("no tree was read".to_owned());
    };
    Ok((frame, place, *tree))
}

/// The card's page drawn by a renderer of its own at `place`'s size.
fn picture(png: Vec<u8>, place: Beneath) -> Result<Frame, String> {
    let mut renderer = Renderer::new(FontDatabase::new());
    match renderer.handle(ToRenderer::CardPicture(Box::new(CardPicture {
        png,
        width: place.width,
        height: place.height,
    }))) {
        FromRenderer::Painted(frame) => Ok(frame),
        other => Err(format!("the card's page was not drawn: {other:?}")),
    }
}

/// `start` over `picture` at `place`, as the window composes them: the
/// picture over the colour the first screen kept where it cleared its
/// place, made opaque, and the first screen's frame over that.
fn composed(start: &Frame, picture: &Frame, place: Beneath) -> Frame {
    let mut canvas = Canvas::new(start.width, start.height, Rgba::TRANSPARENT);
    for y in 0..picture.height {
        for x in 0..picture.width {
            let (Ok(across), Ok(down)) = (
                u32::try_from(i64::from(place.left) + i64::from(x)),
                u32::try_from(i64::from(place.top) + i64::from(y)),
            ) else {
                continue;
            };
            let (Some((red, green, blue, alpha)), Some((under_red, under_green, under_blue, _))) =
                (picture.at(x, y), start.at(across, down))
            else {
                continue;
            };
            canvas.blend(
                across,
                down,
                Rgba::from_rgba8(under_red, under_green, under_blue, 255),
                255,
            );
            canvas.blend(across, down, Rgba::from_rgba8(red, green, blue, 255), alpha);
        }
    }
    for y in 0..start.height {
        for x in 0..start.width {
            if let Some((red, green, blue, alpha)) = start.at(x, y) {
                canvas.blend(x, y, Rgba::from_rgba8(red, green, blue, 255), alpha);
            }
        }
    }
    Frame::from_canvas(&canvas)
}

/// The most two frames differ by in any channel of any pixel, and how many
/// pixels differ at all; [`None`] for frames of different sizes.
fn apart(one: &Frame, other: &Frame) -> Option<(u8, usize)> {
    if (one.width, one.height) != (other.width, other.height) {
        return None;
    }
    let mut most = 0;
    let mut differing = 0;
    for (a, b) in one.pixels.chunks(4).zip(other.pixels.chunks(4)) {
        let far = a
            .iter()
            .zip(b)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        most = most.max(far);
        differing += usize::from(far > 0);
    }
    Some((most, differing))
}

/// The names of what an agent reads as an image.
fn images(tree: &Snapshot) -> Vec<Option<String>> {
    tree.nodes()
        .into_iter()
        .filter(|node| node.role.as_str() == "image")
        .map(|node| node.name.clone())
        .collect()
}

#[test]
fn the_page_box_is_left_clear_and_said_in_device_pixels_at_one_and_two() -> Result<(), String> {
    // The card's rounding, 14 at one, in device pixels.
    for (scale, curve, expected) in [
        (
            1.0,
            14,
            Beneath {
                left: 81,
                top: 145,
                width: 878,
                height: 520,
            },
        ),
        (
            2.0,
            28,
            Beneath {
                left: 162,
                top: 290,
                width: 1756,
                height: 1040,
            },
        ),
    ] {
        let (frame, place, _) = drawn(with_a_card(Vec::new(), scale, true))?;
        assert_eq!(place, Some(expected), "at {scale}");
        let alpha = |x: i32, y: i32| {
            frame
                .at(u32::try_from(x).ok()?, u32::try_from(y).ok()?)
                .map(|pixel| pixel.3)
        };
        let (right, bottom) = (
            expected.left + i32::try_from(expected.width).unwrap_or(0),
            expected.top + i32::try_from(expected.height).unwrap_or(0),
        );
        // Every pixel of it clear but its two rounded bottom corners.
        let rounded = |x: i32, y: i32| {
            y >= bottom - curve && (x < expected.left + curve || x >= right - curve)
        };
        for y in expected.top..bottom {
            for x in expected.left..right {
                if !rounded(x, y) {
                    assert_eq!(alpha(x, y), Some(0), "({x}, {y}) at {scale}");
                }
            }
        }
        // The bar above it and the border beside it drawn.
        assert_eq!(alpha(expected.left + 40, expected.top - 1), Some(255));
        assert_eq!(alpha(expected.left - 1, expected.top + 100), Some(255));
        assert_eq!(alpha(right, expected.top + 100), Some(255));
    }
    Ok(())
}

#[test]
fn composed_at_one_the_card_is_the_reference_drawn_with_its_picture_in_it() -> Result<(), String> {
    let png = chromes_frame()?;
    let (start, place, tree) = drawn(with_a_card(Vec::new(), 1.0, true))?;
    let place = place.ok_or("no place left clear")?;
    let page = picture(png, place)?;
    let seen = composed(&start, &page, place);
    let committed = std::fs::read(reference("card-holding-a-frame.png"))
        .map_err(|why| format!("no reference: {why}"))?;
    let expected = Frame::from_canvas(
        &from_png(&committed).map_err(|why| format!("the reference is not a picture: {why}"))?,
    );
    let (most, differing) = apart(&seen, &expected).ok_or("another size")?;
    assert!(
        most <= 1,
        "{differing} pixels differ, by {most} at most, from the reference"
    );
    // Only on the rounded corners' soft edges, a handful of pixels.
    assert!(differing <= 32, "{differing} pixels differ");
    assert_eq!(images(&tree), vec![Some(PICTURED.to_owned())]);
    Ok(())
}

#[test]
fn composed_at_two_the_card_is_the_card_drawn_with_its_picture_smoothed_in_it() -> Result<(), String>
{
    let png = chromes_frame()?;
    let (in_it, none, tree_in_it) = drawn(with_a_card(png.clone(), 2.0, false))?;
    assert_eq!(none, None, "a picture in the card leaves no place clear");
    let (start, place, tree) = drawn(with_a_card(Vec::new(), 2.0, true))?;
    let place = place.ok_or("no place left clear")?;
    // A frame at one, smoothed to the place at two, as the card's `<img>`
    // smoothed it.
    let page = picture(png, place)?;
    assert_eq!((page.width, page.height), (1756, 1040));
    let seen = composed(&start, &page, place);
    let (most, differing) = apart(&seen, &in_it).ok_or("another size")?;
    assert!(
        most <= 1,
        "{differing} pixels differ, by {most} at most, from the card drawn with its picture"
    );
    assert!(differing <= 32, "{differing} pixels differ");
    assert_eq!(images(&tree), images(&tree_in_it));
    Ok(())
}

#[test]
fn a_translucent_frame_shows_the_cards_surface_through_it_as_it_did_in_the_card()
-> Result<(), String> {
    // Chrome's frame with its left half clear and a band half seen.
    let mut frame = Frame::from_canvas(
        &alo_paint::picture::read(&chromes_frame()?)
            .map_err(|why| format!("not a picture: {why}"))?,
    );
    let width = usize::try_from(frame.width).unwrap_or(0);
    for (at, pixel) in frame.pixels.chunks_mut(4).enumerate() {
        let x = at % width;
        if let Some(alpha) = pixel.get_mut(3) {
            *alpha = match x {
                0..=399 => 0,
                400..=499 => 128,
                _ => *alpha,
            };
        }
    }
    let png = alo_renderer::frame::frame_to_png(&frame).map_err(|why| why.to_string())?;
    for scale in [1.0, 2.0] {
        let (in_it, _, _) = drawn(with_a_card(png.clone(), scale, false))?;
        let (start, place, _) = drawn(with_a_card(Vec::new(), scale, true))?;
        let place = place.ok_or("no place left clear")?;
        let seen = composed(&start, &picture(png.clone(), place)?, place);
        let (most, differing) = apart(&seen, &in_it).ok_or("another size")?;
        assert!(
            most <= 1,
            "at {scale}: {differing} pixels differ, by {most} at most, from the card"
        );
        // The clear half is the card's white surface, not what is behind
        // the first screen.
        let middle = |frame: &Frame| {
            frame.at(
                u32::try_from(place.left).unwrap_or(0) + place.width / 8,
                u32::try_from(place.top).unwrap_or(0) + place.height / 2,
            )
        };
        assert_eq!(middle(&seen), Some((255, 255, 255, 255)), "at {scale}");
        assert_eq!(middle(&seen), middle(&in_it));
    }
    Ok(())
}
