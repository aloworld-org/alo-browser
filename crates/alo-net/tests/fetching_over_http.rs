/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A whole fetch, over a socket that never leaves this machine.
//!
//! The server is thirty lines in this file rather than a dependency, and it
//! speaks HTTP/1.1 badly on purpose in half the tests — which is the point.
//! Nothing here reaches the network: `127.0.0.1` on a port the operating
//! system picks.

use alo_net::{FetchError, Request, fetch};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

/// Answer one request with these exact bytes, then close.
///
/// Returns the port. The bytes are sent verbatim, so a test can send a
/// response no correct server would.
fn serve(bytes: &'static [u8]) -> u16 {
    // Zero when loopback is unavailable, which every test then fails on with
    // its own message rather than on a panic in a helper.
    let Ok(listener) = TcpListener::bind("127.0.0.1:0") else {
        return 0;
    };
    let Ok(address) = listener.local_addr() else {
        return 0;
    };
    let port = address.port();
    std::thread::spawn(move || {
        let Ok((mut socket, _)) = listener.accept() else {
            return;
        };
        // Read the request and ignore it; every test here asks for one thing.
        let mut swallow = [0u8; 4096];
        let _ = socket.read(&mut swallow);
        let _ = socket.write_all(bytes);
        let _ = socket.flush();
    });
    port
}

/// The same, and hand back the request bytes it was sent.
///
/// It reads until the client goes quiet rather than once, because a request
/// with a body is a head and then some bytes, and nothing promises they arrive
/// in one piece.
fn serve_and_hear(bytes: &'static [u8]) -> (u16, mpsc::Receiver<Vec<u8>>) {
    let (say, heard) = mpsc::channel();
    let Ok(listener) = TcpListener::bind("127.0.0.1:0") else {
        return (0, heard);
    };
    let Ok(address) = listener.local_addr() else {
        return (0, heard);
    };
    let port = address.port();
    std::thread::spawn(move || {
        let Ok((mut socket, _)) = listener.accept() else {
            return;
        };
        if socket
            .set_read_timeout(Some(Duration::from_millis(400)))
            .is_err()
        {
            return;
        }
        let mut asked = Vec::new();
        let mut block = [0u8; 4096];
        while let Ok(got) = socket.read(&mut block) {
            if got == 0 {
                break;
            }
            asked.extend_from_slice(block.get(..got).unwrap_or_default());
        }
        let _ = say.send(asked);
        let _ = socket.write_all(bytes);
        let _ = socket.flush();
    });
    (port, heard)
}

fn get(port: u16, path: &str) -> Result<alo_net::Response, FetchError> {
    fetch(&Request::get(at(port, path)))
}

fn at(port: u16, path: &str) -> alo_url::Url {
    let text = format!("http://127.0.0.1:{port}{path}");
    alo_url::parse(&text).unwrap_or_else(|_| alo_url::Url {
        scheme: "about".to_owned(),
        host: None,
        port: None,
        path: "not-a-url".to_owned(),
        query: None,
        fragment: None,
        serialised: "about:not-a-url".to_owned(),
    })
}

#[test]
fn a_page_fetched_over_http_arrives_as_a_page() {
    let port = serve(
        b"HTTP/1.1 200 OK\r\n\
          Content-Type: text/html; charset=utf-8\r\n\
          Content-Length: 25\r\n\
          \r\n\
          <!DOCTYPE html><p>hi</p>\n",
    );
    let response = get(port, "/").expect("a response");
    assert!(response.status.is_ok());
    assert!(response.media_type().is_some_and(|held| held.is_html()));
    assert_eq!(response.text().text.trim(), "<!DOCTYPE html><p>hi</p>");
}

#[test]
fn a_chunked_page_arrives_as_the_bytes_it_adds_up_to() {
    let port = serve(
        b"HTTP/1.1 200 OK\r\n\
          Content-Type: text/html\r\n\
          Transfer-Encoding: chunked\r\n\
          \r\n\
          8\r\n<p>hello\r\n5\r\n</p>!\r\n0\r\n\r\n",
    );
    let response = get(port, "/").expect("a response");
    assert_eq!(response.text().text, "<p>hello</p>!");
}

