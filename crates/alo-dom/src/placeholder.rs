/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether a field shows a placeholder, and what it says (ADR 0043 § 2,
//! queue item 389).
//!
//! HTML's rule, held once for its three askers — the style tree, which
//! computes a `::placeholder` style only for an element that makes one; the
//! box tree, which makes the hint's box; and the agent tree, which reads the
//! hint as a property and never as text — as [`date`](crate::date) holds
//! what a date field holds.
//!
//! An element makes a `::placeholder` when it is a **`<textarea>`**, or an
//! **`<input>`** of a kind that takes one (`text`, `search`, `url`, `tel`,
//! `email`, `password`, `number`), with a **non-empty** `placeholder`, and
//! its value is **empty**. An `<input>`'s hint is shown with its line breaks
//! stripped, as HTML asks; a `<textarea>`'s keeps them. A date field takes
//! none: its format is its text (ADR 0042 § 3).
//!
//! # Every byte is a stranger's
//!
//! The attribute is the page's and has no length limit. Nothing here
//! indexes or counts: stripping is one pass over the characters, and a hint
//! that is nothing but line breaks is stripped to nothing and shows nothing.

use crate::document::Document;
use crate::node::{Element, NodeId};

/// The attribute a field's hint is written in.
pub const ATTRIBUTE: &str = "placeholder";

/// Whether `element` is a field HTML lets show a placeholder, whatever it
/// holds: a `<textarea>`, or an `<input>` of a kind that takes one.
pub fn takes_one(element: &Element) -> bool {
    if element.name.is_html("textarea") {
        return true;
    }
    if !element.name.is_html("input") {
        return false;
    }
    // A missing `type` is `text`, and so is one HTML does not know.
    let Some(kind) = element.attr("type") else {
        return true;
    };
    let known = [
        "hidden",
        "text",
        "search",
        "url",
        "tel",
        "email",
        "password",
        "date",
        "month",
        "week",
        "time",
        "datetime-local",
        "number",
        "range",
        "color",
        "checkbox",
        "radio",
        "file",
        "submit",
        "image",
        "reset",
        "button",
    ];
    if !known.iter().any(|known| kind.eq_ignore_ascii_case(known)) {
        return true;
    }
    [
        "text", "search", "url", "tel", "email", "password", "number",
    ]
    .iter()
    .any(|taking| kind.eq_ignore_ascii_case(taking))
}

/// The hint `element` would show, written as it is shown: its `placeholder`
/// with an `<input>`'s line breaks stripped, or [`None`] when it is not a
/// field that takes one or the hint is empty. Whether it holds a value is not
/// asked: the agent names a field by its hint whether or not it is shown.
pub fn hint(element: &Element) -> Option<String> {
    if !takes_one(element) {
        return None;
    }
    let written = element.attr(ATTRIBUTE)?;
    let shown = if element.name.is_html("input") {
        written
            .chars()
            .filter(|character| !matches!(character, '\n' | '\r'))
            .collect()
    } else {
        written.to_owned()
    };
    (!shown.is_empty()).then_some(shown)
}

/// The hint the element `id` shows now: its [`hint`] while its value is
/// empty, and [`None`] once it holds one — typed, put by an agent, or
/// written in the page — or when it makes no `::placeholder` at all.
///
/// An `<input>`'s value is its `value` attribute, as the box tree draws it
/// ([`field::TEXT`](crate::field::TEXT)). A `<textarea>`'s is what was put
/// into it, which is held in the same attribute, or else its text.
pub fn shown(document: &Document, id: NodeId) -> Option<String> {
    let element = document.element(id)?;
    let hint = hint(element)?;
    let value = element.attr(crate::field::TEXT);
    let empty = match value {
        Some(value) => value.is_empty(),
        None if element.name.is_html("textarea") => document.text_content(id).is_empty(),
        None => true,
    };
    empty.then_some(hint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_document;

    /// What the first element called `name` shows, in `html`.
    fn shown_by(html: &str, name: &str) -> Option<String> {
        let document = parse_document(html);
        let id = document
            .descendants(document.root())
            .find(|id| document.get(*id).is_some_and(|n| n.is_html_element(name)))?;
        shown(&document, id)
    }

    #[test]
    fn every_kind_that_takes_one_shows_it_and_no_other_does() {
        for kind in [
            "", "text", "search", "url", "tel", "email", "password", "number", "TEXT", "nonsense",
        ] {
            let typed = if kind.is_empty() {
                String::new()
            } else {
                format!(" type={kind}")
            };
            let html = format!("<input{typed} placeholder=hint>");
            assert_eq!(shown_by(&html, "input").as_deref(), Some("hint"), "{kind}");
        }
        for kind in [
            "date", "time", "checkbox", "radio", "button", "submit", "hidden", "range", "color",
            "file",
        ] {
            let html = format!("<input type={kind} placeholder=hint>");
            assert_eq!(shown_by(&html, "input"), None, "{kind}");
        }
        assert_eq!(shown_by("<p placeholder=hint>x</p>", "p"), None);
        assert_eq!(
            shown_by("<select placeholder=hint></select>", "select"),
            None
        );
    }

    #[test]
    fn an_input_loses_its_line_breaks_and_a_textarea_keeps_them() {
        assert_eq!(
            shown_by("<input placeholder='one\ntwo\r\nthree'>", "input").as_deref(),
            Some("onetwothree")
        );
        assert_eq!(
            shown_by("<textarea placeholder='one\ntwo'></textarea>", "textarea").as_deref(),
            Some("one\ntwo")
        );
    }

    #[test]
    fn a_value_or_an_empty_hint_shows_nothing() {
        assert_eq!(shown_by("<input placeholder=''>", "input"), None);
        assert_eq!(shown_by("<input placeholder>", "input"), None);
        assert_eq!(
            shown_by("<input placeholder=hint value=held>", "input"),
            None
        );
        assert_eq!(
            shown_by("<input placeholder=hint value=''>", "input").as_deref(),
            Some("hint")
        );
        assert_eq!(
            shown_by("<textarea placeholder=hint>held</textarea>", "textarea"),
            None
        );
        // A value put into a textarea is held in the attribute, and hides it.
        let mut document = parse_document("<textarea placeholder=hint></textarea>");
        let Some(id) = document.descendants(document.root()).find(|id| {
            document
                .get(*id)
                .is_some_and(|n| n.is_html_element("textarea"))
        }) else {
            panic!("the textarea");
        };
        assert_eq!(shown(&document, id).as_deref(), Some("hint"));
        assert!(crate::field::put_text(&mut document, id, "typed"));
        assert_eq!(shown(&document, id), None);
        assert!(crate::field::put_text(&mut document, id, ""));
        assert_eq!(shown(&document, id).as_deref(), Some("hint"));
    }

    #[test]
    fn a_hostile_hint_is_shown_stripped_or_not_at_all_without_a_panic() {
        let long = "x".repeat(100_000);
        let html = format!("<input placeholder='{long}'>");
        assert_eq!(
            shown_by(&html, "input").map(|hint| hint.len()),
            Some(100_000)
        );

        let breaks = "\n\r".repeat(50_000);
        let html = format!("<input placeholder='{breaks}'>");
        assert_eq!(shown_by(&html, "input"), None, "nothing but line breaks");
        let html = format!("<textarea placeholder='{breaks}'></textarea>");
        assert!(
            shown_by(&html, "textarea").is_some(),
            "a textarea keeps them"
        );

        let controls: String = (1u8..32).map(char::from).collect::<String>().repeat(1_000);
        let html = format!("<input placeholder='{controls}'>");
        let Some(hint) = shown_by(&html, "input") else {
            panic!("control characters other than line breaks are kept");
        };
        assert!(!hint.contains('\n') && !hint.contains('\r'));
        assert!(hint.contains('\t'));
    }
}
