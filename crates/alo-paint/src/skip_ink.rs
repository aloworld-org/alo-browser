/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a line under or over text steps aside for the letters.
//!
//! `text-decoration-skip-ink: auto`, its initial value, asks for an
//! underline or an overline to leave a gap where a letter's ink crosses it,
//! so a `g` or a `y` hangs through a broken line rather than one drawn over
//! its tail. A line through the text never does: it is meant to cross it.
//!
//! # The rule, measured
//!
//! Measured from Chrome 154's screenshots, not read from its source (queue
//! item 579): `g j p q y ( ) , ç Q _ ŋ ţ` in `DejaVu Sans` underlined, and
//! `É Å Ê Î Ö Ç Ŕ` overlined, at twenty-two sizes from 8 to 64 px, under
//! `--force-device-scale-factor=1` and `=2`, the line red and the letters
//! blue so that each could be read without the other. Every edge of every
//! gap is a whole device pixel, and this rule puts 1,014 of the 1,022
//! underline edges exactly where Chrome does, and the other eight one pixel
//! off where the edge falls within a fiftieth of a half; it puts every
//! overline's gaps exactly. A line through had no gap anywhere.
//!
//! In **device** pixels, for a line whose top is device row `top` and whose
//! thickness is `t` before it was cut to whole pixels
//! ([`crate::decoration::thickness`]):
//!
//! - **The band** a letter is looked for in runs from half a pixel below the
//!   line's top to `t − ½` below it: the middle of the line, less half a
//!   pixel each side. At a thickness of one it is a single row's middle.
//! - **A glyph's ink** in the band is the leftmost and rightmost of: where
//!   its outline crosses the band's top or bottom, and **every point of its
//!   outline inside the band, a curve's control points included**. The
//!   control points are not a guess: the right of `g`'s descender in `DejaVu
//!   Sans` is a curve whose control point stands 37 font units clear of its
//!   ink, and Chrome's gap there is that much wider — 2.3 device pixels at
//!   64 px at two. One glyph is one stretch of ink, however many places it
//!   crosses the band.
//! - **The gap** is that stretch widened by `t` each side, each end rounded
//!   to the nearest whole pixel. Gaps that meet are one gap.
//!
//! # The property's values, measured
//!
//! Measured the same way (queue item 581): `gjpqy` underlined at 32 px,
//! and `ÉÅÊÎÖÇŔ` overlined at nine sizes from 10 to 64 px, under each value
//! at one and at two.
//!
//! - **`all` is drawn as `auto`**, every gap of every line on the same
//!   pixels. CSS asks `all` to step aside for ideographs too, where `auto`
//!   may not; this engine's `auto` already steps aside for every glyph, and
//!   no ideograph was measured, so the two are one rule here ([`SkipInk`]).
//! - **`none` is the line whole**, under and over alike.
//! - **A line through is whole under all three.**
//! - **The value is the text's own**, not that of the element that asked for
//!   the line: under a `none` underline, a child's `auto` text steps aside
//!   and the parent's own text does not, and the other way round. It
//!   inherits, so the text's value is the nearest one written above it.
//! - **A value that is not one of the three is no value**: `bogus`, and
//!   `none auto`, leave the text with what it would have had without them.
//!   Keywords are read in any case.

use alo_layout::Rect;

use crate::path::{Path, Point, Segment};

/// What `text-decoration-skip-ink` asks of the text it is set on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipInk {
    /// The initial value: a line steps aside for the letters.
    Auto,
    /// Drawn as `auto` (measured, above).
    All,
    /// A line runs whole, through the letters.
    None,
}

impl SkipInk {
    /// The value written, or [`None`] for one that is none of the three —
    /// which is not a value, and leaves the text the value it inherits.
    pub fn parse(written: &str) -> Option<Self> {
        let written = written.trim();
        [
            (Self::Auto, "auto"),
            (Self::All, "all"),
            (Self::None, "none"),
        ]
        .into_iter()
        .find(|(_, name)| written.eq_ignore_ascii_case(name))
        .map(|(value, _)| value)
    }

