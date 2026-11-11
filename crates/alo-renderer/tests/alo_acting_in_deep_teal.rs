/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 974 (ADR 0112, *Answered* 1; ADR 0074 § 2 as amended): deep
//! teal `#0F6B72` is alo acting, and it is never drawn alone. Wherever the
//! interface draws the agent at work — a chat it works in on the panel and
//! on the rail, the working line over a card's page — the teal comes with
//! **alo's mark** (a picture, the owner's small drawing) and **a word**,
//! read off the drawing in numbers: where the mark is, how big, and which
//! words are drawn in the teal. And terracotta, its colour before, is drawn
//! nowhere.

use alo_layout::{Rect, Size};
use alo_paint::DisplayItem;
use alo_renderer::acting::WORKING;
use alo_renderer::agent_work::{AtWork, Now, Step};
use alo_renderer::card::Card;
use alo_renderer::panel::{Chat, Doing, Panel};
use alo_renderer::start::Start;
use alo_renderer::turn::Turn;
use alo_renderer::{FromRenderer, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

/// Deep teal, alo acting.
const TEAL: (u8, u8, u8, u8) = (0x0f, 0x6b, 0x72, 0xff);

/// Terracotta, the acting colour until queue item 974.
const TERRACOTTA: (u8, u8, u8, u8) = (0xb5, 0x57, 0x3a, 0xff);

/// The corpus's faces for the generics, and the design's typefaces.
fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for (family, weight, bytes) in [
        ("DejaVu Sans", Weight::NORMAL, dejavu::sans::regular()),
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

/// What a part drew: the border box of each of alo's marks (the `<img>`s of
/// the class `acting-mark`) and how many of its fills are the owner's navy
/// arcs, `#1b3e5e` — the drawing of the mark itself, as alo-svg puts it
/// down — and each run of text with its colour, and every solid fill.
struct Drawn {
    marks: Vec<(Rect, usize)>,
    words: Vec<(String, (u8, u8, u8, u8))>,
    fills: Vec<(u8, u8, u8, u8)>,
}

/// The owner's navy, the mark's arcs.
const ARCS: (u8, u8, u8, u8) = (0x1b, 0x3e, 0x5e, 0xff);

fn drawn(work: ToRenderer) -> Result<Drawn, String> {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(work);
    if !matches!(answer, FromRenderer::Loaded { .. }) {
        return Err(format!("the part was not built: {answer:?}"));
    }
    let drawing = renderer.rendered().ok_or("nothing drawn")?;
    let document = renderer.document().ok_or("no document")?;
    let solid = |paint: &alo_paint::Paint| match paint {
        alo_paint::Paint::Solid(colour) => Some(colour.to_rgba8()),
        alo_paint::Paint::Gradient { .. } | alo_paint::Paint::Ramp(_) => None,
    };
    let marks = drawing
        .boxes
        .ids()
        .filter(|&id| {
            drawing
                .boxes
                .get(id)
                .filter(|made| !matches!(made.kind, alo_box::BoxKind::Text { .. }))
                .and_then(|made| made.kind.node())
                .is_some_and(|node| {
                    document.attribute_by_name(node, "class") == Some("acting-mark")
                })
        })
        .filter_map(|id| {
            let arcs = drawing
                .display
                .items()
                .iter()
                .filter(|item| match item {
                    DisplayItem::Fill { box_id, paint, .. } => {
                        *box_id == id && solid(paint) == Some(ARCS)
                    }
                    _ => false,
                })
                .count();
            drawing
                .layout
                .get(id)
                .map(|geometry| (geometry.border_box, arcs))
        })
        .collect();
    let mut found = Drawn {
        marks,
        words: Vec::new(),
        fills: Vec::new(),
    };
    for item in drawing.display.items() {
        match item {
            DisplayItem::Text { text, color, .. } => {
                found.words.push((text.clone(), color.to_rgba8()));
            }
            DisplayItem::Fill { paint, .. } => {
                if let Some(colour) = solid(paint) {
                    found.fills.push(colour);
                }
            }
            _ => {}
        }
    }
    Ok(found)
}

/// Each mark's size, and whether it was drawn (some of its arcs filled).
fn marks(drawn: &Drawn) -> Vec<((f32, f32), bool)> {
    drawn
        .marks
        .iter()
        .map(|(rect, arcs)| (square(rect), *arcs > 0))
        .collect()
}

fn in_teal(drawn: &Drawn) -> Vec<&str> {
    drawn
        .words
        .iter()
        .filter(|(_, colour)| *colour == TEAL)
        .map(|(text, _)| text.as_str())
        .collect()
}

fn square(rect: &Rect) -> (f32, f32) {
    (rect.size.width, rect.size.height)
}

#[test]
fn a_chat_at_work_on_the_panel_has_alos_mark_and_working_in_teal() -> Result<(), String> {
    let drawn = drawn(ToRenderer::Panel(Box::new(Panel {
        chats: vec![
            Chat::new("Charging my EV", "evhomeguide.org").current(),
            Chat::new("Weekend in Lisbon", "lisbon.example").doing(Doing::Working),
        ],
        viewport: Size::new(240.0, 560.0),
        scale: 1.0,
        ..Panel::default()
    })))?;
    assert_eq!(in_teal(&drawn), vec![WORKING], "the word, in the teal");
    // The head's mark is drawn inline; the chat's is a picture of its own,
    // 14 square, on the working chat's row, and drawn.
    assert_eq!(marks(&drawn), vec![((14.0, 14.0), true)]);
    assert!(!drawn.fills.contains(&TERRACOTTA));
    Ok(())
}

#[test]
fn a_chat_at_work_on_the_rail_has_alos_mark_and_working_in_teal() -> Result<(), String> {
    let drawn = drawn(ToRenderer::Panel(Box::new(Panel {
        chats: vec![
            Chat::new("Charging my EV", "").current(),
            Chat::new("Weekend in Lisbon", "").doing(Doing::Working),
        ],
        viewport: Size::new(320.0, 600.0),
        hidden: true,
        ..Panel::default()
    })))?;
    assert_eq!(in_teal(&drawn), vec![WORKING]);
    // 12 square, on the word's line under the circle (queue item 985).
    assert_eq!(marks(&drawn), vec![((12.0, 12.0), true)]);
    assert!(!drawn.fills.contains(&TERRACOTTA));
    Ok(())
}

#[test]
fn two_chats_at_work_have_a_mark_each() -> Result<(), String> {
    let drawn = drawn(ToRenderer::Panel(Box::new(Panel {
        chats: vec![
            Chat::new("Weekend in Lisbon", "").doing(Doing::Working),
            Chat::new("Invoice for March", "").doing(Doing::Working),
        ],
        viewport: Size::new(240.0, 560.0),
        ..Panel::default()
    })))?;
    assert_eq!(in_teal(&drawn), vec![WORKING, WORKING]);
    assert_eq!(marks(&drawn), vec![((14.0, 14.0), true); 2]);
    Ok(())
}

#[test]
fn the_working_line_over_a_cards_page_has_alos_mark_its_words_and_the_ring_in_teal()
-> Result<(), String> {
    let url = alo_url::parse("http://127.0.0.1:8765/card.html")
        .map_err(|_| "the card's address".to_owned())?;
    let mut card = Card::new("127.0.0.1:8765/card.html", url).loaded();
    card.agent = Some(AtWork {
        plan: None,
        asked: "Send my name".to_owned(),
        steps: vec![Step {
            line: "Pressing Send".to_owned(),
            why: String::new(),
        }],
        now: Now::Acting,
        choosing: None,
    });
    let drawn = drawn(ToRenderer::Start(Box::new(Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(1040.0, 820.0),
        turns: vec![Turn::Page(card)],
        ..Start::default()
    })))?;
    // The working line's words, *Stop* and the step under way: the teal.
    let teal = in_teal(&drawn);
    assert!(
        teal.iter().any(|text| text.starts_with("alo is working")),
        "{teal:?}"
    );
    assert!(teal.contains(&"Stop"), "{teal:?}");
    assert!(teal.contains(&"Pressing Send"), "{teal:?}");
    // alo's mark, 16 square, is drawn before them.
    assert_eq!(marks(&drawn), vec![((16.0, 16.0), true)]);
    // The tint behind the line, and nothing in terracotta.
    assert!(drawn.fills.contains(&(0xe2, 0xed, 0xee, 0xff)));
    assert!(!drawn.fills.contains(&TERRACOTTA));
    Ok(())
}
