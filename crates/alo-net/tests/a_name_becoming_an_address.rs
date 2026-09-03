//! ADR 0008's two code rules, and the one with an attacker behind it first.
//!
//! Nothing here reaches the network. The rebinding rule is a function of an
//! address and who asked, so it is tested as one; the lookups that do run go to
//! `localhost` and to a name that cannot exist.

use alo_net::resolve::{Reach, Resolver, Unresolved, is_public, reach_for};
use alo_url::Origin;
use std::net::IpAddr;

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

fn address(text: &str) -> IpAddr {
    text.parse().unwrap_or(IpAddr::from([0, 0, 0, 0]))
}

// --- The rule with an attacker behind it -------------------------------------

/// Every range that is not the public internet. A page on the web that could
/// reach one of these would be a way into the network the browser is sitting
/// on — a router's admin page, a printer, a database that trusts its LAN.
#[test]
fn nothing_that_is_not_the_public_internet_counts_as_public() {
    for text in [
        "127.0.0.1",       // loopback
        "127.9.9.9",       // all of 127/8, not only .0.1
        "0.0.0.0",         // unspecified
        "10.0.0.1",        // private
        "172.16.5.4",      // private
        "172.31.255.255",  // the far end of 172.16/12
        "192.168.1.1",     // private, and every home router
        "169.254.169.254", // link-local, and every cloud metadata service
        "100.64.0.1",      // carrier-grade NAT
        "198.18.0.1",      // benchmarking
        "240.0.0.1",       // reserved
        "255.255.255.255", // broadcast
        "::1",             // v6 loopback
        "::",              // v6 unspecified
        "fc00::1",         // v6 unique local
        "fd12:3456::1",    // also fc00::/7
        "fe80::1",         // v6 link local
    ] {
        assert!(
            !is_public(address(text)),
            "{text} was treated as a public address"
        );
    }
}

/// A v4 address wearing a v6 hat. Without this, `::ffff:127.0.0.1` walks
/// straight past every check — it is not v6 loopback, and nothing looks at the
/// v4 address inside it.
#[test]
fn a_v4_address_in_v6_clothing_is_judged_as_the_v4_address_it_is() {
    for text in ["::ffff:127.0.0.1", "::ffff:10.0.0.1", "::ffff:192.168.0.1"] {
        assert!(
            !is_public(address(text)),
            "{text} slipped past as a v6 address"
        );
    }
    assert!(is_public(address("::ffff:93.184.216.34")));
}

#[test]
fn ordinary_addresses_are_public() {
    for text in [
        "93.184.216.34",
        "8.8.8.8",
        "1.1.1.1",
        "2606:2800:220:1:248:1893:25c8:1946",
    ] {
        assert!(is_public(address(text)), "{text} was refused");
    }
    // 172.32 is outside 172.16/12, and a range check written as a prefix
    // comparison gets it wrong.
    assert!(
        is_public(address("172.32.0.1")),
        "the 172.16/12 range was over-wide"
    );
    assert!(
        is_public(address("100.128.0.1")),
        "the 100.64/10 range was over-wide"
    );
}

// --- Who asked decides how far it may reach ----------------------------------

/// A person typing an intranet name should reach it; a public web page causing
/// a request to `192.168.1.1` should not. The two are indistinguishable if you
/// only look at where it resolved.
#[test]
fn how_far_a_request_may_reach_depends_on_who_asked() {
    assert_eq!(
        reach_for(None),
        Reach::Anywhere,
        "the person asked directly"
    );

    let public_page = Origin::of(&url("https://example.com/"));
    assert_eq!(reach_for(Some(&public_page)), Reach::PublicOnly);

    // A page that is itself local is already inside whatever it would reach.
    for local in [
        "http://localhost:8080/",
        "http://127.0.0.1/",
        "http://192.168.1.5/",
    ] {
        let page = Origin::of(&url(local));
        assert_eq!(
            reach_for(Some(&page)),
            Reach::Anywhere,
            "{local} was treated as though it were on the public web"
        );
    }
}

