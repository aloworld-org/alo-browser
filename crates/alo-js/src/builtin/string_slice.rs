/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `String.prototype.slice(start, end)` (ECMA-262 § 22.1.3.23, queue item
//! 488).
//!
//! alo Sites' analytics script cuts `/f/` off a form's action with it. It is
//! three conversions and then a cut: `this` to a string, `start` to an
//! integer, and `end` to one unless it is `undefined`. Each of the three may
//! be an object whose `toString` or `valueOf` is the page's script, so each
//! is asked for (ADR 0031) and what it came to is kept: the string in slot
//! 0 at [`STRING_CONVERTED`], the numbers in slots 1 and 2 by
//! [`numbers`](super::numbers). A page can count the calls, and they come
//! in that order, once each.
//!
//! A negative index counts from the end, an index past either end is held
//! at it, and a `start` at or past `end` is the empty string. The cut is of
//! UTF-16 code units, so it may split a pair and leave a lone surrogate,
//! which is a string like any other.

use crate::abrupt::{Escape, Internal};
use crate::convert::{self, Hint, Primitive};
use crate::object::Value;
use crate::object::array::exact_length;
use crate::object::native::{Answer, Call, Native, Want};
use crate::time::to_integer;

use super::numbers::{self, Numbers, numbers};

/// `slice`, keeping the string and the two numbers.
pub(super) const SLICE: Native = Native::new("slice", slice).keeping(3);

/// The slot the string `this` came to is kept in.
const STRING: usize = 0;

/// The slot `start` is kept in, `end` being the one after it.
const START: usize = 1;

/// The step `slice` comes back at with `this` made a primitive.
const STRING_CONVERTED: u32 = 1;

/// `String.prototype.slice(start, end)`.
fn slice(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let at = call.at();
    // Below the numbers' own steps, `this` has not been kept yet.
    if call.step() < numbers::CONVERTED {
        let primitive = match call.step() {
            0 => match call.this() {
                Value::Undefined | Value::Null => {
                    return Err(Escape::type_error(
                        "String.prototype.slice was called on null or undefined",
                        at,
                    ));
                }
                Value::Object(held) => {
                    return Ok(Answer::want(
                        Want::Primitive {
                            of: Value::Object(held),
                            hint: Hint::String,
                        },
                        STRING_CONVERTED,
                    ));
                }
                other => Primitive::of(other),
            },
            // A conversion answers a primitive or throws, so anything else
            // here is this engine having resumed the wrong builtin.
            STRING_CONVERTED => Primitive::of(call.answer()?),
            _ => None,
        }
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
        // A symbol is the `TypeError` `ToString` gives it.
        let string = convert::to_text(call.objects(), primitive, at)?;
        call.keep(STRING, string)?;
    }
    // `end` is read only when it is there: `undefined` is the whole length,
    // not `ToIntegerOrInfinity(undefined)`, which would be zero.
    let count = if call.argument(1) == Value::Undefined {
        1
    } else {
        2
    };
    if let Numbers::Asked(asked) = numbers(call, count, START)? {
        return Ok(asked);
    }

    let Value::Text(string) = call.kept(STRING)? else {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    };
    let units = call
        .seen()
        .units(string)
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    let length = f64::from(
        u32::try_from(units.len()).map_err(|_| Escape::Broken(Internal::BuiltinIsWrong))?,
    );
    let from = held_within(to_integer(call.kept_number(START)?), length);
    let to = if count == 2 {
        held_within(to_integer(call.kept_number(START + 1)?), length)
    } else {
        length
    };
    let cut = if from < to {
        let (Some(from), Some(to)) = (index(from), index(to)) else {
            return Err(Escape::Broken(Internal::BuiltinIsWrong));
        };
        units
            .get(from..to)
            .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?
            .to_vec()
    } else {
        Vec::new()
    };
    let text = call
        .objects()
        .text(cut)
        .map_err(|why| Escape::refused(why, at))?;
    Ok(Answer::Value(Value::Text(text)))
}

/// An integer index, counted from the end when it is negative and held
/// within `0..=length`: `slice`'s steps 5 and 7.
fn held_within(index: f64, length: f64) -> f64 {
    if index < 0.0 {
        (length + index).max(0.0)
    } else {
        index.min(length)
    }
}

/// A whole number within a string's length, as an index into its units.
fn index(at: f64) -> Option<usize> {
    exact_length(at).and_then(|at| usize::try_from(at).ok())
}

#[cfg(test)]
mod tests {
    use super::held_within;

    #[test]
    fn an_index_counts_from_the_end_when_negative_and_is_held_within_the_string() {
        for (index, held) in [
            (0.0, 0.0),
            (3.0, 3.0),
            (9.0, 5.0),
            (-2.0, 3.0),
            (-9.0, 0.0),
            (f64::INFINITY, 5.0),
            (f64::NEG_INFINITY, 0.0),
        ] {
            let held: f64 = held;
            assert_eq!(held_within(index, 5.0).to_bits(), held.to_bits(), "{index}");
        }
    }
}
