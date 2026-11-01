/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The agent at work on a card's page, on the wire (queue item 704, ADR
//! 0096): from the browser process to the first screen's renderer, read as
//! anything the renderer is sent is — what the agent is doing by a closed
//! list of tags, and at most [`MOST_STEPS`] steps and [`MOST_CHOICES`]
//! choices, refused past either. Every string is text, and enters the
//! document as a text node ([`crate::agent_document`]).
//!
//! And each run kept with a page shown earlier (queue item 820,
//! [`crate::agent_ran`]): at most [`MOST_RUNS`], each of at most
//! [`MOST_STEPS`] steps, how it ended by a closed list of tags.

use super::{Reader, Unreadable, Writer, unreadable};
use crate::agent_ran::{Ended, MOST_RUNS, Ran};
use crate::agent_work::{AtWork, MOST_CHOICES, MOST_STEPS, Now, Step};

impl Writer {
    /// The agent at work, if it is: whether it is, then what was asked,
    /// each step's line and why, and what it is doing now.
    pub(super) fn agent_work(&mut self, work: Option<&AtWork>) {
        let Some(work) = work else {
            self.bool(false);
            return;
        };
        self.bool(true);
        self.text(&work.asked);
        self.steps(&work.steps);
        match &work.now {
            Now::Acting => self.tag(0),
            Now::Asking { what } => {
                self.tag(1);
                self.text(what);
            }
            Now::Stuck { why, choices } => {
                self.tag(2);
                self.text(why);
                self.texts(choices);
            }
            Now::Done { said } => {
                self.tag(3);
                self.text(said);
            }
            Now::Stopped { why } => {
                self.tag(4);
                self.text(why);
            }
        }
    }
}

impl Writer {
    /// The runs kept with a page (queue item 820): how many, then each's
    /// asked, its steps' lines and whys, and how it ended.
    pub(super) fn runs(&mut self, runs: &[Ran]) {
        self.number(runs.len() as u64);
        for ran in runs {
            self.text(&ran.asked);
            self.steps(&ran.steps);
            match &ran.ended {
                Ended::Done { said } => {
                    self.tag(0);
                    self.text(said);
                }
                Ended::Stuck { why } => {
                    self.tag(1);
                    self.text(why);
                }
                Ended::Stopped { why } => {
                    self.tag(2);
                    self.text(why);
                }
            }
        }
    }

    /// Steps: how many, then each's line and why.
    fn steps(&mut self, steps: &[Step]) {
        self.number(steps.len() as u64);
        for step in steps {
            self.text(&step.line);
            self.text(&step.why);
        }
    }
}

impl Reader<'_> {
    /// The runs kept with a page, as [`Writer::runs`] wrote them: at most
    /// [`MOST_RUNS`], each of at most [`MOST_STEPS`] steps.
    pub(super) fn runs(&mut self) -> Result<Vec<Ran>, Unreadable> {
        let count = self.count()?;
        if count > MOST_RUNS {
            return Err(unreadable(format!(
                "a page kept with {count} runs of the agent, and at most {MOST_RUNS} are drawn"
            )));
        }
        let mut runs = Vec::with_capacity(count);
        for _ in 0..count {
            let asked = self.text()?;
            let steps = self.steps()?;
            let ended = match self.tag()? {
                0 => Ended::Done { said: self.text()? },
                1 => Ended::Stuck { why: self.text()? },
                2 => Ended::Stopped { why: self.text()? },
                other => {
                    return Err(unreadable(format!("a run kept as ended by tag {other}")));
                }
            };
            runs.push(Ran {
                asked,
                steps,
                ended,
            });
        }
        Ok(runs)
    }

    /// Steps, as [`Writer::steps`] wrote them: at most [`MOST_STEPS`].
    fn steps(&mut self) -> Result<Vec<Step>, Unreadable> {
        let count = self.count()?;
        if count > MOST_STEPS {
            return Err(unreadable(format!(
                "an agent of {count} steps, and at most {MOST_STEPS} are drawn"
            )));
        }
        let mut steps = Vec::with_capacity(count);
        for _ in 0..count {
            let line = self.text()?;
            steps.push(Step {
                line,
                why: self.text()?,
            });
        }
        Ok(steps)
    }

    /// The agent at work, as [`Writer::agent_work`] wrote it.
    pub(super) fn agent_work(&mut self) -> Result<Option<AtWork>, Unreadable> {
        if !self.bool()? {
            return Ok(None);
        }
        let asked = self.text()?;
        let steps = self.steps()?;
        let now = match self.tag()? {
            0 => Now::Acting,
            1 => Now::Asking { what: self.text()? },
            2 => {
                let why = self.text()?;
                let choices = self.texts()?;
                if choices.len() > MOST_CHOICES {
                    return Err(unreadable(format!(
                        "an agent offering {} choices, and at most {MOST_CHOICES} are drawn",
                        choices.len()
                    )));
                }
                Now::Stuck { why, choices }
            }
            3 => Now::Done { said: self.text()? },
            4 => Now::Stopped { why: self.text()? },
            other => return Err(unreadable(format!("an agent doing what is tagged {other}"))),
        };
        Ok(Some(AtWork { asked, steps, now }))
    }
}
