/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the conversation's agent socket reaches (ADR 0075's amendment for
//! queue item 716, ADR 0055): **the page the conversation shows as a card
//! now**, through one socket for as long as the conversation runs.
//!
//! `alo --chrome` opens the socket over the one tab its Chrome serves. The
//! conversation opens a Chrome per card — a second address chosen closes
//! the first and opens another (ADR 0075 § 4) — so what the socket is
//! handed is a [`relay`]: [`crate::agents::Listening`] sends every ask to
//! it, and it sends each on to the thread serving the live card, as
//! [`Order::Agent`] — the same order, answered and recorded there as `alo
//! --chrome` answers it, kept to the sites a person named
//! ([`AgentSites`]). The thread opening cards tells it which card is live
//! ([`Reached::now`]) as each opens and closes. So the socket **follows the
//! newest page**: an agent naming the page that was replaced is told no tab
//! is open at it, and asking for the tabs is told the one there is now.
//!
//! With no card open — before the first address, between a card closed and
//! the next, a card that would not open — the relay answers itself, as a
//! browser with no tab does: no tabs; a tab outside the sites named refused
//! by its site, before anything else; any other no tab open at it.
//!
//! # A new tab is the window's (queue item 956, ADR 0112 § 4)
//!
//! `open` with no tab named and `new_tab` ask for what only the window
//! makes: a new chat — alo's new tab — empty, or with a card at the
//! address. The relay judges the address here, as [`serving::openable`]
//! judges it for every other road, records the ask, and hands what passed
//! to the window ([`Reached::answered_by`], [`NewTabAsked`]), which answers
//! the caller once the card has opened or not. A window that has not said
//! it answers is a conversation with no window, and the ask is not offered.

use crate::agent_record::{AgentRecord, Caller, Did, Entry};
use crate::agent_scope::AgentScope;
use crate::asking::{Answer, Ask, Refused};
use crate::inbox::{self, Arrival, Orders};
use crate::message::Order;
use crate::pages::{Acted, Pages, RenderedBy, Unanswered};
use crate::serving;
use alo_agent::{Target, Verb};
use alo_net::cause::{ActionId, DocumentId};
use alo_renderer::Snapshot;
use alo_url::Url;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, PoisonError};

/// A new tab an agent outside the browser asked for, handed to the
/// conversation's window to open as a new chat.
#[derive(Debug)]
pub struct NewTabAsked {
    /// The page its card is to show — judged an `http` or `https` address
    /// on a site agents may reach, and as the caller wrote it — or
    /// [`None`] for a chat with nothing in it.
    pub to: Option<(Url, String)>,
    /// Where the answer goes: [`Answer::Opened`] once the card opened,
    /// [`Answer::NewTab`], or why not.
    pub reply: Sender<Answer>,
}

/// How the window is handed a new tab asked for: whether it was there to
/// hear it.
pub type TellNewTab = Box<dyn Fn(NewTabAsked) -> bool + Send>;

/// The window's ear, shared between the relay and whoever installs it.
type Ear = Arc<Mutex<Option<TellNewTab>>>;

/// Which card's thread an agent's ask is sent to: the live card's, or none
/// — and the window a new tab is asked of, once it has said it answers.
#[derive(Clone, Default)]
pub struct Reached {
    live: Arc<Mutex<Option<Orders>>>,
    window: Ear,
}

impl core::fmt::Debug for Reached {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Reached")
            .field("live", &self.live)
            .finish_non_exhaustive()
    }
}

impl Reached {
    /// Nothing reached, and nobody told: for a conversation whose pages no
    /// socket reaches.
    pub fn nobody() -> Self {
        Self::default()
    }

    /// The live card's thread is now `orders` — or, with [`None`], there is
    /// no live card.
    pub fn now(&self, orders: Option<Orders>) {
        *self.live.lock().unwrap_or_else(PoisonError::into_inner) = orders;
    }

