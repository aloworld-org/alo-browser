/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The record of what agents outside this browser read and did (ADR 0055
//! § 7).
//!
//! Written by the browser process and by nothing else: it is the one part
//! that sees both the verb and what came of it, and the one part a page
//! cannot reach. A renderer that could write it could lie about what it did.
//!
//! Like the record of what was fetched (ADR 0012 § 7), it is kept **for the
//! person and about the agent**: nothing an agent can ask is answered from
//! it.
//!
//! # What it says
//!
//! Which caller asked, at which tab, and what came of it: how much of which
//! document was read, or the verb, what it was aimed at, the action it was
//! given and its outcome or refusal — *what it read and what it changed*,
//! which is ADR 0052's agent gate. A request refused before it reached a tab
//! is written too, because *what did it try* is a question this answers.
//!
//! # Bounded
//!
//! At most [`MOST_KEPT`] entries. When it is full the oldest goes and is
//! counted, so a long session keeps what was done last and says how much was
//! not kept.

use crate::asking::Refused;
use alo_agent::{Outcome, Refusal, Target, Verb};
use alo_net::cause::{ActionId, DocumentId};
use core::fmt;
use std::collections::VecDeque;

/// The most entries kept.
pub const MOST_KEPT: usize = 10_000;

/// One agent's connection, numbered in the order they came and never reused
/// (ADR 0003's rule, for callers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Caller(pub u64);

impl fmt::Display for Caller {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "agent#{}", self.0)
    }
}

/// What one request came to.
#[derive(Debug, Clone, PartialEq)]
pub enum Did {
    /// It was told the address of every open tab.
    Listed {
        /// How many there were.
        tabs: usize,
    },
    /// It read a page's tree.
    Read {
        /// The document read.
        document: Option<DocumentId>,
        /// How many nodes it was handed.
        nodes: usize,
    },
    /// It asked for a verb, which the browser accepted under `action`.
    Acted {
        /// The action everything that follows from the verb is attributed to.
        action: ActionId,
        /// The document it acted in.
        document: Option<DocumentId>,
        /// What it aimed at.
        target: Target,
        /// What it asked for.
        verb: Verb,
        /// What came of it: what changed, or why nothing did.
        came_to: Result<Outcome, Refusal>,
    },
    /// It was refused before any page answered.
    Refused(Refused),
}

/// One entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Who asked.
    pub caller: Caller,
    /// The tab it named, as it named it, if it named one.
    pub tab: Option<String>,
    /// What came of it.
    pub did: Did,
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.caller)?;
        if let Some(tab) = &self.tab {
            write!(f, " at {tab}")?;
        }
        match &self.did {
            Did::Listed { tabs } => write!(f, ": listed {tabs} tabs"),
            Did::Read { document, nodes } => {
                write!(f, ": read {nodes} nodes")?;
                document.map_or(Ok(()), |document| write!(f, " of {document}"))
            }
            Did::Acted {
                action,
                target,
                verb,
                came_to,
                ..
            } => {
                write!(f, ": {action} {verb:?} {target}: ")?;
                match came_to {
                    Ok(outcome) => write!(f, "{outcome}"),
                    Err(refusal) => write!(f, "refused, {refusal}"),
                }
            }
            Did::Refused(refused) => write!(f, ": refused, {refused}"),
        }
    }
}

/// Every entry kept, oldest first, and how many were not.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AgentRecord {
    entries: VecDeque<Entry>,
    forgotten: usize,
}

impl AgentRecord {
    /// An empty record.
    pub fn new() -> Self {
        Self::default()
    }

    /// Write `entry`, letting the oldest go if the record is full.
    pub fn write(&mut self, entry: Entry) {
        if self.entries.len() >= MOST_KEPT {
            self.entries.pop_front();
            self.forgotten = self.forgotten.saturating_add(1);
        }
        self.entries.push_back(entry);
    }

    /// Every entry kept, oldest first.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter()
    }

    /// How many entries were let go to keep the rest.
    pub fn forgotten(&self) -> usize {
        self.forgotten
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refused(caller: u64) -> Entry {
        Entry {
            caller: Caller(caller),
            tab: None,
            did: Did::Refused(Refused::NotJson),
        }
    }

    #[test]
    fn a_full_record_lets_the_oldest_go_and_counts_it() {
        let mut record = AgentRecord::new();
        for caller in 0..=u64::try_from(MOST_KEPT).unwrap_or(u64::MAX) {
            record.write(refused(caller));
        }
        assert_eq!(record.entries().count(), MOST_KEPT);
        assert_eq!(record.forgotten(), 1);
        assert_eq!(
            record.entries().next().map(|entry| entry.caller),
            Some(Caller(1))
        );
    }

    #[test]
    fn an_entry_says_who_and_what_in_words() {
        assert_eq!(
            refused(2).to_string(),
            "agent#2: refused, the line is not JSON"
        );
        let listed = Entry {
            caller: Caller(0),
            tab: None,
            did: Did::Listed { tabs: 3 },
        };
        assert_eq!(listed.to_string(), "agent#0: listed 3 tabs");
    }
}
