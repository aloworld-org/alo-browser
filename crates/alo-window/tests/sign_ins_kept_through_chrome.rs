/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ADR 0111, queue item 935: a sign-in made in one Chromium alo drives is
//! kept, sealed, in a jar of the browser process's own — and given to the
//! next Chromium as it starts, through alo restarting too — shown in the
//! cookies a test server of this test's own is sent. Through the
//! machine's Chrome and the Chromium alo carries (its CEF host), headless,
//! each on a new profile removed with it as ever.
//!
//! The server's `/signin` signs a person in: a cookie kept an hour
//! (`sid`), as a site's *Keep me signed in* is, and a session's (`tab`).
//! `/signout` removes `sid`. `/page` sets nothing, and what it is sent is
//! what that Chromium holds.
//!
//! - A new Chromium on the same jar is sent both: one card closed and
//!   another opened, alo still running.
//! - The jar read again from its directory — alo quit and opened again —
//!   holds `sid` alone: a session's cookie is never written.
//! - Signed out in one Chromium, the next is sent nothing.
//! - *Forget all sign-ins* empties the open Chromium's jar at its next look
//!   and the file on the disk: its page loaded again is sent nothing, and
//!   so is the next Chromium.
//! - A Chromium started refusing every site's cookies (ADR 0108 § 1) is
//!   given the jar and sends nothing of it.
//!
//! Never the real network, the person's keychain or their kept sign-ins:
//! the jar is in a directory of the test's own, sealed under a key of the
//! test's own, and the person's own directory is asserted untouched.

#[cfg(target_os = "macos")]
mod carried;

use alo_renderer::settings_privacy::Cookies;
use alo_window::beside::Chromium;
use alo_window::chat_seal::{KEY_BYTES, Key};
use alo_window::chrome::Chrome;
use alo_window::chrome_sign_ins;
use alo_window::chrome_tab::Tab;
use alo_window::sign_ins::{FILE, SignIns};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The size every page is laid out at.
const SIZE: (u32, u32) = (600, 400);

/// How long what a page asks for is waited for.
const AT_MOST: Duration = Duration::from_secs(30);

/// What the server received: each request's path and `Cookie`.
type Log = Arc<Mutex<Vec<(String, String)>>>;

/// A server on this machine: its port, and what it received.
fn serve() -> Result<(u16, Log), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
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
                let cookie = asked
                    .lines()
                    .filter_map(|line| line.split_once(':'))
                    .find(|(key, _)| key.trim().eq_ignore_ascii_case("Cookie"))
                    .map(|(_, value)| value.trim().to_owned())
                    .unwrap_or_default();
                if let Ok(mut log) = logging.lock() {
                    log.push((path.clone(), cookie));
                }
                let set: &[&str] = if path.starts_with("/signin") {
                    &["sid=ana; Path=/; Max-Age=3600; HttpOnly", "tab=1; Path=/"]
                } else if path.starts_with("/signout") {
                    &["sid=; Path=/; Max-Age=0"]
                } else {
                    &[]
                };
                let cookies: String = set
                    .iter()
                    .flat_map(|each| ["Set-Cookie: ", each, "\r\n"])
                    .collect();
                let body = "<!doctype html><p>A page</p>";
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\n{cookies}Cache-Control: no-store\r\n\
                     Connection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Ok((port, log))
}

