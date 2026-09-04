/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A tab: what somebody opened, what it is showing, and what happened to it.
//!
//! # The sentence this file exists to make true
//!
//! ADR 0005: *"The tab keeps the last frame it painted and says what happened.
//! Every other tab is untouched, because no other tab was in that process.
//! Reloading is the user's to ask for; a browser that silently restarts a
//! renderer hides a bug that somebody needs to see."*
//!
//! [`crate::host`] had the second and third halves of that and not the first.
//! It knows renderers, and a renderer that dies is a [`Gone`] with a sentence
//! in it — but a sentence returned to whoever asked is not a tab saying
//! anything, and nothing anywhere kept a frame. A person would have been shown
//! **a blank rectangle**, which is the one outcome the line in `ROADMAP.md`
//! names.
//!
//! # Why the frame is kept here
//!
//! Because the thing that died is the other side. A frame is the one thing
//! ADR 0005 lets processes share — read-only pixels — so the browser process
//! already holds a copy of every one it was sent, and keeping the last is a
//! [`Frame`] per tab rather than anything clever. That is what a tab costs, and
//! it is the price of a dead renderer leaving a page on the screen instead of a
//! hole.
//!
//! # Nothing here restarts anything
//!
//! A tab whose renderer has gone answers **from what it was told** rather than
//! reaching for a renderer, because [`Renderers::ask`] starts one for a site
//! that has none — so a repaint of a dead tab would spawn a fresh process,
//! find it holding no page, and report that nothing is loaded. The bug that
//! killed the first one would have vanished from view. Only a **deliberate**
//! load ([`Tabs::load`]) starts a renderer for a tab that has lost one, which
//! is the same rule `host.rs` states one layer down.
//!
//! # One process per site, and one document per process
//!
//! Two tabs on one site share a renderer (ADR 0005), and a [`crate::Renderer`]
//! holds **one** page — `docs/features.md`'s *"several documents at once, the
//! shape tabs need"* is not built. So the second tab to load on a site
//! displaces the first inside that process, and this file refuses to ask about
//! a page the renderer is no longer holding
//! ([`Lost::HoldsAnotherPage`]) rather than answering with somebody else's.
//! The displaced tab still shows its own last frame, which is what a person is
//! looking at anyway.

use crate::frame::Frame;
use crate::host::{Gone, Renderers};
use crate::message::{FromRenderer, ToRenderer};
use crate::page::Page;
use crate::site::Site;
use alo_url::Url;
use core::fmt;
use std::collections::{HashMap, HashSet};

/// Which tab.
///
/// Allocated in the order tabs were opened and **never reused**, the same rule
/// ADR 0003 gives `alo_box::BoxId` and for a version of the same reason: a
/// closed tab's id must never come to mean a different page, or a caller
/// holding one acts on the wrong one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TabId(u64);

impl TabId {
    /// The id as a number, for diagnostics and for a caller keeping its own
    /// record beside this one.
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl fmt::Display for TabId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tab#{}", self.0)
    }
}

/// One tab: a page somebody opened, and what became of it.
#[derive(Debug, Clone)]
pub struct Tab {
    id: TabId,
    url: Url,
    site: Site,
    painted: Option<Frame>,
    gone: Option<Gone>,
}

impl Tab {
    /// Which tab this is.
    pub fn id(&self) -> TabId {
        self.id
    }

    /// What it was opened at.
    ///
    /// It does not change: navigating is queue item 85, and until there is a
    /// session history to move through, a tab is one address.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Which site it belongs to, and therefore which process renders it.
    pub fn site(&self) -> &Site {
        &self.site
    }

    /// The last frame it painted, if it ever painted one.
    ///
    /// **Kept when its renderer dies**, which is the whole of ADR 0005's
    /// promise about what a person sees afterwards.
    pub fn frame(&self) -> Option<&Frame> {
        self.painted.as_ref()
    }

    /// Whether its renderer is running, as far as this tab has been told.
    ///
    /// A tab finds out that its renderer went away when it next asks for
    /// something — which is also the moment a person would notice, since until
    /// then the tab is showing the frame it already has and nothing on the
    /// screen is wrong.
    pub fn is_live(&self) -> bool {
        self.gone.is_none()
    }

