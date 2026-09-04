/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The preflight cache, written from the attacker's side.
//!
//! Item 61's tests set the rule for this file: name the thing somebody is
//! trying to do, not the header it turns on. A cache is a way of *not* asking a
//! question, so every interesting failure here is a permission obtained without
//! the server granting it — and every test below is named for one.
//!
//! Nothing sleeps. Every moment is passed in, which is the only way a table of
//! expiries says anything: the pairs either side of an expiry are the point.

use alo_net::cause::{Cause, Identities};
use alo_net::cors::Credentials;
use alo_net::preflight::{LONGEST_MEMORY, MOST_KEPT, Preflights, WHEN_NOBODY_SAYS};
use alo_net::{Headers, Partition, Request, Response, Status};
use alo_url::Origin;
use std::time::{Duration, SystemTime};

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

/// The site a page is being read inside.
fn inside(top_level: &str) -> Partition {
    Partition::of(&url(top_level))
}

/// A request from `page` for `target`, with a method and some headers.
fn asked_by(page: &str, target: &str, method: &str, headers: &[(&str, &str)]) -> Request {
    let mut request = Request::get(url(target), a_page()).asked_by(Origin::of(&url(page)));
    method.clone_into(&mut request.method);
    for (name, value) in headers {
        request.headers.add(*name, *value);
    }
    request
}

/// A preflight answer from `target`.
fn answered(target: &str, headers: &[(&str, &str)]) -> Response {
    let mut carried = Headers::new();
    for (name, value) in headers {
        carried.add(*name, *value);
    }
    Response {
        url: url(target),
        status: Status(204),
        headers: carried,
        body: Vec::new(),
    }
}

/// The ordinary answer: this origin, this method, for an hour.
fn allowing(methods: &str, headers: &str, max_age: &str) -> Response {
    answered(
        "https://api.example/thing",
        &[
            ("Access-Control-Allow-Origin", "https://app.example"),
            ("Access-Control-Allow-Methods", methods),
            ("Access-Control-Allow-Headers", headers),
            ("Access-Control-Max-Age", max_age),
        ],
    )
}

/// A fixed moment, so that every expiry in this file is named rather than
/// waited for.
fn noon() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000)
}

// --- The two clauses the item was opened for ---------------------------------

/// The point of the whole file: one question, one `OPTIONS`.
#[test]
fn a_second_request_of_the_same_shape_asks_nobody() {
    let mut remembered = Preflights::new();
    let site = inside("https://app.example/");
    let request = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "DELETE",
        &[],
    );

    assert!(
        remembered.must_ask(&request, Credentials::Omit, &site, noon()),
        "the first DELETE went out with nobody asked"
    );
    assert!(
        remembered
            .allowed(
                &request,
                Credentials::Omit,
                &site,
                &allowing("DELETE", "", "3600"),
                noon()
            )
            .is_ok()
    );
    assert!(
        !remembered.must_ask(
            &request,
            Credentials::Omit,
            &site,
            noon() + Duration::from_secs(1)
        ),
        "the same request was preflighted twice"
    );
    assert_eq!(remembered.counts(), (1, 1), "asked once, spared once");
}

/// A remembered answer is about a shape, not about a URL. Anything the server
/// was not shown is a question it has not answered.
#[test]
fn a_request_of_a_different_shape_is_asked_about_again() {
    let mut remembered = Preflights::new();
    let site = inside("https://app.example/");
    let deleting = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "DELETE",
        &[],
    );
    assert!(
        remembered
            .allowed(
                &deleting,
                Credentials::Omit,
                &site,
                &allowing("DELETE", "", "3600"),
                noon()
            )
            .is_ok()
    );

    let putting = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "PUT",
        &[],
    );
    assert!(
        remembered.must_ask(&putting, Credentials::Omit, &site, noon()),
        "a PUT went out on the strength of a DELETE being allowed"
    );

    let with_a_header = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "DELETE",
        &[("X-Custom", "1")],
    );
    assert!(
        remembered.must_ask(&with_a_header, Credentials::Omit, &site, noon()),
        "a header the server never saw went out unasked about"
    );

    let elsewhere = asked_by(
        "https://app.example/",
        "https://api.example/other",
        "DELETE",
        &[],
    );
    assert!(
        remembered.must_ask(&elsewhere, Credentials::Omit, &site, noon()),
        "a permission for one endpoint covered another"
    );
}

