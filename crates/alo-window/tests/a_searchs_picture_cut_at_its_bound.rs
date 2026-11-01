/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 762 (ADR 0086 § 3): a search's thumbnail, and a site's mark,
//! answered with far more than the bound is cut at the bound — never read
//! whole by `alo-net` and weighed after.
//!
//! Never the network: the proxy is a server on `127.0.0.1` that answers
//! every picture with a picture's first bytes and then 128 MiB more — said
//! up front as its length, or in chunks — and counts what it managed to
//! write before the browser process gave up. Through the functions the
//! search's thread calls (`search_thumbnails::fetch`, `search_marks::fetch`):
//! each picture is left out as more than its bound, and the proxy wrote a
//! small part of what it meant to.

use alo_net::Pool;
use alo_net::cause::Identities;
use alo_net::tls::Trust;
use alo_renderer::results::{LARGEST_MARK, LARGEST_THUMBNAIL, Pictured};
use alo_window::search_marks;
use alo_window::search_protocol::ImageFound;
use alo_window::search_thumbnails::fetch;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

/// What the proxy means to send of each picture: 128 MiB.
const MEANT: usize = 128 * 1024 * 1024;

/// The most it may manage to write before a request cut at its bound stops
/// it: the bound and what the sockets' buffers hold, far short of [`MEANT`].
const STOPPED_BY: usize = 32 * 1024 * 1024;

/// A server answering every request with a PNG's signature and then
/// [`MEANT`] bytes, framed by its length when `said` and in chunks when
/// not: its port, and how many bytes of body it wrote for each request.
fn serve(said: bool) -> Result<(u16, Receiver<usize>), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    let (told, wrote) = channel();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            let told = told.clone();
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
                let framing = if said {
                    format!("Content-Length: {MEANT}")
                } else {
                    "Transfer-Encoding: chunked".to_owned()
                };
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\n{framing}\r\n\
                     Connection: close\r\n\r\n"
                );
                let mut written = 0;
                if socket.write_all(reply.as_bytes()).is_ok() {
                    let mut piece = b"\x89PNG\r\n\x1a\n".to_vec();
                    piece.resize(64 * 1024, 0x5a);
                    while written < MEANT {
                        let sent = if said {
                            socket.write_all(&piece)
                        } else {
                            let mut chunk = format!("{:x}\r\n", piece.len()).into_bytes();
                            chunk.extend_from_slice(&piece);
                            chunk.extend_from_slice(b"\r\n");
                            socket.write_all(&chunk)
                        };
                        if sent.is_err() {
                            break;
                        }
                        written += piece.len();
                    }
                }
                let _ = told.send(written);
            });
        }
    });
    Ok((port, wrote))
}

fn url(written: &str) -> Result<alo_url::Url, String> {
    alo_url::parse(written).map_err(|why| format!("{written}: {why:?}"))
}

fn pool() -> Result<Pool, String> {
    Ok(Pool::with_trust(Trust::of(&[])?).patient_for(Duration::from_secs(20)))
}

/// What the proxy wrote for the one request it was asked.
fn written(wrote: &Receiver<usize>) -> Result<usize, String> {
    wrote
        .recv_timeout(Duration::from_secs(60))
        .map_err(|why| format!("the proxy never said: {why}"))
}

#[test]
fn a_thumbnail_far_past_its_bound_is_cut_there_and_left_out() -> Result<(), String> {
    for said in [true, false] {
        let (proxy, wrote) = serve(said)?;
        let from = url(&format!("http://127.0.0.1:{proxy}/"))?;
        let found = vec![ImageFound {
            pictured: Pictured::new("Huge".to_owned(), url("https://p.example/")?, ""),
            thumbnail: Some(url(&format!("http://127.0.0.1:{proxy}/huge.png"))?),
            mark: None,
        }];
        let fetched = fetch(&mut pool()?, &mut Identities::default(), &from, found);
        assert_eq!(
            fetched
                .images
                .first()
                .and_then(|image| image.picture.as_ref()),
            None,
            "said {said}: kept"
        );
        let why = fetched
            .left_out
            .first()
            .map(|(_, why)| why.clone())
            .unwrap_or_default();
        assert!(
            why.contains(&format!(
                "more than the {LARGEST_THUMBNAIL} this request reads"
            )),
            "said {said}: {why}"
        );
        let sent = written(&wrote)?;
        assert!(
            sent < STOPPED_BY,
            "said {said}: the proxy wrote {sent} bytes"
        );
        eprintln!("said {said}: left out, the proxy wrote {sent} of {MEANT} bytes: {why}");
    }
    Ok(())
}

#[test]
fn a_sites_mark_far_past_its_bound_is_cut_there_and_left_out() -> Result<(), String> {
    let (proxy, wrote) = serve(false)?;
    let from = url(&format!("http://127.0.0.1:{proxy}/"))?;
    let mark = url(&format!("http://127.0.0.1:{proxy}/mark.png"))?;
    let marked = search_marks::fetch(
        &mut pool()?,
        &mut Identities::default(),
        Some(&from),
        &[Some(mark)],
    );
    assert_eq!(marked.marks.first().cloned().flatten(), None, "kept");
    let why = marked
        .left_out
        .first()
        .map(|(_, why)| why.clone())
        .unwrap_or_default();
    assert!(
        why.contains(&format!("more than the {LARGEST_MARK} this request reads")),
        "{why}"
    );
    let sent = written(&wrote)?;
    assert!(sent < STOPPED_BY, "the proxy wrote {sent} bytes");
    Ok(())
}
