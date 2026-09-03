//! What we tell a site about where you came from.
//!
//! # Why the default is not "everything"
//!
//! `Referer` was designed to be helpful and became a leak. A full URL carries
//! the path and the query, and a great many paths and queries are the message:
//! `/reset-password?token=…`, `/documents/the-redundancies-list`,
//! `/search?q=…`. Sending all of that to every image host on a page hands it to
//! people who never asked for it and cannot be expected to protect it.
//!
//! So the default here is the modern browser default:
//! **`strict-origin-when-cross-origin`** — your own site gets the whole URL,
//! anybody else gets the origin, and a downgrade to `http` gets nothing.
//!
//! # The one rule that holds under every policy
//!
//! **A referrer never survives a downgrade.** Going from `https` to `http`,
//! whatever the page asked for, sends nothing — because the thing we would be
//! sending is exactly what an attacker on that connection is there to read.
//! `unsafe-url` is the only policy that overrides it, and it is named that way
//! on purpose.

use alo_url::{Origin, Url};

/// What a site asked us to send.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Policy {
    /// Nothing, ever.
    NoReferrer,
    /// The full URL, except on a downgrade. The old default, and it leaks paths.
    NoReferrerWhenDowngrade,
    /// The origin only, always.
    Origin,
    /// The full URL to ourselves, the origin to anybody else.
    OriginWhenCrossOrigin,
    /// The full URL to ourselves, nothing to anybody else.
    SameOrigin,
    /// The origin only, and nothing on a downgrade.
    StrictOrigin,
    /// The full URL to ourselves, the origin to anybody else, nothing on a
    /// downgrade. What browsers settled on, and the default here.
    #[default]
    StrictOriginWhenCrossOrigin,
    /// Everything, everywhere. Named for what it is.
    UnsafeUrl,
}

impl Policy {
    /// What a `Referrer-Policy` header said, or [`None`] for a value nobody
    /// knows.
    ///
    /// A policy we cannot read leaves the default in place rather than
    /// weakening it — a rule this engine applies to every policy header it
    /// reads, because the alternative is that a typo removes a protection.
    pub fn named(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "no-referrer" => Some(Policy::NoReferrer),
            "no-referrer-when-downgrade" => Some(Policy::NoReferrerWhenDowngrade),
            "origin" => Some(Policy::Origin),
            "origin-when-cross-origin" => Some(Policy::OriginWhenCrossOrigin),
            "same-origin" => Some(Policy::SameOrigin),
            "strict-origin" => Some(Policy::StrictOrigin),
            "strict-origin-when-cross-origin" => Some(Policy::StrictOriginWhenCrossOrigin),
            "unsafe-url" => Some(Policy::UnsafeUrl),
            _ => None,
        }
    }

    /// The last policy in a list that this engine understands.
    ///
    /// A site may send several, and the specification says to take the last one
    /// that is known — which is how a site offers a strict policy to browsers
    /// that have it and a weaker one to those that do not.
    pub fn from_header(value: &str) -> Option<Self> {
        value.split(',').filter_map(Policy::named).next_back()
    }
}

/// What to send as `Referer` when going from `from` to `to`.
///
/// [`None`] means send nothing, which is different from sending an empty one.
pub fn for_request(policy: Policy, from: &Url, to: &Url) -> Option<String> {
    // Never from a page that is not on the web to begin with. There is no
    // useful thing to say and several harmful ones.
    if !matches!(from.scheme.as_str(), "http" | "https") {
        return None;
    }
    let same_origin = Origin::of(from) == Origin::of(to) && !Origin::of(from).is_opaque();
    let downgrading = from.scheme == "https" && to.scheme != "https";

    match policy {
        Policy::NoReferrer => None,
        Policy::UnsafeUrl => Some(stripped(from)),
        Policy::SameOrigin => {
            if same_origin {
                Some(stripped(from))
            } else {
                None
            }
        }
        Policy::Origin => Some(Origin::of(from).to_string()),
        Policy::StrictOrigin => {
            if downgrading {
                None
            } else {
                Some(Origin::of(from).to_string())
            }
        }
        Policy::NoReferrerWhenDowngrade => {
            if downgrading {
                None
            } else {
                Some(stripped(from))
            }
        }
        Policy::OriginWhenCrossOrigin => {
            if same_origin {
                Some(stripped(from))
            } else {
                Some(Origin::of(from).to_string())
            }
        }
        Policy::StrictOriginWhenCrossOrigin => {
            if downgrading {
                None
            } else if same_origin {
                Some(stripped(from))
            } else {
                Some(Origin::of(from).to_string())
            }
        }
    }
}

/// A URL with the parts that must never be sent taken off.
///
/// The fragment, because it is not sent to a server at all and was never the
/// other site's business. Credentials in the URL, because a `Referer` that
/// carried a password would be a password in every server log it passed.
fn stripped(url: &Url) -> String {
    let mut out = format!("{}://", url.scheme);
    if let Some(host) = &url.host {
        out.push_str(&host.to_string());
        if let Some(port) = url.port {
            out.push(':');
            out.push_str(&port.to_string());
        }
    }
    out.push_str(if url.path.is_empty() { "/" } else { &url.path });
    if let Some(query) = &url.query {
        out.push('?');
        out.push_str(query);
    }
    out
}
