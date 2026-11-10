/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ★ What a person pressing a point of a page's window would press, asked
//! of Chrome (queue item 922).
//!
//! Never a way of choosing what to act on (ADR 0002): only of checking that
//! a press lands on the thing named, or of telling what a person's press on
//! a card is over.
//!
//! # The window's point and the page's
//!
//! A point here is in the **window**, in CSS pixels: where Chrome says a
//! node is drawn (`DOM.getContentQuads`) and where a mouse is pressed
//! (`Input.dispatchMouseEvent`). `DOM.getNodeForLocation` takes its point
//! in the **page** instead — the window's point plus how far the page is
//! scrolled, which Chrome tells as its layout viewport's `pageX` and
//! `pageY` (`Page.getLayoutMetrics`). Measured against Chrome 157.0.8103.50
//! (10 November 2026, the session scratchpad's `p922/`): on a page scrolled
//! 1408 across and 1178 down, a button Chrome drew at 400…500 × 330…370 of
//! the window was *No node found at given location* at 450, 350, and was
//! the button at 1858, 1528. A page at its top has the two the same, so
//! only a scrolled page told them apart — Toolshop's *Add to cart* was
//! refused as covered by nothing in the middle of the card.
//!
//! Measured at a page scale of one: a page pinched larger in a card is not.

use crate::chrome::Failed;
use crate::chrome_protocol::{self, Command};
use crate::chrome_tab::Tab;

/// The DOM node a person pressing the window's point `x`, `y` of `tab`'s
/// page would press, by Chrome's `backendNodeId`: [`None`] when Chrome
/// finds nothing there, or the point is too far away to be a page's.
///
/// # Errors
///
/// [`Failed`], when asking Chrome fails or its answer cannot be read.
pub(crate) fn node_at(tab: &mut Tab<'_>, x: i64, y: i64) -> Result<Option<u64>, Failed> {
    let metrics = tab.ask(&Command::Window)?;
    let scrolled = chrome_protocol::scrolled(&metrics).map_err(Failed::Malformed)?;
    let Some((x, y)) = in_the_page(x, y, scrolled) else {
        return Ok(None);
    };
    match tab.ask(&Command::NodeAt { x, y }) {
        Ok(result) => Ok(Some(
            chrome_protocol::node_at(&result).map_err(Failed::Malformed)?,
        )),
        Err(Failed::Declined(_)) => Ok(None),
        Err(why) => Err(why),
    }
}

/// The whole CSS pixel of the page holding the window's point `x`, `y`,
/// with the page scrolled `left` across and `top` down — or [`None`] for a
/// point too far away to be a page's.
fn in_the_page(x: i64, y: i64, (left, top): (f64, f64)) -> Option<(i64, i64)> {
    Some((pixel(x, left)?, pixel(y, top)?))
}

/// The whole pixel holding `at` moved by `by`: within ±2⁵³, where an `f64`
/// holds every whole number exactly.
fn pixel(at: i64, by: f64) -> Option<i64> {
    const EXACT: f64 = 9_007_199_254_740_992.0;
    let at = exact(at)?;
    let moved = (at + by).floor();
    (moved.is_finite() && moved.abs() < EXACT).then(|| whole(moved))
}

/// `at` as an `f64`, when it is within ±2⁵³ and so held exactly.
fn exact(at: i64) -> Option<f64> {
    const MOST: i64 = 1 << 53;
    #[expect(
        clippy::cast_precision_loss,
        reason = "only whole numbers inside ±2⁵³, which an f64 holds exactly"
    )]
    (at.abs() < MOST).then_some(at as f64)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "`pixel` passes only whole numbers inside ±2⁵³, which an i64 holds exactly"
)]
fn whole(point: f64) -> i64 {
    point as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_of_the_window_is_moved_by_how_far_the_page_is_scrolled() {
        // Measured against Chrome 157: the button at 450, 350 of a window
        // over a page scrolled 1408 across and 1178 down.
        assert_eq!(in_the_page(450, 350, (1408.0, 1178.0)), Some((1858, 1528)));
        assert_eq!(in_the_page(513, 349, (0.0, 547.0)), Some((513, 896)));
        assert_eq!(
            in_the_page(35, 249, (0.0, 0.0)),
            Some((35, 249)),
            "a page at its top"
        );
    }

    #[test]
    fn a_page_scrolled_part_of_a_pixel_is_the_pixel_that_holds_the_point() {
        // A page on a screen at two scrolls in halves of a CSS pixel.
        assert_eq!(in_the_page(10, 20, (0.5, 100.5)), Some((10, 120)));
        assert_eq!(in_the_page(-3, 0, (0.25, -0.5)), Some((-3, -1)));
    }

    #[test]
    fn a_point_too_far_away_is_no_page_s() {
        assert_eq!(in_the_page(1 << 53, 0, (0.0, 0.0)), None);
        assert_eq!(in_the_page(0, 0, (f64::NAN, 0.0)), None);
        assert_eq!(in_the_page(0, 0, (0.0, f64::INFINITY)), None);
        assert_eq!(in_the_page(0, 0, (1e300, 0.0)), None);
    }
}
