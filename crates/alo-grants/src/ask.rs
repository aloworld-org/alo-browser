/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page asking, and the three things that can come of it.
//!
//! An [`Ask`] is the facts the browser process knows about one request for
//! one [`Capability`]: who asked, inside what, whether anybody was using the
//! page, and what caused it. The table turns it into a [`Decision`]:
//! **granted** because a person already said so, **refused without a prompt**
//! by a named [`Refusal`], or **a prompt** for a person to answer.
//!
//! # A prompt is a value only the table makes
//!
//! [`Prompted`] has no public constructor and is not `Clone`. The only way to
//! answer is to hold one, and the only way to hold one is for the table to
//! have decided that this ask may be put to a person. So an answer can never
//! be given to an ask §§ 2–3 refused, and one prompt cannot be answered twice.
//!
//! # What the page is told
//!
//! Nothing here reaches a page. ADR 0026 § 3 says what an API makes of a
//! refusal: an ask without a gesture receives what an ask that was not made
//! would, `"prompt"` from `permissions.query()` and `NotAllowedError` from the
//! call. [`Refusal::looks_unasked`] says which refusals are that one.

use crate::capability::Capability;
use alo_net::Cause;
use alo_net::cause::DocumentId;
use alo_storage::StorageKey;
use core::fmt;

/// One page asking for one capability, as the browser process knows it.
///
/// Every field is a fact the browser process holds. None is a renderer's
/// claim: a renderer that could say *this was a secure context* or *the
/// person had just clicked* could say it about anything (ADR 0005).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    /// Who asked, inside what. [`None`] for an opaque origin, which has no key
    /// and so can hold no grant (ADR 0026 § 2).
    pub key: Option<StorageKey>,
    /// What for.
    pub capability: Capability,
    /// The document that asked.
    pub document: DocumentId,
    /// Whether that document is a secure context.
    pub secure: bool,
    /// Whether every frame between the top-level page and the asker allowed
    /// the capability through Permissions Policy. A top-level document is
    /// trivially allowed; a frame needs `allow=` from its embedder, which is
    /// **necessary and not sufficient**.
    pub delegated: bool,
    /// Whether the document holds transient activation: a gesture moments
    /// before.
    pub activated: bool,
    /// What caused it, under ADR 0012. An ask made in answer to an agent's
    /// action is shown to the person, saying so.
    pub cause: Cause,
}

/// Why an ask was refused without a prompt, each one a rule of ADR 0026.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// § 2: an opaque origin has no storage key.
    OpaqueOrigin,
    /// § 2: only a secure context may ask.
    InsecureContext,
    /// § 2: the embedding page did not allow the capability to this frame.
    NotDelegated,
    /// § 5: the person refused this capability for every site.
    RefusedEverywhere,
    /// § 5: the person answered *don't allow* to this key, and it has not
    /// ended.
    RememberedRefusal,
    /// § 3: this document was refused, or dismissed a prompt, for this
    /// capability already.
    RefusedThisDocument,
    /// § 3: nobody was using the page.
    NoGesture,
}

impl Refusal {
    /// The byte it is written as.
    pub fn tag(self) -> u8 {
        match self {
            Refusal::OpaqueOrigin => 1,
            Refusal::InsecureContext => 2,
            Refusal::NotDelegated => 3,
            Refusal::RefusedEverywhere => 4,
            Refusal::RememberedRefusal => 5,
            Refusal::RefusedThisDocument => 6,
            Refusal::NoGesture => 7,
        }
    }

    /// The refusal a byte names, or [`None`].
    pub fn from_tag(tag: u8) -> Option<Self> {
        [
            Refusal::OpaqueOrigin,
            Refusal::InsecureContext,
            Refusal::NotDelegated,
            Refusal::RefusedEverywhere,
            Refusal::RememberedRefusal,
            Refusal::RefusedThisDocument,
            Refusal::NoGesture,
        ]
        .into_iter()
        .find(|refusal| refusal.tag() == tag)
    }

    /// Whether the page is answered as though it had not asked at all
    /// (ADR 0026 § 3), rather than told *denied*.
    pub fn looks_unasked(self) -> bool {
        matches!(self, Refusal::NoGesture)
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Refusal::OpaqueOrigin => "an opaque origin can hold no grant (ADR 0026 § 2)",
            Refusal::InsecureContext => "only a secure context may ask (ADR 0026 § 2)",
            Refusal::NotDelegated => "the embedding page did not allow it here (ADR 0026 § 2)",
            Refusal::RefusedEverywhere => "the person refused it for every site (ADR 0026 § 5)",
            Refusal::RememberedRefusal => "the person said don't allow (ADR 0026 § 5)",
            Refusal::RefusedThisDocument => "this page was refused already (ADR 0026 § 3)",
            Refusal::NoGesture => "nobody was using the page (ADR 0026 § 3)",
        })
    }
}

/// What came of an ask.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// A person already allowed it, and the grant has not ended.
    Granted,
    /// Refused, and no person was asked.
    Refused(Refusal),
    /// A person must answer.
    Prompt(Prompted),
}

/// An ask the table has decided to put to a person.
///
/// Made only by the table, never cloned, and consumed by the answer.
#[derive(Debug, PartialEq, Eq)]
pub struct Prompted {
    pub(crate) key: StorageKey,
    pub(crate) capability: Capability,
    pub(crate) document: DocumentId,
    pub(crate) cause: Cause,
}

impl Prompted {
    /// Who is asking, inside what: the two things the prompt shows (ADR 0026
    /// § 6), and nothing the page chose.
    pub fn key(&self) -> &StorageKey {
        &self.key
    }

    /// What for.
    pub fn capability(&self) -> Capability {
        self.capability
    }

    /// The document that asked.
    pub fn document(&self) -> DocumentId {
        self.document
    }

    /// Whether an agent was acting when the page asked, which the prompt says
    /// (ADR 0026 § 4). The page is never told.
    pub fn agent_was_acting(&self) -> bool {
        matches!(self.cause, Cause::Agent { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_reads_back_and_names_its_section() {
        for tag in 1..=7 {
            let refusal = Refusal::from_tag(tag).expect("a refusal");
            assert_eq!(refusal.tag(), tag);
            assert!(refusal.to_string().contains("ADR 0026 §"), "{refusal}");
        }
        assert_eq!(Refusal::from_tag(0), None);
        assert_eq!(Refusal::from_tag(8), None);
    }

    #[test]
    fn only_an_ask_without_a_gesture_is_answered_as_though_it_were_not_made() {
        for tag in 1..=7 {
            let refusal = Refusal::from_tag(tag).expect("a refusal");
            assert_eq!(refusal.looks_unasked(), refusal == Refusal::NoGesture);
        }
    }
}