/// The clock is the caller's, so the two moments either side of an expiry are
/// both nameable. That pair is the test; a single moment in the middle would
/// pass against a cache that never expired anything.
#[test]
fn an_answer_stops_being_good_on_the_second_the_server_named() {
    let mut remembered = Preflights::new();
    let site = inside("https://app.example/");
    let request = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "DELETE",
        &[],
    );
    assert!(
        remembered
            .allowed(
                &request,
                Credentials::Omit,
                &site,
                &allowing("DELETE", "", "60"),
                noon()
            )
            .is_ok()
    );

    for (after, expected) in [(59, false), (60, true), (61, true)] {
        let moment = noon() + Duration::from_secs(after);
        assert_eq!(
            remembered.must_ask(&request, Credentials::Omit, &site, moment),
            expected,
            "{after}s after an answer good for 60s"
        );
        if !expected {
            // Re-remember, so the sequence above is read as three independent
            // questions rather than as one that consumed the entry.
            assert!(
                remembered
                    .allowed(
                        &request,
                        Credentials::Omit,
                        &site,
                        &allowing("DELETE", "", "60"),
                        noon()
                    )
                    .is_ok()
            );
        }
    }
    assert!(
        remembered.is_empty(),
        "an answer that had expired was skipped over rather than dropped"
    );
}

/// A server that says nothing has said nothing, and Fetch's five seconds is
/// what that means.
#[test]
fn a_server_that_said_nothing_is_remembered_for_five_seconds() {
    let mut remembered = Preflights::new();
    let site = inside("https://app.example/");
    let request = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "DELETE",
        &[],
    );
    let no_max_age = answered(
        "https://api.example/thing",
        &[
            ("Access-Control-Allow-Origin", "https://app.example"),
            ("Access-Control-Allow-Methods", "DELETE"),
        ],
    );
    assert!(
        remembered
            .allowed(&request, Credentials::Omit, &site, &no_max_age, noon())
            .is_ok()
    );
    assert!(!remembered.must_ask(
        &request,
        Credentials::Omit,
        &site,
        noon() + WHEN_NOBODY_SAYS - Duration::from_secs(1)
    ));
    assert!(
        remembered.must_ask(
            &request,
            Credentials::Omit,
            &site,
            noon() + WHEN_NOBODY_SAYS
        ),
        "a server that said nothing was taken to have granted something lasting"
    );
}

// --- Permissions nobody granted ----------------------------------------------

/// The refusal must not be the thing that gets remembered. A cache that stored
/// what it was told regardless of the verdict would turn one refused preflight
/// into a permanent bypass.
#[test]
fn a_permission_the_server_refused_is_never_remembered() {
    let mut remembered = Preflights::new();
    let site = inside("https://app.example/");
    let request = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "DELETE",
        &[],
    );

    let wrong_method = allowing("GET, POST", "", "3600");
    assert!(
        remembered
            .allowed(&request, Credentials::Omit, &site, &wrong_method, noon())
            .is_err(),
        "a DELETE was allowed by an answer naming GET and POST"
    );
    assert!(remembered.is_empty(), "a refused answer was remembered");
    assert!(
        remembered.must_ask(&request, Credentials::Omit, &site, noon()),
        "a refused preflight became a permission"
    );

    let wrong_origin = answered(
        "https://api.example/thing",
        &[
            ("Access-Control-Allow-Origin", "https://somebody.example"),
            ("Access-Control-Allow-Methods", "DELETE"),
        ],
    );
    assert!(
        remembered
            .allowed(&request, Credentials::Omit, &site, &wrong_origin, noon())
            .is_err()
    );
    assert!(remembered.is_empty());
}

