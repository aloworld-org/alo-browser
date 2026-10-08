/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The `style` an element's own declaration wrote (ADR 0034 § 1).
//!
//! CSSOM lets a page's script write an element's style through
//! `element.style` under a policy that refuses every other inline style: a
//! value the declaration wrote is the page's own. ADR 0033 made the `style`
//! attribute the only copy of that block, so whether the attribute's value
//! *came from* the declaration has to be answered from the element, at any
//! later draw.
//!
//! # The text, not a flag
//!
//! An element remembers **the text the declaration last wrote**, set only by
//! [`Document::set_declared_style`], which sets the attribute and the record
//! together as one counted change. The attribute is the declaration's own
//! exactly when its value **equals** that text ([`Element::style_is_declared`])
//! — nothing else is ever read from the record.
//!
//! A flag would have to be cleared by every other writer of the attribute,
//! and [`Element::attrs`] is public: one writer that forgot would be a way
//! round the page's policy. A record compared by value cannot go stale that
//! way. Whatever else changes the attribute — the parser, `setAttribute`, an
//! agent, a writer added next year — makes the two unequal, and the attribute
//! is then judged by the policy. What a stale record can still admit is only
//! the exact text the page's own script wrote there, which the script could
//! have written through `element.style` at any moment.
//!
//! **No other operation reads or clears it.** A clone, when cloning is built,
//! must not carry it: HTML judges a clone's `style` by the policy.

use crate::document::Document;
use crate::node::{Element, NodeId};

impl Element {
    /// Whether this element's `style` attribute is the text its declaration
    /// last wrote — and so the page's own script's, which a policy refusing
    /// inline style does not refuse.
    ///
    /// False for an element with no `style` attribute, and for one whose
    /// declaration never wrote.
    pub fn style_is_declared(&self) -> bool {
        match (self.attr("style"), &self.declared) {
            (Some(attribute), Some(declared)) => attribute == declared,
            _ => false,
        }
    }
}

impl Document {
    /// CSSOM's *update style attribute*: set an element's `style` attribute
    /// to `text`, as its declaration writes it, and remember that it did.
    ///
    /// One counted change, as [`Document::set_attribute`] is. The one caller
    /// is the element's `CSSStyleDeclaration`; anything else that sets the
    /// attribute uses [`Document::set_attribute`], and its value is then
    /// judged by the page's policy.
    ///
    /// [`None`] for an id that is not an element, which changes nothing.
    pub fn set_declared_style(&mut self, id: NodeId, text: &str) -> Option<()> {
        self.edit_element(id, |element| {
            element.set_attr("style", text);
            match &mut element.declared {
                Some(held) => text.clone_into(held),
                None => element.declared = Some(text.to_owned()),
            }
        })?;
        self.note_change();
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{Document, NodeId, parse_document};

    fn paragraph(document: &Document) -> NodeId {
        document
            .descendants(document.root())
            .find(|id| document.get(*id).is_some_and(|n| n.is_html_element("p")))
            .unwrap_or_else(|| panic!("the markup has a <p>"))
    }

    fn declared(document: &Document, id: NodeId) -> bool {
        document
            .element(id)
            .is_some_and(crate::Element::style_is_declared)
    }

    #[test]
    fn markup_is_never_the_declarations() {
        let document = parse_document(r#"<p style="color: red">x</p>"#);
        assert!(!declared(&document, paragraph(&document)));
    }

    #[test]
    fn what_the_declaration_wrote_is_its_own_and_is_one_counted_change() {
        let mut document = parse_document(r#"<p style="color: red">x</p>"#);
        let p = paragraph(&document);
        let before = document.change_count();
        assert_eq!(document.set_declared_style(p, "color: blue;"), Some(()));
        assert_eq!(document.change_count(), before.wrapping_add(1));
        assert_eq!(
            document.element(p).and_then(|e| e.attr("style")),
            Some("color: blue;")
        );
        assert!(declared(&document, p));
    }

    #[test]
    fn any_other_writer_makes_the_attribute_unequal_and_so_not_its_own() {
        let mut document = parse_document("<p>x</p>");
        let p = paragraph(&document);
        document.set_declared_style(p, "color: blue;");
        document.set_attribute(p, "style", "color: red;");
        assert!(!declared(&document, p), "setAttribute replaced it");
        document.set_declared_style(p, "color: blue;");
        document.remove_attribute(p, "style");
        assert!(
            !declared(&document, p),
            "removed: there is nothing to admit"
        );
        if let Some(element) = document.element(p) {
            // A writer that goes round every operation, through the public
            // list, is covered without being told.
            let mut copy = element.clone();
            copy.attrs.push(crate::Attribute::plain("style", "x: y"));
            assert!(!copy.style_is_declared());
        }
    }

    #[test]
    fn a_stale_record_admits_only_the_text_the_declaration_wrote() {
        let mut document = parse_document("<p>x</p>");
        let p = paragraph(&document);
        document.set_declared_style(p, "color: blue;");
        document.set_attribute(p, "style", "color: red;");
        document.set_attribute(p, "style", "color: blue;");
        assert!(
            declared(&document, p),
            "the same text the script wrote, written back"
        );
        document.set_attribute(p, "style", "color: blue; ");
        assert!(!declared(&document, p), "one character more is not it");
    }

    #[test]
    fn the_record_is_counted_in_the_documents_footprint() {
        let mut document = parse_document("<p>x</p>");
        let p = paragraph(&document);
        let before = document.footprint();
        document.set_declared_style(p, "color: blue;");
        let text = "color: blue;".len();
        // The attribute and the record: two copies of the text, and one
        // attribute's own size and name.
        assert!(document.footprint() >= before.saturating_add(2 * text));
    }

    #[test]
    fn something_that_is_not_an_element_has_no_declaration() {
        let mut document = Document::new();
        let root = document.root();
        let before = document.change_count();
        assert_eq!(document.set_declared_style(root, "color: red"), None);
        assert_eq!(document.change_count(), before);
    }
}
