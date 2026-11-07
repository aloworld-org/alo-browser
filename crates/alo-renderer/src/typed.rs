/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a person typed into the one box means, before any model is asked
//! (ADR 0074, queue item 701, `docs/design/shell.md`, *What appears when a
//! person types*).
//!
//! The design's screen 0b offers what is typed three ways, in order: *Go to
//! the site*, when what is typed is an address; then *Search for "…"*; then
//! *Ask alo about "…"*. The first is what Return chooses. This is the
//! deciding, and nothing else: which of the three are offered, and what
//! address the first goes to. The browser process decides it — it holds
//! what is typed — and sends the offers to the first screen's renderer as
//! data ([`crate::start::Start`]), which draws them
//! ([`crate::start_document`]) and decides nothing.
//!
//! # What counts as an address
//!
//! Only what a person would mean as one, read by this repository's own URL
//! parser (`alo-url`) and never by guessing at the words:
//!
//! - **Written with `http://` or `https://`**: whatever the parser makes of
//!   it, when it has a host. Any other scheme — `file:`, `javascript:`,
//!   `data:` — is not a site, and is not offered as one.
//! - **Written without a scheme**: a host, then perhaps a port of digits,
//!   then perhaps a path, query or fragment, read as `https://` and that.
//!   The host is `localhost`, an IPv4 address written as four numbers, an
//!   IPv6 address in brackets, or a name with a dot in it whose every label
//!   is something and whose last begins with a letter — so `example.com`,
//!   `localhost:8080` and `127.0.0.1` are addresses, and `3.14`, `hello`,
//!   `e.g.` and `me@example.com` are not.
//! - **On this machine or its network** (queue item 874):
//!   written without a scheme, `localhost`, a name ending `.localhost`, and
//!   an address that is not the public internet's — loopback, a private
//!   network's (`10.`, `172.16.`–`172.31.`, `192.168.`), link-local, a
//!   unique local IPv6 address, as `alo-net`'s
//!   [`is_public`](alo_net::resolve::is_public) judges them — are read as
//!   `http://`, as Chrome reads them. A server a person runs on their own
//!   machine or a device on their own network almost never has a
//!   certificate, so `https://` there is an error page where `http://` is
//!   the page; nothing between the two ends is a stranger's network. A
//!   public host keeps `https://`.
//!
//! Nothing with white space or a control character in it is an address:
//! `what is x` is a question. Searching and asking are always offered for
//! anything typed, since either can be what a person meant. Asking is the
//! agent's (queue item 704): set up while a page is open as a card and the
//! model can be asked, and then offered before searching
//! ([`offers_for`]), and saying it is not set up and why otherwise; searching
//! goes to the Brave Search API when the browser process holds a key, and
//! says it is not set up and why when it does not (queue item 708,
//! [`crate::results::SetUp`]) — offered, so the screen is the design's, and
//! saying so, rather than pretending.

use alo_net::resolve::is_public;
use alo_url::{Host, Url};
use std::net::{IpAddr, Ipv4Addr};

/// The most characters the box holds. A person types far fewer; what goes
/// past it is not taken, so no key can make the first screen's document
/// without end.
pub const LONGEST_TYPED: usize = 2048;

/// One thing offered under the box for what was typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offer {
    /// *Go to the site*: open this address.
    Go(Url),
    /// *Search for "…"*, with what was typed, trimmed: set up when the
    /// browser process says so (queue item 708, [`crate::results::SetUp`]).
    Search(String),
    /// *Ask alo about "…"*, with what was typed, trimmed: the agent, on the
    /// page open as a card, set up when the browser process says so (queue
    /// item 704).
    Ask(String),
}

impl Offer {
    /// Whether choosing it can do what it says whatever else holds: going to
    /// a site always can; asking (704) and searching (708) can when the
    /// first screen is told each is set up, which this does not know.
    pub fn is_set_up(&self) -> bool {
        matches!(self, Offer::Go(_))
    }
}

/// What is offered for `typed`, in the order the design offers it: going
/// to the site first when it is an address, then searching and asking.
/// Nothing is offered for nothing typed, or only white space.
pub fn offers(typed: &str) -> Vec<Offer> {
    offers_for(typed, false)
}

/// [`offers`], asking alo before searching when `asking_first` — while a
/// page is open as a card and the agent can be asked (queue item 704,
/// `docs/design/shell.md`: a request typed while a card is open is the
/// agent's), so that Return on words asks alo to do them on the page.
pub fn offers_for(typed: &str, asking_first: bool) -> Vec<Offer> {
    let said = typed.trim();
    if said.is_empty() {
        return Vec::new();
    }
    let mut offers = Vec::with_capacity(3);
    if let Some(url) = address(said) {
        offers.push(Offer::Go(url));
    }
    let search = Offer::Search(said.to_owned());
    let ask = Offer::Ask(said.to_owned());
    if asking_first {
        offers.extend([ask, search]);
    } else {
        offers.extend([search, ask]);
    }
    offers
}

