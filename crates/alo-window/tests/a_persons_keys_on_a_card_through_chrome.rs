/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 741, ADR 0076 B, ADR 0024 § 5: the rest of a person's keys
//! over the card's page reach the page in Chrome as a person's input —
//! through the machine's Chrome and through the Chromium alo carries
//! (alo's CEF host, ADR 0079), each the same test.
//!
//! The conversation's window's own path, short of the window server, as
//! item 718's test drives it: the card is opened from the box, a press on
//! its field aimed as the window aims it and counted by the window's own
//! pointer ([`CardPointer`]) gives the card the keyboard, and each key is
//! read as the window reads it ([`card_keys::key`]) and handed to the
//! opener ([`ChromeOpener::person`]). Asserted, as the page says it to the
//! server:
//!
//! 1. **An arrow moves the caret in a field**: `abc`, ← twice and `X` leave
//!    `aXbc`; the forward delete then leaves `aXc`.
//! 2. **⌘A then typing replaces a field's text**: `new`.
//! 3. **A pasted text arrives**: ⌘A, ⌘C, → and ⌘V leave `newnew`.
//! 4. **A function key reaches the page's script**: F5 heard as `keydown`.
//! 5. **The paging keys scroll the page**: pressed outside the field, Page
//!    Down scrolls it down and Home back to its top.
//!
//! ⌘C writes the system's pasteboard, as a person's copy does: what it held
//! as text is read before (`pbpaste`) and written back after (`pbcopy`),
//! however the test ends — anything else it held is lost.
//!
//! The real pointer is never moved and no key is posted to the system.
//! Never the real network: the address is `http://127.0.0.1:PORT/…`.

#![cfg(target_os = "macos")]

mod carried;

use alo_window::beside::Chromium;
use alo_window::card_aim::{self, Aimed};
use alo_window::card_input::CardInput;
use alo_window::card_keys;
use alo_window::card_pointer::{CardPointer, OnThePage};
use alo_window::conversation::{Conversation, Pressed};
use alo_window::conversation_box::Typed;
use alo_window::conversation_chrome::{ChromeOpener, Streamed, Went};
use alo_window::conversation_place::{ConversationPlacement, Parts, start_viewport};
use alo_window::keys::{Arrow, Key, Modifier, Press};
use alo_window::scale::Scale;
use alo_window::typing::edit;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long Chrome is waited for to open a page.
const AT_MOST: Duration = Duration::from_secs(90);

/// How long the page is waited for to say, or a frame to show, what a
/// person's input did.
const SAID_WITHIN: Duration = Duration::from_secs(30);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The page: a field, and under it a page tall enough to scroll; it tells
/// the server its field's value as it changes, each key that goes down
/// with no text, and how far it is scrolled.
const CARD_PAGE: &str = "<!doctype html><title>Card</title>\
    <style>body{margin:0;height:4000px;background:#fff}\
    #field{position:absolute;left:100px;top:60px;width:300px;height:40px;font-size:20px}</style>\
    <input id=field aria-label=Field>\
    <script>const said = (what) => fetch('/said/' + what, {cache: 'no-store'});\
      const field = document.getElementById('field');\
      field.addEventListener('input', () => said('typed?' + encodeURIComponent(field.value)));\
      addEventListener('keydown', (event) => { if (event.key.length > 1) said('key?' + event.key); });\
      addEventListener('scroll', () => said('scrolled?' + Math.round(scrollY)));\
      said('ready');</script>";

/// The middle of the field, in the page's CSS pixels.
const FIELD_AT: (f64, f64) = (250.0, 80.0);

/// Somewhere on the page with nothing on it.
const EMPTY_AT: (f64, f64) = (600.0, 300.0);

/// Every `/said/…` the page told the server, in order, without `/said/`.
type Said = Arc<Mutex<Vec<String>>>;

