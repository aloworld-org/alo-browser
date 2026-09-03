//! The box tree: what gets drawn, and what each of them means.
//!
//! A document says what an author wrote; a box tree says what is on screen.
//! They are not the same shape and that is the point — `display: none` removes
//! a whole subtree, `display: contents` removes one box and keeps its children,
//! and a block container with mixed children grows boxes nobody wrote.
//!
//! **Every box carries its [`Semantics`]** — what it is, what is true of it,
//! what it is called — put there when the box is made rather than worked out
//! afterwards. ADR 0002 is explicit that a layout pass keeping only rectangles
//! cannot be retrofitted into an agent tree, and that is why this item sits
//! before layout rather than after it.
//!
//! **There is no geometry here.** Not a single number. Boxes are made, given
//! meaning, and arranged into the tree layout will walk; where they end up is
//! queue item 5, and mixing the two would mean a box's meaning depended on
//! where it landed.

use crate::display::{Display, Inside, Outside};
use crate::semantics::Semantics;
use alo_css::{IssueKind, Location, StyleIssue};
use alo_dom::{Document, NodeId};
use alo_style::{ComputedStyle, StyleTree};
use core::fmt;
use core::fmt::Write as _;

/// The identity of a box within one [`BoxTree`].
///
/// Allocated in creation order and never reused, exactly as `alo_dom::NodeId`
/// is and for the reason ADR 0003 gives: the agent surface names a box and
/// comes back to it, and an identity that could be recycled would make coming
/// back quietly wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoxId(usize);

impl BoxId {
    /// The id as a number, for diagnostics and test assertions.
    pub fn as_usize(self) -> usize {
        self.0
    }

    /// An id from a number, for a test that needs to ask about a box that is
    /// not there.
    ///
    /// There is no other way to make one: ids come from building a tree, which
    /// is what keeps an id from naming a box in a different document. A test
    /// about "what does an unknown id do" needs one anyway, and this is it —
    /// named so that using it anywhere else looks wrong.
    pub fn from_index_for_tests(index: usize) -> Self {
        Self(index)
    }
}

impl fmt::Display for BoxId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "box#{}", self.0)
    }
}

/// What kind of box this is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoxKind {
    /// A box an element asked for.
    Element {
        /// The element it came from.
        node: NodeId,
        /// What its `display` said, already parsed.
        display: Display,
    },
    /// The text of a text node. It sits in a line and has no children.
    Text {
        /// The text node it came from.
        node: NodeId,
        /// What a person would read.
        text: String,
    },
    /// A box nobody wrote.
    ///
    /// A block container whose children are a mix of block-level and
    /// inline-level boxes wraps each run of inline ones in one of these, so
    /// that every container's children are all of one kind. Layout depends on
    /// that; without it, "lay out these children in lines" and "lay out these
    /// children as blocks" would be the same list.
    Anonymous {
        /// How it sits among its siblings — always block-level today, since
        /// that is the only anonymous box this engine makes.
        outside: Outside,
    },
}

impl BoxKind {
    /// The document node this box came from, if it came from one.
    pub fn node(&self) -> Option<NodeId> {
        match self {
            BoxKind::Element { node, .. } | BoxKind::Text { node, .. } => Some(*node),
            BoxKind::Anonymous { .. } => None,
        }
    }

    /// How this box sits among its siblings.
    pub fn outside(&self) -> Outside {
        match self {
            BoxKind::Element { display, .. } => display.outside().unwrap_or(Outside::Inline),
            // Text always sits in a line; that is what makes it text.
            BoxKind::Text { .. } => Outside::Inline,
            BoxKind::Anonymous { outside } => *outside,
        }
    }

    /// How this box arranges its children.
    pub fn inside(&self) -> Inside {
        match self {
            BoxKind::Element { display, .. } => display.inside().unwrap_or(Inside::Flow),
            BoxKind::Text { .. } | BoxKind::Anonymous { .. } => Inside::Flow,
        }
    }

    /// Whether children of this box are laid out in lines rather than by flex
    /// or grid — the question anonymous boxes depend on.
    fn lays_children_out_in_flow(&self) -> bool {
        matches!(self.inside(), Inside::Flow | Inside::FlowRoot)
    }
}

