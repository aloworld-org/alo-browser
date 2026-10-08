/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Settling a promise, and the rejections nobody handled (ADR 0032 § 5,
//! queue item 333).
//!
//! # Settling is the engine's, because what follows it is
//!
//! `FulfillPromise` and `RejectPromise` write the promise's state and then do
//! two things a builtin cannot: queue a job for every reaction that was
//! waiting (`TriggerPromiseReactions`, which is this engine's job queue) and,
//! for a rejection nothing handles, tell the host
//! (`HostPromiseRejectionTracker`). So a builtin asks for it
//! ([`Want::Settle`](crate::object::native::Want)), as it asks for a job, and
//! this does it. Between the reactions leaving the promise and arriving in the
//! queue nothing allocates, so they are never anywhere the collector cannot
//! see.
//!
//! Each job is a call of the realm's `%PromiseReactionJob%` with the derived
//! promise, the handler for the outcome, which outcome it was, and the value
//! — the specification's `NewPromiseReactionJob`, written as a builtin so
//! that it is an ordinary entry in the one queue a `queueMicrotask` waits in.
//!
//! # A rejection is reported at the end of the checkpoint, if it still needs
//! to be
//!
//! HTML keeps a list of rejected promises *about to be notified* and walks it
//! at the end of every microtask checkpoint, telling the page about each one
//! still unhandled. That is [`Rejections`]: a promise rejected while nothing
//! had reacted to it is appended, and [`Rejections::notify`] reports each one
//! whose `[[PromiseIsHandled]]` is still false — so a `catch` attached later in
//! the same task, or by a job in the same checkpoint, means it is never
//! reported. Each is reported once: the list is emptied as it is walked, and a
//! promise rejects only once.
//!
//! The report is the one an uncaught throw gets (queue item 241), with no
//! calls left behind, because none were: the rejection happened, and the
//! moment nobody reacted to it is the end of the checkpoint rather than a
//! place in the source. HTML's `unhandledrejection` and `rejectionhandled`
//! events are the embedder's, and are not fired.
//!
//! The list is a [`Slots`](crate::object::Slots) cell rooted for the engine's
//! life, as the job queue is, so a rejected promise waiting to be notified is
//! kept alive by it — and its reason with it — until the report has been made.

use crate::abrupt::{Escape, Internal, Thrown};
use crate::heap::{Ref, Root};
use crate::object::promise::State;
use crate::object::{Fault, Held, Objects, Value};

use super::Engine;
use super::unwound::Unwound;

impl Engine {
    /// Settle `promise` with `value`, queue a reaction job for each reaction
    /// waiting on it, and remember it if it was rejected with nothing
    /// handling it.
    ///
    /// **Not a safepoint**: writing the promise and growing two lists are
    /// writes, not allocations.
    ///
    /// # Errors
    ///
    /// [`Escape::Broken`] for something that is not a pending promise, which
    /// is the builtin's bug, or a queue or list this engine has lost.
    pub(super) fn settle(
        &mut self,
        promise: Value,
        fulfilled: bool,
        value: Value,
    ) -> Result<(), Escape> {
        let Value::Object(held) = promise else {
            return Err(Escape::Broken(Internal::BuiltinIsWrong));
        };
        let state = if fulfilled {
            State::Fulfilled
        } else {
            State::Rejected
        };
        let job = self.realm.intrinsics().reaction_job(&self.objects)?;
        let (triggered, handled) = self
            .objects
            .with_promise(held, |promise, barrier| {
                (promise.settle(barrier, state, value), promise.is_handled())
            })
            .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
        let triggered = triggered.ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
        for (derived, handler) in triggered {
            self.jobs.push(
                &mut self.objects,
                Value::Object(job),
                &[derived, handler, Value::Bool(fulfilled), value],
            )?;
        }
        if !fulfilled && !handled {
            self.rejections.remember(&mut self.objects, held)?;
        }
        Ok(())
    }
}

/// The rejected promises about to be notified, oldest first.
#[derive(Debug)]
pub(super) struct Rejections {
    list: Root,
}

impl Rejections {
    /// An empty list.
    ///
    /// # Errors
    ///
    /// [`Escape::Full`] if the heap cannot hold one empty list.
    pub(super) fn new(objects: &mut Objects) -> Result<Self, Escape> {
        let list = objects.slots().map_err(|why| Escape::refused(why, 0))?;
        Ok(Self {
            list: objects.heap_mut().root(list),
        })
    }

    /// Remember a promise rejected with nothing handling it.
    fn remember(&self, objects: &mut Objects, promise: Ref) -> Result<(), Escape> {
        let list = self.list(objects)?;
        objects
            .with_slots(list, |slots, _| slots.push(Value::Object(promise)))
            .ok_or_else(|| Escape::fault(Fault::Gone))
    }

    /// How many are waiting to be notified.
    pub(super) fn waiting(&self, objects: &Objects) -> usize {
        objects
            .heap()
            .holding(&self.list)
            .and_then(|list| objects.slot_count(list))
            .unwrap_or(0)
    }

    /// Report each one still unhandled, oldest first, as an uncaught throw of
    /// its reason, and empty the list; answer how many were reported.
    ///
    /// The report is handed the object model read-only, so it can describe
    /// the reason and cannot run anything — which is also why the list cannot
    /// change while it is walked.
    ///
    /// # Errors
    ///
    /// [`Escape::Broken`] if the list has gone or holds something that is not
    /// a promise, which is this engine's own bug.
    pub(super) fn notify(
        &self,
        objects: &mut Objects,
        report: &mut dyn FnMut(&Objects, &Thrown, &Unwound),
    ) -> Result<usize, Escape> {
        let list = self.list(objects)?;
        let count = objects
            .slot_count(list)
            .ok_or_else(|| Escape::fault(Fault::Gone))?;
        let nobody = Unwound::default();
        let mut reported = 0_usize;
        for at in 0..count {
            let Some(Held::Value(Value::Object(held))) = objects.slot(list, at) else {
                return Err(Escape::Broken(Internal::BuiltinIsWrong));
            };
            let promise = objects
                .as_promise(held)
                .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
            if promise.is_handled() {
                continue;
            }
            let thrown = Thrown::Value {
                value: promise.result(),
                at: 0,
            };
            report(objects, &thrown, &nobody);
            reported = reported.saturating_add(1);
        }
        self.forget(objects)?;
        Ok(reported)
    }

    /// Empty the list without reporting anything: a stopped page's
    /// rejections are dropped with its jobs (ADR 0016 § 7).
    ///
    /// # Errors
    ///
    /// [`Escape::Broken`] if the list has gone.
    pub(super) fn forget(&self, objects: &mut Objects) -> Result<(), Escape> {
        let list = self.list(objects)?;
        objects
            .with_slots(list, |slots, _| slots.truncate(0))
            .ok_or_else(|| Escape::fault(Fault::Gone))
    }

    /// The list, or this engine's bug.
    fn list(&self, objects: &Objects) -> Result<Ref, Escape> {
        objects
            .heap()
            .holding(&self.list)
            .ok_or_else(|| Escape::fault(Fault::Gone))
    }
}
