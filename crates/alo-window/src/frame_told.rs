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
//! To the conversation, which shows a page as a card rather than beside a
//! frame (ADR 0076 B, queue item 717), the same thread tells the frames
//! it streams of the page and that the stream stopped, and where the page
//! is now when its own frame goes somewhere (queue item 742).
//!
//! And by the thread holding the frame's tab strip
//! ([`crate::frame_strip`]), what the strip's own renderer painted of that
//! name, or why it painted nothing: the page's tab is drawn there and
//! nowhere in this process (ADR 0024 § 4).

use crate::agent_lines::AgentLine;
use crate::chrome_follow::Follow;
use crate::chrome_protocol::Named;
use crate::chrome_screencast::Frame as CastFrame;
use alo_renderer::Frame;
use alo_url::Url;

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
    /// The newest frame Chrome streamed of the page, for the conversation's
    /// card (ADR 0076 B, queue item 717).
    Cast(CastFrame),
    /// The stream stopped, Chrome having answered that it did: what was
    /// read while it ran, and what Chrome had in flight, read after.
    Uncast(Uncast),
    /// The page streamed for the conversation's card is now at this
    /// address, by its own frame's commit or move within its document
    /// (queue item 742).
    Moved(Url),
    /// Whether the page streamed for the conversation's card is still
    /// loading: told as it changes, the page streamed from its navigation's
    /// commit (queue item 751).
    Loading(bool),
}

/// What a stream of frames came to, told as it stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uncast {
    /// Frames read while it ran, since the tab was opened.
    pub frames: u64,
    /// Their PNGs' bytes.
    pub bytes: u64,
    /// Frames read after Chrome answered that it had stopped, never shown.
    pub after: u64,
    /// Whether Chrome answered `Page.stopScreencast`.
    pub answered: bool,
}
