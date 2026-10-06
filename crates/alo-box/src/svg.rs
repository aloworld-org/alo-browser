/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What an `<svg>` in a page is as a box (ADR 0022 §§ 1, 4).
//!
//! An **outermost** `<svg>` — one whose parent is not an SVG element — is a
//! replaced box: sized like a picture, and with nothing inside it that is a
//! box. A `<path>` has no flow, no margins and no line, and laying it out as an
//! empty inline box, which is what happened before this file, gave the agent
//! a tree of boxes that meant nothing. What is inside is drawn, after layout,
//! by `alo-svg` (queue item 271 onwards); this file only says how big the box is and
//! what it is called.
//!
//! # Its size
//!
//! CSS first, and that half is layout's: a stylesheet's `width` is a definite
//! size, and a definite size wins over anything here. The `width` and
//! `height` attributes are in that half too. They are presentation attributes
//! (SVG 2, item 279), declarations in the cascade (`alo-style`'s
//! `presentation.rs`) that any stylesheet beats, so `width="50%"` is fifty per
//! cent of what layout knows it is of and `height="2em"` is two of the
//! `<svg>`'s own font size. Then what the element says of itself, which this
//! file reads into a [`NaturalSize`]:
//!
//! - the `width` and `height` attributes, when they are absolute lengths;
//! - a ratio from them when there are both, and otherwise from the `viewBox`;
//! - and nothing at all when there is neither, which layout turns into CSS's
//!   default of 300 × 150.
//!
//! A `width` in per cent or in `em` is not a natural size — it is a share of
//! something outside the picture — and so is not one here. It is not lost: it
//! reached layout through the cascade. A value that is not a size at all was
//! recorded by the cascade when it ignored the declaration, and is not
//! recorded twice.
//!
//! # The bytes are a stranger's
//!
//! Every number here came from an attribute anybody can write. A value that is
//! not a finite, non-negative number is no natural size, and the element is
//! sized as though it had not been written — never a panic, never an infinity
//! handed to layout. The parsing below allocates nothing in proportion to its input.

use crate::natural::NaturalSize;
use alo_dom::{Document, Element, Namespace, NodeId};

/// Whether this element is an `<svg>` drawn as one picture in an HTML page:
/// in the SVG namespace, and not inside another SVG element.
///
/// A nested `<svg>` is a viewport inside its parent's drawing (ADR 0022 § 1)
/// and is not laid out at all. It is never reached here in practice — nothing
/// inside an outermost one becomes a box — but saying so keeps the rule in
/// one place rather than relying on a walk that happens not to go there.
pub fn is_outermost(document: &Document, id: NodeId, element: &Element) -> bool {
    element.name.ns == Namespace::Svg
        && &*element.name.local == "svg"
        && !document
            .parent(id)
            .and_then(|parent| document.element(parent))
            .is_some_and(|parent| parent.name.ns == Namespace::Svg)
}

/// What an outermost `<svg>` says about its own size, and what it said that
/// this engine did not use.
pub fn natural_size(element: &Element) -> (NaturalSize, Vec<String>) {
    let mut refused = Vec::new();
    let width = element.attr("width").and_then(attribute_length);
    let height = element.attr("height").and_then(attribute_length);
    let stated_ratio = match element.attr("viewBox").map(view_box) {
        None => None,
        Some(Some(view)) => view.ratio(),
        Some(None) => {
            refused.push(format!(
                "<svg viewBox={:?}>: not four numbers with a positive width and height",
                element.attr("viewBox").unwrap_or_default(),
            ));
            None
        }
    };
    (
        NaturalSize {
            width,
            height,
            stated_ratio,
        },
        refused,
    )
}

/// The text of an outermost `<svg>`'s first child `<title>`, which is SVG's
/// own way of naming a picture (ADR 0022 § 4).
///
/// The *first* child, and a child only: a `<title>` deeper down names the
/// shape it sits in, not the picture. Whitespace is collapsed, and a title of
/// nothing is no name, as an `alt` of nothing is.
pub fn title(document: &Document, id: NodeId) -> Option<String> {
    let first = document.children(id).find(|child| {
        document
            .element(*child)
            .is_some_and(|element| element.name.ns == Namespace::Svg)
    })?;
    let element = document.element(first)?;
    if &*element.name.local != "title" {
        return None;
    }
    let text = document.text_content(first);
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

/// A `viewBox`: the rectangle of user space an `<svg>` shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewBox {
    /// Its left edge, in user units.
    pub x: f32,
    /// Its top edge, in user units.
    pub y: f32,
    /// How wide it is. Never negative; zero is allowed and draws nothing.
    pub width: f32,
    /// How tall it is. Never negative; zero is allowed and draws nothing.
    pub height: f32,
}

