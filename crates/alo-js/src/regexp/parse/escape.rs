/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What follows a backslash, and a group's name.
//!
//! Most of the grammar's mode-dependence is here. Under `u` or `v` an escape
//! is one of a short closed list and anything else is wrong; without either,
//! the main grammar still refuses a backslash before a letter or a digit that
//! means nothing, and Annex B is what would have read it as itself — so that
//! is refused as Annex B's ([`Legacy::IdentityEscape`]).

use crate::unicode;

use super::super::surrogate::{combined, is_lead, is_trail};
use super::super::tree::{
    Assertion, Class, ClassBody, ClassEscape, ClassItem, Node, Property, Reference,
};
use super::super::wrong::{Legacy, PatternError, Wrong};
use super::{Reader, is_id_continue};

/// The properties that are sets of strings rather than of characters, which
/// only `v` may name. The grammar's own list, not a table.
const PROPERTIES_OF_STRINGS: [&str; 7] = [
    "Basic_Emoji",
    "Emoji_Keycap_Sequence",
    "RGI_Emoji_Modifier_Sequence",
    "RGI_Emoji_Flag_Sequence",
    "RGI_Emoji_Tag_Sequence",
    "RGI_Emoji_ZWJ_Sequence",
    "RGI_Emoji",
];

impl Reader {
    /// `AtomEscape`, after a `\` that was at `start` outside a class.
    pub(super) fn atom_escape(&mut self, start: usize) -> Result<Node, PatternError> {
        let Some(c) = self.peek().and_then(char::from_u32) else {
            return Err(self.wrong(Wrong::EndsInABackslash, start));
        };
        match c {
            'b' => {
                self.at = self.at.saturating_add(1);
                Ok(Node::Assertion(Assertion::Boundary))
            }
            'B' => {
                self.at = self.at.saturating_add(1);
                Ok(Node::Assertion(Assertion::NotBoundary))
            }
            '1'..='9' => {
                let (digits, after) = self.digits(self.at);
                self.at = after;
                let number = super::value_of(&digits);
                self.references.push((Reference::Number(number), start));
                Ok(Node::Backreference(Reference::Number(number)))
            }
            'k' => {
                if !self.unicode && !self.named {
                    return Err(self.wrong(Wrong::Legacy(Legacy::IdentityEscape), start));
                }
                self.at = self.at.saturating_add(1);
                if !self.eat('<') {
                    return Err(self.wrong(Wrong::BadEscape, start));
                }
                let name = self.group_name()?;
                self.references.push((Reference::Name(name.clone()), start));
                Ok(Node::Backreference(Reference::Name(name)))
            }
            'd' | 'D' | 's' | 'S' | 'w' | 'W' => {
                self.at = self.at.saturating_add(1);
                Ok(Node::Class(Class {
                    negated: false,
                    body: ClassBody::Ranges(vec![ClassItem::Escape(class_escape(c))]),
                }))
            }
            'p' | 'P' if self.unicode => {
                self.at = self.at.saturating_add(1);
                Ok(Node::Property(self.property(c == 'P', start)?))
            }
            _ => Ok(Node::Char(self.character_escape(start)?)),
        }
    }

    /// `CharacterEscape`, with the character after the `\` at `start` being
    /// looked at. Shared by an atom and a class, which differ only in what
    /// they take first.
    pub(super) fn character_escape(&mut self, start: usize) -> Result<u32, PatternError> {
        let Some(c) = self.next() else {
            return Err(self.wrong(Wrong::EndsInABackslash, start));
        };
        let Some(letter) = char::from_u32(c) else {
            return self.identity(c, start);
        };
        match letter {
            'f' => Ok(0x0C),
            'n' => Ok(0x0A),
            'r' => Ok(0x0D),
            't' => Ok(0x09),
            'v' => Ok(0x0B),
            'c' => match self.peek().and_then(char::from_u32) {
                Some(control) if control.is_ascii_alphabetic() => {
                    self.at = self.at.saturating_add(1);
                    Ok(u32::from(control) % 32)
                }
                _ => Err(self.legacy_or(Legacy::ControlEscape, Wrong::BadEscape, start)),
            },
            '0' => {
                if self.peek().is_some_and(is_decimal) {
                    return Err(self.legacy_or(Legacy::OctalEscape, Wrong::BadEscape, start));
                }
                Ok(0)
            }
            '1'..='9' => Err(self.legacy_or(Legacy::OctalEscape, Wrong::BadEscape, start)),
            'x' => match self.hex(2) {
                Some(value) => Ok(value),
                None => Err(self.legacy_or(Legacy::IdentityEscape, Wrong::BadEscape, start)),
            },
            'u' => self.unicode_escape(start, self.unicode),
            _ => self.identity(c, start),
        }
    }

    /// A backslash before a character that is not an escape of its own.
    fn identity(&self, c: u32, start: usize) -> Result<u32, PatternError> {
        if self.unicode {
            let allowed = char::from_u32(c).is_some_and(|c| is_syntax_character(c) || c == '/');
            if allowed {
                return Ok(c);
            }
            return Err(self.wrong(Wrong::BadEscape, start));
        }
        if is_id_continue(c) {
            return Err(self.wrong(Wrong::Legacy(Legacy::IdentityEscape), start));
        }
        Ok(c)
    }

