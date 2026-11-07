/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue items 855 and 857: alo's menu over a card's page (queue item 798)
//! ends the words of *Search for “…”* with an ellipsis where the line
//! reaches the menu's edge, and the closing quote follows the ellipsis
//! directly, in each of the five faces the interface may be set in.
//!
//! Found by the hand run of 7 November 2026 (`handrun34/08`): the line ran
//! past the menu's right edge, its closing quote cut. A cut at sixteen
//! characters, made then, was measured to fit ordinary words in Instrument
//! Sans and not sixteen wide letters in any face (`W` 277 to 299 px; the
//! line's room is 196). So item 855 had the stylesheet cut them at the
//! menu's width — and the next hand run (`handrun35/17`) found the quote
//! pinned at the menu's edge, as far from the `…` as the part of a letter
//! that did not fit: a clipping box is as wide as its room. So the renderer
//! now cuts the words by their measured width (`menu_fit.rs`, item 857).
//!
//! What closes it, each a test below: a layout assertion, in numbers, of
//! the line's three pieces in every face — the words already ending in
//! `…`, the quote's box starting where theirs ends, and less room left
//! after the quote than one more `W` would take — and short words drawn
//! whole; and a reference render, `references/card-menu-cut.png`.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test the_page_menus_words_cut_at_its_edge`
//! rewrites the reference. A rewritten reference is a diff somebody reads.

use alo_layout::{MeasureText, TextStyle};
use alo_layout::{Rect, Size};
use alo_paint::{DisplayItem, from_png};
use alo_renderer::card::{Card, PAGE_SIZE};
use alo_renderer::card_menu::CardMenu;
use alo_renderer::frame::frame_to_png;
use alo_renderer::interface_face::InterfaceFace;
use alo_renderer::start::Start;
use alo_renderer::turn::Turn;
use alo_renderer::{Frame, FromRenderer, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, TextMeasurer, Weight};
use std::path::{Path, PathBuf};

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
    database.map_generic("system-ui", "DejaVu Sans");
    database.map_generic("sans-serif", "DejaVu Sans");
    database.map_generic("serif", "DejaVu Serif");
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

/// The card's frame `a_page_as_a_card.rs` holds: Chrome's own.
fn frame() -> Result<Vec<u8>, String> {
    let path = reference("card-frame.png");
    std::fs::read(&path).map_err(|why| format!("{} is not there: {why}", path.display()))
}

/// A card, set in `face`, with alo's menu at 60, 52 of its page over a
/// link, `selected` selected.
fn with_a_menu(face: InterfaceFace, selected: &str) -> Result<Start, String> {
    let url = alo_url::parse("http://127.0.0.1:8765/card.html")
        .unwrap_or_else(|_| alo_url::Url::about_blank());
    let mut card = Card {
        loading: false,
        ..Card::new("127.0.0.1:8765/card.html", url).holding(frame()?)
    };
    card.menu = Some(CardMenu::placed(
        (60, 52),
        PAGE_SIZE,
        CardMenu::offering(Some("http://127.0.0.1:8765/elsewhere.html"), Some(selected)),
    ));
    Ok(Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(1040.0, 820.0),
        turns: vec![Turn::Page(card)],
        face,
        ..Start::default()
    })
}

fn laid_out(start: Start) -> Renderer {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Start(Box::new(start)));
    assert!(
        matches!(&answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{answer:?}"
    );
    renderer
}

/// The border box of every element of `class`, in document order.
fn rects_of(renderer: &Renderer, class: &str) -> Vec<Rect> {
    let (Some(drawing), Some(document)) = (renderer.rendered(), renderer.document()) else {
        return Vec::new();
    };
    drawing
        .boxes
        .ids()
        .filter(|&id| {
            drawing
                .boxes
                .get(id)
                .filter(|made| !matches!(made.kind, alo_box::BoxKind::Text { .. }))
                .and_then(|made| made.kind.node())
                .is_some_and(|node| document.attribute_by_name(node, "class") == Some(class))
        })
        .filter_map(|id| drawing.layout.get(id))
        .map(|geometry| geometry.border_box)
        .collect()
}

/// The runs of text *Search for “…”* is drawn as, and the family of each
/// one's face: from its opening words to its closing quote.
fn searched(renderer: &mut Renderer) -> Vec<(String, String)> {
    let all = runs(renderer);
    let from = all
        .iter()
        .position(|(text, _)| text == "Search for \u{201c}");
    let to = all.iter().rposition(|(text, _)| text == "\u{201d}");
    match (from, to) {
        (Some(from), Some(to)) if from <= to => all.get(from..=to).unwrap_or_default().to_vec(),
        _ => all,
    }
}

