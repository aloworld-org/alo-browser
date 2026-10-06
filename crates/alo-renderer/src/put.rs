/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An agent's `PutText` on a page that runs script: the text offered, put
//! and announced, run to its end before the agent is answered (ADR 0018
//! §§ 3–5, queue item 257).
//!
//! The renderer has decided which field the verb names, against the tree the
//! agent read. On a page whose document is in its heap the text is **not**
//! put in by `alo-agent`'s `apply`: it is one task on the page's loop
//! ([`Held::put_text`]) — a `beforeinput` the page may cancel, then the
//! text, `input` and `change` — and this runs that task
//! ([`crate::run_to`]) and says what it came to: whether the page took the
//! text, and what its script said while it ran.
//!
//! # A page whose script has stopped
//!
//! Its document is still in its heap, and nothing of it runs again (ADR 0016
//! § 7) — whether it had stopped already, or stops during this task before
//! it has answered the `beforeinput`. Text put into one of its fields still
//! goes in — that is `alo-dom`'s
//! rule, not the page's script, exactly as a click on it still ticks a box
//! ([`crate::press`]) — nobody is asked or told, and the answer says so.

use alo_dom::{NodeId, field};

use crate::event_loop::{Typed, Unqueued};
use crate::held::Held;
use crate::run_to::{Said, run_to};

/// What putting text into a field on a page that runs script came to.
#[derive(Debug, Default)]
pub(crate) struct Put {
    /// Whether a listener cancelled the `beforeinput`, so that the field is
    /// as it was.
    pub(crate) canceled: bool,
    /// What the page's script said while the task ran.
    pub(crate) issues: Vec<String>,
}

/// Put `text` into the field `node` on a page whose document is in its
/// heap: queue the task and run the loop until it has run. [`None`] for a
/// page that has never run script, which `alo-agent`'s `apply` acts on
/// instead.
pub(crate) fn put(held: &mut Held, node: NodeId, text: &str) -> Option<Put> {
    let mut said = Said::about("the text");
    let mut canceled = false;
    match held.put_text(node, text) {
        Ok(Some(seq)) => match run_to(held, seq, &mut said).and_then(|turn| turn.typed) {
            Some(Typed::Canceled) => canceled = true,
            Some(Typed::Put) => {}
            // The page stopped before it answered the `beforeinput` — in
            // this task or one before it, which said why. Nobody refused,
            // and a stopped page's field still takes text.
            None => {
                held.change(|document| field::put_text(document, node, text));
            }
        },
        Ok(None) => return None,
        Err(Unqueued::Stopped(why)) => {
            held.change(|document| field::put_text(document, node, text));
            said.say(&format_args!("nobody heard it: {why}"));
        }
        Err(why @ (Unqueued::NoSuchNode(_) | Unqueued::NotADocument)) => said.say(&why),
    }
    Some(Put {
        canceled,
        issues: said.lines(),
    })
}