#[test]
fn a_server_that_says_no_is_a_response_rather_than_a_failure() {
    // The difference matters to everything above: a page for one, a message
    // about the browser for the other.
    let port = serve(b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nnot here!");
    let response = get(port, "/missing").expect("a response, not an error");
    assert!(response.status.is_error());
    assert_eq!(response.status.0, 404);
    assert_eq!(response.text().text, "not here!");
}

#[test]
fn a_server_that_stops_halfway_is_a_failure_rather_than_a_short_page() {
    let port = serve(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nonly this much");
    match get(port, "/") {
        Err(FetchError::Failed { why, .. }) => {
            assert!(why.contains("stopped after"), "{why}");
        }
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn a_server_that_smuggles_is_refused_before_a_page_is_made_of_it() {
    let port = serve(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nContent-Length: 100\r\n\r\nhello");
    match get(port, "/") {
        Err(FetchError::Failed { why, .. }) => assert!(why.contains("disagree"), "{why}"),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_server_that_answers_with_nonsense_is_an_error_with_a_reason() {
    let port = serve(b"this is not HTTP at all\r\n\r\n");
    match get(port, "/") {
        Err(FetchError::Failed { why, .. }) => {
            assert!(why.contains("status line"), "{why}");
        }
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn a_server_that_answers_with_nothing_is_an_error_rather_than_an_empty_page() {
    let port = serve(b"");
    assert!(get(port, "/").is_err(), "an empty answer is not a page");
}

// ---- a request that sends something ----

#[test]
fn a_body_goes_out_after_the_head_with_the_length_that_describes_it() {
    let (port, heard) = serve_and_hear(b"HTTP/1.1 201 Created\r\nContent-Length: 2\r\n\r\nok");
    assert!(port != 0, "no server");
    let response = fetch(&Request::sending(
        at(port, "/form"),
        "POST",
        b"name=ada&trade=engines".to_vec(),
    ))
    .expect("a response");
    assert_eq!(response.status.0, 201);

    let asked = String::from_utf8(heard.recv().unwrap_or_default()).unwrap_or_default();
    assert!(asked.starts_with("POST /form HTTP/1.1\r\n"), "{asked:?}");
    assert!(asked.contains("Content-Length: 22\r\n"), "{asked:?}");
    assert!(
        asked.ends_with("\r\n\r\nname=ada&trade=engines"),
        "the body did not follow the blank line: {asked:?}"
    );
}

/// The difference between a `POST` that sends nothing and a `POST` a server is
/// still waiting on.
#[test]
fn a_method_that_anticipates_content_says_zero_rather_than_nothing() {
    let (port, heard) = serve_and_hear(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n");
    assert!(port != 0, "no server");
    let _ = fetch(&Request::sending(at(port, "/ping"), "POST", Vec::new()));

    let asked = String::from_utf8(heard.recv().unwrap_or_default()).unwrap_or_default();
    assert!(asked.contains("Content-Length: 0\r\n"), "{asked:?}");
}

/// An interim response is not the response, and a client has to read one
/// whether or not it asked for anything: `103 Early Hints` is sent unprompted.
#[test]
fn an_interim_response_is_read_past_rather_than_taken_for_the_answer() {
    let port = serve(
        b"HTTP/1.1 103 Early Hints\r\n\
          Link: </a.css>; rel=preload\r\n\
          \r\n\
          HTTP/1.1 200 OK\r\n\
          Content-Length: 9\r\n\
          \r\n\
          the page!",
    );
    let response = get(port, "/").expect("a response");
    assert_eq!(response.status.0, 200, "an early hint became the page");
    assert_eq!(response.text().text, "the page!");
    assert_eq!(
        response.headers.get("Link"),
        None,
        "an interim response's headers became the answer's"
    );
}

/// A head with no body costs a server almost nothing to send.
#[test]
fn a_server_that_only_ever_says_something_first_is_refused() {
    let mut bytes = String::new();
    for _ in 0..20 {
        bytes.push_str("HTTP/1.1 103 Early Hints\r\n\r\n");
    }
    bytes.push_str("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi");
    // Leaked on purpose: `serve` sends bytes that outlive this call, and a test
    // process ending is what reclaims them.
    let port = serve(Box::leak(bytes.into_boxed_str()).as_bytes());
    match get(port, "/") {
        Err(FetchError::Failed { why, .. }) => assert!(why.contains("interim"), "{why}"),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// `Expect` is a promise that the sender will wait, and nothing here can bound
/// the waiting — so it is refused by name rather than sent and not honoured.
#[test]
fn an_expectation_is_refused_by_name() {
    let (port, _) = serve_and_hear(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
    assert!(port != 0, "no server");
    let mut request = Request::sending(at(port, "/upload"), "POST", b"a form".to_vec());
    request.headers.add("Expect", "100-continue");
    match fetch(&request) {
        Err(FetchError::Failed { why, .. }) => assert!(why.contains("100-continue"), "{why}"),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_host_nothing_is_listening_on_says_so() {
    // Port 1 on loopback, where nothing is. No name is looked up, so this
    // needs no network and no DNS.
    let url = alo_url::parse("http://127.0.0.1:1/").expect("a URL");
    match fetch(&Request::get(url)) {
        Err(FetchError::Failed { why, .. }) => assert!(why.contains("could not reach"), "{why}"),
        other => panic!("expected a failure, got {other:?}"),
    }
}
