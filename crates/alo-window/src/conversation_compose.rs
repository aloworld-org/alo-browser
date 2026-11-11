/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The conversation's window's pixels, as a function of what it was last
//! sent (ADR 0074, queue item 709).
//!
//! ADR 0024 § 2, as [`crate::compose`] is for the window of tabs: given the
//! window's size and scale, the chats panel's last frame and the first
//! screen's — or what to say of a renderer that painted none — it returns
//! the window's pixels, takes no window and no thread, and is tested by a
//! reference render. Each frame was painted by its own renderer at the
//! window's scale, so each of its pixels is one of the window's.

use crate::colours::{background, notice_line, panel, shade};
use crate::compose::copy;
use crate::conversation_place::{ConversationPlacement, Parts, Side};
use crate::notice::Lettering;
use crate::place::PixelRect;
use crate::scale::Scale;
use alo_paint::Canvas;
use alo_renderer::Frame;
use alo_renderer::beneath::Beneath;

/// Everything the conversation's window is made from.
#[derive(Debug, Clone, Copy)]
pub struct ConversationScene<'a> {
    /// The window's size in device pixels.
    pub window: (u32, u32),
    /// How many device pixels one CSS pixel is in the window: what the
    /// lines along the bottom are drawn at. The frames are already at it.
    pub scale: Scale,
    /// The chats panel's last frame, if its renderer ever painted one.
    pub panel: Option<&'a Frame>,
    /// What to tell a person about the panel's renderer, if anything.
    pub panel_sentence: Option<&'a str>,
    /// The first screen's last frame, if its renderer ever painted one.
    pub start: Option<&'a Frame>,
    /// What to tell a person about the first screen's renderer, if anything.
    pub start_sentence: Option<&'a str>,
    /// Whether the panel is shown whole or hidden as its rail (queue item
    /// 711).
    pub side: Side,
    /// Whether the rail shows a chat's name past its edge, over the first
    /// screen (queue item 760).
    pub named: bool,
    /// The whole panel standing over the conversation as a sheet in a
    /// narrow window, as far out as it is (queue item 776).
    pub sheet: Option<SheetScene<'a>>,
    /// The live card's page, drawn beneath the first screen's frame where
    /// that frame left its place clear, in device pixels of the frame
    /// (queue item 869, ADR 0100).
    pub beneath: Option<(&'a Frame, Beneath)>,
}

/// The sheet, as it is composed at one instant.
#[derive(Debug, Clone, Copy)]
pub struct SheetScene<'a> {
    /// The panel's frame, painted whole.
    pub frame: &'a Frame,
    /// Where its left edge is, in device pixels from the window's: below
    /// zero while it slides ([`crate::conversation_place::sheet_left`]).
    pub left: i32,
    /// How dark the shade under it is: the ink's alpha.
    pub shade: f32,
}

impl ConversationScene<'_> {
    /// The first screen as a person sees it: its frame, with the live
    /// card's page drawn beneath where it left that clear (queue item 869)
    /// — what the window composes in the first screen's room, in that
    /// frame's own pixels. Nothing before its renderer painted one.
    pub fn first_screen_seen(&self) -> Option<Frame> {
        let frame = self.start?;
        let Some((picture, place)) = self.beneath else {
            return Some(frame.clone());
        };
        let mut canvas = Canvas::new(frame.width, frame.height, alo_value::Rgba::TRANSPARENT);
        let whole = PixelRect {
            x: 0,
            y: 0,
            width: frame.width,
            height: frame.height,
        };
        beneath(&mut canvas, (picture, place), frame, whole);
        copy(&mut canvas, frame, whole);
        Some(Frame::from_canvas(&canvas))
    }

    /// What is in it to place: the frames' sizes in device pixels, and
    /// which of them has something to say.
    pub fn parts(&self) -> Parts {
        let size = |frame: &Frame| (frame.width, frame.height);
        Parts {
            panel: self.panel.map(size),
            start: self.start.map(size),
            panel_saying: self.panel_sentence.is_some(),
            start_saying: self.start_sentence.is_some(),
            side: self.side,
            named: self.named,
        }
    }
}

