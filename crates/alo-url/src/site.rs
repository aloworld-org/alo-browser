/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where one site ends and another begins.
//!
//! An origin is a rule — scheme, host and port, all three equal — and a machine
//! can decide it from the URL alone. A **site** is not: `bbc.co.uk` and
//! `gov.co.uk` are two organisations and `www.example.com` and `example.com` are
//! one, and nothing in either name says so. What says so is a **list**: the
//! names anybody may register a domain under, kept by Mozilla, changed when a
//! registry changes. The site is the label immediately below the longest such
//! name that a host ends with — the *registrable domain*, the part a person or a
//! company actually holds.
//!
//! # Why this is a security boundary rather than a tidy-up
//!
//! Three things in this browser are decided per site rather than per origin, and
//! each of them is wrong in a different direction if this answer is wrong:
//!
//! - **Cookies** are partitioned by the top-level site (ADR 0007). Too coarse
//!   and two organisations share a jar; too fine and a person signing in at
//!   `example.com` is signed out at `www.example.com`.
//! - **The cache on a disk** is keyed by the same partition (ADR 0011), so the
//!   same answer decides whether one site can time a load and learn that
//!   somebody has been to another.
//! - **A renderer process** holds one site (ADR 0005). Too coarse is two sites
//!   sharing an address space, which is the failure that whole design exists to
//!   prevent.
//!
//! Until now this engine answered all three with the **host**, which is
//! stricter — every subdomain its own site — and stricter is the safe direction
//! to be wrong in, which is why it was what stood here. It was still wrong, and
//! `a.example.com` and `b.example.com` were two sites where they are one.
//!
//! # What is rented, and what a snapshot means
//!
//! The list is `psl`'s, and this is the only file that names it (ADR 0001). It
//! is data nobody can derive: no rule of syntax distinguishes `co.uk` from
//! `bbc.uk`, and guessing at one is how `Domain=co.uk` becomes a cookie for
//! every school and council in the country.
//!
//! `psl` compiles a **snapshot** of the list in rather than fetching it, which
//! is the right trade twice over: a browser whose security boundary arrived over
//! the network would have one only when the network worked, and a boundary that
//! changed under a running program would move where a cookie lives without
//! anybody deciding. The cost is that the snapshot ages, and updating it is a
//! version bump in `Cargo.toml` — a diff somebody reads.
//!
//! Which is a cost only if somebody is prompted to write that diff, and nothing
//! prompted anybody: a suffix delegated after ours was taken reads here as an
//! ordinary registrable domain, and two organisations quietly become one site.
//! [`crate::snapshot`] holds the day this one was taken and fails once it is six
//! months old, so an out-of-date boundary is a message rather than a silence.
//!
//! # The two answers this gives when the list has nothing to say
//!
//! Both are the **host itself**, which is the strict direction:
//!
//! - A host that *is* a public suffix — `com`, `co.uk`, `localhost`, or a name
//!   under a suffix nobody has registered under yet. There is no label below the
//!   suffix to be the site, so the host is its own site and joins nothing.
//! - An **address**. `127.0.0.1` has no registrable domain — reading it as a
//!   name gives the site `0.1`, which would put every machine on a `.0.1`
//!   address into one — so an address is its own site. That is why this takes a
//!   [`Host`] rather than a string: the type has already decided which it is,
//!   and a string has not.

use crate::parts::Host;

/// The site a host belongs to: its registrable domain, or the host itself when
/// there is none.
///
/// The answer every per-site decision in this browser is made against — the
/// cookie partition (ADR 0007), the cache key (ADR 0011) and the renderer
/// process (ADR 0005). It is the host part only: a caller that needs the scheme
/// too keeps it beside this, because a site is *scheme and* registrable domain
/// and joining `http:` to `https:` here would hide that from them.
pub fn of(host: &Host) -> String {
    match host {
        Host::Domain(name) => {
            // Lowercased rather than assumed lowercase. A parsed URL holds an
            // already-lowercased name, but `Host` is an ordinary type our own
            // code can build, and the list is matched byte for byte: `CO.UK`
            // matches no rule, falls to the rule of last resort, and would come
            // back as the registrable domain of `bbc.CO.UK`. That is a wrong
            // answer in the unsafe direction, so it is made unreachable rather
            // than documented.
            let name = name.to_ascii_lowercase();
            match registrable_domain(&name) {
                Some(domain) => domain.to_owned(),
                None => name,
            }
        }
        // An address is its own site. See the note above on `127.0.0.1`.
        address => address.to_string(),
    }
}

/// Whether a domain name is a public suffix — a name **under** which anybody may
/// register, rather than one somebody holds.
///
/// `com`, `co.uk` and `github.io` are; `example.com` and `bbc.co.uk` are not.
/// So is any name of a single label, `localhost` among them, and so is a name
/// under a suffix the list has never heard of, because the list's own rule of
/// last resort is that an unknown final label is a suffix.
///
/// A cookie's `Domain` attribute is the reason this is public: `Domain=co.uk` is
/// a cookie for every organisation in the United Kingdom, and refusing it is not
/// something a parser can do on the shape of the string.
pub fn is_a_public_suffix(name: &str) -> bool {
    registrable_domain(&name.to_ascii_lowercase()).is_none()
}

/// The registrable domain of a lowercase name, or [`None`] when the name is
/// itself a public suffix.
///
/// A trailing dot is **kept**: `example.com.` and `example.com` are two spellings
/// the rest of this browser already treats as two hosts — a cookie set by one is
/// not sent to the other — and a site boundary that quietly joined them would be
/// the only place they met.
fn registrable_domain(lowercase: &str) -> Option<&str> {
    let domain = psl::domain(lowercase.as_bytes())?;
    // `psl` hands back a slice of what it was given, so this cannot fail; it is
    // written as a question rather than an assumption because the alternative
    // spelling is `expect`, and the gate forbids that for the reason that a
    // renderer which aborts is worse than one which reports.
    core::str::from_utf8(domain.as_bytes()).ok()
}

