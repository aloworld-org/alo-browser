/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a box out of flow written in a line stands (queue item 354).
//!
//! `position: absolute` takes a box out of flow, and CSS blockifies it; it
//! is still not a block among blocks. The box tree leaves it among the
//! inline content it was written in, [`crate::inline`] gives it no room on
//! the line and says where it would have stood — an [`inline::Anchor`] —
//! and the layout engine sizes it against its containing block, as it does
//! every absolutely positioned box. Once the line is placed, this moves the
//! box to its anchor in each axis whose two insets are both `auto`: its
//! **static position** (CSS 2 § 10.3.7). An axis with an inset keeps the
//! place its containing block gave it.
//!
//! The box's parent holds lines and is a leaf of the engine's tree, so no
//! empty stand-in can be laid out where the box was, as one is for a box
//! among blocks; the line is the only thing that knows.

use crate::engine::Placed;
use crate::geometry::Point;
use crate::inline;
use alo_box::BoxTree;

/// Which axes of an absolutely positioned box have both their insets `auto`,
/// and so take its static position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Axes {
    /// Left and right are both `auto`.
    pub(crate) across: bool,
    /// Top and bottom are both `auto`.
    pub(crate) down: bool,
}

/// Move a box out of flow written in a line to where the line says it would
/// have stood, in each axis whose insets are both `auto`, with everything
/// already placed inside it (CSS 2 § 10.3.7). Its margin goes from there, as
/// a probe's does: alo Sites' `.sr-only` is `margin: -1px`, and Chrome 154
/// puts it a pixel up and to the left of the end of "Read article".
///
/// `origin` is where the line's formatting context starts on the page.
pub(crate) fn stand_at_static_position(
    boxes: &BoxTree,
    anchor: &inline::Anchor,
    origin: Point,
    out: &mut Placed,
) {
    let (Some(axes), Some(held)) = (
        out.in_line.get(&anchor.box_id).copied(),
        out.geometry.get(&anchor.box_id).copied(),
    ) else {
        return;
    };
    let at = held.border_box.origin;
    let by = Point::new(
        if axes.across {
            origin.x + anchor.at.x + held.margin.left - at.x
        } else {
            0.0
        },
        if axes.down {
            origin.y + anchor.at.y + held.margin.top - at.y
        } else {
            0.0
        },
    );
    for inner in core::iter::once(anchor.box_id).chain(boxes.descendants(anchor.box_id)) {
        if let Some(placed) = out.geometry.get_mut(&inner) {
            placed.border_box = placed.border_box.translated(by);
        }
        if let Some(pieces) = out.fragments.get_mut(&inner) {
            for piece in pieces {
                piece.rect = piece.rect.translated(by);
            }
        }
    }
}
