/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The instructions a program becomes.
//!
//! ADR 0013 § 2: **bytecode from the first line of the compiler**, because a
//! frame that can be suspended and resumed is the shape generators, `async` and
//! a debugger all need, and it is not a thing to retrofit into a tree walker.
//! This file is the instruction set and [`Chunk`] is what a program compiles to.
//!
//! # An enum rather than bytes, and why that is still bytecode
//!
//! The decision ADR 0013 makes is about the *shape* of execution — a flat array
//! of instructions with a program counter, rather than a walk of the tree — and
//! that is what this is. Whether an instruction is a byte with operands after it
//! or a variant with fields in it is a question about size and dispatch speed,
//! which is a measurement nobody has taken (law 3, and `LOOP.md`: a claim about
//! speed is measured on hardware or not made). An enum is the representation
//! that can be read while the semantics are being settled, and packing it into
//! bytes later changes no behaviour — the same argument ADR 0013 § 2 makes for
//! [`Value`](crate::object::Value) being an enum.
//!
//! # What is *not* in an instruction
//!
//! **No heap references.** A chunk is compiled with no heap in sight and could
//! be compiled once and run in two realms, so a string constant is code units
//! here and becomes a cell when a run starts. That is also what keeps a chunk
//! outside the collector's business entirely: there is no edge in it to trace.
//!
//! **No source text.** Every instruction carries the byte offset it came from
//! ([`Chunk::at`]) and nothing else, which is what a `ReferenceError` points at
//! today and what a stack trace (queue item 78) will be built from.

use crate::ast::Binary;
use crate::operate::Simple;

/// One instruction.
///
/// The operand stack is where values are, and each variant says what it takes
/// off it and what it leaves. A `u32` operand is an index — into
/// [`Chunk::texts`], into the frame's slots, or into the code itself for a jump
/// — and never a heap reference.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    /// Push the string constant at this index.
    Text(u32),
    /// Push a number.
    Number(f64),
    /// Push `undefined`.
    Undefined,
    /// Push `null`.
    Null,
    /// Push `true` or `false`.
    Bool(bool),

    /// Drop the top of the stack.
    Pop,
    /// Push a second copy of the top of the stack.
    Dup,
    /// Push second copies of the top **two**, in the order they were:
    /// `a b` becomes `a b a b`.
    DupTwo,

    /// Read a frame slot, which is a `ReferenceError` if it is still in its
    /// dead zone.
    Load(u32),
    /// Write a frame slot, leaving the value on the stack — an assignment is an
    /// expression, and `a = (b = 1)` is why.
    Store(u32),
    /// Give a frame slot its first value, taking it off the stack. This is what
    /// ends a binding's dead zone.
    Initialize(u32),
    /// Put a frame slot back in its dead zone, which is what entering a block
    /// again does to the bindings it declares.
    Uninitialize(u32),
    /// Refuse an assignment to a `const`, naming it. The value has already been
    /// evaluated, because the language evaluates it before it complains.
    RefuseAssignment(u32),

    /// Read a name that is not a frame slot: a realm's lexical binding, then a
    /// property of the global object, then a `ReferenceError`.
    LoadGlobal(u32),
    /// Write one, with the same order and with sloppy mode's rule that an
    /// unresolvable name becomes a global property.
    StoreGlobal(u32),
    /// Give a realm's lexical binding its first value, ending its dead zone.
    /// This is what a top-level `let` or `const` does, and it is a different
    /// instruction from [`Op::StoreGlobal`] because a `const` refuses that one.
    InitializeGlobal(u32),
    /// `typeof` on a name, which is `"undefined"` rather than a
    /// `ReferenceError` when the name resolves to nothing. This is the whole
    /// reason it is an instruction rather than [`Op::TypeOf`] after a load.
    TypeOfGlobal(u32),
    /// `delete` on a name, which sloppy code may write and strict code may not.
    DeleteGlobal(u32),

    /// `a.b` — the object is on the stack.
    GetNamed(u32),
    /// `a[b]` — the object and the key are on the stack.
    GetKeyed,
    /// `a.b = c` — the object and the value are on the stack, and the value
    /// stays.
    SetNamed(u32),
    /// `a[b] = c` — the object, the key and the value are on the stack.
    SetKeyed,
    /// `delete a.b`, which answers whether the property went.
    DeleteNamed(u32),
    /// `delete a[b]`.
    DeleteKeyed,

    /// `-a`, `!a`, `~a`, `+a`, `void a`.
    Unary(Simple),
    /// Every operator that takes two values.
    Binary(Binary),
    /// `typeof` on a value that is already on the stack.
    TypeOf,
    /// `ToNumeric`, which is what `++` and `--` do to the old value before they
    /// add to it — and which is why `a = "1"; a++` leaves a number behind.
    ToNumeric,
    /// `ToString`, which a template substitution does and `+` does **not**:
    /// `` `${a}` `` asks an object for its `toString` first and `"" + a` asks
    /// for its `valueOf` first, and a page can tell.
    ToText,

    /// Push a new object with no prototype and no properties.
    Object,
    /// Define an own property of the object below the value, leaving the
    /// object. This is an object literal's `a: 1`, which *defines* rather than
    /// sets: a setter on a prototype does not see it.
    DefineNamed(u32),
    /// The same with the key on the stack: `{ [a]: 1 }`.
    DefineKeyed,
    /// `{ __proto__: a }`, which is the one property name in an object literal
    /// that is not a property at all.
    SetPrototype,

    /// Go to this instruction.
    Jump(u32),
    /// Go there if the top of the stack is falsy, taking it off either way.
    JumpIfFalse(u32),
    /// Go there if it is falsy, **keeping** it; otherwise take it off. `a && b`.
    JumpIfFalseKeep(u32),
    /// Go there if it is truthy, keeping it. `a || b`.
    JumpIfTrueKeep(u32),
    /// Go there if it is neither `null` nor `undefined`, keeping it; otherwise
    /// take it off. `a ?? b`.
    JumpIfNotNullishKeep(u32),
    /// `a?.b`: if the top of the stack is `null` or `undefined`, go to the end
    /// of the chain. The value **stays either way**, which is what makes this a
    /// different instruction from the one above rather than a spelling of it:
    /// the rest of the chain needs the object it is reading from, and the end of
    /// the chain needs something to drop.
    SkipTheChain(u32),

    /// This value is what the program evaluates to, unless something later says
    /// otherwise.
    Complete,
    /// Nothing produced a value here, which is what an `if` with an empty
    /// branch does to a program's completion.
    CompleteEmpty,
    /// `throw a`.
    Throw,
}

