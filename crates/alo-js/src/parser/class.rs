/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Classes.
//!
//! # A class body is strict, and it is strict from the `{`
//!
//! Not because somebody wrote a directive — a class body has no prologue —
//! but because the specification says so outright. So [`Parser::class_body`]
//! sets it, and everything read inside is read under it: `class A { m() { let
//! implements = 1 } }` is not a program however sloppy the file around it is.
//!
//! # `constructor` is a name until the class says it is not
//!
//! A `constructor` that is a getter, a setter, a generator or `async` is not
//! the constructor and is not allowed to be called one, and a class may only
//! have one. Both are checked here rather than left to whatever runs the class,
//! because both are refusals about *source text* — which is what a parser is
//! for, and what an interpreter would have to re-derive.
//!
//! # `static` is a name too
//!
//! `static a() {}` is a static method, `static() {}` is a method called
//! `static`, and `static = 1` is a field called `static`. The word is read and
//! then put back if what follows says it was a name, which is the same shape
//! [`super::property`] uses for `get`, `set` and `async`.

use crate::ast::{Class, ClassMember, MethodKind, Statement};
use crate::error::{Reason, SyntaxError};
use crate::punctuator::Punctuator;
use crate::word::Keyword;

use super::property::{Owner, names_the_constructor};
use super::{Home, Inside, Leaving, OPERAND, OPERATOR, Parser};

impl Parser<'_> {
    /// `class A extends B { … }`, as a declaration or as an expression.
    ///
    /// A declaration must have a name; an expression need not.
    pub(super) fn class(&mut self, named: bool) -> Result<Class, SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        self.expect_keyword(OPERAND, Keyword::Class)?;
        let saved = self.context;
        // Everything from here to the closing brace is strict code, including
        // the name and the heritage.
        self.context.strict = true;
        let outcome = self.class_body(start, named);
        self.context = saved;
        outcome
    }

    /// The whole of a class once its strictness is in force.
    fn class_body(&mut self, start: usize, named: bool) -> Result<Class, SyntaxError> {
        let name = if named || self.at_name(OPERAND)? {
            if self.at_keyword(OPERAND, Keyword::Extends)? {
                None
            } else {
                Some(self.binding_name(OPERAND)?)
            }
        } else {
            None
        };
        let heritage = if self.eat_keyword(OPERAND, Keyword::Extends)? {
            Some(Box::new(self.value_assignment(true)?))
        } else {
            None
        };
        self.expect(OPERAND, Punctuator::LeftBrace)?;
        self.context.in_class = true;
        let mut members = Vec::new();
        let mut has_a_constructor = false;
        while !self.at(OPERAND, Punctuator::RightBrace)? {
            if self.eat(OPERAND, Punctuator::Semicolon)? {
                continue;
            }
            let member = self.class_member(heritage.is_some())?;
            if let ClassMember::Method(method) = &member {
                if method.kind == MethodKind::Constructor {
                    if has_a_constructor {
                        return Err(SyntaxError::new(
                            Reason::ConstructorIsNotThat,
                            method.function.start,
                        ));
                    }
                    has_a_constructor = true;
                }
            }
            members.push(member);
        }
        self.expect(OPERATOR, Punctuator::RightBrace)?;
        Ok(Class {
            name,
            heritage,
            members,
            start,
            end: self.last_end,
        })
    }

    /// One member: a method, a field, or a static block.
    fn class_member(&mut self, derived: bool) -> Result<ClassMember, SyntaxError> {
        let mut is_static = false;
        if self.at_keyword(OPERAND, Keyword::Static)? {
            let mark = self.mark();
            self.bump(OPERAND)?;
            if self.at(OPERAND, Punctuator::LeftBrace)? {
                let (body, _) = self.static_block()?;
                return Ok(ClassMember::StaticBlock(body));
            }
            // `static` followed by something that could not be a member's name
            // is a member called `static`.
            if self.at(OPERATOR, Punctuator::Assign)?
                || self.at(OPERATOR, Punctuator::LeftParenthesis)?
                || self.at(OPERATOR, Punctuator::Semicolon)?
                || self.at(OPERATOR, Punctuator::RightBrace)?
                || self.newline_before(OPERATOR)?
            {
                self.back_to(&mark);
            } else {
                is_static = true;
            }
        }
        if let Some(mut method) = self.method_if_it_is_one(is_static, Owner::AClass { derived })? {
            if !is_static && names_the_constructor(&method.key) {
                if method.kind != MethodKind::Method
                    || method.function.kind.is_async()
                    || method.function.kind.is_generator()
                {
                    return Err(SyntaxError::new(
                        Reason::ConstructorIsNotThat,
                        method.function.start,
                    ));
                }
                method.kind = MethodKind::Constructor;
            }
            return Ok(ClassMember::Method(method));
        }
        let key = self.property_key()?;
        let value = if self.eat(OPERATOR, Punctuator::Assign)? {
            Some(self.value_assignment(true)?)
        } else {
            None
        };
        self.semicolon()?;
        Ok(ClassMember::Field {
            key,
            value,
            is_static,
        })
    }

    /// `static { … }` — a block that runs once, when the class is made.
    fn static_block(&mut self) -> Result<(Vec<Statement>, bool), SyntaxError> {
        let saved = self.context;
        self.context.inside = Inside::APlainFunction;
        // A static block is not a method and has no `super()`, and it is the
        // one body in the language that is none of the four kinds a function
        // is — so `await` and `yield` are names in it.
        self.context.home = Home::AMethod;
        self.context.leaving = Leaving::default();
        let outcome = self.function_block();
        self.context = saved;
        outcome
    }
}
