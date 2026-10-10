/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What an element's `scrollWidth` and `scrollHeight` measure, in a drawing
//! (ADR 0038 § 4, queue item 370), before they are rounded.
//!
//! CSSOM View's steps, without its quirks branch, since no document is in
//! quirks mode here (law 1):
//!
//! - **The root element** measures the larger of the viewport's scrolling
//!   area and the viewport. The viewport's scrolling area starts at the top
//!   left of the page, the initial containing block's corner, and reaches as
//!   far right and down as the root's border box and what its content
//!   reaches. CSSOM View asks this before it asks whether the element has a
//!   box, so a root under `display: none` measures the viewport.
//! - **An element with no box** measures nothing: [`None`], which the page
//!   reads as `0`.
//! - **Any other element**, `body` included, measures its own scrolling
//!   area ([`alo_layout::BoxGeometry::scrolling_area`]): its padding box,
//!   extended toward its end edges only. An element laid out in pieces — an
//!   inline split around a block — measures its first piece.
//!
//! Overflow above or to the left of the page is in no area: alo Sites' skip
//! link at `left: -999rem` must not make its page sixteen thousand pixels
//! wide.

use alo_dom::{Document, NodeId};
use alo_layout::{BoxGeometry, Size};

use crate::pipeline::Drawing;

/// What `node`'s `scrollWidth` and `scrollHeight` measure in `drawing`, a
/// drawing of `document`: [`None`] when it is not the root element and has
/// no box.
pub fn of(drawing: &Drawing, document: &Document, node: NodeId) -> Option<Size> {
    let geometry = geometry_of(drawing, node);
    if document.document_element() == Some(node) {
        return Some(of_the_root(drawing.layout.viewport(), geometry));
    }
    Some(geometry?.scrolling_area())
}

/// The first box `node` generated that was laid out.
fn geometry_of(drawing: &Drawing, node: NodeId) -> Option<BoxGeometry> {
    let boxes = &drawing.boxes;
    boxes
        .ids()
        .filter(|id| boxes.get(*id).and_then(|held| held.kind.node()) == Some(node))
        .find_map(|id| drawing.layout.get(id))
}

/// The root element's measure: the viewport's scrolling area or the
/// viewport, whichever is larger, each way.
fn of_the_root(viewport: Size, root: Option<BoxGeometry>) -> Size {
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

#[cfg(test)]
mod tests {
    use super::of_the_root;
    use alo_layout::{BoxGeometry, Edges, Rect, Size};

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
        let measured = of_the_root(
            Size::new(800.0, 600.0),
            Some(root(253.2, Size::new(800.0, 253.2))),
        );
        assert_eq!(measured, Size::new(800.0, 600.0));
    }

    #[test]
    fn a_root_taller_or_wider_than_the_viewport_measures_what_it_reaches() {
        let measured = of_the_root(
            Size::new(800.0, 600.0),
            Some(root(1500.0, Size::new(1200.0, 1600.0))),
        );
        assert_eq!(measured, Size::new(1200.0, 1600.0));
    }

    #[test]
    fn a_root_with_no_box_measures_the_viewport() {
        assert_eq!(
            of_the_root(Size::new(640.0, 480.0), None),
            Size::new(640.0, 480.0)
        );
    }
}