    /// Whether an underline or an overline steps aside for the letters. A
    /// line through never does, whatever this is.
    pub fn steps_aside(self) -> bool {
        self != Self::None
    }
}

/// The pieces of `line` left once it steps aside for `glyphs`, each a shape
/// where it is drawn, in CSS pixels, on a screen of `scale` device pixels to
/// the CSS pixel. `thickness` is the line's, in device pixels, before it was
/// cut to whole pixels.
///
/// A line no glyph crosses is itself, whole; a line every glyph covers is no
/// pieces at all. A scale that is not a scale is one.
pub fn pieces(line: Rect, thickness: f32, glyphs: &[Path], scale: f32) -> Vec<Rect> {
    let scale = alo_value::device_scale(scale);
    let top = line.top() * scale;
    let band = (top + 0.5, top + thickness - 0.5);
    let mut gaps: Vec<(f32, f32)> = glyphs
        .iter()
        .filter_map(|glyph| ink(glyph, band.0 / scale, band.1 / scale))
        .map(|(left, right)| {
            (
                round(left * scale - thickness) / scale,
                round(right * scale + thickness) / scale,
            )
        })
        .collect();
    gaps.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut pieces = Vec::new();
    let mut from = line.left();
    for (left, right) in gaps {
        if left > from {
            pieces.push((from, left.min(line.right())));
        }
        from = from.max(right);
    }
    pieces.push((from, line.right()));
    pieces
        .into_iter()
        .filter(|(left, right)| right > left)
        .map(|(left, right)| Rect::new(left, line.top(), right - left, line.size.height))
        .collect()
}

/// The nearest whole pixel, a half going up.
fn round(value: f32) -> f32 {
    (value + 0.5).floor()
}

/// The leftmost and rightmost of a glyph's ink between `top` and `bottom`,
/// in its own coordinates: where its outline crosses either, and every point
/// of the outline between them, control points included. [`None`] for a
/// glyph that does not reach the band.
fn ink(glyph: &Path, top: f32, bottom: f32) -> Option<(f32, f32)> {
    let mut found = Found::default();
    let inside = |point: Point| (top..=bottom).contains(&point.y);
    let mut start = Point::default();
    let mut pen = Point::default();
    for segment in glyph.segments() {
        match *segment {
            Segment::MoveTo(to) => {
                start = to;
                pen = to;
                found.point_if(inside(to), to);
            }
            Segment::LineTo(to) => {
                found.line(pen, to, top, bottom);
                found.point_if(inside(to), to);
                pen = to;
            }
            Segment::QuadTo(control, to) => {
                found.point_if(inside(control), control);
                found.point_if(inside(to), to);
                for edge in [top, bottom] {
                    for t in quad_crossings(pen.y, control.y, to.y, edge) {
                        found.x(quad_at(pen.x, control.x, to.x, t));
                    }
                }
                pen = to;
            }
            Segment::CubicTo(first, second, to) => {
                found.point_if(inside(first), first);
                found.point_if(inside(second), second);
                found.point_if(inside(to), to);
                let ys = [pen.y, first.y, second.y, to.y];
                let xs = [pen.x, first.x, second.x, to.x];
                for edge in [top, bottom] {
                    for t in cubic_crossings(ys, edge) {
                        found.x(cubic_at(xs, t));
                    }
                }
                pen = to;
            }
            Segment::Close => {
                found.line(pen, start, top, bottom);
                pen = start;
            }
        }
    }
    found.extent
}

/// The ink found so far: its leftmost and rightmost.
#[derive(Default)]
struct Found {
    extent: Option<(f32, f32)>,
}

impl Found {
    /// One more place the ink reaches. A coordinate that is not a number
    /// counts for nothing.
    fn x(&mut self, x: f32) {
        if !x.is_finite() {
            return;
        }
        self.extent = Some(match self.extent {
            Some((left, right)) => (left.min(x), right.max(x)),
            None => (x, x),
        });
    }

