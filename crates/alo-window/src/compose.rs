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
//! The tab strip is a renderer's frame of its own (item 297), placed by
//! item 528 in a band across the top of the window, with the selected tab's
//! frame below it. A strip whose renderer has gone keeps its last frame there
//! and says so in a line of its own, as a tab does.

use crate::colours::{background, rail};
use crate::notice::Lettering;
use crate::place::{Contents, PixelRect, Placement};
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
    /// The tab strip's last frame, if it ever painted one.
    pub strip: Option<&'a Frame>,
    /// What to tell a person about the strip, if anything.
    pub strip_sentence: Option<&'a str>,
    /// The selected tab's last frame, if it ever painted one.
    pub frame: Option<&'a Frame>,
    /// What to tell a person about that tab, if anything.
    pub sentence: Option<&'a str>,
}

impl Scene<'_> {
    /// What is in it to place: the frames' sizes in CSS pixels, and which
    /// of them has something to say.
    pub fn contents(&self) -> Contents {
        let size = |frame: &Frame| (frame.width, frame.height);
        Contents {
            strip: self.strip.map(size),
            frame: self.frame.map(size),
            tab_saying: self.sentence.is_some(),
            strip_saying: self.strip_sentence.is_some(),
        }
    }
}

/// The window's pixels.
///
/// The background, then the band across the top in the rail's colour and
/// the strip replicated into it, then the tab's frame replicated below the
/// band, then the lines along the bottom — the tab's, and the strip's above
/// it. A window of no size is a canvas of no size.
pub fn compose(scene: &Scene<'_>, lettering: &Lettering) -> Canvas {
    let (width, height) = scene.window;
    let mut canvas = Canvas::new(width, height, background());
    let replication = scene.replication.max(1);
    let placed = Placement::of(scene.window, replication, scene.contents());
    let band = placed.band;
    canvas.fill_rect(0, 0, band.width, band.height, rail());
    if let Some(strip) = scene.strip {
        replicate(&mut canvas, strip, placed.strip, replication);
    }
    if let Some(frame) = scene.frame {
        replicate(&mut canvas, frame, placed.frame, replication);
    }
    for (sentence, rect) in [
        (scene.sentence, placed.notice),
        (scene.strip_sentence, placed.strip_notice),
    ] {
        if let (Some(sentence), Some(rect)) = (sentence, rect) {
            lettering.draw(&mut canvas, rect, sentence, replication);
        }
    }
    canvas
}

/// Copy `frame` into `rect` of `canvas`, each of its pixels covering a square
/// of `replication` device pixels.
///
/// Blended over what is there rather than written, so a frame with a
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
    use crate::place::STRIP_HEIGHT;

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

    /// A window of `window` with a tab's `frame` and nothing else.
    fn tab_only(window: (u32, u32), replication: u32, frame: Option<&Frame>) -> Scene<'_> {
        Scene {
            window,
            replication,
            strip: None,
            strip_sentence: None,
            frame,
            sentence: None,
        }
    }

    #[test]
    fn a_window_with_nothing_sent_is_the_rail_above_its_background() {
        let canvas = compose(&tab_only((4, 40), 1, None), &lettering());
        assert_eq!((canvas.width(), canvas.height()), (4, 40));
        for y in 0..40 {
            let expected = if y < STRIP_HEIGHT {
                rail()
            } else {
                background()
            };
            assert_eq!(rgba8(&canvas, 3, y), Some(expected.to_rgba8()), "at y {y}");
        }
    }

    #[test]
    fn each_pixel_of_a_frame_covers_a_square_of_the_replication_below_the_band() {
        let frame = two_pixels();
        let top = STRIP_HEIGHT * 2;
        let canvas = compose(&tab_only((5, top + 3), 2, Some(&frame)), &lettering());
        for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            assert_eq!(
                rgba8(&canvas, x, top + y),
                Some((255, 0, 0, 255)),
                "at {x},{y}"
            );
        }
        for (x, y) in [(2, 0), (3, 0), (2, 1), (3, 1)] {
            assert_eq!(
                rgba8(&canvas, x, top + y),
                Some((0, 0, 255, 255)),
                "at {x},{y}"
            );
        }
        // The odd column and row are background, never half a pixel.
        for (x, y) in [(4, 0), (4, 1), (0, 2), (3, 2)] {
            assert_eq!(
                rgba8(&canvas, x, top + y),
                Some(background().to_rgba8()),
                "at {x},{y}"
            );
        }
    }

    #[test]
    fn the_strip_is_drawn_in_the_band_and_the_tab_below_it() {
        let strip = Frame {
            width: 1,
            height: 1,
            pixels: vec![0, 255, 0, 255],
        };
        let frame = two_pixels();
        let canvas = compose(
            &Scene {
                strip: Some(&strip),
                ..tab_only((3, STRIP_HEIGHT + 2), 1, Some(&frame))
            },
            &lettering(),
        );
        assert_eq!(rgba8(&canvas, 0, 0), Some((0, 255, 0, 255)));
        // Where the strip's frame does not reach is the rail.
        assert_eq!(rgba8(&canvas, 1, 0), Some(rail().to_rgba8()));
        assert_eq!(rgba8(&canvas, 0, 1), Some(rail().to_rgba8()));
        assert_eq!(rgba8(&canvas, 0, STRIP_HEIGHT), Some((255, 0, 0, 255)));
        assert_eq!(rgba8(&canvas, 1, STRIP_HEIGHT), Some((0, 0, 255, 255)));
    }

    #[test]
    fn a_transparent_pixel_shows_what_is_behind_it() {
        let clear = Frame {
            width: 1,
            height: 1,
            pixels: vec![0, 0, 0, 0],
        };
        let canvas = compose(
            &Scene {
                strip: Some(&clear),
                ..tab_only((1, STRIP_HEIGHT + 1), 1, Some(&clear))
            },
            &lettering(),
        );
        assert_eq!(rgba8(&canvas, 0, 0), Some(rail().to_rgba8()));
        assert_eq!(
            rgba8(&canvas, 0, STRIP_HEIGHT),
            Some(background().to_rgba8())
        );
    }

    #[test]
    fn a_window_of_no_size_is_a_canvas_of_no_size() {
        let frame = two_pixels();
        let canvas = compose(
            &Scene {
                window: (0, 0),
                replication: 2,
                strip: Some(&frame),
                strip_sentence: Some("gone"),
                frame: Some(&frame),
                sentence: Some("gone"),
            },
            &lettering(),
        );
        assert!(canvas.is_empty());
    }
}
