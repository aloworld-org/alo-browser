/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Throws a builtin asked to have reported rather than propagated (ADR 0018
//! § 3, queue item 254).
//!
//! `dispatchEvent` calls each listener and must carry on past one that
//! throws: HTML *reports the exception* and moves to the next listener. A
//! builtin is handed no engine and no report, so it asks for the call with
//! [`Want::Report`](crate::object::native::Want), and a throw nothing inside
//! that call catches **stops at it** ([`land`](super::Engine)) — the calls it
//! left are taken down, the builtin is answered `undefined`, and the throw is
//! set aside here, with where it had got to.
//!
//! The embedder is handed what was set aside **with the outer run's result**:
//! after [`Engine::run`] or [`Engine::call`] answers, by
//! [`Engine::hand_over_reported`], and inside a checkpoint after every job,
//! through the same report the checkpoint's own throws go to. Either way the
//! throws are said in the order they happened, before the outer run's own.
//!
//! # A thrown value is rooted while it waits
//!
//! A job's throw is reported *as it happens* because nothing roots a thrown
//! value once its call has gone. Here the throw has to outlive the call by
//! design — the builtin carries on and the run may allocate a great deal
//! before it ends — so a thrown object is held by a [`Root`] until it is
//! handed over, and the root is released the moment it has been.
//!
//! # A bound, and saying it was reached
//!
//! [`REPORTS_SET_ASIDE`](crate::bounds::REPORTS_SET_ASIDE) are kept between
//! two hand-overs; past that a throw is counted rather than kept, so a script
//! that dispatches to throwing listeners in a loop costs a counter rather
//! than a root and a trace per throw.
//!
//! [`Engine::run`]: super::Engine::run
//! [`Engine::call`]: super::Engine::call
//! [`Engine::hand_over_reported`]: super::Engine::hand_over_reported

use crate::abrupt::Thrown;
use crate::bounds;
use crate::heap::Root;
use crate::object::Objects;

use super::unwound::Unwound;

/// One throw set aside, the calls it left, and the root that keeps what was
/// thrown.
#[derive(Debug)]
struct Aside {
    thrown: Thrown,
    unwound: Unwound,
    root: Option<Root>,
}

/// The throws set aside since the last hand-over.
#[derive(Debug, Default)]
pub(super) struct SetAside {
    kept: Vec<Aside>,
    dropped: usize,
}

impl SetAside {
    /// Set `thrown` aside, rooting what it threw — or count it, past the
    /// bound.
    ///
    /// Allocates nothing in the heap: a root is an entry in the root list.
    pub(super) fn keep(&mut self, objects: &mut Objects, thrown: Thrown, unwound: Unwound) {
        if self.kept.len() >= bounds::REPORTS_SET_ASIDE {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        let root = match &thrown {
            Thrown::Value { value, .. } => {
                value.reference().map(|held| objects.heap_mut().root(held))
            }
            Thrown::Error { .. } => None,
        };
        self.kept.push(Aside {
            thrown,
            unwound,
            root,
        });
    }

    /// Hand each throw to `report`, oldest first, releasing its root after,
    /// and answer how many were counted rather than kept.
    pub(super) fn hand_over(
        &mut self,
        objects: &mut Objects,
        report: &mut dyn FnMut(&Objects, &Thrown, &Unwound),
    ) -> usize {
        for aside in self.kept.drain(..) {
            report(objects, &aside.thrown, &aside.unwound);
            if let Some(root) = aside.root {
                objects.heap_mut().release(root);
            }
        }
        core::mem::take(&mut self.dropped)
    }

    /// How many are waiting to be handed over, kept or counted.
    pub(super) fn waiting(&self) -> usize {
        self.kept.len().saturating_add(self.dropped)
    }
}
