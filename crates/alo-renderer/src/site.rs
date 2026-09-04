/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which site a page belongs to, and therefore which process renders it.
//!
//! ADR 0005: a site is *"the scheme and the registrable domain, so two tabs on
//! the same site share a process and two sites never do."*
//!
//! # The three answers, and which one decides
//!
//! A page arrives with three different ways of saying where it came from, and
//! this file is where it is settled which of them a **process** is given:
//!
//! - The **origin** — scheme, host and port — is what every security decision
//!   in the browser is made against, and it is the *finest* of the three. It is
//!   not what a process is keyed by: `https://example.com` and
//!   `https://example.com:8443` are two origins that a page can already reach
//!   between with a link and a cookie, so separating them would cost a process
//!   and buy nothing.
//! - The **registrable domain** is the part somebody actually holds, decided
//!   against the public suffix list (`alo_url::site`, queue item 156). It is
//!   the *coarsest*, and on its own it is too coarse: it says nothing about the
//!   scheme, and `http://example.com` is not `https://example.com`.
//! - The **site** — the scheme and the registrable domain — is the one a
//!   process is given, and it is what [`Site`] is.
//!
//! # Except where there is no site, which is the case this file exists for
//!
//! A `data:` URL has no host. Nor does `about:blank`, nor `blob:`, nor a scheme
//! nobody has told this engine about; and `file:` has one only in the sense
//! that it is empty. Reading a site off those the way the paragraph above reads
//! it off `https://` gives the **scheme** and nothing else — so every `data:`
//! document in the browser was one site, and every local file was one site, and
//! each group shared one address space.
//!
//! Those documents do not share an origin. `alo_url::Origin` mints each of them
//! an **opaque** origin, which is the same as itself and nothing else — two
//! `data:` URLs with identical bytes are two origins, and *"one local file
//! being able to read every other one is the oldest exfiltration bug there
//! is"*. Putting two of them in one process is precisely what ADR 0005's first
//! reason forbids: Spectre is a hardware property, no same-origin check written
//! in any language reaches it, and the only mitigation that works is that the
//! process never holds the other document's data in the first place.
//!
//! So **the origin decides whether there is a site at all**. Where it is a
//! tuple, the registrable domain widens it into a site and two tabs share a
//! process. Where it is opaque, there is no site, the document is [`Site::Alone`]
//! and it shares a process with nothing — not with another `data:` URL, not
//! with the same `file:` path opened a second time, and not with itself
//! reloaded in another tab.
//!
//! This is why the answer is taken *from* [`alo_url::Origin::of`] rather than
//! restated here from the URL. Two functions deciding what is opaque is two
//! functions that can come to disagree, and the disagreement would be a process
//! holding two documents the security rules say are strangers.
//!
//! # What it costs, said plainly
//!
//! Somebody who opens twenty local files gets twenty renderers rather than one,
//! and the ceiling ([`crate::host::MOST_RENDERERS`]) evicts the least recently
//! used of them. ADR 0005 already names that price — *"N processes cost N
//! processes. Memory goes up and it is the price of the first reason in this
//! document"* — and this is the direction to pay it in: the failure the other
//! way is two strangers in one address space, which is the failure the whole
//! design exists to prevent.
//!
//! # A site is decided once, when the document is
//!
//! [`Site::of`] mints a fresh identity every time it is called on a URL with an
//! opaque origin, because that is what an opaque origin *is*. So a caller keeps
//! the answer for as long as it keeps the document: [`crate::tab::Tabs::open`]
//! decides a tab's site once and every ask after that uses the one it kept.
//! Deciding it again per request would give the same tab a new process each
//! time it painted.
//!
//! # What changed, and which direction it moved
//!
//! Until queue item 156 a site was the scheme and the **host**, which was
//! stricter: `a.example.com` and `b.example.com` got separate processes where
//! they should share one, which cost memory and never put two sites together.
//! It is looser for those now, and deliberately — one process for one
//! registrable domain is what ADR 0005 decided and what the isolation is sized
//! for. The failure in the other direction, two *sites* sharing a process
//! because we could not tell them apart, is what the list makes impossible
//! rather than unlikely: `bbc.co.uk` and `gov.co.uk` share a suffix and are not
//! one site, and no comparison of host strings could have said so.
//!
//! Queue item 66 moved the hostless documents the other way, and for the same
//! reason read from the other end: they had been one site because nothing had
//! asked what their origin was.

use alo_url::{Opaque, Origin, Url};
use core::fmt;

