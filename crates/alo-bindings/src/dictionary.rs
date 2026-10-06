/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Web IDL dictionaries, read one member at a time (queue item 254).
//!
//! `new Event('x', { bubbles: true })` and `addEventListener('x', f, {
//! once: true })` each take a dictionary, and converting one is a `Get` of
//! each member in order — inherited dictionaries' members first, each
//! dictionary's in lexicographic order — any of which may be a getter that
//! runs the page's script. A native asks the interpreter for that call and
//! is run again at a step it named, keeping nothing else, so a member here
//! is either **its value** or **the call to ask for**; the caller keeps what
//! it has converted so far in its own instance or in its step number.
//!
//! A member the object does not have, or has as `undefined`, is *not
//! present*, and its default — or its absence — is the caller's.

use alo_js::heap::Ref;
use alo_js::object::Found;
use alo_js::object::native::{Answer, Call, Want};
use alo_js::{Escape, Value};

/// A dictionary member, read.
#[derive(Debug)]
pub(crate) enum Member {
    /// Its value — `undefined` when it is not present.
    Value(Value),
    /// Its getter must be called first: ask for this, and the answer is the
    /// member's value.
    Ask(Answer),
}

/// The dictionary argument `which` of `member`: [`None`] for `undefined` or
/// `null`, which convert to a dictionary with nothing present, and the object
/// otherwise.
///
/// # Errors
///
/// A `TypeError` for anything else, as Web IDL's conversion throws.
pub(crate) fn argument(
    call: &Call<'_>,
    which: usize,
    member: &'static str,
) -> Result<Option<Ref>, Escape> {
    match call.argument(which) {
        Value::Undefined | Value::Null => Ok(None),
        Value::Object(held) => Ok(Some(held)),
        _ => Err(Escape::type_error(
            format!(
                "argument {} to '{member}' is not a dictionary",
                which.saturating_add(1)
            ),
            call.at(),
        )),
    }
}

/// Member `name` of `dictionary`, or the getter call to ask for, coming back
/// at `step`.
///
/// Allocates nothing: a name never interned is a name no object has a
/// property under, so it is not present.
///
/// # Errors
///
/// A fault for a dictionary this engine has lost.
pub(crate) fn member(
    call: &Call<'_>,
    dictionary: Ref,
    name: &str,
    step: u32,
) -> Result<Member, Escape> {
    let units: Vec<u16> = name.encode_utf16().collect();
    let Some(key) = call.seen().existing_key(&units) else {
        return Ok(Member::Value(Value::Undefined));
    };
    Ok(match call.seen().get(dictionary, key)? {
        Found::Missing | Found::Getter(Value::Undefined) => Member::Value(Value::Undefined),
        Found::Value(value) => Member::Value(value),
        Found::Getter(getter) => Member::Ask(Answer::want(
            Want::Call {
                callee: getter,
                receiver: Value::Object(dictionary),
                arguments: Vec::new(),
            },
            step,
        )),
    })
}
