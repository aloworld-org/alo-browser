/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `COLR` version 1 glyph's paint program carried out (queue item 785).
//!
//! The program ([`crate::colour_paint::Step`]) is run on a surface of the
//! glyph's own, just large enough, in device pixels: outlines and clip boxes
//! are put through the transforms in force and filled to coverage; a paint
//! fills what the outline waiting covers, or else what the clips let
//! through, each pixel asking its colour at its centre put back into the
//! font's coordinates; a layer is a surface of its own put onto the one
//! beneath by its compositing mode. What the program makes is then held and
//! drawn as a layered glyph is — premultiplied in eight bits, at the text
//! colour's alpha ([`crate::glyph_fade`], queue item 830).
//!
//! The compositing modes are the specification's: Porter and Duff's
//! operators on premultiplied colour, and the W3C blend modes — the
//! separable ones channel by channel, the four that are not (hue,
//! saturation, colour, luminosity) through luminosity and saturation as the
//! W3C's *Compositing and Blending* defines them.

use crate::canvas::Canvas;
use crate::colour_paint::{Fill, Mode, Step};
use crate::coverage::Coverage;
use crate::path::Path;
use crate::raster::fill;
use alo_value::Matrix;

/// A premultiplied colour, `[r, g, b, a]` from 0 to 1.
type Colour = [f32; 4];

/// Run `steps` — in font units, `y` up — with `placement` taking font units
/// to device pixels, and draw what they make onto `target` at `fade` of 255,
/// keeping at each pixel only as much as `inside` lets through.
pub fn draw_painted(
    target: &mut Canvas,
    steps: &[Step],
    placement: Matrix,
    fade: u8,
    inside: &dyn Fn(u32, u32) -> u8,
) {
    if fade == 0 {
        return;
    }
    let Some(area) = Area::reached(steps, placement) else {
        return;
    };
    let made = run(steps, placement, &area);
    let held: Vec<[u16; 4]> = made.iter().map(|colour| held(*colour)).collect();
    crate::glyph_fade::put_held(
        target,
        (area.left, area.top),
        area.width,
        &held,
        fade,
        inside,
    );
}

/// What a program makes, as coverage — its alpha in levels — in device
/// pixels: what a painted glyph casts as its shadow, as a layered glyph
/// casts what it draws ([`crate::glyph_shadow`], queue items 786 and 785).
pub fn alpha(steps: &[Step], placement: Matrix) -> Coverage {
    let Some(area) = Area::reached(steps, placement) else {
        return Coverage::empty();
    };
    let made = run(steps, placement, &area);
    let data = made.iter().map(|colour| {
        let [_, _, _, alpha] = held(*colour);
        u8::try_from(alpha).unwrap_or(u8::MAX)
    });
    Coverage::new(
        area.width,
        area.height,
        (area.left, area.top),
        data.collect(),
    )
}

/// Where a program reaches under `placement`, as `(left, top, right,
/// bottom)`; [`None`] when it reaches nothing.
pub fn reach(steps: &[Step], placement: Matrix) -> Option<(f32, f32, f32, f32)> {
    Area::reached(steps, placement).map(|area| {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a glyph's area, a few thousand pixels from the page's corner at most"
        )]
        let bounds = (
            area.left as f32,
            area.top as f32,
            (i64::from(area.left) + i64::from(area.width)) as f32,
            (i64::from(area.top) + i64::from(area.height)) as f32,
        );
        bounds
    })
}

/// The device pixels a program reaches: every outline and clip box under
/// the transforms in force when it was met.
#[derive(Debug, Clone, Copy)]
struct Area {
    left: i32,
    top: i32,
    width: u32,
    height: u32,
}

impl Area {
    fn reached(steps: &[Step], placement: Matrix) -> Option<Self> {
        let mut transforms = vec![placement];
        let mut reach: Option<(f32, f32, f32, f32)> = None;
        let mut widen = |bounds: Option<(f32, f32, f32, f32)>| {
            if let Some(bounds) = bounds {
                reach = Some(reach.map_or(bounds, |was| {
                    (
                        was.0.min(bounds.0),
                        was.1.min(bounds.1),
                        was.2.max(bounds.2),
                        was.3.max(bounds.3),
                    )
                }));
            }
        };
        for step in steps {
            let current = transforms.last().copied().unwrap_or(placement);
            match step {
                Step::Outline(path) => widen(path.transformed(current).bounds()),
                Step::PushClipBox(left, bottom, right, top) => {
                    widen(
                        clip_box(*left, *bottom, *right, *top)
                            .transformed(current)
                            .bounds(),
                    );
                }
                Step::PushTransform(numbers) => transforms.push(matrix(*numbers).then(current)),
                Step::PopTransform => {
                    transforms.pop();
                }
                _ => {}
            }
        }
        let (left, top, right, bottom) = reach?;
        let (left, top) = (left.floor().max(0.0), top.floor().max(0.0));
        let (right, bottom) = (right.ceil(), bottom.ceil());
        if !(right > left && bottom > top) || right - left > 8192.0 || bottom - top > 8192.0 {
            return None;
        }
        Some(Self {
            left: whole(left)?,
            top: whole(top)?,
            width: u32::try_from(whole(right - left)?).ok()?,
            height: u32::try_from(whole(bottom - top)?).ok()?,
        })
    }

