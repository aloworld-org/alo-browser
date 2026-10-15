/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The levels Chrome makes of a picture drawn smaller than it is on both
//! axes: its mipmap chain, and the copy of it Chrome uploads for a box
//! (queue item 179).
//!
//! Split from [`crate::picture_filter`], which decides which of these a
//! drawing samples and how it mixes them: this file is only what the levels
//! hold, and it changes when the way they are made does.
//!
//! # Measured
//!
//! Chrome 155 (`155.0.8059.41`, headless, its default raster path,
//! screenshots without `--disable-gpu`, the source not read), random-colour
//! pictures each in a file of its own and every pixel compared: the scratchpad
//! of iteration 367, `m179/`.
//!
//! 1. **A level halves each side, rounded down**, and is made as Skia makes
//!    one: two texels averaged on an even side, three weighted one, two, one
//!    on an odd side, and the sum **rounded down** to a level. Seven hundred
//!    stripes of random columns from 64 to 255 wide drawn between 1.4 and 5.6
//!    times smaller: 698 every pixel; rounding to the nearest instead missed
//!    across every even size, and plain pairs on an odd side across nearly
//!    every odd one.
//! 2. **Chrome uploads the largest level that still covers the box.** It
//!    halves both sides, **rounded up**, while the next level would be no
//!    smaller than the box on either axis, and the drawing samples that copy
//!    and its own chain ([`Level::uploaded`]). A 32 × 32 picture drawn 16 × 4
//!    is level one's 16 × 16 drawn at its own width — bilinear, not mipmapped,
//!    because nothing shrinks across.
//! 3. **On even sides that copy is the level itself.** Where an odd side is
//!    halved on the way, the copy is Chrome's own size, a texel larger than
//!    the level, and is Skia's level below it resampled to that size,
//!    bilinear and rounded to the nearest: 316 of 416 rows of odd stripes read
//!    at their own size are that exactly, and the rest within a level but for
//!    some 13, 21, 45 and 75 texels wide, which take another path. That copy
//!    is queue item 611's, which is what is built here until it is measured
//!    out.
//!
//! Every level is premultiplied, as Skia holds them: a texel is
//! premultiplied to a level first ([`crate::bilinear::premultiplied`]).

use crate::bilinear::{Texel, premultiplied};
use crate::canvas::Canvas;

/// One level of a picture: its texels premultiplied, row by row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    width: u32,
    height: u32,
    texels: Vec<Texel>,
}

impl Level {
    /// A picture as its first level, every texel premultiplied.
    pub fn of(picture: &Canvas) -> Self {
        let (width, height) = (picture.width(), picture.height());
        let texels = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .map(|(x, y)| {
                picture
                    .at(x, y)
                    .map_or([0; 4], |colour| premultiplied(colour.to_rgba8()))
            })
            .collect();
        Self {
            width,
            height,
            texels,
        }
    }

