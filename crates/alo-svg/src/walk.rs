/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! From an outermost `<svg>` to a drawing: the walk.
//!
//! In document order, which is SVG's paint order — a later shape is drawn over
//! an earlier one. Each element is visited once, with the transform its
//! ancestors built up, and is one of four things:
//!
//! - **a container** (`<g>`, and `<a>`, which draws as one): its children are
//!   drawn under its transform, faded together if it has an `opacity`;
//! - **a basic shape or a `<path>`**: filled, if its style says so;
//! - **something that is never drawn where it stands** (`<defs>`, `<title>`,
//!   a gradient, a clip path): skipped, silently, because skipping it is what
//!   drawing it correctly means;
//! - **anything else**: not drawn, and recorded, with the item that will draw
//!   it or the reason it never will (ADR 0022 § 7).
//!
//! The walk keeps its own stack rather than recursing, and every count a page
//! could inflate is bounded before the work it causes ([`crate::bounds`]).

use crate::bounds::{DEEPEST, MOST_ELEMENTS, MOST_GROUPS_DEEP, MOST_SEGMENTS};
use crate::fill::{alpha, fill_of};
use crate::length::Viewport;
use crate::shape::{Geometry, Refusal};
use crate::viewport::AspectRatio;
use alo_dom::{Document, Element, Namespace, NodeId};
use alo_paint::{Drawing, DrawingItem};
use alo_style::{ComputedStyle, StyleTree};
use alo_value::{Matrix, Rgba};

/// The most groups one drawing may open, however they are arranged.
///
/// Paint composites each group back over a whole page, so a hundred thousand
/// sibling `<g opacity=".5">`s would cost a hundred thousand pages of pixels
/// even though no two are open at once. Sixty-four is more than any icon or
/// illustration uses.
pub const MOST_GROUPS: usize = 64;

/// A drawing, and everything about it that was not drawn.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Drawn {
    /// What to draw, in the box's coordinates.
    pub drawing: Drawing,
    /// What was refused or left out, one line each, for the page's issues.
    pub issues: Vec<String>,
}

enum Step {
    Enter {
        id: NodeId,
        depth: usize,
        matrix: Matrix,
    },
    CloseGroup,
}

struct Walk<'a> {
    document: &'a Document,
    styles: &'a StyleTree,
    viewport: Viewport,
    drawing: Drawing,
    issues: Vec<String>,
    stack: Vec<Step>,
    elements: usize,
    segments: usize,
    groups: usize,
    groups_open: usize,
}

/// Draw what an outermost `<svg>` holds into a box of `size` CSS pixels.
///
/// Never fails: a drawing that passes a bound is refused **whole** and comes
/// back empty with the reason among its issues, and everything else that
/// cannot be drawn is left out and recorded.
pub fn draw(document: &Document, styles: &StyleTree, svg: NodeId, size: (f32, f32)) -> Drawn {
    let mut issues = Vec::new();
    let Some(element) = document.element(svg) else {
        return Drawn::default();
    };
    if !(size.0 > 0.0 && size.1 > 0.0) {
        return Drawn::default();
    }
    let aspect = match element.attr("preserveAspectRatio") {
        None => AspectRatio::default(),
        Some(text) => AspectRatio::parse(text).unwrap_or_else(|| {
            issues.push(format!(
                "<svg preserveAspectRatio={text:?}>: not a value, so xMidYMid meet"
            ));
            AspectRatio::default()
        }),
    };
    // An unreadable `viewBox` is recorded by the box, which read it first; here
    // it is as though it had not been written.
    let view = element.attr("viewBox").and_then(alo_box::svg::view_box);
    let (base, viewport) = match view {
        Some(view) => match aspect.transform(view, size) {
            Some(matrix) => (
                matrix,
                Viewport {
                    width: view.width,
                    height: view.height,
                },
            ),
            // A viewBox with no area shows nothing, and that is not an error.
            None => return Drawn::default(),
        },
        None => (
            Matrix::IDENTITY,
            Viewport {
                width: size.0,
                height: size.1,
            },
        ),
    };

    let mut walk = Walk {
        document,
        styles,
        viewport,
        drawing: Drawing::new(),
        issues,
        stack: Vec::new(),
        elements: 0,
        segments: 0,
        groups: 0,
        groups_open: 0,
    };
    walk.push_children(svg, 1, base);
    match walk.run() {
        Ok(()) => Drawn {
            drawing: walk.drawing,
            issues: walk.issues,
        },
        Err(why) => {
            let mut issues = walk.issues;
            issues.push(format!("an <svg> was not drawn at all: {why}"));
            Drawn {
                drawing: Drawing::new(),
                issues,
            }
        }
    }
}

