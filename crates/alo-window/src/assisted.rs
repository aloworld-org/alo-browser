/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What assistive technology may ask of alo's own interface, in this
//! browser's own words (ADR 0115 § 2 and § 4, queue item 987).
//!
//! The platform's accessibility bus is reached through AccessKit, which is
//! named in two files and no other ([`crate::accessible`], the adapter, and
//! [`crate::accessible_tree`], the mapping). What they hand the window —
//! that something on the bus is listening, or what it asked — is said here,
//! as values of ours, and so is what each node is offered: one rule, which
//! the mapping publishes and the window checks a request against.
//!
//! # What is asked
//!
//! Three actions, each by node, never by place (ADR 0002):
//!
//! - [`Assist::Click`]: the press a person's press on that node would be,
//!   claimed from the node by the renderer that drew its part and decided
//!   by the browser process as every press is.
//! - [`Assist::SetValue`]: the text a field holds replaced — the field given
//!   the keyboard as a press on it would give it, then everything in it
//!   replaced by the text, as `put_text` puts text.
//! - [`Assist::Focus`]: the person's keyboard moved to that field, as a
//!   press on it would move it. Only a field is offered it: nothing else
//!   in alo's own interface holds the keyboard.
//!
//! Every other action on the bus is refused before it reaches the window
//! ([`crate::accessible_tree::asked`]): a point is never a verb's.
//!
//! # What is never offered or published
//!
//! ADR 0115 § 4, the socket's rule kept exactly: a password field, and a
//! field whose name or hint says it holds a secret
//! ([`crate::agent_page::is_secret`]) — a passcode, a PIN, a card's number,
//! security code or expiry, a one-time code — is published with its role
//! and name and no value, nothing inside it, and is never offered
//! [`Assist::SetValue`].
//!
//! # A card's page
//!
//! ADR 0115 § 3, phase (b) (queue item 988): the page a card shows is read
//! from the tree the agent socket reads ([`crate::chrome_tree`], through the
//! conversation's relay), published under the card's picture of it
//! ([`PublishedPage`]), and acted on by node as the socket's `act` acts
//! ([`crate::assisted_page`]). The same three actions, the same offers and
//! the same secrets as alo's own interface: one rule.
//!
//! # A press that pays
//!
//! ADR 0115, *Answered* 1: a press from the bus that pays is held for the
//! person's yes as alo's agent's is (ADR 0099). No press of alo's own
//! interface pays; a press on a card's page that the person's gate says
//! pays is held ([`crate::assisted_page::judged`]).

use crate::conversation_drawing::Part;
use crate::place::PixelRect;
use alo_box::{BoxId, KnownRole, Role};
use alo_renderer::{Snapshot, SnapshotNode};
use core::fmt;

/// What assistive technology asked be done to a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Assist {
    /// Press it.
    Click,
    /// Put this text in the field, in place of what it holds.
    SetValue(String),
    /// Give the field the keyboard.
    Focus,
}

impl fmt::Display for Assist {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Assist::Click => f.write_str("a press"),
            Assist::SetValue(text) => {
                write!(f, "text put in, {} characters", text.chars().count())
            }
            Assist::Focus => f.write_str("the keyboard moved"),
        }
    }
}

/// An action asked of one node of one part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// Which part's tree the node is in.
    pub part: Part,
    /// The node, by its id in the tree that part last answered.
    pub node: BoxId,
    /// What to do.
    pub assist: Assist,
}

/// An action refused before it reached the window, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unasked {
    /// What was asked, as the bus names it.
    pub action: String,
    /// Why it is refused.
    pub why: &'static str,
}

/// What the bus told the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FromTheBus {
    /// Something on the bus asked for the window's tree: it is read from
    /// each part, and again after each frame, from now on.
    Active,
    /// Nothing on the bus is listening any more.
    Inactive,
    /// An action, by node.
    Asked(Asked),
    /// An action on a node of the card's page, by its id in the tree the
    /// page was last read as (ADR 0115 § 3).
    Page {
        /// The node.
        node: BoxId,
        /// What to do.
        assist: Assist,
    },
    /// An action refused as it was asked.
    Refused(Unasked),
}

