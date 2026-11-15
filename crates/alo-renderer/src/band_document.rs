/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The frame's band's document, built from its state (queue item 991).
//!
//! As the tab strip's is ([`crate::strip_document`], ADR 0024 § 4): the only
//! markup is [`TEMPLATE`], ours and fixed, and the page's title is put into
//! it as a text node with `alo-dom`'s operations, so no title a page chooses
//! becomes an element. It runs no script, and the template has none.
//!
//! # What an agent reads
//!
//! A `banner` holding the page's title, then the application's name — or
//! the title alone, when it is the name ([`Band::names_the_application`]).

use crate::band::{APPLICATION, Band};
use alo_dom::{Document, NodeId, parse_document};

/// The band's markup, compiled in: a document with one empty `banner`.
pub const TEMPLATE: &str = include_str!("../interface/band.html");

/// The band's stylesheet, after alo OS's window band, compiled in.
pub const SHEET: &str = include_str!("../interface/band.css");

/// The id of the template's `banner`, which the title and name go into.
const BAND: &str = "band";

/// Why the band could not be built: an `alo-dom` operation refused, or the
/// template had no `banner`. Either is a defect in this file or its
/// template, never in what the browser process sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unbuilt {
    /// In words.
    pub why: String,
}

/// The band's document for `band`: the template, holding the page's title
/// as text, then the application's name unless the title is it.
///
/// # Errors
///
/// [`Unbuilt`] if the template has no `banner` or `alo-dom` refused an
/// operation, which no state can cause.
pub fn build(band: &Band) -> Result<Document, Unbuilt> {
    let mut document = parse_document(TEMPLATE);
    let banner = document
        .descendants(document.root())
        .find(|&node| document.attribute_by_name(node, "id") == Some(BAND))
        .ok_or_else(|| Unbuilt {
            why: format!("the band's template has no element with the id {BAND:?}"),
        })?;
    // The one place a stranger's string enters the document, and it enters
    // as a text node's data: no parser ever sees it.
    let title = span(&mut document, "title", band.label())?;
    append(&mut document, banner, title)?;
    if band.names_the_application() {
        let name = span(&mut document, "name", APPLICATION)?;
        append(&mut document, banner, name)?;
    }
    Ok(document)
}

/// A `span` of `class` holding `text` as one text node.
fn span(document: &mut Document, class: &str, text: &str) -> Result<NodeId, Unbuilt> {
    let made = document.create_element("span").map_err(|refused| Unbuilt {
        why: format!("a <span> could not be made: {refused}"),
    })?;
    document
        .set_attribute(made, "class", class)
        .ok_or_else(|| Unbuilt {
            why: "class could not be set on a node that is not an element".to_owned(),
        })?;
    let words = document.create_text_node(text);
    append(document, made, words)?;
    Ok(made)
}

fn append(document: &mut Document, parent: NodeId, child: NodeId) -> Result<(), Unbuilt> {
    document
        .append_child(parent, child)
        .map(|_| ())
        .map_err(|refused| Unbuilt {
            why: format!("a node could not be put into the band: {refused}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::NodeKind;
    use alo_layout::Size;

    fn built(title: &str) -> Document {
        let band = Band::new(title, "https://example.com/", Size::new(600.0, 54.0));
        build(&band).unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why))
    }

    /// Each element's class, in document order, and its text.
    fn spans(document: &Document) -> Vec<(String, String)> {
        document
            .descendants(document.root())
            .filter_map(|node| {
                let class = document.attribute_by_name(node, "class")?;
                (class != BAND).then(|| (class.to_owned(), document.text_content(node)))
            })
            .collect()
    }

    #[test]
    fn the_title_then_the_name() {
        assert_eq!(
            spans(&built("Invoices")),
            vec![
                ("title".to_owned(), "Invoices".to_owned()),
                ("name".to_owned(), APPLICATION.to_owned())
            ]
        );
    }

    #[test]
    fn a_title_that_is_the_name_is_drawn_once() {
        assert_eq!(
            spans(&built(APPLICATION)),
            vec![("title".to_owned(), APPLICATION.to_owned())]
        );
    }

    #[test]
    fn a_title_that_looks_like_markup_is_one_text_node() {
        for title in ["<b>bold</b>", "</style><script>x()</script>", "a\0b"] {
            let document = built(title);
            let elements: Vec<String> = document
                .descendants(document.root())
                .filter_map(|node| document.element(node))
                .map(|element| element.name.local.to_string())
                .collect();
            assert!(
                !elements.iter().any(|name| name == "b" || name == "script"),
                "{title:?}: {elements:?}"
            );
            let texts = document
                .descendants(document.root())
                .filter(|&node| matches!(document.kind(node), Some(NodeKind::Text(data)) if data == title))
                .count();
            assert_eq!(texts, 1, "{title:?}");
        }
    }
}
