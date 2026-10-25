/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where each thing in the window goes, in device pixels.
//!
//! Geometry and nothing else, so that it is asserted in numbers (`CLAUDE.md`:
//! *"a layout is a tree with numbers in it"*) and [`crate::compose`] only has
//! to fill the rectangles this hands it.
//!
//! # The scale factor
//!
//! ADR 0057 § 3: a page is laid out and painted for the scale of the screen
//! its window is on, and so is the strip. Both renderers are told the
//! window's [`Scale`] with its size ([`screen`]) and send frames in **device
//! pixels**, which the window places one to one — nothing is replicated or
//! stretched (queue item 562). The scale is the screen's own when it is one
//! this engine has measured, 1.5 among them (queue item 599), and the
//! largest measured one below it otherwise ([`Scale::of_screen`]). A page is
//! laid out in whole CSS pixels, the most that fit the window at the scale,
//! and any device pixel left over is background.

use crate::agent_lines::MOST_AGENT_LINES;
use crate::scale::Scale;
use alo_layout::Size;
use alo_renderer::Screen;

/// How tall the line saying what happened to a tab is, in CSS pixels.
pub const NOTICE_HEIGHT: u32 = 32;

/// How tall the tab strip is, in CSS pixels: six above the tabs and thirty of
/// tab, as `alo-renderer`'s `interface/strip.css` draws them (ADR 0024 § 4).
/// The strip's renderer is told this height and the window's width; the page
/// is laid out in what is left below it.
pub const STRIP_HEIGHT: u32 = 36;

/// A rectangle of whole device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    /// Its left edge.
    pub x: u32,
    /// Its top edge.
    pub y: u32,
    /// How wide.
    pub width: u32,
    /// How tall.
    pub height: u32,
}

impl PixelRect {
    /// Whether it covers no pixel at all.
    pub fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// The size a page in this window is laid out at: the most whole CSS
/// pixels that fit the window's width at the scale, and the most that fit
/// below the strip's band ([`Scale::fitting`]).
///
/// What a renderer is told in a load or a `Resize`, so a frame painted at it
/// and at the scale never runs past the window's edge. A window no taller
/// than the strip leaves a page no height at all.
pub fn viewport(window: (u32, u32), scale: Scale) -> Size {
    let below = window.1.saturating_sub(scale.device(STRIP_HEIGHT));
    Size::new(
        css_length(scale.fitting(window.0)),
        css_length(scale.fitting(below)),
    )
}

/// What a page in this window is laid out for: [`viewport`]'s size on a
/// screen of `scale` (ADR 0057 § 3), as a renderer is told it.
pub fn screen(window: (u32, u32), scale: Scale) -> Screen {
    Screen::new(viewport(window, scale)).at_scale(scale.factor())
}

/// A whole number of CSS pixels as a length.
#[expect(
    clippy::cast_precision_loss,
    reason = "at most `LARGEST_WINDOW`, which an f32 holds exactly"
)]
fn css_length(pixels: u32) -> f32 {
    pixels as f32
}

/// The size the strip is drawn at, above a page laid out at `page`: as wide,
/// and [`STRIP_HEIGHT`] tall.
pub fn strip_viewport(page: Size) -> Size {
    #[expect(clippy::cast_precision_loss, reason = "thirty-six is exact in an f32")]
    Size::new(page.width, STRIP_HEIGHT as f32)
}

/// What there is to place in a window: the sizes of the frames it was last
/// sent, in device pixels, and which of them has something to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Contents {
    /// The strip's last frame, if it has painted one.
    pub strip: Option<(u32, u32)>,
    /// The selected tab's last frame, if it has painted one.
    pub frame: Option<(u32, u32)>,
    /// Whether there is a sentence about the tab.
    pub tab_saying: bool,
    /// Whether there is a sentence about the strip.
    pub strip_saying: bool,
    /// How many sentences there are saying what agents did, or why they
    /// were refused (queue items 623 and 625): at most
    /// [`MOST_AGENT_LINES`] are placed.
    pub agent_lines: usize,
}

