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
//!
//! **A shared worker is not told of through the tab at all**, only to
//! Chrome's own session, which `chrome_tab.rs` asks to hold new targets
//! too. Measured against Chrome 155 on 24 October (item 637): a page's
//! `new SharedWorker` has its script paused through the page, and the
//! worker is attached to Chrome's session alone, waiting. Held there, it
//! answers `Fetch.enable`, `Target.setAutoAttach` and `Network.enable`
//! before it runs, asks for nothing until it is let go, and then its
//! `fetch`, its `importScripts` and what it asks for as a page connects are
//! each paused in its own target, and its WebSocket is told of there.
//! `Worker` is undefined inside it, so it starts no target of its own.
//! Left waiting, it never runs at all. Without Chrome's session holding new
//! targets — a probe of item 634's — its `fetch` reached the server
//! unpaused.
//!
//! # A service worker stops and starts again in the target it had
//!
//! Measured against Chrome 155 on 25 October (item 655), from a Node CDP
//! probe holding a service worker as the tab does: one stopped — by
//! `ServiceWorker.stopWorker`, `ServiceWorker.stopAllWorkers` or a
//! `Target.closeTarget` of its own target, alike — keeps its target and its
//! session, and Chrome tells of the stop there as `Inspector.targetCrashed`.
//! Nothing sent to it is answered while it is stopped. When the page next
//! needs it, it starts again in the same session, told of as
//! `Inspector.targetReloadedAfterCrash`, with no new attach — and
//! **waiting**, its script not run and the page's request through it
//! waiting with it, until it is let go there. Started again, it answers
//! what it is asked within a millisecond, before it runs, where at its
//! first start it answers only once it runs; its requests were still
//! paused and its sockets still told of, though nothing was asked of it
//! again. An update of its script is not this: the new version is a new
//! target, attached and held as any other, and the old one's ends.
//!
//! So a stopped one is [`Holding::stopped`] — a verb neither waits on it
//! nor hears it — and one started again is held again
//! ([`Holding::hold_again`]) and let go once it has answered. Held, an idle
//! one was not stopped by Chrome in forty seconds (item 654); what memory
//! or a crash of its process does is not measured.

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

    /// Whether it answers what it is asked while it is held, before it runs:
    /// a frame, a dedicated worker and a shared worker do; a service worker
    /// answers `Network.enable` only once it runs, measured against Chrome
    /// 155.
    pub fn answers_before_it_runs(self) -> bool {
        self != Self::ServiceWorker
    }

    /// Whether its requests are paused through its own session: every kind
    /// but a dedicated worker's, measured.
    pub fn pauses_its_own(self) -> bool {
        self != Self::Worker
    }

    /// Whether what its script says while a verb runs is heard as the
    /// page's, through its own session: every kind held is — a frame and a
    /// dedicated worker (queue item 652), and a shared and a service worker
    /// (item 654).
    ///
    /// A shared worker's script is every page's that connects to it, and a
    /// service worker's every page's it controls; here those are all this
    /// tab's, because a tab is the one open in its Chrome and a tab its
    /// page opens is closed before it loads anything (`chrome_tab.rs`). So
    /// what either says while a verb runs is the page's script speaking,
    /// as a dedicated worker's is.
    pub fn is_heard(self) -> bool {
        self != Self::Page
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
    kinds: HashMap<String, Held>,
    /// The commands still unanswered, by their ids, and the session each
    /// was sent through.
    awaiting: HashMap<u64, String>,
}

/// One target held.
#[derive(Debug)]
struct Held {
    kind: Kind,
    /// Its target, by Chrome's id.
    target: String,
    /// Whether it has stopped, and answers nothing until it starts again.
    stopped: bool,
}

impl Holding {
    /// `session`, the target `target` of kind `kind`, asked by the commands
    /// `asked` to hold what it starts and pause what it asks for.
    pub fn hold(&mut self, session: &str, target: &str, kind: Kind, asked: &[u64]) {
        self.kinds.insert(
            session.to_owned(),
            Held {
                kind,
                target: target.to_owned(),
                stopped: false,
            },
        );
        self.wait_on(session, asked);
    }

    /// `session`, a target held here that started again, asked again by
    /// `asked`: it runs, and is let go once they are answered. `false`,
    /// and nothing remembered, for a session not held here.
    pub fn hold_again(&mut self, session: &str, asked: &[u64]) -> bool {
        let Some(held) = self.kinds.get_mut(session) else {
            return false;
        };
        held.stopped = false;
        self.wait_on(session, asked);
        true
    }

    fn wait_on(&mut self, session: &str, asked: &[u64]) {
        for id in asked {
            self.awaiting.insert(*id, session.to_owned());
        }
    }

    /// `session`, a target held here, has stopped: until it is held again,
    /// it answers nothing, so nothing waits on it.
    pub fn stopped(&mut self, session: &str) {
        if let Some(held) = self.kinds.get_mut(session) {
            held.stopped = true;
        }
    }

