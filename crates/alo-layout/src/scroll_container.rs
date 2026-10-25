/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which boxes are **scroll containers**: the boxes Chrome scrolls when a
//! page sets their `scrollTop` or `scrollLeft`, and so the only ones an agent
//! can be told it scrolled (queue item 661, ADR 0002).
//!
//! A box whose content spills is not one for spilling. Measured against
//! Chrome 155, a `div` of 10 pixels with `overflow: visible` holding 126
//! pixels of text has `scrollHeight` 126 and `clientHeight` 10, and setting
//! its `scrollTop` to 40 leaves it 0. What makes a scroll container is CSS
//! Overflow 3 § 3:
//!
//! - **`overflow` `hidden`, `scroll` or `auto` on either axis.** One axis is
//!   enough, because the other then computes from `visible` to `auto` and
//!   from `clip` to `hidden`: `overflow-x: hidden` alone scrolls down, as
//!   Chrome does (it computes `overflow-y` as `auto`), and `overflow-x:
//!   hidden; overflow-y: clip` scrolls as `hidden` does. `clip`, on both
//!   axes or beside `visible`, is not one: it cuts the content off and
//!   nothing can scroll to it. `hidden` is one, though no bar is drawn.
//! - **Only a box `overflow` applies to**: a block container, a flex or a
//!   grid container — an `inline-block` and a `<button>` included. An inline
//!   box (`inline flow`, a `<span>`) has no scrolling box at all, whatever
//!   its `overflow`, and Chrome measures it as 0 by 0; nor does a replaced
//!   box, an `<img>` or an `<svg>`, which Chrome does not scroll under
//!   `overflow: auto`.
//! - **A one-line field** — an `<input>` a person types text, a number or a
//!   search into — scrolls along its line in Chrome (`scrollLeft` 40 of a
//!   long value), through an inner box of its own whose `overflow` an author
//!   cannot reach: Chrome computes the field's own as `clip` even under
//!   `overflow: visible`. A date or a time field does not.
//!
//! # The root, and the body
//!
//! The root element's `overflow` is the viewport's, and the viewport is
//! always a scroll container: Chrome scrolls a page through
//! `documentElement.scrollTop` under `overflow: visible`. So the root is one
//! whatever its style, standing for the viewport, and whether it has
//! something to scroll is the viewport's question
//! ([`crate::LayoutTree::scrolls`]). A `<body>` whose `overflow` the
//! viewport takes, when the root's is `visible`, is never one: its own used
//! `overflow` is `visible` ([`crate::viewport`], queue item 662).

use crate::keyword::Overflow;
use crate::style::LayoutStyle;
use alo_box::{BoxId, BoxKind, BoxTree, Inside, Outside, Purpose};
use alo_style::StyleTree;
use std::collections::BTreeSet;

/// The boxes of `boxes` that are scroll containers, the root among them as
/// the viewport's; see the module's own documentation.
pub(crate) fn scroll_containers(boxes: &BoxTree, styles: &StyleTree) -> BTreeSet<BoxId> {
    boxes
        .ids()
        .filter(|&id| {
            Some(id) == boxes.root()
                || (!crate::viewport::takes_the_overflow_of(boxes, styles, id)
                    && is_a_scroll_container(boxes, styles, id))
        })
        .collect()
}

fn is_a_scroll_container(boxes: &BoxTree, styles: &StyleTree, id: BoxId) -> bool {
    let Some(node) = boxes.get(id) else {
        return false;
    };
    let BoxKind::Element { node: element, .. } = node.kind else {
        return false;
    };
    if boxes.natural_size(id).is_some()
        || (node.kind.outside() == Outside::Inline && node.kind.inside() == Inside::Flow)
    {
        return false;
    }
    if holds_a_line_of_text(boxes, id) {
        return true;
    }
    // A value this engine cannot read was recorded when the box's style was
    // read for its own layout; reading it again here is not a second finding.
    styles
        .get(element)
        .is_some_and(|style| scrolls_by_its_style(&crate::style::read(style, &mut Vec::new())))
}

/// Whether `overflow` makes a box a scroll container: `hidden`, `scroll` or
/// `auto` on either axis.
fn scrolls_by_its_style(style: &LayoutStyle) -> bool {
    let scrolls = |overflow: Overflow| {
        matches!(
            overflow,
            Overflow::Hidden | Overflow::Scroll | Overflow::Auto
        )
    };
    scrolls(style.overflow.horizontal) || scrolls(style.overflow.vertical)
}

/// Whether the box is a one-line field whose line is text rather than
/// segments: it holds the inside a field's line is typed into.
fn holds_a_line_of_text(boxes: &BoxTree, id: BoxId) -> bool {
    boxes.children(id).any(|child| {
        boxes.get(child).is_some_and(|held| {
            matches!(
                held.kind,
                BoxKind::Anonymous {
                    purpose: Purpose::Control {
                        one_line: true,
                        in_segments: false,
                        ..
                    },
                    ..
                }
            )
        })
    })
}

