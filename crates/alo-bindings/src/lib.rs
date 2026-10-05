/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The alo browser DOM bindings: where a script's objects and a page's nodes
//! meet.
//!
//! ADR 0017 § 1: this is the **only** crate that depends on both `alo-js` and
//! `alo-dom`. The engine knows nothing of the DOM (ADR 0013 § 6) and the DOM
//! nothing of an engine, so a renderer that never runs script never builds a
//! heap, and stage 1 renders exactly as it did.
//!
//! # What is here
//!
//! - [`DocumentCell`] is a page's document once it is in the page's heap
//!   (§ 2): one cell, one owner, its footprint the document's size, and the
//!   table from node to wrapper.
//! - [`Wrapper`] is a node as a script holds it: its id, its document, and an
//!   ordinary object's part for the expandos every page hangs off a node.
//! - The cell keeps each wrapper **as long as its tree is reachable** and
//!   releases the detached trees nothing holds (§ 3) — [`liveness`], walking
//!   trees by [`tree`].
//! - [`adopt`], [`wrap`], [`node_of`], [`document`] and [`change_document`]
//!   are how a caller puts the document in, has a node's one object, asks what
//!   node an object is (§ 4), and reads or changes the document through
//!   `alo-dom`'s own operations (§ 5).
//! - [`interface`] is what a script sees: one file per interface — `Node`,
//!   `Element`, `Document`, the `ChildNode` mixin and `DOMException` — each
//!   with its prototype and its members (§§ 1, 5 and 8), all of them natives
//!   that reach their node through their `this` and nothing else, behind
//!   Web IDL's brand check, argument count and conversions (`idl.rs`).
//! - [`install()`] makes the prototypes in an engine's realm and puts the
//!   document on its global object as `document`; [`furnish()`] makes them
//!   without the global, for a second document.
//!
//! # What is not here yet
//!
//! The renderer hands a page's document over when its first script is about
//! to run, and draws again when the change count says what it holds is stale
//! (`alo-renderer`'s `held.rs`, queue item 250). **A parser-inserted script
//! running at its own end tag** is item 247: today every script sees the
//! whole parsed document. The members are item 80's and no more — everything
//! else a page reaches for is absent, so `typeof` answers `"undefined"`
//! (ADR 0017 § 8).

mod define;
pub mod document_cell;
pub mod embed;
mod idl;
pub mod install;
pub mod interface;
pub mod liveness;
pub mod tree;
pub mod wrapper;

pub use document_cell::{DocumentCell, Released};
pub use embed::{Unadopted, Wrapping, adopt, change_document, document, node_of, wrap};
pub use install::{furnish, install};
pub use interface::dom_exception::DomException;
pub use interface::{Interface, Interfaces, prototype_of};
pub use wrapper::Wrapper;
