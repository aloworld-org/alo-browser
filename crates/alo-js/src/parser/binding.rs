/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What is written to: a declaration's names, and an assignment's target.
//!
//! # Two ways in, one type out
//!
//! A pattern is read directly where the grammar knows it is one — a parameter
//! list, a `let` — and that is [`Parser::binding_pattern`]. Everywhere else it
//! is read as an **expression** and turned into a pattern once the `=` that
//! follows says it was one: `[a, b] = c` cannot be known to be a pattern until
//! four tokens after it began, so `[a, b]` is read as an array literal and
//! [`Parser::as_pattern`] rewrites it.
//!
//! Both produce [`Pattern`], which is what stops the two paths from drifting.
//! The alternative — a second parser for targets — is the one that ends with
//! `[a.b] = c` working and `[a["b"]] = c` not, because somebody added one case
//! to one of them.
//!
//! # The rewrite refuses rather than repairs
//!
//! `1 = a` and `f() = a` are read happily as expressions and are not programs.
//! [`Parser::as_pattern`] is where that is found, and it answers
//! [`Reason::NotAnAssignmentTarget`] with the offset of the thing that cannot
//! be written to — not of the `=`, because the thing is what somebody has to
//! change.

use crate::ast::{
    ArrayElement, Assign, Element, Expression, ExpressionKind, Key, Pattern, PatternProperty,
    Property,
};
use crate::error::{Reason, SyntaxError};
use crate::punctuator::Punctuator;

use super::{OPERAND, OPERATOR, Parser};

