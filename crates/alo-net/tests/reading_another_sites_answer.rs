/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The same-origin policy, written from the attacker's side.
//!
//! The queue asked for *"a cross-origin read that should fail, in a test that
//! names the attack rather than the header."* So every test here is named for
//! what somebody is trying to do, and the header it turns on is a detail inside
//! it. A file full of `allow_origin_header_is_checked` would pass just as well
//! against an implementation that checked the wrong thing.

use alo_net::cors::{
    self, Credentials, asking_first, asking_first_allowed, may_read, needs_asking_first, readable,
};
use alo_net::{Headers, Request, Response, Status};
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

/// A request from `page` for `target`.
fn asked_by(page: &str, target: &str) -> Request {
    Request::get(url(target)).asked_by(Origin::of(&url(page)))
}

/// A response from `target` carrying these headers.
fn answered(target: &str, headers: &[(&str, &str)]) -> Response {
    let mut carried = Headers::new();
    for (name, value) in headers {
        carried.add(*name, *value);
    }
    Response {
        url: url(target),
        status: Status(200),
        headers: carried,
        body: b"the statement".to_vec(),
    }
}

// --- Reading somebody else's page --------------------------------------------

/// The attack the whole policy exists to stop.
#[test]
fn a_page_cannot_read_another_sites_answer_just_by_asking_for_it() {
    let asking = asked_by("https://evil.example/", "https://bank.example/statement");
    let answer = answered("https://bank.example/statement", &[]);
    let refused = may_read(&asking, Credentials::SameOrigin, &answer);
    assert!(
        refused.is_err(),
        "a page read another site's response with no permission at all"
    );
    let why = refused.err().map(|why| why.to_string()).unwrap_or_default();
    assert!(
        why.contains("Access-Control-Allow-Origin"),
        "the refusal should say what the server would have to send: {why:?}"
    );
}

/// A server that allows one origin has not allowed every origin.
#[test]
fn a_permission_for_one_site_does_not_let_another_site_in() {
    let asking = asked_by("https://evil.example/", "https://bank.example/statement");
    let answer = answered(
        "https://bank.example/statement",
        &[("Access-Control-Allow-Origin", "https://partner.example")],
    );
    assert!(may_read(&asking, Credentials::SameOrigin, &answer).is_err());

    // And the partner it was actually for may read it.
    let partner = asked_by("https://partner.example/", "https://bank.example/statement");
    assert!(may_read(&partner, Credentials::SameOrigin, &answer).is_ok());
}

/// `*` means "anyone may read this, and it contains nothing personal". A
/// request carrying cookies contradicts that by existing — so the two together
/// are refused, or every server that ever wrote `*` for a public file would be
/// handing over its logged-in pages too.
#[test]
fn a_wildcard_does_not_hand_over_a_page_that_was_fetched_with_cookies() {
    let asking = asked_by("https://evil.example/", "https://bank.example/statement");
    let answer = answered(
        "https://bank.example/statement",
        &[("Access-Control-Allow-Origin", "*")],
    );
    assert!(
        may_read(&asking, Credentials::Omit, &answer).is_ok(),
        "a wildcard on a request with no credentials is exactly what it means"
    );
    let refused = may_read(&asking, Credentials::Include, &answer);
    assert!(
        refused.is_err(),
        "a wildcard handed over a response fetched with the victim's cookies"
    );
    let why = refused.err().map(|why| why.to_string()).unwrap_or_default();
    assert!(why.contains("name this origin exactly"), "{why:?}");
}

/// Naming the origin is not enough on its own: a server that names an origin
/// but does not say credentials are allowed has not agreed to this.
#[test]
fn naming_the_origin_does_not_by_itself_permit_a_credentialled_read() {
    let asking = asked_by("https://app.example/", "https://api.example/me");
    let named_only = answered(
        "https://api.example/me",
        &[("Access-Control-Allow-Origin", "https://app.example")],
    );
    assert!(may_read(&asking, Credentials::Include, &named_only).is_err());

    let and_agreed = answered(
        "https://api.example/me",
        &[
            ("Access-Control-Allow-Origin", "https://app.example"),
            ("Access-Control-Allow-Credentials", "true"),
        ],
    );
    assert!(may_read(&asking, Credentials::Include, &and_agreed).is_ok());
}

