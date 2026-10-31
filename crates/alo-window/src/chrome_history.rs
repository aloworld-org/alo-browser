/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The card's *Back* and *Forward* (queue item 737): whether there is
//! somewhere to go, read from the tab's session history, and going there.
//!
//! # Read after each navigation
//!
//! Chrome keeps the tab's session history; this asks for it
//! (`Page.getNavigationHistory`) only when the tab's own frame has committed
//! a document or moved within one since it was last read
//! ([`Tab::navigations`]), and tells the conversation where the card can go
//! only when that changed ([`Travel`]). A page that pushes a hundred states
//! costs one read for each pass of the thread holding it, not one for each.
//!
//! # The card's own history
//!
//! A card's page is opened in the shown window's own tab, which held
//! `data:,` first. That entry is not the card's: *Back* from the card's
//! first page would show a blank page nobody chose. So the entries the card
//! goes through are those after the one the window held as the card's page
//! was gone to ([`Tab::before`]) — all of them once Chrome has dropped it.
//!
//! # A person's, never an agent's
//!
//! Going back or forward is asked of Chrome as its own buttons ask
//! (`Page.navigateToHistoryEntry`), never by the page's script, and only a
//! press on the card's own buttons reaches it
//! ([`crate::message::CardOrder::Travel`]); no agent verb goes back or
//! forward. The entry gone to is the one next to the page **as Chrome holds
//! it when the press is carried out**, read again then, so a page that went
//! somewhere between the drawing and the press is gone back from where it
//! is.

use crate::chrome::Failed;
use crate::chrome_protocol::{self, Command, History};
use crate::chrome_tab::Tab;

/// Where the card can go from its page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Travel {
    /// Whether there is a page of the card's to go back to.
    pub back: bool,
    /// Whether there is a page to go forward to.
    pub forward: bool,
}

/// Which way a person pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// The card's *Back*.
    Back,
    /// The card's *Forward*.
    Forward,
}

/// Where the card can go in `history`, the card's own entries being those
/// after `before`.
pub fn travel(history: &History, before: Option<i64>) -> Travel {
    Travel {
        back: entry(history, before, Way::Back).is_some(),
        forward: entry(history, before, Way::Forward).is_some(),
    }
}

/// The entry of `history` a press `way` goes to, the card's own entries
/// being those after `before`: none at either end.
pub fn entry(history: &History, before: Option<i64>, way: Way) -> Option<i64> {
    let first = before
        .and_then(|before| history.entries.iter().position(|id| *id == before))
        .map_or(0, |at| at + 1);
    let to = match way {
        Way::Back => history.current.checked_sub(1)?,
        Way::Forward => history.current.checked_add(1)?,
    };
    if to < first {
        return None;
    }
    history.entries.get(to).copied()
}

/// What the thread holding a card's tab last read and told of its history.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Told {
    /// [`Tab::navigations`] when the history was last read.
    read_at: Option<u64>,
    /// Where the card was last told it can go.
    told: Option<Travel>,
}

impl Told {
    /// Where the card can go now, if that is not what was last told: the
    /// history read again only when the tab has navigated since it was last
    /// read.
    ///
    /// # Errors
    ///
    /// [`Failed`], when Chrome declines, does not answer or answers what
    /// cannot be read: the history is read again at the next pass.
    pub fn changed(&mut self, tab: &mut Tab<'_>) -> Result<Option<Travel>, Failed> {
        let now = tab.navigations();
        if self.read_at == Some(now) {
            return Ok(None);
        }
        let travel = travel(&read(tab)?, tab.before());
        self.read_at = Some(now);
        if self.told == Some(travel) {
            return Ok(None);
        }
        self.told = Some(travel);
        Ok(Some(travel))
    }
}

/// Go `way` through `tab`'s history, as Chrome's own buttons do, if the
/// card has somewhere to go that way: whether it was asked.
///
/// # Errors
///
/// [`Failed`], when Chrome declines, does not answer or answers what cannot
/// be read.
pub fn go(tab: &mut Tab<'_>, way: Way) -> Result<bool, Failed> {
    let Some(entry) = entry(&read(tab)?, tab.before(), way) else {
        return Ok(false);
    };
    tab.ask(&Command::GoToEntry { entry })?;
    Ok(true)
}

/// `tab`'s session history, as Chrome holds it now.
fn read(tab: &mut Tab<'_>) -> Result<History, Failed> {
    let result = tab.ask(&Command::ReadHistory)?;
    chrome_protocol::history(&result).map_err(Failed::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(current: usize, entries: &[i64]) -> History {
        History {
            current,
            entries: entries.to_vec(),
        }
    }

    #[test]
    fn the_windows_own_blank_page_is_not_the_cards_to_go_back_to() {
        // The window's data:, (1), then the card's first page (2).
        let opened = history(1, &[1, 2]);
        assert_eq!(travel(&opened, Some(1)), Travel::default());
        assert_eq!(entry(&opened, Some(1), Way::Back), None);
        // Read with no window before: the first entry is the card's.
        assert_eq!(
            travel(&opened, None),
            Travel {
                back: true,
                forward: false
            }
        );
    }

    #[test]
    fn a_link_followed_goes_back_and_then_forward() {
        let followed = history(2, &[1, 2, 3]);
        assert_eq!(
            travel(&followed, Some(1)),
            Travel {
                back: true,
                forward: false
            }
        );
        assert_eq!(entry(&followed, Some(1), Way::Back), Some(2));
        assert_eq!(entry(&followed, Some(1), Way::Forward), None);
        let back = history(1, &[1, 2, 3]);
        assert_eq!(
            travel(&back, Some(1)),
            Travel {
                back: false,
                forward: true
            }
        );
        assert_eq!(entry(&back, Some(1), Way::Forward), Some(3));
    }

    #[test]
    fn once_chrome_drops_the_windows_entry_every_entry_is_the_cards() {
        let long = history(1, &[7, 8, 9]);
        assert_eq!(entry(&long, Some(1), Way::Back), Some(7));
        assert_eq!(entry(&long, Some(1), Way::Forward), Some(9));
    }

    #[test]
    fn a_page_at_the_windows_own_entry_goes_nowhere_but_forward_into_the_card() {
        // A script's history.go(-1) on the card's first page: the window's
        // own entry is shown, and only forward, into the card's, goes.
        let gone = history(0, &[1, 2]);
        assert_eq!(entry(&gone, Some(1), Way::Back), None);
        assert_eq!(entry(&gone, Some(1), Way::Forward), Some(2));
    }
}
