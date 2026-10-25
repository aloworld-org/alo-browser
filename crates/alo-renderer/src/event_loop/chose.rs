/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An agent's `Activate` on a list's option, as a task (ADR 0064 § 2, queue
//! item 659).
//!
//! Activating an option chooses it — makes it its list's choice, or toggles
//! it in a list of several — and the page hears what it hears from a
//! person, all of it **one task** (ADR 0016 § 6), each event dispatched as
//! [`dispatched`](super::dispatched) dispatches any event the browser makes,
//! a checkpoint after every listener:
//!
//! 1. the option is chosen — `alo-dom`'s rule ([`alo_dom::choice`]), the
//!    one `alo-agent`'s `apply` runs on a page that never ran script;
//! 2. if that changed what the list holds, an **`input`** is fired at the
//!    list ([`Firing::INPUT`]: an `Event`, bubbling and composed), then a
//!    **`change`** ([`Firing::CHANGE`]), as HTML's *send select update
//!    notifications* fires them. Choosing what the list already holds
//!    changes nothing and fires nothing, as a person's click on it does.
//!
//! There is no event before the choice for a listener to cancel: a
//! person's choice is not asked about either. A listener on `change` may
//! put the old choice back, and the agent's answer is read from the page
//! after the task ([`alo_agent::chosen_after`]), so it then says the option
//! is not chosen.
//!
//! # Said as it happens
//!
//! The task writes into its [`Turn::chose`] the moment the option is
//! chosen, because a page can stop partway — a full heap, the embedder's
//! stop — and whoever carries the verb out must know whether the choice
//! was made. `false` means the page stopped before it was.

use alo_bindings::{Firing, change_document, node_of};
use alo_dom::{NodeId, choice, value};
use alo_js::{Escape, Fault, Root};

use super::task;
use super::{EventLoop, Turn};

impl EventLoop {
    /// Run a [`Work::Choose`](task::Work::Choose) task: the option chosen,
    /// then the `input` already in `list` and a `change`, when what the
    /// list holds changed — whether it was chosen written into `turn` as it
    /// happens.
    pub(super) fn choose(
        &mut self,
        list: &Root,
        option: NodeId,
        turn: &mut Turn,
    ) -> Result<(), Escape> {
        let (input, target) = task::dispatched(&mut self.engine, list)?;
        let (cell, select) =
            node_of(self.engine.objects(), target).ok_or(Escape::fault(Fault::NotAnObject))?;

        let changed = change_document(self.engine.objects(), cell, |document| {
            let before = value::selected(document, select);
            choice::choose(document, option)?;
            Some(value::selected(document, select) != before)
        })
        .flatten()
        // The task was queued for an option of this list.
        .ok_or(Escape::fault(Fault::NotAnObject))?;
        turn.chose = true;
        if !changed {
            return Ok(());
        }
        self.dispatch_event(input, target, turn)?;
        self.awake()?;
        let (target, event) = task::add(&mut self.engine, list, cell, select, &Firing::CHANGE)
            .map_err(task::Unmade::into_escape)?;
        self.dispatch_event(event, target, turn)?;
        Ok(())
    }
}
