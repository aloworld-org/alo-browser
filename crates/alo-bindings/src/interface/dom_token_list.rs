/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `DOMTokenList` (queue item 327): an element's `class`, as a set of
//! tokens, through `el.classList` ([`crate::token_list`]).
//!
//! Every member reads the element's `class` attribute now, and every change
//! is written back through `alo-dom` as the standard's **update steps**: the
//! set serialized, one space between tokens, unless the element has no
//! `class` attribute and the set is empty — then nothing is written, so
//! `el.classList.remove("x")` on an element with no `class` does not give it
//! an empty one.
//!
//! - `length` and `value` (with its setter, which writes the string as
//!   given), and `toString()`, the stringifier, which is `value`.
//! - `contains(token)`, which validates nothing.
//! - `add(...tokens)` and `remove(...tokens)`: every token validated before
//!   anything changes — an empty one is a `SyntaxError`, one with ASCII
//!   whitespace an `InvalidCharacterError`, both `DOMException`s — then the
//!   set rewritten once.
//! - `toggle(token, force)`, answering whether the token is there after it,
//!   and writing nothing when `force` asks for what already is.
//!
//! # Several string arguments
//!
//! `add` and `remove` take any number of `DOMString`s, and an object among
//! them runs the page's `toString`. A native keeps one answer across what it
//! asks for (`element.rs` says why), so **one** object among the tokens is
//! converted in full and a second is refused by name
//! ([`Missing::ASecondArgumentBehindACall`], queue item 221) after the
//! first's `toString` has run — as `setAttribute` refuses two.
//!
//! **Not here** (queue item 328): the indexed getter (`list[0]`) and `item`,
//! `replace`, `supports`, iteration (`forEach`, `keys`, `values`, `entries`
//! and `Symbol.iterator`), and `classList`'s `[PutForwards=value]`, so
//! assigning to `el.classList` is ignored rather than setting `value`.

use alo_js::abrupt::{Internal, Missing};
use alo_js::convert::{self, Primitive};
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Value};

use super::dom_exception;
use crate::define;
use crate::embed;
use crate::idl::{self, Converted, This};
use crate::token_list::TokenList;
use crate::tokens::{self, Invalid};

