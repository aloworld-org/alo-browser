/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue items 861 and 848 through **the window's own road**: the functions
//! `alo --conversation`'s event loop calls for each event
//! ([`ConversationWindow`]) and the two confined renderers that draw the
//! panel and the first screen, every thread's news given back as the event
//! loop gives it (`docs/design/shell.md`, *The one box as a text field*).
//!
//! The clipboard is the test's own — a reader handing over what a person
//! copied elsewhere, a writer keeping what is copied — so the person's is
//! never touched; the time is the test's own clock, so the caret's blink is
//! asked at instants, never slept for. The card's Chromium is one that is
//! not there.
//!
//! Each asserted, in order:
//!
//! 1. A press on the box gives it the keyboard; typing goes in, ← ← moves
//!    the caret back, and a letter goes in there; the caret is drawn where
//!    it stands, found as the column its blink takes away half a
//!    second later.
//! 2. ⌘→ goes to the end, ⇧⌥← selects the word, ⌘X cuts it onto the clipboard, ⌘V pastes what
//!    was copied elsewhere — an address with its newline, as one line.
//! 3. ⌘Z undoes the paste and then the cut; ⇧⌘Z does the paste again.
//! 4. A press at the field's start puts the caret there, a second quick one
//!    selects the word, and a press dragged to past the end selects to it.
//! 5. ⌘A, then Return, goes to the address: chosen, and the box emptied.
//! 6. Input methods are allowed while the box has the keyboard, and a
//!    composition is drawn in it and committed into it.
//!
//! And apart: the box has the keyboard as the window opens, its caret
//! drawn and typing reaching it with no press, and again after ⌘T makes a
//! new chat (queue item 875); a press with ⇧ grows its selection, and a
//! press with ⇧ on anything but a field is refused and said (863).

#![cfg(target_os = "macos")]

use alo_renderer::Frame;
use alo_renderer::host::Renderers;
use alo_renderer::typed::Offer;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::conversation::Field;
use alo_window::conversation_chrome::ChromeOpener;
use alo_window::conversation_drawing::{ConversationDrawing, Drawn};
use alo_window::conversation_window::{ConversationWindow, Heard, Measured};
use alo_window::keys::{Arrow, Held, Key, Modifier, Press};
use alo_window::scale::Scale;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long anything is waited for.
const AT_MOST: Duration = Duration::from_secs(60);

/// The design's window at one, in device pixels: the panel 240 wide, the
/// first screen beside it.
const WINDOW: (u32, u32) = (1280, 820);

/// What was copied elsewhere: an address, with the newline a line copied
/// from a terminal carries.
const COPIED: &str = "https://en.wikipedia.org/wiki/Kampala\n";

/// The box's field's left in the window: the panel's 240 and the field's
/// 183 into the first screen.
const FIELD_LEFT: f64 = 423.0;

/// The middle of the box's field, down the window.
const FIELD_MIDDLE: f64 = 286.0;

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

/// The conversation's window, its threads, what was heard of them, the
/// test's clock and what was copied.
struct Driven {
    window: ConversationWindow,
    heard: Receiver<Heard>,
    said: Vec<String>,
    now: Rc<Cell<Instant>>,
    copied: Rc<RefCell<Vec<String>>>,
}

