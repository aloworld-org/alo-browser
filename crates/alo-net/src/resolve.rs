/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Turning a name into an address, and refusing the answers that are attacks.
//!
//! ADR 0008 is the decision behind this file. Two of its rules are code:
//!
//! **The resolver is the machine's.** Nothing here opens a socket to a DNS
//! server or speaks the protocol. It calls what the operating system was
//! configured to call, which is where a VPN, a corporate network's internal
//! names, a Pi-hole, `/etc/hosts` and the machine's own encrypted DNS already
//! live. A browser that resolved its own way would break all five, invisibly.
//!
//! **A public name that resolves to a private address is refused.** That is DNS
//! rebinding, and it is the one rule here with an attacker behind it.
//!
//! # What this is not
//!
//! It is not a DNS client, so it cannot see a record's TTL — the platform does
//! not hand one back. What is cached here is therefore held for a fixed short
//! time, and that is a guess rather than an answer. A cache that honours real
//! TTLs needs a resolver of our own, which is a decision ADR 0008 deliberately
//! did not take.

use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};

/// How long a resolved name is reused for.
///
/// A guess, and named as one: the platform resolver does not return the record's
/// TTL, so there is nothing truthful to use. Half a minute is short enough that
/// a deployment moving hosts is not broken for long, and long enough that the
/// thirty requests one page makes do not resolve thirty times.
pub const REUSE_FOR: Duration = Duration::from_secs(30);

/// How far a request is allowed to reach.
///
/// This is the rebinding rule, and it is about **who asked** rather than about
/// the address alone. A person typing an intranet name into the address bar
/// should reach it; a public web page causing a request to `192.168.1.1` should
/// not, and the two are indistinguishable if you only look at where it resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// Anywhere. The person asked for this directly, or whatever asked is
    /// itself local.
    Anywhere,
    /// The public internet only. Something on the public web caused this, and
    /// it must not become a way into the network the browser is sitting on.
    PublicOnly,
}

/// Why a name did not become an address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unresolved {
    /// The resolver had no answer.
    NoSuchName {
        /// The name.
        host: String,
        /// What the platform said.
        why: String,
    },
    /// The resolver answered, and every answer was one this browser will not
    /// use.
    ///
    /// Named separately from [`Unresolved::NoSuchName`] because they are
    /// different things to tell somebody, and because this one is the shape of
    /// an attack rather than of a typo.
    RefusedTheAnswer {
        /// The name.
        host: String,
        /// What it resolved to.
        address: IpAddr,
    },
}

impl fmt::Display for Unresolved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unresolved::NoSuchName { host, why } => write!(f, "could not find {host}: {why}"),
            Unresolved::RefusedTheAnswer { host, address } => write!(
                f,
                "{host} resolved to {address}, a private address, and a page on the \
                 public web must not be able to reach one"
            ),
        }
    }
}

impl std::error::Error for Unresolved {}

/// What was resolved, and when.
#[derive(Debug, Clone)]
struct Remembered {
    addresses: Vec<SocketAddr>,
    at: Instant,
}

/// Names, turned into addresses by the machine's own resolver.
#[derive(Debug, Default)]
pub struct Resolver {
    remembered: HashMap<(String, u16), Remembered>,
    asked: usize,
    reused: usize,
}

impl Resolver {
    /// A resolver that remembers nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many names were looked up, and how many were answered from memory.
    pub fn counts(&self) -> (usize, usize) {
        (self.asked, self.reused)
    }

    /// Where this name and port are, as far as this browser will go.
    ///
    /// # Errors
    ///
    /// [`Unresolved`] when the machine has no answer, or when every answer is
    /// one a page on the public web must not be allowed to reach.
    pub fn resolve(
        &mut self,
        host: &str,
        port: u16,
        reach: Reach,
    ) -> Result<Vec<SocketAddr>, Unresolved> {
        let found = self.look_up(host, port)?;
        keep_only_what_may_be_reached(host, found, reach)
    }

    /// The lookup, cached, before any rule is applied.
    ///
    /// The rule is applied afterwards rather than here so that one name looked
    /// up twice with different reaches is one lookup — and so that a cached
    /// answer can never smuggle a permission it was granted the first time.
    fn look_up(&mut self, host: &str, port: u16) -> Result<Vec<SocketAddr>, Unresolved> {
        let key = (host.to_ascii_lowercase(), port);
        if let Some(known) = self.remembered.get(&key) {
            if known.at.elapsed() < REUSE_FOR {
                self.reused += 1;
                return Ok(known.addresses.clone());
            }
        }
        self.asked += 1;
        let addresses: Vec<SocketAddr> = (host, port)
            .to_socket_addrs()
            .map_err(|why| Unresolved::NoSuchName {
                host: host.to_owned(),
                why: why.to_string(),
            })?
            .collect();
        if addresses.is_empty() {
            return Err(Unresolved::NoSuchName {
                host: host.to_owned(),
                why: "the resolver returned no addresses".to_owned(),
            });
        }
        self.remembered.insert(
            key,
            Remembered {
                addresses: addresses.clone(),
                at: Instant::now(),
            },
        );
        Ok(addresses)
    }

