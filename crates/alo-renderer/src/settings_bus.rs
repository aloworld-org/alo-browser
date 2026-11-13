/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Settings*' fifth section, **Screen readers**, as data (ADR 0115,
//! *Phase (c)'s guard*, (a); queue item 989): what alo publishes on the
//! platform's accessibility bus — the tree a screen reader reads, and any
//! other program the person lets read the screen — and whether it takes
//! the actions asked there. Drawn by [`crate::settings_bus_document`],
//! decided by the browser process.
//!
//! # On by default, for the person it is for
//!
//! ADR 0112 requirement 7 asks for the tree published by default, and a
//! person who cannot see the screen cannot find a switch that is off
//! without the very tree it withholds: [`Bus::ReadAndAct`] is the
//! default, and the choice is changed only by a person's press in
//! *Settings* — never by a press from the bus, nor anything over the
//! agent socket (ADR 0026's rule for grants, kept here).
//!
//! # What a press is on
//!
//! Its three choices, in the order they are drawn ([`Bus::ALL`]); the
//! browser process decides a press by its place among them, against the
//! same list it painted, as *Privacy*'s are ([`crate::conversation_claim`]).

/// What alo gives the accessibility bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Bus {
    /// alo's interface and a card's page published, and the actions asked
    /// on them taken as a person's press would be: the default.
    #[default]
    ReadAndAct,
    /// The same published, and every action asked refused — and recorded.
    ReadOnly,
    /// Nothing published but the window itself, and nothing taken.
    Off,
}

impl Bus {
    /// Each, in the order they are drawn.
    pub const ALL: [Bus; 3] = [Bus::ReadAndAct, Bus::ReadOnly, Bus::Off];

    /// What its choice says, and is named.
    pub fn said(self) -> &'static str {
        match self {
            Self::ReadAndAct => "Read and act",
            Self::ReadOnly => "Read only",
            Self::Off => "Off",
        }
    }

    /// The quiet line under its choice, if it has one: what it costs.
    pub fn note(self) -> Option<&'static str> {
        match self {
            Self::ReadAndAct => None,
            Self::ReadOnly => Some("A screen reader can read alo, but nothing it presses is done."),
            Self::Off => Some("A screen reader will not be able to read alo."),
        }
    }

    /// Its word in the settings file.
    pub fn word(self) -> &'static str {
        match self {
            Self::ReadAndAct => "act",
            Self::ReadOnly => "read",
            Self::Off => "off",
        }
    }

    /// The choice `word` names in the settings file, if any.
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|each| each.word() == word)
    }

    /// Whether anything of alo's but the window itself is published.
    pub fn publishes(self) -> bool {
        self != Self::Off
    }

    /// Whether an action asked on the bus may be taken.
    pub fn takes_actions(self) -> bool {
        self == Self::ReadAndAct
    }

    /// The next, after it when `on`, or the one before, round.
    #[must_use]
    pub fn stepped(self, on: bool) -> Self {
        let at = Self::ALL
            .iter()
            .position(|each| *each == self)
            .unwrap_or_default();
        let next = if on {
            (at + 1) % Self::ALL.len()
        } else {
            (at + Self::ALL.len() - 1) % Self::ALL.len()
        };
        Self::ALL.get(next).copied().unwrap_or(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_and_act_is_the_default_and_each_word_reads_back() {
        assert_eq!(Bus::default(), Bus::ReadAndAct);
        for bus in Bus::ALL {
            assert_eq!(Bus::of_word(bus.word()), Some(bus));
        }
        assert_eq!(Bus::of_word("on"), None);
        assert_eq!(
            Bus::of_word("Off"),
            None,
            "a word as written, never its case"
        );
    }

    #[test]
    fn each_says_what_it_gives_and_off_what_it_costs() {
        assert_eq!(
            Bus::Off.note(),
            Some("A screen reader will not be able to read alo.")
        );
        assert_eq!(Bus::ReadAndAct.note(), None);
        let said: std::collections::HashSet<_> = Bus::ALL.iter().map(|bus| bus.said()).collect();
        assert_eq!(said.len(), 3);
        assert_eq!(
            Bus::ALL.map(|bus| (bus.publishes(), bus.takes_actions())),
            [(true, true), (true, false), (false, false)]
        );
    }

    #[test]
    fn an_arrow_steps_round_either_way() {
        assert_eq!(Bus::ReadAndAct.stepped(true), Bus::ReadOnly);
        assert_eq!(Bus::Off.stepped(true), Bus::ReadAndAct);
        assert_eq!(Bus::ReadAndAct.stepped(false), Bus::Off);
        assert_eq!(Bus::ReadOnly.stepped(false), Bus::ReadAndAct);
    }
}
