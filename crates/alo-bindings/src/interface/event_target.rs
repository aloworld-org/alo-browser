/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `EventTarget`: adding a listener, removing one, and dispatching an event
//! from script (ADR 0018 §§ 1–3, queue item 254).
//!
//! - `addEventListener(type, callback, options)` — `options` a boolean
//!   (`capture`) or a dictionary of `capture`, `once`, `passive` and
//!   `signal`. A `null` callback adds nothing; one with the same type,
//!   callback and `capture` as a listener already there adds nothing either.
//!   A `passive` nobody gave is the standard's default: true for `touchstart`,
//!   `touchmove`, `wheel` and `mousewheel` on the document, its document
//!   element or its body.
//! - `removeEventListener(type, callback, options)` — `options` a boolean or
//!   a dictionary of `capture`.
//! - `dispatchEvent(event)` — the event dispatched to this target, untrusted,
//!   its listeners called by this native one at a time through the stepper
//!   ([`crate::dispatch`]); a listener that throws is **reported** and the
//!   next one runs, and the answer is whether nobody cancelled it.
//!
//! # `signal` is a `TypeError`
//!
//! Its type is `AbortSignal`, an interface that does not exist yet, so any
//! `signal` that is not `undefined` fails the conversion with the `TypeError`
//! Web IDL's own conversion throws for a value that is not the member's type
//! (ADR 0018 § 1) — not a refusal invented here, and correct until
//! `AbortSignal` is built.
//!
//! # A type that is an object, and an options getter
//!
//! `type` is a `DOMString`, so an object there runs the page's `toString`, and
//! a getter on the options dictionary runs the page's script too. A native
//! keeps a step number across what it asks for and nothing else, and the
//! type string would have nowhere to wait: so when **both** happen the call
//! is refused by name ([`Missing::ASecondArgumentBehindACall`], queue item
//! 221), as `setAttribute` with two objects is. Either alone is converted in
//! full, and every boolean converted so far rides in the step number.

use alo_js::abrupt::{Internal, Missing};
use alo_js::convert::{self, Hint, Primitive};
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call, Want};
use alo_js::{Escape, Value};

use super::dom_exception;
use crate::define;
use crate::dictionary::{self, Member};
use crate::dispatch::{self, Invoke, Next, Refusal};
use crate::event::Event;
use crate::idl::{self, Brand, This};
use crate::listeners::Wanted;
use crate::wrapper::Wrapper;

/// `EventTarget.prototype`'s members.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    let operations = [
        (
            "addEventListener",
            add_event_listener as fn(&mut Call<'_>) -> _,
        ),
        ("removeEventListener", remove_event_listener),
        ("dispatchEvent", dispatch_event),
    ];
    for (name, body) in operations {
        define::operation(objects, prototype, function_prototype, name, body)?;
    }
    Ok(())
}

/// The first step at which a dictionary member's getter has answered. Below
/// it, step 1 is the type an object's `toString` made.
const MEMBER_STEP: u32 = 16;

/// The members of `AddEventListenerOptions`, inherited first.
const ADD_MEMBERS: [&str; 4] = ["capture", "once", "passive", "signal"];

/// The member of `EventListenerOptions`.
const REMOVE_MEMBERS: [&str; 1] = ["capture"];

/// The options, as bits: what has been converted so far rides in a step.
const CAPTURE: u32 = 1;
const ONCE: u32 = 2;
const PASSIVE_GIVEN: u32 = 4;
const PASSIVE: u32 = 8;

/// `addEventListener(type, callback, options)`.
fn add_event_listener(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let member = "addEventListener";
    let this = idl::this(call, Brand::EventTarget, member)?;
    idl::needs(call, 2, member)?;
    let (kind, options) = match arguments(call, &ADD_MEMBERS, member)? {
        Read::Ready(kind, options) => (kind, options),
        Read::Asked(asked) => return Ok(asked),
    };
    let Some(callback) = callback(call, member)? else {
        return Ok(Answer::Value(Value::Undefined));
    };
    let passive = if options & PASSIVE_GIVEN == 0 {
        passive_by_default(call, this, &kind)?
    } else {
        options & PASSIVE != 0
    };
    let wrapper = target(call)?;
    call.objects()
        .write_embedded::<Wrapper, _>(wrapper, |held, barrier| {
            held.listeners_mut().add(
                barrier,
                Wanted {
                    kind: &kind,
                    callback,
                    capture: options & CAPTURE != 0,
                    once: options & ONCE != 0,
                    passive,
                },
            )
        })
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    Ok(Answer::Value(Value::Undefined))
}

