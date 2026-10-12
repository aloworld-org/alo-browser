/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Constraint validation: whether a control may be submitted as it stands,
//! and if not, why, in the browser's words (ADR 0051 § 3).
//!
//! Each constraint is one of HTML's, a pure function of the document and
//! the control. A control **barred from constraint validation** is never
//! asked: a disabled one, a hidden input, a read-only one, a button that
//! does not submit, and anything inside a `datalist`. Chrome 154 bars a
//! read-only checkbox too, where HTML gives `readonly` no meaning on one;
//! that was measured (`willValidate` is `false`), and this follows Chrome.
//!
//! # Built, and what is said
//!
//! - **`valueMissing`**, with Chrome 154's words for each kind of control:
//!   `Please fill out this field.` for a field, `Please check this box if
//!   you want to proceed.`, `Please select one of these options.` for a
//!   radio group, `Please select an item in the list.` and `Please select a
//!   file.`
//! - **`typeMismatch`** for `email` ([`crate::email`], in Chrome's several
//!   sentences) and `url`, `Please enter a URL.`
//!
//! # Not built, and refused rather than skipped
//!
//! A constraint not built is **not skipped**: if it could fail on the value
//! the control holds, the control is [`Problem::Unchecked`] and its form
//! does not submit, refused by name (ADR 0013 § 3). It can fail only when
//! the attribute that sets it is present *and* the value is one it could
//! reject:
//!
//! - **`patternMismatch`** — a `pattern` and a value (queue item 473);
//! - **`tooShort`** — a `minlength` and a shorter value. It fails only once
//!   the value is a person's or an agent's edit, which nothing here can tell
//!   from the markup's until item 469 holds the dirty value flag;
//! - **`rangeUnderflow`**, **`rangeOverflow`** — a `min` or a `max` and a
//!   value (473). **`stepMismatch`** cannot fail here: its base is the `min`
//!   when there is one, which is already refused, and otherwise the `value`
//!   attribute, which until item 469 *is* the value;
//! - **`typeMismatch`** on an email address whose domain is not ASCII,
//!   which Chrome turns into punycode before judging ([`crate::email`]), and
//!   on a URL that has a scheme and does not parse, which Chrome judges by
//!   rules of its own (`https://a b` is valid there).
//!
//! # What cannot fail here, and why that is the truth
//!
//! - **`tooLong`** fails only after a person's edit past `maxlength`. A
//!   person cannot type past it, and an agent's `PutText` past it is
//!   refused before the page is asked (§ 3), so no value here ever does.
//! - **`badInput`** is a value typed that the control could not parse, and
//!   this engine refuses such text rather than holding it (ADR 0042).
//! - **`customError`** is set only by `setCustomValidity`, queue item 471.

use crate::document::Document;
use crate::email::{self, Judged};
use crate::kind::{self, Kind};
use crate::node::NodeId;
use crate::owner;
use crate::value;
use core::fmt;

/// One of HTML's constraints, by the name `ValidityState` gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constraint {
    /// `valueMissing`.
    ValueMissing,
    /// `typeMismatch`.
    TypeMismatch,
    /// `patternMismatch`.
    PatternMismatch,
    /// `tooLong`.
    TooLong,
    /// `tooShort`.
    TooShort,
    /// `rangeUnderflow`.
    RangeUnderflow,
    /// `rangeOverflow`.
    RangeOverflow,
    /// `stepMismatch`.
    StepMismatch,
    /// `badInput`.
    BadInput,
    /// `customError`.
    CustomError,
}

impl Constraint {
    /// Every constraint, in the order HTML lists them and Chrome chooses
    /// the one a message is about.
    pub const ALL: [Constraint; 10] = [
        Constraint::ValueMissing,
        Constraint::TypeMismatch,
        Constraint::PatternMismatch,
        Constraint::TooLong,
        Constraint::TooShort,
        Constraint::RangeUnderflow,
        Constraint::RangeOverflow,
        Constraint::StepMismatch,
        Constraint::BadInput,
        Constraint::CustomError,
    ];