    fn size(&self) -> usize {
        self.width as usize * self.height as usize
    }

    /// A coverage filled into this area's grid, as fractions.
    fn of(&self, coverage: &Coverage) -> Vec<f32> {
        let mut grid = vec![0.0_f32; self.size()];
        let (x, y) = coverage.origin();
        for down in 0..coverage.height() {
            for across in 0..coverage.width() {
                let (Some(column), Some(row)) = (
                    inside_of(
                        i64::from(x) + i64::from(across) - i64::from(self.left),
                        self.width,
                    ),
                    inside_of(
                        i64::from(y) + i64::from(down) - i64::from(self.top),
                        self.height,
                    ),
                ) else {
                    continue;
                };
                if let Some(cell) = grid.get_mut(row * self.width as usize + column) {
                    *cell = f32::from(coverage.at(across, down)) / 255.0;
                }
            }
        }
        grid
    }
}

/// Run the program on the area, and give back what the outermost surface
/// holds.
fn run(steps: &[Step], placement: Matrix, area: &Area) -> Vec<Colour> {
    let size = area.size();
    let mut transforms = vec![placement];
    let mut clips: Vec<Vec<f32>> = Vec::new();
    let mut layers: Vec<(Mode, Vec<Colour>)> = vec![(Mode::SourceOver, vec![[0.0; 4]; size])];
    let mut waiting: Option<Path> = None;
    for step in steps {
        let current = transforms.last().copied().unwrap_or(placement);
        match step {
            Step::Outline(path) => waiting = Some(path.transformed(current)),
            Step::PushClip => {
                let shape = waiting
                    .take()
                    .map_or_else(|| vec![0.0; size], |path| area.of(&fill(&path)));
                clips.push(within(shape, clips.last()));
            }
            Step::PushClipBox(left, bottom, right, top) => {
                let path = clip_box(*left, *bottom, *right, *top).transformed(current);
                clips.push(within(area.of(&fill(&path)), clips.last()));
            }
            Step::PopClip => {
                clips.pop();
            }
            Step::PushTransform(numbers) => transforms.push(matrix(*numbers).then(current)),
            Step::PopTransform => {
                transforms.pop();
            }
            Step::PushLayer(mode) => layers.push((*mode, vec![[0.0; 4]; size])),
            Step::PopLayer => {
                if layers.len() > 1
                    && let Some((mode, source)) = layers.pop()
                    && let Some((_, under)) = layers.last_mut()
                {
                    for (below, above) in under.iter_mut().zip(source) {
                        *below = composite(mode, above, *below);
                    }
                }
            }
            Step::Paint(paint) => {
                let shape = match waiting.take() {
                    Some(path) => within(area.of(&fill(&path)), clips.last()),
                    None => clips.last().cloned().unwrap_or_else(|| vec![1.0; size]),
                };
                let Some((_, surface)) = layers.last_mut() else {
                    continue;
                };
                paint_onto(surface, &shape, paint, current, area);
            }
        }
    }
    layers
        .into_iter()
        .next()
        .map(|(_, surface)| surface)
        .unwrap_or_default()
}

/// Fill `paint` over `surface` where `shape` covers, each pixel's colour
/// asked at its centre in the font's coordinates.
fn paint_onto(surface: &mut [Colour], shape: &[f32], paint: &Fill, current: Matrix, area: &Area) {
    let Some(back) = current.inverted() else {
        return;
    };
    let row = area.width as usize;
    for (index, (pixel, covered)) in surface.iter_mut().zip(shape).enumerate() {
        if *covered <= 0.0 {
            continue;
        }
        let (Ok(across), Ok(down)) = (i32::try_from(index % row), i32::try_from(index / row))
        else {
            continue;
        };
        #[expect(
            clippy::cast_precision_loss,
            reason = "a pixel of a glyph's area, a few thousand from the page's corner at most"
        )]
        let (x, y) = (
            (area.left + across) as f32 + 0.5,
            (area.top + down) as f32 + 0.5,
        );
        let (fx, fy) = back.apply(x, y);
        let colour = paint.at(fx, fy).map(|channel| channel * covered);
        *pixel = composite(Mode::SourceOver, colour, *pixel);
    }
}

