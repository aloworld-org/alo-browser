/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A thrown value in words, **without running any script** (queue item 239).
//!
//! A report is written as the throw happens, from inside the loop, and the
//! loop may not run script to write it: a getter called to describe an error
//! is a call the page can count, and one that throws or never ends would be a
//! second failure made by the act of reporting the first. So this reads the
//! heap and nothing else.
//!
//! # An error object
//!
//! An object with the `[[ErrorData]]` slot — anything an `Error` constructor
//! made — is said the way `Error.prototype.toString` would say it: its
//! `name`, defaulting to `Error`, then `": "` and its `message`, either left
//! out when it is empty. Both are looked up along the prototype chain, so a
//! page that reassigned `name` is said by the new name. Only a **data**
//! property is read. A getter is never called and an object is never
//! converted — either would run the page's own code — and a symbol, which
//! `ToString` refuses, is not spelled; each is said in brackets instead.
//! What `Error.prototype.toString` itself is, on the object, does not matter:
//! a page replacing it would otherwise choose what its own failure says.
//!
//! # A `DOMException`
//!
//! What a DOM member throws when `alo-dom` refuses its change (ADR 0017 § 5)
//! is not an error object to the engine: it is `alo-bindings`' embedder
//! cell, with `Error.prototype` on its chain but no `[[ErrorData]]`. It is
//! said as `name: message` — `HierarchyRequestError: a document cannot be
//! put inside anything` — read **from the cell's own two slots**, which are
//! what its `name` and `message` getters answer and which nothing a page
//! does can change. Not from the properties along its chain: those are
//! getters, which would run code, and a page that deleted or replaced them
//! would otherwise choose what its own failure says — the same reason an
//! error's `toString` is not consulted. Which object is one is asked by
//! type, never by its prototype, so an object that merely inherits from
//! `DOMException.prototype` is still `an object` (queue item 252).
//!
//! Any other object is still `an object`: describing one means choosing
//! which of its properties to trust, and nothing yet says which (item 78).
//!
//! # How long
//!
//! A page decides how long its strings are, up to the engine's own bound of
//! some 268 million code units, and every report crosses the boundary in one
//! message whose size is capped. So no string is said past
//! [`LONGEST_SAID`] code units; the rest is counted rather than copied.

use alo_bindings::DomException;
use alo_js::convert::{self, Primitive};
use alo_js::heap::Ref;
use alo_js::numeric;
use alo_js::object::{Found, Objects, Value};

/// The most code units of any one string a page made that a report repeats.
pub const LONGEST_SAID: usize = 1024;

/// A value nothing caught, in words.
pub fn thrown(objects: &Objects, value: Value) -> String {
    match value {
        Value::Undefined => "undefined".to_owned(),
        Value::Null => "null".to_owned(),
        Value::Bool(is) => is.to_string(),
        Value::Number(number) => numeric::text_of(number),
        Value::Text(held) => match objects.units(held) {
            Some(units) => quoted(units),
            None => "a string that has gone".to_owned(),
        },
        Value::Symbol(_) => "a symbol".to_owned(),
        Value::Object(held) => match objects.embedded::<DomException>(held) {
            Some(exception) => format!("{}: {}", exception.name(), exception.message()),
            None if objects.is_error(held) => error(objects, held),
            None => "an object".to_owned(),
        },
    }
}

