/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's document, in its heap.
//!
//! ADR 0017 § 2: when a page's first script is about to run, its document
//! moves into the page's heap as **one embedder cell** — this — and has that
//! one owner for the rest of the page's life. Style, layout, paint and the
//! agent borrow it from here for as long as a read lasts, and a change is a
//! write to this cell, so it is an ordinary Rust borrow out of the heap: no
//! second owner, no shared pointer, no runtime flag to fail.
//!
//! The cell also keeps **the table** from node to wrapper, which is what makes
//! asking for a node twice give one object (ADR 0014 § 6), and it is the cell
//! whose tracing keeps each wrapper as long as its tree — that rule is
//! [`crate::liveness`], kept apart because it is the one part of this that a
//! collection runs.
//!
//! Its footprint is the document's size (§ 2): a node lives in `alo-dom`'s
//! arena rather than in a heap slot, and a script that made nodes the heap did
//! not count could fill a renderer below the heap's ceiling.
//!
//! And it holds **the prototype of every interface** a node can be seen as
//! ([`crate::interface`]): a native reaches nothing but its `this`, so the
//! prototype `createElement`'s wrapper inherits from must be reachable from
//! the wrapper it was called on, and this is the cell every wrapper holds.
//! Beside them it holds each interface's **unforgeables** (ADR 0019 § 3),
//! and it is the realm's `[[HostDefined]]`, which is how a constructor —
//! whose `this` is a fresh instance holding nothing — finds it (§ 2).
//!
//! And it holds where the document **is** — its URL, stated by the browser
//! process, and the page's one `Location` that reads it ([`crate::location`])
//! — and where the page has **asked to go** since the renderer last
//! answered ([`crate::navigating`], ADR 0020 § 1): HTML's ongoing navigation
//! is a fact about the page's one navigable, which here is this document's.
//! Beside it, what the page has asked to **fetch** and the promises waiting
//! for the answers ([`crate::fetching`], ADR 0032 § 1). And the
//! **policies** the page holds, stated by the renderer as it states the URL,
//! so that `element.style` reads a refused `style` attribute as the draw
//! does (ADR 0034 § 3, [`crate::style_policy`]).
//!
//! And it holds an edge to the page's **`Window`**, the realm's global
//! object, set by [`crate::install`] as the window is given an edge back
//! (ADR 0037 § 2): a dispatch that starts at a node of this document ends at
//! the window ([`crate::dispatch`]), and this is where it finds it. A
//! document no page's window was associated with — a second one, made by
//! [`crate::furnish`] alone — has none, and its paths stop at its root.
//!
//! It is an object only because everything in the heap that is not the
//! engine's own is one. No script is ever handed it — the document *node* a
//! page sees is a wrapper like any other — so as an object it is the plainest
//! there is: no prototype, no properties, and it refuses any.

use alo_dom::{Document, NodeId};
use alo_js::heap::{Barrier, Field, Ref};
use alo_js::object::{Exotic, Internal, Key, Property};
use alo_net::Policies;
use alo_url::Url;

use crate::fetching::Fetches;
use crate::interface::Interfaces;
use crate::navigating::Ongoing;

/// One slot of the table: the node, and its wrapper.
pub(crate) type Entry = Option<(NodeId, Ref)>;

/// A page's document, as one cell of its heap.
#[derive(Debug)]
pub struct DocumentCell {
    pub(crate) document: Document,
    /// The wrapper of each node that has one, at the node's id.
    pub(crate) table: Vec<Entry>,
    /// A node whose wrapper is being made: its tree is kept by the collection
    /// that making the wrapper may cause, since nothing else holds it yet.
    pub(crate) pending: Option<NodeId>,
    pub(crate) released: Released,
    /// The prototype of each interface, and the unforgeables of those with
    /// any, once [`crate::install`] has made them.
    pub(crate) interfaces: Interfaces,
    /// How many dispatches in progress have each node on their path, at the
    /// node's id: a node on one is kept, tree and wrapper, until they end
    /// ([`crate::dispatch`]). Empty until the first dispatch.
    pub(crate) on_path: Vec<u32>,
    /// The document's URL: `about:blank` until the browser process says
    /// otherwise ([`crate::navigating::locate`]).
    pub(crate) url: Url,
    /// The page's one `Location`, once [`crate::install`] has made it: what
    /// `location` answers on the global object and on the document.
    pub(crate) location: Field,
    /// The page's `Window`, once [`crate::install`] has associated it.
    pub(crate) window: Field,
    /// The policies the page holds: its headers' and every `<meta>`'s the
    /// parser has made so far, stated by the renderer
    /// ([`crate::style_policy::state`]). None until it does.
    pub(crate) policies: Policies,
    /// Where the page has asked to go since the renderer last took it.
    pub(crate) ongoing: Ongoing,
    /// What the page has asked to fetch, and the promises waiting for the
    /// answers ([`crate::fetching`]).
    pub(crate) fetches: Fetches,
}