/// A page on `https` and one on `http` are different origins, and so are two
/// ports. This is the check somebody writing a host comparison gets wrong.
#[test]
fn a_scheme_or_a_port_is_enough_to_make_it_somebody_else() {
    for target in [
        "http://bank.example/statement",
        "https://bank.example:8443/statement",
    ] {
        let asking = asked_by("https://bank.example/", target);
        let answer = answered(target, &[]);
        assert!(
            may_read(&asking, Credentials::SameOrigin, &answer).is_err(),
            "{target} was treated as the same origin as https://bank.example/"
        );
    }
    let same = asked_by("https://bank.example/", "https://bank.example/statement");
    assert!(
        may_read(
            &same,
            Credentials::SameOrigin,
            &answered("https://bank.example/statement", &[])
        )
        .is_ok()
    );
}

/// An opaque origin is the same as itself and nothing else — including another
/// opaque one. A comparison on the serialised string would make every `file:`
/// page and every sandboxed frame one origin, all reading each other.
#[test]
fn two_opaque_origins_are_not_the_same_origin_as_each_other() {
    let one = Origin::of(&url("file:///Users/somebody/a.html"));
    let two = Origin::of(&url("file:///Users/somebody/b.html"));
    assert_eq!(one.to_string(), "null");
    assert_eq!(two.to_string(), "null");
    assert_ne!(one, two, "two opaque origins compared equal");

    let asking = Request::get(url("https://bank.example/statement")).asked_by(one);
    assert!(
        may_read(
            &asking,
            Credentials::SameOrigin,
            &answered("https://bank.example/statement", &[])
        )
        .is_err()
    );
}

/// `null` is what an opaque origin serialises to, and a server that writes it
/// into `Access-Control-Allow-Origin` is not opening a door to one particular
/// page — it is opening one to every sandboxed frame and local file on earth.
/// This engine does the literal thing, and the test exists to make that
/// deliberate rather than accidental.
#[test]
fn a_server_allowing_null_is_allowing_every_sandboxed_page_at_once() {
    let opaque = Origin::of(&url("file:///Users/somebody/a.html"));
    let asking = Request::get(url("https://bank.example/statement")).asked_by(opaque);
    let answer = answered(
        "https://bank.example/statement",
        &[("Access-Control-Allow-Origin", "null")],
    );
    assert!(
        may_read(&asking, Credentials::SameOrigin, &answer).is_ok(),
        "the literal match is what the specification says"
    );
    // But it is never enough for credentials, which is the part that matters.
    assert!(may_read(&asking, Credentials::Include, &answer).is_err());
}

// --- What a page may see of a response it is allowed to read -----------------

/// Being allowed to read the body is not being allowed to read everything. A
/// cross-origin `Set-Cookie` is honoured by the browser and invisible to the
/// page — which is what stops a page reading a session token it was handed but
/// never given.
#[test]
fn a_page_cannot_read_a_set_cookie_it_was_never_given() {
    let asking = asked_by("https://app.example/", "https://api.example/me");
    let answer = answered(
        "https://api.example/me",
        &[
            ("Access-Control-Allow-Origin", "https://app.example"),
            ("Set-Cookie", "session=a-real-token"),
            ("Content-Type", "application/json"),
            ("X-Internal-Id", "42"),
        ],
    );
    let seen = readable(&asking, &answer);
    assert_eq!(seen.get("Content-Type"), Some("application/json"));
    assert_eq!(seen.get("Set-Cookie"), None, "a session token was readable");
    assert_eq!(
        seen.get("X-Internal-Id"),
        None,
        "a header the server did not expose was readable"
    );
}

#[test]
fn a_server_can_name_the_extra_headers_a_page_may_read() {
    let asking = asked_by("https://app.example/", "https://api.example/me");
    let answer = answered(
        "https://api.example/me",
        &[
            ("Access-Control-Allow-Origin", "https://app.example"),
            (
                "Access-Control-Expose-Headers",
                "X-Internal-Id, X-Rate-Limit",
            ),
            ("X-Internal-Id", "42"),
            ("X-Secret", "no"),
        ],
    );
    let seen = readable(&asking, &answer);
    assert_eq!(seen.get("X-Internal-Id"), Some("42"));
    assert_eq!(seen.get("X-Secret"), None);
}