    /// Texels across.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Texels down.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The texel at `x` and `y`, or [`None`] off the level.
    pub fn at(&self, x: u32, y: u32) -> Option<Texel> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = u64::from(y) * u64::from(self.width) + u64::from(x);
        self.texels.get(usize::try_from(index).ok()?).copied()
    }

    /// The next level of the chain, as Skia makes it: each side halved and
    /// rounded down, never under one texel, each texel two, or one, two and
    /// one, of this level's on each axis, the sum rounded down. A level one
    /// texel square is its own next.
    #[must_use]
    pub fn halved(&self) -> Self {
        let (width, height) = ((self.width / 2).max(1), (self.height / 2).max(1));
        let mut texels = Vec::new();
        for y in 0..height {
            let (rows, below) = halving(self.height, y);
            for x in 0..width {
                let (columns, beside) = halving(self.width, x);
                let mut sum = [0_u64; 4];
                for (row, down) in &rows {
                    for (column, across) in &columns {
                        let texel = self.at(*column, *row).unwrap_or([0; 4]);
                        for (part, channel) in sum.iter_mut().zip(texel) {
                            *part += down * across * u64::from(channel);
                        }
                    }
                }
                texels.push(sum.map(|part| level(part / (below * beside))));
            }
        }
        Self {
            width,
            height,
            texels,
        }
    }

    /// This level resampled to `width` by `height`, bilinear and rounded to
    /// the nearest level, a half up: a texel's centre put back across this
    /// level, between the two texels either side, and past the first or the
    /// last texel's centre that texel alone. How Chrome makes a copy whose
    /// side is not the level's (queue item 611). Worked in whole numbers.
    #[must_use]
    pub fn resampled(&self, width: u32, height: u32) -> Self {
        let columns: Vec<_> = (0..width).map(|x| between(x, width, self.width)).collect();
        let rows: Vec<_> = (0..height)
            .map(|y| between(y, height, self.height))
            .collect();
        let mut texels = Vec::new();
        for (near_row, far_row, down, below) in &rows {
            for (near, far, across, beside) in &columns {
                let mut sum = [0_u64; 4];
                for (row, row_share) in [(near_row, below - down), (far_row, *down)] {
                    for (column, share) in [(near, beside - across), (far, *across)] {
                        let texel = self.at(*column, *row).unwrap_or([0; 4]);
                        for (part, channel) in sum.iter_mut().zip(texel) {
                            *part += row_share * share * u64::from(channel);
                        }
                    }
                }
                let whole = below * beside;
                texels.push(sum.map(|part| level((2 * part + whole) / (2 * whole))));
            }
        }
        Self {
            width,
            height,
            texels,
        }
    }

    /// The copy Chrome uploads to draw this picture into a box `across` by
    /// `down` device pixels: the picture halved, each side rounded up, for as
    /// long as the next would still be no smaller than the box on either
    /// axis. On even sides that is the level itself; where an odd side was
    /// halved, Skia's level below it resampled to Chrome's size. With it, how
    /// many halvings down it is.
    pub fn uploaded(&self, across: u32, down: u32) -> (u32, Self) {
        let (mut width, mut height) = (self.width, self.height);
        let mut steps = 0_u32;
        loop {
            let next = (width.div_ceil(2), height.div_ceil(2));
            if next == (width, height) || next.0 < across || next.1 < down {
                break;
            }
            (width, height) = next;
            steps += 1;
        }
        if steps == 0 {
            return (0, self.clone());
        }
        let mut below = self.clone();
        for _ in 1..steps {
            below = below.halved();
        }
        let level = below.halved();
        if (level.width, level.height) == (width, height) {
            (steps, level)
        } else {
            (steps, below.resampled(width, height))
        }
    }
}

/// The texels of a side `count` long that make texel `index` of its next
/// level, each with its weight, and the weights' sum: the texel alone on a
/// side one long, two on an even side, one, two and one on an odd side.
fn halving(count: u32, index: u32) -> (Vec<(u32, u64)>, u64) {
    let first = 2 * index;
    if count <= 1 {
        (vec![(0, 1)], 1)
    } else if count % 2 == 0 {
        (vec![(first, 1), (first + 1, 1)], 2)
    } else {
        (vec![(first, 1), (first + 1, 2), (first + 2, 1)], 4)
    }
}

/// Where texel `index` of `span` samples a side `count` long: the texel
/// before, the texel after, the second's share and the whole the shares are
/// out of. Held to the side's first and last texels.
fn between(index: u32, span: u32, count: u32) -> (u32, u32, u64, u64) {
    let whole = 2 * i64::from(span.max(1));
    let position = (2 * i64::from(index) + 1) * i64::from(count) - i64::from(span);
    let last = i64::from(count.max(1)) - 1;
    let before = position.div_euclid(whole);
    let rest = position.rem_euclid(whole);
    let (near, share) = if position < 0 {
        (0, 0)
    } else if before >= last {
        (last, 0)
    } else {
        (before, rest)
    };
    let index = |value: i64| u32::try_from(value).unwrap_or(0);
    let far = (near + 1).min(last);
    let whole = u64::try_from(whole).unwrap_or(1);
    let share = u64::try_from(share).unwrap_or(0);
    (index(near), index(far), share, whole)
}

