/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo's own updates, checked on a thread of the browser process (ADR 0116
//! §§ 4 and 5, queue item 721): at launch and then every 24 hours while
//! *Settings › Updates*' switch is on, and when *Check now* is pressed.
//!
//! # Off means off
//!
//! With the switch off the thread sends nothing until *Check now* is
//! pressed: it waits on the window's word alone, and says so
//! ([`Told::Waiting`] with no next check). Turned on, it checks at once if
//! the last check is more than a day old, else a day after it.
//!
//! # What it is told, and what it tells
//!
//! The window tells it the switch and *Check now* ([`Updates`]); it tells
//! the window what each check found ([`Told::Found`]) and when it will
//! check next. Nothing here reads or answers the agent socket: no ask
//! starts, delays, refuses or reads an update (§ 4).
//!
//! # A copy that does not update itself has no thread
//!
//! [`alo_update::Updating::NotItself`] and
//! [`alo_update::Updating::FromSoftwareSource`] start nothing: no request
//! is ever made.
//!
//! # At quit
//!
//! [`install_at_quit`] is what the window calls once every card's host has
//! ended: what is ready beside the application is swapped in, and alo
//! exits — it never starts itself again (§ 4).

use alo_net::{Cause, Pool};
use alo_renderer::settings_updates::Automatic;
use alo_update::{Found, Instant, Updater};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

/// How long between two checks while the switch is on: a day.
pub const EVERY: u64 = 24 * 60 * 60;

/// What the thread tells the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Told {
    /// A check ended, `at`, and found this.
    Found {
        /// When it ended.
        at: Instant,
        /// What it found.
        found: Found,
    },
    /// It is waiting: until `next`, or, with none, for the window's word.
    Waiting {
        /// When it checks next by itself, if it will.
        next: Option<Instant>,
    },
}

/// How the thread tells the window: whether the window is still there.
pub type TellUpdates = Box<dyn Fn(Told) -> bool + Send>;

/// What the window tells the thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Work {
    /// The switch, as a person set it.
    Automatic(Automatic),
    /// *Check now* was pressed.
    CheckNow,
}

/// The thread checking for updates.
#[derive(Debug)]
pub struct Updates {
    /// Where the window's word is sent; none once finished.
    work: Option<Sender<Work>>,
    /// The thread.
    thread: Option<JoinHandle<()>>,
}

impl Updates {
    /// Start checking with `updater` over `pool`, each request caused by
    /// `cause`, the switch `automatic`, telling the window with `tell`.
    ///
    /// # Errors
    ///
    /// What the system said when no thread could be started.
    pub fn start(
        updater: Updater,
        (pool, cause): (Pool, Cause),
        automatic: Automatic,
        tell: TellUpdates,
    ) -> std::io::Result<Self> {
        let (work, waiting) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("alo-updates".to_owned())
            .spawn(move || run(&updater, (pool, &cause), automatic, &waiting, &tell))?;
        Ok(Self {
            work: Some(work),
            thread: Some(thread),
        })
    }

    /// The switch, as a person set it.
    pub fn automatic(&self, automatic: Automatic) {
        if let Some(work) = &self.work {
            let _ = work.send(Work::Automatic(automatic));
        }
    }

    /// *Check now*, pressed.
    pub fn check_now(&self) {
        if let Some(work) = &self.work {
            let _ = work.send(Work::CheckNow);
        }
    }

