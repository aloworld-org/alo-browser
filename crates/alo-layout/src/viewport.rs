/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The **viewport**: whose `overflow` it takes, and what it has to scroll
//! (queue item 662, CSS Overflow 3 § 3.3, ADR 0038 § 4).
//!
//! # Whose `overflow` it takes
//!
//! The root element's `overflow` is the viewport's, not the root's own. When
//! the root's is `visible` on both axes, the viewport takes the body
//! element's instead. Either way the element it was taken from has a used
//! `overflow` of `visible`: it neither clips nor scrolls, and lays out as a
//! box of `visible` does. Measured against Chrome 155, on a page of 500 in a
//! viewport of 213:
//!
//! - a `<body style="overflow:auto;height:50px">` under a root of `visible`
//!   is not scrolled (`body.scrollTop` stays 0 when set to 40; the page is
//!   scrolled by 40 through `documentElement.scrollTop` instead, and its
//!   `scrollHeight` is 500), and its content is drawn past its 50 pixels;
//!   the same holds of `hidden`, `scroll`, `clip`, `overflow-x: clip` and
//!   `overflow-y: clip`. A first child's top margin of 30 collapses through
//!   such a body of `hidden` (the body's top is 30), as it would through one
//!   of `visible`, where under a root of `auto` it does not (the body's top
//!   is 0);
//! - under a root of `auto`, that body scrolls itself (40) and clips at its
//!   50 pixels, and the page's `scrollHeight` is 50;
//! - a root of `overflow: hidden; height: 50px` does not clip its own content
//!   at 50: what is past it is drawn, and the page scrolls by 40.
//!
//! [`takes_the_overflow_of`] answers which boxes these are: the root's, and
//! the body's when the root's `overflow` is `visible` and the body's box is
//! the root's child.
//!
//! # What it has to scroll
//!
//! The viewport is always a scroll container: Chrome 155 scrolls a long page
//! through `documentElement.scrollTop` whatever the root's `overflow` is —
//! `visible`, `hidden`, `auto` and `clip`, and `overflow-x: clip` or
//! `overflow-y: clip` beside `visible` alike. It has something to scroll when
//! its scrolling area ([`scrolling_area`]) passes the viewport by **half a
//! device pixel**, as a box's passes its padding box: a page of 213.5 in a
//! viewport of 213 is scrolled by one and a page of 213.4 is not, at a scale
//! of one; at two, 213.25 is scrolled by a half and 213.24 is not.

use crate::geometry::Size;
use crate::keyword::Overflow;
use crate::tree::BoxGeometry;
use alo_box::{BoxId, BoxTree};
use alo_style::StyleTree;

/// Whether `id`'s `overflow` is taken by the viewport, so that its own used
/// `overflow` is `visible`: it is the root's box, or the body's while the
/// root's `overflow` is `visible` on both axes and the body's box is the
/// root's child. See the module's own documentation.
pub(crate) fn takes_the_overflow_of(boxes: &BoxTree, styles: &StyleTree, id: BoxId) -> bool {
    let Some(root) = boxes.root() else {
        return false;
    };
    if id == root {
        return true;
    }
    if boxes.body() != Some(id) || boxes.get(id).and_then(|body| body.parent) != Some(root) {
        return false;
    }
    // The root's issues, if it has any, were recorded when it was laid out.
    boxes
        .get(root)
        .and_then(|held| held.kind.node())
        .and_then(|element| styles.get(element))
        .map(|style| crate::style::read(style, &mut Vec::new()).overflow)
        .is_none_or(|overflow| {
            overflow.horizontal == Overflow::Visible && overflow.vertical == Overflow::Visible
        })
}

/// The viewport's **scrolling area**, or the viewport itself where that is
/// larger, each way: what the root element's `scrollWidth` and
/// `scrollHeight` measure (ADR 0038 § 4). The area starts at the page's top
/// left, the initial containing block's corner, and reaches as far right and
/// down as the root's border box and what its content reaches; a root with
/// no box (`root` [`None`]) has nothing past the viewport.
pub fn scrolling_area(viewport: Size, root: Option<BoxGeometry>) -> Size {
    let Some(root) = root else {
        return viewport;
    };
    let padding = root.padding_box();
    let right = root
        .border_box
        .right()
        .max(padding.origin.x + root.reach.width);
    let bottom = root
        .border_box
        .bottom()
        .max(padding.origin.y + root.reach.height);
    Size::new(viewport.width.max(right), viewport.height.max(bottom))
}

