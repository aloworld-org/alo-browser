/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The closed list an option belongs to, read from a page's tree (queue
//! item 810, `docs/design/shell.md`, approved by the owner 2026-11-06:
//! *while the agent works, a dropdown's options are shown in the agent's
//! own column so nothing invisible captures keys*).
//!
//! alo's own agent choosing an option of a closed list on a card's page
//! (`Activate` on an `option` inside a `combobox`) shows the list's options
//! in its column, the one it chooses ticked: what a person would have seen
//! in the menu, which on a card is never opened (`chrome_choose`). Read
//! from the tree the agent's step was judged on — the names Chrome's tree
//! gives each option, in order — never from a point.

use alo_box::{BoxId, KnownRole, Role};
use alo_renderer::agent_work::{Dropdown, MOST_LISTED};
use alo_renderer::snapshot::{Snapshot, SnapshotNode};

/// The closed list `option` is an option of in `tree`, its options named
/// and the place of `option` among them — or [`None`] when `option` is not
/// an option of a closed list.
pub fn of(tree: &Snapshot, option: BoxId) -> Option<Dropdown> {
    let list = tree
        .nodes()
        .into_iter()
        .filter(|node| node.role == Role::Known(KnownRole::ComboBox))
        .find(|list| options(list).iter().any(|each| each.id == option))?;
    let options = options(list);
    let chosen = options
        .iter()
        .position(|each| each.id == option)
        .filter(|chosen| *chosen < MOST_LISTED)?;
    Some(Dropdown {
        list: list.name.clone().unwrap_or_default(),
        options: options
            .iter()
            .take(MOST_LISTED)
            .map(|each| each.name.clone().unwrap_or_default())
            .collect(),
        chosen,
    })
}

/// Every option under `list`, in order, through its groups.
fn options(list: &SnapshotNode) -> Vec<&SnapshotNode> {
    let mut found = Vec::new();
    let mut waiting: Vec<&SnapshotNode> = list.children.iter().rev().collect();
    while let Some(node) = waiting.pop() {
        if node.role == Role::Known(KnownRole::Option) {
            found.push(node);
        } else {
            waiting.extend(node.children.iter().rev());
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_box::States;
    use alo_layout::Rect;

    fn node(id: usize, role: KnownRole, name: &str, children: Vec<SnapshotNode>) -> SnapshotNode {
        SnapshotNode {
            id: BoxId::from_wire(id),
            role: Role::Known(role),
            name: Some(name.to_owned()),
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

    /// A form holding the closed list *Fruit*, its options in a group, and
    /// a button.
    fn page() -> Snapshot {
        let option = |id, name| node(id, KnownRole::Option, name, Vec::new());
        Snapshot {
            root: Some(node(
                1,
                KnownRole::Form,
                "Order",
                vec![
                    node(
                        2,
                        KnownRole::ComboBox,
                        "Fruit",
                        vec![
                            option(3, "Apples"),
                            node(
                                4,
                                KnownRole::Group,
                                "Stone",
                                vec![option(5, "Cherries"), option(6, "Dates")],
                            ),
                        ],
                    ),
                    node(7, KnownRole::Button, "Send", Vec::new()),
                ],
            )),
        }
    }

    #[test]
    fn an_option_of_a_closed_list_is_listed_with_its_place_among_them() {
        assert_eq!(
            of(&page(), BoxId::from_wire(6)),
            Some(Dropdown {
                list: "Fruit".to_owned(),
                options: vec![
                    "Apples".to_owned(),
                    "Cherries".to_owned(),
                    "Dates".to_owned()
                ],
                chosen: 2,
            })
        );
    }

    #[test]
    fn anything_else_is_no_list() {
        assert_eq!(of(&page(), BoxId::from_wire(7)), None, "a button");
        assert_eq!(of(&page(), BoxId::from_wire(2)), None, "the list itself");
        assert_eq!(of(&page(), BoxId::from_wire(99)), None, "no such node");
    }
}
