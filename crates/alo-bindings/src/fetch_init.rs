/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `fetch(input, init)`'s arguments, converted (ADR 0032 § 2, queue item
//! 335).
//!
//! Web IDL converts both before any of Fetch's own steps run: `input` with
//! `ToString` — no page can make a `Request`, so every object is a string
//! here — then `init` as a `RequestInit`, **one member at a time in
//! lexicographic order**, each a `Get` that may be a getter running the
//! page's script and each converted before the next is read. A native keeps
//! a step number and its slots and nothing else (ADR 0031), so a member is
//! read at one step and, when its getter or its `toString` must run first,
//! finished at the next.
//!
//! What a member converts to is kept in the request steps' own slots as a
//! string, already the value Web IDL would hand over, for
//! [`crate::fetch`] to read when every member is in. Enumerations are
//! checked here, because Web IDL checks them: `mode: "sideways"` is a
//! `TypeError` before Fetch has been asked anything.
//!
//! # What is read, and what is refused by name
//!
//! `body` as a string; `credentials`, `mode`, `redirect` and
//! `referrerPolicy`; `method` as a `ByteString`; and `headers` as a record —
//! a plain object of names to values. The rest are refused by name when a
//! page sets them to anything but their default, rather than ignored: a
//! `signal` nobody honours is a cancel button that does nothing, and a
//! `cache` mode nobody honours is a fetch that behaves otherwise than asked.
//! So are a body that is not a string, headers given as a list of pairs, a
//! header behind a getter or whose value is an object, and a header value
//! with a byte past `0x7F`, which this engine holds as text and so could not
//! send as the byte the page meant.

use alo_js::abrupt::{Internal, Missing};
use alo_js::convert::{self, Primitive};
use alo_js::heap::Ref;
use alo_js::object::native::{Answer, Call};
use alo_js::object::symbol::WellKnown;
use alo_js::object::{Found, Key};
use alo_js::{Escape, Value};

use crate::dictionary::{self, Member};
use crate::headers::Headers;
use crate::idl::{self, Converted};

/// The request steps' slots: the input, as a string.
pub(crate) const INPUT: usize = 0;
/// The body, a string, or `undefined` for none.
pub(crate) const BODY: usize = 1;
/// `credentials`, checked.
pub(crate) const CREDENTIALS: usize = 2;
/// `headers`, as [`encode`] writes them.
pub(crate) const HEADERS: usize = 3;
/// `method`, a `ByteString`.
pub(crate) const METHOD: usize = 4;
/// `mode`, checked.
pub(crate) const MODE: usize = 5;
/// `redirect`, checked.
pub(crate) const REDIRECT: usize = 6;
/// `referrerPolicy`, checked.
pub(crate) const REFERRER_POLICY: usize = 7;
/// How many slots the request steps keep.
pub(crate) const SLOTS: usize = 8;

/// The first step: nothing converted yet.
const START: u32 = 0;
/// `input`'s `toString` has answered.
const INPUT_CONVERTED: u32 = 1;

/// Where reading has got to.
#[derive(Debug)]
pub(crate) enum Reading {
    /// Something must run first: ask for it.
    Asked(Answer),
    /// Every argument is converted and kept.
    Done,
}

/// A `RequestInit` member this engine reads or refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Init {
    Body,
    Cache,
    Credentials,
    Headers,
    Integrity,
    Keepalive,
    Method,
    Mode,
    Priority,
    Redirect,
    Referrer,
    ReferrerPolicy,
    Signal,
    Window,
}

/// Every member, in the lexicographic order Web IDL reads them in.
const MEMBERS: [Init; 14] = [
    Init::Body,
    Init::Cache,
    Init::Credentials,
    Init::Headers,
    Init::Integrity,
    Init::Keepalive,
    Init::Method,
    Init::Mode,
    Init::Priority,
    Init::Redirect,
    Init::Referrer,
    Init::ReferrerPolicy,
    Init::Signal,
    Init::Window,
];