#[cfg(test)]
mod tests {
    use super::scroll_containers;
    use alo_box::{BoxId, BoxTree, build};
    use alo_css::{MediaContext, parse_stylesheet};
    use alo_dom::{Document, parse_document};
    use alo_style::{Origin, SourcedSheet, StyleTree, USER_AGENT_STYLE_SHEET, resolve};

    fn read(page: &str) -> (Document, StyleTree, BoxTree) {
        let document = parse_document(page);
        let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let styles = resolve(
            &document,
            &[SourcedSheet::new(Origin::UserAgent, &agent)],
            &MediaContext::default(),
        );
        let boxes = build(&document, &styles);
        (document, styles, boxes)
    }

    /// Whether the box of the element whose `id` is `id` on `page` is a
    /// scroll container, or [`None`] when it has no box.
    fn is_one(page: &str, id: &str) -> Option<bool> {
        let (document, styles, boxes) = read(page);
        let element = document
            .descendants(document.root())
            .find(|&each| document.attribute_by_name(each, "id") == Some(id))?;
        let target: BoxId = boxes
            .ids()
            .find(|&each| boxes.get(each).and_then(|held| held.kind.node()) == Some(element))?;
        Some(scroll_containers(&boxes, &styles).contains(&target))
    }

    fn div(style: &str) -> String {
        format!("<!doctype html><body><div id=t style='{style}'>a<br>b</div>")
    }

    #[test]
    fn hidden_scroll_or_auto_on_either_axis_makes_one() {
        // Chrome 155 scrolls each of these by 40 and computes the other axis
        // as `auto` or `hidden`.
        for style in [
            "overflow: hidden",
            "overflow: scroll",
            "overflow: auto",
            "overflow-x: hidden",
            "overflow-y: auto",
            "overflow-x: hidden; overflow-y: clip",
            "overflow-x: clip; overflow-y: scroll",
        ] {
            assert_eq!(is_one(&div(style), "t"), Some(true), "{style}");
        }
    }

    #[test]
    fn visible_and_clip_make_none() {
        // Chrome 155 leaves `scrollTop` at 0 for each.
        for style in [
            "",
            "overflow: visible",
            "overflow: clip",
            "overflow-x: clip",
            "overflow-x: clip; overflow-y: visible",
        ] {
            assert_eq!(is_one(&div(style), "t"), Some(false), "{style}");
        }
    }

    #[test]
    fn a_container_of_any_layout_is_one_and_an_inline_box_is_not() {
        for display in ["inline-block", "flex", "grid", "flow-root", "inline-flex"] {
            let page = div(&format!("display: {display}; overflow: auto"));
            assert_eq!(is_one(&page, "t"), Some(true), "{display}");
        }
        let span = "<!doctype html><body>x <span id=t style='overflow:auto'>a b</span>";
        assert_eq!(is_one(span, "t"), Some(false), "an inline box has none");
        let button = "<!doctype html><body><button id=t style='overflow:auto'>a</button>";
        assert_eq!(is_one(button, "t"), Some(true));
        let plain = "<!doctype html><body><button id=t style='height:8px'>a</button>";
        assert_eq!(is_one(plain, "t"), Some(false), "a button's own is visible");
    }

    #[test]
    fn a_replaced_box_is_not_one() {
        let page = "<!doctype html><body>\
             <svg id=t width=10 height=10 style='overflow:auto;display:block'></svg>";
        assert_eq!(is_one(page, "t"), Some(false), "an svg's own is hidden");
    }

    #[test]
    fn a_field_of_text_is_one_whatever_its_style_and_a_date_field_is_not() {
        for kind in ["text", "search", "password", "number", "email"] {
            let page =
                format!("<!doctype html><body><input id=t type={kind} style='overflow:visible'>");
            assert_eq!(is_one(&page, "t"), Some(true), "{kind}");
        }
        let date = "<!doctype html><body><input id=t type=date>";
        assert_eq!(is_one(date, "t"), Some(false));
        let checkbox = "<!doctype html><body><input id=t type=checkbox>";
        assert_eq!(is_one(checkbox, "t"), Some(false));
    }

    #[test]
    fn a_body_whose_overflow_the_viewport_takes_is_not_one() {
        // Chrome 155 leaves `body.scrollTop` at 0 under a root of `visible`,
        // and scrolls the body under a root of `auto`.
        let page = |root: &str| {
            format!(
                "<!doctype html><html style='{root}'>\
                 <body id=t style='overflow:auto;height:50px'>a"
            )
        };
        assert_eq!(is_one(&page(""), "t"), Some(false));
        assert_eq!(is_one(&page("overflow: auto"), "t"), Some(true));
        assert_eq!(is_one(&page("overflow: clip"), "t"), Some(true));
    }

    #[test]
    fn the_root_is_one_as_the_viewports_whatever_its_style() {
        // Chrome 155 scrolls a long page by `documentElement.scrollTop`.
        for style in ["", "overflow: visible", "overflow: auto", "overflow: clip"] {
            let page = format!("<!doctype html><html id=t style='{style}'><body>");
            assert_eq!(is_one(&page, "t"), Some(true), "{style}");
        }
    }
}