    /// The live card's thread, if there is one.
    fn live(&self) -> Option<Orders> {
        self.live
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The conversation's window answers a new tab asked for, by `tell`.
    pub fn answered_by(&self, tell: TellNewTab) {
        *self.window.lock().unwrap_or_else(PoisonError::into_inner) = Some(tell);
    }

    /// Hand `asked` to the window: given back when there is none, or it has
    /// gone.
    fn to_the_window(&self, asked: NewTabAsked) -> Result<(), Sender<Answer>> {
        let window = self.window.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(tell) = window.as_ref() else {
            return Err(asked.reply);
        };
        let reply = asked.reply.clone();
        if tell(asked) { Ok(()) } else { Err(reply) }
    }
}

/// What the conversation's cards are reached by: what agents are kept to,
/// and who is told which card is live.
#[derive(Debug, Clone, Default)]
pub struct Reaching {
    /// The sites a person named with `--agent-site`, or every one, and the
    /// directories they allowed a page saved in with `--agent-saves`.
    pub scope: AgentScope,
    /// Told which card is live.
    pub reached: Reached,
}

/// Start the relay: the orders the socket sends to, and who is told which
/// card is live. The relay ends once every copy of those orders has gone —
/// the socket stopped.
///
/// # Errors
///
/// What the operating system said when it would not start its thread.
pub fn relay(scope: impl Into<AgentScope>) -> std::io::Result<(Orders, Reaching)> {
    let scope: AgentScope = scope.into();
    let (orders, _, inbox) = inbox::inbox();
    let reached = Reached::default();
    let reaching = Reaching {
        scope: scope.clone(),
        reached: reached.clone(),
    };
    std::thread::Builder::new()
        .name("alo-conversation-agents".to_owned())
        .spawn(move || {
            let mut record = AgentRecord::new();
            while let Ok(arrival) = inbox.recv() {
                match arrival {
                    // A new tab: judged here, and the window's to open.
                    Arrival::Order(Order::Agent {
                        caller,
                        ask: ask @ Ok(Ask::Open { tab: None, .. } | Ask::NewTab),
                        reply,
                    }) => {
                        let unoffered = new_tab(&reached, &mut record, &scope, caller, ask, reply);
                        if let Some((ask, reply)) = unoffered {
                            let served =
                                serving::serve(&mut NoCard, &mut record, &scope, caller, ask);
                            let _ = reply.send(served.answer);
                        }
                    }
                    Arrival::Order(Order::Agent { caller, ask, reply }) => {
                        // Sent on to the live card's thread; a card gone as
                        // it was sent is no card, and answered here.
                        let kept = (ask.clone(), reply.clone());
                        let sent = reached.live().is_some_and(|card| {
                            card.send(Order::Agent { caller, ask, reply }).is_ok()
                        });
                        if !sent {
                            let (ask, reply) = kept;
                            let served =
                                serving::serve(&mut NoCard, &mut record, &scope, caller, ask);
                            // A caller gone changes nothing.
                            let _ = reply.send(served.answer);
                        }
                    }
                    Arrival::Abandoned => return,
                    _ => {}
                }
            }
        })?;
    Ok((orders, reaching))
}

/// A new tab `caller` asked for, judged and recorded, and handed to the
/// window — or, when no window answers, `ask` and `reply` given back for
/// serving to refuse as not offered.
fn new_tab(
    reached: &Reached,
    record: &mut AgentRecord,
    scope: &AgentScope,
    caller: Caller,
    ask: Result<Ask, Refused>,
    reply: Sender<Answer>,
) -> Option<(Result<Ask, Refused>, Sender<Answer>)> {
    let (to, site) = match &ask {
        Ok(Ask::Open { address, .. }) => match serving::openable(&scope.sites, address) {
            (Ok(url), site) => (Some((url, address.clone())), site),
            // Refused for the address, as every road refuses it.
            (Err(_), _) => return Some((ask, reply)),
        },
        _ => (None, None),
    };
    let did = Did::Opening {
        to: to.as_ref().map(|(url, _)| url.to_string()),
    };
    match reached.to_the_window(NewTabAsked {
        to,
        reply: reply.clone(),
    }) {
        Ok(()) => {
            record.write(Entry {
                caller,
                tab: None,
                site,
                rendered_by: None,
                did,
            });
            None
        }
        Err(reply) => Some((ask, reply)),
    }
}

/// No card open: no tab to name, read or act in.
struct NoCard;

impl Pages for NoCard {
    type Tab = ();

    fn catch_up(&mut self) {}

    fn shown(&self) -> Vec<((), Url)> {
        Vec::new()
    }

    fn rendered_by(&self, (): ()) -> RenderedBy {
        RenderedBy::ThisEngine
    }

    fn document(&self, (): ()) -> Option<DocumentId> {
        None
    }

    fn read(&mut self, (): ()) -> Result<Box<Snapshot>, Unanswered> {
        Err(Unanswered::Gone(
            "no page is open in the conversation".to_owned(),
        ))
    }

