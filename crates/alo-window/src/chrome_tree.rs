/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Chrome's accessibility tree, read as the tree an agent reads (ADR 0058
//! § 4, queue item 620).
//!
//! Blink builds `Accessibility.getFullAXTree` from its layout objects, so it
//! is close to this browser's own tree and is **not** it. This maps one into
//! a [`Snapshot`] — the same type the engine's renderer sends — by the reading
//! rules `alo-agent`'s tree reads its own boxes by, and names every place
//! where Chrome's says less rather than filling the gap with a guess.
//!
//! # What is read, and what is read through
//!
//! - A node Chrome ignores is read through: its children take its place, as
//!   a `<div>` with nothing to say does in this browser's own tree.
//! - `StaticText` is a [`KnownRole::Text`] node, named by its words with the
//!   space at either end taken off, and its `InlineTextBox`es — the pieces of
//!   its lines — are never read. Text is read through, as in the engine's
//!   tree, when the thing it is in is named by it (a button by its words,
//!   [`alo_agent::name::names_itself_from_content`]) and when it is the
//!   words of a `<label>` that already named the control inside it, unless
//!   it is in a field, where it is what the field holds.
//! - A `ListMarker`'s words are its list item's [`SnapshotNode::marker`],
//!   and the marker is not a node.
//! - `LabelText`, `Legend`, `none`, `presentation` and an unnamed `generic`
//!   are read through. A named `generic`, or one that takes text, is read.
//! - `RootWebArea` is the [`KnownRole::Document`]. ARIA's words are this
//!   browser's roles where it has one; a role of Chrome's own that this
//!   browser has no word for — `InputTime` — is carried as
//!   [`Role::Declared`], spelled as Chrome spells it, never as the nearest
//!   role that is ours.
//!
//! # Where Chrome's tree says less
//!
//! - **Whether a box scrolls**: Chrome's tree does not say, so
//!   [`SnapshotNode::scrolls`] is `false` here for every node — a claim this
//!   tree cannot make true, and the reason it is named.
//! - **A field's hint**: Chrome folds a `placeholder` into the name when
//!   there is no label and does not carry it apart, so
//!   [`SnapshotNode::placeholder`] is never set.
//! - **A broken picture**, `aria-current`, and a `pressed` that is `mixed`:
//!   not read, so never set.
//! - **A node with no DOM node behind it** cannot be named by a verb, and is
//!   read through.
//!
//! # Its ids and its rectangles
//!
//! A node's [`SnapshotNode::id`] is the DOM node it describes, by Chrome's
//! `backendDOMNodeId`, which Chrome allocates once in a document's life — the
//! promise ADR 0003 asks of an id, made by the process that allocates it. Its
//! rectangles are where Chrome drew it (`DOM.getContentQuads`), asked of the
//! caller by `drawn`, in CSS pixels of the window as the page sits.

use crate::chrome::Failed;
use crate::chrome_protocol::AxNode;
use alo_box::{BoxId, Checked, Invalid, KnownRole, Role, States};
use alo_layout::Rect;
use alo_renderer::{Snapshot, SnapshotNode};
use core::fmt;
use std::collections::{HashMap, HashSet};

/// The deepest a tree from Chrome is read. Past it the tree is refused,
/// because a node inside it would be read by a recursion with no bound.
pub const MOST_DEPTH: usize = 512;

/// Why Chrome's tree was not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unread {
    /// Asking Chrome failed.
    Failed(Failed),
    /// No node is outside every other: the tree has no root.
    NoRoot,
    /// A node is named twice — two nodes with one id, or a child under two
    /// parents, or a node inside itself.
    Repeated {
        /// The node, by its id in Chrome's tree.
        node: String,
    },
    /// A child is named that is not in the tree.
    Missing {
        /// The child, by its id in Chrome's tree.
        node: String,
    },
    /// The tree nests deeper than [`MOST_DEPTH`].
    TooDeep {
        /// [`MOST_DEPTH`].
        most: usize,
    },
}

impl fmt::Display for Unread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(why) => why.fmt(f),
            Self::NoRoot => f.write_str("Chrome's tree has no root"),
            Self::Repeated { node } => write!(f, "Chrome's tree names node {node} twice"),
            Self::Missing { node } => {
                write!(f, "Chrome's tree names a node {node} it does not hold")
            }
            Self::TooDeep { most } => write!(f, "Chrome's tree nests deeper than {most}"),
        }
    }
}

