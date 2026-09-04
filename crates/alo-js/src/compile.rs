/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A tree into instructions.
//!
//! The compiler walks the tree the parser built and emits the [`Unit`] the
//! interpreter runs. It decides three things the interpreter therefore does not
//! have to: **where a name lives**, **where a jump goes**, and **what a
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
//! # One unit, many chunks
//!
//! A function is a body of its own, so it is a [`Chunk`] of its own
//! ([`function`]), and the program they all belong to is the [`Unit`] that holds
//! them and the strings they name. The compiler is therefore a small stack of
//! **suspended** compilations rather than one: meeting a function puts the body
//! being compiled aside, compiles the function whole, and puts it back.
//!
//! # What it refuses, and why that is not a stub
//!
//! ADR 0013 § 3: *absent beats approximate*. Some of this language is not built
//! yet — every one of them is a queue item — and a compiler that emitted
//! something plausible for a `new`, a `try` or an array literal would produce a
//! program that runs and is wrong. So each is a [`Refusal`] that **names the
//! item that builds it**, in one list a person can read, and nothing downstream
//! has to wonder whether an instruction means what it says.
//!
//! The second kind of refusal is different and is not about this engine being
//! unfinished: [`Refusal::NotAProgram`] is an early error a *scope* is needed to
//! see, like `let a; let a;`. Queue item 205 owns those in general; the three
//! here are the ones the compiler cannot be correct without, and they are named
//! in that item.

pub mod function;
pub mod hoist;
pub mod scope;

use std::fmt;

use crate::ast::{
    Argument, Assign, Declaration, DeclarationKind, Expression, ExpressionKind, ForInit, Key,
    Member, Pattern, Program, Property, Statement, StatementKind, Template, Unary,
};
use crate::bounds;
use crate::code::{Chunk, Op};
use crate::operate::Simple;
use crate::unit::Unit;

use function::Naming;
use scope::{Assignment, Scopes, Where};

/// Something the language has that this engine has not built.
///
/// Each names the queue item that builds it, because *what to do about it* is
/// the useful half of the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum What {
    /// `new`, a class, `super`, a private name, `new.target` — anything that
    /// constructs (queue item 212).
    AConstruction,
    /// A parameter that is not a plain name: a default, a `...rest`, a pattern,
    /// a repeated name — and `arguments` (queue item 213).
    AParameterForm,
    /// A getter or a setter, which is a property whose *access* is a call
    /// (queue item 214).
    AnAccessor,
    /// `` tag`a${b}` `` (queue item 215).
    ATaggedTemplate,
    /// A function reading a **block's** binding of an enclosing function, which
    /// needs an environment per block (queue item 216).
    ACapturedBlockBinding,
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
    /// `yield` and `await`, and the `async` and generator functions that hold
    /// them (queue item 75).
    ASuspension,
}