/// Whether the viewport has something to scroll at a screen's `scale`: its
/// scrolling area passes it by half a device pixel either way.
pub(crate) fn overflows(viewport: Size, root: Option<BoxGeometry>, scale: f32) -> bool {
    let area = scrolling_area(viewport, root);
    let half = 0.5 / alo_value::device_scale(scale);
    area.width - viewport.width >= half || area.height - viewport.height >= half
}

#[cfg(test)]
mod tests {
    use super::{overflows, scrolling_area, takes_the_overflow_of};
    use crate::geometry::{Edges, Rect, Size};
    use crate::tree::BoxGeometry;
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

    /// Whether the box of the element named `name` on `page` gives its
    /// `overflow` to the viewport, or [`None`] when it has no box.
    fn gives(page: &str, name: &str) -> Option<bool> {
        let (document, styles, boxes) = read(page);
        let element = document.descendants(document.root()).find(|&each| {
            document
                .element(each)
                .is_some_and(|element| element.name.is_html(name))
        })?;
        let target: BoxId = boxes
            .ids()
            .find(|&each| boxes.get(each).and_then(|held| held.kind.node()) == Some(element))?;
        Some(takes_the_overflow_of(&boxes, &styles, target))
    }

    fn page(root: &str, body: &str) -> String {
        format!("<!doctype html><html style='{root}'><body style='{body}'><div>a</div>")
    }

    #[test]
    fn the_root_gives_its_overflow_whatever_it_is() {
        for style in ["", "overflow: hidden", "overflow: auto", "overflow: clip"] {
            assert_eq!(gives(&page(style, ""), "html"), Some(true), "{style}");
        }
    }

    #[test]
    fn the_body_gives_its_overflow_only_under_a_root_of_visible() {
        // Chrome 155 leaves the body's `scrollTop` at 0 for each of these.
        for body in [
            "overflow: auto",
            "overflow: hidden",
            "overflow: scroll",
            "overflow: clip",
            "overflow-x: clip",
        ] {
            assert_eq!(gives(&page("", body), "body"), Some(true), "{body}");
            assert_eq!(
                gives(&page("overflow: visible", body), "body"),
                Some(true),
                "{body}"
            );
        }
        // And scrolls the body under a root that is not `visible`.
        for root in ["overflow: auto", "overflow: clip", "overflow-x: hidden"] {
            assert_eq!(
                gives(&page(root, "overflow: auto"), "body"),
                Some(false),
                "{root}"
            );
        }
    }

    #[test]
    fn nothing_else_gives_its_overflow() {
        assert_eq!(gives(&page("", "overflow: auto"), "div"), Some(false));
        assert_eq!(
            gives(&page("overflow: auto", "overflow: auto"), "div"),
            Some(false)
        );
    }

    fn root(height: f32, reach: Size) -> BoxGeometry {
        BoxGeometry {
            border_box: Rect::new(0.0, 0.0, 800.0, height),
            border: Edges::all(0.0),
            padding: Edges::all(0.0),
            reach,
            ..BoxGeometry::default()
        }
    }

    #[test]
    fn a_root_shorter_than_the_viewport_measures_the_viewport() {
        let measured = scrolling_area(
            Size::new(800.0, 600.0),
            Some(root(253.2, Size::new(800.0, 253.2))),
        );
        assert_eq!(measured, Size::new(800.0, 600.0));
    }

    #[test]
    fn a_root_taller_or_wider_than_the_viewport_measures_what_it_reaches() {
        let measured = scrolling_area(
            Size::new(800.0, 600.0),
            Some(root(1500.0, Size::new(1200.0, 1600.0))),
        );
        assert_eq!(measured, Size::new(1200.0, 1600.0));
    }

    #[test]
    fn a_root_with_no_box_measures_the_viewport() {
        assert_eq!(
            scrolling_area(Size::new(640.0, 480.0), None),
            Size::new(640.0, 480.0)
        );
    }

    #[test]
    fn the_viewport_overflows_by_half_a_device_pixel() {
        // Measured against Chrome 155 in a viewport of 213.
        let viewport = Size::new(800.0, 213.0);
        let tall = |height: f32| Some(root(height, Size::new(800.0, height)));
        assert!(!overflows(viewport, tall(213.4), 1.0));
        assert!(overflows(viewport, tall(213.5), 1.0));
        assert!(!overflows(viewport, tall(213.24), 2.0));
        assert!(overflows(viewport, tall(213.25), 2.0));
        assert!(
            !overflows(viewport, None, 1.0),
            "no root, nothing to scroll"
        );
        let wide = Some(root(10.0, Size::new(800.5, 10.0)));
        assert!(overflows(viewport, wide, 1.0), "across as well as down");
    }
}
