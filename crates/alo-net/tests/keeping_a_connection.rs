/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Connections kept between requests, and the race that comes with keeping
//! them.
//!
//! A kept connection can be closed by the server at any moment and there is no
//! way to be told, so every reuse is a bet. This is about losing it safely.
//!
//! The servers here are a few lines each, on `127.0.0.1`, and they misbehave
//! deliberately: one closes after a single answer, one closes while a
//! connection sits idle, and one takes a request and then dies without
//! answering. Nothing reaches the network.

use alo_net::cause::{Cause, Identities};
use alo_net::{Pool, Request, Trust};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// A response with a body, framed by length so a connection survives it.
fn ok(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
}

/// Read one request off a socket, so the next answer is not written into an
/// unread one.
fn swallow_a_request(socket: &mut TcpStream) -> bool {
    let mut seen = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match socket.read(&mut byte) {
            Ok(0) | Err(_) => return false,
            Ok(_) => {}
        }
        seen.push(byte.first().copied().unwrap_or(0));
        if seen.ends_with(b"\r\n\r\n") {
            return true;
        }
        if seen.len() > 16 * 1024 {
            return false;
        }
    }
}

/// Start a server. `behaviour` is handed each accepted socket and a counter of
/// how many sockets have been accepted so far.
fn serve(behaviour: impl Fn(TcpStream, usize) + Send + 'static) -> (u16, Arc<AtomicUsize>) {
    let sockets = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&sockets);
    let Ok(listener) = TcpListener::bind("127.0.0.1:0") else {
        return (0, sockets);
    };
    let Ok(address) = listener.local_addr() else {
        return (0, sockets);
    };
    let port = address.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(socket) = socket else { return };
            let which = counted.fetch_add(1, Ordering::SeqCst);
            behaviour(socket, which);
        }
    });
    (port, sockets)
}

