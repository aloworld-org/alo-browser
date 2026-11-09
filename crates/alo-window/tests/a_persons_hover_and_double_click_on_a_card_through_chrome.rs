/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 740, ADR 0076 B, ADR 0024 § 5: the rest of a person's pointer
//! over the card's picture of its page reaches the page in Chrome as a
//! person's input — through the machine's Chrome and through the Chromium
//! alo carries (alo's CEF host, ADR 0079), each the same test.
//!
//! The conversation's window's own path, short of the window server, as
//! item 718's test drives it: the card is opened from the box, and each
//! point of the window a person's pointer is at is aimed as the window aims
//! it ([`card_aim::aim`], [`card_aim::on_the_page`]) and handed to the
//! window's own pointer ([`CardPointer`]), whose answer is handed to the
//! opener as the window hands it ([`ChromeOpener::person`]). Asserted, as
//! the page says it to the server and as the frames it streams show it:
//!
//! 1. **A hover shows in a frame**: the pointer moved over a box whose
//!    `:hover` colours it, the page hears `mouseenter` and a frame shows
//!    the colour where the box is; moved off the page box onto the card's
//!    bar, the page hears `mouseleave` and a frame shows the box white.
//! 2. **A double click selects a word**: two presses in quick succession on
//!    `bravo` select it, and the page hears `dblclick`; the last selection
//!    it reports, in the order it reported them ([`numbered`], queue item
//!    889), is `bravo`.
//! 3. **How long a person waits** — said, not asserted: from the hover, a
//!    press and a key handed to the opener to the first frame told to the
//!    window that shows what each did.
//!
//! The real pointer is never moved and no key is posted to the system.
//! Never the real network: the address is `http://127.0.0.1:PORT/…`.

#![cfg(target_os = "macos")]

mod carried;
mod numbered;

use alo_window::beside::Chromium;
use alo_window::card_aim::{self, Aimed};
use alo_window::card_input::CardInput;
use alo_window::card_keys;
use alo_window::card_pointer::{CardPointer, OnThePage};
use alo_window::conversation::{Conversation, Pressed};
use alo_window::conversation_box::Typed;
use alo_window::conversation_chrome::{ChromeOpener, Streamed, Went};
use alo_window::conversation_place::{ConversationPlacement, Parts, start_viewport};
use alo_window::keys::{Key, Press};
use alo_window::scale::Scale;
use alo_window::typing::edit;
use numbered::Said;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long Chrome is waited for to open a page.
const AT_MOST: Duration = Duration::from_secs(90);

/// How long the page is waited for to say, or a frame to show, what a
/// person's input did.
const SAID_WITHIN: Duration = Duration::from_secs(30);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The page: a box coloured while hovered, blue once pressed and green
/// once a key goes down in it, and a line of words; it tells the server
/// what it hears, each report numbered ([`numbered`]).
const CARD_PAGE: &str = "<!doctype html><title>Card</title>\
    <style>body{margin:0;background:#fff}\
    #hot{position:absolute;left:100px;top:60px;width:300px;height:100px;background:#fff;outline:none}\
    #hot:hover{background:#e76f51}\
    #hot.pressed{background:#1d4ed8}\
    #hot.keyed{background:#2a9d8f}\
    #words{position:absolute;left:100px;top:240px;margin:0;font:40px/48px sans-serif}</style>\
    <div id=hot tabindex=0></div><p id=words>bravo charlie</p>\
    <script>let told = 0;\
      const said = (what) => fetch('/said/' + (told += 1) + '/' + what, {cache: 'no-store'});\
      const hot = document.getElementById('hot');\
      hot.addEventListener('mouseenter', () => said('entered'));\
      hot.addEventListener('mouseleave', () => said('left'));\
      hot.addEventListener('mousedown', () => { hot.className = 'pressed'; said('pressed'); });\
      hot.addEventListener('keydown', () => { hot.className = 'keyed'; said('keyed'); });\
      addEventListener('dblclick', () => said('dblclick'));\
      document.addEventListener('selectionchange', () =>\
        said('selected?' + encodeURIComponent(getSelection().toString())));\
      said('ready');</script>";

/// The colours, as a frame's channels from zero to one.
const HOVERED: (f32, f32, f32) = (231.0 / 255.0, 111.0 / 255.0, 81.0 / 255.0);
const WHITE: (f32, f32, f32) = (1.0, 1.0, 1.0);
const PRESSED: (f32, f32, f32) = (29.0 / 255.0, 78.0 / 255.0, 216.0 / 255.0);
const KEYED: (f32, f32, f32) = (42.0 / 255.0, 157.0 / 255.0, 143.0 / 255.0);

/// The middle of the box, in the page's CSS pixels.
const HOT_AT: (f64, f64) = (250.0, 110.0);

/// On the first letters of `bravo`.
const WORD_AT: (f64, f64) = (130.0, 264.0);

/// A server on this machine answering the page and every `/said/…`: its
/// port, and what the page said.
fn serve() -> Result<(u16, Said), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    let said = Said::default();
    let heard = said.clone();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let heard = heard.clone();
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
                            heard.keep(what);
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

