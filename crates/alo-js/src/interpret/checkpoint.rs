/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The two things an event loop asks of the engine: **run this call**, and
//! **drain the job queue** (ADR 0016 § 1, queue item 232).
//!
//! The loop itself — task queues, which task runs next, frames — is the
//! renderer's, and this crate gains no notion of a task. What it gains is the
//! one piece of the loop that has to be in here: the [`Jobs`](crate::job::Jobs)
//! a promise reaction or a `queueMicrotask` callback waits in, and the
//! checkpoint that runs them.
//!
//! # A call with nothing running
//!
//! [`Engine::call`] is how an embedder runs a function it was handed — a
//! listener, a timer's callback, a job. It is a run like [`Engine::run`] with
//! no script frame at the bottom: the callee, its `this` and its arguments are
//! laid out on a fresh stack in the shape every call has, and entered. When
//! the last frame returns, the answer is where the callee stood.
//!
//! # The checkpoint
//!
//! [`Engine::checkpoint`] runs jobs, oldest first, until none is left —
//! **including jobs queued by the jobs it is running**, which is what makes a
//! promise chain finish within one checkpoint. Then it ends the job:
//! [`Heap::end_job`](crate::heap::Heap::end_job), the specification's
//! `ClearKeptObjects`, which lets go of what a dereferenced `WeakRef` was
//! keeping alive.
//!
//! A job that **throws** is reported and the next one runs: HTML's *report
//! the exception*, which a page's other jobs do not see. The report is handed
//! to the embedder as it happens, before anything else allocates, because a
//! thrown value is a reference that nothing roots once its job has gone.
//!
//! Anything else that ends a job — the embedder's [`Stop`](super::Stop), a
//! full heap, a thing this engine has not built, its own bug — ends the
//! checkpoint, and **the queue is emptied** rather than kept: ADR 0016 § 7, a
//! page stopped half way through is in a state no script on it expected, and
//! running its next job would be inventing a continuation.
//!
//! # It never nests, and the borrow is the flag
//!
//! HTML guards the checkpoint with a flag so that a job which calls into
//! script does not start a second checkpoint inside the first. Here a
//! checkpoint holds `&mut Engine` from its first job to its last, a builtin is
//! handed no engine (see [`native`](crate::object::native)), and the report is
//! handed the object model read-only — so there is no way to call
//! [`Engine::checkpoint`] while one is running, and nothing to check.

use crate::abrupt::{Escape, Internal, Thrown};
use crate::heap::Root;
use crate::object::{Held, Objects, Value};

use super::Engine;
use super::frame::{After, Run};

/// What a checkpoint did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Drained {
    /// How many jobs ran, including those that threw.
    pub ran: usize,
    /// How many of them threw, each reported as it did.
    pub threw: usize,
}

impl Engine {
    /// Call `callee` with `this` and `arguments`, with nothing else running,
    /// and answer what it returned.
    ///
    /// The values must be reachable from a [`Root`] the caller holds, as any
    /// value an embedder keeps must be (ADR 0014 § 2): making the run's stack
    /// is an allocation, and only after it are they on the stack.
    ///
    /// # Errors
    ///
    /// [`Escape`]: the callee is not a function or threw, the engine reached
    /// something it has not built, the heap filled, the embedder stopped it,
    /// or this engine has a bug.
    pub fn call(
        &mut self,
        callee: Value,
        this: Value,
        arguments: &[Value],
    ) -> Result<Value, Escape> {
        let (stack, constants) = self.two_lists()?;
        let outcome = self.held(&stack).and_then(|list| {
            self.objects
                .with_slots(list, |slots, _| {
                    slots.push(callee);
                    slots.push(this);
                    for argument in arguments {
                        slots.push(*argument);
                    }
                })
                .ok_or(Escape::Broken(Internal::StackIsWrong))?;
            self.call_laid_out(&stack, &constants, arguments.len())
        });
        self.finish(stack, constants, outcome)
    }

    /// Queue a call of `callee` with `arguments` as a job, behind every job
    /// already waiting — what `queueMicrotask` asks for, and what a promise
    /// reaction will be (ADR 0016 § 1).
    ///
    /// Nothing runs now. The values must be rooted by the caller, for
    /// [`Engine::call`]'s reason; from here on the queue holds them.
    ///
    /// # Errors
    ///
    /// The `TypeError` `queueMicrotask` throws when `callee` is not a
    /// function, at `at`; [`Escape::Broken`] if the queue has lost its list.
    pub fn queue_job(
        &mut self,
        callee: Value,
        arguments: &[Value],
        at: usize,
    ) -> Result<(), Escape> {
        let callable = match callee {
            Value::Object(held) => self.objects.callable(held).is_some(),
            _ => false,
        };
        if !callable {
            return Err(Escape::type_error(
                format!(
                    "{} is not a function, and only a function can be queued",
                    self.describe(callee)
                ),
                at,
            ));
        }
        self.jobs.push(&mut self.objects, callee, arguments)
    }