#[cfg(test)]
mod tests {
    use super::{is_a_public_suffix, of};
    use crate::parts::Host;

    fn site(host: &str) -> String {
        of(&Host::Domain(host.to_owned()))
    }

    /// The item's closing condition, both halves, naming both.
    #[test]
    fn bbc_and_gov_are_two_sites_under_one_suffix_and_www_is_not_a_third() {
        assert_ne!(site("www.bbc.co.uk"), site("www.gov.co.uk"));
        assert_eq!(site("www.bbc.co.uk"), "bbc.co.uk");
        assert_eq!(site("www.gov.co.uk"), "gov.co.uk");

        assert_eq!(site("www.example.com"), site("example.com"));
        assert_eq!(site("a.example.com"), site("b.example.com"));
        assert_eq!(site("deep.down.a.example.com"), "example.com");
    }

    /// A suffix with a dot in it is what a host comparison cannot see, and it is
    /// the whole reason this file rents a list.
    #[test]
    fn a_multi_label_suffix_is_one_suffix_rather_than_two_labels() {
        assert_eq!(site("shop.example.co.uk"), "example.co.uk");
        assert_eq!(site("pages.example.github.io"), "example.github.io");
        assert_eq!(site("example.com.au"), "example.com.au");
    }

    /// The list has wildcard rules and exceptions to them, and a browser that
    /// implemented only the ordinary case would be wrong on whole countries.
    #[test]
    fn a_wildcard_rule_and_its_exception_are_both_honoured() {
        // `*.ck` with `!www.ck`: every name under `.ck` is a suffix except
        // `www.ck`, which is a site.
        assert_eq!(site("a.b.ck"), "a.b.ck");
        assert_eq!(site("www.ck"), "www.ck");
        assert_eq!(site("anything.www.ck"), "www.ck");
    }

    #[test]
    fn a_host_that_is_itself_a_suffix_is_its_own_site() {
        assert_eq!(site("com"), "com");
        assert_eq!(site("co.uk"), "co.uk");
        assert_eq!(site("localhost"), "localhost");
        // A suffix nobody has heard of: the rule of last resort makes the final
        // label the suffix, so this is a site rather than a suffix.
        assert_eq!(site("example.invalid"), "example.invalid");
        assert_eq!(site("invalid"), "invalid");
    }

    #[test]
    fn an_address_is_its_own_site_rather_than_a_name_read_backwards() {
        assert_eq!(
            of(&Host::Ipv4(
                "127.0.0.1".parse().unwrap_or([0, 0, 0, 0].into())
            )),
            "127.0.0.1"
        );
        assert_eq!(
            of(&Host::Ipv6(
                "::1".parse().unwrap_or(std::net::Ipv6Addr::UNSPECIFIED)
            )),
            "[::1]"
        );
    }

    #[test]
    fn a_name_written_in_capitals_is_the_same_site() {
        assert_eq!(site("WWW.BBC.CO.UK"), "bbc.co.uk");
        assert_ne!(site("WWW.BBC.CO.UK"), site("WWW.GOV.CO.UK"));
    }

    /// Two spellings the rest of the browser keeps apart stay apart here.
    #[test]
    fn a_trailing_dot_is_kept_rather_than_joined_to_the_name_without_one() {
        assert_eq!(site("www.example.com."), "example.com.");
        assert_ne!(site("www.example.com."), site("www.example.com"));
    }

    #[test]
    fn a_cookies_domain_attribute_can_be_refused_by_name() {
        assert!(is_a_public_suffix("com"));
        assert!(is_a_public_suffix("co.uk"));
        assert!(is_a_public_suffix("github.io"));
        assert!(is_a_public_suffix("localhost"));
        assert!(!is_a_public_suffix("example.com"));
        assert!(!is_a_public_suffix("bbc.co.uk"));
        assert!(!is_a_public_suffix("example.github.io"));
    }

    /// A host arrives from a stranger's page. Nothing here may panic on one, and
    /// every answer is the host itself — which is the strict direction.
    #[test]
    fn a_malformed_name_is_refused_a_site_rather_than_crashing() {
        for name in [
            "",
            ".",
            "..",
            "...",
            ".com",
            "com.",
            ".example.com.",
            "..example..com..",
            "-",
            "a..b",
            "\u{0}",
            "\u{e9}xample.com",
            "aa\u{300}.co.uk",
        ] {
            let answer = site(name);
            assert!(
                !answer.is_empty() || name.is_empty(),
                "{name:?} lost its site entirely"
            );
            assert!(
                name.to_ascii_lowercase().ends_with(&answer),
                "{name:?} became {answer:?}, which is not part of it"
            );
        }
    }

    /// A name is bounded by nothing but the URL parser, and the parser is not
    /// this file's. Long is not a crash.
    #[test]
    fn a_name_of_ten_thousand_labels_is_answered_rather_than_recursed_into() {
        let many = "a.".repeat(10_000);
        assert_eq!(site(&format!("{many}example.com")), "example.com");
        // Ending in a dot, so the suffix is `a.` and the site the label below
        // it — the trailing-dot rule above, arrived at from the other side.
        assert_eq!(site(&many), "a.a.");
        assert_eq!(site(&"a".repeat(100_000)), "a".repeat(100_000));
    }
}
