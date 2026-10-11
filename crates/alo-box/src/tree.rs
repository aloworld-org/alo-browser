/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

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
use crate::natural::NaturalSize;
use crate::picture::{Img, Shown};
use crate::semantics::Semantics;
use crate::transform::{TextTransform, language_of};
use crate::whitespace::WhiteSpace;
use alo_css::{IssueKind, Location, PseudoElement, StyleIssue};
use alo_dom::{Document, NodeId};
use alo_style::{ComputedStyle, StyleTree};
use core::fmt;
use core::fmt::Write as _;
use std::collections::{BTreeMap, BTreeSet};

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

    /// An id read off the wire from another process.
    ///
    /// The only other way to make one, and it exists because a snapshot that
    /// crossed a process boundary has to arrive with its ids intact or an agent
    /// cannot act on what it just read.
    ///
    /// **An id in a message is a claim, not a fact.** It came from the renderer,
    /// which is the process that parsed a hostile page, and it is meaningful
    /// only against the snapshot it arrived with — never against a different
    /// document, and never as an index into anything the browser process holds.
    /// ADR 0003 says an id is allocated once and never reused; that is a promise
    /// the *allocating* process makes, and a process on the other side of a pipe
    /// is not obliged to keep it.
    pub fn from_wire(index: usize) -> Self {
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
        /// How it sits among its siblings.
        outside: Outside,
        /// Why it exists, which decides how it is laid out.
        purpose: Purpose,
    },
}

/// Why an anonymous box exists.
///
/// Two boxes nobody wrote look the same in a tree and are laid out completely
/// differently, so the tree says which is which rather than leaving layout to
/// guess from its surroundings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Purpose {
    /// A run of inline-level boxes, wrapped so that a container's children are
    /// all of one kind.
    #[default]
    Run,
    /// The inside of a form control.
    ///
    /// Browsers give a `<button>` and an `<input>` an internal box that holds
    /// what they display, and it is why a tall button's label sits in the
    /// middle of it and why an empty field is still one line tall. It is **not**
    /// expressible in the user-agent style sheet: a rule that centred a
    /// button's label would also centre the children of a button an author had
    /// made a flex container, and an author cannot override a rule they cannot
    /// see.
    Control {
        /// Whether what is inside sits in the middle, which a button does and
        /// a text field does not.
        centred: bool,
        /// Whether it is a one-line field — an `<input>` a person types into.
        ///
        /// Such a field stands on the baseline of the line it holds **even
        /// when nothing is typed in it**, so that it does not move when
        /// something is. A button and a `<textarea>` are not, and with
        /// nothing in them they stand on their bottom margin edge as any
        /// box with no line does.
        one_line: bool,
        /// Whether it is at least one line tall. Every control is **even
        /// when it holds nothing**, except a `<button>` element, which is
        /// only when it holds something a line is made of.
        ///
        /// An empty field, an empty `<textarea>` and an `<input
        /// type=submit value="">` are each a line tall in Chrome 154, but
        /// a `<button>` is exactly what it holds: nothing at all when
        /// empty, and its block's height when that is all it holds. alo's
        /// Agenda draws its month arrows as a `<button>` holding a 15 pixel
        /// `<svg>`, and a line's minimum made each one 4.5 taller than a
        /// browser draws it (queue item 437).
        at_least_a_line: bool,
    },
    /// The content slot of a `<details>`: every child but its summary, in
    /// one block (ADR 0048 § 1).
    ///
    /// A block container laid out as any block is, holding blocks or lines.
    /// While its `details` is closed it holds nothing at all and so takes no
    /// room — but it is still there, so a flex or grid `details` keeps the
    /// gap before it (§ 2).
    DetailsContent,
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
            BoxKind::Anonymous { outside, .. } => *outside,
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

    /// Whether this is an **inline box**: inline-level, and holding what it
    /// holds in its parent's lines rather than in a formatting context of
    /// its own — a `<span>`, an `<a>`.
    ///
    /// Only an inline box is broken around a block in it and keeps what is
    /// beside one in its lines. An `inline-block`, `inline-flex` or
    /// `inline-grid` is inline-level too, but it is one atomic box on the
    /// line, and inside it a block is a block like any other (CSS Display 3
    /// § 2.4; queue item 286).
    fn is_inline_box(&self) -> bool {
        self.outside() == Outside::Inline && self.inside() == Inside::Flow
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
    /// The boxes that have a size of their own, and what it is.
    ///
    /// A **replaced** box — an image, and later a video or a plugin — is sized
    /// by its content rather than by its style: an `<img>` with no width lays
    /// out at the picture's own width, and one with a width keeps the picture's
    /// ratio. Nothing in CSS says what that size is, so somebody has to put it
    /// here.
    ///
    /// A side map rather than a field on every box, because almost no box has
    /// one. An `<img>`'s is what [`Shown`] says, told to the tree **before**
    /// it is built by whoever decoded the content (ADR 0046 § 1), and an
    /// outermost `<svg>`'s is read from its own attributes. This crate knows
    /// nothing about pictures and should not start: it is told which `<img>`
    /// shows one and how big, and never decides it.
    ///
    /// A [`NaturalSize`] rather than a width and a height, because an `<svg>`
    /// with only a `viewBox` has a shape and no size, and one with nothing
    /// written has neither — and both are still replaced boxes.
    natural: BTreeMap<BoxId, NaturalSize>,
    /// Which box a `<fieldset>` shows in its block-start band, by fieldset.
    ///
    /// A fieldset's first `<legend>` is not laid out where it was written: it
    /// sits **in** the fieldset's block-start border rather than under it, and
    /// the border is not drawn behind it. Layout cannot work that out on its
    /// own — it has the boxes and their styles and no document — so the tree
    /// that does have the document says which box it is.
    ///
    /// A side map for the same reason [`BoxTree::natural`] is one: almost no
    /// box is a fieldset. It is a relation between two boxes of this tree
    /// rather than a second structure, so there is nothing here that can
    /// disagree with the tree.
    legends: BTreeMap<BoxId, BoxId>,
    /// The boxes that end the line they are on: a `<br>`'s.
    ///
    /// A side set for the same reason [`BoxTree::legends`] is a side map:
    /// almost no box is one, and what makes one is the element it came from,
    /// which layout cannot see. See [`crate::line_break`].
    breaks: BTreeSet<BoxId>,
    /// The boxes that are a field's `::placeholder`: the hint it shows while
    /// it holds nothing (ADR 0043 § 3).
    ///
    /// A text box like the one a value is drawn in, and a different thing:
    /// it is set in the pseudo-element's style rather than the field's, and
    /// an agent never reads it as text. A side set for [`BoxTree::breaks`]'
    /// reason.
    placeholders: BTreeSet<BoxId>,
    /// The boxes that are a list item's `::marker`: the bullet or number
    /// beside it (ADR 0044 § 4).
    ///
    /// A text box, first among its item's children, that takes no room in
    /// the item: layout leaves it out of the item's flow and stands it
    /// outside the item's first line afterwards. Set in the pseudo-element's
    /// style, and read by an agent as the item's `marker` rather than as
    /// text. A side set for [`BoxTree::breaks`]' reason.
    markers: BTreeSet<BoxId>,
    /// The boxes `position: absolute` takes out of flow (queue item 354).
    ///
    /// Each is block-level, because CSS blockifies it, and none of them is a
    /// block among blocks: it takes no room where it stands. So it does not
    /// break the inline box it was written in, and it stays among the inline
    /// content beside it, where layout gives it no room on the line and
    /// takes its static position from there. A side set for
    /// [`BoxTree::breaks`]' reason.
    out_of_flow: BTreeSet<BoxId>,
    /// The boxes that stand on their bottom border edge rather than their
    /// bottom margin edge when they have no line in them: a checkbox's and
    /// a radio's (queue item 443). See [`crate::toggle`]. A side set for
    /// [`BoxTree::breaks`]' reason.
    border_baselines: BTreeSet<BoxId>,
}

impl BoxTree {
    /// The outermost box, or [`None`] if the document generates none at all —
    /// which a document whose root is `display: none` does.
    pub fn root(&self) -> Option<BoxId> {
        self.root
    }

