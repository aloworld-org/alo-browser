/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 713, ADR 0075: the address chosen from the conversation's one
//! box, opened in the machine's Chrome — since queue item 717 (ADR 0076 B)
//! as a card in the conversation, drawn from Chrome's frames.
//!
//! A person's keys, read as the conversation's window reads them, type an
//! address of a server of this test's own into the box a press gave the
//! keyboard, and Return decides to go there (item 701). The address is
//! handed to the thread the window hands it to
//! ([`alo_window::conversation_chrome::ChromeOpener`]), which opens it in a
//! shown Chrome — `alo --chrome`'s path, with no frame of ours — and tells
//! what became of it: opening, then opened, in the first screen's words.
//! Then, each asserted:
//!
//! - Chrome's window is parked off every display, by Chrome's word, and the
//!   page's frames reach the window as PNGs; *expand* puts the same tab's
//!   window where it is asked, by Chrome's word and, on macOS, the window
//!   server's, its stream stopped and Chrome saying so; the server was asked
//!   for the page and nothing else a person typed.
//! - A second address chosen replaces the first: its Chrome is closed and
//!   its profile removed before the second is opened.
//! - The conversation's window closing closes the Chrome showing a page,
//!   waits for it and removes its profile.
//! - Chrome's window going — its Chrome killed, here, as a person's ⌘Q
//!   would end it — is said, and the profile removed.
//!
//! Never the real network: the address is `http://127.0.0.1:PORT/…`.

use alo_window::beside::Chromium;
use alo_window::conversation::{Conversation, Pressed};
use alo_window::conversation_box::Typed;
use alo_window::conversation_chrome::{ChromeOpener, Placing, Streamed, Went};
use alo_window::keys::{Key, Press};
use alo_window::scale::Scale;
use alo_window::typing::edit;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long Chrome is waited for to open a page, or to be said gone: past
/// the thirty seconds Chrome itself is given to answer.
const AT_MOST: Duration = Duration::from_secs(90);

/// Where Chrome's window is put when the card is expanded, and the page's
/// size.
const PLACING: Placing = Placing {
    at: (120, 80),
    size: (900, 600),
};

/// The pages served: a heading each.
fn page(path: &str) -> Option<&'static str> {
    match path {
        "/one.html" => Some("<!doctype html><title>One</title><h1>One</h1>"),
        "/two.html" => Some("<!doctype html><title>Two</title><h1>Two</h1>"),
        _ => None,
    }
}

/// Every path a server was asked for, in order.
type Heard = Arc<Mutex<Vec<String>>>;

