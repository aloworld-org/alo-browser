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
}
