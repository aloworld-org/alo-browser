/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A windowless card's view told its size, its scale, its focus and its
//! hiding by alo, and *Expand* placing no window (queue item 979, ADR
//! 0113).
//!
//! alo's CEF host is started with no window at all
//! (`Chrome::start_windowless`, 600 × 400 CSS pixels at one), and its page,
//! from a server of this test's own on `127.0.0.1`, writes what its view
//! is — `innerWidth`, `innerHeight`, `devicePixelRatio`, its visibility and
//! whether it has the focus — into its own text, again on every change and
//! every 50 ms. Measured before the channel was built: the page said
//! `600x400@1 visible focused` (and `@2` started at two) and nothing alo
//! could do over the pipe changed its size or its scale but emulation (ADR
//! 0113, refused for this).
//!
//! The card's own orders are carried out on it, as the thread holding a
//! card carries them out (`chrome_card::carry_out`), its scale had from the
//! host (`Scaled::ByTheHost`):
//!
//! - **Resized**: a stream asked at 800 × 500 tells the host the view, and
//!   the page says `800x500@1`; its frames are 800 × 500 device pixels.
//! - **Rescaled**: a stream at two, `800x500@2`, frames 1600 × 1000.
//! - **Refused**: a view of no size, one past the host's largest side and
//!   one at a scale past its largest are told, then the focus taken away:
//!   once the page says it lost the focus — lines are carried out in their
//!   order — it is still `800x500@2`.
//! - **Focus and hiding**: `unfocused` then `focused`, `hidden` then
//!   `visible`, each told.
//! - ***Expand***: the card expanded to 1000 × 700 is the page `1000x700@2`
//!   and frames 2000 × 1400, and the target still has no window of
//!   Chromium's (`Browser.getWindowForTarget` refused): none was placed.
//!
//! The host then ends on `Browser.close`, status 0. With no CEF
//! distribution named by `CEF_PATH`, it says why on its error output and
//! returns.

#![cfg(target_os = "macos")]

mod carried;

use alo_window::beside::Chromium;
use alo_window::chrome::Chrome;
use alo_window::chrome_card::{self, Fitted, Scaled};
use alo_window::chrome_screencast::png_size;
use alo_window::chrome_tab::Tab;
use alo_window::chrome_tell::ToHost;
use alo_window::chrome_window::{self, Unplaced};
use alo_window::message::CardOrder;
use alo_window::scale::Scale;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

/// The view the host is started at, in CSS pixels.
const SIZE: (u32, u32) = (600, 400);

/// How long the page is waited for to say each change, and a frame of
/// each size.
const AT_MOST: Duration = Duration::from_secs(15);

/// The page: its view, said in its own text whenever it changes and every
/// 50 ms.
const PAGE: &str = "<!doctype html><title>View</title><p id=said>nothing yet</p><script>\
     function say() { document.getElementById('said').textContent = 'view ' + innerWidth \
     + 'x' + innerHeight + '@' + devicePixelRatio + ' ' + document.visibilityState + ' ' \
     + (document.hasFocus() ? 'focused' : 'unfocused'); }\
     for (const event of ['resize', 'visibilitychange', 'focus', 'blur']) \
     addEventListener(event, say);\
     document.addEventListener('visibilitychange', say);\
     setInterval(say, 50); say();</script>";

/// A server on this machine answering every request with [`PAGE`]: its
/// port.
fn serve() -> Option<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").ok()?;
    let port = listener.local_addr().ok()?.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            if socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .is_err()
            {
                continue;
            }
            let mut asked = [0u8; 4096];
            let _ = socket.read(&mut asked);
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                 Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{PAGE}",
                PAGE.len()
            );
            let _ = socket.write_all(reply.as_bytes());
        }
    });
    Some(port)
}

/// What the page says of its view now, or nothing when it has said
/// nothing yet.
fn said(tab: &mut Tab<'_>) -> Result<Option<String>, String> {
    let snapshot = tab.read().map_err(|why| why.to_string())?;
    Ok(snapshot
        .nodes()
        .iter()
        .map(ToString::to_string)
        .find_map(|node| {
            let at = node.find("view ")?;
            let said: String = node.get(at..)?.chars().take_while(|c| *c != '"').collect();
            Some(said)
        }))
}

