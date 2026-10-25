/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `<textarea>` scrolls whatever its author says (queue item 663).
//!
//! The engine's sheet gives a `<textarea>` `overflow: auto`, as Chrome's
//! does, so the lines it holds past its edge are cut off there and can be
//! scrolled to. An author may write `overflow: visible` over that, and
//! Chrome 155 computes it as `auto` anyway — measured, on `overflow:
//! visible`, on `overflow-y: visible` alone and on a `<textarea>` made a
//! block: each computes `auto` on both axes and is scrolled by 40 when its
//! `scrollTop` is set to 40. Every other value is kept as written: `hidden`
//! and `scroll` are scrolled, `clip` is not, and `overflow-x: clip` beside
//! the sheet's `auto` scrolls down.
//!
//! So an axis whose computed `overflow` is `visible` — written so, or left
//! at the initial value by `initial` or `unset` — is made `auto` here, on
//! the longhand, which every reader asks before the shorthand.

use crate::computed::ComputedStyle;
use alo_css::PropertyName;

/// Make each axis of a `<textarea>`'s `overflow` that computed `visible`
/// `auto`, as Chrome does. `element` is the element `style` is the computed
/// style of; anything but a `<textarea>` is left alone.
pub(crate) fn scroll_whatever_the_author_says(
    element: Option<&alo_dom::Element>,
    style: &mut ComputedStyle,
) {
    if !element.is_some_and(|element| element.name.is_html("textarea")) {
        return;
    }
    let both = style.get("overflow").map(str::to_owned);
    for axis in ["overflow-x", "overflow-y"] {
        let computed = style.get(axis).map(str::to_owned).or_else(|| both.clone());
        if computed.is_none_or(|value| value.trim().eq_ignore_ascii_case("visible")) {
            style.set(&PropertyName::parse(axis), "auto".to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{ComputedStyle, Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
    use alo_css::{MediaContext, parse_stylesheet};
    use alo_dom::parse_document;

    /// The computed `overflow-x` and `overflow-y` of the element whose `id`
    /// is `t` in `page`, read as layout reads them: the longhand, else the
    /// shorthand.
    fn overflow(page: &str) -> (String, String) {
        let document = parse_document(page);
        let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let styles = resolve(
            &document,
            &[SourcedSheet::new(Origin::UserAgent, &agent)],
            &MediaContext::default(),
        );
        let style: ComputedStyle = document
            .descendants(document.root())
            .find(|&id| document.attribute_by_name(id, "id") == Some("t"))
            .and_then(|id| styles.get(id))
            .cloned()
            .unwrap_or_default();
        let read = |axis: &str| {
            style
                .get(axis)
                .or_else(|| style.get("overflow"))
                .unwrap_or("visible")
                .to_owned()
        };
        (read("overflow-x"), read("overflow-y"))
    }

    fn pair(x: &str, y: &str) -> (String, String) {
        (x.to_owned(), y.to_owned())
    }

    #[test]
    fn a_text_area_is_auto_by_the_engines_sheet() {
        assert_eq!(overflow("<textarea id=t></textarea>"), pair("auto", "auto"));
    }

    #[test]
    fn visible_written_by_an_author_computes_auto() {
        for style in [
            "overflow: visible",
            "overflow: VISIBLE",
            "overflow: initial",
            "overflow: unset",
            "overflow: visible; display: block",
        ] {
            assert_eq!(
                overflow(&format!("<textarea id=t style='{style}'></textarea>")),
                pair("auto", "auto"),
                "{style}"
            );
        }
        assert_eq!(
            overflow("<textarea id=t style='overflow-y: visible'></textarea>"),
            pair("auto", "auto")
        );
    }

    #[test]
    fn every_other_value_is_kept() {
        for value in ["hidden", "scroll", "clip"] {
            assert_eq!(
                overflow(&format!(
                    "<textarea id=t style='overflow: {value}'></textarea>"
                )),
                pair(value, value),
                "{value}"
            );
        }
        assert_eq!(
            overflow("<textarea id=t style='overflow-x: clip'></textarea>"),
            pair("clip", "auto")
        );
    }

    #[test]
    fn nothing_but_a_text_area_is_changed() {
        assert_eq!(
            overflow("<div id=t style='overflow: visible'></div>"),
            pair("visible", "visible")
        );
        assert_eq!(overflow("<input id=t>"), pair("visible", "visible"));
    }
}
