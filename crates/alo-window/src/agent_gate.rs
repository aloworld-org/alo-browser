/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The person's gate, for alo's own agent (ADR 0052, queue item 704, ADR
//! 0096 § 5): what it asks the person before doing, judged by the browser
//! process from the page's tree — never by the model, which only proposes.
//!
//! A press is held for the person when the tree says it **pays**, **sends**
//! or **deletes** — the words of what is pressed, or of what is inside it
//! when it has no name of its own ([`PAYS`], [`SENDS`], [`DELETES`]) — or
//! when it is a button inside a form, which **sends the form**. It is held
//! before anything goes to the page, and is done only after the person's
//! press on *Allow*. Typing and scrolling are never held: neither sends
//! anything on its own (a password is refused outright, by the card —
//! `chrome_act`).
//!
//! # What a press that pays is shown with
//!
//! A press held because it pays carries what it pays, read off the same
//! tree by the browser process ([`crate::agent_paying`], queue item 826):
//! the amount, who is paid and the page's own summary lines — and the
//! person answers *Pay* or *Not now*. A press named *Confirm* pays too on a
//! page that shows a total to pay ([`CONFIRMS`]).
//!
//! # What this cannot see
//!
//! It reads what the page says a thing is, and a page can say a thing that
//! pays is called *Continue*, outside any form. That is this gate's limit,
//! and ADR 0096 names it: the gate is on the tree's words, and is said so.
//! Asking more than is needed — a *Search* button inside a form — is the
//! side it errs on.

use alo_agent::Verb;
use alo_box::{BoxId, KnownRole, Role};
use alo_renderer::agent_paying::Paying;
use alo_renderer::{Snapshot, SnapshotNode};

/// Words that say pressing it pays.
pub const PAYS: [&str; 10] = [
    "pay",
    "payment",
    "buy",
    "purchase",
    "checkout",
    "order",
    "subscribe",
    "donate",
    "book",
    "rent",
];

/// Words that say pressing it pays on a page that shows a total to pay
/// (queue item 826): a checkout's last button is as often *Confirm* as
/// *Pay*.
pub const CONFIRMS: [&str; 2] = ["confirm", "complete"];

/// Words that say pressing it sends something of the person's.
pub const SENDS: [&str; 9] = [
    "send", "submit", "post", "publish", "reply", "share", "invite", "transfer", "tweet",
];

/// Words that say pressing it deletes or cancels something.
pub const DELETES: [&str; 9] = [
    "delete",
    "remove",
    "erase",
    "discard",
    "cancel",
    "unsubscribe",
    "trash",
    "destroy",
    "deactivate",
];

/// Why a press is held for the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// It pays.
    Pays,
    /// It sends something.
    Sends,
    /// It deletes or cancels something.
    Deletes,
    /// It is a button inside a form, and sends the form.
    SendsAForm,
}

/// What the person is asked: whether to press it, and why it is asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    /// Why it is held.
    pub held: Held,
    /// The question, in a person's words.
    pub what: String,
    /// What it pays, when it pays (queue item 826).
    pub paying: Option<Paying>,
}

/// Whether `verb` on `node` of `snapshot`, the page at `host`, is held for
/// the person, and what they are asked if it is.
pub fn asked(snapshot: &Snapshot, node: BoxId, verb: &Verb, host: &str) -> Option<Question> {
    if !matches!(verb, Verb::Activate) {
        return None;
    }
    let path = path_to(snapshot, node)?;
    let pressed = *path.last()?;
    let called = words_of(pressed);
    let held = if says(&called, &PAYS) {
        Held::Pays
    } else if says(&called, &DELETES) {
        Held::Deletes
    } else if says(&called, &SENDS) {
        Held::Sends
    } else if says(&called, &CONFIRMS)
        && crate::agent_paying::of(snapshot, node, host)
            .amount
            .is_some()
    {
        Held::Pays
    } else if is_a_button(pressed)
        && path
            .iter()
            .any(|above| above.role == Role::Known(KnownRole::Form))
    {
        Held::SendsAForm
    } else {
        return None;
    };
    let named = if called.is_empty() {
        "it".to_owned()
    } else {
        format!(
            "\u{201c}{}\u{201d}",
            crate::agent_protocol::kept(&called, 60)
        )
    };
    let on = if host.is_empty() {
        String::new()
    } else {
        format!(" on {host}")
    };
    let why = match held {
        Held::Pays => "It may pay for something.",
        Held::Sends => "It sends something from you.",
        Held::Deletes => "It may delete or cancel something.",
        Held::SendsAForm => "It sends the form on this page.",
    };
    Some(Question {
        held,
        what: format!("Press {named}{on}? {why}"),
        paying: (held == Held::Pays).then(|| crate::agent_paying::of(snapshot, node, host)),
    })
}

/// The nodes from the root down to `node`, if `snapshot` holds it.
fn path_to(snapshot: &Snapshot, node: BoxId) -> Option<Vec<&SnapshotNode>> {
    let root = snapshot.root.as_ref()?;
    // Walked without recursing, each waiting node with the path to it.
    let mut waiting: Vec<Vec<&SnapshotNode>> = vec![vec![root]];
    while let Some(path) = waiting.pop() {
        let last = *path.last()?;
        if last.id == node {
            return Some(path);
        }
        for child in &last.children {
            let mut longer = path.clone();
            longer.push(child);
            waiting.push(longer);
        }
    }
    None
}