/// The window's pixels.
///
/// The panel's room — or the rail's — in the panel's colour and the rest in
/// the paper behind the conversation, then each frame in its room one to
/// one, the first screen's first, so the rail naming a chat stands over it
/// (queue item 760); then, while the sheet is out (queue item 776), the
/// shade over all of it and the sheet's frame where it has slid to; then
/// the lines along the bottom — the first screen's, and the panel's above
/// it. A window of no size is a canvas of no size.
pub fn compose_conversation(scene: &ConversationScene<'_>, lettering: &Lettering) -> Canvas {
    let (width, height) = scene.window;
    let mut canvas = Canvas::new(width, height, background());
    compose_onto(&mut canvas, scene, lettering);
    canvas
}

/// The window's pixels inside `region` composed again onto `canvas`, the
/// window's size, holding them as they were composed before (queue item
/// 872, [`crate::conversation_composed`]): put back to the paper and
/// composed exactly as [`compose_conversation`] composes them, every pixel
/// outside left alone. Each step is a pixel's own arithmetic onto that
/// pixel, so a pixel inside comes out as composing the whole window makes
/// it.
pub fn compose_within(
    canvas: &mut Canvas,
    scene: &ConversationScene<'_>,
    lettering: &Lettering,
    region: alo_paint::Region,
) {
    canvas.reset(region, background());
    canvas.confine(Some(region));
    compose_onto(canvas, scene, lettering);
    canvas.confine(None);
}

/// Everything [`compose_conversation`] draws over the paper, onto `canvas`.
fn compose_onto(canvas: &mut Canvas, scene: &ConversationScene<'_>, lettering: &Lettering) {
    let (width, height) = scene.window;
    let placed = ConversationPlacement::of(scene.window, scene.scale, scene.parts());
    let room = placed.panel_room;
    canvas.fill_rect(0, 0, room.width, room.height, panel());
    if let Some(frame) = scene.start {
        // The card's page first, where the first screen left it clear, so
        // what the first screen draws over its place stands over it.
        if let Some(beneath_it) = scene.beneath {
            beneath(canvas, beneath_it, frame, placed.start);
        }
        copy(canvas, frame, placed.start);
    }
    if let Some(frame) = scene.panel {
        copy(canvas, frame, placed.panel);
    }
    if let Some(sheet) = &scene.sheet {
        canvas.fill_rect(0, 0, width, height, shade(sheet.shade));
        slid(canvas, sheet.frame, sheet.left);
    }
    for (sentence, rect) in [
        (scene.start_sentence, placed.start_notice),
        (scene.panel_sentence, placed.panel_notice),
    ] {
        if let (Some(sentence), Some(rect)) = (sentence, rect) {
            lettering.draw(canvas, rect, sentence, scene.scale, notice_line());
        }
    }
}

/// The place of the card's `picture` at `place` in the first screen's
/// `frame` standing at `room`, in the window's pixels: what [`beneath`]
/// draws, and nothing outside it.
pub(crate) fn beneath_rect(
    (picture, place): (&Frame, Beneath),
    frame: (u32, u32),
    room: PixelRect,
) -> PixelRect {
    let span = |at: i32, length: u32, size: u32| {
        let from = i64::from(at).clamp(0, i64::from(size));
        let to = (i64::from(at) + i64::from(length)).clamp(0, i64::from(size));
        (
            u32::try_from(from).unwrap_or(0),
            u32::try_from(to - from).unwrap_or(0),
        )
    };
    let (x, width) = span(place.left, picture.width.min(place.width), frame.0);
    let (y, height) = span(place.top, picture.height.min(place.height), frame.1);
    PixelRect {
        x: room.x + x,
        y: room.y + y,
        width,
        height,
    }
}

