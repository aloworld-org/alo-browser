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

use alo_dom::NodeId;
use alo_dom::activation::{self, Follows};

use crate::event_loop::Unqueued;
use crate::held::Held;
use crate::run_to::{Said, run_to};

/// What pressing a node on a page that runs script came to.
#[derive(Debug, Default)]
pub(crate) struct Pressed {
    /// Where the click said to go — the link's `href`, as written — if
    /// nobody cancelled it, its activation target is a link, and following
    /// the link started a navigation.
    pub(crate) follow: Option<String>,
    /// What the page's script said while the click ran.
    pub(crate) issues: Vec<String>,
}

/// Press `node` on a page whose document is in its heap: queue the click's
/// task and run the loop until it has run. [`None`] for a page that has never
/// run script, which `alo-agent`'s `apply` acts on instead.
pub(crate) fn press(held: &mut Held, node: NodeId) -> Option<Pressed> {
    let mut said = Said::about("the click");
    let mut follow = None;
    match held.activate(node) {
        Ok(Some(seq)) => {
            if let Some(turn) = run_to(held, seq, &mut said) {
                follow = turn
                    .clicked
                    .filter(|clicked| clicked.navigated)
                    .and_then(|clicked| href(clicked.follows));
            }
        }
        Ok(None) => return None,
        Err(Unqueued::Stopped(why)) => {
            let follows = held.change(|document| {
                let done = activation::before(document, node);
                activation::after(document, &done)
            });
            if let Some(Follows::Link { node: link, href }) = follows
                && held.follow(link) == Some(true)
            {
                follow = Some(href);
            }
            said.say(&format_args!("nobody heard it: {why}"));
        }
        Err(why @ (Unqueued::NoSuchNode(_) | Unqueued::NotADocument)) => said.say(&why),
    }
    Some(Pressed {
        follow,
        issues: said.lines(),
    })
}

/// The `href` of the link `follows` says to follow, if it says one.
fn href(follows: Follows) -> Option<String> {
    match follows {
        Follows::Link { href, .. } => Some(href),
        Follows::Nothing | Follows::InputAndChange(_) => None,
    }
}