/// `file:` is an opaque origin, and "we cannot tell who this is" reads the
/// restrictive way — so a local file cannot become a way into a network either.
#[test]
fn an_origin_we_cannot_judge_is_held_to_the_stricter_rule() {
    let opaque = Origin::of(&url("file:///Users/somebody/page.html"));
    assert_eq!(reach_for(Some(&opaque)), Reach::PublicOnly);
}

// --- Resolution itself -------------------------------------------------------

#[test]
fn localhost_resolves_and_is_reachable_when_the_person_asked() {
    let mut resolver = Resolver::new();
    let found = resolver
        .resolve("localhost", 80, Reach::Anywhere)
        .unwrap_or_else(|why| panic!("localhost should resolve on any machine: {why}"));
    assert!(!found.is_empty());
    assert!(found.iter().all(|address| address.port() == 80));
}

/// The rebinding refusal, end to end through the resolver: the same name, the
/// same lookup, refused only because of who was asking.
#[test]
fn a_name_that_resolves_to_loopback_is_refused_for_a_public_page() {
    let mut resolver = Resolver::new();
    let refused = resolver.resolve("localhost", 80, Reach::PublicOnly);
    let Err(Unresolved::RefusedTheAnswer { host, address }) = refused else {
        panic!("a public page was allowed to reach loopback: {refused:?}");
    };
    assert_eq!(host, "localhost");
    assert!(!is_public(address));
}

#[test]
fn a_name_that_does_not_exist_says_so_rather_than_hanging() {
    let mut resolver = Resolver::new();
    let refused = resolver.resolve("nothing.invalid", 80, Reach::Anywhere);
    assert!(
        matches!(refused, Err(Unresolved::NoSuchName { .. })),
        "a name in the reserved .invalid domain resolved to something: {refused:?}"
    );
}

/// The two refusals are different things to tell somebody: one is the shape of
/// a typo, the other is the shape of an attack.
#[test]
fn a_refusal_says_which_kind_it_is() {
    let mut resolver = Resolver::new();
    let rebinding = resolver
        .resolve("localhost", 80, Reach::PublicOnly)
        .err()
        .map(|why| why.to_string())
        .unwrap_or_default();
    assert!(
        rebinding.contains("private address"),
        "the rebinding refusal should say what happened: {rebinding:?}"
    );
    let missing = resolver
        .resolve("nothing.invalid", 80, Reach::Anywhere)
        .err()
        .map(|why| why.to_string())
        .unwrap_or_default();
    assert!(missing.contains("could not find"), "{missing:?}");
}

/// Thirty requests to one host should not be thirty lookups.
#[test]
fn a_name_looked_up_twice_is_looked_up_once() {
    let mut resolver = Resolver::new();
    for _ in 0..5 {
        let _ = resolver.resolve("localhost", 443, Reach::Anywhere);
    }
    let (asked, reused) = resolver.counts();
    assert_eq!(asked, 1, "the name was looked up more than once");
    assert_eq!(reused, 4);
}

/// A cached answer must not carry a permission it was granted the first time.
/// The lookup is shared; the rule is applied afterwards, every time.
#[test]
fn a_cached_answer_does_not_smuggle_the_reach_it_was_first_allowed() {
    let mut resolver = Resolver::new();
    assert!(resolver.resolve("localhost", 80, Reach::Anywhere).is_ok());
    assert!(
        resolver
            .resolve("localhost", 80, Reach::PublicOnly)
            .is_err(),
        "the second lookup reused the first one's permission"
    );
    assert_eq!(resolver.counts().0, 1, "it was resolved twice");
}

/// Changing network means every answer is suspect.
#[test]
fn forgetting_makes_the_next_lookup_a_real_one() {
    let mut resolver = Resolver::new();
    let _ = resolver.resolve("localhost", 80, Reach::Anywhere);
    resolver.forget();
    let _ = resolver.resolve("localhost", 80, Reach::Anywhere);
    assert_eq!(resolver.counts().0, 2);
}
