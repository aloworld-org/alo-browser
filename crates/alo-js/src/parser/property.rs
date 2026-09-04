/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a named member is: its key, and the method it may be.
//!
//! An object literal and a class body are different things that share one
//! grammar here — `a() {}`, `get a() {}`, `*a() {}`, `async a() {}` and
//! `[a]() {}` mean the same in both — so the key and the method live in this
//! file and the two callers hold what differs. A class has `static` and
//! `#private` and no `a: b`; an object literal has `a: b` and no `static`.
//!
//! # `get`, `set` and `async` are names until the next token says otherwise
//!
//! `{ get: 1 }` is a property called `get`, `{ get() {} }` is a method called
//! `get`, and `{ get a() {} }` is a getter. Nothing about the word decides it —
//! what follows it does — which is why [`Parser::method_if_it_is_one`] marks
//! its place before reading one and puts the cursor back when the word turns
//! out to have been an ordinary name.

use crate::ast::{Body, Function, FunctionKind, Key, Method, MethodKind};
use crate::error::{Reason, SyntaxError};
use crate::punctuator::Punctuator;
use crate::token::Kind;
use crate::word::Keyword;

use super::{Home, Inside, Leaving, OPERAND, OPERATOR, Parser};

/// Where a method is being written, which decides what `super` means in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Owner {
    /// An object literal, whose methods have a `super.a` and never a `super()`.
    AnObject,
    /// A class body.
    AClass {
        /// Whether the class extends something, which is what decides whether
        /// its constructor may call `super()`.
        derived: bool,
    },
}

/// Whether a key spells `constructor`.
///
/// A string spells it too — `class A { "constructor"() {} }` really is the
/// constructor — and a computed key never does, because what it names is not
/// known until the class is made.
pub(super) fn names_the_constructor(key: &Key) -> bool {
    match key {
        Key::Name(name) => name == "constructor",
        Key::String(units) => String::from_utf16(units).is_ok_and(|text| text == "constructor"),
        _ => false,
    }
}

impl Parser<'_> {
    /// A property's name, however it is written.
    pub(super) fn property_key(&mut self) -> Result<Key, SyntaxError> {
        if self.at(OPERAND, Punctuator::LeftBracket)? {
            self.bump(OPERAND)?;
            let computed = self.value_assignment(true)?;
            self.expect(OPERATOR, Punctuator::RightBracket)?;
            return Ok(Key::Computed(computed));
        }
        let start = self.start_of_next(OPERAND)?;
        let token = self.bump(OPERAND)?;
        match token.kind {
            Kind::Name(found) => Ok(Key::Name(found.name)),
            Kind::PrivateName(found) => Ok(Key::Private(found.name)),
            Kind::String(units) => Ok(Key::String(units)),
            Kind::Number(value) => Ok(Key::Number(value)),
            // A `BigInt` key is its digits read as a name: `1n` and `"1"` are
            // the same property, which is what the specification says and what
            // makes a `BigInt` key need no representation of its own.
            Kind::BigInt { digits, .. } => Ok(Key::Name(digits)),
            _ => Err(SyntaxError::new(
                Reason::Expected {
                    wanted: "a property name",
                },
                start,
            )),
        }
    }

    /// A method, if what begins here is one.
    ///
    /// Answers [`None`] with the cursor where it found it when this is a
    /// property rather than a method, which is what lets the caller go on and
    /// read `a: b`.
    pub(super) fn method_if_it_is_one(
        &mut self,
        is_static: bool,
        home: Owner,
    ) -> Result<Option<Method>, SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        let mark = self.mark();
        let mut is_async = false;
        let mut is_generator = false;
        let mut accessor = None;
        if self.at_keyword(OPERAND, Keyword::Async)? {
            self.bump(OPERAND)?;
            // `async` on its own line is a name, and what follows it is
            // something else entirely.
            if self.newline_before(OPERAND)? || !self.begins_a_key()? {
                self.back_to(&mark);
            } else {
                is_async = true;
            }
        }
        if self.eat(OPERAND, Punctuator::Times)? {
            is_generator = true;
        }
        if !is_async && !is_generator {
            for (keyword, kind) in [
                (Keyword::Get, MethodKind::Get),
                (Keyword::Set, MethodKind::Set),
            ] {
                if self.at_keyword(OPERAND, keyword)? {
                    self.bump(OPERAND)?;
                    if self.begins_a_key()? {
                        accessor = Some(kind);
                    } else {
                        self.back_to(&mark);
                    }
                    break;
                }
            }
        }
        if !is_async && !is_generator && accessor.is_none() && !self.begins_a_key()? {
            self.back_to(&mark);
            return Ok(None);
        }
        let key = self.property_key()?;
        if !self.at(OPERAND, Punctuator::LeftParenthesis)? {
            // `{ get: 1 }`, `{ async }` — the word was a name after all.
            self.back_to(&mark);
            return Ok(None);
        }
        let kind = accessor.unwrap_or(MethodKind::Method);
        // Whether `super()` may be called inside has to be known *before* the
        // body is read, which is why the home comes in as an argument rather
        // than the class deciding afterwards.
        let plain = kind == MethodKind::Method && !is_async && !is_generator && !is_static;
        let constructs =
            plain && home == Owner::AClass { derived: true } && names_the_constructor(&key);
        let function = self.method_body(start, is_async, is_generator, constructs)?;
        Ok(Some(Method {
            key,
            kind,
            function,
            is_static,
        }))
    }

    /// Whether a property name begins here.
    fn begins_a_key(&mut self) -> Result<bool, SyntaxError> {
        Ok(matches!(
            &self.look(OPERAND)?.kind,
            Kind::Name(_)
                | Kind::PrivateName(_)
                | Kind::String(_)
                | Kind::Number(_)
                | Kind::BigInt { .. }
                | Kind::Punctuator(Punctuator::LeftBracket)
        ))
    }

    /// The parameters and body of a method, in a method's context.
    ///
    /// `in_method` is what makes `super.a` mean something, and it is set here
    /// rather than by the class, because an object literal's method has a
    /// `super` too.
    pub(super) fn method_body(
        &mut self,
        start: usize,
        is_async: bool,
        is_generator: bool,
        constructs: bool,
    ) -> Result<Function, SyntaxError> {
        let saved = self.context;
        self.context.inside = Inside::a_function(is_async, is_generator);
        self.context.home = if constructs {
            Home::ADerivedConstructor
        } else {
            Home::AMethod
        };
        self.context.leaving = Leaving::default();
        let outcome = self.method_parameters_and_body(start, is_async, is_generator);
        self.context = saved;
        outcome
    }

    /// The half of [`Parser::method_body`] that runs with the context set.
    fn method_parameters_and_body(
        &mut self,
        start: usize,
        is_async: bool,
        is_generator: bool,
    ) -> Result<Function, SyntaxError> {
        let (parameters, rest) = self.parameter_list()?;
        let (statements, strict) = self.function_block()?;
        Ok(Function {
            name: None,
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
}
