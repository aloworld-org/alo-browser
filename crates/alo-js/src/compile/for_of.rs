/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `for (a of b)`: the iteration protocol, compiled (queue item 230).
//!
//! # It is calls a page can see, not an instruction that hides them
//!
//! `GetIterator` reads `b[Symbol.iterator]` and calls it; every pass calls the
//! iterator's `next` and reads `done` and `value` from what it answers. Each of
//! those is a property read or a call that a page can intercept — a `next` of
//! its own, a `done` behind a getter — so each is compiled to the ordinary
//! instruction for it, in the specification's order, and the interpreter
//! learns nothing new beyond three checks ([`Op::WellKnown`], [`Op::Iterable`]
//! and [`Op::RequireObject`]). The iterator and its `next` are kept in two
//! frame slots: `next` is read **once**, before the first pass, as
//! `GetIterator` reads it.
//!
//! # Leaving early closes the iterator, and how depends on why
//!
//! An iterator is told it will not be asked again — its `return` is called —
//! whenever the loop ends before the iterator said `done`:
//!
//! - By `break`, by `return`, or by a `continue` of a loop outside this one,
//!   the closing is a [`Finally`] the loop pushes, and those ways out are
//!   routed into it exactly as a `finally` routes them. `return` is called,
//!   anything it throws is thrown, and an answer that is not an object is a
//!   `TypeError`; then the way out carries on.
//! - By a throw from the body, the closing is where a [`Handler`] lands.
//!   `return` is called and **everything it does is ignored** — a throw from
//!   it, or an answer that is not an object — and the original throw carries
//!   on. A second handler, around the call, is what ignores it.
//!
//! A throw from the iterator itself — from `next`, or from reading `done` or
//! `value` — closes nothing: the iterator is the thing that failed. That is
//! why the guarded range begins *after* `value` is read.
//!
//! # `let` and `const` are a binding per pass, and the head has a dead zone
//!
//! `for (const x of list) fns.push(() => x)` makes functions that see
//! different values, so each pass is an environment of its own. And `b` is
//! evaluated with the head's names already declared and uninitialised, so
//! `for (const x of x)` is a `ReferenceError` rather than a read of an outer
//! `x` — one more environment, made and left around `b` alone.
//!
//! # What is refused, and by whom
//!
//! A destructuring head (`for (const [k, v] of …)`) is a pattern, queue item
//! 211's. `for await` is queue item 75's. Neither is approximated.

use crate::ast::{
    DeclarationKind, Expression, ExpressionKind, ForTarget, Member, Pattern, Statement,
};
use crate::code::{Expecting, Handler, Op};
use crate::object::symbol::WellKnown;

use super::scope::Assignment;
use super::try_statement::Finally;
use super::{Compiler, Refusal, What};

