/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 854 (`docs/design/shell.md`, *Settings*; approved by the
//! owner 2026-11-07): a face chosen in *Settings* sets the whole interface
//! at once, with no restart, and is kept beside the chats — through **the
//! window's own road**: the functions `alo --conversation`'s event loop
//! calls ([`ConversationWindow`]) and the two confined renderers, given
//! their faces once as they start.
//!
//! Never a person's chats or settings: everything is kept in a directory
//! of this test's own under the temporary directory, as `alo --conversation
//! --chats DIR` keeps a hand run's, and opened the way the window opens it
//! (`alo-window`'s `window::conversation`: `settings_file::read` beside
//! the chats). The card's Chromium is one that is not there.
//!
//! Each asserted:
//!
//! 1. A first launch: the panel's *Settings* row pressed opens *Settings*
//!    over the conversation (queue item 886); *Geist* pressed sets both
//!    parts in it at once — both renderers paint again, neither started
//!    anew — and writes `settings.txt` beside the chats, `0600`; Escape
//!    closes it, still in Geist; its × closes it too; ⌘, opens it, and a
//!    press on the veil beside its panel closes it.
//! 2. A second launch on the same directory: set in Geist from its first
//!    frame, pixel for pixel the panel the first launch painted in it.
//! 3. A damaged file: Inter, said with why, the panel pixel for pixel the
//!    first launch's in Inter, and the file left as it was.
//!
//! And queue item 856, *Settings* by keys: Tab to its × and on to *Font*,
//! each drawn with its ring; → choosing the next face, kept as a press
//! keeps it; ⇧Tab back; Return pressing its ×.

#![cfg(target_os = "macos")]

use alo_renderer::Frame;
use alo_renderer::host::Renderers;
use alo_renderer::interface_face::InterfaceFace;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::chat_seal::Key as ChatKey;
use alo_window::chat_store::ChatStore;
use alo_window::conversation_chrome::ChromeOpener;
use alo_window::conversation_drawing::{ConversationDrawing, Drawn};
use alo_window::conversation_window::{ConversationWindow, Heard, Measured};
use alo_window::keys::{Arrow, Key, Modifier, Press};
use alo_window::scale::Scale;
use alo_window::settings_file;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long anything is waited for.
const AT_MOST: Duration = Duration::from_secs(60);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The panel's *Settings*, at one: its row 38 tall, 12 over the window's
/// foot.
const SETTINGS_ROW: (f64, f64) = (120.0, 789.0);

/// *Geist*, the second face, at one: its middle 688, 244 into the first
/// screen, which stands 240 in (queue item 886).
const GEIST: (f64, f64) = (928.0, 244.0);

/// *Settings*' ×, at one: 824, 52 into the first screen, 36 square.
const CLOSE: (f64, f64) = (1082.0, 70.0);

/// The veil beside its panel, at one: 80, 400 into the first screen.
const VEIL: (f64, f64) = (320.0, 400.0);

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

/// The conversation's window, its threads, and what was heard of them.
struct Driven {
    window: ConversationWindow,
    heard: Receiver<Heard>,
    /// Every line the window said on the terminal.
    said: Vec<String>,
    /// How many frames each part painted: the panel's, the first screen's.
    painted: (usize, usize),
}

impl Driven {
    /// Give the window what its threads told it, as its event loop does,
    /// until `done` holds of it and what it last asked is painted — never
    /// a frame of a drawing asked before, as the box's caret blinking
    /// (queue item 875) asks more of them.
    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) -> Result<(), String> {
        let until = Instant::now() + AT_MOST;
        while !(done(self) && self.window.drawn_as_asked()) {
            let left = until
                .checked_duration_since(Instant::now())
                .ok_or_else(|| format!("{what} did not happen; the window said {:?}", self.said))?;
            let Ok(heard) = self.heard.recv_timeout(left) else {
                continue;
            };
            match &heard {
                Heard::Drawn(Drawn::Said(part, said)) => {
                    return Err(format!("{part} said {said}"));
                }
                Heard::Drawn(Drawn::Painted(part, _)) => {
                    if *part == alo_window::conversation_drawing::Part::Panel {
                        self.painted.0 += 1;
                    } else {
                        self.painted.1 += 1;
                    }
                }
                _ => {}
            }
            self.window.heard(heard);
            self.said.extend(self.window.take_said());
            let _ = self.window.take_redraw();
            let _ = self.window.take_front();
        }
        Ok(())
    }

    /// A person's click at `at`, in the window's device pixels.
    fn click(&mut self, at: (f64, f64)) {
        self.window.moved(at);
        self.window.main_button_pressed();
        self.window.main_button_released();
        self.said.extend(self.window.take_said());
    }

    /// The panel as its renderer last painted it.
    fn panel(&self) -> Option<Frame> {
        self.window.scene().and_then(|scene| scene.panel.cloned())
    }

    /// The first screen as its renderer last painted it.
    fn start(&self) -> Option<Frame> {
        self.window.scene().and_then(|scene| scene.start.cloned())
    }

    /// Stop the window's threads, as closing it does.
    fn close(mut self) -> Result<Vec<String>, String> {
        self.window.finish();
        self.said.extend(self.window.take_said());
        match self.window.unfinished() {
            Some(why) => Err(why.to_owned()),
            None => Ok(self.said),
        }
    }
}