/// Which process renders a document.
///
/// Two documents with equal [`Site`]s share a renderer; two with different ones
/// never do.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Site {
    /// A site: the scheme and the registrable domain.
    ///
    /// Two tabs here share a process, which is the ordinary case and the one
    /// ADR 0005 is written about.
    Registrable {
        /// The scheme, lowercased. `http://example.com` and
        /// `https://example.com` are two sites however alike the host is.
        scheme: String,
        /// The registrable domain, or the host itself where there is none — an
        /// address, or a name that is a public suffix.
        host: String,
    },
    /// A document whose origin is opaque, in a process of its own.
    ///
    /// There is no site to be part of: `data:`, `blob:`, `about:`, a local file
    /// and any scheme nobody registered are each the same origin as themselves
    /// and nothing else, so each gets a renderer nothing else is ever put into.
    /// The scheme is kept beside the identity so that a person told a renderer
    /// died knows what kind of document it was holding.
    Alone {
        /// The scheme, lowercased.
        scheme: String,
        /// Which opaque origin. Minted once and never reused, so no second
        /// document can be given this process by arriving with the same bytes.
        alone: Opaque,
    },
}

impl Site {
    /// Which process a URL's document gets.
    ///
    /// # This mints an identity, so call it once per document
    ///
    /// A URL with an opaque origin gets a **new** answer every call, which is
    /// what makes two `data:` documents two processes. A caller that asked
    /// twice about one document would be asking for two processes for it — see
    /// the note on deciding a site once at the top of this file.
    pub fn of(url: &Url) -> Self {
        match Origin::of(url) {
            // The port is dropped here rather than never asked for: a site is
            // the scheme and the registrable domain, and `Origin` is where the
            // engine's one answer about what a scheme means already lives.
            Origin::Tuple { scheme, host, .. } => Self::Registrable {
                scheme,
                host: alo_url::site::of(&host),
            },
            Origin::Opaque(alone) => Self::Alone {
                scheme: url.scheme.to_ascii_lowercase(),
                alone,
            },
        }
    }

    /// The scheme.
    pub fn scheme(&self) -> &str {
        match self {
            Self::Registrable { scheme, .. } | Self::Alone { scheme, .. } => scheme,
        }
    }

    /// The registrable domain, or the host itself where there is none — an
    /// address, or a name that is a public suffix.
    ///
    /// [`None`] for a document that has no site: the answer is an identity
    /// rather than a name, and a caller shown an empty string would read it as
    /// a host that every other hostless document shares, which is the belief
    /// this type exists to make impossible.
    pub fn host(&self) -> Option<&str> {
        match self {
            Self::Registrable { host, .. } => Some(host),
            Self::Alone { .. } => None,
        }
    }

    /// Whether this document is in a process nothing else may ever join.
    pub fn is_alone(&self) -> bool {
        matches!(self, Self::Alone { .. })
    }
}

