/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What is wrong with a pattern: an early `SyntaxError`, named.
//!
//! The same rule as [`error`](crate::error): each refusal is a variant rather
//! than a string, so a test asserts on it and a reworded message cannot
//! quietly turn a refusal into an acceptance.
//!
//! # A legacy form is refused as one
//!
//! ADR 0029 § 4. Annex B of the specification widens the pattern grammar for
//! old pages, and only when neither `u` nor `v` is given: a lone `]` or `{`
//! is a literal, `\c` before a digit is a backslash and a `c`, `\1` with no
//! group is an octal escape, `\a` is an `a`, and a lookahead may be repeated.
//! None of that is built. Each is [`Wrong::Legacy`], whose message says it is
//! an Annex B form, so a page that hits one is told exactly that rather than
//! being told its pattern is nonsense — and the line moves, in an amendment to
//! ADR 0029, when a frozen page needs one.
//!
//! Under `u` or `v` the same text is simply wrong, because Annex B never
//! applied there, and it is refused as what it is.

use std::fmt;

/// A pattern that is not one, and where in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternError {
    /// What is wrong.
    pub wrong: Wrong,
    /// The byte offset into the pattern's text, after the opening `/`.
    pub at: usize,
}

/// What is wrong with a pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Wrong {
    /// A flag string that is not a regular expression's, which only a pattern
    /// that did not come from a literal can have.
    BadFlags,
    /// A `)` with no `(` before it.
    UnopenedGroup,
    /// A `(` the pattern ended inside.
    UnclosedGroup,
    /// A `[` the pattern ended inside.
    UnclosedClass,
    /// A pattern ending in a lone `\`.
    EndsInABackslash,
    /// `*`, `+`, `?` or `{…}` with nothing before it that may be repeated.
    NothingToRepeat,
    /// `{3,1}`: a repeat whose least is more than its most.
    RepeatOutOfOrder,
    /// `[z-a]`: a range whose start is after its end.
    RangeOutOfOrder,
    /// `[\d-z]` under `u` or `v`: a class escape as one end of a range.
    ClassEscapeInARange,
    /// `{`, `}` or `]` standing alone under `u` or `v`.
    LoneBracket,
    /// An escape this mode does not have: `\a` or `\-` under `u`, an
    /// incomplete `\x` or `\u`, `\c` without a letter.
    BadEscape,
    /// A `\u{…}` naming more than U+10FFFF.
    CodePointOutOfRange,
    /// `\5` in a pattern with fewer than five groups, under `u` or `v`.
    NoSuchGroup,
    /// `\k<name>` where no group is called `name`.
    NoSuchName,
    /// Two groups of one name that could both take part in one match.
    RepeatedName,
    /// `(?<` with no name a group may have after it.
    BadGroupName,
    /// `(?` followed by nothing a group can begin with.
    BadGroup,
    /// `(?ii:…)`, `(?i-i:…)`, `(?-:…)` or a modifier that is not `i`, `m` or
    /// `s`.
    BadModifiers,
    /// A repeated lookbehind, which never means anything.
    RepeatedLookbehind,
    /// `\p` without a property in braces after it.
    BadProperty,
    /// `\P{…}`, or a `[^…]`, around a property or a class that may match a
    /// string of more than one character, which has no complement.
    NegatedStrings,
    /// `&&` and `--` mixed at one level of a `v` class, a range beside one, or
    /// an operator with nothing on one side.
    BadSetOperation,
    /// A doubled punctuator such as `!!` inside a `v` class, which the grammar
    /// reserves for operations not yet in the language.
    ReservedPunctuator,
    /// A character that must be escaped inside a `v` class — `(`, `)`, `{`,
    /// `}`, `/`, `-` or `|` — that was not.
    UnescapedInASet,
    /// More nesting than [`bounds::DEEPEST_PATTERN`](crate::bounds::DEEPEST_PATTERN).
    TooDeep,
    /// A form only Annex B allows, for old pages, and this engine has not
    /// built (ADR 0029 § 4).
    Legacy(Legacy),
}

