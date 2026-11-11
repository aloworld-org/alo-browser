/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ADR 0111 § 2, queue item 935: a card's Chromium is given the jar's
//! sign-ins before its page opens, and its own are kept in the jar as the
//! card closes — through [`ChromeServing::start_as_a_kept_card`], the road
//! every card of the conversation takes. Through the machine's Chrome and
//! alo's CEF host.
//!
//! A first card opens `/signin`, which sets a cookie kept an hour, and is
//! closed at once — before any look while it runs could have kept it, so
//! only the look as it closes can. A second card opens `/page`, whose very
//! first request must carry the cookie: it was given before the page
//! opened. A card started with no jar, as every test's and `alo --chrome`'s
//! are, is sent nothing.

#[cfg(target_os = "macos")]
mod carried;

use alo_window::agent_sites::AgentSites;
use alo_window::beside::Chromium;
use alo_window::chat_seal::{KEY_BYTES, Key};
use alo_window::chrome_serving::{ChromeServing, Opening};
use alo_window::message::Order;
use alo_window::scale::Scale;
use alo_window::sign_ins::SignIns;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PAGE: (u32, u32) = alo_renderer::card::PAGE_SIZE;

/// Each request's path and `Cookie`, as the server received them.
type Log = Arc<Mutex<Vec<(String, String)>>>;

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
                let set = if path.starts_with("/signin") {
                    "Set-Cookie: sid=ana; Path=/; Max-Age=3600; HttpOnly\r\n"
                } else {
                    ""
                };
                let body = "<!doctype html><p>A page</p>";
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\n{set}Cache-Control: no-store\r\n\
                     Connection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Ok((port, log))
}

fn first_sent(log: &Log, path: &str) -> Option<String> {
    log.lock().ok().and_then(|log| {
        log.iter()
            .find(|(at, _)| at == path)
            .map(|(_, cookie)| cookie.clone())
    })
}

/// A card at `path`, kept in `jar` if there is one, open until the server
/// was asked for `path`, then closed as *Close* closes it.
fn a_card(
    chromium: &Chromium,
    jar: Option<&Arc<SignIns>>,
    (port, log): (u16, &Log),
    path: &str,
) -> Result<(), String> {
    let url = alo_url::parse(&format!("http://127.0.0.1:{port}{path}"))
        .map_err(|why| format!("{why:?}"))?;
    let serving = ChromeServing::start_as_a_kept_card(
        chromium.clone(),
        Opening::Network(url),
        (PAGE, Scale::ONE),
        (AgentSites::Every, None, jar.cloned()),
        Box::new(|_| true),
    )
    .map_err(|why| format!("no card in Chromium: {why}"))?;
    let until = Instant::now() + Duration::from_secs(30);
    while first_sent(log, path).is_none() {
        if Instant::now() > until {
            return Err(format!("{path} was never asked for"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let orders = serving.orders();
    orders
        .send(Order::CloseEverything)
        .map_err(|_| "the card's thread has gone")?;
    drop(orders);
    serving
        .records()
        .ok_or("the card's thread did not finish")?;
    Ok(())
}

fn card_to_card(chromium: &Chromium) -> Result<(), String> {
    let (port, log) = serve()?;
    let scratch: PathBuf = std::env::temp_dir().join(format!(
        "alo-sign-ins-cards-{}-{}",
        std::process::id(),
        matches!(chromium, Chromium::Machines)
    ));
    let _ = std::fs::remove_dir_all(&scratch);
    let jar = Arc::new(SignIns::kept_in(&scratch, Ok(Key::of([0x47; KEY_BYTES]))));
    let result = (|| {
        a_card(chromium, Some(&jar), (port, &log), "/signin")?;
        assert_eq!(first_sent(&log, "/signin").as_deref(), Some(""));
        assert_eq!(
            jar.cookies()
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            vec!["sid"],
            "kept as the card closed"
        );
        a_card(chromium, Some(&jar), (port, &log), "/page")?;
        assert_eq!(
            first_sent(&log, "/page").as_deref(),
            Some("sid=ana"),
            "given before its first request"
        );
        a_card(chromium, None, (port, &log), "/page?nojar")?;
        assert_eq!(first_sent(&log, "/page?nojar").as_deref(), Some(""));
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

#[test]
fn a_sign_in_goes_from_one_card_to_the_next_through_chrome() {
    let result = card_to_card(&Chromium::Machines);
    assert!(result.is_ok(), "{result:?}");
}

#[cfg(target_os = "macos")]
#[test]
fn a_sign_in_goes_from_one_card_to_the_next_through_alos_cef_host() {
    let host = match carried::host("935") {
        Ok(Some(host)) => host,
        Ok(None) => return,
        Err(why) => panic!("{why}"),
    };
    let result = card_to_card(&host.chromium());
    assert!(result.is_ok(), "{result:?}");
}
