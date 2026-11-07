/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A clip, as how much of each page pixel it lets through, and where two
//! clips let different amounts through (queue item 871, ADR 0101).
//!
//! [`crate::render()`] asks a clip one question, for one pixel at a time:
//! how much of this pixel it lets through ([`Clip::at`]). The clips open
//! around an item are combined pixel by pixel ([`Clip::intersected`]), so
//! what an item draws at a pixel depends on the clips only through each
//! one's mask *at that pixel*. That is what lets [`crate::damage`] say an
//! item drawn inside one clip and the same item drawn inside another draw
//! alike wherever the two masks are equal ([`differ`]): it is the same
//! arithmetic on the same numbers.

use crate::coverage::Coverage;
use crate::path::Path;
use crate::raster::fill;
use crate::region::Region;

/// A clip, as coverage over the whole page.
///
/// Held in page coordinates rather than relative to the clipping box, because
/// what it is asked is always "is this pixel inside" — and answering that from
/// a shape with its own origin means an offset at every lookup.
#[derive(Debug, Clone)]
pub(crate) struct Clip {
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl Clip {
    /// A clip letting through what `coverage` covers, and nothing outside it.
    pub(crate) fn from(coverage: &Coverage) -> Self {
        let (left, top) = coverage.origin();
        Self {
            left,
            top,
            width: coverage.width(),
            height: coverage.height(),
            data: coverage.data().to_vec(),
        }
    }

    /// How much of a page pixel this clip lets through.
    pub(crate) fn at(&self, x: u32, y: u32) -> u8 {
        let (Some(column), Some(row)) = (back(x, self.left), back(y, self.top)) else {
            return 0;
        };
        if column >= self.width || row >= self.height {
            return 0;
        }
        let index = (row as usize) * (self.width as usize) + (column as usize);
        self.data.get(index).copied().unwrap_or(0)
    }

    /// How many bytes it holds.
    pub(crate) fn bytes(&self) -> usize {
        self.data.len()
    }

    /// Where this clip and another, `inner`, overlap: over `inner`'s own
    /// pixels, each `inner`'s value times this clip's at the same page pixel,
    /// a pixel off the page's top or left nothing.
    pub(crate) fn intersected(&self, inner: &Self) -> Self {
        let mut data = Vec::with_capacity(inner.data.len());
        let width = inner.width as usize;
        for row in 0..inner.height {
            let values = inner
                .data
                .get(row as usize * width..(row as usize + 1) * width)
                .unwrap_or(&[]);
            // This clip's row at the same page row, if the row is on the page
            // and in this clip at all: everywhere else lets nothing through.
            let outer = crate::render::place(inner.top, row)
                .and_then(|y| back(y, self.top))
                .filter(|&outer_row| outer_row < self.height)
                .and_then(|outer_row| {
                    let from = outer_row as usize * self.width as usize;
                    self.data.get(from..from + self.width as usize)
                });
            let Some(outer) = outer else {
                data.extend(std::iter::repeat_n(0, values.len()));
                continue;
            };
            for (column, &value) in (0..inner.width).zip(values) {
                let through = crate::render::place(inner.left, column)
                    .and_then(|x| back(x, self.left))
                    .and_then(|outer_column| outer.get(outer_column as usize).copied())
                    .unwrap_or(0);
                data.push(multiply(value, through));
            }
        }
        Self {
            left: inner.left,
            top: inner.top,
            width: inner.width,
            height: inner.height,
            data,
        }
    }
}

/// The mask of a clip of `shape`, already under the transform in force: what
/// [`crate::render()`] makes for a clip pushed, and [`crate::damage`] for one
/// it compares, so the two can never make it differently.
pub(crate) fn mask(shape: &Path) -> Coverage {
    fill(shape)
}

/// Every pixel of a canvas of `size` where masks `one` and `other` let
/// different amounts through, as the smallest region holding them all, or
/// [`None`] where they let the same through everywhere on it. A pixel
/// outside a mask lets nothing through, as [`Clip::at`] says.
pub(crate) fn differ(one: &Coverage, other: &Coverage, size: (u32, u32)) -> Option<Region> {
    // Only the pixels either covers can differ: elsewhere both are nothing.
    let span = |mask: &Coverage| {
        let (left, top) = mask.origin();
        let (left, top) = (i64::from(left), i64::from(top));
        (
            left,
            top,
            left + i64::from(mask.width()),
            top + i64::from(mask.height()),
        )
    };
    let (a, b) = (span(one), span(other));
    let limit = |value: i64, most: u32| u32::try_from(value.clamp(0, i64::from(most))).unwrap_or(0);
    let (left, right) = (limit(a.0.min(b.0), size.0), limit(a.2.max(b.2), size.0));
    let (top, bottom) = (limit(a.1.min(b.1), size.1), limit(a.3.max(b.3), size.1));
    let mut found: Option<Region> = None;
    for y in top..bottom {
        let (row_one, row_other) = (row(one, y), row(other, y));
        let at = |row: Option<(i64, &[u8])>, x: u32| {
            row.and_then(|(from, values)| {
                usize::try_from(i64::from(x) - from)
                    .ok()
                    .and_then(|step| values.get(step).copied())
            })
            .unwrap_or(0)
        };
        let differs = |x: &u32| at(row_one, *x) != at(row_other, *x);
        let Some(first) = (left..right).find(differs) else {
            continue;
        };
        let last = (first..right).rev().find(differs).unwrap_or(first);
        if let Some(row) = Region::new(first, y, last + 1, y + 1) {
            found = Some(found.map_or(row, |was| was.joined(row)));
        }
    }
    found
}

/// The row of `mask` at page row `y`, with the page column it starts at, or
/// [`None`] where it has no such row.
fn row(mask: &Coverage, y: u32) -> Option<(i64, &[u8])> {
    let (left, top) = mask.origin();
    let step = usize::try_from(i64::from(y) - i64::from(top)).ok()?;
    let width = mask.width() as usize;
    let values = mask.data().get(step * width..(step + 1) * width)?;
    Some((i64::from(left), values))
}

/// Two coverages combined, as a fraction of a fraction.
pub(crate) fn multiply(left: u8, right: u8) -> u8 {
    let product = u32::from(left) * u32::from(right);
    // Rounded rather than truncated, so that full coverage stays full.
    u8::try_from((product + 127) / 255).unwrap_or(255)
}

/// A page position back into a mask's own coordinates.
fn back(position: u32, origin: i32) -> Option<u32> {
    u32::try_from(i64::from(position) - i64::from(origin)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f32, y: f32, side: f32) -> Coverage {
        mask(&Path::rectangle(x, y, side, side))
    }

    #[test]
    fn two_clips_differ_only_where_their_masks_do() {
        let tall = mask(&Path::rectangle(2.0, 2.0, 10.0, 20.0));
        let short = mask(&Path::rectangle(2.0, 2.0, 10.0, 12.0));
        assert_eq!(
            differ(&tall, &short, (40, 40)),
            Region::new(2, 14, 12, 22),
            "the rows the taller one goes on past the shorter"
        );
        assert_eq!(differ(&tall, &tall.clone(), (40, 40)), None);
        let half = mask(&Path::rectangle(2.0, 2.0, 10.0, 12.5));
        assert_eq!(
            differ(&short, &half, (40, 40)),
            Region::new(2, 14, 12, 15),
            "a half-covered row is a row that differs"
        );
    }

    #[test]
    fn what_differs_off_the_canvas_is_not_said() {
        assert_eq!(
            differ(&square(50.0, 50.0, 4.0), &square(60.0, 60.0, 4.0), (40, 40)),
            None
        );
        assert_eq!(
            differ(&square(-6.0, -6.0, 8.0), &square(-6.0, -6.0, 9.0), (40, 40)),
            Region::new(0, 0, 3, 3)
        );
    }

    #[test]
    fn an_intersection_is_each_pixel_of_the_inner_times_the_outer() {
        let outer = Clip::from(&mask(&Path::rectangle(0.0, 0.0, 6.5, 8.0)));
        let inner = Clip::from(&mask(&Path::rectangle(4.0, 2.0, 6.0, 3.0)));
        let both = outer.intersected(&inner);
        for y in 0..12 {
            for x in 0..12 {
                assert_eq!(
                    both.at(x, y),
                    multiply(inner.at(x, y), outer.at(x, y)),
                    "at {x}, {y}"
                );
            }
        }
        let off = Clip::from(&mask(&Path::rectangle(-3.0, -2.0, 6.0, 4.0)));
        let cut = outer.intersected(&off);
        assert_eq!(cut.at(0, 0), 255);
        assert_eq!(cut.at(4, 0), 0);
    }
}
