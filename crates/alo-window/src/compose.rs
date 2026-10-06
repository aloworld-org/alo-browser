/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The window's pixels, as a function of what it was last sent.
//!
//! ADR 0024 § 2: *"Composition is a function: given the window's size, … the
//! selected tab's frame (or its sentence, if it is gone) and the window's
//! colours, it returns the window's pixels. It lives in its own file, takes no
//! window and no thread, and is tested like every other picture here: a
//! reference render."*
//!
//! So it takes no window and asks nothing of anyone. The event loop calls it
//! on every redraw with what it holds, and what it holds is only ever what a
//! renderer already sent — so a renderer that has stopped answering cannot
//! make a redraw wait, because nothing here could wait for it.
//!
//! The tab strip (item 297) is not here yet. When it is, it is a second frame
//! at the top of the window and this is where it is placed.

use crate::colours::background;
use crate::notice::Lettering;
use crate::place::{PixelRect, Placement};
use alo_paint::Canvas;
use alo_renderer::Frame;
use alo_value::Rgba;

/// Everything a window's pixels are made from.
#[derive(Debug, Clone, Copy)]
pub struct Scene<'a> {
    /// The window's size in device pixels.
    pub window: (u32, u32),
    /// How many device pixels each CSS pixel of a frame becomes
    /// ([`crate::place::replication`]).
    pub replication: u32,
    /// The selected tab's last frame, if it ever painted one.
    pub frame: Option<&'a Frame>,
    /// What to tell a person about that tab, if anything.
    pub sentence: Option<&'a str>,
}

/// The window's pixels.
///
/// The background, then the frame replicated into the top-left corner, then
/// the sentence along the bottom. A window of no size is a canvas of no size.
pub fn compose(scene: &Scene<'_>, lettering: &Lettering) -> Canvas {
    let (width, height) = scene.window;
    let mut canvas = Canvas::new(width, height, background());
    let placed = Placement::of(
        scene.window,
        scene.replication,
        scene.frame.map(|frame| (frame.width, frame.height)),
        scene.sentence.is_some(),
    );
    if let Some(frame) = scene.frame {
        replicate(&mut canvas, frame, placed.frame, scene.replication.max(1));
    }
    if let (Some(sentence), Some(rect)) = (scene.sentence, placed.notice) {
        lettering.draw(&mut canvas, rect, sentence, scene.replication.max(1));
    }
    canvas
}

/// Copy `frame` into `rect` of `canvas`, each of its pixels covering a square
/// of `replication` device pixels.
///
/// Blended over the background rather than written, so a frame with a
/// transparent pixel shows the window's colour there instead of black.
fn replicate(canvas: &mut Canvas, frame: &Frame, rect: PixelRect, replication: u32) {
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            let Some((red, green, blue, alpha)) =
                frame.at((x - rect.x) / replication, (y - rect.y) / replication)
            else {
                continue;
            };
            canvas.blend(x, y, Rgba::from_rgba8(red, green, blue, 255), alpha);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lettering() -> Lettering {
        Lettering::compiled_in().expect("DejaVu Sans is compiled in")
    }

    /// A frame of two by one: red, then blue.
    fn two_pixels() -> Frame {
        Frame {
            width: 2,
            height: 1,
            pixels: vec![255, 0, 0, 255, 0, 0, 255, 255],
        }
    }

    fn rgba8(canvas: &Canvas, x: u32, y: u32) -> Option<(u8, u8, u8, u8)> {
        canvas.at(x, y).map(Rgba::to_rgba8)
    }

    #[test]
    fn a_window_with_nothing_sent_is_its_background() {
        let canvas = compose(
            &Scene {
                window: (4, 3),
                replication: 1,
                frame: None,
                sentence: None,
            },
            &lettering(),
        );
        assert_eq!((canvas.width(), canvas.height()), (4, 3));
        assert!(
            canvas
                .pixels()
                .iter()
                .all(|pixel| pixel.to_rgba8() == background().to_rgba8())
        );
    }

    #[test]
    fn each_pixel_of_a_frame_covers_a_square_of_the_replication() {
        let frame = two_pixels();
        let canvas = compose(
            &Scene {
                window: (5, 3),
                replication: 2,
                frame: Some(&frame),
                sentence: None,
            },
            &lettering(),
        );
        for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            assert_eq!(rgba8(&canvas, x, y), Some((255, 0, 0, 255)), "at {x},{y}");
        }
        for (x, y) in [(2, 0), (3, 0), (2, 1), (3, 1)] {
            assert_eq!(rgba8(&canvas, x, y), Some((0, 0, 255, 255)), "at {x},{y}");
        }
        // The odd column and row are background, never half a pixel.
        for (x, y) in [(4, 0), (4, 1), (0, 2), (3, 2)] {
            assert_eq!(
                rgba8(&canvas, x, y),
                Some(background().to_rgba8()),
                "at {x},{y}"
            );
        }
    }

    #[test]
    fn a_transparent_pixel_shows_the_background() {
        let frame = Frame {
            width: 1,
            height: 1,
            pixels: vec![0, 0, 0, 0],
        };
        let canvas = compose(
            &Scene {
                window: (1, 1),
                replication: 1,
                frame: Some(&frame),
                sentence: None,
            },
            &lettering(),
        );
        assert_eq!(rgba8(&canvas, 0, 0), Some(background().to_rgba8()));
    }

    #[test]
    fn a_window_of_no_size_is_a_canvas_of_no_size() {
        let frame = two_pixels();
        let canvas = compose(
            &Scene {
                window: (0, 0),
                replication: 2,
                frame: Some(&frame),
                sentence: Some("gone"),
            },
            &lettering(),
        );
        assert!(canvas.is_empty());
    }
}