impl Driven {
    /// Give the window what its threads told it, as its event loop does,
    /// until `done` holds of it and what it last asked is painted.
    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) -> Result<(), String> {
        let until = Instant::now() + AT_MOST;
        while !(done(self) && self.window.drawn_as_asked()) {
            let left = until
                .checked_duration_since(Instant::now())
                .ok_or_else(|| format!("{what} did not happen; the window said {:?}", self.said))?;
            let Ok(heard) = self.heard.recv_timeout(left) else {
                continue;
            };
            if let Heard::Drawn(Drawn::Said(part, said)) = &heard {
                return Err(format!("{part} said {said}"));
            }
            self.window.heard(heard);
            self.said.extend(self.window.take_said());
            let _ = self.window.take_redraw();
            let _ = self.window.take_front();
        }
        Ok(())
    }

    /// Move the test's clock on by `millis`, and tell the window the time.
    fn later(&mut self, millis: u64) {
        self.now.set(self.now.get() + Duration::from_millis(millis));
        self.window.tick(self.now.get());
    }

    /// A press of the main button at `at`, in the window's device pixels,
    /// at the test's time, and its release.
    fn click(&mut self, at: (f64, f64)) {
        self.window.moved(at);
        self.window.main_button_pressed_at(self.now.get());
        self.window.main_button_released();
    }

    /// `key` with `held`, writing `written`.
    fn press(&mut self, key: Key, held: &[Modifier], written: Option<&str>) {
        let press = held
            .iter()
            .fold(Press::of(key), |press, modifier| press.with(*modifier));
        self.window.key(&press, written);
    }

    /// ⌘ and `letter`, with ⇧ too when `shift`.
    fn command(&mut self, letter: &str, shift: bool) {
        let mut press = Press::character(letter).with(Modifier::Command);
        if shift {
            press = press.with(Modifier::Shift);
        }
        self.window.key(&press, Some(letter));
    }

    fn type_in(&mut self, text: &str) {
        for letter in text.chars() {
            let written = letter.to_string();
            self.window.key(&Press::character(&written), Some(&written));
        }
    }

    /// What the box holds, its caret and its selection.
    fn the_box(&self) -> (String, usize, Option<(usize, usize)>) {
        let field = self.window.conversation().field(Field::TheBox);
        (field.text().to_owned(), field.caret(), field.selection())
    }

    /// The first screen as last painted.
    fn painted(&self) -> Option<Frame> {
        self.window.scene().and_then(|scene| scene.start.cloned())
    }
}

/// The colour of the pixel at `x`, `y` of `frame`.
fn pixel(frame: &Frame, x: u32, y: u32) -> Option<[u8; 4]> {
    let at = usize::try_from((y * frame.width + x) * 4).ok()?;
    frame.pixels.get(at..at + 4)?.try_into().ok()
}

/// The columns of the box's field, in the first screen's pixels, where
/// `one` and `other` differ on its middle row.
fn columns_differing(one: &Frame, other: &Frame) -> Vec<u32> {
    (183..815)
        .filter(|&x| pixel(one, x, 286) != pixel(other, x, 286))
        .collect()
}

/// The conversation's window at one, both parts painted, its clipboard the
/// test's and its clock too.
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
    let now = Rc::new(Cell::new(Instant::now()));
    let clock = Rc::clone(&now);
    let mut window =
        ConversationWindow::new(Some(Scale::ONE), drawing, chrome).with_clock(move || clock.get());
    let copied = Rc::new(RefCell::new(Vec::new()));
    let kept = Rc::clone(&copied);
    window.copy_with(move |text| {
        kept.borrow_mut().push(text.to_owned());
        Ok(())
    });
    window.paste_with(|| Ok(COPIED.to_owned()));
    window.measured_as(Measured {
        size: WINDOW,
        scale_factor: 1.0,
        outer: Some((100, 100)),
    });
    window.focused(true);
    window.ask_drawn();
    let mut driven = Driven {
        window,
        heard,
        said: Vec::new(),
        now,
        copied,
    };
    driven.until("both parts painted", |driven| {
        driven
            .window
            .scene()
            .is_some_and(|scene| scene.panel.is_some() && scene.start.is_some())
    })?;
    Ok(driven)
}

