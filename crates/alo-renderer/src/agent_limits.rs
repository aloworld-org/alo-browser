/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How far alo's agent may go on one task — the person's to set in
//! *Settings*, under *alo at work* (queue item 878, `docs/design/shell.md`,
//! *Settings*; asked by the owner with 821, 8 November 2026): how many
//! sites a run may open, how many actions it may take, and whether it asks
//! before opening any site or only one it neither found by searching nor
//! was named by the person.
//!
//! The data alone, as the browser process holds it, keeps it in
//! `settings.txt` (`alo-window`'s `settings_file`), reads it at each run's
//! start, and tells the first screen's renderer of it to draw. What is
//! **not** here, because it is not a setting: asking before paying,
//! sending, deleting or sending a form, and never typing a password or a
//! card's details (ADR 0096 §§ 5–6, ADR 0099).
//!
//! # The bounds, and why
//!
//! - **Sites, 1 to 10, 3 unless set** ([`SITES`]): one is the least that
//!   lets a run begun with no page do anything — it must open one; ten is
//!   past any task a person follows step by step, each site a card they
//!   see opened. Links followed inside a site are not counted.
//! - **Actions, 5 to 100 in fives, 20 unless set** ([`ACTIONS`],
//!   [`ACTIONS_STEP`]): five is the least that searches, opens a page and
//!   still acts on it; a hundred is five times the default, and the run's
//!   own token limit (400,000 tokens, about $0.40) still stops a long run on
//!   a large page first. In fives, so the keys reach either end in
//!   nineteen presses, not ninety-five.

use core::ops::RangeInclusive;

/// The sites a run may open: the least, and the most.
pub const SITES: RangeInclusive<u8> = 1..=10;

/// The sites a run may open unless the person set otherwise.
pub const DEFAULT_SITES: u8 = 3;

/// The actions a run may take: the least, and the most.
pub const ACTIONS: RangeInclusive<u16> = 5..=100;

/// The actions a run may take unless the person set otherwise: ADR 0096
/// § 4's twenty.
pub const DEFAULT_ACTIONS: u16 = 20;

/// How far one press of *More* or *Fewer* moves the actions.
pub const ACTIONS_STEP: u16 = 5;

/// Before which sites alo asks the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Asks {
    /// Before none: any public site is opened without a question — the
    /// default since the owner's decision of 8 November 2026 (queue item
    /// 884, ADR 0103 as amended), as other browsers' agents do. What is on
    /// this machine or its network is still never opened unless the person
    /// wrote it, and that is not a setting.
    #[default]
    Never,
    /// Only one it neither found by searching nor was named by the person
    /// — the default before 884.
    Unnamed,
    /// Every site it opens.
    Every,
}

impl Asks {
    /// Each, in the order *Settings* offers them.
    pub const ALL: [Asks; 3] = [Asks::Never, Asks::Unnamed, Asks::Every];

    /// The word a file before queue item 884 kept the old default by. Every
    /// such file wrote it, chosen or not, so it cannot say the person chose
    /// it: it is read as the new default, [`Asks::Never`].
    pub const BEFORE: &str = "unnamed";

    /// The word `settings.txt` keeps it by.
    pub fn word(self) -> &'static str {
        match self {
            Asks::Never => "never",
            Asks::Unnamed => "not-found-or-named",
            Asks::Every => "every",
        }
    }

    /// The choice `word` keeps, if it is one — [`Asks::BEFORE`] read as
    /// [`Asks::Never`].
    pub fn of_word(word: &str) -> Option<Self> {
        if word == Self::BEFORE {
            return Some(Asks::Never);
        }
        Self::ALL.into_iter().find(|asks| asks.word() == word)
    }

    /// The choice after this one, or, not `on`, before it, round from
    /// either end: an arrow on the radio group (queue items 878, 884).
    #[must_use]
    pub fn stepped(self, on: bool) -> Self {
        let all = Self::ALL;
        let at = all
            .iter()
            .position(|each| *each == self)
            .unwrap_or_default();
        let next = if on {
            (at + 1) % all.len()
        } else {
            (at + all.len() - 1) % all.len()
        };
        all.get(next).copied().unwrap_or_default()
    }
}

/// Which of the two numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Limit {
    /// The sites a run may open.
    Sites,
    /// The actions a run may take.
    Actions,
}

/// A press of *Fewer* or *More* on one of the numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Stepped {
    /// Which.
    pub limit: Limit,
    /// More, rather than fewer.
    pub more: bool,
}

/// How far alo's agent may go on one task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Limits {
    /// The most sites a run opens, within [`SITES`].
    pub sites: u8,
    /// The most actions a run takes, within [`ACTIONS`], a multiple of
    /// [`ACTIONS_STEP`].
    pub actions: u16,
    /// Before which sites it asks.
    pub asks: Asks,
}

impl Default for Limits {
    /// The owner's design: three sites, twenty actions, asking only before
    /// a site neither found nor named.
    fn default() -> Self {
        Self {
            sites: DEFAULT_SITES,
            actions: DEFAULT_ACTIONS,
            asks: Asks::Never,
        }
    }
}

