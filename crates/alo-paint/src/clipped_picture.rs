/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A picture drawn inside a clip (queue item 717).
//!
//! A picture is drawn by [`crate::drawn_picture`] straight onto the canvas,
//! smoothed by its own arithmetic onto what is beneath ([`crate::mix_onto`]),
//! which knows nothing of clips. Until item 717 a picture therefore ignored
//! a clip in force: one inside `overflow: hidden` was drawn whole, and one in
//! the bottom of a card with rounded corners drew its own square corners
//! over the card's. The card the conversation shows a page in is such a
//! picture.
//!
//! So a picture under a clip is drawn as it always is, and then **each pixel
//! it could have touched is put back part of the way to what it was before**,
//! by as much as the clip leaves out: wholly outside the clip, the pixel is
//! what it was; wholly inside, what the picture made of it; on a rounded
//! clip's soft edge, between the two by the clip's coverage — which is what
//! a fill under the same clip gets (`render::draw_coverage` multiplies its
//! coverage by the clip's), since drawing over a colour and keeping a
//! fraction `c` of that is the colour drawn at coverage `c`.
//!
//! What it touches is the picture's rectangle under the transform in force,
//! in whole device pixels and one more each way for an outline's smoothed
//! edge. Only those pixels are copied first, never the page.
//!
//! Not measured against Chrome: Chrome clips a picture in its compositor by
//! the same rounded shape, and the card's reference render is alo's design,
//! not Chrome's (`docs/conformance.md`).

use crate::canvas::Canvas;
use crate::drawn_picture::draw_picture;
use crate::picture_filter::Uploads;
use alo_layout::Rect;
use alo_value::{Matrix, Rgba};
use std::sync::Arc;

/// Draw `picture` into `rect` under `transform`, as
/// [`crate::drawn_picture::draw_picture`] does, keeping of what it drew at
/// each pixel only as much as `inside` lets through — 255 all of it, 0 none.
pub fn draw_clipped_picture(
    target: &mut Canvas,
    picture: &Arc<Canvas>,
    rect: Rect,
    transform: Matrix,
    uploads: &mut Uploads,
    inside: &dyn Fn(u32, u32) -> u8,
) {
    let Some(touched) = touched(target, rect, transform) else {
        return;
    };
    let before: Vec<Rgba> = touched
        .pixels()
        .map(|(x, y)| target.at(x, y).unwrap_or(Rgba::TRANSPARENT))
        .collect();
    draw_picture(target, picture, rect, transform, uploads);
    for ((x, y), was) in touched.pixels().zip(before) {
        let kept = inside(x, y);
        if kept == u8::MAX {
            continue;
        }
        target.replace(x, y, |drawn| between(was, drawn, kept));
    }
}

/// A rectangle of whole pixels on the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Touched {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

impl Touched {
    /// Every pixel in it, row by row.
    fn pixels(self) -> impl Iterator<Item = (u32, u32)> {
        (self.top..self.bottom).flat_map(move |y| (self.left..self.right).map(move |x| (x, y)))
    }
}

/// The pixels a picture drawn into `rect` under `transform` can change on
/// `target`: its corners' bounds, a pixel wider each way, held to the
/// canvas — or [`None`] when that is nothing.
fn touched(target: &Canvas, rect: Rect, transform: Matrix) -> Option<Touched> {
    let corners = [
        transform.apply(rect.left(), rect.top()),
        transform.apply(rect.right(), rect.top()),
        transform.apply(rect.left(), rect.bottom()),
        transform.apply(rect.right(), rect.bottom()),
    ];
    let fold = |pick: fn(f32, f32) -> f32, axis: fn(&(f32, f32)) -> f32| {
        corners.iter().map(axis).reduce(pick)
    };
    let left = fold(f32::min, |corner| corner.0)?;
    let right = fold(f32::max, |corner| corner.0)?;
    let top = fold(f32::min, |corner| corner.1)?;
    let bottom = fold(f32::max, |corner| corner.1)?;
    let edge = |value: f32, limit: u32| pixel_at((value).clamp(0.0, 1.0e6), limit);
    let touched = Touched {
        left: edge(left.floor() - 1.0, target.width()),
        top: edge(top.floor() - 1.0, target.height()),
        right: edge(right.ceil() + 1.0, target.width()),
        bottom: edge(bottom.ceil() + 1.0, target.height()),
    };
    (touched.left < touched.right && touched.top < touched.bottom).then_some(touched)
}