    fn point_if(&mut self, inside: bool, point: Point) {
        if inside {
            self.x(point.x);
        }
    }

    /// Where a straight edge crosses the band's top or bottom.
    fn line(&mut self, from: Point, to: Point, top: f32, bottom: f32) {
        for edge in [top, bottom] {
            if (from.y - edge) * (to.y - edge) < 0.0 {
                let t = (edge - from.y) / (to.y - from.y);
                self.x(from.x + t * (to.x - from.x));
            }
        }
    }
}

/// Where along a quadratic, from 0 to 1, its `y` is `edge`.
fn quad_crossings(from: f32, control: f32, to: f32, edge: f32) -> Vec<f32> {
    let a = from - 2.0 * control + to;
    let b = 2.0 * (control - from);
    let c = from - edge;
    let roots = if a.abs() < f32::EPSILON {
        if b.abs() < f32::EPSILON {
            Vec::new()
        } else {
            vec![-c / b]
        }
    } else {
        let discriminant = b * b - 4.0 * a * c;
        if discriminant < 0.0 {
            Vec::new()
        } else {
            let root = discriminant.sqrt();
            vec![(-b + root) / (2.0 * a), (-b - root) / (2.0 * a)]
        }
    };
    roots
        .into_iter()
        .filter(|t| (0.0..=1.0).contains(t))
        .collect()
}

fn quad_at(from: f32, control: f32, to: f32, t: f32) -> f32 {
    let rest = 1.0 - t;
    rest * rest * from + 2.0 * rest * t * control + t * t * to
}

fn cubic_at([from, first, second, to]: [f32; 4], t: f32) -> f32 {
    let rest = 1.0 - t;
    rest * rest * rest * from
        + 3.0 * rest * rest * t * first
        + 3.0 * rest * t * t * second
        + t * t * t * to
}

/// How finely a cubic is searched for where it crosses an edge: a crossing
/// is bracketed between two of this many steps and then halved down to.
const CUBIC_STEPS: u8 = 64;

