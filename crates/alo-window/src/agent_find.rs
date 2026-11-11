/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Text found on a page, answered as the nodes that hold it by name —
//! never as a place on the screen (queue item 956, ADR 0112 § 4; ADR 0002).
//!
//! `find` is a person's *Find on page*, asked by an agent: which things on
//! the page say these words. It is answered from the same tree `read`
//! answers ([`crate::serving`]), so what is found is what a reader of the
//! tree would find, and a node found is named by its `node` number, its role
//! and its name, which a verb can aim at. No rectangle crosses: where a
//! node is drawn is `read`'s to say, and no verb takes it (ADR 0002).
//!
//! # What is found
//!
//! Every node whose name holds the words, compared in lower case — but
//! never the tree's root, the document, whose name is the page's title and
//! not words on the page; and never one whose nearest ancestor found has
//! **the same name**: a heading's text
//! inside the heading, a link's inside the link, is the same words said
//! twice, and the outer one is the one an agent acts on. An ancestor with a
//! different name holding the words too — a list named *Invoices* holding a
//! row *Invoice 3* — is answered beside it.
//!
//! At most [`MOST_FOUND`] are answered, in the order a person reads them,
//! and how many more there were is said rather than any of them dropped
//! silently.

use alo_box::{BoxId, Role};
use alo_renderer::{Snapshot, SnapshotNode};

/// The most nodes one `find` answers.
pub const MOST_FOUND: usize = 256;

/// A node that holds the words, by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundNode {
    /// Its number in the tree it was read from.
    pub node: BoxId,
    /// What it is.
    pub role: Role,
    /// Its name, which holds the words.
    pub name: String,
}

/// What a `find` came to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// The nodes found, at most [`MOST_FOUND`], in reading order.
    pub nodes: Vec<FoundNode>,
    /// How many more there were.
    pub more: usize,
}

/// The nodes of `snapshot` whose names hold `words`.
pub fn find(snapshot: &Snapshot, words: &str) -> Found {
    let wanted = words.to_lowercase();
    let mut found = Found::default();
    if wanted.is_empty() {
        return found;
    }
    // Each node waiting, with the name of its nearest ancestor found; the
    // root, the document, named by the page's title, is never one.
    let mut waiting: Vec<(&SnapshotNode, Option<&str>)> = snapshot
        .root
        .iter()
        .flat_map(|root| root.children.iter().rev())
        .map(|node| (node, None))
        .collect();
    while let Some((node, around)) = waiting.pop() {
        let name = node.name.as_deref().filter(|name| !name.is_empty());
        let holds = name.is_some_and(|name| name.to_lowercase().contains(&wanted));
        let echoed = holds && name == around;
        let around = match name {
            Some(name) if holds && !echoed => {
                if found.nodes.len() < MOST_FOUND {
                    found.nodes.push(FoundNode {
                        node: node.id,
                        role: node.role.clone(),
                        name: name.to_owned(),
                    });
                } else {
                    found.more += 1;
                }
                Some(name)
            }
            _ => around,
        };
        waiting.extend(node.children.iter().rev().map(|child| (child, around)));
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_box::KnownRole;
    use alo_layout::Rect;

    fn node(
        id: usize,
        role: Role,
        name: Option<&str>,
        children: Vec<SnapshotNode>,
    ) -> SnapshotNode {
        SnapshotNode {
            id: BoxId::from_wire(id),
            role,
            name: name.map(str::to_owned),
            states: alo_box::States::default(),
            placeholder: None,
            marker: None,
            rect: Rect::default(),
            rects: Vec::new(),
            offscreen: false,
            scrolls: false,
            children,
        }
    }

    fn text(id: usize, name: &str) -> SnapshotNode {
        node(
            id,
            Role::Declared("StaticText".into()),
            Some(name),
            Vec::new(),
        )
    }

    /// A page titled *Invoices*, a heading *Invoices*, a list of three rows
    /// named *Invoice N*, and a link *Next invoice*.
    fn page() -> Snapshot {
        let row = |id, n: usize| {
            let name = format!("Invoice {n}");
            node(
                id,
                Role::Known(KnownRole::Row),
                Some(&name),
                vec![text(id + 100, &name)],
            )
        };
        Snapshot {
            root: Some(node(
                1,
                Role::Declared("RootWebArea".into()),
                Some("Invoices"),
                vec![
                    node(
                        2,
                        Role::Known(KnownRole::Heading),
                        Some("Invoices"),
                        vec![text(3, "Invoices")],
                    ),
                    node(
                        4,
                        Role::Known(KnownRole::List),
                        Some("Invoice list"),
                        vec![row(5, 1), row(6, 2), row(7, 3)],
                    ),
                    node(
                        8,
                        Role::Known(KnownRole::Link),
                        Some("Next invoice"),
                        vec![text(9, "Next invoice")],
                    ),
                    node(10, Role::Generic, None, vec![text(11, "Nothing here")]),
                ],
            )),
        }
    }

    fn numbers(found: &Found) -> Vec<usize> {
        found
            .nodes
            .iter()
            .map(|found| found.node.as_usize())
            .collect()
    }

    #[test]
    fn every_node_named_with_the_words_is_found_in_reading_order_but_not_its_echo() {
        let found = find(&page(), "invoice 2");
        assert_eq!(numbers(&found), vec![6]);
        assert_eq!(found.nodes[0].name, "Invoice 2");
        assert_eq!(found.nodes[0].role, Role::Known(KnownRole::Row));
        assert_eq!(found.more, 0);
    }

    #[test]
    fn words_are_compared_in_lower_case_and_an_ancestor_with_another_name_is_found_too() {
        let found = find(&page(), "INVOICE");
        // The heading, the list, each row and the link: never the text
        // that repeats its holder.
        assert_eq!(numbers(&found), vec![2, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn the_document_named_by_the_pages_title_is_never_found() {
        // The page is titled as its heading is: the title is not on the
        // page, and the heading is found once, its text not at all.
        let found = find(&page(), "invoices");
        assert_eq!(numbers(&found), vec![2]);
    }

    #[test]
    fn nothing_is_found_for_words_nowhere_or_for_no_words() {
        assert_eq!(find(&page(), "receipt"), Found::default());
        assert_eq!(find(&page(), ""), Found::default());
        assert_eq!(find(&Snapshot { root: None }, "anything"), Found::default());
    }

    #[test]
    fn past_the_most_found_how_many_more_is_said() {
        let rows = (0..MOST_FOUND + 3)
            .map(|at| text(at + 2, "same words"))
            .collect();
        let snapshot = Snapshot {
            root: Some(node(1, Role::Generic, Some("same words"), rows)),
        };
        let found = find(&snapshot, "same");
        assert_eq!(found.nodes.len(), MOST_FOUND);
        assert_eq!(found.more, 3);
        assert_eq!(found.nodes[0].node.as_usize(), 2);
    }
}
