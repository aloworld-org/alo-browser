/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A form submitted by a click, on a page that runs script (ADR 0051 §§ 4
//! and 6, queue items 485 and 543).
//!
//! An agent's `Activate` on a submit button is a click ([`super::activated`]),
//! and so is a person's press and release on one ([`super::pointed`]); a
//! click nobody cancelled submits the button's form owner. This is that
//! submission, run inside **the click's own task**, as HTML has it: the
//! listeners of each event here run before the agent or the person's
//! browser is answered, a checkpoint after every one. The steps are HTML's *form submission
//! algorithm*, `alo-dom`'s rules with the events between them:
//!
//! 1. **Not again.** A form already firing its submission events is not
//!    submitted ([`submitting`], HTML's first guard).
//! 2. **Checked**, unless the form says `novalidate` or the button
//!    `formnovalidate`. A trusted **`invalid`** is fired at each control
//!    that fails, in tree order — cancelable and not bubbling, as Chrome 154
//!    fires it ([`Firing::INVALID`]) — and if any failed, nothing more:
//!    [`Submitted::Invalid`] names each. A listener's `preventDefault` on an
//!    `invalid` only keeps the browser from reporting it, which this engine
//!    has no bubble to do; the form is still not sent.
//! 3. **`submit`** is fired at the form: a trusted `SubmitEvent`, bubbling
//!    and cancelable, whose `submitter` is the button ([`Firing::submit`]) —
//!    trusted whoever pressed, because the browser fires it. A listener that
//!    cancels it keeps the form from being sent ([`Submitted::Canceled`]),
//!    which is how the contact form sends itself by `fetch` instead. The
//!    form's flag is set from before step 2 until this has been dispatched.
//! 4. **Still in its document**, or nothing: HTML's *cannot navigate*, for a
//!    form a listener took away ([`Submitted::Withdrawn`]).
//! 5. **Planned and asked**: the entry list, built *after* the listeners so
//!    that what they changed is sent, and the ask kept in the document cell
//!    as the page's ongoing navigation ([`navigating::start_submission`]),
//!    so a later link or submission in the same task replaces it.
//!
//! `formdata`, which HTML fires between 4 and 5, needs `FormData`, and
//! arrives with it (queue item 472).
//!
//! # A constraint or a form this engine cannot check
//!
//! For an agent, the renderer refuses a press by name **before** the click
//! is queued when the form could fail a constraint not built, or says
//! something this engine does not send ([`crate::submit`]): a refusal means
//! nothing happened. So for an agent [`Submitted::Unchecked`] and
//! [`Submitted::Unsubmittable`] are only what a click's listener made true
//! after that look — a value it put in, an attribute it set — and are said
//! rather than refused, since by then the click has been heard.
//!
//! A person's click is never refused: the person pressed, and the page
//! hears it whatever this engine can send. For a person both are what the
//! form came to, said rather than sent ([`crate::sent`]).

use alo_bindings::navigating::{self, By};
use alo_bindings::{Firing, document, submitting};
use alo_dom::NodeId;
use alo_dom::constraint::{self, Constraint, Problem};
use alo_dom::submission::{self, Method};
use alo_js::heap::Ref;
use alo_js::{Escape, Fault, Root};

use super::task;
use super::{EventLoop, Turn};

/// One control that failed its form's check, and Chrome's words for why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failed {
    /// The control, which `invalid` was fired at.
    pub control: NodeId,
    /// The first constraint it fails.
    pub constraint: Constraint,
    /// What Chrome 154 says, word for word.
    pub message: String,
}

