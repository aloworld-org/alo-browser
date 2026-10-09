/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Event`: its constructor and its members (ADR 0018 § 8, queue item 254).
//!
//! - `new Event(type, eventInitDict)`: the type a `DOMString`, the
//!   dictionary's `bubbles`, `cancelable` and `composed` each `false` unless
//!   given. Calling it without `new` is a `TypeError`.
//! - `type`, `target`, `currentTarget`, `eventPhase`, `bubbles`,
//!   `cancelable`, `defaultPrevented` and `composed`, read-only.
//! - `stopPropagation()`, `stopImmediatePropagation()`, `preventDefault()` —
//!   which does nothing to an event that is not cancelable, or while a
//!   passive listener runs — and `composedPath()`, the path's targets from
//!   the target up while the event is dispatched and empty otherwise.
//! - The four phase constants, `NONE` to `BUBBLING_PHASE`, on the prototype
//!   and on the constructor.
//! - `isTrusted`, Web IDL's `[LegacyUnforgeable]` attribute (queue item 260,
//!   ADR 0019 § 3): not on the prototype but on **every instance**, an
//!   accessor that is enumerable and not configurable, with no setter, whose
//!   getter is one function per realm — made once on `Event`'s unforgeables
//!   object and copied onto each event by [`crate::unforgeable`]. It answers
//!   the flag the dispatch sets: `false` after a script's `dispatchEvent`,
//!   `true` for the browser's (ADR 0018 § 4).
//!
//! The constructor is given its instance before its body runs
//! ([`Instance::Made`](alo_js::object::native::Instance)), so what it has
//! converted is written into the instance as it goes, and a getter on the
//! dictionary — which runs the page's script, and is asked of the
//! interpreter — costs nothing but a step number.
//!
//! The constructor copies the unforgeables at its first step, before the
//! type is converted or the dictionary read — so before any page script can
//! run inside it — finding them through the realm's host, the document cell
//! (ADR 0019 § 2), since its fresh instance reaches nothing.
//!
//! **Absent** (ADR 0018 § 8): `timeStamp`, a clock, item 92's; and law 1's
//! `returnValue`, `cancelBubble`, `srcElement` and `initEvent`.

use alo_js::abrupt::Internal;
use alo_js::convert::{self, Hint, Primitive};
use alo_js::heap::Ref;
use alo_js::object::native::{Answer, Call, Want};
use alo_js::object::{Key, Objects, Property};
use alo_js::{Escape, Fault, Value};

use super::Interface;
use crate::dictionary::{self, Member};
use crate::embed::{self, Wrapping};
use crate::event::{Event, Init, Phase};
use crate::{define, unforgeable};
use crate::{idl, interface};

/// The phase constants, as Web IDL puts them on the prototype and on the
/// constructor.
pub const PHASES: [(&str, Phase); 4] = [
    ("NONE", Phase::None),
    ("CAPTURING_PHASE", Phase::Capturing),
    ("AT_TARGET", Phase::AtTarget),
    ("BUBBLING_PHASE", Phase::Bubbling),
];

/// `Event.prototype`'s members.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    let read_only = [
        ("type", kind as fn(&mut Call<'_>) -> _),
        ("target", target),
        ("currentTarget", current_target),
        ("eventPhase", event_phase),
        ("bubbles", bubbles),
        ("cancelable", cancelable),
        ("defaultPrevented", default_prevented),
        ("composed", composed),
    ];
    for (name, get) in read_only {
        define::attribute(objects, prototype, function_prototype, name, get, None)?;
    }
    let operations = [
        (
            "stopPropagation",
            stop_propagation as fn(&mut Call<'_>) -> _,
        ),
        ("stopImmediatePropagation", stop_immediate_propagation),
        ("preventDefault", prevent_default),
        ("composedPath", composed_path),
    ];
    for (name, body) in operations {
        define::operation(objects, prototype, function_prototype, name, body)?;
    }
    constants(objects, prototype)
}

/// `Event`'s `[LegacyUnforgeable]` member, on its unforgeables object.
pub(super) fn unforgeables(
    objects: &mut Objects,
    unforgeables: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::unforgeable_attribute(
        objects,
        unforgeables,
        function_prototype,
        "isTrusted",
        (is_trusted, None),
    )
}

/// Put the phase constants on `object`: neither writable nor configurable,
/// and enumerable, as Web IDL defines a constant.
pub(crate) fn constants(objects: &mut Objects, object: Ref) -> Result<(), Escape> {
    for (name, phase) in PHASES {
        let units: Vec<u16> = name.encode_utf16().collect();
        let property = Property::data(Value::Number(f64::from(phase.number())), false, true, false);
        match objects.define_named(object, &units, property) {
            Ok(true) => {}
            Ok(false) => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
            Err(named) => return Err(Escape::named(named, 0)),
        }
    }
    Ok(())
}

/// The event `this` is.
fn this<'a>(call: &'a Call<'_>, member: &'static str) -> Result<&'a Event, Escape> {
    match call.this() {
        Value::Object(held) => call.seen().embedded::<Event>(held),
        _ => None,
    }
    .ok_or_else(|| {
        Escape::type_error(
            format!("'{member}' was called on something that is not an Event"),
            call.at(),
        )
    })
}

/// Change the event `this` is.
fn change(
    call: &mut Call<'_>,
    member: &'static str,
    change: impl FnOnce(&mut Event),
) -> Result<Answer, Escape> {
    this(call, member)?;
    if let Value::Object(held) = call.this() {
        call.objects()
            .write_embedded::<Event, _>(held, |event, _| change(event));
    }
    Ok(Answer::Value(Value::Undefined))
}

/// `get type`.
fn kind(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let units = this(call, "type")?.kind().to_vec();
    let at = call.at();
    let held = call
        .objects()
        .text(units)
        .map_err(|why| Escape::refused(why, at))?;
    Ok(Answer::Value(Value::Text(held)))
}

/// A wrapper, or `null`.
fn wrapper_or_null(held: Option<Ref>) -> Answer {
    Answer::Value(held.map_or(Value::Null, Value::Object))
}

/// `get target`.
fn target(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(wrapper_or_null(this(call, "target")?.target()))
}

/// `get currentTarget`.
fn current_target(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(wrapper_or_null(
        this(call, "currentTarget")?.current_target(),
    ))
}

/// `get eventPhase`.
fn event_phase(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let phase = this(call, "eventPhase")?.phase();
    Ok(Answer::Value(Value::Number(f64::from(phase.number()))))
}

/// `get bubbles`.
fn bubbles(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(Value::Bool(
        this(call, "bubbles")?.init(Init::Bubbles),
    )))
}

/// `get cancelable`.
fn cancelable(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(Value::Bool(
        this(call, "cancelable")?.init(Init::Cancelable),
    )))
}

/// `get composed`.
fn composed(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(Value::Bool(
        this(call, "composed")?.init(Init::Composed),
    )))
}

/// `get isTrusted`.
fn is_trusted(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(Value::Bool(
        this(call, "isTrusted")?.trusted(),
    )))
}

/// `get defaultPrevented`.
fn default_prevented(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(Value::Bool(
        this(call, "defaultPrevented")?.canceled(),
    )))
}

/// `stopPropagation()`.
fn stop_propagation(call: &mut Call<'_>) -> Result<Answer, Escape> {
    change(call, "stopPropagation", Event::stop_propagation)
}

/// `stopImmediatePropagation()`.
fn stop_immediate_propagation(call: &mut Call<'_>) -> Result<Answer, Escape> {
    change(
        call,
        "stopImmediatePropagation",
        Event::stop_immediate_propagation,
    )
}

/// `preventDefault()`.
fn prevent_default(call: &mut Call<'_>) -> Result<Answer, Escape> {
    change(call, "preventDefault", Event::prevent_default)
}

/// `composedPath()`: the path's targets, from the target up, as an array —
/// empty when the event is not being dispatched. With no shadow trees
/// nothing on the path is hidden, so it is the whole path.
///
/// **A safepoint** at every target that has no wrapper yet. The array is held
/// in a scope while they are made, and each node is kept by the dispatch
/// ([`crate::dispatch`]).
fn composed_path(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let event = this(call, "composedPath")?;
    let (owner, path) = match event.progress() {
        Some(progress) => (progress.document(), progress.path().to_vec()),
        None => (None, Vec::new()),
    };
    let at = call.at();
    let length = u32::try_from(path.len()).map_err(|_| Escape::Broken(Internal::BuiltinIsWrong))?;
    let above = call.intrinsics()?.array_prototype(call.seen())?;
    let array = call
        .objects()
        .array(Some(above), length)
        .map_err(|why| Escape::refused(why, at))?;
    let Some(owner) = owner else {
        return Ok(Answer::Value(Value::Object(array)));
    };
    let scope = call.objects().heap_mut().open();
    call.objects().heap_mut().hold(array);
    let filled = fill(call.objects(), array, owner, &path, at);
    call.objects().heap_mut().close(scope);
    filled.map(|()| Answer::Value(Value::Object(array)))
}

/// Put each node's wrapper in `array`, in order, with the scope open.
fn fill(
    objects: &mut Objects,
    array: Ref,
    cell: Ref,
    path: &[alo_dom::NodeId],
    at: usize,
) -> Result<(), Escape> {
    for (index, node) in path.iter().enumerate() {
        let prototype = interface::prototype_of(objects, cell, *node);
        let wrapper = match embed::wrap(objects, cell, *node, prototype) {
            Ok(wrapper) => wrapper,
            Err(Wrapping::Refused(refused)) => return Err(Escape::refused(refused, at)),
            Err(Wrapping::NotADocument | Wrapping::NoSuchNode(_)) => {
                return Err(Escape::Broken(Internal::BuiltinIsWrong));
            }
        };
        let key = u32::try_from(index)
            .ok()
            .and_then(Key::index)
            .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
        let property = Property::data(Value::Object(wrapper), true, true, true);
        if !objects.define(array, key, property)? {
            return Err(Escape::Broken(Internal::BuiltinIsWrong));
        }
    }
    Ok(())
}

/// The first step at which a dictionary member's getter has answered; step 1
/// is the type an object's `toString` made.
const MEMBER_STEP: u32 = 16;

/// `EventInit`'s members, then `CustomEventInit`'s.
const MEMBERS: [&str; 4] = ["bubbles", "cancelable", "composed", "detail"];

/// `new Event(type, eventInitDict)`.
pub(crate) fn construct(call: &mut Call<'_>) -> Result<Answer, Escape> {
    constructed(call, Interface::Event)
}

/// The constructor of `Event` or `CustomEvent`, which differ only in whether
/// the dictionary has a `detail`.
pub(crate) fn constructed(call: &mut Call<'_>, interface: Interface) -> Result<Answer, Escape> {
    let name = interface.name();
    if !call.constructing() {
        return Err(Escape::type_error(
            format!("'{name}' is a constructor, and needs 'new'"),
            call.at(),
        ));
    }
    let Value::Object(instance) = call.this() else {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    };
    if call.seen().embedded::<Event>(instance).is_none() {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    }
    if call.step() == 0 {
        // Before anything is converted, so before any page script runs. The
        // realm's host is the document cell `install` named; a realm with
        // none was never installed, which is this crate's bug.
        let page = call.host_defined().ok_or(Escape::fault(Fault::Gone))?;
        unforgeable::copy(call.objects(), page, instance, interface)?;
    }
    idl::needs(call, 1, name)?;
    // A `CustomEvent`'s dictionary has `detail` after the three it inherits.
    let count = if interface == Interface::CustomEvent {
        4
    } else {
        3
    };
    let members = MEMBERS.get(..count).unwrap_or(&[]);
    let first = match call.step() {
        0 => {
            let Some(primitive) = Primitive::of(call.argument(0)) else {
                return Ok(Answer::want(
                    Want::Primitive {
                        of: call.argument(0),
                        hint: Hint::String,
                    },
                    1,
                ));
            };
            let kind = convert::to_units(call.seen(), primitive, call.at())?;
            set(call, instance, |event, _| event.set_kind(kind));
            0
        }
        1 => {
            let kind: Vec<u16> = idl::answered_string(call)?.encode_utf16().collect();
            set(call, instance, |event, _| event.set_kind(kind));
            0
        }
        step => {
            let index = usize::try_from(step - MEMBER_STEP)
                .map_err(|_| Escape::Broken(Internal::BuiltinIsWrong))?;
            let member = members
                .get(index)
                .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
            let value = call.answer()?;
            apply(call, instance, member, value);
            index.saturating_add(1)
        }
    };
    let Some(dictionary) = dictionary::argument(call, 1, name)? else {
        return Ok(Answer::Value(Value::Object(instance)));
    };
    for (index, member) in members.iter().enumerate().skip(first) {
        let step = u32::try_from(index)
            .map(|index| MEMBER_STEP + index)
            .map_err(|_| Escape::Broken(Internal::BuiltinIsWrong))?;
        match dictionary::member(call, dictionary, member, step)? {
            Member::Value(value) => apply(call, instance, member, value),
            Member::Ask(asked) => return Ok(asked),
        }
    }
    Ok(Answer::Value(Value::Object(instance)))
}

/// Write one member's value into the instance: a flag `false` unless given,
/// `detail` `null` unless given.
fn apply(call: &mut Call<'_>, instance: Ref, member: &str, value: Value) {
    let flag = convert::to_boolean(call.seen(), value);
    match member {
        "bubbles" => set(call, instance, |event, _| {
            event.set_init(Init::Bubbles, flag);
        }),
        "cancelable" => set(call, instance, |event, _| {
            event.set_init(Init::Cancelable, flag);
        }),
        "composed" => set(call, instance, |event, _| {
            event.set_init(Init::Composed, flag);
        }),
        _ => {
            let detail = if value == Value::Undefined {
                Value::Null
            } else {
                value
            };
            set(call, instance, |event, barrier| {
                event.set_detail(barrier, detail);
            });
        }
    }
}

/// Change the instance being constructed.
fn set(
    call: &mut Call<'_>,
    instance: Ref,
    change: impl FnOnce(&mut Event, &mut alo_js::heap::Barrier),
) {
    call.objects()
        .write_embedded::<Event, _>(instance, |event, barrier| change(event, barrier));
}
