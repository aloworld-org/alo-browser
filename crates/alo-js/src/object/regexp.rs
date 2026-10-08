/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `RegExp` object: an ordinary object with the compiled pattern beside it
//! (queue item 74, ADR 0029 § 5).
//!
//! The specification gives one three internal slots — `[[RegExpMatcher]]`,
//! `[[OriginalSource]]` and `[[OriginalFlags]]` — and all three are the
//! compiled [`Program`], which carries its source and flags. The program is
//! shared with the [`Unit`](crate::Unit) the literal is in and with every
//! other object that literal has made, and it holds no heap reference, so
//! tracing one of these is tracing the ordinary object it also is.
//!
//! `lastIndex` is not a slot. It is an own data property a page may read and
//! write — writable, not enumerable, not configurable — defined when the
//! object is made, and the only state a match leaves behind.

use std::sync::Arc;

use crate::heap::Tracer;
use crate::regexp::Program;

use super::ordinary::Ordinary;

/// A `RegExp` object.
#[derive(Debug)]
pub struct RegExp {
    ordinary: Ordinary,
    program: Arc<Program>,
}

impl RegExp {
    /// One of this program, inheriting from `prototype`.
    pub fn new(prototype: Option<crate::heap::Ref>, program: Arc<Program>) -> Self {
        Self {
            ordinary: Ordinary::with_prototype(prototype),
            program,
        }
    }

    /// Its compiled pattern.
    pub const fn program(&self) -> &Arc<Program> {
        &self.program
    }

    /// The ordinary object it also is.
    pub const fn ordinary(&self) -> &Ordinary {
        &self.ordinary
    }

    /// The same, to be written through.
    pub const fn ordinary_mut(&mut self) -> &mut Ordinary {
        &mut self.ordinary
    }

    /// Report every edge, which are the ordinary object's: a program holds
    /// none.
    pub fn trace(&self, tracer: &mut Tracer) {
        self.ordinary.trace(tracer);
    }

    /// What it owns beyond its slot, which is its table. The program is
    /// shared and counted by nothing here, as a function's chunk is not.
    pub fn footprint(&self) -> usize {
        self.ordinary.footprint()
    }
}
