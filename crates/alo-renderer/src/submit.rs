/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An agent's `Activate` on a submit button, on a page that has never run
//! script (ADR 0051 §§ 4 and 6, queue item 468).
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
//! What the browser process then decides is its own (`crate::navigate`).
//!
//! On a page that runs script the same press fires `invalid` and `submit`
//! around these steps, which is queue item 484 ([`crate::press`]).

use alo_agent::verb::{Invalid, Outcome, Refusal};
use alo_bindings::navigating::{self, By, Ongoing};
use alo_box::tree::{BoxId, BoxKind, BoxTree};
use alo_dom::constraint::{self, Problem};
use alo_dom::{Document, NodeId, activation, submission};
use alo_url::Url;

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
    // An author who gave the button ARIA state asked for that to be toggled
    // instead (ADR 0018 § 7), as `alo-agent`'s `apply` did.
    if document
        .element(pressed.node)
        .is_some_and(|element| element.attr("aria-checked").is_some())
    {
        return None;
    }
    let target = activation::activation_target(document, pressed.node)?;
    let (form, submitter) = activation::submits(document, target)?;
    Some(submit(document, url, pressed, (form, submitter), ongoing))
}

/// Steps 1 to 3 for a form and the button it is submitted from.
fn submit(
    document: &Document,
    url: &Url,
    pressed: &Pressed<'_>,
    (form, submitter): (NodeId, NodeId),
    ongoing: &mut Ongoing,
) -> Result<Outcome, Refusal> {
    let (button, boxes) = (pressed.button, pressed.boxes);
    if !submission::novalidate(document, form, Some(submitter)) {
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
        if !problems.is_empty() {
            let invalid = problems
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
                .collect();
            return Ok(Outcome::NotSubmitted {
                node: button,
                invalid,
            });
        }
    }
    let plan = submission::plan(document, form, Some(submitter)).map_err(|why| {
        Refusal::NotSubmittable {
            node: button,
            why: why.to_string(),
        }
    })?;
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

/// The box drawn for the element `node`, if one is.
fn box_of(boxes: &BoxTree, node: NodeId) -> Option<BoxId> {
    boxes.ids().find(|id| {
        boxes.get(*id).is_some_and(
            |held| matches!(held.kind, BoxKind::Element { node: of, .. } if of == node),
        )
    })
}
