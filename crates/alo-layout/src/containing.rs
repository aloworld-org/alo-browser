/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which box an absolutely positioned box is placed against.
//!
//! CSS 2 § 10.1: the padding box of its **nearest ancestor that is
//! positioned** — `position` other than `static` — or, when there is none,
//! the **initial containing block**: a rectangle the size of the viewport at
//! the top left of the page. CSS Transforms 1 § 2 adds an ancestor with a
//! `transform` other than `none`, which contains every descendant, positioned
//! or not.
//!
//! The answer is rarely the box's parent, and `taffy` only ever places an
//! absolute child against its parent — so this file says which box it is, and
//! `crate::engine` hands the box to that one. alo Sites' skip link is a child
//! of `body` with no positioned ancestor: it belongs at the top of the page,
//! and was placed 48 pixels down whenever `body` was pushed down by a margin
//! it shares with its first child, as the footer section's is (queue item
//! 355).
//!
//! # What does not make a containing block here
//!
//! `filter`, `backdrop-filter`, `perspective`, `will-change` naming one of
//! those, `contain` and `container-type` each make one too. None of them is
//! read by this engine at all yet, and reading one only to make a containing
//! block of it would tell a page's script the property is supported
//! (`alo-css`' `properties.rs`). They wait for the properties themselves.

use crate::keyword::Positioning;
use crate::style;
use alo_box::{BoxId, BoxTree};
use alo_style::StyleTree;

/// What an absolutely positioned box is placed against, relative to where it
/// sits in the box tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Containing {
    /// Its own parent — the one place `taffy` already puts it. Also the
    /// answer for every box that is not absolutely positioned.
    Parent,
    /// An ancestor further up, by its box.
    Ancestor(BoxId),
    /// The initial containing block: no ancestor is positioned.
    Initial,
    /// An ancestor outside the formatting context being laid out: the box is
    /// inside an atomic inline box — an `inline-block`, a button — that is
    /// laid out on its own, and nothing between them is positioned. The
    /// containing block is somewhere this layout cannot see.
    Beyond,
}

/// What `id` is placed against, within the subtree laid out from `root`.
///
/// `document` says whether `root` is the document's own root box, above
/// which is the initial containing block, or an atomic inline box, above
/// which is the rest of the page.
pub(crate) fn of(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    root: BoxId,
    document: bool,
) -> Containing {
    if id == root || !is_absolute(boxes, styles, id) {
        return Containing::Parent;
    }
    let Some(parent) = boxes.get(id).and_then(|node| node.parent) else {
        return Containing::Parent;
    };
    let mut at = parent;
    loop {
        if contains_absolutes(boxes, styles, at) {
            return if at == parent {
                Containing::Parent
            } else {
                Containing::Ancestor(at)
            };
        }
        if at == root {
            return if document {
                Containing::Initial
            } else {
                Containing::Beyond
            };
        }
        let Some(up) = boxes.get(at).and_then(|node| node.parent) else {
            // The subtree's root is always an ancestor of a box inside it, so
            // a box tree this short is one this layout was not asked about.
            return Containing::Parent;
        };
        at = up;
    }
}

/// Whether a box is absolutely positioned.
pub(crate) fn is_absolute(boxes: &BoxTree, styles: &StyleTree, id: BoxId) -> bool {
    position_of(boxes, styles, id) == Positioning::Absolute
}

/// Whether a box is the containing block of the absolutely positioned boxes
/// under it: it is positioned, or it is transformed.
fn contains_absolutes(boxes: &BoxTree, styles: &StyleTree, id: BoxId) -> bool {
    position_of(boxes, styles, id) != Positioning::Static || is_transformed(boxes, styles, id)
}

/// A box's `position`; `static` for a box nobody wrote, which has no style
/// and does not inherit one.
fn position_of(boxes: &BoxTree, styles: &StyleTree, id: BoxId) -> Positioning {
    boxes
        .get(id)
        .and_then(|node| node.kind.node())
        .and_then(|source| styles.get(source))
        // Every issue in this style is recorded when the box itself is laid
        // out; reading it again here is not a second finding.
        .map_or(Positioning::Static, |computed| {
            style::read(computed, &mut Vec::new()).position
        })
}

