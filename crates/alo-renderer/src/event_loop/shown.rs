/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page told whether it can be seen, as a task (ADR 0039 § 1, queue item
//! 364).
//!
//! HTML's *update the visibility state*: if the document already has the
//! state it is told, nothing happens and nothing is fired; otherwise the
//! state is set and `visibilitychange` is fired at the document — trusted,
//! bubbling, not cancellable — and its path ends at the window, as every
//! path from the document does (ADR 0037 § 3).
//!
//! **Both halves are the task's.** The state is compared and set when the
//! task runs, not when it is queued, so that two changes waiting in the
//! queue are each judged against the state the one before it left: hidden
//! twice is one `visibilitychange`, wherever the second was queued.
//!
//! The event and the document's wrapper are made when the task is queued,
//! as every dispatch's are ([`super::task::listed`]), and a task that finds
//! the state unchanged lets them go unfired.

use alo_bindings::{Firing, Visibility, document, node_of, visibility};
use alo_js::heap::Ref;
use alo_js::{Escape, Fault, Root};

use super::task::{self, Seq, Work};
use super::{EventLoop, Turn, Unqueued};

impl EventLoop {
    /// Queue the task that tells the document `cell` holds its visibility
    /// state is `to`, and fires `visibilitychange` at it if that is a
    /// change.
    ///
    /// `cell` must be rooted by the caller.
    ///
    /// # Errors
    ///
    /// As [`EventLoop::queue_dispatch`].
    pub fn queue_visibility(&mut self, cell: Ref, to: Visibility) -> Result<Seq, Unqueued> {
        if let Some(stopped) = &self.stopped {
            return Err(Unqueued::Stopped(stopped.clone()));
        }
        let root = document(self.objects(), cell)
            .map(alo_dom::Document::root)
            .ok_or(Unqueued::NotADocument)?;
        self.queue_listed(cell, root, &Firing::VISIBILITY_CHANGE, |list| {
            Work::Visibility { list, to }
        })
    }

    /// Run a [`Work::Visibility`](task::Work::Visibility) task: set the
    /// state, and fire `visibilitychange` at the document if it changed.
    pub(super) fn shown(
        &mut self,
        list: &Root,
        to: Visibility,
        turn: &mut Turn,
    ) -> Result<(), Escape> {
        let (event, target) = task::dispatched(&mut self.engine, list)?;
        // The task made the document's wrapper, so it names a document cell.
        let (cell, _) =
            node_of(self.engine.objects(), target).ok_or(Escape::fault(Fault::NotAnObject))?;
        let changed = visibility::update(self.engine.objects(), cell, to)
            .ok_or(Escape::fault(Fault::NotAnObject))?;
        if changed {
            self.dispatch_event(event, target, turn)?;
        }
        Ok(())
    }
}
