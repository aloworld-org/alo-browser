/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What time it is, which this crate is told rather than reads (ADR 0036 § 1).
//!
//! ADR 0013 § 5 gives the engine no clock, and that stands: an engine that
//! reads nothing is an engine a test can hold still. So `Date` is the
//! engine's — every step from a time value to a year is ECMA-262's arithmetic
//! ([`time`](crate::time)) — and **the instant is the embedder's**. A realm is
//! made with a [`Clock`] or with none
//! ([`Engine::with_clock`](crate::Engine::with_clock)), and one made with none
//! refuses to say what time it is, by name: `Date.now()`, `new Date()` and
//! `Date()` throw a `TypeError`, and every use of `Date` that needs no clock
//! still works. A made-up instant would be the approximate answer ADR 0013 § 3
//! forbids, because a page cannot tell it from the truth.
//!
//! [`Fixed`] is the one clock this crate has, and it reads nothing either: one
//! instant, chosen by whoever made it. It is what a test hands a realm, and
//! what the corpus loads every case with, so that no reference depends on the
//! day it was rendered (ADR 0036 § 5). The renderer's real clock — the
//! machine's wall clock, in whole milliseconds — is the renderer's, in a file
//! of its own (ADR 0036 § 2).

use std::fmt;

/// Where a realm's *now* comes from.
///
/// One question: what is the time value now — milliseconds since the epoch,
/// an integer, or `NaN` if the embedder cannot say. The engine passes what it
/// answers through `TimeClip`, so a fraction is truncated and an instant
/// outside ECMA-262's ±8.64 × 10¹⁵ ms is an Invalid Date rather than a panic
/// or a wrong year; but **the grain is the embedder's to decide**, and a clock
/// that answers a fraction is handing the engine something finer than it
/// should have been given (ADR 0036 § 2).
pub trait Clock: fmt::Debug {
    /// The time value now.
    fn now(&self) -> f64;
}

/// A clock that answers one instant, always.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fixed(f64);

impl Fixed {
    /// A clock stopped at `instant`, a time value.
    pub const fn at(instant: f64) -> Self {
        Self(instant)
    }
}

impl Clock for Fixed {
    fn now(&self) -> f64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{Clock, Fixed};

    #[test]
    fn a_fixed_clock_answers_the_same_instant_every_time() {
        let clock = Fixed::at(1_760_000_000_000.0);
        assert_eq!(clock.now().to_bits(), 1_760_000_000_000.0_f64.to_bits());
        assert_eq!(clock.now().to_bits(), clock.now().to_bits());
    }
}
