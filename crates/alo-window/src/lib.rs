/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The alo browser's window: the browser process a person starts.
//!
//! ADR 0024. Its binary is `alo`. It holds the window and nothing of a page
//! (ADR 0005): it owns the tabs, which own the renderers, and it shows what
//! they paint.
//!
//! # How it is put together
//!
//! - [`window`] is the event loop, the one file that names `winit`, and
//!   [`present`] puts pixels in the window, the one file that names
//!   `softbuffer` (ADR 0024 § 1).
//! - [`conductor`] is the thread that owns the tabs and is the only thing
//!   that ever waits on a renderer; it and the event loop speak only in
//!   [`message`]s (§ 2). It waits on its [`inbox`] and never on a server:
//!   [`network`] is the thread that makes every request (ADR 0041), and
//!   [`hold`] keeps a load's first frame back for its style sheets.
//! - [`showing`] is what the window was last sent, and [`compose`] makes the
//!   window's pixels from it — a function, tested by reference render, placed
//!   by [`place`] and saying what happened to a tab with [`notice`].
//! - [`opening`] is what a person named on the command line, [`fonts`] the
//!   fonts renderers start with, and [`beside`] where the renderer program is.
//! - [`agents`] is the socket an agent outside the browser speaks to (ADR
//!   0055), at a place [`socket_place`] makes private; [`protocol`] reads and
//!   writes its lines, the one file that names `serde_json`, into what
//!   [`asking`] says they mean; [`serving`] answers them from the tabs on the
//!   conductor's thread, kept to the sites a person named in
//!   [`agent_sites`], and [`agent_record`] is what it records. The socket
//!   is a Unix one, and Windows is undecided (ADR 0055 § 1).
//!
//! # What it is not yet
//!
//! One window and one tab. The tab strip is item 297, a person's pointer item
//! 298, painting at the window's scale factor item 299 — until then a CSS
//! pixel is replicated to the scale's whole part (§ 3) — and a link opening a
//! tab item 300.

pub mod agent_record;
pub mod agent_sites;
#[cfg(unix)]
pub mod agents;
pub mod asking;
pub mod beside;
pub mod colours;
pub mod compose;
pub mod conductor;
pub mod fonts;
pub mod hold;
pub mod inbox;
pub mod message;
pub mod network;
pub mod notice;
pub mod opening;
pub mod place;
pub mod present;
pub mod protocol;
pub mod serving;
pub mod showing;
#[cfg(unix)]
pub mod socket_place;
pub mod window;
