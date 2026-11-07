/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A rectangle of whole device pixels on a canvas (queue item 870).
//!
//! What a list painted again over the drawing before it repaints
//! ([`crate::repaint()`]), what a canvas is confined to while it does
//! ([`crate::Canvas::confine`]), and where an item can reach
//! ([`crate::extent`]). Its own type because all three speak of the same
//! rectangle, and a rectangle of floats would be a rectangle somebody rounds
//! differently.

/// Whole device pixels from `left` and `top`, up to but not including
/// `right` and `bottom`. Never empty: a rectangle of no pixels is
/// [`None`] wherever one is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    /// The first column in it.
    pub left: u32,
    /// The first row in it.
    pub top: u32,
    /// The first column past it.
    pub right: u32,
    /// The first row past it.
    pub bottom: u32,
}

impl Region {
    /// The pixels from `left`, `top` up to `right`, `bottom`, or [`None`]
    /// when that holds none.
    pub fn new(left: u32, top: u32, right: u32, bottom: u32) -> Option<Self> {
        (left < right && top < bottom).then_some(Self {
            left,
            top,
            right,
            bottom,
        })
    }

    /// A whole canvas of this size, or [`None`] for one of no pixels.
    pub fn whole(width: u32, height: u32) -> Option<Self> {
        Self::new(0, 0, width, height)
    }

    /// Every pixel a shape with these bounds, in device pixels, can touch on
    /// a canvas of `size`: whole pixels outwards, and one more each side,
    /// since a pixel an edge only grazes is still drawn. [`None`] when that
    /// is off the canvas; a bound that is not a finite number is the whole
    /// canvas, since where such a shape lands cannot be said.
    pub fn around(
        (left, top, right, bottom): (f32, f32, f32, f32),
        size: (u32, u32),
    ) -> Option<Self> {
        if ![left, top, right, bottom]
            .iter()
            .all(|value| value.is_finite())
        {
            return Self::whole(size.0, size.1);
        }
        Self::new(
            pixel(left.floor() - 1.0, size.0),
            pixel(top.floor() - 1.0, size.1),
            pixel(right.ceil() + 1.0, size.0),
            pixel(bottom.ceil() + 1.0, size.1),
        )
    }

    /// How many pixels it holds.
    pub fn area(self) -> u64 {
        u64::from(self.right - self.left) * u64::from(self.bottom - self.top)
    }

    /// Whether this pixel is in it.
    pub fn holds(self, x: u32, y: u32) -> bool {
        (self.left..self.right).contains(&x) && (self.top..self.bottom).contains(&y)
    }

    /// The smallest region holding both.
    #[must_use]
    pub fn joined(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    /// The pixels in both, or [`None`] when they do not meet.
    pub fn meeting(self, other: Self) -> Option<Self> {
        Self::new(
            self.left.max(other.left),
            self.top.max(other.top),
            self.right.min(other.right),
            self.bottom.min(other.bottom),
        )
    }
}

/// A whole-pixel position clamped onto `0..=limit`, without a cast that
/// could wrap.
fn pixel(value: f32, limit: u32) -> u32 {
    let most = f32::from(u16::try_from(limit).unwrap_or(u16::MAX));
    let clamped = value.clamp(0.0, most);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=65535, a whole number after floor or ceil"
    )]
    let whole = clamped as u32;
    whole
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shape_reaches_the_whole_pixels_around_it_and_one_more() {
        let region = Region::around((10.25, 4.0, 12.5, 6.0), (100, 100));
        assert_eq!(region, Region::new(9, 3, 14, 7));
    }

    #[test]
    fn a_shape_off_the_canvas_reaches_nothing_and_one_hanging_off_is_cut() {
        assert_eq!(Region::around((-50.0, -50.0, -10.0, -10.0), (20, 20)), None);
        assert_eq!(Region::around((200.0, 0.0, 300.0, 5.0), (20, 20)), None);
        assert_eq!(
            Region::around((-5.0, 15.0, 5.0, 40.0), (20, 20)),
            Region::new(0, 14, 6, 20)
        );
    }

    #[test]
    fn a_bound_that_is_not_a_number_is_the_whole_canvas() {
        assert_eq!(
            Region::around((f32::NAN, 0.0, 1.0, 1.0), (8, 6)),
            Region::whole(8, 6)
        );
    }

    #[test]
    fn regions_join_meet_and_count_their_pixels() {
        let (Some(one), Some(two)) = (Region::new(0, 0, 4, 4), Region::new(2, 3, 10, 5)) else {
            panic!("both have pixels");
        };
        assert_eq!(
            one.joined(two),
            Region {
                left: 0,
                top: 0,
                right: 10,
                bottom: 5
            }
        );
        assert_eq!(one.meeting(two), Region::new(2, 3, 4, 4));
        assert_eq!(
            one.meeting(Region {
                left: 4,
                top: 0,
                right: 5,
                bottom: 1
            }),
            None
        );
        assert_eq!(two.area(), 16);
        assert!(two.holds(2, 3) && !two.holds(10, 3) && !two.holds(2, 5));
    }
}
