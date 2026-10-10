/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a page may ask about how it is shown (ADR 0038 § 5, queue item 366).
//!
//! This crate has no layout and depends on no crate that has one, so it
//! cannot know how big a page's viewport is or where it is scrolled to. It
//! **asks**, through one trait the embedder implements — the renderer, over
//! the page it holds — exactly as `alo-js` asks a [`alo_js::Clock`] the time
//! (ADR 0036 § 1). It knows the questions, never how they are answered, so
//! the only code that can measure a page for a script is the code that draws
//! it.
//!
//! [`show`] hands a page its view. The view is kept beside the page's
//! [`Window`], in Rust state the collector does not walk, since it holds
//! nothing in the heap. It is asked **at every read**, never copied into the
//! heap, so a read after a `Resize` answers the new size (§ 2).
//!
//! A page shown nothing — only an embedder that is not a renderer, such as a
//! test of this crate alone, makes one — refuses each question by name
//! rather than make up a size (§ 5, ADR 0013 § 3).
//!
//! # What is not asked yet
//!
//! An element's scrolling area, the third question § 5 names, is queue item
//! 370's, and arrives with `scrollWidth` and `scrollHeight`.

use core::fmt;

use std::rc::Rc;

use alo_js::interpret::Engine;
use alo_js::{Escape, Fault};

use crate::window::Window;

/// A width and a height, in CSS pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extent {
    /// Across.
    pub width: f64,
    /// Down.
    pub height: f64,
}

/// How far a viewport is scrolled, in CSS pixels: rightward and downward
/// from the top left of the page.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Scrolled {
    /// Rightward.
    pub x: f64,
    /// Downward.
    pub y: f64,
}

/// How a page is shown: what its window's `innerWidth`, `innerHeight`,
/// `scrollX` and `scrollY` read.
pub trait View: fmt::Debug {
    /// The size the page is laid out at: its viewport (ADR 0038 § 2).
    fn viewport(&self) -> Extent;

    /// Where the viewport is scrolled to (ADR 0038 § 3).
    fn scrolled(&self) -> Scrolled;
}

/// Show the page `engine` holds by `view`: what its window's viewport
/// members ask from here on.
///
/// Called by the embedder once, after [`crate::install`] and before any of
/// the page's script runs.
///
/// # Errors
///
/// A `TypeError` when the page was already shown — an embedder showing it
/// twice, where the first view stands — and a fault when the realm's global
/// object is not a [`Window`], which [`crate::install`] would have refused.
pub fn show(engine: &mut Engine, view: Rc<dyn View>) -> Result<(), Escape> {
    let global = engine.global()?;
    let shown = engine
        .objects()
        .write_embedded::<Window, _>(global, |window, _| window.show(view))
        .ok_or(Escape::fault(Fault::NotAnObject))?;
    if shown {
        Ok(())
    } else {
        Err(Escape::type_error("this page already has a view", 0))
    }
}
