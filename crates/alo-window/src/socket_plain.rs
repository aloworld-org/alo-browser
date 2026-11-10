/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The agent socket's answers written plainly, for a person at a terminal
//! (queue item 915): `alo-ask --plain`.
//!
//! An answer on the socket is one JSON object on a line
//! (`docs/contracts/agent-socket.md`), which a program reads and a person
//! does not. This writes the same answer as a person reads it — a tree as
//! one node a line, indented under what it is in:
//!
//! ```text
//! tab https://example.com/shop
//! [1] RootWebArea "Shop"
//!   [12] list "Products", 3 items
//!     [13] listitem "Kettle"
//!       [14] button "Add to cart"
//! ```
//!
//! — each node its number in brackets, its role, its name in quotes and the
//! states the contract gives it, as alo's own agent reads the page
//! ([`crate::agent_page`]). **No position is written**: a rectangle is read
//! and never acted on (ADR 0002), and a person deciding what to press needs
//! the name, which is what a verb takes. A node of no role worth reading — a
//! `generic` box with no name and no states — is not written, and what is
//! inside it is written where it would have been, one level up; a list
//! says how many items it holds.
//!
//! Every other answer is its `said`, the sentence the browser wrote for a
//! person: a verb's outcome, a refusal with its kind beside it, the
//! renderer. The tabs are their addresses, one a line.
//!
//! # Read from the browser, and never trusted further
//!
//! The answer is read by the rented reader, as the socket's other end reads
//! a request ([`crate::protocol`]). A line that is not an answer this
//! knows, or that the reader will not take — a tree nested deeper than it
//! reads — is written as it came, so nothing the browser said is lost.
//! Nothing is cut.

use core::fmt::Write as _;
use serde_json::{Map, Value};

/// What a node with none of its own is written as having.
const UNNAMED: &str = "";

/// `line`, an answer from the agent socket, written for a person: one or
/// more lines, each ending in a newline.
pub fn written(line: &str) -> String {
    let line = line.trim_end_matches(['\n', '\r']);
    let Ok(Value::Object(answer)) = serde_json::from_str::<Value>(line) else {
        return format!("{line}\n");
    };
    let said = answer.get("said").and_then(Value::as_str);
    match answer.get("answer").and_then(Value::as_str) {
        Some("tabs") => tabs(&answer).unwrap_or_else(|| format!("{line}\n")),
        Some("tree") => tree(&answer).unwrap_or_else(|| format!("{line}\n")),
        Some("done") => done(&answer, said).unwrap_or_else(|| format!("{line}\n")),
        Some("refused") => refused(&answer, said).unwrap_or_else(|| format!("{line}\n")),
        Some("renderer") => said.map_or_else(|| format!("{line}\n"), |said| format!("{said}\n")),
        _ => format!("{line}\n"),
    }
}

/// Every tab's address, one a line.
fn tabs(answer: &Map<String, Value>) -> Option<String> {
    let tabs = answer.get("tabs")?.as_array()?;
    if tabs.is_empty() {
        return Some("no tab is open\n".to_owned());
    }
    let mut out = String::new();
    for tab in tabs {
        let _ = writeln!(out, "{}", tab.as_str()?);
    }
    Some(out)
}

/// A verb's outcome, as the browser said it, and what the page's script
/// said while it ran.
fn done(answer: &Map<String, Value>, said: Option<&str>) -> Option<String> {
    let mut out = format!("{}\n", said?);
    for issue in answer.get("issues").and_then(Value::as_array)? {
        let _ = writeln!(out, "  the page said: {}", issue.as_str()?);
    }
    Some(out)
}

/// A refusal: its kind, which a program reads, and its sentence.
fn refused(answer: &Map<String, Value>, said: Option<&str>) -> Option<String> {
    let kind = answer.get("refusal")?.get("kind")?.as_str()?;
    Some(format!("refused ({kind}): {}\n", said.unwrap_or(UNNAMED)))
}

/// The tab and its tree, one node a line.
fn tree(answer: &Map<String, Value>) -> Option<String> {
    let mut out = format!("tab {}\n", answer.get("tab")?.as_str()?);
    let root = answer.get("tree")?;
    if root.is_null() {
        out.push_str("nothing is on the page\n");
        return Some(out);
    }
    // Walked without recursing, with a stack of its own: how deep a tree
    // is is the browser's to bound, never this program's stack's.
    let mut waiting: Vec<(&Value, usize)> = vec![(root, 0)];
    while let Some((node, depth)) = waiting.pop() {
        let node = node.as_object()?;
        let children = node.get("children")?.as_array()?;
        let inside = if worth_writing(node) {
            out.push_str(&line_of(node, children, depth)?);
            depth + 1
        } else {
            depth
        };
        waiting.extend(children.iter().rev().map(|child| (child, inside)));
    }
    Some(out)
}

