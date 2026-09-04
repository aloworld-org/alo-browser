/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A number as text, and text as a number.
//!
//! ADR 0013 § 8 rents exactly this: *number to string and back. Shortest
//! round-trip formatting of a double and correct parsing of one are famously
//! hard, entirely specified, and visible on every page that prints a number.
//! This is the arithmetic equivalent of renting a shaper.* What is rented is
//! Rust's own formatter and parser, which are the two hard halves — the shortest
//! run of digits that reads back as the same double, and correctly rounded
//! parsing of a decimal — and what is written here is the specification's
//! **spelling** of them, which is not physics and which every engine has to
//! agree on to the character.
//!
//! # `Number::toString` is not `{}`
//!
//! Rust prints `1e21` as `1000000000000000000000` and `1e-7` as
//! `0.0000001`; JavaScript prints `1e+21` and `1e-7`. The thresholds are in the
//! specification — twenty-one digits on one side, six leading zeros on the
//! other — and a page can see every one of them, because this is what a number
//! looks like when it is concatenated into a string or written into the
//! document. So [`text_of`] takes the digits and the exponent from the rented
//! formatter and lays them out the way the language says.
//!
//! # `ToNumber` on a string is a grammar rather than a parse
//!
//! `Number(" 12 ")` is 12, `Number("")` is 0, `Number("0x10")` is 16,
//! `Number("1_0")` is `NaN` and `Number("0755")` is 755 — the numeric separator
//! and the legacy octal that [`crate::number`] reads in *source* are not part of
//! this grammar at all, which is why the two files do not share a reader. What
//! they do share is the last step: a decimal literal's value is
//! `str::parse::<f64>`, rounded once, by the same rented arithmetic.

use crate::unicode;

/// A number as the language spells it.
///
/// `Number::toString(x, 10)`, which is what `String(x)`, `${x}` and every
/// concatenation with a string produce.
pub fn text_of(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value == 0.0 {
        // Both zeroes print as `0`: `String(-0)` is `"0"`, and the sign is only
        // visible through `Object.is` and `1 / -0`.
        return "0".to_owned();
    }
    if value < 0.0 {
        return format!("-{}", text_of(-value));
    }
    if value.is_infinite() {
        return "Infinity".to_owned();
    }

    let (digits, exponent) = shortest(value);
    let count = digits.len();
    // The specification's `n`: the value is `s × 10 ** (n - k)`, where `s` is
    // the digits and `k` is how many there are.
    let n = exponent.saturating_add(1);
    let k = i32::try_from(count).unwrap_or(i32::MAX);

    if k <= n && n <= 21 {
        // 100 — every digit, then the zeros that take it up to the point.
        let zeros = usize::try_from(n.saturating_sub(k)).unwrap_or_default();
        return format!("{digits}{}", "0".repeat(zeros));
    }
    if 0 < n && n <= 21 {
        // 1.5 — a point inside the digits.
        let at = usize::try_from(n).unwrap_or_default();
        let (whole, fraction) = digits.split_at(at.min(count));
        return format!("{whole}.{fraction}");
    }
    if -6 < n && n <= 0 {
        // 0.0001 — a zero, a point, and the run of zeros the exponent asks for.
        let zeros = usize::try_from(-n).unwrap_or_default();
        return format!("0.{}{digits}", "0".repeat(zeros));
    }

    // Outside those bands the language writes an exponent, and it writes the
    // sign of the exponent even when it is positive: `1e+21`.
    let power = n.saturating_sub(1);
    let sign = if power < 0 { "-" } else { "+" };
    let magnitude = power.unsigned_abs();
    let (first, rest) = digits.split_at(1.min(count));
    if rest.is_empty() {
        format!("{first}e{sign}{magnitude}")
    } else {
        format!("{first}.{rest}e{sign}{magnitude}")
    }
}

/// The shortest run of digits that reads back as this number, and the power of
/// ten the first of them stands at.
///
/// This is the rented half. Rust's `{:e}` is the shortest round-tripping form,
/// which is the same guarantee the specification asks for in the words *`k` is
/// as small as possible* — so the digits are taken from it rather than
/// recomputed, and only their layout is ours.
///
/// `value` must be finite and above zero, which is every caller in this file.
fn shortest(value: f64) -> (String, i32) {
    let written = format!("{value:e}");
    let (mantissa, exponent) = match written.split_once('e') {
        Some(halves) => halves,
        // Unreachable for a finite number, and a refusal rather than an
        // `unwrap` because ADR 0013 § 4 says this crate does not panic.
        None => (written.as_str(), "0"),
    };
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let exponent = exponent.parse::<i32>().unwrap_or_default();
    // `1.50e2` never comes out of the formatter, but a trailing zero would make
    // `k` larger than it has to be, and `k` decides which band the number is
    // printed in.
    let trimmed = digits.trim_end_matches('0');
    if trimmed.is_empty() {
        return ("0".to_owned(), exponent);
    }
    (trimmed.to_owned(), exponent)
}

