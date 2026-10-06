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
//! - Events (ADR 0018, queue item 254): `EventTarget` at the top of every
//!   node's chain, a [`listeners`] list in each wrapper, the [`Event`] cell
//!   holding a dispatch's state, and the DOM standard's [`dispatch`]
//!   algorithm written once as a stepper that `dispatchEvent` drives — and
//!   that the renderer's event loop drives for an event the browser makes
//!   with [`event::create`] (queue item 255).
//! - `isTrusted` (queue item 260, ADR 0019): each interface's
//!   `[LegacyUnforgeable]` members live on an unforgeables object the
//!   document cell holds, copied onto every instance as it is made — by the
//!   constructors, which find the cell through the realm's `[[HostDefined]]`,
//!   and by [`event::create`].
//! - [`install()`] names the document cell as the realm's host, makes the
//!   prototypes in an engine's realm and puts the document on its global
//!   object as `document`; [`furnish()`] makes them without the global, for a
//!   second document.
//!
//! # What is not here yet
//!
//! The renderer hands a page's document over when its first script is about
//! to run, and draws again when the change count says what it holds is stale
//! (`alo-renderer`'s `held.rs`, queue item 250), and runs each of a page's
//! scripts at its own end tag against the document parsed so far, lending
//! the document back to the parser between them (queue item 247). The
//! members are item 80's and item 254's and no more — everything else a page
//! reaches for is absent, so `typeof` answers `"undefined"` (ADR 0017 § 8).
//! The browser's dispatch is a task the renderer's event loop runs (queue
//! item 255), but nothing the browser does fires one yet: an agent's
//! `Activate` is queue item 256.

mod define;
mod dictionary;
pub mod dispatch;
pub mod document_cell;
pub mod embed;
pub mod event;
mod idl;
pub mod install;
pub mod interface;
pub mod listeners;
pub mod liveness;
pub mod tree;
mod unforgeable;
pub mod wrapper;

pub use document_cell::{DocumentCell, Released};
pub use embed::{Unadopted, Wrapping, adopt, change_document, document, node_of, wrap};
pub use event::{Event, Firing};
pub use install::{furnish, install};
pub use interface::dom_exception::DomException;
pub use interface::{Interface, Interfaces, prototype_of};
pub use wrapper::Wrapper;
