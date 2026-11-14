/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 811: a link's address on an expanded card, in a strip of
//! alo's own at its window's bottom left — Chromium's own status bubble
//! being off for a card's life (queue item 808).
//!
//! Through alo's CEF host: a card opened, parked and streamed; the pointer
//! over a link while it is parked — **no window beside the card's**, as
//! 808 holds; then *Expand*, and the pointer over the link again: in a
//! picture of the expanded card's window, **the strip's paper at its
//! bottom left corner**, 24 points tall and — the address long — a third
//! of the window's 900 points wide, measured in its pixels; the strip cut
//! from that picture compared with the reference
//! `crates/alo-window/tests/references/link-strip.png`; and the pointer
//! moved off the link: **no strip**.
//!
//! Through the machine's Chrome the status bubble is Chrome's own, and
//! nothing of alo's is drawn: not tested.
//!
//! Never the real network: the page is `http://127.0.0.1:PORT/card.html`.

#![cfg(target_os = "macos")]

mod carried;

use alo_paint::Canvas;
use alo_paint::encode::{picture_from_png, to_png};
use alo_value::Rgba;
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

/// Where the expanded card's window is put, and its page's size.
const EXPANDED: ((i32, i32), (u32, u32)) = ((120, 80), (900, 600));

/// The page: a box turning, so it always has a frame to stream, and a
/// link to a long address — absolute, so the strip's words are the same at
/// every port, and never followed: only hovered.
const CARD_PAGE: &str = "<!doctype html><title>Strip</title>\
    <style>body{margin:0;font:20px/30px sans-serif}\
      #turning{position:absolute;left:600px;top:20px;width:40px;height:40px;\
        background:#e76f51;animation:turn 1s linear infinite}\
      @keyframes turn{to{transform:rotate(360deg)}}\
      #link{position:absolute;left:40px;top:120px}</style>\
    <div id=turning></div>\
    <a id=link href='https://example.com/a-long-address/of-somewhere-else/never-followed.html'>A link</a>\
    <script>fetch('/said/ready', {cache: 'no-store'});</script>";
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

/// The pointer at `at`, no button held.
fn hover(orders: &Orders, (x, y): (f64, f64)) -> Result<(), String> {
    orders
        .send(Order::Card(CardOrder::Input(CardInput::Pointer {
            went: Went::Hovered,
            x,
            y,
        })))
        .map_err(|_| "the card's thread has gone".to_owned())
}

/// The pointer moved to `away`, and then — once the card's thread has
/// had time to send that move — to `at`: two moves waiting together are
/// one, the newest (`chrome_card::only_the_newest_moves`), so `at` sent
/// straight after `away` can reach the page as no move at all, the pointer
/// never having left it (queue item 986).
fn hover_again(
    orders: &Orders,
    told: &Receiver<ToFrame>,
    (away, at): ((f64, f64), (f64, f64)),
) -> Result<(), String> {
    hover(orders, away)?;
    drain(told, Duration::from_millis(250));
    hover(orders, at)
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

/// The windows of `process` the window server lists after `long`, the
/// card's news drained meanwhile.
fn listed_after(
    process: u32,
    told: &Receiver<ToFrame>,
    long: Duration,
) -> Result<Vec<Listed>, String> {
    drain(told, long);
    window_server::on_the_screen(process).map_err(|why| why.to_string())
}

/// The window of `listed` that is not `card`'s, if there is one only.
fn beside(listed: &[Listed], card: u64) -> Result<Option<Listed>, String> {
    let others: Vec<&Listed> = listed.iter().filter(|each| each.number != card).collect();
    match others.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(**one)),
        more => Err(format!("more than one window beside the card's: {more:?}")),
    }
}

/// How long the strip is looked for, or looked at going: long enough for a
/// slow machine, and said when it runs out (queue item 986).
const LOOKED_FOR: Duration = Duration::from_secs(15);

