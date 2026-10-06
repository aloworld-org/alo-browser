/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The line that says what happened to a tab, drawn by the browser process.
//!
//! ADR 0005: *"The tab keeps the last frame it painted and says what
//! happened."* `alo-renderer`'s `tab.rs` makes the sentence; this puts it in
//! front of a person, along the bottom of the window and over the frame the
//! tab kept.
//!
//! # Why it is drawn here and not by a renderer
//!
//! Because it is about a renderer that is not answering: the one process that
//! cannot be asked to draw it is the one it is about. It is the browser's own
//! sentence — what `tab.rs` says, and a site's name in it — never a page's
//! markup or a page's title, which ADR 0024 § 4 keeps out of this process.
//!
//! # Why in a face of its own
//!
//! `DejaVu Sans`, compiled in, as the corpus is rendered: so that what the
//! window says looks the same on every machine and its reference render means
//! something, and so that drawing it never depends on what fonts this machine
//! happens to have.

use crate::colours::{notice_ground, notice_ink, notice_rule};
use crate::place::PixelRect;
use alo_paint::{Canvas, fill, outlined_run};
use alo_text::{Direction, Font, Slant, Weight, shape};

/// How big its letters are, in CSS pixels: alo's `--text-sm`.
pub const TEXT_SIZE: f32 = 13.0;

/// How far its words sit from the window's left edge, in CSS pixels.
pub const INSET: f32 = 12.0;

/// What a sentence too long for the window ends with.
const ELLIPSIS: &str = "\u{2026}";

/// The face the browser's own words are drawn in.
#[derive(Debug, Clone)]
pub struct Lettering {
    font: Font,
}

impl Lettering {
    /// `DejaVu Sans`, as compiled in.
    ///
    /// [`None`] only if the bytes compiled into this program are not a font,
    /// which is a broken build rather than anything a page could cause.
    pub fn compiled_in() -> Option<Self> {
        Font::load(
            "DejaVu Sans",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::sans::regular().to_vec(),
        )
        .map(|font| Self { font })
    }

    /// How wide a run of words is, in device pixels, at `replication`.
    fn width_of(&self, words: &str, replication: u32) -> f32 {
        shape(
            words,
            &self.font,
            size_at(replication),
            Direction::LeftToRight,
        )
        .width
    }

    /// As much of `sentence` as fits in `room` device pixels, ending in an
    /// ellipsis when it was cut.
    ///
    /// Cut at a character, never inside one, and from the end: the start of
    /// the sentence names the site, which is the part a person needs to know
    /// which tab it is about.
    pub fn fitted(&self, sentence: &str, room: f32, replication: u32) -> String {
        if self.width_of(sentence, replication) <= room {
            return sentence.to_owned();
        }
        let mut cut: Vec<char> = sentence.chars().collect();
        while !cut.is_empty() {
            cut.pop();
            let kept: String = cut.iter().collect();
            let candidate = format!("{}{ELLIPSIS}", kept.trim_end());
            if self.width_of(&candidate, replication) <= room {
                return candidate;
            }
        }
        String::new()
    }

