//! A request and a response in HTTP/2, over a socket that never leaves this
//! machine.
//!
//! The server here is a few dozen lines that speak just enough of the protocol
//! to answer once. What is under test is the client: that it says hello before
//! it asks, that a request becomes four pseudo-headers and then the rest, that a
//! response is assembled out of frames, and that it answers the things a peer
//! waits for rather than deadlocking against them.

use alo_net::h2::frame::{self, Frame, Setting};
use alo_net::h2::hpack::{self, Field, Table};
use alo_net::h2::{ErrorCode, client};
use alo_net::{Request, Status};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;

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

/// Read exactly this many bytes, or give up.
fn exactly(socket: &mut TcpStream, how_many: usize) -> Option<Vec<u8>> {
    let mut got = vec![0u8; how_many];
    socket.read_exact(&mut got).ok()?;
    Some(got)
}

/// A server that answers one request, and hands back what the client sent it.
///
/// `answer` builds the response headers and body from the request's fields, so
/// a test can assert on what actually went out rather than on what it hoped did.
fn serve(
    answer: impl Fn(&[Field]) -> (Vec<Field>, Vec<u8>) + Send + 'static,
) -> (u16, mpsc::Receiver<Vec<Field>>) {
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
        // The preface, then whatever frames arrive until a HEADERS.
        if exactly(&mut socket, frame::PREFACE.len()).as_deref() != Some(frame::PREFACE) {
            return;
        }
        let mut reading = Table::new(4096);
        let mut writing = Table::new(4096);
        // Set by the HEADERS that ends the loop; there is no sensible value
        // before one arrives, which is what the `Option` says.
        let asked;
        let stream;
        loop {
            let Ok(got) = frame::read(&mut socket, frame::LARGEST_BY_DEFAULT) else {
                return;
            };
            match got {
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
                Frame::Headers {
                    stream: on, block, ..
                } => {
                    let Ok(fields) = hpack::decode(&block, &mut reading) else {
                        return;
                    };
                    asked = fields;
                    stream = on;
                    break;
                }
                _ => {}
            }
        }
        let _ = say.send(asked.clone());

        let (headers, body) = answer(&asked);
        let block = hpack::encode(&headers, &mut writing);
        let _ = socket.write_all(&frame::write(&Frame::Headers {
            stream,
            block,
            end_stream: body.is_empty(),
            end_headers: true,
            priority: None,
        }));
        if !body.is_empty() {
            let _ = socket.write_all(&frame::write(&Frame::Data {
                stream,
                data: body,
                end_stream: true,
            }));
        }
        let _ = socket.flush();
        // Stay alive until the client is finished reading.
        std::thread::sleep(std::time::Duration::from_millis(200));
    });
    (port, heard)
}

