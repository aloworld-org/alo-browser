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
//!
//! # One element, one thing to read
//!
//! Layout breaks an inline box holding a block-level box into a piece on each
//! side, with the block a *sibling* of the pieces. That is where layout needs
//! them and it is not what the document says: the block is still **inside**
//! the inline box, for a person and for a click. So this reads the pieces and
//! the blocks between them as **one thing** — the box tree records which boxes
//! belong to which whole, and this follows it. Still a view: nothing new is
//! built, and the answer comes from the same trees.

use crate::name::{
    accessible_name, label_names_something, names_itself_from_content, normalise, text_of,
};
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
        for part in self.parts(id) {
            self.collect(part, out);
        }
    }

    /// What is inside a box, as the view has it.
    ///
    /// Almost always its children. For an inline box broken around a block it
    /// is the children of **every piece**, with the blocks between them in
    /// their places — and everywhere else, a box that belongs to somebody
    /// else's whole is passed over, because it is read there instead.
    fn parts(&self, id: BoxId) -> Vec<BoxId> {
        let whole = self.boxes.whole_of(id);
        if whole.len() == 1 {
            return self
                .boxes
                .children(id)
                .filter(|child| !self.boxes.belongs_to_another(*child))
                .collect();
        }
        let pieces = self.boxes.pieces_of(id);
        whole
            .into_iter()
            .flat_map(|member| {
                if pieces.contains(&member) {
                    self.boxes.children(member).collect::<Vec<_>>()
                } else {
                    // A block that was taken out of the inline box: it is read
                    // where the document put it, which is here.
                    vec![member]
                }
            })
            .collect()
    }

    /// Who holds a box, as the view has it.
    ///
    /// A block taken out of an inline box is held by that inline box, and
    /// anything inside a later piece is held by the first piece — because
    /// those are one thing, and the box tree's parent is where layout needed
    /// them rather than where the document has them.
    fn view_parent(&self, id: BoxId) -> Option<BoxId> {
        if let Some(first) = self.boxes.broke_out_of(id) {
            return Some(first);
        }
        let parent = self.boxes.get(id)?.parent?;
        Some(self.boxes.first_piece_of(parent))
    }

    /// The exposed nodes beneath a box, reading through everything that has
    /// nothing to say.
    fn exposed_within(&self, id: BoxId) -> Vec<AgentNode<'a>> {
        let mut out = Vec::new();
        for child in self.parts(id) {
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
        // An inline box broken around a block is several boxes and one thing.
        // The **first** piece is the one read, whichever piece happens to hold
        // something; everything the element contains reaches it through
        // `whole_of`. So an agent asked to activate "the link called Docs"
        // finds one link rather than two with the same name and a refusal
        // between them.
        if self.boxes.is_continuation(id) {
            return false;
        }
        // Text a person would read is worth reading — unless the thing that
        // holds it is already *called* by it. A button reads as
        // `button "Save"`; reporting `text "Save"` inside it as well would say
        // the same thing twice and make an agent choose between two nodes that
        // are the same thing.
        // The dots a password field draws are a rendering of a secret, not
        // something to read. Assistive technology never reads a password back
        // and neither does this — the field itself is in the tree, and can be
        // typed into, which is all an agent needs.
        if self.is_a_masked_value(id) {
            return false;
        }
        if node.text().is_some_and(|text| !text.trim().is_empty()) {
            // A `<label>`'s words have already been read, as the name of the
            // control they name. Reading them again would put the same words
            // on the page twice and give an agent two things answering to
            // "Email" — which its verbs would then have to refuse.
            if self.is_inside_a_label_that_names_something(id) {
                return false;
            }
            return !self.is_named_by_its_content(id);
        }
        match &node.semantics.role {
            // The author said this box means nothing. Read through it.
            Role::Presentational => false,
            // A `<div>` or a `<span>` is worth reading only if it was named —
            // or if something can be done to it. `<input type=password>` is
            // the case that matters: ARIA gives it no role on purpose, and a
            // browser that then left it out of the tree would have an agent
            // that cannot sign in to anything.
            Role::Generic => node.semantics.label.is_some() || node.semantics.states.takes_text,
            Role::Known(_) | Role::Declared(_) => true,
        }
    }

    /// Whether this box is the text a password field draws.
    fn is_a_masked_value(&self, id: BoxId) -> bool {
        let Some(node) = self.boxes.get(id) else {
            return false;
        };
        if node.text().is_none() {
            return false;
        }
        node.kind
            .node()
            .and_then(|source| self.document.element(source))
            .is_some_and(|element| {
                element.name.is_html("input")
                    && element
                        .attr("type")
                        .is_some_and(|kind| kind.eq_ignore_ascii_case("password"))
            })
    }

    /// Whether this box is inside a `<label>` that names a control.
    fn is_inside_a_label_that_names_something(&self, id: BoxId) -> bool {
        let mut current = Some(id);
        while let Some(box_id) = current {
            if let Some(source) = self.boxes.get(box_id).and_then(|node| node.kind.node())
                && label_names_something(self.document, source)
            {
                return true;
            }
            current = self.view_parent(box_id);
        }
        false
    }

    /// Whether the nearest thing above this box takes its name from what is
    /// inside it — which is to say, from this.
    fn is_named_by_its_content(&self, id: BoxId) -> bool {
        let mut current = self.view_parent(id);
        while let Some(ancestor) = current {
            let Some(node) = self.boxes.get(ancestor) else {
                return false;
            };
            if self.is_exposed(ancestor) {
                return names_itself_from_content(&node.semantics.role);
            }
            current = self.view_parent(ancestor);
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
            // Up the *view*, so a block taken out of a hidden link is hidden
            // with it rather than left behind by the break.
            current = self.view_parent(box_id);
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
    ///
    /// Every piece of it: an inline box broken around a block says everything
    /// the document put inside it, in order, however layout took it apart.
    pub fn text(&self) -> String {
        text_of(self.tree.boxes, self.id)
    }

    /// Every rectangle this actually occupies, in the order they were laid out.
    ///
    /// # Why one rectangle is not enough
    ///
    /// A link that wraps across two lines occupies two rectangles: the end of
    /// one line and the start of the next. Their union is a box covering most
    /// of the paragraph, including the text on the line between them that
    /// belongs to somebody else.
    ///
    /// That union is fine as an answer to *roughly where is this* and wrong as
    /// an answer to *is any of this on screen*. The first web page made it
    /// visible: `link "Frequently Asked Questions"` came back as 778 pixels
    /// wide, starting at the left margin, which is not where it is.
    ///
    /// Nothing **acts** on these — ADR 0002 means no verb takes a coordinate —
    /// so this is for deciding what is visible and for a person reading the
    /// tree, which are the two things a wrong rectangle quietly spoils.
    pub fn rects(&self) -> Vec<Rect> {
        let mut found = Vec::new();
        for member in self.tree.boxes.whole_of(self.id) {
            let fragments = self.tree.layout.fragments(member);
            if fragments.is_empty() {
                if let Some(rect) = self.tree.layout.border_box(member) {
                    found.push(rect);
                }
                continue;
            }
            // A box that was laid out into a line has one rectangle per line it
            // reaches. `border_box` is already their union, which is the thing
            // being avoided here.
            found.extend(fragments.iter().map(|fragment| fragment.rect));
        }
        found
    }

    /// Where it is, on the page: the whole of it, pieces and all.
    ///
    /// The union of [`AgentNode::rects`], and useful for *roughly where is
    /// this*. For anything that has to be right about a wrapped inline — what
    /// is on screen, what to draw a highlight around — ask for the rectangles
    /// themselves.
    pub fn rect(&self) -> Rect {
        let mut found: Option<Rect> = None;
        for member in self.tree.boxes.whole_of(self.id) {
            let Some(rect) = self.tree.layout.border_box(member) else {
                continue;
            };
            found = Some(match found {
                None => rect,
                Some(held) => union_rects(held, rect),
            });
        }
        found.unwrap_or(Rect::ZERO)
    }

    /// Whether it is outside the window as the page currently sits.
    ///
    /// ADR 0002 rejects exposing the DOM partly because *"a scrolled-away row
    /// looks identical to a visible one"*. This is the answer that makes it
    /// not: the tree knows where everything is, so it can say what is on
    /// screen.
    pub fn is_offscreen(&self) -> bool {
        let viewport = self.tree.layout.viewport();
        let mut rects = self.rects().into_iter().peekable();
        if rects.peek().is_none() {
            // Nothing was laid out, so there is nothing on screen.
            return true;
        }
        // Offscreen only when **every** piece is, which is the whole of the
        // change: a link whose first line has scrolled away is still on screen
        // if its second line has not, and answering from the union would have
        // called it visible whenever the space *between* its pieces was.
        rects.all(|rect| {
            rect.right() <= 0.0
                || rect.bottom() <= 0.0
                || rect.left() >= viewport.width
                || rect.top() >= viewport.height
        })
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

/// The smallest rectangle both of these fit inside.
fn union_rects(left: Rect, right: Rect) -> Rect {
    let x = left.left().min(right.left());
    let y = left.top().min(right.top());
    Rect::new(
        x,
        y,
        left.right().max(right.right()) - x,
        left.bottom().max(right.bottom()) - y,
    )
}

fn write_node(node: &AgentNode<'_>, depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push_str("  ");
    }
    let rect = node.rect();
    // The union, and then how many pieces it is made of when it is more than
    // one — so the outline says "this box is a union" rather than implying the
    // thing is a rectangle when it is not.
    let pieces = node.rects().len();
    let in_pieces = if pieces > 1 {
        format!(" in {pieces} pieces")
    } else {
        String::new()
    };
    // Writing to a `String` cannot fail.
    let _ = writeln!(
        out,
        "{node} at ({}, {}) {}×{}{in_pieces}",
        rect.left(),
        rect.top(),
        rect.size.width,
        rect.size.height,
    );
    for child in node.children() {
        write_node(&child, depth + 1, out);
    }
}
