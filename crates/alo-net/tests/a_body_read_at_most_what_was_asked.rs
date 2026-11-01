/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 762 (ADR 0086 § 3): a request says the most of its response's
//! body it reads, and a body longer than that is cut at the bound — never
//! read whole and then weighed.
//!
//! Each way a body can say where it ends, over HTTP/1.1 against a server on
//! this machine that counts what it managed to write before the client gave
//! up, and over HTTP/2 against one that counts the `DATA` frames it sent and
//! hears the client reset the stream:
//!
//! - **a length said up front** larger than the bound is refused before a
//!   byte of the body is read;
//! - **chunks**, and **a body that runs until the connection closes**, are
//!   cut once they pass it — the server stops long before the gigabyte it
//!   meant to send;
//! - **a body exactly at the bound** is read whole, so the bound is the
//!   most, not one short of it;
//! - **gzip**, eight kibibytes on the wire undoing to eight mebibytes, is
//!   refused;
//! - **with no bound asked**, the same kilobytes are read as before.
//!
//! And the readers themselves, on bytes in memory: at most one byte past the
//! bound is ever taken from the source.

use alo_net::body::{Framing, read_at_most};
use alo_net::cause::{Cause, Identities};
use alo_net::h2::frame::{self, Frame, Setting};
use alo_net::h2::hpack::{self, Field, Table};
use alo_net::h2::{ErrorCode, client};
use alo_net::tls::Trust;
use alo_net::{Pool, Request};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

/// The bound the requests here ask for: a thumbnail's, 256 KiB.
const BOUND: usize = 256 * 1024;

/// What the servers here mean to send when they send a lot: a gigabyte.
const A_GIGABYTE: usize = 1024 * 1024 * 1024;

/// The most a server may manage to write before a client that cut the body
/// at [`BOUND`] stops it: the bound and what the two sockets' buffers hold
/// between them, far short of [`A_GIGABYTE`].
const STOPPED_BY: usize = 32 * 1024 * 1024;

fn a_person() -> Cause {
    Cause::Person {
        tab: Identities::default().a_tab(),
    }
}

fn request(port: u16, path: &str) -> Result<Request, String> {
    let url = alo_url::parse(&format!("http://127.0.0.1:{port}{path}"))
        .map_err(|why| format!("{why:?}"))?;
    Ok(Request::get(url, a_person()))
}

/// How an HTTP/1.1 answer's body is framed.
#[derive(Debug, Clone, Copy)]
enum Framed {
    /// `Content-Length`, then that many bytes.
    Length(usize),
    /// Chunks of 64 KiB, as many as make the length, then the last.
    Chunks(usize),
    /// No length: bytes until the connection closes.
    UntilClose(usize),
    /// `Content-Encoding: gzip`, the bytes given.
    Gzip(&'static [u8]),
}

/// A server answering one request as `framed` says: its port, and how many
/// bytes of body it wrote before the write failed or it was done.
fn serve(framed: Framed) -> Result<(u16, Receiver<usize>), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    let (told, wrote) = channel();
    std::thread::spawn(move || {
        let Some(Ok(mut socket)) = listener.incoming().next() else {
            return;
        };
        let mut head = Vec::new();
        let mut block = [0u8; 1024];
        while let Ok(got) = socket.read(&mut block) {
            head.extend_from_slice(block.get(..got).unwrap_or_default());
            if got == 0 || head.windows(4).any(|end| end == b"\r\n\r\n") {
                break;
            }
        }
        let piece = vec![0x5a_u8; 64 * 1024];
        let mut written = 0;
        let said = match framed {
            Framed::Length(length) => format!("Content-Length: {length}\r\n"),
            Framed::Chunks(_) => "Transfer-Encoding: chunked\r\n".to_owned(),
            Framed::UntilClose(_) => "Connection: close\r\n".to_owned(),
            Framed::Gzip(bytes) => {
                format!(
                    "Content-Encoding: gzip\r\nContent-Length: {}\r\n",
                    bytes.len()
                )
            }
        };
        let head = format!("HTTP/1.1 200 OK\r\nContent-Type: image/png\r\n{said}\r\n");
        if socket.write_all(head.as_bytes()).is_err() {
            let _ = told.send(written);
            return;
        }
        match framed {
            Framed::Gzip(bytes) => {
                if socket.write_all(bytes).is_ok() {
                    written = bytes.len();
                }
            }
            Framed::Length(length) | Framed::UntilClose(length) => {
                while written < length {
                    let taking = piece.len().min(length - written);
                    if socket
                        .write_all(piece.get(..taking).unwrap_or_default())
                        .is_err()
                    {
                        break;
                    }
                    written += taking;
                }
            }
            Framed::Chunks(length) => {
                while written < length {
                    let taking = piece.len().min(length - written);
                    let mut chunk = format!("{taking:x}\r\n").into_bytes();
                    chunk.extend_from_slice(piece.get(..taking).unwrap_or_default());
                    chunk.extend_from_slice(b"\r\n");
                    if socket.write_all(&chunk).is_err() {
                        break;
                    }
                    written += taking;
                }
                if written == length {
                    let _ = socket.write_all(b"0\r\n\r\n");
                }
            }
        }
        let _ = told.send(written);
        // Stay until the client has read what it reads.
        std::thread::sleep(Duration::from_millis(200));
    });
    Ok((port, wrote))
}

