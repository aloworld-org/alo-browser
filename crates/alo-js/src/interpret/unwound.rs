/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a throw nothing caught had got to (queue item 241, cut from 78).
//!
//! A throw that no `try` guards leaves every call it was inside, and the moment
//! [`land`](super::Engine) finds nothing waiting for it is the last moment
//! those calls exist: the run gives every frame back on its way out. So that
//! is where they are read — each call, innermost first, by **the program it was
//! running and the byte offset of the instruction it was on**. For the call the
//! throw came from that is the `throw`, or the instruction whose operation
//! threw; for every call outside it, the call that was waiting on the one
//! inside.
//!
//! # What it is not
//!
//! **Not where an error was made.** A page that catches an error and throws it
//! again is traced from the second `throw`, because that is the throw that
//! escaped. A trace taken when an `Error` is constructed — what a page reads as
//! `error.stack` — is a property of the object, and it is the rest of item 78.
//!
//! **Not names.** A function's `name` is queue item 220, and a trace of a
//! minified page would read `a`, `b` and `c` anyway: the place is what finds
//! the line, and a column is the only thing that can find it in a file that is
//! one line long. Turning an offset into a line and a column needs the source
//! text, which no [`Unit`] keeps ([`Position::of`](crate::Position::of) does
//! it for whoever does).
//!
//! **Not a builtin.** A builtin has no frame and no source, so a throw from
//! inside one is placed at the call that entered it.
//!
//! # A bound, and saying it was reached
//!
//! [`PLACES_IN_A_TRACE`](crate::bounds::PLACES_IN_A_TRACE): a runaway
//! recursion leaves ten thousand calls, and the innermost are kept with the
//! rest counted.

use std::rc::Rc;

use crate::abrupt::{Escape, Internal};
use crate::bounds;
use crate::unit::Unit;

use super::frame::Run;

/// One call a throw left, and the instruction it was on.
#[derive(Debug, Clone)]
pub struct Place {
    unit: Rc<Unit>,
    at: usize,
}

impl Place {
    /// The program the call was running, which an embedder recognises by
    /// identity ([`Rc::ptr_eq`]) as the one it handed to
    /// [`Engine::run`](super::Engine::run).
    pub fn unit(&self) -> &Rc<Unit> {
        &self.unit
    }

    /// The byte offset in that program's source of the instruction the call was
    /// on.
    pub const fn at(&self) -> usize {
        self.at
    }
}

/// The calls a throw nothing caught left, innermost first.
#[derive(Debug, Clone, Default)]
pub struct Unwound {
    places: Vec<Place>,
    left_out: usize,
}

impl Unwound {
    /// Each call, innermost first, at most
    /// [`PLACES_IN_A_TRACE`](crate::bounds::PLACES_IN_A_TRACE) of them.
    ///
    /// Empty when the throw left no call at all: a callee that was not a
    /// function, or a builtin an embedder called directly.
    pub fn places(&self) -> &[Place] {
        &self.places
    }

    /// How many calls further out were left and not kept.
    pub const fn left_out(&self) -> usize {
        self.left_out
    }

    /// Nothing unwound: what every run starts from.
    pub(super) fn clear(&mut self) {
        self.places.clear();
        self.left_out = 0;
    }

    /// The calls of `run`, innermost first, read before any is given back.
    pub(super) fn of(run: &Run) -> Result<Self, Escape> {
        let frames = run.frames.len();
        let kept = frames.min(bounds::PLACES_IN_A_TRACE);
        let mut places = Vec::with_capacity(kept);
        for frame in run.frames.iter().rev().take(kept) {
            let unit = &run
                .units
                .get(frame.unit)
                .ok_or(Escape::Broken(Internal::StackIsWrong))?
                .unit;
            let chunk = unit
                .chunk(frame.chunk)
                .ok_or(Escape::Broken(Internal::JumpIsWrong))?;
            places.push(Place {
                unit: Rc::clone(unit),
                at: chunk.at(frame.now),
            });
        }
        Ok(Self {
            places,
            left_out: frames.saturating_sub(kept),
        })
    }
}