    /// Forget everything, which is what a network change means.
    pub fn forget(&mut self) {
        self.remembered.clear();
    }
}

/// Apply the rebinding rule to a set of answers.
///
/// Every private answer is dropped rather than the whole lookup being refused:
/// a name that resolves to both a public and a private address is a name we can
/// still reach, at the public one. Refusing outright would break real hosts
/// whose resolver returns an internal address alongside an external one.
///
/// # Errors
///
/// [`Unresolved::RefusedTheAnswer`] when nothing is left, naming the first
/// address that was refused — which is the one somebody investigating wants.
fn keep_only_what_may_be_reached(
    host: &str,
    found: Vec<SocketAddr>,
    reach: Reach,
) -> Result<Vec<SocketAddr>, Unresolved> {
    if reach == Reach::Anywhere {
        return Ok(found);
    }
    let refused = found
        .iter()
        .find(|address| !is_public(address.ip()))
        .map(SocketAddr::ip);
    let allowed: Vec<SocketAddr> = found
        .into_iter()
        .filter(|address| is_public(address.ip()))
        .collect();
    if allowed.is_empty() {
        return Err(Unresolved::RefusedTheAnswer {
            host: host.to_owned(),
            address: refused.unwrap_or(IpAddr::from([0, 0, 0, 0])),
        });
    }
    Ok(allowed)
}

/// Whether an address is one on the public internet.
///
/// Written out rather than taken from the standard library, because the methods
/// that would cover it — `is_global`, `is_shared`, `is_benchmarking` — are not
/// stable, and because a security rule spelled out is a security rule somebody
/// can check.
pub fn is_public(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(four) => {
            let [a, b, ..] = four.octets();
            !(four.is_loopback()
                || four.is_private()
                || four.is_link_local()
                || four.is_unspecified()
                || four.is_broadcast()
                || four.is_documentation()
                || four.is_multicast()
                // 100.64.0.0/10 — carrier-grade NAT, which is where an ISP puts
                // its customers. Reachable, and not the public internet.
                || (a == 100 && (64..128).contains(&b))
                // 192.0.0.0/24 — protocol assignments.
                || (a == 192 && b == 0 && four.octets().get(2) == Some(&0))
                // 198.18.0.0/15 — benchmarking.
                || (a == 198 && (18..20).contains(&b))
                // 240.0.0.0/4 — reserved, and 255.255.255.255 with it.
                || a >= 240)
        }
        IpAddr::V6(six) => {
            let first = six.segments().first().copied().unwrap_or(0);
            !(six.is_loopback()
                || six.is_unspecified()
                || six.is_multicast()
                // fc00::/7 — unique local, the v6 equivalent of 10.0.0.0/8.
                || (first & 0xfe00) == 0xfc00
                // fe80::/10 — link local.
                || (first & 0xffc0) == 0xfe80
                // ::ffff:0:0/96 — a v4 address wearing a v6 hat, which must be
                // judged as the v4 address it is. Without this, `::ffff:127.0.0.1`
                // walks straight past every check above.
                || six
                    .to_ipv4_mapped()
                    .is_some_and(|four| !is_public(IpAddr::V4(four))))
        }
    }
}

/// How far a request caused by this initiator may reach.
///
/// Nobody asking means the person did — a typed address, a bookmark — and that
/// may go anywhere. A page on the public web may not. A page that is *itself*
/// local may, because it is already inside whatever it would be reaching into.
pub fn reach_for(initiator: Option<&alo_url::Origin>) -> Reach {
    let Some(origin) = initiator else {
        return Reach::Anywhere;
    };
    match origin {
        alo_url::Origin::Tuple { host, .. } => {
            if host_is_local(&host.to_string()) {
                Reach::Anywhere
            } else {
                Reach::PublicOnly
            }
        }
        // An opaque origin has no host to judge, and the safe reading of "we
        // cannot tell who this is" is the restrictive one. `file:` is opaque
        // (ADR-adjacent, see `Origin::of`), so a local file cannot become a way
        // into a network either.
        alo_url::Origin::Opaque(_) => Reach::PublicOnly,
    }
}

/// Whether a host names something on this machine or its own network.
fn host_is_local(host: &str) -> bool {
    let host = host.trim_matches(['[', ']']).to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") {
        return true;
    }
    host.parse::<IpAddr>()
        .is_ok_and(|address| !is_public(address))
}