/// A clip's shape inside the clip around it.
fn within(mut shape: Vec<f32>, outer: Option<&Vec<f32>>) -> Vec<f32> {
    if let Some(outer) = outer {
        for (cell, keep) in shape.iter_mut().zip(outer) {
            *cell *= keep;
        }
    }
    shape
}

/// A clip box as a path, in font units.
fn clip_box(left: f32, bottom: f32, right: f32, top: f32) -> Path {
    Path::rectangle(left, bottom, right - left, top - bottom)
}

/// The table's six numbers as a matrix.
fn matrix(numbers: [f32; 6]) -> Matrix {
    let [xx, yx, xy, yy, dx, dy] = numbers;
    Matrix {
        a: xx,
        b: yx,
        c: xy,
        d: yy,
        e: dx,
        f: dy,
    }
}

/// A premultiplied colour held as Chrome holds a glyph's picture: a level a
/// channel.
fn held(colour: Colour) -> [u16; 4] {
    colour.map(|channel| {
        let level = (channel.clamp(0.0, 1.0) * 255.0).round();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to zero..one and scaled, so a whole number in 0..=255"
        )]
        let level = level as u16;
        level
    })
}

/// A whole number of pixels as one, [`None`] past a page's reach.
fn whole(value: f32) -> Option<i32> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "checked to be a whole number within a page's reach first"
    )]
    let whole = (value.is_finite() && value.abs() < 1.0e7).then_some(value as i32);
    whole
}

/// `value` as an index under `limit`, if it is one.
fn inside_of(value: i64, limit: u32) -> Option<usize> {
    usize::try_from(value)
        .ok()
        .filter(|index| *index < limit as usize)
}

/// `source` put onto `under` by `mode`, both premultiplied.
fn composite(mode: Mode, source: Colour, under: Colour) -> Colour {
    let [_, _, _, sa] = source;
    let [_, _, _, da] = under;
    let each = |f: &dyn Fn(f32, f32) -> f32| -> Colour {
        [0, 1, 2, 3].map(|index| {
            f(
                source.get(index).copied().unwrap_or(0.0),
                under.get(index).copied().unwrap_or(0.0),
            )
        })
    };
    match mode {
        Mode::Clear => [0.0; 4],
        Mode::Source => source,
        Mode::Destination => under,
        Mode::SourceOver => each(&|s, d| d.mul_add(1.0 - sa, s)),
        Mode::DestinationOver => each(&|s, d| s.mul_add(1.0 - da, d)),
        Mode::SourceIn => each(&|s, _| s * da),
        Mode::DestinationIn => each(&|_, d| d * sa),
        Mode::SourceOut => each(&|s, _| s * (1.0 - da)),
        Mode::DestinationOut => each(&|_, d| d * (1.0 - sa)),
        Mode::SourceAtop => each(&|s, d| s.mul_add(da, d * (1.0 - sa))),
        Mode::DestinationAtop => each(&|s, d| d.mul_add(sa, s * (1.0 - da))),
        Mode::Xor => each(&|s, d| s.mul_add(1.0 - da, d * (1.0 - sa))),
        Mode::Plus => each(&|s, d| (s + d).min(1.0)),
        Mode::Hue | Mode::Saturation | Mode::Color | Mode::Luminosity => {
            blended(source, under, &|backdrop, top| {
                non_separable(mode, backdrop, top)
            })
        }
        _ => blended(source, under, &|backdrop, top| {
            [0, 1, 2].map(|index| {
                separable(
                    mode,
                    backdrop.get(index).copied().unwrap_or(0.0),
                    top.get(index).copied().unwrap_or(0.0),
                )
            })
        }),
    }
}

