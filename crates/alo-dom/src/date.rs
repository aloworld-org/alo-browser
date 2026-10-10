/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a date field holds: HTML's *valid date string* (ADR 0042, queue item
//! 385).
//!
//! An `<input type=date>`'s value is `yyyy-mm-dd` — four or more digits of a
//! year after zero, two of a month from 1 to 12, two of a day that month has —
//! and a `value` that is not one is **sanitised to the empty string**: the
//! field holds nothing, draws its format, and reads as empty. That is a rule
//! about the document with three askers — the box tree, which draws the
//! value or the format; the agent tree, which reads one and never the other;
//! and the agent's `PutText`, which refuses a text that is not a date rather
//! than let the field silently empty it — so it lives here once.
//!
//! # Every byte is a stranger's
//!
//! A page writes the attribute and an agent writes the text, and neither is
//! trusted. Nothing here indexes, and the year is never turned into a number:
//! HTML puts no upper bound on its digits, so a year a thousand digits long is
//! a valid year, and the one fact this needs of it — whether it is a leap year
//! — is decided by its last four digits, because 400 divides 10 000.

use crate::node::Element;

/// Whether `element` is a date field: an `<input>` whose `type` is `date`,
/// in any case.
pub fn is_date_field(element: &Element) -> bool {
    element.name.is_html("input")
        && element
            .attr("type")
            .is_some_and(|kind| kind.eq_ignore_ascii_case("date"))
}

/// The date a date field holds: its value when that is a valid date string,
/// and nothing — the sanitised empty string — when it is anything else or
/// when `element` is not a date field.
pub fn held_date(element: &Element) -> Option<&str> {
    if !is_date_field(element) {
        return None;
    }
    element
        .attr(crate::field::TEXT)
        .filter(|value| is_valid_date_string(value))
}

/// Whether `text` is a valid date string: `yyyy-mm-dd`, with a year of four
/// or more digits that is not zero, a month from `01` to `12` and a day the
/// month has, the 29th of February only in a leap year.
pub fn is_valid_date_string(text: &str) -> bool {
    let mut parts = text.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    if year.len() < 4 || !is_digits(year) || year.bytes().all(|byte| byte == b'0') {
        return false;
    }
    let (Some(month), Some(day)) = (two_digits(month), two_digits(day)) else {
        return false;
    };
    (1..=12).contains(&month) && day >= 1 && day <= days_in(year, month)
}

/// Whether every byte of `text` is an ASCII digit, and there is at least one.
fn is_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// Exactly two ASCII digits, as a number.
fn two_digits(text: &str) -> Option<u8> {
    if text.len() != 2 || !is_digits(text) {
        return None;
    }
    text.parse().ok()
}

/// How many days `month` of `year` has. `year` is all digits and at least
/// four of them; `month` is 1 to 12.
fn days_in(year: &str, month: u8) -> u8 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Whether `year` is a leap year, from its last four digits alone: 4, 100
/// and 400 all divide 10 000, so they decide it whatever comes before.
fn is_leap(year: &str) -> bool {
    let last = year
        .get(year.len().saturating_sub(4)..)
        .and_then(|digits| digits.parse::<u16>().ok())
        .unwrap_or(0);
    last % 4 == 0 && (last % 100 != 0 || last % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_document;

    /// The first `<input>` of `html`.
    fn input(html: &str) -> Element {
        let document = parse_document(html);
        let Some(found) = document
            .descendants(document.root())
            .find_map(|id| document.element(id).filter(|e| e.name.is_html("input")))
        else {
            panic!("no <input> in {html}");
        };
        found.clone()
    }

    #[test]
    fn a_date_is_a_year_a_month_and_a_day_the_month_has() {
        for valid in [
            "2026-10-12",
            "0001-01-01",
            "2026-12-31",
            "2024-02-29",
            "2000-02-29",
            "2026-04-30",
            // HTML bounds the year's digits from below only.
            "275760-09-13",
            "123456789012345678901234567890-01-31",
        ] {
            assert!(is_valid_date_string(valid), "{valid}");
        }
    }

    #[test]
    fn anything_else_is_not_a_date() {
        for invalid in [
            "",
            "-",
            "--",
            "12/10/2026",
            "10-12-2026",
            "2026-1-12",
            "2026-01-1",
            "26-10-12",
            "0000-01-01",
            "2026-00-12",
            "2026-13-12",
            "2026-10-00",
            "2026-10-32",
            "2026-04-31",
            "2026-02-29",
            "1900-02-29",
            "2026-10-12-",
            "2026-10-12T00:00",
            " 2026-10-12",
            "2026-10-12 ",
            "+2026-10-12",
            "2026-+1-12",
            "２０２６-10-12",
            "2026-١٠-12",
            "-2026-10-12",
        ] {
            assert!(!is_valid_date_string(invalid), "{invalid:?}");
        }
    }

    #[test]
    fn hostile_lengths_are_answered_without_a_number_that_overflows() {
        let long_year = "9".repeat(100_000);
        assert!(is_valid_date_string(&format!("{long_year}-01-31")));
        // …9996 is a leap year and …9999 is not, however long.
        let leap = format!("{}6-02-29", "9".repeat(10_000));
        assert!(is_valid_date_string(&leap));
        assert!(!is_valid_date_string(&format!("{long_year}-02-29")));
        assert!(!is_valid_date_string(&"-".repeat(100_000)));
        assert!(!is_valid_date_string(&"0".repeat(100_000)));
        assert!(!is_valid_date_string(&format!(
            "2026-10-{}",
            "1".repeat(100_000)
        )));
    }

    #[test]
    fn a_date_field_holds_its_value_only_when_that_is_a_date() {
        assert_eq!(
            held_date(&input("<input type=date value=2026-10-12>")),
            Some("2026-10-12")
        );
        assert_eq!(
            held_date(&input("<input type=DATE value=2026-10-12>")),
            Some("2026-10-12")
        );
        // Sanitised to the empty string: the field holds nothing.
        assert_eq!(
            held_date(&input("<input type=date value=12/10/2026>")),
            None
        );
        assert_eq!(held_date(&input("<input type=date value=''>")), None);
        assert_eq!(held_date(&input("<input type=date>")), None);
        // A text field holding a date string is a text field, not a date one.
        assert_eq!(held_date(&input("<input value=2026-10-12>")), None);
        assert!(!is_date_field(&input("<input type=datetime-local>")));
        assert!(is_date_field(&input("<input type=date>")));
    }
}