impl ViewBox {
    /// Width over height, or [`None`] for a box with no area — which SVG says
    /// disables rendering, and which has no shape to keep.
    pub fn ratio(self) -> Option<f32> {
        if self.width <= 0.0 || self.height <= 0.0 {
            return None;
        }
        Some(self.width / self.height).filter(|ratio| ratio.is_finite() && *ratio > 0.0)
    }
}

/// Read a `viewBox`: four numbers separated by whitespace, a comma, or both.
///
/// A negative width or height is an error, as SVG says, and so is anything
/// that is not exactly four finite numbers. An error is [`None`], and the
/// `<svg>` is then sized as though it had no `viewBox`.
pub fn view_box(text: &str) -> Option<ViewBox> {
    let mut numbers = [0.0_f32; 4];
    let mut count = 0_usize;
    for word in text
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|word| !word.is_empty())
    {
        // Stops at the fifth rather than counting them all: a `viewBox` of a
        // million numbers is refused after reading five of them.
        let slot = numbers.get_mut(count)?;
        *slot = svg_number(word)?;
        count += 1;
    }
    let [x, y, width, height] = numbers;
    (count == 4 && width >= 0.0 && height >= 0.0).then_some(ViewBox {
        x,
        y,
        width,
        height,
    })
}

/// One number as SVG writes it, finite or nothing.
///
/// Rust's own float parser is wider than SVG's grammar — it takes `inf` and
/// `NaN` — and the finiteness check is what closes that gap, along with an
/// exponent large enough to overflow.
fn svg_number(word: &str) -> Option<f32> {
    if !word
        .bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.' | b'e' | b'E'))
    {
        return None;
    }
    word.parse::<f32>().ok().filter(|value| value.is_finite())
}

