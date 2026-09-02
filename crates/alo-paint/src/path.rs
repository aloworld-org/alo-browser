//! A shape, in our own vocabulary.
//!
//! Everything this engine draws is a path: a rectangle is four lines, a
//! rounded corner is an arc, a glyph is a few dozen curves. Having one shape
//! type means one rasteriser, one set of anti-aliasing rules, and one place
//! for a pixel to be wrong — rather than a box that is drawn one way and a
//! letter that is drawn another and a seam between them that nobody notices
//! until it is on a screen.
//!
//! It is ours rather than the rasteriser's for ADR 0001's reason: the
//! rasteriser is rented, and a rented type in our vocabulary is a rewrite the
//! day we change it.

use core::fmt;

/// A point, in CSS pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    /// Across.
    pub x: f32,
    /// Down.
    pub y: f32,
}

impl Point {
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

/// One step of a path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    /// Lift the pen and put it down here.
    MoveTo(Point),
    /// Draw a straight line to here.
    LineTo(Point),
    /// Draw a curve to the second point, bending towards the first.
    QuadTo(Point, Point),
    /// Draw a curve to the third point, bending towards the first two.
    CubicTo(Point, Point, Point),
    /// Draw back to where the pen was last put down.
    Close,
}

/// A shape to fill.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Path {
    segments: Vec<Segment>,
}

impl Path {
    /// A path with nothing in it.
    pub fn new() -> Self {
        Self::default()
    }

    /// The steps, in order.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Whether there is nothing to draw.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Lift the pen and put it down at a point.
    pub fn move_to(&mut self, to: Point) {
        self.segments.push(Segment::MoveTo(to));
    }

    /// Draw a straight line.
    pub fn line_to(&mut self, to: Point) {
        self.segments.push(Segment::LineTo(to));
    }

    /// Draw a quadratic curve.
    pub fn quad_to(&mut self, control: Point, to: Point) {
        self.segments.push(Segment::QuadTo(control, to));
    }

    /// Draw a cubic curve.
    pub fn cubic_to(&mut self, first: Point, second: Point, to: Point) {
        self.segments.push(Segment::CubicTo(first, second, to));
    }

    /// Close the current outline.
    pub fn close(&mut self) {
        self.segments.push(Segment::Close);
    }

    /// Everything in another path, added to this one.
    ///
    /// Two shapes in one path are filled together, which is what makes a run
    /// of text one shape: a shadow blurred from the whole run is smooth where
    /// two letters touch, where one blur per letter would have been darker
    /// there.
    pub fn extend(&mut self, other: &Path) {
        self.segments.extend_from_slice(&other.segments);
    }

    /// A rectangle.
    pub fn rectangle(x: f32, y: f32, width: f32, height: f32) -> Self {
        let mut path = Self::new();
        path.move_to(Point::new(x, y));
        path.line_to(Point::new(x + width, y));
        path.line_to(Point::new(x + width, y + height));
        path.line_to(Point::new(x, y + height));
        path.close();
        path
    }

    /// Everything in this path moved by an offset.
    ///
    /// A glyph is outlined once at the origin and placed wherever it is
    /// needed, which is the whole reason this exists.
    #[must_use]
    pub fn translated(&self, by: Point) -> Self {
        let moved = |point: Point| Point::new(point.x + by.x, point.y + by.y);
        Self {
            segments: self
                .segments
                .iter()
                .map(|segment| match *segment {
                    Segment::MoveTo(to) => Segment::MoveTo(moved(to)),
                    Segment::LineTo(to) => Segment::LineTo(moved(to)),
                    Segment::QuadTo(control, to) => Segment::QuadTo(moved(control), moved(to)),
                    Segment::CubicTo(first, second, to) => {
                        Segment::CubicTo(moved(first), moved(second), moved(to))
                    }
                    Segment::Close => Segment::Close,
                })
                .collect(),
        }
    }

    /// Every point of this path put through a transform.
    ///
    /// A curve stays a curve: an affine transform moves control points, and
    /// the curve through the moved points is the moved curve. That is why
    /// `transform` needs no new geometry — a rotated letter is the same
    /// outline with different numbers in it.
    #[must_use]
    pub fn transformed(&self, by: alo_value::Matrix) -> Self {
        let moved = |point: Point| {
            let (x, y) = by.apply(point.x, point.y);
            Point::new(x, y)
        };
        Self {
            segments: self
                .segments
                .iter()
                .map(|segment| match *segment {
                    Segment::MoveTo(to) => Segment::MoveTo(moved(to)),
                    Segment::LineTo(to) => Segment::LineTo(moved(to)),
                    Segment::QuadTo(control, to) => Segment::QuadTo(moved(control), moved(to)),
                    Segment::CubicTo(first, second, to) => {
                        Segment::CubicTo(moved(first), moved(second), moved(to))
                    }
                    Segment::Close => Segment::Close,
                })
                .collect(),
        }
    }