/// How the bus tells the window: whether the window was there to hear it.
pub type TellTheWindow = std::sync::Arc<dyn Fn(FromTheBus) -> bool + Send + Sync>;

/// Which of the three actions a node is offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Offers {
    /// [`Assist::Click`].
    pub click: bool,
    /// [`Assist::SetValue`].
    pub set_value: bool,
    /// [`Assist::Focus`].
    pub focus: bool,
}

impl Offers {
    /// Whether `assist` is among them.
    pub fn holds(self, assist: &Assist) -> bool {
        match assist {
            Assist::Click => self.click,
            Assist::SetValue(_) => self.set_value,
            Assist::Focus => self.focus,
        }
    }
}

/// Why a field's value is never published (ADR 0115 § 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Secret {
    /// A password field: ARIA gives it no role, and it takes text.
    Password,
    /// A field whose name or hint says it holds a secret.
    Named,
}

/// Why `node`'s value is never published, if it is not.
///
/// A password field of alo's own has no role (ARIA gives it none); one on
/// a card's page is a `textbox` in Chrome's tree, which says nothing more
/// of it, and is known by what it holds being only a password's dots
/// ([`crate::agent_page::is_dots`], as the socket's text withholds them).
pub fn secret(node: &SnapshotNode) -> Option<Secret> {
    if !node.states.takes_text {
        return None;
    }
    if matches!(node.role, Role::Generic | Role::Presentational) || holds_dots(node) {
        return Some(Secret::Password);
    }
    crate::agent_page::is_secret(node).then_some(Secret::Named)
}

/// Whether a run of text inside `node` is only a password's dots.
fn holds_dots(node: &SnapshotNode) -> bool {
    let mut waiting: Vec<&SnapshotNode> = node.children.iter().collect();
    while let Some(each) = waiting.pop() {
        if each.name.as_deref().is_some_and(crate::agent_page::is_dots) {
            return true;
        }
        waiting.extend(each.children.iter());
    }
    false
}

/// What a person presses: what a press on it claims in alo's own
/// interface, and what assistive technology is offered [`Assist::Click`]
/// on.
const PRESSED: [KnownRole; 9] = [
    KnownRole::Button,
    KnownRole::Link,
    KnownRole::Tab,
    KnownRole::MenuItem,
    KnownRole::Option,
    KnownRole::CheckBox,
    KnownRole::Radio,
    KnownRole::Switch,
    KnownRole::Summary,
];

/// Which actions `node` is offered: a press on what a person presses or
/// types into, the keyboard for a field, and its text for a field that
/// can be typed into and holds no secret — none on anything disabled.
pub fn offers(node: &SnapshotNode) -> Offers {
    if node.states.disabled {
        return Offers::default();
    }
    let field = node.states.takes_text;
    let pressed = matches!(node.role, Role::Known(known) if PRESSED.contains(&known));
    Offers {
        click: pressed || field,
        set_value: field && !node.states.read_only && secret(node).is_none(),
        focus: field,
    }
}

/// What the field `node` holds, as published: the words of the runs of
/// text inside it, in order — or nothing at all for a field whose value is
/// never published, or a node that is not a field.
pub fn value(node: &SnapshotNode) -> Option<String> {
    if !node.states.takes_text || secret(node).is_some() {
        return None;
    }
    let mut words = Vec::new();
    // Without recursing: a tree is answered at most 256 deep, and a stack
    // of its own costs nothing to bound.
    let mut waiting: Vec<&SnapshotNode> = node.children.iter().rev().collect();
    while let Some(each) = waiting.pop() {
        if each.role == Role::Known(KnownRole::Text)
            && let Some(name) = each.name.as_deref()
        {
            words.push(name);
        }
        waiting.extend(each.children.iter().rev());
    }
    Some(words.join("\n"))
}

