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
//! - **a basic shape or a `<path>`**: filled, then stroked, as its style says
//!   (SVG's paint order, which `paint-order` could change and does not yet);
//! - **something that is never drawn where it stands** (`<defs>`, `<title>`,
//!   a gradient, a clip path): skipped, silently, because skipping it is what
//!   drawing it correctly means;
//! - **anything else**: not drawn, and recorded, with the item that will draw
//!   it or the reason it never will (ADR 0022 § 7).
//!
//! The walk keeps its own stack rather than recursing, and every count a page
//! could inflate is bounded before the work it causes ([`crate::bounds`]).

use crate::bbox::object_bounding_box;
use crate::bounds::{DEEPEST, MOST_DASHES, MOST_ELEMENTS, MOST_GROUPS_DEEP, MOST_SEGMENTS};
use crate::fill::fill_of;
use crate::length::Viewport;
use crate::paint::alpha;
use crate::shape::{Geometry, Refusal, fills};
use crate::stroke::stroke_of;
use crate::viewport::AspectRatio;
use alo_dom::{Document, Element, Namespace, NodeId};
use alo_paint::{Drawing, DrawingItem, FillRule, Path};
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

        self.refused(element, style);
        let opacity = alpha(style.get("opacity")).unwrap_or(1.0);

        if container {
            let matrix = self.under_own_transform(name, style, None, matrix);
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

    /// The transform an element draws under: its own `transform` property —
    /// the attribute, unless a stylesheet replaced it — inside what its
    /// ancestors built up. `fill_box` is a shape's object bounding box, which
    /// `transform-box: fill-box` measures against.
    fn under_own_transform(
        &mut self,
        name: &str,
        style: &ComputedStyle,
        fill_box: Option<crate::bbox::Rect>,
        matrix: Matrix,
    ) -> Matrix {
        crate::transform::own(name, style, self.viewport, fill_box, &mut self.issues)
            .map_or(matrix, |own| own.then(matrix))
    }

    /// Record what an element asks for that ADR 0022 § 7 refuses, because this
    /// is where it is first read.
    fn refused(&mut self, element: &Element, style: &ComputedStyle) {
        let name = &*element.name.local;
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
    }

    /// A basic shape or a `<path>`, filled and then stroked, as its style
    /// says, if it is visible.
    ///
    /// The stroke is outlined in **user space** and the outline transformed
    /// with the shape, so a stroke under a squashing or skewing transform is
    /// squashed and skewed with it, as SVG says, rather than drawn at an even
    /// width over a transformed path.
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
        if hidden || opacity <= 0.0 {
            return Ok(());
        }
        let name = &*element.name.local;
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
        // Measured before anything else can be refused, because the box is the
        // shape's own geometry, untransformed and unstroked.
        let matrix = self.under_own_transform(name, style, object_bounding_box(&path), matrix);
        let fill = if fills(name, &path) {
            fill_of(style, &mut self.issues)
        } else {
            None
        };
        let stroke = stroke_of(style, self.viewport, &mut self.issues)?;
        if fill.is_none() && stroke.is_none() {
            return Ok(());
        }
        self.not_applied(element, style);
        self.count(&path)?;

        let outline = match stroke {
            None => None,
            Some(stroke) => {
                if let Some(dashes) = &stroke.stroke.dashes {
                    let dashes = crate::dashes::count(&path, &dashes.lengths);
                    #[expect(
                        clippy::cast_precision_loss,
                        reason = "a bound of a few thousand is exact in a double"
                    )]
                    let most = MOST_DASHES as f64;
                    // A count that is not a number is one nobody can vouch
                    // for, and is refused with the ones that are too large.
                    if dashes.is_nan() || dashes > most {
                        return Err(format!(
                            "<{name}>: a stroke-dasharray that would cut one path into more than {MOST_DASHES} dashes"
                        ));
                    }
                }
                alo_paint::raster::outline(&path, &stroke.stroke, resolution(matrix))
                    .map(|outline| (outline, stroke.color))
            }
        };
        if let Some((outline, _)) = &outline {
            self.count(outline)?;
        }

        // A shape's own `opacity` fades what it draws as one. With one thing
        // drawn that is the same as fading its colour, and costs no page of
        // pixels; with a fill and a stroke, which overlap, it is a group, or
        // the fill would show through the stroke's inner half.
        let grouped = opacity < 1.0 && fill.is_some() && outline.is_some();
        let fade = if grouped {
            self.open_group(opacity)?;
            1.0
        } else {
            opacity
        };
        if let Some(fill) = fill {
            self.paint(path.transformed(matrix), fill.color, fill.rule, fade);
        }
        if let Some((outline, color)) = outline {
            self.paint(outline.transformed(matrix), color, FillRule::NonZero, fade);
        }
        if grouped {
            self.close_group();
        }
        Ok(())
    }

    /// One shape, filled, faded by `fade`, unless that leaves nothing to see.
    fn paint(&mut self, path: Path, color: Rgba, rule: FillRule, fade: f32) {
        let color = Rgba {
            alpha: color.alpha * fade,
            ..color
        };
        if !color.is_invisible() {
            self.drawing.push(DrawingItem::Fill { path, color, rule });
        }
    }

    /// Count a path the drawing is about to hold against the drawing's bound.
    fn count(&mut self, path: &Path) -> Result<(), String> {
        self.segments += path.segments().len();
        if self.segments > MOST_SEGMENTS {
            return Err(format!("more than {MOST_SEGMENTS} path segments"));
        }
        Ok(())
    }

    /// Record what a drawn shape asks for that changes how it is painted and
    /// is not applied: the order of its fill and stroke, and a stroke that
    /// keeps its width whatever the transform.
    fn not_applied(&mut self, element: &Element, style: &ComputedStyle) {
        let name = &*element.name.local;
        for (property, ordinary) in [("paint-order", "normal"), ("vector-effect", "none")] {
            let value = element.attr(property).or_else(|| style.get(property));
            if value.is_some_and(|value| !value.trim().eq_ignore_ascii_case(ordinary)) {
                self.issues
                    .push(format!("<{name} {property}>: not applied yet"));
            }
        }
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

/// How many pixels one user unit becomes along the longer of its two axes,
/// which is how finely a stroke's offset curves must be made to look smooth
/// once they are drawn.
fn resolution(matrix: Matrix) -> f32 {
    matrix.a.hypot(matrix.b).max(matrix.c.hypot(matrix.d))
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
