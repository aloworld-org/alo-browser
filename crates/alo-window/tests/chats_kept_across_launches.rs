/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue items 757 and 758 (ADR 0092): the conversation's chats are kept
//! across launches, each named by the first words said in it — through
//! **the window's own road**: the functions `alo --conversation`'s event
//! loop calls ([`ConversationWindow`]), the two confined renderers, and the
//! thread asking the search provider.
//!
//! Never the real API, the network or a person's chats: the provider is a
//! server on `127.0.0.1`, and the chats are kept in a directory of this
//! test's own under the temporary directory, as `alo --conversation --chats
//! DIR` keeps them. The card's Chromium is one that is not there.
//!
//! Each asserted:
//!
//! 1. A first launch: two searches in one chat, one in another made with
//!    *New chat*; each chat named by its first words in the panel, and
//!    written to a file of its own, its words and addresses and no picture.
//! 2. A second launch on the same directory: a new chat on screen, the two
//!    from before under *Earlier* by their names, newest first; a file
//!    that does not read said, with why, and left as it is.
//! 3. An earlier chat pressed in the panel is on screen again, its turns
//!    drawn — the newest's first mark where it was drawn before — and in
//!    use again, under *Open*.

#![cfg(target_os = "macos")]

use alo_renderer::Frame;
use alo_renderer::host::Renderers;
use alo_renderer::panel::{NEW_CHAT, drawn_order};
use alo_renderer::results::{Found, Showing};
use alo_renderer::turn::Turn;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::brave_search::Provider;
use alo_window::chat_record;
use alo_window::chat_seal::Key as ChatKey;
use alo_window::chat_store::{ChatStore, file_of};
use alo_window::conversation_chrome::ChromeOpener;
use alo_window::conversation_drawing::{ConversationDrawing, Drawn};
use alo_window::conversation_place::start_viewport;
use alo_window::conversation_search::Searcher;
use alo_window::conversation_thread::Thread;
use alo_window::conversation_window::{ConversationWindow, Heard, Measured};
use alo_window::keys::{Key, Press};
use alo_window::scale::Scale;
use alo_window::search_key::Key as SearchKey;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long anything is waited for.
const AT_MOST: Duration = Duration::from_secs(60);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The box, at one: 400 into the first screen.
const THE_BOX: (f64, f64) = (640.0, 285.0);

/// The panel's *New chat*, at one: its button at 10, 56, 220 by 40.
const NEW_CHAT_BUTTON: (f64, f64) = (120.0, 76.0);

/// The second row under *Earlier* with one chat open, at one: the open
/// row at 178 and the earlier ones from 254, 40 apart, each 38 tall.
const SECOND_EARLIER: (f64, f64) = (120.0, 313.0);

/// What the test's provider is given as a key: not a key.
const NOT_A_KEY: &str = "test-key-for-a-local-server";

/// The first search's answer.
const FIRST: &str = r#"{"type":"search","web":{"type":"search","results":[
    {"title":"References and Borrowing","url":"https://doc.example.org/book/references.html",
     "description":"The borrow checker checks every reference.","profile":{"name":"Book"}},
    {"title":"Fighting the borrow checker","url":"https://blog.example/fighting"}
  ]}}"#;

/// The second search's answer.
const SECOND: &str = r#"{"type":"search","web":{"type":"search","results":[
    {"title":"Lifetimes explained","url":"https://e.example/lifetimes",
     "description":"Every reference has a lifetime."},
    {"title":"Elision","url":"https://elision.example/"}
  ]}}"#;

/// A provider on this machine: its port.
fn provide() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            std::thread::spawn(move || {
                let _ = socket.set_read_timeout(Some(Duration::from_secs(5)));
                let mut head = Vec::new();
                let mut block = [0u8; 1024];
                while let Ok(got) = socket.read(&mut block) {
                    head.extend_from_slice(block.get(..got).unwrap_or_default());
                    if got == 0 || head.windows(4).any(|end| end == b"\r\n\r\n") {
                        break;
                    }
                }
                let head = String::from_utf8_lossy(&head).into_owned();
                let (status, body) = match head.split(' ').nth(1) {
                    Some("/res/v1/web/search?q=rust+borrow+checker") => ("200 OK", FIRST),
                    Some("/res/v1/web/search?q=lifetimes") => ("200 OK", SECOND),
                    _ => ("500 Internal Server Error", ""),
                };
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Ok(port)
}

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
}

