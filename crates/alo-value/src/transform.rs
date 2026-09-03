/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `transform`: a shape's points, moved.
//!
//! Every transform CSS has that does not need a third dimension — translate,
//! scale, rotate, skew, and the matrix they all reduce to — is one 2×3 affine
//! matrix. Carrying the *functions* rather than the matrix would have meant
//! resolving them at every use; carrying only the matrix would have lost what
//! the author wrote. So this holds the functions, and hands out the matrix
//! when something with a font and a box asks for it.
//!
//! # Why a percentage needs the box
//!
//! `translate(50%, 0)` is half of the box's own width, and `transform-origin`
//! is `50% 50%` of it unless the author says otherwise. Neither can be
//! resolved when the value is parsed, because a value is parsed before there
//! is a box — which is the same reason `alo_value::LengthPercentage` carries
//! percentages rather than resolving them.
//!
//! # Not here
//!
//! Anything three-dimensional: `perspective`, `rotate3d`, `matrix3d`,
//! `translateZ`. A third dimension changes what a *stacking context* is and
//! what a renderer has to be, and pretending one is flat is a wrong pixel that
//! looks nearly right.

use crate::length::{FontMetrics, LengthPercentage};
use core::fmt;

/// An affine transform, as the six numbers CSS's `matrix()` takes.
///
/// A point `(x, y)` becomes `(a·x + c·y + e, b·x + d·y + f)`. Written the way
/// CSS writes it rather than as rows and columns, so that a `matrix()` in a
/// style sheet and this type are visibly the same thing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix {
    /// How much of the source `x` reaches the result's `x`.
    pub a: f32,
    /// How much of the source `x` reaches the result's `y`.
    pub b: f32,
    /// How much of the source `y` reaches the result's `x`.
    pub c: f32,
    /// How much of the source `y` reaches the result's `y`.
    pub d: f32,
    /// How far across.
    pub e: f32,
    /// How far down.
    pub f: f32,
}

impl Matrix {
    /// The transform that changes nothing.
    pub const IDENTITY: Matrix = Matrix {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    /// A move.
    pub fn translation(across: f32, down: f32) -> Self {
        Self {
            e: across,
            f: down,
            ..Self::IDENTITY
        }
    }

    /// This transform, then another — which is the order CSS composes them in,
    /// left to right along the value.
    #[must_use]
    pub fn then(self, next: Matrix) -> Self {
        Self {
            a: next.a * self.a + next.c * self.b,
            b: next.b * self.a + next.d * self.b,
            c: next.a * self.c + next.c * self.d,
            d: next.b * self.c + next.d * self.d,
            e: next.a * self.e + next.c * self.f + next.e,
            f: next.b * self.e + next.d * self.f + next.f,
        }
    }

    /// Where a point ends up.
    pub fn apply(self, x: f32, y: f32) -> (f32, f32) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// Whether this transform moves nothing.
    ///
    /// Worth asking: almost every box has no transform, and a renderer that
    /// knows that can leave every path alone.
    pub fn is_identity(self) -> bool {
        let same = |left: f32, right: f32| (left - right).abs() < 1.0e-6;
        same(self.a, 1.0)
            && same(self.b, 0.0)
            && same(self.c, 0.0)
            && same(self.d, 1.0)
            && same(self.e, 0.0)
            && same(self.f, 0.0)
    }

    /// The transform that undoes this one, or [`None`] when it flattens
    /// everything onto a line and there is nothing to undo.
    ///
    /// Needed to ask *where a pixel came from*: a gradient under a transform
    /// is measured in the box's own coordinates, so the renderer maps the
    /// pixel back before asking what colour it is.
    pub fn inverted(self) -> Option<Self> {
        let determinant = self.a * self.d - self.b * self.c;
        if determinant.abs() < 1.0e-12 {
            return None;
        }
        Some(Self {
            a: self.d / determinant,
            b: -self.b / determinant,
            c: -self.c / determinant,
            d: self.a / determinant,
            e: (self.c * self.f - self.d * self.e) / determinant,
            f: (self.b * self.e - self.a * self.f) / determinant,
        })
    }

    /// Roughly how much this transform grows what it is applied to.
    ///
    /// The square root of the area it multiplies by. Exact for a uniform scale
    /// and for a rotation; for a skew or a scale that differs by axis it is
    /// the average, which is what a blur radius — a single number — can carry.
    pub fn scale_factor(self) -> f32 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }
}

