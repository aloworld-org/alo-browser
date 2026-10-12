/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether an email field's value is an email address, and what Chrome says
//! when it is not (ADR 0051 § 3, `typeMismatch`).
//!
//! The rule is HTML's *valid email address*: a local part of the characters
//! it lists, an `@`, and a domain of labels made of letters, digits and
//! hyphens, each one to sixty-three long and neither starting nor ending in
//! a hyphen. With `multiple`, the value is a comma-separated list of them.
//!
//! Chrome 154 explains a failure in one of several sentences, chosen by
//! asking in a fixed order. The order and the words were measured — each
//! value below given to Chrome and its `validationMessage` read back — and
//! Chrome's source was not read:
//!
//! 1. no `@`: `Please include an '@' in the email address. 'nope' is
//!    missing an '@'.`
//! 2. nothing before the first `@`: `Please enter a part followed by '@'.
//!    '@b' is incomplete.`
//! 3. nothing after it: `Please enter a part following '@'. 'a@' is
//!    incomplete.` (asked before 4: `a b@` is said to be incomplete)
//! 4. the first character before it that a local part may not hold: `A part
//!    followed by '@' should not contain the symbol ' '.`
//! 5. the first character after it that a domain may not hold: `A part
//!    following '@' should not contain the symbol '_'.` (a second `@` is
//!    one: `a@b@c`)
//! 6. a domain beginning or ending with `.`, or holding `..`: `'.' is used
//!    at a wrong position in 'b..c'.`
//! 7. anything else, a label that begins or ends with a hyphen or is too
//!    long: `Please enter an email address.`
//!
//! With `multiple`, an empty address in the list is `Please enter a
//! non-empty email address.`, and the first address that fails is the one
//! explained.
//!
//! **A domain that is not ASCII is not judged here.** Chrome turns it into
//! punycode as the value is set (`a@é` becomes `a@xn--9ca`) and then judges
//! that, and this engine does not hold that conversion. Such an address is
//! [`Judged::Unjudged`], and the constraint refuses rather than guesses.

/// What became of one email field's value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Judged {
    /// Every address in it is valid.
    Valid,
    /// One is not, and this is what Chrome says about it.
    Invalid(String),
    /// One has a domain that is not ASCII, which this engine does not judge.
    Unjudged,
}

/// The characters a local part may hold, besides ASCII letters and digits.
const LOCAL_SYMBOLS: &str = ".!#$%&'*+/=?^_`{|}~-";

/// Judge the value of an email field — `multiple` when it takes a list —
/// already sanitised ([`crate::value::value`]). The empty value is valid:
/// whether one is required is `valueMissing`'s question.
pub fn judge(value: &str, multiple: bool) -> Judged {
    if value.is_empty() {
        return Judged::Valid;
    }
    if !multiple {
        return judge_one(value);
    }
    for address in value.split(',') {
        if address.is_empty() {
            return Judged::Invalid("Please enter a non-empty email address.".to_owned());
        }
        let judged = judge_one(address);
        if judged != Judged::Valid {
            return judged;
        }
    }
    Judged::Valid
}

/// Judge one address.
fn judge_one(address: &str) -> Judged {
    let Some((local, domain)) = address.split_once('@') else {
        return Judged::Invalid(format!(
            "Please include an '@' in the email address. '{address}' is missing an '@'."
        ));
    };
    if local.is_empty() {
        return Judged::Invalid(format!(
            "Please enter a part followed by '@'. '{address}' is incomplete."
        ));
    }
    if domain.is_empty() {
        return Judged::Invalid(format!(
            "Please enter a part following '@'. '{address}' is incomplete."
        ));
    }
    if let Some(symbol) = local
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || LOCAL_SYMBOLS.contains(*c)))
    {
        return Judged::Invalid(format!(
            "A part followed by '@' should not contain the symbol '{symbol}'."
        ));
    }
    if !domain.is_ascii() {
        return Judged::Unjudged;
    }
    if let Some(symbol) = domain
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-')))
    {
        return Judged::Invalid(format!(
            "A part following '@' should not contain the symbol '{symbol}'."
        ));
    }
    if domain.starts_with('.') || domain.ends_with('.') || domain.contains("..") {
        return Judged::Invalid(format!("'.' is used at a wrong position in '{domain}'."));
    }
    if domain.split('.').all(is_a_label) {
        Judged::Valid
    } else {
        Judged::Invalid("Please enter an email address.".to_owned())
    }
}

