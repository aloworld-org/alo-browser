/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which pixels a shape's edge passes through, and where to sample them again.
//!
//! The rented rasteriser samples each pixel four rows by four columns, and
//! snaps where an edge crosses a row to a quarter of a pixel. That is too
//! coarse for a small curve: measured against the exact area (queue item
//! 459), the corner pixel of a square rounded 2.5 was covered 16 of 255
//! where its area is 35, and a dot twelve pixels across lost 0.64 of a pixel
//! of area and was drawn 111 on its left edge and 95 on its right. Its curves
//! are flattened into chords inside them, which loses the area, and the
//! quarter snap rounds both sides the same way rather than outwards, which
//! is the lean.
//!
//! Filling the same shape four times larger and averaging each four by four
//! block samples every pixel sixteen by sixteen, and that is within 3 of 255
//! of the exact area on both shapes and draws the dot's two edges alike. But
//! it costs sixteen times the pixels, and nearly every pixel of a shape is
//! wholly inside it or wholly outside, where the coarse answer is already
//! exact. So the coarse pass is kept and only **the tiles an edge passes
//! through are sampled again** — every partly covered pixel, which is the
//! only kind whose answer changes, and so with no seam where the two meet.
//!
//! This file decides which tiles and in what windows, from the coarse
//! coverage alone. The rasteriser that fills the windows is
//! [`crate::raster`]'s, and is the only one that knows what it rents.
//!
//! # Which pixels
//!
//! A pixel is sampled again when it is partly covered, or when a pixel
//! beside it (diagonally too) is covered differently: an edge the coarse
//! pass snapped onto a pixel's boundary leaves a whole pixel next to an
//! empty one and no partial pixel at all, and that pixel is where the edge
//! really is. Outside the coverage counts as empty. What the coarse pass
//! missed entirely — a sliver thinner than a quarter of a pixel, lying
//! between its sample rows — leaves nothing behind to find, and stays
//! missed, as it was.

/// How many times larger a window is filled: sixteen samples a pixel each
/// way, as four of the rented rasteriser's four.
pub const SCALE: u32 = 4;

/// The side of a tile, in pixels: the unit that is sampled again or not.
const TILE: u32 = 16;

/// The widest window, in tiles. A thousand and twenty-four pixels is four
/// thousand and ninety-six samples, so no window, however wide the shape,
/// asks for more than four by one thousand and twenty-four of them.
const RUN: u32 = 64;

/// The tallest window when windows are made coarse, in tiles.
const STRIP: u32 = 16;

/// How much work windows may ask for before they are made coarse: each
/// window fills the whole path again, so a path of many segments with an
/// edge in many places is filled in fewer, larger windows instead.
const BUDGET: usize = 1 << 22;

/// A rectangle of the coverage to sample again, in whole pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

/// The tiles an edge passes through, and the windows that cover them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    width: u32,
    height: u32,
    columns: u32,
    marked: Vec<bool>,
    windows: Vec<Window>,
}

impl Plan {
    /// The plan for coverage `width` by `height`, of a shape of `segments`
    /// segments.
    ///
    /// Coverage whose length does not match its size plans nothing.
    pub fn of(coverage: &[u8], width: u32, height: u32, segments: usize) -> Self {
        let columns = width.div_ceil(TILE);
        let rows = height.div_ceil(TILE);
        let size_matches = coverage.len() == (width as usize).saturating_mul(height as usize);
        let marked: Vec<bool> = (0..rows)
            .flat_map(|row| (0..columns).map(move |column| (column, row)))
            .map(|(column, row)| size_matches && has_an_edge(coverage, width, height, column, row))
            .collect();
        let mut plan = Self {
            width,
            height,
            columns,
            marked,
            windows: Vec::new(),
        };
        plan.windows = plan.runs();
        if plan
            .windows
            .len()
            .saturating_mul(segments.saturating_add(1))
            > BUDGET
        {
            plan.windows = plan.strips();
        }
        plan
    }

    /// The windows to fill again, none overlapping.
    pub fn windows(&self) -> &[Window] {
        &self.windows
    }

