/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which filter an upright picture goes through for the box it is drawn
//! into, and how two mipmap levels are mixed when it is drawn smaller on
//! both axes, as Chrome 155 does it (queue item 179).
//!
//! Split from [`crate::drawn_picture`], which decides which pixels a picture
//! covers, from [`crate::bilinear`], which samples one level, and from
//! [`crate::mip_level`], which makes the levels: this file is only the
//! choice and the mix.
//!
//! # The rule, measured
//!
//! Chrome 155 (`155.0.8059.41`, headless, its default raster path,
//! screenshots without `--disable-gpu`, the source not read), random-colour
//! pictures each in a file of its own, every pixel at one and at two
//! (iteration 367's scratchpad `m179/`):
//!
//! 1. **Bilinear unless both axes shrink.** A picture drawn no smaller than
//!    it is on either axis, in device pixels, is [`crate::bilinear`]'s alone,
//!    however much smaller it is on the other: a 64 × 4 picture drawn 2 × 4
//!    to 63 × 4, every pixel; 32 × 32 drawn 4 × 32 and 4 × 40, every pixel.
//! 2. **Both shrinking, the copy Chrome uploads** ([`Level::uploaded`]) is
//!    drawn: bilinear when the box is no smaller than the copy on an axis,
//!    otherwise mixed from two levels of the copy's own chain.
//! 3. **Which two, and how much of each, in 256ths of a level of detail.**
//!    The ratio is the larger of the copy's width over the box's and its
//!    height over the box's. Its logarithm is taken as a GPU takes it — the
//!    exponent, and the mantissa's log2 interpolated straight between its
//!    values at 1, 1.25, 1.5, 1.75 and 2 — rounded to the nearest 256th,
//!    less 120: [`detail`]. Its whole part names the nearer level and its
//!    256ths the farther's share. Nothing at or under none: bilinear. Of 773
//!    ratios measured — 700 random stripes drawn 1.4 to 5.6 times smaller and
//!    a 128-texel picture drawn 17 to 127 wide — 755 have exactly that
//!    share, and every other one lies within a thirtieth of a 256th of a
//!    half, where Chrome rounds either way: holding the mantissa or the
//!    table to fewer bits moved none of them. The plain logarithm, less
//!    Skia's 0.475, missed a share by one or two 256ths in most of them.
//! 4. **Mixed unrounded, held once.** Each level's bilinear mix, in 65,536ths
//!    of a level, is weighed by its share in 256ths and the sum held in
//!    sixteenths of a level, as one level's mix is ([`Mix::held`]). Holding
//!    each level's mix first missed 19,497 of 1,578,102 channels of a 128 ×
//!    128 picture drawn 17 to 127 wide where this misses 1,537 — the bottom
//!    rows of a picture 127 tall, whose vertical weight Chrome works out in
//!    floats.
//!
//! A translucent picture is mixed premultiplied, as its levels are; that is
//! measured for a picture drawn no smaller ([`crate::bilinear`]) and here
//! only for opaque ones (queue item 612).
//!
//! # What this does not decide
//!
//! A turned picture, which [`crate::drawn_picture`] samples through its
//! inverse transform, one texel at a time (queue item 613). Nor
//! `image-rendering` (601).

use crate::bilinear::{Mix, PARTS, Tap, mixed, taps};
use crate::canvas::Canvas;
use crate::mip_level::Level;
use std::sync::Arc;

/// A picture ready to be drawn into one box: what every pixel of the box
/// takes from it.
#[derive(Debug, Clone)]
pub struct Filtered {
    near: Sampled,
    far: Option<(Sampled, u32)>,
}

/// One level and where each column and row of the box samples it.
#[derive(Debug, Clone)]
struct Sampled {
    level: Level,
    columns: Vec<Tap>,
    rows: Vec<Tap>,
}

impl Sampled {
    fn new(level: Level, across: u32, down: u32) -> Self {
        let columns = taps(across, level.width());
        let rows = taps(down, level.height());
        Self {
            level,
            columns,
            rows,
        }
    }

    /// The mix for the box's pixel `x` across and `y` down, unrounded.
    fn parts(&self, x: u32, y: u32) -> Option<[u64; 4]> {
        let column = *self.columns.get(usize::try_from(x).ok()?)?;
        let row = *self.rows.get(usize::try_from(y).ok()?)?;
        mixed(|x, y| self.level.at(x, y), column, row)
    }
}

