/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where the chats panel and the first screen go in the conversation's
//! window, in device pixels (ADR 0074, queue item 709).
//!
//! Geometry and nothing else, as [`crate::place`] is for the window of tabs,
//! so that it is asserted in numbers and
//! [`crate::conversation_compose`] only fills the rectangles this hands it.
//!
//! The design (`docs/design/shell.md`, *The window*) puts the panel down the
//! left, [`PANEL_WIDTH`] CSS pixels wide and the window's full height, and
//! the conversation in the rest. Each part's renderer is told its own room
//! at the window's [`Scale`] and paints a frame in device pixels, which is
//! placed one to one from its room's top-left corner — never stretched, and
//! cut at its room's edges when it is larger, as it is after a window
//! shrinks and before the new frame arrives (ADR 0024 § 2). A window
//! narrower than the panel is all panel.

use crate::place::{NOTICE_HEIGHT, PixelRect};
use crate::scale::Scale;
use alo_layout::Size;

/// How wide the chats panel is, in CSS pixels: the design's 240.
pub const PANEL_WIDTH: u32 = 240;

/// The size the panel is laid out at in a `window` of device pixels at
/// `scale`: [`PANEL_WIDTH`], or as much of it as the window has, and the
/// most whole CSS pixels that fit the window's height.
pub fn panel_viewport(window: (u32, u32), scale: Scale) -> Size {
    Size::new(
        css_length(scale.fitting(panel_room(window, scale))),
        css_length(scale.fitting(window.1)),
    )
}

/// The size the first screen is laid out at: the most whole CSS pixels
/// that fit what the panel leaves of the window's width, and its height.
pub fn start_viewport(window: (u32, u32), scale: Scale) -> Size {
    Size::new(
        css_length(scale.fitting(window.0 - panel_room(window, scale))),
        css_length(scale.fitting(window.1)),
    )
}

/// How many device pixels across the panel's room is: [`PANEL_WIDTH`] at
/// the scale, or the whole window when it is narrower.
fn panel_room(window: (u32, u32), scale: Scale) -> u32 {
    scale.device(PANEL_WIDTH).min(window.0)
}

/// A whole number of CSS pixels as a length.
#[expect(
    clippy::cast_precision_loss,
    reason = "at most `LARGEST_WINDOW`, which an f32 holds exactly"
)]
fn css_length(pixels: u32) -> f32 {
    pixels as f32
}

/// What there is to place: the sizes of the frames the two renderers last
/// sent, in device pixels, and which of them has something to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Parts {
    /// The panel's last frame, if it has painted one.
    pub panel: Option<(u32, u32)>,
    /// The first screen's last frame, if it has painted one.
    pub start: Option<(u32, u32)>,
    /// Whether there is a sentence about the panel.
    pub panel_saying: bool,
    /// Whether there is a sentence about the first screen.
    pub start_saying: bool,
}

/// Where the panel, the first screen and the lines about them go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConversationPlacement {
    /// The panel's room down the left, [`PANEL_WIDTH`] at the scale and the
    /// window's whole height: in the panel's colour until it has painted.
    pub panel_room: PixelRect,
    /// The part of it the panel's frame covers, from its top-left corner.
    pub panel: PixelRect,
    /// The first screen's room, everything right of the panel's.
    pub start_room: PixelRect,
    /// The part of it the first screen's frame covers, from its top-left
    /// corner.
    pub start: PixelRect,
    /// The line saying what happened to the first screen's renderer, along
    /// the bottom of the window, when there is something to say.
    pub start_notice: Option<PixelRect>,
    /// The line saying what happened to the panel's renderer, above the
    /// first screen's — or along the bottom when that has nothing to say.
    pub panel_notice: Option<PixelRect>,
}

