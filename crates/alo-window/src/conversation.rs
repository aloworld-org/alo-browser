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
//! One chat, new and on screen, to begin with, and nothing else: no kept
//! sites and no chat waiting, because nothing yet keeps either, and a first
//! screen that showed sites nobody added would be the browser inventing a
//! person's life.
//!
//! # What a press does
//!
//! Queue item 710. A person's press on either part is claimed by the
//! renderer that drew it and decided by the browser process
//! ([`crate::conversation_drawing`], `alo-renderer`'s
//! `conversation_decide`), and what was decided is [`Conversation::act`]ed
//! on here. Each chat has an identity of the browser process's own
//! ([`ChatId`]), which is what a decision names — never anything a renderer
//! said. Two decisions change what is held today:
//!
//! - **A chat** becomes the one on screen, and the panel is drawn again
//!   showing it so.
//! - **New chat** adds a chat with no name yet — the panel calls it *New
//!   chat* — first among the open ones, and on screen.
//!
//! Every other — *Settings*, a kept site, *Add a site*, a waiting chat, the
//! box, *Go* — is decided and kept as [`Conversation::last_pressed`], and
//! **changes nothing yet**: there is no settings screen (705), nothing
//! keeps a site or a waiting chat, and the box takes no typing (701). What
//! the first screen shows does not follow the chat on screen either, since
//! every chat today is new and shows the first screen (703).
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
use alo_renderer::conversation_decide::{Decided, Shown};
use alo_renderer::panel::{Chat, Panel, drawn_order};
use alo_renderer::start::{KeptSite, Start};

/// What the first screen says first, at any hour.
pub const GREETING: &str = "Hello.";

/// A chat, as the browser process knows it: a number it gave the chat
/// when it was made, never reused, and never sent to a renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChatId(u64);

/// What the parts showed, as the browser process names each thing in them.
pub type Showing = Shown<ChatId, KeptSite>;

/// What a press on either part came to, in the browser process's names.
pub type Pressed = Decided<ChatId, KeptSite>;

/// The conversation interface, as the browser process holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    /// Every chat, in the order the browser process keeps them, each with
    /// its identity.
    chats: Vec<(ChatId, Chat)>,
    /// The identity the next chat made is given.
    next: u64,
    /// What the first screen says first.
    greeting: String,
    /// What the last press on either part came to, if there was one.
    last_pressed: Option<Pressed>,
}

impl Default for Conversation {
    /// One new chat, on screen, and the greeting.
    fn default() -> Self {
        Self {
            chats: vec![(ChatId(0), Chat::new("", "").current())],
            next: 1,
            greeting: GREETING.to_owned(),
            last_pressed: None,
        }
    }
}

impl Conversation {
    /// The chats panel as it is now, laid out at `viewport` on a screen of
    /// `scale`.
    pub fn panel(&self, viewport: Size, scale: Scale) -> Panel {
        Panel {
            chats: self.chats.iter().map(|(_, chat)| chat.clone()).collect(),
            viewport,
            scale: 1.0,
        }
        .at_scale(scale.factor())
    }

    /// What the panel and the first screen show as they are now, each list
    /// in the order its part draws it: what a claim on either is decided
    /// against once they are painted.
    pub fn showing(&self) -> Showing {
        let chats: Vec<Chat> = self.chats.iter().map(|(_, chat)| chat.clone()).collect();
        Shown {
            chats: drawn_order(&chats)
                .into_iter()
                .filter_map(|place| self.chats.get(place).map(|(id, _)| *id))
                .collect(),
            sites: Vec::new(),
            waiting: Vec::new(),
        }
    }

    /// Act on what a press came to, and keep it as the last pressed:
    /// whether the panel or the first screen now shows something else, and
    /// so is to be drawn again.
    ///
    /// A chat becomes the one on screen; *New chat* adds one, on screen.
    /// Everything else changes nothing yet (see the module's words), and
    /// neither does a chat this does not hold, which the decision refused
    /// unless it went between being decided and being acted on.
    pub fn act(&mut self, pressed: Pressed) -> bool {
        let changed = match &pressed {
            Pressed::Chat(id) => self.put_on_screen(*id),
            Pressed::NewChat => {
                self.new_chat();
                true
            }
            _ => false,
        };
        self.last_pressed = Some(pressed);
        changed
    }

    /// What the last press on either part came to, if there was one.
    pub fn last_pressed(&self) -> Option<&Pressed> {
        self.last_pressed.as_ref()
    }

    /// The chat on screen, if one is.
    pub fn on_screen(&self) -> Option<ChatId> {
        self.chats
            .iter()
            .find(|(_, chat)| chat.current)
            .map(|(id, _)| *id)
    }

