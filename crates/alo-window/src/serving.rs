/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Answering an agent outside this browser from the tabs (ADR 0055 §§ 2, 5
//! and 7).
//!
//! Run on the thread that holds the tabs — the conductor's for this
//! engine's, so a verb reaches a renderer over the boundary that already
//! carries it and nothing new crosses into one, and
//! [`crate::chrome_serving`]'s for a tab in Chrome. Whichever renderer draws
//! them ([`crate::pages`]), everything below is done here, the same way and
//! in the same order (queue item 630). Every request is written into the
//! [`AgentRecord`] here, whatever it came to, with the renderer that drew the
//! page it reached.
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
//!
//! # Opened, found and saved (queue item 956, ADR 0112 § 4)
//!
//! `open` names where to go as well as which tab: the address to go to is
//! judged — an `http` or `https` one, on a site agents may reach — after
//! the tab's site and before any tab is looked at, so a refusal of either
//! says nothing about what is open. A tab sent there is named by where it
//! arrived; one a server sent on to a site agents may not reach is
//! answered as not opened for the agent, and never named. With no tab
//! named it asks for a new one, and so does `new_tab`, which only the
//! conversation opens ([`crate::conversation_agents`]): here both are
//! [`Refused::NotOffered`], after the address is judged.
//!
//! `find` is answered from the tree `read` answers, by name and never by
//! place ([`crate::agent_find`]). `print_pdf` judges its path against the
//! directories a person allowed ([`crate::agent_saves`]) where `open`
//! judges its address — before any tab — and the file is written here, by
//! the browser process, never by a renderer: one that answers with
//! something that is not a PDF has nothing written for it.

use crate::agent_record::{AgentRecord, Caller, Did, Entry};
use crate::agent_saves::{self, Unsaved};
use crate::agent_scope::AgentScope;
use crate::agent_sites::AgentSites;
use crate::asking::{Answer, Ask, MOST_DEPTH, Refused};
use crate::pages::{Acted, Pages, RenderedBy, Unanswered};
use alo_agent::{Target, Verb};
use alo_renderer::site::Site;
use alo_renderer::{Snapshot, SnapshotNode};
use alo_url::Url;

/// What came of one request.
#[derive(Debug, Clone, PartialEq)]
pub struct Served<T> {
    /// What the caller is told.
    pub answer: Answer,
    /// The tab a verb ran in, which the window should draw again: the verb
    /// may have changed its page.
    pub acted: Option<T>,
}

/// Answer `ask` from `pages` on behalf of `caller`, kept to `scope`, and
/// record it.
pub fn serve<P: Pages>(
    pages: &mut P,
    record: &mut AgentRecord,
    scope: &AgentScope,
    caller: Caller,
    ask: Result<Ask, Refused>,
) -> Served<P::Tab> {
    let sites = &scope.sites;
    let named = ask.as_ref().ok().and_then(Ask::tab).map(str::to_owned);
    let mut site = None;
    let mut rendered_by = None;
    let (answer, did, acted) = match ask {
        Err(refused) => refusal(refused),
        Ok(Ask::Tabs) => {
            pages.catch_up();
            let addresses: Vec<String> = pages
                .shown()
                .iter()
                .map(|(_, address)| address)
                .filter(|address| sites.allows(address))
                .map(ToString::to_string)
                .collect();
            let did = Did::Listed {
                tabs: addresses.len(),
            };
            (Answer::Tabs(addresses), did, None)
        }
        Ok(Ask::Read { tab }) => {
            let (found, at) = reached(pages, sites, &tab);
            site = at;
            match found {
                Err(refused) => refusal(refused),
                Ok(id) => {
                    rendered_by = Some(pages.rendered_by(id));
                    let (answer, did) = read(pages, id, tab);
                    (answer, did, None)
                }
            }
        }
        Ok(Ask::Renderer { tab }) => {
            let (found, at) = reached(pages, sites, &tab);
            site = at;
            match found {
                Err(refused) => refusal(refused),
                Ok(id) => {
                    // The browser process's own knowledge, never the page's
                    // word: no renderer is asked, so a tab with nothing
                    // loaded, or one whose renderer has not started, is
                    // answered all the same (ADR 0058 § 6, queue item 622).
                    let drawn = pages.rendered_by(id);
                    rendered_by = Some(drawn.clone());
                    let answer = Answer::Renderer {
                        tab,
                        rendered_by: drawn,
                    };
                    (answer, Did::AskedRenderer, None)
                }
            }
        }
        Ok(Ask::Act { tab, target, verb }) => {
            let (found, at) = reached(pages, sites, &tab);
            site = at;
            match found {
                Err(refused) => refusal(refused),
                Ok(id) => {
                    rendered_by = Some(pages.rendered_by(id));
                    let (answer, did) = act(pages, id, tab, &target, &verb);
                    (answer, did, Some(id))
                }
            }
        }
        Ok(
            ask @ (Ask::Open { .. }
            | Ask::NewTab
            | Ask::Launched { .. }
            | Ask::Find { .. }
            | Ask::PrintPdf { .. }),
        ) => {
            let came = opened_found_or_saved(pages, scope, ask);
            site = came.site;
            rendered_by = came.rendered_by;
            (came.answer, came.did, came.acted)
        }
    };
    record.write(Entry {
        caller,
        tab: named,
        site,
        rendered_by,
        did,
    });
    Served { answer, acted }
}

/// What came of an `open`, `new_tab`, `find` or `print_pdf` (queue item
/// 956): the answer and the record's entry, the tab a page was sent from,
/// and the site and renderer the entry names.
struct Came<T> {
    answer: Answer,
    did: Did,
    acted: Option<T>,
    site: Option<Site>,
    rendered_by: Option<RenderedBy>,
}

/// Answer `ask` — an `open`, `new_tab`, `find` or `print_pdf` — from
/// `pages`, kept to `scope`.
fn opened_found_or_saved<P: Pages>(pages: &mut P, scope: &AgentScope, ask: Ask) -> Came<P::Tab> {
    let sites = &scope.sites;
    let mut site = None;
    let mut rendered_by = None;
    let (answer, did, acted) = match ask {
        Ask::Open {
            tab: Some(tab),
            address,
        } => {
            let (found, at) = reached_after(pages, sites, &tab, || openable(sites, &address).0);
            site = at;
            match found {
                Err(refused) => refusal(refused),
                Ok((id, to)) => {
                    rendered_by = Some(pages.rendered_by(id));
                    let (answer, did) = go(pages, sites, id, &address, &to);
                    (answer, did, Some(id))
                }
            }
        }
        // Only the conversation opens a new tab, and answers these before
        // they reach here (`conversation_agents`); judged all the same.
        Ask::Open { tab: None, address } => {
            let (judged, at) = openable(sites, &address);
            site = at;
            refusal(judged.err().unwrap_or(Refused::NotOffered { ask: "open" }))
        }
        Ask::NewTab => refusal(Refused::NotOffered { ask: "new_tab" }),
        // A second launch is the conversation's to take (queue item 976):
        // judged all the same, and not offered where one tab is held.
        Ask::Launched { address } => {
            let (judged, at) = openable(sites, &address);
            site = at;
            refusal(
                judged
                    .err()
                    .unwrap_or(Refused::NotOffered { ask: "launched" }),
            )
        }
        Ask::Find { tab, text } => {
            let (found, at) = reached(pages, sites, &tab);
            site = at;
            match found {
                Err(refused) => refusal(refused),
                Ok(id) => {
                    rendered_by = Some(pages.rendered_by(id));
                    let (answer, did) = find_in(pages, id, tab, text);
                    (answer, did, None)
                }
            }
        }
        Ask::PrintPdf { tab, path } => {
            let (found, at) = reached_after(pages, sites, &tab, || {
                scope
                    .saves
                    .judge(&path)
                    .map_err(|why| unsaved(path.clone(), why))
            });
            site = at;
            match found {
                Err(refused) => refusal(refused),
                Ok((id, file)) => {
                    rendered_by = Some(pages.rendered_by(id));
                    let (answer, did) = print(pages, id, tab, &file);
                    (answer, did, None)
                }
            }
        }
        // Every other ask is `serve`'s own, and never handed here.
        other => refusal(Refused::NotOffered {
            ask: match other {
                Ask::Tabs => "tabs",
                Ask::Read { .. } => "read",
                Ask::Renderer { .. } => "renderer",
                _ => "act",
            },
        }),
    };
    Came {
        answer,
        did,
        acted,
        site,
        rendered_by,
    }
}

/// A request refused before any page answered.
fn refusal<T>(refused: Refused) -> (Answer, Did, Option<T>) {
    (
        Answer::Refused(refused.clone()),
        Did::Refused(refused),
        None,
    )
}

/// The one open tab at `tab`, if an agent kept to `sites` may reach it —
/// and, whenever `tab` is an address, its site.
///
/// In this order, so that a refusal says nothing about what is open: the
/// address read, then its site judged, and only then the tabs looked at.
fn reached<P: Pages>(
    pages: &mut P,
    sites: &AgentSites,
    tab: &str,
) -> (Result<P::Tab, Refused>, Option<Site>) {
    let (found, site) = reached_after(pages, sites, tab, || Ok(()));
    (found.map(|(id, ())| id), site)
}

/// The tab reached and what else was judged, or why not — and, whenever the
/// tab is named by an address, its site.
type Judged<Tab, T> = (Result<(Tab, T), Refused>, Option<Site>);

/// [`reached`], with what else the request is refused for without a page
/// — an address to go to, a path to save at — judged by `before` after the
/// tab's site and before any tab is looked at: the tab, and what `before`
/// answered.
fn reached_after<P: Pages, T>(
    pages: &mut P,
    sites: &AgentSites,
    tab: &str,
    before: impl FnOnce() -> Result<T, Refused>,
) -> Judged<P::Tab, T> {
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
        Ok(site) => {
            let judged = match before() {
                Ok(judged) => judged,
                Err(refused) => return (Err(refused), Some(site)),
            };
            pages.catch_up();
            (
                find(pages, &address, tab).map(|id| (id, judged)),
                Some(site),
            )
        }
    }
}