impl What {
    /// The queue item that builds it.
    pub const fn item(self) -> u16 {
        match self {
            What::AConstruction => 212,
            What::AParameterForm => 213,
            What::AnAccessor => 214,
            What::ATaggedTemplate => 215,
            What::ACapturedBlockBinding => 216,
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
            What::AConstruction => "`new`, a class, `super` or a private name",
            What::AParameterForm => "a parameter that is not a plain name, or `arguments`",
            What::AnAccessor => "a getter or a setter",
            What::ATaggedTemplate => "a tagged template",
            What::ACapturedBlockBinding => "a function reading a name a block declared outside it",
            What::ACatch => "`try`, `catch` and `finally`",
            What::TakingAValueApart => {
                "an array literal, a spread, a destructuring pattern, `for…in` or `for…of`"
            }
            What::ARegularExpression => "a regular expression literal",
            What::ABigInt => "a `BigInt` literal",
            What::AModule => "`import` and `export`",
            What::ASuspension => "`yield`, `await` and the functions that hold them",
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
pub fn compile(program: &Program) -> Result<Unit, Refusal> {
    std::thread::scope(|scope| {
        let started = std::thread::Builder::new()
            .name("alo-js compile".to_owned())
            .stack_size(bounds::STACK_FOR_A_COMPILE)
            .spawn_scoped(scope, || here(program));
        match started {
            Ok(work) => match work.join() {
                Ok(unit) => unit,
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
fn here(program: &Program) -> Result<Unit, Refusal> {
    let mut compiler = Compiler {
        unit: Unit::new(),
        chunk: Chunk::new(program.strict),
        enclosing: Vec::new(),
        chains: Vec::new(),
        script: true,
        suspended: Vec::new(),
        scopes: Scopes::new(),
    };
    compiler.program(program)?;
    compiler.unit.finish(compiler.chunk);
    Ok(compiler.unit)
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

/// A body put aside while a function written inside it is compiled.
#[derive(Debug)]
struct Suspended {
    chunk: Chunk,
    enclosing: Vec<Enclosing>,
    chains: Vec<Vec<usize>>,
    script: bool,
}

/// The compiler.
struct Compiler {
    /// The program every chunk goes into, and the strings they all share.
    unit: Unit,
    /// The body being compiled now.
    chunk: Chunk,
    enclosing: Vec<Enclosing>,
    /// The optional chains being compiled, innermost last, each holding the
    /// jumps that short-circuit it.
    chains: Vec<Vec<usize>>,
    /// Whether the body being compiled is the script's own, which is the one
    /// body with a completion value.
    script: bool,
    /// The bodies put aside, outermost first.
    suspended: Vec<Suspended>,
    /// The scopes, which are **not** put aside: a function is compiled inside
    /// the scopes it was written in, and that is what makes it a closure.
    scopes: Scopes,
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
        // A function declared at a script's top level is var-scoped, so it is a
        // property of the global object like any other `var` — and it is given
        // its value before the first statement runs, which is the whole of what
        // "hoisted" means for one.
        let declared = hoist::functions(&program.body);
        for function in &declared {
            if let Some(name) = &function.name {
                if !names.contains(name) {
                    names.push(name.clone());
                }
            }
        }
        for name in &names {
            let at = self.text(name)?;
            self.chunk.declare_var(at);
        }
        let mut lexically: Vec<String> = Vec::new();
        for one in hoist::lexical(&program.body) {
            if lexically.contains(&one.name) || names.contains(&one.name) {
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
            lexically.push(one.name);
        }

        // The completion value starts as `undefined`: a script of nothing but
        // declarations evaluates to that rather than to whatever was in the
        // slot.
        self.chunk.emit(Op::CompleteEmpty, 0);

        for function in declared {
            let Some(name) = function.name.clone() else {
                continue;
            };
            let which = self.function_chunk(function, Naming::Outside)?;
            let at = function.start;
            self.chunk.emit(Op::Closure(which), at);
            let text = self.text(&name)?;
            self.chunk.emit(Op::StoreGlobal(text), at);
            self.chunk.emit(Op::Pop, at);
        }

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
                self.completes(at);
            }
            // Three statements that emit nothing, for two different reasons.
            // `debugger` with nobody attached is specified to do nothing at
            // all, which is exactly this rather than a refusal; and a function
            // declaration has already been made and given its name at the top
            // of the body or the block that holds it.
            StatementKind::Empty | StatementKind::Debugger | StatementKind::Function(_) => {}
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
                self.completes_empty(at);
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
            StatementKind::Return(value) => self.leave_function(value.as_ref(), at)?,
            StatementKind::Try { .. } => {
                return Err(Refusal::NotBuiltYet {
                    what: What::ACatch,
                    at,
                });
            }
            StatementKind::Class(_) => {
                return Err(Refusal::NotBuiltYet {
                    what: What::AConstruction,
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

    /// `return a;`.
    ///
    /// A function's answer, which is a different thing from a script's
    /// completion value: one is written by the program and read by its caller,
    /// and the other is what the whole script evaluated to.
    fn leave_function(&mut self, value: Option<&Expression>, at: usize) -> Result<(), Refusal> {
        if self.script {
            return Err(Refusal::NotAProgram {
                why: "a `return` outside a function has nothing to return from".to_owned(),
                at,
            });
        }
        match value {
            Some(value) => self.expression(value)?,
            None => {
                self.chunk.emit(Op::Undefined, at);
            }
        }
        self.chunk.emit(Op::Return, at);
        Ok(())
    }

    /// The value of an expression statement: a script's completion, or nothing
    /// a function's caller could see.
    fn completes(&mut self, at: usize) {
        if self.script {
            self.chunk.emit(Op::Complete, at);
        } else {
            self.chunk.emit(Op::Pop, at);
        }
    }

    /// The same for a statement that produced no value at all.
    fn completes_empty(&mut self, at: usize) {
        if self.script {
            self.chunk.emit(Op::CompleteEmpty, at);
        }
    }

    /// A `{ … }`: its own bindings, in their own dead zones.
    fn block(&mut self, body: &[Statement]) -> Result<(), Refusal> {
        self.scopes.open_block();
        let outcome = self.block_body(body);
        self.scopes.close();
        outcome
    }

    /// The inside of a block, with the scope already open.
    fn block_body(&mut self, body: &[Statement]) -> Result<(), Refusal> {
        self.declare_lexically(body)?;
        self.declare_functions(body)?;
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
            let assignment = if one.mutable {
                Assignment::Allowed
            } else {
                Assignment::Refused
            };
            if self.scopes.declare(&one.name, slot, assignment).is_err() {
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

    /// Make this block's own function declarations, before anything else in it
    /// runs.
    ///
    /// A function declared in a block is that block's — Annex B's second,
    /// var-scoped meaning is legacy and law 1 refuses it — so it is a frame slot
    /// like the block's `let`, and it is given its value here rather than left
    /// in a dead zone: a function declaration is readable above the line that
    /// writes it, which is the one thing that makes it different from
    /// `let f = function () {}`.
    fn declare_functions(&mut self, body: &[Statement]) -> Result<(), Refusal> {
        for function in hoist::functions(body) {
            let Some(name) = function.name.clone() else {
                continue;
            };
            let at = function.start;
            let slot = self.slot(at)?;
            if self
                .scopes
                .declare(&name, slot, Assignment::Allowed)
                .is_err()
            {
                return Err(Refusal::NotAProgram {
                    why: format!("'{name}' is declared twice in the same block"),
                    at,
                });
            }
            let text = self.text(&name)?;
            self.chunk.name_slot(slot, text);
            let which = self.function_chunk(function, Naming::Outside)?;
            self.chunk.emit(Op::Closure(which), at);
            self.chunk.emit(Op::Initialize(slot), at);
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
                    // The binding already exists — a property of the global
                    // object, or a binding of the enclosing function — and a
                    // `var` with no initialiser leaves whatever is there, which
                    // is what makes `var a = 1; var a;` still one.
                    let Some(init) = &declarator.init else {
                        continue;
                    };
                    self.expression(init)?;
                    let put = self.where_to_put(name, at)?;
                    self.store_name(put, at);
                    self.chunk.emit(Op::Pop, at);
                }
                DeclarationKind::Let | DeclarationKind::Const => {
                    match &declarator.init {
                        Some(init) => self.expression(init)?,
                        None => {
                            self.chunk.emit(Op::Undefined, at);
                        }
                    }
                    match self.resolve(name, at)? {
                        Where::Local { slot, .. } => {
                            self.chunk.emit(Op::Initialize(slot), at);
                        }
                        Where::Binding { hops, slot, .. } => {
                            self.chunk.emit(Op::InitializeBinding { hops, slot }, at);
                        }
                        Where::Captured => return Err(captured(at)),
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
        self.completes_empty(at);
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
        self.completes_empty(at);
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
        self.completes_empty(at);
        // The head's own scope, so `for (let i = 0; …)` does not leak `i`.
        //
        // One slot rather than one per pass: the language copies a `let` head
        // into every iteration, and **nothing here can tell**, because a
        // function may not read a block's binding from outside (queue item 216)
        // and a closure is the only thing that could see the difference.
        self.scopes.open_block();
        let outcome = self.for_inside(init, test, update, body, at, label);
        self.scopes.close();
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
                    let assignment = if declaration.kind == DeclarationKind::Let {
                        Assignment::Allowed
                    } else {
                        Assignment::Refused
                    };
                    for name in names {
                        let slot = self.slot(at)?;
                        if self.scopes.declare(&name, slot, assignment).is_err() {
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
        self.completes_empty(at);
        self.expression(discriminant)?;
        // Kept in a slot rather than on the stack: a `break` out of a case
        // would otherwise have to know how deep the stack was when it left.
        let held = self.slot(at)?;
        self.chunk.emit(Op::Initialize(held), at);

        self.scopes.open_block();
        let outcome = self.switch_inside(cases, held, at, label);
        self.scopes.close();
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
        for case in cases {
            self.declare_functions(&case.body)?;
        }

        self.enter(label, false);
        let outcome = self.switch_cases(cases, held, at);
        // A `switch` is not a loop: `continue` inside one leaves it entirely,
        // which is what `repeats: false` says.
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
            ExpressionKind::Name(name) => {
                self.load_name(name, at)?;
            }
            ExpressionKind::This => {
                self.chunk.emit(Op::This, at);
            }
            ExpressionKind::Function(function) => {
                let which = self.function_chunk(function, Naming::Itself)?;
                self.chunk.emit(Op::Closure(which), at);
            }
            ExpressionKind::Call {
                callee,
                arguments,
                optional,
            } => self.call(callee, arguments, *optional, at)?,
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
            // arm each in [`not_built_yet`], so that the list of what is
            // missing is one list rather than a refusal scattered through the
            // compiler.
            ExpressionKind::Array(_)
            | ExpressionKind::Class(_)
            | ExpressionKind::New { .. }
            | ExpressionKind::TaggedTemplate { .. }
            | ExpressionKind::Super
            | ExpressionKind::NewTarget
            | ExpressionKind::PrivateName(_)
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

    /// `f(a)`, `o.m(a)`, `f?.(a)`.
    ///
    /// The stack a call leaves behind it is **the callee, the `this` it was
    /// reached through, and then the arguments** — one shape for every form,
    /// because deciding `this` at the call site is what makes `o.m()` different
    /// from `var m = o.m; m()` and the difference is not something the callee
    /// can work out for itself.
    fn call(
        &mut self,
        callee: &Expression,
        arguments: &[Argument],
        optional: bool,
        at: usize,
    ) -> Result<(), Refusal> {
        let mut argc = 0_u32;
        for argument in arguments {
            match argument {
                Argument::Item(_) => argc = argc.saturating_add(1),
                Argument::Spread(_) => {
                    return Err(Refusal::NotBuiltYet {
                        what: What::TakingAValueApart,
                        at,
                    });
                }
            }
        }

        match &callee.kind {
            ExpressionKind::Member {
                object,
                member,
                optional: through,
            } => {
                // The object goes in a slot rather than staying under the
                // function: a short circuit must leave **one** value on the
                // stack for the chain's landing to drop, and an object left
                // underneath would be a second.
                let held = self.slot(at)?;
                self.expression(object)?;
                if *through {
                    self.short_circuit(at)?;
                }
                self.chunk.emit(Op::Initialize(held), at);
                self.chunk.emit(Op::Load(held), at);
                if let Member::Computed(key) = member {
                    self.expression(key)?;
                }
                self.read_member(member, at)?;
                if optional {
                    self.short_circuit(at)?;
                }
                self.chunk.emit(Op::Load(held), at);
            }
            ExpressionKind::Super => {
                return Err(Refusal::NotBuiltYet {
                    what: What::AConstruction,
                    at,
                });
            }
            _ => {
                self.expression(callee)?;
                if optional {
                    self.short_circuit(at)?;
                }
                // A plain call passes `undefined`, and the **callee's** own
                // strictness then decides whether that stays `undefined` or
                // becomes the global object. That is the specification's order
                // and it is not the caller's business.
                self.chunk.emit(Op::Undefined, at);
            }
        }

        for argument in arguments {
            if let Argument::Item(value) = argument {
                self.expression(value)?;
            }
        }
        self.chunk.emit(Op::Call(argc), at);
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
            // read (queue item 215). An untagged one with a bad escape never
            // parsed.
            return Err(Refusal::NotBuiltYet {
                what: What::ATaggedTemplate,
                at,
            });
        };
        let text = self.units(cooked)?;
        self.chunk.emit(Op::Text(text), at);
        Ok(())
    }

    /// `{ a: 1, [b]: 2, __proto__: c, m() {} }`.
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
                    self.define_property(key, at, |compiler| compiler.expression(value))?;
                }
                Property::Method(method) => {
                    if method.kind != crate::ast::MethodKind::Method {
                        // A getter or a setter is a property whose *access* is
                        // a call, which is a different mechanism from a
                        // property that holds one (queue item 214).
                        return Err(Refusal::NotBuiltYet {
                            what: What::AnAccessor,
                            at,
                        });
                    }
                    let function = &method.function;
                    self.define_property(&method.key, at, |compiler| {
                        let which = compiler.function_chunk(function, Naming::Outside)?;
                        compiler.chunk.emit(Op::Closure(which), at);
                        Ok(())
                    })?;
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

    /// One property of an object literal: its key, then whatever `value`
    /// leaves on the stack, then the definition.
    fn define_property(
        &mut self,
        key: &Key,
        at: usize,
        value: impl FnOnce(&mut Self) -> Result<(), Refusal>,
    ) -> Result<(), Refusal> {
        match key {
            Key::Computed(expression) => {
                self.expression(expression)?;
                value(self)?;
                self.chunk.emit(Op::DefineKeyed, at);
            }
            Key::Name(name) => {
                let text = self.text(name)?;
                value(self)?;
                self.chunk.emit(Op::DefineNamed(text), at);
            }
            Key::String(units) => {
                let text = self.units(units)?;
                value(self)?;
                self.chunk.emit(Op::DefineNamed(text), at);
            }
            Key::Number(number) => {
                let text = self.text(&crate::numeric::text_of(*number))?;
                value(self)?;
                self.chunk.emit(Op::DefineNamed(text), at);
            }
            Key::Private(_) => {
                return Err(Refusal::NotBuiltYet {
                    what: What::AConstruction,
                    at,
                });
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
                    if self.resolve(name, at)? == Where::Global {
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
                            what: What::AConstruction,
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
                match self.resolve(name, at)? {
                    // A declared binding cannot be deleted, and the language
                    // answers `false` rather than throwing.
                    Where::Local { .. } | Where::Binding { .. } => {
                        self.chunk.emit(Op::Bool(false), at);
                    }
                    Where::Captured => return Err(captured(at)),
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
            self.where_to_put(name, at)?
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
                what: What::AConstruction,
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
                what: What::AConstruction,
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

    // --- Names, slots and bindings ------------------------------------------

    /// Where a name lives, refusing the two this engine cannot compile.
    ///
    /// One is [`Where::Captured`]. The other is **`arguments`**, which inside a
    /// function is not a global at all but an object the call makes — so
    /// letting it resolve to the realm would turn *this engine has not built
    /// the arguments object* into *this page has a typo*, which is exactly the
    /// wrong answer that reads like a right one. Queue item 213.
    fn resolve(&self, name: &str, at: usize) -> Result<Where, Refusal> {
        let place = self.scopes.find(name);
        if place == Where::Captured {
            return Err(captured(at));
        }
        if place == Where::Global && !self.script && name == "arguments" {
            return Err(Refusal::NotBuiltYet {
                what: What::AParameterForm,
                at,
            });
        }
        Ok(place)
    }

    /// Where a name is written, given where it lives.
    fn put(&mut self, place: Where, name: &str, at: usize) -> Result<Put, Refusal> {
        Ok(match place {
            Where::Local {
                slot,
                assignment: Assignment::Allowed,
            } => Put::Slot(slot),
            Where::Binding {
                hops,
                slot,
                assignment: Assignment::Allowed,
            } => Put::Binding {
                hops,
                slot,
                name: self.text(name)?,
            },
            Where::Local {
                assignment: Assignment::Refused,
                ..
            }
            | Where::Binding {
                assignment: Assignment::Refused,
                ..
            } => Put::Constant(self.text(name)?),
            Where::Local {
                assignment: Assignment::Ignored,
                ..
            }
            | Where::Binding {
                assignment: Assignment::Ignored,
                ..
            } => Put::Ignored,
            Where::Captured => return Err(captured(at)),
            Where::Global => Put::Global(self.text(name)?),
        })
    }

    /// Where a name is written, refusing a name no function may reach.
    fn where_to_put(&mut self, name: &str, at: usize) -> Result<Put, Refusal> {
        let place = self.resolve(name, at)?;
        self.put(place, name, at)
    }

    /// Read a name, answering where writing it back would go.
    fn load_name(&mut self, name: &str, at: usize) -> Result<Put, Refusal> {
        let place = self.resolve(name, at)?;
        match place {
            Where::Local { slot, .. } => {
                self.chunk.emit(Op::Load(slot), at);
            }
            Where::Binding { hops, slot, .. } => {
                let text = self.text(name)?;
                let pc = self.chunk.emit(Op::LoadBinding { hops, slot }, at);
                self.chunk.name_instruction(pc, text);
            }
            Where::Captured => return Err(captured(at)),
            Where::Global => {
                let text = self.text(name)?;
                self.chunk.emit(Op::LoadGlobal(text), at);
            }
        }
        self.put(place, name, at)
    }

    /// Write the value on the stack back to a name, leaving it there.
    fn store_name(&mut self, put: Put, at: usize) {
        match put {
            Put::Slot(slot) => {
                self.chunk.emit(Op::Store(slot), at);
            }
            Put::Binding { hops, slot, name } => {
                let pc = self.chunk.emit(Op::StoreBinding { hops, slot }, at);
                self.chunk.name_instruction(pc, name);
            }
            Put::Global(text) => {
                self.chunk.emit(Op::StoreGlobal(text), at);
            }
            // A `const` this compiler can see: the value has been evaluated,
            // because the language evaluates it before it complains about it.
            Put::Constant(name) => {
                self.chunk.emit(Op::RefuseAssignment(name), at);
            }
            // A function expression's own name in sloppy code. The value was
            // evaluated and stays on the stack, because an assignment is an
            // expression whatever it did or did not store.
            Put::Ignored => {}
        }
    }

    /// The index of a name among the program's texts.
    fn text(&mut self, name: &str) -> Result<u32, Refusal> {
        let units: Vec<u16> = name.encode_utf16().collect();
        self.units(&units)
    }

    /// The same, for code units that are already what they are.
    fn units(&mut self, units: &[u16]) -> Result<u32, Refusal> {
        self.unit
            .text_index(units)
            .ok_or_else(|| Refusal::NotAProgram {
                why: "this program holds more distinct strings than an index can name".to_owned(),
                at: 0,
            })
    }

    /// Take a frame slot of the body being compiled.
    fn slot(&mut self, at: usize) -> Result<u32, Refusal> {
        self.chunk.take_slot().ok_or_else(|| Refusal::NotAProgram {
            why: "this program declares more bindings than a frame can hold".to_owned(),
            at,
        })
    }

    /// Take a binding of the environment the body being compiled is given.
    fn binding(&mut self, at: usize) -> Result<u32, Refusal> {
        self.chunk
            .take_binding()
            .ok_or_else(|| Refusal::NotAProgram {
                why: "this function declares more names than an environment can hold".to_owned(),
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
        ExpressionKind::Class(_)
        | ExpressionKind::New { .. }
        | ExpressionKind::Super
        | ExpressionKind::NewTarget
        | ExpressionKind::PrivateName(_) => What::AConstruction,
        ExpressionKind::TaggedTemplate { .. } => What::ATaggedTemplate,
        ExpressionKind::RegularExpression(_) => What::ARegularExpression,
        ExpressionKind::BigInt { .. } => What::ABigInt,
        ExpressionKind::Yield { .. } | ExpressionKind::Await(_) => What::ASuspension,
        _ => What::AModule,
    };
    Refusal::NotBuiltYet { what, at }
}

/// A name a function may not reach, because a block declared it outside.
fn captured(at: usize) -> Refusal {
    Refusal::NotBuiltYet {
        what: What::ACapturedBlockBinding,
        at,
    }
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
    /// A binding of an environment, with the name it was written as so that a
    /// dead zone can say which one.
    Binding {
        /// How many environments out.
        hops: u32,
        /// Which binding of it.
        slot: u32,
        /// Which text names it.
        name: u32,
    },
    /// A name the realm answers for, which decides at run time whether it is a
    /// lexical binding, a property of the global object, or nothing — and which
    /// refuses a `const` of its own.
    Global(u32),
    /// A `const` this compiler can see is one. It still has a place, because
    /// reading it is ordinary.
    Constant(u32),
    /// A named function expression's own name in sloppy code, where an
    /// assignment is specified to do nothing at all.
    Ignored,
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
    use crate::unit::Unit;

    fn unit(source: &str) -> Result<Unit, Refusal> {
        match script(source) {
            Ok(program) => compile(&program),
            Err(why) => panic!("{source} does not parse: {why}"),
        }
    }

    #[test]
    fn a_name_in_a_block_is_a_slot_and_one_outside_it_is_not() {
        let Ok(unit) = unit("{ let a = 1; a; } b;") else {
            panic!("that compiles");
        };
        let code = unit.script().code();
        assert!(code.contains(&Op::Load(0)), "the block's own name");
        assert!(
            code.iter().any(|op| matches!(op, Op::LoadGlobal(_))),
            "and the one nothing declares"
        );
    }

    #[test]
    fn a_functions_own_name_is_a_binding_and_the_function_is_a_chunk_of_its_own() {
        let Ok(unit) = unit("function f(a) { return a; } f(1);") else {
            panic!("that compiles");
        };
        assert_eq!(unit.chunks(), 2, "the script, and the function");
        let Some(inside) = unit.chunk(1) else {
            panic!("the function is chunk one");
        };
        assert_eq!(inside.parameters(), 1);
        assert_eq!(inside.bindings(), 1);
        assert!(
            inside
                .code()
                .contains(&Op::LoadBinding { hops: 0, slot: 0 }),
            "a parameter is a binding of its own environment"
        );
        assert!(
            unit.script().code().contains(&Op::Call(1)),
            "and the script calls it with one argument"
        );
    }

    #[test]
    fn what_is_not_built_says_which_item_builds_it() {
        for (source, what) in [
            ("new f()", What::AConstruction),
            ("class A {}", What::AConstruction),
            ("function f(a = 1) {}", What::AParameterForm),
            ("({ get a() { return 1; } })", What::AnAccessor),
            ("f`a`", What::ATaggedTemplate),
            (
                "{ let a = 1; (function () { return a; }); }",
                What::ACapturedBlockBinding,
            ),
            ("try { a; } catch {}", What::ACatch),
            ("[1, 2]", What::TakingAValueApart),
            ("for (const a of b) {}", What::TakingAValueApart),
            ("/a/", What::ARegularExpression),
            ("1n", What::ABigInt),
            ("function* f() {}", What::ASuspension),
        ] {
            match unit(source) {
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
            match unit(source) {
                Err(Refusal::NotAProgram { .. }) => {}
                other => panic!("{source} is not a program: {other:?}"),
            }
        }
    }
}