/// The node `id` of `tree`, if it holds one.
pub fn find(tree: &Snapshot, id: BoxId) -> Option<&SnapshotNode> {
    let mut waiting: Vec<&SnapshotNode> = tree.root.iter().collect();
    while let Some(node) = waiting.pop() {
        if node.id == id {
            return Some(node);
        }
        // A field is published as a leaf: nothing inside it is a node of
        // the tree the bus reads, so nothing inside it is found.
        if !node.states.takes_text {
            waiting.extend(node.children.iter());
        }
    }
    None
}

/// One part of the window as it is published: where its frame stands in
/// the window, in device pixels, and its tree as its renderer last read it.
#[derive(Debug, Clone, PartialEq)]
pub struct PublishedPart {
    /// Where the part's frame is composed in the window.
    pub at: PixelRect,
    /// Its tree.
    pub tree: Snapshot,
}

/// What a card's page was last read as, for the bus.
#[derive(Debug, Clone, PartialEq)]
pub enum PageRead {
    /// Its tree, as the agent socket's `read` answers it.
    Tree(Box<Snapshot>),
    /// A tree past the socket's bounds, refused as the socket refuses it
    /// (`Refused::TreeTooDeep`): published as its graft alone, said to be
    /// too large to read, never half a tree said to be whole.
    TooLarge,
}

/// A card's page as it is published (ADR 0115 § 3): where its page box is
/// in the window, and what it was last read as.
#[derive(Debug, Clone, PartialEq)]
pub struct PublishedPage {
    /// The card's page box in the window, in device pixels — left, top,
    /// width, height — where a person's press on the page is aimed: its
    /// nodes' rectangles, CSS pixels of the page as it sits, are placed
    /// from its corner at the window's scale.
    pub at: (f64, f64, f64, f64),
    /// What it was read as.
    pub read: PageRead,
}

/// The window's tree as it is to be published.
#[derive(Debug, Clone, PartialEq)]
pub struct Published {
    /// The window's scale: device pixels to a CSS pixel.
    pub scale: f32,
    /// The chats panel, or the rail, once its tree has been read.
    pub panel: Option<PublishedPart>,
    /// The first screen, once its tree has been read.
    pub start: Option<PublishedPart>,
    /// The card's page, once it has been read while its card shows its
    /// picture (ADR 0115 § 3): grafted under that picture.
    pub page: Option<PublishedPage>,
    /// The part whose field has the keyboard, if one does.
    pub keyboard: Option<Part>,
    /// Whether this is the first publishing since the bus asked for the
    /// tree: everything is sent as new.
    pub fresh: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_box::States;
    use alo_layout::Rect;

