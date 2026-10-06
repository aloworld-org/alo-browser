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
/// divided by the replication, rounded down.
///
/// What a renderer is told in a load or a `Resize`, so a frame painted at it
/// and replicated never runs past the window's edge.
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
        (window.1 / replication) as f32,
    )
}

/// Where a tab's frame and the line about it go in a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    /// The part of the window the frame covers, replicated, from the top-left
    /// corner — cut at the window's edge when the frame is larger, which it is
    /// after a window shrinks and before the new frame arrives.
    pub frame: PixelRect,
    /// The line saying what happened, along the bottom of the window, when
    /// there is something to say.
    pub notice: Option<PixelRect>,
}

impl Placement {
    /// Where things go in a `window` of device pixels, replicating each CSS
    /// pixel `replication` times, for a frame of `frame` pixels (or none) and
    /// with or without a sentence to show.
    ///
    /// A frame larger than the window is cut, never scaled: what was painted
    /// is shown as it was painted until the renderer paints the new size (ADR
    /// 0024 § 2). The notice is drawn over the frame rather than pushing it up,
    /// for the same reason — the frame is a picture of a page at a size, and
    /// moving it would show it at a size it was not.
    pub fn of(
        window: (u32, u32),
        replication: u32,
        frame: Option<(u32, u32)>,
        saying: bool,
    ) -> Self {
        let replication = replication.max(1);
        let (frame_width, frame_height) = frame.unwrap_or((0, 0));
        let frame = PixelRect {
            x: 0,
            y: 0,
            width: frame_width.saturating_mul(replication).min(window.0),
            height: frame_height.saturating_mul(replication).min(window.1),
        };
        let notice = saying.then(|| {
            let height = NOTICE_HEIGHT.saturating_mul(replication).min(window.1);
            PixelRect {
                x: 0,
                y: window.1 - height,
                width: window.0,
                height,
            }
        });
        Self { frame, notice }
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

    #[test]
    fn a_page_is_laid_out_in_css_pixels() {
        assert_eq!(viewport((960, 800), 2), Size::new(480.0, 400.0));
        assert_eq!(viewport((480, 400), 1), Size::new(480.0, 400.0));
        // An odd device pixel is background, never half a CSS pixel.
        assert_eq!(viewport((961, 801), 2), Size::new(480.0, 400.0));
        assert_eq!(viewport((1, 1), 2), Size::new(0.0, 0.0));
        assert_eq!(viewport((10, 10), 0), Size::new(10.0, 10.0));
    }

    #[test]
    fn a_frame_the_size_of_the_window_covers_it() {
        let placed = Placement::of((960, 800), 2, Some((480, 400)), false);
        assert_eq!(
            placed.frame,
            PixelRect {
                x: 0,
                y: 0,
                width: 960,
                height: 800
            }
        );
        assert_eq!(placed.notice, None);
    }

    #[test]
    fn a_window_grown_before_its_frame_shows_the_old_frame_at_its_old_size() {
        let placed = Placement::of((560, 460), 1, Some((480, 400)), false);
        assert_eq!(
            placed.frame,
            PixelRect {
                x: 0,
                y: 0,
                width: 480,
                height: 400
            }
        );
    }

    #[test]
    fn a_window_shrunk_before_its_frame_cuts_the_old_frame_at_its_edge() {
        let placed = Placement::of((300, 200), 2, Some((480, 400)), false);
        assert_eq!(
            placed.frame,
            PixelRect {
                x: 0,
                y: 0,
                width: 300,
                height: 200
            }
        );
    }

    #[test]
    fn no_frame_covers_nothing() {
        let placed = Placement::of((480, 400), 1, None, false);
        assert!(placed.frame.is_empty());
    }

    #[test]
    fn the_notice_runs_along_the_bottom_at_its_height_in_css_pixels() {
        let placed = Placement::of((480, 400), 1, Some((480, 400)), true);
        assert_eq!(
            placed.notice,
            Some(PixelRect {
                x: 0,
                y: 368,
                width: 480,
                height: 32
            })
        );
        let doubled = Placement::of((960, 800), 2, Some((480, 400)), true);
        assert_eq!(
            doubled.notice,
            Some(PixelRect {
                x: 0,
                y: 736,
                width: 960,
                height: 64
            })
        );
        // Over the frame, which stays where and as big as it was painted.
        assert_eq!(doubled.frame.height, 800);
    }

    #[test]
    fn a_window_shorter_than_the_notice_is_all_notice() {
        let placed = Placement::of((100, 20), 1, None, true);
        assert_eq!(
            placed.notice,
            Some(PixelRect {
                x: 0,
                y: 0,
                width: 100,
                height: 20
            })
        );
    }

    #[test]
    fn a_frame_too_large_to_multiply_is_cut_rather_than_overflowing() {
        let placed = Placement::of(
            (100, 100),
            MOST_REPLICATION,
            Some((u32::MAX, u32::MAX)),
            false,
        );
        assert_eq!((placed.frame.width, placed.frame.height), (100, 100));
    }
}