/// The server was never shown the harder question. An answer about a request
/// with no cookies says nothing about one that carries them.
#[test]
fn an_answer_given_without_cookies_does_not_cover_a_request_that_carries_them() {
    let mut remembered = Preflights::new();
    let site = inside("https://app.example/");
    let request = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "DELETE",
        &[],
    );
    assert!(
        remembered
            .allowed(
                &request,
                Credentials::Omit,
                &site,
                &allowing("DELETE", "", "3600"),
                noon()
            )
            .is_ok()
    );
    assert!(
        remembered.must_ask(&request, Credentials::Include, &site, noon()),
        "a credentialled DELETE went out on an answer given to one without credentials"
    );
    assert!(
        !remembered.must_ask(&request, Credentials::SameOrigin, &site, noon()),
        "the answer did not cover the request it was actually given for"
    );
}

/// And the other direction is fine: a server that agreed to be read by this
/// origin *with* credentials has agreed to the stricter case too.
#[test]
fn an_answer_given_with_cookies_covers_a_request_without_them() {
    let mut remembered = Preflights::new();
    let site = inside("https://app.example/");
    let request = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "DELETE",
        &[],
    );
    let credentialled = answered(
        "https://api.example/thing",
        &[
            ("Access-Control-Allow-Origin", "https://app.example"),
            ("Access-Control-Allow-Credentials", "true"),
            ("Access-Control-Allow-Methods", "DELETE"),
            ("Access-Control-Max-Age", "3600"),
        ],
    );
    assert!(
        remembered
            .allowed(
                &request,
                Credentials::Include,
                &site,
                &credentialled,
                noon()
            )
            .is_ok()
    );
    assert!(!remembered.must_ask(&request, Credentials::Include, &site, noon()));
    assert!(!remembered.must_ask(&request, Credentials::Omit, &site, noon()));
}

/// `*` is "and anything else you care to ask", which is a sentence about the
/// request in front of the server. Remembering it as a wildcard would let a
/// page obtain a `DELETE` on the strength of a `PUT` having been allowed.
#[test]
fn a_wildcard_is_remembered_as_what_it_allowed_rather_than_as_a_licence() {
    let mut remembered = Preflights::new();
    let site = inside("https://app.example/");
    let putting = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "PUT",
        &[],
    );
    assert!(
        remembered
            .allowed(
                &putting,
                Credentials::Omit,
                &site,
                &allowing("*", "*", "3600"),
                noon()
            )
            .is_ok()
    );
    assert!(
        !remembered.must_ask(&putting, Credentials::Omit, &site, noon()),
        "the answer did not cover the request that produced it"
    );

    let deleting = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "DELETE",
        &[],
    );
    assert!(
        remembered.must_ask(&deleting, Credentials::Omit, &site, noon()),
        "a wildcard was remembered as a standing permission for every method"
    );

    let with_a_header = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "PUT",
        &[("X-Custom", "1")],
    );
    assert!(
        remembered.must_ask(&with_a_header, Credentials::Omit, &site, noon()),
        "a wildcard was remembered as a standing permission for every header"
    );
}

/// A server has to name `Authorization`, and the cache must not become the way
/// round that. It needs no rule of its own: a header is remembered only when the
/// server named it, and this is the test that says so.
#[test]
fn a_remembered_answer_never_hands_over_authorization_the_server_did_not_name() {
    let mut remembered = Preflights::new();
    let site = inside("https://app.example/");
    let plain = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "DELETE",
        &[("X-Custom", "1")],
    );
    assert!(
        remembered
            .allowed(
                &plain,
                Credentials::Omit,
                &site,
                &allowing("DELETE", "*", "3600"),
                noon()
            )
            .is_ok()
    );

    let with_a_token = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "DELETE",
        &[("Authorization", "Bearer a-real-token")],
    );
    assert!(
        remembered.must_ask(&with_a_token, Credentials::Omit, &site, noon()),
        "a token went out on an answer that never mentioned Authorization"
    );
}