/// What reading one of an error's two properties without a call found.
enum Part {
    /// Nothing, or `undefined`: the default stands.
    Absent,
    /// A primitive, spelled as `ToString` spells it.
    Said(String),
    /// Something only script could spell, and why it was not.
    Unsaid(&'static str),
}

/// An error object, as `Error.prototype.toString` would say it, with what it
/// could not say without a call named after it.
fn error(objects: &Objects, held: Ref) -> String {
    let name = part(objects, held, "name");
    let message = part(objects, held, "message");
    let mut unsaid = Vec::new();
    let name = match name {
        Part::Absent => "Error".to_owned(),
        Part::Said(name) => name,
        Part::Unsaid(why) => {
            unsaid.push(format!("its name is {why}"));
            "an error".to_owned()
        }
    };
    let message = match message {
        Part::Absent => String::new(),
        Part::Said(message) => message,
        Part::Unsaid(why) => {
            unsaid.push(format!("its message is {why}"));
            String::new()
        }
    };
    let mut said = match (name.is_empty(), message.is_empty()) {
        (true, true) => "an error with an empty name and message".to_owned(),
        (true, false) => message,
        (false, true) => name,
        (false, false) => format!("{name}: {message}"),
    };
    if !unsaid.is_empty() {
        said.push_str(" (");
        said.push_str(&unsaid.join("; "));
        said.push(')');
    }
    said
}

/// One of an error's properties, read along its prototype chain without
/// calling anything.
fn part(objects: &Objects, held: Ref, name: &str) -> Part {
    let units: Vec<u16> = name.encode_utf16().collect();
    // A name no object has was never interned, and interning one here would
    // allocate; nothing has it, so nothing along this chain does either.
    let Some(key) = objects.existing_key(&units) else {
        return Part::Absent;
    };
    let value = match objects.get(held, key) {
        Ok(Found::Missing | Found::Getter(Value::Undefined)) => return Part::Absent,
        Ok(Found::Value(value)) => value,
        Ok(Found::Getter(_)) => return Part::Unsaid("a getter, which was not called"),
        Err(_) => return Part::Unsaid("something this engine could not read"),
    };
    match value {
        Value::Undefined => Part::Absent,
        Value::Object(_) => Part::Unsaid("an object, which was not converted"),
        Value::Symbol(_) => Part::Unsaid("a symbol"),
        primitive => match Primitive::of(primitive)
            .map(|primitive| convert::to_units(objects, primitive, 0))
        {
            Some(Ok(units)) => Part::Said(plain(&units)),
            _ => Part::Unsaid("something this engine could not read"),
        },
    }
}

/// A string's code units in quotes, as a thrown string is said, cut at
/// [`LONGEST_SAID`].
fn quoted(units: &[u16]) -> String {
    let (kept, more) = cut(units);
    let mut said = format!("{:?}", String::from_utf16_lossy(kept));
    said.push_str(&more);
    said
}

/// A string's code units as they are, cut at [`LONGEST_SAID`].
fn plain(units: &[u16]) -> String {
    let (kept, more) = cut(units);
    let mut said = String::from_utf16_lossy(kept);
    said.push_str(&more);
    said
}

/// The first [`LONGEST_SAID`] code units, never ending half way through a
/// surrogate pair, and what to say about the rest.
fn cut(units: &[u16]) -> (&[u16], String) {
    if units.len() <= LONGEST_SAID {
        return (units, String::new());
    }
    let mut end = LONGEST_SAID;
    if units
        .get(end.saturating_sub(1))
        .is_some_and(|unit| (0xD800..0xDC00).contains(unit))
    {
        end = end.saturating_sub(1);
    }
    let kept = units.get(..end).unwrap_or(units);
    let left = units.len().saturating_sub(kept.len());
    (kept, format!("… and {left} more code units"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_js::object::Property;

    fn units(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn a_short_string_is_said_whole() {
        assert_eq!(quoted(&units("no")), "\"no\"");
        assert_eq!(plain(&units("no")), "no");
    }

    #[test]
    fn a_long_string_is_cut_and_the_rest_counted() {
        let long = units(&"x".repeat(LONGEST_SAID + 5));
        let said = plain(&long);
        assert_eq!(said.chars().filter(|&it| it == 'x').count(), LONGEST_SAID);
        assert!(said.ends_with("… and 5 more code units"), "{said}");
        let said = quoted(&long);
        assert!(said.starts_with('"'), "{said}");
        assert!(said.ends_with("\"… and 5 more code units"), "{said}");
    }

    #[test]
    fn a_cut_never_splits_a_surrogate_pair() {
        // A run of "x" so that the pair 😀 straddles the cut.
        let mut text = "x".repeat(LONGEST_SAID - 1);
        text.push('😀');
        let said = plain(&units(&text));
        assert!(!said.contains('\u{FFFD}'), "half a pair was kept");
        assert!(said.ends_with("… and 2 more code units"), "{said}");
    }

    #[test]
    fn a_symbol_name_is_not_spelled() {
        // No page can name a symbol yet (there is no `Symbol` global, item
        // 73), so the error is made here, in the heap, as a page would make it.
        let mut objects = Objects::new();
        let Ok(made) = objects.error(None) else {
            panic!("an empty heap holds one error");
        };
        let Ok(symbol) = objects.symbol(None) else {
            panic!("and one symbol");
        };
        let Ok(message) = objects.text(units("x")) else {
            panic!("and one string");
        };
        for (name, value) in [
            ("name", Value::Symbol(symbol)),
            ("message", Value::Text(message)),
        ] {
            let Ok(key) = objects.key(&units(name)) else {
                panic!("and its names");
            };
            assert_eq!(
                objects.define(made, key, Property::data(value, true, false, true)),
                Ok(true)
            );
        }
        assert_eq!(
            thrown(&objects, Value::Object(made)),
            "an error: x (its name is a symbol)"
        );
    }

    #[test]
    fn a_name_no_object_has_was_never_asked_for() {
        // Nothing in this heap has a `name` or a `message`, so neither is
        // interned — and describing the error interns neither.
        let mut objects = Objects::new();
        let Ok(made) = objects.error(None) else {
            panic!("an empty heap holds one error");
        };
        assert_eq!(objects.interned(), 0);
        assert_eq!(thrown(&objects, Value::Object(made)), "Error");
        assert_eq!(objects.interned(), 0);
    }

    #[test]
    fn primitives_are_said_as_themselves() {
        let objects = Objects::new();
        assert_eq!(thrown(&objects, Value::Number(4.0)), "4");
        assert_eq!(thrown(&objects, Value::Null), "null");
        assert_eq!(thrown(&objects, Value::Undefined), "undefined");
        assert_eq!(thrown(&objects, Value::Bool(false)), "false");
    }
}
