/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A button at the right end of one of the browser's own lines: where it
//! stands and how it is drawn (ADR 0116 § 4, queue item 990).
//!
//! The one there is is *Quit and update*, on the first screen's line saying
//! an update is ready. It is the browser's own drawing, as the line is
//! ([`crate::notice`]): no renderer paints it and no renderer claims a press
//! on it. The browser process decides a press on it from its own
//! composition ([`crate::conversation_aim::on_the_button`]), so nothing a
//! page or a renderer says can press it — and the agent socket, which
//! reaches the page shown in a card and takes no position
//! (`docs/contracts/agent-socket.md`), has nothing to name it by.
//!
//! # Its size
//!
//! [`WIDTH`] by [`HEIGHT`] CSS pixels, [`INSET`] from the line's right end
//! and centred down it, at the window's scale; the line's words end
//! [`GAP`] short of it. A line too short or too narrow to hold it whole
//! has none: half a button is not one to press.

use crate::colours::LineColours;
use crate::notice::Lettering;
use crate::place::PixelRect;
use crate::scale::Scale;
use alo_paint::Canvas;

/// Which button a line holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineButton {
    /// *Quit and update*, on the line saying an update is ready.
    QuitAndUpdate,
}

impl LineButton {
    /// What it says, and is named.
    pub fn label(self) -> &'static str {
        match self {
            Self::QuitAndUpdate => alo_update::quitting::QUIT_AND_UPDATE,
        }
    }
}

/// How wide the button is, in CSS pixels: its label, *Quit and update* at
/// the line's 13 pixels, and room either side of it.
pub const WIDTH: u32 = 128;

/// How tall, in CSS pixels: the line's 32 less four above and below.
pub const HEIGHT: u32 = 24;

/// How far its right edge is from the line's, in CSS pixels: the line's
/// own inset ([`crate::notice::INSET`]).
pub const INSET: u32 = 12;

/// How far the line's words end before it, in CSS pixels.
pub const GAP: u32 = 12;

/// Where a button stands on `line`, in device pixels at `scale`, if the
/// line holds one whole.
pub fn placed(line: PixelRect, scale: Scale) -> Option<PixelRect> {
    let (width, height, inset) = (
        scale.device(WIDTH),
        scale.device(HEIGHT),
        scale.device(INSET),
    );
    if line.height < height || line.width < width + 2 * inset {
        return None;
    }
    Some(PixelRect {
        x: line.x + line.width - inset - width,
        y: line.y + (line.height - height) / 2,
        width,
        height,
    })
}

/// How many device pixels at the right of `line` the words leave for
/// `button`, past the line's inset: the button and the gap before it.
pub fn trail(line: PixelRect, button: PixelRect, scale: Scale) -> u32 {
    (line.x + line.width)
        .saturating_sub(scale.device(INSET))
        .saturating_sub(button.x)
        + scale.device(GAP)
}

/// The line `line` saying `sentence` in `colours`, with a button saying
/// `label` at its right end in `button_colours` when it holds one — or the
/// line alone, as [`Lettering::draw`] draws it, when it does not.
pub fn draw_line(
    lettering: &Lettering,
    canvas: &mut Canvas,
    line: PixelRect,
    (sentence, label): (&str, &str),
    scale: Scale,
    (colours, button_colours): (LineColours, LineColours),
) {
    let Some(button) = placed(line, scale) else {
        lettering.draw(canvas, line, sentence, scale, colours);
        return;
    };
    lettering.draw_between(
        canvas,
        line,
        sentence,
        scale,
        colours,
        (0, trail(line, button, scale)),
    );
    draw(lettering, canvas, button, label, scale, button_colours);
}

/// The button at `button`: a ring one CSS pixel thick in the colours'
/// rule, its ground inside, and `label` centred on it in their ink — cut
/// as a line's words are if it were ever wider than the room inside.
pub fn draw(
    lettering: &Lettering,
    canvas: &mut Canvas,
    button: PixelRect,
    label: &str,
    scale: Scale,
    colours: LineColours,
) {
    if button.is_empty() {
        return;
    }
    let (left, top) = (signed(button.x), signed(button.y));
    canvas.fill_rect(left, top, button.width, button.height, colours.rule);
    let ring = scale.device(1);
    if button.width > 2 * ring && button.height > 2 * ring {
        canvas.fill_rect(
            left + signed(ring),
            top + signed(ring),
            button.width - 2 * ring,
            button.height - 2 * ring,
            colours.ground,
        );
    }
    let room = whole(button.width) - 2.0 * whole(scale.device(GAP / 2));
    let words = lettering.fitted(label, room, scale);
    let across = lettering.width_of(&words, scale);
    let from = (whole(button.width) - across) / 2.0;
    // Begun on a whole device pixel, so the label is drawn alike wherever
    // the button stands.
    lettering.letter(
        canvas,
        button,
        &words,
        whole(button.x) + from.max(0.0).floor(),
        scale,
        colours.ink,
    );
}

