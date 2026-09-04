/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The cache, over a socket, as a load actually uses it.
//!
//! `what_the_cache_serves.rs` asserts the decisions with a clock it controls.
//! This asserts that a load makes them — that a second `Pool::follow` of a
//! cacheable thing does not reach the server, that a `no-store` one does, and
//! that a revalidation sends the validator and uses the stored body when the
//! answer is `304`. Nothing here leaves this machine.

use alo_net::cause::{Cause, Identities};
use alo_net::{Partition, Pool, Request, Trust};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
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

fn pool() -> Pool {
    Pool::with_trust(Trust::of(&[]).unwrap_or_else(|_| unreachable_trust()))
        .patient_for(Duration::from_millis(500))
}

fn unreachable_trust() -> Trust {
    Trust::of(&[]).unwrap_or_else(|_| unreachable_trust())
}

/// Read one request, and hand back what it said.
fn take_a_request(socket: &mut TcpStream) -> Option<String> {
    let mut seen = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match socket.read(&mut byte) {
            Ok(0) | Err(_) => return None,
            Ok(_) => {}
        }
        seen.push(byte.first().copied().unwrap_or(0));
        if seen.ends_with(b"\r\n\r\n") {
            return Some(String::from_utf8_lossy(&seen).into_owned());
        }
        if seen.len() > 16 * 1024 {
            return None;
        }
    }
}

/// A server that records every request it is asked, and answers by a rule.
fn serve(
    answer: impl Fn(usize, &str) -> String + Send + 'static,
) -> (u16, Arc<AtomicUsize>, Arc<Mutex<Vec<String>>>) {
    let asked = Arc::new(AtomicUsize::new(0));
    let requests: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let counted = Arc::clone(&asked);
    let recorded = Arc::clone(&requests);
    let Ok(listener) = TcpListener::bind("127.0.0.1:0") else {
        return (0, asked, requests);
    };
    let Ok(address) = listener.local_addr() else {
        return (0, asked, requests);
    };
    let port = address.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else { return };
            while let Some(request) = take_a_request(&mut socket) {
                let so_far = counted.fetch_add(1, Ordering::SeqCst);
                if let Ok(mut held) = recorded.lock() {
                    held.push(request.clone());
                }
                let reply = answer(so_far, &request);
                if socket.write_all(reply.as_bytes()).is_err() {
                    break;
                }
                let _ = socket.flush();
            }
        }
    });
    (port, asked, requests)
}

fn body(headers: &str, text: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n{headers}\r\n{text}",
        text.len()
    )
}

#[test]
fn a_second_load_of_a_fresh_thing_never_reaches_the_server() {
    let (port, asked, _) = serve(|_, _| body("Cache-Control: max-age=60\r\n", "the page"));
    assert!(port != 0, "no server");
    let target = url(&format!("http://127.0.0.1:{port}/a"));

    let site = Partition::of(&target);
    let mut pool = pool();
    let first = pool
        .follow(&Request::get(target.clone(), a_person()), &site)
        .unwrap_or_else(|why| panic!("first load: {why}"));
    let second = pool
        .follow(&Request::get(target, a_person()), &site)
        .unwrap_or_else(|why| panic!("second load: {why}"));

    assert_eq!(first.body, b"the page");
    assert_eq!(second.body, b"the page");
    assert_eq!(
        asked.load(Ordering::SeqCst),
        1,
        "the server was asked twice for something it said was good for a minute"
    );
    assert_eq!(pool.cache().counts().0, 1, "the hit was not counted");
}

#[test]
fn a_no_store_response_is_asked_for_every_time() {
    let (port, asked, _) = serve(|_, _| body("Cache-Control: no-store\r\n", "private"));
    assert!(port != 0, "no server");
    let target = url(&format!("http://127.0.0.1:{port}/secret"));

    let site = Partition::of(&target);
    let mut pool = pool();
    for _ in 0..3 {
        let got = pool
            .follow(&Request::get(target.clone(), a_person()), &site)
            .unwrap_or_else(|why| panic!("{why}"));
        assert_eq!(got.body, b"private");
    }
    assert_eq!(
        asked.load(Ordering::SeqCst),
        3,
        "a no-store response was kept and served again"
    );
    assert_eq!(pool.cache().len(), 0);
}

/// The whole revalidation round trip: the second load goes out carrying the
/// validator, the server says `304` with no body, and the body the caller gets
/// is the one that was stored.
#[test]
fn a_stale_thing_is_revalidated_and_the_stored_body_is_what_comes_back() {
    let (port, asked, requests) = serve(|so_far, request| {
        if so_far == 0 {
            return body(
                "Cache-Control: max-age=0\r\nETag: \"v1\"\r\n",
                "the original body",
            );
        }
        assert!(
            request.contains("If-None-Match: \"v1\""),
            "the second request did not carry the validator: {request}"
        );
        "HTTP/1.1 304 Not Modified\r\nCache-Control: max-age=60\r\nETag: \"v1\"\r\n\r\n".to_owned()
    });
    assert!(port != 0, "no server");
    let target = url(&format!("http://127.0.0.1:{port}/b"));

    let site = Partition::of(&target);
    let mut pool = pool();
    let first = pool
        .follow(&Request::get(target.clone(), a_person()), &site)
        .unwrap_or_else(|why| panic!("first load: {why}"));
    assert_eq!(first.body, b"the original body");

    let second = pool
        .follow(&Request::get(target.clone(), a_person()), &site)
        .unwrap_or_else(|why| panic!("revalidation: {why}"));
    assert_eq!(
        second.body, b"the original body",
        "a 304 has no body, so this had to come from what was stored"
    );
    assert_eq!(asked.load(Ordering::SeqCst), 2);

    // And the `304` said it is good for a minute now, so a third load is free.
    let third = pool
        .follow(&Request::get(target, a_person()), &site)
        .unwrap_or_else(|why| panic!("third load: {why}"));
    assert_eq!(third.body, b"the original body");
    assert_eq!(
        asked.load(Ordering::SeqCst),
        2,
        "the refreshed lifetime from the 304 was not applied"
    );

    let sent = requests.lock().map(|held| held.len()).unwrap_or_default();
    assert_eq!(sent, 2);
}

/// A response with no `Date` at all. Every age calculation on one falls back to
/// zero unless something supplies the moment it arrived, and zero reads as
/// "brand new" — the optimistic direction to be wrong in.
#[test]
fn a_response_with_no_date_is_still_cached_correctly() {
    let (port, asked, _) = serve(|_, _| {
        // No `Date`, which a real server always sends and a minimal one does not.
        "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nCache-Control: max-age=60\r\n\r\nhere".to_owned()
    });
    assert!(port != 0, "no server");
    let target = url(&format!("http://127.0.0.1:{port}/c"));

    let site = Partition::of(&target);
    let mut pool = pool();
    let _ = pool.follow(&Request::get(target.clone(), a_person()), &site);
    let again = pool
        .follow(&Request::get(target, a_person()), &site)
        .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(again.body, b"here");
    assert_eq!(asked.load(Ordering::SeqCst), 1);
}
