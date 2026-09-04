/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Functions: their parameters, and their bodies.
//!
//! # A parameter list is where a speculative parse is paid for
//!
//! [`Parser::parameter_list`] is called twice for every `(` that turns out to
//! open an arrow function's parameters and once for every one that does not —
//! see [`super`] for the memory that keeps the second case from being paid for
//! twice. It must therefore **fail rather than refuse**: an error out of here
//! is often not an error at all but a parenthesised expression being read the
//! wrong way, and the caller puts the cursor back and reads it the other way.
//!
//! # A body decides its own strictness, and its own reserved words
//!
//! `"use strict"` at the top of a body makes the body strict, which changes
//! which words are names inside it. [`Parser::function_block`] reads the
//! prologue, so the change happens before the statements it governs are
//! parsed — and the context is put back on the way out, because a strict
//! function inside a sloppy script does not make the rest of the script
//! strict.
//!
//! What this does **not** yet do is check a parameter list against a
//! strictness declared after it: `function f(arg) { "use strict" }` is refused
//! by the specification when `arg` is a word strict code reserves, and here the
//! parameters were already read under the old strictness. It is written down in
//! the queue rather than left to be discovered, with the rest of the early
//! errors that need a scope.

use crate::ast::{Body, Element, Function, FunctionKind, Pattern, Statement};
use crate::error::{Reason, SyntaxError};
use crate::punctuator::Punctuator;
use crate::word::Keyword;

use super::{Home, Inside, Leaving, OPERAND, OPERATOR, Parser};

impl Parser<'_> {
    /// `function a(b) { c }`, as a declaration.
    ///
    /// The `function` has not been consumed yet; `is_async` says whether an
    /// `async` before it has been.
    pub(super) fn function_declaration(&mut self, is_async: bool) -> Result<Function, SyntaxError> {
        self.function(is_async, true)
    }

    /// `function (a) { b }`, as an expression — where the name is optional.
    pub(super) fn function_expression(&mut self, is_async: bool) -> Result<Function, SyntaxError> {
        self.function(is_async, false)
    }

    /// The whole of a function, from its `function` to its `}`.
    fn function(&mut self, is_async: bool, named: bool) -> Result<Function, SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        self.expect_keyword(OPERAND, Keyword::Function)?;
        let is_generator = self.eat(OPERAND, Punctuator::Times)?;
        // The name of a function is read in the context *outside* it for
        // `await` and `yield`, and the context inside it for everything else —
        // a distinction nothing here can act on until item 205's scopes, and
        // one that changes no program a person writes.
        let name = if named || self.at_name(OPERAND)? {
            Some(self.binding_name(OPERAND)?)
        } else {
            None
        };
        let saved = self.context;
        self.context.inside = Inside::a_function(is_async, is_generator);
        // A function is not a method however it was written down: `super` in
        // one is not the `super` of the object literal it sits in.
        self.context.home = Home::Nowhere;
        self.context.leaving = Leaving::default();
        let outcome = self.parameters_and_body(start, name, is_async, is_generator);
        self.context = saved;
        outcome
    }

    /// The parameters and body of a function, once its context is set.
    fn parameters_and_body(
        &mut self,
        start: usize,
        name: Option<String>,
        is_async: bool,
        is_generator: bool,
    ) -> Result<Function, SyntaxError> {
        let (parameters, rest) = self.parameter_list()?;
        let (statements, strict) = self.function_block()?;
        Ok(Function {
            name,
            parameters,
            rest,
            body: Body::Block(statements),
            kind: FunctionKind::of(is_async, is_generator),
            is_arrow: false,
            strict,
            start,
            end: self.last_end,
        })
    }

    /// `(a, b = 1, ...c)`.
    pub(super) fn parameter_list(
        &mut self,
    ) -> Result<(Vec<Element>, Option<Pattern>), SyntaxError> {
        self.expect(OPERAND, Punctuator::LeftParenthesis)?;
        let mut parameters = Vec::new();
        let mut rest = None;
        while !self.at(OPERAND, Punctuator::RightParenthesis)? {
            if self.at(OPERAND, Punctuator::Spread)? {
                let at = self.start_of_next(OPERAND)?;
                self.bump(OPERAND)?;
                let pattern = self.binding_pattern()?;
                if self.at(OPERATOR, Punctuator::Assign)? {
                    return Err(SyntaxError::new(Reason::RestCannotHaveADefault, at));
                }
                rest = Some(pattern);
                // `...a` takes what is left, so a comma after it is a
                // parameter that could never be given a value.
                if self.at(OPERATOR, Punctuator::Comma)? {
                    let at = self.start_of_next(OPERATOR)?;
                    return Err(SyntaxError::new(Reason::RestMustBeLast, at));
                }
                break;
            }
            parameters.push(self.binding_element()?);
            if !self.eat(OPERATOR, Punctuator::Comma)? {
                break;
            }
        }
        self.expect(OPERATOR, Punctuator::RightParenthesis)?;
        Ok((parameters, rest))
    }

    /// `{ … }` as a function's body, with the strictness its prologue asks for.
    pub(super) fn function_block(&mut self) -> Result<(Vec<Statement>, bool), SyntaxError> {
        self.expect(OPERAND, Punctuator::LeftBrace)?;
        let saved = self.context.strict;
        let (body, strict) = self.directives_then_statements(Some(Punctuator::RightBrace))?;
        self.expect(OPERATOR, Punctuator::RightBrace)?;
        self.context.strict = saved;
        Ok((body, strict))
    }
}
