/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a run is made of: one stack, one list of constants, and a frame per
//! call that has not returned.
//!
//! # The frames are a list here and the values are in the heap
//!
//! ADR 0014 § 2 names the closed list of places a live reference may be, and
//! this file is where the interpreter keeps its side of that promise. A
//! [`Frame`] is bookkeeping — where in the stack this call's things start, and
//! which instruction it is on — and it holds **no value at all**. Everything a
//! call could lose is somewhere the collector walks:
//!
//! - the callee, its `this` and its arguments are **on the stack**, which is a
//!   cell, below where the call's own operands begin;
//! - its **environment** is a [`Root`], which is the list of things held until
//!   somebody gives them back, and it is given back by the `return` that ends
//!   the frame — or by the instruction that leaves a block for the one outside
//!   it (see [`environment`](super::environment)).
//!
//! So a `Frame` is a handful of indices and a `Rc` index, and losing one loses
//! nothing but a place to carry on from.
//!
//! # One stack for every frame
//!
//! A call does not get a stack of its own: it continues the caller's, and
//! [`Frame::base`] is the floor below which its instructions may not reach.
//! That is what makes the arguments a caller pushed into the parameters a callee
//! reads without anything being copied twice, and it is what makes
//! [`bounds::VALUES_ON_THE_STACK`](crate::bounds) a bound on the whole run
//! rather than on one call.
//!
//! # A program is loaded once per run, however many times it is entered
//!
//! A function made by one script and called by the next brings its own
//! [`Unit`] with it, so a run may reach more than one — and each needs its
//! string constants as heap cells and its property keys interned. [`Loaded`] is
//! that work done once, and `offset` is where that unit's constants begin in the
//! run's single list.

use std::rc::Rc;

use crate::abrupt::{Escape, Internal};
use crate::code::Chunk;
use crate::convert::Hint;
use crate::heap::{Ref, Root};
use crate::object::Key;
use crate::object::native::Body;
use crate::unit::Unit;

/// What the value a call answers with is for.
///
/// A call is not always an expression. An instruction half way through
/// something else can want one — reading a property that is a getter, writing
/// one that is a setter, asking an object for a primitive — and what happens to
/// the answer is different in each case. So the frame records it, and *leaving*
/// a call is one `match` rather than four kinds of frame.
///
/// It holds no [`Value`](crate::object::Value), for the same reason the rest of
/// [`Frame`] does not: everything a call could lose is on the stack, which the
/// collector walks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum After {
    /// The answer is what the instruction that made this call leaves behind,
    /// and it lands where the callee stood. `f()`, and a getter.
    Answer,
    /// The answer is dropped, because what the instruction evaluates to is
    /// already on the stack below the call. A setter: `a.b = c` evaluates to
    /// `c` whatever the setter chose to return.
    Discard,
    /// The answer is `typeof`'d — the one instruction whose value is a question
    /// *about* what a getter returned rather than the thing itself.
    TypeOf,
    /// The answer is one step of turning an object into a primitive, and the
    /// instruction that wanted the primitive runs again once there is one.
    Convert(Converting),
    /// The answer belongs to the builtin that asked for the call: it lands in
    /// that builtin's answer slot and the builtin runs again (queue item 219).
    ///
    /// It names no builtin because it does not have to — calls nest, so the one
    /// waiting is always the last of [`Run::builtins`].
    Builtin,
    /// The call was `new f()`, so the answer is what the body returned **if it
    /// is an object**, and otherwise the object that was made for it — which
    /// is still in the `this` slot, because nothing can assign to `this`
    /// (queue item 212).
    Construct,
}

/// Where a conversion that had to call something has got to.
///
/// `OrdinaryToPrimitive` tries two names in turn, and either of them can be a
/// call — so a conversion is a small state machine rather than one call, and
/// this is its state. Every field is a number or a flag, and the object being
/// converted is named by *where it is on the stack* rather than held here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Converting {
    /// Which of the two calls is outstanding.
    pub(crate) step: Step,
    /// Where the object being converted is in the stack — and where its
    /// primitive is written when there is one, so that the instruction reading
    /// that operand finds it there when it runs again.
    pub(crate) at: usize,
    /// Which primitive was asked for.
    pub(crate) hint: Hint,
    /// Which name the search carries on at if this call does not produce one.
    pub(crate) next: usize,
    /// The byte offset of the instruction that wanted it, for a message.
    pub(crate) source: usize,
    /// Who wanted it, which decides what runs once there is one.
    pub(crate) then: Then,
}

/// Who a conversion's primitive is for.
///
/// The primitive is written into the slot at [`Converting::at`] either way —
/// what differs is what runs next, and there is no third answer: an instruction
/// left an operand there, or a builtin left its answer slot there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Then {
    /// The instruction that wanted it runs again, with the primitive in the
    /// operand the object was in.
    Instruction,
    /// The builtin that asked for it runs again, with the primitive as its
    /// answer.
    Builtin,
}

