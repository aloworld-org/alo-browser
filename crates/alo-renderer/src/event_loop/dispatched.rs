/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A dispatch from the browser, as a task (ADR 0018 § 3, ADR 0016 §§ 3 and
//! 6, queue item 255).
//!
//! The dispatch algorithm is `alo-bindings`' and is written once
//! ([`alo_bindings::dispatch`]): a stepper whose state is in the event. A
//! script's `dispatchEvent` drives it from a native, calling every listener
//! while the script is still running. This drives the same steps for the
//! browser, **with nothing else running**: each listener is an ordinary call
//! into the engine, and so each is followed by a **microtask checkpoint**
//! (ADR 0016 § 3) — two listeners on a node the browser dispatched to see each
//! other's microtasks run between them, which the same two dispatched by a
//! script do not.
//!
//! # The order inside one listener's turn
//!
//! The standard's *inner invoke* calls the listener, and *calling* it is
//! *clean up after running script* — the checkpoint — before inner invoke
//! goes on to unset the passive flag and look at `stopImmediatePropagation`.
//! So the checkpoint runs **before** the stepper is told the listener has
//! returned: a microtask that stops the dispatch stops it before the next
//! listener, and one that calls `preventDefault` after a passive listener
//! still finds the passive flag set.
//!
//! # Throws and stops
//!
//! A listener that throws is reported, its jobs still run, and the dispatch
//! carries on — and so does one whose `handleEvent` getter throws, or whose
//! callback cannot be called. Anything else that ends a call — the embedder's
//! `Stop`, a full heap — ends the task, and the loop stops the page (ADR 0016
//! § 7): the dispatch is abandoned where it was, which is safe only because
//! nothing on the page runs again.

use alo_bindings::dispatch::{self, Invoke, Next};
use alo_js::heap::Ref;
use alo_js::{Escape, Fault, Root, Value};

use super::task;
use super::{EventLoop, Turn};

impl EventLoop {
    /// Run a [`Work::Dispatch`](task::Work::Dispatch) task: begin the
    /// dispatch, trusted, then call every listener it names, each followed by
    /// a checkpoint, until it is done.
    pub(super) fn dispatch(&mut self, list: &Root, turn: &mut Turn) -> Result<(), Escape> {
        let (event, target) = task::dispatched(&mut self.engine, list)?;
        // The task made both, and a script could not have reached the event
        // to dispatch it first: a refusal is our bug, and stops the page.
        dispatch::begin(self.engine.objects(), event, target, true)
            .map_err(|_refused| Escape::fault(Fault::NotAnObject))?;
        loop {
            self.awake()?;
            let (callback, this) = match dispatch::next(self.engine.objects(), event)? {
                Next::Done { .. } => return Ok(()),
                Next::Call { callback, this } => (callback, this),
            };
            let outcome = self.listener(callback, this, event);
            self.reported(turn);
            match outcome {
                Ok(()) => {}
                Err(Escape::Thrown(thrown)) => self.report(&thrown, turn),
                Err(escape) => return Err(escape),
            }
            self.checkpoint(turn)?;
            dispatch::returned(self.engine.objects(), event);
        }
    }

    /// Call one listener's callback with the event, as [`dispatch::invoke`]
    /// says: the function itself, or the object's `handleEvent` — its getter
    /// first, if it has one.
    ///
    /// Every value here is held while it is used: the event by the task's
    /// root, the callback by the event, the current target by the document
    /// cell, and what a getter answered by the engine, which keeps its last
    /// call's answer until the next call has run.
    fn listener(&mut self, callback: Ref, this: Ref, event: Ref) -> Result<(), Escape> {
        let argument = [Value::Object(event)];
        match dispatch::invoke(self.engine.objects(), callback, this)? {
            Invoke::Call { callee, this } => self.engine.call(callee, this, &argument),
            Invoke::Get { getter, this } => {
                let callee = self.engine.call(getter, Value::Object(this), &[])?;
                self.engine.call(callee, Value::Object(this), &argument)
            }
        }
        .map(drop)
    }
}
