/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which part of the conversation's window a point is over, and where in it
//! (ADR 0074, ADR 0024 § 5, queue item 710).
//!
//! As [`crate::aim`] is for the window of tabs: the browser process decides
//! from **its own composition** — the [`ConversationPlacement`] the window
//! was drawn from — never by asking a renderer. A point is the window
//! server's, in device pixels from the window's top-left corner; what a
//! part's renderer is sent is CSS pixels from its frame's corner, a device
//! pixel being `1 / scale` of one.
//!
//! A point is over a part where that part's last frame covers the window
//! and no line the window is saying something in is drawn over it: a press
//! on a line about a renderer is the browser's own drawing, nobody's to
//! claim. Where a part has painted nothing — before its first frame, or
//! where its frame does not reach after the window grew — a press is on its
//! room's background, and nobody's either: the person pressed nothing that
//! renderer drew.

use crate::aim::css;
use crate::conversation_drawing::Part;
use crate::conversation_place::ConversationPlacement;
use crate::place::PixelRect;
use crate::scale::Scale;
use alo_paint::Point;

/// What the point `at`, in device pixels, is over in a window placed as
/// `placed` at `scale`: a part, and where in it in its CSS pixels, or
/// [`None`] when it is over nothing a renderer drew.
pub fn aim(placed: &ConversationPlacement, scale: Scale, at: (f64, f64)) -> Option<(Part, Point)> {
    let over = |rect: Option<PixelRect>| rect.is_some_and(|rect| covers(rect, at));
    if over(placed.panel_notice) || over(placed.start_notice) {
        return None;
    }
    [(Part::Panel, placed.panel), (Part::Start, placed.start)]
        .into_iter()
        .find(|(_, frame)| covers(*frame, at))
        .map(|(part, frame)| {
            (
                part,
                css(scale, at.0 - f64::from(frame.x), at.1 - f64::from(frame.y)),
            )
        })
}

/// Whether `rect` covers the point `at`.
fn covers(rect: PixelRect, at: (f64, f64)) -> bool {
    let (left, top) = (f64::from(rect.x), f64::from(rect.y));
    (left..left + f64::from(rect.width)).contains(&at.0)
        && (top..top + f64::from(rect.height)).contains(&at.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation_place::Parts;

    /// The design's window at `scale`, both parts painted to fill their
    /// rooms, and lines said for both or neither.
    fn the_window(scale: Scale, saying: bool) -> ConversationPlacement {
        let device = |css: u32| scale.device(css);
        ConversationPlacement::of(
            (device(1280), device(820)),
            scale,
            Parts {
                panel: Some((device(240), device(820))),
                start: Some((device(1040), device(820))),
                panel_saying: saying,
                start_saying: saying,
            },
        )
    }

    fn on(part: Part, x: f32, y: f32) -> (Part, Point) {
        (part, Point::new(x, y))
    }

    #[test]
    fn the_left_240_is_the_panels_and_the_rest_the_first_screens() {
        let placed = the_window(Scale::ONE, false);
        assert_eq!(
            aim(&placed, Scale::ONE, (0.0, 0.0)),
            Some(on(Part::Panel, 0.0, 0.0))
        );
        assert_eq!(
            aim(&placed, Scale::ONE, (120.0, 153.0)),
            Some(on(Part::Panel, 120.0, 153.0))
        );
        assert_eq!(
            aim(&placed, Scale::ONE, (239.5, 819.5)),
            Some(on(Part::Panel, 239.5, 819.5))
        );
        assert_eq!(
            aim(&placed, Scale::ONE, (240.0, 0.0)),
            Some(on(Part::Start, 0.0, 0.0))
        );
        assert_eq!(
            aim(&placed, Scale::ONE, (640.0, 285.0)),
            Some(on(Part::Start, 400.0, 285.0))
        );
        assert_eq!(
            aim(&placed, Scale::ONE, (1279.0, 819.0)),
            Some(on(Part::Start, 1039.0, 819.0))
        );
    }

    #[test]
    fn at_two_a_point_is_halved_into_its_parts_css_pixels() {
        let placed = the_window(Scale::TWO, false);
        assert_eq!(
            aim(&placed, Scale::TWO, (240.0, 306.0)),
            Some(on(Part::Panel, 120.0, 153.0))
        );
        assert_eq!(
            aim(&placed, Scale::TWO, (479.0, 1.0)),
            Some(on(Part::Panel, 239.5, 0.5))
        );
        assert_eq!(
            aim(&placed, Scale::TWO, (1280.0, 570.0)),
            Some(on(Part::Start, 400.0, 285.0))
        );
        let at_one_and_a_half = the_window(Scale::ONE_AND_A_HALF, false);
        assert_eq!(
            aim(&at_one_and_a_half, Scale::ONE_AND_A_HALF, (360.0, 300.0)),
            Some(on(Part::Start, 0.0, 200.0))
        );
    }

    #[test]
    fn outside_the_window_or_over_a_line_is_nobodys() {
        let placed = the_window(Scale::ONE, false);
        for at in [(-0.5, 10.0), (1280.0, 10.0), (10.0, 820.0), (f64::NAN, 1.0)] {
            assert_eq!(aim(&placed, Scale::ONE, at), None, "{at:?}");
        }
        // Lines along the bottom: the first screen's from 788, the panel's
        // from 756, across the whole window.
        let saying = the_window(Scale::ONE, true);
        assert_eq!(aim(&saying, Scale::ONE, (100.0, 800.0)), None);
        assert_eq!(aim(&saying, Scale::ONE, (600.0, 760.0)), None);
        assert_eq!(
            aim(&saying, Scale::ONE, (100.0, 755.0)),
            Some(on(Part::Panel, 100.0, 755.0))
        );
    }

    #[test]
    fn where_a_part_has_painted_nothing_is_nobodys() {
        let nothing = ConversationPlacement::of((1280, 820), Scale::ONE, Parts::default());
        assert_eq!(aim(&nothing, Scale::ONE, (120.0, 150.0)), None);
        assert_eq!(aim(&nothing, Scale::ONE, (640.0, 285.0)), None);
        // A window grown past its frames: the first screen's frame is 1040
        // by 820 from 240, and below and right of it is nobody's.
        let grown = ConversationPlacement::of(
            (1400, 900),
            Scale::ONE,
            Parts {
                panel: Some((240, 820)),
                start: Some((1040, 820)),
                ..Parts::default()
            },
        );
        assert_eq!(aim(&grown, Scale::ONE, (1300.0, 100.0)), None);
        assert_eq!(aim(&grown, Scale::ONE, (100.0, 850.0)), None);
        assert_eq!(
            aim(&grown, Scale::ONE, (1279.0, 819.0)),
            Some(on(Part::Start, 1039.0, 819.0))
        );
    }
}
