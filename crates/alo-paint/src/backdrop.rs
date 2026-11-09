/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What is drawn behind a box, blurred (queue item 885, ADR 0104).
//!
//! `backdrop-filter: blur(σ)` takes what the page drew before the box, inside
//! the box's border box, blurs it, and draws that under the box's own
//! background. This file is how it is blurred, fitted to what Google Chrome
//! 157 draws on its GPU path at one, two and eight (measured, not read from
//! its source; ADR 0104 has the numbers):
//!
//! - **The standard deviation is the length written**, times the scale, in
//!   device pixels — not half of it, as a shadow's blur radius is
//!   ([`crate::blur`]).
//! - **Only the crop is read**: the border box, its edges on whole device
//!   pixels as [`crop_of`] says. Past its edge the crop is **mirrored with
//!   the edge pixel repeated** (`abc|cba`), never the page beyond it: white
//!   inside and black just outside stays white to the edge.
//! - **Up to σ = 4 a Gaussian**, `exp(−i²/2σ²)` for `|i| ≤ ⌈3σ⌉`, normalised,
//!   across and then down.
//! - **Past 4, smaller first**: halved `L` times until `σ / 2^L ≤ 4`, each
//!   halving the mean of pixel pairs from the crop's own top left (an odd
//!   last pixel paired with itself), blurred there with the edge mirrored at
//!   that size, and grown back a halving at a time by bilinear sampling at
//!   `(x + ½) / 2 − ½`, clamped. Grown back in one step per axis here
//!   ([`Taps`]), the halvings' weights multiplied out: the same numbers,
//!   and a pixel computed alike whatever region it is asked in.
//! - In the colour the canvas holds, each channel on its own, premultiplied
//!   by alpha — the same as straight for the opaque pages measured.
//!
//! What it does not do: read a pixel outside the crop. So a box's blurred
//! backdrop depends on nothing but what was drawn inside its own crop before
//! it, which is what lets [`crate::damage`] say when it cannot have changed.

use crate::canvas::Canvas;
use crate::clip::{Clip, multiply};
use crate::coverage::Coverage;
use crate::kept::{Key, Made};
use crate::path::Path;
use crate::region::Region;
use alo_value::Rgba;

/// The largest standard deviation blurred at full size, in device pixels:
/// past it the crop is halved first.
pub const MOST_AT_FULL_SIZE: f32 = 4.0;

/// The largest standard deviation drawn at all, in device pixels: past it
/// the blur is a broken value rather than a picture, and is drawn as this.
pub const MOST: f32 = 1024.0;

/// One pixel, premultiplied: red, green, blue and alpha, each from 0 to 1.
type Pixel = [f32; 4];

/// A crop blurred, at the size it was blurred at: what a drawing keeps for
/// the next while what it was made from cannot have changed
/// ([`crate::kept`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Blurred {
    /// How large the crop is, in device pixels.
    size: (u32, u32),
    /// How many times it was halved before it was blurred.
    halvings: u32,
    /// How large it was when blurred.
    small: (u32, u32),
    /// Its pixels then, row by row.
    pixels: Vec<Pixel>,
}

impl Blurred {
    /// How many bytes it holds.
    pub fn bytes(&self) -> usize {
        self.pixels.len() * core::mem::size_of::<Pixel>()
    }

    /// How many times the crop was halved before it was blurred.
    pub fn halvings(&self) -> u32 {
        self.halvings
    }
}

/// The pixels a backdrop is read from and drawn into, on a canvas of
/// `size`, for a border box whose edges are `(left, top, right, bottom)` in
/// device pixels: each axis from its start rounded half up, as long as the
/// box rounded half up — but no longer than the box reaches past where its
/// rounded start is. Measured in Chrome 157 on 128 boxes at left and width
/// in eighths, at one and two: all 128, where the background's own rounding
/// of each edge on its own fits 118 ([`crate::snap`]). [`None`] when it
/// holds no pixel of the canvas.
pub fn crop_of(
    (left, top, right, bottom): (f32, f32, f32, f32),
    size: (u32, u32),
) -> Option<Region> {
    let axis = |start: f32, end: f32, limit: u32| -> Option<(u32, u32)> {
        if !(start.is_finite() && end.is_finite()) {
            return None;
        }
        let from = (start + 0.5).floor();
        let long = (end - start).min(end - from);
        let to = from + (long + 0.5).floor();
        let most = f32::from(u16::try_from(limit).unwrap_or(u16::MAX));
        let (from, to) = (from.clamp(0.0, most), to.clamp(0.0, most));
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "whole numbers clamped to 0..=65535"
        )]
        let whole = (from as u32, to as u32);
        Some(whole)
    };
    let (left, right) = axis(left, right, size.0)?;
    let (top, bottom) = axis(top, bottom, size.1)?;
    Region::new(left, top, right, bottom)
}

