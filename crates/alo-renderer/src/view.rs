/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How a page is shown, as its script asks (ADR 0038 § 5, queue items 366
//! and 370): the renderer's [`alo_bindings::View`].
//!
//! `alo-bindings` has no layout, so a page's `innerWidth`, `innerHeight`,
//! `scrollX` and `scrollY` ask this, at every read, and so do its elements'
//! `scrollWidth` and `scrollHeight`. It answers from what only the renderer
//! holds:
//!
//! - **the viewport**, the size every drawing of the page is laid out at —
//!   [`crate::Page`]'s, kept here too so that a read in the middle of a
//!   script needs no borrow of the renderer. The renderer sets it in the two
//!   places a page's size is ever set: when the page is loaded and when it is
//!   resized ([`PageView::resized`]), so a read after a `Resize` answers the
//!   new size (§ 2);
//! - **the scroll position**, held here and nowhere else (§ 3). It is the
//!   top left, `0` and `0`, because nothing scrolls a viewport yet: no wheel
//!   in the window, no `scrollTo`, and an agent's `Scroll` changes nothing
//!   in the frame. **Whatever first scrolls one moves this position**, and is
//!   not built without it, so that the frame and the page never disagree
//!   about where the page is.
//! - **the page's easel** ([`crate::easel`]): what it is drawn with and what
//!   it was last drawn as, shared with the renderer. An element's scrolling
//!   area is measured on it **now**, of the document as the script has left
//!   it so far: the last drawing if nothing has changed since, and otherwise
//!   a drawing made then, the one pipeline every drawing is made by, and kept
//!   as the next one (§ 4). What it measures is [`crate::scrolling_area`]'s.
//!
//! One view is made for each page loaded, and handed to its realm when its
//! first script is about to run ([`crate::held::Held::scripted`]).

use core::cell::{Cell, RefCell};
use std::rc::Rc;

use alo_bindings::{Extent, Scrolled, Unmeasured, View};
use alo_dom::{Document, NodeId};
use alo_layout::Size;

use crate::easel::Easel;
use crate::scrolling_area;

/// A page's viewport and scroll position, as its script reads them, and the
/// easel its elements are measured on.
#[derive(Debug)]
pub struct PageView {
    viewport: Cell<Size>,
    scrolled: Scrolled,
    easel: Rc<RefCell<Easel>>,
}

impl PageView {
    /// A page laid out at `viewport`, scrolled to its top left, drawn on
    /// `easel`.
    pub fn at(viewport: Size, easel: Rc<RefCell<Easel>>) -> Self {
        Self {
            viewport: Cell::new(viewport),
            scrolled: Scrolled::default(),
            easel,
        }
    }

    /// The page is laid out at `viewport` from now on.
    pub fn resized(&self, viewport: Size) {
        self.viewport.set(viewport);
    }

    /// The easel the page is drawn on, for the renderer to tell it what the
    /// page is governed by as it loads.
    pub(crate) fn easel(&self) -> &RefCell<Easel> {
        &self.easel
    }
}

impl View for PageView {
    fn viewport(&self) -> Extent {
        let Size { width, height } = self.viewport.get();
        Extent {
            width: f64::from(width),
            height: f64::from(height),
        }
    }

    fn scrolled(&self) -> Scrolled {
        self.scrolled
    }

    fn scrolling_area(
        &self,
        document: &Document,
        node: NodeId,
    ) -> Result<Option<Extent>, Unmeasured> {
        let mut easel = self.easel.try_borrow_mut().map_err(|_| Unmeasured)?;
        easel.fresh(document, self.viewport.get());
        let Some(drawing) = easel.drawn() else {
            return Ok(None);
        };
        Ok(
            scrolling_area::of(drawing, document, node).map(|Size { width, height }| Extent {
                width: f64::from(width),
                height: f64::from(height),
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::PageView;
    use crate::easel::Easel;
    use alo_bindings::{Extent, Scrolled, Unmeasured, View};
    use alo_layout::Size;
    use alo_text::FontDatabase;
    use core::cell::RefCell;
    use std::rc::Rc;

    fn easel() -> Rc<RefCell<Easel>> {
        Rc::new(RefCell::new(Easel::new(FontDatabase::new())))
    }

    #[test]
    fn a_view_answers_the_size_it_was_last_given_and_the_top_left() {
        let view = PageView::at(Size::new(800.0, 600.0), easel());
        assert_eq!(
            view.viewport(),
            Extent {
                width: 800.0,
                height: 600.0
            }
        );
        assert_eq!(view.scrolled(), Scrolled { x: 0.0, y: 0.0 });
        view.resized(Size::new(640.0, 480.0));
        assert_eq!(
            view.viewport(),
            Extent {
                width: 640.0,
                height: 480.0
            }
        );
        assert_eq!(view.scrolled(), Scrolled { x: 0.0, y: 0.0 });
    }

    #[test]
    fn a_measurement_is_drawn_once_and_kept_until_the_document_changes() {
        let easel = easel();
        let view = PageView::at(Size::new(800.0, 600.0), Rc::clone(&easel));
        let mut document = alo_dom::parse_document(
            "<!DOCTYPE html><html><body style='margin:0'><div style='height:900px'></div></body></html>",
        );
        let Some(root) = document.document_element() else {
            panic!("a document element");
        };
        let tall = Extent {
            width: 800.0,
            height: 900.0,
        };
        assert_eq!(view.scrolling_area(&document, root), Ok(Some(tall)));
        assert_eq!(view.scrolling_area(&document, root), Ok(Some(tall)));
        assert_eq!(
            easel.borrow().draws(),
            1,
            "the second read is the first drawing's"
        );
        let Ok(div) = document.create_element("div") else {
            panic!("an element");
        };
        // Made and in no tree: no box, and the document drawn is unchanged.
        assert_eq!(view.scrolling_area(&document, div), Ok(None), "no box");
        assert_eq!(easel.borrow().draws(), 1);
        // Put in the page, it is a change, and the page is drawn again.
        let body = document.descendants(document.root()).find(|id| {
            document
                .element(*id)
                .is_some_and(|e| &*e.name.local == "body")
        });
        let Some(body) = body else {
            panic!("a body");
        };
        assert!(document.append_child(body, div).is_ok());
        assert_eq!(
            view.scrolling_area(&document, div),
            Ok(Some(Extent {
                width: 800.0,
                height: 0.0
            }))
        );
        assert_eq!(
            easel.borrow().draws(),
            2,
            "a node put in the page is a change"
        );
        // In use by somebody else: refused, not waited on and not guessed.
        let held = easel.borrow_mut();
        assert_eq!(view.scrolling_area(&document, root), Err(Unmeasured));
        drop(held);
    }
}
