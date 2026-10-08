/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A promise: an ordinary object with the state the specification keeps in
//! its internal slots (ADR 0032 § 5, queue item 333).
//!
//! `[[PromiseState]]`, `[[PromiseResult]]`, the reactions waiting for it to
//! settle and `[[PromiseIsHandled]]`. Everything else about it is the ordinary
//! answer — a page may hang properties off a promise like off anything.
//!
//! # One list of reactions rather than two
//!
//! The specification keeps `[[PromiseFulfillReactions]]` and
//! `[[PromiseRejectReactions]]`, and every `then` appends to both, at the same
//! moment, in the same order. So they are one list of [`Reaction`]s here, each
//! with both of its handlers, and settling reads the half it needs. Two lists
//! that could never disagree would be two places to keep from disagreeing.
//!
//! # Settling lets go
//!
//! A settled promise never reacts again — a `then` on one queues its job at
//! once — so [`Promise::settle`] hands the reactions back and keeps none. Their
//! handlers stop being reachable from the promise at the moment they become
//! reachable from the job queue, and a promise chain that has finished holds
//! nothing it no longer needs.
//!
//! # Its size is counted
//!
//! A `then` grows the list, and growing it is a
//! [`Heap::write`](crate::heap::Heap::write), which measures what the cell owns
//! before and after. A page that calls `then` on a promise that never settles,
//! for ever, meets the heap's ceiling rather than this process's memory.

use crate::heap::{Barrier, Ref, Tracer};

use super::ordinary::Ordinary;
use super::value::{Stored, Value};

/// `[[PromiseState]]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Not yet fulfilled or rejected.
    Pending,
    /// Fulfilled with its result.
    Fulfilled,
    /// Rejected with its result as the reason.
    Rejected,
}

/// One `then` waiting for a promise to settle: `PromiseReaction` records for
/// both outcomes, sharing their capability.
#[derive(Debug)]
struct Reaction {
    /// The promise `then` answered, which the handler's outcome resolves.
    derived: Stored,
    /// What runs on fulfilment, or `undefined` to pass the value through.
    on_fulfilled: Stored,
    /// What runs on rejection, or `undefined` to pass the reason through.
    on_rejected: Stored,
}

/// What a settled promise hands back: for each reaction in the order the
/// `then`s were called, its derived promise and the handler for the outcome.
pub type Triggered = Vec<(Value, Value)>;

/// A promise.
#[derive(Debug)]
pub struct Promise {
    ordinary: Ordinary,
    state: State,
    /// The value or the reason, once it has settled.
    result: Stored,
    reactions: Vec<Reaction>,
    /// `[[PromiseIsHandled]]`: whether anything has ever reacted to it.
    handled: bool,
}

impl Promise {
    /// A pending promise with this prototype and nothing waiting on it.
    pub fn new(prototype: Option<Ref>) -> Self {
        Self {
            ordinary: Ordinary::with_prototype(prototype),
            state: State::Pending,
            result: Stored::default(),
            reactions: Vec::new(),
            handled: false,
        }
    }

    /// `[[PromiseState]]`.
    pub const fn state(&self) -> State {
        self.state
    }

    /// `[[PromiseResult]]`: `undefined` while it is pending.
    pub const fn result(&self) -> Value {
        self.result.get()
    }

    /// `[[PromiseIsHandled]]`.
    pub const fn is_handled(&self) -> bool {
        self.handled
    }

    /// Say that something reacts to it: `HostPromiseRejectionTracker`'s
    /// `"handle"`, which a rejection nobody had handled is told by.
    pub const fn handle(&mut self) {
        self.handled = true;
    }

    /// Wait for it to settle: `PerformPromiseThen` on a pending promise.
    ///
    /// Each is a reference gained by a cell that already exists, so each is
    /// told to the barrier, for the nursery ADR 0014 § 5 keeps the hook for.
    pub fn react(
        &mut self,
        barrier: &mut Barrier,
        derived: Value,
        on_fulfilled: Value,
        on_rejected: Value,
    ) {
        for value in [derived, on_fulfilled, on_rejected] {
            barrier.stored(None, value.reference());
        }
        self.reactions.push(Reaction {
            derived: Stored::holding(derived),
            on_fulfilled: Stored::holding(on_fulfilled),
            on_rejected: Stored::holding(on_rejected),
        });
    }

