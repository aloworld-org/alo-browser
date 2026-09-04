/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Numeric literals: the grammar, and the value.
//!
//! # The grammar is ours and the rounding is not
//!
//! Deciding whether `0b1_0` is a number, whether `0755` is one, and whether
//! `3in x` is two tokens are all grammar, and all three are decided here.
//! Turning `0.1` into the nearest `f64` is not grammar — it is the arithmetic
//! ADR 0013 § 8 says to rent, and Rust's own `str::parse::<f64>` is correctly
//! rounded, so the decimal path composes a plain literal and hands it over.
//! There is nothing to rent that the standard library does not already do.
//!
//! **The other three bases cannot use it**, because `0x1p3` is a Rust float and
//! not a JavaScript one, and because a hexadecimal literal of two hundred
//! digits has to round **once**. [`from_power_of_two`] is that: it walks the
//! bits, keeps the first fifty-three, and rounds to nearest with ties to even
//! using a guard bit and a sticky bit — the same single rounding the
//! specification's "the mathematical value, then rounded" describes.
//!
//! # Two forms are refused rather than read
//!
//! `0755` and `08`, which ADR 0013 § 3 sends to the legacy tail. Both exist
//! only in sloppy code, and `0755` meaning four hundred and ninety-three is a
//! reading that surprises the person, never the browser.

use crate::error::{Reason, SyntaxError};
use crate::{read, unicode};

/// What a numeric literal is worth.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// An ordinary number, already rounded to the nearest `f64`.
    Number(f64),
    /// A `BigInt`, kept as its digits.
    ///
    /// Digits rather than a value, because a `BigInt` is arbitrary precision
    /// and there is no arbitrary-precision integer in this crate yet — that is
    /// queue item 71's object model, and inventing one here would be a second
    /// place for it to live. The separators are already gone, so what is kept
    /// is exactly the digits in the base beside them.
    BigInt {
        /// The digits, with any `_` removed.
        digits: String,
        /// What base they are in.
        radix: Radix,
    },
}

/// The base a literal was written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Radix {
    /// `0b1010`
    Binary,
    /// `0o755`
    Octal,
    /// `493`
    Decimal,
    /// `0x1ED`
    Hexadecimal,
}

impl Radix {
    /// The base as a number.
    pub fn value(self) -> u32 {
        match self {
            Self::Binary => 2,
            Self::Octal => 8,
            Self::Decimal => 10,
            Self::Hexadecimal => 16,
        }
    }

    /// How many bits one digit is worth, for the three that are powers of two.
    fn bits_per_digit(self) -> Option<u32> {
        match self {
            Self::Binary => Some(1),
            Self::Octal => Some(3),
            Self::Hexadecimal => Some(4),
            Self::Decimal => None,
        }
    }
}

/// Read the numeric literal beginning at `at`.
///
/// `at` is either a decimal digit or a `.` the caller has already seen a digit
/// after.
///
/// # Errors
///
/// A legacy octal literal, a misplaced separator, a missing run of digits, a
/// `BigInt` suffix on something that is not whole, or a name straight after the
/// number.
pub fn scan(source: &str, at: usize) -> Result<(Value, usize), SyntaxError> {
    let (value, end) = match prefix(source, at) {
        Some(radix) => in_base(source, at.saturating_add(2), radix)?,
        None => in_decimal(source, at)?,
    };
    // `3in x` is not `3 in x`: the grammar forbids a name or a digit straight
    // after a literal, so that one program is not two ways of writing another.
    if let Some(c) = read::char_at(source, end) {
        if unicode::starts_a_name(c) || c.is_ascii_digit() {
            return Err(SyntaxError::new(Reason::NumberFollowedByName, end));
        }
    }
    Ok((value, end))
}

/// The base a `0x`, `0o` or `0b` names, if the literal has one.
fn prefix(source: &str, at: usize) -> Option<Radix> {
    if read::char_at(source, at) != Some('0') {
        return None;
    }
    match read::char_at(source, at.saturating_add(1)) {
        Some('x' | 'X') => Some(Radix::Hexadecimal),
        Some('o' | 'O') => Some(Radix::Octal),
        Some('b' | 'B') => Some(Radix::Binary),
        _ => None,
    }
}

/// A literal in base two, eight or sixteen, starting just past its prefix.
fn in_base(source: &str, at: usize, radix: Radix) -> Result<(Value, usize), SyntaxError> {
    let (digits, end) = digits(source, at, radix.value())?;
    if read::char_at(source, end) == Some('n') {
        return Ok((Value::BigInt { digits, radix }, end.saturating_add(1)));
    }
    let bits = radix.bits_per_digit().unwrap_or(4);
    Ok((Value::Number(from_power_of_two(&digits, bits)), end))
}

