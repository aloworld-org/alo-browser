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
//! § 3): on a page whose document is in its heap, a task on its loop — and
//! [`Held::activate`] is how an agent's `Activate` is one (§§ 5–6), and
//! [`Held::put_text`] how its `PutText` is (§ 5). **A page that has never
//! run script is dispatched to by nobody** — it has no heap, so no wrapper
//! and no listener, and it is not given a heap to find that out. Stage 1's
//! pages behave exactly as they did.
//!
//! # Where the page has asked to go
//!
//! A page whose document is in its heap keeps its ongoing navigation in the
//! cell (ADR 0020 § 1), where a script's `click()` records it; the browser's
//! own click on such a page records it there too ([`Held::follow`]), so one
//! order holds across both, and the renderer takes it when the message's
//! work is done ([`Held::take_navigation`]). A page that has never run script
//! has no cell and nothing that could ask behind the renderer's back, so the
//! renderer asks for it directly.
//!
//! # What the page has asked to fetch
//!
//! A script's `fetch` is an ask kept in the cell as a navigation is (ADR
//! 0032 § 1), taken when the message's work is done
//! ([`Held::take_fetches`]); the answer to each is a task of its own, queued
//! by [`Held::deliver`]. A page that has never run script has asked for
//! nothing and waits for nothing.
//!
//! # A heap that will not take the document
//!
//! A document larger than the heap's ceiling cannot be adopted, since the
//! cell's footprint is the document's size. The heap hands it back
//! ([`alo_bindings::Unadopted`]), the page stays [`Held::Parsed`], and it is
//! still rendered: a page whose script cannot run is still a page.

use core::fmt;

