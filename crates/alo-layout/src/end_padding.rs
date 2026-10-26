/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a scroll container's **inline-end padding** is added to, when it
//! works out how far it scrolls across (queue item 674).
//!
//! CSS Overflow 3 § 2.2 extends a scroll container's scrollable overflow by
//! its end padding, and `taffy` adds it to everything the box holds: its
//! children's boxes and whatever spills out of them alike. Chrome 155 adds
//! it only to the box's **own** content — its lines and its children's
//! boxes — and takes a child's spill as it is. Measured headless, three runs
//! alike, a `div` 80 wide with `overflow: auto` and 2 px of padding, in
//! `DejaVu Sans Mono`:
//!
//! - holding `Ring Ring Ring` (134.86 px) on its own line, `scrollWidth` is
//!   139: its left padding, its line and its right padding;
//! - holding a `div` that holds that line, 137: the child's box is 80 wide,
//!   and the line spilling out of it is not padded; with 10 px of padding,
//!   155 and 145;
//! - holding a `div` that holds a `div` 130 wide, 132, where one 130 wide
//!   of its own gives 134.
//!
//! A `<textarea>` is the second of these. Chrome lays its text out in an
//! inner box its author cannot reach, as wide as its content box, and the
//! lines spill out of that: a textarea 80 wide with 2 px of padding holding
//! `Ring Ring Ring` under `white-space: nowrap` has a `scrollWidth` of 137,
//! 145 with 10 px, and 137 with 2 px on the left and 10 on the right —
//! where this engine, adding the padding to the spill, said 138.86 and
//! 146.86. Down, a textarea's inner box is as tall as its lines in Chrome,
//! so the padding below is added to them, which is what `taffy` already
//! gives.
//!
//! [`text_areas`] corrects the textarea, the one box whose inner box this
//! engine builds itself (`alo_box::Purpose::Control`). Every other box keeps
//! `taffy`'s answer, and a `div` holding a `div` whose line spills still
//! scrolls by its padding further than Chrome's: queue item 675.

use crate::geometry::Size;
use crate::tree::BoxGeometry;
use alo_box::{BoxId, BoxTree};
use std::collections::BTreeMap;

/// Take a `<textarea>`'s inline-end padding off the lines that spill out of
/// its inner box, leaving it on the inner box itself; see the module's own
/// documentation. `holds_lines` says which box is a textarea's inner box,
/// and `geometry` holds the boxes of the subtree just laid out.
pub(crate) fn text_areas(
    boxes: &BoxTree,
    geometry: &mut BTreeMap<BoxId, BoxGeometry>,
    holds_lines: impl Fn(BoxId) -> bool,
) {
    // Only the boxes this layout placed: a subtree is laid out on its own
    // for each atomic inline in a line, and walking every box of the
    // document each time made a page of millions of pictures quadratic.
    let pairs: Vec<(BoxId, BoxId)> = geometry
        .keys()
        .copied()
        .flat_map(|outer| {
            boxes
                .children(outer)
                .filter(|inner| holds_lines(*inner))
                .map(move |inner| (outer, inner))
        })
        .collect();
    for (text_area, inner) in pairs {
        let Some(held) = geometry.get(&inner).copied() else {
            continue;
        };
        let Some(outer) = geometry.get_mut(&text_area) else {
            continue;
        };
        outer.reach = Size::new(reach_across(*outer, held), outer.reach.height);
    }
}

/// How far a textarea's content reaches across from its padding box's left,
/// `inner` being its inner box: the inner box with the padding after it, or
/// the lines spilling out of it, whichever is further.
fn reach_across(outer: BoxGeometry, inner: BoxGeometry) -> f32 {
    let from = outer.padding_box().origin.x;
    let padded = inner.border_box.right() + outer.padding.right - from;
    let spilled = inner.padding_box().origin.x + inner.reach.width - from;
    padded.max(spilled).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Edges, Point, Rect};

    fn geometry(
        origin: Point,
        size: Size,
        border: f32,
        padding: Edges,
        reach: Size,
    ) -> BoxGeometry {
        BoxGeometry {
            border_box: Rect { origin, size },
            border: Edges::all(border),
            padding,
            reach,
            ..BoxGeometry::default()
        }
    }

    /// A textarea 80 wide inside, 2 px of padding on the left and 10 on the
    /// right, and a border of 1, at (8, 8).
    fn text_area() -> BoxGeometry {
        let padding = Edges {
            top: 2.0,
            right: 10.0,
            bottom: 2.0,
            left: 2.0,
        };
        geometry(
            Point::new(8.0, 8.0),
            Size::new(94.0, 56.0),
            1.0,
            padding,
            Size::new(146.859_38, 64.0),
        )
    }

    /// Its inner box, filling its content box, its lines reaching `across`.
    fn inner(across: f32) -> BoxGeometry {
        geometry(
            Point::new(11.0, 11.0),
            Size::new(80.0, 50.0),
            0.0,
            Edges::all(0.0),
            Size::new(across, 60.0),
        )
    }

    #[test]
    fn lines_spilling_out_of_the_inner_box_are_not_padded() {
        // Chrome 155's `scrollWidth`: 137, its left padding and the line.
        let across = reach_across(text_area(), inner(134.859_38));
        assert!((across - 136.859_38).abs() < 1e-3, "{across}");
    }

    #[test]
    fn the_inner_box_itself_is_padded() {
        // Lines that fit: the inner box, 2 px in, and 10 px after it.
        let across = reach_across(text_area(), inner(40.0));
        assert!((across - 92.0).abs() < 1e-3, "{across}");
    }
}