/// What a backdrop's blur is kept as from one drawing to the next
/// ([`crate::kept`]): its crop and its standard deviation in device pixels,
/// bit for bit — all it is made from but the pixels it reads, which
/// [`crate::damage`] answers for.
pub(crate) fn key(crop: Region, sigma: f32) -> Key {
    Key::of(
        Made::Backdrop,
        &[
            sigma.to_bits(),
            crop.left,
            crop.top,
            crop.right,
            crop.bottom,
        ],
        &Path::new(),
    )
}

/// `crop` of `canvas` blurred by a standard deviation of `sigma` device
/// pixels.
///
/// Computed a row at a time (queue item 894): the first halving read
/// straight from the canvas, never a premultiplied copy of the whole crop
/// made first, and each pass along rows of pixels held side by side. Every
/// number is the sum of the same products in the same order as pixel by
/// pixel, so every bit is the same (`reference`, which the tests hold this
/// to).
pub(crate) fn blurred(canvas: &Canvas, crop: Region, sigma: f32) -> Blurred {
    let size = (crop.right - crop.left, crop.bottom - crop.top);
    let sigma = if sigma.is_finite() {
        sigma.clamp(0.0, MOST)
    } else {
        0.0
    };
    let (mut pixels, mut small, mut halved, mut halvings) =
        if sigma > MOST_AT_FULL_SIZE && size.0 > 1 && size.1 > 1 {
            (
                halved_crop(canvas, crop),
                (size.0.div_ceil(2), size.1.div_ceil(2)),
                sigma / 2.0,
                1,
            )
        } else {
            (premultiplied_crop(canvas, crop), size, sigma, 0)
        };
    while halved > MOST_AT_FULL_SIZE && small.0 > 1 && small.1 > 1 {
        pixels = halve(&pixels, small);
        small = (small.0.div_ceil(2), small.1.div_ceil(2));
        halved /= 2.0;
        halvings += 1;
    }
    if halved > 0.0 {
        let weights = gaussian(halved);
        pixels = across(&pixels, small, &weights);
        pixels = down(&pixels, small, &weights);
    }
    Blurred {
        size,
        halvings,
        small,
        pixels,
    }
}

/// Row `y` of `canvas` from column `left` to `right`, when all of it is on
/// the canvas.
fn row_of(canvas: &Canvas, (left, right): (u32, u32), y: u32) -> Option<&[Rgba]> {
    if right > canvas.width() || y >= canvas.height() {
        return None;
    }
    let start = (y as usize) * (canvas.width() as usize);
    canvas
        .pixels()
        .get(start + left as usize..start + right as usize)
}

/// `crop` of `canvas`, every pixel premultiplied, row by row: a pixel off
/// the canvas transparent.
fn premultiplied_crop(canvas: &Canvas, crop: Region) -> Vec<Pixel> {
    let mut pixels = Vec::with_capacity(
        ((crop.right - crop.left) as usize) * ((crop.bottom - crop.top) as usize),
    );
    for y in crop.top..crop.bottom {
        match row_of(canvas, (crop.left, crop.right), y) {
            Some(row) => pixels.extend(row.iter().map(|color| premultiplied(*color))),
            None => pixels.extend(
                (crop.left..crop.right)
                    .map(|x| premultiplied(canvas.at(x, y).unwrap_or(Rgba::TRANSPARENT))),
            ),
        }
    }
    pixels
}

/// [`halve`] of [`premultiplied_crop`], read from the canvas without the
/// crop's copy: each pixel premultiplied as it is read, the same pairs
/// meaned in the same order.
fn halved_crop(canvas: &Canvas, crop: Region) -> Vec<Pixel> {
    let (width, height) = ((crop.right - crop.left), (crop.bottom - crop.top));
    let (half_width, half_height) = (width.div_ceil(2), height.div_ceil(2));
    let mut out = Vec::with_capacity((half_width as usize) * (half_height as usize));
    let read = |y: u32| -> Vec<Pixel> {
        match row_of(canvas, (crop.left, crop.right), crop.top + y) {
            Some(row) => row.iter().map(|color| premultiplied(*color)).collect(),
            None => (crop.left..crop.right)
                .map(|x| premultiplied(canvas.at(x, crop.top + y).unwrap_or(Rgba::TRANSPARENT)))
                .collect(),
        }
    };
    for y in 0..half_height {
        let upper = read((2 * y).min(height - 1));
        let lower = read((2 * y + 1).min(height - 1));
        halve_rows(&upper, &lower, half_width, &mut out);
    }
    out
}

