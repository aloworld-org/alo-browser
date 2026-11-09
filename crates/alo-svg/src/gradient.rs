/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `<linearGradient>`, as the colours a shape is painted with (queue item
//! 275, ADR 0107).
//!
//! What SVG 2's *Paint Servers* chapter says, and what Chrome 157 draws
//! (`cases/svg-linear-gradients`):
//!
//! - **The line** runs from `(x1, y1)` to `(x2, y2)` — by default from
//!   `0%, 0%` to `100%, 0%`, left to right — with lines of one colour at
//!   right angles to it in the gradient's own space.
//! - **`gradientUnits`**: `objectBoundingBox`, the default, makes that
//!   space the shape's bounding box as a unit square, so a number is a
//!   fraction of it and a percentage the same fraction; a gradient across a
//!   wide, short box is squashed with it, its lines of one colour no longer
//!   at right angles. `userSpaceOnUse` makes it the shape's own user space,
//!   where lengths and percentages of the viewport mean what they mean on a
//!   shape. A shape with no width or no height has no box to be a fraction
//!   of, and is painted with nothing.
//! - **`gradientTransform`** moves the gradient's space inside that, read
//!   in the `transform` attribute's grammar.
//! - **`<stop>`s**, the gradient's element children in order: each
//!   `offset` a number or a percentage held to between nothing and one and
//!   never less than the one before; each colour its `stop-color` (black
//!   when nothing is written) faded by its `stop-opacity`, both properties
//!   of the cascade. No stops paint nothing; one paints its colour; two or
//!   more are mixed as straight colours, then premultiplied.
//! - **`spreadMethod`**: `pad`, the default, holds the end colours past
//!   the line's ends; `reflect` and `repeat` run the line again.
//! - **A line with no length** paints the last stop's colour.
//!
//! The paint property's opacity — `fill-opacity` or `stroke-opacity` —
//! fades every stop.
//!
//! **Not followed yet:** `href`, by which one gradient takes the stops and
//! attributes another does not write (item 912). A gradient that has one
//! is drawn from what it writes itself, and it is recorded.

use crate::bounds::MOST_STOPS;
use crate::length::{Axis, user_units};
use crate::paint::{Server, alpha};
use crate::server::{Shade, Target};
use alo_dom::{Document, Element, Namespace, NodeId};
use alo_paint::Ramp;
use alo_paint::colour_paint::{ColourLine, Extend};
use alo_style::StyleTree;
use alo_value::{LengthPercentage, Matrix, Rgba};

/// Which space a gradient's line is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Units {
    /// The shape's bounding box, as a unit square.
    ObjectBoundingBox,
    /// The shape's own user space.
    UserSpaceOnUse,
}

/// What the `<linearGradient>` at `gradient` paints `target` with.
///
/// # Errors
///
/// More than [`MOST_STOPS`] stops, which refuses the whole drawing.
pub fn linear(
    document: &Document,
    styles: &StyleTree,
    gradient: NodeId,
    server: &Server,
    target: &Target,
    issues: &mut Vec<String>,
) -> Result<Option<Shade>, String> {
    let Some(element) = document.element(gradient) else {
        return Ok(None);
    };
    if element
        .attrs
        .iter()
        .any(|attribute| &*attribute.name.local == "href")
    {
        issues.push(format!(
            "{}: {}: a gradient's href is not followed yet (item 912), so only what it writes itself",
            server.property, server.written
        ));
    }
    let stops = stops(document, styles, gradient, server.opacity)?;
    let (Some(first), Some(last)) = (stops.first(), stops.last()) else {
        return Ok(None);
    };
    if stops.len() == 1 {
        return Ok(Some(Shade::Colour(first.1)));
    }
    let last = last.1;

    let Some((units, space)) = space(element, target, issues) else {
        return Ok(None);
    };
    let own = own_transform(element, issues);
    let extend = spread(element, issues);

    let mut point = |name: &str, initial: f32, axis: Axis| {
        coordinate(element, name, units, axis, target)
            .unwrap_or_else(|why| {
                issues.push(format!("<linearGradient {name}>: {why}, so {initial}"));
                Some(initial)
            })
            .unwrap_or(initial)
    };
    let user_width = |fraction: f32| match units {
        Units::ObjectBoundingBox => fraction,
        Units::UserSpaceOnUse => fraction * target.viewport.width,
    };
    let start = (
        point("x1", user_width(0.0), Axis::Horizontal),
        point("y1", 0.0, Axis::Vertical),
    );
    let end = (
        point("x2", user_width(1.0), Axis::Horizontal),
        point("y2", 0.0, Axis::Vertical),
    );
    if start == end {
        return Ok(Some(Shade::Colour(last)));
    }
    let to_drawing = own.then(space).then(target.matrix);
    let line = ColourLine { stops, extend };
    if let Some(ramp) = Ramp::between(start, end, to_drawing, line) {
        return Ok(Some(Shade::Ramp(ramp)));
    }
    issues.push(format!(
        "{}: {}: a gradient flattened onto a line paints nothing",
        server.property, server.written
    ));
    Ok(None)
}

