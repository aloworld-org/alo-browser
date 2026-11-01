/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 808, ADR 0076 B4 as amended: no window of a parked card's
//! Chromium is seen, its own or one it opens beside it.
//!
//! Found by hand: the pointer over a link in a parked card's page showed
//! Chromium's status bubble — the link's address — as an opaque window of
//! its own at the parked window's bottom left corner, 41 points of it on
//! the Retina display. Measured through alo's CEF host for this item, the
//! card parked by a display at two (1670, 34 here, 878 × 552), a person's
//! input sent over the pipe as the conversation's window sends it: the
//! pointer over a link showed the bubble (1669, 565, 292 × 22, then 511
//! wide), seen; a press on a button asking where the person is showed the
//! permission prompt (320 × 266 at 1710, 66, opaque, just right of the
//! display); a `title`'s tooltip over 2.4 s and a press in a field with a
//! `datalist` showed nothing; a press on a closed list opened its menu,
//! moved onto the display by macOS (queue item 810, cut from this one).
//!
//! Through alo's CEF host, parked by the display at two, the card streaming:
//!
//! 1. the pointer over a link — the page hearing `mouseover` — and held
//!    there for 2.4 s: at every reading of the window server **no window of
//!    the host's process is drawn on any display**, read against every
//!    display's bounds, and **none is listed but the card's own**;
//! 2. a press on a button asking for the person's place — the page hearing
//!    its answer, the prompt dismissed unshown — and the same for 2.4 s;
//! 3. and it streamed throughout.
//!
//! Through the machine's Chrome nothing of alo's stands between a page and
//! its bubble or prompts (ADR 0076 B4's amendment, § 3): not tested.
//!
//! Never the real network: the page is `http://127.0.0.1:PORT/card.html`.

#![cfg(target_os = "macos")]

mod carried;

use alo_window::agent_sites::AgentSites;
use alo_window::card_input::{CardInput, Went};
use alo_window::chrome_protocol::Bounds;
use alo_window::chrome_serving::{ChromeServing, Opening};
use alo_window::frame_told::ToFrame;
use alo_window::inbox::Orders;
use alo_window::message::{CardOrder, Order};
use alo_window::scale::Scale;
use alo_window::window_server::{self, Listed};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The card's page box, as `alo-renderer` lays it out.
const PAGE: (u32, u32) = alo_renderer::card::PAGE_SIZE;

/// The page: a box turning, so that it always has a frame to stream; a
/// link to a long address, whose `mouseover` it tells; and a button asking
/// where the person is, whose answer it tells.
const CARD_PAGE: &str = "<!doctype html><title>Bubbles</title>\
    <style>body{margin:0;font:20px sans-serif}\
      #turning{position:absolute;left:600px;top:20px;width:40px;height:40px;\
        background:#e76f51;animation:turn 1s linear infinite}\
      @keyframes turn{to{transform:rotate(360deg)}}\
      a,button{position:absolute;left:40px}</style>\
    <div id=turning></div>\
    <a id=link style='top:40px' \
      href='/a-long-address-of-somewhere-else-on-this-test-server/with/more/of/it'>A link</a>\
    <button id=where style='top:360px'>Where</button>\
    <script>const said = (what) => fetch('/said/' + what, {cache: 'no-store'});\
      document.getElementById('link').addEventListener('mouseover', () => said('over'));\
      document.getElementById('where').addEventListener('click', () =>\
        navigator.geolocation.getCurrentPosition(() => said('placed'),\
          (why) => said('unplaced-' + why.code)));\
      said('ready');</script>";

/// Over the link's words, in the page's CSS pixels.
const LINK_AT: (f64, f64) = (60.0, 50.0);

/// Over the button, in the page's CSS pixels.
const WHERE_AT: (f64, f64) = (60.0, 370.0);

/// How long the window server is read after each input: the bubble was
/// listed within 0.4 s and grew at 1.6 s, the prompt within 0.4 s.
const WATCHED: Duration = Duration::from_millis(2400);

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

