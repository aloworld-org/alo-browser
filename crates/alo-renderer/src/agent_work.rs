/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The agent at work on a card's page, as the browser process tells the
//! first screen's renderer of it (queue item 704, ADR 0096): data, and
//! nothing else.
//!
//! The browser process runs the agent (`alo-window`'s
//! `conversation_agent`); this is what the person is shown of it, drawn
//! under the card's page ([`crate::agent_document`]): what they asked, one
//! line per step in a person's words with why when it chose, and what it
//! is doing now — acting, asking the person before it presses something
//! that pays, sends, deletes or sends a form, stuck with choices, done, or
//! stopped and why.
//!
//! Every string here is a stranger's: what the person typed, and what the
//! model wrote of each step. Each enters the document as a text node.

/// The most steps the wire carries: past the most a run takes (twenty in
/// `alo-window`'s `conversation_agent`), so a renderer is never sent more
/// than it is ready to draw.
pub const MOST_STEPS: usize = 40;

/// The most choices a stuck agent offers.
pub const MOST_CHOICES: usize = 4;

/// What the page is said to be while the agent acts on it, the design's
/// words (`docs/design/shell.md`, *The agent at work*).
pub const WORKING: &str = "alo is working on this page \u{b7} click anywhere to take over";

/// The agent at work on a card's page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtWork {
    /// What the person asked of it.
    pub asked: String,
    /// Each step it took, oldest first.
    pub steps: Vec<Step>,
    /// What it is doing now.
    pub now: Now,
}

/// One step, in a person's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// What it did: *Typing "Kampala" into Search Wikipedia*.
    pub line: String,
    /// Why, when it chose; empty when it did not.
    pub why: String,
}

/// What the agent is doing now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Now {
    /// Acting on the page: the page is outlined, and *Stop* is drawn.
    Acting,
    /// Waiting on the person before it does what it asks: the question,
    /// with *Allow* and *Not now* (ADR 0052's person's gate, given a face).
    Asking {
        /// What it would do, in a person's words.
        what: String,
    },
    /// Stuck: why, and what it could do instead, the closest to what was
    /// asked first.
    Stuck {
        /// Why it cannot go on.
        why: String,
        /// What it offers, at most [`MOST_CHOICES`].
        choices: Vec<String>,
    },
    /// Finished: what came of it, in full.
    Done {
        /// The result, in the model's words.
        said: String,
    },
    /// Stopped before it finished, and why: the person stopped it or took
    /// over, it reached its limit, or the model could not be asked.
    Stopped {
        /// Why.
        why: String,
    },
}

impl AtWork {
    /// The agent starting on `asked`, with no step yet.
    pub fn starting(asked: impl Into<String>) -> Self {
        Self {
            asked: asked.into(),
            steps: Vec::new(),
            now: Now::Acting,
        }
    }

    /// Whether it is still on the page — acting, or waiting on the person
    /// — so that the page is outlined and *Stop* is drawn.
    pub fn is_working(&self) -> bool {
        matches!(self.now, Now::Acting | Now::Asking { .. })
    }

    /// What its finished line says: *Done in 1 step*, *Done in 6 steps*.
    pub fn done_in(&self) -> String {
        match self.steps.len() {
            1 => "Done in 1 step".to_owned(),
            steps => format!("Done in {steps} steps"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_working_while_it_acts_or_asks_and_not_after() {
        let mut work = AtWork::starting("send it");
        assert!(work.is_working());
        work.now = Now::Asking {
            what: "Press \u{201c}Send\u{201d}".to_owned(),
        };
        assert!(work.is_working());
        for now in [
            Now::Done {
                said: "Sent.".to_owned(),
            },
            Now::Stopped {
                why: "You stopped it.".to_owned(),
            },
            Now::Stuck {
                why: "No form.".to_owned(),
                choices: Vec::new(),
            },
        ] {
            work.now = now;
            assert!(!work.is_working());
        }
    }

    #[test]
    fn done_counts_its_steps_in_words() {
        let mut work = AtWork::starting("x");
        assert_eq!(work.done_in(), "Done in 0 steps");
        work.steps.push(Step {
            line: "a".to_owned(),
            why: String::new(),
        });
        assert_eq!(work.done_in(), "Done in 1 step");
        work.steps.push(work.steps[0].clone());
        assert_eq!(work.done_in(), "Done in 2 steps");
    }
}