/// `tab` read until its page says `wanted`, within [`AT_MOST`]: how long
/// that took.
fn until_said(tab: &mut Tab<'_>, wanted: &str) -> Result<Duration, String> {
    let asked = Instant::now();
    let until = asked + AT_MOST;
    loop {
        let now = said(tab)?;
        if now.as_deref() == Some(wanted) {
            return Ok(asked.elapsed());
        }
        if Instant::now() > until {
            return Err(format!(
                "the page never said {wanted:?} in {AT_MOST:?}: it says {now:?}"
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// `tab`'s stream read until a frame `wanted` device pixels comes, within
/// [`AT_MOST`].
fn until_framed(tab: &mut Tab<'_>, wanted: (u32, u32)) -> Result<(), String> {
    let until = Instant::now() + AT_MOST;
    let mut seen = Vec::new();
    loop {
        if let Some(frame) = tab.take_frame().map_err(|why| why.to_string())? {
            let sized = frame.png().ok().as_deref().and_then(png_size);
            if sized == Some(wanted) {
                return Ok(());
            }
            seen.push(sized);
        }
        if Instant::now() > until {
            return Err(format!(
                "no frame of {wanted:?} in {AT_MOST:?}: frames of {seen:?}"
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A stream of the card at `page` CSS pixels and `scale`, as the
/// conversation asks for one.
fn stream(page: (u32, u32), scale: Scale) -> CardOrder {
    CardOrder::Stream {
        width: scale.device(page.0),
        height: scale.device(page.1),
        page,
        scale,
    }
}

#[test]
fn a_windowless_cards_view_is_told_its_size_scale_focus_and_hiding_and_expand_places_nothing() {
    let Some(host) = carried::host("979").unwrap() else {
        return;
    };
    told(&host.chromium()).unwrap();
}

/// The page opened windowless through the host `chromium`, its view told
/// and seen in numbers, and the host ended.
fn told(chromium: &Chromium) -> Result<(), String> {
    let Chromium::Carried(program) = chromium else {
        return Err("only the Chromium alo carries opens a page windowless".to_owned());
    };
    let port = serve().ok_or("no server")?;
    let mut chrome =
        Chrome::start_windowless(program, SIZE, Scale::ONE).map_err(|why| why.to_string())?;
    let address = alo_url::parse(&format!("http://127.0.0.1:{port}/view.html"))
        .map_err(|why| format!("{why:?}"))?;
    {
        let mut tab = Tab::open(&mut chrome, &address, SIZE).map_err(|why| why.to_string())?;
        let mut fitted = Fitted::parked(SIZE, Scaled::ByTheHost(Scale::ONE));
        until_said(&mut tab, "view 600x400@1 visible focused")?;
        chrome_card::carry_out(&mut tab, stream(SIZE, Scale::ONE), None, &mut fitted);
        until_framed(&mut tab, (600, 400))?;

        chrome_card::carry_out(&mut tab, stream((800, 500), Scale::ONE), None, &mut fitted);
        let took = until_said(&mut tab, "view 800x500@1 visible focused")?;
        until_framed(&mut tab, (800, 500))?;
        eprintln!("resized to 800 × 500 at one: said in {took:?}");

        chrome_card::carry_out(&mut tab, stream((800, 500), Scale::TWO), None, &mut fitted);
        let took = until_said(&mut tab, "view 800x500@2 visible focused")?;
        until_framed(&mut tab, (1600, 1000))?;
        eprintln!("rescaled to two: said in {took:?}");
        assert_eq!(fitted.scaled, Scaled::ByTheHost(Scale::TWO));

        refused(&mut tab)?;
        focus_and_hiding(&mut tab)?;

        chrome_card::carry_out(
            &mut tab,
            CardOrder::Expand {
                at: (0, 0),
                size: (1000, 700),
            },
            None,
            &mut fitted,
        );
        let took = until_said(&mut tab, "view 1000x700@2 visible focused")?;
        until_framed(&mut tab, (2000, 1400))?;
        eprintln!("expanded to 1000 × 700 at two: said in {took:?}");
        assert!(!fitted.parked);
        assert_eq!(fitted.page, (1000, 700));
        let window = chrome_window::placed(&mut tab);
        eprintln!("the target's window after Expand: {window:?}");
        assert!(
            matches!(window, Err(Unplaced::Failed(_))),
            "a windowless page has no window of Chromium's, and Expand placed none: {window:?}"
        );
    }
    let ended = chrome.close();
    assert!(
        ended.is_some_and(|status| status.success()),
        "the host did not end in order on Browser.close: {ended:?}"
    );
    Ok(())
}

/// Views out of the host's bounds told, refused: once a line told after
/// them is carried out, the page is the view it was.
fn refused(tab: &mut Tab<'_>) -> Result<(), String> {
    for out_of_bounds in [
        ToHost::View {
            size: (0, 0),
            scale: Scale::TWO,
        },
        ToHost::View {
            size: (20_000, 400),
            scale: Scale::TWO,
        },
        ToHost::View {
            size: (900, 600),
            scale: Scale::whole(8),
        },
    ] {
        tab.tell_host(out_of_bounds)
            .map_err(|why| why.to_string())?;
    }
    tab.tell_host(ToHost::Focused(false))
        .map_err(|why| why.to_string())?;
    until_said(tab, "view 800x500@2 visible unfocused")?;
    eprintln!("three views out of bounds refused, the focus taken after them");
    Ok(())
}

/// The focus given back, and the card hidden and shown, each told and
/// seen by the page.
fn focus_and_hiding(tab: &mut Tab<'_>) -> Result<(), String> {
    for (told, wanted) in [
        (ToHost::Focused(true), "view 800x500@2 visible focused"),
        (ToHost::Hidden(true), "view 800x500@2 hidden focused"),
        (ToHost::Hidden(false), "view 800x500@2 visible focused"),
    ] {
        tab.tell_host(told).map_err(|why| why.to_string())?;
        let took = until_said(tab, wanted)?;
        eprintln!("{told:?}: said {wanted:?} in {took:?}");
    }
    Ok(())
}
