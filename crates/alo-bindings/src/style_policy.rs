/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether an element's `style` attribute is applied under the page's
//! policies (ADR 0034 §§ 1–3, queue item 343).
//!
//! **One function, two askers.** The renderer asks [`applied`] for every
//! element it draws, and a refused attribute contributes no declarations;
//! `element.style` asks it on every read, and a refused attribute reads as
//! `""`. Both must give the same answer, which is why it is one function:
//! a `CSSStyleDeclaration` that read the refused text would write it back
//! as its own on the page's first `el.style.anything = …`, and the record
//! would then admit what was injected (§ 3).
//!
//! The answer is yes when the attribute is the text the element's
//! declaration last wrote ([`alo_dom::Element::style_is_declared`]) — CSSOM
//! exempts what `element.style` writes, and so does every engine — or when
//! every policy the page enforces allows it as a `style` attribute: no nonce,
//! and a digest that counts only under `'unsafe-hashes'`.
//!
//! The policies are those the page holds **now**: its headers, and every
//! `<meta>` policy the parser has made so far, which the renderer states into
//! the document's cell ([`state`]) when the page's heap is made and whenever
//! a `<meta>` adds one, as it states the page's URL.

use alo_dom::Element;
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_net::Policies;
use alo_net::csp::{Content, Inline, Refusal};

use crate::document_cell::DocumentCell;

/// Whether `element`'s `style` attribute is applied under `policies`: the
/// first enforced refusal if it is not.
///
/// An element with no `style` attribute has nothing to refuse.
///
/// # Errors
///
/// The [`Refusal`] of the first enforced policy that does not allow the
/// attribute.
pub fn applied(element: &Element, policies: &Policies) -> Result<(), Refusal> {
    let Some(text) = element.attr("style") else {
        return Ok(());
    };
    if element.style_is_declared() {
        return Ok(());
    }
    policies.allows_inline(Inline::Style, None, Content::attribute(text))
}

/// State the policies the page holds into the document cell `cell`, in
/// place of what it held: whether `cell` is a document cell.
pub fn state(objects: &mut Objects, cell: Ref, policies: Policies) -> bool {
    objects
        .write_embedded::<DocumentCell, _>(cell, |held, _| held.policies = policies)
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::applied;
    use alo_dom::{Document, NodeId, parse_document};
    use alo_net::{Headers, Policies};

    fn under(policy: &str) -> Policies {
        let mut headers = Headers::new();
        headers.add("Content-Security-Policy", policy);
        Policies::stated_by(&headers)
    }

    fn watched(policy: &str) -> Policies {
        let mut headers = Headers::new();
        headers.add("Content-Security-Policy-Report-Only", policy);
        Policies::stated_by(&headers)
    }

    fn paragraph(document: &Document) -> NodeId {
        document
            .descendants(document.root())
            .find(|id| document.get(*id).is_some_and(|n| n.is_html_element("p")))
            .unwrap_or_else(|| panic!("the markup has a <p>"))
    }

    fn is_applied(document: &Document, policies: &Policies) -> bool {
        document
            .element(paragraph(document))
            .is_some_and(|element| applied(element, policies).is_ok())
    }

    #[test]
    fn a_style_attribute_in_markup_is_refused_by_a_policy_without_unsafe_inline() {
        let document = parse_document(r#"<p style="color: red">x</p>"#);
        assert!(!is_applied(&document, &under("style-src 'self'")));
        assert!(!is_applied(&document, &under("default-src 'none'")));
        assert!(is_applied(&document, &under("style-src 'unsafe-inline'")));
        assert!(is_applied(&document, &under("script-src 'none'")));
        assert!(is_applied(&document, &Policies::none()));
        assert!(
            is_applied(&document, &watched("style-src 'none'")),
            "a report-only policy refuses nothing",
        );
    }

    #[test]
    fn a_digest_counts_for_an_attribute_only_under_unsafe_hashes() {
        let document = parse_document(r#"<p style="color: red">x</p>"#);
        // SHA-256 of `color: red`, in base64.
        let digest = "'sha256-NerDAUWfwD31YdZHveMrq0GLjsNFMwxLpZl0dPUeCcw='";
        assert!(!is_applied(
            &document,
            &under(&format!("style-src {digest}"))
        ));
        assert!(is_applied(
            &document,
            &under(&format!("style-src 'unsafe-hashes' {digest}"))
        ));
    }

    #[test]
    fn what_the_declaration_wrote_is_applied_under_any_policy_until_replaced() {
        let mut document = parse_document("<p>x</p>");
        let p = paragraph(&document);
        let refusing = under("style-src 'none'");
        assert!(is_applied(&document, &refusing), "nothing to refuse");
        document.set_declared_style(p, "color: blue;");
        assert!(is_applied(&document, &refusing));
        document.set_attribute(p, "style", "color: red;");
        assert!(
            !is_applied(&document, &refusing),
            "a setAttribute replacing it is judged by the policy",
        );
    }
}
