/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Quit and update* (ADR 0116 § 4, queue item 990): the button at the end
//! of the first screen's line saying an update is ready, and what a press
//! on it does.
//!
//! # Drawn and decided by the browser
//!
//! The line is the browser's own ([`crate::notice`]), drawn over the first
//! screen's frame, never inside a page; so is the button on it
//! ([`crate::line_button`]). A press on it is decided here from the
//! window's own composition ([`crate::conversation_aim::on_the_button`]),
//! before any renderer is handed one: no renderer draws it, claims it or
//! is asked about it.
//!
//! # Pressed
//!
//! [`alo_update::quitting::quit_and_update`] decides: while an agent is
//! working in any chat — a run at work or held for the person — alo stays
//! and the line says *An agent is working. alo will update when you
//! quit.*, as a refused press is said, until the next press; otherwise the
//! window asks its event loop to quit ([`ConversationWindow::take_quit`]),
//! which ends as a person's quit or closing the window ends — every card's
//! host ended, then what is ready swapped in, then alo exits. alo is never
//! started again by it.
//!
//! # Not an agent's
//!
//! Only the window server's main button reaches it. The agent socket
//! reaches the page shown in a card, its verbs take no position, and no
//! ask names the window's lines or updates (`docs/contracts/agent-socket.md`,
//! [`crate::protocol`]); nothing here is added to it.

use super::ConversationWindow;
use crate::conversation_place::ConversationPlacement;
use crate::line_button::LineButton;
use alo_update::quitting::{Pressed, quit_and_update};

impl ConversationWindow {
    /// The line saying an update is ready and the button at its end, when an
    /// update is ready.
    pub(super) fn ready_line(&self) -> (Option<&str>, Option<LineButton>) {
        match self.updating.ready.as_deref() {
            Some(line) => (Some(line), Some(LineButton::QuitAndUpdate)),
            None => (None, None),
        }
    }

    /// Whether the point `at`, in device pixels, is on *Quit and update*
    /// as the window is drawn now.
    pub(super) fn on_quit_and_update(&self, at: (f64, f64)) -> bool {
        self.scene().is_some_and(|scene| {
            let placed = ConversationPlacement::of(scene.window, scene.scale, scene.parts());
            crate::conversation_aim::on_the_button(&placed, at)
        })
    }

    /// Whether an agent is working in any chat: a run of alo's agent at
    /// work, or held while the page is the person's. One runs at a time,
    /// in the chat on screen: another chat chosen stops it.
    pub(super) fn agent_working(&self) -> bool {
        self.run.is_some() || self.conversation.agent_run().is_some()
    }

    /// *Quit and update* pressed: alo waits, saying why, while an agent
    /// works, and otherwise quits.
    pub(super) fn quit_and_update(&mut self) {
        match quit_and_update(self.agent_working()) {
            Pressed::Waits(why) => self.tell(why.to_owned()),
            Pressed::Quits => {
                self.said.push("quitting to update".to_owned());
                self.updating.quits = true;
            }
        }
        self.redraw = true;
    }

    /// Whether the window is to quit, as a person's quit does, since this
    /// was last asked: *Quit and update* pressed with no agent working.
    pub fn take_quit(&mut self) -> bool {
        std::mem::take(&mut self.updating.quits)
    }
}
