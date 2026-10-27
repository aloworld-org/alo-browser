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
//!
//! Both frames are painted by their renderers at the window's scale, in
//! device pixels (ADR 0057 § 3, queue item 562), so each of their pixels is
//! one of the window's: nothing is replicated or stretched here.
//!
//! The frame of ours around Chrome's window ([`compose_frame`], queue items
//! 679 and 681) is composed the same way, from what it was told and nothing
//! else: the page's tab as its strip's renderer painted it, which renderer
//! draws the page, and what agents' verbs were answered with.

use crate::agent_lines::AgentLine;
use crate::colours::{agent_line, background, notice_line, rail, refused_line, renderer_line};
use crate::frame_place::FramePlacement;
use crate::mark::{self, Marked};
use crate::notice::Lettering;
use crate::place::{Contents, PixelRect, Placement};
use crate::scale::Scale;
use alo_paint::Canvas;
use alo_renderer::Frame;
use alo_value::Rgba;

/// Everything a window's pixels are made from.
#[derive(Debug, Clone, Copy)]
pub struct Scene<'a> {
    /// The window's size in device pixels.
    pub window: (u32, u32),
    /// How many device pixels one CSS pixel is in the window
    /// ([`Scale::of_screen`]): what the strip's band and the lines along
    /// the bottom are drawn at. The frames are already at it.
    pub scale: Scale,
    /// The tab strip's last frame, if it ever painted one.
    pub strip: Option<&'a Frame>,
    /// What to tell a person about the strip, if anything.
    pub strip_sentence: Option<&'a str>,
    /// The selected tab's last frame, if it ever painted one.
    pub frame: Option<&'a Frame>,
    /// What to tell a person about that tab, if anything.
    pub sentence: Option<&'a str>,
    /// What agents' verbs were answered with, oldest first, as long as the
    /// window keeps them: a verb that ran (queue item 623) or one that was
    /// refused (625).
    pub agent_lines: &'a [AgentLine],
    /// The rectangles of the box an agent's verb acted on, over `frame` —
    /// ringed on it (queue item 624).
    pub marked: &'a [Marked],
}

impl Scene<'_> {
    /// What is in it to place: the frames' sizes in device pixels, and
    /// which of them has something to say.
    pub fn contents(&self) -> Contents {
        let size = |frame: &Frame| (frame.width, frame.height);
        Contents {
            strip: self.strip.map(size),
            frame: self.frame.map(size),
            tab_saying: self.sentence.is_some(),
            strip_saying: self.strip_sentence.is_some(),
            agent_lines: self.agent_lines.len(),
        }
    }
}

/// The window's pixels.
///
/// The background, then the band across the top in the rail's colour and
/// the strip in it, then the tab's frame below the band and the ring around
/// the box an agent acted on in it, then the lines along the bottom — the
/// tab's, the strip's above it, and what agents did above both, a refusal
/// in colours of its own. A window of no size is a canvas of no size.
pub fn compose(scene: &Scene<'_>, lettering: &Lettering) -> Canvas {
    let (width, height) = scene.window;
    let mut canvas = Canvas::new(width, height, background());
    let scale = scene.scale;
    let placed = Placement::of(scene.window, scale, scene.contents());
    let band = placed.band;
    canvas.fill_rect(0, 0, band.width, band.height, rail());
    if let Some(strip) = scene.strip {
        copy(&mut canvas, strip, placed.strip);
    }
    if let Some(frame) = scene.frame {
        copy(&mut canvas, frame, placed.frame);
        mark::draw(&mut canvas, scene.marked, placed.frame, scale);
    }
    for (sentence, rect, colours) in [
        (scene.sentence, placed.notice, notice_line()),
        (scene.strip_sentence, placed.strip_notice, notice_line()),
    ] {
        if let (Some(sentence), Some(rect)) = (sentence, rect) {
            lettering.draw(&mut canvas, rect, sentence, scale, colours);
        }
    }
    draw_agent_lines(
        &mut canvas,
        lettering,
        scene.agent_lines,
        placed.agent_lines,
        scale,
    );
    canvas
}

/// Everything the frame of ours around Chrome's window is drawn from (ADR
/// 0062, queue items 642 and 679).
#[derive(Debug, Clone, Copy)]
pub struct FrameScene<'a> {
    /// The frame's size in device pixels.
    pub window: (u32, u32),
    /// How many device pixels one CSS pixel is on its screen.
    pub scale: Scale,
    /// The page's tab, as the frame's strip's renderer last painted it
    /// (queue item 681), if it ever did.
    pub strip: Option<&'a Frame>,
    /// Why the strip's renderer painted no tab, if it said so since it
    /// last painted one: shown in the tab's place.
    pub strip_sentence: Option<&'a str>,
    /// Which renderer draws the page, in the one wording there is of it:
    /// [`crate::pages::RenderedBy`]'s, as the socket's `renderer` answer
    /// says it.
    pub renderer: &'a str,
    /// What agents' verbs were answered with, oldest first, as long as the
    /// frame keeps them ([`crate::agent_lines::AgentLines`]).
    pub agent_lines: &'a [AgentLine],
}