impl Walk<'_> {
    fn push_children(&mut self, id: NodeId, depth: usize, matrix: Matrix) {
        let children: Vec<NodeId> = self.document.children(id).collect();
        for child in children.into_iter().rev() {
            self.stack.push(Step::Enter {
                id: child,
                depth,
                matrix,
            });
        }
    }

    fn run(&mut self) -> Result<(), String> {
        while let Some(step) = self.stack.pop() {
            match step {
                Step::CloseGroup => self.close_group(),
                Step::Enter { id, depth, matrix } => self.enter(id, depth, matrix)?,
            }
        }
        Ok(())
    }

    fn enter(&mut self, id: NodeId, depth: usize, matrix: Matrix) -> Result<(), String> {
        let Some(element) = self.document.element(id) else {
            // Text and comments draw nothing; `<text>` is item 276.
            return Ok(());
        };
        if element.name.ns != Namespace::Svg {
            return Ok(());
        }
        self.elements += 1;
        if self.elements > MOST_ELEMENTS {
            return Err(format!("more than {MOST_ELEMENTS} elements"));
        }
        if depth > DEEPEST {
            return Err(format!("elements nested more than {DEEPEST} deep"));
        }
        let Some(style) = self.styles.get(id) else {
            return Ok(());
        };
        if style
            .get("display")
            .is_some_and(|display| display.trim().eq_ignore_ascii_case("none"))
        {
            return Ok(());
        }
        let name = &*element.name.local;
        let container = matches!(name, "g" | "a");
        let shape = matches!(
            name,
            "rect" | "circle" | "ellipse" | "line" | "polyline" | "polygon" | "path"
        );
        if !container && !shape {
            if let Some(why) = not_drawn(name) {
                self.issues.push(format!("<{name}> is not drawn: {why}"));
            }
            return Ok(());
        }

        let matrix = self.own_transform(element, style, matrix);
        let opacity = alpha(style.get("opacity")).unwrap_or(1.0);

        if container {
            if opacity <= 0.0 {
                return Ok(());
            }
            if opacity < 1.0 {
                self.open_group(opacity)?;
                self.stack.push(Step::CloseGroup);
            }
            self.push_children(id, depth + 1, matrix);
            return Ok(());
        }
        self.draw_shape(element, style, matrix, opacity)
    }

    /// The transform an element draws under: its own `transform` attribute
    /// inside what its ancestors built up. Also records what the element asks
    /// for that is not applied, because this is where it is first read.
    fn own_transform(
        &mut self,
        element: &Element,
        style: &ComputedStyle,
        matrix: Matrix,
    ) -> Matrix {
        let name = &*element.name.local;
        if style.get("transform").is_some() {
            self.issues.push(format!(
                "<{name}>: the transform property on an element inside an <svg> is not applied yet (item 279)"
            ));
        }
        for refused in [
            "clip-path",
            "mask",
            "filter",
            "marker-start",
            "marker-mid",
            "marker-end",
        ] {
            let value = element.attr(refused).or_else(|| style.get(refused));
            if value.is_some_and(|value| !value.trim().eq_ignore_ascii_case("none")) {
                self.issues
                    .push(format!("<{name} {refused}>: not applied (ADR 0022 § 7)"));
            }
        }
        let Some(text) = element.attr("transform") else {
            return matrix;
        };
        if let Some(own) = crate::transform::parse(text) {
            own.then(matrix)
        } else {
            self.issues.push(format!(
                "<{name} transform={text:?}>: not a transform list, ignored"
            ));
            matrix
        }
    }

    /// A basic shape or a `<path>`, filled, if it is visible and its style
    /// fills it.
    fn draw_shape(
        &mut self,
        element: &Element,
        style: &ComputedStyle,
        matrix: Matrix,
        opacity: f32,
    ) -> Result<(), String> {
        let hidden = style.get("visibility").is_some_and(|visibility| {
            let visibility = visibility.trim();
            visibility.eq_ignore_ascii_case("hidden") || visibility.eq_ignore_ascii_case("collapse")
        });
        if hidden {
            return Ok(());
        }
        let geometry = Geometry {
            element,
            viewport: self.viewport,
            metrics: style.metrics(),
        };
        let path = match geometry.path(&mut self.issues) {
            Ok(Some(path)) => path,
            Ok(None) => return Ok(()),
            Err(Refusal::Shape(why)) => {
                self.issues.push(why);
                return Ok(());
            }
            Err(Refusal::Drawing(why)) => return Err(why),
        };
        let Some(fill) = fill_of(style, &mut self.issues) else {
            return Ok(());
        };
        self.segments += path.segments().len();
        if self.segments > MOST_SEGMENTS {
            return Err(format!("more than {MOST_SEGMENTS} path segments"));
        }
        // A shape's own `opacity` fades its one fill, which is the same as a
        // group of one and costs no page of pixels. When a shape has a stroke
        // as well (item 273) the two overlap, and this becomes a group.
        let color = Rgba {
            alpha: fill.color.alpha * opacity,
            ..fill.color
        };
        if color.is_invisible() {
            return Ok(());
        }
        self.drawing.push(DrawingItem::Fill {
            path: path.transformed(matrix),
            color,
            rule: fill.rule,
        });
        Ok(())
    }

    fn open_group(&mut self, opacity: f32) -> Result<(), String> {
        self.groups += 1;
        if self.groups > MOST_GROUPS {
            return Err(format!("more than {MOST_GROUPS} opacity groups"));
        }
        if self.groups_open >= MOST_GROUPS_DEEP {
            return Err(format!(
                "opacity groups nested more than {MOST_GROUPS_DEEP} deep"
            ));
        }
        self.groups_open += 1;
        self.drawing.push(DrawingItem::PushGroup { opacity });
        Ok(())
    }

    /// The end of a group, or — when nothing was drawn in it — the group
    /// taken back out, so an empty `<g opacity>` costs paint nothing.
    fn close_group(&mut self) {
        self.groups_open = self.groups_open.saturating_sub(1);
        if matches!(
            self.drawing.items().last(),
            Some(DrawingItem::PushGroup { .. })
        ) {
            self.drawing.pop();
        } else {
            self.drawing.push(DrawingItem::PopGroup);
        }
    }
}

/// Why an element that is not a container or a basic shape is not drawn, or
/// [`None`] for one that is never drawn where it stands and so is correct to
/// skip without a word.
fn not_drawn(name: &str) -> Option<&'static str> {
    Some(match name {
        "title" | "desc" | "metadata" | "defs" | "symbol" | "linearGradient" | "radialGradient"
        | "pattern" | "clipPath" | "mask" | "marker" | "filter" | "style" | "script" => {
            return None;
        }
        "use" => "<use> is item 274",
        "text" => "text in SVG is item 276",
        "svg" => "a nested <svg> viewport is item 278",
        "image" => "a picture inside a picture is refused (ADR 0022 § 7)",
        "foreignObject" => "HTML inside SVG is refused (ADR 0022 § 7)",
        "switch" => "<switch> is not drawn yet",
        "animate" | "animateMotion" | "animateTransform" | "set" => {
            "SMIL animation is refused (ADR 0022 § 7)"
        }
        _ => "not an SVG element this engine draws",
    })
}