/// `DOMTokenList.prototype`'s members.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "length",
        length,
        None,
    )?;
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "value",
        value,
        Some(set_value),
    )?;
    let operations = [
        ("contains", contains as fn(&mut Call<'_>) -> _),
        ("add", add),
        ("remove", remove),
        ("toggle", toggle),
        ("toString", value),
    ];
    for (name, body) in operations {
        define::operation(objects, prototype, function_prototype, name, body)?;
    }
    Ok(())
}

/// Web IDL's brand check: `this` must be a `DOMTokenList`, or the member's
/// `TypeError`. Answers its element.
fn this(call: &Call<'_>, member: &'static str) -> Result<This, Escape> {
    let held = match call.this() {
        Value::Object(held) => call
            .seen()
            .embedded::<TokenList>(held)
            .and_then(TokenList::element)
            .and_then(|wrapper| embed::node_of(call.seen(), wrapper)),
        _ => None,
    };
    held.map(|(owner, node)| This { owner, node })
        .ok_or_else(|| {
            Escape::type_error(
                format!("'{member}' was used on something that is not a DOMTokenList"),
                call.at(),
            )
        })
}

/// The element's `class` attribute, if it has one.
fn class(call: &Call<'_>, this: This) -> Result<Option<String>, Escape> {
    Ok(idl::read(call, this.owner)?
        .element(this.node)
        .and_then(|element| element.attr("class"))
        .map(str::to_owned))
}

/// The update steps: write `set` back as the `class` attribute — unless
/// there was none and the set is empty.
fn update(call: &mut Call<'_>, this: This, had: bool, set: &str) -> Result<(), Escape> {
    if !had && set.is_empty() {
        return Ok(());
    }
    idl::change(call, this.owner, |document| {
        document.set_attribute(this.node, "class", set)
    })?;
    Ok(())
}

/// The escape for a token validation refused.
fn refused(call: &mut Call<'_>, this: This, invalid: Invalid) -> Escape {
    dom_exception::thrown_named(call, this.owner, invalid.name(), invalid.message())
}

/// `get length`.
fn length(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = this(call, "length")?;
    let count = tokens::parse(&class(call, this)?.unwrap_or_default()).len();
    // `length` is an `unsigned long`, and a set parsed from a string the
    // engine could make has far fewer tokens than that holds.
    let count = u32::try_from(count).map_err(|_| Escape::Broken(Internal::BuiltinIsWrong))?;
    let count = f64::from(count);
    Ok(Answer::Value(Value::Number(count)))
}

/// `get value`, and `toString()`.
fn value(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = this(call, "value")?;
    let value = class(call, this)?.unwrap_or_default();
    idl::answer_text(call, Some(value))
}

/// `set value`: the attribute, set to what was given.
fn set_value(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = this(call, "value")?;
    idl::needs(call, 1, "value")?;
    let value = match idl::only_string(call)? {
        Converted::Ready(value) => value,
        Converted::Asked(asked) => return Ok(asked),
    };
    idl::change(call, this.owner, |document| {
        document.set_attribute(this.node, "class", &value)
    })?;
    Ok(Answer::Value(Value::Undefined))
}

/// `contains(token)`.
fn contains(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = this(call, "contains")?;
    idl::needs(call, 1, "contains")?;
    let token = match idl::only_string(call)? {
        Converted::Ready(token) => token,
        Converted::Asked(asked) => return Ok(asked),
    };
    let value = class(call, this)?.unwrap_or_default();
    let found = tokens::parse(&value).contains(&token.as_str());
    Ok(Answer::Value(Value::Bool(found)))
}

/// Every argument as a `DOMString`, in order — or what to ask for first.
///
/// Step 0 converts the primitives and asks for the first object, coming
/// back at step 1, which reads that object's string and converts the
/// primitives again, which runs no script.
fn every_token(call: &Call<'_>) -> Result<Result<Vec<String>, Answer>, Escape> {
    let mut answered = match call.step() {
        0 => None,
        1 => Some(idl::answered_string(call)?),
        _ => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    };
    let asked = answered.is_some();
    let mut met_an_object = false;
    let mut out = Vec::with_capacity(call.count());
    for which in 0..call.count() {
        let argument = call.argument(which);
        if let Some(primitive) = Primitive::of(argument) {
            let units = convert::to_units(call.seen(), primitive, call.at())?;
            out.push(String::from_utf16_lossy(&units));
            continue;
        }
        if met_an_object {
            return Err(Escape::NotBuiltYet(Missing::ASecondArgumentBehindACall));
        }
        met_an_object = true;
        match answered.take() {
            Some(string) => out.push(string),
            None if !asked => match idl::string(call, argument, 1)? {
                Converted::Ready(string) => out.push(string),
                Converted::Asked(answer) => return Ok(Err(answer)),
            },
            None => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
        }
    }
    Ok(Ok(out))
}

/// `add(...tokens)`.
fn add(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = this(call, "add")?;
    let tokens = match every_token(call)? {
        Ok(tokens) => tokens,
        Err(asked) => return Ok(asked),
    };
    if let Err(invalid) = tokens::validate(tokens.iter().map(String::as_str)) {
        return Err(refused(call, this, invalid));
    }
    let had = class(call, this)?;
    let set = tokens::added(had.as_deref().unwrap_or_default(), &tokens);
    update(call, this, had.is_some(), &set)?;
    Ok(Answer::Value(Value::Undefined))
}

/// `remove(...tokens)`.
fn remove(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = this(call, "remove")?;
    let tokens = match every_token(call)? {
        Ok(tokens) => tokens,
        Err(asked) => return Ok(asked),
    };
    if let Err(invalid) = tokens::validate(tokens.iter().map(String::as_str)) {
        return Err(refused(call, this, invalid));
    }
    let had = class(call, this)?;
    let set = tokens::removed(had.as_deref().unwrap_or_default(), &tokens);
    update(call, this, had.is_some(), &set)?;
    Ok(Answer::Value(Value::Undefined))
}

/// `toggle(token, force)`.
fn toggle(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = this(call, "toggle")?;
    idl::needs(call, 1, "toggle")?;
    let token = match idl::only_string(call)? {
        Converted::Ready(token) => token,
        Converted::Asked(asked) => return Ok(asked),
    };
    if let Err(invalid) = tokens::validate([token.as_str()]) {
        return Err(refused(call, this, invalid));
    }
    // An optional boolean: `undefined` is no `force` at all.
    let force = match call.argument(1) {
        Value::Undefined => None,
        given => Some(convert::to_boolean(call.seen(), given)),
    };
    let had = class(call, this)?;
    let toggled = tokens::toggled(had.as_deref().unwrap_or_default(), &token, force);
    if let Some(set) = toggled.write {
        update(call, this, had.is_some(), &set)?;
    }
    Ok(Answer::Value(Value::Bool(toggled.present)))
}
