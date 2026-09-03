//! The agent tree, as a message.
//!
//! ADR 0002 is emphatic that the agent tree is a **view** of the layout tree
//! and never a parallel structure, *"because two structures eventually
//! disagree and the agent acts on the one that is wrong"*. That is still true,
//! and it is true **inside the renderer**, which is where both trees are.
//!
//! A borrowed reference cannot cross a process. So what crosses is this: a
//! description of the tree **at one instant**, owned, which is a copy and
//! cannot be anything else.
//!
//! # Why a copy is safe here, and a coordinate would not be
//!
//! Every node carries its [`alo_box::BoxId`], and ADR 0003 allocates those
//! once and never reuses them. A verb sent back naming a node therefore finds
//! **the same node or nothing at all** — never a different node that has moved
//! into the same place. That is the whole reason ADR 0002 refuses coordinates,
//! and it is what makes a snapshot a moment old still safe to act on.

use alo_agent::AgentTree;
use alo_box::{BoxId, Role, States};
use alo_layout::Rect;
use core::fmt;
use core::fmt::Write as _;

/// One thing an agent can read, and everything under it.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotNode {
    /// Which box it is, so that a verb can name it later.
    pub id: BoxId,
    /// What it is.
    pub role: Role,
    /// What it is called, if anything.
    pub name: Option<String>,
    /// What is true of it.
    pub states: States,
    /// Where it is on the page.
    pub rect: Rect,
    /// Whether it is outside the window as the page currently sits.
    pub offscreen: bool,
    /// Whether it has more content than room for it.
    pub scrolls: bool,
    /// What is inside it, in the order a person meets them.
    pub children: Vec<SnapshotNode>,
}

/// A whole interface, read at one instant.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snapshot {
    /// The outermost thing worth reading, if the page has one.
    pub root: Option<SnapshotNode>,
}

impl Snapshot {
    /// Read a tree.
    pub fn of(tree: &AgentTree<'_>) -> Self {
        Self {
            root: tree.root().as_ref().map(describe),
        }
    }

    /// Every node, outermost first, in the order a person meets them.
    pub fn nodes(&self) -> Vec<&SnapshotNode> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            gather(root, &mut out);
        }
        out
    }

    /// Whether there is nothing to read.
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    /// The same lines [`AgentTree::to_outline`] writes.
    ///
    /// Deliberately identical: a snapshot that read differently from the tree
    /// it came from would be the second structure ADR 0002 forbids, and the
    /// cheapest way to notice is for a test to compare the two strings.
    pub fn to_outline(&self) -> String {
        let mut out = String::new();
        if let Some(root) = &self.root {
            write_node(root, 0, &mut out);
        }
        out
    }
}

fn gather<'a>(node: &'a SnapshotNode, out: &mut Vec<&'a SnapshotNode>) {
    out.push(node);
    for child in &node.children {
        gather(child, out);
    }
}

fn describe(node: &alo_agent::AgentNode<'_>) -> SnapshotNode {
    SnapshotNode {
        id: node.id(),
        role: node.role(),
        name: node.name(),
        states: node.states(),
        rect: node.rect(),
        offscreen: node.is_offscreen(),
        scrolls: node.scrolls(),
        children: node.children().iter().map(describe).collect(),
    }
}

fn write_node(node: &SnapshotNode, depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push_str("  ");
    }
    // Writing to a `String` cannot fail.
    let _ = writeln!(
        out,
        "{node} at ({}, {}) {}×{}",
        node.rect.left(),
        node.rect.top(),
        node.rect.size.width,
        node.rect.size.height,
    );
    for child in &node.children {
        write_node(child, depth + 1, out);
    }
}

impl fmt::Display for SnapshotNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.role)?;
        if let Some(name) = &self.name {
            write!(f, " {name:?}")?;
        }
        if !self.states.is_unremarkable() {
            write!(f, " [{}]", self.states)?;
        }
        Ok(())
    }
}
