/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `Date` object: an ordinary object with the `[[DateValue]]` slot beside
//! it (queue item 356, ADR 0036).
//!
//! The slot is a time value — an integral number of milliseconds within
//! ±8.64 × 10¹⁵ of the epoch, or `NaN` for an Invalid Date — and it holds no
//! heap reference, so tracing one of these is tracing the ordinary object it
//! also is. Only `Date`'s own builtins read or write it: every other property
//! of a date is the ordinary answer.

use crate::heap::{Ref, Tracer};
use crate::time;

use super::ordinary::Ordinary;

/// A `Date` object.
#[derive(Debug)]
pub struct Date {
    ordinary: Ordinary,
    /// `[[DateValue]]`: always through [`time::time_clip`].
    value: f64,
}

impl Date {
    /// An Invalid Date inheriting from `prototype`, which is what `new Date`
    /// is given before its body works out which instant it is.
    pub fn new(prototype: Option<Ref>) -> Self {
        Self {
            ordinary: Ordinary::with_prototype(prototype),
            value: f64::NAN,
        }
    }

    /// `[[DateValue]]`.
    pub const fn value(&self) -> f64 {
        self.value
    }

    /// Set `[[DateValue]]` to `value`, clipped — so a date holds a time value
    /// whatever it is handed.
    pub fn set(&mut self, value: f64) {
        self.value = time::time_clip(value);
    }

    /// The ordinary object it also is.
    pub const fn ordinary(&self) -> &Ordinary {
        &self.ordinary
    }

    /// The same, to be written through.
    pub const fn ordinary_mut(&mut self) -> &mut Ordinary {
        &mut self.ordinary
    }

    /// Report every edge, which are the ordinary object's: a time value holds
    /// none.
    pub fn trace(&self, tracer: &mut Tracer) {
        self.ordinary.trace(tracer);
    }

    /// What it owns beyond its slot, which is its table.
    pub fn footprint(&self) -> usize {
        self.ordinary.footprint()
    }
}

#[cfg(test)]
mod tests {
    use super::Date;

    #[test]
    fn a_date_is_invalid_until_it_is_set_and_holds_only_a_time_value() {
        let mut date = Date::new(None);
        assert!(date.value().is_nan());
        date.set(1.9);
        assert_eq!(date.value().to_bits(), 1.0_f64.to_bits());
        date.set(8.64e15 + 1.0);
        assert!(
            date.value().is_nan(),
            "one past the range is an Invalid Date"
        );
        date.set(-0.0);
        assert_eq!(date.value().to_bits(), 0.0_f64.to_bits());
    }
}
