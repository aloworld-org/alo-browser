/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The conversation's window as it was last composed, composed again only
//! where what it is made from changed (queue item 872, ADR 0102).
//!
//! A letter typed beside a card changes about a thirtieth of the first
//! screen's frame. The window composed all 4.4 million of its pixels again
//! for it, at two — the paper, the panel, the card's page beneath and the
//! first screen over it, a pixel at a time in floats — and then turned each
//! into the word the window server takes: 25 ms and 7 ms of the 26–38 the
//! window's redraw took after the renderer answered. Now the window's
//! pixels, and their words, are kept; what they were made from is compared
//! with what they are made from now; and only the window's pixels that
//! anything changed under are composed again ([`compose_within`]) and
//! turned into words again.
//!
//! # Why the pixels come out the same
//!
//! Composing is a pixel's own arithmetic onto that pixel: the paper, then
//! the panel's colour, the card's page and the first screen's frame, the
//! panel's frame, the shade and the sheet, the lines along the bottom — each
//! reads the pixel it writes and the one pixel of what it draws there, and
//! nothing else. So a window pixel none of whose inputs changed composes as
//! it did, and every pixel whose inputs may have changed is composed again,
//! inside the region, exactly as composing the whole window would. What may
//! have changed is said by comparing:
//!
//! - **the window's size, scale, side, the rail's name and both lines** —
//!   another, and everything is composed again, as it is while the sheet is
//!   out, whose shade lies over all of it;
//! - **where everything is placed** — another, and everything;
//! - **the panel's frame** — another, and the panel's place;
//! - **the first screen's frame** — of another size, everything; of the same
//!   size, the smallest rectangle holding every pixel whose bytes differ;
//! - **the card's page beneath** — another picture or place, both places.

use crate::conversation_compose::{
    ConversationScene, beneath_rect, compose_conversation, compose_within,
};
use crate::conversation_place::{ConversationPlacement, Side};
use crate::notice::Lettering;
use crate::place::PixelRect;
use crate::present::packed;
use crate::scale::Scale;
use alo_paint::{Canvas, Region};
use alo_renderer::Frame;
use alo_renderer::beneath::Beneath;

/// What composing the window again did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recomposed {
    /// Nothing it is made from changed.
    Nothing,
    /// The pixels of this region were composed again, and no others.
    Within(Region),
    /// Every pixel.
    Whole,
}

impl Recomposed {
    /// How many pixels were composed again, of a window of `size`.
    pub fn area(self, size: (u32, u32)) -> u64 {
        match self {
            Recomposed::Nothing => 0,
            Recomposed::Within(region) => region.area(),
            Recomposed::Whole => u64::from(size.0) * u64::from(size.1),
        }
    }
}

/// What the window was composed from, kept to compare the next with.
#[derive(Debug, Clone, PartialEq)]
struct Made {
    window: (u32, u32),
    scale: Scale,
    side: Side,
    named: bool,
    panel_sentence: Option<String>,
    start_sentence: Option<String>,
    sheet: bool,
    placed: ConversationPlacement,
    panel: Option<Frame>,
    start: Option<Frame>,
    beneath: Option<(Frame, Beneath)>,
}

impl Made {
    fn of(scene: &ConversationScene<'_>, placed: ConversationPlacement) -> Self {
        Self {
            window: scene.window,
            scale: scene.scale,
            side: scene.side,
            named: scene.named,
            panel_sentence: scene.panel_sentence.map(str::to_owned),
            start_sentence: scene.start_sentence.map(str::to_owned),
            sheet: scene.sheet.is_some(),
            placed,
            panel: scene.panel.cloned(),
            start: scene.start.cloned(),
            beneath: scene
                .beneath
                .map(|(picture, place)| (picture.clone(), place)),
        }
    }

