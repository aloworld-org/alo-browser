/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Settings*, as the browser process holds it (queue item 854,
//! `docs/design/shell.md`, *Settings*; approved by the owner 2026-11-07):
//! whether it is the page on screen, and the face the interface's words
//! are set in.
//!
//! *Settings* opens as a page in the conversation's place — the panel's
//! *Settings* row pressed — and stands there until *Back to the chat* or
//! Escape is pressed, a chat is chosen in the panel or *New chat* is: the
//! chat on screen is left as it was underneath, its turns, its scroll and
//! a live card kept. Its one section is the font: choosing a face sets the
//! whole interface in it at once, with no restart, and is kept beside the
//! chats ([`crate::settings_file`]) so the next launch is set in it too.
//!
//! **By keys** (queue item 856): Tab and ⇧Tab move the keyboard between
//! *Back to the chat* and the radio group *Font*, round from either end;
//! on the group the arrows choose the face before or after the one chosen,
//! round from either end, as the ARIA radio group pattern and a Mac's radio
//! buttons have it; Space or Return on *Back to the chat* presses it. Where
//! the keyboard is is drawn as a ring only once Tab has put it there, and a
//! press of the pointer puts it away, as a browser's `:focus-visible` does
//! ([`crate::settings_keys`] reads the keys).

use alo_renderer::interface_face::InterfaceFace;
use alo_renderer::start::SettingsKeyed;

/// *Settings*: open or not, and the face chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
}

impl Settings {
    /// Set in `face`, as read back from where it was kept, and closed.
    pub fn set_in(face: InterfaceFace) -> Self {
        Self {
            face,
            ..Self::default()
        }
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
    /// chat* and *Font*, or the one before, round from either end — from
    /// nothing, the first or the last. Whether it is open, and so moved.
    pub fn tab(&mut self, back: bool) -> bool {
        if !self.open {
            return false;
        }
        self.keyed = Some(match (self.keyed, back) {
            (Some(SettingsKeyed::Back), false) | (None | Some(SettingsKeyed::Back), true) => {
                SettingsKeyed::Font
            }
            _ => SettingsKeyed::Back,
        });
        true
    }

    /// An arrow on *Font*: the face after the one chosen when `on`, or the
    /// one before, round from either end, chosen — and kept, as a press on
    /// it is. Whether the keyboard is on *Font*, and so another was chosen.
    pub fn step(&mut self, on: bool) -> bool {
        if self.keyed() != Some(SettingsKeyed::Font) {
            return false;
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
        // Queue item 856.
        let mut settings = Settings::default();
        assert!(!settings.tab(false), "closed, nothing to move between");
        assert_eq!(settings.keyed(), None);
        assert!(settings.open());
        assert_eq!(settings.keyed(), None, "nothing until Tab");
        assert!(settings.tab(false));
        assert_eq!(settings.keyed(), Some(SettingsKeyed::Back));
        assert!(settings.tab(false));
        assert_eq!(settings.keyed(), Some(SettingsKeyed::Font));
        assert!(settings.tab(false));
        assert_eq!(settings.keyed(), Some(SettingsKeyed::Back), "round");
        assert!(settings.tab(true));
        assert_eq!(settings.keyed(), Some(SettingsKeyed::Font), "⇧Tab round");
        assert!(settings.tab(true));
        assert_eq!(settings.keyed(), Some(SettingsKeyed::Back));
        // ⇧Tab from nothing is the last.
        assert!(settings.unkey());
        assert!(!settings.unkey(), "put away already");
        assert!(settings.tab(true));
        assert_eq!(settings.keyed(), Some(SettingsKeyed::Font));
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
        assert!(settings.tab(true));
        assert!(settings.step(true));
        assert_eq!(settings.face(), InterfaceFace::Geist);
        assert_eq!(settings.take_chosen(), Some(InterfaceFace::Geist), "kept");
        assert!(settings.step(false));
        assert!(settings.step(false));
        assert_eq!(settings.face(), InterfaceFace::InstrumentSans, "round");
        assert!(settings.step(true));
        assert_eq!(settings.face(), InterfaceFace::Inter, "round again");
        // On Back, the arrows are nothing.
        assert!(settings.tab(false));
        assert!(!settings.step(true));
        assert_eq!(settings.face(), InterfaceFace::Inter);
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