    /// Its name, as `ValidityState` spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Constraint::ValueMissing => "valueMissing",
            Constraint::TypeMismatch => "typeMismatch",
            Constraint::PatternMismatch => "patternMismatch",
            Constraint::TooLong => "tooLong",
            Constraint::TooShort => "tooShort",
            Constraint::RangeUnderflow => "rangeUnderflow",
            Constraint::RangeOverflow => "rangeOverflow",
            Constraint::StepMismatch => "stepMismatch",
            Constraint::BadInput => "badInput",
            Constraint::CustomError => "customError",
        }
    }
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What is wrong with a control's value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// It fails this constraint, and this is the browser's message.
    Fails {
        /// Which.
        constraint: Constraint,
        /// What Chrome 154 says, word for word.
        message: String,
    },
    /// It could fail this constraint, which is not built, so nobody can say
    /// whether it does.
    Unchecked {
        /// Which.
        constraint: Constraint,
    },
}

/// Chrome 154's words for a field nobody filled in.
const FILL_OUT: &str = "Please fill out this field.";

/// Whether `node` is a candidate for constraint validation: a submittable
/// element that is not barred from it.
pub fn is_candidate(document: &Document, node: NodeId) -> bool {
    let Some(element) = document.element(node) else {
        return false;
    };
    if !owner::is_submittable(document, node)
        || crate::disabled::is_disabled(document, node, element)
    {
        return false;
    }
    let kind = kind::of(element).or_else(|| kind::of_button(element));
    if matches!(kind, Some(Kind::Hidden | Kind::Reset | Kind::Button)) {
        return false;
    }
    let reads_only = element.name.is_html("input") || element.name.is_html("textarea");
    if reads_only && element.attr("readonly").is_some() {
        return false;
    }
    !in_a_datalist(document, node)
}

/// What is wrong with the control `node`, if anything: the first
/// constraint, in [`Constraint::ALL`]'s order, that it fails or could fail.
/// [`None`] for a control that is not a candidate.
pub fn problem(document: &Document, node: NodeId) -> Option<Problem> {
    if !is_candidate(document, node) {
        return None;
    }
    value_missing(document, node)
        .or_else(|| type_mismatch(document, node))
        .or_else(|| unchecked(document, node))
}

/// What is wrong with each control `form` owns, in tree order.
pub fn problems(document: &Document, form: NodeId) -> Vec<(NodeId, Problem)> {
    owner::controls(document, form)
        .into_iter()
        .filter_map(|node| problem(document, node).map(|problem| (node, problem)))
        .collect()
}

fn fails(constraint: Constraint, message: impl Into<String>) -> Problem {
    Problem::Fails {
        constraint,
        message: message.into(),
    }
}

fn could_fail(constraint: Constraint) -> Problem {
    Problem::Unchecked { constraint }
}

/// `valueMissing`.
fn value_missing(document: &Document, node: NodeId) -> Option<Problem> {
    let element = document.element(node)?;
    let missing = Constraint::ValueMissing;
    if element.name.is_html("select") {
        return (element.attr("required").is_some() && !has_a_choice(document, node))
            .then(|| fails(missing, "Please select an item in the list."));
    }
    if element.name.is_html("textarea") {
        return (element.attr("required").is_some() && value::value(document, node).is_empty())
            .then(|| fails(missing, FILL_OUT));
    }
    let kind = kind::of(element)?;
    if kind == Kind::Radio {
        // A group is missing its choice when any radio in it is required,
        // and every radio in it says so.
        let group = crate::activation::group(document, node);
        let group = if group.is_empty() { vec![node] } else { group };
        let required = group.iter().any(|radio| {
            document
                .element(*radio)
                .is_some_and(|radio| radio.attr("required").is_some())
        });
        let chosen = group.iter().any(|radio| value::checked(document, *radio));
        return (required && !chosen)
            .then(|| fails(missing, "Please select one of these options."));
    }
    if !kind.can_be_required() || element.attr("required").is_none() {
        return None;
    }
    match kind {
        Kind::Checkbox if !value::checked(document, node) => Some(fails(
            missing,
            "Please check this box if you want to proceed.",
        )),
        // No file is ever chosen without a window to choose it in (ADR 0051
        // § 8), so a required one is always missing.
        Kind::File => Some(fails(missing, "Please select a file.")),
        Kind::Checkbox => None,
        _ if value::value(document, node).is_empty() => Some(fails(missing, FILL_OUT)),
        _ => None,
    }
}