/// The page at `address`, if an agent kept to `sites` may open it: an
/// `http` or `https` address on a site it may reach — and, whenever it is
/// one, its site. Judged before any tab is looked at, here and by the
/// conversation's relay.
pub fn openable(sites: &AgentSites, address: &str) -> (Result<Url, Refused>, Option<Site>) {
    let not = || Refused::NotOpenable {
        address: address.to_owned(),
    };
    let Ok(url) = alo_url::parse(address) else {
        return (Err(not()), None);
    };
    if !matches!(url.scheme.as_str(), "http" | "https") {
        return (Err(not()), None);
    }
    match sites.reach(&url) {
        Err(site) => {
            let refused = Refused::NotGranted {
                tab: address.to_owned(),
                site: site.clone(),
            };
            (Err(refused), Some(site))
        }
        Ok(site) => (Ok(url), Some(site)),
    }
}

/// Send the page in tab `id` to `to`, which the caller wrote as `address`,
/// and name the tab where it arrived — when an agent kept to `sites` may
/// reach it there.
fn go<P: Pages>(
    pages: &mut P,
    sites: &AgentSites,
    id: P::Tab,
    address: &str,
    to: &Url,
) -> (Answer, Did) {
    let action = match pages.go(id, to) {
        Ok(action) => action,
        Err(Unanswered::NotOffered) => {
            let refused = Refused::NotOffered { ask: "open" };
            return (Answer::Refused(refused.clone()), Did::Refused(refused));
        }
        Err(why) => {
            let refused = Refused::NotOpened {
                address: address.to_owned(),
                said: unanswered_said(&why),
            };
            return (Answer::Refused(refused.clone()), Did::Refused(refused));
        }
    };
    pages.catch_up();
    let did = Did::Went {
        action,
        to: to.to_string(),
    };
    let arrived = pages
        .shown()
        .into_iter()
        .find(|(each, _)| *each == id)
        .map(|(_, at)| at);
    match arrived {
        Some(at) if sites.allows(&at) => (
            Answer::Opened {
                tab: at.to_string(),
            },
            did,
        ),
        // Sent on by its server to where agents are kept from: the page
        // went, and is not named to the agent.
        _ => {
            let refused = Refused::NotOpened {
                address: address.to_owned(),
                said: "it went on to a site agents may not reach".to_owned(),
            };
            (Answer::Refused(refused), did)
        }
    }
}

