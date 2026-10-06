/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A script's `el.click()` in progress (ADR 0018 § 6, queue item 261).
//!
//! HTML gives every element a **click in progress flag**: set while its
//! `click()` dispatches, so a second `click()` from inside one of that
//! click's listeners does nothing. Here the flag is having a [`Clicking`],
//! which the element's wrapper holds ([`crate::Wrapper`]) — the element whose
//! `click()` was called, which is the `this` of that call and so is wrapped
//! already.
//!
//! # And it is where the native keeps what it made
//!
//! A native keeps a step number across a listener and nothing else
//! (`alo_js::object::native`), and `click()` makes the events it dispatches
//! — the click, then the `input` and `change` a toggled box fires — and must
//! remember what the click's pre-activation changed, to put it back if a
//! listener cancels. All three are held here, the events as strong edges of
//! the wrapper, so a collection inside any listener keeps them.

use alo_dom::NodeId;
use alo_dom::activation::Activation;
use alo_js::heap::{Barrier, Field, Ref, Tracer};

/// One `click()` in progress at an element.
#[derive(Debug)]
pub struct Clicking {
    /// The event being dispatched now: the click, then `input`, then
    /// `change`.
    event: Field,
    /// The wrapper the `input` and `change` are fired at — the activation
    /// target, which may be an ancestor of the element clicked.
    target: Field,
    /// What the click's pre-activation did, to undo or finish.
    activation: Activation,
}

impl Clicking {
    /// A click in progress, dispatching `event`, nothing activated yet.
    pub(crate) fn new(barrier: &mut Barrier, event: Ref) -> Self {
        let mut held = Field::empty();
        held.set(barrier, Some(event));
        Self {
            event: held,
            target: Field::empty(),
            activation: Activation::None,
        }
    }

    /// The event being dispatched now.
    pub const fn event(&self) -> Option<Ref> {
        self.event.get()
    }

    /// Where `input` and `change` are fired, once they are.
    pub const fn target(&self) -> Option<Ref> {
        self.target.get()
    }

    /// What the pre-activation did.
    pub const fn activation(&self) -> &Activation {
        &self.activation
    }

    /// Remember what the pre-activation did.
    pub(crate) fn activated(&mut self, activation: Activation) {
        self.activation = activation;
    }

    /// Dispatch `event` at `target` next.
    pub(crate) fn firing(&mut self, barrier: &mut Barrier, target: Ref, event: Ref) {
        self.target.set(barrier, Some(target));
        self.event.set(barrier, Some(event));
    }

    /// Drop its references, as the click ends.
    pub(crate) fn let_go(&mut self, barrier: &mut Barrier) {
        self.event.set(barrier, None);
        self.target.set(barrier, None);
    }

    /// Its references, as edges of the wrapper that holds it.
    pub fn trace(&self, tracer: &mut Tracer) {
        self.event.trace(tracer);
        self.target.trace(tracer);
    }

    /// The bytes it owns: the radios a pre-activation unchecked.
    pub fn footprint(&self) -> usize {
        match &self.activation {
            Activation::Radio { unchecked, .. } => {
                unchecked.capacity().saturating_mul(size_of::<NodeId>())
            }
            _ => 0,
        }
    }
}
