/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a throw lands (queue item 210).
//!
//! Every instruction answers a `Result`, and an [`Escape::Thrown`] coming out
//! of one is no longer the end of the run: the loop hands it here, and this
//! looks for a `try` that guards the instruction — in the running call, then in
//! the call that made it, and outwards. Finding one, it takes down everything
//! the throw interrupted and lands the thrown value where the compiler said
//! (see [`try_statement`](crate::compile)).
//!
//! # A reported call is as far as a throw goes
//!
//! A builtin that asked for a call with
//! [`Want::Report`](crate::object::native::Want) — `dispatchEvent` calling a
//! listener — is a wall: a `try` inside that call may catch the throw, and a
//! `try` around the `dispatchEvent` may not, because the standard reports a
//! listener's throw and carries on (ADR 0018 § 3). So the search stops at the
//! innermost such call, and a throw that reaches it is taken down to it, set
//! aside for the embedder ([`reported`](super::reported)) and answered as
//! `undefined`.
//!
//! # Only the page's own escapes are caught
//!
//! A `catch` reaches exactly [`Escape::is_the_pages`]: a value a script threw,
//! and an error the language specifies. A full heap is the embedder's, a
//! stopped script is the browser process's, something not built yet is a
//! sentence for a person and a lost reference is this engine's bug — and a
//! `catch` that could swallow any of those would let a page carry on past a
//! decision that was not its to make.
//!
//! # A frame is searched by the instruction it is running
//!
//! Not by its program counter less one. An instruction that needs an object
//! turned into a primitive **rewinds** the counter to itself, so that it runs
//! again once the `valueOf` has answered; a throw from that `valueOf` would
//! then be judged by the instruction *before* — usually inside the same `try`,
//! and not always. [`Frame::now`] is written once per instruction by the loop
//! and never rewound, so the search asks about the instruction that was
//! actually running in every frame, whether it threw or made the call that
//! did.
//!
//! # What is taken down, and why nothing allocates until it is
//!
//! Every call made since the `try` began — the frames, which each give their
//! environment back, and the builtins waiting inside them — and every block
//! the `try`'s frame went into, down to the count the handler recorded. Then
//! the stack is cut to the frame's operands and the thrown value is pushed.
//!
//! A thrown **value** is in a Rust local from the `throw` that took it off the
//! stack to the push that puts it back, so nothing between those two may
//! allocate: releasing a root and cutting a list do not. An error the
//! **engine** threw is not a value yet, and becomes one only after the stack
//! is cut — so the object it becomes is made with nothing left in a local.

use crate::abrupt::{Escape, Internal, Thrown};
use crate::builtin::error::Family;
use crate::code::Handler;
use crate::object::{Found, Property, Value};

use super::Engine;
use super::frame::{Run, Slot, Waiting};
use super::unwound::Unwound;

impl Engine {
    /// Land a throw in the `try` that guards it, or answer the escape if
    /// nothing does.
    ///
    /// # Errors
    ///
    /// The escape itself, when it is not the page's or no `try` is waiting
    /// for it; or a fault, a full heap, or a string too long while making an
    /// error the engine threw into an object.
    pub(super) fn land(&mut self, run: &mut Run, escape: Escape) -> Result<(), Escape> {
        let Escape::Thrown(thrown) = escape else {
            return Err(escape);
        };
        // A builtin waiting on a call it asked to have reported is as far out
        // as this throw may go: only the frames inside that call may catch it.
        let boundary = Self::reporting(run);
        let first = boundary.map_or(0, |place| {
            run.frames
                .iter()
                .position(|frame| frame.callee_at >= place)
                .unwrap_or(run.frames.len())
        });
        let Some((which, handler)) = Self::guarding(run, first)? else {
            if let Some(place) = boundary {
                return self.set_aside(run, first, place, thrown);
            }
            // The last moment the calls it left exist: the run gives them back
            // on its way out (queue item 241).
            self.unwound = Unwound::of(run)?;
            return Err(Escape::Thrown(thrown));
        };
        self.take_down(run, which, handler)?;
        let base = run.base()?;
        let stack = run.stack;
        self.objects
            .with_slots(stack, |slots, _| slots.truncate(base))
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        match thrown {
            Thrown::Value { value, .. } => self.push(run, value)?,
            Thrown::Error { kind, message, at } => {
                self.push_error(run, Family::from(kind), &message, at)?;
            }
        }
        let frame = run.frame_mut()?;
        frame.pc = handler.landing;
        Ok(())
    }

    /// Where the answer of the innermost call a builtin asked to have
    /// reported lands — its callee's place — if one is running.
    fn reporting(run: &Run) -> Option<usize> {
        run.builtins
            .iter()
            .rev()
            .find(|waiting| waiting.reporting)
            .map(Waiting::answer_at)
    }

