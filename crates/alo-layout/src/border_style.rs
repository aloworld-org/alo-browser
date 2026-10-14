/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What kind of line one side's border is: the question layout asks to know
//! whether the side has a width at all, and paint asks to know how to draw
//! it. One reading, so the two cannot disagree about which sides exist.
//!
//! # Where the style is written
//!
//! `border-<side>-style` is a longhand, and `border-style` is split into
//! it as it is written (`alo_css`'s counted shorthands). `border-<side>` and
//! `border` are not split — their parts are told apart by kind — so a block
//! holds them whole and this reads them, longhand first.
//!
//! A shorthand **resets** what it does not name: `border-left: 8px` makes
//! the left side's style `none`, its initial value, even under a
//! `border: 4px solid` written before it. So the first of the side's
//! shorthand and `border` that is written decides, whether or not it names
//! a style. Chrome 154 computes exactly that (measured, queue item 595).
//!
//! # A style of `none`
//!
//! **A side whose style is `none` or `hidden` has no width**, however its
//! width was written: CSS Backgrounds 3 computes `border-*-width` as zero
//! then, and Chrome 154 does (`border-width: 4px 10px; border-left-style:
//! none` is a left width of 0px, measured). A side with no style written
//! anywhere is `none` too, which is why a width alone never draws a border.
//!
//! What this does not do yet: a shorthand written *after* its longhand
//! (`border-left-style: none; border-left: 5px solid`) still loses to it,
//! because the cascade competes the two as different names (queue item
//! 603).

use alo_style::ComputedStyle;

/// The style of one side's border — `top`, `right`, `bottom` or `left` —
/// lowercased, or `None` when nothing written gives it one, which is its
/// initial value, `none`.
pub fn border_style(style: &ComputedStyle, side: &str) -> Option<String> {
    if let Some(text) = style.get(&format!("border-{side}-style")) {
        return Some(text.trim().to_ascii_lowercase());
    }
    [format!("border-{side}"), "border".to_owned()]
        .iter()
        .find_map(|property| style.get(property))
        .and_then(|text| alo_value::parse_border(text).style)
        .map(|kind| kind.to_ascii_lowercase())
}

/// Whether a side whose style is `style` has any border at all: not when
/// it is `none` or `hidden`, nor when no style was written.
pub fn draws_a_line(style: Option<&str>) -> bool {
    style.is_some_and(|style| style != "none" && style != "hidden")
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_css::{MediaContext, parse_stylesheet};
    use alo_dom::parse_document;
    use alo_style::{Origin, SourcedSheet, resolve};

    /// Each side's style, top, right, bottom and left, of a box given
    /// `css`.
    fn styles(css: &str) -> [Option<String>; 4] {
        let document = parse_document("<html><body><div id=x></div></body></html>");
        let sheet = parse_stylesheet(&format!("#x {{ {css} }}"));
        let sheets = [SourcedSheet::new(Origin::Author, &sheet)];
        let tree = resolve(&document, &sheets, &MediaContext::default());
        let style = document
            .descendants(document.root())
            .find(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some("x"))
            })
            .and_then(|id| tree.get(id));
        ["top", "right", "bottom", "left"]
            .map(|side| style.and_then(|style| border_style(style, side)))
    }

    fn named(styles: [Option<&str>; 4]) -> [Option<String>; 4] {
        styles.map(|style| style.map(str::to_owned))
    }

    #[test]
    fn the_longhand_beats_the_side_shorthand_which_beats_border() {
        assert_eq!(
            styles("border: 1px solid red; border-top: 2px dashed red; border-left-style: None"),
            named([Some("dashed"), Some("solid"), Some("solid"), Some("none")]),
        );
    }

    #[test]
    fn border_style_is_its_longhands() {
        assert_eq!(
            styles("border: 5px solid; border-style: inset none"),
            named([Some("inset"), Some("none"), Some("inset"), Some("none")]),
        );
    }

    #[test]
    fn a_side_shorthand_without_a_style_resets_it_to_none() {
        assert_eq!(
            styles("border: 4px solid blue; border-left: 8px"),
            named([Some("solid"), Some("solid"), Some("solid"), None]),
        );
    }

    #[test]
    fn nothing_written_is_no_style() {
        assert_eq!(styles("border-width: 8px"), named([None; 4]));
    }

    #[test]
    fn only_a_visible_style_draws_a_line() {
        assert!(draws_a_line(Some("solid")));
        assert!(draws_a_line(Some("inset")));
        assert!(!draws_a_line(Some("none")));
        assert!(!draws_a_line(Some("hidden")));
        assert!(!draws_a_line(None));
    }
}
