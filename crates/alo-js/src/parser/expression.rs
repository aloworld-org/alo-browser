/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Expressions: everything that has a value.
//!
//! # Precedence is a number, once
//!
//! [`Parser::binary`] climbs precedences rather than having one function per
//! level, which is the usual shape and is a dozen functions that differ by a
//! constant. [`precedence`] is the table, and `**` is the one right-associative
//! operator in the language.
//!
//! Three operators are **not** in it. `&&`, `||` and `??` do not evaluate their
//! right side, and the specification refuses to give `??` a precedence against
//! the other two at all — `a ?? b || c` is not a program, because either
//! reading is one half of every reader gets it wrong. That refusal cannot be
//! expressed as a number, so [`Parser::short_circuit`] is a shape rather than a
//! row in the table, and it is the only reason those three are apart.
//!
//! # An arrow function is decided after the parenthesis it began with
//!
//! `(a, b)` and `(a, b) => c` are the same characters until the `)` has been
//! passed, and the second is not an expression at all — it is a parameter list.
//! [`Parser::arrow_if_it_is_one`] tries the parameter list and puts the cursor
//! back when what follows is not a `=>`, which is the second of the two
//! ambiguities queue item 70 named. See [`super`] for why trying it is not
//! quadratic on a page that nests parentheses.
//!
//! # A pattern that is still an expression
//!
//! `{ a = 1 }` is a destructuring pattern and is not an object literal. Which
//! it is, is decided by whether an `=` follows the whole of it, so it is read,
//! its refusal is **kept** rather than raised, and the refusal is dropped if it
//! becomes a pattern — see [`super::Parser::kept_refusal`]. That is the whole
//! of the cover grammar this parser needs, because the other cover the
//! specification has is the arrow parameter list above, and that is settled by
//! trying it.

use crate::ast::{
    Argument, ArrayElement, Assign, Binary, Body, Element, Expression, ExpressionKind, Function,
    FunctionKind, Key, Logical, Member, Pattern, Property, Template, Unary,
};
use crate::error::{Reason, SyntaxError};
use crate::lexer::Goal;
use crate::punctuator::Punctuator;
use crate::token::{Kind, Token};
use crate::word::Keyword;

use super::property::Owner;
use super::{Home, Inside, Leaving, OPERAND, OPERATOR, Parser};

/// Refuse anything but a name or a member where one is written to.
///
/// `++` and `--` write to what they are given, and there are only two things
/// in the language that can be written to without being taken apart first.
fn must_be_simple(expression: &Expression, at: usize) -> Result<(), SyntaxError> {
    match expression.kind {
        ExpressionKind::Name(_) | ExpressionKind::Member { .. } => Ok(()),
        _ => Err(SyntaxError::new(Reason::NotAnAssignmentTarget, at)),
    }
}

/// How tightly a binary operator binds. Higher wins.
///
/// `??`, `||` and `&&` are absent for the reason at the top of this file.
fn precedence(operator: Binary) -> u8 {
    match operator {
        Binary::BitOr => 1,
        Binary::BitXor => 2,
        Binary::BitAnd => 3,
        Binary::Equal | Binary::NotEqual | Binary::StrictlyEqual | Binary::StrictlyNotEqual => 4,
        Binary::Less
        | Binary::Greater
        | Binary::LessOrEqual
        | Binary::GreaterOrEqual
        | Binary::InstanceOf
        | Binary::In => 5,
        Binary::ShiftLeft | Binary::ShiftRight | Binary::ShiftRightUnsigned => 6,
        Binary::Add | Binary::Subtract => 7,
        Binary::Multiply | Binary::Divide | Binary::Remainder => 8,
        Binary::Power => 9,
    }
}

/// The assignment operator a punctuator spells, if it spells one.
fn assignment_operator(punctuator: Punctuator) -> Option<Assign> {
    Some(match punctuator {
        Punctuator::Assign => Assign::Assign,
        Punctuator::AddAssign => Assign::Add,
        Punctuator::SubtractAssign => Assign::Subtract,
        Punctuator::TimesAssign => Assign::Multiply,
        Punctuator::DivideAssign => Assign::Divide,
        Punctuator::RemainderAssign => Assign::Remainder,
        Punctuator::PowerAssign => Assign::Power,
        Punctuator::ShiftLeftAssign => Assign::ShiftLeft,
        Punctuator::ShiftRightAssign => Assign::ShiftRight,
        Punctuator::ShiftRightUnsignedAssign => Assign::ShiftRightUnsigned,
        Punctuator::BitAndAssign => Assign::BitAnd,
        Punctuator::BitOrAssign => Assign::BitOr,
        Punctuator::BitXorAssign => Assign::BitXor,
        Punctuator::AndAssign => Assign::And,
        Punctuator::OrAssign => Assign::Or,
        Punctuator::CoalesceAssign => Assign::Coalesce,
        _ => return None,
    })
}