/// Look for `text` in the page in tab `id`.
fn find_in<P: Pages>(pages: &mut P, id: P::Tab, tab: String, text: String) -> (Answer, Did) {
    if let Err(refused) = loaded(pages, id, &tab) {
        return (Answer::Refused(refused.clone()), Did::Refused(refused));
    }
    let refused = match pages.read(id) {
        Ok(_) if !still_at(pages, id, &tab) => Refused::NoSuchTab { tab },
        Ok(snapshot) => {
            let found = crate::agent_find::find(&snapshot, &text);
            let did = Did::Found {
                document: pages.document(id),
                found: found.nodes.len() + found.more,
            };
            let answer = Answer::Found {
                tab,
                text,
                found: found.nodes,
                more: found.more,
            };
            return (answer, did);
        }
        Err(why) => unanswered(tab, why, "find"),
    };
    (Answer::Refused(refused.clone()), Did::Refused(refused))
}

/// Save the page in tab `id` as a PDF at `file`, which the person's
/// directories let be written.
fn print<P: Pages>(
    pages: &mut P,
    id: P::Tab,
    tab: String,
    file: &std::path::Path,
) -> (Answer, Did) {
    if let Err(refused) = loaded(pages, id, &tab) {
        return (Answer::Refused(refused.clone()), Did::Refused(refused));
    }
    let document = pages.document(id);
    let path = file.to_string_lossy().into_owned();
    let refused = match pages.print_pdf(id) {
        // Printed, and by then the tab shows another page: the PDF is not
        // of the address judged, and is not written.
        Ok(_) if !still_at(pages, id, &tab) => Refused::NoSuchTab { tab },
        Ok(bytes) if !bytes.starts_with(agent_saves::HEADER) => Refused::NotCarriedOut {
            tab,
            said: "what the renderer answered is not a PDF, and nothing was written".to_owned(),
        },
        Ok(bytes) => match agent_saves::save(file, &bytes) {
            Ok(()) => {
                let did = Did::Printed {
                    document,
                    path: path.clone(),
                    bytes: bytes.len(),
                };
                let answer = Answer::Printed {
                    tab,
                    path,
                    bytes: bytes.len(),
                };
                return (answer, did);
            }
            Err(why) => unsaved(path, why),
        },
        Err(why) => unanswered(tab, why, "print_pdf"),
    };
    (Answer::Refused(refused.clone()), Did::Refused(refused))
}