    /// Made from `scene` now, placed as it was: only the frames that
    /// differ copied.
    fn refresh(&mut self, scene: &ConversationScene<'_>) {
        if self.panel.as_ref() != scene.panel {
            self.panel = scene.panel.cloned();
        }
        if self.start.as_ref() != scene.start {
            match (&mut self.start, scene.start) {
                (Some(kept), Some(frame)) => kept.clone_from(frame),
                (kept, frame) => *kept = frame.cloned(),
            }
        }
        let beneath = self
            .beneath
            .as_ref()
            .map(|(picture, place)| (picture, *place));
        if beneath != scene.beneath {
            self.beneath = scene
                .beneath
                .map(|(picture, place)| (picture.clone(), place));
        }
    }
}

/// The window's pixels as last composed, their words, and what they were
/// composed from.
#[derive(Debug, Default)]
pub struct Composed {
    kept: Option<(Made, Canvas, Vec<u32>)>,
}

impl Composed {
    /// Nothing composed yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// The window's pixels for `scene`, as the words the window server
    /// takes, row by row, composed again only where they can have changed;
    /// and what was composed again.
    pub fn compose(
        &mut self,
        scene: &ConversationScene<'_>,
        lettering: &Lettering,
    ) -> (&[u32], Recomposed) {
        let placed = ConversationPlacement::of(scene.window, scene.scale, scene.parts());
        let recomposed = match self.kept.as_mut() {
            Some((made, canvas, words)) => match changed(made, scene, placed) {
                Recomposed::Nothing => Recomposed::Nothing,
                Recomposed::Within(region) => {
                    compose_within(canvas, scene, lettering, region);
                    pack(canvas, words, region);
                    made.refresh(scene);
                    Recomposed::Within(region)
                }
                Recomposed::Whole => {
                    self.kept = None;
                    Recomposed::Whole
                }
            },
            None => Recomposed::Whole,
        };
        if recomposed == Recomposed::Whole {
            let canvas = compose_conversation(scene, lettering);
            let words = canvas.pixels().iter().map(|pixel| packed(*pixel)).collect();
            self.kept = Some((Made::of(scene, placed), canvas, words));
        }
        let words = self.kept.as_ref().map_or(&[][..], |(_, _, words)| words);
        (words, recomposed)
    }
}

/// Where the window's pixels can differ, composed from `scene` placed at
/// `placed` rather than from `made`.
fn changed(
    made: &Made,
    scene: &ConversationScene<'_>,
    placed: ConversationPlacement,
) -> Recomposed {
    if made.window != scene.window
        || made.scale != scene.scale
        || made.side != scene.side
        || made.named != scene.named
        || made.panel_sentence.as_deref() != scene.panel_sentence
        || made.start_sentence.as_deref() != scene.start_sentence
        || made.sheet
        || scene.sheet.is_some()
        || made.placed != placed
    {
        return Recomposed::Whole;
    }
    let mut found: Option<Region> = None;
    let mut add = |rect: PixelRect| {
        if let Some(region) = Region::new(
            rect.x,
            rect.y,
            rect.x.saturating_add(rect.width).min(scene.window.0),
            rect.y.saturating_add(rect.height).min(scene.window.1),
        ) {
            found = Some(found.map_or(region, |was| was.joined(region)));
        }
    };
    if made.panel.as_ref() != scene.panel {
        add(placed.panel_room);
        add(placed.panel);
    }
    match (made.start.as_ref(), scene.start) {
        (None, None) => {}
        (Some(was), Some(is)) if (was.width, was.height) == (is.width, is.height) => {
            if let Some(rect) = differing(was, is) {
                add(PixelRect {
                    x: placed.start.x + rect.x,
                    y: placed.start.y + rect.y,
                    ..rect
                });
            }
        }
        _ => return Recomposed::Whole,
    }
    let was_beneath = made
        .beneath
        .as_ref()
        .map(|(picture, place)| (picture, *place));
    if was_beneath != scene.beneath {
        for (beneath, frame) in [
            (was_beneath, made.start.as_ref()),
            (scene.beneath, scene.start),
        ] {
            if let (Some(beneath), Some(frame)) = (beneath, frame) {
                add(beneath_rect(
                    beneath,
                    (frame.width, frame.height),
                    placed.start,
                ));
            }
        }
    }
    found.map_or(Recomposed::Nothing, Recomposed::Within)
}

