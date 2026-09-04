/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Every request this engine makes says what caused it.
//!
//! Queue item 67 and ADR 0012. Named for the question rather than for the
//! field, the way items 61, 62 and 165's tests are.
//!
//! # What is being asserted, and what it is worth
//!
//! The half that a test cannot reach is the important one: **there is no
//! constructor that will make a request without a cause**, which is a fact
//! about a signature and is checked by the `compile_fail` example on
//! [`alo_net::Request::get`] rather than here. What this file does is the other
//! half — that the requests the engine makes *on its own behalf* carry a true
//! cause rather than an invented one.
//!
//! That is where ADR 0012 § 2 is either kept or quietly lost. There are four
//! such requests today, none of which any page asked for: a redirect hop, a
//! range request resuming a download, a CORS preflight, and a violation report.
//! Each is attributed to whatever caused the thing it is *about*, which is the
//! rule that let the decision have no `Unknown` in it. A fifth one appearing
//! with a fresh cause of its own is exactly the drift this file exists to
//! catch.
//!
//! # And one attack, because attribution is a claim
//!
//! A record saying *the person did that* is evidence, so the interesting
//! question is who can write it. A server cannot: it may answer `302` and send
//! a request anywhere it likes, and the cause that arrives at the other end is
//! still the one the browser process assigned. That test is named for the
//! attack rather than for the field.

use alo_net::cause::{Cause, Identities};
use alo_net::cors::asking_first;
use alo_net::csp::{Disposition, Policies};
use alo_net::csp_report::{Endpoints, Page};
use alo_net::download::{Download, Step};
use alo_net::redirect::{Next, next};
use alo_net::{Headers, Purpose, Request, Response, Status};
use alo_url::Origin;

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

/// The browser process's supply of identities. One, because there is one
/// browser process in this file.
fn minting() -> Identities {
    Identities::default()
}

/// A response that points somewhere else.
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

// --- The three causes are three different answers ---------------------------

/// The distinction the record exists to make. Two requests for the same URL,
/// from the same origin, are not the same line in a record — one is somebody
/// navigating and one is a page fetching, and a browser that could not tell
/// them apart could not answer *what did the agent do*.
#[test]
fn a_person_a_page_and_an_agent_are_three_different_causes() {
    let mut ids = minting();
    let tab = ids.a_tab();
    let document = ids.a_document();
    let action = ids.an_action();
    let target = url("https://example.com/thing");

    let navigated = Request::get(target.clone(), Cause::Person { tab });
    let fetched = Request::get(target.clone(), Cause::Document { document });
    let acted = Request::get(target, Cause::Agent { action, document });

    assert_ne!(navigated.cause, fetched.cause);
    assert_ne!(fetched.cause, acted.cause);
    assert_ne!(navigated.cause, acted.cause);

    // And each says which, in the words a person reads.
    assert!(navigated.to_string().contains("the person, in tab#0"));
    assert!(fetched.to_string().contains("caused by document#0"));
    assert!(acted.to_string().contains("action#0, in document#0"));
}

/// ADR 0012 § 3's first link, and the one that makes an agent's work findable
/// at all: a request an agent caused says so *and* says where it acted.
#[test]
fn an_agent_action_is_named_by_every_request_it_caused() {
    let mut ids = minting();
    let document = ids.a_document();
    let action = ids.an_action();
    let cause = Cause::Agent { action, document };

    let asked = Request::get(url("https://example.com/a"), cause.clone())
        .for_purpose(Purpose::Fetch)
        .asked_by(Origin::of(&url("https://example.com/")));

    let Cause::Agent {
        action: named,
        document: acted_in,
    } = &asked.cause
    else {
        panic!("an agent's request lost its action: {}", asked.cause);
    };
    assert_eq!(*named, action);
    assert_eq!(*acted_in, document);
    assert_eq!(asked.cause.in_document(), Some(document));
}

// --- The requests nobody asked for ------------------------------------------

/// A redirect is not a new intention, so it is not a new cause.
#[test]
fn a_redirect_is_attributed_to_whatever_asked_for_the_first_hop() {
    let mut ids = minting();
    let cause = Cause::Person { tab: ids.a_tab() };
    let sent = Request::get(url("https://example.com/a"), cause.clone());

    let Ok(Next::Follow(hop)) = next(&sent, &pointing("https://example.com/a", 302, "/b")) else {
        panic!("a 302 was not followed");
    };
    assert_eq!(hop.cause, cause, "the hop invented a cause of its own");
}

/// The attack, rather than the field. A server may send a request anywhere it
/// likes by answering `302`; what it may not do is change the sentence the
/// record will hold about who wanted it. If it could, any site could write
/// *the person did that* into somebody's own browser by redirecting a fetch its
/// page had made.
#[test]
fn a_server_cannot_turn_a_pages_fetch_into_something_the_person_did() {
    let mut ids = minting();
    let document = ids.a_document();
    let fetched = Request::get(url("https://example.com/a"), Cause::Document { document })
        .for_purpose(Purpose::Fetch);

    let Ok(Next::Follow(hop)) = next(
        &fetched,
        &pointing("https://example.com/a", 302, "https://elsewhere.test/b"),
    ) else {
        panic!("a cross-origin 302 was not followed");
    };

    assert_eq!(hop.cause, Cause::Document { document });
    assert!(
        !matches!(hop.cause, Cause::Person { .. }),
        "a redirect promoted a page's fetch to a person's navigation",
    );
    // The origin boundary still did its own job, which is item 55's, and this
    // is here so that a change to one is not read as covering the other.
    assert_eq!(hop.initiator, fetched.initiator);
}

