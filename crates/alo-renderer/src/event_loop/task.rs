/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The task queue: what the loop runs next, in the order it was queued.
//!
//! # One number across everything
//!
//! ADR 0016 § 2: HTML lets a browser choose among its task queues however it
//! likes, and we take the one choice a test can state — **one sequence number
//! across every queue, and the lowest runs next**. Nothing here is due later
//! than it was queued yet: a timer, which is the first task that is, is queue
//! item 92, and it changes *when* a task is due rather than this order among
//! the due ones. So the queue is one list, oldest at the front, and every task
//! is numbered as it joins it.
//!
//! # A waiting task holds its script by a root
//!
//! ADR 0016 § 1. A task's callees and arguments are references into the heap,
//! and this queue is outside it, so each task that holds script holds it by a
//! [`Root`] — one per *task*, never one per value: a list in the heap holding
//! `this`, the arguments and every callee, rooted when the task is queued and
//! released when it has run or been dropped. A script task holds its source
//! text, which is Rust's and needs no root.
//!
//! **A [`Root`] is not released by being dropped**, so nothing here lets a task
//! go except through [`Tasks::release`], which every path that ends a task
//! calls.

use std::collections::VecDeque;

use alo_js::object::Held;
use alo_js::{Engine, Escape, Fault, Root, Value};

/// Which task: its place in the one order every task is run in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seq(u64);

impl Seq {
    /// The number, which counts up from zero in the order tasks were queued.
    pub const fn number(self) -> u64 {
        self.0
    }
}

/// What a task does.
#[derive(Debug)]
pub(super) enum Work {
    /// Run a classic script's text.
    Script(String),
    /// Call each callee in turn with the same `this` and arguments — one
    /// listener after another for one event, or a timer's one callback.
    Calls {
        /// `this`, then the arguments, then the callees, in one rooted list.
        list: Root,
        /// How many arguments there are.
        arguments: usize,
        /// How many callees there are.
        callees: usize,
    },
}

/// A task waiting its turn.
#[derive(Debug)]
pub(super) struct Task {
    /// Its place in the order.
    pub(super) seq: Seq,
    /// What it does.
    pub(super) work: Work,
}

/// One call a [`Work::Calls`] task makes, read back out of its list.
#[derive(Debug, Clone, Copy)]
pub(super) struct Call {
    /// The function.
    pub(super) callee: Value,
    /// Its `this`.
    pub(super) this: Value,
}

/// The tasks waiting, oldest first.
#[derive(Debug, Default)]
pub(super) struct Tasks {
    waiting: VecDeque<Task>,
    /// The number the next task queued will have.
    next: u64,
}

impl Tasks {
    /// How many tasks are waiting.
    pub(super) fn len(&self) -> usize {
        self.waiting.len()
    }

    /// Put `work` at the back, and answer the number it was given.
    pub(super) fn push(&mut self, work: Work) -> Seq {
        let seq = Seq(self.next);
        self.next = self.next.saturating_add(1);
        self.waiting.push_back(Task { seq, work });
        seq
    }

    /// The oldest task, taken off the queue.
    pub(super) fn pop(&mut self) -> Option<Task> {
        self.waiting.pop_front()
    }

    /// Let go of every task waiting, running none of them (ADR 0016 § 7).
    pub(super) fn drop_all(&mut self, engine: &mut Engine) {
        while let Some(task) = self.waiting.pop_front() {
            Self::release(engine, task.work);
        }
    }

    /// Let go of what a task was holding.
    pub(super) fn release(engine: &mut Engine, work: Work) {
        match work {
            Work::Script(_) => {}
            Work::Calls { list, .. } => engine.objects().heap_mut().release(list),
        }
    }
}