    /// What happened to its renderer, if something did.
    pub fn gone(&self) -> Option<&Gone> {
        self.gone.as_ref()
    }

    /// What to tell a person, in words, when there is something to tell them.
    ///
    /// The sentence rather than the structure, because this is the half of
    /// *"and says so"* that reaches somebody who is not a program.
    pub fn what_happened(&self) -> Option<String> {
        self.gone.as_ref().map(ToString::to_string)
    }
}

/// Why a tab could not be answered.
///
/// Distinct from [`FromRenderer::Failed`] and from [`FromRenderer::Refused`],
/// which are both a live renderer doing its job. This is the browser process
/// saying there was nothing it could sensibly ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lost {
    /// There is no tab with that id — it was closed, or never opened.
    NoSuchTab(TabId),
    /// Its renderer is not there any more, and this is what happened.
    Gone(Gone),
    /// Its site's renderer is holding another tab's page.
    ///
    /// One process per site (ADR 0005) and one document per process, so asking
    /// would have answered about the wrong page. The tab still has its own last
    /// frame; what it does not have is a renderer that could paint it again.
    HoldsAnotherPage {
        /// The tab whose page that renderer is holding. It may itself have been
        /// closed since — ids are never reused, so this still names the page
        /// rather than a page.
        holder: TabId,
    },
}

impl fmt::Display for Lost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Lost::NoSuchTab(id) => write!(f, "there is no {id}"),
            Lost::Gone(gone) => write!(f, "{gone}"),
            Lost::HoldsAnotherPage { holder } => write!(
                f,
                "its renderer is holding {holder}'s page, and a renderer holds one page"
            ),
        }
    }
}

impl std::error::Error for Lost {}

/// What is known about a tab at the moment it asks for something.
///
/// Everything [`may_ask`] is allowed to decide on, and nothing else — see that
/// function for why it is a value rather than a look at `self`.
#[derive(Debug, Clone, Copy)]
struct Asking<'a> {
    /// The tab asking.
    id: TabId,
    /// Whether this is a person deliberately loading a page, rather than a
    /// repaint, a read or a verb.
    deliberate: bool,
    /// What this tab was last told about its renderer.
    gone: Option<&'a Gone>,
    /// Which tab's page that site's renderer is holding, if any.
    holder: Option<TabId>,
    /// Whether a renderer for the site is running at all.
    running: bool,
}

/// Whether a tab may ask its site's renderer, decided before anything is sent.
///
/// A pure function, which is the shape queue items 55, 154 and 188 already use
/// and for a version of the same reason: every rule here is a rule about **not
/// starting a process**, and a rule about not doing something is asserted
/// honestly only when nothing is moving. A test of it spawns nothing, so the
/// refusals are checked in full rather than in the two or three arrangements a
/// test with real processes can reach.
///
/// The [`Gone`] it returns for a renderer that stopped is *made here*: nothing
/// told this tab, which is exactly the case — a renderer evicted to stay under
/// [`crate::host::MOST_RENDERERS`] goes away without anybody dying.
fn may_ask(asking: &Asking<'_>, site: &Site) -> Result<(), Lost> {
    // A load is the person asking for this on purpose, and ADR 0005 says
    // reloading is theirs to ask for. So it passes every rule below: a dead
    // tab is reloadable, and a displaced one takes its renderer back.
    if asking.deliberate {
        return Ok(());
    }
    if let Some(gone) = asking.gone {
        return Err(Lost::Gone(gone.clone()));
    }
    match asking.holder {
        Some(holder) if holder != asking.id => Err(Lost::HoldsAnotherPage { holder }),
        // This tab's page is the one that renderer holds, and there is no such
        // renderer any more. Nothing died: it was stopped — by the ceiling on
        // how many may exist, or by whoever closed it — and the tab is told
        // now rather than a fresh process being started to answer for a page
        // it has never seen.
        Some(_) => {
            if asking.running {
                Ok(())
            } else {
                Err(Lost::Gone(Gone {
                    site: site.to_string(),
                    why: "it was stopped".to_owned(),
                }))
            }
        }
        // Nothing has been loaded on this site at all. The renderer answers
        // that better than a guess here would: it says nothing is loaded.
        None => Ok(()),
    }
}

