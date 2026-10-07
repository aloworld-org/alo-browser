/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a site was allowed and did with it, without what it carried.
//!
//! ADR 0026 § 8: every ask, every answer (a dismissal, and a refusal without a
//! prompt with the rule that refused it), every start and end of a use, every
//! revocation and every expiry. Each names the capability, the time and the
//! cause; the storage key is the history it is filed in.
//!
//! # No content, by construction
//!
//! An [`Entry`] has no field that could hold a frame, a sound, a position, a
//! notification's text or what was on the clipboard. *"A record of where
//! somebody was is a tracker we would be keeping on them."*
//!
//! # Sixty-four, and the drop is counted
//!
//! A history keeps the newest [`KEPT`] entries. The oldest goes first, and
//! [`History::dropped`] counts every one that went, so a short history never
//! reads as a quiet one.
//!
//! # A cause that outlives its process
//!
//! The history is written to a disk, and an identity in a file is not an
//! identity (`alo_net::deed`). So a cause is kept as `alo-net`'s [`Link`]:
//! the same three causes, as numbers that name nothing live.
//!
//! An ending has no cause, and that is not a fourth one: nobody did it. Thirty
//! days passing, or a page closing, is a fact about time and documents, and an
//! entry that named somebody for it would be naming the wrong person.

use crate::ask::Refusal;
use crate::capability::Capability;
use crate::grant::{Answer, Ending};
use alo_net::deed::Link;
use std::collections::VecDeque;
use std::time::SystemTime;

/// How many entries a storage key's history keeps (ADR 0026 § 8).
pub const KEPT: usize = 64;

/// What happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Happened {
    /// A page asked.
    Asked {
        /// What caused the ask.
        by: Link,
    },
    /// The ask was refused and nobody was prompted.
    RefusedWithoutAPrompt {
        /// The rule that refused it.
        refusal: Refusal,
        /// What caused the ask.
        by: Link,
    },
    /// A person answered a prompt.
    Answered {
        /// What they said.
        answer: Answer,
        /// The person, in the tab they answered in.
        by: Link,
    },
    /// A person dismissed a prompt, which refuses that document and stores
    /// nothing.
    Dismissed {
        /// The person, in the tab they dismissed it in.
        by: Link,
    },
    /// The site began using what it was granted.
    UseBegan {
        /// What caused the use.
        by: Link,
    },
    /// That use ended: the page stopped, the page closed, or the grant was
    /// revoked or ended under it.
    UseEnded {
        /// What caused the use that ended.
        by: Link,
    },
    /// A person took the grant or the refusal away.
    Revoked {
        /// The person, in the tab they did it in.
        by: Link,
    },
    /// The grant or the refusal ended on its own terms.
    Ended {
        /// Which terms.
        ending: Ending,
    },
}

/// One thing that happened to one capability of one storage key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// When.
    pub at: SystemTime,
    /// Which capability.
    pub capability: Capability,
    /// What.
    pub happened: Happened,
}

/// One storage key's history: its newest [`KEPT`] entries, and how many older
/// ones were dropped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    entries: VecDeque<Entry>,
    dropped: u64,
}

impl History {
    /// A history read back from a disk.
    ///
    /// [`None`] when it holds more than [`KEPT`] entries, which no table this
    /// engine wrote does.
    pub fn from_parts(entries: Vec<Entry>, dropped: u64) -> Option<Self> {
        (entries.len() <= KEPT).then(|| Self {
            entries: entries.into(),
            dropped,
        })
    }

    /// Add an entry, dropping the oldest when it is full.
    pub fn add(&mut self, entry: Entry) {
        if self.entries.len() >= KEPT && self.entries.pop_front().is_some() {
            self.dropped = self.dropped.saturating_add(1);
        }
        self.entries.push_back(entry);
    }

    /// The entries, oldest first.
    pub fn entries(&self) -> impl DoubleEndedIterator<Item = &Entry> + ExactSizeIterator {
        self.entries.iter()
    }

    /// How many entries were dropped to keep it at [`KEPT`].
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn asked(second: u64) -> Entry {
        Entry {
            at: UNIX_EPOCH + Duration::from_secs(second),
            capability: Capability::Notifications,
            happened: Happened::Asked {
                by: Link::Document { document: 7 },
            },
        }
    }

    #[test]
    fn the_sixty_fifth_entry_drops_the_first_and_counts_it() {
        let mut history = History::default();
        for second in 0..64 {
            history.add(asked(second));
        }
        assert_eq!((history.entries().len(), history.dropped()), (64, 0));
        history.add(asked(64));
        history.add(asked(65));
        assert_eq!((history.entries().len(), history.dropped()), (64, 2));
        assert_eq!(history.entries().next(), Some(&asked(2)));
        assert_eq!(history.entries().last(), Some(&asked(65)));
    }

    #[test]
    fn a_history_longer_than_any_this_engine_keeps_is_refused() {
        let full: Vec<Entry> = (0..64).map(asked).collect();
        assert!(History::from_parts(full.clone(), 3).is_some());
        let mut over = full;
        over.push(asked(64));
        assert_eq!(History::from_parts(over, 0), None);
    }
}
