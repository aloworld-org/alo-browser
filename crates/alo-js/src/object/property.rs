/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a property is, and what may replace it.
//!
//! ADR 0014 § 11: *a property is data or accessor with the three attributes the
//! language specifies; **nothing here has a fourth**.* That last clause is the
//! one worth holding to. An engine that adds a private flag to a property —
//! "this one is really an array index", "this one came from the DOM" — has made
//! every rule below take an extra case, and the rules below are what a page
//! froze an object with.
//!
//! # The rules are here rather than in a builtin
//!
//! `Object.defineProperty` is queue item 73's, and what it may *do* is not: a
//! non-configurable property that can be redefined is a page's promise broken,
//! whether the redefinition came from a builtin, from the DOM's bindings or
//! from the interpreter storing to a field. So [`Property::may_replace`] is the
//! specification's `ValidateAndApplyPropertyDescriptor`, in the object model,
//! asked by everything that defines anything.
//!
//! # What is cut, and where it went
//!
//! A **partial** descriptor — `{ writable: false }` with no `value` — is item
//! 73's, because it is `Object.defineProperty`'s own reading of an argument
//! object rather than a rule about properties. Everything here takes a complete
//! property, which is what the interpreter has when it stores a field and what
//! a partial descriptor becomes once it has been completed against what is
//! already there.

use crate::heap::{Barrier, Tracer};

use super::value::{Stored, Value};

/// A property of an object.
#[derive(Debug)]
pub struct Property {
    what: What,
    enumerable: bool,
    configurable: bool,
}

/// The two kinds a property may be.
///
/// Not [`Clone`], because both hold [`Stored`] and copying one into a cell is a
/// store (ADR 0014 § 5).
#[derive(Debug)]
enum What {
    /// A value, and whether it may be written.
    Data { value: Stored, writable: bool },
    /// A getter and a setter, either of which may be absent.
    ///
    /// Held as [`Stored`] rather than as
    /// [`Field`](crate::heap::Field) so that "no getter" is `undefined` — which
    /// is what the language says it is, and what a page reading the descriptor
    /// back gets.
    Accessor { get: Stored, set: Stored },
}

impl Property {
    /// A data property.
    pub const fn data(value: Value, writable: bool, enumerable: bool, configurable: bool) -> Self {
        Self {
            what: What::Data {
                value: Stored::holding(value),
                writable,
            },
            enumerable,
            configurable,
        }
    }

    /// A data property with the attributes an assignment gives one: writable,
    /// enumerable and configurable.
    ///
    /// This is what `a.b = 1` on an object with no such property creates, and
    /// what an object literal's fields are.
    pub const fn plain(value: Value) -> Self {
        Self::data(value, true, true, true)
    }

    /// An accessor property. Either half may be [`Value::Undefined`].
    pub const fn accessor(get: Value, set: Value, enumerable: bool, configurable: bool) -> Self {
        Self {
            what: What::Accessor {
                get: Stored::holding(get),
                set: Stored::holding(set),
            },
            enumerable,
            configurable,
        }
    }

    /// The value of a data property, or [`None`] if this is an accessor.
    pub const fn value(&self) -> Option<Value> {
        match &self.what {
            What::Data { value, .. } => Some(value.get()),
            What::Accessor { .. } => None,
        }
    }

    /// The getter of an accessor property, or [`None`] if this is data.
    ///
    /// [`Some`]`(`[`Value::Undefined`]`)` is an accessor with no getter, which
    /// reads as `undefined` rather than being an error — the distinction the
    /// two levels of option carry.
    pub const fn getter(&self) -> Option<Value> {
        match &self.what {
            What::Accessor { get, .. } => Some(get.get()),
            What::Data { .. } => None,
        }
    }

    /// The setter of an accessor property, or [`None`] if this is data.
    pub const fn setter(&self) -> Option<Value> {
        match &self.what {
            What::Accessor { set, .. } => Some(set.get()),
            What::Data { .. } => None,
        }
    }

    /// Whether this is a data property rather than an accessor.
    pub const fn is_data(&self) -> bool {
        matches!(self.what, What::Data { .. })
    }

    /// Whether a data property may be written to. An accessor answers `false`:
    /// what it has instead is a setter.
    pub const fn is_writable(&self) -> bool {
        match self.what {
            What::Data { writable, .. } => writable,
            What::Accessor { .. } => false,
        }
    }

    /// Whether a `for…in` or `Object.keys` sees it.
    pub const fn is_enumerable(&self) -> bool {
        self.enumerable
    }

    /// Whether it may be deleted or redefined.
    pub const fn is_configurable(&self) -> bool {
        self.configurable
    }

    /// Write a new value into a data property, through the barrier.
    ///
    /// The caller has already decided it may: this is the store, and
    /// [`Property::is_writable`] is the decision. It answers `false` on an
    /// accessor, which is not a failure — it is the caller asking the wrong
    /// question, and answering rather than panicking is the rule the whole
    /// crate is written to.
    pub fn write(&mut self, barrier: &mut Barrier, value: Value) -> bool {
        match &mut self.what {
            What::Data { value: held, .. } => {
                held.set(barrier, value);
                true
            }
            What::Accessor { .. } => false,
        }
    }

