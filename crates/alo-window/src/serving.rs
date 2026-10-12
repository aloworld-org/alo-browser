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
//! is not a request, a name that is not an address — is refused before any
//! tab is looked at, so a refusal cannot say what a person has open beyond
//! what the caller already named. The grant that comes first of all is the
//! socket's: only what can open its path reaches this at all
//! ([`crate::socket_place`]). A grant per site is item 497's, and goes first
//! here when it comes.

use crate::agent_record::{AgentRecord, Caller, Did, Entry};
use crate::asking::{Answer, Ask, MOST_DEPTH, Refused};
use alo_agent::{Target, Verb};
use alo_renderer::{FromRenderer, Lost, Snapshot, SnapshotNode, Tab, TabId, Tabs};

/// What came of one request.
#[derive(Debug, Clone, PartialEq)]
pub struct Served {
    /// What the caller is told.
    pub answer: Answer,
    /// The tab a verb ran in, which the window should draw again: the verb
    /// may have changed its page.
    pub acted: Option<TabId>,
}

/// Answer `ask` from `tabs` on behalf of `caller`, and record it.
pub fn serve(
    tabs: &mut Tabs,
    record: &mut AgentRecord,
    caller: Caller,
    ask: Result<Ask, Refused>,
) -> Served {
    let named = ask.as_ref().ok().and_then(Ask::tab).map(str::to_owned);
    let (answer, did, acted) = match ask {
        Err(refused) => (
            Answer::Refused(refused.clone()),
            Did::Refused(refused),
            None,
        ),
        Ok(Ask::Tabs) => {
            let addresses: Vec<String> = tabs.all().iter().map(address_of).collect();
            let did = Did::Listed {
                tabs: addresses.len(),
            };
            (Answer::Tabs(addresses), did, None)
        }
        Ok(Ask::Read { tab }) => match find(tabs, &tab) {
            Err(refused) => (
                Answer::Refused(refused.clone()),
                Did::Refused(refused),
                None,
            ),
            Ok(id) => {
                let (answer, did) = read(tabs, id, tab);
                (answer, did, None)
            }
        },
        Ok(Ask::Act { tab, target, verb }) => match find(tabs, &tab) {
            Err(refused) => (
                Answer::Refused(refused.clone()),
                Did::Refused(refused),
                None,
            ),
            Ok(id) => {
                let (answer, did) = act(tabs, id, tab, &target, &verb);
                (answer, did, Some(id))
            }
        },
    };
    record.write(Entry {
        caller,
        tab: named,
        did,
    });
    Served { answer, acted }
}

/// The address a tab is named by: its page's, or where it was opened if
/// nothing has loaded.
fn address_of(tab: &Tab) -> String {
    tab.address().unwrap_or_else(|| tab.url()).to_string()
}

/// The one open tab at `tab`.
fn find(tabs: &Tabs, tab: &str) -> Result<TabId, Refused> {
    let Ok(address) = alo_url::parse(tab) else {
        return Err(Refused::NotAnAddress {
            tab: tab.to_owned(),
        });
    };
    let address = address.to_string();
    let at: Vec<TabId> = tabs
        .all()
        .iter()
        .filter(|open| address_of(open) == address)
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
        serve(tabs, record, Caller(0), Ok(ask)).answer
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

    #[test]
    fn a_request_that_is_not_one_is_recorded_as_refused() {
        let (mut tabs, mut record) = (tabs(), AgentRecord::new());
        let served = serve(&mut tabs, &mut record, Caller(4), Err(Refused::NotJson));
        assert_eq!(served.answer, Answer::Refused(Refused::NotJson));
        assert_eq!(served.acted, None);
        let entry = record.entries().next().cloned();
        assert_eq!(
            entry,
            Some(Entry {
                caller: Caller(4),
                tab: None,
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
