/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The `transform` attribute.
//!
//! Its grammar is SVG's, not CSS's: numbers without units, angles in degrees
//! without `deg`, `rotate` that takes a centre, and functions separated by
//! commas as well as by whitespace. `transform="rotate(45 12 12)"` is not a
//! CSS value at all, which is why [`crate::parse_transform`] is not asked.
//!
//! A list with an error in it is **ignored whole**, as an invalid declaration
//! is: drawing a shape under half of the transform its author wrote would put
//! it somewhere nobody asked for.
//!
//! # As a declaration
//!
//! SVG 2 makes the attribute the **presentation attribute** of CSS's
//! `transform` property (item 287), so a stylesheet's `transform` replaces it
//! rather than composing with it. For the cascade to hold it, it has to be a
//! value CSS can read, and [`as_css`] writes it as one: the single `matrix()`
//! it comes to. Nothing is lost by flattening, because the attribute has no
//! percentages and no units — every function in it is already a matrix — and
//! `transform-origin` is applied about the whole list, which is the same as
//! applying it about each function in turn.

use crate::svg_number::Numbers;
use crate::transform::Matrix;

/// The most arguments any function takes: `matrix`'s six.
const MOST_ARGUMENTS: usize = 6;

/// A `transform` attribute as one matrix, or [`None`] if it has an error.
///
/// Functions apply right to left — `translate(10) scale(2)` scales first —
/// which is what writing them left to right means.
pub fn parse(text: &str) -> Option<Matrix> {
    let mut matrix = Matrix::IDENTITY;
    let mut rest = text.trim_start_matches(is_separator);
    while !rest.is_empty() {
        let open = rest.find('(')?;
        let name = rest.get(..open)?.trim_end_matches(is_whitespace);
        let after = rest.get(open + 1..)?;
        let close = after.find(')')?;
        let arguments = arguments(after.get(..close)?)?;
        matrix = function(name, &arguments)?.then(matrix);
        rest = after.get(close + 1..)?.trim_start_matches(is_separator);
    }
    // Each function is finite on its own, and two of them can still multiply
    // to an infinity — `scale(1e38) scale(1e38)` — which is not a transform
    // anything can be drawn under, so the list is refused as an error.
    finite(matrix).then_some(matrix)
}

fn finite(matrix: Matrix) -> bool {
    [matrix.a, matrix.b, matrix.c, matrix.d, matrix.e, matrix.f]
        .iter()
        .all(|value| value.is_finite())
}

/// A `transform` attribute as the CSS `transform` value it stands for, or
/// [`None`] if it has an error.
///
/// `matrix(a, b, c, d, e, f)`, with each number written as the shortest text
/// that reads back as the same `f32`, so the CSS parser recovers the matrix
/// exactly.
pub fn as_css(text: &str) -> Option<String> {
    parse(text).map(|matrix| matrix.to_string())
}

/// A function's arguments, or [`None`] if there are more than any function
/// takes or one is not a number. Bounded before anything is kept, so an
/// argument list of a million numbers is refused after reading seven.
fn arguments(text: &str) -> Option<Vec<f32>> {
    let mut numbers = Numbers::new(text);
    let mut found = Vec::with_capacity(MOST_ARGUMENTS);
    for value in numbers.by_ref() {
        if found.len() == MOST_ARGUMENTS {
            return None;
        }
        found.push(value);
    }
    (!numbers.failed()).then_some(found)
}

fn function(name: &str, arguments: &[f32]) -> Option<Matrix> {
    Some(match (name, arguments) {
        ("matrix", &[across, down_x, across_y, down, move_x, move_y]) => Matrix {
            a: across,
            b: down_x,
            c: across_y,
            d: down,
            e: move_x,
            f: move_y,
        },
        ("translate", &[x]) => Matrix::translation(x, 0.0),
        ("translate", &[x, y]) => Matrix::translation(x, y),
        ("scale", &[both]) => scale(both, both),
        ("scale", &[x, y]) => scale(x, y),
        ("rotate", &[angle]) => rotation(angle),
        ("rotate", &[angle, x, y]) => Matrix::translation(-x, -y)
            .then(rotation(angle))
            .then(Matrix::translation(x, y)),
        ("skewX", &[angle]) => Matrix {
            c: angle.to_radians().tan(),
            ..Matrix::IDENTITY
        },
        ("skewY", &[angle]) => Matrix {
            b: angle.to_radians().tan(),
            ..Matrix::IDENTITY
        },
        _ => return None,
    })
    .filter(|matrix| finite(*matrix))
}