/// Every run of text drawn, and the family of its face.
fn runs(renderer: &mut Renderer) -> Vec<(String, String)> {
    let _ = renderer.handle(ToRenderer::Paint);
    renderer
        .rendered()
        .map(|drawing| {
            drawing
                .display
                .items()
                .iter()
                .filter_map(|item| match item {
                    DisplayItem::Text { text, font, .. } => {
                        Some((text.clone(), font.family().to_owned()))
                    }
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Words wider than the menu in every face: sixteen `W`s, then more.
const WIDE: &str = "WWWWWWWWWWWWWWWW and the hills of Kampala by the lake";

/// The menu's lines run from 142 to 360 (218 inside its border); a line's
/// words stand 12 inside it, from 154 to 348.
const WORDS_FROM: f32 = 154.0;
const WORDS_TO: f32 = 348.0;

/// How wide `text` is in `face`, as the menu's line sets it.
fn width_in(face: InterfaceFace, text: &str) -> f32 {
    let fonts = fonts();
    let style = TextStyle {
        families: vec![
            face.family().to_owned(),
            "system-ui".to_owned(),
            "sans-serif".to_owned(),
        ],
        size: 13.0,
        weight: 500,
        white_space: alo_box::WhiteSpace::Pre,
        ..TextStyle::default()
    };
    TextMeasurer::new(&fonts).measure(text, &style, None).width
}

#[test]
fn wide_words_end_in_an_ellipsis_the_quote_right_after_it_in_every_face() -> Result<(), String> {
    for face in InterfaceFace::ALL {
        let mut renderer = laid_out(with_a_menu(face, WIDE)?);
        let one = |class: &str| -> Result<Rect, String> {
            match rects_of(&renderer, class).as_slice() {
                [rect] => Ok(*rect),
                other => Err(format!("{face:?}: one {class}: {other:?}")),
            }
        };
        let (lead, words, end) = (one("mlead")?, one("mwords")?, one("mend")?);
        // The three pieces one after another: the opening words at the
        // line's start, the words' box as wide as the letters kept and the
        // `…`, the quote's box starting where it ends.
        assert!(
            (lead.left() - WORDS_FROM).abs() < 0.02,
            "{face:?}: {lead:?}"
        );
        assert!(
            (words.left() - lead.right()).abs() < 0.02,
            "{face:?}: {words:?}"
        );
        assert!(
            (end.left() - words.right()).abs() < 0.02,
            "{face:?}: {end:?}"
        );
        // Drawn cut: as many `W`s as the room holds, then `…` in the same
        // run — no ellipsis of the stylesheet's, so no room between them
        // and the quote — then the quote, every piece in the face.
        let runs = searched(&mut renderer);
        let texts: Vec<&str> = runs.iter().map(|(text, _)| text.as_str()).collect();
        let kept = match texts.as_slice() {
            ["Search for \u{201c}", cut, "\u{201d}"] => cut
                .strip_suffix('\u{2026}')
                .filter(|kept| !kept.is_empty() && kept.chars().all(|letter| letter == 'W'))
                .ok_or_else(|| format!("{face:?}: {runs:?}"))?,
            _ => return Err(format!("{face:?}: {runs:?}")),
        };
        assert!(
            runs.iter().all(|(_, drawn)| drawn == face.family()),
            "{face:?}: {runs:?}"
        );
        // The words' box as wide as what it holds.
        let held = width_in(face, &format!("{kept}\u{2026}"));
        assert!(
            (words.size.width - held).abs() < 0.02,
            "{face:?}: {words:?} holds {held}"
        );
        // Inside the line, and the most that fits: one more `W` would not.
        assert!(end.right() <= WORDS_TO + 0.02, "{face:?}: {end:?}");
        let one_more = width_in(face, &format!("{kept}W\u{2026}")) - held;
        assert!(
            WORDS_TO - end.right() < one_more,
            "{face:?}: {} left after the quote, and a W is {one_more}",
            WORDS_TO - end.right()
        );
        // And the line is the menu's 30, as every other.
        assert!(words.size.height <= 30.02, "{face:?}: {words:?}");
    }
    Ok(())
}

#[test]
fn short_words_are_drawn_whole_and_the_quote_follows_them() -> Result<(), String> {
    for face in InterfaceFace::ALL {
        let mut renderer = laid_out(with_a_menu(face, "Kampala")?);
        let ends = rects_of(&renderer, "mend");
        assert!(
            ends.iter().all(|end| end.right() < WORDS_TO - 40.0),
            "{face:?}: the quote right after the words: {ends:?}"
        );
        let runs = searched(&mut renderer);
        let texts: Vec<&str> = runs.iter().map(|(text, _)| text.as_str()).collect();
        assert_eq!(
            texts,
            ["Search for \u{201c}", "Kampala", "\u{201d}"],
            "{face:?}: nothing cut"
        );
    }
    Ok(())
}

#[test]
fn the_menu_cut_is_drawn_as_the_reference() -> Result<(), String> {
    let mut renderer = laid_out(with_a_menu(InterfaceFace::Inter, WIDE)?);
    let frame = match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => frame,
        other => return Err(format!("{other:?}")),
    };
    let path = reference("card-menu-cut.png");
    let drawn = frame_to_png(&frame).map_err(|why| why.to_string())?;
    if std::env::var_os("ALO_UPDATE_REFERENCES").is_some() {
        return std::fs::write(&path, &drawn).map_err(|why| why.to_string());
    }
    let committed = std::fs::read(&path).map_err(|why| {
        format!(
            "{} is not there ({why}); ALO_UPDATE_REFERENCES=1 writes it",
            path.display()
        )
    })?;
    let expected = Frame::from_canvas(&from_png(&committed).map_err(|why| why.to_string())?);
    assert_eq!(
        (frame.width, frame.height),
        (expected.width, expected.height)
    );
    let moved = frame
        .pixels
        .chunks(4)
        .zip(expected.pixels.chunks(4))
        .position(|(drawn, committed)| drawn != committed);
    assert_eq!(
        moved, None,
        "card-menu-cut.png: pixels moved. If this is intended, ALO_UPDATE_REFERENCES=1 \
         rewrites the reference"
    );
    Ok(())
}