/// Each pair of pixels across `upper` and `lower` made one, their mean, put
/// at the end of `out`: `half_width` of them, an odd last pixel paired with
/// itself.
fn halve_rows(upper: &[Pixel], lower: &[Pixel], half_width: u32, out: &mut Vec<Pixel>) {
    let last = upper.len().min(lower.len()).saturating_sub(1);
    let get = |row: &[Pixel], x: usize| row.get(x.min(last)).copied().unwrap_or_default();
    for x in 0..half_width as usize {
        let (one, two) = (get(upper, 2 * x), get(upper, 2 * x + 1));
        let (three, four) = (get(lower, 2 * x), get(lower, 2 * x + 1));
        let mut mean = [0.0; 4];
        for ((((value, one), two), three), four) in
            mean.iter_mut().zip(one).zip(two).zip(three).zip(four)
        {
            *value = f32::midpoint(f32::midpoint(one, two), f32::midpoint(three, four));
        }
        out.push(mean);
    }
}

/// Draw `blurred`, the blur of `crop`, onto `canvas` where `coverage` — the
/// border box with its radii, filled as its background is — covers, inside
/// the clip in force: what was there covered by it as much as the shape and
/// the clip together cover each pixel, as Chrome draws it. Only the pixels
/// a confined canvas keeps are computed.
pub(crate) fn draw(
    canvas: &mut Canvas,
    crop: Region,
    blurred: &Blurred,
    coverage: &Coverage,
    clip: Option<&Clip>,
) {
    let (left, top) = coverage.origin();
    let span = |start: i32, long: u32, limit: u32| {
        let clamp = |at: i64| u32::try_from(at.clamp(0, i64::from(limit))).unwrap_or(0);
        (
            clamp(i64::from(start)),
            clamp(i64::from(start) + i64::from(long)),
        )
    };
    let (from_x, to_x) = span(left, coverage.width(), canvas.width());
    let (from_y, to_y) = span(top, coverage.height(), canvas.height());
    let shape = Region::new(from_x, from_y, to_x, to_y);
    let Some(mut within) = shape.and_then(|shape| shape.meeting(crop)) else {
        return;
    };
    if let Some(confined) = canvas.confinement() {
        let Some(both) = within.meeting(confined) else {
            return;
        };
        within = both;
    }
    let local = Region {
        left: within.left - crop.left,
        top: within.top - crop.top,
        right: within.right - crop.left,
        bottom: within.bottom - crop.top,
    };
    let grown = grown_back(blurred, local);
    let width = (local.right - local.left) as usize;
    for (y, pixels) in (within.top..within.bottom).zip(grown.chunks_exact(width.max(1))) {
        // The shape's coverage along this row, from the column `within`
        // starts at: what `at` reads pixel by pixel.
        let row = i64::from(y) - i64::from(top);
        let column = i64::from(within.left) - i64::from(left);
        let covered = match (u32::try_from(row), usize::try_from(column)) {
            (Ok(row), Ok(column)) if row < coverage.height() => {
                let start = (row as usize) * (coverage.width() as usize);
                coverage
                    .data()
                    .get(start..start + coverage.width() as usize)
                    .and_then(|line| line.get(column..))
                    .unwrap_or(&[])
            }
            _ => &[],
        };
        for ((x, pixel), &covered) in (within.left..within.right).zip(pixels).zip(covered) {
            let inside = clip.map_or(u8::MAX, |clip| clip.at(x, y));
            let combined = multiply(covered, inside);
            if combined == 0 {
                continue;
            }
            canvas.blend(x, y, straight(*pixel), combined);
        }
    }
}

/// A colour premultiplied by its alpha.
fn premultiplied(color: Rgba) -> Pixel {
    let alpha = color.alpha.clamp(0.0, 1.0);
    [
        color.red * alpha,
        color.green * alpha,
        color.blue * alpha,
        alpha,
    ]
}

/// A premultiplied pixel as a colour.
fn straight([red, green, blue, alpha]: Pixel) -> Rgba {
    if alpha <= 0.0 {
        return Rgba::TRANSPARENT;
    }
    // Divided by one, a number is itself (queue item 894): an opaque page's
    // every pixel, written without the division.
    if alpha.to_bits() == 1.0_f32.to_bits() && !(red.is_nan() || green.is_nan() || blue.is_nan()) {
        return Rgba {
            red,
            green,
            blue,
            alpha,
        };
    }
    Rgba {
        red: red / alpha,
        green: green / alpha,
        blue: blue / alpha,
        alpha: alpha.min(1.0),
    }
}

