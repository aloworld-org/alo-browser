/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The renderer: everything that touches a page, behind one boundary.
//!
//! ADR 0005 decides the shape of a browser before there is a hostile page to
//! defend against: a privileged **browser process** owns the network, the
//! disk, the display and the user; a **renderer per site** owns everything
//! that touches a page and has almost no privilege; work crosses as typed
//! messages **in one direction**.
//!
//! This crate is the renderer's side of that. Today it runs in the same
//! process as everything else, and that is deliberate — the expensive half of
//! the decision is the **shape**, not the `fork`. An engine written against a
//! synchronous, ambient, reach-anywhere API cannot be pulled apart afterwards
//! without rewriting everything that used it. So the boundary exists now, and
//! queue item 29 changes the transport.
//!
//! # What the boundary promises
//!
//! - **One direction.** [`Renderer::handle`] takes a [`ToRenderer`] and
//!   returns a [`FromRenderer`]. There is no callback in the signature, no
//!   handle to call back through, and nowhere to wait.
//! - **Nothing ambient.** Everything a renderer needs arrives in the message
//!   or in its constructor. Two renderers built the same way and sent the same
//!   messages answer the same way.
//! - **Everything crossing is a value that could be sent.** Every message type
//!   is owned, `Clone` and `Send + 'static` — no borrows, no lifetimes, no
//!   pointers into somebody else's tree. That is the property that makes a
//!   transport possible; choosing the transport is item 29's, and inventing a
//!   wire format before there is a process to send it to would be inventing.
//! - **A refusal is an answer.** A request the renderer cannot serve comes back
//!   as [`FromRenderer::Failed`] and leaves it usable. Nothing panics its way
//!   out of a page.
//!
//! # What is inside, and why the corpus reaches in
//!
//! [`pipeline`] is the engine's stages in one call, and it produces every
//! intermediate tree. Those never cross the boundary — a display list is not
//! something a browser process asks for. The corpus asserts on them anyway,
//! because it is a test of the engine's insides and ADR 0005 says tests stay
//! single-process. The pipeline is *inside* the renderer, and so is the
//! corpus.

pub mod face;
pub mod fonts;
pub mod frame;
pub mod host;
pub mod message;
pub mod page;
pub mod pipe;
pub mod pipeline;
pub mod renderer;
pub mod sandbox;
pub mod serve;
pub mod site;
pub mod snapshot;
pub mod wire;

pub use frame::Frame;
pub use message::{Failure, FromRenderer, ToRenderer};
pub use page::Page;
pub use pipeline::{
    Rendered, render, render_document, render_document_with, render_with, render_with_resources,
};
pub use renderer::Renderer;
pub use snapshot::{Snapshot, SnapshotNode};
