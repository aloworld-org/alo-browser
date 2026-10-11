/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A checkbox and a radio: the boxes that stand on their bottom border edge.
//!
//! An atomic box with no line in it stands on the line by its bottom
//! **margin** edge (CSS 2 § 10.8.1), and that is where a picture and an
//! empty `inline-block` stand. A checkbox and a radio do not. Chrome 154
//! stands each on the bottom of its **border** box, measured with the
//! corpus's font at 16 and 32 pixels, with its own margins and with none,
//! and at 13 and 20 pixels square (queue item 443). With the user-agent
//! sheet's `margin-bottom: 3px` the difference is three pixels: a checkbox
//! beside a word would stand that much higher, and its line that much
//! taller, than in every browser.
//!
//! Nothing in CSS says this about an element, so the user-agent sheet
//! cannot, and the tree that knows which element a box came from says it,
//! the way it says which box is a `<br>`. Where the line puts a box is
//! `alo_layout::baseline`'s; this is only which boxes these are.
//!
//! Only the two that were measured. A range and a colour well draw
//! themselves too, and stand where browsers stand them only once a page
//! shows where that is.

use alo_dom::Element;

/// Whether an element's box stands on its bottom border edge: an `<input>`
/// whose `type` is `checkbox` or `radio`, in any case.
pub(crate) fn stands_on_its_border_edge(element: &Element) -> bool {
    element.name.is_html("input")
        && element.attr("type").is_some_and(|kind| {
            kind.eq_ignore_ascii_case("checkbox") || kind.eq_ignore_ascii_case("radio")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::parse_document;

    /// Whether the first element in some markup stands on its border edge.
    fn first(html: &str) -> bool {
        let document = parse_document(html);
        document
            .descendants(document.root())
            .filter_map(|id| document.element(id))
            .find(|element| {
                !["html", "head", "body"]
                    .iter()
                    .any(|n| element.name.is_html(n))
            })
            .is_some_and(stands_on_its_border_edge)
    }

    #[test]
    fn a_checkbox_and_a_radio_stand_on_their_border_edge() {
        assert!(first("<input type=checkbox>"));
        assert!(first("<input type=radio>"));
        assert!(first("<input type=CheckBox>"));
    }

    #[test]
    fn nothing_else_does() {
        assert!(!first("<input>"));
        assert!(!first("<input type=text>"));
        assert!(!first("<input type=range>"));
        assert!(!first("<button type=checkbox>"));
        assert!(!first("<span type=radio>"));
    }
}
