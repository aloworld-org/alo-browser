/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An element's `style` attribute, as the declarations attached to it.
//!
//! CSS Style Attributes: the attribute's value is the contents of a
//! declaration block — no selector and no braces — read by the sheet's own
//! parser, with the sheet's refusals and the sheet's shorthand splitting
//! ([`alo_css::parse_declaration_list`]). CSS Cascade 4 § 6.1 then places
//! those declarations above every declaration of the same importance that a
//! selector reached, which is [`crate::cascade`]'s business (ADR 0033 § 1).
//!
//! **It is read from the attribute every time it is asked.** Nothing parsed is
//! kept on the element: the attribute is the only copy of the inline block,
//! so there is nothing here that could disagree with it.
//!
//! Only an HTML or an SVG element has one, as only they are
//! `ElementCSSInlineStyle`. A `style` inside a `<template>`'s contents is
//! never asked about, because those contents are not in the document and
//! nothing in them is styled.

use alo_css::{DeclarationBlock, StyleIssue};
use alo_dom::{Element, Namespace};

/// The declarations an element's `style` attribute holds, and a record of
/// every one that could not be read.
///
/// Nothing for an element with no `style` attribute, and nothing for one
/// outside the HTML and SVG namespaces, whose `style` is an attribute
/// nobody reads.
pub fn declarations(element: &Element, issues: &mut Vec<StyleIssue>) -> DeclarationBlock {
    if !matches!(element.name.ns, Namespace::Html | Namespace::Svg) {
        return DeclarationBlock::new();
    }
    let Some(text) = element.attr("style") else {
        return DeclarationBlock::new();
    };
    let (block, dropped) = alo_css::parse_declaration_list(text);
    // A location within the attribute says little on its own, so each issue
    // also says which element's attribute it was.
    issues.extend(dropped.into_iter().map(|issue| StyleIssue {
        source: format!(
            "style attribute of <{}>: {}",
            element.name.local, issue.source
        ),
        ..issue
    }));
    block
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_css::{IssueKind, PropertyName};
    use alo_dom::{Document, parse_document};

    fn element_with_id<'a>(document: &'a Document, wanted: &str) -> &'a Element {
        document
            .descendants(document.root())
            .filter_map(|id| document.element(id))
            .find(|element| element.attr("id") == Some(wanted))
            .unwrap_or_else(|| panic!("no element with id {wanted}"))
    }

    #[test]
    fn a_style_attribute_is_the_declarations_it_holds() {
        let document = parse_document(r#"<p id=x style="color: red; margin: 0 !important">t</p>"#);
        let mut issues = Vec::new();
        let block = declarations(element_with_id(&document, "x"), &mut issues);
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(block.len(), 6, "a colour, and a margin with its four sides");
        assert_eq!(
            block
                .get(&PropertyName::parse("margin-top"))
                .map(|d| d.importance.is_important()),
            Some(true),
        );
    }

    #[test]
    fn an_element_with_no_style_attribute_has_nothing_attached() {
        let document = parse_document("<p id=x>t</p>");
        let mut issues = Vec::new();
        assert!(declarations(element_with_id(&document, "x"), &mut issues).is_empty());
        assert!(issues.is_empty());
    }

    #[test]
    fn an_svg_element_has_one_and_a_mathml_element_does_not() {
        let document = parse_document(
            r#"<svg><rect id=s style="fill: red"/></svg><math><mi id=m style="color: red">x</mi></math>"#,
        );
        let mut issues = Vec::new();
        assert_eq!(
            declarations(element_with_id(&document, "s"), &mut issues).len(),
            1
        );
        assert!(declarations(element_with_id(&document, "m"), &mut issues).is_empty());
    }

    #[test]
    fn a_declaration_that_cannot_be_read_is_said_with_its_element() {
        let document = parse_document(r#"<div id=x style="color: red; 12px; width: 3px">t</div>"#);
        let mut issues = Vec::new();
        let block = declarations(element_with_id(&document, "x"), &mut issues);
        assert_eq!(block.len(), 2);
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!(issues[0].kind, IssueKind::InvalidDeclaration);
        assert_eq!(issues[0].source, "style attribute of <div>: 12px;");
    }
}
