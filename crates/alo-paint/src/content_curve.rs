/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The curve a picture is trimmed to: its box's own `border-radius`, at
//! its content edge (queue item 769).
//!
//! An `<img>` with `border-radius: 5px` was drawn square: the engine
//! rounded a box's background, and clipped an `overflow` box's children to
//! its padding edge's curve, but drew a picture over its content box whole.
//! CSS Backgrounds 3 § 5.3: *the content of replaced elements is always
//! trimmed to the content edge curve* — the border box's radii, resolved
//! against it and fitted to it, each less the border **and the padding**
//! beside it, never less than square.
//!
//! Measured in Chrome 156.0.8078.76 at eight, not read from its source: 27
//! black pictures of 40 × 40, 60 × 30 and 41.5 × 33.25, with radii of 4,
//! 10, 20 and 100 pixels, 50 %, `12px 4px` and `30px / 10px`, borders of 0,
//! 1.5, 2 and 3 (transparent) and paddings of 0, 2.25, 3 and 5 — every
//! one the content edge's curve to within 0.09 of a pixel's coverage (the
//! smoothing), where the padding edge's curve misses 14 of them and the
//! border edge's 20 by whole pixels. A radius smaller than the border and
//! padding beside it leaves the picture square.
//!
//! The rectangle is the one the picture is drawn into — its content box
//! with each edge rounded on its own to device pixels (`build.rs`) — and
//! the radii are the snapped border box's, as an `overflow` clip's are.

use crate::corner::Corners;
use alo_layout::{Edges, Rect};

/// The corners a picture in a box is trimmed to: the radii `written` — read
/// for the box's border box `border_box` and fitted to it — less `border`
/// and `padding` on each side. Square when nothing is left of any.
pub fn of(written: Corners, border_box: Rect, border: Edges, padding: Edges) -> Corners {
    written.fitted_to(border_box.size).inside(Edges {
        top: border.top + padding.top,
        right: border.right + padding.right,
        bottom: border.bottom + padding.bottom,
        left: border.left + padding.left,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_layout::{Point, Size};

    fn close(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4
    }

    fn the_box(width: f32, height: f32) -> Rect {
        Rect {
            origin: Point { x: 8.0, y: 8.0 },
            size: Size { width, height },
        }
    }

    #[test]
    fn a_picture_with_no_border_or_padding_has_the_boxs_own_corners() {
        let corners = of(
            Corners::all(10.0),
            the_box(40.0, 40.0),
            Edges::ZERO,
            Edges::ZERO,
        );
        assert!(close(corners.top_left, (10.0, 10.0)));
        assert!(close(corners.bottom_right, (10.0, 10.0)));
    }

    #[test]
    fn the_border_and_the_padding_both_come_off_each_radius() {
        // 20 less 3 of border and 5 of padding: the content edge's curve,
        // where the padding edge's would be 17.
        let corners = of(
            Corners::all(20.0),
            the_box(56.0, 56.0),
            Edges::all(3.0),
            Edges::all(5.0),
        );
        for corner in [
            corners.top_left,
            corners.top_right,
            corners.bottom_right,
            corners.bottom_left,
        ] {
            assert!(close(corner, (12.0, 12.0)), "{corner:?}");
        }
    }

    #[test]
    fn a_radius_smaller_than_what_is_inside_it_leaves_the_picture_square() {
        let corners = of(
            Corners::all(4.0),
            the_box(50.0, 50.0),
            Edges::ZERO,
            Edges::all(5.0),
        );
        assert!(corners.are_square());
    }

    #[test]
    fn radii_too_large_for_the_box_are_fitted_to_it_first() {
        // 100 on a box 60 × 30 fits to 15 (30 / 200), then less 2 of
        // border and 3 of padding on every side: 10.
        let corners = of(
            Corners::all(100.0),
            the_box(60.0, 30.0),
            Edges::all(2.0),
            Edges::all(3.0),
        );
        assert!(close(corners.top_left, (10.0, 10.0)), "{corners:?}");
    }

    #[test]
    fn each_side_takes_its_own_border_and_padding() {
        let corners = of(
            Corners::all(12.0),
            the_box(80.0, 80.0),
            Edges {
                top: 1.0,
                right: 2.0,
                bottom: 3.0,
                left: 4.0,
            },
            Edges {
                top: 0.5,
                right: 1.5,
                bottom: 2.5,
                left: 3.5,
            },
        );
        assert!(close(corners.top_left, (4.5, 10.5)), "{corners:?}");
        assert!(close(corners.top_right, (8.5, 10.5)), "{corners:?}");
        assert!(close(corners.bottom_right, (8.5, 6.5)), "{corners:?}");
        assert!(close(corners.bottom_left, (4.5, 6.5)), "{corners:?}");
    }
}
