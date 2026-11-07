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

pub use crate::agent_paying::Paying;
pub use crate::agent_plan::Plan;

/// The most steps the wire carries: past the most a run takes (twenty in
/// `alo-window`'s `conversation_agent`), so a renderer is never sent more
/// than it is ready to draw.
pub const MOST_STEPS: usize = 40;

/// The most choices a stuck agent offers.
pub const MOST_CHOICES: usize = 4;

/// The most options of a closed list the agent's column lists (queue item
/// 798's sibling, 810); past them it says how many more.
pub const MOST_OPTIONS: usize = 8;

/// The most options the wire carries for one list: past any list a
/// person chooses in by reading it.
pub const MOST_LISTED: usize = 500;

/// What the page is said to be while the agent acts on it, the design's
/// words (`docs/design/shell.md`, *The agent at work*).
pub const WORKING: &str = "alo is working on this page \u{b7} click anywhere to take over";

/// What the agent says while the page is the person's, having been taken
/// over (queue item 825).
pub const PAUSED: &str =
    "You took over: the page is yours. alo carries on from it as you leave it.";

/// The agent at work on a card's page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtWork {
    /// What the person asked of it.
    pub asked: String,
    /// Its plan, once it wrote one (queue item 824).
    pub plan: Option<Plan>,
    /// Each step it took, oldest first.
    pub steps: Vec<Step>,
    /// What it is doing now.
    pub now: Now,
    /// The closed list it is choosing in, while the newest step chooses in
    /// one (queue item 810): its options listed in its own column, never a
    /// menu opened in a window nobody sees.
    pub choosing: Option<Dropdown>,
}

/// A closed list's options, as the agent choosing in it shows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dropdown {
    /// The list's name.
    pub list: String,
    /// Its options' names, in order.
    pub options: Vec<String>,
    /// The place of the option being chosen among them.
    pub chosen: usize,
}

impl Dropdown {
    /// The options shown in the column: at most [`MOST_OPTIONS`], the one
    /// chosen among them — the options from the start, or, for one past
    /// them, those ending with it — and how many are not shown.
    pub fn shown(&self) -> (Vec<(usize, &str)>, usize) {
        let first = (self.chosen + 1).saturating_sub(MOST_OPTIONS);
        let shown: Vec<(usize, &str)> = self
            .options
            .iter()
            .enumerate()
            .skip(first)
            .take(MOST_OPTIONS)
            .map(|(place, name)| (place, name.as_str()))
            .collect();
        let more = self.options.len().saturating_sub(shown.len());
        (shown, more)
    }
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
        /// What a press that pays pays, as the page says it (queue item
        /// 826): shown with the question, which is answered *Pay* or *Not
        /// now*; [`None`] for a press that does not pay. Boxed: few
        /// questions pay, and every card carries what it is doing.
        paying: Option<Box<Paying>>,
    },
    /// Held while the page is the person's: they pressed it while it worked
    /// (*click anywhere to take over*), and it carries on from the page as
    /// they leave it when they press *Let alo continue* (queue item 825).
    Paused,
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
            plan: None,
            steps: Vec::new(),
            now: Now::Acting,
            choosing: None,
        }
    }

    /// Whether it is still on the page — acting, or waiting on the person
    /// — so that the page is outlined and *Stop* is drawn. Paused, the page
    /// is the person's, and it is not.
    pub fn is_working(&self) -> bool {
        matches!(self.now, Now::Acting | Now::Asking { .. })
    }

    /// Whether it is held while the page is the person's (queue item 825).
    pub fn is_paused(&self) -> bool {
        matches!(self.now, Now::Paused)
    }

    /// Whether its task is finished, so that every step of its plan is
    /// done.
    pub fn is_finished(&self) -> bool {
        matches!(self.now, Now::Done { .. })
    }

    /// What its finished line says: *Done in 1 step*, *Done in 6 steps*.
    pub fn done_in(&self) -> String {
        done_in(self.steps.len())
    }
}

/// What a run's finished line says after `steps` steps: *Done in 1 step*,
/// *Done in 6 steps*.
pub fn done_in(steps: usize) -> String {
    match steps {
        1 => "Done in 1 step".to_owned(),
        steps => format!("Done in {steps} steps"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_long_lists_options_are_shown_eight_at_a_time_holding_the_chosen() {
        let list = |chosen| super::Dropdown {
            list: "Fruit".to_owned(),
            options: (0..12).map(|place| format!("Option {place}")).collect(),
            chosen,
        };
        let early = list(2);
        let (shown, more) = early.shown();
        assert_eq!(shown.first().copied(), Some((0, "Option 0")));
        assert_eq!((shown.len(), more), (8, 4));
        let late = list(10);
        let (shown, more) = late.shown();
        assert_eq!(shown.first().copied(), Some((3, "Option 3")));
        assert_eq!(shown.last().copied(), Some((10, "Option 10")));
        assert_eq!((shown.len(), more), (8, 4));
    }

    use super::*;

    #[test]
    fn it_is_working_while_it_acts_or_asks_and_not_after() {
        let mut work = AtWork::starting("send it");
        assert!(work.is_working());
        work.now = Now::Asking {
            what: "Press \u{201c}Send\u{201d}".to_owned(),
            paying: None,
        };
        assert!(work.is_working());
        work.now = Now::Paused;
        assert!(!work.is_working());
        assert!(work.is_paused());
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