/// Each pair of pixels across and each pair down made one, their mean —
/// an odd last pixel paired with itself.
fn halve(pixels: &[Pixel], (width, height): (u32, u32)) -> Vec<Pixel> {
    let (half_width, half_height) = (width.div_ceil(2), height.div_ceil(2));
    let row = |y: u32| {
        let start = (y.min(height - 1) as usize) * (width as usize);
        pixels.get(start..start + width as usize).unwrap_or(&[])
    };
    let mut out = Vec::with_capacity((half_width as usize) * (half_height as usize));
    for y in 0..half_height {
        halve_rows(row(2 * y), row(2 * y + 1), half_width, &mut out);
    }
    out
}

/// The Gaussian of `sigma`, from its middle out: `exp(−i²/2σ²)` for
/// `0 ≤ i ≤ ⌈3σ⌉`, all of it, both sides, adding to one.
fn gaussian(sigma: f32) -> Vec<f32> {
    let reach = (3.0 * sigma).ceil().max(1.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "sigma is at most MOST_AT_FULL_SIZE here, so the reach is a few pixels"
    )]
    let reach = reach as usize;
    let mut weights: Vec<f32> = (0..=reach)
        .map(|i| {
            #[expect(clippy::cast_precision_loss, reason = "a few pixels")]
            let i = i as f32;
            (-(i * i) / (2.0 * sigma * sigma)).exp()
        })
        .collect();
    let total: f32 =
        weights.iter().skip(1).sum::<f32>() * 2.0 + weights.first().copied().unwrap_or(1.0);
    for weight in &mut weights {
        *weight /= total;
    }
    weights
}

/// Where position `at` falls inside `0..length` once the edge is mirrored
/// with its last pixel repeated, as often as it takes.
fn mirrored(at: i64, length: i64) -> usize {
    if length <= 0 {
        return 0;
    }
    let period = 2 * length;
    let folded = at.rem_euclid(period);
    let inside = if folded < length {
        folded
    } else {
        period - 1 - folded
    };
    usize::try_from(inside).unwrap_or(0)
}

/// Each row blurred across by `weights`, the edge mirrored: where every tap
/// is inside the row, read from it directly; nearer the edge, mirrored.
fn across(pixels: &[Pixel], (width, height): (u32, u32), weights: &[f32]) -> Vec<Pixel> {
    let length = i64::from(width);
    let reach = weights.len().saturating_sub(1);
    let mut out = Vec::with_capacity(pixels.len());
    for y in 0..height as usize {
        let row = pixels
            .get(y * width as usize..(y + 1) * width as usize)
            .unwrap_or(&[]);
        for (x, at) in (0..length).zip(0_usize..) {
            if at >= reach && at + reach < row.len() {
                out.push(convolved(weights, |offset| {
                    row.get(at.wrapping_add_signed(isize::try_from(offset).unwrap_or(0)))
                        .copied()
                }));
            } else {
                out.push(convolved(weights, |offset| {
                    row.get(mirrored(x + offset, length)).copied()
                }));
            }
        }
    }
    out
}

/// Each column blurred down by `weights`, the edge mirrored: a row of the
/// result at a time, each tap's row added across it — for every pixel the
/// same products added in the same order as down its column.
fn down(pixels: &[Pixel], (width, height): (u32, u32), weights: &[f32]) -> Vec<Pixel> {
    let length = i64::from(height);
    let width = width as usize;
    let mut out = vec![[0.0; 4]; pixels.len()];
    let row = |at: usize| pixels.get(at * width..(at + 1) * width).unwrap_or(&[]);
    for (y, slots) in (0..length).zip(out.chunks_exact_mut(width.max(1))) {
        let mut add = |weight: f32, source: &[Pixel]| {
            for (slot, pixel) in slots.iter_mut().zip(source) {
                for (total, channel) in slot.iter_mut().zip(pixel) {
                    *total += weight * channel;
                }
            }
        };
        add(
            weights.first().copied().unwrap_or(1.0),
            row(mirrored(y, length)),
        );
        for (i, weight) in weights.iter().enumerate().skip(1) {
            let i = i64::try_from(i).unwrap_or(0);
            add(*weight, row(mirrored(y - i, length)));
            add(*weight, row(mirrored(y + i, length)));
        }
    }
    out
}

/// The weighted sum of the pixels `pixel` gives at each offset from the
/// middle, both ways.
fn convolved(weights: &[f32], pixel: impl Fn(i64) -> Option<Pixel>) -> Pixel {
    let mut sum = [0.0_f32; 4];
    let mut add = |weight: f32, value: Option<Pixel>| {
        if let Some(value) = value {
            for (total, channel) in sum.iter_mut().zip(value) {
                *total += weight * channel;
            }
        }
    };
    add(weights.first().copied().unwrap_or(1.0), pixel(0));
    for (i, weight) in weights.iter().enumerate().skip(1) {
        let i = i64::try_from(i).unwrap_or(0);
        add(*weight, pixel(-i));
        add(*weight, pixel(i));
    }
    sum
}

