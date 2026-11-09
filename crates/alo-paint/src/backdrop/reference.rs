/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The backdrop's blur as queue item 885 first computed it, pixel by pixel
//! (ADR 0104): kept only for tests, which hold the quicker path of queue
//! item 894 to it every bit — the crop premultiplied whole, halved,
//! blurred and grown back, then drawn a pixel at a time.

use super::{Blurred, MOST, MOST_AT_FULL_SIZE, Pixel, Taps, gaussian, mirrored, premultiplied};
use crate::canvas::Canvas;
use crate::clip::{Clip, multiply};
use crate::coverage::Coverage;
use crate::region::Region;
use alo_value::Rgba;

/// `crop` of `canvas` blurred by a standard deviation of `sigma` device
/// pixels.
pub(super) fn blurred(canvas: &Canvas, crop: Region, sigma: f32) -> Blurred {
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
pub(super) fn draw(
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

/// `blurred` grown back to full size over `local`, a region of the crop:
/// its pixels row by row. Each is computed by the same sums whatever
/// region it is asked in, so a confined repaint grows back exactly the
/// pixels drawing it whole does.
pub(super) fn grown_back(blurred: &Blurred, local: Region) -> Vec<Pixel> {
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

/// A premultiplied pixel as a colour, divided at every alpha.
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
