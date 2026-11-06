/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `COLR` version 1 glyph's paint graph, in this engine's vocabulary
//! (queue item 785).
//!
//! A version 1 glyph is not layers of flat colour but a small program:
//! fill a glyph's outline (or a clip of it) with a solid colour or a
//! gradient — linear through three points, radial between two circles, or
//! swept around a point — under transforms, inside clips, in groups put
//! together by a compositing mode. The rented parser walks the program
//! (`ttf-parser`'s `colr::Painter`, in [`crate::glyph::painted`], the one
//! file allowed to name it); what it walks is recorded here as
//! [`Step`]s, in the font's own units with `y` up, and carried out by
//! [`crate::painted_glyph`].
//!
//! The geometry of each gradient is the specification's (OpenType `COLR`,
//! *Gradients*): reading it is the format, and the arithmetic below is ours.

use crate::path::Path;
use alo_value::Rgba;

/// One step of a glyph's paint program, as the parser walks it.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// The outline the next step uses: as a clip ([`Step::PushClip`]), or
    /// filled by the next [`Step::Paint`] when no clip takes it — in font
    /// units, `y` up.
    Outline(Path),
    /// Fill: the outline waiting, or else everything the clips in force let
    /// through.
    Paint(Fill),
    /// Clip what follows to the outline waiting.
    PushClip,
    /// Clip what follows to a box, `(left, bottom, right, top)` in font
    /// units.
    PushClipBox(f32, f32, f32, f32),
    /// The end of the innermost clip.
    PopClip,
    /// Draw what follows on a surface of its own, put onto what is beneath
    /// by `Mode` when it ends.
    PushLayer(Mode),
    /// The end of the innermost layer.
    PopLayer,
    /// What follows is in coordinates this matrix maps into the ones before
    /// it, as `[xx, yx, xy, yy, dx, dy]`.
    PushTransform([f32; 6]),
    /// The end of the innermost transform.
    PopTransform,
}

/// What a [`Step::Paint`] fills with, in font units.
#[derive(Debug, Clone, PartialEq)]
pub enum Fill {
    /// One colour.
    Solid(Rgba),
    /// Linear: colour stop 0 at `p0`, 1 at `p1`, the lines of one colour
    /// parallel to `p0`–`p2`.
    Linear {
        /// The start, the end and the rotation point.
        points: [(f32, f32); 3],
        /// The colour line.
        line: ColourLine,
    },
    /// Radial: between the circle `(x0, y0, r0)` at stop 0 and `(x1, y1, r1)`
    /// at stop 1.
    Radial {
        /// The first circle, then the second, each `(x, y, r)`.
        circles: [(f32, f32, f32); 2],
        /// The colour line.
        line: ColourLine,
    },
    /// Swept around a centre, counter-clockwise in the font's coordinates,
    /// from one angle to another, as the table writes them: half-turns less
    /// one (`−1` is 0°, `1` a whole turn).
    Sweep {
        /// The centre.
        centre: (f32, f32),
        /// The start and end angles, in half-turns.
        angles: (f32, f32),
        /// The colour line.
        line: ColourLine,
    },
}

/// A gradient's colours along it, and what is past its ends.
#[derive(Debug, Clone, PartialEq)]
pub struct ColourLine {
    /// Each stop's offset and colour, in the order the table lists them.
    pub stops: Vec<(f32, Rgba)>,
    /// What is drawn before the first stop and after the last.
    pub extend: Extend,
}

/// What a gradient draws past its ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extend {
    /// The end colours, on.
    Pad,
    /// The line again, and again.
    Repeat,
    /// The line back, then forth.
    Reflect,
}

