/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The event loop: which task runs next, and the microtask checkpoint after
//! every piece of script (ADR 0016, queue item 235).
//!
//! # Whose it is
//!
//! ADR 0016 § 1. The loop — the task queue, the choice of the next task, and
//! later the rendering steps — is the renderer's, because it knows about
//! messages, documents and frames and the engine knows about none of them.
//! The engine owns exactly one piece of it: the **job queue**, in its heap,
//! which `queueMicrotask` and a promise reaction both wait in. So this module
//! asks [`Engine`] for two things only — *run this call*, *drain the jobs* —
//! and installs `queueMicrotask` as a function that asks the engine to queue
//! one ([`microtask`]).
//!
//! # A turn
//!
//! [`EventLoop::run_next`] takes the **oldest** task (ADR 0016 § 2 — one
//! sequence number across everything, [`task`]) and runs it. A task is one or
//! more pieces of script: a classic script's text, or a list of callees called
//! one after another with the same arguments, as a dispatch from the browser
//! process calls each listener in turn. **After every piece, the loop performs
//! a microtask checkpoint** (§ 3), because each one leaves the engine with
//! nothing running — which is why two listeners on a button a *person* clicked
//! see each other's microtasks run between them, and the same two called by a
//! script's `element.click()` do not.
//!
//! A throw nothing caught is [reported](Report) and the loop runs on. Anything
//! else that ends a piece of script — the embedder's [`Stop`], a full heap, a
//! thing the engine has not built, its bug — **stops the page** (§ 7): every
//! task waiting is dropped and its roots released, the job queue is emptied,
//! and nothing the page queued runs again. Reloading is the person's to ask
//! for.
//!
//! # The quiet point
//!
//! ADR 0016 § 4. Between one task's last checkpoint and the next task, no
//! script is running, no native code holds a scope and the job has ended, so
//! the keep-alive set is empty: *everything live is a root or reachable from
//! one* is trivially true. The loop checks that it is, after every task, and a
//! loop that finds it is not has found a bug in itself or in the engine — so
//! it stops the page rather than run the next task on a heap it cannot vouch
//! for. Two things will happen here and do not yet, each owed by the item that
//! makes it possible: asking for a collection when the loop has a reason to,
//! and queueing a `FinalizationRegistry`'s cleanups as tasks (item 73 builds
//! the registry).
//!
//! # What is not here yet
//!
//! **Nothing in a page queues a task yet.** A timer firing is item 92, an
//! event dispatched to listeners is item 81 and a response is item 83; each
//! will queue through [`EventLoop::queue_calls`]. And the [`Renderer`]
//! does not hold a loop yet: its messages becoming tasks, an `Act` answered
//! only after its task's checkpoint, and the scripts a page carries are queue
//! item 233. A ceiling on waiting tasks (ADR 0016's *the numbers*) arrives with
//! the first task a page can queue for itself; today every task is queued by
//! the renderer.
//!
//! [`Renderer`]: crate::Renderer

mod microtask;
mod report;
mod task;

use core::fmt;

use alo_js::interpret::{Engine, Stop, Trouble};
use alo_js::{Escape, Value, script};

pub use report::Report;
pub use task::Seq;
use task::{Tasks, Work};

/// Why a page's loop has stopped for good.
#[derive(Debug, Clone, PartialEq)]
pub enum Stopped {
    /// A piece of script, or a job, ended some way other than a throw: the
    /// embedder's [`Stop`], a full heap, something the engine has not built,
    /// or the engine's own bug.
    Escaped(Escape),
    /// The quiet point between two tasks was not quiet — a scope still open,
    /// or something still kept for a job that had ended. A bug, in the loop
    /// or in the engine.
    NotQuiet {
        /// References still held by open scopes.
        scoped: usize,
        /// References still in the job's keep-alive set.
        kept: usize,
    },
}

