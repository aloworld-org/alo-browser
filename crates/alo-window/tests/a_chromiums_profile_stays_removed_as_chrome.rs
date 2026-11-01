/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 733: a Chromium's profile, once alo has removed it, is not
//! made again — through the machine's Chrome and through alo's CEF host,
//! headless and shown.
//!
//! **Found** on 30 October 2026 (item 727): about a second after alo
//! removed a host's profile, the same path was made again holding an empty
//! `Dictionaries` directory and an empty `chrome_debug.log`. Twenty such
//! directories are still in this Mac's temporary directory, all made
//! between 12:55 and 13:24 that day, while the host was being built — when
//! it still ended inside `AppKit`'s `-terminate:`, at once, with CEF never
//! shut down and its helpers left to end on their own (ADR 0079 § 8b). None
//! has been made since the host ends in order (queue item 731, ADR 0080:
//! its pages closed, `cef::shutdown` run, status 0), through every gate
//! run of 31 October and 1 November. A remade directory holds no
//! `alo.lock`, so ADR 0069's sweep leaves it for good.
//!
//! **Whose it is, measured** on 1 November 2026 with CEF 156.0.14, the host
//! made to end at once again (its `-terminate:` given `exit(0)` instead of
//! closing its pages): its GPU process outlived it, the only helper still
//! running as alo removed the profile 50 ms after `Browser.close`, and as
//! it ended, 1.6 s after, the profile was there again holding
//! `Dictionaries` and `chrome_debug.log` — the very two — and this test
//! failed. Ending in order, `cef::shutdown` waits for its helpers before
//! the host ends, and nothing outlives it to write.
//!
//! Each Chromium here is started on a page of this test's own server and
//! closed as alo closes it (`Browser.close`, its handle dropped), and then
//! **looked at again once nothing of it is running** — no process left
//! whose command line names its profile, which every helper's does
//! (`--user-data-dir`) — and once more [`LOOK_AGAIN`] after: the profile
//! is gone, and stays gone. Never the real network.

#![cfg(target_os = "macos")]

mod carried;

use alo_window::beside::Chromium;
use alo_window::chrome::Chrome;
use alo_window::chrome_tab::Tab;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// How long a remade profile is looked for after the last process naming
/// it has gone: past the second after which one was found made.
const LOOK_AGAIN: Duration = Duration::from_secs(3);

/// How long a Chromium's helpers are given to end once it has.
const HELPERS_WITHIN: Duration = Duration::from_secs(30);

/// The page the server answers.
const PAGE: &str = "<!doctype html><title>Kept nowhere</title><p>Nothing is kept.";

/// A server on this machine answering every request with [`PAGE`]: its
/// port.
fn serve() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
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
                if asked.is_empty() {
                    return;
                }
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{PAGE}",
                    PAGE.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Ok(port)
}

/// The processes whose command line names `profile`: each one's line.
fn naming(profile: &Path) -> Result<Vec<String>, String> {
    let listed = Command::new("/bin/ps")
        .args(["-axww", "-o", "pid=,command="])
        .output()
        .map_err(|why| format!("ps did not run: {why}"))?;
    let profile = profile.to_string_lossy();
    Ok(String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter(|line| line.contains(profile.as_ref()))
        .map(str::to_owned)
        .collect())
}

/// What `profile` holds, if it is there.
fn holding(profile: &Path) -> Option<Vec<String>> {
    let entries = std::fs::read_dir(profile).ok()?;
    let mut held: Vec<String> = entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    held.sort();
    Some(held)
}

/// `chromium` started on the server's page, shown or headless, and closed
/// as alo closes it: its profile gone once nothing of it runs, and still
/// gone [`LOOK_AGAIN`] after.
fn stays_removed(chromium: &Chromium, shown: bool, port: u16, who: &str) -> Result<(), String> {
    let mut chrome = Chrome::start_from(chromium, shown).map_err(|why| why.to_string())?;
    let profile = chrome.profile().to_path_buf();
    {
        let address = alo_url::parse(&format!("http://127.0.0.1:{port}/kept.html"))
            .map_err(|why| format!("{why:?}"))?;
        let _tab = Tab::open(&mut chrome, &address, (800, 600)).map_err(|why| why.to_string())?;
    }
    let ended = chrome.close();
    assert!(
        ended.is_some_and(|status| status.success()),
        "{who}: ended {ended:?}"
    );
    let until = Instant::now() + HELPERS_WITHIN;
    let mut left = naming(&profile)?;
    while !left.is_empty() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
        left = naming(&profile)?;
    }
    assert_eq!(left, Vec::<String>::new(), "{who}: still running");
    assert_eq!(
        holding(&profile),
        None,
        "{who}: {} is there once nothing of it runs",
        profile.display()
    );
    let until = Instant::now() + LOOK_AGAIN;
    while Instant::now() < until {
        if let Some(held) = holding(&profile) {
            let _ = std::fs::remove_dir_all(&profile);
            return Err(format!(
                "{who}: {} was made again, holding {held:?}",
                profile.display()
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    eprintln!("{who}: its profile stayed removed");
    Ok(())
}

#[test]
fn the_machines_chromes_profile_stays_removed() -> Result<(), String> {
    let port = serve()?;
    stays_removed(&Chromium::Machines, false, port, "Chrome, headless")?;
    stays_removed(&Chromium::Machines, true, port, "Chrome, shown")
}

#[test]
fn alos_cef_hosts_profile_stays_removed() -> Result<(), String> {
    let Some(host) = carried::host("733")? else {
        return Ok(());
    };
    let port = serve()?;
    stays_removed(&host.chromium(), false, port, "alo's CEF host, headless")?;
    stays_removed(&host.chromium(), true, port, "alo's CEF host, shown")
}
