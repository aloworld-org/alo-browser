/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The tabs as the strip shows them: which one is selected, what each is
//! called, and the renderer that draws them (queue item 528).
//!
//! # Selected, beside the order
//!
//! ADR 0024 § 6: *"`Tabs` gains a **selected** tab and an order, both the
//! browser process's, and both part of the state § 4 sends."* The order is
//! the order tabs were opened in, which is [`Tabs::all`]'s. Which one is
//! selected is kept here rather than by whoever drives the window, because
//! closing a tab has to choose another and closing is this type's to do.
//! The tabs either side of the selected one ([`Tabs::next`],
//! [`Tabs::previous`]) are read here too, going round the order, for the
//! browser's keys (queue item 529).
//!
//! # The strip's renderer is the tabs', and never reaped
//!
//! § 4: the strip is drawn by a renderer of its own, at a site no page can
//! name ([`crate::strip::site`], minted once per [`Tabs`]). It lives among
//! the same [`crate::host::Renderers`] as every page's, under the same
//! confinement, and its site is among those [`Tabs::close`] says are wanted
//! while any tab is open, so reaping a closed tab's renderer never stops it.
//! Closing the last tab does, as closing the window stops every renderer.
//!
//! The strip holds no state of its own: each [`Strip`] is all of it, sent
//! whole. So a strip renderer that the ceiling on renderers evicted is started
//! again by the next drawing, and loses nothing.
//!
//! # A strip whose renderer has gone keeps its last frame
//!
//! § 4: *"its renderer dying is a strip that keeps its last frame and says
//! so, as a tab does."* A strip renderer that died, or said nothing for as
//! long as any renderer is given, is not asked again: the last frame it
//! painted and the sentence saying what happened are what the window has, and
//! no strip is drawn by a fresh process behind the person's back (ADR 0005:
//! *"a browser that silently restarts a renderer hides a bug that somebody
//! needs to see"*).

use super::{Lost, Tab, TabId, Tabs};
use crate::frame::Frame;
use crate::host::Gone;
use crate::message::{FromRenderer, ToRenderer};
use crate::site::Site;
use crate::strip::{self, Condition, Strip, StripTab};
use alo_layout::Size;

/// The tab strip's renderer, as the tabs keep it.
#[derive(Debug, Clone)]
pub struct StripRenderer {
    /// Its site, which no page can name.
    site: Site,
    /// The last frame it painted.
    painted: Option<Frame>,
    /// What happened to it, if it has gone.
    gone: Option<Gone>,
}

impl StripRenderer {
    /// A strip that has drawn nothing yet, at a site minted for it.
    pub(super) fn new() -> Self {
        Self {
            site: strip::site(),
            painted: None,
            gone: None,
        }
    }

    /// The site its renderer runs at.
    pub fn site(&self) -> &Site {
        &self.site
    }

    /// The last frame it painted, kept when its renderer has gone.
    pub fn frame(&self) -> Option<&Frame> {
        self.painted.as_ref()
    }

    /// What happened to its renderer, if something did.
    pub fn gone(&self) -> Option<&Gone> {
        self.gone.as_ref()
    }

    /// What to tell a person about it, in words, when there is something.
    pub fn what_happened(&self) -> Option<String> {
        self.gone.as_ref().map(ToString::to_string)
    }
}

impl Tabs {
    /// The tab a person is looking at, if any is open.
    pub fn selected(&self) -> Option<TabId> {
        self.selected
    }

    /// Select tab `id`. Whether there is such a tab to select; when there is
    /// not, the selection is unchanged.
    pub fn select(&mut self, id: TabId) -> bool {
        if self.tab(id).is_none() {
            return false;
        }
        self.selected = Some(id);
        true
    }

    /// The tab after the selected one in the order, or the first when the
    /// selected one is last; [`None`] when nothing is selected (queue item
    /// 529). With one tab open it is that tab.
    pub fn next(&self) -> Option<TabId> {
        let at = self.selected_at()?;
        let after = at.checked_add(1).filter(|after| *after < self.list.len());
        self.list.get(after.unwrap_or(0)).map(Tab::id)
    }

    /// The tab before the selected one in the order, or the last when the
    /// selected one is first; [`None`] when nothing is selected.
    pub fn previous(&self) -> Option<TabId> {
        let at = self.selected_at()?;
        let before = at
            .checked_sub(1)
            .or_else(|| self.list.len().checked_sub(1))?;
        self.list.get(before).map(Tab::id)
    }

    /// Where the selected tab is in the order.
    fn selected_at(&self) -> Option<usize> {
        let selected = self.selected?;
        self.list.iter().position(|tab| tab.id == selected)
    }