/// The card's `picture` onto `canvas` one to one at `place` in the first
/// screen's `frame`, standing in its `room` — the place's own corner
/// wherever the frame put it, the part of it outside the frame not drawn
/// (queue item 869).
///
/// Over the colour the frame kept where it left the place clear, made
/// opaque: the colour that was under the picture's place — the card's
/// surface — so a page's translucent pixels show it, as they did over it in
/// the card ([`alo_paint::beneath`] keeps a cleared pixel's colour). The
/// frame then goes over the picture.
fn beneath(
    canvas: &mut Canvas,
    (picture, place): (&Frame, Beneath),
    frame: &Frame,
    room: PixelRect,
) {
    // Only the rows and columns a confined canvas would keep (queue item
    // 872): the others would be refused pixel by pixel all the same.
    let (rows, columns) = kept_steps(
        canvas,
        (
            i64::from(room.x) + i64::from(place.left),
            i64::from(room.y) + i64::from(place.top),
        ),
        (
            picture.width.min(place.width),
            picture.height.min(place.height),
        ),
    );
    for y in rows {
        let Some(down) = within(place.top, y, frame.height) else {
            continue;
        };
        for x in columns.clone() {
            let Some(across) = within(place.left, x, frame.width) else {
                continue;
            };
            let (Some((red, green, blue, alpha)), Some((under_red, under_green, under_blue, _))) =
                (picture.at(x, y), frame.at(across, down))
            else {
                continue;
            };
            let (at_x, at_y) = (room.x + across, room.y + down);
            if alpha == u8::MAX {
                // Opaque, as nearly every pixel of a page is: its own colour,
                // which is what drawing it over anything makes.
                canvas.replace(at_x, at_y, |_| {
                    alo_value::Rgba::from_rgba8(red, green, blue, 255)
                });
                continue;
            }
            canvas.blend(
                at_x,
                at_y,
                alo_value::Rgba::from_rgba8(under_red, under_green, under_blue, 255),
                u8::MAX,
            );
            canvas.blend(
                at_x,
                at_y,
                alo_value::Rgba::from_rgba8(red, green, blue, 255),
                alpha,
            );
        }
    }
}

/// The steps down and across a run of `size` pixels whose first is at
/// `from` in the window that land on what `canvas` is confined to: all of
/// them on a canvas that is not.
pub(crate) fn kept_steps(
    canvas: &Canvas,
    from: (i64, i64),
    size: (u32, u32),
) -> (core::ops::Range<u32>, core::ops::Range<u32>) {
    let Some(region) = canvas.confinement() else {
        return (0..size.1, 0..size.0);
    };
    let clamp = |base: i64, length: u32, at: u32| {
        u32::try_from((i64::from(at) - base).clamp(0, i64::from(length))).unwrap_or(0)
    };
    let range = |base: i64, length: u32, (low, high): (u32, u32)| {
        let first = clamp(base, length, low);
        first..clamp(base, length, high).max(first)
    };
    (
        range(from.1, size.1, (region.top, region.bottom)),
        range(from.0, size.0, (region.left, region.right)),
    )
}

/// `step` on from `at`, if that is inside `0..size`.
fn within(at: i32, step: u32, size: u32) -> Option<u32> {
    let along = i64::from(at) + i64::from(step);
    u32::try_from(along).ok().filter(|along| *along < size)
}

