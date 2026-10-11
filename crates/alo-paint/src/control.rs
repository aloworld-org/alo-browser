/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a form control draws to say what state it is in.
//!
//! A checked checkbox and an unchecked one were the same picture — a bordered
//! square — from the day controls were built until queue item 182. The state
//! was right in the agent tree the whole time (`checkbox "Bacon"
//! [checked=true]`), and wrong on the screen, which is the worse way round: a
//! person cannot read the tree.
//!
//! # Why this is not in the user-agent style sheet
//!
//! For the same reason `alo_box::Purpose::Control` is not: a tick is not a
//! property. CSS has no way to say "and draw a check inside it", and the
//! nearest thing — a `::before` with a character in it — would put the mark at
//! the mercy of whichever font happened to load. This is what `appearance:
//! auto` means in a real browser: the control draws itself.
//!
//! # Two kinds of control draw a mark
//!
//! An `<input type=checkbox>` or `type=radio` draws Chrome's whole picture
//! of itself — its edge, its inside, its fill — and that is
//! [`crate::toggle`]. Anything else an author made into a checkbox with
//! `role` and `aria-checked` has no picture of its own: it draws its state
//! inside its **padding box**, over its own background and inside the
//! border its author can see and set. Both draw the same marks, from here.
//!
//! # The marks are Chrome's
//!
//! Measured from Chrome 154's screenshots at eight device pixels to one,
//! at 13 and 20 pixels square (queue item 454), by fitting each shape's
//! coverage pixel by pixel: the tick is a stroke along three points, the
//! dash a rounded bar, the dot a circle six tenths across. Each is a
//! fraction of the control, not a number of pixels, because Chrome's grow
//! with the control.
//!
//! # The colours
//!
//! `accent-color`, which is the property CSS has for exactly this question,
//! falling back to [`DEFAULT_ACCENT`]. The mark itself is black or white by
//! whichever shows up against the accent — so `accent-color: yellow` gives a
//! black tick rather than an invisible white one.

use crate::corner::{Corners, rounded_rectangle};
use crate::path::{Path, Point};
use alo_box::{Checked, KnownRole, Role};
use alo_layout::Rect;
use alo_value::Rgba;

/// The colour a control draws its state in when the page has not said:
/// Chrome 154's `#0075ff`, measured from its screenshots (queue item 454).
///
/// A blue rather than the text colour: a control's state is a different kind
/// of thing from the words around it, and every platform says so in colour. A
/// page that wants another one writes `accent-color`.
pub const DEFAULT_ACCENT: Rgba = Rgba {
    red: 0.0,
    green: 117.0 / 255.0,
    blue: 1.0,
    alpha: 1.0,
};

/// The colour a control that cannot be operated draws its state in.
///
/// The same grey as the user-agent sheet's control border, deliberately: one
/// grey for "this control is not live", in both places that draw one.
///
/// It is a **muted** state rather than an absent one. "You cannot change this"
/// and "this is off" are different things to be told, and a disabled control
/// that drew nothing would be telling somebody the second when the first is
/// true.
pub const DISABLED_ACCENT: Rgba = Rgba {
    red: 0.463,
    green: 0.463,
    blue: 0.463,
    alpha: 1.0,
};

/// What a control draws inside itself to say what state it is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// This is on. What a checked checkbox draws.
    Tick,
    /// Neither on nor off. What a checkbox in the mixed state draws — the
    /// "select all" box above a list where some of the list is selected.
    Dash,
    /// This is the one chosen. What a checked radio draws.
    Dot,
}