impl Init {
    /// Its name, as a page writes it.
    const fn name(self) -> &'static str {
        match self {
            Init::Body => "body",
            Init::Cache => "cache",
            Init::Credentials => "credentials",
            Init::Headers => "headers",
            Init::Integrity => "integrity",
            Init::Keepalive => "keepalive",
            Init::Method => "method",
            Init::Mode => "mode",
            Init::Priority => "priority",
            Init::Redirect => "redirect",
            Init::Referrer => "referrer",
            Init::ReferrerPolicy => "referrerPolicy",
            Init::Signal => "signal",
            Init::Window => "window",
        }
    }

    /// The values its enumeration allows, for a member that is one.
    const fn allowed(self) -> &'static [&'static str] {
        match self {
            Init::Credentials => &["omit", "same-origin", "include"],
            Init::Mode => &["navigate", "same-origin", "no-cors", "cors"],
            Init::Redirect => &["follow", "error", "manual"],
            Init::ReferrerPolicy => &[
                "",
                "no-referrer",
                "no-referrer-when-downgrade",
                "same-origin",
                "origin",
                "strict-origin",
                "origin-when-cross-origin",
                "strict-origin-when-cross-origin",
                "unsafe-url",
            ],
            _ => &[],
        }
    }

    /// Where a string member is kept.
    const fn slot(self) -> Option<usize> {
        match self {
            Init::Credentials => Some(CREDENTIALS),
            Init::Method => Some(METHOD),
            Init::Mode => Some(MODE),
            Init::Redirect => Some(REDIRECT),
            Init::ReferrerPolicy => Some(REFERRER_POLICY),
            _ => None,
        }
    }

    /// The enumeration's name, for the message.
    const fn enumeration(self) -> &'static str {
        match self {
            Init::Credentials => "RequestCredentials",
            Init::Mode => "RequestMode",
            Init::Redirect => "RequestRedirect",
            _ => "ReferrerPolicy",
        }
    }
}

/// The step that comes back with member `index`'s value from its getter.
fn got(index: usize) -> u32 {
    // At most 14 members: neither the conversion nor the arithmetic fails.
    u32::try_from(index).map_or(u32::MAX, |index| index.saturating_mul(2).saturating_add(2))
}

/// The step that comes back with member `index`'s value converted.
fn converted(index: usize) -> u32 {
    got(index).saturating_add(1)
}

/// Convert `fetch`'s arguments, as far as they can be without asking for
/// something, keeping each in its slot.
///
/// # Errors
///
/// A `TypeError` for an argument Web IDL refuses, and a refusal by name for
/// what this engine has not built; and this crate's bug at a step it never
/// named.
pub(crate) fn read(call: &mut Call<'_>) -> Result<Reading, Escape> {
    let from = match call.step() {
        START => {
            match idl::string(call, call.argument(0), INPUT_CONVERTED)? {
                Converted::Ready(input) => keep_text(call, INPUT, &input)?,
                Converted::Asked(answer) => return Ok(Reading::Asked(answer)),
            }
            0
        }
        INPUT_CONVERTED => {
            let input = idl::answered_string(call)?;
            keep_text(call, INPUT, &input)?;
            0
        }
        step => {
            let after = step
                .checked_sub(2)
                .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
            let index =
                usize::try_from(after / 2).map_err(|_| Escape::Broken(Internal::BuiltinIsWrong))?;
            let member = *MEMBERS
                .get(index)
                .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
            if after % 2 == 0 {
                let value = call.answer()?;
                if let Some(answer) = take(call, member, index, value)? {
                    return Ok(Reading::Asked(answer));
                }
            } else {
                let text = idl::answered_string(call)?;
                checked(call, member, &text)?;
            }
            index.saturating_add(1)
        }
    };
    let Some(init) = dictionary::argument(call, 1, "fetch")? else {
        return Ok(Reading::Done);
    };
    for (index, member) in MEMBERS.iter().enumerate().skip(from) {
        let value = match dictionary::member(call, init, member.name(), got(index))? {
            Member::Ask(answer) => return Ok(Reading::Asked(answer)),
            Member::Value(value) => value,
        };
        if let Some(answer) = take(call, *member, index, value)? {
            return Ok(Reading::Asked(answer));
        }
    }
    Ok(Reading::Done)
}

/// A refusal by name of `what`, which this engine has not built.
fn not_built(what: &'static str) -> Escape {
    Escape::NotBuiltYet(Missing::InTheEmbedder(what))
}

