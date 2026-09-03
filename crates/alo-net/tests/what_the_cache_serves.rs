//! A table of responses and clocks, and what the cache does with each.
//!
//! `ROADMAP.md` on this item: *"subtly wrong here is invisible for months and
//! then serves somebody a stale bank page."* The invisibility is the problem —
//! a cache that is wrong is not wrong when you look at it, it is wrong an hour
//! later. So nothing here reads the clock. Every case names a moment, and the
//! cases that matter most are the ones asserted at two moments either side of
//! an expiry.

use alo_net::cache::{Answer, Cache, asking_whether_it_changed};
use alo_net::{Headers, Purpose, Request, Response, Status};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A fixed moment to measure everything from: 2023-11-14 22:13:20 GMT.
const START: u64 = 1_700_000_000;

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
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

fn asking(target: &str) -> Request {
    Request::get(url(target))
}

/// A response carrying these headers, dated when it was sent.
fn answered(sent_at: u64, headers: &[(&str, &str)]) -> Response {
    let mut carried = Headers::new();
    carried.add("Date", alo_net::httpdate::format(at(sent_at)));
    for (name, value) in headers {
        carried.add(*name, *value);
    }
    Response {
        url: url("https://example.com/a"),
        status: Status(200),
        headers: carried,
        body: b"the stored body".to_vec(),
    }
}

/// Store one response and ask about it at a moment.
fn served(headers: &[(&str, &str)], asked_at: u64) -> Answer {
    let mut cache = Cache::new();
    let request = asking("https://example.com/a");
    let kept = cache.keep(&request, &answered(START, headers), at(START), at(START));
    assert!(kept, "the response was not stored at all: {headers:?}");
    cache.answer(&request, at(asked_at))
}

fn is_hit(answer: &Answer) -> bool {
    matches!(answer, Answer::Stored(_))
}
fn is_revalidate(answer: &Answer) -> bool {
    matches!(answer, Answer::Revalidate { .. })
}

// --- The table ---------------------------------------------------------------

/// Each row is a response, a moment, and what should happen. The pairs either
/// side of an expiry are the point: a cache that is wrong is not wrong when you
/// look at it.
#[test]
fn the_table_of_responses_and_clocks() {
    const ONE_HOUR: &[(&str, &str)] = &[("Cache-Control", "max-age=3600"), ("ETag", "\"v1\"")];
    const NO_VALIDATOR: &[(&str, &str)] = &[("Cache-Control", "max-age=3600")];

    /// What a row is: what it is testing, the response's headers, when it is
    /// asked about, and what should come back.
    type Row = (
        &'static str,
        &'static [(&'static str, &'static str)],
        u64,
        fn(&Answer) -> bool,
    );

    let table: &[Row] = &[
        ("fresh, well inside its hour", ONE_HOUR, START + 60, is_hit),
        (
            "fresh, one second before it expires",
            ONE_HOUR,
            START + 3599,
            is_hit,
        ),
        (
            "stale, one second after",
            ONE_HOUR,
            START + 3601,
            is_revalidate,
        ),
        ("stale by an hour", ONE_HOUR, START + 7200, is_revalidate),
        // The same expiry with nothing to revalidate against is a plain miss:
        // there is no question to ask that is cheaper than asking for all of it.
        (
            "stale with no validator is a miss, not a revalidate",
            NO_VALIDATOR,
            START + 3601,
            |a| matches!(a, Answer::Fetch),
        ),
        // `no-cache` is stored and never served without asking. It is not
        // `no-store`, which would not be here to ask about.
        (
            "no-cache is always revalidated, even one second old",
            &[("Cache-Control", "no-cache"), ("ETag", "\"v1\"")],
            START + 1,
            is_revalidate,
        ),
        // `Expires` is a moment, and only means anything against the `Date` the
        // same response carried.
        (
            "Expires an hour out, before",
            &[
                ("Expires", "Wed, 15 Nov 2023 00:13:20 GMT"),
                ("ETag", "\"v1\""),
            ],
            START + 3599,
            is_hit,
        ),
        (
            "Expires an hour out, after",
            &[
                ("Expires", "Wed, 15 Nov 2023 00:13:20 GMT"),
                ("ETag", "\"v1\""),
            ],
            START + 7201,
            is_revalidate,
        ),
        // An `Expires` in the past is already stale, which is the oldest way of
        // saying "do not cache this".
        (
            "Expires already past",
            &[
                ("Expires", "Thu, 01 Jan 1970 00:00:00 GMT"),
                ("ETag", "\"v1\""),
            ],
            START,
            is_revalidate,
        ),
        // A date nobody can parse must never be read as permission.
        (
            "an Expires nobody can parse is expired",
            &[("Expires", "next Tuesday"), ("ETag", "\"v1\"")],
            START,
            is_revalidate,
        ),
        // `max-age` wins over `Expires` when both are there.
        (
            "max-age beats a contradicting Expires",
            &[
                ("Cache-Control", "max-age=3600"),
                ("Expires", "Thu, 01 Jan 1970 00:00:00 GMT"),
                ("ETag", "\"v1\""),
            ],
            START + 60,
            is_hit,
        ),
    ];

    for (what, headers, asked_at, expected) in table {
        let answer = served(headers, *asked_at);
        assert!(
            expected(&answer),
            "{what}: at {}s the cache said {answer:?}",
            asked_at - START
        );
    }
}

