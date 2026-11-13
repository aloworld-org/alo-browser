/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A card's page read and acted on for assistive technology (ADR 0115 § 3,
//! phase (b), queue item 988): a thread of the browser process that asks
//! **the same orders the agent socket sends** — the conversation's relay
//! ([`crate::conversation_agents`]), answered by the thread serving the
//! live card as [`Caller::ASSISTIVE`] — and no other road. Its actions are
//! written in the card's agent record as every caller's are; its reads,
//! asked after every frame, are not ([`crate::serving::serve`]), so they
//! never push the agents' actions out of it (ADR 0012 § 4).
//!
//! # One road, windowed or windowless
//!
//! The page's tree is the socket's `read` ([`Ask::Read`]): Chrome's
//! `Accessibility.getFullAXTree` over CDP, mapped by
//! [`crate::chrome_tree`] and bounded as the socket bounds it — a tree too
//! deep is refused as the socket refuses it, and said here to be too large
//! ([`PageRead::TooLarge`]). Chromium's own platform objects for the card's
//! window are never read: a windowless card has none (ADR 0113 § 7), and
//! one road is one tree.
//!
//! # By node, as `act` acts
//!
//! A press ([`Assist::Click`]) is the socket's `activate` on that node, a
//! text put in ([`Assist::SetValue`]) its `put_text` — so a password and a
//! card's details are refused by the card as they are for every agent
//! (`Refused::Password`, `Refused::CardDetails`) — and the keyboard moved
//! ([`Assist::Focus`], offered only on a field) is a press on the field,
//! as ADR 0115 § 2 has it: what a person's press on it does. Never a
//! point: the socket has no verb that takes one (ADR 0002).
//!
//! # A press that pays is held
//!
//! ADR 0115, *Answered* 1, as alo's agent's is (ADR 0099,
//! [`crate::conversation_agent`]): the page is read again just before the
//! press and judged by the person's gate ([`crate::agent_gate::asked`]);
//! one it says pays is told to the window as [`PageHeard::Held`], with what
//! it pays, and is pressed only once the person allowed it — and only if
//! the page, read again, would ask exactly the same, amount and all. A
//! press the gate would hold for sending or deleting is not held: the bus
//! is the person's own hands as much as anything's, and *Answered* 1 holds
//! only paying.
//!
//! # Never waited on, and every wait bounded
//!
//! The window's event loop calls nothing that waits (ADR 0024 § 2): it
//! hands work here and is told what came of it ([`TellPage`]). Each ask
//! waits at most [`PATIENCE`]. Of the reads waiting, only one is made: the
//! page read as it is now is the newest of them.

use crate::agent_gate::{self, Held, Question};
use crate::agent_record::Caller;
use crate::asking::{Answer, Ask, Refused};
use crate::assisted::{Assist, PageRead};
use crate::inbox::Orders;
use crate::message::Order;
use alo_agent::{Outcome, Target, Verb};
use alo_box::BoxId;
use alo_renderer::Snapshot;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;
use std::time::Duration;

/// The longest one ask of the card's page is waited for.
pub const PATIENCE: Duration = Duration::from_secs(30);

/// What the window hands the thread.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Work {
    /// Read the page as it is now.
    Read,
    /// Do `assist` to `node` — allowed by the person, when it was held, as
    /// `allowed` asked them.
    Act {
        node: BoxId,
        assist: Assist,
        allowed: Option<Question>,
    },
}

/// What the thread tells the window.
#[derive(Debug, Clone, PartialEq)]
pub enum PageHeard {
    /// The page, as it was read.
    Read(PageRead),
    /// It could not be read, and why: no card's page open, or the socket's
    /// refusal in its words.
    Unread(String),
    /// A press that pays, held for the person's yes (ADR 0115, *Answered*
    /// 1): nothing was pressed.
    Held {
        /// The node.
        node: BoxId,
        /// What was asked.
        assist: Assist,
        /// What the person is asked, and what the press pays.
        question: Question,
    },
    /// What came of an action.
    Acted {
        /// The node.
        node: BoxId,
        /// What was asked.
        assist: Assist,
        /// Whether the card carried it out.
        done: bool,
        /// What it came to, in words.
        came_to: String,
    },
}

