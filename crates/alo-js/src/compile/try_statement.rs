/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `try`, `catch` and `finally`, and every way out of a body that a `finally`
//! stands in the way of (queue item 210).
//!
//! # A throw lands through a table; everything else lands through code
//!
//! A throw can come from any instruction — a property read, a call, a
//! conversion three frames down — so where it lands is a [`Handler`] in the
//! chunk rather than an instruction, and the interpreter finds it when
//! something is thrown. A `break`, a `continue` and a `return` are different:
//! the compiler can see each one, so each is compiled to go where it is going.
//!
//! # `finally` is the hard half, and it is a completion carried across a jump
//!
//! A `finally` runs on the way out of a `try` **whichever way** it left —
//! normally, by a throw, by a `return`, a `break` or a `continue` — and then
//! the leaving carries on as though the `finally` had not been there, unless
//! the `finally` itself left some other way. So the way out has to be
//! *remembered* across the block, and it is, in two frame slots the `try`
//! takes: which way it was leaving (a number) and the value it was leaving
//! with (what was thrown, or what was returned).
//!
//! Each `break`, `continue` and `return` inside the guarded blocks whose
//! target is outside them becomes: write its number into the first slot, leave
//! the blocks it was in, and jump into the `finally`. At the `finally`'s end
//! the slot is read and each way out is compiled **again, from there** — which
//! is how a `break` out of two nested `try`s runs both `finally`s, innermost
//! first: from the inner one's end, the same `break` meets the outer one.
//!
//! Numbers rather than one instruction per kind of completion, because a
//! number in a slot is a value the collector already understands, and because
//! the dispatch at the end is the language's own `===` and conditional jump
//! rather than an instruction that exists only for this.
//!
//! # What a script evaluates to survives a `finally`
//!
//! `try { 1 } finally { 2 }` evaluates to `1`: a `finally` that ends normally
//! leaves the completion its `try` had. So a script puts its completion aside
//! before the block ([`Op::Completion`]) and writes it back after. A `finally`
//! that leaves abruptly keeps its own, which is why the block also starts from
//! an empty completion rather than the `try`'s.
//!
//! # Why the interpreter may judge a throw by the instruction before
//!
//! It does not: a frame records the instruction it is running
//! ([`Frame::now`](crate::interpret)), and that is what the table is searched
//! with. Saying so here because the obvious shortcut — the program counter
//! less one — is wrong for exactly one kind of instruction, the one that
//! rewinds itself to run again once a `valueOf` has answered.

use crate::ast::{Binary, Catch, Pattern, Statement};
use crate::code::{Handler, Op};

use super::hoist;
use super::scope::Assignment;
use super::{Compiler, Refusal, What};

/// A `try` that left normally, or that has nothing to carry.
const NORMAL: u32 = 0;
/// A `try` that left by a throw, with the thrown value in the value slot.
const THROWN: u32 = 1;
/// The first number a `break`, `continue` or `return` is given.
const FIRST_EXIT: u32 = 2;

/// A way out of a body that the compiler can see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Exit {
    /// A `break` (or, `repeating`, a `continue`) to the `which`th thing that
    /// may be left.
    Jump {
        /// Which entry of the compiler's `enclosing` it leaves.
        which: usize,
        /// Whether it goes to that thing's next pass rather than past its end.
        repeating: bool,
    },
    /// A `return`, with the value it returns on the stack.
    Return,
}

/// A `finally` being compiled: where its way out is remembered, and the ways
/// out that have been routed into it.
#[derive(Debug)]
pub(super) struct Finally {
    /// The frame slot holding which way the guarded blocks were left.
    kind: u32,
    /// The frame slot holding what was thrown or returned.
    value: u32,
    /// How many environments were in force when the `try` began, which is how
    /// many are left when control reaches the `finally`.
    depth: usize,
    /// How many things a `break` may leave were open when the `try` began: one
    /// whose index is below this is outside the `try`, so leaving it passes
    /// through the `finally`.
    enclosing: usize,
    /// The ways out routed into this `finally`, each numbered by its place
    /// here plus [`FIRST_EXIT`].
    exits: Vec<Exit>,
    /// The jumps into the `finally`, waiting for where its block begins.
    entries: Vec<usize>,
}

impl Compiler {
    /// `try { … } catch (a) { … } finally { … }`, with either half missing.
    pub(super) fn try_statement(
        &mut self,
        block: &[Statement],
        handler: Option<&Catch>,
        finalizer: Option<&[Statement]>,
        at: usize,
    ) -> Result<(), Refusal> {
        // The statement's completion starts empty, as an `if`'s does:
        // `1; try {} catch {}` is `undefined`, not `1`.
        self.completes_empty(at);
        let depth = self.environments;
        let environments = u32::try_from(depth).map_err(|_| super::lost(at))?;

        let Some(body) = finalizer else {
            self.guarded(block, handler, false, environments, at)?;
            return Ok(());
        };
        let kind = self.slot(at)?;
        let value = self.slot(at)?;
        self.finallys.push(Finally {
            kind,
            value,
            depth,
            enclosing: self.enclosing.len(),
            exits: Vec::new(),
            entries: Vec::new(),
        });
        let outcome = self.guarded(block, handler, true, environments, at);
        // Popped whether or not the blocks compiled, so a refusal leaves the
        // compiler as it found it.
        let Some(finally) = self.finallys.pop() else {
            return Err(super::lost(at));
        };
        let unguarded = outcome?;
        self.finally(&finally, unguarded, body, at)
    }