/// A count of pixels as a length.
#[expect(
    clippy::cast_precision_loss,
    reason = "a whole number below 2^24 is exact in an f32, and no window is that wide"
)]
fn whole(pixels: u32) -> f32 {
    pixels as f32
}

/// A pixel position as the signed kind a canvas fills from.
fn signed(pixels: u32) -> i32 {
    i32::try_from(pixels).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colours::{button_ground, line_button, notice_ground, notice_ink, notice_line};
    use alo_value::Rgba;

    fn rect(x: u32, y: u32, width: u32, height: u32) -> PixelRect {
        PixelRect {
            x,
            y,
            width,
            height,
        }
    }

    fn lettering() -> Lettering {
        Lettering::compiled_in().expect("DejaVu Sans is compiled in")
    }

    #[test]
    fn the_button_is_128_by_24_twelve_in_from_the_lines_right_end_and_centred_down_it() {
        // The design's window at one: the line along its bottom.
        let line = rect(0, 788, 1280, 32);
        assert_eq!(placed(line, Scale::ONE), Some(rect(1140, 792, 128, 24)));
        assert_eq!(trail(line, rect(1140, 792, 128, 24), Scale::ONE), 140);
        // At two, every length doubled.
        let line = rect(0, 1576, 2560, 64);
        assert_eq!(placed(line, Scale::TWO), Some(rect(2280, 1584, 256, 48)));
        // At one and a half, each rounded as a renderer's canvas is: 192
        // by 36, 18 in, the line 48 tall.
        let line = rect(0, 1182, 1920, 48);
        assert_eq!(
            placed(line, Scale::ONE_AND_A_HALF),
            Some(rect(1710, 1188, 192, 36))
        );
    }

    #[test]
    fn a_line_that_cannot_hold_the_button_whole_has_none() {
        // 128 and 12 either side is 152: one narrower holds none.
        assert_eq!(
            placed(rect(0, 0, 152, 32), Scale::ONE),
            Some(rect(12, 4, 128, 24))
        );
        assert_eq!(placed(rect(0, 0, 151, 32), Scale::ONE), None);
        // A line cut short by a window lower than it holds none.
        assert_eq!(placed(rect(0, 0, 600, 23), Scale::ONE), None);
    }

    #[test]
    fn its_label_fits_inside_it_with_room_either_side_at_each_scale() {
        let lettering = lettering();
        for scale in [Scale::ONE, Scale::ONE_AND_A_HALF, Scale::TWO] {
            let label = alo_update::quitting::QUIT_AND_UPDATE;
            let room = whole(scale.device(WIDTH)) - 2.0 * whole(scale.device(GAP / 2));
            assert_eq!(lettering.fitted(label, room, scale), label, "{scale:?}");
        }
    }

    #[test]
    fn the_button_is_its_ring_its_ground_and_its_label_and_the_words_stop_before_it() {
        let lettering = lettering();
        let mut canvas = Canvas::new(400, 32, Rgba::WHITE);
        let line = rect(0, 0, 400, 32);
        let sentence = "alo 0.4.2 is ready. It installs when you quit alo, and this goes on";
        draw_line(
            &lettering,
            &mut canvas,
            line,
            (sentence, alo_update::quitting::QUIT_AND_UPDATE),
            Scale::ONE,
            (notice_line(), line_button()),
        );
        let at = |x: u32, y: u32| canvas.at(x, y).map(Rgba::to_rgba8);
        // The button at 260..388 by 4..28: its ring, then its ground.
        assert_eq!(at(260, 4), Some(notice_ink().to_rgba8()));
        assert_eq!(at(387, 27), Some(notice_ink().to_rgba8()));
        assert_eq!(at(261, 5), Some(button_ground().to_rgba8()));
        assert_eq!(at(259, 16), Some(notice_ground().to_rgba8()));
        assert_eq!(at(388, 16), Some(notice_ground().to_rgba8()));
        // Its label drawn on it.
        let inked = (262..386)
            .flat_map(|x| (6..26).map(move |y| (x, y)))
            .filter(|(x, y)| at(*x, *y) != Some(button_ground().to_rgba8()))
            .count();
        assert!(inked > 50, "no label: {inked} pixels");
        // The words, cut, end at least the gap before it: nothing but the
        // line's ground in the twelve columns before the button.
        for x in 248..260 {
            for y in 1..32 {
                assert_eq!(at(x, y), Some(notice_ground().to_rgba8()), "ink at {x},{y}");
            }
        }
    }

    #[test]
    fn a_line_too_narrow_for_the_button_is_drawn_as_any_line() {
        let lettering = lettering();
        let line = rect(0, 0, 140, 32);
        let mut with = Canvas::new(140, 32, Rgba::WHITE);
        draw_line(
            &lettering,
            &mut with,
            line,
            ("gone", "Quit and update"),
            Scale::ONE,
            (notice_line(), line_button()),
        );
        let mut plain = Canvas::new(140, 32, Rgba::WHITE);
        lettering.draw(&mut plain, line, "gone", Scale::ONE, notice_line());
        assert_eq!(with.pixels(), plain.pixels());
    }
}
