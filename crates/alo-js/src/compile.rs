/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A tree into instructions.
//!
//! The compiler walks the tree the parser built and emits the [`Chunk`] the
//! interpreter runs. It decides three things the interpreter therefore does not
//! have to: **which slot a name is**, **where a jump goes**, and **what a
//! program's completion value is**.
//!
//! # It runs on a stack of its own, for the parser's reason
//!
//! A compiler recurses over the tree once per level, and the tree is as deep as
//! [`bounds::DEEPEST_NESTING`] plus [`bounds::DEEPEST_EXPRESSION`] — four
//! thousand levels, which is a bound chosen against a *walker's* frame and not
//! against a compiler's. So this does what [`Parser::program`] does and for the
//! identical reason (*a limit somebody else chooses is not a limit*): it runs on
//! a scoped thread with [`bounds::STACK_FOR_A_COMPILE`], and the depth it can
//! take is then the same in a debug build, a release build and a renderer.
//!
//! [`Parser::program`]: crate::parser::Parser::program
//!
//! # What it refuses, and why that is not a stub
//!
//! ADR 0013 § 3: *absent beats approximate*. Half of this language is not built
//! yet — every one of them is a queue item — and a compiler that emitted
//! something plausible for a call, a `try` or an array literal would produce a
//! program that runs and is wrong. So each is a [`Refusal`] that **names the
//! item that builds it**, in one list a person can read, and nothing downstream
//! has to wonder whether an instruction means what it says.
//!
//! The second kind of refusal is different and is not about this engine being
//! unfinished: [`Refusal::NotAProgram`] is an early error a *scope* is needed to
//! see, like `let a; let a;`. Queue item 205 owns those in general; the two here
//! are the ones the compiler cannot be correct without, and they are named in
//! that item.

pub mod hoist;
pub mod scope;

use std::fmt;

use crate::ast::{
    Assign, Declaration, DeclarationKind, Expression, ExpressionKind, ForInit, Key, Member,
    Pattern, Program, Property, Statement, StatementKind, Template, Unary,
};
use crate::bounds;
use crate::code::{Chunk, Op};
use crate::operate::Simple;

use scope::{Blocks, Where};

/// Something the language has that this engine has not built.
///
/// Each names the queue item that builds it, because *what to do about it* is
/// the useful half of the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum What {
    /// A function, an arrow, a class or a method — anything with a body of its
    /// own (queue item 209).
    AFunction,
    /// A call, a `new`, a `super`, a tagged template: anything that would run a
    /// function (queue item 209).
    ACall,
    /// `this`, which needs something to have been called (queue item 209).
    This,
    /// `try`, `catch` and `finally` (queue item 210).
    ACatch,
    /// An array literal, a spread, a destructuring pattern, `for…in` or
    /// `for…of` (queue item 211).
    TakingAValueApart,
    /// A regular expression literal (queue item 74).
    ARegularExpression,
    /// A `BigInt` literal (queue item 207).
    ABigInt,
    /// `import`, `export`, `import.meta` and `import()` (queue item 77).
    AModule,
    /// `yield` and `await` (queue item 75).
    ASuspension,
}

impl What {
    /// The queue item that builds it.
    pub const fn item(self) -> u16 {
        match self {
            What::AFunction | What::ACall | What::This => 209,
            What::ACatch => 210,
            What::TakingAValueApart => 211,
            What::ARegularExpression => 74,
            What::ABigInt => 207,
            What::AModule => 77,
            What::ASuspension => 75,
        }
    }

    /// What it is, in a person's words.
    const fn describe(self) -> &'static str {
        match self {
            What::AFunction => "a function, an arrow, a class or a method",
            What::ACall => "calling something",
            What::This => "`this`",
            What::ACatch => "`try`, `catch` and `finally`",
            What::TakingAValueApart => {
                "an array literal, a spread, a destructuring pattern, `for…in` or `for…of`"
            }
            What::ARegularExpression => "a regular expression literal",
            What::ABigInt => "a `BigInt` literal",
            What::AModule => "`import` and `export`",
            What::ASuspension => "`yield` and `await`",
        }
    }
}

/// Why a program did not compile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The language has this and this engine has not built it yet.
    NotBuiltYet {
        /// What it is.
        what: What,
        /// Where in the source.
        at: usize,
    },
    /// The program is not a program: an early error that needs a scope to see.
    NotAProgram {
        /// What is wrong, in words.
        why: String,
        /// Where in the source.
        at: usize,
    },
}

impl Refusal {
    /// Where in the source it is.
    pub const fn at(&self) -> usize {
        match self {
            Refusal::NotBuiltYet { at, .. } | Refusal::NotAProgram { at, .. } => *at,
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::NotBuiltYet { what, at } => write!(
                out,
                "{} is not built yet — queue item {} builds it (at byte {at})",
                what.describe(),
                what.item()
            ),
            Refusal::NotAProgram { why, at } => write!(out, "{why} (at byte {at})"),
        }
    }
}

/// Compile a program, on a stack of the engine's own.
///
/// # Errors
///
/// [`Refusal`], naming what it could not compile and where.
pub fn compile(program: &Program) -> Result<Chunk, Refusal> {
    std::thread::scope(|scope| {
        let started = std::thread::Builder::new()
            .name("alo-js compile".to_owned())
            .stack_size(bounds::STACK_FOR_A_COMPILE)
            .spawn_scoped(scope, || here(program));
        match started {
            Ok(work) => match work.join() {
                Ok(chunk) => chunk,
                // A panic is a bug in this crate, and a bug reported as a
                // refusal is a bug nobody finds — the parser's argument,
                // unchanged.
                Err(panic) => std::panic::resume_unwind(panic),
            },
            Err(_) => Err(Refusal::NotAProgram {
                why: "this engine could not take a stack to compile on".to_owned(),
                at: 0,
            }),
        }
    })
}

/// The compile itself, once it is on the stack it asked for.
fn here(program: &Program) -> Result<Chunk, Refusal> {
    let mut compiler = Compiler {
        chunk: Chunk::new(program.strict),
        blocks: Blocks::new(),
        enclosing: Vec::new(),
        chains: Vec::new(),
    };
    compiler.program(program)?;
    Ok(compiler.chunk)
}

/// Something a `break` or a `continue` may leave.
#[derive(Debug)]
struct Enclosing {
    /// The label written in front of it, if there was one.
    label: Option<String>,
    /// Whether a `continue` may name it: a `switch` and a labelled block may be
    /// broken out of and never continued.
    repeats: bool,
    /// The jumps out of it, waiting for its end.
    breaks: Vec<usize>,
    /// The jumps back into it, waiting for where its next pass begins.
    continues: Vec<usize>,
}