    /// Stop the thread and wait for it: a check under way is let finish,
    /// for it touches nothing of the installed application.
    pub fn finish(&mut self) {
        self.work = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Updates {
    fn drop(&mut self) {
        self.finish();
    }
}

/// When the next check is due, the switch `automatic`, the last check
/// ended `last` (if ever), `now`.
pub fn due(automatic: Automatic, last: Option<Instant>, now: Instant) -> Option<Instant> {
    match automatic {
        Automatic::Off => None,
        Automatic::On => Some(match last {
            Some(last) if last <= now && now.seconds() - last.seconds() < EVERY => {
                Instant::at(last.seconds() + EVERY)
            }
            _ => now,
        }),
    }
}

/// The thread: check when due, else wait for the window's word or the time.
fn run(
    updater: &Updater,
    (mut pool, cause): (Pool, &Cause),
    mut automatic: Automatic,
    waiting: &mpsc::Receiver<Work>,
    tell: &TellUpdates,
) {
    // At launch, a check: whatever the last one was, alo just started.
    let mut next = (automatic == Automatic::On).then(Instant::now);
    loop {
        let now = Instant::now();
        if next.is_some_and(|at| at <= now) {
            let found = updater.check(&mut pool, cause, now);
            if !tell(Told::Found {
                at: Instant::now(),
                found,
            }) {
                return;
            }
            next = due(automatic, Some(Instant::now()), Instant::now());
        }
        if !tell(Told::Waiting { next }) {
            return;
        }
        let word = match next {
            Some(at) => {
                let wait = at.seconds().saturating_sub(Instant::now().seconds());
                match waiting.recv_timeout(Duration::from_secs(wait)) {
                    Ok(word) => Some(word),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            None => match waiting.recv() {
                Ok(word) => Some(word),
                Err(_) => return,
            },
        };
        match word {
            Some(Work::CheckNow) => next = Some(Instant::now()),
            Some(Work::Automatic(set)) => {
                automatic = set;
                let last = updater.kept.checked().map(|(at, _)| at);
                next = due(automatic, last, Instant::now());
            }
            None => {}
        }
    }
}

/// What this alo is, as it starts: its updater, if it updates itself, or
/// what *Settings* says when it does not; and its version line.
#[derive(Debug, Clone)]
pub struct ThisAlo {
    /// Its updater, when it updates itself.
    pub updater: Option<Updater>,
    /// What *Settings* says when it does not.
    pub not_itself: Option<&'static str>,
    /// Why the build's update address or keys were refused, for the
    /// terminal, if they were.
    pub refused: Option<String>,
    /// alo's version and the Chromium it was built with, as *Settings*
    /// says them.
    pub version: String,
}

/// What the alo whose program is `program` is, keeping its updates in the
/// folder `kept` — beside the chats, outside any profile.
pub fn this_alo(program: &std::path::Path, kept: Option<std::path::PathBuf>) -> ThisAlo {
    let version = alo_update::Version::read(env!("CARGO_PKG_VERSION"))
        .unwrap_or(alo_update::Version::UNRELEASED);
    let line = format!("alo {version}, built with {}.", crate::cef_pin::VERSION);
    let bundle = crate::bundle::Bundle::around(program);
    let resources = bundle.as_ref().map(crate::bundle::Bundle::resources);
    let updating = alo_update::given::read(
        resources,
        version,
        alo_update::platform::this(),
        std::path::Path::new("/"),
    );
    let not_itself = updating.said();
    let (updater, refused) = match (updating, resources, kept) {
        (alo_update::Updating::Itself { address, keys }, Some(resources), Some(kept)) => {
            let application = resources.parent().and_then(std::path::Path::parent);
            let updater = application.map(|application| Updater {
                address,
                keys,
                kept: alo_update::kept::Kept::at(kept),
                application: application.to_path_buf(),
                version,
                platform: alo_update::platform::this().unwrap_or_default().to_owned(),
                piece: alo_update::package::PIECE,
            });
            (updater, None)
        }
        (alo_update::Updating::NotItself { why }, _, _) => (None, why),
        _ => (None, None),
    };
    ThisAlo {
        not_itself: if updater.is_some() {
            None
        } else {
            not_itself.or(Some(alo_update::given::NOT_ITSELF))
        },
        updater,
        refused,
        version: line,
    }
}

/// At launch, before any Chromium starts: what an earlier swap left beside
/// the application removed (ADR 0116 § 3). Whether anything was.
pub fn swept(updater: &Updater) -> bool {
    alo_update::stage::sweep(&updater.application)
}

/// Install what is ready beside the application at quit, once every card's
/// host has ended: what happened, in a sentence for the terminal, if
/// anything did.
pub fn install_at_quit(updater: &Updater) -> Option<String> {
    match alo_update::quitting::install(&updater.application, updater.version) {
        alo_update::quitting::Installed::Nothing => None,
        alo_update::quitting::Installed::Swapped(version) => Some(format!(
            "alo {version} is installed, and opens the next time alo is opened"
        )),
        alo_update::quitting::Installed::Refused(why) => Some(why),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: u64 = EVERY;

    /// ADR 0116 § 4: the agent socket has no part in it — no ask starts,
    /// delays, refuses or reads an update, so every ask that would is
    /// refused as an ask the socket does not have.
    #[test]
    fn the_agent_socket_has_no_ask_that_reaches_an_update() {
        for ask in [
            "update",
            "updates",
            "check_for_updates",
            "check_now",
            "install_update",
            "quit_and_update",
            "quit",
        ] {
            let line = format!(r#"{{"v":1,"id":1,"ask":"{ask}"}}"#);
            let request = crate::protocol::read(line.as_bytes());
            assert!(request.ask.is_err(), "{ask}: {request:?}");
        }
    }

    #[test]
    fn off_is_never_due_and_on_is_due_a_day_after_the_last_check_or_at_once() {
        let now = Instant::at(10 * DAY);
        assert_eq!(due(Automatic::Off, None, now), None);
        assert_eq!(due(Automatic::Off, Some(Instant::at(0)), now), None);
        assert_eq!(due(Automatic::On, None, now), Some(now));
        assert_eq!(
            due(Automatic::On, Some(Instant::at(9 * DAY + 60)), now),
            Some(Instant::at(10 * DAY + 60))
        );
        assert_eq!(
            due(Automatic::On, Some(Instant::at(9 * DAY)), now),
            Some(now)
        );
        // A last check in the future (a clock moved back) is no reason to wait.
        assert_eq!(
            due(Automatic::On, Some(Instant::at(11 * DAY)), now),
            Some(now)
        );
    }
}
