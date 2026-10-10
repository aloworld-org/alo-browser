/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where an atomic inline-level box stands on the line it sits in.
//!
//! CSS puts the baseline of an `inline-block` on **the baseline of its last
//! line box in normal flow** — a button on its label's, a text field on its
//! value's, an inline-block of text on its last line's — so a button beside a
//! sentence reads as part of it rather than standing a descent above it. Only
//! when there is no such line, or the box's own `overflow` is not `visible`,
//! is the baseline its **bottom margin edge**, which is also where a picture
//! and an SVG stand: they have no lines at all.
//!
//! An **`inline-flex`** box stands on its *first* baseline, which CSS
//! Flexbox 1 § 8.5 takes from its items: the first item's own first
//! baseline — the first line found going down into it, first child first —
//! or, when that item has no line in it, its bottom border edge. A flex
//! container with no item has no baseline, and stands on its bottom margin
//! edge like an empty `inline-block`. A link made `inline-flex` to centre its
//! text in a tall box is how alo Sites draws a menu, and standing it on its
//! bottom edge put a descent under every item in the menu.
//!
//! One box with no line still has a line's baseline: a **one-line field with
//! nothing typed in it** stands where its text will, the baseline of a line
//! holding only its strut, so that it does not drop when somebody types. The
//! engine records that line for it as it records any other; browsers agree on
//! it, and an empty `<input>` beside a label is the commonest thing on a form.
//! A button and a `<textarea>` with nothing in them get no such line.
//!
//! [`crate::engine`] lays out the box's subtree, records the baselines of the
//! first and last lines of every inline formatting context in it, and asks
//! [`of_atomic`] which of those is the box's. `None` is the answer "its bottom
//! margin edge", and the engine measures that itself, because only it knows
//! the margins.
//!
//! # What is refused, and why that is the old answer
//!
//! A box whose baseline this engine does not work out ends the search with no
//! answer, and the whole box stands on its bottom margin edge as every atomic
//! box did before this file existed. Taking some other line instead would be
//! a guess that looks like a rule; queue a page that needs it and it gets its
//! own item. Refused:
//!
//! - a **grid container**, whose baseline CSS takes from its first row;
//! - a **flex container met looking for a last line** — an `inline-block`
//!   whose last child is one — whose last baseline is its last item's;
//! - a **row of flex items any of which is aligned on its baseline**, where
//!   the container's baseline is the line those items share (item 396);
//! - a **scroll container**, whose baseline browsers do not agree on.
//!
//! `order` is not read by this engine, so "the first item" is the first in
//! document order, which is where `order` would leave it.

use crate::engine;
use crate::keyword::{Alignment, FlexDirection, Overflow, Positioning};
use crate::style::{self, LayoutStyle};
use crate::tree::BoxGeometry;
use alo_box::{BoxId, BoxTree, Inside};
use alo_style::StyleTree;
use std::collections::BTreeMap;

/// Where the baselines of one inline formatting context's lines stand, on the
/// same axis as the geometry they were placed in.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Lines {
    /// The first line's baseline.
    pub(crate) first: f32,
    /// The last line's, which is the first's when there is one line.
    pub(crate) last: f32,
}

impl Lines {
    /// One line, which is first and last at once.
    pub(crate) fn one(baseline: f32) -> Self {
        Self {
            first: baseline,
            last: baseline,
        }
    }
}

/// How far below the top of `root`'s border box its baseline sits, or `None`
/// when it stands on its bottom margin edge.
///
/// `lines` holds, for each inline formatting context laid out under `root`,
/// where the baselines of its first and last lines ended up, on the same axis
/// as `geometry`.
pub(crate) fn of_atomic(
    boxes: &BoxTree,
    styles: &StyleTree,
    root: BoxId,
    lines: &BTreeMap<BoxId, Lines>,
    geometry: &BTreeMap<BoxId, BoxGeometry>,
) -> Option<f32> {
    let top = geometry.get(&root)?.border_box.origin.y;
    // An `inline-flex` box aligns on its first baseline; an `inline-block`,
    // for the reason CSS 2 gave and browsers kept, on its last line's.
    let which = match boxes.get(root).map(|node| node.kind.inside()) {
        Some(Inside::Flex) => Which::First,
        _ => Which::Last,
    };
    let laid_out = LaidOut {
        boxes,
        styles,
        lines,
        geometry,
    };
    match laid_out.search(root, which) {
        Found::Line(y) => Some(y - top),
        Found::Nothing | Found::Refused => None,
    }
}

/// Which of a box's baselines is being looked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    /// The first line's, looking down from the top.
    First,
    /// The last line's, looking up from the bottom.
    Last,
}

