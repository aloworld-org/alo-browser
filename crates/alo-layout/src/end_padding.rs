/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a scroll container's **end padding** is added to, when it works out
//! how far it scrolls across and down (queue items 674 and 675).
//!
//! CSS Overflow 3 § 2.2 extends a scroll container's scrollable overflow by
//! its end padding, and `taffy` adds it to everything the box holds: its
//! children's boxes and whatever spills out of them alike. Chrome adds it
//! only to the box's **own** content — its lines, and its in-flow
//! children's **margin boxes** — and takes the rest as it is: a child's
//! border box, what spills out of it, and a box positioned against the
//! container. Measured headless in Chrome 156, three runs alike, a box 80
//! wide and 50 tall with `overflow: auto` and 2 px of padding, in `DejaVu
//! Sans Mono`, a block, a flex and a grid container alike:
//!
//! - holding `Ring Ring Ring` (134.86 px) on its own line, `scrollWidth` is
//!   139: its left padding, its line and its right padding (Chrome 155,
//!   item 674);
//! - holding a child 60 wide whose line that is, 137: the line spilling
//!   out of the child is not padded — nor with 10 px of padding on the
//!   right (137), where the child's own box, padded, gives 72;
//! - holding a child holding a block 130 wide, 132, where a block 130 wide
//!   of its own gives 134;
//! - holding a child 20 tall holding a block 120 tall, `scrollHeight` 122:
//!   down is the same rule;
//! - holding a child 60 wide and 20 tall with 30 px of margin after it and
//!   60 below, 94 and 84: the margin box, padded — and 94 still for a
//!   child 0 tall with that margin, so an empty margin box counts;
//! - holding a child 100 wide with a margin of −20 after it, 102: its
//!   border box, not padded, past its margin box, padded (84);
//! - holding a box positioned against it, 120 by 70, 120 and 70;
//! - holding a child under `overflow: clip` or `auto`, only that child's
//!   box: what it cuts off reaches nothing.
//!
//! A `<textarea>` is the second of these. Chrome lays its text out in an
//! inner box its author cannot reach, as wide as its content box, and the
//! lines spill out of that across; down, the inner box is as tall as its
//! lines, so the padding below is added to them, which is what `taffy`
//! already gives. [`text_areas`] corrects the textarea, whose inner box this
//! engine builds itself (`alo_box::Purpose::Control`) and lays out as tall
//! as the textarea's content box.
//!
//! [`scroll_containers`] corrects every other scroll container holding a
//! child in flow. One holding only lines keeps `taffy`'s answer, which pads
//! them as Chrome does. Two things Chrome counts are not here, because what
//! they need is not in a box's geometry: a child moved by `position:
//! relative` is padded where it stood before it moved and taken as it is
//! where it was moved to (queue item 693), and a grid's tracks are padded
//! (queue item 694).

use crate::geometry::{Rect, Size};
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

/// A box a scroll container holds, and what of it the container's
/// scrolling area is made of.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Held {
    pub(crate) id: BoxId,
    /// Whether it is in the container's flow, so that its margin box is the
    /// container's own content and is padded; a box positioned against the
    /// container is not.
    pub(crate) in_flow: bool,
    /// Whether what spills out of it reaches the container, across and
    /// down: not out of a scroll container, nor across or down out of a box
    /// that clips that axis.
    pub(crate) spills: (bool, bool),
}

/// A scroll container and the boxes its scrolling area is made of.
#[derive(Clone, Debug)]
pub(crate) struct Container {
    pub(crate) id: BoxId,
    pub(crate) held: Vec<Held>,
}

