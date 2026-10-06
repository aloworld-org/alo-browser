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
//! the page's listeners decide what it does. This runs that task — and any
//! task queued before it, oldest first, since one order holds across all of
//! them (ADR 0016 § 2) — and says what it came to: whether a link is to be
//! followed, and what the page's script said while it ran.
//!
//! # A page whose script has stopped
//!
//! Its document is still in its heap, and nothing of it runs again (ADR 0016
//! § 7). A click on it still does what a click does to the document — a box
//! is still ticked, a link still followed — because that is `alo-dom`'s rule,
//! not the page's script; nobody hears it, and the answer says so. It does
//! not change ARIA state either: that promise was the page's (ADR 0018 § 7).
//!
//! # How much it says
//!
//! What one `Act` says crosses in one message, so it is bounded as a load's
//! is ([`crate::scripts`]): at most [`MOST_REPORTS`] lines, each at most
//! [`LONGEST_LINE`](crate::said::LONGEST_LINE) characters, and then how many
//! more there were.

use alo_dom::NodeId;
use alo_dom::activation::{self, Follows};

use crate::event_loop::{MOST_REPORTS, Unqueued};
use crate::held::Held;
use crate::said;

/// What pressing a node on a page that runs script came to.
#[derive(Debug, Default)]
pub(crate) struct Pressed {
    /// Where the click said to go, if nobody cancelled it and its
    /// activation target is a link.
    pub(crate) follow: Option<String>,
    /// What the page's script said while the click ran.
    pub(crate) issues: Vec<String>,
}

impl Pressed {
    /// Say `what` about the click, if there is room, or count it.
    fn say(&mut self, what: &dyn core::fmt::Display, left_out: &mut usize) {
        if self.issues.len() < MOST_REPORTS {
            self.issues
                .push(said::line(&format_args!("the click: {what}")));
        } else {
            *left_out = left_out.saturating_add(1);
        }
    }
}

/// Press `node` on a page whose document is in its heap: queue the click's
/// task and run the loop until it has run. [`None`] for a page that has never
/// run script, which `alo-agent`'s `apply` acts on instead.
pub(crate) fn press(held: &mut Held, node: NodeId) -> Option<Pressed> {
    let mut pressed = Pressed::default();
    let mut left_out = 0_usize;
    let seq = match held.activate(node) {
        Ok(Some(seq)) => seq,
        Ok(None) => return None,
        Err(Unqueued::Stopped(why)) => {
            let follows = held.change(|document| {
                let done = activation::before(document, node);
                activation::after(document, &done)
            });
            pressed.follow = followed(follows);
            pressed.say(&format_args!("nobody heard it: {why}"), &mut left_out);
            return Some(pressed);
        }
        Err(why @ (Unqueued::NoSuchNode(_) | Unqueued::NotADocument)) => {
            pressed.say(&why, &mut left_out);
            return Some(pressed);
        }
    };
    let Some(page_loop) = held.event_loop() else {
        return Some(pressed);
    };
    let room = |pressed: &Pressed| MOST_REPORTS.saturating_sub(pressed.issues.len());
    while let Some(turn) = page_loop.run_next_within(room(&pressed)) {
        for report in &turn.reports {
            pressed.say(report, &mut left_out);
        }
        left_out = left_out.saturating_add(turn.unreported);
        if let Some(stopped) = &turn.stopped {
            pressed.say(stopped, &mut left_out);
        }
        if turn.task == seq {
            pressed.follow = followed(turn.clicked.map(|clicked| clicked.follows));
            break;
        }
    }
    if left_out > 0 {
        pressed.issues.push(format!(
            "{left_out} more things the click's script said were not said: one action says at \
             most {MOST_REPORTS}"
        ));
    }
    Some(pressed)
}

/// Where `follows` says to go, if anywhere.
fn followed(follows: Option<Follows>) -> Option<String> {
    match follows {
        Some(Follows::Link { href, .. }) => Some(href),
        Some(Follows::Nothing | Follows::InputAndChange(_)) | None => None,
    }
}