impl fmt::Display for Matrix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "matrix({}, {}, {}, {}, {}, {})",
            self.a, self.b, self.c, self.d, self.e, self.f,
        )
    }
}

/// One function from a `transform` value.
#[derive(Debug, Clone, PartialEq)]
pub enum Function {
    /// Move, by lengths that may be percentages of the box's own size.
    Translate(LengthPercentage, LengthPercentage),
    /// Grow or shrink, about the transform's origin.
    Scale(f32, f32),
    /// Turn, clockwise, in degrees — clockwise because `y` runs down the page.
    Rotate(f32),
    /// Slant, in degrees: `x` by the first, `y` by the second.
    Skew(f32, f32),
    /// The six numbers themselves.
    Matrix(Matrix),
}

/// A whole `transform` value: its functions, in the order they were written.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Transform {
    /// The functions, left to right.
    pub functions: Vec<Function>,
}

impl Transform {
    /// The one matrix these functions come to, about an origin.
    ///
    /// Each function is read in the coordinates the ones **before** it left
    /// behind, so `rotate(90deg) translateX(10px)` moves ten pixels along an
    /// axis that has already been turned — which puts it ten pixels *down*.
    /// A point therefore passes through them right to left, which is why they
    /// are composed from the end.
    pub fn matrix(&self, metrics: FontMetrics, size: (f32, f32), origin: (f32, f32)) -> Matrix {
        let mut matrix = Matrix::IDENTITY;
        for function in &self.functions {
            matrix = one(function, metrics, size).then(matrix);
        }
        if matrix.is_identity() {
            return Matrix::IDENTITY;
        }
        // Everything happens about the origin, so move it to zero, transform,
        // and move it back. That is what makes `rotate` turn a box about its
        // middle rather than about the top-left corner of the page.
        Matrix::translation(-origin.0, -origin.1)
            .then(matrix)
            .then(Matrix::translation(origin.0, origin.1))
    }

    /// Whether this transform moves nothing at all.
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }
}

