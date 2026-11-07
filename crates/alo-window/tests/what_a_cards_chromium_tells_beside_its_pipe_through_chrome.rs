/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue items 799 and 798: what the Chromium alo carries tells of a card's
//! page beside its pipe, as the conversation is told it.
//!
//! Through alo's CEF host, the card parked and streaming, a person's input
//! sent as the conversation's window sends it:
//!
//! 1. **the pointer's shape** (799) — over a link the hand (`pointer`),
//!    over a paragraph's words the I-beam (`text`), over a field the I-beam,
//!    over the page's empty ground the arrow;
//! 2. **a secondary press** (798) — over the link, a menu over it with its
//!    address and its words selected, as a Mac selects them; over a word a double click selected,
//!    a menu with that word; on a page that takes `contextmenu` for
//!    itself, nothing — and the page heard each `contextmenu`;
//! 3. **a ⌘ press and a middle press on the link** (798) — its address,
//!    told to be opened, and the page **not** gone anywhere: the card is
//!    still at the page it was.
//!
//! Through the machine's Chrome nothing is told: the shape stays the arrow
//! and such a press is nobody's in the window (`conversation_window.rs`),
//! so none is tested here.
//!
//! Never the real network: the page is `http://127.0.0.1:PORT/card.html`.

#![cfg(target_os = "macos")]

mod carried;

use alo_window::agent_sites::AgentSites;
use alo_window::card_input::{CardInput, OtherPress, Went};
use alo_window::chrome_serving::{ChromeServing, Opening};
use alo_window::chrome_told::{Menu, Told};
use alo_window::frame_told::ToFrame;
use alo_window::inbox::Orders;
use alo_window::message::{CardOrder, Order};
use alo_window::scale::Scale;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The card's page box, as `alo-renderer` lays it out.
const PAGE: (u32, u32) = alo_renderer::card::PAGE_SIZE;

/// The page: a box turning, so it always has a frame to stream; a link; a
/// paragraph of words; a field; and a box that takes `contextmenu` for
/// itself. It tells the server each `contextmenu` it hears.
const CARD_PAGE: &str = "<!doctype html><title>Told</title>\
    <style>body{margin:0;font:20px/30px sans-serif}\
      #turning{position:absolute;left:800px;top:20px;width:40px;height:40px;\
        background:#e76f51;animation:turn 1s linear infinite}\
      @keyframes turn{to{transform:rotate(360deg)}}\
      #link,#words,#field,#own{position:absolute;left:40px}\
      #own{width:200px;height:60px;background:#ddd}</style>\
    <div id=turning></div>\
    <a id=link style='top:40px' href='/elsewhere.html'>A link</a>\
    <p id=words style='top:100px;margin:0'>Kampala stands on hills</p>\
    <input id=field style='top:180px;width:300px;font:20px sans-serif'>\
    <div id=own style='top:260px'></div>\
    <script>const said = (what) => fetch('/said/' + what, {cache: 'no-store'});\
      document.addEventListener('contextmenu', (e) => said('menu-' + e.target.id));\
      document.getElementById('own').addEventListener('contextmenu', (e) => e.preventDefault());\
      said('ready');</script>";

/// Over the link's words, in the page's CSS pixels.
const LINK_AT: (f64, f64) = (60.0, 52.0);

/// Over *Kampala*, the paragraph's first word.
const WORD_AT: (f64, f64) = (70.0, 115.0);

/// Over the field.
const FIELD_AT: (f64, f64) = (100.0, 195.0);

/// Over the box that takes its own `contextmenu`.
const OWN_AT: (f64, f64) = (100.0, 290.0);

/// Over the page's empty ground.
const GROUND_AT: (f64, f64) = (600.0, 450.0);

/// Every `/said/…` the page told the server, and every page asked for.
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
                let body = match path {
                    "/card.html" => CARD_PAGE,
                    "/elsewhere.html" => {
                        if let Ok(mut heard) = heard.lock() {
                            heard.push("elsewhere-asked".to_owned());
                        }
                        "<!doctype html><title>Elsewhere</title>Elsewhere"
                    }
                    _ => {
                        if let (Some(what), Ok(mut heard)) =
                            (path.strip_prefix("/said/"), heard.lock())
                        {
                            heard.push(what.to_owned());
                        }
                        ""
                    }
                };
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Ok((port, said))
}

