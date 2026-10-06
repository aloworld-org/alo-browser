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
//! more pieces of script: a classic script's text, a list of callees called
//! one after another with the same arguments, or a **dispatch from the
//! browser** ([`EventLoop::queue_dispatch`], [`dispatched`]), which steps
//! `alo-bindings`' one dispatch algorithm and calls each listener it names.
//! **After every piece, the loop performs a microtask checkpoint** (§ 3),
//! because each one leaves the engine with nothing running — which is why two
//! listeners on a button a *person* clicked see each other's microtasks run
//! between them, and the same two dispatched by a script's `dispatchEvent` or
//! `element.click()` do not.
//!
//! A throw nothing caught is [reported](Report) and the loop runs on — and so
//! is a throw a builtin asked to have reported, a listener's inside a
//! script's `dispatchEvent` (ADR 0018 § 3), handed over by the engine after
//! the piece of script it happened in and said before that piece's own. Anything
//! else that ends a piece of script — the embedder's [`Stop`], a full heap, a
//! thing the engine has not built, its bug — **stops the page** (§ 7): every
//! task waiting is dropped and its roots released, the job queue is emptied,
//! and nothing the page queued runs again. Reloading is the person's to ask
//! for.
//!
//! # Where a throw was
//!
//! The loop compiles each script itself rather than asking the engine to, and
//! keeps it under the name it was queued with ([`source`], queue item 241):
//! the engine places a throw by program and byte offset, and only the loop
//! has the text that turns an offset into a line and a column, and the name
//! a person would look for.
//!
//! # How much a turn says
//!
//! A job can queue a job that throws and queues another, so one task can end
//! in more reports than anything should hold — and every one would be
//! described, placed and kept until the turn ended. A turn keeps at most
//! [`MOST_REPORTS`], and fewer if its caller has less room
//! ([`EventLoop::run_next_within`]); past that a report is **counted and not
//! described**, so a page that throws for ever costs a counter rather than
//! memory. The page goes on running either way: a report kept or not, the
//! next job and the next task run as they would have.
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
//! **Nothing in a page queues a task yet.** A timer firing is item 92 and a
//! response is item 83; each will queue through [`EventLoop::queue_calls`].
//! The browser's dispatch is queued through [`Held::dispatch`], and an
//! agent's `Activate` on a page that runs script is one ([`Held::activate`],
//! [`activated`], item 256), run before the `Act` is answered, and so is its
//! `PutText` ([`Held::put_text`], `typed.rs`, item 257). The [`Renderer`]
//! holds one loop per page and queues a task for each of the page's own
//! scripts as it loads ([`crate::scripts`], item 236); the loop running between messages,
//! for tasks a page queued itself, and an `Act` answered after its task's
//! checkpoint are queue item 233. A ceiling on waiting tasks (ADR 0016's *the
//! numbers*) arrives with the first task a page can queue for itself; today
//! every task is queued by the renderer.
//!
//! [`Renderer`]: crate::Renderer
//! [`Held::dispatch`]: crate::held::Held::dispatch
//! [`Held::activate`]: crate::held::Held::activate
//! [`Held::put_text`]: crate::held::Held::put_text

mod activated;
mod described;
mod dispatched;
mod microtask;
mod report;
mod source;
mod task;
mod typed;

use core::fmt;

use std::rc::Rc;

use alo_bindings::Firing;
use alo_dom::NodeId;
use alo_js::heap::Ref;
use alo_js::interpret::{Engine, Stop};
use alo_js::{Escape, Thrown, Value, compile, script};

pub use activated::Clicked;
pub use report::Report;
use source::Sources;
pub use source::{Place, Trace};
pub use task::Seq;
use task::{Tasks, Unmade, Work};
pub use typed::Typed;

/// The most reports one turn keeps (queue item 242).
///
/// A report is bounded — a page's strings cut at 1024 code units, a trace at
/// 32 places — so this is what bounds a turn: 256 reports of a few kilobytes
/// each is well under a megabyte, and far more than a person reads before
/// they stop reading. A page that throws more than this in one task is
/// throwing in a loop, and the first 256 say what the loop is.
pub const MOST_REPORTS: usize = 256;

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

