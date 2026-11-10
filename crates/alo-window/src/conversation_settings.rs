/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Settings*, as the browser process holds it (queue item 854,
//! `docs/design/shell.md`, *Settings*; approved by the owner 2026-11-07):
//! whether it is open, and the face the interface's words are set in.
//!
//! *Settings* opens over the conversation (queue item 886) — the panel's
//! *Settings* row pressed, or ⌘, — and stands there until its × or Escape
//! is pressed, the veil around it is, a chat is chosen in the panel or
//! *New chat* is: the chat on screen is drawn under it as it was, its
//! turns, its scroll and a live card kept, and the keyboard is given back
//! to where it was ([`Keyboard`]). Its first section is the font: choosing a face sets the
//! whole interface in it at once, with no restart, and is kept beside the
//! chats ([`crate::settings_file`]) so the next launch is set in it too.
//!
//! **By keys** (queue item 856): Tab and ⇧Tab move the keyboard between
//! its × and the radio group *Font*, round from either end; on the group
//! the arrows choose the face before or after the one chosen, round from
//! either end, as the ARIA radio group pattern and a Mac's radio buttons
//! have it; Space or Return on its × presses it. Where
//! the keyboard is is drawn as a ring only once Tab has put it there, and a
//! press of the pointer puts it away, as a browser's `:focus-visible` does
//! ([`crate::settings_keys`] reads the keys).
//!
//! Its second section, **alo at work** (queue item 878), holds how far
//! alo's agent may go on one task ([`Limits`]): a press of *Fewer* or
//! *More* moves a number, stopping at its ends, and a choice of asking is
//! made by a press on it; by keys, Tab goes on from *Font* to the sites,
//! the actions and the radio group *Ask before opening*, and back round;
//! on a number ← ↓ make it fewer and → ↑ more, as a stepper's arrows do,
//! and on the group → ↑ choose the next and ← ↓ the one before, round. Kept beside the face, and
//! read by the window as each run starts.
//!
//! Its fourth, **Privacy** (ADR 0108, queue item 920), holds what sites'
//! cookies are kept and whether alo answers their cookie banners
//! ([`Privacy`]): a choice made by a press on it; by keys, Tab goes on
//! after *Search and answers* to the radio groups *Cookies* and *Cookie
//! banners*, where → ↑ choose the next and ← ↓ the one before, round. Kept
//! beside the face, and told to the whole process as it is made
//! ([`crate::privacy_chosen`]).
//!
//! **Scrolled** (queue item 880) as the conversation is ([`Scrolled`]): in
//! a window too short for the whole page, by the wheel, the paging keys
//! and the bar, from its top each time it opens; and as Tab moves the
//! keyboard, what it moved to is scrolled into view, [`REVEALED`] clear of
//! the room's edges, once its renderer says where it stands.

use crate::conversation_drawing::SettingsDrawn;
use crate::scrolled::Scrolled;
use alo_renderer::agent_limits::{Asks, Limit, Limits, Stepped};
use alo_renderer::conversation_scroll::Reach;
use alo_renderer::interface_face::InterfaceFace;
use alo_renderer::settings_privacy::{Privacy, PrivacyPress};
use alo_renderer::start::SettingsKeyed;

/// *Settings*: open or not, and the face chosen.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Settings {
    /// Whether it is the page on screen.
    open: bool,
    /// The face the interface's words are set in.
    face: InterfaceFace,
    /// A face a person just chose, not yet kept.
    chosen: Option<InterfaceFace>,
    /// What of it the keyboard is on, once Tab put it there (queue item
    /// 856).
    keyed: Option<SettingsKeyed>,
    /// How far alo's agent may go (queue item 878).
    limits: Limits,
    /// Whether a person changed the limits since this was last asked, so
    /// that they are to be kept.
    limits_changed: bool,
    /// How far it is scrolled (queue item 880).
    scrolled: Scrolled,
    /// What Tab moved the keyboard to, to be scrolled into view once its
    /// renderer says where it was drawn.
    reveal: Option<SettingsKeyed>,
    /// *Privacy*'s choices (ADR 0108, queue item 920).
    privacy: Privacy,
    /// Whether a person changed them since this was last asked, so that
    /// they are to be kept.
    privacy_changed: bool,
}

