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
//!
//! A plan (queue item 824) is refused past [`crate::agent_plan::MOST_PLAN`]
//! steps, a step past [`crate::agent_plan::PLAN_LINE`] characters or one on
//! no step; what a press pays (826) past
//! [`crate::agent_paying::MOST_SUMMARY`] lines or a line past
//! [`crate::agent_paying::PAYING_LINE`] characters.

use super::{Reader, Unreadable, Writer, unreadable};
use crate::agent_ran::{Ended, MOST_RUNS, Ran};
use crate::agent_work::{
    AtWork, Dropdown, MOST_CHOICES, MOST_LISTED, MOST_STEPS, Now, Paying, Plan, Step,
};

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
        self.plan(work.plan.as_ref());
        self.steps(&work.steps);
        match &work.now {
            Now::Acting => self.tag(0),
            Now::Asking { what, paying } => {
                self.tag(1);
                self.text(what);
                self.paying(paying.as_deref());
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
            Now::Paused => self.tag(5),
            // Handed over to sign in (queue item 821).
            Now::SignIn => self.tag(6),
        }
        // The closed list it chooses in (queue item 810).
        self.bool(work.choosing.is_some());
        if let Some(choosing) = &work.choosing {
            self.text(&choosing.list);
            self.texts(&choosing.options);
            self.number(choosing.chosen as u64);
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
            self.plan(ran.plan.as_ref());
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

    /// A plan, if there is one (queue item 824): its steps, then the place
    /// of the one it is on.
    fn plan(&mut self, plan: Option<&Plan>) {
        self.bool(plan.is_some());
        if let Some(plan) = plan {
            self.texts(&plan.steps);
            self.number(plan.on as u64);
        }
    }

    /// What a press pays, if it pays (queue item 826): the amount if the
    /// page shows one, who is paid, and the page's summary lines.
    fn paying(&mut self, paying: Option<&Paying>) {
        self.bool(paying.is_some());
        if let Some(paying) = paying {
            self.bool(paying.amount.is_some());
            if let Some(amount) = &paying.amount {
                self.text(amount);
            }
            self.text(&paying.merchant);
            self.texts(&paying.lines);
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
            let plan = self.plan()?;
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
                plan,
                steps,
                ended,
            });
        }
        Ok(runs)
    }

    /// A plan, as [`Writer::plan`] wrote it, refused past its bounds.
    fn plan(&mut self) -> Result<Option<Plan>, Unreadable> {
        if !self.bool()? {
            return Ok(None);
        }
        let steps = self.texts()?;
        let on = usize::try_from(self.number()?).unwrap_or(usize::MAX);
        let plan = Plan { steps, on };
        if !plan.is_bounded() {
            return Err(unreadable(format!(
                "a plan of {} steps on its step {}, past what is drawn: one to {} steps of at most \
                 {} characters, on one of them",
                plan.steps.len(),
                plan.on,
                crate::agent_plan::MOST_PLAN,
                crate::agent_plan::PLAN_LINE
            )));
        }
        Ok(Some(plan))
    }

    /// What a press pays, as [`Writer::paying`] wrote it, refused past its
    /// bounds.
    fn paying(&mut self) -> Result<Option<Paying>, Unreadable> {
        if !self.bool()? {
            return Ok(None);
        }
        let amount = if self.bool()? {
            Some(self.text()?)
        } else {
            None
        };
        let paying = Paying {
            amount,
            merchant: self.text()?,
            lines: self.texts()?,
        };
        if !paying.is_bounded() {
            return Err(unreadable(format!(
                "what a press pays told in {} lines, past the {} drawn of at most {} characters",
                paying.lines.len(),
                crate::agent_paying::MOST_SUMMARY,
                crate::agent_paying::PAYING_LINE
            )));
        }
        Ok(Some(paying))
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
        let plan = self.plan()?;
        let steps = self.steps()?;
        let now = match self.tag()? {
            0 => Now::Acting,
            1 => Now::Asking {
                what: self.text()?,
                paying: self.paying()?.map(Box::new),
            },
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
            5 => Now::Paused,
            6 => Now::SignIn,
            other => return Err(unreadable(format!("an agent doing what is tagged {other}"))),
        };
        let choosing = if self.bool()? {
            let list = self.text()?;
            let options = self.texts()?;
            let chosen = usize::try_from(self.number()?)
                .ok()
                .filter(|chosen| *chosen < options.len() && options.len() <= MOST_LISTED)
                .ok_or_else(|| unreadable("a closed list's option chosen past its options"))?;
            Some(Dropdown {
                list,
                options,
                chosen,
            })
        } else {
            None
        };
        Ok(Some(AtWork {
            asked,
            plan,
            steps,
            now,
            choosing,
        }))
    }
}
