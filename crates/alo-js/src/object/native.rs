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
//! # A native that makes an object is told the realm's prototypes
//!
//! `[].values()` makes an iterator that inherits from
//! `%ArrayIteratorPrototype%`, and the result of its `next` inherits from
//! `Object.prototype` (queue item 230). Neither is reachable from `this` by
//! anything a page could not change, so the interpreter hands a builtin the
//! realm's [`Intrinsics`] beside the heap ([`Call::intrinsics`]). They are
//! read, never written: an intrinsic is the realm's, and a builtin that could
//! replace one could change what every later `[]` is.
//!
//! # And an embedder's native is told the realm's host
//!
//! A native reaches its page through its `this` (ADR 0017 § 4) — except when
//! its `this` is an object the engine has only just made, which reaches
//! nothing: a constructor's instance. So a native is also handed the realm's
//! `[[HostDefined]]` ([`Call::host_defined`], ADR 0019 § 1), the one
//! reference its embedder set, of a type this engine never learns. A builtin
//! of the engine's own never asks for it.
//!
//! # A native that reads the time is told the realm's clock
//!
//! `Date.now()` and `new Date()` ask what time it is, and the engine has no
//! clock of its own (ADR 0013 § 5): the embedder hands a realm one, or none
//! (ADR 0036 § 1). The interpreter hands it on to every builtin
//! ([`Call::now`]), which answers a time value or, in a realm with no clock,
//! the `TypeError` that says so.
//!
//! # A native that runs a long loop is told the embedder's stop
//!
//! The interpreter asks [`Stop`](crate::interpret::Stop) on every backward
//! jump and every call, which bounds every loop a script writes. A builtin's
//! own loop is not a jump the interpreter sees, and one of them is long
//! enough to matter: a regular expression's matcher (ADR 0029 § 3). So a
//! native is handed the switch ([`Call::stop_asked`]) and a builtin whose
//! work a page can make large asks it inside.
//!
//! # A step says where, the slots say what, and both are on the stack
//!
//! A suspended builtin's place in its own body is a `u32` step. What it *had*
//! there — `forEach`'s `len` and `k`, `map`'s array — is in **slots** it
//! declares when it is made ([`Native::keeping`], ADR 0031): that many values,
//! at most [`bounds::KEPT_BY_A_BUILTIN`](crate::bounds), reserved as
//! `undefined` on the interpreter's stack directly above its arguments when it
//! is entered and taken down with the call. Its region is
//! `callee | this | args | kept | answer`. When the call it asked for finishes,
//! the answer is written **into the slot just above its kept ones** and it
//! runs again from the step it named, with [`Call::answer`] reading that slot.
//!
//! [`Call::keep`] writes a slot on the stack at once, through its barrier, and
//! [`Call::kept`] reads it from there; neither copies the slots into the
//! `Call`. So a builtin holds no reference across a suspension — or across an
//! allocation — that the collector cannot see: a value is rooted the moment it
//! is kept. A builtin that declares none has the region it always had, and
//! keeps nothing but its step.
//!
//! The step still says only *where*. A small value the builtin itself bounds,
//! such as which of `addEventListener`'s options it has read, may ride in it;
//! nothing a page can make as large as it likes may (ADR 0031 § 5).
//!
//! # A native that is a closure is told what it was made around
//!
//! A promise's `resolve` is a function the specification makes per promise,
//! closing over it (queue item 333). A function pointer closes over nothing,
//! so the function object holds that one value
//! ([`Function::held`](super::Function::held)) and the interpreter hands it
//! over with every call ([`Call::held`]). The callee is on the stack for the
//! whole call, so what it holds is rooted for as long as the body runs.
//!
//! # What a native may keep across an allocation
//!
//! The same thing everything else may: what is in a [`Scope`](crate::heap::Scope),
//! a [`Root`](crate::heap::Root) or one of its slots. A builtin's `this` and its
//! arguments are still on the interpreter's stack while it runs — the call is
//! not taken down until the answer exists — so they are walked by the collector
//! without the builtin doing anything. Anything a builtin *makes* and means to
//! keep past a second allocation it keeps in a slot before that allocation.
//!
//! The one new place that rule bites is [`Want::Call`]'s argument list, which is
//! a `Vec` in a Rust local until the interpreter pushes it: **nothing may
//! allocate between building it and returning it.** Every builtin here builds it
//! last, out of values that are already on the stack.