impl Mark {
    /// Which mark a box with this role in this state draws, if any.
    ///
    /// The mark follows **the shape of the answer** rather than the element:
    /// a radio is one-of-several and gets a dot, a checkbox is on-or-off and
    /// gets a tick. A `switch` is on-or-off too, so it gets the tick — which
    /// is also what a browser draws, since `role` is a declaration to
    /// assistive technology and does not change how a control is rendered.
    ///
    /// `aria-checked="mixed"` on a **radio** draws nothing, because ARIA says
    /// a radio in the mixed state is to be treated as unchecked: one of a
    /// group either is the chosen one or is not.
    pub fn for_state(role: &Role, checked: Option<Checked>) -> Option<Mark> {
        let Role::Known(known) = role else {
            return None;
        };
        match (known, checked?) {
            (KnownRole::Radio, Checked::Yes) => Some(Mark::Dot),
            (KnownRole::CheckBox | KnownRole::Switch, Checked::Yes) => Some(Mark::Tick),
            (KnownRole::CheckBox | KnownRole::Switch, Checked::Mixed) => Some(Mark::Dash),
            _ => None,
        }
    }
}

/// The colour a control's own background is filled with behind its mark.
///
/// `accent-color` if the page set one; grey if the control is disabled,
/// whatever the page set, because a page cannot make a dead control look live.
pub fn accent(style: Option<&alo_style::ComputedStyle>, disabled: bool) -> Rgba {
    if disabled {
        return DISABLED_ACCENT;
    }
    style
        .and_then(|style| style.color("accent-color"))
        .unwrap_or(DEFAULT_ACCENT)
}

/// The colour the mark itself is drawn in: whichever of black and white shows
/// up against the accent.
///
/// A fixed white tick would vanish inside `accent-color: yellow`, and a page
/// that set a pale accent would have made its own controls unreadable without
/// being able to see why.
pub fn mark_color(accent: Rgba) -> Rgba {
    // The sRGB luminance weights. Green carries most of what an eye reads as
    // brightness, which is why a mid-green background wants a black mark and a
    // mid-blue one wants white.
    let luminance = 0.2126 * accent.red + 0.7152 * accent.green + 0.0722 * accent.blue;
    if luminance > 0.5 {
        Rgba::BLACK
    } else {
        Rgba::WHITE
    }
}

/// The shape of a mark, drawn to fill the given area.
pub fn mark(kind: Mark, within: Rect) -> Path {
    let field = field(within);
    match kind {
        Mark::Tick => tick(field),
        Mark::Dash => dash(field),
        Mark::Dot => dot(field),
    }
}

/// Where a mark is drawn: the largest square that fits in the control,
/// centred in it.
///
/// A square rather than the box itself, so that a checkbox somebody made
/// forty pixels wide draws a tick rather than a stretched one. A mark is a
/// symbol, and a symbol that changes shape with its container stops being
/// readable as the same symbol. Chrome 154 does the same: a checkbox 30 by
/// 13 draws a 13-pixel square in its middle (queue item 454).
pub(crate) fn field(within: Rect) -> Rect {
    let side = within.size.width.min(within.size.height).max(0.0);
    Rect::new(
        within.left() + (within.size.width - side) / 2.0,
        within.top() + (within.size.height - side) / 2.0,
        side,
        side,
    )
}

/// The tick: a stroke through three points of a unit square, a sixth of
/// it wide, with flat ends and a sharp corner at the bottom.
///
/// Chrome 154's, fitted to its screenshots: at 20 pixels square, eight
/// device pixels to one, this stroke's coverage differs from Chrome's by
/// 0.2% of a pixel on average.
const TICK: [(f32, f32); 3] = [(0.2, 0.5), (0.4, 0.7), (0.8, 0.2)];

/// How wide the tick's stroke is, as a share of the control.
const TICK_WIDTH: f32 = 0.16;

