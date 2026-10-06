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
//!   [`message`]s (§ 2).
//! - [`showing`] is what the window was last sent, and [`compose`] makes the
//!   window's pixels from it — a function, tested by reference render, placed
//!   by [`place`] and saying what happened to a tab with [`notice`].
//! - [`opening`] is what a person named on the command line, [`fonts`] the
//!   fonts renderers start with, and [`beside`] where the renderer program is.
//!
//! # What it is not yet
//!
//! One window and one tab. The tab strip is item 297, a person's pointer item
//! 298, painting at the window's scale factor item 299 — until then a CSS
//! pixel is replicated to the scale's whole part (§ 3) — and a link opening a
//! tab item 300.

pub mod beside;
pub mod colours;
pub mod compose;
pub mod conductor;
pub mod fonts;
pub mod message;
pub mod notice;
pub mod opening;
pub mod place;
pub mod present;
pub mod showing;
pub mod window;
