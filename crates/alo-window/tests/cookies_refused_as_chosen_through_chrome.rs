/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 920, ADR 0108 § 1: what a Chromium alo drives keeps of
//! sites' cookies is what *Settings › Privacy* chose — another site's
//! refused (the default), every site's refused, or all kept — shown in the
//! cookies a test server of this test's own is sent back. Through the
//! machine's Chrome and the Chromium alo carries (its CEF host), headless.
//!
//! The page is at `127.0.0.1` and sets a cookie of its own, kept an hour;
//! it holds a frame from `localhost` — another site — which sets one too,
//! `SameSite=None; Secure` so that a Chromium would send it in another
//! site's page at all (Chromium takes `localhost` as secure for a cookie).
//! The page is then opened again in the same Chromium, and what the server
//! received with each request is what that Chromium kept.
//!
//! Never the real network.

#[cfg(target_os = "macos")]
mod carried;

use alo_renderer::settings_privacy::Cookies;
use alo_window::beside::Chromium;
use alo_window::chrome::Chrome;
use alo_window::chrome_tab::Tab;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The size every page is laid out at.
const SIZE: (u32, u32) = (600, 400);

/// How long what a page asks for is waited for.
const AT_MOST: Duration = Duration::from_secs(30);

/// What the server received: each request's host, path and `Cookie`.
type Log = Arc<Mutex<Vec<(String, String, String)>>>;

/// A server on this machine: its port, and what it received.
fn serve() -> Option<(u16, Log)> {
    let listener = TcpListener::bind("127.0.0.1:0").ok()?;
    let port = listener.local_addr().ok()?.port();
    let log: Log = Arc::default();
    let logging = Arc::clone(&log);
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let logging = Arc::clone(&logging);
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
                let asked = String::from_utf8_lossy(&asked).into_owned();
                let path = asked.split(' ').nth(1).unwrap_or_default().to_owned();
                if path.is_empty() {
                    return;
                }
                let header = |name: &str| {
                    asked
                        .lines()
                        .filter_map(|line| line.split_once(':'))
                        .find(|(key, _)| key.trim().eq_ignore_ascii_case(name))
                        .map(|(_, value)| value.trim().to_owned())
                        .unwrap_or_default()
                };
                let host = header("Host");
                let host = host.split(':').next().unwrap_or_default().to_owned();
                if let Ok(mut log) = logging.lock() {
                    log.push((host, path.clone(), header("Cookie")));
                }
                let (set, body) = if path.starts_with("/top") {
                    (
                        "first=1; Path=/; Max-Age=3600",
                        format!(
                            "<!doctype html><h1>A page</h1>\
                             <iframe src=\"http://localhost:{port}/frame\"></iframe>"
                        ),
                    )
                } else if path.starts_with("/frame") {
                    (
                        "third=1; Path=/; Max-Age=3600; SameSite=None; Secure",
                        "<!doctype html><p>Another site</p>".to_owned(),
                    )
                } else {
                    ("", String::new())
                };
                let cookie = if set.is_empty() {
                    String::new()
                } else {
                    format!("Set-Cookie: {set}\r\n")
                };
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\n{cookie}Cache-Control: no-store\r\n\
                     Connection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Some((port, log))
}

/// What the server received with each request for `path` on `host`, in
/// order.
fn cookies_sent(log: &Log, host: &str, path: &str) -> Vec<String> {
    log.lock()
        .map(|log| {
            log.iter()
                .filter(|(each, at, _)| each == host && at.starts_with(path))
                .map(|(_, _, cookie)| cookie.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// The page opened in `chrome` at `address`, and waited for until the
/// server has been asked for its frame `frames` times.
fn open(chrome: &mut Chrome, address: &str, log: &Log, frames: usize) -> Result<(), String> {
    let address = alo_url::parse(address).map_err(|why| format!("{why:?}"))?;
    let _tab = Tab::open(chrome, &address, SIZE).map_err(|why| why.to_string())?;
    let until = Instant::now() + AT_MOST;
    while cookies_sent(log, "localhost", "/frame").len() < frames {
        if Instant::now() > until {
            return Err(format!("the frame was never asked for: {log:?}"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

/// The page opened twice through `chromium` started keeping `cookies`:
/// what the second opening sent back, the page's own and the frame's.
fn sent_back(chromium: &Chromium, cookies: Cookies) -> Result<(String, String), String> {
    let (port, log) = serve().ok_or("no server")?;
    let mut chrome =
        Chrome::start_keeping(chromium, false, cookies).map_err(|why| why.to_string())?;
    open(
        &mut chrome,
        &format!("http://127.0.0.1:{port}/top"),
        &log,
        1,
    )?;
    open(
        &mut chrome,
        &format!("http://127.0.0.1:{port}/top?again"),
        &log,
        2,
    )?;
    let top = cookies_sent(&log, "127.0.0.1", "/top");
    let frame = cookies_sent(&log, "localhost", "/frame");
    // The first opening was sent nothing: the profile was new.
    assert_eq!(top.first().map(String::as_str), Some(""), "{top:?}");
    assert_eq!(frame.first().map(String::as_str), Some(""), "{frame:?}");
    Ok((
        top.get(1).cloned().unwrap_or_default(),
        frame.get(1).cloned().unwrap_or_default(),
    ))
}

/// Each of the three choices through `chromium`.
fn each_choice(chromium: &Chromium) -> Result<(), String> {
    // Another site's refused, the page's own kept: the default.
    assert_eq!(
        sent_back(chromium, Cookies::RefuseOthers)?,
        ("first=1".to_owned(), String::new())
    );
    // Every site's refused.
    assert_eq!(
        sent_back(chromium, Cookies::RefuseAll)?,
        (String::new(), String::new())
    );
    // All kept.
    assert_eq!(
        sent_back(chromium, Cookies::KeepAll)?,
        ("first=1".to_owned(), "third=1".to_owned())
    );
    Ok(())
}

#[test]
fn the_machines_chrome_keeps_of_cookies_what_settings_chose() {
    let result = each_choice(&Chromium::Machines);
    assert!(result.is_ok(), "{result:?}");
}

#[cfg(target_os = "macos")]
#[test]
fn alos_own_chromium_keeps_of_cookies_what_settings_chose() {
    let host = match carried::host("920") {
        Ok(Some(host)) => host,
        Ok(None) => return,
        Err(why) => panic!("{why}"),
    };
    let result = each_choice(&host.chromium());
    assert!(result.is_ok(), "{result:?}");
}
