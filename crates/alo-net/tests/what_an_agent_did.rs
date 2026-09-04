/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The durable half of the record, driven by the real thing that makes
//! requests — and by a real restart.
//!
//! `kept.rs`'s own tests are about what a file may hold and what the bound does.
//! This file is about the two claims queue item 202 closes on, and there is only
//! one way to check either: browse through a [`Pool`] the way a browser process
//! does, drop everything, open the directory again, and read what is there.
//!
//! - **An agent's work survives a restart and a person's browsing does not.**
//! - **A session-scoped profile leaves no file behind at all** — never written,
//!   rather than written and deleted, which is ADR 0011 § 2's rule and the whole
//!   of how private browsing is answered here.
//!
//! The server is a few lines on `127.0.0.1`, and nothing here reaches the
//! network.

use alo_net::cause::{Cause, Identities};
use alo_net::chain::Documents;
use alo_net::kept::Kept;
use alo_net::{Pool, Purpose, Request, Trust};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
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

/// A server that answers every request on every connection with the same thing.
fn always(answer: &'static str) -> u16 {
    let Ok(listener) = TcpListener::bind("127.0.0.1:0") else {
        return 0;
    };
    let Ok(address) = listener.local_addr() else {
        return 0;
    };
    let port = address.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else { return };
            std::thread::spawn(move || {
                while swallow_a_request(&mut socket) {
                    if socket.write_all(answer.as_bytes()).is_err() {
                        return;
                    }
                    let _ = socket.flush();
                }
            });
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

/// A pool that trusts nobody, which is enough: nothing here is `https`.
fn pool() -> Pool {
    Pool::with_trust(trusting_nobody()).patient_for(Duration::from_millis(500))
}

fn trusting_nobody() -> Trust {
    Trust::of(&[]).unwrap_or_else(|_| trusting_nobody())
}

/// A directory of this test's own, in the place the machine keeps temporary
/// things. Named after the caller so two tests never share one.
fn somewhere(called: &str) -> PathBuf {
    let place = std::env::temp_dir().join(format!(
        "alo-agent-record-{}-{called}",
        std::process::id().wrapping_mul(2_654_435_761)
    ));
    let _ = std::fs::remove_dir_all(&place);
    place
}

/// A browser process browsing: a page the person opened and fetched from, an
/// action taken in it, and a page that action opened and fetched from.
///
/// Every request goes through the real [`Pool`], so what ends up in the record
/// is what the engine wrote rather than what a test composed.
fn browse(pool: &mut Pool, port: u16) -> (Identities, Documents) {
    let mut minting = Identities::default();
    let mut documents = Documents::default();
    let tab = minting.a_tab();

    // What the person opened, and what that page fetched for itself.
    let theirs = documents.opened(&mut minting, Cause::Person { tab });
    fetched(
        pool,
        &Request::get(url(port, "/their-page"), Cause::Person { tab }),
    );
    fetched(
        pool,
        &Request::get(
            url(port, "/theirs.css"),
            Cause::Document { document: theirs },
        ),
    );

    // What the agent did: a verb accepted, a page loaded because of it, and
    // what that page fetched for itself.
    let action = minting.an_action();
    let opened = documents.opened(
        &mut minting,
        Cause::Agent {
            action,
            document: theirs,
        },
    );
    fetched(
        pool,
        &Request::get(
            url(port, "/the-page-it-opened"),
            Cause::Agent {
                action,
                document: theirs,
            },
        ),
    );
    fetched(
        pool,
        &Request::get(url(port, "/its.js"), Cause::Document { document: opened })
            .for_purpose(Purpose::Script),
    );

    (minting, documents)
}

/// One request, through the real pool, and it has to have worked.
fn fetched(pool: &mut Pool, request: &Request) {
    assert!(
        pool.fetch(request).is_ok(),
        "{} did not answer, so there is nothing to have recorded",
        request.url,
    );
}

/// Every line of the durable record, as a person would read them.
fn kept_lines(kept: &mut Kept) -> Vec<String> {
    kept.all()
        .iter()
        .flat_map(|deed| deed.requests.iter().map(ToString::to_string))
        .collect()
}

// --- It survives a restart, and a person's browsing does not ----------------

/// The item's first closing clause, over a real restart: the `Kept` and its
/// directory are dropped and a second one is opened on the same path.
#[test]
fn an_agents_work_survives_a_restart_and_a_persons_browsing_does_not() {
    let port = always("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
    let place = somewhere("restart");

    {
        let mut pool = pool().keeping_what_an_agent_did(Kept::at(&place).expect("a record"));
        let (_, documents) = browse(&mut pool, port);
        assert_eq!(
            pool.activity().len(),
            4,
            "the session's record holds everything, which is what this is taken from",
        );
        let kept = pool.what_an_agent_did(&documents).expect("a record");
        assert_eq!(kept.len(), 1, "one action");
    }

    let mut reopened = Kept::at(&place).expect("the same directory");
    let said = kept_lines(&mut reopened);
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(
        said.iter().any(|line| line.contains("/the-page-it-opened")),
        "the load the agent caused did not survive: {said:?}",
    );
    assert!(
        said.iter().any(|line| line.contains("/its.js")),
        "what the page it opened fetched did not survive: {said:?}",
    );
    assert!(
        said.iter().all(|line| line.contains("action#0")),
        "a line was kept that does not say which action it followed from: {said:?}",
    );

    let whole = format!("{:?}", reopened.all());
    assert!(
        !whole.contains("/theirs.css") && !whole.contains("/their-page"),
        "a person's own browsing was written to a disk: {whole}",
    );
    assert_eq!(reopened.unreadable(), 0);
    let _ = std::fs::remove_dir_all(&place);
}

/// The chain is frozen, and this is where that matters: the documents it was
/// walked against belonged to a process that has ended.
#[test]
fn a_line_read_after_a_restart_still_says_what_caused_it() {
    let port = always("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
    let place = somewhere("frozen");

    {
        let mut pool = pool().keeping_what_an_agent_did(Kept::at(&place).expect("a record"));
        let (_, documents) = browse(&mut pool, port);
        pool.what_an_agent_did(&documents).expect("a record");
    }

    let mut reopened = Kept::at(&place).expect("the same directory");
    let said = kept_lines(&mut reopened);
    let Some(line) = said.iter().find(|line| line.contains("/its.js")) else {
        panic!("nothing was kept: {said:?}");
    };
    assert!(
        line.contains(
            "caused by document#1, caused by action#0, in document#0, \
                       caused by the person, in tab#0"
        ),
        "the whole chain did not survive the process that walked it: {line}",
    );
    assert!(line.starts_with("GET "), "{line}");
    assert!(line.contains("(script)"), "the purpose is kept: {line}");
    assert!(line.ends_with("— answered 200"), "{line}");
    let _ = std::fs::remove_dir_all(&place);
}

// --- A session-scoped profile leaves nothing behind -------------------------

/// The item's second closing clause. **Never written**, rather than written and
/// deleted: the directory a record would have lived in does not exist, so there
/// is nothing to recover and no window between two operations for a crash to
/// land in.
#[test]
fn a_private_profile_leaves_no_file_behind_at_all() {
    let port = always("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
    let place = somewhere("private");

    let mut pool = pool();
    let (_, documents) = browse(&mut pool, port);

    assert_eq!(
        pool.activity().len(),
        4,
        "the session's own record is unaffected: it is in memory and dies with the process",
    );
    assert!(
        pool.what_an_agent_did(&documents).is_none(),
        "a pool nobody gave a record to kept one anyway",
    );
    assert!(
        !pool.forget_what_an_agent_did(),
        "there was something to delete, which means something was written",
    );
    assert!(
        !place.exists(),
        "a session-scoped profile left a directory behind: {place:?}",
    );
}

// --- What a person can do with it -------------------------------------------

/// Deleting is real, and it survives a restart: ADR 0012 § 7's *"the file, not a
/// flag on it"*.
#[test]
fn deleting_it_removes_the_files_and_they_do_not_come_back() {
    let port = always("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
    let place = somewhere("deleted");

    {
        let mut pool = pool().keeping_what_an_agent_did(Kept::at(&place).expect("a record"));
        let (_, documents) = browse(&mut pool, port);
        assert_eq!(
            pool.what_an_agent_did(&documents).map(|kept| kept.len()),
            Some(1),
            "nothing was kept to delete",
        );

        assert!(pool.forget_what_an_agent_did());
        let left: Vec<PathBuf> = std::fs::read_dir(&place)
            .expect("a listing")
            .flatten()
            .map(|entry| entry.path())
            .collect();
        assert!(left.is_empty(), "deleting left files behind: {left:?}");
    }

    let mut reopened = Kept::at(&place).expect("the same directory");
    assert!(
        reopened.all().is_empty(),
        "what was deleted came back after a restart",
    );
    let _ = std::fs::remove_dir_all(&place);
}

/// Reading brings it up to date, so there is no version of this that answers
/// *what did the agent do* with a record somebody forgot to bring up to date —
/// and asking twice does not write anything twice.
#[test]
fn reading_it_is_what_brings_it_up_to_date_and_asking_twice_changes_nothing() {
    let port = always("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
    let place = somewhere("up-to-date");
    let mut pool = pool().keeping_what_an_agent_did(Kept::at(&place).expect("a record"));

    let (_, documents) = browse(&mut pool, port);

    let Some(kept) = pool.what_an_agent_did(&documents) else {
        panic!("no record");
    };
    let first = kept_lines(kept);
    assert_eq!(first.len(), 2, "{first:?}");

    let Some(again) = pool.what_an_agent_did(&documents) else {
        panic!("no record");
    };
    assert_eq!(kept_lines(again), first, "asking twice wrote it twice");
    assert_eq!(again.missed(), 0, "a line went by without being counted");
    let _ = std::fs::remove_dir_all(&place);
}