/// A blend mode's general form on premultiplied colours (W3C *Compositing
/// and Blending*): the blend where both are, each alone where the other is
/// not.
fn blended(
    source: Colour,
    under: Colour,
    blend: &dyn Fn([f32; 3], [f32; 3]) -> [f32; 3],
) -> Colour {
    let [sr, sg, sb, sa] = source;
    let [dr, dg, db, da] = under;
    let straight = |channels: [f32; 3], alpha: f32| {
        if alpha > 0.0 {
            channels.map(|channel| (channel / alpha).clamp(0.0, 1.0))
        } else {
            [0.0; 3]
        }
    };
    let mixed = blend(straight([dr, dg, db], da), straight([sr, sg, sb], sa));
    let channel = |index: usize, s: f32, d: f32| {
        let b = mixed.get(index).copied().unwrap_or(0.0);
        (sa * da).mul_add(b, s.mul_add(1.0 - da, d * (1.0 - sa)))
    };
    [
        channel(0, sr, dr),
        channel(1, sg, dg),
        channel(2, sb, db),
        sa + da - sa * da,
    ]
}

/// A separable blend mode on one straight channel: `backdrop` beneath,
/// `top` the source.
fn separable(mode: Mode, backdrop: f32, top: f32) -> f32 {
    let (cb, cs) = (backdrop, top);
    let hard_light = |cb: f32, cs: f32| {
        if cs <= 0.5 {
            cb * 2.0 * cs
        } else {
            let screen_with = 2.0f32.mul_add(cs, -1.0);
            cb + screen_with - cb * screen_with
        }
    };
    match mode {
        Mode::Multiply => cb * cs,
        Mode::Screen => cb + cs - cb * cs,
        Mode::Overlay => hard_light(cs, cb),
        Mode::Darken => cb.min(cs),
        Mode::Lighten => cb.max(cs),
        Mode::ColorDodge => {
            if cb <= 0.0 {
                0.0
            } else if cs >= 1.0 {
                1.0
            } else {
                (cb / (1.0 - cs)).min(1.0)
            }
        }
        Mode::ColorBurn => {
            if cb >= 1.0 {
                1.0
            } else if cs <= 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - cb) / cs).min(1.0)
            }
        }
        Mode::HardLight => hard_light(cb, cs),
        Mode::SoftLight => {
            if cs <= 0.5 {
                cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
            } else {
                let d = if cb <= 0.25 {
                    ((16.0 * cb - 12.0) * cb + 4.0) * cb
                } else {
                    cb.sqrt()
                };
                cb + (2.0 * cs - 1.0) * (d - cb)
            }
        }
        Mode::Difference => (cb - cs).abs(),
        Mode::Exclusion => cb + cs - 2.0 * cb * cs,
        _ => cs,
    }
}

/// The four blend modes that are not separable, on straight colours.
fn non_separable(mode: Mode, backdrop: [f32; 3], top: [f32; 3]) -> [f32; 3] {
    match mode {
        Mode::Hue => set_lum(set_sat(top, sat(backdrop)), lum(backdrop)),
        Mode::Saturation => set_lum(set_sat(backdrop, sat(top)), lum(backdrop)),
        Mode::Color => set_lum(top, lum(backdrop)),
        _ => set_lum(backdrop, lum(top)),
    }
}

fn lum([r, g, b]: [f32; 3]) -> f32 {
    0.3f32.mul_add(r, 0.59f32.mul_add(g, 0.11 * b))
}

fn clip_colour(colour: [f32; 3]) -> [f32; 3] {
    let l = lum(colour);
    let n = colour.iter().copied().fold(f32::MAX, f32::min);
    let x = colour.iter().copied().fold(f32::MIN, f32::max);
    colour.map(|c| {
        let mut c = c;
        if n < 0.0 && l - n > 0.0 {
            c = l + (c - l) * l / (l - n);
        }
        if x > 1.0 && x - l > 0.0 {
            c = l + (c - l) * (1.0 - l) / (x - l);
        }
        c
    })
}

fn set_lum(colour: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(colour);
    clip_colour(colour.map(|c| c + d))
}

fn sat(colour: [f32; 3]) -> f32 {
    let n = colour.iter().copied().fold(f32::MAX, f32::min);
    let x = colour.iter().copied().fold(f32::MIN, f32::max);
    x - n
}

