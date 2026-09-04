/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The internal methods, as a trait — and it is the trait an embedder gets.
//!
//! ADR 0014 § 11: *internal methods are a trait, and it is the **same** trait
//! ADR 0013 § 6 promised the embedder. The specification already writes arrays,
//! functions, proxies and the DOM's own oddities as exotic objects with their
//! own internal methods, so one mechanism serves both and there is no second one
//! to keep in step.*
//!
//! An engine that answered this differently — ordinary objects in an enum, the
//! DOM behind a callback, a proxy special-cased in the interpreter — has three
//! places to change when the specification adds a rule and three chances to
//! change only two of them. `document.all`, an `HTMLCollection`'s indices and a
//! `Location` that refuses to be redefined are all exotic in exactly the sense
//! the specification means, and each is an [`Exotic`] here rather than a case in
//! this crate.
//!
//! # What is in the trait and what is beside it
//!
//! The five here are the **own-property** internal methods, and they are the
//! ones an exotic object genuinely reimplements: they need nothing but the
//! object itself. The chain-walking ones — `[[Get]]`, `[[Set]]`,
//! `[[HasProperty]]` — are written once in [`access`](super::access) in terms
//! of these, which is how the specification itself defines `OrdinaryGet`.
//!
//! **A proxy is the case that will want more**, because it overrides `[[Get]]`
//! rather than `[[GetOwnProperty]]`, and it does so by calling a script's trap:
//! it needs the interpreter (queue item 72) before it needs anything here. When
//! it lands, what it adds is a way for an exotic object to intercept the walk —
//! and it will be added to this trait rather than beside it, for the reason
//! above.
//!
//! # And what an exotic object may not do
//!
//! It may not allocate. These methods are called with a cell borrowed out of
//! the heap, so an allocation would be a collection with the borrow live — the
//! borrow checker refuses it rather than trusting anybody to remember, which is
//! the ordinary Rust reason this design is cheaper here than in C++.

use crate::heap::{Barrier, Ref, Trace};

use super::key::Key;
use super::property::Property;

/// The own-property internal methods of an object.
///
/// Implemented by the ordinary object ([`Ordinary`](super::Ordinary)) and by
/// anything an embedder puts in the heap that wants to behave like an object.
pub trait Internal {
    /// `[[GetOwnProperty]]`: the property this object itself has under `key`.
    fn own_property(&self, key: Key) -> Option<&Property>;

    /// The same, to be written to — which is how a data property's value is
    /// changed without redefining it.
    fn own_property_mut(&mut self, key: Key) -> Option<&mut Property>;

    /// `[[DefineOwnProperty]]`, after the caller has decided it is allowed.
    ///
    /// The validation — is it configurable, is the object extensible — is
    /// [`access`](super::access)'s, because it is the same for every object and
    /// an exotic one that reimplemented it would be a place for the rules to
    /// drift. What this does is the storing, and an exotic object that refuses
    /// a key for a reason of its own answers `false` here.
    fn define_own(&mut self, barrier: &mut Barrier, key: Key, property: Property) -> bool;

    /// `[[Delete]]`, after the caller has checked the property is configurable.
    fn delete_own(&mut self, key: Key) -> bool;

    /// `[[OwnPropertyKeys]]`, in the specification's order.
    fn own_keys(&self) -> Vec<Key>;

    /// `[[GetPrototypeOf]]`.
    fn prototype(&self) -> Option<Ref>;

    /// `[[SetPrototypeOf]]`, after the caller has checked for a cycle and for
    /// extensibility.
    ///
    /// An exotic object with an immutable prototype — `Object.prototype`
    /// itself, and a `Location` — answers `false`.
    fn set_prototype(&mut self, barrier: &mut Barrier, to: Option<Ref>) -> bool;

    /// `[[IsExtensible]]`.
    fn is_extensible(&self) -> bool;

    /// `[[PreventExtensions]]`.
    fn prevent_extensions(&mut self) -> bool;
}

/// An object an embedder put in the heap.
///
/// ADR 0013 § 6: *`alo-js` defines one trait for objects an embedder supplies,
/// and the DOM is one embedder among several — the console and a test harness
/// are others.* The demand is [`Trace`], because the thing is in the graph the
/// collector walks; [`Internal`] is what makes it an **object** rather than
/// something merely stored, and the two together are the whole contract.
///
/// A node's wrapper (queue item 80) is one of these, and it is the only thing
/// that will depend on both `alo-js` and `alo-dom`.
///
/// [`Debug`](std::fmt::Debug) is asked for as well, and it is not decoration: a
/// [`Cell`](super::Cell) is what a heap of hostile objects is made of, and a
/// heap nobody can print is a heap nobody can debug when a collection goes
/// wrong at three in the morning.
pub trait Exotic: Internal + Trace + std::fmt::Debug {
    /// What this object is, for a message a person reads.
    ///
    /// Not a class name a script can see — that is `Symbol.toStringTag` and it
    /// is an ordinary property. This is for the engine's own reports, so that a
    /// refusal names `an HTMLDivElement` rather than `a foreign object`.
    fn describe(&self) -> &'static str;
}
