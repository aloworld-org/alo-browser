/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! SVG's basic shapes as paths, in user space.
//!
//! A `<path>` is its data, read by [`crate::path_data`]; every other shape is
//! built here. SVG 2 defines every basic shape as an equivalent path — where it starts,
//! which way it runs, where its arcs are — and this file builds exactly that
//! path. The direction matters: under `evenodd` it does not, but under
//! `nonzero` two shapes in one path wound opposite ways cancel, and a dash
//! pattern starts where the path starts and runs the way it runs.
//!
//! A path is what is **stroked**; whether it is also filled is [`fills`]'s
//! question, because a `<line>` and a subpath of no length have an outline and
//! no inside.
//!
//! An elliptical quarter arc is a cubic curve with its handles at
//! 0.552 284 75 of the radius, the standard approximation, which is within
//! 0.03% of the true ellipse — far under a pixel at any size an icon is drawn.
//!
//! # Errors
//!
//! A geometry attribute that is not a length makes the shape **not drawn**,
//! and says so. A size of zero draws nothing and says nothing, because SVG
//! says that is what zero means. A negative size is an error, as SVG says.

use crate::bounds::{MOST_PATH_SEGMENTS, MOST_POINTS};
use crate::length::{Axis, Viewport, user_units};
use crate::number::Numbers;
use crate::path_data::{self, TooMany};
use alo_dom::Element;
use alo_paint::{Path, Point, Segment};
use alo_value::FontMetrics;

/// How far along the radius an arc's handles sit, for a quarter ellipse.
const KAPPA: f32 = 0.552_284_8;

/// Why a shape could not be drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// This shape is not drawn, and this is why.
    Shape(String),
    /// The whole drawing is refused (a bound in [`crate::bounds`] was passed).
    Drawing(String),
}

/// What a shape is in user space, as SVG 2 defines its path.
pub struct Geometry<'a> {
    /// The element.
    pub element: &'a Element,
    /// What a percentage is a share of.
    pub viewport: Viewport,
    /// The element's font, for `em`.
    pub metrics: FontMetrics,
}

