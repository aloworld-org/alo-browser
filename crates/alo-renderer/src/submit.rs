/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An agent's `Activate` on a submit button (ADR 0051 §§ 4 and 6, queue
//! items 468 and 485).
//!
//! # On a page that has never run script
//!
//! The press is HTML's form submission algorithm with nobody listening, so
//! nobody can cancel it, and its steps are `alo-dom`'s:
//!
//! 1. **checked**, unless the form says `novalidate` or the button
//!    `formnovalidate` ([`alo_dom::constraint`]). A control that could fail
//!    a constraint not built stops the press, refused by name
//!    ([`Refusal::Unchecked`]); one that fails a constraint that is built
//!    stops it as an outcome, [`Outcome::NotSubmitted`], naming each control
//!    and Chrome's words for it;
//! 2. **planned** — the entry list, the method, the action and the
//!    enctype ([`alo_dom::submission`]), or a refusal by name for what is
//!    not built ([`Refusal::NotSubmittable`]);
//! 3. **asked**, as the page's ongoing navigation, the one a link records
//!    (ADR 0020 § 1), with its action resolved against the document's base
//!    URL. The answer is [`Outcome::Submitted`], with how and where to.
//!
//! What the browser process then decides is its own (`crate::navigate`). A
//! page whose script has **stopped** is pressed the same way: nobody is left
//! to hear it.
//!
//! # On a page that runs script
//!
//! The press is a click the page's listeners hear, and the submission runs
//! inside the click's task with `invalid` and `submit` between its steps
//! (`crate::event_loop`'s `submitted`). Two things stay here:
//!
//! - **what refuses the press before the click is queued** ([`refused`]): a
//!   constraint this engine cannot check, or a form it does not send — the
//!   same refusals by name, decided on the document as the agent read it,
//!   so that a refusal still means nothing happened;
//! - **what the agent is told** of what the submission came to ([`told`]).

use alo_agent::verb::{Invalid, Outcome, Refusal};
use alo_bindings::navigating::{self, By, Ongoing};
use alo_box::tree::{BoxId, BoxKind, BoxTree};
use alo_dom::constraint::{self, Problem};
use alo_dom::submission::Unsubmittable;
use alo_dom::{Document, NodeId, activation, submission};
use alo_url::Url;

use crate::event_loop::{Failed, Submitted};

/// What was pressed: its box, its node, and what it is called.
pub(crate) struct Pressed<'a> {
    /// The box the agent named.
    pub(crate) button: BoxId,
    /// The node it came from.
    pub(crate) node: NodeId,
    /// What it is called, for an answer that it was only pressed.
    pub(crate) name: Option<String>,
    /// The boxes it is among.
    pub(crate) boxes: &'a BoxTree,
}

/// Press what `pressed` names, in `document` at `url`, and say what came of
/// it: [`None`] when it submits no form, which is answered as the press it
/// was.
///
/// A navigation it asks for is kept in `ongoing`, replacing any before it.
pub(crate) fn press(
    document: &Document,
    url: &Url,
    pressed: &Pressed<'_>,
    ongoing: &mut Ongoing,
) -> Option<Result<Outcome, Refusal>> {
    let submitting = submits(document, pressed)?;
    Some(submit(document, url, pressed, submitting, ongoing))
}

/// What refuses a press on a page that runs script before its click is
/// queued, decided on `document` as the agent read it: a control that could
/// fail a constraint not built, or — when no control fails one that is — a
/// form this engine does not send. [`None`] for a press that submits no form
/// or that the click may go on to submit.
pub(crate) fn refused(document: &Document, pressed: &Pressed<'_>) -> Option<Refusal> {
    let (form, submitter) = submits(document, pressed)?;
    match checked(document, pressed, (form, submitter)) {
        Err(refusal) => Some(refusal),
        // The click will fire `invalid` at each, which is not a refusal.
        Ok(Some(_)) => None,
        Ok(None) => submission::plan(document, form, Some(submitter))
            .err()
            .map(|why| unsubmittable(pressed.button, &why)),
    }
}