/// Step 1: the box pressed, typed into, the caret moved back, a letter put
/// in, and the caret drawn where it stands.
fn typed_moved_and_its_caret_drawn(driven: &mut Driven) -> Result<(), String> {
    // 1. The box pressed, typed into, the caret moved back, a letter put in.
    driven.click((640.0, FIELD_MIDDLE));
    driven.until("the box given the keyboard", |driven| {
        driven.window.conversation().field_focused() == Some(Field::TheBox)
            && driven.window.presses_decided()
    })?;
    driven.type_in("kampla");
    driven.press(Key::Arrow(Arrow::Left), &[], None);
    driven.press(Key::Arrow(Arrow::Left), &[], None);
    driven.type_in("a");
    assert_eq!(driven.the_box(), ("kampala".to_owned(), 5, None));
    driven.until("kampala painted with its caret", |driven| {
        driven.painted().is_some()
    })?;
    let shown = driven.painted().ok_or("nothing painted")?;
    // Half a second on, the caret's hidden half: the one thing that
    // changes is the caret, a column wide at one, after `kampa`.
    driven.later(500);
    let hidden_at = driven.now.get();
    driven.until("the caret hidden", |driven| {
        driven
            .painted()
            .is_some_and(|frame| frame.pixels != shown.pixels)
    })?;
    let hidden = driven.painted().ok_or("nothing painted")?;
    let caret = columns_differing(&shown, &hidden);
    assert_eq!(caret.len(), 1, "one column: {caret:?}");
    let ink = Some([0x10, 0x2a, 0x43, 0xff]);
    assert!(caret.iter().all(|&x| pixel(&shown, x, 286) == ink));
    // After `kampa` in Inter at 16, 183 in: past the `p`, before `la`.
    let at = caret.first().copied().unwrap_or_default();
    assert!((230..250).contains(&at), "the caret at {at}");
    // And shown again at a second.
    assert_eq!(
        driven.window.wake_at(),
        Some(hidden_at + Duration::from_millis(500))
    );
    driven.later(500);
    driven.until("the caret shown again", |driven| {
        driven
            .painted()
            .is_some_and(|frame| frame.pixels == shown.pixels)
    })?;

    Ok(())
}

/// Steps 2 and 3: a word selected, cut and pasted, then undone and done
/// again.
fn selected_cut_pasted_and_undone(driven: &mut Driven) -> Result<(), String> {
    // 2. ⌘→ to the end, ⇧⌥← selects the word; ⌘X cuts it; ⌘V pastes
    //    what was copied.
    driven.press(Key::Arrow(Arrow::Right), &[Modifier::Command], None);
    assert_eq!(driven.the_box().1, 7);
    driven.press(
        Key::Arrow(Arrow::Left),
        &[Modifier::Shift, Modifier::Option],
        None,
    );
    assert_eq!(driven.the_box(), ("kampala".to_owned(), 0, Some((0, 7))));
    driven.command("x", false);
    assert_eq!(driven.copied.borrow().as_slice(), ["kampala"]);
    assert_eq!(driven.the_box(), (String::new(), 0, None));
    driven.command("v", false);
    let address = COPIED.trim_end();
    assert_eq!(driven.the_box(), (address.to_owned(), 37, None));
    driven.until("the address painted", |driven| driven.painted().is_some())?;

    // 3. ⌘Z the paste, ⌘Z the cut — `kampala` back, selected as it was —
    //    then ⇧⌘Z ⇧⌘Z the cut and the paste again.
    driven.command("z", false);
    assert_eq!(driven.the_box(), (String::new(), 0, None));
    driven.command("z", false);
    assert_eq!(driven.the_box(), ("kampala".to_owned(), 0, Some((0, 7))));
    driven.command("z", true);
    driven.command("z", true);
    assert_eq!(driven.the_box(), (address.to_owned(), 37, None));
    driven.until("the address painted again", |driven| {
        driven.painted().is_some()
    })?;

    Ok(())
}

/// Steps 4 and 5: presses placing the caret and selecting, a drag, and
/// Return going to the address.
fn pressed_dragged_and_sent(driven: &mut Driven) -> Result<(), String> {
    // 4. A press at the field's start puts the caret there; a second quick
    //    one selects the word; a press dragged past the end selects to it.
    driven.click((FIELD_LEFT + 1.0, FIELD_MIDDLE));
    driven.until("the caret placed at the start", |driven| {
        driven.the_box().1 == 0 && driven.the_box().2.is_none()
    })?;
    driven.later(100);
    driven.click((FIELD_LEFT + 1.0, FIELD_MIDDLE));
    driven.until("the first word selected", |driven| {
        driven.the_box().2 == Some((0, 5))
    })?;
    driven.later(800);
    driven.window.moved((FIELD_LEFT + 1.0, FIELD_MIDDLE));
    driven.window.main_button_pressed_at(driven.now.get());
    driven.until("a press placed", |driven| driven.the_box().2.is_none())?;
    driven.window.moved((FIELD_LEFT + 600.0, FIELD_MIDDLE));
    driven.window.main_button_released();
    assert_eq!(driven.the_box().2, Some((0, 37)), "dragged to the end");

    // 5. ⌘A, then Return goes to the address, and the box is emptied.
    driven.command("a", false);
    assert_eq!(driven.the_box().2, Some((0, 37)));
    driven.press(Key::Return, &[], Some("\r"));
    let went = match driven.window.conversation().chosen() {
        Some(Offer::Go(url)) => url.to_string(),
        other => return Err(format!("not gone: {other:?}")),
    };
    assert_eq!(went, "https://en.wikipedia.org/wiki/Kampala");
    assert_eq!(driven.the_box(), (String::new(), 0, None));

    Ok(())
}