/// Even a wildcard exposure does not reach `Set-Cookie`. It is not the page's
/// to read under any arrangement.
#[test]
fn a_wildcard_exposure_still_does_not_reach_set_cookie() {
    let asking = asked_by("https://app.example/", "https://api.example/me");
    let answer = answered(
        "https://api.example/me",
        &[
            ("Access-Control-Allow-Origin", "https://app.example"),
            ("Access-Control-Expose-Headers", "*"),
            ("Set-Cookie", "session=a-real-token"),
            ("X-Anything", "yes"),
        ],
    );
    let seen = readable(&asking, &answer);
    assert_eq!(seen.get("X-Anything"), Some("yes"));
    assert_eq!(seen.get("Set-Cookie"), None);
}

/// Its own page, all of it. The policy is about other people's answers.
#[test]
fn a_page_reads_everything_of_its_own_origins_answer() {
    let asking = asked_by("https://app.example/", "https://app.example/me");
    let answer = answered(
        "https://app.example/me",
        &[("Set-Cookie", "session=fine"), ("X-Internal-Id", "42")],
    );
    let seen = readable(&asking, &answer);
    assert_eq!(seen.get("X-Internal-Id"), Some("42"));
    assert_eq!(seen.get("Set-Cookie"), Some("session=fine"));
}

// --- Asking first ------------------------------------------------------------

/// The rule is not "is it dangerous", it is "could a form have done this
/// already". Asking first about something a form could already do would make
/// the web slower and protect nothing.
#[test]
fn what_a_form_could_already_have_sent_is_not_asked_about_first() {
    for (method, header) in [
        ("GET", None),
        ("HEAD", None),
        (
            "POST",
            Some(("Content-Type", "application/x-www-form-urlencoded")),
        ),
        ("POST", Some(("Content-Type", "text/plain"))),
        ("GET", Some(("Accept", "text/html"))),
    ] {
        let mut request = asked_by("https://app.example/", "https://api.example/thing");
        request.method = method.to_owned();
        if let Some((name, value)) = header {
            request.headers.add(name, value);
        }
        assert!(
            !needs_asking_first(&request),
            "{method} with {header:?} was preflighted, and a form could have sent it"
        );
    }
}

/// A `DELETE` that arrived and was then refused is a `DELETE` that happened.
#[test]
fn a_method_a_form_cannot_send_is_asked_about_before_it_is_sent() {
    for method in ["DELETE", "PUT", "PATCH"] {
        let mut request = asked_by("https://app.example/", "https://api.example/thing");
        request.method = method.to_owned();
        assert!(
            needs_asking_first(&request),
            "{method} was sent without asking, and by then it had happened"
        );
    }
}

#[test]
fn a_header_a_form_cannot_set_is_asked_about_first() {
    let mut request = asked_by("https://app.example/", "https://api.example/thing");
    request.headers.add("X-Custom", "1");
    assert!(needs_asking_first(&request));

    let mut json = asked_by("https://app.example/", "https://api.example/thing");
    json.method = "POST".to_owned();
    json.headers.add("Content-Type", "application/json");
    assert!(
        needs_asking_first(&json),
        "a JSON body is not something a form could have posted"
    );
}

/// The question "may I do this" must not itself do anything on somebody's
/// behalf.
#[test]
fn the_question_carries_no_credentials_of_its_own() {
    let mut request = asked_by("https://app.example/", "https://api.example/thing");
    request.method = "DELETE".to_owned();
    request.headers.add("Authorization", "Bearer a-real-token");
    request.headers.add("Cookie", "session=abc");

    let question = asking_first(&request);
    assert_eq!(question.method, "OPTIONS");
    assert_eq!(question.headers.get("Authorization"), None);
    assert_eq!(question.headers.get("Cookie"), None);
    assert_eq!(
        question.headers.get("Access-Control-Request-Method"),
        Some("DELETE")
    );
    assert_eq!(
        question.headers.get("Access-Control-Request-Headers"),
        Some("authorization"),
        "the header it wanted to send was not named, or one the browser sets was"
    );
}

