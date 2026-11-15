/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The frame around Chromium's window, as a function of what it was last
//! told (ADR 0062, queue items 642, 679 and 991).
//!
//! Composed as this engine's window is ([`crate::compose`]): from what it
//! was told and nothing else, taking no window and no thread. Its look is
//! alo OS's window band, light only — white, a one-pixel line `#CBD5E1`
//! along its foot — with macOS's own window buttons left on Chromium's
//! window and alo OS's three controls not drawn, since on alo OS the system
//! draws them. In it, where [`FramePlacement`] puts them:
//!
//! - the page's title and the application's name, as the frame's strip's
//!   renderer painted them ([`crate::frame_strip`]), one of its pixels to
//!   each device pixel — never shaped here, because a title is a stranger's
//!   string (ADR 0024 § 4) — or, when that renderer has said why it painted
//!   nothing, that sentence in their place;
//! - what agents' verbs were answered with, in the colours of a verb that
//!   ran and of a refusal, as this engine's window draws them;
//! - which renderer draws the page, in plain words
//!   ([`crate::pages::RenderedBy::plainly`]), in alo OS's muted grey. The
//!   exact wording — `Chrome/156…, the Chromium alo carries, driven by this
//!   browser` — stays what agents, the socket and the record read; only
//!   the drawn words are plain.
//!
//! The browser's own words are drawn in its own compiled-in face
//! ([`Lettering`]), as every line of this engine's window is.

use crate::agent_lines::AgentLine;
use crate::colours::{band_ground, band_line, notice_line, renderer_line};
use crate::compose::{copy, draw_agent_lines};
use crate::frame_place::FramePlacement;
use crate::notice::Lettering;
use crate::scale::Scale;
use alo_paint::Canvas;
use alo_renderer::Frame;

/// Everything the frame of ours around Chromium's window is drawn from.
#[derive(Debug, Clone, Copy)]
pub struct FrameScene<'a> {
    /// The frame's size in device pixels.
    pub window: (u32, u32),
    /// How many device pixels one CSS pixel is on its screen.
    pub scale: Scale,
    /// The page's title and the application's name, as the frame's strip's
    /// renderer last painted them (queue item 991), if it ever did.
    pub band: Option<&'a Frame>,
    /// Why the strip's renderer painted nothing, if it said so since it
    /// last painted: shown in the title's place.
    pub band_sentence: Option<&'a str>,
    /// Which renderer draws the page, in plain words
    /// ([`crate::pages::RenderedBy::plainly`]).
    pub renderer: &'a str,
    /// What agents' verbs were answered with, oldest first, as long as the
    /// frame keeps them ([`crate::agent_lines::AgentLines`]).
    pub agent_lines: &'a [AgentLine],
}

impl FrameScene<'_> {
    /// Where its pieces go, given the face its words are drawn in.
    pub fn placed(&self, lettering: &Lettering) -> FramePlacement {
        FramePlacement::of(
            self.window,
            self.scale,
            lettering.line_width(self.renderer, self.scale),
            self.agent_lines.len(),
        )
    }
}