/// `removeEventListener(type, callback, options)`.
fn remove_event_listener(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let member = "removeEventListener";
    idl::this(call, Brand::EventTarget, member)?;
    idl::needs(call, 2, member)?;
    let (kind, options) = match arguments(call, &REMOVE_MEMBERS, member)? {
        Read::Ready(kind, options) => (kind, options),
        Read::Asked(asked) => return Ok(asked),
    };
    let Some(callback) = callback(call, member)? else {
        return Ok(Answer::Value(Value::Undefined));
    };
    let wrapper = target(call)?;
    call.objects()
        .write_embedded::<Wrapper, _>(wrapper, |held, barrier| {
            held.listeners_mut()
                .remove(barrier, &kind, callback, options & CAPTURE != 0)
        })
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    Ok(Answer::Value(Value::Undefined))
}

/// What reading the type and the options came to.
#[derive(Debug)]
enum Read {
    /// The type and the options' bits.
    Ready(Vec<u16>, u32),
    /// Something has to be run first.
    Asked(Answer),
}

/// Convert the type and the options, at whatever step this is.
fn arguments(call: &Call<'_>, members: &[&str], member: &'static str) -> Result<Read, Escape> {
    let step = call.step();
    if step >= MEMBER_STEP {
        // A getter has answered: the type is a primitive (or the call was
        // refused before asking), and the bits so far ride in the step.
        let index = usize::try_from((step - MEMBER_STEP) / MEMBER_STEP)
            .map_err(|_| Escape::Broken(Internal::BuiltinIsWrong))?;
        let bits = (step - MEMBER_STEP) % MEMBER_STEP;
        let name = members
            .get(index)
            .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
        let bits = apply(call, name, call.answer()?, bits, member)?;
        let kind = primitive_kind(call)?.ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
        return options_from(call, members, index.saturating_add(1), bits, member, false)
            .map(|read| finish(read, kind));
    }
    let (kind, by_object) = match step {
        0 => match primitive_kind(call)? {
            Some(kind) => (kind, false),
            None => {
                return Ok(Read::Asked(Answer::want(
                    Want::Primitive {
                        of: call.argument(0),
                        hint: Hint::String,
                    },
                    1,
                )));
            }
        },
        1 => (idl::answered_string(call)?.encode_utf16().collect(), true),
        _ => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    };
    let read = match call.argument(2) {
        Value::Undefined | Value::Null => Options::Ready(0),
        Value::Object(_) => options_from(call, members, 0, 0, member, by_object)?,
        other => Options::Ready(if convert::to_boolean(call.seen(), other) {
            CAPTURE
        } else {
            0
        }),
    };
    Ok(finish(read, kind))
}

/// The options dictionary's members from `first` on, or the getter to ask
/// for.
#[derive(Debug)]
enum Options {
    Ready(u32),
    Ask(Answer),
}

/// Read the options dictionary from member `first`, with `bits` converted so
/// far.
fn options_from(
    call: &Call<'_>,
    members: &[&str],
    first: usize,
    mut bits: u32,
    member: &'static str,
    by_object: bool,
) -> Result<Options, Escape> {
    let Value::Object(dictionary) = call.argument(2) else {
        return Ok(Options::Ready(bits));
    };
    for (index, name) in members.iter().enumerate().skip(first) {
        let index = u32::try_from(index).map_err(|_| Escape::Broken(Internal::BuiltinIsWrong))?;
        let step = MEMBER_STEP + index * MEMBER_STEP + bits;
        match dictionary::member(call, dictionary, name, step)? {
            Member::Value(value) => bits = apply(call, name, value, bits, member)?,
            Member::Ask(_) if by_object => {
                return Err(Escape::NotBuiltYet(Missing::ASecondArgumentBehindACall));
            }
            Member::Ask(asked) => return Ok(Options::Ask(asked)),
        }
    }
    Ok(Options::Ready(bits))
}

/// The two halves, put together.
fn finish(options: Options, kind: Vec<u16>) -> Read {
    match options {
        Options::Ready(bits) => Read::Ready(kind, bits),
        Options::Ask(asked) => Read::Asked(asked),
    }
}

/// One member's value, into the bits.
fn apply(
    call: &Call<'_>,
    name: &str,
    value: Value,
    bits: u32,
    member: &'static str,
) -> Result<u32, Escape> {
    let set = |bit| {
        if convert::to_boolean(call.seen(), value) {
            bits | bit
        } else {
            bits
        }
    };
    Ok(match name {
        "capture" => set(CAPTURE),
        "once" => set(ONCE),
        // Not present: `passive` has no default and `signal` is optional.
        "passive" | "signal" if value == Value::Undefined => bits,
        "passive" => set(PASSIVE) | PASSIVE_GIVEN,
        "signal" => {
            return Err(Escape::type_error(
                format!(
                    "'signal' in the options to '{member}' is not an AbortSignal, \
                     which this browser does not have yet"
                ),
                call.at(),
            ));
        }
        _ => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    })
}

