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
        });
        id
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
    for child in document.children(parent) {
        generated.extend(build_one(document, styles, child, tree));
    }
    generated
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

    // A block-level box inside an inline-level one: CSS splits the inline box
    // in three around it. This engine does not, and treats the inline box as a
    // block container instead — which is what the shape ends up looking like
    // and is not what the specification says. Recorded rather than silent, and
    // queue item 13 is where it gets done properly.
    if tree
        .get(parent)
        .is_some_and(|node| node.kind.outside() == Outside::Inline)
    {
        tree.issues.push(StyleIssue {
            kind: IssueKind::UnsupportedStructure,
            source: format!("{parent} is inline and holds a block-level box"),
            at: Location { line: 0, column: 0 },
        });
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
    fn a_block_inside_an_inline_is_approximated_and_says_so() {
        let tree = boxes("<span id=s>text<p>block</p></span>", "");
        assert!(
            tree.issues()
                .iter()
                .any(|issue| issue.kind == IssueKind::UnsupportedStructure),
            "the approximation is recorded: {:?}",
            tree.issues(),
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