/// How a layer is put onto what is beneath it (`COLR`'s `CompositeMode`):
/// Porter and Duff's operators, then the W3C blend modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(missing_docs, reason = "each is the mode the specification names")]
pub enum Mode {
    Clear,
    Source,
    Destination,
    SourceOver,
    DestinationOver,
    SourceIn,
    DestinationIn,
    SourceOut,
    DestinationOut,
    SourceAtop,
    DestinationAtop,
    Xor,
    Plus,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Multiply,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl ColourLine {
    /// The colour at `t` along the line, its extend applied, premultiplied
    /// once mixed: colours are mixed as straight colours between stops, as
    /// Chrome 157 mixes them.
    pub fn at(&self, t: f32) -> [f32; 4] {
        let mut stops: Vec<(f32, Rgba)> = self.stops.clone();
        stops.sort_by(|one, other| one.0.total_cmp(&other.0));
        let (Some(first), Some(last)) = (stops.first().copied(), stops.last().copied()) else {
            return [0.0; 4];
        };
        let span = last.0 - first.0;
        let t = if span <= 0.0 || !t.is_finite() {
            t
        } else {
            match self.extend {
                Extend::Pad => t,
                Extend::Repeat => first.0 + (t - first.0).rem_euclid(span),
                Extend::Reflect => {
                    let along = (t - first.0).rem_euclid(2.0 * span);
                    first.0
                        + if along > span {
                            2.0 * span - along
                        } else {
                            along
                        }
                }
            }
        };
        if t <= first.0 {
            return premultiplied(first.1);
        }
        if t >= last.0 {
            return premultiplied(last.1);
        }
        for pair in stops.windows(2) {
            let [(from, below), (to, above)] = pair else {
                continue;
            };
            if t >= *from && t <= *to {
                let part = if to > from {
                    (t - from) / (to - from)
                } else {
                    0.0
                };
                // Mixed as straight colours, then premultiplied: Chrome 157's
                // way, where the specification's premultiplied mix was 22
                // levels off a stop at 0.6 (queue item 785).
                let mix = |a: f32, b: f32| (b - a).mul_add(part, a);
                return premultiplied(Rgba {
                    red: mix(below.red, above.red),
                    green: mix(below.green, above.green),
                    blue: mix(below.blue, above.blue),
                    alpha: mix(below.alpha, above.alpha),
                });
            }
        }
        premultiplied(last.1)
    }
}

/// A colour premultiplied by its alpha, as `[r, g, b, a]` from 0 to 1.
pub fn premultiplied(colour: Rgba) -> [f32; 4] {
    let alpha = colour.alpha.clamp(0.0, 1.0);
    [
        colour.red * alpha,
        colour.green * alpha,
        colour.blue * alpha,
        alpha,
    ]
}

impl Fill {
    /// Its colour at the point `(x, y)` in font units, premultiplied.
    pub fn at(&self, x: f32, y: f32) -> [f32; 4] {
        match self {
            Fill::Solid(colour) => premultiplied(*colour),
            Fill::Linear { points, line } => line.at(linear_t(*points, (x, y))),
            Fill::Radial { circles, line } => {
                radial_t(*circles, (x, y), line.extend).map_or([0.0; 4], |t| line.at(t))
            }
            Fill::Sweep {
                centre,
                angles,
                line,
            } => line.at(sweep_t(*centre, *angles, (x, y))),
        }
    }
}

/// Where a point is along a three-point linear gradient: `p0`–`p1` turned
/// to stand at right angles to `p0`–`p2`, as the specification draws it.
fn linear_t([p0, p1, p2]: [(f32, f32); 3], point: (f32, f32)) -> f32 {
    // The normal to p0–p2, and p1 projected onto it from p0: the direction
    // and length the colour line runs.
    let (wx, wy) = (p2.0 - p0.0, p2.1 - p0.1);
    let (nx, ny) = (wy, -wx);
    let (vx, vy) = (p1.0 - p0.0, p1.1 - p0.1);
    let normal = nx.mul_add(nx, ny * ny);
    let (dx, dy) = if normal > 0.0 {
        let along = vx.mul_add(nx, vy * ny) / normal;
        (nx * along, ny * along)
    } else {
        (vx, vy)
    };
    let length = dx.mul_add(dx, dy * dy);
    if length <= 0.0 {
        return 0.0;
    }
    (point.0 - p0.0).mul_add(dx, (point.1 - p0.1) * dy) / length
}

/// Where a point is along a two-circle radial gradient: the largest `t`
/// whose circle, between the two, passes through it with a radius not
/// below nothing — or [`None`] where no circle does (drawn clear). Past
/// its ends a padded gradient reaches only as far as its circles do.
fn radial_t(
    [(x0, y0, r0), (x1, y1, r1)]: [(f32, f32, f32); 2],
    (px, py): (f32, f32),
    extend: Extend,
) -> Option<f32> {
    // |p − c(t)| = r(t), with c(t) = c0 + t·(c1 − c0) and r(t) = r0 + t·dr:
    // a·t² − 2b·t + c = 0.
    let (cdx, cdy, dr) = (x1 - x0, y1 - y0, r1 - r0);
    let (pdx, pdy) = (px - x0, py - y0);
    let a = cdx.mul_add(cdx, cdy * cdy) - dr * dr;
    let b = pdx.mul_add(cdx, pdy * cdy) + r0 * dr;
    let c = pdx.mul_add(pdx, pdy * pdy) - r0 * r0;
    let candidates: Vec<f32> = if a.abs() < 1e-6 {
        if b.abs() < 1e-12 {
            Vec::new()
        } else {
            vec![c / (2.0 * b)]
        }
    } else {
        let discriminant = b.mul_add(b, -(a * c));
        if discriminant < 0.0 {
            Vec::new()
        } else {
            let root = discriminant.sqrt();
            vec![(b + root) / a, (b - root) / a]
        }
    };
    candidates
        .into_iter()
        .filter(|t| dr.mul_add(*t, r0) >= 0.0)
        .filter(|t| extend != Extend::Pad || t.is_finite())
        .reduce(f32::max)
}

/// Where a point is around a sweep: its angle about the centre,
/// counter-clockwise from the `x` axis, as a fraction of the way from the
/// start angle to the end one.
fn sweep_t(centre: (f32, f32), (start, end): (f32, f32), (px, py): (f32, f32)) -> f32 {
    let angle = (py - centre.1).atan2(px - centre.0).to_degrees();
    let angle = if angle < 0.0 { angle + 360.0 } else { angle };
    // Written with a bias of one, so that a whole turn fits the table's
    // number: a value `v` is `(v + 1) · 180` degrees.
    let (start, end) = ((start + 1.0) * 180.0, (end + 1.0) * 180.0);
    if (end - start).abs() < f32::EPSILON {
        return if angle < start { 0.0 } else { 1.0 };
    }
    (angle - start) / (end - start)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(extend: Extend) -> ColourLine {
        ColourLine {
            stops: vec![(0.0, Rgba::BLACK), (1.0, Rgba::WHITE)],
            extend,
        }
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn a_linear_gradient_runs_at_right_angles_to_its_third_point() {
        // p0 at the origin, p1 at (100, 100), p2 straight up: the line runs
        // along x only, its length p1's projection, 100.
        let points = [(0.0, 0.0), (100.0, 100.0), (0.0, 50.0)];
        assert!(close(linear_t(points, (50.0, 0.0)), 0.5));
        assert!(close(linear_t(points, (50.0, 80.0)), 0.5));
        assert!(close(linear_t(points, (100.0, -20.0)), 1.0));
    }

    #[test]
    fn a_radial_gradient_is_the_largest_circle_through_a_point() {
        // Concentric, 0 to 100: a point 25 out is a quarter of the way.
        let circles = [(0.0, 0.0, 0.0), (0.0, 0.0, 100.0)];
        let t = radial_t(circles, (0.0, 25.0), Extend::Pad);
        assert!(t.is_some_and(|t| close(t, 0.25)), "{t:?}");
        // Two circles, the second off to one side and larger: (100, 50) is
        // on the second, but on a larger one past it too, at 62/42 — the
        // circle at (147.6, 0) of radius 69.05.
        let circles = [(0.0, 0.0, 10.0), (100.0, 0.0, 50.0)];
        let t = radial_t(circles, (100.0, 50.0), Extend::Pad);
        assert!(t.is_some_and(|t| close(t, 62.0 / 42.0)), "{t:?}");
    }

    #[test]
    fn a_sweep_goes_round_counter_clockwise_from_its_start() {
        // −1 and 1 as the table writes them: 0° to 360°.
        let angles = (-1.0, 1.0);
        assert!(close(sweep_t((0.0, 0.0), angles, (1.0, 0.0)), 0.0));
        assert!(close(sweep_t((0.0, 0.0), angles, (0.0, 1.0)), 0.25));
        assert!(close(sweep_t((0.0, 0.0), angles, (-1.0, 0.0)), 0.5));
        assert!(close(sweep_t((0.0, 0.0), angles, (0.0, -1.0)), 0.75));
    }

    #[test]
    fn past_its_ends_a_line_pads_repeats_or_reflects() {
        let grey = |line: &ColourLine, t: f32| line.at(t)[0];
        assert!(close(grey(&line(Extend::Pad), 1.25), 1.0));
        assert!(close(grey(&line(Extend::Repeat), 1.25), 0.25));
        assert!(close(grey(&line(Extend::Reflect), 1.25), 0.75));
        assert!(close(grey(&line(Extend::Reflect), -0.25), 0.25));
    }

    #[test]
    fn colours_are_mixed_straight_then_premultiplied() {
        // Red opaque to blue clear: half way is a purple at half alpha,
        // premultiplied — as Chrome 157 draws it.
        let line = ColourLine {
            stops: vec![
                (0.0, Rgba::from_rgba8(255, 0, 0, 255)),
                (1.0, Rgba::from_rgba8(0, 0, 255, 0)),
            ],
            extend: Extend::Pad,
        };
        let [red, green, blue, alpha] = line.at(0.5);
        assert!(
            close(red, 0.25) && close(green, 0.0) && close(blue, 0.25) && close(alpha, 0.5),
            "{red} {green} {blue} {alpha}"
        );
    }
}