/// Whether a `<select>` has an option chosen that is not its placeholder.
///
/// Its placeholder is HTML's *placeholder label option*: in a select that
/// takes one option and shows one, the first option, if its value is empty
/// and its parent is the select rather than an `optgroup`.
fn has_a_choice(document: &Document, select: NodeId) -> bool {
    let chosen = value::selected(document, select);
    let Some(element) = document.element(select) else {
        return false;
    };
    let placeholder = (element.attr("multiple").is_none()
        && value::display_size(document, select) == 1)
        .then(|| value::options(document, select).first().copied())
        .flatten()
        .filter(|first| {
            document.parent(*first) == Some(select)
                && value::option_value(document, *first).is_empty()
        });
    match chosen.as_slice() {
        [] => false,
        [only] => Some(*only) != placeholder,
        _ => true,
    }
}

/// `typeMismatch`, for the two states that have it.
fn type_mismatch(document: &Document, node: NodeId) -> Option<Problem> {
    let element = document.element(node)?;
    let mismatch = Constraint::TypeMismatch;
    let held = value::value(document, node);
    match kind::of(element)? {
        Kind::Email => match email::judge(&held, element.attr("multiple").is_some()) {
            Judged::Valid => None,
            Judged::Invalid(message) => Some(fails(mismatch, message)),
            Judged::Unjudged => Some(could_fail(mismatch)),
        },
        Kind::Url if held.is_empty() => None,
        Kind::Url if !has_a_scheme(&held) => Some(fails(mismatch, "Please enter a URL.")),
        Kind::Url if alo_url::parse(&held).is_err() => Some(could_fail(mismatch)),
        _ => None,
    }
}

