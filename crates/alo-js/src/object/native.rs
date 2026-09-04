/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A function whose body is Rust: what a builtin *is* (queue item 218).
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
//! # A native does not get the interpreter, and that is the bound
//!
//! [`Call`] is what a builtin is handed: the heap, its `this`, its arguments
//! and where in the source it was called from. There is no engine in it and no
//! stack, so **a native cannot call back into the script** — which is why
//! `Function.prototype.call`, `Array.prototype.map` and a `ToPrimitive` on an
//! argument are queue item 219 rather than something this file quietly allows.
//! A builtin that needs one says so with
//! [`Missing::AConversionInsideABuiltin`](crate::abrupt::Missing), because a
//! builtin that guessed instead would be a wrong answer wearing a right one's
//! clothes.
//!
//! The other half of that bound is what makes a native call cheap: it needs no
//! frame, cannot recurse, and returns before the instruction that wanted it
//! carries on.
//!
//! # What a native may keep across an allocation
//!
//! The same thing everything else may: what is in a [`Scope`](crate::heap::Scope)
//! or a [`Root`](crate::heap::Root). A builtin's `this` and its arguments are
//! still on the interpreter's stack while it runs — the call is not taken down
//! until the answer exists — so they are walked by the collector without the
//! builtin doing anything. Anything a builtin *makes* and means to keep past a
//! second allocation is its own to hold.

use crate::abrupt::Escape;

use super::{Objects, Value};

/// The body of a builtin: Rust, given a [`Call`], answering a value.
pub type Body = fn(&mut Call<'_>) -> Result<Value, Escape>;

/// A function this engine wrote.
#[derive(Debug, Clone, Copy)]
pub struct Native {
    name: &'static str,
    body: Body,
}

impl Native {
    /// A native of this name and this body.
    ///
    /// The name is for a message a person reads, and is **not** the `name`
    /// property a page can see — a function's own `name` and `length` are queue
    /// item 220, and giving one of them a value here would be inventing the
    /// other.
    pub const fn new(name: &'static str, body: Body) -> Self {
        Self { name, body }
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
    use super::{Call, Native};
    use crate::abrupt::Escape;
    use crate::object::{Objects, Value};

    #[expect(
        clippy::unnecessary_wraps,
        reason = "the signature is `Body`, which every builtin shares"
    )]
    fn first(call: &mut Call<'_>) -> Result<Value, Escape> {
        Ok(call.argument(0))
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
        assert_eq!(native.body()(&mut call), Ok(Value::Number(1.0)));
    }

    #[test]
    fn an_argument_nobody_passed_is_undefined_rather_than_a_refusal() {
        let mut objects = Objects::new();
        let mut call = Call::new(&mut objects, Value::Undefined, &[], 0);
        assert_eq!(call.argument(0), Value::Undefined);
        assert_eq!(call.argument(9), Value::Undefined);
        assert_eq!(first(&mut call), Ok(Value::Undefined));
    }
}
