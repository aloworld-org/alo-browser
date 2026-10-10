/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A load's first frame, held back until its style sheets have answered —
//! within a bound of the window's own (ADR 0035 § 5, ADR 0041 § 3, queue
//! item 351).
//!
//! When a tab's `Load` is answered, the sheets that answer asked for are the
//! load's **owed** sheets, and the tab is held: the conductor does everything
//! it would — lays out, supplies fonts, tells the page it is shown — but
//! paints none of it, and the window goes on showing what it was last sent.
//! The hold ends at the first of:
//!
//! - every owed sheet answered — delivered, refused or failed, since each is
//!   a sheet that is not coming any more;
//! - [`LONGEST_HOLD`] passing, after which the page is painted with what has
//!   arrived and the person is told [`SHOWN_BEFORE_STYLE`];
//! - the tab no longer showing that document.
//!
//! A load that asked for no sheet is not held. A sheet asked for after the
//! load — by a script, in a delivery's answer — is never owed, and holds
//! nothing back: HTML's rule, and ADR 0035 § 5's.
//!
//! This is the bookkeeping, and only that: which tab is held for which
//! document, what it is owed and until when, and what was to be said about it
//! meanwhile — a sheet that failed, say — which waits to be said with its
//! first frame rather than over a frame that is not its page's. The conductor decides what to
//! do when a hold ends, and the network stack never learns the bound — no
//! exchange is ended because it passed.

use alo_net::cause::DocumentId;
use alo_renderer::TabId;
use std::time::{Duration, Instant};

/// How long a load's first frame waits for its style sheets.
///
/// ADR 0041 § 3: *a few seconds, well inside `PATIENCE`*. The bound exists
/// for a server that trickles — one that stays inside `alo-net`'s thirty
/// seconds of patience per read for as long as it likes — so it must be far
/// shorter than that, or a person would look at nothing for half a minute
/// before being told anything was wrong. Three seconds is the point past
/// which a blank page reads as a broken one rather than a slow one, and it
/// is longer than an ordinary connection takes to answer a sheet from a page's
/// own server. It is a guess until somebody measures it: ADR 0041's *How we
/// will know* says what would show it is too short or too long.
pub const LONGEST_HOLD: Duration = Duration::from_secs(3);

/// What the person is told when a page is painted at the bound, whatever was
/// late — the same words for every sheet, as ADR 0035 § 3 says of a failed
/// one.
pub const SHOWN_BEFORE_STYLE: &str = "this page is shown before its style arrived";

/// One tab's hold.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Hold {
    tab: TabId,
    document: DocumentId,
    owed: Vec<u64>,
    until: Instant,
    /// What was to be said while it was held.
    deferred: Vec<String>,
}

/// Every tab held, and for what.
#[derive(Debug, Default)]
pub struct Holds {
    held: Vec<Hold>,
}

impl Holds {
    /// Nobody held.
    pub fn new() -> Self {
        Self::default()
    }

    /// Tab `tab` has loaded `document`, whose load's answer asked for the
    /// sheets `owed`, at `now`. Any hold it had for an earlier document is
    /// dropped; it is held again only if this load owes a sheet.
    pub fn loaded(&mut self, tab: TabId, document: DocumentId, owed: Vec<u64>, now: Instant) {
        self.held.retain(|hold| hold.tab != tab);
        if owed.is_empty() {
            return;
        }
        self.held.push(Hold {
            tab,
            document,
            owed,
            until: now.checked_add(LONGEST_HOLD).unwrap_or(now),
            deferred: Vec::new(),
        });
    }

    /// Whether tab `tab`, showing `showing`, is held: nothing of it may be
    /// painted.
    pub fn is_held(&self, tab: TabId, showing: Option<DocumentId>) -> bool {
        self.held
            .iter()
            .any(|hold| hold.tab == tab && Some(hold.document) == showing)
    }

    /// Sheet `sheet` of tab `tab`'s page has been answered. When that was
    /// the last sheet its load owed, the hold ends, and what was deferred
    /// while it lasted comes back to be said.
    pub fn answered(&mut self, tab: TabId, sheet: u64) -> Option<Vec<String>> {
        let at = self.held.iter().position(|hold| hold.tab == tab)?;
        let hold = self.held.get_mut(at)?;
        hold.owed.retain(|owed| *owed != sheet);
        if hold.owed.is_empty() {
            return Some(self.held.remove(at).deferred);
        }
        None
    }

    /// Keep `said`, about held tab `tab`'s page, to be said when its hold
    /// ends. Said by nobody if the tab is not held, or stops showing the
    /// page it was about.
    pub fn defer(&mut self, tab: TabId, said: Vec<String>) {
        if let Some(hold) = self.held.iter_mut().find(|hold| hold.tab == tab) {
            hold.deferred.extend(said);
        }
    }

    /// The soonest moment a hold's bound passes, if anybody is held: as long
    /// as the conductor may wait for anything else.
    pub fn until(&self) -> Option<Instant> {
        self.held.iter().map(|hold| hold.until).min()
    }

