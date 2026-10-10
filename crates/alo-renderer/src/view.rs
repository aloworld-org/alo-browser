/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How a page is shown, as its script asks (ADR 0038 § 5, queue item 366):
//! the renderer's [`alo_bindings::View`].
//!
//! `alo-bindings` has no layout, so a page's `innerWidth`, `innerHeight`,
//! `scrollX` and `scrollY` ask this, at every read. It answers from two
//! things only the renderer holds:
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
//!
//! One view is made for each page loaded, and handed to its realm when its
//! first script is about to run ([`crate::held::Held::scripted`]).
//!
//! # Not here yet
//!
//! An element's scrolling area, measured from the layout the page would be
//! drawn with at the moment it is asked, is queue item 370's.

use core::cell::Cell;

use alo_bindings::{Extent, Scrolled, View};
use alo_layout::Size;

/// A page's viewport and scroll position, as its script reads them.
#[derive(Debug)]
pub struct PageView {
    viewport: Cell<Size>,
    scrolled: Scrolled,
}

impl PageView {
    /// A page laid out at `viewport`, scrolled to its top left.
    pub fn at(viewport: Size) -> Self {
        Self {
            viewport: Cell::new(viewport),
            scrolled: Scrolled::default(),
        }
    }

    /// The page is laid out at `viewport` from now on.
    pub fn resized(&self, viewport: Size) {
        self.viewport.set(viewport);
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
}

#[cfg(test)]
mod tests {
    use super::PageView;
    use alo_bindings::{Extent, Scrolled, View};
    use alo_layout::Size;

    #[test]
    fn a_view_answers_the_size_it_was_last_given_and_the_top_left() {
        let view = PageView::at(Size::new(800.0, 600.0));
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
}