fn pool() -> Result<Pool, String> {
    Ok(Pool::with_trust(Trust::of(&[])?).patient_for(Duration::from_secs(20)))
}

/// What the server said it wrote, waiting for it.
fn written(wrote: &Receiver<usize>) -> Result<usize, String> {
    wrote
        .recv_timeout(Duration::from_secs(60))
        .map_err(|why| format!("the server never said: {why}"))
}

#[test]
fn a_length_said_past_the_bound_is_refused_before_its_body_is_read() -> Result<(), String> {
    // Under the engine's own most, so it is the request's bound that
    // refuses it.
    let (port, wrote) = serve(Framed::Length(128 * 1024 * 1024))?;
    let asked = request(port, "/large.png")?.reading_at_most(BOUND as u64);
    let fetched = pool()?.fetch(&asked);
    let why = fetched.err().ok_or("a gigabyte was read")?;
    assert!(
        why.contains("more than the 262144 this request reads"),
        "{why}"
    );
    let sent = written(&wrote)?;
    assert!(sent < STOPPED_BY, "the server wrote {sent} bytes");
    Ok(())
}

#[test]
fn chunks_and_a_body_until_close_are_cut_once_past_the_bound() -> Result<(), String> {
    for framed in [Framed::Chunks(A_GIGABYTE), Framed::UntilClose(A_GIGABYTE)] {
        let (port, wrote) = serve(framed)?;
        let asked = request(port, "/large.png")?.reading_at_most(BOUND as u64);
        let fetched = pool()?.fetch(&asked);
        let why = fetched
            .err()
            .ok_or(format!("{framed:?}: a gigabyte was read"))?;
        assert!(why.contains("this request reads"), "{framed:?}: {why}");
        let sent = written(&wrote)?;
        assert!(
            sent < STOPPED_BY,
            "{framed:?}: the server wrote {sent} bytes"
        );
    }
    Ok(())
}

#[test]
fn a_body_exactly_at_the_bound_is_read_whole_and_no_bound_reads_as_before() -> Result<(), String> {
    for framed in [
        Framed::Length(BOUND),
        Framed::Chunks(BOUND),
        Framed::UntilClose(BOUND),
    ] {
        let (port, _) = serve(framed)?;
        let asked = request(port, "/exact.png")?.reading_at_most(BOUND as u64);
        let response = pool()?.fetch(&asked)?;
        assert_eq!(response.body.len(), BOUND, "{framed:?}");
        let (port, _) = serve(framed)?;
        let response = pool()?.fetch(&request(port, "/exact.png")?)?;
        assert_eq!(response.body.len(), BOUND, "{framed:?}, unbounded");
    }
    // One past, the other way.
    let (port, _) = serve(Framed::Length(BOUND + 1))?;
    let asked = request(port, "/over.png")?.reading_at_most(BOUND as u64);
    assert!(pool()?.fetch(&asked).is_err(), "one byte past");
    Ok(())
}