    /// Tab `id` has been taken out of the order from place `at`: when it was
    /// the selected one, select the tab now in its place — the one after it —
    /// or the one before when it was last.
    pub(super) fn closed_at(&mut self, id: TabId, at: usize) {
        if self.selected != Some(id) {
            return;
        }
        let neighbour = self
            .list
            .get(at)
            .or_else(|| at.checked_sub(1).and_then(|before| self.list.get(before)));
        self.selected = neighbour.map(Tab::id);
    }

    /// The strip as it is now, at `viewport`: every tab in order, each with
    /// its title and the address it shows, which one is selected, and each
    /// one's condition — gone when its renderer has, loading when `loading`
    /// says so, shown otherwise.
    ///
    /// Whether a tab is loading is the caller's to say, because a load is
    /// something the caller is in the middle of, and these tabs only learn of
    /// it when it has answered.
    pub fn strip(&self, viewport: Size, loading: impl Fn(TabId) -> bool) -> Strip {
        let tabs = self
            .list
            .iter()
            .map(|tab| {
                let condition = if tab.gone.is_some() {
                    Condition::Gone
                } else if loading(tab.id) {
                    Condition::Loading
                } else {
                    Condition::Shown
                };
                let shown = StripTab::new(
                    tab.title.as_str(),
                    tab.address().unwrap_or(&tab.url).to_string(),
                )
                .in_condition(condition);
                if self.selected == Some(tab.id) {
                    shown.selected()
                } else {
                    shown
                }
            })
            .collect();
        Strip { tabs, viewport }
    }

    /// The strip's renderer, and what it last painted.
    pub fn strip_renderer(&self) -> &StripRenderer {
        &self.strip
    }

    /// Draw `strip` in the strip's renderer and paint it, keeping the frame.
    ///
    /// The answer is the paint's — [`FromRenderer::Painted`] — or, when the
    /// strip could not be built, what the renderer said instead.
    ///
    /// # Errors
    ///
    /// [`Lost::Gone`] when its renderer died or stopped answering, now or
    /// before: a strip whose renderer has gone is not drawn again, and keeps
    /// the last frame it painted.
    pub fn draw_strip(&mut self, strip: &Strip) -> Result<FromRenderer, Lost> {
        if let Some(gone) = &self.strip.gone {
            return Err(Lost::Gone(gone.clone()));
        }
        let site = self.strip.site.clone();
        let built = match self
            .renderers
            .ask(&site, &ToRenderer::Strip(Box::new(strip.clone())))
        {
            Ok(built) => built,
            Err(gone) => return Err(self.strip_lost(gone)),
        };
        if !matches!(built, FromRenderer::Loaded { .. }) {
            return Ok(built);
        }
        match self.renderers.ask(&site, &ToRenderer::Paint) {
            Ok(FromRenderer::Painted(frame)) => {
                self.strip.painted = Some(frame.clone());
                Ok(FromRenderer::Painted(frame))
            }
            Ok(other) => Ok(other),
            Err(gone) => Err(self.strip_lost(gone)),
        }
    }

