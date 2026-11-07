/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 810: a person's press on a closed list on a card's page —
//! the normal dropdown, never a menu at a display's edge.
//!
//! Through alo's CEF host, the card parked at one and streaming: a person's
//! press on the page's `<select>` is held — the conversation is told
//! (`ToFrame::ListPressed`) and, for a second, **no window of the host's
//! but the card's own** is listed; its release is held with it, and the
//! page hears no `mousedown`. Then *Expand*, as the conversation asks on
//! hearing it: **the list's menu**, a window of the host's at the menu's
//! layer, listed over the expanded window — where Chrome's own opens it —
//! and closed with Escape, choosing nothing.
//!
//! Through the machine's Chrome the same road is taken (the press held,
//! *Expand*, the press again): not tested here, since Chrome's parked
//! window is not unseen.
//!
//! Never the real network: the page is `http://127.0.0.1:PORT/card.html`.

#![cfg(target_os = "macos")]

mod carried;

use alo_window::agent_sites::AgentSites;
use alo_window::card_input::{CardInput, Went};
use alo_window::chrome_menu::{self, MENU_LAYER};
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

/// The page: a box turning, so it always has a frame to stream, and one
/// closed list; it tells each `mousedown` and `change` it hears.
const CARD_PAGE: &str = "<!doctype html><title>List</title>\
    <style>body{margin:0;font:20px sans-serif}\
      #turning{position:absolute;left:600px;top:20px;width:40px;height:40px;\
        background:#e76f51;animation:turn 1s linear infinite}\
      @keyframes turn{to{transform:rotate(360deg)}}\
      select{position:absolute;left:40px;top:40px;font:20px sans-serif}</style>\
    <div id=turning></div>\
    <select id=fruit><option>Apples<option selected>Bananas<option>Cherries</select>\
    <script>const said = (what) => fetch('/said/' + what, {cache: 'no-store'});\
      const fruit = document.getElementById('fruit');\
      fruit.addEventListener('mousedown', () => said('down'));\
      fruit.addEventListener('change', () => said('change-' + fruit.value));\
      said('ready');</script>";

/// Over the list, in the page's CSS pixels.
const LIST_AT: (f64, f64) = (60.0, 52.0);

/// Where the expanded card's window is put, and its page's size.
const EXPANDED: ((i32, i32), (u32, u32)) = ((120, 80), (900, 600));

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

/// Send the card a person's pointer `went` at `at`.
fn pointer(orders: &Orders, went: Went, (x, y): (f64, f64)) -> Result<(), String> {
    orders
        .send(Order::Card(CardOrder::Input(CardInput::Pointer {
            went,
            x,
            y,
        })))
        .map_err(|_| "the card's thread has gone".to_owned())
}

/// Whether the card's thread told, within `long`, that a list was pressed.
fn list_pressed_within(told: &Receiver<ToFrame>, long: Duration) -> bool {
    let until = Instant::now() + long;
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        match told.recv_timeout(left) {
            Ok(ToFrame::ListPressed) => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    false
}

/// Drain what the card's thread told for `long`.
fn drain(told: &Receiver<ToFrame>, long: Duration) {
    let until = Instant::now() + long;
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        if told.recv_timeout(left).is_err() {
            break;
        }
    }
}

#[test]
fn a_closed_list_pressed_on_a_card_opens_its_menu_once_expanded_through_alos_cef_host()
-> Result<(), String> {
    let Some(host) = carried::host("810")? else {
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
    let process = serving.process();
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
    drain(&told, Duration::from_millis(500));
    let parked: Vec<Listed> =
        window_server::on_the_screen(process).map_err(|why| why.to_string())?;
    let [card] = parked.as_slice() else {
        return Err(format!("not one window parked: {parked:?}"));
    };

    // The press held, and told; its release held too.
    pointer(&orders, Went::Pressed { clicks: 1 }, LIST_AT)?;
    assert!(
        list_pressed_within(&told, Duration::from_secs(5)),
        "the list's press was not told"
    );
    pointer(&orders, Went::Released { clicks: 1 }, LIST_AT)?;
    drain(&told, Duration::from_secs(1));
    let held = window_server::on_the_screen(process).map_err(|why| why.to_string())?;
    assert!(
        held.iter().all(|window| window.number == card.number),
        "a window beside the parked card: {held:?}"
    );
    let so_far = said.lock().map(|said| said.clone()).unwrap_or_default();
    assert!(
        !so_far.iter().any(|each| each == "down"),
        "the press reached the page: {so_far:?}"
    );

    // Expanded: the press made again there, and the list's own menu over
    // the expanded window.
    let ((left, top), (width, height)) = EXPANDED;
    orders
        .send(Order::Card(CardOrder::Expand {
            at: (left, top),
            size: (width, height),
        }))
        .map_err(|_| "the card's thread has gone")?;
    heard(&said, "down")?;
    let until = Instant::now() + Duration::from_secs(5);
    let menu = loop {
        let listed = window_server::on_the_screen(process).map_err(|why| why.to_string())?;
        if let Some(menu) = listed.iter().find(|window| window.layer == MENU_LAYER) {
            break *menu;
        }
        if Instant::now() > until {
            return Err(format!("no menu once expanded: {listed:?}"));
        }
        drain(&told, Duration::from_millis(100));
    };
    let at = menu.bounds.ok_or("the menu at a fraction of a point")?;
    eprintln!("alo's CEF host: the list's menu once expanded {menu:?}");
    let window_right = i64::from(left) + i64::from(width);
    let window_bottom = i64::from(top) + i64::from(height) + 32;
    assert!(
        at.left >= i64::from(left)
            && at.left < window_right
            && at.top >= i64::from(top)
            && at.top < window_bottom,
        "the menu is not over the expanded window: {at:?}"
    );
    assert_eq!(menu.alpha, 255, "seen");
    let closed = chrome_menu::close_any(process, &parked).map_err(|why| format!("{why:?}"))?;
    eprintln!("alo's CEF host: Escape posted to the menu: {closed:?}");
    drain(&told, Duration::from_millis(800));
    let so_far = said.lock().map(|said| said.clone()).unwrap_or_default();
    assert!(
        !so_far.iter().any(|each| each.starts_with("change-")),
        "a choice nobody made: {so_far:?}"
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
