//! URLs, checked against the standard's own examples.
//!
//! The table below is **ours**, and every row is a case from the WHATWG URL
//! Standard's own text or from the behaviour it specifies — the examples in
//! §4.1 and §4.3, the IDNA rules in §3.5, and the "special scheme" table in
//! §1.1. It is written here rather than fetched, for the reason
//! `docs/autonomy/LOOP.md` gives: a test suite that reached the network would
//! be flaky, would fail on an aeroplane, and would let somebody else break our
//! build.
//!
//! It is not the whole of `web-platform-tests`. It is the part a person can
//! read and check by eye, which is the part worth committing.

use alo_url::{Host, Origin, join, parse};
use std::net::{Ipv4Addr, Ipv6Addr};

/// `input` parses, and writes back out as `serialised`.
const ROUND_TRIPS: &[(&str, &str)] = &[
    ("https://example.com", "https://example.com/"),
    ("HTTPS://EXAMPLE.COM", "https://example.com/"),
    ("https://example.com/", "https://example.com/"),
    (
        "https://example.com/a/b?c=d#e",
        "https://example.com/a/b?c=d#e",
    ),
    // A default port is dropped: it is the same URL either way, and keeping it
    // would make two spellings of one address compare differently.
    ("https://example.com:443/", "https://example.com/"),
    ("http://example.com:80/", "http://example.com/"),
    ("https://example.com:8443/", "https://example.com:8443/"),
    // Dot segments are resolved when it is parsed, not when it is fetched.
    ("https://example.com/a/./b/../c", "https://example.com/a/c"),
    ("https://example.com/a/../../b", "https://example.com/b"),
    // A space is not a URL character.
    ("https://example.com/a b", "https://example.com/a%20b"),
    // The userinfo is kept, and it is exactly why a person cannot read a URL.
    ("https://user@example.com/", "https://user@example.com/"),
    ("file:///etc/hosts", "file:///etc/hosts"),
    ("data:text/plain,hello", "data:text/plain,hello"),
];

#[test]
fn every_url_writes_back_out_the_way_the_standard_says() {
    for (input, expected) in ROUND_TRIPS {
        let parsed = parse(input).unwrap_or_else(|why| panic!("{input}: {why}"));
        assert_eq!(&parsed.serialised, expected, "{input}");
    }
}

#[test]
fn what_is_not_a_url_is_refused_rather_than_guessed_at() {
    for input in [
        "",
        "  ",
        "not a url",
        "/just/a/path",
        "//example.com/",
        "https://",
        // A port that does not fit in the sixteen bits a port has.
        "https://example.com:99999/",
        "http://[not an address]/",
    ] {
        assert!(parse(input).is_err(), "{input:?} should be refused");
    }
}

#[test]
fn a_relative_reference_is_resolved_against_the_page_it_was_written_on() {
    let base = parse("https://example.com/one/two?q=1#f").expect("a base");
    for (reference, expected) in [
        ("three", "https://example.com/one/three"),
        ("/three", "https://example.com/three"),
        ("../three", "https://example.com/three"),
        ("./three", "https://example.com/one/three"),
        ("?q=2", "https://example.com/one/two?q=2"),
        ("#g", "https://example.com/one/two?q=1#g"),
        ("", "https://example.com/one/two?q=1"),
        ("//other.example/x", "https://other.example/x"),
        // An absolute reference replaces the base entirely, which is what
        // makes a link to somewhere else work at all.
        ("http://other.example/x", "http://other.example/x"),
    ] {
        let resolved = join(&base, reference).unwrap_or_else(|why| panic!("{reference}: {why}"));
        assert_eq!(&resolved.serialised, expected, "{reference}");
    }
}

#[test]
fn the_parts_are_the_parts_and_not_slices_of_a_string() {
    let parsed = parse("https://example.com:8443/a/b?c=d#e").expect("a URL");
    assert_eq!(parsed.scheme, "https");
    assert_eq!(parsed.host, Some(Host::Domain("example.com".to_owned())));
    assert_eq!(parsed.port, Some(8443));
    assert_eq!(parsed.path, "/a/b");
    assert_eq!(parsed.query.as_deref(), Some("c=d"));
    assert_eq!(parsed.fragment.as_deref(), Some("e"));
    assert_eq!(parsed.effective_port(), Some(8443));
    assert!(parsed.is_special());
}

#[test]
fn a_port_nobody_wrote_is_the_schemes_own() {
    let parsed = parse("https://example.com/").expect("a URL");
    assert_eq!(parsed.port, None, "nothing was written");
    assert_eq!(
        parsed.effective_port(),
        Some(443),
        "and this is what it means"
    );

    let data = parse("data:text/plain,hello").expect("a URL");
    assert_eq!(
        data.effective_port(),
        None,
        "there is nothing to connect to"
    );
    assert!(!data.is_special());
}

#[test]
fn an_address_is_an_address_and_not_a_name_that_looks_like_one() {
    let four = parse("http://127.0.0.1/").expect("a URL");
    assert_eq!(four.host, Some(Host::Ipv4(Ipv4Addr::LOCALHOST)));

    // The forms of an IPv4 address that a browser has to treat as the same
    // machine, and that a naive string comparison treats as three machines.
    for shorthand in ["http://127.1/", "http://2130706433/", "http://0177.0.0.1/"] {
        let parsed = parse(shorthand).unwrap_or_else(|why| panic!("{shorthand}: {why}"));
        assert_eq!(
            parsed.host,
            Some(Host::Ipv4(Ipv4Addr::LOCALHOST)),
            "{shorthand} is the same machine",
        );
    }

    let six = parse("http://[::1]/").expect("a URL");
    assert_eq!(six.host, Some(Host::Ipv6(Ipv6Addr::LOCALHOST)));
    assert_eq!(six.serialised, "http://[::1]/", "the brackets survive");
}