    /// Write a window's fine coverage — [`SCALE`] times its width by
    /// [`SCALE`] times its height — back into the coarse coverage, each
    /// block of samples averaged into its pixel.
    ///
    /// Only pixels of tiles an edge passes through are written: they are the
    /// same pixels whatever windows covered them, so the answer does not
    /// depend on how the work was divided.
    pub fn write(&self, window: Window, fine: &[u8], coverage: &mut [u8]) {
        let fine_width = (window.width as usize) * (SCALE as usize);
        for y in window.top..window.top.saturating_add(window.height).min(self.height) {
            for x in window.left..window.left.saturating_add(window.width).min(self.width) {
                if !self.is_marked(x / TILE, y / TILE) {
                    continue;
                }
                let mut sum: u32 = 0;
                for dy in 0..SCALE {
                    let row = ((y - window.top) * SCALE + dy) as usize;
                    let start = row * fine_width + ((x - window.left) * SCALE) as usize;
                    sum += fine
                        .get(start..start + SCALE as usize)
                        .map_or(0, |block| block.iter().map(|&value| u32::from(value)).sum());
                }
                let index = (y as usize) * (self.width as usize) + (x as usize);
                if let Some(pixel) = coverage.get_mut(index) {
                    *pixel = u8::try_from((sum + 8) / (SCALE * SCALE)).unwrap_or(u8::MAX);
                }
            }
        }
    }

    fn is_marked(&self, column: u32, row: u32) -> bool {
        let index = (row as usize) * (self.columns as usize) + (column as usize);
        column < self.columns && self.marked.get(index).copied().unwrap_or(false)
    }

    /// Each tile row's runs of marked tiles, no run wider than [`RUN`].
    fn runs(&self) -> Vec<Window> {
        let mut windows = Vec::new();
        for row in 0..self.height.div_ceil(TILE) {
            let mut column = 0;
            while column < self.columns {
                if !self.is_marked(column, row) {
                    column += 1;
                    continue;
                }
                let start = column;
                while column < self.columns && column - start < RUN && self.is_marked(column, row) {
                    column += 1;
                }
                windows.push(self.window(start, row, column, row + 1));
            }
        }
        windows
    }

    /// Cells of [`RUN`] by [`STRIP`] tiles holding a marked tile, each cut to
    /// the marked tiles' extent inside it.
    fn strips(&self) -> Vec<Window> {
        let mut windows = Vec::new();
        let rows = self.height.div_ceil(TILE);
        for cell_row in (0..rows).step_by(STRIP as usize) {
            for cell_column in (0..self.columns).step_by(RUN as usize) {
                let mut extent: Option<(u32, u32, u32, u32)> = None;
                for row in cell_row..(cell_row + STRIP).min(rows) {
                    for column in cell_column..(cell_column + RUN).min(self.columns) {
                        if self.is_marked(column, row) {
                            extent = Some(extent.map_or(
                                (column, row, column + 1, row + 1),
                                |(left, top, right, bottom)| {
                                    (
                                        left.min(column),
                                        top,
                                        right.max(column + 1),
                                        bottom.max(row + 1),
                                    )
                                },
                            ));
                        }
                    }
                }
                if let Some((left, top, right, bottom)) = extent {
                    windows.push(self.window(left, top, right, bottom));
                }
            }
        }
        windows
    }

    /// Tiles `left..right` across and `top..bottom` down, in pixels, cut to
    /// the coverage.
    fn window(&self, left: u32, top: u32, right: u32, bottom: u32) -> Window {
        let (x0, y0) = (left * TILE, top * TILE);
        Window {
            left: x0,
            top: y0,
            width: (right * TILE).min(self.width) - x0,
            height: (bottom * TILE).min(self.height) - y0,
        }
    }
}