/// The one the roadmap warns about, on its own so it cannot be lost in a table.
#[test]
fn a_response_that_is_right_now_and_wrong_an_hour_later() {
    let headers = [("Cache-Control", "max-age=3600"), ("ETag", "\"v1\"")];
    assert!(
        is_hit(&served(&headers, START + 3599)),
        "it expired a second early"
    );
    assert!(
        !is_hit(&served(&headers, START + 3601)),
        "an expired response was served as though it were fresh"
    );
}

// --- Age, which is not "how long we have had it" -----------------------------

/// A response can arrive already old. A cache that started counting from
/// arrival grants it a second full lifetime, which is how one `max-age=3600`
/// becomes six hours of staleness across a chain of caches.
#[test]
fn a_response_that_arrives_already_old_expires_early() {
    let mut cache = Cache::new();
    let request = asking("https://example.com/a");
    let response = answered(
        START,
        &[
            ("Cache-Control", "max-age=3600"),
            ("Age", "3500"),
            ("ETag", "\"v1\""),
        ],
    );
    assert!(cache.keep(&request, &response, at(START), at(START)));

    assert!(
        is_hit(&cache.answer(&request, at(START + 50))),
        "it had a hundred seconds left and was refused at fifty"
    );
    assert!(
        !is_hit(&cache.answer(&request, at(START + 150))),
        "an Age of 3500 was ignored, so a stale response was served"
    );
}

/// Time in transit is time the response spent ageing. On a slow connection that
/// is seconds, and a five-second response that took two seconds to arrive is
/// fresh for three.
#[test]
fn time_spent_in_transit_counts_against_a_short_lifetime() {
    let mut cache = Cache::new();
    let request = asking("https://example.com/a");
    let response = answered(START, &[("Cache-Control", "max-age=5"), ("ETag", "\"v1\"")]);
    // Asked at START, arrived two seconds later.
    assert!(cache.keep(&request, &response, at(START), at(START + 2)));
    assert!(is_hit(&cache.answer(&request, at(START + 4))));
    assert!(
        !is_hit(&cache.answer(&request, at(START + 6))),
        "the two seconds it spent arriving were not counted"
    );
}

// --- Guessing, when nothing was said -----------------------------------------

/// A tenth of the time since it last changed, capped at a day. Something edited
/// a year ago is unlikely to change in the next hour.
#[test]
fn a_response_with_no_expiry_is_guessed_from_when_it_last_changed() {
    // Changed 1000 seconds before it was sent: a hundred seconds of freshness.
    let changed = alo_net::httpdate::format(at(START - 1000));
    let headers = [("Last-Modified", changed.as_str()), ("ETag", "\"v1\"")];
    assert!(is_hit(&served(&headers, START + 50)));
    assert!(!is_hit(&served(&headers, START + 150)));
}

#[test]
fn the_guess_is_capped_so_nothing_is_fresh_for_a_week() {
    // Changed ten years ago; a tenth of that is a year, and the cap is a day.
    let changed = alo_net::httpdate::format(at(START - 10 * 365 * 86_400));
    let headers = [("Last-Modified", changed.as_str()), ("ETag", "\"v1\"")];
    assert!(is_hit(&served(&headers, START + 23 * 3600)));
    assert!(
        !is_hit(&served(&headers, START + 25 * 3600)),
        "a guess was allowed to outlive the cap"
    );
}

