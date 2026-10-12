/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An agent's `Activate` on a page that runs script: the click, run to its
//! end before the agent is answered (ADR 0018 §§ 3–7, queue item 256).
//!
//! The renderer has decided what the verb is aimed at, against the tree the
//! agent read. On a page whose document is in its heap that decision is
//! **not** carried into the document by `alo-agent`'s `apply`: it is a
//! `click`, queued as one task on the page's loop ([`Held::activate`]), and
//! the page's listeners decide what it does. This runs that task
//! ([`crate::run_to`]) and says what it came to: whether a link was
//! followed — asked for in the document cell, beside whatever the page's
//! script asked for (ADR 0020 § 1) — and what the page's script said while
//! it ran.
//!
//! # A page whose script has stopped
//!
//! Its document is still in its heap, and nothing of it runs again (ADR 0016
//! § 7). A click on it still does what a click does to the document — a box
//! is still ticked, a link still followed — because that is `alo-dom`'s rule,
//! not the page's script; nobody hears it, and the answer says so. It does
//! not change ARIA state either: that promise was the page's (ADR 0018 § 7).
//! A submit button's form on it is submitted as on a page that never ran
//! script ([`crate::submit`]), since nobody is left to cancel anything.
//!
//! # A submit button, on a page that runs script
//!
//! Its click's task submits its form, with `invalid` at each failing control
//! or a `SubmitEvent` the page may cancel before anything is asked (ADR 0051
//! § 4, queue item 485), and what that came to is answered here for the
//! renderer to tell the agent ([`crate::submit`]'s `told`).

use alo_agent::verb::{Outcome, Refusal};
use alo_bindings::navigating::Ongoing;
use alo_dom::activation::{self, Follows};
use alo_dom::{Document, NodeId};
use alo_url::Url;

use crate::event_loop::{Submitted, Unqueued};
use crate::held::Held;
use crate::run_to::{Said, run_to};
use crate::said;
use crate::submit;

/// What an agent's verb came to on a page that runs script, before the
/// page's navigation is taken.
#[derive(Debug)]
pub(crate) enum Ran {
    /// A press, told once what the page asked for is known ([`told`]).
    Pressed(Pressed),
    /// Anything else, already told, and what the page's script said.
    Told(Outcome, Vec<String>),
}

/// What pressing a node on a page that runs script came to.
#[derive(Debug, Default)]
pub(crate) struct Pressed {
    /// Where the click said to go — the link's `href`, as written — if
    /// nobody cancelled it, its activation target is a link, and following
    /// the link started a navigation.
    pub(crate) follow: Option<String>,
    /// What the page's script said while the click ran.
    pub(crate) issues: Vec<String>,
    /// What submitting a form came to, if the click submitted one.
    pub(crate) submitted: Option<Submitted>,
    /// Whether the click was on a submit button of a page whose script has
    /// stopped, whose form is then submitted as a page that never ran
    /// script submits it.
    pub(crate) submits_without_script: bool,
}

/// Press `node` on a page whose document is in its heap: queue the click's
/// task and run the loop until it has run. [`None`] for a page that has never
/// run script, which `alo-agent`'s `apply` acts on instead.
pub(crate) fn press(held: &mut Held, node: NodeId) -> Option<Pressed> {
    let mut said = Said::about("the click");
    let mut follow = None;
    let mut submitted = None;
    let mut submits_without_script = false;
    match held.activate(node) {
        Ok(Some(seq)) => {
            if let Some(clicked) = run_to(held, seq, &mut said).and_then(|turn| turn.clicked) {
                submitted = clicked.submitted;
                if clicked.navigated {
                    follow = href(clicked.follows);
                }
            }
        }
        Ok(None) => return None,
        Err(Unqueued::Stopped(why)) => {
            let follows = held.change(|document| {
                let done = activation::before(document, node);
                activation::after(document, &done)
            });
            match follows {
                Some(Follows::Link { node: link, href }) if held.follow(link) == Some(true) => {
                    follow = Some(href);
                }
                Some(Follows::Submit { .. }) => submits_without_script = true,
                _ => {}
            }
            said.say(&format_args!("nobody heard it: {why}"));
        }
        Err(why @ (Unqueued::NoSuchNode(_) | Unqueued::NotADocument)) => said.say(&why),
    }
    Some(Pressed {
        follow,
        issues: said.lines(),
        submitted,
        submits_without_script,
    })
}

/// What the agent is told of `pressed`, a press on `on` in the page at
/// `url` whose document is now `document`, with what the page asked for
/// in `ongoing`: a link followed, a form's submission as it came out
/// ([`submit::told`]), or the press it was — and on a page whose script has
/// stopped, its form pressed as a scriptless page's is ([`submit::press`]).
///
/// # Errors
///
/// That form's refusal, by name.
pub(crate) fn told(
    pressed: Pressed,
    on: &submit::Pressed<'_>,
    document: Option<&Document>,
    url: &Url,
    ongoing: &mut Ongoing,
) -> Result<(Outcome, Vec<String>), Refusal> {
    let mut issues = pressed.issues;
    if let Some(to) = pressed.follow {
        return Ok((
            Outcome::Followed {
                node: on.button,
                to,
            },
            issues,
        ));
    }
    let activated = Outcome::Activated {
        node: on.button,
        name: on.name.clone(),
    };
    if let Some(submitted) = pressed.submitted {
        let (outcome, line) = submit::told(submitted, on, ongoing);
        issues.extend(line.map(|line| said::line(&line)));
        return Ok((outcome, issues));
    }
    if pressed.submits_without_script
        && let Some(answer) =
            document.and_then(|document| submit::press(document, url, on, ongoing))
    {
        return answer.map(|outcome| (outcome, issues));
    }
    Ok((activated, issues))
}

/// The `href` of the link `follows` says to follow, if it says one.
fn href(follows: Follows) -> Option<String> {
    match follows {
        Follows::Link { href, .. } => Some(href),
        Follows::Nothing
        | Follows::InputAndChange(_)
        | Follows::Toggled { .. }
        | Follows::Submit { .. } => None,
    }
}
