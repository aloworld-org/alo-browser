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
//!
//! # Hidden
//!
//! A person may hide the panel (queue item 711): it is then the design's
//! rail, [`RAIL_WIDTH`] wide, and the first screen has the rest — the
//! window laid out without the panel, and a way back on the rail. Which it
//! is is the window's [`Side`].
//!
//! # Over the first screen
//!
//! Two things of the panel's stand over the first screen rather than
//! beside it. The rail, while it shows the name of the chat the pointer is
//! over (queue item 760), is laid out wider and drawn on nothing, and its
//! frame is placed whole from the window's left, over the first screen,
//! rather than cut at its room ([`Parts::named`]). And in a narrow window
//! the whole panel slides over the conversation as a sheet (queue item
//! 776), its left edge where the slide has it ([`sheet_left`]), over a
//! shade.

use crate::place::{NOTICE_HEIGHT, PixelRect};
use crate::scale::Scale;
use alo_layout::Size;

/// How wide the chats panel is, in CSS pixels: the design's 240.
pub const PANEL_WIDTH: u32 = 240;

/// How wide the panel is hidden, as the design's rail, in CSS pixels: 64.
pub const RAIL_WIDTH: u32 = 64;

/// What stands down the window's left: the chats panel, or — hidden by a
/// person — its rail (queue item 711).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    /// The whole panel, [`PANEL_WIDTH`] wide.
    #[default]
    Panel,
    /// The rail, [`RAIL_WIDTH`] wide.
    Rail,
}

impl Side {
    /// How wide it is, in CSS pixels.
    pub fn width(self) -> u32 {
        match self {
            Side::Panel => PANEL_WIDTH,
            Side::Rail => RAIL_WIDTH,
        }
    }
}

/// The size the panel is laid out at in a `window` of device pixels at
/// `scale`, the panel shown whole: see [`panel_viewport_as`].
pub fn panel_viewport(window: (u32, u32), scale: Scale) -> Size {
    panel_viewport_as(window, scale, Side::Panel)
}

/// The size the panel is laid out at in a `window` of device pixels at
/// `scale`, as `side`: its width, or as much of it as the window has, and
/// the most whole CSS pixels that fit the window's height.
pub fn panel_viewport_as(window: (u32, u32), scale: Scale, side: Side) -> Size {
    Size::new(
        css_length(scale.fitting(panel_room(window, scale, side))),
        css_length(scale.fitting(window.1)),
    )
}

/// The size the panel's renderer lays it out at in a `window` of device
/// pixels at `scale`, `width` CSS pixels wide — the panel's, the rail's, or
/// the rail's while it names a chat (queue item 760) — or as much of it as
/// the window has, and the most whole CSS pixels that fit the window's
/// height.
pub fn panel_viewport_of(window: (u32, u32), scale: Scale, width: u32) -> Size {
    Size::new(
        css_length(scale.fitting(scale.device(width).min(window.0))),
        css_length(scale.fitting(window.1)),
    )
}

/// The size the first screen is laid out at, the panel shown whole: see
/// [`start_viewport_as`].
pub fn start_viewport(window: (u32, u32), scale: Scale) -> Size {
    start_viewport_as(window, scale, Side::Panel)
}

/// The size the first screen is laid out at beside `side`: the most whole
/// CSS pixels that fit what it leaves of the window's width, and its
/// height.
pub fn start_viewport_as(window: (u32, u32), scale: Scale, side: Side) -> Size {
    Size::new(
        css_length(scale.fitting(window.0 - panel_room(window, scale, side))),
        css_length(scale.fitting(window.1)),
    )
}

/// How many device pixels across the panel's room is as `side`: its width
/// at the scale, or the whole window when it is narrower.
fn panel_room(window: (u32, u32), scale: Scale, side: Side) -> u32 {
    scale.device(side.width()).min(window.0)
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
    /// Whether that sentence has a button at its end: *Quit and update*,
    /// on the line saying an update is ready (queue item 990).
    pub start_button: Option<crate::line_button::LineButton>,
    /// Whether the panel is shown whole or hidden as its rail.
    pub side: Side,
    /// Whether the rail shows a chat's name past its edge (queue item
    /// 760): its frame is then placed whole, over the first screen.
    pub named: bool,
}