/// The address `typed` names, if it is one (see the module's words).
pub fn address(typed: &str) -> Option<Url> {
    let typed = typed.trim();
    if typed.is_empty()
        || typed
            .chars()
            .any(|each| each.is_whitespace() || each.is_control())
    {
        return None;
    }
    if typed.contains("://") {
        let url = alo_url::parse(typed).ok()?;
        return (is_the_web(&url) && url.host.is_some()).then_some(url);
    }
    // A person's address and a mail address look alike only to a parser:
    // `me@example.com` read as `https://me@example.com` is a sign-in.
    if typed.contains('@') {
        return None;
    }
    let ends = typed.find(['/', '?', '#']).unwrap_or(typed.len());
    let host = host_of(typed.get(..ends)?)?;
    if !is_a_host(host) {
        return None;
    }
    let url = alo_url::parse(&format!("https://{typed}")).ok()?;
    let host = url.host.as_ref()?;
    if is_on_this_network(host) {
        return alo_url::parse(&format!("http://{typed}")).ok();
    }
    Some(url)
}

/// Whether `host`, as parsed, is this machine or its own network's: read
/// as `http://` when typed with no scheme (queue item 874).
fn is_on_this_network(host: &Host) -> bool {
    match host {
        Host::Domain(name) => {
            let name = name.trim_end_matches('.');
            name.eq_ignore_ascii_case("localhost")
                || name
                    .rsplit_once('.')
                    .is_some_and(|(_, last)| last.eq_ignore_ascii_case("localhost"))
        }
        Host::Ipv4(four) => !is_public(IpAddr::V4(*four)),
        Host::Ipv6(six) => !is_public(IpAddr::V6(*six)),
    }
}

/// Whether `url` is one a site is at: `http` or `https`.
fn is_the_web(url: &Url) -> bool {
    matches!(url.scheme.as_str(), "http" | "https")
}

/// The host of `authority`, a host with perhaps a port of digits after it;
/// [`None`] when what follows a colon is not a port.
fn host_of(authority: &str) -> Option<&str> {
    let ends = if authority.starts_with('[') {
        authority.find(']')? + 1
    } else {
        authority.find(':').unwrap_or(authority.len())
    };
    let (host, rest) = authority.split_at(ends);
    match rest.strip_prefix(':') {
        None if rest.is_empty() => Some(host),
        Some(port) if !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()) => {
            Some(host)
        }
        _ => None,
    }
}