impl Parser<'_> {
    /// An expression, including the comma operator.
    ///
    /// This is the production called `Expression`, and it is what a statement,
    /// a parenthesis and a `for` header hold. An argument and an array element
    /// are **not** this — a comma separates them — which is why they call
    /// [`Parser::assignment`] instead.
    pub(super) fn expression(&mut self, allow_in: bool) -> Result<Expression, SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        let first = self.assignment(allow_in)?;
        if !self.at(OPERATOR, Punctuator::Comma)? {
            return Ok(first);
        }
        let mut all = vec![first];
        while self.eat(OPERATOR, Punctuator::Comma)? {
            all.push(self.assignment(allow_in)?);
        }
        Ok(self.expression_at(ExpressionKind::Sequence(all), start))
    }

    /// An expression that is finished being one, so a refusal kept in case it
    /// became a pattern is raised here.
    pub(super) fn value_expression(&mut self, allow_in: bool) -> Result<Expression, SyntaxError> {
        let expression = self.expression(allow_in)?;
        if let Some(refusal) = self.kept_refusal.take() {
            return Err(refusal);
        }
        Ok(expression)
    }

    /// One assignment expression: the whole of the grammar apart from the
    /// comma operator.
    pub(super) fn assignment(&mut self, allow_in: bool) -> Result<Expression, SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        self.deeper(start)?;
        let out = self.assignment_inner(allow_in, start);
        self.shallower();
        out
    }

    fn assignment_inner(
        &mut self,
        allow_in: bool,
        start: usize,
    ) -> Result<Expression, SyntaxError> {
        if self.context.inside.yields() && self.at_keyword(OPERAND, Keyword::Yield)? {
            return self.yield_expression(start, allow_in);
        }
        if let Some(arrow) = self.arrow_if_it_is_one(allow_in)? {
            return Ok(arrow);
        }
        let left = self.conditional(allow_in)?;
        let punctuator = match &self.look(OPERATOR)?.kind {
            Kind::Punctuator(punctuator) => *punctuator,
            _ => return Ok(left),
        };
        let Some(operator) = assignment_operator(punctuator) else {
            return Ok(left);
        };
        let at = self.start_of_next(OPERATOR)?;
        self.bump(OPERATOR)?;
        let target = self.as_pattern(left)?;
        if operator != Assign::Assign && !matches!(target, Pattern::Name(_) | Pattern::Member(_)) {
            return Err(SyntaxError::new(Reason::PatternNeedsAPlainAssignment, at));
        }
        let value = self.assignment(allow_in)?;
        Ok(self.expression_at(
            ExpressionKind::Assignment {
                operator,
                target,
                value: Box::new(value),
            },
            start,
        ))
    }

    /// `yield`, `yield a`, `yield* a`.
    ///
    /// A line ending after `yield` ends it, which is why it is written out
    /// rather than being read as a unary operator: `yield\na` yields nothing
    /// and then evaluates `a`.
    fn yield_expression(
        &mut self,
        start: usize,
        allow_in: bool,
    ) -> Result<Expression, SyntaxError> {
        self.expect_keyword(OPERAND, Keyword::Yield)?;
        let delegate = !self.newline_before(OPERAND)? && self.eat(OPERAND, Punctuator::Times)?;
        let argument = if delegate {
            Some(Box::new(self.assignment(allow_in)?))
        } else if self.newline_before(OPERAND)? || !self.begins_an_expression()? {
            None
        } else {
            Some(Box::new(self.assignment(allow_in)?))
        };
        Ok(self.expression_at(ExpressionKind::Yield { argument, delegate }, start))
    }

    /// Whether the next token could begin an expression at all.
    ///
    /// Asked where an expression is optional — after `yield` and `return` — so
    /// that a `)` or a `;` ends the statement rather than being read as one.
    fn begins_an_expression(&mut self) -> Result<bool, SyntaxError> {
        Ok(match &self.look(OPERAND)?.kind {
            Kind::End => false,
            Kind::Punctuator(p) => matches!(
                p,
                Punctuator::LeftParenthesis
                    | Punctuator::LeftBracket
                    | Punctuator::LeftBrace
                    | Punctuator::Plus
                    | Punctuator::Minus
                    | Punctuator::Not
                    | Punctuator::BitNot
                    | Punctuator::Increment
                    | Punctuator::Decrement
            ),
            _ => true,
        })
    }

    /// `a ? b : c`
    fn conditional(&mut self, allow_in: bool) -> Result<Expression, SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        let test = self.short_circuit(allow_in)?;
        if !self.eat(OPERATOR, Punctuator::Question)? {
            return Ok(test);
        }
        // Both arms are `AssignmentExpression[+In]`: an `in` inside them is
        // never the `in` of a `for` header, because the `?` has already
        // committed the header to being an expression.
        let consequent = self.assignment(true)?;
        self.expect(OPERATOR, Punctuator::Colon)?;
        let alternate = self.assignment(allow_in)?;
        Ok(self.expression_at(
            ExpressionKind::Conditional {
                test: Box::new(test),
                consequent: Box::new(consequent),
                alternate: Box::new(alternate),
            },
            start,
        ))
    }

    /// `a && b`, `a || b` and `a ?? b`, which may not be mixed.
    fn short_circuit(&mut self, allow_in: bool) -> Result<Expression, SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        let (mut left, and_or) = self.or(allow_in)?;
        if !self.at(OPERATOR, Punctuator::Coalesce)? {
            return Ok(left);
        }
        let at = self.start_of_next(OPERATOR)?;
        if and_or {
            return Err(SyntaxError::new(Reason::CoalesceMixedWithAndOr, at));
        }
        while self.eat(OPERATOR, Punctuator::Coalesce)? {
            let at = self.start_of_next(OPERAND)?;
            let (right, right_and_or) = self.or(allow_in)?;
            if right_and_or {
                return Err(SyntaxError::new(Reason::CoalesceMixedWithAndOr, at));
            }
            left = self.expression_at(
                ExpressionKind::Logical {
                    operator: Logical::Coalesce,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                start,
            );
        }
        Ok(left)
    }

    /// `a || b`, answering whether one was written here — which is what the
    /// rule above `??` needs and what a tree cannot say, since `(a || b) ?? c`
    /// and `a || b ?? c` are the same tree and only one is a program.
    fn or(&mut self, allow_in: bool) -> Result<(Expression, bool), SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        let (mut left, mut wrote_one) = self.and(allow_in)?;
        while self.eat(OPERATOR, Punctuator::Or)? {
            wrote_one = true;
            let (right, _) = self.and(allow_in)?;
            left = self.expression_at(
                ExpressionKind::Logical {
                    operator: Logical::Or,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                start,
            );
        }
        Ok((left, wrote_one))
    }

    /// `a && b`, answering the same question as [`Parser::or`].
    fn and(&mut self, allow_in: bool) -> Result<(Expression, bool), SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        let mut left = self.binary(allow_in, 1)?;
        let mut wrote_one = false;
        while self.eat(OPERATOR, Punctuator::And)? {
            wrote_one = true;
            let right = self.binary(allow_in, 1)?;
            left = self.expression_at(
                ExpressionKind::Logical {
                    operator: Logical::And,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                start,
            );
        }
        Ok((left, wrote_one))
    }

    /// The binary operators that evaluate both sides, by precedence.
    fn binary(&mut self, allow_in: bool, least: u8) -> Result<Expression, SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        let (mut left, left_was_unary) = self.unary(allow_in)?;
        let mut first = true;
        while let Some(operator) = self.binary_operator(allow_in)? {
            let binds = precedence(operator);
            if binds < least {
                break;
            }
            let at = self.start_of_next(OPERATOR)?;
            if operator == Binary::Power && first && left_was_unary {
                // `-a ** b` is refused rather than given a reading, because
                // `(-a) ** b` and `-(a ** b)` are different numbers.
                return Err(SyntaxError::new(Reason::PowerAfterAUnary, at));
            }
            first = false;
            self.bump(OPERATOR)?;
            // `**` is the one right-associative operator, so its right side is
            // parsed at its own precedence rather than one above it.
            let next_least = if operator == Binary::Power {
                binds
            } else {
                binds.saturating_add(1)
            };
            let right = self.binary(allow_in, next_least)?;
            left = self.expression_at(
                ExpressionKind::Binary {
                    operator,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                start,
            );
        }
        Ok(left)
    }

    /// The binary operator the next token is, if it is one.
    ///
    /// `in` and `instanceof` are words rather than punctuation, and `in` is not
    /// an operator at all in the first clause of a `for` — which is the whole
    /// of what `allow_in` carries through this file.
    fn binary_operator(&mut self, allow_in: bool) -> Result<Option<Binary>, SyntaxError> {
        let token = self.look(OPERATOR)?;
        Ok(match &token.kind {
            Kind::Punctuator(p) => match p {
                Punctuator::BitOr => Some(Binary::BitOr),
                Punctuator::BitXor => Some(Binary::BitXor),
                Punctuator::BitAnd => Some(Binary::BitAnd),
                Punctuator::Equal => Some(Binary::Equal),
                Punctuator::NotEqual => Some(Binary::NotEqual),
                Punctuator::StrictlyEqual => Some(Binary::StrictlyEqual),
                Punctuator::StrictlyNotEqual => Some(Binary::StrictlyNotEqual),
                Punctuator::Less => Some(Binary::Less),
                Punctuator::Greater => Some(Binary::Greater),
                Punctuator::LessOrEqual => Some(Binary::LessOrEqual),
                Punctuator::GreaterOrEqual => Some(Binary::GreaterOrEqual),
                Punctuator::ShiftLeft => Some(Binary::ShiftLeft),
                Punctuator::ShiftRight => Some(Binary::ShiftRight),
                Punctuator::ShiftRightUnsigned => Some(Binary::ShiftRightUnsigned),
                Punctuator::Plus => Some(Binary::Add),
                Punctuator::Minus => Some(Binary::Subtract),
                Punctuator::Times => Some(Binary::Multiply),
                Punctuator::Divide => Some(Binary::Divide),
                Punctuator::Remainder => Some(Binary::Remainder),
                Punctuator::Power => Some(Binary::Power),
                _ => None,
            },
            Kind::Name(found) if !found.escaped => match found.name.as_str() {
                "instanceof" => Some(Binary::InstanceOf),
                "in" if allow_in => Some(Binary::In),
                _ => None,
            },
            _ => None,
        })
    }

    /// The unary operators, and whether one was written — which `**` needs,
    /// because it refuses an unparenthesised unary on its left.
    fn unary(&mut self, allow_in: bool) -> Result<(Expression, bool), SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        let operator = match &self.look(OPERAND)?.kind {
            Kind::Punctuator(Punctuator::Plus) => Some(Unary::Plus),
            Kind::Punctuator(Punctuator::Minus) => Some(Unary::Minus),
            Kind::Punctuator(Punctuator::BitNot) => Some(Unary::BitNot),
            Kind::Punctuator(Punctuator::Not) => Some(Unary::Not),
            Kind::Name(found) if !found.escaped => match found.name.as_str() {
                "delete" => Some(Unary::Delete),
                "void" => Some(Unary::Void),
                "typeof" => Some(Unary::TypeOf),
                _ => None,
            },
            _ => None,
        };
        if let Some(operator) = operator {
            self.bump(OPERAND)?;
            let (argument, _) = self.unary(allow_in)?;
            return Ok((
                self.expression_at(
                    ExpressionKind::Unary {
                        operator,
                        argument: Box::new(argument),
                    },
                    start,
                ),
                true,
            ));
        }
        if self.context.inside.awaits() && self.at_keyword(OPERAND, Keyword::Await)? {
            self.bump(OPERAND)?;
            let (argument, _) = self.unary(allow_in)?;
            return Ok((
                self.expression_at(ExpressionKind::Await(Box::new(argument)), start),
                true,
            ));
        }
        Ok((self.update(allow_in, start)?, false))
    }

    /// `++a`, `a--`, and whatever they apply to.
    fn update(&mut self, allow_in: bool, start: usize) -> Result<Expression, SyntaxError> {
        for (punctuator, increment) in [
            (Punctuator::Increment, true),
            (Punctuator::Decrement, false),
        ] {
            if self.at(OPERAND, punctuator)? {
                self.bump(OPERAND)?;
                let (argument, _) = self.unary(allow_in)?;
                let at = argument.start;
                must_be_simple(&argument, at)?;
                return Ok(self.expression_at(
                    ExpressionKind::Update {
                        increment,
                        prefix: true,
                        argument: Box::new(argument),
                    },
                    start,
                ));
            }
        }
        let argument = self.left_hand_side(allow_in)?;
        // A line ending before a `++` ends the statement: `a\n++b` is two
        // statements and not one.
        if self.newline_before(OPERATOR)? {
            return Ok(argument);
        }
        for (punctuator, increment) in [
            (Punctuator::Increment, true),
            (Punctuator::Decrement, false),
        ] {
            if self.at(OPERATOR, punctuator)? {
                let at = self.start_of_next(OPERATOR)?;
                self.bump(OPERATOR)?;
                must_be_simple(&argument, at)?;
                return Ok(self.expression_at(
                    ExpressionKind::Update {
                        increment,
                        prefix: false,
                        argument: Box::new(argument),
                    },
                    start,
                ));
            }
        }
        Ok(argument)
    }

    /// Everything that can be called, read from, or constructed.
    fn left_hand_side(&mut self, allow_in: bool) -> Result<Expression, SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        let first = if self.at_keyword(OPERAND, Keyword::New)? {
            self.new_expression(allow_in)?
        } else {
            self.primary(allow_in)?
        };
        let (expression, optional) = self.suffixes(first, start, true, allow_in)?;
        if optional {
            return Ok(self.expression_at(ExpressionKind::Chain(Box::new(expression)), start));
        }
        Ok(expression)
    }

    /// `new a.b(c)`, and `new.target`.
    ///
    /// The callee of a `new` is a member expression and never a call, which is
    /// what makes `new a.b()` construct `a.b` rather than call it: the `(…)`
    /// belongs to the `new`.
    fn new_expression(&mut self, allow_in: bool) -> Result<Expression, SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        self.expect_keyword(OPERAND, Keyword::New)?;
        if self.at(OPERATOR, Punctuator::Dot)? {
            self.bump(OPERATOR)?;
            // `expect_keyword` rather than a name compared with `"target"`,
            // because it is the one that refuses `new.\u0074arget`.
            self.expect_keyword(OPERATOR, Keyword::Target)?;
            if !self.context.inside.is_a_function() {
                return Err(SyntaxError::new(Reason::NewTargetOutsideAFunction, start));
            }
            return Ok(self.expression_at(ExpressionKind::NewTarget, start));
        }
        let inner = if self.at_keyword(OPERAND, Keyword::New)? {
            self.new_expression(allow_in)?
        } else {
            self.primary(allow_in)?
        };
        let (callee, optional) = self.suffixes(inner, start, false, allow_in)?;
        if optional {
            return Err(SyntaxError::new(Reason::OptionalChainInNew, start));
        }
        let arguments = if self.at(OPERATOR, Punctuator::LeftParenthesis)? {
            self.arguments()?
        } else {
            Vec::new()
        };
        Ok(self.expression_at(
            ExpressionKind::New {
                callee: Box::new(callee),
                arguments,
            },
            start,
        ))
    }

    /// The `.a`, `[a]`, `(a)`, `?.a` and `` `a` `` that follow an expression,
    /// answering whether any of them was optional.
    fn suffixes(
        &mut self,
        mut expression: Expression,
        start: usize,
        allow_calls: bool,
        allow_in: bool,
    ) -> Result<(Expression, bool), SyntaxError> {
        let mut optional_anywhere = false;
        loop {
            if self.eat(OPERATOR, Punctuator::Dot)? {
                let member = self.member_name(OPERATOR)?;
                expression = self.expression_at(
                    ExpressionKind::Member {
                        object: Box::new(expression),
                        member,
                        optional: false,
                    },
                    start,
                );
                continue;
            }
            if self.at(OPERATOR, Punctuator::LeftBracket)? {
                self.bump(OPERATOR)?;
                let index = self.expression(true)?;
                self.expect(OPERATOR, Punctuator::RightBracket)?;
                expression = self.expression_at(
                    ExpressionKind::Member {
                        object: Box::new(expression),
                        member: Member::Computed(Box::new(index)),
                        optional: false,
                    },
                    start,
                );
                continue;
            }
            if allow_calls && self.at(OPERATOR, Punctuator::LeftParenthesis)? {
                let arguments = self.arguments()?;
                expression = self.expression_at(
                    ExpressionKind::Call {
                        callee: Box::new(expression),
                        arguments,
                        optional: false,
                    },
                    start,
                );
                continue;
            }
            if self.at(OPERATOR, Punctuator::OptionalChain)? {
                optional_anywhere = true;
                expression = self.optional_link(expression, start, allow_calls)?;
                continue;
            }
            if matches!(self.look(OPERATOR)?.kind, Kind::Template(_)) {
                if optional_anywhere {
                    let at = self.start_of_next(OPERATOR)?;
                    return Err(SyntaxError::new(
                        Reason::TaggedTemplateInAnOptionalChain,
                        at,
                    ));
                }
                let template = self.template(true, allow_in)?;
                expression = self.expression_at(
                    ExpressionKind::TaggedTemplate {
                        tag: Box::new(expression),
                        template,
                    },
                    start,
                );
                continue;
            }
            return Ok((expression, optional_anywhere));
        }
    }

    /// One `?.` link: `a?.b`, `a?.(b)` or `a?.[b]`.
    ///
    /// Its own function because a chain that short-circuits is the thing this
    /// grammar has most nearly like a statement of its own, and because
    /// [`Parser::suffixes`] is already the longest loop in the file.
    fn optional_link(
        &mut self,
        expression: Expression,
        start: usize,
        allow_calls: bool,
    ) -> Result<Expression, SyntaxError> {
        let at = self.start_of_next(OPERATOR)?;
        self.bump(OPERATOR)?;
        if allow_calls && self.at(OPERATOR, Punctuator::LeftParenthesis)? {
            let arguments = self.arguments()?;
            return Ok(self.expression_at(
                ExpressionKind::Call {
                    callee: Box::new(expression),
                    arguments,
                    optional: true,
                },
                start,
            ));
        }
        if self.at(OPERATOR, Punctuator::LeftBracket)? {
            self.bump(OPERATOR)?;
            let index = self.expression(true)?;
            self.expect(OPERATOR, Punctuator::RightBracket)?;
            return Ok(self.expression_at(
                ExpressionKind::Member {
                    object: Box::new(expression),
                    member: Member::Computed(Box::new(index)),
                    optional: true,
                },
                start,
            ));
        }
        if matches!(self.look(OPERATOR)?.kind, Kind::Template(_)) {
            // `` a?.b`c` `` is refused rather than read: a tag is called, and
            // a chain that short-circuits would have to call nothing.
            return Err(SyntaxError::new(
                Reason::TaggedTemplateInAnOptionalChain,
                at,
            ));
        }
        let member = self.member_name(OPERATOR)?;
        Ok(self.expression_at(
            ExpressionKind::Member {
                object: Box::new(expression),
                member,
                optional: true,
            },
            start,
        ))
    }

    /// The name after a `.`, which may be `#private` and may be a keyword —
    /// `a.if` is a property called `if`.
    fn member_name(&mut self, goal: Goal) -> Result<Member, SyntaxError> {
        if let Kind::PrivateName(found) = &self.look(goal)?.kind {
            let name = found.name.clone();
            let at = self.start_of_next(goal)?;
            self.bump(goal)?;
            if !self.context.in_class {
                return Err(SyntaxError::new(Reason::PrivateNameOutsideAClass, at));
            }
            return Ok(Member::Private(name));
        }
        Ok(Member::Name(self.any_name(goal)?))
    }

    /// `(a, ...b)` — the arguments of a call.
    fn arguments(&mut self) -> Result<Vec<Argument>, SyntaxError> {
        self.expect(OPERATOR, Punctuator::LeftParenthesis)?;
        let mut arguments = Vec::new();
        while !self.at(OPERAND, Punctuator::RightParenthesis)? {
            if self.eat(OPERAND, Punctuator::Spread)? {
                arguments.push(Argument::Spread(self.value_assignment(true)?));
            } else {
                arguments.push(Argument::Item(self.value_assignment(true)?));
            }
            if !self.eat(OPERATOR, Punctuator::Comma)? {
                break;
            }
        }
        self.expect(OPERATOR, Punctuator::RightParenthesis)?;
        Ok(arguments)
    }

    /// One assignment expression that is finished being one — see
    /// [`Parser::value_expression`], which this is the comma-free half of.
    pub(super) fn value_assignment(&mut self, allow_in: bool) -> Result<Expression, SyntaxError> {
        let expression = self.assignment(allow_in)?;
        if let Some(refusal) = self.kept_refusal.take() {
            return Err(refusal);
        }
        Ok(expression)
    }

    /// Everything with no operator in it: a literal, a name, a bracket.
    fn primary(&mut self, allow_in: bool) -> Result<Expression, SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        if matches!(self.look(OPERAND)?.kind, Kind::Template(_)) {
            let template = self.template(false, allow_in)?;
            return Ok(self.expression_at(ExpressionKind::Template(template), start));
        }
        if self.at(OPERAND, Punctuator::LeftBracket)? {
            return self.array_literal(start, allow_in);
        }
        if self.at(OPERAND, Punctuator::LeftBrace)? {
            return self.object_literal(start, allow_in);
        }
        if self.at(OPERAND, Punctuator::LeftParenthesis)? {
            self.bump(OPERAND)?;
            let inside = self.expression(true)?;
            self.expect(OPERATOR, Punctuator::RightParenthesis)?;
            // The parentheses are not a node: see `ast`. What they change is
            // what may follow, and that is already decided by the time this
            // returns.
            return Ok(inside);
        }
        if self.at_keyword(OPERAND, Keyword::Function)? {
            let function = self.function_expression(false)?;
            return Ok(self.expression_at(ExpressionKind::Function(Box::new(function)), start));
        }
        if self.at_keyword(OPERAND, Keyword::Class)? {
            let class = self.class(false)?;
            return Ok(self.expression_at(ExpressionKind::Class(Box::new(class)), start));
        }
        if self.async_function_follows()? {
            self.bump(OPERAND)?;
            let function = self.function_expression(true)?;
            return Ok(self.expression_at(ExpressionKind::Function(Box::new(function)), start));
        }
        if self.at_keyword(OPERAND, Keyword::Import)? {
            return self.import_expression(start);
        }
        if self.at_keyword(OPERAND, Keyword::Super)? {
            self.bump(OPERAND)?;
            return self.super_expression(start);
        }
        let token = self.bump(OPERAND)?;
        self.literal_or_name(token, start)
    }

    /// `super.a` and `super(a)`, which are the only two things `super` is.
    fn super_expression(&mut self, start: usize) -> Result<Expression, SyntaxError> {
        let calling = self.at(OPERATOR, Punctuator::LeftParenthesis)?;
        let reading =
            self.at(OPERATOR, Punctuator::Dot)? || self.at(OPERATOR, Punctuator::LeftBracket)?;
        let allowed = if calling {
            self.context.home == Home::ADerivedConstructor
        } else {
            reading && (self.context.home != Home::Nowhere)
        };
        if !allowed {
            return Err(SyntaxError::new(Reason::SuperWhereThereIsNone, start));
        }
        Ok(self.expression_at(ExpressionKind::Super, start))
    }

    /// `import(a)` and `import.meta`.
    fn import_expression(&mut self, start: usize) -> Result<Expression, SyntaxError> {
        self.expect_keyword(OPERAND, Keyword::Import)?;
        if self.eat(OPERATOR, Punctuator::Dot)? {
            self.expect_keyword(OPERATOR, Keyword::Meta)?;
            if !self.is_module() {
                return Err(SyntaxError::new(Reason::ImportMetaInAScript, start));
            }
            return Ok(self.expression_at(ExpressionKind::ImportMeta, start));
        }
        self.expect(OPERATOR, Punctuator::LeftParenthesis)?;
        let specifier = self.value_assignment(true)?;
        let options = if self.eat(OPERATOR, Punctuator::Comma)? {
            if self.at(OPERAND, Punctuator::RightParenthesis)? {
                None
            } else {
                Some(Box::new(self.value_assignment(true)?))
            }
        } else {
            None
        };
        self.eat(OPERATOR, Punctuator::Comma)?;
        self.expect(OPERATOR, Punctuator::RightParenthesis)?;
        Ok(self.expression_at(
            ExpressionKind::ImportCall {
                specifier: Box::new(specifier),
                options,
            },
            start,
        ))
    }

    /// A literal, a keyword that is a value, or a name.
    fn literal_or_name(&mut self, token: Token, start: usize) -> Result<Expression, SyntaxError> {
        let kind = match token.kind {
            Kind::Number(value) => ExpressionKind::Number(value),
            Kind::BigInt { digits, radix } => ExpressionKind::BigInt { digits, radix },
            Kind::String(units) => ExpressionKind::String(units),
            Kind::RegularExpression(literal) => ExpressionKind::RegularExpression(literal),
            Kind::PrivateName(found) => {
                // `#a in b` is the one place a private name is an expression of
                // its own, and the `in` is what makes it one. Anything else is
                // refused here rather than parsed and rejected later.
                if !self.context.in_class {
                    return Err(SyntaxError::new(Reason::PrivateNameOutsideAClass, start));
                }
                if !self.at_keyword(OPERATOR, Keyword::In)? {
                    return Err(SyntaxError::new(Reason::Expected { wanted: "in" }, start));
                }
                ExpressionKind::PrivateName(found.name)
            }
            Kind::Name(_) => {
                let keyword = match &token.kind {
                    Kind::Name(found) if !found.escaped => crate::word::keyword(&found.name),
                    _ => None,
                };
                match keyword {
                    Some(Keyword::This) => ExpressionKind::This,
                    Some(Keyword::Null) => ExpressionKind::Null,
                    Some(Keyword::True) => ExpressionKind::Boolean(true),
                    Some(Keyword::False) => ExpressionKind::Boolean(false),
                    _ => ExpressionKind::Name(self.name_of(&token)?),
                }
            }
            _ => return Err(SyntaxError::new(Reason::NotAnExpression, start)),
        };
        Ok(self.expression_at(kind, start))
    }

    /// `[a, , b, ...c]`
    fn array_literal(&mut self, start: usize, allow_in: bool) -> Result<Expression, SyntaxError> {
        self.expect(OPERAND, Punctuator::LeftBracket)?;
        let mut elements = Vec::new();
        loop {
            if self.at(OPERAND, Punctuator::RightBracket)? {
                break;
            }
            if self.eat(OPERAND, Punctuator::Comma)? {
                elements.push(ArrayElement::Hole);
                continue;
            }
            if self.eat(OPERAND, Punctuator::Spread)? {
                elements.push(ArrayElement::Spread(self.assignment(allow_in)?));
            } else {
                elements.push(ArrayElement::Item(self.assignment(allow_in)?));
            }
            if !self.eat(OPERATOR, Punctuator::Comma)? {
                break;
            }
        }
        self.expect(OPERATOR, Punctuator::RightBracket)?;
        Ok(self.expression_at(ExpressionKind::Array(elements), start))
    }

    /// `{ a, b: c, d() {}, ...e }`
    fn object_literal(&mut self, start: usize, allow_in: bool) -> Result<Expression, SyntaxError> {
        self.expect(OPERAND, Punctuator::LeftBrace)?;
        let mut properties = Vec::new();
        while !self.at(OPERAND, Punctuator::RightBrace)? {
            properties.push(self.object_property(allow_in)?);
            if !self.eat(OPERATOR, Punctuator::Comma)? {
                break;
            }
        }
        self.expect(OPERATOR, Punctuator::RightBrace)?;
        Ok(self.expression_at(ExpressionKind::Object(properties), start))
    }

    /// One property of an object literal.
    fn object_property(&mut self, allow_in: bool) -> Result<Property, SyntaxError> {
        if self.eat(OPERAND, Punctuator::Spread)? {
            return Ok(Property::Spread(self.assignment(allow_in)?));
        }
        if let Some(method) = self.method_if_it_is_one(false, Owner::AnObject)? {
            return Ok(Property::Method(method));
        }
        let at = self.start_of_next(OPERAND)?;
        let key = self.property_key()?;
        if self.eat(OPERATOR, Punctuator::Colon)? {
            let value = self.assignment(allow_in)?;
            return Ok(Property::Named {
                key,
                value,
                shorthand: false,
            });
        }
        // Shorthand: `{ a }`, and `{ a = 1 }`, which is a pattern rather than a
        // literal — see the head of this file for why the refusal is kept.
        let Key::Name(name) = key.clone() else {
            return Err(SyntaxError::new(Reason::Expected { wanted: ":" }, at));
        };
        // A shorthand is a name being used, so the words that may not be one
        // are refused here rather than where the key was read: `{ if }` is not
        // a program, and `{ if: 1 }` is.
        let token = Token {
            kind: Kind::Name(crate::word::Word {
                name: name.clone(),
                escaped: false,
            }),
            start: at,
            end: at,
            newline_before: false,
        };
        self.name_of(&token)?;
        if !self.at(OPERATOR, Punctuator::Assign)? {
            let value = self.expression_at(ExpressionKind::Name(name), at);
            return Ok(Property::Named {
                key,
                value,
                shorthand: true,
            });
        }
        let equals = self.start_of_next(OPERATOR)?;
        self.bump(OPERATOR)?;
        if self.kept_refusal.is_none() {
            self.kept_refusal = Some(SyntaxError::new(Reason::NotAnAssignmentTarget, equals));
        }
        let default = self.assignment(allow_in)?;
        let value = self.expression_at(
            ExpressionKind::Assignment {
                operator: Assign::Assign,
                target: Pattern::Name(name),
                value: Box::new(default),
            },
            at,
        );
        Ok(Property::Named {
            key,
            value,
            shorthand: true,
        })
    }

    /// A template literal, in the pieces the lexer reads it as.
    ///
    /// This is the other half of what [`Goal`] is for: the `}` that ends a
    /// substitution is asked for as [`Goal::TemplateContinuation`], so the text
    /// after it is a template's own and never division.
    fn template(&mut self, tagged: bool, allow_in: bool) -> Result<Template, SyntaxError> {
        use crate::template::Part;
        let mut pieces = Vec::new();
        let mut expressions = Vec::new();
        let start = self.start_of_next(OPERAND)?;
        let token = self.bump(OPERAND)?;
        let Kind::Template(first) = token.kind else {
            return Err(SyntaxError::new(
                Reason::Expected { wanted: "`" },
                token.start,
            ));
        };
        let mut part = first.part;
        pieces.push(first);
        while part == Part::Head || part == Part::Middle {
            expressions.push(self.expression(allow_in)?);
            let token = self.bump(Goal::TemplateContinuation)?;
            let Kind::Template(piece) = token.kind else {
                return Err(SyntaxError::new(
                    Reason::NotATemplateContinuation,
                    token.start,
                ));
            };
            part = piece.part;
            pieces.push(piece);
        }
        if !tagged && pieces.iter().any(|piece| piece.cooked.is_none()) {
            return Err(SyntaxError::new(Reason::UnreadableEscapeInATemplate, start));
        }
        Ok(Template {
            pieces,
            expressions,
        })
    }

    /// Whether `async function` begins here, on one line.
    ///
    /// A line ending between the two words makes `async` a name and
    /// `function` the beginning of a statement, so the two have to be looked
    /// at together.
    pub(super) fn async_function_follows(&mut self) -> Result<bool, SyntaxError> {
        if !self.at_keyword(OPERAND, Keyword::Async)? {
            return Ok(false);
        }
        let mark = self.mark();
        self.bump(OPERAND)?;
        let follows =
            !self.newline_before(OPERAND)? && self.at_keyword(OPERAND, Keyword::Function)?;
        self.back_to(&mark);
        Ok(follows)
    }

    /// An arrow function, if that is what begins here.
    ///
    /// Three shapes, and the first two are decided by one token of lookahead:
    /// `a => b`, `async a => b`, and `(…) => b` — which is the one that has to
    /// be tried, because its parameter list and a parenthesised expression are
    /// the same characters. See [`super`] for the memory that keeps trying it
    /// from being quadratic.
    fn arrow_if_it_is_one(&mut self, allow_in: bool) -> Result<Option<Expression>, SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        if self.at_name(OPERAND)? && !self.at_keyword(OPERAND, Keyword::Async)? {
            return self.arrow_from_one_name(start, false, allow_in);
        }
        if self.at_keyword(OPERAND, Keyword::Async)? {
            let mark = self.mark();
            self.bump(OPERAND)?;
            if !self.newline_before(OPERAND)? {
                if self.at_name(OPERAND)? {
                    if let Some(arrow) = self.arrow_from_one_name(start, true, allow_in)? {
                        return Ok(Some(arrow));
                    }
                } else if self.at(OPERAND, Punctuator::LeftParenthesis)? {
                    if let Some(arrow) = self.arrow_from_a_list(start, true, allow_in)? {
                        return Ok(Some(arrow));
                    }
                }
            }
            self.back_to(&mark);
            // `async` on its own is a name, and `async => a` is an arrow whose
            // parameter is called `async` — which the branch above cannot see,
            // because it consumed the word first.
            return self.arrow_from_one_name(start, false, allow_in);
        }
        if self.at(OPERAND, Punctuator::LeftParenthesis)? {
            return self.arrow_from_a_list(start, false, allow_in);
        }
        Ok(None)
    }

    /// `a => b`: one name, no parentheses.
    fn arrow_from_one_name(
        &mut self,
        start: usize,
        is_async: bool,
        allow_in: bool,
    ) -> Result<Option<Expression>, SyntaxError> {
        let mark = self.mark();
        let Ok(name) = self.binding_name(OPERAND) else {
            self.back_to(&mark);
            return Ok(None);
        };
        if !self.at(OPERATOR, Punctuator::Arrow)? {
            self.back_to(&mark);
            return Ok(None);
        }
        if self.newline_before(OPERATOR)? {
            let at = self.start_of_next(OPERATOR)?;
            return Err(SyntaxError::new(Reason::ArrowOnANewLine, at));
        }
        self.bump(OPERATOR)?;
        let parameters = vec![Element {
            pattern: Pattern::Name(name),
            default: None,
        }];
        let function = self.arrow_body(start, parameters, None, is_async, allow_in)?;
        Ok(Some(self.expression_at(
            ExpressionKind::Function(Box::new(function)),
            start,
        )))
    }

    /// `(a, b) => c`: the shape that has to be tried.
    fn arrow_from_a_list(
        &mut self,
        start: usize,
        is_async: bool,
        allow_in: bool,
    ) -> Result<Option<Expression>, SyntaxError> {
        let parenthesis = self.start_of_next(OPERAND)?;
        if self.not_a_parameter_list.contains(&parenthesis) {
            return Ok(None);
        }
        let mark = self.mark();
        let attempt = self.parameter_list();
        let Ok((parameters, rest)) = attempt else {
            self.back_to(&mark);
            self.not_a_parameter_list.insert(parenthesis);
            return Ok(None);
        };
        if !self.at(OPERATOR, Punctuator::Arrow)? {
            self.back_to(&mark);
            self.not_a_parameter_list.insert(parenthesis);
            return Ok(None);
        }
        if self.newline_before(OPERATOR)? {
            let at = self.start_of_next(OPERATOR)?;
            return Err(SyntaxError::new(Reason::ArrowOnANewLine, at));
        }
        self.bump(OPERATOR)?;
        let function = self.arrow_body(start, parameters, rest, is_async, allow_in)?;
        Ok(Some(self.expression_at(
            ExpressionKind::Function(Box::new(function)),
            start,
        )))
    }

    /// The body of an arrow, which is a block or one expression.
    fn arrow_body(
        &mut self,
        start: usize,
        parameters: Vec<Element>,
        rest: Option<Pattern>,
        is_async: bool,
        allow_in: bool,
    ) -> Result<Function, SyntaxError> {
        let saved = self.context;
        // An arrow is never a generator, and it keeps the `this`, the `super`
        // and the `new.target` of what it was written inside — which is why
        // [`super::Home`] is not touched here.
        self.context.inside = Inside::a_function(is_async, false);
        self.context.leaving = Leaving::default();
        let (body, strict) = if self.at(OPERAND, Punctuator::LeftBrace)? {
            let (statements, strict) = self.function_block()?;
            (Body::Block(statements), strict)
        } else {
            let value = self.value_assignment(allow_in)?;
            (Body::Expression(Box::new(value)), self.context.strict)
        };
        let end = self.last_end;
        self.context = saved;
        Ok(Function {
            name: None,
            parameters,
            rest,
            body,
            kind: FunctionKind::of(is_async, false),
            is_arrow: true,
            strict,
            start,
            end,
        })
    }
}