/// Eight kibibytes that gzip undoes to eight mebibytes, frozen (see
/// `tests/compressed/README.md`).
const BOMB: &[u8] = include_bytes!("compressed/bomb.gz");

#[test]
fn gzip_undoing_to_more_than_the_bound_is_refused() -> Result<(), String> {
    assert!(BOMB.len() < BOUND, "{} bytes on the wire", BOMB.len());
    let (port, _) = serve(Framed::Gzip(BOMB))?;
    let asked = request(port, "/zipped.png")?.reading_at_most(BOUND as u64);
    let why = pool()?
        .fetch(&asked)
        .err()
        .ok_or("a gzip bomb was undone")?;
    assert!(why.contains("more than"), "{why}");
    let (port, _) = serve(Framed::Gzip(BOMB))?;
    let whole = pool()?.fetch(&request(port, "/zipped.png")?)?;
    assert_eq!(whole.body.len(), 8 * 1024 * 1024, "unbounded, undone whole");
    Ok(())
}

#[test]
fn the_readers_take_at_most_one_byte_past_the_bound() {
    /// A source that counts what is taken from it.
    struct Counted<'b> {
        bytes: &'b [u8],
        taken: usize,
    }
    impl Read for Counted<'_> {
        fn read(&mut self, into: &mut [u8]) -> std::io::Result<usize> {
            let left = self.bytes.get(self.taken..).unwrap_or_default();
            let taking = left.len().min(into.len());
            if let (Some(to), Some(from)) = (into.get_mut(..taking), left.get(..taking)) {
                to.copy_from_slice(from);
            }
            self.taken += taking;
            Ok(taking)
        }
    }
    let long = vec![7_u8; 10_000];
    let mut source = Counted {
        bytes: &long,
        taken: 0,
    };
    let (body, why) = read_at_most(&mut source, Framing::UntilClose, 100);
    assert_eq!(body.len(), 100);
    assert!(why.is_some_and(|why| why.why.contains("more than the 100")));
    assert_eq!(source.taken, 101, "one byte past, to know it was past");
    let mut source = Counted {
        bytes: &long,
        taken: 0,
    };
    let (body, why) = read_at_most(&mut source, Framing::Exactly(10_000), 100);
    assert!(body.is_empty() && why.is_some());
    assert_eq!(source.taken, 0, "said too long, nothing read");
    let mut source = Counted {
        bytes: &long,
        taken: 0,
    };
    let (body, why) = read_at_most(&mut source, Framing::UntilClose, 10_000);
    assert_eq!((body.len(), why.is_none()), (10_000, true), "exactly at it");
}

// --- HTTP/2 -------------------------------------------------------------------