/// The frame's pixels: the rail's colour edge to edge — the band across
/// the top of this engine's own window, because the frame stands where that
/// band would be — and in it, as [`FramePlacement`] places them, the
/// page's tab, which renderer draws the page and then what agents did, a
/// refusal in colours of its own. A frame of no size is a canvas of no
/// size.
///
/// The tab is the strip's renderer's frame, one of its pixels to each
/// device pixel; when that renderer has since said why it painted none,
/// the sentence stands in the tab's place, as a gone tab's does in this
/// engine's window — there is no room below the band to keep both.
pub fn compose_frame(scene: &FrameScene<'_>, lettering: &Lettering) -> Canvas {
    let (width, height) = scene.window;
    let mut canvas = Canvas::new(width, height, rail());
    let scale = scene.scale;
    let strip = match (scene.strip_sentence, scene.strip) {
        (Some(sentence), _) => lettering.line_width(sentence, scale),
        (None, Some(frame)) => frame.width,
        (None, None) => 0,
    };
    let placed = FramePlacement::of(
        scene.window,
        scale,
        strip,
        lettering.line_width(scene.renderer, scale),
        scene.agent_lines.len(),
    );
    match (scene.strip_sentence, scene.strip) {
        (Some(sentence), _) => {
            lettering.draw(&mut canvas, placed.strip, sentence, scale, notice_line());
        }
        (None, Some(frame)) => copy(&mut canvas, frame, placed.strip),
        (None, None) => {}
    }
    lettering.draw(
        &mut canvas,
        placed.renderer,
        scene.renderer,
        scale,
        renderer_line(),
    );
    draw_agent_lines(
        &mut canvas,
        lettering,
        scene.agent_lines,
        placed.agent_lines,
        scale,
    );
    canvas
}

/// Draw each of `lines` into its rectangle, if it was given one, in the
/// colours of a verb that ran or of a refusal.
fn draw_agent_lines(
    canvas: &mut Canvas,
    lettering: &Lettering,
    lines: &[AgentLine],
    rects: impl IntoIterator<Item = Option<PixelRect>>,
    scale: Scale,
) {
    for (line, rect) in lines.iter().zip(rects) {
        let colours = if line.refused {
            refused_line()
        } else {
            agent_line()
        };
        if let Some(rect) = rect {
            lettering.draw(canvas, rect, &line.sentence, scale, colours);
        }
    }
}

