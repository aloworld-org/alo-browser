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
//!
//! # An operand that is an object stops the operator rather than converting it
//!
//! Turning an object into a primitive means calling a method the page wrote,
//! and only the interpreter can call one (queue item 214). So an operator that
//! meets an object where it needs a primitive answers [`Applied::Wants`] —
//! **which** operand, and with which hint — and the interpreter converts that
//! operand, writes the answer back where it stood, and applies the operator
//! again. By then that side is a primitive and the same question is asked of
//! the other one, so each operand's `valueOf` is called exactly once and in the
//! order written below.
//!
//! Asking here rather than in the interpreter is the point: which operand `a >
//! b` converts first, and whether `==` converts at all, are this file's rules
//! and nowhere else's.

use crate::abrupt::{Escape, Missing};
use crate::ast::{Binary, Unary};
use crate::convert::{self, Hint, Primitive};
use crate::object::{Objects, Value};

/// Which operand an operator is asking about.
///
/// A unary operator's one operand is [`Side::Left`], because it is the only
/// one there is to name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// `a` of `a + b`, and the operand of `-a`.
    Left,
    /// `b` of `a + b`.
    Right,
}

/// What applying an operator came to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Applied {
    /// The answer.
    Answer(Value),
    /// An operand is an object and this operator needs a primitive from it
    /// first. The caller converts it — which is a call — and applies the
    /// operator again with the answer in that operand's place.
    Wants {
        /// Which operand.
        side: Side,
        /// Which primitive it is being asked for.
        hint: Hint,
    },
}

/// What stopped an operator short: it needs a primitive, or it is over.
///
/// Two of them so that the body of every operator below can be written with
/// `?` and read as the specification does, rather than as a chain of matches
/// on a three-valued answer.
enum Stop {
    /// An operand must be converted first.
    Wants { side: Side, hint: Hint },
    /// Something the program cannot go on from.
    Escape(Escape),
}

impl From<Escape> for Stop {
    fn from(escape: Escape) -> Self {
        Self::Escape(escape)
    }
}

impl From<crate::object::Fault> for Stop {
    fn from(fault: crate::object::Fault) -> Self {
        Self::Escape(Escape::fault(fault))
    }
}

/// What an operator's answer becomes at the boundary of this file.
fn applied(outcome: Result<Value, Stop>) -> Result<Applied, Escape> {
    match outcome {
        Ok(value) => Ok(Applied::Answer(value)),
        Err(Stop::Wants { side, hint }) => Ok(Applied::Wants { side, hint }),
        Err(Stop::Escape(escape)) => Err(escape),
    }
}

/// The primitive an operand already is, or the request that makes it one.
fn primitive(value: Value, side: Side, hint: Hint) -> Result<Primitive, Stop> {
    Primitive::of(value).ok_or(Stop::Wants { side, hint })
}

/// `ToNumber` on an operand, which needs it to be a primitive first.
fn number(objects: &Objects, value: Value, side: Side, at: usize) -> Result<f64, Stop> {
    Ok(convert::to_number(
        objects,
        primitive(value, side, Hint::Number)?,
        at,
    )?)
}

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
    operator: Simple,
    value: Value,
    at: usize,
) -> Result<Applied, Escape> {
    applied(apply_one(objects, operator, value, at))
}