/// Where along a cubic, from 0 to 1, its `y` is `edge`: each change of side
/// between two of [`CUBIC_STEPS`] samples, halved twenty-four times. A curve
/// that only touches the edge between two samples adds no crossing, and
/// needs none: its control points already say how far it reaches.
fn cubic_crossings(ys: [f32; 4], edge: f32) -> Vec<f32> {
    let side = |t: f32| cubic_at(ys, t) - edge;
    let mut crossings = Vec::new();
    for step in 0..CUBIC_STEPS {
        let mut low = f32::from(step) / f32::from(CUBIC_STEPS);
        let mut high = f32::from(step + 1) / f32::from(CUBIC_STEPS);
        if side(low) * side(high) > 0.0 {
            continue;
        }
        for _ in 0..24 {
            let middle = f32::midpoint(low, high);
            if side(low) * side(middle) <= 0.0 {
                high = middle;
            } else {
                low = middle;
            }
        }
        crossings.push(f32::midpoint(low, high));
    }
    crossings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glyph::outline_at;
    use alo_text::{Direction, Font, Slant, Weight, shape};

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    /// The pieces' ends, in device pixels.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "whole device pixels on a small page"
    )]
    fn ends(pieces: &[Rect], scale: f32) -> Vec<(i32, i32)> {
        pieces
            .iter()
            .map(|piece| {
                let (left, right) = (piece.left() * scale, piece.right() * scale);
                (left.round() as i32, right.round() as i32)
            })
            .collect()
    }

    fn square(left: f32, top: f32, side: f32) -> Path {
        Path::rectangle(left, top, side, side)
    }

    /// A stroke from 10 to 30 down, `width` wide: across any line at 20.
    fn bar(left: f32, width: f32) -> Path {
        Path::rectangle(left, 10.0, width, 20.0)
    }

    #[test]
    fn the_three_values_are_read_in_any_case_and_nothing_else_is() {
        assert_eq!(SkipInk::parse("auto"), Some(SkipInk::Auto));
        assert_eq!(SkipInk::parse(" ALL "), Some(SkipInk::All));
        assert_eq!(SkipInk::parse("None"), Some(SkipInk::None));
        for refused in ["", "bogus", "none auto", "auto,", "inherit", "0"] {
            assert_eq!(SkipInk::parse(refused), None, "{refused:?}");
        }
        assert!(SkipInk::Auto.steps_aside() && SkipInk::All.steps_aside());
        assert!(!SkipInk::None.steps_aside());
    }

    #[test]
    fn a_glyph_across_the_line_leaves_a_gap_as_wide_as_its_ink_and_a_thickness_each_side() {
        // A line 2.0 device pixels thick from row 20; a shape from x 30 to
        // 40 across it: the gap is 28 to 42.
        let line = Rect::new(0.0, 20.0, 100.0, 2.0);
        let pieces = pieces(line, 2.0, &[bar(30.0, 10.0)], 1.0);
        assert_eq!(ends(&pieces, 1.0), vec![(0, 28), (42, 100)]);
        for piece in &pieces {
            assert!(close(piece.top(), 20.0) && close(piece.size.height, 2.0));
        }
    }

    #[test]
    fn a_gap_ends_on_the_nearest_whole_pixel() {
        // Ink from 30.4 to 39.6 at 1.4 thick: 29.0 and 41.0 exactly, and a
        // half goes up — 30.9 − 1.4 is 29.5, which is 30.
        let line = Rect::new(0.0, 20.0, 100.0, 1.0);
        assert_eq!(
            ends(&pieces(line, 1.4, &[bar(30.4, 9.2)], 1.0), 1.0),
            vec![(0, 29), (41, 100)]
        );
        assert_eq!(
            ends(&pieces(line, 1.4, &[bar(30.9, 8.0)], 1.0), 1.0)[0],
            (0, 30)
        );
    }

    #[test]
    fn ink_beside_the_band_does_not_break_the_line() {
        // Above the line, and below it.
        let line = Rect::new(0.0, 20.0, 100.0, 2.0);
        let glyphs = [square(30.0, 5.0, 10.0), square(60.0, 22.0, 10.0)];
        assert_eq!(ends(&pieces(line, 2.0, &glyphs, 1.0), 1.0), vec![(0, 100)]);
    }

    #[test]
    fn the_band_is_the_lines_middle_less_half_a_pixel_each_side() {
        // A line 3.0 thick from row 20 looks from 20.5 to 22.5. A shape
        // whose bottom is at 20.4 is above it; one whose top is at 22.6 is
        // below it; one from 22.4 is in it.
        let line = Rect::new(0.0, 20.0, 100.0, 3.0);
        let above = Path::rectangle(30.0, 10.0, 5.0, 10.4);
        let below = Path::rectangle(50.0, 22.6, 5.0, 5.0);
        assert_eq!(
            ends(&pieces(line, 3.0, &[above, below], 1.0), 1.0),
            vec![(0, 100)]
        );
        let reaching = Path::rectangle(50.0, 22.4, 5.0, 5.0);
        assert_eq!(
            ends(&pieces(line, 3.0, &[reaching], 1.0), 1.0),
            vec![(0, 47), (58, 100)]
        );
    }

    #[test]
    fn a_line_one_pixel_thick_looks_along_its_middle() {
        let line = Rect::new(0.0, 20.0, 100.0, 1.0);
        let crossing = Path::rectangle(30.0, 20.4, 4.0, 4.0);
        let missing = Path::rectangle(60.0, 20.6, 4.0, 4.0);
        assert_eq!(
            ends(&pieces(line, 1.0, &[crossing, missing], 1.0), 1.0),
            vec![(0, 29), (35, 100)]
        );
    }

    #[test]
    fn a_curves_control_point_inside_the_band_counts_as_ink() {
        // A curve from (10, 0) bending towards (40, 21) and back to (10,
        // 40): its ink at row 21 reaches x 25, its control point x 40.
        let mut curve = Path::new();
        curve.move_to(Point::new(10.0, 0.0));
        curve.quad_to(Point::new(40.0, 21.0), Point::new(10.0, 40.0));
        curve.close();
        let line = Rect::new(0.0, 20.0, 100.0, 2.0);
        assert_eq!(
            ends(&pieces(line, 2.0, &[curve], 1.0), 1.0),
            vec![(0, 8), (42, 100)]
        );
    }

    #[test]
    fn a_cubic_is_found_where_it_crosses() {
        // A cubic whose control points are both far above the band and whose
        // curve dips into it: only the crossings find the ink.
        let mut curve = Path::new();
        curve.move_to(Point::new(10.0, 0.0));
        curve.cubic_to(
            Point::new(20.0, 40.0),
            Point::new(40.0, 40.0),
            Point::new(50.0, 0.0),
        );
        curve.close();
        let line = Rect::new(0.0, 20.0, 100.0, 2.0);
        let found = ink(&curve, 20.5, 20.5).expect("the curve reaches the band");
        // Its `y` is 120·t·(1 − t), which is 20.5 where `t` is 0.2187 and
        // 0.7813, and its `x` is then 17.79 and 42.21.
        assert!((found.0 - 17.79).abs() < 0.01, "{found:?}");
        assert!((found.1 - 42.21).abs() < 0.01, "{found:?}");
        assert_eq!(ends(&pieces(line, 2.0, &[curve], 1.0), 1.0).len(), 2);
    }

    #[test]
    fn gaps_that_meet_are_one_gap_and_a_gap_past_the_end_cuts_it() {
        let line = Rect::new(4.0, 20.0, 60.0, 1.0);
        let glyphs = [
            square(2.0, 15.0, 6.0),
            square(30.0, 15.0, 6.0),
            square(37.0, 15.0, 6.0),
            square(62.0, 15.0, 6.0),
        ];
        assert_eq!(
            ends(&pieces(line, 1.0, &glyphs, 1.0), 1.0),
            vec![(9, 29), (44, 61)]
        );
    }

    #[test]
    fn a_line_every_glyph_covers_is_no_pieces() {
        let line = Rect::new(4.0, 20.0, 10.0, 1.0);
        assert!(pieces(line, 1.0, &[square(0.0, 15.0, 20.0)], 1.0).is_empty());
    }

    #[test]
    fn at_two_the_gap_is_in_device_pixels() {
        // The same shape at two: ink 60 to 80 device pixels across a line
        // 3.0 device pixels thick, so a gap from 57 to 83 — a half CSS pixel
        // each end.
        let line = Rect::new(0.0, 20.0, 100.0, 1.5);
        let pieces = pieces(line, 3.0, &[bar(30.0, 10.0)], 2.0);
        assert_eq!(ends(&pieces, 2.0), vec![(0, 57), (83, 200)]);
    }

    #[test]
    fn nothing_that_is_not_a_number_breaks_the_line() {
        let line = Rect::new(0.0, 20.0, 100.0, 1.0);
        let mut nonsense = Path::new();
        nonsense.move_to(Point::new(f32::NAN, 20.5));
        nonsense.line_to(Point::new(f32::INFINITY, 10.0));
        nonsense.quad_to(Point::new(f32::NAN, f32::NAN), Point::new(5.0, f32::NAN));
        nonsense.close();
        assert_eq!(
            ends(&pieces(line, 1.0, &[nonsense], 1.0), 1.0),
            vec![(0, 100)]
        );
        for scale in [0.0, -1.0, f32::NAN] {
            let at = pieces(line, 1.0, &[square(30.0, 15.0, 10.0)], scale);
            assert_eq!(ends(&at, 1.0), vec![(0, 29), (41, 100)]);
        }
    }

    // Each expectation below is a gap read from Chrome 154's screenshots
    // (the module's documentation): the device column the line stops at and
    // the one it starts again at. Each glyph's pen is where Chrome's layout
    // put it, and the line's row and thickness are `crate::decoration`'s.

    fn sans() -> Font {
        Font::load(
            "DejaVu Sans",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::sans::regular().to_vec(),
        )
        .expect("the font this crate is tested with")
    }

    /// One letter's shape with its pen at `pen`, as drawn at `scale`.
    fn letter(font: &Font, character: char, size: f32, scale: f32, pen: (f32, f32)) -> Path {
        let id = shape(&character.to_string(), font, size, Direction::LeftToRight)
            .glyphs
            .first()
            .map(|glyph| glyph.glyph_id)
            .expect("one glyph");
        outline_at(font, id, size, scale)
            .expect("a glyph")
            .path
            .translated(Point::new(pen.0, pen.1))
    }

    /// Where the line breaks around one letter: the gap's ends.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "whole device pixels on a small page"
    )]
    fn gap(line: Rect, thickness: f32, glyph: &Path, scale: f32) -> (i32, i32) {
        let pieces = pieces(line, thickness, core::slice::from_ref(glyph), scale);
        assert_eq!(pieces.len(), 2, "one gap: {pieces:?}");
        (
            (pieces[0].right() * scale).round() as i32,
            (pieces[1].left() * scale).round() as i32,
        )
    }

    #[test]
    fn a_g_at_thirty_two_is_chromes_gap() {
        // 32 px at one: 3.2 thick, drawn as three rows from 1950; the
        // baseline two above. The `g`'s pen at 33.109375.
        let font = sans();
        let g = letter(&font, 'g', 32.0, 1.0, (33.109_375, 1948.0));
        let line = Rect::new(4.0, 1950.0, 400.0, 3.0);
        assert_eq!(gap(line, 3.2, &g, 1.0), (34, 53));
    }

    #[test]
    fn a_g_at_sixty_four_at_two_reaches_its_control_point() {
        // 64 px at two: 12.8 thick, drawn as twelve device rows from 6872;
        // the baseline seven above, at 6865. The right of the gap is the
        // descender's control point, not its curve: the curve alone would end
        // the gap at 205.
        let font = sans();
        let g = letter(&font, 'g', 64.0, 2.0, (62.218_75, 3432.5));
        let line = Rect::new(4.0, 3436.0, 800.0, 6.0);
        assert_eq!(gap(line, 12.8, &g, 2.0), (127, 207));
        let j = letter(&font, 'j', 64.0, 2.0, (123.1875, 3432.5));
        assert_eq!(gap(line, 12.8, &j, 2.0), (231, 283));
    }

    #[test]
    fn a_line_at_ten_px_looks_along_one_rows_middle() {
        // 10 px at one: 1.0 thick, one row at 152, the baseline one above.
        let font = sans();
        let line = Rect::new(4.0, 152.0, 200.0, 1.0);
        let g = letter(&font, 'g', 10.0, 1.0, (13.093_75, 151.0));
        assert_eq!(gap(line, 1.0, &g, 1.0), (13, 19));
        let j = letter(&font, 'j', 10.0, 1.0, (22.609_375, 151.0));
        assert_eq!(gap(line, 1.0, &j, 1.0), (21, 25));
    }

    #[test]
    fn an_overline_steps_aside_for_an_accent() {
        // 64 px at one, overlined: six rows from 3511, over a piece whose
        // top is 3517 and baseline 59 below that.
        let font = sans();
        let e = letter(&font, 'É', 64.0, 1.0, (62.218_75, 3576.0));
        let line = Rect::new(4.0, 3511.0, 500.0, 6.0);
        assert_eq!(gap(line, 6.4, &e, 1.0), (78, 97));
    }
}
