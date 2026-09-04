/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a redirect carries, what it drops, and where it refuses to go.
//!
//! Almost all of this needs no server, which is the point of `redirect::next`
//! being a function from a request and a response to a decision. Every rule
//! below is a security rule, and a security rule that can only be checked by
//! standing up a socket is a security rule that gets checked less often.

use alo_net::cause::{Cause, Identities};
use alo_net::redirect::{MOST_HOPS, Next, Refusal, Trail, next};
use alo_net::{Headers, Partition, Pool, Purpose, Request, Response, Status, Trust};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
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

/// A redirect response, as though it came from `from`.
fn pointing(from: &str, status: u16, to: &str) -> Response {
    let mut headers = Headers::new();
    headers.add("Location", to);
    Response {
        url: url(from),
        status: Status(status),
        headers,
        body: Vec::new(),
    }
}

/// The next request, or a description of why there wasn't one.
fn hop(sent: &Request, got: &Response) -> Result<Request, String> {
    match next(sent, got) {
        Ok(Next::Follow(request)) => Ok(*request),
        Ok(Next::Keep) => Err("kept".to_owned()),
        Err(refusal) => Err(refusal.to_string()),
    }
}

// --- What crosses an origin, and what must not -------------------------------

/// The one that matters most. A site being redirected to did not have this
/// site's credentials a moment ago and must not have them now.
#[test]
fn authorization_does_not_cross_to_another_origin() {
    let mut sent = Request::get(url("https://bank.example/statement"), a_person());
    sent.headers.add("Authorization", "Bearer a-real-token");
    sent.headers.add("Accept", "text/html");

    let away = hop(
        &sent,
        &pointing(
            "https://bank.example/statement",
            302,
            "https://evil.example/",
        ),
    )
    .expect("a cross-origin redirect is followed, just not with the credentials");
    assert_eq!(away.url.serialised, "https://evil.example/");
    assert_eq!(
        away.headers.get("Authorization"),
        None,
        "the token was handed to another origin"
    );
    assert_eq!(
        away.headers.get("Accept"),
        Some("text/html"),
        "an ordinary header should still cross"
    );

    let home = hop(
        &sent,
        &pointing("https://bank.example/statement", 302, "/statement/2024"),
    )
    .expect("a same-origin redirect is followed");
    assert_eq!(
        home.headers.get("Authorization"),
        Some("Bearer a-real-token"),
        "the credentials were dropped on a redirect that never left the origin"
    );
}

/// A scheme is part of an origin, so `https` to `http` is a crossing — which
/// is the case somebody hand-writing a host comparison gets wrong.
#[test]
fn a_downgrade_to_http_is_a_crossing_even_though_the_host_is_the_same() {
    let mut sent = Request::get(url("https://example.com/account"), a_person());
    sent.headers.add("Authorization", "Bearer a-real-token");
    sent.headers.add("Cookie", "session=abc");

    let down = hop(
        &sent,
        &pointing(
            "https://example.com/account",
            302,
            "http://example.com/account",
        ),
    )
    .expect("followed");
    assert_eq!(down.headers.get("Authorization"), None);
    assert_eq!(
        down.headers.get("Cookie"),
        None,
        "a session cookie was about to be sent in the clear"
    );
}

/// A port is part of an origin too.
#[test]
fn another_port_on_the_same_host_is_another_origin() {
    let mut sent = Request::get(url("https://example.com/a"), a_person());
    sent.headers.add("Authorization", "Bearer t");
    let across = hop(
        &sent,
        &pointing("https://example.com/a", 307, "https://example.com:8443/a"),
    )
    .expect("followed");
    assert_eq!(across.headers.get("Authorization"), None);
}

// --- What happens to the method ---------------------------------------------

/// Every browser has turned a redirected `POST` into a `GET` since the
/// nineteen-nineties. Silently re-submitting a form somewhere new is worse than
/// being wrong about an RFC.
#[test]
fn a_post_redirected_by_301_or_302_or_303_becomes_a_get() {
    for status in [301, 302, 303] {
        let mut sent = Request::sending(
            url("https://example.com/pay"),
            "POST",
            b"amount=100".to_vec(),
            a_person(),
        );
        sent.headers
            .add("Content-Type", "application/x-www-form-urlencoded");
        sent.headers.add("Accept", "text/html");

        let away = hop(
            &sent,
            &pointing("https://example.com/pay", status, "/thanks"),
        )
        .unwrap_or_else(|why| panic!("{status} should be followed: {why}"));
        assert_eq!(away.method, "GET", "a {status} kept the POST");
        assert_eq!(
            away.headers.get("Content-Type"),
            None,
            "a {status} kept a header describing a body that is gone"
        );
        assert_eq!(
            away.headers.get("Accept"),
            Some("text/html"),
            "a {status} dropped a header that had nothing to do with the body"
        );
        assert!(
            away.body.is_empty(),
            "a {status} carried the body into a GET, where its length no longer describes it"
        );
    }
}

