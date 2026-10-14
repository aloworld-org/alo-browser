/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How big a replaced box is, from what its style fixed and what its content
//! says.
//!
//! This is CSS 2's rule for replaced elements (§§ 10.3.2 and 10.6.2), the
//! rule every browser follows for an `<img>` and an `<svg>`. It needs three
//! things a content can have — a width, a height and a ratio, each perhaps
//! missing ([`NaturalSize`]) — and whatever the style already fixed.
//!
//! - A dimension the style fixed is used as it is.
//! - A missing one follows from the other through the ratio, or is the
//!   content's own, or is CSS's **default object size**, 300 × 150.
//! - With neither fixed and only a ratio — an `<svg>` with a `viewBox` and no
//!   size — the width is the room it was given, as a block's would be, and
//!   the height follows. Without room to fill, it is 300 wide.
//!
//! Then the limits, `min-width` and its siblings ([`used_size`], CSS 2
//! § 10.4). A dimension the style fixed is clamped on its own, and the one
//! that follows from it is worked out from the clamped value and clamped in
//! turn — so `width: 60px; max-height: 12px` on a 2:1 picture is 60 by 12,
//! never 24 by 12. With neither fixed, the content's size is brought inside
//! the limits keeping its ratio, by § 10.4's table.
//!
//! The ratio is never `taffy`'s to apply. Handed one, it holds a box's
//! height at least its width over the ratio whatever the style fixed — a
//! square picture at `width: 30px; height: 20px` came out 30 by 30 — and
//! carries a `max-height` across into a narrower width (queue item 590).
//!
//! No `taffy` here: `engine.rs` and `arena.rs` are the only files that may
//! name it, and this is arithmetic that does not need to.

use crate::geometry::Size;
use alo_box::NaturalSize;

/// CSS's default object size: what a replaced box is when nothing says
/// otherwise. It is the size of an `<iframe>` or a `<canvas>` with no
/// attributes, and of an `<svg>` with nothing written on it.
pub const DEFAULT: Size = Size {
    width: 300.0,
    height: 150.0,
};

/// The content-box size of a replaced box.
///
/// `width` and `height` are what the style already fixed, `room` the width
/// available to it if that is definite.
pub fn concrete_size(
    natural: NaturalSize,
    width: Option<f32>,
    height: Option<f32>,
    room: Option<f32>,
) -> Size {
    let ratio = natural.ratio();
    let height_for = |width: f32| {
        ratio
            .map(|ratio| width / ratio)
            .or(natural.height)
            .unwrap_or(DEFAULT.height)
    };
    let width_for = |height: f32| {
        ratio
            .map(|ratio| height * ratio)
            .or(natural.width)
            .unwrap_or(DEFAULT.width)
    };
    let (width, height) = match (width, height) {
        (Some(width), Some(height)) => (width, height),
        (Some(width), None) => (width, height_for(width)),
        (None, Some(height)) => (width_for(height), height),
        (None, None) => match (natural.width, natural.height) {
            (Some(width), Some(height)) => (width, height),
            (Some(width), None) => (width, height_for(width)),
            (None, Some(height)) => (width_for(height), height),
            (None, None) if ratio.is_some() => {
                let width = room.unwrap_or(DEFAULT.width);
                (width, height_for(width))
            }
            (None, None) => (DEFAULT.width, DEFAULT.height),
        },
    };
    Size { width, height }
}

/// What a replaced box's style asks of its content box: a width and a height
/// where it fixed one, and the limits on each. A limit not written is `0`
/// for a minimum and infinite for a maximum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Asked {
    /// The width the style fixed, if it fixed one.
    pub width: Option<f32>,
    /// The height the style fixed, if it fixed one.
    pub height: Option<f32>,
    /// `min-width` and `min-height`.
    pub min: Size,
    /// `max-width` and `max-height`.
    pub max: Size,
}

