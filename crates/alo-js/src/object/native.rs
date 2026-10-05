/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A function whose body is Rust: what a builtin *is* (queue items 218 and 219).
//!
//! Item 209 made a function a value and gave it a chunk of compiled code.
//! `Object.prototype.toString` has no chunk and never will — there is no
//! script it was compiled from — so this is the other kind of body a function
//! object may have. Everything else about it is unchanged: it is the same cell,
//! `typeof` answers `"function"`, and a page may hang properties off it.
//!
//! # It is a function pointer rather than a boxed closure
//!
//! A `Box<dyn Fn>` would let a builtin capture state, and every piece of state
//! a builtin could capture is either a reference — which the collector must
//! walk and a boxed closure hides from it — or a realm, which is the thing a
//! builtin is reached *through*. A plain `fn` can capture neither, so a native
//! function holds no edge at all and tracing one is nothing. That is worth more
//! than the convenience.
//!
//! # A native still does not get the interpreter — it *asks*
//!
//! [`Call`] is what a builtin is handed: the heap, its `this`, its arguments
//! and where in the source it was called from. There is no engine in it and no
//! stack, and item 219 did not put one there. What it added instead is
//! [`Answer::Want`]: a builtin that needs the script run — `f.call(o)`,
//! `toLocaleString` invoking `this.toString()`, a `ToPrimitive` on an argument —
//! **returns saying what it wants and which step to come back at**, and the
//! interpreter lays that call out on its own stack like any other.
//!
//! That keeps the bound the item was built around. A builtin never nests a Rust
//! call inside a Rust call, so a builtin that asks for itself for ever grows the
//! interpreter's list of calls rather than this process's stack, and is the
//! `RangeError` a runaway function is.
//!
//! # A step is a number, and the answer arrives on the stack
//!
//! A suspended builtin keeps no state of its own beyond a `u32`: everything
//! else it needs is its `this` and its arguments, which are still where the
//! caller put them. When the call it asked for finishes, its answer is written
//! **into the slot just above its arguments** and it runs again from the step
//! it named, with [`Call::answer`] reading that slot. So a builtin holds no
//! reference across a suspension that the collector cannot see — the same rule
//! as everywhere else, kept by there being nothing to hold it in.
//!
//! # What a native may keep across an allocation
//!
//! The same thing everything else may: what is in a [`Scope`](crate::heap::Scope)
//! or a [`Root`](crate::heap::Root). A builtin's `this` and its arguments are
//! still on the interpreter's stack while it runs — the call is not taken down
//! until the answer exists — so they are walked by the collector without the
//! builtin doing anything. Anything a builtin *makes* and means to keep past a
//! second allocation is its own to hold.
//!
//! The one new place that rule bites is [`Want::Call`]'s argument list, which is
//! a `Vec` in a Rust local until the interpreter pushes it: **nothing may
//! allocate between building it and returning it.** Every builtin here builds it
//! last, out of values that are already on the stack.

use crate::abrupt::Escape;
use crate::convert::Hint;

use super::{Objects, Value};

/// The body of a builtin: Rust, given a [`Call`], answering an [`Answer`].
pub type Body = fn(&mut Call<'_>) -> Result<Answer, Escape>;

/// What a builtin's body says when it returns.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// It is finished, and this is what it evaluates to.
    Value(Value),
    /// It cannot finish yet: the interpreter is to do this first and run the
    /// body again.
    Want {
        /// What it wants done.
        want: Want,
        /// Which step it runs again at, where [`Call::answer`] is what the
        /// interpreter did. Never zero, which is the step a fresh call starts
        /// at — a builtin that named zero would loop.
        step: u32,
    },
}

impl Answer {
    /// Ask for `want`, and come back at `step`.
    pub const fn want(want: Want, step: u32) -> Self {
        Self::Want { want, step }
    }
}

