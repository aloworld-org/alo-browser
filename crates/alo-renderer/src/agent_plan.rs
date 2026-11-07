/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The agent's plan (queue item 824, `docs/design/shell.md`, *The agent's
//! plan*, approved by the owner 2026-11-06): before it acts, alo's agent
//! writes two to five short steps of the whole task, and the column shows
//! them — the steps done ticked, the one it is on in bold, the rest greyed.
//!
//! Data, and nothing else: the model writes the plan with its first action
//! and a new one whenever it must change (`alo-window`'s `agent_protocol`),
//! the browser process keeps the newest on the live card and in the chat's
//! file, and the first screen draws it ([`crate::agent_document`]). Every
//! step is a stranger's words, the model's, and enters the document as a
//! text node.
//!
//! **Not `chrome_mark`'s plan.** `alo-window`'s `chrome_mark::Plan` (item
//! 822) is what is asked of Chrome to mark a box after a verb — nothing,
//! take a mark away, or mark nodes. This is the agent's plan of its task.

/// The most steps a plan holds.
pub const MOST_PLAN: usize = 5;

/// The longest a step of a plan is, in characters.
pub const PLAN_LINE: usize = 80;

/// What the plan is named, to an agent reading the screen.
pub const PLAN: &str = "Plan";

/// The agent's plan of its task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Its steps, in order: at least one, at most [`MOST_PLAN`], each at
    /// most [`PLAN_LINE`] characters.
    pub steps: Vec<String>,
    /// The place of the step it is on, counting from nought: always one of
    /// [`Plan::steps`].
    pub on: usize,
}

/// Where a step of a plan stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stands {
    /// Done: drawn ticked.
    Done,
    /// The one the agent is on: drawn bold.
    Now,
    /// Not reached: drawn greyed.
    Ahead,
}

impl Stands {
    /// The word an agent reads before the step's words: *Done*, *Now*,
    /// *Next*.
    pub fn word(self) -> &'static str {
        match self {
            Self::Done => "Done",
            Self::Now => "Now",
            Self::Ahead => "Next",
        }
    }
}

impl Plan {
    /// A plan of `steps`, on the one at `on` — or none when no step has any
    /// words: steps without words dropped, those past [`MOST_PLAN`] too,
    /// each cut to [`PLAN_LINE`] characters, and `on` past the last the
    /// last.
    pub fn of(steps: &[String], on: usize) -> Option<Self> {
        let steps: Vec<String> = steps
            .iter()
            .map(|step| cut(step))
            .filter(|step| !step.is_empty())
            .take(MOST_PLAN)
            .collect();
        let last = steps.len().checked_sub(1)?;
        Some(Self {
            steps,
            on: on.min(last),
        })
    }

    /// Where its step at `at` stands, the task `finished` or not: every
    /// step is done once it is.
    pub fn stands(&self, at: usize, finished: bool) -> Stands {
        if finished || at < self.on {
            Stands::Done
        } else if at == self.on {
            Stands::Now
        } else {
            Stands::Ahead
        }
    }

    /// Whether it is within the bounds the wire and the chat's file hold:
    /// one to [`MOST_PLAN`] steps, none empty or past [`PLAN_LINE`]
    /// characters, and on one of them.
    pub fn is_bounded(&self) -> bool {
        (1..=MOST_PLAN).contains(&self.steps.len())
            && self.on < self.steps.len()
            && self
                .steps
                .iter()
                .all(|step| !step.trim().is_empty() && step.chars().count() <= PLAN_LINE)
    }
}

/// `step` on one line, trimmed, cut to [`PLAN_LINE`] characters with `…`.
fn cut(step: &str) -> String {
    let flat: String = step
        .trim()
        .chars()
        .map(|each| if each.is_control() { ' ' } else { each })
        .collect();
    if flat.chars().count() <= PLAN_LINE {
        return flat;
    }
    let kept: String = flat.chars().take(PLAN_LINE - 1).collect();
    format!("{}\u{2026}", kept.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps(said: &[&str]) -> Vec<String> {
        said.iter().map(|&step| step.to_owned()).collect()
    }

    #[test]
    fn a_plan_keeps_five_steps_with_words_each_on_a_line() {
        let plan = Plan::of(
            &steps(&["Open search", " ", "Type\nKampala", "a", "b", "c", "d"]),
            1,
        );
        assert_eq!(
            plan,
            Some(Plan {
                steps: steps(&["Open search", "Type Kampala", "a", "b", "c"]),
                on: 1,
            })
        );
        assert_eq!(Plan::of(&steps(&["", "  "]), 0), None);
        let long = "w".repeat(PLAN_LINE + 10);
        let cut = Plan::of(&[long], 0).map(|plan| plan.steps[0].clone());
        assert_eq!(cut.as_ref().map(|cut| cut.chars().count()), Some(PLAN_LINE));
        assert!(cut.is_some_and(|cut| cut.ends_with('\u{2026}')));
    }

    #[test]
    fn a_step_past_the_last_is_the_last() {
        assert_eq!(
            Plan::of(&steps(&["a", "b"]), 9).map(|plan| plan.on),
            Some(1)
        );
    }

    #[test]
    fn steps_before_it_are_done_it_is_now_and_after_it_ahead() {
        let plan = Plan {
            steps: steps(&["a", "b", "c"]),
            on: 1,
        };
        assert_eq!(
            [0, 1, 2].map(|at| plan.stands(at, false)),
            [Stands::Done, Stands::Now, Stands::Ahead]
        );
        assert_eq!([0, 1, 2].map(|at| plan.stands(at, true)), [Stands::Done; 3]);
        assert_eq!(
            [Stands::Done, Stands::Now, Stands::Ahead].map(Stands::word),
            ["Done", "Now", "Next"]
        );
    }

    #[test]
    fn its_bounds_are_one_to_five_steps_each_with_words_and_on_one() {
        let plan = |steps: Vec<String>, on| Plan { steps, on };
        assert!(plan(steps(&["a"]), 0).is_bounded());
        assert!(!plan(Vec::new(), 0).is_bounded());
        assert!(!plan(steps(&["a", "b"]), 2).is_bounded());
        assert!(!plan(steps(&["a", " "]), 0).is_bounded());
        assert!(!plan(steps(&["a"; MOST_PLAN + 1]), 0).is_bounded());
        assert!(!plan(vec!["w".repeat(PLAN_LINE + 1)], 0).is_bounded());
    }
}