    /// The smallest rectangle every point of this path fits inside, as
    /// `(left, top, right, bottom)`.
    ///
    /// The control points are included, so the box is never too small — a
    /// curve stays inside its control points. It may be a little too large,
    /// which costs a few pixels of blank mask and never a clipped glyph.
    pub fn bounds(&self) -> Option<(f32, f32, f32, f32)> {
        let mut found: Option<(f32, f32, f32, f32)> = None;
        let mut extend = |point: Point| {
            found = Some(match found {
                None => (point.x, point.y, point.x, point.y),
                Some((left, top, right, bottom)) => (
                    left.min(point.x),
                    top.min(point.y),
                    right.max(point.x),
                    bottom.max(point.y),
                ),
            });
        };
        for segment in &self.segments {
            match *segment {
                Segment::MoveTo(to) | Segment::LineTo(to) => extend(to),
                Segment::QuadTo(control, to) => {
                    extend(control);
                    extend(to);
                }
                Segment::CubicTo(first, second, to) => {
                    extend(first);
                    extend(second);
                    extend(to);
                }
                Segment::Close => {}
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.001
    }

    #[test]
    fn a_new_path_has_nothing_in_it() {
        let path = Path::new();
        assert!(path.is_empty());
        assert!(path.segments().is_empty());
        assert_eq!(path.bounds(), None);
    }

    #[test]
    fn a_transform_moves_every_point_and_leaves_the_shape_a_shape() {
        let path = Path::rectangle(0.0, 0.0, 10.0, 10.0);
        let moved = path.transformed(alo_value::Matrix::translation(5.0, -5.0));
        assert_eq!(moved.bounds(), Some((5.0, -5.0, 15.0, 5.0)));
        assert_eq!(moved.segments().len(), path.segments().len());
    }

    #[test]
    fn a_curves_control_points_move_with_it() {
        let mut path = Path::new();
        path.move_to(Point::new(0.0, 0.0));
        path.cubic_to(
            Point::new(1.0, 0.0),
            Point::new(2.0, 1.0),
            Point::new(2.0, 2.0),
        );
        let doubled = path.transformed(alo_value::Matrix {
            a: 2.0,
            d: 2.0,
            ..alo_value::Matrix::IDENTITY
        });
        assert_eq!(
            doubled.segments().get(1),
            Some(&Segment::CubicTo(
                Point::new(2.0, 0.0),
                Point::new(4.0, 2.0),
                Point::new(4.0, 4.0),
            )),
        );
    }

    #[test]
    fn a_rectangle_is_four_corners_and_a_close() {
        let path = Path::rectangle(10.0, 20.0, 100.0, 50.0);
        assert_eq!(path.segments().len(), 5);
        assert_eq!(path.bounds(), Some((10.0, 20.0, 110.0, 70.0)));
        assert_eq!(path.segments().last(), Some(&Segment::Close));
    }

    #[test]
    fn every_kind_of_segment_is_kept_in_order() {
        let mut path = Path::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(1.0, 0.0));
        path.quad_to(Point::new(2.0, 0.0), Point::new(2.0, 1.0));
        path.cubic_to(
            Point::new(2.0, 2.0),
            Point::new(1.0, 3.0),
            Point::new(0.0, 3.0),
        );
        path.close();

        assert_eq!(path.segments().len(), 5);
        assert!(matches!(path.segments().first(), Some(Segment::MoveTo(_))));
        assert!(matches!(path.segments().get(2), Some(Segment::QuadTo(..))));
        assert!(matches!(path.segments().get(3), Some(Segment::CubicTo(..))));
    }

    #[test]
    fn the_bounds_include_the_control_points_so_a_curve_is_never_clipped() {
        let mut path = Path::new();
        path.move_to(Point::new(0.0, 0.0));
        // The curve bulges towards (5, 10) and ends back at the bottom left.
        path.quad_to(Point::new(5.0, 10.0), Point::new(0.0, 4.0));
        let (left, top, right, bottom) = path.bounds().expect("bounds");
        assert!(close(left, 0.0) && close(top, 0.0));
        assert!(
            close(right, 5.0) && close(bottom, 10.0),
            "a little too large is a few blank pixels; too small is a clipped glyph",
        );
    }

    #[test]
    fn translating_moves_every_point_and_leaves_the_shape_alone() {
        let path = Path::rectangle(0.0, 0.0, 10.0, 10.0);
        let moved = path.translated(Point::new(5.0, -5.0));
        assert_eq!(moved.bounds(), Some((5.0, -5.0, 15.0, 5.0)));
        assert_eq!(moved.segments().len(), path.segments().len());

        let mut curved = Path::new();
        curved.move_to(Point::new(0.0, 0.0));
        curved.cubic_to(
            Point::new(1.0, 1.0),
            Point::new(2.0, 2.0),
            Point::new(3.0, 3.0),
        );
        let shifted = curved.translated(Point::new(10.0, 10.0));
        assert_eq!(
            shifted.segments().get(1),
            Some(&Segment::CubicTo(
                Point::new(11.0, 11.0),
                Point::new(12.0, 12.0),
                Point::new(13.0, 13.0),
            )),
        );
    }

    #[test]
    fn a_point_prints_as_a_point() {
        assert_eq!(Point::new(1.5, -2.0).to_string(), "(1.5, -2)");
        assert_eq!(Point::default(), Point::new(0.0, 0.0));
    }
}
