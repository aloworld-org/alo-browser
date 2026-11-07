/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A run of alo's agent as the chat keeps it (queue item 820, ADR 0092 as
//! amended, ADR 0085): what the person asked, each step in words, and how
//! it ended — kept with the page it worked on, and drawn under that page's
//! line once the page is shown earlier ([`crate::conversation_document`]).
//!
//! It is what the person was shown of the run, less two things the browser
//! process takes out before it is ever here (`alo-window`'s `agent_kept`):
//! **what was typed into a field** — a typing step is kept as
//! [`TYPED`] unless the person's own words said what was typed — and the
//! choices a stuck run offered, which were buttons on a live card and are
//! no use on a line. Never anything of the page beyond what the steps
//! already say; never a picture.
//!
//! Every string here is a stranger's — what the person asked, what the
//! model wrote — and enters the document as a text node.

use crate::agent_plan::Plan;
use crate::agent_work::{Step, done_in};

/// The most runs kept with one page: the newest; an older one past this is
/// not kept.
pub const MOST_RUNS: usize = 20;

/// What a typing step is kept as when what was typed is not the person's
/// own words.
pub const TYPED: &str = "Typed into a field";

/// What a run still at work when alo closed is read back as having come
/// to.
pub const CLOSED: &str = "alo was closed before this was finished.";

/// A run of the agent, as a chat keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    /// What the person asked of it.
    pub asked: String,
    /// Its plan as it last stood, if it wrote one (queue item 824), its
    /// words kept as every other line's are.
    pub plan: Option<Plan>,
    /// Each step it took, oldest first, in words.
    pub steps: Vec<Step>,
    /// How it ended.
    pub ended: Ended,
}

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ended {
    /// Finished: what came of it, in full.
    Done {
        /// The result, in the model's words.
        said: String,
    },
    /// Stuck, and why.
    Stuck {
        /// Why it could not go on.
        why: String,
    },
    /// Stopped before it finished, and why — the person, its limits, the
    /// model, or alo closing ([`CLOSED`]).
    Stopped {
        /// Why.
        why: String,
    },
}

impl Ran {
    /// A run asked `asked`, with no step yet — and, until it ends, read
    /// back as stopped by alo closing, which is what it comes to if alo
    /// closes first.
    pub fn starting(asked: impl Into<String>) -> Self {
        Self {
            asked: asked.into(),
            plan: None,
            steps: Vec::new(),
            ended: Ended::Stopped {
                why: CLOSED.to_owned(),
            },
        }
    }

    /// What its finished line says: *Done in 1 step*, *Done in 6 steps*.
    pub fn done_in(&self) -> String {
        done_in(self.steps.len())
    }

    /// Whether it finished, so that every step of its plan is done.
    pub fn is_finished(&self) -> bool {
        matches!(self.ended, Ended::Done { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_starting_is_read_as_stopped_by_alo_closing_and_counts_its_steps() {
        let mut ran = Ran::starting("send it");
        assert_eq!(
            ran.ended,
            Ended::Stopped {
                why: CLOSED.to_owned()
            }
        );
        assert_eq!(ran.done_in(), "Done in 0 steps");
        ran.steps.push(Step {
            line: TYPED.to_owned(),
            why: String::new(),
        });
        assert_eq!(ran.done_in(), "Done in 1 step");
    }
}