impl ConversationPlacement {
    /// Where things go in a `window` of device pixels at `scale`, for the
    /// `parts` it holds.
    ///
    /// The lines are [`NOTICE_HEIGHT`] at the scale, across the whole
    /// window, over the frames rather than pushing them — a frame is a
    /// picture at a size, and moving it would show it at one it was not.
    pub fn of(window: (u32, u32), scale: Scale, parts: Parts) -> Self {
        let room = panel_room(window, scale);
        let panel_room = PixelRect {
            x: 0,
            y: 0,
            width: room,
            height: window.1,
        };
        let start_room = PixelRect {
            x: room,
            y: 0,
            width: window.0 - room,
            height: window.1,
        };
        let line = scale.device(NOTICE_HEIGHT);
        let mut bottom = window.1;
        let mut stack = |saying: bool| {
            saying.then(|| {
                let height = line.min(bottom);
                bottom -= height;
                PixelRect {
                    x: 0,
                    y: bottom,
                    width: window.0,
                    height,
                }
            })
        };
        let start_notice = stack(parts.start_saying);
        let panel_notice = stack(parts.panel_saying);
        Self {
            panel_room,
            panel: within(panel_room, parts.panel),
            start_room,
            start: within(start_room, parts.start),
            start_notice,
            panel_notice,
        }
    }
}

