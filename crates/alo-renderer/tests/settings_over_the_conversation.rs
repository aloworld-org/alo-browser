/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 886 (`docs/design/shell.md`, *Settings*; approved by the
//! owner 2026-11-08): *Settings* stands over the chat on screen, which is
//! drawn under it as it was — a live card's page among it, its last frame
//! drawn in the card rather than beneath the first screen — blurred and
//! dimmed by the veil around the panel (`backdrop-filter`, queue item 885).
//!
//! Held by what was painted, in device pixels — never by a clock:
//!
//! - the card's page is a picture in the list, before the veil's backdrop,
//!   which reads the whole screen, so the page is blurred with the rest:
//!   where its stripes are sharp with *Settings* closed, they are grey
//!   under the veil; a reference of it, `settings-over-a-card.png`;
//! - Tab moving the keyboard's ring paints again only around the rings,
//!   never the whole frame, and blurs nothing again (ADR 0104): the
//!   veil's blur is taken as the last drawing kept it, and the card's
//!   frame, the same bytes, as the last drawing decoded it;
//! - *Settings* opened over the screen, and a new frame of the page under
//!   the veil — anything under it — paint the veil again, every pixel
//!   exactly as a renderer that never drew the screen paints it, at one and
//!   at two.

use alo_paint::{Canvas, DisplayItem, Point, Repainted};
use alo_renderer::card::Card;
use alo_renderer::conversation_claim::Claim;
use alo_renderer::start::{SettingsKeyed, Start};
use alo_renderer::turn::Turn;
use alo_renderer::{Frame, FromRenderer, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_value::Rgba;
use std::path::Path;

/// The corpus's faces, the generics mapped to them, and the design's
/// typefaces, as `the_cards_page_beneath.rs` gives them: loaded once, since
/// every screen here is drawn twice.
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

/// A frame of a page: stripes of black and white `wide` device pixels,
/// 1696 by 1100 — the card's page box at two, as Chrome streams it.
fn stripes(wide: u32) -> Vec<u8> {
    let mut canvas = Canvas::new(1696, 1100, Rgba::WHITE);
    for y in 0..1100 {
        for x in 0..1696 {
            if (x / wide).is_multiple_of(2) {
                canvas.replace(x, y, |_| Rgba::BLACK);
            }
        }
    }
    alo_paint::to_png(&canvas).unwrap_or_default()
}

/// The design's first screen at `scale`, a live card holding `frame` in
/// it, and *Settings* over it when `open`, the keyboard on `keyed`.
fn screen(scale: f32, frame: Vec<u8>, open: bool, keyed: Option<SettingsKeyed>) -> Start {
    let url =
        alo_url::parse("http://127.0.0.1:8739/").unwrap_or_else(|_| alo_url::Url::about_blank());
    Start {
        greeting: "Hello.".to_owned(),
        viewport: alo_layout::Size::new(1040.0, 820.0),
        scale,
        turns: vec![Turn::Page(
            Card::new("127.0.0.1:8739", url).loaded().holding(frame),
        )],
        settings: open,
        settings_keyed: keyed,
        ..Start::default()
    }
}

/// `renderer` told `start` and asked to paint: the frame, and what of it
/// was painted for it.
fn painted(renderer: &mut Renderer, start: Start) -> Result<(Frame, Repainted), String> {
    match renderer.handle(ToRenderer::Start(Box::new(start))) {
        FromRenderer::Loaded { issues, .. } if issues.is_empty() => {}
        other => return Err(format!("the first screen was not held cleanly: {other:?}")),
    }
    let FromRenderer::Painted(frame) = renderer.handle(ToRenderer::Paint) else {
        return Err("nothing was painted".to_owned());
    };
    let repainted = renderer
        .rendered()
        .map(|drawing| drawing.repainted)
        .ok_or("no drawing was kept")?;
    Ok((frame, repainted))
}

/// `start` as a renderer that never drew anything paints it: whole.
fn drawn_whole(start: Start) -> Result<Frame, String> {
    let (frame, repainted) = painted(&mut Renderer::new(fonts()), start)?;
    if repainted != Repainted::Whole {
        return Err(format!("a first drawing was {repainted:?}"));
    }
    Ok(frame)
}

/// The first pixel two frames of one size differ at, if any.
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

/// The red of the pixel at `x`, `y`.
fn red(frame: &Frame, x: u32, y: u32) -> u8 {
    frame.at(x, y).map_or(0, |pixel| pixel.0)
}

#[test]
fn over_a_card_its_page_is_drawn_and_blurred_with_the_rest() -> Result<(), String> {
    let (closed, _) = painted(
        &mut Renderer::new(fonts()),
        screen(1.0, stripes(24), false, None),
    )?;
    let mut renderer = Renderer::new(fonts());
    let (open, _) = painted(&mut renderer, screen(1.0, stripes(24), true, None))?;
    let items = renderer
        .rendered()
        .map(|drawing| drawing.display.items().to_vec())
        .unwrap_or_default();
    let picture = items
        .iter()
        .position(
            |item| matches!(item, DisplayItem::Picture { rect, .. } if rect.size.width > 600.0),
        )
        .ok_or("the card's page is a picture in the list")?;
    let veil = items
        .iter()
        .position(|item| {
            matches!(item, DisplayItem::Backdrop { rect, .. }
            if rect.size.width >= 1040.0 && rect.size.height >= 820.0)
        })
        .ok_or("the veil blurs the whole screen")?;
    if veil < picture {
        return Err(format!(
            "the veil at {veil} is before the page at {picture}"
        ));
    }
    // Beside the panel, over the card's page from 81 — two stripes and
    // more, 24 in from its edge: a stripe's edge is sharp with *Settings*
    // closed, black beside white; under the veil, about 30 levels apart.
    let row = 400;
    let (dark, light) = (105..155)
        .map(|x| red(&closed, x, row))
        .fold((u8::MAX, 0), |(dark, light), level| {
            (dark.min(level), light.max(level))
        });
    let (blurred_dark, blurred_light) = (105..155)
        .map(|x| red(&open, x, row))
        .fold((u8::MAX, 0), |(dark, light), level| {
            (dark.min(level), light.max(level))
        });
    if !(dark < 40 && light > 215) {
        return Err(format!("closed, the page's stripes are {dark} to {light}"));
    }
    if blurred_light - blurred_dark > 40 {
        return Err(format!(
            "under the veil, the stripes are still {blurred_dark} to {blurred_light}"
        ));
    }
    let reference =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/references/settings-over-a-card.png");
    let drawn = alo_renderer::frame::frame_to_png(&open).map_err(|why| why.to_string())?;
    if std::env::var_os("ALO_UPDATE_REFERENCES").is_some() {
        return std::fs::write(&reference, &drawn).map_err(|why| why.to_string());
    }
    let committed = std::fs::read(&reference)
        .map_err(|why| format!("{why}; ALO_UPDATE_REFERENCES=1 writes it"))?;
    let committed = alo_paint::from_png(&committed).map_err(|why| why.to_string())?;
    match first_difference(&open, &Frame::from_canvas(&committed)) {
        None => Ok(()),
        Some(at) => Err(format!(
            "settings-over-a-card.png: pixels moved, the first at {at:?}"
        )),
    }
}

#[test]
fn a_tab_paints_again_only_around_the_ring_and_blurs_nothing_again() -> Result<(), String> {
    let frame = stripes(24);
    let mut renderer = Renderer::new(fonts());
    painted(&mut renderer, screen(2.0, frame.clone(), true, None))?;
    for keyed in [
        Some(SettingsKeyed::Close),
        Some(SettingsKeyed::Font),
        Some(SettingsKeyed::Sites),
        None,
    ] {
        let start = screen(2.0, frame.clone(), true, keyed);
        let (drawn, repainted) = painted(&mut renderer, start.clone())?;
        // The ring put away and the ring drawn, and the rectangle holding
        // both: never the whole frame, as the veil's crop would be.
        let Repainted::Within(region) = repainted else {
            return Err(format!("{keyed:?} painted {repainted:?}"));
        };
        let whole = u64::from(drawn.width) * u64::from(drawn.height);
        if region.area() * 2 >= whole {
            return Err(format!("{keyed:?} painted {region:?} of {whole}"));
        }
        let blurred = renderer
            .rendered()
            .map_or(u32::MAX, |drawing| drawing.kept.made().backdrops);
        if blurred != 0 {
            return Err(format!("{keyed:?} blurred the veil again"));
        }
        if let Some(at) = first_difference(&drawn, &drawn_whole(start)?) {
            return Err(format!("{keyed:?}: not as drawn whole at {at:?}"));
        }
    }
    Ok(())
}

#[test]
fn a_change_under_the_veil_paints_it_again_exactly_as_drawn_whole() -> Result<(), String> {
    for scale in [1.0, 2.0] {
        let mut renderer = Renderer::new(fonts());
        // Opened over the card, painted over the screen without it.
        painted(&mut renderer, screen(scale, stripes(24), false, None))?;
        let start = screen(scale, stripes(24), true, None);
        let (drawn, _) = painted(&mut renderer, start.clone())?;
        if let Some(at) = first_difference(&drawn, &drawn_whole(start)?) {
            return Err(format!("at {scale}, opened: not as drawn whole at {at:?}"));
        }
        for (wide, keyed) in [
            (10, None),
            (10, Some(SettingsKeyed::Font)),
            (40, Some(SettingsKeyed::Font)),
        ] {
            let start = screen(scale, stripes(wide), true, keyed);
            let (drawn, _) = painted(&mut renderer, start.clone())?;
            if let Some(at) = first_difference(&drawn, &drawn_whole(start)?) {
                return Err(format!(
                    "at {scale}, stripes of {wide}: not as drawn whole at {at:?}"
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn a_press_on_the_veil_closes_settings_and_never_presses_what_is_under_it() -> Result<(), String> {
    // The box at the conversation's foot, 740 to 796, under the veil below
    // the panel, which ends at 788; and the card's page beside the panel.
    let claimed = |open: bool, (x, y): (f32, f32)| -> Result<Option<Claim>, String> {
        let mut renderer = Renderer::new(fonts());
        painted(&mut renderer, screen(1.0, stripes(24), open, None))?;
        match renderer.handle(ToRenderer::ConversationPress(Point::new(x, y))) {
            FromRenderer::ConversationClaimed(claim) => Ok(claim),
            other => Err(format!("{other:?}")),
        }
    };
    let the_box = (520.0, 792.0);
    if claimed(false, the_box)? != Some(Claim::TheBox) {
        return Err("closed, the box is pressed there".to_owned());
    }
    for at in [the_box, (120.0, 400.0), (960.0, 300.0)] {
        let claim = claimed(true, at)?;
        if claim != Some(Claim::LeaveSettings) {
            return Err(format!("open, {at:?} claimed {claim:?}"));
        }
    }
    Ok(())
}