/// A literal in base ten, which is the only one with a point and an exponent.
fn in_decimal(source: &str, at: usize) -> Result<(Value, usize), SyntaxError> {
    let mut integer = String::new();
    let mut fraction = String::new();
    let mut exponent = String::new();
    let mut has_point = false;
    let mut has_exponent = false;
    let mut end = at;

    if read::char_at(source, end) == Some('.') {
        has_point = true;
        end = end.saturating_add(1);
        let (read_fraction, after) = digits(source, end, 10)?;
        fraction = read_fraction;
        end = after;
    } else {
        refuse_a_legacy_octal(source, end)?;
        let (read_integer, after) = digits(source, end, 10)?;
        integer = read_integer;
        end = after;
        if read::char_at(source, end) == Some('.') {
            has_point = true;
            end = end.saturating_add(1);
            // `1.` is a number and `1.foo` is a property access on one, so the
            // digits after a point are optional rather than missing.
            if read::char_at(source, end).is_some_and(|c| c.is_ascii_digit()) {
                let (read_fraction, after) = digits(source, end, 10)?;
                fraction = read_fraction;
                end = after;
            }
        }
    }

    if matches!(read::char_at(source, end), Some('e' | 'E')) {
        has_exponent = true;
        end = end.saturating_add(1);
        let mut sign = "";
        match read::char_at(source, end) {
            Some('+') => end = end.saturating_add(1),
            Some('-') => {
                sign = "-";
                end = end.saturating_add(1);
            }
            _ => {}
        }
        let (magnitude, after) = digits(source, end, 10)?;
        exponent = format!("{sign}{magnitude}");
        end = after;
    }

    if read::char_at(source, end) == Some('n') {
        if has_point || has_exponent {
            return Err(SyntaxError::new(Reason::BigIntIsNotAnInteger, end));
        }
        return Ok((
            Value::BigInt {
                digits: integer,
                radix: Radix::Decimal,
            },
            end.saturating_add(1),
        ));
    }

    Ok((
        Value::Number(rounded(&integer, &fraction, &exponent, at)?),
        end,
    ))
}

/// `0755` and `08`, refused by name.
///
/// Also `0_1`, which is the separator inside the same legacy form and gets the
/// separator's own message because that is the mistake somebody made.
fn refuse_a_legacy_octal(source: &str, at: usize) -> Result<(), SyntaxError> {
    if read::char_at(source, at) != Some('0') {
        return Ok(());
    }
    match read::char_at(source, at.saturating_add(1)) {
        Some(c) if c.is_ascii_digit() => Err(SyntaxError::new(Reason::LegacyOctalLiteral, at)),
        Some('_') => Err(SyntaxError::new(
            Reason::MisplacedNumericSeparator,
            at.saturating_add(1),
        )),
        _ => Ok(()),
    }
}

/// The run of digits at `at`, with the separators taken out.
///
/// A `_` is only a separator **between** digits, so a leading, trailing or
/// doubled one is refused rather than ignored: `1__0` and `1_` are not other
/// ways of writing ten.
fn digits(source: &str, at: usize, radix: u32) -> Result<(String, usize), SyntaxError> {
    let mut out = String::new();
    let mut end = at;
    let mut after_a_digit = false;
    loop {
        match read::char_at(source, end) {
            Some('_') => {
                let next_is_a_digit =
                    read::char_at(source, end.saturating_add(1)).is_some_and(|c| c.is_digit(radix));
                if !after_a_digit || !next_is_a_digit {
                    return Err(SyntaxError::new(Reason::MisplacedNumericSeparator, end));
                }
                after_a_digit = false;
                end = end.saturating_add(1);
            }
            Some(c) if c.is_digit(radix) => {
                out.push(c);
                after_a_digit = true;
                end = end.saturating_add(1);
            }
            _ => break,
        }
    }
    if out.is_empty() {
        return Err(SyntaxError::new(Reason::MissingDigits, at));
    }
    Ok((out, end))
}

/// A decimal literal's value, rounded once by the standard library.
///
/// The text handed over is composed here rather than cut out of the source, so
/// it is always a form `f64` parses: a separator can no longer be in it, and an
/// empty integer, fraction or exponent becomes a zero. The error is unreachable
/// for that reason and is a refusal rather than an `unwrap` because ADR 0013
/// § 4 says this crate does not panic on anything.
fn rounded(integer: &str, fraction: &str, exponent: &str, at: usize) -> Result<f64, SyntaxError> {
    let integer = if integer.is_empty() { "0" } else { integer };
    let fraction = if fraction.is_empty() { "0" } else { fraction };
    let exponent = if exponent.is_empty() { "0" } else { exponent };
    format!("{integer}.{fraction}e{exponent}")
        .parse::<f64>()
        .map_err(|_| SyntaxError::new(Reason::MissingDigits, at))
}

/// The value of digits in a base that is a power of two, rounded once.
///
/// `str::parse` cannot be used: `0x1p3` is a Rust hexadecimal float and not a
/// JavaScript one, and a literal with more than fifty-three significant bits
/// has to be rounded a single time rather than at every digit. So the bits are
/// walked: the first fifty-three become the significand, the next is the guard,
/// and everything after it is folded into a sticky bit — nearest, ties to even.
///
/// Nothing is allocated by the walk, which matters: a literal is as long as the
/// page chose, and materialising its bits would be a page deciding how much
/// memory this process uses.
pub fn from_power_of_two(digits: &str, bits_per_digit: u32) -> f64 {
    let mut significand = 0.0f64;
    let mut kept = 0u32;
    let mut started = false;
    let mut last_bit_is_one = false;
    let mut guard = false;
    let mut have_guard = false;
    let mut sticky = false;
    let mut dropped = 0usize;

    for c in digits.chars() {
        let Some(value) = c.to_digit(16) else {
            continue;
        };
        for shift in (0..bits_per_digit).rev() {
            let bit = (value >> shift) & 1 == 1;
            if !started {
                if !bit {
                    continue;
                }
                started = true;
            }
            if kept < 53 {
                significand = significand.mul_add(2.0, if bit { 1.0 } else { 0.0 });
                last_bit_is_one = bit;
                kept = kept.saturating_add(1);
            } else if have_guard {
                sticky |= bit;
                dropped = dropped.saturating_add(1);
            } else {
                guard = bit;
                have_guard = true;
                dropped = dropped.saturating_add(1);
            }
        }
    }

    if guard && (sticky || last_bit_is_one) {
        significand += 1.0;
    }
    if dropped == 0 {
        return significand;
    }
    let scale = i32::try_from(dropped).unwrap_or(i32::MAX);
    significand * 2.0f64.powi(scale)
}