/// Where the panel, the first screen and the lines about them go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConversationPlacement {
    /// The panel's room down the left, [`PANEL_WIDTH`] at the scale — or
    /// [`RAIL_WIDTH`], hidden — and the window's whole height: in the
    /// panel's colour until it has painted.
    pub panel_room: PixelRect,
    /// The part of it the panel's frame covers, from its top-left corner —
    /// past it, over the first screen, while the rail shows a chat's name.
    pub panel: PixelRect,
    /// The first screen's room, everything right of the panel's.
    pub start_room: PixelRect,
    /// The part of it the first screen's frame covers, from its top-left
    /// corner.
    pub start: PixelRect,
    /// The line saying what happened to the first screen's renderer, along
    /// the bottom of the window, when there is something to say.
    pub start_notice: Option<PixelRect>,
    /// The button at that line's right end, when it has one and holds it
    /// whole ([`crate::line_button`], queue item 990): the browser's own,
    /// pressed where the browser drew it.
    pub start_button: Option<PixelRect>,
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
        let room = panel_room(window, scale, parts.side);
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
        let start_button = start_notice
            .filter(|_| parts.start_button.is_some())
            .and_then(|line| crate::line_button::placed(line, scale));
        // The rail with a chat's name beside it reaches over the first
        // screen; anything else of the panel's is cut at its room.
        let reach = if parts.named && parts.side == Side::Rail {
            PixelRect {
                width: window.0,
                ..panel_room
            }
        } else {
            panel_room
        };
        Self {
            panel_room,
            panel: within(reach, parts.panel),
            start_room,
            start: within(start_room, parts.start),
            start_notice,
            start_button,
            panel_notice,
        }
    }
}