impl From<Failed> for Unread {
    fn from(why: Failed) -> Self {
        Self::Failed(why)
    }
}

/// `nodes`, Chrome's whole tree, read as this browser's — with `window` the
/// width and height the page sits in, and `drawn` answering where the DOM
/// node it is given was drawn.
///
/// # Errors
///
/// [`Unread`], for a tree that is not a tree, or a `drawn` that failed.
pub fn snapshot(
    nodes: &[AxNode],
    (width, height): (f64, f64),
    drawn: &mut dyn FnMut(u64) -> Result<Vec<Rect>, Failed>,
) -> Result<Snapshot, Unread> {
    let window = (pixels_of(width), pixels_of(height));
    let mut by_id = HashMap::with_capacity(nodes.len());
    for node in nodes {
        if by_id.insert(node.id.as_str(), node).is_some() {
            return Err(Unread::Repeated {
                node: node.id.clone(),
            });
        }
    }
    let children: HashSet<&str> = nodes
        .iter()
        .flat_map(|node| node.children.iter().map(String::as_str))
        .collect();
    let root = nodes
        .iter()
        .find(|node| !children.contains(node.id.as_str()))
        .ok_or(Unread::NoRoot)?;
    let mut reading = Reading {
        by_id,
        seen: HashSet::new(),
        window,
        drawn,
    };
    let mut read = reading.read(root, Context::default(), 0)?;
    // A root read through is a page with several outermost things; the first
    // is the one a person meets.
    Ok(Snapshot {
        root: if read.is_empty() {
            None
        } else {
            Some(read.swap_remove(0))
        },
    })
}

/// What a node's place in the tree says about the text inside it.
#[derive(Debug, Clone, Copy, Default)]
struct Context {
    /// The nearest thing read above is named by what is inside it.
    named_by_content: bool,
    /// It is inside a `<label>` whose words named the control it holds, and
    /// not inside a field.
    in_a_naming_label: bool,
}

struct Reading<'a, 'd> {
    by_id: HashMap<&'a str, &'a AxNode>,
    seen: HashSet<&'a str>,
    window: (f32, f32),
    drawn: &'d mut dyn FnMut(u64) -> Result<Vec<Rect>, Failed>,
}

