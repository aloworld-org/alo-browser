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
//! One box with no line still has a line's baseline: a **one-line field with
//! nothing typed in it** stands where its text will, the baseline of a line
//! holding only its strut, so that it does not drop when somebody types. The
//! engine records that line for it as it records any other; browsers agree on
//! it, and an empty `<input>` beside a label is the commonest thing on a form.
//! A button and a `<textarea>` with nothing in them get no such line.
//!
//! [`crate::engine`] lays out the box's subtree, records the baseline of the
//! last line of every inline formatting context in it, and asks [`last_line`]
//! which of those is the box's. `None` is the answer "its bottom margin edge",
//! and the engine measures that itself, because only it knows the margins.
//!
//! # What is refused, and why that is the old answer
//!
//! The search walks down through the box's in-flow block children, last one
//! first, and stops at the first line it finds. A child whose baseline is not
//! a line of its own — a **flex or grid container**, whose baseline CSS takes
//! from its items, or a **scroll container**, whose baseline browsers do not
//! agree on — ends the search with no answer, and the whole box stands on its
//! bottom margin edge as every atomic box did before this file existed. Taking
//! an earlier line from above such a child would be a guess that looks like a
//! rule; queue a page that needs it and it gets its own item.

use crate::engine;
use crate::keyword::{Overflow, Positioning};
use crate::style;
use crate::tree::BoxGeometry;
use alo_box::{BoxId, BoxTree, Inside};
use alo_style::StyleTree;
use std::collections::BTreeMap;

/// How far below the top of `root`'s border box its baseline sits, or `None`
/// when it stands on its bottom margin edge.
///
/// `lines` holds, for each inline formatting context laid out under `root`,
/// where the baseline of its last line ended up, on the same axis as
/// `geometry`.
pub(crate) fn last_line(
    boxes: &BoxTree,
    styles: &StyleTree,
    root: BoxId,
    lines: &BTreeMap<BoxId, f32>,
    geometry: &BTreeMap<BoxId, BoxGeometry>,
) -> Option<f32> {
    let top = geometry.get(&root)?.border_box.origin.y;
    match search(boxes, styles, root, lines) {
        Found::Line(y) => Some(y - top),
        Found::Nothing | Found::Refused => None,
    }
}

/// What looking inside one box turned up.
enum Found {
    /// A line, with its baseline here.
    Line(f32),
    /// No line in normal flow: look at the box before it.
    Nothing,
    /// A box whose baseline this engine does not work out: stop looking.
    Refused,
}

fn search(boxes: &BoxTree, styles: &StyleTree, id: BoxId, lines: &BTreeMap<BoxId, f32>) -> Found {
    let Some(node) = boxes.get(id) else {
        return Found::Nothing;
    };
    // A picture or an SVG is a rectangle with no lines in it.
    if boxes.natural_size(id).is_some() {
        return Found::Nothing;
    }
    let held = node
        .kind
        .node()
        .and_then(|source| styles.get(source))
        // Every issue in this style was already recorded when the box was
        // built; reading it again here is not a second finding.
        .map(|computed| style::read(computed, &mut Vec::new()));
    if let Some(held) = &held
        && (held.overflow.horizontal != Overflow::Visible
            || held.overflow.vertical != Overflow::Visible)
    {
        return Found::Refused;
    }
    if !matches!(node.kind.inside(), Inside::Flow | Inside::FlowRoot) {
        // A flex or grid container's baseline is its items', which is not
        // this search. Refused, and said so in the module comment.
        return Found::Refused;
    }
    // An empty one-line field holds no inline content, and its line is
    // recorded all the same.
    if engine::is_inline_formatting_context(boxes, id) || lines.contains_key(&id) {
        // Its lines are recorded if it has any; what is inside them belongs
        // to the line, not to a search for one.
        return lines.get(&id).map_or(Found::Nothing, |y| Found::Line(*y));
    }
    for child in node.children.iter().rev() {
        let out_of_flow = boxes
            .get(*child)
            .and_then(|held| held.kind.node())
            .and_then(|source| styles.get(source))
            .is_some_and(|computed| {
                style::read(computed, &mut Vec::new()).position == Positioning::Absolute
            });
        if out_of_flow {
            continue;
        }
        match search(boxes, styles, *child, lines) {
            Found::Nothing => {}
            answer => return answer,
        }
    }
    Found::Nothing
}