/// A server on this machine answering [`page`]: its port, and every path
/// it was asked for.
fn serve() -> Result<(u16, Heard), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    let asked_for = Arc::new(Mutex::new(Vec::new()));
    let heard = Arc::clone(&asked_for);
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let heard = Arc::clone(&heard);
            std::thread::spawn(move || {
                if socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .is_err()
                {
                    return;
                }
                let mut asked = Vec::new();
                let mut block = [0u8; 1024];
                while let Ok(got) = socket.read(&mut block) {
                    asked.extend_from_slice(block.get(..got).unwrap_or_default());
                    if got == 0 || asked.windows(4).any(|end| end == b"\r\n\r\n") {
                        break;
                    }
                }
                let asked = String::from_utf8_lossy(&asked);
                let Some(path) = asked.split(' ').nth(1).filter(|path| !path.is_empty()) else {
                    // A connection Chrome opened ahead of need asked nothing.
                    return;
                };
                if let Ok(mut heard) = heard.lock() {
                    heard.push(path.to_owned());
                }
                let (status, body) = match page(path) {
                    Some(body) => ("200 OK", body),
                    None => ("404 Not Found", ""),
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
    Ok((port, asked_for))
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

/// What the opener tells next, within [`AT_MOST`], passing over the
/// frames it streams.
fn next(told: &Receiver<Went>) -> Result<Went, String> {
    loop {
        match told.recv_timeout(AT_MOST) {
            Ok(Went::Framed(_)) => {}
            Ok(went) => return Ok(went),
            Err(why) => return Err(format!("the opener said nothing more: {why}")),
        }
    }
}

/// The next frame the opener tells of, within [`AT_MOST`]: a PNG's bytes.
fn framed(told: &Receiver<Went>) -> Result<Vec<u8>, String> {
    loop {
        match told.recv_timeout(AT_MOST) {
            Ok(Went::Framed(png)) => return Ok(png),
            Ok(_) => {}
            Err(why) => return Err(format!("no frame of the page came: {why}")),
        }
    }
}

/// Whether the process `id` runs.
fn runs(id: u32) -> Result<bool, String> {
    std::process::Command::new("kill")
        .args(["-0", &id.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .map_err(|why| why.to_string())
}

/// Whether `profile` is gone and Chrome `process` has ended.
fn ended(process: u32, profile: &Path) -> Result<bool, String> {
    Ok(!runs(process)? && !profile.exists())
}

/// The address Return on `typed` decided to go to, opened by `opener`,
/// and what was said of it: the opened Chrome's process and profile.
fn go(
    conversation: &mut Conversation,
    typed: &str,
    opener: &ChromeOpener,
    told: &Receiver<Went>,
) -> Result<(u32, std::path::PathBuf), String> {
    let chose = type_and_return(conversation, typed);
    if !matches!(chose, Typed::Chose(_)) {
        return Err(format!("Return on {typed} chose nothing: {chose:?}"));
    }
    let url = conversation
        .going()
        .cloned()
        .ok_or_else(|| format!("{typed} is not an address to go to"))?;
    if url.to_string() != typed {
        return Err(format!("{typed} was read as {url}"));
    }
    opener.open(url.clone(), Streamed::at(Scale::ONE));
    let opening = next(told)?;
    if opening != Went::Opening(url.clone())
        || opening.said() != format!("opening {typed} in Chrome, to be shown in a card")
    {
        return Err(format!("not said to be opening: {opening:?}"));
    }
    let (went_to, shown) = match next(told)? {
        Went::Opened { url, shown } => (url, shown),
        other => return Err(format!("{typed} was not opened: {}", other.said())),
    };
    if went_to != url {
        return Err(format!("{went_to} was opened, not {url}"));
    }
    if !(shown.by.starts_with("Chrome/") && shown.by.ends_with(", driven by this browser")) {
        return Err(format!("not said to be drawn by Chrome: {}", shown.by));
    }
    let window = shown.window.ok_or("a shown Chrome's window was not read")?;
    // Parked left of every display while the page is a card.
    if window.left + window.width > 0 {
        return Err(format!("Chrome's window is not parked: {window:?}"));
    }
    let png = framed(told)?;
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("a frame of the page is not a PNG".to_owned());
    }
    if !runs(shown.process)? || !shown.profile.is_dir() {
        return Err(format!(
            "Chrome {} does not run with its profile at {}",
            shown.process,
            shown.profile.display()
        ));
    }
    Ok((shown.process, shown.profile))
}

/// The card expanded: its stream stopped, Chrome saying so, and the same
/// tab's window over the conversation where it was asked.
fn expand(opener: &ChromeOpener, told: &Receiver<Went>, process: u32) -> Result<(), String> {
    opener.expand(PLACING);
    match next(told)? {
        Went::Unstreamed(uncast) if uncast.answered && uncast.frames > 0 => {}
        other => return Err(format!("the stream did not stop as it expanded: {other:?}")),
    }
    #[cfg(target_os = "macos")]
    on_the_screen_at(
        process,
        alo_window::chrome_protocol::Bounds {
            left: 120,
            top: 80,
            width: 900,
            // The page's 600 and Chrome's title bar, as fitted.
            height: 0,
        },
    )?;
    #[cfg(not(target_os = "macos"))]
    let _ = process;
    Ok(())
}

/// Wait, at most ten seconds, for the window server to list an ordinary
/// window of `process` on the screen with its corner and width at
/// `bounds`'.
#[cfg(target_os = "macos")]
fn on_the_screen_at(
    process: u32,
    bounds: alo_window::chrome_protocol::Bounds,
) -> Result<(), String> {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let listed =
            alo_window::window_server::on_the_screen(process).map_err(|why| why.to_string())?;
        if listed.iter().any(|window| {
            window.layer == 0
                && window.bounds.is_some_and(|at| {
                    (at.left, at.top, at.width) == (bounds.left, bounds.top, bounds.width)
                })
        }) {
            return Ok(());
        }
        if Instant::now() > until {
            return Err(format!(
                "the window server lists no window of Chrome {process} at {bounds:?}: {listed:?}"
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Wait, at most [`AT_MOST`], for `done`.
fn waiting_for(mut done: impl FnMut() -> Result<bool, String>) -> Result<bool, String> {
    let until = Instant::now() + AT_MOST;
    loop {
        if done()? {
            return Ok(true);
        }
        if Instant::now() > until {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// An opener, and what it tells the window.
fn opener() -> Result<(ChromeOpener, Receiver<Went>), String> {
    let (tell, told) = channel();
    let opener = ChromeOpener::start(
        Chromium::Machines,
        Box::new(move |went| tell.send(went).is_ok()),
    )
    .map_err(|why| why.to_string())?;
    Ok((opener, told))
}

#[test]
fn an_address_typed_and_returned_opens_in_chrome_and_a_second_replaces_it() -> Result<(), String> {
    let (port, asked_for) = serve()?;
    let (opener, told) = opener()?;
    let mut conversation = Conversation::default();
    assert!(
        conversation.act(Pressed::TheBox),
        "the box has the keyboard"
    );

    let one = format!("http://127.0.0.1:{port}/one.html");
    let (first, first_profile) = go(&mut conversation, &one, &opener, &told)?;
    expand(&opener, &told, first)?;
    let asked: Vec<String> = asked_for.lock().map_err(|why| why.to_string())?.clone();
    assert_eq!(
        asked.first().map(String::as_str),
        Some("/one.html"),
        "the page was asked for: {asked:?}"
    );
    assert!(
        asked
            .iter()
            .all(|path| path == "/one.html" || path == "/favicon.ico"),
        "nothing else reached the server: {asked:?}"
    );

    // A second address replaces the first: its Chrome is closed, waited for
    // and its profile removed before the second is started.
    // The box was emptied as the first was gone to (queue item 742), and
    // keeps the keyboard.
    assert_eq!(
        conversation
            .start(alo_layout::Size::new(1040.0, 820.0), Scale::ONE)
            .typed,
        ""
    );
    let two = format!("http://127.0.0.1:{port}/two.html");
    let (second, second_profile) = go(&mut conversation, &two, &opener, &told)?;
    assert_ne!(first, second, "a Chrome of its own");
    assert!(
        ended(first, &first_profile)?,
        "the first Chrome {first} has ended and {} is removed",
        first_profile.display()
    );
    assert!(
        asked_for
            .lock()
            .map_err(|why| why.to_string())?
            .iter()
            .any(|path| path == "/two.html")
    );

    // The window closing closes the Chrome showing a page.
    assert!(opener.finish(), "the thread and its Chrome finished");
    assert!(
        ended(second, &second_profile)?,
        "the second Chrome {second} has ended and {} is removed",
        second_profile.display()
    );
    // A frame streamed before it closed may still be waiting; nothing else.
    let said: Vec<Went> = told
        .try_iter()
        .filter(|went| !matches!(went, Went::Framed(_) | Went::Unstreamed(_)))
        .collect();
    assert!(said.is_empty(), "said as the window closed: {said:?}");
    Ok(())
}

#[test]
fn chrome_gone_from_under_the_conversation_is_said() -> Result<(), String> {
    let (port, _) = serve()?;
    let (opener, told) = opener()?;
    let mut conversation = Conversation::default();
    let _ = conversation.act(Pressed::TheBox);
    let one = format!("http://127.0.0.1:{port}/one.html");
    let (process, profile) = go(&mut conversation, &one, &opener, &told)?;

    let killed = std::process::Command::new("kill")
        .arg(process.to_string())
        .status()
        .map_err(|why| why.to_string())?;
    assert!(killed.success(), "Chrome {process} was told to end");
    let closed = next(&told)?;
    assert_eq!(
        closed.said(),
        format!("Chrome's window showing {one} was closed")
    );
    assert!(
        waiting_for(|| ended(process, &profile))?,
        "Chrome {process} ended and {} removed",
        profile.display()
    );
    assert!(opener.finish());
    Ok(())
}