/// The key this test seals its chats under: never the keychain's.
fn test_key() -> ChatKey {
    ChatKey::of([0x2b; 32])
}

/// The conversation's window at one, both parts painted, keeping its chats
/// and its settings in `directory` as the window opens them.
fn the_window(directory: &Path) -> Result<Driven, String> {
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
    let (store, opened) = ChatStore::open(directory, test_key())?;
    let file = settings_file::file_in(directory);
    let mut window = ConversationWindow::new(Some(Scale::ONE), drawing, chrome)
        .with_settings(settings_file::read(&file), Some(file))
        .with_chats(store, opened);
    window.measured_as(Measured {
        size: WINDOW,
        scale_factor: 1.0,
        outer: Some((100, 100)),
    });
    window.focused(true);
    window.ask_drawn();
    let said = window.take_said();
    let mut driven = Driven {
        window,
        heard,
        said,
        painted: (0, 0),
    };
    driven.until("both parts painted", |driven| {
        driven.panel().is_some() && driven.start().is_some()
    })?;
    Ok(driven)
}

/// A directory of this test's own, never a person's.
fn scratch(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("alo-font-chosen-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    directory
}

/// The face the window holds the interface set in.
fn face(driven: &Driven) -> InterfaceFace {
    driven.window.conversation().settings().face()
}

/// Whether *Settings* is on screen.
fn open(driven: &Driven) -> bool {
    driven.window.conversation().settings().is_open()
}

#[test]
fn a_font_chosen_applies_at_once_is_kept_across_launches_and_a_damaged_file_is_inter()
-> Result<(), String> {
    let directory = scratch("kept");
    let file = settings_file::file_in(&directory);

    // 1. The first launch, in Inter: nothing kept, nothing said of it.
    let mut first = the_window(&directory)?;
    assert_eq!(face(&first), InterfaceFace::Inter);
    assert!(
        first.said.iter().all(|line| !line.contains("settings")),
        "{:?}",
        first.said
    );
    let inter_panel = first.panel().ok_or("no panel")?;
    let chat_start = first.start().ok_or("no first screen")?;
    first.click(SETTINGS_ROW);
    first.until("Settings on screen, painted", |driven| {
        open(driven) && driven.start().as_ref() != Some(&chat_start)
    })?;
    let settings_in_inter = first.start().ok_or("no first screen")?;
    let painted = first.painted;
    first.click(GEIST);
    first.until("both parts painted in Geist", |driven| {
        face(driven) == InterfaceFace::Geist
            && driven.painted.0 > painted.0
            && driven.painted.1 > painted.1
            && driven.start().as_ref() != Some(&settings_in_inter)
    })?;
    assert!(open(&first), "choosing keeps Settings open");
    assert!(
        first
            .said
            .iter()
            .any(|line| line == "alo is set in Geist, as chosen in Settings"),
        "{:?}",
        first.said
    );
    // Kept at once, beside the chats, this user's alone.
    assert_eq!(
        std::fs::read_to_string(&file).ok().as_deref(),
        Some(
            settings_file::text_of(
                InterfaceFace::Geist,
                alo_renderer::agent_limits::Limits::default()
            )
            .as_str()
        )
    );
    let mode = std::fs::metadata(&file).map(|meta| meta.permissions().mode() & 0o777);
    assert_eq!(mode.ok(), Some(0o600));
    // Escape: back to the chat, still in Geist; the panel no longer the
    // one painted in Inter.
    let painted = first.painted;
    first.window.key(&Press::of(Key::Escape), Some("\u{1b}"));
    first.until("Settings closed and both parts painted", |driven| {
        !open(driven) && driven.painted.0 > painted.0 && driven.painted.1 > painted.1
    })?;
    let geist_panel = first.panel().ok_or("no panel")?;
    assert_ne!(geist_panel, inter_panel, "the panel painted again in Geist");
    // Its × closes it too.
    first.click(SETTINGS_ROW);
    first.until("Settings open again", open)?;
    first.click(CLOSE);
    first.until("Settings closed by its ×", |driven| !open(driven))?;
    // ⌘, opens it; a press on the veil closes it (queue item 886).
    let comma = Press::character(",").with(Modifier::Command);
    first.window.key(&comma, Some(","));
    first.until("Settings opened by ⌘,", open)?;
    first.click(VEIL);
    first.until("Settings closed by the veil", |driven| !open(driven))?;
    first.close()?;

    // 2. The second launch: in Geist from its first frame.
    let mut second = the_window(&directory)?;
    assert_eq!(face(&second), InterfaceFace::Geist);
    second.until("the panel painted as before", |driven| {
        driven.panel().as_ref() == Some(&geist_panel)
    })?;
    second.close()?;

    // 3. A damaged file: Inter, said, and left as it was.
    std::fs::write(&file, "font Comic Sans\n").map_err(|why| why.to_string())?;
    let mut third = the_window(&directory)?;
    assert_eq!(face(&third), InterfaceFace::Inter);
    assert!(
        third.said.iter().any(|line| line
            == &format!(
                "the settings in {} do not read (it names a font alo does not carry: \
                 \"Comic Sans\"), so alo is set in Inter",
                file.display()
            )),
        "{:?}",
        third.said
    );
    third.until("the panel painted in Inter", |driven| {
        driven.panel().as_ref() == Some(&inter_panel)
    })?;
    third.close()?;
    assert_eq!(
        std::fs::read_to_string(&file).ok().as_deref(),
        Some("font Comic Sans\n")
    );
    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}

#[test]
fn settings_is_walked_by_keys_a_face_chosen_by_an_arrow_and_back_by_return() -> Result<(), String> {
    let directory = scratch("keys");
    let file = settings_file::file_in(&directory);
    let mut driven = the_window(&directory)?;
    let key = |driven: &mut Driven, press: Press| driven.window.key(&press, None);
    let keyed = |driven: &Driven| {
        driven
            .window
            .conversation()
            .start(
                alo_window::conversation_place::start_viewport(WINDOW, Scale::ONE),
                Scale::ONE,
            )
            .settings_keyed
    };
    driven.click(SETTINGS_ROW);
    driven.until("Settings open", open)?;
    let plain = driven.start().ok_or("no first screen")?;
    assert_eq!(keyed(&driven), None, "no ring until Tab");
    // Tab: its ×, ringed — the first screen painted again.
    key(&mut driven, Press::of(Key::Tab));
    assert_eq!(
        keyed(&driven),
        Some(alo_renderer::start::SettingsKeyed::Close)
    );
    driven.until("Close ringed", |driven| {
        driven.start().as_ref().is_some_and(|frame| *frame != plain)
    })?;
    // Tab: *Font*; → the face after Inter, Geist, set and kept at once.
    key(&mut driven, Press::of(Key::Tab));
    assert_eq!(
        keyed(&driven),
        Some(alo_renderer::start::SettingsKeyed::Font)
    );
    key(&mut driven, Press::of(Key::Arrow(Arrow::Right)));
    assert_eq!(face(&driven), InterfaceFace::Geist);
    assert_eq!(
        std::fs::read_to_string(&file).ok().as_deref(),
        Some(
            settings_file::text_of(
                InterfaceFace::Geist,
                alo_renderer::agent_limits::Limits::default()
            )
            .as_str()
        )
    );
    // ← ← round to Instrument Sans, the last.
    key(&mut driven, Press::of(Key::Arrow(Arrow::Left)));
    key(&mut driven, Press::of(Key::Arrow(Arrow::Left)));
    assert_eq!(face(&driven), InterfaceFace::InstrumentSans);
    // A key with ⌘ is none of these: nothing moved.
    key(&mut driven, Press::of(Key::Tab).with(Modifier::Command));
    assert_eq!(
        keyed(&driven),
        Some(alo_renderer::start::SettingsKeyed::Font)
    );
    // ⇧Tab back to its ×; Return presses it: the chat again.
    key(&mut driven, Press::of(Key::Tab).with(Modifier::Shift));
    assert_eq!(
        keyed(&driven),
        Some(alo_renderer::start::SettingsKeyed::Close)
    );
    driven.window.key(&Press::of(Key::Return), Some("\r"));
    assert!(!open(&driven), "Return pressed its ×");
    // Opened again by the pointer: no ring; a press puts a ring away.
    driven.click(SETTINGS_ROW);
    driven.until("Settings open again", open)?;
    assert_eq!(keyed(&driven), None);
    key(&mut driven, Press::of(Key::Tab));
    driven.click(GEIST);
    driven.until("Geist pressed", |driven| {
        face(driven) == InterfaceFace::Geist
    })?;
    assert_eq!(keyed(&driven), None, "the pointer put the ring away");
    driven.close()?;
    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}