/// For each position of one axis at full size, the run of positions it is
/// grown back from at the blurred size, and their weights: the halvings'
/// bilinear samplings multiplied out.
#[derive(Debug, Clone, PartialEq)]
struct Taps {
    /// Each position's first source, and its weights from there on.
    of: Vec<(usize, Vec<f32>)>,
}

impl Taps {
    /// An axis `length` long at full size, halved `halvings` times.
    fn of(length: u32, halvings: u32) -> Self {
        let mut lengths = vec![length];
        for _ in 0..halvings {
            let last = lengths.last().copied().unwrap_or(1);
            lengths.push(last.div_ceil(2));
        }
        let of = (0..length as usize)
            .map(|at| {
                let mut weights: Vec<(usize, f32)> = vec![(at, 1.0)];
                for step in lengths.windows(2) {
                    let smaller = step.get(1).copied().unwrap_or(1);
                    let last = f64::from(smaller.saturating_sub(1));
                    let mut next: Vec<(usize, f32)> = Vec::new();
                    for (position, weight) in weights {
                        #[expect(
                            clippy::cast_precision_loss,
                            reason = "a position on a canvas, far under 2^52"
                        )]
                        let sample = (f64::midpoint(position as f64, 0.5) - 0.5).clamp(0.0, last);
                        let below = sample.floor();
                        #[expect(
                            clippy::cast_possible_truncation,
                            clippy::cast_sign_loss,
                            reason = "a whole, non-negative position clamped inside the axis"
                        )]
                        let first = below as usize;
                        let second = (first + 1).min(smaller.saturating_sub(1) as usize);
                        #[expect(
                            clippy::cast_possible_truncation,
                            reason = "a fraction between 0 and 1"
                        )]
                        let along = (sample - below) as f32;
                        for (source, share) in [(first, 1.0 - along), (second, along)] {
                            match next.iter_mut().find(|(each, _)| *each == source) {
                                Some((_, total)) => *total += weight * share,
                                None => next.push((source, weight * share)),
                            }
                        }
                    }
                    weights = next;
                }
                weights.sort_by_key(|(source, _)| *source);
                let first = weights.first().map_or(0, |(source, _)| *source);
                let mut run = Vec::new();
                for (source, weight) in weights {
                    while first + run.len() < source {
                        run.push(0.0);
                    }
                    run.push(weight);
                }
                (first, run)
            })
            .collect();
        Self { of }
    }
}