    /// Whether `next` may replace this property, which is the specification's
    /// `ValidateAndApplyPropertyDescriptor` for a complete descriptor.
    ///
    /// A **configurable** property may be replaced by anything: that is what
    /// configurable means. Everything below is therefore the non-configurable
    /// case, and each clause is a promise a page made when it froze something:
    ///
    /// - it cannot become configurable again, or freezing would be undoable;
    /// - its enumerability cannot change, since a page can see that;
    /// - it cannot change between data and accessor;
    /// - a non-writable data property cannot become writable, and its value
    ///   cannot change — by `SameValue`, which is why
    ///   [`Value::same_value`] exists;
    /// - an accessor's getter and setter cannot change.
    ///
    /// A non-configurable **writable** data property is the one case that is
    /// less strict than it looks: the value may change, and it may become
    /// non-writable, because a page that left it writable did not promise
    /// otherwise.
    pub fn may_replace(&self, next: &Self) -> bool {
        if self.configurable {
            return true;
        }
        if next.configurable || next.enumerable != self.enumerable {
            return false;
        }
        match (&self.what, &next.what) {
            (
                What::Data {
                    value: was,
                    writable: could_write,
                },
                What::Data {
                    value: now,
                    writable: may_write,
                },
            ) => {
                if *could_write {
                    return true;
                }
                !*may_write && was.get().same_value(now.get())
            }
            (
                What::Accessor { get: was, set: had },
                What::Accessor {
                    get: now,
                    set: takes,
                },
            ) => was.get().same_value(now.get()) && had.get().same_value(takes.get()),
            // Between the two kinds, which a non-configurable property may not
            // cross.
            _ => false,
        }
    }

    /// Report every edge this property holds.
    pub fn trace(&self, tracer: &mut Tracer) {
        match &self.what {
            What::Data { value, .. } => value.trace(tracer),
            What::Accessor { get, set } => {
                get.trace(tracer);
                set.trace(tracer);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Property, Value};

    #[test]
    fn a_configurable_property_may_be_replaced_by_anything() {
        let was = Property::plain(Value::Number(1.0));
        let accessor = Property::accessor(Value::Undefined, Value::Undefined, false, false);
        assert!(was.may_replace(&accessor));
    }

    #[test]
    fn a_frozen_property_keeps_its_value_and_its_kind() {
        // What `Object.freeze` leaves behind: not configurable, not writable.
        let frozen = Property::data(Value::Number(1.0), false, true, false);
        assert!(frozen.may_replace(&Property::data(Value::Number(1.0), false, true, false)));
        assert!(!frozen.may_replace(&Property::data(Value::Number(2.0), false, true, false)));
        assert!(!frozen.may_replace(&Property::data(Value::Number(1.0), true, true, false)));
        assert!(!frozen.may_replace(&Property::data(Value::Number(1.0), false, false, false)));
        assert!(!frozen.may_replace(&Property::data(Value::Number(1.0), false, true, true)));
        assert!(!frozen.may_replace(&Property::accessor(
            Value::Undefined,
            Value::Undefined,
            true,
            false
        )));
    }

    #[test]
    fn a_sealed_but_writable_property_may_still_change_value() {
        // Not configurable and writable is what `Object.seal` leaves: the value
        // may change, and it may be made non-writable, and that is a door that
        // only shuts one way.
        let sealed = Property::data(Value::Number(1.0), true, true, false);
        assert!(sealed.may_replace(&Property::data(Value::Number(2.0), true, true, false)));
        assert!(sealed.may_replace(&Property::data(Value::Number(2.0), false, true, false)));

        let shut = Property::data(Value::Number(2.0), false, true, false);
        assert!(!shut.may_replace(&Property::data(Value::Number(2.0), true, true, false)));
    }

    #[test]
    fn a_frozen_value_is_compared_by_same_value() {
        // The two zeroes are different values and `NaN` is one value, which is
        // the whole reason this comparison is not `==`.
        let frozen = Property::data(Value::Number(0.0), false, true, false);
        assert!(!frozen.may_replace(&Property::data(Value::Number(-0.0), false, true, false)));

        let nan = Property::data(Value::Number(f64::NAN), false, true, false);
        assert!(nan.may_replace(&Property::data(Value::Number(f64::NAN), false, true, false)));
    }

    #[test]
    fn a_frozen_accessor_keeps_both_halves() {
        let frozen = Property::accessor(Value::Undefined, Value::Undefined, true, false);
        assert!(frozen.may_replace(&Property::accessor(
            Value::Undefined,
            Value::Undefined,
            true,
            false
        )));
        assert!(!frozen.may_replace(&Property::accessor(
            Value::Null,
            Value::Undefined,
            true,
            false
        )));
    }
}
