/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A request that sends something, over HTTP/2.
//!
//! Until this, every request went out with `END_STREAM` on its `HEADERS` —
//! truthful, and no `POST`. What is under test here is the half of the protocol
//! that only a body reaches: `DATA` frames cut to the peer's frame size, a
//! window that shuts part way through and is **waited on** rather than
//! overrun, and what happens when the answer comes before the request has
//! finished going out.
//!
//! The server is a hundred lines that speak just enough of the protocol to
//! collect one request. It sets a read timeout on its own socket and treats a
//! timeout as *the client has stopped of its own accord* — which is exactly the
//! condition the window test needs to be able to see, and which is why it is
//! written here rather than reasoned about.

use alo_net::cause::{Cause, Identities};
use alo_net::h2::frame::{self, Frame, Setting};
use alo_net::h2::hpack::{self, Field, Table};
use alo_net::h2::{ErrorCode, client};
use alo_net::{Request, Status};
use std::io::{Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::time::Duration;

/// What caused every request in this file: a person, in a tab of their own.
///
/// ADR 0012 § 1 makes the cause an argument rather than something a caller may
/// forget, so a test has to say what it means too — and what these mean is
/// somebody opening a page. The same tab each time, because it is one person.
fn a_person() -> Cause {
    Cause::Person {
        tab: Identities::default().a_tab(),
    }
}

/// Long enough that a client with more to send has sent it, short enough that a
/// test that is going to fail does not take all afternoon.
const QUIET: Duration = Duration::from_millis(400);

/// What both windows start at, before anybody says otherwise. A body larger
/// than this is a body that cannot go out in one breath, whatever the server
/// later allows.
const AT_FIRST: usize = 65_535;

fn url(text: &str) -> alo_url::Url {
    alo_url::parse(text).unwrap_or_else(|_| alo_url::Url {
        scheme: "about".to_owned(),
        host: None,
        port: None,
        path: "not-a-url".to_owned(),
        query: None,
        fragment: None,
        serialised: "about:not-a-url".to_owned(),
    })
}

/// What the server was sent, as it saw it.
#[derive(Debug, Default)]
struct Seen {
    /// The request's decoded header block.
    fields: Vec<Field>,
    /// The body, joined.
    body: Vec<u8>,
    /// The size of each `DATA` frame, in the order they arrived.
    frames: Vec<usize>,
    /// How many body bytes had arrived when the client stopped sending of its
    /// own accord — which is to say, before this server made any more room.
    before_room: usize,
    /// Whether the request ever ended.
    ended: bool,
    /// Whether the client gave up on the stream.
    reset: bool,
}

/// A server that collects one request and answers it.
///
/// `answer_first` makes it reply the moment the headers are in, without reading
/// the body — a `413` in the wild, and the case where the rest of the request is
/// bytes nobody wants.
fn serve(status: &'static str, answer_first: bool) -> (u16, mpsc::Receiver<Seen>) {
    let (say, heard) = mpsc::channel();
    let Ok(listener) = TcpListener::bind("127.0.0.1:0") else {
        return (0, heard);
    };
    let Ok(address) = listener.local_addr() else {
        return (0, heard);
    };
    let port = address.port();

    std::thread::spawn(move || {
        let Some(Ok(mut socket)) = listener.incoming().next() else {
            return;
        };
        if socket.set_read_timeout(Some(QUIET)).is_err() {
            return;
        }
        let mut preface = vec![0u8; frame::PREFACE.len()];
        if socket.read_exact(&mut preface).is_err() || preface != frame::PREFACE {
            return;
        }

        let mut reading = Table::new(4096);
        let mut writing = Table::new(4096);
        let mut seen = Seen::default();
        let stream = loop {
            match frame::read(&mut socket, frame::LARGEST_BY_DEFAULT) {
                Ok(Frame::Settings { ack: false, .. }) => {
                    let _ = socket.write_all(&frame::write(&Frame::Settings {
                        ack: false,
                        values: vec![(Setting::MAX_CONCURRENT_STREAMS, 100)],
                    }));
                    let _ = socket.write_all(&frame::write(&Frame::Settings {
                        ack: true,
                        values: Vec::new(),
                    }));
                }
                Ok(Frame::Headers {
                    stream: on,
                    block,
                    end_stream,
                    ..
                }) => {
                    seen.fields = hpack::decode(&block, &mut reading).unwrap_or_default();
                    seen.ended = end_stream;
                    break on;
                }
                // Anything else before the request is this server's business
                // and nothing the tests assert on.
                Ok(_) => {}
                Err(_) => return,
            }
        };

        if answer_first {
            answer(&mut socket, &mut writing, stream, status);
        }

        let mut made_room = false;
        while !seen.ended {
            match frame::read(&mut socket, frame::LARGEST_BY_DEFAULT) {
                Ok(Frame::Data {
                    data, end_stream, ..
                }) => {
                    seen.frames.push(data.len());
                    seen.body.extend_from_slice(&data);
                    seen.ended = end_stream;
                }
                Ok(Frame::ResetStream { .. }) => {
                    seen.reset = true;
                    break;
                }
                Ok(_) => {}
                Err(_) => {
                    // Nothing more is coming on its own: either the window is
                    // shut or the client has finished with this stream. Making
                    // room once tells the two apart — a client that was waiting
                    // sends the rest, and one that was not stays quiet.
                    if made_room {
                        break;
                    }
                    seen.before_room = seen.body.len();
                    made_room = true;
                    for on in [0, stream] {
                        let _ = socket.write_all(&frame::write(&Frame::WindowUpdate {
                            stream: on,
                            increase: 1_000_000,
                        }));
                    }
                    let _ = socket.flush();
                }
            }
        }

        if !answer_first {
            answer(&mut socket, &mut writing, stream, status);
        }
        let _ = say.send(seen);
        // Stay alive until the client is finished reading.
        std::thread::sleep(Duration::from_millis(200));
    });
    (port, heard)
}

fn answer(socket: &mut TcpStream, writing: &mut Table, stream: u32, status: &str) {
    let block = hpack::encode(&[Field::new(":status", status)], writing);
    let _ = socket.write_all(&frame::write(&Frame::Headers {
        stream,
        block,
        end_stream: true,
        end_headers: true,
        priority: None,
    }));
    let _ = socket.flush();
}

fn send(port: u16, request: &Request) -> Result<alo_net::Response, alo_net::h2::frame::Broken> {
    let mut socket =
        TcpStream::connect(("127.0.0.1", port)).map_err(|why| alo_net::h2::frame::Broken {
            why: format!("connect: {why}"),
            error: ErrorCode::InternalError,
            fatal: true,
        })?;
    let mut speaking = client::Speaking::new();
    client::exchange(&mut socket, &mut speaking, request)
}

fn value<'a>(seen: &'a Seen, name: &str) -> Option<&'a str> {
    seen.fields
        .iter()
        .find(|field| field.name == name)
        .map(|field| field.value.as_str())
}