/// Step 6: an input method allowed, composing in the box and committing
/// into it.
fn composed(driven: &mut Driven) -> Result<(), String> {
    // 6. Input methods are allowed while the box has the keyboard, the
    //    candidates under its caret; a composition is drawn, then put in.
    let asked = driven.window.take_ime();
    assert_eq!(asked.allowed, Some(true));
    driven.window.composing("nihao", Some((5, 5)));
    assert_eq!(
        driven
            .window
            .conversation()
            .field(Field::TheBox)
            .composing()
            .map(|composing| composing.text.clone()),
        Some("nihao".to_owned())
    );
    driven.until("the composition painted", |driven| {
        driven.painted().is_some()
    })?;
    // The address a turn now, the box stands at the window's foot: 880
    // wide in the first screen's 1040, its field 343 in and 748 down; the
    // caret after `nihao`, a pixel wide and the field's 40 tall.
    let candidates = driven.window.take_ime().caret;
    assert!(
        candidates.is_some_and(|(left, top, width, height)| (343.0 + 30.0..343.0 + 60.0)
            .contains(&left)
            && (top - 748.0).abs() < f64::EPSILON
            && (width - 1.0).abs() < f64::EPSILON
            && (height - 40.0).abs() < f64::EPSILON),
        "{candidates:?}"
    );
    driven.window.committed("你好");
    assert_eq!(driven.the_box(), ("你好".to_owned(), 2, None));
    Ok(())
}

#[test]
fn the_box_is_typed_moved_selected_cut_pasted_undone_pressed_and_sent() -> Result<(), String> {
    let mut driven = the_window()?;
    typed_moved_and_its_caret_drawn(&mut driven)?;
    selected_cut_pasted_and_undone(&mut driven)?;
    pressed_dragged_and_sent(&mut driven)?;
    composed(&mut driven)?;
    driven.window.finish();
    Ok(())
}

#[test]
fn search_chats_takes_a_paste_its_keys_and_undo_as_the_box_does() -> Result<(), String> {
    let mut driven = the_window()?;
    // *Search chats*: its field 111 down the panel and 46 in, 20 tall.
    driven.click((120.0, 121.0));
    driven.until("Search chats given the keyboard", |driven| {
        driven.window.conversation().field_focused() == Some(Field::SearchChats)
    })?;
    driven.command("v", false);
    let searched = |driven: &Driven| {
        let field = driven.window.conversation().field(Field::SearchChats);
        (field.text().to_owned(), field.caret(), field.selection())
    };
    let address = COPIED.trim_end();
    assert_eq!(searched(&driven), (address.to_owned(), 37, None));
    assert_eq!(driven.the_box(), (String::new(), 0, None), "not the box's");
    // ⌥⌫ takes the last word; Escape empties it; ⌘Z brings it back.
    driven.press(Key::Backspace, &[Modifier::Option], None);
    assert_eq!(searched(&driven).0, "https://en.wikipedia.org/wiki/");
    driven.press(Key::Escape, &[], Some("\u{1b}"));
    assert_eq!(searched(&driven).0, "");
    driven.command("z", false);
    assert_eq!(searched(&driven).0, "https://en.wikipedia.org/wiki/");
    // Thirty characters do not fit its 174: scrolled to keep the caret at
    // their end in view, a press at its left edge puts the caret at the
    // first character shown there, not at the words' start.
    driven.until("Search chats painted", |driven| {
        driven
            .window
            .scene()
            .is_some_and(|scene| scene.panel.is_some())
    })?;
    driven.click((47.0, 121.0));
    driven.until("the caret at the first character shown", |driven| {
        (1..30).contains(&searched(driven).1)
    })?;
    // ⌘← back to the start: scrolled back, the same press is the start.
    driven.press(Key::Arrow(Arrow::Left), &[Modifier::Command], None);
    driven.until("Search chats scrolled back", |driven| {
        searched(driven).1 == 0
    })?;
    driven.press(Key::Arrow(Arrow::Right), &[], None);
    // A second later, so it is a first press again, not the second of two.
    driven.later(1_000);
    driven.click((47.0, 121.0));
    driven.until("the caret at the start", |driven| searched(driven).1 == 0)?;
    driven.window.finish();
    Ok(())
}

