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
//!   size, the rows whose bytes differ, each run of them the smallest
//!   rectangle holding its differing pixels (queue item 896);
//! - **the card's page beneath** — another picture of the same size at the
//!   same place, its pixels that differ, found as the first screen's and
//!   put where the picture lands one to one, inside the part of its place
//!   the window shows (queue item 873: a live card's 15 pictures a second
//!   each composed its whole place again, 1.8 million pixels and 14.8 ms of
//!   the window's event loop at two, for a box turning in a sixth of it);
//!   moved, of another size, come or gone, both places.
//!
//! What may have changed is then joined as a drawing's damage is
//! ([`alo_paint::pieces::cut`]): far apart, two rings in *Settings* are
//! composed again as two rectangles of about 198,000 pixels at two, not the
//! 725,000 of the rectangle holding both. A pixel two rectangles share is
//! composed twice, alike.

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recomposed {
    /// Nothing it is made from changed.
    Nothing,
    /// The pixels of these regions were composed again, and no others.
    Within(Vec<Region>),
    /// Every pixel.
    Whole,
}

impl Recomposed {
    /// How many pixels were composed again, of a window of `size`.
    pub fn area(&self, size: (u32, u32)) -> u64 {
        match self {
            Recomposed::Nothing => 0,
            Recomposed::Within(regions) => regions.iter().map(|region| region.area()).sum(),
            Recomposed::Whole => u64::from(size.0) * u64::from(size.1),
        }
    }
    /// The regions composed again — none when nothing was — or [`None`] for
    /// every pixel: what the pixels kept on the GPU are given (ADR 0105).
    pub fn regions(&self) -> Option<&[Region]> {
        match self {
            Recomposed::Nothing => Some(&[]),
            Recomposed::Within(regions) => Some(regions),
            Recomposed::Whole => None,
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
                Recomposed::Within(regions) => {
                    for &region in &regions {
                        compose_within(canvas, scene, lettering, region);
                        pack(canvas, words, region);
                    }
                    made.refresh(scene);
                    Recomposed::Within(regions)
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
    let mut found: Vec<Region> = Vec::new();
    let mut add = |rect: PixelRect| {
        if let Some(region) = Region::new(
            rect.x,
            rect.y,
            rect.x.saturating_add(rect.width).min(scene.window.0),
            rect.y.saturating_add(rect.height).min(scene.window.1),
        ) {
            found.push(region);
        }
    };
    if made.panel.as_ref() != scene.panel {
        add(placed.panel_room);
        add(placed.panel);
    }
    match (made.start.as_ref(), scene.start) {
        (None, None) => {}
        (Some(was), Some(is)) if (was.width, was.height) == (is.width, is.height) => {
            for rect in differing(was, is) {
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
    match (was_beneath, scene.beneath, scene.start) {
        _ if was_beneath == scene.beneath => {}
        // A new picture of the card, of the same size at the same place
        // (queue item 873): only where its pixels differ, as the first
        // screen's — each window pixel of it is the picture's one pixel
        // there, one to one, over the frame's.
        (Some((was, at)), Some((is, place)), Some(frame))
            if at == place && (was.width, was.height) == (is.width, is.height) =>
        {
            let shown = beneath_rect((is, place), (frame.width, frame.height), placed.start);
            for rect in differing(was, is) {
                if let Some(rect) = on_window(rect, place, placed.start, shown) {
                    add(rect);
                }
            }
        }
        _ => beneath_moved(&mut add, made, scene, placed),
    }
    let pieces = alo_paint::pieces::cut(&found, &[]);
    if pieces.is_empty() {
        Recomposed::Nothing
    } else {
        Recomposed::Within(pieces)
    }
}

/// The picture's `rect`, in its own pixels, where it lands in the window:
/// at `place` in the first screen's frame standing at `start`, inside
/// `shown`, the part of the place the window shows.
fn on_window(
    rect: PixelRect,
    place: Beneath,
    start: PixelRect,
    shown: PixelRect,
) -> Option<PixelRect> {
    let span = |origin: u32, at: i32, from: u32, length: u32, within: (u32, u32)| {
        let first = i64::from(origin) + i64::from(at) + i64::from(from);
        let low = first.max(i64::from(within.0));
        let high = (first + i64::from(length)).min(i64::from(within.0) + i64::from(within.1));
        Some((u32::try_from(low).ok()?, u32::try_from(high - low).ok()?))
    };
    let (x, width) = span(
        start.x,
        place.left,
        rect.x,
        rect.width,
        (shown.x, shown.width),
    )?;
    let (y, height) = span(
        start.y,
        place.top,
        rect.y,
        rect.height,
        (shown.y, shown.height),
    )?;
    (width > 0 && height > 0).then_some(PixelRect {
        x,
        y,
        width,
        height,
    })
}

/// Where the card's picture was and is, each whole: it moved, changed
/// size, came or went.
fn beneath_moved(
    add: &mut impl FnMut(PixelRect),
    made: &Made,
    scene: &ConversationScene<'_>,
    placed: ConversationPlacement,
) {
    let was_beneath = made
        .beneath
        .as_ref()
        .map(|(picture, place)| (picture, *place));
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

/// Where `one` and `other`, two frames of one size, differ: each run of
/// rows whose bytes differ, as the smallest rectangle holding every pixel
/// of it whose bytes differ — none where none do.
fn differing(one: &Frame, other: &Frame) -> Vec<PixelRect> {
    let row = (one.width as usize) * 4;
    if row == 0 {
        return Vec::new();
    }
    let was: Vec<&[u8]> = one.pixels.chunks_exact(row).collect();
    let is: Vec<&[u8]> = other.pixels.chunks_exact(row).collect();
    let differs = |y: usize| was.get(y) != is.get(y);
    let mut found = Vec::new();
    let mut y = 0;
    while y < was.len() {
        if !differs(y) {
            y += 1;
            continue;
        }
        let top = y;
        while y < was.len() && differs(y) {
            y += 1;
        }
        if let Some(rect) = columns(&was, &is, (top, y), one.width) {
            found.push(rect);
        }
    }
    found
}

/// The smallest rectangle holding every pixel whose bytes differ in rows
/// `top` to `bottom` (not including it) of `was` and `is`, rows of `width`
/// pixels.
fn columns(
    was: &[&[u8]],
    is: &[&[u8]],
    (top, bottom): (usize, usize),
    width: u32,
) -> Option<PixelRect> {
    let (mut left, mut right) = (width, 0);
    for y in top..bottom {
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
        if let Some(last) = (right..width).rev().find(pixel) {
            right = last + 1;
        }
    }
    let (top, bottom) = (u32::try_from(top).ok()?, u32::try_from(bottom).ok()?);
    (left < right).then_some(PixelRect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
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
            Some(recomposed.clone()),
            Region::new(580, 70, 592, 79).map(|region| Recomposed::Within(vec![region])),
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
            Recomposed::Within(vec![Region {
                left: 480,
                top: 20,
                right: 560,
                bottom: 60
            }]),
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
    fn a_new_picture_of_the_card_composes_again_only_where_its_pixels_changed() {
        // Queue item 873: a live card's page turning a box in a sixth of
        // its picture composed the whole place again, 15 times a second.
        let (panel, start) = (varied(480, 120, 1), varied(220, 120, 2));
        let picture = varied(90, 40, 3);
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
        let turned = touched(
            &picture,
            PixelRect {
                x: 20,
                y: 5,
                width: 10,
                height: 8,
            },
        );
        let next = scene(&panel, &start, Some((&turned, place)));
        let (words, recomposed) = composed.compose(&next, &lettering());
        assert_eq!(
            words,
            whole(&next).as_slice(),
            "every word as composed whole"
        );
        assert_eq!(
            recomposed,
            Recomposed::Within(vec![Region {
                left: 490,
                top: 25,
                right: 500,
                bottom: 33
            }]),
            "the picture's changed pixels, where they land in the window"
        );
        assert_eq!(recomposed.area((700, 120)), 10 * 8);

        // A change half off the frame's left edge: only the part shown.
        let slid = touched(
            &turned,
            PixelRect {
                x: 2,
                y: 30,
                width: 10,
                height: 6,
            },
        );
        let last = scene(&panel, &start, Some((&slid, place)));
        let (words, recomposed) = composed.compose(&last, &lettering());
        assert_eq!(words, whole(&last).as_slice());
        assert_eq!(
            recomposed,
            Recomposed::Within(vec![Region {
                left: 480,
                top: 50,
                right: 482,
                bottom: 56
            }]),
            "what is off the frame's left is not composed"
        );
        // A change entirely off the frame: nothing at all.
        let hidden = touched(
            &slid,
            PixelRect {
                x: 0,
                y: 0,
                width: 4,
                height: 3,
            },
        );
        let (words, recomposed) =
            composed.compose(&scene(&panel, &start, Some((&hidden, place))), &lettering());
        assert_eq!(
            words,
            whole(&scene(&panel, &start, Some((&hidden, place)))).as_slice()
        );
        assert_eq!(recomposed, Recomposed::Nothing);
    }

    #[test]
    fn a_card_turning_a_box_composes_again_the_box_and_not_its_page() {
        // The hand run's page at two, in proportion: a picture of 1200 by
        // 700 whose box of 280 by 280 turned and whose bar of 240 by 40
        // slid, far below it — two rectangles, not the 840,000 pixels of
        // the place; every word as composed whole.
        let (panel, start) = (varied(100, 900, 1), varied(1300, 900, 2));
        let picture = varied(1200, 700, 3);
        let place = Beneath {
            left: 40,
            top: 100,
            width: 1200,
            height: 700,
        };
        let mut composed = Composed::new();
        let mut first = scene(&panel, &start, Some((&picture, place)));
        first.window = (1400, 900);
        composed.compose(&first, &lettering());
        let turned = touched(
            &touched(
                &picture,
                PixelRect {
                    x: 400,
                    y: 60,
                    width: 280,
                    height: 280,
                },
            ),
            PixelRect {
                x: 100,
                y: 600,
                width: 240,
                height: 40,
            },
        );
        let mut next = scene(&panel, &start, Some((&turned, place)));
        next.window = (1400, 900);
        let (words, recomposed) = composed.compose(&next, &lettering());
        assert_eq!(
            words,
            whole(&next).as_slice(),
            "every word as composed whole"
        );
        let Recomposed::Within(regions) = &recomposed else {
            panic!("composed {recomposed:?}");
        };
        assert_eq!(regions.len(), 2, "{regions:?}");
        assert_eq!(recomposed.area((1400, 900)), 280 * 280 + 240 * 40);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_pixels_kept_on_the_gpu_are_every_word_composed_whole_given_only_what_changed() {
        // Queue item 873 (ADR 0105): what the window keeps on the GPU, read
        // back after each redraw, is every word composed whole — given a
        // letter's bytes for a letter, a picture's changed pixels for a
        // picture, and the whole window only when it is composed whole.
        let mut held = crate::present_kept::Held::headless().unwrap_or_else(|why| panic!("{why}"));
        let (panel, start) = (varied(480, 120, 1), varied(220, 120, 2));
        let picture = varied(90, 40, 3);
        let place = Beneath {
            left: -10,
            top: 20,
            width: 90,
            height: 40,
        };
        let mut composed = Composed::new();
        let mut shown = |scene: &ConversationScene<'_>| {
            let (words, recomposed) = composed.compose(scene, &lettering());
            let written = held.write(scene.window, words, recomposed.regions());
            assert_eq!(
                held.read_back(),
                Ok(whole(scene)),
                "every word kept as composed whole"
            );
            written
        };
        let whole_window = 700 * 120 * 4;
        assert_eq!(
            shown(&scene(&panel, &start, Some((&picture, place)))),
            whole_window
        );
        let letter = touched(
            &start,
            PixelRect {
                x: 100,
                y: 70,
                width: 12,
                height: 9,
            },
        );
        assert_eq!(
            shown(&scene(&panel, &letter, Some((&picture, place)))),
            12 * 9 * 4,
            "a letter's bytes, of {whole_window}"
        );
        assert_eq!(
            shown(&scene(&panel, &letter, Some((&picture, place)))),
            0,
            "nothing changed, nothing written"
        );
        let turned = touched(
            &picture,
            PixelRect {
                x: 20,
                y: 5,
                width: 10,
                height: 8,
            },
        );
        assert_eq!(
            shown(&scene(&panel, &letter, Some((&turned, place)))),
            10 * 8 * 4,
            "a picture's changed pixels"
        );
        let mut said = scene(&panel, &letter, Some((&turned, place)));
        said.start_sentence = Some("the first screen's renderer went");
        assert_eq!(shown(&said), whole_window, "composed whole, written whole");
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

    #[test]
    fn two_changes_far_apart_in_the_first_screen_compose_again_as_two_rectangles() {
        // Queue item 896: two rings in *Settings*, one at the top right and
        // one half way down the left — two rectangles, not the one holding
        // both; every word as composed whole.
        let (panel, start) = (varied(100, 1000, 1), varied(1200, 1000, 2));
        let mut composed = Composed::new();
        let mut first = scene(&panel, &start, None);
        first.window = (1300, 1000);
        composed.compose(&first, &lettering());
        let rings = touched(
            &touched(
                &start,
                PixelRect {
                    x: 700,
                    y: 40,
                    width: 40,
                    height: 40,
                },
            ),
            PixelRect {
                x: 100,
                y: 600,
                width: 300,
                height: 120,
            },
        );
        let mut next = scene(&panel, &rings, None);
        next.window = (1300, 1000);
        let (words, recomposed) = composed.compose(&next, &lettering());
        assert_eq!(
            words,
            whole(&next).as_slice(),
            "every word as composed whole"
        );
        let Recomposed::Within(regions) = &recomposed else {
            panic!("composed {recomposed:?}");
        };
        assert_eq!(regions.len(), 2, "{regions:?}");
        assert_eq!(recomposed.area((1300, 1000)), 40 * 40 + 300 * 120);
    }
}