/// One function as a matrix.
fn one(function: &Function, metrics: FontMetrics, size: (f32, f32)) -> Matrix {
    match function {
        Function::Translate(across, down) => {
            Matrix::translation(across.to_px(metrics, size.0), down.to_px(metrics, size.1))
        }
        Function::Scale(across, down) => Matrix {
            a: *across,
            d: *down,
            ..Matrix::IDENTITY
        },
        Function::Rotate(degrees) => {
            let radians = degrees.to_radians();
            Matrix {
                a: radians.cos(),
                b: radians.sin(),
                c: -radians.sin(),
                d: radians.cos(),
                ..Matrix::IDENTITY
            }
        }
        Function::Skew(across, down) => Matrix {
            b: down.to_radians().tan(),
            c: across.to_radians().tan(),
            ..Matrix::IDENTITY
        },
        Function::Matrix(matrix) => *matrix,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::length::Length;

    fn metrics() -> FontMetrics {
        FontMetrics::estimated(16.0, 16.0)
    }

    fn close(left: (f32, f32), right: (f32, f32)) -> bool {
        (left.0 - right.0).abs() < 0.001 && (left.1 - right.1).abs() < 0.001
    }

    fn transform(functions: Vec<Function>) -> Transform {
        Transform { functions }
    }

    fn px(value: f32) -> LengthPercentage {
        LengthPercentage::Length(Length::px(value))
    }

    #[test]
    fn no_functions_move_nothing() {
        let matrix = Transform::default().matrix(metrics(), (100.0, 50.0), (50.0, 25.0));
        assert!(matrix.is_identity());
        assert!(Transform::default().is_empty());
        assert_eq!(matrix.apply(7.0, 9.0), (7.0, 9.0));
    }

    #[test]
    fn a_translation_moves_a_point_and_nothing_else() {
        let matrix = transform(vec![Function::Translate(px(10.0), px(-4.0))]).matrix(
            metrics(),
            (100.0, 50.0),
            (50.0, 25.0),
        );
        assert!(close(matrix.apply(0.0, 0.0), (10.0, -4.0)));
        assert!(close(matrix.apply(100.0, 50.0), (110.0, 46.0)));
        assert!(!matrix.is_identity());
    }

    #[test]
    fn a_percentage_in_a_translation_is_of_the_boxs_own_size() {
        let matrix = transform(vec![Function::Translate(
            LengthPercentage::Percentage(50.0),
            LengthPercentage::Percentage(100.0),
        )])
        .matrix(metrics(), (100.0, 50.0), (0.0, 0.0));
        assert!(close(matrix.apply(0.0, 0.0), (50.0, 50.0)));
    }

    #[test]
    fn a_rotation_turns_about_the_origin_rather_than_the_page() {
        let matrix =
            transform(vec![Function::Rotate(90.0)]).matrix(metrics(), (100.0, 100.0), (50.0, 50.0));
        // The middle stays where it is.
        assert!(close(matrix.apply(50.0, 50.0), (50.0, 50.0)));
        // The top-left corner comes round to the top-right, because `y` runs
        // down the page and clockwise is what that makes positive.
        assert!(close(matrix.apply(0.0, 0.0), (100.0, 0.0)));
    }

    #[test]
    fn a_scale_grows_about_the_origin() {
        let matrix =
            transform(vec![Function::Scale(2.0, 3.0)]).matrix(metrics(), (10.0, 10.0), (5.0, 5.0));
        assert!(
            close(matrix.apply(5.0, 5.0), (5.0, 5.0)),
            "the middle holds"
        );
        assert!(close(matrix.apply(10.0, 10.0), (15.0, 20.0)));
        assert!((matrix.scale_factor() - 6.0_f32.sqrt()).abs() < 0.001);
    }

    #[test]
    fn a_skew_slants_one_axis_along_the_other() {
        let matrix =
            transform(vec![Function::Skew(45.0, 0.0)]).matrix(metrics(), (10.0, 10.0), (0.0, 0.0));
        assert!(close(matrix.apply(0.0, 10.0), (10.0, 10.0)));
        assert!(close(matrix.apply(0.0, 0.0), (0.0, 0.0)));
    }

    #[test]
    fn functions_apply_left_to_right_in_each_others_coordinates() {
        // Turned a quarter, then moved ten "right" — which by then is down.
        let matrix = transform(vec![
            Function::Rotate(90.0),
            Function::Translate(px(10.0), px(0.0)),
        ])
        .matrix(metrics(), (0.0, 0.0), (0.0, 0.0));
        assert!(close(matrix.apply(0.0, 0.0), (0.0, 10.0)));
    }

    #[test]
    fn a_transform_can_be_undone_unless_it_flattens_everything() {
        let matrix = transform(vec![Function::Rotate(30.0), Function::Scale(2.0, 0.5)]).matrix(
            metrics(),
            (100.0, 100.0),
            (50.0, 50.0),
        );
        let back = matrix.inverted().expect("it can be undone");
        let (x, y) = matrix.apply(13.0, 29.0);
        assert!(close(back.apply(x, y), (13.0, 29.0)));

        let flat =
            transform(vec![Function::Scale(1.0, 0.0)]).matrix(metrics(), (10.0, 10.0), (0.0, 0.0));
        assert_eq!(flat.inverted(), None);
    }

    #[test]
    fn a_matrix_reads_back_as_the_six_numbers_it_is() {
        assert_eq!(Matrix::IDENTITY.to_string(), "matrix(1, 0, 0, 1, 0, 0)",);
    }
}