#[test]
fn the_box_has_the_keyboard_as_the_window_opens_and_on_a_new_chat() -> Result<(), String> {
    let mut driven = the_window()?;
    // Queue item 875: nothing pressed, the box has the keyboard.
    assert_eq!(
        driven.window.conversation().field_focused(),
        Some(Field::TheBox)
    );
    driven.type_in("kampala");
    assert_eq!(driven.the_box(), ("kampala".to_owned(), 7, None));
    driven.until("kampala painted", |driven| driven.painted().is_some())?;
    let shown = driven.painted().ok_or("nothing painted")?;
    // Its caret drawn and blinking: the one column its hidden half takes
    // away, after `kampala`.
    driven.later(500);
    driven.until("the caret hidden", |driven| {
        driven
            .painted()
            .is_some_and(|frame| frame.pixels != shown.pixels)
    })?;
    let hidden = driven.painted().ok_or("nothing painted")?;
    let caret = columns_differing(&shown, &hidden);
    assert_eq!(caret.len(), 1, "one column: {caret:?}");
    let at = caret.first().copied().unwrap_or_default();
    assert!((240..260).contains(&at), "the caret after kampala at {at}");
    // *Search chats* pressed takes the keyboard; ⌘T, *New chat*'s key,
    // makes a chat whose box has it again.
    driven.click((120.0, 121.0));
    driven.until("Search chats given the keyboard", |driven| {
        driven.window.conversation().field_focused() == Some(Field::SearchChats)
    })?;
    let chats = driven.window.conversation().showing().chats.len();
    let before = driven.window.conversation().on_screen();
    driven.command("t", false);
    assert_eq!(
        driven.window.conversation().showing().chats.len(),
        chats + 1
    );
    assert_ne!(driven.window.conversation().on_screen(), before);
    assert_eq!(
        driven.window.conversation().field_focused(),
        Some(Field::TheBox)
    );
    driven.type_in("x");
    assert_eq!(driven.the_box().0, "kampalax", "typed with no press");
    // ⌘T held down repeating is one chat, not one a repeat.
    driven.window.key(
        &Press::character("t").with(Modifier::Command).repeating(),
        Some("t"),
    );
    assert_eq!(
        driven.window.conversation().showing().chats.len(),
        chats + 1
    );
    driven.window.finish();
    Ok(())
}

