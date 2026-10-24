/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Every target of a tab in Chrome, held until its requests are paused
//! (ADR 0060 § 2, queue item 633).
//!
//! A page is more than its tab. A frame from another site is a process and a
//! target of its own, and so are a dedicated worker, a service worker and a
//! shared worker; each would make requests the tab's own `Fetch` never sees.
//! So the tab's session is asked to attach to every target it starts, each
//! **waiting** before it runs (`Target.setAutoAttach`, waiting for the
//! debugger), and each attached is asked the same, and to pause every
//! request it makes, before it is let go. This keeps count of which are held
//! and what each still waits on.
//!
//! # Measured against Chrome 155
//!
//! With a local server logging every request it received: a cross-site
//! frame's `fetch` and the dedicated worker it starts, a service worker's
//! script and its `fetch` while it installs, and a dedicated worker's script
//! and `fetch` were each paused before the server saw them, and only a
//! WebSocket was not (ADR 0060 § 3).
//!
//! **A dedicated worker has no `Fetch` of its own**: `Fetch.enable` on it is
//! refused as a method Chrome does not know (`-32601`), and every request it
//! makes is paused through the document that started it. So it is held and
//! asked to hold what it starts, and is not asked to pause. Every other kind
//! that is not told to pause is a kind whose requests would escape — so a
//! refusal from one of them, or a kind of target this does not know, is not
//! let go but answered as [`Unheld`], and the tab is closed.

use crate::chrome_protocol::Declined;
use core::fmt;
use std::collections::HashMap;

/// What a held target is, by Chrome's `targetInfo.type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// The tab itself.
    Page,
    /// A frame in another process — another site's — with its own requests.
    Frame,
    /// A dedicated worker, whose requests are paused through its document.
    Worker,
    /// A service worker.
    ServiceWorker,
    /// A shared worker.
    SharedWorker,
}

impl Kind {
    /// The kind Chrome names `kind`, when it is one a tab's session is told
    /// of and this knows how to hold.
    pub fn of(kind: &str) -> Option<Self> {
        [
            Self::Frame,
            Self::Worker,
            Self::ServiceWorker,
            Self::SharedWorker,
        ]
        .into_iter()
        .find(|held| held.chrome_word() == kind)
    }

    /// Chrome's word for it.
    pub fn chrome_word(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::Frame => "iframe",
            Self::Worker => "worker",
            Self::ServiceWorker => "service_worker",
            Self::SharedWorker => "shared_worker",
        }
    }

    /// Whether its requests are paused through its own session: every kind
    /// but a dedicated worker's, measured.
    pub fn pauses_its_own(self) -> bool {
        self != Self::Worker
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Page => "the page",
            Self::Frame => "a frame",
            Self::Worker => "a worker",
            Self::ServiceWorker => "a service worker",
            Self::SharedWorker => "a shared worker",
        })
    }
}

/// A target of a tab that could not be held, so that its requests would
/// have left Chrome unseen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unheld {
    /// What it is, in Chrome's own word.
    pub kind: String,
    /// What Chrome said when it was asked to hold it, if it was asked: a
    /// kind this does not know is not asked.
    pub declined: Option<Declined>,
}

impl fmt::Display for Unheld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a {:?} of the tab could not be held, so its requests would leave unseen",
            self.kind
        )?;
        match &self.declined {
            Some(declined) => write!(f, ": {declined}"),
            None => f.write_str(": nothing here knows how to hold one"),
        }
    }
}

/// The targets of one tab held, by the session each is spoken to through.
#[derive(Debug, Default)]
pub struct Holding {
    /// Every target held, let go or not yet.
    kinds: HashMap<String, Kind>,
    /// The commands still unanswered, by their ids, and the session each
    /// was sent through.
    awaiting: HashMap<u64, String>,
}

impl Holding {
    /// `session`, a target of kind `kind`, asked by the commands `asked` to
    /// hold what it starts and pause what it asks for.
    pub fn hold(&mut self, session: &str, kind: Kind, asked: &[u64]) {
        self.kinds.insert(session.to_owned(), kind);
        for id in asked {
            self.awaiting.insert(*id, session.to_owned());
        }
    }

