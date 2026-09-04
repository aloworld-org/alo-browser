/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which environment is in force, and the three ways that changes.
//!
//! A call is given one and every name it reads is `hops` links out from it. A
//! **block** that declares something is one too (queue item 216), so the
//! environment in force is not a property of the frame's whole life: it is
//! pushed on the way into a block, popped on the way out, and **copied** at each
//! pass of a `for (let …)` head.
//!
//! # The frame holds one root, not a stack of them
//!
//! [`Frame::environment`] is the environment in force *now*, and the ones it was
//! made under are reached through its parent link — which is the same chain
//! `hops` walks, so there is nothing else to keep. Rooting only the innermost is
//! enough because the chain keeps the rest: a parent a child holds is not
//! garbage.
//!
//! What the frame counts instead is [`Frame::environments`], how many it has
//! pushed and not popped. Nothing a script writes decides it — the compiler
//! emits the pushes and the pops in pairs, and a `break` out of three blocks
//! emits three pops — so a pop with nothing to pop is a disagreement between the
//! compiler and this loop, which is [`Internal::StackIsWrong`] like every other
//! one.
//!
//! # A copy is a sibling
//!
//! [`Op::CopyEnvironment`] replaces the environment in force with one under the
//! *same* parent, so the depth is unchanged and every `hops` the compiler
//! counted still means what it meant. That is what makes each pass of a
//! `for (let i = …)` a binding of its own: the closure made in one pass keeps
//! the environment that pass ran in, and the increment for the next pass writes
//! into a different one.

use crate::abrupt::{Escape, Internal};
use crate::heap::Ref;
use crate::object::Fault;

use super::Engine;
use super::frame::Run;

impl Engine {
    /// `Op::PushEnvironment`: go into a block that declares something.
    pub(super) fn push_environment(
        &mut self,
        run: &mut Run,
        bindings: u32,
        at: usize,
    ) -> Result<(), Escape> {
        let bindings =
            usize::try_from(bindings).map_err(|_| Escape::Broken(Internal::StackIsWrong))?;
        let parent = self.environment_of(run)?;
        // A safepoint, and the parent survives it because the frame's root is
        // holding it — which is the whole reason the root is swapped *after*
        // the allocation rather than before.
        let made = self
            .objects
            .environment(parent, bindings)
            .map_err(|why| Escape::refused(why, at))?;
        self.take(run, made, true)
    }

    /// `Op::PopEnvironment`: come out of one.
    pub(super) fn pop_environment(&mut self, run: &mut Run) -> Result<(), Escape> {
        if run.frame()?.environments == 0 {
            // The compiler said there was a block to leave. There is not, so
            // the compiler and this loop disagree, which is our bug.
            return Err(Escape::Broken(Internal::StackIsWrong));
        }
        let held = self
            .environment_of(run)?
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        let parent = self
            .objects
            .enclosing(held)
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        let root = parent.map(|held| self.objects.heap_mut().root(held));
        let frame = run.frame_mut()?;
        frame.environments = frame.environments.saturating_sub(1);
        let was = std::mem::replace(&mut frame.environment, root);
        if let Some(was) = was {
            self.objects.heap_mut().release(was);
        }
        Ok(())
    }

    /// `Op::CopyEnvironment`: `CreatePerIterationEnvironment`.
    pub(super) fn copy_environment(&mut self, run: &mut Run, at: usize) -> Result<(), Escape> {
        let held = self
            .environment_of(run)?
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        let made = self
            .objects
            .copy_environment(held)
            .map_err(|why| Escape::refused(why, at))?
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        self.take(run, made, false)
    }

    /// Make this environment the one in force, giving the last one's root back.
    ///
    /// The new one is rooted **before** the old one is released, because
    /// releasing first would leave the only reference to it in a Rust local
    /// across a call that can collect.
    fn take(&mut self, run: &mut Run, made: Ref, deeper: bool) -> Result<(), Escape> {
        let root = self.objects.heap_mut().root(made);
        let frame = run.frame_mut()?;
        if deeper {
            frame.environments = frame.environments.saturating_add(1);
        }
        let was = frame.environment.replace(root);
        if let Some(was) = was {
            self.objects.heap_mut().release(was);
        }
        Ok(())
    }

    /// The environment `hops` out from the one in force.
    pub(super) fn environment_at(&self, run: &Run, hops: u32) -> Result<Ref, Escape> {
        let mut held = self
            .environment_of(run)?
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        for _ in 0..hops {
            held = self
                .objects
                .enclosing(held)
                .ok_or_else(|| Escape::fault(Fault::Gone))?
                .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        }
        Ok(held)
    }

    /// The environment in force, which a script's own frame has none of until it
    /// goes into a block.
    pub(super) fn environment_of(&self, run: &Run) -> Result<Option<Ref>, Escape> {
        match &run.frame()?.environment {
            Some(root) => Ok(Some(
                self.objects
                    .heap()
                    .holding(root)
                    .ok_or_else(|| Escape::fault(Fault::Gone))?,
            )),
            None => Ok(None),
        }
    }
}
