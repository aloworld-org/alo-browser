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
//!   process opened it ([`crate::chrome_tab::Tab::open`]) — unless an agent
//!   asked for the tab ([`crate::chrome_tab::Tab::open_for_agent`]), and
//!   then it is the agent's, by the action minted when the ask was
//!   accepted, done in the tab rather than in a document ([`Place::Tab`],
//!   ADR 0063). So a page in such a tab, and everything it asks for, leads
//!   back to that action and not to the person.
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
//! after the last hop; from then the new document is the tab's, and until
//! then the old one is — a request the old page makes as it is left is the
//! old page's.
//!
//! **Not always before what the new page asks for.** Chrome pauses a request
//! in its browser process and tells of a commit from the page's renderer, so
//! under load the new page's stylesheet is paused before its commit is read
//! (queue item 647, measured against Chrome 155). So the first hop's
//! `networkId`, which is the loader of the document it commits, is kept
//! with the document arriving ([`Causes::window`]), and a request paused in
//! that window is decided by its own loader once Chrome tells of it
//! ([`crate::chrome_owed`]).
//!
//! # A frame is a document of its own
//!
//! Each frame of the tab is held the same way (queue item 636): a navigation
//! of a frame is a `Document` request paused for that frame, through the
//! tab's session or the frame's own target, and its first hop mints the
//! frame's document — caused by what the document holding the frame was
//! doing, or, for a frame that already holds one, by that one, as a page's
//! own navigation is the page's. Chrome tells of a frame being made
//! (`Page.frameAttached`) before it asks for anything, and of its commit
//! before what its document asks for: a same-site frame's through the tab's
//! session, an other-site frame's through its own target, once that target
//! is asked to tell of it (`Page.enable`) — measured against Chrome 155 on
//! 25 October 2026. So the chain of a frame's `fetch` is the frame's
//! document, then the page's, then the person.
//!
//! # What this does not say yet
//!
//! - **A service worker's and a shared worker's requests** are named for no
//!   frame of the tab's, and so are the tab's own frame's: the page's.
//! - **A frame's navigation to a scheme Chrome hands elsewhere** is not
//!   judged as the tab's own frame's is.
//! - **What came back is Chrome's.** A request let go is a line saying so
//!   ([`Happened::LetGo`]), never a status this browser did not read.
//! - **A frozen page is not recorded**: it reaches no network, and what it
//!   asked for is [`crate::chrome_tab::Tab::refused`].

use alo_net::activity::{Activity, Happened};
use alo_net::cause::{ActionId, Cause, DocumentId, Identities, Place, TabId};
use alo_net::chain::Documents;
use alo_net::request::{Purpose, Request};
use alo_url::Url;
use std::collections::{HashMap, HashSet};
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

/// The most frames of one tab whose documents are remembered at once.
///
/// **Measured rather than chosen**: Chrome 155 makes at most 1000 frames in
/// one page — a page that adds 1200 `<iframe>`s holds 1202 elements and
/// `window.frames.length` is 1000, and the tab is told of 1000 frames
/// attached (2026-10-25). A frame taken out of its page is forgotten with
/// every frame inside it, and so is every frame a document held when its
/// frame commits another; so a page never reaches this, and a frame beyond
/// it is the page's, as a frame this tab was never told of is.
pub const MOST_FRAMES: usize = 1024;

/// One frame of a tab, and the document it holds.
#[derive(Debug, Default)]
struct Frame {
    /// The frame it is inside, by Chrome's id; [`None`] for the tab's own.
    holder: Option<String>,
    /// The document it holds, once one has committed.
    page: Option<DocumentId>,
    /// The document a navigation of it under way will commit, minted at its
    /// first hop.
    arriving: Option<DocumentId>,
    /// The loader Chrome loads that document by — its first hop's
    /// `networkId` — when Chrome named one.
    loader: Option<String>,
    /// The cause of its navigation under way, which every hop carries.
    hop: Option<Cause>,
    /// A navigation of it asked for while a verb was carried out: where to,
    /// without its fragment, and the agent's cause.
    asked: Option<(String, Cause)>,
}