/// Wait until the page has said something beginning `wanted`, and say it.
fn heard(said: &Said, wanted: &str) -> Result<String, String> {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        let so_far = said.lock().map(|said| said.clone()).unwrap_or_default();
        if let Some(found) = so_far.iter().find(|each| each.starts_with(wanted)) {
            return Ok(found.clone());
        }
        if Instant::now() > until {
            return Err(format!("the page never said {wanted}; it said {so_far:?}"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A person's pointer, or button, at the page's point `at`, as the
/// conversation's window sends it.
fn input(orders: &Orders, went: Went, (x, y): (f64, f64)) -> Result<(), String> {
    orders
        .send(Order::Card(CardOrder::Input(CardInput::Pointer {
            went,
            x,
            y,
        })))
        .map_err(|_| "the card's thread has gone".to_owned())
}

/// The frames told within `long`, counted.
fn frames_within(told: &Receiver<ToFrame>, long: Duration) -> usize {
    let until = Instant::now() + long;
    let mut frames = 0;
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        match told.recv_timeout(left) {
            Ok(ToFrame::Cast(_)) => frames += 1,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    frames
}

/// What the window server listed of `process` while [`WATCHED`] passed,
/// read every 200 ms, the card's frames drained meanwhile: every listing,
/// and how many frames were told.
fn watched(process: u32, told: &Receiver<ToFrame>) -> Result<(Vec<Vec<Listed>>, usize), String> {
    let until = Instant::now() + WATCHED;
    let mut listings = Vec::new();
    let mut frames = 0;
    while Instant::now() < until {
        frames += frames_within(told, Duration::from_millis(200));
        listings.push(window_server::on_the_screen(process).map_err(|why| why.to_string())?);
    }
    Ok((listings, frames))
}

/// Require of every listing that no window is seen on `displays` and that
/// none is listed but the card's own, numbered `card`.
fn none_beside(
    what: &str,
    listings: &[Vec<Listed>],
    card: u64,
    displays: &[Bounds],
) -> Result<(), String> {
    for listed in listings {
        let seen: Vec<&Listed> = listed
            .iter()
            .filter(|window| window_server::seen(window, displays))
            .collect();
        if !seen.is_empty() {
            return Err(format!(
                "{what}: seen on a display: {seen:?}; the displays {displays:?}"
            ));
        }
        if listed.iter().any(|window| window.number != card) {
            return Err(format!(
                "{what}: a window beside the card's own: {listed:?}"
            ));
        }
    }
    Ok(())
}

#[test]
fn no_window_of_a_parked_cards_chromium_is_seen_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("808")? else {
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
        Scale::TWO,
        AgentSites::Every,
        Box::new(move |news| tell.send(news).is_ok()),
    )
    .map_err(|why| format!("no card in Chromium: {why}"))?;
    let process = serving.process();
    let orders = serving.orders();
    orders
        .send(Order::Card(CardOrder::Stream {
            width: PAGE.0 * 2,
            height: PAGE.1 * 2,
            page: PAGE,
            scale: Scale::TWO,
        }))
        .map_err(|_| "the card's thread has gone")?;
    heard(&said, "ready")?;
    let displays: Vec<Bounds> = window_server::displays()
        .map_err(|why| why.to_string())?
        .iter()
        .map(|display| display.bounds)
        .collect();
    let (parked, _) = watched(process, &told)?;
    let card = match parked.last().map(Vec::as_slice) {
        Some([only]) => only.number,
        other => return Err(format!("not one window parked: {other:?}")),
    };
    none_beside("parked", &parked, card, &displays)?;

    // The pointer over the link: Chromium's status bubble, before.
    input(&orders, Went::Hovered, LINK_AT)?;
    heard(&said, "over")?;
    let (over_the_link, frames_over) = watched(process, &told)?;
    none_beside("over a link", &over_the_link, card, &displays)?;

    // A press asking where the person is: Chromium's permission prompt,
    // before; dismissed unshown, the page told it was refused.
    input(&orders, Went::Hovered, WHERE_AT)?;
    input(&orders, Went::Pressed { clicks: 1 }, WHERE_AT)?;
    input(&orders, Went::Released { clicks: 1 }, WHERE_AT)?;
    let (asked, frames_asked) = watched(process, &told)?;
    none_beside("asked where", &asked, card, &displays)?;
    let answer = heard(&said, "unplaced-")?;
    eprintln!(
        "alo's CEF host: parked as {:?}; over a link and asked where, {} and {} readings, \
         none but the card's; the page heard {answer}; {frames_over} and {frames_asked} \
         frames meanwhile",
        parked.last(),
        over_the_link.len(),
        asked.len()
    );
    assert!(
        frames_over >= 5 && frames_asked >= 5,
        "the card did not stream: {frames_over} and {frames_asked} frames"
    );

    orders
        .send(Order::CloseEverything)
        .map_err(|_| "the card's thread has gone")?;
    drop(orders);
    serving
        .records()
        .ok_or("the card's thread did not finish")?;
    Ok(())
}