/// A server on this machine answering the page and every `/said/…`: its
/// port, and what the page said.
fn serve() -> Result<(u16, Said), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    let said = Arc::new(Mutex::new(Vec::new()));
    let heard = Arc::clone(&said);
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let heard = Arc::clone(&heard);
            std::thread::spawn(move || {
                let _ = socket.set_read_timeout(Some(Duration::from_secs(5)));
                let mut asked = Vec::new();
                let mut block = [0u8; 1024];
                while let Ok(got) = socket.read(&mut block) {
                    asked.extend_from_slice(block.get(..got).unwrap_or_default());
                    if got == 0 || asked.windows(4).any(|end| end == b"\r\n\r\n") {
                        break;
                    }
                }
                let asked = String::from_utf8_lossy(&asked);
                let Some(path) = asked.split(' ').nth(1) else {
                    return;
                };
                let (status, body) = match path {
                    "/card.html" => ("200 OK", CARD_PAGE),
                    _ => match path.strip_prefix("/said/") {
                        Some(what) => {
                            if let Ok(mut heard) = heard.lock() {
                                heard.push(what.to_owned());
                            }
                            ("200 OK", "")
                        }
                        None => ("404 Not Found", ""),
                    },
                };
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Ok((port, said))
}

/// The window, its parts painted to fill their rooms at one, as it aims a
/// point: where it is placed, and the first screen's viewport.
fn the_window() -> (ConversationPlacement, alo_layout::Size) {
    let placed = ConversationPlacement::of(
        WINDOW,
        Scale::ONE,
        Parts {
            panel: Some((240, 820)),
            start: Some((1040, 820)),
            ..Parts::default()
        },
    );
    (placed, start_viewport(WINDOW, Scale::ONE))
}

/// The window's device pixel at which a person sees the page's point
/// `on`, at one: the panel's 240, then the card's page box at 81, 145.
fn on_the_window(on: (f64, f64)) -> (f64, f64) {
    (240.0 + 81.0 + on.0, 145.0 + on.1)
}

/// A card open on the test's page: the conversation holding it, the
/// opener holding its Chrome and what it tells, the window's pointer, what
/// the page said, and who.
struct Card<'w> {
    conversation: Conversation,
    opener: ChromeOpener,
    told: Receiver<Went>,
    pointer: CardPointer,
    said: Said,
    who: &'w str,
    /// What the opener told besides frames, said when something fails.
    other: Vec<String>,
}