/// What caused what, in one tab in Chrome.
///
/// Every frame of the tab is a document of its own (queue item 636): a
/// frame's navigation mints its document, caused by what the document
/// holding it was doing, and what the frame then asks for is that
/// document's — so the walk back from a frame's `fetch` passes through the
/// frame and then the page. Chrome names the frame each request is for,
/// and a dedicated worker's requests are named for the frame whose document
/// started it (measured against Chrome 155, 2026-10-25), so a worker's are
/// its document's. A request for a frame this tab does not remember — never
/// told of, or beyond [`MOST_FRAMES`] — is the tab's own frame's, as every
/// frame's was before 636: the page holds it, which is true, if less than
/// the whole truth.
#[derive(Debug)]
pub struct Causes {
    /// What caused the tab's first page: the person, or the agent's ask
    /// that opened the tab (ADR 0063). What its own frame and its frames ask
    /// for before any document is in them is caused by it too.
    first: Cause,
    /// The tab's own frame, by Chrome's id: the tab's target id.
    own: String,
    /// Every frame remembered, by Chrome's id, the tab's own among them.
    frames: HashMap<String, Frame>,
    /// The verb being carried out, and the documents the tab's frames held
    /// as it began.
    ///
    /// The documents are half of it because **a verb's action causes what
    /// the page it acted in asks for, and not what a page that replaced it
    /// asks for.** An `activate` that follows a link is answered when the
    /// page has gone, not when the page that arrives has finished asking for
    /// things, so without the document the cause of a new page's stylesheet
    /// would depend on whether this machine answered the verb before Chrome
    /// paused the stylesheet — which it did on a quiet machine and did not
    /// under load (24 October 2026). ADR 0012 says every request says what
    /// caused it; a cause that is one thing when the machine is busy and
    /// another when it is not says nothing. A frame's document is in the
    /// page the verb acted on, so it is acted in too.
    acting: Option<(ActionId, HashSet<DocumentId>)>,
}

impl Causes {
    /// A tab this process opened, `tab`, whose own frame Chrome names
    /// `own`: its first page is the person's.
    pub fn of(tab: TabId, own: &str) -> Self {
        Self::beginning_with(Cause::Person { tab }, own)
    }

    /// A tab opened as `action`, an agent's ask, `tab`, whose own frame
    /// Chrome names `own`: its first page is that action's, done in the tab
    /// (ADR 0063).
    pub fn opened_by(action: ActionId, tab: TabId, own: &str) -> Self {
        Self::beginning_with(
            Cause::Agent {
                action,
                place: Place::Tab(tab),
            },
            own,
        )
    }

    /// A tab whose first page `first` caused.
    fn beginning_with(first: Cause, own: &str) -> Self {
        Self {
            first,
            own: own.to_owned(),
            frames: HashMap::from([(own.to_owned(), Frame::default())]),
            acting: None,
        }
    }

    /// Chrome made the frame `frame` inside the frame `holder`.
    pub fn made(&mut self, frame: &str, holder: &str) {
        if frame == self.own || self.frames.contains_key(frame) || self.frames.len() >= MOST_FRAMES
        {
            return;
        }
        self.frames.insert(
            frame.to_owned(),
            Frame {
                holder: Some(holder.to_owned()),
                ..Frame::default()
            },
        );
    }

    /// The frame `frame` was taken out of its page: it is forgotten, with
    /// every frame inside it.
    pub fn removed(&mut self, frame: &str) {
        if frame == self.own {
            return;
        }
        self.forget_inside(frame);
        self.frames.remove(frame);
    }

