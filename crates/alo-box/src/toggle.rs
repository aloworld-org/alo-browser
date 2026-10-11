/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A checkbox and a radio: the boxes that stand on their bottom border edge
//! and draw themselves.
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
//! The same two draw Chrome's own artwork rather than a border and a
//! background (queue item 454): a rounded square or a circle, white inside
//! a grey edge, filled with the accent colour when checked. Which of the two
//! a box is decides its shape, and that is the **element's** type rather
//! than its role — `<input type=checkbox role=switch>` is still drawn as a
//! checkbox, because `role` speaks to assistive technology and not to the
//! screen. What it looks like is `alo_paint::toggle`'s.
//!
//! Only the two that were measured. A range and a colour well draw
//! themselves too, and stand where browsers stand them only once a page
//! shows where that is.

use alo_dom::Element;

/// Which of the two controls that draw themselves a box is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    /// `<input type=checkbox>`: a rounded square.
    CheckBox,
    /// `<input type=radio>`: a circle.
    Radio,
}

/// Which toggle an element is: an `<input>` whose `type` is `checkbox` or
/// `radio`, in any case. Its box stands on its bottom border edge and draws
/// itself.
pub(crate) fn of(element: &Element) -> Option<Toggle> {
    if !element.name.is_html("input") {
        return None;
    }
    let kind = element.attr("type")?;
    if kind.eq_ignore_ascii_case("checkbox") {
        Some(Toggle::CheckBox)
    } else if kind.eq_ignore_ascii_case("radio") {
        Some(Toggle::Radio)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::parse_document;

    /// Which toggle the first element in some markup is.
    fn first(html: &str) -> Option<Toggle> {
        let document = parse_document(html);
        document
            .descendants(document.root())
            .filter_map(|id| document.element(id))
            .find(|element| {
                !["html", "head", "body"]
                    .iter()
                    .any(|n| element.name.is_html(n))
            })
            .and_then(of)
    }

    #[test]
    fn a_checkbox_and_a_radio_are_toggles_of_their_own_kind() {
        assert_eq!(first("<input type=checkbox>"), Some(Toggle::CheckBox));
        assert_eq!(first("<input type=radio>"), Some(Toggle::Radio));
        assert_eq!(first("<input type=CheckBox>"), Some(Toggle::CheckBox));
        // The element says what it is; a role does not change the picture.
        assert_eq!(
            first("<input type=checkbox role=radio>"),
            Some(Toggle::CheckBox)
        );
    }

    #[test]
    fn nothing_else_is() {
        assert_eq!(first("<input>"), None);
        assert_eq!(first("<input type=text>"), None);
        assert_eq!(first("<input type=range>"), None);
        assert_eq!(first("<button type=checkbox>"), None);
        assert_eq!(first("<span type=radio>"), None);
        assert_eq!(first("<div role=checkbox aria-checked=true>"), None);
    }
}
