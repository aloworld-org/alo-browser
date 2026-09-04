/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Statements, and the declarations that are only statements in some places.
//!
//! # A body without braces holds a statement and not a declaration
//!
//! `if (a) let b = 1;` is not a program: a declaration there would have no
//! scope to belong to. [`Parser::statement`] reads both and
//! [`Parser::nested_statement`] reads only the first, which is what every
//! single-statement body calls. Annex B allows one spelling of it — a bare
//! `function` in sloppy code — and ADR 0013 § 3 sends Annex B to the legacy
//! tail, so it is refused here by name rather than by accident.
//!
//! # Three words are names until the token after them
//!
//! `let` is a declaration before a name, a `[` or a `{`, and is an ordinary
//! name everywhere else — `let = 1` is a program in sloppy code. `async` is a
//! declaration before `function` **on the same line**. And any name before a
//! `:` is a label rather than an expression. All three mark their place, read
//! ahead, and put the cursor back, because one token of lookahead is not
//! enough for any of them.
//!
//! # `for` is four statements wearing one keyword
//!
//! `for (a; b; c)`, `for (a in b)`, `for (a of b)` and `for await (a of b)`
//! share a prefix that is ambiguous until the token after the first clause. So
//! the clause is read with `in` **not** an operator — which is the whole of
//! what `allow_in` carries through [`super::expression`] — and what follows
//! decides which of the four this was. `for (a in b; ;)` is not a program, and
//! reading `a in b` as an expression is what would make it one.

use crate::ast::{
    Catch, Declaration, DeclarationKind, Declarator, ForInit, ForTarget, Pattern, Statement,
    StatementKind, SwitchCase,
};
use crate::error::{Reason, SyntaxError};
use crate::punctuator::Punctuator;
use crate::token::Kind;
use crate::word::Keyword;

use super::{OPERAND, OPERATOR, Parser};

