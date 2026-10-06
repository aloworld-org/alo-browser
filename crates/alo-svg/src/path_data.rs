/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Path data: the `d` attribute, as a path.
//!
//! SVG 2 § 9.3's grammar, every command, in capitals (absolute) and lower
//! case (relative to where the pen is): `M` move, `L` line, `H` and `V`
//! horizontal and vertical lines, `C` and `S` cubic curves, `Q` and `T`
//! quadratic ones, `A` elliptical arcs (turned into cubic curves by
//! [`crate::arc`]), and `Z` close. Ours rather than rented, as ADR 0022 § 2
//! decided, because the two things around the grammar are what matter:
//!
//! - **The error rule.** Path data with an error in it is drawn *up to the
//!   last complete command before the error, and no further* (SVG 2 § 9.5.4).
//!   So `M 0 0 L 10 0 L 10 x` is a line to (10, 0), and a parameter set left
//!   half written at the end is not drawn at all. Data that does not begin
//!   with a move draws nothing.
//! - **The bound.** Segments per path are counted as they are made
//!   ([`MOST_PATH_SEGMENTS`]), and past the bound the whole drawing is refused,
//!   never cut short silently (ADR 0022 § 5).
//!
//! Every point the parser makes is finite, or it is the error that stops the
//! path: `1e38` plus `1e38` overflows an `f32`, and a relative path can add up
//! to that one step at a time.

use crate::arc::{self, Arc};
use crate::bounds::MOST_PATH_SEGMENTS;
use crate::number::{is_whitespace, scan};
use alo_paint::{Path, Point};

/// What path data drew, and where it stopped if it stopped early.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    /// Everything up to the first error.
    pub path: Path,
    /// The byte the first error is at, or [`None`] if there was none.
    pub error: Option<usize>,
}

/// Path data with more segments than [`MOST_PATH_SEGMENTS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooMany;

/// Path data as a path, drawn up to its first error.
///
/// # Errors
///
/// [`TooMany`] when the data asks for more segments than
/// [`MOST_PATH_SEGMENTS`], which refuses the drawing rather than the path.
pub fn parse(text: &str) -> Result<Parsed, TooMany> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        at: 0,
        path: Path::new(),
        pen: Point::default(),
        start: Point::default(),
        closed: false,
        cubic_handle: None,
        quad_handle: None,
    };
    let error = parser.run()?;
    Ok(Parsed {
        path: parser.path,
        error,
    })
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
    path: Path,
    /// Where the pen is.
    pen: Point,
    /// Where the current subpath began, which is where `Z` draws back to.
    start: Point,
    /// Whether the last command was `Z`, so that anything but a move after it
    /// begins a new subpath at the same start (SVG 2 § 9.3.4).
    closed: bool,
    /// The last cubic curve's second handle, for `S` to reflect.
    cubic_handle: Option<Point>,
    /// The last quadratic curve's handle, for `T` to reflect.
    quad_handle: Option<Point>,
}

