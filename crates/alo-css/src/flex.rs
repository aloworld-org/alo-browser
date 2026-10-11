/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `flex`, split into its three longhands as it is written.
//!
//! Told apart by **kind**, as [`crate::list_style`] is, and split here for
//! the reason [`DeclarationBlock::push`](crate::DeclarationBlock::push)
//! gives: nothing reads the shorthand itself, so left whole, `flex: 1` was
//! read as no flex at all. alo Agenda's calendar rows (`.calToggle` and
//! `.calName`) and alo Settings' dialog head and body all grow by it, and
//! none of them did (queue item 440).
//!
//! # The grammar, CSS Flexbox 1 § 7.1
//!
//! `none | [ <'flex-grow'> <'flex-shrink'>? || <'flex-basis'> ]`. The two
//! factors are non-negative numbers and stand together; the basis is any
//! other value, before them or after them. `none` is `0 0 auto`, `auto` is
//! `1 1 auto`, a shrink left out is `1`, a basis left out is `0%`, and a
//! grow left out beside a basis is `1`. A unitless zero is a factor unless
//! two factors are already in front of it, when it is the basis `0px`.
//!
//! Each answer was measured: headless Chrome 154, `getComputedStyle` of
//! thirty-two values, on 2026-10-11. `flex: 1` is `1 1 0%` there, `1 1 0`
//! is `1 1 0px`, and `1 2 3`, `1 0px 2`, `30px 40px`, `-1` and `none 1` are
//! refused.
//!
//! # What is left whole
//!
//! A value of no shape this grammar allows, and a value holding `var()`,
//! which could be any of the three until it is substituted. Each is split
//! into nothing, as [`crate::shorthand`] leaves a value it cannot split, so
//! the longhands keep whatever else set them. A CSS-wide keyword alone sets
//! all three to it.

/// The longhands `flex` sets, as CSSOM asks for them.
pub(crate) static LONGHANDS: [&str; 3] = ["flex-grow", "flex-shrink", "flex-basis"];

/// The three longhands with the value each takes, or nothing when the value
/// cannot be split.
pub(crate) fn split(value: &str) -> Vec<(String, String)> {
    let value = value.trim();
    if is_wide_keyword(value) {
        return LONGHANDS
            .iter()
            .map(|longhand| ((*longhand).to_owned(), value.to_owned()))
            .collect();
    }
    if value.to_ascii_lowercase().contains("var(") {
        return Vec::new();
    }
    match three(value) {
        Some([grow, shrink, basis]) => LONGHANDS
            .iter()
            .zip([grow, shrink, basis])
            .map(|(longhand, value)| ((*longhand).to_owned(), value))
            .collect(),
        None => Vec::new(),
    }
}

/// Grow, shrink and basis, or [`None`] when `value` is not a `flex`.
fn three(value: &str) -> Option<[String; 3]> {
    let parts = crate::list_style::parts(value)?;
    if let [only] = parts.as_slice() {
        if only.eq_ignore_ascii_case("none") {
            return Some(written("0", "0", "auto"));
        }
        if only.eq_ignore_ascii_case("auto") {
            return Some(written("1", "1", "auto"));
        }
    }
    let mut factors: Vec<&str> = Vec::new();
    let mut basis: Option<&str> = None;
    // Where the factors are in the value: they must stand together.
    let mut factors_ended = false;
    for part in &parts {
        // A zero is a factor until there are two of them.
        if is_factor(part) && factors.len() < 2 && !factors_ended {
            factors.push(part);
            continue;
        }
        if !factors.is_empty() {
            factors_ended = true;
        }
        if basis.is_some() || !is_basis(part) {
            return None;
        }
        basis = Some(part);
    }
    if factors.is_empty() && basis.is_none() {
        return None;
    }
    let grow = factors.first().copied().unwrap_or("1");
    let shrink = factors.get(1).copied().unwrap_or("1");
    let basis = match basis {
        // The one place a unitless zero is a length.
        Some(zero) if is_number(zero) => "0px",
        Some(basis) => basis,
        None => "0%",
    };
    Some(written(grow, shrink, basis))
}

/// The three values, owned.
fn written(grow: &str, shrink: &str, basis: &str) -> [String; 3] {
    [grow.to_owned(), shrink.to_owned(), basis.to_owned()]
}

/// Whether a part is a flex factor: a number that is not negative.
fn is_factor(part: &str) -> bool {
    is_number(part) && !part.starts_with('-')
}

/// Whether a part can be a basis: anything that is not a number, a
/// keyword every property takes, or `none` — except a zero, which is a
/// length when it is written where only a basis can be.
fn is_basis(part: &str) -> bool {
    if is_number(part) {
        return part.trim_start_matches('+').parse::<f64>() == Ok(0.0);
    }
    !part.eq_ignore_ascii_case("none") && !is_wide_keyword(part)
}