/// What an agent is told of `submitted`, what its press's click came to on a
/// page that runs script, with the navigation the page asked for meanwhile
/// in `ongoing`: the outcome, and a line to say when the form was not sent
/// for a reason the outcome cannot carry.
pub(crate) fn told(
    submitted: Submitted,
    pressed: &Pressed<'_>,
    ongoing: &Ongoing,
) -> (Outcome, Option<String>) {
    let node = pressed.button;
    let activated = || Outcome::Activated {
        node,
        name: pressed.name.clone(),
    };
    let not_sent = |why: &str| Some(format!("{node}'s form was not sent: {why}"));
    match submitted {
        Submitted::Invalid(failed) => (
            Outcome::NotSubmitted {
                node,
                invalid: failed
                    .into_iter()
                    .map(
                        |Failed {
                             control,
                             constraint,
                             message,
                         }| Invalid {
                            control: box_of(pressed.boxes, control),
                            constraint,
                            message,
                        },
                    )
                    .collect(),
            },
            None,
        ),
        Submitted::Asked {
            method,
            navigated: true,
        } => match ongoing.last() {
            Some(asked) => (
                Outcome::Submitted {
                    node,
                    method,
                    to: asked.url.serialised.clone(),
                },
                None,
            ),
            None => (activated(), None),
        },
        // The action went nowhere, which the answer's lines say.
        Submitted::Asked { .. } => (activated(), None),
        Submitted::Canceled => (
            activated(),
            not_sent("the page cancelled its submit event, and took the press"),
        ),
        Submitted::Withdrawn => (
            activated(),
            not_sent("the page took the form out of its document as it was submitted"),
        ),
        Submitted::AlreadySubmitting => (
            activated(),
            not_sent("it was already being submitted, and a form is not submitted again"),
        ),
        Submitted::Unchecked {
            control,
            constraint,
        } => {
            let refusal = Refusal::Unchecked {
                node,
                control: box_of(pressed.boxes, control),
                constraint,
            };
            (activated(), Some(refusal.to_string()))
        }
        Submitted::Unsubmittable(why) => (activated(), not_sent(&why)),
    }
}

/// The form a press on what `pressed` names submits, and the button that
/// submits it, if it submits one.
fn submits(document: &Document, pressed: &Pressed<'_>) -> Option<(NodeId, NodeId)> {
    // An author who gave the button ARIA state asked for that to be toggled
    // instead (ADR 0018 § 7), as `alo-agent`'s `apply` did.
    if document
        .element(pressed.node)
        .is_some_and(|element| element.attr("aria-checked").is_some())
    {
        return None;
    }
    let target = activation::activation_target(document, pressed.node)?;
    activation::submits(document, target)
}

/// Step 1: the form's controls checked, unless it says not to. A control
/// that could fail a constraint not built is a refusal; those that fail one
/// that is are answered, each with Chrome's words; [`None`] when nothing
/// stops the form.
fn checked(
    document: &Document,
    pressed: &Pressed<'_>,
    (form, submitter): (NodeId, NodeId),
) -> Result<Option<Vec<Invalid>>, Refusal> {
    if submission::novalidate(document, form, Some(submitter)) {
        return Ok(None);
    }
    let (button, boxes) = (pressed.button, pressed.boxes);
    let problems = constraint::problems(document, form);
    if let Some((control, constraint)) =
        problems
            .iter()
            .find_map(|(control, problem)| match problem {
                Problem::Unchecked { constraint } => Some((*control, *constraint)),
                Problem::Fails { .. } => None,
            })
    {
        return Err(Refusal::Unchecked {
            node: button,
            control: box_of(boxes, control),
            constraint,
        });
    }
    if problems.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        problems
            .into_iter()
            .filter_map(|(control, problem)| match problem {
                Problem::Fails {
                    constraint,
                    message,
                } => Some(Invalid {
                    control: box_of(boxes, control),
                    constraint,
                    message,
                }),
                Problem::Unchecked { .. } => None,
            })
            .collect(),
    ))
}

/// Steps 1 to 3 for a form and the button it is submitted from.
fn submit(
    document: &Document,
    url: &Url,
    pressed: &Pressed<'_>,
    (form, submitter): (NodeId, NodeId),
    ongoing: &mut Ongoing,
) -> Result<Outcome, Refusal> {
    let button = pressed.button;
    if let Some(invalid) = checked(document, pressed, (form, submitter))? {
        return Ok(Outcome::NotSubmitted {
            node: button,
            invalid,
        });
    }
    let plan = submission::plan(document, form, Some(submitter))
        .map_err(|why| unsubmittable(button, &why))?;
    let method = plan.method;
    if ongoing.start(navigating::submit(document, url, plan, By::Browser))
        && let Some(asked) = ongoing.last()
    {
        return Ok(Outcome::Submitted {
            node: button,
            method,
            to: asked.url.serialised.clone(),
        });
    }
    // The action went nowhere, which the answer's lines say.
    Ok(Outcome::Activated {
        node: button,
        name: pressed.name.clone(),
    })
}

/// The refusal for a form this engine does not send, pressed at `button`.
fn unsubmittable(button: BoxId, why: &Unsubmittable) -> Refusal {
    Refusal::NotSubmittable {
        node: button,
        why: why.to_string(),
    }
}

/// The box drawn for the element `node`, if one is.
fn box_of(boxes: &BoxTree, node: NodeId) -> Option<BoxId> {
    boxes.ids().find(|id| {
        boxes.get(*id).is_some_and(
            |held| matches!(held.kind, BoxKind::Element { node: of, .. } if of == node),
        )
    })
}
