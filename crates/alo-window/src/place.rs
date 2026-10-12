/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where each thing in the window goes, in device pixels.
//!
//! Geometry and nothing else, so that it is asserted in numbers (`CLAUDE.md`:
//! *"a layout is a tree with numbers in it"*) and [`crate::compose`] only has
//! to fill the rectangles this hands it.
//!
//! # The scale factor, until item 299
//!
//! ADR 0024 § 3: a renderer paints in CSS pixels today, and a window on most
//! Macs has two device pixels to every one of those. Until the renderer paints
//! at the window's scale, **each pixel of a frame is replicated** to the whole
//! part of the scale factor — geometry right, text coarse — and a fractional
//! remainder is left as background rather than stretched. The page is told
//! the window's size in CSS pixels, and no ratio at all.

use alo_layout::Size;

/// The largest number of device pixels one CSS pixel is replicated to.
///
/// No screen has a scale factor near it; the bound is there so that a window
/// server reporting nonsense costs a window of background rather than a
/// multiplication that overflows.
pub const MOST_REPLICATION: u32 = 8;

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

/// How many device pixels one CSS pixel becomes: the whole part of the
/// window's scale factor, at least one and at most [`MOST_REPLICATION`].
///
/// A factor that is not a finite number, or is below one, is one — the window is
/// drawn at the size it says it is rather than not drawn.
pub fn replication(scale_factor: f64) -> u32 {
    if !scale_factor.is_finite() {
        return 1;
    }
    // Counted down rather than cast, so there is no conversion to get wrong:
    // the largest whole number the factor reaches.
    (1..=MOST_REPLICATION)
        .rev()
        .find(|whole| scale_factor >= f64::from(*whole))
        .unwrap_or(1)
}

/// The size a page in this window is laid out at: the window's device pixels
/// divided by the replication, rounded down, less the strip above it.
///
/// What a renderer is told in a load or a `Resize`, so a frame painted at it
/// and replicated never runs past the window's edge. A window no taller than
/// the strip leaves a page no height at all.
pub fn viewport(window: (u32, u32), replication: u32) -> Size {
    let replication = replication.max(1);
    // Whole CSS pixels, so a frame replicated by a whole number covers whole
    // device pixels.
    #[expect(
        clippy::cast_precision_loss,
        reason = "a whole number below 2^24 is exact in an f32, and no window is that wide"
    )]
    Size::new(
        (window.0 / replication) as f32,
        (window.1 / replication).saturating_sub(STRIP_HEIGHT) as f32,
    )
}

/// The size the strip is drawn at, above a page laid out at `page`: as wide,
/// and [`STRIP_HEIGHT`] tall.
pub fn strip_viewport(page: Size) -> Size {
    #[expect(clippy::cast_precision_loss, reason = "thirty-six is exact in an f32")]
    Size::new(page.width, STRIP_HEIGHT as f32)
}

/// What there is to place in a window: the sizes of the frames it was last
/// sent, in CSS pixels, and which of them has something to say.
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
}

/// Where the strip, a tab's frame and the lines about them go in a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    /// The band across the top the strip is drawn in, [`STRIP_HEIGHT`]
    /// replicated, the window's whole width — cut at its bottom edge.
    pub band: PixelRect,
    /// The part of the band the strip's frame covers, from its top-left
    /// corner, cut at the band's edges.
    pub strip: PixelRect,
    /// The part of the window the tab's frame covers, replicated, from just
    /// below the band — cut at the window's edge when the frame is larger,
    /// which it is after a window shrinks and before the new frame arrives.
    pub frame: PixelRect,
    /// The line saying what happened to the tab, along the bottom of the
    /// window, when there is something to say.
    pub notice: Option<PixelRect>,
    /// The line saying what happened to the strip, above the tab's line —
    /// or along the bottom when the tab has nothing to say.
    pub strip_notice: Option<PixelRect>,
}