/// The type, when it is a primitive and so needs nothing run; [`None`] when
/// it is an object.
fn primitive_kind(call: &Call<'_>) -> Result<Option<Vec<u16>>, Escape> {
    match Primitive::of(call.argument(0)) {
        Some(primitive) => convert::to_units(call.seen(), primitive, call.at()).map(Some),
        None => Ok(None),
    }
}

/// The `EventListener?` argument: an object — a function, or anything with a
/// `handleEvent` looked up when it is called — or [`None`] for `null` and
/// `undefined`.
fn callback(call: &Call<'_>, member: &'static str) -> Result<Option<Ref>, Escape> {
    match call.argument(1) {
        Value::Null | Value::Undefined => Ok(None),
        Value::Object(held) => Ok(Some(held)),
        _ => Err(Escape::type_error(
            format!("argument 2 to '{member}' is not an object"),
            call.at(),
        )),
    }
}

/// The standard's *default passive value*.
fn passive_by_default(call: &Call<'_>, this: This, kind: &[u16]) -> Result<bool, Escape> {
    let scrolls = ["touchstart", "touchmove", "wheel", "mousewheel"]
        .iter()
        .any(|name| name.encode_utf16().eq(kind.iter().copied()));
    if !scrolls {
        return Ok(false);
    }
    let document = idl::read(call, this.owner)?;
    Ok(this.node == document.root()
        || Some(this.node) == document.document_element()
        || Some(this.node) == document.body())
}

/// The wrapper the member was called on, which the brand check has passed.
fn target(call: &Call<'_>) -> Result<Ref, Escape> {
    match call.this() {
        Value::Object(held) => Ok(held),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// Step 1 of `dispatchEvent`: a listener returned or was reported.
const RETURNED: u32 = 1;

/// Step 2: a callback object's `handleEvent` getter answered.
const HANDLE_EVENT: u32 = 2;

/// `dispatchEvent(event)`.
fn dispatch_event(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let member = "dispatchEvent";
    let this = idl::this(call, Brand::EventTarget, member)?;
    idl::needs(call, 1, member)?;
    let event = match call.argument(0) {
        Value::Object(held) if call.seen().embedded::<Event>(held).is_some() => held,
        _ => {
            return Err(Escape::type_error(
                "argument 1 to 'dispatchEvent' is not an Event",
                call.at(),
            ));
        }
    };
    match call.step() {
        0 => {
            let wrapper = target(call)?;
            match dispatch::begin(call.objects(), event, wrapper, false) {
                Ok(()) => {}
                Err(Refusal::Dispatching) => {
                    return Err(dom_exception::thrown_named(
                        call,
                        this.owner,
                        "InvalidStateError",
                        "the event is already being dispatched",
                    ));
                }
                Err(Refusal::NotAnEvent | Refusal::NotATarget) => {
                    return Err(Escape::Broken(Internal::BuiltinIsWrong));
                }
            }
        }
        RETURNED => dispatch::returned(call.objects(), event),
        HANDLE_EVENT if call.reported() => dispatch::returned(call.objects(), event),
        HANDLE_EVENT => {
            // The getter answered `handleEvent`: call it on the object.
            let receiver = call
                .seen()
                .embedded::<Event>(event)
                .and_then(Event::progress)
                .and_then(dispatch::Progress::invoking)
                .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
            return Ok(Answer::want(
                Want::Report {
                    callee: call.answer()?,
                    receiver: Value::Object(receiver),
                    arguments: vec![Value::Object(event)],
                },
                RETURNED,
            ));
        }
        _ => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
    drive(call, event)
}

/// Ask the stepper what is next, and ask the interpreter for it.
///
/// Nothing allocates between the stepper's answer and the call being asked
/// for: the callback is held by the event while it runs, and its `this` by
/// the document cell.
fn drive(call: &mut Call<'_>, event: Ref) -> Result<Answer, Escape> {
    let (callback, this) = match dispatch::next(call.objects(), event)? {
        Next::Done { canceled } => return Ok(Answer::Value(Value::Bool(!canceled))),
        Next::Call { callback, this } => (callback, this),
    };
    let (callee, receiver, step, arguments) = match dispatch::invoke(call.seen(), callback, this)? {
        Invoke::Call { callee, this } => (callee, this, RETURNED, vec![Value::Object(event)]),
        // A getter is a call of its own, and its throw is reported like the
        // listener's would be.
        Invoke::Get { getter, this } => (getter, Value::Object(this), HANDLE_EVENT, Vec::new()),
    };
    Ok(Answer::want(
        Want::Report {
            callee,
            receiver,
            arguments,
        },
        step,
    ))
}