impl Driven {
    /// Give the window what its threads told it, as its event loop does,
    /// until `done` holds of it.
    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) -> Result<(), String> {
        let until = Instant::now() + AT_MOST;
        while !done(self) {
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

    /// A person's click at `at`, in the window's device pixels.
    fn click(&mut self, at: (f64, f64)) {
        self.window.moved(at);
        self.window.main_button_pressed();
        self.window.main_button_released();
    }

    /// `text` typed, as the window reads keys, then Return.
    fn type_and_return(&mut self, text: &str) {
        for letter in text.chars() {
            let written = letter.to_string();
            self.window.key(&Press::character(&written), Some(&written));
        }
        self.window.key(&Press::of(Key::Return), None);
        self.said.extend(self.window.take_said());
    }

    /// The chat on screen's turns.
    fn thread(&self) -> Thread {
        self.window
            .conversation()
            .thread()
            .cloned()
            .unwrap_or_default()
    }

    /// The panel's rows in the order it draws them: each chat's label,
    /// whether it is open, and whether it is on screen.
    fn rows(&self) -> Vec<(String, bool, bool)> {
        let chats = self
            .window
            .conversation()
            .panel(alo_layout::Size::new(240.0, 820.0), Scale::ONE)
            .chats;
        drawn_order(&chats)
            .into_iter()
            .filter_map(|place| chats.get(place))
            .map(|chat| (chat.label().to_owned(), chat.open, chat.current))
            .collect()
    }

    /// The first screen as its renderer last painted it.
    fn painted(&self) -> Option<Frame> {
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

/// The colour of the pixel at `x`, `y` of `frame`.
fn pixel(frame: &Frame, x: u32, y: u32) -> Option<[u8; 4]> {
    let at = usize::try_from((y * frame.width + x) * 4).ok()?;
    frame.pixels.get(at..at + 4)?.try_into().ok()
}

/// The design's four tints a site's letter stands on, in order.
const TINTS: [[u8; 4]; 4] = [
    [0x2f, 0x6b, 0x5e, 0xff],
    [0x3e, 0x5c, 0x8a, 0xff],
    [0x5a, 0x6b, 0x8a, 0xff],
    [0x6b, 0x65, 0x5c, 0xff],
];

/// The tint the site at `address` is drawn on.
fn tint(address: &str) -> Option<[u8; 4]> {
    let url = alo_url::parse(address).ok()?;
    TINTS.get(Found::new("", url, "").tint()).copied()
}

/// The conversation's window at one, both parts painted, searching through
/// the provider on `port`, keeping its chats in `chats`.
fn the_window(port: u16, chats: &Path) -> Result<Driven, String> {
    let renderers = Renderers::running(renderer()?, &[]).with_machine(alo_window::fonts::frozen());
    let (tell, heard) = channel();
    let drawn = tell.clone();
    let told = tell.clone();
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
    let key = SearchKey::of(NOT_A_KEY).ok_or("the test's key is not one a header carries")?;
    let provider = Provider::at(&format!("http://127.0.0.1:{port}/res/v1/web/search"), key)?;
    let searcher = Searcher::start(
        Ok(provider),
        Box::new(move |s| told.send(Heard::Searched(s)).is_ok()),
    )
    .map_err(|why| format!("no search thread: {why}"))?;
    let (store, opened) = ChatStore::open(chats, test_key())?;
    let mut window = ConversationWindow::new(Some(Scale::ONE), drawing, chrome)
        .with_search(searcher)
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
    };
    driven.until("both parts painted", |driven| {
        driven
            .window
            .scene()
            .is_some_and(|scene| scene.panel.is_some() && scene.start.is_some())
    })?;
    Ok(driven)
}

/// Whether the newest turn is a search whose sites have come, and the chat
/// holds `turns`.
fn found(driven: &Driven, turns: usize) -> bool {
    driven.thread().turns().len() == turns
        && matches!(
            driven.thread().turns().last(),
            Some(Turn::Searched(results)) if matches!(results.showing, Showing::Found(_))
        )
}

/// Search `words` from the box, and wait for its sites in the chat's
/// `turns`th turn.
fn search(driven: &mut Driven, words: &str, turns: usize) -> Result<(), String> {
    driven.click(THE_BOX);
    driven.until("the box given the keyboard", |driven| {
        driven
            .window
            .conversation()
            .start(start_viewport(WINDOW, Scale::ONE), Scale::ONE)
            .focused
    })?;
    driven.type_and_return(words);
    driven.until(&format!("{words}'s sites"), |driven| found(driven, turns))
}

/// A directory of this test's own, never a person's.
fn scratch(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("alo-chats-kept-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    directory
}

/// Step 1: the first launch, two chats made and kept.
fn first_launch(port: u16, chats: &Path) -> Result<(), String> {
    let mut driven = the_window(port, chats)?;
    assert_eq!(driven.rows(), vec![(NEW_CHAT.to_owned(), true, true)]);
    search(&mut driven, "rust borrow checker", 1)?;
    search(&mut driven, "lifetimes", 2)?;
    assert_eq!(
        driven.rows(),
        vec![("rust borrow checker".to_owned(), true, true)],
        "named by its first words, not its newest"
    );
    driven.click(NEW_CHAT_BUTTON);
    driven.until("a new chat on screen", |driven| driven.rows().len() == 2)?;
    search(&mut driven, "lifetimes", 1)?;
    assert_eq!(
        driven.rows(),
        vec![
            ("lifetimes".to_owned(), true, true),
            ("rust borrow checker".to_owned(), true, false)
        ]
    );
    let said = driven.close()?;
    assert!(
        said.iter().all(|line| !line.contains("not kept")),
        "{said:?}"
    );
    // Each chat in a file of its own, sealed (ADR 0098): no word of it on
    // the disk in the clear.
    for number in [0, 1] {
        let bytes = std::fs::read(file_of(chats, number)).map_err(|why| why.to_string())?;
        for word in [
            &b"lifetimes"[..],
            b"borrow",
            b"doc.example.org",
            b"alo chat",
        ] {
            assert!(
                !bytes.windows(word.len()).any(|window| window == word),
                "chat {number} holds {:?} in the clear",
                String::from_utf8_lossy(word)
            );
        }
    }
    // Its words and addresses, opened, no picture.
    let older = kept_words(chats, 0)?;
    let newer = kept_words(chats, 1)?;
    for written in [&older, &newer] {
        assert!(
            !written.contains("picture") && !written.contains("\"mark\""),
            "{written}"
        );
    }
    let older = chat_record::read(older.as_bytes()).map_err(|why| why.0)?;
    assert_eq!(older.name, "rust borrow checker");
    assert_eq!(
        older.turns.iter().map(Turn::said).collect::<Vec<_>>(),
        vec!["rust borrow checker", "lifetimes"]
    );
    let Some(Turn::Searched(first)) = older.turns.first() else {
        return Err(format!("not a search: {:?}", older.turns.first()));
    };
    let Showing::Found(sites) = &first.showing else {
        return Err(format!("no sites: {:?}", first.showing));
    };
    assert_eq!(
        sites
            .iter()
            .map(|site| (site.title.as_str(), site.url.to_string()))
            .collect::<Vec<_>>(),
        vec![
            (
                "References and Borrowing",
                "https://doc.example.org/book/references.html".to_owned()
            ),
            (
                "Fighting the borrow checker",
                "https://blog.example/fighting".to_owned()
            )
        ]
    );
    let newer = chat_record::read(newer.as_bytes()).map_err(|why| why.0)?;
    assert_eq!(newer.name, "lifetimes");
    assert_eq!(newer.turns.len(), 1);
    Ok(())
}

#[test]
fn chats_are_kept_across_launches_named_by_their_first_words() -> Result<(), String> {
    let port = provide()?;
    let chats = scratch("kept");
    first_launch(port, &chats)?;

    // 2. A file that does not read, beside them: said, and left as it is.
    let damaged = file_of(&chats, 5);
    std::fs::write(&damaged, b"not a chat").map_err(|why| why.to_string())?;
    let mut driven = the_window(port, &chats)?;
    assert!(
        driven
            .said
            .iter()
            .any(|line| line.contains(&damaged.display().to_string())
                && line.contains("could not be read")
                && line.contains("not a sealed chat")),
        "{:?}",
        driven.said
    );
    assert_eq!(
        driven.rows(),
        vec![
            (NEW_CHAT.to_owned(), true, true),
            ("lifetimes".to_owned(), false, false),
            ("rust borrow checker".to_owned(), false, false)
        ],
        "a new chat, and the two from before under Earlier, newest first"
    );
    assert_eq!(
        driven
            .window
            .conversation()
            .on_screen()
            .map(alo_window::conversation::ChatId::number),
        Some(6),
        "numbered past the damaged file"
    );

    // 3. The older chat pressed: on screen, open, its two turns drawn from
    //    the newest's top — its first mark where a first mark is drawn.
    let second = tint("https://e.example/").ok_or("a tint")?;
    driven.click(SECOND_EARLIER);
    driven.until("the older chat drawn", |driven| {
        driven.thread().turns().len() == 2
            && driven
                .painted()
                .and_then(|frame| pixel(&frame, 81, 177))
                .is_some_and(|colour| colour == second)
    })?;
    // First under Open: in use again, the most recent (queue item 760).
    assert_eq!(
        driven.rows(),
        vec![
            ("rust borrow checker".to_owned(), true, true),
            (NEW_CHAT.to_owned(), true, false),
            ("lifetimes".to_owned(), false, false)
        ],
        "in use again, under Open"
    );
    assert_eq!(
        driven
            .thread()
            .turns()
            .iter()
            .map(Turn::said)
            .collect::<Vec<_>>(),
        vec!["rust borrow checker", "lifetimes"]
    );
    let _ = driven.close()?;
    assert_eq!(
        std::fs::read(&damaged).map_err(|why| why.to_string())?,
        b"not a chat",
        "untouched"
    );
    let _ = std::fs::remove_dir_all(&chats);
    Ok(())
}

/// The key this test's chats are sealed under: its own, never the
/// keychain's (ADR 0098).
fn test_key() -> ChatKey {
    ChatKey::of([0x2a; 32])
}

/// The words of the chat numbered `number` kept in `chats`, opened under
/// the test's key.
fn kept_words(chats: &std::path::Path, number: u64) -> Result<String, String> {
    ChatStore::open(chats, test_key())?.0.words(number)
}

/// The first chat under *Earlier*, a row above [`SECOND_EARLIER`].
const FIRST_EARLIER: (f64, f64) = (120.0, 273.0);

/// That row's *Delete*, at the row's end (queue item 789): 24 tall in the
/// middle of the row, its right edge at 220.
const ITS_DELETE: (f64, f64) = (200.0, 273.0);

/// The panel's *Undo*: in the dark pill 8 above *Settings*, whose row ends
/// 12 above the window's foot — the pill 726 to 762 — at its right end.
const UNDO: (f64, f64) = (205.0, 744.0);

/// The colour of a pixel of the panel as it was last painted.
fn panel_pixel(driven: &Driven, at: (u32, u32)) -> Option<[u8; 4]> {
    driven
        .window
        .scene()
        .and_then(|scene| scene.panel.cloned())
        .and_then(|frame| pixel(&frame, at.0, at.1))
}

/// Hover the first earlier row until it is painted offering *Delete* —
/// the button's ground, the palette's `--line-soft` — then press *Delete*.
fn delete_the_first_earlier(driven: &mut Driven) -> Result<(), String> {
    let left = driven.rows().len() - 1;
    // A panel painted before, still offering Delete, is not this one.
    driven.until("the panel painted with no Delete", |driven| {
        panel_pixel(driven, (215, 263)) != Some([0xe1, 0xda, 0xce, 0xff])
    })?;
    driven.window.moved(FIRST_EARLIER);
    driven.until("Delete painted on the hovered row", |driven| {
        panel_pixel(driven, (215, 263)) == Some([0xe1, 0xda, 0xce, 0xff])
    })?;
    driven.click(ITS_DELETE);
    driven.until("the chat deleted", |driven| driven.rows().len() == left)
}

#[test]
fn a_chat_deleted_goes_from_the_panel_at_once_and_its_file_when_its_undo_has_passed()
-> Result<(), String> {
    let port = provide()?;
    let chats = scratch("deleted");
    first_launch(port, &chats)?;
    let mut driven = the_window(port, &chats)?;
    let rows = |names: &[&str]| -> Vec<(String, bool, bool)> {
        names
            .iter()
            .enumerate()
            .map(|(at, name)| ((*name).to_owned(), at == 0, at == 0))
            .collect()
    };
    assert_eq!(
        driven.rows(),
        rows(&[NEW_CHAT, "lifetimes", "rust borrow checker"])
    );

    // 1. Deleted: gone from the panel at once, its file kept for its Undo,
    //    which the panel's foot offers.
    delete_the_first_earlier(&mut driven)?;
    assert_eq!(driven.rows(), rows(&[NEW_CHAT, "rust borrow checker"]));
    assert!(
        file_of(&chats, 1).exists(),
        "kept until its Undo has passed"
    );
    let deleted = driven
        .window
        .conversation()
        .panel(alo_layout::Size::new(240.0, 820.0), Scale::ONE)
        .deleted;
    assert_eq!(deleted.as_deref(), Some("lifetimes"));

    // 2. Undo: back where it stood, its turns as they were.
    driven.until("Undo painted", |driven| {
        panel_pixel(driven, (30, 730)) == Some([0x2b, 0x27, 0x22, 0xff])
    })?;
    driven.click(UNDO);
    driven.until("the chat brought back", |driven| driven.rows().len() == 3)?;
    assert_eq!(
        driven.rows(),
        rows(&[NEW_CHAT, "lifetimes", "rust borrow checker"])
    );
    assert!(driven.window.wake_at().is_none(), "nothing is held now");

    // 3. Deleted again, and its five seconds passed: its file removed, and
    //    no other.
    delete_the_first_earlier(&mut driven)?;
    let until = driven.window.wake_at().ok_or("woken as the Undo passes")?;
    let before = until
        .checked_sub(Duration::from_millis(1))
        .ok_or("a moment before the Undo passes")?;
    driven.window.tick(before);
    assert!(file_of(&chats, 1).exists(), "a moment before, still kept");
    driven.window.tick(until);
    driven.said.extend(driven.window.take_said());
    assert!(!file_of(&chats, 1).exists(), "removed once its Undo passed");
    assert!(file_of(&chats, 0).exists(), "the other chat's file is kept");
    assert_eq!(
        driven
            .window
            .conversation()
            .panel(alo_layout::Size::new(240.0, 820.0), Scale::ONE)
            .deleted,
        None
    );
    assert!(
        driven
            .said
            .iter()
            .any(|line| line.contains("numbered 1 was deleted, and its file removed")),
        "{:?}",
        driven.said
    );

    // 4. Deleted, and alo closed before its Undo passed: removed as it
    //    closes.
    delete_the_first_earlier(&mut driven)?;
    assert!(file_of(&chats, 0).exists());
    let said = driven.close()?;
    assert!(!file_of(&chats, 0).exists(), "{said:?}");

    // 5. Launched again: neither chat is there.
    let driven = the_window(port, &chats)?;
    assert_eq!(driven.rows(), rows(&[NEW_CHAT]));
    let _ = driven.close()?;
    let _ = std::fs::remove_dir_all(&chats);
    Ok(())
}