    /// Draw the notice saying `sentence` into `rect` of `canvas`.
    ///
    /// The ground, a one-CSS-pixel rule along its top, and the sentence on one
    /// line, cut to fit.
    pub fn draw(&self, canvas: &mut Canvas, rect: PixelRect, sentence: &str, replication: u32) {
        if rect.is_empty() {
            return;
        }
        let (left, top) = (signed(rect.x), signed(rect.y));
        canvas.fill_rect(left, top, rect.width, rect.height, notice_ground());
        canvas.fill_rect(
            left,
            top,
            rect.width,
            replication.min(rect.height),
            notice_rule(),
        );

        let scale = whole(replication);
        let inset = INSET * scale;
        let room = whole(rect.width) - 2.0 * inset;
        if room <= 0.0 {
            return;
        }
        let words = self.fitted(sentence, room, replication);
        if words.is_empty() {
            return;
        }
        let size = size_at(replication);
        let metrics = self.font.metrics(size);
        // Centred on the line by the face's own extent above and below the
        // baseline, so the words sit in the middle whatever they are.
        let baseline = whole(rect.y)
            + (whole(rect.height) - (metrics.ascender + metrics.descender)) / 2.0
            + metrics.ascender;
        let path = outlined_run(
            &words,
            &self.font,
            size,
            0.0,
            (whole(rect.x) + inset, baseline),
        );
        let coverage = fill(&path);
        let (origin_x, origin_y) = coverage.origin();
        for y in 0..coverage.height() {
            for x in 0..coverage.width() {
                let amount = coverage.at(x, y);
                if amount == 0 {
                    continue;
                }
                let (Some(at_x), Some(at_y)) = (
                    origin_x
                        .checked_add_unsigned(x)
                        .and_then(|at| u32::try_from(at).ok()),
                    origin_y
                        .checked_add_unsigned(y)
                        .and_then(|at| u32::try_from(at).ok()),
                ) else {
                    continue;
                };
                // Kept to the notice: a descender must not reach the page.
                if at_y < rect.y || at_y >= rect.y + rect.height || at_x >= rect.x + rect.width {
                    continue;
                }
                canvas.blend(at_x, at_y, notice_ink(), amount);
            }
        }
    }
}

/// The text size in device pixels at a replication.
fn size_at(replication: u32) -> f32 {
    TEXT_SIZE * whole(replication)
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
    use alo_value::Rgba;

    fn lettering() -> Lettering {
        Lettering::compiled_in().expect("DejaVu Sans is compiled in")
    }

    #[test]
    fn a_sentence_that_fits_is_kept_whole() {
        let words = lettering().fitted("the renderer is gone", 1000.0, 1);
        assert_eq!(words, "the renderer is gone");
    }

    #[test]
    fn a_sentence_too_long_is_cut_at_its_end_with_an_ellipsis() {
        let lettering = lettering();
        let sentence = "the renderer for https://bank.example is gone: it said nothing";
        let words = lettering.fitted(sentence, 120.0, 1);
        assert!(words.ends_with(ELLIPSIS), "{words:?}");
        assert!(words.starts_with("the renderer"), "{words:?}");
        assert!(lettering.width_of(&words, 1) <= 120.0);
    }

    #[test]
    fn a_window_too_narrow_for_even_an_ellipsis_says_nothing_rather_than_overflowing() {
        assert_eq!(lettering().fitted("gone", 1.0, 1), "");
    }

    #[test]
    fn the_notice_is_its_ground_with_ink_on_it_and_a_rule_on_top() {
        let mut canvas = Canvas::new(300, 40, Rgba::WHITE);
        let rect = PixelRect {
            x: 0,
            y: 8,
            width: 300,
            height: 32,
        };
        lettering().draw(&mut canvas, rect, "gone", 1);
        assert_eq!(canvas.at(0, 7), Some(Rgba::WHITE), "it drew above itself");
        assert_eq!(
            canvas.at(0, 8).map(Rgba::to_rgba8),
            Some(notice_rule().to_rgba8())
        );
        assert_eq!(
            canvas.at(299, 39).map(Rgba::to_rgba8),
            Some(notice_ground().to_rgba8())
        );
        let inked = (0..300)
            .flat_map(|x| (9..40).map(move |y| (x, y)))
            .filter(|(x, y)| {
                canvas.at(*x, *y).map(Rgba::to_rgba8) != Some(notice_ground().to_rgba8())
            })
            .count();
        assert!(inked > 20, "no words were drawn: {inked} pixels");
    }

    #[test]
    fn the_words_stay_inside_the_notice() {
        let mut canvas = Canvas::new(60, 10, Rgba::WHITE);
        let rect = PixelRect {
            x: 0,
            y: 2,
            width: 60,
            height: 6,
        };
        lettering().draw(&mut canvas, rect, "gjpqy gjpqy", 1);
        for x in 0..60 {
            for y in [0, 1, 8, 9] {
                assert_eq!(canvas.at(x, y), Some(Rgba::WHITE), "ink at {x},{y}");
            }
        }
    }
}