/// A whole, non-negative pixel position, at most `limit`, without a
/// float-to-integer cast; a position that is not a number is the start.
fn pixel_at(value: f32, limit: u32) -> u32 {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    // Halving search: a canvas is a few thousand pixels across.
    let (mut low, mut high) = (0u32, limit);
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if f32::from(u16::try_from(middle).unwrap_or(u16::MAX)) <= value {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    low
}

/// `was`, moved towards `drawn` by `kept` of 255.
fn between(was: Rgba, drawn: Rgba, kept: u8) -> Rgba {
    let part = f32::from(kept) / 255.0;
    let mix = |from: f32, to: f32| (to - from).mul_add(part, from);
    Rgba {
        red: mix(was.red, drawn.red),
        green: mix(was.green, drawn.green),
        blue: mix(was.blue, drawn.blue),
        alpha: mix(was.alpha, drawn.alpha),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn red_square() -> Arc<Canvas> {
        Arc::new(Canvas::new(4, 4, Rgba::from_rgba8(255, 0, 0, 255)))
    }

    fn drawn(inside: &dyn Fn(u32, u32) -> u8) -> Canvas {
        let mut canvas = Canvas::new(8, 8, Rgba::WHITE);
        draw_clipped_picture(
            &mut canvas,
            &red_square(),
            Rect::new(2.0, 2.0, 4.0, 4.0),
            Matrix::IDENTITY,
            &mut Uploads::default(),
            inside,
        );
        canvas
    }

    fn at(canvas: &Canvas, x: u32, y: u32) -> Option<(u8, u8, u8, u8)> {
        canvas.at(x, y).map(Rgba::to_rgba8)
    }

    #[test]
    fn a_clip_that_lets_everything_through_changes_nothing() {
        let mut unclipped = Canvas::new(8, 8, Rgba::WHITE);
        draw_picture(
            &mut unclipped,
            &red_square(),
            Rect::new(2.0, 2.0, 4.0, 4.0),
            Matrix::IDENTITY,
            &mut Uploads::default(),
        );
        let clipped = drawn(&|_, _| u8::MAX);
        assert_eq!(clipped.pixels(), unclipped.pixels());
    }

    #[test]
    fn what_the_clip_leaves_out_is_what_was_there() {
        // A clip of the left half of the canvas.
        let canvas = drawn(&|x, _| if x < 4 { u8::MAX } else { 0 });
        assert_eq!(at(&canvas, 2, 3), Some((255, 0, 0, 255)));
        assert_eq!(at(&canvas, 3, 5), Some((255, 0, 0, 255)));
        assert_eq!(at(&canvas, 4, 3), Some((255, 255, 255, 255)));
        assert_eq!(at(&canvas, 5, 5), Some((255, 255, 255, 255)));
        // Outside the picture, untouched either way.
        assert_eq!(at(&canvas, 0, 0), Some((255, 255, 255, 255)));
    }

    #[test]
    fn a_soft_edge_keeps_part_of_the_picture_as_a_fill_would() {
        let canvas = drawn(&|_, _| 128);
        // Half of red over white: 255, and about half of the white kept.
        assert_eq!(at(&canvas, 3, 3), Some((255, 127, 127, 255)));
        let mut filled = Canvas::new(1, 1, Rgba::WHITE);
        filled.blend(0, 0, Rgba::from_rgba8(255, 0, 0, 255), 128);
        assert_eq!(at(&filled, 0, 0), at(&canvas, 3, 3));
    }

    #[test]
    fn what_is_touched_is_the_rectangle_and_a_pixel_more_on_the_canvas() {
        let canvas = Canvas::new(10, 10, Rgba::WHITE);
        assert_eq!(
            touched(&canvas, Rect::new(2.5, 3.0, 4.0, 2.0), Matrix::IDENTITY),
            Some(Touched {
                left: 1,
                top: 2,
                right: 8,
                bottom: 6
            })
        );
        assert_eq!(
            touched(
                &canvas,
                Rect::new(-50.0, -50.0, 500.0, 500.0),
                Matrix::IDENTITY
            ),
            Some(Touched {
                left: 0,
                top: 0,
                right: 10,
                bottom: 10
            })
        );
        assert_eq!(
            touched(&canvas, Rect::new(40.0, 0.0, 4.0, 4.0), Matrix::IDENTITY),
            None
        );
    }
}
