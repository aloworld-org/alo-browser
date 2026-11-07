/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 869 (ADR 0100) through **the window's own road**: the
//! functions `alo --conversation`'s event loop calls for each event
//! ([`ConversationWindow`]), the confined renderers that draw the panel and
//! the first screen, and the one that draws the card's page, every thread's
//! news given back in the order it was told.
//!
//! With a live card streaming beside the box, a key's edit took up to a
//! second to be drawn: each of Chrome's frames painted the whole first
//! screen again in the renderer that draws the box — about two hundred
//! milliseconds at two, measured — and an edit waited behind the frame
//! being painted. Now a frame of the card asks for its picture alone,
//! drawn by a renderer of its own beneath the first screen, which leaves
//! the page's place clear. Each asserted, in order, on the order things
//! were told and never on a wait:
//!
//! 1. The card's first frame seen, its own pixels in the page box.
//! 2. Its next frames: each seen in its place, and **the first screen not
//!    drawn again for any of them** — no frame painted, nothing drawn
//!    through — and the window not drawn again until the picture is; then
//!    the page's place left clear in the first screen's frame at 81, 123,
//!    878 × 520 of it, the newest frame beneath it at that size.
//! 3. A letter typed between two frames: the first frame of the first
//!    screen painted after the key is the edit's, the next frame's picture
//!    seen beneath it.
//! 4. Another card: none of the first card's pictures is ever seen in its
//!    place, and its own is.
//!
//! The card's Chromium is one that is not there; its frames are the test's.

#![cfg(target_os = "macos")]

use alo_renderer::Frame;
use alo_renderer::beneath::Beneath;
use alo_renderer::frame::frame_to_png;
use alo_renderer::host::Renderers;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::conversation::Field;
use alo_window::conversation_chrome::{ChromeOpener, Went};
use alo_window::conversation_compose::ConversationScene;
use alo_window::conversation_drawing::{ConversationDrawing, Drawn, Part};
use alo_window::conversation_window::{ConversationWindow, Heard, Measured};
use alo_window::keys::Press;
use alo_window::scale::Scale;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long anything is waited for.
const AT_MOST: Duration = Duration::from_secs(60);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The card's page box at one: 878 × 520.
const PAGE: (u32, u32) = (878, 520);

/// Where the first screen leaves the page's place clear at one, in its
/// own pixels, for a conversation of the card alone: the page box, 81 in
/// from its left and 123 down — the card's 80 and its 1 of border, under
/// the person's line and the card's bar.
const PLACE: Beneath = Beneath {
    left: 81,
    top: 123,
    width: PAGE.0,
    height: PAGE.1,
};

/// The box at the window's foot, under a card.
const THE_BOX: (f64, f64) = (640.0, 768.0);

fn renderer() -> Result<PathBuf, String> {
    let path = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "no renderer at {}: `cargo test --workspace` builds it, `cargo test -p alo-window` \
             alone does not",
            path.display()
        ))
    }
}

/// A frame of the card's page, its pixels its own for `seed`.
fn frame(seed: u32) -> Frame {
    let mut pixels = Vec::new();
    for y in 0..PAGE.1 {
        for x in 0..PAGE.0 {
            let byte = |shift: u32| u8::try_from((x * 7 + y * 3 + seed * 41 + shift) % 256);
            pixels.extend_from_slice(&[
                byte(0).unwrap_or(0),
                byte(85).unwrap_or(0),
                byte(170).unwrap_or(0),
                255,
            ]);
        }
    }
    Frame {
        width: PAGE.0,
        height: PAGE.1,
        pixels,
    }
}

/// The conversation's window, and what its threads told it.
struct Driven {
    window: ConversationWindow,
    heard: Receiver<Heard>,
}