fn set_sat(colour: [f32; 3], s: f32) -> [f32; 3] {
    let n = colour.iter().copied().fold(f32::MAX, f32::min);
    let x = colour.iter().copied().fold(f32::MIN, f32::max);
    if x - n <= 0.0 {
        return [0.0; 3];
    }
    colour.map(|c| (c - n) * s / (x - n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colour_paint::{ColourLine, Extend};
    use alo_value::Rgba;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// A square 0–100 of the font, filled; drawn at a tenth onto a canvas,
    /// `y` flipped, so it covers 0–10 by 0–10.
    fn square(fill: Fill) -> Vec<Step> {
        vec![
            Step::Outline(Path::rectangle(0.0, 0.0, 100.0, 100.0)),
            Step::PushClip,
            Step::Paint(fill),
            Step::PopClip,
        ]
    }

    fn placed() -> Matrix {
        Matrix {
            a: 0.1,
            b: 0.0,
            c: 0.0,
            d: -0.1,
            e: 0.0,
            f: 10.0,
        }
    }

    fn drawn(steps: &[Step]) -> Canvas {
        let mut canvas = Canvas::new(12, 12, Rgba::WHITE);
        draw_painted(&mut canvas, steps, placed(), u8::MAX, &|_, _| u8::MAX);
        canvas
    }

    fn at(canvas: &Canvas, x: u32, y: u32) -> (u8, u8, u8) {
        let (red, green, blue, _) = canvas.at(x, y).map_or((0, 0, 0, 0), Rgba::to_rgba8);
        (red, green, blue)
    }

    #[test]
    fn a_glyph_clipped_and_filled_solid_is_its_colour_inside_and_nothing_outside() {
        let canvas = drawn(&square(Fill::Solid(Rgba::from_rgba8(230, 57, 70, 255))));
        assert_eq!(at(&canvas, 5, 5), (230, 57, 70));
        assert_eq!(at(&canvas, 11, 5), (255, 255, 255));
    }

    #[test]
    fn a_linear_gradient_runs_across_the_glyph_in_the_fonts_coordinates() {
        let line = ColourLine {
            stops: vec![(0.0, Rgba::BLACK), (1.0, Rgba::WHITE)],
            extend: Extend::Pad,
        };
        let canvas = drawn(&square(Fill::Linear {
            points: [(0.0, 0.0), (100.0, 0.0), (0.0, 100.0)],
            line,
        }));
        // Pixel 2's centre is 25 units in: a quarter of the way to white.
        let (grey, ..) = at(&canvas, 2, 5);
        assert_eq!(grey, 64);
        let (grey, ..) = at(&canvas, 7, 1);
        assert_eq!(grey, 191);
    }

    #[test]
    fn a_transform_moves_what_is_painted_under_it() {
        let mut steps = vec![Step::PushTransform([1.0, 0.0, 0.0, 1.0, 20.0, 0.0])];
        steps.extend(square(Fill::Solid(Rgba::BLACK)));
        steps.push(Step::PopTransform);
        let canvas = drawn(&steps);
        assert_eq!(at(&canvas, 1, 5), (255, 255, 255), "moved 2 px right");
        assert_eq!(at(&canvas, 11, 5), (0, 0, 0));
    }

    #[test]
    fn a_layer_is_put_on_by_its_mode() {
        // Blue beneath, a red layer multiplied over its right half.
        let mut steps = square(Fill::Solid(Rgba::from_rgba8(0, 0, 255, 255)));
        steps.push(Step::PushLayer(Mode::Multiply));
        steps.extend([
            Step::Outline(Path::rectangle(50.0, 0.0, 50.0, 100.0)),
            Step::PushClip,
            Step::Paint(Fill::Solid(Rgba::from_rgba8(255, 0, 0, 255))),
            Step::PopClip,
        ]);
        steps.push(Step::PopLayer);
        let canvas = drawn(&steps);
        assert_eq!(at(&canvas, 2, 5), (0, 0, 255));
        assert_eq!(at(&canvas, 7, 5), (0, 0, 0), "red times blue");
    }

    #[test]
    fn porter_duff_and_blend_modes_are_the_specifications() {
        let red = [1.0, 0.0, 0.0, 1.0];
        let half_blue = [0.0, 0.0, 0.5, 0.5];
        let over = composite(Mode::SourceOver, half_blue, red);
        assert!(close(over[0], 0.5) && close(over[2], 0.5) && close(over[3], 1.0));
        let inside = composite(Mode::SourceIn, half_blue, red);
        assert!(close(inside[2], 0.5) && close(inside[3], 0.5));
        let out = composite(Mode::DestinationOut, half_blue, red);
        assert!(close(out[0], 0.5) && close(out[3], 0.5));
        let screen = composite(Mode::Screen, [0.5, 0.5, 0.5, 1.0], [0.5, 0.5, 0.5, 1.0]);
        assert!(close(screen[0], 0.75));
        let luminous = composite(Mode::Luminosity, [1.0, 1.0, 1.0, 1.0], red);
        assert!(
            close(luminous[0], 1.0) && close(luminous[1], 1.0),
            "{luminous:?}"
        );
    }
}
