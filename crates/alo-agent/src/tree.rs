//! The agent tree: the layout tree, read.
//!
//! **It is a view, not a structure.** ADR 0002 is unambiguous about why:
//! *"if the two could disagree, the agent would eventually act on something
//! that is not on screen."* So nothing is built here. An [`AgentNode`] is a
//! box's id and a borrow of the trees that already exist, and every question
//! it answers is answered from them, when asked.
//!
//! That is also why **reading is never watching**. There is no subscription
//! and no stream: a caller asks, and `alo-os`'s capability model decides who
//! may ask.
//!
//! # What is exposed, and what is read through
//!
//! A box appears in this tree when it has something to say — a role, a name,
//! or text a person would read. A `<div>` with neither is **read through**:
//! its children take its place, exactly as a screen reader does. A page is
//! mostly `<div>`s, and a tree that showed all of them would bury the twelve
//! rows an agent is looking for.
//!
//! `aria-hidden` removes a box and everything under it, because that is the
//! author saying so.

use crate::name::{accessible_name, names_itself_from_content, normalise, text_of};
use alo_box::{BoxId, BoxNode, BoxTree, KnownRole, Role, States};
use alo_dom::Document;
use alo_layout::{LayoutTree, Rect};
use core::fmt;
use core::fmt::Write as _;

/// The interface, as an agent or a screen reader reads it.
///
/// Borrows everything and owns nothing: see the module documentation.
#[derive(Debug, Clone, Copy)]
pub struct AgentTree<'a> {
    document: &'a Document,
    boxes: &'a BoxTree,
    layout: &'a LayoutTree,
}

impl<'a> AgentTree<'a> {
    /// Read a laid-out document.
    pub fn new(document: &'a Document, boxes: &'a BoxTree, layout: &'a LayoutTree) -> Self {
        Self {
            document,
            boxes,
            layout,
        }
    }

    /// The outermost thing worth reading, if the page has one.
    pub fn root(&self) -> Option<AgentNode<'a>> {
        let root = self.boxes.root()?;
        if self.is_exposed(root) {
            return Some(self.node(root));
        }
        // The root is usually the `<html>` element, which is a document and is
        // exposed; if it is not, whatever it reads through to is the root.
        self.exposed_within(root).into_iter().next()
    }

    /// Every node worth reading, in the order a person would meet them.
    pub fn nodes(&self) -> Vec<AgentNode<'a>> {
        let Some(root) = self.boxes.root() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        self.collect(root, &mut out);
        out
    }

    /// The nodes with this role, in document order.
    ///
    /// This is how an agent finds a thing: by what it *is*, never by where it
    /// is. ADR 0002 refuses a coordinate for the same reason.
    pub fn with_role(&self, role: &Role) -> Vec<AgentNode<'a>> {
        self.nodes()
            .into_iter()
            .filter(|node| node.role() == *role)
            .collect()
    }

    /// The nodes called this, ignoring case and surrounding space.
    ///
    /// The name is what a person sees, so matching it the way a person would
    /// read it out is the point — "Save" finds a button labelled `Save `.
    pub fn named(&self, name: &str) -> Vec<AgentNode<'a>> {
        let wanted = normalise(name);
        self.nodes()
            .into_iter()
            .filter(|node| match (&node.name(), &wanted) {
                (Some(held), Some(wanted)) => held.eq_ignore_ascii_case(wanted),
                _ => false,
            })
            .collect()
    }

    /// The tree as indented lines: what each thing is, what it is called, what
    /// is true of it, and where it is.
    ///
    /// This is what a test asserts on and what an agent would be shown. It is
    /// deliberately readable — "listitem \"Invoice 12\" [selected=true]" is the
    /// sentence ADR 0002 opens with.
    pub fn to_outline(&self) -> String {
        let mut out = String::new();
        if let Some(root) = self.root() {
            write_node(&root, 0, &mut out);
        }
        out
    }

    /// Where a link goes, as the author wrote it.
    ///
    /// Not resolved against the page's address: resolving is the network
    /// stack's, which is stage 2, and handing back what was written is what a
    /// record should say anyway.
    pub fn href_of(&self, id: BoxId) -> Option<String> {
        let source = self.boxes.get(id)?.kind.node()?;
        self.document
            .element(source)?
            .attr("href")
            .map(str::to_owned)
    }

    fn node(&self, id: BoxId) -> AgentNode<'a> {
        AgentNode {
            tree: AgentTree {
                document: self.document,
                boxes: self.boxes,
                layout: self.layout,
            },
            id,
        }
    }

    fn collect(&self, id: BoxId, out: &mut Vec<AgentNode<'a>>) {
        if self.is_hidden(id) {
            return;
        }
        if self.is_exposed(id) {
            out.push(self.node(id));
        }
        for child in self.boxes.children(id) {
            self.collect(child, out);
        }
    }

    /// The exposed nodes beneath a box, reading through everything that has
    /// nothing to say.
    fn exposed_within(&self, id: BoxId) -> Vec<AgentNode<'a>> {
        let mut out = Vec::new();
        for child in self.boxes.children(id) {
            if self.is_hidden(child) {
                continue;
            }
            if self.is_exposed(child) {
                out.push(self.node(child));
            } else {
                out.extend(self.exposed_within(child));
            }
        }
        out
    }

    /// Whether a box says anything worth reading.
    fn is_exposed(&self, id: BoxId) -> bool {
        let Some(node) = self.boxes.get(id) else {
            return false;
        };
        if self.is_hidden(id) {
            return false;
        }
        // Text a person would read is worth reading — unless the thing that
        // holds it is already *called* by it. A button reads as
        // `button "Save"`; reporting `text "Save"` inside it as well would say
        // the same thing twice and make an agent choose between two nodes that
        // are the same thing.
        if node.text().is_some_and(|text| !text.trim().is_empty()) {
            return !self.is_named_by_its_content(id);
        }
        match &node.semantics.role {
            // The author said this box means nothing. Read through it.
            Role::Presentational => false,
            // A `<div>` or a `<span>` is worth reading only if it was named.
            Role::Generic => node.semantics.label.is_some(),
            Role::Known(_) | Role::Declared(_) => true,
        }
    }

    /// Whether the nearest thing above this box takes its name from what is
    /// inside it — which is to say, from this.
    fn is_named_by_its_content(&self, id: BoxId) -> bool {
        let mut current = self.boxes.get(id).and_then(|node| node.parent);
        while let Some(ancestor) = current {
            let Some(node) = self.boxes.get(ancestor) else {
                return false;
            };
            if self.is_exposed(ancestor) {
                return names_itself_from_content(&node.semantics.role);
            }
            current = node.parent;
        }
        false
    }

    /// Whether a box is hidden from an agent, it or anything above it.
    ///
    /// `aria-hidden` is the author saying "do not read this", and it applies
    /// to the whole subtree — which is why this walks up rather than checking
    /// one box.
    fn is_hidden(&self, id: BoxId) -> bool {
        let mut current = Some(id);
        while let Some(box_id) = current {
            let Some(node) = self.boxes.get(box_id) else {
                return false;
            };
            if node.semantics.states.hidden {
                return true;
            }
            current = node.parent;
        }
        false
    }
}

