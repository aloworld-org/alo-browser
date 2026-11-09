/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 892: *Settings* opened again finds what its last opening
//! made. Its panel's two shadows took 42 of opening's 54 ms at two to blur;
//! closed, they are laid aside (`alo_paint::Kept`, within its bound), and
//! the next opening takes them, its fills and its clips as they were made.
//!
//! Queue item 894: and its veil's blur. The screen under the veil is the
//! very list it was blurred from, so the blur laid aside with it is taken
//! — whether the card's frame was drawn in the first screen while *Settings*
//! was closed, or beneath it as the window asks then (its decoded picture
//! laid aside too, `alo_renderer::pictures::Decoded`); a new frame of the
//! page under it is blurred anew.
//!
//! Held by what was made, never by a clock: at one and at two, opened over
//! a card, closed, typed beside, and opened again — the second opening
//! blurs no shadow and no backdrop, makes no fill or mask, and takes what
//! it does from what was laid aside; and every frame is every byte of one a
//! renderer that never drew paints. The card's frame is drawn at its own
//! size, which `alo-paint`'s own tests hold to the mix it was drawn by
//! before.

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
    striped(scale, 24)
}

/// The same, stripes `wide` device pixels wide.
fn striped(scale: f32, wide: u32) -> Vec<u8> {
    let (width, height) = if scale > 1.5 {
        (1756, 1040)
    } else {
        (878, 520)
    };
    let mut canvas = Canvas::new(width, height, Rgba::WHITE);
    for y in 0..height {
        for x in 0..width {
            if (x / wide).is_multiple_of(2) {
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

/// `start` with its card's page drawn beneath it, as the window asks for
/// the first screen while *Settings* is closed (ADR 0100).
fn beneath(mut start: Start) -> Start {
    let url =
        alo_url::parse("http://127.0.0.1:8739/").unwrap_or_else(|_| alo_url::Url::about_blank());
    start.turns = vec![Turn::Page(
        Card::new("127.0.0.1:8739", url)
            .loaded()
            .holding(stripes(start.scale))
            .pictured_beneath(),
    )];
    start
}

/// At `scale`: opened, closed and typed beside — the card's frame drawn in
/// the first screen or, `under`, beneath it — and opened again.
fn opened_again(scale: f32, under: bool) -> Result<(), String> {
    {
        let mut renderer = Renderer::new(fonts());
        painted(&mut renderer, screen(scale, false, None))?;
        let opened = screen(scale, true, None);
        let (frame, drawing) = painted(&mut renderer, opened.clone())?;
        if drawing.kept.made().blurred < 2 {
            return Err(format!("at {scale}, opened: {:?}", drawing.kept.made()));
        }
        if let Some(at) = first_difference(&frame, &drawn_whole(opened.clone())?) {
            return Err(format!("at {scale}, opened: not as drawn whole at {at:?}"));
        }
        let mut typed = screen(scale, false, None);
        if under {
            typed = beneath(typed);
        }
        for (index, letters) in ["", "h", "hi"].into_iter().enumerate() {
            letters.clone_into(&mut typed.typed);
            typed.focused = index > 0;
            let (frame, drawing) = painted(&mut renderer, typed.clone())?;
            if drawing.kept.aside_bytes() == 0 {
                return Err(format!("at {scale}, closed: nothing laid aside"));
            }
            if let Some(at) = first_difference(&frame, &drawn_whole(typed.clone())?) {
                return Err(format!("at {scale}, closed: not as drawn whole at {at:?}"));
            }
        }
        let (frame, drawing) = painted(&mut renderer, opened.clone())?;
        let made = drawing.kept.made();
        if made.blurred != 0
            || made.filled != 0
            || made.masks != 0
            || made.aside == 0
            || made.backdrops != 0
            || made.blurs_alike != 1
        {
            return Err(format!("at {scale}, opened again: {made:?}"));
        }
        if let Some(at) = first_difference(&frame, &drawn_whole(opened.clone())?) {
            return Err(format!(
                "at {scale}, opened again: not as drawn whole at {at:?}"
            ));
        }
        // Closed, and a new frame of the page: opened over it, the veil's
        // blur is made anew.
        painted(&mut renderer, typed)?;
        let mut moved = opened;
        moved.turns = vec![Turn::Page(
            Card::new(
                "127.0.0.1:8739",
                alo_url::parse("http://127.0.0.1:8739/")
                    .unwrap_or_else(|_| alo_url::Url::about_blank()),
            )
            .loaded()
            .holding(striped(scale, 32)),
        )];
        let (frame, drawing) = painted(&mut renderer, moved.clone())?;
        let made = drawing.kept.made();
        if (made.backdrops, made.blurs_alike) != (1, 0) {
            return Err(format!("at {scale}, opened over a new frame: {made:?}"));
        }
        if let Some(at) = first_difference(&frame, &drawn_whole(moved)?) {
            return Err(format!(
                "at {scale}, opened over a new frame: not as drawn whole at {at:?}"
            ));
        }
    }
    Ok(())
}

#[test]
fn at_one_settings_opened_again_takes_what_its_last_opening_made() -> Result<(), String> {
    opened_again(1.0, false)
}

#[test]
fn at_two_settings_opened_again_takes_what_its_last_opening_made() -> Result<(), String> {
    opened_again(2.0, false)
}

#[test]
fn at_one_settings_opened_again_over_a_card_drawn_beneath_while_closed_takes_its_blur()
-> Result<(), String> {
    opened_again(1.0, true)
}

#[test]
fn at_two_settings_opened_again_over_a_card_drawn_beneath_while_closed_takes_its_blur()
-> Result<(), String> {
    opened_again(2.0, true)
}
