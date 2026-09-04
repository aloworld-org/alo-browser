/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a page's author is told when their own policy stopped something.
//!
//! Named for what somebody gets rather than for the header that carries it, the
//! way items 61, 62 and 165's tests are. The three questions the item asks are
//! each a test here: **both dispositions report**, **a report says which
//! directive and which URL without saying more than it may**, and **a report
//! that cannot be sent is not a load that fails**.
//!
//! The collector is a socket on `127.0.0.1` in this file rather than a
//! dependency, for the reason `fetching_over_http.rs` gives: nothing here
//! reaches the network, and a suite that did would fail on an aeroplane.

use alo_net::cause::{Cause, Identities};
use alo_net::csp::{Content, Disposition, Policies};
use alo_net::csp_report::{Blocked, Endpoints, Page, Violation};
use alo_net::{Headers, Purpose, Request, Trust};
use alo_url::Origin;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

/// What caused every request in this file: a document fetching what it needs.
///
/// ADR 0012 § 1 makes the cause an argument rather than something a caller may
/// forget, so a test has to say what it means too — and what these mean is a
/// page asking for a subresource rather than a person navigating.
fn a_page() -> Cause {
    Cause::Document {
        document: Identities::default().a_document(),
    }
}

/// The page every test below is on.
const PAGE: &str = "https://shop.example.com/checkout?step=2";

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

/// A request the page made, as the page.
fn wants(target: &str, purpose: Purpose) -> Request {
    Request::get(url(target), a_page())
        .for_purpose(purpose)
        .asked_by(Origin::of(&url(PAGE)))
}

fn policies(enforced: &[&str], watched: &[&str]) -> Policies {
    let mut headers = Headers::new();
    for value in enforced {
        headers.add("Content-Security-Policy", *value);
    }
    for value in watched {
        headers.add("Content-Security-Policy-Report-Only", *value);
    }
    Policies::stated_by(&headers)
}

/// The page a report is about, at [`PAGE`].
fn about() -> Page {
    Page::at(url(PAGE), a_page()).came_from("https://search.example/")
}

/// The whole reason reporting exists: a site sends the report-only header for a
/// week, reads what it *would* have blocked, and only then enforces. A browser
/// that reported one disposition and not the other would leave them nothing to
/// read.
#[test]
fn a_policy_being_enforced_and_one_being_watched_both_report() {
    let policies = policies(
        &["script-src 'self'; report-uri /csp-enforced"],
        &["script-src 'none'; report-uri /csp-watched"],
    );
    let injected = wants("https://evil.test/steal.js", Purpose::Script);

    let violations = policies.violations(&injected, None);
    assert_eq!(violations.len(), 2, "one of the two policies said nothing");

    let mut posted: Vec<String> = Vec::new();
    for violation in &violations {
        let posting = violation.posts(&about());
        assert!(posting.unusable.is_empty(), "{:?}", posting.unusable);
        for post in &posting.posts {
            posted.push(post.url.to_string());
        }
    }
    assert_eq!(
        posted,
        vec![
            "https://shop.example.com/csp-enforced".to_owned(),
            "https://shop.example.com/csp-watched".to_owned(),
        ],
    );

    // And only one of them stopped anything.
    assert!(policies.allows(&injected, None).is_err());
    let dispositions: Vec<Disposition> = violations.iter().map(|one| one.disposition).collect();
    assert_eq!(
        dispositions,
        vec![Disposition::Enforce, Disposition::Report]
    );
}

/// Inline content has no URL, and the report says the specification's word for
/// that rather than an empty field.
#[test]
fn inline_content_a_policy_refused_is_reported_as_inline() {
    let policies = policies(&["script-src 'self'; report-uri /csp"], &[]);
    let violations = policies.inline_violations(
        alo_net::csp::Inline::Script,
        None,
        Content::element("steal()"),
    );
    let one = violations.first().expect("a violation");
    assert_eq!(one.blocked, Blocked::Inline);
    let posting = one.posts(&about());
    let body = body_of(&posting.posts);
    assert!(body.contains("\"blocked-uri\":\"inline\""), "{body}");
}