/// Which of a conversion's two kinds of call is outstanding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    /// A getter was called to *find* the method, so what it answers **is** the
    /// method — `valueOf` is allowed to be an accessor.
    Fetching,
    /// The method itself was called, so what it answers is the primitive, or is
    /// an object and the search carries on.
    Calling,
}

/// One program, with its constants made and its keys interned.
#[derive(Debug)]
pub(crate) struct Loaded {
    /// The program.
    pub(crate) unit: Rc<Unit>,
    /// Where its constants begin in the run's list of them.
    pub(crate) offset: usize,
    /// Its texts as property keys, in the same order.
    pub(crate) keys: Vec<Key>,
}

/// One call that has not returned.
#[derive(Debug)]
pub(crate) struct Frame {
    /// Which loaded program its code is in.
    pub(crate) unit: usize,
    /// Which chunk of that program.
    pub(crate) chunk: u32,
    /// The environment in force, held until it is left or the call returns.
    ///
    /// It is the one the call was given until an [`Op::PushEnvironment`] goes
    /// into a block, and the ones it was made under are reached through its
    /// parent link rather than kept here. [`None`] is the script's own frame
    /// before it enters any block, whose names are the realm's.
    ///
    /// [`Op::PushEnvironment`]: crate::code::Op
    pub(crate) environment: Option<Root>,
    /// How many block environments this call has pushed and not popped, which
    /// is what makes an unbalanced pop this engine's own mistake rather than a
    /// walk off the end of the chain.
    pub(crate) environments: u32,
    /// Where the callee sits in the stack. Everything from here up is this
    /// call's, and a `return` truncates to it.
    pub(crate) callee_at: usize,
    /// Where its `this` sits, which is always just above the callee.
    pub(crate) this_at: usize,
    /// Where its frame slots begin.
    pub(crate) locals_at: usize,
    /// Where its operands begin.
    pub(crate) base: usize,
    /// Which instruction it is on.
    pub(crate) pc: usize,
    /// Which instruction it is running — or, for a frame that is not the
    /// innermost, which instruction made the call it is waiting on.
    ///
    /// Not `pc` less one: an instruction that needs an object turned into a
    /// primitive rewinds `pc` to itself so that it runs again, and a throw
    /// from the `valueOf` it called would then be judged by the instruction
    /// before. This is what a `try` is looked for by (queue item 210).
    pub(crate) now: usize,
    /// What the caller wanted this call's answer for.
    pub(crate) after: After,
}

/// One builtin that has been entered and has not answered (queue item 219).
///
/// A builtin gets no [`Frame`], because there are no instructions to be part
/// way through: what it is part way through is *its own Rust*, and the honest
/// record of that is the step it named. Everything else here is where its
/// things are, which is the same shape a frame keeps and for the same reason —
/// **nothing that could be collected is held in this struct**. Its `this`, its
/// arguments, the values it keeps (ADR 0031) and the answer it is waiting for
/// are all on the stack.
///
/// [`Body`] is a function pointer, so it is a number too: a builtin holds no
/// edge (see [`native`](crate::object::native)), which is what makes this
/// copyable and traceless.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Waiting {
    /// The Rust to run.
    pub(crate) body: Body,
    /// Where its callee sits, with its `this` and arguments above.
    pub(crate) callee_at: usize,
    /// How many arguments it was called with.
    pub(crate) argc: usize,
    /// How many values it keeps, in the slots from [`Waiting::kept_at`] up.
    pub(crate) kept: usize,
    /// The byte offset it was called from, for a message.
    pub(crate) at: usize,
    /// What its own answer is for, which is what a `return` would have carried.
    pub(crate) after: After,
    /// Which step of its body runs next.
    pub(crate) step: u32,
    /// Whether its body runs next, rather than a call it is waiting for.
    pub(crate) ready: bool,
    /// What the slot at [`Waiting::answer_at`] holds for it.
    pub(crate) answer: Slot,
    /// Whether the call it is waiting on was asked for with
    /// [`Want::Report`](crate::object::native::Want), so that a throw nothing
    /// inside that call catches stops here, is set aside for the embedder, and
    /// answers it `undefined` (ADR 0018 § 3). True only while that call runs.
    pub(crate) reporting: bool,
    /// Whether the call it is waiting on was asked for with
    /// [`Want::Catch`](crate::object::native::Want), so that a throw nothing
    /// inside that call catches stops here and is its answer (queue item
    /// 333). True only while that call runs.
    pub(crate) catching: bool,
}

/// What a waiting builtin's answer slot holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Slot {
    /// Nothing yet: it has asked for nothing, or is waiting on what it asked.
    Empty,
    /// What the call or conversion it asked for answered.
    Answered,
    /// The `undefined` a reported throw left, in place of an answer.
    Reported,
    /// What a caught call threw, in place of an answer.
    Caught,
}

impl Waiting {
    /// Where the first value it keeps is: directly above its arguments, which
    /// is where the stack ended when it was entered.
    pub(crate) const fn kept_at(&self) -> usize {
        self.callee_at.saturating_add(2).saturating_add(self.argc)
    }