/// What to do with member `member`'s value: keep it, refuse it, or ask for
/// its conversion and come back.
fn take(
    call: &mut Call<'_>,
    member: Init,
    index: usize,
    value: Value,
) -> Result<Option<Answer>, Escape> {
    let is = |wanted: &str| text_is(call, value, wanted);
    match member {
        Init::Body => match value {
            Value::Undefined | Value::Null => {}
            Value::Text(_) => call.keep(BODY, value)?,
            _ => {
                return Err(not_built(
                    "a fetch body that is not a string is not built (ADR 0032, What this does not \
                     decide)",
                ));
            }
        },
        Init::Cache if !(value == Value::Undefined || is("default")) => {
            return Err(not_built(
                "fetch's 'cache' is not built (ADR 0032, What this does not decide)",
            ));
        }
        Init::Integrity if !(value == Value::Undefined || is("")) => {
            return Err(not_built(
                "fetch's 'integrity' is not built (ADR 0032, What this does not decide)",
            ));
        }
        Init::Keepalive if convert::to_boolean(call.seen(), value) => {
            return Err(not_built(
                "a fetch that outlives its document is not built (ADR 0032, What this does not \
                 decide)",
            ));
        }
        Init::Priority if !(value == Value::Undefined || is("auto")) => {
            return Err(not_built(
                "fetch's 'priority' is not built (ADR 0032, What this does not decide)",
            ));
        }
        Init::Referrer if !(value == Value::Undefined || is("about:client")) => {
            return Err(not_built(
                "fetch's 'referrer' is not built (ADR 0032, What this does not decide)",
            ));
        }
        Init::Signal if !matches!(value, Value::Undefined | Value::Null) => {
            return Err(not_built(
                "a fetch's 'signal' is not built (ADR 0032, What this does not decide)",
            ));
        }
        Init::Window if !matches!(value, Value::Undefined | Value::Null) => {
            return Err(Escape::type_error(
                "a fetch's 'window' can only be null",
                call.at(),
            ));
        }
        Init::Headers => match value {
            Value::Undefined => {}
            Value::Object(record) => {
                let encoded = headers(call, record)?;
                keep_text(call, HEADERS, &encoded)?;
            }
            _ => {
                return Err(Escape::type_error(
                    "a fetch's 'headers' is not an object of names and values",
                    call.at(),
                ));
            }
        },
        Init::Credentials | Init::Method | Init::Mode | Init::Redirect | Init::ReferrerPolicy
            if value != Value::Undefined =>
        {
            match idl::string(call, value, converted(index))? {
                Converted::Ready(text) => checked(call, member, &text)?,
                Converted::Asked(answer) => return Ok(Some(answer)),
            }
        }
        _ => {}
    }
    Ok(None)
}

/// Whether `value` is the string `wanted`.
fn text_is(call: &Call<'_>, value: Value, wanted: &str) -> bool {
    match value {
        Value::Text(held) => call
            .seen()
            .units(held)
            .is_some_and(|units| units.iter().copied().eq(wanted.encode_utf16())),
        _ => false,
    }
}

/// Keep a string member, converted, if Web IDL accepts it.
///
/// # Errors
///
/// A `TypeError` for a value its enumeration does not have, and for a
/// method that is not a `ByteString`.
fn checked(call: &mut Call<'_>, member: Init, text: &str) -> Result<(), Escape> {
    let slot = member
        .slot()
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    if member == Init::Method {
        if !is_byte_string(text) {
            return Err(Escape::type_error(
                "a fetch's 'method' is not a ByteString",
                call.at(),
            ));
        }
    } else if !member.allowed().contains(&text) {
        return Err(Escape::type_error(
            format!("{text:?} is not a {}", member.enumeration()),
            call.at(),
        ));
    }
    keep_text(call, slot, text)
}

/// Keep `text` in slot `which` as a string in the heap.
fn keep_text(call: &mut Call<'_>, which: usize, text: &str) -> Result<(), Escape> {
    let at = call.at();
    let made = call
        .objects()
        .text(text.encode_utf16().collect())
        .map_err(|why| Escape::refused(why, at))?;
    call.keep(which, Value::Text(made))
}

/// Whether every character of `text` is a byte: Web IDL's `ByteString`.
fn is_byte_string(text: &str) -> bool {
    text.chars().all(|unit| u32::from(unit) <= 0xFF)
}