    /// Every tab whose bound has passed by `now`, no longer held — to be
    /// painted, and said to have been shown before its style — with what was
    /// deferred while it was.
    pub fn passed(&mut self, now: Instant) -> Vec<(TabId, Vec<String>)> {
        let (passed, kept): (Vec<Hold>, Vec<Hold>) = core::mem::take(&mut self.held)
            .into_iter()
            .partition(|hold| hold.until <= now);
        self.held = kept;
        passed
            .into_iter()
            .map(|hold| (hold.tab, hold.deferred))
            .collect()
    }

    /// Drop the hold of every tab not showing the document it was held for:
    /// closed, or loaded with another page. `showing` says what a tab shows.
    pub fn keep_showing(&mut self, showing: impl Fn(TabId) -> Option<DocumentId>) {
        self.held
            .retain(|hold| showing(hold.tab) == Some(hold.document));
    }

    /// Nobody held: every tab is closing.
    pub fn clear(&mut self) {
        self.held.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_net::cause::Identities;

    /// A tab, and two documents it could show.
    fn minted() -> (TabId, DocumentId, DocumentId) {
        let mut minting = Identities::default();
        (minting.a_tab(), minting.a_document(), minting.a_document())
    }

    #[test]
    fn a_load_that_owes_no_sheet_is_not_held() {
        let (tab, document, _) = minted();
        let mut holds = Holds::new();
        holds.loaded(tab, document, Vec::new(), Instant::now());
        assert!(!holds.is_held(tab, Some(document)));
        assert_eq!(holds.until(), None);
    }

    #[test]
    fn a_load_is_held_until_its_last_sheet_is_answered() {
        let (tab, document, _) = minted();
        let mut holds = Holds::new();
        holds.loaded(tab, document, vec![1, 2], Instant::now());
        assert!(holds.is_held(tab, Some(document)));
        assert_eq!(holds.answered(tab, 2), None, "one is still owed");
        assert!(holds.is_held(tab, Some(document)));
        holds.defer(tab, vec!["the second sheet failed".to_owned()]);
        assert_eq!(holds.answered(tab, 7), None, "never owed, so nothing");
        assert_eq!(
            holds.answered(tab, 1),
            Some(vec!["the second sheet failed".to_owned()]),
            "the last one ends it, with what waited to be said"
        );
        assert!(!holds.is_held(tab, Some(document)));
        assert_eq!(holds.answered(tab, 1), None, "and it ends once");
    }

    #[test]
    fn the_bound_ends_a_hold_at_longest_hold_and_not_before() {
        let (tab, document, _) = minted();
        let mut holds = Holds::new();
        let then = Instant::now();
        holds.loaded(tab, document, vec![1], then);
        assert_eq!(holds.until(), then.checked_add(LONGEST_HOLD));
        let just_before = then
            .checked_add(LONGEST_HOLD)
            .and_then(|at| at.checked_sub(Duration::from_millis(1)))
            .unwrap_or(then);
        assert!(holds.passed(just_before).is_empty());
        assert!(holds.is_held(tab, Some(document)));
        let at = then.checked_add(LONGEST_HOLD).unwrap_or(then);
        holds.defer(tab, vec!["late".to_owned()]);
        assert_eq!(holds.passed(at), vec![(tab, vec!["late".to_owned()])]);
        assert!(!holds.is_held(tab, Some(document)));
        assert_eq!(holds.until(), None);
    }

    #[test]
    fn the_soonest_bound_is_the_one_waited_for() {
        let mut minting = Identities::default();
        let (tab, other) = (minting.a_tab(), minting.a_tab());
        let (first, second) = (minting.a_document(), minting.a_document());
        let mut holds = Holds::new();
        let then = Instant::now();
        let later = then.checked_add(Duration::from_secs(1)).unwrap_or(then);
        holds.loaded(other, second, vec![1], later);
        holds.loaded(tab, first, vec![1], then);
        assert_eq!(holds.until(), then.checked_add(LONGEST_HOLD));
    }

    #[test]
    fn a_tab_showing_another_document_is_not_held() {
        let (tab, first, second) = minted();
        let mut holds = Holds::new();
        holds.loaded(tab, first, vec![1], Instant::now());
        assert!(!holds.is_held(tab, Some(second)));
        assert!(!holds.is_held(tab, None));
        holds.keep_showing(|_| Some(second));
        assert_eq!(holds.until(), None, "its hold is dropped");
    }

    #[test]
    fn a_new_load_replaces_the_last_hold() {
        let (tab, first, second) = minted();
        let mut holds = Holds::new();
        holds.loaded(tab, first, vec![1], Instant::now());
        holds.loaded(tab, second, vec![4], Instant::now());
        assert!(!holds.is_held(tab, Some(first)));
        assert!(holds.is_held(tab, Some(second)));
        assert_eq!(holds.answered(tab, 1), None, "the first load's sheet");
        assert_eq!(holds.answered(tab, 4), Some(Vec::new()));
        holds.loaded(tab, first, Vec::new(), Instant::now());
        assert_eq!(holds.until(), None, "a load owing nothing ends it");
    }
}