/// How the thread tells the window: whether it was there to hear it.
pub type TellPage = Box<dyn Fn(PageHeard) -> bool + Send>;

/// The thread reading and acting on the card's page for the bus.
#[derive(Debug)]
pub struct PageReader {
    /// Where work is sent; [`None`] once let go.
    work: Option<Sender<Work>>,
    /// Never joined: a thread waiting on a page answers within
    /// [`PATIENCE`], and the window closing waits for no page.
    _thread: JoinHandle<()>,
}

impl PageReader {
    /// A thread asking `orders` — the conversation's relay — for the live
    /// card's page, telling the window with `tell`.
    ///
    /// # Errors
    ///
    /// What the system said when no thread could be started.
    pub fn start(orders: Orders, tell: TellPage) -> std::io::Result<Self> {
        let (work, asked) = channel();
        let thread = std::thread::Builder::new()
            .name("alo-assisted-page".to_owned())
            .spawn(move || serve(&orders, &asked, &tell))?;
        Ok(Self {
            work: Some(work),
            _thread: thread,
        })
    }

    /// Read the page as it is now.
    pub fn read(&self) {
        self.send(Work::Read);
    }

    /// Do `assist` to `node` of the page — once `allowed`, the question the
    /// person said yes to, when the press was held.
    pub fn act(&self, node: BoxId, assist: Assist, allowed: Option<Question>) {
        self.send(Work::Act {
            node,
            assist,
            allowed,
        });
    }

    fn send(&self, work: Work) {
        if let Some(sender) = &self.work {
            // A thread gone is a window closing, which waits on nothing.
            let _ = sender.send(work);
        }
    }
}

/// Do the work `asked` hands over, in order — of the reads waiting, only
/// the last — until the window lets go of it.
fn serve(orders: &Orders, asked: &Receiver<Work>, tell: &TellPage) {
    while let Ok(first) = asked.recv() {
        let waiting: Vec<Work> = core::iter::once(first).chain(asked.try_iter()).collect();
        let last_read = waiting.iter().rposition(|work| *work == Work::Read);
        for (place, work) in waiting.into_iter().enumerate() {
            let heard = match work {
                Work::Read if Some(place) == last_read => match read(orders) {
                    Ok((_, read)) => PageHeard::Read(read),
                    Err(why) => PageHeard::Unread(why),
                },
                Work::Read => continue,
                Work::Act {
                    node,
                    assist,
                    allowed,
                } => act(orders, node, assist, allowed.as_ref()),
            };
            if !tell(heard) {
                return;
            }
        }
    }
}

/// `ask`, as assistive technology, answered within [`PATIENCE`].
fn ask(orders: &Orders, ask: Ask) -> Result<Answer, String> {
    let (reply, answered) = channel();
    orders
        .send(Order::Agent {
            caller: Caller::ASSISTIVE,
            ask: Ok(ask),
            reply,
        })
        .map_err(|_| "the conversation has closed".to_owned())?;
    answered.recv_timeout(PATIENCE).map_err(|_| {
        format!(
            "the card's page did not answer within {} seconds",
            PATIENCE.as_secs()
        )
    })
}

/// The live card's tab and its page as it is now.
fn read(orders: &Orders) -> Result<(String, PageRead), String> {
    let tab = match ask(orders, Ask::Tabs)? {
        Answer::Tabs(tabs) => tabs
            .into_iter()
            .next()
            .ok_or_else(|| "no card's page is open".to_owned())?,
        other => return Err(other.said().unwrap_or_default()),
    };
    match ask(orders, Ask::Read { tab: tab.clone() })? {
        Answer::Tree { snapshot, .. } => Ok((tab, PageRead::Tree(snapshot))),
        Answer::Refused(Refused::TreeTooDeep { .. }) => Ok((tab, PageRead::TooLarge)),
        other => Err(other.said().unwrap_or_default()),
    }
}