impl Parser<'_> {
    /// A pattern where the grammar already knows there is one: a name, an
    /// array pattern or an object pattern.
    pub(super) fn binding_pattern(&mut self) -> Result<Pattern, SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        self.deeper(start)?;
        let out = self.binding_pattern_inner();
        self.shallower();
        out
    }

    fn binding_pattern_inner(&mut self) -> Result<Pattern, SyntaxError> {
        if self.at(OPERAND, Punctuator::LeftBracket)? {
            return self.array_pattern();
        }
        if self.at(OPERAND, Punctuator::LeftBrace)? {
            return self.object_pattern();
        }
        Ok(Pattern::Name(self.binding_name(OPERAND)?))
    }

    /// A pattern and the value it takes when there is none: `a = 1`.
    pub(super) fn binding_element(&mut self) -> Result<Element, SyntaxError> {
        let pattern = self.binding_pattern()?;
        let default = if self.eat(OPERATOR, Punctuator::Assign)? {
            Some(self.assignment(true)?)
        } else {
            None
        };
        Ok(Element { pattern, default })
    }

    /// `[a, , b, ...c]`
    fn array_pattern(&mut self) -> Result<Pattern, SyntaxError> {
        self.expect(OPERAND, Punctuator::LeftBracket)?;
        let mut elements = Vec::new();
        let mut rest = None;
        loop {
            if self.at(OPERAND, Punctuator::RightBracket)? {
                break;
            }
            if self.eat(OPERAND, Punctuator::Comma)? {
                elements.push(None);
                continue;
            }
            if self.at(OPERAND, Punctuator::Spread)? {
                let at = self.start_of_next(OPERAND)?;
                self.bump(OPERAND)?;
                rest = Some(Box::new(self.binding_pattern()?));
                if self.at(OPERATOR, Punctuator::Assign)? {
                    return Err(SyntaxError::new(Reason::RestCannotHaveADefault, at));
                }
                if self.at(OPERATOR, Punctuator::Comma)? {
                    let at = self.start_of_next(OPERATOR)?;
                    return Err(SyntaxError::new(Reason::RestMustBeLast, at));
                }
                break;
            }
            elements.push(Some(self.binding_element()?));
            if !self.eat(OPERATOR, Punctuator::Comma)? {
                break;
            }
        }
        self.expect(OPERATOR, Punctuator::RightBracket)?;
        Ok(Pattern::Array { elements, rest })
    }

    /// `{ a, b: c, d = 1, ...e }`
    fn object_pattern(&mut self) -> Result<Pattern, SyntaxError> {
        self.expect(OPERAND, Punctuator::LeftBrace)?;
        let mut properties = Vec::new();
        let mut rest = None;
        while !self.at(OPERAND, Punctuator::RightBrace)? {
            if self.at(OPERAND, Punctuator::Spread)? {
                self.bump(OPERAND)?;
                // `{ ...a }` takes what is left of an object, and what is left
                // cannot itself be taken apart — the specification allows only
                // a name here.
                rest = Some(Box::new(Pattern::Name(self.binding_name(OPERAND)?)));
                if self.at(OPERATOR, Punctuator::Comma)? {
                    let at = self.start_of_next(OPERATOR)?;
                    return Err(SyntaxError::new(Reason::RestMustBeLast, at));
                }
                break;
            }
            properties.push(self.pattern_property()?);
            if !self.eat(OPERATOR, Punctuator::Comma)? {
                break;
            }
        }
        self.expect(OPERATOR, Punctuator::RightBrace)?;
        Ok(Pattern::Object { properties, rest })
    }

    /// One property of an object pattern.
    fn pattern_property(&mut self) -> Result<PatternProperty, SyntaxError> {
        let at = self.start_of_next(OPERAND)?;
        let key = self.property_key()?;
        if self.eat(OPERATOR, Punctuator::Colon)? {
            let value = self.binding_element()?;
            return Ok(PatternProperty {
                key,
                value,
                shorthand: false,
            });
        }
        let Key::Name(name) = key.clone() else {
            return Err(SyntaxError::new(Reason::Expected { wanted: ":" }, at));
        };
        let default = if self.eat(OPERATOR, Punctuator::Assign)? {
            Some(self.assignment(true)?)
        } else {
            None
        };
        Ok(PatternProperty {
            key,
            value: Element {
                pattern: Pattern::Name(name),
                default,
            },
            shorthand: true,
        })
    }

    /// An expression read as the thing it is being assigned to.
    ///
    /// The refusal kept for `{ a = 1 }` is dropped here, because reaching this
    /// is what says the thing holding it was a pattern after all — see
    /// [`super::Parser::kept_refusal`].
    pub(super) fn as_pattern(&mut self, expression: Expression) -> Result<Pattern, SyntaxError> {
        let pattern = self.rewrite(expression)?;
        self.kept_refusal = None;
        Ok(pattern)
    }

    /// The rewrite itself, which recurses.
    fn rewrite(&mut self, expression: Expression) -> Result<Pattern, SyntaxError> {
        let at = expression.start;
        match expression.kind {
            ExpressionKind::Name(name) => Ok(Pattern::Name(name)),
            ExpressionKind::Member { .. } => Ok(Pattern::Member(Box::new(expression))),
            ExpressionKind::Array(items) => self.rewrite_array(items),
            ExpressionKind::Object(properties) => self.rewrite_object(properties),
            _ => Err(SyntaxError::new(Reason::NotAnAssignmentTarget, at)),
        }
    }

    /// An array literal read as an array pattern.
    fn rewrite_array(&mut self, items: Vec<ArrayElement>) -> Result<Pattern, SyntaxError> {
        let mut elements = Vec::new();
        let mut rest = None;
        let count = items.len();
        for (index, item) in items.into_iter().enumerate() {
            match item {
                ArrayElement::Hole => elements.push(None),
                ArrayElement::Item(expression) => {
                    elements.push(Some(self.rewrite_element(expression)?));
                }
                ArrayElement::Spread(expression) => {
                    let spread_at = expression.start;
                    if index.saturating_add(1) != count {
                        return Err(SyntaxError::new(Reason::RestMustBeLast, spread_at));
                    }
                    rest = Some(Box::new(self.rewrite(expression)?));
                }
            }
        }
        Ok(Pattern::Array { elements, rest })
    }

    /// An object literal read as an object pattern.
    fn rewrite_object(&mut self, items: Vec<Property>) -> Result<Pattern, SyntaxError> {
        let mut properties = Vec::new();
        let mut rest = None;
        let count = items.len();
        for (index, item) in items.into_iter().enumerate() {
            match item {
                Property::Named {
                    key,
                    value,
                    shorthand,
                } => {
                    properties.push(PatternProperty {
                        key,
                        value: self.rewrite_element(value)?,
                        shorthand,
                    });
                }
                Property::Spread(expression) => {
                    let spread_at = expression.start;
                    if index.saturating_add(1) != count {
                        return Err(SyntaxError::new(Reason::RestMustBeLast, spread_at));
                    }
                    rest = Some(Box::new(self.rewrite(expression)?));
                }
                Property::Method(method) => {
                    return Err(SyntaxError::new(
                        Reason::NotAnAssignmentTarget,
                        method.function.start,
                    ));
                }
            }
        }
        Ok(Pattern::Object { properties, rest })
    }

    /// One element of a pattern, where an `=` in the expression was a default
    /// rather than an assignment.
    fn rewrite_element(&mut self, expression: Expression) -> Result<Element, SyntaxError> {
        let (start, end) = (expression.start, expression.end);
        match expression.kind {
            ExpressionKind::Assignment {
                operator: Assign::Assign,
                target,
                value,
            } => Ok(Element {
                pattern: target,
                default: Some(*value),
            }),
            kind => Ok(Element {
                pattern: self.rewrite(Expression { kind, start, end })?,
                default: None,
            }),
        }
    }
}
