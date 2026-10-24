/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Every request of a page in Chrome, in the record, with the cause this
//! process gave it (ADR 0060 § 2, ADR 0012, queue item 634).
//!
//! [`crate::chrome_tab`] decides each request a page from the network makes
//! before Chrome sends it. This is where the decision becomes a line of
//! `alo-net`'s [`Activity`] — the same record, the same six fields, the same
//! bounds as the engine's — and where each line's [`Cause`] is chosen.
//!
//! # A cause is what this process was doing, never what Chrome says
//!
//! ADR 0012 § 4: a renderer may say what it **wants** and never **who
//! wanted it**. Chrome is the renderer here, so what it says of a request is
//! its address, its method, its kind and its frame, and nothing it says
//! names a cause. The cause comes from [`Causes`], which knows what this
//! process did to the tab:
//!
//! - **The tab's first page is the person's** ([`Cause::Person`]): this
//!   process opened it ([`crate::chrome_tab::Tab::open`]).
//! - **While a verb is being carried out, the tab's requests are the
//!   agent's** ([`Cause::Agent`]), by the action minted when the verb was
//!   accepted ([`crate::chrome_act`]), in the document it acted in.
//! - **A navigation the page's own frame asked for while a verb was carried
//!   out is the agent's too**, even when Chrome pauses its first hop after
//!   the verb has answered: a click's navigation is told of in the click's
//!   task (`Page.frameRequestedNavigation`, measured in item 621), and is
//!   kept against the address it asked for. Only a first hop to *that*
//!   address takes the agent's cause, so a navigation that never came is
//!   never pinned on the next one that does.
//! - **Every other request is the document's** ([`Cause::Document`]): the
//!   page the tab's own frame holds now.
//!
//! # A document is minted at its first hop, and held from its commit
//!
//! A navigation of the tab's own frame is a `Document` request paused in the
//! tab with the tab's id as its frame. Its first hop mints a document with
//! the navigation's cause ([`Documents::opened`]: minting and recording are
//! one act); a hop a redirect led to carries the same cause and mints
//! nothing, which is ADR 0012 § 2's *a redirect to whatever asked for the
//! first hop*. Chrome tells of the frame's commit (`Page.frameNavigated`)
//! after the last hop and before anything the new page asks for, measured
//! against Chrome 155; from then the new document is the tab's, and until
//! then the old one is — a request the old page makes as it is left is the
//! old page's.
//!
//! # What this does not say yet
//!
//! - **A frame is not a document of its own here.** A frame's requests, and
//!   a worker's, are the tab's document's: the page that holds the frame or
//!   started the worker. Each frame minting its own document, caused by the
//!   one holding it, is queue item 636.
//! - **What came back is Chrome's.** A request let go is a line saying so
//!   ([`Happened::LetGo`]), never a status this browser did not read.
//! - **A frozen page is not recorded**: it reaches no network, and what it
//!   asked for is [`crate::chrome_tab::Tab::refused`].
//! - **A tab an agent asked to open** ([`crate::chrome_tab::Tab::open_for_agent`])
//!   has its first page recorded as the person's, whose tab it is: ADR
//!   0012's causes have no place for an agent opening one, and which it
//!   should be is queue item 638's decision.

use alo_net::activity::{Activity, Happened};
use alo_net::cause::{ActionId, Cause, DocumentId, Identities, TabId};
use alo_net::chain::Documents;
use alo_net::request::{Purpose, Request};
use alo_url::Url;
use std::time::SystemTime;

/// What the tabs of one Chrome asked for, and what caused each.
///
/// One per [`crate::chrome::Chrome`], held on this side of the pipe: the
/// identities it mints, the documents those name, and the session's
/// [`Activity`]. Nothing Chrome or a page sends can write to it except as a
/// request this process decided.
#[derive(Debug, Default)]
pub struct Record {
    identities: Identities,
    documents: Documents,
    activity: Activity,
}

impl Record {
    /// Every line written, oldest first, as `alo-net` keeps them.
    pub fn activity(&self) -> &Activity {
        &self.activity
    }

    /// What caused each document's load, which a line's chain is walked
    /// against ([`alo_net::activity::Entry::chain`]).
    pub fn documents(&self) -> &Documents {
        &self.documents
    }