impl Driven {
    /// Give the window what its threads told it, in order, until `done`
    /// holds of it and what it last asked is drawn — the first screen and
    /// the card's page — keeping each piece of news the window was given.
    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) -> Result<Vec<Told>, String> {
        let until = Instant::now() + AT_MOST;
        let mut told = Vec::new();
        while !(done(self) && self.window.drawn_as_asked()) {
            let left = until
                .checked_duration_since(Instant::now())
                .ok_or_else(|| format!("{what} did not happen, after {told:?}"))?;
            let Ok(heard) = self.heard.recv_timeout(left) else {
                continue;
            };
            if let Heard::Drawn(Drawn::Said(part, said)) = &heard {
                return Err(format!("{part} said {said}"));
            }
            let kind = Told::of(&heard);
            self.window.heard(heard);
            let _ = self.window.take_said();
            let _ = self.window.take_front();
            told.push((kind, self.window.take_redraw()));
        }
        Ok(told.into_iter().map(|(kind, _)| kind).collect())
    }

    /// Whether the first screen as seen shows `page` at [`PLACE`].
    fn seen(&self, page: &Frame) -> bool {
        self.window
            .scene()
            .is_some_and(|scene| seen_at_the_top(&scene, page))
    }

    /// Whether the card's place, at [`PLACE`], shows `page`.
    fn shows(&self, page: &Frame) -> bool {
        self.window
            .scene()
            .is_some_and(|scene| in_place(&scene, page, true))
    }

    /// Whether the card's place, wherever it is, shows `page`.
    fn shows_anywhere(&self, page: &Frame) -> bool {
        self.window
            .scene()
            .is_some_and(|scene| in_place(&scene, page, false))
    }
}

/// What the window was told, by kind, which is what is asserted on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Told {
    /// A part painted.
    Painted(Part),
    /// The drawing thread drew through a drawing.
    Through,
    /// Where the first screen left the card's place clear.
    Beneath,
    /// The card's page drawn for it.
    Pictured,
    /// A frame of the card, or anything else from Chrome's thread.
    Chrome,
    /// Anything else.
    Other,
}

impl Told {
    fn of(heard: &Heard) -> Self {
        match heard {
            Heard::Drawn(Drawn::Painted(part, _)) => Self::Painted(*part),
            Heard::Drawn(Drawn::Through(_)) => Self::Through,
            Heard::Drawn(Drawn::Beneath(_)) => Self::Beneath,
            Heard::Drawn(Drawn::Pictured { .. }) => Self::Pictured,
            Heard::Chrome(_) => Self::Chrome,
            _ => Self::Other,
        }
    }
}

/// Whether `scene` holds `page` as the card's picture — at [`PLACE`] when
/// `at_the_top`, for a card that is the conversation's one turn — and
/// shows it at every pixel there the first screen left clear: what the
/// first screen draws over the place — its rounded corners, words offered
/// over the box — stands over it, as it stood over the picture in the
/// card (`alo-renderer`'s `the_cards_page_beneath.rs` holds that the two
/// are the same).
fn in_place(scene: &ConversationScene<'_>, page: &Frame, at_the_top: bool) -> bool {
    let (Some(start), Some(seen), Some((picture, place))) =
        (scene.start, scene.first_screen_seen(), scene.beneath)
    else {
        return false;
    };
    // Each pixel of the place inside the frame: clear in the first
    // screen's frame and showing the page, or covered by what it drew.
    let on_the_frame = |x: u32, y: u32| {
        let across = u32::try_from(i64::from(place.left) + i64::from(x)).ok()?;
        let down = u32::try_from(i64::from(place.top) + i64::from(y)).ok()?;
        Some((start.at(across, down)?, seen.at(across, down)?))
    };
    (place == PLACE || !at_the_top)
        && picture == page
        && (0..page.height).all(|y| {
            (0..page.width).all(|x| {
                on_the_frame(x, y)
                    .is_none_or(|(drawn, shown)| drawn.3 != 0 || Some(shown) == page.at(x, y))
            })
        })
}

/// How far into the page its bottom corners are rounded: the card's 14
/// less its border of 1.
const ROUNDED: u32 = 13;

