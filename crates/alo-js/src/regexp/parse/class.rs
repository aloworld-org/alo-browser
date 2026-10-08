/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `[…]`, in the language's two grammars for one.
//!
//! Without `v` a class is characters, ranges and escapes, and the one subtle
//! thing is the dash: `[a-]`, `[-a]` and `[a-b-c]` each have a `-` that is a
//! character, and only a dash with a character on both sides makes a range.
//!
//! With `v` a class is a **set expression**: a union, or `&&` intersections,
//! or `--` differences, one kind per level, of operands that may themselves
//! be classes and may be strings (`\q{abc}`). The punctuation that `v`
//! reserves for later — `!!`, `##` and the rest — and the characters it
//! requires escaped are refused here. None of it compiles yet (queue item
//! 324), but whether a `v` pattern is a pattern is decided now.

use super::super::tree::{Class, ClassBody, ClassItem, SetExpression, SetOperand};
use super::super::wrong::{Legacy, PatternError, Wrong};
use super::Reader;
use super::escape::{class_escape, is_decimal, is_of_strings};

/// A class member before it is known whether a range follows.
enum Atom {
    /// A character, which may begin or end a range.
    Char(u32),
    /// Anything else, which may not.
    Other(ClassItem),
}

impl Reader {
    /// `CharacterClass`, at its `[`.
    pub(super) fn class(&mut self) -> Result<Class, PatternError> {
        let start = self.at;
        self.at = self.at.saturating_add(1);
        let negated = self.eat('^');
        if self.sets {
            return self.set_class(start, negated);
        }
        let body = ClassBody::Ranges(self.ranges(start)?);
        Ok(Class { negated, body })
    }

    /// `ClassContents` without `v`, through the closing `]`.
    fn ranges(&mut self, start: usize) -> Result<Vec<ClassItem>, PatternError> {
        let mut items = Vec::new();
        loop {
            if self.peek().is_none() {
                return Err(self.wrong(Wrong::UnclosedClass, start));
            }
            if self.eat(']') {
                return Ok(items);
            }
            let first_at = self.at;
            let first = self.class_atom(start)?;
            let ranged = self.sees('-')
                && self
                    .peek_second()
                    .is_some_and(|after| after != u32::from(']'));
            if !ranged {
                items.push(match first {
                    Atom::Char(c) => ClassItem::Char(c),
                    Atom::Other(item) => item,
                });
                continue;
            }
            self.at = self.at.saturating_add(1);
            let last = self.class_atom(start)?;
            match (first, last) {
                (Atom::Char(from), Atom::Char(to)) => {
                    if from > to {
                        return Err(self.wrong(Wrong::RangeOutOfOrder, first_at));
                    }
                    items.push(ClassItem::Range(from, to));
                }
                _ => {
                    return Err(self.legacy_or(
                        Legacy::ClassEscapeInARange,
                        Wrong::ClassEscapeInARange,
                        first_at,
                    ));
                }
            }
        }
    }

    /// `ClassAtom`: a character, or a `\` and what it means inside a class.
    fn class_atom(&mut self, start: usize) -> Result<Atom, PatternError> {
        let Some(c) = self.next() else {
            return Err(self.wrong(Wrong::UnclosedClass, start));
        };
        if c != u32::from('\\') {
            return Ok(Atom::Char(c));
        }
        let escape_at = self.at.saturating_sub(1);
        let Some(letter) = self.peek().and_then(char::from_u32) else {
            return Err(self.wrong(Wrong::EndsInABackslash, escape_at));
        };
        match letter {
            'b' => {
                self.at = self.at.saturating_add(1);
                Ok(Atom::Char(0x08))
            }
            '-' if self.unicode => {
                self.at = self.at.saturating_add(1);
                Ok(Atom::Char(u32::from('-')))
            }
            'd' | 'D' | 's' | 'S' | 'w' | 'W' => {
                self.at = self.at.saturating_add(1);
                Ok(Atom::Other(ClassItem::Escape(class_escape(letter))))
            }
            'p' | 'P' if self.unicode => {
                self.at = self.at.saturating_add(1);
                Ok(Atom::Other(ClassItem::Property(
                    self.property(letter == 'P', escape_at)?,
                )))
            }
            _ => Ok(Atom::Char(self.character_escape(escape_at)?)),
        }
    }

    // --- `v` ----------------------------------------------------------------