    /// The cause of a request for the frame `frame`, asked for at `url`,
    /// which falls in a navigation of that frame as `hop` says; a first hop
    /// mints the document the navigation will commit, in `record`, loaded by
    /// `loader` when Chrome named one.
    pub fn of_request(
        &mut self,
        record: &mut Record,
        frame: &str,
        url: &str,
        hop: Hop,
        loader: Option<&str>,
    ) -> Cause {
        if hop == Hop::None {
            return self.of_frame(frame);
        }
        // A navigation of a frame not remembered is the page's, and mints
        // nothing: nothing would hold the document it minted.
        if !self.frames.contains_key(frame) {
            return self.of_frame(&self.own);
        }
        let navigation = self.of_navigation(frame);
        let Some(this) = self.frames.get_mut(frame) else {
            return navigation;
        };
        if let (Hop::Redirected, Some(cause)) = (hop, &this.hop) {
            return cause.clone();
        }
        // A first hop — or a hop a redirect led to, of a navigation whose
        // first hop this tab never paused, which is taken as a first.
        let cause = match this.asked.take() {
            Some((to, cause)) if to == without_fragment(url) => cause,
            _ => navigation,
        };
        this.hop = Some(cause.clone());
        this.arriving = Some(record.opened(cause.clone()));
        this.loader = loader.map(str::to_owned);
        cause
    }

    /// The document arriving in the frame `frame`, and the loader Chrome
    /// loads it by, while the frame holds another: the window in which a
    /// request paused for the frame may be either's (queue item 647). A
    /// frame not remembered is the tab's own.
    pub fn window(&self, frame: &str) -> Option<(DocumentId, &str)> {
        let this = self.frames.get(self.remembered(frame))?;
        this.page?;
        Some((this.arriving?, this.loader.as_deref()?))
    }

    /// Whether the frame `frame` is remembered: told of, and not gone with
    /// its page or the document holding it.
    pub fn remembers(&self, frame: &str) -> bool {
        self.frames.contains_key(frame)
    }

    /// The cause of what the page asks for, now: the agent's while a verb is
    /// carried out, and otherwise the page's.
    pub fn of_page(&self) -> Cause {
        self.of_frame(&self.own)
    }

    /// The cause of what the frame `frame` asks for, now: its document's,
    /// or the agent's in it while a verb is carried out; before a frame's
    /// first document has committed, the one arriving, and before that what
    /// the frame holding it asks for. A frame not remembered is the tab's
    /// own.
    pub fn of_frame(&self, frame: &str) -> Cause {
        let mut at = self.remembered(frame);
        // Each holder was remembered before the frame inside it, so this
        // walks up and ends; it is bounded all the same, since a walk that
        // trusted that would hang the browser process on the first bug.
        for _ in 0..=self.frames.len() {
            let Some(this) = self.frames.get(at) else {
                break;
            };
            if let Some(document) = this.page {
                return self.in_document(document);
            }
            // Before a frame's first document has committed, nothing else in
            // it has asked for anything: whatever is asked is that one's.
            if let Some(document) = this.arriving {
                return Cause::Document { document };
            }
            match &this.holder {
                Some(holder) => at = self.remembered(holder),
                None => break,
            }
        }
        self.first.clone()
    }

    /// The document the tab's own frame holds, once one has committed.
    pub fn page(&self) -> Option<DocumentId> {
        self.document_of(&self.own)
    }

    /// The document the frame `frame` holds, once one has committed.
    pub fn document_of(&self, frame: &str) -> Option<DocumentId> {
        self.frames.get(frame).and_then(|this| this.page)
    }

    /// The frame `frame` asked to go to `url`: while a verb is carried out
    /// in the document it holds, that navigation is the agent's.
    pub fn asked_to_go(&mut self, frame: &str, url: &str) {
        let Some(document) = self.document_of(frame) else {
            return;
        };
        if let Some(action) = self.acting_in(document)
            && let Some(this) = self.frames.get_mut(frame)
        {
            this.asked = Some((
                without_fragment(url).to_owned(),
                Cause::Agent {
                    action,
                    place: Place::Document(document),
                },
            ));
        }
    }

    /// The frame `frame` committed a new document: the one its navigation
    /// minted, or — for a document that came of no request this tab paused,
    /// as `about:blank` does — one minted now, with the navigation's cause.
    /// The frames the document it held held are gone with it.
    pub fn committed(&mut self, record: &mut Record, frame: &str) {
        if !self.frames.contains_key(frame) {
            return;
        }
        let navigation = self.of_navigation(frame);
        self.forget_inside(frame);
        if let Some(this) = self.frames.get_mut(frame) {
            let document = this
                .arriving
                .take()
                .unwrap_or_else(|| record.opened(navigation));
            this.page = Some(document);
            this.hop = None;
            this.loader = None;
        }
    }

