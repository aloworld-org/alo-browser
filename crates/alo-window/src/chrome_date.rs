/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A date put into Chrome's date field, a segment at a time (queue item
//! 632, ADR 0042 § 4, ADR 0058 § 4).
//!
//! An agent puts a date into a date field as a valid date string,
//! `2026-10-12`, whichever renderer drew the page. Chrome builds the field
//! of three segments — a day, a month and a year — that typing's text
//! (`Input.insertText`) does not reach, so the date is carried in as a
//! person types it: each segment given the keyboard and its part typed in
//! as digits.
//!
//! # The order is read, never guessed
//!
//! Which segment comes first is Chrome's, from the machine's region:
//! `dd/mm/yyyy` on the Mac this was measured on, `mm/dd/yyyy` on another.
//! This browser does not guess it. Measured against Chrome 155.0.8059.86,
//! each segment is in the page's accessibility tree as a `spinbutton` under
//! the field, in the order it is laid out, and describes a shadow element
//! whose `pseudo` attribute names which part it is —
//! `-webkit-datetime-edit-day-field`, `-month-field`, `-year-field` — in
//! every language Chrome speaks, where the segment's name (`Day`) is
//! Chrome's words and the region's. So [`segments`] finds them in the tree
//! and [`Part::of_pseudo`] says which each is, and a field that does not
//! show one day, one month and one year is not typed into at all.
//!
//! # What Chrome's field holds, and what it does not
//!
//! Measured: a segment given the keyboard starts again at the first digit
//! typed, so `10` typed into a day holding `29` is `10`; a part typed in
//! full — `05`, `2026` — is that part; Backspace empties a segment, and the
//! field is empty while any segment is; a date outside the field's `min`
//! and `max` is held, and fails its range, as the engine holds it. Two valid
//! date strings are **not** held as written, and are refused before a key
//! is sent ([`Untypable`]): one past `275760-09-13`, the last day Chrome's
//! field holds, which leaves it empty or holds another; and a year of more
//! than four digits that starts with `0`, which Chrome writes without its
//! zeros, so that `02026-01-02` would be held as `2026-01-02`.

use crate::chrome_protocol::AxNode;
use core::fmt;
use std::collections::HashMap;

/// A part of a date, as a segment of Chrome's date field holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Part {
    /// The day of the month.
    Day,
    /// The month.
    Month,
    /// The year.
    Year,
}

impl Part {
    /// The part a segment is, by the `pseudo` attribute of the shadow
    /// element it describes, or [`None`] for anything else.
    pub fn of_pseudo(pseudo: &str) -> Option<Self> {
        match pseudo {
            "-webkit-datetime-edit-day-field" => Some(Self::Day),
            "-webkit-datetime-edit-month-field" => Some(Self::Month),
            "-webkit-datetime-edit-year-field" => Some(Self::Year),
            _ => None,
        }
    }
}

/// Why a date was not typed into Chrome's date field: nothing was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Untypable {
    /// It is past `275760-09-13`, the last day Chrome's field holds.
    PastTheLastDay {
        /// The date, as the agent wrote it.
        text: String,
    },
    /// Its year has more than four digits and starts with `0`, which
    /// Chrome's field holds without its zeros.
    LeadingZeros {
        /// The date, as the agent wrote it.
        text: String,
    },
    /// The field does not show one day, one month and one year segment.
    Segments {
        /// How many segments the field shows.
        found: usize,
    },
}

impl fmt::Display for Untypable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PastTheLastDay { text } => write!(
                f,
                "{text:?} is past 275760-09-13, the last day Chrome's date field holds"
            ),
            Self::LeadingZeros { text } => write!(
                f,
                "Chrome's date field would hold the year of {text:?} without its leading zeros"
            ),
            Self::Segments { found } => write!(
                f,
                "Chrome's date field shows {found} segments, not one each of a day, a month and a year"
            ),
        }
    }
}

/// What each part of a date is typed as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digits {
    year: String,
    month: String,
    day: String,
}