/// A JSON post is not something a form could have sent, and the header it is
/// unsafe *by* is `Content-Type`. A cache that decided a request's shape by
/// header name alone would let any content type through on one answer.
#[test]
fn a_content_type_a_form_could_not_have_meant_is_part_of_the_question() {
    let mut remembered = Preflights::new();
    let site = inside("https://app.example/");
    let json = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "POST",
        &[("Content-Type", "application/json")],
    );
    assert!(
        remembered.must_ask(&json, Credentials::Omit, &site, noon()),
        "a JSON post was sent with nobody asked"
    );
    // The server has to name it, exactly as it would any other unsafe header.
    assert!(
        remembered
            .allowed(
                &json,
                Credentials::Omit,
                &site,
                &allowing("POST", "", "3600"),
                noon()
            )
            .is_err(),
        "a JSON post was allowed by an answer that never mentioned Content-Type"
    );
    assert!(
        remembered
            .allowed(
                &json,
                Credentials::Omit,
                &site,
                &allowing("POST", "content-type", "3600"),
                noon()
            )
            .is_ok()
    );
    assert!(!remembered.must_ask(&json, Credentials::Omit, &site, noon()));

    // And a form's own content type is not part of the question at all: such a
    // request was never preflighted in the first place.
    let form = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "POST",
        &[("Content-Type", "text/plain")],
    );
    assert!(!remembered.must_ask(&form, Credentials::Omit, &site, noon()));
    assert_eq!(
        remembered.counts().1,
        1,
        "a request that needed no preflight was counted as one this cache spared"
    );
}

// --- Who is asking, and inside what ------------------------------------------

/// ADR 0011 section 1's argument, on this cache: an entry that makes one site's
/// request faster because *another* site already asked answers "have you been
/// there" to anybody who times a load, and survives clearing cookies.
#[test]
fn one_site_does_not_learn_that_another_site_already_asked() {
    let mut remembered = Preflights::new();
    let tracker = asked_by(
        "https://ads.example/",
        "https://api.example/thing",
        "DELETE",
        &[],
    );
    assert!(
        remembered
            .allowed(
                &tracker,
                Credentials::Omit,
                &inside("https://news.example/"),
                &answered(
                    "https://api.example/thing",
                    &[
                        ("Access-Control-Allow-Origin", "https://ads.example"),
                        ("Access-Control-Allow-Methods", "DELETE"),
                        ("Access-Control-Max-Age", "3600"),
                    ],
                ),
                noon()
            )
            .is_ok()
    );
    assert!(
        !remembered.must_ask(
            &tracker,
            Credentials::Omit,
            &inside("https://news.example/"),
            noon()
        ),
        "the site that asked did not get its own answer back"
    );
    assert!(
        remembered.must_ask(
            &tracker,
            Credentials::Omit,
            &inside("https://shop.example/"),
            noon()
        ),
        "the same embedded page learned it had been somewhere else"
    );
    // The partition is the registrable domain since queue item 156, so a
    // subdomain of the same site is the same site.
    assert!(!remembered.must_ask(
        &tracker,
        Credentials::Omit,
        &inside("https://www.news.example/"),
        noon()
    ));
}

/// An answer about one page's origin says nothing about another's, even inside
/// the same site.
#[test]
fn a_page_does_not_use_an_answer_given_about_a_different_origin() {
    let mut remembered = Preflights::new();
    let site = inside("https://app.example/");
    let request = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "DELETE",
        &[],
    );
    assert!(
        remembered
            .allowed(
                &request,
                Credentials::Omit,
                &site,
                &allowing("DELETE", "", "3600"),
                noon()
            )
            .is_ok()
    );

    let neighbour = asked_by(
        "https://other.app.example/",
        "https://api.example/thing",
        "DELETE",
        &[],
    );
    assert!(
        remembered.must_ask(&neighbour, Credentials::Omit, &site, noon()),
        "an answer named for one origin was used by another"
    );
}

