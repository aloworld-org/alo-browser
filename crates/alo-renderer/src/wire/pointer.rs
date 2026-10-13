/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A person's pointer on the wire (ADR 0024 § 5, queue item 535).
//!
//! It goes to a renderer, from the browser process, and a confined renderer
//! reads it as it reads anything: what the button did by a closed list of
//! tags, and the point as two finite numbers — a NaN or an infinity is
//! refused by [`Reader::float`], since a comparison that answers "false" to
//! everything would turn the hit test's bounds into ones that pass.

use super::{Reader, Unreadable, Writer, unreadable};
use crate::pointer::{Pointer, Went};
use alo_paint::Point;

impl Writer {
    /// What the button did, then where.
    pub(super) fn pointer(&mut self, pointer: &Pointer) {
        self.tag(match pointer.went {
            Went::Down => 0,
            Went::Up => 1,
        });
        self.float(pointer.at.x);
        self.float(pointer.at.y);
    }
}

impl Reader<'_> {
    /// What the button did, then where.
    pub(super) fn pointer(&mut self) -> Result<Pointer, Unreadable> {
        let went = match self.tag()? {
            0 => Went::Down,
            1 => Went::Up,
            other => return Err(unreadable(format!("a button that went {other}"))),
        };
        let x = self.float()?;
        let y = self.float()?;
        Ok(Pointer {
            went,
            at: Point::new(x, y),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::message::ToRenderer;
    use crate::pointer::Pointer;
    use crate::wire::{read_to_renderer, write_to_renderer};

    #[test]
    fn a_pointer_crosses_whole() {
        for pointer in [
            Pointer::down(70.6, 50.3),
            Pointer::up(-12.25, 1.0e6),
            Pointer::up(0.0, -0.0),
        ] {
            let message = ToRenderer::Pointer(pointer);
            assert_eq!(read_to_renderer(&write_to_renderer(&message)), Ok(message));
        }
    }

    #[test]
    fn a_point_that_is_not_a_number_is_refused() {
        for (x, y) in [
            (f32::NAN, 1.0),
            (1.0, f32::INFINITY),
            (f32::NEG_INFINITY, 0.0),
        ] {
            let bytes = write_to_renderer(&ToRenderer::Pointer(Pointer::down(x, y)));
            assert!(read_to_renderer(&bytes).is_err(), "({x}, {y}) was read");
        }
    }

    #[test]
    fn a_button_that_went_anywhere_else_is_refused() {
        let mut bytes = write_to_renderer(&ToRenderer::Pointer(Pointer::down(1.0, 2.0)));
        for went in [2_u8, 7, 255] {
            if let Some(tag) = bytes.get_mut(1) {
                *tag = went;
            }
            assert!(
                read_to_renderer(&bytes).is_err(),
                "a button that went {went}"
            );
        }
    }

    #[test]
    fn a_pointer_cut_short_or_run_long_is_refused() {
        let bytes = write_to_renderer(&ToRenderer::Pointer(Pointer::up(3.0, 4.0)));
        for length in 0..bytes.len() {
            let cut = bytes.get(..length).unwrap_or_default();
            assert!(read_to_renderer(cut).is_err(), "{length} bytes were read");
        }
        let mut long = bytes.clone();
        long.push(0);
        assert!(read_to_renderer(&long).is_err());
    }
}
