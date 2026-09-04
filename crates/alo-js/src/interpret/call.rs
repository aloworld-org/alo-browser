/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Making a function, entering it, and leaving it.
//!
//! # A call is a frame, not a recursion
//!
//! [`Engine::enter`] pushes a [`Frame`] and returns to the same loop, which is
//! why a program cannot choose how much of *this process's* stack it uses by
//! nesting calls. What it can choose is how many frames there are, and that is
//! bounded twice: by [`bounds::CALLS_ON_THE_STACK`] and by
//! [`bounds::VALUES_ON_THE_STACK`], both of which are a `RangeError` — the
//! error the language specifies for a recursion that will not end, which a page
//! may catch, rather than a process that stops.
//!
//! # What the stack looks like at a call
//!
//! ```text
//!   … the caller's operands … | callee | this | arg0 … argN | locals … | operands …
//!                               ^                             ^          ^
//!                               callee_at                     locals_at  base
//! ```
//!
//! Every form of call leaves that shape, which is why the callee needs to know
//! nothing about how it was reached. `return` truncates to `callee_at` and
//! pushes the answer, so the arguments and the `this` go with the frame.
//!
//! # `this` is decided by the callee's strictness, not the caller's
//!
//! The compiler pushes `undefined` for a plain call and the object for a method
//! call, and this file then applies `OrdinaryCallBindThis`: **strict code takes
//! what it was given**, and sloppy code turns `undefined` and `null` into the
//! global object. A primitive `this` in sloppy code should become a wrapper
//! object, which is queue item 73 — so it says so, rather than passing the
//! primitive through and being quietly wrong about `this.length`.
//!
//! An **arrow** is the case that skips all of it: it has no `this` of its own,
//! so the one it captured where it was written is used and what the caller
//! pushed is overwritten. That is the whole of the difference, and it is one
//! `match`.
//!
//! # A call an instruction wanted, rather than one the program wrote
//!
//! A getter, a setter and a `valueOf` are calls nothing in the source spells:
//! the instruction is half way through and needs one before it can finish.
//! [`Engine::begin_call`] is how it asks — it lays the same shape out on the
//! stack, from a place the instruction chooses, so the callee still knows
//! nothing about how it was reached. What differs is [`After`], which the
//! frame carries and `return` reads: the answer lands where the callee stood, or
//! is dropped, or is a step of a conversion that then runs the instruction
//! again.

use std::rc::Rc;

use crate::abrupt::{Escape, Internal, Missing};
use crate::bounds;
use crate::heap::Ref;
use crate::object::Value;
use crate::unit::Unit;

use super::Engine;
use super::frame::{After, Frame, Loaded, Run};

impl Engine {
    /// `Op::Closure`: a function of a chunk of the running program, over the
    /// environment in force where it was written — which is a block's if it was
    /// written inside one.
    pub(super) fn make_closure(
        &mut self,
        run: &mut Run,
        which: u32,
        at: usize,
    ) -> Result<(), Escape> {
        let unit = Rc::clone(&run.loaded()?.unit);
        let this_at = run.frame()?.this_at;
        let environment = self.environment_of(run)?;
        let arrow = unit
            .chunk(which)
            .ok_or(Escape::Broken(Internal::JumpIsWrong))?
            .is_arrow();
        // An arrow takes the `this` that is in force here, and takes it whether
        // it writes `this` or not: an arrow nested inside it may, and by then
        // this frame has gone.
        let captured = if arrow {
            Some(self.value_at(run, this_at)?)
        } else {
            None
        };
        // A safepoint. The environment is kept by this frame's root and the
        // captured value by the stack, so both survive it.
        let held = self
            .objects
            .function(unit, which, environment, captured)
            .map_err(|why| Escape::refused(why, at))?;
        self.push(run, Value::Object(held))
    }

    /// `Op::Call`: the callee, its `this` and `argc` arguments are on the stack.
    pub(super) fn enter(&mut self, run: &mut Run, argc: u32, at: usize) -> Result<(), Escape> {
        let argc = usize::try_from(argc).map_err(|_| Escape::Broken(Internal::StackIsWrong))?;
        let height = self.height(run)?;
        let callee_at = height
            .checked_sub(argc.saturating_add(2))
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        self.enter_at(run, callee_at, argc, at, After::Answer)
    }