/// Where the sheet's left edge is in device pixels at `scale`, its left
/// edge `left` CSS pixels from the window's (queue item 776): rounded to a
/// whole device pixel, so the panel's frame is placed one to one.
pub fn sheet_left(left: f32, scale: Scale) -> i32 {
    let device = (left * scale.factor()).round();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a slide's left edge, from −176 CSS pixels times the scale to 0, rounded"
    )]
    let device = device as i32;
    device
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
    fn the_rail_naming_a_chat_reaches_over_the_first_screen_and_nothing_else_does() {
        let window = (759, 820);
        let named = Parts {
            panel: Some((320, 820)),
            start: Some((695, 820)),
            side: Side::Rail,
            named: true,
            ..Parts::default()
        };
        let placed = ConversationPlacement::of(window, Scale::ONE, named);
        assert_eq!(placed.panel_room, rect(0, 0, 64, 820));
        assert_eq!(placed.panel, rect(0, 0, 320, 820), "over the first screen");
        assert_eq!(placed.start_room, rect(64, 0, 695, 820));
        // Not naming one, the same frame is cut at the rail's room; nor
        // does the whole panel ever reach past its own.
        let placed = ConversationPlacement::of(
            window,
            Scale::ONE,
            Parts {
                named: false,
                ..named
            },
        );
        assert_eq!(placed.panel, rect(0, 0, 64, 820));
        let placed = ConversationPlacement::of(
            (1280, 820),
            Scale::ONE,
            Parts {
                side: Side::Panel,
                panel: Some((320, 820)),
                ..named
            },
        );
        assert_eq!(placed.panel, rect(0, 0, 240, 820));
        // At two, twice the device pixels.
        let placed = ConversationPlacement::of(
            (1518, 1640),
            Scale::TWO,
            Parts {
                panel: Some((640, 1640)),
                ..named
            },
        );
        assert_eq!(placed.panel, rect(0, 0, 640, 1640));
    }

    #[test]
    fn the_sheets_left_edge_is_on_a_whole_device_pixel() {
        assert_eq!(sheet_left(-176.0, Scale::ONE), -176);
        assert_eq!(sheet_left(-74.25, Scale::ONE), -74);
        assert_eq!(sheet_left(-74.25, Scale::TWO), -149);
        assert_eq!(sheet_left(0.0, Scale::TWO), 0);
        assert_eq!(sheet_left(-2.75, Scale::ONE_AND_A_HALF), -4);
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
        assert_eq!(both.start_button, None, "no button asked for");
    }

    #[test]
    fn the_ready_lines_button_stands_at_its_right_end_and_only_on_it() {
        // Queue item 990: the design's window at one, the update's line
        // along the bottom and a renderer's line above it.
        let both = ConversationPlacement::of(
            (1280, 820),
            Scale::ONE,
            Parts {
                panel_saying: true,
                start_saying: true,
                start_button: Some(crate::line_button::LineButton::QuitAndUpdate),
                ..Parts::default()
            },
        );
        assert_eq!(both.start_notice, Some(rect(0, 788, 1280, 32)));
        assert_eq!(both.start_button, Some(rect(1140, 792, 128, 24)));
        assert_eq!(both.panel_notice, Some(rect(0, 756, 1280, 32)));
        // At two, twice as far in and twice the size.
        let at_two = ConversationPlacement::of(
            (2560, 1640),
            Scale::TWO,
            Parts {
                start_saying: true,
                start_button: Some(crate::line_button::LineButton::QuitAndUpdate),
                ..Parts::default()
            },
        );
        assert_eq!(at_two.start_notice, Some(rect(0, 1576, 2560, 64)));
        assert_eq!(at_two.start_button, Some(rect(2280, 1584, 256, 48)));
        // No line, no button, whatever was asked.
        let silent = ConversationPlacement::of(
            (1280, 820),
            Scale::ONE,
            Parts {
                start_button: Some(crate::line_button::LineButton::QuitAndUpdate),
                ..Parts::default()
            },
        );
        assert_eq!((silent.start_notice, silent.start_button), (None, None));
    }

    #[test]
    fn hidden_the_panel_is_the_rail_64_wide_and_the_first_screen_the_rest_at_one_and_two() {
        // Queue item 711: at one, the design's window less the rail.
        let window = (1280, 820);
        assert_eq!(
            panel_viewport_as(window, Scale::ONE, Side::Rail),
            Size::new(64.0, 820.0)
        );
        assert_eq!(
            start_viewport_as(window, Scale::ONE, Side::Rail),
            Size::new(1216.0, 820.0)
        );
        let placed = ConversationPlacement::of(
            window,
            Scale::ONE,
            Parts {
                side: Side::Rail,
                ..painted((64, 820), (1216, 820))
            },
        );
        assert_eq!(placed.panel_room, rect(0, 0, 64, 820));
        assert_eq!(placed.panel, rect(0, 0, 64, 820));
        assert_eq!(placed.start_room, rect(64, 0, 1216, 820));
        assert_eq!(placed.start, rect(64, 0, 1216, 820));
        // At two, twice the device pixels and the same CSS ones.
        let window = (2560, 1640);
        assert_eq!(
            panel_viewport_as(window, Scale::TWO, Side::Rail),
            Size::new(64.0, 820.0)
        );
        assert_eq!(
            start_viewport_as(window, Scale::TWO, Side::Rail),
            Size::new(1216.0, 820.0)
        );
        let placed = ConversationPlacement::of(
            window,
            Scale::TWO,
            Parts {
                side: Side::Rail,
                ..painted((128, 1640), (2432, 1640))
            },
        );
        assert_eq!(placed.panel_room, rect(0, 0, 128, 1640));
        assert_eq!(placed.start_room, rect(128, 0, 2432, 1640));
        assert_eq!(placed.start, rect(128, 0, 2432, 1640));
        // Shown again, the panel's 240 as before.
        assert_eq!(
            start_viewport_as(window, Scale::TWO, Side::Panel),
            start_viewport(window, Scale::TWO)
        );
        assert_eq!(Side::default(), Side::Panel);
        // A window narrower than the rail is all rail.
        assert_eq!(
            start_viewport_as((50, 100), Scale::ONE, Side::Rail),
            Size::new(0.0, 100.0)
        );
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
