/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What an operator does to two values.
//!
//! Every one of these is written in terms of [`convert`], which is why that is
//! a file of its own: `a < b`, `a == b` and `a + b` differ from each other
//! almost entirely in *which* conversion they ask for and in what order, and an
//! engine that inlined the conversions into the operators would have three
//! copies of `ToPrimitive`'s ordering rule to keep in step.
//!
//! # The three that are famous for being subtle, and are written out here
//!
//! **`+` is not addition.** It is `ToPrimitive` on both sides with no hint, and
//! then *either* concatenation or addition depending on what came back — so
//! `1 + "1"` is `"11"` and `1 + null` is `1`. The order matters and is
//! specified: both sides become primitives *before* either is asked whether it
//! is a string.
//!
//! **`<` evaluates left to right and answers three things.** Less, not less,
//! and *undefined* — which is what `NaN < 1` is, and which is why `a > b` is
//! not `!(a <= b)`. The three-valued answer is [`Option<bool>`] here rather than
//! a `bool` with a convention, because a convention is what an engine gets
//! wrong at four call sites and right at three.
//!
//! **`==` converts and `===` does not.** The loose one is a short list of
//! rules that ends in the strict one, and each rule is one step: a boolean
//! becomes a number, a string meeting a number becomes a number, an object
//! meeting a primitive becomes a primitive. Written as a loop with a bound
//! rather than as recursion with a hope.

use crate::abrupt::Escape;
use crate::ast::{Binary, Unary};
use crate::convert::{self, Hint, Names};
use crate::object::{Objects, Value};

/// The unary operators that are a function of one value and nothing else.
///
/// `typeof` and `delete` are **not** here, and that is the point of the type
/// rather than an omission: `typeof a` must answer for a name that resolves to
/// nothing, and `delete a.b` needs the object and the key rather than the value
/// they produced. Both are instructions of their own, so neither can arrive
/// here by mistake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Simple {
    /// `void a`
    Void,
    /// `!a`
    Not,
    /// `+a`
    Plus,
    /// `-a`
    Minus,
    /// `~a`
    BitNot,
}

impl Simple {
    /// The simple operator this is, or [`None`] for the two that are not.
    pub const fn of(operator: Unary) -> Option<Self> {
        match operator {
            Unary::Void => Some(Self::Void),
            Unary::Not => Some(Self::Not),
            Unary::Plus => Some(Self::Plus),
            Unary::Minus => Some(Self::Minus),
            Unary::BitNot => Some(Self::BitNot),
            Unary::TypeOf | Unary::Delete => None,
        }
    }
}

/// `-a`, `+a`, `!a`, `~a` and `void a`.
///
/// # Errors
///
/// Whatever the conversions refuse.
pub fn unary(
    objects: &Objects,
    names: &Names,
    operator: Simple,
    value: Value,
    at: usize,
) -> Result<Value, Escape> {
    Ok(match operator {
        Simple::Void => Value::Undefined,
        Simple::Not => Value::Bool(!convert::to_boolean(objects, value)),
        Simple::Plus => Value::Number(convert::to_number(objects, names, value, at)?),
        Simple::Minus => Value::Number(-convert::to_number(objects, names, value, at)?),
        Simple::BitNot => {
            let number = convert::to_number(objects, names, value, at)?;
            Value::Number(f64::from(!convert::to_int32(number)))
        }
    })
}