    /// Put a call the *instruction* wanted on the stack, and enter it.
    ///
    /// Everything from `callee_at` up is replaced by the call's own things, so
    /// an instruction hands over the operands it was working on and gets the
    /// answer in their place. Nothing between reading those values and writing
    /// them back allocates, which is what makes it safe to hold them in Rust
    /// locals while the slots that were holding them are gone.
    pub(super) fn begin_call(
        &mut self,
        run: &mut Run,
        callee_at: usize,
        ask: Ask<'_>,
    ) -> Result<(), Escape> {
        let stack = run.stack;
        self.objects
            .with_slots(stack, |slots, _| {
                slots.truncate(callee_at);
                slots.push(ask.callee);
                slots.push(ask.receiver);
                for value in ask.arguments {
                    slots.push(*value);
                }
            })
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        self.enter_at(run, callee_at, ask.arguments.len(), ask.at, ask.after)
    }

    /// How many values are on the stack.
    pub(super) fn height(&self, run: &Run) -> Result<usize, Escape> {
        self.objects
            .slot_count(run.stack)
            .ok_or(Escape::Broken(Internal::StackIsWrong))
    }

    /// Enter the call whose callee sits at `callee_at`.
    fn enter_at(
        &mut self,
        run: &mut Run,
        callee_at: usize,
        argc: usize,
        at: usize,
        after: After,
    ) -> Result<(), Escape> {
        let height = self.height(run)?;
        if callee_at < run.base().unwrap_or(usize::MAX)
            || height != callee_at.saturating_add(2).saturating_add(argc)
        {
            // The compiler said the stack would look like this. It does not, so
            // the compiler and this loop disagree, which is our bug.
            return Err(Escape::Broken(Internal::StackIsWrong));
        }
        let this_at = callee_at.saturating_add(1);

        let callee = self.value_at(run, callee_at)?;
        let Value::Object(held) = callee else {
            return Err(self.not_a_function(callee, at));
        };
        let Some((unit, chunk, environment, captured)) = self.code_of(held) else {
            return Err(self.not_a_function(callee, at));
        };
        let Some((parameters, bindings, locals, strict, own)) = shape_of(&unit, chunk) else {
            return Err(Escape::Broken(Internal::JumpIsWrong));
        };

        let this = match captured {
            Some(value) => value,
            None => self.receiver(run, this_at, strict)?,
        };
        self.write_at(run, this_at, this)?;

        if run.frames.len() >= bounds::CALLS_ON_THE_STACK {
            return Err(Escape::range_error(
                "this script calls more deeply than this engine will go",
                at,
            ));
        }

        // Two safepoints, and everything either of them could lose is already
        // somewhere the collector walks: the callee and its arguments are on
        // the stack, and the environment being closed over is held by the
        // function cell that is on it.
        let loaded = self.load_program(run, &unit)?;
        let made = self
            .objects
            .environment(environment, bindings)
            .map_err(|why| Escape::refused(why, at))?;
        let root = self.objects.heap_mut().root(made);

        for which in 0..parameters {
            let value = if which < argc {
                self.value_at(run, this_at.saturating_add(1).saturating_add(which))?
            } else {
                // A parameter nobody passed is `undefined` and is **given** that
                // rather than left in a dead zone, which is what makes reading
                // one a value and reading a `let` above its line an error.
                Value::Undefined
            };
            self.write_binding(made, which, value)?;
        }
        if let Some(slot) = own {
            // A named function expression can see itself, before anything has
            // assigned it anywhere.
            let slot = usize::try_from(slot).map_err(|_| Escape::Broken(Internal::StackIsWrong))?;
            self.write_binding(made, slot, callee)?;
        }

        let locals_at = height;
        let base = locals_at.saturating_add(locals);
        if base > bounds::VALUES_ON_THE_STACK {
            return Err(Escape::range_error(
                "this script needs more values at once than this engine will hold",
                at,
            ));
        }
        let stack = run.stack;
        self.objects
            .with_slots(stack, |slots, _| slots.grow_to(base))
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;

        run.frames.push(Frame {
            unit: loaded,
            chunk,
            environment: Some(root),
            environments: 0,
            callee_at,
            this_at,
            locals_at,
            base,
            pc: 0,
            after,
        });
        Ok(())
    }

    /// `Op::Return`: the answer is on top of the stack.
    pub(super) fn give_back(&mut self, run: &mut Run) -> Result<(), Escape> {
        let value = self.pop(run)?;
        let Some(frame) = run.frames.pop() else {
            return Err(Escape::Broken(Internal::StackIsWrong));
        };
        if let Some(root) = frame.environment {
            // The environment is not freed here: a closure made inside this
            // call is holding it, and if none was, the next collection takes
            // it. What ends is *this frame's* claim on it.
            self.objects.heap_mut().release(root);
        }
        let stack = run.stack;
        if frame.after == After::Discard {
            // A setter's answer is not what the assignment evaluates to, and
            // the value it does evaluate to is already below.
            return self
                .objects
                .with_slots(stack, |slots, _| slots.truncate(frame.callee_at))
                .ok_or(Escape::Broken(Internal::StackIsWrong));
        }
        // No allocation between taking the value off and putting it back, so
        // nothing can collect while it is in a Rust local — which is the one
        // place in this engine where that argument has to be made rather than
        // avoided.
        self.objects
            .with_slots(stack, |slots, _| {
                slots.truncate(frame.callee_at);
                slots.push(value);
            })
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        // Everything below reads the answer off the stack rather than out of a
        // local, because both of them allocate.
        match frame.after {
            After::Answer | After::Discard => Ok(()),
            After::TypeOf => {
                let value = self.peek(run, 0)?;
                let answer = self.type_of(value, Self::where_now(run))?;
                self.replace(run, 1, answer)
            }
            After::Convert(state) => self.carry_on(run, state),
        }
    }

