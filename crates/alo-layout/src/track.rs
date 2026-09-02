//! Grid tracks: the rows and columns a grid is made of.
//!
//! `grid-template-columns: repeat(3, minmax(0, 1fr))` is one value with three
//! grammars nested in it, and every one of them has to be read exactly or the
//! grid is the wrong shape. So the grammar is small, written out, and refuses
//! what it does not have.
//!
//! **What is not here**, and is refused rather than approximated:
//! `grid-template-areas`, named grid lines, and `subgrid`. They are real CSS
//! and they are not what alo's own screens are written with; the method this
//! repository uses is to let a screen that fails schedule the work, and a
//! refusal is recorded every time one is met so that a failing screen says so.

use crate::sizing::function_argument;
use alo_value::{LengthPercentage, is_keyword, parse_length_percentage, parse_number};
use core::fmt;

/// How big one track may be, at one end of a range.
#[derive(Debug, Clone, PartialEq)]
pub enum TrackSize {
    /// A length or a percentage.
    Length(LengthPercentage),
    /// `auto`.
    Auto,
    /// `min-content`.
    MinContent,
    /// `max-content`.
    MaxContent,
    /// A share of what is left over: the `2` of `2fr`.
    Fraction(f32),
}

impl TrackSize {
    /// Read one track size.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if is_keyword(text, "auto") {
            return Some(TrackSize::Auto);
        }
        if is_keyword(text, "min-content") {
            return Some(TrackSize::MinContent);
        }
        if is_keyword(text, "max-content") {
            return Some(TrackSize::MaxContent);
        }
        if let Some(number) = text.strip_suffix("fr").or_else(|| text.strip_suffix("FR")) {
            let share = parse_number(number)?;
            // A negative share of the leftover space is not a share of it.
            return (share >= 0.0).then_some(TrackSize::Fraction(share));
        }
        Some(TrackSize::Length(parse_length_percentage(text)?))
    }
}

impl fmt::Display for TrackSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrackSize::Length(value) => write!(f, "{value}"),
            TrackSize::Auto => f.write_str("auto"),
            TrackSize::MinContent => f.write_str("min-content"),
            TrackSize::MaxContent => f.write_str("max-content"),
            TrackSize::Fraction(share) => write!(f, "{share}fr"),
        }
    }
}

/// One track of a grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// How small it may get.
    pub min: TrackSize,
    /// How large it may get.
    pub max: TrackSize,
}

impl Track {
    /// A track that is exactly one size.
    pub fn exactly(size: TrackSize) -> Self {
        Self {
            min: size.clone(),
            max: size,
        }
    }

    /// Read one track, which may be a `minmax()` or a single size.
    pub fn parse(text: &str) -> Option<Self> {
        if let Some(inner) = function_argument(text, "minmax") {
            let (min, max) = split_once_at_top_level(inner, ',')?;
            return Some(Self {
                min: TrackSize::parse(min)?,
                max: TrackSize::parse(max)?,
            });
        }
        Some(Self::exactly(TrackSize::parse(text)?))
    }
}

impl fmt::Display for Track {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.min == self.max {
            write!(f, "{}", self.max)
        } else {
            write!(f, "minmax({}, {})", self.min, self.max)
        }
    }
}

/// How many times a `repeat()` repeats.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RepeatCount {
    /// A number written out.
    Times(u16),
    /// As many as fit, keeping the empty ones.
    AutoFill,
    /// As many as fit, collapsing the empty ones.
    AutoFit,
}

/// One entry of a track list: a track, or a repetition of several.
#[derive(Debug, Clone, PartialEq)]
pub enum TrackListEntry {
    /// One track.
    Single(Track),
    /// A repetition.
    Repeat {
        /// How many times.
        count: RepeatCount,
        /// What is repeated.
        tracks: Vec<Track>,
    },
}

/// A whole `grid-template-rows` or `grid-template-columns`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TrackList {
    /// The entries, in order.
    pub entries: Vec<TrackListEntry>,
}

impl TrackList {
    /// Read a track list, or [`None`] if any part of it is something this
    /// engine does not implement.
    ///
    /// All or nothing on purpose: half a track list is a grid of the wrong
    /// shape, and a grid of the wrong shape is harder to diagnose than no grid
    /// at all.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() || is_keyword(text, "none") {
            return Some(Self::default());
        }
        let mut entries = Vec::new();
        for part in split_at_top_level(text) {
            entries.push(parse_entry(&part)?);
        }
        (!entries.is_empty()).then_some(Self { entries })
    }

    /// Whether the list says nothing, which is what `none` means.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl fmt::Display for TrackList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.entries.is_empty() {
            return f.write_str("none");
        }
        for (index, entry) in self.entries.iter().enumerate() {
            if index > 0 {
                f.write_str(" ")?;
            }
            match entry {
                TrackListEntry::Single(track) => write!(f, "{track}")?,
                TrackListEntry::Repeat { count, tracks } => {
                    f.write_str("repeat(")?;
                    match count {
                        RepeatCount::Times(times) => write!(f, "{times}")?,
                        RepeatCount::AutoFill => f.write_str("auto-fill")?,
                        RepeatCount::AutoFit => f.write_str("auto-fit")?,
                    }
                    for track in tracks {
                        write!(f, ", {track}")?;
                    }
                    f.write_str(")")?;
                }
            }
        }
        Ok(())
    }
}