/// The copies of pictures one paint has uploaded, in the order it drew them,
/// and whether each has levels: Chrome keeps one decoded picture per file,
/// one copy of it per level it uploads, and whether a copy has mipmap levels
/// is decided by the first draw that uploads it.
#[derive(Debug, Default)]
pub struct Uploads {
    copies: Vec<Upload>,
}

/// One uploaded copy: its picture, how many halvings down it is, and
/// whether it has levels.
#[derive(Debug)]
struct Upload {
    picture: Arc<Canvas>,
    steps: u32,
    levelled: bool,
}

impl Uploads {
    /// The filter for `picture` drawn into a box `across` by `down` device
    /// pixels, after every draw this paint has already made.
    ///
    /// The copy drawn is the one Chrome uploads for the box
    /// ([`Level::uploaded`]). It is drawn through its levels when the box is
    /// smaller than it on both axes — and so, from then on, is every later
    /// draw of the same copy, by its larger ratio, however it is drawn.
    pub fn filtered(&mut self, picture: &Arc<Canvas>, across: u32, down: u32) -> Filtered {
        // A box no smaller than the picture on an axis uploads the picture
        // itself: no halving of it covers that axis.
        let (steps, copy) = Level::of(picture).uploaded(across, down);
        let needs = across < copy.width() && down < copy.height();
        let earlier = self
            .copies
            .iter_mut()
            .find(|upload| Arc::ptr_eq(&upload.picture, picture) && upload.steps == steps);
        let levelled = if let Some(upload) = earlier {
            upload.levelled |= needs;
            upload.levelled
        } else {
            self.copies.push(Upload {
                picture: Arc::clone(picture),
                steps,
                levelled: needs,
            });
            needs
        };
        Filtered::new(copy, across, down, levelled)
    }
}

impl Filtered {
    /// The filter for `picture` drawn alone into a box `across` by `down`
    /// device pixels: as the first draw of it in a paint.
    pub fn alone(picture: &Arc<Canvas>, across: u32, down: u32) -> Self {
        Uploads::default().filtered(picture, across, down)
    }

    /// The filter for an uploaded `copy` drawn into a box `across` by `down`:
    /// bilinear unless it has `levelled`, and then mixed from the two levels
    /// its larger ratio names.
    fn new(copy: Level, across: u32, down: u32, levelled: bool) -> Self {
        if !levelled {
            return Self::bilinear(copy, across, down);
        }
        let detail = detail(copy.width(), copy.height(), across, down);
        let (Ok(whole_levels), Ok(share)) =
            (u32::try_from(detail / 256), u32::try_from(detail % 256))
        else {
            return Self::bilinear(copy, across, down);
        };
        let mut near = copy;
        for _ in 0..whole_levels {
            let next = near.halved();
            if next == near {
                break;
            }
            near = next;
        }
        let far = near.halved();
        if share == 0 || far == near {
            return Self::bilinear(near, across, down);
        }
        Self {
            near: Sampled::new(near, across, down),
            far: Some((Sampled::new(far, across, down), share)),
        }
    }

    fn bilinear(level: Level, across: u32, down: u32) -> Self {
        Self {
            near: Sampled::new(level, across, down),
            far: None,
        }
    }

    /// What the box's pixel `x` across and `y` down takes, or [`None`] off
    /// the box.
    pub fn mix(&self, x: u32, y: u32) -> Option<Mix> {
        let near = self.near.parts(x, y)?;
        let Some((far, share)) = &self.far else {
            return Some(Mix::held(near, PARTS));
        };
        let far = far.parts(x, y)?;
        let share = u64::from(*share);
        let mut both = [0_u64; 4];
        for ((sum, near), far) in both.iter_mut().zip(near).zip(far) {
            *sum = near * (256 - share) + far * share;
        }
        Some(Mix::held(both, PARTS * 256))
    }
}

/// The level of detail for a copy `width` by `height` drawn into a box
/// `across` by `down`, in 256ths: the larger ratio's logarithm as a GPU
/// takes it, to the nearest 256th, less 120. At or under none, the copy is
/// drawn alone.
fn detail(width: u32, height: u32, across: u32, down: u32) -> i64 {
    let ratio = |of: u32, into: u32| f64::from(of) / f64::from(into.max(1));
    let larger = ratio(width, across).max(ratio(height, down));
    let scaled = (log2_as_drawn(larger) * 256.0 + 0.5).floor();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a ratio of two sides of at most 16,384 has a logarithm under 15, so this is under 3,840"
    )]
    let whole = scaled.clamp(0.0, 1.0e6) as i64;
    whole - 120
}