/// A `width` or `height` attribute as CSS pixels, if it is an absolute size.
///
/// A plain number is user units, which on an outermost `<svg>` are CSS pixels.
/// A length in an absolute unit is converted. A percentage, a font-relative
/// length or a `calc()` is a share of something outside the picture, which
/// layout resolves from the declaration (see the module comment), and anything
/// else is not a size.
fn attribute_length(text: &str) -> Option<f32> {
    let text = text.trim();
    let pixels = match svg_number(text) {
        Some(number) => number,
        None => match alo_value::parse_length_percentage(text)? {
            alo_value::LengthPercentage::Length(length) => length.to_absolute_px()?,
            alo_value::LengthPercentage::Percentage(_) | alo_value::LengthPercentage::Calc(_) => {
                return None;
            }
        },
    };
    (pixels.is_finite() && pixels >= 0.0).then_some(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::parse_document;

    fn svg_in(html: &str) -> (Document, NodeId) {
        let document = parse_document(html);
        let id = document
            .descendants(document.root())
            .find(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| &*element.name.local == "svg")
            })
            .expect("an <svg>");
        (document, id)
    }

    fn natural(attributes: &str) -> (NaturalSize, Vec<String>) {
        let (document, id) = svg_in(&format!("<svg {attributes}></svg>"));
        natural_size(document.element(id).expect("an element"))
    }

    #[test]
    fn only_an_svg_outside_svg_is_outermost() {
        let (document, outer) = svg_in("<p><svg><svg id=inner></svg></svg></p>");
        let element = document.element(outer).expect("an element");
        assert!(is_outermost(&document, outer, element));
        let inner = document
            .descendants(outer)
            .find(|id| *id != outer)
            .expect("the nested one");
        let element = document.element(inner).expect("an element");
        assert!(!is_outermost(&document, inner, element));
    }

    #[test]
    fn width_and_height_attributes_are_a_natural_size_and_a_ratio() {
        let (size, refused) = natural("width=48 height=24");
        assert_eq!(size.width, Some(48.0));
        assert_eq!(size.height, Some(24.0));
        assert_eq!(size.ratio(), Some(2.0));
        assert!(refused.is_empty());
    }

    #[test]
    fn a_unit_in_an_attribute_is_converted() {
        assert_eq!(natural("width=48px").0.width, Some(48.0));
        assert_eq!(natural("width=1in").0.width, Some(96.0));
        assert_eq!(natural("width='  12 '").0.width, Some(12.0));
    }

    #[test]
    fn a_view_box_is_a_ratio_and_not_a_size() {
        let (size, refused) = natural("viewBox='0 0 24 12'");
        assert_eq!(size.width, None);
        assert_eq!(size.height, None);
        assert_eq!(size.ratio(), Some(2.0));
        assert!(refused.is_empty());
    }

    #[test]
    fn nothing_written_is_nothing_known() {
        let (size, refused) = natural("");
        assert_eq!(size, NaturalSize::default());
        assert!(refused.is_empty());
    }

    /// A relative size is layout's, through the cascade, and is neither a
    /// natural size nor something this file refused.
    #[test]
    fn a_relative_size_is_not_a_natural_size() {
        let (size, refused) = natural("width=50% height=2em viewBox='0 0 4 1'");
        assert_eq!(size.width, None);
        assert_eq!(size.height, None);
        assert_eq!(size.ratio(), Some(4.0), "the viewBox still gives the shape");
        assert!(refused.is_empty(), "{refused:?}");
        let (size, _) = natural("width='calc(10px + 1em)'");
        assert_eq!(size.width, None);
    }

    #[test]
    fn view_box_separators_may_be_commas_spaces_or_both() {
        let expected = ViewBox {
            x: -1.0,
            y: 2.5,
            width: 24.0,
            height: 0.5,
        };
        assert_eq!(view_box("-1 2.5 24 .5"), Some(expected));
        assert_eq!(view_box("-1,2.5,24,.5"), Some(expected));
        assert_eq!(view_box("  -1 ,\t2.5\n, 24  0.5 "), Some(expected));
        assert_eq!(view_box("0 0 2.4e1 1E1").map(|v| v.width), Some(24.0));
    }

    #[test]
    fn a_view_box_with_no_area_is_kept_and_has_no_ratio() {
        let flat = view_box("0 0 24 0").expect("zero is allowed");
        assert_eq!(flat.ratio(), None);
        assert_eq!(natural("viewBox='0 0 0 0'").0.ratio(), None);
    }

    /// `LOOP.md` stage 2 § 2: what a stranger wrote is refused, never a
    /// panic, and never a number layout cannot use.
    #[test]
    fn hostile_view_boxes_are_refused() {
        let million = "1 ".repeat(1_000_000);
        for bad in [
            "",
            "0 0 24",
            "0 0 24 24 24",
            "0 0 -24 24",
            "0 0 24 -1",
            "0 0 inf 24",
            "0 0 NaN 24",
            "0 0 1e39 24",
            "0 0 24px 24",
            "0 0 24 24;",
            "a b c d",
            "0 0 0x18 24",
            "0 0 calc(1) 24",
            "\u{0}\u{0}\u{0}\u{0}",
            "0 0 ２４ 24",
            million.as_str(),
        ] {
            assert_eq!(view_box(bad), None, "{bad:.40?}");
        }
        // `1e-99999999999` underflows to zero rather than overflowing, and zero
        // is a legal height: kept, and drawn as nothing.
        assert_eq!(
            view_box("0 0 24 1e-99999999999").map(ViewBox::ratio),
            Some(None),
        );
    }

    /// The cascade records a hostile size when it ignores the declaration;
    /// here it is only not a natural size.
    #[test]
    fn hostile_sizes_are_no_natural_size() {
        for bad in [
            "-1",
            "inf",
            "NaN",
            "1e39",
            "1e39px",
            "twelve",
            "12 12",
            "calc(1px / 0)",
            "",
        ] {
            let (size, refused) = natural(&format!("width='{bad}'"));
            assert_eq!(size.width, None, "{bad:?}");
            assert!(refused.is_empty(), "{bad:?} was recorded twice");
        }
    }

    #[test]
    fn a_title_child_names_the_picture() {
        let (document, id) = svg_in("<svg><title>  The alo\n hand </title><path/></svg>");
        assert_eq!(title(&document, id).as_deref(), Some("The alo hand"));
    }

    #[test]
    fn only_a_first_child_title_names_it() {
        let (document, id) = svg_in("<svg><path/><title>Late</title></svg>");
        assert_eq!(title(&document, id), None);
        let (document, id) = svg_in("<svg><g><title>Deep</title></g></svg>");
        assert_eq!(title(&document, id), None);
        let (document, id) = svg_in("<svg> <title> </title></svg>");
        assert_eq!(title(&document, id), None, "a title of nothing");
    }
}
