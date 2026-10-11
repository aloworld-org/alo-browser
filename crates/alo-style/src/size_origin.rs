/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a font size came from, and what crossing into or out of
//! `monospace` does to it (ADR 0045).
//!
//! Every engine a person compares us with draws `font-family: monospace`
//! text at 13 px where the page asked for `medium`, and carries a size
//! reached through `em` or a percentage across by 13/16, while a fixed size
//! stays. No specification says so; alo's own pages are drawn by it
//! everywhere else, which is why it is here.
//!
//! So a computed font size is two facts: its pixels, which is all layout,
//! paint and the text stack ever read, and how it was reached, which is the
//! style's business and stops here (ADR 0045 § 6).

use crate::metrics::{DEFAULT_FONT_SIZE, MONOSPACE_FONT_SIZE};

/// How a computed font size was reached (ADR 0045 § 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizeOrigin {
    /// One of the absolute-size keywords, held as its ratio of `medium`: it
    /// is always that ratio of the element's *own* family's medium. The
    /// root's initial `medium` is one.
    Keyword(f32),
    /// Reached through the parent's font — `em`, `ex`, `ch`, `lh`, a
    /// percentage, `smaller` or `larger` — from a parent that was not fixed.
    Relative,
    /// Everything else: the absolute units, `rem` and `rlh`, the viewport
    /// units, any math function whatever it holds, and a relative size of a
    /// fixed parent.
    Fixed,
}

impl Default for SizeOrigin {
    /// The initial font size is `medium`.
    fn default() -> Self {
        SizeOrigin::Keyword(1.0)
    }
}

/// A computed font size and how it was reached.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontSize {
    /// In CSS pixels.
    pub px: f32,
    /// How.
    pub origin: SizeOrigin,
}

impl FontSize {
    /// This size, carried from a parent that is `monospace`-sized or not to
    /// an element that is or is not (ADR 0045 § 4).
    ///
    /// A keyword is read again in the new family, a relative size is scaled
    /// by the ratio of the two mediums, and a fixed size is left alone. On
    /// one side of the line nothing changes at all, so a page that never
    /// writes `monospace` alone computes every size it computed before.
    #[must_use]
    pub fn across(self, from_monospace: bool, to_monospace: bool) -> FontSize {
        if from_monospace == to_monospace {
            return self;
        }
        let (from, to) = (medium(from_monospace), medium(to_monospace));
        let px = match self.origin {
            SizeOrigin::Keyword(ratio) => to * ratio,
            SizeOrigin::Relative => self.px * to / from,
            SizeOrigin::Fixed => self.px,
        };
        FontSize { px, ..self }
    }
}

/// Whether a computed `font-family` makes an element `monospace`-sized
/// (ADR 0045 § 2): one family, and that family the generic `monospace`.
///
/// Nothing else is. `monospace, monospace` is two families, which is the
/// whole reason normalize.css writes it; `"monospace"` quoted names a family
/// rather than the generic; `ui-monospace` is another generic. The value is
/// read after `var()`, so one written through a custom property counts.
pub fn monospace_sized(family: Option<&str>) -> bool {
    family.is_some_and(|family| family.trim().eq_ignore_ascii_case("monospace"))
}

/// The `medium` of an element, given whether it is `monospace`-sized
/// (ADR 0045 § 3).
pub fn medium(monospace: bool) -> f32 {
    if monospace {
        MONOSPACE_FONT_SIZE
    } else {
        DEFAULT_FONT_SIZE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.0001
    }

    fn size(px: f32, origin: SizeOrigin) -> FontSize {
        FontSize { px, origin }
    }

    #[test]
    fn only_the_generic_monospace_alone_is_monospace_sized() {
        for family in ["monospace", "MONOSPACE", " Monospace "] {
            assert!(monospace_sized(Some(family)), "{family}");
        }
        for family in [
            "monospace, monospace",
            "\"Courier New\", monospace",
            "ui-monospace",
            "\"monospace\"",
            "'monospace'",
            "sans-serif",
            "",
        ] {
            assert!(!monospace_sized(Some(family)), "{family}");
        }
        assert!(!monospace_sized(None), "the initial family is not");
    }

    #[test]
    fn the_two_mediums_are_sixteen_and_thirteen() {
        assert!(close(medium(false), 16.0));
        assert!(close(medium(true), 13.0));
    }

    #[test]
    fn a_keyword_is_read_again_in_the_new_family() {
        let large = size(19.2, SizeOrigin::Keyword(6.0 / 5.0));
        assert!(close(large.across(false, true).px, 15.6));
        assert!(close(
            large.across(false, true).across(true, false).px,
            19.2
        ));
        assert_eq!(
            large.across(false, true).origin,
            large.origin,
            "and stays a keyword"
        );
    }

    #[test]
    fn a_relative_size_is_scaled_by_the_two_mediums() {
        let twenty = size(20.0, SizeOrigin::Relative);
        assert!(close(twenty.across(false, true).px, 16.25));
        let twenty_six = size(26.0, SizeOrigin::Relative);
        assert!(close(twenty_six.across(true, false).px, 32.0));
    }

    #[test]
    fn a_fixed_size_stays() {
        let fixed = size(13.76, SizeOrigin::Fixed);
        assert_eq!(fixed.across(false, true), fixed);
        assert_eq!(fixed.across(true, false), fixed);
    }

    #[test]
    fn on_one_side_of_the_line_nothing_changes() {
        for origin in [
            SizeOrigin::Keyword(2.0),
            SizeOrigin::Relative,
            SizeOrigin::Fixed,
        ] {
            let odd = size(17.0, origin);
            assert_eq!(odd.across(false, false), odd);
            assert_eq!(odd.across(true, true), odd);
        }
    }
}