/// What the server was sent with each request for `path`, in order.
fn sent(log: &Log, path: &str) -> Vec<String> {
    log.lock()
        .map(|log| {
            log.iter()
                .filter(|(at, _)| at.starts_with(path))
                .map(|(_, cookie)| cookie.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// The cookies `path` was last sent, sorted, as one line.
fn last_sent(log: &Log, path: &str) -> String {
    let mut each: Vec<String> = sent(log, path)
        .last()
        .map(|line| line.split("; ").map(str::to_owned).collect())
        .unwrap_or_default();
    each.retain(|cookie| !cookie.is_empty());
    each.sort();
    each.join("; ")
}

/// A directory of the test's own for the jar, removed when done.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("alo-sign-ins-chrome-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A Chromium started keeping `cookies`, given `jar`, which opens `path`
/// on the server at `port` — waited for until the server has been asked
/// for it once more — and is looked at as it closes, as a card is.
fn a_card(
    chromium: &Chromium,
    (jar, cookies): (&Arc<SignIns>, Cookies),
    (port, log): (u16, &Log),
    path: &str,
) -> Result<(), String> {
    let mut chrome =
        Chrome::start_keeping(chromium, false, cookies).map_err(|why| why.to_string())?;
    let mut keeping = chrome_sign_ins::given(&mut chrome, Arc::clone(jar))
        .map_err(|why| format!("not given: {why}"))?;
    let before = sent(log, path).len();
    let address = alo_url::parse(&format!("http://127.0.0.1:{port}{path}"))
        .map_err(|why| format!("{why:?}"))?;
    let mut tab = Tab::open(&mut chrome, &address, SIZE).map_err(|why| why.to_string())?;
    let until = Instant::now() + AT_MOST;
    while sent(log, path).len() <= before {
        if Instant::now() > until {
            return Err(format!("{path} was never asked for"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    keeping.look(&mut tab)
}

/// Every step of the file's comment, through `chromium`.
fn kept_through(chromium: &Chromium) -> Result<(), String> {
    let (port, log) = serve()?;
    let scratch = Scratch::new(match chromium {
        Chromium::Machines => "chrome",
        Chromium::Carried(_) => "carried",
    });
    let key = || Ok(Key::of([0x35; KEY_BYTES]));
    let server = (port, &log);
    let default = Cookies::RefuseOthers;

    // Signed in, in one card; another card, on the same jar, is sent both.
    let jar = Arc::new(SignIns::kept_in(&scratch.0, key()));
    a_card(chromium, (&jar, default), server, "/signin")?;
    assert_eq!(
        last_sent(&log, "/signin"),
        "",
        "a new profile, signed in to nothing"
    );
    a_card(chromium, (&jar, default), server, "/page")?;
    assert_eq!(last_sent(&log, "/page"), "sid=ana; tab=1");
    assert!(scratch.0.join(FILE).exists(), "written to the disk");
    drop(jar);

    // alo quit and opened again: what lasts, and not a session's.
    let jar = Arc::new(SignIns::kept_in(&scratch.0, key()));
    a_card(chromium, (&jar, default), server, "/page")?;
    assert_eq!(last_sent(&log, "/page"), "sid=ana");

    // Refusing every site's cookies, the Chromium is given them and sends
    // nothing.
    a_card(chromium, (&jar, Cookies::RefuseAll), server, "/page")?;
    assert_eq!(last_sent(&log, "/page"), "");
    // And takes nothing away from the jar by holding them unsent.
    a_card(chromium, (&jar, default), server, "/page")?;
    assert_eq!(last_sent(&log, "/page"), "sid=ana");

    // Signed out in one, the next is sent nothing; signed in again.
    a_card(chromium, (&jar, default), server, "/signout")?;
    a_card(chromium, (&jar, default), server, "/page")?;
    assert_eq!(last_sent(&log, "/page"), "");
    a_card(chromium, (&jar, default), server, "/signin")?;

    // Forget all, while a card is open: its Chromium's own jar emptied at
    // its next look, the page loaded again sent nothing, the file gone.
    let mut chrome =
        Chrome::start_keeping(chromium, false, default).map_err(|why| why.to_string())?;
    let mut keeping = chrome_sign_ins::given(&mut chrome, Arc::clone(&jar))
        .map_err(|why| format!("not given: {why}"))?;
    let page = alo_url::parse(&format!("http://127.0.0.1:{port}/page?open"))
        .map_err(|why| format!("{why:?}"))?;
    {
        let mut tab = Tab::open(&mut chrome, &page, SIZE).map_err(|why| why.to_string())?;
        keeping.look(&mut tab)?;
    }
    assert_eq!(last_sent(&log, "/page?open"), "sid=ana; tab=1");
    jar.forget_all_under(Ok(Some(Key::of([0x36; KEY_BYTES]))))?;
    assert!(!scratch.0.join(FILE).exists(), "the file removed");
    {
        let mut tab = Tab::open(&mut chrome, &page, SIZE).map_err(|why| why.to_string())?;
        keeping.look(&mut tab)?;
    }
    let again = alo_url::parse(&format!("http://127.0.0.1:{port}/page?again"))
        .map_err(|why| format!("{why:?}"))?;
    {
        let _tab = Tab::open(&mut chrome, &again, SIZE).map_err(|why| why.to_string())?;
        let until = Instant::now() + AT_MOST;
        while sent(&log, "/page?again").is_empty() {
            if Instant::now() > until {
                return Err("the page was never loaded again".to_owned());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    assert_eq!(
        last_sent(&log, "/page?again"),
        "",
        "the open card forgot them"
    );
    drop(chrome);
    assert_eq!(jar.cookies(), Vec::new());
    a_card(chromium, (&jar, default), server, "/page")?;
    assert_eq!(last_sent(&log, "/page"), "");
    drop(jar);
    let jar = SignIns::kept_in(&scratch.0, Ok(Key::of([0x36; KEY_BYTES])));
    assert_eq!(jar.cookies(), Vec::new(), "nothing kept after it");
    Ok(())
}

/// The modification time and length of the person's own kept sign-ins,
/// if there are any: to be the same after the test as before.
fn the_persons_own() -> Option<(std::time::SystemTime, u64)> {
    let directory = alo_window::sign_ins::for_this_person().ok()?;
    let about = std::fs::metadata(Path::new(&directory).join(FILE)).ok()?;
    Some((about.modified().ok()?, about.len()))
}

#[test]
fn the_machines_chrome_keeps_a_sign_in_across_chromiums_and_restarts() {
    let before = the_persons_own();
    let result = kept_through(&Chromium::Machines);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(the_persons_own(), before, "the person's own never touched");
}

#[cfg(target_os = "macos")]
#[test]
fn alos_own_chromium_keeps_a_sign_in_across_chromiums_and_restarts() {
    let host = match carried::host("935") {
        Ok(Some(host)) => host,
        Ok(None) => return,
        Err(why) => panic!("{why}"),
    };
    let before = the_persons_own();
    let result = kept_through(&host.chromium());
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(the_persons_own(), before, "the person's own never touched");
}
