/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where each thing in the frame around Chromium's window goes, in device
//! pixels (ADR 0062, queue items 679 and 991).
//!
//! Geometry and nothing else, as [`crate::place`] is for this engine's own
//! window, so that it is asserted in numbers and
//! [`crate::frame_compose::compose_frame`] only fills the rectangles this
//! hands it.
//!
//! # alo OS's window band, read left to right
//!
//! The frame is a band [`BAND_HEIGHT`] tall standing on Chromium's window,
//! white, with a one-pixel line along its foot (queue item 991). macOS's own
//! window buttons are on Chromium's window, not here. In the band, side by
//! side:
//!
//! - **first, the page's title and the application's name** — drawn by the
//!   frame's strip's renderer ([`crate::frame_strip`]), never here, because
//!   a title is a stranger's string (ADR 0024 § 4) — from the band's left
//!   edge, the band's whole height, a whole number of CSS pixels wide
//!   ([`FramePlacement::band_width`], what that renderer lays the band out
//!   at). With no agent's line it takes all the band left of which renderer
//!   draws the page; with one or more it takes half of that, so a title
//!   never pushes what an agent did out of sight, nor an agent's line the
//!   title. Or, when that renderer has said why it painted nothing, that
//!   sentence in the same place;
//! - **then what agents' verbs were answered with**, oldest first and the
//!   newest at the right, sharing what is left equally, [`GAP`] apart, the
//!   last taking the pixels a share cannot divide;
//! - **last, which renderer draws the page**, in plain words, against the
//!   band's right edge — ADR 0058 § 6, *"somewhere a person can read, the
//!   product says what is rendering"* — as wide as its sentence needs and
//!   cut only by the frame's own edge, so no agent's line and no title can
//!   push it out of sight. Right-aligned, as a window's quiet detail is,
//!   rather than after the application's name, whose end only the strip's
//!   renderer knows.
//!
//! The lines and the renderer's words stand above the foot's line, which
//! runs the band's whole width under everything.

use crate::agent_lines::MOST_AGENT_LINES;
use crate::place::PixelRect;
use crate::scale::Scale;

/// How tall the band is, in CSS pixels: alo OS's window band.
pub const BAND_HEIGHT: u32 = 54;

/// How far apart the pieces of the band are, in CSS pixels: the band's own
/// ground shows between them, so two refusals in a row read as two.
pub const GAP: u32 = 1;

/// Where each piece of the frame goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramePlacement {
    /// The page's title and the application's name, as the strip's
    /// renderer paints them, from the frame's left edge, the band's whole
    /// height: empty when there is no room for any.
    pub band: PixelRect,
    /// How wide the strip's renderer lays the band out, in CSS pixels:
    /// `band`'s width, which is this many CSS pixels' device pixels.
    pub band_width: u32,
    /// The sentence saying which renderer draws the page, against the
    /// frame's right edge, above the foot's line.
    pub renderer: PixelRect,
    /// The lines saying what agents did (queue items 623 and 625), oldest
    /// first, above the foot's line. Entry `i` is the `i`th, and there is
    /// none past the last, nor any when the band has no room for them.
    pub agent_lines: [Option<PixelRect>; MOST_AGENT_LINES],
    /// The line along the band's foot, one CSS pixel tall, edge to edge.
    pub line: PixelRect,
}