fn scale(x: f32, y: f32) -> Matrix {
    Matrix {
        a: x,
        d: y,
        ..Matrix::IDENTITY
    }
}

/// A turn, clockwise on the page because `y` runs down it.
fn rotation(degrees: f32) -> Matrix {
    let (sin, cos) = degrees.to_radians().sin_cos();
    Matrix {
        a: cos,
        b: sin,
        c: -sin,
        d: cos,
        e: 0.0,
        f: 0.0,
    }
}

fn is_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c')
}

fn is_separator(c: char) -> bool {
    c == ',' || is_whitespace(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str, x: f32, y: f32) -> (f32, f32) {
        let (x, y) = parse(text).expect("a transform").apply(x, y);
        // Rounded, because a rotation by a whole number of degrees is not
        // exact in floating point.
        ((x * 1000.0).round() / 1000.0, (y * 1000.0).round() / 1000.0)
    }

    #[test]
    fn each_function_moves_a_point_as_svg_says() {
        assert_eq!(at("translate(10)", 1.0, 1.0), (11.0, 1.0));
        assert_eq!(at("translate(10, -5)", 1.0, 1.0), (11.0, -4.0));
        assert_eq!(at("scale(2)", 3.0, 4.0), (6.0, 8.0));
        assert_eq!(at("scale(2 3)", 3.0, 4.0), (6.0, 12.0));
        assert_eq!(
            at("rotate(90)", 1.0, 0.0),
            (0.0, 1.0),
            "clockwise on a page"
        );
        assert_eq!(at("rotate(90 10 10)", 11.0, 10.0), (10.0, 11.0));
        assert_eq!(at("skewX(45)", 0.0, 2.0), (2.0, 2.0));
        assert_eq!(at("skewY(45)", 2.0, 0.0), (2.0, 2.0));
        assert_eq!(at("matrix(1 0 0 1 5 6)", 0.0, 0.0), (5.0, 6.0));
    }

    #[test]
    fn a_list_applies_right_to_left() {
        // Scale first, then move: (1, 1) → (2, 2) → (12, 2).
        assert_eq!(at("translate(10) scale(2)", 1.0, 1.0), (12.0, 2.0));
        // Move first, then scale: (1, 1) → (11, 1) → (22, 2).
        assert_eq!(at("scale(2),translate(10)", 1.0, 1.0), (22.0, 2.0));
        assert_eq!(at("  ", 3.0, 4.0), (3.0, 4.0), "nothing is the identity");
    }

    #[test]
    fn a_list_with_an_error_in_it_is_ignored_whole() {
        for text in [
            "translate(10",
            "translate(10) bogus(1)",
            "rotate()",
            "scale(1 2 3)",
            "translate(1e99999)",
            "scale(1e38) scale(1e38)",
            "matrix(1 0 0 1 0 0 0)",
            "translate(10px)",
            "rotate(45deg)",
            "translate 10",
        ] {
            assert_eq!(parse(text), None, "{text}");
        }
    }

    /// What the cascade holds is what the attribute meant: the CSS parser
    /// reads the declaration back as the very same matrix.
    #[test]
    fn as_css_reads_back_as_the_same_matrix() {
        for text in [
            "rotate(45 12 12)",
            "translate(10,-5) scale(.3)",
            "skewX(30) skewY(-10)",
            "matrix(1 2 3 4 5 6)",
            "scale(1e-7) translate(3e30)",
            " ",
        ] {
            let css = as_css(text).expect("a transform");
            let read = crate::parse_transform(&css)
                .expect("a CSS transform")
                .matrix(
                    crate::FontMetrics::estimated(16.0, 16.0),
                    (0.0, 0.0),
                    (0.0, 0.0),
                );
            assert_eq!(read, parse(text).expect("a transform"), "{text} as {css}");
        }
        assert_eq!(as_css("rotate(45deg)"), None);
    }

    #[test]
    fn a_function_with_a_million_arguments_is_refused_after_seven() {
        let text = format!("matrix({})", "1 ".repeat(1_000_000));
        assert_eq!(parse(&text), None);
    }
}