/// Pictures of the window `card`, the card's news drained between them,
/// until one's strip is `wanted` — there ([`Some`]) or gone ([`None`]) —
/// within [`LOOKED_FOR`]: that picture and its strip. Before each picture
/// the pointer is moved to `away` and then to `at` ([`hover_again`]), so
/// that a move the page took before it was ready is made again, never
/// waited on.
fn until_strip(
    told: &Receiver<ToFrame>,
    orders: &Orders,
    (card, window): (u64, Bounds),
    (away, at): ((f64, f64), (f64, f64)),
    wanted: bool,
) -> Result<(Canvas, Option<Bounds>), String> {
    let started = Instant::now();
    loop {
        hover_again(orders, told, (away, at))?;
        drain(told, Duration::from_millis(300));
        let picture = picture_of(card)?;
        let plate = plate_in(&picture, window);
        if plate.is_some() == wanted {
            return Ok((picture, plate));
        }
        if started.elapsed() > LOOKED_FOR {
            return Err(format!(
                "{} after {:?} of looking at the expanded window's bottom left",
                if wanted {
                    "no strip"
                } else {
                    "the strip still there"
                },
                started.elapsed()
            ));
        }
        drain(told, Duration::from_millis(200));
    }
}

/// The interface's paper, `#f4f1ec`, as the display's colours show it:
/// read (243, 241, 236) in a capture of this Mac's Retina display.
const PAPER: (u8, u8, u8) = (0xf4, 0xf1, 0xec);

/// A picture of the window numbered `window`, as `screencapture` takes it:
/// its own pixels, title bar and all, without its shadow.
fn picture_of(window: u64) -> Result<Canvas, String> {
    let taken = std::env::temp_dir().join(format!("alo-811-strip-{}.png", std::process::id()));
    let done = std::process::Command::new("screencapture")
        .args(["-x", "-o", "-l", &window.to_string()])
        .arg(&taken)
        .status()
        .map_err(|why| format!("screencapture did not start: {why}"))?;
    if !done.success() {
        return Err(format!("screencapture failed: {done}"));
    }
    let bytes = std::fs::read(&taken).map_err(|why| why.to_string())?;
    let _ = std::fs::remove_file(&taken);
    picture_from_png(&bytes).map_err(|why| format!("an unreadable capture: {why:?}"))
}

/// The pixel of `picture` at `x`, `y`, as eight-bit channels.
fn pixel(picture: &Canvas, x: u32, y: u32) -> Option<(u8, u8, u8, u8)> {
    let at = usize::try_from(y * picture.width() + x).ok()?;
    picture.pixels().get(at).map(|each| each.to_rgba8())
}

/// Whether `pixel` is the paper, within the display's rounding.
fn paper(pixel: Option<(u8, u8, u8, u8)>) -> bool {
    pixel.is_some_and(|(red, green, blue, alpha)| {
        alpha == 255
            && red.abs_diff(PAPER.0) <= 2
            && green.abs_diff(PAPER.1) <= 2
            && blue.abs_diff(PAPER.2) <= 2
    })
}

/// The strip's plate in `picture` of the window at `window`, in points:
/// paper measured up six points in from the left — from six points above
/// the foot, past the window's rounded corner — and across two points
/// above the foot, under the words; or [`None`] when no paper is there.
fn plate_in(picture: &Canvas, window: Bounds) -> Option<Bounds> {
    let scale = picture.width() / u32::try_from(window.width).ok()?.max(1);
    let foot = picture.height().checked_sub(2 * scale)?;
    let up = (0..picture.height())
        .rev()
        .skip(usize::try_from(6 * scale).ok()?)
        .take_while(|y| paper(pixel(picture, 6 * scale, *y)))
        .count();
    let across = (20 * scale..picture.width())
        .take_while(|x| paper(pixel(picture, *x, foot)))
        .count();
    if up == 0 || across == 0 {
        return None;
    }
    let points = |pixels: usize, more: u32| {
        i64::try_from(pixels)
            .ok()
            .map(|pixels| (pixels + i64::from(more)) / i64::from(scale))
    };
    Some(Bounds {
        left: 0,
        top: window.height - points(up, 6 * scale)?,
        width: points(across, 20 * scale)?,
        height: points(up, 6 * scale)?,
    })
}

