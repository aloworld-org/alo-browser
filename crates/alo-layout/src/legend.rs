/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A fieldset's legend, and the band it sits in.
//!
//! Everywhere else in this crate a box's block-start border is a strip of
//! thickness along the top of its border box. A `<fieldset>` showing a legend
//! is the one place CSS asks for something else: the legend sits **in** that
//! border rather than under it, and the border is not drawn behind it. That is
//! what makes a group of controls look like a group with a name on it, and it
//! is the reason a fieldset is worth using at all.
//!
//! So a fieldset with a legend has a **band** where its block-start border
//! would be:
//!
//! ```text
//!         ┌── the band, as tall as the legend ──────────────┐
//!   ──────┘  Pizza Size  └───────────────────────────────────   ← the stroke,
//!   │                                                       │     through the
//!   │   the padding, and then what the fieldset holds        │     middle of it
//! ```
//!
//! The band replaces the border rather than adding to it: a fieldset is
//! exactly as tall as one whose legend were an ordinary first child would be,
//! *minus* the border's own thickness, which is what a browser does and what
//! keeps two fieldsets side by side lining up with each other.
//!
//! **This is the whole of it.** [`crate::engine`] gives such a fieldset no
//! block-start border for the layout run — there is a band there instead —
//! and calls [`raise`] afterwards, which moves the legend into the band and
//! records it. Paint reads the band off the geometry; nothing else in layout
//! knows a fieldset from any other block.

use crate::geometry::Rect;
use crate::style;
use crate::tree::BoxGeometry;
use alo_box::{BoxId, BoxTree};
use alo_css::StyleIssue;
use alo_style::StyleTree;
use std::collections::BTreeMap;

/// A block-start border with something sitting in it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    /// How tall the band is: the legend's margin box, or the border's own
    /// thickness when the legend is shorter than that.
    pub height: f32,
    /// How thick the border drawn through it is — the `border-top-width` the
    /// style asked for, which the layout run itself was not given.
    pub stroke: f32,
    /// The part of the band the border is **not** drawn in, as distances from
    /// the border box's left edge: where the legend's margin box starts, and
    /// where it ends.
    ///
    /// Relative rather than on the page, because it is a fact about this box
    /// rather than about where the page put it — and because a caller that has
    /// the box has its origin already.
    pub gap: (f32, f32),
}

impl Band {
    /// How far below the top of the border box the stroke is drawn.
    ///
    /// Centred in the band, which is what puts the line through the middle of
    /// the legend's words rather than along the top of them.
    pub fn inset(self) -> f32 {
        ((self.height - self.stroke) / 2.0).max(0.0)
    }
}

/// Move every fieldset's legend into its band, and record the band.
///
/// Called once a formatting context has been laid out and before anything is
/// placed inside it, so that the lines of text in a legend are laid out
/// against the position it ends up at rather than the one it was laid out in.
pub(crate) fn raise(
    boxes: &BoxTree,
    styles: &StyleTree,
    geometry: &mut BTreeMap<BoxId, BoxGeometry>,
    issues: &mut Vec<StyleIssue>,
) {
    for (fieldset, legend) in boxes.legends() {
        let (Some(field), Some(placed)) = (
            geometry.get(&fieldset).copied(),
            geometry.get(&legend).copied(),
        ) else {
            // A fieldset in another formatting context: this one's geometry
            // knows nothing about it, and the run that does will do it.
            continue;
        };
        // The layout ran with no block-start border, so the legend is one
        // padding down from the top of the fieldset. The band is where the
        // padding would have started, so that is how far it comes up.
        let raise_by = field.padding.top;
        for id in core::iter::once(legend).chain(boxes.descendants(legend)) {
            if let Some(held) = geometry.get_mut(&id) {
                held.border_box = moved_up(held.border_box, raise_by);
            }
        }

        let stroke = stroke_of(boxes, styles, fieldset, issues);
        let margin_box = Rect::new(
            placed.border_box.left() - placed.margin.left,
            placed.border_box.top() - placed.margin.top - raise_by,
            placed.border_box.size.width + placed.margin.left + placed.margin.right,
            placed.border_box.size.height + placed.margin.top + placed.margin.bottom,
        );
        if let Some(held) = geometry.get_mut(&fieldset) {
            held.band = Some(Band {
                height: margin_box.size.height.max(stroke),
                stroke,
                gap: (
                    margin_box.left() - field.border_box.left(),
                    margin_box.right() - field.border_box.left(),
                ),
            });
        }
    }
}

/// How thick a fieldset's block-start border is, from its style.
///
/// From the style rather than from the layout, and that is the point: the
/// layout was given a zero border there because the band stands in for it, so
/// the geometry no longer knows. A border width is never a percentage in CSS,
/// which is why there is no basis to resolve one against here.
fn stroke_of(
    boxes: &BoxTree,
    styles: &StyleTree,
    fieldset: BoxId,
    issues: &mut Vec<StyleIssue>,
) -> f32 {
    let Some(ours) = boxes
        .get(fieldset)
        .and_then(|node| node.kind.node())
        .and_then(|source| styles.get(source))
        .map(|computed| style::read(computed, issues))
    else {
        return 0.0;
    };
    ours.border.top.to_px(ours.metrics, 0.0).max(0.0)
}

fn moved_up(rect: Rect, by: f32) -> Rect {
    Rect::new(
        rect.left(),
        rect.top() - by,
        rect.size.width,
        rect.size.height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stroke_is_drawn_through_the_middle_of_the_band() {
        let band = Band {
            height: 20.0,
            stroke: 2.0,
            gap: (10.0, 60.0),
        };
        assert!(
            (band.inset() - 9.0).abs() < 0.0001,
            "nine above it and nine below: {}",
            band.inset(),
        );
    }

    #[test]
    fn a_border_thicker_than_the_band_starts_at_the_top_rather_than_above_it() {
        // The band is never shorter than the stroke, so this cannot happen
        // from a layout — but a negative inset would draw the border outside
        // the box, and "cannot happen" is not a reason to leave that open.
        let band = Band {
            height: 4.0,
            stroke: 12.0,
            gap: (0.0, 0.0),
        };
        assert!(band.inset().abs() < 0.0001);
    }
}