    /// The `try` block and the `catch`, answering the ranges a `finally` must
    /// catch a throw from — none when there is no `finally`.
    fn guarded(
        &mut self,
        block: &[Statement],
        handler: Option<&Catch>,
        finally: bool,
        environments: u32,
        at: usize,
    ) -> Result<Vec<(usize, usize)>, Refusal> {
        let start = self.chunk.here();
        self.block(block, at)?;
        let end = self.chunk.here();
        let mut unguarded = Vec::new();

        let Some(handler) = handler else {
            // `try … finally`: a throw from the block goes to the `finally`.
            self.leave_into_finally(at)?;
            unguarded.push((start, end));
            return Ok(unguarded);
        };

        // Leaving the block normally goes past the `catch`. The instruction at
        // `end` is this jump, or the one into the `finally`: never one that
        // can throw.
        let over = if finally {
            self.leave_into_finally(at)?;
            None
        } else {
            Some(self.chunk.emit(Op::Jump(0), at))
        };
        let landing = self.chunk.here();
        // Recorded now, after every `try` inside the block recorded its own,
        // which is what makes the first range holding an instruction the
        // innermost.
        self.chunk.protect(Handler {
            start,
            end,
            landing,
            environments,
        });
        self.catch(handler, at)?;
        let caught_to = self.chunk.here();
        if let Some(over) = over {
            self.patch(over)?;
        } else {
            self.leave_into_finally(at)?;
            unguarded.push((landing, caught_to));
        }
        Ok(unguarded)
    }

    /// `catch (a) { … }`: the thrown value is on the stack when this begins.
    fn catch(&mut self, handler: &Catch, at: usize) -> Result<(), Refusal> {
        // The `catch` block's completion starts empty too, so that
        // `try { 1; throw 0 } catch {}` is `undefined` rather than `1`.
        self.completes_empty(at);
        let name = match &handler.parameter {
            None => None,
            Some(Pattern::Name(name)) => Some(name.as_str()),
            Some(_) => {
                return Err(Refusal::NotBuiltYet {
                    what: What::TakingAValueApart,
                    at,
                });
            }
        };
        let Some(name) = name else {
            // `catch { … }`: nothing looks at what was thrown.
            self.chunk.emit(Op::Pop, at);
            return self.block(&handler.body, at);
        };
        refuse_a_second_name(name, &handler.body, at)?;

        // The parameter is a scope of its own, and the block is another inside
        // it — two environments, as the specification has, so that a closure
        // made in the block sees the binding the `catch` was given.
        self.scopes.open_block();
        let outcome = self.catch_inside(name, &handler.body, at);
        self.scopes.close();
        outcome
    }

    /// The `catch` with its parameter's scope open.
    fn catch_inside(&mut self, name: &str, body: &[Statement], at: usize) -> Result<(), Refusal> {
        let slot = self.bindings_here(at)?;
        if self
            .scopes
            .declare(name, slot, Assignment::Allowed)
            .is_err()
        {
            return Err(super::lost(at));
        }
        // A safepoint, with the thrown value still on the stack.
        let opened = self.open_environment(at)?;
        if !opened {
            return Err(super::lost(at));
        }
        self.chunk.emit(Op::InitializeBinding { hops: 0, slot }, at);
        self.block(body, at)?;
        self.close_environment(opened, at);
        Ok(())
    }

    /// Leave a guarded block normally, into the `finally`.
    fn leave_into_finally(&mut self, at: usize) -> Result<(), Refusal> {
        let Some(kind) = self.finallys.last().map(|finally| finally.kind) else {
            return Err(super::lost(at));
        };
        self.chunk.emit(Op::Number(f64::from(NORMAL)), at);
        self.chunk.emit(Op::Initialize(kind), at);
        let jump = self.chunk.emit(Op::Jump(0), at);
        let Some(finally) = self.finallys.last_mut() else {
            return Err(super::lost(at));
        };
        finally.entries.push(jump);
        Ok(())
    }