    /// Make the chat `id` the one on screen: whether that changed anything,
    /// which it does not when it already was or when no chat is `id`.
    fn put_on_screen(&mut self, id: ChatId) -> bool {
        if !self.chats.iter().any(|(each, _)| *each == id) || self.on_screen() == Some(id) {
            return false;
        }
        for (each, chat) in &mut self.chats {
            chat.current = *each == id;
        }
        true
    }

    /// Add a chat with no name yet, first, on screen.
    fn new_chat(&mut self) {
        for (_, chat) in &mut self.chats {
            chat.current = false;
        }
        let id = ChatId(self.next);
        self.next += 1;
        self.chats.insert(0, (id, Chat::new("", "").current()));
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
    use alo_renderer::conversation_decide::Refusal;
    use alo_renderer::message::Failure;
    use alo_renderer::panel::{Doing, NEW_CHAT};

    /// The labels of the panel's chats, in order, and which is on screen.
    fn rows(conversation: &Conversation) -> (Vec<String>, Vec<bool>) {
        let panel = conversation.panel(Size::new(240.0, 820.0), Scale::ONE);
        (
            panel
                .chats
                .iter()
                .map(|chat| chat.label().to_owned())
                .collect(),
            panel.chats.iter().map(|chat| chat.current).collect(),
        )
    }

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
    fn new_chat_adds_one_first_and_on_screen() {
        let mut conversation = Conversation::default();
        let first = conversation.on_screen();
        assert!(
            conversation.act(Pressed::NewChat),
            "the panel is drawn again"
        );
        assert_eq!(
            rows(&conversation),
            (
                vec![NEW_CHAT.to_owned(), NEW_CHAT.to_owned()],
                vec![true, false]
            )
        );
        let showing = conversation.showing();
        assert_eq!(showing.chats.len(), 2);
        assert_eq!(showing.chats.last().copied(), first);
        assert_ne!(conversation.on_screen(), first, "a chat of its own");
        assert_eq!(conversation.last_pressed(), Some(&Pressed::NewChat));
    }

    #[test]
    fn a_chat_pressed_becomes_the_one_on_screen() {
        let mut conversation = Conversation::default();
        let first = conversation.on_screen();
        let _ = conversation.act(Pressed::NewChat);
        let Some(first) = first else {
            panic!("a new conversation has a chat on screen");
        };
        assert!(conversation.act(Pressed::Chat(first)));
        assert_eq!(conversation.on_screen(), Some(first));
        assert_eq!(rows(&conversation).1, vec![false, true]);
        // Pressing it again changes nothing.
        assert!(!conversation.act(Pressed::Chat(first)));
        assert_eq!(conversation.last_pressed(), Some(&Pressed::Chat(first)));
    }

    #[test]
    fn a_chat_it_does_not_hold_changes_nothing() {
        let mut conversation = Conversation::default();
        let before = conversation.clone();
        assert!(!conversation.act(Pressed::Chat(ChatId(99))));
        assert_eq!(rows(&conversation), rows(&before));
        assert_eq!(conversation.on_screen(), before.on_screen());
    }

    #[test]
    fn every_other_press_is_kept_and_changes_nothing_yet() {
        for pressed in [
            Pressed::Settings,
            Pressed::Site(KeptSite::new("Mail", "https://mail.example/")),
            Pressed::AddASite,
            Pressed::Waiting(ChatId(0)),
            Pressed::TheBox,
            Pressed::Go,
            Pressed::Nothing,
            Pressed::Refused(Refusal::Unasked),
            Pressed::Failed(Failure::NotTheConversation),
        ] {
            let mut conversation = Conversation::default();
            let before = conversation.clone();
            assert!(!conversation.act(pressed.clone()), "{pressed:?}");
            assert_eq!(
                conversation.panel(Size::new(240.0, 820.0), Scale::ONE),
                before.panel(Size::new(240.0, 820.0), Scale::ONE),
                "{pressed:?}"
            );
            assert_eq!(
                conversation.start(Size::new(1040.0, 820.0), Scale::ONE),
                before.start(Size::new(1040.0, 820.0), Scale::ONE),
                "{pressed:?}"
            );
            assert_eq!(conversation.last_pressed(), Some(&pressed));
        }
    }

    #[test]
    fn what_is_showing_is_in_the_order_the_panel_draws() {
        let mut conversation = Conversation::default();
        let _ = conversation.act(Pressed::NewChat);
        let _ = conversation.act(Pressed::NewChat);
        let ids: Vec<ChatId> = conversation.chats.iter().map(|(id, _)| *id).collect();
        assert_eq!(conversation.showing().chats, ids);
        assert_eq!(ids, vec![ChatId(2), ChatId(1), ChatId(0)]);
        assert!(conversation.showing().sites.is_empty());
        assert!(conversation.showing().waiting.is_empty());
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