/// Whether `label` is one of a domain's labels: one to sixty-three letters,
/// digits and hyphens, not beginning or ending with a hyphen.
fn is_a_label(label: &str) -> bool {
    (1..=63).contains(&label.len()) && !label.starts_with('-') && !label.ends_with('-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(value: &str, multiple: bool) -> String {
        match judge(value, multiple) {
            Judged::Valid => String::new(),
            Judged::Invalid(message) => message,
            Judged::Unjudged => "unjudged".to_owned(),
        }
    }

    /// Each value and its `validationMessage` in Chrome 154.
    #[test]
    fn each_failure_is_explained_in_chromes_words() {
        let long = format!("a@{}.c", "x".repeat(64));
        let cases: &[(&str, bool, &str)] = &[
            (
                "nope",
                false,
                "Please include an '@' in the email address. 'nope' is missing an '@'.",
            ),
            (
                "a b",
                false,
                "Please include an '@' in the email address. 'a b' is missing an '@'.",
            ),
            (
                "@b",
                false,
                "Please enter a part followed by '@'. '@b' is incomplete.",
            ),
            (
                "@",
                false,
                "Please enter a part followed by '@'. '@' is incomplete.",
            ),
            (
                "a@",
                false,
                "Please enter a part following '@'. 'a@' is incomplete.",
            ),
            (
                "a b@",
                false,
                "Please enter a part following '@'. 'a b@' is incomplete.",
            ),
            (
                "a b@c",
                false,
                "A part followed by '@' should not contain the symbol ' '.",
            ),
            (
                "a(b@c",
                false,
                "A part followed by '@' should not contain the symbol '('.",
            ),
            (
                "é@b",
                false,
                "A part followed by '@' should not contain the symbol 'é'.",
            ),
            (
                "a@b c",
                false,
                "A part following '@' should not contain the symbol ' '.",
            ),
            (
                "a@b_c",
                false,
                "A part following '@' should not contain the symbol '_'.",
            ),
            (
                "a@@b",
                false,
                "A part following '@' should not contain the symbol '@'.",
            ),
            (
                "a@b@c",
                false,
                "A part following '@' should not contain the symbol '@'.",
            ),
            (
                "a@[1.2.3.4]",
                false,
                "A part following '@' should not contain the symbol '['.",
            ),
            (
                "a@b c.",
                false,
                "A part following '@' should not contain the symbol ' '.",
            ),
            ("a@.b", false, "'.' is used at a wrong position in '.b'."),
            ("a@b.", false, "'.' is used at a wrong position in 'b.'."),
            (
                "a@b..c",
                false,
                "'.' is used at a wrong position in 'b..c'.",
            ),
            ("a@.", false, "'.' is used at a wrong position in '.'."),
            ("a@-b", false, "Please enter an email address."),
            ("a@b-.c", false, "Please enter an email address."),
            ("a@b.c-", false, "Please enter an email address."),
            (&long, false, "Please enter an email address."),
            ("a@é", false, "unjudged"),
        ];
        for (value, multiple, expected) in cases {
            assert_eq!(said(value, *multiple), *expected, "{value:?}");
        }
    }

    /// Each list and its `validationMessage` in Chrome 154, with `multiple`.
    #[test]
    fn the_first_address_in_a_list_that_fails_is_the_one_explained() {
        let cases: &[(&str, bool, &str)] = &[
            (
                "a@b,c",
                true,
                "Please include an '@' in the email address. 'c' is missing an '@'.",
            ),
            ("a@b,,c@d", true, "Please enter a non-empty email address."),
            ("a@b,", true, "Please enter a non-empty email address."),
            (",", true, "Please enter a non-empty email address."),
            (
                "a@b,c d@e",
                true,
                "A part followed by '@' should not contain the symbol ' '.",
            ),
            ("a@b,c@d", true, ""),
        ];
        for (value, multiple, expected) in cases {
            assert_eq!(said(value, *multiple), *expected, "{value:?}");
        }
    }

    /// Values Chrome 154 found valid, `validationMessage` empty.
    #[test]
    fn what_html_calls_an_address_is_valid() {
        let longest = format!("a@{}.c", "x".repeat(63));
        let cases: &[(&str, bool, &str)] = &[
            (&longest, false, ""),
            (".a@b", false, ""),
            ("a..b@c", false, ""),
            ("a@1.2", false, ""),
            ("A@B.C", false, ""),
            ("a@b.c", false, ""),
            ("", false, ""),
        ];
        for (value, multiple, expected) in cases {
            assert_eq!(said(value, *multiple), *expected, "{value:?}");
        }
    }
}
