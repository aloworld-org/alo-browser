/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A card's page as the agent's model is sent it (queue item 704, ADR 0096
//! § 3): its tree — the one an agent outside this browser reads over the
//! socket (ADR 0002, ADR 0055) — written as text, one node a line, each
//! its number, its role, its name and its states, indented under what it
//! is in, and nothing a person could not read off the page.
//!
//! # What is withheld
//!
//! What a field holds is the text inside it, and some of that is never
//! sent:
//!
//! - **A password**: Chrome's tree holds a password field's value only as
//!   dots, and dots are never sent — a node whose words are only dots is
//!   written as [`WITHHELD`].
//! - **A field whose name or hint says it holds a secret** — a password, a
//!   passcode, a PIN, a card's number, a security code, a one-time code
//!   ([`is_secret`]) — has everything inside it written as [`WITHHELD`],
//!   whatever it holds. A field a page names otherwise is not known to be
//!   one, and that is this rule's limit, said in ADR 0096.
//!
//! No rectangle is sent: a position is never the model's to act on (ADR
//! 0002), and where a node is on the page is said only as `offscreen`.
//!
//! # How much
//!
//! At most [`MOST_CHARACTERS`] characters. A page whose tree is longer is
//! written to there, and then how many nodes were not sent — so the model
//! is told that it was not sent the whole page, rather than believing it
//! was.

use alo_renderer::{Snapshot, SnapshotNode};
use core::fmt::Write as _;

/// The most characters of a tree sent for one step: about twenty-five
/// thousand tokens.
pub const MOST_CHARACTERS: usize = 100_000;

/// What stands for a value that is not sent.
pub const WITHHELD: &str = "(withheld)";

/// `snapshot` as the model is sent it.
pub fn text(snapshot: &Snapshot) -> String {
    let mut out = String::new();
    let mut unsent = 0usize;
    // Walked without recursing: a tree is answered at most 256 deep, and a
    // stack of its own costs nothing to bound.
    let mut waiting: Vec<(&SnapshotNode, usize, bool)> =
        snapshot.root.iter().map(|root| (root, 0, false)).collect();
    while let Some((node, depth, withheld)) = waiting.pop() {
        let line = line_of(node, depth, withheld);
        if unsent > 0 || out.len() + line.len() > MOST_CHARACTERS {
            unsent += 1;
        } else {
            out.push_str(&line);
        }
        let inside = withheld || is_secret(node);
        waiting.extend(
            node.children
                .iter()
                .rev()
                .map(|child| (child, depth + 1, inside)),
        );
    }
    if unsent > 0 {
        let _ = writeln!(
            out,
            "(the rest of the page, {unsent} more nodes, is not sent: the tree is longer than \
             alo sends)"
        );
    }
    out
}

/// One node's line, `depth` deep, its words withheld when it is inside a
/// field that holds a secret, or when they are a password's dots.
fn line_of(node: &SnapshotNode, depth: usize, withheld: bool) -> String {
    let mut line = "  ".repeat(depth);
    let _ = write!(line, "[{}] ", node.id.as_usize());
    let dots = node.name.as_deref().is_some_and(is_dots);
    if (withheld || dots) && node.name.is_some() {
        let _ = write!(line, "{} {WITHHELD}", node.role.as_str());
    } else {
        // The words the tree writes itself, by the same function.
        let _ = write!(line, "{node}");
    }
    if node.offscreen {
        line.push_str(" offscreen");
    }
    line.push('\n');
    line
}

/// Whether `said` is only the dots a password field is drawn with.
fn is_dots(said: &str) -> bool {
    let said = said.trim();
    !said.is_empty()
        && said
            .chars()
            .all(|each| matches!(each, '•' | '●' | '*' | '∙'))
}

/// The words, any one of which in a field's name or hint says it holds a
/// secret.
const SECRET_WORDS: [&str; 8] = [
    "password",
    "passcode",
    "passphrase",
    "pin",
    "cvc",
    "cvv",
    "otp",
    "passwort",
];

/// The phrases that, in a field's name or hint, say it holds a secret.
const SECRET_PHRASES: [&str; 6] = [
    "card number",
    "security code",
    "one-time",
    "one time code",
    "verification code",
    "mot de passe",
];

/// Whether `node` is a field — it takes text — whose name or hint says it
/// holds a secret.
pub fn is_secret(node: &SnapshotNode) -> bool {
    if !node.states.takes_text {
        return false;
    }
    [node.name.as_deref(), node.placeholder.as_deref()]
        .into_iter()
        .flatten()
        .any(says_a_secret)
}

/// Whether `said` names a secret, by a word or a phrase, whatever its case.
fn says_a_secret(said: &str) -> bool {
    let lower = said.to_lowercase();
    let words: Vec<&str> = lower
        .split(|each: char| !each.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    words.iter().any(|word| SECRET_WORDS.contains(word))
        || SECRET_PHRASES.iter().any(|phrase| lower.contains(phrase))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_box::{BoxId, KnownRole, Role, States};
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
            states: States::default(),
            placeholder: None,
            marker: None,
            rect: Rect::new(10.0, 20.0, 30.0, 40.0),
            rects: vec![Rect::new(10.0, 20.0, 30.0, 40.0)],
            offscreen: false,
            scrolls: false,
            children,
        }
    }

    fn field(id: usize, name: &str, holds: &str) -> SnapshotNode {
        let mut field = node(
            id,
            Role::Known(KnownRole::TextBox),
            Some(name),
            vec![node(
                id + 100,
                Role::Known(KnownRole::Text),
                Some(holds),
                Vec::new(),
            )],
        );
        field.states.takes_text = true;
        field
    }

    fn page(children: Vec<SnapshotNode>) -> Snapshot {
        Snapshot {
            root: Some(node(1, Role::Known(KnownRole::Document), None, children)),
        }
    }

    #[test]
    fn each_node_is_a_line_of_its_number_role_name_and_states_indented() {
        let mut button = node(3, Role::Known(KnownRole::Button), Some("Send"), Vec::new());
        button.states.disabled = true;
        button.offscreen = true;
        let written = text(&page(vec![field(2, "Name", "Ada"), button]));
        assert_eq!(
            written,
            "[1] document\n  [2] textbox \"Name\"\n    [102] text \"Ada\"\n  [3] button \"Send\" [disabled] offscreen\n"
        );
        assert!(!written.contains("10, 20"), "no position is sent");
    }

    #[test]
    fn a_passwords_dots_and_what_a_secret_field_holds_are_withheld() {
        let written = text(&page(vec![
            field(2, "Password", "\u{2022}\u{2022}\u{2022}\u{2022}"),
            field(3, "Card number", "4242 4242 4242 4242"),
            field(4, "Your PIN", "1234"),
            field(5, "Spinner speed", "fast"),
            field(6, "Shipping", "hunter2"),
        ]));
        assert!(!written.contains("4242"), "{written}");
        assert!(!written.contains("1234"), "{written}");
        assert!(!written.contains('\u{2022}'), "{written}");
        assert!(written.contains("[102] text (withheld)"), "{written}");
        assert!(written.contains("[103] text (withheld)"), "{written}");
        assert!(written.contains("[104] text (withheld)"), "{written}");
        // A word inside another word is not one: these are sent.
        assert!(written.contains("\"fast\""), "{written}");
        assert!(written.contains("\"hunter2\""), "{written}");
        // The fields' own names, a person's to read, are sent.
        assert!(written.contains("[3] textbox \"Card number\""), "{written}");
    }

    #[test]
    fn a_tree_past_the_most_sent_says_how_many_nodes_were_not() {
        let many: Vec<SnapshotNode> = (0..20_000)
            .map(|at| {
                node(
                    at + 10,
                    Role::Known(KnownRole::Text),
                    Some("words words"),
                    Vec::new(),
                )
            })
            .collect();
        let written = text(&page(many));
        assert!(written.len() <= MOST_CHARACTERS + 200);
        let last = written.lines().last().unwrap_or_default();
        assert!(
            last.starts_with("(the rest of the page, ") && last.contains("more nodes, is not sent"),
            "{last}"
        );
    }
}