    /// What `session` is, if it is a target held here.
    pub fn kind(&self, session: &str) -> Option<Kind> {
        self.kinds.get(session).copied()
    }

    /// Whether `id` is a command sent to hold a target.
    pub fn awaits(&self, id: u64) -> bool {
        self.awaiting.contains_key(&id)
    }

    /// Chrome's answer to the command `id`: the session to let go once it
    /// was the last thing that target waited on.
    ///
    /// # Errors
    ///
    /// [`Unheld`], when Chrome declined it.
    pub fn answered(
        &mut self,
        id: u64,
        answer: Result<(), Declined>,
    ) -> Result<Option<String>, Unheld> {
        let Some(session) = self.awaiting.remove(&id) else {
            return Ok(None);
        };
        if let Err(declined) = answer {
            let kind = self
                .kinds
                .get(&session)
                .map_or("target", |kind| kind.chrome_word());
            return Err(Unheld {
                kind: kind.to_owned(),
                declined: Some(declined),
            });
        }
        let waiting = self.awaiting.values().any(|other| *other == session);
        Ok((!waiting).then_some(session))
    }

    /// `session` has gone: its target ended.
    pub fn forget(&mut self, session: &str) {
        self.kinds.remove(session);
        self.awaiting.retain(|_, other| other != session);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declined() -> Declined {
        Declined {
            code: -32601,
            message: "'Fetch.enable' wasn't found".to_owned(),
        }
    }

    #[test]
    fn the_kinds_chrome_names_and_only_those() {
        assert_eq!(Kind::of("iframe"), Some(Kind::Frame));
        assert_eq!(Kind::of("worker"), Some(Kind::Worker));
        assert_eq!(Kind::of("service_worker"), Some(Kind::ServiceWorker));
        assert_eq!(Kind::of("shared_worker"), Some(Kind::SharedWorker));
        for unknown in ["page", "other", "browser", "", "IFRAME", "worklet"] {
            assert_eq!(Kind::of(unknown), None, "{unknown:?}");
        }
        assert!(!Kind::Worker.pauses_its_own());
        for kind in [Kind::Frame, Kind::ServiceWorker, Kind::SharedWorker] {
            assert!(kind.pauses_its_own(), "{kind}");
        }
    }

    #[test]
    fn a_target_is_let_go_only_when_everything_it_was_asked_is_answered() {
        let mut holding = Holding::default();
        holding.hold("frame", Kind::Frame, &[3, 4]);
        holding.hold("worker", Kind::Worker, &[5]);
        assert!(holding.awaits(3) && holding.awaits(5) && !holding.awaits(6));
        assert_eq!(holding.answered(3, Ok(())), Ok(None), "4 is still owed");
        assert_eq!(holding.answered(5, Ok(())), Ok(Some("worker".to_owned())));
        assert_eq!(holding.answered(4, Ok(())), Ok(Some("frame".to_owned())));
        assert_eq!(holding.answered(4, Ok(())), Ok(None), "answered once");
        assert_eq!(holding.kind("frame"), Some(Kind::Frame));
        holding.forget("frame");
        assert_eq!(holding.kind("frame"), None);
    }

    #[test]
    fn a_target_that_would_not_be_held_is_unheld_by_name() {
        let mut holding = Holding::default();
        holding.hold("sw", Kind::ServiceWorker, &[7, 8]);
        let unheld = holding.answered(7, Err(declined()));
        assert_eq!(
            unheld,
            Err(Unheld {
                kind: "service_worker".to_owned(),
                declined: Some(declined()),
            })
        );
        assert_eq!(
            unheld.map_err(|why| why.to_string()),
            Err(
                "a \"service_worker\" of the tab could not be held, so its requests would \
                 leave unseen: Chrome declined it (-32601): 'Fetch.enable' wasn't found"
                    .to_owned()
            )
        );
    }

    #[test]
    fn a_target_that_ended_owes_nothing() {
        let mut holding = Holding::default();
        holding.hold("gone", Kind::Frame, &[1]);
        holding.forget("gone");
        assert!(!holding.awaits(1));
        assert_eq!(holding.answered(1, Ok(())), Ok(None));
    }
}