/// What looking inside one box turned up.
enum Found {
    /// A line, with its baseline here.
    Line(f32),
    /// No line in normal flow: look at the next box.
    Nothing,
    /// A box whose baseline this engine does not work out: stop looking.
    Refused,
}

/// A subtree once it has been laid out, which is what a search reads.
struct LaidOut<'a> {
    boxes: &'a BoxTree,
    styles: &'a StyleTree,
    lines: &'a BTreeMap<BoxId, Lines>,
    geometry: &'a BTreeMap<BoxId, BoxGeometry>,
}

impl LaidOut<'_> {
    fn search(&self, id: BoxId, which: Which) -> Found {
        let Some(node) = self.boxes.get(id) else {
            return Found::Nothing;
        };
        // A picture or an SVG is a rectangle with no lines in it.
        if self.boxes.natural_size(id).is_some() {
            return Found::Nothing;
        }
        let held = self.style_of(id);
        if let Some(held) = &held
            && (held.overflow.horizontal != Overflow::Visible
                || held.overflow.vertical != Overflow::Visible)
        {
            return Found::Refused;
        }
        match (node.kind.inside(), which) {
            (Inside::Flow | Inside::FlowRoot, _) => {}
            (Inside::Flex, Which::First) => return self.first_item(id, held.as_ref()),
            // A grid's baseline is its first row's, and a flex container's
            // last is its last item's: neither is this search. Refused, and
            // said so in the module comment.
            (Inside::Flex, Which::Last) | (Inside::Grid, _) => return Found::Refused,
        }
        // An empty one-line field holds no inline content, and its line is
        // recorded all the same.
        if engine::is_inline_formatting_context(self.boxes, id) || self.lines.contains_key(&id) {
            // Its lines are recorded if it has any; what is inside them
            // belongs to the line, not to a search for one.
            return self.lines.get(&id).map_or(Found::Nothing, |lines| {
                Found::Line(match which {
                    Which::First => lines.first,
                    Which::Last => lines.last,
                })
            });
        }
        let mut children = self.in_flow_children(id);
        if which == Which::Last {
            children.reverse();
        }
        for child in children {
            match self.search(child, which) {
                Found::Nothing => {}
                answer => return answer,
            }
        }
        Found::Nothing
    }

    /// A flex container's first baseline: its first item's (CSS Flexbox 1
    /// § 8.5), or none when it has no item.
    fn first_item(&self, id: BoxId, held: Option<&LayoutStyle>) -> Found {
        let items = self.in_flow_children(id);
        let along_a_row = held.is_none_or(|held| {
            matches!(
                held.flex.direction,
                FlexDirection::Row | FlexDirection::RowReverse
            )
        });
        let container_aligns = held.map_or(Alignment::Normal, |held| held.align.align_items);
        if along_a_row
            && items
                .iter()
                .any(|item| self.aligns_on_its_baseline(*item, container_aligns))
        {
            return Found::Refused;
        }
        let Some(first) = items.first() else {
            return Found::Nothing;
        };
        match self.search(*first, Which::First) {
            // An item with no line in it has a baseline made from its border
            // box, along its bottom edge.
            Found::Nothing => self
                .geometry
                .get(first)
                .map_or(Found::Refused, |item| Found::Line(item.border_box.bottom())),
            answer => answer,
        }
    }

    /// Whether a flex item's `align-self`, or with none its container's
    /// `align-items`, lines it up on its baseline.
    fn aligns_on_its_baseline(&self, item: BoxId, container: Alignment) -> bool {
        let own = self
            .style_of(item)
            .map_or(Alignment::Normal, |held| held.align.align_self);
        match own {
            Alignment::Normal => container == Alignment::Baseline,
            other => other == Alignment::Baseline,
        }
    }

    /// The children that take part in this box's own layout: everything but
    /// an absolutely positioned box.
    fn in_flow_children(&self, id: BoxId) -> Vec<BoxId> {
        self.boxes
            .children(id)
            .filter(|child| {
                self.style_of(*child)
                    .is_none_or(|held| held.position != Positioning::Absolute)
            })
            .collect()
    }

    /// The layout style of a box an element made; `None` for a box nobody
    /// wrote.
    fn style_of(&self, id: BoxId) -> Option<LayoutStyle> {
        self.boxes
            .get(id)
            .and_then(|node| node.kind.node())
            .and_then(|source| self.styles.get(source))
            // Every issue in this style was already recorded when the box was
            // built; reading it again here is not a second finding.
            .map(|computed| style::read(computed, &mut Vec::new()))
    }
}