impl Asked {
    /// Nothing fixed and nothing limited.
    pub const NOTHING: Self = Self {
        width: None,
        height: None,
        min: Size {
            width: 0.0,
            height: 0.0,
        },
        max: Size {
            width: f32::INFINITY,
            height: f32::INFINITY,
        },
    };
}

/// The content-box size of a replaced box, its limits applied.
///
/// A maximum below its minimum is the minimum (CSS 2 § 10.4: `min-width`
/// wins). `room` is the width available to the box, if definite.
pub fn used_size(natural: NaturalSize, asked: Asked, room: Option<f32>) -> Size {
    let max = Size {
        width: asked.max.width.max(asked.min.width),
        height: asked.max.height.max(asked.min.height),
    };
    let min = asked.min;
    let width_in = |width: f32| width.min(max.width).max(min.width);
    let height_in = |height: f32| height.min(max.height).max(min.height);
    match (asked.width, asked.height) {
        (Some(width), Some(height)) => Size {
            width: width_in(width),
            height: height_in(height),
        },
        (Some(width), None) => {
            let width = width_in(width);
            let height = concrete_size(natural, Some(width), None, room).height;
            Size {
                width,
                height: height_in(height),
            }
        }
        (None, Some(height)) => {
            let height = height_in(height);
            let width = concrete_size(natural, None, Some(height), room).width;
            Size {
                width: width_in(width),
                height,
            }
        }
        (None, None) => {
            let tentative = concrete_size(natural, None, None, room);
            if natural.ratio().is_some() {
                within_limits(tentative, min, max)
            } else {
                Size {
                    width: width_in(tentative.width),
                    height: height_in(tentative.height),
                }
            }
        }
    }
}