/// Whether the first screen as a person sees it shows `page` at
/// [`PLACE`], every pixel of it but the two bottom corners the card's
/// rounding draws over — however it came to be drawn there.
fn seen_at_the_top(scene: &ConversationScene<'_>, page: &Frame) -> bool {
    let Some(seen) = scene.first_screen_seen() else {
        return false;
    };
    let (left, top) = (
        u32::try_from(PLACE.left).unwrap_or(0),
        u32::try_from(PLACE.top).unwrap_or(0),
    );
    let rounded =
        |x: u32, y: u32| y >= page.height - ROUNDED && (x < ROUNDED || x >= page.width - ROUNDED);
    (0..page.height).all(|y| {
        (0..page.width)
            .filter(|&x| !rounded(x, y))
            .all(|x| seen.at(x + left, y + top) == page.at(x, y))
    })
}

/// The window at one, both parts painted, the box's renderer and the
/// card's page's real ones.
fn the_window() -> Result<Driven, String> {
    let renderers = Renderers::running(renderer()?, &[]).with_machine(alo_window::fonts::frozen());
    let (tell, heard) = channel();
    let drawn = tell.clone();
    let drawing = ConversationDrawing::start(
        renderers,
        Box::new(move |d| drawn.send(Heard::Drawn(d)).is_ok()),
    )
    .map_err(|why| format!("no drawing thread: {why}"))?;
    let chrome = ChromeOpener::start(
        Chromium::Carried(PathBuf::from("/nonexistent/alo Chromium")),
        Box::new(move |w| tell.send(Heard::Chrome(w)).is_ok()),
    )
    .map_err(|why| format!("no Chrome thread: {why}"))?;
    // The test's clock, which stands still: what the first screen draws
    // by the time — the conversation's bar fading — is the same whenever
    // it is asked.
    let now = Instant::now();
    let mut window =
        ConversationWindow::new(Some(Scale::ONE), drawing, chrome).with_clock(move || now);
    window.measured_as(Measured {
        size: WINDOW,
        scale_factor: 1.0,
        outer: Some((100, 100)),
    });
    window.focused(true);
    window.ask_drawn();
    let mut driven = Driven { window, heard };
    driven.until("both parts painted", |driven| {
        driven
            .window
            .scene()
            .is_some_and(|scene| scene.panel.is_some() && scene.start.is_some())
    })?;
    Ok(driven)
}

/// Step 1: a card opened, its first frame seen.
fn opened(driven: &mut Driven, first: &Frame) -> Result<(), String> {
    let url = alo_url::parse("http://127.0.0.1:8765/card.html").map_err(|why| why.to_string())?;
    let png = frame_to_png(first).map_err(|why| why.to_string())?;
    driven.window.heard(Heard::Chrome(Went::Opening(url)));
    driven.window.heard(Heard::Chrome(Went::Framed(png)));
    driven.until("the card's first frame seen", |driven| driven.seen(first))?;
    Ok(())
}

/// After step 2: the page's place left clear in the first screen's frame,
/// and the newest frame drawn beneath it, at the place's size.
fn left_clear(driven: &Driven, newest: &Frame) -> Result<(), String> {
    assert!(driven.shows(newest), "the newest frame beneath the place");
    let scene = driven.window.scene().ok_or("no scene")?;
    let (picture, place) = scene.beneath.ok_or("the card's page is not beneath")?;
    assert_eq!(place, PLACE, "the page box, left clear");
    assert_eq!((picture.width, picture.height), PAGE);
    // Clear in the first screen's own frame: its corner and its middle.
    let start = scene.start.ok_or("no first screen")?;
    let alpha = |x: u32, y: u32| start.at(x, y).map(|pixel| pixel.3);
    assert_eq!(alpha(81, 123), Some(0), "the place's corner");
    assert_eq!(alpha(81 + 439, 123 + 260), Some(0), "its middle");
    assert_eq!(alpha(80, 300), Some(255), "the card's border beside it");
    assert_eq!(alpha(300, 122), Some(255), "the card's bar above it");
    Ok(())
}

