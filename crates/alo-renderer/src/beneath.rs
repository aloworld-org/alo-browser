/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a part's picture drawn beneath it stands (queue item 869, ADR
//! 0100).
//!
//! The card's page is drawn beneath the first screen by the conversation's
//! window, and the first screen leaves its place clear
//! ([`alo_paint::beneath`]). The window asks the first screen's renderer
//! where that place is in the frame it just painted
//! ([`crate::ToRenderer::PictureBeneath`]), and draws the card's picture
//! there, at that size ([`crate::card_picture`]).
//!
//! In device pixels of the frame: the place is the `<img>`'s content box
//! snapped to whole device pixels, as every picture's is, so its edges are
//! whole numbers at any scale. It may stand partly above or left of the
//! frame — the conversation scrolled — and is then still the whole place:
//! the window draws the picture whole, and the frame stands over what of it
//! is not the clear place.

use alo_paint::{DisplayItem, DisplayList};

/// The largest a place's side is taken to be, in device pixels: a card's
/// page box at the largest scale this engine draws at is well inside it.
pub const MOST_SIDE: u32 = 8192;

/// The furthest a place stands from the frame's corner, in device pixels.
pub const FURTHEST: i32 = 1 << 20;

/// A place in a frame, in its device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Beneath {
    /// How far its left edge is right of the frame's.
    pub left: i32,
    /// How far its top edge is below the frame's.
    pub top: i32,
    /// How wide.
    pub width: u32,
    /// How tall.
    pub height: u32,
}

impl Beneath {
    /// Whether it is a place a renderer could have drawn: some size, each
    /// side at most [`MOST_SIDE`], no further than [`FURTHEST`].
    pub fn is_bounded(self) -> bool {
        (1..=MOST_SIDE).contains(&self.width)
            && (1..=MOST_SIDE).contains(&self.height)
            && self.left.abs() <= FURTHEST
            && self.top.abs() <= FURTHEST
    }
}

/// Where the first picture `display` draws beneath stands, in device
/// pixels: nothing when it draws none, or one of no size.
pub fn of(display: &DisplayList) -> Option<Beneath> {
    let scale = display.scale();
    display.items().iter().find_map(|item| match item {
        DisplayItem::Beneath { rect, .. } => {
            let left = device(rect.left() * scale)?;
            let top = device(rect.top() * scale)?;
            let width = u32::try_from(device(rect.right() * scale)? - left).ok()?;
            let height = u32::try_from(device(rect.bottom() * scale)? - top).ok()?;
            Some(Beneath {
                left,
                top,
                width,
                height,
            })
            .filter(|place| place.is_bounded())
        }
        _ => None,
    })
}

/// A device pixel's edge, which a snapped place's is to within a rounding:
/// [`None`] past [`FURTHEST`] or not a number.
fn device(value: f32) -> Option<i32> {
    let rounded = value.round();
    let furthest = 1_048_576.0;
    if !rounded.is_finite() || rounded.abs() > furthest {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "rounded, finite and at most 2²⁰ either way just above"
    )]
    let whole = rounded as i32;
    Some(whole)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_place_is_bounded_only_with_a_size_inside_the_bounds() {
        let place = Beneath {
            left: -40,
            top: 12,
            width: 1756,
            height: 1040,
        };
        assert!(place.is_bounded());
        assert!(!Beneath { width: 0, ..place }.is_bounded());
        assert!(
            !Beneath {
                height: MOST_SIDE + 1,
                ..place
            }
            .is_bounded()
        );
        assert!(
            !Beneath {
                left: -FURTHEST - 1,
                ..place
            }
            .is_bounded()
        );
    }

    #[test]
    fn an_edge_is_its_device_pixel() {
        assert_eq!(device(275.999_97), Some(276));
        assert_eq!(device(-3.0), Some(-3));
        assert_eq!(device(f32::NAN), None);
        assert_eq!(device(2.0e7), None);
    }
}