/// What collections have let go of, counted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Released {
    /// Detached trees released, each because no script held any node in it.
    pub trees: u64,
    /// The nodes those trees held.
    pub nodes: u64,
}

impl DocumentCell {
    /// The cell `document` becomes.
    pub fn new(document: Document) -> Self {
        Self {
            document,
            table: Vec::new(),
            pending: None,
            released: Released::default(),
            interfaces: Interfaces::default(),
            on_path: Vec::new(),
            url: Url::about_blank(),
            location: Field::default(),
            window: Field::default(),
            policies: Policies::none(),
            ongoing: Ongoing::default(),
            fetches: Fetches::default(),
        }
    }

    /// The document, to read.
    pub const fn document(&self) -> &Document {
        &self.document
    }

    /// The document, to change through `alo-dom`'s own operations (ADR 0017
    /// § 5), whose validity rules hold for every caller alike.
    pub const fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }

    /// The document's URL.
    pub const fn url(&self) -> &Url {
        &self.url
    }

    /// The page's one `Location`, once it has been made.
    pub const fn location(&self) -> Option<Ref> {
        self.location.get()
    }

    /// The page's `Window`, once it has been associated: the realm's global
    /// object, whose document this is.
    pub const fn window(&self) -> Option<Ref> {
        self.window.get()
    }

    /// The policies the page holds, as the renderer last stated them.
    pub const fn policies(&self) -> &Policies {
        &self.policies
    }

    /// Where the page has asked to go since the renderer last took it.
    pub const fn ongoing(&self) -> &Ongoing {
        &self.ongoing
    }

    /// What the page has asked to fetch, and what it waits for.
    pub const fn fetches(&self) -> &Fetches {
        &self.fetches
    }

    /// The wrapper `node` has, if it has one.
    pub fn wrapper(&self, node: NodeId) -> Option<Ref> {
        self.table
            .get(node.as_usize())
            .copied()
            .flatten()
            .map(|(_, wrapper)| wrapper)
    }

    /// How many nodes have a wrapper.
    pub fn wrapped(&self) -> usize {
        self.table.iter().flatten().count()
    }

    /// The prototype of each interface a node can be seen as, and each
    /// interface's unforgeables.
    pub const fn interfaces(&self) -> &Interfaces {
        &self.interfaces
    }

    /// What collections have let go of, so far.
    pub const fn released(&self) -> Released {
        self.released
    }

    /// Whether `node` is on the path of a dispatch in progress.
    pub fn on_path(&self, node: NodeId) -> bool {
        self.on_path
            .get(node.as_usize())
            .is_some_and(|count| *count > 0)
    }

    /// A dispatch has begun along `path`: keep each node on it.
    pub(crate) fn enter_path(&mut self, path: &[NodeId]) {
        for node in path {
            let at = node.as_usize();
            if self.on_path.len() <= at {
                self.on_path
                    .resize(self.document.node_count().max(at.saturating_add(1)), 0);
            }
            if let Some(count) = self.on_path.get_mut(at) {
                *count = count.saturating_add(1);
            }
        }
    }

    /// The dispatch along `path` has ended.
    pub(crate) fn leave_path(&mut self, path: &[NodeId]) {
        for node in path {
            if let Some(count) = self.on_path.get_mut(node.as_usize()) {
                *count = count.saturating_sub(1);
            }
        }
    }

    /// Record that `wrapper` is `node`'s, through the barrier every store of
    /// a reference passes (ADR 0014 § 5).
    pub(crate) fn remember(&mut self, barrier: &mut Barrier, node: NodeId, wrapper: Ref) {
        let at = node.as_usize();
        if self.table.len() <= at {
            self.table
                .resize(self.document.node_count().max(at.saturating_add(1)), None);
        }
        if let Some(slot) = self.table.get_mut(at) {
            barrier.stored(slot.map(|(_, was)| was), Some(wrapper));
            *slot = Some((node, wrapper));
        }
    }
}

impl Internal for DocumentCell {
    fn own_property(&self, _key: Key) -> Option<&Property> {
        None
    }

    fn define_own(&mut self, _barrier: &mut Barrier, _key: Key, _property: Property) -> bool {
        false
    }

    fn delete_own(&mut self, _key: Key) -> bool {
        // There is nothing to delete, and deleting what is not there succeeds.
        true
    }

    fn own_keys(&self) -> Vec<Key> {
        Vec::new()
    }

    fn prototype(&self) -> Option<Ref> {
        None
    }

    fn set_prototype(&mut self, _barrier: &mut Barrier, to: Option<Ref>) -> bool {
        to.is_none()
    }

    fn is_extensible(&self) -> bool {
        false
    }

    fn prevent_extensions(&mut self) -> bool {
        true
    }
}

impl Exotic for DocumentCell {
    fn describe(&self) -> &'static str {
        "a page's document"
    }
}