/// What a string is worth as a number.
///
/// The specification's `StringNumericLiteral`, which is not the source
/// grammar: no separators, no `BigInt` suffix, an empty string is zero, and
/// `Infinity` is spelled out. Anything it cannot read whole is `NaN` — never an
/// error, because `ToNumber` on a string has no failure the language can see.
pub fn number_of(units: &[u16]) -> f64 {
    let trimmed = trim(units);
    if trimmed.is_empty() {
        return 0.0;
    }
    let Some(text) = ascii(trimmed) else {
        // A code unit that is not ASCII cannot be part of any numeric literal,
        // and the whitespace around one has already been taken off.
        return f64::NAN;
    };

    let (sign, body) = match text.as_bytes().first() {
        Some(b'+') => (1.0, text.get(1..).unwrap_or_default()),
        Some(b'-') => (-1.0, text.get(1..).unwrap_or_default()),
        _ => (1.0, text.as_str()),
    };
    if body.is_empty() {
        return f64::NAN;
    }
    if body == "Infinity" {
        return sign * f64::INFINITY;
    }
    if let Some(radix) = radix_of(body) {
        // `0x`, `0o` and `0b` take no sign at all: `Number("-0x10")` is `NaN`.
        if sign < 0.0 || text.starts_with('+') {
            return f64::NAN;
        }
        return in_base(body.get(2..).unwrap_or_default(), radix);
    }
    if !is_a_decimal(body) {
        return f64::NAN;
    }
    match body.parse::<f64>() {
        Ok(number) => sign * number,
        Err(_) => f64::NAN,
    }
}

/// Take `StrWhiteSpace` off both ends.
///
/// Whitespace **and** line terminators, which is wider than the source
/// grammar's `WhiteSpace` and is why this asks [`unicode`] for both.
fn trim(units: &[u16]) -> &[u16] {
    let skippable = |unit: u16| {
        char::from_u32(u32::from(unit))
            .is_some_and(|c| unicode::is_whitespace(c) || unicode::is_line_terminator(c))
    };
    let mut from = 0;
    while units.get(from).copied().is_some_and(skippable) {
        from = from.saturating_add(1);
    }
    let mut to = units.len();
    while to > from
        && units
            .get(to.saturating_sub(1))
            .copied()
            .is_some_and(skippable)
    {
        to = to.saturating_sub(1);
    }
    units.get(from..to).unwrap_or_default()
}

/// The code units as ASCII text, or [`None`] if any of them is not ASCII.
fn ascii(units: &[u16]) -> Option<String> {
    units
        .iter()
        .map(|unit| {
            u8::try_from(*unit)
                .ok()
                .filter(u8::is_ascii)
                .map(char::from)
        })
        .collect()
}

/// The base a `0x`, `0o` or `0b` prefix names.
fn radix_of(body: &str) -> Option<u32> {
    let mut characters = body.chars();
    if characters.next()? != '0' {
        return None;
    }
    match characters.next()? {
        'x' | 'X' => Some(16),
        'o' | 'O' => Some(8),
        'b' | 'B' => Some(2),
        _ => None,
    }
}

/// The value of a run of digits in base two, eight or sixteen.
///
/// Long runs are common in nothing anybody writes and possible in anything a
/// page passes to `Number`, so this is `f64` arithmetic rather than an integer
/// parse that would overflow: `Number("0x" + "f".repeat(100))` is a finite
/// number in every engine and must not be a wrong one here.
fn in_base(digits: &str, radix: u32) -> f64 {
    if digits.is_empty() {
        return f64::NAN;
    }
    let mut value = 0.0f64;
    for c in digits.chars() {
        let Some(digit) = c.to_digit(radix) else {
            return f64::NAN;
        };
        value = value.mul_add(f64::from(radix), f64::from(digit));
    }
    value
}