/// What is to be done with a press on `node` of `tree`, the page at
/// `host`, the person having allowed the question `allowed`, if they
/// were asked one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Judged {
    /// Press it.
    Press,
    /// Hold it for the person's yes: it pays.
    Hold(Question),
    /// The person allowed a question the page no longer asks — the button
    /// renamed, gone, the total changed — so nothing is pressed.
    Changed,
}

/// How a press on `node` of `tree`, at `host`, is judged by the person's
/// gate for the bus: held only when it pays (ADR 0115, *Answered* 1), and,
/// once allowed, pressed only if the page still asks exactly what the
/// person was asked, as alo's agent's press is.
pub fn judged(tree: &Snapshot, node: BoxId, host: &str, allowed: Option<&Question>) -> Judged {
    let pays = agent_gate::asked(tree, node, &Verb::Activate, host)
        .filter(|question| question.held == Held::Pays);
    match (pays, allowed) {
        (None, None) => Judged::Press,
        (Some(question), None) => Judged::Hold(question),
        (still, Some(allowed)) if still.as_ref() == Some(allowed) => Judged::Press,
        (_, Some(_)) => Judged::Changed,
    }
}

/// What `outcome` of `assist` is said as: never the text put in, which
/// the record keeps only by its length, as alo's own fields' is.
fn came_to(assist: &Assist, outcome: &Outcome) -> String {
    match (assist, outcome) {
        (Assist::SetValue(_), Outcome::TextCanceled { node, .. }) => {
            format!("the page cancelled the text put into {node}")
        }
        (Assist::SetValue(_), other) => format!("the text put into {}", other.node()),
        (_, other) => other.to_string(),
    }
}

/// The verb `assist` is on the card's page: the socket's own.
fn verb_of(assist: &Assist) -> Verb {
    match assist {
        // The keyboard moved is a press on the field, as a person's is.
        Assist::Click | Assist::Focus => Verb::Activate,
        Assist::SetValue(text) => Verb::PutText(text.clone()),
    }
}

