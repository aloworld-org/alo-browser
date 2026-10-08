/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `DOMTokenList` as a script holds it: `el.classList` (queue item 327).
//!
//! An embedder cell holding **its element's wrapper** and an ordinary
//! object's part. It holds no tokens: every member reads the element's
//! `class` attribute through the wrapper and computes the set from it
//! ([`crate::tokens`]), so the list is live by construction rather than kept
//! in step with the attribute.
//!
//! # `[SameObject]`
//!
//! `classList` answers the same object every time, so the element's wrapper
//! holds its list once it has made one ([`crate::Wrapper::class_list`]), and
//! the list holds the wrapper. Both edges are strong: a page holding only
//! `el.classList` keeps `el`'s wrapper and so its tree (ADR 0017 § 3), and
//! the cycle is a cycle the collector traces like any other.
//!
//! The list is made by [`made`], from the getter on `Element.prototype`; its
//! members are [`crate::interface::dom_token_list`]'s.

use alo_js::heap::{Barrier, Field, Ref, Trace, Tracer};
use alo_js::object::{Exotic, Internal, Key, Objects, Ordinary, Property};
use alo_js::{Escape, Fault};

use crate::document_cell::DocumentCell;
use crate::interface::Interface;
use crate::wrapper::Wrapper;

/// An element's `classList`.
#[derive(Debug)]
pub struct TokenList {
    own: Ordinary,
    element: Field,
}

impl TokenList {
    /// The wrapper of the element whose `class` this is the list of.
    pub const fn element(&self) -> Option<Ref> {
        self.element.get()
    }
}

impl Internal for TokenList {
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

impl Trace for TokenList {
    fn trace(&self, tracer: &mut Tracer) {
        self.element.trace(tracer);
        self.own.trace(tracer);
    }

    fn footprint(&self) -> usize {
        self.own.footprint()
    }
}

impl Exotic for TokenList {
    fn describe(&self) -> &'static str {
        "a DOMTokenList"
    }
}

/// The `classList` of the element `wrapper` is, in the document `owner`
/// holds, for a member at source position `at`: the one it already has, or one made now, inheriting from
/// `DOMTokenList.prototype`, and kept by the wrapper.
///
/// **A safepoint** when the list is new. `wrapper` must be held by the
/// caller — a member's `this` is.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling; a fault when `wrapper` is not
/// a wrapper or the interfaces were never made.
pub(crate) fn made(
    objects: &mut Objects,
    owner: Ref,
    wrapper: Ref,
    at: usize,
) -> Result<Ref, Escape> {
    let had = objects
        .embedded::<Wrapper>(wrapper)
        .ok_or(Escape::fault(Fault::NotAnObject))?
        .class_list();
    if let Some(list) = had {
        return Ok(list);
    }
    let prototype = objects
        .embedded::<DocumentCell>(owner)
        .ok_or(Escape::fault(Fault::NotAnObject))?
        .interfaces()
        .prototype(Interface::DomTokenList)
        .ok_or(Escape::fault(Fault::Gone))?;
    let list = TokenList {
        own: Ordinary::with_prototype(Some(prototype)),
        element: Field::holding(wrapper),
    };
    let made = objects
        .foreign(Box::new(list))
        .map_err(|why| Escape::refused(why, at))?;
    // Kept by the wrapper before anything else allocates.
    objects
        .write_embedded::<Wrapper, _>(wrapper, |held, barrier| held.keep_class_list(barrier, made))
        .ok_or(Escape::fault(Fault::NotAnObject))?;
    Ok(made)
}