/// What submitting a form from a click came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Submitted {
    /// The form was already firing its submission events, and so was not
    /// submitted again (HTML's guard).
    AlreadySubmitting,
    /// A control could fail a constraint this engine does not check yet, so
    /// nothing was fired and nothing sent.
    Unchecked {
        /// The control.
        control: NodeId,
        /// The constraint.
        constraint: Constraint,
    },
    /// Controls failed their constraints, `invalid` was fired at each, and
    /// nothing was sent.
    Invalid(Vec<Failed>),
    /// A listener cancelled `submit`, and nothing was sent.
    Canceled,
    /// A listener took the form out of its document, and nothing was sent.
    Withdrawn,
    /// The form says something this engine does not send, and why.
    Unsubmittable(String),
    /// The submission was asked for.
    Asked {
        /// How.
        method: Method,
        /// Whether it started a navigation: `false` for an action that
        /// resolves to nothing, which the document cell says why of.
        navigated: bool,
    },
}

impl EventLoop {
    /// Submit `form` from `submitter`, in the document `cell` holds, within
    /// the click's task: the events are fired at wrappers added to its
    /// rooted `list`.
    pub(super) fn submit(
        &mut self,
        list: &Root,
        cell: Ref,
        (form, submitter): (NodeId, NodeId),
        turn: &mut Turn,
    ) -> Result<Submitted, Escape> {
        match submitting::begin(self.engine.objects(), cell, form) {
            Some(true) => {}
            Some(false) => return Ok(Submitted::AlreadySubmitting),
            None => return Err(Escape::fault(Fault::NotAnObject)),
        }
        let fired = self.fire_submission(list, cell, (form, submitter), turn);
        // Unset whatever the listeners did, a throw that stopped the page
        // included: the flag is the dispatch's, and the dispatch is over.
        submitting::end(self.engine.objects(), cell, form);
        if let Some(stopped) = fired? {
            return Ok(stopped);
        }

        let read =
            document(self.engine.objects(), cell).ok_or(Escape::fault(Fault::NotAnObject))?;
        if !read.is_attached(form) {
            return Ok(Submitted::Withdrawn);
        }
        let plan = match submission::plan(read, form, Some(submitter)) {
            Ok(plan) => plan,
            Err(why) => return Ok(Submitted::Unsubmittable(why.to_string())),
        };
        let method = plan.method;
        let navigated =
            navigating::start_submission(self.engine.objects(), cell, plan, By::Browser)
                .ok_or(Escape::fault(Fault::NotAnObject))?;
        Ok(Submitted::Asked { method, navigated })
    }

    /// Steps 2 and 3, with the form's flag set: what stopped the submission,
    /// or [`None`] when nothing did and it goes on to be asked for.
    fn fire_submission(
        &mut self,
        list: &Root,
        cell: Ref,
        (form, submitter): (NodeId, NodeId),
        turn: &mut Turn,
    ) -> Result<Option<Submitted>, Escape> {
        let read =
            document(self.engine.objects(), cell).ok_or(Escape::fault(Fault::NotAnObject))?;
        if !submission::novalidate(read, form, Some(submitter)) {
            let mut failed = Vec::new();
            for (control, problem) in constraint::problems(read, form) {
                match problem {
                    Problem::Unchecked { constraint } => {
                        return Ok(Some(Submitted::Unchecked {
                            control,
                            constraint,
                        }));
                    }
                    Problem::Fails {
                        constraint,
                        message,
                    } => failed.push(Failed {
                        control,
                        constraint,
                        message,
                    }),
                }
            }
            if !failed.is_empty() {
                for each in &failed {
                    self.awake()?;
                    let (target, event) =
                        task::add(&mut self.engine, list, cell, each.control, &Firing::INVALID)
                            .map_err(task::Unmade::into_escape)?;
                    self.dispatch_event(event, target, turn)?;
                }
                return Ok(Some(Submitted::Invalid(failed)));
            }
        }

        self.awake()?;
        let button = task::hold(self.engine.objects(), list, cell, submitter)
            .map_err(task::Unmade::into_escape)?;
        let (target, event) =
            task::add(&mut self.engine, list, cell, form, &Firing::submit(button))
                .map_err(task::Unmade::into_escape)?;
        if self.dispatch_event(event, target, turn)? {
            return Ok(Some(Submitted::Canceled));
        }
        Ok(None)
    }
}