/// Where the keyboard was as *Settings* opened: what it is given back to as
/// *Settings* closes by Escape, its × or a press on the veil around it
/// (queue item 886). A chat chosen, or *New chat*, puts it where they put
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Keyboard {
    /// Nothing had it.
    #[default]
    Nowhere,
    /// The one box.
    TheBox,
    /// The panel's *Search chats*.
    SearchChats,
    /// The live card's page.
    TheCard,
}

/// How far clear of the room's top or foot what the keyboard moves to is
/// scrolled, in CSS pixels: past its ring of 3, with the room's own gap.
pub const REVEALED: f32 = 16.0;

impl Settings {
    /// Set in `face`, as read back from where it was kept, and closed.
    pub fn set_in(face: InterfaceFace) -> Self {
        Self {
            face,
            ..Self::default()
        }
    }

    /// The same, the agent kept to `limits`, as read back (queue item 878).
    #[must_use]
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// How far alo's agent may go, as set now.
    pub fn limits(self) -> Limits {
        self.limits
    }

    /// The same, *Privacy*'s choices `privacy`, as read back (queue item
    /// 920).
    #[must_use]
    pub fn with_privacy(mut self, privacy: Privacy) -> Self {
        self.privacy = privacy;
        self
    }

    /// *Privacy*'s choices, as made now.
    pub fn privacy(self) -> Privacy {
        self.privacy
    }

    /// A choice of *Privacy* made: whether it is another, and so is to be
    /// kept.
    pub fn choose_privacy(&mut self, pressed: PrivacyPress) -> bool {
        let made = self.privacy.choosing(pressed);
        if made == self.privacy {
            return false;
        }
        self.privacy = made;
        self.privacy_changed = true;
        true
    }

    /// Whether a person changed *Privacy*'s choices since this was last
    /// asked, once.
    pub fn take_privacy_changed(&mut self) -> bool {
        core::mem::replace(&mut self.privacy_changed, false)
    }

    /// A press of *Fewer* or *More* (queue item 878): its number moved,
    /// stopping at its ends. Whether it moved, and so is to be kept.
    pub fn press(&mut self, stepped: Stepped) -> bool {
        let moved = self.limits.step(stepped);
        self.set_limits(moved)
    }

    /// A choice of asking made: whether it is another, and so is to be
    /// kept.
    pub fn choose_asks(&mut self, asks: Asks) -> bool {
        self.set_limits(Limits {
            asks,
            ..self.limits
        })
    }

    /// The limits now `limits`: whether they changed.
    fn set_limits(&mut self, limits: Limits) -> bool {
        if limits == self.limits {
            return false;
        }
        self.limits = limits;
        self.limits_changed = true;
        true
    }

    /// Whether a person changed the limits since this was last asked, once.
    pub fn take_limits_changed(&mut self) -> bool {
        core::mem::replace(&mut self.limits_changed, false)
    }

    /// Whether *Settings* is the page on screen.
    pub fn is_open(self) -> bool {
        self.open
    }

    /// The face the interface's words are set in.
    pub fn face(self) -> InterfaceFace {
        self.face
    }

    /// Open it, the keyboard on nothing of it yet: whether it was not open
    /// already.
    pub fn open(&mut self) -> bool {
        let opened = !core::mem::replace(&mut self.open, true);
        if opened {
            self.keyed = None;
            // From its top, its reach measured again (queue item 880).
            self.scrolled.reset();
            self.reveal = None;
        }
        opened
    }

    /// Close it, back to the chat on screen: whether it was open.
    pub fn close(&mut self) -> bool {
        self.keyed = None;
        core::mem::replace(&mut self.open, false)
    }