/// Step 2: three more frames, each seen, none drawing the first screen.
fn streamed(driven: &mut Driven) -> Result<(), String> {
    for seed in 2..5 {
        let page = frame(seed);
        let png = frame_to_png(&page).map_err(|why| why.to_string())?;
        driven.window.heard(Heard::Chrome(Went::Framed(png)));
        assert!(
            !driven.window.take_redraw(),
            "a frame changes nothing drawn until its picture is"
        );
        let told = driven.until("the next frame seen", |driven| driven.seen(&page))?;
        assert!(
            !told
                .iter()
                .any(|told| matches!(told, Told::Painted(Part::Start) | Told::Through)),
            "a frame of the card drew the first screen again: {told:?}"
        );
        assert_eq!(told, [Told::Pictured], "only its picture");
    }
    Ok(())
}

/// Step 3: a letter typed between two frames, drawn in the first frame of
/// the first screen painted after it.
fn typed_between_frames(driven: &mut Driven) -> Result<(), String> {
    driven.window.moved(THE_BOX);
    driven.window.main_button_pressed();
    driven.window.main_button_released();
    driven.until("the box given the keyboard", |driven| {
        driven.window.conversation().field_focused() == Some(Field::TheBox)
    })?;
    let before = driven.window.scene().and_then(|scene| scene.start.cloned());
    let page = frame(5);
    driven.window.heard(Heard::Chrome(Went::Framed(
        frame_to_png(&page).map_err(|why| why.to_string())?,
    )));
    driven.window.key(&Press::character("k"), Some("k"));
    let later = frame(6);
    driven.window.heard(Heard::Chrome(Went::Framed(
        frame_to_png(&later).map_err(|why| why.to_string())?,
    )));
    let told = driven.until("the letter and the later frame seen", |driven| {
        driven.shows(&later)
    })?;
    let painted: Vec<&Told> = told
        .iter()
        .filter(|told| matches!(told, Told::Painted(Part::Start)))
        .collect();
    assert_eq!(painted.len(), 1, "the edit's frame alone: {told:?}");
    let field = driven.window.conversation().field(Field::TheBox);
    assert_eq!(field.text(), "k");
    let after = driven.window.scene().and_then(|scene| scene.start.cloned());
    assert!(before != after, "the first screen painted with the letter");
    Ok(())
}

/// Step 4: another card, none of the first card's pictures seen in it.
fn another_card(driven: &mut Driven, old: &Frame) -> Result<(), String> {
    let url = alo_url::parse("http://127.0.0.1:8765/other.html").map_err(|why| why.to_string())?;
    let page = frame(9);
    driven.window.heard(Heard::Chrome(Went::Opening(url)));
    let held = |driven: &Driven| {
        driven
            .window
            .scene()
            .and_then(|scene| scene.beneath)
            .is_some_and(|(picture, _)| picture == old)
    };
    assert!(
        !held(driven),
        "the first card's picture kept as the other opens"
    );
    driven.window.heard(Heard::Chrome(Went::Framed(
        frame_to_png(&page).map_err(|why| why.to_string())?,
    )));
    let until = Instant::now() + AT_MOST;
    while !(driven.shows_anywhere(&page) && driven.window.drawn_as_asked()) {
        let left = until
            .checked_duration_since(Instant::now())
            .ok_or("the other card's page was not seen")?;
        let Ok(heard) = driven.heard.recv_timeout(left) else {
            continue;
        };
        driven.window.heard(heard);
        let _ = driven.window.take_said();
        assert!(
            !held(driven),
            "the first card's picture held for the other card's place"
        );
    }
    Ok(())
}

#[test]
fn a_cards_frames_never_draw_the_first_screen_and_an_edit_is_drawn_in_its_next_frame() {
    let mut driven = the_window().unwrap_or_else(|why| panic!("{why}"));
    let first = frame(1);
    opened(&mut driven, &first).unwrap_or_else(|why| panic!("{why}"));
    streamed(&mut driven).unwrap_or_else(|why| panic!("{why}"));
    left_clear(&driven, &frame(4)).unwrap_or_else(|why| panic!("{why}"));
    typed_between_frames(&mut driven).unwrap_or_else(|why| panic!("{why}"));
    let last = frame(6);
    another_card(&mut driven, &last).unwrap_or_else(|why| panic!("{why}"));
    driven.window.finish();
    assert_eq!(driven.window.unfinished(), None, "every thread finished");
}