/// Whether `text` begins with a URL's scheme and its colon: a letter, then
/// letters, digits, `+`, `-` and `.`. Without one, a URL cannot be absolute,
/// and Chrome says `Please enter a URL.` (`nope`, `//a.b`).
fn has_a_scheme(text: &str) -> bool {
    let Some((scheme, _)) = text.split_once(':') else {
        return false;
    };
    let mut characters = scheme.chars();
    characters.next().is_some_and(|c| c.is_ascii_alphabetic())
        && characters.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// The constraints this engine does not check, when the value is one they
/// could reject.
fn unchecked(document: &Document, node: NodeId) -> Option<Problem> {
    let element = document.element(node)?;
    let kind = kind::of(element);
    let typed = element.name.is_html("textarea") || kind.is_some_and(Kind::is_typed_text);
    let held = value::value(document, node);
    if held.is_empty() && kind != Some(Kind::Range) {
        return None;
    }
    if kind.is_some_and(Kind::is_typed_text) && element.attr("pattern").is_some() {
        return Some(could_fail(Constraint::PatternMismatch));
    }
    if typed
        && let Some(least) = element.attr("minlength").and_then(non_negative)
        && held.encode_utf16().count() < least
    {
        return Some(could_fail(Constraint::TooShort));
    }
    if kind.is_some_and(Kind::is_ranged) {
        if element.attr("min").is_some() {
            return Some(could_fail(Constraint::RangeUnderflow));
        }
        if element.attr("max").is_some() {
            return Some(could_fail(Constraint::RangeOverflow));
        }
    }
    None
}

/// A number read by HTML's *rules for parsing non-negative integers*: white
/// space before it, a sign, and the digits up to the first thing that is
/// not one. [`None`] when there are no digits or the number is below nought.
pub fn non_negative(text: &str) -> Option<usize> {
    let text = text.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let digits = rest.get(..end).unwrap_or_default();
    if digits.is_empty() {
        return None;
    }
    // A number too long for a `usize` is larger than any value is long.
    let number = digits.parse::<usize>().unwrap_or(usize::MAX);
    (!negative || number == 0).then_some(number)
}

/// Whether `node` has a `datalist` ancestor.
fn in_a_datalist(document: &Document, node: NodeId) -> bool {
    let mut at = document.parent(node);
    while let Some(ancestor) = at {
        if document
            .element(ancestor)
            .is_some_and(|element| element.name.is_html("datalist"))
        {
            return true;
        }
        at = document.parent(ancestor);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_document;

    fn named(document: &Document, name: &str) -> NodeId {
        let Some(found) = document.descendants(document.root()).find(|node| {
            document
                .element(*node)
                .is_some_and(|element| element.attr("name") == Some(name))
        }) else {
            panic!("nothing named {name}");
        };
        found
    }

    /// What is said about each control, as `validationMessage` says it, or
    /// `unchecked <constraint>`.
    fn said(document: &Document, name: &str) -> String {
        match problem(document, named(document, name)) {
            None => String::new(),
            Some(Problem::Fails { message, .. }) => message,
            Some(Problem::Unchecked { constraint }) => format!("unchecked {constraint}"),
        }
    }

    /// Chrome 154's `validationMessage` for each, on the same markup.
    #[test]
    fn a_missing_value_is_said_as_chrome_says_it() {
        let document = parse_document(
            "<form><input name=t required><input name=t2 required value=' '>\
             <input type=checkbox name=c required>\
             <input type=radio name=r required value=1><input type=radio name=r value=2>\
             <input type=radio name=q value=1><input type=radio name=q value=2 checked required>\
             <select name=s required><option value=''>pick</option><option>a</option></select>\
             <select name=s2 required><option value=''>pick</option>\
             <option selected value=''>x</option></select>\
             <select name=s3 required size=2><option value=''>p</option></select>\
             <select name=s4 required><optgroup><option value=''>p</option></optgroup>\
             <option>x</option></select>\
             <select name=s6 required></select>\
             <select name=s7 required multiple><option value=''>p</option></select>\
             <input type=file name=fi required><textarea name=ta required></textarea>\
             <input type=date name=d required><input type=number name=n required>\
             <input type=email name=em required multiple>\
             <input type=range name=ra required><input type=color name=co required></form>",
        );
        let cases = [
            ("t", FILL_OUT),
            ("t2", ""),
            ("c", "Please check this box if you want to proceed."),
            ("r", "Please select one of these options."),
            ("q", ""),
            ("s", "Please select an item in the list."),
            ("s2", ""),
            ("s3", "Please select an item in the list."),
            ("s4", ""),
            ("s6", "Please select an item in the list."),
            ("s7", "Please select an item in the list."),
            ("fi", "Please select a file."),
            ("ta", FILL_OUT),
            ("d", FILL_OUT),
            ("n", FILL_OUT),
            ("em", FILL_OUT),
            ("ra", ""),
            ("co", ""),
        ];
        for (name, expected) in cases {
            assert_eq!(said(&document, name), expected, "{name}");
        }
        // Both radios in a group with nothing chosen are missing it.
        let radios: Vec<NodeId> = document
            .descendants(document.root())
            .filter(|node| {
                document
                    .element(*node)
                    .is_some_and(|element| element.attr("name") == Some("r"))
            })
            .collect();
        assert_eq!(radios.len(), 2);
        assert!(
            radios
                .iter()
                .all(|radio| matches!(problem(&document, *radio), Some(Problem::Fails { .. })))
        );
    }

    #[test]
    fn a_barred_control_is_never_asked() {
        let document = parse_document(
            "<form><input type=hidden name=h required>\
             <input name=ro required readonly><input name=di required disabled>\
             <input type=checkbox name=cro required readonly>\
             <textarea name=tro required readonly></textarea>\
             <fieldset disabled><input name=fs required>\
             <legend><input name=lg required></legend></fieldset>\
             <datalist><input name=dl required></datalist>\
             <button type=reset name=rs>r</button><input type=submit name=sb required></form>",
        );
        for barred in ["h", "ro", "di", "cro", "tro", "fs", "dl", "rs"] {
            assert!(
                !is_candidate(&document, named(&document, barred)),
                "{barred}"
            );
            assert_eq!(said(&document, barred), "", "{barred}");
        }
        // A fieldset's first legend is not disabled with it.
        assert_eq!(said(&document, "lg"), FILL_OUT);
        assert!(is_candidate(&document, named(&document, "sb")));
        assert_eq!(
            said(&document, "sb"),
            "",
            "a submit button has no constraint"
        );
    }

    #[test]
    fn a_mismatched_type_is_said_and_one_nobody_can_judge_is_unchecked() {
        let document = parse_document(
            "<form><input type=email name=e1 value=nope>\
             <input type=email name=e2 value=' a@b.c '><input type=email name=e3 value='a@é'>\
             <input type=url name=u1 value=nope><input type=url name=u2 value=' https://a.b '>\
             <input type=url name=u3 value='a:b'><input type=url name=u4 value='//a.b'>\
             <input type=url name=u5 value='https://a b'></form>",
        );
        let cases = [
            (
                "e1",
                "Please include an '@' in the email address. 'nope' is missing an '@'.",
            ),
            ("e2", ""),
            ("e3", "unchecked typeMismatch"),
            ("u1", "Please enter a URL."),
            ("u2", ""),
            ("u3", ""),
            ("u4", "Please enter a URL."),
            ("u5", "unchecked typeMismatch"),
        ];
        for (name, expected) in cases {
            assert_eq!(said(&document, name), expected, "{name}");
        }
    }

    #[test]
    fn a_constraint_not_built_is_unchecked_only_where_it_could_fail() {
        let document = parse_document(
            "<form><input name=pt pattern='[0-9]+' value=ab><input name=pt0 pattern='[0-9]+'>\
             <input name=mn minlength=5 value=ab><input name=mn2 minlength=2 value=ab>\
             <textarea name=ta minlength=3>ab</textarea>\
             <input name=ml maxlength=2 value=abcd>\
             <input type=number name=nm min=5 value=1><input type=number name=nx max=5 value=9>\
             <input type=number name=ns step=2 value=3><input type=number name=n0 min=5>\
             <input type=date name=dm min=2026-01-01 value=2026-10-20>\
             <input type=time name=delivery min=11:00 max=21:00 step=900>\
             <input type=range name=ra min=0></form>",
        );
        let cases = [
            ("pt", "unchecked patternMismatch"),
            ("pt0", ""),
            ("mn", "unchecked tooShort"),
            ("mn2", ""),
            ("ta", "unchecked tooShort"),
            ("ml", ""),
            ("nm", "unchecked rangeUnderflow"),
            ("nx", "unchecked rangeOverflow"),
            ("ns", ""),
            ("n0", ""),
            ("dm", "unchecked rangeUnderflow"),
            ("delivery", ""),
            ("ra", "unchecked rangeUnderflow"),
        ];
        for (name, expected) in cases {
            assert_eq!(said(&document, name), expected, "{name}");
        }
    }

    #[test]
    fn a_forms_problems_are_its_own_controls_in_tree_order() {
        let document = parse_document(
            "<input name=before form=f required><form id=f><input name=a required>\
             <input name=b value=x required><input type=email name=c value=x></form>\
             <form><input name=other required></form>",
        );
        let Some(form) = document.descendants(document.root()).find(|node| {
            document
                .element(*node)
                .is_some_and(|element| element.attr("id") == Some("f"))
        }) else {
            panic!("the form");
        };
        let names: Vec<&str> = problems(&document, form)
            .into_iter()
            .filter_map(|(node, _)| document.element(node).and_then(|e| e.attr("name")))
            .collect();
        assert_eq!(names, ["before", "a", "c"]);
    }

    #[test]
    fn a_non_negative_integer_is_read_as_html_reads_one() {
        assert_eq!(non_negative("5"), Some(5));
        assert_eq!(non_negative(" 12 "), Some(12));
        assert_eq!(non_negative("+3"), Some(3));
        assert_eq!(non_negative("-1"), None);
        assert_eq!(non_negative("-0"), Some(0));
        assert_eq!(non_negative("1.5"), Some(1));
        assert_eq!(non_negative("7px"), Some(7));
        assert_eq!(non_negative(""), None);
        assert_eq!(non_negative("x1"), None);
        assert_eq!(non_negative("99999999999999999999999"), Some(usize::MAX));
    }
}
