/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where each thing in the frame around Chrome's window goes, in device
//! pixels (ADR 0062, queue item 679).
//!
//! Geometry and nothing else, as [`crate::place`] is for this engine's own
//! window, so that it is asserted in numbers and
//! [`crate::compose::compose_frame`] only fills the rectangles this hands
//! it.
//!
//! # One band, read left to right
//!
//! The frame is a band as tall as the tab strip, standing on Chrome's
//! window, and it has no room below the band the way this engine's window
//! has room along its bottom. So everything is in the band, side by side:
//!
//! - **first, the page's tab** (queue item 681) — the strip its own
//!   renderer painted ([`crate::frame_strip`]), as wide as it was painted,
//!   or, when that renderer has said why it painted nothing, that sentence
//!   as wide as it needs — where a tab is in this engine's window, at the
//!   left of the band; nothing, until there is one;
//! - **then which renderer draws the page** — ADR 0058 § 6, *"somewhere a
//!   person can read, the product says what is rendering"* — as wide as its
//!   sentence needs, and cut only by the frame's own edge, so no agent's
//!   line can push it out of sight;
//! - **then what agents' verbs were answered with**, oldest first and the
//!   newest at the right, sharing what is left equally, [`GAP`] apart, the
//!   last taking the pixels a share cannot divide, so the lines reach the
//!   frame's right edge exactly.
//!
//! A line too long for its share is cut by [`crate::notice::Lettering`] to
//! an ellipsis, as a line of this engine's window too narrow for it is.

use crate::agent_lines::MOST_AGENT_LINES;
use crate::place::PixelRect;
use crate::scale::Scale;

/// How far apart the pieces of the band are, in CSS pixels: the frame's
/// own colour shows between them, so two refusals in a row read as two.
pub const GAP: u32 = 1;

/// Where the renderer's sentence and the agents' lines go in the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramePlacement {
    /// The page's tab, from the frame's left edge, the band's whole
    /// height: empty when there is no tab to show.
    pub strip: PixelRect,
    /// The sentence saying which renderer draws the page, after the tab,
    /// the band's whole height.
    pub renderer: PixelRect,
    /// The lines saying what agents did (queue items 623 and 625), oldest
    /// first. Entry `i` is the `i`th, and there is none past the last, nor
    /// any when the band has no room right of the renderer's sentence.
    pub agent_lines: [Option<PixelRect>; MOST_AGENT_LINES],
}

