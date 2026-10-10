/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page shown and a page left, as tasks (ADR 0039 § 2, queue item 373).
//!
//! **`pageshow`** is the last step of a page's load: a `PageTransitionEvent`
//! fired at the window, whose `persisted` is `false`. It is an ordinary
//! dispatch from the browser whose target is the window rather than a node
//! ([`EventLoop::queue_page_show`]), so it runs as every other dispatch does.
//!
//! **Leaving** is HTML's *unload a document*, as one task
//! ([`EventLoop::queue_leave`]):
//! 1. `pagehide` at the window, made as `pageshow` is;
//! 2. the visibility state updated to `hidden` by *update the visibility
//!    state*'s steps — so `visibilitychange` is fired at the document if it
//!    was `visible`, and not if it was already `hidden` (`shown.rs`);
//! 3. `unload` at the window.
//!
//! Each listener is followed by a microtask checkpoint, as in every task.
//! What becomes of the page afterwards — every task still waiting dropped,
//! the job queue emptied, the whole loop let go of — is the renderer's
//! ([`crate::transition`]): a task cannot discard the loop it runs in.
//!
//! Every event and both targets are made when the task is queued, as every
//! dispatch's are, and held by one rooted list until it has run.
//!
//! # Not here
//!
//! HTML fires all three with its *legacy target override flag*, so a
//! listener reads the document as their `target` though they are dispatched
//! at the window. Here their `target` is the window, which is what they are
//! dispatched at; the override is queue item 374's.

use alo_bindings::event::{self, Firing};
use alo_bindings::{DocumentCell, Visibility, node_of, visibility};
use alo_js::heap::Ref;
use alo_js::object::{Held, Objects};
use alo_js::{Escape, Fault, Root, Value};

use super::task::{self, Seq, Unmade, Work};
use super::{EventLoop, Stopped, Turn, Unqueued};

/// How many objects a [`Work::Leave`] list holds: the window, the
/// `pagehide`, the document's wrapper, the `visibilitychange` and the
/// `unload`.
const LEAVING: usize = 5;

impl EventLoop {
    /// Queue `pageshow` at the window of the document `cell` holds (ADR 0039
    /// § 2): one dispatch from the browser, as [`EventLoop::queue_dispatch`]
    /// queues one at a node.
    ///
    /// `cell` must be rooted by the caller.
    ///
    /// # Errors
    ///
    /// As [`EventLoop::queue_dispatch`]; [`Unqueued::NotADocument`] for a
    /// document no window was associated with.
    pub fn queue_page_show(&mut self, cell: Ref) -> Result<Seq, Unqueued> {
        if let Some(stopped) = &self.stopped {
            return Err(Unqueued::Stopped(stopped.clone()));
        }
        let made = self.listed_at_window(cell, |objects, list, cell, window| {
            task::push(objects, list, window)?;
            let shown = event::create(objects, cell, &Firing::PAGE_SHOW)?;
            task::push(objects, list, shown)?;
            Ok(())
        });
        self.queued(made, |list| Work::Dispatch { list })
    }

    /// Queue the leaving steps of the page whose document `cell` holds (ADR
    /// 0039 § 2): one task, `pagehide`, then `hidden`, then `unload`.
    ///
    /// `cell` must be rooted by the caller.
    ///
    /// # Errors
    ///
    /// As [`EventLoop::queue_page_show`].
    pub fn queue_leave(&mut self, cell: Ref) -> Result<Seq, Unqueued> {
        if let Some(stopped) = &self.stopped {
            return Err(Unqueued::Stopped(stopped.clone()));
        }
        let root = alo_bindings::document(self.objects(), cell)
            .map(alo_dom::Document::root)
            .ok_or(Unqueued::NotADocument)?;
        let made = self.listed_at_window(cell, |objects, list, cell, window| {
            task::push(objects, list, window)?;
            let hide = event::create(objects, cell, &Firing::PAGE_HIDE)?;
            task::push(objects, list, hide)?;
            task::fill(objects, list, cell, root, &Firing::VISIBILITY_CHANGE)?;
            let unload = event::create(objects, cell, &Firing::UNLOAD)?;
            task::push(objects, list, unload)?;
            Ok(())
        });
        self.queued(made, |list| Work::Leave { list })
    }

    /// A rooted list, filled by `fill` with the window of the document `cell`
    /// holds and whatever else the task needs; let go of if `fill` fails.
    fn listed_at_window(
        &mut self,
        cell: Ref,
        fill: impl FnOnce(&mut Objects, &Root, Ref, Ref) -> Result<(), Unmade>,
    ) -> Result<Root, Unmade> {
        let objects = self.engine.objects();
        // The realm roots the window, and the cell holds it: nothing here
        // needs to.
        let window = objects
            .embedded::<DocumentCell>(cell)
            .ok_or(Unmade::NotADocument)?
            .window()
            .ok_or(Unmade::NotADocument)?;
        let list = objects.slots().map_err(|why| Escape::refused(why, 0))?;
        let list = objects.heap_mut().root(list);
        match fill(objects, &list, cell, window) {
            Ok(()) => Ok(list),
            Err(why) => {
                objects.heap_mut().release(list);
                Err(why)
            }
        }
    }

    /// Queue the work `work` makes of a list, or say why there is none — and
    /// stop the page if the heap could not hold it.
    fn queued(
        &mut self,
        made: Result<Root, Unmade>,
        work: impl FnOnce(Root) -> Work,
    ) -> Result<Seq, Unqueued> {
        match made {
            Ok(list) => Ok(self.tasks.push(work(list))),
            Err(Unmade::NoSuchNode(node)) => Err(Unqueued::NoSuchNode(node)),
            Err(Unmade::NotADocument) => Err(Unqueued::NotADocument),
            Err(Unmade::Escaped(escape)) => {
                let why = Stopped::Escaped(escape);
                self.stop(why.clone());
                Err(Unqueued::Stopped(why))
            }
        }
    }

    /// Run a [`Work::Leave`](task::Work::Leave) task: `pagehide`, then the
    /// state `hidden` and `visibilitychange` if it changed, then `unload`.
    pub(super) fn leave(&mut self, list: &Root, turn: &mut Turn) -> Result<(), Escape> {
        let [window, hide, document, change, unload] = leaving(self.engine.objects(), list)?;
        self.dispatch_event(hide, window, turn)?;
        // The task made the document's wrapper, so it names a document cell.
        let (cell, _) =
            node_of(self.engine.objects(), document).ok_or(Escape::fault(Fault::NotAnObject))?;
        let changed = visibility::update(self.engine.objects(), cell, Visibility::Hidden)
            .ok_or(Escape::fault(Fault::NotAnObject))?;
        if changed {
            self.dispatch_event(change, document, turn)?;
        }
        self.dispatch_event(unload, window, turn)?;
        Ok(())
    }
}

/// The five objects a [`Work::Leave`] list holds, in order.
///
/// # Errors
///
/// [`Escape::Broken`] if the list has gone or does not hold them, which is
/// the engine's bug or ours.
fn leaving(objects: &Objects, list: &Root) -> Result<[Ref; LEAVING], Escape> {
    let held = objects
        .heap()
        .holding(list)
        .ok_or(Escape::fault(Fault::Gone))?;
    let mut out = [held; LEAVING];
    for (at, slot) in out.iter_mut().enumerate() {
        *slot = match objects.slot(held, at) {
            Some(Held::Value(Value::Object(object))) => object,
            Some(Held::Value(_) | Held::Uninitialized) | None => {
                return Err(Escape::fault(Fault::Gone));
            }
        };
    }
    Ok(out)
}