/// One box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxNode {
    /// What kind of box it is.
    pub kind: BoxKind,
    /// What it means: what it is, what is true of it, what it is called.
    pub semantics: Semantics,
    /// Its children, in order.
    pub children: Vec<BoxId>,
    /// The box that holds it, or [`None`] for the root.
    pub parent: Option<BoxId>,
    /// The inline box this block-level box was taken out of, when CSS broke
    /// one around it.
    ///
    /// The block is a *sibling* of the pieces in the box tree, because that is
    /// where layout needs it. It is still **inside** the inline box in the
    /// document, and for a person and for a click — so a reader that shows one
    /// thing per element has to be able to find its way back.
    pub broke_out_of: Option<BoxId>,
    /// Whether more pieces of this box follow it, because it was broken.
    ///
    /// Only the *first* piece carries this. It is here so that "is this box
    /// broken" is one field rather than a search: a reader asks it of every
    /// box it walks, and a search would make reading a page quadratic.
    pub has_more_pieces: bool,
    /// The first fragment of this box, when this one is a continuation of it.
    ///
    /// An inline box holding a block-level box is **broken around it**, into
    /// one box on each side. Both come from the same element, and both are
    /// real boxes: each draws its own background and its own border, which is
    /// exactly what CSS asks for and what makes a split link's underline stop
    /// and start again. This is what says they are two pieces of one thing
    /// rather than two things.
    pub continued_from: Option<BoxId>,
}

impl BoxNode {
    /// The text of this box, if it is a text box.
    pub fn text(&self) -> Option<&str> {
        match &self.kind {
            BoxKind::Text { text, .. } => Some(text),
            _ => None,
        }
    }
}

/// Every box of one document.
#[derive(Debug, Clone)]
pub struct BoxTree {
    boxes: Vec<BoxNode>,
    root: Option<BoxId>,
    issues: Vec<StyleIssue>,
}

impl BoxTree {
    /// The outermost box, or [`None`] if the document generates none at all —
    /// which a document whose root is `display: none` does.
    pub fn root(&self) -> Option<BoxId> {
        self.root
    }

    /// One box.
    pub fn get(&self, id: BoxId) -> Option<&BoxNode> {
        self.boxes.get(id.0)
    }