    /// Settle: `FulfillPromise` or `RejectPromise`, without the jobs — which
    /// are the engine's to queue — answering what was waiting.
    ///
    /// [`None`] for a promise that has already settled, or for `Pending` as
    /// the outcome: either is the caller's bug, since a promise's resolving
    /// functions settle it at most once.
    pub fn settle(
        &mut self,
        barrier: &mut Barrier,
        state: State,
        value: Value,
    ) -> Option<Triggered> {
        if self.state != State::Pending || state == State::Pending {
            return None;
        }
        self.state = state;
        self.result.set(barrier, value);
        let triggered = self
            .reactions
            .drain(..)
            .map(|reaction| {
                let handler = if state == State::Fulfilled {
                    reaction.on_fulfilled.get()
                } else {
                    reaction.on_rejected.get()
                };
                (reaction.derived.get(), handler)
            })
            .collect();
        self.reactions.shrink_to_fit();
        Some(triggered)
    }

    /// The ordinary object it also is.
    pub const fn ordinary(&self) -> &Ordinary {
        &self.ordinary
    }

    /// The same, to be written through.
    pub const fn ordinary_mut(&mut self) -> &mut Ordinary {
        &mut self.ordinary
    }

    /// Report every edge: the object's, the result, and each reaction's.
    pub fn trace(&self, tracer: &mut Tracer) {
        self.ordinary.trace(tracer);
        self.result.trace(tracer);
        for reaction in &self.reactions {
            reaction.derived.trace(tracer);
            reaction.on_fulfilled.trace(tracer);
            reaction.on_rejected.trace(tracer);
        }
    }

    /// What it owns beyond its slot: its table, and its reactions.
    pub fn footprint(&self) -> usize {
        self.ordinary.footprint().saturating_add(
            self.reactions
                .capacity()
                .saturating_mul(size_of::<Reaction>()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{State, Triggered};
    use crate::object::{Objects, Value};

    #[test]
    fn it_settles_once_and_hands_back_each_reaction_with_the_handler_for_its_outcome() {
        let mut objects = Objects::new();
        let Ok(held) = objects.promise(None) else {
            panic!("an empty heap holds a promise");
        };
        let reacted = objects.with_promise(held, |promise, barrier| {
            promise.react(
                barrier,
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(3.0),
            );
            promise.react(
                barrier,
                Value::Number(4.0),
                Value::Undefined,
                Value::Number(6.0),
            );
            promise.handle();
        });
        assert_eq!(reacted, Some(()));
        assert_eq!(
            objects
                .as_promise(held)
                .map(|promise| (promise.state(), promise.is_handled())),
            Some((State::Pending, true))
        );
        let triggered: Option<Option<Triggered>> = objects
            .with_promise(held, |promise, barrier| {
                promise.settle(barrier, State::Rejected, Value::Bool(true))
            });
        assert_eq!(
            triggered,
            Some(Some(vec![
                (Value::Number(1.0), Value::Number(3.0)),
                (Value::Number(4.0), Value::Number(6.0)),
            ]))
        );
        assert_eq!(
            objects.as_promise(held).map(super::Promise::result),
            Some(Value::Bool(true))
        );
        let again = objects.with_promise(held, |promise, barrier| {
            promise.settle(barrier, State::Fulfilled, Value::Null)
        });
        assert_eq!(again, Some(None), "a settled promise does not settle again");
        let pending = objects.with_promise(held, |promise, barrier| {
            promise.settle(barrier, State::Pending, Value::Null)
        });
        assert_eq!(pending, Some(None), "and nothing settles to pending");
    }

    #[test]
    fn a_promise_is_only_a_promise() {
        let mut objects = Objects::new();
        let Ok(object) = objects.object(None) else {
            panic!("an empty heap holds an object");
        };
        assert!(objects.as_promise(object).is_none());
        assert_eq!(objects.with_promise(object, |_, _| ()), None);
    }
}
