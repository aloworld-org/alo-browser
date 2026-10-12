/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A property of a string: `"abc".length`, `"abc"[1]`, `"/f/x".slice(3)`
//! (queue item 488).
//!
//! The specification reads one by making a `String` object of the string —
//! `ToObject` — and asking it, with the string itself as the receiver. That
//! wrapper is new, so it has nothing but what `StringCreate` gave it: its
//! `length` and one fixed property per code unit, then `String.prototype`
//! behind it. This file answers those three questions without making the
//! wrapper, which nothing could tell apart: no script can reach it, and a
//! getter found on the chain is called with the string as `this`, which is
//! what `GetValue` passes.
//!
//! Writing and deleting are the same wrapper asked the other two questions.
//! `OrdinarySet` cannot store into a string, so a write is a setter's call or
//! a refusal — silence in sloppy code and a `TypeError` in strict — and a
//! delete is refused only for the properties the wrapper is made with.
//!
//! A number's, a boolean's and a symbol's properties are the same reading
//! over their own prototypes, which do not exist yet: they stay
//! [`Missing::AWrapperObject`], and so does `ToObject` of a string where an
//! object is really kept — a sloppy function's `this`, `Object.prototype`'s
//! methods — which is queue item 489. Iterating a string reads a method
//! `String.prototype` does not have yet, and is refused by name
//! ([`Missing::AStringIterator`], queue item 490) rather than called not
//! iterable.

use crate::abrupt::{Escape, Internal, Missing};
use crate::heap::Ref;
use crate::object::symbol::WellKnown;
use crate::object::{Fault, Key, Value};

use super::Engine;

/// What a string has of its own under a key.
pub(super) enum Own {
    /// Its `length`.
    Length(f64),
    /// The code unit at an index, which reads as a string of that one unit.
    Unit(u16),
}

impl Engine {
    /// The property `key` is of the string `text` itself, if it is one:
    /// `StringGetOwnProperty`, and `length`.
    ///
    /// # Errors
    ///
    /// A fault for a reference that is not a string, which is this engine's
    /// own mistake.
    pub(super) fn own_of_string(&self, text: Ref, key: Key) -> Result<Option<Own>, Escape> {
        let units = self
            .objects
            .units(text)
            .ok_or(Escape::fault(Fault::NotAnObject))?;
        if let Some(at) = key.as_index() {
            return Ok(usize::try_from(at)
                .ok()
                .and_then(|at| units.get(at))
                .map(|unit| Own::Unit(*unit)));
        }
        let is_length = key
            .as_text()
            .and_then(|name| self.objects.units(name))
            .is_some_and(|name| name.iter().copied().eq("length".encode_utf16()));
        if !is_length {
            return Ok(None);
        }
        // A string is at most `LONGEST_STRING` units, which a `u32` holds.
        let length =
            u32::try_from(units.len()).map_err(|_| Escape::Broken(Internal::StackIsWrong))?;
        Ok(Some(Own::Length(f64::from(length))))
    }

    /// What `text[key]` reads before the chain is walked: its own property's
    /// value, or [`None`] and the walk starts at `String.prototype`.
    ///
    /// **Allocates** the one-unit string an index reads as, so whatever the
    /// caller means to keep must be on the stack.
    ///
    /// # Errors
    ///
    /// [`Escape::Full`] for a heap at its ceiling, and a fault for a
    /// reference that is not a string.
    pub(super) fn read_of_string(
        &mut self,
        text: Ref,
        key: Key,
        at: usize,
    ) -> Result<Option<Value>, Escape> {
        Ok(match self.own_of_string(text, key)? {
            Some(Own::Length(length)) => Some(Value::Number(length)),
            Some(Own::Unit(unit)) => Some(Value::Text(
                self.objects
                    .text(vec![unit])
                    .map_err(|why| Escape::refused(why, at))?,
            )),
            None => None,
        })
    }

    /// A refusal by name for a key `String.prototype` is owed and does not
    /// have yet, where reading it as missing would be a wrong answer rather
    /// than an absent one: `Symbol.iterator`, without which `for…of` would
    /// call a string not iterable (queue item 490). A method a page can
    /// feature-test is simply absent, as `[].push` is.
    ///
    /// # Errors
    ///
    /// [`Missing::AStringIterator`] for that key, and a fault if the realm
    /// has lost the symbol.
    pub(super) fn unbuilt_on_string(&self, key: Key) -> Result<(), Escape> {
        let iterator = self
            .realm
            .intrinsics()
            .well_known_key(&self.objects, WellKnown::Iterator)?;
        if key == iterator {
            return Err(Escape::NotBuiltYet(Missing::AStringIterator));
        }
        Ok(())
    }

    /// `String.prototype`, where a read of a string's property walks from.
    ///
    /// # Errors
    ///
    /// A fault if the realm has lost it, which is this engine's own bug.
    pub(super) fn string_prototype(&self) -> Result<Ref, Escape> {
        self.realm.intrinsics().string_prototype(&self.objects)
    }
}
