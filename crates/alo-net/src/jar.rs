/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where cookies are kept, and which ones a request may carry.
//!
//! [`crate::cookie`] reads one `Set-Cookie` and decides whether it is
//! acceptable at all. This file decides *which stored cookies go out with which
//! request*, and that is where ADR 0007's promise is either kept or quietly
//! lost.
//!
//! # The promise, in one sentence
//!
//! Every lookup takes a **partition** — the top-level site the person is
//! looking at — and there is no way to ask for cookies without one. A caller
//! cannot accidentally get the unpartitioned set, because there is no function
//! that returns it.
//!
//! # What is not here
//!
//! The escape hatch. ADR 0007 says a person may grant one embedded site access
//! inside one top-level site, and specifies it by what it must not be: never a
//! global toggle, never an allowlist we ship. It is not implemented here
//! because a grant is a thing a person makes in an interface, and there is no
//! interface yet — but nothing in this file would have to change to add it,
//! which is the test of whether the shape is right.

use crate::cookie::{Cookie, Partition, SameSite, covers, path_applies};
use alo_url::Url;
use std::collections::BTreeMap;
use std::time::SystemTime;

/// The most cookies one site may keep, and the most in all.
///
/// Bounds, for the reason every other bound in this crate exists: without one,
/// how much memory a browser spends is a number somebody else chooses.
pub const MOST_PER_SITE: usize = 180;
/// The most cookies held across every site.
pub const MOST_IN_ALL: usize = 10_000;

/// How a request came to be made, which decides what may go with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// The person navigated here by following a link or typing.
    ///
    /// The only kind that carries a `Lax` cookie across a site boundary.
    Navigated,
    /// A page asked for it — an image, a stylesheet, a script, a fetch.
    Embedded,
}

/// What is kept.
#[derive(Debug, Default)]
pub struct Jar {
    /// Keyed by the whole of a cookie's identity, so a cookie in one partition
    /// can never overwrite the same-named cookie in another. `BTreeMap` rather
    /// than a hash map: the order is stable, so what a request carries is the
    /// same on every run and a test can assert it.
    held: BTreeMap<(Partition, String, String, String), Cookie>,
}

impl Jar {
    /// An empty jar.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many cookies are held.
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether anything is held.
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Keep a cookie, or forget it if it has already expired.
    ///
    /// A cookie is deleted by being set again with an expiry in the past, which
    /// is the only way a server can delete one — so storing an expired cookie
    /// and storing a deletion are the same operation, and separating them would
    /// mean a site could never remove anything.
    pub fn keep(&mut self, cookie: Cookie, now: SystemTime) {
        let key = cookie.key();
        if cookie.has_expired(now) {
            self.held.remove(&key);
            return;
        }
        let site = key.0.clone();
        self.held.insert(key, cookie);
        self.forget_the_expired(now);
        self.stay_within_bounds(&site);
    }

    /// The `Cookie` header for this request, or [`None`] when there is nothing
    /// to send.
    ///
    /// `within` is the top-level site. There is no version of this function
    /// that does not take one.
    pub fn header_for(
        &self,
        target: &Url,
        within: &Partition,
        how: How,
        now: SystemTime,
    ) -> Option<String> {
        let sending = self.for_request(target, within, how, now);
        if sending.is_empty() {
            return None;
        }
        Some(
            sending
                .iter()
                .map(|cookie| format!("{}={}", cookie.name, cookie.value))
                .collect::<Vec<_>>()
                .join("; "),
        )
    }

    /// Which cookies this request may carry.
    ///
    /// Longest path first, which is what servers expect when two cookies share
    /// a name at different depths.
    pub fn for_request(
        &self,
        target: &Url,
        within: &Partition,
        how: How,
        now: SystemTime,
    ) -> Vec<&Cookie> {
        let Some(host) = target.host.as_ref().map(ToString::to_string) else {
            return Vec::new();
        };
        let secure = target.scheme == "https";
        // The site boundary. `Partition::of` is the host today, which is
        // stricter than the registrable domain a public suffix list would give
        // (queue item 156) — and stricter is the safe direction.
        let same_site = Partition::of(target) == *within;

        let mut sending: Vec<&Cookie> = self
            .held
            .values()
            .filter(|cookie| {
                if cookie.partition != *within {
                    return false;
                }
                if cookie.has_expired(now) {
                    return false;
                }
                if cookie.secure && !secure {
                    return false;
                }
                if cookie.covers_subdomains {
                    if !covers(&cookie.domain, &host) {
                        return false;
                    }
                } else if cookie.domain != host {
                    return false;
                }
                if !path_applies(&cookie.path, &target.path) {
                    return false;
                }
                may_cross(cookie.same_site, same_site, how)
            })
            .collect();
        sending.sort_by(|one, two| {
            two.path
                .len()
                .cmp(&one.path.len())
                .then_with(|| one.name.cmp(&two.name))
        });
        sending
    }

    /// Forget everything belonging to one top-level site.
    ///
    /// What "clear this site's data" has to mean once cookies are partitioned:
    /// not only the cookies that site set, but every cookie anybody set *inside*
    /// it. Unpartitioned, that second set was unreachable, which is one more
    /// thing partitioning makes possible rather than merely safer.
    pub fn forget_site(&mut self, site: &Partition) {
        self.held.retain(|(partition, ..), _| partition != site);
    }

    /// Drop everything that lasts only as long as a session.
    pub fn end_the_session(&mut self) {
        self.held
            .retain(|_, cookie| !cookie.is_for_this_session_only());
    }

    fn forget_the_expired(&mut self, now: SystemTime) {
        self.held.retain(|_, cookie| !cookie.has_expired(now));
    }

    /// Keep the bounds, dropping the shortest-lived first.
    ///
    /// A session cookie goes before one with an expiry, because a site that
    /// asked for something to last has said it matters more than something it
    /// did not.
    fn stay_within_bounds(&mut self, crowded: &Partition) {
        let mut here: Vec<_> = self
            .held
            .keys()
            .filter(|(partition, ..)| partition == crowded)
            .cloned()
            .collect();
        while here.len() > MOST_PER_SITE {
            if let Some(key) = self.least_worth_keeping(&here) {
                self.held.remove(&key);
                here.retain(|held| held != &key);
            } else {
                break;
            }
        }
        while self.held.len() > MOST_IN_ALL {
            let all: Vec<_> = self.held.keys().cloned().collect();
            if let Some(key) = self.least_worth_keeping(&all) {
                self.held.remove(&key);
            } else {
                break;
            }
        }
    }

    fn least_worth_keeping(
        &self,
        among: &[(Partition, String, String, String)],
    ) -> Option<(Partition, String, String, String)> {
        among
            .iter()
            .filter_map(|key| self.held.get(key).map(|cookie| (key, cookie)))
            .min_by_key(|(_, cookie)| cookie.expires.unwrap_or(SystemTime::UNIX_EPOCH))
            .map(|(key, _)| key.clone())
    }
}

/// Whether a cookie may go on a request that crosses a site boundary.
///
/// The whole of `SameSite` in five lines, and the middle case is the one that
/// removes a class of CSRF: a `Lax` cookie goes on a **navigation** from
/// another site — clicking a link to your bank — but not on anything a page
/// *embedded*, which is what a form post or an image request from an attacker's
/// page would be.
fn may_cross(rule: SameSite, same_site: bool, how: How) -> bool {
    if same_site {
        return true;
    }
    match rule {
        SameSite::Strict => false,
        SameSite::Lax => how == How::Navigated,
        SameSite::None => true,
    }
}