/// What a builtin may ask the interpreter to do for it.
///
/// Two rather than one because a conversion is not a call: turning an object
/// into a primitive is `OrdinaryToPrimitive`, which is a search over two names
/// and may be two calls or none. That algorithm lives in
/// [`convert`](crate::convert) and the interpreter drives it; a builtin
/// spelling it out again would be a second copy of a rule that has to agree
/// with the first.
#[derive(Debug, Clone, PartialEq)]
pub enum Want {
    /// Call `callee` with `receiver` as its `this`, and answer with what it
    /// returned.
    Call {
        /// What to call. Not being callable is the `TypeError` any other call
        /// on a non-function is, produced in the one place that produces it.
        callee: Value,
        /// Its `this`.
        receiver: Value,
        /// Its arguments, in order.
        arguments: Vec<Value>,
    },
    /// `ToPrimitive(of, hint)`, and answer with the primitive.
    ///
    /// `of` must be an object: every other value is already a primitive and
    /// [`Primitive::of`](crate::convert::Primitive::of) answers it without
    /// anything being run.
    Primitive {
        /// The object to convert.
        of: Value,
        /// Which primitive is wanted.
        hint: Hint,
    },
}

/// What a builtin constructor is given before its body runs (queue item 227).
///
/// A constructor this engine wrote is told its instance rather than making it,
/// because a body that made its own would have nowhere to keep it while it
/// asked the script for something: a native keeps a step number and nothing
/// else. So the interpreter does `OrdinaryCreateFromConstructor` — reading the
/// constructor's own `prototype`, which on a builtin is fixed — and puts the
/// instance in the `this` slot, which the collector walks and the body reads
/// with [`Call::this`], exactly as a script's constructor gets its instance
/// (queue item 212). This says which kind of object that is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instance {
    /// An object with the `[[ErrorData]]` slot. The `Error` constructors make
    /// one whether they are called or constructed, which is the
    /// specification's: `TypeError('x')` and `new TypeError('x')` are the same
    /// object.
    Error,
}

/// A function this engine wrote.
#[derive(Debug, Clone, Copy)]
pub struct Native {
    name: &'static str,
    body: Body,
    instance: Option<Instance>,
}

impl Native {
    /// A native of this name and this body.
    ///
    /// The name is for a message a person reads, and is **not** the `name`
    /// property a page can see — a function's own `name` and `length` are queue
    /// item 220, and giving one of them a value here would be inventing the
    /// other.
    pub const fn new(name: &'static str, body: Body) -> Self {
        Self {
            name,
            body,
            instance: None,
        }
    }

    /// A native that is also a constructor, and is given an instance of this
    /// kind in its `this` slot before its body runs (queue item 227).
    pub const fn constructor(name: &'static str, body: Body, instance: Instance) -> Self {
        Self {
            name,
            body,
            instance: Some(instance),
        }
    }

    /// The instance it is given, which is [`Some`] exactly when it has a
    /// `[[Construct]]`.
    pub const fn instance(&self) -> Option<Instance> {
        self.instance
    }

    /// What it is called, for a message.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Its body.
    pub const fn body(&self) -> Body {
        self.body
    }
}

/// What a builtin is given when it is called.
#[derive(Debug)]
pub struct Call<'a> {
    objects: &'a mut Objects,
    this: Value,
    arguments: &'a [Value],
    at: usize,
    step: u32,
    answer: Option<Value>,
}

impl<'a> Call<'a> {
    /// What the interpreter hands over.
    pub const fn new(
        objects: &'a mut Objects,
        this: Value,
        arguments: &'a [Value],
        at: usize,
    ) -> Self {
        Self {
            objects,
            this,
            arguments,
            at,
            step: 0,
            answer: None,
        }
    }

    /// Say that this is a builtin being run again, at `step`, and that `answer`
    /// is what the interpreter did for it.
    pub const fn resume(&mut self, step: u32, answer: Value) {
        self.step = step;
        self.answer = Some(answer);
    }

    /// Which step this is: zero the first time, and afterwards whatever the
    /// body named when it asked for something.
    pub const fn step(&self) -> u32 {
        self.step
    }

    /// What the interpreter did for it.
    ///
    /// # Errors
    ///
    /// [`Internal::BuiltinIsWrong`](crate::abrupt::Internal) when the body asks
    /// at step zero, where nothing has been done for it yet. That is this
    /// engine's own mistake rather than a page's, so it is reported as one.
    pub const fn answer(&self) -> Result<Value, Escape> {
        match self.answer {
            Some(value) => Ok(value),
            None => Err(Escape::Broken(crate::abrupt::Internal::BuiltinIsWrong)),
        }
    }

    /// The heap, to read a property or to make a string.
    pub const fn objects(&mut self) -> &mut Objects {
        self.objects
    }

    /// The same, when nothing is being changed.
    pub const fn seen(&self) -> &Objects {
        self.objects
    }

