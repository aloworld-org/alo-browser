/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How many characters wide a text field is.
//!
//! A field a person types text into is as wide as its `size` attribute says,
//! in characters, and twenty when it says nothing usable. That is the
//! **element's** to say, as which face a control draws is ([`crate::face`]);
//! how wide a character is needs a font, and is layout's
//! (`alo_layout::field_width`).
//!
//! Measured in Chrome 154 (queue item 511): a `text`, `search`, `email`,
//! `url`, `tel` or `password` field, or one whose `type` is missing or
//! unknown, honours `size`. A `number` field does not, and is twenty
//! whatever it says. `size` is read as HTML reads a non-negative integer —
//! `" 7"`, `"7px"` and `"+7"` are 7 and `"5.7"` is 5 — and `0`, a negative
//! number, no digits, or more than 2,147,483,647 are each twenty.
//!
//! A date or a time field is not one: Chrome sizes it to its segments and
//! its calendar's indicator (queue item 517), so it keeps the user-agent
//! sheet's `20ch` until that is measured.

use alo_dom::Element;

/// What a field is when its `size` says nothing usable.
const DEFAULT: u32 = 20;

/// The largest `size` HTML reflects; anything larger is no size at all.
const LARGEST: usize = 2_147_483_647;

/// The types that are not text fields at all, or not ones this sizes.
const NOT_SIZED_HERE: [&str; 15] = [
    "checkbox",
    "radio",
    "hidden",
    "color",
    "file",
    "range",
    "image",
    "button",
    "submit",
    "reset",
    "date",
    "month",
    "week",
    "time",
    "datetime-local",
];

/// How many characters wide an element is as a text field, or [`None`] when
/// it is not one.
pub(crate) fn of(element: &Element) -> Option<u32> {
    if !element.name.is_html("input") {
        return None;
    }
    let kind = element.attr("type").unwrap_or("text").trim();
    if kind.eq_ignore_ascii_case("number") {
        return Some(DEFAULT);
    }
    if NOT_SIZED_HERE
        .iter()
        .any(|other| kind.eq_ignore_ascii_case(other))
    {
        return None;
    }
    // `search`, `email`, `url`, `tel` and `password` read `size` as `text`
    // does, and every type HTML does not know is `text`.
    Some(
        element
            .attr("size")
            .and_then(alo_dom::constraint::non_negative)
            .filter(|size| (1..=LARGEST).contains(size))
            .and_then(|size| u32::try_from(size).ok())
            .unwrap_or(DEFAULT),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::parse_document;

    /// The `<input>` types that honour `size`.
    const SIZED: [&str; 6] = ["text", "search", "email", "url", "tel", "password"];

    /// How many characters wide the first `<input>` in some markup is.
    fn first(html: &str) -> Option<u32> {
        let document = parse_document(html);
        document
            .descendants(document.root())
            .filter_map(|id| document.element(id))
            .find(|element| {
                !["html", "head", "body"]
                    .iter()
                    .any(|n| element.name.is_html(n))
            })
            .and_then(of)
    }

    #[test]
    fn a_field_is_twenty_characters_unless_its_size_says_otherwise() {
        assert_eq!(first("<input>"), Some(20));
        assert_eq!(first("<input size=5>"), Some(5));
        assert_eq!(first("<input size=1>"), Some(1));
    }

    #[test]
    fn every_type_that_takes_text_honours_its_size() {
        for kind in SIZED.iter().chain(&["TEXT", "Password", "nonsense", ""]) {
            assert_eq!(
                first(&format!("<input type='{kind}' size=3>")),
                Some(3),
                "{kind}"
            );
        }
    }

    #[test]
    fn a_number_field_is_twenty_whatever_its_size_says() {
        assert_eq!(first("<input type=number size=5>"), Some(20));
        assert_eq!(first("<input type=Number>"), Some(20));
    }

    #[test]
    fn size_is_read_as_chrome_reads_it() {
        // Chrome 154, measured (queue item 511).
        for (size, characters) in [
            (" 7", 7),
            ("7px", 7),
            ("+5", 5),
            ("5.7", 5),
            ("0", 20),
            ("-0", 20),
            ("-3", 20),
            ("abc", 20),
            ("", 20),
            ("2147483647", 2_147_483_647),
            ("2147483648", 20),
            ("99999999999", 20),
        ] {
            assert_eq!(
                first(&format!("<input size='{size}'>")),
                Some(characters),
                "{size:?}"
            );
        }
    }

    #[test]
    fn nothing_else_is_sized_here() {
        for kind in NOT_SIZED_HERE {
            assert_eq!(
                first(&format!("<input type={kind} size=3>")),
                None,
                "{kind}"
            );
        }
        assert_eq!(first("<textarea cols=3></textarea>"), None);
        assert_eq!(first("<select size=3></select>"), None);
        assert_eq!(first("<button>b</button>"), None);
    }
}