    /// Where the answer to whatever it asked for is written.
    ///
    /// Directly above the values it keeps, which is where the stack ends once
    /// they are reserved — so a builtin's whole region is `callee | this | args
    /// | kept | answer` and nothing it asked for can disturb what is below.
    pub(crate) const fn answer_at(&self) -> usize {
        self.kept_at().saturating_add(self.kept)
    }
}

/// One run of one program.
#[derive(Debug)]
pub(crate) struct Run {
    /// The value stack, which is a cell.
    pub(crate) stack: Ref,
    /// The string constants of every loaded program, one after another.
    pub(crate) constants: Ref,
    /// The programs this run has entered.
    pub(crate) units: Vec<Loaded>,
    /// The calls that have not returned, the running one last.
    pub(crate) frames: Vec<Frame>,
    /// The builtins that have not answered, the innermost last.
    pub(crate) builtins: Vec<Waiting>,
}

impl Run {
    /// How many calls are on the stack, of either kind.
    ///
    /// A builtin waiting for the call it asked for is as much a call that has
    /// not returned as a frame is, and it costs the same kind of memory — so
    /// [`bounds::CALLS_ON_THE_STACK`](crate::bounds) counts both. Counting only
    /// frames would let a builtin that asks for itself run until the *value*
    /// bound caught it, which under-counts what it is bounding.
    pub(crate) fn calls(&self) -> usize {
        self.frames.len().saturating_add(self.builtins.len())
    }

    /// The builtin whose body runs next, if one is ready.
    pub(crate) fn ready(&self) -> Option<Waiting> {
        self.builtins
            .last()
            .copied()
            .filter(|waiting| waiting.ready)
    }

    /// Say that the innermost builtin's answer has arrived.
    pub(crate) fn answered(&mut self) -> Result<(), Escape> {
        let waiting = self
            .builtins
            .last_mut()
            .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
        waiting.ready = true;
        // The call it asked for is over, so a throw from here on is the
        // builtin's own, and goes where any builtin's throw goes.
        waiting.reporting = false;
        waiting.catching = false;
        Ok(())
    }

    /// The call that is running.
    pub(crate) fn frame(&self) -> Result<&Frame, Escape> {
        self.frames
            .last()
            .ok_or(Escape::Broken(Internal::StackIsWrong))
    }

    /// The same, to move its program counter.
    pub(crate) fn frame_mut(&mut self) -> Result<&mut Frame, Escape> {
        self.frames
            .last_mut()
            .ok_or(Escape::Broken(Internal::StackIsWrong))
    }

    /// The program the running call's code is in.
    pub(crate) fn loaded(&self) -> Result<&Loaded, Escape> {
        self.units
            .get(self.frame()?.unit)
            .ok_or(Escape::Broken(Internal::StackIsWrong))
    }

    /// The chunk it is running.
    pub(crate) fn chunk(&self) -> Result<&Chunk, Escape> {
        let frame = self.frame()?;
        self.loaded()?
            .unit
            .chunk(frame.chunk)
            .ok_or(Escape::Broken(Internal::JumpIsWrong))
    }

    /// Where the running call's operands begin.
    pub(crate) fn base(&self) -> Result<usize, Escape> {
        Ok(self.frame()?.base)
    }

    /// Whether the code running is strict, which decides what an assignment to
    /// an unresolvable name and a failed write both do.
    pub(crate) fn strict(&self) -> Result<bool, Escape> {
        Ok(self.chunk()?.strict())
    }

    /// The property key a text index names.
    pub(crate) fn key(&self, which: u32) -> Result<Key, Escape> {
        let at = usize::try_from(which).map_err(|_| Escape::Broken(Internal::StackIsWrong))?;
        self.loaded()?
            .keys
            .get(at)
            .copied()
            .ok_or(Escape::Broken(Internal::StackIsWrong))
    }

    /// The code units a text index names.
    pub(crate) fn text(&self, which: u32) -> Result<Vec<u16>, Escape> {
        self.loaded()?
            .unit
            .text(which)
            .map(<[u16]>::to_vec)
            .ok_or(Escape::Broken(Internal::StackIsWrong))
    }

    /// Where in the run's list of constants a text index's string is.
    pub(crate) fn constant(&self, which: u32) -> Result<usize, Escape> {
        let at = usize::try_from(which).map_err(|_| Escape::Broken(Internal::StackIsWrong))?;
        let loaded = self.loaded()?;
        if at >= loaded.keys.len() {
            return Err(Escape::Broken(Internal::StackIsWrong));
        }
        Ok(loaded.offset.saturating_add(at))
    }

    /// Which loaded program this one is, if the run has entered it already.
    ///
    /// By identity rather than by value: two programs with the same
    /// instructions are still two, and comparing their code would be an
    /// expensive way to get a wrong answer for the strings.
    pub(crate) fn already_loaded(&self, unit: &Rc<Unit>) -> Option<usize> {
        self.units
            .iter()
            .position(|loaded| Rc::ptr_eq(&loaded.unit, unit))
    }
}
