/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 701: going to a website from the one box, with no model (ADR
//! 0074, `docs/design/shell.md`, *What appears when a person types*).
//!
//! What closes it here, each a test below: a person's keys, read as the
//! conversation's window reads them, typing `example.com` into the box a
//! press gave the keyboard, and Return deciding to go to
//! `https://example.com/`; what is typed reaching the first screen's
//! renderer as data and nothing of it the panel's; and the first screen
//! with it drawn the same by the confined renderer the window's thread
//! holds as by this process. Opening the address in Chrome from this
//! window is queue item 713's, asserted in
//! `a_page_chosen_in_the_conversation_opened_in_chrome`.

use alo_renderer::host::Renderers;
use alo_renderer::typed::{Offer, offers};
use alo_renderer::{Frame, FromRenderer, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_window::beside::renderer_beside;
use alo_window::conversation::{Conversation, Pressed};
use alo_window::conversation_box::{Typed, said};
use alo_window::conversation_drawing::{Asked, ConversationDrawing, Drawn, Part};
use alo_window::conversation_place::{panel_viewport, start_viewport};
use alo_window::keys::{Key, Modifier, Press};
use alo_window::scale::Scale;
use alo_window::typing::edit;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

/// The design's window at one, in device pixels.
const AT_ONE: (u32, u32) = (1280, 820);

/// What the window asks to be drawn of `conversation` at one.
fn asked(conversation: &Conversation) -> Asked {
    Asked {
        panel: conversation.panel(panel_viewport(AT_ONE, Scale::ONE), Scale::ONE),
        start: conversation.start(start_viewport(AT_ONE, Scale::ONE), Scale::ONE),
        showing: conversation.showing(),
    }
}

/// Each character of `text` pressed as a key that writes it, as the window
/// reads one: what the last came to.
fn keys(conversation: &mut Conversation, text: &str) -> Typed {
    let mut last = Typed::Unchanged;
    for letter in text.chars() {
        let written = letter.to_string();
        if let Some(edit) = edit(&Press::character(&written), Some(&written)) {
            last = conversation.edit(edit);
        }
    }
    last
}

/// A key that writes nothing pressed, as the window reads one.
fn key(conversation: &mut Conversation, press: &Press) -> Typed {
    edit(press, None).map_or(Typed::Unchanged, |edit| conversation.edit(edit))
}

#[test]
fn typing_example_com_and_return_decides_to_go_to_https_example_com() {
    let mut conversation = Conversation::default();
    // Before the box has the keyboard, keys type nothing.
    assert_eq!(keys(&mut conversation, "nothing"), Typed::Unchanged);
    assert!(conversation.act(Pressed::TheBox));
    assert_eq!(keys(&mut conversation, "example.con"), Typed::Changed);
    assert_eq!(
        key(&mut conversation, &Press::of(Key::Backspace)),
        Typed::Changed
    );
    assert_eq!(keys(&mut conversation, "m"), Typed::Changed);
    // ⌘ held is the browser's or the system's, never a letter in the box.
    let held = Press::character("q").with(Modifier::Command);
    assert_eq!(edit(&held, Some("q")), None);
    let start = asked(&conversation).start;
    assert_eq!(start.typed, "example.com");
    assert_eq!(start.offers, offers("example.com"));

    let chosen = key(&mut conversation, &Press::of(Key::Return));
    let Typed::Chose(Offer::Go(url)) = &chosen else {
        panic!("Return did not choose to go: {chosen:?}");
    };
    assert_eq!(url.to_string(), "https://example.com/");
    assert_eq!(
        conversation.going().map(ToString::to_string).as_deref(),
        Some("https://example.com/")
    );
    assert_eq!(
        said(
            &Offer::Go(url.clone()),
            &alo_renderer::results::SetUp::Ready
        ),
        "going to https://example.com/"
    );
    // Escape empties the box, and the offers go with what was typed.
    assert_eq!(
        key(&mut conversation, &Press::of(Key::Escape)),
        Typed::Changed
    );
    assert!(asked(&conversation).start.offers.is_empty());
}

#[test]
fn what_is_typed_reaches_the_first_screen_and_nothing_of_it_the_panel() {
    let mut conversation = Conversation::default();
    let before = asked(&conversation);
    let _ = conversation.act(Pressed::TheBox);
    let _ = keys(&mut conversation, "</script><b>secret");
    let after = asked(&conversation);
    assert_eq!(after.panel, before.panel, "the panel is sent the same");
    assert_eq!(after.showing, before.showing);
    assert_eq!(after.start.typed, "</script><b>secret");
    assert!(
        !format!("{:?}", after.panel).contains("secret"),
        "{:?}",
        after.panel
    );
}

/// The corpus's faces, and what `system-ui`, `sans-serif` and `serif` mean.
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
    // The design's typefaces, as the browser process hands them to the
    // interface's renderers (ADR 0084).
    for font in alo_renderer::typefaces::fonts() {
        database.add(font);
    }
    database
}

/// How long the confined renderers may take to answer.
const AT_MOST: Duration = Duration::from_secs(60);

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

/// The first screen as the window's thread has its confined renderer paint
/// it for `conversation`, after both parts first painted as a new
/// conversation's.
fn painted_through_a_confined_renderer(conversation: &Conversation) -> Result<Frame, String> {
    let renderers = Renderers::running(renderer()?, &[]).with_machine(alo_window::fonts::frozen());
    let (tell, told) = channel();
    let mut drawing =
        ConversationDrawing::start(renderers, Box::new(move |drawn| tell.send(drawn).is_ok()))
            .map_err(|why| format!("no thread: {why}"))?;
    drawing.draw(asked(&Conversation::default()));
    let (mut panel, mut start) = (false, false);
    while !(panel && start) {
        match next(&told)? {
            Drawn::Painted(Part::Panel, _) => panel = true,
            Drawn::Painted(Part::Start, _) => start = true,
            other => return Err(format!("not a frame: {other:?}")),
        }
    }
    // Only the first screen differs, so only it is drawn again.
    drawing.draw(asked(conversation));
    let frame = match next(&told)? {
        Drawn::Painted(Part::Start, frame) => frame,
        other => return Err(format!("the first screen was not drawn again: {other:?}")),
    };
    if !drawing.finish() {
        return Err("the drawing thread did not finish".to_owned());
    }
    Ok(frame)
}

fn next(told: &Receiver<Drawn>) -> Result<Drawn, String> {
    told.recv_timeout(AT_MOST)
        .map_err(|why| format!("nothing was told: {why}"))
}

#[test]
fn the_first_screen_typed_into_is_drawn_the_same_by_its_confined_renderer() {
    let mut conversation = Conversation::default();
    let _ = conversation.act(Pressed::TheBox);
    let _ = keys(&mut conversation, "example.com");
    let frame =
        painted_through_a_confined_renderer(&conversation).unwrap_or_else(|why| panic!("{why}"));
    let mut here = Renderer::new(fonts());
    let built = here.handle(ToRenderer::Start(Box::new(asked(&conversation).start)));
    assert!(matches!(built, FromRenderer::Loaded { .. }), "{built:?}");
    let FromRenderer::Painted(painted_here) = here.handle(ToRenderer::Paint) else {
        panic!("not painted here");
    };
    assert_eq!((frame.width, frame.height), (1040, 820));
    assert!(
        frame == painted_here,
        "the confined renderer painted another first screen"
    );
}
