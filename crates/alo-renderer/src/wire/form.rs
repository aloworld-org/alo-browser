/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A form's submission on the wire (ADR 0051 §§ 5 and 6, queue item 468):
//! the submission an ask carries, and what an agent is told about one.
//!
//! The submission comes from a renderer, so every part is read as a
//! stranger's: the method, the enctype and each entry's kind by a closed
//! list of tags, and the entries counted against what is left before any is
//! read. **A method or enctype no form could make is refused here**, as a
//! renderer that broke the boundary — a tag outside the list is not a
//! request this process could send. How many entries, and how long, is
//! bounded by the message itself ([`super::LARGEST_MESSAGE`]), which is the
//! bound ADR 0051 § 5 names.

use alo_agent::verb::{Invalid, Outcome, Refusal};
use alo_bindings::navigating::Submission;
use alo_box::tree::BoxId;
use alo_dom::constraint::Constraint;
use alo_dom::submission::{Enctype, Entry, Method, Value};

use super::fetch::{tag_of, tagged};
use super::{Reader, Unreadable, Writer, unreadable};

/// Each method by its tag.
const METHODS: [Method; 2] = [Method::Get, Method::Post];

/// Each enctype by its tag.
const ENCTYPES: [Enctype; 3] = [Enctype::UrlEncoded, Enctype::Multipart, Enctype::TextPlain];

impl Writer {
    /// A submission, if the ask is one.
    pub(super) fn submission(&mut self, submission: Option<&Submission>) {
        let Some(submission) = submission else {
            self.bool(false);
            return;
        };
        self.bool(true);
        self.tag(tag_of(&METHODS, &submission.method));
        self.tag(tag_of(&ENCTYPES, &submission.enctype));
        self.number(submission.entries.len() as u64);
        for entry in &submission.entries {
            self.text(&entry.name);
            match &entry.value {
                Value::Text(text) => {
                    self.tag(0);
                    self.text(text);
                }
                Value::NoFile => self.tag(1),
            }
        }
    }

    fn maybe_id(&mut self, id: Option<BoxId>) {
        match id {
            Some(id) => {
                self.bool(true);
                self.id(id);
            }
            None => self.bool(false),
        }
    }

    /// The two outcomes a press on a submit button has beyond `Activated`:
    /// tagged 5 and 6, after the five [`super`] writes.
    pub(super) fn submitted(&mut self, outcome: &Outcome) {
        match outcome {
            Outcome::Submitted { node, method, to } => {
                self.tag(5);
                self.id(*node);
                self.tag(tag_of(&METHODS, method));
                self.text(to);
            }
            Outcome::NotSubmitted { node, invalid } => {
                self.tag(6);
                self.id(*node);
                self.number(invalid.len() as u64);
                for one in invalid {
                    self.maybe_id(one.control);
                    match &one.constraint {
                        Some(constraint) => {
                            self.bool(true);
                            self.tag(tag_of(&Constraint::ALL, constraint));
                        }
                        None => self.bool(false),
                    }
                    self.text(&one.message);
                }
            }
            Outcome::Activated { .. }
            | Outcome::Followed { .. }
            | Outcome::TextPut { .. }
            | Outcome::Scrolled { .. }
            | Outcome::TextCanceled { .. } => {}
        }
    }

    /// The three refusals forms brought: tagged 9 to 11, after the nine
    /// [`super`] writes.
    pub(super) fn form_refusal(&mut self, refusal: &Refusal) {
        match refusal {
            Refusal::TooLong { node, most, length } => {
                self.tag(9);
                self.id(*node);
                self.number(*most as u64);
                self.number(*length as u64);
            }
            Refusal::Unchecked {
                node,
                control,
                constraint,
            } => {
                self.tag(10);
                self.id(*node);
                self.maybe_id(*control);
                self.tag(tag_of(&Constraint::ALL, constraint));
            }
            Refusal::NotSubmittable { node, why } => {
                self.tag(11);
                self.id(*node);
                self.text(why);
            }
            _ => {}
        }
    }
}

impl Reader<'_> {
    /// A submission, if the ask is one: every part a claim.
    pub(super) fn submission(&mut self) -> Result<Option<Submission>, Unreadable> {
        if !self.bool()? {
            return Ok(None);
        }
        let method = tagged(&METHODS, self.tag()?, "a submission's method")?;
        let enctype = tagged(&ENCTYPES, self.tag()?, "a submission's enctype")?;
        let how_many = self.count()?;
        let mut entries = Vec::new();
        for _ in 0..how_many {
            let name = self.text()?;
            let value = match self.tag()? {
                0 => Value::Text(self.text()?),
                1 => Value::NoFile,
                other => return Err(unreadable(format!("an entry's value tagged {other}"))),
            };
            entries.push(Entry { name, value });
        }
        Ok(Some(Submission {
            method,
            enctype,
            entries,
        }))
    }

    fn maybe_id(&mut self) -> Result<Option<BoxId>, Unreadable> {
        if self.bool()? {
            Ok(Some(self.id()?))
        } else {
            Ok(None)
        }
    }

    fn constraint(&mut self) -> Result<Constraint, Unreadable> {
        tagged(&Constraint::ALL, self.tag()?, "a constraint")
    }

    fn size(&mut self, what: &str) -> Result<usize, Unreadable> {
        usize::try_from(self.number()?)
            .map_err(|_| unreadable(format!("{what} larger than this machine can hold")))
    }

    /// An outcome tagged 5 or 6.
    pub(super) fn submitted(&mut self, tag: u8) -> Result<Outcome, Unreadable> {
        match tag {
            5 => Ok(Outcome::Submitted {
                node: self.id()?,
                method: tagged(&METHODS, self.tag()?, "a submission's method")?,
                to: self.text()?,
            }),
            6 => {
                let node = self.id()?;
                let how_many = self.count()?;
                let mut invalid = Vec::new();
                for _ in 0..how_many {
                    invalid.push(Invalid {
                        control: self.maybe_id()?,
                        constraint: if self.bool()? {
                            Some(self.constraint()?)
                        } else {
                            None
                        },
                        message: self.text()?,
                    });
                }
                Ok(Outcome::NotSubmitted { node, invalid })
            }
            other => Err(unreadable(format!("an outcome tagged {other}"))),
        }
    }

    /// A refusal tagged 9 to 11.
    pub(super) fn form_refusal(&mut self, tag: u8) -> Result<Refusal, Unreadable> {
        match tag {
            9 => Ok(Refusal::TooLong {
                node: self.id()?,
                most: self.size("a maxlength")?,
                length: self.size("a text's length")?,
            }),
            10 => Ok(Refusal::Unchecked {
                node: self.id()?,
                control: self.maybe_id()?,
                constraint: self.constraint()?,
            }),
            11 => Ok(Refusal::NotSubmittable {
                node: self.id()?,
                why: self.text()?,
            }),
            other => Err(unreadable(format!("a refusal tagged {other}"))),
        }
    }
}
