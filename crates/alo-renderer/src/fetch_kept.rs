/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The browser process's count of what each document's keep-alive requests
//! have in flight (ADR 0040 § 4, queue item 369).
//!
//! A page may have at most [`MOST_KEPT_ALIVE`] bytes of keep-alive body in
//! flight together, Fetch's own number. The renderer counts it too, so that
//! `sendBeacon` can answer `false` before it returns — but **the renderer's
//! count is a claim**, and a renderer that was taken over could mark every
//! ask keep-alive and leave behind as many reports as it liked. So this
//! process counts again, from its own decisions: a keep-alive request is
//! counted when it is decided and taken off when it is made, or when it is
//! not made at all. An ask that would take a document past the bound is
//! refused by a rule of its own ([`crate::fetch_decide::Rule::KeptAlive`]),
//! which an honest renderer never meets because it answered `false` first.
//!
//! **Per document, not per tab**: a beacon a page sends as it is left is
//! still counted after the tab shows the next page, until it is made. And a
//! document whose count has come back to nothing is forgotten, so the count
//! holds only the documents something is in flight for.

use std::collections::HashMap;

use alo_net::cause::DocumentId;

pub use alo_bindings::fetching::MOST_KEPT_ALIVE;

/// What each document's keep-alive requests have in flight, in bytes of
/// body.
#[derive(Debug, Clone, Default)]
pub struct KeptAlive {
    bytes: HashMap<DocumentId, usize>,
}

impl KeptAlive {
    /// The bytes `document` has in flight.
    pub fn of(&self, document: DocumentId) -> usize {
        self.bytes.get(&document).copied().unwrap_or(0)
    }

    /// Count `bytes` more for `document`, if that stays within
    /// [`MOST_KEPT_ALIVE`]. Whether it did; nothing is counted when it does
    /// not.
    pub fn claim(&mut self, document: DocumentId, bytes: usize) -> bool {
        let total = self.of(document).saturating_add(bytes);
        if total > MOST_KEPT_ALIVE {
            return false;
        }
        self.bytes.insert(document, total);
        true
    }

    /// Take `bytes` off `document`'s count: a request it claimed was made,
    /// or will not be.
    pub fn release(&mut self, document: DocumentId, bytes: usize) {
        let left = self.of(document).saturating_sub(bytes);
        if left == 0 {
            self.bytes.remove(&document);
        } else {
            self.bytes.insert(document, left);
        }
    }

    /// How many documents have something in flight.
    pub fn documents(&self) -> usize {
        self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use alo_net::cause::Identities;

    use super::*;

    #[test]
    fn a_document_is_bounded_by_fetchs_number_and_freed_as_each_is_made() {
        let mut identities = Identities::default();
        let (one, two) = (identities.a_document(), identities.a_document());
        let mut kept = KeptAlive::default();
        assert!(kept.claim(one, MOST_KEPT_ALIVE));
        assert!(!kept.claim(one, 1), "one byte past the bound");
        assert_eq!(kept.of(one), MOST_KEPT_ALIVE, "a refusal counts nothing");
        // Another document has its own.
        assert!(kept.claim(two, MOST_KEPT_ALIVE));
        assert!(!kept.claim(two, 1));
        kept.release(one, MOST_KEPT_ALIVE);
        assert_eq!(kept.of(one), 0);
        assert_eq!(kept.documents(), 1, "a document with nothing is forgotten");
        assert!(kept.claim(one, 1));
        // An empty body costs nothing here; the other bounds count it.
        assert!(kept.claim(two, 0));
    }

    #[test]
    fn a_hostile_size_is_refused_rather_than_wrapping() {
        let mut kept = KeptAlive::default();
        let document = Identities::default().a_document();
        assert!(kept.claim(document, 1));
        assert!(!kept.claim(document, usize::MAX));
        kept.release(document, usize::MAX);
        assert_eq!(kept.of(document), 0);
        assert_eq!(kept.documents(), 0);
    }
}