impl fmt::Display for Stopped {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Stopped::Escaped(escape) => write!(out, "the page stopped: {escape}"),
            Stopped::NotQuiet { scoped, kept } => write!(
                out,
                "the page stopped: between tasks, {scoped} references were still in scopes \
                 and {kept} still kept for an ended job"
            ),
        }
    }
}

/// What one task did.
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    /// Which task it was.
    pub task: Seq,
    /// Script that did not finish, in the order it happened.
    pub reports: Vec<Report>,
    /// How many jobs ran in the checkpoints after its pieces of script.
    pub jobs: usize,
    /// Why the page stopped, if this task stopped it.
    pub stopped: Option<Stopped>,
}

/// A page's event loop, and the engine its script runs in.
#[derive(Debug)]
pub struct EventLoop {
    engine: Engine,
    tasks: Tasks,
    /// The engine's switch, read before every piece of script: a page asked to
    /// stop while nothing was running stops before the next thing does.
    stop: Stop,
    stopped: Option<Stopped>,
}

impl EventLoop {
    /// A loop with nothing waiting, over an engine with `queueMicrotask` on its
    /// global object.
    ///
    /// # Errors
    ///
    /// [`Escape::Full`] if the heap cannot hold a realm, and
    /// [`Escape::Broken`] if the engine could not be given `queueMicrotask`,
    /// which is its bug.
    pub fn new() -> Result<Self, Escape> {
        let mut engine = Engine::new()?;
        microtask::install(&mut engine)?;
        let stop = engine.stop();
        Ok(Self {
            engine,
            tasks: Tasks::default(),
            stop,
            stopped: None,
        })
    }

    /// The engine, for the embedder's own bindings and for a test that reads
    /// what a script left behind.
    pub fn engine(&mut self) -> &mut Engine {
        &mut self.engine
    }

    /// The switch that stops this page's script, from any thread.
    pub fn stop_switch(&self) -> Stop {
        self.stop.clone()
    }

    /// Why the page stopped, if it has.
    pub fn stopped(&self) -> Option<&Stopped> {
        self.stopped.as_ref()
    }

    /// How many tasks are waiting.
    pub fn waiting(&self) -> usize {
        self.tasks.len()
    }

    /// Queue a task that runs a classic script's text.
    ///
    /// # Errors
    ///
    /// Why the page stopped, if it has: a stopped page runs nothing more.
    pub fn queue_script(&mut self, text: impl Into<String>) -> Result<Seq, Stopped> {
        if let Some(stopped) = &self.stopped {
            return Err(stopped.clone());
        }
        Ok(self.tasks.push(Work::Script(text.into())))
    }

    /// Queue a task that calls each of `callees`, in order, with `this` and
    /// `arguments`, with a checkpoint after each.
    ///
    /// The values must be reachable from a root the caller holds until this
    /// returns — holding them is an allocation (ADR 0014 § 2). From then on the
    /// task holds them, by one root, until it has run or been dropped. A callee
    /// that is not a function is the `TypeError` any call of one is, reported
    /// when its turn comes.
    ///
    /// # Errors
    ///
    /// Why the page stopped, if it has — including because the heap was too
    /// full to hold the task, which stops it here.
    pub fn queue_calls(
        &mut self,
        callees: &[Value],
        this: Value,
        arguments: &[Value],
    ) -> Result<Seq, Stopped> {
        if let Some(stopped) = &self.stopped {
            return Err(stopped.clone());
        }
        match task::calls(&mut self.engine, callees, this, arguments) {
            Ok(work) => Ok(self.tasks.push(work)),
            Err(escape) => {
                let why = Stopped::Escaped(escape);
                self.stop(why.clone());
                Err(why)
            }
        }
    }

