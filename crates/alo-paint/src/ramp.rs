/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A colour that runs along a line drawn anywhere in the plane: what an SVG
//! `<linearGradient>` fills a shape with (queue item 275, ADR 0107).
//!
//! A CSS gradient is measured against a box and runs at an angle through its
//! middle ([`crate::paint`]). An SVG one runs from one point to another in
//! a coordinate system of its own — the shape's bounding box, or the user
//! space it is drawn in — under the shape's transform and the gradient's
//! own. Whatever those transforms are, how far along the gradient a point is
//! is an affine function of where the point is, `t = a·x + b·y + c`, because
//! an affine map of a projection onto a line is still one. So that is what a
//! ramp carries: the three numbers, in the coordinates of the drawing it
//! belongs to, and its colours. Moving the drawing to where its box is moves
//! the three numbers with it ([`Ramp::transformed`]), and paint asks the
//! colour at the middle of each pixel, as it asks a CSS gradient's.
//!
//! The colours and what lies past the line's ends are a [`ColourLine`], the
//! one a `COLR` glyph's gradient uses: its stops are mixed as straight
//! colours and premultiplied afterwards, which is how Chrome mixes an SVG
//! gradient's too (measured, `cases/svg-linear-gradients`).

use crate::colour_paint::{ColourLine, Extend};
use alo_value::{Matrix, Rgba};
use core::fmt;

/// A colour along a line: `t = a·x + b·y + c` at the point `(x, y)`, and
/// the colours `t` runs through.
#[derive(Debug, Clone, PartialEq)]
pub struct Ramp {
    /// How much `t` grows for each unit across.
    a: f32,
    /// How much `t` grows for each unit down.
    b: f32,
    /// What `t` is at the origin.
    c: f32,
    /// The colours, and what is past the ends.
    line: ColourLine,
}

impl Ramp {
    /// A ramp from `start`, where `t` is nothing, to `end`, where it is
    /// one — both in a space `to_drawing` maps into the drawing's — with
    /// lines of one colour at right angles to the line between them in that
    /// space.
    ///
    /// [`None`] when the two points are the same, which has no direction,
    /// or `to_drawing` flattens the plane onto a line, which has no inverse.
    pub fn between(
        start: (f32, f32),
        end: (f32, f32),
        to_drawing: Matrix,
        line: ColourLine,
    ) -> Option<Self> {
        let (dx, dy) = (end.0 - start.0, end.1 - start.1);
        let length = dx.mul_add(dx, dy * dy);
        if !(length > 0.0 && length.is_finite()) {
            return None;
        }
        let own = Self {
            a: dx / length,
            b: dy / length,
            c: -start.0.mul_add(dx, start.1 * dy) / length,
            line,
        };
        own.transformed(to_drawing)
    }

    /// The same ramp in the coordinates `matrix` maps these into: at each
    /// point, the colour this ramp has where the point came from.
    ///
    /// [`None`] when the matrix flattens the plane, or the numbers stop
    /// being finite.
    #[must_use]
    pub fn transformed(&self, matrix: Matrix) -> Option<Self> {
        let back = matrix.inverted()?;
        // t(back(x, y)), with back(x, y) = (ba·x + bc·y + be, bb·x + bd·y + bf).
        let a = self.a.mul_add(back.a, self.b * back.b);
        let b = self.a.mul_add(back.c, self.b * back.d);
        let c = self.a.mul_add(back.e, self.b.mul_add(back.f, self.c));
        (a.is_finite() && b.is_finite() && c.is_finite()).then(|| Self {
            a,
            b,
            c,
            line: self.line.clone(),
        })
    }

    /// How far along the line the point `(x, y)` is: nothing at its start,
    /// one at its end.
    pub fn along(&self, x: f32, y: f32) -> f32 {
        self.a.mul_add(x, self.b.mul_add(y, self.c))
    }

    /// The colour at the point `(x, y)`, as a straight colour.
    pub fn at(&self, x: f32, y: f32) -> Rgba {
        let [red, green, blue, alpha] = self.line.at(self.along(x, y));
        if alpha <= 0.0 {
            return Rgba::TRANSPARENT;
        }
        Rgba {
            red: red / alpha,
            green: green / alpha,
            blue: blue / alpha,
            alpha,
        }
    }

    /// Whether filling with this would change nothing: every stop is
    /// invisible.
    pub fn is_invisible(&self) -> bool {
        self.line
            .stops
            .iter()
            .all(|(_, colour)| colour.is_invisible())
    }

    /// The same ramp with every stop faded by `opacity`, from nothing to
    /// one: a shape's own `opacity`, where it draws one thing.
    #[must_use]
    pub fn faded(&self, opacity: f32) -> Self {
        let mut faded = self.clone();
        for (_, colour) in &mut faded.line.stops {
            colour.alpha *= opacity.clamp(0.0, 1.0);
        }
        faded
    }

    /// The colours, and what is past the ends.
    pub fn line(&self) -> &ColourLine {
        &self.line
    }
}

impl fmt::Display for Ramp {
    /// `ramp(0.01x + 0y - 0.5, rgb(…) 0, rgb(…) 1, pad)`: the field, every
    /// stop, and what is past the ends.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Nothing added to each, so a negative zero reads as nothing.
        write!(
            f,
            "ramp({}x + {}y + {}",
            self.a + 0.0,
            self.b + 0.0,
            self.c + 0.0
        )?;
        for (offset, colour) in &self.line.stops {
            write!(f, ", {colour} {offset}")?;
        }
        let extend = match self.line.extend {
            Extend::Pad => "pad",
            Extend::Repeat => "repeat",
            Extend::Reflect => "reflect",
        };
        write!(f, ", {extend})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn red_to_blue(extend: Extend) -> ColourLine {
        ColourLine {
            stops: vec![
                (0.0, Rgba::from_rgba8(255, 0, 0, 255)),
                (1.0, Rgba::from_rgba8(0, 0, 255, 255)),
            ],
            extend,
        }
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1.0e-4
    }