/// Every opaque origin serialises to `null`, so a key holding one would be
/// shared between two pages that are by definition not each other — the rule
/// `alo_url` states as a type and this file has to keep.
#[test]
fn an_opaque_origin_is_never_a_key() {
    let mut remembered = Preflights::new();
    let site = inside("https://app.example/");
    let mut sandboxed = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "DELETE",
        &[],
    );
    sandboxed.initiator = Some(Origin::of(&url("data:text/html,<p>hello")));
    assert!(sandboxed.initiator.as_ref().is_some_and(Origin::is_opaque));

    let permissive = answered(
        "https://api.example/thing",
        &[
            ("Access-Control-Allow-Origin", "*"),
            ("Access-Control-Allow-Methods", "DELETE"),
            ("Access-Control-Max-Age", "3600"),
        ],
    );
    assert!(
        remembered
            .allowed(&sandboxed, Credentials::Omit, &site, &permissive, noon())
            .is_ok(),
        "the answer itself was fine; it is the remembering that must not happen"
    );
    assert!(remembered.is_empty(), "an opaque origin became a key");
    assert!(remembered.must_ask(&sandboxed, Credentials::Omit, &site, noon()));
}

// --- Bytes a stranger sent ---------------------------------------------------

/// `Access-Control-Max-Age` is a number a server chose, which means it is not
/// necessarily a number. Every one of these is an answer rather than a panic.
#[test]
fn a_max_age_nobody_could_read_is_an_answer_rather_than_a_crash() {
    let site = inside("https://app.example/");
    let request = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "DELETE",
        &[],
    );

    // (what the server sent, how long that is worth, and what it says). `None`
    // is not remembered at all, which is a different outcome from remembered
    // briefly — and conflating the two is how a header nobody can read comes to
    // mean the same thing as a header saying "do not".
    let table: [(&str, Option<Duration>, &str); 12] = [
        ("0", None, "zero is a server declining to be remembered"),
        ("-1", None, "how a server spells do not remember this"),
        ("-99999", None, "negative is negative however large"),
        (
            "",
            Some(WHEN_NOBODY_SAYS),
            "an empty header is not a number, so it is a server saying nothing",
        ),
        ("   ", Some(WHEN_NOBODY_SAYS), "nor is whitespace"),
        ("abc", Some(WHEN_NOBODY_SAYS), "nor is a word"),
        (
            "3.5",
            Some(WHEN_NOBODY_SAYS),
            "nor is a fraction, in this grammar",
        ),
        ("1e9", Some(WHEN_NOBODY_SAYS), "nor is exponent notation"),
        (
            " 3600 ",
            Some(Duration::from_secs(3600)),
            "surrounding space is not the number",
        ),
        (
            "+3600",
            Some(Duration::from_secs(3600)),
            "a sign the standard does not use, still a number",
        ),
        (
            "99999999999999999999999999",
            Some(LONGEST_MEMORY),
            "larger than any integer, and enormously above the cap",
        ),
        (
            "9223372036854775808",
            Some(LONGEST_MEMORY),
            "one past i64, and the same reading",
        ),
    ];

    for (sent, worth, why) in table {
        let mut remembered = Preflights::new();
        assert!(
            remembered
                .allowed(
                    &request,
                    Credentials::Omit,
                    &site,
                    &allowing("DELETE", "", sent),
                    noon()
                )
                .is_ok(),
            "{sent:?} was not even readable as an answer"
        );
        let Some(worth) = worth else {
            assert!(
                remembered.is_empty(),
                "Access-Control-Max-Age: {sent:?} was remembered — {why}"
            );
            continue;
        };
        // The pair either side of the expiry, which is the only pair that says
        // the number was read rather than merely accepted.
        assert!(
            !remembered.must_ask(
                &request,
                Credentials::Omit,
                &site,
                noon() + worth - Duration::from_secs(1)
            ),
            "Access-Control-Max-Age: {sent:?} was forgotten too early — {why}"
        );
        assert!(
            remembered.must_ask(&request, Credentials::Omit, &site, noon() + worth),
            "Access-Control-Max-Age: {sent:?} outlived what it was worth — {why}"
        );
    }
}