// --- What is never kept ------------------------------------------------------

#[test]
fn no_store_is_not_kept_at_all() {
    let mut cache = Cache::new();
    let request = asking("https://example.com/a");
    assert!(
        !cache.keep(
            &request,
            &answered(START, &[("Cache-Control", "no-store, max-age=3600")]),
            at(START),
            at(START)
        ),
        "a no-store response was written down"
    );
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.answer(&request, at(START)), Answer::Fetch);
}

/// A `POST` is not a thing to reuse, and answering a `GET` from a stored `HEAD`
/// would be a blank page.
#[test]
fn a_post_is_never_kept_and_a_head_is_not_a_get() {
    let mut cache = Cache::new();
    let mut posting = asking("https://example.com/a");
    posting.method = "POST".to_owned();
    assert!(!cache.keep(
        &posting,
        &answered(START, &[("Cache-Control", "max-age=60")]),
        at(START),
        at(START)
    ));

    let mut heading = asking("https://example.com/a");
    heading.method = "HEAD".to_owned();
    assert!(cache.keep(
        &heading,
        &answered(START, &[("Cache-Control", "max-age=60")]),
        at(START),
        at(START)
    ));
    assert_eq!(
        cache.answer(&asking("https://example.com/a"), at(START)),
        Answer::Fetch,
        "a GET was answered out of a stored HEAD"
    );
}

/// A status nothing has a rule for is not kept. "Have not thought about it"
/// must not default to keeping it.
#[test]
fn a_status_with_no_rule_is_not_kept() {
    let mut cache = Cache::new();
    let request = asking("https://example.com/a");
    let mut response = answered(START, &[("Cache-Control", "max-age=3600")]);
    response.status = Status(418);
    assert!(!cache.keep(&request, &response, at(START), at(START)));
}

// --- Vary, which is where one person gets another person's page --------------

#[test]
fn a_response_chosen_by_a_header_is_not_reused_for_a_different_one() {
    let mut cache = Cache::new();
    let mut french = asking("https://example.com/a");
    french.headers.add("Accept-Language", "fr");
    let response = answered(
        START,
        &[
            ("Cache-Control", "max-age=3600"),
            ("Vary", "Accept-Language"),
        ],
    );
    assert!(cache.keep(&french, &response, at(START), at(START)));

    assert!(
        is_hit(&cache.answer(&french, at(START + 60))),
        "the same request missed"
    );

    let mut german = asking("https://example.com/a");
    german.headers.add("Accept-Language", "de");
    assert_eq!(
        cache.answer(&german, at(START + 60)),
        Answer::Fetch,
        "a German reader was served the French page"
    );
}

/// An absent header and an empty one are different, and a server may well
/// answer them differently.
#[test]
fn a_header_that_is_absent_is_not_a_header_that_is_empty() {
    let mut cache = Cache::new();
    let bare = asking("https://example.com/a");
    let response = answered(
        START,
        &[
            ("Cache-Control", "max-age=3600"),
            ("Vary", "Accept-Language"),
        ],
    );
    assert!(cache.keep(&bare, &response, at(START), at(START)));

    let mut empty = asking("https://example.com/a");
    empty.headers.add("Accept-Language", "");
    assert_eq!(cache.answer(&empty, at(START + 60)), Answer::Fetch);
    assert!(is_hit(&cache.answer(&bare, at(START + 60))));
}

/// The server saying it cannot promise this answers any other request. There is
/// no key that would be right, so there is no key.
#[test]
fn vary_star_is_never_stored() {
    let mut cache = Cache::new();
    let request = asking("https://example.com/a");
    assert!(!cache.keep(
        &request,
        &answered(START, &[("Cache-Control", "max-age=3600"), ("Vary", "*")]),
        at(START),
        at(START)
    ));
    assert_eq!(cache.len(), 0);
}

#[test]
fn several_varied_headers_all_have_to_match() {
    let mut cache = Cache::new();
    let mut first = asking("https://example.com/a");
    first.headers.add("Accept-Language", "fr");
    first.headers.add("Accept-Encoding", "br");
    let response = answered(
        START,
        &[
            ("Cache-Control", "max-age=3600"),
            ("Vary", "Accept-Language, Accept-Encoding"),
        ],
    );
    assert!(cache.keep(&first, &response, at(START), at(START)));

    let mut half = asking("https://example.com/a");
    half.headers.add("Accept-Language", "fr");
    half.headers.add("Accept-Encoding", "gzip");
    assert_eq!(cache.answer(&half, at(START + 60)), Answer::Fetch);
}