    /// Whether `session` is a target held here that has stopped.
    pub fn is_stopped(&self, session: &str) -> bool {
        self.kinds.get(session).is_some_and(|held| held.stopped)
    }

    /// What `session` is, if it is a target held here.
    pub fn kind(&self, session: &str) -> Option<Kind> {
        self.kinds.get(session).map(|held| held.kind)
    }

    /// The session of every target held whose script is heard while a verb
    /// runs ([`Kind::is_heard`]) and that has not stopped, in a fixed order.
    pub fn heard(&self) -> Vec<String> {
        let mut heard: Vec<String> = self
            .kinds
            .iter()
            .filter(|(_, held)| held.kind.is_heard() && !held.stopped)
            .map(|(session, _)| session.clone())
            .collect();
        heard.sort();
        heard
    }

    /// The target of every one held of kind `kind`, by Chrome's id, in a
    /// fixed order.
    pub fn targets(&self, kind: Kind) -> Vec<String> {
        let mut targets: Vec<String> = self
            .kinds
            .values()
            .filter(|held| held.kind == kind)
            .map(|held| held.target.clone())
            .collect();
        targets.sort();
        targets
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
                .map_or("target", |held| held.kind.chrome_word());
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
        for kind in [Kind::Page, Kind::Frame, Kind::Worker, Kind::SharedWorker] {
            assert!(kind.answers_before_it_runs(), "{kind}");
        }
        assert!(!Kind::ServiceWorker.answers_before_it_runs());
        for kind in [Kind::Frame, Kind::ServiceWorker, Kind::SharedWorker] {
            assert!(kind.pauses_its_own(), "{kind}");
        }
        for kind in [
            Kind::Frame,
            Kind::Worker,
            Kind::ServiceWorker,
            Kind::SharedWorker,
        ] {
            assert!(kind.is_heard(), "{kind}");
        }
        assert!(!Kind::Page.is_heard());
    }

    #[test]
    fn every_target_held_is_heard_and_only_while_held() {
        let mut holding = Holding::default();
        holding.hold("w", "W", Kind::Worker, &[]);
        holding.hold("sw", "SW", Kind::ServiceWorker, &[]);
        holding.hold("f", "F", Kind::Frame, &[1]);
        holding.hold("shared", "S", Kind::SharedWorker, &[]);
        assert_eq!(holding.heard(), ["f", "shared", "sw", "w"]);
        holding.forget("f");
        holding.forget("sw");
        assert_eq!(holding.heard(), ["shared", "w"]);
    }

    #[test]
    fn a_target_is_let_go_only_when_everything_it_was_asked_is_answered() {
        let mut holding = Holding::default();
        holding.hold("frame", "F", Kind::Frame, &[3, 4]);
        holding.hold("worker", "W", Kind::Worker, &[5]);
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
        holding.hold("sw", "SW", Kind::ServiceWorker, &[7, 8]);
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
    fn a_stopped_target_is_not_heard_until_it_is_held_again() {
        let mut holding = Holding::default();
        holding.hold("sw", "SW", Kind::ServiceWorker, &[]);
        holding.hold("f", "F", Kind::Frame, &[]);
        holding.stopped("sw");
        assert!(holding.is_stopped("sw") && !holding.is_stopped("f"));
        assert_eq!(holding.heard(), ["f"]);
        assert_eq!(holding.kind("sw"), Some(Kind::ServiceWorker), "still held");
        assert_eq!(holding.targets(Kind::ServiceWorker), ["SW"]);

        assert!(holding.hold_again("sw", &[9, 10]));
        assert!(!holding.is_stopped("sw"));
        assert_eq!(holding.heard(), ["f", "sw"]);
        assert_eq!(holding.answered(9, Ok(())), Ok(None), "10 is still owed");
        assert_eq!(holding.answered(10, Ok(())), Ok(Some("sw".to_owned())));
    }

    #[test]
    fn only_a_target_held_is_stopped_or_held_again() {
        let mut holding = Holding::default();
        holding.stopped("never");
        assert!(!holding.is_stopped("never"));
        assert!(!holding.hold_again("never", &[1]));
        assert!(!holding.awaits(1));
        holding.hold("sw", "SW", Kind::ServiceWorker, &[]);
        holding.stopped("sw");
        holding.forget("sw");
        assert!(!holding.is_stopped("sw") && !holding.hold_again("sw", &[2]));
        assert!(holding.targets(Kind::ServiceWorker).is_empty());
    }

    #[test]
    fn a_target_that_ended_owes_nothing() {
        let mut holding = Holding::default();
        holding.hold("gone", "G", Kind::Frame, &[1]);
        holding.forget("gone");
        assert!(!holding.awaits(1));
        assert_eq!(holding.answered(1, Ok(())), Ok(None));
    }
}