impl Digits {
    /// The parts of `text`, which [`alo_dom::date::is_valid_date_string`]
    /// says is a date, as Chrome's segments are typed: each as it is
    /// written — `05`, `0005` — and so as it is held.
    ///
    /// # Errors
    ///
    /// [`Untypable`] for a date Chrome's field does not hold as written,
    /// and for a text that is not a valid date string, which an agent's
    /// decision never passes ([`Untypable::PastTheLastDay`], as the one
    /// nothing could hold).
    pub fn of(text: &str) -> Result<Self, Untypable> {
        let past = || Untypable::PastTheLastDay {
            text: text.to_owned(),
        };
        if !alo_dom::date::is_valid_date_string(text) {
            return Err(past());
        }
        let mut parts = text.split('-');
        let (Some(year), Some(month), Some(day)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(past());
        };
        if year.len() > 4 && year.starts_with('0') {
            return Err(Untypable::LeadingZeros {
                text: text.to_owned(),
            });
        }
        // Without leading zeros, seven digits or more is past 275760.
        let holds = year.len() <= 6
            && match (year.parse::<u32>(), month.parse::<u8>(), day.parse::<u8>()) {
                (Ok(year), Ok(month), Ok(day)) => (year, month, day) <= (275_760, 9, 13),
                _ => false,
            };
        if !holds {
            return Err(past());
        }
        Ok(Self {
            year: year.to_owned(),
            month: month.to_owned(),
            day: day.to_owned(),
        })
    }

    /// The digits `part` is typed as.
    pub fn of_part(&self, part: Part) -> &str {
        match part {
            Part::Year => &self.year,
            Part::Month => &self.month,
            Part::Day => &self.day,
        }
    }
}

/// The DOM nodes of the segments of the date field `dom`, by Chrome's
/// `backendNodeId`, in the order `nodes` — Chrome's whole tree — holds
/// them, which is the order they are laid out in: the `spinbutton`s at or
/// under the node describing `dom`, whether Chrome ignores the nodes
/// between or not.
pub fn segments(nodes: &[AxNode], dom: u64) -> Vec<u64> {
    let by_id: HashMap<&str, &AxNode> = nodes.iter().map(|node| (node.id.as_str(), node)).collect();
    let mut found = Vec::new();
    let mut seen = std::collections::HashSet::new();
    // In order: the last child pushed first, so the first is taken first.
    let mut waiting: Vec<&AxNode> = nodes
        .iter()
        .filter(|node| node.node == Some(dom))
        .take(1)
        .collect();
    // A tree read already ([`crate::chrome_tree`]) holds no loop; `seen`
    // keeps this from following one regardless.
    while let Some(node) = waiting.pop() {
        if !seen.insert(node.id.as_str()) {
            continue;
        }
        if node.role == "spinbutton"
            && let Some(segment) = node.node
        {
            found.push(segment);
        }
        waiting.extend(
            node.children
                .iter()
                .rev()
                .filter_map(|child| by_id.get(child.as_str()).copied()),
        );
    }
    found
}