/// One thing an agent can read and act on.
#[derive(Debug, Clone, Copy)]
pub struct AgentNode<'a> {
    tree: AgentTree<'a>,
    id: BoxId,
}

impl<'a> AgentNode<'a> {
    /// Which box this is.
    ///
    /// The name an agent comes back with. `alo_box::BoxId` is allocated once
    /// and never reused, so an id that no longer names anything says so rather
    /// than naming something else — ADR 0003, which the agent surface is what
    /// it was written for.
    pub fn id(&self) -> BoxId {
        self.id
    }

    /// What this is.
    ///
    /// A text box is a run of text, which is a thing an agent reads even
    /// though no element has that role — text is not an element.
    pub fn role(&self) -> Role {
        let Some(node) = self.node() else {
            return Role::Generic;
        };
        if node.text().is_some() {
            return Role::Known(KnownRole::Text);
        }
        node.semantics.role.clone()
    }

    /// What is true of it.
    pub fn states(&self) -> States {
        self.node()
            .map_or_else(States::default, |node| node.semantics.states)
    }

    /// What it is called, if anything.
    ///
    /// A run of text is called by what it says, which is the one case where
    /// the name and the text are the same thing.
    pub fn name(&self) -> Option<String> {
        if let Some(text) = self.node().and_then(BoxNode::text) {
            return normalise(text);
        }
        accessible_name(self.tree.document, self.tree.boxes, self.id)
    }

    /// What a person would read inside it.
    pub fn text(&self) -> String {
        text_of(self.tree.boxes, self.id)
    }

    /// Where it is, on the page.
    pub fn rect(&self) -> Rect {
        self.tree.layout.border_box(self.id).unwrap_or(Rect::ZERO)
    }

    /// Whether it is outside the window as the page currently sits.
    ///
    /// ADR 0002 rejects exposing the DOM partly because *"a scrolled-away row
    /// looks identical to a visible one"*. This is the answer that makes it
    /// not: the tree knows where everything is, so it can say what is on
    /// screen.
    pub fn is_offscreen(&self) -> bool {
        let viewport = self.tree.layout.viewport();
        let rect = self.rect();
        rect.right() <= 0.0
            || rect.bottom() <= 0.0
            || rect.left() >= viewport.width
            || rect.top() >= viewport.height
    }

    /// The things inside it worth reading, with everything that says nothing
    /// read through.
    pub fn children(&self) -> Vec<AgentNode<'a>> {
        self.tree.exposed_within(self.id)
    }

    /// Whether this has more content than room for it, and so is a thing that
    /// scrolls.
    ///
    /// Asked of the layout rather than of the style: a box with
    /// `overflow: auto` and nothing spilling out of it does not scroll, and an
    /// agent told otherwise would ask for a scroll that does nothing.
    pub fn scrolls(&self) -> bool {
        self.tree
            .layout
            .get(self.id)
            .is_some_and(alo_layout::BoxGeometry::overflows)
    }

    fn node(&self) -> Option<&'a BoxNode> {
        self.tree.boxes.get(self.id)
    }
}

impl fmt::Display for AgentNode<'_> {
    /// `role "name" [states]`, with the parts that say nothing left out.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.role())?;
        if let Some(name) = self.name() {
            write!(f, " {name:?}")?;
        }
        let states = self.states();
        if !states.is_unremarkable() {
            write!(f, " [{states}]")?;
        }
        Ok(())
    }
}

fn write_node(node: &AgentNode<'_>, depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push_str("  ");
    }
    let rect = node.rect();
    // Writing to a `String` cannot fail.
    let _ = writeln!(
        out,
        "{node} at ({}, {}) {}×{}",
        rect.left(),
        rect.top(),
        rect.size.width,
        rect.size.height,
    );
    for child in node.children() {
        write_node(&child, depth + 1, out);
    }
}
