/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 890: a drawing whose boxes did not change is not laid out
//! again. Every drawing of the first screen is a new document; a Tab in
//! *Settings* changes only a ring's class, which is a `box-shadow`, so the
//! drawing before's layout is kept (`alo_renderer::kept_layout`).
//!
//! Held by the tree laid out, in numbers — never by a clock: at one and at
//! two, *Settings* opened over a card, Tab through every stop, a face
//! chosen, a window a pixel narrower, closed and opened again, letters
//! typed — every
//! drawing's layout, kept or made, is every number of the layout made
//! afresh from its boxes and styles; every Tab keeps it, and opening,
//! closing, a face and another window make it; and every frame drawn with
//! a layout kept is every byte of one a renderer that never drew paints.

use alo_paint::{Canvas, Repainted};
use alo_renderer::card::Card;
use alo_renderer::interface_face::InterfaceFace;
use alo_renderer::pipeline::Drawing;
use alo_renderer::start::{SettingsKeyed, Start};
use alo_renderer::turn::Turn;
use alo_renderer::{Frame, FromRenderer, Renderer, ToRenderer};
use alo_text::TextMeasurer;
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

/// `drawing`'s layout made afresh from its own boxes and styles, for its
/// viewport and in its faces, written out with every number.
fn laid_out_afresh(drawing: &Drawing) -> String {
    let made = alo_layout::compute(
        &drawing.boxes,
        &drawing.styles,
        drawing.layout.viewport(),
        &TextMeasurer::new(&drawing.fonts),
    );
    format!("{made:?}")
}

/// Every step at `scale`: the layout kept only where it is the layout
/// made, and every frame whose layout was kept as drawn whole.
fn kept_only_where_alike(scale: f32) -> Result<(), String> {
    {
        let mut renderer = Renderer::new(fonts());
        painted(&mut renderer, screen(scale, false, None))?;
        let mut steps: Vec<(&str, Start, bool)> =
            vec![("opened", screen(scale, true, None), false)];
        for keyed in SettingsKeyed::ALL {
            steps.push(("a Tab", screen(scale, true, Some(keyed)), true));
        }
        steps.push(("the keyboard put away", screen(scale, true, None), true));
        // Then back to the default (Manrope since queue item 975), so the ring
        // again is over the screen as it was.
        for face in [InterfaceFace::Geist, InterfaceFace::Manrope] {
            let mut start = screen(scale, true, Some(SettingsKeyed::Font));
            start.face = face;
            steps.push(("a face chosen", start, false));
        }
        steps.push((
            "the ring again",
            screen(scale, true, Some(SettingsKeyed::Close)),
            true,
        ));
        let mut narrower = screen(scale, true, Some(SettingsKeyed::Close));
        narrower.viewport = alo_layout::Size::new(1039.0, 820.0);
        steps.push(("a window a pixel narrower", narrower, false));
        steps.push(("closed", screen(scale, false, None), false));
        steps.push(("opened again", screen(scale, true, None), false));
        // A letter changes a text box and no element's style.
        for (letters, keeps) in [("h", false), ("hi", false), ("hi", true)] {
            let mut typed = screen(scale, false, None);
            letters.clone_into(&mut typed.typed);
            typed.focused = true;
            steps.push(("a letter typed", typed, keeps));
        }
        for (what, start, keeps) in steps {
            let (frame, drawing) = painted(&mut renderer, start.clone())?;
            if drawing.layout_kept != keeps {
                return Err(format!(
                    "at {scale}, {what}: the layout kept is {}",
                    drawing.layout_kept
                ));
            }
            if format!("{:?}", drawing.layout) != laid_out_afresh(&drawing) {
                return Err(format!("at {scale}, {what}: not the layout made afresh"));
            }
            if keeps && let Some(at) = first_difference(&frame, &drawn_whole(start)?) {
                return Err(format!("at {scale}, {what}: not as drawn whole at {at:?}"));
            }
        }
    }
    Ok(())
}

#[test]
fn at_one_a_layout_is_kept_only_where_it_is_the_layout_made() -> Result<(), String> {
    kept_only_where_alike(1.0)
}

#[test]
fn at_two_a_layout_is_kept_only_where_it_is_the_layout_made() -> Result<(), String> {
    kept_only_where_alike(2.0)
}
