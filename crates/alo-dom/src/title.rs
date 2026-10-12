/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! HTML's document title: what `document.title` reads, and what a tab is
//! called (queue item 528, ADR 0024 § 4).
//!
//! HTML's getter, step for step:
//!
//! 1. When the document element is an SVG `svg`, the value is the child text
//!    content of its first child that is an SVG `title`.
//! 2. Otherwise it is the child text content of **the title element** — the
//!    first HTML `title` in the document, in tree order, wherever it is — or
//!    nothing when there is none.
//! 3. ASCII white space is stripped from both ends and every run of it inside
//!    is collapsed to one space.
//!
//! *Child* text content is the text nodes that are children, joined — not
//! descendants. A `<title>` is parsed as text, so a page's `<b>x</b>` inside
//! one is that text, angle brackets and all; one built by script can hold an
//! element, whose text is not the title's.
//!
//! The title is a stranger's string. It is read here and carried as text; the
//! tab strip puts it into its document as a text node, never as markup.

use crate::document::Document;
use crate::name::Namespace;
use crate::node::{Node, NodeId};

impl Document {
    /// The document's title, as `document.title` reads it.
    pub fn title(&self) -> String {
        let element = match self.svg_root() {
            Some(svg) => self
                .children(svg)
                .find(|child| self.is_named(*child, &Namespace::Svg, "title")),
            None => self
                .descendants(self.root())
                .find(|node| self.is_named(*node, &Namespace::Html, "title")),
        };
        element.map_or_else(String::new, |title| {
            strip_and_collapse(&self.child_text_content(title))
        })
    }

    /// The document element, when it is an SVG `svg`.
    fn svg_root(&self) -> Option<NodeId> {
        self.document_element()
            .filter(|root| self.is_named(*root, &Namespace::Svg, "svg"))
    }

    /// Whether `node` is an element named `local` in `ns`.
    fn is_named(&self, node: NodeId, ns: &Namespace, local: &str) -> bool {
        self.element(node)
            .is_some_and(|element| element.name.ns == *ns && &*element.name.local == local)
    }

    /// The text nodes that are children of `id`, joined in order.
    fn child_text_content(&self, id: NodeId) -> String {
        self.children(id)
            .filter_map(|child| self.get(child).and_then(Node::text))
            .collect()
    }
}

/// Whether `c` is ASCII white space as the Infra standard has it: tab, line
/// feed, form feed, carriage return and space — and not every Unicode space.
fn is_ascii_white_space(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{c}' | '\r' | ' ')
}

/// Infra's *strip and collapse ASCII whitespace*.
fn strip_and_collapse(text: &str) -> String {
    text.split(is_ascii_white_space)
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use crate::parse::parse_document;
    use crate::xml::read_svg;

    #[test]
    fn a_title_is_its_text_with_white_space_collapsed() {
        let document =
            parse_document("<title>\n  Invoices \t\r\n for\u{c} March  </title><p>body</p>");
        assert_eq!(document.title(), "Invoices for March");
    }

    #[test]
    fn a_page_with_no_title_has_an_empty_one() {
        assert_eq!(parse_document("<p>no title</p>").title(), "");
        assert_eq!(parse_document("<title> \n </title>").title(), "");
    }

    #[test]
    fn the_first_title_in_tree_order_is_the_one_wherever_it_is() {
        let document =
            parse_document("<body><p><title>first</title></p><title>second</title></body>");
        assert_eq!(document.title(), "first");
    }

    #[test]
    fn a_titles_markup_is_text() {
        let document = parse_document("<title><b>bold</b> &amp; </style><script>x()</title>");
        assert_eq!(document.title(), "<b>bold</b> & </style><script>x()");
    }

    #[test]
    fn white_space_that_is_not_ascii_is_kept() {
        // A no-break space and an ideographic space are a title's letters.
        let document = parse_document("<title>\u{a0}a\u{3000}b </title>");
        assert_eq!(document.title(), "\u{a0}a\u{3000}b");
    }

    #[test]
    fn only_the_titles_own_text_children_count() {
        let mut document = parse_document("<title>a</title>");
        let title = document.descendants(document.root()).find(|id| {
            document
                .get(*id)
                .is_some_and(|n| n.is_html_element("title"))
        });
        let bold = document.create_element("b");
        let inner = document.create_text_node("hidden");
        let after = document.create_text_node(" b");
        assert!(title.is_some() && bold.is_ok());
        if let (Some(title), Ok(bold)) = (title, bold) {
            assert!(document.append_child(bold, inner).is_ok());
            assert!(document.append_child(title, bold).is_ok());
            assert!(document.append_child(title, after).is_ok());
        }
        assert_eq!(document.title(), "a b");
    }

    #[test]
    fn an_svg_documents_title_is_its_roots_own_title_child() {
        let document = read_svg(
            br#"<svg xmlns="http://www.w3.org/2000/svg"><g><title>inner</title></g><title> a  chart </title></svg>"#,
        );
        assert_eq!(
            document.map(|document| document.title()),
            Ok("a chart".to_owned())
        );
    }
}
