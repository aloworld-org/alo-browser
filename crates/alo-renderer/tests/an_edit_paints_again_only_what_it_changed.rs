/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 870 (ADR 0101): a letter typed into the one box beside a live
//! card paints again only what it changed — the box and the offers under
//! it — and never the card's shadows, fills, clips or the place left clear
//! for its page, which took the first screen's renderer 103 ms of the 137
//! an edit cost at two.
//!
//! Held by what was painted, in device pixels — never by a clock:
//!
//! - each letter after the first paints again a region of at most a
//!   twentieth of the frame, at the box, beside a card and with none;
//! - the first letter beside a card, which opens the offers and ends the
//!   conversation's clip higher, and emptying the box, which closes them,
//!   each paint again at most a quarter of the frame — the box, the offers
//!   and the strip between the clip's two feet — and blur neither of the
//!   card's shadows again (queue item 871);
//! - every edit — a letter, the offers opening and closing, a selection,
//!   typing over it, the caret moved, the card closed — comes out exactly,
//!   every byte of every pixel, as a renderer that never drew the screen
//!   before paints it, at one and at two.

use alo_paint::Repainted;
use alo_renderer::card::Card;
use alo_renderer::field::Editing;
use alo_renderer::start::Start;
use alo_renderer::turn::Turn;
use alo_renderer::typed::offers_for;
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

#[test]
fn a_letter_beside_a_card_paints_again_only_the_box_and_its_offers() -> Result<(), String> {
    for card in [true, false] {
        let mut renderer = Renderer::new(fonts());
        painted(&mut renderer, screen(2.0, card, ""))?;
        painted(&mut renderer, screen(2.0, card, "h"))?;
        for typed in ["he", "hel", "hell", "hello"] {
            let (frame, repainted) = painted(&mut renderer, screen(2.0, card, typed))?;
            let whole = u64::from(frame.width) * u64::from(frame.height);
            let Repainted::Within(region) = repainted else {
                return Err(format!(
                    "typing {typed:?} with a card {card} painted {repainted:?}, \
                     where only the box and its offers changed"
                ));
            };
            assert!(
                region.area() * 20 <= whole,
                "typing {typed:?} with a card {card} painted {region:?}, {} of {whole} pixels",
                region.area(),
            );
        }
    }
    Ok(())
}

#[test]
fn the_offers_opening_and_closing_beside_a_card_paint_again_only_where_the_clip_moved()
-> Result<(), String> {
    let mut renderer = Renderer::new(fonts());
    painted(&mut renderer, screen(2.0, true, ""))?;
    for (edit, typed) in [("opening the offers", "h"), ("closing them", "")] {
        let (frame, repainted) = painted(&mut renderer, screen(2.0, true, typed))?;
        let whole = u64::from(frame.width) * u64::from(frame.height);
        let Repainted::Within(region) = repainted else {
            return Err(format!(
                "{edit} beside a card painted {repainted:?}, where only the box, the \
                 offers and the conversation's foot changed"
            ));
        };
        assert!(
            region.area() * 4 <= whole,
            "{edit} beside a card painted {region:?}, {} of {whole} pixels",
            region.area(),
        );
        let made = renderer
            .rendered()
            .map(|drawing| drawing.kept.made())
            .ok_or("no drawing was kept")?;
        assert_eq!(
            made.blurred, 0,
            "{edit} beside a card blurred its shadows again: {made:?}"
        );
    }
    Ok(())
}

#[test]
fn every_edit_at_one_comes_out_exactly_as_drawn_whole() -> Result<(), String> {
    each_exactly_as_drawn_whole(1.0, &EVERY)
}

#[test]
fn the_offers_opening_a_letter_and_a_selection_at_two_come_out_exactly_as_drawn_whole()
-> Result<(), String> {
    each_exactly_as_drawn_whole(2.0, &EVERY[..6])
}

#[test]
fn the_offers_closing_and_the_card_going_at_two_come_out_exactly_as_drawn_whole()
-> Result<(), String> {
    each_exactly_as_drawn_whole(2.0, &EVERY[5..9])
}

/// The edits a person makes in turn: what each is, what is typed, where
/// the caret and the selection's start are, and whether the card is open.
type Edit = (&'static str, &'static str, (usize, usize), bool);

/// Every edit, in turn. At two, a frame is four times the pixels, so its
/// two tests take them in two runs, each a road of its own: the offers
/// opening, a letter, a selection and typing over it; then, from a renderer
/// that drew none of that, the offers closing and the card going.
const EVERY: [Edit; 11] = [
    ("empty", "", (0, 0), true),
    ("the first letter, opening the offers", "h", (1, 1), true),
    ("a letter", "hi", (2, 2), true),
    ("words", "hi there", (8, 8), true),
    ("all of it selected", "hi there", (8, 0), true),
    ("typed over", "x", (1, 1), true),
    ("the caret moved", "xyz", (1, 1), true),
    ("emptied, closing the offers", "", (0, 0), true),
    ("the card closed", "", (0, 0), false),
    ("a letter with no card", "h", (1, 1), false),
    ("another", "ha", (2, 2), false),
];

/// Each of `edits` at `scale`, painted in turn by one renderer, against a
/// renderer that never drew before.
fn each_exactly_as_drawn_whole(scale: f32, edits: &[Edit]) -> Result<(), String> {
    let mut renderer = Renderer::new(fonts());
    for &(edit, typed, (caret, anchor), card) in edits {
        let mut start = screen(scale, card, typed);
        start.editing = Editing::caret_at(caret).selecting_from(anchor);
        let (frame, _) = painted(&mut renderer, start.clone())?;
        if let Some((x, y)) = first_difference(&frame, &drawn_whole(start)?) {
            return Err(format!(
                "{edit} at {scale} was painted otherwise than drawn whole, first at {x}, {y}"
            ));
        }
    }
    Ok(())
}
