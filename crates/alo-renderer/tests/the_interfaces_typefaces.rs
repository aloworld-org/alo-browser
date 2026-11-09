/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue items 706, 853 and 854: the interface's text is shaped in the
//! design's own typefaces (ADR 0084) — the mark and the greeting in
//! Newsreader, every other word in Inter, or in the face a person chose in
//! *Settings*, each set to the weight its stylesheet asks for — and, for a
//! renderer never handed them, in the generics its stylesheet falls back
//! to.
//!
//! Read off the drawing itself: every run of text the panel and the first
//! screen put down, with the family and the weight of the font the shaper
//! drew it in. Not a picture somebody looked at.

use alo_layout::Size;
use alo_paint::DisplayItem;
use alo_renderer::interface_face::InterfaceFace;
use alo_renderer::panel::{Chat, Doing, Panel};
use alo_renderer::settings_document::SAMPLE;
use alo_renderer::start::{KeptSite, Start};
use alo_renderer::typed::offers;
use alo_renderer::typefaces::{self, INTER, NEWSREADER};
use alo_renderer::{FromRenderer, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

/// The corpus's faces and generics, as every interface test has them, and —
/// when `typefaces` — the design's after them, as the browser process
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
fn the_panels_mark_is_newsreader_and_its_words_inter() {
    let runs = runs(panel(), true);
    assert_eq!(drawn_in(&runs, "alo"), Some((NEWSREADER, 400)), "{runs:?}");
    assert_eq!(
        drawn_in(&runs, "Weekend in Lisbon"),
        Some((INTER, 400)),
        "{runs:?}"
    );
    // The chat on screen is 600, the variable font set to that instance.
    assert_eq!(
        drawn_in(&runs, "Charging my EV"),
        Some((INTER, 600)),
        "{runs:?}"
    );
    assert_eq!(drawn_in(&runs, "New chat"), Some((INTER, 600)), "{runs:?}");
    // Inter has a `⌘`, so *New chat*'s `⌘T` is one run in it (queue item
    // 853; Instrument Sans has none, below). Nothing in the panel is drawn
    // in anything but Inter and Newsreader.
    assert_eq!(drawn_in(&runs, "⌘T"), Some((INTER, 400)), "{runs:?}");
    let elsewhere: Vec<&str> = runs
        .iter()
        .filter(|(_, family, _)| family != INTER && family != NEWSREADER)
        .map(|(text, _, _)| text.as_str())
        .collect();
    assert_eq!(elsewhere, Vec::<&str>::new());
}

#[test]
fn the_first_screens_greeting_is_newsreader_and_its_words_inter() {
    let runs = runs(start(), true);
    assert_eq!(
        drawn_in(&runs, "Good morning."),
        Some((NEWSREADER, 400)),
        "{runs:?}"
    );
    assert_eq!(
        drawn_in(&runs, "Go to the site"),
        Some((INTER, 400)),
        "{runs:?}"
    );
    let elsewhere: Vec<&Run> = runs
        .iter()
        .filter(|(_, family, _)| family != INTER && family != NEWSREADER)
        .collect();
    assert_eq!(elsewhere, Vec::<&Run>::new());
}

/// The same part, its words set in `face` (queue item 854).
fn set_in(work: ToRenderer, face: InterfaceFace) -> ToRenderer {
    match work {
        ToRenderer::Panel(mut panel) => {
            panel.face = face;
            ToRenderer::Panel(panel)
        }
        ToRenderer::Start(mut start) => {
            start.face = face;
            ToRenderer::Start(start)
        }
        other => other,
    }
}

#[test]
fn a_face_chosen_sets_every_word_of_both_parts_and_never_the_mark_or_the_greeting() {
    for face in InterfaceFace::ALL {
        let family = face.family();
        let panel = runs(set_in(panel(), face), true);
        let start = runs(set_in(start(), face), true);
        assert_eq!(drawn_in(&panel, "alo"), Some((NEWSREADER, 400)), "{face:?}");
        assert_eq!(
            drawn_in(&start, "Good morning."),
            Some((NEWSREADER, 400)),
            "{face:?}"
        );
        assert_eq!(
            drawn_in(&panel, "Charging my EV"),
            Some((family, 600)),
            "{face:?}"
        );
        assert_eq!(
            drawn_in(&start, "Go to the site"),
            Some((family, 400)),
            "{face:?}"
        );
        // Every other word in the face; only a `⌘` the face lacks is drawn
        // by the next family that has one, as a browser falls back.
        let elsewhere: Vec<&str> = panel
            .iter()
            .chain(&start)
            .filter(|(_, drawn, _)| drawn != family && drawn != NEWSREADER)
            .map(|(text, _, _)| text.as_str())
            .collect();
        let lacking = if face == InterfaceFace::Inter {
            Vec::<&str>::new()
        } else {
            vec!["⌘"]
        };
        assert_eq!(elsewhere, lacking, "{face:?}");
        // The rail too: its initials and the hovered chat's name, which are
        // a button's words (a control's own face until 854 found them).
        let rail = runs(
            ToRenderer::Panel(Box::new(Panel {
                chats: vec![
                    Chat::new("Weekend in Lisbon", "").current().hovered(),
                    Chat::new("rust", ""),
                ],
                viewport: Size::new(320.0, 600.0),
                hidden: true,
                face,
                ..Panel::default()
            })),
            true,
        );
        assert_eq!(drawn_in(&rail, "WI"), Some((family, 600)), "{rail:?}");
        assert_eq!(drawn_in(&rail, "R"), Some((family, 600)), "{rail:?}");
        assert_eq!(
            drawn_in(&rail, "Weekend in Lisbon"),
            Some((family, 500)),
            "{rail:?}"
        );
    }
}

#[test]
fn settings_draws_each_face_in_itself_whatever_the_page_is_set_in() {
    for chosen in [InterfaceFace::Inter, InterfaceFace::PlusJakartaSans] {
        let runs = runs(
            ToRenderer::Start(Box::new(Start {
                viewport: Size::new(1040.0, 820.0),
                face: chosen,
                settings: true,
                ..Start::default()
            })),
            true,
        );
        // The page's own words in the face chosen.
        assert_eq!(
            drawn_in(&runs, "Settings"),
            Some((chosen.family(), 600)),
            "{runs:?}"
        );
        assert_eq!(drawn_in(&runs, "Font"), Some((chosen.family(), 600)));
        // Each face's name and its sample in that face: the sample's
        // first line, at 400 (queue item 901).
        let samples: Vec<(&str, u16)> = runs
            .iter()
            .filter(|(text, _, _)| text.starts_with("If you live") && SAMPLE.starts_with(text))
            .map(|(_, family, weight)| (family.as_str(), *weight))
            .collect();
        assert_eq!(
            samples,
            InterfaceFace::ALL
                .iter()
                .map(|face| (face.family(), 400))
                .collect::<Vec<_>>(),
            "{runs:?}"
        );
        for face in InterfaceFace::ALL {
            let name = if face == InterfaceFace::Inter {
                "Inter \u{b7} Default".to_owned()
            } else {
                face.name().to_owned()
            };
            assert_eq!(drawn_in(&runs, &name), Some((face.family(), 600)), "{name}");
        }
    }
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