impl Placement {
    /// Where things go in a `window` of device pixels, replicating each CSS
    /// pixel `replication` times, for what it holds.
    ///
    /// A frame larger than its place is cut, never scaled: what was painted
    /// is shown as it was painted until the renderer paints the new size (ADR
    /// 0024 § 2). The notices are drawn over the page's frame rather than
    /// pushing it up, for the same reason — the frame is a picture of a page
    /// at a size, and moving it would show it at a size it was not — and
    /// never over the strip, which is the browser's own.
    pub fn of(window: (u32, u32), replication: u32, contents: Contents) -> Self {
        let replication = replication.max(1);
        let band = PixelRect {
            x: 0,
            y: 0,
            width: window.0,
            height: STRIP_HEIGHT.saturating_mul(replication).min(window.1),
        };
        let below = window.1 - band.height;
        let cut = |size: Option<(u32, u32)>, room: (u32, u32)| {
            let (width, height) = size.unwrap_or((0, 0));
            (
                width.saturating_mul(replication).min(room.0),
                height.saturating_mul(replication).min(room.1),
            )
        };
        let (strip_width, strip_height) = cut(contents.strip, (band.width, band.height));
        let (frame_width, frame_height) = cut(contents.frame, (window.0, below));
        let line = NOTICE_HEIGHT.saturating_mul(replication);
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_whole_scale_factor_is_replicated_whole() {
        assert_eq!(replication(1.0), 1);
        assert_eq!(replication(2.0), 2);
        assert_eq!(replication(3.0), 3);
    }

    #[test]
    fn a_fractional_scale_factor_is_rounded_down() {
        assert_eq!(replication(1.5), 1);
        assert_eq!(replication(2.25), 2);
        assert_eq!(replication(2.999), 2);
    }

    #[test]
    fn a_scale_factor_that_is_nonsense_is_one_or_the_most() {
        assert_eq!(replication(0.0), 1);
        assert_eq!(replication(0.5), 1);
        assert_eq!(replication(-2.0), 1);
        assert_eq!(replication(f64::NAN), 1);
        assert_eq!(replication(f64::INFINITY), 1);
        assert_eq!(replication(1e300), MOST_REPLICATION);
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
        assert_eq!(viewport((960, 872), 2), Size::new(480.0, 400.0));
        assert_eq!(viewport((480, 436), 1), Size::new(480.0, 400.0));
        // An odd device pixel is background, never half a CSS pixel.
        assert_eq!(viewport((961, 873), 2), Size::new(480.0, 400.0));
        // A window no taller than the strip leaves the page nothing.
        assert_eq!(viewport((100, 36), 1), Size::new(100.0, 0.0));
        assert_eq!(viewport((1, 1), 2), Size::new(0.0, 0.0));
        assert_eq!(viewport((10, 46), 0), Size::new(10.0, 10.0));
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
        let placed = Placement::of((960, 872), 2, Contents::default());
        assert_eq!(placed.band, rect(0, 0, 960, 72));
        assert!(placed.strip.is_empty(), "no strip has painted");
        assert!(placed.frame.is_empty(), "no tab has painted");
        assert_eq!((placed.notice, placed.strip_notice), (None, None));
    }

    #[test]
    fn the_strip_covers_the_band_and_the_page_is_below_it() {
        let placed = Placement::of(
            (960, 872),
            2,
            Contents {
                strip: Some((480, 36)),
                frame: Some((480, 400)),
                ..Contents::default()
            },
        );
        assert_eq!(placed.strip, rect(0, 0, 960, 72));
        assert_eq!(placed.frame, rect(0, 72, 960, 800));
    }

    #[test]
    fn a_strip_painted_larger_than_the_band_is_cut_to_it() {
        let placed = Placement::of(
            (300, 436),
            1,
            Contents {
                strip: Some((480, 50)),
                ..Contents::default()
            },
        );
        assert_eq!(placed.strip, rect(0, 0, 300, 36));
    }

    #[test]
    fn a_window_grown_before_its_frame_shows_the_old_frame_at_its_old_size() {
        let placed = Placement::of((560, 496), 1, a_frame((480, 400)));
        assert_eq!(placed.frame, rect(0, 36, 480, 400));
    }

    #[test]
    fn a_window_shrunk_before_its_frame_cuts_the_old_frame_at_its_edge() {
        let placed = Placement::of((300, 200), 2, a_frame((480, 400)));
        assert_eq!(placed.frame, rect(0, 72, 300, 128));
    }

    #[test]
    fn the_notice_runs_along_the_bottom_at_its_height_in_css_pixels() {
        let saying = Contents {
            tab_saying: true,
            ..a_frame((480, 400))
        };
        let placed = Placement::of((480, 436), 1, saying);
        assert_eq!(placed.notice, Some(rect(0, 404, 480, 32)));
        let doubled = Placement::of((960, 872), 2, saying);
        assert_eq!(doubled.notice, Some(rect(0, 808, 960, 64)));
        // Over the frame, which stays where and as big as it was painted.
        assert_eq!(doubled.frame, rect(0, 72, 960, 800));
    }

    #[test]
    fn the_strips_notice_is_above_the_tabs_or_along_the_bottom_alone() {
        let both = Placement::of(
            (480, 436),
            1,
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
            1,
            Contents {
                strip_saying: true,
                ..Contents::default()
            },
        );
        assert_eq!(alone.notice, None);
        assert_eq!(alone.strip_notice, Some(rect(0, 404, 480, 32)));
    }

    #[test]
    fn a_window_shorter_than_its_notices_gives_them_what_is_below_the_band() {
        let placed = Placement::of(
            (100, 56),
            1,
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
            1,
            Contents {
                tab_saying: true,
                ..Contents::default()
            },
        );
        assert_eq!(band.band, rect(0, 0, 100, 20));
        assert!(band.notice.is_some_and(PixelRect::is_empty));
    }

    #[test]
    fn a_frame_too_large_to_multiply_is_cut_rather_than_overflowing() {
        let placed = Placement::of(
            (100, 400),
            MOST_REPLICATION,
            Contents {
                strip: Some((u32::MAX, u32::MAX)),
                frame: Some((u32::MAX, u32::MAX)),
                ..Contents::default()
            },
        );
        assert_eq!((placed.strip.width, placed.strip.height), (100, 288));
        assert_eq!((placed.frame.width, placed.frame.height), (100, 112));
    }
}
