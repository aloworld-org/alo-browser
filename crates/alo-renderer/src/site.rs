/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which site a page belongs to, and therefore which process renders it.
//!
//! ADR 0005: a site is *"the scheme and the registrable domain, so two tabs on
//! the same site share a process and two sites never do."* Both halves are here
//! now — the registrable domain is `alo_url::site`'s answer, decided against the
//! public suffix list (queue item 156), and the scheme is kept beside it because
//! `http://example.com` and `https://example.com` are two sites however alike
//! the host is.
//!
//! # What changed when the list arrived, and which direction it moved
//!
//! Until item 156 a site was the scheme and the **host**, which was stricter:
//! `a.example.com` and `b.example.com` got separate processes where they should
//! share one, which cost memory and never put two sites together. It is looser
//! now, and deliberately — one process for one registrable domain is what
//! ADR 0005 decided and what the isolation is sized for. The failure in the
//! other direction, two *sites* sharing a process because we could not tell
//! them apart, is what the list makes impossible rather than unlikely:
//! `bbc.co.uk` and `gov.co.uk` share a suffix and are not one site, and no
//! comparison of host strings could have said so.

use core::fmt;

/// A site: the scheme and the registrable domain.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Site {
    scheme: String,
    host: String,
}

impl Site {
    /// The site a URL belongs to.
    ///
    /// A URL with no host — `data:`, `about:` — gets a site of its own, named
    /// after the scheme, because those pages are not one another's origin
    /// either (see `Origin::of`).
    pub fn of(url: &alo_url::Url) -> Self {
        Self {
            scheme: url.scheme.to_ascii_lowercase(),
            host: url
                .host
                .as_ref()
                .map_or_else(String::new, alo_url::site::of),
        }
    }

    /// The scheme.
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    /// The registrable domain, or the host itself where there is none — an
    /// address, or a name that is a public suffix.
    pub fn host(&self) -> &str {
        &self.host
    }
}

impl fmt::Display for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.host.is_empty() {
            write!(f, "{}:", self.scheme)
        } else {
            write!(f, "{}://{}", self.scheme, self.host)
        }
    }
}