/// The stroke along [`TICK`], written out as the outline a stroke with flat
/// ends and a mitred corner covers: down one side of both strokes and back
/// up the other.
///
/// Written out rather than handed to the rented stroker, because the shape
/// is fixed and six points are a shape that can be read and tested; a
/// stroker that could fail would make a tick that could fail.
fn tick(field: Rect) -> Path {
    let at = |(x, y): (f32, f32)| {
        (
            field.left() + x * field.size.width,
            field.top() + y * field.size.height,
        )
    };
    let [start, corner, end] = TICK.map(at);
    let half = TICK_WIDTH * field.size.height / 2.0;
    let first = offset(start, corner, half);
    let second = offset(corner, end, half);
    let mut path = Path::new();
    let mut point = |(x, y): (f32, f32), first_point: bool| {
        if first_point {
            path.move_to(Point::new(x, y));
        } else {
            path.line_to(Point::new(x, y));
        }
    };
    // One side of the stroke, then the other: each side is the two strokes'
    // edges on it, meeting where their lines cross.
    for (index, side) in [1.0_f32, -1.0].into_iter().enumerate() {
        let along = |(x, y): (f32, f32), (dx, dy): (f32, f32)| (x + side * dx, y + side * dy);
        let ends = [along(start, first), along(end, second)];
        let meet = crossing(
            (along(start, first), along(corner, first)),
            (along(corner, second), along(end, second)),
        )
        .unwrap_or_else(|| along(corner, first));
        let [from, to] = if index == 0 { ends } else { [ends[1], ends[0]] };
        point(from, index == 0);
        point(meet, false);
        point(to, false);
    }
    path.close();
    path
}

/// The offset, `half` long, square to the line from `from` to `to`.
fn offset(from: (f32, f32), to: (f32, f32), half: f32) -> (f32, f32) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = dx.hypot(dy);
    if length > 0.0 {
        (-dy / length * half, dx / length * half)
    } else {
        (0.0, 0.0)
    }
}

/// Where the line through `a` crosses the line through `b`, if they cross.
fn crossing(a: ((f32, f32), (f32, f32)), b: ((f32, f32), (f32, f32))) -> Option<(f32, f32)> {
    let ((ax, ay), (ax2, ay2)) = a;
    let ((bx, by), (bx2, by2)) = b;
    let (adx, ady) = (ax2 - ax, ay2 - ay);
    let (bdx, bdy) = (bx2 - bx, by2 - by);
    let denominator = adx * bdy - ady * bdx;
    if denominator.abs() <= f32::EPSILON {
        return None;
    }
    let t = ((bx - ax) * bdy - (by - ay) * bdx) / denominator;
    Some((ax + t * adx, ay + t * ady))
}

/// How much of the control's width a dash covers, and how thick it is: a
/// bar with round ends, Chrome 154's, fitted to its screenshots.
const DASH: (f32, f32) = (0.62, 0.15);

fn dash(field: Rect) -> Path {
    let width = field.size.width * DASH.0;
    let height = field.size.height * DASH.1;
    let bar = Rect::new(
        field.left() + (field.size.width - width) / 2.0,
        field.top() + (field.size.height - height) / 2.0,
        width,
        height,
    );
    rounded_rectangle(bar, Corners::all(height / 2.0))
}

/// How much of the control a radio's dot covers across: Chrome 154's, 7.8
/// pixels of 13 and 12 of 20.
const DOT: f32 = 0.6;

