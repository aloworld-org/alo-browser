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

use alo_renderer::interface_face::InterfaceFace;

/// *Settings*: open or not, and the face chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Settings {
    /// Whether it is the page on screen.
    open: bool,
    /// The face the interface's words are set in.
    face: InterfaceFace,
    /// A face a person just chose, not yet kept.
    chosen: Option<InterfaceFace>,
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

    /// Open it: whether it was not open already.
    pub fn open(&mut self) -> bool {
        !core::mem::replace(&mut self.open, true)
    }

    /// Close it, back to the chat on screen: whether it was open.
    pub fn close(&mut self) -> bool {
        core::mem::replace(&mut self.open, false)
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
