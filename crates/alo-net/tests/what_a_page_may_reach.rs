/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Three policies a site states about itself, and the attacks they answer.
//!
//! Named the way item 61's tests were, for the same reason: a test called
//! `hsts_header_is_parsed` passes against an implementation that honours the
//! header over plain HTTP, which is the one thing it must never do.

use alo_net::hsts::Known;
use alo_net::mixed::{Verdict, is_trustworthy, what_to_do};
use alo_net::referrer::{Policy, for_request};
use alo_net::{Headers, Purpose, Request};
use alo_url::Origin;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const NOW: u64 = 1_700_000_000;

fn now() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(NOW)
}

fn later(seconds: u64) -> SystemTime {
    now() + Duration::from_secs(seconds)
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

fn saying(value: &str) -> Headers {
    let mut headers = Headers::new();
    headers.add("Strict-Transport-Security", value);
    headers
}

// --- HSTS --------------------------------------------------------------------

/// The attack: somebody types `example.com`, the browser tries `http://`, and a
/// network in between answers before the real server is ever asked. No amount
/// of correct TLS helps — the whole thing happens before any TLS begins.
#[test]
fn a_site_visited_once_is_never_reached_insecurely_again() {
    let mut known = Known::new();
    assert!(!known.must_be_secure("bank.example", now()));

    known.learn("bank.example", &saying("max-age=31536000"), true, now());
    assert!(
        known.must_be_secure("bank.example", now()),
        "the site said never to do that again and was not believed"
    );
    assert!(known.must_be_secure("bank.example", later(1000)));
    assert!(!known.must_be_secure("bank.example", later(31_536_001)));
}

/// The rule that turns this from a defence into a denial of service if it is
/// missed. An attacker already rewriting your traffic could otherwise pin any
/// domain for two years.
#[test]
fn an_attacker_rewriting_plain_http_cannot_pin_a_domain_forever() {
    let mut known = Known::new();
    known.learn(
        "somebody-elses-site.example",
        &saying("max-age=63072000; includeSubDomains"),
        false,
        now(),
    );
    assert!(
        !known.must_be_secure("somebody-elses-site.example", now()),
        "a header over plain HTTP was honoured"
    );
    assert!(known.is_empty());
}

/// An address belongs to whoever holds it today. A rule keyed on one would
/// follow the address rather than the site.
#[test]
fn an_address_cannot_pin_itself() {
    let mut known = Known::new();
    known.learn("192.168.1.1", &saying("max-age=31536000"), true, now());
    known.learn("[::1]", &saying("max-age=31536000"), true, now());
    assert!(known.is_empty());
    assert!(!known.must_be_secure("192.168.1.1", now()));
}

/// A site that could pin itself and never unpin would be a site nobody could
/// move off TLS in an emergency.
#[test]
fn a_site_can_release_itself() {
    let mut known = Known::new();
    known.learn("shop.example", &saying("max-age=31536000"), true, now());
    assert!(known.must_be_secure("shop.example", now()));
    known.learn("shop.example", &saying("max-age=0"), true, now());
    assert!(!known.must_be_secure("shop.example", now()));
}

/// A header with no `max-age` says nothing, and saying nothing must not clear a
/// pin that was set properly earlier.
#[test]
fn a_header_that_says_nothing_does_not_undo_one_that_said_something() {
    let mut known = Known::new();
    known.learn("shop.example", &saying("max-age=31536000"), true, now());
    known.learn("shop.example", &saying("includeSubDomains"), true, now());
    assert!(known.must_be_secure("shop.example", now()));
}

/// The subdomain walk goes label by label. A suffix comparison says
/// `evil-example.com` is under `example.com`, and here that would mean an
/// attacker's domain inheriting somebody else's pin — or, worse, a pin somebody
/// could set on a name they do not own.
#[test]
fn covering_subdomains_covers_subdomains_and_not_lookalikes() {
    let mut known = Known::new();
    known.learn(
        "example.com",
        &saying("max-age=31536000; includeSubDomains"),
        true,
        now(),
    );
    assert!(known.must_be_secure("www.example.com", now()));
    assert!(known.must_be_secure("a.b.example.com", now()));
    assert!(
        !known.must_be_secure("evil-example.com", now()),
        "a lookalike domain inherited the pin"
    );
    assert!(!known.must_be_secure("example.com.evil.test", now()));
}

#[test]
fn without_include_subdomains_a_subdomain_is_not_covered() {
    let mut known = Known::new();
    known.learn("example.com", &saying("max-age=31536000"), true, now());
    assert!(known.must_be_secure("example.com", now()));
    assert!(!known.must_be_secure("www.example.com", now()));
}

/// A site that pins itself for a century after moving off TLS has removed
/// itself from this browser for a century.
#[test]
fn a_pin_longer_than_the_cap_is_capped() {
    let mut known = Known::new();
    known.learn("example.com", &saying("max-age=999999999"), true, now());
    assert!(known.must_be_secure("example.com", later(365 * 24 * 3600)));
    assert!(!known.must_be_secure("example.com", later(10 * 365 * 24 * 3600)));
}

// --- Mixed content -----------------------------------------------------------

fn asked_by(page: &str, target: &str, purpose: Purpose) -> Request {
    Request::get(url(target))
        .for_purpose(purpose)
        .asked_by(Origin::of(&url(page)))
}

/// A script replaced in transit does not look at the page. It *is* the page.
/// Nothing recovers from that, so nothing is offered.
#[test]
fn a_secure_page_cannot_load_a_script_over_plain_http_at_all() {
    for purpose in [Purpose::Script, Purpose::Style, Purpose::Fetch] {
        let request = asked_by(
            "https://bank.example/",
            "http://cdn.example/x",
            purpose.clone(),
        );
        assert!(
            matches!(what_to_do(&request), Verdict::Refused { .. }),
            "{purpose} was allowed onto a secure page over plain http"
        );
    }
}

/// An image replaced in transit is a wrong picture — bad, and not the same
/// thing. A great many sites have an `http://` URL in their markup and a
/// perfectly good `https://` server.
#[test]
fn an_image_is_tried_over_tls_rather_than_simply_refused() {
    let request = asked_by(
        "https://news.example/",
        "http://cdn.example/logo.png",
        Purpose::Image,
    );
    let Verdict::TryItSecurely { instead } = what_to_do(&request) else {
        panic!("an image was refused outright rather than upgraded");
    };
    assert_eq!(instead.serialised, "https://cdn.example/logo.png");
}

/// There is no network between the two ends, so there is nothing in between to
/// attack. Refusing it would break every developer on earth while protecting
/// nobody.
#[test]
fn localhost_over_plain_http_is_secure() {
    for target in [
        "http://localhost:8080/x.js",
        "http://127.0.0.1/x.js",
        "http://[::1]/x.js",
    ] {
        assert!(
            is_trustworthy(&url(target)),
            "{target} was treated as insecure"
        );
        let request = asked_by("https://app.example/", target, Purpose::Script);
        assert_eq!(
            what_to_do(&request),
            Verdict::Fine,
            "{target} was blocked as mixed content"
        );
    }
}

/// An insecure page loading insecure things is not *mixed*. It is consistently
/// bad, and it is the page's own problem rather than a promise being broken.
#[test]
fn an_insecure_page_loading_insecure_things_is_not_this_rule() {
    let request = asked_by(
        "http://old.example/",
        "http://old.example/x.js",
        Purpose::Script,
    );
    assert_eq!(what_to_do(&request), Verdict::Fine);
}

/// A person may visit an insecure site. That is their business and their
/// address bar, not a secure page reaching for something.
#[test]
fn a_person_typing_an_http_address_is_not_mixed_content() {
    let request = Request::get(url("http://old.example/"));
    assert_eq!(what_to_do(&request), Verdict::Fine);
}

// --- Referrer ----------------------------------------------------------------

/// A full URL carries the path and the query, and a great many of those are the
/// message. Sending them to every image host on a page hands them to people who
/// never asked and cannot be expected to protect them.
#[test]
fn another_site_is_not_told_which_page_you_were_reading() {
    let sent = for_request(
        Policy::default(),
        &url("https://clinic.example/results/hiv-test?patient=4821"),
        &url("https://cdn.example/logo.png"),
    );
    assert_eq!(
        sent.as_deref(),
        Some("https://clinic.example"),
        "the path and query went to a third party"
    );
}

#[test]
fn your_own_site_is_told_the_whole_url() {
    let sent = for_request(
        Policy::default(),
        &url("https://clinic.example/results/x?a=b"),
        &url("https://clinic.example/style.css"),
    );
    assert_eq!(
        sent.as_deref(),
        Some("https://clinic.example/results/x?a=b")
    );
}

/// The rule that holds under every policy but the one named for breaking it:
/// what we would be sending is exactly what an attacker on that connection is
/// there to read.
#[test]
fn nothing_is_sent_across_a_downgrade_to_plain_http() {
    for policy in [
        Policy::default(),
        Policy::StrictOrigin,
        Policy::NoReferrerWhenDowngrade,
    ] {
        assert_eq!(
            for_request(
                policy,
                &url("https://clinic.example/results/x"),
                &url("http://tracker.example/pixel")
            ),
            None,
            "{policy:?} sent a referrer down to plain http"
        );
    }
    // The one that does not, and it is named for what it is.
    assert!(
        for_request(
            Policy::UnsafeUrl,
            &url("https://clinic.example/results/x"),
            &url("http://tracker.example/pixel")
        )
        .is_some()
    );
}

/// A fragment is not sent to a server at all, and a password in a URL would end
/// up in every log the referrer passed through.
#[test]
fn a_fragment_is_never_sent() {
    let sent = for_request(
        Policy::UnsafeUrl,
        &url("https://example.com/page?a=b#the-secret-part"),
        &url("https://elsewhere.example/"),
    );
    assert_eq!(sent.as_deref(), Some("https://example.com/page?a=b"));
}

/// A site may send several so that browsers which know a strict policy take it
/// and older ones take a weaker one — so the last *known* value wins, not the
/// last value.
#[test]
fn the_last_policy_a_browser_understands_is_the_one_that_applies() {
    assert_eq!(
        Policy::from_header("no-referrer, strict-origin-when-cross-origin"),
        Some(Policy::StrictOriginWhenCrossOrigin)
    );
    assert_eq!(
        Policy::from_header("strict-origin, something-invented-later"),
        Some(Policy::StrictOrigin),
        "an unknown value at the end discarded a known one"
    );
}

/// A policy nobody can read leaves the default in place rather than weakening
/// it. The alternative is that a typo removes a protection.
#[test]
fn a_policy_nobody_can_read_leaves_the_default_alone() {
    assert_eq!(Policy::from_header("no-such-policy"), None);
    assert_eq!(Policy::default(), Policy::StrictOriginWhenCrossOrigin);
}

#[test]
fn no_referrer_sends_nothing_even_to_your_own_site() {
    assert_eq!(
        for_request(
            Policy::NoReferrer,
            &url("https://example.com/a"),
            &url("https://example.com/b")
        ),
        None
    );
}