fn dot(field: Rect) -> Path {
    let diameter = field.size.width.min(field.size.height) * DOT;
    let rect = Rect::new(
        field.left() + (field.size.width - diameter) / 2.0,
        field.top() + (field.size.height - diameter) / 2.0,
        diameter,
        diameter,
    );
    // A circle is a rounded rectangle whose radii are half its side, which is
    // the same four arcs `corner.rs` already draws — so there is one circle in
    // this crate rather than two that could disagree.
    rounded_rectangle(rect, Corners::all(diameter / 2.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_checked_checkbox_ticks_and_an_unchecked_one_draws_nothing() {
        let role = Role::Known(KnownRole::CheckBox);
        assert_eq!(Mark::for_state(&role, Some(Checked::Yes)), Some(Mark::Tick));
        assert_eq!(Mark::for_state(&role, Some(Checked::No)), None);
        // A box that cannot be checked says nothing about checkedness, and
        // draws nothing rather than drawing "off".
        assert_eq!(Mark::for_state(&role, None), None);
    }

    #[test]
    fn a_mixed_checkbox_is_a_third_thing_rather_than_either() {
        let role = Role::Known(KnownRole::CheckBox);
        let marks: Vec<Option<Mark>> = [Checked::No, Checked::Yes, Checked::Mixed]
            .into_iter()
            .map(|checked| Mark::for_state(&role, Some(checked)))
            .collect();
        assert_eq!(marks, vec![None, Some(Mark::Tick), Some(Mark::Dash)]);
    }

    #[test]
    fn a_checked_radio_is_a_dot_rather_than_a_tick() {
        assert_eq!(
            Mark::for_state(&Role::Known(KnownRole::Radio), Some(Checked::Yes)),
            Some(Mark::Dot)
        );
    }

    #[test]
    fn a_radio_in_the_mixed_state_is_treated_as_unchecked() {
        // ARIA's own rule: `mixed` is not a state one of a group can be in.
        assert_eq!(
            Mark::for_state(&Role::Known(KnownRole::Radio), Some(Checked::Mixed)),
            None
        );
    }

    #[test]
    fn a_switch_is_on_or_off_and_so_it_ticks() {
        assert_eq!(
            Mark::for_state(&Role::Known(KnownRole::Switch), Some(Checked::Yes)),
            Some(Mark::Tick)
        );
    }

    #[test]
    fn something_that_is_not_a_control_draws_no_mark() {
        assert_eq!(
            Mark::for_state(&Role::Known(KnownRole::Button), Some(Checked::Yes)),
            None
        );
        assert_eq!(Mark::for_state(&Role::Presentational, None), None);
    }

    #[test]
    fn a_disabled_control_still_draws_its_state_in_a_different_colour() {
        // The two halves of the same rule: the mark is still chosen, and the
        // colour says the control is not live.
        assert_eq!(
            Mark::for_state(&Role::Known(KnownRole::CheckBox), Some(Checked::Yes)),
            Some(Mark::Tick)
        );
        assert_eq!(accent(None, true), DISABLED_ACCENT);
        assert_eq!(accent(None, false), DEFAULT_ACCENT);
        assert_ne!(DISABLED_ACCENT, DEFAULT_ACCENT);
    }

    #[test]
    fn a_mark_shows_up_against_its_accent_either_way() {
        assert_eq!(mark_color(DEFAULT_ACCENT), Rgba::WHITE);
        assert_eq!(mark_color(Rgba::WHITE), Rgba::BLACK);
        assert_eq!(mark_color(Rgba::BLACK), Rgba::WHITE);
        // Yellow is the case a fixed white tick would have disappeared into.
        assert_eq!(mark_color(Rgba::new(1.0, 1.0, 0.0, 1.0)), Rgba::BLACK);
    }

    fn bounds_of(path: &Path) -> (f32, f32, f32, f32) {
        path.bounds().unwrap_or((0.0, 0.0, 0.0, 0.0))
    }

    #[test]
    fn every_mark_is_centred_in_its_control() {
        // Chrome's tick is not centred — it sits a little low and a little
        // right, and is drawn where Chrome draws it — so it is the dash and
        // the dot that are.
        let control = Rect::new(10.0, 20.0, 12.0, 12.0);
        for kind in [Mark::Dash, Mark::Dot] {
            let (left, top, right, bottom) = bounds_of(&mark(kind, control));
            let across =
                f32::midpoint(left, right) - f32::midpoint(control.left(), control.right());
            let down = f32::midpoint(top, bottom) - f32::midpoint(control.top(), control.bottom());
            assert!(
                across.abs() < 0.05 && down.abs() < 0.05,
                "{kind:?} is off centre by ({across}, {down})"
            );
        }
    }

    #[test]
    fn every_mark_stays_inside_its_control() {
        let control = Rect::new(10.0, 20.0, 12.0, 12.0);
        for kind in [Mark::Tick, Mark::Dash, Mark::Dot] {
            let (left, top, right, bottom) = bounds_of(&mark(kind, control));
            assert!(
                left >= control.left()
                    && top >= control.top()
                    && right <= control.right()
                    && bottom <= control.bottom(),
                "{kind:?} sticks out of its control: {:?}",
                (left, top, right, bottom)
            );
        }
    }

    #[test]
    fn a_mark_in_a_wide_control_is_still_square() {
        // A checkbox somebody made forty pixels wide draws a tick, not a
        // stretched one.
        let (left, top, right, bottom) =
            bounds_of(&mark(Mark::Tick, Rect::new(0.0, 0.0, 40.0, 12.0)));
        let width = right - left;
        let height = bottom - top;
        let square = bounds_of(&mark(Mark::Tick, Rect::new(0.0, 0.0, 12.0, 12.0)));
        assert!((width - (square.2 - square.0)).abs() < 0.01);
        assert!((height - (square.3 - square.1)).abs() < 0.01);
    }

    #[test]
    fn the_three_marks_are_three_different_shapes() {
        let control = Rect::new(0.0, 0.0, 13.0, 13.0);
        let tick = mark(Mark::Tick, control);
        let dash = mark(Mark::Dash, control);
        let dot = mark(Mark::Dot, control);
        assert_ne!(bounds_of(&tick), bounds_of(&dash));
        assert_ne!(bounds_of(&dash), bounds_of(&dot));
        assert_ne!(bounds_of(&tick), bounds_of(&dot));
        // A dot is round and the other two are not: a circle is arcs.
        assert!(
            dot.segments()
                .iter()
                .any(|segment| matches!(segment, crate::path::Segment::CubicTo(..)))
        );
        assert!(
            !tick
                .segments()
                .iter()
                .any(|segment| matches!(segment, crate::path::Segment::CubicTo(..)))
        );
    }

    #[test]
    fn the_tick_is_chromes_stroke_through_three_points() {
        // At 20 pixels the stroke is 3.2 wide, so its flat end at the top
        // right is 3.2 long, square to the long stroke, and centred on
        // (16, 4).
        let tick = mark(Mark::Tick, Rect::new(0.0, 0.0, 20.0, 20.0));
        let points: Vec<Point> = tick
            .segments()
            .iter()
            .filter_map(|segment| match segment {
                crate::path::Segment::MoveTo(to) | crate::path::Segment::LineTo(to) => Some(*to),
                _ => None,
            })
            .collect();
        assert_eq!(points.len(), 6, "six corners: {points:?}");
        let near =
            |point: Point, x: f32, y: f32| (point.x - x).abs() < 0.01 && (point.y - y).abs() < 0.01;
        // The long stroke's direction is (8, -10); its half-width offset is
        // 1.6 along (10, 8) / √164.
        let (dx, dy) = (1.6 * 10.0 / 164.0_f32.sqrt(), 1.6 * 8.0 / 164.0_f32.sqrt());
        let top_end: Vec<&Point> = points
            .iter()
            .filter(|point| {
                near(**point, 16.0 + dx, 4.0 + dy) || near(**point, 16.0 - dx, 4.0 - dy)
            })
            .collect();
        assert_eq!(top_end.len(), 2, "the flat end at the top: {points:?}");
        // The short stroke's end is centred on (4, 10), and runs along
        // (1, 1), so its end is square to that.
        let half = 1.6 / 2.0_f32.sqrt();
        assert!(
            points
                .iter()
                .any(|point| near(*point, 4.0 + half, 10.0 - half))
        );
        assert!(
            points
                .iter()
                .any(|point| near(*point, 4.0 - half, 10.0 + half))
        );
    }

    #[test]
    fn a_control_with_no_room_draws_a_mark_with_no_area() {
        // A zero-sized control is not a crash and not a mark drawn somewhere
        // else: it is a mark of no size, which draws nothing.
        let (left, top, right, bottom) =
            bounds_of(&mark(Mark::Tick, Rect::new(5.0, 5.0, 0.0, 0.0)));
        assert!((right - left).abs() < f32::EPSILON && (bottom - top).abs() < f32::EPSILON);
    }
}
