/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `CSSStyleDeclaration` as a script holds it: `el.style` (ADR 0033 § 3,
//! queue item 342).
//!
//! An embedder cell holding **its element's wrapper** and an ordinary
//! object's part, built exactly as `classList` is ([`crate::token_list`]).
//! It holds no declarations: every member parses the element's `style`
//! attribute through the wrapper ([`alo_css::InlineStyle`]), and every
//! change is written back to the attribute, so the attribute and the
//! declaration are one fact and cannot disagree.
//!
//! The ordinary part is where a page's write to a name this engine does not
//! act on lands — `el.style.cursor = "default"` — as an expando does on any
//! object (ADR 0033 § 4).
//!
//! # `[SameObject]`
//!
//! `style` answers the same object every time, so the element's wrapper
//! holds its declaration once it has made one ([`crate::Wrapper::style`]),
//! and the declaration holds the wrapper. Both edges are strong: a page
//! holding only `el.style` keeps `el`'s wrapper and so its tree (ADR 0017
//! § 3), and the cycle is a cycle the collector traces like any other.
//!
//! The declaration is made by [`made`], from the getter on
//! `HTMLElement.prototype` and `SVGElement.prototype`
//! ([`crate::interface::element_css_inline_style`]); its members are
//! [`crate::interface::css_style_declaration`]'s.

use alo_js::heap::{Barrier, Field, Ref, Trace, Tracer};
use alo_js::object::{Exotic, Internal, Key, Objects, Ordinary, Property};
use alo_js::{Escape, Fault};

use crate::document_cell::DocumentCell;
use crate::interface::Interface;
use crate::wrapper::Wrapper;

/// An element's `style`.
#[derive(Debug)]
pub struct StyleDeclaration {
    own: Ordinary,
    element: Field,
}

impl StyleDeclaration {
    /// The wrapper of the element whose `style` attribute this declares.
    pub const fn element(&self) -> Option<Ref> {
        self.element.get()
    }
}

impl Internal for StyleDeclaration {
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

impl Trace for StyleDeclaration {
    fn trace(&self, tracer: &mut Tracer) {
        self.element.trace(tracer);
        self.own.trace(tracer);
    }

    fn footprint(&self) -> usize {
        self.own.footprint()
    }
}

impl Exotic for StyleDeclaration {
    fn describe(&self) -> &'static str {
        "a CSSStyleDeclaration"
    }
}

/// The `style` of the element `wrapper` is, in the document `owner` holds,
/// for a member at source position `at`: the one it already has, or one
/// made now, inheriting from `CSSStyleDeclaration.prototype`, and kept by
/// the wrapper.
///
/// **A safepoint** when the declaration is new. `wrapper` must be held by the
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
        .style();
    if let Some(declaration) = had {
        return Ok(declaration);
    }
    let prototype = objects
        .embedded::<DocumentCell>(owner)
        .ok_or(Escape::fault(Fault::NotAnObject))?
        .interfaces()
        .prototype(Interface::CssStyleDeclaration)
        .ok_or(Escape::fault(Fault::Gone))?;
    let declaration = StyleDeclaration {
        own: Ordinary::with_prototype(Some(prototype)),
        element: Field::holding(wrapper),
    };
    let made = objects
        .foreign(Box::new(declaration))
        .map_err(|why| Escape::refused(why, at))?;
    // Kept by the wrapper before anything else allocates.
    objects
        .write_embedded::<Wrapper, _>(wrapper, |held, barrier| held.keep_style(barrier, made))
        .ok_or(Escape::fault(Fault::NotAnObject))?;
    Ok(made)
}
