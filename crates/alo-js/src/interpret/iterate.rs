/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The three instructions the iteration protocol adds (queue item 230).
//!
//! Everything else `for…of` does is a call or a property read a page can
//! intercept, so it is compiled to the ordinary instructions for those (see
//! [`code`](crate::code)'s module comment). What is left is a symbol no source
//! can spell and the `TypeError`s the protocol specifies for an answer of the
//! wrong shape. None of them allocates, and none takes anything off the stack:
//! each either lets the value stand or throws.

use crate::abrupt::Escape;
use crate::code::Expecting;
use crate::object::Value;
use crate::object::symbol::WellKnown;

use super::Engine;
use super::frame::Run;

impl Engine {
    /// Push a well-known symbol, which the realm made and roots.
    pub(super) fn push_well_known(
        &mut self,
        run: &mut Run,
        which: WellKnown,
    ) -> Result<(), Escape> {
        let held = self.realm.intrinsics().well_known(&self.objects, which)?;
        self.push(run, Value::Symbol(held))
    }

    /// `GetIterator`'s answer to what `obj[Symbol.iterator]` read as.
    ///
    /// `GetMethod` makes `null` and `undefined` "there is no method", and
    /// `GetIterator` makes no method a `TypeError`; a method that is there and
    /// cannot be called is `GetMethod`'s own `TypeError`. Both before the call,
    /// which is why this is an instruction rather than left to the call to
    /// refuse — the message is the one a person can act on.
    pub(super) fn iterable(&self, run: &Run, at: usize) -> Result<(), Escape> {
        let method = self.peek(run, 0)?;
        match method {
            Value::Undefined | Value::Null => Err(Escape::type_error(
                "this value is not iterable: it has no Symbol.iterator method",
                at,
            )),
            _ if self.function_of(method).is_none() => Err(Escape::type_error(
                "this value is not iterable: its Symbol.iterator is not a function",
                at,
            )),
            _ => Ok(()),
        }
    }

    /// A `TypeError` unless the top of the stack is an object.
    pub(super) fn require_object(
        &self,
        run: &Run,
        expecting: Expecting,
        at: usize,
    ) -> Result<(), Escape> {
        if matches!(self.peek(run, 0)?, Value::Object(_)) {
            return Ok(());
        }
        Err(Escape::type_error(
            match expecting {
                Expecting::AnIterator => {
                    "Symbol.iterator answered something that is not an object, so it is not an iterator"
                }
                Expecting::AResult => {
                    "an iterator's next() answered something that is not an object"
                }
                Expecting::AClosedResult => {
                    "an iterator's return() answered something that is not an object"
                }
            },
            at,
        ))
    }
}
