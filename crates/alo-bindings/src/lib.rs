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
//!
//! # What is not here yet
//!
//! **No script can reach any of it.** The interfaces — `Node`, `Element`,
//! `Document`, `Text`, each in a file of its own with its prototype and its
//! members — `DOMException`, the brand check's `TypeError` and `document` on
//! the global object are queue item 249; the renderer moving a page's
//! document in when its first script runs, and rendering again when the
//! change count says what it holds is stale, are item 250. This crate is the
//! part both stand on: what a wrapper is and how long it lives, which is the
//! clause a script can observe and the one that had to be right first.

pub mod document_cell;
pub mod embed;
pub mod liveness;
pub mod tree;
pub mod wrapper;

pub use document_cell::{DocumentCell, Released};
pub use embed::{Wrapping, adopt, change_document, document, node_of, wrap};
pub use wrapper::Wrapper;