impl<'a> Reading<'a, '_> {
    /// What `node` reads as: itself, or what is inside it when it is read
    /// through, or nothing.
    fn read(
        &mut self,
        node: &'a AxNode,
        context: Context,
        depth: usize,
    ) -> Result<Vec<SnapshotNode>, Unread> {
        if depth > MOST_DEPTH {
            return Err(Unread::TooDeep { most: MOST_DEPTH });
        }
        if !self.seen.insert(node.id.as_str()) {
            return Err(Unread::Repeated {
                node: node.id.clone(),
            });
        }
        if node.ignored {
            return self.read_within(node, context, depth);
        }
        match node.role.as_str() {
            // A piece of a line of its `StaticText`, and a marker its list
            // item carries.
            "InlineTextBox" | "ListMarker" => Ok(Vec::new()),
            "StaticText" => {
                let words = trimmed(node.name.as_deref());
                let read_through = context.named_by_content || context.in_a_naming_label;
                match (words, node.node) {
                    (Some(words), Some(dom)) if !read_through => Ok(vec![self.node(
                        dom,
                        Role::Known(KnownRole::Text),
                        Some(words),
                        Vec::new(),
                    )?]),
                    _ => Ok(Vec::new()),
                }
            }
            "LabelText" => {
                let names = self.holds_a_control(node, depth)?;
                let context = Context {
                    in_a_naming_label: context.in_a_naming_label || names,
                    ..context
                };
                self.read_within(node, context, depth)
            }
            role => {
                let role = role_of(role);
                let name = trimmed(node.name.as_deref());
                let states = states_of(node, &role);
                let said = match &role {
                    Role::Presentational => false,
                    Role::Generic => name.is_some() || states.takes_text,
                    Role::Known(_) | Role::Declared(_) => true,
                };
                let Some(dom) = node.node.filter(|_| said) else {
                    return self.read_within(node, context, depth);
                };
                let within = Context {
                    named_by_content: alo_agent::name::names_itself_from_content(&role),
                    in_a_naming_label: context.in_a_naming_label && !states.takes_text,
                };
                let children = self.read_within(node, within, depth)?;
                let mut read = self.node(dom, role, name, children)?;
                read.states = states;
                read.marker = self.marker_of(node);
                Ok(vec![read])
            }
        }
    }

    /// What every child of `node` reads as, in order.
    fn read_within(
        &mut self,
        node: &'a AxNode,
        context: Context,
        depth: usize,
    ) -> Result<Vec<SnapshotNode>, Unread> {
        let mut read = Vec::new();
        for child in &node.children {
            let child = self.child(child)?;
            read.extend(self.read(child, context, depth + 1)?);
        }
        Ok(read)
    }

    fn child(&self, id: &str) -> Result<&'a AxNode, Unread> {
        self.by_id.get(id).copied().ok_or_else(|| Unread::Missing {
            node: id.to_owned(),
        })
    }

    /// A node read as itself, placed where Chrome drew it.
    fn node(
        &mut self,
        dom: u64,
        role: Role,
        name: Option<String>,
        children: Vec<SnapshotNode>,
    ) -> Result<SnapshotNode, Unread> {
        let rects = (self.drawn)(dom)?;
        let (width, height) = self.window;
        let offscreen = rects.iter().all(|rect| {
            rect.right() <= 0.0
                || rect.bottom() <= 0.0
                || rect.left() >= width
                || rect.top() >= height
        });
        let rect = rects.iter().copied().reduce(union).unwrap_or(Rect::ZERO);
        Ok(SnapshotNode {
            // A `usize` holds every id on the machines this is built for; one
            // that did not fit would be a claim no node could answer to.
            id: BoxId::from_wire(usize::try_from(dom).unwrap_or(usize::MAX)),
            role,
            name,
            states: States::default(),
            placeholder: None,
            marker: None,
            rect,
            rects,
            offscreen,
            scrolls: false,
            children,
        })
    }

    /// The words of the `ListMarker` among `node`'s children, if it has one.
    fn marker_of(&self, node: &AxNode) -> Option<String> {
        node.children
            .iter()
            .filter_map(|child| self.by_id.get(child.as_str()))
            .find(|child| child.role == "ListMarker")
            .and_then(|marker| trimmed(marker.name.as_deref()))
    }

    /// Whether a `<label>`'s node holds a control, which its words then name.
    fn holds_a_control(&self, node: &AxNode, depth: usize) -> Result<bool, Unread> {
        if depth > MOST_DEPTH {
            return Err(Unread::TooDeep { most: MOST_DEPTH });
        }
        for child in &node.children {
            let child = self.child(child)?;
            if (!child.ignored && is_a_control(&child.role))
                || self.holds_a_control(child, depth + 1)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Whether Chrome's `role` is a thing a `<label>` names.
fn is_a_control(role: &str) -> bool {
    matches!(
        role,
        "textbox"
            | "searchbox"
            | "checkbox"
            | "radio"
            | "switch"
            | "combobox"
            | "listbox"
            | "slider"
            | "spinbutton"
            | "button"
            | "date"
            | "DateTime"
    ) || role.starts_with("Input")
}

/// This browser's role for Chrome's `role`.
fn role_of(role: &str) -> Role {
    let known = match role {
        "generic" => return Role::Generic,
        "none" | "presentation" | "Legend" | "LineBreak" => return Role::Presentational,
        "RootWebArea" | "document" => KnownRole::Document,
        "banner" => KnownRole::Banner,
        "navigation" => KnownRole::Navigation,
        "main" => KnownRole::Main,
        "complementary" => KnownRole::Complementary,
        "contentinfo" => KnownRole::ContentInfo,
        "region" => KnownRole::Region,
        "form" => KnownRole::Form,
        "search" => KnownRole::Search,
        "article" => KnownRole::Article,
        "heading" => KnownRole::Heading,
        "paragraph" => KnownRole::Paragraph,
        "list" => KnownRole::List,
        "listitem" => KnownRole::ListItem,
        "group" => KnownRole::Group,
        "separator" => KnownRole::Separator,
        "blockquote" => KnownRole::Blockquote,
        "figure" => KnownRole::Figure,
        "image" | "img" => KnownRole::Image,
        "button" => KnownRole::Button,
        "link" => KnownRole::Link,
        "textbox" => KnownRole::TextBox,
        "searchbox" => KnownRole::SearchBox,
        "checkbox" => KnownRole::CheckBox,
        "radio" => KnownRole::Radio,
        "radiogroup" => KnownRole::RadioGroup,
        "switch" => KnownRole::Switch,
        "combobox" => KnownRole::ComboBox,
        "listbox" => KnownRole::ListBox,
        "option" => KnownRole::Option,
        "slider" => KnownRole::Slider,
        "spinbutton" => KnownRole::SpinButton,
        "date" => KnownRole::Date,
        "progressbar" => KnownRole::ProgressBar,
        "meter" => KnownRole::Meter,
        "dialog" => KnownRole::Dialog,
        "status" => KnownRole::Status,
        "grid" => KnownRole::Grid,
        "row" => KnownRole::Row,
        "cell" | "gridcell" => KnownRole::Cell,
        "rowgroup" => KnownRole::RowGroup,
        "columnheader" => KnownRole::ColumnHeader,
        "rowheader" => KnownRole::RowHeader,
        "table" => KnownRole::Table,
        "tab" => KnownRole::Tab,
        "tablist" => KnownRole::TabList,
        "tabpanel" => KnownRole::TabPanel,
        "menu" => KnownRole::Menu,
        "menuitem" => KnownRole::MenuItem,
        "DisclosureTriangle" => KnownRole::Summary,
        "Details" => KnownRole::Details,
        other => return Role::Declared(other.into()),
    };
    Role::Known(known)
}

/// What Chrome says is true of `node`, in this browser's words.
fn states_of(node: &AxNode, role: &Role) -> States {
    let said = |name: &str| {
        node.properties
            .iter()
            .find(|(property, _)| property == name)
            .map(|(_, value)| value.as_str())
    };
    let flag = |name: &str| said(name).map(|value| value == "true");
    States {
        disabled: flag("disabled").unwrap_or(false),
        checked: match said("checked") {
            Some("true") => Some(Checked::Yes),
            Some("false") => Some(Checked::No),
            Some("mixed") => Some(Checked::Mixed),
            _ => None,
        },
        selected: flag("selected"),
        expanded: flag("expanded"),
        pressed: match said("pressed") {
            Some("true") => Some(true),
            Some("false") => Some(false),
            _ => None,
        },
        required: flag("required").unwrap_or(false),
        read_only: flag("readonly").unwrap_or(false),
        busy: flag("busy").unwrap_or(false),
        invalid: match said("invalid") {
            None | Some("false") => None,
            Some(_) => Some(Invalid::Yes),
        },
        hidden: false,
        level: if *role == Role::Known(KnownRole::Heading) {
            said("level").and_then(|level| level.parse().ok())
        } else {
            None
        },
        current: None,
        // A field Chrome says can be edited, as text — never the editable
        // block inside it, which is only where the text is drawn.
        takes_text: said("editable").is_some() && flag("focusable") == Some(true),
        broken: false,
    }
}

/// `words` with the space at either end taken off, or [`None`] when nothing
/// is left.
fn trimmed(words: Option<&str>) -> Option<String> {
    words
        .map(str::trim)
        .filter(|words| !words.is_empty())
        .map(str::to_owned)
}

/// A quad's four corners, `x, y` pairs in CSS pixels, as the smallest
/// rectangle holding them: what a box turned by a transform occupies.
/// [`None`] for corners too far away to be a distance layout can hold.
pub fn rect_of(quad: &[f64; 8]) -> Option<Rect> {
    let (xs, ys): (Vec<f64>, Vec<f64>) = quad
        .chunks_exact(2)
        .filter_map(|corner| Some((*corner.first()?, *corner.get(1)?)))
        .unzip();
    let low = |values: &[f64]| values.iter().copied().fold(f64::INFINITY, f64::min);
    let high = |values: &[f64]| values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (left, top) = (pixels_of(low(&xs)), pixels_of(low(&ys)));
    let (right, bottom) = (pixels_of(high(&xs)), pixels_of(high(&ys)));
    let rect = Rect::new(left, top, right - left, bottom - top);
    [rect.left(), rect.top(), rect.size.width, rect.size.height]
        .iter()
        .all(|n| n.is_finite())
        .then_some(rect)
}

/// A distance in pixels, as layout holds one. One too large for that is
/// infinite, and refused by the caller.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a pixel is an f32 in layout, and one out of its range becomes infinite and is refused"
)]
fn pixels_of(number: f64) -> f32 {
    number as f32
}