/// The forms of Annex B's pattern grammar, each refused by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Legacy {
    /// A lone `{`, `}` or `]` read as itself.
    LoneBracket,
    /// `\c` not followed by a letter, read as a backslash and a `c`.
    ControlEscape,
    /// `\0` followed by a digit, or `\N` with fewer than `N` groups, read as an
    /// octal escape or as the digit itself.
    OctalEscape,
    /// A backslash before a letter or a digit that has no meaning — `\a`,
    /// `\k`, `\p` without `u` — read as the character itself.
    IdentityEscape,
    /// A repeated lookahead, `(?=a)*`.
    RepeatedLookahead,
    /// A class escape as one end of a range, `[\d-z]`, read as three
    /// characters.
    ClassEscapeInARange,
}

impl fmt::Display for Legacy {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Legacy::LoneBracket => "a lone `{`, `}` or `]` read as itself",
            Legacy::ControlEscape => "`\\c` without a letter after it",
            Legacy::OctalEscape => "an octal escape, or `\\N` with fewer than N groups",
            Legacy::IdentityEscape => "a backslash before a letter or digit with no meaning",
            Legacy::RepeatedLookahead => "a repeated lookahead",
            Legacy::ClassEscapeInARange => "a class escape as one end of a range",
        })
    }
}

impl fmt::Display for Wrong {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Wrong::BadFlags => out.write_str("these are not a regular expression's flags"),
            Wrong::UnopenedGroup => out.write_str("a `)` closes a group that was never opened"),
            Wrong::UnclosedGroup => out.write_str("a group was opened and never closed"),
            Wrong::UnclosedClass => out.write_str("a `[` class was opened and never closed"),
            Wrong::EndsInABackslash => out.write_str("a pattern cannot end in a lone `\\`"),
            Wrong::NothingToRepeat => out.write_str("there is nothing here that may be repeated"),
            Wrong::RepeatOutOfOrder => {
                out.write_str("a repeat's least number is more than its most")
            }
            Wrong::RangeOutOfOrder => out.write_str("a class range starts after it ends"),
            Wrong::ClassEscapeInARange => {
                out.write_str("a class escape cannot be one end of a range")
            }
            Wrong::LoneBracket => out.write_str("a lone `{`, `}` or `]` must be escaped"),
            Wrong::BadEscape => out.write_str("this escape means nothing here"),
            Wrong::CodePointOutOfRange => out.write_str("a `\\u{…}` names more than U+10FFFF"),
            Wrong::NoSuchGroup => out.write_str("a backreference names a group there is not"),
            Wrong::NoSuchName => out.write_str("`\\k` names a group no group is called"),
            Wrong::RepeatedName => {
                out.write_str("two groups of one name could both take part in a match")
            }
            Wrong::BadGroupName => out.write_str("a group's name is not a name"),
            Wrong::BadGroup => out.write_str("`(?` begins no kind of group"),
            Wrong::BadModifiers => out.write_str(
                "a group's modifiers are `i`, `m` and `s`, each at most once, and not `(?-:`",
            ),
            Wrong::RepeatedLookbehind => out.write_str("a lookbehind cannot be repeated"),
            Wrong::BadProperty => out.write_str("`\\p` needs a property in braces after it"),
            Wrong::NegatedStrings => {
                out.write_str("a class that may match a string has no complement")
            }
            Wrong::BadSetOperation => out.write_str(
                "a set operation needs an operand on each side and one operator per level",
            ),
            Wrong::ReservedPunctuator => {
                out.write_str("a doubled punctuator is reserved inside a `v` class")
            }
            Wrong::UnescapedInASet => {
                out.write_str("this character must be escaped in a `v` class")
            }
            Wrong::TooDeep => write!(
                out,
                "a pattern may nest at most {} deep",
                crate::bounds::DEEPEST_PATTERN
            ),
            Wrong::Legacy(legacy) => write!(
                out,
                "{legacy} is a form only Annex B of the specification allows, for old pages, \
                 and this engine refuses it until a page needs it (ADR 0029)"
            ),
        }
    }
}

impl fmt::Display for PatternError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.wrong.fmt(out)
    }
}

#[cfg(test)]
mod tests {
    use super::{Legacy, Wrong};

    #[test]
    fn a_legacy_form_says_it_is_one() {
        let message = Wrong::Legacy(Legacy::OctalEscape).to_string();
        assert!(message.contains("Annex B"), "{message}");
        assert!(!Wrong::NoSuchGroup.to_string().contains("Annex B"));
    }
}