/// The segments of a field, each with the part its element says it is, as
/// the order to type into them: exactly one day, one month and one year.
///
/// # Errors
///
/// [`Untypable::Segments`] for any other set, including a segment that is
/// no part: which digits go into it would be a guess.
pub fn laid_out(segments: &[(u64, Option<Part>)]) -> Result<Vec<(u64, Part)>, Untypable> {
    let refused = Untypable::Segments {
        found: segments.len(),
    };
    let order: Option<Vec<(u64, Part)>> = segments
        .iter()
        .map(|(node, part)| part.map(|part| (*node, part)))
        .collect();
    let order = order.ok_or_else(|| refused.clone())?;
    let one_each = order.len() == 3
        && [Part::Day, Part::Month, Part::Year]
            .iter()
            .all(|wanted| order.iter().filter(|(_, part)| part == wanted).count() == 1);
    if one_each { Ok(order) } else { Err(refused) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ax(id: &str, role: &str, dom: Option<u64>, children: &[&str]) -> AxNode {
        AxNode {
            id: id.to_owned(),
            role: role.to_owned(),
            node: dom,
            children: children.iter().map(|child| (*child).to_owned()).collect(),
            ..AxNode::default()
        }
    }

    #[test]
    fn a_segment_is_the_part_its_pseudo_element_names() {
        assert_eq!(
            Part::of_pseudo("-webkit-datetime-edit-day-field"),
            Some(Part::Day)
        );
        assert_eq!(
            Part::of_pseudo("-webkit-datetime-edit-month-field"),
            Some(Part::Month)
        );
        assert_eq!(
            Part::of_pseudo("-webkit-datetime-edit-year-field"),
            Some(Part::Year)
        );
        assert_eq!(Part::of_pseudo("-webkit-datetime-edit-hour-field"), None);
        assert_eq!(Part::of_pseudo("-WEBKIT-DATETIME-EDIT-DAY-FIELD"), None);
        assert_eq!(Part::of_pseudo(""), None);
    }

    #[test]
    fn each_part_is_typed_as_it_is_written() {
        let digits = Digits::of("2026-10-02").unwrap();
        assert_eq!(digits.of_part(Part::Year), "2026");
        assert_eq!(digits.of_part(Part::Month), "10");
        assert_eq!(digits.of_part(Part::Day), "02");
        assert_eq!(
            Digits::of("0005-01-02").unwrap().of_part(Part::Year),
            "0005"
        );
        assert_eq!(
            Digits::of("275760-09-13").unwrap().of_part(Part::Year),
            "275760"
        );
        assert_eq!(
            Digits::of("10000-01-01").unwrap().of_part(Part::Year),
            "10000"
        );
    }

    #[test]
    fn a_date_chromes_field_does_not_hold_as_written_is_not_typed() {
        let past = |text: &str| Untypable::PastTheLastDay {
            text: text.to_owned(),
        };
        assert_eq!(Digits::of("275760-09-14"), Err(past("275760-09-14")));
        assert_eq!(Digits::of("275760-10-01"), Err(past("275760-10-01")));
        assert_eq!(Digits::of("275761-01-01"), Err(past("275761-01-01")));
        assert_eq!(Digits::of("1000000-01-01"), Err(past("1000000-01-01")));
        // A hostile length, all digits, is past it too and does not overflow.
        let long = format!("{}-01-01", "9".repeat(4096));
        assert_eq!(Digits::of(&long), Err(past(&long)));
        assert_eq!(
            Digits::of("02026-01-02"),
            Err(Untypable::LeadingZeros {
                text: "02026-01-02".to_owned(),
            })
        );
        // Not a date at all, which an agent's decision never passes.
        for text in ["", "2026-02-30", "12/10/2026", "2026-1-2", "x"] {
            assert_eq!(Digits::of(text), Err(past(text)), "{text:?}");
        }
    }

    #[test]
    fn the_segments_are_found_in_the_order_they_are_laid_out() {
        // A field holding an ignored wrapper of the three segments, and the
        // picker's button; a spin button of another field after it.
        let nodes = [
            ax("1", "RootWebArea", Some(1), &["2", "9"]),
            ax("2", "Date", Some(7), &["3", "8"]),
            ax("3", "none", None, &["4", "5", "6"]),
            ax("4", "spinbutton", Some(13), &[]),
            ax("5", "spinbutton", Some(11), &[]),
            ax("6", "spinbutton", Some(15), &[]),
            ax("8", "button", Some(16), &[]),
            ax("9", "spinbutton", Some(30), &[]),
        ];
        assert_eq!(segments(&nodes, 7), [13, 11, 15]);
        assert!(segments(&nodes, 99).is_empty(), "a field not in the tree");
        let looped = [
            ax("1", "Date", Some(1), &["2"]),
            ax("2", "spinbutton", Some(2), &["1"]),
        ];
        assert_eq!(segments(&looped, 1), [2], "a loop ends regardless");
    }

    #[test]
    fn a_field_is_typed_into_only_with_one_of_each_part() {
        let (day, month, year) = (Some(Part::Day), Some(Part::Month), Some(Part::Year));
        assert_eq!(
            laid_out(&[(1, month), (2, day), (3, year)]),
            Ok(vec![(1, Part::Month), (2, Part::Day), (3, Part::Year)])
        );
        let refused = |found| Err(Untypable::Segments { found });
        assert_eq!(laid_out(&[]), refused(0));
        assert_eq!(laid_out(&[(1, day), (2, month)]), refused(2));
        assert_eq!(laid_out(&[(1, day), (2, day), (3, year)]), refused(3));
        assert_eq!(laid_out(&[(1, day), (2, None), (3, year)]), refused(3));
        assert_eq!(
            laid_out(&[(1, day), (2, month), (3, year), (4, year)]),
            refused(4)
        );
    }

    #[test]
    fn why_a_date_was_not_typed_reads_as_a_sentence() {
        assert_eq!(
            Untypable::PastTheLastDay {
                text: "275761-01-01".to_owned()
            }
            .to_string(),
            "\"275761-01-01\" is past 275760-09-13, the last day Chrome's date field holds"
        );
        assert_eq!(
            Untypable::Segments { found: 2 }.to_string(),
            "Chrome's date field shows 2 segments, not one each of a day, a month and a year"
        );
    }
}