/// `a + b`, `a * b`, `a < b`, `a == b`, `a & b`, `a in b`, `a instanceof b`.
///
/// **This allocates** for a string concatenation, so it is a safepoint.
///
/// # Errors
///
/// Whatever the conversions refuse, a `TypeError` for `in` and `instanceof` on
/// something that is not an object, and a `RangeError` for a string longer than
/// this engine will make.
pub fn binary(
    objects: &mut Objects,
    names: &Names,
    operator: Binary,
    left: Value,
    right: Value,
    at: usize,
) -> Result<Value, Escape> {
    match operator {
        Binary::Add => add(objects, names, left, right, at),
        Binary::Subtract => {
            let (one, other) = numbers(objects, names, left, right, at)?;
            Ok(Value::Number(one - other))
        }
        Binary::Multiply => {
            let (one, other) = numbers(objects, names, left, right, at)?;
            Ok(Value::Number(one * other))
        }
        Binary::Divide => {
            let (one, other) = numbers(objects, names, left, right, at)?;
            // Dividing by zero is an infinity rather than an error: the
            // language has no integer division and no trap.
            Ok(Value::Number(one / other))
        }
        Binary::Remainder => {
            let (one, other) = numbers(objects, names, left, right, at)?;
            // The C `fmod`: the answer takes the sign of the dividend, so
            // `-1 % 2` is `-1` and not `1`.
            Ok(Value::Number(one % other))
        }
        Binary::Power => {
            let (one, other) = numbers(objects, names, left, right, at)?;
            Ok(Value::Number(exponentiate(one, other)))
        }
        Binary::ShiftLeft => {
            let (one, other) = numbers(objects, names, left, right, at)?;
            Ok(Value::Number(f64::from(
                convert::to_int32(one).wrapping_shl(shift_by(other)),
            )))
        }
        Binary::ShiftRight => {
            let (one, other) = numbers(objects, names, left, right, at)?;
            Ok(Value::Number(f64::from(
                convert::to_int32(one).wrapping_shr(shift_by(other)),
            )))
        }
        Binary::ShiftRightUnsigned => {
            let (one, other) = numbers(objects, names, left, right, at)?;
            // The only one whose answer can be above `i32::MAX`: `-1 >>> 0` is
            // 4294967295, which is why it converts to an unsigned integer and
            // the other two do not.
            Ok(Value::Number(f64::from(
                convert::to_uint32(one).wrapping_shr(shift_by(other)),
            )))
        }
        Binary::BitAnd => {
            let (one, other) = integers(objects, names, left, right, at)?;
            Ok(Value::Number(f64::from(one & other)))
        }
        Binary::BitOr => {
            let (one, other) = integers(objects, names, left, right, at)?;
            Ok(Value::Number(f64::from(one | other)))
        }
        Binary::BitXor => {
            let (one, other) = integers(objects, names, left, right, at)?;
            Ok(Value::Number(f64::from(one ^ other)))
        }
        Binary::Less | Binary::Greater | Binary::LessOrEqual | Binary::GreaterOrEqual => {
            relational(objects, names, operator, left, right, at)
        }
        Binary::Equal => Ok(Value::Bool(loosely_equal(objects, names, left, right, at)?)),
        Binary::NotEqual => Ok(Value::Bool(!loosely_equal(
            objects, names, left, right, at,
        )?)),
        Binary::StrictlyEqual => Ok(Value::Bool(strictly_equal(objects, left, right))),
        Binary::StrictlyNotEqual => Ok(Value::Bool(!strictly_equal(objects, left, right))),
        Binary::In => {
            let Value::Object(object) = right else {
                return Err(Escape::type_error(
                    "the right-hand side of 'in' must be an object",
                    at,
                ));
            };
            let key = convert::to_property_key(objects, names, left, at)?;
            Ok(Value::Bool(objects.has(object, key)?))
        }
        Binary::InstanceOf => {
            if !matches!(right, Value::Object(_)) {
                return Err(Escape::type_error(
                    "the right-hand side of 'instanceof' must be an object",
                    at,
                ));
            }
            // `OrdinaryHasInstance` asks whether the right-hand side is
            // callable before it looks at anything else, and nothing in this
            // heap is (queue item 209). So this is the specification's own
            // `TypeError` rather than a refusal of ours: it is the same error a
            // real engine gives for `1 instanceof {}`.
            Err(Escape::type_error(
                "the right-hand side of 'instanceof' is not callable",
                at,
            ))
        }
    }
}

/// Both sides as numbers, left first — which is the order a `valueOf` with a
/// side effect can see.
fn numbers(
    objects: &Objects,
    names: &Names,
    left: Value,
    right: Value,
    at: usize,
) -> Result<(f64, f64), Escape> {
    let one = convert::to_number(objects, names, left, at)?;
    let other = convert::to_number(objects, names, right, at)?;
    Ok((one, other))
}

/// Both sides as `ToInt32`, in the same order.
fn integers(
    objects: &Objects,
    names: &Names,
    left: Value,
    right: Value,
    at: usize,
) -> Result<(i32, i32), Escape> {
    let (one, other) = numbers(objects, names, left, right, at)?;
    Ok((convert::to_int32(one), convert::to_int32(other)))
}

/// How far a shift shifts: the low five bits of its right-hand side, which is
/// what makes `1 << 32` one rather than zero.
fn shift_by(right: f64) -> u32 {
    convert::to_uint32(right) & 31
}

