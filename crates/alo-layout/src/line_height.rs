/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The line height a line box uses: the computed one, on a grid of 1/64 px
//! (ADR 0050 § 3).
//!
//! The computed value stays exact. `getComputedStyle` reads `18.221px` for
//! `137%` at 13.3 px, and so does this engine's cascade, which is what an
//! `lh` unit is measured in. A line box uses that value placed on Chrome's
//! layout unit of 1/64 px, and how it is placed depends on how it was
//! written, measured in Chrome 154:
//!
//! - **A number** is multiplied by the font size rounded to 1/64, and the
//!   product is **floored** to 1/64. At 13.309 px a `line-height` of 1 is
//!   13.3125: the size rounded first, where flooring the size itself would
//!   give 13.296875.
//! - **A length or a percentage** is **rounded** to 1/64. `17.34px` is
//!   17.34375.
//!
//! The grid is the screen's, 1/64 of a device pixel (ADR 0057 § 2), so at
//! a scale of two both rules round onto 1/128 of a CSS pixel.
//!
//! So this reads the computed text as well as the number: a number computes
//! to itself and inherits as one (`alo_style`'s `record_computed_font`),
//! and everything else computes to pixels.

use crate::layout_unit::{floored, rounded};
use alo_style::ComputedStyle;

/// The line height a line box of this style uses, in CSS pixels, or
/// [`None`] for `normal`.
pub fn used_line_height(style: &ComputedStyle) -> Option<f32> {
    let computed = style.set_line_height()?;
    let scale = style.metrics().scale();
    let used = match style.number("line-height") {
        Some(multiple) => floored(rounded(style.font_size(), scale) * multiple, scale),
        None => rounded(computed, scale),
    };
    // A value past the grid's range saturates at its edge, as Chrome's does
    // (ADR 0054 § 5); one that is somehow not a number keeps its computed
    // value, which the cascade has already checked is finite.
    Some(if used.is_finite() { used } else { computed })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_unit::UNITS_PER_PX;
    use alo_css::{MediaContext, parse_stylesheet};
    use alo_dom::parse_document;
    use alo_style::{Origin, SourcedSheet, resolve};

    /// The line height a `p` with these declarations uses.
    fn used(declarations: &str) -> Option<f32> {
        used_at(declarations, 1.0)
    }

    /// The same, on a screen of `scale` device pixels to the CSS pixel.
    fn used_at(declarations: &str, scale: f32) -> Option<f32> {
        let document = parse_document("<p id=x>text</p>");
        let author = parse_stylesheet(&format!("p {{ {declarations} }}"));
        let sheets = [SourcedSheet::new(Origin::Author, &author)];
        let device = MediaContext::default().at_scale(scale);
        let styles = resolve(&document, &sheets, &device);
        let node = document
            .descendants(document.root())
            .find(|id| document.element(*id).and_then(|e| e.attr("id")) == Some("x"))?;
        used_line_height(styles.get(node)?)
    }

    /// `units` sixty-fourths of a pixel, as measured in Chrome 154.
    fn sixty_fourths(units: f32) -> f32 {
        units / UNITS_PER_PX
    }

    #[test]
    fn normal_is_no_set_line_height() {
        assert_eq!(used("font-size: 16px"), None);
        assert_eq!(used("font-size: 16px; line-height: normal"), None);
    }

    #[test]
    fn a_number_is_floored_from_the_font_size_rounded() {
        // ADR 0050 § 3, each as Chrome 154 measures it.
        assert_eq!(
            used("font-size: 13.309px; line-height: 1"),
            Some(sixty_fourths(852.0)),
            "the size rounded to 852, where floored it would be 851",
        );
        assert_eq!(
            used("font-size: 16px; line-height: 1.23"),
            Some(sixty_fourths(1259.0)),
            "1259.52 floored",
        );
        assert_eq!(
            used("font-size: 13.3px; line-height: 1.37"),
            Some(sixty_fourths(1165.0)),
            "1165.87 floored",
        );
        assert_eq!(
            used("font-size: 26.7px; line-height: 1.23"),
            Some(sixty_fourths(2102.0))
        );
        assert_eq!(
            used("font-size: 8px; line-height: 1.15"),
            Some(sixty_fourths(588.0))
        );
        assert_eq!(used("font-size: 16px; line-height: 1.5"), Some(24.0));
    }

    #[test]
    fn a_length_or_a_percentage_is_rounded() {
        assert_eq!(
            used("font-size: 16px; line-height: 17.34px"),
            Some(sixty_fourths(1110.0)),
            "1109.76 rounded",
        );
        assert_eq!(
            used("font-size: 16px; line-height: 18.01px"),
            Some(sixty_fourths(1153.0))
        );
        assert_eq!(
            used("font-size: 13.3px; line-height: 1.5em"),
            Some(sixty_fourths(1277.0)),
            "1276.8 rounded",
        );
        assert_eq!(
            used("font-size: 13.32px; line-height: 137%"),
            Some(18.25),
            "1167.9 rounded",
        );
    }

    #[test]
    fn at_a_scale_of_two_the_grid_is_a_hundred_and_twenty_eighth() {
        // ADR 0057 § 2, as Chrome 154 lays it out under
        // `--force-device-scale-factor=2`: `17.6px` is 1,126.4 sixty-fourths,
        // rounded to 1,126 at one and to 1,126.5 at two.
        assert_eq!(
            used_at("font-size: 16px; line-height: 17.6px", 1.0),
            Some(sixty_fourths(1126.0))
        );
        assert_eq!(
            used_at("font-size: 16px; line-height: 17.6px", 2.0),
            Some(sixty_fourths(1126.5))
        );
        // A number at two: 16 × 1.23 is 1,259.52 sixty-fourths, floored onto
        // 1/128 px at 1,259.5.
        assert_eq!(
            used_at("font-size: 16px; line-height: 1.23", 2.0),
            Some(sixty_fourths(1259.5))
        );
    }

    #[test]
    fn a_line_height_past_the_grids_range_saturates_at_its_edge() {
        let enormous = used("font-size: 16px; line-height: 1e37px");
        assert!(
            enormous.is_some_and(|height| height.is_finite() && height < 33_554_432.0),
            "{enormous:?}"
        );
    }
}