/// The tabs a browser process has open, and the renderers under them.
///
/// One of these is what a window is made of. It owns the [`Renderers`] rather
/// than borrowing them so that every [`Gone`] passes through here — a tab that
/// was not told its renderer died is a tab showing a page that cannot answer.
#[derive(Debug)]
pub struct Tabs {
    renderers: Renderers,
    /// Every open tab, in the order they were opened.
    list: Vec<Tab>,
    /// Which tab's page each site's renderer is holding.
    ///
    /// Kept for a **closed** tab too, deliberately: the renderer still holds
    /// that page, and forgetting whose it was would let the next tab on the
    /// site paint it and believe it was its own.
    held: HashMap<Site, TabId>,
    opened: u64,
}

impl Tabs {
    /// Tabs over these renderers.
    pub fn over(renderers: Renderers) -> Self {
        Self {
            renderers,
            list: Vec::new(),
            held: HashMap::new(),
            opened: 0,
        }
    }

    /// The renderers under them, to ask how many processes exist or which one
    /// a site is in.
    ///
    /// Read-only on purpose. Sending work through [`Renderers::ask`] directly
    /// would take a [`Gone`] out of the one path that tells the tabs about it.
    pub fn renderers(&self) -> &Renderers {
        &self.renderers
    }

    /// Open a tab at a URL. Nothing is loaded and no process is started yet.
    pub fn open(&mut self, url: Url) -> TabId {
        let id = TabId(self.opened);
        self.opened += 1;
        let site = Site::of(&url);
        self.list.push(Tab {
            id,
            url,
            site,
            painted: None,
            gone: None,
        });
        id
    }

    /// Close one. Whether there was one to close.
    ///
    /// **A site nothing has open any more loses its renderer**, which is the
    /// reaping half of the lifecycle. This file does not decide that a process
    /// ends: it says which sites still have a tab on them and
    /// [`Renderers::reap`] decides what that means, which is the same division
    /// as everywhere else here — tabs know what a person opened, `host.rs`
    /// knows what a process is.
    ///
    /// Closing one of two tabs on a site stops nothing, because the other tab
    /// is still showing a page out of that process.
    pub fn close(&mut self, id: TabId) -> bool {
        let before = self.list.len();
        self.list.retain(|tab| tab.id != id);
        if self.list.len() == before {
            return false;
        }
        let wanted = self.sites_open();
        for stopped in self.renderers.reap(&wanted) {
            // The page went with the process, so nothing is holding it now —
            // and a `held` entry left behind would refuse the next tab on this
            // site ([`Lost::HoldsAnotherPage`]) on behalf of a renderer that no
            // longer exists.
            self.held.remove(&stopped);
        }
        true
    }

    /// The sites that still have a tab open on them.
    ///
    /// The whole of what this file contributes to reaping, and it is deliberate
    /// that a tab whose renderer has **gone** still names its site: the tab is
    /// open, a person may reload it, and there is no renderer held for a dead
    /// site to stop anyway.
    fn sites_open(&self) -> HashSet<Site> {
        self.list.iter().map(|tab| tab.site.clone()).collect()
    }

    /// Every tab, in the order they were opened.
    pub fn all(&self) -> &[Tab] {
        &self.list
    }

    /// One tab, by id.
    pub fn tab(&self, id: TabId) -> Option<&Tab> {
        self.list.iter().find(|tab| tab.id == id)
    }

    /// How many are open.
    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// Whether none are.
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// Load a page into a tab.
    ///
    /// The deliberate act: it starts a renderer for a tab that has lost one and
    /// takes the site's renderer back from a tab that had displaced it. Nothing
    /// else here does either.
    ///
    /// # Errors
    ///
    /// [`Lost::NoSuchTab`] for an id nobody opened, and [`Lost::Gone`] when the
    /// renderer could not be started or died on the way.
    pub fn load(&mut self, id: TabId, page: Page) -> Result<FromRenderer, Lost> {
        self.ask(id, &ToRenderer::Load(Box::new(page)))
    }

