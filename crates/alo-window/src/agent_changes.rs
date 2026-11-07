/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the person changed while the page was theirs (queue item 825,
//! `docs/design/shell.md`, *Taking over and carrying on*, approved by the
//! owner 2026-11-06): told to the model when they press *Let alo
//! continue*, so the run goes on from the page as they left it.
//!
//! Read off two trees of the same card — the last the run read before the
//! person took over, and the one read as it carries on — by the browser
//! process, never by a script and never by the model:
//!
//! - **The address**, before and after, when it moved: the person went to
//!   another page, and what its fields hold is the page's own, sent in its
//!   tree as any page's is.
//! - **Each field whose value changed**, on the same page — a field that
//!   takes text, by its words; a checkbox or a radio, by whether it is
//!   checked; a list, by the option chosen — named as the tree names it
//!   and matched by the node's number, which the page keeps while it is
//!   the same document. A field that came or went is said so.
//! - **Never what a secret field holds.** A password's dots, and a field
//!   whose name or hint says it holds a password, a card's number, its
//!   security code or its expiry ([`crate::agent_page::is_secret`]), is said
//!   only as changed, by its name: what it holds is not sent.
//! - **Where the run had got to**: how many of its steps it had taken, and
//!   the step of its plan it was on.
//!
//! At most [`MOST_CHANGES`] fields are said, each value cut to
//! [`MOST_VALUE`] characters, and how many more there were.
//!
//! **Its limit**, said: the tree the run last read was read before its
//! last verb was carried out, when the person took over while that verb
//! was under way — so a field that verb filled reads as the person's. The
//! verb is in the steps the model is told of, with what came of it.

use alo_box::{BoxId, Checked, KnownRole, Role};
use alo_renderer::agent_plan::Plan;
use alo_renderer::{Snapshot, SnapshotNode};

/// The most fields whose change is said.
pub const MOST_CHANGES: usize = 12;

/// The longest a field's value is said, in characters.
pub const MOST_VALUE: usize = 80;

/// What a field holds, as the person might change it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Holds {
    /// Text: its words.
    Text(String),
    /// Checked, not, or partly.
    Checked(Checked),
    /// A list's option chosen, by its name; empty when none is.
    Chosen(String),
}

/// A field of the page.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Field {
    id: BoxId,
    name: String,
    holds: Holds,
    secret: bool,
}

/// The page at `address`, read as `tree`.
#[derive(Debug, Clone, Copy)]
pub struct Read<'a> {
    /// Its address.
    pub address: &'a str,
    /// Its tree.
    pub tree: &'a Snapshot,
}

/// Where the run had got to when the person took over.
#[derive(Debug, Clone, Copy)]
pub struct Reached<'a> {
    /// How many of its verbs it had carried out.
    pub steps: usize,
    /// Its plan, if it had written one.
    pub plan: Option<&'a Plan>,
}

/// What the person changed between `before` — the page as the run left it
/// — and `after`, and where the run had got to: the words the model is
/// told.
pub fn said(before: Read<'_>, after: Read<'_>, reached: Reached<'_>) -> String {
    let mut lines = Vec::new();
    if before.address == after.address {
        lines.extend(fields_changed(before.tree, after.tree));
        if lines.is_empty() {
            lines.push("The person changed no field alo can see on the page.".to_owned());
        }
    } else {
        lines.push(format!(
            "The person went from {} to {}.",
            before.address, after.address
        ));
    }
    let steps = match reached.steps {
        1 => "alo had taken 1 step".to_owned(),
        steps => format!("alo had taken {steps} steps"),
    };
    let on = reached
        .plan
        .and_then(|plan| {
            plan.steps.get(plan.on).map(|step| {
                format!(
                    ", and was on step {} of its plan, \u{201c}{step}\u{201d}",
                    plan.on + 1
                )
            })
        })
        .unwrap_or_default();
    lines.push(format!("{steps}{on}."));
    lines.join(" ")
}

/// Each change of a field between `before` and `after`, a sentence each,
/// at most [`MOST_CHANGES`] and how many more.
fn fields_changed(before: &Snapshot, after: &Snapshot) -> Vec<String> {
    let before = fields(before);
    let after = fields(after);
    let mut said = Vec::new();
    for now in &after {
        match before.iter().find(|was| was.id == now.id) {
            Some(was) if was.holds == now.holds => {}
            Some(was) => said.push(changed(was, now)),
            None => said.push(format!("{} appeared.", up(&named(&now.name)))),
        }
    }
    for was in &before {
        if !after.iter().any(|now| now.id == was.id) {
            said.push(format!("{} is gone.", up(&named(&was.name))));
        }
    }
    if said.len() > MOST_CHANGES {
        let more = said.len() - MOST_CHANGES;
        said.truncate(MOST_CHANGES);
        said.push(format!("And {more} more fields changed."));
    }
    said
}