/// What each value the iterator hands out is given to.
#[derive(Debug, Clone, Copy)]
enum Head<'a> {
    /// `for (let a of b)` and `for (const a of b)`: a binding of a fresh
    /// environment each pass, at this binding of it.
    Lexical(u32),
    /// `for (var a of b)` and `for (a of b)`: a name, assigned each pass.
    Name(&'a str),
    /// `for (a.b of c)` and `for (a[b] of c)`: a property, assigned each pass.
    Member(&'a Expression),
}

/// The head as written, before anything about it is declared.
enum Written<'a> {
    /// `let` or `const`, and whether it may be assigned to.
    Lexical(&'a str, bool),
    /// Anything that assigns.
    Assigned(Head<'a>),
}

impl Compiler {
    /// `for (a of b) c`, with the label in front of it if there is one.
    pub(super) fn for_of(
        &mut self,
        left: &ForTarget,
        right: &Expression,
        is_await: bool,
        body: &Statement,
        at: usize,
        label: Option<&str>,
    ) -> Result<(), Refusal> {
        if is_await {
            return Err(Refusal::NotBuiltYet {
                what: What::ASuspension,
                at,
            });
        }
        let written = written(left, at)?;
        self.completes_empty(at);
        match written {
            Written::Lexical(name, mutable) => {
                self.scopes.open_block();
                let outcome = self.lexically(name, mutable, right, body, at, label);
                self.scopes.close();
                outcome
            }
            Written::Assigned(head) => {
                self.expression(right)?;
                self.iterate(head, body, at, label)
            }
        }
    }

    /// The `let` and `const` forms, with the head's scope open.
    fn lexically(
        &mut self,
        name: &str,
        mutable: bool,
        right: &Expression,
        body: &Statement,
        at: usize,
        label: Option<&str>,
    ) -> Result<(), Refusal> {
        let slot = self.bindings_here(at)?;
        let assignment = if mutable {
            Assignment::Allowed
        } else {
            Assignment::Refused
        };
        if self.scopes.declare(name, slot, assignment).is_err() {
            // The scope was opened for this one name.
            return Err(super::lost(at));
        }
        // The dead zone `b` is evaluated in: the head's own name, declared and
        // never initialised, in an environment of its own.
        let opened = self.open_environment(at)?;
        self.expression(right)?;
        self.close_environment(opened, at);
        self.iterate(Head::Lexical(slot), body, at, label)
    }

    /// `GetIterator` on the value on the stack, then the loop.
    fn iterate(
        &mut self,
        head: Head<'_>,
        body: &Statement,
        at: usize,
        label: Option<&str>,
    ) -> Result<(), Refusal> {
        let iterator = self.slot(at)?;
        let next = self.slot(at)?;
        let next_name = self.text("next")?;
        let done_name = self.text("done")?;
        let value_name = self.text("value")?;

        // `method = GetMethod(b, @@iterator)`, then `Call(method, b)`. The
        // iterable waits in the iterator's slot until the iterator replaces it.
        self.chunk.emit(Op::Initialize(iterator), at);
        self.chunk.emit(Op::Load(iterator), at);
        self.chunk.emit(Op::WellKnown(WellKnown::Iterator), at);
        self.chunk.emit(Op::GetKeyed, at);
        self.chunk.emit(Op::Iterable, at);
        self.chunk.emit(Op::Load(iterator), at);
        self.chunk.emit(Op::Call(0), at);
        self.chunk
            .emit(Op::RequireObject(Expecting::AnIterator), at);
        self.chunk.emit(Op::Dup, at);
        self.chunk.emit(Op::Initialize(iterator), at);
        self.chunk.emit(Op::GetNamed(next_name), at);
        self.chunk.emit(Op::Initialize(next), at);

        // A pass: `next()`, then `done`, then `value`.
        let top = self.chunk.here();
        self.chunk.emit(Op::Load(next), at);
        self.chunk.emit(Op::Load(iterator), at);
        self.chunk.emit(Op::Call(0), at);
        self.chunk.emit(Op::RequireObject(Expecting::AResult), at);
        self.chunk.emit(Op::Dup, at);
        self.chunk.emit(Op::GetNamed(done_name), at);
        let more = self.chunk.emit(Op::JumpIfFalse(0), at);
        self.chunk.emit(Op::Pop, at);
        let exhausted = self.chunk.emit(Op::Jump(0), at);
        self.patch(more)?;
        self.chunk.emit(Op::GetNamed(value_name), at);

        // The body, guarded: a `break` or `return` out of it closes the
        // iterator through `closing`, and a throw through the handler below.
        let depth = self.environments;
        self.enter(label, true);
        let own = self.enclosing.len().saturating_sub(1);
        let kind = self.slot(at)?;
        let value = self.slot(at)?;
        self.finallys
            .push(Finally::closing(kind, value, depth, own));
        let start = self.chunk.here();
        let outcome = self.pass(head, body, at);
        let end = self.chunk.here();
        let Some(closing) = self.finallys.pop() else {
            return Err(super::lost(at));
        };
        if let Err(refusal) = outcome {
            self.enclosing.pop();
            return Err(refusal);
        }
        let Ok(back) = u32::try_from(top) else {
            return Err(super::too_long(at));
        };
        self.chunk.emit(Op::Jump(back), at);

        let environments = u32::try_from(depth).map_err(|_| super::lost(at))?;
        self.closed_by_a_throw(iterator, closing.value(), start, end, environments, at)?;
        if closing.routed() {
            self.route_here(&closing)?;
            self.close(iterator, at)?;
            // The way out goes on from here; nothing reaches the fall-through,
            // since every way in wrote one of the ways out it tests for.
            self.carry_on(&closing, at)?;
        }
        self.patch(exhausted)?;
        self.finish_loop(top, at)
    }

    /// One pass's binding and body.
    fn pass(&mut self, head: Head<'_>, body: &Statement, at: usize) -> Result<(), Refusal> {
        match head {
            Head::Lexical(slot) => {
                let opened = self.open_environment(at)?;
                if !opened {
                    return Err(super::lost(at));
                }
                self.chunk.emit(Op::InitializeBinding { hops: 0, slot }, at);
                self.statement(body)?;
                self.close_environment(opened, at);
            }
            Head::Name(name) => {
                let put = self.where_to_put(name, at)?;
                self.store_name(put, at);
                self.chunk.emit(Op::Pop, at);
                self.statement(body)?;
            }
            Head::Member(target) => {
                self.assign_each(target, at)?;
                self.statement(body)?;
            }
        }
        Ok(())
    }

    /// `a.b = value` or `a[b] = value`, where the value is already on the
    /// stack and `a` and `b` are evaluated now — after it, as `ForIn/OfBody
    /// Evaluation` evaluates the head on every pass.
    fn assign_each(&mut self, target: &Expression, at: usize) -> Result<(), Refusal> {
        let ExpressionKind::Member {
            object,
            member,
            optional: false,
        } = &target.kind
        else {
            return Err(Refusal::NotAProgram {
                why: "this is not something a value can be assigned to".to_owned(),
                at,
            });
        };
        let held = self.slot(at)?;
        self.chunk.emit(Op::Initialize(held), at);
        self.expression(object)?;
        if let Member::Computed(key) = member {
            self.expression(key)?;
        }
        self.chunk.emit(Op::Load(held), at);
        self.write_member(member, at)?;
        self.chunk.emit(Op::Pop, at);
        Ok(())
    }

    /// Where a throw from the body lands: close the iterator, ignoring
    /// anything the closing does, and throw on what was thrown.
    fn closed_by_a_throw(
        &mut self,
        iterator: u32,
        thrown: u32,
        start: usize,
        end: usize,
        environments: u32,
        at: usize,
    ) -> Result<(), Refusal> {
        let landing = self.chunk.here();
        self.chunk.protect(Handler {
            start,
            end,
            landing,
            environments,
        });
        self.chunk.emit(Op::Initialize(thrown), at);

        let guarded = self.chunk.here();
        let return_name = self.text("return")?;
        self.chunk.emit(Op::Load(iterator), at);
        self.chunk.emit(Op::GetNamed(return_name), at);
        let there = self.chunk.emit(Op::JumpIfNotNullishKeep(0), at);
        let absent = self.chunk.emit(Op::Jump(0), at);
        self.patch(there)?;
        self.chunk.emit(Op::Load(iterator), at);
        self.chunk.emit(Op::Call(0), at);
        self.chunk.emit(Op::Pop, at);
        let called = self.chunk.emit(Op::Jump(0), at);
        let unguarded = self.chunk.here();

        // A throw from reading `return` or from calling it is dropped.
        let ignored = self.chunk.here();
        self.chunk.protect(Handler {
            start: guarded,
            end: unguarded,
            landing: ignored,
            environments,
        });
        self.chunk.emit(Op::Pop, at);

        self.patch(absent)?;
        self.patch(called)?;
        self.chunk.emit(Op::Load(thrown), at);
        self.chunk.emit(Op::Throw, at);
        Ok(())
    }

    /// `IteratorClose` for a way out that is not a throw: `return` is called if
    /// there is one, and what it throws or answers wrongly is not ignored.
    fn close(&mut self, iterator: u32, at: usize) -> Result<(), Refusal> {
        let return_name = self.text("return")?;
        self.chunk.emit(Op::Load(iterator), at);
        self.chunk.emit(Op::GetNamed(return_name), at);
        let there = self.chunk.emit(Op::JumpIfNotNullishKeep(0), at);
        let absent = self.chunk.emit(Op::Jump(0), at);
        self.patch(there)?;
        self.chunk.emit(Op::Load(iterator), at);
        self.chunk.emit(Op::Call(0), at);
        self.chunk
            .emit(Op::RequireObject(Expecting::AClosedResult), at);
        self.chunk.emit(Op::Pop, at);
        self.patch(absent)
    }
}

/// Read the head as written: a name to bind or assign, or something not built.
fn written(left: &ForTarget, at: usize) -> Result<Written<'_>, Refusal> {
    match left {
        ForTarget::Declaration(declaration) => {
            let [declarator] = declaration.declarators.as_slice() else {
                return Err(Refusal::NotAProgram {
                    why: "a `for…of` head declares exactly one thing".to_owned(),
                    at,
                });
            };
            if declarator.init.is_some() {
                return Err(Refusal::NotAProgram {
                    why: "a `for…of` head may not give its name a value".to_owned(),
                    at,
                });
            }
            let Pattern::Name(name) = &declarator.pattern else {
                return Err(Refusal::NotBuiltYet {
                    what: What::TakingAValueApart,
                    at,
                });
            };
            Ok(match declaration.kind {
                DeclarationKind::Var => Written::Assigned(Head::Name(name)),
                DeclarationKind::Let => Written::Lexical(name, true),
                DeclarationKind::Const => Written::Lexical(name, false),
            })
        }
        ForTarget::Target(Pattern::Name(name)) => Ok(Written::Assigned(Head::Name(name))),
        ForTarget::Target(Pattern::Member(target)) => Ok(Written::Assigned(Head::Member(target))),
        ForTarget::Target(Pattern::Array { .. } | Pattern::Object { .. }) => {
            Err(Refusal::NotBuiltYet {
                what: What::TakingAValueApart,
                at,
            })
        }
    }
}