/// How many numbers each command's parameter set has, or [`None`] for a
/// letter that is not a command.
fn parameters(command: u8) -> Option<usize> {
    Some(match command.to_ascii_uppercase() {
        b'Z' => 0,
        b'H' | b'V' => 1,
        b'M' | b'L' | b'T' => 2,
        b'S' | b'Q' => 4,
        b'C' => 6,
        b'A' => 7,
        _ => return None,
    })
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(is_whitespace) {
            self.at += 1;
        }
    }

    /// Whitespace, at most one comma, whitespace; whether there was a comma.
    fn skip_separator(&mut self) -> bool {
        self.skip_whitespace();
        let comma = self.peek() == Some(b',');
        if comma {
            self.at += 1;
            self.skip_whitespace();
        }
        comma
    }

    /// Every command in turn; the byte of the first error, if there is one.
    fn run(&mut self) -> Result<Option<usize>, TooMany> {
        let mut first = true;
        loop {
            self.skip_whitespace();
            let Some(letter) = self.peek() else {
                return Ok(None);
            };
            let Some(count) = parameters(letter) else {
                return Ok(Some(self.at));
            };
            if first && !letter.eq_ignore_ascii_case(&b'M') {
                return Ok(Some(self.at));
            }
            first = false;
            self.at += 1;
            if count == 0 {
                self.close()?;
                continue;
            }
            if let Some(error) = self.sets(letter, count)? {
                return Ok(Some(error));
            }
        }
    }

    /// One command's parameter sets, as many as are written after it; the
    /// byte of an error, if there is one.
    fn sets(&mut self, letter: u8, count: usize) -> Result<Option<usize>, TooMany> {
        let mut values = [0.0_f32; 7];
        let mut command = letter;
        let mut first = true;
        loop {
            let before = self.at;
            let comma = if first {
                self.skip_whitespace();
                false
            } else {
                self.skip_separator()
            };
            if !self.peek().is_some_and(starts_number) {
                // No more sets. The first one is required, and a comma must be
                // followed by one.
                if first || comma {
                    return Ok(Some(self.at));
                }
                self.at = before;
                return Ok(None);
            }
            let set_start = self.at;
            for (index, value) in values.iter_mut().take(count).enumerate() {
                if index > 0 {
                    self.skip_separator();
                }
                let flag = command.eq_ignore_ascii_case(&b'A') && matches!(index, 3 | 4);
                let read = if flag { self.flag() } else { self.number() };
                let Some(read) = read else {
                    return Ok(Some(self.at));
                };
                *value = read;
            }
            if !self.apply(command, &values)? {
                return Ok(Some(set_start));
            }
            // After a move, further pairs are lines, relative if it was (§ 9.3.3).
            command = match command {
                b'M' => b'L',
                b'm' => b'l',
                other => other,
            };
            first = false;
        }
    }

    fn number(&mut self) -> Option<f32> {
        let (value, end) = scan(self.bytes, self.at)?;
        self.at = end;
        Some(value)
    }

    /// An arc flag: one `0` or `1`, which needs nothing after it before the
    /// next number (`a1 1 0 0110 10` is valid).
    fn flag(&mut self) -> Option<f32> {
        let value = match self.peek()? {
            b'0' => 0.0,
            b'1' => 1.0,
            _ => return None,
        };
        self.at += 1;
        Some(value)
    }

    /// A point relative to the pen when the command is lower case.
    fn point(&self, relative: bool, x: f32, y: f32) -> Option<Point> {
        let point = if relative {
            Point::new(self.pen.x + x, self.pen.y + y)
        } else {
            Point::new(x, y)
        };
        (point.x.is_finite() && point.y.is_finite()).then_some(point)
    }

    /// One parameter set drawn; `false` if a point it makes is not finite.
    fn apply(&mut self, command: u8, values: &[f32; 7]) -> Result<bool, TooMany> {
        let relative = command.is_ascii_lowercase();
        let [v0, v1, v2, v3, v4, v5, v6] = *values;
        let upper = command.to_ascii_uppercase();
        let (mut cubic_handle, mut quad_handle) = (None, None);
        match upper {
            b'M' => {
                let Some(to) = self.point(relative, v0, v1) else {
                    return Ok(false);
                };
                self.closed = false;
                self.path.move_to(to);
                self.start = to;
                self.pen = to;
            }
            b'L' | b'H' | b'V' => {
                let to = match upper {
                    b'H' if relative => self.point(true, v0, 0.0),
                    b'H' => self.point(false, v0, self.pen.y),
                    b'V' if relative => self.point(true, 0.0, v0),
                    b'V' => self.point(false, self.pen.x, v0),
                    _ => self.point(relative, v0, v1),
                };
                let Some(to) = to else {
                    return Ok(false);
                };
                self.begin();
                self.path.line_to(to);
                self.pen = to;
            }
            b'C' | b'S' => {
                let points = if upper == b'C' {
                    (
                        self.point(relative, v0, v1),
                        self.point(relative, v2, v3),
                        self.point(relative, v4, v5),
                    )
                } else {
                    (
                        Some(reflect(self.cubic_handle, self.pen)),
                        self.point(relative, v0, v1),
                        self.point(relative, v2, v3),
                    )
                };
                let (Some(first), Some(second), Some(to)) = points else {
                    return Ok(false);
                };
                if !(first.x.is_finite() && first.y.is_finite()) {
                    return Ok(false);
                }
                self.begin();
                self.path.cubic_to(first, second, to);
                cubic_handle = Some(second);
                self.pen = to;
            }
            b'Q' | b'T' => {
                let points = if upper == b'Q' {
                    (self.point(relative, v0, v1), self.point(relative, v2, v3))
                } else {
                    (
                        Some(reflect(self.quad_handle, self.pen)),
                        self.point(relative, v0, v1),
                    )
                };
                let (Some(handle), Some(to)) = points else {
                    return Ok(false);
                };
                if !(handle.x.is_finite() && handle.y.is_finite()) {
                    return Ok(false);
                }
                self.begin();
                self.path.quad_to(handle, to);
                quad_handle = Some(handle);
                self.pen = to;
            }
            _ => {
                let Some(to) = self.point(relative, v5, v6) else {
                    return Ok(false);
                };
                let arc = Arc {
                    from: self.pen,
                    rx: v0,
                    ry: v1,
                    rotation: v2,
                    large: v3 != 0.0,
                    sweep: v4 != 0.0,
                    to,
                };
                // Made aside, so an arc refused part way leaves nothing behind.
                let mut piece = Path::new();
                if !arc::append(arc, &mut piece) {
                    return Ok(false);
                }
                self.begin();
                self.path.extend(&piece);
                self.pen = to;
            }
        }
        self.cubic_handle = cubic_handle;
        self.quad_handle = quad_handle;
        self.counted()?;
        Ok(true)
    }

    /// Before anything but a move is drawn: after `Z`, a new subpath at the
    /// same start (SVG 2 § 9.3.4), said outright so the path needs no reader
    /// to know the rule.
    fn begin(&mut self) {
        if self.closed {
            self.path.move_to(self.start);
            self.closed = false;
        }
    }

    /// `Z`: back to where the subpath began.
    fn close(&mut self) -> Result<(), TooMany> {
        self.path.close();
        self.pen = self.start;
        self.closed = true;
        self.cubic_handle = None;
        self.quad_handle = None;
        self.counted()
    }

    fn counted(&self) -> Result<(), TooMany> {
        if self.path.segments().len() > MOST_PATH_SEGMENTS {
            Err(TooMany)
        } else {
            Ok(())
        }
    }
}

