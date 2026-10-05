/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The job queue: what runs at the next microtask checkpoint (queue item 232).
//!
//! # It is the engine's, and it is in the heap
//!
//! ADR 0016 § 1. A job — a `queueMicrotask` callback today, a promise reaction
//! when item 75 builds promises — is a function and its arguments, and every
//! one of them is a reference the collector must see. ADR 0014 § 2 does not
//! let a reference live in a Rust value somebody outside the engine is holding
//! unless that is a [`Root`], so a queue held by the renderer would be a root
//! per job, taken and released once per promise reaction on every page. This
//! is one [`Slots`](crate::object::Slots) cell instead, rooted once for the
//! engine's life: an ordinary structure the collector walks, and a job nobody
//! will run any more is garbage like anything else.
//!
//! # The layout
//!
//! Jobs sit back to back in the one list, oldest first, each as its callee
//! followed by its arguments. How many arguments each has is a number and not
//! a reference, so it lives on this side of the heap, in [`Jobs::counts`]. The
//! oldest job begins at [`Jobs::head`]; a job that has been taken is not
//! removed at once, because removing from the front moves everything behind it.
//! The list is given back when it empties, and compacted when what has been run
//! outweighs what has not — so a job that queues one more job for ever holds
//! one job's worth of slots, not one per job it has run.
//!
//! # Its length is bounded by the heap, not by a number of its own
//!
//! ADR 0016 § 3. Growing the list is a [`Heap::write`](crate::heap::Heap::write),
//! which counts the bytes it grew by, so a page queueing without end meets
//! [`bounds::HEAP_CEILING`](crate::bounds::HEAP_CEILING) at its next
//! allocation and is the full heap ADR 0014 § 9 stops a tab for. And it cannot
//! queue without end inside one job without a backward jump or a call, both of
//! which read the embedder's [`Stop`](crate::interpret::Stop).

use std::collections::VecDeque;

use crate::abrupt::{Escape, Internal};
use crate::heap::{Ref, Root};
use crate::object::{Fault, Held, Objects, Value};

/// The jobs waiting for the next checkpoint, oldest first.
#[derive(Debug)]
pub struct Jobs {
    /// The callees and arguments, back to back.
    list: Root,
    /// How many arguments each waiting job has, oldest first.
    counts: VecDeque<usize>,
    /// Where in the list the oldest waiting job begins.
    head: usize,
}

impl Jobs {
    /// An empty queue.
    ///
    /// # Errors
    ///
    /// [`Escape::Full`] if the heap cannot hold one empty list, which is a heap
    /// that was full before anything ran.
    pub fn new(objects: &mut Objects) -> Result<Self, Escape> {
        let list = objects.slots().map_err(|why| Escape::refused(why, 0))?;
        let list = objects.heap_mut().root(list);
        Ok(Self {
            list,
            counts: VecDeque::new(),
            head: 0,
        })
    }

    /// How many jobs are waiting.
    pub fn len(&self) -> usize {
        self.counts.len()
    }

    /// Whether none is.
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// Queue a call of `callee` with `arguments`, behind every job already
    /// waiting.
    ///
    /// **Not a safepoint**: growing a list is a write, not an allocation, so the
    /// values may come straight out of a Rust local — which is where a builtin's
    /// [`Want::Job`](crate::object::native::Want) carries them.
    ///
    /// # Errors
    ///
    /// [`Escape::Broken`] if the queue has lost its list, which is this
    /// engine's own bug.
    pub fn push(
        &mut self,
        objects: &mut Objects,
        callee: Value,
        arguments: &[Value],
    ) -> Result<(), Escape> {
        let list = self.list(objects)?;
        objects
            .with_slots(list, |slots, _| {
                slots.push(callee);
                for argument in arguments {
                    slots.push(*argument);
                }
            })
            .ok_or_else(|| Escape::fault(Fault::Gone))?;
        self.counts.push_back(arguments.len());
        Ok(())
    }

    /// Move the oldest job on to the top of `stack` as a call — its callee,
    /// `undefined` as its `this`, then its arguments — and answer how many
    /// arguments it has, or [`None`] if nothing is waiting.
    ///
    /// **Not a safepoint**, and that is the point of doing it this way: between
    /// the values leaving the queue and arriving on the stack nothing
    /// allocates, so they are never anywhere the collector cannot see. The
    /// stack must already exist and be rooted.
    ///
    /// # Errors
    ///
    /// [`Escape::Broken`] if either list has gone or the queue's own count of a
    /// job disagrees with its list, which is this engine's own bug.
    pub fn take_onto(
        &mut self,
        objects: &mut Objects,
        stack: Ref,
    ) -> Result<Option<usize>, Escape> {
        let Some(count) = self.counts.pop_front() else {
            return Ok(None);
        };
        let list = self.list(objects)?;
        let end = self.head.saturating_add(1).saturating_add(count);
        let mut job = Vec::with_capacity(count.saturating_add(2));
        for at in self.head..end {
            match objects.slot(list, at) {
                Some(Held::Value(value)) => job.push(value),
                Some(Held::Uninitialized) | None => {
                    return Err(Escape::Broken(Internal::StackIsWrong));
                }
            }
        }
        // The callee, then `this`, then the arguments: the shape every call
        // has on the stack (see `interpret::call`).
        job.insert(1.min(job.len()), Value::Undefined);
        objects
            .with_slots(stack, |slots, _| {
                for value in job {
                    slots.push(value);
                }
            })
            .ok_or_else(|| Escape::fault(Fault::Gone))?;
        self.head = end;
        self.give_back_what_has_run(objects, list)?;
        Ok(Some(count))
    }

