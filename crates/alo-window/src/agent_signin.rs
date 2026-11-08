/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether a page alo's agent reached asks the person to sign in (queue
//! item 821, ADR 0103, `docs/design/shell.md`, *Beginning with no page*):
//! read off the page's tree by the browser process, never asked of the
//! model.
//!
//! A page asks to sign in when, on the screen, it holds **a field for a
//! password** — one that takes text whose name or hint says *password*,
//! *passcode*, *passphrase*, *passwort* or *mot de passe*, or whose words
//! are a filled password's dots. It is judged only on a page the run has
//! just reached — opened, or gone to by a link or a form — so a page the
//! person was let continue on does not stop it again; and whenever a page
//! refuses the agent a password, which is the agent trying to sign in.
//! Then the run hands the page over, as when the person takes it over, and
//! *Let alo continue* carries it on from the page as they leave it.
//!
//! **Its limit**: a sign-in page whose password field is named otherwise,
//! or that asks for a password only after an address is given, is not
//! known to be one until the agent is refused a password there.

use crate::agent_page::is_dots;
use alo_renderer::{Snapshot, SnapshotNode};

/// The words, any one of which in a field's name or hint says it is for a
/// password.
const PASSWORD_WORDS: [&str; 4] = ["password", "passcode", "passphrase", "passwort"];

/// Whether `snapshot` asks the person to sign in: it shows a field for a
/// password.
pub fn asks_to_sign_in(snapshot: &Snapshot) -> bool {
    // Walked without recursing, as the tree is written.
    let mut waiting: Vec<&SnapshotNode> = snapshot.root.iter().collect();
    while let Some(node) = waiting.pop() {
        if !node.offscreen && is_for_a_password(node) {
            return true;
        }
        waiting.extend(node.children.iter());
    }
    false
}

/// Whether `node` is a field for a password.
fn is_for_a_password(node: &SnapshotNode) -> bool {
    if !node.states.takes_text {
        return false;
    }
    let named = [node.name.as_deref(), node.placeholder.as_deref()]
        .into_iter()
        .flatten()
        .any(|said| {
            let lower = said.to_lowercase();
            lower.contains("mot de passe")
                || lower
                    .split(|each: char| !each.is_alphanumeric())
                    .any(|word| PASSWORD_WORDS.contains(&word))
        });
    named
        || node.name.as_deref().is_some_and(is_dots)
        || node
            .children
            .iter()
            .any(|child| child.name.as_deref().is_some_and(is_dots))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_box::{BoxId, KnownRole, Role, States};
    use alo_layout::Rect;

    fn node(id: usize, role: KnownRole, name: Option<&str>, field: bool) -> SnapshotNode {
        SnapshotNode {
            id: BoxId::from_wire(id),
            role: Role::Known(role),
            name: name.map(str::to_owned),
            states: States {
                takes_text: field,
                ..States::default()
            },
            placeholder: None,
            marker: None,
            rect: Rect::default(),
            rects: Vec::new(),
            offscreen: false,
            scrolls: false,
            children: Vec::new(),
        }
    }

    fn page(children: Vec<SnapshotNode>) -> Snapshot {
        let mut root = node(1, KnownRole::Document, Some("Sign in"), false);
        root.children = children;
        Snapshot { root: Some(root) }
    }

    #[test]
    fn a_password_field_on_screen_is_a_sign_in_page() {
        assert!(asks_to_sign_in(&page(vec![
            node(2, KnownRole::TextBox, Some("Email"), true),
            node(3, KnownRole::TextBox, Some("Password"), true),
        ])));
        let mut hinted = node(3, KnownRole::TextBox, None, true);
        hinted.placeholder = Some("Mot de passe".to_owned());
        assert!(asks_to_sign_in(&page(vec![hinted])));
        assert!(asks_to_sign_in(&page(vec![node(
            3,
            KnownRole::TextBox,
            Some("\u{2022}\u{2022}\u{2022}\u{2022}"),
            true
        )])));
    }

    #[test]
    fn a_page_that_only_speaks_of_passwords_or_hides_the_field_is_not() {
        // Words about a password, a link to reset one, a button to sign in:
        // none is a field.
        assert!(!asks_to_sign_in(&page(vec![
            node(2, KnownRole::Link, Some("Forgot your password?"), false),
            node(3, KnownRole::Button, Some("Sign in"), false),
            node(4, KnownRole::Text, Some("Choose a strong password."), false),
        ])));
        // A card's security code is paying, not signing in.
        assert!(!asks_to_sign_in(&page(vec![node(
            2,
            KnownRole::TextBox,
            Some("Security code"),
            true
        )])));
        // Not on the screen.
        let mut hidden = node(3, KnownRole::TextBox, Some("Password"), true);
        hidden.offscreen = true;
        assert!(!asks_to_sign_in(&page(vec![hidden])));
        assert!(!asks_to_sign_in(&Snapshot { root: None }));
    }
}
