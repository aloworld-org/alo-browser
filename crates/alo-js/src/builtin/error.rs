/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Error` and the six native errors: what a `catch` binds (queue item 227).
//!
//! Cut from item 73 because item 210 — `try`, `catch` and `finally` — waits on
//! exactly this, and item 210 is what the frozen service worker is refused at
//! today (byte 2853). A thrown `TypeError` has had a kind and a message since
//! item 72 and has not been a **value**; these are the objects it becomes.
//!
//! # Seven constructors, seven prototypes, one body
//!
//! `Error`, `EvalError`, `RangeError`, `ReferenceError`, `SyntaxError`,
//! `TypeError` and `URIError`. Each prototype has `constructor`, `name` and an
//! empty `message`, and the six native errors' prototypes inherit from
//! `Error.prototype` — which is where `toString` is, once — while their
//! **constructors** inherit from `Error` itself, which is the specification's
//! and is why `TypeError.__proto__ === Error`. Every constructor runs the same
//! body: what differs between them is the prototype their instance is made
//! from, and that is read off the constructor rather than passed in.
//!
//! # The instance is made before the body runs
//!
//! A native keeps a step number and nothing else across a call it asks for
//! (item 219), and this constructor may ask for two — a `toString` on the
//! message, a getter for `cause` — so an instance it made itself would be held
//! by nothing while the script ran. The interpreter makes it instead
//! ([`Instance::Error`]) and hands it over in the `this` slot, which the
//! collector walks, whether the constructor was called or constructed: the
//! specification makes those the same object.
//!
//! # What is not here
//!
//! - **`AggregateError`** reads an iterable of errors, which needs the
//!   iteration protocol (items 211 and 75). Queue item 229.
//! - **`Error.prototype.toString` with a `message` that needs a call** — a
//!   getter, or an object whose own `toString` must run — is refused by name
//!   ([`Missing::AMessageBehindACall`]): by then `name` has been read, and
//!   keeping it across the call is the traced scratch state item 221 builds.
//!   Reading `name` again afterwards would be a second call a page can count.
//!   Queue item 228.
//! - **`stack`, `captureStackTrace` and a `name`/`length` on the constructor**
//!   are items 78 and 220. A function has no own `name` or `length` here yet,
//!   and these are functions.

use crate::abrupt::{Escape, Internal, Missing};
use crate::convert::{self, Hint, Primitive};
use crate::heap::{Ref, Root};
use crate::object::native::{Answer, Call, Instance, Want};
use crate::object::{Fault, Found, Key, Native, Objects, Property, Value};

/// Which of the seven an error constructor is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// `Error`, whose prototype the other six inherit from.
    Error,
    /// `EvalError`, which nothing in the language throws any more and which a
    /// page may still construct.
    EvalError,
    /// `RangeError`.
    RangeError,
    /// `ReferenceError`.
    ReferenceError,
    /// `SyntaxError`.
    SyntaxError,
    /// `TypeError`.
    TypeError,
    /// `URIError`.
    UriError,
}

impl Family {
    /// All seven, `Error` first because the rest are made from it.
    pub const ALL: [Self; 7] = [
        Self::Error,
        Self::EvalError,
        Self::RangeError,
        Self::ReferenceError,
        Self::SyntaxError,
        Self::TypeError,
        Self::UriError,
    ];

    /// The name a page reads: the global it is bound to, and its prototype's
    /// `name`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Error => "Error",
            Self::EvalError => "EvalError",
            Self::RangeError => "RangeError",
            Self::ReferenceError => "ReferenceError",
            Self::SyntaxError => "SyntaxError",
            Self::TypeError => "TypeError",
            Self::UriError => "URIError",
        }
    }

    /// Where it is in [`Family::ALL`].
    pub const fn index(self) -> usize {
        match self {
            Self::Error => 0,
            Self::EvalError => 1,
            Self::RangeError => 2,
            Self::ReferenceError => 3,
            Self::SyntaxError => 4,
            Self::TypeError => 5,
            Self::UriError => 6,
        }
    }
}

impl From<crate::abrupt::Kind> for Family {
    /// The constructor an error this engine throws belongs to — which item
    /// 210's `catch` asks when it turns one into a value.
    fn from(kind: crate::abrupt::Kind) -> Self {
        match kind {
            crate::abrupt::Kind::TypeError => Self::TypeError,
            crate::abrupt::Kind::RangeError => Self::RangeError,
            crate::abrupt::Kind::ReferenceError => Self::ReferenceError,
        }
    }
}

/// Make the seven constructors and their prototypes, and answer a root on each
/// constructor, in [`Family::ALL`]'s order.
///
/// Each prototype is reached from its constructor's `prototype`, which may be
/// neither changed nor deleted, so rooting the constructor keeps both — even
/// after a page has deleted the global that named it.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling, and a fault for a reference this
/// engine has lost.
pub(super) fn make(
    objects: &mut Objects,
    object_prototype: Ref,
    function_prototype: Ref,
) -> Result<Vec<Root>, Escape> {
    let mut roots = Vec::with_capacity(Family::ALL.len());
    let (error, error_prototype) = one(
        objects,
        Family::Error,
        object_prototype,
        function_prototype,
        function_prototype,
    )?;
    roots.push(error);
    let error = objects
        .heap()
        .holding(roots.first().ok_or(Escape::fault(Fault::Gone))?)
        .ok_or(Escape::fault(Fault::Gone))?;
    for family in Family::ALL.into_iter().skip(1) {
        let (root, _) = one(objects, family, error_prototype, error, function_prototype)?;
        roots.push(root);
    }
    Ok(roots)
}

/// One constructor and its prototype, linked both ways, answering a root on the
/// constructor and the prototype it holds.
///
/// `above` is what the prototype inherits from and `inherits` what the
/// constructor does: `Object.prototype` and `Function.prototype` for `Error`,
/// and `Error.prototype` and `Error` for the six.
fn one(
    objects: &mut Objects,
    family: Family,
    above: Ref,
    inherits: Ref,
    function_prototype: Ref,
) -> Result<(Root, Ref), Escape> {
    let scope = objects.heap_mut().open();
    let outcome = linked(objects, family, above, inherits, function_prototype);
    objects.heap_mut().close(scope);
    outcome
}

/// [`one`], with the scope already open: everything made is held in it until a
/// property or the root owns it.
fn linked(
    objects: &mut Objects,
    family: Family,
    above: Ref,
    inherits: Ref,
    function_prototype: Ref,
) -> Result<(Root, Ref), Escape> {
    let prototype = objects
        .object(Some(above))
        .map_err(|why| Escape::refused(why, 0))?;
    objects.heap_mut().hold(prototype);
    let constructor = objects
        .native(
            Native::constructor(family.name(), construct, Instance::Error),
            Some(inherits),
        )
        .map_err(|why| Escape::refused(why, 0))?;
    objects.heap_mut().hold(constructor);

    // `prototype` on a builtin constructor is fixed: not writable, not
    // enumerable, not configurable. That is what lets the interpreter treat
    // anything else there as its own bug when it makes an instance.
    let key = held_key(objects, "prototype")?;
    objects.define(
        constructor,
        key,
        Property::data(Value::Object(prototype), false, false, false),
    )?;
    let key = held_key(objects, "constructor")?;
    objects.define(
        prototype,
        key,
        Property::data(Value::Object(constructor), true, false, true),
    )?;
    for (name, value) in [("name", family.name()), ("message", "")] {
        let key = held_key(objects, name)?;
        // The string needs no hold: nothing allocates between making it and
        // the property that owns it, which a doctored run confirmed.
        let text = objects
            .text(value.encode_utf16().collect())
            .map_err(|why| Escape::refused(why, 0))?;
        objects.define(
            prototype,
            key,
            Property::data(Value::Text(text), true, false, true),
        )?;
    }
    if family == Family::Error {
        super::method(
            objects,
            prototype,
            function_prototype,
            "toString",
            to_string,
        )?;
    }
    Ok((objects.heap_mut().root(constructor), prototype))
}

/// Intern a name and hold the string that spells it in the open scope.
fn held_key(objects: &mut Objects, name: &str) -> Result<Key, Escape> {
    let units: Vec<u16> = name.encode_utf16().collect();
    let key = objects.key(&units).map_err(|why| Escape::refused(why, 0))?;
    if let Some(held) = key.reference() {
        objects.heap_mut().hold(held);
    }
    Ok(key)
}

/// The body every error constructor shares: `Error ( message [ , options ] )`.
///
/// Step 0 has the instance and the arguments; step 1 has the primitive a
/// `message` object converted to; step 2 has what a `cause` getter answered.
/// The instance is in the `this` slot throughout, so whatever has been put on
/// it survives the script running in between.
fn construct(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let Value::Object(made) = call.this() else {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    };
    if !call.seen().is_error(made) {
        // The interpreter makes the instance before this runs, every time.
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    }
    match call.step() {
        0 => {
            let message = call.argument(0);
            if !matches!(message, Value::Undefined) {
                let Some(primitive) = Primitive::of(message) else {
                    // `ToString` of an object is `ToPrimitive` with a string
                    // hint first, which runs the page's own `toString`.
                    return Ok(Answer::want(
                        Want::Primitive {
                            of: message,
                            hint: Hint::String,
                        },
                        1,
                    ));
                };
                own(call, made, "message", primitive)?;
            }
            cause(call, made)
        }
        1 => {
            let Some(primitive) = Primitive::of(call.answer()?) else {
                // A conversion answers a primitive or throws.
                return Err(Escape::Broken(Internal::BuiltinIsWrong));
            };
            own(call, made, "message", primitive)?;
            cause(call, made)
        }
        2 => {
            let answer = call.answer()?;
            own_value(call, made, "cause", answer)?;
            Ok(Answer::Value(Value::Object(made)))
        }
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// `InstallErrorCause`, and the end of the constructor if no getter runs.
///
/// Only an **object** `options` with a `cause` — its own or inherited, since
/// the test is `HasProperty` — gives the error one. `cause: undefined` is a
/// cause, and no `cause` at all is not, which a page tells apart with `in`.
fn cause(call: &mut Call<'_>, made: Ref) -> Result<Answer, Escape> {
    let Value::Object(options) = call.argument(1) else {
        return Ok(Answer::Value(Value::Object(made)));
    };
    let at = call.at();
    let key = key(call.objects(), "cause", at)?;
    if !call.seen().has(options, key)? {
        return Ok(Answer::Value(Value::Object(made)));
    }
    let value = match call.seen().get(options, key)? {
        Found::Value(value) => value,
        Found::Missing | Found::Getter(Value::Undefined) => Value::Undefined,
        Found::Getter(getter) => {
            return Ok(Answer::want(
                Want::Call {
                    callee: getter,
                    receiver: Value::Object(options),
                    arguments: Vec::new(),
                },
                2,
            ));
        }
    };
    own_value(call, made, "cause", value)?;
    Ok(Answer::Value(Value::Object(made)))
}

/// Give the instance its own `message`, converted — writable, configurable and
/// **not enumerable**, which is why `Object.keys(new Error('x'))` is empty.
fn own(call: &mut Call<'_>, made: Ref, name: &str, primitive: Primitive) -> Result<(), Escape> {
    let scope = call.objects().heap_mut().open();
    let outcome = owned(call, made, name, primitive);
    call.objects().heap_mut().close(scope);
    outcome
}

/// [`own`], with the scope open. The key is interned and held **before** the
/// string is made, so nothing allocates between making it and storing it.
fn owned(call: &mut Call<'_>, made: Ref, name: &str, primitive: Primitive) -> Result<(), Escape> {
    let at = call.at();
    let key = key(call.objects(), name, at)?;
    if let Some(held) = key.reference() {
        call.objects().heap_mut().hold(held);
    }
    let value = convert::to_text(call.objects(), primitive, at)?;
    define(call, made, key, value)
}

/// Give the instance its own `cause`, which is stored as it is: a cause is any
/// value, and nothing converts it.
fn own_value(call: &mut Call<'_>, made: Ref, name: &str, value: Value) -> Result<(), Escape> {
    let at = call.at();
    // `value` is an argument's property or an answer on the stack, and so is
    // reachable from something the collector walks across the interning.
    let key = key(call.objects(), name, at)?;
    define(call, made, key, value)
}

/// `CreateNonEnumerableDataPropertyOrThrow`.
fn define(call: &mut Call<'_>, made: Ref, key: Key, value: Value) -> Result<(), Escape> {
    let at = call.at();
    if call
        .objects()
        .define(made, key, Property::data(value, true, false, true))?
    {
        Ok(())
    } else {
        Err(Escape::type_error(
            "an error could not be given a property it was being made with",
            at,
        ))
    }
}

/// A name as a key.
fn key(objects: &mut Objects, name: &str, at: usize) -> Result<Key, Escape> {
    let units: Vec<u16> = name.encode_utf16().collect();
    objects.key(&units).map_err(|why| Escape::refused(why, at))
}

/// `Error.prototype.toString`.
///
/// `name` is read first and may be a getter (step 1) or an object whose
/// `toString` must run (step 2); `message` is read once `name` is a string.
/// `name` defaults to `"Error"` and `message` to the empty string, and either
/// being empty leaves out the `": "` — so `new Error()` is `"Error"` and an
/// error whose name a page blanked is just its message.
fn to_string(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let at = call.at();
    let Value::Object(held) = call.this() else {
        return Err(Escape::type_error(
            "Error.prototype.toString was called on something that is not an object",
            at,
        ));
    };
    match call.step() {
        0 => {
            let key = key(call.objects(), "name", at)?;
            match call.seen().get(held, key)? {
                Found::Value(value) => named(call, held, value),
                Found::Missing | Found::Getter(Value::Undefined) => {
                    named(call, held, Value::Undefined)
                }
                Found::Getter(getter) => Ok(Answer::want(
                    Want::Call {
                        callee: getter,
                        receiver: Value::Object(held),
                        arguments: Vec::new(),
                    },
                    1,
                )),
            }
        }
        1 => {
            let value = call.answer()?;
            named(call, held, value)
        }
        2 => {
            let Some(primitive) = Primitive::of(call.answer()?) else {
                return Err(Escape::Broken(Internal::BuiltinIsWrong));
            };
            let name = convert::to_units(call.seen(), primitive, at)?;
            finish(call, held, name)
        }
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// `ToString(name)`, or `"Error"` when it is `undefined`.
fn named(call: &mut Call<'_>, held: Ref, value: Value) -> Result<Answer, Escape> {
    if matches!(value, Value::Undefined) {
        return finish(call, held, "Error".encode_utf16().collect());
    }
    let Some(primitive) = Primitive::of(value) else {
        return Ok(Answer::want(
            Want::Primitive {
                of: value,
                hint: Hint::String,
            },
            2,
        ));
    };
    let name = convert::to_units(call.seen(), primitive, call.at())?;
    finish(call, held, name)
}

/// Read `message` and put the two together.
///
/// Nothing here runs the script, which is the point: `name` is a Rust value
/// now and would not survive a call. A `message` that needs one is refused by
/// name (queue item 228) rather than read twice or read out of order.
fn finish(call: &mut Call<'_>, held: Ref, name: Vec<u16>) -> Result<Answer, Escape> {
    let at = call.at();
    let key = key(call.objects(), "message", at)?;
    let message = match call.seen().get(held, key)? {
        Found::Missing | Found::Getter(Value::Undefined) | Found::Value(Value::Undefined) => {
            Vec::new()
        }
        Found::Value(value) => match Primitive::of(value) {
            Some(primitive) => convert::to_units(call.seen(), primitive, at)?,
            None => return Err(Escape::NotBuiltYet(Missing::AMessageBehindACall)),
        },
        Found::Getter(_) => return Err(Escape::NotBuiltYet(Missing::AMessageBehindACall)),
    };
    let units = if name.is_empty() {
        message
    } else if message.is_empty() {
        name
    } else {
        let mut units = name;
        units.extend(": ".encode_utf16());
        units.extend(message);
        units
    };
    let text = call
        .objects()
        .text(units)
        .map_err(|why| Escape::refused(why, at))?;
    Ok(Answer::Value(Value::Text(text)))
}

#[cfg(test)]
mod tests {
    use super::Family;
    use crate::builtin::Intrinsics;
    use crate::object::{Found, Objects, Value};

    /// The text a property holds, read by name.
    fn text(objects: &mut Objects, on: crate::heap::Ref, name: &str) -> Option<String> {
        let units: Vec<u16> = name.encode_utf16().collect();
        let Ok(Found::Value(Value::Text(held))) = objects.get_named(on, &units) else {
            return None;
        };
        objects.units(held).map(String::from_utf16_lossy)
    }

    #[test]
    fn the_seven_are_built_with_the_collector_running_at_every_allocation() {
        // The integration tests turn stress on after the engine exists, so this
        // is the only place the scopes in `linked` are tested: each prototype,
        // constructor, key and name string is held by a Rust local across the
        // allocations that follow it until a property or a root owns it.
        let mut objects = Objects::new();
        objects.heap_mut().stress(true);
        let Ok(intrinsics) = Intrinsics::new(&mut objects) else {
            panic!("an empty heap holds the intrinsics however often it collects");
        };
        objects.heap_mut().stress(false);
        assert_eq!(objects.heap().scoped(), 0, "every scope was closed");

        let Ok(error) = intrinsics.error_constructor(&objects, Family::Error) else {
            panic!("Error is rooted");
        };
        for family in Family::ALL {
            let Ok(constructor) = intrinsics.error_constructor(&objects, family) else {
                panic!("{} is rooted", family.name());
            };
            let units: Vec<u16> = "prototype".encode_utf16().collect();
            let Ok(Found::Value(Value::Object(prototype))) = objects.get_named(constructor, &units)
            else {
                panic!("{} has its prototype", family.name());
            };
            assert_eq!(
                text(&mut objects, prototype, "name").as_deref(),
                Some(family.name())
            );
            assert_eq!(
                text(&mut objects, prototype, "message").as_deref(),
                Some("")
            );
            let units: Vec<u16> = "constructor".encode_utf16().collect();
            assert_eq!(
                objects.get_named(prototype, &units),
                Ok(Found::Value(Value::Object(constructor)))
            );
            let inherits = objects.prototype(constructor);
            if family == Family::Error {
                assert_eq!(
                    inherits.ok().flatten(),
                    intrinsics.function_prototype(&objects).ok()
                );
            } else {
                assert_eq!(inherits, Ok(Some(error)));
            }
            let Some(callable) = objects.callable(constructor) else {
                panic!("{} is a function", family.name());
            };
            let crate::object::Code::Native(native) = callable.code() else {
                panic!("{} is a builtin", family.name());
            };
            assert!(native.instance().is_some(), "{} constructs", family.name());
        }
    }

    #[test]
    fn an_error_this_engine_throws_names_the_family_it_belongs_to() {
        use crate::abrupt::Kind;
        assert_eq!(Family::from(Kind::TypeError), Family::TypeError);
        assert_eq!(Family::from(Kind::RangeError), Family::RangeError);
        assert_eq!(Family::from(Kind::ReferenceError), Family::ReferenceError);
        for (at, family) in Family::ALL.into_iter().enumerate() {
            assert_eq!(family.index(), at);
        }
    }
}