/// The change of one field, from `was` to `now`.
fn changed(was: &Field, now: &Field) -> String {
    let name = named(&now.name);
    if was.secret || now.secret {
        return format!(
            "The person changed {name}, which holds a secret: what it holds is not sent."
        );
    }
    match (&was.holds, &now.holds) {
        (Holds::Checked(_), Holds::Checked(checked)) => {
            let checked = match checked {
                Checked::Yes => "checked",
                Checked::No => "unchecked",
                Checked::Mixed => "partly checked",
            };
            format!("The person {checked} {name}.")
        }
        (Holds::Chosen(_), Holds::Chosen(chosen)) if chosen.is_empty() => {
            format!("The person chose nothing in {name}.")
        }
        (Holds::Chosen(_), Holds::Chosen(chosen)) => {
            format!("The person chose {} in {name}.", quoted(chosen))
        }
        (Holds::Text(from), Holds::Text(to)) if to.is_empty() => {
            format!("The person emptied {name}, which held {}.", quoted(from))
        }
        (Holds::Text(from), Holds::Text(to)) if from.is_empty() => {
            format!("The person typed {} into {name}.", quoted(to))
        }
        (Holds::Text(from), Holds::Text(to)) => format!(
            "The person changed {name} from {} to {}.",
            quoted(from),
            quoted(to)
        ),
        _ => format!("The person changed {name}."),
    }
}

/// A field's name as said: quoted, or *an unnamed field*.
fn named(name: &str) -> String {
    if name.is_empty() {
        "an unnamed field".to_owned()
    } else {
        format!("the field {}", quoted(name))
    }
}

/// `said` with its first letter upper case.
fn up(said: &str) -> String {
    let mut letters = said.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(letters).collect()
    })
}

/// `said` in quotes, cut to [`MOST_VALUE`] characters.
fn quoted(said: &str) -> String {
    format!(
        "\u{201c}{}\u{201d}",
        crate::agent_protocol::kept(said, MOST_VALUE)
    )
}

/// Every field of `tree`, in order.
fn fields(tree: &Snapshot) -> Vec<Field> {
    let mut found = Vec::new();
    let Some(root) = &tree.root else {
        return found;
    };
    // Walked without recursing, as every reader of a tree here is; a
    // field's own insides are its value, not fields of their own.
    let mut waiting: Vec<(&SnapshotNode, bool)> = vec![(root, false)];
    while let Some((node, secret_above)) = waiting.pop() {
        let secret = secret_above || crate::agent_page::is_secret(node);
        if let Some(holds) = holds(node) {
            let dots = matches!(&holds, Holds::Text(text) if crate::agent_page::is_dots(text));
            found.push(Field {
                id: node.id,
                name: node.name.clone().unwrap_or_default().trim().to_owned(),
                holds,
                secret: secret || dots,
            });
            continue;
        }
        waiting.extend(node.children.iter().rev().map(|child| (child, secret)));
    }
    found
}

/// What `node` holds, if it is a field.
fn holds(node: &SnapshotNode) -> Option<Holds> {
    if matches!(
        node.role,
        Role::Known(KnownRole::ComboBox | KnownRole::ListBox)
    ) && !node.states.takes_text
    {
        let chosen = inside(node)
            .into_iter()
            .find(|option| {
                option.role == Role::Known(KnownRole::Option)
                    && option.states.selected == Some(true)
            })
            .and_then(|option| option.name.clone())
            .unwrap_or_default();
        return Some(Holds::Chosen(chosen.trim().to_owned()));
    }
    if let Some(checked) = node.states.checked {
        return Some(Holds::Checked(checked));
    }
    if node.states.takes_text {
        let words: Vec<String> = inside(node)
            .into_iter()
            .skip(1)
            .filter_map(|inner| inner.name.as_deref())
            .map(str::trim)
            .filter(|words| !words.is_empty())
            .map(str::to_owned)
            .collect();
        return Some(Holds::Text(words.join(" ")));
    }
    None
}

