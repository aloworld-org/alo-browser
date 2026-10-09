/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An event target's listener list (ADR 0018 § 1, queue item 254).
//!
//! **It is a field of the target's wrapper**: each entry its callback, its
//! type, and its `capture`, `once` and `passive`. The callback is a strong
//! edge of the wrapper, so a listener lives exactly as long as its node's
//! wrapper does (ADR 0017 § 3) — and a detached tree nothing in script
//! reaches goes at the next collection, listener, closure and all. A node
//! that was never wrapped has no list, which is why a dispatch asks the
//! document cell's table rather than making a wrapper to find out.
//!
//! # A listener is named by a number, and *removed* is being gone
//!
//! The standard copies a target's list when a dispatch reaches it and gives
//! each listener a *removed* flag, so that one removed during the dispatch is
//! skipped even though the copy still names it. Here every listener is given
//! an id when it is added — never reused within its list — and the copy a
//! dispatch takes is of ids. A listener that has been removed is one whose id
//! the list no longer has, so the copy names it and finds nothing: the flag,
//! without a second place to keep it.
//!
//! # A node's wrapper or the window
//!
//! The window holds a list too (ADR 0037 § 2), and it is reached through
//! the same two functions as a node's — [`of`] to read and [`change`] to
//! write — so `addEventListener`'s options, `handleEvent`, `once`, `passive`
//! and the *removed* rule cannot come to differ between the two.
//!
//! # Its size is counted
//!
//! Every entry counts in its wrapper's footprint, type and all (ADR 0014
//! § 9), so a page that adds listeners for ever meets the heap's ceiling as
//! an array that grows for ever does.

use alo_js::heap::{Barrier, Field, Ref, Tracer};
use alo_js::object::Objects;

use crate::window::Window;
use crate::wrapper::Wrapper;

/// The listener list of `target` — a node's wrapper or a `Window` — or
/// [`None`] for any other object.
pub fn of(objects: &Objects, target: Ref) -> Option<&Listeners> {
    match objects.embedded::<Wrapper>(target) {
        Some(wrapper) => Some(wrapper.listeners()),
        None => objects.embedded::<Window>(target).map(Window::listeners),
    }
}

/// Change the listener list of `target` — a node's wrapper or a `Window` —
/// with `change`, answering what it answers, or [`None`] for any other
/// object.
///
/// Not a safepoint: nothing in `change` can reach the heap but the barrier.
pub fn change<R>(
    objects: &mut Objects,
    target: Ref,
    change: impl FnOnce(&mut Listeners, &mut Barrier) -> R,
) -> Option<R> {
    if objects.embedded::<Wrapper>(target).is_some() {
        return objects.write_embedded::<Wrapper, _>(target, |held, barrier| {
            change(held.listeners_mut(), barrier)
        });
    }
    objects.write_embedded::<Window, _>(target, |held, barrier| {
        change(held.listeners_mut(), barrier)
    })
}

/// One listener: what is called, and when.
#[derive(Debug)]
pub struct Listener {
    id: u64,
    kind: Vec<u16>,
    callback: Field,
    capture: bool,
    once: bool,
    passive: bool,
}

impl Listener {
    /// The callback: a function, or an object whose `handleEvent` is called.
    pub const fn callback(&self) -> Option<Ref> {
        self.callback.get()
    }

    /// Whether it is called in the capture phase rather than the bubble.
    pub const fn capture(&self) -> bool {
        self.capture
    }

    /// Whether it is removed before its first call.
    pub const fn once(&self) -> bool {
        self.once
    }

    /// Whether `preventDefault` does nothing while it runs.
    pub const fn passive(&self) -> bool {
        self.passive
    }
}

/// What a listener is, before it is added.
#[derive(Debug, Clone, Copy)]
pub struct Wanted<'a> {
    /// The event type it listens for, as the code units a page gave.
    pub kind: &'a [u16],
    /// Its callback.
    pub callback: Ref,
    /// `capture`.
    pub capture: bool,
    /// `once`.
    pub once: bool,
    /// `passive`.
    pub passive: bool,
}

/// A target's listeners, in the order they were added.
#[derive(Debug, Default)]
pub struct Listeners {
    entries: Vec<Listener>,
    next: u64,
}

impl Listeners {
    /// Add a listener, unless one with the same type, callback and `capture`
    /// is already there — the standard's *add an event listener* — and say
    /// whether it was added.
    pub fn add(&mut self, barrier: &mut Barrier, wanted: Wanted<'_>) -> bool {
        if self
            .position(wanted.kind, wanted.callback, wanted.capture)
            .is_some()
        {
            return false;
        }
        let id = self.next;
        self.next = self.next.saturating_add(1);
        let mut callback = Field::empty();
        callback.set(barrier, Some(wanted.callback));
        self.entries.push(Listener {
            id,
            kind: wanted.kind.to_vec(),
            callback,
            capture: wanted.capture,
            once: wanted.once,
            passive: wanted.passive,
        });
        true
    }

    /// Remove the listener with this type, callback and `capture`, if there
    /// is one — *remove an event listener* — and say whether there was.
    pub fn remove(
        &mut self,
        barrier: &mut Barrier,
        kind: &[u16],
        callback: Ref,
        capture: bool,
    ) -> bool {
        match self.position(kind, callback, capture) {
            Some(at) => {
                self.take(barrier, at);
                true
            }
            None => false,
        }
    }

    /// Remove the listener with this id, which a `once` listener is before
    /// it is called.
    pub fn remove_id(&mut self, barrier: &mut Barrier, id: u64) {
        if let Some(at) = self.entries.iter().position(|entry| entry.id == id) {
            self.take(barrier, at);
        }
    }

    /// The listener with this id, if it has not been removed.
    pub fn get(&self, id: u64) -> Option<&Listener> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// The ids of every listener for `kind`, in order: the copy a dispatch
    /// takes when it reaches this target.
    pub fn matching(&self, kind: &[u16]) -> Vec<u64> {
        self.entries
            .iter()
            .filter(|entry| entry.kind == kind)
            .map(|entry| entry.id)
            .collect()
    }

    /// How many listeners there are.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every callback, as a strong edge of the wrapper that holds the list.
    pub fn trace(&self, tracer: &mut Tracer) {
        for entry in &self.entries {
            entry.callback.trace(tracer);
        }
    }

    /// The bytes the list owns.
    pub fn footprint(&self) -> usize {
        self.entries.iter().fold(
            self.entries
                .capacity()
                .saturating_mul(size_of::<Listener>()),
            |sum, entry| sum.saturating_add(entry.kind.capacity().saturating_mul(2)),
        )
    }

    /// Where the listener with this type, callback and `capture` is.
    fn position(&self, kind: &[u16], callback: Ref, capture: bool) -> Option<usize> {
        self.entries.iter().position(|entry| {
            entry.capture == capture && entry.callback.get() == Some(callback) && entry.kind == kind
        })
    }

    /// Take the entry at `at` out, telling the collector its callback went.
    fn take(&mut self, barrier: &mut Barrier, at: usize) {
        if at < self.entries.len() {
            let mut gone = self.entries.remove(at);
            gone.callback.set(barrier, None);
        }
    }
}