use alo_bindings::fetching::{self, Asked};
use alo_bindings::navigating::{self, By, Ongoing};
use alo_bindings::{
    Firing, Identity, Responded, Unadopted, adopt, change_document, document, install, introduce,
    offer,
};
use alo_dom::{Document, NodeId};
use alo_js::Escape;
use alo_js::heap::{Ref, Root};
use alo_js::object::Refused;
use alo_url::Url;

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
        self.queue(|page_loop, cell| page_loop.queue_dispatch(cell, node, firing))
    }

    /// Queue an agent's `Activate` of `node` as a task on the page's loop —
    /// the activation steps around a trusted `click` (ADR 0018 §§ 5–6) —
    /// answering which task, or [`None`] on a page that has never run
    /// script, where nobody listens and `alo-agent`'s `apply` acts instead.
    ///
    /// Nothing runs here: the task runs when the loop reaches it.
    ///
    /// # Errors
    ///
    /// As [`Held::dispatch`].
    pub fn activate(&mut self, node: NodeId) -> Result<Option<Seq>, Unqueued> {
        self.queue(|page_loop, cell| page_loop.queue_activation(cell, node))
    }

    /// Queue an agent's `PutText` of `text` into the field `node` as a task
    /// on the page's loop — a trusted `beforeinput` the page may cancel, then
    /// the text, `input` and `change` (ADR 0018 § 5) — answering which task,
    /// or [`None`] on a page that has never run script, where nobody listens
    /// and `alo-agent`'s `apply` puts the text in instead.
    ///
    /// Nothing runs here: the task runs when the loop reaches it.
    ///
    /// # Errors
    ///
    /// As [`Held::dispatch`].
    pub fn put_text(&mut self, node: NodeId, text: &str) -> Result<Option<Seq>, Unqueued> {
        self.queue(|page_loop, cell| page_loop.queue_put_text(cell, node, text))
    }

    /// Follow the link `link` as the browser's click, keeping the ask in the
    /// document cell beside any the page's script made: whether a navigation
    /// started — [`None`] on a page that has never run script, whose ask the
    /// renderer makes itself, or if the root has stopped naming a document.
    pub fn follow(&mut self, link: NodeId) -> Option<bool> {
        match self {
            Held::Parsed(_) => None,
            Held::Scripted(scripted) => {
                let objects = scripted.script.engine().objects();
                let cell = objects.heap().holding(&scripted.cell)?;
                navigating::start(objects, cell, link, By::Browser)
            }
        }
    }

    /// Where the page has asked to go since this was last called, leaving
    /// nothing — always nothing on a page that has never run script.
    pub fn take_navigation(&mut self) -> Ongoing {
        match self {
            Held::Parsed(_) => Ongoing::default(),
            Held::Scripted(scripted) => {
                let objects = scripted.script.engine().objects();
                objects
                    .heap()
                    .holding(&scripted.cell)
                    .and_then(|cell| navigating::take(objects, cell))
                    .unwrap_or_default()
            }
        }
    }

    /// Every fetch the page has asked for since this was last called, oldest
    /// first, leaving none — always none on a page that has never run
    /// script (ADR 0032 § 1).
    pub fn take_fetches(&mut self) -> Vec<Asked> {
        match self {
            Held::Parsed(_) => Vec::new(),
            Held::Scripted(scripted) => {
                let objects = scripted.script.engine().objects();
                objects
                    .heap()
                    .holding(&scripted.cell)
                    .and_then(|cell| fetching::take(objects, cell))
                    .unwrap_or_default()
            }
        }
    }

    /// Queue the delivery of the answer to the page's fetch `number` —
    /// `responded`, or a network error for [`None`] — as a task on the
    /// page's loop: which task, or [`None`] when nothing on the page waits
    /// for it, which a page that has never run script never does.
    ///
    /// Nothing runs here: the task runs when the loop reaches it.
    ///
    /// # Errors
    ///
    /// [`Unqueued`]: the page has stopped, or its heap could not hold the
    /// response or the task; or the renderer's root on the document has
    /// stopped naming it, which is the engine's bug.
    pub fn deliver(
        &mut self,
        number: u64,
        responded: Option<Responded>,
    ) -> Result<Option<Seq>, Unqueued> {
        self.queue(|page_loop, cell| page_loop.queue_delivery(cell, number, responded))
            .map(Option::flatten)
    }

    /// Queue a task with `queue`, handed the page's loop and its document
    /// cell — [`None`] on a page that has never run script.
    fn queue<T>(
        &mut self,
        queue: impl FnOnce(&mut EventLoop, Ref) -> Result<T, Unqueued>,
    ) -> Result<Option<T>, Unqueued> {
        match self {
            Held::Parsed(_) => Ok(None),
            Held::Scripted(scripted) => {
                let cell = scripted
                    .script
                    .objects()
                    .heap()
                    .holding(&scripted.cell)
                    .ok_or(Unqueued::NotADocument)?;
                queue(&mut scripted.script, cell).map(Some)
            }
        }
    }

    /// The page's event loop, making it — and moving the document into its
    /// heap, at `url`, with `document` on its global object, `navigator`
    /// saying what `identity` says (ADR 0030 § 4) and `fetch` asking the
    /// browser process (ADR 0032) — if no script has run yet.
    ///
    /// **Called when the page's first script is about to run**, and not
    /// before: a page none of whose scripts may run never builds a heap.
    ///
    /// # Errors
    ///
    /// [`NoScript`] if the page cannot run script. Its document stays where
    /// it was readable from — handed back when the heap refused it, in the
    /// heap when it was adopted and its interfaces could not be made.
    pub fn scripted(
        &mut self,
        url: &Url,
        identity: Identity<'_>,
    ) -> Result<&mut EventLoop, NoScript> {
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
            // Before any of the page's script can read it. Not a document
            // cell only if `adopt` made something else, which it does not.
            navigating::locate(engine.objects(), made, url.clone());
            let cell = engine.objects().heap_mut().root(made);
            let installed = install(engine, made)
                .and_then(|_| introduce(engine, made, identity).map(drop))
                .and_then(|()| offer(engine, made));
            *self = Held::Scripted(Box::new(Scripted { script, cell }));
            installed.map_err(NoScript::Engine)?;
        }
        // Not [`None`]: a parsed page became a scripted one above, or
        // returned. Answered rather than assumed.
        self.event_loop()
            .ok_or(NoScript::Engine(Escape::fault(alo_js::Fault::Gone)))
    }
}
