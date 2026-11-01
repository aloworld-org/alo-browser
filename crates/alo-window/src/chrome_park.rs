/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a card's window is parked so that the Chromium alo carries draws
//! its page at the conversation's scale (queue item 772, ADR 0090 as
//! amended).
//!
//! A Chromium told `--force-device-scale-factor` keeps that scale for its
//! life: nothing it takes once running changes it — not its window moved
//! onto a display of another, nor `Emulation.setDeviceMetricsOverride`,
//! which tells the page two and streams the frame at one, nor a stream
//! started again or its window resized (measured, the amendment). Started
//! without it, a Chromium draws a window at the scale of the display that
//! holds the most of it, and draws it again at another's as soon as the
//! window is moved there, while it streams — Chrome and alo's CEF host
//! alike. And the window server keeps a strip of a window asked off every
//! display on one: 40 points of it inside a display's right edge, 32 rows
//! inside its bottom edge, 153 points inside the leftmost display's left
//! edge, wherever no other display is.
//!
//! So the Chromium alo carries, whose parked window is drawn fully
//! transparent and lets a person's pointer through (ADR 0076 B4 as
//! amended), is started untold, and its window is **parked by a display at
//! the conversation's scale**: asked just past one of that display's edges
//! with no display beyond it ([`places`]), where the window server leaves
//! its strip on that display. Where it went is read back, and the scale of
//! the display holding the most of it ([`drawn_at`]) must be the one
//! asked, or the next place is tried; with none, it is parked where every
//! card was ([`PARKED`]) and drawn at whatever display keeps it. The
//! conversation's window moved to a display of another scale asks for the
//! card's stream at it, and the window is parked again by the new scale
//! (`chrome_card.rs`).
//!
//! The machine's Chrome is never parked so: its parked window cannot be
//! made unseen (ADR 0076 B4's amendment, § 3), and a strip of it beside
//! the conversation would be a strip of the page a person sees. It is told
//! its scale at start, as ADR 0090 decided, and keeps it (queue item 780).

use crate::chrome_protocol::Bounds;
use crate::chrome_tab::Tab;
use crate::chrome_window::{self, PARKED};
use crate::scale::Scale;
use crate::window_server::{self, Display};

/// Where a card's window was parked, and what that came to.
#[derive(Debug, Clone, PartialEq)]
pub struct Parked {
    /// The scale it was parked by.
    pub scale: Scale,
    /// Where it was asked to be, in the screen's points: where it is fitted
    /// while it stays parked, so that it stays where it was parked.
    pub at: (i32, i32),
    /// The scale of the display holding the most of it where it went, by
    /// [`drawn_at`] — or [`None`] when that could not be said.
    pub drawn: Option<Scale>,
    /// The displays it was parked by, as the window server listed them.
    pub displays: Vec<Display>,
}

/// Park `tab`'s window by a display drawing at `scale`: each of
/// [`places`] tried in turn until the window server leaves it on a display
/// at `scale`, read back; or, with none, at [`PARKED`]. Never brought in
/// front. Whatever is declined is said by [`Parked::drawn`], and the card
/// is streamed as it is drawn.
pub fn park(tab: &mut Tab<'_>, scale: Scale) -> Parked {
    // Unlisted — off macOS, or the window server not answering — there is
    // nothing to park by, and the window goes where every card's did.
    let displays = window_server::displays().unwrap_or_default();
    let size = chrome_window::placed(tab)
        .map(|placed| (placed.window.bounds.width, placed.window.bounds.height))
        .ok();
    for at in size.map_or_else(Vec::new, |size| places(&displays, scale, size)) {
        if let Ok(placed) = chrome_window::put_at(tab, at.0, at.1) {
            let drawn = drawn_at(placed.window.bounds, &displays);
            if drawn == Some(scale) {
                return Parked {
                    scale,
                    at,
                    drawn,
                    displays,
                };
            }
        }
    }
    let drawn = chrome_window::put_at(tab, PARKED.0, PARKED.1)
        .ok()
        .and_then(|placed| drawn_at(placed.window.bounds, &displays));
    Parked {
        scale,
        at: PARKED,
        drawn,
        displays,
    }
}

/// Where a window `size` points large is asked to be parked by a display
/// drawing at exactly `scale`, in the order tried: for each such display,
/// in the order listed, just past its right edge, its left edge and its
/// bottom edge, its top or left edge kept — each only where the window
/// asked there meets no display at all, so that the window server moves it
/// back onto that one. Above a display is never asked: a window asked there
/// is put wholly under the menu bar (measured).
pub fn places(displays: &[Display], scale: Scale, (width, height): (i64, i64)) -> Vec<(i32, i32)> {
    let mut places = Vec::new();
    for display in displays
        .iter()
        .filter(|display| Scale::named(display.factor) == Some(scale))
    {
        let edge = display.bounds;
        let right = edge.left.saturating_add(edge.width);
        let bottom = edge.top.saturating_add(edge.height);
        for (left, top) in [
            (right, edge.top),
            (edge.left.saturating_sub(width), edge.top),
            (edge.left, bottom),
        ] {
            let asked = Bounds {
                left,
                top,
                width,
                height,
            };
            if displays.iter().any(|other| area(asked, other.bounds) > 0) {
                continue;
            }
            if let (Ok(left), Ok(top)) = (i32::try_from(left), i32::try_from(top))
                && !places.contains(&(left, top))
            {
                places.push((left, top));
            }
        }
    }
    places
}