// --- A body goes out ---------------------------------------------------------

#[test]
fn a_body_goes_out_in_data_frames_and_arrives_as_it_was_written() {
    let (port, heard) = serve("200", false);
    assert!(port != 0, "no server");
    let sent = b"name=ada&trade=engines".to_vec();

    let response = send(
        port,
        &Request::sending(
            url(&format!("http://127.0.0.1:{port}/form")),
            "POST",
            sent.clone(),
            a_person(),
        ),
    )
    .unwrap_or_else(|why| panic!("the exchange failed: {why}"));
    assert_eq!(response.status, Status(200));

    let seen = heard.recv().unwrap_or_default();
    assert_eq!(seen.body, sent, "the body did not arrive as it was written");
    assert!(seen.ended, "the request never ended");
    assert_eq!(value(&seen, ":method"), Some("POST"));
    assert_eq!(
        seen.frames.len(),
        1,
        "a body this size is one frame: {:?}",
        seen.frames
    );
}

/// Optional in HTTP/2, where `END_STREAM` is what frames a body — and, when it
/// is there, it has to agree with the bytes. So it is written from the bytes,
/// and a caller's is dropped: a header and a body disagreeing about where a
/// message ends is the request half of request smuggling.
#[test]
fn the_length_that_goes_out_is_the_bodys_rather_than_a_callers() {
    let (port, heard) = serve("200", false);
    assert!(port != 0, "no server");
    let mut request = Request::sending(
        url(&format!("http://127.0.0.1:{port}/form")),
        "POST",
        b"1234567890".to_vec(),
        a_person(),
    );
    request.headers.add("Content-Length", "3");
    let _ = send(port, &request);

    let seen = heard.recv().unwrap_or_default();
    assert_eq!(value(&seen, "content-length"), Some("10"));
    assert_eq!(
        seen.fields
            .iter()
            .filter(|field| field.name == "content-length")
            .count(),
        1,
        "two lengths went out, which is a message saying two things"
    );
    assert_eq!(seen.body.len(), 10);
}

/// A `GET` says nothing about a length it does not have, and its `HEADERS`
/// carries `END_STREAM` — which is the truthful way to say there is no body.
#[test]
fn a_request_with_nothing_to_send_still_ends_with_its_headers() {
    let (port, heard) = serve("200", false);
    assert!(port != 0, "no server");
    let _ = send(
        port,
        &Request::get(url(&format!("http://127.0.0.1:{port}/")), a_person()),
    );

    let seen = heard.recv().unwrap_or_default();
    assert!(seen.ended, "a GET did not end with its headers");
    assert!(seen.body.is_empty());
    assert_eq!(value(&seen, "content-length"), None);
}

