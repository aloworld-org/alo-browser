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
pub(crate) fn blurred(canvas: &Canvas, crop: Region, sigma: f32) -> Blurred {
    let size = (crop.right - crop.left, crop.bottom - crop.top);
    let mut pixels = Vec::with_capacity((size.0 as usize) * (size.1 as usize));
    for y in crop.top..crop.bottom {
        for x in crop.left..crop.right {
            pixels.push(premultiplied(canvas.at(x, y).unwrap_or(Rgba::TRANSPARENT)));
        }
    }
    let sigma = if sigma.is_finite() {
        sigma.clamp(0.0, MOST)
    } else {
        0.0
    };
    let (mut small, mut halved, mut halvings) = (size, sigma, 0);
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
    for y in within.top..within.bottom {
        for x in within.left..within.right {
            let covered = at(coverage, x, y);
            let inside = clip.map_or(u8::MAX, |clip| clip.at(x, y));
            let combined = multiply(covered, inside);
            if combined == 0 {
                continue;
            }
            let index = ((y - within.top) as usize) * width + (x - within.left) as usize;
            let Some(pixel) = grown.get(index) else {
                continue;
            };
            canvas.blend(x, y, straight(*pixel), combined);
        }
    }
}

/// How much `coverage` covers the canvas's pixel `(x, y)`.
fn at(coverage: &Coverage, x: u32, y: u32) -> u8 {
    let (left, top) = coverage.origin();
    let (column, row) = (
        i64::from(x) - i64::from(left),
        i64::from(y) - i64::from(top),
    );
    match (u32::try_from(column), u32::try_from(row)) {
        (Ok(column), Ok(row)) => coverage.at(column, row),
        _ => 0,
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
    let get = |x: u32, y: u32| {
        pixels
            .get((y.min(height - 1) as usize) * (width as usize) + x.min(width - 1) as usize)
            .copied()
            .unwrap_or_default()
    };
    let mut out = Vec::with_capacity((half_width as usize) * (half_height as usize));
    for y in 0..half_height {
        for x in 0..half_width {
            let (one, two) = (get(2 * x, 2 * y), get(2 * x + 1, 2 * y));
            let (three, four) = (get(2 * x, 2 * y + 1), get(2 * x + 1, 2 * y + 1));
            let mut mean = [0.0; 4];
            for ((((value, one), two), three), four) in
                mean.iter_mut().zip(one).zip(two).zip(three).zip(four)
            {
                *value = f32::midpoint(f32::midpoint(one, two), f32::midpoint(three, four));
            }
            out.push(mean);
        }
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

/// Each row blurred across by `weights`, the edge mirrored.
fn across(pixels: &[Pixel], (width, height): (u32, u32), weights: &[f32]) -> Vec<Pixel> {
    let length = i64::from(width);
    let mut out = Vec::with_capacity(pixels.len());
    for y in 0..height as usize {
        let row = pixels
            .get(y * width as usize..(y + 1) * width as usize)
            .unwrap_or(&[]);
        for x in 0..length {
            out.push(convolved(weights, |offset| {
                row.get(mirrored(x + offset, length)).copied()
            }));
        }
    }
    out
}

/// Each column blurred down by `weights`, the edge mirrored.
fn down(pixels: &[Pixel], (width, height): (u32, u32), weights: &[f32]) -> Vec<Pixel> {
    let length = i64::from(height);
    let mut out = vec![[0.0; 4]; pixels.len()];
    for x in 0..width as usize {
        for y in 0..length {
            let value = convolved(weights, |offset| {
                pixels
                    .get(mirrored(y + offset, length) * width as usize + x)
                    .copied()
            });
            if let Some(slot) = usize::try_from(y)
                .ok()
                .and_then(|y| out.get_mut(y * width as usize + x))
            {
                *slot = value;
            }
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
/// pixels drawing it whole does.
fn grown_back(blurred: &Blurred, local: Region) -> Vec<Pixel> {
    let width = (local.right - local.left) as usize;
    let height = (local.bottom - local.top) as usize;
    if blurred.halvings == 0 {
        let mut out = Vec::with_capacity(width * height);
        for y in local.top..local.bottom {
            for x in local.left..local.right {
                out.push(
                    blurred
                        .pixels
                        .get((y as usize) * (blurred.size.0 as usize) + x as usize)
                        .copied()
                        .unwrap_or_default(),
                );
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
    // Across first, for every small row a pixel asked for is grown from.
    let mut grown_across = Vec::with_capacity((last_row - first_row) * width);
    for small_row in first_row..last_row {
        for x in local.left..local.right {
            let (first, run) = across_taps
                .of
                .get(x as usize)
                .map_or((0, &[][..]), |(first, run)| (*first, run.as_slice()));
            let mut sum = [0.0_f32; 4];
            for (offset, weight) in run.iter().enumerate() {
                if let Some(pixel) = blurred.pixels.get(small_row * small_width + first + offset) {
                    for (total, channel) in sum.iter_mut().zip(pixel) {
                        *total += weight * channel;
                    }
                }
            }
            grown_across.push(sum);
        }
    }
    let mut out = Vec::with_capacity(width * height);
    for (first, run) in rows {
        for column in 0..width {
            let mut sum = [0.0_f32; 4];
            for (offset, weight) in run.iter().enumerate() {
                let row = first + offset - first_row;
                if let Some(pixel) = grown_across.get(row * width + column) {
                    for (total, channel) in sum.iter_mut().zip(pixel) {
                        *total += weight * channel;
                    }
                }
            }
            out.push(sum);
        }
    }
    out
}

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
}
