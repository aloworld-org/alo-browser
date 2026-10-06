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
//! # A field's text is its `value` attribute, for now
//!
//! The standard keeps an `<input>`'s *value* apart from its `value`
//! attribute, which only sets its default until the value is dirty. That
//! split is item 82's (forms), with checkedness; until it lands the
//! attribute is the text, as it has been since stage 1 — it is what the box
//! tree draws and the agent reads back — and only where it is written
//! changes when it does, not who calls this or when.

use crate::document::Document;
use crate::node::NodeId;

/// The attribute a field's text is held in, until item 82 holds the value
/// apart from it.
pub const TEXT: &str = "value";

/// Replace the text of the field `node` with `text`, answering whether it
/// was — not for a node that is not an element, which an agent's decision
/// never names.
pub fn put_text(document: &mut Document, node: NodeId, text: &str) -> bool {
    document.set_attribute(node, TEXT, text).is_some()
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
