/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the browser process holds of its own interface as a conversation
//! (ADR 0074, queue item 709), and what it tells the renderers that draw it.
//!
//! The interface is the chats panel on the left and, beside it, what the
//! chat on screen shows — for a new chat, the first screen. Each is a
//! document of ours drawn by a renderer of its own from state the browser
//! process sends as data (`alo-renderer`'s `panel` and `start`), never from
//! markup. This is that state, and the one place it is decided.
//!
//! # What it holds today
//!
//! One chat, new and on screen, and nothing else: no kept sites and no chat
//! waiting, because nothing yet keeps either, and a first screen that showed
//! sites nobody added would be the browser inventing a person's life. A
//! press on the panel or the first screen, which would change what is held
//! here, is queue item 710.
//!
//! # The greeting
//!
//! The design greets a person by the time of day — *Good morning.* — and the
//! browser process is the one that would know it. It does not yet: this
//! engine reads no time zone (ADR 0036 § 3 keeps a page's local time UTC),
//! and a greeting of the morning said in the evening is a wrong sentence on
//! the first screen a person sees. So it says [`GREETING`], which is true at
//! any hour, until the machine's zone is read.

use crate::scale::Scale;
use alo_layout::Size;
use alo_renderer::panel::{Chat, Panel};
use alo_renderer::start::Start;

/// What the first screen says first, at any hour.
pub const GREETING: &str = "Hello.";

/// The conversation interface, as the browser process holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    /// Every chat, in the order the panel shows them.
    chats: Vec<Chat>,
    /// What the first screen says first.
    greeting: String,
}

impl Default for Conversation {
    /// One new chat, on screen, and the greeting.
    fn default() -> Self {
        Self {
            chats: vec![Chat::new("", "").current()],
            greeting: GREETING.to_owned(),
        }
    }
}

impl Conversation {
    /// The chats panel as it is now, laid out at `viewport` on a screen of
    /// `scale`.
    pub fn panel(&self, viewport: Size, scale: Scale) -> Panel {
        Panel {
            chats: self.chats.clone(),
            viewport,
            scale: 1.0,
        }
        .at_scale(scale.factor())
    }

    /// The first screen as it is now, laid out at `viewport` on a screen of
    /// `scale`.
    pub fn start(&self, viewport: Size, scale: Scale) -> Start {
        Start {
            greeting: self.greeting.clone(),
            sites: Vec::new(),
            waiting: Vec::new(),
            viewport,
            scale: 1.0,
        }
        .at_scale(scale.factor())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_renderer::panel::{Doing, NEW_CHAT};

    #[test]
    fn a_new_conversation_is_one_new_chat_on_screen() {
        let panel = Conversation::default().panel(Size::new(240.0, 820.0), Scale::ONE);
        let [chat] = panel.chats.as_slice() else {
            panic!("one chat: {panel:?}");
        };
        assert_eq!(chat.label(), NEW_CHAT);
        assert!(chat.open && chat.current);
        assert_eq!(chat.doing, Doing::Nothing);
        assert_eq!(chat.mark(), None, "no site is open in it");
        assert_eq!(panel.viewport, Size::new(240.0, 820.0));
    }

    #[test]
    fn the_first_screen_greets_and_shows_nothing_nobody_kept() {
        let start = Conversation::default().start(Size::new(1040.0, 820.0), Scale::ONE);
        assert_eq!(start.greeting, GREETING);
        assert!(start.sites.is_empty());
        assert!(start.waiting.is_empty());
        assert_eq!(start.viewport, Size::new(1040.0, 820.0));
    }

    #[test]
    fn both_are_drawn_at_the_windows_scale() {
        let conversation = Conversation::default();
        for scale in [Scale::ONE, Scale::ONE_AND_A_HALF, Scale::TWO] {
            let panel = conversation.panel(Size::new(240.0, 410.0), scale);
            let start = conversation.start(Size::new(520.0, 410.0), scale);
            assert_eq!(panel.scale.to_bits(), scale.factor().to_bits());
            assert_eq!(start.scale.to_bits(), scale.factor().to_bits());
        }
    }
}