/// Where the strip, a tab's frame and the lines about them go in a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    /// The band across the top the strip is drawn in, [`STRIP_HEIGHT`] at
    /// the scale, the window's whole width — cut at its bottom edge.
    pub band: PixelRect,
    /// The part of the band the strip's frame covers, from its top-left
    /// corner, cut at the band's edges.
    pub strip: PixelRect,
    /// The part of the window the tab's frame covers, one device pixel to
    /// each of its pixels, from just below the band — cut at the window's edge when the frame is larger,
    /// which it is after a window shrinks and before the new frame arrives.
    pub frame: PixelRect,
    /// The line saying what happened to the tab, along the bottom of the
    /// window, when there is something to say.
    pub notice: Option<PixelRect>,
    /// The line saying what happened to the strip, above the tab's line —
    /// or along the bottom when the tab has nothing to say.
    pub strip_notice: Option<PixelRect>,
    /// The lines saying what agents did, in the words they were answered
    /// with (queue items 623 and 625), oldest first: above both of those, or
    /// along the bottom when neither has anything to say, the newest
    /// lowest. Entry `i` is the `i`th sentence, and there is none past the
    /// last.
    pub agent_lines: [Option<PixelRect>; MOST_AGENT_LINES],
}

impl Placement {
    /// Where things go in a `window` of device pixels at `scale`, for what
    /// it holds.
    ///
    /// The band and the lines are their heights in CSS pixels at the scale,
    /// as many device pixels as a renderer paints that many ([`Scale::device`]):
    /// at 1.5 the band is 54 and a line 48.
    ///
    /// A frame is placed at its own size in device pixels, which is what its
    /// renderer painted at the scale it was told. A frame larger than its
    /// place is cut, never scaled: what was painted is shown as it was
    /// painted until the renderer paints the new size or the new scale (ADR
    /// 0024 § 2). The notices are drawn over the page's frame rather than
    /// pushing it up, for the same reason — the frame is a picture of a page
    /// at a size, and moving it would show it at a size it was not — and
    /// never over the strip, which is the browser's own.
    pub fn of(window: (u32, u32), scale: Scale, contents: Contents) -> Self {
        let band = PixelRect {
            x: 0,
            y: 0,
            width: window.0,
            height: scale.device(STRIP_HEIGHT).min(window.1),
        };
        let below = window.1 - band.height;
        let cut = |size: Option<(u32, u32)>, room: (u32, u32)| {
            let (width, height) = size.unwrap_or((0, 0));
            (width.min(room.0), height.min(room.1))
        };
        let (strip_width, strip_height) = cut(contents.strip, (band.width, band.height));
        let (frame_width, frame_height) = cut(contents.frame, (window.0, below));
        let line = scale.device(NOTICE_HEIGHT);
        // From the bottom up: the tab's line, then the strip's, each as much
        // of its height as is left above the band.
        let mut bottom = window.1;
        let mut stack = |saying: bool| {
            saying.then(|| {
                let height = line.min(bottom - band.height);
                bottom -= height;
                PixelRect {
                    x: 0,
                    y: bottom,
                    width: window.0,
                    height,
                }
            })
        };
        let notice = stack(contents.tab_saying);
        let strip_notice = stack(contents.strip_saying);
        // The newest of the agents' lines lowest, so stacked first.
        let count = contents.agent_lines.min(MOST_AGENT_LINES);
        let mut agent_lines = [None; MOST_AGENT_LINES];
        for slot in agent_lines.iter_mut().take(count).rev() {
            *slot = stack(true);
        }
        Self {
            band,
            strip: PixelRect {
                x: 0,
                y: 0,
                width: strip_width,
                height: strip_height,
            },
            frame: PixelRect {
                x: 0,
                y: band.height,
                width: frame_width,
                height: frame_height,
            },
            notice,
            strip_notice,
            agent_lines,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::MOST_SCALE;

    #[test]
    fn a_renderer_is_told_the_viewport_on_a_screen_of_the_whole_scale() {
        let at_two = screen((960, 872), Scale::TWO);
        assert_eq!(at_two.size, Size::new(480.0, 400.0));
        assert_eq!(at_two.scale().to_bits(), 2.0_f32.to_bits());
        let at_one = screen((480, 436), Scale::ONE);
        assert_eq!(at_one.size, Size::new(480.0, 400.0));
        assert_eq!(at_one.scale().to_bits(), 1.0_f32.to_bits());
        // At 1.5, the most whole CSS pixels that fit: 1,333 of 2,000 device
        // pixels across, and 897 of the 1,346 below a band of 54.
        let half = screen((2000, 1400), Scale::ONE_AND_A_HALF);
        assert_eq!(half.size, Size::new(1333.0, 897.0));
        assert_eq!(half.scale().to_bits(), 1.5_f32.to_bits());
        let most = screen((800, 800), Scale::whole(u32::MAX));
        assert_eq!(most.size, Size::new(100.0, 64.0));
        assert_eq!(most.scale().to_bits(), 8.0_f32.to_bits());
    }

    fn rect(x: u32, y: u32, width: u32, height: u32) -> PixelRect {
        PixelRect {
            x,
            y,
            width,
            height,
        }
    }

    /// A tab's frame and nothing to say.
    fn a_frame(frame: (u32, u32)) -> Contents {
        Contents {
            frame: Some(frame),
            ..Contents::default()
        }
    }

    #[test]
    fn a_page_is_laid_out_in_css_pixels_below_the_strip() {
        assert_eq!(viewport((960, 872), Scale::TWO), Size::new(480.0, 400.0));
        assert_eq!(viewport((480, 436), Scale::ONE), Size::new(480.0, 400.0));
        assert_eq!(
            viewport((720, 654), Scale::ONE_AND_A_HALF),
            Size::new(480.0, 400.0)
        );
        // An odd device pixel is background, never half a CSS pixel.
        assert_eq!(viewport((961, 873), Scale::TWO), Size::new(480.0, 400.0));
        // At 1.5 a device pixel short of a CSS pixel's one and a half is
        // background too.
        assert_eq!(
            viewport((721, 655), Scale::ONE_AND_A_HALF),
            Size::new(480.0, 400.0)
        );
        assert_eq!(
            viewport((719, 653), Scale::ONE_AND_A_HALF),
            Size::new(479.0, 399.0)
        );
        // A window no taller than the strip leaves the page nothing.
        assert_eq!(viewport((100, 36), Scale::ONE), Size::new(100.0, 0.0));
        assert_eq!(viewport((1, 1), Scale::TWO), Size::new(0.0, 0.0));
        assert_eq!(
            viewport((3, 54), Scale::ONE_AND_A_HALF),
            Size::new(2.0, 0.0)
        );
    }

    #[test]
    fn the_strip_is_as_wide_as_the_page_and_its_own_height() {
        assert_eq!(
            strip_viewport(Size::new(480.0, 400.0)),
            Size::new(480.0, 36.0)
        );
    }

    #[test]
    fn the_band_is_the_strips_height_across_the_whole_window() {
        let placed = Placement::of((960, 872), Scale::TWO, Contents::default());
        assert_eq!(placed.band, rect(0, 0, 960, 72));
        assert!(placed.strip.is_empty(), "no strip has painted");
        assert!(placed.frame.is_empty(), "no tab has painted");
        assert_eq!((placed.notice, placed.strip_notice), (None, None));
        assert_eq!(placed.agent_lines, [None; MOST_AGENT_LINES]);
    }

    #[test]
    fn the_strip_covers_the_band_and_the_page_is_below_it() {
        let placed = Placement::of(
            (960, 872),
            Scale::TWO,
            Contents {
                strip: Some((960, 72)),
                frame: Some((960, 800)),
                ..Contents::default()
            },
        );
        assert_eq!(placed.strip, rect(0, 0, 960, 72));
        assert_eq!(placed.frame, rect(0, 72, 960, 800));
    }

    #[test]
    fn a_frame_is_placed_at_its_own_size_in_device_pixels_never_replicated() {
        // A frame painted at one, shown in a window at two before its
        // renderer paints at two: as it was painted, a quarter of the room.
        let placed = Placement::of(
            (960, 872),
            Scale::TWO,
            Contents {
                strip: Some((480, 36)),
                frame: Some((480, 400)),
                ..Contents::default()
            },
        );
        assert_eq!(placed.strip, rect(0, 0, 480, 36));
        assert_eq!(placed.frame, rect(0, 72, 480, 400));
    }

    #[test]
    fn at_one_and_a_half_the_band_and_the_lines_are_their_heights_at_it() {
        let placed = Placement::of(
            (720, 654),
            Scale::ONE_AND_A_HALF,
            Contents {
                strip: Some((720, 54)),
                frame: Some((720, 600)),
                tab_saying: true,
                strip_saying: true,
                agent_lines: 1,
            },
        );
        assert_eq!(placed.band, rect(0, 0, 720, 54));
        assert_eq!(placed.strip, rect(0, 0, 720, 54));
        assert_eq!(placed.frame, rect(0, 54, 720, 600));
        assert_eq!(placed.notice, Some(rect(0, 606, 720, 48)));
        assert_eq!(placed.strip_notice, Some(rect(0, 558, 720, 48)));
        assert_eq!(
            placed.agent_lines,
            [Some(rect(0, 510, 720, 48)), None, None]
        );
    }

    #[test]
    fn a_strip_painted_larger_than_the_band_is_cut_to_it() {
        let placed = Placement::of(
            (300, 436),
            Scale::ONE,
            Contents {
                strip: Some((480, 50)),
                ..Contents::default()
            },
        );
        assert_eq!(placed.strip, rect(0, 0, 300, 36));
    }

    #[test]
    fn a_window_grown_before_its_frame_shows_the_old_frame_at_its_old_size() {
        let placed = Placement::of((560, 496), Scale::ONE, a_frame((480, 400)));
        assert_eq!(placed.frame, rect(0, 36, 480, 400));
    }

    #[test]
    fn a_window_shrunk_before_its_frame_cuts_the_old_frame_at_its_edge() {
        let placed = Placement::of((300, 200), Scale::TWO, a_frame((480, 400)));
        assert_eq!(placed.frame, rect(0, 72, 300, 128));
    }

    #[test]
    fn the_notice_runs_along_the_bottom_at_its_height_in_css_pixels() {
        let saying = Contents {
            tab_saying: true,
            ..a_frame((480, 400))
        };
        let placed = Placement::of((480, 436), Scale::ONE, saying);
        assert_eq!(placed.notice, Some(rect(0, 404, 480, 32)));
        let doubled = Placement::of(
            (960, 872),
            Scale::TWO,
            Contents {
                tab_saying: true,
                ..a_frame((960, 800))
            },
        );
        assert_eq!(doubled.notice, Some(rect(0, 808, 960, 64)));
        // Over the frame, which stays where and as big as it was painted.
        assert_eq!(doubled.frame, rect(0, 72, 960, 800));
    }

    #[test]
    fn the_strips_notice_is_above_the_tabs_or_along_the_bottom_alone() {
        let both = Placement::of(
            (480, 436),
            Scale::ONE,
            Contents {
                tab_saying: true,
                strip_saying: true,
                ..Contents::default()
            },
        );
        assert_eq!(both.notice, Some(rect(0, 404, 480, 32)));
        assert_eq!(both.strip_notice, Some(rect(0, 372, 480, 32)));
        let alone = Placement::of(
            (480, 436),
            Scale::ONE,
            Contents {
                strip_saying: true,
                ..Contents::default()
            },
        );
        assert_eq!(alone.notice, None);
        assert_eq!(alone.strip_notice, Some(rect(0, 404, 480, 32)));
    }

    #[test]
    fn what_an_agent_did_is_a_line_above_the_notices_or_along_the_bottom_alone() {
        let alone = Placement::of(
            (480, 436),
            Scale::ONE,
            Contents {
                agent_lines: 1,
                ..a_frame((480, 400))
            },
        );
        assert_eq!(alone.agent_lines, [Some(rect(0, 404, 480, 32)), None, None]);
        assert_eq!((alone.notice, alone.strip_notice), (None, None));
        // Over the frame, which stays where and as big as it was painted.
        assert_eq!(alone.frame, rect(0, 36, 480, 400));
        let with_the_tabs = Placement::of(
            (480, 436),
            Scale::ONE,
            Contents {
                tab_saying: true,
                agent_lines: 1,
                ..Contents::default()
            },
        );
        assert_eq!(with_the_tabs.notice, Some(rect(0, 404, 480, 32)));
        assert_eq!(
            with_the_tabs.agent_lines,
            [Some(rect(0, 372, 480, 32)), None, None]
        );
        let doubled = Placement::of(
            (960, 872),
            Scale::TWO,
            Contents {
                tab_saying: true,
                strip_saying: true,
                agent_lines: 1,
                ..Contents::default()
            },
        );
        assert_eq!(doubled.notice, Some(rect(0, 808, 960, 64)));
        assert_eq!(doubled.strip_notice, Some(rect(0, 744, 960, 64)));
        assert_eq!(
            doubled.agent_lines,
            [Some(rect(0, 680, 960, 64)), None, None]
        );
    }

    #[test]
    fn a_window_shorter_than_its_notices_gives_them_what_is_below_the_band() {
        let placed = Placement::of(
            (100, 56),
            Scale::ONE,
            Contents {
                tab_saying: true,
                strip_saying: true,
                ..Contents::default()
            },
        );
        assert_eq!(placed.notice, Some(rect(0, 36, 100, 20)));
        assert_eq!(placed.strip_notice, Some(rect(0, 36, 100, 0)));
        // And a window shorter than the band is all band, saying nothing.
        let band = Placement::of(
            (100, 20),
            Scale::ONE,
            Contents {
                tab_saying: true,
                ..Contents::default()
            },
        );
        assert_eq!(band.band, rect(0, 0, 100, 20));
        assert!(band.notice.is_some_and(PixelRect::is_empty));
    }

    #[test]
    fn a_frame_larger_than_any_window_is_cut_at_the_windows_edge() {
        let placed = Placement::of(
            (100, 400),
            Scale::whole(MOST_SCALE),
            Contents {
                strip: Some((u32::MAX, u32::MAX)),
                frame: Some((u32::MAX, u32::MAX)),
                ..Contents::default()
            },
        );
        assert_eq!((placed.strip.width, placed.strip.height), (100, 288));
        assert_eq!((placed.frame.width, placed.frame.height), (100, 112));
    }

    #[test]
    fn several_agent_lines_stack_up_the_newest_lowest_and_no_more_than_three() {
        let two = Placement::of(
            (480, 436),
            Scale::ONE,
            Contents {
                tab_saying: true,
                agent_lines: 2,
                ..a_frame((480, 400))
            },
        );
        assert_eq!(two.notice, Some(rect(0, 404, 480, 32)));
        assert_eq!(
            two.agent_lines,
            [
                Some(rect(0, 340, 480, 32)),
                Some(rect(0, 372, 480, 32)),
                None
            ]
        );
        // At 1.5, three of 48, and a fourth is not placed.
        let many = Placement::of(
            (720, 654),
            Scale::ONE_AND_A_HALF,
            Contents {
                agent_lines: 4,
                ..a_frame((720, 600))
            },
        );
        assert_eq!(
            many.agent_lines,
            [
                Some(rect(0, 510, 720, 48)),
                Some(rect(0, 558, 720, 48)),
                Some(rect(0, 606, 720, 48)),
            ]
        );
        assert_eq!(many.frame, rect(0, 54, 720, 600));
        // A window too short for them gives the oldest what is left, or
        // nothing, and never the band.
        let short = Placement::of(
            (100, 80),
            Scale::ONE,
            Contents {
                agent_lines: 2,
                ..Contents::default()
            },
        );
        assert_eq!(
            short.agent_lines,
            [Some(rect(0, 36, 100, 12)), Some(rect(0, 48, 100, 32)), None]
        );
    }
}