    /// The `this` it was called with.
    ///
    /// Whatever the caller wrote, unchanged: a builtin is strict code, so
    /// `OrdinaryCallBindThis` does not turn `undefined` into the global object
    /// here. Every builtin that cares says what it does with a primitive.
    pub const fn this(&self) -> Value {
        self.this
    }

    /// The argument at `which`, which is `undefined` past the end.
    ///
    /// The language has no missing argument: `f()` and `f(undefined)` are the
    /// same call to the callee, so this answers rather than refuses.
    pub fn argument(&self, which: usize) -> Value {
        self.arguments
            .get(which)
            .copied()
            .unwrap_or(Value::Undefined)
    }

    /// How many arguments were actually passed.
    pub const fn count(&self) -> usize {
        self.arguments.len()
    }

    /// The byte offset in the source the call came from, for a message.
    pub const fn at(&self) -> usize {
        self.at
    }
}

#[cfg(test)]
mod tests {
    use super::{Answer, Call, Native, Want};
    use crate::abrupt::{Escape, Internal};
    use crate::convert::Hint;
    use crate::object::{Objects, Value};

    #[expect(
        clippy::unnecessary_wraps,
        reason = "the signature is `Body`, which every builtin shares"
    )]
    fn first(call: &mut Call<'_>) -> Result<Answer, Escape> {
        Ok(Answer::Value(call.argument(0)))
    }

    /// A builtin of the shape item 219 is for: it wants its argument converted,
    /// and answers with what the conversion produced.
    fn converted(call: &mut Call<'_>) -> Result<Answer, Escape> {
        if call.step() == 0 {
            return Ok(Answer::want(
                Want::Primitive {
                    of: call.argument(0),
                    hint: Hint::String,
                },
                1,
            ));
        }
        Ok(Answer::Value(call.answer()?))
    }

    #[test]
    fn a_native_holds_no_edge_and_answers_what_it_was_given() {
        let mut objects = Objects::new();
        let native = Native::new("first", first);
        assert_eq!(native.name(), "first");
        let arguments = [Value::Number(1.0)];
        let mut call = Call::new(&mut objects, Value::Null, &arguments, 7);
        assert_eq!(call.this(), Value::Null);
        assert_eq!(call.count(), 1);
        assert_eq!(call.at(), 7);
        assert_eq!(call.step(), 0);
        assert_eq!(
            native.body()(&mut call),
            Ok(Answer::Value(Value::Number(1.0)))
        );
    }

    #[test]
    fn an_argument_nobody_passed_is_undefined_rather_than_a_refusal() {
        let mut objects = Objects::new();
        let mut call = Call::new(&mut objects, Value::Undefined, &[], 0);
        assert_eq!(call.argument(0), Value::Undefined);
        assert_eq!(call.argument(9), Value::Undefined);
        assert_eq!(first(&mut call), Ok(Answer::Value(Value::Undefined)));
    }

    #[test]
    fn a_builtin_that_wants_something_names_the_step_it_comes_back_at() {
        let mut objects = Objects::new();
        let Ok(held) = objects.object(None) else {
            panic!("an empty heap holds an object");
        };
        let arguments = [Value::Object(held)];
        let mut call = Call::new(&mut objects, Value::Undefined, &arguments, 0);
        assert_eq!(
            converted(&mut call),
            Ok(Answer::want(
                Want::Primitive {
                    of: Value::Object(held),
                    hint: Hint::String,
                },
                1
            ))
        );
    }

    #[test]
    fn a_resumed_builtin_reads_the_answer_and_one_that_was_not_says_so() {
        let mut objects = Objects::new();
        // Asking for an answer at step zero is this engine resuming a builtin
        // that never suspended, which is our bug rather than a page's.
        let fresh = Call::new(&mut objects, Value::Undefined, &[], 0);
        assert_eq!(
            fresh.answer(),
            Err(Escape::Broken(Internal::BuiltinIsWrong)),
            "nothing has been done for a builtin on its first step"
        );

        let mut again = Call::new(&mut objects, Value::Undefined, &[], 0);
        again.resume(1, Value::Number(4.0));
        assert_eq!(again.step(), 1);
        assert_eq!(again.answer(), Ok(Value::Number(4.0)));
        assert_eq!(converted(&mut again), Ok(Answer::Value(Value::Number(4.0))));
    }
}
