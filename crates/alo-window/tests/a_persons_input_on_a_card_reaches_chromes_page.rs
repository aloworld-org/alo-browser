/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 718, ADR 0076 B, ADR 0024 § 5: a person's press, keys and
//! wheel over the card's picture of its page reach the page in Chrome as a
//! person's input — through the machine's Chrome and through the Chromium
//! alo carries (alo's CEF host, ADR 0079), each the same test.
//!
//! The conversation's window's own path, short of the window server: a
//! person's keys type an address of this test's own server into the box and
//! Return opens it as a card ([`ChromeOpener`], as item 713's test does);
//! the card holds the first frame Chrome streams. Then each point a person
//! presses or turns a wheel over is a point of the window in device pixels,
//! aimed as the window aims it ([`card_aim::aim`]) against the design's
//! window at one, and each key is read as the window reads it
//! ([`card_keys::key`]) while the card has the keyboard — and handed to the
//! opener as the window hands it ([`ChromeOpener::person`]). Each asserted,
//! as the page itself says it to the server (its script reads its field's
//! value, its `scrollY` and its `location`):
//!
//! 1. **A press in the field and typing reach the field**: the press gives
//!    the card the keyboard, the box loses it, and `Ada Lx`, then
//!    Backspace, leave the field's value `Ada L`; the box holds nothing of
//!    it.
//! 2. **A press on a link follows it**: the page at the link's address says
//!    where it is, and a frame streamed after it shows its colour where the
//!    link was pressed.
//! 3. **A wheel scrolls the page**: three notches down, 120 CSS pixels.
//!
//! The real pointer is never moved and no key is posted to the system:
//! every event is the window's own reading of one.
//!
//! Never the real network: the address is `http://127.0.0.1:PORT/…`.

#![cfg(target_os = "macos")]

mod carried;

use alo_window::beside::Chromium;
use alo_window::card_aim::{self, Aimed, Turned};
use alo_window::card_input::{CardInput, Went as Button};
use alo_window::card_keys;
use alo_window::conversation::{Conversation, Pressed};
use alo_window::conversation_box::Typed;
use alo_window::conversation_chrome::{ChromeOpener, Streamed, Went};
use alo_window::conversation_place::{ConversationPlacement, Parts, start_viewport};
use alo_window::keys::{Key, Press};
use alo_window::scale::Scale;
use alo_window::typing::edit;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long Chrome is waited for to open a page, past the thirty seconds
/// Chrome itself is given to answer.
const AT_MOST: Duration = Duration::from_secs(90);

/// How long the page is waited for to say what a person's input did.
const SAID_WITHIN: Duration = Duration::from_secs(30);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The first page: a field and a link under it, each at a place of its own
/// in the page's CSS pixels; it tells the server its field's value as it
/// changes.
const CARD_PAGE: &str = "<!doctype html><title>Card</title>\
    <style>body{margin:0;background:#fff}\
    #field{position:absolute;left:100px;top:60px;width:300px;height:40px;font-size:20px}\
    #next{position:absolute;left:100px;top:160px;width:300px;height:100px;\
      display:block;background:#1d4ed8;color:#fff}</style>\
    <input id=field aria-label=Field><a id=next href=/two.html>Two</a>\
    <script>const said = (what) => fetch('/said/' + what, {cache: 'no-store'});\
      const field = document.getElementById('field');\
      field.addEventListener('input', () => said('typed?' + encodeURIComponent(field.value)));\
      said('ready');</script>";

/// The page the link leads to: all one colour and tall, telling the server
/// where it is and how far it is scrolled.
const TWO_PAGE: &str = "<!doctype html><title>Two</title>\
    <style>body{margin:0;height:4000px;background:#e76f51}</style><h1>Two</h1>\
    <script>const said = (what) => fetch('/said/' + what, {cache: 'no-store'});\
      addEventListener('scroll', () => said('scrolled?' + Math.round(scrollY)));\
      said('arrived?' + encodeURIComponent(location.href));</script>";

/// The colour of [`TWO_PAGE`], as a frame's channels from zero to one.
const TWO: (f32, f32, f32) = (231.0 / 255.0, 111.0 / 255.0, 81.0 / 255.0);

/// Where the field is pressed, in the page's CSS pixels: its middle.
const FIELD_AT: (f64, f64) = (250.0, 80.0);

/// Where the link is pressed: its middle.
const LINK_AT: (f64, f64) = (250.0, 210.0);

/// Where the wheel is turned.
const WHEEL_AT: (f64, f64) = (500.0, 300.0);

/// Every `/said/…` the pages told the server, in order, without `/said/`.
type Said = Arc<Mutex<Vec<String>>>;