/// Do `assist` to `node`, as the socket's `act` does, once the gate lets
/// it: what came of it.
fn act(orders: &Orders, node: BoxId, assist: Assist, allowed: Option<&Question>) -> PageHeard {
    let verb = verb_of(&assist);
    let unacted = |why: String| PageHeard::Acted {
        node,
        assist: assist.clone(),
        done: false,
        came_to: why,
    };
    let (tab, now) = match read(orders) {
        Ok(read) => read,
        Err(why) => return unacted(format!("not done: {why}")),
    };
    if verb == Verb::Activate {
        let PageRead::Tree(tree) = &now else {
            return unacted("not done: the page is too large to read".to_owned());
        };
        let host = alo_url::parse(&tab)
            .map(|url| alo_url::reading::host(&url))
            .unwrap_or_default();
        match judged(tree, node, &host, allowed) {
            Judged::Press => {}
            Judged::Hold(question) => {
                return PageHeard::Held {
                    node,
                    assist,
                    question,
                };
            }
            Judged::Changed => {
                return unacted(
                    "not done: the page changed while the person was asked, so nothing was \
                     pressed"
                        .to_owned(),
                );
            }
        }
    }
    match ask(
        orders,
        Ask::Act {
            tab,
            target: Target::Node(node),
            verb,
        },
    ) {
        Ok(Answer::Done { outcome, .. }) => PageHeard::Acted {
            came_to: came_to(&assist, &outcome),
            node,
            assist,
            done: true,
        },
        // Text put in is never said back, whatever refused it.
        Ok(Answer::Refused(refused)) if matches!(assist, Assist::SetValue(_)) => {
            unacted(format!("refused: {}", refused.kind()))
        }
        Ok(Answer::Refused(refused)) => unacted(format!("refused: {refused}")),
        Ok(other) => unacted(other.said().unwrap_or_default()),
        Err(why) => unacted(format!("not done: {why}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::{self, Arrival};
    use alo_box::{KnownRole, Role, States};
    use alo_layout::Rect;
    use alo_renderer::SnapshotNode;
    use std::sync::{Arc, Mutex};

    const TAB: &str = "http://127.0.0.1:9/pay.html";

    fn node(id: usize, role: KnownRole, name: &str, children: Vec<SnapshotNode>) -> SnapshotNode {
        SnapshotNode {
            id: BoxId::from_wire(id),
            role: Role::Known(role),
            name: Some(name.to_owned()),
            states: States::default(),
            placeholder: None,
            marker: None,
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            rects: vec![Rect::new(0.0, 0.0, 10.0, 10.0)],
            offscreen: false,
            scrolls: false,
            children,
        }
    }

    /// A checkout: *Pay UGX 45,000*, *Send a note* and *Say hello*.
    fn checkout(pay: &str) -> Snapshot {
        Snapshot {
            root: Some(node(
                1,
                KnownRole::Document,
                "Kampala Coffee",
                vec![
                    node(2, KnownRole::Button, pay, Vec::new()),
                    node(3, KnownRole::Button, "Send a note", Vec::new()),
                    node(4, KnownRole::Button, "Say hello", Vec::new()),
                ],
            )),
        }
    }

    #[test]
    fn a_press_that_pays_is_held_and_pressed_once_allowed_only_as_asked() {
        let page = checkout("Pay UGX 45,000");
        let Judged::Hold(question) = judged(&page, BoxId::from_wire(2), "shop.example", None)
        else {
            panic!("a press that pays is held");
        };
        assert_eq!(question.held, Held::Pays);
        assert_eq!(
            question
                .paying
                .as_ref()
                .and_then(|paying| paying.amount.as_deref()),
            Some("UGX 45,000")
        );
        // Allowed, and the page asks the same: pressed.
        assert_eq!(
            judged(&page, BoxId::from_wire(2), "shop.example", Some(&question)),
            Judged::Press
        );
        // The total changed while the person read: not pressed.
        let changed = checkout("Pay UGX 90,000");
        assert_eq!(
            judged(
                &changed,
                BoxId::from_wire(2),
                "shop.example",
                Some(&question)
            ),
            Judged::Changed
        );
        // Allowed, and the button gone: not pressed.
        assert_eq!(
            judged(&page, BoxId::from_wire(9), "shop.example", Some(&question)),
            Judged::Changed
        );
    }

    #[test]
    fn a_press_that_sends_or_says_hello_is_not_held() {
        let page = checkout("Pay UGX 45,000");
        for node in [3, 4] {
            assert_eq!(
                judged(&page, BoxId::from_wire(node), "shop.example", None),
                Judged::Press,
                "{node}"
            );
        }
    }

    #[test]
    fn text_put_in_is_never_said_back() {
        let node = BoxId::from_wire(9);
        let put = Outcome::TextPut {
            node,
            text: "hunter2".to_owned(),
        };
        let said = came_to(&Assist::SetValue("hunter2".to_owned()), &put);
        assert!(!said.contains("hunter2"), "{said}");
        assert_eq!(said, "the text put into box#9");
    }

    #[test]
    fn the_keyboard_moved_is_a_press_and_text_is_put_text() {
        assert_eq!(verb_of(&Assist::Click), Verb::Activate);
        assert_eq!(verb_of(&Assist::Focus), Verb::Activate);
        assert_eq!(
            verb_of(&Assist::SetValue("porto".to_owned())),
            Verb::PutText("porto".to_owned())
        );
    }

    /// Each ask a fake card was sent, by whom.
    type Asked = Arc<Mutex<Vec<(Caller, Ask)>>>;

    /// A card's thread answering with `page`, refusing a tree too deep
    /// when `deep`, and keeping every ask.
    fn card(page: Snapshot, deep: bool) -> (Orders, Asked) {
        let (orders, _, inbox) = inbox::inbox();
        let asked = Arc::new(Mutex::new(Vec::new()));
        let kept = Arc::clone(&asked);
        std::thread::spawn(move || {
            while let Ok(Arrival::Order(Order::Agent { caller, ask, reply })) = inbox.recv() {
                let Ok(ask) = ask else { return };
                if let Ok(mut kept) = kept.lock() {
                    kept.push((caller, ask.clone()));
                }
                let answer = match ask {
                    Ask::Tabs => Answer::Tabs(vec![TAB.to_owned()]),
                    Ask::Read { tab } if deep => Answer::Refused(Refused::TreeTooDeep {
                        tab,
                        most: crate::asking::MOST_DEPTH,
                    }),
                    Ask::Read { tab } => Answer::Tree {
                        tab,
                        snapshot: Box::new(page.clone()),
                    },
                    Ask::Act {
                        tab,
                        target: Target::Node(node),
                        ..
                    } => Answer::Done {
                        tab,
                        outcome: Outcome::Activated { node, name: None },
                        issues: Vec::new(),
                    },
                    _ => return,
                };
                let _ = reply.send(answer);
            }
        });
        (orders, asked)
    }

    fn heard_from(orders: Orders, then: impl FnOnce(&PageReader)) -> Vec<PageHeard> {
        let (tell, told) = channel();
        let reader = PageReader::start(orders, Box::new(move |heard| tell.send(heard).is_ok()))
            .expect("a thread");
        then(&reader);
        let mut heard = Vec::new();
        while let Ok(each) = told.recv_timeout(Duration::from_secs(5)) {
            heard.push(each);
            if heard.len() == 2 {
                break;
            }
        }
        heard
    }

    #[test]
    fn it_reads_and_presses_as_assistive_technology_and_holds_what_pays() {
        let (orders, asked) = card(checkout("Pay UGX 45,000"), false);
        let heard = heard_from(orders, |reader| {
            reader.read();
            reader.act(BoxId::from_wire(2), Assist::Click, None);
        });
        assert_eq!(heard.len(), 2, "{heard:?}");
        assert!(matches!(&heard[0], PageHeard::Read(PageRead::Tree(_))));
        assert!(
            matches!(&heard[1], PageHeard::Held { node, .. } if node.as_usize() == 2),
            "{heard:?}"
        );
        let asked = asked.lock().map(|asked| asked.clone()).unwrap_or_default();
        assert!(asked.iter().all(|(caller, _)| *caller == Caller::ASSISTIVE));
        assert!(
            !asked.iter().any(|(_, ask)| matches!(ask, Ask::Act { .. })),
            "nothing pressed while held: {asked:?}"
        );
        assert_eq!(Caller::ASSISTIVE.to_string(), "assistive technology");
    }

    #[test]
    fn a_press_not_held_is_the_sockets_activate_on_the_node() {
        let (orders, asked) = card(checkout("Pay UGX 45,000"), false);
        let heard = heard_from(orders, |reader| {
            reader.act(BoxId::from_wire(4), Assist::Click, None);
            reader.act(BoxId::from_wire(4), Assist::SetValue("hi".to_owned()), None);
        });
        assert!(
            heard
                .iter()
                .all(|heard| matches!(heard, PageHeard::Acted { done: true, .. })),
            "{heard:?}"
        );
        let acts: Vec<Verb> = asked
            .lock()
            .map(|asked| asked.clone())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(_, ask)| match ask {
                Ask::Act { target, verb, .. } => {
                    assert_eq!(target, Target::Node(BoxId::from_wire(4)));
                    Some(verb)
                }
                _ => None,
            })
            .collect();
        assert_eq!(acts, [Verb::Activate, Verb::PutText("hi".to_owned())]);
    }

    #[test]
    fn a_tree_too_deep_for_the_socket_is_read_as_too_large() {
        let (orders, _) = card(checkout("Pay"), true);
        let heard = heard_from(orders, |reader| {
            reader.read();
            reader.act(BoxId::from_wire(2), Assist::Click, None);
        });
        assert_eq!(heard.first(), Some(&PageHeard::Read(PageRead::TooLarge)));
        assert!(
            matches!(heard.get(1), Some(PageHeard::Acted { done: false, .. })),
            "{heard:?}"
        );
    }
}