impl Geometry<'_> {
    /// The shape's path, [`None`] for one that draws nothing, or why not.
    ///
    /// Anything that is not a basic shape or a `<path>` is [`None`]: what is and is not
    /// drawn is the walk's question, not this one.
    ///
    /// # Errors
    ///
    /// [`Refusal::Shape`] for a geometry attribute in error, and
    /// [`Refusal::Drawing`] for a `points` list past [`MOST_POINTS`] or path
    /// data past [`MOST_PATH_SEGMENTS`].
    pub fn path(&self, issues: &mut Vec<String>) -> Result<Option<Path>, Refusal> {
        match &*self.element.name.local {
            "rect" => self.rect(),
            "circle" => self.circle(),
            "ellipse" => self.ellipse(),
            "polygon" => self.points(true, issues),
            "polyline" => self.points(false, issues),
            "path" => Ok(self.data(issues)?),
            "line" => self.line(),
            _ => Ok(None),
        }
    }

    /// SVG 2's line: from (`x1`, `y1`) to (`x2`, `y2`), each zero when not
    /// written. A line from a point to itself is still a line, and a round cap
    /// draws it as a dot.
    fn line(&self) -> Result<Option<Path>, Refusal> {
        let x1 = self.length("x1", Axis::Horizontal, 0.0)?;
        let y1 = self.length("y1", Axis::Vertical, 0.0)?;
        let x2 = self.length("x2", Axis::Horizontal, 0.0)?;
        let y2 = self.length("y2", Axis::Vertical, 0.0)?;
        let mut path = Path::new();
        path.move_to(Point::new(x1, y1));
        path.line_to(Point::new(x2, y2));
        Ok(Some(path))
    }

    /// An attribute in user units, `fallback` when it is not written.
    fn length(&self, name: &str, axis: Axis, fallback: f32) -> Result<f32, Refusal> {
        let Some(text) = self.element.attr(name) else {
            return Ok(fallback);
        };
        user_units(text, axis, self.viewport, self.metrics).map_err(|why| {
            Refusal::Shape(format!(
                "<{} {name}={text:?}>: {why}",
                self.element.name.local
            ))
        })
    }

    /// An attribute that may be `auto` or absent, which are the same; a
    /// negative value is an error and is treated as `auto`, as SVG 2 says of
    /// a property value out of range.
    fn radius(&self, name: &str, axis: Axis) -> Result<Option<f32>, Refusal> {
        match self.element.attr(name) {
            None => Ok(None),
            Some(text) if text.trim().eq_ignore_ascii_case("auto") => Ok(None),
            Some(_) => {
                let value = self.length(name, axis, 0.0)?;
                Ok((value >= 0.0).then_some(value))
            }
        }
    }

    /// A size that must not be negative.
    fn size(&self, name: &str, axis: Axis) -> Result<f32, Refusal> {
        let value = self.length(name, axis, 0.0)?;
        if value < 0.0 {
            return Err(Refusal::Shape(format!(
                "<{} {name}>: a negative size",
                self.element.name.local
            )));
        }
        Ok(value)
    }

    fn rect(&self) -> Result<Option<Path>, Refusal> {
        let x = self.length("x", Axis::Horizontal, 0.0)?;
        let y = self.length("y", Axis::Vertical, 0.0)?;
        let width = self.size("width", Axis::Horizontal)?;
        let height = self.size("height", Axis::Vertical)?;
        if width == 0.0 || height == 0.0 {
            return Ok(None);
        }
        // One radius written and the other not: the other is the same.
        let (rx, ry) = match (
            self.radius("rx", Axis::Horizontal)?,
            self.radius("ry", Axis::Vertical)?,
        ) {
            (None, None) => (0.0, 0.0),
            (Some(rx), None) => (rx, rx),
            (None, Some(ry)) => (ry, ry),
            (Some(rx), Some(ry)) => (rx, ry),
        };
        Ok(Some(rounded_rect(
            x,
            y,
            width,
            height,
            rx.min(width / 2.0),
            ry.min(height / 2.0),
        )))
    }

    fn circle(&self) -> Result<Option<Path>, Refusal> {
        let cx = self.length("cx", Axis::Horizontal, 0.0)?;
        let cy = self.length("cy", Axis::Vertical, 0.0)?;
        let r = self.size("r", Axis::Other)?;
        Ok((r > 0.0).then(|| ellipse(cx, cy, r, r)))
    }

    fn ellipse(&self) -> Result<Option<Path>, Refusal> {
        let cx = self.length("cx", Axis::Horizontal, 0.0)?;
        let cy = self.length("cy", Axis::Vertical, 0.0)?;
        let (rx, ry) = match (
            self.radius("rx", Axis::Horizontal)?,
            self.radius("ry", Axis::Vertical)?,
        ) {
            (None, None) => return Ok(None),
            (Some(rx), None) => (rx, rx),
            (None, Some(ry)) => (ry, ry),
            (Some(rx), Some(ry)) => (rx, ry),
        };
        Ok((rx > 0.0 && ry > 0.0).then(|| ellipse(cx, cy, rx, ry)))
    }

    /// A `polygon` (closed) or `polyline` (open, though a fill closes it).
    ///
    /// Drawn up to the first error in `points`, as SVG 2 says, and an odd
    /// coordinate at the end is the error that most often ends one.
    fn points(&self, closed: bool, issues: &mut Vec<String>) -> Result<Option<Path>, Refusal> {
        let name = &self.element.name.local;
        let Some(text) = self.element.attr("points") else {
            return Ok(None);
        };
        let mut numbers = Numbers::new(text);
        let mut path = Path::new();
        let mut pairs = 0_usize;
        let mut odd = false;
        while let Some(x) = numbers.next() {
            let Some(y) = numbers.next() else {
                odd = !numbers.failed();
                break;
            };
            pairs += 1;
            if pairs > MOST_POINTS {
                return Err(Refusal::Drawing(format!(
                    "<{name} points>: more than {MOST_POINTS} points"
                )));
            }
            if pairs == 1 {
                path.move_to(Point::new(x, y));
            } else {
                path.line_to(Point::new(x, y));
            }
        }
        if numbers.failed() || odd {
            issues.push(format!(
                "<{name} points={:?}>: drawn up to its first error",
                truncated(text),
            ));
        }
        if pairs < 2 {
            return Ok(None);
        }
        if closed {
            path.close();
        }
        Ok(Some(path))
    }

    /// A `<path>`: its `d`, drawn up to its first error, which is recorded.
    ///
    /// No `d` and `d="none"` are no path. Data of only moves is no path
    /// either, because a move on its own is not a subpath anything draws; data
    /// of moves and closes is kept, because `M 5 5 Z` is a subpath of no
    /// length that a round or square cap draws as a dot — though [`fills`]
    /// finds nothing inside it.
    fn data(&self, issues: &mut Vec<String>) -> Result<Option<Path>, Refusal> {
        let Some(text) = self.element.attr("d") else {
            return Ok(None);
        };
        if text.trim().eq_ignore_ascii_case("none") {
            return Ok(None);
        }
        let parsed = path_data::parse(text).map_err(|TooMany| {
            Refusal::Drawing(format!(
                "<path d>: more than {MOST_PATH_SEGMENTS} path segments"
            ))
        })?;
        if let Some(at) = parsed.error {
            issues.push(format!(
                "<path d={:?}>: drawn up to its first error, at byte {at}",
                truncated(text),
            ));
        }
        let drawn = parsed
            .path
            .segments()
            .iter()
            .any(|segment| !matches!(segment, Segment::MoveTo(_)));
        Ok(drawn.then_some(parsed.path))
    }
}