/// The base-two logarithm of `ratio`, at least one, as Chrome's GPU takes it:
/// the exponent, and the mantissa's logarithm straight between its values at
/// quarters from one to two.
fn log2_as_drawn(ratio: f64) -> f64 {
    if !ratio.is_finite() || ratio <= 1.0 {
        return 0.0;
    }
    let exponent = ratio.log2().floor();
    let mantissa = ratio / exponent.exp2();
    let quarters = (mantissa - 1.0) * 4.0;
    let below = quarters.floor().clamp(0.0, 3.0);
    let at = |quarter: f64| (1.0 + quarter / 4.0).log2();
    let (start, end) = (at(below), at(below + 1.0));
    exponent + start + (end - start) * (quarters - below)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mix_onto::onto;
    use alo_value::Rgba;

    #[test]
    fn the_logarithm_is_straight_between_quarters() {
        // On a quarter it is the logarithm itself.
        assert!((log2_as_drawn(2.0) - 1.0).abs() < 1e-12);
        assert!((log2_as_drawn(2.5) - 2.5_f64.log2()).abs() < 1e-12);
        assert!((log2_as_drawn(6.0) - 6.0_f64.log2()).abs() < 1e-12);
        // Between, it is the straight line, below the curve: 1.125 is half
        // way from log2(1) to log2(1.25).
        let half = 1.25_f64.log2() / 2.0;
        assert!((log2_as_drawn(1.125) - half).abs() < 1e-12);
        assert!(log2_as_drawn(1.125) < 1.125_f64.log2());
        assert!(log2_as_drawn(0.5).abs() < 1e-12);
        assert!(log2_as_drawn(f64::NAN).abs() < 1e-12);
    }

    #[test]
    fn the_detail_is_chromes_on_the_measured_ratios() {
        // Iteration 367's probes: a 32-texel side drawn 20, 16, 13 and 12
        // wide had the farther level's share 53, 136, 212 and 241, and 23 wide
        // none at all. (Drawn 22 wide it had 17, where this is 18: 32/22 is
        // 137.503 256ths, one of the ties a thirtieth of a 256th either side
        // of a half that Chrome rounds either way.)
        assert_eq!(detail(32, 32, 20, 31), 53);
        assert_eq!(detail(32, 32, 16, 31), 136);
        assert_eq!(detail(32, 32, 13, 31), 212);
        assert_eq!(detail(32, 32, 12, 31), 241);
        assert!(detail(32, 32, 23, 31) <= 0);
        // A whole level further down at four times as small.
        assert_eq!(detail(32, 32, 31, 8), 256 + 136);
    }

    fn noise(width: u32, height: u32) -> Arc<Canvas> {
        let mut canvas = Canvas::new(width, height, Rgba::TRANSPARENT);
        let mut state = 179_u32;
        for y in 0..height {
            for x in 0..width {
                let mut next = || {
                    state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                    u8::try_from((state >> 16) & 0xff).expect("a byte")
                };
                let colour = Rgba::from_rgba8(next(), next(), next(), 255);
                canvas.blend(x, y, colour, 255);
            }
        }
        Arc::new(canvas)
    }

    fn drawn(picture: &Arc<Canvas>, across: u32, down: u32) -> Vec<(u8, u8, u8, u8)> {
        pixels(&Filtered::alone(picture, across, down), across, down)
    }

    fn pixels(filtered: &Filtered, across: u32, down: u32) -> Vec<(u8, u8, u8, u8)> {
        (0..down)
            .flat_map(|y| (0..across).map(move |x| (x, y)))
            .map(|(x, y)| onto(filtered.mix(x, y).expect("in the box"), Rgba::WHITE).to_rgba8())
            .collect()
    }

    fn bilinear(picture: &Canvas, across: u32, down: u32) -> Vec<(u8, u8, u8, u8)> {
        let columns = taps(across, picture.width());
        let rows = taps(down, picture.height());
        rows.iter()
            .flat_map(|row| columns.iter().map(move |column| (*column, *row)))
            .map(|(column, row)| {
                let mix = crate::bilinear::sample(picture, column, row).expect("on it");
                onto(mix, Rgba::WHITE).to_rgba8()
            })
            .collect()
    }

    #[test]
    fn a_picture_shrunk_on_one_axis_only_is_bilinear() {
        let picture = noise(64, 4);
        for across in [2, 17, 40, 63] {
            assert_eq!(drawn(&picture, across, 4), bilinear(&picture, across, 4));
        }
        let square = noise(32, 32);
        assert_eq!(drawn(&square, 4, 40), bilinear(&square, 4, 40));
    }

    #[test]
    fn a_picture_shrunk_a_little_on_both_axes_is_still_bilinear() {
        // 32 to 23 is a detail of nothing.
        let picture = noise(32, 32);
        assert_eq!(drawn(&picture, 23, 24), bilinear(&picture, 23, 24));
    }

    #[test]
    fn a_picture_shrunk_on_both_axes_is_not_bilinear() {
        let picture = noise(32, 32);
        assert_ne!(drawn(&picture, 20, 20), bilinear(&picture, 20, 20));
    }

    #[test]
    fn drawn_into_one_pixel_a_picture_is_its_last_level() {
        // Two by two, one texel black, drawn into one pixel: the copy is the
        // one-texel level, 255 · 3 / 4 rounded down to 191, drawn alone.
        let mut picture = Canvas::new(2, 2, Rgba::WHITE);
        picture.blend(0, 0, Rgba::BLACK, 255);
        let one = drawn(&Arc::new(picture), 1, 1);
        assert_eq!(one, [(191, 191, 191, 255)]);
    }

    #[test]
    fn two_levels_are_mixed_by_the_farther_ones_share() {
        // An 8 × 8 picture whose left half is black drawn 2 × 3: the copy is
        // level one's 4 × 4 (2 × 2 would not cover three rows), and its
        // larger ratio, 4/2, is a detail of 136 — its own pixel mixed with
        // the 2 × 2 level's at 136/256.
        let mut picture = Canvas::new(8, 8, Rgba::WHITE);
        for y in 0..8 {
            for x in 0..4 {
                picture.blend(x, y, Rgba::BLACK, 255);
            }
        }
        let filtered = Filtered::alone(&Arc::new(picture), 2, 3);
        let (near, share) = (&filtered.near, filtered.far.as_ref().map(|(_, s)| *s));
        assert_eq!((near.level.width(), near.level.height()), (4, 4));
        assert_eq!(share, Some(136));
        // The left column is black at both levels, and the mix is black.
        assert_eq!(
            onto(filtered.mix(0, 1).expect("in it"), Rgba::WHITE).to_rgba8(),
            (0, 0, 0, 255)
        );
    }

    #[test]
    fn off_the_box_there_is_nothing() {
        let picture = noise(16, 16);
        let filtered = Filtered::alone(&picture, 5, 5);
        assert!(filtered.mix(5, 0).is_none());
        assert!(filtered.mix(0, 5).is_none());
    }

    #[test]
    fn after_a_draw_through_levels_every_draw_of_that_copy_is_levelled() {
        // Iteration 367's page A: a 32 × 32 picture drawn 20 × 20, then
        // 40 × 20 — larger across, smaller down — which alone is bilinear,
        // and after it is mixed by its larger ratio, 32/20, at 53/256.
        let picture = noise(32, 32);
        let mut uploads = Uploads::default();
        let _ = uploads.filtered(&picture, 20, 20);
        let later = uploads.filtered(&picture, 40, 20);
        assert_eq!(later.far.as_ref().map(|(_, share)| *share), Some(53));
        assert_ne!(pixels(&later, 40, 20), bilinear(&picture, 40, 20));
        // Another picture of the same texels is a file of its own.
        let other = Arc::new(Canvas::clone(&picture));
        let apart = uploads.filtered(&other, 40, 20);
        assert_eq!(pixels(&apart, 40, 20), bilinear(&picture, 40, 20));
    }

    #[test]
    fn a_draw_before_any_through_levels_is_bilinear() {
        // Page D: 40 × 20 first, then 20 × 20, which is levelled itself.
        let picture = noise(32, 32);
        let mut uploads = Uploads::default();
        let first = uploads.filtered(&picture, 40, 20);
        assert!(first.far.is_none());
        let second = uploads.filtered(&picture, 20, 20);
        assert_eq!(second.far.as_ref().map(|(_, share)| *share), Some(53));
        // Page I: and every draw of it after that is levelled too.
        let third = uploads.filtered(&picture, 20, 40);
        assert_eq!(third.far.as_ref().map(|(_, share)| *share), Some(53));
    }

    #[test]
    fn a_copy_a_level_down_leaves_the_picture_itself_unlevelled() {
        // Page H: 16 × 16 is level one's copy drawn at its own size; the
        // 40 × 20 after it is the picture's own copy, which has no levels.
        let picture = noise(32, 32);
        let mut uploads = Uploads::default();
        let first = uploads.filtered(&picture, 16, 16);
        assert!(first.far.is_none());
        assert_eq!(first.near.level.width(), 16);
        let later = uploads.filtered(&picture, 40, 20);
        assert_eq!(pixels(&later, 40, 20), bilinear(&picture, 40, 20));
    }
}