/// `headers` as a record of `ByteString`s, encoded for its slot.
///
/// Web IDL's union picks the sequence form when the object has a
/// `Symbol.iterator` method, and that form — a list of pairs, or another
/// `Headers` — is refused by name. A record's own enumerable string-keyed
/// properties are read in order; a getter among them, or a value that is an
/// object, would need a call per header and is refused by name.
///
/// # Errors
///
/// A `TypeError` for a name or value that is not a `ByteString` or a symbol
/// value, and a refusal by name for the forms above.
fn headers(call: &Call<'_>, record: Ref) -> Result<String, Escape> {
    let at = call.at();
    let iterator = call
        .intrinsics()?
        .well_known_key(call.seen(), WellKnown::Iterator)?;
    let listed = match call.seen().get(record, iterator)? {
        Found::Missing | Found::Value(Value::Undefined | Value::Null) => false,
        Found::Getter(_) | Found::Value(_) => true,
    };
    if listed || call.seen().embedded::<Headers>(record).is_some() {
        return Err(not_built(
            "a fetch's headers given as a list of pairs or as a Headers are not built (ADR 0032, \
             What this does not decide)",
        ));
    }
    let mut pairs = Vec::new();
    for key in call.seen().own_keys(record)? {
        let Some(name) = key_text(call, key) else {
            continue;
        };
        let Some(property) = call.seen().own_property(record, key)? else {
            continue;
        };
        if !property.is_enumerable() {
            continue;
        }
        let value = property.value().ok_or_else(|| {
            not_built(
                "a fetch header behind a getter is not built: it needs a call per header, and \
                 waits for a page that does it",
            )
        })?;
        let Some(primitive) = Primitive::of(value) else {
            return Err(not_built(
                "a fetch header whose value is an object is not built: it needs a call per \
                 header, and waits for a page that does it",
            ));
        };
        let value = String::from_utf16_lossy(&convert::to_units(call.seen(), primitive, at)?);
        if !is_byte_string(&name) || !is_byte_string(&value) {
            return Err(Escape::type_error(
                format!("the fetch header {name:?} is not a ByteString"),
                at,
            ));
        }
        pairs.push((name, value));
    }
    Ok(encode(&pairs))
}

/// A string-keyed property's name, or [`None`] for a symbol.
fn key_text(call: &Call<'_>, key: Key) -> Option<String> {
    if let Some(index) = key.as_index() {
        return Some(index.to_string());
    }
    let held = key.as_text()?;
    call.seen().units(held).map(String::from_utf16_lossy)
}

/// Header pairs as one string: each name and value as its length in
/// characters, a colon, and itself — so that nothing a page writes in either
/// can be read back as a boundary.
pub(crate) fn encode(pairs: &[(String, String)]) -> String {
    let mut out = String::new();
    for (name, value) in pairs {
        for part in [name, value] {
            out.push_str(&part.chars().count().to_string());
            out.push(':');
            out.push_str(part);
        }
    }
    out
}

/// What [`encode`] wrote, read back — [`None`] for anything it did not
/// write, which is this crate's bug.
pub(crate) fn decode(encoded: &str) -> Option<Vec<(String, String)>> {
    let mut parts = Vec::new();
    let mut rest = encoded;
    while !rest.is_empty() {
        let (count, after) = rest.split_once(':')?;
        let count: usize = count.parse().ok()?;
        let end = after
            .char_indices()
            .nth(count)
            .map_or(after.len(), |(at, _)| at);
        if after.get(..end)?.chars().count() != count {
            return None;
        }
        parts.push(after.get(..end)?.to_owned());
        rest = after.get(end..)?;
    }
    if parts.len() % 2 != 0 {
        return None;
    }
    let mut pairs = Vec::new();
    let mut parts = parts.into_iter();
    while let (Some(name), Some(value)) = (parts.next(), parts.next()) {
        pairs.push((name, value));
    }
    Some(pairs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members_are_read_in_lexicographic_order() {
        let names: Vec<&str> = MEMBERS.iter().map(|member| member.name()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
    }

    #[test]
    fn a_member_comes_back_at_its_own_two_steps() {
        let steps: Vec<u32> = (0..MEMBERS.len())
            .flat_map(|index| [got(index), converted(index)])
            .collect();
        let expected: Vec<u32> = (2..2 + 2 * 14).collect();
        assert_eq!(steps, expected);
    }

    #[test]
    fn headers_round_trip_whatever_they_hold() {
        let pairs = vec![
            ("x-a".to_owned(), "1:2:3".to_owned()),
            ("x-é".to_owned(), String::new()),
            ("4:".to_owned(), "é:é".to_owned()),
        ];
        assert_eq!(decode(&encode(&pairs)), Some(pairs));
        assert_eq!(decode(""), Some(Vec::new()));
        for broken in ["3:ab", "x:y", "1:a", "9:"] {
            assert_eq!(decode(broken), None, "{broken:?}");
        }
    }

    #[test]
    fn a_byte_string_is_every_character_at_most_ff() {
        assert!(is_byte_string("GET ÿ"));
        assert!(!is_byte_string("Ā"));
    }
}