    /// A tab nothing has ever been.
    pub(crate) fn a_tab(&mut self) -> TabId {
        self.identities.a_tab()
    }

    /// An action nothing has ever been: a verb accepted.
    pub(crate) fn an_action(&mut self) -> ActionId {
        self.identities.an_action()
    }

    /// A document whose load `cause` caused.
    fn opened(&mut self, cause: Cause) -> DocumentId {
        self.documents.opened(&mut self.identities, cause)
    }

    /// Write down that `method` `url`, of kind `purpose`, was asked for, by
    /// `cause`, and what became of it.
    pub(crate) fn write(
        &mut self,
        url: Url,
        method: &str,
        purpose: Purpose,
        cause: Cause,
        happened: Happened,
    ) {
        let request = Request {
            method: method.to_owned(),
            ..Request::get(url, cause)
        }
        .for_purpose(purpose);
        self.activity
            .happened(&request, SystemTime::now(), happened);
    }
}

/// What kind of thing a request Chrome paused is, from Chrome's word for
/// it: `alo-net`'s purpose when it has the same one, and
/// [`Purpose::Unnamed`] when it has none.
///
/// `Fetch` and `XHR` are both a script's fetch, as they are in this engine;
/// `Ping` is a beacon, as `navigator.sendBeacon` sends one. A font, a
/// worker's script (Chrome's `Other`), a manifest and the rest have no
/// purpose of this engine's, and a nearest one would be a word nobody said.
pub fn purpose_of(kind: &str) -> Purpose {
    match kind {
        "Document" => Purpose::Document,
        "Stylesheet" => Purpose::Style,
        "Image" => Purpose::Image,
        "Script" => Purpose::Script,
        "Fetch" | "XHR" => Purpose::Fetch,
        "Ping" => Purpose::Beacon,
        "CSPViolationReport" => Purpose::Report,
        _ => Purpose::Unnamed,
    }
}

/// Where a request a tab paused falls in a navigation of the tab's own
/// frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hop {
    /// Not one: anything a page asks for.
    None,
    /// A navigation's first hop.
    First,
    /// A hop a redirect led to.
    Redirected,
}

/// What caused what, in one tab in Chrome.
#[derive(Debug)]
pub struct Causes {
    tab: TabId,
    /// The document the tab's own frame holds, once one has committed.
    page: Option<DocumentId>,
    /// The document a navigation under way will commit, minted at its first
    /// hop.
    arriving: Option<DocumentId>,
    /// The cause of the navigation under way, which every hop carries.
    hop: Option<Cause>,
    /// A navigation asked for while a verb was carried out: where to,
    /// without its fragment, and the agent's cause.
    asked: Option<(String, Cause)>,
    /// The verb being carried out.
    acting: Option<ActionId>,
}

impl Causes {
    /// A tab this process opened, `tab`: its first page is the person's.
    pub fn of(tab: TabId) -> Self {
        Self {
            tab,
            page: None,
            arriving: None,
            hop: None,
            asked: None,
            acting: None,
        }
    }

    /// The cause of a request asked for at `url`, which falls in a
    /// navigation of the tab's own frame as `hop` says; a first hop mints
    /// the document the navigation will commit, in `record`.
    pub fn of_request(&mut self, record: &mut Record, url: &str, hop: Hop) -> Cause {
        match (hop, &self.hop) {
            (Hop::None, _) => self.of_page(),
            (Hop::Redirected, Some(cause)) => cause.clone(),
            // A hop a redirect led to, of a navigation whose first hop this
            // tab never paused, is taken as a first.
            (Hop::First | Hop::Redirected, _) => {
                let cause = match self.asked.take() {
                    Some((to, cause)) if to == without_fragment(url) => cause,
                    _ => self.of_navigation(),
                };
                self.hop = Some(cause.clone());
                self.arriving = Some(record.opened(cause.clone()));
                cause
            }
        }
    }

