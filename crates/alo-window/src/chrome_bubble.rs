/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The bubble Chrome shows over a form it would not send, read from its
//! tree (queue item 631, ADR 0051's amendment of 25 October 2026).
//!
//! When a press on a submit button finds a control failing its checks,
//! Chrome sends nothing, gives the first failing control the keyboard, and
//! draws its words beside it in a bubble. Measured against Chrome
//! 155.0.8059.86, headless and shown alike, that bubble is in the page's
//! accessibility tree as **an `alert` describing no DOM node**, a child of
//! the page's root, named with the words a person reads in it. A page's own
//! `role="alert"` always describes an element, so a page cannot make one;
//! and **each bubble is a node of its own**, a press refused twice in a row
//! two ids, a click anywhere else taking it away. So a bubble that is in
//! the tree after a press and was not before it is that press's.
//!
//! # What its words are
//!
//! Its name is Chrome's message, a space, and what follows it in the bubble,
//! with its spaces run together: a `pattern`'s `title` follows, so the
//! words are `Please match the requested format. Digits only`, and most
//! messages end in the space. The words here are the name with its trailing
//! space taken off: what a person reads, which for every message this
//! engine also says is this engine's message exactly.
//!
//! # What it does not say
//!
//! **Which constraint.** A page may write any words it likes with
//! `setCustomValidity`, `Please fill out this field.` among them, so the
//! words are not taken for a constraint. **Which other controls failed**:
//! Chrome shows one bubble, for the first. And **nothing at all** when a
//! page cancels the control's `invalid` event, or when the failing control
//! cannot be given the keyboard — a hidden `required` field — though the
//! form is not sent either (queue item 646).

use crate::chrome_protocol::AxNode;
use std::collections::HashSet;

/// What Chrome's bubble says, and which control it is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bubble {
    /// The control it is about, by Chrome's `backendNodeId`: the one Chrome
    /// gave the keyboard.
    pub control: u64,
    /// Its words, as a person reads them.
    pub message: String,
}

