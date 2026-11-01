/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The agent at work on a card's page, on the wire (queue item 704, ADR
//! 0096): from the browser process to the first screen's renderer, read as
//! anything the renderer is sent is — what the agent is doing by a closed
//! list of tags, and at most [`MOST_STEPS`] steps and [`MOST_CHOICES`]
//! choices, refused past either. Every string is text, and enters the
//! document as a text node ([`crate::agent_document`]).

use super::{Reader, Unreadable, Writer, unreadable};
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
        self.number(work.steps.len() as u64);
        for step in &work.steps {
            self.text(&step.line);
            self.text(&step.why);
        }
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

impl Reader<'_> {
    /// The agent at work, as [`Writer::agent_work`] wrote it.
    pub(super) fn agent_work(&mut self) -> Result<Option<AtWork>, Unreadable> {
        if !self.bool()? {
            return Ok(None);
        }
        let asked = self.text()?;
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
