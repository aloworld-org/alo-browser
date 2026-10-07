/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A person's answer, and when it stops being one.
//!
//! ADR 0026 § 5: *"A prompt offers three answers, and no fourth"*, and every
//! one of them ends.
//!
//! - **Allow while this page is open** ends with the document that asked. It
//!   is never a [`Remembered`] row: it lives in the table's memory beside the
//!   document, and a restart, which ends every document, ends it too.
//! - **Allow on this site** ends [`THIRTY_DAYS`] after the person last opened
//!   the site themselves.
//! - **Don't allow** is remembered on the same terms, so a page refused once
//!   cannot ask again on every visit.
//!
//! # The clock fails closed
//!
//! *"A grant whose recorded time is in the future of the current clock is
//! treated as expired, so a clock set backwards cannot extend one."* Both the
//! moment of the answer and the moment of the last visit are held to that,
//! because either is enough to make a grant look younger than it is.

use crate::capability::Capability;
use alo_storage::StorageKey;
use std::time::{Duration, SystemTime};

/// How long *allow on this site* and *don't allow* last after the person's
/// last visit.
///
/// Ours to choose and ours to change (ADR 0026 § 5): a change with a
/// measurement behind it is a change of this constant, not a new decision.
pub const THIRTY_DAYS: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// What a person said to a prompt.
///
/// Dismissing the prompt is not one of these. It refuses that one document and
/// stores nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Allowed until the document that asked is gone.
    WhileThisPageIsOpen,
    /// Allowed until thirty days after the person last opened the site.
    OnThisSite,
    /// Refused, and remembered on the same terms as [`Answer::OnThisSite`].
    DoNotAllow,
}

impl Answer {
    /// The byte it is written as.
    pub fn tag(self) -> u8 {
        match self {
            Answer::WhileThisPageIsOpen => 1,
            Answer::OnThisSite => 2,
            Answer::DoNotAllow => 3,
        }
    }

    /// The answer a byte names, or [`None`].
    pub fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Answer::WhileThisPageIsOpen),
            2 => Some(Answer::OnThisSite),
            3 => Some(Answer::DoNotAllow),
            _ => None,
        }
    }
}

/// Which of the two remembered answers a row holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kept {
    /// *Allow on this site*.
    Allowed,
    /// *Don't allow*.
    Refused,
}

/// A remembered answer: one row of the list a person sees (ADR 0026 § 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remembered {
    /// To whom.
    pub key: StorageKey,
    /// For what.
    pub capability: Capability,
    /// Which answer.
    pub kept: Kept,
    /// When the person gave it.
    pub given: SystemTime,
    /// When the person last opened the key's origin themselves. The answer is
    /// such an act, so it starts as [`Remembered::given`].
    pub visited: SystemTime,
    /// When the site last used it, which the list shows and which never
    /// extends it.
    pub last_used: Option<SystemTime>,
}

/// Why a grant ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// The document an *allow while open* grant was made to is gone.
    PageClosed,
    /// Thirty days passed since the person last opened the site.
    ThirtyDays,
    /// A time the row holds is after the current clock: the clock went back,
    /// and that cannot be allowed to extend anything.
    ClockWentBack,
}

impl Ending {
    /// The byte it is written as.
    pub fn tag(self) -> u8 {
        match self {
            Ending::PageClosed => 1,
            Ending::ThirtyDays => 2,
            Ending::ClockWentBack => 3,
        }
    }

    /// The ending a byte names, or [`None`].
    pub fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Ending::PageClosed),
            2 => Some(Ending::ThirtyDays),
            3 => Some(Ending::ClockWentBack),
            _ => None,
        }
    }
}

impl Remembered {
    /// When it ends, measured from the last visit. [`None`] only when that is
    /// further ahead than this machine's clock can name, which is a row that
    /// [`Remembered::over`] has already refused for being in the future.
    pub fn ends(&self) -> Option<SystemTime> {
        self.visited.checked_add(THIRTY_DAYS)
    }

    /// Whether it has ended by `now`, and why. [`None`] while it holds.
    pub fn over(&self, now: SystemTime) -> Option<Ending> {
        if self.given > now || self.visited > now || self.given > self.visited {
            return Some(Ending::ClockWentBack);
        }
        match self.ends() {
            Some(ends) if now < ends => None,
            _ => Some(Ending::ThirtyDays),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_net::Partition;
    use alo_url::Origin;
    use std::time::UNIX_EPOCH;

    fn at(days: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_700_000_000) + Duration::from_secs(days * 86_400)
    }

    fn row(given: SystemTime, visited: SystemTime) -> Remembered {
        let url = alo_url::parse("https://meet.example/").expect("a URL");
        Remembered {
            key: StorageKey::of(&Origin::of(&url), &Partition::of(&url)).expect("a key"),
            capability: Capability::Camera,
            kept: Kept::Allowed,
            given,
            visited,
            last_used: None,
        }
    }

    #[test]
    fn a_row_holds_for_thirty_days_from_the_last_visit_and_not_a_second_more() {
        let granted = row(at(0), at(10));
        assert_eq!(granted.over(at(10)), None);
        assert_eq!(
            granted.over(at(39) + Duration::from_secs(86_399)),
            None,
            "a grant ended a second early"
        );
        assert_eq!(granted.over(at(40)), Some(Ending::ThirtyDays));
        assert_eq!(granted.ends(), Some(at(40)));
    }

    #[test]
    fn a_time_in_the_future_of_the_clock_ends_the_row() {
        assert_eq!(row(at(5), at(5)).over(at(4)), Some(Ending::ClockWentBack));
        assert_eq!(row(at(0), at(5)).over(at(4)), Some(Ending::ClockWentBack));
        // A visit recorded before the answer is a row no clock that only went
        // forward could have written.
        assert_eq!(row(at(5), at(1)).over(at(6)), Some(Ending::ClockWentBack));
    }

    #[test]
    fn every_tag_reads_back_and_no_other_byte_does() {
        for answer in [
            Answer::WhileThisPageIsOpen,
            Answer::OnThisSite,
            Answer::DoNotAllow,
        ] {
            assert_eq!(Answer::from_tag(answer.tag()), Some(answer));
        }
        for ending in [
            Ending::PageClosed,
            Ending::ThirtyDays,
            Ending::ClockWentBack,
        ] {
            assert_eq!(Ending::from_tag(ending.tag()), Some(ending));
        }
        assert_eq!(Answer::from_tag(0), None);
        assert_eq!(Ending::from_tag(4), None);
    }
}