impl Parser<'_> {
    /// One statement or declaration.
    ///
    /// A statement's expressions are a tree of their own, so the depth counted
    /// by [`Parser::linked`] is put back around each of them: a file of ten
    /// thousand statements is ten thousand shallow trees, not one deep one.
    pub(super) fn statement(&mut self) -> Result<Statement, SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        self.deeper(start)?;
        let out = self.beside(|parser| parser.statement_inner(start, true));
        self.shallower();
        out
    }

    /// One statement where a declaration may not go — a body without braces.
    fn nested_statement(&mut self) -> Result<Statement, SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        self.deeper(start)?;
        let out = self.beside(|parser| parser.statement_inner(start, false));
        self.shallower();
        out
    }

    fn statement_inner(
        &mut self,
        start: usize,
        declarations_allowed: bool,
    ) -> Result<Statement, SyntaxError> {
        if self.at(OPERAND, Punctuator::LeftBrace)? {
            let body = self.block()?;
            return Ok(self.statement_at(StatementKind::Block(body), start));
        }
        if self.eat(OPERAND, Punctuator::Semicolon)? {
            return Ok(self.statement_at(StatementKind::Empty, start));
        }
        if let Some(kind) = self.declaration_kind()? {
            if !declarations_allowed {
                return Err(SyntaxError::new(
                    Reason::DeclarationWhereAStatementIsWanted,
                    start,
                ));
            }
            let declaration = self.declaration(kind, true)?;
            self.semicolon()?;
            return Ok(self.statement_at(StatementKind::Declaration(declaration), start));
        }
        if self.at_keyword(OPERAND, Keyword::Function)? || self.async_function_follows()? {
            if !declarations_allowed {
                return Err(SyntaxError::new(
                    Reason::DeclarationWhereAStatementIsWanted,
                    start,
                ));
            }
            let is_async = self.eat_keyword(OPERAND, Keyword::Async)?;
            let function = self.function_declaration(is_async)?;
            return Ok(self.statement_at(StatementKind::Function(Box::new(function)), start));
        }
        if self.at_keyword(OPERAND, Keyword::Class)? {
            if !declarations_allowed {
                return Err(SyntaxError::new(
                    Reason::DeclarationWhereAStatementIsWanted,
                    start,
                ));
            }
            let class = self.class(true)?;
            return Ok(self.statement_at(StatementKind::Class(Box::new(class)), start));
        }
        if self.at_keyword(OPERAND, Keyword::If)? {
            return self.if_statement(start);
        }
        if self.at_keyword(OPERAND, Keyword::For)? {
            return self.for_statement(start);
        }
        if self.at_keyword(OPERAND, Keyword::While)? {
            return self.while_statement(start);
        }
        if self.at_keyword(OPERAND, Keyword::Do)? {
            return self.do_while_statement(start);
        }
        if self.at_keyword(OPERAND, Keyword::Switch)? {
            return self.switch_statement(start);
        }
        if self.at_keyword(OPERAND, Keyword::Try)? {
            return self.try_statement(start);
        }
        if self.at_keyword(OPERAND, Keyword::Return)? {
            return self.return_statement(start);
        }
        if self.at_keyword(OPERAND, Keyword::Throw)? {
            return self.throw_statement(start);
        }
        if self.at_keyword(OPERAND, Keyword::Break)?
            || self.at_keyword(OPERAND, Keyword::Continue)?
        {
            return self.break_or_continue(start);
        }
        if self.eat_keyword(OPERAND, Keyword::Debugger)? {
            self.semicolon()?;
            return Ok(self.statement_at(StatementKind::Debugger, start));
        }
        if self.at_keyword(OPERAND, Keyword::With)? {
            return Err(SyntaxError::new(Reason::WithIsRefused, start));
        }
        if self.at_keyword(OPERAND, Keyword::Import)? && self.import_declaration_follows()? {
            return self.import_declaration(start);
        }
        if self.at_keyword(OPERAND, Keyword::Export)? {
            return self.export_declaration(start);
        }
        if let Some(labelled) = self.label_if_it_is_one(start)? {
            return Ok(labelled);
        }
        let expression = self.value_expression(true)?;
        self.semicolon()?;
        Ok(self.statement_at(StatementKind::Expression(expression), start))
    }

    /// `{ … }`
    pub(super) fn block(&mut self) -> Result<Vec<Statement>, SyntaxError> {
        self.expect(OPERAND, Punctuator::LeftBrace)?;
        let mut body = Vec::new();
        while !self.at(OPERAND, Punctuator::RightBrace)? {
            if self.at_end(OPERAND)? {
                let at = self.start_of_next(OPERAND)?;
                return Err(SyntaxError::new(Reason::Expected { wanted: "}" }, at));
            }
            body.push(self.statement()?);
        }
        self.expect(OPERATOR, Punctuator::RightBrace)?;
        Ok(body)
    }

    /// Which declaration keyword begins here, if one does.
    ///
    /// `let` is the one that has to be looked past: it is a declaration before
    /// a name, a `[` or a `{`, and a name everywhere else.
    fn declaration_kind(&mut self) -> Result<Option<DeclarationKind>, SyntaxError> {
        if self.at_keyword(OPERAND, Keyword::Var)? {
            return Ok(Some(DeclarationKind::Var));
        }
        if self.at_keyword(OPERAND, Keyword::Const)? {
            return Ok(Some(DeclarationKind::Const));
        }
        if !self.at_keyword(OPERAND, Keyword::Let)? {
            return Ok(None);
        }
        let mark = self.mark();
        self.bump(OPERAND)?;
        let declares = self.at_name(OPERAND)?
            || self.at(OPERAND, Punctuator::LeftBracket)?
            || self.at(OPERAND, Punctuator::LeftBrace)?;
        self.back_to(&mark);
        Ok(declares.then_some(DeclarationKind::Let))
    }

    /// `var a = 1, b`, without the `;`.
    ///
    /// The keyword has been looked at and not consumed; this consumes it.
    /// `allow_in` is false in a `for` header, which is also where a declarator
    /// may have no value — `for (const a of b)` gives it one per pass — so the
    /// one flag carries both, and [`Parser::every_value_is_given`] is what
    /// puts the requirement back when the header turns out to be a plain
    /// `for`.
    pub(super) fn declaration(
        &mut self,
        kind: DeclarationKind,
        allow_in: bool,
    ) -> Result<Declaration, SyntaxError> {
        self.bump(OPERAND)?;
        let mut declarators = Vec::new();
        loop {
            // One declarator is a tree beside the next rather than below it,
            // which is what keeps `var a = …, b = …, …` from adding up.
            declarators.push(self.beside(|parser| parser.declarator(allow_in))?);
            if !self.eat(OPERATOR, Punctuator::Comma)? {
                break;
            }
        }
        let declaration = Declaration { kind, declarators };
        if allow_in {
            let at = self.last_end;
            Self::every_value_is_given(&declaration, at)?;
        }
        Ok(declaration)
    }

    /// One name — or pattern — a declaration declares.
    fn declarator(&mut self, allow_in: bool) -> Result<Declarator, SyntaxError> {
        let pattern = self.binding_pattern()?;
        let init = if self.eat(OPERATOR, Punctuator::Assign)? {
            Some(self.value_assignment(allow_in)?)
        } else {
            None
        };
        Ok(Declarator { pattern, init })
    }

    /// Refuse a declaration that binds something to nothing.
    ///
    /// `const a` has no value it could ever be given, and `let [a]` takes apart
    /// a value that is not there. Only a plain `let` or `var` name may stand
    /// alone, and only outside a `for…of` header.
    fn every_value_is_given(declaration: &Declaration, at: usize) -> Result<(), SyntaxError> {
        for declarator in &declaration.declarators {
            if declarator.init.is_some() {
                continue;
            }
            if declaration.kind == DeclarationKind::Const
                || !matches!(declarator.pattern, Pattern::Name(_))
            {
                return Err(SyntaxError::new(Reason::ConstWithoutAValue, at));
            }
        }
        Ok(())
    }

    /// `if (a) b; else c;`
    fn if_statement(&mut self, start: usize) -> Result<Statement, SyntaxError> {
        self.expect_keyword(OPERAND, Keyword::If)?;
        self.expect(OPERAND, Punctuator::LeftParenthesis)?;
        let test = self.value_expression(true)?;
        self.expect(OPERATOR, Punctuator::RightParenthesis)?;
        let consequent = Box::new(self.nested_statement()?);
        let alternate = if self.eat_keyword(OPERAND, Keyword::Else)? {
            Some(Box::new(self.nested_statement()?))
        } else {
            None
        };
        Ok(self.statement_at(
            StatementKind::If {
                test,
                consequent,
                alternate,
            },
            start,
        ))
    }

    /// `while (a) b;`
    fn while_statement(&mut self, start: usize) -> Result<Statement, SyntaxError> {
        self.expect_keyword(OPERAND, Keyword::While)?;
        self.expect(OPERAND, Punctuator::LeftParenthesis)?;
        let test = self.value_expression(true)?;
        self.expect(OPERATOR, Punctuator::RightParenthesis)?;
        let body = Box::new(self.loop_body()?);
        Ok(self.statement_at(StatementKind::While { test, body }, start))
    }

    /// `do a; while (b)`
    ///
    /// The one statement whose `;` may be left out even without a line ending:
    /// `do a; while (b) c` is two statements, which the specification writes as
    /// a rule of its own rather than as ordinary insertion.
    fn do_while_statement(&mut self, start: usize) -> Result<Statement, SyntaxError> {
        self.expect_keyword(OPERAND, Keyword::Do)?;
        let body = Box::new(self.loop_body()?);
        self.expect_keyword(OPERAND, Keyword::While)?;
        self.expect(OPERAND, Punctuator::LeftParenthesis)?;
        let test = self.value_expression(true)?;
        self.expect(OPERATOR, Punctuator::RightParenthesis)?;
        self.eat(OPERATOR, Punctuator::Semicolon)?;
        Ok(self.statement_at(StatementKind::DoWhile { body, test }, start))
    }

    /// The body of a loop, where `break` and `continue` mean something.
    fn loop_body(&mut self) -> Result<Statement, SyntaxError> {
        let saved = self.context;
        self.context.leaving.a_loop = true;
        let out = self.nested_statement();
        self.context = saved;
        out
    }

    /// The four statements that begin `for`.
    fn for_statement(&mut self, start: usize) -> Result<Statement, SyntaxError> {
        self.expect_keyword(OPERAND, Keyword::For)?;
        let is_await = self.context.inside.awaits() && self.eat_keyword(OPERAND, Keyword::Await)?;
        self.expect(OPERAND, Punctuator::LeftParenthesis)?;
        if self.at(OPERAND, Punctuator::Semicolon)? {
            self.bump(OPERAND)?;
            return self.rest_of_a_plain_for(start, None);
        }
        if let Some(kind) = self.declaration_kind()? {
            let declaration = self.declaration(kind, false)?;
            if let Some(statement) = self.for_in_or_of(start, is_await, || {
                ForTarget::Declaration(declaration.clone())
            })? {
                return Ok(statement);
            }
            self.expect(OPERATOR, Punctuator::Semicolon)?;
            Self::every_value_is_given(&declaration, start)?;
            return self.rest_of_a_plain_for(start, Some(ForInit::Declaration(declaration)));
        }
        let expression = self.expression(false)?;
        let target_at = expression.start;
        if self.at_keyword(OPERATOR, Keyword::In)? || self.at_keyword(OPERATOR, Keyword::Of)? {
            let pattern = self.as_pattern(expression)?;
            let statement =
                self.for_in_or_of(start, is_await, || ForTarget::Target(pattern.clone()))?;
            return statement
                .ok_or_else(|| SyntaxError::new(Reason::Expected { wanted: "of" }, target_at));
        }
        if let Some(refusal) = self.kept_refusal.take() {
            return Err(refusal);
        }
        self.expect(OPERATOR, Punctuator::Semicolon)?;
        self.rest_of_a_plain_for(start, Some(ForInit::Expression(expression)))
    }

    /// The `in b)` or `of b)` of a `for`, once its first clause is read.
    fn for_in_or_of(
        &mut self,
        start: usize,
        is_await: bool,
        left: impl Fn() -> ForTarget,
    ) -> Result<Option<Statement>, SyntaxError> {
        if self.eat_keyword(OPERATOR, Keyword::In)? {
            let right = self.value_expression(true)?;
            self.expect(OPERATOR, Punctuator::RightParenthesis)?;
            let body = Box::new(self.loop_body()?);
            return Ok(Some(self.statement_at(
                StatementKind::ForIn {
                    left: left(),
                    right,
                    body,
                },
                start,
            )));
        }
        if self.eat_keyword(OPERATOR, Keyword::Of)? {
            // `of` takes one value rather than a comma expression: `for (a of
            // b, c)` is not a program, and reading `Expression` here would make
            // it one.
            let right = self.value_assignment(true)?;
            self.expect(OPERATOR, Punctuator::RightParenthesis)?;
            let body = Box::new(self.loop_body()?);
            return Ok(Some(self.statement_at(
                StatementKind::ForOf {
                    left: left(),
                    right,
                    is_await,
                    body,
                },
                start,
            )));
        }
        Ok(None)
    }

    /// `; b; c) d` — everything after a plain `for`'s first semicolon.
    fn rest_of_a_plain_for(
        &mut self,
        start: usize,
        init: Option<ForInit>,
    ) -> Result<Statement, SyntaxError> {
        let test = if self.at(OPERAND, Punctuator::Semicolon)? {
            None
        } else {
            Some(self.value_expression(true)?)
        };
        self.expect(OPERATOR, Punctuator::Semicolon)?;
        let update = if self.at(OPERAND, Punctuator::RightParenthesis)? {
            None
        } else {
            Some(self.value_expression(true)?)
        };
        self.expect(OPERATOR, Punctuator::RightParenthesis)?;
        let body = Box::new(self.loop_body()?);
        Ok(self.statement_at(
            StatementKind::For {
                init,
                test,
                update,
                body,
            },
            start,
        ))
    }

    /// `switch (a) { case b: … default: … }`
    fn switch_statement(&mut self, start: usize) -> Result<Statement, SyntaxError> {
        self.expect_keyword(OPERAND, Keyword::Switch)?;
        self.expect(OPERAND, Punctuator::LeftParenthesis)?;
        let discriminant = self.value_expression(true)?;
        self.expect(OPERATOR, Punctuator::RightParenthesis)?;
        self.expect(OPERAND, Punctuator::LeftBrace)?;
        let saved = self.context;
        self.context.leaving.a_switch = true;
        let cases = self.switch_cases();
        self.context = saved;
        let cases = cases?;
        self.expect(OPERATOR, Punctuator::RightBrace)?;
        Ok(self.statement_at(
            StatementKind::Switch {
                discriminant,
                cases,
            },
            start,
        ))
    }

    /// The cases of a `switch`, in the order they were written.
    fn switch_cases(&mut self) -> Result<Vec<SwitchCase>, SyntaxError> {
        let mut cases = Vec::new();
        while !self.at(OPERAND, Punctuator::RightBrace)? {
            if self.at_end(OPERAND)? {
                let at = self.start_of_next(OPERAND)?;
                return Err(SyntaxError::new(Reason::Expected { wanted: "}" }, at));
            }
            let test = if self.eat_keyword(OPERAND, Keyword::Default)? {
                None
            } else {
                self.expect_keyword(OPERAND, Keyword::Case)?;
                Some(self.value_expression(true)?)
            };
            self.expect(OPERATOR, Punctuator::Colon)?;
            let mut body = Vec::new();
            while !self.at(OPERAND, Punctuator::RightBrace)?
                && !self.at_keyword(OPERAND, Keyword::Case)?
                && !self.at_keyword(OPERAND, Keyword::Default)?
            {
                if self.at_end(OPERAND)? {
                    let at = self.start_of_next(OPERAND)?;
                    return Err(SyntaxError::new(Reason::Expected { wanted: "}" }, at));
                }
                body.push(self.statement()?);
            }
            cases.push(SwitchCase { test, body });
        }
        Ok(cases)
    }

    /// `try { … } catch (a) { … } finally { … }`
    fn try_statement(&mut self, start: usize) -> Result<Statement, SyntaxError> {
        self.expect_keyword(OPERAND, Keyword::Try)?;
        let block = self.block()?;
        let handler = if self.eat_keyword(OPERAND, Keyword::Catch)? {
            // `catch { }` without a binding is ordinary modern code: a handler
            // that does not look at what was thrown says so by leaving the
            // parameter out.
            let parameter = if self.eat(OPERAND, Punctuator::LeftParenthesis)? {
                let pattern = self.binding_pattern()?;
                self.expect(OPERATOR, Punctuator::RightParenthesis)?;
                Some(pattern)
            } else {
                None
            };
            Some(Catch {
                parameter,
                body: self.block()?,
            })
        } else {
            None
        };
        let finalizer = if self.eat_keyword(OPERAND, Keyword::Finally)? {
            Some(self.block()?)
        } else {
            None
        };
        if handler.is_none() && finalizer.is_none() {
            return Err(SyntaxError::new(
                Reason::Expected { wanted: "catch" },
                start,
            ));
        }
        Ok(self.statement_at(
            StatementKind::Try {
                block,
                handler,
                finalizer,
            },
            start,
        ))
    }

    /// `return`, and the line ending that ends it.
    fn return_statement(&mut self, start: usize) -> Result<Statement, SyntaxError> {
        self.expect_keyword(OPERAND, Keyword::Return)?;
        if !self.context.inside.is_a_function() {
            return Err(SyntaxError::new(Reason::ReturnOutsideAFunction, start));
        }
        let value = if self.newline_before(OPERAND)?
            || self.at(OPERAND, Punctuator::Semicolon)?
            || self.at(OPERAND, Punctuator::RightBrace)?
            || self.at_end(OPERAND)?
        {
            None
        } else {
            Some(self.value_expression(true)?)
        };
        self.semicolon()?;
        Ok(self.statement_at(StatementKind::Return(value), start))
    }

    /// `throw a`, which unlike `return` has nothing to throw when a line ends.
    fn throw_statement(&mut self, start: usize) -> Result<Statement, SyntaxError> {
        self.expect_keyword(OPERAND, Keyword::Throw)?;
        if self.newline_before(OPERAND)? {
            let at = self.start_of_next(OPERAND)?;
            return Err(SyntaxError::new(
                Reason::Expected {
                    wanted: "something to throw, on this line",
                },
                at,
            ));
        }
        let value = self.value_expression(true)?;
        self.semicolon()?;
        Ok(self.statement_at(StatementKind::Throw(value), start))
    }

    /// `break`, `continue`, and the label either may carry.
    fn break_or_continue(&mut self, start: usize) -> Result<Statement, SyntaxError> {
        let leaving = self.eat_keyword(OPERAND, Keyword::Break)?;
        if !leaving {
            self.expect_keyword(OPERAND, Keyword::Continue)?;
        }
        // A line ending after either ends it, so `break\nouter` leaves the
        // innermost loop and then evaluates `outer`.
        let label = if self.newline_before(OPERATOR)? || !self.at_name(OPERATOR)? {
            None
        } else {
            Some(self.any_name(OPERATOR)?)
        };
        let allowed = if leaving {
            self.context.can_break()
        } else {
            self.context.leaving.a_loop
        };
        // A label reaches out of the innermost loop, so it is the one case
        // this cannot decide without the scope item 205 will bring — and
        // refusing it here would refuse programs that are fine.
        if !allowed && label.is_none() {
            return Err(SyntaxError::new(Reason::NothingToLeave, start));
        }
        self.semicolon()?;
        let kind = if leaving {
            StatementKind::Break(label)
        } else {
            StatementKind::Continue(label)
        };
        Ok(self.statement_at(kind, start))
    }

    /// `outer: for (…) …`, if a label is what begins here.
    fn label_if_it_is_one(&mut self, start: usize) -> Result<Option<Statement>, SyntaxError> {
        if !self.at_name(OPERAND)? {
            return Ok(None);
        }
        let mark = self.mark();
        let Ok(label) = self.binding_name(OPERAND) else {
            self.back_to(&mark);
            return Ok(None);
        };
        if !self.at(OPERATOR, Punctuator::Colon)? {
            self.back_to(&mark);
            return Ok(None);
        }
        self.bump(OPERATOR)?;
        // A labelled function declaration is Annex B in sloppy code and an
        // early error in strict code; ADR 0013 § 3 refuses the appendix, so
        // what is left is the error.
        let body = Box::new(self.nested_statement()?);
        Ok(Some(self.statement_at(
            StatementKind::Labelled { label, body },
            start,
        )))
    }

    /// Whether an `import` here begins a declaration rather than `import(…)`
    /// or `import.meta`, which are expressions.
    fn import_declaration_follows(&mut self) -> Result<bool, SyntaxError> {
        let mark = self.mark();
        self.bump(OPERAND)?;
        let declaration = !self.at(OPERATOR, Punctuator::LeftParenthesis)?
            && !self.at(OPERATOR, Punctuator::Dot)?;
        self.back_to(&mark);
        Ok(declaration)
    }

    /// The string a module declaration names, as code units.
    pub(super) fn module_specifier(&mut self) -> Result<Vec<u16>, SyntaxError> {
        let at = self.start_of_next(OPERAND)?;
        let token = self.bump(OPERAND)?;
        match token.kind {
            Kind::String(units) => Ok(units),
            _ => Err(SyntaxError::new(
                Reason::Expected {
                    wanted: "a module specifier",
                },
                at,
            )),
        }
    }
}