/// What a node is called: its name, or the words inside it when it has
/// none.
fn words_of(node: &SnapshotNode) -> String {
    if let Some(name) = node.name.as_deref().filter(|name| !name.trim().is_empty()) {
        return name.trim().to_owned();
    }
    let mut words = Vec::new();
    let mut waiting = vec![node];
    while let Some(inside) = waiting.pop() {
        if let Some(name) = inside.name.as_deref() {
            words.push(name.trim().to_owned());
        }
        waiting.extend(inside.children.iter().rev());
    }
    words.retain(|word| !word.is_empty());
    words.join(" ")
}

/// Whether `node` is a button.
fn is_a_button(node: &SnapshotNode) -> bool {
    node.role == Role::Known(KnownRole::Button)
}

/// Whether `said` holds one of `words` as a word of its own, whatever its
/// case.
fn says(said: &str, words: &[&str]) -> bool {
    said.to_lowercase()
        .split(|each: char| !each.is_alphanumeric())
        .any(|word| words.contains(&word))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_box::States;
    use alo_layout::Rect;

    fn node(
        id: usize,
        role: KnownRole,
        name: Option<&str>,
        children: Vec<SnapshotNode>,
    ) -> SnapshotNode {
        SnapshotNode {
            id: BoxId::from_wire(id),
            role: Role::Known(role),
            name: name.map(str::to_owned),
            states: States::default(),
            placeholder: None,
            marker: None,
            rect: Rect::default(),
            rects: Vec::new(),
            offscreen: false,
            scrolls: false,
            children,
        }
    }

    /// A page with a form holding a search field and *Go*, and outside it
    /// *Send*, *Buy now*, *Delete everything*, a link *Read more* and an
    /// unnamed button holding the words *Remove*.
    fn page() -> Snapshot {
        Snapshot {
            root: Some(node(
                1,
                KnownRole::Document,
                None,
                vec![
                    node(
                        2,
                        KnownRole::Form,
                        None,
                        vec![
                            node(3, KnownRole::TextBox, Some("Search"), Vec::new()),
                            node(4, KnownRole::Button, Some("Go"), Vec::new()),
                        ],
                    ),
                    node(5, KnownRole::Button, Some("Send"), Vec::new()),
                    node(6, KnownRole::Button, Some("Buy now"), Vec::new()),
                    node(7, KnownRole::Link, Some("Delete everything"), Vec::new()),
                    node(8, KnownRole::Link, Some("Read more"), Vec::new()),
                    node(
                        9,
                        KnownRole::Button,
                        None,
                        vec![node(10, KnownRole::Text, Some("Remove"), Vec::new())],
                    ),
                    node(11, KnownRole::Button, Some("Say hello"), Vec::new()),
                    node(12, KnownRole::Button, Some("Confirm"), Vec::new()),
                ],
            )),
        }
    }

    fn held(node: usize) -> Option<Held> {
        asked(
            &page(),
            BoxId::from_wire(node),
            &Verb::Activate,
            "a.example",
        )
        .map(|asked| asked.held)
    }

    #[test]
    fn paying_sending_deleting_and_a_forms_button_are_held() {
        assert_eq!(held(4), Some(Held::SendsAForm));
        assert_eq!(held(5), Some(Held::Sends));
        assert_eq!(held(6), Some(Held::Pays));
        assert_eq!(held(7), Some(Held::Deletes));
        assert_eq!(held(9), Some(Held::Deletes), "by the words inside it");
    }

    #[test]
    fn confirm_pays_only_on_a_page_showing_a_total_and_a_paying_press_says_what_it_pays() {
        assert_eq!(held(12), None, "no total on the page");
        let mut page = page();
        if let Some(root) = page.root.as_mut() {
            root.children.push(node(
                13,
                KnownRole::Text,
                Some("Total UGX 45,000"),
                Vec::new(),
            ));
        }
        let asked = asked(&page, BoxId::from_wire(12), &Verb::Activate, "shop.example");
        assert_eq!(asked.as_ref().map(|asked| asked.held), Some(Held::Pays));
        let paying = asked.and_then(|asked| asked.paying);
        assert_eq!(
            paying.as_ref().and_then(|paying| paying.amount.as_deref()),
            Some("UGX 45,000")
        );
        assert_eq!(
            paying.map(|paying| paying.merchant),
            Some("shop.example".to_owned())
        );
        // Only a press that pays carries what it pays.
        let sends = super::asked(&page, BoxId::from_wire(5), &Verb::Activate, "shop.example");
        assert_eq!(sends.and_then(|asked| asked.paying), None);
    }

    #[test]
    fn anything_else_is_not_held_nor_is_typing_or_scrolling() {
        assert_eq!(held(8), None);
        assert_eq!(held(11), None);
        assert_eq!(held(99), None, "a node not on the page");
        let typing = Verb::PutText("Send".to_owned());
        assert_eq!(
            asked(&page(), BoxId::from_wire(5), &typing, "a.example"),
            None
        );
    }

    #[test]
    fn the_question_names_what_is_pressed_where_and_why() {
        let asked = asked(
            &page(),
            BoxId::from_wire(5),
            &Verb::Activate,
            "127.0.0.1:8080",
        );
        assert_eq!(
            asked.map(|asked| asked.what),
            Some(
                "Press \u{201c}Send\u{201d} on 127.0.0.1:8080? It sends something from you."
                    .to_owned()
            )
        );
    }
}