/// The bubble in `after` that was not in `before`, and the control Chrome
/// gave the keyboard to: [`None`] when there is no new one, when there are
/// several (which Chrome was not measured to show, and which would leave
/// which is the press's a guess), when it says nothing, or when no one
/// control has the keyboard.
pub fn shown(before: &[AxNode], after: &[AxNode]) -> Option<Bubble> {
    let was: HashSet<&str> = before.iter().map(|node| node.id.as_str()).collect();
    let roots: HashSet<&str> = after
        .iter()
        .filter(|node| node.role == "RootWebArea")
        .flat_map(|node| node.children.iter().map(String::as_str))
        .collect();
    let mut bubbles = after.iter().filter(|node| {
        node.role == "alert"
            && node.node.is_none()
            && !node.ignored
            && roots.contains(node.id.as_str())
            && !was.contains(node.id.as_str())
    });
    let bubble = bubbles.next()?;
    if bubbles.next().is_some() {
        return None;
    }
    let message = bubble.name.as_deref()?.trim_end();
    if message.is_empty() {
        return None;
    }
    let mut focused = after.iter().filter(|node| {
        node.role != "RootWebArea"
            && !node.ignored
            && node
                .properties
                .iter()
                .any(|(name, value)| name == "focused" && value == "true")
    });
    let control = focused.next()?.node?;
    if focused.next().is_some() {
        return None;
    }
    Some(Bubble {
        control,
        message: message.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ax(id: &str, role: &str, dom: Option<u64>, children: &[&str]) -> AxNode {
        AxNode {
            id: id.to_owned(),
            role: role.to_owned(),
            node: dom,
            children: children.iter().map(|child| (*child).to_owned()).collect(),
            ..AxNode::default()
        }
    }

    fn focused(mut node: AxNode) -> AxNode {
        node.properties
            .push(("focused".to_owned(), "true".to_owned()));
        node
    }

    fn named(mut node: AxNode, name: &str) -> AxNode {
        node.name = Some(name.to_owned());
        node
    }

    /// A page with a form, as Chrome 155's tree read it before a press:
    /// the root, the page's own alert, a field and a button.
    fn before() -> Vec<AxNode> {
        vec![
            focused(ax("12", "RootWebArea", Some(12), &["13"])),
            ax("13", "generic", Some(13), &["16", "3", "9"]),
            named(
                ax("16", "alert", Some(16), &[]),
                "Please fill out this field. ",
            ),
            ax("3", "textbox", Some(3), &[]),
            ax("9", "button", Some(9), &[]),
        ]
    }

    /// The same page after a press Chrome refused: a bubble beside the
    /// root's first child, and the field given the keyboard.
    fn after(words: &str) -> Vec<AxNode> {
        vec![
            focused(ax("12", "RootWebArea", Some(12), &["-1000000012", "13"])),
            named(ax("-1000000012", "alert", None, &[]), words),
            ax("13", "generic", Some(13), &["16", "3", "9"]),
            named(
                ax("16", "alert", Some(16), &[]),
                "Please fill out this field. ",
            ),
            focused(ax("3", "textbox", Some(3), &[])),
            ax("9", "button", Some(9), &[]),
        ]
    }

    #[test]
    fn a_new_bubble_is_the_press_and_its_words_are_what_a_person_reads() {
        assert_eq!(
            shown(&before(), &after("Please fill out this field. ")),
            Some(Bubble {
                control: 3,
                message: "Please fill out this field.".to_owned(),
            })
        );
        assert_eq!(
            shown(
                &before(),
                &after("Please match the requested format. Digits only")
            )
            .map(|bubble| bubble.message),
            Some("Please match the requested format. Digits only".to_owned()),
            "a pattern's title is read after its message, as it is shown"
        );
    }

    #[test]
    fn a_page_cannot_make_a_bubble_of_its_own() {
        // The page's alert, with Chrome's words, is no bubble: it describes
        // an element.
        let mut page = before();
        if let Some(field) = page.get_mut(3) {
            *field = focused(ax("3", "textbox", Some(3), &[]));
        }
        assert_eq!(shown(&before(), &page), None);
        // Nor is one with no element that is not the root's child.
        let mut deep = after("Please fill out this field. ");
        if let Some(root) = deep.first_mut() {
            root.children = vec!["13".to_owned()];
        }
        if let Some(generic) = deep.get_mut(2) {
            generic.children.push("-1000000012".to_owned());
        }
        assert_eq!(shown(&before(), &deep), None);
    }

    #[test]
    fn a_bubble_that_was_there_before_the_press_is_not_the_press() {
        let lingering = after("Please fill out this field. ");
        assert_eq!(shown(&lingering, &lingering), None);
        // A press refused again shows a bubble of its own.
        let mut again = after("Please fill out this field. ");
        if let Some(root) = again.first_mut() {
            root.children = vec!["-1000000014".to_owned(), "13".to_owned()];
        }
        if let Some(bubble) = again.get_mut(1) {
            bubble.id = "-1000000014".to_owned();
        }
        assert_eq!(
            shown(&lingering, &again).map(|bubble| bubble.control),
            Some(3)
        );
    }

    #[test]
    fn a_bubble_that_leaves_a_guess_is_not_read() {
        assert_eq!(shown(&before(), &after("  ")), None, "it says nothing");
        let mut unnamed = after("x");
        if let Some(bubble) = unnamed.get_mut(1) {
            bubble.name = None;
        }
        assert_eq!(shown(&before(), &unnamed), None, "it has no name");
        let mut ignored = after("x");
        if let Some(bubble) = ignored.get_mut(1) {
            bubble.ignored = true;
        }
        assert_eq!(shown(&before(), &ignored), None, "Chrome ignores it");
        let mut two = after("x");
        if let Some(root) = two.first_mut() {
            root.children.push("-7".to_owned());
        }
        two.push(named(ax("-7", "alert", None, &[]), "y"));
        assert_eq!(shown(&before(), &two), None, "two new bubbles");
        let mut nobody = after("x");
        if let Some(field) = nobody.get_mut(4) {
            field.properties.clear();
        }
        assert_eq!(shown(&before(), &nobody), None, "no control has the keys");
        let mut both = after("x");
        if let Some(button) = both.get_mut(5) {
            *button = focused(ax("9", "button", Some(9), &[]));
        }
        assert_eq!(shown(&before(), &both), None, "two controls have them");
        let mut elementless = after("x");
        if let Some(field) = elementless.get_mut(4) {
            field.node = None;
        }
        assert_eq!(shown(&before(), &elementless), None, "keys on no element");
        assert_eq!(shown(&[], &[]), None, "an empty tree");
    }
}