/// A server speaking HTTP/2 that answers one request with `frames` `DATA`
/// frames of 16 KiB: its port, and how many it sent and whether the client
/// reset the stream with `CANCEL` before it was done.
fn serve_h2(frames: usize) -> Result<(u16, Receiver<(usize, bool)>), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    let (told, heard) = channel();
    std::thread::spawn(move || {
        let Some(Ok(mut socket)) = listener.incoming().next() else {
            return;
        };
        let Some((stream, mut writing)) = greet(&mut socket) else {
            return;
        };
        let block = hpack::encode(&[Field::new(":status", "200")], &mut writing);
        let _ = socket.write_all(&frame::write(&Frame::Headers {
            stream,
            block,
            end_stream: false,
            end_headers: true,
            priority: None,
        }));
        let _ = socket.set_nonblocking(false);
        let mut reader = socket.try_clone().ok();
        let (cancelled_tell, cancelled) = channel();
        std::thread::spawn(move || {
            while let Some(reading) = reader.as_mut() {
                match frame::read(reading, frame::LARGEST_BY_DEFAULT) {
                    Ok(Frame::ResetStream {
                        error: ErrorCode::Cancel,
                        ..
                    }) => {
                        let _ = cancelled_tell.send(());
                        return;
                    }
                    Ok(_) => {}
                    Err(_) => return,
                }
            }
        });
        let mut sent = 0;
        let mut reset = false;
        while sent < frames {
            if cancelled.try_recv().is_ok() {
                reset = true;
                break;
            }
            let written = socket.write_all(&frame::write(&Frame::Data {
                stream,
                data: vec![0x5a; 16 * 1024],
                end_stream: sent + 1 == frames,
            }));
            if written.is_err() {
                break;
            }
            sent += 1;
            // The client's own windows are not overrun: it widens them as it
            // takes the body, and a server waits for that in earnest. Here
            // the frames are small and the pace slow enough for its answers.
            std::thread::sleep(Duration::from_millis(2));
        }
        reset |= cancelled.recv_timeout(Duration::from_secs(2)).is_ok();
        let _ = told.send((sent, reset));
    });
    Ok((port, heard))
}

/// The preface and the settings, and the request's `HEADERS`: its stream
/// and the table to answer with.
fn greet(socket: &mut TcpStream) -> Option<(u32, Table)> {
    let mut preface = vec![0u8; frame::PREFACE.len()];
    socket.read_exact(&mut preface).ok()?;
    if preface != frame::PREFACE {
        return None;
    }
    loop {
        match frame::read(socket, frame::LARGEST_BY_DEFAULT).ok()? {
            Frame::Settings { ack: false, .. } => {
                let _ = socket.write_all(&frame::write(&Frame::Settings {
                    ack: false,
                    values: vec![(Setting::MAX_CONCURRENT_STREAMS, 100)],
                }));
                let _ = socket.write_all(&frame::write(&Frame::Settings {
                    ack: true,
                    values: Vec::new(),
                }));
            }
            Frame::Headers { stream, .. } => return Some((stream, Table::new(4096))),
            _ => {}
        }
    }
}

#[test]
fn over_http_2_a_stream_past_the_bound_is_reset_and_refused() -> Result<(), String> {
    // A gigabyte would be 65536 frames; a thousand, 16 MiB, is past any
    // bound here and over in seconds.
    let (port, heard) = serve_h2(1000)?;
    let asked = request(port, "/large.png")?.reading_at_most(BOUND as u64);
    let mut socket = TcpStream::connect(("127.0.0.1", port)).map_err(|why| why.to_string())?;
    let mut speaking = client::Speaking::new();
    let answered = client::exchange(&mut socket, &mut speaking, &asked);
    let why = answered.err().ok_or("16 MiB were read")?;
    assert!(
        why.why.contains("more than the 262144 this request reads"),
        "{}",
        why.why
    );
    assert_eq!(why.error, ErrorCode::Cancel);
    let (sent, reset) = heard
        .recv_timeout(Duration::from_secs(30))
        .map_err(|why| why.to_string())?;
    assert!(
        reset,
        "the client never reset the stream; {sent} frames sent"
    );
    assert!(sent < 1000, "every frame was sent");
    // At the bound, a body of 16 frames — 256 KiB — is read whole.
    let (port, _) = serve_h2(16)?;
    let mut socket = TcpStream::connect(("127.0.0.1", port)).map_err(|why| why.to_string())?;
    let mut speaking = client::Speaking::new();
    let whole = client::exchange(&mut socket, &mut speaking, &asked).map_err(|why| why.why)?;
    assert_eq!(whole.body.len(), BOUND);
    Ok(())
}
