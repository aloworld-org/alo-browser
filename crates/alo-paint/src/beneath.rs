/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A picture drawn beneath the canvas rather than on it (queue item 869,
//! ADR 0100).
//!
//! The card the conversation shows a page in holds Chrome's newest frame of
//! the page, many times a second. Drawn as the card's picture, every frame
//! painted the whole first screen again, and an edit in the one box waited
//! behind that paint. So the first screen leaves the picture's place
//! **clear**, and whoever shows the canvas draws the picture under it: what
//! the canvas draws over the picture's place after it — a menu, the box,
//! the conversation's edges — still stands over the picture.
//!
//! Clear by as much as the picture would have covered: wholly inside its
//! rectangle and the clip in force, a pixel's alpha is nothing; on a rounded
//! clip's soft edge, its alpha is kept by what the clip leaves out. Its
//! colour is kept, since straight alpha weighs it by its alpha anyway. Over
//! an opaque picture that is exactly the colour the picture would have
//! made there: kept `1 − c` of what was under it, and `c` of the picture
//! ([`crate::clipped_picture`], which keeps a fraction `c` of what a
//! picture drew).

use crate::canvas::Canvas;
use crate::coverage::Coverage;

/// Make each pixel of `target` that `covered` covers clear by as much as
/// it is covered and `inside` lets through — 255 all of it, 0 none — its
/// colour kept.
pub fn clear(target: &mut Canvas, covered: &Coverage, inside: &dyn Fn(u32, u32) -> u8) {
    let (left, top) = covered.origin();
    for row in 0..covered.height() {
        for column in 0..covered.width() {
            let value = covered.at(column, row);
            if value == 0 {
                continue;
            }
            let (Some(x), Some(y)) = (step(left, column), step(top, row)) else {
                continue;
            };
            let through = u32::from(value) * u32::from(inside(x, y));
            if through == 0 {
                continue;
            }
            // How much of the pixel is left: what neither the picture nor
            // the clip covers.
            let kept = 1.0 - f32::from(u16::try_from(through).unwrap_or(u16::MAX)) / 65_025.0;
            target.replace(x, y, |was| alo_value::Rgba {
                alpha: was.alpha * kept.clamp(0.0, 1.0),
                ..was
            });
        }
    }
}

/// A page pixel `step` on from `base`, if it is on the page at all.
fn step(base: i32, step: u32) -> Option<u32> {
    let step = i32::try_from(step).ok()?;
    u32::try_from(base.checked_add(step)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::fill_on_page;
    use crate::{FillRule, Path};
    use alo_value::Rgba;

    fn blue() -> Rgba {
        Rgba::from_rgba8(0, 0, 255, 255)
    }

    #[test]
    fn a_pixel_covered_is_clear_its_colour_kept_and_one_beside_it_untouched() {
        let mut canvas = Canvas::new(6, 6, blue());
        let covered = fill_on_page(
            &Path::rectangle(1.0, 1.0, 3.0, 2.0),
            FillRule::NonZero,
            (6, 6),
        );
        clear(&mut canvas, &covered, &|_, _| u8::MAX);
        let at = |x, y| canvas.at(x, y).map(Rgba::to_rgba8);
        assert_eq!(at(1, 1), Some((0, 0, 255, 0)), "inside, clear");
        assert_eq!(at(3, 2), Some((0, 0, 255, 0)), "its far corner");
        assert_eq!(at(4, 2), Some((0, 0, 255, 255)), "beside it");
        assert_eq!(at(1, 3), Some((0, 0, 255, 255)), "under it");
        assert_eq!(at(0, 0), Some((0, 0, 255, 255)));
    }

    /// A picture of a stranger's colours: every pixel its own.
    fn noise(width: u32, height: u32) -> std::sync::Arc<Canvas> {
        let mut picture = Canvas::new(width, height, Rgba::WHITE);
        for y in 0..height {
            for x in 0..width {
                let byte = |seed: u32| u8::try_from((x * 37 + y * 101 + seed) % 256).unwrap_or(0);
                picture.replace(x, y, |_| {
                    Rgba::from_rgba8(byte(0), byte(85), byte(170), 255)
                });
            }
        }
        std::sync::Arc::new(picture)
    }

    /// A rounded clip's soft edge: let through less towards the right.
    fn soft(x: u32, _: u32) -> u8 {
        u8::try_from(255u32.saturating_sub(x.saturating_sub(6) * 40)).unwrap_or(0)
    }

    #[test]
    fn what_stands_over_a_clear_place_composed_over_the_picture_is_the_picture_drawn_in_place() {
        use crate::clipped_picture::draw_clipped_picture;
        use crate::drawn_picture::draw_picture;
        use crate::picture_filter::Uploads;
        use alo_layout::Rect;
        use alo_value::Matrix;
        let ground = Rgba::from_rgba8(240, 236, 228, 255);
        let over = |canvas: &mut Canvas| {
            // A translucent menu drawn over part of the picture after it.
            for y in 3..6 {
                for x in 1..9 {
                    canvas.blend(x, y, Rgba::from_rgba8(20, 140, 60, 255), 150);
                }
            }
        };
        let picture = noise(8, 8);
        let rect = Rect::new(2.0, 2.0, 8.0, 8.0);
        // Drawn in the canvas, as a card's picture was.
        let mut in_place = Canvas::new(12, 12, ground);
        draw_clipped_picture(
            &mut in_place,
            &picture,
            rect,
            Matrix::IDENTITY,
            &mut Uploads::default(),
            &soft,
            u8::MAX,
        );
        over(&mut in_place);
        // Its place left clear, and the picture drawn under the canvas.
        let mut clear_place = Canvas::new(12, 12, ground);
        let covered = fill_on_page(
            &Path::rectangle(2.0, 2.0, 8.0, 8.0),
            FillRule::NonZero,
            (12, 12),
        );
        clear(&mut clear_place, &covered, &soft);
        over(&mut clear_place);
        let mut under = Canvas::new(12, 12, Rgba::TRANSPARENT);
        draw_picture(
            &mut under,
            &picture,
            rect,
            Matrix::IDENTITY,
            &mut Uploads::default(),
        );
        under.draw_over(&clear_place, 1.0);
        for y in 0..12 {
            for x in 0..12 {
                let (Some(was), Some(now)) = (in_place.at(x, y), under.at(x, y)) else {
                    panic!("({x}, {y}) is on both canvases");
                };
                let (was, now) = (was.to_rgba8(), now.to_rgba8());
                let near = |a: u8, b: u8| a.abs_diff(b) <= 1;
                assert!(
                    near(was.0, now.0)
                        && near(was.1, now.1)
                        && near(was.2, now.2)
                        && near(was.3, now.3),
                    "({x}, {y}): drawn in place {was:?}, composed {now:?}"
                );
            }
        }
    }

    #[test]
    fn a_clip_keeps_what_it_leaves_out() {
        let mut canvas = Canvas::new(4, 1, blue());
        let covered = fill_on_page(
            &Path::rectangle(0.0, 0.0, 4.0, 1.0),
            FillRule::NonZero,
            (4, 1),
        );
        // Nothing, a quarter, half and all of each pixel let through.
        clear(&mut canvas, &covered, &|x, _| match x {
            0 => 0,
            1 => 64,
            2 => 128,
            _ => 255,
        });
        let alpha = |x| canvas.at(x, 0).map(|pixel| pixel.to_rgba8().3);
        assert_eq!(alpha(0), Some(255));
        assert_eq!(alpha(1), Some(191));
        assert_eq!(alpha(2), Some(127));
        assert_eq!(alpha(3), Some(0));
    }
}