impl Card<'_> {
    /// Where the window's point `at` is on the page, as the window's own
    /// [`card_aim`] has it while the card shows its picture.
    fn on(&self, at: (f64, f64)) -> Option<OnThePage> {
        let (placed, viewport) = the_window();
        let painted = self
            .conversation
            .card()
            .and_then(|card| card.frame.as_ref())
            .map(|_| card_aim::OnScreen {
                viewport,
                down: 0.0,
                offered: 0,
                stands: alo_renderer::split_place::Stands::InTheConversation,
            })?;
        Some(OnThePage {
            at: card_aim::on_the_page(&placed, Scale::ONE, painted, at),
            over: matches!(
                card_aim::aim(&placed, Scale::ONE, Some(painted), at),
                Some(Aimed::Page { .. })
            ),
        })
    }

    /// The cursor moved to the window's point `at`, as the window hands
    /// the move on.
    fn move_to(&mut self, at: (f64, f64)) {
        let on = self.on(at);
        if let Some(input) = self.pointer.moved(on) {
            self.opener.person(input);
        }
    }

    /// A press of the main button and its release at the window's point
    /// `at`, as the window sends them.
    fn click_at(&mut self, at: (f64, f64)) -> Result<(), String> {
        let on = self
            .on(at)
            .filter(|on| on.over)
            .ok_or(format!("{at:?} is not over the card's page"))?;
        let _ = self.conversation.press_the_page();
        let pressed = self.pointer.pressed(Instant::now(), on.at);
        self.opener.person(pressed);
        if let Some(released) = self.pointer.released(Some(on.at)) {
            self.opener.person(released);
        }
        Ok(())
    }

    /// Wait until the page has said `wanted`.
    fn heard(&self, wanted: &str) -> Result<(), String> {
        let until = Instant::now() + SAID_WITHIN;
        loop {
            let so_far = self
                .said
                .lock()
                .map(|said| said.clone())
                .unwrap_or_default();
            if so_far.iter().any(|each| each == wanted) {
                return Ok(());
            }
            if Instant::now() > until {
                return Err(format!(
                    "{}'s page never said {wanted}; it said {so_far:?}",
                    self.who
                ));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// What the page said that begins `kind`, in order.
    fn said_of(&self, kind: &str) -> Vec<String> {
        self.said
            .lock()
            .map(|said| {
                said.iter()
                    .filter(|each| each.starts_with(kind))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Type `text` into the box and press Return, as the window reads keys.
fn type_and_return(conversation: &mut Conversation, text: &str) -> Typed {
    for letter in text.chars() {
        let written = letter.to_string();
        if let Some(edit) = edit(&Press::character(&written), Some(&written)) {
            let _ = conversation.edit(edit);
        }
    }
    edit(&Press::of(Key::Return), None).map_or(Typed::Unchanged, |edit| conversation.edit(edit))
}

/// The test's page typed into the box and opened as a card in `chromium`,
/// its first frame held and the page ready.
fn opened(chromium: Chromium, who: &str) -> Result<Card<'_>, String> {
    let (port, said) = serve()?;
    let (tell, told) = channel();
    let opener = ChromeOpener::start(chromium, Box::new(move |went| tell.send(went).is_ok()))
        .map_err(|why| why.to_string())?;
    let mut conversation = Conversation::default();
    let _ = conversation.act(Pressed::TheBox);
    let address = format!("http://127.0.0.1:{port}/card.html");
    if !matches!(
        type_and_return(&mut conversation, &address),
        Typed::Chose(_)
    ) {
        return Err(format!("Return on {address} chose nothing"));
    }
    let url = conversation
        .going()
        .cloned()
        .ok_or("the address is not one to go to")?;
    opener.open(
        url.clone(),
        Streamed::at(alo_renderer::card::PAGE_SIZE, Scale::ONE),
    );
    conversation.open_card(url);
    let mut card = Card {
        conversation,
        opener,
        told,
        pointer: CardPointer::default(),
        said,
        who,
        other: Vec::new(),
    };
    let until = Instant::now() + AT_MOST;
    while card
        .conversation
        .card()
        .is_some_and(|held| held.frame.is_none())
    {
        if Instant::now() > until {
            return Err(format!(
                "{who} streamed no frame of the card: {:?}",
                card.other
            ));
        }
        match card.told.recv_timeout(Duration::from_millis(200)) {
            Ok(Went::Framed(png)) => {
                let _ = card.conversation.framed(png);
            }
            Ok(went) => card.other.push(went.said()),
            Err(_) => {}
        }
    }
    card.heard("ready")?;
    Ok(card)
}

impl Card<'_> {
    /// A person's key, read as the window reads it, sent to the page while
    /// the card has the keyboard: whether it was sent.
    fn key(&self, press: &Press, written: Option<&str>) -> bool {
        if !self.conversation.keys_to_the_card() {
            return false;
        }
        match card_keys::key(press, written) {
            Some(key) => {
                self.opener.person(CardInput::Key(key));
                true
            }
            None => false,
        }
    }

    /// Letters typed, each as the window reads it.
    fn type_text(&self, text: &str) {
        for letter in text.chars() {
            let written = letter.to_string();
            let _ = self.key(&Press::character(&written), Some(&written));
        }
    }

    /// The page's shortcut: `letter` with ⌘ held.
    fn command(&self, letter: &str) -> Result<(), String> {
        let press = Press::character(letter).with(Modifier::Command);
        if self.key(&press, Some(letter)) {
            Ok(())
        } else {
            Err(format!("{}: ⌘{letter} was not sent to the page", self.who))
        }
    }

    /// The last value the page said its field holds.
    fn field(&self) -> Option<String> {
        self.said_of("typed?").last().cloned()
    }
}

/// What the system's pasteboard holds as text, written back when this is
/// dropped.
struct PasteboardPutBack(Vec<u8>);

impl PasteboardPutBack {
    fn read() -> Result<Self, String> {
        std::process::Command::new("/usr/bin/pbpaste")
            .output()
            .map(|read| Self(read.stdout))
            .map_err(|why| format!("pbpaste: {why}"))
    }
}

impl Drop for PasteboardPutBack {
    fn drop(&mut self) {
        let written = std::process::Command::new("/usr/bin/pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut copy| {
                if let Some(mut into) = copy.stdin.take() {
                    into.write_all(&self.0)?;
                }
                copy.wait()
            });
        if !written.is_ok_and(|status| status.success()) {
            eprintln!("the pasteboard could not be put back");
        }
    }
}

/// The first and second closing conditions, and the forward delete: an
/// arrow moves the caret, ⌘A then typing replaces the field's text.
fn arrows_and_select_all(card: &mut Card<'_>) -> Result<(), String> {
    let who = card.who;
    let field = on_the_window(FIELD_AT);
    card.move_to(field);
    card.click_at(field)?;
    assert!(
        card.conversation.keys_to_the_card(),
        "{who}: the page has no keys"
    );
    card.type_text("abc");
    card.heard("typed?abc")?;
    for _ in 0..2 {
        assert!(
            card.key(&Press::of(Key::Arrow(Arrow::Left)), None),
            "{who}: ← not sent"
        );
    }
    card.type_text("X");
    card.heard("typed?aXbc")?;
    assert!(card.key(&Press::of(Key::Delete), None), "{who}: ⌦ not sent");
    card.heard("typed?aXc")?;
    assert_eq!(card.field().as_deref(), Some("typed?aXc"), "{who}");
    card.command("a")?;
    card.type_text("new");
    card.heard("typed?new")?;
    assert_eq!(card.field().as_deref(), Some("typed?new"), "{who}");
    eprintln!("{who}: the field said {:?}", card.said_of("typed?"));
    Ok(())
}

/// The third: what was copied, pasted.
fn copied_and_pasted(card: &Card<'_>) -> Result<(), String> {
    card.command("a")?;
    card.command("c")?;
    assert!(
        card.key(&Press::of(Key::Arrow(Arrow::Right)), None),
        "→ not sent"
    );
    card.command("v")?;
    card.heard("typed?newnew")?;
    assert_eq!(
        card.field().as_deref(),
        Some("typed?newnew"),
        "{}",
        card.who
    );
    eprintln!("{}: pasted, the field said newnew", card.who);
    Ok(())
}

/// The fourth and fifth: a function key heard, and the paging keys scroll.
fn function_and_paging_keys(card: &mut Card<'_>) -> Result<(), String> {
    assert!(card.key(&Press::of(Key::Function(5)), None), "F5 not sent");
    card.heard("key?F5")?;
    // Out of the field, onto the page itself.
    let empty = on_the_window(EMPTY_AT);
    card.move_to(empty);
    card.click_at(empty)?;
    assert!(
        card.key(&Press::of(Key::PageDown), None),
        "Page Down not sent"
    );
    let until = Instant::now() + SAID_WITHIN;
    while !card.said_of("scrolled?").iter().any(|said| {
        said.trim_start_matches("scrolled?")
            .parse::<u32>()
            .is_ok_and(|down| down > 100)
    }) {
        if Instant::now() > until {
            return Err(format!(
                "{}: Page Down did not scroll the page: {:?}",
                card.who,
                card.said_of("")
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(card.key(&Press::of(Key::Home), None), "Home not sent");
    card.heard("scrolled?0")?;
    eprintln!(
        "{}: F5 heard; scrolled {:?}",
        card.who,
        card.said_of("scrolled?")
    );
    Ok(())
}

/// Everything this item asks of a person's keys, through `chromium`.
fn through(chromium: Chromium, who: &str) -> Result<(), String> {
    let _put_back = PasteboardPutBack::read()?;
    let mut card = opened(chromium, who)?;
    arrows_and_select_all(&mut card)?;
    copied_and_pasted(&card)?;
    function_and_paging_keys(&mut card)?;
    if !card.opener.finish() {
        return Err(format!("{who}: the opener's thread did not finish"));
    }
    Ok(())
}

#[test]
fn a_persons_arrows_shortcuts_and_paging_keys_on_a_card_reach_the_page_through_chrome()
-> Result<(), String> {
    through(Chromium::Machines, "Chrome")
}

#[test]
fn a_persons_arrows_shortcuts_and_paging_keys_on_a_card_reach_the_page_through_alos_cef_host()
-> Result<(), String> {
    let Some(host) = carried::host("741")? else {
        return Ok(());
    };
    through(host.chromium(), "alo's CEF host")
}
