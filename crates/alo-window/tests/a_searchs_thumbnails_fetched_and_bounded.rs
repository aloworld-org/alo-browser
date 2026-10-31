/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 743 (ADR 0086 § 3): a search's thumbnails, fetched by the
//! browser process from the provider's image proxy alone, and bounded
//! before anything is sent to a renderer.
//!
//! Never the network: the proxy is a server on `127.0.0.1` serving
//! pictures made here, by hand, and bytes that are not pictures; a second
//! server stands for any other site, and must never be asked.
//!
//! Each asserted, of one fetch of thirty images found:
//!
//! - a picture is kept, its bytes as they arrived;
//! - one larger than a thumbnail is left out, and so is one that is not a
//!   picture this engine reads, one answered 404 and one answered with a
//!   redirect to the other server — whose server is never asked;
//! - bytes that begin as a picture and do not decode are kept here, since
//!   only the confined renderer decodes them (its drawing of them is
//!   `alo-renderer`'s `a_searchs_images_and_sites.rs`);
//! - a thumbnail named at the other server is never fetched;
//! - an image with no thumbnail is kept, with no picture;
//! - no more than twenty images are kept, and the proxy is asked for no
//!   more thumbnails than that;
//! - every thumbnail is asked with `Accept: image/*` and no key, cookie or
//!   `Referer`.

use alo_net::Pool;
use alo_net::cause::Identities;
use alo_net::tls::Trust;
use alo_paint::canvas::Canvas;
use alo_renderer::results::{LARGEST_THUMBNAIL, MOST_IMAGES, Pictured};
use alo_value::Rgba;
use alo_window::search_protocol::ImageFound;
use alo_window::search_thumbnails::fetch;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

/// A picture made by hand, as a PNG.
fn picture() -> Vec<u8> {
    let canvas = Canvas::new(500, 375, Rgba::from_rgba8(0x2f, 0x6b, 0x5e, 255));
    alo_paint::encode::to_png(&canvas).unwrap_or_default()
}

/// What the proxy answers for `target`, given the other server's `port`.
fn answer(target: &str, elsewhere: u16) -> (String, Vec<u8>) {
    let ok = "200 OK".to_owned();
    match target {
        "/large.png" => {
            let mut bytes = picture();
            bytes.resize(LARGEST_THUMBNAIL + 1, 0);
            (ok, bytes)
        }
        "/text.png" => (ok, b"<html>not a picture</html>".to_vec()),
        "/undecodable.png" => {
            let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
            bytes.extend_from_slice(&[0x5a; 64]);
            (ok, bytes)
        }
        "/moved.png" => (
            format!("302 Found\r\nLocation: http://127.0.0.1:{elsewhere}/picture.png"),
            Vec::new(),
        ),
        "/missing.png" => ("404 Not Found".to_owned(), Vec::new()),
        _ => (ok, picture()),
    }
}

/// A server on this machine answering by `answer`, each request's head told
/// to `asked`: its port.
fn serve(asked: Sender<String>, elsewhere: u16) -> Result<u16, String> {
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
                let (status, body) = answer(&target, elsewhere);
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

/// An image found on a page of its own, its thumbnail at `thumbnail`.
fn found(n: usize, thumbnail: Option<alo_url::Url>) -> Result<ImageFound, String> {
    Ok(ImageFound {
        pictured: Pictured::new(
            format!("Image {n}"),
            url(&format!("https://p{n}.example/"))?,
            "",
        ),
        thumbnail,
    })
}

#[test]
fn thumbnails_come_only_from_the_proxy_bounded_in_size_and_count() -> Result<(), String> {
    let (told_elsewhere, asked_elsewhere) = channel();
    let elsewhere = serve(told_elsewhere, 1)?;
    let (told, asked) = channel();
    let proxy = serve(told, elsewhere)?;
    let at = |path: &str| url(&format!("http://127.0.0.1:{proxy}{path}"));
    let mut images = vec![
        found(0, Some(at("/picture.png")?))?,
        found(1, Some(at("/large.png")?))?,
        found(2, Some(at("/text.png")?))?,
        found(3, Some(at("/undecodable.png")?))?,
        found(4, Some(at("/missing.png")?))?,
        found(5, Some(at("/moved.png")?))?,
        found(
            6,
            Some(url(&format!("http://127.0.0.1:{elsewhere}/picture.png"))?),
        )?,
        found(7, None)?,
    ];
    for n in 8..30 {
        images.push(found(n, Some(at(&format!("/many-{n}.png"))?))?);
    }
    let mut pool = Pool::with_trust(Trust::of(&[])?).patient_for(Duration::from_secs(10));
    let fetched = fetch(&mut pool, &mut Identities::default(), &at("/")?, images);

    assert_eq!(fetched.images.len(), MOST_IMAGES, "twenty kept of thirty");
    let held: Vec<bool> = fetched
        .images
        .iter()
        .map(|image| image.picture.is_some())
        .collect();
    let mut expected = vec![true, false, false, true, false, false, false, false];
    expected.extend(vec![true; MOST_IMAGES - 8]);
    assert_eq!(held, expected);
    assert_eq!(
        fetched
            .images
            .first()
            .and_then(|image| image.picture.clone()),
        Some(picture()),
        "the bytes as they arrived"
    );
    let why: Vec<(usize, String)> = fetched.left_out.clone();
    let places: Vec<usize> = why.iter().map(|(at, _)| *at).collect();
    assert_eq!(places, vec![1, 2, 4, 5, 6]);
    let says = |at: usize| {
        why.iter()
            .find(|(each, _)| *each == at)
            .map(|(_, why)| why.clone())
            .unwrap_or_default()
    };
    assert!(
        says(1).contains(&format!("{} bytes", LARGEST_THUMBNAIL + 1)),
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

    let heads: Vec<String> = asked.try_iter().collect();
    assert_eq!(
        heads.len(),
        6 + (MOST_IMAGES - 8),
        "each thumbnail at the proxy asked once, none past twenty"
    );
    for head in &heads {
        let lower = head.to_lowercase();
        assert!(lower.contains("accept: image/*"), "{head}");
        for never in ["x-subscription-token", "cookie", "referer"] {
            assert!(!lower.contains(never), "{never}: {head}");
        }
    }
    assert!(
        asked_elsewhere.try_iter().next().is_none(),
        "the other server was never asked"
    );
    Ok(())
}