/// The smallest rectangle of `one` and `other`, two frames of one size,
/// holding every pixel whose bytes differ, or [`None`] where none do.
fn differing(one: &Frame, other: &Frame) -> Option<PixelRect> {
    let row = (one.width as usize) * 4;
    if row == 0 {
        return None;
    }
    let was: Vec<&[u8]> = one.pixels.chunks_exact(row).collect();
    let is: Vec<&[u8]> = other.pixels.chunks_exact(row).collect();
    let differs = |y: &usize| was.get(*y) != is.get(*y);
    let top = (0..was.len()).find(differs)?;
    let bottom = (top..was.len()).rev().find(differs).unwrap_or(top);
    let (mut left, mut right) = (one.width, 0);
    for y in top..=bottom {
        let (Some(a), Some(b)) = (was.get(y), is.get(y)) else {
            continue;
        };
        let pixel = |x: &u32| {
            let at = (*x as usize) * 4;
            a.get(at..at + 4) != b.get(at..at + 4)
        };
        if let Some(first) = (0..left).find(pixel) {
            left = first;
        }
        if let Some(last) = (right..one.width).rev().find(pixel) {
            right = last + 1;
        }
    }
    let (top, bottom) = (u32::try_from(top).ok()?, u32::try_from(bottom).ok()?);
    (left < right).then_some(PixelRect {
        x: left,
        y: top,
        width: right - left,
        height: bottom + 1 - top,
    })
}