/// `a + b`, which is two operators sharing a spelling.
fn add(
    objects: &mut Objects,
    names: &Names,
    left: Value,
    right: Value,
    at: usize,
) -> Result<Value, Escape> {
    // Both sides become primitives *first*: `1 + {valueOf(){return "a"}}` is a
    // concatenation, and an engine that asked "is either a string" before
    // converting would have made it an addition.
    let one = convert::to_primitive(objects, names, left, Hint::Default, at)?;
    let other = convert::to_primitive(objects, names, right, Hint::Default, at)?;
    if matches!(one, Value::Text(_)) || matches!(other, Value::Text(_)) {
        let mut units = convert::to_units(objects, names, one, at)?;
        units.extend(convert::to_units(objects, names, other, at)?);
        let held = objects
            .text(units)
            .map_err(|why| Escape::refused(why, at))?;
        return Ok(Value::Text(held));
    }
    let one = convert::to_number(objects, names, one, at)?;
    let other = convert::to_number(objects, names, other, at)?;
    Ok(Value::Number(one + other))
}

/// `a ** b`, which is **not** `f64::powf` in two places.
///
/// IEEE 754 says `pow(-1, ±∞)` is 1 and `pow(1, NaN)` is 1, on the reasoning
/// that a magnitude of one is one however far you take it. The language says
/// both are `NaN`, on the reasoning that a page asking for it has made a
/// mistake. Rust follows IEEE, so the two cases where they differ are written
/// out — and they are written out here rather than left to be discovered,
/// because `Math.pow(-1, Infinity)` is one of those numbers a test suite
/// contains and a hand-written test never does.
fn exponentiate(base: f64, exponent: f64) -> f64 {
    if exponent.is_nan() {
        return f64::NAN;
    }
    #[expect(
        clippy::float_cmp,
        reason = "the specification names the exact magnitude one, not a neighbourhood of it"
    )]
    let magnitude_is_one = base.abs() == 1.0;
    if exponent.is_infinite() && magnitude_is_one {
        return f64::NAN;
    }
    base.powf(exponent)
}

/// `<`, `>`, `<=` and `>=`, all four written in terms of one comparison.
fn relational(
    objects: &Objects,
    names: &Names,
    operator: Binary,
    left: Value,
    right: Value,
    at: usize,
) -> Result<Value, Escape> {
    // Which side is *evaluated* first is already decided — the compiler emitted
    // them in order — and what is decided here is which side is **converted**
    // first, which a `valueOf` with a side effect can see.
    let answer = match operator {
        Binary::Less => less_than(objects, names, left, right, true, at)?,
        Binary::Greater => less_than(objects, names, right, left, false, at)?,
        Binary::LessOrEqual => less_than(objects, names, right, left, false, at)?.map(|is| !is),
        _ => less_than(objects, names, left, right, true, at)?.map(|is| !is),
    };
    // `undefined` — which is what a `NaN` on either side produces — is false for
    // all four, and that is why `!(a < b)` is not `a >= b`.
    Ok(Value::Bool(answer.unwrap_or(false)))
}

/// The specification's `IsLessThan`, whose answer is three-valued.
///
/// [`None`] is its `undefined`: a comparison with a `NaN` in it, which every
/// operator turns into `false` and which `<=` must not turn into `true`.
fn less_than(
    objects: &Objects,
    names: &Names,
    left: Value,
    right: Value,
    left_first: bool,
    at: usize,
) -> Result<Option<bool>, Escape> {
    let (one, other) = if left_first {
        let one = convert::to_primitive(objects, names, left, Hint::Number, at)?;
        let other = convert::to_primitive(objects, names, right, Hint::Number, at)?;
        (one, other)
    } else {
        let other = convert::to_primitive(objects, names, right, Hint::Number, at)?;
        let one = convert::to_primitive(objects, names, left, Hint::Number, at)?;
        (one, other)
    };

    if let (Value::Text(first), Value::Text(second)) = (one, other) {
        let (Some(first), Some(second)) = (objects.units(first), objects.units(second)) else {
            return Err(Escape::fault(crate::object::Fault::NotAnObject));
        };
        // Code unit by code unit, which is what makes `"Z" < "a"` true and is
        // not the same as any locale's idea of order.
        return Ok(Some(first < second));
    }

    let first = convert::to_number(objects, names, one, at)?;
    let second = convert::to_number(objects, names, other, at)?;
    if first.is_nan() || second.is_nan() {
        return Ok(None);
    }
    Ok(Some(first < second))
}