    /// The navigation of the frame `frame` under way went no further: a hop
    /// of it was refused. Its document was caused and minted, and is never
    /// the frame's; it is answered, when there was one.
    pub fn ended(&mut self, frame: &str) -> Option<DocumentId> {
        let this = self.frames.get_mut(frame)?;
        this.hop = None;
        this.loader = None;
        this.arriving.take()
    }

    /// `action` is being carried out on the tab, or [`None`] once it has
    /// been. The documents the tab's frames hold as it begins are
    /// remembered with it: the action causes what *those* documents ask
    /// for, and nothing a document which replaces one asks for.
    pub fn acting(&mut self, action: Option<ActionId>) {
        self.acting = action.map(|action| {
            let held = self.frames.values().filter_map(|this| this.page).collect();
            (action, held)
        });
    }

    /// The action being carried out, when `document` is one it is carried
    /// out in.
    fn acting_in(&self, document: DocumentId) -> Option<ActionId> {
        match &self.acting {
            Some((action, held)) if held.contains(&document) => Some(*action),
            _ => None,
        }
    }

    /// What `document` asks for is caused by: the agent's while a verb is
    /// carried out in it, and otherwise its own.
    fn in_document(&self, document: DocumentId) -> Cause {
        match self.acting_in(document) {
            Some(action) => Cause::Agent {
                action,
                place: Place::Document(document),
            },
            None => Cause::Document { document },
        }
    }

    /// The cause of a navigation of the frame `frame` nothing asked for while
    /// a verb was carried out: what the document it holds asks for, when it
    /// holds one; for the tab's own frame's first, what opened the tab; and for
    /// another frame's first, what the frame holding it asks for.
    fn of_navigation(&self, frame: &str) -> Cause {
        let Some(this) = self.frames.get(frame) else {
            return self.of_page();
        };
        match (this.page, &this.holder) {
            (Some(document), _) => self.in_document(document),
            (None, Some(holder)) => self.of_frame(holder),
            (None, None) => self.first.clone(),
        }
    }

    /// `frame` when it is remembered, and otherwise the tab's own frame.
    fn remembered<'f>(&'f self, frame: &'f str) -> &'f str {
        if self.frames.contains_key(frame) {
            frame
        } else {
            &self.own
        }
    }

