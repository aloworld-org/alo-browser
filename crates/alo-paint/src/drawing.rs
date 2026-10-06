/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A drawing: what a box holds that is shapes rather than boxes.
//!
//! An `<svg>` is one replaced box, and what is inside it is not boxes but a
//! list of shapes (ADR 0022 §§ 1, 2). `alo-svg` makes that list after layout,
//! and paint is handed it **by box**, beside the box's picture — so paint draws
//! a list of paths and never learns that they came from SVG. That is the seam
//! that keeps a second *drawing* model from becoming a second *tree*.
//!
//! The vocabulary is paint's because the thing being described is paint's: a
//! path, what fills it, and which parts are faded together. It is small on
//! purpose, and a new kind of paint is a new word here rather than a new seam.
//!
//! **A stroke is a fill** (item 273). Its maker outlines it in the shape's own
//! coordinates with [`crate::raster::outline`] — before the shape's transform,
//! because a stroke squashed by a transform must be squashed with it — and
//! hands over the outline, filled by the non-zero rule. Paint therefore draws
//! one thing, and a stroke and a fill of the same colour can never disagree
//! about anti-aliasing.
//!
//! # Where a drawing is
//!
//! In the box's own coordinates: CSS pixels from the top-left corner of its
//! **content box**, with every SVG transform already applied. Paint moves it to
//! wherever layout put the box, and under whatever CSS transform the box has,
//! exactly as it moves a picture.

use crate::fill_rule::FillRule;
use crate::path::Path;
use alo_value::Rgba;

/// One step of a drawing.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawingItem {
    /// A shape, filled with one colour.
    Fill {
        /// What to fill, in the box's coordinates.
        path: Path,
        /// What with, with `fill-opacity` already in its alpha.
        color: Rgba,
        /// What counts as inside.
        rule: FillRule,
    },
    /// Everything until the matching [`DrawingItem::PopGroup`] is drawn
    /// together and faded once — what `opacity` on a group or a shape is.
    PushGroup {
        /// How much of the group reaches the page, from nothing to one.
        opacity: f32,
    },
    /// The end of the innermost group.
    PopGroup,
}

/// Everything one box draws inside itself, in the order to draw it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Drawing {
    items: Vec<DrawingItem>,
}

impl Drawing {
    /// A drawing with nothing in it.
    pub fn new() -> Self {
        Self::default()
    }

    /// The items, in paint order.
    pub fn items(&self) -> &[DrawingItem] {
        &self.items
    }

    /// How many items there are.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether there is nothing to draw.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Add an item to the end.
    pub fn push(&mut self, item: DrawingItem) {
        self.items.push(item);
    }

    /// Take the last item back off, for a maker that opened something and
    /// then found it held nothing.
    pub fn pop(&mut self) -> Option<DrawingItem> {
        self.items.pop()
    }

    /// Whether anything in it would put a pixel on a page.
    ///
    /// A drawing of groups with nothing filled in them, or of invisible fills,
    /// is nothing — and a box that draws nothing should not put a clip and a
    /// group into the display list to say so.
    pub fn draws_anything(&self) -> bool {
        self.items.iter().any(|item| match item {
            DrawingItem::Fill { path, color, .. } => !path.is_empty() && !color.is_invisible(),
            DrawingItem::PushGroup { .. } | DrawingItem::PopGroup => false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_drawing_draws_nothing() {
        let drawing = Drawing::new();
        assert!(drawing.is_empty());
        assert_eq!(drawing.len(), 0);
        assert!(!drawing.draws_anything());
    }

    #[test]
    fn groups_and_invisible_fills_are_not_anything_to_draw() {
        let mut drawing = Drawing::new();
        drawing.push(DrawingItem::PushGroup { opacity: 0.5 });
        drawing.push(DrawingItem::Fill {
            path: Path::rectangle(0.0, 0.0, 4.0, 4.0),
            color: Rgba::TRANSPARENT,
            rule: FillRule::NonZero,
        });
        drawing.push(DrawingItem::PopGroup);
        assert_eq!(drawing.len(), 3);
        assert_eq!(drawing.pop(), Some(DrawingItem::PopGroup));
        drawing.push(DrawingItem::PopGroup);
        assert!(!drawing.draws_anything());

        drawing.push(DrawingItem::Fill {
            path: Path::rectangle(0.0, 0.0, 4.0, 4.0),
            color: Rgba::BLACK,
            rule: FillRule::EvenOdd,
        });
        assert!(drawing.draws_anything());
        assert!(matches!(
            drawing.items().last(),
            Some(DrawingItem::Fill {
                rule: FillRule::EvenOdd,
                ..
            })
        ));
    }
}
