/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What kind of control an `<input>` is: HTML's `type` attribute, as a
//! state rather than as text (ADR 0051 §§ 3 and 4).
//!
//! A missing `type`, and one HTML does not know, is the text state. Which
//! attributes mean anything on an input — `required`, `readonly`,
//! `maxlength`, `pattern`, `min` — is a fact about its state, and the
//! constraints and the entry list ask it here rather than each keeping a
//! list of their own that would one day disagree.

use crate::node::Element;

/// An `<input>`'s state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `hidden`.
    Hidden,
    /// `text`, a missing `type` and every `type` HTML does not know.
    Text,
    /// `search`.
    Search,
    /// `tel`.
    Tel,
    /// `url`.
    Url,
    /// `email`.
    Email,
    /// `password`.
    Password,
    /// `date`.
    Date,
    /// `month`.
    Month,
    /// `week`.
    Week,
    /// `time`.
    Time,
    /// `datetime-local`.
    DateTimeLocal,
    /// `number`.
    Number,
    /// `range`.
    Range,
    /// `color`.
    Color,
    /// `checkbox`.
    Checkbox,
    /// `radio`.
    Radio,
    /// `file`.
    File,
    /// `submit`.
    Submit,
    /// `image`.
    Image,
    /// `reset`.
    Reset,
    /// `button`.
    Button,
}

/// Each state and the keyword that names it.
const KEYWORDS: &[(&str, Kind)] = &[
    ("hidden", Kind::Hidden),
    ("text", Kind::Text),
    ("search", Kind::Search),
    ("tel", Kind::Tel),
    ("url", Kind::Url),
    ("email", Kind::Email),
    ("password", Kind::Password),
    ("date", Kind::Date),
    ("month", Kind::Month),
    ("week", Kind::Week),
    ("time", Kind::Time),
    ("datetime-local", Kind::DateTimeLocal),
    ("number", Kind::Number),
    ("range", Kind::Range),
    ("color", Kind::Color),
    ("checkbox", Kind::Checkbox),
    ("radio", Kind::Radio),
    ("file", Kind::File),
    ("submit", Kind::Submit),
    ("image", Kind::Image),
    ("reset", Kind::Reset),
    ("button", Kind::Button),
];

/// The state of `element` if it is an `<input>`, and [`None`] for any
/// other element.
pub fn of(element: &Element) -> Option<Kind> {
    if !element.name.is_html("input") {
        return None;
    }
    let Some(written) = element.attr("type") else {
        return Some(Kind::Text);
    };
    Some(
        KEYWORDS
            .iter()
            .find(|(keyword, _)| written.eq_ignore_ascii_case(keyword))
            .map_or(Kind::Text, |(_, kind)| *kind),
    )
}

/// The state of a `<button>` — [`Kind::Submit`], [`Kind::Reset`] or
/// [`Kind::Button`] by its `type`, a missing or unknown one being a submit
/// button — and [`None`] for any other element.
pub fn of_button(element: &Element) -> Option<Kind> {
    if !element.name.is_html("button") {
        return None;
    }
    let written = element.attr("type").unwrap_or_default();
    Some(if written.eq_ignore_ascii_case("reset") {
        Kind::Reset
    } else if written.eq_ignore_ascii_case("button") {
        Kind::Button
    } else {
        Kind::Submit
    })
}

/// Whether `element` is a submit button: a `<button>` whose type is
/// `submit`, missing or unknown, or an `<input>` of type `submit` or
/// `image`.
pub fn is_submit_button(element: &Element) -> bool {
    matches!(
        of_button(element).or_else(|| of(element)),
        Some(Kind::Submit | Kind::Image)
    )
}

impl Kind {
    /// The keyword that names it, as HTML writes it.
    pub fn keyword(self) -> &'static str {
        KEYWORDS
            .iter()
            .find(|(_, kind)| *kind == self)
            .map_or("text", |(keyword, _)| keyword)
    }

    /// Whether a person types its value as text: the states `maxlength`,
    /// `minlength` and `pattern` apply to.
    pub const fn is_typed_text(self) -> bool {
        matches!(
            self,
            Kind::Text | Kind::Search | Kind::Tel | Kind::Url | Kind::Email | Kind::Password
        )
    }

    /// Whether it holds a date, a time or a number: the states `min`, `max`
    /// and `step` apply to.
    pub const fn is_ranged(self) -> bool {
        matches!(
            self,
            Kind::Date
                | Kind::Month
                | Kind::Week
                | Kind::Time
                | Kind::DateTimeLocal
                | Kind::Number
                | Kind::Range
        )
    }

    /// Whether `required` means anything on it.
    pub const fn can_be_required(self) -> bool {
        self.is_typed_text()
            || matches!(
                self,
                Kind::Date
                    | Kind::Month
                    | Kind::Week
                    | Kind::Time
                    | Kind::DateTimeLocal
                    | Kind::Number
                    | Kind::Checkbox
                    | Kind::Radio
                    | Kind::File
            )
    }

    /// Whether it is a button: it has no value a person gives it, and
    /// sends one only when it is the button that submitted its form.
    pub const fn is_button(self) -> bool {
        matches!(
            self,
            Kind::Submit | Kind::Image | Kind::Reset | Kind::Button
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_document;

    fn kinds(markup: &str) -> Vec<Option<Kind>> {
        let document = parse_document(markup);
        document
            .descendants(document.root())
            .filter_map(|id| document.element(id))
            .filter(|element| element.name.is_html("input") || element.name.is_html("p"))
            .map(of)
            .collect()
    }

    #[test]
    fn a_type_is_read_in_any_case_and_an_unknown_one_is_text() {
        assert_eq!(
            kinds("<input><input type=EMAIL><input type=nonsense><input type=''><p>"),
            [
                Some(Kind::Text),
                Some(Kind::Email),
                Some(Kind::Text),
                Some(Kind::Text),
                None,
            ]
        );
    }

    #[test]
    fn every_keyword_names_its_own_state() {
        for (keyword, kind) in KEYWORDS {
            assert_eq!(kind.keyword(), *keyword);
            let markup = format!("<input type={keyword}>");
            assert_eq!(kinds(&markup), [Some(*kind)]);
        }
    }

    #[test]
    fn a_button_submits_unless_it_says_otherwise() {
        let document = parse_document(
            "<button>a</button><button type=RESET>b</button><button type=button>c</button>\
             <button type=nonsense>d</button><input type=submit><input type=image>\
             <input type=reset><input>",
        );
        let submits: Vec<bool> = document
            .descendants(document.root())
            .filter_map(|id| document.element(id))
            .filter(|element| element.name.is_html("input") || element.name.is_html("button"))
            .map(is_submit_button)
            .collect();
        assert_eq!(
            submits,
            [true, false, false, true, true, true, false, false]
        );
    }

    #[test]
    fn what_applies_to_each_state_is_a_fact_about_it() {
        assert!(Kind::Email.is_typed_text());
        assert!(!Kind::Date.is_typed_text());
        assert!(Kind::Date.is_ranged());
        assert!(Kind::Range.is_ranged());
        assert!(!Kind::Text.is_ranged());
        assert!(Kind::File.can_be_required());
        assert!(!Kind::Range.can_be_required());
        assert!(!Kind::Hidden.can_be_required());
        assert!(Kind::Image.is_button());
        assert!(!Kind::Checkbox.is_button());
    }
}