/// CSS 2 § 10.4's table: a size with neither dimension fixed, brought
/// inside its limits keeping its shape where the limits allow.
///
/// `max` is already no smaller than `min`. A size with no area has no shape
/// to keep, and each dimension is clamped on its own.
fn within_limits(size: Size, min: Size, max: Size) -> Size {
    let Size { width, height } = size;
    if width <= 0.0 || height <= 0.0 {
        return Size {
            width: width.min(max.width).max(min.width),
            height: height.min(max.height).max(min.height),
        };
    }
    let (over_width, under_width) = (width > max.width, width < min.width);
    let (over_height, under_height) = (height > max.height, height < min.height);
    let (width, height) = match (over_width, under_width, over_height, under_height) {
        (true, _, true, _) if max.width / width <= max.height / height => {
            (max.width, (max.width * height / width).max(min.height))
        }
        (true, _, true, _) => ((max.height * width / height).max(min.width), max.height),
        (_, true, _, true) if min.width / width <= min.height / height => {
            ((min.height * width / height).min(max.width), min.height)
        }
        (_, true, _, true) => (min.width, (min.width * height / width).min(max.height)),
        (_, true, true, _) => (min.width, max.height),
        (true, _, _, true) => (max.width, min.height),
        (true, _, _, _) => (max.width, (max.width * height / width).max(min.height)),
        (_, true, _, _) => (min.width, (min.width * height / width).min(max.height)),
        (_, _, true, _) => ((max.height * width / height).max(min.width), max.height),
        (_, _, _, true) => ((min.height * width / height).min(max.width), min.height),
        _ => (width, height),
    };
    Size { width, height }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(width: f32, height: f32) -> Size {
        Size { width, height }
    }

    fn ratio_only(ratio: f32) -> NaturalSize {
        NaturalSize {
            stated_ratio: Some(ratio),
            ..NaturalSize::default()
        }
    }

    #[test]
    fn a_picture_with_nothing_fixed_is_its_own_size() {
        let picture = NaturalSize::sized(24.0, 12.0);
        assert_eq!(
            concrete_size(picture, None, None, Some(500.0)),
            size(24.0, 12.0)
        );
    }

    #[test]
    fn one_fixed_dimension_keeps_the_ratio() {
        let picture = NaturalSize::sized(24.0, 12.0);
        assert_eq!(
            concrete_size(picture, Some(80.0), None, None),
            size(80.0, 40.0)
        );
        assert_eq!(
            concrete_size(picture, None, Some(30.0), None),
            size(60.0, 30.0)
        );
    }

    #[test]
    fn both_fixed_is_what_the_style_said_whatever_the_content_says() {
        for natural in [
            NaturalSize::sized(24.0, 12.0),
            ratio_only(3.0),
            NaturalSize::default(),
        ] {
            assert_eq!(
                concrete_size(natural, Some(56.0), Some(56.0), Some(500.0)),
                size(56.0, 56.0),
            );
        }
    }

    /// An `<svg viewBox>` with no size fills the room it has, as a block
    /// would, and its height follows from its shape.
    #[test]
    fn a_ratio_and_no_size_fills_the_room() {
        assert_eq!(
            concrete_size(ratio_only(2.0), None, None, Some(200.0)),
            size(200.0, 100.0)
        );
        assert_eq!(
            concrete_size(ratio_only(2.0), None, None, None),
            size(300.0, 150.0),
            "with no room it is the default width",
        );
        assert_eq!(
            concrete_size(ratio_only(0.5), None, Some(40.0), None),
            size(20.0, 40.0)
        );
    }

    #[test]
    fn with_nothing_at_all_it_is_three_hundred_by_one_hundred_and_fifty() {
        let nothing = NaturalSize::default();
        assert_eq!(concrete_size(nothing, None, None, Some(1000.0)), DEFAULT);
        assert_eq!(
            concrete_size(nothing, Some(100.0), None, None),
            size(100.0, 150.0)
        );
        assert_eq!(
            concrete_size(nothing, None, Some(10.0), None),
            size(300.0, 10.0)
        );
    }

    #[test]
    fn half_a_size_and_no_ratio_takes_the_default_for_the_other_half() {
        let wide = NaturalSize {
            width: Some(48.0),
            ..NaturalSize::default()
        };
        assert_eq!(concrete_size(wide, None, None, None), size(48.0, 150.0));
        let tall = NaturalSize {
            height: Some(48.0),
            ..NaturalSize::default()
        };
        assert_eq!(concrete_size(tall, None, None, None), size(300.0, 48.0));
    }

    #[test]
    fn half_a_size_and_a_ratio_gives_the_other_half() {
        let wide = NaturalSize {
            width: Some(48.0),
            stated_ratio: Some(4.0),
            ..NaturalSize::default()
        };
        assert_eq!(
            concrete_size(wide, None, None, Some(999.0)),
            size(48.0, 12.0)
        );
    }

    /// A picture with no area has no ratio, so a fixed width falls back to the
    /// picture's own height rather than dividing by zero.
    #[test]
    fn a_picture_with_no_area_never_divides_by_zero() {
        let flat = NaturalSize::sized(10.0, 0.0);
        assert_eq!(concrete_size(flat, Some(80.0), None, None), size(80.0, 0.0));
        let thin = NaturalSize::sized(0.0, 10.0);
        assert_eq!(concrete_size(thin, None, Some(40.0), None), size(0.0, 40.0));
    }

    fn asked(width: Option<f32>, height: Option<f32>) -> Asked {
        Asked {
            width,
            height,
            ..Asked::NOTHING
        }
    }

    /// The case queue item 590 was opened by: a square picture at 30 by 20
    /// is 30 by 20, its ratio set aside.
    #[test]
    fn both_fixed_is_both_whatever_the_ratio() {
        let square = NaturalSize::sized(20.0, 20.0);
        assert_eq!(
            used_size(square, asked(Some(30.0), Some(20.0)), Some(500.0)),
            size(30.0, 20.0)
        );
    }

    /// The fixed width is kept and the height that follows from it is
    /// capped; the cap is not carried back across into the width.
    #[test]
    fn a_cap_on_the_dimension_that_follows_is_not_carried_back() {
        let wide = NaturalSize::sized(40.0, 20.0);
        let capped = Asked {
            max: Size::new(f32::INFINITY, 12.0),
            ..asked(Some(60.0), None)
        };
        assert_eq!(used_size(wide, capped, None), size(60.0, 12.0));
        let floored = Asked {
            min: Size::new(50.0, 0.0),
            ..asked(None, Some(10.0))
        };
        assert_eq!(used_size(wide, floored, None), size(50.0, 10.0));
    }

    /// A fixed dimension is clamped first, and the other follows from the
    /// clamped one.
    #[test]
    fn the_dimension_that_follows_follows_the_clamped_one() {
        let wide = NaturalSize::sized(40.0, 20.0);
        let narrowed = Asked {
            max: Size::new(30.0, f32::INFINITY),
            ..asked(Some(60.0), None)
        };
        assert_eq!(used_size(wide, narrowed, None), size(30.0, 15.0));
    }

    /// A maximum below its minimum is the minimum.
    #[test]
    fn a_minimum_outranks_a_maximum() {
        let square = NaturalSize::sized(20.0, 20.0);
        let crossed = Asked {
            min: Size::new(25.0, 0.0),
            max: Size::new(10.0, f32::INFINITY),
            ..asked(Some(40.0), Some(5.0))
        };
        assert_eq!(used_size(square, crossed, None), size(25.0, 5.0));
    }

    /// CSS 2 § 10.4's table, one row at a time, on a 40 by 20 picture with
    /// nothing fixed.
    #[test]
    fn with_nothing_fixed_the_limits_keep_the_shape() {
        let wide = NaturalSize::sized(40.0, 20.0);
        let limited = |min: (f32, f32), max: (f32, f32)| {
            used_size(
                wide,
                Asked {
                    min: Size::new(min.0, min.1),
                    max: Size::new(max.0, max.1),
                    ..Asked::NOTHING
                },
                None,
            )
        };
        let unlimited = f32::INFINITY;
        // Inside every limit.
        assert_eq!(
            limited((0.0, 0.0), (unlimited, unlimited)),
            size(40.0, 20.0)
        );
        // Too wide; too narrow; too tall; too short.
        assert_eq!(limited((0.0, 0.0), (25.0, unlimited)), size(25.0, 12.5));
        assert_eq!(
            limited((50.0, 0.0), (unlimited, unlimited)),
            size(50.0, 25.0)
        );
        assert_eq!(limited((0.0, 0.0), (unlimited, 10.0)), size(20.0, 10.0));
        assert_eq!(
            limited((0.0, 30.0), (unlimited, unlimited)),
            size(60.0, 30.0)
        );
        // Too wide and too tall, the width the tighter, then the height.
        assert_eq!(limited((0.0, 0.0), (30.0, 18.0)), size(30.0, 15.0));
        assert_eq!(limited((0.0, 0.0), (30.0, 10.0)), size(20.0, 10.0));
        // Too narrow and too short, the width the looser, then the height.
        assert_eq!(
            limited((50.0, 30.0), (unlimited, unlimited)),
            size(60.0, 30.0)
        );
        assert_eq!(
            limited((80.0, 30.0), (unlimited, unlimited)),
            size(80.0, 40.0)
        );
        // Too narrow and too tall; too wide and too short.
        assert_eq!(limited((50.0, 0.0), (unlimited, 10.0)), size(50.0, 10.0));
        assert_eq!(limited((0.0, 25.0), (30.0, unlimited)), size(30.0, 25.0));
        // A limit that leaves no shape to keep is still met.
        assert_eq!(limited((0.0, 30.0), (40.0, unlimited)), size(40.0, 30.0));
    }

    /// With no ratio there is no shape to keep: each dimension is clamped on
    /// its own.
    #[test]
    fn with_no_ratio_each_dimension_is_clamped_on_its_own() {
        let nothing = NaturalSize::default();
        let limited = Asked {
            max: Size::new(100.0, 60.0),
            ..Asked::NOTHING
        };
        assert_eq!(used_size(nothing, limited, None), size(100.0, 60.0));
    }
}