// --- Revalidation ------------------------------------------------------------

#[test]
fn revalidating_asks_with_both_validators_when_both_exist() {
    let changed = alo_net::httpdate::format(at(START - 100));
    let answer = served(
        &[
            ("Cache-Control", "max-age=1"),
            ("ETag", "\"v1\""),
            ("Last-Modified", changed.as_str()),
        ],
        START + 100,
    );
    let Answer::Revalidate { conditions } = answer else {
        panic!("a stale response with validators should be revalidated: {answer:?}");
    };
    let asking = asking_whether_it_changed(&asking("https://example.com/a"), &conditions);
    assert_eq!(asking.headers.get("If-None-Match"), Some("\"v1\""));
    assert_eq!(
        asking.headers.get("If-Modified-Since"),
        Some(changed.as_str())
    );
}

/// A `304` says "still good, and here is what is different now". The body is
/// kept; the headers are updated — and a `Content-Length` on a `304` describes a
/// body it did not send, so believing it would make the stored body's length a
/// lie.
#[test]
fn a_304_refreshes_the_headers_and_keeps_the_body() {
    let mut cache = Cache::new();
    let request = asking("https://example.com/a");
    assert!(cache.keep(
        &request,
        &answered(START, &[("Cache-Control", "max-age=1"), ("ETag", "\"v1\"")]),
        at(START),
        at(START)
    ));

    let mut not_modified = answered(
        START + 100,
        &[
            ("Cache-Control", "max-age=3600"),
            ("ETag", "\"v1\""),
            ("Content-Length", "0"),
        ],
    );
    not_modified.status = Status(304);
    not_modified.body = Vec::new();

    let refreshed = cache
        .refresh(&request, &not_modified, at(START + 100))
        .unwrap_or_else(|| panic!("there was something stored to refresh"));
    assert_eq!(
        refreshed.body, b"the stored body",
        "the body was thrown away"
    );
    assert_eq!(refreshed.headers.get("Cache-Control"), Some("max-age=3600"));
    assert_ne!(
        refreshed.headers.get("Content-Length"),
        Some("0"),
        "a 304 described a body it did not send and was believed"
    );
    // And it is fresh again, for the new hour rather than the old second.
    assert!(is_hit(&cache.answer(&request, at(START + 200))));
}

#[test]
fn refreshing_something_that_was_never_stored_says_so() {
    let mut cache = Cache::new();
    let mut not_modified = answered(START, &[]);
    not_modified.status = Status(304);
    assert_eq!(
        cache.refresh(
            &asking("https://example.com/gone"),
            &not_modified,
            at(START)
        ),
        None
    );
}

// --- What a request may ask for ----------------------------------------------

/// Somebody pressed reload.
#[test]
fn a_request_that_says_no_cache_is_revalidated_however_fresh_the_answer_is() {
    let mut cache = Cache::new();
    let plain = asking("https://example.com/a");
    assert!(cache.keep(
        &plain,
        &answered(
            START,
            &[("Cache-Control", "max-age=3600"), ("ETag", "\"v1\"")]
        ),
        at(START),
        at(START)
    ));
    assert!(is_hit(&cache.answer(&plain, at(START + 1))));

    let mut reloading = asking("https://example.com/a");
    reloading.headers.add("Cache-Control", "no-cache");
    assert!(is_revalidate(&cache.answer(&reloading, at(START + 1))));
}

/// A request that needs it to stay fresh a while longer than it will.
#[test]
fn min_fresh_refuses_something_about_to_expire() {
    let mut cache = Cache::new();
    let request = asking("https://example.com/a");
    assert!(cache.keep(
        &request,
        &answered(
            START,
            &[("Cache-Control", "max-age=100"), ("ETag", "\"v1\"")]
        ),
        at(START),
        at(START)
    ));

    let mut demanding = asking("https://example.com/a");
    demanding.headers.add("Cache-Control", "min-fresh=60");
    assert!(
        is_hit(&cache.answer(&demanding, at(START + 10))),
        "it had ninety seconds left"
    );
    assert!(
        is_revalidate(&cache.answer(&demanding, at(START + 50))),
        "it had fifty seconds left and sixty were asked for"
    );
}

