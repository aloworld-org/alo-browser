/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Turning an object into a primitive, which is a conversation with the script.
//!
//! `1 + {}` is one instruction, and finishing it may mean calling `valueOf`,
//! finding that it answered with another object, and calling `toString`. Each
//! of those is a call, a call is a frame, and a frame runs in the same loop as
//! everything else — so the instruction cannot simply *wait*. What it does
//! instead is the whole of this file:
//!
//! 1. it hands the object over ([`Engine::convert_at`]) and **rewinds its own
//!    program counter**, so the instruction will run again;
//! 2. each call comes back here ([`Engine::carry_on`]), which either has a
//!    primitive or starts the next call;
//! 3. the primitive is written **into the operand's own stack slot**, and the
//!    instruction runs again with it in place.
//!
//! # Why writing it back is enough, and no state is kept about the operator
//!
//! An instruction is written peek-then-replace: it reads its operands where they
//! lie and takes them off only once the answer exists (see the module comment on
//! [`interpret`](super)). So running one a second time is not a retry — it is
//! the same instruction on operands one of which is now a primitive, which is
//! exactly what the specification's next step is. `a + b` with objects on both
//! sides runs three times and calls each side's `valueOf` once, in the order
//! [`operate`](crate::operate) asks for them.
//!
//! # `Symbol.toPrimitive` first
//!
//! Before either name, an object's `Symbol.toPrimitive` is asked for (queue
//! item 356). If there is one it is called with the hint as a string and its
//! answer is final — an object is a `TypeError`, not a reason to try `valueOf`
//! — and if there is none the search below begins. Only `Date.prototype` has
//! one today, since no page can spell the symbol (item 73).
//!
//! # It cannot loop
//!
//! Two things could. A method that keeps answering with an object does not,
//! because the search carries on at the *next* name and there are two:
//! [`Converting::next`] only ever grows, and running out is the `TypeError` the
//! specification gives. And a `valueOf` that reads the property it is being
//! called for makes a *frame* each time, which is
//! [`bounds::CALLS_ON_THE_STACK`](crate::bounds) and a `RangeError` a page can
//! catch.

use crate::abrupt::{Escape, Internal};
use crate::convert::{self, Hint, Primitive, Wanted};
use crate::object::Value;

use super::Engine;
use super::call::Ask;
use super::frame::{After, Converting, Run, Step, Then};

impl Engine {
    /// Begin turning the object at `at` into a primitive, and run the
    /// instruction at `pc` again once it is one.
    ///
    /// The rewind is done here rather than by each caller because it is the
    /// half that is easy to leave out and impossible to see afterwards: an
    /// instruction that started a conversion and did *not* rewind would carry
    /// on with the object still in its operand.
    pub(super) fn convert_at(
        &mut self,
        run: &mut Run,
        at: usize,
        hint: Hint,
        pc: usize,
        source: usize,
    ) -> Result<(), Escape> {
        run.frame_mut()?.pc = pc;
        self.want_primitive(run, at, hint, convert::EXOTIC, source, Then::Instruction)
    }

    /// `OrdinaryToPrimitive` from the name at `from`, as the call it needs.
    pub(super) fn want_primitive(
        &mut self,
        run: &mut Run,
        at: usize,
        hint: Hint,
        from: usize,
        source: usize,
        then: Then,
    ) -> Result<(), Escape> {
        let Value::Object(object) = self.value_at(run, at)? else {
            // Only an operand that is an object is ever handed over, and the
            // slot it is in belongs to the instruction that handed it over.
            return Err(Escape::Broken(Internal::StackIsWrong));
        };
        let (callee, step, next) =
            match convert::primitive_of(&self.objects, &self.names, object, hint, from, source)? {
                Wanted::Call { method, next } => (method, Step::Calling, next),
                Wanted::Fetch { getter, next } => (getter, Step::Fetching, next),
                Wanted::Exotic { method } => (method, Step::CallingExotic, convert::ORDINARY),
                Wanted::FetchExotic { getter } => (getter, Step::FetchingExotic, convert::ORDINARY),
            };
        let receiver = self.value_at(run, at)?;
        let callee_at = self.height(run)?;
        // `Symbol.toPrimitive` is handed the hint as a string, which `Names`
        // roots, so nothing here allocates before the call holds it.
        let hinted = [self.names.hint_text(hint)];
        let arguments: &[Value] = if step == Step::CallingExotic {
            &hinted
        } else {
            &[]
        };
        self.begin_call(
            run,
            callee_at,
            Ask {
                callee: Value::Object(callee),
                receiver,
                arguments,
                at: source,
                after: After::Convert(Converting {
                    step,
                    at,
                    hint,
                    next,
                    source,
                    then,
                }),
            },
        )
    }