    /// The cause of what the page asks for, now: the agent's while a verb is
    /// carried out, and otherwise the page's.
    pub fn of_page(&self) -> Cause {
        match (self.acting, self.page) {
            (Some(action), Some(document)) => Cause::Agent { action, document },
            (None, Some(document)) => Cause::Document { document },
            // Before the tab's first page has committed, nothing else has
            // asked for anything: whatever is asked is the first page's.
            (_, None) => self
                .arriving
                .map_or(Cause::Person { tab: self.tab }, |document| {
                    Cause::Document { document }
                }),
        }
    }

    /// The tab's own frame asked to go to `url`: while a verb is carried
    /// out, that navigation is the agent's.
    pub fn asked_to_go(&mut self, url: &str) {
        if let (Some(action), Some(document)) = (self.acting, self.page) {
            self.asked = Some((
                without_fragment(url).to_owned(),
                Cause::Agent { action, document },
            ));
        }
    }

    /// The tab's own frame committed a new document: the one its navigation
    /// minted, or — for a document that came of no request this tab paused,
    /// as `about:blank` does — one minted now, with the navigation's cause.
    pub fn committed(&mut self, record: &mut Record) {
        let document = self
            .arriving
            .take()
            .unwrap_or_else(|| record.opened(self.of_navigation()));
        self.page = Some(document);
        self.hop = None;
    }

    /// The navigation under way went no further: a hop of it was refused.
    /// Its document was caused and minted, and is never the tab's.
    pub fn ended(&mut self) {
        self.arriving = None;
        self.hop = None;
    }

    /// `action` is being carried out on the tab, or [`None`] once it has
    /// been.
    pub fn acting(&mut self, action: Option<ActionId>) {
        self.acting = action;
    }

    /// The cause of a navigation nothing asked for while a verb was carried
    /// out: the agent's while one is, the page's when it holds one, and the
    /// person's for the tab's first.
    fn of_navigation(&self) -> Cause {
        match (self.acting, self.page) {
            (Some(action), Some(document)) => Cause::Agent { action, document },
            (None, Some(document)) => Cause::Document { document },
            (_, None) => Cause::Person { tab: self.tab },
        }
    }
}