use crate::abrupt::{Escape, Internal};
use crate::builtin::Intrinsics;
use crate::clock::Clock;
use crate::convert::Hint;
use crate::heap::Ref;
use crate::interpret::Stop;

use super::{Held, Objects, Value};

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
/// A call and a conversion are two rather than one because a conversion is not
/// a call: turning an object into a primitive is `OrdinaryToPrimitive`, which
/// is a search over two names and may be two calls or none. That algorithm
/// lives in [`convert`](crate::convert) and the interpreter drives it; a
/// builtin spelling it out again would be a second copy of a rule that has to
/// agree with the first — and so is its ordinary half alone, which a
/// `Symbol.toPrimitive` method ends with. A job is the third, and runs nothing now at all. A
/// reported call is the fourth: a call, but one whose throw the builtin never
/// sees (ADR 0018 § 3). A caught call is the fifth: a call whose throw the
/// builtin is handed as its answer. Settling a promise is the sixth, because
/// what settling does — queue a job per reaction, and remember a rejection
/// nobody handled — is the engine's (queue item 333).
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
    /// `OrdinaryToPrimitive(of, hint)`, and answer with the primitive: the
    /// search over `valueOf` and `toString` alone, without asking for
    /// `Symbol.toPrimitive` first — which is what a `Symbol.toPrimitive`
    /// method's own last step is, and asking for it again would never end
    /// (queue item 356). `hint` is [`Hint::Number`] or [`Hint::String`].
    Ordinary {
        /// The object to convert.
        of: Value,
        /// Which primitive is tried for first.
        hint: Hint,
    },
    /// Queue a call of `callee` with `arguments` as a job, to run at the next
    /// microtask checkpoint, and answer `undefined` (queue item 232).
    ///
    /// This is how an embedder's `queueMicrotask` reaches the engine's job
    /// queue (ADR 0016 § 1): a builtin is handed no engine, so it asks, as it
    /// does for a call. A `callee` that is not callable is the `TypeError`
    /// `queueMicrotask` throws, thrown where the builtin was called rather
    /// than later, when the job would have run.
    Job {
        /// What to call when the job runs.
        callee: Value,
        /// Its arguments, in order. Its `this` is `undefined`.
        arguments: Vec<Value>,
    },
    /// Call `callee` as [`Want::Call`] does, but **report** a throw rather
    /// than propagate it (ADR 0018 § 3): the throw is set aside for the
    /// embedder exactly as a job's is, and the builtin is answered
    /// `undefined` with [`Call::reported`] true.
    ///
    /// HTML's *report the exception* inside a builtin — what
    /// `dispatchEvent` does with a listener that throws, and what a promise
    /// reaction or a registry's cleanup will ask for. Only the page's own
    /// escapes are reported: a stop, a full heap or this engine's bug still
    /// ends the run, as it would have ended the job.
    Report {
        /// What to call. Not being callable is the `TypeError` any call of a
        /// non-function is, and it is reported like any other throw.
        callee: Value,
        /// Its `this`.
        receiver: Value,
        /// Its arguments, in order.
        arguments: Vec<Value>,
    },
    /// Call `callee` as [`Want::Call`] does, but hand a throw nothing inside
    /// the call catches **back to the builtin** as its answer, with
    /// [`Call::threw`] true (queue item 333).
    ///
    /// The specification's `Completion(Call(…))` followed by *if it is an
    /// abrupt completion*: a promise's executor that throws rejects the
    /// promise, and so does a reaction whose handler throws, rather than
    /// either throw reaching the caller. An error the engine threw arrives as
    /// the object a `catch` would have bound. Only the page's own escapes are
    /// caught, as a `catch`'s are: a stop, a full heap or this engine's bug
    /// still ends the run.
    Catch {
        /// What to call. Not being callable is the `TypeError` any call of a
        /// non-function is, and it is caught like any other throw.
        callee: Value,
        /// Its `this`.
        receiver: Value,
        /// Its arguments, in order.
        arguments: Vec<Value>,
    },
    /// Settle `promise` with `value`, queue a job for each reaction that was
    /// waiting on it, and answer `undefined`: `FulfillPromise` or
    /// `RejectPromise` (queue item 333).
    ///
    /// A rejection with nothing handling it is remembered by the engine and,
    /// if nothing has handled it by the end of the checkpoint, reported
    /// (`HostPromiseRejectionTracker`). A `promise` that is not one, or has
    /// settled already, is this engine's bug: a promise's resolving functions
    /// settle it at most once.
    Settle {
        /// The promise to settle.
        promise: Value,
        /// Whether it is fulfilled, rather than rejected.
        fulfilled: bool,
        /// The value it is fulfilled with, or the reason it is rejected with.
        value: Value,
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
#[derive(Debug, Clone, Copy)]
pub enum Instance {
    /// An object with the `[[ErrorData]]` slot. The `Error` constructors make
    /// one whether they are called or constructed, which is the
    /// specification's: `TypeError('x')` and `new TypeError('x')` are the same
    /// object.
    Error,
    /// An embedder's object, made by this function from the constructor's
    /// `prototype` — **only when constructed**. A Web IDL constructor called
    /// without `new` is a `TypeError`, which its body throws on seeing
    /// [`Call::constructing`] false, and an instance made for a call that
    /// throws would be garbage before it was seen.
    ///
    /// A function pointer for [`Native`]'s reason: it holds no edge. The
    /// instance is made before the body runs, in the `this` slot the collector
    /// walks, so a body that suspends to convert an argument keeps what it
    /// has converted *in its instance*, which is where the standard keeps it
    /// (ADR 0031 § 6).
    Made(Make),
    /// A pending promise with nothing waiting on it (queue item 333) — **only
    /// when constructed**: `Promise()` without `new` is a `TypeError`, which
    /// its body throws on seeing [`Call::constructing`] false.
    Promise,
    /// An Invalid Date, whose time value the body sets (queue item 356) —
    /// **only when constructed**: `Date()` without `new` answers a string and
    /// makes no object, which its body does on seeing [`Call::constructing`]
    /// false.
    Date,
}

/// How an embedder's constructor makes its instance: from the prototype it
/// inherits from, the object, not yet in the heap.
pub type Make = fn(Option<Ref>) -> Box<dyn super::Exotic>;

impl Instance {
    /// Whether a plain call, without `new`, is given one too.
    pub const fn when_called(self) -> bool {
        matches!(self, Self::Error)
    }
}

/// A function this engine wrote.
#[derive(Debug, Clone, Copy)]
pub struct Native {
    name: &'static str,
    body: Body,
    instance: Option<Instance>,
    kept: usize,
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
            kept: 0,
        }
    }

    /// A native that is also a constructor, and is given an instance of this
    /// kind in its `this` slot before its body runs (queue item 227).
    pub const fn constructor(name: &'static str, body: Body, instance: Instance) -> Self {
        Self {
            name,
            body,
            instance: Some(instance),
            kept: 0,
        }
    }

    /// The same native, keeping `slots` values on the stack across the calls
    /// it asks for, read with [`Call::kept`] and written with [`Call::keep`]
    /// (ADR 0031).
    ///
    /// The count is the builtin's own and never its input's. More than
    /// [`bounds::KEPT_BY_A_BUILTIN`](crate::bounds) is refused when its
    /// function is made ([`Refused::KeepsTooMuch`](crate::object::Refused)),
    /// which is when a realm is furnished.
    #[must_use]
    pub const fn keeping(mut self, slots: usize) -> Self {
        self.kept = slots;
        self
    }

    /// How many values it keeps: zero unless it said otherwise.
    pub const fn kept(&self) -> usize {
        self.kept
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

/// Where a builtin's slots are: which list, from where, and how many.
#[derive(Debug, Clone, Copy)]
struct Kept {
    stack: Ref,
    at: usize,
    count: usize,
}

/// What a builtin is given when it is called.
#[derive(Debug)]
pub struct Call<'a> {
    objects: &'a mut Objects,
    intrinsics: Option<&'a Intrinsics>,
    host: Option<Ref>,
    stop: Option<&'a Stop>,
    clock: Option<&'a dyn Clock>,
    held: Value,
    this: Value,
    arguments: &'a [Value],
    slots: Option<Kept>,
    at: usize,
    step: u32,
    answer: Option<Value>,
    reported: bool,
    threw: bool,
    constructing: bool,
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
            intrinsics: None,
            host: None,
            stop: None,
            clock: None,
            held: Value::Undefined,
            this,
            arguments,
            slots: None,
            at,
            step: 0,
            answer: None,
            reported: false,
            threw: false,
            constructing: false,
        }
    }

    /// The same call, of a function made around `held`.
    #[must_use]
    pub const fn holding(mut self, held: Value) -> Self {
        self.held = held;
        self
    }

    /// What the function being called was made around
    /// ([`Objects::native_holding`](super::Objects::native_holding)), or
    /// `undefined` for one made around nothing.
    pub const fn held(&self) -> Value {
        self.held
    }

    /// The same call, made by `new` rather than called.
    #[must_use]
    pub const fn constructed(mut self) -> Self {
        self.constructing = true;
        self
    }

    /// Whether it was made by `new` — Web IDL's *`NewTarget` is not
    /// `undefined`*. A constructor that is [`Instance::Made`] finds its
    /// instance in [`Call::this`] exactly when this is true.
    pub const fn constructing(&self) -> bool {
        self.constructing
    }

    /// The same call, in a realm whose intrinsics it may read.
    #[must_use]
    pub const fn within(mut self, intrinsics: &'a Intrinsics) -> Self {
        self.intrinsics = Some(intrinsics);
        self
    }

    /// The realm's intrinsics, for a builtin that makes an object.
    ///
    /// # Errors
    ///
    /// [`Internal::BuiltinIsWrong`] for a call made without them, which only
    /// a test that built a [`Call`] by hand does: the interpreter always
    /// passes them.
    pub const fn intrinsics(&self) -> Result<&'a Intrinsics, Escape> {
        match self.intrinsics {
            Some(intrinsics) => Ok(intrinsics),
            None => Err(Escape::Broken(Internal::BuiltinIsWrong)),
        }
    }

    /// The same call, keeping `count` slots in `stack` from `at` up.
    ///
    /// Only the interpreter says this, having reserved them; a [`Call`] built
    /// by hand keeps nothing, and reaching for a slot in one is
    /// [`Internal::BuiltinIsWrong`].
    #[must_use]
    pub(crate) const fn keeping(mut self, stack: Ref, at: usize, count: usize) -> Self {
        self.slots = Some(Kept { stack, at, count });
        self
    }

    /// What slot `which` holds: `undefined` until the builtin keeps something
    /// there (ADR 0031 § 4).
    ///
    /// Read from the stack each time rather than from a copy, so it is what
    /// was last kept even after a collection.
    ///
    /// # Errors
    ///
    /// [`Internal::BuiltinIsWrong`] for a slot at or past the number the
    /// builtin declared, which is its author's mistake rather than a page's.
    pub fn kept(&self, which: usize) -> Result<Value, Escape> {
        let at = self.slot(which)?;
        let stack = self.slots.map(|kept| kept.stack);
        match stack.and_then(|stack| self.objects.slot(stack, at)) {
            Some(Held::Value(value)) => Ok(value),
            Some(Held::Uninitialized) | None => Err(Escape::Broken(Internal::StackIsWrong)),
        }
    }

    /// Keep `value` in slot `which`, written to the stack at once through its
    /// barrier — so it is rooted from this moment, before anything the body
    /// does next allocates (ADR 0031 § 4).
    ///
    /// # Errors
    ///
    /// [`Internal::BuiltinIsWrong`] for a slot at or past the number the
    /// builtin declared.
    pub fn keep(&mut self, which: usize, value: Value) -> Result<(), Escape> {
        let at = self.slot(which)?;
        let stack = self
            .slots
            .map(|kept| kept.stack)
            .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
        self.objects
            .with_slots(stack, |slots, barrier| slots.set(barrier, at, value))
            .filter(|wrote| *wrote)
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        Ok(())
    }

    /// The number slot `which` holds — an index or a length the builtin kept
    /// there, which an `f64` holds exactly up to 2⁵³ − 1 (ADR 0031 § 3).
    ///
    /// # Errors
    ///
    /// [`Internal::BuiltinIsWrong`] for a slot it never declared, or one that
    /// holds anything but a number: a builtin that kept an index and reads
    /// back something else has a bug of its own.
    pub fn kept_number(&self, which: usize) -> Result<f64, Escape> {
        match self.kept(which)? {
            Value::Number(number) => Ok(number),
            _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
        }
    }

    /// Where slot `which` is on the stack, if the builtin declared it.
    fn slot(&self, which: usize) -> Result<usize, Escape> {
        match self.slots {
            Some(kept) if which < kept.count => Ok(kept.at.saturating_add(which)),
            _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
        }
    }

    /// The same call, under the embedder's switch for stopping a script.
    #[must_use]
    pub const fn stopped_by(mut self, stop: &'a Stop) -> Self {
        self.stop = Some(stop);
        self
    }

    /// Whether the embedder has asked for the script to stop. A builtin that
    /// loops over work a page chose asks this inside the loop, and answers
    /// [`Escape::Interrupted`] when it is true. A call made without the switch
    /// — only a test that built a [`Call`] by hand — is never stopped.
    pub fn stop_asked(&self) -> bool {
        self.stop.is_some_and(Stop::asked)
    }

    /// The same call, in a realm whose time is told by `clock` — or by none
    /// (ADR 0036 § 1).
    #[must_use]
    pub const fn timed_by(mut self, clock: Option<&'a dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// The time value now, by the realm's clock, through `TimeClip`: an
    /// integral number of milliseconds in range, or `NaN` from a clock that
    /// cannot say (ADR 0036 § 1).
    ///
    /// # Errors
    ///
    /// A `TypeError` saying so when the realm was given no clock: a realm with
    /// none refuses to say what time it is, by name, rather than make an
    /// instant up — which a page could not tell from the truth.
    pub fn now(&self) -> Result<f64, Escape> {
        match self.clock {
            Some(clock) => Ok(crate::time::time_clip(clock.now())),
            None => Err(Escape::type_error(
                "this realm was given no clock, so it cannot say what time it is",
                self.at,
            )),
        }
    }

    /// The same call, in a realm whose `[[HostDefined]]` is `host`.
    #[must_use]
    pub const fn hosted_by(mut self, host: Option<Ref>) -> Self {
        self.host = host;
        self
    }

    /// The realm's `[[HostDefined]]`: the reference its embedder set, or
    /// [`None`] if it set none (ADR 0019 § 1).
    ///
    /// The realm roots it, so it names the same cell for the whole call. An
    /// embedder's native that finds [`None`] is running in a realm its
    /// embedder never finished making — the embedder's bug, not a page's.
    pub const fn host_defined(&self) -> Option<Ref> {
        self.host
    }

    /// Say that this is a builtin being run again, at `step`, and that `answer`
    /// is what the interpreter did for it.
    pub const fn resume(&mut self, step: u32, answer: Value) {
        self.step = step;
        self.answer = Some(answer);
    }

    /// Say that the call it asked for with [`Want::Report`] threw, and that
    /// the throw was reported: its answer is `undefined`.
    pub const fn was_reported(&mut self) {
        self.reported = true;
    }

    /// Whether the call it asked for threw and was reported rather than
    /// answering — always false for anything but [`Want::Report`].
    pub const fn reported(&self) -> bool {
        self.reported
    }

    /// Say that the call it asked for with [`Want::Catch`] threw, and that
    /// its answer is what was thrown.
    pub const fn was_caught(&mut self) {
        self.threw = true;
    }

    /// Whether the call it asked for threw, so that [`Call::answer`] is what
    /// it threw rather than what it returned — always false for anything but
    /// [`Want::Catch`].
    pub const fn threw(&self) -> bool {
        self.threw
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
            None => Err(Escape::Broken(Internal::BuiltinIsWrong)),
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

    #[test]
    fn a_native_keeps_nothing_unless_it_says_how_many() {
        let plain = Native::new("first", first);
        assert_eq!(plain.kept(), 0);
        assert_eq!(plain.keeping(3).kept(), 3);
        assert_eq!(Native::new("first", first).keeping(9).kept(), 9);
    }

    #[test]
    fn a_slot_is_read_and_written_on_the_list_itself() {
        let mut objects = Objects::new();
        let Ok(stack) = objects.slots() else {
            panic!("an empty heap holds a list");
        };
        // callee, this, one argument, then two kept slots as the interpreter
        // reserves them.
        let reserved = objects.with_slots(stack, |slots, _| {
            for value in [
                Value::Undefined,
                Value::Undefined,
                Value::Number(1.0),
                Value::Undefined,
                Value::Undefined,
            ] {
                slots.push(value);
            }
        });
        assert_eq!(reserved, Some(()));
        let arguments = [Value::Number(1.0)];
        let mut call =
            Call::new(&mut objects, Value::Undefined, &arguments, 0).keeping(stack, 3, 2);
        assert_eq!(
            call.kept(0),
            Ok(Value::Undefined),
            "a fresh slot is undefined"
        );
        assert_eq!(call.keep(1, Value::Number(5.0)), Ok(()));
        assert_eq!(call.kept(1), Ok(Value::Number(5.0)));
        assert_eq!(call.kept_number(1), Ok(5.0));
        assert_eq!(
            call.kept_number(0),
            Err(Escape::Broken(Internal::BuiltinIsWrong)),
            "undefined is not the number a builtin kept"
        );
        assert_eq!(
            call.keep(2, Value::Null),
            Err(Escape::Broken(Internal::BuiltinIsWrong)),
            "a slot past the count is never the answer slot"
        );
        assert_eq!(call.kept(2), Err(Escape::Broken(Internal::BuiltinIsWrong)));
        // The write went to the list, not to a copy in the call.
        assert!(matches!(
            objects.slot(stack, 4),
            Some(crate::object::Held::Value(Value::Number(5.0)))
        ));
        assert_eq!(objects.slot_count(stack), Some(5));
    }

    #[test]
    fn a_call_built_by_hand_keeps_nothing() {
        let mut objects = Objects::new();
        let mut call = Call::new(&mut objects, Value::Undefined, &[], 0);
        assert_eq!(call.kept(0), Err(Escape::Broken(Internal::BuiltinIsWrong)));
        assert_eq!(
            call.keep(0, Value::Null),
            Err(Escape::Broken(Internal::BuiltinIsWrong))
        );
    }
}
