//! Where every box ended up, in numbers.
//!
//! Positions are **on the page**, not relative to a parent. A caller asking
//! where a box is nearly always wants the page, and a caller that wants the
//! parent-relative offset can subtract — whereas the other way round means
//! walking up the tree, which is how "where is this" becomes an O(depth)
//! question inside a loop.
//!
//! Every number is a CSS pixel.

use crate::geometry::{Edges, Rect, Size};
use alo_box::BoxId;
use alo_css::StyleIssue;
use core::fmt;
use core::fmt::Write as _;
use std::collections::BTreeMap;

/// Where one box is and how big it is.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BoxGeometry {
    /// The box including its padding and border, positioned on the page.
    ///
    /// This is the rectangle a background is painted in and the one a person
    /// would point at, which is why it is the one named without qualification.
    pub border_box: Rect,
    /// How thick the border is on each side.
    pub border: Edges,
    /// How much padding there is inside the border.
    pub padding: Edges,
    /// How much margin there is outside the box.
    pub margin: Edges,
    /// How big the content inside is, including whatever spills out of the box
    /// — what a scrollbar would be sized against.
    pub scrollable: Size,
}

impl BoxGeometry {
    /// The rectangle inside the border, where padding starts.
    pub fn padding_box(self) -> Rect {
        self.border_box.shrunk_by(self.border)
    }

    /// The rectangle the content sits in, inside the padding.
    pub fn content_box(self) -> Rect {
        self.padding_box().shrunk_by(self.padding)
    }

    /// Whether the content is larger than the box that holds it.
    pub fn overflows(self) -> bool {
        let content = self.content_box().size;
        self.scrollable.width > content.width || self.scrollable.height > content.height
    }
}

impl fmt::Display for BoxGeometry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.border_box)
    }
}

/// Where every box of one document ended up.
#[derive(Debug, Clone, Default)]
pub struct LayoutTree {
    geometry: BTreeMap<BoxId, BoxGeometry>,
    issues: Vec<StyleIssue>,
    viewport: Size,
}

impl LayoutTree {
    /// Where one box is, or [`None`] for a box that was not laid out.
    pub fn get(&self, id: BoxId) -> Option<BoxGeometry> {
        self.geometry.get(&id).copied()
    }

    /// The rectangle a box occupies on the page, border included.
    pub fn border_box(&self, id: BoxId) -> Option<Rect> {
        Some(self.get(id)?.border_box)
    }

    /// Everything layout could not do exactly, with the value that caused it.
    pub fn issues(&self) -> &[StyleIssue] {
        &self.issues
    }

    /// The window this layout was done for.
    pub fn viewport(&self) -> Size {
        self.viewport
    }

    /// How many boxes were laid out.
    pub fn len(&self) -> usize {
        self.geometry.len()
    }

    /// Whether nothing was laid out.
    pub fn is_empty(&self) -> bool {
        self.geometry.is_empty()
    }

    /// Every box, by id, in a stable order.
    pub fn iter(&self) -> impl Iterator<Item = (BoxId, BoxGeometry)> + '_ {
        self.geometry.iter().map(|(id, geometry)| (*id, *geometry))
    }

    pub(crate) fn from_parts(
        geometry: BTreeMap<BoxId, BoxGeometry>,
        issues: Vec<StyleIssue>,
        viewport: Size,
    ) -> Self {
        Self {
            geometry,
            issues,
            viewport,
        }
    }

    /// The layout as one line per box: what it is, and where it ended up.
    ///
    /// This is what a test asserts on. `CLAUDE.md` asks for a **layout
    /// assertion in numbers** for anything that positions or sizes, and a
    /// whole tree written out is the strongest form of one: a change that
    /// moves a box shows which box and by how much, which no assertion on a
    /// single rectangle does.
    pub fn to_outline(&self, boxes: &alo_box::BoxTree) -> String {
        let mut out = String::new();
        if let Some(root) = boxes.root() {
            self.write_outline(boxes, root, 0, &mut out);
        }
        out
    }

    fn write_outline(
        &self,
        boxes: &alo_box::BoxTree,
        id: alo_box::BoxId,
        depth: usize,
        out: &mut String,
    ) {
        let Some(node) = boxes.get(id) else { return };
        for _ in 0..depth {
            out.push_str("  ");
        }
        let description = match &node.kind {
            alo_box::BoxKind::Element { display, .. } => {
                format!("{display} · {}", node.semantics)
            }
            alo_box::BoxKind::Text { text, .. } => format!("text {text:?}"),
            alo_box::BoxKind::Anonymous { .. } => "anonymous".to_owned(),
        };
        match self.get(id) {
            Some(geometry) => {
                let _ = writeln!(out, "{description} → {}", geometry.border_box);
            }
            None => {
                let _ = writeln!(out, "{description} → not laid out");
            }
        }
        for child in node.children.clone() {
            self.write_outline(boxes, child, depth + 1, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    fn geometry() -> BoxGeometry {
        BoxGeometry {
            border_box: Rect::new(10.0, 20.0, 100.0, 60.0),
            border: Edges::all(2.0),
            padding: Edges::all(8.0),
            margin: Edges::all(4.0),
            scrollable: Size::new(80.0, 40.0),
        }
    }

    #[test]
    fn the_boxes_nest_inside_each_other_in_the_order_css_says() {
        let geometry = geometry();
        assert_eq!(geometry.border_box, Rect::new(10.0, 20.0, 100.0, 60.0));
        assert_eq!(
            geometry.padding_box(),
            Rect::new(12.0, 22.0, 96.0, 56.0),
            "inside the border",
        );
        assert_eq!(
            geometry.content_box(),
            Rect::new(20.0, 30.0, 80.0, 40.0),
            "and then inside the padding",
        );
    }

    #[test]
    fn a_box_says_whether_its_content_spills_out_of_it() {
        let fits = geometry();
        assert!(!fits.overflows(), "80×40 of content in 80×40 of room");

        let spills = BoxGeometry {
            scrollable: Size::new(200.0, 40.0),
            ..geometry()
        };
        assert!(spills.overflows());
    }

    #[test]
    fn a_box_that_was_not_laid_out_says_so_rather_than_reporting_zero() {
        let tree = LayoutTree::default();
        assert_eq!(tree.get(alo_box::BoxId::from_index_for_tests(0)), None);
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.viewport(), Size::ZERO);
    }

    #[test]
    fn a_position_is_on_the_page_rather_than_relative_to_a_parent() {
        // The contract this file states, asserted so that a change to it
        // breaks a test rather than a caller.
        let geometry = geometry();
        assert_eq!(geometry.border_box.origin, Point::new(10.0, 20.0));
        assert_eq!(geometry.to_string(), "100×60 at (10, 20)");
    }
}