    /// Paint a tab, and keep what came back as the last frame it painted.
    ///
    /// # Errors
    ///
    /// As [`Tabs::ask`].
    pub fn paint(&mut self, id: TabId) -> Result<FromRenderer, Lost> {
        self.ask(id, &ToRenderer::Paint)
    }

    /// Read a tab's page as an agent reads it.
    ///
    /// # Errors
    ///
    /// As [`Tabs::ask`].
    pub fn read(&mut self, id: TabId) -> Result<FromRenderer, Lost> {
        self.ask(id, &ToRenderer::ReadTree)
    }

    /// Find the fonts a tab's page asked for by name and send them over.
    ///
    /// [`Renderers::supply`] is the half that looks at the machine; this is the
    /// half that tells the tabs when the renderer died while being handed one.
    ///
    /// # Errors
    ///
    /// As [`Tabs::ask`]. Families this machine does not have are an answer
    /// rather than an error — see [`Renderers::supply`].
    pub fn supply(&mut self, id: TabId, families: &[String]) -> Result<Vec<String>, Lost> {
        let site = self.site_of(id)?;
        match self.renderers.supply(&site, families) {
            Ok(absent) => Ok(absent),
            Err(gone) => Err(self.lost(&site, gone)),
        }
    }

    /// Send one piece of work on behalf of a tab.
    ///
    /// The one door, so that everything a renderer says about being alive
    /// reaches the tabs that are in it. A [`FromRenderer::Painted`] is kept as
    /// the tab's last frame on the way past.
    ///
    /// # Errors
    ///
    /// [`Lost::NoSuchTab`] for an id nobody opened, [`Lost::Gone`] when the
    /// tab's renderer is not there, and [`Lost::HoldsAnotherPage`] when it is
    /// there and holding a different tab's page. A renderer that answered with
    /// a refusal or a failure is **not** an error — that is the renderer doing
    /// its job, and it comes back as a [`FromRenderer`].
    pub fn ask(&mut self, id: TabId, work: &ToRenderer) -> Result<FromRenderer, Lost> {
        let site = self.site_of(id)?;
        let deliberate = matches!(work, ToRenderer::Load(_));
        let told = self.tab(id).and_then(Tab::gone).cloned();
        let asking = Asking {
            id,
            deliberate,
            gone: told.as_ref(),
            holder: self.held.get(&site).copied(),
            running: self.renderers.holds(&site),
        };
        if let Err(refused) = may_ask(&asking, &site) {
            return Err(match refused {
                // Made rather than reported: nothing told this tab, so nothing
                // has recorded it either, and a tab asked twice should say the
                // same thing both times.
                Lost::Gone(gone) => self.lost(&site, gone),
                other => other,
            });
        }

        let answer = match self.renderers.ask(&site, work) {
            Ok(answer) => answer,
            Err(gone) => return Err(self.lost(&site, gone)),
        };
        match &answer {
            FromRenderer::Painted(frame) => {
                let frame = frame.clone();
                if let Some(tab) = self.list.iter_mut().find(|tab| tab.id == id) {
                    tab.painted = Some(frame);
                }
            }
            FromRenderer::Loaded { .. } => {
                self.held.insert(site, id);
                if let Some(tab) = self.list.iter_mut().find(|tab| tab.id == id) {
                    // It answered, so it is alive, whatever it was last known
                    // to be. This is the only place a tab stops being gone.
                    tab.gone = None;
                }
            }
            _ => {}
        }
        Ok(answer)
    }

    fn site_of(&self, id: TabId) -> Result<Site, Lost> {
        self.tab(id)
            .map(|tab| tab.site.clone())
            .ok_or(Lost::NoSuchTab(id))
    }

