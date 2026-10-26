/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a word too wide for its line is cut: `overflow-wrap` (CSS Text 3
//! § 5.5, queue item 666).
//!
//! A line breaks where UAX #14 says it may, and a word with no such place in
//! it is laid down whole and sticks out. `overflow-wrap: break-word` and
//! `anywhere` say otherwise: where nothing else would let the line break, it
//! may break between any two **clusters** — what the shaper draws as one,
//! so never inside a letter. Chrome 155 was measured with the corpus's
//! `DejaVu Sans` at 16 px in boxes 50 wide:
//!
//! - A word that does not fit goes to the next line first, by the break
//!   before it, and is cut only if it still does not fit there: `ab
//!   cccccccccccc dd` is `ab ` · `ccccc` · `ccccc` · `cc dd`, and what is
//!   left of the word carries on its line with what follows.
//! - A word that starts after another box's text on a line the break before
//!   it cannot leave is cut where it stands: `aaa<span>bbbbbbbbbb</span>` is
//!   `aaabb` · `bbbb` · `bbbb`.
//! - Each line holds as many clusters as fit, and at least one.
//! - It cuts only where a line may wrap: under `nowrap` the word sticks out.
//! - **The two values differ in one place**: how narrow a box may be. A cut
//!   `break-word` makes is not counted in a box's min-content width, so a
//!   `width: min-content` box of `ab cccccccccccc` is the long word wide;
//!   one `anywhere` makes is, so the same box is its widest letter wide
//!   (CSS Text 3 § 5.5, and Chrome agrees). A box of a definite width of
//!   nought is cut under either, a letter a line.
//!
//! `word-wrap`, the property's legacy name, is not read (queue item 668).

/// What `overflow-wrap` says about a word too wide for its line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverflowWrap {
    /// It is laid down whole and sticks out.
    #[default]
    Normal,
    /// It is cut between clusters where nothing else lets the line break,
    /// and the cut is not counted in the box's min-content width.
    BreakWord,
    /// The same, and the cut counts in the box's min-content width.
    Anywhere,
}

impl OverflowWrap {
    /// What an `overflow-wrap` value says, or [`None`] for one this engine
    /// does not implement.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        [
            (OverflowWrap::Normal, "normal"),
            (OverflowWrap::BreakWord, "break-word"),
            (OverflowWrap::Anywhere, "anywhere"),
        ]
        .into_iter()
        .find(|(_, name)| text.eq_ignore_ascii_case(name))
        .map(|(value, _)| value)
    }

    /// Whether a word too wide for its line is cut, when the lines are laid
    /// out to find the box's min-content width (`narrowest`) or not.
    pub fn cuts(self, narrowest: bool) -> bool {
        match self {
            OverflowWrap::Normal => false,
            OverflowWrap::BreakWord => !narrowest,
            OverflowWrap::Anywhere => true,
        }
    }
}

/// What the line builder knows about cutting words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Cutting {
    /// Whether the lines are laid out for the box's min-content width.
    pub(crate) narrowest: bool,
    /// Whether a word has been cut that the min-content width does not
    /// count cut.
    pub(crate) uncounted: bool,
}

/// Where a word is cut for the line it is being laid on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Cut {
    /// What is left of it fits: it is laid down whole.
    Fits,
    /// Not one cluster of it fits on a line that already holds something:
    /// the line ends before it.
    Before,
    /// The line ends after the cluster ending at this byte, which is this
    /// far along the word.
    After(usize, f32),
}

/// Where the rest of a word is cut, given where each of its clusters ends
/// and how far along it that is (`clusters`, as
/// [`crate::MeasureText::cluster_ends`] gives them), the byte and the
/// distance it has been laid down to (`from`), whether something of that
/// width still fits on the line (`fits`), and whether the line holds nothing
/// yet (`empty`).
///
/// A line holding nothing takes a cluster whether it fits or not: there is
/// nowhere else for it.
pub(crate) fn cut(
    clusters: &[(usize, f32)],
    from: (usize, f32),
    fits: impl Fn(f32) -> bool,
    empty: bool,
) -> Cut {
    let (at, along) = from;
    let rest: Vec<(usize, f32)> = clusters
        .iter()
        .copied()
        .filter(|&(end, _)| end > at)
        .collect();
    let Some(&(last, whole)) = rest.last() else {
        return Cut::Fits;
    };
    if fits(whole - along) {
        return Cut::Fits;
    }
    let fitting = rest
        .iter()
        .take_while(|&&(_, reached)| fits(reached - along))
        .last()
        .copied();
    match (fitting, rest.first().copied()) {
        (Some((end, reached)), _) => Cut::After(end, reached),
        (None, Some((end, reached))) if empty && end < last => Cut::After(end, reached),
        (None, Some(_)) if empty => Cut::Fits,
        _ => Cut::Before,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Five letters ten wide each.
    fn word() -> Vec<(usize, f32)> {
        (1..=5u8)
            .map(|end| (usize::from(end), f32::from(end) * 10.0))
            .collect()
    }

    fn within(room: f32) -> impl Fn(f32) -> bool {
        move |width| width <= room
    }

    #[test]
    fn its_values_are_read() {
        assert_eq!(OverflowWrap::parse("normal"), Some(OverflowWrap::Normal));
        assert_eq!(
            OverflowWrap::parse(" Break-Word "),
            Some(OverflowWrap::BreakWord)
        );
        assert_eq!(
            OverflowWrap::parse("anywhere"),
            Some(OverflowWrap::Anywhere)
        );
        assert_eq!(OverflowWrap::parse("break-all"), None);
        assert_eq!(OverflowWrap::default(), OverflowWrap::Normal);
    }

    #[test]
    fn only_anywhere_cuts_a_word_for_the_narrowest_width() {
        assert!(!OverflowWrap::Normal.cuts(false));
        assert!(!OverflowWrap::Normal.cuts(true));
        assert!(OverflowWrap::BreakWord.cuts(false));
        assert!(!OverflowWrap::BreakWord.cuts(true));
        assert!(OverflowWrap::Anywhere.cuts(false));
        assert!(OverflowWrap::Anywhere.cuts(true));
    }

    #[test]
    fn a_word_that_fits_is_not_cut() {
        assert_eq!(cut(&word(), (0, 0.0), within(50.0), true), Cut::Fits);
        assert_eq!(cut(&word(), (3, 30.0), within(20.0), true), Cut::Fits);
        assert_eq!(cut(&[], (0, 0.0), within(0.0), true), Cut::Fits);
    }

    #[test]
    fn a_line_holds_as_many_clusters_as_fit() {
        assert_eq!(
            cut(&word(), (0, 0.0), within(25.0), true),
            Cut::After(2, 20.0)
        );
        assert_eq!(
            cut(&word(), (2, 20.0), within(25.0), true),
            Cut::After(4, 40.0)
        );
        assert_eq!(cut(&word(), (4, 40.0), within(25.0), true), Cut::Fits);
    }

    #[test]
    fn an_empty_line_takes_one_cluster_that_does_not_fit() {
        assert_eq!(
            cut(&word(), (0, 0.0), within(0.0), true),
            Cut::After(1, 10.0)
        );
        // The last is the end of the word, which is no cut at all.
        assert_eq!(cut(&word(), (4, 40.0), within(0.0), true), Cut::Fits);
    }

    #[test]
    fn a_line_holding_something_ends_before_a_cluster_that_does_not_fit() {
        assert_eq!(cut(&word(), (0, 0.0), within(5.0), false), Cut::Before);
        assert_eq!(
            cut(&word(), (0, 0.0), within(15.0), false),
            Cut::After(1, 10.0)
        );
    }
}