/// `307` and `308` exist so a server can ask for the behaviour the
/// specification describes, and they are honoured exactly.
#[test]
fn a_post_redirected_by_307_or_308_stays_a_post() {
    for status in [307, 308] {
        let mut sent = Request::sending(
            url("https://example.com/pay"),
            "POST",
            br#"{"amount":100}"#.to_vec(),
            a_person(),
        );
        sent.headers.add("Content-Type", "application/json");

        let away = hop(
            &sent,
            &pointing("https://example.com/pay", status, "/pay/2"),
        )
        .unwrap_or_else(|why| panic!("{status} should be followed: {why}"));
        assert_eq!(away.method, "POST", "a {status} changed the method");
        assert_eq!(
            away.headers.get("Content-Type"),
            Some("application/json"),
            "a {status} dropped a header describing a body it kept"
        );
        assert_eq!(
            away.body, br#"{"amount":100}"#,
            "a {status} kept the method and lost the body, which is not the same request"
        );
    }
}

/// `HEAD` is already bodiless and safe. Turning it into a `GET` would fetch a
/// body nobody asked for.
#[test]
fn a_head_stays_a_head_through_every_kind_of_redirect() {
    for status in [301, 302, 303, 307, 308] {
        let mut sent = Request::get(url("https://example.com/a"), a_person());
        sent.method = "HEAD".to_owned();
        let away = hop(&sent, &pointing("https://example.com/a", status, "/b"))
            .unwrap_or_else(|why| panic!("{status}: {why}"));
        assert_eq!(
            away.method, "HEAD",
            "a {status} turned a HEAD into something"
        );
    }
}

// --- Where it refuses to go --------------------------------------------------

/// A server that could redirect a load into `file:///` would be reading the
/// disk of whoever opened the page. This engine fetches `file:` when asked
/// directly and refuses to be *sent* there.
#[test]
fn a_redirect_into_file_or_data_is_refused_by_name() {
    for (scheme, location) in [
        ("file", "file:///etc/passwd"),
        ("data", "data:text/html,<script>alert(1)</script>"),
    ] {
        let sent = Request::get(url("https://example.com/a"), a_person());
        let refused = next(&sent, &pointing("https://example.com/a", 302, location));
        let Err(Refusal::ASchemeWeDoNotFetch { scheme: named }) = refused else {
            panic!("a redirect to {scheme}: was not refused as a scheme");
        };
        assert_eq!(named, scheme);
    }
}

#[test]
fn a_location_that_is_not_a_url_is_refused_rather_than_guessed_at() {
    let sent = Request::get(url("https://example.com/a"), a_person());
    let refused = next(
        &sent,
        &pointing("https://example.com/a", 302, "http://[not a host]/"),
    );
    assert!(
        matches!(refused, Err(Refusal::NotAUrl { .. })),
        "a broken Location did not produce a NotAUrl refusal"
    );
}

/// A relative `Location` resolves against where the response came *from*. After
/// one hop that is not where the request started, and resolving against the
/// original would send the second hop to the wrong host entirely.
#[test]
fn a_relative_location_resolves_against_the_response_not_the_original_request() {
    let sent = Request::get(url("https://first.example/a/b"), a_person());
    // The request still names the first host; the response came from the second.
    let away = hop(&sent, &pointing("https://second.example/x/y", 302, "../z")).expect("followed");
    assert_eq!(away.url.serialised, "https://second.example/z");
}

// --- Not every 3xx is a redirect --------------------------------------------

/// A redirect that does not say where is not a redirect, and the body that came
/// with it is the only thing there is to show.
#[test]
fn a_3xx_with_no_location_is_the_answer_rather_than_a_refusal() {
    let sent = Request::get(url("https://example.com/a"), a_person());
    let got = Response {
        url: url("https://example.com/a"),
        status: Status(302),
        headers: Headers::new(),
        body: b"<p>choose one</p>".to_vec(),
    };
    assert_eq!(next(&sent, &got), Ok(Next::Keep));
}

#[test]
fn a_200_is_not_a_redirect_even_when_it_carries_a_location() {
    let sent = Request::get(url("https://example.com/a"), a_person());
    let mut got = pointing("https://example.com/a", 200, "https://elsewhere.example/");
    got.status = Status(201);
    assert_eq!(next(&sent, &got), Ok(Next::Keep));
}

/// The purpose and the initiator survive, because a redirect must not be able
/// to launder a request into looking like something else asked for it.
#[test]
fn what_asked_for_the_load_is_unchanged_by_being_redirected() {
    let asker = alo_url::Origin::of(&url("https://page.example/"));
    let sent = Request::get(url("https://example.com/a.css"), a_person())
        .for_purpose(Purpose::Style)
        .asked_by(asker.clone());
    let away = hop(
        &sent,
        &pointing(
            "https://example.com/a.css",
            302,
            "https://cdn.example/a.css",
        ),
    )
    .expect("followed");
    assert_eq!(away.purpose, Purpose::Style);
    assert_eq!(away.initiator, Some(asker));
}

