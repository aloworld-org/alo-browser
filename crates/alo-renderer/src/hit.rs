/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which element a person's pointer is over (ADR 0024 § 5, queue item 298).
//!
//! The renderer is the only process holding the boxes, so it is the one that
//! answers. A point arrives in the frame's CSS pixels, which are the page's
//! while nothing scrolls a viewport ([`crate::view`]); the box drawn last
//! over it is `alo-paint`'s [`alo_paint::box_at`], read from the display list
//! the frame was painted from, so a pointer finds what a person sees.
//!
//! What a pointer event is aimed at is an **element**, never a box, and these
//! are measured in Chrome 154 with `elementFromPoint`, not read from its
//! source:
//!
//! - **A box an element made** is that element.
//! - **A run of text** is the element it is in, in the document: a
//!   `display: contents` `<span>` has no box and is still what its own text
//!   finds. A field's own text, and its placeholder, are the field.
//!   A list item's marker is the item wherever it is drawn, which Chrome
//!   finds over its bullet only (queue item 541).
//! - **A box nobody wrote** is the element it is inside.
//! - **A point over no box** inside the viewport is the root element, which
//!   Chrome finds below a page as short as its content and to the right of a
//!   body narrower than the window.
//! - **A point outside the viewport** is over nothing.

use crate::pipeline::Drawing;
use alo_box::{BoxId, BoxKind};
use alo_dom::{Document, NodeId};
use alo_paint::Point;

/// The element a person's pointer at `at`, in the frame's CSS pixels, is
/// over in `drawing`, a drawing of `document`, or [`None`] when `at` is
/// outside the viewport or the document has no element at all.
pub fn element_at(drawing: &Drawing, document: &Document, at: Point) -> Option<NodeId> {
    let viewport = drawing.layout.viewport();
    let inside = (0.0..viewport.width).contains(&at.x) && (0.0..viewport.height).contains(&at.y);
    if !inside {
        return None;
    }
    alo_paint::box_at(&drawing.display, at)
        .and_then(|found| element_of(drawing, document, found))
        .or_else(|| document.document_element())
}

/// The element a box is aimed at: its own, the one its text is in, or the one
/// a box nobody wrote is inside.
fn element_of(drawing: &Drawing, document: &Document, found: BoxId) -> Option<NodeId> {
    let mut next = Some(found);
    while let Some(id) = next {
        let node = drawing.boxes.get(id)?;
        match &node.kind {
            BoxKind::Element { node, .. } => return Some(*node),
            BoxKind::Text { node, .. } => return element_holding(document, *node),
            BoxKind::Anonymous { .. } => next = node.parent,
        }
    }
    None
}

/// `node` itself when it is an element, or else the nearest element it is in.
fn element_holding(document: &Document, node: NodeId) -> Option<NodeId> {
    let mut next = Some(node);
    while let Some(id) = next {
        if document.element(id).is_some() {
            return Some(id);
        }
        next = document.parent(id);
    }
    None
}
