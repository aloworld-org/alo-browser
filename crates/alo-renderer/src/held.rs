/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a page's document is: the renderer's, or its script's.
//!
//! ADR 0017 § 2. **Until a page runs script, its document is the
//! renderer's**, owned here exactly as a stage 1 page's always was, and a
//! page that runs no script never builds a heap. **When its first script is
//! about to run, the document moves into the page's heap** as one cell, the
//! renderer holds that cell by one [`Root`], and it never moves back: the
//! page keeps it there until the page goes, and the heap goes with it.
//!
//! Either way, everything that reads the document — style, layout, paint,
//! the agent's tree — **borrows** it through [`Held::document`] for as long
//! as the read lasts, and `apply` changes it through [`Held::change`]. So
//! there is one owner at every moment, and a borrow out of the heap is an
//! ordinary Rust borrow that ends before anything could run script.
//!
//! # The browser's dispatch
//!
//! [`Held::dispatch`] is how the browser fires an event at a node (ADR 0018
//! § 3): on a page whose document is in its heap, a task on its loop. **A page
//! that has never run script is dispatched to by nobody** — it has no heap,
//! so no wrapper and no listener, and it is not given a heap to find that out.
//! Stage 1's pages behave exactly as they did.
//!
//! # A heap that will not take the document
//!
//! A document larger than the heap's ceiling cannot be adopted, since the
//! cell's footprint is the document's size. The heap hands it back
//! ([`alo_bindings::Unadopted`]), the page stays [`Held::Parsed`], and it is
//! still rendered: a page whose script cannot run is still a page.

use core::fmt;

use alo_bindings::{Firing, Unadopted, adopt, change_document, document, install};
use alo_dom::{Document, NodeId};
use alo_js::Escape;
use alo_js::heap::Root;
use alo_js::object::Refused;

use crate::event_loop::{EventLoop, Seq, Unqueued};

/// A page's document, wherever it is.
#[derive(Debug)]
pub enum Held {
    /// No script of the page's has run, and the renderer owns its document.
    Parsed(Document),
    /// The page's script has run: the document is in its heap. Boxed, since
    /// an engine is many times the size of a document's handle.
    Scripted(Box<Scripted>),
}

/// A page whose document is in its heap.
#[derive(Debug)]
pub struct Scripted {
    /// The page's event loop, and the engine whose heap holds the document.
    script: EventLoop,
    /// The renderer's one root on the document cell.
    cell: Root,
}

/// Why a page's script could not be given its document.
#[derive(Debug)]
pub enum NoScript {
    /// The engine could not be made, or could not be given the document's
    /// interfaces: its heap is full, or its bug.
    Engine(Escape),
    /// The heap would not take the document. It was handed back, and the
    /// page is rendered from it as though it ran no script.
    Refused(Refused),
}

impl fmt::Display for NoScript {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NoScript::Engine(escape) => write!(out, "this page has no engine: {escape}"),
            NoScript::Refused(refused) => write!(
                out,
                "this page's document could not be put in its heap: {refused}"
            ),
        }
    }
}

impl Held {
    /// The document, to read, for as long as the read lasts.
    ///
    /// [`None`] only if the cell the renderer roots has stopped being a
    /// document, which is the engine's bug.
    pub fn document(&self) -> Option<&Document> {
        match self {
            Held::Parsed(parsed) => Some(parsed),
            Held::Scripted(scripted) => {
                let objects = scripted.script.objects();
                let cell = objects.heap().holding(&scripted.cell)?;
                document(objects, cell)
            }
        }
    }

    /// Change the document with `change` — `alo-dom`'s own operations, under
    /// the rules they hold for every caller (ADR 0017 § 5).
    ///
    /// Not a safepoint: nothing in `change` can reach the heap.
    pub fn change<R>(&mut self, change: impl FnOnce(&mut Document) -> R) -> Option<R> {
        match self {
            Held::Parsed(parsed) => Some(change(parsed)),
            Held::Scripted(scripted) => {
                let objects = scripted.script.engine().objects();
                let cell = objects.heap().holding(&scripted.cell)?;
                change_document(objects, cell, change)
            }
        }
    }

    /// The page's event loop, if its script has run.
    pub fn event_loop(&mut self) -> Option<&mut EventLoop> {
        match self {
            Held::Parsed(_) => None,
            Held::Scripted(scripted) => Some(&mut scripted.script),
        }
    }

    /// Queue the browser's dispatch of the event `firing` describes to
    /// `node`, as a task on the page's loop, answering which task — or
    /// [`None`] on a page that has never run script, which nobody is
    /// listening to and which is not given a heap to find that out.
    ///
    /// Nothing runs here: the task runs when the loop reaches it.
    ///
    /// # Errors
    ///
    /// [`Unqueued`]: the page has stopped, or its heap could not hold the
    /// task; the document has no such node; or the renderer's root on the
    /// document has stopped naming it, which is the engine's bug.
    pub fn dispatch(&mut self, node: NodeId, firing: &Firing<'_>) -> Result<Option<Seq>, Unqueued> {
        match self {
            Held::Parsed(_) => Ok(None),
            Held::Scripted(scripted) => {
                let cell = scripted
                    .script
                    .objects()
                    .heap()
                    .holding(&scripted.cell)
                    .ok_or(Unqueued::NotADocument)?;
                scripted.script.queue_dispatch(cell, node, firing).map(Some)
            }
        }
    }

    /// The page's event loop, making it — and moving the document into its
    /// heap, with `document` on its global object — if no script has run yet.
    ///
    /// **Called when the page's first script is about to run**, and not
    /// before: a page none of whose scripts may run never builds a heap.
    ///
    /// # Errors
    ///
    /// [`NoScript`] if the page cannot run script. Its document stays where
    /// it was readable from — handed back when the heap refused it, in the
    /// heap when it was adopted and its interfaces could not be made.
    pub fn scripted(&mut self) -> Result<&mut EventLoop, NoScript> {
        if let Held::Parsed(parsed) = self {
            let mut script = EventLoop::new().map_err(NoScript::Engine)?;
            let taken = core::mem::take(parsed);
            let engine = script.engine();
            let made = match adopt(engine.objects(), taken) {
                Ok(made) => made,
                Err(Unadopted { refused, document }) => {
                    // Back where it was. [`None`] is the heap losing it,
                    // which it does not; the page is then empty, and said
                    // to be refused either way.
                    *parsed = document.unwrap_or_default();
                    return Err(NoScript::Refused(refused));
                }
            };
            let cell = engine.objects().heap_mut().root(made);
            let installed = install(engine, made);
            *self = Held::Scripted(Box::new(Scripted { script, cell }));
            installed.map_err(NoScript::Engine)?;
        }
        // Not [`None`]: a parsed page became a scripted one above, or
        // returned. Answered rather than assumed.
        self.event_loop()
            .ok_or(NoScript::Engine(Escape::fault(alo_js::Fault::Gone)))
    }
}
