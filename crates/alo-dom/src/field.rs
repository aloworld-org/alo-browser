/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What text put into a field does to it (ADR 0018 § 5, queue item 257).
//!
//! An agent's `PutText` replaces a field's text whole. That is a rule about
//! the document with two callers — `alo-agent`'s `apply`, on a page that has
//! never run script, and the renderer's `PutText` task, which runs it
//! between the `beforeinput` a page may cancel and the `input` that says it
//! happened — so it lives here once, for ADR 0017 § 5's reason, as
//! [`activation`](crate::activation) does for a click.
//!
//! # A field's text is its `value` attribute, or its content, for now
//!
//! The standard keeps a field's *value* apart from what the markup wrote,
//! which only sets its default until the value is dirty. That split is item
//! 469's (ADR 0051 § 1), with checkedness; until it lands the markup is the
//! text, as it has been since stage 1 — it is what the box tree draws and
//! the agent reads back — and only where it is written changes when it does,
//! not who calls this or when.
//!
//! An `<input>`'s default is its `value` attribute, so that is where its
//! text is held. A `<textarea>`'s default is **its content** — it has no
//! `value` attribute, and Chrome draws none that a page writes on one — so
//! text put into it replaces its content (queue item 131). Writing it into
//! a `value` attribute instead, as this once did for both, put it where
//! nothing draws or reads it: the agent was told the text was put and the
//! field still showed what it held before.
//!
//! # Fields whose text is not held yet
//!
//! A `time`, `month`, `week` or `datetime-local` field is typed into, as a
//! date field is, and nothing here draws what one holds: each draws in
//! segments whose order is the person's (ADR 0042 § 3 decided this for a
//! date alone). So text is **not** put into one. An agent is refused by
//! name ([`unbuilt`]) rather than told it put a time that nobody can see.

use crate::document::Document;
use crate::node::{Element, NodeId};

/// The attribute an `<input>`'s text is held in, until item 469 holds the
/// value apart from it.
pub const TEXT: &str = "value";

/// The `<input>` types a person types into whose text this engine does not
/// hold yet, because nothing draws it.
const UNBUILT: &[&str] = &["time", "month", "week", "datetime-local"];

/// Whether a field's text is its content rather than its [`TEXT`]
/// attribute: whether it is a `<textarea>`.
pub fn is_held_as_content(element: &Element) -> bool {
    element.name.is_html("textarea")
}

/// The type of an `<input>` that is typed into and whose text this engine
/// does not hold yet, as HTML names it — `time` — or [`None`] for any other
/// element.
pub fn unbuilt(element: &Element) -> Option<&'static str> {
    if !element.name.is_html("input") {
        return None;
    }
    let kind = element.attr("type")?;
    UNBUILT
        .iter()
        .find(|unbuilt| kind.eq_ignore_ascii_case(unbuilt))
        .copied()
}

/// Replace the text of the field `node` with `text`, answering whether it
/// was — not for a node that is not an element, which an agent's decision
/// never names.
///
/// A `<textarea>`'s content is replaced whole, by one text node or by
/// nothing when `text` is empty; any other element's [`TEXT`] attribute is
/// set.
pub fn put_text(document: &mut Document, node: NodeId, text: &str) -> bool {
    match document.element(node) {
        Some(element) if is_held_as_content(element) => {
            document.replace_all_with_text(node, text).is_some()
        }
        Some(_) => document.set_attribute(node, TEXT, text).is_some(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_document;

    /// The first element called `name`.
    fn first(document: &Document, name: &str) -> NodeId {
        let Some(found) = document
            .descendants(document.root())
            .find(|id| document.get(*id).is_some_and(|n| n.is_html_element(name)))
        else {
            panic!("no <{name}>");
        };
        found
    }

    #[test]
    fn text_put_into_a_field_replaces_what_it_held_and_counts_as_a_change() {
        let mut document = parse_document("<input value=before><p>words</p>");
        let field = first(&document, "input");
        let changes = document.change_count();
        assert!(put_text(&mut document, field, "after"));
        assert_eq!(
            document.element(field).and_then(|e| e.attr("value")),
            Some("after"),
        );
        assert!(document.change_count() > changes);

        // The empty text is text too: the field is emptied, not left alone.
        assert!(put_text(&mut document, field, ""));
        assert_eq!(
            document.element(field).and_then(|e| e.attr("value")),
            Some(""),
        );
    }

    #[test]
    fn text_put_into_a_textarea_replaces_its_content_and_writes_no_attribute() {
        let mut document = parse_document("<textarea>one<b>x</b>two</textarea>");
        let field = first(&document, "textarea");
        let changes = document.change_count();
        assert!(put_text(&mut document, field, "Ring twice"));
        assert_eq!(document.text_content(field), "Ring twice");
        assert_eq!(document.children(field).count(), 1, "one text node");
        assert_eq!(
            document.element(field).and_then(|e| e.attr(TEXT)),
            None,
            "a textarea has no value attribute, and nothing reads one"
        );
        assert!(document.change_count() > changes);

        // Emptied, with nothing left in it.
        assert!(put_text(&mut document, field, ""));
        assert_eq!(document.children(field).count(), 0);
    }

    #[test]
    fn the_fields_whose_text_is_not_held_are_named_by_their_type() {
        let document = parse_document(
            "<input type=TIME><input type=month><input type=week>\
             <input type=datetime-local><input type=date><input><textarea></textarea>",
        );
        let kinds: Vec<Option<&str>> = document
            .descendants(document.root())
            .filter_map(|id| document.element(id))
            .filter(|element| element.name.is_html("input") || element.name.is_html("textarea"))
            .map(unbuilt)
            .collect();
        assert_eq!(
            kinds,
            [
                Some("time"),
                Some("month"),
                Some("week"),
                Some("datetime-local"),
                None,
                None,
                None,
            ]
        );
    }

    #[test]
    fn a_node_that_is_not_an_element_takes_no_text() {
        let mut document = parse_document("<p>words</p>");
        let paragraph = first(&document, "p");
        let Some(words) = document.first_child(paragraph) else {
            panic!("the paragraph has its text");
        };
        let changes = document.change_count();
        assert!(!put_text(&mut document, words, "replaced"));
        assert_eq!(document.text_content(paragraph), "words");
        assert_eq!(document.change_count(), changes);
    }
}