    /// The children of a box, in order.
    pub fn children(&self, id: BoxId) -> impl Iterator<Item = BoxId> + '_ {
        self.get(id)
            .map(|node| node.children.as_slice())
            .unwrap_or_default()
            .iter()
            .copied()
    }

    /// Every box beneath one, in tree order, not including it.
    pub fn descendants(&self, id: BoxId) -> Vec<BoxId> {
        let mut out = Vec::new();
        let mut stack: Vec<BoxId> = self.children(id).collect();
        stack.reverse();
        while let Some(current) = stack.pop() {
            out.push(current);
            let children: Vec<_> = self.children(current).collect();
            for child in children.into_iter().rev() {
                stack.push(child);
            }
        }
        out
    }

    /// How many boxes there are.
    pub fn len(&self) -> usize {
        self.boxes.len()
    }

    /// Whether the document generated no boxes at all.
    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }

    /// Everything the tree could not build exactly, with the text that caused
    /// it.
    pub fn issues(&self) -> &[StyleIssue] {
        &self.issues
    }

    /// A tree with no boxes at all, for a test that needs one without a
    /// document.
    pub fn empty_for_tests() -> Self {
        Self {
            boxes: Vec::new(),
            root: None,
            issues: Vec::new(),
        }
    }

    /// The tree as indented lines, one box per line: what it is, what it
    /// means, and where it came from.
    ///
    /// This is what a test asserts on, and it is deliberately readable — a
    /// failure that shows two trees side by side says what moved, which an
    /// equality check on a structure does not.
    pub fn to_outline(&self) -> String {
        let mut out = String::new();
        if let Some(root) = self.root {
            // Writing to a `String` cannot fail.
            let _ = self.write_outline(root, 0, &mut out);
        }
        out
    }

    fn write_outline(&self, id: BoxId, depth: usize, out: &mut String) -> fmt::Result {
        let Some(node) = self.get(id) else {
            return Ok(());
        };
        for _ in 0..depth {
            out.push_str("  ");
        }
        match &node.kind {
            BoxKind::Element { display, .. } => {
                write!(out, "{display} · {}", node.semantics)?;
            }
            BoxKind::Text { text, .. } => write!(out, "text {text:?}")?,
            BoxKind::Anonymous { outside } => {
                let outside = match outside {
                    Outside::Block => "block",
                    Outside::Inline => "inline",
                };
                write!(out, "anonymous {outside}")?;
            }
        }
        out.push('\n');
        for child in &node.children {
            self.write_outline(*child, depth + 1, out)?;
        }
        Ok(())
    }

    fn push(&mut self, kind: BoxKind, semantics: Semantics) -> BoxId {
        let id = BoxId(self.boxes.len());
        self.boxes.push(BoxNode {
            kind,
            semantics,
            children: Vec::new(),
            parent: None,
            broke_out_of: None,
            has_more_pieces: false,
            continued_from: None,
        });
        id
    }

    /// Another piece of a box that was broken around a block.
    ///
    /// The same element, the same `display`, the same meaning — a second box,
    /// because CSS says the inline box is split rather than stretched.
    fn continue_box(&mut self, first: BoxId) -> Option<BoxId> {
        let node = self.boxes.get(first.0)?;
        let (kind, semantics) = (node.kind.clone(), node.semantics.clone());
        let id = self.push(kind, semantics);
        if let Some(node) = self.boxes.get_mut(id.0) {
            node.continued_from = Some(first);
        }
        if let Some(node) = self.boxes.get_mut(first.0) {
            node.has_more_pieces = true;
        }
        Some(id)
    }

    /// The inline box a block-level box was taken out of, if it was.
    pub fn broke_out_of(&self, id: BoxId) -> Option<BoxId> {
        self.get(id).and_then(|node| node.broke_out_of)
    }

    /// Whether a box belongs to another box's whole rather than standing on
    /// its own: a later piece of a broken inline, or a block taken out of one.
    ///
    /// A reader walking the tree meets these where layout put them and should
    /// pass over them, because they are read as part of the thing they came
    /// from.
    pub fn belongs_to_another(&self, id: BoxId) -> bool {
        self.is_continuation(id) || self.broke_out_of(id).is_some()
    }

    /// Every box that makes up one thing, in the order a person meets them.
    ///
    /// For almost every box that is the box itself. For an inline box broken
    /// around a block it is every piece **and every block between them** — the
    /// element as the document has it, which is what a reader has to show and
    /// what layout deliberately took apart.
    pub fn whole_of(&self, id: BoxId) -> Vec<BoxId> {
        if !self.is_broken(id) {
            return vec![id];
        }
        let pieces = self.pieces_of(id);
        let Some(first) = pieces.first().copied() else {
            return vec![id];
        };
        let wanted: Vec<BoxId> = pieces
            .into_iter()
            .chain(
                (0..self.boxes.len())
                    .map(BoxId)
                    .filter(|held| self.broke_out_of(*held) == Some(first)),
            )
            .collect();
        // In tree order rather than in the order they were created: a second
        // block is built before the first piece that follows it, so creation
        // order is not document order once there are two of them.
        let Some(root) = self.root() else {
            return wanted;
        };
        core::iter::once(root)
            .chain(self.descendants(root))
            .filter(|held| wanted.contains(held))
            .collect()
    }

    /// Whether a box is a later piece of one that was broken around a block.
    ///
    /// A reader that shows a thing once — the agent tree — asks this, because
    /// a link broken in two is one link.
    pub fn is_continuation(&self, id: BoxId) -> bool {
        self.get(id)
            .is_some_and(|node| node.continued_from.is_some())
    }

    /// Whether a box is one piece of an inline box broken around a block.
    ///
    /// True of every piece, the first included — which is what a reader needs
    /// to know before deciding which one to show.
    pub fn is_broken(&self, id: BoxId) -> bool {
        self.get(id)
            .is_some_and(|node| node.continued_from.is_some() || node.has_more_pieces)
    }

    /// The first piece of a box, which is the box itself unless it is a later
    /// piece of one that was broken.
    pub fn first_piece_of(&self, id: BoxId) -> BoxId {
        self.get(id)
            .and_then(|node| node.continued_from)
            .unwrap_or(id)
    }

    /// Every piece of a box, in order, given any one of them.
    ///
    /// A box that was never broken is one piece: itself. That is what lets a
    /// reader ask this question without first asking whether there is anything
    /// to ask about.
    pub fn pieces_of(&self, id: BoxId) -> Vec<BoxId> {
        let first = self.first_piece_of(id);
        let mut out = vec![first];
        out.extend((0..self.boxes.len()).map(BoxId).filter(|held| {
            self.get(*held)
                .is_some_and(|node| node.continued_from == Some(first))
        }));
        out
    }

    fn adopt(&mut self, parent: BoxId, children: &[BoxId]) {
        for child in children {
            if let Some(node) = self.boxes.get_mut(child.0) {
                node.parent = Some(parent);
            }
        }
        if let Some(node) = self.boxes.get_mut(parent.0) {
            node.children.extend_from_slice(children);
        }
    }
}

