//! Which site a page belongs to, and therefore which process renders it.
//!
//! # The one thing to be honest about here
//!
//! ADR 0005 says a site is *"the scheme and the registrable domain, so two tabs
//! on the same site share a process and two sites never do."* The registrable
//! domain needs a public suffix list, which this engine does not have yet —
//! queue item 156.
//!
//! So today a site is the scheme and the **host**. That is **stricter**, which
//! is the safe direction: `a.example.com` and `b.example.com` get separate
//! processes where they should share one. It costs memory and it never puts two
//! sites together.
//!
//! It is said out loud here rather than quietly using the host, because the
//! failure in the other direction — two sites sharing a process because we
//! could not tell them apart — is precisely what this whole structure exists to
//! prevent, and somebody adding the public suffix list needs to find this note
//! rather than discover the assumption.

use core::fmt;

/// A site, as this engine can currently tell them apart.
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
                .map_or_else(String::new, |host| host.to_string().to_ascii_lowercase()),
        }
    }

    /// The scheme.
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    /// The host, which is standing in for the registrable domain until item
    /// 156.
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