/// Whether `host`, as typed, is one a person means as a site's.
fn is_a_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") || host.parse::<Ipv4Addr>().is_ok() {
        return true;
    }
    if host.starts_with('[') {
        // The parser says whether it is an IPv6 address.
        return host.len() > 2;
    }
    let labels: Vec<&str> = host.split('.').collect();
    labels.len() >= 2
        && labels.iter().all(|label| !label.is_empty())
        && labels
            .last()
            .and_then(|last| last.chars().next())
            .is_some_and(char::is_alphabetic)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn went(typed: &str) -> Option<String> {
        address(typed).map(|url| url.to_string())
    }

    #[test]
    fn a_name_with_a_dot_is_an_https_address() {
        assert_eq!(went("example.com").as_deref(), Some("https://example.com/"));
        assert_eq!(
            went("  Example.COM  ").as_deref(),
            Some("https://example.com/")
        );
        assert_eq!(
            went("evhomeguide.org/chargers?near=me#map").as_deref(),
            Some("https://evhomeguide.org/chargers?near=me#map")
        );
        assert_eq!(
            went("shop.example.co.uk").as_deref(),
            Some("https://shop.example.co.uk/")
        );
    }

    #[test]
    fn an_address_written_whole_is_taken_as_written() {
        assert_eq!(went("https://a.b/c").as_deref(), Some("https://a.b/c"));
        assert_eq!(
            went("http://example.com:8080/x").as_deref(),
            Some("http://example.com:8080/x")
        );
        assert_eq!(
            went("http://localhost").as_deref(),
            Some("http://localhost/")
        );
    }

    #[test]
    fn this_machine_and_its_network_typed_bare_are_http() {
        // Queue item 874: `127.0.0.1:8739` opened as `https://` was an SSL
        // error.
        for (typed, url) in [
            ("127.0.0.1:8739", "http://127.0.0.1:8739/"),
            ("localhost:3000", "http://localhost:3000/"),
            ("localhost", "http://localhost/"),
            ("LocalHost:3000/a?b#c", "http://localhost:3000/a?b#c"),
            ("app.localhost:5173", "http://app.localhost:5173/"),
            ("192.168.1.5:8080", "http://192.168.1.5:8080/"),
            ("192.168.1.5", "http://192.168.1.5/"),
            ("10.0.0.2/admin", "http://10.0.0.2/admin"),
            ("172.16.0.1", "http://172.16.0.1/"),
            ("172.31.255.254:81", "http://172.31.255.254:81/"),
            ("169.254.1.1", "http://169.254.1.1/"),
            ("127.0.0.1:3000/a", "http://127.0.0.1:3000/a"),
            ("[::1]:8080", "http://[::1]:8080/"),
            ("[::1]", "http://[::1]/"),
            ("[fd00::5]:8080", "http://[fd00::5]:8080/"),
            ("[fe80::1]", "http://[fe80::1]/"),
            ("[::ffff:127.0.0.1]:80", "http://[::ffff:7f00:1]/"),
            // A port the parser drops for `https` is kept for `http`.
            ("localhost:443", "http://localhost:443/"),
        ] {
            assert_eq!(went(typed).as_deref(), Some(url), "{typed:?}");
        }
    }

    #[test]
    fn a_public_host_typed_bare_stays_https() {
        for (typed, url) in [
            ("example.com:8080", "https://example.com:8080/"),
            ("8.8.8.8", "https://8.8.8.8/"),
            ("1.1.1.1:853", "https://1.1.1.1:853/"),
            ("172.32.0.1", "https://172.32.0.1/"),
            ("172.15.255.255", "https://172.15.255.255/"),
            ("192.169.1.5", "https://192.169.1.5/"),
            ("11.0.0.1", "https://11.0.0.1/"),
            ("[2001:4860:4860::8888]", "https://[2001:4860:4860::8888]/"),
            // A name only ending in the letters, or holding the word, is
            // not this machine.
            ("notlocalhost.com", "https://notlocalhost.com/"),
            ("localhost.example.com", "https://localhost.example.com/"),
            ("evil-localhost.com", "https://evil-localhost.com/"),
            ("shop.notlocalhost", "https://shop.notlocalhost/"),
        ] {
            assert_eq!(went(typed).as_deref(), Some(url), "{typed:?}");
        }
    }

    #[test]
    fn a_scheme_written_is_kept_on_this_machine_too() {
        assert_eq!(
            went("https://localhost:8443/").as_deref(),
            Some("https://localhost:8443/")
        );
        assert_eq!(
            went("https://192.168.1.5/").as_deref(),
            Some("https://192.168.1.5/")
        );
        assert_eq!(
            went("http://example.com/").as_deref(),
            Some("http://example.com/")
        );
    }

    #[test]
    fn this_machines_words_are_still_searches() {
        for typed in [
            "localhost is down",
            "127.0.0.1 refused",
            "127.0.0",
            "127.0.0.1:",
            "127.0.0.1:port",
            "me@localhost",
            "mylocalhost",
            "192.168.1.5:8080 not loading",
        ] {
            assert_eq!(went(typed), None, "{typed:?}");
        }
    }

    #[test]
    fn words_questions_and_nothing_are_not_addresses() {
        for typed in [
            "",
            "   ",
            "hello",
            "hello world",
            "what is x",
            "what is example.com",
            "3.14",
            "e.g.",
            ".com",
            "a..b",
            "me@example.com",
            "localhost:http",
            "example.com:",
            "[::1",
        ] {
            assert_eq!(went(typed), None, "{typed:?}");
        }
    }

    #[test]
    fn another_scheme_is_never_a_site() {
        for typed in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "javascript://example.com/%0aalert(1)",
            "data:text/html,<b>x</b>",
            "ftp://example.com/",
            "mailto:me@example.com",
            "about:blank",
            "https://",
        ] {
            assert_eq!(went(typed), None, "{typed:?}");
        }
    }

    #[test]
    fn markup_and_control_characters_are_never_an_address() {
        for typed in ["<b>", "</script>", "a\u{0}.com", "exam\nple.com", "a\tb.c"] {
            assert_eq!(went(typed), None, "{typed:?}");
        }
        assert_eq!(went(&"a".repeat(LONGEST_TYPED)), None);
    }

    #[test]
    fn an_address_is_offered_to_go_to_first_then_search_and_ask() {
        let offered = offers(" example.com ");
        let go = alo_url::parse("https://example.com/").ok();
        assert_eq!(
            offered,
            vec![
                Offer::Go(go.unwrap_or_else(Url::about_blank)),
                Offer::Search("example.com".to_owned()),
                Offer::Ask("example.com".to_owned()),
            ]
        );
        assert!(offered.first().is_some_and(Offer::is_set_up));
        assert!(!offered.iter().skip(1).any(Offer::is_set_up));
        // While a card is open and the agent can be asked, asking comes
        // before searching, after going to an address.
        let asking = offers_for("example.com", true);
        assert!(matches!(
            asking.as_slice(),
            [Offer::Go(_), Offer::Ask(_), Offer::Search(_)]
        ));
        assert!(matches!(
            offers_for("send it", true).as_slice(),
            [Offer::Ask(_), Offer::Search(_)]
        ));
    }

    #[test]
    fn anything_else_is_offered_to_search_first_and_nothing_for_nothing() {
        assert_eq!(
            offers("what is x"),
            vec![
                Offer::Search("what is x".to_owned()),
                Offer::Ask("what is x".to_owned()),
            ]
        );
        assert!(offers("").is_empty());
        assert!(offers(" \t ").is_empty());
        assert_eq!(
            offers("<b>"),
            vec![
                Offer::Search("<b>".to_owned()),
                Offer::Ask("<b>".to_owned())
            ]
        );
    }
}