/// Build the boxes for a document.
///
/// The style tree decides what exists: `display: none` generates nothing at
/// all, `display: contents` generates no box but keeps its children, and
/// everything else generates one box that then gets its children.
pub fn build(document: &Document, styles: &StyleTree) -> BoxTree {
    let mut tree = BoxTree {
        boxes: Vec::new(),
        root: None,
        issues: Vec::new(),
    };
    let mut roots = build_children(document, styles, document.root(), &mut tree);

    // A document normally makes exactly one box, for `<html>`. If it made
    // several — which only happens when the root is `display: contents` — they
    // need something to hang from, and an anonymous block is what CSS uses.
    tree.root = match roots.len() {
        0 => None,
        1 => roots.pop(),
        _ => {
            let anonymous = tree.push(
                BoxKind::Anonymous {
                    outside: Outside::Block,
                },
                Semantics::anonymous(),
            );
            tree.adopt(anonymous, &roots);
            Some(anonymous)
        }
    };
    tree
}

/// The boxes a node's children generate, in order, already wrapped in
/// anonymous boxes where the parent needs them to be.
fn build_children(
    document: &Document,
    styles: &StyleTree,
    parent: NodeId,
    tree: &mut BoxTree,
) -> Vec<BoxId> {
    let mut generated = Vec::new();
    // A field shows what it holds. An `<input>` has no children in the
    // document — its text is an attribute — so the box for that text is one
    // nobody wrote, exactly as CSS says the inside of a replaced control is.
    if let Some(text) = field_text(document, parent) {
        generated.push(tree.push(BoxKind::Text { node: parent, text }, Semantics::anonymous()));
    }
    for child in document.children(parent) {
        generated.extend(build_one(document, styles, child, tree));
    }
    generated
}

/// The text an `<input>` shows, if it is one and it holds any.
///
/// Only the kinds that show their value as text. A checkbox's `value` is what
/// it submits, not what it says, and drawing it in the box would be a word
/// nobody wrote.
fn field_text(document: &Document, id: NodeId) -> Option<String> {
    let element = document.element(id)?;
    if !element.name.is_html("input") {
        return None;
    }
    let kind = element
        .attr("type")
        .map_or_else(|| "text".to_owned(), str::to_ascii_lowercase);
    if !matches!(
        kind.as_str(),
        "text" | "search" | "email" | "url" | "tel" | "password" | "number"
    ) {
        return None;
    }
    let value = element.attr("value")?;
    if value.is_empty() {
        return None;
    }
    Some(match kind.as_str() {
        // A password shows that it holds something and never what.
        "password" => "•".repeat(value.chars().count()),
        _ => value.to_owned(),
    })
}

/// The boxes one node generates: none, one, or — for `display: contents` —
/// however many its children generate.
fn build_one(
    document: &Document,
    styles: &StyleTree,
    id: NodeId,
    tree: &mut BoxTree,
) -> Vec<BoxId> {
    if let Some(node) = document.get(id)
        && let Some(text) = node.text()
    {
        // Whitespace is kept here and decided about in `arrange`, because
        // whether it is content depends on what is beside it: the space in
        // `<a>All</a> <a>Due</a>` is the gap between two words, and the
        // newline between two `<p>`s is nothing at all. Dropping it here would
        // have rendered "AllDue".
        let semantics = Semantics::anonymous();
        return vec![tree.push(
            BoxKind::Text {
                node: id,
                text: text.to_owned(),
            },
            semantics,
        )];
    }

    let Some(element) = document.element(id) else {
        // A comment, a processing instruction, the doctype: in the document,
        // never on the screen.
        return Vec::new();
    };
    let style = styles.get(id);
    let display = display_of(style, id, tree);

    match display {
        Display::None => Vec::new(),
        Display::Contents => build_children(document, styles, id, tree),
        Display::Box { .. } => {
            let semantics = Semantics::of(document, id, element);
            let box_id = tree.push(BoxKind::Element { node: id, display }, semantics);
            let children = build_children(document, styles, id, tree);
            if display.outside() == Some(Outside::Inline) && holds_a_block(tree, &children) {
                // CSS breaks an inline box around a block-level one. The
                // pieces come back as several boxes rather than one, and the
                // block goes between them — which is what makes it a *sibling*
                // of the anonymous blocks the pieces end up in, one level up.
                return split_around_blocks(tree, box_id, children);
            }
            let arranged = arrange(tree, box_id, children);
            tree.adopt(box_id, &arranged);
            vec![box_id]
        }
    }
}