/// The handle a smooth curve begins with: the last curve's handle reflected
/// through the pen, or the pen itself when the last command was not a curve
/// of the same kind (SVG 2 § 9.3.6).
fn reflect(handle: Option<Point>, pen: Point) -> Point {
    handle.map_or(pen, |handle| {
        Point::new(2.0 * pen.x - handle.x, 2.0 * pen.y - handle.y)
    })
}

/// Whether a number can begin with this byte.
fn starts_number(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'.' | b'+' | b'-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_paint::Segment;

    fn p(x: f32, y: f32) -> Point {
        Point::new(x, y)
    }

    fn segments(text: &str) -> Vec<Segment> {
        let parsed = parse(text).expect("within the bound");
        assert_eq!(parsed.error, None, "{text:?} has an error");
        parsed.path.segments().to_vec()
    }

    fn up_to_error(text: &str) -> (Vec<Segment>, usize) {
        let parsed = parse(text).expect("within the bound");
        let error = parsed
            .error
            .unwrap_or_else(|| panic!("{text:?} had no error"));
        (parsed.path.segments().to_vec(), error)
    }

    #[test]
    fn move_line_and_close_absolute_and_relative() {
        use Segment::{Close, LineTo, MoveTo};
        assert_eq!(
            segments("M 10 20 L 30 40 Z"),
            vec![MoveTo(p(10.0, 20.0)), LineTo(p(30.0, 40.0)), Close]
        );
        assert_eq!(
            segments("m 10 20 l 30 40 z"),
            vec![MoveTo(p(10.0, 20.0)), LineTo(p(40.0, 60.0)), Close]
        );
    }

    #[test]
    fn pairs_after_a_move_are_lines_relative_if_the_move_was() {
        use Segment::{LineTo, MoveTo};
        assert_eq!(
            segments("M1 1 2 2 3 3"),
            vec![
                MoveTo(p(1.0, 1.0)),
                LineTo(p(2.0, 2.0)),
                LineTo(p(3.0, 3.0))
            ]
        );
        assert_eq!(
            segments("m1 1 2 2 3 3"),
            vec![
                MoveTo(p(1.0, 1.0)),
                LineTo(p(3.0, 3.0)),
                LineTo(p(6.0, 6.0))
            ]
        );
        // A second relative move is from where the pen is.
        assert_eq!(
            segments("m1 1 m2 2"),
            vec![MoveTo(p(1.0, 1.0)), MoveTo(p(3.0, 3.0))]
        );
    }

    #[test]
    fn horizontal_and_vertical_lines_keep_the_other_coordinate() {
        use Segment::{LineTo, MoveTo};
        assert_eq!(
            segments("M 5 6 H 20 V 30 h -5 v -10"),
            vec![
                MoveTo(p(5.0, 6.0)),
                LineTo(p(20.0, 6.0)),
                LineTo(p(20.0, 30.0)),
                LineTo(p(15.0, 30.0)),
                LineTo(p(15.0, 20.0)),
            ]
        );
    }

    #[test]
    fn cubic_curves_and_their_smooth_continuations() {
        use Segment::{CubicTo, MoveTo};
        assert_eq!(
            segments("M0 0 C 1 2 3 4 5 6 S 9 10 11 12"),
            vec![
                MoveTo(p(0.0, 0.0)),
                CubicTo(p(1.0, 2.0), p(3.0, 4.0), p(5.0, 6.0)),
                // The first handle is (3, 4) reflected through (5, 6).
                CubicTo(p(7.0, 8.0), p(9.0, 10.0), p(11.0, 12.0)),
            ]
        );
        assert_eq!(
            segments("M10 10 c 1 2 3 4 5 6 s 4 4 6 6"),
            vec![
                MoveTo(p(10.0, 10.0)),
                CubicTo(p(11.0, 12.0), p(13.0, 14.0), p(15.0, 16.0)),
                CubicTo(p(17.0, 18.0), p(19.0, 20.0), p(21.0, 22.0)),
            ]
        );
        // After something that is not a cubic curve, the handle is the pen.
        assert_eq!(
            segments("M0 0 L 4 4 S 6 8 10 10"),
            vec![
                MoveTo(p(0.0, 0.0)),
                Segment::LineTo(p(4.0, 4.0)),
                CubicTo(p(4.0, 4.0), p(6.0, 8.0), p(10.0, 10.0)),
            ]
        );
    }

    #[test]
    fn quadratic_curves_and_their_smooth_continuations() {
        use Segment::{MoveTo, QuadTo};
        assert_eq!(
            segments("M0 0 Q 5 10 10 0 T 20 0 t 10 0"),
            vec![
                MoveTo(p(0.0, 0.0)),
                QuadTo(p(5.0, 10.0), p(10.0, 0.0)),
                QuadTo(p(15.0, -10.0), p(20.0, 0.0)),
                QuadTo(p(25.0, 10.0), p(30.0, 0.0)),
            ]
        );
        assert_eq!(
            segments("m0 0 q 5 10 10 0"),
            vec![MoveTo(p(0.0, 0.0)), QuadTo(p(5.0, 10.0), p(10.0, 0.0))]
        );
        // A `T` after a cubic curve does not reflect the cubic's handle.
        assert_eq!(
            segments("M0 0 C 1 1 2 2 3 3 T 6 6").get(2),
            Some(&QuadTo(p(3.0, 3.0), p(6.0, 6.0)))
        );
    }

    #[test]
    fn arcs_absolute_and_relative_with_packed_flags() {
        let absolute = segments("M 0 0 A 10 10 0 0 1 10 10");
        let relative = segments("M 0 0 a10,10 0 0,1 10,10");
        let packed = segments("M0 0a10 10 0 0110 10");
        assert_eq!(absolute.len(), 2);
        assert_eq!(absolute, relative);
        assert_eq!(absolute, packed);
        assert!(matches!(
            absolute.get(1),
            Some(Segment::CubicTo(_, _, end)) if *end == p(10.0, 10.0)
        ));
    }

    #[test]
    fn an_arc_with_a_zero_radius_is_a_line_and_one_with_no_length_is_nothing() {
        use Segment::{LineTo, MoveTo};
        assert_eq!(
            segments("M 0 0 A 0 5 0 0 1 10 0"),
            vec![MoveTo(p(0.0, 0.0)), LineTo(p(10.0, 0.0))]
        );
        assert_eq!(segments("M 3 3 A 5 5 0 1 1 3 3"), vec![MoveTo(p(3.0, 3.0))]);
    }

    #[test]
    fn a_command_after_close_starts_again_where_the_subpath_began() {
        use Segment::{Close, LineTo, MoveTo};
        assert_eq!(
            segments("M 10 10 L 20 10 Z l 0 10"),
            vec![
                MoveTo(p(10.0, 10.0)),
                LineTo(p(20.0, 10.0)),
                Close,
                MoveTo(p(10.0, 10.0)),
                LineTo(p(10.0, 20.0)),
            ]
        );
    }

    #[test]
    fn numbers_need_no_separators_where_signs_and_points_divide_them() {
        use Segment::{LineTo, MoveTo};
        assert_eq!(
            segments("M.5.5L-1-1,2,2"),
            vec![
                MoveTo(p(0.5, 0.5)),
                LineTo(p(-1.0, -1.0)),
                LineTo(p(2.0, 2.0))
            ]
        );
        assert_eq!(segments("M1e1 2E-1"), vec![MoveTo(p(10.0, 0.2))]);
        assert_eq!(segments(""), vec![]);
        assert_eq!(segments("  \n "), vec![]);
    }

    #[test]
    fn data_is_drawn_up_to_the_last_command_before_its_first_error() {
        use Segment::{LineTo, MoveTo};
        let (drawn, at) = up_to_error("M 0 0 L 10 0 L 10 x");
        assert_eq!(drawn, vec![MoveTo(p(0.0, 0.0)), LineTo(p(10.0, 0.0))]);
        assert_eq!(at, 18, "the byte of the x");

        // A set left half written is not drawn.
        let (drawn, _) = up_to_error("M 0 0 L 10 0 20");
        assert_eq!(drawn.len(), 2);
        let (drawn, _) = up_to_error("M 0 0 C 1 1 2 2");
        assert_eq!(drawn.len(), 1);

        // An unknown letter, a command with nothing after it, and a comma
        // before a command are errors.
        assert_eq!(up_to_error("M 0 0 X 1 1").0.len(), 1);
        assert_eq!(up_to_error("M 0 0 L").0.len(), 1);
        assert_eq!(up_to_error("M 0 0, L 1 1").0.len(), 1);
        assert_eq!(up_to_error("M 0 0 L 1 1,").0.len(), 2);
        assert_eq!(up_to_error("M, 0 0").0.len(), 0);

        // An arc flag is a single 0 or 1.
        assert_eq!(up_to_error("M 0 0 A 5 5 0 2 1 10 0").0.len(), 1);
        assert_eq!(up_to_error("M 0 0 A 5 5 0 .5 1 10 0").0.len(), 1);
    }

    #[test]
    fn data_that_does_not_begin_with_a_move_draws_nothing() {
        for text in ["L 10 10", "Z", "10 10", "z M 0 0"] {
            let (drawn, at) = up_to_error(text);
            assert!(drawn.is_empty(), "{text}");
            assert_eq!(at, 0, "{text}");
        }
    }

    #[test]
    fn numbers_that_are_not_finite_are_errors_and_so_are_sums_that_overflow() {
        assert_eq!(up_to_error("M 0 0 L 1e99999 0").0.len(), 1);
        assert_eq!(up_to_error("M 0 0 L inf 0").0.len(), 1);
        assert_eq!(up_to_error("M 0 0 L NaN 0").0.len(), 1);
        // Each step fits; their sum does not.
        let (drawn, _) = up_to_error("m 3e38 0 l 3e38 0");
        assert_eq!(drawn, vec![Segment::MoveTo(p(3e38, 0.0))]);
        // A smooth handle reflected past a float.
        let (drawn, _) = up_to_error("M 3e38 0 Q -3e38 0 3e38 0 T 0 0");
        assert_eq!(drawn.len(), 2);
        // An arc that would reach past a float.
        assert_eq!(up_to_error("M 0 0 A 3e38 3e38 0 1 1 1 0").0.len(), 1);
    }

    #[test]
    fn the_segment_bound_refuses_and_one_at_it_does_not() {
        // The move is one segment; each further pair is one line.
        let at = format!("M0 0{}", " 1 1".repeat(MOST_PATH_SEGMENTS - 1));
        assert_eq!(
            parse(&at).expect("at the bound").path.segments().len(),
            MOST_PATH_SEGMENTS
        );
        let past = format!("M0 0{}", " 1 1".repeat(MOST_PATH_SEGMENTS));
        assert_eq!(parse(&past), Err(TooMany));

        // Arcs count every curve they become, and closes count too.
        let arcs = format!(
            "M0 0{}",
            " A1 1 0 1 1 0 2 A1 1 0 1 1 0 0".repeat(MOST_PATH_SEGMENTS)
        );
        assert_eq!(parse(&arcs), Err(TooMany));
        let closes = format!("M0 0{}", "Z".repeat(MOST_PATH_SEGMENTS));
        assert_eq!(parse(&closes), Err(TooMany));
    }

    #[test]
    fn adversarial_data_never_panics() {
        let long_number = format!("M 0 0 L 1{} 0", "0".repeat(100_000));
        let deep = "M".repeat(10_000);
        for text in [
            long_number.as_str(),
            deep.as_str(),
            "M",
            "M 0",
            "M 0 0 A",
            "M 0 0 A 1 1 0 1",
            "M 0 0 a 1 1 0 1 1",
            "M-.e1",
            "M 0 0 ,,,,",
            "M 0 0 L 1 1 1 1 1",
            "\u{0}\u{1}M\u{FFFD}",
            "M 0 0 L ١ ٢",
            "M 1e-45 1e-45 A 1e-45 1e-45 1e30 1 1 0 0",
            "M 3.4e38 3.4e38 A 3.4e38 1e-45 -1e30 0 0 -3.4e38 -3.4e38",
        ] {
            let _ = parse(text);
        }
    }
}