/// The report's own shape: which directive, which URL, which policy, and which
/// disposition — the four things an author needs to find the script their tag
/// manager added.
#[test]
fn a_report_says_which_directive_and_which_policy_decided() {
    let policies = policies(&["default-src 'none'; report-uri /csp"], &[]);
    let violations = policies.violations(&wants("https://evil.test/x.js", Purpose::Script), None);
    let posting = violations.first().expect("a violation").posts(&about());
    let body = body_of(&posting.posts);

    for expected in [
        "\"document-uri\":\"https://shop.example.com/checkout?step=2\"",
        "\"referrer\":\"https://search.example/\"",
        "\"blocked-uri\":\"https://evil.test\"",
        // `default-src` decided it; the *effective* directive is the one that
        // governs a script, which is what the specification's field means.
        "\"effective-directive\":\"script-src\"",
        "\"violated-directive\":\"script-src\"",
        "\"original-policy\":\"default-src 'none'; report-uri /csp\"",
        "\"disposition\":\"enforce\"",
        "\"status-code\":200",
    ] {
        assert!(body.contains(expected), "{expected} is not in {body}");
    }

    // The fields nothing here can honestly fill are absent rather than zero.
    for invented in [
        "line-number",
        "column-number",
        "source-file",
        "script-sample",
    ] {
        assert!(
            !body.contains(invented),
            "{invented} was invented in {body}"
        );
    }
}

/// The rule that decides what a report may say. A report is posted to a server
/// the *page* chose, so a full cross-origin URL in one would be a way for a
/// page to read a URL it was refused — a redirect's destination, a capability
/// token in somebody else's query.
#[test]
fn a_cross_origin_url_reaches_a_collector_as_an_origin_and_nothing_more() {
    let policies = policies(&["script-src 'self'; report-uri /csp"], &[]);
    let violations = policies.violations(
        &wants(
            "https://evil.test/steal.js?session=s3cret#anchor",
            Purpose::Script,
        ),
        None,
    );
    let body = body_of(
        &violations
            .first()
            .expect("a violation")
            .posts(&about())
            .posts,
    );

    assert!(
        body.contains("\"blocked-uri\":\"https://evil.test\""),
        "{body}"
    );
    assert!(
        !body.contains("s3cret"),
        "a token reached a collector: {body}"
    );
    assert!(
        !body.contains("steal.js"),
        "a path reached a collector: {body}"
    );
    assert!(
        !body.contains("anchor"),
        "a fragment reached a collector: {body}"
    );
}

/// A `data:` URL's body *is* the content, so a report naming one in full would
/// post whatever the page was refused.
#[test]
fn a_data_url_is_reported_as_its_scheme_alone() {
    let policies = policies(&["img-src 'self'; report-uri /csp"], &[]);
    let violations = policies.violations(
        &wants("data:image/png;base64,AAAASECRETAAAA", Purpose::Image),
        None,
    );
    let body = body_of(
        &violations
            .first()
            .expect("a violation")
            .posts(&about())
            .posts,
    );
    assert!(body.contains("\"blocked-uri\":\"data\""), "{body}");
    assert!(!body.contains("SECRET"), "{body}");
}

/// `report-to` names a group, and a group means something only against the
/// `Reporting-Endpoints` header of the same response.
#[test]
fn a_group_name_is_resolved_against_the_pages_own_endpoints() {
    let policies = policies(&["script-src 'self'; report-to collector"], &[]);
    let violations = policies.violations(&wants("https://evil.test/x.js", Purpose::Script), None);
    let one = violations.first().expect("a violation");

    let mut headers = Headers::new();
    headers.add(
        "Reporting-Endpoints",
        "collector=\"https://reports.example/csp\"",
    );
    let posting = one.posts(&about().reporting_to(Endpoints::stated_by(&headers)));
    let post = posting.posts.first().expect("a post");
    assert_eq!(post.url.to_string(), "https://reports.example/csp");
    assert_eq!(
        post.headers.get("Content-Type"),
        Some("application/reports+json"),
    );
    let body = String::from_utf8_lossy(&post.body).into_owned();
    assert!(
        body.starts_with('['),
        "the Reporting API's envelope is a list"
    );
    assert!(body.contains("\"type\":\"csp-violation\""), "{body}");
    assert!(
        body.contains("\"effectiveDirective\":\"script-src\""),
        "{body}"
    );
    assert!(
        body.contains("\"blockedURL\":\"https://evil.test\""),
        "{body}"
    );

    // And a group nobody defined is said rather than swallowed.
    let nowhere = one.posts(&about());
    assert!(nowhere.posts.is_empty());
    assert_eq!(nowhere.unusable.len(), 1, "{:?}", nowhere.unusable);
}