    /// Every box in the tree, in the order they were built.
    ///
    /// For asking a question *of a kind of box* rather than following the
    /// tree's shape — which is a different thing and one a walk answers badly,
    /// because a broken inline's pieces are siblings here rather than nested.
    pub fn ids(&self) -> impl Iterator<Item = BoxId> + '_ {
        (0..self.boxes.len()).map(BoxId)
    }

    /// What a replaced box's content says about its size, or [`None`] for a
    /// box that is not replaced.
    pub fn natural_size(&self, id: BoxId) -> Option<NaturalSize> {
        self.natural.get(&id).copied()
    }

    /// The legend a `<fieldset>` shows in its block-start band, if it has one.
    ///
    /// The box is a child of the fieldset like any other and is **first**
    /// among them, wherever the `<legend>` was written: a fieldset renders its
    /// first legend at the top whatever else comes before it in the document.
    /// What makes it the legend rather than an ordinary first child is this
    /// answer, and layout asks it to know where to put the border.
    pub fn rendered_legend(&self, id: BoxId) -> Option<BoxId> {
        self.legends.get(&id).copied()
    }

    /// Every fieldset that shows a legend, with the legend it shows.
    pub fn legends(&self) -> impl Iterator<Item = (BoxId, BoxId)> + '_ {
        self.legends
            .iter()
            .map(|(fieldset, legend)| (*fieldset, *legend))
    }

    /// Whether a box ends the line it is on, as a `<br>`'s does.
    ///
    /// Layout asks it of every box in a line: what comes after one starts a
    /// new line whether or not the line was full.
    pub fn is_forced_break(&self, id: BoxId) -> bool {
        self.breaks.contains(&id)
    }

    /// Whether a box is a field's `::placeholder` — a hint, not a value.
    ///
    /// Layout asks it so that the hint adds nothing to its field's size, and
    /// the agent tree so that it never reads the hint as the field's text
    /// (ADR 0043 §§ 3 and 5).
    pub fn is_placeholder(&self, id: BoxId) -> bool {
        self.placeholders.contains(&id)
    }

    /// Whether a box is a list item's `::marker`, which stands outside the
    /// item's first line and takes no room in it.
    ///
    /// Layout asks it so that the marker joins no line and is placed beside
    /// one, and the agent tree so that it reads the marker as the item's
    /// property rather than as text (ADR 0044 §§ 4–5).
    pub fn is_marker(&self, id: BoxId) -> bool {
        self.markers.contains(&id)
    }

    /// Whether `position: absolute` takes a box out of flow.
    ///
    /// Layout asks it of every box in a line: such a box takes no room on
    /// the line, and stands where the line would have put it when its insets
    /// leave it there (CSS 2 § 10.3.7; queue item 354).
    pub fn is_out_of_flow(&self, id: BoxId) -> bool {
        self.out_of_flow.contains(&id)
    }

    /// Whether a box with no line in it stands on its bottom border edge
    /// rather than its bottom margin edge, as a checkbox and a radio do.
    ///
    /// Layout asks it of an atomic box to know where its baseline is
    /// (queue item 443).
    pub fn stands_on_border_edge(&self, id: BoxId) -> bool {
        self.border_baselines.contains(&id)
    }

    /// The marker box of the list item `id`, if it has one.
    pub fn marker_of(&self, id: BoxId) -> Option<BoxId> {
        self.get(id)?
            .children
            .first()
            .copied()
            .filter(|child| self.is_marker(*child))
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

    /// The style a box's **text** is set with.
    ///
    /// A text box came from a text node, and a text node has no style of its
    /// own: text inherits everything from the element that holds it, so the
    /// element is what to ask. An anonymous box has no node at all and the same
    /// answer applies, which is why this walks upwards rather than reading one
    /// box.
    ///
    /// It lives here because it is a question about the shape of *this* tree
    /// and about nothing else, and because three separate places were each
    /// walking it — layout, to measure a line; paint, to draw one; and the
    /// count of families a page asked for. Three copies of a rule is three
    /// chances for one of them to stop agreeing with the others about which
    /// font a line is in, which is a rendering difference nobody could explain.
    ///
    /// A field's `::placeholder` box is the one box that has a style of its
    /// own to answer with: the pseudo-element's, computed as its field's
    /// child (ADR 0043 § 3). So layout and paint read the hint's colour and
    /// font here, as they read any text's, and neither learns what a
    /// pseudo-element is.
    ///
    /// [`None`] only for a box with no styled element above it at all, which is
    /// a box outside any document — a tree built for a test.
    pub fn nearest_style<'s>(&self, styles: &'s StyleTree, id: BoxId) -> Option<&'s ComputedStyle> {
        let pseudo = if self.is_placeholder(id) {
            Some(PseudoElement::Placeholder)
        } else if self.is_marker(id) {
            Some(PseudoElement::Marker)
        } else {
            None
        };
        if let Some(pseudo) = pseudo
            && let Some(style) = self
                .get(id)
                .and_then(|node| node.kind.node())
                .and_then(|element| styles.pseudo(element, pseudo))
        {
            return Some(style);
        }
        let mut current = Some(id);
        while let Some(box_id) = current {
            if let Some(style) = self
                .get(box_id)
                .and_then(|node| node.kind.node())
                .and_then(|source| styles.get(source))
            {
                return Some(style);
            }
            current = self.get(box_id).and_then(|node| node.parent);
        }
        None
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
            natural: BTreeMap::new(),
            legends: BTreeMap::new(),
            breaks: BTreeSet::new(),
            placeholders: BTreeSet::new(),
            markers: BTreeSet::new(),
            out_of_flow: BTreeSet::new(),
            border_baselines: BTreeSet::new(),
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
                if self.is_forced_break(id) {
                    out.push_str(" · line break");
                }
            }
            BoxKind::Text { text, .. } => {
                write!(out, "text {text:?}")?;
                if self.is_placeholder(id) {
                    out.push_str(" · ::placeholder");
                }
                if self.is_marker(id) {
                    out.push_str(" · ::marker");
                }
            }
            BoxKind::Anonymous { outside, .. } => {
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

/// Build the boxes for a document that shows no pictures.
///
/// Every `<img>` is then broken: its `alt` as text, or a box of nothing
/// (ADR 0046). A drawing that read pictures builds with [`build_showing`].
pub fn build(document: &Document, styles: &StyleTree) -> BoxTree {
    build_showing(document, styles, &Shown::default())
}

/// Build the boxes for a document, told which `<img>` elements show a
/// picture and how big each one is.
///
/// The style tree decides what exists: `display: none` generates nothing at
/// all, `display: contents` generates no box but keeps its children, and
/// everything else generates one box that then gets its children. `shown`
/// decides what an `<img>` box is (ADR 0046 § 1): a picture, its `alt` as
/// text, or a box of nothing.
pub fn build_showing(document: &Document, styles: &StyleTree, shown: &Shown) -> BoxTree {
    let mut tree = BoxTree {
        natural: BTreeMap::new(),
        legends: BTreeMap::new(),
        breaks: BTreeSet::new(),
        placeholders: BTreeSet::new(),
        markers: BTreeSet::new(),
        out_of_flow: BTreeSet::new(),
        border_baselines: BTreeSet::new(),
        boxes: Vec::new(),
        root: None,
        issues: Vec::new(),
    };
    let mut roots = build_children(document, styles, shown, document.root(), &mut tree);

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
                    purpose: Purpose::Run,
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
    shown: &Shown,
    parent: NodeId,
    tree: &mut BoxTree,
) -> Vec<BoxId> {
    let mut generated = Vec::new();
    // A field shows what it holds. An `<input>` has no children in the
    // document — its text is an attribute — so the box for that text is one
    // nobody wrote, exactly as CSS says the inside of a replaced control is.
    if let Some(text) = field_text(document, parent) {
        // Drawn in the field's case; read by an agent as the field's value,
        // which is the field's and not the drawing's (ADR 0049 § 4).
        let text = transformed(document, styles, styles.get(parent), parent, text, tree);
        generated.push(tree.push(BoxKind::Text { node: parent, text }, Semantics::anonymous()));
    }
    // A field holding nothing shows its hint instead, where its text would
    // be: a run in the same line, set in its `::placeholder` style (ADR 0043
    // §§ 2–3). Never both — a field that shows a hint holds no value.
    if let Some(text) = alo_dom::placeholder::shown(document, parent) {
        // In the hint's own style's case, which inherits the field's.
        let style = styles
            .pseudo(parent, PseudoElement::Placeholder)
            .or_else(|| styles.get(parent));
        let text = transformed(document, styles, style, parent, text, tree);
        let hint = tree.push(BoxKind::Text { node: parent, text }, Semantics::anonymous());
        tree.placeholders.insert(hint);
        generated.push(hint);
    }
    for child in document.children(parent) {
        generated.extend(build_one(document, styles, shown, child, tree));
    }
    generated
}

/// What an empty date field draws: how a date is written in it (ADR 0042).
///
/// The order is the person's region once they choose one (item 387); until
/// then it is ISO 8601's, which is the field's own value form, so what the
/// field draws, submits and reads back are one form (§ 2). The words are
/// English, the language the browser itself is written in, until that is the
/// person's to choose as well.
const DATE_FORMAT: &str = "yyyy-mm-dd";

/// The text an `<input>` shows, if it is one and it holds any.
///
/// Only the kinds that show their value as text. A checkbox's `value` is what
/// it submits, not what it says, and drawing it in the box would be a word
/// nobody wrote.
///
/// A date field always shows something: the date it holds, or — empty, or
/// holding a `value` HTML sanitises to empty — [`DATE_FORMAT`], as its text
/// in its own colour, on the line a value would stand on (ADR 0042 § 3).
fn field_text(document: &Document, id: NodeId) -> Option<String> {
    let element = document.element(id)?;
    if !element.name.is_html("input") {
        return None;
    }
    if alo_dom::date::is_date_field(element) {
        return Some(
            alo_dom::date::held_date(element)
                .unwrap_or(DATE_FORMAT)
                .to_owned(),
        );
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
    shown: &Shown,
    id: NodeId,
    tree: &mut BoxTree,
) -> Vec<BoxId> {
    if let Some(node) = document.get(id)
        && let Some(text) = node.text()
    {
        // Whitespace is **collapsed** here and *decided about* in `arrange`,
        // because they are different questions. Collapsing is a fact about the
        // text: `one   two` is one space wherever it appears. Whether what is
        // left counts as content depends on what is beside it — the space in
        // `<a>All</a> <a>Due</a>` is the gap between two words, and the newline
        // between two `<p>`s is nothing at all. Dropping it here would have
        // rendered "AllDue".
        //
        // Then the text is put in its case, after collapsing and before
        // anything measures it, so that layout, paint and the agent read one
        // text, the one drawn (ADR 0049 § 1).
        let collapsed = white_space_of(document, styles, id).apply(text);
        let holder = document.parent(id);
        let text = match holder {
            Some(holder) => transformed(
                document,
                styles,
                styles.get(holder),
                holder,
                collapsed,
                tree,
            ),
            None => collapsed,
        };
        let semantics = Semantics::anonymous();
        return vec![tree.push(BoxKind::Text { node: id, text }, semantics)];
    }

    let Some(element) = document.element(id) else {
        // A comment, a processing instruction, the doctype: in the document,
        // never on the screen.
        return Vec::new();
    };
    let style = styles.get(id);
    let display = display_of(style, id, tree);
    let made = build_element(document, styles, shown, id, element, display, tree);
    if is_absolutely_positioned(style) && matches!(display, Display::Box { .. }) {
        tree.out_of_flow.extend(made.iter().copied());
    }
    made
}

/// The boxes an element generates, its `display` decided.
fn build_element(
    document: &Document,
    styles: &StyleTree,
    shown: &Shown,
    id: NodeId,
    element: &alo_dom::Element,
    display: Display,
    tree: &mut BoxTree,
) -> Vec<BoxId> {
    let img = Img::of(element, id, shown);
    if let Some(made) = &img
        && matches!(display, Display::Box { .. })
    {
        return img_box(document, styles, id, element, display, made.clone(), tree);
    }
    match display {
        Display::None => Vec::new(),
        // A replaced element has no children to promote, so CSS makes
        // `contents` on one mean `none` — and an `<svg>`'s children are
        // shapes, which must not leak into the page as boxes. CSS Display
        // names `<img>` among them whether or not its picture is shown.
        Display::Contents if img.is_some() || crate::svg::is_outermost(document, id, element) => {
            Vec::new()
        }
        Display::Contents => build_children(document, styles, shown, id, tree),
        Display::Box { .. } if crate::svg::is_outermost(document, id, element) => {
            vec![svg_box(document, id, element, display, tree)]
        }
        Display::Box { .. } => {
            let semantics = Semantics::of(document, id, element);
            let box_id = tree.push(BoxKind::Element { node: id, display }, semantics);
            if crate::line_break::is_forced_break(element, display) {
                tree.breaks.insert(box_id);
            }
            if crate::toggle::stands_on_its_border_edge(element) {
                tree.border_baselines.insert(box_id);
            }
            let children = match crate::details::slots(document, id) {
                Some(slots) => details_children(document, styles, shown, id, &slots, tree),
                None => build_children(document, styles, shown, id, tree),
            };
            // A form control holds what it displays in a box nobody wrote —
            // only while the author has left it a flow container, because
            // making it a flex or grid container replaces that arrangement
            // with one of their own.
            if matches!(display.inside(), Some(Inside::Flow | Inside::FlowRoot))
                && let Some(purpose) = control_content(element)
            {
                let purpose = with_its_line(tree, purpose, &children);
                let inner = tree.push(
                    BoxKind::Anonymous {
                        outside: Outside::Block,
                        purpose,
                    },
                    Semantics::anonymous(),
                );
                tree.adopt(inner, &children);
                tree.adopt(box_id, &[inner]);
                return vec![box_id];
            }
            let inline_box = tree
                .get(box_id)
                .is_some_and(|node| node.kind.is_inline_box());
            if inline_box && holds_a_block(tree, &children) {
                // CSS breaks an inline box around a block-level one. The
                // pieces come back as several boxes rather than one, and the
                // block goes between them — which is what makes it a *sibling*
                // of the anonymous blocks the pieces end up in, one level up.
                return split_around_blocks(tree, box_id, children);
            }
            let mut arranged = arrange(tree, box_id, children);
            // A fieldset shows its legend in its block-start border rather
            // than under it, so the legend goes first among the children
            // whatever came before it in the document.
            if let Some(legend) = legend_of(document, element, display, tree, &arranged) {
                arranged.retain(|child| *child != legend);
                arranged.insert(0, legend);
                tree.legends.insert(box_id, legend);
            }
            // A list item's marker, first, after the children are arranged:
            // it joins none of their lines and no anonymous block, because
            // it stands outside the item's first line (ADR 0044 § 4).
            if let Some(text) = styles.marker_text(id) {
                // In the marker's own case, which the user-agent sheet makes
                // `none` whatever the item's (ADR 0049 § 6).
                let text = transformed(
                    document,
                    styles,
                    styles.pseudo(id, PseudoElement::Marker),
                    id,
                    text.to_owned(),
                    tree,
                );
                let marker = tree.push(BoxKind::Text { node: id, text }, Semantics::anonymous());
                tree.markers.insert(marker);
                arranged.insert(0, marker);
            }
            tree.adopt(box_id, &arranged);
            vec![box_id]
        }
    }
}

/// The boxes a `<details>` holds (ADR 0048 §§ 1–2): its summary's, first
/// wherever it was written, then one content box holding every other
/// child's — or, closed, holding nothing, because a closed `details`' content
/// makes no boxes and so is neither laid out, drawn nor read.
///
/// A `details` with no summary has an empty summary slot, recorded: a
/// browser makes it a summary saying "Details", which is text nobody wrote
/// and is queue item 433's to decide.
fn details_children(
    document: &Document,
    styles: &StyleTree,
    shown: &Shown,
    id: NodeId,
    slots: &crate::details::Slots,
    tree: &mut BoxTree,
) -> Vec<BoxId> {
    let mut made = Vec::new();
    if let Some(summary) = slots.summary {
        made = build_one(document, styles, shown, summary, tree);
    } else {
        tree.issues.push(StyleIssue {
            kind: IssueKind::UnsupportedValue,
            source: format!(
                "a <details> with no <summary> (on {id}): the summary a browser makes for it was not made"
            ),
            at: Location { line: 0, column: 0 },
        });
    }
    let content = tree.push(
        BoxKind::Anonymous {
            outside: Outside::Block,
            purpose: Purpose::DetailsContent,
        },
        Semantics::anonymous(),
    );
    if slots.open {
        let held: Vec<BoxId> = slots
            .content
            .iter()
            .flat_map(|child| build_one(document, styles, shown, *child, tree))
            .collect();
        let arranged = arrange(tree, content, held);
        tree.adopt(content, &arranged);
    }
    made.push(content);
    made
}

/// The box an `<img>` makes (ADR 0046).
///
/// A picture is a replaced box of its size. With no picture, an `alt` is one
/// text box inside a box that is **not** replaced, so its `display` is its
/// style's: `inline` by default, a run that wraps with its line and takes no
/// `width`. The text box's node is the `<img>`, so its style is the
/// `<img>`'s, as a field's value is the field's. With neither, a replaced box
/// of nothing. Either way without a picture, the box carries `broken` (§ 5).
fn img_box(
    document: &Document,
    styles: &StyleTree,
    id: NodeId,
    element: &alo_dom::Element,
    display: Display,
    img: Img,
    tree: &mut BoxTree,
) -> Vec<BoxId> {
    let mut semantics = Semantics::of(document, id, element);
    semantics.states.broken = img.is_broken();
    let box_id = tree.push(BoxKind::Element { node: id, display }, semantics);
    match img {
        Img::Picture(size) => {
            tree.natural.insert(box_id, size);
        }
        Img::Nothing => {
            tree.natural.insert(box_id, crate::picture::nothing());
        }
        Img::Text(alt) => {
            // Collapsed as the text of a child of the `<img>` would be: by
            // the `<img>`'s own `white-space`, which the text inherits.
            let text = white_space_at(styles, document, Some(id)).apply(&alt);
            let text = transformed(document, styles, styles.get(id), id, text, tree);
            let text = tree.push(BoxKind::Text { node: id, text }, Semantics::anonymous());
            let arranged = arrange(tree, box_id, vec![text]);
            tree.adopt(box_id, &arranged);
        }
    }
    vec![box_id]
}

/// The one box an outermost `<svg>` makes (ADR 0022 § 1).
///
/// Whatever its `display` says about its inside, it gets **no children**:
/// what is inside an `<svg>` is drawn, not laid out, so a `<path>` makes no
/// box any more than a pixel of an `<img>` does. Its natural size comes from
/// its own attributes, and a `viewBox` that could not be read is recorded.
fn svg_box(
    document: &Document,
    id: NodeId,
    element: &alo_dom::Element,
    display: Display,
    tree: &mut BoxTree,
) -> BoxId {
    let semantics = Semantics::of(document, id, element);
    let box_id = tree.push(BoxKind::Element { node: id, display }, semantics);
    let (natural, refused) = crate::svg::natural_size(element);
    tree.natural.insert(box_id, natural);
    tree.issues
        .extend(refused.into_iter().map(|source| StyleIssue {
            kind: IssueKind::UnsupportedValue,
            source,
            at: Location { line: 0, column: 0 },
        }));
    box_id
}

/// What `display` an element ends up with, recording a value this engine does
/// not implement rather than guessing at it.
///
/// An absolutely positioned box is **blockified** (CSS Display § 2.7, CSS 2
/// § 9.7): whatever `display` says about its outside, it does not sit in a
/// line, so it is never wrapped into one. alo Sites' skip link is an inline
/// `<a>` with `position: absolute; left: -999rem`, and before this it was
/// drawn in a line of its own at the top of every page alo Sites publishes.
fn display_of(style: Option<&ComputedStyle>, id: NodeId, tree: &mut BoxTree) -> Display {
    let display = written_display(style, id, tree);
    if is_absolutely_positioned(style) {
        display.blockified()
    } else {
        display
    }
}

/// `display` as written, or the initial value if nothing set it or this
/// engine does not implement what was written.
fn written_display(style: Option<&ComputedStyle>, id: NodeId, tree: &mut BoxTree) -> Display {
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

/// Whether `position` takes the box out of flow.
///
/// Only `absolute`, because it is the only such value layout places:
/// `fixed` and `sticky` are refused there and fall back to `static`, said
/// once in layout's issues, so blockifying them here would make a box
/// block-level and leave it in flow, which no value of `position` does.
/// What layout refuses is recorded there, not again here.
fn is_absolutely_positioned(style: Option<&ComputedStyle>) -> bool {
    style
        .and_then(|style| style.get("position"))
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("absolute"))
}

/// Put a box's children into a shape layout can walk: all block-level, or all
/// inline-level, never a mix.
///
/// A flow container does this by wrapping each run of inline-level boxes
/// beside a block. A flex or grid container has items instead, and only its
/// text needs a wrapper; see [`wrap_text_runs`].
fn arrange(tree: &mut BoxTree, parent: BoxId, children: Vec<BoxId>) -> Vec<BoxId> {
    let container_is_flow = tree
        .get(parent)
        .is_some_and(|node| node.kind.lays_children_out_in_flow());
    if !container_is_flow {
        return wrap_text_runs(tree, children);
    }

    // An inline box holds only what sits in a line: whatever is out of flow
    // in it stays there, because it was never a block among blocks.
    let in_an_inline = tree
        .get(parent)
        .is_some_and(|node| node.kind.is_inline_box());
    let any_block = children.iter().any(|child| is_in_flow_block(tree, *child));
    let any_out_of_flow = children.iter().any(|child| tree.is_out_of_flow(*child));
    if in_an_inline || (!any_block && (!any_out_of_flow || holds_inline_content(tree, &children))) {
        // Every child sits in a line, or is out of flow among things that
        // do, so this is one run of text and the spaces in it are the gaps
        // between words.
        return children;
    }

    let mut arranged: Vec<BoxId> = Vec::new();
    let mut run: Vec<BoxId> = Vec::new();
    for child in children {
        if is_in_flow_block(tree, child) {
            flush_segment(tree, &mut run, &mut arranged);
            arranged.push(child);
        } else {
            run.push(child);
        }
    }
    flush_segment(tree, &mut run, &mut arranged);
    arranged
}

/// Whether a box is a block among blocks: block-level and in flow.
///
/// A box `position: absolute` takes out of flow is block-level too, and
/// takes no room where it stands, so it is neither a reason to wrap what is
/// beside it in anonymous blocks nor to break the inline box it is in.
fn is_in_flow_block(tree: &BoxTree, id: BoxId) -> bool {
    !tree.is_out_of_flow(id)
        && tree
            .get(id)
            .is_some_and(|node| node.kind.outside() == Outside::Block)
}

/// A control's inside, told whether it is at least a line tall now that what
/// it holds is known: a `<button>` element is when it holds something a line
/// is made of — text, an inline `<svg>` — and not when it holds only blocks
/// or nothing (queue item 437). Every other control already is.
fn with_its_line(tree: &BoxTree, purpose: Purpose, children: &[BoxId]) -> Purpose {
    let Purpose::Control {
        centred,
        one_line,
        at_least_a_line: false,
    } = purpose
    else {
        return purpose;
    };
    let in_lines: Vec<BoxId> = children
        .iter()
        .copied()
        .filter(|child| !is_in_flow_block(tree, *child))
        .collect();
    Purpose::Control {
        centred,
        one_line,
        at_least_a_line: holds_inline_content(tree, &in_lines),
    }
}

/// Whether any of these boxes is something a line is made of: an inline-level
/// box in flow, or text that is not only whitespace.
fn holds_inline_content(tree: &BoxTree, children: &[BoxId]) -> bool {
    children
        .iter()
        .any(|child| !tree.is_out_of_flow(*child) && !is_only_whitespace(tree, *child))
}

/// Finish what stood between two blocks.
///
/// When it holds something a line is made of, it is one run, and a box out
/// of flow in it stays in it: `Read article<span class=sr-only>…</span>` is
/// one line, and the `<span>` stands at its end (queue item 354). When it
/// holds only boxes out of flow and whitespace, each box stands among the
/// blocks as it was written, and the whitespace separates nothing: alo
/// Sites' skip link, before `<header>`, is a box of its own and no line.
fn flush_segment(tree: &mut BoxTree, run: &mut Vec<BoxId>, arranged: &mut Vec<BoxId>) {
    if holds_inline_content(tree, run) {
        flush_run(tree, run, arranged);
        return;
    }
    arranged.extend(run.drain(..).filter(|child| tree.is_out_of_flow(*child)));
}

/// The items of a flex or grid container.
///
/// Every child that is an element is an item in its own right, whatever its
/// `display` says, so wrapping it would invent a row nobody asked for. Text is
/// not an element, and CSS Flexbox § 4 and CSS Grid § 6 say what it becomes:
/// *"each contiguous sequence of child text runs is wrapped in an anonymous
/// block container"* item. Its lines are then laid out like any other block's,
/// `line-height` and all; measured bare, a button's label was only as tall as
/// its font. A sequence that is only whitespace is the gap between two items
/// and is not rendered.
fn wrap_text_runs(tree: &mut BoxTree, children: Vec<BoxId>) -> Vec<BoxId> {
    let mut items: Vec<BoxId> = Vec::new();
    let mut run: Vec<BoxId> = Vec::new();
    for child in children {
        if tree.get(child).and_then(BoxNode::text).is_some() {
            run.push(child);
        } else {
            flush_run(tree, &mut run, &mut items);
            items.push(child);
        }
    }
    flush_run(tree, &mut run, &mut items);
    items
}

/// The box a `<fieldset>` shows in its block-start band, out of the children
/// it ended up with.
///
/// **The first `<legend>` among them**, which is what HTML calls the fieldset's
/// rendered legend — the one whose words name the group, and the one the
/// fieldset's border is broken around. Every other legend inside a fieldset is
/// an ordinary block.
///
/// Three things are deliberately not one:
///
/// - A fieldset the author made a **flex or grid container** has no rendered
///   legend here. Its children are items in an arrangement of the author's,
///   and lifting one of them into the border would be this engine overruling a
///   layout somebody wrote.
/// - An **inline-level** legend is not one either: it is in a run with the text
///   beside it by the time this is asked, which is why this looks at the
///   arranged children rather than at the raw ones. A band holds a block.
/// - A legend that generated **no box** — `display: none`, or `contents` —
///   cannot be found here, which is the right answer rather than a missing
///   case: there is nothing to put in the band.
fn legend_of(
    document: &Document,
    element: &alo_dom::Element,
    display: Display,
    tree: &BoxTree,
    children: &[BoxId],
) -> Option<BoxId> {
    if !element.name.is_html("fieldset")
        || !matches!(display.inside(), Some(Inside::Flow | Inside::FlowRoot))
    {
        return None;
    }
    children.iter().copied().find(|child| {
        tree.get(*child)
            .and_then(|node| node.kind.node())
            .and_then(|node| document.element(node))
            .is_some_and(|element| element.name.is_html("legend"))
    })
}

/// Whether an element is a form control that holds what it shows in a box of
/// its own, and what that box is: whether it centres what is in it, and
/// whether it is a one-line field.
///
/// A button's label sits in the middle of it; a field's text sits at the start
/// and one line down. Both are things browsers do with an internal box rather
/// than with a style sheet rule, which is exactly why they are here.
fn control_content(element: &alo_dom::Element) -> Option<Purpose> {
    let button = Purpose::Control {
        centred: true,
        one_line: false,
        at_least_a_line: true,
    };
    if element.name.is_html("button") {
        return Some(Purpose::Control {
            centred: true,
            one_line: false,
            at_least_a_line: false,
        });
    }
    if element.name.is_html("textarea") {
        return Some(Purpose::Control {
            centred: false,
            one_line: false,
            at_least_a_line: true,
        });
    }
    if !element.name.is_html("input") {
        return None;
    }
    let kind = element
        .attr("type")
        .map_or_else(|| "text".to_owned(), str::to_ascii_lowercase);
    match kind.as_str() {
        "button" | "submit" | "reset" => Some(button),
        // A checkbox and a radio draw themselves; there is nothing inside them
        // to hold, and a box would only make them taller.
        "checkbox" | "radio" | "hidden" | "image" | "range" | "color" => None,
        _ => Some(Purpose::Control {
            centred: false,
            one_line: true,
            at_least_a_line: true,
        }),
    }
}

/// `text`, collapsed already, as it is drawn: in the case `style`'s
/// `text-transform` asks for, in the language of `element`, which holds it
/// (ADR 0049 §§ 1–2).
///
/// A value this engine does not build draws the text as written and is
/// recorded once, against the element it was given to (§ 3), rather than
/// once for every run of text that inherits it.
fn transformed(
    document: &Document,
    styles: &StyleTree,
    style: Option<&ComputedStyle>,
    element: NodeId,
    text: String,
    tree: &mut BoxTree,
) -> String {
    let Some(value) = style.and_then(|style| style.get("text-transform")) else {
        return text;
    };
    let transform = TextTransform::parse(value);
    if transform == TextTransform::Unbuilt {
        let source = format!(
            "text-transform: {value} (on {})",
            given_to(document, styles, element, value)
        );
        if !tree.issues.iter().any(|issue| issue.source == source) {
            tree.issues.push(StyleIssue {
                kind: IssueKind::UnsupportedValue,
                source,
                at: Location { line: 0, column: 0 },
            });
        }
    }
    transform.apply(text, language_of(document, Some(element)))
}

/// The element a `text-transform` of `value` reaching `element` was given
/// to: the furthest ancestor up to which it is inherited unchanged.
fn given_to(document: &Document, styles: &StyleTree, element: NodeId, value: &str) -> NodeId {
    let mut given = element;
    while let Some(parent) = document.parent(given)
        && styles
            .get(parent)
            .and_then(|style| style.get("text-transform"))
            == Some(value)
    {
        given = parent;
    }
    given
}

/// How the whitespace around a text node is treated.
///
/// `white-space` inherits and a text node has no style of its own, so the
/// answer comes from the element holding it — the same walk a text box's font
/// takes, and for the same reason.
fn white_space_of(document: &Document, styles: &StyleTree, id: NodeId) -> WhiteSpace {
    white_space_at(styles, document, document.parent(id))
}

/// The `white-space` of an element, inherited from the nearest ancestor that
/// has one, for text it holds.
fn white_space_at(styles: &StyleTree, document: &Document, element: Option<NodeId>) -> WhiteSpace {
    let mut current = element;
    while let Some(ancestor) = current {
        if let Some(style) = styles.get(ancestor)
            && let Some(text) = style.get("white-space")
        {
            // A value this engine does not implement leaves the initial one,
            // which is what CSS does with any value it cannot parse.
            return WhiteSpace::parse(text).unwrap_or_default();
        }
        current = document.parent(ancestor);
    }
    WhiteSpace::default()
}

/// Whether any of these boxes is a block in flow, which an inline box has
/// to be broken around.
fn holds_a_block(tree: &BoxTree, children: &[BoxId]) -> bool {
    children.iter().any(|child| is_in_flow_block(tree, *child))
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
        if !is_in_flow_block(tree, child) {
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

/// Whether a box is a text box holding nothing but whitespace — document
/// white space, which a no-break space is not: `&nbsp;` alone is something
/// to draw.
fn is_only_whitespace(tree: &BoxTree, id: BoxId) -> bool {
    tree.get(id).and_then(BoxNode::text).is_some_and(|text| {
        text.trim_matches(|c: char| c.is_ascii_whitespace())
            .is_empty()
    })
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
            purpose: Purpose::Run,
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

    /// The boxes of some markup in which the `<img id=shown>` shows a picture
    /// eight by four.
    fn boxes_showing(html: &str) -> BoxTree {
        let document = parse_document(html);
        let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let sheets = [SourcedSheet::new(Origin::UserAgent, &agent)];
        let styles = resolve(&document, &sheets, &MediaContext::default());
        let mut shown = Shown::default();
        if let Some(img) = document.descendants(document.root()).find(|id| {
            document
                .element(*id)
                .is_some_and(|element| element.attr("id") == Some("shown"))
        }) {
            shown.show(img, NaturalSize::sized(8.0, 4.0));
        }
        build_showing(&document, &styles, &shown)
    }

    #[test]
    fn an_img_is_a_picture_its_alt_or_nothing() {
        let tree =
            boxes_showing("<p><img id=shown alt=Shown><img alt='Not shown'><img alt=''></p>");
        let outline = tree.to_outline();
        assert!(
            outline.contains(concat!(
                "      inline flow · image \"Shown\"\n",
                "      inline flow · image \"Not shown\" [broken]\n",
                "        text \"Not shown\"\n",
                "      inline flow · image [broken]\n",
            )),
            "{outline}",
        );
        let imgs: Vec<BoxId> = tree
            .ids()
            .filter(|id| {
                tree.get(*id)
                    .is_some_and(|node| node.semantics.role.to_string() == "image")
            })
            .collect();
        let natural: Vec<Option<NaturalSize>> =
            imgs.iter().map(|id| tree.natural_size(*id)).collect();
        assert_eq!(
            natural,
            [
                Some(NaturalSize::sized(8.0, 4.0)),
                None,
                Some(NaturalSize::sized(0.0, 0.0)),
            ],
            "a picture, text that is not replaced, and a replaced box of nothing",
        );
    }

    #[test]
    fn an_img_with_display_contents_makes_nothing_whatever_it_shows() {
        let outline = body_outline(
            "<p><img alt=Gone style='display: contents'><img style='display: contents'></p>",
            "",
        );
        assert!(!outline.contains("image"), "{outline}");
        assert!(!outline.contains("Gone"), "{outline}");
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

    /// The `<svg>` box of `tree`, which the tests below have exactly one of.
    fn svg_of(tree: &BoxTree) -> BoxId {
        tree.ids()
            .find(|id| {
                tree.get(*id)
                    .is_some_and(|node| node.semantics.role.to_string() == "image")
            })
            .expect("an <svg> box")
    }

    /// ADR 0022 § 1, on alo's offline screen's own markup: one box, and the
    /// four paths inside it are not boxes.
    #[test]
    fn an_svg_is_one_box_and_its_shapes_are_not_boxes() {
        let html = "<main><svg viewBox='0 0 24 24' fill=none aria-hidden=true>\
             <path d='M18 11V6'/><path d='M14 10V4'/>\
             <g><circle r=1 /></g><text>words</text></svg></main>";
        let tree = boxes(html, "");
        let svg = svg_of(&tree);
        assert_eq!(tree.children(svg).count(), 0);
        assert_eq!(
            body_outline(html, ""),
            "block flow · generic\n  block flow · main\n    inline flow-root · image [hidden]\n",
        );
        let natural = tree.natural_size(svg).expect("an <svg> is replaced");
        assert_eq!((natural.width, natural.height), (None, None));
        assert_eq!(natural.ratio(), Some(1.0));
    }

    #[test]
    fn an_svg_the_author_made_a_container_still_has_no_boxes_inside() {
        let html = "<svg><rect width=4 height=4 /></svg>";
        let tree = boxes(html, "svg { display: flex }");
        let svg = svg_of(&tree);
        assert!(
            tree.get(svg)
                .is_some_and(|node| node.kind.inside() == Inside::Flex)
        );
        assert_eq!(tree.children(svg).count(), 0);
        let html = "<svg>text <foreignObject><p>x</p></foreignObject></svg>";
        let tree = boxes(html, "svg { display: block }");
        assert_eq!(tree.children(svg_of(&tree)).count(), 0);
    }

    #[test]
    fn display_contents_on_an_svg_is_none() {
        assert_eq!(
            body_outline(
                "<p>a</p><svg><text>b</text></svg>",
                "svg { display: contents }"
            ),
            "block flow · generic\n  block flow · paragraph\n    text \"a\"\n",
        );
    }

    #[test]
    fn an_svg_takes_its_size_from_its_attributes() {
        let tree = boxes("<svg width=48 height=24></svg>", "");
        let natural = tree.natural_size(svg_of(&tree)).expect("replaced");
        assert_eq!(natural, NaturalSize::sized(48.0, 24.0));

        let tree = boxes("<svg></svg>", "");
        assert_eq!(
            tree.natural_size(svg_of(&tree)),
            Some(NaturalSize::default()),
            "an <svg> with nothing written is still replaced",
        );
    }

    /// A per-cent `width` is used now, by layout through the cascade (item
    /// 279), so the box no longer records it; a `viewBox` it cannot read it
    /// still does.
    #[test]
    fn what_an_svg_wrote_that_was_not_used_is_recorded() {
        let tree = boxes("<svg width=50% viewBox='0 0 -1 1'></svg>", "");
        let issues: Vec<String> = tree.issues().iter().map(ToString::to_string).collect();
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(
            issues.iter().any(|issue| issue.contains("viewBox")),
            "{issues:?}"
        );
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
    fn a_flex_container_wraps_only_its_text_because_every_element_is_an_item() {
        let outline = body_outline(
            "<div id=f>text <b>bold</b><p>block</p></div>",
            "#f { display: flex }",
        );
        assert_eq!(
            outline.matches("anonymous").count(),
            1,
            "in flex, a mix of children is not a mix of anything; only the \
             text is wrapped:\n{outline}",
        );
        assert!(outline.contains("block flex"), "{outline}");
        let anonymous = outline
            .lines()
            .position(|line| line.contains("anonymous"))
            .unwrap_or(usize::MAX);
        let text = outline
            .lines()
            .position(|line| line.contains("text \"text \""))
            .unwrap_or(0);
        assert_eq!(text, anonymous + 1, "the text is inside it:\n{outline}");
        let bold = outline
            .lines()
            .find(|line| line.contains("text \"bold\""))
            .unwrap_or_default();
        let wrapper = outline
            .lines()
            .find(|line| line.contains("anonymous"))
            .unwrap_or_default();
        assert!(
            indent(bold) > indent(wrapper),
            "and the <b>'s own text is in the <b>, deeper than the wrapper's \
             level:\n{outline}",
        );
    }

    /// How far a line of an outline is indented.
    fn indent(line: &str) -> usize {
        line.len() - line.trim_start().len()
    }

    #[test]
    fn a_grid_wraps_each_run_of_text_and_drops_a_run_of_only_whitespace() {
        let outline = body_outline(
            "<div id=g>one <span style='display: contents'>two</span>\
             <i>item</i>\n  <i>item</i> three</div>",
            "#g { display: grid }",
        );
        assert_eq!(
            outline.matches("anonymous").count(),
            2,
            "\"one \" and \"two\" are one run, and \" three\" another; the \
             newline between the <i>s is no item:\n{outline}",
        );
        assert_eq!(outline.matches("text").count(), 5, "{outline}");
    }

    #[test]
    fn a_flex_container_holding_only_whitespace_has_no_items() {
        let outline = body_outline("<div id=f>\n   \n</div>", "#f { display: flex }");
        assert!(!outline.contains("anonymous"), "{outline}");
        assert!(!outline.contains("text"), "{outline}");
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
    fn an_absolutely_positioned_inline_is_block_level_and_in_no_line() {
        // alo Sites' skip link: an inline `<a>` beside a block, which would
        // otherwise be wrapped in a line nobody wrote.
        let outline = body_outline(
            "<a id=skip href=#main>Skip</a><main id=main><p>body</p></main>",
            "#skip { position: absolute; left: -999rem; top: 0 }",
        );
        assert!(outline.contains("block flow · link"), "{outline}");
        assert!(!outline.contains("anonymous block"), "{outline}");
    }

    #[test]
    fn blockifying_keeps_what_a_box_lays_out_inside_it() {
        let outline = body_outline(
            "<p><span id=a>a</span><span id=b>b</span></p>",
            "#a { display: inline-block; position: ABSOLUTE }
             #b { display: inline-flex; position: absolute }",
        );
        assert!(outline.contains("block flow-root"), "{outline}");
        assert!(outline.contains("block flex"), "{outline}");
    }

    #[test]
    fn a_position_that_leaves_a_box_in_flow_leaves_it_inline() {
        // `fixed` and `sticky` are refused by layout and fall back to
        // `static`, so they stay where `display` put them.
        for position in ["static", "relative", "fixed", "sticky"] {
            let outline = body_outline(
                "<p>one <a id=a href=#>two</a> three</p>",
                &format!("#a {{ position: {position} }}"),
            );
            assert!(
                outline.contains("inline flow · link"),
                "{position}: {outline}"
            );
        }
    }

    #[test]
    fn display_none_or_contents_is_not_blockified_into_a_box() {
        let gone = body_outline(
            "<p>one <a id=a href=#>two</a></p>",
            "#a { display: none; position: absolute }",
        );
        assert!(!gone.contains("link"), "{gone}");
        let contents = body_outline(
            "<p>one <a id=a href=#>two</a></p>",
            "#a { display: contents; position: absolute }",
        );
        assert!(!contents.contains("link"), "{contents}");
        assert!(contents.contains("two"), "{contents}");
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

    /// The inline-level element boxes arranging their children by `inside`.
    fn inline_level(tree: &BoxTree, inside: Inside) -> Vec<BoxId> {
        (0..tree.len())
            .map(BoxId)
            .filter(|id| {
                tree.get(*id).is_some_and(|node| {
                    matches!(node.kind, BoxKind::Element { .. })
                        && node.kind.outside() == Outside::Inline
                        && node.kind.inside() == inside
                })
            })
            .collect()
    }

    /// Whether every child of `id` is an element's block box, and there are
    /// `count` of them.
    fn holds_blocks(tree: &BoxTree, id: BoxId, count: usize) -> bool {
        let children: Vec<&BoxNode> = tree.children(id).filter_map(|c| tree.get(c)).collect();
        children.len() == count
            && children.iter().all(|child| {
                child.kind.outside() == Outside::Block
                    && matches!(child.kind, BoxKind::Element { .. })
            })
    }

    /// Queue item 286: an `inline-block` is one atomic box on its line,
    /// and a block inside it is a block like any other, not a reason to
    /// break it.
    #[test]
    fn an_inline_block_holding_blocks_is_one_box_holding_them() {
        let tree = boxes(
            "<div>ab<span id=i><p>cd</p><p>ef</p></span>gh</div>",
            "#i { display: inline-block }",
        );
        let outline = tree.to_outline();
        let made = inline_level(&tree, Inside::FlowRoot);
        assert_eq!(made.len(), 1, "one box, not pieces:\n{outline}");
        assert!(
            !(0..tree.len())
                .map(BoxId)
                .any(|id| tree.is_continuation(id)),
            "and nothing continues it:\n{outline}",
        );
        assert!(
            made.iter().all(|id| holds_blocks(&tree, *id, 2)),
            "its two paragraphs, both blocks, its own:\n{outline}",
        );
        // It is inline-level, so the text on each side of it shares its
        // line: the `<div>` holds one run, with nothing wrapped.
        assert!(!outline.contains("anonymous"), "{outline}");
    }

    /// Inside an `inline-block`, text beside a block is wrapped in an
    /// anonymous block as it is inside any block container, rather than
    /// left in lines it would share with the block.
    #[test]
    fn text_beside_a_block_in_an_inline_block_is_wrapped() {
        let tree = boxes(
            "<div><span id=i>cd<p>ef</p>gh</span></div>",
            "#i { display: inline-block }",
        );
        let outline = tree.to_outline();
        let made = inline_level(&tree, Inside::FlowRoot);
        assert_eq!(made.len(), 1, "{outline}");
        let kinds: Vec<Outside> = made
            .iter()
            .flat_map(|id| tree.children(*id))
            .filter_map(|child| tree.get(child).map(|node| node.kind.outside()))
            .collect();
        assert_eq!(
            kinds,
            [Outside::Block, Outside::Block, Outside::Block],
            "an anonymous block, the paragraph, an anonymous block:\n{outline}",
        );
        assert_eq!(
            outline.matches("anonymous").count(),
            2,
            "one for each run of text:\n{outline}"
        );
    }

    /// alo Sites' brand, in its full page: a link made `inline-flex` holds
    /// its logo, which the sheet makes `display: block`. A flex container's
    /// children are its items whatever their `display`, so the link is one
    /// box holding the picture, not two empty pieces with the picture
    /// between them.
    #[test]
    fn an_inline_flex_holding_a_block_is_one_box_holding_it() {
        let tree = boxes(
            "<nav><a id=i href=/><img alt=Nordwind></a></nav>",
            "#i { display: inline-flex } img { display: block }",
        );
        let outline = tree.to_outline();
        let made = inline_level(&tree, Inside::Flex);
        assert_eq!(made.len(), 1, "one link, not pieces:\n{outline}");
        assert!(
            made.iter().all(|id| holds_blocks(&tree, *id, 1)),
            "holding its picture:\n{outline}",
        );
        assert!(!outline.contains("anonymous"), "{outline}");
    }

    /// Queue item 426's markup: an `inline-grid`'s blocks are its items.
    #[test]
    fn an_inline_grid_holding_blocks_is_one_box_holding_them() {
        let tree = boxes(
            "<body><div id=i><div></div><div></div></div></body>",
            "#i { display: inline-grid }",
        );
        let outline = tree.to_outline();
        let made = inline_level(&tree, Inside::Grid);
        assert_eq!(made.len(), 1, "one grid, not four pieces:\n{outline}");
        assert!(
            made.iter().all(|id| holds_blocks(&tree, *id, 2)),
            "holding both blocks:\n{outline}",
        );
    }

    /// Queue item 354, from alo Sites' blog cards: a box out of flow is not
    /// a block among blocks, so the inline box it is in is not broken around
    /// it, and nothing is wrapped in an anonymous block for it.
    #[test]
    fn an_inline_holding_a_box_out_of_flow_is_not_broken_around_it() {
        let html = "<div><a href=#p>Read article<span id=s>: notes</span></a></div>";
        let tree = boxes(html, "#s { position: absolute }");
        let outline = tree.to_outline();
        assert_eq!(
            outline
                .lines()
                .filter(|line| line.contains("inline flow · link"))
                .count(),
            1,
            "one link, unbroken:\n{outline}",
        );
        assert!(!outline.contains("anonymous"), "{outline}");
        assert!(
            outline.contains("block flow · generic"),
            "still blockified:\n{outline}"
        );
        let span = (0..tree.len())
            .map(BoxId)
            .find(|id| tree.is_out_of_flow(*id))
            .expect("the span is out of flow");
        let parent = tree.get(span).and_then(|node| node.parent);
        assert!(
            parent
                .and_then(|parent| tree.get(parent))
                .is_some_and(|node| node.kind.outside() == Outside::Inline),
            "it is the link's child:\n{outline}",
        );
        assert_eq!(
            (0..tree.len())
                .map(BoxId)
                .filter(|id| tree.is_out_of_flow(*id))
                .count(),
            1,
            "and nothing else is out of flow",
        );
    }

    /// Between two blocks, a box out of flow goes with the text it is among:
    /// one anonymous block holds "one", the box and " three". Among blocks
    /// with no text beside it, it is a box of its own, as alo Sites' skip
    /// link is (`an_absolutely_positioned_inline_is_block_level_and_in_no_line`).
    #[test]
    fn a_box_out_of_flow_among_text_beside_a_block_stays_in_its_run() {
        let outline = body_outline(
            "<div>one<span id=s>two</span> three<p>block</p></div>",
            "#s { position: absolute }",
        );
        assert_eq!(
            outline.matches("anonymous").count(),
            1,
            "one run, not two:\n{outline}",
        );
        let lines: Vec<&str> = outline.lines().collect();
        let depth = |line: &str| line.len() - line.trim_start().len();
        let two = lines
            .iter()
            .position(|line| line.contains("text \"two\""))
            .expect("the span's text");
        let span = two
            .checked_sub(1)
            .and_then(|at| lines.get(at))
            .expect("the span");
        let run = lines
            .iter()
            .find(|line| line.contains("anonymous"))
            .expect("the run");
        assert!(
            span.contains("block flow · generic") && depth(span) > depth(run),
            "the span is inside the run:\n{outline}",
        );
        // A container of nothing but whitespace is still one, as it was.
        let tree = boxes("<div> </div>", "");
        assert!(
            tree.to_outline().contains("text \" \""),
            "{}",
            tree.to_outline()
        );
    }

    /// What the box a control holds its content in is, for each control.
    fn inside_of(html: &str) -> Vec<Purpose> {
        let tree = boxes(html, "");
        let Some(root) = tree.root() else {
            return Vec::new();
        };
        tree.descendants(root)
            .into_iter()
            .filter_map(|id| match tree.get(id).map(|node| &node.kind) {
                Some(BoxKind::Anonymous {
                    purpose: purpose @ Purpose::Control { .. },
                    ..
                }) => Some(*purpose),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn only_a_field_a_person_types_one_line_into_holds_one_line() {
        let field = Purpose::Control {
            centred: false,
            one_line: true,
            at_least_a_line: true,
        };
        for html in [
            "<input>",
            "<input type=date>",
            "<input type=email value=a>",
            "<input type=PASSWORD>",
        ] {
            assert_eq!(inside_of(html), vec![field], "{html}");
        }
        let button = Purpose::Control {
            centred: true,
            one_line: false,
            at_least_a_line: true,
        };
        for html in ["<input type=submit>", "<input type=button value=''>"] {
            assert_eq!(inside_of(html), vec![button], "{html}");
        }
        assert_eq!(
            inside_of("<textarea></textarea>"),
            vec![Purpose::Control {
                centred: false,
                one_line: false,
                at_least_a_line: true,
            }],
        );
        assert!(inside_of("<input type=checkbox>").is_empty());
    }

    /// Queue item 437: a `<button>` element is what it holds, and nothing
    /// when it holds nothing, where every other control is at least a line
    /// tall (measured in Chrome 154).
    #[test]
    fn only_a_button_element_is_no_taller_than_what_it_holds() {
        let button = |at_least_a_line| Purpose::Control {
            centred: true,
            one_line: false,
            at_least_a_line,
        };
        for html in [
            "<button></button>",
            "<button> </button>",
            "<BUTTON type=submit>\n  <svg style='display: block'></svg>\n</BUTTON>",
            "<button><div>Go</div></button>",
        ] {
            assert_eq!(inside_of(html), vec![button(false)], "{html}");
        }
        for html in [
            "<button>Go</button>",
            "<button><svg></svg></button>",
            "<button><svg style='display: block'></svg>New</button>",
        ] {
            assert_eq!(inside_of(html), vec![button(true)], "{html}");
        }
    }

    /// The text boxes a document's fields show, in order.
    fn shown(html: &str) -> Vec<String> {
        let tree = boxes(html, "");
        let Some(root) = tree.root() else {
            return Vec::new();
        };
        tree.descendants(root)
            .into_iter()
            .filter_map(|id| tree.get(id).and_then(BoxNode::text).map(str::to_owned))
            .collect()
    }

    /// ADR 0042 §§ 2–3: an empty date field draws how a date is written, as
    /// its text; one holding a date draws it in the same order; a `value`
    /// that is not a date is sanitised to empty and draws the format.
    #[test]
    fn a_date_field_shows_its_date_or_how_one_is_written() {
        assert_eq!(shown("<input type=date>"), ["yyyy-mm-dd"]);
        assert_eq!(shown("<input type=date value=''>"), ["yyyy-mm-dd"]);
        assert_eq!(shown("<input type=DATE value=2026-10-12>"), ["2026-10-12"]);
        for sanitised in ["12/10/2026", "2026-02-29", "2026-13-01", "0000-01-01"] {
            assert_eq!(
                shown(&format!("<input type=date value={sanitised}>")),
                ["yyyy-mm-dd"],
                "{sanitised}",
            );
        }
        let hostile = format!("<input type=date value={}>", "9".repeat(50_000));
        assert_eq!(shown(&hostile), ["yyyy-mm-dd"]);
        // The other temporal kinds wait for a page (item 388), and an empty
        // text field still shows nothing.
        assert!(shown("<input type=time>").is_empty());
        assert!(shown("<input>").is_empty());
    }

    /// The hints a document's fields show, in order: the text of every box
    /// that is a `::placeholder`.
    fn hints(html: &str) -> Vec<String> {
        let tree = boxes(html, "");
        let Some(root) = tree.root() else {
            return Vec::new();
        };
        tree.descendants(root)
            .into_iter()
            .filter(|id| tree.is_placeholder(*id))
            .filter_map(|id| tree.get(id).and_then(BoxNode::text).map(str::to_owned))
            .collect()
    }

    /// ADR 0043 § 2: each kind that takes a hint shows it while it holds
    /// nothing, as a box of its own where its text would be; an `<input>`'s
    /// line breaks are stripped and a `<textarea>`'s kept; a date field, an
    /// empty attribute and a held value make none.
    #[test]
    fn a_field_holding_nothing_shows_its_hint_as_a_placeholder() {
        for kind in [
            "text", "search", "url", "tel", "email", "password", "number", "Email",
        ] {
            let html = format!("<input type={kind} placeholder='you@company.eu'>");
            assert_eq!(hints(&html), ["you@company.eu"], "{kind}");
            assert_eq!(shown(&html), ["you@company.eu"], "and nothing else: {kind}");
        }
        assert_eq!(hints("<input placeholder=hint>"), ["hint"]);
        assert_eq!(hints("<input placeholder='one\ntwo'>"), ["onetwo"]);
        assert_eq!(
            hints("<textarea placeholder='one\ntwo'></textarea>"),
            ["one\ntwo"]
        );
        for none in [
            "<input type=date placeholder=hint>",
            "<input type=checkbox placeholder=hint>",
            "<input placeholder=''>",
            "<input placeholder='\n\n'>",
            "<textarea placeholder=hint>held</textarea>",
        ] {
            assert!(hints(none).is_empty(), "{none}");
        }
        // A value shows instead, and is not a placeholder.
        assert!(hints("<input placeholder=hint value=held>").is_empty());
        assert_eq!(shown("<input placeholder=hint value=held>"), ["held"]);
        // A password's hint is written out: it is not the secret.
        assert_eq!(
            shown("<input type=password placeholder=Secret>"),
            ["Secret"]
        );

        // A hostile hint is shown without a panic.
        let long = format!("<input placeholder='{}'>", "x".repeat(100_000));
        assert_eq!(hints(&long).first().map(String::len), Some(100_000));
    }

    /// ADR 0044 § 4: a list item that draws a marker has it as its first
    /// box, a text box set in `::marker`'s style, before any anonymous block
    /// its children were wrapped in; one that makes none, or draws none, has
    /// none.
    #[test]
    fn a_closed_details_is_its_summary_and_an_empty_content_box() {
        assert_eq!(
            body_outline(
                "<body><details><summary>Q</summary><p>A</p></details></body>",
                ""
            ),
            "block flow · generic\n\
             \x20 block flow · group\n\
             \x20   block flow · summary [expanded=false]\n\
             \x20     text \"Q\"\n\
             \x20   anonymous block\n"
        );
    }

    #[test]
    fn an_open_details_holds_the_rest_in_its_content_box_after_the_summary() {
        // The summary first wherever it was written; a second summary is an
        // ordinary block in the content; text beside a block is wrapped.
        assert_eq!(
            body_outline(
                "<body><details open>x<p>A</p><summary>Q</summary><summary>R</summary></details></body>",
                ""
            ),
            "block flow · generic\n\
             \x20 block flow · group\n\
             \x20   block flow · summary [expanded=true]\n\
             \x20     text \"Q\"\n\
             \x20   anonymous block\n\
             \x20     anonymous block\n\
             \x20       text \"x\"\n\
             \x20     block flow · paragraph\n\
             \x20       text \"A\"\n\
             \x20     block flow · generic\n\
             \x20       text \"R\"\n"
        );
    }

    #[test]
    fn a_details_with_no_summary_has_only_its_content_box_and_says_so() {
        let tree = boxes("<details><p>A</p></details>", "");
        let issues: Vec<String> = tree.issues().iter().map(ToString::to_string).collect();
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(
            issues[0].contains("<details> with no <summary>"),
            "{issues:?}"
        );
        assert!(
            !tree.to_outline().contains("paragraph"),
            "closed, its content makes no boxes:\n{}",
            tree.to_outline()
        );
    }

    #[test]
    fn a_list_items_marker_is_its_first_box_and_joins_no_block() {
        let html = "<ul><li>one<li>two<ul><li>three</ul></ul><ol start=3><li>Draft</ol>\
                    <ul class=bare><li>four</ul><ol class=roman><li>five</ol>";
        let css = ".bare { list-style: none } .roman { list-style-type: upper-roman } \
                   li::marker { color: #b4532a }";
        let tree = boxes(html, css);
        let root = tree.root().expect("a root");
        let markers: Vec<(BoxId, String)> = tree
            .descendants(root)
            .into_iter()
            .filter(|id| tree.is_marker(*id))
            .filter_map(|id| Some((id, tree.get(id)?.text()?.to_owned())))
            .collect();
        let texts: Vec<&str> = markers.iter().map(|(_, text)| text.as_str()).collect();
        assert_eq!(texts, ["• ", "• ", "◦ ", "3. "]);
        for (marker, _) in &markers {
            let item = tree.get(*marker).and_then(|node| node.parent);
            assert_eq!(item.and_then(|item| tree.marker_of(item)), Some(*marker));
        }
        // "two" sits beside a block, so it is wrapped; the marker is not.
        let outline = tree.to_outline();
        assert!(outline.contains("text \"• \" · ::marker"), "{outline}");
        // Set in its own style, not its item's.
        let document = parse_document("<ul><li>a</ul>");
        let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let author = parse_stylesheet("li { color: #102a43 } li::marker { color: #b4532a }");
        let sheets = [
            SourcedSheet::new(Origin::UserAgent, &agent),
            SourcedSheet::new(Origin::Author, &author),
        ];
        let styles = resolve(&document, &sheets, &MediaContext::default());
        let tree = build(&document, &styles);
        let marker = tree.ids().find(|id| tree.is_marker(*id)).expect("a marker");
        let colour = tree
            .nearest_style(&styles, marker)
            .and_then(|style| style.color("color"))
            .map(alo_value::Rgba::to_rgba8);
        assert_eq!(colour, Some((0xb4, 0x53, 0x2a, 255)));
    }

    #[test]
    fn a_placeholder_is_set_in_its_own_style_and_says_so() {
        let document = parse_document("<input class=f placeholder=hint>");
        let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let author = parse_stylesheet(".f { color: #102a43 } .f::placeholder { color: #7a6f62 }");
        let sheets = [
            SourcedSheet::new(Origin::UserAgent, &agent),
            SourcedSheet::new(Origin::Author, &author),
        ];
        let styles = resolve(&document, &sheets, &MediaContext::default());
        let tree = build(&document, &styles);
        let hint = tree
            .ids()
            .find(|id| tree.is_placeholder(*id))
            .expect("a hint");
        let colour = tree
            .nearest_style(&styles, hint)
            .and_then(|style| style.color("color"))
            .map(alo_value::Rgba::to_rgba8);
        assert_eq!(colour, Some((0x7a, 0x6f, 0x62, 255)));
        assert!(tree.to_outline().contains("text \"hint\" · ::placeholder"));
    }

    /// The box a `<fieldset>` made, found by what it means.
    fn fieldset_of(tree: &BoxTree) -> BoxId {
        let root = tree.root().expect("a root");
        core::iter::once(root)
            .chain(tree.descendants(root))
            .find(|id| {
                tree.get(*id)
                    .is_some_and(|node| node.semantics.role.to_string().starts_with("group"))
            })
            .expect("a fieldset")
    }

    #[test]
    fn a_fieldset_says_which_legend_it_shows_in_its_border() {
        let tree = boxes("<fieldset><legend>Size</legend><p>one</p></fieldset>", "");
        let fieldset = fieldset_of(&tree);
        let legend = tree.rendered_legend(fieldset).expect("a rendered legend");
        assert_eq!(
            tree.children(fieldset).next(),
            Some(legend),
            "and it is the first child, which is where it is drawn",
        );
        assert_eq!(
            tree.get(legend).and_then(BoxNode::text),
            None,
            "the legend's box, not its text",
        );
    }

    #[test]
    fn a_legend_written_after_something_else_is_still_shown_first() {
        // HTML renders a fieldset's *first legend* at the top whatever comes
        // before it in the document, which is why hoisting it is the box
        // tree's job rather than something layout could infer.
        let tree = boxes(
            "<fieldset><p>one</p><legend>Size</legend><p>two</p></fieldset>",
            "",
        );
        let fieldset = fieldset_of(&tree);
        let legend = tree.rendered_legend(fieldset).expect("a rendered legend");
        assert_eq!(tree.children(fieldset).next(), Some(legend));
        assert_eq!(
            tree.children(fieldset).count(),
            3,
            "and nothing else moved: the two paragraphs are still there",
        );
    }

    #[test]
    fn only_the_first_legend_of_a_fieldset_is_the_one_it_shows() {
        let tree = boxes(
            "<fieldset><legend>Size</legend><legend>Again</legend></fieldset>",
            "",
        );
        let fieldset = fieldset_of(&tree);
        let legend = tree.rendered_legend(fieldset).expect("a rendered legend");
        assert_eq!(
            tree.children(fieldset).next(),
            Some(legend),
            "the second legend is an ordinary block",
        );
        assert_eq!(tree.legends().count(), 1);
    }

    #[test]
    fn a_br_is_a_box_that_ends_its_line() {
        assert_eq!(
            body_outline("<p>one<br>two</p>", ""),
            "block flow · generic\n  \
             block flow · paragraph\n    \
             text \"one\"\n    \
             inline flow · generic · line break\n    \
             text \"two\"\n",
        );
        let tree = boxes("<p>one<br><span>two</span></p>", "");
        let breaks: Vec<BoxId> = tree.ids().filter(|id| tree.is_forced_break(*id)).collect();
        assert_eq!(breaks.len(), 1, "the <br> and nothing else");
    }

    #[test]
    fn a_checkbox_and_a_radio_stand_on_their_border_edge() {
        let tree = boxes(
            "<p>a<input type=checkbox><input type=radio><input><span>b</span></p>",
            "",
        );
        let standing: Vec<BoxId> = tree
            .ids()
            .filter(|id| tree.stands_on_border_edge(*id))
            .collect();
        assert_eq!(
            standing.len(),
            2,
            "the checkbox and the radio, and nothing else"
        );
    }

    #[test]
    fn a_br_the_author_hid_or_made_a_block_is_no_break() {
        let hidden = boxes("<p>one<br>two</p>", "br { display: none }");
        assert!(!hidden.ids().any(|id| hidden.is_forced_break(id)));
        let block = boxes("<p>one<br>two</p>", "br { display: block }");
        assert!(!block.ids().any(|id| block.is_forced_break(id)));
    }

    #[test]
    fn a_fieldset_the_author_arranged_has_no_legend_in_its_border() {
        // An author who made the fieldset a flex container arranged its
        // children themselves, and lifting one of them into the border would
        // be this engine overruling a layout somebody wrote.
        let tree = boxes(
            "<fieldset id=f><legend>Size</legend><p>one</p></fieldset>",
            "#f { display: flex }",
        );
        assert_eq!(tree.legends().count(), 0);
        assert_eq!(tree.rendered_legend(fieldset_of(&tree)), None);
    }

    #[test]
    fn a_legend_that_generates_no_box_leaves_the_border_whole() {
        let tree = boxes(
            "<fieldset><legend>Size</legend><p>one</p></fieldset>",
            "legend { display: none }",
        );
        assert_eq!(tree.rendered_legend(fieldset_of(&tree)), None);
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
