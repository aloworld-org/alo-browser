/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The alo browser document tree.
//!
//! `html5ever` parses; **we hold the tree**. That split is ADR 0001: the HTML
//! parser is rented because parsing HTML to specification is physics and
//! carries none of this engine's value, while the tree it produces is the
//! thing every later stage — style, layout, paint, and the agent surface of
//! ADR 0002 — reads and reasons about. So the tree is ours, in our types, and
//! [`parse`] is the only module that has ever heard of `html5ever`.
//!
//! # What is here
//!
//! - [`Document`] owns every node and is the only thing that can change the
//!   links between them.
//! - [`NodeId`] names one node, for the life of that document, and is never
//!   reused (ADR 0003). The agent tree will need to name a node and come back
//!   to it, and identity added later means rewriting everything holding a
//!   reference — which is why it is here in the first commit.
//! - [`Node`], [`NodeKind`], [`Element`] and [`Attribute`] are what a node is.
//! - [`QualifiedName`] and [`Namespace`] are how one is named.
//! - [`Document::insert_before`], [`Document::append_child`],
//!   [`Document::replace_child`], [`Document::remove_child`] and
//!   [`Document::remove`] change a tree's shape under the DOM standard's
//!   validity rules ([`mutation`], [`validity`]), answering a [`Refusal`] the
//!   standard names rather than `false`; [`Document::release`] lets go of a
//!   detached tree nothing reaches ([`release`]).
//!
//! # What is not here, by decision
//!
//! **A JavaScript engine.** Script reaches these operations through the
//! bindings (ADR 0017 § 1); this crate depends on no engine, so a page that
//! never runs script never builds one.
//!
//! **Quirks mode.** Law 1 refuses it. What the parser thought is kept in
//! [`Document::quirks_signal`] and never acted on.
//!
//! **The legacy DOM surface.** No `document.write`, no live collections (law
//! 1; ADR 0017 § 8).
//!
//! # Example
//!
//! ```
//! use alo_dom::{NodeKind, parse_document};
//!
//! let document = parse_document("<p class=lead>alo <em>browser</em></p>");
//! let paragraph = document
//!     .descendants(document.root())
//!     .find(|id| document.get(*id).is_some_and(|n| n.is_html_element("p")))
//!     .expect("the parser gives every document a body with our <p> in it");
//!
//! assert_eq!(document.text_content(paragraph), "alo browser");
//! assert_eq!(
//!     document.element(paragraph).and_then(|e| e.attr("class")),
//!     Some("lead"),
//! );
//! assert_eq!(
//!     document.serialize_node(paragraph),
//!     r#"<p class="lead">alo <em>browser</em></p>"#,
//! );
//! ```

pub mod document;
pub mod mutation;
pub mod name;
pub mod node;
pub mod parse;
pub mod release;
pub mod scripts;
pub mod serialize;
pub mod sheets;
pub mod validity;

pub use document::{Children, Descendants, Document, QuirksSignal};
pub use name::{Namespace, QualifiedName};
pub use node::{Attribute, Element, Node, NodeId, NodeKind};
pub use parse::{ParseIssue, parse_document, parse_fragment};
pub use validity::Refusal;