/// What `display` an element ends up with, recording a value this engine does
/// not implement rather than guessing at it.
fn display_of(style: Option<&ComputedStyle>, id: NodeId, tree: &mut BoxTree) -> Display {
    let Some(value) = style.and_then(|style| style.get("display")) else {
        return Display::INITIAL;
    };
    if let Some(display) = Display::parse(value) {
        return display;
    }
    tree.issues.push(StyleIssue {
        kind: IssueKind::UnsupportedValue,
        source: format!("display: {value} (on {id})"),
        at: Location { line: 0, column: 0 },
    });
    Display::INITIAL
}

/// Put a box's children into a shape layout can walk: all block-level, or all
/// inline-level, never a mix.
///
/// Only a flow container does this. In flex and grid every child is an item in
/// its own right, whatever its `display` says, so wrapping them would invent a
/// row nobody asked for.
fn arrange(tree: &mut BoxTree, parent: BoxId, children: Vec<BoxId>) -> Vec<BoxId> {
    let container_is_flow = tree
        .get(parent)
        .is_some_and(|node| node.kind.lays_children_out_in_flow());
    if !container_is_flow {
        // In flex and grid every child is an item in its own right, so
        // wrapping them would invent a row nobody asked for. The whitespace
        // between two items is not an item, though, and CSS says so.
        return children
            .into_iter()
            .filter(|child| !is_only_whitespace(tree, *child))
            .collect();
    }

    let any_block = children.iter().any(|child| {
        tree.get(*child)
            .is_some_and(|node| node.kind.outside() == Outside::Block)
    });
    if !any_block {
        // Every child sits in a line, so this is one run of text and the
        // spaces in it are the gaps between words.
        return children;
    }

    let mut arranged: Vec<BoxId> = Vec::new();
    let mut run: Vec<BoxId> = Vec::new();
    for child in children {
        let is_block = tree
            .get(child)
            .is_some_and(|node| node.kind.outside() == Outside::Block);
        if is_block {
            flush_run(tree, &mut run, &mut arranged);
            arranged.push(child);
        } else {
            run.push(child);
        }
    }
    flush_run(tree, &mut run, &mut arranged);
    arranged
}

/// Whether any of these boxes is block-level.
fn holds_a_block(tree: &BoxTree, children: &[BoxId]) -> bool {
    children.iter().any(|child| {
        tree.get(*child)
            .is_some_and(|node| node.kind.outside() == Outside::Block)
    })
}

/// Break an inline box around every block-level box inside it.
///
/// CSS: *"the inline box is broken around the block-level box, splitting the
/// inline box into two boxes, one on each side"*. So this hands back a flat
/// run — piece, block, piece, block, piece — and the caller's own `arrange`
/// wraps each run of pieces in an anonymous block. The block becomes a sibling
/// of those anonymous blocks, which is exactly where the specification puts
/// it, and it is why this cannot be done by rearranging children in place.
///
/// A piece with **nothing in it is kept**, which is what CSS asks for — "even
/// if either side is empty" — because an empty inline with a border still
/// draws that border. It costs nothing when the border is not there: a line
/// box holding only empty inline boxes with no border and no padding is
/// zero-height and treated as not existing, and `crate` hands that rule to
/// `alo_layout::inline`, which is where a line is built and is the only place
/// that can tell.
fn split_around_blocks(tree: &mut BoxTree, first: BoxId, children: Vec<BoxId>) -> Vec<BoxId> {
    let mut out: Vec<BoxId> = Vec::new();
    let mut piece = first;
    let mut run: Vec<BoxId> = Vec::new();

    for child in children {
        let is_block = tree
            .get(child)
            .is_some_and(|node| node.kind.outside() == Outside::Block);
        if !is_block {
            run.push(child);
            continue;
        }
        close_piece(tree, piece, &mut run, &mut out);
        if let Some(node) = tree.boxes.get_mut(child.0) {
            node.broke_out_of = Some(first);
        }
        out.push(child);
        // Every later piece is a continuation of the first, whichever piece it
        // follows: they are all the same element.
        let Some(next) = tree.continue_box(first) else {
            return out;
        };
        piece = next;
    }
    close_piece(tree, piece, &mut run, &mut out);
    out
}

