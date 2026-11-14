/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Settings*' sixth section, **Updates**, as data (ADR 0116 § 4, queue
//! item 721): alo's version and its Chromium's, when alo last checked and
//! what it found, the switch *Check for updates automatically* and the
//! button *Check now* — or, for a copy that does not update itself, the
//! sentence that says so. Drawn by [`crate::settings_updates_document`],
//! decided by the browser process.
//!
//! # On by default, and said
//!
//! The owner's answer of 14 November 2026: the check is on by default
//! ([`Automatic::On`]), *Settings* turns it off, and the first screen says
//! once that it is on. Off means off: alo sends nothing until *Check now*
//! is pressed, and that press is the person's cause.
//!
//! # A stranger's words, bounded
//!
//! The lines are the browser process's own sentences — but what a check
//! found can carry a version a manifest named, so each is cut to
//! [`LONGEST`] characters on the wire, and drawn as text.
//!
//! # What a press is on
//!
//! Its two choices, then *Check now*, in the order they are drawn
//! ([`UpdatesShown::pressables`]); the browser process decides a press by
//! its place among them, against the same list it painted.

/// The most characters of one line of the section.
pub const LONGEST: usize = 400;

/// What *Check now* says, and is named.
pub const CHECK_NOW: &str = "Check now";

/// Whether alo checks for updates by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Automatic {
    /// At launch and once a day while it runs: the default.
    #[default]
    On,
    /// Never, until *Check now* is pressed.
    Off,
}

impl Automatic {
    /// Each, in the order they are drawn.
    pub const ALL: [Automatic; 2] = [Automatic::On, Automatic::Off];

    /// What its choice says, and is named.
    pub fn said(self) -> &'static str {
        match self {
            Self::On => "On",
            Self::Off => "Off",
        }
    }

    /// The quiet line under its choice, if it has one: what it costs.
    pub fn note(self) -> Option<&'static str> {
        match self {
            Self::On => Some("Once a day. It sends nothing about you."),
            Self::Off => Some(
                "alo sends nothing until you press Check now, and may miss Chromium\u{2019}s \
                 security fixes.",
            ),
        }
    }

    /// Its word in the settings file.
    pub fn word(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
        }
    }

    /// The choice `word` names in the settings file, if any.
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|each| each.word() == word)
    }

    /// The other, either way: two choices step round to each other.
    #[must_use]
    pub fn stepped(self) -> Self {
        match self {
            Self::On => Self::Off,
            Self::Off => Self::On,
        }
    }
}

/// Something of the section a person presses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpdatesPress {
    /// This choice of the switch.
    Automatic(Automatic),
    /// *Check now*: not a choice, a deed the browser process does.
    CheckNow,
}

/// The section, as the browser process sends it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdatesShown {
    /// alo's version and its Chromium's, in a line.
    pub version: String,
    /// Whether this copy updates itself, and what follows from it.
    pub state: UpdatesState,
}

/// Whether this copy updates itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdatesState {
    /// It does: the switch as chosen, and when it last checked and what it
    /// found, in a line.
    Itself {
        /// The switch.
        automatic: Automatic,
        /// When it last checked, and what it found.
        checked: String,
    },
    /// It does not, and this says why: *This copy of alo does not update
    /// itself.*, or *Updates come from your software source.*
    Not(String),
}

impl UpdatesShown {
    /// Everything a person may press, in the order it is drawn: nothing in
    /// a copy that does not update itself.
    pub fn pressables(&self) -> Vec<UpdatesPress> {
        match self.state {
            UpdatesState::Itself { .. } => Automatic::ALL
                .into_iter()
                .map(UpdatesPress::Automatic)
                .chain([UpdatesPress::CheckNow])
                .collect(),
            UpdatesState::Not(_) => Vec::new(),
        }
    }
}

/// `said` cut to [`LONGEST`] characters.
pub fn bounded(said: &str) -> String {
    said.chars().take(LONGEST).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_is_the_default_and_each_word_reads_back() {
        assert_eq!(Automatic::default(), Automatic::On);
        for each in Automatic::ALL {
            assert_eq!(Automatic::of_word(each.word()), Some(each));
            assert_eq!(each.stepped().stepped(), each);
        }
        assert_eq!(Automatic::of_word("Off"), None);
        assert_eq!(Automatic::On.stepped(), Automatic::Off);
    }

    #[test]
    fn a_copy_that_does_not_update_itself_offers_nothing_to_press() {
        let itself = UpdatesShown {
            version: "alo 0.4.1, with Chromium 156.0.8078.96".into(),
            state: UpdatesState::Itself {
                automatic: Automatic::On,
                checked: "Not checked yet.".into(),
            },
        };
        assert_eq!(
            itself.pressables(),
            vec![
                UpdatesPress::Automatic(Automatic::On),
                UpdatesPress::Automatic(Automatic::Off),
                UpdatesPress::CheckNow
            ]
        );
        let not = UpdatesShown {
            state: UpdatesState::Not("This copy of alo does not update itself.".into()),
            ..itself
        };
        assert!(not.pressables().is_empty());
        assert_eq!(bounded(&"x".repeat(500)).chars().count(), LONGEST);
    }
}
