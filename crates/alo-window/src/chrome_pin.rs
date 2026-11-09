/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A name a card's Chromium takes as alo resolved it, rather than asking
//! again (queue item 881, ADR 0103 § 7).
//!
//! alo's agent opens a site only once the browser process has resolved its
//! name with the rebinding rule — public answers only (ADR 0103 § 2.3). The
//! card's Chromium would then resolve the name again as it loads the page,
//! and a name whose answer changed between the two — public to alo, private
//! a moment later — would reach the person's network. So the Chromium a
//! card the agent opened is started in is told the address alo found, with
//! Chromium's own switch for it, `--host-resolver-rules=MAP host address`,
//! and never asks a resolver for that name at all.
//!
//! Measured on this Mac (the session scratchpad's `m881/`) in Google Chrome
//! 157.0.8103.38 and in the Chromium alo carries (CEF 156), each started as
//! a card: with no rule `http://pinned.invalid:P/` is `ERR_NAME_NOT_RESOLVED`;
//! with `MAP pinned.invalid 127.0.0.1` the page is asked of 127.0.0.1:P with
//! `Host: pinned.invalid:P`; of two rules for one name the first is taken; a
//! rule overrides a name the machine resolves itself (`localhost`); and an
//! IPv6 address is written in brackets.
//!
//! Each card is a Chromium of its own, started as it opens
//! (`conversation_chrome.rs`), so the rule holds for the agent's card and no
//! other: a page the person opens — their own typed local address among
//! them (queue item 874) — is never given one.
//!
//! A rule is written only for a name made of letters, digits, dots, hyphens
//! and underscores ([`Pinned::new`]): a URL's host may hold a comma or a
//! `*`, which in the switch would be a second rule or a pattern, and such a
//! name is not pinned — and so, by the agent's gate, not opened.

use std::net::IpAddr;

/// A name and the one address its card's Chromium is to load it from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pinned {
    host: String,
    address: IpAddr,
}

impl Pinned {
    /// `host` pinned to `address` — or nothing, for a host that is not a
    /// plain name: one with anything but ASCII letters, digits, `.`, `-` and
    /// `_`, or none at all.
    pub fn new(host: &str, address: IpAddr) -> Option<Self> {
        let plain = !host.is_empty()
            && host
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));
        plain.then(|| Self {
            host: host.to_ascii_lowercase(),
            address,
        })
    }

    /// The name.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The address it is loaded from.
    pub fn address(&self) -> IpAddr {
        self.address
    }

    /// The switch a Chromium is started with to take it:
    /// `--host-resolver-rules=MAP host address`, an IPv6 address in
    /// brackets.
    pub fn switch(&self) -> String {
        match self.address {
            IpAddr::V4(address) => format!("--host-resolver-rules=MAP {} {address}", self.host),
            IpAddr::V6(address) => format!("--host-resolver-rules=MAP {} [{address}]", self.host),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_pinned_to_its_address_in_chromiums_own_switch() {
        let four = Pinned::new("News.Example.org", IpAddr::from([93, 184, 215, 14]));
        assert_eq!(
            four.as_ref().map(Pinned::switch).as_deref(),
            Some("--host-resolver-rules=MAP news.example.org 93.184.215.14")
        );
        let six = Pinned::new(
            "v6.example",
            IpAddr::from([0x2001, 0xdb8, 0, 0, 0, 0, 0, 1]),
        );
        assert_eq!(
            six.as_ref().map(Pinned::switch).as_deref(),
            Some("--host-resolver-rules=MAP v6.example [2001:db8::1]")
        );
        assert_eq!(
            Pinned::new("_dmarc.a-b.example", IpAddr::from([1, 1, 1, 1])).map(|pin| pin.host),
            Some("_dmarc.a-b.example".to_owned())
        );
    }

    #[test]
    fn a_name_that_would_be_a_second_rule_or_a_pattern_is_never_pinned() {
        let address = IpAddr::from([93, 184, 215, 14]);
        for host in [
            "",
            "a.example, MAP * 127.0.0.1",
            "a,b.example",
            "*.example",
            "a?.example",
            "a b.example",
            "[::1]",
            "bücher.example",
        ] {
            assert_eq!(Pinned::new(host, address), None, "{host:?}");
        }
    }
}
