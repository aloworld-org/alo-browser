/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 747 (ADR 0088): each site's own mark, fetched by the browser
//! process from the provider's image proxy alone, and bounded before
//! anything is sent to a renderer.
//!
//! Never the network: the proxy is a server on `127.0.0.1` serving icons
//! made here, by hand, and bytes that are not icons; a second server stands
//! for the site itself, and must never be asked.
//!
//! Each asserted, of one fetch of the marks of thirty results:
//!
//! - an icon is kept, its bytes as they arrived;
//! - one larger than a mark may be is left out, and so is one that is not a
//!   picture, one answered 404 and one answered with a redirect to the site
//!   — which is never asked;
//! - bytes that begin as a picture and do not decode are kept here, since
//!   only the confined renderer decodes them (it draws them as the site's
//!   letter: `alo-renderer`'s `a_sites_own_mark.rs`);
//! - a mark named at the site itself is never fetched;
//! - a result with no mark has none;
//! - an icon named by many results is asked once;
//! - no more than twenty distinct icons are asked for one answer;
//! - every icon is asked with `Accept: image/*` and no key, cookie or
//!   `Referer`.

use alo_net::Pool;
use alo_net::cause::Identities;
use alo_net::tls::Trust;
use alo_paint::canvas::Canvas;
use alo_renderer::results::{LARGEST_MARK, MOST_MARKS};
use alo_value::Rgba;
use alo_window::search_marks::fetch;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

/// An icon made by hand, 32 square as Brave's proxy sends them, as a PNG.
fn icon() -> Vec<u8> {
    let canvas = Canvas::new(32, 32, Rgba::from_rgba8(0xc4, 0x30, 0x2b, 255));
    alo_paint::encode::to_png(&canvas).unwrap_or_default()
}

/// What the proxy answers for `target`, given the site's `port`.
fn answer(target: &str, site: u16) -> (String, Vec<u8>) {
    let ok = "200 OK".to_owned();
    match target {
        "/large.png" => {
            let mut bytes = icon();
            bytes.resize(LARGEST_MARK + 1, 0);
            (ok, bytes)
        }
        "/text.png" => (ok, b"<html>not an icon</html>".to_vec()),
        "/undecodable.png" => {
            let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
            bytes.extend_from_slice(&[0x5a; 64]);
            (ok, bytes)
        }
        "/moved.png" => (
            format!("302 Found\r\nLocation: http://127.0.0.1:{site}/favicon.ico"),
            Vec::new(),
        ),
        "/missing.png" => ("404 Not Found".to_owned(), Vec::new()),
        _ => (ok, icon()),
    }
}

/// A server on this machine answering by `answer`, each request's head told
/// to `asked`: its port.
fn serve(asked: Sender<String>, site: u16) -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let asked = asked.clone();
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
                let target = head.split(' ').nth(1).unwrap_or_default().to_owned();
                let _ = asked.send(head);
                let (status, body) = answer(&target, site);
                let mut reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: image/png\r\nContent-Length: {}\r\n\
                     Connection: close\r\n\r\n",
                    body.len()
                )
                .into_bytes();
                reply.extend_from_slice(&body);
                let _ = socket.write_all(&reply);
            });
        }
    });
    Ok(port)
}

fn url(written: &str) -> Result<alo_url::Url, String> {
    alo_url::parse(written).map_err(|why| format!("{written}: {why:?}"))
}

#[test]
fn marks_come_only_from_the_proxy_once_each_bounded_in_size_and_count() -> Result<(), String> {
    let (told_site, asked_site) = channel();
    let site = serve(told_site, 1)?;
    let (told, asked) = channel();
    let proxy = serve(told, site)?;
    let at = |path: &str| url(&format!("http://127.0.0.1:{proxy}{path}"));
    let mut wanted = vec![
        Some(at("/icon.png")?),
        Some(at("/large.png")?),
        Some(at("/text.png")?),
        Some(at("/undecodable.png")?),
        Some(at("/missing.png")?),
        Some(at("/moved.png")?),
        Some(url(&format!("http://127.0.0.1:{site}/favicon.ico"))?),
        None,
        // The first icon again, as a second video on the same site names it.
        Some(at("/icon.png")?),
    ];
    for n in 9..30 {
        wanted.push(Some(at(&format!("/many-{n}.png"))?));
    }
    let mut pool = Pool::with_trust(Trust::of(&[])?).patient_for(Duration::from_secs(10));
    let marked = fetch(
        &mut pool,
        &mut Identities::default(),
        Some(&at("/")?),
        &wanted,
    );

    assert_eq!(
        marked.marks.len(),
        wanted.len(),
        "one answer for each result"
    );
    let held: Vec<bool> = marked.marks.iter().map(Option::is_some).collect();
    // Six distinct addresses were asked before the many (the first, large,
    // text, undecodable, missing, moved); the rest of the twenty are 14 of
    // the 21 many.
    let asked_first = 6;
    let mut expected = vec![true, false, false, true, false, false, false, false, true];
    expected.extend(vec![true; MOST_MARKS - asked_first]);
    expected.extend(vec![false; 21 - (MOST_MARKS - asked_first)]);
    assert_eq!(held, expected);
    assert_eq!(
        marked.marks.first().cloned().flatten(),
        Some(icon()),
        "the bytes as they arrived"
    );
    let says = |at: usize| {
        marked
            .left_out
            .iter()
            .find(|(each, _)| *each == at)
            .map(|(_, why)| why.clone())
            .unwrap_or_default()
    };
    assert!(
        says(1).contains(&format!("{} bytes", LARGEST_MARK + 1)),
        "{}",
        says(1)
    );
    assert!(says(2).contains("not a picture"), "{}", says(2));
    assert!(says(4).contains("answered 404"), "{}", says(4));
    assert!(says(5).contains("answered 302"), "{}", says(5));
    assert!(
        says(6).contains("not the search's image proxy"),
        "{}",
        says(6)
    );
    assert!(says(29).contains("first 20"), "{}", says(29));

    let heads: Vec<String> = asked.try_iter().collect();
    assert_eq!(
        heads.len(),
        MOST_MARKS,
        "each distinct icon at the proxy asked once, none past twenty"
    );
    let icons = heads
        .iter()
        .filter(|head| head.starts_with("GET /icon.png "))
        .count();
    assert_eq!(icons, 1, "an icon two results name is asked once");
    for head in &heads {
        let lower = head.to_lowercase();
        assert!(lower.contains("accept: image/*"), "{head}");
        for never in ["x-subscription-token", "cookie", "referer"] {
            assert!(!lower.contains(never), "{never}: {head}");
        }
    }
    assert!(
        asked_site.try_iter().next().is_none(),
        "the site itself was never asked"
    );
    Ok(())
}

#[test]
fn with_no_proxy_named_no_mark_is_fetched() -> Result<(), String> {
    let mut pool = Pool::with_trust(Trust::of(&[])?).patient_for(Duration::from_secs(1));
    let marked = fetch(
        &mut pool,
        &mut Identities::default(),
        None,
        &[Some(url("https://imgs.search.brave.com/a")?), None],
    );
    assert_eq!(marked.marks, vec![None, None]);
    assert_eq!(marked.left_out.len(), 1);
    Ok(())
}
