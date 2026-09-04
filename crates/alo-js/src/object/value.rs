/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a property holds, and how a heap object holds one.
//!
//! ADR 0013 § 2's last paragraph is the whole of the representation decision:
//! *no `unsafe` in the value representation either. NaN-boxing and tagged
//! pointers are the obvious first `unsafe` in any engine, they are worth real
//! performance, and they are refused under law 4 on the same terms: measured
//! first, ADR second. **An enum is the starting representation**, and it is
//! allowed to be replaced by a safe compact one whenever somebody has the
//! numbers.* So [`Value`] is an enum, it is [`Copy`], and it is sixteen bytes
//! rather than eight.
//!
//! # Why this is here rather than in the interpreter
//!
//! Queue item 72 owns values in the sense of *producing* them. A property table
//! cannot exist without one to hold, though, so the type lands with the object
//! model that needs it and item 72 inherits it rather than inventing a second.
//!
//! # The one thing that is absent rather than approximate
//!
//! There is no `BigInt` here. ADR 0013 § 3: *absent beats approximate.* A
//! `BigInt` is arbitrary-precision arithmetic, which is a decision about
//! renting rather than a variant to add — [`crate::token::Kind::BigInt`] keeps
//! its digits as text for exactly this reason, and queue item 207 is where the
//! decision is made. A variant holding a `f64` would be a wrong answer that
//! reads like a right one.
//!
//! # And [`Stored`], which is the value half of ADR 0014 § 5
//!
//! A [`Field`](crate::heap::Field) is a heap reference held by a cell, written
//! only through a [`Barrier`]. A value may *be* a heap reference — a string, a
//! symbol, an object — so a value held by a cell needs the same barrier, and
//! [`Stored`] is that: the same rule, for a type that is a reference only
//! sometimes.

use crate::heap::{Barrier, Ref, Tracer};

/// A JavaScript value.
///
/// [`Copy`], because everything in an engine passes one around and a value that
/// had to be cloned would put an allocation in the middle of a property read.
/// The three variants carrying a [`Ref`] are the ones the collector cares
/// about; the rest are the language's primitives, held inline.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Value {
    /// `undefined`, which is also what a property nobody defined reads as.
    #[default]
    Undefined,
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A number, which is an IEEE 754 double and nothing else.
    Number(f64),
    /// A string, which is a heap object — see [`Text`](super::Text).
    Text(Ref),
    /// A symbol, which is a heap object because its *identity* is the value.
    Symbol(Ref),
    /// An object.
    Object(Ref),
}

impl Value {
    /// The heap reference in this value, if it is one.
    pub const fn reference(self) -> Option<Ref> {
        match self {
            Value::Text(held) | Value::Symbol(held) | Value::Object(held) => Some(held),
            Value::Undefined | Value::Null | Value::Bool(_) | Value::Number(_) => None,
        }
    }

    /// Whether this value is an object, which is what an internal method may be
    /// asked of.
    pub const fn is_object(self) -> bool {
        matches!(self, Value::Object(_))
    }

    /// The specification's `SameValue`, which is what redefining a property is
    /// judged by.
    ///
    /// It is **not** `==` on a [`f64`]: `SameValue(NaN, NaN)` is true and
    /// `SameValue(+0, -0)` is false, both the opposite of what the arithmetic
    /// operator answers. Getting this wrong makes
    /// [`Object.defineProperty`](super::Property) accept a redefinition of a
    /// non-configurable property that the language forbids, which is a hole in
    /// something a page froze on purpose.
    pub fn same_value(self, other: Self) -> bool {
        match (self, other) {
            (Value::Number(left), Value::Number(right)) => {
                if left.is_nan() && right.is_nan() {
                    return true;
                }
                // `to_bits` distinguishes the two zeroes, which is the whole
                // reason it is here rather than `==`.
                left.to_bits() == right.to_bits()
            }
            _ => self == other,
        }
    }
}

/// A value held by a heap object.
///
/// Not [`Clone`], for [`Field`](crate::heap::Field)'s reason: copying one into
/// another cell is a **store**, and a store goes through [`Stored::set`] and its
/// [`Barrier`] (ADR 0014 § 5). A derive here would be the second way to write
/// one that the decision says there must not be.
#[derive(Debug, Default)]
pub struct Stored(Value);

impl Stored {
    /// A place holding `value`, at the moment the object holding it is built.
    ///
    /// No barrier, for the reason [`Field::holding`](crate::heap::Field) gives:
    /// a barrier records a store *into the heap*, and this is on its way to
    /// [`Heap::allocate`](crate::heap::Heap::allocate). An object the marker has
    /// never seen has no edge to tell it about.
    pub const fn holding(value: Value) -> Self {
        Self(value)
    }

    /// What it holds.
    pub const fn get(&self) -> Value {
        self.0
    }

    /// Store a value, telling the collector about the reference in it if there
    /// is one.
    pub fn set(&mut self, barrier: &mut Barrier, value: Value) {
        barrier.stored(self.0.reference(), value.reference());
        self.0 = value;
    }

    /// Report the edge, if this value is one.
    pub fn trace(&self, tracer: &mut Tracer) {
        if let Some(held) = self.0.reference() {
            tracer.edge(held);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Value;

    #[test]
    fn same_value_is_not_the_arithmetic_comparison() {
        assert!(Value::Number(f64::NAN).same_value(Value::Number(f64::NAN)));
        assert!(!Value::Number(0.0).same_value(Value::Number(-0.0)));
        assert!(Value::Number(0.0).same_value(Value::Number(0.0)));
        assert!(Value::Number(1.5).same_value(Value::Number(1.5)));
    }

    #[test]
    fn values_of_different_kinds_are_never_the_same() {
        assert!(!Value::Undefined.same_value(Value::Null));
        assert!(!Value::Bool(true).same_value(Value::Number(1.0)));
        assert!(Value::Undefined.same_value(Value::Undefined));
    }
}
