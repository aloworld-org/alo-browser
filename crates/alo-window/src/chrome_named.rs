/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the page in Chrome is called, by Chrome's word (queue item 681).
//!
//! The frame around Chrome's window shows the page's tab, as this engine's
//! window shows each of its own in the strip. Chrome keeps a page's title
//! and address in its list of targets, `Target.getTargets`, and changes
//! them there as the page retitles itself or goes somewhere — so the thread
//! holding Chrome reads them from that list as it reads where Chrome's
//! window is, and tells the frame only when they change ([`Naming`]).
//!
//! # A stranger's string
//!
//! A title is the page's to choose. This process keeps it as text and hands
//! it on as data, and it is drawn only by the tab strip's own sandboxed
//! renderer, as a text node (ADR 0024 § 4): never shaped here, never given
//! to the window server as a window's title, never markup.
//!
//! # A reading that failed
//!
//! Says nothing new. The frame keeps showing the name it was last told, and
//! the next reading tells it again if the page was renamed meanwhile: a
//! name that is a moment old is still the page's, which a frame's position
//! is not (ADR 0062 § 3).

use crate::chrome::Failed;
use crate::chrome_protocol::{self, Command, Named};
use crate::chrome_tab::Tab;

/// What Chrome calls the page of `tab` and where it is — or [`None`] when
/// Chrome no longer holds it.
///
/// # Errors
///
/// [`Failed`], when Chrome was not asked, did not answer or answered with
/// something that is not its list of targets.
pub fn read(tab: &mut Tab<'_>) -> Result<Option<Named>, Failed> {
    let listed = tab.ask_chrome(&Command::Targets)?;
    chrome_protocol::named(&listed, tab.target()).map_err(Failed::Malformed)
}

/// What the frame was last told the page is called, so that it is told
/// only when that changes.
#[derive(Debug, Default)]
pub struct Naming {
    told: Option<Named>,
}

impl Naming {
    /// A reading came to `now`: what to tell the frame, if it is not what
    /// it was told last. A reading that named nothing tells nothing — the
    /// page gone is the frame's position's to say ([`crate::chrome_frame`]).
    pub fn heard(&mut self, now: Option<Named>) -> Option<Named> {
        let now = now?;
        if self.told.as_ref() == Some(&now) {
            return None;
        }
        self.told = Some(now.clone());
        Some(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(title: &str, address: &str) -> Named {
        Named {
            title: title.to_owned(),
            address: address.to_owned(),
        }
    }

    #[test]
    fn the_frame_is_told_a_name_only_when_it_changes() {
        let mut naming = Naming::default();
        let first = named("First", "https://example.com/");
        let second = named("Second", "https://example.com/");
        let moved = named("Second", "https://example.com/b");
        assert_eq!(naming.heard(Some(first.clone())), Some(first.clone()));
        assert_eq!(naming.heard(Some(first)), None, "the same name is not news");
        assert_eq!(naming.heard(Some(second.clone())), Some(second));
        assert_eq!(
            naming.heard(Some(moved.clone())),
            Some(moved.clone()),
            "the same title somewhere else is a different tab to show"
        );
        assert_eq!(naming.heard(None), None, "nothing named, nothing told");
        assert_eq!(naming.heard(Some(moved)), None, "and nothing forgotten");
    }
}
