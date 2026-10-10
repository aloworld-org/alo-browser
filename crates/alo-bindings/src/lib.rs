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
//! - The global object is the page's [`Window`] (ADR 0037, queue item 362):
//!   an engine made by [`engine()`] has one, which [`install()`] makes whole
//!   — `Window.prototype`, its `window`, `self` and `location`, and an edge
//!   to and from its document. It is an event target: it holds its own
//!   listeners, and every event dispatched at a node of its document but
//!   `load` reaches it last when bubbling and first when capturing
//!   ([`dispatch`]).
//! - The window's `innerWidth`, `innerHeight`, `scrollX`, `scrollY`,
//!   `pageXOffset` and `pageYOffset` (ADR 0038, queue item 366) ask the
//!   page's [`View`], which the embedder hands it with [`show()`]: this crate
//!   has no layout, so it asks, at every read, and a page shown nothing
//!   refuses by name. So do an element's `scrollWidth` and `scrollHeight`
//!   (queue item 370), which the view measures in the middle of the script.
//! - `document.visibilityState` and `document.hidden` (ADR 0039 § 1, queue
//!   item 364) read the [`Visibility`] the renderer states into the
//!   document cell ([`visibility`]); the renderer's task that changes it
//!   fires `visibilitychange` ([`Firing::VISIBILITY_CHANGE`]).
//! - [`introduce()`] puts the page's [`Navigator`] on the global object as
//!   `navigator` (ADR 0030, queue item 325): the user agent string and the
//!   platform the browser process told the renderer, as an [`Identity`], and
//!   nothing this crate composes.
//!
//! - `location`, on the global object and on the document, is the page's one
//!   [`Location`] (queue item 360), made by [`install()`] and reading the
//!   document's URL each time it is asked; everything that would navigate
//!   is refused by name until item 85 ([`location`]).
//!
//! - An element's `classList` is a [`TokenList`] (queue item 327), made once
//!   and kept by its wrapper, computing its tokens from the `class`
//!   attribute each time it is asked ([`token_list`]).
//! - An HTML or SVG element's `style` is a [`StyleDeclaration`] (ADR 0033,
//!   queue item 342), made once and kept by its wrapper the same way, which
//!   parses the `style` attribute every time it is asked and writes every
//!   change back to it ([`style_declaration`]). Whether that attribute is
//!   applied under the page's policies is one function, which the
//!   renderer's draw and the declaration both ask, so a refused attribute
//!   reads as `""` (ADR 0034, queue item 343, [`style_policy`]).
//! - `querySelectorAll` on a document, an element or a fragment answers a
//!   static [`NodeList`] (queue item 329): `alo-css` parses the string and
//!   its one matcher matches it, and the list holds each match's wrapper,
//!   answering its indices as Web IDL's indexed getter ([`node_list`]).
//!
//! - `fetch` on the global object (ADR 0032, queue item 335) is an **ask**
//!   recorded in the document cell under a fresh number ([`fetching`]), and
//!   answers a pending promise; its arguments are converted by Web IDL's
//!   rules ([`fetch_init`]) and its request made by Fetch's steps
//!   ([`fetch`]), and every failure rejects. The renderer takes the asks
//!   with [`fetching::take`] and delivers each answer as a task
//!   ([`delivering::answer`]), settling the promise with a read-only
//!   [`Response`] and its [`Headers`] ([`response`], [`headers`]).
//! - `navigator.sendBeacon` (ADR 0040, queue item 369) is an ask too, one
//!   that claims to outlive its page ([`fetching::Keepalive`]); the cell
//!   counts what such asks have in flight against
//!   [`fetching::MOST_KEPT_ALIVE`] so the call can answer `false`, and
//!   takes each off the count as its answer arrives
//!   ([`fetching::answered`]; the call's steps are `beacon.rs`'s).
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
//! item 255), and an agent's `Activate` is one: a `click` that is a
//! `PointerEvent` ([`Firing::CLICK`], queue item 256), its chain `UIEvent`,
//! `MouseEvent` and `PointerEvent`, none with a constructor on the global.
//! A script's `el.click()` (queue item 261) is `HTMLElement`'s, driven from
//! its native as `dispatchEvent`'s is (`scripted.rs`), with the same
//! activation rule around it (`clicking.rs` holds what it keeps); a link
//! it activates is followed by asking, kept in the document cell until the
//! renderer answers ([`navigating`], ADR 0020, queue item 263).
//! An agent's `PutText` is one too: a `beforeinput` and an `input` that are
//! `InputEvent`s ([`Firing::before_replacing`], [`Firing::replaced`], queue
//! item 257), inheriting `UIEvent`, also without a constructor.

mod beacon;
mod clicking;
mod define;
pub mod delivering;
mod dictionary;
pub mod dispatch;
pub mod document_cell;
pub mod embed;
pub mod event;
pub mod fetch;
mod fetch_init;
pub mod fetching;
pub mod headers;
mod idl;
pub mod install;
pub mod interface;
pub mod listeners;
pub mod liveness;
pub mod location;
pub mod navigating;
pub mod navigator;
pub mod node_list;
pub mod response;
mod scripted;
pub mod style_declaration;
mod style_names;
pub mod style_policy;
pub mod token_list;
mod tokens;
pub mod tree;
mod unforgeable;
pub mod view;
pub mod visibility;
pub mod window;
pub mod wrapper;

pub use document_cell::{DocumentCell, Released};
pub use embed::{Unadopted, Wrapping, adopt, change_document, document, node_of, wrap};
pub use event::{Event, Fired, Firing, Shape};
pub use fetch::offer;
pub use headers::Headers;
pub use install::{furnish, install};
pub use interface::dom_exception::DomException;
pub use interface::{Interface, Interfaces, prototype_of};
pub use location::Location;
pub use navigator::{Identity, Navigator, introduce};
pub use node_list::NodeList;
pub use response::{Responded, Response};
pub use style_declaration::StyleDeclaration;
pub use token_list::TokenList;
pub use view::{Extent, Scrolled, Unmeasured, View, show};
pub use visibility::Visibility;
pub use window::{Window, engine};
pub use wrapper::Wrapper;