/// The smallest rectangle both of these fit inside.
fn union(left: Rect, right: Rect) -> Rect {
    let (x, y) = (left.left().min(right.left()), left.top().min(right.top()));
    Rect::new(
        x,
        y,
        left.right().max(right.right()) - x,
        left.bottom().max(right.bottom()) - y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A node of Chrome's tree, as `ax_nodes` would read it.
    fn ax(id: &str, role: &str, name: &str, children: &[&str]) -> AxNode {
        AxNode {
            id: id.to_owned(),
            ignored: false,
            role: role.to_owned(),
            name: Some(name.to_owned()),
            value: None,
            properties: Vec::new(),
            children: children.iter().map(|child| (*child).to_owned()).collect(),
            node: id.parse().ok(),
        }
    }

    fn with(mut node: AxNode, properties: &[(&str, &str)]) -> AxNode {
        node.properties = properties
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        node
    }

    fn ignored(id: &str, children: &[&str]) -> AxNode {
        AxNode {
            ignored: true,
            role: "none".to_owned(),
            name: None,
            ..ax(id, "none", "", children)
        }
    }

    /// Every node drawn as a 10 by 10 square at its DOM node's number.
    fn read(nodes: &[AxNode]) -> Result<Snapshot, Unread> {
        snapshot(nodes, (600.0, 700.0), &mut |node| {
            let at = f32::from(u16::try_from(node).unwrap_or(u16::MAX));
            Ok(vec![Rect::new(at, at, 10.0, 10.0)])
        })
    }

    fn outline(nodes: &[AxNode]) -> Vec<String> {
        read(nodes)
            .unwrap()
            .nodes()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn a_labels_words_name_its_field_and_are_not_read_again() {
        let nodes = [
            ax("1", "RootWebArea", "", &["2"]),
            ax("2", "paragraph", "", &["3"]),
            ax("3", "LabelText", "", &["4", "6"]),
            ax("4", "StaticText", "Customer name: ", &["5"]),
            AxNode {
                node: None,
                ..ax("5", "InlineTextBox", "Customer name: ", &[])
            },
            with(
                ax("6", "textbox", "Customer name:", &["7"]),
                &[("editable", "plaintext"), ("focusable", "true")],
            ),
            with(ax("7", "generic", "", &["8"]), &[("editable", "plaintext")]),
            // What the field holds is read, inside it.
            ax("8", "StaticText", "Ada", &[]),
        ];
        assert_eq!(
            outline(&nodes),
            [
                "document",
                "paragraph",
                r#"textbox "Customer name:""#,
                r#"text "Ada""#
            ]
        );
        let snapshot = read(&nodes).unwrap();
        let field = snapshot.nodes()[2];
        assert!(field.states.takes_text);
        assert_eq!(field.id, BoxId::from_wire(6));
        // The editable block inside it only draws its text.
        assert!(!snapshot.nodes()[3].states.takes_text);
    }

    #[test]
    fn a_label_naming_nothing_is_read_as_its_words() {
        let nodes = [
            ax("1", "RootWebArea", "", &["2"]),
            ax("2", "LabelText", "", &["3"]),
            ax("3", "StaticText", " Alone ", &[]),
        ];
        assert_eq!(outline(&nodes), ["document", r#"text "Alone""#]);
    }

    #[test]
    fn a_thing_named_by_its_words_does_not_hold_them_again() {
        let nodes = [
            ax("1", "RootWebArea", "", &["2", "4", "6"]),
            ax("2", "button", "Save", &["3"]),
            ax("3", "StaticText", "Save", &[]),
            ax("4", "heading", "Title", &["5"]),
            ax("5", "StaticText", "Title", &[]),
            ax("6", "paragraph", "", &["7"]),
            ax("7", "StaticText", "Words", &[]),
        ];
        assert_eq!(
            outline(&nodes),
            [
                "document",
                r#"button "Save""#,
                r#"heading "Title""#,
                "paragraph",
                r#"text "Words""#
            ]
        );
    }

    #[test]
    fn what_says_nothing_is_read_through() {
        let nodes = [
            ax("1", "RootWebArea", "", &["2", "5", "7", "9"]),
            ignored("2", &["3"]),
            ax("3", "generic", "", &["4"]),
            ax("4", "link", "Docs", &[]),
            ax("5", "Legend", "", &["6"]),
            ax("6", "StaticText", "Size", &[]),
            ax("7", "generic", "Named", &["8"]),
            ax("8", "StaticText", "   ", &[]),
            ax("9", "none", "", &["10"]),
            ax("10", "image", "A cat", &[]),
        ];
        assert_eq!(
            outline(&nodes),
            [
                "document",
                r#"link "Docs""#,
                r#"text "Size""#,
                r#"generic "Named""#,
                r#"image "A cat""#
            ]
        );
    }

    #[test]
    fn a_role_of_chromes_own_is_carried_as_chrome_spells_it() {
        let nodes = [
            ax("1", "RootWebArea", "", &["2"]),
            ax("2", "InputTime", "When:", &[]),
        ];
        let snapshot = read(&nodes).unwrap();
        assert_eq!(snapshot.nodes()[1].role, Role::Declared("InputTime".into()));
        assert_eq!(outline(&nodes), ["document", r#"InputTime "When:""#]);
    }

    #[test]
    fn a_list_items_marker_is_carried_by_the_item() {
        let nodes = [
            ax("1", "RootWebArea", "", &["2"]),
            ax("2", "list", "", &["3"]),
            ax("3", "listitem", "", &["4", "6"]),
            ax("4", "ListMarker", "3. ", &["5"]),
            ax("5", "StaticText", "3. ", &[]),
            ax("6", "StaticText", "Third", &[]),
        ];
        let snapshot = read(&nodes).unwrap();
        assert_eq!(snapshot.nodes()[2].marker.as_deref(), Some("3."));
        assert_eq!(
            outline(&nodes),
            ["document", "list", r#"listitem [marker="3."]"#]
        );
    }

    #[test]
    fn what_chrome_says_is_true_is_carried_in_our_words() {
        let nodes = [
            ax("1", "RootWebArea", "", &["2", "3", "4", "5", "6"]),
            with(
                ax("2", "checkbox", "Mixed", &[]),
                &[("checked", "mixed"), ("disabled", "true")],
            ),
            with(
                ax("3", "textbox", "Mail", &[]),
                &[
                    ("required", "true"),
                    ("readonly", "true"),
                    ("invalid", "true"),
                ],
            ),
            with(ax("4", "heading", "Big", &[]), &[("level", "2")]),
            with(
                ax("5", "button", "Menu", &[]),
                &[("expanded", "false"), ("pressed", "mixed")],
            ),
            // A level means nothing on a paragraph.
            with(
                ax("6", "paragraph", "", &[]),
                &[("level", "2"), ("invalid", "false")],
            ),
        ];
        let snapshot = read(&nodes).unwrap();
        let states: Vec<_> = snapshot.nodes().iter().map(|node| node.states).collect();
        assert_eq!(states[1].checked, Some(Checked::Mixed));
        assert!(states[1].disabled);
        assert!(states[2].required && states[2].read_only);
        assert_eq!(states[2].invalid, Some(Invalid::Yes));
        assert_eq!(states[3].level, Some(2));
        assert_eq!(states[4].expanded, Some(false));
        assert_eq!(states[4].pressed, None);
        assert!(states[5].is_unremarkable());
    }

    #[test]
    fn a_node_is_placed_where_chrome_drew_it() {
        let nodes = [
            ax("1", "RootWebArea", "", &["2", "3", "4"]),
            ax("2", "link", "Wrapped", &[]),
            ax("3", "link", "Below", &[]),
            ax("4", "link", "Nowhere", &[]),
        ];
        let snapshot = snapshot(&nodes, (600.0, 700.0), &mut |node| {
            Ok(match node {
                1 => vec![Rect::new(0.0, 0.0, 600.0, 700.0)],
                2 => vec![
                    Rect::new(500.0, 10.0, 100.0, 20.0),
                    Rect::new(0.0, 30.0, 40.0, 20.0),
                ],
                3 => vec![Rect::new(0.0, 700.0, 40.0, 20.0)],
                _ => Vec::new(),
            })
        })
        .unwrap();
        let [_, wrapped, below, nowhere] = snapshot.nodes()[..] else {
            panic!("four nodes");
        };
        assert_eq!(wrapped.rects.len(), 2);
        assert_eq!(
            (
                wrapped.rect.left(),
                wrapped.rect.top(),
                wrapped.rect.size.width,
                wrapped.rect.size.height
            ),
            (0.0, 10.0, 600.0, 40.0)
        );
        assert!(!wrapped.offscreen);
        assert!(below.offscreen);
        assert!(nowhere.offscreen);
        assert_eq!(nowhere.rect, Rect::ZERO);
    }

    #[test]
    fn a_quad_is_read_as_the_rectangle_round_it() {
        let rect = rect_of(&[
            117.3125, 16.0, 270.3125, 16.0, 270.3125, 37.0, 117.3125, 37.0,
        ])
        .unwrap();
        assert_eq!(
            (rect.left(), rect.top(), rect.size.width, rect.size.height),
            (117.3125, 16.0, 153.0, 21.0)
        );
        // Turned a quarter, by a transform.
        let turned = rect_of(&[10.0, 0.0, 20.0, 10.0, 10.0, 20.0, 0.0, 10.0]).unwrap();
        assert_eq!(
            (
                turned.left(),
                turned.top(),
                turned.size.width,
                turned.size.height
            ),
            (0.0, 0.0, 20.0, 20.0)
        );
        assert_eq!(rect_of(&[0.0, 0.0, 1e300, 0.0, 1e300, 1.0, 0.0, 1.0]), None);
    }

    #[test]
    fn a_tree_that_is_not_one_is_refused_as_a_kind() {
        let looped = [
            ax("1", "RootWebArea", "", &["2"]),
            ax("2", "generic", "", &["3"]),
            ax("3", "generic", "", &["2"]),
        ];
        // Every node is somebody's child, so nothing is outside the loop.
        assert_eq!(read(&looped[1..]), Err(Unread::NoRoot));
        assert_eq!(
            read(&looped),
            Err(Unread::Repeated {
                node: "2".to_owned()
            })
        );
        let shared = [
            ax("1", "RootWebArea", "", &["2", "3"]),
            ax("2", "paragraph", "", &["4"]),
            ax("3", "paragraph", "", &["4"]),
            ax("4", "StaticText", "Twice", &[]),
        ];
        assert_eq!(
            read(&shared),
            Err(Unread::Repeated {
                node: "4".to_owned()
            })
        );
        let twice = [ax("1", "RootWebArea", "", &[]), ax("1", "link", "x", &[])];
        assert_eq!(
            read(&twice),
            Err(Unread::Repeated {
                node: "1".to_owned()
            })
        );
        let missing = [ax("1", "RootWebArea", "", &["9"])];
        assert_eq!(
            read(&missing),
            Err(Unread::Missing {
                node: "9".to_owned()
            })
        );
        assert_eq!(read(&[]), Err(Unread::NoRoot));
        // A label whose control is looked for through a loop.
        let label_loop = [
            ax("1", "RootWebArea", "", &["2"]),
            ax("2", "LabelText", "", &["3"]),
            ax("3", "generic", "", &["2"]),
        ];
        assert_eq!(read(&label_loop), Err(Unread::TooDeep { most: MOST_DEPTH }));
    }

    #[test]
    fn a_tree_too_deep_is_refused_before_it_is_read() {
        let deep: Vec<AxNode> = (1..=MOST_DEPTH + 2)
            .map(|at| {
                let child = (at + 1).to_string();
                let children: &[&str] = if at <= MOST_DEPTH + 1 {
                    &[child.as_str()]
                } else {
                    &[]
                };
                ax(&at.to_string(), "group", "", children)
            })
            .collect();
        assert_eq!(read(&deep), Err(Unread::TooDeep { most: MOST_DEPTH }));
        assert!(read(&deep[deep.len() - MOST_DEPTH..]).is_ok());
    }

    #[test]
    fn asking_where_a_node_was_drawn_can_fail_the_reading() {
        let nodes = [ax("1", "RootWebArea", "", &[])];
        assert_eq!(
            snapshot(&nodes, (1.0, 1.0), &mut |_| Err(Failed::Gone)),
            Err(Unread::Failed(Failed::Gone))
        );
    }
}
