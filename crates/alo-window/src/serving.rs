/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Answering an agent outside this browser from the tabs (ADR 0055 §§ 2, 5
//! and 7).
//!
//! Run on the conductor's thread, which holds the tabs, so a verb reaches a
//! renderer over the boundary that already carries it and nothing new crosses
//! into one. Every request is written into the [`AgentRecord`] here, whatever
//! it came to.
//!
//! # A tab is named by its address
//!
//! Never by a number: an index is the coordinate ADR 0002 refuses, a guess
//! about a strip that may have changed between reading and acting. The
//! address is read as a URL, so `https://Example.com` names the tab at
//! `https://example.com/`. Two tabs at one address refuse as
//! [`Refused::Ambiguous`].
//!
//! # Before any tab is looked at
//!
//! ADR 0055 § 6: what a request is refused for without a page — a line that
//! is not a request, a name that is not an address, an address on a site
//! agents are kept from — is refused before any tab is looked at, so a
//! refusal cannot say what a person has open beyond what the caller already
//! named. The grant that comes first of all is the socket's: only what can
//! open its path reaches this at all ([`crate::socket_place`]).
//!
//! # Kept to the sites a person named
//!
//! Queue item 497 and ADR 0055 § 8: under [`AgentSites::Only`], an address
//! on another site is refused as [`Refused::NotGranted`], by its site and
//! before its tab is found — two tabs there, or none, are refused alike.
//! The site judged is that of **the page the tab shows**, never of where it
//! was opened: a tab is found by its page's address, so a tab opened on a
//! named site that was redirected, or that navigated itself, somewhere else
//! is named only by where it is now, and refused there. A list of the tabs
//! leaves out every one an agent may not reach, so it cannot learn what else
//! is open. And every entry in the record says the site of the address
//! named, refused or not.

use crate::agent_record::{AgentRecord, Caller, Did, Entry};
use crate::agent_sites::AgentSites;
use crate::asking::{Answer, Ask, MOST_DEPTH, Refused};
use alo_agent::{Target, Verb};
use alo_renderer::site::Site;
use alo_renderer::{FromRenderer, Lost, Snapshot, SnapshotNode, Tab, TabId, Tabs};
use alo_url::Url;

/// What came of one request.
#[derive(Debug, Clone, PartialEq)]
pub struct Served {
    /// What the caller is told.
    pub answer: Answer,
    /// The tab a verb ran in, which the window should draw again: the verb
    /// may have changed its page.
    pub acted: Option<TabId>,
}

/// Answer `ask` from `tabs` on behalf of `caller`, kept to `sites`, and
/// record it.
pub fn serve(
    tabs: &mut Tabs,
    record: &mut AgentRecord,
    sites: &AgentSites,
    caller: Caller,
    ask: Result<Ask, Refused>,
) -> Served {
    let named = ask.as_ref().ok().and_then(Ask::tab).map(str::to_owned);
    let mut site = None;
    let (answer, did, acted) = match ask {
        Err(refused) => refusal(refused),
        Ok(Ask::Tabs) => {
            let addresses: Vec<String> = tabs
                .all()
                .iter()
                .map(shown)
                .filter(|address| sites.allows(address))
                .map(ToString::to_string)
                .collect();
            let did = Did::Listed {
                tabs: addresses.len(),
            };
            (Answer::Tabs(addresses), did, None)
        }
        Ok(Ask::Read { tab }) => {
            let (found, at) = reached(tabs, sites, &tab);
            site = at;
            match found {
                Err(refused) => refusal(refused),
                Ok(id) => {
                    let (answer, did) = read(tabs, id, tab);
                    (answer, did, None)
                }
            }
        }
        Ok(Ask::Act { tab, target, verb }) => {
            let (found, at) = reached(tabs, sites, &tab);
            site = at;
            match found {
                Err(refused) => refusal(refused),
                Ok(id) => {
                    let (answer, did) = act(tabs, id, tab, &target, &verb);
                    (answer, did, Some(id))
                }
            }
        }
    };
    record.write(Entry {
        caller,
        tab: named,
        site,
        did,
    });
    Served { answer, acted }
}

/// A request refused before any page answered.
fn refusal(refused: Refused) -> (Answer, Did, Option<TabId>) {
    (
        Answer::Refused(refused.clone()),
        Did::Refused(refused),
        None,
    )
}

/// The address a tab is named by: its page's, or where it was opened if
/// nothing has loaded.
fn shown(tab: &Tab) -> &Url {
    tab.address().unwrap_or_else(|| tab.url())
}

/// The one open tab at `tab`, if an agent kept to `sites` may reach it —
/// and, whenever `tab` is an address, its site.
///
/// In this order, so that a refusal says nothing about what is open: the
/// address read, then its site judged, and only then the tabs looked at.
fn reached(tabs: &Tabs, sites: &AgentSites, tab: &str) -> (Result<TabId, Refused>, Option<Site>) {
    let Ok(address) = alo_url::parse(tab) else {
        let refused = Refused::NotAnAddress {
            tab: tab.to_owned(),
        };
        return (Err(refused), None);
    };
    match sites.reach(&address) {
        Err(site) => {
            let refused = Refused::NotGranted {
                tab: tab.to_owned(),
                site: site.clone(),
            };
            (Err(refused), Some(site))
        }
        Ok(site) => (find(tabs, &address, tab), Some(site)),
    }
}

/// The one open tab at `address`, which the caller wrote as `tab`.
fn find(tabs: &Tabs, address: &Url, tab: &str) -> Result<TabId, Refused> {
    let address = address.to_string();
    let at: Vec<TabId> = tabs
        .all()
        .iter()
        .filter(|open| shown(open).to_string() == address)
        .map(Tab::id)
        .collect();
    match at.as_slice() {
        [] => Err(Refused::NoSuchTab {
            tab: tab.to_owned(),
        }),
        [one] => Ok(*one),
        many => Err(Refused::Ambiguous {
            tab: tab.to_owned(),
            count: many.len(),
        }),
    }
}

/// Refused when nothing is loaded in tab `id` — said without asking its
/// renderer, which asking would start.
fn loaded(tabs: &Tabs, id: TabId, tab: &str) -> Result<(), Refused> {
    if tabs.tab(id).and_then(Tab::document).is_none() {
        return Err(Refused::NothingLoaded {
            tab: tab.to_owned(),
        });
    }
    Ok(())
}

/// Read the page in tab `id`.
fn read(tabs: &mut Tabs, id: TabId, tab: String) -> (Answer, Did) {
    if let Err(refused) = loaded(tabs, id, &tab) {
        return (Answer::Refused(refused.clone()), Did::Refused(refused));
    }
    let document = tabs.tab(id).and_then(Tab::document);
    let refused = match tabs.read(id) {
        Ok(FromRenderer::Tree(snapshot)) if deepest(&snapshot) > MOST_DEPTH => {
            Refused::TreeTooDeep {
                tab,
                most: MOST_DEPTH,
            }
        }
        Ok(FromRenderer::Tree(snapshot)) => {
            let did = Did::Read {
                document,
                nodes: snapshot.nodes().len(),
            };
            return (Answer::Tree { tab, snapshot }, did);
        }
        answered => gone(tabs, id, tab, answered),
    };
    (Answer::Refused(refused.clone()), Did::Refused(refused))
}

/// Run `verb` on `target` in the page in tab `id`.
fn act(tabs: &mut Tabs, id: TabId, tab: String, target: &Target, verb: &Verb) -> (Answer, Did) {
    if let Err(refused) = loaded(tabs, id, &tab) {
        return (Answer::Refused(refused.clone()), Did::Refused(refused));
    }
    let document = tabs.tab(id).and_then(Tab::document);
    let (action, answered) = match tabs.act(id, target.clone(), verb.clone()) {
        Ok((action, answered)) => (action, answered),
        Err(lost) => {
            let refused = gone(tabs, id, tab, Err(lost));
            return (Answer::Refused(refused.clone()), Did::Refused(refused));
        }
    };
    let acted = |came_to| Did::Acted {
        action,
        document,
        target: target.clone(),
        verb: verb.clone(),
        came_to,
    };
    match answered {
        FromRenderer::Acted {
            outcome, issues, ..
        } => {
            let did = acted(Ok(outcome.clone()));
            let answer = Answer::Done {
                tab,
                outcome,
                issues,
            };
            (answer, did)
        }
        FromRenderer::Refused(refusal) => {
            let did = acted(Err(refusal.clone()));
            (Answer::Refused(Refused::Verb { tab, refusal }), did)
        }
        other => {
            let refused = gone(tabs, id, tab, Ok(other));
            (Answer::Refused(refused.clone()), Did::Refused(refused))
        }
    }
}

/// Why tab `id` could not answer, for an answer that was not the one asked
/// for: in the tab's own words when it has them.
fn gone(tabs: &Tabs, id: TabId, tab: String, answered: Result<FromRenderer, Lost>) -> Refused {
    let said = match answered {
        Ok(FromRenderer::Failed(failure)) => failure.to_string(),
        Ok(_) => "its renderer answered something it was not asked".to_owned(),
        Err(lost) => tabs
            .tab(id)
            .and_then(Tab::what_happened)
            .unwrap_or_else(|| lost.to_string()),
    };
    Refused::TabGone { tab, said }
}

/// How many nodes deep a tree is, counted without recursing, so a tree too
/// deep to answer is measured safely.
fn deepest(snapshot: &Snapshot) -> usize {
    let mut deepest = 0;
    let mut waiting: Vec<(&SnapshotNode, usize)> =
        snapshot.root.iter().map(|root| (root, 1)).collect();
    while let Some((node, depth)) = waiting.pop() {
        deepest = deepest.max(depth);
        waiting.extend(node.children.iter().map(|child| (child, depth + 1)));
    }
    deepest
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_renderer::host::Renderers;
    use std::path::PathBuf;

    /// Tabs whose renderers could never start: everything here is answered
    /// before one would be asked for.
    fn tabs() -> Tabs {
        Tabs::over(Renderers::running(
            PathBuf::from("/nowhere/alo-renderer"),
            &[],
        ))
    }

    fn served(tabs: &mut Tabs, record: &mut AgentRecord, ask: Ask) -> Answer {
        serve(tabs, record, &AgentSites::Every, Caller(0), Ok(ask)).answer
    }

    fn url(address: &str) -> alo_url::Url {
        alo_url::parse(address).unwrap_or_else(|_| alo_url::Url::about_blank())
    }

    #[test]
    fn a_tab_is_named_by_its_address_and_two_at_one_are_not_picked_between() {
        let (mut tabs, mut record) = (tabs(), AgentRecord::new());
        tabs.open(url("https://example.com/"));
        tabs.open(url("https://example.org/a"));
        tabs.open(url("https://example.org/a"));
        assert_eq!(
            served(&mut tabs, &mut record, Ask::Tabs),
            Answer::Tabs(vec![
                "https://example.com/".to_owned(),
                "https://example.org/a".to_owned(),
                "https://example.org/a".to_owned(),
            ])
        );
        let read = |tab: &str| Ask::Read {
            tab: tab.to_owned(),
        };
        assert_eq!(
            served(&mut tabs, &mut record, read("https://EXAMPLE.com")),
            Answer::Refused(Refused::NothingLoaded {
                tab: "https://EXAMPLE.com".to_owned()
            }),
            "read as an address, and answered without a renderer"
        );
        assert_eq!(
            served(&mut tabs, &mut record, read("https://example.org/a")),
            Answer::Refused(Refused::Ambiguous {
                tab: "https://example.org/a".to_owned(),
                count: 2
            })
        );
        assert_eq!(
            served(&mut tabs, &mut record, read("https://example.net/")),
            Answer::Refused(Refused::NoSuchTab {
                tab: "https://example.net/".to_owned()
            })
        );
        assert_eq!(
            served(&mut tabs, &mut record, read("not an address")),
            Answer::Refused(Refused::NotAnAddress {
                tab: "not an address".to_owned()
            })
        );
        assert_eq!(record.entries().count(), 5, "every request is recorded");
    }

    /// Kept to `https://example.com`, as a person would name it.
    fn kept() -> AgentSites {
        AgentSites::named(["https://example.com"]).unwrap_or_default()
    }

    #[test]
    fn a_site_not_named_is_refused_by_its_site_before_any_tab_is_looked_at() {
        let (mut tabs, mut record, sites) = (tabs(), AgentRecord::new(), kept());
        assert_ne!(sites, AgentSites::Every);
        tabs.open(url("https://example.org/a"));
        tabs.open(url("https://example.org/a"));
        let mut ask = |tab: &str| {
            let read = Ask::Read {
                tab: tab.to_owned(),
            };
            serve(&mut tabs, &mut record, &sites, Caller(0), Ok(read)).answer
        };
        let refused = |tab: &str| {
            Answer::Refused(Refused::NotGranted {
                tab: tab.to_owned(),
                site: Site::of(&url(tab)),
            })
        };
        // Two tabs open at one, none at the other: refused alike, so the
        // refusal cannot say which.
        assert_eq!(
            ask("https://example.org/a"),
            refused("https://example.org/a")
        );
        assert_eq!(
            ask("https://example.org/b"),
            refused("https://example.org/b")
        );
        assert_eq!(ask("http://example.com/"), refused("http://example.com/"));
        // A site named, and the tab looked for only then.
        assert_eq!(
            ask("https://www.example.com/"),
            Answer::Refused(Refused::NoSuchTab {
                tab: "https://www.example.com/".to_owned()
            })
        );
        let sites: Vec<Option<String>> = record
            .entries()
            .map(|entry| entry.site.as_ref().map(ToString::to_string))
            .collect();
        assert_eq!(
            sites,
            [
                Some("https://example.org".to_owned()),
                Some("https://example.org".to_owned()),
                Some("http://example.com".to_owned()),
                Some("https://example.com".to_owned()),
            ],
            "the site is in every entry, refused or not"
        );
    }

    #[test]
    fn a_verb_on_a_site_not_named_never_reaches_a_tab() {
        let (mut tabs, mut record, sites) = (tabs(), AgentRecord::new(), kept());
        tabs.open(url("https://example.org/"));
        let act = Ask::Act {
            tab: "https://example.org/".to_owned(),
            target: Target::Named("Buy".to_owned()),
            verb: Verb::Activate,
        };
        let served = serve(&mut tabs, &mut record, &sites, Caller(1), Ok(act));
        assert_eq!(served.acted, None, "nothing to draw again: nothing ran");
        assert!(matches!(
            served.answer,
            Answer::Refused(Refused::NotGranted { .. })
        ));
        assert!(matches!(
            record.entries().next().map(|entry| &entry.did),
            Some(Did::Refused(Refused::NotGranted { .. }))
        ));
    }

    #[test]
    fn the_tabs_listed_are_only_those_an_agent_may_reach() {
        let (mut tabs, mut record, sites) = (tabs(), AgentRecord::new(), kept());
        tabs.open(url("https://example.org/private"));
        tabs.open(url("https://shop.example.com/"));
        tabs.open(url("data:text/html,hi"));
        let listed = serve(&mut tabs, &mut record, &sites, Caller(0), Ok(Ask::Tabs));
        assert_eq!(
            listed.answer,
            Answer::Tabs(vec!["https://shop.example.com/".to_owned()])
        );
        assert!(matches!(
            record.entries().next(),
            Some(Entry {
                site: None,
                did: Did::Listed { tabs: 1 },
                ..
            })
        ));
    }

    #[test]
    fn a_name_that_is_not_an_address_has_no_site() {
        let (mut tabs, mut record, sites) = (tabs(), AgentRecord::new(), kept());
        let read = Ask::Read {
            tab: "not an address".to_owned(),
        };
        let served = serve(&mut tabs, &mut record, &sites, Caller(0), Ok(read));
        assert!(matches!(
            served.answer,
            Answer::Refused(Refused::NotAnAddress { .. })
        ));
        assert_eq!(
            record.entries().next().and_then(|entry| entry.site.clone()),
            None
        );
    }

    #[test]
    fn a_request_that_is_not_one_is_recorded_as_refused() {
        let (mut tabs, mut record) = (tabs(), AgentRecord::new());
        let served = serve(
            &mut tabs,
            &mut record,
            &AgentSites::Every,
            Caller(4),
            Err(Refused::NotJson),
        );
        assert_eq!(served.answer, Answer::Refused(Refused::NotJson));
        assert_eq!(served.acted, None);
        let entry = record.entries().next().cloned();
        assert_eq!(
            entry,
            Some(Entry {
                caller: Caller(4),
                tab: None,
                site: None,
                did: Did::Refused(Refused::NotJson)
            })
        );
    }

    #[test]
    fn a_tree_is_measured_without_recursing() {
        let leaf = |children| SnapshotNode {
            id: alo_box::BoxId::from_index_for_tests(0),
            role: alo_box::Role::Generic,
            name: None,
            states: alo_box::States::default(),
            placeholder: None,
            marker: None,
            rect: alo_layout::Rect::default(),
            rects: Vec::new(),
            offscreen: false,
            scrolls: false,
            children,
        };
        let mut node = leaf(Vec::new());
        for _ in 0..MOST_DEPTH {
            node = leaf(vec![node]);
        }
        let snapshot = Snapshot { root: Some(node) };
        assert_eq!(deepest(&snapshot), MOST_DEPTH + 1);
        assert_eq!(deepest(&Snapshot::default()), 0);
    }
}
