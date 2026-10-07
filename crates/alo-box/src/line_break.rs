/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `<br>`: the one element whose box is the end of a line.
//!
//! HTML's rendering section says a `<br>` is expected to render as a **forced
//! line break**: whatever comes after it starts a new line, and two in a row
//! leave a blank line between. Nothing in CSS can say that about an element —
//! `display` has no value for it — so the user-agent style sheet cannot, and
//! the tree that knows which element a box came from says it instead, the way
//! it says which `<legend>` a fieldset shows in its border.
//!
//! What a break *does* — ending the line it is on, and making a line of its
//! own when there is nothing else on it — is the line builder's, in
//! `alo_layout::inline`. This is only which boxes are one.
//!
//! # Only while it is an inline box
//!
//! A `<br>` is inline by the user-agent sheet's default, and that is when it
//! is a break. An author who gives one another `display` gets the box they
//! wrote: a block or an inline-block with nothing in it, which takes no room.
//! Browsers keep a `<br>` a break whatever its `display` says short of
//! `none`; this engine does not yet, and `docs/conformance.md` says so,
//! because no page has been seen to do it.

use crate::display::{Display, Inside, Outside};
use alo_dom::Element;

/// Whether an element's box is a forced line break.
pub(crate) fn is_forced_break(element: &Element, display: Display) -> bool {
    element.name.is_html("br")
        && display.outside() == Some(Outside::Inline)
        && display.inside() == Some(Inside::Flow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::parse_document;

    /// Whether the first `name` element in some markup is a break under
    /// `display`.
    fn first_of(html: &str, name: &str, display: &str) -> bool {
        let document = parse_document(html);
        let display = Display::parse(display).expect("a display this engine reads");
        document
            .descendants(document.root())
            .filter_map(|id| document.element(id))
            .find(|element| element.name.is_html(name))
            .is_some_and(|element| is_forced_break(element, display))
    }

    #[test]
    fn an_inline_br_is_a_break() {
        assert!(first_of("<p>one<br>two</p>", "br", "inline"));
    }

    #[test]
    fn a_br_given_another_display_is_the_box_the_author_wrote() {
        assert!(!first_of("<p>one<br>two</p>", "br", "block"));
        assert!(!first_of("<p>one<br>two</p>", "br", "inline-block"));
        assert!(!first_of("<p>one<br>two</p>", "br", "inline-flex"));
    }

    #[test]
    fn no_other_element_is_a_break() {
        assert!(!first_of("<p>one<span></span>two</p>", "span", "inline"));
        assert!(!first_of("<p>one<wbr>two</p>", "wbr", "inline"));
    }
}