/// Whether this is a `StrUnsignedDecimalLiteral` and nothing else.
///
/// Rust's own parser accepts `inf`, `NaN`, `1_0` and a trailing `f32` suffix in
/// some versions of the grammar, and every one of those would be a number here
/// that is `NaN` in every other engine. So the shape is checked first and the
/// arithmetic is rented second.
fn is_a_decimal(body: &str) -> bool {
    let mut characters = body.chars().peekable();
    let mut digits_before = 0_usize;
    while characters.peek().is_some_and(char::is_ascii_digit) {
        characters.next();
        digits_before = digits_before.saturating_add(1);
    }
    let mut digits_after = 0_usize;
    if characters.peek() == Some(&'.') {
        characters.next();
        while characters.peek().is_some_and(char::is_ascii_digit) {
            characters.next();
            digits_after = digits_after.saturating_add(1);
        }
    }
    if digits_before == 0 && digits_after == 0 {
        return false;
    }
    match characters.next() {
        None => true,
        Some('e' | 'E') => {
            if matches!(characters.peek(), Some('+' | '-')) {
                characters.next();
            }
            let mut exponent = 0_usize;
            while characters.peek().is_some_and(char::is_ascii_digit) {
                characters.next();
                exponent = exponent.saturating_add(1);
            }
            exponent > 0 && characters.next().is_none()
        }
        Some(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{number_of, text_of};

    fn units(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    /// Two numbers, compared the way a test of an exact conversion has to.
    ///
    /// By bits rather than by `==`, which is `clippy::float_cmp`'s point: what
    /// these tests assert is that a *particular* double came out, and the two
    /// zeroes are different doubles.
    fn same(left: f64, right: f64) -> bool {
        left.to_bits() == right.to_bits()
    }

    #[test]
    fn a_number_is_spelled_the_way_the_language_says() {
        assert_eq!(text_of(0.0), "0");
        assert_eq!(text_of(-0.0), "0");
        assert_eq!(text_of(1.0), "1");
        assert_eq!(text_of(-1.5), "-1.5");
        assert_eq!(text_of(100.0), "100");
        assert_eq!(text_of(0.1), "0.1");
        assert_eq!(text_of(1.0 / 3.0), "0.3333333333333333");
        assert_eq!(text_of(f64::NAN), "NaN");
        assert_eq!(text_of(f64::INFINITY), "Infinity");
        assert_eq!(text_of(f64::NEG_INFINITY), "-Infinity");
    }

    #[test]
    fn the_two_bands_where_it_becomes_an_exponent() {
        // Rust would print these as `1000000000000000000000` and `0.0000001`,
        // which is the whole reason this file exists.
        assert_eq!(text_of(1e21), "1e+21");
        assert_eq!(text_of(1e-7), "1e-7");
        assert_eq!(text_of(1e20), "100000000000000000000");
        assert_eq!(text_of(1e-6), "0.000001");
        assert_eq!(text_of(1.5e-7), "1.5e-7");
        assert_eq!(text_of(1.25e22), "1.25e+22");
        assert_eq!(text_of(f64::MAX), "1.7976931348623157e+308");
        assert_eq!(text_of(5e-324), "5e-324");
    }

    #[test]
    fn a_string_is_read_by_the_other_grammar() {
        assert!(same(number_of(&units("")), 0.0));
        assert!(same(number_of(&units("  \n\t ")), 0.0));
        assert!(same(number_of(&units(" 12 ")), 12.0));
        assert!(same(number_of(&units("-1.5e2")), -150.0));
        assert!(same(number_of(&units("0x10")), 16.0));
        assert!(same(number_of(&units("0b101")), 5.0));
        assert!(same(number_of(&units("0o17")), 15.0));
        // Not the source grammar: a leading zero is decimal here.
        assert!(same(number_of(&units("0755")), 755.0));
        assert!(same(number_of(&units("Infinity")), f64::INFINITY));
        assert!(same(number_of(&units("-Infinity")), f64::NEG_INFINITY));
        assert!(same(number_of(&units(".5")), 0.5));
        assert!(same(number_of(&units("5.")), 5.0));
    }

    #[test]
    fn the_forms_rust_would_read_and_the_language_does_not() {
        for text in [
            "1_0", "inf", "nan", "0x", "-0x10", "1e", "1.2.3", "12a", "+",
        ] {
            assert!(
                number_of(&units(text)).is_nan(),
                "{text} is not a number in this language"
            );
        }
    }

    #[test]
    fn every_number_reads_back_as_itself() {
        for value in [
            0.1,
            1.0 / 3.0,
            1e21,
            5e-324,
            f64::MAX,
            123_456_789_012_345_680.0,
            -2.5e-10,
        ] {
            let printed = text_of(value);
            assert!(
                same(number_of(&units(&printed)), value),
                "{printed} came back as something else"
            );
        }
    }
}
