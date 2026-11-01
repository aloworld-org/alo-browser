/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! No page closed while a context menu is open (queue item 734): what the
//! host closes at once, and what waits for the menu to go first.
//!
//! **Measured** on this Mac with CEF 156.0.14: a page closed while its
//! context menu was still shown — by `Browser.close`, by `Target.closeTarget`
//! then `Browser.close`, or by a person's close button — ended the host with
//! `SIGSEGV` on its main thread inside CEF's framework, reading freed memory
//! (`0xcdcdcdcdcdcdcdcd`), from a task the outer run loop ran after the
//! page's `on_before_close`; CEF never said the menu was dismissed
//! (`on_context_menu_dismissed`). Leaving the window open as its page went
//! (no `on_browser_destroyed` close) crashed the same, so it is not the
//! host's window closing under the menu: the menu outlives the page it was
//! shown for. A menu dismissed first (Escape) left the host ending in order.
//!
//! So the host never closes a page with a menu shown: an ask to close every
//! page ([`Closing::close_all`]) or one window ([`Closing::close_window`])
//! while one is shown is **owed** — the menu is dismissed (`dismissed_mac.rs`)
//! and what was owed is carried out once CEF says it is gone
//! ([`Closing::dismissed`]). A menu CEF was told of but never showed would
//! leave it owed for good, so the host gives up waiting after
//! [`GIVE_UP_AFTER`] and closes anyway ([`Closing::given_up`]).

/// How long a close owed waits for a menu to go before it is carried out
/// anyway, in milliseconds: a second, long past an Escape's reaching a menu
/// in the same process, and far short of the thirty seconds alo gives a
/// closing Chromium.
pub const GIVE_UP_AFTER: i64 = 1000;

/// What the host is to close, and whether it waits for a menu first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Closing {
    /// How many context menus CEF said it is showing and has not said are
    /// gone.
    shown: usize,
    /// What was asked while one was.
    owed: Owed,
}

/// What was asked to close while a menu was shown.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Owed {
    /// Every page, the host ending.
    pub all: bool,
    /// The windows of these pages, by their browser's identifier, a person
    /// closing each.
    pub windows: Vec<i32>,
}

impl Owed {
    /// Whether nothing is owed.
    pub fn is_empty(&self) -> bool {
        !self.all && self.windows.is_empty()
    }
}

/// Whether an ask to close is carried out now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Now {
    /// Close now: no menu is shown.
    Close,
    /// A menu is shown: dismiss it, and close once it is gone. `first` is
    /// whether this is the first ask owed, which starts the wait.
    Owed {
        /// Whether nothing was owed before.
        first: bool,
    },
}

impl Closing {
    /// CEF is about to show a context menu.
    pub fn shown(&mut self) {
        self.shown = self.shown.saturating_add(1);
    }

    /// CEF said a context menu is gone: what was owed, to carry out now,
    /// once no menu is shown.
    pub fn dismissed(&mut self) -> Owed {
        self.shown = self.shown.saturating_sub(1);
        if self.shown == 0 {
            std::mem::take(&mut self.owed)
        } else {
            Owed::default()
        }
    }

    /// The wait for a menu to go is over, whether or not it went: what was
    /// owed, to carry out now; no menu is counted as shown after.
    pub fn given_up(&mut self) -> Owed {
        self.shown = 0;
        std::mem::take(&mut self.owed)
    }

    /// Every page is to close.
    pub fn close_all(&mut self) -> Now {
        self.owe(|owed| owed.all = true)
    }

    /// The window of the page `browser` is to close, as a person asked.
    pub fn close_window(&mut self, browser: i32) -> Now {
        self.owe(|owed| {
            if !owed.windows.contains(&browser) {
                owed.windows.push(browser);
            }
        })
    }

    /// Now or owed, as `owing` puts it in what is owed.
    fn owe(&mut self, owing: impl FnOnce(&mut Owed)) -> Now {
        if self.shown == 0 {
            return Now::Close;
        }
        let first = self.owed.is_empty();
        owing(&mut self.owed);
        Now::Owed { first }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_no_menu_shown_everything_closes_now() {
        let mut closing = Closing::default();
        assert_eq!(closing.close_all(), Now::Close);
        assert_eq!(closing.close_window(1), Now::Close);
        // A menu shown and gone leaves nothing waiting.
        closing.shown();
        assert!(closing.dismissed().is_empty());
        assert_eq!(closing.close_all(), Now::Close);
    }

    #[test]
    fn with_a_menu_shown_a_close_is_owed_until_it_is_gone() {
        let mut closing = Closing::default();
        closing.shown();
        assert_eq!(closing.close_all(), Now::Owed { first: true });
        assert_eq!(
            closing.close_all(),
            Now::Owed { first: false },
            "asked twice, owed once"
        );
        assert_eq!(closing.close_window(7), Now::Owed { first: false });
        assert_eq!(closing.close_window(7), Now::Owed { first: false });
        assert_eq!(
            closing.dismissed(),
            Owed {
                all: true,
                windows: vec![7]
            }
        );
        assert!(closing.dismissed().is_empty(), "carried out once");
        assert_eq!(closing.close_window(7), Now::Close);
    }

    #[test]
    fn two_menus_shown_wait_for_both() {
        let mut closing = Closing::default();
        closing.shown();
        closing.shown();
        assert_eq!(closing.close_window(3), Now::Owed { first: true });
        assert!(closing.dismissed().is_empty(), "one still shown");
        assert_eq!(
            closing.dismissed(),
            Owed {
                all: false,
                windows: vec![3]
            }
        );
    }

    #[test]
    fn a_menu_never_said_gone_is_given_up_on() {
        let mut closing = Closing::default();
        closing.shown();
        assert_eq!(closing.close_all(), Now::Owed { first: true });
        assert_eq!(
            closing.given_up(),
            Owed {
                all: true,
                windows: Vec::new()
            }
        );
        assert_eq!(closing.close_all(), Now::Close, "no menu counted after");
        assert!(closing.given_up().is_empty());
    }
}