/// The compiler.
struct Compiler {
    chunk: Chunk,
    blocks: Blocks,
    enclosing: Vec<Enclosing>,
    /// The optional chains being compiled, innermost last, each holding the
    /// jumps that short-circuit it.
    chains: Vec<Vec<usize>>,
}

impl Compiler {
    // --- The program --------------------------------------------------------

    /// A whole script: what it declares before it runs, then what it does.
    fn program(&mut self, program: &Program) -> Result<(), Refusal> {
        if program.source != crate::ast::Source::Script {
            return Err(Refusal::NotBuiltYet {
                what: What::AModule,
                at: 0,
            });
        }

        let mut names = Vec::new();
        hoist::vars(&program.body, &mut names);
        for name in &names {
            let at = self.text(name)?;
            self.chunk.declare_var(at);
        }
        let mut declared: Vec<String> = Vec::new();
        for one in hoist::lexical(&program.body) {
            if declared.contains(&one.name) || names.contains(&one.name) {
                return Err(Refusal::NotAProgram {
                    why: format!(
                        "'{}' is declared twice at this script's top level",
                        one.name
                    ),
                    at: one.at,
                });
            }
            let at = self.text(&one.name)?;
            self.chunk.declare_lexical(at, one.mutable);
            declared.push(one.name);
        }

        // The completion value starts as `undefined`: a script of nothing but
        // declarations evaluates to that rather than to whatever was in the
        // slot.
        self.chunk.emit(Op::CompleteEmpty, 0);
        self.statements(&program.body)
    }

    // --- Statements ---------------------------------------------------------

    fn statements(&mut self, statements: &[Statement]) -> Result<(), Refusal> {
        for statement in statements {
            self.statement(statement)?;
        }
        Ok(())
    }

    fn statement(&mut self, statement: &Statement) -> Result<(), Refusal> {
        let at = statement.start;
        match &statement.kind {
            StatementKind::Expression(expression) => {
                self.expression(expression)?;
                self.chunk.emit(Op::Complete, at);
            }
            StatementKind::Empty | StatementKind::Debugger => {
                // `debugger` with nobody attached is specified to do nothing at
                // all, which is exactly this rather than a refusal.
            }
            StatementKind::Block(body) => {
                self.block(body)?;
            }
            StatementKind::Declaration(declaration) => self.declaration(declaration, at)?,
            StatementKind::If {
                test,
                consequent,
                alternate,
            } => {
                // An `if` whose branch produces nothing makes the program's
                // completion `undefined` rather than leaving the last one —
                // `2; if (true) {}` is `undefined`, and `2; {}` is `2`.
                self.chunk.emit(Op::CompleteEmpty, at);
                self.expression(test)?;
                let otherwise = self.chunk.emit(Op::JumpIfFalse(0), at);
                self.statement(consequent)?;
                match alternate {
                    Some(alternate) => {
                        let over = self.chunk.emit(Op::Jump(0), at);
                        self.patch(otherwise)?;
                        self.statement(alternate)?;
                        self.patch(over)?;
                    }
                    None => self.patch(otherwise)?,
                }
            }
            StatementKind::While { test, body } => self.while_loop(test, body, at, None)?,
            StatementKind::DoWhile { body, test } => self.do_while(body, test, at, None)?,
            StatementKind::For {
                init,
                test,
                update,
                body,
            } => self.for_loop(
                init.as_ref(),
                test.as_ref(),
                update.as_ref(),
                body,
                at,
                None,
            )?,
            StatementKind::Switch {
                discriminant,
                cases,
            } => self.switch(discriminant, cases, at, None)?,
            StatementKind::Labelled { label, body } => self.labelled(label, body, at)?,
            StatementKind::Break(label) => self.leave(label.as_deref(), at, false)?,
            StatementKind::Continue(label) => self.leave(label.as_deref(), at, true)?,
            StatementKind::Throw(value) => {
                self.expression(value)?;
                self.chunk.emit(Op::Throw, at);
            }
            StatementKind::Try { .. } => {
                return Err(Refusal::NotBuiltYet {
                    what: What::ACatch,
                    at,
                });
            }
            // A `return` is here rather than with the calls because what it is
            // missing is a function to return *from*.
            StatementKind::Function(_) | StatementKind::Class(_) | StatementKind::Return(_) => {
                return Err(Refusal::NotBuiltYet {
                    what: What::AFunction,
                    at,
                });
            }
            StatementKind::ForIn { .. } | StatementKind::ForOf { .. } => {
                return Err(Refusal::NotBuiltYet {
                    what: What::TakingAValueApart,
                    at,
                });
            }
            StatementKind::Import(_) | StatementKind::Export(_) => {
                return Err(Refusal::NotBuiltYet {
                    what: What::AModule,
                    at,
                });
            }
        }
        Ok(())
    }

    /// A `{ … }`: its own bindings, in their own dead zones.
    fn block(&mut self, body: &[Statement]) -> Result<(), Refusal> {
        self.blocks.open();
        let outcome = self.block_body(body);
        self.blocks.close();
        outcome
    }

    /// The inside of a block, with the scope already open.
    fn block_body(&mut self, body: &[Statement]) -> Result<(), Refusal> {
        self.declare_lexically(body)?;
        self.statements(body)
    }

    /// Give this statement list's own `let` and `const` names their slots, and
    /// put each slot in its dead zone.
    ///
    /// The `Uninitialize` matters on the second pass of a loop rather than the
    /// first: the block is entered again, and a binding it declares has not been
    /// reached yet.
    fn declare_lexically(&mut self, body: &[Statement]) -> Result<(), Refusal> {
        for one in hoist::lexical(body) {
            let slot = self.slot(one.at)?;
            if self.blocks.declare(&one.name, slot, one.mutable).is_err() {
                return Err(Refusal::NotAProgram {
                    why: format!("'{}' is declared twice in the same block", one.name),
                    at: one.at,
                });
            }
            let name = self.text(&one.name)?;
            self.chunk.name_slot(slot, name);
            self.chunk.emit(Op::Uninitialize(slot), one.at);
        }
        Ok(())
    }