    /// Stop a throw at the reported call whose callee sits at `place`: take
    /// down every call inside it, set the throw aside with where it had got
    /// to, and answer the builtin that asked `undefined` (ADR 0018 § 3).
    ///
    /// A thrown value is in a Rust local until it is rooted, and nothing
    /// between allocates: releasing roots and cutting a list do not, and a
    /// root is an entry in the root list rather than a cell.
    fn set_aside(
        &mut self,
        run: &mut Run,
        first: usize,
        place: usize,
        thrown: Thrown,
    ) -> Result<(), Escape> {
        let unwound = Unwound::of_from(run, first)?;
        while run.frames.len() > first {
            let Some(frame) = run.frames.pop() else {
                return Err(Escape::Broken(Internal::StackIsWrong));
            };
            if let Some(root) = frame.environment {
                self.objects.heap_mut().release(root);
            }
        }
        while run
            .builtins
            .last()
            .is_some_and(|waiting| waiting.callee_at >= place)
        {
            run.builtins.pop();
        }
        self.set_aside.keep(&mut self.objects, thrown, unwound);
        let stack = run.stack;
        self.objects
            .with_slots(stack, |slots, _| {
                slots.truncate(place);
                slots.push(Value::Undefined);
            })
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        let waiting = run
            .builtins
            .last_mut()
            .filter(|waiting| waiting.reporting && waiting.answer_at() == place)
            .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
        waiting.answer = Slot::Reported;
        run.answered()
    }

    /// The innermost frame from `first` inwards with a `try` around the
    /// instruction it is running, and that `try`'s handler.
    fn guarding(run: &Run, first: usize) -> Result<Option<(usize, Handler)>, Escape> {
        for (which, frame) in run.frames.iter().enumerate().rev() {
            if which < first {
                break;
            }
            let chunk = run
                .units
                .get(frame.unit)
                .and_then(|loaded| loaded.unit.chunk(frame.chunk))
                .ok_or(Escape::Broken(Internal::JumpIsWrong))?;
            if let Some(handler) = chunk.handler(frame.now) {
                if handler.landing > chunk.code().len() {
                    return Err(Escape::Broken(Internal::JumpIsWrong));
                }
                return Ok(Some((which, handler)));
            }
        }
        Ok(None)
    }

    /// Take down every call above frame `which`, and every block it went into
    /// since the `try` began.
    fn take_down(&mut self, run: &mut Run, which: usize, handler: Handler) -> Result<(), Escape> {
        while run.frames.len() > which.saturating_add(1) {
            let Some(frame) = run.frames.pop() else {
                return Err(Escape::Broken(Internal::StackIsWrong));
            };
            if let Some(root) = frame.environment {
                self.objects.heap_mut().release(root);
            }
        }
        // A builtin waiting inside this frame was entered from one of its
        // instructions, so its callee is above the frame's operands; one this
        // frame was itself called by is below its callee.
        let base = run.base()?;
        while run
            .builtins
            .last()
            .is_some_and(|waiting| waiting.callee_at >= base)
        {
            run.builtins.pop();
        }
        while run.frame()?.environments > handler.environments {
            self.pop_environment(run)?;
        }
        if run.frame()?.environments != handler.environments {
            // Fewer blocks than when the `try` began, inside the `try`: the
            // compiler and this loop disagree.
            return Err(Escape::Broken(Internal::StackIsWrong));
        }
        Ok(())
    }

    /// Push an error the engine threw as the object a page catches: an
    /// instance of its family's constructor with its message as an own
    /// property, which is what `new TypeError(message)` would have made.
    fn push_error(
        &mut self,
        run: &mut Run,
        family: Family,
        message: &str,
        at: usize,
    ) -> Result<(), Escape> {
        let constructor = self
            .realm
            .intrinsics()
            .error_constructor(&self.objects, family)?;
        let key = self
            .objects
            .key(&units("prototype"))
            .map_err(|why| Escape::refused(why, at))?;
        // Neither writable nor configurable, so it is the object it was made
        // with — and the intrinsic's root holds it.
        let Found::Value(Value::Object(above)) = self.objects.get(constructor, key)? else {
            return Err(Escape::Broken(Internal::ConstructorIsWrong));
        };
        let made = self
            .objects
            .error(Some(above))
            .map_err(|why| Escape::refused(why, at))?;
        // On the stack before the message allocates.
        self.push(run, Value::Object(made))?;

        let scope = self.objects.heap_mut().open();
        let outcome = self.give_message(made, message, at);
        self.objects.heap_mut().close(scope);
        outcome
    }

    /// The message, own and not enumerable, with the scope open. The key is
    /// held before the string is made, so nothing allocates between making the
    /// string and storing it.
    fn give_message(
        &mut self,
        made: crate::heap::Ref,
        message: &str,
        at: usize,
    ) -> Result<(), Escape> {
        let key = self
            .objects
            .key(&units("message"))
            .map_err(|why| Escape::refused(why, at))?;
        if let Some(held) = key.reference() {
            self.objects.heap_mut().hold(held);
        }
        let text = self
            .objects
            .text(units(message))
            .map_err(|why| Escape::refused(why, at))?;
        let defined = self.objects.define(
            made,
            key,
            Property::data(Value::Text(text), true, false, true),
        )?;
        if defined {
            Ok(())
        } else {
            // A new object with nothing on it refused a property: ours.
            Err(Escape::Broken(Internal::StackIsWrong))
        }
    }
}

/// A name as the code units a key or a string is made of.
fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}