    /// A `v` class after its `[` and any `^`, through its `]`, one level
    /// deeper than the class it is in.
    fn set_class(&mut self, start: usize, negated: bool) -> Result<Class, PatternError> {
        self.enter(start)?;
        let outcome = self.set_expression(start);
        self.leave();
        let expression = outcome?;
        if !self.eat(']') {
            return Err(self.wrong(Wrong::UnclosedClass, start));
        }
        if negated && may_contain_strings(&expression) {
            return Err(self.wrong(Wrong::NegatedStrings, start));
        }
        Ok(Class {
            negated,
            body: ClassBody::Set(expression),
        })
    }

    /// `ClassSetExpression`, up to the `]` that ends it, which is left.
    fn set_expression(&mut self, start: usize) -> Result<SetExpression, PatternError> {
        if self.peek().is_none() {
            return Err(self.wrong(Wrong::UnclosedClass, start));
        }
        if self.sees(']') {
            return Ok(SetExpression::Union(Vec::new()));
        }
        let first_at = self.at;
        let first = self.set_member(start)?;
        if self.sees_double('&') || self.sees_double('-') {
            let intersection = self.sees_double('&');
            if matches!(first, SetOperand::Range(..)) {
                return Err(self.wrong(Wrong::BadSetOperation, first_at));
            }
            let mut operands = vec![first];
            let operator = if intersection { '&' } else { '-' };
            while self.sees_double(operator) {
                let operator_at = self.at;
                self.at = self.at.saturating_add(2);
                if intersection && self.sees('&') {
                    return Err(self.wrong(Wrong::BadSetOperation, operator_at));
                }
                if self.sees(']') {
                    return Err(self.wrong(Wrong::BadSetOperation, operator_at));
                }
                operands.push(self.set_operand(start)?);
            }
            if !self.sees(']') {
                let at = self.at;
                if self.peek().is_none() {
                    return Err(self.wrong(Wrong::UnclosedClass, start));
                }
                return Err(self.wrong(Wrong::BadSetOperation, at));
            }
            return Ok(if intersection {
                SetExpression::Intersection(operands)
            } else {
                SetExpression::Subtraction(operands)
            });
        }
        let mut members = vec![first];
        loop {
            if self.peek().is_none() {
                return Err(self.wrong(Wrong::UnclosedClass, start));
            }
            if self.sees(']') {
                return Ok(SetExpression::Union(members));
            }
            if self.sees_double('&') || self.sees_double('-') {
                return Err(self.wrong(Wrong::BadSetOperation, self.at));
            }
            members.push(self.set_member(start)?);
        }
    }

    /// A union's member: an operand, or a range between two characters.
    fn set_member(&mut self, start: usize) -> Result<SetOperand, PatternError> {
        let first_at = self.at;
        let operand = self.set_operand(start)?;
        let SetOperand::Char(from) = operand else {
            return Ok(operand);
        };
        if !self.sees('-') || self.sees_double('-') {
            return Ok(operand);
        }
        self.at = self.at.saturating_add(1);
        let to = self.set_character(start)?;
        if from > to {
            return Err(self.wrong(Wrong::RangeOutOfOrder, first_at));
        }
        Ok(SetOperand::Range(from, to))
    }

    /// `ClassSetOperand`: a nested class, `\q{…}`, an escape, or a character.
    fn set_operand(&mut self, start: usize) -> Result<SetOperand, PatternError> {
        let Some(c) = self.peek().and_then(char::from_u32) else {
            return Err(self.wrong(Wrong::UnclosedClass, start));
        };
        if c == '[' {
            let nested_at = self.at;
            self.at = self.at.saturating_add(1);
            let negated = self.eat('^');
            return Ok(SetOperand::Nested(self.set_class(nested_at, negated)?));
        }
        if c == '\\' {
            let escape_at = self.at;
            match self.peek_second().and_then(char::from_u32) {
                Some('q') => {
                    self.at = self.at.saturating_add(2);
                    return self.strings(escape_at);
                }
                Some(letter @ ('d' | 'D' | 's' | 'S' | 'w' | 'W')) => {
                    self.at = self.at.saturating_add(2);
                    return Ok(SetOperand::Escape(class_escape(letter)));
                }
                Some(letter @ ('p' | 'P')) => {
                    self.at = self.at.saturating_add(2);
                    return Ok(SetOperand::Property(
                        self.property(letter == 'P', escape_at)?,
                    ));
                }
                _ => {}
            }
        }
        Ok(SetOperand::Char(self.set_character(start)?))
    }