    /// The `finally`: where a throw lands, the block itself, and then every
    /// way out that was routed into it, carried on from here.
    fn finally(
        &mut self,
        finally: &Finally,
        guarded: Vec<(usize, usize)>,
        body: &[Statement],
        at: usize,
    ) -> Result<(), Refusal> {
        let environments = u32::try_from(finally.depth).map_err(|_| super::lost(at))?;

        // A throw lands here with what was thrown on the stack.
        let landing = self.chunk.here();
        for (start, end) in guarded {
            self.chunk.protect(Handler {
                start,
                end,
                landing,
                environments,
            });
        }
        self.chunk.emit(Op::Initialize(finally.value), at);
        self.chunk.emit(Op::Number(f64::from(THROWN)), at);
        self.chunk.emit(Op::Initialize(finally.kind), at);

        // Every other way in jumps to the block itself.
        for jump in &finally.entries {
            self.patch(*jump)?;
        }
        let kept = if self.script {
            let kept = self.slot(at)?;
            self.chunk.emit(Op::Completion, at);
            self.chunk.emit(Op::Initialize(kept), at);
            self.chunk.emit(Op::CompleteEmpty, at);
            Some(kept)
        } else {
            None
        };
        self.block(body, at)?;
        if let Some(kept) = kept {
            // It ended normally, so the `try`'s completion stands.
            self.chunk.emit(Op::Load(kept), at);
            self.chunk.emit(Op::Complete, at);
        }

        // Then carry on leaving the way the guarded blocks were leaving.
        for (number, exit) in (FIRST_EXIT..).zip(finally.exits.iter().copied()) {
            let next = self.when_kind_is(finally.kind, number, at);
            if exit == Exit::Return {
                self.chunk.emit(Op::Load(finally.value), at);
            }
            self.exit(exit, at)?;
            self.patch(next)?;
        }
        let normal = self.when_kind_is(finally.kind, THROWN, at);
        self.chunk.emit(Op::Load(finally.value), at);
        self.chunk.emit(Op::Throw, at);
        self.patch(normal)
    }

    /// Test the way-out slot against a number, answering the jump to patch to
    /// wherever the code for a different answer begins.
    fn when_kind_is(&mut self, kind: u32, number: u32, at: usize) -> usize {
        self.chunk.emit(Op::Load(kind), at);
        self.chunk.emit(Op::Number(f64::from(number)), at);
        self.chunk.emit(Op::Binary(Binary::StrictlyEqual), at);
        self.chunk.emit(Op::JumpIfFalse(0), at)
    }

    /// Leave by `break`, `continue` or `return` — through a `finally` if one
    /// is between here and where it is going.
    ///
    /// For a `return` the value is on the stack.
    pub(super) fn exit(&mut self, exit: Exit, at: usize) -> Result<(), Refusal> {
        let crossing = self.finallys.last().is_some_and(|finally| match exit {
            Exit::Jump { which, .. } => which < finally.enclosing,
            Exit::Return => true,
        });
        if !crossing {
            return match exit {
                Exit::Jump { which, repeating } => self.jump_out(which, repeating, at),
                Exit::Return => {
                    self.chunk.emit(Op::Return, at);
                    Ok(())
                }
            };
        }
        let Some(finally) = self.finallys.last_mut() else {
            return Err(super::lost(at));
        };
        // The same way out twice is one number, so the dispatch at the end is
        // as long as the number of *different* ways out rather than of places
        // that write one.
        let index = if let Some(index) = finally.exits.iter().position(|had| *had == exit) {
            index
        } else {
            finally.exits.push(exit);
            finally.exits.len().saturating_sub(1)
        };
        let number = u32::try_from(index)
            .ok()
            .and_then(|index| index.checked_add(FIRST_EXIT))
            .ok_or_else(|| super::too_long(at))?;
        let (kind, value, depth) = (finally.kind, finally.value, finally.depth);
        if exit == Exit::Return {
            self.chunk.emit(Op::Initialize(value), at);
        }
        self.chunk.emit(Op::Number(f64::from(number)), at);
        self.chunk.emit(Op::Initialize(kind), at);
        // The blocks between here and the `try` are left now, because the
        // `finally` runs in the environment the `try` began in.
        for _ in depth..self.environments {
            self.chunk.emit(Op::PopEnvironment, at);
        }
        let jump = self.chunk.emit(Op::Jump(0), at);
        let Some(finally) = self.finallys.last_mut() else {
            return Err(super::lost(at));
        };
        finally.entries.push(jump);
        Ok(())
    }
}

/// `catch (e) { let e; }` and `catch (e) { var e; }` are not programs.
///
/// The first is the specification's early error and is not a choice: a
/// second binding of the same name in the block would shadow the one the
/// `catch` was given, and the program would run and be wrong. The second is
/// the same rule for a `var`, which Annex B relaxes for old pages — that
/// relaxation is the legacy tail (queue item 142), opened by a page that needs
/// it rather than granted in advance.
fn refuse_a_second_name(name: &str, body: &[Statement], at: usize) -> Result<(), Refusal> {
    let lexical = hoist::lexical(body).into_iter().map(|one| one.name);
    let functions = hoist::functions(body)
        .into_iter()
        .filter_map(|function| function.name.clone());
    let mut vars = Vec::new();
    hoist::vars(body, &mut vars);
    if lexical.chain(functions).chain(vars).any(|had| had == name) {
        return Err(Refusal::NotAProgram {
            why: format!("'{name}' is the catch's own name and is declared again in its block"),
            at,
        });
    }
    Ok(())
}