/// A request willing to take something stale gets it — unless the response said
/// it must not be, which is exactly what `must-revalidate` is for.
#[test]
fn max_stale_is_honoured_except_where_the_server_forbade_it() {
    let willing = |extra: &str| {
        let mut request = asking("https://example.com/a");
        request.headers.add("Cache-Control", extra);
        request
    };

    let mut ordinary = Cache::new();
    assert!(ordinary.keep(
        &asking("https://example.com/a"),
        &answered(
            START,
            &[("Cache-Control", "max-age=10"), ("ETag", "\"v1\"")]
        ),
        at(START),
        at(START)
    ));
    assert!(
        is_hit(&ordinary.answer(&willing("max-stale=100"), at(START + 60))),
        "a caller happy with stale was refused"
    );
    assert!(
        !is_hit(&ordinary.answer(&willing("max-stale=10"), at(START + 60))),
        "fifty seconds stale was served to somebody who allowed ten"
    );

    let mut strict = Cache::new();
    assert!(strict.keep(
        &asking("https://example.com/a"),
        &answered(
            START,
            &[
                ("Cache-Control", "max-age=10, must-revalidate"),
                ("ETag", "\"v1\"")
            ]
        ),
        at(START),
        at(START)
    ));
    assert!(
        !is_hit(&strict.answer(&willing("max-stale=100"), at(START + 60))),
        "must-revalidate was overridden by the request"
    );
}

// --- Bookkeeping -------------------------------------------------------------

#[test]
fn a_write_that_makes_something_a_lie_forgets_it() {
    let mut cache = Cache::new();
    let request = asking("https://example.com/a");
    assert!(cache.keep(
        &request,
        &answered(
            START,
            &[("Cache-Control", "max-age=3600"), ("ETag", "\"v1\"")]
        ),
        at(START),
        at(START)
    ));
    assert!(is_hit(&cache.answer(&request, at(START + 1))));
    cache.forget(&request);
    assert_eq!(cache.answer(&request, at(START + 1)), Answer::Fetch);
}

/// A bound, not a tuning: without one, a page that fetches ten thousand images
/// has made the browser hold them for as long as it runs.
#[test]
fn the_cache_does_not_grow_without_end() {
    let mut cache = Cache::new();
    for n in 0..(alo_net::cache::MOST_KEPT + 50) {
        let request = asking(&format!("https://example.com/{n}"));
        assert!(cache.keep(
            &request,
            &answered(START, &[("Cache-Control", "max-age=3600")]),
            at(START),
            at(START)
        ));
    }
    assert_eq!(cache.len(), alo_net::cache::MOST_KEPT);
    // The oldest went, the newest stayed.
    assert_eq!(
        cache.answer(&asking("https://example.com/0"), at(START)),
        Answer::Fetch
    );
    assert!(is_hit(&cache.answer(
        &asking(&format!(
            "https://example.com/{}",
            alo_net::cache::MOST_KEPT + 49
        )),
        at(START)
    )));
}

#[test]
fn the_counts_say_what_the_cache_actually_did() {
    let mut cache = Cache::new();
    let request = asking("https://example.com/a");
    assert_eq!(cache.answer(&request, at(START)), Answer::Fetch);
    assert!(cache.keep(
        &request,
        &answered(
            START,
            &[("Cache-Control", "max-age=10"), ("ETag", "\"v1\"")]
        ),
        at(START),
        at(START)
    ));
    let _ = cache.answer(&request, at(START + 1));
    let _ = cache.answer(&request, at(START + 100));
    assert_eq!(cache.counts(), (1, 1, 1), "hits, revalidations, misses");
}

/// The purpose a request was made for does not change what may answer it.
#[test]
fn a_stylesheet_and_a_document_share_one_stored_response() {
    let mut cache = Cache::new();
    let document = asking("https://example.com/a").for_purpose(Purpose::Document);
    assert!(cache.keep(
        &document,
        &answered(START, &[("Cache-Control", "max-age=3600")]),
        at(START),
        at(START)
    ));
    let style = asking("https://example.com/a").for_purpose(Purpose::Style);
    assert!(is_hit(&cache.answer(&style, at(START + 1))));
}
