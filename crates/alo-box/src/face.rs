/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A button and a text field: the controls Chrome draws a picture of while
//! the page leaves their border and background alone.
//!
//! Chrome 154 computes a `2px outset` border for a button and a `2px inset`
//! one for a field, and draws neither. In the room they take it draws a
//! one-pixel grey edge with rounded corners, over the control's fill
//! (queue item 505). A page that writes any border or background of its
//! own gets its CSS drawn instead. Whether it did is the style's question,
//! and what the picture is belongs to `alo_paint::face`. This is only which
//! boxes are these controls, which is the **element's** to say, as it is
//! for a checkbox ([`crate::toggle`]).
//!
//! Only what was measured. A `<select>` and a `<textarea>` keep their one
//! solid pixel here, and a colour well, a file chooser and a range draw
//! themselves differently again, which waits for a page that shows how.

use alo_dom::Element;

/// Which of the two controls with Chrome's picture a box is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    /// A `<button>`, or an `<input>` of type `button`, `submit` or `reset`:
    /// filled `#efefef`.
    Button,
    /// An `<input>` a person types into, a date or a time included: filled
    /// white.
    Field,
}

/// The `<input>` types that are a field. A missing or unknown `type` is
/// `text`, as HTML says, so it is a field too.
const NOT_A_FIELD: [&str; 10] = [
    "checkbox", "radio", "hidden", "color", "file", "range", "image", "button", "submit", "reset",
];

/// Which face an element draws, if it is a button or a field.
pub(crate) fn of(element: &Element) -> Option<Face> {
    if element.name.is_html("button") {
        return Some(Face::Button);
    }
    if !element.name.is_html("input") {
        return None;
    }
    let kind = element.attr("type").unwrap_or("text").trim();
    if ["button", "submit", "reset"]
        .iter()
        .any(|button| kind.eq_ignore_ascii_case(button))
    {
        Some(Face::Button)
    } else if NOT_A_FIELD
        .iter()
        .any(|other| kind.eq_ignore_ascii_case(other))
    {
        None
    } else {
        Some(Face::Field)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::parse_document;

    /// Which face the first element in some markup draws.
    fn first(html: &str) -> Option<Face> {
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
    fn a_button_is_a_button_whatever_its_own_type() {
        assert_eq!(first("<button>b</button>"), Some(Face::Button));
        assert_eq!(first("<button type=reset>b</button>"), Some(Face::Button));
        for kind in ["button", "submit", "RESET"] {
            assert_eq!(
                first(&format!("<input type={kind}>")),
                Some(Face::Button),
                "{kind}"
            );
        }
    }

    #[test]
    fn a_field_is_any_input_a_person_types_into() {
        for kind in [
            "",
            " type=text",
            " type=search",
            " type=email",
            " type=password",
            " type=Date",
            " type=time",
            " type=number",
            " type=nonsense",
        ] {
            assert_eq!(
                first(&format!("<input{kind}>")),
                Some(Face::Field),
                "{kind}"
            );
        }
    }

    #[test]
    fn nothing_else_is() {
        for kind in [
            "checkbox", "radio", "hidden", "color", "file", "range", "image",
        ] {
            assert_eq!(first(&format!("<input type={kind}>")), None, "{kind}");
        }
        assert_eq!(first("<select></select>"), None);
        assert_eq!(first("<textarea></textarea>"), None);
        assert_eq!(first("<div role=button>b</div>"), None);
    }
}
