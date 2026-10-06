/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a replaced box's content says about its own size.
//!
//! CSS calls these the **natural dimensions**: a width, a height and a ratio,
//! any of which may be missing. A decoded picture has all three. An `<svg>`
//! with only a `viewBox` has a ratio and no size at all, and one with nothing
//! has none of them — and those are different answers, which is why this is
//! three optional numbers rather than a width and a height.
//!
//! What a box *does* with them, given what its style says, is CSS's default
//! sizing rule and belongs to layout. This is only what the content claims.

/// A replaced box's natural width, height and ratio, each of which may be
/// absent.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NaturalSize {
    /// Its own width in CSS pixels, if it has one.
    pub width: Option<f32>,
    /// Its own height in CSS pixels, if it has one.
    pub height: Option<f32>,
    /// Width over height, when the content states one apart from its size —
    /// an `<svg>`'s `viewBox`. [`NaturalSize::ratio`] is the one to read.
    pub stated_ratio: Option<f32>,
}

impl NaturalSize {
    /// Content with a width and a height, as a decoded picture has.
    pub fn sized(width: f32, height: f32) -> Self {
        Self {
            width: Some(width),
            height: Some(height),
            stated_ratio: None,
        }
    }

    /// Width over height: the two natural dimensions when there are both, and
    /// the stated ratio otherwise.
    ///
    /// Never zero, negative or infinite. A picture zero pixels tall has no
    /// shape to keep, and a ratio of nothing would make every width it was
    /// applied to into an infinite height.
    pub fn ratio(&self) -> Option<f32> {
        let from_size = match (self.width, self.height) {
            (Some(width), Some(height)) if height > 0.0 => Some(width / height),
            _ => None,
        };
        from_size
            .filter(|ratio| ratio.is_finite() && *ratio > 0.0)
            .or_else(|| {
                self.stated_ratio
                    .filter(|ratio| ratio.is_finite() && *ratio > 0.0)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_has_the_ratio_of_its_own_size() {
        assert_eq!(NaturalSize::sized(80.0, 40.0).ratio(), Some(2.0));
    }

    #[test]
    fn a_size_outranks_a_stated_ratio() {
        let both = NaturalSize {
            stated_ratio: Some(1.0),
            ..NaturalSize::sized(30.0, 10.0)
        };
        assert_eq!(both.ratio(), Some(3.0));
    }

    #[test]
    fn a_stated_ratio_stands_in_when_there_is_no_size() {
        let only = NaturalSize {
            stated_ratio: Some(0.5),
            ..NaturalSize::default()
        };
        assert_eq!(only.ratio(), Some(0.5));
        let half = NaturalSize {
            width: Some(10.0),
            stated_ratio: Some(4.0),
            ..NaturalSize::default()
        };
        assert_eq!(half.ratio(), Some(4.0));
    }

    #[test]
    fn a_shape_with_no_area_has_no_ratio() {
        assert_eq!(NaturalSize::sized(10.0, 0.0).ratio(), None);
        assert_eq!(NaturalSize::sized(0.0, 10.0).ratio(), None);
        assert_eq!(NaturalSize::default().ratio(), None);
        let nothing = NaturalSize {
            stated_ratio: Some(f32::INFINITY),
            ..NaturalSize::default()
        };
        assert_eq!(nothing.ratio(), None);
    }
}
