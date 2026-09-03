/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a line may break.
//!
//! **This is the only file that names `unicode-linebreak`.** Where a line may
//! break is UAX #14, a specification with a table in it, and a table is
//! exactly the kind of thing ADR 0001 says to rent. Breaking at spaces would
//! be wrong for most of the world's writing: Thai has no spaces, German
//! compounds break at hyphens, and a `/` in a URL is a break opportunity that
//! is not a space.
//!
//! What is ours is what to do with the opportunities — which is
//! [`crate::line`].

use unicode_linebreak::{BreakOpportunity, linebreaks};

/// Somewhere a line may end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakPoint {
    /// The byte offset after which the line may end.
    pub offset: usize,
    /// Whether the line **must** end here — after a newline — rather than may.
    pub mandatory: bool,
}

/// Every place a line may end in this text, in order.
///
/// The last opportunity is always the end of the text, which is not a break so
/// much as the fact that the text stops. Callers rely on it being there.
pub fn opportunities(text: &str) -> Vec<BreakPoint> {
    if text.is_empty() {
        return Vec::new();
    }
    linebreaks(text)
        .map(|(offset, opportunity)| BreakPoint {
            offset,
            mandatory: opportunity == BreakOpportunity::Mandatory,
        })
        .collect()
}

/// Whether a line may end at this byte offset.
pub fn may_break_at(text: &str, offset: usize) -> bool {
    opportunities(text)
        .iter()
        .any(|point| point.offset == offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offsets(text: &str) -> Vec<usize> {
        opportunities(text)
            .into_iter()
            .map(|point| point.offset)
            .collect()
    }

    #[test]
    fn nothing_has_nowhere_to_break() {
        assert!(opportunities("").is_empty());
    }

    #[test]
    fn a_line_may_break_after_a_space_and_at_the_end() {
        assert_eq!(offsets("one two"), vec![4, 7]);
        assert_eq!(
            offsets("one"),
            vec![3],
            "the end of the text is always an opportunity",
        );
    }

    #[test]
    fn a_newline_is_a_break_that_must_happen() {
        let points = opportunities("one\ntwo");
        assert_eq!(
            points.first().copied(),
            Some(BreakPoint {
                offset: 4,
                mandatory: true
            })
        );
        assert_eq!(
            points.last().copied(),
            Some(BreakPoint {
                offset: 7,
                mandatory: true,
            }),
            "the end of the text is mandatory too: the text stops",
        );
    }

    #[test]
    fn breaking_is_not_the_same_as_finding_spaces() {
        // A hyphen is a break opportunity and is not a space.
        assert_eq!(offsets("well-known"), vec![5, 10]);
        // And a slash in a path is one as well.
        assert!(may_break_at("a/b", 2));
        // While the middle of a word is not.
        assert!(!may_break_at("hello", 2));
    }

    #[test]
    fn a_run_of_spaces_is_one_opportunity_rather_than_several() {
        assert_eq!(offsets("one   two"), vec![6, 9]);
    }

    #[test]
    fn the_offsets_are_bytes_and_land_on_character_boundaries() {
        let text = "héllo wörld";
        for point in opportunities(text) {
            assert!(
                text.is_char_boundary(point.offset),
                "{} is not a boundary in {text:?}",
                point.offset,
            );
        }
    }
}