/// A sum already divided down to a level.
fn level(value: u64) -> u8 {
    u8::try_from(value).unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_value::Rgba;

    /// A level of grey texels, one row after another.
    fn greys(width: u32, values: &[u8]) -> Level {
        Level {
            width,
            height: u32::try_from(values.len()).expect("small") / width,
            texels: values.iter().map(|v| [*v, *v, *v, 255]).collect(),
        }
    }

    fn row(level: &Level) -> Vec<u8> {
        (0..level.width())
            .map(|x| level.at(x, 0).expect("on it")[0])
            .collect()
    }

    #[test]
    fn an_even_side_averages_pairs_rounded_down() {
        // 10 and 13 make 11.5, which is 11; 200 and 201 make 200.
        let halved = greys(4, &[10, 13, 200, 201]).halved();
        assert_eq!((halved.width(), halved.height()), (2, 1));
        assert_eq!(row(&halved), [11, 200]);
    }

    #[test]
    fn an_odd_side_weighs_one_two_one() {
        // Five texels make two: (0 + 2·40 + 80) / 4 = 40 and
        // (80 + 2·100 + 255) / 4 = 133.75, which is 133.
        let halved = greys(5, &[0, 40, 80, 100, 255]).halved();
        assert_eq!(row(&halved), [40, 133]);
    }

    #[test]
    fn both_axes_are_halved_together_and_rounded_once() {
        // A 2 × 2 block of 1, 2, 2 and 2 is 7/4, which is 1 — not 1 then 2.
        let halved = greys(2, &[1, 2, 2, 2]).halved();
        assert_eq!((halved.width(), halved.height()), (1, 1));
        assert_eq!(row(&halved), [1]);
        // Three rows of an odd height, one, two, one: (0 + 2·4 + 255) / 4.
        let tall = greys(1, &[0, 4, 255]).halved();
        assert_eq!(tall.at(0, 0), Some([65, 65, 65, 255]));
    }

    #[test]
    fn a_single_texel_is_its_own_next_level() {
        let one = greys(1, &[77]);
        assert_eq!(one.halved(), one);
    }

    #[test]
    fn a_level_is_made_of_premultiplied_texels() {
        let mut canvas = Canvas::new(2, 1, Rgba::TRANSPARENT);
        canvas.blend(0, 0, Rgba::from_rgba8(200, 0, 0, 128), 255);
        let level = Level::of(&canvas);
        let (red, _, _, alpha) = canvas.at(0, 0).expect("on it").to_rgba8();
        assert_eq!(level.at(0, 0), Some(premultiplied((red, 0, 0, alpha))));
        assert_eq!(level.at(1, 0), Some([0, 0, 0, 0]));
        assert_eq!(level.at(2, 0), None);
        assert_eq!(level.at(0, 1), None);
    }

    #[test]
    fn a_resampled_texel_is_bilinear_rounded_to_the_nearest() {
        // Five texels to three, measured as two thirds and a third, the
        // middle alone, and a third and two thirds: (2·30 + 90) / 3 = 50,
        // 150, and (210 + 2·255) / 3 = 240.
        let level = greys(5, &[30, 90, 150, 210, 255]).resampled(3, 1);
        assert_eq!(row(&level), [50, 150, 240]);
        // Seven to four: 0.625 and 0.375 of 0 and 100 is 37.5, a half up.
        let seven = greys(7, &[0, 100, 100, 100, 100, 100, 100]).resampled(4, 1);
        assert_eq!(row(&seven).first(), Some(&38));
    }

    #[test]
    fn the_uploaded_copy_is_the_largest_level_covering_the_box() {
        let picture = greys(32, &[9; 32 * 32]);
        // 16 × 16 covers 16 × 4; 8 × 8 would not.
        let (steps, copy) = picture.uploaded(16, 4);
        assert_eq!(steps, 1);
        assert_eq!((copy.width(), copy.height()), (16, 16));
        // Nothing smaller covers a box as tall as the picture.
        assert_eq!(picture.uploaded(4, 32), (0, picture.clone()));
        // 32 to 4 square is three halvings.
        let (steps, small) = picture.uploaded(4, 4);
        assert_eq!(steps, 3);
        assert_eq!((small.width(), small.height()), (4, 4));
    }

    #[test]
    fn on_even_sides_the_copy_is_the_level_itself() {
        let picture = greys(4, &[0, 3, 100, 200, 0, 3, 100, 200]);
        assert_eq!(picture.uploaded(2, 1), (1, picture.halved()));
    }

    #[test]
    fn an_odd_side_makes_a_copy_a_texel_larger_than_the_level() {
        // Five across is three in Chrome's copy, where Skia's level is two;
        // the next halving, to two by one, would not cover two rows.
        let picture = greys(5, &[30, 90, 150, 210, 255].repeat(4));
        let (_, copy) = picture.uploaded(2, 2);
        assert_eq!((copy.width(), copy.height()), (3, 2));
        assert_eq!(row(&copy), [50, 150, 240]);
        assert_eq!(copy.at(2, 1), Some([240, 240, 240, 255]));
    }

    #[test]
    fn a_box_of_no_size_uploads_down_to_one_texel() {
        let (steps, copy) = greys(8, &[1; 64]).uploaded(0, 0);
        assert_eq!(steps, 3);
        assert_eq!((copy.width(), copy.height()), (1, 1));
    }
}