/// `===`, which converts nothing.
pub fn strictly_equal(objects: &Objects, left: Value, right: Value) -> bool {
    match (left, right) {
        (Value::Number(one), Value::Number(other)) => {
            // Not `same_value`: `NaN === NaN` is false and `0 === -0` is true,
            // both the opposite of what redefining a property is judged by —
            // which is exactly the comparison `float_cmp` warns about and
            // exactly the one the language specifies.
            #[expect(
                clippy::float_cmp,
                reason = "`===` on two numbers is IEEE equality, NaN and both zeroes included"
            )]
            let same = one == other;
            same
        }
        (Value::Text(one), Value::Text(other)) => {
            if one == other {
                return true;
            }
            match (objects.units(one), objects.units(other)) {
                (Some(first), Some(second)) => first == second,
                // A reference naming nothing is the engine's own bug, and the
                // honest answer to "are these equal" is no.
                _ => false,
            }
        }
        (Value::Undefined, Value::Undefined) | (Value::Null, Value::Null) => true,
        (Value::Bool(one), Value::Bool(other)) => one == other,
        (Value::Symbol(one), Value::Symbol(other)) | (Value::Object(one), Value::Object(other)) => {
            one == other
        }
        _ => false,
    }
}

/// `==`, which is the strict one with a short list of conversions in front.
///
/// Written as a loop with a bound rather than as recursion: each rule makes one
/// side simpler, so three passes is the most any pair needs, and a fourth would
/// mean a rule that does not make progress.
fn loosely_equal(
    objects: &mut Objects,
    names: &Names,
    left: Value,
    right: Value,
    at: usize,
) -> Result<bool, Escape> {
    let mut one = left;
    let mut other = right;
    for _ in 0..4 {
        // Same kind: the strict comparison decides, which is why `NaN == NaN`
        // is false here too.
        if kind(one) == kind(other) {
            return Ok(strictly_equal(objects, one, other));
        }
        match (one, other) {
            // The one pair the language calls equal across kinds.
            (Value::Null, Value::Undefined) | (Value::Undefined, Value::Null) => return Ok(true),
            // A string meeting a number becomes a number, and a boolean
            // becomes one whatever it meets. The two rules are one arm each
            // because they change the same side.
            (Value::Number(_), Value::Text(_)) | (_, Value::Bool(_)) => {
                other = Value::Number(convert::to_number(objects, names, other, at)?);
            }
            (Value::Text(_), Value::Number(_)) | (Value::Bool(_), _) => {
                one = Value::Number(convert::to_number(objects, names, one, at)?);
            }
            (Value::Object(_), Value::Number(_) | Value::Text(_) | Value::Symbol(_)) => {
                one = convert::to_primitive(objects, names, one, Hint::Default, at)?;
            }
            (Value::Number(_) | Value::Text(_) | Value::Symbol(_), Value::Object(_)) => {
                other = convert::to_primitive(objects, names, other, Hint::Default, at)?;
            }
            // Anything else — `null == 0`, `undefined == ""`, a symbol against
            // a string — is false, and that is a rule rather than an omission.
            _ => return Ok(false),
        }
    }
    Ok(false)
}

/// Which of the seven kinds a value is, for the comparison above.
fn kind(value: Value) -> u8 {
    match value {
        Value::Undefined => 0,
        Value::Null => 1,
        Value::Bool(_) => 2,
        Value::Number(_) => 3,
        Value::Text(_) => 4,
        Value::Symbol(_) => 5,
        Value::Object(_) => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::{exponentiate, strictly_equal};
    use crate::object::{Objects, Value};

    #[test]
    fn exponentiation_is_the_languages_rather_than_the_hardwares() {
        assert!(exponentiate(-1.0, f64::INFINITY).is_nan());
        assert!(exponentiate(1.0, f64::NAN).is_nan());
        assert!((exponentiate(2.0, 10.0) - 1024.0).abs() < f64::EPSILON);
        // `NaN ** 0` is 1, which surprises everybody and is specified.
        assert!((exponentiate(f64::NAN, 0.0) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn strict_equality_is_not_same_value() {
        let objects = Objects::new();
        assert!(!strictly_equal(
            &objects,
            Value::Number(f64::NAN),
            Value::Number(f64::NAN)
        ));
        assert!(strictly_equal(
            &objects,
            Value::Number(0.0),
            Value::Number(-0.0)
        ));
        assert!(!strictly_equal(&objects, Value::Null, Value::Undefined));
    }
}