    /// Give back every frame's environment, whatever ended the run.
    ///
    /// A script that threw leaves its frames where they were, and a root
    /// nobody released would keep an environment — and everything it reaches —
    /// alive for the life of the engine.
    pub(super) fn let_go(&mut self, run: &mut Run) {
        while let Some(frame) = run.frames.pop() {
            if let Some(root) = frame.environment {
                self.objects.heap_mut().release(root);
            }
        }
    }

    /// Put a value in a binding, which is this engine's own mistake if there is
    /// no such binding: the compiler counted them.
    fn write_binding(&mut self, environment: Ref, at: usize, value: Value) -> Result<(), Escape> {
        self.objects
            .with_environment(environment, |environment, barrier| {
                environment.set(barrier, at, value)
            })
            .filter(|wrote| *wrote)
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        Ok(())
    }

    /// What a function's code is, or [`None`] if the reference is not one.
    fn code_of(&self, held: Ref) -> Option<Code> {
        let function = self.objects.callable(held)?;
        Some((
            Rc::clone(function.unit()),
            function.chunk(),
            function.environment(),
            function.captured(),
        ))
    }

    /// `OrdinaryCallBindThis` for a function that has a `this` of its own.
    fn receiver(&self, run: &Run, at: usize, strict: bool) -> Result<Value, Escape> {
        let given = self.value_at(run, at)?;
        if strict {
            return Ok(given);
        }
        match given {
            Value::Undefined | Value::Null => Ok(Value::Object(self.realm.global(&self.objects)?)),
            Value::Object(_) => Ok(given),
            // `ToObject` on a primitive, which is a wrapper object and is queue
            // item 73. Saying so is the honest answer; passing the primitive
            // through would make `this.length` inside a sloppy method quietly
            // wrong.
            Value::Bool(_) | Value::Number(_) | Value::Text(_) | Value::Symbol(_) => {
                Err(Escape::NotBuiltYet(Missing::AWrapperObject))
            }
        }
    }

    /// Make this program's constants and intern its keys, once per run.
    fn load_program(&mut self, run: &mut Run, unit: &Rc<Unit>) -> Result<usize, Escape> {
        if let Some(at) = run.already_loaded(unit) {
            return Ok(at);
        }
        let constants = run.constants;
        let offset = self
            .objects
            .slot_count(constants)
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        let keys = self.intern(unit, constants)?;
        run.units.push(Loaded {
            unit: Rc::clone(unit),
            offset,
            keys,
        });
        Ok(run.units.len().saturating_sub(1))
    }

    /// The `TypeError` for calling something that is not a function, which is
    /// the page's own mistake and one of the commonest there is.
    pub(super) fn not_a_function(&self, value: Value, at: usize) -> Escape {
        Escape::type_error(format!("{} is not a function", self.describe(value)), at)
    }
}

/// A call an instruction wants, as one thing rather than five arguments.
#[derive(Debug, Clone, Copy)]
pub(super) struct Ask<'a> {
    /// What to call.
    pub(super) callee: Value,
    /// Its `this`, which for a getter, a setter and a `valueOf` is the object
    /// the property was reached through.
    pub(super) receiver: Value,
    /// Its arguments: none for a getter, the value for a setter.
    pub(super) arguments: &'a [Value],
    /// The byte offset the instruction came from, for a message.
    pub(super) at: usize,
    /// What its answer is for.
    pub(super) after: After,
}

/// A function's code, read off its cell so that the borrow of the heap is over
/// before the call allocates anything.
type Code = (Rc<Unit>, u32, Option<Ref>, Option<Value>);

/// What a call needs to know about the chunk it is entering.
fn shape_of(unit: &Unit, chunk: u32) -> Option<(usize, usize, usize, bool, Option<u32>)> {
    let chunk = unit.chunk(chunk)?;
    Some((
        chunk.parameters(),
        chunk.bindings(),
        chunk.locals(),
        chunk.strict(),
        chunk.own_slot(),
    ))
}
