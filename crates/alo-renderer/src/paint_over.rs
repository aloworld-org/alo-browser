/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a drawing is painted onto: the last drawing's own canvas, painted
//! over where the two differ, whenever that canvas is one it can be (queue
//! item 870, ADR 0101).
//!
//! A letter typed into the one box is a new first screen, styled and laid
//! out again — about four milliseconds — and was painted again whole: at
//! two with a card in the conversation, 103 ms of the card's two shadows
//! blurred, its fills and clips and the place left clear for its page, for a
//! letter that changed none of them. Now the last drawing is handed to the
//! next ([`crate::easel`]); when its canvas is the same size and was made in
//! the same colour, it is painted again only where the two display lists
//! can differ ([`alo_paint::repaint()`]), and comes out as a canvas painted
//! whole would. Otherwise it is painted whole, as before.

use std::rc::Rc;

use alo_paint::{Canvas, DisplayList, Repainted, repaint};
use alo_value::Rgba;

use crate::pipeline::Drawing;

/// `display` painted onto a canvas of `size` device pixels made in
/// `ground`: over `before`'s canvas where that is one it can be, or onto a
/// new one. What it painted is said with it.
pub fn painted(
    display: &DisplayList,
    size: (u32, u32),
    ground: Rgba,
    before: Option<Rc<Drawing>>,
) -> (Canvas, Repainted) {
    let fits = |drawing: &Drawing| {
        (drawing.canvas.width(), drawing.canvas.height()) == size && drawing.ground == ground
    };
    match before {
        Some(before) if fits(&before) => match Rc::try_unwrap(before) {
            // The drawing is nobody else's: its canvas is painted over in
            // place.
            Ok(Drawing {
                display: was,
                mut canvas,
                ..
            }) => {
                let repainted = repaint(&was, display, &mut canvas, ground);
                (canvas, repainted)
            }
            // Somebody still reads it: a copy is painted over.
            Err(shared) => {
                let mut canvas = shared.canvas.clone();
                let repainted = repaint(&shared.display, display, &mut canvas, ground);
                (canvas, repainted)
            }
        },
        _ => {
            let mut canvas = Canvas::new(size.0, size.1, ground);
            alo_paint::render(display, &mut canvas);
            (canvas, Repainted::Whole)
        }
    }
}