    /// What of it the keyboard is on, while it is open (queue item 856).
    pub fn keyed(self) -> Option<SettingsKeyed> {
        self.keyed.filter(|_| self.open)
    }

    /// Tab, or ⇧Tab when `back`: the keyboard on the next of *Back to the
    /// chat*, *Font* and *alo at work*'s three (queue item 878), or the one
    /// before, round from either end — from nothing, the first or the last.
    /// Whether it is open, and so moved.
    pub fn tab(&mut self, back: bool) -> bool {
        self.tab_through(back, &[])
    }

    /// Tab, or ⇧Tab when `back`, as [`Settings::tab`], through `more` after
    /// *alo at work*'s three: *Search and answers*' stops, as it is drawn
    /// (queue item 905). Whether it is open, and so moved.
    pub fn tab_through(&mut self, back: bool, more: &[SettingsKeyed]) -> bool {
        if !self.open {
            return false;
        }
        let all: Vec<SettingsKeyed> = SettingsKeyed::ALL.iter().chain(more).copied().collect();
        let at = self
            .keyed
            .and_then(|keyed| all.iter().position(|each| *each == keyed));
        let next = match (at, back) {
            (None, false) => 0,
            (None, true) => all.len() - 1,
            (Some(at), false) => (at + 1) % all.len(),
            (Some(at), true) => (at + all.len() - 1) % all.len(),
        };
        self.keyed = all.get(next).copied();
        self.reveal = self.keyed;
        true
    }

    /// The keyboard on `keyed`, as an arrow on a chooser of *Search and
    /// answers* moves it to the choice made (queue item 905), scrolled into
    /// view: while it is open.
    pub fn key_on(&mut self, keyed: SettingsKeyed) {
        if self.open {
            self.keyed = Some(keyed);
            self.reveal = Some(keyed);
        }
    }

    /// How far it is scrolled (queue item 880).
    pub fn scrolled(self) -> Scrolled {
        self.scrolled
    }

    /// How far it is scrolled, to be moved — while it is open.
    pub fn scrolled_mut(&mut self) -> Option<&mut Scrolled> {
        self.open.then_some(&mut self.scrolled)
    }

    /// Its renderer measured it, drawn as `drawn`, to reach as `reach`
    /// says in a room `room` tall: how far it is scrolled kept inside that,
    /// and what Tab moved the keyboard to scrolled into view once it is
    /// drawn there. Whether that moved it.
    pub fn reached(&mut self, reach: Reach, drawn: SettingsDrawn, room: f32) -> bool {
        if !self.open {
            return false;
        }
        let mut moved = self.scrolled.reached(reach, room);
        if let Some((keyed, span)) = drawn.keyed
            && self.reveal == Some(keyed)
        {
            self.reveal = None;
            moved |= self.scrolled.into_view(span, REVEALED, room);
        }
        moved
    }

    /// An arrow on *Font*: the face after the one chosen when `on`, or the
    /// one before, round from either end, chosen — and kept, as a press on
    /// it is. Whether the keyboard is on *Font*, and so another was chosen.
    pub fn step(&mut self, on: bool) -> bool {
        match self.keyed() {
            Some(SettingsKeyed::Font) => {}
            // A number: more or fewer, as a stepper's arrows (queue item
            // 878).
            Some(SettingsKeyed::Sites) => {
                return self.press(Stepped {
                    limit: Limit::Sites,
                    more: on,
                });
            }
            Some(SettingsKeyed::Actions) => {
                return self.press(Stepped {
                    limit: Limit::Actions,
                    more: on,
                });
            }
            // Three choices: → ↑ the next, ← ↓ the one before, round (884).
            Some(SettingsKeyed::Asks) => {
                let other = self.limits.asks.stepped(on);
                return self.choose_asks(other);
            }
            // *Privacy*'s two groups (queue item 920): → ↑ the next, ← ↓
            // the one before, round.
            Some(SettingsKeyed::Cookies) => {
                let other = self.privacy.cookies.stepped(on);
                return self.choose_privacy(PrivacyPress::Cookies(other));
            }
            Some(SettingsKeyed::Banners) => {
                let other = self.privacy.banners.stepped(on);
                return self.choose_privacy(PrivacyPress::Banners(other));
            }
            Some(SettingsKeyed::Close | SettingsKeyed::Service(_)) | None => return false,
        }
        let faces = InterfaceFace::ALL;
        let at = faces
            .iter()
            .position(|face| *face == self.face)
            .unwrap_or_default();
        let next = if on {
            (at + 1) % faces.len()
        } else {
            (at + faces.len() - 1) % faces.len()
        };
        faces.get(next).is_some_and(|face| self.choose(*face))
    }

