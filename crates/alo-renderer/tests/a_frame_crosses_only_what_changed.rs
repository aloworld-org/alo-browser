/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 872 (ADR 0102): asked to paint only what changed, the first
//! screen's renderer sends only the rectangle its drawings since the last
//! frame it sent painted again — and that rectangle, put over the frame the
//! asker holds, is the frame a renderer that never drew sends whole, every
//! byte.
//!
//! Held by the bytes that cross, never by a clock:
//!
//! - a letter beside a card crosses in at most a twentieth of the bytes of
//!   the whole frame, the first letter, which opens the offers, in at most a
//!   quarter, and nothing changed in a patch of no pixels;
//! - two drawings between two asks cross as one rectangle holding both, and
//!   an ask for the whole frame between them starts the changes again from
//!   it;
//! - a first frame, and a frame of another size, cross whole.

use alo_renderer::card::Card;
use alo_renderer::field::Editing;
use alo_renderer::start::Start;
use alo_renderer::turn::Turn;
use alo_renderer::typed::offers_for;
use alo_renderer::wire::{read_from_renderer, write_from_renderer};
use alo_renderer::{Frame, FromRenderer, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

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

/// The first screen of the design's window at `scale`, the box holding the
/// keyboard and `typed` with the caret at its end and what it offers, and
/// a live card whose page is drawn beneath it when `card`.
fn screen(scale: f32, card: bool, typed: &str) -> Start {
    let turns = if card {
        let url = alo_url::parse("http://127.0.0.1:8739/")
            .unwrap_or_else(|_| alo_url::Url::about_blank());
        vec![Turn::Page(
            Card::new("127.0.0.1:8739", url).loaded().pictured_beneath(),
        )]
    } else {
        Vec::new()
    };
    Start {
        greeting: "Hello.".to_owned(),
        viewport: alo_layout::Size::new(1040.0, 820.0),
        scale,
        typed: typed.to_owned(),
        focused: true,
        editing: Editing::caret_at(typed.chars().count()),
        offers: offers_for(typed, card),
        turns,
        ..Start::default()
    }
}

/// `start` told to `renderer`, and its paint asked by `paint`, across the
/// wire as it crosses between processes: the answer, and how many bytes it
/// took.
fn asked(
    renderer: &mut Renderer,
    start: Start,
    paint: ToRenderer,
) -> Result<(FromRenderer, usize), String> {
    match renderer.handle(ToRenderer::Start(Box::new(start))) {
        FromRenderer::Loaded { issues, .. } if issues.is_empty() => {}
        other => return Err(format!("the first screen was not held cleanly: {other:?}")),
    }
    let bytes = write_from_renderer(&renderer.handle(paint));
    let answer = read_from_renderer(&bytes).map_err(|why| why.why)?;
    Ok((answer, bytes.len()))
}

/// `start` as a renderer that never drew sends it: whole.
fn whole(start: Start) -> Result<Frame, String> {
    match asked(&mut Renderer::new(fonts()), start, ToRenderer::Paint)? {
        (FromRenderer::Painted(frame), _) => Ok(frame),
        (other, _) => Err(format!("a first frame came as {other:?}")),
    }
}

/// What `answer` makes of the frame `held`, as the asker holds it: whether
/// it was a patch.
fn held_after(held: &mut Frame, answer: FromRenderer) -> Result<bool, String> {
    match answer {
        FromRenderer::Painted(frame) => {
            *held = frame;
            Ok(false)
        }
        FromRenderer::PaintedOver(patch) => {
            if patch.onto(held) {
                Ok(true)
            } else {
                Err(format!(
                    "a patch not of the frame held: {}×{}",
                    patch.width, patch.height
                ))
            }
        }
        other => Err(format!("a paint answered {other:?}")),
    }
}

#[test]
fn a_letter_beside_a_card_crosses_as_the_rectangle_it_changed() -> Result<(), String> {
    let mut renderer = Renderer::new(fonts());
    let (first, _) = asked(&mut renderer, screen(2.0, true, ""), ToRenderer::PaintOver)?;
    let FromRenderer::Painted(mut held) = first else {
        return Err(format!("a first frame crossed as {first:?}"));
    };
    let frame_bytes = held.pixels.len();
    for (typed, most) in [("h", 4), ("he", 20), ("hel", 20), ("hel", 20), ("", 4)] {
        let (answer, bytes) = asked(
            &mut renderer,
            screen(2.0, true, typed),
            ToRenderer::PaintOver,
        )?;
        if !held_after(&mut held, answer)? {
            return Err(format!("typing {typed:?} crossed whole, {bytes} bytes"));
        }
        assert!(
            bytes * most <= frame_bytes,
            "typing {typed:?} crossed in {bytes} bytes of a {frame_bytes}-byte frame"
        );
        if held != whole(screen(2.0, true, typed))? {
            return Err(format!(
                "typing {typed:?}, the frame held is not the frame sent whole"
            ));
        }
    }
    let (nothing, bytes) = asked(&mut renderer, screen(2.0, true, ""), ToRenderer::PaintOver)?;
    assert!(
        matches!(&nothing, FromRenderer::PaintedOver(patch) if patch.area() == 0),
        "nothing changed crossed as {bytes} bytes"
    );
    Ok(())
}

#[test]
fn drawings_between_two_asks_cross_together_and_a_whole_frame_starts_them_again()
-> Result<(), String> {
    let mut renderer = Renderer::new(fonts());
    let (first, _) = asked(&mut renderer, screen(1.0, true, ""), ToRenderer::Paint)?;
    let FromRenderer::Painted(mut held) = first else {
        return Err(format!("a first frame crossed as {first:?}"));
    };
    // Drawn twice, asked once: a letter that opens the offers and ends the
    // conversation higher, then the caret moved back before it.
    match renderer.handle(ToRenderer::Start(Box::new(screen(1.0, true, "h")))) {
        FromRenderer::Loaded { .. } => {}
        other => return Err(format!("the first letter answered {other:?}")),
    }
    let mut moved = screen(1.0, true, "h");
    moved.editing = Editing::caret_at(0);
    let (answer, _) = asked(&mut renderer, moved.clone(), ToRenderer::PaintOver)?;
    if !held_after(&mut held, answer)? {
        return Err("two drawings crossed whole".to_owned());
    }
    assert_eq!(held, whole(moved)?, "both changes, every byte");
    // Asked whole, then for what changed: from the whole frame on.
    let (answer, _) = asked(&mut renderer, screen(1.0, true, "x"), ToRenderer::Paint)?;
    held_after(&mut held, answer)?;
    let (answer, _) = asked(
        &mut renderer,
        screen(1.0, false, "xy"),
        ToRenderer::PaintOver,
    )?;
    held_after(&mut held, answer)?;
    assert_eq!(
        held,
        whole(screen(1.0, false, "xy"))?,
        "the card closed, every byte"
    );
    // Another size crosses whole.
    let (answer, _) = asked(
        &mut renderer,
        screen(2.0, false, "xy"),
        ToRenderer::PaintOver,
    )?;
    assert!(
        !held_after(&mut held, answer)?,
        "a frame of another size crosses whole"
    );
    assert_eq!(held, whole(screen(2.0, false, "xy"))?);
    Ok(())
}
