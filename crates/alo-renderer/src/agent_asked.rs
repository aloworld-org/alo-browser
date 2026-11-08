/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A run of alo's agent asked with no page open (queue item 821, ADR 0103,
//! `docs/design/shell.md`, *Beginning with no page*), as the browser
//! process tells the first screen's renderer of it: a turn of its own until
//! the run opens a page — then the turn is that page's card, the run moved
//! onto it — and, when it ends having opened none, kept as what it said.
//!
//! While it finds the site — searching, asking the person whether it may
//! open one — its column is drawn in the turn under what the person asked
//! ([`crate::agent_document::beginning`]): the working line with *Stop*, its
//! plan, its steps, a question with *Allow* and *Not now*, or how it ended.
//! Once a newer turn comes, or the chat is read back from its file, it is
//! drawn as a page shown earlier's runs are ([`crate::ran_document`]).
//!
//! Every string here is a stranger's — what the person asked, what the
//! model wrote — and enters the document as a text node.

use crate::agent_ran::Ran;
use crate::agent_work::AtWork;

/// A run asked with no page open: what the person asked, the agent at work
/// while it is live, and the run as the chat keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// What the person asked, their bubble.
    pub said: String,
    /// The agent at work, while the turn is the newest and the run is live
    /// or has just ended in it; [`None`] once put away or read back.
    pub agent: Option<AtWork>,
    /// The run as the chat keeps it (queue item 820): at most one.
    pub ran: Vec<Ran>,
}

impl Asked {
    /// A turn asked `said` with no page open, its run not yet begun: the
    /// browser process puts the agent at work in it as it starts the run.
    pub fn of(said: impl Into<String>) -> Self {
        Self {
            said: said.into(),
            agent: None,
            ran: Vec::new(),
        }
    }

    /// Put away as a newer turn comes: no longer live, drawn as it is kept.
    pub fn put_away(&mut self) {
        self.agent = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_put_away_as_kept() {
        let mut asked = Asked::of("find the museum's hours");
        assert_eq!(asked.agent, None);
        asked.agent = Some(AtWork::starting("find the museum's hours"));
        asked.ran = vec![Ran::starting("find the museum's hours")];
        asked.put_away();
        assert_eq!(asked.agent, None);
        assert_eq!(asked.ran.len(), 1, "kept");
    }
}
