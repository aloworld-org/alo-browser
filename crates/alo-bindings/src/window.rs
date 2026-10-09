/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's global object: its `Window` (ADR 0037, queue item 362).
//!
//! ECMAScript lets a host make a realm's global object itself, and HTML
//! does: the global is a `Window`. So [`engine`] makes an engine whose global
//! object is this cell ([`alo_js::interpret::Engine::with_global`]), made
//! from the realm's `Object.prototype` and furnished by the engine with the
//! language's values and builtins exactly as an ordinary one would be.
//! [`crate::install`] then sets its prototype to `Window.prototype`, before
//! any script can run, so no page sees the interval.
//!
//! The cell holds (ADR 0037 § 2):
//!
//! - **an ordinary object's part** — its prototype and its own properties,
//!   which is where every `var`, every function declaration and everything
//!   the realm and the embedder put on the global object live;
//! - **its listener list**, the same [`Listeners`] a node's wrapper holds,
//!   traced and counted the same way: a `Window` is its own wrapper, with no
//!   node behind it for a wrapper to stand in for;
//! - **an edge to its document cell**, its *associated `Document`*, set by
//!   [`crate::install`] — which also gives the document cell an edge back,
//!   so a dispatch that starts at a node finds the window there.
//!
//! The engine roots the global object for the realm's life, so the window,
//! its listeners and its document live as long as the page does.
//!
//! # What is not here
//!
//! The `Window`'s immutable prototype (`[[SetPrototypeOf]]` refusing, queue
//! item 363) is reachable only through `Object.setPrototypeOf`, item 73's;
//! until then the cell's prototype is changeable as an ordinary object's is,
//! which nothing a page can call today can tell apart. There is no
//! `WindowProxy` (ADR 0037 § 4): `window`, `self`, `globalThis` and the
//! top-level `this` are this cell.

use std::rc::Rc;

use alo_js::heap::{Barrier, Field, Ref, Trace, Tracer};
use alo_js::interpret::Engine;
use alo_js::object::{Exotic, Internal, Key, Ordinary, Property};
use alo_js::{Clock, Escape};

use crate::listeners::Listeners;

/// A page's global object.
#[derive(Debug)]
pub struct Window {
    own: Ordinary,
    listeners: Listeners,
    document: Field,
}

impl Window {
    /// Its listeners.
    pub const fn listeners(&self) -> &Listeners {
        &self.listeners
    }

    /// The same, to change — each change through the barrier its methods
    /// take.
    pub const fn listeners_mut(&mut self) -> &mut Listeners {
        &mut self.listeners
    }

    /// The document cell of its associated `Document`, once
    /// [`crate::install`] has given it one.
    pub const fn document(&self) -> Option<Ref> {
        self.document.get()
    }

    /// Give it its document cell, through the barrier.
    pub(crate) fn associate(&mut self, barrier: &mut Barrier, document: Ref) {
        self.document.set(barrier, Some(document));
    }
}

/// How the engine makes a page's global object: a `Window` inheriting from
/// `prototype`, the realm's `Object.prototype`, with no listener and no
/// document yet.
pub fn make(prototype: Option<Ref>) -> Box<dyn Exotic> {
    Box::new(Window {
        own: Ordinary::with_prototype(prototype),
        listeners: Listeners::default(),
        document: Field::default(),
    })
}

/// An engine for a page: one whose global object is a [`Window`], told the
/// time by `clock` or, given none, refusing to say what time it is (ADR 0036
/// § 1). [`crate::install`] requires one.
///
/// # Errors
///
/// As [`Engine::with_global`]: [`Escape::Full`] if the heap cannot hold a
/// realm.
pub fn engine(clock: Option<Rc<dyn Clock>>) -> Result<Engine, Escape> {
    Engine::with_global(make, clock)
}

impl Internal for Window {
    fn own_property(&self, key: Key) -> Option<&Property> {
        self.own.own_property(key)
    }

    fn define_own(&mut self, barrier: &mut Barrier, key: Key, property: Property) -> bool {
        self.own.define_own(barrier, key, property)
    }

    fn delete_own(&mut self, key: Key) -> bool {
        self.own.delete_own(key)
    }

    fn own_keys(&self) -> Vec<Key> {
        self.own.own_keys()
    }

    fn prototype(&self) -> Option<Ref> {
        self.own.prototype()
    }

    fn set_prototype(&mut self, barrier: &mut Barrier, to: Option<Ref>) -> bool {
        self.own.set_prototype(barrier, to)
    }

    fn is_extensible(&self) -> bool {
        self.own.is_extensible()
    }

    fn prevent_extensions(&mut self) -> bool {
        self.own.prevent_extensions()
    }
}

impl Trace for Window {
    fn trace(&self, tracer: &mut Tracer) {
        self.own.trace(tracer);
        self.listeners.trace(tracer);
        self.document.trace(tracer);
    }

    fn footprint(&self) -> usize {
        self.own
            .footprint()
            .saturating_add(self.listeners.footprint())
    }
}

impl Exotic for Window {
    fn describe(&self) -> &'static str {
        "a Window"
    }
}
