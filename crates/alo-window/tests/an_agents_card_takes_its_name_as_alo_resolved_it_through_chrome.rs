/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 881, ADR 0103 § 7: a page alo's agent opens is loaded from
//! the address the browser process resolved its name to, and Chrome never
//! resolves the name again — through the machine's Chrome and through the
//! Chromium alo carries (alo's CEF host), each the same test.
//!
//! The card is opened as the window opens one for the agent
//! ([`ChromeOpener::open_pinned`]), its name `rebind.invalid` pinned to the
//! address the gate found — here this machine's own server, standing for
//! the public answer, so that nothing reaches a network. A `.invalid` name
//! is one no resolver ever answers (RFC 6761), so it stands for the answer
//! the name gives a moment later: had Chrome asked anything for it, the
//! page could not have opened from the pinned address.
//!
//! Asserted, as the server hears it:
//!
//! 1. **Pinned, the page is asked of the address the gate found**, with its
//!    own name as its `Host`.
//! 2. **Opened as a person's address is — unpinned — Chrome resolves the
//!    name itself**, and cannot open it: what a card the person opens
//!    does, unchanged.
//!
//! The real pointer is never moved and no key is posted to the system.
//! Never the real network.

#![cfg(target_os = "macos")]

mod carried;

use alo_window::beside::Chromium;
use alo_window::chrome_pin::Pinned;
use alo_window::conversation_chrome::{ChromeOpener, Streamed, Went};
use alo_window::scale::Scale;
use std::io::{Read, Write};
use std::net::{IpAddr, TcpListener};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long Chrome is waited for to open a page, or say it could not.
const AT_MOST: Duration = Duration::from_secs(90);

/// The name the agent opened, which no resolver answers.
const NAME: &str = "rebind.invalid";

/// The page.
const PAGE: &str = "<!doctype html><title>Pinned</title><p>pinned";

/// What the server was asked: each request's line and its `Host`.
type Heard = Arc<Mutex<Vec<String>>>;

/// A server on this machine answering every request with [`PAGE`]: its
/// port, and what it was asked. A connection that asks for nothing — one
/// of Chrome's preconnects — is not written down.
fn serve() -> Result<(u16, Heard), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    let heard = Arc::new(Mutex::new(Vec::new()));
    let hearing = Arc::clone(&heard);
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let hearing = Arc::clone(&hearing);
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
                let Some(line) = asked.lines().next().filter(|line| !line.is_empty()) else {
                    return;
                };
                let host = asked
                    .lines()
                    .find_map(|header| {
                        header
                            .split_once(':')
                            .filter(|(name, _)| name.eq_ignore_ascii_case("host"))
                            .map(|(_, value)| value.trim().to_owned())
                    })
                    .unwrap_or_default();
                if let Ok(mut heard) = hearing.lock() {
                    heard.push(format!("{line} | {host}"));
                }
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{PAGE}",
                    PAGE.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Ok((port, heard))
}

/// What became of opening `url` in `chromium` as the window opens a card,
/// pinned as `pinned` says: opened, or why not — and the opener's thread
/// finished either way.
fn opened(
    chromium: Chromium,
    url: &str,
    pinned: Option<Pinned>,
) -> Result<Result<(), String>, String> {
    let (tell, told): (_, Receiver<Went>) = channel();
    let opener = ChromeOpener::start(chromium, Box::new(move |went| tell.send(went).is_ok()))
        .map_err(|why| why.to_string())?;
    let url = alo_url::parse(url).map_err(|why| format!("{why:?}"))?;
    opener.open_pinned(
        url,
        Streamed::at(alo_renderer::card::PAGE_SIZE, Scale::ONE),
        pinned,
    );
    let until = Instant::now() + AT_MOST;
    let came = loop {
        if Instant::now() > until {
            return Err("Chrome neither opened the page nor said why not".to_owned());
        }
        match told.recv_timeout(Duration::from_millis(200)) {
            Ok(Went::Opened { .. }) => break Ok(()),
            Ok(Went::Unopened { why, .. }) => break Err(why),
            Ok(_) | Err(_) => {}
        }
    };
    if !opener.finish() {
        return Err("the opener's thread did not finish".to_owned());
    }
    Ok(came)
}

/// Both conditions, through `chromium`.
fn through(chromium: &Chromium, who: &str) -> Result<(), String> {
    let (port, heard) = serve()?;
    let url = format!("http://{NAME}:{port}/page.html");
    let pinned = Pinned::new(NAME, IpAddr::from([127, 0, 0, 1])).ok_or("a plain name")?;
    // 1. Pinned: asked of the address the gate found, by its own name.
    opened(chromium.clone(), &url, Some(pinned))?
        .map_err(|why| format!("{who}: the pinned page did not open: {why}"))?;
    let asked = heard.lock().map(|heard| heard.clone()).unwrap_or_default();
    assert_eq!(
        asked.first().map(String::as_str),
        Some(format!("GET /page.html HTTP/1.1 | {NAME}:{port}").as_str()),
        "{who}: what the server was asked: {asked:?}"
    );
    // 2. Unpinned, as a person's address opens: Chrome asks for the name
    // itself, and nothing answers it.
    if let Ok(mut heard) = heard.lock() {
        heard.clear();
    }
    let unpinned = opened(chromium.clone(), &url, None)?;
    assert!(
        unpinned
            .as_ref()
            .is_err_and(|why| why.contains("ERR_NAME_NOT_RESOLVED")),
        "{who}: opened unpinned: {unpinned:?}"
    );
    let asked = heard.lock().map(|heard| heard.clone()).unwrap_or_default();
    assert!(asked.is_empty(), "{who}: asked unpinned: {asked:?}");
    eprintln!("{who}: pinned, the page was asked of the address found; unpinned, {unpinned:?}");
    Ok(())
}

#[test]
fn an_agents_card_is_loaded_from_the_address_alo_resolved_through_chrome() -> Result<(), String> {
    through(&Chromium::Machines, "Chrome")
}

#[test]
fn an_agents_card_is_loaded_from_the_address_alo_resolved_through_alos_cef_host()
-> Result<(), String> {
    let Some(host) = carried::host("881")? else {
        return Ok(());
    };
    through(&host.chromium(), "alo's CEF host")
}