/// `blurred` grown back to full size over `local`, a region of the crop:
/// its pixels row by row. Each is computed by the same sums whatever
/// region it is asked in, so a confined repaint grows back exactly the
/// pixels drawing it whole does — and a row at a time (queue item 894),
/// each tap's row added across the row it grows, in the order the taps are
/// listed, as pixel by pixel.
fn grown_back(blurred: &Blurred, local: Region) -> Vec<Pixel> {
    let width = (local.right - local.left) as usize;
    let height = (local.bottom - local.top) as usize;
    if blurred.halvings == 0 {
        let mut out = Vec::with_capacity(width * height);
        let full = blurred.size.0 as usize;
        for y in local.top..local.bottom {
            let start = (y as usize) * full;
            match blurred
                .pixels
                .get(start + local.left as usize..start + local.right as usize)
            {
                Some(row) => out.extend_from_slice(row),
                None => out.extend((local.left..local.right).map(|x| {
                    blurred
                        .pixels
                        .get(start + x as usize)
                        .copied()
                        .unwrap_or_default()
                })),
            }
        }
        return out;
    }
    let across_taps = Taps::of(blurred.size.0, blurred.halvings);
    let down_taps = Taps::of(blurred.size.1, blurred.halvings);
    let small_width = blurred.small.0 as usize;
    let rows: Vec<&(usize, Vec<f32>)> = (local.top..local.bottom)
        .filter_map(|y| down_taps.of.get(y as usize))
        .collect();
    let first_row = rows.iter().map(|(first, _)| *first).min().unwrap_or(0);
    let last_row = rows
        .iter()
        .map(|(first, run)| first + run.len())
        .max()
        .unwrap_or(first_row);
    let columns: Vec<(usize, &[f32])> = (local.left..local.right)
        .map(|x| {
            across_taps
                .of
                .get(x as usize)
                .map_or((0, &[][..]), |(first, run)| (*first, run.as_slice()))
        })
        .collect();
    // Across first, for every small row a pixel asked for is grown from.
    let mut grown_across = Vec::with_capacity((last_row - first_row) * width);
    for small_row in first_row..last_row {
        let source = blurred
            .pixels
            .get(small_row * small_width..(small_row + 1) * small_width)
            .unwrap_or(&[]);
        for &(first, run) in &columns {
            let mut sum = [0.0_f32; 4];
            let taps = source.get(first..).unwrap_or(&[]);
            for (weight, pixel) in run.iter().zip(taps) {
                for (total, channel) in sum.iter_mut().zip(pixel) {
                    *total += weight * channel;
                }
            }
            grown_across.push(sum);
        }
    }
    let mut out = vec![[0.0_f32; 4]; width * height];
    for ((first, run), slots) in rows.into_iter().zip(out.chunks_exact_mut(width.max(1))) {
        for (offset, weight) in run.iter().enumerate() {
            let row = first + offset - first_row;
            let Some(source) = grown_across.get(row * width..(row + 1) * width) else {
                continue;
            };
            for (slot, pixel) in slots.iter_mut().zip(source) {
                for (total, channel) in slot.iter_mut().zip(pixel) {
                    *total += weight * channel;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod reference;

#[cfg(test)]
mod tests {
    use super::*;

    fn grey(level: f32) -> Rgba {
        Rgba {
            red: level,
            green: level,
            blue: level,
            alpha: 1.0,
        }
    }

    /// A canvas `width` wide and `height` tall whose column `x` is `level(x)`.
    fn columns(width: u32, height: u32, level: impl Fn(u32) -> f32) -> Canvas {
        let mut canvas = Canvas::new(width, height, Rgba::BLACK);
        for y in 0..height {
            for x in 0..width {
                canvas.replace(x, y, |_| grey(level(x)));
            }
        }
        canvas
    }

    /// The red channel of a blurred crop grown back whole, along its middle
    /// row, in levels of 255.
    fn middle_row(blurred: &Blurred) -> Vec<f32> {
        let whole = Region::new(0, 0, blurred.size.0, blurred.size.1).expect("a crop");
        let grown = grown_back(blurred, whole);
        let row = (blurred.size.1 / 2) as usize;
        let width = blurred.size.0 as usize;
        grown
            .get(row * width..(row + 1) * width)
            .unwrap_or(&[])
            .iter()
            .map(|[red, ..]| red * 255.0)
            .collect()
    }

    #[test]
    fn a_crop_starts_rounded_and_is_as_long_as_the_box_reaches_past_that() {
        // Measured in Chrome 157 (m885's snapping probe).
        let size = (400, 400);
        assert_eq!(
            crop_of((10.0, 10.0, 98.0, 66.0), size),
            Region::new(10, 10, 98, 66)
        );
        // A start rounded up keeps the box's end: 10.5 to 30.375 is 11 to
        // 30, as long as 19.375 rounded.
        assert_eq!(
            crop_of((10.5, 0.0, 30.375, 4.0), size),
            Region::new(11, 0, 30, 4)
        );
        // A start rounded down is as long as the box: 10.25 by 20.5 is 10
        // to 31.
        assert_eq!(
            crop_of((10.25, 0.0, 30.75, 4.0), size),
            Region::new(10, 0, 31, 4)
        );
        assert_eq!(crop_of((-20.0, -20.0, -5.0, -5.0), size), None);
        assert_eq!(crop_of((f32::NAN, 0.0, 4.0, 4.0), size), None);
        assert_eq!(
            crop_of((390.0, 0.0, 420.0, 4.0), size),
            Region::new(390, 0, 400, 4),
            "cut at the canvas"
        );
    }

    #[test]
    fn up_to_four_a_line_spreads_as_chromes_gaussian() {
        // A one-pixel white line on black, blurred by σ = 2: Chrome 157 at
        // one drew 1, 2, 7, 17, 31, 45, 51, 45, 31, 17, 7, 2, 1 about it
        // (m885's line probe, `backdrop-filter: blur(2px)`), the Gaussian
        // to ⌈3σ⌉ = 6 either side, rounded.
        let canvas = columns(41, 9, |x| if x == 20 { 1.0 } else { 0.0 });
        let crop = Region::new(0, 0, 41, 9).expect("a crop");
        let blurred = blurred(&canvas, crop, 2.0);
        assert_eq!(blurred.halvings(), 0);
        let row = middle_row(&blurred);
        let drawn: Vec<u8> = row
            .get(14..27)
            .unwrap_or(&[])
            .iter()
            .map(|level| {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a level"
                )]
                let byte = (level + 0.5).floor() as u8;
                byte
            })
            .collect();
        assert_eq!(drawn, [1, 2, 7, 17, 31, 45, 51, 45, 31, 17, 7, 2, 1]);
        let total: f32 = row.iter().sum();
        assert!((total - 255.0).abs() < 0.01, "nothing is lost: {total}");
    }

    #[test]
    fn past_four_it_is_halved_first_and_grown_back() {
        let canvas = columns(64, 12, |x| if x < 32 { 1.0 } else { 0.0 });
        let crop = Region::new(0, 0, 64, 12).expect("a crop");
        for (sigma, halvings) in [
            (4.0, 0),
            (6.0, 1),
            (8.0, 1),
            (12.0, 2),
            (16.0, 2),
            (24.0, 3),
        ] {
            assert_eq!(
                blurred(&canvas, crop, sigma).halvings(),
                halvings,
                "σ {sigma}"
            );
        }
        let row = middle_row(&blurred(&canvas, crop, 8.0));
        // An edge, softened symmetrically about its middle.
        for (x, (left, right)) in row.iter().zip(row.iter().rev()).take(32).enumerate() {
            let right = 255.0 - right;
            assert!((left - right).abs() < 0.01, "{x}: {left} against {right}");
        }
        assert!(
            row.windows(2)
                .all(|pair| matches!(pair, [one, two] if *one >= *two - 1e-4)),
            "{row:?}"
        );
    }

    #[test]
    fn the_crop_is_mirrored_at_its_edge_and_nothing_past_it_is_read() {
        // White inside a crop, black just outside it: the blur stays white
        // to the edge, as Chrome 157 drew it (m885's edge probe).
        let canvas = columns(60, 10, |x| if (10..50).contains(&x) { 1.0 } else { 0.0 });
        let crop = Region::new(10, 0, 50, 10).expect("a crop");
        for sigma in [3.0, 8.0] {
            let row = middle_row(&blurred(&canvas, crop, sigma));
            assert!(
                row.iter().all(|level| (level - 255.0).abs() < 0.01),
                "{row:?}"
            );
        }
        // A stripe at the edge is mirrored: a blur of `abc|cba`.
        let canvas = columns(40, 6, |x| if x < 4 { 1.0 } else { 0.0 });
        let crop = Region::new(0, 0, 40, 6).expect("a crop");
        let row = middle_row(&blurred(&canvas, crop, 3.0));
        let mirrored = columns(80, 6, |x| if (36..44).contains(&x) { 1.0 } else { 0.0 });
        let whole = Region::new(0, 0, 80, 6).expect("a crop");
        let reference = middle_row(&blurred(&mirrored, whole, 3.0));
        for (x, (level, expected)) in row
            .iter()
            .zip(reference.iter().skip(40))
            .take(20)
            .enumerate()
        {
            assert!(
                (level - expected).abs() < 0.01,
                "{x}: {level} against {expected}"
            );
        }
    }

    #[test]
    fn a_region_grown_back_is_every_bit_what_the_whole_grows_back() {
        let canvas = columns(53, 37, |x| {
            f32::from(u8::try_from((x * 37) % 11).unwrap_or(0)) / 10.0
        });
        let crop = Region::new(2, 3, 51, 36).expect("a crop");
        for sigma in [1.5, 6.0, 13.0] {
            let blurred = blurred(&canvas, crop, sigma);
            let (width, height) = blurred.size;
            let whole = grown_back(&blurred, Region::new(0, 0, width, height).expect("whole"));
            let part = Region::new(7, 5, 30, 21).expect("a part");
            let grown = grown_back(&blurred, part);
            let mut grown = grown.iter();
            for y in part.top..part.bottom {
                for x in part.left..part.right {
                    let one = whole
                        .get((y * width + x) as usize)
                        .expect("a pixel of the whole");
                    let other = grown.next().expect("a pixel of the part");
                    assert!(
                        one.iter()
                            .zip(other)
                            .all(|(a, b)| a.to_bits() == b.to_bits()),
                        "σ {sigma} at {x}, {y}"
                    );
                }
            }
            assert!(grown.next().is_none(), "no more than the part");
        }
    }

    #[test]
    fn no_blur_and_a_translucent_crop_come_back_as_they_were() {
        let mut canvas = Canvas::new(8, 8, Rgba::TRANSPARENT);
        canvas.replace(3, 3, |_| Rgba::from_rgba8(200, 100, 50, 128));
        let crop = Region::new(0, 0, 8, 8).expect("a crop");
        let blurred = blurred(&canvas, crop, 0.0);
        let grown = grown_back(&blurred, crop);
        let back = straight(grown.get(3 * 8 + 3).copied().expect("a pixel"));
        assert_eq!(back.to_rgba8(), (200, 100, 50, 128));
        assert_eq!(
            straight(grown.first().copied().expect("a pixel")),
            Rgba::TRANSPARENT
        );
        assert_eq!(blurred.bytes(), 64 * 16);
    }
    /// A canvas of `size` whose every pixel is drawn from `seed`: some
    /// opaque, some translucent, some clear — what a page's crop holds.
    fn noise((width, height): (u32, u32), seed: u32) -> Canvas {
        let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            u8::try_from(state >> 24).unwrap_or(0)
        };
        let mut canvas = Canvas::new(width, height, Rgba::WHITE);
        for y in 0..height {
            for x in 0..width {
                let (red, green, blue) = (next(), next(), next());
                let alpha = match next() % 4 {
                    0 => 0,
                    1 => next(),
                    _ => u8::MAX,
                };
                canvas.replace(x, y, |_| Rgba::from_rgba8(red, green, blue, alpha));
            }
        }
        canvas
    }

    fn bits(pixels: &[Pixel]) -> Vec<[u32; 4]> {
        pixels.iter().map(|pixel| pixel.map(f32::to_bits)).collect()
    }

    fn canvas_bits(canvas: &Canvas) -> Vec<[u32; 4]> {
        canvas
            .pixels()
            .iter()
            .map(|color| [color.red, color.green, color.blue, color.alpha].map(f32::to_bits))
            .collect()
    }

    const SIGMAS: [f32; 12] = [
        0.0, 0.5, 1.5, 3.0, 4.0, 4.01, 6.0, 8.0, 13.0, 16.0, 24.0, 40.0,
    ];

    #[test]
    fn the_blur_a_row_at_a_time_is_every_bit_the_blur_pixel_by_pixel() {
        // Queue item 894: crops of odd and even sizes, at the canvas's
        // edges and inside it, one or two pixels thin, and past the canvas,
        // at every kind of σ — no halving, one, several, more than the crop
        // can take.
        let canvas = noise((97, 61), 7);
        let crops = [
            (0, 0, 97, 61),
            (3, 5, 90, 58),
            (10, 10, 11, 40),
            (10, 10, 60, 12),
            (50, 20, 97, 61),
            (1, 1, 2, 2),
            (40, 30, 120, 90),
        ];
        for (left, top, right, bottom) in crops {
            let crop = Region::new(left, top, right, bottom).expect("a crop");
            for sigma in SIGMAS {
                let quick = blurred(&canvas, crop, sigma);
                let slow = reference::blurred(&canvas, crop, sigma);
                assert_eq!(
                    (quick.size, quick.halvings, quick.small),
                    (slow.size, slow.halvings, slow.small),
                    "σ {sigma} over {crop:?}"
                );
                assert_eq!(
                    bits(&quick.pixels),
                    bits(&slow.pixels),
                    "σ {sigma} over {crop:?}"
                );
                let (width, height) = quick.size;
                for local in [
                    Region::new(0, 0, width, height),
                    Region::new(width / 3, height / 4, width - width / 5, height - 1),
                    Region::new(width - 1, 0, width, height),
                ]
                .into_iter()
                .flatten()
                {
                    assert_eq!(
                        bits(&grown_back(&quick, local)),
                        bits(&reference::grown_back(&slow, local)),
                        "σ {sigma} over {crop:?}, grown over {local:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_backdrop_drawn_a_row_at_a_time_is_every_bit_as_drawn_pixel_by_pixel() {
        // Under a rounded shape partly off the canvas, through a clip and
        // without, whole and confined, over translucent pixels and opaque.
        let size = (83, 59);
        let page = noise(size, 11);
        let shapes = [
            crate::corner::rounded_rectangle(
                alo_layout::Rect::new(6.5, 4.25, 60.0, 44.0),
                crate::corner::Corners::all(12.0),
            ),
            crate::corner::rounded_rectangle(
                alo_layout::Rect::new(-20.0, 30.0, 70.0, 50.0),
                crate::corner::Corners::all(9.0),
            ),
        ];
        let clip = Clip::from(&crate::clip::mask(&crate::path::Path::rectangle(
            10.0, 0.0, 50.0, 40.5,
        )));
        for (index, shape) in shapes.iter().enumerate() {
            let coverage = crate::raster::fill_on_page(shape, crate::FillRule::NonZero, size);
            let crop = if index == 0 {
                Region::new(6, 4, 67, 48)
            } else {
                Region::new(0, 30, 50, 59)
            }
            .expect("a crop");
            for sigma in [2.0, 9.0, 16.0] {
                let made = blurred(&page, crop, sigma);
                for through in [None, Some(&clip)] {
                    for confined in [None, Region::new(20, 10, 45, 55)] {
                        let mut quick = page.clone();
                        let mut slow = page.clone();
                        quick.confine(confined);
                        slow.confine(confined);
                        draw(&mut quick, crop, &made, &coverage, through);
                        reference::draw(&mut slow, crop, &made, &coverage, through);
                        assert_eq!(
                            canvas_bits(&quick),
                            canvas_bits(&slow),
                            "shape {index}, σ {sigma}, clipped {}, confined {confined:?}",
                            through.is_some()
                        );
                    }
                }
            }
        }
    }
}
