/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Sites that have said they should never be reached without TLS.
//!
//! # The attack this stops, and the one it does not
//!
//! Somebody types `example.com`. The browser tries `http://` first, a network
//! in between answers, and the person never reaches the real site at all — the
//! redirect to `https://` that the real server would have sent never happens,
//! because the real server was never asked. That is `sslstrip`, it is twenty
//! years old, and no amount of correct TLS prevents it: the whole attack takes
//! place before any TLS begins.
//!
//! HSTS is a site saying *"having reached me once, never do that again"*. The
//! first visit is still exposed — that is the gap a preload list fills, and this
//! engine does not ship one (see below). Every visit after it is not.
//!
//! # Two rules that are easy to get wrong and fatal to get wrong
//!
//! - **A `Strict-Transport-Security` header arriving over plain HTTP is
//!   ignored.** Honouring it would let the attacker who is already rewriting
//!   your traffic pin a domain forever, which turns a defence into a denial of
//!   service.
//! - **It never applies to an IP address.** `https://192.168.1.1` has no name to
//!   pin, and a rule keyed on one would be pinned against every machine that
//!   ever holds that address.

use crate::headers::Headers;
use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, SystemTime};

/// The longest a site may pin itself for.
///
/// Two years, which is what the common advice asks for. A cap because the value
/// is a number a site chooses and a mistake in it is unreachable for as long as
/// it says — a site that pins itself for a century after moving off TLS has
/// removed itself from this browser for a century.
pub const LONGEST: Duration = Duration::from_secs(2 * 365 * 24 * 60 * 60);

/// What one host has said about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pinned {
    /// When it stops applying.
    pub until: SystemTime,
    /// Whether it covers everything under the name as well.
    pub covers_subdomains: bool,
}

/// Which sites have said never to reach them insecurely.
#[derive(Debug, Default)]
pub struct Known {
    held: HashMap<String, Pinned>,
}

impl Known {
    /// Knowing nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many hosts are pinned.
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether none are.
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Learn from a response.
    ///
    /// `over_tls` is whether the response that carried this arrived securely.
    /// **It must be true or nothing is learned** — see this module's own note.
    pub fn learn(&mut self, host: &str, headers: &Headers, over_tls: bool, now: SystemTime) {
        if !over_tls || !is_a_name(host) {
            return;
        }
        let Some(said) = headers.get("Strict-Transport-Security") else {
            return;
        };
        let mut seconds: Option<u64> = None;
        let mut covers_subdomains = false;
        for part in said.split(';') {
            let (name, argument) = match part.split_once('=') {
                Some((name, argument)) => (name.trim(), argument.trim().trim_matches('"')),
                None => (part.trim(), ""),
            };
            match name.to_ascii_lowercase().as_str() {
                "max-age" => seconds = argument.parse::<u64>().ok(),
                "includesubdomains" => covers_subdomains = true,
                _ => {}
            }
        }
        // A header with no `max-age` says nothing, and saying nothing must not
        // clear a pin that was set properly earlier.
        let Some(seconds) = seconds else {
            return;
        };
        let host = host.to_ascii_lowercase();
        if seconds == 0 {
            // Zero is how a site releases itself, and it has to work: a site
            // that could pin itself and never unpin would be a site nobody
            // could move off TLS in an emergency.
            self.held.remove(&host);
            return;
        }
        let held_for = Duration::from_secs(seconds).min(LONGEST);
        self.held.insert(
            host,
            Pinned {
                until: now + held_for,
                covers_subdomains,
            },
        );
    }

    /// Whether this host must be reached over TLS.
    pub fn must_be_secure(&self, host: &str, now: SystemTime) -> bool {
        if !is_a_name(host) {
            return false;
        }
        let host = host.to_ascii_lowercase();
        if let Some(pinned) = self.held.get(&host) {
            if pinned.until > now {
                return true;
            }
        }
        // A parent that covers its subdomains covers this one. Walked label by
        // label rather than by suffix, because `evil-example.com` ends with
        // `example.com` and is not under it.
        let mut rest = host.as_str();
        while let Some((_, parent)) = rest.split_once('.') {
            if let Some(pinned) = self.held.get(parent) {
                if pinned.covers_subdomains && pinned.until > now {
                    return true;
                }
            }
            rest = parent;
        }
        false
    }

    /// Forget anything that has run out.
    pub fn forget_the_expired(&mut self, now: SystemTime) {
        self.held.retain(|_, pinned| pinned.until > now);
    }

    /// Forget one host, which is what clearing a site's data has to mean.
    pub fn forget(&mut self, host: &str) {
        self.held.remove(&host.to_ascii_lowercase());
    }
}

/// Whether this is a name rather than an address.
///
/// An address has nothing to pin: it belongs to whoever holds it today, and a
/// rule keyed on one would follow the address rather than the site.
fn is_a_name(host: &str) -> bool {
    let bare = host.trim_matches(['[', ']']);
    bare.parse::<IpAddr>().is_err() && !bare.is_empty()
}