    /// `\q{…}`, after its `q`.
    fn strings(&mut self, escape_at: usize) -> Result<SetOperand, PatternError> {
        if !self.eat('{') {
            return Err(self.wrong(Wrong::BadEscape, escape_at));
        }
        let mut strings = vec![Vec::new()];
        loop {
            if self.peek().is_none() {
                return Err(self.wrong(Wrong::UnclosedClass, escape_at));
            }
            if self.eat('}') {
                return Ok(SetOperand::Strings(strings));
            }
            if self.eat('|') {
                strings.push(Vec::new());
                continue;
            }
            let c = self.set_character(escape_at)?;
            if let Some(string) = strings.last_mut() {
                string.push(c);
            }
        }
    }

    /// `ClassSetCharacter`.
    fn set_character(&mut self, start: usize) -> Result<u32, PatternError> {
        let at = self.at;
        let Some(c) = self.next() else {
            return Err(self.wrong(Wrong::UnclosedClass, start));
        };
        let Some(letter) = char::from_u32(c) else {
            return Ok(c);
        };
        if letter == '\\' {
            return match self.peek().and_then(char::from_u32) {
                Some(punctuator) if is_reserved_punctuator(punctuator) => {
                    self.at = self.at.saturating_add(1);
                    Ok(u32::from(punctuator))
                }
                Some('b') => {
                    self.at = self.at.saturating_add(1);
                    Ok(0x08)
                }
                Some('1'..='9') => Err(self.wrong(Wrong::BadEscape, at)),
                Some('0') if self.peek_second().is_some_and(is_decimal) => {
                    Err(self.wrong(Wrong::BadEscape, at))
                }
                _ => self.character_escape(at),
            };
        }
        if matches!(letter, '(' | ')' | '[' | ']' | '{' | '}' | '/' | '-' | '|') {
            return Err(self.wrong(Wrong::UnescapedInASet, at));
        }
        if is_double_punctuator(letter) && self.peek() == Some(c) {
            return Err(self.wrong(Wrong::ReservedPunctuator, at));
        }
        Ok(c)
    }

    /// Whether the next two characters are both `c`.
    fn sees_double(&self, c: char) -> bool {
        self.sees(c) && self.peek_second() == Some(u32::from(c))
    }
}

/// `ClassSetReservedPunctuator`: what a `\` may stand before in a `v` class
/// and nowhere else.
const fn is_reserved_punctuator(c: char) -> bool {
    matches!(
        c,
        '&' | '-' | '!' | '#' | '%' | ',' | ':' | ';' | '<' | '=' | '>' | '@' | '`' | '~'
    )
}

/// The characters whose doubling `v` reserves.
const fn is_double_punctuator(c: char) -> bool {
    matches!(
        c,
        '&' | '!'
            | '#'
            | '$'
            | '%'
            | '*'
            | '+'
            | ','
            | '.'
            | ':'
            | ';'
            | '<'
            | '='
            | '>'
            | '?'
            | '@'
            | '^'
            | '`'
            | '~'
    )
}

/// `MayContainStrings`: whether a set may hold a string of other than one
/// character, which a complement cannot be taken of.
fn may_contain_strings(expression: &SetExpression) -> bool {
    match expression {
        SetExpression::Union(operands) => operands.iter().any(operand_may_contain_strings),
        SetExpression::Intersection(operands) => {
            !operands.is_empty() && operands.iter().all(operand_may_contain_strings)
        }
        SetExpression::Subtraction(operands) => {
            operands.first().is_some_and(operand_may_contain_strings)
        }
    }
}

/// [`may_contain_strings`] of one operand.
fn operand_may_contain_strings(operand: &SetOperand) -> bool {
    match operand {
        SetOperand::Char(_) | SetOperand::Range(..) | SetOperand::Escape(_) => false,
        SetOperand::Nested(class) => match &class.body {
            ClassBody::Set(expression) => !class.negated && may_contain_strings(expression),
            ClassBody::Ranges(_) => false,
        },
        SetOperand::Strings(strings) => strings.iter().any(|string| string.len() != 1),
        SetOperand::Property(property) => is_of_strings(property),
    }
}
