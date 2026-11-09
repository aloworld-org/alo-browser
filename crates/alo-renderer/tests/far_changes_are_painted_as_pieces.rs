/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 891: two far changes are painted again as pieces, each on
//! its own, rather than as the one rectangle holding both. Tab from *Close*
//! to *Font* in *Settings* over a card puts a ring away at the panel's top
//! right and draws one half way down its left; the rectangle holding both
//! held the card's picture, the veil's blur and a score of runs of text.
//!
//! Held by what was painted, in device pixels — never by a clock: at one
//! and at two, that Tab paints two pieces whose pixels are under half of
//! the rectangle holding them, blurring nothing again; every Tab through
//! *Settings* comes out every byte of a frame a renderer that never drew
//! paints.

use alo_paint::{Canvas, Repainted};
use alo_renderer::card::Card;
use alo_renderer::pipeline::Drawing;
use alo_renderer::start::{SettingsKeyed, Start};
use alo_renderer::turn::Turn;
use alo_renderer::{Frame, FromRenderer, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_value::Rgba;
use std::rc::Rc;

/// The corpus's faces, the generics mapped to them, and the design's
/// typefaces, as `settings_over_the_conversation.rs` gives them: loaded
/// once, since every screen here is drawn twice.
fn fonts() -> FontDatabase {
    static LOADED: std::sync::OnceLock<FontDatabase> = std::sync::OnceLock::new();
    LOADED.get_or_init(load_fonts).clone()
}

fn load_fonts() -> FontDatabase {
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

/// A frame of a page the size the card's page box is at `scale` — 878 by
/// 520 CSS pixels — as Chrome streams it: stripes of black and white 24
/// device pixels wide, so that it is drawn at its own size.
fn stripes(scale: f32) -> Vec<u8> {
    let (width, height) = if scale > 1.5 {
        (1756, 1040)
    } else {
        (878, 520)
    };
    let mut canvas = Canvas::new(width, height, Rgba::WHITE);
    for y in 0..height {
        for x in 0..width {
            if (x / 24).is_multiple_of(2) {
                canvas.replace(x, y, |_| Rgba::BLACK);
            }
        }
    }
    alo_paint::to_png(&canvas).unwrap_or_default()
}

/// The design's first screen at `scale`, a live card holding its frame,
/// and *Settings* over it when `open`, the keyboard on `keyed`.
fn screen(scale: f32, open: bool, keyed: Option<SettingsKeyed>) -> Start {
    let url =
        alo_url::parse("http://127.0.0.1:8739/").unwrap_or_else(|_| alo_url::Url::about_blank());
    Start {
        greeting: "Hello.".to_owned(),
        viewport: alo_layout::Size::new(1040.0, 820.0),
        scale,
        turns: vec![Turn::Page(
            Card::new("127.0.0.1:8739", url)
                .loaded()
                .holding(stripes(scale)),
        )],
        settings: open,
        settings_keyed: keyed,
        ..Start::default()
    }
}

/// `renderer` told `start` and asked to paint: the frame, and the drawing.
fn painted(renderer: &mut Renderer, start: Start) -> Result<(Frame, Rc<Drawing>), String> {
    match renderer.handle(ToRenderer::Start(Box::new(start))) {
        FromRenderer::Loaded { issues, .. } if issues.is_empty() => {}
        other => return Err(format!("the first screen was not held cleanly: {other:?}")),
    }
    let FromRenderer::Painted(frame) = renderer.handle(ToRenderer::Paint) else {
        return Err("nothing was painted".to_owned());
    };
    let drawing = renderer.rendered().ok_or("no drawing was kept")?;
    Ok((frame, drawing))
}

/// `start` as a renderer that never drew anything paints it: whole.
fn drawn_whole(start: Start) -> Result<Frame, String> {
    let (frame, drawing) = painted(&mut Renderer::new(fonts()), start)?;
    if drawing.repainted != Repainted::Whole {
        return Err(format!("a first drawing was {:?}", drawing.repainted));
    }
    Ok(frame)
}

/// The first pixel two frames differ at, if any.
fn first_difference(seen: &Frame, wanted: &Frame) -> Option<(u32, u32)> {
    if seen == wanted {
        return None;
    }
    if (seen.width, seen.height) != (wanted.width, wanted.height) {
        return Some((seen.width, seen.height));
    }
    (0..wanted.height)
        .flat_map(|y| (0..wanted.width).map(move |x| (x, y)))
        .find(|&(x, y)| seen.at(x, y) != wanted.at(x, y))
}

/// At `scale`: the far Tab in two pieces, and every Tab as drawn whole.
fn in_pieces(scale: f32) -> Result<(), String> {
    {
        let mut renderer = Renderer::new(fonts());
        painted(&mut renderer, screen(scale, true, None))?;
        painted(
            &mut renderer,
            screen(scale, true, Some(SettingsKeyed::Close)),
        )?;
        let start = screen(scale, true, Some(SettingsKeyed::Font));
        let (frame, drawing) = painted(&mut renderer, start.clone())?;
        let Repainted::Within(bounds) = drawing.repainted else {
            return Err(format!("at {scale}, painted {:?}", drawing.repainted));
        };
        let made = drawing.kept.made();
        if made.pieces != 2 || made.painted * 2 > bounds.area() || made.backdrops != 0 {
            return Err(format!("at {scale}: {made:?} within {bounds:?}"));
        }
        if let Some(at) = first_difference(&frame, &drawn_whole(start)?) {
            return Err(format!("at {scale}: not as drawn whole at {at:?}"));
        }
        for keyed in [
            Some(SettingsKeyed::Sites),
            Some(SettingsKeyed::Actions),
            Some(SettingsKeyed::Asks),
            Some(SettingsKeyed::Close),
            None,
        ] {
            let start = screen(scale, true, keyed);
            let (frame, drawing) = painted(&mut renderer, start.clone())?;
            if drawing.kept.made().backdrops != 0 {
                return Err(format!("at {scale}, {keyed:?} blurred the veil again"));
            }
            if let Some(at) = first_difference(&frame, &drawn_whole(start)?) {
                return Err(format!(
                    "at {scale}, {keyed:?}: not as drawn whole at {at:?}"
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn at_one_two_far_rings_are_two_pieces_each_as_drawn_whole() -> Result<(), String> {
    in_pieces(1.0)
}

#[test]
fn at_two_two_far_rings_are_two_pieces_each_as_drawn_whole() -> Result<(), String> {
    in_pieces(2.0)
}
