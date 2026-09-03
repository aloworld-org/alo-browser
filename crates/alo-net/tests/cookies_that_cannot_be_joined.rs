/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ADR 0007, as assertions.
//!
//! The central promise is that a cookie set by one site inside another cannot
//! be seen from a third — so the first test in this file is the one that would
//! catch the whole decision being quietly undone, and the rest are the rules
//! that hold it up.

use alo_net::cookie::{Cookie, Partition, SameSite};
use alo_net::jar::{How, Jar};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const NOW: u64 = 1_700_000_000;

fn now() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(NOW)
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

fn inside(site: &str) -> Partition {
    Partition::of(&url(site))
}

/// Parse a `Set-Cookie` sent by `from`, seen inside `within`.
fn set(header: &str, from: &str, within: &str) -> Result<Cookie, String> {
    Cookie::parse(header, &url(from), &inside(within)).map_err(|why| why.why)
}

fn sent_to(jar: &Jar, target: &str, within: &str, how: How) -> String {
    jar.header_for(&url(target), &inside(within), how, now())
        .unwrap_or_default()
}

// --- The promise -------------------------------------------------------------

/// The whole of ADR 0007 in one test. One embedded site, two top-level sites,
/// and no way to tell that the person is the same person.
#[test]
fn one_embedded_site_cannot_join_a_person_across_two_top_level_sites() {
    let mut jar = Jar::new();

    let on_news = set(
        "id=aaa; Secure; SameSite=None",
        "https://ads.example/p",
        "https://news.example/",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    jar.keep(on_news, now());

    // Inside a different top-level site, the same embedded server sees nothing.
    assert_eq!(
        sent_to(
            &jar,
            "https://ads.example/p",
            "https://shop.example/",
            How::Embedded
        ),
        "",
        "an embedded site was handed the identifier it set on another site"
    );

    // It may set its own there, and the two do not meet.
    let on_shop = set(
        "id=bbb; Secure; SameSite=None",
        "https://ads.example/p",
        "https://shop.example/",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    jar.keep(on_shop, now());

    assert_eq!(
        sent_to(
            &jar,
            "https://ads.example/p",
            "https://news.example/",
            How::Embedded
        ),
        "id=aaa"
    );
    assert_eq!(
        sent_to(
            &jar,
            "https://ads.example/p",
            "https://shop.example/",
            How::Embedded
        ),
        "id=bbb"
    );
    assert_eq!(jar.len(), 2, "the two partitions collapsed into one cookie");
}

/// Clearing a site's data has to mean everything set *inside* it, not only what
/// it set itself. Unpartitioned, that second set was unreachable.
#[test]
fn forgetting_a_site_forgets_what_others_set_inside_it() {
    let mut jar = Jar::new();
    for (from, within) in [
        ("https://ads.example/p", "https://news.example/"),
        ("https://news.example/", "https://news.example/"),
        ("https://ads.example/p", "https://shop.example/"),
    ] {
        let cookie =
            set("id=x; Secure; SameSite=None", from, within).unwrap_or_else(|why| panic!("{why}"));
        jar.keep(cookie, now());
    }
    assert_eq!(jar.len(), 3);
    jar.forget_site(&inside("https://news.example/"));
    assert_eq!(jar.len(), 1, "something set inside news.example survived");
    assert_eq!(
        sent_to(
            &jar,
            "https://ads.example/p",
            "https://shop.example/",
            How::Embedded
        ),
        "id=x"
    );
}

// --- SameSite, which is where a class of CSRF stops existing ------------------

/// The default when a site does not say. The historical default was `None`; a
/// cookie with no `SameSite` is one whose author did not think about cross-site
/// use, and the safe reading of that is not "send it everywhere".
#[test]
fn a_cookie_that_says_nothing_is_lax_rather_than_none() {
    let cookie = set(
        "session=abc",
        "https://bank.example/",
        "https://bank.example/",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(cookie.same_site, SameSite::Lax);
}

/// The middle case is the one that matters: a `Lax` cookie goes on a
/// **navigation** from another site — clicking a link to your bank — but not on
/// anything a page embedded, which is what an attacker's form post would be.
#[test]
fn a_lax_cookie_survives_a_link_and_not_an_embedded_request() {
    let mut jar = Jar::new();
    let cookie = set(
        "session=abc",
        "https://bank.example/",
        "https://bank.example/",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    jar.keep(cookie, now());

    assert_eq!(
        sent_to(
            &jar,
            "https://bank.example/",
            "https://bank.example/",
            How::Embedded
        ),
        "session=abc",
        "its own site could not see its own cookie"
    );
    assert_eq!(
        sent_to(
            &jar,
            "https://bank.example/",
            "https://evil.example/",
            How::Embedded
        ),
        "",
        "an embedded cross-site request carried the session — that is the CSRF"
    );
    // Note the partition: after a navigation, the bank *is* the top-level site.
    assert_eq!(
        sent_to(
            &jar,
            "https://bank.example/",
            "https://bank.example/",
            How::Navigated
        ),
        "session=abc"
    );
}

#[test]
fn a_strict_cookie_does_not_even_survive_a_link() {
    let mut jar = Jar::new();
    let cookie = set(
        "session=abc; SameSite=Strict",
        "https://bank.example/",
        "https://evil.example/",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    jar.keep(cookie, now());
    assert_eq!(
        sent_to(
            &jar,
            "https://bank.example/",
            "https://evil.example/",
            How::Navigated
        ),
        ""
    );
}

/// A cross-site cookie sent in the clear is one any network can read and replay.
#[test]
fn same_site_none_without_secure_is_refused() {
    let refused = set(
        "id=x; SameSite=None",
        "https://ads.example/",
        "https://news.example/",
    );
    let why = match refused {
        Ok(cookie) => panic!("an insecure cross-site cookie was accepted: {cookie:?}"),
        Err(why) => why,
    };
    assert!(
        why.contains("secure"),
        "the refusal should say why: {why:?}"
    );
}

/// A `SameSite` nobody can read is a site that did not successfully say
/// anything, so it gets the default rather than the permissive reading.
#[test]
fn a_samesite_nobody_can_read_is_the_default() {
    let cookie = set(
        "a=b; SameSite=Sometimes",
        "https://x.example/",
        "https://x.example/",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(cookie.same_site, SameSite::Lax);
}

// --- The prefixes, enforced rather than parsed -------------------------------

/// The whole value of a prefix is that a server reading the name back can trust
/// what it implies. Storing one that does not qualify removes that value
/// without telling anybody.
#[test]
fn a_host_prefixed_cookie_that_does_not_qualify_is_rejected_not_relaxed() {
    for (header, what) in [
        ("__Host-a=b", "not Secure"),
        ("__Host-a=b; Secure; Domain=example.com", "has a Domain"),
        ("__Host-a=b; Secure; Path=/admin", "Path is not /"),
    ] {
        let refused = set(header, "https://example.com/", "https://example.com/");
        assert!(
            refused.is_err(),
            "a __Host- cookie that {what} was accepted: {refused:?}"
        );
    }
    // And the one that does qualify.
    let good = set(
        "__Host-a=b; Secure; Path=/",
        "https://example.com/",
        "https://example.com/",
    );
    assert!(good.is_ok(), "a valid __Host- cookie was refused: {good:?}");
}

#[test]
fn a_secure_prefixed_cookie_without_secure_is_rejected() {
    assert!(
        set(
            "__Secure-a=b",
            "https://example.com/",
            "https://example.com/"
        )
        .is_err()
    );
    assert!(
        set(
            "__Secure-a=b; Secure",
            "https://example.com/",
            "https://example.com/"
        )
        .is_ok()
    );
}

// --- Domain and path ---------------------------------------------------------

/// The dot matters. A comparison by suffix alone says `evil-example.com` is
/// covered by `example.com`.
#[test]
fn a_domain_that_merely_ends_the_same_way_is_not_covered() {
    assert!(alo_net::cookie::covers("example.com", "www.example.com"));
    assert!(alo_net::cookie::covers("example.com", "example.com"));
    assert!(
        !alo_net::cookie::covers("example.com", "evil-example.com"),
        "a lookalike host was treated as a subdomain"
    );
}

#[test]
fn a_cookie_for_a_domain_the_page_is_not_part_of_is_refused() {
    assert!(
        set(
            "a=b; Domain=other.example",
            "https://example.com/",
            "https://example.com/"
        )
        .is_err()
    );
    assert!(
        set(
            "a=b; Domain=com",
            "https://example.com/",
            "https://example.com/"
        )
        .is_err()
    );
    assert!(
        set(
            "a=b; Domain=example.com",
            "https://www.example.com/",
            "https://www.example.com/"
        )
        .is_ok()
    );
}

/// `Domain=com` is refused by counting labels; `Domain=co.uk` is not, and the
/// cookie it would set is one for every school, council and company in the
/// country. Queue item 156 rented the list that knows the difference.
#[test]
fn a_cookie_for_a_public_suffix_with_a_dot_in_it_is_refused() {
    for (header, from) in [
        ("a=b; Domain=co.uk", "https://www.bbc.co.uk/"),
        ("a=b; Domain=github.io", "https://alo.github.io/"),
        ("a=b; Domain=com.au", "https://shop.example.com.au/"),
    ] {
        assert!(
            set(header, from, from).is_err(),
            "{from} set a cookie for a whole public suffix with {header}"
        );
    }
    // And the site itself is still allowed, which is what makes the refusals
    // above a boundary rather than a ban.
    assert!(
        set(
            "a=b; Domain=bbc.co.uk",
            "https://www.bbc.co.uk/",
            "https://www.bbc.co.uk/"
        )
        .is_ok()
    );
    assert!(
        set(
            "a=b; Domain=example.com.au",
            "https://shop.example.com.au/",
            "https://shop.example.com.au/"
        )
        .is_ok()
    );
}

/// A page whose whole host is a public suffix is asking for the cookie it would
/// have had anyway. `localhost` is the one anybody meets, and refusing it would
/// be refusing something the page could do nothing about — so it is accepted and
/// kept host-only rather than covering everything under the suffix.
#[test]
fn a_page_at_a_host_that_is_a_suffix_gets_a_host_only_cookie() {
    let cookie = set(
        "a=b; Domain=localhost",
        "http://localhost/",
        "http://localhost/",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(cookie.domain, "localhost");
    assert!(
        !cookie.covers_subdomains,
        "a cookie at a public suffix covered everything under it"
    );
}

/// `covers` reads a host as a name, and an address is not one: `127.0.0.1` ends
/// with `.0.1` the way `www.example.com` ends with `.example.com`. A cookie for
/// `0.1` would be shared by every machine on an address ending that way.
#[test]
fn a_page_at_an_address_cannot_set_a_cookie_for_part_of_the_address() {
    assert!(
        set("a=b; Domain=0.1", "http://127.0.0.1/", "http://127.0.0.1/").is_err(),
        "an address was read as a name and gave up part of itself"
    );
    let itself = set(
        "a=b; Domain=127.0.0.1",
        "http://127.0.0.1/",
        "http://127.0.0.1/",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(itself.domain, "127.0.0.1");
    assert!(!itself.covers_subdomains, "an address grew subdomains");
}

// --- Where the partition's boundary is -------------------------------------------

/// The other half of item 156, and the one a person notices: two subdomains of
/// one organisation are one top-level site, so an embedded service keeps what it
/// was given when the person moves between them. Before the list, `www.` and the
/// bare name were two sites and a sign-in did not survive the difference.
#[test]
fn two_subdomains_of_one_site_are_one_partition() {
    let mut jar = Jar::new();
    let set_on_www = set(
        "id=aaa; Secure; SameSite=None",
        "https://ads.example/p",
        "https://www.news.example/",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    jar.keep(set_on_www, now());

    assert_eq!(
        sent_to(
            &jar,
            "https://ads.example/p",
            "https://shop.news.example/",
            How::Embedded
        ),
        "id=aaa",
        "one organisation's two subdomains were two sites"
    );
}

/// And the boundary the same change must not lose: two organisations under one
/// public suffix are two sites, which no comparison of host strings could say.
#[test]
fn two_organisations_under_one_suffix_are_two_partitions() {
    let mut jar = Jar::new();
    let on_the_bbc = set(
        "id=aaa; Secure; SameSite=None",
        "https://ads.example/p",
        "https://www.bbc.co.uk/",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    jar.keep(on_the_bbc, now());

    assert_eq!(
        sent_to(
            &jar,
            "https://ads.example/p",
            "https://www.gov.co.uk/",
            How::Embedded
        ),
        "",
        "a suffix two organisations share was read as one site"
    );
}

/// Without a `Domain`, a cookie is for exactly the host that set it.
#[test]
fn a_cookie_with_no_domain_does_not_reach_a_subdomain() {
    let mut jar = Jar::new();
    let cookie = set("a=b", "https://example.com/", "https://example.com/")
        .unwrap_or_else(|why| panic!("{why}"));
    jar.keep(cookie, now());
    assert_eq!(
        sent_to(
            &jar,
            "https://example.com/",
            "https://example.com/",
            How::Embedded
        ),
        "a=b"
    );
    assert_eq!(
        sent_to(
            &jar,
            "https://www.example.com/",
            "https://example.com/",
            How::Embedded
        ),
        "",
        "a host-only cookie reached a subdomain"
    );
}

#[test]
fn a_path_is_a_prefix_at_a_boundary_rather_than_a_string_prefix() {
    let mut jar = Jar::new();
    let cookie = set(
        "a=b; Path=/admin",
        "https://example.com/admin/x",
        "https://example.com/",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    jar.keep(cookie, now());
    assert_eq!(
        sent_to(
            &jar,
            "https://example.com/admin/x",
            "https://example.com/",
            How::Embedded
        ),
        "a=b"
    );
    assert_eq!(
        sent_to(
            &jar,
            "https://example.com/administrator",
            "https://example.com/",
            How::Embedded
        ),
        "",
        "a path was matched as a bare string prefix"
    );
}

/// Longest path first, which is what servers expect when two cookies share a
/// name at different depths.
#[test]
fn the_deeper_path_is_sent_first() {
    let mut jar = Jar::new();
    for header in ["a=shallow; Path=/", "a=deep; Path=/one/two"] {
        let cookie = set(
            header,
            "https://example.com/one/two/three",
            "https://example.com/",
        )
        .unwrap_or_else(|why| panic!("{why}"));
        jar.keep(cookie, now());
    }
    assert_eq!(
        sent_to(
            &jar,
            "https://example.com/one/two/three",
            "https://example.com/",
            How::Embedded
        ),
        "a=deep; a=shallow"
    );
}

// --- Secure, expiry and deletion ---------------------------------------------

#[test]
fn a_secure_cookie_never_goes_out_in_the_clear() {
    let mut jar = Jar::new();
    let cookie = set(
        "a=b; Secure",
        "https://example.com/",
        "https://example.com/",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    jar.keep(cookie, now());
    assert_eq!(
        sent_to(
            &jar,
            "https://example.com/",
            "https://example.com/",
            How::Embedded
        ),
        "a=b"
    );
    assert_eq!(
        sent_to(
            &jar,
            "http://example.com/",
            "http://example.com/",
            How::Embedded
        ),
        "",
        "a Secure cookie was sent over http"
    );
}

/// Setting it again with an expiry in the past is the only way a server can
/// delete one, so storing an expired cookie and deleting are one operation.
#[test]
fn a_cookie_is_deleted_by_being_set_again_in_the_past() {
    let mut jar = Jar::new();
    let cookie = set("a=b", "https://example.com/", "https://example.com/")
        .unwrap_or_else(|why| panic!("{why}"));
    jar.keep(cookie, now());
    assert_eq!(jar.len(), 1);

    let gone = set(
        "a=; Max-Age=0",
        "https://example.com/",
        "https://example.com/",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    jar.keep(gone, now());
    assert_eq!(jar.len(), 0, "a deletion did not delete");
}

/// A cookie set to expire in the year 9999 is not a preference, it is a
/// permanent identifier.
#[test]
fn an_expiry_further_off_than_the_cap_is_capped() {
    let cookie = set(
        "a=b; Expires=Fri, 31 Dec 9999 23:59:59 GMT",
        "https://example.com/",
        "https://example.com/",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    let limit = SystemTime::now() + alo_net::cookie::LONGEST_LIFE + Duration::from_secs(60);
    assert!(
        cookie.expires.is_some_and(|at| at <= limit),
        "an expiry in the year 9999 was kept as written"
    );
}

#[test]
fn a_session_cookie_does_not_survive_the_session() {
    let mut jar = Jar::new();
    for header in ["stays=1; Max-Age=3600", "goes=1"] {
        let cookie = set(header, "https://example.com/", "https://example.com/")
            .unwrap_or_else(|why| panic!("{why}"));
        jar.keep(cookie, now());
    }
    jar.end_the_session();
    assert_eq!(
        sent_to(
            &jar,
            "https://example.com/",
            "https://example.com/",
            How::Embedded
        ),
        "stays=1"
    );
}

// --- What is refused outright ------------------------------------------------

#[test]
fn a_nameless_or_oversized_cookie_is_refused() {
    assert!(
        set(
            "just-a-value",
            "https://example.com/",
            "https://example.com/"
        )
        .is_err()
    );
    assert!(set("=b", "https://example.com/", "https://example.com/").is_err());
    let huge = format!("a={}", "x".repeat(alo_net::cookie::LARGEST_COOKIE));
    assert!(set(&huge, "https://example.com/", "https://example.com/").is_err());
}

/// A bound, because without one a site chooses how much memory a browser spends.
#[test]
fn one_site_cannot_fill_the_jar() {
    let mut jar = Jar::new();
    for n in 0..(alo_net::jar::MOST_PER_SITE + 50) {
        let cookie = set(
            &format!("c{n}=v; Max-Age={}", 100 + n),
            "https://example.com/",
            "https://example.com/",
        )
        .unwrap_or_else(|why| panic!("{why}"));
        jar.keep(cookie, now());
    }
    assert_eq!(jar.len(), alo_net::jar::MOST_PER_SITE);
}

/// One site filling its own jar must not evict another site's cookies.
#[test]
fn a_site_filling_its_jar_does_not_evict_another_site() {
    let mut jar = Jar::new();
    let theirs = set(
        "keep=me; Max-Age=3600",
        "https://other.example/",
        "https://other.example/",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    jar.keep(theirs, now());

    for n in 0..(alo_net::jar::MOST_PER_SITE + 20) {
        let cookie = set(
            &format!("c{n}=v; Max-Age={}", 100 + n),
            "https://example.com/",
            "https://example.com/",
        )
        .unwrap_or_else(|why| panic!("{why}"));
        jar.keep(cookie, now());
    }
    assert_eq!(
        sent_to(
            &jar,
            "https://other.example/",
            "https://other.example/",
            How::Embedded
        ),
        "keep=me",
        "one site's flood evicted another site's cookie"
    );
}