impl FramePlacement {
    /// Where things go in a frame of `window` device pixels at `scale`,
    /// whose page's tab is `strip` device pixels across — nought for none —
    /// and whose renderer's sentence needs `renderer`, for `lines` agents'
    /// lines — of which at most [`MOST_AGENT_LINES`] are placed.
    pub fn of(window: (u32, u32), scale: Scale, strip: u32, renderer: u32, lines: usize) -> Self {
        let (width, height) = window;
        let gap = scale.device(GAP);
        let strip = PixelRect {
            x: 0,
            y: 0,
            width: strip.min(width),
            height,
        };
        let after_strip = if strip.width == 0 {
            0
        } else {
            strip.width.saturating_add(gap).min(width)
        };
        let renderer = PixelRect {
            x: after_strip,
            y: 0,
            width: renderer.min(width.saturating_sub(after_strip)),
            height,
        };
        let mut agent_lines = [None; MOST_AGENT_LINES];
        let count = u32::try_from(lines.min(MOST_AGENT_LINES)).unwrap_or(0);
        let start = renderer
            .x
            .saturating_add(renderer.width)
            .saturating_add(gap);
        let room = width
            .saturating_sub(start)
            .saturating_sub(gap.saturating_mul(count.saturating_sub(1)));
        let each = room.checked_div(count).unwrap_or(0);
        if each == 0 || height == 0 {
            return Self {
                strip,
                renderer,
                agent_lines,
            };
        }
        let mut x = start;
        for (at, placed) in (1..=count).zip(agent_lines.iter_mut()) {
            let wide = if at == count {
                width.saturating_sub(x)
            } else {
                each
            };
            *placed = Some(PixelRect {
                x,
                y: 0,
                width: wide,
                height,
            });
            x = x.saturating_add(each).saturating_add(gap);
        }
        Self {
            strip,
            renderer,
            agent_lines,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: u32, width: u32, height: u32) -> PixelRect {
        PixelRect {
            x,
            y: 0,
            width,
            height,
        }
    }

    #[test]
    fn with_no_lines_the_band_holds_the_renderers_sentence_alone() {
        let placed = FramePlacement::of((2400, 72), Scale::TWO, 0, 600, 0);
        assert_eq!(placed.renderer, rect(0, 600, 72));
        assert_eq!(placed.agent_lines, [None; MOST_AGENT_LINES]);
    }

    #[test]
    fn one_line_takes_all_the_band_right_of_the_renderer() {
        let placed = FramePlacement::of((2400, 72), Scale::TWO, 0, 600, 1);
        assert_eq!(placed.renderer, rect(0, 600, 72));
        // Two device pixels of gap at two.
        assert_eq!(placed.agent_lines, [Some(rect(602, 1798, 72)), None, None]);
    }

    #[test]
    fn three_lines_share_what_is_left_and_the_last_reaches_the_edge() {
        // 1200 - 315 - 1 = 884 left at one, less two gaps: 882, 294 each.
        let placed = FramePlacement::of((1200, 36), Scale::ONE, 0, 315, 3);
        assert_eq!(
            placed.agent_lines,
            [
                Some(rect(316, 294, 36)),
                Some(rect(611, 294, 36)),
                Some(rect(906, 294, 36)),
            ]
        );
        // 883 left: the pixel a share cannot divide goes to the last.
        let placed = FramePlacement::of((1201, 36), Scale::ONE, 0, 315, 3);
        assert_eq!(placed.agent_lines[2], Some(rect(906, 295, 36)));
        for line in placed.agent_lines.into_iter().flatten() {
            assert!(line.x + line.width <= 1201);
        }
    }

    #[test]
    fn at_one_and_a_half_the_gap_is_two_device_pixels() {
        // One CSS pixel is two device pixels at 1.5 (`Scale::device`).
        // 901 - 402 - 2 = 497 left, 248 each and the odd pixel last.
        let placed = FramePlacement::of((901, 54), Scale::ONE_AND_A_HALF, 0, 400, 2);
        assert_eq!(placed.renderer, rect(0, 400, 54));
        assert_eq!(
            placed.agent_lines,
            [Some(rect(402, 248, 54)), Some(rect(652, 249, 54)), None]
        );
    }

    #[test]
    fn more_lines_than_are_kept_are_placed_as_the_most_that_are() {
        let most = FramePlacement::of((1200, 36), Scale::ONE, 0, 315, MOST_AGENT_LINES);
        assert_eq!(
            FramePlacement::of((1200, 36), Scale::ONE, 0, 315, MOST_AGENT_LINES + 4),
            most
        );
    }

    #[test]
    fn a_frame_narrower_than_the_renderers_sentence_holds_only_it_cut() {
        let placed = FramePlacement::of((300, 36), Scale::ONE, 0, 315, 2);
        assert_eq!(placed.renderer, rect(0, 300, 36));
        assert_eq!(placed.agent_lines, [None; MOST_AGENT_LINES]);
        // Room for less than a pixel each is no room.
        let placed = FramePlacement::of((318, 36), Scale::ONE, 0, 315, 3);
        assert_eq!(placed.agent_lines, [None; MOST_AGENT_LINES]);
    }

    #[test]
    fn the_pages_tab_comes_first_and_the_renderer_after_it_a_gap_apart() {
        // A tab of 216 by 36 at two, 432 device pixels: the renderer's
        // sentence two device pixels after it, one line after that.
        let placed = FramePlacement::of((2400, 72), Scale::TWO, 432, 600, 1);
        assert_eq!(placed.strip, rect(0, 432, 72));
        assert_eq!(placed.renderer, rect(434, 600, 72));
        assert_eq!(placed.agent_lines, [Some(rect(1036, 1364, 72)), None, None]);
        // At 1.5 the tab is 324 across and a gap is two.
        let placed = FramePlacement::of((1200, 54), Scale::ONE_AND_A_HALF, 324, 400, 0);
        assert_eq!(placed.strip, rect(0, 324, 54));
        assert_eq!(placed.renderer, rect(326, 400, 54));
    }

    #[test]
    fn with_no_tab_the_renderer_starts_at_the_left_edge() {
        let placed = FramePlacement::of((1200, 36), Scale::ONE, 0, 315, 1);
        assert!(placed.strip.is_empty());
        assert_eq!(placed.renderer, rect(0, 315, 36));
        assert_eq!(placed.agent_lines[0], Some(rect(316, 884, 36)));
    }

    #[test]
    fn a_frame_narrower_than_its_tab_holds_only_the_tab_cut() {
        let placed = FramePlacement::of((200, 36), Scale::ONE, 216, 315, 2);
        assert_eq!(placed.strip, rect(0, 200, 36));
        assert!(placed.renderer.is_empty(), "{placed:?}");
        assert_eq!(placed.renderer.x + placed.renderer.width, 200);
        assert_eq!(placed.agent_lines, [None; MOST_AGENT_LINES]);
        // Room for a gap and a little of the renderer's sentence.
        let placed = FramePlacement::of((300, 36), Scale::ONE, 216, 315, 1);
        assert_eq!(placed.renderer, rect(217, 83, 36));
        assert_eq!(placed.agent_lines, [None; MOST_AGENT_LINES]);
    }

    #[test]
    fn a_frame_of_no_size_places_nothing_with_size() {
        let placed = FramePlacement::of((0, 0), Scale::ONE, 0, 315, 2);
        assert!(placed.renderer.is_empty());
        let tabbed = FramePlacement::of((0, 0), Scale::ONE, 216, 315, 2);
        assert!(tabbed.strip.is_empty() && tabbed.renderer.is_empty());
        assert_eq!(placed.agent_lines, [None; MOST_AGENT_LINES]);
        let flat = FramePlacement::of((1200, 0), Scale::ONE, 0, 315, 2);
        assert_eq!(flat.agent_lines, [None; MOST_AGENT_LINES]);
    }
}