/// Whether a part is a CSS number: digits with a sign, a point and an
/// exponent, and nothing else.
fn is_number(part: &str) -> bool {
    let unsigned = part.strip_prefix(['+', '-']).unwrap_or(part);
    unsigned.starts_with(|letter: char| letter.is_ascii_digit() || letter == '.')
        && unsigned
            .chars()
            .all(|letter| letter.is_ascii_digit() || matches!(letter, '.' | 'e' | 'E' | '+' | '-'))
        && unsigned.parse::<f64>().is_ok_and(f64::is_finite)
}

/// Whether a value is one of the keywords every property takes.
fn is_wide_keyword(value: &str) -> bool {
    ["inherit", "initial", "unset", "revert", "revert-layer"]
        .iter()
        .any(|keyword| value.eq_ignore_ascii_case(keyword))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split_into(value: &str) -> Option<[String; 3]> {
        let split = split(value);
        let values: Vec<String> = split.into_iter().map(|(_, value)| value).collect();
        values.try_into().ok()
    }

    fn flex(grow: &str, shrink: &str, basis: &str) -> [String; 3] {
        written(grow, shrink, basis)
    }

    #[test]
    fn the_longhands_come_out_in_order_with_their_names() {
        let names: Vec<String> = split("1").into_iter().map(|(name, _)| name).collect();
        assert_eq!(names, ["flex-grow", "flex-shrink", "flex-basis"]);
    }

    #[test]
    fn one_number_grows_from_nothing() {
        assert_eq!(split_into("1"), Some(flex("1", "1", "0%")));
        assert_eq!(
            split_into("0"),
            Some(flex("0", "1", "0%")),
            "a zero is a factor"
        );
        assert_eq!(split_into(".5"), Some(flex(".5", "1", "0%")));
        assert_eq!(split_into("1.5e1"), Some(flex("1.5e1", "1", "0%")));
    }

    #[test]
    fn none_and_auto_are_their_own_three() {
        assert_eq!(split_into("none"), Some(flex("0", "0", "auto")));
        assert_eq!(split_into("AUTO"), Some(flex("1", "1", "auto")));
    }

    #[test]
    fn two_numbers_are_grow_and_shrink() {
        assert_eq!(split_into("2 3"), Some(flex("2", "3", "0%")));
        assert_eq!(split_into("1 0"), Some(flex("1", "0", "0%")));
        assert_eq!(split_into("0 0"), Some(flex("0", "0", "0%")));
    }

    #[test]
    fn a_basis_stands_before_or_after_the_factors() {
        assert_eq!(split_into("30px"), Some(flex("1", "1", "30px")));
        assert_eq!(split_into("1 30px"), Some(flex("1", "1", "30px")));
        assert_eq!(split_into("30px 2"), Some(flex("2", "1", "30px")));
        assert_eq!(split_into("1 2 3px"), Some(flex("1", "2", "3px")));
        assert_eq!(split_into("3px 1 2"), Some(flex("1", "2", "3px")));
        assert_eq!(split_into("1 auto"), Some(flex("1", "1", "auto")));
        assert_eq!(split_into("auto 1"), Some(flex("1", "1", "auto")));
        assert_eq!(split_into("0 0 auto"), Some(flex("0", "0", "auto")));
        assert_eq!(split_into("content"), Some(flex("1", "1", "content")));
        assert_eq!(
            split_into("calc(10px + 1em)"),
            Some(flex("1", "1", "calc(10px + 1em)"))
        );
        assert_eq!(split_into("1 calc(5px)"), Some(flex("1", "1", "calc(5px)")));
    }

    #[test]
    fn a_zero_after_two_factors_is_a_length() {
        assert_eq!(split_into("1 1 0"), Some(flex("1", "1", "0px")));
        assert_eq!(split_into("0 0 0"), Some(flex("0", "0", "0px")));
    }

    #[test]
    fn a_wide_keyword_sets_all_three_and_is_nothing_beside_another_part() {
        assert_eq!(
            split_into("initial"),
            Some(flex("initial", "initial", "initial"))
        );
        assert_eq!(split_into("inherit 1"), None);
    }

    #[test]
    fn what_chrome_refuses_is_left_whole() {
        for value in [
            "-1",
            "1 -2",
            "1 2 3",
            "1 0px 2",
            "30px 40px",
            "none 1",
            "1 none",
            "auto auto",
            "1 2 3px 4",
            "",
            "  ",
        ] {
            assert_eq!(split_into(value), None, "{value:?}");
        }
    }

    #[test]
    fn a_value_holding_a_variable_is_left_whole() {
        assert_eq!(split_into("var(--grow)"), None);
        assert_eq!(split_into("1 var(--basis)"), None);
    }

    #[test]
    fn a_hostile_value_is_split_or_left_whole_without_a_panic() {
        let long = "9".repeat(10_000);
        for value in [
            "1 calc(",
            "calc((((",
            "))) 1",
            "1e999",
            "+",
            "-",
            ".",
            "e1",
            "1e",
            "١",
            "1\u{0}",
            long.as_str(),
            "\"1\"",
            "'unclosed",
        ] {
            let _ = split(value);
        }
        assert_eq!(split_into("1 calc("), None, "an unclosed function");
    }
}
