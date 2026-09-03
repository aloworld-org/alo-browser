/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a secure page may load insecurely, which is almost nothing.
//!
//! # Why this is not just "block http on an https page"
//!
//! Because the answer differs by **what the thing is**, and the difference is
//! about what an attacker who could replace it would get:
//!
//! - A **script** or a **stylesheet** replaced in transit runs as the page. The
//!   attacker is not looking at the page; they *are* the page. Nothing recovers
//!   from that, so it is refused outright and no fallback is offered.
//! - An **image** replaced in transit is a wrong picture. Bad, and not the same
//!   thing. Those are tried again over TLS first, because a great many sites
//!   have an `http://` URL in their markup and a perfectly good `https://`
//!   server, and blocking them would break pages for nothing.
//!
//! # And one exception that is not a loophole
//!
//! `http://localhost` is secure. Not by convention — there is no network
//! between the two ends, so there is nothing in between to attack. Refusing it
//! would break every developer on earth while protecting nobody.

use crate::request::{Purpose, Request};
use crate::resolve::is_public;
use alo_url::{Origin, Url};
use std::net::IpAddr;

/// What to do about a subresource on an insecure connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Nothing is wrong: the page is insecure too, or the target is.
    Fine,
    /// Try it over TLS instead. If that fails it is refused rather than
    /// retried insecurely — a fallback that fell back would be no rule at all.
    TryItSecurely {
        /// The same URL, over `https`.
        instead: Url,
    },
    /// Refused, with nothing offered.
    Refused {
        /// What it was.
        what: String,
    },
}

/// Whether an origin is one an attacker cannot get between us and.
///
/// `https` obviously. `http://localhost` too, and that is the interesting case:
/// there is no network between the two ends, so there is nothing in between to
/// attack.
pub fn is_trustworthy(url: &Url) -> bool {
    match url.scheme.as_str() {
        // `file:` is in the list because a local file came from the disk
        // rather than from a network, so there was nothing in between. It is
        // still an opaque *origin* (see `Origin::of`), which is a different
        // question from whether it was tampered with in transit.
        "https" | "wss" | "data" | "about" | "blob" | "file" => true,
        "http" | "ws" => url.host.as_ref().is_some_and(|host| {
            let host = host.to_string();
            let bare = host.trim_matches(['[', ']']).to_ascii_lowercase();
            if bare == "localhost" || bare.ends_with(".localhost") {
                return true;
            }
            bare.parse::<IpAddr>()
                .is_ok_and(|address| address.is_loopback() || !is_public(address))
        }),
        _ => false,
    }
}

/// What to do about this request, given the page that made it.
pub fn what_to_do(request: &Request) -> Verdict {
    let Some(asker) = &request.initiator else {
        // Nobody asked, so this is a person going somewhere rather than a
        // secure page reaching for something. A person may visit an insecure
        // site; that is their business and their address bar.
        return Verdict::Fine;
    };
    if !is_a_secure_page(asker) {
        // An insecure page loading insecure things is not *mixed*. It is
        // consistently bad, and it is the page's own problem rather than a
        // promise being broken.
        return Verdict::Fine;
    }
    if is_trustworthy(&request.url) {
        return Verdict::Fine;
    }
    match request.purpose {
        // Replaced in transit, these *are* the page. A violation report is
        // refused for the other reason: it carries the URLs a secure page was
        // refused, and sending that in clear would hand it to exactly the
        // attacker the policy was written about.
        Purpose::Script | Purpose::Style | Purpose::Fetch | Purpose::Document | Purpose::Report => {
            Verdict::Refused {
                what: request.purpose.to_string(),
            }
        }
        // Replaced in transit, this is a wrong picture.
        Purpose::Image => match secured(&request.url) {
            Some(instead) => Verdict::TryItSecurely { instead },
            None => Verdict::Refused {
                what: request.purpose.to_string(),
            },
        },
    }
}

/// Whether the page that asked was itself reached securely.
fn is_a_secure_page(asker: &Origin) -> bool {
    match asker {
        Origin::Tuple { scheme, host, .. } => {
            if scheme == "https" || scheme == "wss" {
                return true;
            }
            let bare = host.to_string().to_ascii_lowercase();
            bare == "localhost"
                || bare.ends_with(".localhost")
                || bare
                    .parse::<IpAddr>()
                    .is_ok_and(|address| address.is_loopback())
        }
        // An opaque origin has no scheme to judge, and the safe reading is that
        // it is not secure — so its insecure loads are its own business.
        Origin::Opaque(_) => false,
    }
}

/// The same URL over TLS, if that is a thing it could be.
fn secured(url: &Url) -> Option<Url> {
    if url.scheme != "http" {
        return None;
    }
    let text = url.serialised.replacen("http://", "https://", 1);
    alo_url::parse(&text).ok()
}
