/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How heavy an element's text is: `font-weight`, read once.
//!
//! Two crates need this answer. Layout measures a run in the face the weight
//! chooses, and paint draws it in the face the weight chooses, and a run is
//! only drawn as wide as it was laid out when both chose the same face. Each
//! used to read the property for itself, and they disagreed: layout took
//! `bold` as 700 and paint took only a number, so every heading the
//! user-agent sheet makes bold was measured in the bold face and drawn in the
//! regular one, narrower than the room it was given (queue item 402). So the
//! reading lives here, beside the style it reads, and both ask it.
//!
//! `bolder` and `lighter` are relative to the parent's weight, and CSS
//! resolves them in the cascade, so a child inherits the number and not the
//! word. This engine does not yet; until it does both read as `normal`, the
//! same in layout as in paint, and queue item 403 is the rest.

use crate::computed::ComputedStyle;

/// The weight CSS calls `normal`, and the initial value.
pub const NORMAL_WEIGHT: u16 = 400;

/// The weight CSS calls `bold`.
pub const BOLD_WEIGHT: u16 = 700;

impl ComputedStyle {
    /// This element's `font-weight` on CSS's 1..=1000 scale.
    ///
    /// A number is rounded and held to the scale, `normal` is 400 and `bold`
    /// 700. A value this engine cannot read is `normal`, which is what an
    /// absent property is.
    pub fn font_weight(&self) -> u16 {
        if let Some(number) = self.number("font-weight") {
            if !number.is_finite() {
                return NORMAL_WEIGHT;
            }
            let clamped = number.clamp(1.0, 1000.0).round();
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "finite, held to one..=1000 and rounded"
            )]
            let weight = clamped as u16;
            return weight;
        }
        match self.get("font-weight").map(str::trim) {
            Some(value) if value.eq_ignore_ascii_case("bold") => BOLD_WEIGHT,
            _ => NORMAL_WEIGHT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Origin, SourcedSheet, resolve};
    use alo_css::{MediaContext, parse_stylesheet};

    /// The weight the sheet `css` gives the element whose `id` is `wanted`.
    fn weight(html: &str, css: &str, wanted: &str) -> Option<u16> {
        let document = alo_dom::parse_document(html);
        let sheet = parse_stylesheet(css);
        let sheets = [SourcedSheet::new(Origin::Author, &sheet)];
        let styles = resolve(&document, &sheets, &MediaContext::default());
        let id = document
            .descendants(document.root())
            .find(|id| document.element(*id).and_then(|e| e.attr("id")) == Some(wanted))?;
        Some(styles.get(id)?.font_weight())
    }

    #[test]
    fn the_keywords_are_the_numbers_they_stand_for() {
        let html = "<p id=a>a</p><p id=b>b</p><p id=c>c</p>";
        let css = "#a { font-weight: bold } #b { font-weight: normal } #c { font-weight: BOLD }";
        assert_eq!(weight(html, css, "a"), Some(700));
        assert_eq!(weight(html, css, "b"), Some(400));
        assert_eq!(weight(html, css, "c"), Some(700));
    }

    #[test]
    fn a_number_is_rounded_and_held_to_the_scale() {
        let html = "<p id=a>a</p><p id=b>b</p><p id=c>c</p><p id=d>d</p>";
        let css = "#a { font-weight: 600 } #b { font-weight: 550.6 } \
                   #c { font-weight: 5000 } #d { font-weight: 0 }";
        assert_eq!(weight(html, css, "a"), Some(600));
        assert_eq!(weight(html, css, "b"), Some(551));
        assert_eq!(weight(html, css, "c"), Some(1000));
        assert_eq!(weight(html, css, "d"), Some(1));
    }

    #[test]
    fn nothing_set_or_nothing_readable_is_normal() {
        let html = "<p id=a>a</p><p id=b>b</p><p id=c>c</p>";
        let css = "#b { font-weight: heavy } #c { font-weight: bolder }";
        assert_eq!(ComputedStyle::new().font_weight(), NORMAL_WEIGHT);
        assert_eq!(weight(html, css, "a"), Some(400));
        assert_eq!(weight(html, css, "b"), Some(400));
        assert_eq!(weight(html, css, "c"), Some(400));
    }

    #[test]
    fn a_child_inherits_its_parents_weight() {
        let html = "<h1 id=a><span id=b>b</span></h1>";
        assert_eq!(weight(html, "h1 { font-weight: bold }", "b"), Some(700));
    }
}