/// The five of them, written as the specification writes them.
fn apply_one(objects: &Objects, operator: Simple, value: Value, at: usize) -> Result<Value, Stop> {
    Ok(match operator {
        // The two that take a value as it is: `void` throws it away and
        // `ToBoolean` is defined on an object without asking it anything.
        Simple::Void => Value::Undefined,
        Simple::Not => Value::Bool(!convert::to_boolean(objects, value)),
        Simple::Plus => Value::Number(number(objects, value, Side::Left, at)?),
        Simple::Minus => Value::Number(-number(objects, value, Side::Left, at)?),
        Simple::BitNot => {
            let number = number(objects, value, Side::Left, at)?;
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
    operator: Binary,
    left: Value,
    right: Value,
    at: usize,
) -> Result<Applied, Escape> {
    applied(apply_two(objects, operator, left, right, at))
}

/// Every one of them, written as the specification writes them.
fn apply_two(
    objects: &mut Objects,
    operator: Binary,
    left: Value,
    right: Value,
    at: usize,
) -> Result<Value, Stop> {
    match operator {
        Binary::Add => add(objects, left, right, at),
        Binary::Subtract => {
            let (one, other) = numbers(objects, left, right, at)?;
            Ok(Value::Number(one - other))
        }
        Binary::Multiply => {
            let (one, other) = numbers(objects, left, right, at)?;
            Ok(Value::Number(one * other))
        }
        Binary::Divide => {
            let (one, other) = numbers(objects, left, right, at)?;
            // Dividing by zero is an infinity rather than an error: the
            // language has no integer division and no trap.
            Ok(Value::Number(one / other))
        }
        Binary::Remainder => {
            let (one, other) = numbers(objects, left, right, at)?;
            // The C `fmod`: the answer takes the sign of the dividend, so
            // `-1 % 2` is `-1` and not `1`.
            Ok(Value::Number(one % other))
        }
        Binary::Power => {
            let (one, other) = numbers(objects, left, right, at)?;
            Ok(Value::Number(exponentiate(one, other)))
        }
        Binary::ShiftLeft => {
            let (one, other) = numbers(objects, left, right, at)?;
            Ok(Value::Number(f64::from(
                convert::to_int32(one).wrapping_shl(shift_by(other)),
            )))
        }
        Binary::ShiftRight => {
            let (one, other) = numbers(objects, left, right, at)?;
            Ok(Value::Number(f64::from(
                convert::to_int32(one).wrapping_shr(shift_by(other)),
            )))
        }
        Binary::ShiftRightUnsigned => {
            let (one, other) = numbers(objects, left, right, at)?;
            // The only one whose answer can be above `i32::MAX`: `-1 >>> 0` is
            // 4294967295, which is why it converts to an unsigned integer and
            // the other two do not.
            Ok(Value::Number(f64::from(
                convert::to_uint32(one).wrapping_shr(shift_by(other)),
            )))
        }
        Binary::BitAnd => {
            let (one, other) = integers(objects, left, right, at)?;
            Ok(Value::Number(f64::from(one & other)))
        }
        Binary::BitOr => {
            let (one, other) = integers(objects, left, right, at)?;
            Ok(Value::Number(f64::from(one | other)))
        }
        Binary::BitXor => {
            let (one, other) = integers(objects, left, right, at)?;
            Ok(Value::Number(f64::from(one ^ other)))
        }
        Binary::Less | Binary::Greater | Binary::LessOrEqual | Binary::GreaterOrEqual => {
            relational(objects, operator, left, right, at)
        }
        Binary::Equal => Ok(Value::Bool(loosely_equal(objects, left, right, at)?)),
        Binary::NotEqual => Ok(Value::Bool(!loosely_equal(objects, left, right, at)?)),
        Binary::StrictlyEqual => Ok(Value::Bool(strictly_equal(objects, left, right))),
        Binary::StrictlyNotEqual => Ok(Value::Bool(!strictly_equal(objects, left, right))),
        Binary::In => {
            let Value::Object(object) = right else {
                return Err(Escape::type_error(
                    "the right-hand side of 'in' must be an object",
                    at,
                )
                .into());
            };
            let key =
                convert::to_property_key(objects, primitive(left, Side::Left, Hint::String)?, at)?;
            Ok(Value::Bool(objects.has(object, key)?))
        }
        Binary::InstanceOf => instance_of(objects, left, right, at),
    }
}

/// `a instanceof b`.
///
/// `Symbol.hasInstance` comes first in the specification and arrives with the
/// builtins (queue item 73), so what is here is `OrdinaryHasInstance` — and its
/// **order** is the part worth writing out, because two of its three answers
/// come before anything is read off the right-hand side.
fn instance_of(objects: &Objects, left: Value, right: Value, at: usize) -> Result<Value, Stop> {
    let Value::Object(target) = right else {
        return Err(Escape::type_error(
            "the right-hand side of 'instanceof' must be an object",
            at,
        )
        .into());
    };
    if objects.callable(target).is_none() {
        return Err(
            Escape::type_error("the right-hand side of 'instanceof' is not callable", at).into(),
        );
    }
    // A primitive is not an instance of anything, and the specification answers
    // that **before** it reads `prototype` — so `1 instanceof f` is `false`
    // rather than anything the right-hand side could decide.
    if !matches!(left, Value::Object(_)) {
        return Ok(Value::Bool(false));
    }
    // What comes next is `Get(C, "prototype")`, and a function in this engine
    // has no `prototype` property yet: that is what `[[Construct]]` is for and
    // it is queue item 212. Answering `false` here would be answering a
    // question this engine did not ask.
    Err(Escape::NotBuiltYet(Missing::APrototype).into())
}

/// Both sides as numbers, left first — which is the order a `valueOf` with a
/// side effect can see.
fn numbers(objects: &Objects, left: Value, right: Value, at: usize) -> Result<(f64, f64), Stop> {
    let one = number(objects, left, Side::Left, at)?;
    let other = number(objects, right, Side::Right, at)?;
    Ok((one, other))
}

/// Both sides as `ToInt32`, in the same order.
fn integers(objects: &Objects, left: Value, right: Value, at: usize) -> Result<(i32, i32), Stop> {
    let (one, other) = numbers(objects, left, right, at)?;
    Ok((convert::to_int32(one), convert::to_int32(other)))
}

/// How far a shift shifts: the low five bits of its right-hand side, which is
/// what makes `1 << 32` one rather than zero.
fn shift_by(right: f64) -> u32 {
    convert::to_uint32(right) & 31
}

/// `a + b`, which is two operators sharing a spelling.
fn add(objects: &mut Objects, left: Value, right: Value, at: usize) -> Result<Value, Stop> {
    // Both sides become primitives *first*: `1 + {valueOf(){return "a"}}` is a
    // concatenation, and an engine that asked "is either a string" before
    // converting would have made it an addition.
    let one = primitive(left, Side::Left, Hint::Default)?;
    let other = primitive(right, Side::Right, Hint::Default)?;
    if matches!(one.value(), Value::Text(_)) || matches!(other.value(), Value::Text(_)) {
        let mut units = convert::to_units(objects, one, at)?;
        units.extend(convert::to_units(objects, other, at)?);
        let held = objects
            .text(units)
            .map_err(|why| Escape::refused(why, at))?;
        return Ok(Value::Text(held));
    }
    let one = convert::to_number(objects, one, at)?;
    let other = convert::to_number(objects, other, at)?;
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
    operator: Binary,
    left: Value,
    right: Value,
    at: usize,
) -> Result<Value, Stop> {
    // Which side is *evaluated* first is already decided — the compiler emitted
    // them in order — and what is decided here is which side is **converted**
    // first, which a `valueOf` with a side effect can see.
    let answer = match operator {
        Binary::Less => less_than(objects, left, right, true, at)?,
        Binary::Greater => less_than(objects, right, left, false, at)?,
        Binary::LessOrEqual => less_than(objects, right, left, false, at)?.map(|is| !is),
        _ => less_than(objects, left, right, true, at)?.map(|is| !is),
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
    left: Value,
    right: Value,
    left_first: bool,
    at: usize,
) -> Result<Option<bool>, Stop> {
    // `left` and `right` are this comparison's two sides, which for `>` and
    // `<=` were handed over the other way round — so which *operand* each one
    // is comes from `left_first` too, and a request to convert must name the
    // operand the interpreter has on its stack rather than the argument here.
    let (near, far) = if left_first {
        (Side::Left, Side::Right)
    } else {
        (Side::Right, Side::Left)
    };
    let (one, other) = if left_first {
        let one = primitive(left, near, Hint::Number)?;
        let other = primitive(right, far, Hint::Number)?;
        (one, other)
    } else {
        let other = primitive(right, far, Hint::Number)?;
        let one = primitive(left, near, Hint::Number)?;
        (one, other)
    };

    if let (Value::Text(first), Value::Text(second)) = (one.value(), other.value()) {
        let (Some(first), Some(second)) = (objects.units(first), objects.units(second)) else {
            return Err(Escape::fault(crate::object::Fault::NotAnObject).into());
        };
        // Code unit by code unit, which is what makes `"Z" < "a"` true and is
        // not the same as any locale's idea of order.
        return Ok(Some(first < second));
    }

    let first = convert::to_number(objects, one, at)?;
    let second = convert::to_number(objects, other, at)?;
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
    left: Value,
    right: Value,
    at: usize,
) -> Result<bool, Stop> {
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
                other = Value::Number(number(objects, other, Side::Right, at)?);
            }
            (Value::Text(_), Value::Number(_)) | (Value::Bool(_), _) => {
                one = Value::Number(number(objects, one, Side::Left, at)?);
            }
            // The two that need a call: an object meeting a primitive becomes
            // one, which is the interpreter's to do and this loop's to ask for.
            (Value::Object(_), Value::Number(_) | Value::Text(_) | Value::Symbol(_)) => {
                return Err(Stop::Wants {
                    side: Side::Left,
                    hint: Hint::Default,
                });
            }
            (Value::Number(_) | Value::Text(_) | Value::Symbol(_), Value::Object(_)) => {
                return Err(Stop::Wants {
                    side: Side::Right,
                    hint: Hint::Default,
                });
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
