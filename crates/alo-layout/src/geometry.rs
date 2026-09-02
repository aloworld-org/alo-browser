//! Rectangles, in CSS pixels.
//!
//! These are ours rather than the layout engine's, for the reason ADR 0001
//! gives about every rented thing: `taffy` is a judgement call taken because
//! it gets us laying out sooner, and it is meant to be replaceable. A geometry
//! type from it in our public API would make replacing it a rewrite of
//! everything that reads a rectangle.
//!
//! Every number here is a **CSS pixel**, which is not necessarily a device
//! pixel: a screen at twice the density draws two of those for each of these.
//! That conversion belongs to paint, and keeping it out of layout is what lets
//! a layout be asserted as a number rather than as a number on a machine.

use core::fmt;

/// A point.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    /// Distance from the left edge.
    pub x: f32,
    /// Distance from the top edge.
    pub y: f32,
}

impl Point {
    /// The origin.
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    /// A point.
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

/// How big something is.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    /// Across.
    pub width: f32,
    /// Down.
    pub height: f32,
}

impl Size {
    /// Nothing at all.
    pub const ZERO: Size = Size {
        width: 0.0,
        height: 0.0,
    };

    /// A size.
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}×{}", self.width, self.height)
    }
}

/// A rectangle: where something is and how big it is.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Its top-left corner.
    pub origin: Point,
    /// Its size.
    pub size: Size,
}

impl Rect {
    /// A rectangle with nothing in it, at the origin.
    pub const ZERO: Rect = Rect {
        origin: Point::ZERO,
        size: Size::ZERO,
    };

    /// A rectangle, from its corner and its size.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    /// The left edge.
    pub fn left(self) -> f32 {
        self.origin.x
    }

    /// The top edge.
    pub fn top(self) -> f32 {
        self.origin.y
    }

    /// The right edge.
    pub fn right(self) -> f32 {
        self.origin.x + self.size.width
    }

    /// The bottom edge.
    pub fn bottom(self) -> f32 {
        self.origin.y + self.size.height
    }

    /// This rectangle moved by an offset — how a box's position relative to
    /// its parent becomes its position on the page.
    #[must_use]
    pub fn translated(self, by: Point) -> Self {
        Self {
            origin: Point::new(self.origin.x + by.x, self.origin.y + by.y),
            size: self.size,
        }
    }

    /// This rectangle with its edges pulled inwards.
    #[must_use]
    pub fn shrunk_by(self, edges: Edges) -> Self {
        Self {
            origin: Point::new(self.origin.x + edges.left, self.origin.y + edges.top),
            size: Size::new(
                (self.size.width - edges.left - edges.right).max(0.0),
                (self.size.height - edges.top - edges.bottom).max(0.0),
            ),
        }
    }

    /// Whether a point is inside this rectangle, counting the top and left
    /// edges and not the bottom and right.
    ///
    /// Half-open on purpose: two rectangles that share an edge should not both
    /// claim the same point, or a hit test would find two boxes where a person
    /// sees one.
    pub fn contains(self, point: Point) -> bool {
        point.x >= self.left()
            && point.x < self.right()
            && point.y >= self.top()
            && point.y < self.bottom()
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}×{} at {}",
            self.size.width, self.size.height, self.origin
        )
    }
}

/// A measurement on each of the four sides — margins, padding, borders.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Edges {
    /// The top.
    pub top: f32,
    /// The right.
    pub right: f32,
    /// The bottom.
    pub bottom: f32,
    /// The left.
    pub left: f32,
}

impl Edges {
    /// Nothing on any side.
    pub const ZERO: Edges = Edges {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    /// The same on every side.
    pub fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// How much the two horizontal sides take together.
    pub fn horizontal(self) -> f32 {
        self.left + self.right
    }

    /// How much the two vertical sides take together.
    pub fn vertical(self) -> f32 {
        self.top + self.bottom
    }
}

impl fmt::Display for Edges {
    /// Written the way CSS writes four sides: top, right, bottom, left.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} {}",
            self.top, self.right, self.bottom, self.left
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Equal to within far less than a pixel. Comparing floats exactly is
    /// fragile in a way that has nothing to do with what these tests are
    /// checking.
    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.0001
    }

    #[test]
    fn a_rectangles_edges_are_where_they_should_be() {
        let rect = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert!(close(rect.left(), 10.0));
        assert!(close(rect.top(), 20.0));
        assert!(close(rect.right(), 110.0));
        assert!(close(rect.bottom(), 70.0));
        assert_eq!(rect.to_string(), "100×50 at (10, 20)");
    }

    #[test]
    fn translating_moves_the_corner_and_not_the_size() {
        let moved = Rect::new(10.0, 20.0, 100.0, 50.0).translated(Point::new(5.0, -5.0));
        assert_eq!(moved, Rect::new(15.0, 15.0, 100.0, 50.0));
    }

    #[test]
    fn shrinking_pulls_every_edge_in() {
        let inner = Rect::new(0.0, 0.0, 100.0, 100.0).shrunk_by(Edges::all(10.0));
        assert_eq!(inner, Rect::new(10.0, 10.0, 80.0, 80.0));
    }

    #[test]
    fn shrinking_past_nothing_stops_at_nothing_rather_than_going_negative() {
        let inner = Rect::new(0.0, 0.0, 10.0, 10.0).shrunk_by(Edges::all(20.0));
        assert_eq!(inner.size, Size::ZERO);
        assert_eq!(inner.origin, Point::new(20.0, 20.0));
    }

    #[test]
    fn a_shared_edge_belongs_to_one_rectangle_and_not_both() {
        let left = Rect::new(0.0, 0.0, 50.0, 50.0);
        let right = Rect::new(50.0, 0.0, 50.0, 50.0);
        let on_the_seam = Point::new(50.0, 25.0);
        assert!(!left.contains(on_the_seam), "the left one lets it go");
        assert!(right.contains(on_the_seam), "and the right one takes it");
        assert!(
            left.contains(Point::new(0.0, 0.0)),
            "and owns its own corner"
        );
    }

    #[test]
    fn a_point_outside_a_rectangle_is_outside_it() {
        let rect = Rect::new(10.0, 10.0, 10.0, 10.0);
        assert!(!rect.contains(Point::new(9.0, 15.0)));
        assert!(!rect.contains(Point::new(15.0, 9.0)));
        assert!(!rect.contains(Point::new(20.0, 15.0)));
        assert!(rect.contains(Point::new(19.0, 19.0)));
    }

    #[test]
    fn edges_add_up_per_axis() {
        let edges = Edges {
            top: 1.0,
            right: 2.0,
            bottom: 4.0,
            left: 8.0,
        };
        assert!(close(edges.horizontal(), 10.0));
        assert!(close(edges.vertical(), 5.0));
        assert_eq!(edges.to_string(), "1 2 4 8");
        assert!(close(Edges::all(3.0).horizontal(), 6.0));
        assert!(close(Edges::ZERO.vertical(), 0.0));
    }

    #[test]
    fn nothing_is_at_the_origin_and_has_no_size() {
        assert_eq!(Rect::ZERO.origin, Point::ZERO);
        assert_eq!(Rect::ZERO.size, Size::ZERO);
        assert_eq!(Size::new(3.0, 4.0).to_string(), "3×4");
        assert_eq!(Point::ZERO.to_string(), "(0, 0)");
    }
}
