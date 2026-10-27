/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Everything the frame of ours around Chrome's window is told (ADR 0062,
//! queue items 642, 679 and 681).
//!
//! By the thread holding Chrome, three things: where to stand, as Chrome's
//! window moves, hides and closes ([`Follow`], [`crate::chrome_follow`]);
//! what an agent's verb was answered with, for the person watching
//! ([`AgentLine`], queue items 623 and 625 through Chrome) — the line
//! [`AgentLine::of`] makes of the answer this process sent, so nothing a
//! page or Chrome says can become one; and what Chrome calls the page and
//! where it is ([`Named`], [`crate::chrome_named`]). The frame keeps the
//! lines as this engine's window keeps them
//! ([`crate::agent_lines::AgentLines`]).
//!
//! And by the thread holding the frame's tab strip
//! ([`crate::frame_strip`]), what the strip's own renderer painted of that
//! name, or why it painted nothing: the page's tab is drawn there and
//! nowhere in this process (ADR 0024 § 4).

use crate::agent_lines::AgentLine;
use crate::chrome_follow::Follow;
use crate::chrome_protocol::Named;
use alo_renderer::Frame;

/// What the frame around Chrome's window is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToFrame {
    /// Where to stand, or that Chrome's window is gone.
    Stand(Follow),
    /// What an agent's verb was answered with: a sentence of a verb that
    /// ran, or a refusal.
    Agent(AgentLine),
    /// What Chrome calls the page, and where it is: a stranger's strings,
    /// which the frame hands to its strip's renderer and never draws.
    Named(Named),
    /// The page's tab, as the strip's renderer painted it, in device
    /// pixels at the frame's scale.
    Strip(Frame),
    /// Why the strip's renderer painted no tab, in this browser's words.
    StripSaid(String),
}