    /// Run the oldest task waiting, and say what it did — or [`None`] if
    /// nothing is waiting or the page has stopped.
    pub fn run_next(&mut self) -> Option<Turn> {
        if self.stopped.is_some() {
            return None;
        }
        let task = self.tasks.pop()?;
        let mut turn = Turn {
            task: task.seq,
            reports: Vec::new(),
            jobs: 0,
            stopped: None,
        };
        let outcome = self.perform(&task.work, &mut turn);
        Tasks::release(&mut self.engine, task.work);
        let outcome = outcome
            .map_err(Stopped::Escaped)
            .and_then(|()| self.quiet_point());
        if let Err(why) = outcome {
            self.stop(why.clone());
            turn.stopped = Some(why);
        }
        Some(turn)
    }

    /// Run every piece of a task's script, each followed by a checkpoint.
    fn perform(&mut self, work: &Work, turn: &mut Turn) -> Result<(), Escape> {
        match work {
            Work::Script(text) => {
                self.script(text, turn)?;
                self.checkpoint(turn)
            }
            Work::Calls {
                list,
                arguments,
                callees,
            } => {
                for which in 0..*callees {
                    self.awake()?;
                    let (call, values) = task::call(&mut self.engine, list, *arguments, which)?;
                    match self.engine.call(call.callee, call.this, &values) {
                        Ok(_) => {}
                        Err(Escape::Thrown(thrown)) => turn
                            .reports
                            .push(Report::thrown(self.engine.objects(), &thrown)),
                        Err(escape) => return Err(escape),
                    }
                    self.checkpoint(turn)?;
                }
                Ok(())
            }
        }
    }

    /// Parse and run a classic script, reporting what a page would see
    /// reported.
    fn script(&mut self, text: &str, turn: &mut Turn) -> Result<(), Escape> {
        self.awake()?;
        let program = match script(text) {
            Ok(program) => program,
            Err(why) => {
                turn.reports.push(Report::NotParsed(why.to_string()));
                return Ok(());
            }
        };
        match self.engine.evaluate(&program) {
            Ok(_) => Ok(()),
            Err(Trouble::NotCompiled(refusal)) => {
                turn.reports.push(Report::NotCompiled(refusal.to_string()));
                Ok(())
            }
            Err(Trouble::Escaped(Escape::Thrown(thrown))) => {
                turn.reports
                    .push(Report::thrown(self.engine.objects(), &thrown));
                Ok(())
            }
            Err(Trouble::Escaped(escape)) => Err(escape),
        }
    }

    /// A microtask checkpoint: every job, oldest first, including those jobs
    /// queue; then the job ends.
    fn checkpoint(&mut self, turn: &mut Turn) -> Result<(), Escape> {
        let mut reports = Vec::new();
        let drained = self.engine.checkpoint(&mut |objects, thrown| {
            reports.push(Report::thrown(objects, thrown));
        });
        turn.reports.append(&mut reports);
        let drained = drained?;
        turn.jobs = turn.jobs.saturating_add(drained.ran);
        Ok(())
    }

    /// Refuse to start a piece of script once the page has been asked to stop.
    ///
    /// The engine reads the switch at every call and every backward jump, which
    /// a straight-line script makes neither of — so a page asked to stop while
    /// it was idle would otherwise run its next task's first script to the end.
    fn awake(&self) -> Result<(), Escape> {
        if self.stop.asked() {
            Err(Escape::Interrupted)
        } else {
            Ok(())
        }
    }

    /// ADR 0016 § 4's quiet point, checked.
    fn quiet_point(&mut self) -> Result<(), Stopped> {
        let heap = self.engine.objects().heap();
        let (scoped, kept) = (heap.scoped(), heap.kept());
        if scoped == 0 && kept == 0 {
            Ok(())
        } else {
            Err(Stopped::NotQuiet { scoped, kept })
        }
    }

    /// Stop the page: drop every task and its roots, and every job.
    fn stop(&mut self, why: Stopped) {
        self.tasks.drop_all(&mut self.engine);
        // A queue that has lost its list is a second bug behind whatever
        // stopped the page. The first is what is said, and the page is stopped
        // either way.
        let _second = self.engine.abandon();
        self.stopped = Some(why);
    }
}