/// Which space the line is drawn in, and how that space maps into the
/// shape's user space: [`None`] for a box a shape with no width or no
/// height cannot give.
fn space(element: &Element, target: &Target, issues: &mut Vec<String>) -> Option<(Units, Matrix)> {
    let units = match element.attr("gradientUnits").map(str::trim) {
        None | Some("objectBoundingBox") => Units::ObjectBoundingBox,
        Some("userSpaceOnUse") => Units::UserSpaceOnUse,
        Some(other) => {
            issues.push(format!(
                "<linearGradient gradientUnits={other:?}>: not a value, so objectBoundingBox"
            ));
            Units::ObjectBoundingBox
        }
    };
    let space = match units {
        Units::UserSpaceOnUse => Matrix::IDENTITY,
        Units::ObjectBoundingBox => {
            let bbox = target
                .bbox
                .filter(|bbox| bbox.width > 0.0 && bbox.height > 0.0)?;
            Matrix {
                a: bbox.width,
                b: 0.0,
                c: 0.0,
                d: bbox.height,
                e: bbox.x,
                f: bbox.y,
            }
        }
    };
    Some((units, space))
}

/// `gradientTransform`, in the `transform` attribute's grammar; nothing
/// for something that is not one, recorded.
fn own_transform(element: &Element, issues: &mut Vec<String>) -> Matrix {
    let Some(text) = element.attr("gradientTransform") else {
        return Matrix::IDENTITY;
    };
    alo_value::svg_transform::parse(text).unwrap_or_else(|| {
        issues.push(format!(
            "<linearGradient gradientTransform={:?}>: not a transform, so none",
            crate::shape::truncated(text)
        ));
        Matrix::IDENTITY
    })
}

/// `spreadMethod`: `pad` when nothing, or something that is not one, is
/// written — the latter recorded.
fn spread(element: &Element, issues: &mut Vec<String>) -> Extend {
    match element.attr("spreadMethod").map(str::trim) {
        None | Some("pad") => Extend::Pad,
        Some("reflect") => Extend::Reflect,
        Some("repeat") => Extend::Repeat,
        Some(other) => {
            issues.push(format!(
                "<linearGradient spreadMethod={other:?}>: not a value, so pad"
            ));
            Extend::Pad
        }
    }
}

/// One of the line's four coordinates: [`None`] for nothing written, and
/// an error for something written that is not one.
fn coordinate(
    element: &Element,
    name: &str,
    units: Units,
    axis: Axis,
    target: &Target,
) -> Result<Option<f32>, &'static str> {
    let Some(text) = element.attr(name) else {
        return Ok(None);
    };
    let value = match units {
        // A fraction of the box: a number is one, and so is a percentage;
        // a length is its number of pixels, as though that were one.
        Units::ObjectBoundingBox => match alo_value::svg_number::number(text) {
            Some(number) => number,
            None => match alo_value::parse_length_percentage(text.trim()) {
                Some(LengthPercentage::Percentage(percent)) => percent / 100.0,
                Some(length) => length.to_px(target.metrics, 1.0),
                None => return Err("not a number, a length or a percentage"),
            },
        },
        Units::UserSpaceOnUse => user_units(text, axis, target.viewport, target.metrics)?,
    };
    if value.is_finite() {
        Ok(Some(value))
    } else {
        Err("not a finite length")
    }
}

