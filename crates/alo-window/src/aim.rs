/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which frame a point in the window is over, and where in it (ADR 0024
//! § 5, queue item 535).
//!
//! The browser process decides this from **its own composition** — the
//! [`Placement`] the window was drawn from — never by asking a renderer,
//! which holds only its own frame. A point is the window server's, in
//! device pixels from the window's top-left corner; what a renderer is sent
//! is CSS pixels from its frame's corner, a device pixel being `1 / scale`
//! of one at the window's whole scale (ADR 0057 § 3). Where a person's
//! 1×1 pixel lands at a scale above one is not measured (queue item 568);
//! the point is the device pixel's corner divided, as at one.
//!
//! A point is over **the page** when it is below the strip's band and over
//! no line the window is saying something in: a notice is the browser's own
//! drawing over the page, and a press on it is not the page's. It is over
//! the page even where the last frame does not reach — after the window
//! grew, before the new frame arrives — because the page is laid out at the
//! window's size, and the renderer answers for its own viewport. Over the
//! band it is **the strip's**, and a press there chooses the tab the strip's
//! renderer claims (item 539).

use crate::place::{PixelRect, Placement};
use alo_paint::Point;

/// What a point in the window is over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Aimed {
    /// The selected tab's page, at this point in its CSS pixels.
    Page(Point),
    /// The tab strip, at this point in its CSS pixels.
    Strip(Point),
    /// Nothing a renderer draws: outside the window, or over a line the
    /// window is saying something in.
    Nothing,
}

/// What the point `at`, in device pixels, is over in a `window` of device
/// pixels placed as `placed`, `scale` device pixels to the CSS pixel.
pub fn aim(window: (u32, u32), placed: &Placement, scale: u32, at: (f64, f64)) -> Aimed {
    let (x, y) = at;
    let inside = (0.0..f64::from(window.0)).contains(&x) && (0.0..f64::from(window.1)).contains(&y);
    let over = |rect: Option<PixelRect>| rect.is_some_and(|rect| covers(rect, at));
    if !inside || over(placed.notice) || over(placed.strip_notice) {
        return Aimed::Nothing;
    }
    if y < f64::from(placed.band.height) {
        return Aimed::Strip(css(scale, x, y));
    }
    Aimed::Page(on_page(placed, scale, at))
}

/// Where the point `at`, in device pixels, is in the page's CSS pixels,
/// wherever it is — inside the window or not, over a notice or the band.
/// What a release is sent as: it goes to the page its press went to.
pub fn on_page(placed: &Placement, scale: u32, at: (f64, f64)) -> Point {
    css(scale, at.0, at.1 - f64::from(placed.band.height))
}

/// Whether `rect` covers the point `at`.
fn covers(rect: PixelRect, at: (f64, f64)) -> bool {
    let (left, top) = (f64::from(rect.x), f64::from(rect.y));
    (left..left + f64::from(rect.width)).contains(&at.0)
        && (top..top + f64::from(rect.height)).contains(&at.1)
}

/// Device pixels (`x`, `y`) as CSS pixels: divided by the scale.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a point in a window is within an f32's range, and its last bits are below a 64th of a pixel"
)]
fn css(scale: u32, x: f64, y: f64) -> Point {
    let scale = f64::from(scale.max(1));
    Point::new((x / scale) as f32, (y / scale) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::place::Contents;

    /// A window of 960 × 872 device pixels at scale 2, the strip and a page
    /// painted at 2, and lines said for neither or both.
    fn placed(saying: bool) -> Placement {
        Placement::of(
            (960, 872),
            2,
            Contents {
                strip: Some((960, 72)),
                frame: Some((960, 800)),
                tab_saying: saying,
                strip_saying: saying,
            },
        )
    }

    fn page(x: f32, y: f32) -> Aimed {
        Aimed::Page(Point::new(x, y))
    }

    #[test]
    fn below_the_band_is_the_page_in_its_own_css_pixels() {
        let placed = placed(false);
        assert_eq!(placed.band.height, 72, "36 CSS pixels at scale 2");
        assert_eq!(aim((960, 872), &placed, 2, (141.2, 72.0)), page(70.6, 0.0));
        assert_eq!(aim((960, 872), &placed, 2, (0.0, 871.0)), page(0.0, 399.5));
        assert_eq!(
            aim((960, 872), &placed, 2, (959.5, 600.5)),
            page(479.75, 264.25)
        );
    }

    #[test]
    fn the_band_is_the_strips() {
        let placed = placed(false);
        assert_eq!(
            aim((960, 872), &placed, 2, (100.0, 0.0)),
            Aimed::Strip(Point::new(50.0, 0.0))
        );
        assert_eq!(
            aim((960, 872), &placed, 2, (100.0, 71.9)),
            Aimed::Strip(Point::new(50.0, 35.95))
        );
    }

    #[test]
    fn outside_the_window_is_nothing() {
        let placed = placed(false);
        for at in [(-0.5, 100.0), (960.0, 100.0), (10.0, 872.0), (10.0, -1.0)] {
            assert_eq!(aim((960, 872), &placed, 2, at), Aimed::Nothing, "{at:?}");
        }
        assert_eq!(
            aim((960, 872), &placed, 2, (f64::NAN, 100.0)),
            Aimed::Nothing
        );
    }

    #[test]
    fn a_line_the_window_is_saying_is_not_the_pages() {
        let placed = placed(true);
        // The tab's line is the bottom 64 device pixels, the strip's the 64
        // above it.
        assert_eq!(
            placed.notice.map(|rect| (rect.y, rect.height)),
            Some((808, 64))
        );
        assert_eq!(
            placed.strip_notice.map(|rect| (rect.y, rect.height)),
            Some((744, 64))
        );
        assert_eq!(aim((960, 872), &placed, 2, (10.0, 808.0)), Aimed::Nothing);
        assert_eq!(aim((960, 872), &placed, 2, (10.0, 744.0)), Aimed::Nothing);
        assert_eq!(aim((960, 872), &placed, 2, (10.0, 743.0)), page(5.0, 335.5));
    }

    #[test]
    fn beyond_the_last_frame_is_still_the_page() {
        let small = Placement::of(
            (960, 872),
            2,
            Contents {
                frame: Some((100, 100)),
                ..Contents::default()
            },
        );
        assert_eq!(
            aim((960, 872), &small, 2, (900.0, 800.0)),
            page(450.0, 364.0)
        );
    }

    #[test]
    fn a_release_is_placed_on_the_page_wherever_it_is() {
        let placed = placed(true);
        assert_eq!(on_page(&placed, 2, (-20.0, 10.0)), Point::new(-10.0, -31.0));
        assert_eq!(
            on_page(&placed, 2, (2000.0, 900.0)),
            Point::new(1000.0, 414.0)
        );
        assert_eq!(on_page(&placed, 1, (10.0, 46.0)), Point::new(10.0, -26.0));
    }
}