    #[test]
    fn a_ramp_runs_from_its_start_to_its_end_and_is_even_across_it() {
        let Some(ramp) = Ramp::between(
            (10.0, 0.0),
            (30.0, 0.0),
            Matrix::IDENTITY,
            red_to_blue(Extend::Pad),
        ) else {
            panic!("a ramp");
        };
        assert!(close(ramp.along(10.0, 5.0), 0.0));
        assert!(close(ramp.along(20.0, -40.0), 0.5));
        assert!(close(ramp.along(30.0, 99.0), 1.0));
        let middle = ramp.at(20.0, 0.0);
        assert!(close(middle.red, 0.5) && close(middle.blue, 0.5));
        assert_eq!(ramp.at(0.0, 0.0), Rgba::from_rgba8(255, 0, 0, 255));
        assert_eq!(ramp.at(50.0, 0.0), Rgba::from_rgba8(0, 0, 255, 255));
    }

    #[test]
    fn a_squashed_ramp_keeps_its_lines_of_one_colour_squashed_with_it() {
        // A diagonal ramp across a unit square, stretched to 100 by 10: the
        // lines of one colour run corner to corner of the stretched box, not
        // at right angles to its diagonal.
        let stretch = Matrix {
            a: 100.0,
            d: 10.0,
            ..Matrix::IDENTITY
        };
        let Some(ramp) = Ramp::between((0.0, 0.0), (1.0, 1.0), stretch, red_to_blue(Extend::Pad))
        else {
            panic!("a ramp");
        };
        assert!(close(ramp.along(100.0, 0.0), 0.5));
        assert!(close(ramp.along(0.0, 10.0), 0.5));
        assert!(close(ramp.along(50.0, 5.0), 0.5));
        assert!(close(ramp.along(100.0, 10.0), 1.0));
    }

    #[test]
    fn moving_a_ramp_moves_its_colours_with_it() {
        let Some(ramp) = Ramp::between(
            (0.0, 0.0),
            (10.0, 0.0),
            Matrix::IDENTITY,
            red_to_blue(Extend::Pad),
        ) else {
            panic!("a ramp");
        };
        let Some(moved) = ramp.transformed(Matrix::translation(100.0, 7.0)) else {
            panic!("moved");
        };
        assert!(close(moved.along(105.0, 0.0), 0.5));
        assert!(close(moved.along(110.0, 0.0), 1.0));
    }

    #[test]
    fn a_ramp_with_no_length_or_a_flattened_plane_is_none() {
        let line = red_to_blue(Extend::Pad);
        assert_eq!(
            Ramp::between((1.0, 1.0), (1.0, 1.0), Matrix::IDENTITY, line.clone()),
            None
        );
        let flat = Matrix {
            d: 0.0,
            ..Matrix::IDENTITY
        };
        assert_eq!(Ramp::between((0.0, 0.0), (1.0, 0.0), flat, line), None);
    }

    #[test]
    fn past_its_ends_a_ramp_repeats_or_reflects_as_asked() {
        let at = |extend| {
            Ramp::between(
                (0.0, 0.0),
                (10.0, 0.0),
                Matrix::IDENTITY,
                red_to_blue(extend),
            )
            .map(|ramp| ramp.at(12.5, 0.0).blue)
        };
        assert!(at(Extend::Pad).is_some_and(|blue| close(blue, 1.0)));
        assert!(at(Extend::Repeat).is_some_and(|blue| close(blue, 0.25)));
        assert!(at(Extend::Reflect).is_some_and(|blue| close(blue, 0.75)));
    }

    #[test]
    fn a_translucent_stop_is_mixed_as_a_straight_colour() {
        let line = ColourLine {
            stops: vec![
                (0.0, Rgba::from_rgba8(255, 0, 0, 255)),
                (1.0, Rgba::from_rgba8(0, 0, 255, 0)),
            ],
            extend: Extend::Pad,
        };
        let Some(ramp) = Ramp::between((0.0, 0.0), (1.0, 0.0), Matrix::IDENTITY, line) else {
            panic!("a ramp");
        };
        let middle = ramp.at(0.5, 0.0);
        assert!(close(middle.red, 0.5) && close(middle.blue, 0.5) && close(middle.alpha, 0.5));
        assert!(!ramp.is_invisible());
        assert_eq!(ramp.at(2.0, 0.0), Rgba::TRANSPARENT);
    }

    #[test]
    fn a_faded_ramp_fades_every_stop_and_to_nothing_is_invisible() {
        let Some(ramp) = Ramp::between(
            (0.0, 0.0),
            (1.0, 0.0),
            Matrix::IDENTITY,
            red_to_blue(Extend::Pad),
        ) else {
            panic!("a ramp");
        };
        assert!(close(ramp.faded(0.5).at(0.0, 0.0).alpha, 0.5));
        assert!(ramp.faded(0.0).is_invisible());
        assert!(!ramp.is_invisible());
    }

    #[test]
    fn a_ramp_reads_as_its_field_its_stops_and_its_ends() {
        let Some(ramp) = Ramp::between(
            (0.0, 0.0),
            (2.0, 0.0),
            Matrix::IDENTITY,
            red_to_blue(Extend::Reflect),
        ) else {
            panic!("a ramp");
        };
        assert_eq!(
            ramp.to_string(),
            "ramp(0.5x + 0y + 0, rgb(255 0 0) 0, rgb(0 0 255) 1, reflect)"
        );
    }
}