/// The colour `png` shows at the page's point `on`, whatever scale the
/// frame came at, if it is `wanted`.
fn shows(png: &[u8], on: (f64, f64), wanted: (f32, f32, f32)) -> bool {
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
        (pixel.red - wanted.0).abs() < 0.02
            && (pixel.green - wanted.1).abs() < 0.02
            && (pixel.blue - wanted.2).abs() < 0.02
    })
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

    /// Wait for a frame that shows `wanted` at the page's point `on`: how
    /// long after `since` it was told.
    fn frame_showing(
        &mut self,
        on: (f64, f64),
        wanted: (f32, f32, f32),
        since: Instant,
    ) -> Result<Duration, String> {
        let until = Instant::now() + SAID_WITHIN;
        while let Some(left) = until.checked_duration_since(Instant::now()) {
            match self.told.recv_timeout(left) {
                Ok(Went::Framed(png)) => {
                    let told = Instant::now();
                    let _ = self.conversation.framed(png.clone());
                    if shows(&png, on, wanted) {
                        return Ok(told.duration_since(since));
                    }
                }
                Ok(went) => self.other.push(went.said()),
                Err(_) => break,
            }
        }
        Err(format!(
            "{}: no frame showed {wanted:?} at {on:?}: {:?}",
            self.who, self.other
        ))
    }

    /// Wait until the page has said `wanted`.
    fn heard(&self, wanted: &str) -> Result<(), String> {
        let until = Instant::now() + SAID_WITHIN;
        loop {
            if self.said.has(wanted) {
                return Ok(());
            }
            if Instant::now() > until {
                return Err(format!(
                    "{}'s page never said {wanted}; it said {:?}",
                    self.who,
                    self.said.in_order()
                ));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// What the page said that begins `kind`, in the order it said it.
    fn said_of(&self, kind: &str) -> Vec<String> {
        self.said.of(kind)
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

/// The first closing condition: a hover shows in a frame, and ends once
/// the pointer leaves the page box — how long the first took, said.
fn hovered(card: &mut Card<'_>) -> Result<Duration, String> {
    // Over the card's bar first, outside the page box: nothing is sent.
    let bar = (on_the_window(HOT_AT).0, 120.0);
    assert_eq!(
        card.on(bar).map(|on| on.over),
        Some(false),
        "{}: the bar is not the page",
        card.who
    );
    card.move_to(bar);
    let since = Instant::now();
    card.move_to(on_the_window((HOT_AT.0 - 4.0, HOT_AT.1)));
    card.move_to(on_the_window(HOT_AT));
    let took = card.frame_showing(HOT_AT, HOVERED, since)?;
    card.heard("entered")?;
    // Off the page box onto the bar: the hover ends.
    card.move_to(bar);
    card.frame_showing(HOT_AT, WHITE, Instant::now())?;
    card.heard("left")?;
    eprintln!(
        "{}: hovered, a frame showed it {} ms after the move; left, it ended",
        card.who,
        took.as_millis()
    );
    Ok(took)
}

/// The second: a double click selects the word under it.
fn double_clicked(card: &mut Card<'_>) -> Result<(), String> {
    let at = on_the_window(WORD_AT);
    card.move_to(at);
    card.click_at(at)?;
    card.click_at(at)?;
    card.heard("dblclick")?;
    card.heard("selected?bravo")?;
    let selected = card.said_of("selected?");
    assert_eq!(
        selected.last().map(String::as_str),
        Some("selected?bravo"),
        "{}: what the page said was selected: {selected:?}",
        card.who
    );
    eprintln!("{}: a double click selected {selected:?}", card.who);
    Ok(())
}

/// The third, said: a press and a key, each to the first frame that shows
/// what it did.
fn pressed_and_keyed(card: &mut Card<'_>) -> Result<(Duration, Duration), String> {
    let at = on_the_window(HOT_AT);
    card.move_to(at);
    // Long enough after the double click that this is a first press.
    std::thread::sleep(alo_window::card_clicks::QUICK);
    let since = Instant::now();
    card.click_at(at)?;
    let press = card.frame_showing(HOT_AT, PRESSED, since)?;
    card.heard("pressed")?;
    assert!(
        card.conversation.keys_to_the_card(),
        "{}: the page has no keys",
        card.who
    );
    let key = card_keys::key(&Press::character("k"), Some("k"))
        .ok_or("a letter sends nothing to the page")?;
    let since = Instant::now();
    card.opener.person(CardInput::Key(key));
    let keyed = card.frame_showing(HOT_AT, KEYED, since)?;
    card.heard("keyed")?;
    eprintln!(
        "{}: a press to a frame showing it {} ms, a key {} ms",
        card.who,
        press.as_millis(),
        keyed.as_millis()
    );
    Ok((press, keyed))
}

/// Everything this item asks of a person's pointer, through `chromium`.
fn through(chromium: Chromium, who: &str) -> Result<(), String> {
    let mut card = opened(chromium, who)?;
    let hover = hovered(&mut card)?;
    double_clicked(&mut card)?;
    let (press, key) = pressed_and_keyed(&mut card)?;
    eprintln!(
        "{who}: to a frame told to the window — a hover {} ms, a press {} ms, a key {} ms",
        hover.as_millis(),
        press.as_millis(),
        key.as_millis()
    );
    if !card.opener.finish() {
        return Err(format!("{who}: the opener's thread did not finish"));
    }
    Ok(())
}

#[test]
fn a_persons_hover_and_double_click_on_a_card_reach_the_page_through_chrome() -> Result<(), String>
{
    through(Chromium::Machines, "Chrome")
}

#[test]
fn a_persons_hover_and_double_click_on_a_card_reach_the_page_through_alos_cef_host()
-> Result<(), String> {
    let Some(host) = carried::host("740")? else {
        return Ok(());
    };
    through(host.chromium(), "alo's CEF host")
}
