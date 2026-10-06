/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The `transform` property on an element inside an `<svg>` (item 287): the
//! box it is measured against and the point it turns about.
//!
//! **One property, one grammar.** The `transform` attribute is the property's
//! presentation attribute, and the cascade holds it as the CSS `matrix()` it
//! comes to (`alo-style`'s `presentation.rs`), so what is read here is the
//! computed property whoever wrote it. A stylesheet's `transform` therefore
//! *replaces* the element's attribute; the two compose only across
//! ancestors, where a child's transform is drawn inside its parent's.
//!
//! **What SVG changes about CSS's transform** is where it is measured, because
//! an element inside an `<svg>` has no CSS box:
//!
//! - `transform-box` is the box: `view-box` (the initial value), the nearest
//!   viewport at the origin of its user space and the size of its `viewBox`;
//!   or `fill-box`, the element's object bounding box ([`crate::bbox`]).
//!   `content-box` is `fill-box` and `border-box` is `stroke-box`, as CSS
//!   Transforms says for an element with no CSS box.
//! - `transform-origin` is a point in that box, `0 0` unless the author says
//!   otherwise — not CSS's `50% 50%` — so `rotate(90deg)` turns a shape about
//!   the origin of its user space, as the attribute always has.
//! - A `translate`'s percentages are of that box's size.
//!
//! **Not yet:** `fill-box` on a container (`<g>`), whose box is the union of
//! everything it holds under their own transforms, and `stroke-box`, which
//! adds the stroke. Each is recorded, item 288, and measured against the
//! nearest box this engine can give — the view box for a container, the fill
//! box for a shape, whose centre a stroke of even width does not move.

use crate::bbox::Rect;
use crate::length::Viewport;
use alo_style::ComputedStyle;
use alo_value::{LengthPercentage, Matrix};

/// The transform an element's own `transform` property draws it under, in its
/// parent's user space, or [`None`] when it has none, or none that can be
/// drawn — which is recorded.
///
/// `fill_box` is the element's object bounding box, when it is a shape and has
/// one; a container passes [`None`].
pub fn own(
    name: &str,
    style: &ComputedStyle,
    viewport: Viewport,
    fill_box: Option<Rect>,
    issues: &mut Vec<String>,
) -> Option<Matrix> {
    let text = style.get("transform")?;
    let Some(transform) = alo_value::parse_transform(text) else {
        issues.push(format!(
            "<{name}>: transform {text:?} is not a transform this engine draws, so it is ignored"
        ));
        return None;
    };
    if transform.is_empty() {
        return None;
    }
    let reference = reference_box(name, style, viewport, fill_box, issues);
    let (across, down) = origin(name, style, issues);
    let metrics = style.metrics();
    let origin = (
        reference.x + across.to_px(metrics, reference.width),
        reference.y + down.to_px(metrics, reference.height),
    );
    let matrix = transform.matrix(metrics, (reference.width, reference.height), origin);
    let finite = [matrix.a, matrix.b, matrix.c, matrix.d, matrix.e, matrix.f]
        .iter()
        .all(|value| value.is_finite());
    if !finite {
        issues.push(format!(
            "<{name}>: transform {text:?} does not come to a finite transform, so it is ignored"
        ));
        return None;
    }
    Some(matrix)
}

/// The box `transform-box` names, as SVG 2 measures it for an element with no
/// CSS box.
fn reference_box(
    name: &str,
    style: &ComputedStyle,
    viewport: Viewport,
    fill_box: Option<Rect>,
    issues: &mut Vec<String>,
) -> Rect {
    // The view box sits at the origin of the user space the `viewBox` set up,
    // not at the `viewBox`'s own corner, and is the `viewBox`'s size.
    let view_box = Rect {
        x: 0.0,
        y: 0.0,
        width: viewport.width,
        height: viewport.height,
    };
    let Some(text) = style.get("transform-box") else {
        return view_box;
    };
    let word = text.trim().to_ascii_lowercase();
    match word.as_str() {
        "view-box" => view_box,
        "fill-box" | "content-box" => fill_box.unwrap_or_else(|| {
            issues.push(format!(
                "<{name}>: transform-box: {word} on a container is not measured yet (item 288), so the view box is"
            ));
            view_box
        }),
        "stroke-box" | "border-box" => {
            issues.push(format!(
                "<{name}>: transform-box: {word} is not measured yet (item 288), so the fill box is"
            ));
            fill_box.unwrap_or(view_box)
        }
        _ => {
            issues.push(format!(
                "<{name}>: transform-box {text:?} is not a box, so the view box is used"
            ));
            view_box
        }
    }
}

/// `transform-origin`, or `0 0` — SVG's initial value for an element with no
/// CSS box — when there is none or it cannot be read.
fn origin(
    name: &str,
    style: &ComputedStyle,
    issues: &mut Vec<String>,
) -> (LengthPercentage, LengthPercentage) {
    let zero = (
        LengthPercentage::Percentage(0.0),
        LengthPercentage::Percentage(0.0),
    );
    let Some(text) = style.get("transform-origin") else {
        return zero;
    };
    alo_value::parse_transform_origin(text).unwrap_or_else(|| {
        issues.push(format!(
            "<{name}>: transform-origin {text:?} is not a point, so 0 0 is used"
        ));
        zero
    })
}