/// An address without its fragment, which a request Chrome pauses never
/// carries.
fn without_fragment(url: &str) -> &str {
    url.split_once('#').map_or(url, |(before, _)| before)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_net::chain::End;

    fn url(text: &str) -> Url {
        alo_url::parse(text).unwrap()
    }

    #[test]
    fn chromes_kinds_are_this_engines_purposes_or_unnamed() {
        for (kind, purpose) in [
            ("Document", Purpose::Document),
            ("Stylesheet", Purpose::Style),
            ("Image", Purpose::Image),
            ("Script", Purpose::Script),
            ("Fetch", Purpose::Fetch),
            ("XHR", Purpose::Fetch),
            ("Ping", Purpose::Beacon),
            ("CSPViolationReport", Purpose::Report),
        ] {
            assert_eq!(purpose_of(kind), purpose, "{kind}");
        }
        for kind in [
            "Font", "Media", "Other", "Manifest", "Prefetch", "", "document",
        ] {
            assert_eq!(purpose_of(kind), Purpose::Unnamed, "{kind}");
        }
    }

    #[test]
    fn the_first_page_is_the_persons_and_what_it_loads_is_its_own() {
        let mut record = Record::default();
        let tab = record.a_tab();
        let mut causes = Causes::of(tab);
        let first = causes.of_request(&mut record, "http://a/start", Hop::First);
        assert_eq!(first, Cause::Person { tab });
        let hop = causes.of_request(&mut record, "http://b/page", Hop::Redirected);
        assert_eq!(hop, first, "a redirect is whatever asked for the first hop");
        assert_eq!(record.documents().remembered(), 1, "one document, not two");
        causes.committed(&mut record);
        let Cause::Document { document } =
            causes.of_request(&mut record, "http://b/s.css", Hop::None)
        else {
            panic!("a page's sheet is not the page's");
        };
        assert_eq!(
            record.documents().cause_of(document),
            Some(&Cause::Person { tab })
        );
        let chain = record.documents().chain(&Cause::Document { document });
        assert_eq!(chain.person(), Some(tab));
        assert_eq!(chain.end(), End::Person(tab));
    }

    #[test]
    fn a_navigation_asked_for_while_acting_is_the_agents_after_the_verb_has_answered() {
        let mut record = Record::default();
        let mut causes = Causes::of(record.a_tab());
        causes.of_request(&mut record, "http://a/", Hop::First);
        causes.committed(&mut record);
        let action = record.an_action();
        causes.acting(Some(action));
        let during = causes.of_request(&mut record, "http://a/f", Hop::None);
        assert!(matches!(during, Cause::Agent { action: acted, .. } if acted == action));
        causes.asked_to_go("http://a/next#there");
        causes.acting(None);
        assert!(
            matches!(causes.of_page(), Cause::Document { .. }),
            "the verb has answered"
        );
        let going = causes.of_request(&mut record, "http://a/next", Hop::First);
        assert!(matches!(going, Cause::Agent { action: acted, .. } if acted == action));
        causes.committed(&mut record);
        let next = causes.of_request(&mut record, "http://a/next.css", Hop::None);
        assert!(record.documents().chain(&next).followed_from(action));
    }

    #[test]
    fn a_navigation_the_verb_asked_for_and_never_made_is_not_pinned_on_the_next() {
        let mut record = Record::default();
        let mut causes = Causes::of(record.a_tab());
        causes.of_request(&mut record, "http://a/", Hop::First);
        causes.committed(&mut record);
        let page = causes.of_page();
        let action = record.an_action();
        causes.acting(Some(action));
        causes.asked_to_go("http://a/never");
        causes.acting(None);
        let elsewhere = causes.of_request(&mut record, "http://a/later", Hop::First);
        assert_eq!(elsewhere, page, "the page went on its own");
        // And the ask is spent, so it is not waiting for the next.
        causes.committed(&mut record);
        let again = causes.of_request(&mut record, "http://a/never", Hop::First);
        assert!(matches!(again, Cause::Document { .. }), "{again}");
    }

    #[test]
    fn what_the_old_page_asks_for_before_the_new_one_commits_is_the_old_pages() {
        let mut record = Record::default();
        let mut causes = Causes::of(record.a_tab());
        causes.of_request(&mut record, "http://a/", Hop::First);
        causes.committed(&mut record);
        let old = causes.of_page();
        causes.of_request(&mut record, "http://a/next", Hop::First);
        assert_eq!(
            causes.of_request(&mut record, "http://a/beacon", Hop::None),
            old
        );
        causes.committed(&mut record);
        assert_ne!(causes.of_page(), old);
    }

    #[test]
    fn a_navigation_refused_at_a_hop_never_becomes_the_page() {
        let mut record = Record::default();
        let mut causes = Causes::of(record.a_tab());
        causes.of_request(&mut record, "http://a/", Hop::First);
        causes.committed(&mut record);
        let page = causes.of_page();
        causes.of_request(&mut record, "http://a/away", Hop::First);
        causes.ended();
        assert_eq!(record.documents().remembered(), 2, "caused, and minted");
        assert_eq!(causes.of_page(), page);
        // A later commit — Chrome's own error page, say — is not the
        // refused navigation's document.
        causes.committed(&mut record);
        assert_eq!(record.documents().remembered(), 3);
    }

    #[test]
    fn a_document_from_no_request_is_minted_as_it_commits() {
        let mut record = Record::default();
        let tab = record.a_tab();
        let mut causes = Causes::of(tab);
        causes.committed(&mut record);
        assert_eq!(record.documents().remembered(), 1);
        let Cause::Document { document } = causes.of_page() else {
            panic!("no page");
        };
        assert_eq!(
            record.documents().cause_of(document),
            Some(&Cause::Person { tab })
        );
    }

    #[test]
    fn a_line_holds_what_this_process_gave_it() {
        let mut record = Record::default();
        let tab = record.a_tab();
        record.write(
            url("http://a/f"),
            "POST",
            Purpose::Fetch,
            Cause::Person { tab },
            Happened::LetGo,
        );
        let line = record.activity().latest().unwrap();
        assert_eq!(line.method(), "POST");
        assert_eq!(line.purpose(), &Purpose::Fetch);
        assert_eq!(line.happened(), &Happened::LetGo);
        assert_eq!(
            line.to_string(),
            format!(
                "POST http://a/f (fetch), caused by the person, in {tab} — let go, to a network \
                 that is not this engine's"
            )
        );
    }
}