    /// `RegExpUnicodeEscapeSequence`, after its `u`. `full` is the `[+UnicodeMode]`
    /// form: `\u{…}`, and a pair of escapes that is one code point.
    fn unicode_escape(&mut self, start: usize, full: bool) -> Result<u32, PatternError> {
        if full && self.eat('{') {
            let mut value: u32 = 0;
            let mut any = false;
            while let Some(digit) = self.peek().and_then(hex_value) {
                any = true;
                value = value.saturating_mul(16).saturating_add(digit);
                if value > 0x10_FFFF {
                    return Err(self.wrong(Wrong::CodePointOutOfRange, start));
                }
                self.at = self.at.saturating_add(1);
            }
            if !any || !self.eat('}') {
                return Err(self.wrong(Wrong::BadEscape, start));
            }
            return Ok(value);
        }
        let Some(value) = self.hex(4) else {
            return Err(self.legacy_or(Legacy::IdentityEscape, Wrong::BadEscape, start));
        };
        if full && is_lead(value) && self.sees('\\') && self.peek_second() == Some(u32::from('u')) {
            let back = self.at;
            self.at = self.at.saturating_add(2);
            match self.hex(4) {
                Some(trail) if is_trail(trail) => return Ok(combined(value, trail)),
                _ => self.at = back,
            }
        }
        Ok(value)
    }

    /// Exactly `count` hexadecimal digits, taken, or nothing taken.
    fn hex(&mut self, count: usize) -> Option<u32> {
        let mut value: u32 = 0;
        for offset in 0..count {
            let digit = self
                .char_at(self.at.saturating_add(offset))
                .and_then(hex_value)?;
            value = value * 16 + digit;
        }
        self.at = self.at.saturating_add(count);
        Some(value)
    }

    /// `GroupName`, after its `<` and through its `>`.
    ///
    /// A name may spell a character with `\u`, in either mode, and without
    /// `u` a character outside the first plane arrives as two code units and
    /// is put back together.
    pub(super) fn group_name(&mut self) -> Result<Vec<u16>, PatternError> {
        let start = self.at;
        let mut name = Vec::new();
        loop {
            let Some(c) = self.next() else {
                return Err(self.wrong(Wrong::BadGroupName, start));
            };
            if c == u32::from('>') {
                if name.is_empty() {
                    return Err(self.wrong(Wrong::BadGroupName, start));
                }
                return Ok(name);
            }
            let point = if c == u32::from('\\') {
                if !self.eat('u') {
                    return Err(self.wrong(Wrong::BadGroupName, start));
                }
                self.unicode_escape(start, true)
                    .map_err(|_| self.wrong(Wrong::BadGroupName, start))?
            } else if is_lead(c) && self.peek().is_some_and(is_trail) {
                let trail = self.next().unwrap_or(0xDC00);
                combined(c, trail)
            } else {
                c
            };
            let allowed = char::from_u32(point).is_some_and(|point| {
                if name.is_empty() {
                    unicode::starts_a_name(point)
                } else {
                    unicode::continues_a_name(point)
                }
            });
            let Some(point) = char::from_u32(point).filter(|_| allowed) else {
                return Err(self.wrong(Wrong::BadGroupName, start));
            };
            let mut units = [0_u16; 2];
            name.extend_from_slice(point.encode_utf16(&mut units));
        }
    }

    /// `{Name=Value}` or `{NameOrValue}` after `\p` or `\P`. Only the form is
    /// checked: whether the name is a property is the tables' to say (queue
    /// item 322), and nothing compiles one until then.
    pub(super) fn property(
        &mut self,
        negated: bool,
        start: usize,
    ) -> Result<Property, PatternError> {
        if !self.eat('{') {
            return Err(self.wrong(Wrong::BadProperty, start));
        }
        // A lone name-or-value may have digits in it, as a value may; a name
        // before `=` may not.
        let name = self.property_word();
        let value = if self.eat('=') {
            if name.chars().any(|c| c.is_ascii_digit()) {
                return Err(self.wrong(Wrong::BadProperty, start));
            }
            Some(self.property_word())
        } else {
            None
        };
        if name.is_empty() || value.as_ref().is_some_and(String::is_empty) || !self.eat('}') {
            return Err(self.wrong(Wrong::BadProperty, start));
        }
        let property = Property {
            negated,
            name,
            value,
        };
        if negated && self.sets && is_of_strings(&property) {
            return Err(self.wrong(Wrong::NegatedStrings, start));
        }
        Ok(property)
    }

    /// The characters of a property's name or value: letters, digits and `_`.
    fn property_word(&mut self) -> String {
        let mut word = String::new();
        while let Some(c) = self
            .peek()
            .and_then(char::from_u32)
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        {
            word.push(c);
            self.at = self.at.saturating_add(1);
        }
        word
    }
}

/// Whether a property names a set of strings, which only `v` may use.
pub(super) fn is_of_strings(property: &Property) -> bool {
    property.value.is_none() && PROPERTIES_OF_STRINGS.contains(&property.name.as_str())
}

/// The class escape a letter names.
pub(super) const fn class_escape(c: char) -> ClassEscape {
    match c {
        'd' => ClassEscape::Digit,
        'D' => ClassEscape::NotDigit,
        's' => ClassEscape::Space,
        'S' => ClassEscape::NotSpace,
        'w' => ClassEscape::Word,
        _ => ClassEscape::NotWord,
    }
}

/// `SyntaxCharacter`: the characters a pattern gives a meaning of their own.
pub(super) const fn is_syntax_character(c: char) -> bool {
    matches!(
        c,
        '^' | '$' | '\\' | '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
    )
}

/// Whether a character is a decimal digit.
pub(super) fn is_decimal(c: u32) -> bool {
    (u32::from('0')..=u32::from('9')).contains(&c)
}

/// A hexadecimal digit's value.
fn hex_value(c: u32) -> Option<u32> {
    char::from_u32(c)?.to_digit(16)
}