// --- Chains that do not end --------------------------------------------------

#[test]
fn two_urls_pointing_at_each_other_are_a_circle_rather_than_a_hang() {
    let mut trail = Trail::from(&url("https://example.com/a"));
    assert_eq!(trail.and_then(&url("https://example.com/b")), Ok(()));
    let back = trail.and_then(&url("https://example.com/a"));
    let Err(Refusal::ACircle { url: named }) = back else {
        panic!("going back to the start was not a circle: {back:?}");
    };
    assert_eq!(named, "https://example.com/a");
}

#[test]
fn a_chain_of_distinct_urls_still_stops() {
    let mut trail = Trail::from(&url("https://example.com/0"));
    for step in 1..=MOST_HOPS {
        assert_eq!(
            trail.and_then(&url(&format!("https://example.com/{step}"))),
            Ok(()),
            "hop {step} of {MOST_HOPS} was refused too early"
        );
    }
    assert_eq!(trail.hops(), MOST_HOPS);
    assert_eq!(
        trail.and_then(&url("https://example.com/one-too-many")),
        Err(Refusal::TooManyHops { hops: MOST_HOPS })
    );
}

#[test]
fn the_trail_says_where_a_load_has_been_in_order() {
    let mut trail = Trail::from(&url("https://a.example/"));
    let _ = trail.and_then(&url("https://b.example/"));
    let _ = trail.and_then(&url("https://c.example/"));
    assert_eq!(
        trail.been().collect::<Vec<_>>(),
        vec![
            "https://a.example/",
            "https://b.example/",
            "https://c.example/"
        ]
    );
}

// --- End to end, over a socket that never leaves this machine ----------------

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
            let so_far = counted.fetch_add(1, Ordering::SeqCst);
            behaviour(socket, so_far);
        }
    });
    (port, sockets)
}

fn pool() -> Pool {
    // Trusting nobody is fine: nothing here is `https`, and half a second of
    // patience rather than a browser's thirty keeps the suite quick.
    Pool::with_trust(Trust::of(&[]).unwrap_or_else(|_| unreachable_trust()))
        .patient_for(Duration::from_millis(500))
}

fn unreachable_trust() -> Trust {
    // `Trust::of(&[])` cannot fail; this is the shape the type asks for.
    Trust::of(&[]).unwrap_or_else(|_| unreachable_trust())
}

/// The whole thing, once, over a real socket: two hops and a body at the end,
/// with the response reporting the URL it actually came from rather than the
/// one that was asked for.
#[test]
fn a_load_follows_a_chain_and_reports_where_it_ended() {
    let (port, _) = serve(|mut socket, answered| {
        if !swallow_a_request(&mut socket) {
            return;
        }
        let answer = match answered {
            0 => "HTTP/1.1 302 Found\r\nLocation: /second\r\nContent-Length: 0\r\n\r\n".to_owned(),
            1 => "HTTP/1.1 301 Moved Permanently\r\nLocation: /third\r\nContent-Length: 0\r\n\r\n"
                .to_owned(),
            _ => "HTTP/1.1 200 OK\r\nContent-Length: 7\r\n\r\narrived".to_owned(),
        };
        let _ = socket.write_all(answer.as_bytes());
        let _ = socket.flush();
    });
    assert!(port != 0, "no server");

    let asked = Request::get(url(&format!("http://127.0.0.1:{port}/first")), a_person());
    let got = pool()
        .follow(&asked, &Partition::of(&asked.url))
        .unwrap_or_else(|why| panic!("the chain should have been followed: {why}"));
    assert_eq!(got.status, Status(200));
    assert_eq!(got.body, b"arrived");
    assert_eq!(
        got.url.path, "/third",
        "the response has to say where it came from, not what was asked for"
    );
}

/// A server pointing at itself is a load that ends, with words somebody can
/// read, rather than a tab that will not close.
#[test]
fn a_server_that_redirects_to_itself_ends_the_load_rather_than_hanging() {
    let (port, sockets) = serve(|mut socket, _| {
        if !swallow_a_request(&mut socket) {
            return;
        }
        let _ = socket
            .write_all(b"HTTP/1.1 302 Found\r\nLocation: /round\r\nContent-Length: 0\r\n\r\n");
        let _ = socket.flush();
    });
    assert!(port != 0, "no server");

    let asked = Request::get(url(&format!("http://127.0.0.1:{port}/round")), a_person());
    let refused = pool().follow(&asked, &Partition::of(&asked.url));
    let why = match refused {
        Ok(got) => panic!("a self-redirect produced a {} response", got.status),
        Err(why) => why,
    };
    assert!(
        why.contains("circle"),
        "the refusal should say what happened, and said {why:?}"
    );
    assert!(
        sockets.load(Ordering::SeqCst) <= MOST_HOPS + 1,
        "it kept asking after it should have stopped"
    );
}