/// Whether `node` is written: not when it is a `generic` box — or one of
/// no role — with no name and no states, which says nothing a person
/// reads.
fn worth_writing(node: &Map<String, Value>) -> bool {
    let role = node.get("role").and_then(Value::as_str).unwrap_or(UNNAMED);
    let plain = matches!(role, "generic" | "none" | "presentation" | "");
    !plain
        || node.contains_key("name")
        || node.contains_key("states")
        || node.contains_key("placeholder")
}

/// One node's line, `depth` deep.
fn line_of(node: &Map<String, Value>, children: &[Value], depth: usize) -> Option<String> {
    let mut line = "  ".repeat(depth);
    let id = node.get("node")?.as_u64()?;
    let role = node.get("role")?.as_str()?;
    let _ = write!(line, "[{id}] {role}");
    if let Some(name) = node.get("name").and_then(Value::as_str) {
        let _ = write!(line, " \"{}\"", one_line(name));
    }
    if let Some(items) = items_of(role, children) {
        let _ = write!(
            line,
            ", {items} {}",
            if items == 1 { "item" } else { "items" }
        );
    }
    if let Some(states) = node.get("states").and_then(Value::as_object) {
        for (state, value) in states {
            match value {
                Value::Bool(true) => {
                    let _ = write!(line, " {state}");
                }
                Value::String(value) => {
                    let _ = write!(line, " {state}={value}");
                }
                other => {
                    let _ = write!(line, " {state}={other}");
                }
            }
        }
    }
    if let Some(placeholder) = node.get("placeholder").and_then(Value::as_str) {
        let _ = write!(line, " placeholder \"{}\"", one_line(placeholder));
    }
    for flag in ["offscreen", "undrawn"] {
        if node.get(flag).and_then(Value::as_bool) == Some(true) {
            let _ = write!(line, " {flag}");
        }
    }
    line.push('\n');
    Some(line)
}

/// How many items a list of `role` holds among its `children`, for a role
/// that is a list of something — when it holds any.
fn items_of(role: &str, children: &[Value]) -> Option<usize> {
    let item = match role {
        "list" => "listitem",
        "listbox" => "option",
        "menu" | "menubar" => "menuitem",
        "tablist" => "tab",
        "radiogroup" => "radio",
        _ => return None,
    };
    let items = children
        .iter()
        .filter(|child| child.get("role").and_then(Value::as_str) == Some(item))
        .count();
    // A list a page wrote with no items of that role among its children
    // says nothing by a count of none.
    (items > 0).then_some(items)
}