/// A preflight answer is a permission, and a permission nobody can revoke is
/// not one. A server that wrote a year once should not have to wait a year out.
#[test]
fn a_server_cannot_ask_to_be_remembered_for_ever() {
    let mut remembered = Preflights::new();
    let site = inside("https://app.example/");
    let request = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "DELETE",
        &[],
    );
    assert!(
        remembered
            .allowed(
                &request,
                Credentials::Omit,
                &site,
                &allowing("DELETE", "", "31536000"),
                noon()
            )
            .is_ok()
    );
    assert!(!remembered.must_ask(
        &request,
        Credentials::Omit,
        &site,
        noon() + LONGEST_MEMORY - Duration::from_secs(1)
    ));
    assert!(
        remembered.must_ask(&request, Credentials::Omit, &site, noon() + LONGEST_MEMORY),
        "a year the server asked for was a year it got"
    );
}

/// The last moment this machine can name, to within a second.
///
/// There is no constant for it — `SystemTime`'s range is the platform's — so it
/// is found by setting each bit that still fits, high to low. Adding to it is
/// what the code under test must refuse rather than panic on, and a test that
/// panicked while building the argument would have proved nothing.
fn the_end_of_time() -> SystemTime {
    let mut end = SystemTime::UNIX_EPOCH;
    for shift in (0..64).rev() {
        if let Some(later) = end.checked_add(Duration::from_secs(1u64 << shift)) {
            end = later;
        }
    }
    end
}

/// A clock so near the end of representable time that the cap does not fit in
/// it is not one to do arithmetic on. Not remembering is always correct.
#[test]
fn a_clock_at_the_end_of_time_is_refused_rather_than_overflowed() {
    let mut remembered = Preflights::new();
    let site = inside("https://app.example/");
    let request = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "DELETE",
        &[],
    );
    let end = the_end_of_time();
    assert!(
        end.checked_add(LONGEST_MEMORY).is_none(),
        "the end of time was not found, so this asserts nothing"
    );
    assert!(
        remembered
            .allowed(
                &request,
                Credentials::Omit,
                &site,
                &allowing("DELETE", "", "3600"),
                end
            )
            .is_ok()
    );
    assert!(remembered.is_empty());
}

/// Without a bound, a page that preflights two thousand URLs has made the
/// browser hold two thousand answers for as long as it runs.
#[test]
fn the_answers_held_do_not_grow_with_what_a_page_asks_for() {
    let mut remembered = Preflights::new();
    let site = inside("https://app.example/");
    for which in 0..MOST_KEPT * 2 {
        let request = asked_by(
            "https://app.example/",
            &format!("https://api.example/thing/{which}"),
            "DELETE",
            &[],
        );
        let answer = answered(
            &format!("https://api.example/thing/{which}"),
            &[
                ("Access-Control-Allow-Origin", "https://app.example"),
                ("Access-Control-Allow-Methods", "DELETE"),
                ("Access-Control-Max-Age", "3600"),
            ],
        );
        assert!(
            remembered
                .allowed(&request, Credentials::Omit, &site, &answer, noon())
                .is_ok()
        );
    }
    assert!(remembered.len() <= MOST_KEPT, "held {}", remembered.len());

    // The oldest went, and the newest is still there.
    let oldest = asked_by(
        "https://app.example/",
        "https://api.example/thing/0",
        "DELETE",
        &[],
    );
    assert!(remembered.must_ask(&oldest, Credentials::Omit, &site, noon()));
    let newest = asked_by(
        "https://app.example/",
        &format!("https://api.example/thing/{}", MOST_KEPT * 2 - 1),
        "DELETE",
        &[],
    );
    assert!(!remembered.must_ask(&newest, Credentials::Omit, &site, noon()));
}

/// What "clear this browsing data" has to be able to do here as much as
/// anywhere else.
#[test]
fn forgetting_everything_really_forgets_it() {
    let mut remembered = Preflights::new();
    let site = inside("https://app.example/");
    let request = asked_by(
        "https://app.example/",
        "https://api.example/thing",
        "DELETE",
        &[],
    );
    assert!(
        remembered
            .allowed(
                &request,
                Credentials::Omit,
                &site,
                &allowing("DELETE", "", "3600"),
                noon()
            )
            .is_ok()
    );
    remembered.empty();
    assert!(remembered.is_empty());
    assert!(remembered.must_ask(&request, Credentials::Omit, &site, noon()));
}