fn url(port: u16, path: &str) -> alo_url::Url {
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

fn pool() -> Pool {
    // Trusting nobody is fine: nothing here is `https`, and it avoids reading
    // the machine's certificate store for a test that does not need it. Half a
    // second of patience rather than a browser's thirty, so that the test
    // about a server which never answers takes half a second rather than
    // making the whole suite wait thirty.
    Pool::with_trust(Trust::of(&[]).unwrap_or_else(|_| unreachable_trust()))
        .patient_for(std::time::Duration::from_millis(500))
}

fn unreachable_trust() -> Trust {
    // `Trust::of(&[])` cannot fail; this is the shape the type asks for.
    Trust::of(&[]).unwrap_or_else(|_| unreachable_trust())
}

#[test]
fn two_fetches_of_one_host_use_one_socket() {
    // The point of the whole item: opening a socket costs a round trip, and a
    // page asks for thirty things from the same host.
    let (port, sockets) = serve(|mut socket, _| {
        // Answer for as long as this connection keeps asking.
        while swallow_a_request(&mut socket) {
            if socket.write_all(ok("hello").as_bytes()).is_err() {
                return;
            }
            let _ = socket.flush();
        }
    });

    let mut pool = pool();
    for _ in 0..3 {
        let response = pool
            .fetch(&Request::get(url(port, "/"), a_person()))
            .expect("a response");
        assert_eq!(response.text().text, "hello");
    }
    assert_eq!(
        sockets.load(Ordering::SeqCst),
        1,
        "one socket, three requests"
    );
    assert_eq!(pool.reused(), 2, "the second and third were on the first");
    assert_eq!(pool.idle(), 1, "and it is still being kept");
}

#[test]
fn a_server_that_asks_to_close_is_not_kept() {
    let (port, sockets) = serve(|mut socket, _| {
        if swallow_a_request(&mut socket) {
            let _ = socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nhi");
            let _ = socket.flush();
        }
    });

    let mut pool = pool();
    for _ in 0..2 {
        assert!(
            pool.fetch(&Request::get(url(port, "/"), a_person()))
                .is_ok()
        );
    }
    assert_eq!(
        sockets.load(Ordering::SeqCst),
        2,
        "asked to close, so closed"
    );
    assert_eq!(pool.idle(), 0, "and nothing is being kept");
}

#[test]
fn a_body_that_ends_when_the_connection_does_ends_the_connection() {
    // There is nothing left open to reuse, by definition — a pool that kept
    // this would hand out a closed socket.
    let (port, sockets) = serve(|mut socket, _| {
        if swallow_a_request(&mut socket) {
            let _ = socket.write_all(b"HTTP/1.1 200 OK\r\n\r\nread until I go away");
            let _ = socket.flush();
        }
    });

    let mut pool = pool();
    for _ in 0..2 {
        assert!(
            pool.fetch(&Request::get(url(port, "/"), a_person()))
                .is_ok()
        );
    }
    assert_eq!(sockets.load(Ordering::SeqCst), 2);
    assert_eq!(pool.idle(), 0);
}

#[test]
fn a_connection_closed_while_it_waited_is_tried_again_rather_than_failing() {
    // **The race this whole file is about.** The server answers once, then
    // hangs up while the connection sits in the pool. The next fetch bets on
    // it, loses, and must quietly try again on a new socket.
    let (port, sockets) = serve(|mut socket, _| {
        if swallow_a_request(&mut socket) {
            let _ = socket.write_all(ok("first").as_bytes());
            let _ = socket.flush();
        }
        // …and then go away, without saying `Connection: close`, which is
        // exactly what a server with an idle timeout does.
    });

    let mut pool = pool();
    let first = pool
        .fetch(&Request::get(url(port, "/"), a_person()))
        .expect("a response");
    assert_eq!(first.text().text, "first");
    assert_eq!(pool.idle(), 1, "kept, because nothing said not to");

    // Give the server's side time to actually close.
    std::thread::sleep(std::time::Duration::from_millis(120));

    let second = pool.fetch(&Request::get(url(port, "/"), a_person()));
    assert!(
        second.is_ok(),
        "a lost bet is a retry, not a failure: {second:?}",
    );
    assert_eq!(sockets.load(Ordering::SeqCst), 2, "it opened a new one");
}

#[test]
fn a_request_that_must_not_happen_twice_is_never_tried_again() {
    // A `POST` that failed after the server received it is a payment that has
    // happened. Sending it again is a payment that has happened twice — so
    // the retry is about the *method*, not about how likely it seems.
    let (port, sockets) = serve(|mut socket, which| {
        if swallow_a_request(&mut socket) && which == 0 {
            let _ = socket.write_all(ok("first").as_bytes());
            let _ = socket.flush();
        }
        // The second socket is accepted and answered with nothing, so a retry
        // would be visible as a second accepted socket that also failed.
    });

    let mut pool = pool();
    assert!(
        pool.fetch(&Request::get(url(port, "/"), a_person()))
            .is_ok()
    );
    std::thread::sleep(std::time::Duration::from_millis(120));

    let mut posting = Request::get(url(port, "/pay"), a_person());
    posting.method = "POST".to_owned();
    let answer = pool.fetch(&posting);

    assert!(answer.is_err(), "a POST is not repeated: {answer:?}");
    assert_eq!(
        sockets.load(Ordering::SeqCst),
        1,
        "and no second socket was opened to repeat it on",
    );
}

#[test]
fn an_https_connection_is_never_handed_out_for_an_http_one() {
    // The scheme is part of which server a connection goes to. Handing an
    // `http` connection out for an `https` request would send a page's
    // cookies in the clear.
    let (port, _) = serve(|mut socket, _| {
        while swallow_a_request(&mut socket) {
            if socket.write_all(ok("plain").as_bytes()).is_err() {
                return;
            }
            let _ = socket.flush();
        }
    });

    let mut pool = pool();
    assert!(
        pool.fetch(&Request::get(url(port, "/"), a_person()))
            .is_ok()
    );
    assert_eq!(pool.idle(), 1);

    let secure = format!("https://127.0.0.1:{port}/");
    let Ok(secure) = alo_url::parse(&secure) else {
        panic!("a URL");
    };
    // It must not reuse the plain connection; it opens a new one and the TLS
    // handshake fails against a server speaking plain HTTP.
    assert!(pool.fetch(&Request::get(secure, a_person())).is_err());
    assert_eq!(pool.idle(), 1, "the plain one is still there, untouched");
}

#[test]
fn a_request_the_server_never_answers_is_a_failure_rather_than_a_hang() {
    let (port, _) = serve(|mut socket, _| {
        // Take the request and say nothing, then close.
        let _ = swallow_a_request(&mut socket);
    });
    let mut pool = pool();
    assert!(
        pool.fetch(&Request::get(url(port, "/"), a_person()))
            .is_err()
    );
}
