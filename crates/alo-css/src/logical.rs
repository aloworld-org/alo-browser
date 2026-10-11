/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which physical side a logical box property names, in the one writing
//! mode and the one direction this engine lays out.
//!
//! CSS Logical Properties 1 names a margin or a padding by where it falls
//! in the flow — `margin-inline-start`, `padding-block-end` — rather than
//! by which edge of the screen it is on. Which edge that is depends on the
//! element's `writing-mode` and `direction`, and this engine reads neither:
//! every box is laid out `horizontal-tb` and left to right. In that one
//! flow the block axis runs top to bottom and the inline axis left to
//! right, so block-start **is** the top, inline-start **is** the left, and
//! a logical declaration is mapped to its physical side as it is written
//! ([`DeclarationBlock::push`](crate::DeclarationBlock::push)). That makes
//! the two compete in the cascade as the one property they are, by order of
//! appearance, which is what CSS Logical 1 § 3 says a logical property and
//! its physical counterpart do.
//!
//! # Where this is wrong, and what it waits for
//!
//! The mapping is left to right's, and it is **baked in where it is
//! written**: a page with `dir="rtl"` gets its `margin-inline-start` on the
//! left, where it should be on the right. That is the same answer the rest
//! of this engine gives such a page — `direction` is read nowhere, so its
//! text, its flex rows and its grid columns run left to right as well — and
//! a right-to-left page's half of this is queue item 421. The day
//! `writing-mode` (queue item 98) or `direction` is read, this mapping
//! stops being exact and must move to computed-value time, per element.
//!
//! alo Sites' text-and-image section asked for it: its overlapping text
//! card is `margin-inline-start: -4rem`, and was laid out with no margin at
//! all, beside the picture instead of over it.

/// Each logical longhand this engine maps, and the physical side it is in
/// a left-to-right, top-to-bottom flow.
///
/// Margins and padding only, because they are what a page has asked for.
/// `inset-*`, `border-*-start` and the logical sizes (`inline-size`) have
/// the same shape and are not here until a page writes one.
static SIDES: [(&str, &str); 8] = [
    ("margin-block-start", "margin-top"),
    ("margin-block-end", "margin-bottom"),
    ("margin-inline-start", "margin-left"),
    ("margin-inline-end", "margin-right"),
    ("padding-block-start", "padding-top"),
    ("padding-block-end", "padding-bottom"),
    ("padding-inline-start", "padding-left"),
    ("padding-inline-end", "padding-right"),
];

/// The physical side the logical longhand `name` is laid out as, or
/// [`None`] when `name` is not one this engine maps. `name` is compared as
/// written, so a caller lowercases it first, as a parsed
/// [`PropertyName`](crate::PropertyName) already is.
pub(crate) fn physical(name: &str) -> Option<&'static str> {
    SIDES
        .iter()
        .find(|(logical, _)| *logical == name)
        .map(|(_, physical)| *physical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Declaration, DeclarationBlock, Importance, PropertyName};

    #[test]
    fn the_block_axis_is_top_to_bottom_and_the_inline_axis_left_to_right() {
        assert_eq!(physical("margin-block-start"), Some("margin-top"));
        assert_eq!(physical("padding-block-end"), Some("padding-bottom"));
        assert_eq!(physical("margin-inline-start"), Some("margin-left"));
        assert_eq!(physical("padding-inline-end"), Some("padding-right"));
    }

    #[test]
    fn anything_else_names_no_side() {
        assert_eq!(physical("margin-left"), None, "already physical");
        assert_eq!(physical("margin-inline"), None, "a shorthand");
        assert_eq!(physical("inset-inline-start"), None, "not mapped yet");
        assert_eq!(
            physical("Margin-Inline-Start"),
            None,
            "the caller lowercases"
        );
        assert_eq!(physical("--margin-inline-start"), None);
    }

    fn value(block: &DeclarationBlock, name: &str) -> Option<String> {
        block
            .get(&PropertyName::parse(name))
            .map(|declaration| declaration.value.clone())
    }

    /// Written after a physical side, the logical longhand wins; written
    /// before one, it loses — order decides, as for one property.
    #[test]
    fn a_logical_longhand_and_its_side_compete_by_order() {
        let mut block = DeclarationBlock::new();
        block.push(Declaration::new("margin", "0", Importance::Normal));
        block.push(Declaration::new(
            "margin-inline-start",
            "-4rem",
            Importance::Normal,
        ));
        assert_eq!(value(&block, "margin-left").as_deref(), Some("-4rem"));
        assert_eq!(value(&block, "margin-right").as_deref(), Some("0"));

        block.push(Declaration::new("margin-left", "1px", Importance::Normal));
        assert_eq!(value(&block, "margin-left").as_deref(), Some("1px"));
        assert_eq!(
            block.written().count(),
            3,
            "the side it implies is not written",
        );
    }

    #[test]
    fn a_logical_longhand_keeps_its_importance() {
        let mut block = DeclarationBlock::new();
        block.push(Declaration::new(
            "padding-block-end",
            "2px",
            Importance::Important,
        ));
        assert!(
            block
                .get(&PropertyName::parse("padding-bottom"))
                .is_some_and(|declaration| declaration.importance.is_important())
        );
    }
}
