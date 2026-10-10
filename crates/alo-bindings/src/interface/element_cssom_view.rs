/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! CSSOM View's `partial interface Element`, its scrolling area only (ADR
//! 0038 § 4, queue item 370): `scrollWidth` and `scrollHeight`, each a
//! `readonly attribute long` on `Element.prototype`.
//!
//! Each asks the page's [`View`] **now**, in the middle of the script, of
//! the document as it is at that moment: the view measures it from the
//! layout the page would be drawn with if it were drawn now, and keeps that
//! layout for the next drawing. A script that appends a row and reads
//! `scrollHeight` reads the height with the row.
//!
//! What each answers, without CSSOM View's quirks branch, since no document
//! is in quirks mode here (law 1):
//!
//! 1. **`0` for an element with no box**: under `display: none`, in a tree
//!    not in the document, or in a document no window shows — one made
//!    beside the page's, which has no view.
//! 2. For the **root element**, the larger of the viewport's scrolling area
//!    and the viewport.
//! 3. For **any other element**, `body` included, its own scrolling area:
//!    its padding box extended by its content toward its end edges, right
//!    and bottom. What spills out to the left or above cannot be scrolled to
//!    and is not counted.
//! 4. Rounded to the nearest integer, a half up, and clamped to
//!    `0 ..= 2³¹ − 1` as `innerWidth` is ([`super::window_cssom_view::long`]).
//!
//! The view answers 2 and 3; this answers 1's document with no window, and 4.
//!
//! A page whose window was shown no view refuses by name, a `TypeError`, as
//! `innerWidth` does, rather than make up a size (§ 5, ADR 0013 § 3). So does
//! a view that could not measure, which is the embedder's bug.
//!
//! # Not here
//!
//! `clientWidth`, `clientHeight`, `offsetWidth`, `getBoundingClientRect` and
//! the rest of an element's geometry, each opened by a page (ADR 0038's *What
//! this does not decide*), and `scrollTop` and `scrollLeft`, which scroll.
//!
//! [`View`]: crate::view::View

use alo_js::Escape;
use alo_js::heap::Ref;
use alo_js::object::native::{Answer, Call};
use alo_js::object::{Objects, Value};

use super::window_cssom_view::long;
use crate::define;
use crate::document_cell::DocumentCell;
use crate::idl::{self, Brand};
use crate::view::Extent;
use crate::window::Window;

/// `Element.prototype`'s two members.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "scrollWidth",
        scroll_width,
        None,
    )?;
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "scrollHeight",
        scroll_height,
        None,
    )
}

/// `scrollWidth`.
fn scroll_width(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let area = measured(call, "scrollWidth")?;
    Ok(Answer::Value(Value::Number(
        area.map_or(0.0, |area| long(area.width)),
    )))
}

/// `scrollHeight`.
fn scroll_height(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let area = measured(call, "scrollHeight")?;
    Ok(Answer::Value(Value::Number(
        area.map_or(0.0, |area| long(area.height)),
    )))
}

/// The scrolling area of the element `call` was made on, as its page's view
/// measures it now: [`None`] when it has no box, or its document no window.
fn measured(call: &Call<'_>, name: &'static str) -> Result<Option<Extent>, Escape> {
    let this = idl::this(call, Brand::Element, name)?;
    let seen = call.seen();
    let Some(window) = seen
        .embedded::<DocumentCell>(this.owner)
        .and_then(DocumentCell::window)
    else {
        // A document no page's window was associated with is shown nowhere:
        // nothing in it has a box.
        return Ok(None);
    };
    let view = seen
        .embedded::<Window>(window)
        .and_then(Window::view)
        .ok_or_else(|| {
            Escape::type_error(
                format!("this page was given no view, so it cannot say its '{name}'"),
                call.at(),
            )
        })?;
    let document = idl::read(call, this.owner)?;
    view.scrolling_area(document, this.node).map_err(|_| {
        Escape::type_error(
            format!("this page could not be measured for its '{name}': its renderer was busy"),
            call.at(),
        )
    })
}
