/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Settings › Privacy*'s choices, held for the whole browser process
//! (ADR 0108, queue item 920): what the window sets — as it opens, from the
//! settings file, and as a person chooses — and what each Chromium started
//! and each card's thread asks.
//!
//! - **Cookies** are asked as a Chromium is started, and written into its
//!   new profile ([`crate::chrome_cookies`]): a Chromium keeps the choice
//!   it was started with for its life. Each address opened from the
//!   conversation starts a Chromium of its own (`conversation_chrome.rs`,
//!   *One page at a time*), so a choice is used **from the next page alo
//!   opens**, and a page already open keeps the one it opened with — which
//!   *Settings* says.
//! - **Cookie banners** are asked by a card's thread each time its page
//!   has loaded ([`crate::consent_answer`]), so a choice is used from the
//!   next page loaded, in the card already open too.
//!
//! Set by the window alone: no page, renderer or agent can.

use alo_renderer::settings_privacy::{Banners, Cookies, Privacy};
use std::sync::{PoisonError, RwLock};

/// The choices of this process: the defaults until the window reads
/// them.
pub fn process() -> &'static Chosen {
    static PROCESS: std::sync::OnceLock<Chosen> = std::sync::OnceLock::new();
    PROCESS.get_or_init(Chosen::default)
}

/// *Privacy*'s choices, as made now.
#[derive(Debug, Default)]
pub struct Chosen {
    made: RwLock<Privacy>,
}

impl Chosen {
    /// Both, as made now.
    pub fn now(&self) -> Privacy {
        *self.made.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// What is kept of cookies by a Chromium started now.
    pub fn cookies(&self) -> Cookies {
        self.now().cookies
    }

    /// Whether a page loaded now has its cookie banner answered.
    pub fn banners(&self) -> Banners {
        self.now().banners
    }

    /// Both made `privacy`.
    pub fn set(&self, privacy: Privacy) {
        *self.made.write().unwrap_or_else(PoisonError::into_inner) = privacy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_until_set_then_what_was_set() {
        let chosen = Chosen::default();
        assert_eq!(chosen.cookies(), Cookies::RefuseOthers);
        assert_eq!(chosen.banners(), Banners::Answer);
        chosen.set(Privacy {
            cookies: Cookies::KeepAll,
            banners: Banners::Leave,
        });
        assert_eq!(chosen.cookies(), Cookies::KeepAll);
        assert_eq!(chosen.banners(), Banners::Leave);
    }
}