/// A resumed download asks for the rest of the same thing, so it is the same
/// cause. It is the request most likely to be made long after whoever caused it
/// has stopped looking, which is why it is asserted rather than assumed.
#[test]
fn a_range_request_that_resumes_keeps_the_cause_of_the_download() {
    let mut ids = minting();
    let cause = Cause::Person { tab: ids.a_tab() };
    let wanted = Request::get(url("https://example.com/big.bin"), cause.clone());

    let mut download = Download::new();
    let first = download.asking(&wanted);
    assert_eq!(first.headers.get("Range"), None, "nothing to continue yet");
    assert_eq!(first.cause, cause, "the first ask invented a cause");

    // Half of it arrives and the body stops, which is the whole reason a second
    // request exists at all.
    let mut headers = Headers::new();
    headers.add("Content-Length", "10");
    headers.add("ETag", "\"v1\"");
    headers.add("Accept-Ranges", "bytes");
    let cut = Response {
        url: url("https://example.com/big.bin"),
        status: Status(200),
        headers,
        body: b"01234".to_vec(),
    };
    assert_eq!(download.take(&cut, true), Ok(Step::More));

    let resuming = download.asking(&wanted);
    assert_eq!(resuming.headers.get("Range"), Some("bytes=5-"));
    assert_eq!(
        resuming.cause, cause,
        "the request for the rest was made by nobody",
    );
}

/// A preflight exists for exactly one other request, and is attributed to it.
/// It is also the request a page is least aware of, since nothing in the page
/// wrote it.
#[test]
fn a_preflight_is_attributed_to_the_request_it_asks_about() {
    let mut ids = minting();
    let document = ids.a_document();
    let mut sending = Request::sending(
        url("https://api.example.test/orders"),
        "POST",
        b"{}".to_vec(),
        Cause::Document { document },
    )
    .for_purpose(Purpose::Fetch)
    .asked_by(Origin::of(&url("https://shop.example/")));
    sending.headers.add("Content-Type", "application/json");

    let question = asking_first(&sending);
    assert_eq!(question.method, "OPTIONS");
    assert_eq!(
        question.cause,
        Cause::Document { document },
        "the OPTIONS was a request nothing could account for",
    );
}

/// The one request on the web that no page asked for and no person asked for
/// either — and therefore the one that would have grown an `Unknown` if the
/// cause had not been carried as far as the document.
#[test]
fn a_violation_report_is_attributed_to_the_load_that_violated_the_policy() {
    let mut ids = minting();
    let tab = ids.a_tab();
    let document = ids.a_document();

    let mut headers = Headers::new();
    headers.add(
        "Content-Security-Policy",
        "script-src 'self'; report-uri /collect",
    );
    let policies = Policies::stated_by(&headers);

    // The page itself was opened by somebody; the script it asked for is the
    // page's own doing. Two causes, one of which is about to be recorded.
    let page = Page::at(url("https://shop.example/checkout"), Cause::Person { tab })
        .reporting_to(Endpoints::default());
    let script = Request::get(
        url("https://cdn.elsewhere.test/tracker.js"),
        Cause::Document { document },
    )
    .for_purpose(Purpose::Script)
    .asked_by(Origin::of(&url("https://shop.example/")));

    let violations = policies.violations(&script, None);
    let violation = violations.first().expect("a blocked script is a violation");
    assert_eq!(violation.disposition, Disposition::Enforce);

    let posting = violation.posts(&page);
    let post = posting.posts.first().expect("a report to post");
    assert_eq!(post.purpose, Purpose::Report);
    assert_eq!(
        post.cause,
        Cause::Person { tab },
        "a report is about the document's load, so it carries that load's cause",
    );
}

// --- Nothing gets to be anonymous -------------------------------------------

/// Read as one statement: every request this file could make the engine
/// produce names a cause, and each is a cause something else already had.
///
/// The value is in it being a sweep rather than five assertions in five files.
/// A new engine-made request added without a cause of its own is the failure
/// this catches, and it is the failure that would otherwise be found by
/// somebody reading a record with a gap in it.
#[test]
fn nothing_the_engine_makes_for_itself_arrives_without_a_cause() {
    let mut ids = minting();
    let document = ids.a_document();
    let cause = Cause::Document { document };

    let mut sending = Request::sending(
        url("https://api.example.test/orders"),
        "POST",
        b"{}".to_vec(),
        cause.clone(),
    );
    sending.headers.add("Content-Type", "application/json");

    let made: Vec<Request> = vec![
        asking_first(&sending),
        Download::new().asking(&Request::get(
            url("https://example.com/big.bin"),
            cause.clone(),
        )),
        match next(
            &Request::get(url("https://example.com/a"), cause.clone()),
            &pointing("https://example.com/a", 307, "/b"),
        ) {
            Ok(Next::Follow(hop)) => *hop,
            other => panic!("a 307 was not followed: {other:?}"),
        },
    ];

    for request in &made {
        assert_eq!(
            request.cause, cause,
            "{request} was made with a cause nothing else had",
        );
        assert!(
            request.to_string().contains("caused by"),
            "{request} printed without saying what caused it",
        );
    }
}