fn ok_with(body: &'static str) -> impl Fn(&[Field]) -> (Vec<Field>, Vec<u8>) + Send + 'static {
    move |_| {
        (
            vec![
                Field::new(":status", "200"),
                Field::new("content-type", "text/html"),
            ],
            body.as_bytes().to_vec(),
        )
    }
}

fn ask(port: u16, request: &Request) -> Result<alo_net::Response, String> {
    let mut socket =
        TcpStream::connect(("127.0.0.1", port)).map_err(|why| format!("connect: {why}"))?;
    let mut speaking = client::Speaking::new();
    client::exchange(&mut socket, &mut speaking, request).map_err(|why| why.why)
}

// --- A request and a response ------------------------------------------------

#[test]
fn a_request_and_its_response_go_through() {
    let (port, heard) = serve(ok_with("<p>over two</p>"));
    assert!(port != 0, "no server");

    let response = ask(
        port,
        &Request::get(url(&format!("http://127.0.0.1:{port}/a/b?c=d"))),
    )
    .unwrap_or_else(|why| panic!("the exchange failed: {why}"));

    assert_eq!(response.status, Status(200));
    assert_eq!(response.body, b"<p>over two</p>");
    assert_eq!(response.headers.get("content-type"), Some("text/html"));

    let asked = heard.recv().unwrap_or_default();
    assert_eq!(asked.first().map(|f| f.name.as_str()), Some(":method"));
}

/// There is no request line. The method, scheme, path and authority are headers
/// whose names begin with a colon — a character no ordinary header name may
/// contain, which is what makes them impossible to forge from an ordinary one.
#[test]
fn a_request_becomes_four_pseudo_headers_and_they_come_first() {
    let (port, heard) = serve(ok_with("x"));
    assert!(port != 0, "no server");
    let _ = ask(
        port,
        &Request::get(url(&format!("http://127.0.0.1:{port}/a/b?c=d"))),
    );

    let asked = heard.recv().unwrap_or_default();
    let names: Vec<&str> = asked.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names.first().copied().map(|n| n.starts_with(':')),
        Some(true)
    );
    let first_ordinary = names.iter().position(|name| !name.starts_with(':'));
    let last_pseudo = names.iter().rposition(|name| name.starts_with(':'));
    assert!(
        first_ordinary.is_none() || last_pseudo < first_ordinary,
        "a pseudo-header came after an ordinary one: {names:?}"
    );

    let value = |name: &str| {
        asked
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.value.clone())
            .unwrap_or_default()
    };
    assert_eq!(value(":method"), "GET");
    assert_eq!(value(":scheme"), "http");
    assert_eq!(value(":path"), "/a/b?c=d", "the query is part of the path");
    assert_eq!(value(":authority"), format!("127.0.0.1:{port}"));
}

/// `Connection`, `Host` and their friends describe one hop of an HTTP/1.1
/// connection. HTTP/2 has its own way of saying all of it, and a server
/// receiving one must treat the message as malformed — so sending one is not a
/// compatibility gesture, it is a broken request.
#[test]
fn the_headers_that_describe_a_hop_are_not_sent() {
    let (port, heard) = serve(ok_with("x"));
    assert!(port != 0, "no server");
    let mut request = Request::get(url(&format!("http://127.0.0.1:{port}/")));
    request.headers.add("Connection", "keep-alive");
    request.headers.add("Host", "somewhere.else");
    request.headers.add("Accept", "text/html");
    let _ = ask(port, &request);

    let asked = heard.recv().unwrap_or_default();
    let names: Vec<&str> = asked.iter().map(|f| f.name.as_str()).collect();
    assert!(!names.contains(&"connection"), "{names:?}");
    assert!(
        !names.contains(&"host"),
        "Host is :authority here: {names:?}"
    );
    assert!(names.contains(&"accept"), "an ordinary header was dropped");
    assert_eq!(
        asked
            .iter()
            .find(|f| f.name == ":authority")
            .map(|f| f.value.as_str()),
        Some(format!("127.0.0.1:{port}").as_str()),
        "a caller's Host was allowed to become the authority"
    );
}

/// A name with a capital in it is malformed, not merely unconventional.
#[test]
fn header_names_go_out_lowercase() {
    let (port, heard) = serve(ok_with("x"));
    assert!(port != 0, "no server");
    let mut request = Request::get(url(&format!("http://127.0.0.1:{port}/")));
    request.headers.add("X-Custom-Thing", "1");
    let _ = ask(port, &request);

    let asked = heard.recv().unwrap_or_default();
    assert!(
        asked.iter().all(|f| f.name == f.name.to_lowercase()),
        "a capital went out: {:?}",
        asked.iter().map(|f| f.name.clone()).collect::<Vec<_>>()
    );
}

/// The same rule ADR 0007 applies to cookies, applied to compression: a
/// credential is never put in a table, ours or any relay's.
#[test]
fn a_credential_is_marked_never_indexed() {
    let (port, heard) = serve(ok_with("x"));
    assert!(port != 0, "no server");
    let mut request = Request::get(url(&format!("http://127.0.0.1:{port}/")));
    request.headers.add("Authorization", "Bearer a-real-token");
    request.headers.add("Accept", "text/html");
    let _ = ask(port, &request);

    let asked = heard.recv().unwrap_or_default();
    let secret = asked.iter().find(|f| f.name == "authorization");
    assert_eq!(
        secret.map(|f| f.never_indexed),
        Some(true),
        "a bearer token was compressible into a shared table"
    );
    assert_eq!(
        asked
            .iter()
            .find(|f| f.name == "accept")
            .map(|f| f.never_indexed),
        Some(false),
        "an ordinary header should still be indexable"
    );
}

