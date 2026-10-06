/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Running a page's loop until an agent's task has run, and keeping what the
//! page's script said meanwhile (ADR 0018 § 3, queue items 256 and 257).
//!
//! An agent's verb on a page that runs script is one task on the page's loop
//! — a click ([`crate::press`]), text put into a field ([`crate::put`]) —
//! and the agent is answered after it. This runs that task, and any task
//! queued before it, oldest first, since one order holds across all of them
//! (ADR 0016 § 2), and answers the task's own [`Turn`].
//!
//! # How much it says
//!
//! What one `Act` says crosses in one message, so it is bounded as a load's
//! is ([`crate::scripts`]): at most [`MOST_REPORTS`] lines, each at most
//! [`LONGEST_LINE`](crate::said::LONGEST_LINE) characters, and then how many
//! more there were.

use crate::event_loop::{MOST_REPORTS, Seq, Turn};
use crate::held::Held;
use crate::said;

/// What an agent's task said, as the lines an `Act` answers with.
#[derive(Debug)]
pub(crate) struct Said {
    /// What each line is about — `"the click"`, say.
    about: &'static str,
    lines: Vec<String>,
    left_out: usize,
}

impl Said {
    /// Nothing said yet, about `about`.
    pub(crate) const fn about(about: &'static str) -> Self {
        Self {
            about,
            lines: Vec::new(),
            left_out: 0,
        }
    }

    /// Say `what`, if there is room, or count it.
    pub(crate) fn say(&mut self, what: &dyn core::fmt::Display) {
        if self.lines.len() < MOST_REPORTS {
            let about = self.about;
            self.lines
                .push(said::line(&format_args!("{about}: {what}")));
        } else {
            self.left_out = self.left_out.saturating_add(1);
        }
    }

    /// How many more lines there is room for.
    fn room(&self) -> usize {
        MOST_REPORTS.saturating_sub(self.lines.len())
    }

    /// The lines, and one more saying how many were left out, if any were.
    pub(crate) fn lines(mut self) -> Vec<String> {
        if self.left_out > 0 {
            let (left_out, about) = (self.left_out, self.about);
            self.lines.push(format!(
                "{left_out} more things {about}'s script said were not said: one action says \
                 at most {MOST_REPORTS}"
            ));
        }
        self.lines
    }
}

/// Run the page's loop until the task `seq` has run, saying what every task
/// it ran said, and answer that task's turn — [`None`] if the loop stopped
/// first, or the page has no loop.
pub(crate) fn run_to(held: &mut Held, seq: Seq, said: &mut Said) -> Option<Turn> {
    let page_loop = held.event_loop()?;
    while let Some(turn) = page_loop.run_next_within(said.room()) {
        for report in &turn.reports {
            said.say(report);
        }
        said.left_out = said.left_out.saturating_add(turn.unreported);
        if let Some(stopped) = &turn.stopped {
            said.say(stopped);
        }
        if turn.task == seq {
            return Some(turn);
        }
    }
    None
}