/// The scale a window at `window` is drawn at: that of the display holding
/// the most of it, as `AppKit` gives a window the screen holding the most of
/// it — or [`None`] when it is on none, or that display's scale is not one
/// this engine has measured.
pub fn drawn_at(window: Bounds, displays: &[Display]) -> Option<Scale> {
    let (most, display) = displays
        .iter()
        .map(|display| (area(window, display.bounds), display))
        .max_by_key(|(area, _)| *area)?;
    if most == 0 {
        return None;
    }
    Scale::named(display.factor)
}

/// How many square points two rectangles share.
fn area(one: Bounds, other: Bounds) -> i64 {
    let side = |start: i64, length: i64, other_start: i64, other_length: i64| {
        let end = start
            .saturating_add(length)
            .min(other_start.saturating_add(other_length));
        end.saturating_sub(start.max(other_start)).max(0)
    };
    side(one.left, one.width, other.left, other.width).saturating_mul(side(
        one.top,
        one.height,
        other.top,
        other.height,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(left: i64, top: i64, width: i64, height: i64) -> Bounds {
        Bounds {
            left,
            top,
            width,
            height,
        }
    }

    fn display(bounds: Bounds, factor: f64) -> Display {
        Display { bounds, factor }
    }

    /// This Mac's three displays, as the window server listed them (queue
    /// items 771 and 772): the Retina display at two at the origin, one at
    /// one left of it, and a portrait one at one left of that.
    fn three() -> Vec<Display> {
        vec![
            display(at(0, 0, 1710, 1107), 2.0),
            display(at(-3000, -80, 1080, 1920), 1.0),
            display(at(-1920, 0, 1920, 1080), 1.0),
        ]
    }

    /// A card's window, the page box and the title bar.
    const CARD: (i64, i64) = (878, 552);

    #[test]
    fn a_window_is_parked_past_a_free_edge_of_a_display_at_the_scale() {
        // At two: right of the Retina display and under it — never left
        // of it, where the display at one is.
        assert_eq!(places(&three(), Scale::TWO, CARD), [(1710, 0), (0, 1107)]);
        // At one: left of the portrait display, and under each display at
        // one — never right of the portrait one, where the other display
        // at one is, nor right of the other, where the Retina display is.
        assert_eq!(
            places(&three(), Scale::ONE, CARD),
            [(-3878, -80), (-3000, 1840), (-1920, 1080)]
        );
        // No display at the scale, nothing to park by.
        assert_eq!(places(&three(), Scale::ONE_AND_A_HALF, CARD), []);
        // One display: every edge but the top.
        let alone = [display(at(0, 0, 1440, 900), 2.0)];
        assert_eq!(
            places(&alone, Scale::TWO, CARD),
            [(1440, 0), (-878, 0), (0, 900)]
        );
        assert_eq!(places(&alone, Scale::ONE, CARD), []);
        // A display at 1.25 is no display at one: a window on it is drawn
        // at 1.25, which no card is asked at.
        let between = [display(at(0, 0, 1440, 900), 1.25)];
        assert_eq!(places(&between, Scale::ONE, CARD), []);
    }

    #[test]
    fn a_window_is_drawn_at_the_scale_of_the_display_holding_most_of_it() {
        // Where the window server put the windows asked at the places
        // above, measured through alo's CEF host: 40 points inside the
        // Retina display's right edge, at two; 153 inside the portrait
        // display's left edge, at one; 32 rows inside the bottom of the
        // display at one, at one.
        assert_eq!(drawn_at(at(1670, 34, 878, 552), &three()), Some(Scale::TWO));
        assert_eq!(drawn_at(at(-3725, 0, 878, 552), &three()), Some(Scale::ONE));
        assert_eq!(
            drawn_at(at(-1000, 1048, 878, 552), &three()),
            Some(Scale::ONE)
        );
        assert_eq!(drawn_at(at(0, 1075, 878, 552), &three()), Some(Scale::TWO));
        // Across the edge between the display at one and the Retina one:
        // the Retina display holds 478 of it, the other 400.
        assert_eq!(
            drawn_at(at(-400, 100, 878, 552), &three()),
            Some(Scale::TWO)
        );
        assert_eq!(
            drawn_at(at(-500, 100, 878, 552), &three()),
            Some(Scale::ONE)
        );
        // On no display, or only touching one, it is drawn at no scale
        // said here.
        assert_eq!(drawn_at(at(-20000, 0, 878, 552), &three()), None);
        assert_eq!(drawn_at(at(1710, 0, 878, 552), &three()), None);
        assert_eq!(drawn_at(at(0, 0, 878, 552), &[]), None);
        // On a display at a scale never measured.
        let between = [display(at(0, 0, 1440, 900), 1.25)];
        assert_eq!(drawn_at(at(0, 0, 878, 552), &between), None);
    }

    #[test]
    fn rectangles_share_what_they_overlap() {
        assert_eq!(area(at(0, 0, 10, 10), at(5, 5, 10, 10)), 25);
        assert_eq!(area(at(0, 0, 10, 10), at(10, 0, 10, 10)), 0);
        assert_eq!(area(at(0, 0, 10, 10), at(-20, -20, 5, 5)), 0);
        assert_eq!(area(at(0, 0, 10, 10), at(2, 3, 4, 5)), 20);
    }
}
