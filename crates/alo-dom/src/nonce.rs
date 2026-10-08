/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The nonce an element presents to a page's policy: Content Security
//! Policy's *is element nonceable*, for a `<script>` and a `<style>` alike
//! (ADR 0034 § 2).
//!
//! A nonce is a secret the page put in its header and on the elements it
//! wrote, so it is worth exactly as much as the guarantee that *the page*
//! wrote the element. *Is element nonceable* names the two shapes in which an
//! injection inherits a real element's nonce, and here an element in either
//! shape presents **no nonce at all**:
//!
//! - an attribute whose name or value contains `<script` or `<style` — what a
//!   dangling `<script src=… x="` leaves when it swallows the markup up to the
//!   page's own `nonce`;
//! - an attribute named twice in the tag, which the parser repairs by keeping
//!   the first and so hides from the tree (see
//!   [`crate::node::Element::had_duplicate_attributes`]).
//!
//! Reading the markup and nothing else, this runs no policy: whether the
//! nonce it presents is one a policy names is `alo-net`'s.

use crate::node::Element;

/// The nonce `element` presents, or [`None`] where it wrote none or its
/// markup is in a shape an injection leaves.
pub fn presented(element: &Element) -> Option<String> {
    let nonce = element.attr("nonce")?;
    if element.had_duplicate_attributes {
        return None;
    }
    let injected = |text: &str| {
        let lower = text.to_ascii_lowercase();
        lower.contains("<script") || lower.contains("<style")
    };
    if element
        .attrs
        .iter()
        .any(|attribute| injected(&attribute.name.local) || injected(&attribute.value))
    {
        return None;
    }
    Some(nonce.to_owned())
}

#[cfg(test)]
mod tests {
    use super::presented;
    use crate::{Document, parse_document};

    fn first_style(document: &Document) -> Option<String> {
        document
            .descendants(document.root())
            .filter_map(|id| document.element(id))
            .find(|element| element.name.is_html("style"))
            .and_then(presented)
    }

    #[test]
    fn a_style_presents_the_nonce_the_page_wrote_on_it() {
        let document = parse_document("<style nonce=abc>p{}</style>");
        assert_eq!(first_style(&document).as_deref(), Some("abc"));
    }

    #[test]
    fn a_style_in_a_shape_an_injection_leaves_presents_none() {
        for markup in [
            "<style x='<script' nonce=abc>p{}</style>",
            "<style nonce=abc nonce=def>p{}</style>",
            "<style id=a nonce=abc id=b>p{}</style>",
            "<style>p{}</style>",
        ] {
            let document = parse_document(markup);
            assert_eq!(first_style(&document), None, "{markup}");
        }
    }
}
