/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Everything the frame of ours around Chrome's window is told, by the
//! thread holding Chrome (ADR 0062, queue items 642 and 679).
//!
//! Two things, and only two: where to stand, as Chrome's window moves, hides
//! and closes ([`Follow`], [`crate::chrome_follow`]); and what an agent's
//! verb was answered with, for the person watching ([`AgentLine`], queue
//! items 623 and 625 through Chrome) — the line [`AgentLine::of`] makes of
//! the answer this process sent, so nothing a page or Chrome says can become
//! one. The frame keeps the lines as this engine's window keeps them
//! ([`crate::agent_lines::AgentLines`]).

use crate::agent_lines::AgentLine;
use crate::chrome_follow::Follow;

/// What the frame around Chrome's window is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToFrame {
    /// Where to stand, or that Chrome's window is gone.
    Stand(Follow),
    /// What an agent's verb was answered with: a sentence of a verb that
    /// ran, or a refusal.
    Agent(AgentLine),
}