    /// Empty the queue: a stopped page's jobs are dropped, not kept for later
    /// (ADR 0016 § 7). What they held becomes garbage.
    ///
    /// # Errors
    ///
    /// [`Escape::Broken`] if the queue has lost its list.
    pub fn clear(&mut self, objects: &mut Objects) -> Result<(), Escape> {
        let list = self.list(objects)?;
        objects
            .with_slots(list, |slots, _| slots.truncate(0))
            .ok_or_else(|| Escape::fault(Fault::Gone))?;
        self.counts.clear();
        self.head = 0;
        Ok(())
    }

    /// The list, or this engine's bug.
    fn list(&self, objects: &Objects) -> Result<Ref, Escape> {
        objects
            .heap()
            .holding(&self.list)
            .ok_or_else(|| Escape::fault(Fault::Gone))
    }

    /// Give back the slots of jobs that have been taken, once they are all of
    /// the list or more than half of it.
    fn give_back_what_has_run(&mut self, objects: &mut Objects, list: Ref) -> Result<(), Escape> {
        let len = objects
            .slot_count(list)
            .ok_or_else(|| Escape::fault(Fault::Gone))?;
        if self.head < len && self.head <= len / 2 {
            return Ok(());
        }
        let head = self.head;
        objects
            .with_slots(list, |slots, _| slots.remove_front(head))
            .ok_or_else(|| Escape::fault(Fault::Gone))?;
        self.head = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Jobs;
    use crate::object::{Held, Objects, Value};

    #[test]
    fn a_job_comes_off_as_a_call_and_the_oldest_comes_first() {
        let mut objects = Objects::new();
        let Ok(mut jobs) = Jobs::new(&mut objects) else {
            panic!("an empty heap holds a queue");
        };
        let Ok(stack) = objects.slots() else {
            panic!("an empty heap holds a stack");
        };
        assert!(jobs.is_empty());
        assert_eq!(
            jobs.push(&mut objects, Value::Number(1.0), &[Value::Bool(true)]),
            Ok(())
        );
        assert_eq!(jobs.push(&mut objects, Value::Number(2.0), &[]), Ok(()));
        assert_eq!(jobs.len(), 2);

        assert_eq!(jobs.take_onto(&mut objects, stack), Ok(Some(1)));
        let read = |objects: &Objects, at| objects.slot(stack, at);
        assert_eq!(read(&objects, 0), Some(Held::Value(Value::Number(1.0))));
        assert_eq!(read(&objects, 1), Some(Held::Value(Value::Undefined)));
        assert_eq!(read(&objects, 2), Some(Held::Value(Value::Bool(true))));

        assert_eq!(jobs.take_onto(&mut objects, stack), Ok(Some(0)));
        assert_eq!(read(&objects, 3), Some(Held::Value(Value::Number(2.0))));
        assert_eq!(read(&objects, 4), Some(Held::Value(Value::Undefined)));
        assert_eq!(jobs.take_onto(&mut objects, stack), Ok(None));
        assert!(jobs.is_empty());
    }

    #[test]
    fn a_queue_refilled_for_ever_holds_one_jobs_worth_of_slots() {
        let mut objects = Objects::new();
        let Ok(mut jobs) = Jobs::new(&mut objects) else {
            panic!("an empty heap holds a queue");
        };
        let Ok(stack) = objects.slots() else {
            panic!("an empty heap holds a stack");
        };
        let Ok(list) = jobs.list(&objects) else {
            panic!("the queue holds its list");
        };
        assert_eq!(jobs.push(&mut objects, Value::Number(0.0), &[]), Ok(()));
        for turn in 1..1_000_u32 {
            assert_eq!(
                jobs.push(&mut objects, Value::Number(f64::from(turn)), &[]),
                Ok(())
            );
            assert_eq!(jobs.take_onto(&mut objects, stack), Ok(Some(0)));
            assert!(
                objects.slot_count(list).unwrap_or(usize::MAX) <= 4,
                "what has run is given back"
            );
        }
        assert_eq!(jobs.len(), 1);
    }

    #[test]
    fn clearing_drops_every_job() {
        let mut objects = Objects::new();
        let Ok(mut jobs) = Jobs::new(&mut objects) else {
            panic!("an empty heap holds a queue");
        };
        let Ok(stack) = objects.slots() else {
            panic!("an empty heap holds a stack");
        };
        assert_eq!(jobs.push(&mut objects, Value::Null, &[Value::Null]), Ok(()));
        assert_eq!(jobs.clear(&mut objects), Ok(()));
        assert!(jobs.is_empty());
        assert_eq!(jobs.take_onto(&mut objects, stack), Ok(None));
        assert_eq!(objects.slot_count(stack), Some(0));
    }
}