/// `text` on one line: a name a page broke over several is written with
/// its breaks as spaces, and a quote inside it escaped.
fn one_line(text: &str) -> String {
    text.split(['\n', '\r'])
        .collect::<Vec<_>>()
        .join(" ")
        .replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asking::{Answer, Refused};
    use crate::protocol;
    use alo_agent::Outcome;
    use alo_box::{BoxId, KnownRole, Role, States};
    use alo_layout::Rect;
    use alo_renderer::{Snapshot, SnapshotNode};

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
            states: States::default(),
            placeholder: None,
            marker: None,
            rect: Rect::new(717.5, 626.25, 535.75, 444.5),
            rects: vec![Rect::new(717.5, 626.25, 535.75, 444.5)],
            offscreen: false,
            scrolls: false,
            children,
        }
    }

    /// The answer as the browser writes it on the wire.
    fn on_the_wire(answer: &Answer) -> String {
        protocol::write(None, answer)
    }

    fn shop() -> Snapshot {
        let item = |id: usize, name: &str| {
            node(
                id,
                Role::Known(KnownRole::ListItem),
                Some(name),
                vec![node(
                    id + 1,
                    Role::Known(KnownRole::Button),
                    Some("Add to cart"),
                    Vec::new(),
                )],
            )
        };
        let wrapper = node(
            3,
            Role::Generic,
            None,
            vec![node(
                4,
                Role::Known(KnownRole::List),
                Some("Products"),
                vec![item(10, "Kettle"), item(20, "Teapot"), item(30, "Mug")],
            )],
        );
        let mut field = node(
            5,
            Role::Known(KnownRole::TextBox),
            Some("Search"),
            Vec::new(),
        );
        field.states.required = true;
        field.placeholder = Some("Find a \"thing\"".to_owned());
        field.offscreen = true;
        let mut heading = node(
            6,
            Role::Known(KnownRole::Heading),
            Some("Our\nshop"),
            Vec::new(),
        );
        heading.states.level = Some(2);
        Snapshot {
            root: Some(node(
                1,
                Role::Declared("RootWebArea".into()),
                Some("Shop"),
                vec![heading, wrapper, field],
            )),
        }
    }

    #[test]
    fn a_tree_is_one_node_a_line_indented_with_no_position() {
        let line = on_the_wire(&Answer::Tree {
            tab: "https://example.com/shop".to_owned(),
            snapshot: Box::new(shop()),
        });
        assert_eq!(
            written(&line),
            "tab https://example.com/shop\n\
             [1] RootWebArea \"Shop\"\n\
             \x20 [6] heading \"Our shop\" level=2\n\
             \x20 [4] list \"Products\", 3 items\n\
             \x20   [10] listitem \"Kettle\"\n\
             \x20     [11] button \"Add to cart\"\n\
             \x20   [20] listitem \"Teapot\"\n\
             \x20     [21] button \"Add to cart\"\n\
             \x20   [30] listitem \"Mug\"\n\
             \x20     [31] button \"Add to cart\"\n\
             \x20 [5] textbox \"Search\" required placeholder \"Find a \\\"thing\\\"\" offscreen\n"
        );
        // The rectangles are on the wire, and none of their numbers is
        // written.
        assert!(line.contains("\"rect\""));
        for number in ["717.5", "626.25", "535.75", "444.5"] {
            assert!(line.contains(number));
            assert!(!written(&line).contains(number));
        }
    }

    #[test]
    fn a_list_with_no_items_of_its_own_is_given_no_count() {
        let group = node(
            8,
            Role::Known(KnownRole::Group),
            Some("Categories"),
            Vec::new(),
        );
        let list = node(7, Role::Known(KnownRole::List), None, vec![group]);
        let line = on_the_wire(&Answer::Tree {
            tab: "https://example.com/".to_owned(),
            snapshot: Box::new(Snapshot { root: Some(list) }),
        });
        assert_eq!(
            written(&line),
            "tab https://example.com/\n[7] list\n  [8] group \"Categories\"\n"
        );
    }

    #[test]
    fn a_page_with_nothing_on_it_says_so() {
        let line = on_the_wire(&Answer::Tree {
            tab: "https://example.com/".to_owned(),
            snapshot: Box::new(Snapshot { root: None }),
        });
        assert_eq!(
            written(&line),
            "tab https://example.com/\nnothing is on the page\n"
        );
    }

    #[test]
    fn tabs_are_their_addresses_one_a_line() {
        let two = on_the_wire(&Answer::Tabs(vec![
            "https://a.example/".to_owned(),
            "https://b.example/x".to_owned(),
        ]));
        assert_eq!(written(&two), "https://a.example/\nhttps://b.example/x\n");
        assert_eq!(
            written(&on_the_wire(&Answer::Tabs(Vec::new()))),
            "no tab is open\n"
        );
    }

    #[test]
    fn a_verb_done_is_its_sentence_and_what_the_page_said() {
        let line = on_the_wire(&Answer::Done {
            tab: "https://example.com/shop".to_owned(),
            outcome: Outcome::Activated {
                node: BoxId::from_wire(11),
                name: Some("Add to cart".to_owned()),
            },
            issues: vec!["console.log: added".to_owned()],
        });
        assert_eq!(
            written(&line),
            "activated box#11 \"Add to cart\"\n  the page said: console.log: added\n"
        );
    }

    #[test]
    fn a_refusal_is_its_kind_and_its_sentence() {
        let line = on_the_wire(&Answer::Refused(Refused::NoSuchTab {
            tab: "https://nowhere.example/".to_owned(),
        }));
        let plain = written(&line);
        assert!(plain.starts_with("refused (NoSuchTab): "), "{plain}");
        assert!(plain.ends_with('\n'));
        assert_eq!(plain.lines().count(), 1);
    }

    #[test]
    fn what_is_not_an_answer_it_knows_is_written_as_it_came() {
        for line in [
            "not json at all",
            "[1,2]",
            "{\"answer\":\"later\",\"v\":2}",
            "{\"answer\":\"tree\",\"tab\":\"x\",\"tree\":{\"role\":\"button\"}}",
        ] {
            assert_eq!(written(&format!("{line}\n")), format!("{line}\n"));
        }
    }

    #[test]
    fn a_tree_deeper_than_the_reader_takes_is_written_as_it_came() {
        let mut deepest = node(
            9999,
            Role::Known(KnownRole::Button),
            Some("Deep"),
            Vec::new(),
        );
        for id in (0..200).rev() {
            deepest = node(id, Role::Known(KnownRole::Group), Some("g"), vec![deepest]);
        }
        let line = on_the_wire(&Answer::Tree {
            tab: "https://example.com/".to_owned(),
            snapshot: Box::new(Snapshot {
                root: Some(deepest),
            }),
        });
        assert_eq!(written(&line), line);
    }
}
