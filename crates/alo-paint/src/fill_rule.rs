/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which parts of a shape are inside it.
//!
//! A path that crosses itself, or holds one outline inside another, has to be
//! told what *inside* means. CSS never asks — every box, border and letter is
//! filled by the non-zero rule — and SVG's `fill-rule` does (ADR 0022 § 3),
//! so the rule is a property of a fill rather than something the rasteriser
//! assumes.

use core::fmt;

/// How a crossing shape decides its inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FillRule {
    /// Inside wherever the outlines around a point wind a net number of times
    /// other than zero. The rule fonts are drawn with, and the rule for
    /// everything CSS fills.
    #[default]
    NonZero,
    /// Inside wherever a ray from a point crosses an odd number of outlines,
    /// whichever way they run: a star drawn in one stroke has a hole in it.
    EvenOdd,
}

impl FillRule {
    /// The rule `fill-rule` names, or [`None`] for a word that is not one.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.eq_ignore_ascii_case("nonzero") {
            Some(Self::NonZero)
        } else if text.eq_ignore_ascii_case("evenodd") {
            Some(Self::EvenOdd)
        } else {
            None
        }
    }
}

impl fmt::Display for FillRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NonZero => "nonzero",
            Self::EvenOdd => "evenodd",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_rules_are_read_by_name_whatever_their_case() {
        assert_eq!(FillRule::parse("nonzero"), Some(FillRule::NonZero));
        assert_eq!(FillRule::parse(" EvenOdd "), Some(FillRule::EvenOdd));
        assert_eq!(FillRule::parse("odd"), None);
        assert_eq!(FillRule::parse(""), None);
    }

    #[test]
    fn the_rule_nobody_chose_is_the_one_css_uses() {
        assert_eq!(FillRule::default(), FillRule::NonZero);
        assert_eq!(FillRule::EvenOdd.to_string(), "evenodd");
        assert_eq!(FillRule::NonZero.to_string(), "nonzero");
    }
}