/// Work out each container's reach as Chrome does — its children's margin
/// boxes padded, what they reach and what is positioned against it taken as
/// it is; see the module's own documentation. A container holding nothing
/// in flow keeps the reach it has.
pub(crate) fn scroll_containers(
    geometry: &mut BTreeMap<BoxId, BoxGeometry>,
    containers: &[Container],
) {
    for container in containers {
        if !container.held.iter().any(|held| held.in_flow) {
            continue;
        }
        let Some(outer) = geometry.get(&container.id).copied() else {
            continue;
        };
        let held = container
            .held
            .iter()
            .filter_map(|held| Some((geometry.get(&held.id).copied()?, *held)));
        let reach = reach_of(outer, held);
        if let Some(outer) = geometry.get_mut(&container.id) {
            outer.reach = reach;
        }
    }
}

/// How far a scroll container's content reaches across and down from its
/// padding box's top left, `held` being the boxes it holds with where they
/// were placed.
fn reach_of(outer: BoxGeometry, held: impl Iterator<Item = (BoxGeometry, Held)>) -> Size {
    let from = outer.padding_box().origin;
    let (mut across, mut down) = (0.0_f32, 0.0_f32);
    for (inner, held) in held {
        // Its border box and what spills out of it, taken as they are. Like
        // `taffy`, nothing is taken from a box neither of which reaches past
        // its top left, nor from one wholly before the scroll origin, which
        // nothing can be scrolled to.
        let border_box = inner.border_box;
        let reached = inner.padding_box().origin;
        let right = if held.spills.0 {
            border_box.right().max(reached.x + inner.reach.width)
        } else {
            border_box.right()
        };
        let bottom = if held.spills.1 {
            border_box.bottom().max(reached.y + inner.reach.height)
        } else {
            border_box.bottom()
        };
        let (right, bottom) = (right - from.x, bottom - from.y);
        if right > border_box.origin.x - from.x
            && bottom > border_box.origin.y - from.y
            && right > 0.0
            && bottom > 0.0
        {
            across = across.max(right);
            down = down.max(bottom);
        }
        // Its margin box, padded, even with no width or height: Chrome pads
        // a child 60 wide and 0 tall with 30 px of margin after it to 94.
        if held.in_flow {
            let margin_box = margin_box(inner);
            across = across.max(margin_box.right() - from.x + outer.padding.right);
            down = down.max(margin_box.bottom() - from.y + outer.padding.bottom);
        }
    }
    Size::new(across.max(0.0), down.max(0.0))
}

