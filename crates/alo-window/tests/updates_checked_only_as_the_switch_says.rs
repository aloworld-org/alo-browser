/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ADR 0116 § 6, *Nothing leaves* (queue item 721): with *Settings ›
//! Updates*' switch off, an idle alo makes no update request — the thread
//! says it waits for the window's word alone, and a feed of this test's own
//! on `127.0.0.1` saw nothing — until *Check now* is pressed, which is
//! exactly the two `GET`s; with it on, the check at launch is exactly the
//! two `GET`s, carrying no cookie and no query, saying `alo-update`, and
//! the next is a day later.
//!
//! The feed answers what no key signed, so each check is refused after its
//! two requests and nothing is downloaded. Every wait is bounded and waits
//! *until* the thread says something (queue item 986).

use alo_net::cause::{Cause, Identities};
use alo_net::{Pool, Trust};
use alo_renderer::settings_updates::Automatic;
use alo_update::address::Address;
use alo_update::kept::Kept;
use alo_update::signature::{Key, Keys};
use alo_update::{Found, Instant, Updater, Version, test_key};
use alo_window::conversation_updates::{EVERY, Told, Updates};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The longest any one thing here is waited for.
const PATIENCE: Duration = Duration::from_secs(20);

/// Every request's head a feed was sent.
type Heads = Arc<Mutex<Vec<String>>>;

/// A feed answering anything with a few words, keeping every request's
/// head: its port and the heads.
fn feed() -> Result<(u16, Heads), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    let heads = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&heads);
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else { return };
            let _ = socket.set_read_timeout(Some(Duration::from_secs(5)));
            let mut seen = Vec::new();
            let mut byte = [0u8; 1];
            while !seen.ends_with(b"\r\n\r\n") && seen.len() < 16 * 1024 {
                match socket.read(&mut byte) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => seen.extend_from_slice(&byte),
                }
            }
            if let Ok(mut heads) = kept.lock() {
                heads.push(String::from_utf8_lossy(&seen).into_owned());
            }
            let _ = socket.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nunsigned\n",
            );
        }
    });
    Ok((port, heads))
}

/// An updater of alo 0.4.1 in a folder of this test's own, asking the
/// feed at `port`, trusting the worthless test keys — which nothing a
/// person runs trusts, and which signed nothing the feed serves.
fn updater(name: &str, port: u16) -> Result<Updater, String> {
    let root =
        std::env::temp_dir().join(format!("alo-window-updates-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let application = root.join("alo browser.app");
    std::fs::create_dir_all(&application).map_err(|why| why.to_string())?;
    Ok(Updater {
        address: Address::read(&format!("http://127.0.0.1:{port}/feed"))?,
        keys: Keys {
            in_use: Key::read(test_key::PUBLIC)?,
            spare: Key::read(test_key::SPARE)?,
        },
        kept: Kept::at(root.join("Updates")),
        application,
        version: Version::of(0, 4, 1),
        platform: "macos-arm64".to_owned(),
        piece: 16 * 1024,
    })
}

/// The thread, started with the switch `automatic`, and what it tells.
fn started(updater: Updater, automatic: Automatic) -> Result<(Updates, Receiver<Told>), String> {
    let (tell, told) = mpsc::channel();
    let pool = Pool::with_trust(Trust::of(&[])?).patient_for(Duration::from_secs(5));
    let cause = Cause::Person {
        tab: Identities::default().a_tab(),
    };
    let thread = Updates::start(
        updater,
        (pool, cause),
        automatic,
        Box::new(move |said| tell.send(said).is_ok()),
    )
    .map_err(|why| why.to_string())?;
    Ok((thread, told))
}

/// What the thread tells next, waited for at most [`PATIENCE`].
fn next(told: &Receiver<Told>) -> Option<Told> {
    told.recv_timeout(PATIENCE).ok()
}

fn sent(heads: &Heads) -> Vec<String> {
    heads.lock().map(|heads| heads.clone()).unwrap_or_default()
}

/// Each head is a plain `GET` of the manifest or its signature, saying
/// `alo-update`, with no cookie and no query.
fn plain(heads: &[String]) -> bool {
    heads.iter().all(|head| {
        let lower = head.to_ascii_lowercase();
        (head.starts_with("GET /feed/macos-arm64/manifest HTTP/1.1\r\n")
            || head.starts_with("GET /feed/macos-arm64/manifest.minisig HTTP/1.1\r\n"))
            && lower.contains("\r\nuser-agent: alo-update\r\n")
            && !lower.contains("cookie")
    })
}

#[test]
fn off_sends_nothing_until_check_now_and_then_exactly_the_two_gets() {
    let (port, heads) = feed().expect("a feed");
    let (mut updates, told) =
        started(updater("off", port).expect("an updater"), Automatic::Off).expect("started");
    assert_eq!(next(&told), Some(Told::Waiting { next: None }));
    assert_eq!(sent(&heads), Vec::<String>::new(), "nothing left while off");
    updates.check_now();
    let Some(Told::Found { found, .. }) = next(&told) else {
        panic!("no check");
    };
    eprintln!("found: {}", found.said());
    assert!(matches!(found, Found::Refused(_)), "{found:?}");
    let heads_sent = sent(&heads);
    eprintln!(
        "sent: {:?}",
        heads_sent
            .iter()
            .filter_map(|head| head.lines().next())
            .collect::<Vec<_>>()
    );
    assert_eq!(heads_sent.len(), 2);
    assert!(plain(&heads_sent), "{heads_sent:?}");
    // Still off: waiting for the window's word alone.
    assert_eq!(next(&told), Some(Told::Waiting { next: None }));
    updates.finish();
    assert_eq!(sent(&heads).len(), 2);
}

#[test]
fn on_checks_at_launch_with_exactly_the_two_gets_and_next_a_day_later() {
    let (port, heads) = feed().expect("a feed");
    let before = Instant::now();
    let (mut updates, told) =
        started(updater("on", port).expect("an updater"), Automatic::On).expect("started");
    let Some(Told::Found { found, .. }) = next(&told) else {
        panic!("no check at launch");
    };
    assert!(matches!(found, Found::Refused(_)), "{found:?}");
    let Some(Told::Waiting { next: Some(at) }) = next(&told) else {
        panic!("no next check");
    };
    eprintln!(
        "next check in {} s",
        at.seconds().saturating_sub(before.seconds())
    );
    assert!(at.seconds() >= before.seconds() + EVERY);
    assert!(at.seconds() <= Instant::now().seconds() + EVERY);
    let heads_sent = sent(&heads);
    assert_eq!(heads_sent.len(), 2);
    assert!(plain(&heads_sent), "{heads_sent:?}");
    // Turned off: nothing due.
    updates.automatic(Automatic::Off);
    assert_eq!(next(&told), Some(Told::Waiting { next: None }));
    updates.finish();
    assert_eq!(sent(&heads).len(), 2);
}