/// The strip `plate`, cut from `picture` of the window at `window`,
/// compared with the committed reference `name` pixel for pixel — or
/// written in its place when `ALO_UPDATE_REFERENCES` is set.
fn holds(picture: &Canvas, window: Bounds, plate: Bounds, name: &str) -> Result<(), String> {
    let scale = picture.width()
        / u32::try_from(window.width)
            .map_err(|why| why.to_string())?
            .max(1);
    let width = u32::try_from(plate.width).map_err(|why| why.to_string())? * scale;
    let height = u32::try_from(plate.height).map_err(|why| why.to_string())? * scale;
    let top = picture.height().saturating_sub(height);
    let mut cut = Canvas::new(width, height, Rgba::from_rgba8(0, 0, 0, 0));
    for y in 0..height {
        for x in 0..width {
            if let Some((red, green, blue, alpha)) = pixel(picture, x, top + y) {
                cut.blend(x, y, Rgba::from_rgba8(red, green, blue, alpha), 255);
            }
        }
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name);
    let drawn = to_png(&cut).map_err(|why| format!("{name} not encoded: {why:?}"))?;
    // Compared as written: a picture with clear pixels in it — the
    // window's rounded corner — is the same after a PNG's round trip.
    let cut = picture_from_png(&drawn).map_err(|why| format!("{name}: {why:?}"))?;
    if std::env::var_os("ALO_UPDATE_REFERENCES").is_some() {
        return std::fs::write(&path, &drawn)
            .map_err(|why| format!("{} could not be written: {why}", path.display()));
    }
    let committed = std::fs::read(&path).map_err(|why| {
        format!(
            "{} is not there ({why}); ALO_UPDATE_REFERENCES=1 writes it",
            path.display()
        )
    })?;
    let expected =
        picture_from_png(&committed).map_err(|why| format!("{name} is not a picture: {why:?}"))?;
    if (cut.width(), cut.height()) != (expected.width(), expected.height()) {
        return Err(format!(
            "{name}: the strip is {}×{}, and the reference {}×{}",
            cut.width(),
            cut.height(),
            expected.width(),
            expected.height()
        ));
    }
    let moved = cut
        .pixels()
        .iter()
        .zip(expected.pixels())
        .filter(|(drawn, committed)| drawn.to_rgba8() != committed.to_rgba8())
        .count();
    if moved == 0 {
        Ok(())
    } else {
        Err(format!(
            "{name}: {moved} pixels moved. If this is intended, ALO_UPDATE_REFERENCES=1 \
             rewrites the reference"
        ))
    }
}

#[test]
fn an_expanded_cards_link_is_shown_in_alos_own_strip_through_alos_cef_host() -> Result<(), String> {
    let Some(host) = carried::host("811")? else {
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
    let parked = listed_after(process, &told, Duration::from_millis(800))?;
    let card = match parked.as_slice() {
        [only] => only.number,
        other => return Err(format!("not one window parked: {other:?}")),
    };
    hover(&orders, (60.0, 135.0))?;
    let over_parked = listed_after(process, &told, Duration::from_millis(1200))?;
    assert_eq!(
        beside(&over_parked, card)?,
        None,
        "a strip beside a parked card"
    );

    let ((left, top), (width, height)) = EXPANDED;
    orders
        .send(Order::Card(CardOrder::Expand {
            at: (left, top),
            size: (width, height),
        }))
        .map_err(|_| "the card's thread has gone")?;
    drain(&told, Duration::from_millis(2500));
    hover_again(&orders, &told, ((300.0, 400.0), (60.0, 135.0)))?;
    // The card's window, waited for until the window server lists it at
    // its expanded width.
    let started = Instant::now();
    let window = loop {
        let over = listed_after(process, &told, Duration::from_millis(200))?;
        let expanded = over
            .iter()
            .find(|each| each.number == card)
            .and_then(|each| each.bounds)
            .filter(|bounds| bounds.width == i64::from(width));
        if let Some(window) = expanded {
            break window;
        }
        if started.elapsed() > LOOKED_FOR {
            return Err(format!(
                "the card's window not listed {width} wide after {:?}: {over:?}",
                started.elapsed()
            ));
        }
    };
    let (over_picture, plate) = until_strip(
        &told,
        &orders,
        (card, window),
        ((300.0, 400.0), (60.0, 135.0)),
        true,
    )?;
    let plate = plate.ok_or("no strip at the expanded window's bottom left")?;
    eprintln!("alo's CEF host: the expanded window {window:?}; its strip {plate:?} in points");
    assert_eq!(plate.height, 24, "as tall as the strip: {plate:?}");
    assert_eq!(
        plate.width, 300,
        "a long address cut at a third of the window's 900: {plate:?}"
    );
    holds(&over_picture, window, plate, "link-strip.png")?;

    hover(&orders, (500.0, 400.0))?;
    let (_, off) = until_strip(
        &told,
        &orders,
        (card, window),
        ((60.0, 135.0), (500.0, 400.0)),
        false,
    )?;
    assert_eq!(off, None, "the strip stays off a link");

    orders
        .send(Order::CloseEverything)
        .map_err(|_| "the card's thread has gone")?;
    drop(orders);
    serving
        .records()
        .ok_or("the card's thread did not finish")?;
    Ok(())
}
