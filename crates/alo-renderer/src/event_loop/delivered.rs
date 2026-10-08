/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The answer to one of a page's fetches, as a task (ADR 0032 § 1, ADR 0016
//! § 2, queue item 335).
//!
//! The browser process sends each answer as a message of its own, and its
//! handling is a task of its own: `alo-bindings` makes the `Response` — or
//! nothing, for a network error — and names the call that settles the
//! promise waiting under the ask's number ([`delivering::answer`]), and this
//! queues that call like any other ([`EventLoop::queue_calls`]), with a
//! checkpoint after it that runs every reaction the promise had.
//!
//! The response is a Rust local between being made and being held by the
//! task, so it is rooted across the one allocation in between — queueing —
//! and let go of once the task holds it.

use alo_bindings::Responded;
use alo_bindings::delivering;
use alo_js::Value;
use alo_js::heap::Ref;

use super::task::Seq;
use super::{EventLoop, Stopped, Unqueued};

impl EventLoop {
    /// Queue the delivery of the answer to ask `number` — `responded`, or a
    /// network error for [`None`] — to the document `cell` holds: which
    /// task, or [`None`] when nothing on the page waits for it.
    ///
    /// `cell` must be rooted by the caller.
    ///
    /// # Errors
    ///
    /// [`Unqueued::Stopped`] when the page has stopped, or the heap could
    /// not hold the response or the task, which stops it here.
    pub fn queue_delivery(
        &mut self,
        cell: Ref,
        number: u64,
        responded: Option<Responded>,
    ) -> Result<Option<Seq>, Unqueued> {
        if let Some(stopped) = &self.stopped {
            return Err(Unqueued::Stopped(stopped.clone()));
        }
        let delivery = match delivering::answer(self.engine.objects(), cell, number, responded) {
            Ok(Some(delivery)) => delivery,
            Ok(None) => return Ok(None),
            Err(escape) => {
                let why = Stopped::Escaped(escape);
                self.stop(why.clone());
                return Err(Unqueued::Stopped(why));
            }
        };
        let held = match delivery.arguments {
            [_, Value::Object(response)] => Some(self.engine.objects().heap_mut().root(response)),
            _ => None,
        };
        let queued = self.queue_calls(&[delivery.callee], Value::Undefined, &delivery.arguments);
        if let Some(root) = held {
            self.engine.objects().heap_mut().release(root);
        }
        queued.map(Some).map_err(Unqueued::Stopped)
    }
}