    /// How many jobs are waiting for the next checkpoint.
    pub fn jobs_waiting(&self) -> usize {
        self.jobs.len()
    }

    /// Perform a microtask checkpoint: run every job, oldest first, until none
    /// is left, then end the job.
    ///
    /// `report` is told of each job that threw, as it throws, and is handed
    /// the object model read-only so that it can describe what was thrown and
    /// cannot run anything.
    ///
    /// # Errors
    ///
    /// Any [`Escape`] other than a throw — [`Escape::Interrupted`],
    /// [`Escape::Full`], [`Escape::NotBuiltYet`], [`Escape::Broken`] — which
    /// ends the checkpoint with the remaining jobs dropped (ADR 0016 § 7). The
    /// job is ended either way.
    pub fn checkpoint(
        &mut self,
        report: &mut dyn FnMut(&Objects, &Thrown),
    ) -> Result<Drained, Escape> {
        match self.drain(report) {
            Ok(drained) => {
                self.objects.heap_mut().end_job();
                Ok(drained)
            }
            Err(escape) => self.abandon().and(Err(escape)),
        }
    }

    /// Drop every job waiting, run none of them, and end the job.
    ///
    /// What a loop does when a **task** ended some way other than a throw —
    /// stopped, a full heap, a thing this engine has not built — and the page
    /// is stopped with it (ADR 0016 § 7). A checkpoint does the same for
    /// itself; this is the same rule for the run that came before one, where
    /// a checkpoint would be the wrong thing to call because it would run the
    /// jobs.
    ///
    /// # Errors
    ///
    /// [`Escape::Broken`] if the queue has lost its list. The job is ended
    /// either way.
    pub fn abandon(&mut self) -> Result<(), Escape> {
        let outcome = self.jobs.clear(&mut self.objects);
        self.objects.heap_mut().end_job();
        outcome
    }

    /// Run jobs until none is left.
    fn drain(&mut self, report: &mut dyn FnMut(&Objects, &Thrown)) -> Result<Drained, Escape> {
        let mut drained = Drained::default();
        while !self.jobs.is_empty() {
            // Every job is a call, and a call reads the switch — but a job
            // whose callee is a builtin enters no frame, and a queue of those
            // would otherwise never look.
            if self.stop.asked() {
                return Err(Escape::Interrupted);
            }
            drained.ran = drained.ran.saturating_add(1);
            match self.run_job() {
                Ok(()) => {}
                Err(Escape::Thrown(thrown)) => {
                    drained.threw = drained.threw.saturating_add(1);
                    report(&self.objects, &thrown);
                }
                Err(escape) => return Err(escape),
            }
        }
        Ok(drained)
    }

    /// Run the oldest job.
    ///
    /// What it returns goes nowhere — a job's answer is nobody's — so unlike
    /// [`Engine::call`] it is not kept, and the value the embedder's last run
    /// answered stays kept across the checkpoint.
    fn run_job(&mut self) -> Result<(), Escape> {
        // The lists first: the job's values are in the queue, which is rooted,
        // until they are moved on to the stack with nothing allocated between.
        let (stack, constants) = self.two_lists()?;
        let outcome = self.held(&stack).and_then(|list| {
            match self.jobs.take_onto(&mut self.objects, list)? {
                Some(argc) => self.call_laid_out(&stack, &constants, argc),
                None => Err(Escape::Broken(Internal::StackIsWrong)),
            }
        });
        self.objects.heap_mut().release(stack);
        self.objects.heap_mut().release(constants);
        outcome.map(|_| ())
    }

    /// Enter the call laid out at the bottom of `stack` and walk it to its end.
    fn call_laid_out(
        &mut self,
        stack: &Root,
        constants: &Root,
        argc: usize,
    ) -> Result<Value, Escape> {
        let mut run = Run {
            stack: self.held(stack)?,
            constants: self.held(constants)?,
            units: Vec::new(),
            frames: Vec::new(),
            builtins: Vec::new(),
        };
        let outcome = self
            .enter_at(&mut run, 0, argc, 0, After::Answer)
            .and_then(|()| self.walk(&mut run));
        self.let_go(&mut run);
        outcome?;
        // The last frame to return left its answer where the callee stood.
        match self.objects.slot(run.stack, 0) {
            Some(Held::Value(value)) => Ok(value),
            Some(Held::Uninitialized) | None => Err(Escape::Broken(Internal::StackIsWrong)),
        }
    }
}