/// The work of calling each of `callees` with `this` and `arguments`, its
/// values rooted.
///
/// **Making the list is an allocation**, so until it exists the values must be
/// reachable from a root the caller holds, as every value an embedder keeps
/// must be (ADR 0014 § 2). From the moment this answers, the task holds them.
///
/// # Errors
///
/// [`Escape::Full`] if the heap cannot hold the list; [`Escape::Broken`] if
/// the list it just made has gone, which is the engine's own bug.
pub(super) fn calls(
    engine: &mut Engine,
    callees: &[Value],
    this: Value,
    arguments: &[Value],
) -> Result<Work, Escape> {
    let objects = engine.objects();
    let list = objects.slots().map_err(|why| Escape::refused(why, 0))?;
    // Nothing allocates between making the list and filling it.
    let written = objects.with_slots(list, |slots, _| {
        slots.push(this);
        for argument in arguments {
            slots.push(*argument);
        }
        for callee in callees {
            slots.push(*callee);
        }
    });
    let root = objects.heap_mut().root(list);
    if written.is_none() {
        objects.heap_mut().release(root);
        return Err(Escape::fault(Fault::Gone));
    }
    Ok(Work::Calls {
        list: root,
        arguments: arguments.len(),
        callees: callees.len(),
    })
}

/// The `which`th call a [`Work::Calls`] task makes, and its arguments.
///
/// # Errors
///
/// [`Escape::Broken`] if the list has gone or is shorter than the task says,
/// which is the engine's bug or ours.
pub(super) fn call(
    engine: &mut Engine,
    list: &Root,
    arguments: usize,
    which: usize,
) -> Result<(Call, Vec<Value>), Escape> {
    let objects = engine.objects();
    let held = objects
        .heap()
        .holding(list)
        .ok_or(Escape::fault(Fault::Gone))?;
    let value = |at: usize| match objects.slot(held, at) {
        Some(Held::Value(value)) => Ok(value),
        Some(Held::Uninitialized) | None => Err(Escape::fault(Fault::Gone)),
    };
    let this = value(0)?;
    let mut values = Vec::with_capacity(arguments);
    for at in 1..=arguments {
        values.push(value(at)?);
    }
    let callee = value(arguments.saturating_add(1).saturating_add(which))?;
    Ok((Call { callee, this }, values))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tasks_are_numbered_in_the_order_they_join_and_leave_oldest_first() {
        let mut tasks = Tasks::default();
        let first = tasks.push(Work::Script("1".to_owned()));
        let second = tasks.push(Work::Script("2".to_owned()));
        assert!(first < second);
        assert_eq!((first.number(), second.number()), (0, 1));
        assert_eq!(tasks.pop().map(|task| task.seq), Some(first));
        let third = tasks.push(Work::Script("3".to_owned()));
        assert_eq!(tasks.pop().map(|task| task.seq), Some(second));
        assert_eq!(tasks.pop().map(|task| task.seq), Some(third));
        assert!(tasks.pop().is_none());
    }

    #[test]
    fn a_call_is_read_back_as_it_was_laid_out() {
        let Ok(mut engine) = Engine::new() else {
            panic!("an empty heap holds an engine");
        };
        let callees = [Value::Number(7.0), Value::Number(8.0)];
        let arguments = [Value::Bool(true), Value::Null];
        let Ok(Work::Calls {
            list,
            arguments: count,
            callees: many,
        }) = calls(&mut engine, &callees, Value::Number(1.0), &arguments)
        else {
            panic!("an empty heap holds a list");
        };
        assert_eq!((count, many), (2, 2));
        for (which, expected) in callees.iter().enumerate() {
            let Ok((call, values)) = super::call(&mut engine, &list, count, which) else {
                panic!("the list holds every call it was given");
            };
            assert_eq!(call.callee, *expected);
            assert_eq!(call.this, Value::Number(1.0));
            assert_eq!(values, arguments);
        }
        assert!(super::call(&mut engine, &list, count, 2).is_err());
        Tasks::release(
            &mut engine,
            Work::Calls {
                list,
                arguments: count,
                callees: many,
            },
        );
    }
}