/// Copy `frame` into `rect` of `canvas`, one of its pixels to each device
/// pixel, from its top-left corner.
///
/// Blended over what is there rather than written, so a frame with a
/// transparent pixel shows the window's colour there instead of black.
fn copy(canvas: &mut Canvas, frame: &Frame, rect: PixelRect) {
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            let Some((red, green, blue, alpha)) = frame.at(x - rect.x, y - rect.y) else {
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

    /// What the window keeps after an agent activated box#3.
    fn activated() -> [AgentLine; 1] {
        [AgentLine {
            sentence: "activated box#3".to_owned(),
            refused: false,
        }]
    }

    fn rgba8(canvas: &Canvas, x: u32, y: u32) -> Option<(u8, u8, u8, u8)> {
        canvas.at(x, y).map(Rgba::to_rgba8)
    }

    /// A window of `window` with a tab's `frame` and nothing else.
    fn tab_only(window: (u32, u32), scale: Scale, frame: Option<&Frame>) -> Scene<'_> {
        Scene {
            window,
            scale,
            strip: None,
            strip_sentence: None,
            frame,
            sentence: None,
            agent_lines: &[],
            marked: &[],
        }
    }

    #[test]
    fn a_window_with_nothing_sent_is_the_rail_above_its_background() {
        let canvas = compose(&tab_only((4, 40), Scale::ONE, None), &lettering());
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

    /// A frame of `window` saying Chrome draws the page, and `lines`.
    fn framed(window: (u32, u32), scale: Scale, lines: &[AgentLine]) -> FrameScene<'_> {
        FrameScene {
            window,
            scale,
            strip: None,
            strip_sentence: None,
            renderer: "Chrome/155.0.8059.86, driven by this browser",
            agent_lines: lines,
        }
    }

    #[test]
    fn the_pages_tab_is_drawn_first_one_to_one_and_the_renderer_after_it() {
        let lettering = lettering();
        // A painted tab of four by two device pixels, one of them clear.
        let mut painted = Canvas::new(4, 2, Rgba::from_rgba8(0, 0, 0, 0));
        painted.fill_rect(0, 0, 4, 1, Rgba::from_rgba8(10, 20, 30, 255));
        painted.fill_rect(0, 1, 3, 1, Rgba::from_rgba8(10, 20, 30, 255));
        let tab = Frame::from_canvas(&painted);
        let scene = FrameScene {
            strip: Some(&tab),
            ..framed((1200, 36), Scale::ONE, &[])
        };
        let canvas = compose_frame(&scene, &lettering);
        for (x, y) in [(0, 0), (3, 0), (0, 1)] {
            assert_eq!(rgba8(&canvas, x, y), Some((10, 20, 30, 255)), "at {x},{y}");
        }
        // Its clear pixel shows the rail, and so does all below it.
        assert_eq!(rgba8(&canvas, 3, 1), Some(rail().to_rgba8()));
        assert_eq!(rgba8(&canvas, 0, 2), Some(rail().to_rgba8()));
        // The renderer's sentence starts a gap after it.
        let placed = FramePlacement::of(
            (1200, 36),
            Scale::ONE,
            4,
            lettering.line_width(scene.renderer, Scale::ONE),
            0,
        );
        assert_eq!(placed.renderer.x, 5);
        assert_eq!(rgba8(&canvas, 4, 20), Some(rail().to_rgba8()), "the gap");
        let alone = compose_frame(&framed((1200, 36), Scale::ONE, &[]), &lettering);
        let shifted = (0..36).all(|y| {
            (0..placed.renderer.width).all(|x| rgba8(&alone, x, y) == rgba8(&canvas, x + 5, y))
        });
        assert!(shifted, "the same sentence, moved past the tab");
    }

    #[test]
    fn a_strip_that_said_why_it_painted_nothing_says_it_in_the_tabs_place() {
        let lettering = lettering();
        let tab = Frame::from_canvas(&Canvas::new(4, 2, Rgba::from_rgba8(10, 20, 30, 255)));
        let sentence = "the tab strip could not be drawn";
        let scene = FrameScene {
            strip: Some(&tab),
            strip_sentence: Some(sentence),
            ..framed((1200, 36), Scale::ONE, &[])
        };
        let canvas = compose_frame(&scene, &lettering);
        let needs = lettering.line_width(sentence, Scale::ONE);
        // The sentence's ground under its rule, not the old tab, at its
        // left edge.
        assert_eq!(rgba8(&canvas, 0, 0), Some(notice_line().rule.to_rgba8()));
        assert_eq!(rgba8(&canvas, 0, 1), Some(notice_line().ground.to_rgba8()));
        assert_eq!(
            rgba8(&canvas, needs - 1, 35),
            Some(notice_line().ground.to_rgba8())
        );
        assert_eq!(
            rgba8(&canvas, needs, 20),
            Some(rail().to_rgba8()),
            "the gap"
        );
    }

    #[test]
    fn the_frame_around_chromes_window_is_the_rail_and_says_which_renderer() {
        let lettering = lettering();
        // A frame of 1200 by 36 points at two.
        let scene = framed((2400, 72), Scale::TWO, &[]);
        let canvas = compose_frame(&scene, &lettering);
        assert_eq!((canvas.width(), canvas.height()), (2400, 72));
        let needs = lettering.line_width(scene.renderer, Scale::TWO);
        assert!((400..1200).contains(&needs), "{needs}");
        // Past the renderer's sentence, the rail edge to edge.
        for y in 0..72 {
            for x in needs..2400 {
                assert_eq!(rgba8(&canvas, x, y), Some(rail().to_rgba8()), "at {x},{y}");
            }
        }
        // Its words are in it, and only in it.
        let inked = |x0: u32, x1: u32| {
            (0..72).any(|y| (x0..x1).any(|x| rgba8(&canvas, x, y) != Some(rail().to_rgba8())))
        };
        assert!(inked(0, needs));
        let nothing = compose_frame(&framed((0, 0), Scale::TWO, &[]), &lettering);
        assert_eq!((nothing.width(), nothing.height()), (0, 0));
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
        let scene = framed((1200, 36), Scale::ONE, &lines);
        let canvas = compose_frame(&scene, &lettering);
        let placed = FramePlacement::of(
            (1200, 36),
            Scale::ONE,
            0,
            lettering.line_width(scene.renderer, Scale::ONE),
            2,
        );
        let [Some(refused), Some(ran), None] = placed.agent_lines else {
            panic!("two lines are placed: {placed:?}");
        };
        // The ground of each, under its rule, at its right end.
        let ground = |rect: PixelRect| rgba8(&canvas, rect.x + rect.width - 1, rect.height - 2);
        assert_eq!(ground(refused), Some(refused_line().ground.to_rgba8()));
        assert_eq!(ground(ran), Some(agent_line().ground.to_rgba8()));
        // The gap between them, and before the first, is the rail.
        for x in [refused.x - 1, ran.x - 1] {
            assert_eq!(rgba8(&canvas, x, 20), Some(rail().to_rgba8()), "at {x}");
        }
        assert_eq!(ran.x + ran.width, 1200);
    }

    #[test]
    fn each_pixel_of_a_frame_is_one_device_pixel_below_the_band_at_the_scale() {
        // A frame its renderer painted at two is in device pixels already:
        // two of them across, one down, below a band of the strip's height
        // at two.
        let frame = two_pixels();
        let top = STRIP_HEIGHT * 2;
        let canvas = compose(
            &tab_only((5, top + 3), Scale::TWO, Some(&frame)),
            &lettering(),
        );
        assert_eq!(rgba8(&canvas, 0, top - 1), Some(rail().to_rgba8()));
        assert_eq!(rgba8(&canvas, 0, top), Some((255, 0, 0, 255)));
        assert_eq!(rgba8(&canvas, 1, top), Some((0, 0, 255, 255)));
        // Past the frame is background: nothing is replicated or stretched.
        for (x, y) in [(2, 0), (3, 0), (4, 0), (0, 1), (1, 1), (4, 2)] {
            assert_eq!(
                rgba8(&canvas, x, top + y),
                Some(background().to_rgba8()),
                "at {x},{y}"
            );
        }
    }

    #[test]
    fn at_one_and_a_half_a_frame_is_one_to_one_below_a_band_of_fifty_four() {
        let frame = two_pixels();
        let canvas = compose(
            &tab_only((3, 56), Scale::ONE_AND_A_HALF, Some(&frame)),
            &lettering(),
        );
        assert_eq!(rgba8(&canvas, 0, 53), Some(rail().to_rgba8()));
        assert_eq!(rgba8(&canvas, 0, 54), Some((255, 0, 0, 255)));
        assert_eq!(rgba8(&canvas, 1, 54), Some((0, 0, 255, 255)));
        for (x, y) in [(2, 54), (0, 55), (1, 55)] {
            assert_eq!(
                rgba8(&canvas, x, y),
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
                ..tab_only((3, STRIP_HEIGHT + 2), Scale::ONE, Some(&frame))
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
                ..tab_only((1, STRIP_HEIGHT + 1), Scale::ONE, Some(&clear))
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
                scale: Scale::TWO,
                strip: Some(&frame),
                strip_sentence: Some("gone"),
                frame: Some(&frame),
                sentence: Some("gone"),
                agent_lines: &activated(),
                marked: &[],
            },
            &lettering(),
        );
        assert!(canvas.is_empty());
    }

    #[test]
    fn the_ring_is_drawn_over_the_frame_and_under_the_browsers_lines() {
        let frame = Frame {
            width: 40,
            height: 64,
            pixels: [255, 255, 255, 255].repeat(40 * 64),
        };
        // A box at 6,6 of the frame, 10 wide and 40 tall: 6,42 to 16,82 in
        // the window, its ring 2,38 to 20,86 — whose bottom is under the
        // agent's line, from 68 down, and is not drawn.
        let ringed = [Marked {
            x: 6,
            y: 6,
            width: 10,
            height: 40,
        }];
        let window = (40, STRIP_HEIGHT + 64);
        let canvas = compose(
            &Scene {
                agent_lines: &activated(),
                marked: &ringed,
                ..tab_only(window, Scale::ONE, Some(&frame))
            },
            &lettering(),
        );
        let accent = Some(crate::colours::mark().to_rgba8());
        let white = Some((255, 255, 255, 255));
        let line_top = window.1 - crate::place::NOTICE_HEIGHT;
        assert_eq!(line_top, 68);
        for (x, y) in [(2, 38), (19, 39), (2, 67), (19, 67)] {
            assert_eq!(rgba8(&canvas, x, y), accent, "the ring at {x},{y}");
        }
        for (x, y) in [(4, 40), (6, 42), (10, 50), (1, 38), (20, 38), (2, 37)] {
            assert_eq!(rgba8(&canvas, x, y), white, "the page at {x},{y}");
        }
        // The agent's line is over it.
        assert_eq!(
            rgba8(&canvas, 2, line_top + 2),
            Some(agent_line().ground.to_rgba8())
        );
        // With no frame there is nothing to ring.
        let bare = compose(
            &Scene {
                marked: &ringed,
                ..tab_only(window, Scale::ONE, None)
            },
            &lettering(),
        );
        assert_eq!(rgba8(&bare, 2, 38), Some(background().to_rgba8()));
    }
}