/// Whether an edge passes through a tile: whether the tile, and the pixels
/// all round it, are anything other than all empty or all covered.
fn has_an_edge(coverage: &[u8], width: u32, height: u32, column: u32, row: u32) -> bool {
    let at = |x: i64, y: i64| -> u8 {
        if x < 0 || y < 0 || x >= i64::from(width) || y >= i64::from(height) {
            return 0;
        }
        let index = usize::try_from(y * i64::from(width) + x).unwrap_or(usize::MAX);
        coverage.get(index).copied().unwrap_or(0)
    };
    let (left, top) = (i64::from(column * TILE), i64::from(row * TILE));
    let first = at(left - 1, top - 1);
    if first != 0 && first != u8::MAX {
        return true;
    }
    (top - 1..=top + i64::from(TILE))
        .any(|y| (left - 1..=left + i64::from(TILE)).any(|x| at(x, y) != first))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Coverage `width` by `height`, covered wholly at and right of `edge`.
    fn stepped(width: u32, height: u32, edge: u32) -> Vec<u8> {
        (0..height)
            .flat_map(|_| (0..width).map(move |x| if x >= edge { 255 } else { 0 }))
            .collect()
    }

    #[test]
    fn nothing_with_no_edge_is_sampled_again() {
        assert!(Plan::of(&[0; 64 * 64], 64, 64, 4).windows().is_empty());
        // Covered all round, the middle tile has no edge in it.
        let plan = Plan::of(&[255; 48 * 48], 48, 48, 4);
        assert!(!plan.is_marked(1, 1) && plan.is_marked(0, 1));
    }

    #[test]
    fn a_whole_pixel_beside_outside_is_an_edge() {
        // A shape that fills its coverage exactly is still sampled where its
        // edge is, because outside it is empty.
        let plan = Plan::of(&[255; 20 * 20], 20, 20, 4);
        assert_eq!(
            plan.windows(),
            [
                Window {
                    left: 0,
                    top: 0,
                    width: 20,
                    height: 16
                },
                Window {
                    left: 0,
                    top: 16,
                    width: 20,
                    height: 4
                },
            ]
        );
    }

    #[test]
    fn an_edge_on_a_tiles_boundary_marks_the_tiles_either_side() {
        // A step at x 16, read in the middle tile row: the last empty column
        // is in the first tile, the first covered one in the second.
        let plan = Plan::of(&stepped(64, 48, 16), 64, 48, 4);
        assert!(plan.is_marked(0, 1) && plan.is_marked(1, 1));
        // The third is covered all round; the fourth meets the outside.
        assert!(!plan.is_marked(2, 1) && plan.is_marked(3, 1));
        let middle: Vec<Window> = plan
            .windows()
            .iter()
            .copied()
            .filter(|window| window.top == 16)
            .collect();
        assert_eq!(
            middle,
            [
                Window {
                    left: 0,
                    top: 16,
                    width: 32,
                    height: 16
                },
                Window {
                    left: 48,
                    top: 16,
                    width: 16,
                    height: 16
                },
            ]
        );
    }

    #[test]
    fn one_partial_pixel_marks_its_tile_and_the_tiles_it_touches() {
        let mut coverage = vec![0; 48 * 48];
        // Pixel (16, 16), the corner of the middle tile.
        if let Some(pixel) = coverage.get_mut(16 * 48 + 16) {
            *pixel = 100;
        }
        let plan = Plan::of(&coverage, 48, 48, 4);
        for row in 0..3 {
            for column in 0..3 {
                let touched = column <= 1 && row <= 1;
                assert_eq!(plan.is_marked(column, row), touched, "tile {column},{row}");
            }
        }
    }

    #[test]
    fn a_window_is_never_wider_than_a_thousand_and_twenty_four_pixels() {
        let width = TILE * RUN * 2 + 5;
        let plan = Plan::of(&vec![100; (width * 4) as usize], width, 4, 4);
        let widths: Vec<u32> = plan.windows().iter().map(|window| window.width).collect();
        assert_eq!(widths, [1024, 1024, 5]);
    }

    #[test]
    fn a_path_of_many_segments_is_sampled_in_fewer_windows() {
        // One tile in four covered: twenty runs a tile row.
        let width = TILE * 80;
        let coverage: Vec<u8> = (0..TILE * 4)
            .flat_map(|_| (0..width).map(|x| if (x / TILE) % 4 == 0 { 255 } else { 0 }))
            .collect();
        let few = Plan::of(&coverage, width, TILE * 4, 4);
        let many = Plan::of(&coverage, width, TILE * 4, BUDGET);
        assert_eq!(few.windows().len(), 4 * 20);
        assert_eq!(
            many.windows().len(),
            2,
            "one cell of 64 tiles and one of 16"
        );
        // Either way the same tiles are marked, and only they are written.
        assert_eq!(few.marked, many.marked);
    }

    #[test]
    fn a_window_writes_back_each_block_averaged_into_marked_pixels_only() {
        // Three tile columns: an edge in the first, the second covered all
        // round, the third meeting the outside.
        let mut coverage = stepped(48, 48, 8);
        let plan = Plan::of(&coverage, 48, 48, 4);
        assert!(plan.is_marked(0, 1) && !plan.is_marked(1, 1) && plan.is_marked(2, 1));
        let window = Window {
            left: 0,
            top: 0,
            width: 48,
            height: 48,
        };
        // Fine samples: every fourth sample column covered, a quarter of each
        // pixel.
        let fine: Vec<u8> = (0..48 * SCALE)
            .flat_map(|_| (0..48 * SCALE).map(|x| if x % 4 == 0 { 255 } else { 0 }))
            .collect();
        let before = coverage.clone();
        plan.write(window, &fine, &mut coverage);
        for y in 0..48 {
            for x in 0..48 {
                let index = (y * 48 + x) as usize;
                let wanted = if plan.is_marked(x / TILE, y / TILE) {
                    64
                } else {
                    before[index]
                };
                assert_eq!(coverage[index], wanted, "pixel {x},{y}");
            }
        }
    }

    #[test]
    fn coverage_that_is_not_its_size_plans_nothing() {
        assert!(Plan::of(&[100; 3], 4, 4, 4).windows().is_empty());
    }
}
