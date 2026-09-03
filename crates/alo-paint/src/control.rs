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
//! # What it draws over
//!
//! The **padding box** — inside the border, not over it. A page that sets a
//! border on a checkbox can see that border and asked for it; what the control
//! draws for its own state goes inside. Browsers replace the border entirely
//! when a box is checked; that would be one fewer thing an author can control,
//! for no gain in legibility.
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

/// The colour a control draws its state in when the page has not said.
///
/// A blue rather than the text colour: a control's state is a different kind
/// of thing from the words around it, and every platform says so in colour. A
/// page that wants another one writes `accent-color`.
pub const DEFAULT_ACCENT: Rgba = Rgba {
    red: 0.043,
    green: 0.341,
    blue: 0.816,
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

/// Where a mark is drawn: the largest square that fits in the control.
///
/// A square rather than the box itself, so that a checkbox somebody made
/// forty pixels wide draws a tick rather than a stretched one. A mark is a
/// symbol, and a symbol that changes shape with its container stops being
/// readable as the same symbol.
fn field(within: Rect) -> Rect {
    let side = within.size.width.min(within.size.height).max(0.0);
    Rect::new(
        within.left() + (within.size.width - side) / 2.0,
        within.top() + (within.size.height - side) / 2.0,
        side,
        side,
    )
}

/// The tick, as a closed six-point outline in a unit square.
///
/// A filled polygon rather than a stroked polyline, because this engine fills
/// paths and does not stroke them — the two straight strokes of a check and
/// their mitred join, written out. Reading them in order: down the right side
/// of the long stroke, back up the short one, and along the top of both.
const TICK: [(f32, f32); 6] = [
    (0.7825, 0.190),
    (0.8975, 0.305),
    (0.3925, 0.810),
    (0.1025, 0.520),
    (0.2175, 0.405),
    (0.3925, 0.580),
];

fn tick(field: Rect) -> Path {
    let mut path = Path::new();
    for (index, (x, y)) in TICK.iter().enumerate() {
        let point = Point::new(
            field.left() + x * field.size.width,
            field.top() + y * field.size.height,
        );
        if index == 0 {
            path.move_to(point);
        } else {
            path.line_to(point);
        }
    }
    path.close();
    path
}

/// How much of the control's width a dash covers, and how thick it is.
const DASH: (f32, f32) = (0.62, 0.18);

fn dash(field: Rect) -> Path {
    let width = field.size.width * DASH.0;
    let height = field.size.height * DASH.1;
    Path::rectangle(
        field.left() + (field.size.width - width) / 2.0,
        field.top() + (field.size.height - height) / 2.0,
        width,
        height,
    )
}

/// How much of the control a radio's dot covers across.
const DOT: f32 = 0.52;

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
        // The numbers in `TICK` were written by hand, so this is the assertion
        // that says they are the numbers of a *centred* tick rather than of
        // one that leans.
        let control = Rect::new(10.0, 20.0, 12.0, 12.0);
        for kind in [Mark::Tick, Mark::Dash, Mark::Dot] {
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
    fn a_control_with_no_room_draws_a_mark_with_no_area() {
        // A zero-sized control is not a crash and not a mark drawn somewhere
        // else: it is a mark of no size, which draws nothing.
        let (left, top, right, bottom) =
            bounds_of(&mark(Mark::Tick, Rect::new(5.0, 5.0, 0.0, 0.0)));
        assert!((right - left).abs() < f32::EPSILON && (bottom - top).abs() < f32::EPSILON);
    }
}
