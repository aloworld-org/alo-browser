/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An agent's `Activate` on a list's option, on a page that runs script:
//! the option chosen and the list's `input` and `change` fired, run to its
//! end before the agent is answered (ADR 0064 § 2, queue item 659).
//!
//! The renderer has decided which option the verb names, against the tree
//! the agent read. On a page whose document is in its heap the option is
//! **not** chosen by `alo-agent`'s `apply`: it is one task on the page's
//! loop ([`Held::choose`]), and this runs that task ([`crate::run_to`]) and
//! says what its script said while it ran. Whether the option is chosen
//! after it is read from the document by the renderer, since a listener
//! may have changed it back.
//!
//! # A page whose script has stopped
//!
//! Its document is still in its heap, and nothing of it runs again (ADR
//! 0016 § 7) — whether it had stopped already, or stops during this task
//! before the option is chosen. The option is still chosen — that is
//! `alo-dom`'s rule, not the page's script, exactly as text still goes into
//! one of its fields ([`crate::put`]) — nobody is told, and the answer says
//! so.

use alo_dom::{NodeId, choice};

use crate::event_loop::Unqueued;
use crate::held::Held;
use crate::run_to::{Said, run_to};

/// Choose the option `option` of the list `list` on a page whose document
/// is in its heap: queue the task and run the loop until it has run, and
/// answer what the page's script said. [`None`] for a page that has never
/// run script, which `alo-agent`'s `apply` acts on instead.
pub(crate) fn choose(held: &mut Held, list: NodeId, option: NodeId) -> Option<Vec<String>> {
    let mut said = Said::about("the choice");
    match held.choose(list, option) {
        Ok(Some(seq)) => {
            let chose = run_to(held, seq, &mut said).is_some_and(|turn| turn.chose);
            // The page stopped before the option was chosen — in this task
            // or one before it, which said why. A stopped page's list still
            // takes a choice.
            if !chose {
                held.change(|document| choice::choose(document, option));
            }
        }
        Ok(None) => return None,
        Err(Unqueued::Stopped(why)) => {
            held.change(|document| choice::choose(document, option));
            said.say(&format_args!("nobody heard it: {why}"));
        }
        Err(why @ (Unqueued::NoSuchNode(_) | Unqueued::NotADocument)) => said.say(&why),
    }
    Some(said.lines())
}
