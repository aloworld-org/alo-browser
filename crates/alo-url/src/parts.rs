/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A URL, in parts.
//!
//! The parts rather than a string with indices into it, because everything
//! that reads a URL reads *one* of them: the network wants the host and the
//! port, the cache wants the whole of it, the same-origin policy wants three
//! of them and nothing else, and a page's own link wants what it was written
//! as. A type that answered all of those with `&str` slices of one buffer
//! would be a type every caller had to be careful with.

use core::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};

/// What a URL names as its host.
///
/// Three kinds and not one string, because they compare differently and a
/// browser gets that wrong at its peril: `127.0.0.1` and `127.1` are the same
/// address written twice, and `[::1]` is not a domain called `::1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Host {
    /// A domain name, **already in its ASCII form**. See [`crate::parse`] for
    /// why it is never held as the Unicode the author typed.
    Domain(String),
    /// An IPv4 address.
    Ipv4(Ipv4Addr),
    /// An IPv6 address.
    Ipv6(Ipv6Addr),
}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Host::Domain(name) => f.write_str(name),
            Host::Ipv4(address) => write!(f, "{address}"),
            // The brackets are part of how an IPv6 host is written in a URL,
            // and leaving them off produces something that does not parse
            // back — which is how a serialise-and-reparse loop loses a host.
            Host::Ipv6(address) => write!(f, "[{address}]"),
        }
    }
}

/// A URL.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Url {
    /// The scheme, lowercased: `https`, `file`, `data`.
    pub scheme: String,
    /// The host, for the schemes that have one. `data:` and `about:` do not.
    pub host: Option<Host>,
    /// The port, **only when it was written and is not the default**.
    ///
    /// `https://example.com:443` and `https://example.com` are the same URL,
    /// and a port kept here for the first would make them compare differently.
    pub port: Option<u16>,
    /// The path, as it will be sent.
    pub path: String,
    /// What followed `?`, without the `?`.
    pub query: Option<String>,
    /// What followed `#`, without the `#`.
    ///
    /// **Never sent anywhere.** It is the page's own business, and a fragment
    /// that reached a server would be a leak rather than a bug.
    pub fragment: Option<String>,
    /// The whole of it, as this engine would write it back out.
    pub serialised: String,
}

impl Url {
    /// The port to actually connect to: the one written, or the scheme's own.
    ///
    /// [`None`] for a scheme with no notion of a port, which is what says
    /// `data:` is not something to open a socket for.
    pub fn effective_port(&self) -> Option<u16> {
        self.port.or_else(|| default_port(&self.scheme))
    }

    /// Whether this scheme is one the web's security rules apply to.
    ///
    /// WHATWG calls these *special*. It is not a list of what this engine can
    /// fetch — that is the network's business — but of which schemes have a
    /// host, a port and an origin worth comparing.
    pub fn is_special(&self) -> bool {
        default_port(&self.scheme).is_some() || self.scheme == "file"
    }
}

/// The port a scheme uses when nobody wrote one.
pub fn default_port(scheme: &str) -> Option<u16> {
    match scheme {
        "http" | "ws" => Some(80),
        "https" | "wss" => Some(443),
        "ftp" => Some(21),
        _ => None,
    }
}

impl fmt::Display for Url {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.serialised)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scheme_that_has_a_port_says_so_and_one_that_does_not_says_that() {
        assert_eq!(default_port("http"), Some(80));
        assert_eq!(default_port("https"), Some(443));
        assert_eq!(default_port("ws"), Some(80));
        assert_eq!(default_port("wss"), Some(443));
        assert_eq!(default_port("data"), None);
        assert_eq!(default_port("about"), None);
        // `file` has no port and is still special, which is the one place the
        // two questions come apart.
        assert_eq!(default_port("file"), None);
    }

    #[test]
    fn an_ipv6_host_writes_its_brackets_and_a_domain_does_not() {
        assert_eq!(
            Host::Domain("example.com".to_owned()).to_string(),
            "example.com"
        );
        assert_eq!(Host::Ipv4(Ipv4Addr::LOCALHOST).to_string(), "127.0.0.1",);
        assert_eq!(
            Host::Ipv6(Ipv6Addr::LOCALHOST).to_string(),
            "[::1]",
            "without the brackets it does not parse back",
        );
    }

    #[test]
    fn two_kinds_of_host_that_read_alike_are_not_the_same_host() {
        assert_ne!(
            Host::Domain("127.0.0.1".to_owned()),
            Host::Ipv4(Ipv4Addr::LOCALHOST),
            "a domain that looks like an address is not one",
        );
    }
}