    /// The pointer pressed: the keyboard's ring put away until Tab is
    /// pressed again. Whether one was drawn.
    pub fn unkey(&mut self) -> bool {
        self.keyed.take().is_some() && self.open
    }

    /// A person chose `face`: whether it is another than the one the
    /// interface was set in. A face chosen is to be kept, once.
    pub fn choose(&mut self, face: InterfaceFace) -> bool {
        if face == self.face {
            return false;
        }
        self.face = face;
        self.chosen = Some(face);
        true
    }

    /// The face a person chose since this was last asked, to be kept.
    pub fn take_chosen(&mut self) -> Option<InterfaceFace> {
        self.chosen.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_opens_closed_in_inter_and_closes_once() {
        let mut settings = Settings::default();
        assert!(!settings.is_open());
        assert_eq!(settings.face(), InterfaceFace::Inter);
        assert!(settings.open());
        assert!(!settings.open(), "open already");
        assert!(settings.is_open());
        assert!(settings.close());
        assert!(!settings.close(), "closed already");
    }

    #[test]
    fn tab_moves_the_keyboard_between_back_and_font_round_from_either_end() {
        // Queue item 856; since 878, through alo at work's three too.
        let mut settings = Settings::default();
        assert!(!settings.tab(false), "closed, nothing to move between");
        assert_eq!(settings.keyed(), None);
        assert!(settings.open());
        assert_eq!(settings.keyed(), None, "nothing until Tab");
        assert!(settings.tab(false));
        assert_eq!(settings.keyed(), Some(SettingsKeyed::Close));
        assert!(settings.tab(false));
        assert_eq!(settings.keyed(), Some(SettingsKeyed::Font));
        assert!(settings.tab(true));
        assert_eq!(settings.keyed(), Some(SettingsKeyed::Close));
        assert!(settings.tab(true));
        assert_eq!(settings.keyed(), Some(SettingsKeyed::Asks), "⇧Tab round");
        // ⇧Tab from nothing is the last.
        assert!(settings.unkey());
        assert!(!settings.unkey(), "put away already");
        assert!(settings.tab(true));
        assert_eq!(settings.keyed(), Some(SettingsKeyed::Asks));
        // Closed and opened again, nothing.
        assert!(settings.close());
        assert_eq!(settings.keyed(), None);
        assert!(settings.open());
        assert_eq!(settings.keyed(), None);
    }

    #[test]
    fn the_arrows_on_font_choose_the_next_face_round_and_are_kept() {
        let mut settings = Settings::default();
        assert!(settings.open());
        assert!(!settings.step(true), "the keyboard not on Font");
        assert_eq!(settings.face(), InterfaceFace::Inter);
        assert!(settings.tab(false));
        assert!(settings.tab(false));
        assert!(settings.step(true));
        assert_eq!(settings.face(), InterfaceFace::Geist);
        assert_eq!(settings.take_chosen(), Some(InterfaceFace::Geist), "kept");
        assert!(settings.step(false));
        assert!(settings.step(false));
        assert_eq!(settings.face(), InterfaceFace::InstrumentSans, "round");
        assert!(settings.step(true));
        assert_eq!(settings.face(), InterfaceFace::Inter, "round again");
        // On Back, the arrows are nothing.
        assert!(settings.tab(true));
        assert!(!settings.step(true));
        assert_eq!(settings.face(), InterfaceFace::Inter);
    }

    #[test]
    fn tab_goes_on_through_alo_at_work_and_the_arrows_move_its_numbers() {
        // Queue item 878.
        let mut settings = Settings::default();
        assert!(settings.open());
        let mut went = Vec::new();
        for _ in 0..6 {
            assert!(settings.tab(false));
            went.extend(settings.keyed());
        }
        assert_eq!(
            went,
            [
                SettingsKeyed::Close,
                SettingsKeyed::Font,
                SettingsKeyed::Sites,
                SettingsKeyed::Actions,
                SettingsKeyed::Asks,
                SettingsKeyed::Close,
            ]
        );
        assert!(settings.tab(true));
        assert_eq!(settings.keyed(), Some(SettingsKeyed::Asks), "⇧Tab round");
        // Three choices (queue item 884), from *Never*: on, on, round.
        assert!(settings.step(true));
        assert_eq!(settings.limits().asks, Asks::Unnamed);
        assert!(settings.take_limits_changed());
        assert!(!settings.take_limits_changed(), "kept once");
        assert!(settings.step(true));
        assert_eq!(settings.limits().asks, Asks::Every);
        assert!(settings.step(true));
        assert_eq!(settings.limits().asks, Asks::Never, "round");
        assert!(settings.step(false));
        assert_eq!(settings.limits().asks, Asks::Every, "back round");
        assert!(settings.step(false));
        assert_eq!(settings.limits().asks, Asks::Unnamed);
        assert!(settings.tab(true));
        assert_eq!(settings.keyed(), Some(SettingsKeyed::Actions));
        assert!(settings.step(true));
        assert_eq!(settings.limits().actions, 25);
        assert!(settings.tab(true));
        for _ in 0..12 {
            let _ = settings.step(true);
        }
        assert_eq!(settings.limits().sites, 10, "stops at its most");
        assert!(!settings.step(true), "past it, nothing");
        assert!(settings.step(false));
        assert_eq!(settings.limits().sites, 9);
        // A face is not a limit, nor a limit a face.
        assert_eq!(settings.face(), InterfaceFace::Inter);
        assert_eq!(settings.take_chosen(), None);
    }

    #[test]
    fn a_press_moves_a_number_or_makes_a_choice_and_read_back_is_not_changed() {
        let mut settings = Settings::set_in(InterfaceFace::Geist)
            .with_limits(Limits::checked(1, 5, Asks::Every).unwrap_or_default());
        assert!(!settings.take_limits_changed(), "read back, not changed");
        assert!(!settings.press(Stepped {
            limit: Limit::Sites,
            more: false
        }));
        assert!(settings.press(Stepped {
            limit: Limit::Actions,
            more: true
        }));
        assert_eq!(settings.limits().actions, 10);
        assert!(settings.choose_asks(Asks::Unnamed));
        assert!(!settings.choose_asks(Asks::Unnamed));
        assert!(settings.take_limits_changed());
    }

    #[test]
    fn a_face_chosen_sets_it_and_is_kept_once_and_the_same_one_is_nothing() {
        let mut settings = Settings::set_in(InterfaceFace::Geist);
        assert_eq!(settings.face(), InterfaceFace::Geist);
        assert_eq!(settings.take_chosen(), None, "read back, not chosen");
        assert!(!settings.choose(InterfaceFace::Geist));
        assert_eq!(settings.take_chosen(), None);
        assert!(settings.choose(InterfaceFace::DmSans));
        assert_eq!(settings.face(), InterfaceFace::DmSans);
        assert_eq!(settings.take_chosen(), Some(InterfaceFace::DmSans));
        assert_eq!(settings.take_chosen(), None, "kept once");
    }
}