/// Finish one piece of a broken inline box, keeping it only if it holds
/// something.
fn close_piece(tree: &mut BoxTree, piece: BoxId, run: &mut Vec<BoxId>, out: &mut Vec<BoxId>) {
    let taken = core::mem::take(run);
    if !taken.is_empty() {
        let arranged = arrange(tree, piece, taken);
        tree.adopt(piece, &arranged);
    }
    out.push(piece);
}

/// Whether a box is a text box holding nothing but whitespace.
fn is_only_whitespace(tree: &BoxTree, id: BoxId) -> bool {
    tree.get(id)
        .and_then(BoxNode::text)
        .is_some_and(|text| text.trim().is_empty())
}

/// Wrap a run of inline-level boxes in one anonymous block box.
///
/// A run that is nothing but whitespace is dropped rather than wrapped: the
/// newline between two `<p>`s separates nothing, and a box for it would be a
/// box layout has to learn to ignore.
fn flush_run(tree: &mut BoxTree, run: &mut Vec<BoxId>, arranged: &mut Vec<BoxId>) {
    if run.is_empty() {
        return;
    }
    if run.iter().all(|child| is_only_whitespace(tree, *child)) {
        run.clear();
        return;
    }
    let anonymous = tree.push(
        BoxKind::Anonymous {
            outside: Outside::Block,
        },
        Semantics::anonymous(),
    );
    let taken = core::mem::take(run);
    tree.adopt(anonymous, &taken);
    arranged.push(anonymous);
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_css::{MediaContext, parse_stylesheet};
    use alo_dom::parse_document;
    use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};

    /// Build the boxes for some markup, with the engine's own sheet and one of
    /// the author's.
    fn boxes(html: &str, css: &str) -> BoxTree {
        let document = parse_document(html);
        let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let author = parse_stylesheet(css);
        let sheets = [
            SourcedSheet::new(Origin::UserAgent, &agent),
            SourcedSheet::new(Origin::Author, &author),
        ];
        let styles = resolve(&document, &sheets, &MediaContext::default());
        build(&document, &styles)
    }

    /// The outline of the boxes under `<body>`, which is what the tests care
    /// about — the wrapper elements are the same every time.
    fn body_outline(html: &str, css: &str) -> String {
        let tree = boxes(html, css);
        let Some(root) = tree.root() else {
            return String::new();
        };
        let body = tree
            .descendants(root)
            .into_iter()
            .find(|id| {
                tree.get(*id).is_some_and(|node| {
                    matches!(&node.kind, BoxKind::Element { .. })
                        && node.semantics.role.to_string() == "generic"
                }) && tree.get(*id).is_some_and(|node| node.parent == Some(root))
            })
            .unwrap_or(root);
        let mut out = String::new();
        let _ = tree.write_outline(body, 0, &mut out);
        out
    }

    #[test]
    fn a_document_makes_one_box_per_element_that_asks_for_one() {
        let tree = boxes("<p>hello</p>", "");
        assert!(!tree.is_empty());
        let outline = tree.to_outline();
        assert!(outline.starts_with("block flow · document\n"), "{outline}");
        assert!(outline.contains("block flow · paragraph"), "{outline}");
        assert!(outline.contains("text \"hello\""), "{outline}");
    }

    #[test]
    fn what_is_never_drawn_makes_no_box() {
        let tree = boxes("<head><title>t</title></head><body><p>p</p></body>", "");
        let outline = tree.to_outline();
        assert!(!outline.contains("\"t\""), "the title's text is not drawn");
        assert_eq!(
            outline.matches("text").count(),
            1,
            "only the paragraph's text is: {outline}",
        );
    }

    #[test]
    fn display_none_removes_the_whole_subtree() {
        let outline = body_outline(
            "<div id=a><p>gone</p></div><p>kept</p>",
            "#a { display: none }",
        );
        assert!(!outline.contains("gone"), "{outline}");
        assert!(outline.contains("kept"), "{outline}");
    }

    #[test]
    fn display_contents_removes_the_box_and_keeps_the_children() {
        let with = body_outline("<div id=a><p>one</p><p>two</p></div>", "");
        let without = body_outline(
            "<div id=a><p>one</p><p>two</p></div>",
            "#a { display: contents }",
        );
        assert_eq!(with.matches("paragraph").count(), 2);
        assert_eq!(without.matches("paragraph").count(), 2);
        assert!(
            without.matches("generic").count() < with.matches("generic").count(),
            "the div's own box is gone:\n{with}\n---\n{without}",
        );
    }

    #[test]
    fn a_mix_of_block_and_inline_children_grows_anonymous_boxes() {
        let outline = body_outline("<div>text <b>bold</b><p>block</p>more</div>", "");
        assert_eq!(
            outline.matches("anonymous block").count(),
            2,
            "one run before the block and one after:\n{outline}",
        );
    }

    #[test]
    fn children_that_are_all_inline_need_no_anonymous_box() {
        let outline = body_outline("<div>text <b>bold</b> more</div>", "");
        assert!(!outline.contains("anonymous"), "{outline}");
    }

    #[test]
    fn children_that_are_all_block_need_no_anonymous_box() {
        let outline = body_outline("<div><p>a</p><p>b</p></div>", "");
        assert!(!outline.contains("anonymous"), "{outline}");
    }

    #[test]
    fn a_flex_container_wraps_nothing_because_every_child_is_an_item() {
        let outline = body_outline(
            "<div id=f>text <b>bold</b><p>block</p></div>",
            "#f { display: flex }",
        );
        assert!(
            !outline.contains("anonymous"),
            "in flex, a mix of children is not a mix of anything:\n{outline}",
        );
        assert!(outline.contains("block flex"), "{outline}");
    }

    #[test]
    fn whitespace_between_blocks_makes_no_box() {
        let outline = body_outline("<div>\n  <p>a</p>\n  <p>b</p>\n</div>", "");
        assert_eq!(
            outline.matches("text").count(),
            2,
            "only the two paragraphs' text, not the newlines between them:\n{outline}",
        );
        assert!(
            !outline.contains("anonymous"),
            "and so the children are all block-level and need no wrapper:\n{outline}",
        );
    }

    #[test]
    fn a_display_this_engine_refuses_falls_back_and_is_recorded() {
        let tree = boxes("<div id=a>t</div>", "#a { display: table }");
        assert_eq!(tree.issues().len(), 1);
        assert_eq!(tree.issues()[0].kind, IssueKind::UnsupportedValue);
        assert!(tree.issues()[0].source.contains("table"));
        assert!(
            tree.to_outline().contains("inline flow · generic"),
            "and the box falls back to the initial value:\n{}",
            tree.to_outline(),
        );
    }

    #[test]
    fn an_inline_holding_a_block_is_broken_around_it() {
        let tree = boxes("<div><span id=s>before<p>block</p>after</span></div>", "");
        let outline = tree.to_outline();
        // Three children of the `<div>`: an anonymous block holding the first
        // piece of the span, the `<p>`, and an anonymous block holding the
        // second piece. The block is a *sibling* of the anonymous blocks,
        // which is what CSS asks for.
        let lines: Vec<&str> = outline.lines().collect();
        let spans = lines
            .iter()
            .filter(|line| line.contains("inline flow"))
            .count();
        assert_eq!(spans, 2, "the span is in two pieces:\n{outline}");
        assert!(outline.contains("block flow · paragraph"), "{outline}");
        assert_eq!(
            tree.issues(),
            &[],
            "nothing is approximated any more: {:?}",
            tree.issues(),
        );
    }

    #[test]
    fn the_pieces_of_a_broken_inline_are_pieces_of_one_thing() {
        let tree = boxes("<div><span id=s>before<p>block</p>after</span></div>", "");
        let pieces: Vec<BoxId> = (0..tree.len())
            .map(BoxId)
            .filter(|id| {
                tree.get(*id)
                    .is_some_and(|node| matches!(node.kind, BoxKind::Element { .. }))
            })
            .filter(|id| tree.is_continuation(*id))
            .collect();
        assert_eq!(pieces.len(), 1, "one of the two pieces is a continuation");

        let first = tree
            .get(*pieces.first().expect("a piece"))
            .and_then(|node| node.continued_from)
            .expect("it says which box it continues");
        assert_eq!(
            tree.pieces_of(first).len(),
            2,
            "and asking any piece finds both",
        );
        assert_eq!(
            tree.pieces_of(first),
            tree.pieces_of(*pieces.first().expect("a piece"))
        );
    }

    #[test]
    fn a_block_at_the_end_of_an_inline_leaves_an_empty_piece_after_it() {
        let tree = boxes("<div><span>text<p>block</p></span></div>", "");
        let outline = tree.to_outline();
        assert_eq!(
            outline
                .lines()
                .filter(|line| line.contains("inline flow"))
                .count(),
            2,
            "CSS keeps the empty piece — an empty inline with a border still \
             draws one, and a line that holds only empty inlines with no \
             border is dropped where it is built:\n{outline}",
        );
        assert_eq!(tree.issues(), &[], "and nothing is approximated");
    }

    #[test]
    fn a_block_at_the_start_of_an_inline_leaves_an_empty_piece_before_it() {
        let tree = boxes("<div><span><p>block</p>text</span></div>", "");
        let outline = tree.to_outline();
        assert_eq!(
            outline
                .lines()
                .filter(|line| line.contains("inline flow"))
                .count(),
            2,
            "one piece on each side, even though the first holds nothing:\n{outline}",
        );
    }

    #[test]
    fn every_box_carries_what_it_means() {
        let tree = boxes("<ul><li id=r aria-selected=true>Row</li></ul>", "");
        let outline = tree.to_outline();
        assert!(outline.contains("block flow · list"), "{outline}");
        assert!(
            outline.contains("listitem [selected=true]"),
            "the row says it is selected, which is ADR 0002's example:\n{outline}",
        );
    }

    #[test]
    fn a_box_knows_its_parent_and_its_children() {
        let tree = boxes("<div><p>a</p></div>", "");
        let root = tree.root().expect("a root");
        assert_eq!(tree.get(root).and_then(|node| node.parent), None);
        for child in tree.descendants(root) {
            assert!(
                tree.get(child).and_then(|node| node.parent).is_some(),
                "{child} has no parent",
            );
        }
    }

    #[test]
    fn ids_are_allocated_in_order_and_never_reused() {
        let tree = boxes("<div><p>a</p><p>b</p></div>", "");
        let mut seen: Vec<usize> = tree
            .descendants(tree.root().expect("a root"))
            .into_iter()
            .map(BoxId::as_usize)
            .collect();
        seen.push(tree.root().expect("a root").as_usize());
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), tree.len(), "no id names two boxes");
        assert_eq!(BoxId(3).to_string(), "box#3");
    }

    #[test]
    fn a_document_whose_root_generates_nothing_makes_no_boxes() {
        let tree = boxes("<p>t</p>", "html { display: none }");
        assert!(tree.root().is_none());
        assert!(tree.is_empty());
        assert_eq!(tree.to_outline(), "");
    }

    #[test]
    fn a_root_that_is_contents_gets_an_anonymous_box_to_hang_from() {
        // Both wrappers have to go: with only `html` set, `body` is still one
        // box and one box needs nothing to hang from.
        let tree = boxes("<p>a</p><p>b</p>", "html, body { display: contents }");
        let root = tree.root().expect("something to hang from");
        assert!(matches!(
            tree.get(root).map(|node| &node.kind),
            Some(BoxKind::Anonymous { .. }),
        ));
        assert!(tree.to_outline().starts_with("anonymous block"));
    }

    #[test]
    fn a_text_box_reports_its_text_and_an_element_box_does_not() {
        let tree = boxes("<p>hello</p>", "");
        let root = tree.root().expect("a root");
        let texts: Vec<&str> = tree
            .descendants(root)
            .into_iter()
            .filter_map(|id| tree.get(id).and_then(BoxNode::text))
            .collect();
        assert_eq!(texts, vec!["hello"]);
        assert!(tree.get(root).and_then(BoxNode::text).is_none());
    }

    #[test]
    fn a_box_id_from_another_tree_resolves_to_nothing() {
        let small = boxes("<p>a</p>", "");
        let large = boxes("<div><p>a</p><p>b</p><p>c</p></div>", "");
        let far = BoxId(large.len() - 1);
        assert!(small.get(far).is_none());
        assert!(small.children(far).next().is_none());
        assert!(small.descendants(far).is_empty());
    }
}