    fn node(role: Role, name: Option<&str>, children: Vec<SnapshotNode>) -> SnapshotNode {
        SnapshotNode {
            id: BoxId::from_wire(1),
            role,
            name: name.map(str::to_owned),
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

    fn field(role: Role, name: &str, holds: &str) -> SnapshotNode {
        let mut field = node(
            role,
            Some(name),
            vec![node(Role::Known(KnownRole::Text), Some(holds), Vec::new())],
        );
        field.states.takes_text = true;
        field
    }

    #[test]
    fn a_field_is_offered_all_three_and_publishes_what_it_holds() {
        let the_box = field(Role::Known(KnownRole::TextBox), "Ask anything", "lisbon");
        assert_eq!(
            offers(&the_box),
            Offers {
                click: true,
                set_value: true,
                focus: true
            }
        );
        assert_eq!(value(&the_box).as_deref(), Some("lisbon"));
        assert_eq!(secret(&the_box), None);
    }

    #[test]
    fn a_button_and_a_link_are_offered_a_press_and_nothing_else() {
        for role in [KnownRole::Button, KnownRole::Link] {
            let pressed = node(Role::Known(role), Some("New chat"), Vec::new());
            assert_eq!(
                offers(&pressed),
                Offers {
                    click: true,
                    ..Offers::default()
                },
                "{role:?}"
            );
            assert_eq!(value(&pressed), None);
        }
        // A heading, a list, a run of text: nothing.
        for role in [KnownRole::Heading, KnownRole::List, KnownRole::Text] {
            assert_eq!(
                offers(&node(Role::Known(role), Some("Open"), Vec::new())),
                Offers::default()
            );
        }
    }

    #[test]
    fn nothing_disabled_is_offered_anything() {
        let mut go = node(Role::Known(KnownRole::Button), Some("Go"), Vec::new());
        go.states.disabled = true;
        assert_eq!(offers(&go), Offers::default());
        let mut the_box = field(Role::Known(KnownRole::TextBox), "Ask", "a");
        the_box.states.disabled = true;
        assert_eq!(offers(&the_box), Offers::default());
    }

    #[test]
    fn a_password_and_a_cards_details_publish_no_value_and_take_no_text() {
        // A password field has no role (ARIA), and takes text.
        let password = field(Role::Generic, "Password", "hunter2");
        assert_eq!(secret(&password), Some(Secret::Password));
        assert_eq!(value(&password), None);
        assert!(!offers(&password).set_value);
        // Still a field the keyboard can be given, and pressed.
        assert!(offers(&password).focus && offers(&password).click);
        for name in ["Card number", "Security code", "Expiry date", "PIN"] {
            let card = field(Role::Known(KnownRole::TextBox), name, "4242 4242 4242 4242");
            assert_eq!(secret(&card), Some(Secret::Named), "{name}");
            assert_eq!(value(&card), None, "{name}");
            assert!(!offers(&card).set_value, "{name}");
        }
        // A hint says it as well as a name does.
        let mut hinted = field(Role::Known(KnownRole::TextBox), "Code", "123");
        hinted.placeholder = Some("one-time code".to_owned());
        assert_eq!(value(&hinted), None);
        // A read-only field is not typed into either, but is read.
        let mut read_only = field(Role::Known(KnownRole::TextBox), "Address", "a.example");
        read_only.states.read_only = true;
        assert!(!offers(&read_only).set_value);
        assert_eq!(value(&read_only).as_deref(), Some("a.example"));
    }

    #[test]
    fn a_pages_password_known_by_its_dots_publishes_no_value_and_takes_no_text() {
        // Chrome's tree has a password field as a `textbox` holding dots.
        let password = field(
            Role::Known(KnownRole::TextBox),
            "Your secret phrase",
            "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}",
        );
        assert_eq!(secret(&password), Some(Secret::Password));
        assert_eq!(value(&password), None);
        assert!(!offers(&password).set_value);
        assert!(offers(&password).click && offers(&password).focus);
        // Words that hold a dot among others are no password's.
        let words = field(Role::Known(KnownRole::TextBox), "Note", "a \u{2022} b");
        assert_eq!(secret(&words), None);
        assert_eq!(value(&words).as_deref(), Some("a \u{2022} b"));
    }

    #[test]
    fn a_node_is_found_by_its_id_but_never_inside_a_field() {
        let mut inside = field(Role::Known(KnownRole::TextBox), "Ask", "secret words");
        inside.id = BoxId::from_wire(7);
        if let Some(text) = inside.children.first_mut() {
            text.id = BoxId::from_wire(8);
        }
        let mut root = node(Role::Known(KnownRole::Main), None, vec![inside]);
        root.id = BoxId::from_wire(3);
        let tree = Snapshot { root: Some(root) };
        assert_eq!(
            find(&tree, BoxId::from_wire(7)).map(|n| n.id.as_usize()),
            Some(7)
        );
        assert_eq!(
            find(&tree, BoxId::from_wire(3)).map(|n| n.id.as_usize()),
            Some(3)
        );
        assert!(find(&tree, BoxId::from_wire(8)).is_none());
        assert!(find(&tree, BoxId::from_wire(99)).is_none());
    }

    #[test]
    fn each_action_is_said_in_words_and_text_put_only_by_its_length() {
        assert_eq!(Assist::Click.to_string(), "a press");
        assert_eq!(Assist::Focus.to_string(), "the keyboard moved");
        assert_eq!(
            Assist::SetValue("hunter2".to_owned()).to_string(),
            "text put in, 7 characters"
        );
    }
}
