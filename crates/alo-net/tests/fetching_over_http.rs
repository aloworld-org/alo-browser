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

fn get(port: u16, path: &str) -> Result<alo_net::Response, FetchError> {
    let text = format!("http://127.0.0.1:{port}{path}");
    let url = alo_url::parse(&text).unwrap_or_else(|_| alo_url::Url {
        scheme: "about".to_owned(),
        host: None,
        port: None,
        path: "not-a-url".to_owned(),
        query: None,
        fragment: None,
        serialised: "about:not-a-url".to_owned(),
    });
    fetch(&Request::get(url))
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