/// A compiled program.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chunk {
    code: Vec<Op>,
    at: Vec<usize>,
    texts: Vec<Vec<u16>>,
    locals: usize,
    slot_names: Vec<Option<u32>>,
    vars: Vec<u32>,
    lexical: Vec<Lexical>,
    strict: bool,
}

/// One `let` or `const` a program declares at its top level.
///
/// These do not become frame slots: they belong to the **realm**, because a
/// second script in the same page sees them and a frame does not outlive its
/// script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lexical {
    /// Which text names it.
    pub name: u32,
    /// Whether it may be assigned to again — `let` may, `const` may not.
    pub mutable: bool,
}

impl Chunk {
    /// An empty chunk for a program of this strictness.
    pub fn new(strict: bool) -> Self {
        Self {
            strict,
            ..Self::default()
        }
    }

    /// The instructions.
    pub fn code(&self) -> &[Op] {
        &self.code
    }

    /// The instruction at `pc`.
    pub fn op(&self, pc: usize) -> Option<Op> {
        self.code.get(pc).copied()
    }

    /// The byte offset in the source that the instruction at `pc` came from.
    ///
    /// Zero for an instruction nothing recorded a place for, which is a chunk
    /// somebody built by hand rather than one the compiler made.
    pub fn at(&self, pc: usize) -> usize {
        self.at.get(pc).copied().unwrap_or_default()
    }

    /// The string constants, in the order the instructions index them.
    pub fn texts(&self) -> &[Vec<u16>] {
        &self.texts
    }

    /// The text at an index, for a message that names it.
    pub fn text(&self, at: u32) -> Option<&[u16]> {
        self.texts.get(usize::try_from(at).ok()?).map(Vec::as_slice)
    }

    /// How many slots a frame running this needs, above the one the completion
    /// value lives in.
    pub fn locals(&self) -> usize {
        self.locals
    }

    /// The `var` names to put on the global object before anything runs.
    pub fn vars(&self) -> &[u32] {
        &self.vars
    }

    /// The `let` and `const` names to declare in the realm before anything
    /// runs.
    pub fn lexical(&self) -> &[Lexical] {
        &self.lexical
    }

    /// Whether this is strict code, which changes what an assignment to an
    /// unresolvable name does.
    pub fn strict(&self) -> bool {
        self.strict
    }