/// Why a page was not saved at `path`, as the kind that crosses.
fn unsaved(path: String, why: Unsaved) -> Refused {
    match why {
        Unsaved::NotAPath => Refused::NotAPath { path },
        Unsaved::NotAllowedThere => Refused::NotAllowedThere { path },
        Unsaved::Exists => Refused::Exists { path },
        Unsaved::NotWritten(said) => Refused::NotWritten { path, said },
    }
}

/// The one open tab at `address`, which the caller wrote as `tab`.
fn find<P: Pages>(pages: &P, address: &Url, tab: &str) -> Result<P::Tab, Refused> {
    let address = address.to_string();
    let at: Vec<P::Tab> = pages
        .shown()
        .into_iter()
        .filter(|(_, shown)| shown.to_string() == address)
        .map(|(id, _)| id)
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
fn loaded<P: Pages>(pages: &P, id: P::Tab, tab: &str) -> Result<(), Refused> {
    if pages.document(id).is_none() {
        return Err(Refused::NothingLoaded {
            tab: tab.to_owned(),
        });
    }
    Ok(())
}

/// Read the page in tab `id`.
fn read<P: Pages>(pages: &mut P, id: P::Tab, tab: String) -> (Answer, Did) {
    if let Err(refused) = loaded(pages, id, &tab) {
        return (Answer::Refused(refused.clone()), Did::Refused(refused));
    }
    let refused = match pages.read(id) {
        // Read, and by then the tab shows another page: the tree is not of
        // the address judged, and is not answered (queue item 630).
        Ok(_) if !still_at(pages, id, &tab) => Refused::NoSuchTab { tab },
        Ok(snapshot) if deepest(&snapshot) > MOST_DEPTH => Refused::TreeTooDeep {
            tab,
            most: MOST_DEPTH,
        },
        Ok(snapshot) => {
            let did = Did::Read {
                document: pages.document(id),
                nodes: snapshot.nodes().len(),
            };
            return (Answer::Tree { tab, snapshot }, did);
        }
        Err(why) => unanswered(tab, why, "read"),
    };
    (Answer::Refused(refused.clone()), Did::Refused(refused))
}

/// Whether tab `id` is still the one open tab at `tab`, the address it was
/// found at and its site judged by.
///
/// A renderer asked over a pipe goes on being told of things while it is
/// asked, and a page in Chrome may go elsewhere on its own — a script, a
/// refresh — at any moment. So a read that began on one page can end on the
/// next, on another site; this is asked once it has.
fn still_at<P: Pages>(pages: &P, id: P::Tab, tab: &str) -> bool {
    alo_url::parse(tab).is_ok_and(|address| find(pages, &address, tab) == Ok(id))
}

/// Run `verb` on `target` in the page in tab `id`.
fn act<P: Pages>(
    pages: &mut P,
    id: P::Tab,
    tab: String,
    target: &Target,
    verb: &Verb,
) -> (Answer, Did) {
    if let Err(refused) = loaded(pages, id, &tab) {
        return (Answer::Refused(refused.clone()), Did::Refused(refused));
    }
    let document = pages.document(id);
    let (action, acted) = match pages.act(id, target, verb) {
        Ok(answered) => answered,
        Err(why) => {
            let refused = unanswered(tab, why, "act");
            return (Answer::Refused(refused.clone()), Did::Refused(refused));
        }
    };
    let did = |came_to| Did::Acted {
        action,
        document,
        target: target.clone(),
        verb: verb.clone(),
        came_to,
    };
    match acted {
        Acted::Done { outcome, issues } => {
            let did = did(Ok(outcome.clone()));
            let answer = Answer::Done {
                tab,
                outcome,
                issues,
            };
            (answer, did)
        }
        Acted::Refused(refusal) => {
            let did = did(Err(refusal.clone()));
            (Answer::Refused(Refused::Verb { tab, refusal }), did)
        }
    }
}

/// Why the tab at `tab` did not answer `ask`, as the kind that crosses.
fn unanswered(tab: String, why: Unanswered, ask: &'static str) -> Refused {
    match why {
        Unanswered::Gone(said) => Refused::TabGone { tab, said },
        Unanswered::NotCarriedOut(said) => Refused::NotCarriedOut { tab, said },
        Unanswered::Password(node) => Refused::Password { tab, node },
        Unanswered::CardDetails(node) => Refused::CardDetails { tab, node },
        Unanswered::NotOffered => Refused::NotOffered { ask },
    }
}

/// Why a tab did not do what it was asked, in words, for a person.
fn unanswered_said(why: &Unanswered) -> String {
    match why {
        Unanswered::Gone(said) | Unanswered::NotCarriedOut(said) => said.clone(),
        Unanswered::Password(node) | Unanswered::CardDetails(node) => {
            format!("{node} cannot be acted on by an agent")
        }
        Unanswered::NotOffered => "its renderer does not do it".to_owned(),
    }
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
    use crate::pages::RenderedBy;
    use alo_net::cause::DocumentId;
    use alo_renderer::Tabs;
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
        serve(tabs, record, &AgentScope::default(), Caller(0), Ok(ask)).answer
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
            serve(
                &mut tabs,
                &mut record,
                &AgentScope::from(sites.clone()),
                Caller(0),
                Ok(read),
            )
            .answer
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
        let served = serve(
            &mut tabs,
            &mut record,
            &AgentScope::from(sites.clone()),
            Caller(1),
            Ok(act),
        );
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
        let listed = serve(
            &mut tabs,
            &mut record,
            &AgentScope::from(sites.clone()),
            Caller(0),
            Ok(Ask::Tabs),
        );
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
        let served = serve(
            &mut tabs,
            &mut record,
            &AgentScope::from(sites.clone()),
            Caller(0),
            Ok(read),
        );
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
            &AgentScope::default(),
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
                rendered_by: None,
                did: Did::Refused(Refused::NotJson)
            })
        );
    }

    /// One tab of a renderer that is not this engine, counting every time
    /// it is looked at, whose page goes to `then` as it is read — as a page
    /// in Chrome may, going elsewhere on its own while it is asked.
    struct Moving {
        at: alo_url::Url,
        then: Option<alo_url::Url>,
        looked: usize,
    }

    impl Pages for Moving {
        type Tab = ();

        fn catch_up(&mut self) {
            self.looked += 1;
        }

        fn shown(&self) -> Vec<((), alo_url::Url)> {
            vec![((), self.at.clone())]
        }

        fn rendered_by(&self, (): ()) -> RenderedBy {
            RenderedBy::Chrome {
                product: "Chrome/0".to_owned(),
            }
        }

        fn document(&self, (): ()) -> Option<DocumentId> {
            Some(alo_net::cause::Identities::default().a_document())
        }

        fn read(&mut self, (): ()) -> Result<Box<Snapshot>, Unanswered> {
            self.looked += 1;
            if let Some(then) = self.then.take() {
                self.at = then;
            }
            Ok(Box::default())
        }

        fn act(
            &mut self,
            (): (),
            _: &Target,
            _: &Verb,
        ) -> Result<(alo_net::cause::ActionId, Acted), Unanswered> {
            self.looked += 1;
            Err(Unanswered::NotCarriedOut("not here".to_owned()))
        }
    }

    #[test]
    fn a_site_not_named_is_refused_without_a_page_of_any_renderer_being_asked() {
        let mut pages = Moving {
            at: url("https://example.org/"),
            then: None,
            looked: 0,
        };
        let mut record = AgentRecord::new();
        for ask in [
            Ask::Read {
                tab: "https://example.org/".to_owned(),
            },
            Ask::Renderer {
                tab: "https://example.org/".to_owned(),
            },
            Ask::Act {
                tab: "https://example.org/".to_owned(),
                target: Target::Named("Buy".to_owned()),
                verb: Verb::Activate,
            },
        ] {
            let served = serve(
                &mut pages,
                &mut record,
                &AgentScope::from(kept()),
                Caller(0),
                Ok(ask),
            );
            assert!(matches!(
                served.answer,
                Answer::Refused(Refused::NotGranted { .. })
            ));
        }
        assert_eq!(pages.looked, 0);
        assert!(
            record.entries().all(|entry| entry.rendered_by.is_none()),
            "no tab was reached, so no renderer is named"
        );
    }

    #[test]
    fn a_page_that_went_elsewhere_while_it_was_read_is_not_answered() {
        let mut pages = Moving {
            at: url("https://example.com/a"),
            then: Some(url("https://example.org/b")),
            looked: 0,
        };
        let mut record = AgentRecord::new();
        let read = Ask::Read {
            tab: "https://example.com/a".to_owned(),
        };
        let served = serve(
            &mut pages,
            &mut record,
            &AgentScope::from(kept()),
            Caller(0),
            Ok(read),
        );
        assert_eq!(
            served.answer,
            Answer::Refused(Refused::NoSuchTab {
                tab: "https://example.com/a".to_owned()
            })
        );
        assert_eq!(pages.looked, 2, "caught up, then read");
        let entry = record.entries().next();
        assert_eq!(
            entry.and_then(|entry| entry.rendered_by.clone()),
            Some(RenderedBy::Chrome {
                product: "Chrome/0".to_owned()
            }),
            "the tab was reached, and its renderer is named"
        );
        // Read where it is now, it is refused for its site.
        let read = Ask::Read {
            tab: "https://example.org/b".to_owned(),
        };
        let served = serve(
            &mut pages,
            &mut record,
            &AgentScope::from(kept()),
            Caller(0),
            Ok(read),
        );
        assert!(matches!(
            served.answer,
            Answer::Refused(Refused::NotGranted { .. })
        ));
    }

    #[test]
    fn the_renderer_of_a_tab_is_answered_without_its_renderer_being_asked() {
        let (mut tabs, mut record) = (tabs(), AgentRecord::new());
        tabs.open(url("https://example.com/"));
        let tab = "https://EXAMPLE.com".to_owned();
        assert_eq!(
            served(&mut tabs, &mut record, Ask::Renderer { tab: tab.clone() }),
            Answer::Renderer {
                tab: tab.clone(),
                rendered_by: RenderedBy::ThisEngine
            },
            "nothing is loaded and no renderer could start, and it is answered"
        );
        assert_eq!(
            record.entries().next(),
            Some(&Entry {
                caller: Caller(0),
                tab: Some(tab),
                site: Some(Site::of(&url("https://example.com/"))),
                rendered_by: Some(RenderedBy::ThisEngine),
                did: Did::AskedRenderer,
            })
        );

        let mut pages = Moving {
            at: url("https://example.com/"),
            then: None,
            looked: 0,
        };
        let renderer = Ask::Renderer {
            tab: "https://example.com/".to_owned(),
        };
        let served = serve(
            &mut pages,
            &mut record,
            &AgentScope::from(kept()),
            Caller(1),
            Ok(renderer),
        );
        assert_eq!(
            served.answer,
            Answer::Renderer {
                tab: "https://example.com/".to_owned(),
                rendered_by: RenderedBy::Chrome {
                    product: "Chrome/0".to_owned()
                }
            }
        );
        assert_eq!(served.acted, None, "nothing ran, so nothing is drawn again");
        assert_eq!(pages.looked, 1, "caught up, and the page never read");
        let renderer = Ask::Renderer {
            tab: "https://example.net/".to_owned(),
        };
        let served = serve(
            &mut pages,
            &mut record,
            &AgentScope::from(kept()),
            Caller(1),
            Ok(renderer),
        );
        assert!(
            matches!(served.answer, Answer::Refused(Refused::NotGranted { .. })),
            "kept to its sites, as a read is"
        );
    }

    #[test]
    fn a_verb_not_carried_out_crosses_as_its_own_kind() {
        let mut pages = Moving {
            at: url("https://example.com/"),
            then: None,
            looked: 0,
        };
        let mut record = AgentRecord::new();
        let act = Ask::Act {
            tab: "https://example.com/".to_owned(),
            target: Target::Named("Buy".to_owned()),
            verb: Verb::Activate,
        };
        let served = serve(
            &mut pages,
            &mut record,
            &AgentScope::from(kept()),
            Caller(0),
            Ok(act),
        );
        assert_eq!(
            served.answer,
            Answer::Refused(Refused::NotCarriedOut {
                tab: "https://example.com/".to_owned(),
                said: "not here".to_owned()
            })
        );
        assert_eq!(served.acted, Some(()), "the tab may have changed");
    }

    /// One tab of a Chromium that goes where it is sent — or on to `lands`
    /// — prints `pdf`, and whose page names one link *Next page*, counting
    /// every time it is looked at.
    struct Paper {
        at: alo_url::Url,
        lands: Option<alo_url::Url>,
        pdf: Vec<u8>,
        looked: usize,
    }

    impl Paper {
        fn at(address: &str) -> Self {
            Self {
                at: url(address),
                lands: None,
                pdf: b"%PDF-1.7 a page".to_vec(),
                looked: 0,
            }
        }
    }

    impl Pages for Paper {
        type Tab = ();

        fn catch_up(&mut self) {
            self.looked += 1;
        }

        fn shown(&self) -> Vec<((), alo_url::Url)> {
            vec![((), self.at.clone())]
        }

        fn rendered_by(&self, (): ()) -> RenderedBy {
            RenderedBy::Carried {
                product: "Chrome/0".to_owned(),
            }
        }

        fn document(&self, (): ()) -> Option<DocumentId> {
            Some(alo_net::cause::Identities::default().a_document())
        }

        fn read(&mut self, (): ()) -> Result<Box<Snapshot>, Unanswered> {
            self.looked += 1;
            let node = |id, role, name: Option<&str>, children| SnapshotNode {
                id: alo_box::BoxId::from_wire(id),
                role,
                name: name.map(str::to_owned),
                states: alo_box::States::default(),
                placeholder: None,
                marker: None,
                rect: alo_layout::Rect::default(),
                rects: Vec::new(),
                offscreen: false,
                scrolls: false,
                children,
            };
            let link = node(
                5,
                alo_box::Role::Known(alo_box::KnownRole::Link),
                Some("Next page"),
                Vec::new(),
            );
            Ok(Box::new(Snapshot {
                root: Some(node(1, alo_box::Role::Generic, Some("A page"), vec![link])),
            }))
        }

        fn act(
            &mut self,
            (): (),
            _: &Target,
            _: &Verb,
        ) -> Result<(alo_net::cause::ActionId, Acted), Unanswered> {
            self.looked += 1;
            Err(Unanswered::NotCarriedOut("not here".to_owned()))
        }

        fn go(
            &mut self,
            (): (),
            to: &alo_url::Url,
        ) -> Result<alo_net::cause::ActionId, Unanswered> {
            self.looked += 1;
            self.at = self.lands.take().unwrap_or_else(|| to.clone());
            Ok(alo_net::cause::Identities::default().an_action())
        }

        fn print_pdf(&mut self, (): ()) -> Result<Vec<u8>, Unanswered> {
            self.looked += 1;
            Ok(self.pdf.clone())
        }
    }

    /// A directory of a test's own, a page let saved in it.
    fn saving_in(name: &str) -> (std::path::PathBuf, AgentScope) {
        let directory =
            std::env::temp_dir().join(format!("alo-serving-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap_or_default();
        let directory = std::fs::canonicalize(&directory).unwrap_or(directory);
        let saves = crate::agent_saves::AgentSaves::named([directory.clone()]).unwrap_or_default();
        (directory, AgentScope::new(kept(), saves))
    }

    #[test]
    fn a_page_is_saved_as_a_pdf_only_where_the_person_allowed() {
        let (directory, scope) = saving_in("allowed");
        let mut pages = Paper::at("https://example.com/");
        let mut record = AgentRecord::new();
        let file = directory.join("page.pdf");
        let path = file.to_string_lossy().into_owned();
        let print = |path: &str| Ask::PrintPdf {
            tab: "https://example.com/".to_owned(),
            path: path.to_owned(),
        };
        let served = serve(&mut pages, &mut record, &scope, Caller(0), Ok(print(&path)));
        assert_eq!(
            served.answer,
            Answer::Printed {
                tab: "https://example.com/".to_owned(),
                path: path.clone(),
                bytes: 15,
            }
        );
        assert_eq!(std::fs::read(&file).unwrap_or_default(), b"%PDF-1.7 a page");
        assert!(matches!(
            record.entries().last().map(|entry| &entry.did),
            Some(Did::Printed { bytes: 15, .. })
        ));
        // Asked again, nothing is written over it.
        let served = serve(&mut pages, &mut record, &scope, Caller(0), Ok(print(&path)));
        assert_eq!(
            served.answer,
            Answer::Refused(Refused::Exists { path: path.clone() })
        );
        // Anywhere else, and with no directory allowed, refused before any
        // tab is looked at — a tab nobody has open alike.
        let looked = pages.looked;
        let elsewhere = std::env::temp_dir().join("elsewhere.pdf");
        let elsewhere = elsewhere.to_string_lossy().into_owned();
        for (scope, path) in [
            (&scope, elsewhere.clone()),
            (
                &AgentScope::from(kept()),
                directory.join("b.pdf").to_string_lossy().into_owned(),
            ),
        ] {
            let served = serve(&mut pages, &mut record, scope, Caller(0), Ok(print(&path)));
            assert_eq!(
                served.answer,
                Answer::Refused(Refused::NotAllowedThere { path: path.clone() })
            );
        }
        let nobody = Ask::PrintPdf {
            tab: "https://example.com/nobody".to_owned(),
            path: elsewhere,
        };
        assert!(matches!(
            serve(&mut pages, &mut record, &scope, Caller(0), Ok(nobody)).answer,
            Answer::Refused(Refused::NotAllowedThere { .. })
        ));
        assert_eq!(pages.looked, looked, "no tab was looked at");
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn what_is_not_a_pdf_is_never_written_and_a_renderer_that_prints_none_says_so() {
        let (directory, scope) = saving_in("not-a-pdf");
        let mut pages = Paper::at("https://example.com/");
        pages.pdf = b"<html>".to_vec();
        let file = directory.join("page.pdf");
        let print = Ask::PrintPdf {
            tab: "https://example.com/".to_owned(),
            path: file.to_string_lossy().into_owned(),
        };
        let served = serve(
            &mut pages,
            &mut AgentRecord::new(),
            &scope,
            Caller(0),
            Ok(print.clone()),
        );
        assert!(matches!(
            served.answer,
            Answer::Refused(Refused::NotCarriedOut { .. })
        ));
        assert!(!file.exists());
        // A renderer that does not print — this engine's — is not offered.
        let mut pages = Moving {
            at: url("https://example.com/"),
            then: None,
            looked: 0,
        };
        let served = serve(
            &mut pages,
            &mut AgentRecord::new(),
            &scope,
            Caller(0),
            Ok(print),
        );
        assert_eq!(
            served.answer,
            Answer::Refused(Refused::NotOffered { ask: "print_pdf" })
        );
        assert!(!file.exists());
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_tab_is_sent_only_to_an_address_an_agent_may_open_and_named_where_it_arrived() {
        let scope = AgentScope::from(kept());
        let mut pages = Paper::at("https://example.com/");
        let mut record = AgentRecord::new();
        let open = |address: &str| Ask::Open {
            tab: Some("https://example.com/".to_owned()),
            address: address.to_owned(),
        };
        // Refused before any tab is looked at: not an address, not http,
        // and a site agents are kept from.
        for (address, kind) in [
            ("not an address", "NotOpenable"),
            ("file:///etc/passwd", "NotOpenable"),
            ("javascript:alert(1)", "NotOpenable"),
            ("https://example.org/", "NotGranted"),
        ] {
            let served = serve(
                &mut pages,
                &mut record,
                &scope,
                Caller(0),
                Ok(open(address)),
            );
            assert!(
                matches!(&served.answer, Answer::Refused(refused) if refused.kind() == kind),
                "{address}: {:?}",
                served.answer
            );
        }
        assert_eq!(pages.looked, 0, "no tab was looked at");
        let served = serve(
            &mut pages,
            &mut record,
            &scope,
            Caller(0),
            Ok(open("https://www.example.com/next")),
        );
        assert_eq!(
            served.answer,
            Answer::Opened {
                tab: "https://www.example.com/next".to_owned()
            }
        );
        assert!(matches!(
            record.entries().last().map(|entry| &entry.did),
            Some(Did::Went { .. })
        ));
        // Sent on by its server to a site agents are kept from: not named.
        pages.lands = Some(url("https://example.org/elsewhere"));
        let served = serve(
            &mut pages,
            &mut record,
            &scope,
            Caller(0),
            Ok(Ask::Open {
                tab: Some("https://www.example.com/next".to_owned()),
                address: "https://example.com/again".to_owned(),
            }),
        );
        assert!(
            matches!(&served.answer, Answer::Refused(Refused::NotOpened { said, .. })
                if !said.contains("example.org")),
            "{:?}",
            served.answer
        );
    }

    #[test]
    fn a_new_tab_is_not_offered_where_only_the_conversation_opens_one() {
        let scope = AgentScope::from(kept());
        let mut pages = Paper::at("https://example.com/");
        let mut record = AgentRecord::new();
        let new = |address: &str| Ask::Open {
            tab: None,
            address: address.to_owned(),
        };
        assert_eq!(
            serve(
                &mut pages,
                &mut record,
                &scope,
                Caller(0),
                Ok(new("https://example.com/"))
            )
            .answer,
            Answer::Refused(Refused::NotOffered { ask: "open" })
        );
        assert!(matches!(
            serve(
                &mut pages,
                &mut record,
                &scope,
                Caller(0),
                Ok(new("https://example.org/"))
            )
            .answer,
            Answer::Refused(Refused::NotGranted { .. })
        ));
        assert_eq!(
            serve(&mut pages, &mut record, &scope, Caller(0), Ok(Ask::NewTab)).answer,
            Answer::Refused(Refused::NotOffered { ask: "new_tab" })
        );
        // A second launch (queue item 976): judged by its address, and
        // not offered where one tab is held.
        let launched = |address: &str| Ask::Launched {
            address: address.to_owned(),
        };
        assert_eq!(
            serve(
                &mut pages,
                &mut record,
                &scope,
                Caller(0),
                Ok(launched("https://example.com/"))
            )
            .answer,
            Answer::Refused(Refused::NotOffered { ask: "launched" })
        );
        assert!(matches!(
            serve(
                &mut pages,
                &mut record,
                &scope,
                Caller(0),
                Ok(launched("https://example.org/"))
            )
            .answer,
            Answer::Refused(Refused::NotGranted { .. })
        ));
        assert_eq!(pages.looked, 0);
    }

    #[test]
    fn words_are_found_by_name_and_answered_without_a_place() {
        let mut pages = Paper::at("https://example.com/");
        let mut record = AgentRecord::new();
        let served = serve(
            &mut pages,
            &mut record,
            &AgentScope::from(kept()),
            Caller(0),
            Ok(Ask::Find {
                tab: "https://example.com/".to_owned(),
                text: "next".to_owned(),
            }),
        );
        let Answer::Found { found, more, .. } = served.answer else {
            panic!("not found: {:?}", served.answer);
        };
        assert_eq!(more, 0);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Next page");
        assert_eq!(found[0].node.as_usize(), 5);
        assert!(matches!(
            record.entries().last().map(|entry| &entry.did),
            Some(Did::Found { found: 1, .. })
        ));
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
