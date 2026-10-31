/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 706: the interface's text is shaped in the design's own
//! typefaces (ADR 0084) — the mark and the greeting in Newsreader, every
//! other word in Instrument Sans, each set to the weight its stylesheet asks
//! for — and, for a renderer never handed them, in the generics its
//! stylesheet falls back to.
//!
//! Read off the drawing itself: every run of text the panel and the first
//! screen put down, with the family and the weight of the font the shaper
//! drew it in. Not a picture somebody looked at.

use alo_layout::Size;
use alo_paint::DisplayItem;
use alo_renderer::panel::{Chat, Doing, Panel};
use alo_renderer::start::{KeptSite, Start};
use alo_renderer::typed::offers;
use alo_renderer::typefaces::{self, INSTRUMENT_SANS, NEWSREADER};
use alo_renderer::{FromRenderer, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

/// The corpus's faces and generics, as every interface test has them, and —
/// when `typefaces` — the design's two after them, as the browser process
/// hands an interface renderer its fonts.
fn fonts(typefaces: bool) -> FontDatabase {
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
    if typefaces {
        for font in typefaces::fonts() {
            database.add(font);
        }
    }
    database
}

/// One run of text as it was drawn: what it says, and the family and weight
/// of the font it was shaped in.
type Run = (String, String, u16);

/// Every run of text a part's drawing put down, in order.
fn runs(work: ToRenderer, typefaces: bool) -> Vec<Run> {
    let mut renderer = Renderer::new(fonts(typefaces));
    let answer = renderer.handle(work);
    assert!(
        matches!(answer, FromRenderer::Loaded { .. }),
        "the part was built: {answer:?}"
    );
    renderer
        .rendered()
        .map(|drawing| {
            drawing
                .display
                .items()
                .iter()
                .filter_map(|item| match item {
                    DisplayItem::Text { text, font, .. } => Some((
                        text.clone(),
                        font.family().to_owned(),
                        font.weight().value(),
                    )),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The design's panel: the open chat, one at work, one done, one from before.
fn panel() -> ToRenderer {
    ToRenderer::Panel(Box::new(Panel {
        chats: vec![
            Chat::new("Charging my EV", "evhomeguide.org").current(),
            Chat::new("Weekend in Lisbon", "lisbon.example").doing(Doing::Working),
            Chat::new("Invoice for March", "books.example").doing(Doing::Done),
            Chat::new("Compare phone plans", "").earlier(),
        ],
        viewport: Size::new(240.0, 560.0),
        scale: 1.0,
        ..Panel::default()
    }))
}

/// The design's first screen with words typed in the box and offered.
fn start() -> ToRenderer {
    ToRenderer::Start(Box::new(Start {
        greeting: "Good morning.".to_owned(),
        sites: vec![KeptSite::new("Mail", "https://mail.example/")],
        viewport: Size::new(1040.0, 820.0),
        typed: "evhomeguide.org".to_owned(),
        focused: true,
        offers: offers("evhomeguide.org"),
        ..Start::default()
    }))
}

/// The family and weight a run saying exactly `text` was drawn in.
fn drawn_in<'a>(runs: &'a [Run], text: &str) -> Option<(&'a str, u16)> {
    runs.iter()
        .find(|(said, _, _)| said == text)
        .map(|(_, family, weight)| (family.as_str(), *weight))
}

#[test]
fn the_panels_mark_is_newsreader_and_its_words_instrument_sans() {
    let runs = runs(panel(), true);
    assert_eq!(drawn_in(&runs, "alo"), Some((NEWSREADER, 400)), "{runs:?}");
    assert_eq!(
        drawn_in(&runs, "Weekend in Lisbon"),
        Some((INSTRUMENT_SANS, 400)),
        "{runs:?}"
    );
    // The chat on screen is 600, the variable font set to that instance.
    assert_eq!(
        drawn_in(&runs, "Charging my EV"),
        Some((INSTRUMENT_SANS, 600)),
        "{runs:?}"
    );
    assert_eq!(
        drawn_in(&runs, "New chat"),
        Some((INSTRUMENT_SANS, 600)),
        "{runs:?}"
    );
    // Instrument Sans has no `⌘`, so *New chat*'s `⌘T` is two runs: the `⌘`
    // in the stylesheet's next family that has one — here the corpus's
    // `system-ui` — and the `T` in Instrument Sans, as a browser falls back
    // glyph by glyph (`alo-paint`'s `ink_runs`). Nothing else in the panel
    // is drawn in anything but the design's two.
    assert_eq!(drawn_in(&runs, "⌘"), Some(("DejaVu Sans", 400)), "{runs:?}");
    assert_eq!(
        drawn_in(&runs, "T"),
        Some((INSTRUMENT_SANS, 400)),
        "{runs:?}"
    );
    let elsewhere: Vec<&str> = runs
        .iter()
        .filter(|(_, family, _)| family != INSTRUMENT_SANS && family != NEWSREADER)
        .map(|(text, _, _)| text.as_str())
        .collect();
    assert_eq!(elsewhere, vec!["⌘"]);
}

#[test]
fn the_first_screens_greeting_is_newsreader_and_its_words_instrument_sans() {
    let runs = runs(start(), true);
    assert_eq!(
        drawn_in(&runs, "Good morning."),
        Some((NEWSREADER, 400)),
        "{runs:?}"
    );
    assert_eq!(
        drawn_in(&runs, "Go to the site"),
        Some((INSTRUMENT_SANS, 400)),
        "{runs:?}"
    );
    let elsewhere: Vec<&Run> = runs
        .iter()
        .filter(|(_, family, _)| family != INSTRUMENT_SANS && family != NEWSREADER)
        .collect();
    assert_eq!(elsewhere, Vec::<&Run>::new());
}

#[test]
fn a_renderer_never_handed_them_draws_the_stylesheets_generics() {
    let panel = runs(panel(), false);
    assert_eq!(
        drawn_in(&panel, "alo"),
        Some(("DejaVu Serif", 400)),
        "{panel:?}"
    );
    assert_eq!(
        drawn_in(&panel, "Weekend in Lisbon"),
        Some(("DejaVu Sans", 400)),
        "{panel:?}"
    );
    let start = runs(start(), false);
    assert_eq!(
        drawn_in(&start, "Good morning."),
        Some(("DejaVu Serif", 400)),
        "{start:?}"
    );
}