#[test]
fn a_host_written_in_another_script_is_held_as_what_it_resolves_to() {
    // The reason IDNA is rented rather than skipped. `münchen.example` is a
    // real host and has to work; the ASCII form is what a security decision
    // compares, and what a person is shown is a different question this
    // engine has not reached yet.
    let parsed = parse("https://münchen.example/").expect("a URL");
    assert_eq!(
        parsed.host,
        Some(Host::Domain("xn--mnchen-3ya.example".to_owned())),
        "held as punycode, so two spellings of one host are one host",
    );

    // And the reason it is a security question: these two are *not* the same
    // host, however alike they look. The second has a Cyrillic а.
    let latin = parse("https://apple.com/").expect("a URL");
    let cyrillic = parse("https://аpple.com/").expect("a URL");
    assert_ne!(
        latin.host, cyrillic.host,
        "a look-alike is not the same host"
    );
    assert_ne!(Origin::of(&latin), Origin::of(&cyrillic));
}

#[test]
fn an_origin_is_the_three_things_and_nothing_else() {
    let same = [
        "https://example.com/",
        "https://example.com/somewhere/else",
        "https://example.com:443/?a=b#c",
        "https://user:pass@example.com/",
    ];
    let first = Origin::of(&parse(same[0]).expect("a URL"));
    for other in &same[1..] {
        assert_eq!(
            first,
            Origin::of(&parse(other).expect("a URL")),
            "{other} is the same origin: the path, the query, the fragment and \
             the credentials are none of the origin's business",
        );
    }

    for different in [
        "http://example.com/",
        "https://example.com:8443/",
        "https://other.example/",
        "https://sub.example.com/",
    ] {
        assert_ne!(
            first,
            Origin::of(&parse(different).expect("a URL")),
            "{different} is a different origin",
        );
    }
}

#[test]
fn an_origin_writes_out_the_way_the_standard_says() {
    for (url, expected) in [
        ("https://example.com/a/b", "https://example.com"),
        ("https://example.com:443/", "https://example.com"),
        ("https://example.com:8443/", "https://example.com:8443"),
        ("http://example.com/", "http://example.com"),
        ("http://[::1]:8080/", "http://[::1]:8080"),
    ] {
        let origin = Origin::of(&parse(url).expect("a URL"));
        assert_eq!(origin.to_string(), expected, "{url}");
    }
}

#[test]
fn an_opaque_origin_is_the_same_as_itself_and_nothing_else() {
    // The rule this whole type exists for. Two `data:` URLs with identical
    // bytes are two origins — if they were one, every `data:` frame on a page
    // could read every other one.
    let one = Origin::of(&parse("data:text/plain,hello").expect("a URL"));
    let two = Origin::of(&parse("data:text/plain,hello").expect("a URL"));

    assert!(one.is_opaque());
    assert!(two.is_opaque());
    assert_eq!(one, one, "an origin is always itself");
    assert!(one.is_same_origin(&one));
    assert_ne!(one, two, "and never another, however alike they look");
    assert!(!one.is_same_origin(&two));
    assert_eq!(one.to_string(), "null");
}

#[test]
fn a_local_file_is_its_own_origin_and_so_is_a_scheme_nobody_considered() {
    // One local file reading every other one is the oldest exfiltration bug
    // there is. And a scheme this engine has not been told about inherits
    // nothing, because "unknown" must never mean "probably fine".
    for url in [
        "file:///etc/hosts",
        "file:///home/someone/notes.txt",
        "about:blank",
        "some-scheme-nobody-registered:whatever",
    ] {
        let origin = Origin::of(&parse(url).expect("a URL"));
        assert!(origin.is_opaque(), "{url} should have an origin of its own");
    }

    let one = Origin::of(&parse("file:///etc/hosts").expect("a URL"));
    let two = Origin::of(&parse("file:///etc/hosts").expect("a URL"));
    assert_ne!(one, two, "even the same file, opened twice");
}

#[test]
fn nothing_a_stranger_can_send_makes_this_panic() {
    // `docs/autonomy/LOOP.md`, stage 2: anything that reads bytes from outside
    // returns an error rather than panicking. In a renderer a crash is a
    // denial of service, and a URL is the first thing a stranger controls.
    let base = parse("https://example.com/a/b").expect("a base");
    let nasty: Vec<String> = vec![
        String::new(),
        "\0".to_owned(),
        "https://".to_owned() + &"a".repeat(100_000),
        "https://example.com/".to_owned() + &"../".repeat(50_000),
        "https://example.com/?".to_owned() + &"a=b&".repeat(50_000),
        "https://[".to_owned() + &":".repeat(1000) + "]/",
        "https://example.com:".to_owned() + &"9".repeat(1000),
        "\u{feff}https://example.com/".to_owned(),
        "https://exam\u{202e}ple.com/".to_owned(),
        "%".repeat(10_000),
        "http://\u{0}.com/".to_owned(),
        "javascript:alert(1)".to_owned(),
        "https://example.com/\u{d7ff}\u{e000}".to_owned(),
    ];
    for input in &nasty {
        // Either answer is fine. Not returning one is not.
        let parsed = parse(input);
        if let Ok(url) = &parsed {
            let _ = Origin::of(url);
            let _ = url.effective_port();
            let _ = url.to_string();
        }
        let _ = join(&base, input);
    }
}