#[test]
fn a_press_with_shift_grows_the_boxs_selection_and_is_refused_elsewhere() -> Result<(), String> {
    let mut driven = the_window()?;
    driven.type_in("open en.wikipedia.org now");
    driven.until("the words painted", |driven| driven.painted().is_some())?;
    // A press at the start puts the caret there; ⇧ with a press past the
    // end grows the selection to it.
    driven.click((FIELD_LEFT + 1.0, FIELD_MIDDLE));
    driven.until("the caret placed at the start", |driven| {
        driven.the_box().1 == 0 && driven.the_box().2.is_none()
    })?;
    driven.later(1_000);
    driven.window.held(Held::NOTHING.with(Modifier::Shift));
    driven.click((FIELD_LEFT + 600.0, FIELD_MIDDLE));
    driven.until("the selection grown to the end", |driven| {
        driven.the_box().2 == Some((0, 25))
    })?;
    // ⇧ with a press in the middle, quick after: grown, never a word
    // selected as a second press would.
    driven.later(100);
    driven.click((FIELD_LEFT + 60.0, FIELD_MIDDLE));
    driven.until("the selection's end moved into the words", |driven| {
        driven
            .the_box()
            .2
            .is_some_and(|(start, end)| start == 0 && (3..12).contains(&end))
    })?;
    assert_eq!(driven.window.told(), None, "a field's press says nothing");
    // ⇧ with a press on *New chat* is nobody's: refused and said, the box
    // and the chats as they were.
    let chats = driven.window.conversation().showing().chats.len();
    let kept = driven.the_box();
    driven.later(1_000);
    driven.click((120.0, 79.0));
    driven.until("the press refused", |driven| driven.window.told().is_some())?;
    assert!(
        driven.window.told().is_some_and(|told| told.contains("⇧")),
        "{:?}",
        driven.window.told()
    );
    assert_eq!(driven.window.conversation().showing().chats.len(), chats);
    assert_eq!(driven.the_box(), kept);
    // ⇧ let go, the same press is *New chat*.
    driven.window.held(Held::NOTHING);
    driven.later(1_000);
    driven.click((120.0, 79.0));
    driven.until("a new chat", |driven| {
        driven.window.conversation().showing().chats.len() == chats + 1
    })?;
    driven.window.finish();
    Ok(())
}

#[test]
fn macs_control_keys_move_kill_and_yank_across_both_fields() -> Result<(), String> {
    // Queue item 865: the text system's standard bindings, the kill buffer
    // the window's own and never the clipboard.
    let mut driven = the_window()?;
    let control = |driven: &mut Driven, letter: &str| {
        driven.window.key(
            &Press::character(letter).with(Modifier::Control),
            Some("\u{b}"),
        );
    };
    driven.type_in("en.wikipedia.org kampala");
    control(&mut driven, "a");
    assert_eq!(driven.the_box().1, 0, "⌃A to the start");
    control(&mut driven, "f");
    control(&mut driven, "f");
    assert_eq!(driven.the_box().1, 2, "⌃F ⌃F");
    control(&mut driven, "b");
    assert_eq!(driven.the_box().1, 1, "⌃B");
    control(&mut driven, "e");
    assert_eq!(driven.the_box().1, 24, "⌃E to the end");
    control(&mut driven, "t");
    assert_eq!(
        driven.the_box().0,
        "en.wikipedia.org kampaal",
        "⌃T at the end"
    );
    control(&mut driven, "h");
    control(&mut driven, "h");
    assert_eq!(driven.the_box().0, "en.wikipedia.org kampa", "⌃H ⌃H");
    // ⌃P to the start, ⌃D the letter after.
    control(&mut driven, "p");
    control(&mut driven, "d");
    assert_eq!(
        driven.the_box(),
        ("n.wikipedia.org kampa".to_owned(), 0, None)
    );
    // ⌃N to the end; ⌥← to the last word's start; ⌃K kills it.
    control(&mut driven, "n");
    driven.press(Key::Arrow(Arrow::Left), &[Modifier::Option], None);
    control(&mut driven, "k");
    assert_eq!(driven.the_box().0, "n.wikipedia.org ");
    assert!(driven.copied.borrow().is_empty(), "never the clipboard");
    // ⌃Y yanks it into *Search chats*, the other field.
    driven.click((120.0, 121.0));
    driven.until("Search chats given the keyboard", |driven| {
        driven.window.conversation().field_focused() == Some(Field::SearchChats)
            && driven.window.presses_decided()
    })?;
    control(&mut driven, "y");
    let searched = driven.window.conversation().field(Field::SearchChats);
    assert_eq!((searched.text(), searched.caret()), ("kampa", 5));
    // ⌘Z undoes the yank alone.
    driven.command("z", false);
    assert_eq!(
        driven
            .window
            .conversation()
            .field(Field::SearchChats)
            .text(),
        ""
    );
    driven.window.finish();
    Ok(())
}
