/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The record of what assistive technology asked of alo's own interface
//! and what came of it (ADR 0115, *Answered* 1; queue item 987).
//!
//! Every action from the platform's accessibility bus is in the record as
//! *"assistive technology"*: a screen reader and an agent driving the bus
//! through `alo-app` send the same actions, and nothing says which, so the
//! record names neither and claims nothing about who. It is written by the
//! browser process alone, which is the one part that sees both what was
//! asked and what came of it, as [`crate::agent_record`] is for the socket.
//!
//! Each entry is also said on the terminal, as a refused press is: a
//! person reading it there sees what was done in their window that they
//! did not do with their own hands.
//!
//! # Bounded
//!
//! At most [`MOST_KEPT`] entries; when it is full the oldest goes and is
//! counted.

use std::collections::VecDeque;

/// The most entries kept.
pub const MOST_KEPT: usize = 1_000;

/// Who every entry says asked.
pub const WHO: &str = "assistive technology";

/// What assistive technology did, newest last.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssistedRecord {
    entries: VecDeque<String>,
    forgotten: usize,
}

impl AssistedRecord {
    /// Write that `what` came of what was asked, and say it: the line, as
    /// it is kept and said.
    pub fn write(&mut self, what: &str) -> String {
        let line = format!("{WHO}: {what}");
        if self.entries.len() == MOST_KEPT {
            self.entries.pop_front();
            self.forgotten += 1;
        }
        self.entries.push_back(line.clone());
        line
    }

    /// Every entry kept, oldest first.
    pub fn entries(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(String::as_str)
    }

    /// How many entries were let go to keep the rest.
    pub fn forgotten(&self) -> usize {
        self.forgotten
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_entry_names_assistive_technology_and_the_oldest_goes_first() {
        let mut record = AssistedRecord::default();
        assert_eq!(
            record.write("a press on node 4 of the chats panel came to New chat"),
            "assistive technology: a press on node 4 of the chats panel came to New chat"
        );
        for each in 0..MOST_KEPT {
            record.write(&each.to_string());
        }
        assert_eq!(record.entries().count(), MOST_KEPT);
        assert_eq!(record.forgotten(), 1);
        assert_eq!(record.entries().next(), Some("assistive technology: 0"));
    }
}