impl fmt::Display for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Registrable { scheme, host } => write!(f, "{scheme}://{host}"),
            // `null` is what WHATWG serialises an opaque origin as, and the
            // number after it is ours: two of these are different processes,
            // and a person reading two identical sentences about two renderers
            // would have no way to tell which one they were being told about.
            Self::Alone { scheme, alone } => write!(f, "{scheme}:({alone:#})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Site;
    use alo_url::Url;

    fn url(text: &str) -> Url {
        match alo_url::parse(text) {
            Ok(url) => url,
            Err(why) => panic!("{text} is not a URL: {why}"),
        }
    }

    fn site(text: &str) -> Site {
        Site::of(&url(text))
    }

    // --- Where there is a site ------------------------------------------------

    /// ADR 0005's own sentence: two tabs on the same site share a process and
    /// two sites never do.
    #[test]
    fn the_registrable_domain_is_what_a_process_is_given_rather_than_the_host() {
        assert_eq!(
            site("https://a.example.com/"),
            site("https://b.example.com/")
        );
        assert_eq!(
            site("https://www.example.com/"),
            site("https://example.com/")
        );
        assert_ne!(
            site("https://www.bbc.co.uk/"),
            site("https://www.gov.co.uk/")
        );
        assert_eq!(site("https://a.example.com/").host(), Some("example.com"));
    }

    #[test]
    fn the_scheme_is_part_of_it_because_two_schemes_are_two_sites() {
        assert_ne!(site("http://example.com/"), site("https://example.com/"));
        assert_eq!(
            site("https://example.com/one"),
            site("https://example.com/two")
        );
    }

    /// The site is not the origin, and this is the case that says which of the
    /// two a process is keyed by. Two ports are two origins; they are one site,
    /// and separating them would cost a process to divide pages that can
    /// already reach one another.
    #[test]
    fn a_port_is_the_origins_business_and_not_a_second_process() {
        assert_eq!(
            site("https://example.com:8443/"),
            site("https://example.com/"),
        );
        assert_eq!(
            site("https://example.com:8443/").host(),
            Some("example.com")
        );
    }

    /// An address has no registrable domain — read as a name, `127.0.0.1` has
    /// the site `0.1` — so it is its own, which `alo_url::site` decides and
    /// this asserts is what reaches a process.
    #[test]
    fn an_address_is_a_site_of_its_own_rather_than_a_name_read_backwards() {
        assert_eq!(site("http://127.0.0.1:8080/").host(), Some("127.0.0.1"));
        assert_ne!(site("http://127.0.0.1/"), site("http://127.0.0.2/"));
        assert_eq!(site("http://[::1]/").host(), Some("[::1]"));
    }

    /// A site is a value a caller keeps, and for a document that has one it is
    /// the same answer however often it is asked for. The opposite property is
    /// the point of the tests below, so both are asserted rather than one.
    #[test]
    fn asking_twice_about_a_page_with_a_site_gives_the_same_answer() {
        assert_eq!(site("https://example.com/"), site("https://example.com/"));
        assert!(!site("https://example.com/").is_alone());
    }

    // --- Where there is not ---------------------------------------------------

    /// The item's closing condition. Two `data:` documents are two origins —
    /// `alo_url` says so and has said so since queue item 50 — and until this
    /// they were one process.
    #[test]
    fn two_data_urls_are_two_processes_even_with_the_same_bytes() {
        let one = site("data:text/html,<p>hello");
        let two = site("data:text/html,<p>hello");
        assert!(one.is_alone());
        assert!(two.is_alone());
        assert_ne!(one, two, "two opaque origins were given one process");
        assert_eq!(one.host(), None);
        assert_eq!(one.scheme(), "data");
    }

    /// *"One local file being able to read every other one is the oldest
    /// exfiltration bug there is"* — `alo_url::origin`'s own words, and the
    /// process split now agrees with the origin rather than undoing it.
    #[test]
    fn two_local_files_are_two_processes_and_so_is_one_file_opened_twice() {
        let one = site("file:///Users/somebody/a.html");
        let two = site("file:///Users/somebody/b.html");
        let again = site("file:///Users/somebody/a.html");
        assert!(one.is_alone() && two.is_alone());
        assert_ne!(one, two);
        assert_ne!(one, again, "the same path twice was given one process");
        assert_eq!(one.scheme(), "file");
    }

    /// Unknown must never mean "probably fine". A scheme nobody has told this
    /// engine about has an opaque origin, so it gets a process nothing else is
    /// in — rather than a process shared with every other page on a scheme
    /// nobody considered.
    #[test]
    fn a_scheme_nobody_told_us_about_shares_a_process_with_nothing() {
        for text in ["about:blank", "wibble:something", "blob:whatever"] {
            let one = Site::of(&url(text));
            let two = Site::of(&url(text));
            assert!(one.is_alone(), "{text} was given a site");
            assert_ne!(one, two, "{text} twice was one process");
        }
    }

    /// A person is told which renderer went. Two of these are two processes, so
    /// two identical sentences would be a message nobody could act on.
    #[test]
    fn what_a_person_is_told_names_the_scheme_and_tells_two_of_them_apart() {
        assert_eq!(
            site("https://a.example.com/").to_string(),
            "https://example.com"
        );
        let one = site("data:text/plain,x").to_string();
        let two = site("data:text/plain,x").to_string();
        assert!(one.starts_with("data:("), "{one}");
        assert!(one.contains("null"), "{one}");
        assert_ne!(one, two, "two processes were described identically");
    }

    /// A URL comes off a stranger's page. Nothing here may panic on one, and
    /// every answer is either a site or a process of its own — there is no
    /// third outcome, and no URL falls through to sharing a process by
    /// accident.
    #[test]
    fn every_url_a_page_could_hold_is_answered_rather_than_crashed_on() {
        let odd = [
            "https://example.com",
            "https://xn--80ak6aa92e.com/",
            "https://a.very.deep.chain.of.labels.example.co.uk/",
            "http://[2001:db8::1]:65535/",
            "https://example.com.:443/",
            "https://localhost/",
            "https://com/",
            "data:,",
            "file:///",
            "about:srcdoc",
            "javascript:alert(1)",
            "mailto:somebody@example.com",
            "ws://example.com/socket",
            "ftp://files.example.com/",
        ];
        for text in odd {
            let Ok(parsed) = alo_url::parse(text) else {
                continue;
            };
            let site = Site::of(&parsed);
            assert!(!site.scheme().is_empty(), "{text} lost its scheme");
            match site.host() {
                Some(host) => assert!(!host.is_empty(), "{text} became an empty site"),
                None => assert!(site.is_alone(), "{text} has no host and no identity"),
            }
        }
    }
}