// --- What a malformed response is --------------------------------------------

#[test]
fn a_response_with_no_status_is_refused() {
    let (port, _) = serve(|_| (vec![Field::new("content-type", "text/html")], b"x".to_vec()));
    assert!(port != 0, "no server");
    let why = ask(
        port,
        &Request::get(url(&format!("http://127.0.0.1:{port}/"))),
    )
    .err()
    .unwrap_or_default();
    assert!(why.contains(":status"), "{why:?}");
}

/// A pseudo-header after an ordinary one is how a message is smuggled past
/// something that only reads the first few headers.
#[test]
fn a_header_before_the_status_is_refused() {
    let (port, _) = serve(|_| {
        (
            vec![
                Field::new("content-type", "text/html"),
                Field::new(":status", "200"),
            ],
            b"x".to_vec(),
        )
    });
    assert!(port != 0, "no server");
    let why = ask(
        port,
        &Request::get(url(&format!("http://127.0.0.1:{port}/"))),
    )
    .err()
    .unwrap_or_default();
    assert!(why.contains("before :status"), "{why:?}");
}

#[test]
fn a_response_carrying_a_request_pseudo_header_is_refused() {
    let (port, _) = serve(|_| {
        (
            vec![Field::new(":status", "200"), Field::new(":method", "GET")],
            b"x".to_vec(),
        )
    });
    assert!(port != 0, "no server");
    let why = ask(
        port,
        &Request::get(url(&format!("http://127.0.0.1:{port}/"))),
    )
    .err()
    .unwrap_or_default();
    assert!(why.contains("only a request may have"), "{why:?}");
}

#[test]
fn a_response_carrying_a_hop_header_is_refused() {
    let (port, _) = serve(|_| {
        (
            vec![
                Field::new(":status", "200"),
                Field::new("transfer-encoding", "chunked"),
            ],
            b"x".to_vec(),
        )
    });
    assert!(port != 0, "no server");
    let why = ask(
        port,
        &Request::get(url(&format!("http://127.0.0.1:{port}/"))),
    )
    .err()
    .unwrap_or_default();
    assert!(why.contains("HTTP/2 forbids"), "{why:?}");
}

/// A status that is not a number is not a status, and must not become one.
#[test]
fn a_status_that_is_not_a_number_is_refused() {
    let (port, _) = serve(|_| (vec![Field::new(":status", "OK")], b"x".to_vec()));
    assert!(port != 0, "no server");
    let why = ask(
        port,
        &Request::get(url(&format!("http://127.0.0.1:{port}/"))),
    )
    .err()
    .unwrap_or_default();
    assert!(why.contains("not a number"), "{why:?}");
}

// --- The error codes are the protocol's own ---------------------------------

#[test]
fn a_malformed_response_ends_the_stream_rather_than_the_connection() {
    let (port, _) = serve(|_| (vec![Field::new(":status", "OK")], b"x".to_vec()));
    assert!(port != 0, "no server");
    let mut socket = TcpStream::connect(("127.0.0.1", port)).unwrap_or_else(|why| panic!("{why}"));
    let mut speaking = client::Speaking::new();
    let broken = client::exchange(
        &mut socket,
        &mut speaking,
        &Request::get(url(&format!("http://127.0.0.1:{port}/"))),
    );
    let Err(why) = broken else {
        panic!("a response with a non-numeric status was accepted");
    };
    assert_eq!(why.error, ErrorCode::ProtocolError);
    assert!(
        !why.fatal,
        "one malformed response should not end a connection carrying other streams"
    );
}