    /// Add an instruction, saying where in the source it came from, and answer
    /// where it landed.
    pub fn emit(&mut self, op: Op, at: usize) -> usize {
        self.code.push(op);
        self.at.push(at);
        self.code.len().saturating_sub(1)
    }

    /// Where the next instruction will land, which is what a jump is patched
    /// to.
    pub fn here(&self) -> usize {
        self.code.len()
    }

    /// Point a jump that was emitted before its target was known at `to`.
    ///
    /// Answers whether it could: a jump index that is not an instruction, or a
    /// target past what a `u32` can name, is the compiler's own mistake and is
    /// reported rather than written.
    pub fn patch(&mut self, jump: usize, to: usize) -> bool {
        let Ok(target) = u32::try_from(to) else {
            return false;
        };
        let Some(op) = self.code.get_mut(jump) else {
            return false;
        };
        match op {
            Op::Jump(there)
            | Op::JumpIfFalse(there)
            | Op::JumpIfFalseKeep(there)
            | Op::JumpIfTrueKeep(there)
            | Op::JumpIfNotNullishKeep(there)
            | Op::SkipTheChain(there) => {
                *there = target;
                true
            }
            _ => false,
        }
    }

    /// The index of these code units among the constants, adding them if they
    /// are new.
    ///
    /// Shared rather than repeated, which matters more than it looks: every
    /// property name and every global name is a text, so a loop reading `a.b`
    /// has one entry rather than one per instruction, and the run that turns
    /// texts into heap strings makes one string.
    pub fn text_index(&mut self, units: &[u16]) -> Option<u32> {
        if let Some(at) = self.texts.iter().position(|held| held == units) {
            return u32::try_from(at).ok();
        }
        let at = u32::try_from(self.texts.len()).ok()?;
        self.texts.push(units.to_vec());
        Some(at)
    }

    /// The name of a frame slot, where it has one.
    ///
    /// A binding's slot does; a slot the compiler took to hold a `switch`'s
    /// discriminant or the old value of an `a.b++` does not. It is here so that
    /// a `ReferenceError` about a dead zone can say *which* binding, which is
    /// the difference between a message somebody can act on and one they cannot.
    pub fn slot_name(&self, slot: u32) -> Option<&[u16]> {
        let which = *self.slot_names.get(usize::try_from(slot).ok()?)?;
        self.text(which?)
    }

    /// Say which name a slot holds.
    pub fn name_slot(&mut self, slot: u32, name: u32) {
        if let Some(held) = self
            .slot_names
            .get_mut(usize::try_from(slot).unwrap_or(usize::MAX))
        {
            *held = Some(name);
        }
    }

    /// Take a frame slot, which is never given back — a block that has ended
    /// keeps its slot rather than lending it to the next one.
    ///
    /// Reusing them is a saving of memory a script chose the size of, and the
    /// bound on how many there can be is the tree's own depth
    /// ([`bounds::DEEPEST_EXPRESSION`](crate::bounds)): a program cannot nest
    /// blocks deeper than the parser would build them.
    pub fn take_slot(&mut self) -> Option<u32> {
        let at = u32::try_from(self.locals).ok()?;
        self.locals = self.locals.saturating_add(1);
        self.slot_names.push(None);
        Some(at)
    }

    /// Record a `var` this program declares.
    pub fn declare_var(&mut self, name: u32) {
        if !self.vars.contains(&name) {
            self.vars.push(name);
        }
    }

    /// Record a top-level `let` or `const`.
    pub fn declare_lexical(&mut self, name: u32, mutable: bool) {
        self.lexical.push(Lexical { name, mutable });
    }
}

#[cfg(test)]
mod tests {
    use super::{Chunk, Op};

    #[test]
    fn a_text_is_kept_once_however_often_it_is_named() {
        let mut chunk = Chunk::new(false);
        let units: Vec<u16> = "a".encode_utf16().collect();
        assert_eq!(chunk.text_index(&units), Some(0));
        assert_eq!(chunk.text_index(&units), Some(0));
        assert_eq!(chunk.texts().len(), 1);
    }

    #[test]
    fn a_jump_is_patched_and_anything_else_refuses_to_be() {
        let mut chunk = Chunk::new(false);
        let jump = chunk.emit(Op::Jump(0), 7);
        let other = chunk.emit(Op::Pop, 8);
        assert!(chunk.patch(jump, 12));
        assert_eq!(chunk.op(jump), Some(Op::Jump(12)));
        assert!(!chunk.patch(other, 12), "a Pop is not a jump");
        assert!(!chunk.patch(99, 12), "and neither is nothing");
        assert_eq!(chunk.at(jump), 7);
    }
}