fn parse_entry(text: &str) -> Option<TrackListEntry> {
    if let Some(inner) = function_argument(text, "repeat") {
        let (count, rest) = split_once_at_top_level(inner, ',')?;
        let count = count.trim();
        let count = if is_keyword(count, "auto-fill") {
            RepeatCount::AutoFill
        } else if is_keyword(count, "auto-fit") {
            RepeatCount::AutoFit
        } else {
            let times = parse_number(count)?;
            // A repetition of nothing, or of half a track, is not a value.
            if times < 1.0 || times.fract() != 0.0 || times > f32::from(u16::MAX) {
                return None;
            }
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "checked above to be a whole number in range"
            )]
            RepeatCount::Times(times as u16)
        };
        let mut tracks = Vec::new();
        for part in split_at_top_level(rest) {
            tracks.push(Track::parse(&part)?);
        }
        return (!tracks.is_empty()).then_some(TrackListEntry::Repeat { count, tracks });
    }
    Some(TrackListEntry::Single(Track::parse(text)?))
}

/// Split on whitespace, but not inside parentheses.
///
/// `minmax(0, 1fr) auto` is two tracks; the space inside the `minmax` is not a
/// separator. Splitting on whitespace without counting brackets is the mistake
/// this exists to avoid.
fn split_at_top_level(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for character in text.chars() {
        match character {
            '(' => {
                depth += 1;
                current.push(character);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(character);
            }
            _ if character.is_whitespace() && depth == 0 => {
                if !current.is_empty() {
                    parts.push(core::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

/// Split once on a separator that is not inside parentheses.
fn split_once_at_top_level(text: &str, separator: char) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    for (index, character) in text.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if character == separator && depth == 0 => {
                return Some((
                    text.get(..index)?,
                    text.get(index + character.len_utf8()..)?,
                ));
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(text: &str) -> Option<String> {
        Some(TrackList::parse(text)?.to_string())
    }

    #[test]
    fn a_list_of_plain_sizes_reads_as_written() {
        assert_eq!(written("100px 1fr auto"), Some("100px 1fr auto".to_owned()));
        assert_eq!(written("50% 50%"), Some("50% 50%".to_owned()));
        assert_eq!(
            written("min-content max-content"),
            Some("min-content max-content".to_owned()),
        );
    }

    #[test]
    fn none_and_nothing_are_an_empty_list() {
        assert!(TrackList::parse("none").expect("none").is_empty());
        assert!(TrackList::parse("").expect("empty").is_empty());
        assert_eq!(written("none"), Some("none".to_owned()));
    }

    #[test]
    fn minmax_keeps_both_ends() {
        let list = TrackList::parse("minmax(0, 1fr)").expect("minmax");
        assert_eq!(
            list.entries,
            vec![TrackListEntry::Single(Track {
                min: TrackSize::Length(alo_value::LengthPercentage::Length(
                    alo_value::Length::ZERO,
                )),
                max: TrackSize::Fraction(1.0),
            })],
        );
        assert_eq!(list.to_string(), "minmax(0px, 1fr)");
    }

    #[test]
    fn a_space_inside_a_function_is_not_a_separator() {
        let list = TrackList::parse("minmax(0, 1fr) auto").expect("two tracks");
        assert_eq!(list.entries.len(), 2, "not four");
    }

    #[test]
    fn repeat_repeats_what_it_is_given() {
        let list = TrackList::parse("repeat(3, 1fr)").expect("repeat");
        assert_eq!(
            list.entries,
            vec![TrackListEntry::Repeat {
                count: RepeatCount::Times(3),
                tracks: vec![Track::exactly(TrackSize::Fraction(1.0))],
            }],
        );
        assert_eq!(list.to_string(), "repeat(3, 1fr)");

        let several = TrackList::parse("repeat(2, 100px 1fr)").expect("two per repetition");
        assert_eq!(several.to_string(), "repeat(2, 100px, 1fr)");
    }

    #[test]
    fn repeat_takes_the_two_words_that_mean_as_many_as_fit() {
        assert_eq!(
            written("repeat(auto-fill, minmax(200px, 1fr))"),
            Some("repeat(auto-fill, minmax(200px, 1fr))".to_owned()),
        );
        assert_eq!(
            written("repeat(auto-fit, 1fr)"),
            Some("repeat(auto-fit, 1fr)".to_owned()),
        );
    }

    #[test]
    fn a_repetition_of_a_fraction_of_a_track_is_not_a_value() {
        for text in [
            "repeat(0, 1fr)",
            "repeat(1.5, 1fr)",
            "repeat(-2, 1fr)",
            "repeat(3)",
        ] {
            assert_eq!(TrackList::parse(text), None, "{text}");
        }
    }

    #[test]
    fn what_this_engine_does_not_implement_takes_the_whole_list_with_it() {
        for text in [
            "[full-start] 1fr [full-end]",
            "subgrid",
            "1fr banana",
            "repeat(2, 1fr) [line]",
            "50vw",
        ] {
            assert_eq!(
                TrackList::parse(text),
                None,
                "{text}: half a track list is a grid of the wrong shape",
            );
        }
    }

    #[test]
    fn a_negative_fraction_is_not_a_share_of_anything() {
        assert_eq!(TrackSize::parse("-1fr"), None);
        assert_eq!(TrackSize::parse("0fr"), Some(TrackSize::Fraction(0.0)));
        assert_eq!(TrackSize::parse("2.5fr"), Some(TrackSize::Fraction(2.5)));
    }

    #[test]
    fn splitting_counts_brackets_rather_than_spaces() {
        assert_eq!(
            split_at_top_level("a (b c) d"),
            vec!["a".to_owned(), "(b c)".to_owned(), "d".to_owned()],
        );
        assert_eq!(
            split_once_at_top_level("f(1, 2), rest", ','),
            Some(("f(1, 2)", " rest")),
        );
        assert_eq!(split_once_at_top_level("no separator", ','), None);
    }
}