    fn declaration(&mut self, declaration: &Declaration, at: usize) -> Result<(), Refusal> {
        for declarator in &declaration.declarators {
            let Pattern::Name(name) = &declarator.pattern else {
                return Err(Refusal::NotBuiltYet {
                    what: What::TakingAValueApart,
                    at,
                });
            };
            match declaration.kind {
                DeclarationKind::Var => {
                    // The binding is already on the global object; a `var` with
                    // no initialiser leaves whatever is there, which is what
                    // makes `var a = 1; var a;` still one.
                    let Some(init) = &declarator.init else {
                        continue;
                    };
                    self.expression(init)?;
                    let text = self.text(name)?;
                    self.chunk.emit(Op::StoreGlobal(text), at);
                    self.chunk.emit(Op::Pop, at);
                }
                DeclarationKind::Let | DeclarationKind::Const => {
                    match &declarator.init {
                        Some(init) => self.expression(init)?,
                        None => {
                            self.chunk.emit(Op::Undefined, at);
                        }
                    }
                    match self.blocks.find(name) {
                        Where::Local { slot, .. } => {
                            self.chunk.emit(Op::Initialize(slot), at);
                        }
                        Where::Global => {
                            let text = self.text(name)?;
                            self.chunk.emit(Op::InitializeGlobal(text), at);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    // --- Loops, switches and labels ----------------------------------------

    fn labelled(&mut self, label: &str, body: &Statement, at: usize) -> Result<(), Refusal> {
        // A label in front of a loop belongs to the loop, so that
        // `continue outer` reaches the loop's own next pass rather than jumping
        // to a place that would run its head again.
        match &body.kind {
            StatementKind::While { test, body } => self.while_loop(test, body, at, Some(label)),
            StatementKind::DoWhile { body, test } => self.do_while(body, test, at, Some(label)),
            StatementKind::For {
                init,
                test,
                update,
                body,
            } => self.for_loop(
                init.as_ref(),
                test.as_ref(),
                update.as_ref(),
                body,
                at,
                Some(label),
            ),
            StatementKind::Switch {
                discriminant,
                cases,
            } => self.switch(discriminant, cases, at, Some(label)),
            _ => {
                // Anything else may be broken out of and never continued.
                self.enter(Some(label), false);
                let outcome = self.statement(body);
                let leaving = self.finish_switch(at);
                outcome?;
                leaving
            }
        }
    }

    fn while_loop(
        &mut self,
        test: &Expression,
        body: &Statement,
        at: usize,
        label: Option<&str>,
    ) -> Result<(), Refusal> {
        self.chunk.emit(Op::CompleteEmpty, at);
        let top = self.chunk.here();
        self.expression(test)?;
        let out = self.chunk.emit(Op::JumpIfFalse(0), at);
        self.enter(label, true);
        let outcome = self.statement(body);
        self.close_loop(top, at)?;
        outcome?;
        self.patch(out)
    }

    fn do_while(
        &mut self,
        body: &Statement,
        test: &Expression,
        at: usize,
        label: Option<&str>,
    ) -> Result<(), Refusal> {
        self.chunk.emit(Op::CompleteEmpty, at);
        let top = self.chunk.here();
        // A `continue` in a `do … while` goes to the test rather than to the
        // top, which is why the place its next pass begins is not known until
        // the body has been compiled.
        self.enter(label, true);
        let outcome = self.statement(body);
        let again = self.chunk.here();
        outcome?;
        self.expression(test)?;
        let out = self.chunk.emit(Op::JumpIfFalse(0), at);
        let Ok(target) = u32::try_from(top) else {
            return Err(too_long(at));
        };
        self.chunk.emit(Op::Jump(target), at);
        self.patch(out)?;
        self.finish_loop(again, at)
    }

    fn for_loop(
        &mut self,
        init: Option<&ForInit>,
        test: Option<&Expression>,
        update: Option<&Expression>,
        body: &Statement,
        at: usize,
        label: Option<&str>,
    ) -> Result<(), Refusal> {
        self.chunk.emit(Op::CompleteEmpty, at);
        // The head's own scope, so `for (let i = 0; …)` does not leak `i`.
        //
        // One slot rather than one per pass: the language copies a `let` head
        // into every iteration, and nothing can tell — a closure is what would
        // (queue item 209), and there are none.
        self.blocks.open();
        let outcome = self.for_inside(init, test, update, body, at, label);
        self.blocks.close();
        outcome
    }

    fn for_inside(
        &mut self,
        init: Option<&ForInit>,
        test: Option<&Expression>,
        update: Option<&Expression>,
        body: &Statement,
        at: usize,
        label: Option<&str>,
    ) -> Result<(), Refusal> {
        match init {
            Some(ForInit::Declaration(declaration)) => {
                if declaration.kind != DeclarationKind::Var {
                    let mut names = Vec::new();
                    for declarator in &declaration.declarators {
                        if let Pattern::Name(name) = &declarator.pattern {
                            names.push(name.clone());
                        }
                    }
                    for name in names {
                        let slot = self.slot(at)?;
                        let mutable = declaration.kind == DeclarationKind::Let;
                        if self.blocks.declare(&name, slot, mutable).is_err() {
                            return Err(Refusal::NotAProgram {
                                why: format!("'{name}' is declared twice in this loop's header"),
                                at,
                            });
                        }
                        let spelling = self.text(&name)?;
                        self.chunk.name_slot(slot, spelling);
                        self.chunk.emit(Op::Uninitialize(slot), at);
                    }
                }
                self.declaration(declaration, at)?;
            }
            Some(ForInit::Expression(expression)) => {
                self.expression(expression)?;
                self.chunk.emit(Op::Pop, at);
            }
            None => {}
        }

        let top = self.chunk.here();
        let out = match test {
            Some(test) => {
                self.expression(test)?;
                Some(self.chunk.emit(Op::JumpIfFalse(0), at))
            }
            None => None,
        };
        self.enter(label, true);
        let outcome = self.statement(body);
        // `continue` goes to the update rather than to the test, which is the
        // difference between `for (;;i++)` counting and not.
        let again = self.chunk.here();
        outcome?;
        if let Some(update) = update {
            self.expression(update)?;
            self.chunk.emit(Op::Pop, at);
        }
        let Ok(target) = u32::try_from(top) else {
            return Err(too_long(at));
        };
        self.chunk.emit(Op::Jump(target), at);
        if let Some(out) = out {
            self.patch(out)?;
        }
        self.finish_loop(again, at)
    }

    fn switch(
        &mut self,
        discriminant: &Expression,
        cases: &[crate::ast::SwitchCase],
        at: usize,
        label: Option<&str>,
    ) -> Result<(), Refusal> {
        self.chunk.emit(Op::CompleteEmpty, at);
        self.expression(discriminant)?;
        // Kept in a slot rather than on the stack: a `break` out of a case
        // would otherwise have to know how deep the stack was when it left.
        let held = self.slot(at)?;
        self.chunk.emit(Op::Initialize(held), at);

        self.blocks.open();
        let outcome = self.switch_inside(cases, held, at, label);
        self.blocks.close();
        outcome
    }

    fn switch_inside(
        &mut self,
        cases: &[crate::ast::SwitchCase],
        held: u32,
        at: usize,
        label: Option<&str>,
    ) -> Result<(), Refusal> {
        // Every case body is one block, so a `let` in one case is in the dead
        // zone of the whole `switch` rather than of its own case — which is why
        // each case's declarations are taken into the same scope rather than
        // into one of its own.
        for case in cases {
            self.declare_lexically(&case.body)?;
        }

        self.enter(label, false);
        let outcome = self.switch_cases(cases, held, at);
        // A `switch` is not a loop: `continue` inside one leaves it entirely,
        // which is what `repeats: None` says.
        let leaving = self.finish_switch(at);
        outcome?;
        leaving
    }

    fn switch_cases(
        &mut self,
        cases: &[crate::ast::SwitchCase],
        held: u32,
        at: usize,
    ) -> Result<(), Refusal> {
        let mut go_to = Vec::new();
        let mut fallback = None;
        for case in cases {
            let Some(test) = &case.test else {
                // The `default` is tested last however early it is written,
                // which is why it is remembered rather than emitted here.
                fallback = Some(go_to.len());
                go_to.push(usize::MAX);
                continue;
            };
            self.chunk.emit(Op::Load(held), at);
            self.expression(test)?;
            self.chunk
                .emit(Op::Binary(crate::ast::Binary::StrictlyEqual), at);
            let next = self.chunk.emit(Op::JumpIfFalse(0), at);
            go_to.push(self.chunk.emit(Op::Jump(0), at));
            self.patch(next)?;
        }
        let none_matched = self.chunk.emit(Op::Jump(0), at);

        let mut starts = Vec::new();
        for case in cases {
            starts.push(self.chunk.here());
            self.statements(&case.body)?;
        }
        let end = self.chunk.here();

        for (jump, start) in go_to.iter().zip(starts.iter()) {
            if *jump != usize::MAX && !self.chunk.patch(*jump, *start) {
                return Err(too_long(at));
            }
        }
        let landing = match fallback.and_then(|which| starts.get(which).copied()) {
            Some(start) => start,
            None => end,
        };
        if !self.chunk.patch(none_matched, landing) {
            return Err(too_long(at));
        }
        Ok(())
    }

    /// Go into something a `break` may leave.
    fn enter(&mut self, label: Option<&str>, repeats: bool) {
        self.enclosing.push(Enclosing {
            label: label.map(str::to_owned),
            repeats,
            breaks: Vec::new(),
            continues: Vec::new(),
        });
    }

    /// Come out of a loop whose next pass begins where it was entered.
    fn close_loop(&mut self, again: usize, at: usize) -> Result<(), Refusal> {
        let Ok(target) = u32::try_from(again) else {
            return Err(too_long(at));
        };
        self.chunk.emit(Op::Jump(target), at);
        self.finish_loop(again, at)
    }

    /// Patch a loop's `break`s to here and its `continue`s to `again`.
    fn finish_loop(&mut self, again: usize, at: usize) -> Result<(), Refusal> {
        let Some(enclosing) = self.enclosing.pop() else {
            return Err(lost(at));
        };
        let here = self.chunk.here();
        for jump in enclosing.breaks {
            if !self.chunk.patch(jump, here) {
                return Err(too_long(at));
            }
        }
        for jump in enclosing.continues {
            if !self.chunk.patch(jump, again) {
                return Err(too_long(at));
            }
        }
        Ok(())
    }

    /// Patch a `switch`'s `break`s to here; it has no `continue`s of its own.
    fn finish_switch(&mut self, at: usize) -> Result<(), Refusal> {
        self.finish_loop(self.chunk.here(), at)
    }

    /// `break` and `continue`.
    fn leave(&mut self, label: Option<&str>, at: usize, repeating: bool) -> Result<(), Refusal> {
        let jump = self.chunk.emit(Op::Jump(0), at);
        let found = self.enclosing.iter_mut().rev().find(|enclosing| {
            let named = match label {
                Some(label) => enclosing.label.as_deref() == Some(label),
                None => true,
            };
            named && (!repeating || enclosing.repeats)
        });
        let Some(enclosing) = found else {
            // A `break` naming a label nothing declares is an early error, and
            // the compiler has to catch it: there is no instruction it could
            // emit instead. Queue item 205 names this one.
            let what = if repeating { "continue" } else { "break" };
            return Err(Refusal::NotAProgram {
                why: match label {
                    Some(label) => format!("'{what} {label}' names no label that is open here"),
                    None => format!("'{what}' is not inside anything it could leave"),
                },
                at,
            });
        };
        if repeating {
            enclosing.continues.push(jump);
        } else {
            enclosing.breaks.push(jump);
        }
        Ok(())
    }

    // --- Expressions --------------------------------------------------------

    fn expression(&mut self, expression: &Expression) -> Result<(), Refusal> {
        let at = expression.start;
        match &expression.kind {
            ExpressionKind::Number(number) => {
                self.chunk.emit(Op::Number(*number), at);
            }
            ExpressionKind::String(units) => {
                let text = self.units(units)?;
                self.chunk.emit(Op::Text(text), at);
            }
            ExpressionKind::Boolean(is) => {
                self.chunk.emit(Op::Bool(*is), at);
            }
            ExpressionKind::Null => {
                self.chunk.emit(Op::Null, at);
            }
            ExpressionKind::Name(name) => match self.blocks.find(name) {
                Where::Local { slot, .. } => {
                    self.chunk.emit(Op::Load(slot), at);
                }
                Where::Global => {
                    let text = self.text(name)?;
                    self.chunk.emit(Op::LoadGlobal(text), at);
                }
            },
            ExpressionKind::Template(template) => self.template(template, at)?,
            ExpressionKind::Object(properties) => self.object(properties, at)?,
            ExpressionKind::Unary { operator, argument } => self.unary(*operator, argument, at)?,
            ExpressionKind::Update {
                increment,
                prefix,
                argument,
            } => self.update(*increment, *prefix, argument, at)?,
            ExpressionKind::Binary {
                operator,
                left,
                right,
            } => {
                self.expression(left)?;
                self.expression(right)?;
                self.chunk.emit(Op::Binary(*operator), at);
            }
            ExpressionKind::Logical {
                operator,
                left,
                right,
            } => self.short_circuiting(*operator, left, right, at)?,
            ExpressionKind::Conditional {
                test,
                consequent,
                alternate,
            } => self.conditional(test, consequent, alternate, at)?,
            ExpressionKind::Sequence(expressions) => self.sequence(expressions, at)?,
            ExpressionKind::Member {
                object,
                member,
                optional,
            } => self.member(object, member, *optional, at)?,
            ExpressionKind::Chain(inner) => self.chain(inner, at)?,
            ExpressionKind::Assignment {
                operator,
                target,
                value,
            } => self.assignment(*operator, target, value, at)?,
            // Everything the language has and this engine has not built. One
            // arm each in [`Compiler::not_built_yet`], so that the list of what
            // is missing is one list rather than a refusal scattered through
            // the compiler.
            ExpressionKind::Array(_)
            | ExpressionKind::Function(_)
            | ExpressionKind::Class(_)
            | ExpressionKind::Call { .. }
            | ExpressionKind::New { .. }
            | ExpressionKind::TaggedTemplate { .. }
            | ExpressionKind::Super
            | ExpressionKind::NewTarget
            | ExpressionKind::PrivateName(_)
            | ExpressionKind::This
            | ExpressionKind::RegularExpression(_)
            | ExpressionKind::BigInt { .. }
            | ExpressionKind::Yield { .. }
            | ExpressionKind::Await(_)
            | ExpressionKind::ImportMeta
            | ExpressionKind::ImportCall { .. } => {
                return Err(not_built_yet(&expression.kind, at));
            }
        }
        Ok(())
    }

    /// `a && b`, `a || b`, `a ?? b` — the ones that may not evaluate their
    /// right side.
    fn short_circuiting(
        &mut self,
        operator: crate::ast::Logical,
        left: &Expression,
        right: &Expression,
        at: usize,
    ) -> Result<(), Refusal> {
        self.expression(left)?;
        let over = self.chunk.emit(
            match operator {
                crate::ast::Logical::And => Op::JumpIfFalseKeep(0),
                crate::ast::Logical::Or => Op::JumpIfTrueKeep(0),
                crate::ast::Logical::Coalesce => Op::JumpIfNotNullishKeep(0),
            },
            at,
        );
        self.expression(right)?;
        self.patch(over)
    }

    /// `a ? b : c`.
    fn conditional(
        &mut self,
        test: &Expression,
        consequent: &Expression,
        alternate: &Expression,
        at: usize,
    ) -> Result<(), Refusal> {
        self.expression(test)?;
        let otherwise = self.chunk.emit(Op::JumpIfFalse(0), at);
        self.expression(consequent)?;
        let over = self.chunk.emit(Op::Jump(0), at);
        self.patch(otherwise)?;
        self.expression(alternate)?;
        self.patch(over)
    }

    /// `a, b, c` — every one evaluated, the last one kept.
    fn sequence(&mut self, expressions: &[Expression], at: usize) -> Result<(), Refusal> {
        let last = expressions.len().saturating_sub(1);
        for (which, expression) in expressions.iter().enumerate() {
            self.expression(expression)?;
            if which != last {
                self.chunk.emit(Op::Pop, at);
            }
        }
        Ok(())
    }

    /// `a.b`, `a[b]`, `a?.b`.
    fn member(
        &mut self,
        object: &Expression,
        member: &Member,
        optional: bool,
        at: usize,
    ) -> Result<(), Refusal> {
        self.expression(object)?;
        if optional {
            self.short_circuit(at)?;
        }
        if let Member::Computed(key) = member {
            self.expression(key)?;
        }
        self.read_member(member, at)
    }

    /// `` `a${b}c` ``, which is `ToString` on each substitution and then a join.
    fn template(&mut self, template: &Template, at: usize) -> Result<(), Refusal> {
        let mut pieces = template.pieces.iter();
        let Some(first) = pieces.next() else {
            return Err(Refusal::NotAProgram {
                why: "a template with no pieces at all".to_owned(),
                at,
            });
        };
        self.push_piece(first, at)?;
        for (expression, piece) in template.expressions.iter().zip(pieces) {
            self.expression(expression)?;
            // `ToString` rather than letting `+` decide: a template asks an
            // object for its `toString` first and `+` asks for its `valueOf`
            // first, and a page can tell the two apart.
            self.chunk.emit(Op::ToText, at);
            self.chunk.emit(Op::Binary(crate::ast::Binary::Add), at);
            self.push_piece(piece, at)?;
            self.chunk.emit(Op::Binary(crate::ast::Binary::Add), at);
        }
        Ok(())
    }

    fn push_piece(&mut self, piece: &crate::template::Piece, at: usize) -> Result<(), Refusal> {
        let Some(cooked) = &piece.cooked else {
            // Only a *tagged* template may hold a piece that could not be
            // read, and a tagged template is a call (queue item 209). An
            // untagged one with a bad escape never parsed.
            return Err(Refusal::NotBuiltYet {
                what: What::ACall,
                at,
            });
        };
        let text = self.units(cooked)?;
        self.chunk.emit(Op::Text(text), at);
        Ok(())
    }

    /// `{ a: 1, [b]: 2, __proto__: c }`.
    fn object(&mut self, properties: &[Property], at: usize) -> Result<(), Refusal> {
        self.chunk.emit(Op::Object, at);
        for property in properties {
            match property {
                Property::Named {
                    key,
                    value,
                    shorthand,
                } => {
                    // `{ __proto__: a }` sets the prototype and defines no
                    // property, and `{ ["__proto__"]: a }` and
                    // `{ __proto__ }` define one — the difference is the
                    // grammar rather than the name.
                    if !*shorthand && is_proto(key) {
                        self.expression(value)?;
                        self.chunk.emit(Op::SetPrototype, at);
                        continue;
                    }
                    match key {
                        Key::Computed(expression) => {
                            self.expression(expression)?;
                            self.expression(value)?;
                            self.chunk.emit(Op::DefineKeyed, at);
                        }
                        Key::Name(name) => {
                            let text = self.text(name)?;
                            self.expression(value)?;
                            self.chunk.emit(Op::DefineNamed(text), at);
                        }
                        Key::String(units) => {
                            let text = self.units(units)?;
                            self.expression(value)?;
                            self.chunk.emit(Op::DefineNamed(text), at);
                        }
                        Key::Number(number) => {
                            let text = self.text(&crate::numeric::text_of(*number))?;
                            self.expression(value)?;
                            self.chunk.emit(Op::DefineNamed(text), at);
                        }
                        Key::Private(_) => {
                            return Err(Refusal::NotBuiltYet {
                                what: What::AFunction,
                                at,
                            });
                        }
                    }
                }
                Property::Method(_) => {
                    return Err(Refusal::NotBuiltYet {
                        what: What::AFunction,
                        at,
                    });
                }
                Property::Spread(_) => {
                    return Err(Refusal::NotBuiltYet {
                        what: What::TakingAValueApart,
                        at,
                    });
                }
            }
        }
        Ok(())
    }

    fn unary(&mut self, operator: Unary, argument: &Expression, at: usize) -> Result<(), Refusal> {
        if let Some(simple) = Simple::of(operator) {
            self.expression(argument)?;
            self.chunk.emit(Op::Unary(simple), at);
            return Ok(());
        }
        match operator {
            Unary::TypeOf => {
                // A name that resolves to nothing is `"undefined"` rather than
                // a `ReferenceError`, and that is the only reason this is not
                // an ordinary evaluation followed by an instruction.
                if let ExpressionKind::Name(name) = &argument.kind {
                    if self.blocks.find(name) == Where::Global {
                        let text = self.text(name)?;
                        self.chunk.emit(Op::TypeOfGlobal(text), at);
                        return Ok(());
                    }
                }
                self.expression(argument)?;
                self.chunk.emit(Op::TypeOf, at);
                Ok(())
            }
            _ => self.delete(argument, at),
        }
    }

    /// `delete a.b`, `delete a[b]`, `delete a`.
    fn delete(&mut self, argument: &Expression, at: usize) -> Result<(), Refusal> {
        match &argument.kind {
            ExpressionKind::Member {
                object,
                member,
                optional: false,
            } => {
                self.expression(object)?;
                match member {
                    Member::Name(name) => {
                        let text = self.text(name)?;
                        self.chunk.emit(Op::DeleteNamed(text), at);
                    }
                    Member::Computed(key) => {
                        self.expression(key)?;
                        self.chunk.emit(Op::DeleteKeyed, at);
                    }
                    Member::Private(_) => {
                        return Err(Refusal::NotBuiltYet {
                            what: What::AFunction,
                            at,
                        });
                    }
                }
                Ok(())
            }
            ExpressionKind::Name(name) => {
                if self.chunk.strict() {
                    return Err(Refusal::NotAProgram {
                        why: format!("strict code may not write 'delete {name}'"),
                        at,
                    });
                }
                match self.blocks.find(name) {
                    // A `let` or `const` binding cannot be deleted, and the
                    // language answers `false` rather than throwing.
                    Where::Local { .. } => {
                        self.chunk.emit(Op::Bool(false), at);
                    }
                    Where::Global => {
                        let text = self.text(name)?;
                        self.chunk.emit(Op::DeleteGlobal(text), at);
                    }
                }
                Ok(())
            }
            // `delete 1`, `delete (a, b)` — evaluated for its effects, and
            // `true`.
            _ => {
                self.expression(argument)?;
                self.chunk.emit(Op::Pop, at);
                self.chunk.emit(Op::Bool(true), at);
                Ok(())
            }
        }
    }

    /// `a++`, `--a`.
    fn update(
        &mut self,
        increment: bool,
        prefix: bool,
        argument: &Expression,
        at: usize,
    ) -> Result<(), Refusal> {
        let operator = if increment {
            crate::ast::Binary::Add
        } else {
            crate::ast::Binary::Subtract
        };
        match &argument.kind {
            ExpressionKind::Name(name) => {
                let put = self.load_name(name, at)?;
                self.chunk.emit(Op::ToNumeric, at);
                if !prefix {
                    self.chunk.emit(Op::Dup, at);
                }
                self.chunk.emit(Op::Number(1.0), at);
                self.chunk.emit(Op::Binary(operator), at);
                self.store_name(put, at);
                if !prefix {
                    // The old value is underneath, which is what `a++` is.
                    self.chunk.emit(Op::Pop, at);
                }
                Ok(())
            }
            ExpressionKind::Member {
                object,
                member,
                optional: false,
            } => {
                self.expression(object)?;
                let keyed = matches!(member, Member::Computed(_));
                if let Member::Computed(key) = member {
                    self.expression(key)?;
                }
                if keyed {
                    self.chunk.emit(Op::DupTwo, at);
                } else {
                    self.chunk.emit(Op::Dup, at);
                }
                self.read_member(member, at)?;
                self.chunk.emit(Op::ToNumeric, at);
                let old = if prefix {
                    None
                } else {
                    // The old value goes in a slot: the object and the key are
                    // underneath it and the store needs them on top.
                    let slot = self.slot(at)?;
                    self.chunk.emit(Op::Dup, at);
                    self.chunk.emit(Op::Initialize(slot), at);
                    Some(slot)
                };
                self.chunk.emit(Op::Number(1.0), at);
                self.chunk.emit(Op::Binary(operator), at);
                self.write_member(member, at)?;
                if let Some(slot) = old {
                    self.chunk.emit(Op::Pop, at);
                    self.chunk.emit(Op::Load(slot), at);
                }
                Ok(())
            }
            _ => Err(Refusal::NotAProgram {
                why: "only a name or a property may be incremented".to_owned(),
                at,
            }),
        }
    }

    // --- Assignment ---------------------------------------------------------

    fn assignment(
        &mut self,
        operator: Assign,
        target: &Pattern,
        value: &Expression,
        at: usize,
    ) -> Result<(), Refusal> {
        match target {
            Pattern::Name(name) => self.assign_to_name(operator, name, value, at),
            Pattern::Member(member) => self.assign_to_member(operator, member, value, at),
            Pattern::Array { .. } | Pattern::Object { .. } => Err(Refusal::NotBuiltYet {
                what: What::TakingAValueApart,
                at,
            }),
        }
    }

    fn assign_to_name(
        &mut self,
        operator: Assign,
        name: &str,
        value: &Expression,
        at: usize,
    ) -> Result<(), Refusal> {
        if let Some(jump) = logical(operator) {
            // The jump takes the old value off itself on the path that carries
            // on, and leaves it on the path that short-circuits, so both sides
            // of it leave exactly one value.
            let put = self.load_name(name, at)?;
            let over = self.chunk.emit(jump, at);
            self.expression(value)?;
            self.store_name(put, at);
            return self.patch(over);
        }
        let put = if let Some(binary) = arithmetic(operator) {
            let put = self.load_name(name, at)?;
            self.expression(value)?;
            self.chunk.emit(Op::Binary(binary), at);
            put
        } else {
            self.expression(value)?;
            self.where_to_put(name)?
        };
        self.store_name(put, at);
        Ok(())
    }

    fn assign_to_member(
        &mut self,
        operator: Assign,
        member: &Expression,
        value: &Expression,
        at: usize,
    ) -> Result<(), Refusal> {
        let ExpressionKind::Member {
            object,
            member: which,
            optional: false,
        } = &member.kind
        else {
            return Err(Refusal::NotAProgram {
                why: "this is not something a value can be assigned to".to_owned(),
                at,
            });
        };

        if let Some(jump) = logical(operator) {
            // The object — and the key, if there is one — go in slots, because
            // the short circuit leaves the stack at a different depth than the
            // assignment does and a jump may not change how deep it is.
            let held = self.slot(at)?;
            self.expression(object)?;
            self.chunk.emit(Op::Initialize(held), at);
            let key = match which {
                Member::Computed(key) => {
                    let slot = self.slot(at)?;
                    self.expression(key)?;
                    self.chunk.emit(Op::Initialize(slot), at);
                    Some(slot)
                }
                _ => None,
            };
            self.chunk.emit(Op::Load(held), at);
            if let Some(slot) = key {
                self.chunk.emit(Op::Load(slot), at);
            }
            self.read_member(which, at)?;
            let over = self.chunk.emit(jump, at);
            self.chunk.emit(Op::Load(held), at);
            if let Some(slot) = key {
                self.chunk.emit(Op::Load(slot), at);
            }
            self.expression(value)?;
            self.write_member(which, at)?;
            return self.patch(over);
        }

        self.expression(object)?;
        if let Member::Computed(key) = which {
            self.expression(key)?;
        }
        if let Some(binary) = arithmetic(operator) {
            if matches!(which, Member::Computed(_)) {
                self.chunk.emit(Op::DupTwo, at);
            } else {
                self.chunk.emit(Op::Dup, at);
            }
            self.read_member(which, at)?;
            self.expression(value)?;
            self.chunk.emit(Op::Binary(binary), at);
        } else {
            self.expression(value)?;
        }
        self.write_member(which, at)
    }

    /// Read `a.b` or `a[b]` from what is already on the stack.
    fn read_member(&mut self, member: &Member, at: usize) -> Result<(), Refusal> {
        match member {
            Member::Name(name) => {
                let text = self.text(name)?;
                self.chunk.emit(Op::GetNamed(text), at);
                Ok(())
            }
            Member::Computed(_) => {
                self.chunk.emit(Op::GetKeyed, at);
                Ok(())
            }
            Member::Private(_) => Err(Refusal::NotBuiltYet {
                what: What::AFunction,
                at,
            }),
        }
    }

    /// Write `a.b` or `a[b]` from what is already on the stack.
    fn write_member(&mut self, member: &Member, at: usize) -> Result<(), Refusal> {
        match member {
            Member::Name(name) => {
                let text = self.text(name)?;
                self.chunk.emit(Op::SetNamed(text), at);
                Ok(())
            }
            Member::Computed(_) => {
                self.chunk.emit(Op::SetKeyed, at);
                Ok(())
            }
            Member::Private(_) => Err(Refusal::NotBuiltYet {
                what: What::AFunction,
                at,
            }),
        }
    }

    /// An optional link: if what is on the stack is nullish, the rest of the
    /// chain is skipped and the whole of it is `undefined`.
    fn short_circuit(&mut self, at: usize) -> Result<(), Refusal> {
        let jump = self.chunk.emit(Op::SkipTheChain(0), at);
        let Some(chain) = self.chains.last_mut() else {
            return Err(Refusal::NotAProgram {
                why: "an optional link outside a chain".to_owned(),
                at,
            });
        };
        chain.push(jump);
        Ok(())
    }

    /// The whole of a chain that has an optional link in it.
    fn chain(&mut self, inner: &Expression, at: usize) -> Result<(), Refusal> {
        self.chains.push(Vec::new());
        let outcome = self.expression(inner);
        let jumps = self.chains.pop().unwrap_or_default();
        outcome?;
        if jumps.is_empty() {
            return Ok(());
        }
        let over = self.chunk.emit(Op::Jump(0), at);
        let landing = self.chunk.here();
        // The nullish value is what the jump left; the chain is `undefined`
        // rather than that value, which is why it is dropped here.
        self.chunk.emit(Op::Pop, at);
        self.chunk.emit(Op::Undefined, at);
        for jump in jumps {
            if !self.chunk.patch(jump, landing) {
                return Err(too_long(at));
            }
        }
        self.patch(over)
    }

    // --- Names and slots ----------------------------------------------------

    /// Where a name is written, refusing an assignment to a `const`.
    fn where_to_put(&mut self, name: &str) -> Result<Put, Refusal> {
        Ok(match self.blocks.find(name) {
            Where::Local {
                slot,
                mutable: true,
            } => Put::Slot(slot),
            Where::Local {
                slot,
                mutable: false,
            } => Put::Constant {
                slot,
                name: self.text(name)?,
            },
            Where::Global => Put::Global(self.text(name)?),
        })
    }

    /// Read a name, answering where writing it back would go.
    fn load_name(&mut self, name: &str, at: usize) -> Result<Put, Refusal> {
        let put = self.where_to_put(name)?;
        match put {
            Put::Slot(slot) | Put::Constant { slot, .. } => {
                self.chunk.emit(Op::Load(slot), at);
            }
            Put::Global(text) => {
                self.chunk.emit(Op::LoadGlobal(text), at);
            }
        }
        Ok(put)
    }

    /// Write the value on the stack back to a name, leaving it there.
    fn store_name(&mut self, put: Put, at: usize) {
        match put {
            Put::Slot(slot) => self.chunk.emit(Op::Store(slot), at),
            Put::Global(text) => self.chunk.emit(Op::StoreGlobal(text), at),
            // A `const` this compiler can see: the value has been evaluated,
            // because the language evaluates it before it complains about it.
            Put::Constant { name, .. } => self.chunk.emit(Op::RefuseAssignment(name), at),
        };
    }

    /// The index of a name among the chunk's texts.
    fn text(&mut self, name: &str) -> Result<u32, Refusal> {
        let units: Vec<u16> = name.encode_utf16().collect();
        self.units(&units)
    }

    /// The same, for code units that are already what they are.
    fn units(&mut self, units: &[u16]) -> Result<u32, Refusal> {
        self.chunk
            .text_index(units)
            .ok_or_else(|| Refusal::NotAProgram {
                why: "this program holds more distinct strings than an index can name".to_owned(),
                at: 0,
            })
    }

    /// Take a frame slot.
    fn slot(&mut self, at: usize) -> Result<u32, Refusal> {
        self.chunk.take_slot().ok_or_else(|| Refusal::NotAProgram {
            why: "this program declares more bindings than a frame can hold".to_owned(),
            at,
        })
    }

    /// Point a jump at where the next instruction will land.
    fn patch(&mut self, jump: usize) -> Result<(), Refusal> {
        let here = self.chunk.here();
        if self.chunk.patch(jump, here) {
            return Ok(());
        }
        Err(too_long(self.chunk.at(jump)))
    }
}

/// Which queue item builds the expression this engine cannot compile.
///
/// One place rather than one refusal beside each arm, because *what is not
/// built yet* is a list somebody reads rather than a fact scattered through a
/// compiler.
fn not_built_yet(kind: &ExpressionKind, at: usize) -> Refusal {
    let what = match kind {
        // The empty array literal too: an array is an exotic object, and the
        // exotic part is its `length`.
        ExpressionKind::Array(_) => What::TakingAValueApart,
        ExpressionKind::Function(_) | ExpressionKind::Class(_) => What::AFunction,
        ExpressionKind::Call { .. }
        | ExpressionKind::New { .. }
        | ExpressionKind::TaggedTemplate { .. }
        | ExpressionKind::Super
        | ExpressionKind::NewTarget
        | ExpressionKind::PrivateName(_) => What::ACall,
        ExpressionKind::This => What::This,
        ExpressionKind::RegularExpression(_) => What::ARegularExpression,
        ExpressionKind::BigInt { .. } => What::ABigInt,
        ExpressionKind::Yield { .. } | ExpressionKind::Await(_) => What::ASuspension,
        _ => What::AModule,
    };
    Refusal::NotBuiltYet { what, at }
}

/// A program with more instructions than a jump can name.
fn too_long(at: usize) -> Refusal {
    Refusal::NotAProgram {
        why: "this program is longer than this engine will compile".to_owned(),
        at,
    }
}

/// The compiler lost track of what it was inside, which is its own bug.
fn lost(at: usize) -> Refusal {
    Refusal::NotAProgram {
        why: "this engine lost track of what this statement is inside".to_owned(),
        at,
    }
}

/// Whether this key is the `__proto__` that is not a property.
///
/// `{ __proto__: a }` sets the prototype; `{ ["__proto__"]: a }` and
/// `{ __proto__ }` define an ordinary property called `__proto__`. The
/// difference is the grammar rather than the name, which is why the shorthand
/// and the computed forms are asked about at the call site rather than here.
fn is_proto(key: &Key) -> bool {
    match key {
        Key::Name(name) => name == "__proto__",
        Key::String(units) => units.as_slice() == "__proto__".encode_utf16().collect::<Vec<u16>>(),
        Key::Computed(_) | Key::Number(_) | Key::Private(_) => false,
    }
}

/// Where an assignment puts its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Put {
    /// A frame slot.
    Slot(u32),
    /// A name the realm answers for, which decides at run time whether it is a
    /// lexical binding, a property of the global object, or nothing — and which
    /// refuses a `const` of its own.
    Global(u32),
    /// A `const` in a block, which this compiler can see is one. It still has a
    /// slot, because reading it is ordinary.
    Constant {
        /// Its slot, for reading.
        slot: u32,
        /// Its name, for the message when something assigns to it.
        name: u32,
    },
}

/// The binary operator inside a compound assignment, if it has one.
fn arithmetic(operator: Assign) -> Option<crate::ast::Binary> {
    use crate::ast::Binary;
    Some(match operator {
        Assign::Assign | Assign::And | Assign::Or | Assign::Coalesce => return None,
        Assign::Add => Binary::Add,
        Assign::Subtract => Binary::Subtract,
        Assign::Multiply => Binary::Multiply,
        Assign::Divide => Binary::Divide,
        Assign::Remainder => Binary::Remainder,
        Assign::Power => Binary::Power,
        Assign::ShiftLeft => Binary::ShiftLeft,
        Assign::ShiftRight => Binary::ShiftRight,
        Assign::ShiftRightUnsigned => Binary::ShiftRightUnsigned,
        Assign::BitAnd => Binary::BitAnd,
        Assign::BitOr => Binary::BitOr,
        Assign::BitXor => Binary::BitXor,
    })
}

/// The jump a logical assignment short-circuits with, if it is one.
///
/// `a ||= b` assigns only when `a` is falsy, and — the part that is a rule
/// rather than a shorthand — it does not assign at all otherwise, so a setter
/// is not called and a `const` is not refused.
fn logical(operator: Assign) -> Option<Op> {
    match operator {
        Assign::And => Some(Op::JumpIfFalseKeep(0)),
        Assign::Or => Some(Op::JumpIfTrueKeep(0)),
        Assign::Coalesce => Some(Op::JumpIfNotNullishKeep(0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Refusal, What, compile};
    use crate::code::Op;
    use crate::parser::script;

    fn chunk(source: &str) -> Result<crate::code::Chunk, Refusal> {
        match script(source) {
            Ok(program) => compile(&program),
            Err(why) => panic!("{source} does not parse: {why}"),
        }
    }

    #[test]
    fn a_name_in_a_block_is_a_slot_and_one_outside_it_is_not() {
        let Ok(chunk) = chunk("{ let a = 1; a; } b;") else {
            panic!("that compiles");
        };
        assert!(chunk.code().contains(&Op::Load(0)), "the block's own name");
        assert!(
            chunk
                .code()
                .iter()
                .any(|op| matches!(op, Op::LoadGlobal(_))),
            "and the one nothing declares"
        );
    }

    #[test]
    fn what_is_not_built_says_which_item_builds_it() {
        for (source, what) in [
            ("f()", What::ACall),
            ("function f() {}", What::AFunction),
            ("try { a; } catch {}", What::ACatch),
            ("[1, 2]", What::TakingAValueApart),
            ("for (const a of b) {}", What::TakingAValueApart),
            ("/a/", What::ARegularExpression),
            ("1n", What::ABigInt),
            ("this", What::This),
        ] {
            match chunk(source) {
                Err(Refusal::NotBuiltYet { what: named, .. }) => {
                    assert_eq!(named, what, "{source}");
                }
                other => panic!("{source} should name an item: {other:?}"),
            }
        }
    }

    #[test]
    fn the_early_errors_the_compiler_cannot_be_correct_without() {
        for source in [
            "{ let a; let a; }",
            "let a; const a = 1;",
            "break outer;",
            "outer: { continue outer; }",
        ] {
            match chunk(source) {
                Err(Refusal::NotAProgram { .. }) => {}
                other => panic!("{source} is not a program: {other:?}"),
            }
        }
    }
}