/// `canvas`'s pixels inside `region` turned into `words` again.
fn pack(canvas: &Canvas, words: &mut [u32], region: Region) {
    let width = canvas.width() as usize;
    for y in region.top..region.bottom {
        let from = y as usize * width;
        let (Some(pixels), Some(row)) = (
            canvas
                .pixels()
                .get(from + region.left as usize..from + region.right as usize),
            words.get_mut(from + region.left as usize..from + region.right as usize),
        ) else {
            continue;
        };
        for (word, pixel) in row.iter_mut().zip(pixels) {
            *word = packed(*pixel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lettering() -> Lettering {
        Lettering::compiled_in().expect("DejaVu Sans is compiled in")
    }

    /// A frame of `width` by `height` whose every pixel is its own colour,
    /// some of them half seen, so that a pixel composed from the wrong one
    /// shows.
    fn varied(width: u32, height: u32, seed: u8) -> Frame {
        let mut pixels = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let byte = |value: u32| u8::try_from(value % 256).unwrap_or(0);
                let (x, y) = (byte(x), byte(y));
                let alpha = if (x ^ y) % 5 == 0 { 128 } else { 255 };
                pixels.extend([x.wrapping_mul(7) ^ seed, y.wrapping_mul(3), x ^ y, alpha]);
            }
        }
        Frame {
            width,
            height,
            pixels,
        }
    }

    /// The same frame with a rectangle of it painted another colour.
    fn touched(frame: &Frame, rect: PixelRect) -> Frame {
        let mut frame = frame.clone();
        for y in rect.y..rect.y + rect.height {
            for x in rect.x..rect.x + rect.width {
                let at = ((y * frame.width + x) * 4) as usize;
                if let Some(pixel) = frame.pixels.get_mut(at..at + 4) {
                    pixel.copy_from_slice(&[9, 200, 40, 255]);
                }
            }
        }
        frame
    }

    fn scene<'a>(
        panel: &'a Frame,
        start: &'a Frame,
        beneath: Option<(&'a Frame, Beneath)>,
    ) -> ConversationScene<'a> {
        ConversationScene {
            window: (700, 120),
            scale: Scale::TWO,
            panel: Some(panel),
            panel_sentence: None,
            start: Some(start),
            start_sentence: None,
            side: Side::Panel,
            named: false,
            sheet: None,
            beneath,
        }
    }

    /// Every word of the window composed whole.
    fn whole(scene: &ConversationScene<'_>) -> Vec<u32> {
        compose_conversation(scene, &lettering())
            .pixels()
            .iter()
            .map(|pixel| packed(*pixel))
            .collect()
    }

    #[test]
    fn a_change_in_the_first_screen_composes_again_only_where_its_pixels_changed() {
        let (panel, start) = (varied(480, 120, 1), varied(220, 120, 2));
        let picture = varied(90, 40, 3);
        let place = Beneath {
            left: 30,
            top: 20,
            width: 90,
            height: 40,
        };
        let mut composed = Composed::new();
        let first = scene(&panel, &start, Some((&picture, place)));
        let (_, recomposed) = composed.compose(&first, &lettering());
        assert_eq!(recomposed, Recomposed::Whole);
        let (_, again) = composed.compose(&first, &lettering());
        assert_eq!(again, Recomposed::Nothing, "nothing changed");

        let letter = touched(
            &start,
            PixelRect {
                x: 100,
                y: 70,
                width: 12,
                height: 9,
            },
        );
        let next = scene(&panel, &letter, Some((&picture, place)));
        let (words, recomposed) = composed.compose(&next, &lettering());
        assert_eq!(
            words,
            whole(&next).as_slice(),
            "every word as composed whole"
        );
        assert_eq!(
            Some(recomposed),
            Region::new(580, 70, 592, 79).map(Recomposed::Within),
            "the letter's pixels, where the first screen stands"
        );
        assert_eq!(recomposed.area((700, 120)), 12 * 9);
    }

    #[test]
    fn a_new_picture_of_the_card_composes_again_only_its_place() {
        let (panel, start) = (varied(480, 120, 1), varied(220, 120, 2));
        let (picture, newer) = (varied(90, 40, 3), varied(90, 40, 4));
        let place = Beneath {
            left: -10,
            top: 20,
            width: 90,
            height: 40,
        };
        let mut composed = Composed::new();
        composed.compose(
            &scene(&panel, &start, Some((&picture, place))),
            &lettering(),
        );
        let next = scene(&panel, &start, Some((&newer, place)));
        let (words, recomposed) = composed.compose(&next, &lettering());
        assert_eq!(words, whole(&next).as_slice());
        assert_eq!(
            recomposed,
            Recomposed::Within(Region {
                left: 480,
                top: 20,
                right: 560,
                bottom: 60
            }),
            "the place, less what is off the frame's left"
        );
        let moved = Beneath { left: 50, ..place };
        let last = scene(&panel, &start, Some((&newer, moved)));
        let (words, recomposed) = composed.compose(&last, &lettering());
        assert_eq!(words, whole(&last).as_slice());
        assert_eq!(recomposed.area((700, 120)), 140 * 40, "where it was and is");
        let (words, _) = composed.compose(&scene(&panel, &start, None), &lettering());
        assert_eq!(
            words,
            whole(&scene(&panel, &start, None)).as_slice(),
            "and gone"
        );
    }

    #[test]
    fn the_panel_a_line_or_another_size_composes_again_what_it_has_to() {
        let (panel, start) = (varied(480, 120, 1), varied(220, 120, 2));
        let mut composed = Composed::new();
        composed.compose(&scene(&panel, &start, None), &lettering());
        let hovered = touched(
            &panel,
            PixelRect {
                x: 4,
                y: 4,
                width: 3,
                height: 3,
            },
        );
        let next = scene(&hovered, &start, None);
        let (words, recomposed) = composed.compose(&next, &lettering());
        assert_eq!(words, whole(&next).as_slice());
        assert_eq!(recomposed.area((700, 120)), 480 * 120, "the panel's place");
        let mut said = scene(&hovered, &start, None);
        said.start_sentence = Some("the first screen's renderer went");
        let (words, recomposed) = composed.compose(&said, &lettering());
        assert_eq!(words, whole(&said).as_slice());
        assert_eq!(recomposed, Recomposed::Whole);
        let wider = varied(230, 120, 2);
        let mut grown = scene(&hovered, &wider, None);
        grown.window = (710, 120);
        let (words, recomposed) = composed.compose(&grown, &lettering());
        assert_eq!(words, whole(&grown).as_slice());
        assert_eq!(recomposed, Recomposed::Whole);
    }
}