impl FramePlacement {
    /// Where things go in a frame of `window` device pixels at `scale`,
    /// whose renderer's sentence needs `renderer` device pixels, for
    /// `lines` agents' lines — of which at most [`MOST_AGENT_LINES`] are
    /// placed.
    pub fn of(window: (u32, u32), scale: Scale, renderer: u32, lines: usize) -> Self {
        let (width, height) = window;
        let gap = scale.device(GAP);
        let foot = scale.device(1).min(height);
        let above = height - foot;
        let line = PixelRect {
            x: 0,
            y: above,
            width,
            height: foot,
        };
        let said = renderer.min(width);
        let renderer = PixelRect {
            x: width - said,
            y: 0,
            width: said,
            height: above,
        };
        let count = u32::try_from(lines.min(MOST_AGENT_LINES)).unwrap_or(0);
        let left = renderer.x;
        let band_room = if count == 0 { left } else { left / 2 };
        let band_width = scale.fitting(band_room);
        let band = PixelRect {
            x: 0,
            y: 0,
            width: scale.device(band_width).min(band_room),
            height,
        };
        let mut agent_lines = [None; MOST_AGENT_LINES];
        let start = band.width.saturating_add(gap);
        let end = if said == 0 {
            width
        } else {
            left.saturating_sub(gap)
        };
        let room = end
            .saturating_sub(start)
            .saturating_sub(gap.saturating_mul(count.saturating_sub(1)));
        let each = room.checked_div(count).unwrap_or(0);
        if each > 0 && above > 0 {
            let mut x = start;
            for (at, placed) in (1..=count).zip(agent_lines.iter_mut()) {
                let wide = if at == count {
                    end.saturating_sub(x)
                } else {
                    each
                };
                *placed = Some(PixelRect {
                    x,
                    y: 0,
                    width: wide,
                    height: above,
                });
                x = x.saturating_add(each).saturating_add(gap);
            }
        }
        Self {
            band,
            band_width,
            renderer,
            agent_lines,
            line,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: u32, y: u32, width: u32, height: u32) -> PixelRect {
        PixelRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn with_no_lines_the_title_takes_all_left_of_the_renderer() {
        // 1200 by 54 at one, the renderer's words 280 across at the right.
        let placed = FramePlacement::of((1200, 54), Scale::ONE, 280, 0);
        assert_eq!(placed.renderer, rect(920, 0, 280, 53));
        assert_eq!(placed.band, rect(0, 0, 920, 54));
        assert_eq!(placed.band_width, 920);
        assert_eq!(placed.line, rect(0, 53, 1200, 1));
        assert_eq!(placed.agent_lines, [None; MOST_AGENT_LINES]);
        // At two, every number doubled, and the band laid out at the same
        // CSS width.
        let placed = FramePlacement::of((2400, 108), Scale::TWO, 560, 0);
        assert_eq!(placed.renderer, rect(1840, 0, 560, 106));
        assert_eq!(placed.band, rect(0, 0, 1840, 108));
        assert_eq!(placed.band_width, 920);
        assert_eq!(placed.line, rect(0, 106, 2400, 2));
    }

    #[test]
    fn one_line_takes_the_half_the_title_leaves() {
        let placed = FramePlacement::of((1200, 54), Scale::ONE, 280, 1);
        // 920 left of the renderer: the band 460, a pixel of gap, the line
        // to a pixel short of the renderer.
        assert_eq!(placed.band, rect(0, 0, 460, 54));
        assert_eq!(
            placed.agent_lines,
            [Some(rect(461, 0, 458, 53)), None, None]
        );
        let placed = FramePlacement::of((2400, 108), Scale::TWO, 560, 1);
        assert_eq!(placed.band, rect(0, 0, 920, 108));
        assert_eq!(placed.band_width, 460);
        assert_eq!(
            placed.agent_lines,
            [Some(rect(922, 0, 916, 106)), None, None]
        );
    }

    #[test]
    fn three_lines_share_what_is_left_and_the_last_reaches_the_renderer() {
        // 920 left, the band 460; 461 to 919 is 458, less two gaps 456,
        // 152 each.
        let placed = FramePlacement::of((1200, 54), Scale::ONE, 280, 3);
        assert_eq!(
            placed.agent_lines,
            [
                Some(rect(461, 0, 152, 53)),
                Some(rect(614, 0, 152, 53)),
                Some(rect(767, 0, 152, 53)),
            ]
        );
        // One more pixel to share: it goes to the last.
        let placed = FramePlacement::of((1201, 54), Scale::ONE, 280, 3);
        assert_eq!(placed.band.width, 460);
        assert_eq!(placed.agent_lines[2], Some(rect(767, 0, 153, 53)));
        assert_eq!(placed.renderer.x, 921);
    }

    #[test]
    fn at_one_and_a_half_the_band_is_whole_css_pixels_and_a_gap_two() {
        // 1500 by 81 device pixels is 1000 by 54 points at 1.5; the foot's
        // line is two device pixels, as one CSS pixel is at 1.5.
        let placed = FramePlacement::of((1500, 81), Scale::ONE_AND_A_HALF, 401, 0);
        assert_eq!(placed.line, rect(0, 79, 1500, 2));
        assert_eq!(placed.renderer, rect(1099, 0, 401, 79));
        // 1099 device pixels hold 732 CSS pixels' 1098.
        assert_eq!(placed.band_width, 732);
        assert_eq!(placed.band, rect(0, 0, 1098, 81));
        let placed = FramePlacement::of((1500, 81), Scale::ONE_AND_A_HALF, 401, 2);
        // Half of 1099 is 549, which holds 366 CSS pixels' 549.
        assert_eq!(placed.band_width, 366);
        assert_eq!(placed.band.width, 549);
        // 551 to 1097 is 546, less a gap of two 544, 272 each.
        assert_eq!(
            placed.agent_lines,
            [
                Some(rect(551, 0, 272, 79)),
                Some(rect(825, 0, 272, 79)),
                None
            ]
        );
    }

    #[test]
    fn more_lines_than_are_kept_are_placed_as_the_most_that_are() {
        let most = FramePlacement::of((1200, 54), Scale::ONE, 280, MOST_AGENT_LINES);
        assert_eq!(
            FramePlacement::of((1200, 54), Scale::ONE, 280, MOST_AGENT_LINES + 4),
            most
        );
    }

    #[test]
    fn a_frame_narrower_than_the_renderers_sentence_holds_only_it_cut() {
        let placed = FramePlacement::of((200, 54), Scale::ONE, 280, 2);
        assert_eq!(placed.renderer, rect(0, 0, 200, 53));
        assert!(placed.band.is_empty(), "{placed:?}");
        assert_eq!(placed.agent_lines, [None; MOST_AGENT_LINES]);
    }

    #[test]
    fn with_no_renderers_words_the_lines_reach_the_edge() {
        let placed = FramePlacement::of((1200, 54), Scale::ONE, 0, 1);
        assert_eq!(placed.renderer, rect(1200, 0, 0, 53));
        assert_eq!(placed.band, rect(0, 0, 600, 54));
        assert_eq!(placed.agent_lines[0], Some(rect(601, 0, 599, 53)));
    }

    #[test]
    fn a_frame_of_no_size_places_nothing_with_size() {
        let placed = FramePlacement::of((0, 0), Scale::ONE, 280, 2);
        assert!(placed.renderer.is_empty() && placed.band.is_empty());
        assert!(placed.line.is_empty());
        assert_eq!(placed.agent_lines, [None; MOST_AGENT_LINES]);
        let flat = FramePlacement::of((1200, 0), Scale::ONE, 280, 2);
        assert_eq!(flat.agent_lines, [None; MOST_AGENT_LINES]);
        assert!(flat.band.is_empty());
    }
}