    /// Tell every tab on a site that its renderer has gone, and give the
    /// caller the same sentence back.
    ///
    /// **Every** tab on the site, because one process held all of them — and
    /// no tab anywhere else, because no other tab was in that process, which is
    /// the entire claim ADR 0005 makes. Frames are left exactly as they are:
    /// what a tab is showing is the last thing it painted, and a renderer dying
    /// does not change what is on the screen.
    fn lost(&mut self, site: &Site, gone: Gone) -> Lost {
        for tab in self.list.iter_mut().filter(|tab| &tab.site == site) {
            tab.gone = Some(gone.clone());
        }
        // The page it was holding went with it, so the next tab to ask about
        // this site is not competing with anybody.
        self.held.remove(site);
        Lost::Gone(gone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        match alo_url::parse(text) {
            Ok(url) => url,
            Err(why) => panic!("{text} is not a URL: {why}"),
        }
    }

    fn site(text: &str) -> Site {
        Site::of(&url(text))
    }

    /// Renderers over a program that is not one: it exits at once, which is
    /// what a renderer that died looks like from this side.
    ///
    /// Enough for every rule about what happens when there is no renderer to
    /// talk to, and it needs no engine — the real binary, confined, is what
    /// `tests/a_tab_whose_renderer_died.rs` uses, and it is a much slower way
    /// to ask a question about bookkeeping.
    fn nowhere() -> Tabs {
        Tabs::over(Renderers::running("/usr/bin/true", &[]))
    }

    fn gone_at(site: &str) -> Gone {
        Gone {
            site: site.to_owned(),
            why: "it exited".to_owned(),
        }
    }

    // --- What a tab is -------------------------------------------------------

    #[test]
    fn a_tab_knows_its_site_and_two_tabs_on_one_site_agree_about_it() {
        let mut tabs = nowhere();
        let one = tabs.open(url("https://a.bank.example/statement"));
        let two = tabs.open(url("https://b.bank.example/settings"));
        let other = tabs.open(url("https://news.example/today"));

        assert_eq!(tabs.len(), 3);
        assert_eq!(
            tabs.tab(one).map(|tab| tab.site().clone()),
            tabs.tab(two).map(|tab| tab.site().clone()),
            "two subdomains of one site disagreed about their site",
        );
        assert_ne!(
            tabs.tab(one).map(|tab| tab.site().clone()),
            tabs.tab(other).map(|tab| tab.site().clone()),
        );
        assert!(tabs.tab(one).is_some_and(Tab::is_live));
        assert!(tabs.tab(one).is_some_and(|tab| tab.frame().is_none()));
    }

    /// ADR 0003's rule, one layer up: a caller holding an id must never find
    /// it naming a different page.
    #[test]
    fn a_closed_tabs_id_is_never_given_to_another_tab() {
        let mut tabs = nowhere();
        let first = tabs.open(url("https://example.com/"));
        assert!(tabs.close(first));
        assert!(!tabs.close(first), "it closed twice");

        let second = tabs.open(url("https://example.com/"));
        assert_ne!(first, second);
        assert!(tabs.tab(first).is_none());
        assert!(tabs.tab(second).is_some());
    }

    #[test]
    fn asking_about_a_tab_nobody_opened_says_so_rather_than_anything_else() {
        let mut tabs = nowhere();
        let opened = tabs.open(url("https://example.com/"));
        assert!(tabs.close(opened));
        assert_eq!(tabs.paint(opened), Err(Lost::NoSuchTab(opened)));
        assert_eq!(
            tabs.paint(opened).map_err(|why| why.to_string()),
            Err(format!("there is no {opened}")),
        );
    }

    // --- What happens when a renderer is not there ---------------------------

    /// A renderer that is not there to answer is a tab that says what
    /// happened — and the other tabs are untouched, which is the whole claim.
    #[test]
    fn a_tab_whose_renderer_is_not_there_says_so_and_no_other_tab_changes() {
        let mut tabs = nowhere();
        let here = tabs.open(url("https://example.com/"));
        let elsewhere = tabs.open(url("https://news.example/"));

        let load = tabs.load(
            here,
            Page::new("<p>hi</p>", alo_layout::Size::new(10.0, 10.0)),
        );
        assert!(matches!(load, Err(Lost::Gone(_))), "{load:?}");
        assert!(tabs.tab(here).is_some_and(|tab| !tab.is_live()));
        assert!(
            tabs.tab(here)
                .and_then(Tab::what_happened)
                .is_some_and(|said| said.contains("example.com")),
            "the tab did not say whose renderer it was",
        );
        assert!(
            tabs.tab(elsewhere).is_some_and(Tab::is_live),
            "one tab's renderer failing took another tab with it",
        );
        assert!(
            tabs.renderers().is_empty(),
            "a renderer that could not answer was kept",
        );
    }

    /// Every tab in the dead process, and no tab outside it.
    #[test]
    fn a_renderer_going_takes_every_tab_on_its_site_and_no_other() {
        let mut tabs = nowhere();
        let one = tabs.open(url("https://a.bank.example/"));
        let two = tabs.open(url("https://b.bank.example/"));
        let other = tabs.open(url("https://news.example/"));
        let bank = site("https://bank.example/");

        let told = tabs.lost(&bank, gone_at("https://bank.example"));
        assert!(matches!(told, Lost::Gone(_)));
        assert!(tabs.tab(one).is_some_and(|tab| !tab.is_live()));
        assert!(tabs.tab(two).is_some_and(|tab| !tab.is_live()));
        assert!(tabs.tab(other).is_some_and(Tab::is_live));
    }

    /// The frame is what a person is looking at, so it is the one thing a
    /// death must not take.
    #[test]
    fn a_dead_tab_keeps_the_last_frame_it_painted() {
        let mut tabs = nowhere();
        let here = tabs.open(url("https://example.com/"));
        let painted = Frame {
            width: 2,
            height: 1,
            pixels: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        if let Some(tab) = tabs.list.iter_mut().find(|tab| tab.id == here) {
            tab.painted = Some(painted.clone());
        }

        let _ = tabs.lost(
            &site("https://example.com/"),
            gone_at("https://example.com"),
        );
        assert!(tabs.tab(here).is_some_and(|tab| !tab.is_live()));
        assert_eq!(
            tabs.tab(here).and_then(Tab::frame),
            Some(&painted),
            "a dead tab lost the picture it was showing",
        );
    }

    /// ADR 0005: a browser that silently restarts a renderer hides a bug that
    /// somebody needs to see. So a dead tab answers from what it knows.
    #[test]
    fn a_dead_tab_is_not_repainted_by_starting_another_renderer() {
        let mut tabs = nowhere();
        let here = tabs.open(url("https://example.com/"));
        let _ = tabs.lost(
            &site("https://example.com/"),
            gone_at("https://example.com"),
        );

        let again = tabs.paint(here);
        assert_eq!(again, Err(Lost::Gone(gone_at("https://example.com"))));
        assert_eq!(
            tabs.renderers().started(),
            0,
            "a dead tab was quietly given a new process to answer with",
        );
        assert_eq!(
            again.map_err(|why| why.to_string()),
            Err("the renderer for https://example.com is gone: it exited".to_owned()),
            "a dead tab said something a person could not be shown",
        );
    }

    // --- What closing a tab means --------------------------------------------

    /// The whole of what this file contributes to reaping: which sites still
    /// have a tab on them. What that costs a process is
    /// [`Renderers::reap`]'s, and `tests/a_renderer_nothing_wants.rs` watches
    /// the process go.
    #[test]
    fn a_site_is_wanted_while_any_tab_on_it_is_open_and_not_after() {
        let mut tabs = nowhere();
        let statement = tabs.open(url("https://a.bank.example/statement"));
        let settings = tabs.open(url("https://b.bank.example/settings"));
        let _news = tabs.open(url("https://news.example/"));
        let bank = site("https://bank.example/");

        assert!(tabs.sites_open().contains(&bank));
        assert!(tabs.close(statement));
        assert!(
            tabs.sites_open().contains(&bank),
            "closing one of two tabs on a site gave the site up",
        );
        assert!(tabs.close(settings));
        assert!(
            !tabs.sites_open().contains(&bank),
            "a site with no tab left open on it was still wanted",
        );
        assert!(tabs.sites_open().contains(&site("https://news.example/")));
    }

    /// A tab whose renderer has gone is still a tab somebody has open, so it
    /// still names its site — and closing *it* is what gives the site up.
    #[test]
    fn a_tab_whose_renderer_went_still_wants_its_site_until_it_is_closed() {
        let mut tabs = nowhere();
        let here = tabs.open(url("https://example.com/"));
        let _ = tabs.lost(
            &site("https://example.com/"),
            gone_at("https://example.com"),
        );

        assert!(tabs.tab(here).is_some_and(|tab| !tab.is_live()));
        assert!(tabs.sites_open().contains(&site("https://example.com/")));
        assert!(tabs.close(here));
        assert!(tabs.sites_open().is_empty());
    }

    // --- The decision, on its own --------------------------------------------

    fn asking(id: TabId) -> Asking<'static> {
        Asking {
            id,
            deliberate: false,
            gone: None,
            holder: None,
            running: false,
        }
    }

    #[test]
    fn nothing_is_asked_on_behalf_of_a_tab_whose_renderer_has_gone() {
        let gone = gone_at("https://example.com");
        let refused = may_ask(
            &Asking {
                gone: Some(&gone),
                ..asking(TabId(0))
            },
            &site("https://example.com/"),
        );
        assert_eq!(refused, Err(Lost::Gone(gone)));
    }

    /// Reloading is the user's to ask for, and this is the sentence that makes
    /// it possible: a load passes every refusal above.
    #[test]
    fn a_deliberate_load_is_allowed_where_nothing_else_is() {
        let gone = gone_at("https://example.com");
        assert_eq!(
            may_ask(
                &Asking {
                    deliberate: true,
                    gone: Some(&gone),
                    holder: Some(TabId(9)),
                    ..asking(TabId(0))
                },
                &site("https://example.com/"),
            ),
            Ok(()),
        );
    }

    #[test]
    fn a_tab_whose_page_its_renderer_no_longer_holds_is_not_answered_about() {
        assert_eq!(
            may_ask(
                &Asking {
                    holder: Some(TabId(9)),
                    running: true,
                    ..asking(TabId(0))
                },
                &site("https://example.com/"),
            ),
            Err(Lost::HoldsAnotherPage { holder: TabId(9) }),
        );
        assert_eq!(
            Lost::HoldsAnotherPage { holder: TabId(9) }.to_string(),
            "its renderer is holding tab#9's page, and a renderer holds one page",
        );
    }

    /// A renderer evicted to stay under the ceiling went away without dying,
    /// and a tab that asked would otherwise have started a fresh one and been
    /// told nothing is loaded — which is the silent restart by another road.
    #[test]
    fn a_tab_whose_renderer_was_stopped_is_told_rather_than_given_a_new_one() {
        let refused = may_ask(
            &Asking {
                holder: Some(TabId(0)),
                running: false,
                ..asking(TabId(0))
            },
            &site("https://example.com/"),
        );
        assert_eq!(
            refused,
            Err(Lost::Gone(Gone {
                site: "https://example.com".to_owned(),
                why: "it was stopped".to_owned(),
            })),
        );
    }

    #[test]
    fn a_tab_holding_its_own_page_in_a_running_renderer_is_asked() {
        assert_eq!(
            may_ask(
                &Asking {
                    holder: Some(TabId(0)),
                    running: true,
                    ..asking(TabId(0))
                },
                &site("https://example.com/"),
            ),
            Ok(()),
        );
    }

    /// Nothing has been loaded on this site at all, so the renderer answers it
    /// better than a rule here would: it says nothing is loaded.
    #[test]
    fn a_tab_on_a_site_nothing_has_loaded_is_left_to_the_renderer_to_answer() {
        assert_eq!(
            may_ask(&asking(TabId(0)), &site("https://example.com/")),
            Ok(()),
        );
    }
}