/// Whether a box has a `transform` other than `none`, read as paint reads it:
/// a value paint cannot read draws nothing, so it contains nothing either.
/// The identity — `scale(1)` — is still a transform, and still contains.
fn is_transformed(boxes: &BoxTree, styles: &StyleTree, id: BoxId) -> bool {
    boxes
        .get(id)
        .and_then(|node| node.kind.node())
        .and_then(|source| styles.get(source))
        .and_then(|computed| computed.get("transform"))
        .and_then(alo_value::parse_transform)
        .is_some_and(|transform| !transform.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_css::{MediaContext, parse_stylesheet};
    use alo_dom::{Document, parse_document};
    use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};

    /// The box tree and styles of a page, and the document they came from.
    fn page(html: &str, css: &str) -> (BoxTree, StyleTree, Document) {
        let document = parse_document(html);
        let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let sheet = parse_stylesheet(css);
        let styles = resolve(
            &document,
            &[
                SourcedSheet::new(Origin::UserAgent, &agent),
                SourcedSheet::new(Origin::Author, &sheet),
            ],
            &MediaContext::default(),
        );
        let boxes = alo_box::build(&document, &styles);
        (boxes, styles, document)
    }

    /// The box the element with this `id` made.
    fn box_of(boxes: &BoxTree, document: &Document, id: &str) -> Option<BoxId> {
        let node = document.descendants(document.root()).find(|candidate| {
            document
                .element(*candidate)
                .is_some_and(|element| element.attr("id") == Some(id))
        })?;
        boxes
            .ids()
            .find(|candidate| boxes.get(*candidate).and_then(|held| held.kind.node()) == Some(node))
    }

    fn containing(html: &str, css: &str, id: &str) -> Option<(Containing, BoxTree, Document)> {
        let (boxes, styles, document) = page(html, css);
        let target = box_of(&boxes, &document, id)?;
        let root = boxes.root()?;
        let answer = of(&boxes, &styles, target, root, true);
        Some((answer, boxes, document))
    }

    #[test]
    fn a_box_in_the_flow_is_its_parents() {
        let (answer, ..) = containing("<div><p id=t>x</p></div>", "", "t").expect("laid out");
        assert_eq!(answer, Containing::Parent);
    }

    #[test]
    fn with_no_positioned_ancestor_it_is_the_initial_containing_block() {
        let (answer, ..) = containing(
            "<div><a id=t>skip</a></div>",
            "#t { position: absolute; top: 0 }",
            "t",
        )
        .expect("laid out");
        assert_eq!(answer, Containing::Initial);
    }

    #[test]
    fn a_positioned_parent_is_the_parent() {
        let (answer, ..) = containing(
            "<div id=p><span id=t>x</span></div>",
            "#p { position: relative } #t { position: absolute }",
            "t",
        )
        .expect("laid out");
        assert_eq!(answer, Containing::Parent);
    }

    #[test]
    fn the_nearest_positioned_ancestor_beyond_the_parent_is_the_one() {
        let (answer, boxes, document) = containing(
            "<div id=far><div id=near><section><span id=t>x</span></section></div></div>",
            "#far, #near { position: relative } #t { position: absolute }",
            "t",
        )
        .expect("laid out");
        assert_eq!(
            answer,
            Containing::Ancestor(box_of(&boxes, &document, "near").expect("a box")),
            "the nearer of the two",
        );
    }

    #[test]
    fn an_absolute_ancestor_contains_too() {
        let (answer, boxes, document) = containing(
            "<div id=a><section><span id=t>x</span></section></div>",
            "#a, #t { position: absolute }",
            "t",
        )
        .expect("laid out");
        assert_eq!(
            answer,
            Containing::Ancestor(box_of(&boxes, &document, "a").expect("a box"))
        );
    }

    #[test]
    fn a_transformed_ancestor_contains_and_none_or_nonsense_does_not() {
        let html = "<div id=a><section><span id=t>x</span></section></div>";
        for (transform, contains) in [
            ("translateY(1.5rem)", true),
            ("scale(1)", true),
            ("none", false),
            ("wobble(3)", false),
        ] {
            let css = format!("#a {{ transform: {transform} }} #t {{ position: absolute }}");
            let (answer, boxes, document) = containing(html, &css, "t").expect("laid out");
            let expected = if contains {
                Containing::Ancestor(box_of(&boxes, &document, "a").expect("a box"))
            } else {
                Containing::Initial
            };
            assert_eq!(answer, expected, "transform: {transform}");
        }
    }

    /// The subtree is named directly: the box tree today breaks an atomic
    /// inline box around an absolutely positioned child (queue item 286), so
    /// no page yet puts one inside an `inline-block` for layout to find.
    #[test]
    fn inside_a_subtree_laid_out_on_its_own_with_nothing_positioned_it_is_beyond() {
        let (boxes, styles, document) = page(
            "<div id=own><section><b id=t>x</b></section></div>",
            "#t { position: absolute }",
        );
        let target = box_of(&boxes, &document, "t").expect("a box");
        let own = box_of(&boxes, &document, "own").expect("a box");
        assert_eq!(of(&boxes, &styles, target, own, false), Containing::Beyond);
        assert_eq!(
            of(&boxes, &styles, target, own, true),
            Containing::Initial,
            "the same box in the document's own layout",
        );
    }

    #[test]
    fn the_root_of_what_is_laid_out_is_its_own_parents() {
        let (boxes, styles, document) = page("<p id=t>x</p>", "#t { position: absolute }");
        let target = box_of(&boxes, &document, "t").expect("a box");
        assert_eq!(
            of(&boxes, &styles, target, target, false),
            Containing::Parent
        );
    }
}
