/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What an `<img>` is, given whether its picture is shown (ADR 0046).
//!
//! An `<img>` is three different boxes depending on one fact this crate
//! cannot know: whether its bytes became a picture. A picture is a replaced
//! box of the picture's size; with none, a non-empty `alt` is a run of text in
//! the element's own style; and with neither it is a replaced box of nothing.
//!
//! So the fact is decided **once per drawing, by whoever read the pictures**,
//! before the tree is built, and handed in as a [`Shown`]. Deciding it here by
//! looking at `src` would draw a corrupt picture's empty box rather than its
//! text, and deciding it after the tree is built would leave layout, paint and
//! the agent each knowing a different thing (§ 1).

use crate::natural::NaturalSize;
use alo_dom::{Element, NodeId};
use std::collections::BTreeMap;

/// The `<img>` elements whose picture is shown, each with its natural size.
///
/// Every `<img>` not in it has no picture — no `src`, one the drawing was not
/// handed, bytes that are no picture. [`Shown::default`] is a drawing with no
/// pictures at all, which is every `<img>` broken.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shown {
    sizes: BTreeMap<NodeId, NaturalSize>,
}

impl Shown {
    /// Say that this `<img>` shows a picture of this size.
    ///
    /// Saying it twice is the later one winning, as a replaced picture does.
    pub fn show(&mut self, img: NodeId, size: NaturalSize) {
        self.sizes.insert(img, size);
    }

    /// The natural size of the picture this `<img>` shows, or [`None`] when it
    /// shows none.
    pub fn size_of(&self, img: NodeId) -> Option<NaturalSize> {
        self.sizes.get(&img).copied()
    }
}

/// What box an `<img>` makes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Img {
    /// It shows a picture: a replaced box of the picture's own size.
    Picture(NaturalSize),
    /// It shows none and represents text: a non-replaced box holding this,
    /// its `alt` as written (§ 2).
    Text(String),
    /// It shows none and represents nothing: a replaced box whose natural
    /// width and height are 0 and which has no ratio (§ 3).
    Nothing,
}

impl Img {
    /// What the `<img>` `id` is, or [`None`] for any other element.
    ///
    /// An `alt` represents text when it is present and, untrimmed, not empty:
    /// HTML's `alt=""` is the author saying the picture is decoration, and
    /// `alt=" "` is a space somebody wrote.
    pub(crate) fn of(element: &Element, id: NodeId, shown: &Shown) -> Option<Self> {
        if !element.name.is_html("img") {
            return None;
        }
        if let Some(size) = shown.size_of(id) {
            return Some(Self::Picture(size));
        }
        Some(match element.attr("alt") {
            Some(alt) if !alt.is_empty() => Self::Text(alt.to_owned()),
            _ => Self::Nothing,
        })
    }

    /// Whether the picture failed, which an agent is told (§ 5).
    pub(crate) fn is_broken(&self) -> bool {
        !matches!(self, Self::Picture(_))
    }
}

/// What a replaced box of nothing claims about its size: 0 by 0, no ratio.
///
/// Never CSS's default object size, 300 × 150, which is for content with no
/// dimensions at all rather than content whose dimensions are nothing.
pub(crate) fn nothing() -> NaturalSize {
    NaturalSize::sized(0.0, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::parse_document;

    fn img_of(html: &str, shown: impl Fn(NodeId) -> Shown) -> Img {
        let document = parse_document(html);
        let id = document
            .descendants(document.root())
            .find(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| element.name.is_html("img"))
            })
            .expect("an <img>");
        let element = document.element(id).expect("an element");
        Img::of(element, id, &shown(id)).expect("an <img> is an Img")
    }

    fn none(_: NodeId) -> Shown {
        Shown::default()
    }

    #[test]
    fn a_shown_picture_is_its_size_whatever_its_alt() {
        let shown = |id| {
            let mut shown = Shown::default();
            shown.show(id, NaturalSize::sized(8.0, 4.0));
            shown
        };
        let img = img_of("<img src=a.png alt='A chart'>", shown);
        assert_eq!(img, Img::Picture(NaturalSize::sized(8.0, 4.0)));
        assert!(!img.is_broken());
    }

    #[test]
    fn with_no_picture_an_alt_is_its_text_as_written() {
        assert_eq!(
            img_of("<img src=a.png alt='  Orange   notebook '>", none),
            Img::Text("  Orange   notebook ".to_owned()),
        );
        assert_eq!(
            img_of("<img alt=' '>", none),
            Img::Text(" ".to_owned()),
            "a space somebody wrote is not an empty alt",
        );
    }

    #[test]
    fn with_no_picture_and_no_or_an_empty_alt_it_is_nothing() {
        for html in ["<img src=a.png>", "<img src=a.png alt=''>", "<img>"] {
            let img = img_of(html, none);
            assert_eq!(img, Img::Nothing, "{html}");
            assert!(img.is_broken(), "{html}");
        }
    }

    #[test]
    fn nothing_is_zero_by_zero_with_no_ratio() {
        assert_eq!(nothing().width, Some(0.0));
        assert_eq!(nothing().height, Some(0.0));
        assert_eq!(nothing().ratio(), None);
    }

    #[test]
    fn only_an_img_is_an_img() {
        let document = parse_document("<div alt=x></div><span alt=y></span>");
        let shown = Shown::default();
        let made: Vec<Img> = document
            .descendants(document.root())
            .filter_map(|id| Img::of(document.element(id)?, id, &shown))
            .collect();
        assert!(made.is_empty(), "{made:?}");
    }
}
