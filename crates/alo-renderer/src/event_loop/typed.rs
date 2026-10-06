/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An agent's `PutText`, as a task (ADR 0018 § 5, queue item 257).
//!
//! `PutText` is what replacing a field's text fires, and in this order, all
//! of it **one task** (ADR 0016 § 6), each event dispatched as
//! [`dispatched`](super::dispatched) dispatches any event the browser makes,
//! a checkpoint after every listener:
//!
//! 1. a trusted **`beforeinput`** at the field — an `InputEvent` whose
//!    `inputType` is `"insertReplacementText"` and whose `data` is the text
//!    ([`Firing::before_replacing`]), bubbling, composed and cancelable;
//! 2. if a listener **cancelled** it, nothing more: the field is as it was,
//!    and the task says so ([`Typed::Canceled`]);
//! 3. otherwise the field's text is replaced — `alo-dom`'s rule
//!    ([`alo_dom::field`]), the one `alo-agent`'s `apply` runs on a page that
//!    never ran script — and an **`input`** is fired, the same `InputEvent`
//!    except that it cannot be cancelled ([`Firing::replaced`]), and then a
//!    **`change`**, an `Event` ([`Firing::CHANGE`]).
//!
//! A listener on `input` therefore reads the field's new text, and a
//! listener on `beforeinput` the old one with the new in `data`.
//!
//! # Said as it happens
//!
//! The task writes what it came to into its [`Turn::typed`] **as each step
//! happens** rather than when it ends, because a page can stop partway — a
//! full heap, the embedder's stop — and the agent's answer must then say
//! whether the text went in. [`None`] means the page never answered the
//! `beforeinput`: it stopped first, and nobody refused.
//!
//! # Why `change` follows at once
//!
//! A person's typing fires `change` when the field is committed — on
//! leaving it, or on Enter. An agent's text is put in whole and committed
//! as it is put, so its `change` comes straight after its `input`, as ADR
//! 0018 § 5 says. When focus exists (item 258) this is where a decision
//! about when a field is committed would change, and nowhere else.
//!
//! # A field a listener took away
//!
//! A `beforeinput` listener may remove the field from its document without
//! cancelling. The text still goes into it, since the element still exists
//! and nobody refused, and `input` and `change` are still fired at it — a
//! dispatch to a node outside its document reaches the node's own
//! listeners and nobody else's.

use alo_bindings::{Firing, change_document, node_of};
use alo_dom::field;
use alo_js::{Escape, Fault, Root};

use super::task;
use super::{EventLoop, Turn};

/// What a `PutText` task came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typed {
    /// A listener cancelled the `beforeinput`, and the field is as it was.
    Canceled,
    /// The text went in. The `input` and `change` after it were fired if the
    /// turn did not stop.
    Put,
}

impl EventLoop {
    /// Run a [`Work::PutText`](task::Work::PutText) task: the `beforeinput`,
    /// and unless it was cancelled the text, the `input` and the `change` —
    /// each outcome written into `turn` as it happens.
    pub(super) fn put_text(
        &mut self,
        list: &Root,
        text: &str,
        turn: &mut Turn,
    ) -> Result<(), Escape> {
        let (before, target) = task::dispatched(&mut self.engine, list)?;
        let (cell, node) =
            node_of(self.engine.objects(), target).ok_or(Escape::fault(Fault::NotAnObject))?;

        if self.dispatch_event(before, target, turn)? {
            turn.typed = Some(Typed::Canceled);
            return Ok(());
        }

        let put = change_document(self.engine.objects(), cell, |document| {
            field::put_text(document, node, text)
        });
        // The task made the field's wrapper, so the field is an element.
        if put != Some(true) {
            return Err(Escape::fault(Fault::NotAnObject));
        }
        turn.typed = Some(Typed::Put);
        for firing in [Firing::replaced(text), Firing::CHANGE] {
            self.awake()?;
            let (target, event) = task::add(&mut self.engine, list, cell, node, &firing)
                .map_err(task::Unmade::into_escape)?;
            self.dispatch_event(event, target, turn)?;
        }
        Ok(())
    }
}