/// Why the browser's dispatch was not queued.
#[derive(Debug, Clone, PartialEq)]
pub enum Unqueued {
    /// The page has stopped — before this, or because the heap was too full
    /// to hold the task, which stops it here.
    Stopped(Stopped),
    /// The document has no such node: it was removed, or never existed.
    NoSuchNode(NodeId),
    /// What was given as the document is not a document cell, which is the
    /// caller's bug.
    NotADocument,
}

impl fmt::Display for Unqueued {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unqueued::Stopped(stopped) => stopped.fmt(out),
            Unqueued::NoSuchNode(node) => {
                write!(
                    out,
                    "nothing was dispatched: node {node} is not in the page"
                )
            }
            Unqueued::NotADocument => {
                write!(
                    out,
                    "nothing was dispatched: that is not the page's document"
                )
            }
        }
    }
}

/// What one task did.
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    /// Which task it was.
    pub task: Seq,
    /// Script that did not finish, in the order it happened — the first of
    /// them, up to the room the turn was given.
    pub reports: Vec<Report>,
    /// How many more did not finish and were counted rather than said,
    /// because the turn had no room left for them.
    pub unreported: usize,
    /// How many jobs ran in the checkpoints after its pieces of script.
    pub jobs: usize,
    /// Why the page stopped, if this task stopped it.
    pub stopped: Option<Stopped>,
    /// What an activation task's click came to, if this was one and it ran
    /// to its end.
    pub clicked: Option<Clicked>,
    /// What a `PutText` task's text came to, if this was one, as far as it
    /// got — [`None`] if the page stopped before it answered the
    /// `beforeinput`.
    pub typed: Option<Typed>,
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
    /// Every script this loop compiled, so a throw can be placed in the one
    /// it happened in (queue item 241).
    sources: Sources,
    /// How many reports the turn running now may keep.
    room: usize,
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
            sources: Sources::default(),
            room: MOST_REPORTS,
        })
    }

    /// The engine, for the embedder's own bindings and for a test that reads
    /// what a script left behind.
    pub fn engine(&mut self) -> &mut Engine {
        &mut self.engine
    }

    /// The engine's objects, to read — how the renderer borrows a document
    /// that lives in this loop's heap (ADR 0017 § 2) without running anything.
    pub fn objects(&self) -> &alo_js::Objects {
        let (_, objects) = self.engine.intrinsics();
        objects
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

    /// Queue a task that runs a classic script's text, under the name a throw
    /// in it is placed by — `script 3`, say.
    ///
    /// # Errors
    ///
    /// Why the page stopped, if it has: a stopped page runs nothing more.
    pub fn queue_script(
        &mut self,
        name: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<Seq, Stopped> {
        if let Some(stopped) = &self.stopped {
            return Err(stopped.clone());
        }
        Ok(self.tasks.push(Work::Script {
            name: name.into(),
            text: text.into(),
        }))
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

    /// Queue the browser's dispatch of the event `firing` describes to `node`,
    /// in the document `cell` holds (ADR 0018 § 3): one task, which makes
    /// `node`'s wrapper if it has none and the event — trusted, since the
    /// browser fires it (§ 4) — and holds both by one root until it has run
    /// or been dropped.
    ///
    /// When it runs, each listener the dispatch reaches is called with nothing
    /// else running and **followed by a microtask checkpoint**, before the
    /// dispatch decides who is next. A throw is reported and the dispatch
    /// carries on; anything else stops the page.
    ///
    /// `cell` must be rooted by the caller, as the renderer roots a page's
    /// document ([`crate::held`]).
    ///
    /// # Errors
    ///
    /// [`Unqueued`]: the page has stopped, or the heap could not hold the task
    /// and so stops it here; no such node; or no document.
    pub fn queue_dispatch(
        &mut self,
        cell: Ref,
        node: NodeId,
        firing: &Firing<'_>,
    ) -> Result<Seq, Unqueued> {
        if let Some(stopped) = &self.stopped {
            return Err(Unqueued::Stopped(stopped.clone()));
        }
        self.queue_listed(cell, node, firing, |list| Work::Dispatch { list })
    }

    /// Queue an agent's `Activate` of `node`, in the document `cell` holds
    /// (ADR 0018 §§ 5–6): one task that runs HTML's activation steps around
    /// the dispatch of a trusted `click` — a `PointerEvent` with no position
    /// — and fires the `input` and `change` a toggled box fires after it.
    /// What it came to is the task's [`Turn::clicked`].
    ///
    /// `cell` must be rooted by the caller.
    ///
    /// # Errors
    ///
    /// As [`EventLoop::queue_dispatch`].
    pub fn queue_activation(&mut self, cell: Ref, node: NodeId) -> Result<Seq, Unqueued> {
        if let Some(stopped) = &self.stopped {
            return Err(Unqueued::Stopped(stopped.clone()));
        }
        self.queue_listed(cell, node, &Firing::CLICK, |list| Work::Activate { list })
    }

    /// Queue an agent's `PutText` of `text` into the field `node`, in the
    /// document `cell` holds (ADR 0018 § 5): one task that dispatches a
    /// trusted `beforeinput` — an `InputEvent` whose `inputType` is
    /// `"insertReplacementText"` and whose `data` is `text` — and, unless a
    /// listener cancels it, replaces the field's text and fires `input` and
    /// `change`. What it came to is the task's [`Turn::typed`].
    ///
    /// `cell` must be rooted by the caller.
    ///
    /// # Errors
    ///
    /// As [`EventLoop::queue_dispatch`].
    pub fn queue_put_text(&mut self, cell: Ref, node: NodeId, text: &str) -> Result<Seq, Unqueued> {
        if let Some(stopped) = &self.stopped {
            return Err(Unqueued::Stopped(stopped.clone()));
        }
        let before = Firing::before_replacing(text);
        self.queue_listed(cell, node, &before, |list| Work::PutText {
            list,
            text: text.to_owned(),
        })
    }

    /// Queue the work `work` makes of a rooted list of `node`'s wrapper and
    /// the event `firing` describes.
    fn queue_listed(
        &mut self,
        cell: Ref,
        node: NodeId,
        firing: &Firing<'_>,
        work: impl FnOnce(alo_js::Root) -> Work,
    ) -> Result<Seq, Unqueued> {
        match task::listed(&mut self.engine, cell, node, firing) {
            Ok(list) => Ok(self.tasks.push(work(list))),
            Err(Unmade::NoSuchNode(node)) => Err(Unqueued::NoSuchNode(node)),
            Err(Unmade::NotADocument) => Err(Unqueued::NotADocument),
            Err(Unmade::Escaped(escape)) => {
                let why = Stopped::Escaped(escape);
                self.stop(why.clone());
                Err(Unqueued::Stopped(why))
            }
        }
    }

    /// Run the oldest task waiting, and say what it did — or [`None`] if
    /// nothing is waiting or the page has stopped. At most [`MOST_REPORTS`]
    /// reports are kept; the rest are counted in [`Turn::unreported`].
    pub fn run_next(&mut self) -> Option<Turn> {
        self.run_next_within(MOST_REPORTS)
    }

    /// [`run_next`](Self::run_next), keeping at most `room` reports — fewer
    /// than [`MOST_REPORTS`] when the caller's own ceiling spans several turns
    /// and some of it is spent. Room of nothing keeps nothing and counts
    /// everything; the task runs the same either way.
    pub fn run_next_within(&mut self, room: usize) -> Option<Turn> {
        if self.stopped.is_some() {
            return None;
        }
        let task = self.tasks.pop()?;
        self.room = room.min(MOST_REPORTS);
        let mut turn = Turn {
            task: task.seq,
            reports: Vec::new(),
            unreported: 0,
            jobs: 0,
            stopped: None,
            clicked: None,
            typed: None,
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
            Work::Script { name, text } => {
                self.script(name, text, turn)?;
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
                    let outcome = self.engine.call(call.callee, call.this, &values);
                    self.reported(turn);
                    match outcome {
                        Ok(_) => {}
                        Err(Escape::Thrown(thrown)) => self.report(&thrown, turn),
                        Err(escape) => return Err(escape),
                    }
                    self.checkpoint(turn)?;
                }
                Ok(())
            }
            Work::Dispatch { list } => self.dispatch(list, turn),
            Work::Activate { list } => {
                turn.clicked = Some(self.activate(list, turn)?);
                Ok(())
            }
            Work::PutText { list, text } => self.put_text(list, text, turn),
        }
    }

    /// Parse and run a classic script, reporting what a page would see
    /// reported.
    ///
    /// The script is kept, under its name, before it runs: a throw in it is
    /// placed by it, and so is one in a function it declared that throws long
    /// after it has finished.
    fn script(&mut self, name: &str, text: &str, turn: &mut Turn) -> Result<(), Escape> {
        self.awake()?;
        let program = match script(text) {
            Ok(program) => program,
            Err(why) => {
                self.keep(turn, || Report::NotParsed(why.to_string()));
                return Ok(());
            }
        };
        let unit = match compile(&program) {
            Ok(unit) => Rc::new(unit),
            Err(refusal) => {
                self.keep(turn, || Report::NotCompiled(refusal.to_string()));
                return Ok(());
            }
        };
        self.sources.keep(&unit, name, text);
        let outcome = self.engine.run(&unit);
        self.reported(turn);
        match outcome {
            Ok(_) => Ok(()),
            Err(Escape::Thrown(thrown)) => {
                self.report(&thrown, turn);
                Ok(())
            }
            Err(escape) => Err(escape),
        }
    }

    /// Report a throw nothing caught, while what it threw is still alive and
    /// the engine still knows where it was.
    fn report(&mut self, thrown: &Thrown, turn: &mut Turn) {
        if self.counted(turn) {
            return;
        }
        let trace = self.sources.trace(self.engine.unwound());
        turn.reports
            .push(Report::thrown(self.engine.objects(), thrown, trace));
    }

    /// Report every throw a builtin asked to have reported during the piece
    /// of script that has just ended — a listener's, inside a script's
    /// `dispatchEvent` (ADR 0018 § 3) — before that piece's own throw, since
    /// they happened first.
    ///
    /// Handing them over allocates nothing, so a thrown value the caller is
    /// still holding survives it.
    fn reported(&mut self, turn: &mut Turn) {
        let room = self.room.saturating_sub(turn.reports.len());
        let mut reports = Vec::new();
        let mut unreported = 0_usize;
        let sources = &self.sources;
        let more = self
            .engine
            .hand_over_reported(&mut |objects, thrown, unwound| {
                if reports.len() < room {
                    reports.push(Report::thrown(objects, thrown, sources.trace(unwound)));
                } else {
                    unreported = unreported.saturating_add(1);
                }
            });
        turn.reports.append(&mut reports);
        turn.unreported = turn
            .unreported
            .saturating_add(unreported)
            .saturating_add(more);
    }

    /// Keep a report the turn has room for, or count it.
    fn keep(&self, turn: &mut Turn, report: impl FnOnce() -> Report) {
        if !self.counted(turn) {
            turn.reports.push(report());
        }
    }

    /// Count a report the turn has no room for, answering whether it did —
    /// asked before one is made, because describing and placing a throw is
    /// the cost a page throwing in a loop would otherwise make us pay.
    fn counted(&self, turn: &mut Turn) -> bool {
        if turn.reports.len() < self.room {
            false
        } else {
            turn.unreported = turn.unreported.saturating_add(1);
            true
        }
    }

    /// A microtask checkpoint: every job, oldest first, including those jobs
    /// queue; then the job ends.
    fn checkpoint(&mut self, turn: &mut Turn) -> Result<(), Escape> {
        let room = self.room.saturating_sub(turn.reports.len());
        let mut reports = Vec::new();
        let mut unreported = 0_usize;
        let sources = &self.sources;
        let drained = self.engine.checkpoint(&mut |objects, thrown, unwound| {
            if reports.len() < room {
                reports.push(Report::thrown(objects, thrown, sources.trace(unwound)));
            } else {
                unreported = unreported.saturating_add(1);
            }
        });
        turn.reports.append(&mut reports);
        turn.unreported = turn.unreported.saturating_add(unreported);
        let drained = drained?;
        turn.unreported = turn.unreported.saturating_add(drained.unreported);
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