    /// The strip's renderer has gone: keep what happened, and say it.
    fn strip_lost(&mut self, gone: Gone) -> Lost {
        self.strip.gone = Some(gone.clone());
        Lost::Gone(gone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Renderers;
    use alo_url::Url;

    /// Tabs over renderers that can never start: nothing here asks one.
    fn tabs() -> Tabs {
        Tabs::over(Renderers::running("/nonexistent/alo-render", &[]))
    }

    fn url(address: &str) -> Url {
        alo_url::parse(address).unwrap_or_else(|_| Url::about_blank())
    }

    fn selected_titles(tabs: &Tabs) -> Vec<(String, bool)> {
        tabs.strip(Size::new(100.0, 36.0), |_| false)
            .tabs
            .into_iter()
            .map(|tab| (tab.address, tab.selected))
            .collect()
    }

    #[test]
    fn nothing_is_selected_until_a_tab_is() {
        let mut tabs = tabs();
        assert_eq!(tabs.selected(), None);
        let first = tabs.open(url("https://a.example/"));
        assert_eq!(tabs.selected(), None);
        assert!(tabs.select(first));
        assert_eq!(tabs.selected(), Some(first));
    }

    #[test]
    fn a_tab_nobody_has_open_is_not_selected() {
        let mut tabs = tabs();
        let first = tabs.open(url("https://a.example/"));
        assert!(tabs.select(first));
        assert!(tabs.close(first));
        assert!(!tabs.select(first), "a closed tab is not selected");
        assert_eq!(tabs.selected(), None);
    }

    #[test]
    fn closing_the_selected_tab_selects_the_one_after_it_or_else_before() {
        let mut tabs = tabs();
        let a = tabs.open(url("https://a.example/"));
        let b = tabs.open(url("https://b.example/"));
        let c = tabs.open(url("https://c.example/"));
        assert!(tabs.select(b));
        assert!(tabs.close(b));
        assert_eq!(tabs.selected(), Some(c), "the one after it");
        assert!(tabs.close(c));
        assert_eq!(tabs.selected(), Some(a), "the one before, as it was last");
        assert!(tabs.close(a));
        assert_eq!(tabs.selected(), None, "and none when none is open");
    }

    #[test]
    fn the_next_and_the_previous_tab_go_round_the_order() {
        let mut tabs = tabs();
        assert_eq!((tabs.next(), tabs.previous()), (None, None));
        let a = tabs.open(url("https://a.example/"));
        assert_eq!(tabs.next(), None, "nothing is selected");
        assert!(tabs.select(a));
        assert_eq!((tabs.next(), tabs.previous()), (Some(a), Some(a)));
        let b = tabs.open(url("https://b.example/"));
        let c = tabs.open(url("https://c.example/"));
        assert_eq!((tabs.next(), tabs.previous()), (Some(b), Some(c)));
        assert!(tabs.select(b));
        assert_eq!((tabs.next(), tabs.previous()), (Some(c), Some(a)));
        assert!(tabs.select(c));
        assert_eq!((tabs.next(), tabs.previous()), (Some(a), Some(b)));
    }

    #[test]
    fn closing_another_tab_keeps_the_selection() {
        let mut tabs = tabs();
        let a = tabs.open(url("https://a.example/"));
        let b = tabs.open(url("https://b.example/"));
        assert!(tabs.select(b));
        assert!(tabs.close(a));
        assert_eq!(tabs.selected(), Some(b));
    }

    #[test]
    fn the_strip_is_every_tab_in_order_with_the_selected_one_marked() {
        let mut tabs = tabs();
        let _ = tabs.open(url("https://a.example/"));
        let b = tabs.open(url("https://b.example/x"));
        let _ = tabs.open(Url::about_blank());
        assert!(tabs.select(b));
        assert_eq!(
            selected_titles(&tabs),
            vec![
                ("https://a.example/".to_owned(), false),
                ("https://b.example/x".to_owned(), true),
                ("about:blank".to_owned(), false),
            ]
        );
        let strip = tabs.strip(Size::new(640.0, 36.0), |id| id == b);
        assert_eq!(strip.viewport, Size::new(640.0, 36.0));
        let conditions: Vec<Condition> = strip.tabs.iter().map(|tab| tab.condition).collect();
        assert_eq!(
            conditions,
            vec![Condition::Shown, Condition::Loading, Condition::Shown]
        );
        // Nothing has loaded, so no tab has a title, and each is shown by
        // its address.
        assert!(strip.tabs.iter().all(|tab| tab.title.is_empty()));
        assert_eq!(
            strip.tabs.get(1).map(StripTab::label),
            Some("https://b.example/x")
        );
    }

    #[test]
    fn a_tab_whose_renderer_has_gone_is_gone_in_the_strip_even_while_loading() {
        let mut tabs = tabs();
        let a = tabs.open(url("https://a.example/"));
        if let Some(tab) = tabs.list.iter_mut().find(|tab| tab.id == a) {
            tab.gone = Some(Gone {
                site: "https://a.example".to_owned(),
                why: "it exited".to_owned(),
            });
        }
        let strip = tabs.strip(Size::new(100.0, 36.0), |_| true);
        assert_eq!(
            strip.tabs.first().map(|tab| tab.condition),
            Some(Condition::Gone)
        );
    }

    #[test]
    fn a_strip_whose_renderer_has_gone_is_not_asked_again() {
        let mut tabs = tabs();
        let gone = Gone {
            site: tabs.strip_renderer().site().to_string(),
            why: "it said nothing for 4s, so it was stopped".to_owned(),
        };
        let _ = tabs.strip_lost(gone.clone());
        let started = tabs.renderers().started();
        let drawn = tabs.draw_strip(&Strip::default());
        assert_eq!(drawn, Err(Lost::Gone(gone)));
        assert_eq!(tabs.renderers().started(), started, "nothing was started");
        assert!(tabs.strip_renderer().what_happened().is_some());
    }

    #[test]
    fn the_strips_site_is_wanted_while_any_tab_is_open() {
        let mut tabs = tabs();
        let site = tabs.strip_renderer().site().clone();
        let a = tabs.open(url("https://a.example/"));
        let b = tabs.open(url("https://b.example/"));
        assert!(tabs.sites_open().contains(&site));
        assert!(tabs.close(a));
        assert!(tabs.sites_open().contains(&site), "one tab is still open");
        assert!(tabs.close(b));
        assert!(!tabs.sites_open().contains(&site), "the last one closed");
    }
}
