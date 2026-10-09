/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A wrapper lives as long as its tree is reachable (ADR 0017 § 3).
//!
//! The DOM answers *when is a node alive?*: exactly when its **tree** is,
//! since `parentNode` and `firstChild` walk the whole tree from any node in
//! it. So the document cell tells the collector, at every collection:
//!
//! - every wrapper in **the document's own tree** is a strong edge — alive as
//!   long as the document, so `el.foo = 1` on a node in the page survives any
//!   number of collections;
//! - the wrappers of each **detached tree** are a **ring of ephemerons**, each
//!   the key to the next in the tree's order and the last to the first, so
//!   ADR 0014 § 7's fixpoint marks the whole ring the moment any one is
//!   marked. A ring of *n* wrappers is *n* pairs, not *n²*.
//!
//! A node on the **path of a dispatch in progress** is kept as the standard's
//! path keeps it (ADR 0018 § 2): its wrapper is a strong edge wherever its
//! tree is, and its tree is not released, until the dispatch ends — so a
//! listener that detaches an ancestor and drops it still has that ancestor's
//! bubble listeners called.
//!
//! And at the sweep, it lets go of what did not survive: a wrapper that died
//! leaves the table, and every detached tree **none** of whose nodes still has
//! a wrapper is released from `alo-dom` — its nodes dropped, their ids left as
//! tombstones that answer nothing and are never handed out again. A tree
//! nobody ever wrapped is unreachable the moment it is detached, and goes at
//! the next collection.
//!
//! # Each tree is walked once, and nothing is allocated
//!
//! Both halves walk **trees**, not wrappers: the document's tree once, then
//! each detached tree once, found by `alo-dom`'s cursor over detached roots.
//! Asking each wrapper for its tree's root instead would cost the tree's
//! depth per wrapper, and a page that builds a chain a million deep and holds
//! every link would make every collection quadratic. Walked, a collection
//! costs the arena's size, the same order as the marking it is part of — and
//! with no stack and no list, since ADR 0014 § 8 says a collection allocates
//! nothing it has not already got ([`crate::tree`]).
//!
//! # A walk that does not finish keeps everything
//!
//! A walk gives up when it has taken more steps than a well-formed arena
//! could need, which `alo-dom`'s validity rules say never happens. If one
//! does, the cell cannot say which tree a wrapper is in, so it keeps **every**
//! wrapper strongly and releases no tree it could not see all the way round:
//! a leak for one collection is a cost, and freeing a node a script still
//! holds is a wrong answer.

use alo_dom::{Document, NodeId};
use alo_js::heap::{Ref, Survivors, Trace, Tracer};

use crate::document_cell::{DocumentCell, Entry};
use crate::tree;

impl Trace for DocumentCell {
    fn trace(&self, tracer: &mut Tracer) {
        // The prototypes are the page's, alive as long as its document is.
        self.interfaces.trace(tracer);
        // So is its `Location`, and its `Window`.
        self.location.trace(tracer);
        self.window.trace(tracer);
        // A promise waiting for a fetch's answer lives until it comes.
        self.fetches.trace(tracer);
        let document = &self.document;
        let mut whole = walk(document, document.root(), |node| {
            if let Some(wrapper) = self.wrapper(node) {
                tracer.edge(wrapper);
            }
        });

        let mut after = None;
        while let Some(root) = document.next_detached_root(after) {
            after = Some(root);
            let mut first: Option<Ref> = None;
            let mut previous: Option<Ref> = None;
            whole &= walk(document, root, |node| {
                if let Some(wrapper) = self.wrapper(node) {
                    if self.on_path(node) {
                        tracer.edge(wrapper);
                    }
                    match previous {
                        Some(before) => tracer.ephemeron(before, wrapper),
                        None => first = Some(wrapper),
                    }
                    previous = Some(wrapper);
                }
            });
            if let (Some(last), Some(first)) = (previous, first)
                && last != first
            {
                tracer.ephemeron(last, first);
            }
        }

        if !whole {
            for (_, wrapper) in self.table.iter().flatten() {
                tracer.edge(*wrapper);
            }
        }
    }

    fn footprint(&self) -> usize {
        self.document
            .footprint()
            .saturating_add(self.table.len().saturating_mul(size_of::<Entry>()))
            .saturating_add(self.on_path.len().saturating_mul(size_of::<u32>()))
            .saturating_add(self.fetches.footprint())
    }

    fn clear_weak(&mut self, survivors: &Survivors) {
        for slot in &mut self.table {
            if slot.is_some_and(|(_, wrapper)| !survivors.alive(wrapper)) {
                *slot = None;
            }
        }

        let mut after = None;
        while let Some(root) = self.document.next_detached_root(after) {
            after = Some(root);
            let mut held = false;
            let whole = walk(&self.document, root, |node| {
                held |= self.pending == Some(node)
                    || self.wrapper(node).is_some()
                    || self.on_path(node);
            });
            if whole
                && !held
                && let Some(nodes) = self.document.release(root)
            {
                self.released.trees = self.released.trees.saturating_add(1);
                self.released.nodes = self
                    .released
                    .nodes
                    .saturating_add(u64::try_from(nodes).unwrap_or(u64::MAX));
            }
        }
    }
}

/// Visit every node of the tree rooted at `root`, `root` first, and say
/// whether the walk got all the way round.
fn walk(document: &Document, root: NodeId, mut visit: impl FnMut(NodeId)) -> bool {
    let limit = tree::budget(document);
    let mut spent = 0_usize;
    let mut at = root;
    loop {
        visit(at);
        match tree::next(document, at, root, &mut spent, limit) {
            Some(next) if next == root => return true,
            Some(next) => at = next,
            None => return false,
        }
    }
}