/// A server on this machine answering the two pages and every `/said/…`:
/// its port, and what the pages said.
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
                    "/two.html" => ("200 OK", TWO_PAGE),
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

/// Wait until the pages have said `wanted`: what they had said by then, or
/// why not, with everything they said.
fn heard(said: &Said, wanted: &str, who: &str) -> Result<(), String> {
    let until = Instant::now() + SAID_WITHIN;
    loop {
        let so_far = said.lock().map(|said| said.clone()).unwrap_or_default();
        if so_far.iter().any(|each| each == wanted) {
            return Ok(());
        }
        if Instant::now() > until {
            return Err(format!(
                "{who}'s page never said {wanted}; it said {so_far:?}"
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
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

/// Where in the page a person's point `at` of the window is, aimed as the
/// window aims it while `conversation`'s card shows its picture.
fn aimed(conversation: &Conversation, at: (f64, f64)) -> Result<(f64, f64), String> {
    let (placed, viewport) = the_window();
    let card = conversation
        .card()
        .and_then(|card| card.frame.as_ref())
        .map(|_| viewport);
    match card_aim::aim(&placed, Scale::ONE, card, at) {
        Some(Aimed::Page { x, y }) => Ok((x, y)),
        other => Err(format!("{at:?} was aimed at {other:?}, not the page")),
    }
}

/// A person's click at the page's point `on`: pressed and released as the
/// window sends them, the press giving the card the keyboard.
fn click(
    conversation: &mut Conversation,
    opener: &ChromeOpener,
    on: (f64, f64),
) -> Result<(), String> {
    let (x, y) = aimed(conversation, on_the_window(on))?;
    let _ = conversation.press_the_page();
    for went in [Button::Pressed, Button::Released] {
        opener.person(CardInput::Pointer { went, x, y });
    }
    Ok(())
}

/// A person's key, read as the window reads it, sent to the page while the
/// card has the keyboard.
fn key(conversation: &Conversation, opener: &ChromeOpener, press: &Press, written: Option<&str>) {
    if conversation.keys_to_the_card()
        && let Some(key) = card_keys::key(press, written)
    {
        opener.person(CardInput::Key(key));
    }
}

/// Type `text` into the box and press Return, as the window reads keys:
/// what Return came to.
fn type_and_return(conversation: &mut Conversation, text: &str) -> Typed {
    for letter in text.chars() {
        let written = letter.to_string();
        if let Some(edit) = edit(&Press::character(&written), Some(&written)) {
            let _ = conversation.edit(edit);
        }
    }
    edit(&Press::of(Key::Return), None).map_or(Typed::Unchanged, |edit| conversation.edit(edit))
}

/// The newest frame the opener tells of within `long`, if any, the card
/// given each — and anything else said, kept to say.
fn frames(
    told: &Receiver<Went>,
    conversation: &mut Conversation,
    long: Duration,
    other: &mut Vec<String>,
) -> Option<Vec<u8>> {
    let until = Instant::now() + long;
    let mut newest = None;
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        match told.recv_timeout(left) {
            Ok(Went::Framed(png)) => {
                let _ = conversation.framed(png.clone());
                newest = Some(png);
            }
            Ok(went) => other.push(went.said()),
            Err(_) => break,
        }
    }
    newest
}

/// Whether `png` shows [`TWO`] at the page's point `on`, whatever scale the
/// frame came at.
fn shows_two(png: &[u8], on: (f64, f64)) -> bool {
    let Ok(canvas) = alo_paint::picture::read(png) else {
        return false;
    };
    let scale = f64::from(canvas.width()) / f64::from(alo_renderer::card::PAGE_SIZE.0);
    let at = |side: f64| {
        let device = (side * scale).floor();
        if (0.0..f64::from(u32::MAX)).contains(&device) {
            format!("{device}").parse::<u32>().ok()
        } else {
            None
        }
    };
    let (Some(x), Some(y)) = (at(on.0), at(on.1)) else {
        return false;
    };
    canvas.at(x, y).is_some_and(|pixel| {
        (pixel.red - TWO.0).abs() < 0.02
            && (pixel.green - TWO.1).abs() < 0.02
            && (pixel.blue - TWO.2).abs() < 0.02
    })
}

/// A card open on the test's page: the conversation holding it, the opener
/// holding its Chrome and what it tells, what the pages said, and who.
struct Card<'w> {
    conversation: Conversation,
    opener: ChromeOpener,
    told: Receiver<Went>,
    said: Said,
    port: u16,
    who: &'w str,
    /// What the opener told besides frames, said when something fails.
    other: Vec<String>,
}

impl Card<'_> {
    /// The newest frame told within a fifth of a second, if any, the card
    /// given each.
    fn frame(&mut self) -> Option<Vec<u8>> {
        frames(
            &self.told,
            &mut self.conversation,
            Duration::from_millis(200),
            &mut self.other,
        )
    }

    /// What the pages said that begins `kind`, in order.
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
    opener.open(url.clone(), Streamed::at(Scale::ONE));
    // As the window does on being told it is opening.
    conversation.open_card(url);
    let mut card = Card {
        conversation,
        opener,
        told,
        said,
        port,
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
        let _ = card.frame();
    }
    heard(&card.said, "ready", who)?;
    Ok(card)
}

/// The first closing condition: a press in the field, and typing — the card
/// has the keyboard, the box has nothing of it, and the field's value is
/// what was typed.
fn typed_into_the_field(card: &mut Card<'_>) -> Result<(), String> {
    let who = card.who;
    click(&mut card.conversation, &card.opener, FIELD_AT)?;
    assert!(
        card.conversation.keys_to_the_card(),
        "{who}: the page has no keys"
    );
    let the_box = |conversation: &Conversation| {
        conversation
            .start(alo_layout::Size::new(1040.0, 820.0), Scale::ONE)
            .typed
    };
    let before = the_box(&card.conversation);
    for letter in ["A", "d", "a", " ", "L", "x"] {
        let press = Press::character(letter);
        key(&card.conversation, &card.opener, &press, Some(letter));
    }
    let backspace = Press::of(Key::Backspace);
    key(&card.conversation, &card.opener, &backspace, Some("\u{8}"));
    heard(&card.said, "typed?Ada%20L", who)?;
    let typed = card.said_of("typed?");
    assert_eq!(
        typed.last().map(String::as_str),
        Some("typed?Ada%20L"),
        "{who}: the field's value, as the page said it: {typed:?}"
    );
    assert_eq!(
        the_box(&card.conversation),
        before,
        "{who}: the box was typed into"
    );
    eprintln!("{who}: the field said {typed:?}");
    Ok(())
}

/// The second: a press on the link follows it, and a frame after shows the
/// page it led to.
fn followed_the_link(card: &mut Card<'_>) -> Result<(), String> {
    let who = card.who;
    click(&mut card.conversation, &card.opener, LINK_AT)?;
    let two = format!(
        "arrived?{}",
        encoded(&format!("http://127.0.0.1:{}/two.html", card.port))
    );
    heard(&card.said, &two, who)?;
    let until = Instant::now() + SAID_WITHIN;
    while !card.frame().is_some_and(|png| shows_two(&png, LINK_AT)) {
        if Instant::now() > until {
            return Err(format!(
                "{who}: no frame showed the page the link led to: {:?}",
                card.other
            ));
        }
    }
    eprintln!("{who}: the link was followed, and a frame shows {two}");
    Ok(())
}

/// The third: a wheel turned three notches down over the page scrolls it
/// 120.
fn scrolled_by_the_wheel(card: &Card<'_>) -> Result<(), String> {
    let (x, y) = aimed(&card.conversation, on_the_window(WHEEL_AT))?;
    let (right, down) = card_aim::scrolled(
        Turned::Lines {
            across: 0.0,
            down: -3.0,
        },
        Scale::ONE,
    );
    card.opener.person(CardInput::Wheel { x, y, right, down });
    heard(&card.said, "scrolled?120", card.who)?;
    eprintln!(
        "{}: the wheel scrolled the page, as it said: {:?}",
        card.who,
        card.said_of("scrolled?")
    );
    Ok(())
}

/// Everything this item asks of a person's input, through `chromium`, named
/// `who` in what is said.
fn through(chromium: Chromium, who: &str) -> Result<(), String> {
    let mut card = opened(chromium, who)?;
    typed_into_the_field(&mut card)?;
    followed_the_link(&mut card)?;
    scrolled_by_the_wheel(&card)?;
    if !card.opener.finish() {
        return Err(format!("{who}: the opener's thread did not finish"));
    }
    Ok(())
}

/// `text` as `encodeURIComponent` writes it, for the characters an address
/// of this test's holds.
fn encoded(text: &str) -> String {
    text.chars()
        .map(|each| match each {
            ':' => "%3A".to_owned(),
            '/' => "%2F".to_owned(),
            other => other.to_string(),
        })
        .collect()
}

#[test]
fn a_persons_press_keys_and_wheel_on_a_card_reach_the_page_through_chrome() -> Result<(), String> {
    through(Chromium::Machines, "Chrome")
}

#[test]
fn a_persons_press_keys_and_wheel_on_a_card_reach_the_page_through_alos_cef_host()
-> Result<(), String> {
    let Some(host) = carried::host("718")? else {
        return Ok(());
    };
    through(host.chromium(), "alo's CEF host")
}