/// The gradient's stops, in order, each offset held to the line and to
/// never going back, each colour faded by its `stop-opacity` and by
/// `opacity`.
///
/// # Errors
///
/// More than [`MOST_STOPS`] of them.
fn stops(
    document: &Document,
    styles: &StyleTree,
    gradient: NodeId,
    opacity: f32,
) -> Result<Vec<(f32, Rgba)>, String> {
    let mut found: Vec<(f32, Rgba)> = Vec::new();
    for child in document.children(gradient) {
        let Some(element) = document.element(child) else {
            continue;
        };
        if element.name.ns != Namespace::Svg || &*element.name.local != "stop" {
            continue;
        }
        if found.len() == MOST_STOPS {
            return Err(format!("a gradient of more than {MOST_STOPS} stops"));
        }
        let offset = element
            .attr("offset")
            .and_then(|text| alpha(Some(text)))
            .unwrap_or(0.0);
        let held = found.last().map_or(0.0, |(before, _)| *before);
        let style = styles.get(child);
        let color = style
            .and_then(|style| style.color("stop-color"))
            .unwrap_or(Rgba::BLACK);
        let faded = alpha(style.and_then(|style| style.get("stop-opacity"))).unwrap_or(1.0);
        found.push((
            offset.max(held),
            Rgba {
                alpha: color.alpha * faded * opacity,
                ..color
            },
        ));
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bbox::Rect;
    use crate::length::Viewport;
    use crate::server::Servers;
    use alo_dom::parse_document;
    use alo_value::FontMetrics;

    fn target(bbox: Rect) -> Target {
        Target {
            bbox: Some(bbox),
            matrix: Matrix::IDENTITY,
            viewport: Viewport {
                width: 100.0,
                height: 50.0,
            },
            metrics: FontMetrics::default(),
        }
    }

    fn square() -> Rect {
        Rect {
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
        }
    }

    /// What `<linearGradient id=g>` in `markup` paints `target` with.
    fn painted(
        markup: &str,
        target: &Target,
        opacity: f32,
    ) -> (Result<Option<Shade>, String>, Vec<String>) {
        let document = parse_document(markup);
        let styles = alo_style::resolve(&document, &[], &alo_css::MediaContext::default());
        let Some(gradient) = Servers::of(&document).named("g") else {
            panic!("the gradient");
        };
        let server = Server {
            property: "fill",
            written: "url(#g)".to_owned(),
            id: Some("g".to_owned()),
            fallback: None,
            opacity,
        };
        let mut issues = Vec::new();
        let found = linear(&document, &styles, gradient, &server, target, &mut issues);
        (found, issues)
    }

    fn ramp_of(found: Result<Option<Shade>, String>) -> Ramp {
        match found {
            Ok(Some(Shade::Ramp(ramp))) => ramp,
            other => panic!("not a ramp: {other:?}"),
        }
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1.0e-4
    }

    const TWO: &str = r"<svg><linearGradient id=g><stop offset=0 stop-color=red /><stop offset=1 stop-color=blue /></linearGradient></svg>";

    #[test]
    fn by_default_a_gradient_runs_left_to_right_across_the_shapes_box() {
        let (found, issues) = painted(TWO, &target(square()), 1.0);
        let ramp = ramp_of(found);
        assert!(close(ramp.along(10.0, 0.0), 0.0));
        assert!(close(ramp.along(20.0, 99.0), 0.5));
        assert!(close(ramp.along(30.0, 0.0), 1.0));
        assert_eq!(ramp.at(0.0, 0.0), Rgba::from_rgba8(255, 0, 0, 255));
        assert!(issues.is_empty());
    }

    #[test]
    fn a_diagonal_across_a_wide_box_is_squashed_with_it() {
        let markup = r#"<svg><linearGradient id=g x2="100%" y2="1"><stop stop-color=red /><stop offset="100%" stop-color=blue /></linearGradient></svg>"#;
        let wide = Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 10.0,
        };
        let ramp = ramp_of(painted(markup, &target(wide), 1.0).0);
        // The corners on the other diagonal are both half way.
        assert!(close(ramp.along(100.0, 0.0), 0.5));
        assert!(close(ramp.along(0.0, 10.0), 0.5));
    }

    #[test]
    fn in_user_space_a_percentage_is_of_the_viewport_and_a_transform_moves_it() {
        let markup = r#"<svg><linearGradient id=g gradientUnits="userSpaceOnUse" x1="10%" x2="20" gradientTransform="translate(5)"><stop stop-color=red /><stop offset=1 stop-color=blue /></linearGradient></svg>"#;
        let ramp = ramp_of(painted(markup, &target(square()), 1.0).0);
        // From 10 to 20 in the gradient's space, which is 15 to 25 here.
        assert!(close(ramp.along(15.0, 3.0), 0.0));
        assert!(close(ramp.along(25.0, 3.0), 1.0));
    }

    #[test]
    fn stops_are_held_to_the_line_and_never_go_back_and_are_faded() {
        let markup = r#"<svg><linearGradient id=g><stop offset="-1" stop-color=red /><stop offset="0.6" stop-color=lime stop-opacity="50%"/><stop offset="0.2" stop-color=blue /><stop offset="7" /></linearGradient></svg>"#;
        let ramp = ramp_of(painted(markup, &target(square()), 0.5).0);
        let stops = &ramp.line().stops;
        let offsets: Vec<f32> = stops.iter().map(|(offset, _)| *offset).collect();
        assert_eq!(offsets, vec![0.0, 0.6, 0.6, 1.0]);
        let alphas: Vec<f32> = stops.iter().map(|(_, color)| color.alpha).collect();
        assert!(close(alphas[1], 0.25) && close(alphas[0], 0.5));
        assert_eq!(
            stops.get(3).map(|(_, color)| (color.red, color.green)),
            Some((0.0, 0.0))
        );
    }

    #[test]
    fn no_stops_paint_nothing_one_paints_its_colour_and_no_length_the_last() {
        let none = r"<svg><linearGradient id=g></linearGradient></svg>";
        assert_eq!(painted(none, &target(square()), 1.0).0, Ok(None));
        let one = r"<svg><linearGradient id=g><stop stop-color=lime /></linearGradient></svg>";
        assert_eq!(
            painted(one, &target(square()), 1.0).0,
            Ok(Some(Shade::Colour(Rgba::from_rgba8(0, 255, 0, 255))))
        );
        let point = r#"<svg><linearGradient id=g x2="0"><stop stop-color=red /><stop stop-color=blue /></linearGradient></svg>"#;
        assert_eq!(
            painted(point, &target(square()), 1.0).0,
            Ok(Some(Shade::Colour(Rgba::from_rgba8(0, 0, 255, 255))))
        );
    }

    #[test]
    fn a_box_with_no_height_has_nothing_to_be_a_fraction_of() {
        let flat = Rect {
            x: 0.0,
            y: 5.0,
            width: 10.0,
            height: 0.0,
        };
        assert_eq!(painted(TWO, &target(flat), 1.0).0, Ok(None));
    }

    #[test]
    fn spread_methods_and_what_is_not_a_value() {
        let markup = |spread: &str| {
            format!(
                r#"<svg><linearGradient id=g spreadMethod="{spread}"><stop stop-color=red /><stop offset=1 stop-color=blue /></linearGradient></svg>"#
            )
        };
        for (spread, extend, recorded) in [
            ("reflect", Extend::Reflect, 0),
            ("repeat", Extend::Repeat, 0),
            ("pad", Extend::Pad, 0),
            ("mirror", Extend::Pad, 1),
        ] {
            let (found, issues) = painted(&markup(spread), &target(square()), 1.0);
            assert_eq!(ramp_of(found).line().extend, extend, "{spread}");
            assert_eq!(issues.len(), recorded, "{spread}");
        }
    }

    #[test]
    fn an_href_is_recorded_and_bad_values_are_their_defaults() {
        let markup = r##"<svg><linearGradient id=g href="#h" x1="wide" gradientUnits="box" gradientTransform="spin(3)"><stop stop-color=red /><stop offset=1 stop-color=blue /></linearGradient></svg>"##;
        let (found, issues) = painted(markup, &target(square()), 1.0);
        let ramp = ramp_of(found);
        assert!(close(ramp.along(10.0, 0.0), 0.0));
        assert_eq!(issues.len(), 4, "{issues:?}");
    }

    #[test]
    fn stops_past_the_bound_refuse_the_drawing() {
        let stops = "<stop/>".repeat(MOST_STOPS + 1);
        let markup = format!("<svg><linearGradient id=g>{stops}</linearGradient></svg>");
        assert!(painted(&markup, &target(square()), 1.0).0.is_err());
        let stops = "<stop/>".repeat(MOST_STOPS);
        let markup = format!("<svg><linearGradient id=g>{stops}</linearGradient></svg>");
        assert!(painted(&markup, &target(square()), 1.0).0.is_ok());
    }
}
