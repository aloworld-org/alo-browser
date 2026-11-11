/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a card's page is called, told to the conversation as it changes
//! (queue item 958, ADR 0112 § 1): the window's title follows the open
//! card's page, as alo OS asks of a browser.
//!
//! Chrome keeps a page's title in its list of targets
//! ([`crate::chrome_named`]). Reading that list is a command, so it is read
//! at most every [`EVERY`] while the card streams — a page that retitles
//! itself is followed within that — and the conversation is told only when
//! the title is not what it was last told ([`ToFrame::Named`]). A reading
//! that fails, or a card not streamed, tells nothing: the title is a moment
//! old, never wrong about which page.
//!
//! # A stranger's string
//!
//! A title is the page's to choose, and this is data handed on — never
//! shaped here. Where it goes from the window is
//! [`crate::window_title`]'s to decide.

use crate::chrome_card::TellCard;
use crate::chrome_named;
use crate::chrome_tab::Tab;
use crate::frame_told::ToFrame;
use std::time::{Duration, Instant};

/// How often a streamed card's page's title is read at most.
pub const EVERY: Duration = Duration::from_secs(1);

/// What the conversation was last told a card's page is called, and when
/// to read it again.
#[derive(Debug, Default)]
pub struct Titling {
    told: Option<String>,
    next: Option<Instant>,
}

impl Titling {
    /// Whether the title is due to be read at `now`.
    fn due(&self, now: Instant) -> bool {
        self.next.is_none_or(|next| now >= next)
    }

    /// A reading at `now` came to `title`: what to tell, if it is not what
    /// was told last.
    fn heard(&mut self, now: Instant, title: String) -> Option<String> {
        self.next = Some(now + EVERY);
        if self.told.as_deref() == Some(title.as_str()) {
            return None;
        }
        self.told = Some(title.clone());
        Some(title)
    }

    /// A reading at `now` failed: tried again after [`EVERY`].
    fn unheard(&mut self, now: Instant) {
        self.next = Some(now + EVERY);
    }
}

/// Tell `tell` what `tab`'s page is called, if it streams, the title is due
/// to be read, and it is not what `titling` last told.
pub fn tell_titled(tab: &mut Tab<'_>, titling: &mut Titling, tell: TellCard<'_>) {
    let now = Instant::now();
    if !tab.casting().is_on() || !titling.due(now) {
        return;
    }
    let Some(tell) = tell else {
        return;
    };
    match chrome_named::read(tab) {
        Ok(Some(named)) => {
            if let Some(title) = titling.heard(now, named.title.clone()) {
                // Nobody to hear it is a conversation that has gone.
                let _ = tell(ToFrame::Named(crate::chrome_protocol::Named {
                    title,
                    address: named.address,
                }));
            }
        }
        Ok(None) | Err(_) => titling.unheard(now),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_is_told_once_until_it_changes_and_read_at_most_every_second() {
        let mut titling = Titling::default();
        let start = Instant::now();
        assert!(titling.due(start));
        assert_eq!(
            titling.heard(start, "Inbox".to_owned()),
            Some("Inbox".to_owned())
        );
        assert!(!titling.due(start + EVERY / 2));
        assert!(titling.due(start + EVERY));
        assert_eq!(titling.heard(start + EVERY, "Inbox".to_owned()), None);
        assert_eq!(
            titling.heard(start + EVERY * 2, "Inbox (2)".to_owned()),
            Some("Inbox (2)".to_owned())
        );
        titling.unheard(start + EVERY * 3);
        assert!(!titling.due(start + EVERY * 3));
        assert!(titling.due(start + EVERY * 4));
    }
}
