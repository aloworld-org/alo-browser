/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo's own updates in the window (ADR 0116 §§ 4 and 5, queue item 721):
//! *Settings › Updates* told what this copy is and what the last check
//! found; the switch and *Check now* handed to the thread checking
//! ([`crate::conversation_updates`]); an update ready said in the first
//! screen's line, the window's own, never inside a page; the first screen
//! saying once that the check is on; and what is ready swapped in at quit,
//! after every card's host has ended.

use super::{ConversationWindow, Heard};
use crate::conversation_updates::{ThisAlo, Told, Updates, install_at_quit};
use alo_renderer::settings_updates::{Automatic, UpdatesShown, UpdatesState, bounded};
use alo_update::{Found, Instant, Updater};

/// What the first screen says once, the first time alo starts with the
/// check on (ADR 0116 § 5).
pub const CHECKS_ONCE_A_DAY: &str = "alo checks for security updates once a day. It sends \
     nothing about you. Settings \u{203a} Updates turns it off.";

/// The file in the updater's folder whose presence says the first screen
/// has said [`CHECKS_ONCE_A_DAY`].
const TOLD: &str = "told";

/// The window's updates: whether this copy updates itself, the thread
/// checking, and what it last found.
#[derive(Debug, Default)]
pub(crate) struct Updating {
    /// The updater, when this copy updates itself.
    updater: Option<Updater>,
    /// The thread checking, while there is one.
    thread: Option<Updates>,
    /// alo's version and its Chromium's, as *Settings* says them.
    version: String,
}

impl ConversationWindow {
    /// The same window, this alo as it started — whether it updates
    /// itself, and if not why, and its version — and the thread checking,
    /// in a copy that does (`updates`).
    #[must_use]
    pub fn with_updates(mut self, (this_alo, thread): (ThisAlo, Option<Updates>)) -> Self {
        let ThisAlo {
            updater,
            not_itself,
            refused,
            version,
        } = this_alo;
        if let Some(why) = refused {
            self.tell(format!("this copy of alo does not update itself: {why}"));
        }
        let checked = updater
            .as_ref()
            .map(|updater| checked_line(updater.kept.checked()));
        let state = match (checked, not_itself) {
            (Some(checked), _) => UpdatesState::Itself {
                automatic: self.conversation.settings().updates(),
                checked,
            },
            (None, said) => {
                UpdatesState::Not(said.unwrap_or(alo_update::given::NOT_ITSELF).to_owned())
            }
        };
        let _ = self.conversation.show_updates(UpdatesShown {
            version: bounded(&version),
            state,
        });
        // Said once, the first time a copy that checks starts with it on.
        if let Some(updater) = &updater
            && self.conversation.settings().updates() == Automatic::On
            && !updater.kept.folder().join(TOLD).exists()
            && updater.kept.made().is_ok()
            && std::fs::write(updater.kept.folder().join(TOLD), "").is_ok()
        {
            self.tell(CHECKS_ONCE_A_DAY.to_owned());
        }
        self.updating = Updating {
            updater,
            thread,
            version,
        };
        self
    }

    /// What the thread checking said.
    pub(super) fn updates_told(&mut self, told: Told) {
        let Told::Found { at, found } = told else {
            return;
        };
        if let Some(line) = found.ready_line() {
            self.tell(line);
        } else if !matches!(found, Found::UpToDate { .. }) {
            self.said.push(found.said());
        }
        let shown = UpdatesShown {
            version: bounded(&self.updating.version),
            state: UpdatesState::Itself {
                automatic: self.conversation.settings().updates(),
                checked: bounded(&checked_line(Some((at, found.said())))),
            },
        };
        if self.conversation.show_updates(shown) {
            self.redraw = true;
        }
    }

    /// The switch a person just set, if they did, kept and handed to the
    /// thread; *Check now*, if pressed, handed to it.
    pub(super) fn updates_chosen(&mut self, changed: bool) {
        if changed && let Some(thread) = &self.updating.thread {
            thread.automatic(self.conversation.settings().updates());
        }
        if self.conversation.take_check_now()
            && let Some(thread) = &self.updating.thread
        {
            thread.check_now();
        }
    }

    /// The thread stopped, and what is ready installed — once every card's
    /// host has ended, as alo quits. alo is never started again by this.
    pub(super) fn updates_finished(&mut self) {
        if let Some(mut thread) = self.updating.thread.take() {
            thread.finish();
        }
        if let Some(updater) = &self.updating.updater
            && let Some(said) = install_at_quit(updater)
        {
            self.said.push(said);
        }
    }
}

/// When the last check ended and what it found, as *Settings* says it.
pub fn checked_line(checked: Option<(Instant, String)>) -> String {
    let Some((at, found)) = checked else {
        return "Not checked yet.".to_owned();
    };
    let written = at.to_string();
    let piece = |from: usize, to: usize| written.get(from..to).unwrap_or("");
    let month = match piece(5, 7) {
        "01" => "January",
        "02" => "February",
        "03" => "March",
        "04" => "April",
        "05" => "May",
        "06" => "June",
        "07" => "July",
        "08" => "August",
        "09" => "September",
        "10" => "October",
        "11" => "November",
        _ => "December",
    };
    format!(
        "{} {month} {} at {} UTC: {found}",
        piece(8, 10).trim_start_matches('0'),
        piece(0, 4),
        piece(11, 16)
    )
}

impl Heard {
    /// What the thread checking for updates said, as the window hears it.
    pub fn updates(told: Told) -> Self {
        Heard::Updates(told)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_check_is_said_in_words_and_none_as_not_yet() {
        assert_eq!(checked_line(None), "Not checked yet.");
        assert_eq!(
            checked_line(Some((
                Instant::at(1_794_646_800),
                "alo 0.4.1 is up to date.".into()
            ))),
            "14 November 2026 at 09:00 UTC: alo 0.4.1 is up to date."
        );
        assert_eq!(
            checked_line(Some((Instant::at(0), "x".into()))),
            "1 January 1970 at 00:00 UTC: x"
        );
    }
}
