/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 708: a press on a site's title among a search's results
//! opens that site in the card, the way an address chosen from the box
//! does (717, 718, 742) — through the window's own road: the cursor moved
//! to the title where a person sees it, the main button pressed and
//! released, the first screen's confined renderer claiming the title by its
//! place, the browser process deciding it against what was painted, and the
//! machine's Chrome opening the page as a card.
//!
//! Never the real API, and never the network: the provider and the page are
//! a server on `127.0.0.1`, the provider's answer written by hand.

#![cfg(target_os = "macos")]

use alo_renderer::host::Renderers;
use alo_renderer::results::Showing;
use alo_window::beside::{Chromium, renderer_beside};
use alo_window::brave_search::Provider;
use alo_window::conversation_chrome::{ChromeOpener, Went};
use alo_window::conversation_drawing::{ConversationDrawing, Drawn};
use alo_window::conversation_place::start_viewport;
use alo_window::conversation_search::Searcher;
use alo_window::conversation_window::{ConversationWindow, Heard, Measured};
use alo_window::keys::{Key, Press};
use alo_window::scale::Scale;
use alo_window::search_key::Key as SearchKey;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

/// How long anything is waited for: Chrome starting and loading a page.
const AT_MOST: Duration = Duration::from_secs(90);

/// The design's window at one, in device pixels.
const WINDOW: (u32, u32) = (1280, 820);

/// The box, at one: 400 into the first screen.
const THE_BOX: (f64, f64) = (640.0, 285.0);

/// The first site's title, at one: the panel's 240, then where the
/// renderer's layout assertion of the All tab puts it — 80 across, 192 to
/// 213 down.
const FIRST_TITLE: (f64, f64) = (240.0 + 140.0, 202.5);

/// What the test's provider is given as a key: not a key.
const NOT_A_KEY: &str = "test-key-for-a-local-server";

/// The page the first site is.
const PAGE: &str = "<!doctype html><title>Found</title>\
    <style>body{margin:0;background:#2a9d8f}</style><h1>Found</h1>";

/// A provider and the site it finds, on this machine: its port.
fn serve() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    let answer = format!(
        r#"{{"type":"search","web":{{"results":[
          {{"title":"The page found","url":"http://127.0.0.1:{port}/found.html",
            "description":"Opened in the card.","profile":{{"name":"Here"}}}},
          {{"title":"Another","url":"https://another.example/"}}]}}}}"#
    );
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let answer = answer.clone();
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
                let (status, kind, body) = match head.split(' ').nth(1) {
                    Some("/res/v1/web/search?q=a+page") => {
                        ("200 OK", "application/json", answer.as_str())
                    }
                    Some("/found.html") => ("200 OK", "text/html; charset=utf-8", PAGE),
                    _ => ("404 Not Found", "text/plain", ""),
                };
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
                     Cache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
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

/// The conversation's window at one, searching through the provider on
/// `port`, and how it hears its threads.
fn the_window(port: u16) -> Result<(ConversationWindow, Receiver<Heard>), String> {
    let renderers = Renderers::running(renderer()?, &[]).with_machine(alo_window::fonts::frozen());
    let (tell, heard): (Sender<Heard>, Receiver<Heard>) = channel();
    let (drawn, told) = (tell.clone(), tell.clone());
    let drawing = ConversationDrawing::start(
        renderers,
        Box::new(move |d| drawn.send(Heard::Drawn(d)).is_ok()),
    )
    .map_err(|why| format!("no drawing thread: {why}"))?;
    let chrome = ChromeOpener::start(
        Chromium::Machines,
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
    let mut window =
        ConversationWindow::new(Some(Scale::ONE), drawing, chrome).with_search(searcher);
    window.measured_as(Measured {
        size: WINDOW,
        scale_factor: 1.0,
        outer: Some((100, 100)),
    });
    window.focused(true);
    window.ask_drawn();
    Ok((window, heard))
}

/// Give `window` what its threads told it, as its event loop does, until
/// `done` holds of it.
fn until(
    window: &mut ConversationWindow,
    heard: &Receiver<Heard>,
    what: &str,
    done: &dyn Fn(&ConversationWindow) -> bool,
) -> Result<(), String> {
    let ends = Instant::now() + AT_MOST;
    let mut said = Vec::new();
    while !done(window) {
        let left = ends
            .checked_duration_since(Instant::now())
            .ok_or_else(|| format!("{what} did not happen; the window said {said:?}"))?;
        let Ok(news) = heard.recv_timeout(left) else {
            continue;
        };
        match &news {
            Heard::Drawn(Drawn::Said(part, why)) => return Err(format!("{part} said {why}")),
            Heard::Chrome(Went::Unopened { why, .. }) => {
                return Err(format!("the page did not open: {why}"));
            }
            _ => {}
        }
        window.heard(news);
        said.extend(window.take_said());
        let _ = window.take_redraw();
        let _ = window.take_front();
    }
    Ok(())
}

/// A person's click at `at`, in the window's device pixels.
fn click(window: &mut ConversationWindow, at: (f64, f64)) {
    window.moved(at);
    window.main_button_pressed();
    window.main_button_released();
}

#[test]
fn a_search_results_title_pressed_opens_its_page_in_the_card_through_chrome() -> Result<(), String>
{
    let port = serve()?;
    let (mut window, heard) = the_window(port)?;
    let painted = |window: &ConversationWindow| {
        window
            .scene()
            .is_some_and(|scene| scene.panel.is_some() && scene.start.is_some())
    };
    until(&mut window, &heard, "both parts painted", &painted)?;
    click(&mut window, THE_BOX);
    until(
        &mut window,
        &heard,
        "the box given the keyboard",
        &|window| {
            window
                .conversation()
                .start(start_viewport(WINDOW, Scale::ONE), Scale::ONE)
                .focused
        },
    )?;
    for letter in "a page".chars() {
        let written = letter.to_string();
        window.key(&Press::character(&written), Some(&written));
    }
    window.key(&Press::of(Key::Return), None);
    until(&mut window, &heard, "the sites found", &|window| {
        matches!(
            window.conversation().results().map(|r| &r.showing),
            Some(Showing::Found(found)) if !found.is_empty()
        )
    })?;
    // Pressed only once the first screen painted the sites: a press is
    // decided against what the person saw.
    until(&mut window, &heard, "the sites painted", &|window| {
        window
            .scene()
            .and_then(|scene| scene.start)
            .is_some_and(|frame| {
                let at = usize::try_from((177 * frame.width + 81) * 4).unwrap_or(usize::MAX);
                frame.pixels.get(at..at + 3) != Some(&[0xf4, 0xf1, 0xec][..])
            })
    })?;
    click(&mut window, FIRST_TITLE);
    let page = format!("http://127.0.0.1:{port}/found.html");
    until(&mut window, &heard, "the card's first frame", &|window| {
        window.told().is_some()
            || window
                .conversation()
                .card()
                .is_some_and(|card| card.frame.is_some())
    })?;
    assert_eq!(window.told(), None, "the press on the title was refused");
    let card = window.conversation().card().cloned().ok_or("no card")?;
    assert_eq!(card.url.to_string(), page);
    assert_eq!(
        card.said, "a page",
        "the card answers what was searched for"
    );
    assert!(
        window.conversation().results().is_none(),
        "the card stands in the results' place"
    );
    window.finish();
    match window.unfinished() {
        Some(why) => Err(why.to_owned()),
        None => Ok(()),
    }
}