/// `node` and everything inside it, in order.
fn inside(node: &SnapshotNode) -> Vec<&SnapshotNode> {
    let mut all = Vec::new();
    let mut waiting = vec![node];
    while let Some(next) = waiting.pop() {
        all.push(next);
        waiting.extend(next.children.iter().rev());
    }
    all
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
            name: (!name.is_empty()).then(|| name.to_owned()),
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

    fn field(id: usize, name: &str, holds: &str) -> SnapshotNode {
        let inside = if holds.is_empty() {
            Vec::new()
        } else {
            vec![node(id + 100, KnownRole::Text, holds, Vec::new())]
        };
        let mut field = node(id, KnownRole::TextBox, name, inside);
        field.states.takes_text = true;
        field
    }

    fn checkbox(id: usize, name: &str, checked: Checked) -> SnapshotNode {
        let mut box_ = node(id, KnownRole::CheckBox, name, Vec::new());
        box_.states.checked = Some(checked);
        box_
    }

    fn list(id: usize, name: &str, chosen: usize) -> SnapshotNode {
        let options = ["Small", "Large"]
            .iter()
            .enumerate()
            .map(|(at, option)| {
                let mut option = node(id + 10 + at, KnownRole::Option, option, Vec::new());
                option.states.selected = Some(at == chosen);
                option
            })
            .collect();
        node(id, KnownRole::ComboBox, name, options)
    }

    fn page(children: Vec<SnapshotNode>) -> Snapshot {
        Snapshot {
            root: Some(node(1, KnownRole::Document, "", children)),
        }
    }

    const AT: &str = "https://shop.example/basket";

    fn told(before: &Snapshot, after: &Snapshot, address: &str) -> String {
        said(
            Read {
                address: AT,
                tree: before,
            },
            Read {
                address,
                tree: after,
            },
            Reached {
                steps: 3,
                plan: None,
            },
        )
    }

    #[test]
    fn each_field_changed_is_said_by_its_name_with_what_it_holds_now() {
        let before = page(vec![
            field(2, "Name", "Ada"),
            field(3, "Note", ""),
            checkbox(4, "Gift wrap", Checked::No),
            list(5, "Size", 0),
            field(6, "Town", "Kampala"),
        ]);
        let after = page(vec![
            field(2, "Name", "Ada Lovelace"),
            field(3, "Note", "Ring twice"),
            checkbox(4, "Gift wrap", Checked::Yes),
            list(5, "Size", 1),
            field(6, "Town", ""),
        ]);
        assert_eq!(
            told(&before, &after, AT),
            "The person changed the field \u{201c}Name\u{201d} from \u{201c}Ada\u{201d} to \
             \u{201c}Ada Lovelace\u{201d}. The person typed \u{201c}Ring twice\u{201d} into the \
             field \u{201c}Note\u{201d}. The person checked the field \u{201c}Gift wrap\u{201d}. \
             The person chose \u{201c}Large\u{201d} in the field \u{201c}Size\u{201d}. The person \
             emptied the field \u{201c}Town\u{201d}, which held \u{201c}Kampala\u{201d}. alo had \
             taken 3 steps."
        );
    }

    #[test]
    fn a_secret_fields_value_is_never_said_only_that_it_changed() {
        let before = page(vec![
            field(2, "Password", ""),
            field(3, "Card number", ""),
            field(4, "Shipping", "\u{2022}\u{2022}"),
        ]);
        let after = page(vec![
            field(2, "Password", "\u{2022}\u{2022}\u{2022}\u{2022}"),
            field(3, "Card number", "4242 4242 4242 4242"),
            field(4, "Shipping", "\u{2022}\u{2022}\u{2022}"),
        ]);
        let told = told(&before, &after, AT);
        assert!(!told.contains("4242"), "{told}");
        assert!(!told.contains('\u{2022}'), "{told}");
        assert_eq!(
            told.matches("which holds a secret: what it holds is not sent")
                .count(),
            3,
            "{told}"
        );
        assert!(told.contains("\u{201c}Card number\u{201d}"), "{told}");
    }

    #[test]
    fn another_address_is_said_as_before_and_after_and_no_field_is_compared() {
        let before = page(vec![field(2, "Name", "Ada")]);
        let after = page(vec![field(2, "Name", "Grace")]);
        let told = told(&before, &after, "https://shop.example/pay");
        assert_eq!(
            told,
            "The person went from https://shop.example/basket to https://shop.example/pay. alo \
             had taken 3 steps."
        );
    }

    #[test]
    fn fields_that_came_or_went_nothing_changed_and_where_the_plan_was() {
        let before = page(vec![field(2, "Name", "Ada")]);
        let after = page(vec![field(7, "Coupon", "")]);
        let told = told(&before, &after, AT);
        assert_eq!(
            told,
            "The field \u{201c}Coupon\u{201d} appeared. The field \u{201c}Name\u{201d} is gone. \
             alo had taken 3 steps."
        );
        let same = said(
            Read {
                address: AT,
                tree: &before,
            },
            Read {
                address: AT,
                tree: &before,
            },
            Reached {
                steps: 1,
                plan: Some(&Plan {
                    steps: vec!["Fill in the form".to_owned(), "Pay".to_owned()],
                    on: 1,
                }),
            },
        );
        assert_eq!(
            same,
            "The person changed no field alo can see on the page. alo had taken 1 step, and was \
             on step 2 of its plan, \u{201c}Pay\u{201d}."
        );
    }

    #[test]
    fn past_twelve_changes_it_says_how_many_more() {
        let before = page((0..20).map(|at| field(10 + at, "F", "a")).collect());
        let after = page((0..20).map(|at| field(10 + at, "F", "b")).collect());
        let told = told(&before, &after, AT);
        assert_eq!(told.matches("from \u{201c}a\u{201d}").count(), MOST_CHANGES);
        assert!(told.contains("And 8 more fields changed."), "{told}");
    }
}
