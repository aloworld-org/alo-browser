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
//!   area and the viewport ([`alo_layout::viewport::scrolling_area`], which
//!   the agent's answer to whether the page scrolls asks too). The
//!   viewport's scrolling area starts at the top left of the page, the
//!   initial containing block's corner, and reaches as far right and down as
//!   the root's border box and what its content reaches. CSSOM View asks
//!   this before it asks whether the element has a box, so a root under
//!   `display: none` measures the viewport.
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
        return Some(alo_layout::viewport::scrolling_area(
            drawing.layout.viewport(),
            geometry,
        ));
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