    fn act(&mut self, (): (), _: &Target, _: &Verb) -> Result<(ActionId, Acted), Unanswered> {
        Err(Unanswered::Gone(
            "no page is open in the conversation".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_sites::AgentSites;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    fn ask(orders: &Orders, ask: Ask) -> Answer {
        let (reply, answered) = channel();
        orders
            .send(Order::Agent {
                caller: Caller(1),
                ask: Ok(ask),
                reply,
            })
            .unwrap();
        answered.recv_timeout(Duration::from_secs(5)).unwrap()
    }

    #[test]
    fn a_new_tab_judged_here_is_handed_to_the_window_and_none_without_one() {
        let sites = AgentSites::named(["http://127.0.0.1:8080"]).unwrap();
        let (orders, reaching) = relay(sites).unwrap();
        let open = |address: &str| Ask::Open {
            tab: None,
            address: address.to_owned(),
        };
        // No window answers yet: not offered, after the address is judged.
        assert_eq!(
            ask(&orders, open("http://127.0.0.1:8080/a")),
            Answer::Refused(Refused::NotOffered { ask: "open" })
        );
        assert_eq!(
            ask(&orders, Ask::NewTab),
            Answer::Refused(Refused::NotOffered { ask: "new_tab" })
        );
        let (heard, hear) = channel();
        reaching
            .reached
            .answered_by(Box::new(move |asked| heard.send(asked).is_ok()));
        // Refused here for its address, and the window never asked.
        assert!(matches!(
            ask(&orders, open("https://example.com/")),
            Answer::Refused(Refused::NotGranted { .. })
        ));
        assert!(matches!(
            ask(&orders, open("file:///etc/hosts")),
            Answer::Refused(Refused::NotOpenable { .. })
        ));
        assert!(hear.try_recv().is_err());
        // Judged, and handed to the window, which answers.
        let (reply, answered) = channel();
        orders
            .send(Order::Agent {
                caller: Caller(3),
                ask: Ok(open("http://127.0.0.1:8080/a")),
                reply,
            })
            .unwrap();
        let asked = hear.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            asked
                .to
                .as_ref()
                .map(|(url, said)| (url.to_string(), said.as_str())),
            Some((
                "http://127.0.0.1:8080/a".to_owned(),
                "http://127.0.0.1:8080/a"
            ))
        );
        asked
            .reply
            .send(Answer::Opened {
                tab: "http://127.0.0.1:8080/a".to_owned(),
            })
            .unwrap();
        assert!(matches!(
            answered.recv_timeout(Duration::from_secs(5)).unwrap(),
            Answer::Opened { .. }
        ));
        let (reply, _answered) = channel();
        orders
            .send(Order::Agent {
                caller: Caller(3),
                ask: Ok(Ask::NewTab),
                reply,
            })
            .unwrap();
        assert!(
            hear.recv_timeout(Duration::from_secs(5))
                .unwrap()
                .to
                .is_none()
        );
    }

    #[test]
    fn with_no_card_there_are_no_tabs_and_a_site_not_named_is_refused_by_it() {
        let sites = AgentSites::named(["http://127.0.0.1:8080"]).unwrap();
        let (orders, _reaching) = relay(sites).unwrap();
        assert_eq!(ask(&orders, Ask::Tabs), Answer::Tabs(Vec::new()));
        let page = "http://127.0.0.1:8080/one.html".to_owned();
        assert!(matches!(
            ask(&orders, Ask::Read { tab: page.clone() }),
            Answer::Refused(Refused::NoSuchTab { tab }) if tab == page
        ));
        let elsewhere = "https://example.com/".to_owned();
        assert!(matches!(
            ask(&orders, Ask::Read { tab: elsewhere.clone() }),
            Answer::Refused(Refused::NotGranted { tab, .. }) if tab == elsewhere
        ));
    }

    #[test]
    fn an_ask_goes_to_the_live_card_and_back_here_once_it_has_gone() {
        let (orders, reaching) = relay(AgentSites::Every).unwrap();
        let (card, _, inbox) = inbox::inbox();
        reaching.reached.now(Some(card));
        let (reply, answered) = channel();
        orders
            .send(Order::Agent {
                caller: Caller(2),
                ask: Ok(Ask::Tabs),
                reply,
            })
            .unwrap();
        // The card's thread is handed the ask, and answers it.
        let Ok(Arrival::Order(Order::Agent { reply, ask, .. })) =
            inbox.recv_timeout(Duration::from_secs(5))
        else {
            panic!("the live card was not sent the ask");
        };
        assert_eq!(ask, Ok(Ask::Tabs));
        reply
            .send(Answer::Tabs(vec!["http://a.example/".to_owned()]))
            .unwrap();
        assert_eq!(
            answered.recv_timeout(Duration::from_secs(5)).unwrap(),
            Answer::Tabs(vec!["http://a.example/".to_owned()])
        );
        // The card closed: answered here, with no tab.
        reaching.reached.now(None);
        assert_eq!(ask_here(&orders), Answer::Tabs(Vec::new()));
        // A card whose thread has gone without saying so is no card either.
        let (gone, _, inbox) = inbox::inbox();
        drop(inbox);
        reaching.reached.now(Some(gone));
        assert_eq!(ask_here(&orders), Answer::Tabs(Vec::new()));
    }

    fn ask_here(orders: &Orders) -> Answer {
        ask(orders, Ask::Tabs)
    }
}