/// A box's margin box: its border box grown by its margins, a negative one
/// shrinking it.
fn margin_box(inner: BoxGeometry) -> Rect {
    let (border_box, margin) = (inner.border_box, inner.margin);
    Rect::new(
        border_box.origin.x - margin.left,
        border_box.origin.y - margin.top,
        border_box.size.width + margin.left + margin.right,
        border_box.size.height + margin.top + margin.bottom,
    )
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

    /// A scroll container 80 by 50 inside, with a border of 1 and 2 px of
    /// padding but `right` on the right, at (8, 8).
    fn container(right: f32) -> BoxGeometry {
        let padding = Edges {
            top: 2.0,
            right,
            bottom: 2.0,
            left: 2.0,
        };
        geometry(
            Point::new(8.0, 8.0),
            Size::new(83.0 + right, 56.0),
            1.0,
            padding,
            Size::new(84.0 + right, 54.0),
        )
    }

    /// A child `width` by `height` at the container's content box's top
    /// left, its own content reaching `reach` from its top left.
    fn child(width: f32, height: f32, reach: Size) -> BoxGeometry {
        geometry(
            Point::new(11.0, 11.0),
            Size::new(width, height),
            0.0,
            Edges::all(0.0),
            reach,
        )
    }

    fn in_flow(spills: bool) -> Held {
        Held {
            id: BoxId::from_index_for_tests(1),
            in_flow: true,
            spills: (spills, spills),
        }
    }

    fn near(ours: Size, across: f32, down: f32) -> bool {
        (ours.width - across).abs() < 1e-3 && (ours.height - down).abs() < 1e-3
    }

    #[test]
    fn what_spills_out_of_a_child_is_not_padded() {
        // Chrome 156: a child 60 wide whose line reaches 134.86 scrolls 137,
        // and 137 again with 10 px of padding on the right.
        for right in [2.0, 10.0] {
            let line = child(60.0, 20.0, Size::new(134.859_38, 19.0));
            let reach = reach_of(container(right), [(line, in_flow(true))].into_iter());
            assert!(near(reach, 136.859_38, 24.0), "{right}: {reach:?}");
        }
    }

    #[test]
    fn a_childs_box_is_padded() {
        // A block 130 wide of the container's own: 134, and 142 with 10.
        for right in [2.0, 10.0] {
            let wide = child(130.0, 10.0, Size::new(130.0, 10.0));
            let reach = reach_of(container(right), [(wide, in_flow(true))].into_iter());
            assert!(near(reach, 132.0 + right, 14.0), "{right}: {reach:?}");
        }
    }

    #[test]
    fn down_is_the_same_rule() {
        // A child 20 tall holding a block 120 tall scrolls 122, not 124.
        let tall = child(60.0, 20.0, Size::new(10.0, 120.0));
        let reach = reach_of(container(2.0), [(tall, in_flow(true))].into_iter());
        assert!(near(reach, 64.0, 122.0), "{reach:?}");
    }

    #[test]
    fn a_childs_margin_box_is_padded() {
        // 30 px of margin after a child 60 wide and 60 below one 20 tall:
        // 94 and 84 — and a child 0 tall is padded all the same.
        for height in [20.0, 0.0] {
            let mut spaced = child(60.0, height, Size::new(0.0, 0.0));
            spaced.margin = Edges {
                top: 0.0,
                right: 30.0,
                bottom: 60.0,
                left: 0.0,
            };
            let reach = reach_of(container(2.0), [(spaced, in_flow(true))].into_iter());
            assert!(near(reach, 94.0, 64.0 + height), "{height}: {reach:?}");
        }
    }

    #[test]
    fn a_negative_margin_leaves_the_border_box_unpadded() {
        // A child 100 wide with −20 after it: 102, its border box.
        let mut pulled = child(100.0, 20.0, Size::new(0.0, 0.0));
        pulled.margin.right = -20.0;
        let reach = reach_of(container(2.0), [(pulled, in_flow(true))].into_iter());
        assert!(near(reach, 102.0, 24.0), "{reach:?}");
    }

    #[test]
    fn a_child_that_cuts_off_its_content_counts_only_its_box() {
        let clipped = child(60.0, 20.0, Size::new(130.0, 20.0));
        let reach = reach_of(container(2.0), [(clipped, in_flow(false))].into_iter());
        assert!(near(reach, 64.0, 24.0), "{reach:?}");
    }

    #[test]
    fn a_box_positioned_against_the_container_is_not_padded() {
        // 120 by 70 at the padding box's top left: 120 and 70.
        let placed = geometry(
            Point::new(9.0, 9.0),
            Size::new(120.0, 70.0),
            0.0,
            Edges::all(0.0),
            Size::new(120.0, 70.0),
        );
        let positioned = Held {
            in_flow: false,
            ..in_flow(true)
        };
        let reach = reach_of(container(2.0), [(placed, positioned)].into_iter());
        assert!(near(reach, 120.0, 70.0), "{reach:?}");
    }

    #[test]
    fn a_container_holding_nothing_in_flow_keeps_its_reach() {
        let mut placed = BTreeMap::new();
        let (outer, inner) = (
            BoxId::from_index_for_tests(0),
            BoxId::from_index_for_tests(1),
        );
        placed.insert(outer, container(2.0));
        placed.insert(inner, child(130.0, 10.0, Size::new(130.0, 10.0)));
        let positioned = Held {
            in_flow: false,
            ..in_flow(true)
        };
        scroll_containers(
            &mut placed,
            &[Container {
                id: outer,
                held: vec![positioned],
            }],
        );
        let kept = placed.get(&outer).map(|held| held.reach);
        assert!(kept.is_some_and(|kept| near(kept, 86.0, 54.0)), "{kept:?}");
    }
}