/// Whatever a server writes into a reporting directive, no report is posted
/// somewhere it should not be and nothing panics.
#[test]
fn nothing_a_server_can_write_turns_a_report_into_a_way_in() {
    for value in [
        "script-src 'self'; report-uri",
        "script-src 'self'; report-uri javascript:alert(1)",
        "script-src 'self'; report-uri file:///etc/passwd",
        "script-src 'self'; report-uri data:text/plain,x",
        "script-src 'self'; report-uri ///",
        "script-src 'self'; report-uri \u{0}",
        "script-src 'self'; report-to",
        "script-src 'self'; report-to one two three",
        "script-src 'self'; report-to \u{0}",
    ] {
        let policies = policies(&[value], &[]);
        let violations =
            policies.violations(&wants("https://evil.test/x.js", Purpose::Script), None);
        let one = violations.first().expect("a violation");
        for post in &one.posts(&about()).posts {
            assert!(
                matches!(post.url.scheme.as_str(), "http" | "https"),
                "{value:?} posted a report to {}",
                post.url,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Over a socket, which is the half that is not a pure function.
// ---------------------------------------------------------------------------

fn pool() -> alo_net::Pool {
    // Trusting nobody: nothing here is `https`. Half a second of patience so
    // that the test about a collector nobody is running takes half a second.
    alo_net::Pool::with_trust(Trust::of(&[]).unwrap_or_else(|_| unreachable_trust()))
        .patient_for(Duration::from_millis(500))
}

fn unreachable_trust() -> Trust {
    Trust::of(&[]).unwrap_or_else(|_| unreachable_trust())
}

/// Take one request, hand back its bytes, and answer with `response`.
fn collector(response: &'static str) -> (u16, mpsc::Receiver<Vec<u8>>) {
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
            // A report is a head and a body, and the body's length is in the
            // head — so stop when the whole of it has arrived rather than
            // waiting for a client that has nothing more to say.
            if whole_request(&asked) {
                break;
            }
        }
        let _ = say.send(asked);
        let _ = socket.write_all(response.as_bytes());
        let _ = socket.flush();
    });
    (port, heard)
}

/// Whether these bytes are a whole request: a head, and as many body bytes as
/// its `Content-Length` promised.
fn whole_request(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    let Some((head, body)) = text.split_once("\r\n\r\n") else {
        return false;
    };
    let declared = head
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .and_then(|length| length.trim().parse::<usize>().ok())
        .unwrap_or(0);
    body.len() >= declared
}

/// A page served from this machine, so that a report to this machine is one a
/// local page made. See the test below for what happens when it is not.
fn local_page(port: u16) -> Page {
    Page::at(url(&format!("http://127.0.0.1:{port}/checkout")), a_page())
}

/// The item's third clause, from the other side: the report is actually posted,
/// as a `POST`, saying what it is, with the report as its body.
#[test]
fn a_report_arrives_at_the_collector_as_a_post() {
    let (port, heard) = collector("HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n");
    assert!(port != 0, "loopback is unavailable");

    let policies = policies(&["script-src 'self'; report-uri /csp"], &[]);
    let asked = Request::get(url("https://evil.test/x.js"), a_page())
        .for_purpose(Purpose::Script)
        .asked_by(Origin::of(&url(&format!(
            "http://127.0.0.1:{port}/checkout"
        ))));
    let violations = policies.violations(&asked, None);
    let posting = violations
        .first()
        .expect("a violation")
        .posts(&local_page(port));

    let failed = pool().report(&posting.posts);
    assert!(failed.is_empty(), "{failed:?}");

    let sent = heard
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_default();
    let sent = String::from_utf8_lossy(&sent).into_owned();
    assert!(sent.starts_with("POST /csp HTTP/1.1\r\n"), "{sent}");
    assert!(
        sent.contains("Content-Type: application/csp-report"),
        "{sent}"
    );
    assert!(sent.contains("\"csp-report\""), "{sent}");
    assert!(
        sent.contains("\"effective-directive\":\"script-src\""),
        "{sent}"
    );
    assert!(
        sent.contains("\"blocked-uri\":\"https://evil.test\""),
        "{sent}"
    );
}

/// The item's own clause: a report that cannot be sent is not a load that
/// fails. The collector here is a port nobody is listening on, which is what a
/// collector that has been decommissioned looks like from the outside.
#[test]
fn a_report_that_cannot_be_sent_is_not_a_load_that_fails() {
    // Bind and drop, so the port is one nothing answers on.
    let Ok(listener) = TcpListener::bind("127.0.0.1:0") else {
        panic!("loopback is unavailable");
    };
    let port = listener
        .local_addr()
        .map(|at| at.port())
        .unwrap_or_default();
    drop(listener);

    let policies = policies(
        &[&format!(
            "script-src 'self'; report-uri http://127.0.0.1:{port}/csp"
        )],
        &[],
    );
    let asked = Request::get(url("https://evil.test/x.js"), a_page())
        .for_purpose(Purpose::Script)
        .asked_by(Origin::of(&url(&format!(
            "http://127.0.0.1:{port}/checkout"
        ))));
    let violations = policies.violations(&asked, None);
    let posting = violations
        .first()
        .expect("a violation")
        .posts(&local_page(port));

    let failed = pool().report(&posting.posts);
    assert_eq!(
        failed.len(),
        1,
        "a dead collector was reported as delivered"
    );
    let said = failed.join(" ");
    assert!(said.contains("could not be posted"), "{said}");
    assert!(said.contains(&port.to_string()), "{said}");

    // And the load's own answer is exactly what it was before anybody tried.
    assert!(policies.allows(&asked, None).is_err());
}

/// A collector that answers an error is a delivery that did not happen, and
/// saying so is the difference between "nothing was violated" and "nobody
/// heard about it".
#[test]
fn a_collector_that_answers_an_error_is_named_rather_than_believed() {
    let (port, _heard) = collector("HTTP/1.1 500 Server Error\r\nContent-Length: 0\r\n\r\n");
    assert!(port != 0, "loopback is unavailable");

    let one = Request::sending(
        url(&format!("http://127.0.0.1:{port}/csp")),
        "POST",
        b"{}".to_vec(),
        a_page(),
    )
    .for_purpose(Purpose::Report);
    let failed = pool().report(&[one]);
    assert_eq!(failed.len(), 1);
    assert!(failed.join(" ").contains("500"), "{failed:?}");
}

/// A page on the public web must not be able to point `report-uri` at the
/// machine the browser is running on. That is queue item 58's rebinding rule
/// reaching reporting, and it reaches it because a report carries the page's
/// own origin as its initiator — which is what makes it attributable at all.
#[test]
fn a_public_page_cannot_report_to_the_machine_the_browser_is_on() {
    let (port, _heard) = collector("HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n");
    assert!(port != 0, "loopback is unavailable");

    let policies = policies(
        &[&format!(
            "script-src 'self'; report-uri http://127.0.0.1:{port}/probe"
        )],
        &[],
    );
    let violations = policies.violations(&wants("https://evil.test/x.js", Purpose::Script), None);
    let posting = violations.first().expect("a violation").posts(&about());
    assert_eq!(posting.posts.len(), 1, "it is a URL, and it resolves");

    let failed = pool().report(&posting.posts);
    assert_eq!(failed.len(), 1, "a public page probed this machine");
}

/// Every test above that builds a body reads it the same way.
fn body_of(posts: &[Request]) -> String {
    posts
        .iter()
        .map(|post| String::from_utf8_lossy(&post.body).into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

/// A violation is built by a policy objecting and by nothing else, which is
/// what stops a caller reporting something no policy said.
#[test]
fn a_violation_reads_as_what_it_did_rather_than_as_a_block() {
    let watched = policies(&[], &["script-src 'none'; report-uri /csp"]);
    let violations: Vec<Violation> =
        watched.violations(&wants("https://evil.test/x.js", Purpose::Script), None);
    let said = violations.first().expect("a violation").to_string();
    assert!(said.contains("nothing was blocked"), "{said}");
}