/// The part of `room` a frame of `size` covers from its top-left corner,
/// cut at the room's edges; nothing when there is no frame.
fn within(room: PixelRect, size: Option<(u32, u32)>) -> PixelRect {
    let (width, height) = size.unwrap_or((0, 0));
    PixelRect {
        x: room.x,
        y: room.y,
        width: width.min(room.width),
        height: height.min(room.height),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::MOST_SCALE;

    fn rect(x: u32, y: u32, width: u32, height: u32) -> PixelRect {
        PixelRect {
            x,
            y,
            width,
            height,
        }
    }

    /// Both parts painted at these sizes, in device pixels.
    fn painted(panel: (u32, u32), start: (u32, u32)) -> Parts {
        Parts {
            panel: Some(panel),
            start: Some(start),
            ..Parts::default()
        }
    }

    #[test]
    fn at_one_the_panel_is_240_down_the_left_and_the_first_screen_the_rest() {
        let window = (1280, 820);
        assert_eq!(panel_viewport(window, Scale::ONE), Size::new(240.0, 820.0));
        assert_eq!(start_viewport(window, Scale::ONE), Size::new(1040.0, 820.0));
        let placed =
            ConversationPlacement::of(window, Scale::ONE, painted((240, 820), (1040, 820)));
        assert_eq!(placed.panel_room, rect(0, 0, 240, 820));
        assert_eq!(placed.panel, rect(0, 0, 240, 820));
        assert_eq!(placed.start_room, rect(240, 0, 1040, 820));
        assert_eq!(placed.start, rect(240, 0, 1040, 820));
        assert_eq!((placed.panel_notice, placed.start_notice), (None, None));
    }

    #[test]
    fn at_two_the_same_window_is_twice_the_device_pixels_and_the_same_css_ones() {
        let window = (2560, 1640);
        assert_eq!(panel_viewport(window, Scale::TWO), Size::new(240.0, 820.0));
        assert_eq!(start_viewport(window, Scale::TWO), Size::new(1040.0, 820.0));
        let placed =
            ConversationPlacement::of(window, Scale::TWO, painted((480, 1640), (2080, 1640)));
        assert_eq!(placed.panel_room, rect(0, 0, 480, 1640));
        assert_eq!(placed.panel, rect(0, 0, 480, 1640));
        assert_eq!(placed.start_room, rect(480, 0, 2080, 1640));
        assert_eq!(placed.start, rect(480, 0, 2080, 1640));
    }

    #[test]
    fn at_one_and_a_half_the_panel_is_360_device_pixels() {
        let window = (1920, 1230);
        let scale = Scale::ONE_AND_A_HALF;
        assert_eq!(panel_viewport(window, scale), Size::new(240.0, 820.0));
        assert_eq!(start_viewport(window, scale), Size::new(1040.0, 820.0));
        let placed = ConversationPlacement::of(window, scale, painted((360, 1230), (1560, 1230)));
        assert_eq!(placed.panel_room, rect(0, 0, 360, 1230));
        assert_eq!(placed.start_room, rect(360, 0, 1560, 1230));
        assert_eq!(placed.start, rect(360, 0, 1560, 1230));
        // An odd device pixel is the room's, never half a CSS pixel.
        assert_eq!(
            start_viewport((1921, 1231), scale),
            Size::new(1040.0, 820.0)
        );
        assert_eq!(
            start_viewport((1919, 1229), scale),
            Size::new(1039.0, 819.0)
        );
    }

    #[test]
    fn nothing_painted_is_two_empty_rooms() {
        let placed = ConversationPlacement::of((1280, 820), Scale::ONE, Parts::default());
        assert_eq!(placed.panel_room, rect(0, 0, 240, 820));
        assert!(placed.panel.is_empty());
        assert!(placed.start.is_empty());
        assert_eq!(placed.start.x, 240);
    }

    #[test]
    fn a_window_narrower_than_the_panel_is_all_panel() {
        let window = (300, 400);
        assert_eq!(panel_viewport(window, Scale::TWO), Size::new(150.0, 200.0));
        assert_eq!(start_viewport(window, Scale::TWO), Size::new(0.0, 200.0));
        let placed = ConversationPlacement::of(
            window,
            Scale::TWO,
            Parts {
                panel: Some((480, 1640)),
                start: Some((2080, 1640)),
                ..Parts::default()
            },
        );
        assert_eq!(placed.panel_room, rect(0, 0, 300, 400));
        assert_eq!(placed.panel, rect(0, 0, 300, 400), "cut at its room");
        assert_eq!(placed.start_room, rect(300, 0, 0, 400));
        assert!(placed.start.is_empty());
    }

    #[test]
    fn a_window_grown_before_its_frames_shows_them_at_their_old_size() {
        let placed = ConversationPlacement::of(
            (1400, 900),
            Scale::ONE,
            Parts {
                panel: Some((240, 820)),
                start: Some((1040, 820)),
                ..Parts::default()
            },
        );
        assert_eq!(placed.panel, rect(0, 0, 240, 820));
        assert_eq!(placed.start_room, rect(240, 0, 1160, 900));
        assert_eq!(placed.start, rect(240, 0, 1040, 820));
    }

    #[test]
    fn the_lines_run_along_the_bottom_the_first_screens_lowest() {
        let both = ConversationPlacement::of(
            (1280, 820),
            Scale::ONE,
            Parts {
                panel_saying: true,
                start_saying: true,
                ..Parts::default()
            },
        );
        assert_eq!(both.start_notice, Some(rect(0, 788, 1280, 32)));
        assert_eq!(both.panel_notice, Some(rect(0, 756, 1280, 32)));
        let alone = ConversationPlacement::of(
            (2560, 1640),
            Scale::TWO,
            Parts {
                panel_saying: true,
                ..Parts::default()
            },
        );
        assert_eq!(alone.start_notice, None);
        assert_eq!(alone.panel_notice, Some(rect(0, 1576, 2560, 64)));
        // A window shorter than its lines gives the upper what is left.
        let short = ConversationPlacement::of(
            (100, 40),
            Scale::ONE,
            Parts {
                panel_saying: true,
                start_saying: true,
                ..Parts::default()
            },
        );
        assert_eq!(short.start_notice, Some(rect(0, 8, 100, 32)));
        assert_eq!(short.panel_notice, Some(rect(0, 0, 100, 8)));
    }

    #[test]
    fn a_frame_larger_than_any_window_is_cut_at_its_rooms_edge() {
        let placed = ConversationPlacement::of(
            (4000, 400),
            Scale::whole(MOST_SCALE),
            Parts {
                panel: Some((u32::MAX, u32::MAX)),
                start: Some((u32::MAX, u32::MAX)),
                ..Parts::default()
            },
        );
        assert_eq!(placed.panel, rect(0, 0, 1920, 400));
        assert_eq!(placed.start, rect(1920, 0, 2080, 400));
    }
}
