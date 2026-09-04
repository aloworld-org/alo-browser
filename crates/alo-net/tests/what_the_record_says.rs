/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The session's record, written by the real thing that makes requests.
//!
//! `activity.rs`'s own tests are about what a line may hold. This file is about
//! the claim ADR 0012 § 6 makes — *everything, for the session* — and there is
//! only one way to check that: make requests through [`Pool`], the way a
//! browser process does, and read what came out. A test that wrote its own
//! lines would be testing the type rather than the promise.
//!
//! The servers are a few lines each, on `127.0.0.1`, and nothing here reaches
//! the network.

use alo_net::activity::{Entry, Happened};
use alo_net::cause::{Cause, Identities};
use alo_net::chain::Documents;
use alo_net::{Partition, Pool, Purpose, Request, Trust};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

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
/// how many have been accepted so far.
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

/// A server that answers every request on every connection with these bytes.
fn always(answer: &'static str) -> u16 {
    let (port, _) = serve(move |mut socket, _| {
        while swallow_a_request(&mut socket) {
            if socket.write_all(answer.as_bytes()).is_err() {
                return;
            }
            let _ = socket.flush();
        }
    });
    port
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

/// A pool that trusts nobody, which is enough: nothing here is `https`, and it
/// avoids reading the machine's certificate store for a test that does not need
/// it. Half a second of patience rather than a browser's thirty.
fn pool() -> Pool {
    Pool::with_trust(trusting_nobody()).patient_for(Duration::from_millis(500))
}

fn trusting_nobody() -> Trust {
    // `Trust::of(&[])` cannot fail; this is the shape the type asks for.
    Trust::of(&[]).unwrap_or_else(|_| trusting_nobody())
}

/// What the record says, one line per request, in order.
fn lines(pool: &Pool) -> Vec<String> {
    pool.activity().entries().map(ToString::to_string).collect()
}

/// A browser process, in the two pieces ADR 0012 § 4 says only it holds: what
/// mints an identity, and what caused each document's load.
fn browser() -> (Identities, Documents) {
    (Identities::default(), Documents::default())
}

// --- Everything, for the session (ADR 0012 § 6) -----------------------------

/// The claim, over a load that is more than one request: a redirect hop is a
/// request, so it is a line, and it carries the cause of the load it belongs to.
#[test]
fn every_request_a_load_made_is_a_line_including_the_hops() {
    let port = always("HTTP/1.1 302 Found\r\nLocation: /arrived\r\nContent-Length: 0\r\n\r\n");
    let (mut minting, mut documents) = browser();
    let tab = minting.a_tab();
    let opened = documents.opened(&mut minting, Cause::Person { tab });
    let mut pool = pool();

    // It redirects to `/arrived`, which redirects to `/arrived` — the same
    // server answers everything — so the load ends in a refusal. What is being
    // asserted is the record of the requests it made on the way.
    let asked = Request::get(url(port, "/start"), Cause::Document { document: opened });
    let _ = pool.follow(&asked, &Partition::of(&url(port, "/start")));

    let said = lines(&pool);
    assert!(said.len() >= 2, "a redirect chain was one line: {said:?}");
    assert!(
        said.first().is_some_and(|line| line.contains("/start")),
        "the first request is not the first line: {said:?}",
    );
    assert!(
        said.iter().any(|line| line.contains("/arrived")),
        "a hop that was fetched left no line: {said:?}",
    );
    assert!(
        pool.activity()
            .entries()
            .all(|line| line.cause() == &Cause::Document { document: opened }),
        "a hop was attributed to something other than the load it belongs to: {said:?}",
    );
}

/// What the cache answered is a line too, and it says so rather than looking
/// like a second trip to the server. *What did this page load* includes what it
/// never went to the network for.
#[test]
fn what_the_cache_answered_is_a_line_that_says_it_was_the_cache() {
    let port =
        always("HTTP/1.1 200 OK\r\nCache-Control: max-age=600\r\nContent-Length: 5\r\n\r\nhello");
    let mut pool = pool();
    let within = Partition::of(&url(port, "/logo.png"));
    let asked = Request::get(url(port, "/logo.png"), a_person(&mut Identities::default()))
        .for_purpose(Purpose::Image);

    for _ in 0..2 {
        pool.follow(&asked, &within).expect("a response");
    }

    let said = lines(&pool);
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(
        said.first()
            .is_some_and(|line| line.ends_with("answered 200")),
        "{said:?}",
    );
    assert!(
        said.last()
            .is_some_and(|line| line.ends_with("served from the cache, 200")),
        "the second load was recorded as though it had been fetched: {said:?}",
    );
    assert!(
        said.iter().all(|line| line.contains("(image)")),
        "the purpose is part of what is recorded: {said:?}",
    );
}

/// Nothing came back at all, which is not a status and is not a refusal of
/// ours. A record that filed it as one would be inventing a sentence a server
/// never said.
#[test]
fn a_server_that_is_not_there_is_a_line_saying_nothing_happened() {
    // A port that was listening and is not any more, so the connection is
    // refused rather than left hanging.
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback");
    let port = listener.local_addr().expect("an address").port();
    drop(listener);

    let mut pool = pool();
    let asked = Request::get(url(port, "/"), a_person(&mut Identities::default()));
    assert!(pool.fetch(&asked).is_err(), "something answered");

    let Some(line) = pool.activity().latest() else {
        panic!("a request that failed left no line");
    };
    assert!(matches!(line.happened(), Happened::Failed { .. }), "{line}");
    assert!(line.to_string().contains("did not happen"), "{line}");
}

/// A rule of ours refused a hop that was composed and never sent. ADR 0012 § 5
/// asks for it **by name**, and a silence here would make a load this engine
/// stopped look like a load nobody attempted.
#[test]
fn a_redirect_in_a_circle_is_a_line_naming_the_rule_that_refused_it() {
    let port = always("HTTP/1.1 302 Found\r\nLocation: /round\r\nContent-Length: 0\r\n\r\n");
    let mut pool = pool();
    let asked = Request::get(url(port, "/round"), a_person(&mut Identities::default()));

    let refused = pool.follow(&asked, &Partition::of(&url(port, "/round")));
    assert!(refused.is_err(), "a circle was followed");

    let Some(line) = pool.activity().latest() else {
        panic!("a refusal left no line");
    };
    let Happened::Refused { rule } = line.happened() else {
        panic!("a refusal was recorded as something else: {line}");
    };
    assert!(rule.contains("circle"), "the rule is not named: {rule}");
    assert!(line.to_string().contains("/round"), "{line}");
}

// --- What may never be in it (ADR 0012 § 5) ---------------------------------

/// The half that matters, over a real exchange: the record holds what was asked
/// of whom and never what was in it.
#[test]
fn nothing_a_request_carried_is_in_the_record() {
    let port = always("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
    let mut pool = pool();
    let mut sending = Request::sending(
        url(port, "/transfer"),
        "POST",
        b"amount=500&to=someone".to_vec(),
        a_person(&mut Identities::default()),
    );
    sending.headers.add("Cookie", "session=hunter2");
    sending
        .headers
        .add("Authorization", "Bearer sk-live-secret");

    pool.fetch(&sending).expect("a response");

    let Some(line) = pool.activity().latest() else {
        panic!("nothing was written down");
    };
    for held in [line.to_string(), format!("{line:?}")] {
        assert!(!held.contains("hunter2"), "a cookie reached the record");
        assert!(!held.contains("sk-live-secret"), "a credential did");
        assert!(!held.contains("amount=500"), "a body did");
    }
    assert!(line.to_string().starts_with("POST "), "{line}");
    assert!(line.to_string().contains("/transfer"), "{line}");
}

// --- The chain, over the record (ADR 0012 § 3) ------------------------------

/// The question the whole decision exists to answer, asked of a real session:
/// an agent activated something, a page loaded because of it, and everything
/// that page fetched leads back to the action — while the person's own browsing
/// leads back to them and to no action at all.
#[test]
fn an_agents_work_is_reachable_from_every_line_that_followed_from_it() {
    let port = always("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
    let (mut minting, mut documents) = browser();
    let tab = minting.a_tab();
    let read = documents.opened(&mut minting, Cause::Person { tab });
    let action = minting.an_action();
    let opened = documents.opened(
        &mut minting,
        Cause::Agent {
            action,
            document: read,
        },
    );
    let mut pool = pool();

    // What the person's own page fetched, and then what the page the agent
    // opened fetched.
    pool.fetch(&Request::get(
        url(port, "/theirs.css"),
        Cause::Document { document: read },
    ))
    .expect("a response");
    pool.fetch(&Request::get(
        url(port, "/its.js"),
        Cause::Document { document: opened },
    ))
    .expect("a response");

    let said: Vec<&Entry> = pool.activity().entries().collect();
    assert_eq!(said.len(), 2, "{said:?}");
    let Some((theirs, its)) = said.first().zip(said.last()) else {
        panic!("two requests were not two lines");
    };

    assert_eq!(
        theirs.chain(&documents).action(),
        None,
        "a person's own browsing was attributed to an agent",
    );
    assert_eq!(theirs.chain(&documents).person(), Some(tab));

    assert_eq!(
        its.chain(&documents).action(),
        Some(action),
        "what the agent set off did not lead back to it: {its}",
    );
    assert!(its.chain(&documents).followed_from(action));
    assert_eq!(
        its.chain(&documents).person(),
        Some(tab),
        "and the far end is still the person who asked",
    );
}

// --- What a person can do with it (ADR 0012 § 7) ----------------------------

/// Deleting is real: the lines go, rather than a flag being set beside them.
#[test]
fn a_person_can_empty_the_record_and_it_is_emptied() {
    let port = always("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
    let mut pool = pool();
    for path in ["/one", "/two", "/three"] {
        pool.fetch(&Request::get(
            url(port, path),
            a_person(&mut Identities::default()),
        ))
        .expect("a response");
    }
    assert_eq!(pool.activity().len(), 3);

    pool.forget_the_record();
    assert!(pool.activity().is_empty());
    assert_eq!(pool.activity().forgotten(), 0, "it still counted them");
    assert_eq!(pool.activity().latest(), None);
}

/// What caused a request nobody's page made: a person, in a tab of their own.
fn a_person(minting: &mut Identities) -> Cause {
    Cause::Person {
        tab: minting.a_tab(),
    }
}