/// Whether a shape's path has an inside to fill.
///
/// Not a `<line>`, which SVG says has none, and not a path with nothing in it
/// but moves and closes; everything else is filled, even a polyline of two
/// points, whose inside is nothing and fills nothing.
pub fn fills(name: &str, path: &Path) -> bool {
    name != "line"
        && path.segments().iter().any(|segment| {
            matches!(
                segment,
                Segment::LineTo(_) | Segment::QuadTo(..) | Segment::CubicTo(..)
            )
        })
}

/// The first few dozen characters of an attribute, for an issue that quotes
/// it: a million-point list is not worth repeating in full.
pub(crate) fn truncated(text: &str) -> &str {
    let end = text
        .char_indices()
        .nth(48)
        .map_or(text.len(), |(index, _)| index);
    text.get(..end).unwrap_or(text)
}

/// SVG 2's rectangle, rounded or not: clockwise from the top edge's left end
/// (or its top-left corner, when there is no rounding).
pub fn rounded_rect(x: f32, y: f32, width: f32, height: f32, rx: f32, ry: f32) -> Path {
    if rx <= 0.0 || ry <= 0.0 {
        return Path::rectangle(x, y, width, height);
    }
    let (right, bottom) = (x + width, y + height);
    let (hx, hy) = (rx * KAPPA, ry * KAPPA);
    let mut path = Path::new();
    path.move_to(Point::new(x + rx, y));
    path.line_to(Point::new(right - rx, y));
    path.cubic_to(
        Point::new(right - rx + hx, y),
        Point::new(right, y + ry - hy),
        Point::new(right, y + ry),
    );
    path.line_to(Point::new(right, bottom - ry));
    path.cubic_to(
        Point::new(right, bottom - ry + hy),
        Point::new(right - rx + hx, bottom),
        Point::new(right - rx, bottom),
    );
    path.line_to(Point::new(x + rx, bottom));
    path.cubic_to(
        Point::new(x + rx - hx, bottom),
        Point::new(x, bottom - ry + hy),
        Point::new(x, bottom - ry),
    );
    path.line_to(Point::new(x, y + ry));
    path.cubic_to(
        Point::new(x, y + ry - hy),
        Point::new(x + rx - hx, y),
        Point::new(x + rx, y),
    );
    path.close();
    path
}