/// Wait until the page has said `wanted`.
fn heard(said: &Said, wanted: &str) -> Result<(), String> {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        let so_far = said.lock().map(|said| said.clone()).unwrap_or_default();
        if so_far.iter().any(|each| each == wanted) {
            return Ok(());
        }
        if Instant::now() > until {
            return Err(format!("the page never said {wanted}; it said {so_far:?}"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Send the card `input`, as the conversation's window does.
fn send(orders: &Orders, input: CardInput) -> Result<(), String> {
    orders
        .send(Order::Card(CardOrder::Input(input)))
        .map_err(|_| "the card's thread has gone".to_owned())
}

/// The pointer at `at`, no button held.
fn hover(orders: &Orders, (x, y): (f64, f64)) -> Result<(), String> {
    send(
        orders,
        CardInput::Pointer {
            went: Went::Hovered,
            x,
            y,
        },
    )
}

/// Everything the card's Chromium told within `long`, in order, the
/// frames drained meanwhile.
fn told_within(told: &Receiver<ToFrame>, long: Duration) -> Vec<Told> {
    let until = Instant::now() + long;
    let mut all = Vec::new();
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        match told.recv_timeout(left) {
            Ok(ToFrame::Told(each)) => all.push(each),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    all
}

/// Wait at most ten seconds for the card's Chromium to tell something
/// `wanted` takes, and say it — failing with all it told meanwhile.
fn told_one<T>(
    told: &Receiver<ToFrame>,
    what: &str,
    wanted: impl Fn(&Told) -> Option<T>,
) -> Result<T, String> {
    let until = Instant::now() + Duration::from_secs(10);
    let mut all = Vec::new();
    while Instant::now() < until {
        for each in told_within(told, Duration::from_millis(100)) {
            if let Some(found) = wanted(&each) {
                return Ok(found);
            }
            all.push(each);
        }
    }
    Err(format!("{what}: never told; told {all:?}"))
}

/// The shape the page asks for once the pointer has gone to `at` and
/// half a second has passed: the newest told, or — Chromium telling a
/// shape only as it changes — `shape`, the one before, which this keeps.
fn shape_at(
    orders: &Orders,
    told: &Receiver<ToFrame>,
    at: (f64, f64),
    shape: &mut Option<&'static str>,
) -> Result<String, String> {
    hover(orders, at)?;
    for each in told_within(told, Duration::from_millis(600)) {
        if let Told::Cursor(told) = each {
            *shape = Some(told);
        }
    }
    shape
        .map(str::to_owned)
        .ok_or_else(|| format!("no shape told by {at:?}"))
}

/// Part 2: a secondary press over the link, over a word selected, and over
/// a box that takes its own `contextmenu`.
fn menus(orders: &Orders, told: &Receiver<ToFrame>, said: &Said, port: u16) -> Result<(), String> {
    let (orders, told, said) = (orders.clone(), told, said);
    // 2. A secondary press: over the link, over a word selected, and over a
    // box that takes its own.
    let secondary = |(x, y): (f64, f64)| CardInput::Other {
        press: OtherPress::Secondary,
        x,
        y,
    };
    send(&orders, secondary(LINK_AT))?;
    let on_link = told_one(told, "a menu over the link", |each| match each {
        Told::Menu(menu) => Some(menu.clone()),
        _ => None,
    })?;
    heard(said, "menu-link")?;
    // On a Mac a secondary press selects the word under it, a link's too
    // (measured, CEF 156.0.14): Chromium tells the link's words selected.
    assert_eq!(
        on_link,
        Menu {
            at: (60, 52),
            link: Some(format!("http://127.0.0.1:{port}/elsewhere.html")),
            selection: Some("A link".to_owned()),
            editable: false,
        }
    );
    for went in [
        Went::Pressed { clicks: 1 },
        Went::Released { clicks: 1 },
        Went::Pressed { clicks: 2 },
        Went::Released { clicks: 2 },
    ] {
        send(
            &orders,
            CardInput::Pointer {
                went,
                x: WORD_AT.0,
                y: WORD_AT.1,
            },
        )?;
    }
    send(&orders, secondary(WORD_AT))?;
    let on_word = told_one(told, "a menu over a word selected", |each| match each {
        Told::Menu(menu) => Some(menu.clone()),
        _ => None,
    })?;
    assert_eq!(on_word.selection.as_deref(), Some("Kampala"), "{on_word:?}");
    assert_eq!(on_word.link, None);
    send(&orders, secondary(OWN_AT))?;
    heard(said, "menu-own")?;
    let after_own = told_within(told, Duration::from_millis(800));
    assert!(
        !after_own.iter().any(|each| matches!(each, Told::Menu(_))),
        "a page that took its own contextmenu: {after_own:?}"
    );

    Ok(())
}

/// Part 3: a ⌘ press and a middle press on the link — told, and not
/// followed.
fn links(orders: &Orders, told: &Receiver<ToFrame>, said: &Said, port: u16) -> Result<(), String> {
    let (orders, told, said) = (orders.clone(), told, said);
    // 3. A ⌘ press and a middle press on the link: told, and not followed.
    for press in [OtherPress::Command, OtherPress::Middle] {
        send(
            &orders,
            CardInput::Other {
                press,
                x: LINK_AT.0,
                y: LINK_AT.1,
            },
        )?;
        let opened = told_one(told, "a link to open", |each| match each {
            Told::Open(address) => Some(address.clone()),
            _ => None,
        })?;
        assert_eq!(opened, format!("http://127.0.0.1:{port}/elsewhere.html"));
    }
    std::thread::sleep(Duration::from_millis(500));
    let so_far = said.lock().map(|said| said.clone()).unwrap_or_default();
    assert!(
        !so_far.iter().any(|each| each == "elsewhere-asked"),
        "the link was followed by Chromium: {so_far:?}"
    );

    Ok(())
}

#[test]
fn a_cards_chromium_tells_its_pointer_menus_and_links_through_alos_cef_host() -> Result<(), String>
{
    let Some(host) = carried::host("799")? else {
        return Ok(());
    };
    let (port, said) = serve()?;
    let url = alo_url::parse(&format!("http://127.0.0.1:{port}/card.html"))
        .map_err(|why| format!("{why:?}"))?;
    let (tell, told) = channel();
    let serving = ChromeServing::start_as_a_card(
        host.chromium(),
        Opening::Network(url),
        PAGE,
        Scale::ONE,
        AgentSites::Every,
        Box::new(move |news| tell.send(news).is_ok()),
    )
    .map_err(|why| format!("no card in Chromium: {why}"))?;
    let orders = serving.orders();
    orders
        .send(Order::Card(CardOrder::Stream {
            width: PAGE.0,
            height: PAGE.1,
            page: PAGE,
            scale: Scale::ONE,
        }))
        .map_err(|_| "the card's thread has gone")?;
    heard(&said, "ready")?;
    let _ = told_within(&told, Duration::from_millis(500));

    // 1. The pointer's shape.
    let mut shape = None;
    let over_link = shape_at(&orders, &told, LINK_AT, &mut shape)?;
    let over_words = shape_at(&orders, &told, WORD_AT, &mut shape)?;
    let over_ground = shape_at(&orders, &told, GROUND_AT, &mut shape)?;
    let over_field = shape_at(&orders, &told, FIELD_AT, &mut shape)?;
    eprintln!(
        "alo's CEF host told the shapes: link {over_link}, words {over_words}, \
         field {over_field}, ground {over_ground}"
    );
    assert_eq!(
        (
            over_link.as_str(),
            over_words.as_str(),
            over_field.as_str(),
            over_ground.as_str()
        ),
        ("pointer", "text", "text", "default")
    );

    menus(&orders, &told, &said, port)?;
    links(&orders, &told, &said, port)?;

    orders
        .send(Order::CloseEverything)
        .map_err(|_| "the card's thread has gone")?;
    drop(orders);
    serving
        .records()
        .ok_or("the card's thread did not finish")?;
    Ok(())
}
