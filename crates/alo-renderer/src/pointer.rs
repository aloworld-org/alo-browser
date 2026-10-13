/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A person's pointer, as the browser process sends it to a renderer (ADR
//! 0024 § 5, queue item 535).
//!
//! **The one coordinate that ever reaches a renderer**, and only a
//! person's: the browser process decides from its own composition which
//! frame a point is in and sends it in that frame's CSS pixels. Nothing an
//! agent can send becomes one of these — an agent's verbs name what they act
//! on and never where it is (ADR 0002), and `alo-window`'s socket has no
//! request that carries a point.
//!
//! What is sent is the main button going down or coming up, and where.
//! Moving without a button (queue item 542), the other buttons (544), a
//! press with a modifier held (545) and how many presses came in quick
//! succession (547) are not sent yet; the window drops them.

use alo_paint::Point;
use core::fmt;

/// What the main button did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Went {
    /// It was pressed.
    Down,
    /// It was let go.
    Up,
}

/// A person's main button going down or up, at a point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pointer {
    /// What the button did.
    pub went: Went,
    /// Where, in the frame's CSS pixels from its top-left corner. A release
    /// may be outside the frame: it goes to the frame the press went to,
    /// wherever the pointer has got to by then.
    pub at: Point,
}

impl Pointer {
    /// The main button pressed at (`x`, `y`).
    pub fn down(x: f32, y: f32) -> Self {
        Self {
            went: Went::Down,
            at: Point::new(x, y),
        }
    }

    /// The main button let go at (`x`, `y`).
    pub fn up(x: f32, y: f32) -> Self {
        Self {
            went: Went::Up,
            at: Point::new(x, y),
        }
    }
}

impl fmt::Display for Pointer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let went = match self.went {
            Went::Down => "pressed",
            Went::Up => "let go",
        };
        write!(f, "a person's main button {went} at {}", self.at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pointer_says_what_it_did_and_where() {
        assert_eq!(
            Pointer::down(70.5, 50.0).to_string(),
            "a person's main button pressed at (70.5, 50)",
        );
        assert_eq!(
            Pointer::up(-3.0, 9.0).to_string(),
            "a person's main button let go at (-3, 9)",
        );
    }
}
