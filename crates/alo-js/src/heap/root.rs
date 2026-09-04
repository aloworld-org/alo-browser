/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The closed list of places a live reference may be.
//!
//! ADR 0014 § 2. The collector is **precise**, which is not a performance
//! choice: conservative stack scanning needs `unsafe` to read the machine stack
//! at all (law 4) and retains by accident, so a stale word in a register keeps a
//! page's heap alive and nobody can reproduce it.
//!
//! Precision costs a discipline instead, and the discipline is this file. The
//! places a reference may be while the engine allocates are:
//!
//! - **[`Scope`]** — what native code holds. A builtin written in Rust has
//!   locals like anything else, and a scope is where it puts the references it
//!   needs to keep across an allocation. They nest and unwind like the calls
//!   they belong to, which is why they are a stack.
//! - **[`Root`]** — what outlives a call: a realm's globals, and the embedder's
//!   own roots (ADR 0014 § 6, the document). Held until somebody gives it back.
//! - **the keep-alive set** — ADR 0014 § 7's last rule. A `WeakRef` that has
//!   been dereferenced keeps its target alive for the rest of the **job**, so a
//!   script cannot see one reference answer twice differently. Cleared by
//!   [`Heap::end_job`](super::Heap::end_job).
//!
//! And two that do not exist yet, named here so it is clear they are owed
//! rather than forgotten: the interpreter's frames and its value stack (queue
//! item 72), which for this reason will live in structures the collector walks
//! rather than in Rust locals.
//!
//! **A reference held anywhere but these, across a point where the engine may
//! allocate, is a bug** — and it is a bug that is correct in every ordinary run
//! and wrong only under
//! [`Heap::stress`](super::Heap::stress), which is why that mode is not
//! optional.

use std::collections::HashSet;

use crate::heap::reference::Ref;

/// A place native code puts what it must keep across an allocation.
///
/// Not [`Clone`] and not [`Copy`], which is what makes it impossible to hold on
/// to one after it has been closed: [`Heap::close`](super::Heap::close) takes
/// it by value, so the token that named the scope is gone with it and the depth
/// it stood at cannot be written to by mistake.
#[derive(Debug)]
pub struct Scope {
    depth: usize,
}

impl Scope {
    /// The depth it stood at, taking the token with it.
    ///
    /// By value, which is the whole point of the type: closing a scope spends
    /// the thing that named it, so there is nothing left to close twice.
    pub(in crate::heap) const fn spend(self) -> usize {
        self.depth
    }
}

/// A reference held until somebody gives it back.
///
/// Not [`Clone`] for [`Scope`]'s reason: releasing one twice would release
/// whatever had taken its place.
#[derive(Debug)]
pub struct Root {
    at: usize,
}

impl Root {
    /// Where it is held.
    pub(in crate::heap) const fn at(&self) -> usize {
        self.at
    }

    /// Where it is held, taking the token with it — [`Scope::spend`]'s reason.
    pub(in crate::heap) const fn spend(self) -> usize {
        self.at
    }
}

/// Everything the collector starts from.
#[derive(Debug, Default)]
pub(in crate::heap) struct Roots {
    scoped: Vec<Ref>,
    held: Vec<Option<Ref>>,
    spare: Vec<usize>,
    kept: HashSet<Ref>,
}

impl Roots {
    /// Open a scope. What is held in it lives until it is closed.
    pub(in crate::heap) fn open(&mut self) -> Scope {
        Scope {
            depth: self.scoped.len(),
        }
    }

    /// Keep `held` alive until the innermost open scope closes.
    ///
    /// It takes no scope token, and that is the honest signature rather than a
    /// tidier one: the scopes are a stack of depths into a single list, so a
    /// hold lands in whichever scope is innermost *now*. A version taking a
    /// token could be handed an outer one and would keep the promise the
    /// argument makes only when it was the innermost anyway.
    ///
    /// Holding with no scope open leaks the reference until something closes
    /// below it. Nothing here can see that mistake, so
    /// [`Heap::scoped`](super::Heap::scoped) reports the count and a test of an
    /// engine that has finished its work asserts it is zero.
    pub(in crate::heap) fn hold(&mut self, held: Ref) {
        self.scoped.push(held);
    }

    /// How many references the scopes are holding.
    pub(in crate::heap) fn scoped(&self) -> usize {
        self.scoped.len()
    }

    /// Close a scope, letting go of everything held in it.
    ///
    /// Truncating rather than popping a known number is deliberate: a scope
    /// closed while an inner one is still open closes the inner one too, which
    /// is what unwinding does anyway and is the only behaviour here that cannot
    /// leave the stack describing something that is not true.
    pub(in crate::heap) fn close(&mut self, depth: usize) {
        self.scoped.truncate(depth);
    }

    /// Keep `held` alive until it is released.
    pub(in crate::heap) fn root(&mut self, held: Ref) -> Root {
        if let Some(at) = self.spare.pop() {
            if let Some(slot) = self.held.get_mut(at) {
                *slot = Some(held);
                return Root { at };
            }
        }
        self.held.push(Some(held));
        Root {
            at: self.held.len().saturating_sub(1),
        }
    }

    /// Let go of a root.
    pub(in crate::heap) fn release(&mut self, at: usize) {
        if let Some(slot) = self.held.get_mut(at) {
            *slot = None;
            self.spare.push(at);
        }
    }

    /// What a root is holding, if it is still holding it.
    pub(in crate::heap) fn holding(&self, root: &Root) -> Option<Ref> {
        self.held.get(root.at()).copied().flatten()
    }

    /// Keep `held` alive for the rest of the job.
    ///
    /// A set rather than a list, because a script may dereference the same
    /// `WeakRef` in a loop and a list would grow without bound from one object.
    pub(in crate::heap) fn keep_alive(&mut self, held: Ref) {
        self.kept.insert(held);
    }

    /// The job ended; the keep-alive set goes with it.
    pub(in crate::heap) fn end_job(&mut self) {
        self.kept.clear();
    }

    /// How many references the keep-alive set is holding.
    pub(in crate::heap) fn kept(&self) -> usize {
        self.kept.len()
    }

    /// Every root, in no order that matters — marking is monotone, so the order
    /// they are walked in cannot change what is alive.
    pub(in crate::heap) fn each(&self) -> impl Iterator<Item = Ref> + '_ {
        self.scoped
            .iter()
            .copied()
            .chain(self.held.iter().copied().flatten())
            .chain(self.kept.iter().copied())
    }
}