/// The frame's pixels: the band's white edge to edge, the title and name as
/// their renderer painted them, what agents did and which renderer draws
/// the page, and the line along the foot over all of it. A frame of no size
/// is a canvas of no size.
pub fn compose_frame(scene: &FrameScene<'_>, lettering: &Lettering) -> Canvas {
    let (width, height) = scene.window;
    let mut canvas = Canvas::new(width, height, band_ground());
    let scale = scene.scale;
    let placed = scene.placed(lettering);
    match (scene.band_sentence, scene.band) {
        (Some(sentence), _) => {
            lettering.draw(&mut canvas, placed.band, sentence, scale, notice_line());
        }
        (None, Some(frame)) => copy(&mut canvas, frame, placed.band),
        (None, None) => {}
    }
    draw_agent_lines(
        &mut canvas,
        lettering,
        scene.agent_lines,
        placed.agent_lines,
        scale,
    );
    lettering.draw(
        &mut canvas,
        placed.renderer,
        scene.renderer,
        scale,
        renderer_line(),
    );
    let line = placed.line;
    if !line.is_empty() {
        canvas.fill_rect(
            i32::try_from(line.x).unwrap_or(i32::MAX),
            i32::try_from(line.y).unwrap_or(i32::MAX),
            line.width,
            line.height,
            band_line(),
        );
    }
    canvas
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colours::{agent_line, refused_line, renderer_ink};
    use crate::place::PixelRect;
    use alo_value::Rgba;

    fn lettering() -> Lettering {
        Lettering::compiled_in().expect("DejaVu Sans is compiled in")
    }

    fn rgba8(canvas: &Canvas, x: u32, y: u32) -> Option<(u8, u8, u8, u8)> {
        canvas.at(x, y).map(Rgba::to_rgba8)
    }

    const WHITE: Option<(u8, u8, u8, u8)> = Some((0xff, 0xff, 0xff, 0xff));
    const LINE: Option<(u8, u8, u8, u8)> = Some((0xcb, 0xd5, 0xe1, 0xff));

    /// A frame of `window` saying Chromium draws the page, and `lines`.
    fn framed(window: (u32, u32), scale: Scale, lines: &[AgentLine]) -> FrameScene<'_> {
        FrameScene {
            window,
            scale,
            band: None,
            band_sentence: None,
            renderer: "Page drawn by Chromium 156, carried inside alo",
            agent_lines: lines,
        }
    }

    #[test]
    fn a_frame_with_nothing_painted_is_white_above_its_line() {
        let canvas = compose_frame(&framed((1200, 54), Scale::ONE, &[]), &lettering());
        for y in 0..53 {
            assert_eq!(rgba8(&canvas, 0, y), WHITE, "at y {y}");
        }
        for x in [0, 600, 1199] {
            assert_eq!(rgba8(&canvas, x, 53), LINE, "the foot at {x}");
        }
        // At two the line is two device pixels.
        let canvas = compose_frame(&framed((2400, 108), Scale::TWO, &[]), &lettering());
        assert_eq!(rgba8(&canvas, 0, 105), WHITE);
        assert_eq!(rgba8(&canvas, 0, 106), LINE);
        assert_eq!(rgba8(&canvas, 2399, 107), LINE);
        let nothing = compose_frame(&framed((0, 0), Scale::TWO, &[]), &lettering());
        assert_eq!((nothing.width(), nothing.height()), (0, 0));
    }

    #[test]
    fn the_band_is_copied_one_to_one_at_the_left_and_the_line_over_it() {
        let lettering = lettering();
        // A painted band of four by 54 device pixels, one of them clear.
        let mut painted = Canvas::new(4, 54, Rgba::from_rgba8(0, 0, 0, 0));
        painted.fill_rect(0, 0, 3, 54, Rgba::from_rgba8(10, 20, 30, 255));
        painted.fill_rect(3, 1, 1, 53, Rgba::from_rgba8(10, 20, 30, 255));
        let band = Frame::from_canvas(&painted);
        let scene = FrameScene {
            band: Some(&band),
            ..framed((1200, 54), Scale::ONE, &[])
        };
        let canvas = compose_frame(&scene, &lettering);
        for (x, y) in [(0, 0), (2, 0), (0, 52), (3, 1)] {
            assert_eq!(rgba8(&canvas, x, y), Some((10, 20, 30, 255)), "at {x},{y}");
        }
        // Its clear pixel shows the band's white, and the foot's line is
        // drawn over it.
        assert_eq!(rgba8(&canvas, 3, 0), WHITE);
        assert_eq!(rgba8(&canvas, 0, 53), LINE);
        assert_eq!(rgba8(&canvas, 4, 20), WHITE, "past the painted band");
    }

    #[test]
    fn the_renderers_words_stand_against_the_right_edge_in_the_muted_ink() {
        let lettering = lettering();
        let scene = framed((1200, 54), Scale::ONE, &[]);
        let canvas = compose_frame(&scene, &lettering);
        let placed = scene.placed(&lettering);
        let needs = lettering.line_width(scene.renderer, Scale::ONE);
        assert_eq!(
            placed.renderer,
            PixelRect {
                x: 1200 - needs,
                y: 0,
                width: needs,
                height: 53
            }
        );
        let ink = Some(renderer_ink().to_rgba8());
        let inked =
            |x0: u32, x1: u32| (0..53).any(|y| (x0..x1).any(|x| rgba8(&canvas, x, y) == ink));
        assert!(inked(placed.renderer.x, 1200), "its words are in it");
        assert!(!inked(0, placed.renderer.x), "and only in it");
    }

    #[test]
    fn a_strip_that_said_why_it_painted_nothing_says_it_in_the_titles_place() {
        let lettering = lettering();
        let band = Frame::from_canvas(&Canvas::new(4, 54, Rgba::from_rgba8(10, 20, 30, 255)));
        let scene = FrameScene {
            band: Some(&band),
            band_sentence: Some("the tab strip could not be drawn"),
            ..framed((1200, 54), Scale::ONE, &[])
        };
        let canvas = compose_frame(&scene, &lettering);
        let placed = scene.placed(&lettering);
        assert_eq!(rgba8(&canvas, 0, 0), Some(notice_line().rule.to_rgba8()));
        assert_eq!(rgba8(&canvas, 0, 1), Some(notice_line().ground.to_rgba8()));
        assert_eq!(
            rgba8(&canvas, placed.band.width - 1, 40),
            Some(notice_line().ground.to_rgba8())
        );
    }

    #[test]
    fn the_frames_lines_are_drawn_where_they_are_placed_in_their_colours() {
        let lettering = lettering();
        let lines = [
            AgentLine {
                sentence: "nothing on the page is \"A pony\"".to_owned(),
                refused: true,
            },
            AgentLine {
                sentence: "activated box#6".to_owned(),
                refused: false,
            },
        ];
        let scene = framed((1200, 54), Scale::ONE, &lines);
        let canvas = compose_frame(&scene, &lettering);
        let placed = scene.placed(&lettering);
        let [Some(refused), Some(ran), None] = placed.agent_lines else {
            panic!("two lines are placed: {placed:?}");
        };
        // The ground of each, under its rule, at its right end.
        let ground = |rect: PixelRect| rgba8(&canvas, rect.x + rect.width - 1, rect.height - 2);
        assert_eq!(ground(refused), Some(refused_line().ground.to_rgba8()));
        assert_eq!(ground(ran), Some(agent_line().ground.to_rgba8()));
        // The gap before each, and after the last, is the band's white.
        for x in [refused.x - 1, ran.x - 1, ran.x + ran.width] {
            assert_eq!(rgba8(&canvas, x, 20), WHITE, "at {x}");
        }
        assert_eq!(ran.x + ran.width + 1, placed.renderer.x);
    }
}