    /// One of a conversion's calls has returned.
    pub(super) fn carry_on(&mut self, run: &mut Run, state: Converting) -> Result<(), Escape> {
        let answered = self.pop(run)?;
        match state.step {
            // What a getter answered *is* the method — if it is callable. If it
            // is not, the specification's `IsCallable` check moves on to the
            // other name rather than throwing.
            Step::Fetching => match self.function_of(answered) {
                Some(_) => self.call_method(run, answered, state),
                None => self.want_primitive(
                    run,
                    state.at,
                    state.hint,
                    state.next,
                    state.source,
                    state.then,
                ),
            },
            // `GetMethod`: `undefined` and `null` are no method, so the search
            // carries on at `OrdinaryToPrimitive`; anything else must be
            // callable.
            Step::FetchingExotic => match answered {
                Value::Undefined | Value::Null => self.want_primitive(
                    run,
                    state.at,
                    state.hint,
                    convert::ORDINARY,
                    state.source,
                    state.then,
                ),
                _ if self.function_of(answered).is_some() => self.call_method(
                    run,
                    answered,
                    Converting {
                        step: Step::CallingExotic,
                        ..state
                    },
                ),
                _ => Err(Escape::type_error(
                    "the getter for Symbol.toPrimitive answered something that is not a function",
                    state.source,
                )),
            },
            Step::CallingExotic => {
                if Primitive::of(answered).is_none() {
                    return Err(Escape::type_error(
                        "Symbol.toPrimitive answered an object, which is not a primitive value",
                        state.source,
                    ));
                }
                self.write_at(run, state.at, answered)?;
                match state.then {
                    Then::Instruction => Ok(()),
                    Then::Builtin => run.answered(),
                }
            }
            Step::Calling => {
                if Primitive::of(answered).is_some() {
                    // The answer takes the place the object was in — an
                    // instruction's operand, or a builtin's answer slot — and
                    // whichever of the two wanted it runs next.
                    self.write_at(run, state.at, answered)?;
                    return match state.then {
                        Then::Instruction => Ok(()),
                        Then::Builtin => run.answered(),
                    };
                }
                // An object again, so this name has not converted anything: the
                // other one is tried, and `next` is why the same one is not.
                self.want_primitive(
                    run,
                    state.at,
                    state.hint,
                    state.next,
                    state.source,
                    state.then,
                )
            }
        }
    }

    /// Call the method a getter handed over: `valueOf` or `toString` with no
    /// arguments, or `Symbol.toPrimitive` with the hint's string.
    fn call_method(
        &mut self,
        run: &mut Run,
        method: Value,
        state: Converting,
    ) -> Result<(), Escape> {
        let receiver = self.value_at(run, state.at)?;
        let callee_at = self.height(run)?;
        let hinted = [self.names.hint_text(state.hint)];
        let (step, arguments): (Step, &[Value]) = if state.step == Step::CallingExotic {
            (Step::CallingExotic, &hinted)
        } else {
            (Step::Calling, &[])
        };
        self.begin_call(
            run,
            callee_at,
            Ask {
                callee: method,
                receiver,
                arguments,
                at: state.source,
                after: After::Convert(Converting { step, ..state }),
            },
        )
    }
}
