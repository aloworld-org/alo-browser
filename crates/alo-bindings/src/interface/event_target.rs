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
//!   `touchmove`, `wheel` and `mousewheel` on the window, the document, its
//!   document element or its body.
//! - `removeEventListener(type, callback, options)` — `options` a boolean or
//!   a dictionary of `capture`.
//! - `dispatchEvent(event)` — the event dispatched to this target, untrusted,
//!   its listeners called by this native one at a time through the stepper
//!   ([`crate::dispatch`]), driven as `scripted.rs` drives it; a listener that throws is **reported** and the
//!   next one runs, and the answer is whether nobody cancelled it.
//!
//! # A node or the window
//!
//! Every member's `this` is a node's wrapper **or the page's `Window`**
//! (ADR 0037 § 2), and anything else is the `TypeError` of Web IDL's brand
//! check — except `undefined` and `null`, which Web IDL makes the realm's
//! global object, so a bare `addEventListener(…)` is the window's. Both hold their listeners the same way and are reached through
//! the same two functions ([`crate::listeners`]), so nothing here differs
//! between them but the default `passive`.
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

use alo_dom::NodeId;
use alo_js::abrupt::{Internal, Missing};
use alo_js::convert::{self, Hint, Primitive};
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call, Want};
use alo_js::{Escape, Fault, Value};

use super::dom_exception;
use crate::define;
use crate::dictionary::{self, Member};
use crate::dispatch::{self, Refusal};
use crate::document_cell::DocumentCell;
use crate::embed;
use crate::event::Event;
use crate::idl;
use crate::listeners::{self, Wanted};
use crate::scripted::{self, Driven};
use crate::window::Window;

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
    let this = target(call, member)?;
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
    listeners::change(call.objects(), this.object, |list, barrier| {
        list.add(
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
    let this = target(call, member)?;
    idl::needs(call, 2, member)?;
    let (kind, options) = match arguments(call, &REMOVE_MEMBERS, member)? {
        Read::Ready(kind, options) => (kind, options),
        Read::Asked(asked) => return Ok(asked),
    };
    let Some(callback) = callback(call, member)? else {
        return Ok(Answer::Value(Value::Undefined));
    };
    listeners::change(call.objects(), this.object, |list, barrier| {
        list.remove(barrier, &kind, callback, options & CAPTURE != 0)
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

/// The standard's *default passive value*: the window is the first target
/// it names.
fn passive_by_default(call: &Call<'_>, this: Target, kind: &[u16]) -> Result<bool, Escape> {
    let scrolls = ["touchstart", "touchmove", "wheel", "mousewheel"]
        .iter()
        .any(|name| name.encode_utf16().eq(kind.iter().copied()));
    if !scrolls {
        return Ok(false);
    }
    let Some(node) = this.node else {
        return Ok(true);
    };
    let document = idl::read(call, this.owner)?;
    Ok(node == document.root()
        || Some(node) == document.document_element()
        || Some(node) == document.body())
}

/// A member's `this`, once the brand check has passed.
#[derive(Debug, Clone, Copy)]
struct Target {
    /// The object: a node's wrapper or the window.
    object: Ref,
    /// The document cell it belongs to: its node's, or the window's
    /// document's.
    owner: Ref,
    /// Its node, or [`None`] for the window.
    node: Option<NodeId>,
}

/// The brand check: `this` as a node's wrapper or a `Window` (ADR 0037
/// § 2).
///
/// # Errors
///
/// A `TypeError` naming `member` for anything else.
fn target(call: &Call<'_>, member: &'static str) -> Result<Target, Escape> {
    let refused = || {
        Escape::type_error(
            format!("'{member}' was called on something that is not an EventTarget"),
            call.at(),
        )
    };
    let object = match call.this() {
        Value::Object(object) => object,
        // Web IDL: an operation called with no `this` is called on the
        // realm's global object, which is the window — so a page's bare
        // `addEventListener(…)` is the window's.
        Value::Undefined | Value::Null => call
            .host_defined()
            .and_then(|cell| call.seen().embedded::<DocumentCell>(cell))
            .and_then(DocumentCell::window)
            .ok_or_else(refused)?,
        _ => return Err(refused()),
    };
    if let Some((owner, node)) = embed::node_of(call.seen(), object) {
        return Ok(Target {
            object,
            owner,
            node: Some(node),
        });
    }
    let window = call.seen().embedded::<Window>(object).ok_or_else(refused)?;
    // A window is given its document before any script can run, and a
    // member of `EventTarget` cannot be reached before then.
    let owner = window.document().ok_or(Escape::fault(Fault::Gone))?;
    Ok(Target {
        object,
        owner,
        node: None,
    })
}

/// `dispatchEvent(event)`.
fn dispatch_event(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let member = "dispatchEvent";
    let this = target(call, member)?;
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
    let driven = if call.step() == 0 {
        match dispatch::begin(call.objects(), event, this.object, false) {
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
        scripted::drive(call, event, 0)?
    } else {
        scripted::resume(call, event, 0)?
    };
    Ok(match driven {
        Driven::Asked(asked) => asked,
        Driven::Done { canceled } => Answer::Value(Value::Bool(!canceled)),
    })
}