impl Limits {
    /// These limits, if each is within its bounds — the actions a multiple
    /// of [`ACTIONS_STEP`] — and [`None`] otherwise.
    pub fn checked(sites: u8, actions: u16, asks: Asks) -> Option<Self> {
        (SITES.contains(&sites)
            && ACTIONS.contains(&actions)
            && actions.is_multiple_of(ACTIONS_STEP))
        .then_some(Self {
            sites,
            actions,
            asks,
        })
    }

    /// Whether `stepped` can move its number: not past either end.
    pub fn can(self, stepped: Stepped) -> bool {
        match (stepped.limit, stepped.more) {
            (Limit::Sites, true) => self.sites < *SITES.end(),
            (Limit::Sites, false) => self.sites > *SITES.start(),
            (Limit::Actions, true) => self.actions < *ACTIONS.end(),
            (Limit::Actions, false) => self.actions > *ACTIONS.start(),
        }
    }

    /// These limits with `stepped` made, stopping at either end rather than
    /// going round: a number is not a choice among equals, and going round
    /// from 100 actions to 5 would surprise.
    #[must_use]
    pub fn step(self, stepped: Stepped) -> Self {
        if !self.can(stepped) {
            return self;
        }
        let mut stepped_to = self;
        match (stepped.limit, stepped.more) {
            (Limit::Sites, true) => stepped_to.sites += 1,
            (Limit::Sites, false) => stepped_to.sites -= 1,
            (Limit::Actions, true) => stepped_to.actions += ACTIONS_STEP,
            (Limit::Actions, false) => stepped_to.actions -= ACTIONS_STEP,
        }
        stepped_to
    }

    /// The presses *Settings* offers, in the order it draws them: *Fewer*
    /// and *More* for the sites, then for the actions — each only while it
    /// can move its number.
    pub fn presses(self) -> Vec<Stepped> {
        [Limit::Sites, Limit::Actions]
            .into_iter()
            .flat_map(|limit| [false, true].map(|more| Stepped { limit, more }))
            .filter(|stepped| self.can(*stepped))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_the_owners_design_and_within_its_bounds() {
        let limits = Limits::default();
        assert_eq!((limits.sites, limits.actions), (3, 20));
        assert_eq!(limits.asks, Asks::Never);
        assert_eq!(
            Limits::checked(limits.sites, limits.actions, limits.asks),
            Some(limits)
        );
    }

    #[test]
    fn each_number_is_checked_against_its_bounds() {
        assert!(Limits::checked(1, 5, Asks::Every).is_some());
        assert!(Limits::checked(10, 100, Asks::Unnamed).is_some());
        assert_eq!(Limits::checked(0, 20, Asks::Unnamed), None);
        assert_eq!(Limits::checked(11, 20, Asks::Unnamed), None);
        assert_eq!(Limits::checked(3, 0, Asks::Unnamed), None);
        assert_eq!(Limits::checked(3, 105, Asks::Unnamed), None);
        assert_eq!(Limits::checked(3, 21, Asks::Unnamed), None, "in fives");
    }

    #[test]
    fn a_step_moves_by_one_site_or_five_actions_and_stops_at_either_end() {
        let more_sites = Stepped {
            limit: Limit::Sites,
            more: true,
        };
        let fewer_actions = Stepped {
            limit: Limit::Actions,
            more: false,
        };
        let limits = Limits::default();
        assert_eq!(limits.step(more_sites).sites, 4);
        assert_eq!(limits.step(fewer_actions).actions, 15);
        let mut least = limits;
        for _ in 0..30 {
            least = least.step(fewer_actions);
        }
        assert_eq!(least.actions, 5, "stops, never round");
        let mut most = limits;
        for _ in 0..30 {
            most = most.step(more_sites);
        }
        assert_eq!(most.sites, 10);
        // At an end, the press that would pass it is not offered.
        assert_eq!(
            most.presses(),
            vec![
                Stepped {
                    limit: Limit::Sites,
                    more: false
                },
                Stepped {
                    limit: Limit::Actions,
                    more: false
                },
                Stepped {
                    limit: Limit::Actions,
                    more: true
                },
            ]
        );
        assert_eq!(limits.presses().len(), 4);
    }

    #[test]
    fn asking_is_kept_by_a_word() {
        for asks in Asks::ALL {
            assert_eq!(Asks::of_word(asks.word()), Some(asks));
        }
        assert_eq!(Asks::of_word("always"), None);
        // A file from before queue item 884: its old default, which it
        // always wrote, read as the new one (queue item 884).
        assert_eq!(Asks::of_word("unnamed"), Some(Asks::Never));
        assert_eq!(Asks::default(), Asks::Never);
        // The arrows, round from either end.
        assert_eq!(Asks::Never.stepped(true), Asks::Unnamed);
        assert_eq!(Asks::Every.stepped(true), Asks::Never);
        assert_eq!(Asks::Never.stepped(false), Asks::Every);
        assert_eq!(Asks::Unnamed.stepped(false), Asks::Never);
    }
}
