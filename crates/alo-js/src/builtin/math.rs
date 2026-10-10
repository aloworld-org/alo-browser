/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Math`, the namespace object (ECMA-262 § 21.3, queue item 365).
//!
//! Opened by alo Sites' analytics script, whose `pagehide` listener stopped
//! at `Math.max` in its `height()`. It is an ordinary object inheriting from
//! `Object.prototype` — not a function, and not a constructor — bound to
//! `Math` on the global object, writable and configurable and not enumerable.
//! Its eight values are neither writable, enumerable nor configurable, and
//! `Math[Symbol.toStringTag]` is `"Math"`, so `Object.prototype.toString`
//! names it.
//!
//! Every function converts its arguments with `ToNumber`, once each and in
//! order, keeping each ([`numbers`](super::numbers), ADR 0031) because an
//! argument may be an object whose `valueOf` is the page's script. `max`,
//! `min` and `hypot` take as many as a page passes, and fold them as they go
//! ([`math_fold`](super::math_fold)).
//!
//! # The arithmetic is rented, and two answers are written out
//!
//! The transcendental functions are *implementation-approximated* in the
//! specification, which names only their edge cases: the signed zeros, the
//! infinities and `NaN`. Rust's `f64` methods are the platform's mathematics
//! library, and each answers those edge cases as ECMA-262 does. Two do not,
//! and are written here: `Math.round` rounds a half towards `+∞`, where
//! `f64::round` rounds it away from zero, and `Math.sign` keeps a zero's sign,
//! where `f64::signum` answers `1` for `+0`. `Math.pow` is `**`, whose two
//! departures from IEEE 754 are [`operate`](crate::operate)'s.
//!
//! # What is not here
//!
//! `Math.random` (queue item 367) needs a source of randomness, which the
//! engine may not read from the machine on its own (ADR 0013 § 5, as ADR 0036
//! § 1 decided for a clock); what it is and who hands it in is a decision.
//! `Math.f16round` and `Math.sumPrecise` (queue item 368) are each specified
//! with something this engine does not have yet: binary16 beside
//! `Float16Array`, and an iterable read through the iteration protocol. Each
//! is absent rather than approximate (ADR 0013 § 3), so `typeof Math.random`
//! is `"undefined"`, which is what a page's feature test reads.

use crate::abrupt::Escape;
use crate::convert;
use crate::heap::Ref;
use crate::object::native::{Answer, Call};
use crate::object::{Key, Native, Objects, Property, Value};
use crate::operate;

use super::math_fold;
use super::numbers::{Numbers, numbers};

/// The eight values, as ECMA-262 § 21.3.1 names them.
const VALUES: [(&str, f64); 8] = [
    ("E", std::f64::consts::E),
    ("LN10", std::f64::consts::LN_10),
    ("LN2", std::f64::consts::LN_2),
    ("LOG10E", std::f64::consts::LOG10_E),
    ("LOG2E", std::f64::consts::LOG2_E),
    ("PI", std::f64::consts::PI),
    ("SQRT1_2", std::f64::consts::FRAC_1_SQRT_2),
    ("SQRT2", std::f64::consts::SQRT_2),
];

/// The functions, in § 21.3.2's order, each keeping what it converts.
const FUNCTIONS: [Native; 34] = [
    Native::new("abs", abs).keeping(1),
    Native::new("acos", acos).keeping(1),
    Native::new("acosh", acosh).keeping(1),
    Native::new("asin", asin).keeping(1),
    Native::new("asinh", asinh).keeping(1),
    Native::new("atan", atan).keeping(1),
    Native::new("atanh", atanh).keeping(1),
    Native::new("atan2", atan2).keeping(2),
    Native::new("cbrt", cbrt).keeping(1),
    Native::new("ceil", ceil).keeping(1),
    Native::new("clz32", clz32).keeping(1),
    Native::new("cos", cos).keeping(1),
    Native::new("cosh", cosh).keeping(1),
    Native::new("exp", exp).keeping(1),
    Native::new("expm1", expm1).keeping(1),
    Native::new("floor", floor).keeping(1),
    Native::new("fround", fround).keeping(1),
    Native::new("hypot", hypot).keeping(math_fold::KEPT),
    Native::new("imul", imul).keeping(2),
    Native::new("log", log).keeping(1),
    Native::new("log1p", log1p).keeping(1),
    Native::new("log10", log10).keeping(1),
    Native::new("log2", log2).keeping(1),
    Native::new("max", max).keeping(math_fold::KEPT),
    Native::new("min", min).keeping(math_fold::KEPT),
    Native::new("pow", pow).keeping(2),
    Native::new("round", round).keeping(1),
    Native::new("sign", sign).keeping(1),
    Native::new("sin", sin).keeping(1),
    Native::new("sinh", sinh).keeping(1),
    Native::new("sqrt", sqrt).keeping(1),
    Native::new("tan", tan).keeping(1),
    Native::new("tanh", tanh).keeping(1),
    Native::new("trunc", trunc).keeping(1),
];

/// Make `Math` and bind it on the global object.
///
/// **This allocates repeatedly.** The object is held in a scope until the
/// global object owns it, and everything after that hangs off it.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling, and a fault for a reference
/// this engine has lost — or a native that keeps more than a builtin may.
pub(crate) fn furnish(
    objects: &mut Objects,
    global: Ref,
    object_prototype: Ref,
    function_prototype: Ref,
    to_string_tag: Key,
) -> Result<(), Escape> {
    let scope = objects.heap_mut().open();
    let outcome = furnished(
        objects,
        global,
        object_prototype,
        function_prototype,
        to_string_tag,
    );
    objects.heap_mut().close(scope);
    outcome
}

/// [`furnish`], with the scope already open.
fn furnished(
    objects: &mut Objects,
    global: Ref,
    object_prototype: Ref,
    function_prototype: Ref,
    to_string_tag: Key,
) -> Result<(), Escape> {
    let math = objects
        .object(Some(object_prototype))
        .map_err(|why| Escape::refused(why, 0))?;
    objects.heap_mut().hold(math);
    // Interning the name allocates, and `math` is held across it.
    let key = name(objects, "Math")?;
    objects.define(
        global,
        key,
        Property::data(Value::Object(math), true, false, true),
    )?;

    for (value_name, value) in VALUES {
        let key = name(objects, value_name)?;
        objects.define(
            math,
            key,
            Property::data(Value::Number(value), false, false, false),
        )?;
    }

    // Not writable, not enumerable, configurable, as every tag is.
    let tag = objects
        .text("Math".encode_utf16().collect())
        .map_err(|why| Escape::refused(why, 0))?;
    objects.define(
        math,
        to_string_tag,
        Property::data(Value::Text(tag), false, false, true),
    )?;

    for native in FUNCTIONS {
        super::native_method(objects, math, function_prototype, native)?;
    }
    Ok(())
}

/// A property key for `text`, interned.
fn name(objects: &mut Objects, text: &str) -> Result<Key, Escape> {
    let units: Vec<u16> = text.encode_utf16().collect();
    objects.key(&units).map_err(|why| Escape::refused(why, 0))
}

/// A function of one number: convert it, then answer `op` of it.
fn unary(call: &mut Call<'_>, op: fn(f64) -> f64) -> Result<Answer, Escape> {
    match numbers(call, 1, 0)? {
        Numbers::Asked(answer) => Ok(answer),
        Numbers::Done => Ok(Answer::Value(Value::Number(op(call.kept_number(0)?)))),
    }
}

/// A function of two numbers: convert the first, then the second, then answer
/// `op` of them.
fn binary(call: &mut Call<'_>, op: fn(f64, f64) -> f64) -> Result<Answer, Escape> {
    match numbers(call, 2, 0)? {
        Numbers::Asked(answer) => Ok(answer),
        Numbers::Done => {
            let (first, second) = (call.kept_number(0)?, call.kept_number(1)?);
            Ok(Answer::Value(Value::Number(op(first, second))))
        }
    }
}

/// `Math.abs(x)`.
fn abs(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::abs)
}

/// `Math.acos(x)`.
fn acos(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::acos)
}

/// `Math.acosh(x)`.
fn acosh(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::acosh)
}

/// `Math.asin(x)`.
fn asin(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::asin)
}

/// `Math.asinh(x)`.
fn asinh(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::asinh)
}

/// `Math.atan(x)`.
fn atan(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::atan)
}

/// `Math.atanh(x)`.
fn atanh(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::atanh)
}

/// `Math.atan2(y, x)`: `y` first, as it is converted first.
fn atan2(call: &mut Call<'_>) -> Result<Answer, Escape> {
    binary(call, f64::atan2)
}

/// `Math.cbrt(x)`.
fn cbrt(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::cbrt)
}

/// `Math.ceil(x)`: `-0` for a number between -1 and 0.
fn ceil(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::ceil)
}

/// `Math.clz32(x)`: the leading zero bits of `ToUint32(x)`.
fn clz32(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, |x| f64::from(convert::to_uint32(x).leading_zeros()))
}

/// `Math.cos(x)`.
fn cos(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::cos)
}

/// `Math.cosh(x)`.
fn cosh(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::cosh)
}

/// `Math.exp(x)`.
fn exp(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::exp)
}

/// `Math.expm1(x)`.
fn expm1(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::exp_m1)
}

/// `Math.floor(x)`.
fn floor(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::floor)
}

/// `Math.fround(x)`: the nearest binary32, ties to even, back as a number.
fn fround(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, |x| {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "rounding to the nearest binary32 is what the function is for"
        )]
        let single = x as f32;
        f64::from(single)
    })
}

/// `Math.hypot(...args)`.
fn hypot(call: &mut Call<'_>) -> Result<Answer, Escape> {
    math_fold::fold(call, math_fold::HYPOT)
}

/// `Math.imul(a, b)`: the low thirty-two bits of the product, signed.
fn imul(call: &mut Call<'_>) -> Result<Answer, Escape> {
    binary(call, |a, b| {
        // `ToUint32` of each and the product modulo 2³², read as signed, is
        // the wrapping product of the two `ToInt32`s: the same low bits.
        f64::from(convert::to_int32(a).wrapping_mul(convert::to_int32(b)))
    })
}

/// `Math.log(x)`.
fn log(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::ln)
}

/// `Math.log1p(x)`.
fn log1p(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::ln_1p)
}

/// `Math.log10(x)`.
fn log10(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::log10)
}

/// `Math.log2(x)`.
fn log2(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::log2)
}

/// `Math.max(...args)`.
fn max(call: &mut Call<'_>) -> Result<Answer, Escape> {
    math_fold::fold(call, math_fold::MAX)
}

/// `Math.min(...args)`.
fn min(call: &mut Call<'_>) -> Result<Answer, Escape> {
    math_fold::fold(call, math_fold::MIN)
}

/// `Math.pow(base, exponent)`, which is `base ** exponent`.
fn pow(call: &mut Call<'_>) -> Result<Answer, Escape> {
    binary(call, operate::exponentiate)
}

/// `Math.round(x)`.
fn round(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, rounded)
}

/// `Math.sign(x)`.
fn sign(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, signed)
}

/// `Math.sin(x)`.
fn sin(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::sin)
}

/// `Math.sinh(x)`.
fn sinh(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::sinh)
}

/// `Math.sqrt(x)`.
fn sqrt(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::sqrt)
}

/// `Math.tan(x)`.
fn tan(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::tan)
}

/// `Math.tanh(x)`.
fn tanh(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::tanh)
}

/// `Math.trunc(x)`.
fn trunc(call: &mut Call<'_>) -> Result<Answer, Escape> {
    unary(call, f64::trunc)
}

/// The integral number closest to `x`, a half going towards `+∞`, and a zero
/// keeping the sign of what was rounded: `Math.round(-0.5)` is `-0`.
///
/// `x - floor(x)` is exact for every finite `x` that is not already whole —
/// they share an exponent, or `floor(x)` is zero — and compared with the
/// one-half that rounding cannot cross, so no `x + 0.5` is ever rounded
/// before it is floored. That sum is where the familiar mistakes are:
/// `0.49999999999999994 + 0.5` is `1`.
fn rounded(x: f64) -> f64 {
    if !x.is_finite() || x.fract() == 0.0 {
        return x;
    }
    let below = x.floor();
    let nearest = if x - below >= 0.5 { below + 1.0 } else { below };
    if nearest == 0.0 && x.is_sign_negative() {
        -0.0
    } else {
        nearest
    }
}

/// `1` or `-1` by sign, and a zero or a `NaN` as itself.
fn signed(x: f64) -> f64 {
    if x.is_nan() || x == 0.0 {
        x
    } else {
        1.0_f64.copysign(x)
    }
}

#[cfg(test)]
mod tests {
    use super::{FUNCTIONS, rounded, signed};
    use crate::bounds;

    #[test]
    fn a_half_rounds_towards_positive_infinity_and_a_zero_keeps_its_sign() {
        assert_eq!(rounded(2.5).to_bits(), 3.0_f64.to_bits());
        assert_eq!(rounded(-2.5).to_bits(), (-2.0_f64).to_bits());
        assert_eq!(rounded(-0.5).to_bits(), (-0.0_f64).to_bits());
        assert_eq!(rounded(-0.2).to_bits(), (-0.0_f64).to_bits());
        assert_eq!(rounded(0.2).to_bits(), 0.0_f64.to_bits());
        assert_eq!(
            rounded(0.499_999_999_999_999_94).to_bits(),
            0.0_f64.to_bits()
        );
        assert_eq!(
            rounded(-0.500_000_000_000_000_1).to_bits(),
            (-1.0_f64).to_bits()
        );
        // Past 2⁵², every number is whole and is its own answer.
        assert_eq!(
            rounded(4_503_599_627_370_497.0).to_bits(),
            4_503_599_627_370_497.0_f64.to_bits()
        );
    }

    #[test]
    fn sign_keeps_both_zeros_and_nan() {
        assert_eq!(signed(0.0).to_bits(), 0.0_f64.to_bits());
        assert_eq!(signed(-0.0).to_bits(), (-0.0_f64).to_bits());
        assert!(signed(f64::NAN).is_nan());
        assert_eq!(signed(f64::NEG_INFINITY).to_bits(), (-1.0_f64).to_bits());
        assert_eq!(signed(5e-324).to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn no_function_keeps_more_than_a_builtin_may() {
        for native in FUNCTIONS {
            assert!(
                native.kept() <= bounds::KEPT_BY_A_BUILTIN,
                "{}",
                native.name()
            );
        }
    }
}