    /// Forget every frame inside `frame`, however deep, and not `frame`.
    fn forget_inside(&mut self, frame: &str) {
        let mut gone = HashSet::from([frame.to_owned()]);
        loop {
            let inside: Vec<String> = self
                .frames
                .iter()
                .filter(|(id, this)| {
                    !gone.contains(*id)
                        && this
                            .holder
                            .as_ref()
                            .is_some_and(|holder| gone.contains(holder))
                })
                .map(|(id, _)| id.clone())
                .collect();
            if inside.is_empty() {
                break;
            }
            gone.extend(inside);
        }
        gone.remove(frame);
        self.frames.retain(|id, _| !gone.contains(id));
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

    /// ADR 0063: a tab an agent asked for. Its first page's every hop is
    /// the action's, in the tab; what the page asks for is the page's, whose
    /// chain ends at the action and the tab — never at the person.
    #[test]
    fn a_tab_an_agent_opened_begins_with_its_action_and_not_the_person() {
        let mut record = Record::default();
        let tab = record.a_tab();
        let action = record.an_action();
        let opening = Cause::Agent {
            action,
            place: Place::Tab(tab),
        };
        let mut causes = Causes::opened_by(action, tab, "T");
        assert_eq!(causes.of_page(), opening, "before anything has arrived");
        let first = causes.of_request(&mut record, "T", "http://a/start", Hop::First, None);
        assert_eq!(first, opening);
        let hop = causes.of_request(&mut record, "T", "http://b/page", Hop::Redirected, None);
        assert_eq!(
            hop, opening,
            "a redirect is whatever asked for the first hop"
        );
        causes.committed(&mut record, "T");
        let sheet = causes.of_request(&mut record, "T", "http://b/s.css", Hop::None, None);
        let Cause::Document { document } = sheet else {
            panic!("a page's sheet is not the page's: {sheet}");
        };
        assert_eq!(record.documents().cause_of(document), Some(&opening));
        let chain = record.documents().chain(&sheet);
        assert_eq!(chain.end(), End::Opened(action, tab));
        assert_eq!(chain.action(), Some(action));
        assert_eq!(chain.person(), None, "{chain}");
        assert!(chain.is_whole());
    }

    #[test]
    fn the_first_page_is_the_persons_and_what_it_loads_is_its_own() {
        let mut record = Record::default();
        let tab = record.a_tab();
        let mut causes = Causes::of(tab, "T");
        let first = causes.of_request(&mut record, "T", "http://a/start", Hop::First, None);
        assert_eq!(first, Cause::Person { tab });
        let hop = causes.of_request(&mut record, "T", "http://b/page", Hop::Redirected, None);
        assert_eq!(hop, first, "a redirect is whatever asked for the first hop");
        assert_eq!(record.documents().remembered(), 1, "one document, not two");
        causes.committed(&mut record, "T");
        let Cause::Document { document } =
            causes.of_request(&mut record, "T", "http://b/s.css", Hop::None, None)
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
        let mut causes = Causes::of(record.a_tab(), "T");
        causes.of_request(&mut record, "T", "http://a/", Hop::First, None);
        causes.committed(&mut record, "T");
        let action = record.an_action();
        causes.acting(Some(action));
        let during = causes.of_request(&mut record, "T", "http://a/f", Hop::None, None);
        assert!(matches!(during, Cause::Agent { action: acted, .. } if acted == action));
        causes.asked_to_go("T", "http://a/next#there");
        causes.acting(None);
        assert!(
            matches!(causes.of_page(), Cause::Document { .. }),
            "the verb has answered"
        );
        let going = causes.of_request(&mut record, "T", "http://a/next", Hop::First, None);
        assert!(matches!(going, Cause::Agent { action: acted, .. } if acted == action));
        causes.committed(&mut record, "T");
        let next = causes.of_request(&mut record, "T", "http://a/next.css", Hop::None, None);
        assert!(record.documents().chain(&next).followed_from(action));
    }

    #[test]
    fn a_page_the_verb_brought_asks_for_its_own_things_even_before_the_verb_answers() {
        // The test above is the quick machine: the verb answers before the
        // page it brought asks for anything. This is the busy one, where it
        // does not — and the cause must be the same either way, or ADR 0012's
        // promise holds only on an idle laptop. It did not until 24 October
        // 2026: under load, the second page's stylesheet was pinned on the
        // agent's action and the test above passed anyway.
        let mut record = Record::default();
        let mut causes = Causes::of(record.a_tab(), "T");
        causes.of_request(&mut record, "T", "http://a/", Hop::First, None);
        causes.committed(&mut record, "T");
        let action = record.an_action();
        causes.acting(Some(action));
        causes.asked_to_go("T", "http://a/next");
        let going = causes.of_request(&mut record, "T", "http://a/next", Hop::First, None);
        assert!(
            matches!(going, Cause::Agent { action: acted, .. } if acted == action),
            "the navigation itself is the agent's"
        );
        causes.committed(&mut record, "T");
        // Still acting: the verb has not answered. The page that arrived is
        // nonetheless the one asking.
        let sheet = causes.of_request(&mut record, "T", "http://a/next.css", Hop::None, None);
        assert!(
            matches!(sheet, Cause::Document { .. }),
            "the page that arrived asked for its own sheet, not the agent: {sheet:?}"
        );
        // And it still leads back to the action, through that page.
        assert!(record.documents().chain(&sheet).followed_from(action));
        // The verb answering afterwards changes nothing.
        causes.acting(None);
        assert_eq!(
            causes.of_request(&mut record, "T", "http://a/next.js", Hop::None, None),
            sheet
        );
    }

    #[test]
    fn a_navigation_the_verb_asked_for_and_never_made_is_not_pinned_on_the_next() {
        let mut record = Record::default();
        let mut causes = Causes::of(record.a_tab(), "T");
        causes.of_request(&mut record, "T", "http://a/", Hop::First, None);
        causes.committed(&mut record, "T");
        let page = causes.of_page();
        let action = record.an_action();
        causes.acting(Some(action));
        causes.asked_to_go("T", "http://a/never");
        causes.acting(None);
        let elsewhere = causes.of_request(&mut record, "T", "http://a/later", Hop::First, None);
        assert_eq!(elsewhere, page, "the page went on its own");
        // And the ask is spent, so it is not waiting for the next.
        causes.committed(&mut record, "T");
        let again = causes.of_request(&mut record, "T", "http://a/never", Hop::First, None);
        assert!(matches!(again, Cause::Document { .. }), "{again}");
    }

    #[test]
    fn what_the_old_page_asks_for_before_the_new_one_commits_is_the_old_pages() {
        let mut record = Record::default();
        let mut causes = Causes::of(record.a_tab(), "T");
        causes.of_request(&mut record, "T", "http://a/", Hop::First, None);
        causes.committed(&mut record, "T");
        let old = causes.of_page();
        causes.of_request(&mut record, "T", "http://a/next", Hop::First, None);
        assert_eq!(
            causes.of_request(&mut record, "T", "http://a/beacon", Hop::None, None),
            old
        );
        causes.committed(&mut record, "T");
        assert_ne!(causes.of_page(), old);
    }

    /// Item 647: between a first hop and its commit, a frame holds one
    /// document and is getting another, and the window names the one
    /// arriving with the loader its first hop's `networkId` gave.
    #[test]
    fn a_frame_going_to_a_new_document_names_it_and_its_loader_until_it_commits() {
        let mut record = Record::default();
        let mut causes = Causes::of(record.a_tab(), "T");
        causes.of_request(&mut record, "T", "http://a/", Hop::First, Some("L1"));
        assert_eq!(causes.window("T"), None, "nothing held yet: no other to be");
        causes.committed(&mut record, "T");
        assert_eq!(causes.window("T"), None);
        causes.of_request(&mut record, "T", "http://a/go", Hop::First, Some("L2"));
        causes.of_request(
            &mut record,
            "T",
            "http://a/next",
            Hop::Redirected,
            Some("L2"),
        );
        let Some((arriving, loader)) = causes.window("T") else {
            panic!("no window");
        };
        assert_eq!(loader, "L2");
        assert_eq!(
            causes.window("X").map(|(document, _)| document),
            Some(arriving)
        );
        causes.committed(&mut record, "T");
        assert_eq!(causes.page(), Some(arriving));
        assert_eq!(causes.window("T"), None);
        // Refused at a hop: answered, and no window after.
        causes.of_request(&mut record, "T", "http://a/away", Hop::First, Some("L3"));
        let (away, _) = causes.window("T").unwrap();
        assert_eq!(causes.ended("T"), Some(away));
        assert_eq!(causes.window("T"), None);
        assert_eq!(causes.ended("T"), None);
        // A first hop Chrome named no loader for opens no window.
        causes.of_request(&mut record, "T", "http://a/x", Hop::First, None);
        assert_eq!(causes.window("T"), None);
    }

    #[test]
    fn a_navigation_refused_at_a_hop_never_becomes_the_page() {
        let mut record = Record::default();
        let mut causes = Causes::of(record.a_tab(), "T");
        causes.of_request(&mut record, "T", "http://a/", Hop::First, None);
        causes.committed(&mut record, "T");
        let page = causes.of_page();
        causes.of_request(&mut record, "T", "http://a/away", Hop::First, None);
        causes.ended("T");
        assert_eq!(record.documents().remembered(), 2, "caused, and minted");
        assert_eq!(causes.of_page(), page);
        // A later commit — Chrome's own error page, say — is not the
        // refused navigation's document.
        causes.committed(&mut record, "T");
        assert_eq!(record.documents().remembered(), 3);
    }

    #[test]
    fn a_document_from_no_request_is_minted_as_it_commits() {
        let mut record = Record::default();
        let tab = record.a_tab();
        let mut causes = Causes::of(tab, "T");
        causes.committed(&mut record, "T");
        assert_eq!(record.documents().remembered(), 1);
        let Cause::Document { document } = causes.of_page() else {
            panic!("no page");
        };
        assert_eq!(
            record.documents().cause_of(document),
            Some(&Cause::Person { tab })
        );
    }

    /// A tab whose own frame `T` has committed its first page.
    fn a_page() -> (Record, Causes, DocumentId) {
        let mut record = Record::default();
        let mut causes = Causes::of(record.a_tab(), "T");
        causes.of_request(&mut record, "T", "http://a/", Hop::First, None);
        causes.committed(&mut record, "T");
        let page = causes.page().unwrap();
        (record, causes, page)
    }

    #[test]
    fn a_frames_fetch_is_its_own_documents_whose_cause_is_the_page() {
        let (mut record, mut causes, page) = a_page();
        causes.made("F", "T");
        let going = causes.of_request(&mut record, "F", "http://b/frame", Hop::First, None);
        assert_eq!(
            going,
            Cause::Document { document: page },
            "the page loads it"
        );
        causes.committed(&mut record, "F");
        let frame = causes.document_of("F").unwrap();
        assert_ne!(frame, page);
        let fetch = causes.of_request(&mut record, "F", "http://b/f", Hop::None, None);
        assert_eq!(fetch, Cause::Document { document: frame });
        let chain = record.documents().chain(&fetch);
        assert_eq!(
            chain.links().get(1),
            Some(&Cause::Document { document: page }),
            "{chain:?}"
        );
        assert!(chain.person().is_some() && chain.is_whole(), "{chain:?}");
        // The page's own are still the page's.
        assert_eq!(
            causes.of_request(&mut record, "T", "http://a/f", Hop::None, None),
            Cause::Document { document: page }
        );
    }

    #[test]
    fn a_frame_inside_a_frame_is_caused_by_the_frame_holding_it() {
        let (mut record, mut causes, page) = a_page();
        causes.made("F", "T");
        causes.of_request(&mut record, "F", "http://b/frame", Hop::First, None);
        causes.committed(&mut record, "F");
        let frame = causes.document_of("F").unwrap();
        causes.made("G", "F");
        assert_eq!(
            causes.of_request(&mut record, "G", "http://b/inner", Hop::First, None),
            Cause::Document { document: frame }
        );
        causes.committed(&mut record, "G");
        let inner = causes.of_request(&mut record, "G", "http://b/g", Hop::None, None);
        let links = record.documents().chain(&inner).links().to_vec();
        assert_eq!(links.get(1), Some(&Cause::Document { document: frame }));
        assert_eq!(links.get(2), Some(&Cause::Document { document: page }));
    }

    #[test]
    fn what_a_frame_asks_before_its_document_commits_is_the_arriving_ones() {
        let (mut record, mut causes, page) = a_page();
        causes.made("F", "T");
        assert_eq!(
            causes.of_request(&mut record, "F", "http://a/early", Hop::None, None),
            Cause::Document { document: page },
            "a frame with nothing in it yet is its holder's"
        );
        causes.of_request(&mut record, "F", "http://b/frame", Hop::First, None);
        let hop = causes.of_request(&mut record, "F", "http://c/frame", Hop::Redirected, None);
        assert_eq!(hop, Cause::Document { document: page }, "a redirect");
        assert_eq!(record.documents().remembered(), 2, "one frame document");
        let Cause::Document { document } =
            causes.of_request(&mut record, "F", "http://c/f", Hop::None, None)
        else {
            panic!("not a document's");
        };
        assert_ne!(document, page);
        causes.committed(&mut record, "F");
        assert_eq!(causes.document_of("F"), Some(document));
    }

    #[test]
    fn a_frame_not_remembered_is_the_pages_and_mints_nothing() {
        let (mut record, mut causes, page) = a_page();
        let before = record.documents().remembered();
        assert_eq!(
            causes.of_request(&mut record, "X", "http://b/frame", Hop::First, None),
            Cause::Document { document: page }
        );
        causes.committed(&mut record, "X");
        assert_eq!(record.documents().remembered(), before);
        assert_eq!(causes.document_of("X"), None);
        assert_eq!(
            causes.of_request(&mut record, "X", "http://b/f", Hop::None, None),
            Cause::Document { document: page }
        );
    }

    #[test]
    fn a_frame_removed_is_forgotten_with_what_it_held_and_the_tabs_own_never_is() {
        let (mut record, mut causes, page) = a_page();
        causes.made("F", "T");
        causes.committed(&mut record, "F");
        causes.made("G", "F");
        causes.committed(&mut record, "G");
        causes.removed("F");
        assert_eq!(causes.document_of("F"), None);
        assert_eq!(causes.document_of("G"), None, "inside what was removed");
        causes.removed("T");
        assert_eq!(causes.page(), Some(page));
    }

    #[test]
    fn a_new_document_holds_none_of_the_frames_the_old_one_held() {
        let (mut record, mut causes, _) = a_page();
        causes.made("F", "T");
        causes.committed(&mut record, "F");
        causes.of_request(&mut record, "T", "http://a/next", Hop::First, None);
        causes.committed(&mut record, "T");
        assert_eq!(causes.document_of("F"), None);
        let next = causes.page().unwrap();
        assert_eq!(
            causes.of_request(&mut record, "F", "http://a/f", Hop::None, None),
            Cause::Document { document: next },
            "a frame gone with its page is the tab's own"
        );
    }

    #[test]
    fn no_more_frames_than_chrome_makes_are_remembered() {
        let (mut record, mut causes, page) = a_page();
        for frame in 0..MOST_FRAMES + 8 {
            causes.made(&frame.to_string(), "T");
        }
        assert_eq!(causes.frames.len(), MOST_FRAMES);
        let last = (MOST_FRAMES + 7).to_string();
        assert_eq!(
            causes.of_request(&mut record, &last, "http://b/frame", Hop::First, None),
            Cause::Document { document: page }
        );
        causes.removed("0");
        causes.made(&last, "T");
        assert!(causes.frames.contains_key(&last), "a place came free");
    }

    #[test]
    fn a_verb_in_the_page_is_the_agents_in_its_frames_and_not_in_a_frame_it_brought() {
        let (mut record, mut causes, _) = a_page();
        causes.made("F", "T");
        causes.of_request(&mut record, "F", "http://b/frame", Hop::First, None);
        causes.committed(&mut record, "F");
        let frame = causes.document_of("F").unwrap();
        let action = record.an_action();
        causes.acting(Some(action));
        assert_eq!(
            causes.of_request(&mut record, "F", "http://b/f", Hop::None, None),
            Cause::Agent {
                action,
                place: Place::Document(frame)
            }
        );
        // A link in the frame, followed after the verb has answered, is
        // still the agent's, and the frame's new document asks for its own.
        causes.asked_to_go("F", "http://b/next#part");
        causes.made("N", "T");
        let brought = causes.of_request(&mut record, "N", "http://c/new", Hop::First, None);
        assert!(matches!(brought, Cause::Agent { .. }), "{brought}");
        causes.committed(&mut record, "N");
        let new = causes.document_of("N").unwrap();
        assert_eq!(
            causes.of_request(&mut record, "N", "http://c/n", Hop::None, None),
            Cause::Document { document: new },
            "a frame the verb brought asks for its own"
        );
        causes.acting(None);
        let going = causes.of_request(&mut record, "F", "http://b/next", Hop::First, None);
        assert_eq!(
            going,
            Cause::Agent {
                action,
                place: Place::Document(frame)
            }
        );
        causes.committed(&mut record, "F");
        let next = causes.of_request(&mut record, "F", "http://b/n.css", Hop::None, None);
        assert!(record.documents().chain(&next).followed_from(action));
        assert!(matches!(next, Cause::Document { .. }));
    }

    #[test]
    fn a_frames_refused_navigation_never_becomes_its_document() {
        let (mut record, mut causes, _) = a_page();
        causes.made("F", "T");
        causes.of_request(&mut record, "F", "http://b/frame", Hop::First, None);
        causes.committed(&mut record, "F");
        let frame = causes.document_of("F");
        causes.of_request(&mut record, "F", "http://b/away", Hop::First, None);
        causes.ended("F");
        assert_eq!(causes.document_of("F"), frame);
        assert_eq!(
            causes.of_request(&mut record, "F", "http://b/x", Hop::None, None),
            Cause::Document {
                document: frame.unwrap()
            }
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
