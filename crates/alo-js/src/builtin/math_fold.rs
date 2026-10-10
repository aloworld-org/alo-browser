/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Math.max`, `Math.min` and `Math.hypot`: one number made of **every**
//! argument, however many a page passes (queue item 365, ADR 0031).
//!
//! The specification converts each argument with `ToNumber`, once and in
//! order, and only then looks at the numbers. A `NaN` part of the way through
//! does not stop the conversions after it — `Math.max(NaN, { valueOf() { … } })`
//! still runs the page's `valueOf` — and a page can count those calls. So each
//! is converted as it is reached and folded into the answer at once: for these
//! three, the fold of the list in order is the answer the list would have
//! given, and nothing as long as the page chooses has to be kept.
//!
//! The index and the answer so far are kept in the builtin's two slots
//! (ADR 0031 § 1), because the count is the page's and ADR 0031 § 5 forbids
//! putting it in the step. A conversion that runs script is asked for and
//! comes back at [`CONVERTED`], where the slot says which argument it was.
//! The loop is as long as the page made the call, so it asks the embedder's
//! stop on every pass (ADR 0031 § 7).

use crate::abrupt::{Escape, Internal};
use crate::convert::{self, Hint, Primitive};
use crate::object::Value;
use crate::object::native::{Answer, Call, Want};

/// The slots a fold keeps: the index, and the answer so far.
pub(super) const KEPT: usize = 2;

/// The slot the index of the next argument is kept in.
const INDEX: usize = 0;
/// The slot the answer so far is kept in.
const SO_FAR: usize = 1;

/// The step a fold comes back at with the primitive an argument converted to.
const CONVERTED: u32 = 1;

/// One fold: where it starts, and how one number joins it.
#[derive(Clone, Copy)]
pub(super) struct Fold {
    /// The answer for no arguments at all.
    pub(super) empty: f64,
    /// The answer so far, with one more number in it.
    pub(super) join: fn(f64, f64) -> f64,
}

/// `Math.max`'s: the largest, `+0` above `-0`, and `NaN` if any is.
pub(super) const MAX: Fold = Fold {
    empty: f64::NEG_INFINITY,
    join: larger,
};

/// `Math.min`'s: the smallest, `-0` below `+0`, and `NaN` if any is.
pub(super) const MIN: Fold = Fold {
    empty: f64::INFINITY,
    join: smaller,
};

/// `Math.hypot`'s: the square root of the sum of squares, without the
/// overflow squaring would cause, `+∞` if any is infinite even beside a
/// `NaN`, and `+0` for no arguments or only zeros. `f64::hypot` answers all
/// three of those as the specification does, so a running one does too.
pub(super) const HYPOT: Fold = Fold {
    empty: 0.0,
    join: f64::hypot,
};

/// Run `fold` over every argument of the call, at whatever step it is.
///
/// # Errors
///
/// A `TypeError` for a symbol, whatever a page's `valueOf` throws,
/// [`Escape::Interrupted`] when the embedder asks the script to stop, and
/// [`Internal::BuiltinIsWrong`] for a step or a slot this file never wrote.
pub(super) fn fold(call: &mut Call<'_>, fold: Fold) -> Result<Answer, Escape> {
    let (mut index, mut so_far) = match call.step() {
        0 => (0, fold.empty),
        CONVERTED => {
            // A conversion answers a primitive or throws, so anything else is
            // this engine having resumed the wrong builtin.
            let primitive =
                Primitive::of(call.answer()?).ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
            let number = convert::to_number(call.seen(), primitive, call.at())?;
            let index = index_of(call.kept_number(INDEX)?)?;
            let so_far = (fold.join)(call.kept_number(SO_FAR)?, number);
            (index.saturating_add(1), so_far)
        }
        _ => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    };
    while index < call.count() {
        if call.stop_asked() {
            return Err(Escape::Interrupted);
        }
        let value = call.argument(index);
        let Some(primitive) = Primitive::of(value) else {
            call.keep(INDEX, Value::Number(number_of(index)?))?;
            call.keep(SO_FAR, Value::Number(so_far))?;
            return Ok(Answer::want(
                Want::Primitive {
                    of: value,
                    hint: Hint::Number,
                },
                CONVERTED,
            ));
        };
        let number = convert::to_number(call.seen(), primitive, call.at())?;
        so_far = (fold.join)(so_far, number);
        index = index.saturating_add(1);
    }
    Ok(Answer::Value(Value::Number(so_far)))
}

/// `Math.max`'s join.
fn larger(so_far: f64, number: f64) -> f64 {
    if so_far.is_nan() || number.is_nan() {
        f64::NAN
    } else if number > so_far || (number == 0.0 && so_far == 0.0 && so_far.is_sign_negative()) {
        // `-0 < +0` is false, so the zeros are told apart by their sign.
        number
    } else {
        so_far
    }
}

/// `Math.min`'s join.
fn smaller(so_far: f64, number: f64) -> f64 {
    if so_far.is_nan() || number.is_nan() {
        f64::NAN
    } else if number < so_far || (number == 0.0 && so_far == 0.0 && number.is_sign_negative()) {
        number
    } else {
        so_far
    }
}

/// An index as the number a slot keeps (ADR 0031 § 3). An argument count is
/// bounded by the stack, far below 2⁵³, so the conversion is exact.
fn number_of(index: usize) -> Result<f64, Escape> {
    u32::try_from(index)
        .map(f64::from)
        .map_err(|_| Escape::Broken(Internal::BuiltinIsWrong))
}

/// The index a slot kept, or this engine's bug if it kept anything else.
fn index_of(number: f64) -> Result<usize, Escape> {
    if !(0.0..=f64::from(u32::MAX)).contains(&number) || number.fract() != 0.0 {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    }
    // Checked above: whole, and within a `u32`.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the number was checked to be a whole number within a u32"
    )]
    let index = number as u32;
    usize::try_from(index).map_err(|_| Escape::Broken(Internal::BuiltinIsWrong))
}

#[cfg(test)]
mod tests {
    use super::{larger, smaller};

    #[test]
    fn the_zeros_are_ordered_by_their_sign() {
        assert!(larger(-0.0, 0.0).is_sign_positive());
        assert!(larger(0.0, -0.0).is_sign_positive());
        assert!(smaller(0.0, -0.0).is_sign_negative());
        assert!(smaller(-0.0, 0.0).is_sign_negative());
    }

    #[test]
    fn a_nan_stays_whatever_follows_it() {
        assert!(larger(f64::NAN, f64::INFINITY).is_nan());
        assert!(larger(1.0, f64::NAN).is_nan());
        assert!(smaller(f64::NAN, f64::NEG_INFINITY).is_nan());
        assert!(smaller(1.0, f64::NAN).is_nan());
    }

    #[test]
    fn a_running_hypot_lets_an_infinity_beat_a_nan_in_either_order() {
        assert!(f64::NAN.hypot(f64::INFINITY).is_infinite());
        assert!(f64::NEG_INFINITY.hypot(f64::NAN).is_infinite());
        assert!(0.0_f64.hypot(-0.0).is_sign_positive());
    }
}