/// SVG 2's ellipse: from its rightmost point, clockwise on the page.
pub fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Path {
    let (hx, hy) = (rx * KAPPA, ry * KAPPA);
    let mut path = Path::new();
    path.move_to(Point::new(cx + rx, cy));
    path.cubic_to(
        Point::new(cx + rx, cy + hy),
        Point::new(cx + hx, cy + ry),
        Point::new(cx, cy + ry),
    );
    path.cubic_to(
        Point::new(cx - hx, cy + ry),
        Point::new(cx - rx, cy + hy),
        Point::new(cx - rx, cy),
    );
    path.cubic_to(
        Point::new(cx - rx, cy - hy),
        Point::new(cx - hx, cy - ry),
        Point::new(cx, cy - ry),
    );
    path.cubic_to(
        Point::new(cx + hx, cy - ry),
        Point::new(cx + rx, cy - hy),
        Point::new(cx + rx, cy),
    );
    path.close();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::{Document, parse_document};

    const VIEW: Viewport = Viewport {
        width: 100.0,
        height: 50.0,
    };

    fn document(markup: &str) -> Document {
        parse_document(&format!("<svg>{markup}</svg>"))
    }

    fn path_of(markup: &str) -> (Result<Option<Path>, Refusal>, Vec<String>) {
        let document = document(markup);
        let id = document
            .descendants(document.root())
            .filter(|id| document.element(*id).is_some())
            .last()
            .expect("an element");
        let element = document.element(id).expect("an element");
        let mut issues = Vec::new();
        let geometry = Geometry {
            element,
            viewport: VIEW,
            metrics: FontMetrics::default(),
        };
        (geometry.path(&mut issues), issues)
    }

    fn drawn(markup: &str) -> Path {
        match path_of(markup) {
            (Ok(Some(path)), _) => path,
            other => panic!("{markup} drew nothing: {other:?}"),
        }
    }

    #[test]
    fn a_rect_is_its_four_corners_clockwise_from_the_top_left() {
        let path = drawn(r#"<rect x="10" y="5" width="20" height="10"/>"#);
        assert_eq!(path, Path::rectangle(10.0, 5.0, 20.0, 10.0));
        assert_eq!(path.bounds(), Some((10.0, 5.0, 30.0, 15.0)));
    }

    #[test]
    fn a_rects_percentages_are_shares_of_the_viewport() {
        let path = drawn(r#"<rect x="10%" y="10%" width="50%" height="50%"/>"#);
        assert_eq!(path.bounds(), Some((10.0, 5.0, 60.0, 30.0)));
    }

    #[test]
    fn a_rounded_rect_starts_after_its_corner_and_its_radii_are_clamped() {
        let path = drawn(r#"<rect width="20" height="10" rx="4"/>"#);
        assert_eq!(
            path.segments().first(),
            Some(&Segment::MoveTo(Point::new(4.0, 0.0))),
            "one radius written: the other is the same",
        );
        assert_eq!(path.segments().len(), 10);
        assert_eq!(path.bounds(), Some((0.0, 0.0, 20.0, 10.0)));

        // rx past half the width is half the width, and ry follows rx.
        let clamped = drawn(r#"<rect width="20" height="10" rx="50" ry="3"/>"#);
        assert_eq!(
            clamped.segments().first(),
            Some(&Segment::MoveTo(Point::new(10.0, 0.0)))
        );
        assert_eq!(
            clamped.segments().get(3),
            Some(&Segment::LineTo(Point::new(20.0, 7.0))),
            "the right edge runs from ry to height − ry",
        );
    }

    #[test]
    fn a_circle_is_four_arcs_from_its_rightmost_point() {
        let path = drawn(r#"<circle cx="10" cy="10" r="5"/>"#);
        assert_eq!(
            path.segments().first(),
            Some(&Segment::MoveTo(Point::new(15.0, 10.0)))
        );
        assert!(
            matches!(
                path.segments().get(1),
                Some(Segment::CubicTo(_, _, end)) if *end == Point::new(10.0, 15.0)
            ),
            "clockwise on the page: down first"
        );
        assert_eq!(path.segments().len(), 6);
        assert_eq!(path.bounds(), Some((5.0, 5.0, 15.0, 15.0)));
    }

    #[test]
    fn an_arcs_handles_keep_it_within_a_hair_of_the_true_circle() {
        // The middle of a quarter arc, at t = 1/2, against the true radius.
        let path = ellipse(0.0, 0.0, 100.0, 100.0);
        let Some(Segment::CubicTo(one, two, end)) = path.segments().get(1).copied() else {
            panic!("an arc");
        };
        let start = Point::new(100.0, 0.0);
        let x = 0.125 * start.x + 0.375 * one.x + 0.375 * two.x + 0.125 * end.x;
        let y = 0.125 * start.y + 0.375 * one.y + 0.375 * two.y + 0.125 * end.y;
        let radius = x.hypot(y);
        assert!((radius - 100.0).abs() < 0.03, "{radius}");
    }

    #[test]
    fn an_ellipse_takes_a_missing_radius_from_the_other() {
        let path = drawn(r#"<ellipse cx="50" cy="25" rx="40" ry="20"/>"#);
        assert_eq!(path.bounds(), Some((10.0, 5.0, 90.0, 45.0)));
        let round = drawn(r#"<ellipse rx="3"/>"#);
        assert_eq!(round.bounds(), Some((-3.0, -3.0, 3.0, 3.0)));
        assert!(matches!(path_of("<ellipse/>").0, Ok(None)));
    }

    #[test]
    fn a_polygon_closes_and_a_polyline_does_not() {
        let polygon = drawn(r#"<polygon points="0,0 10,0 10,10"/>"#);
        assert_eq!(polygon.segments().len(), 4);
        assert_eq!(polygon.segments().last(), Some(&Segment::Close));
        let polyline = drawn(r#"<polyline points="0 0 10 0 10 10"/>"#);
        assert_eq!(polyline.segments().len(), 3);
        assert_eq!(
            polyline.segments().last(),
            Some(&Segment::LineTo(Point::new(10.0, 10.0)))
        );
    }

    #[test]
    fn points_are_drawn_up_to_their_first_error_and_it_is_recorded() {
        let (path, issues) = path_of(r#"<polygon points="0,0 10,0 10,10 5"/>"#);
        let path = path.expect("drawn").expect("a path");
        assert_eq!(path.segments().len(), 4, "the odd coordinate is dropped");
        assert_eq!(issues.len(), 1);

        let (path, issues) = path_of(r#"<polygon points="0,0 x 10,10"/>"#);
        assert!(matches!(path, Ok(None)), "one point is not a shape");
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn zero_draws_nothing_and_negative_is_an_error() {
        for nothing in [
            r#"<rect width="0" height="10"/>"#,
            r#"<rect width="10"/>"#,
            r#"<circle r="0"/>"#,
            "<circle/>",
            r#"<polygon points=""/>"#,
            "<g/>",
        ] {
            assert!(matches!(path_of(nothing).0, Ok(None)), "{nothing}");
        }
        for wrong in [
            r#"<rect width="-1" height="10"/>"#,
            r#"<circle r="-1"/>"#,
            r#"<rect x="ten" width="1" height="1"/>"#,
            r#"<circle cx="1e99999" r="1"/>"#,
        ] {
            assert!(
                matches!(path_of(wrong).0, Err(Refusal::Shape(_))),
                "{wrong}"
            );
        }
        // A negative radius on a rect is `auto`, not an error.
        let path = drawn(r#"<rect width="10" height="10" rx="-2"/>"#);
        assert_eq!(path, Path::rectangle(0.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn a_line_is_a_path_with_nothing_inside_it() {
        let path = drawn(r#"<line x1="1" y1="2" x2="50%" y2="10"/>"#);
        assert_eq!(
            path.segments(),
            [
                Segment::MoveTo(Point::new(1.0, 2.0)),
                Segment::LineTo(Point::new(50.0, 10.0)),
            ]
        );
        assert!(!fills("line", &path));
        // A line from a point to itself is kept, for a cap to draw.
        let point = drawn("<line/>");
        assert_eq!(point.segments().len(), 2);
        assert!(matches!(
            path_of(r#"<line x2="far"/>"#).0,
            Err(Refusal::Shape(_))
        ));
    }

    #[test]
    fn moves_and_closes_are_kept_for_a_cap_and_have_nothing_to_fill() {
        assert!(matches!(path_of(r#"<path d="M 5 5"/>"#).0, Ok(None)));
        assert!(matches!(path_of(r#"<path d="M 5 5 M 6 6"/>"#).0, Ok(None)));
        let dot = drawn(r#"<path d="M 5 5 Z"/>"#);
        assert!(!fills("path", &dot));
        let drawn_line = drawn(r#"<path d="M 5 5 L 6 6"/>"#);
        assert!(fills("path", &drawn_line));
        assert!(fills("polyline", &drawn(r#"<polyline points="0 0 4 4"/>"#)));
    }

    #[test]
    fn a_points_list_past_the_bound_refuses_the_drawing_and_one_at_it_does_not() {
        let at = "1 1 ".repeat(MOST_POINTS);
        let (path, _) = path_of(&format!(r#"<polyline points="{at}"/>"#));
        let path = path.expect("drawn").expect("a path");
        assert_eq!(path.segments().len(), MOST_POINTS);

        let past = "1 1 ".repeat(MOST_POINTS + 1);
        let (path, _) = path_of(&format!(r#"<polyline points="{past}"/>"#));
        assert!(matches!(path, Err(Refusal::Drawing(_))));
    }
}
