/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whose bucket a piece of storage is in.
//!
//! ADR 0025 § 1: a bucket is keyed by **the origin of the document** using it
//! and **the top-level site** it sits under, as ADR 0007's [`Partition`] — the
//! same type the cookie jar and the cache use, so that there is one answer to
//! *what is a site* and storage can never disagree with cookies about where a
//! boundary is.
//!
//! # A key cannot be made for an opaque origin
//!
//! *"An opaque origin has no storage at all."* That is every `file:`, `data:`
//! and `about:` document under `alo_url::Origin::of`. It is enforced by the
//! type rather than by a check somewhere downstream: [`StorageKey::of`] answers
//! [`None`] for one, and nothing in this crate accepts anything but a
//! [`StorageKey`]. A store that cannot be handed an opaque origin cannot be
//! talked into keeping something for one.
//!
//! The ADR names the trap next to it: [`Partition::of`] gives every hostless
//! top-level URL the one partition `"opaque"`. A key here always carries the
//! document's own tuple origin beside its partition, so that shared value is
//! never a key on its own.

use alo_net::Partition;
use alo_url::Origin;
use core::fmt;

/// The origin and top-level site a bucket belongs to.
///
/// Ordered and hashable so a store can file buckets by it. Its parts are text
/// because that is what is written to a disk and read back, and because two
/// keys are the same key exactly when their text is.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StorageKey {
    /// The document's origin, serialised: `https://example.com`, with the port
    /// only when it is not the scheme's own. Never `null`.
    origin: String,
    /// The site that origin belongs to, which is what clearing a site's data
    /// is asked by (ADR 0025 § 8).
    site: String,
    /// The top-level site it is under.
    partition: String,
}

impl StorageKey {
    /// The key for a document of this origin under this top-level site.
    ///
    /// [`None`] for an opaque origin, which has no storage.
    pub fn of(origin: &Origin, partition: &Partition) -> Option<Self> {
        match origin {
            Origin::Opaque(_) => None,
            Origin::Tuple { host, .. } => Some(Self {
                origin: origin.to_string(),
                site: alo_url::site::of(host),
                partition: partition.site().to_owned(),
            }),
        }
    }

    /// A key as it was written to a disk, or [`None`] when it is not one this
    /// crate could have made.
    ///
    /// The parts come from a file and are believed only this far: none is
    /// empty, and the origin is not the serialisation of an opaque one. That a
    /// key names the bucket it was found in is checked by the directory, which
    /// knows where it was found.
    ///
    /// Public because storage is not the only thing filed by a storage key:
    /// `alo-grants` keeps a person's permissions under the same key (ADR 0026
    /// § 2) and reads its own file back the same way, as a stranger's.
    pub fn from_parts(origin: String, site: String, partition: String) -> Option<Self> {
        if origin.is_empty() || origin == "null" || site.is_empty() || partition.is_empty() {
            return None;
        }
        Some(Self {
            origin,
            site,
            partition,
        })
    }

    /// The document's origin, serialised.
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// The site the origin belongs to.
    pub fn site(&self) -> &str {
        &self.site
    }

    /// The top-level site the bucket is under.
    pub fn partition(&self) -> &str {
        &self.partition
    }
}

impl fmt::Display for StorageKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} under {}", self.origin, self.partition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> alo_url::Url {
        alo_url::parse(text).expect("a URL")
    }

    fn key(document: &str, top_level: &str) -> Option<StorageKey> {
        StorageKey::of(&Origin::of(&url(document)), &Partition::of(&url(top_level)))
    }

    #[test]
    fn an_opaque_origin_has_no_key_at_all() {
        for document in [
            "file:///Users/someone/page.html",
            "data:text/html,<p>hello",
            "about:blank",
        ] {
            assert_eq!(
                key(document, "https://news.example/"),
                None,
                "{document} was given storage"
            );
        }
    }

    #[test]
    fn the_same_origin_under_two_top_level_sites_is_two_buckets() {
        let alone = key("https://widget.example/", "https://widget.example/").expect("a key");
        let in_news = key("https://widget.example/", "https://news.example/").expect("a key");
        let in_shop = key("https://widget.example/", "https://shop.example/").expect("a key");
        assert_ne!(alone, in_news);
        assert_ne!(in_news, in_shop);
        assert_eq!(in_news.origin(), in_shop.origin());
        assert_eq!(in_news.site(), "widget.example");
        assert_eq!(in_news.partition(), "news.example");
    }

    #[test]
    fn an_origin_is_scheme_host_and_port_and_its_site_is_the_registrable_domain() {
        let key = key("https://a.example.com:8443/x", "https://a.example.com/").expect("a key");
        assert_eq!(key.origin(), "https://a.example.com:8443");
        assert_eq!(key.site(), "example.com");
        assert_eq!(key.partition(), "example.com");
        assert_ne!(
            Some(key),
            self::key("http://a.example.com:8443/x", "https://a.example.com/"),
            "http and https were one origin"
        );
    }

    /// The partition every hostless top-level URL shares is never a key alone:
    /// the document's own origin is always beside it.
    #[test]
    fn a_frame_under_a_hostless_top_level_page_is_keyed_by_its_own_origin() {
        let one = key("https://a.example/", "file:///page.html").expect("a key");
        let two = key("https://b.example/", "file:///page.html").expect("a key");
        assert_eq!(one.partition(), "opaque");
        assert_ne!(one, two);
    }

    #[test]
    fn a_key_read_back_from_a_disk_is_refused_when_no_document_could_have_made_it() {
        let made = |origin: &str, site: &str, partition: &str| {
            StorageKey::from_parts(origin.to_owned(), site.to_owned(), partition.to_owned())
        };
        assert!(made("https://example.com", "example.com", "example.com").is_some());
        assert_eq!(made("", "example.com", "example.com"), None);
        assert_eq!(made("null", "example.com", "example.com"), None);
        assert_eq!(made("https://example.com", "", "example.com"), None);
        assert_eq!(made("https://example.com", "example.com", ""), None);
    }
}