/// A frame larger than the peer said it would read is a `FRAME_SIZE_ERROR`, and
/// one on a `DATA` frame ends the connection rather than the stream.
#[test]
fn a_body_larger_than_one_frame_is_cut_to_the_frame_size() {
    let (port, heard) = serve("200", false);
    assert!(port != 0, "no server");
    let sent: Vec<u8> = (0..100_000u32).map(|at| (at % 251) as u8).collect();

    let response = send(
        port,
        &Request::sending(
            url(&format!("http://127.0.0.1:{port}/upload")),
            "POST",
            sent.clone(),
            a_person(),
        ),
    )
    .unwrap_or_else(|why| panic!("the exchange failed: {why}"));
    assert_eq!(response.status, Status(200));

    let seen = heard.recv().unwrap_or_default();
    assert_eq!(seen.body, sent, "a hundred kilobytes did not survive");
    let largest = frame::LARGEST_BY_DEFAULT as usize;
    assert!(
        seen.frames.iter().all(|size| *size <= largest),
        "a frame past what the peer accepts: {:?}",
        seen.frames
    );
    assert!(
        seen.frames.len() > 1,
        "a hundred kilobytes went in one frame"
    );
}

/// The clause the item was written for. Both windows start at sixty-four
/// kilobytes and nothing may go past them until the server says so, so a body
/// larger than one is a body that stops half way — and stopping is the correct
/// behaviour rather than a stall.
#[test]
fn a_window_that_closes_mid_body_is_waited_on_rather_than_overrun() {
    let (port, heard) = serve("200", false);
    assert!(port != 0, "no server");
    let sent: Vec<u8> = (0..100_000u32).map(|at| (at % 251) as u8).collect();

    let response = send(
        port,
        &Request::sending(
            url(&format!("http://127.0.0.1:{port}/upload")),
            "POST",
            sent.clone(),
            a_person(),
        ),
    )
    .unwrap_or_else(|why| panic!("the exchange failed: {why}"));
    assert_eq!(response.status, Status(200));

    let seen = heard.recv().unwrap_or_default();
    assert_eq!(
        seen.before_room, AT_FIRST,
        "before this server made any more room, {} bytes had arrived against a window of \
         {AT_FIRST} — under it is a client that stalled, over it is one that overran",
        seen.before_room
    );
    assert_eq!(seen.body, sent, "the rest never came");
    assert!(seen.ended);
}

/// A server may answer before it has read the request — a `413`, a redirect, a
/// server that had made its mind up. The rest of the body is then bytes nobody
/// wants, and a stream this engine simply stopped writing to would stay open
/// until the connection ended.
#[test]
fn an_answer_before_the_body_is_finished_ends_the_stream_rather_than_leaving_it_open() {
    let (port, heard) = serve("413", true);
    assert!(port != 0, "no server");
    let sent = vec![b'x'; 100_000];

    let response = send(
        port,
        &Request::sending(
            url(&format!("http://127.0.0.1:{port}/upload")),
            "POST",
            sent,
            a_person(),
        ),
    )
    .unwrap_or_else(|why| panic!("the exchange failed: {why}"));
    assert_eq!(response.status, Status(413), "the answer was not read");

    let seen = heard.recv().unwrap_or_default();
    assert!(
        seen.reset,
        "the request was abandoned without saying so, leaving a stream open for ever"
    );
    assert!(
        seen.body.len() <= AT_FIRST,
        "more went out than the window allowed"
    );
}

// --- What this engine will not promise ---------------------------------------

/// `Expect` is a promise that the sender will wait, and nothing here can bound
/// the waiting — so it is refused by name rather than sent and not honoured.
/// Nothing on the web can reach this: Fetch forbids the header to scripts.
#[test]
fn an_expectation_is_refused_by_name_and_nothing_is_sent() {
    let mut request = Request::sending(
        url("http://127.0.0.1:1/upload"),
        "POST",
        b"a form".to_vec(),
        a_person(),
    );
    request.headers.add("Expect", "100-continue");

    // No server, and none needed: the refusal happens before a byte is written,
    // which is what this asserts by holding the wire afterwards.
    let mut wire = Cursor::new(Vec::new());
    let mut speaking = client::Speaking::new();
    let why = client::exchange(&mut wire, &mut speaking, &request)
        .err()
        .unwrap_or_else(|| panic!("an expectation this engine cannot keep was accepted"));

    assert!(why.why.contains("100-continue"), "{:?}", why.why);
    assert!(!why.fatal, "a refusal of ours is not a broken connection");
    assert!(
        wire.get_ref().is_empty(),
        "the connection was written to before the request was refused"
    );
}

#[test]
fn an_expectation_is_refused_whatever_it_asks_for() {
    let mut request = Request::sending(
        url("http://127.0.0.1:1/"),
        "POST",
        b"x".to_vec(),
        a_person(),
    );
    request.headers.add("Expect", "the-moon-on-a-stick");
    let mut wire = Cursor::new(Vec::new());
    let mut speaking = client::Speaking::new();
    assert!(
        client::exchange(&mut wire, &mut speaking, &request).is_err(),
        "an expectation nobody has heard of was sent as though it would be kept"
    );
}