/// `frame` onto `canvas` one to one from `left` device pixels across —
/// below zero while it slides in from the window's left edge, the part of
/// it left of the window not drawn — and from the window's top.
fn slid(canvas: &mut Canvas, frame: &Frame, left: i32) {
    let skipped = u32::try_from(left.saturating_neg()).unwrap_or(0);
    let from = u32::try_from(left).unwrap_or(0);
    let width = frame.width.saturating_sub(skipped);
    for y in 0..frame.height.min(canvas.height()) {
        for x in 0..width {
            let Some((red, green, blue, alpha)) = frame.at(x + skipped, y) else {
                continue;
            };
            canvas.blend(
                from + x,
                y,
                alo_value::Rgba::from_rgba8(red, green, blue, 255),
                alpha,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_value::Rgba;

    fn lettering() -> Lettering {
        Lettering::compiled_in().expect("DejaVu Sans is compiled in")
    }

    fn rgba8(canvas: &Canvas, x: u32, y: u32) -> Option<(u8, u8, u8, u8)> {
        canvas.at(x, y).map(Rgba::to_rgba8)
    }

    /// A frame of `width` by `height` in one colour.
    fn solid(width: u32, height: u32, colour: (u8, u8, u8, u8)) -> Frame {
        let (red, green, blue, alpha) = colour;
        Frame {
            width,
            height,
            pixels: [red, green, blue, alpha].repeat(usize::try_from(width * height).unwrap_or(0)),
        }
    }

    /// A window of `window` at `scale` with nothing sent.
    fn bare(window: (u32, u32), scale: Scale) -> ConversationScene<'static> {
        ConversationScene {
            window,
            scale,
            panel: None,
            panel_sentence: None,
            start: None,
            start_sentence: None,
            side: Side::Panel,
            named: false,
            sheet: None,
            beneath: None,
        }
    }

    #[test]
    fn nothing_sent_is_the_panels_colour_beside_the_paper() {
        let canvas = compose_conversation(&bare((600, 10), Scale::TWO), &lettering());
        assert_eq!((canvas.width(), canvas.height()), (600, 10));
        for y in [0, 9] {
            assert_eq!(rgba8(&canvas, 0, y), Some(panel().to_rgba8()));
            assert_eq!(rgba8(&canvas, 479, y), Some(panel().to_rgba8()));
            assert_eq!(rgba8(&canvas, 480, y), Some(background().to_rgba8()));
            assert_eq!(rgba8(&canvas, 599, y), Some(background().to_rgba8()));
        }
    }

    /// The card's page is drawn under the first screen's frame, one to one
    /// at its place in the frame, showing where the frame is clear and as
    /// much as it is clear, never outside the first screen's room — and
    /// [`ConversationScene::first_screen_seen`] is the room as composed
    /// (queue item 869).
    #[test]
    fn the_cards_page_shows_beneath_the_first_screen_where_it_is_clear_inside_its_room() {
        // Blue, clear at (2, 1)–(3, 2) and half clear at (2, 1).
        let mut start = solid(6, 4, (0, 0, 255, 255));
        for (x, y, alpha) in [(2, 1, 127), (3, 1, 0), (2, 2, 0), (3, 2, 0)] {
            let at = usize::try_from((y * 6 + x) * 4 + 3).unwrap_or(0);
            start.pixels[at] = alpha;
        }
        let green = solid(3, 3, (0, 255, 0, 255));
        let place = Beneath {
            left: 2,
            top: 1,
            width: 3,
            height: 3,
        };
        let scene = ConversationScene {
            start: Some(&start),
            beneath: Some((&green, place)),
            ..bare((246, 4), Scale::ONE)
        };
        let canvas = compose_conversation(&scene, &lettering());
        assert_eq!(rgba8(&canvas, 243, 2), Some((0, 255, 0, 255)), "clear");
        assert_eq!(rgba8(&canvas, 242, 2), Some((0, 255, 0, 255)));
        let (red, half_green, half_blue, alpha) = rgba8(&canvas, 242, 1).unwrap_or_default();
        assert_eq!((red, alpha), (0, 255));
        assert!(half_green.abs_diff(128) <= 1 && half_blue.abs_diff(127) <= 1);
        assert_eq!(rgba8(&canvas, 244, 2), Some((0, 0, 255, 255)), "drawn over");
        assert_eq!(rgba8(&canvas, 243, 3), Some((0, 0, 255, 255)));
        let seen = scene
            .first_screen_seen()
            .unwrap_or_else(|| solid(0, 0, (0, 0, 0, 0)));
        for y in 0..4 {
            for x in 0..6 {
                assert_eq!(
                    seen.at(x, y),
                    rgba8(&canvas, 240 + x, y),
                    "seen as composed at {x},{y}"
                );
            }
        }
        // A clear pixel of the page shows the colour the frame kept where
        // it cleared its place — the card's surface — not the paper.
        let mut clear_page = green.clone();
        // The pixel at (1, 1) of three across: the fifth, its alpha.
        clear_page.pixels[4 * 4 + 3] = 0;
        let translucent = ConversationScene {
            beneath: Some((&clear_page, place)),
            ..scene
        };
        let canvas = compose_conversation(&translucent, &lettering());
        assert_eq!(
            rgba8(&canvas, 243, 2),
            Some((0, 0, 255, 255)),
            "the kept colour"
        );
        assert_eq!(rgba8(&canvas, 242, 2), Some((0, 255, 0, 255)));
        // A place left of the frame's edge is not drawn on the panel.
        let left = ConversationScene {
            beneath: Some((&green, Beneath { left: -2, ..place })),
            ..scene
        };
        let canvas = compose_conversation(&left, &lettering());
        assert_eq!(rgba8(&canvas, 239, 2), Some(panel().to_rgba8()));
        assert_eq!(rgba8(&canvas, 238, 2), Some(panel().to_rgba8()));
    }

    #[test]
    fn each_frame_is_one_to_one_in_its_room() {
        let red = solid(2, 3, (255, 0, 0, 255));
        let blue = solid(3, 2, (0, 0, 255, 255));
        let scene = ConversationScene {
            panel: Some(&red),
            start: Some(&blue),
            ..bare((260, 5), Scale::ONE)
        };
        let canvas = compose_conversation(&scene, &lettering());
        for (x, y) in [(0, 0), (1, 2)] {
            assert_eq!(rgba8(&canvas, x, y), Some((255, 0, 0, 255)), "at {x},{y}");
        }
        // Past the panel's frame, its room's colour.
        assert_eq!(rgba8(&canvas, 2, 0), Some(panel().to_rgba8()));
        assert_eq!(rgba8(&canvas, 0, 3), Some(panel().to_rgba8()));
        for (x, y) in [(240, 0), (242, 1)] {
            assert_eq!(rgba8(&canvas, x, y), Some((0, 0, 255, 255)), "at {x},{y}");
        }
        assert_eq!(rgba8(&canvas, 243, 0), Some(background().to_rgba8()));
        assert_eq!(rgba8(&canvas, 240, 2), Some(background().to_rgba8()));
    }

    #[test]
    fn a_clear_pixel_shows_its_rooms_colour() {
        let clear = solid(1, 1, (0, 0, 0, 0));
        let scene = ConversationScene {
            panel: Some(&clear),
            start: Some(&clear),
            ..bare((241, 1), Scale::ONE)
        };
        let canvas = compose_conversation(&scene, &lettering());
        assert_eq!(rgba8(&canvas, 0, 0), Some(panel().to_rgba8()));
        assert_eq!(rgba8(&canvas, 240, 0), Some(background().to_rgba8()));
    }

    #[test]
    fn a_renderer_that_went_is_said_over_the_frames_along_the_bottom() {
        let white = solid(240, 100, (255, 255, 255, 255));
        let scene = ConversationScene {
            panel: Some(&white),
            panel_sentence: Some("the chats panel could not be drawn"),
            ..bare((400, 100), Scale::ONE)
        };
        let canvas = compose_conversation(&scene, &lettering());
        // The line's rule at its top, and its ground, across the window.
        assert_eq!(rgba8(&canvas, 0, 68), Some(notice_line().rule.to_rgba8()));
        assert_eq!(
            rgba8(&canvas, 399, 99),
            Some(notice_line().ground.to_rgba8())
        );
        // Above it, the panel's frame as it was painted.
        assert_eq!(rgba8(&canvas, 0, 67), Some((255, 255, 255, 255)));
    }

    #[test]
    fn the_rail_naming_a_chat_is_drawn_over_the_first_screen_where_it_draws_something() {
        // A rail frame 4 wide, its first column solid red and the rest
        // clear but for one red pixel at (3, 0): the name past the edge.
        let mut pixels = Vec::new();
        for y in 0..2 {
            for x in 0..4 {
                let solid = x == 0 || (x, y) == (3, 0);
                pixels.extend_from_slice(if solid {
                    &[255, 0, 0, 255]
                } else {
                    &[0, 0, 0, 0]
                });
            }
        }
        let rail = Frame {
            width: 4,
            height: 2,
            pixels,
        };
        let blue = solid(10, 2, (0, 0, 255, 255));
        let scene = ConversationScene {
            panel: Some(&rail),
            start: Some(&blue),
            side: Side::Rail,
            named: true,
            ..bare((74, 2), Scale::ONE)
        };
        let canvas = compose_conversation(&scene, &lettering());
        // The rail's room is 64: past it the first screen, but where the
        // rail's frame draws its name, the name.
        assert_eq!(rgba8(&canvas, 0, 0), Some((255, 0, 0, 255)));
        assert_eq!(rgba8(&canvas, 3, 0), Some((255, 0, 0, 255)));
        assert_eq!(rgba8(&canvas, 64, 0), Some((0, 0, 255, 255)));
        assert_eq!(rgba8(&canvas, 3, 1), Some(panel().to_rgba8()));
    }

    #[test]
    fn the_sheet_stands_where_it_slid_to_over_a_shade() {
        let white = solid(240, 4, (255, 255, 255, 255));
        let paper = solid(10, 4, (0xf4, 0xf1, 0xec, 0xff));
        let scene = ConversationScene {
            start: Some(&paper),
            side: Side::Rail,
            sheet: Some(SheetScene {
                frame: &white,
                left: -200,
                shade: 0.32,
            }),
            beneath: None,
            ..bare((300, 4), Scale::ONE)
        };
        let canvas = compose_conversation(&scene, &lettering());
        // Its 40 right-most columns are in, from 0 to 39.
        assert_eq!(rgba8(&canvas, 0, 0), Some((255, 255, 255, 255)));
        assert_eq!(rgba8(&canvas, 39, 3), Some((255, 255, 255, 255)));
        // Past it, the first screen and the bare paper under the shade:
        // the ink, alo OS's navy since queue item 974, at 0.32 over the
        // porcelain.
        let shaded = |under: u8, ink: u8| {
            let mixed = f32::from(under) * 0.68 + f32::from(ink) * 0.32;
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a mix of two channels, inside 0..=255"
            )]
            let mixed = mixed.round() as u8;
            mixed
        };
        // Over the rail's room, the panel's colour shaded; over the first
        // screen and the paper, the porcelain shaded.
        let rail = Some((
            shaded(0xee, 0x10),
            shaded(0xea, 0x2a),
            shaded(0xe2, 0x43),
            255,
        ));
        let paper = Some((
            shaded(0xf4, 0x10),
            shaded(0xf1, 0x2a),
            shaded(0xec, 0x43),
            255,
        ));
        for (x, expected) in [
            (40, rail),
            (63, rail),
            (64, paper),
            (70, paper),
            (299, paper),
        ] {
            let got = rgba8(&canvas, x, 1);
            let near = got.zip(expected).is_some_and(|(got, expected)| {
                got.0.abs_diff(expected.0) <= 1
                    && got.1.abs_diff(expected.1) <= 1
                    && got.2.abs_diff(expected.2) <= 1
                    && got.3 == 255
            });
            assert!(near, "at {x}: {got:?}, not {expected:?}");
        }
        // All the way out, it covers the window's left 240.
        let scene = ConversationScene {
            sheet: Some(SheetScene {
                frame: &white,
                left: 0,
                shade: 0.32,
            }),
            beneath: None,
            ..scene
        };
        let canvas = compose_conversation(&scene, &lettering());
        assert_eq!(rgba8(&canvas, 239, 2), Some((255, 255, 255, 255)));
        assert_ne!(rgba8(&canvas, 240, 2), Some((255, 255, 255, 255)));
    }

    #[test]
    fn a_window_of_no_size_is_a_canvas_of_no_size() {
        let frame = solid(2, 2, (1, 2, 3, 255));
        let scene = ConversationScene {
            panel: Some(&frame),
            panel_sentence: Some("gone"),
            start: Some(&frame),
            start_sentence: Some("gone"),
            ..bare((0, 0), Scale::TWO)
        };
        assert!(compose_conversation(&scene, &lettering()).is_empty());
    }
}