/// A `Cookie` is set by the browser, never by the page. Counting it as an
/// author header gets two things wrong at once: every credentialled request
/// would be preflighted, and the preflight would tell the server the page had
/// asked for something it cannot ask for.
#[test]
fn a_cookie_the_browser_added_does_not_force_a_preflight() {
    let mut request = asked_by("https://app.example/", "https://api.example/thing");
    request.headers.add("Cookie", "session=abc");
    request.headers.add("Accept", "text/html");
    assert!(
        !needs_asking_first(&request),
        "a plain GET was preflighted because the browser had attached a cookie"
    );
}

/// A server that answers the preflight without allowing the method has not
/// allowed the method, however friendly the rest of its answer is.
#[test]
fn a_preflight_that_does_not_allow_the_method_stops_the_request() {
    let mut request = asked_by("https://app.example/", "https://api.example/thing");
    request.method = "DELETE".to_owned();
    let answer = answered(
        "https://api.example/thing",
        &[
            ("Access-Control-Allow-Origin", "https://app.example"),
            ("Access-Control-Allow-Methods", "GET, POST"),
        ],
    );
    let refused = asking_first_allowed(&request, Credentials::Omit, &answer);
    assert!(refused.is_err(), "a DELETE went out unallowed");
    let why = refused.err().map(|why| why.to_string()).unwrap_or_default();
    assert!(why.contains("DELETE"), "{why:?}");
}

/// `*` is written by people who mean "my public API", and a credential is
/// never that. So a wildcard never covers `Authorization` — the server has to
/// name it.
#[test]
fn a_wildcard_on_allowed_headers_never_covers_authorization() {
    let mut request = asked_by("https://app.example/", "https://api.example/thing");
    request.method = "DELETE".to_owned();
    request.headers.add("Authorization", "Bearer a-real-token");

    let wildcard = answered(
        "https://api.example/thing",
        &[
            ("Access-Control-Allow-Origin", "https://app.example"),
            ("Access-Control-Allow-Methods", "DELETE"),
            ("Access-Control-Allow-Headers", "*"),
        ],
    );
    assert!(
        asking_first_allowed(&request, Credentials::Omit, &wildcard).is_err(),
        "a wildcard was read as covering Authorization"
    );

    let named = answered(
        "https://api.example/thing",
        &[
            ("Access-Control-Allow-Origin", "https://app.example"),
            ("Access-Control-Allow-Methods", "DELETE"),
            ("Access-Control-Allow-Headers", "Authorization"),
        ],
    );
    assert!(asking_first_allowed(&request, Credentials::Omit, &named).is_ok());
}

/// A preflight is itself a cross-origin read, and a server that does not permit
/// the origin has not permitted anything.
#[test]
fn a_preflight_answer_that_does_not_permit_the_origin_stops_everything() {
    let mut request = asked_by("https://evil.example/", "https://api.example/thing");
    request.method = "DELETE".to_owned();
    let answer = answered(
        "https://api.example/thing",
        &[("Access-Control-Allow-Methods", "DELETE")],
    );
    assert!(asking_first_allowed(&request, Credentials::Omit, &answer).is_err());
}

// --- The response a page may hold but not look at ----------------------------

/// This is the policy working, not failing: an image from another site draws
/// and a script runs, and neither hands the page anything readable.
#[test]
fn an_opaque_response_says_nothing_at_all_including_whether_it_worked() {
    let answer = answered(
        "https://cdn.example/logo.png",
        &[("Content-Type", "image/png"), ("X-Internal-Id", "42")],
    );
    let opaque = cors::made_opaque(&answer);
    assert_eq!(opaque.status, Status(0), "the status leaked");
    assert!(opaque.headers.is_empty());
    assert!(opaque.body.is_empty());
    assert_eq!(
        opaque.url, answer.url,
        "the page still knows what it asked for, which it always did"
    );
}
