/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Running the instructions, and the [`Engine`] that owns everything they need.
//!
//! # The stack is in the heap, which is the clause with the longest reach
//!
//! ADR 0014 § 2 names the places a live reference may be, and *the
//! interpreter's frames and its value stack* is one of them — *which for that
//! reason live in structures the collector walks rather than in Rust locals*.
//! So the stack is a [`Slots`](crate::object::Slots) cell, rooted for the run,
//! and every push and pop goes through the heap's barrier.
//!
//! That decides how every instruction here is written: **operands are read
//! where they lie and taken off only once the answer exists**. An instruction
//! that popped its two operands into Rust locals and then allocated a string
//! would be correct in every ordinary run and wrong under
//! [`Heap::stress`](crate::heap::Heap::stress), because the only reference to
//! one of them was the stack slot it had just given up. The test suite runs
//! every program both ways for exactly this reason.
//!
//! # The layout of one run's slots
//!
//! Slot zero is the **completion value** — what the program evaluates to —
//! which is in the stack rather than in a Rust local for the same reason as
//! everything else. Then the frame's own slots, one per binding the compiler
//! gave one to. Then the operands.
//!
//! # It does not recurse, and that is a property rather than an accident
//!
//! A tree-walking interpreter runs a script as deep as the script nests, so a
//! page chooses how much stack this process uses. A loop over a flat array of
//! instructions does not: the deepest expression in the world is a taller
//! *stack of values*, which is bounded by [`bounds::VALUES_ON_THE_STACK`] and
//! costs no frames at all.
//!
//! # Stopping is the embedder's, and there is no clock in here
//!
//! ADR 0013 § 4: *the interpreter is interruptible. A script that will not
//! finish is stopped by the embedder — the browser process decides that a tab
//! has stopped answering, which is a person's judgement about a page and not an
//! engine's timer.* [`Stop`] is that switch. It is checked on every **backward**
//! jump, which is the only way a program can run for ever, and it costs a read
//! of a flag per iteration rather than per instruction.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::abrupt::{Escape, Internal, Missing, Thrown};
use crate::ast::Program;
use crate::bounds;
use crate::code::{Chunk, Op};
use crate::compile::{self, Refusal};
use crate::convert::{self, Names};
use crate::heap::{Ref, Root};
use crate::object::{Found, Held, Key, Objects, Property, Set, Value};
use crate::operate::{self, Simple};
use crate::realm::{Realm, Resolved};

/// The switch an embedder throws to stop a script.
///
/// Cloneable and shared, because the thing that decides a tab has stopped
/// answering is not the thread the script is on. It carries no time and no
/// deadline: *when* is the browser process's judgement (ADR 0013 § 5 gives this
/// crate no clock at all), and this is only the answer.
#[derive(Debug, Clone, Default)]
pub struct Stop(Arc<AtomicBool>);

impl Stop {
    /// A switch that has not been thrown.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stop the script that is running, or the next one to run.
    pub fn ask(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Whether it has been asked for.
    pub fn asked(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// Put it back, so the engine may run something again.
    pub fn clear(&self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

/// Everything that did not go the way the program said.
#[derive(Debug, Clone, PartialEq)]
pub enum Trouble {
    /// It did not compile — which is not a run that failed but a run that never
    /// started.
    NotCompiled(Refusal),
    /// It ran, and ended some way other than with a value.
    Escaped(Escape),
}

impl std::fmt::Display for Trouble {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Trouble::NotCompiled(refusal) => refusal.fmt(out),
            Trouble::Escaped(escape) => escape.fmt(out),
        }
    }
}

/// An engine: one heap, one realm, and the things a run needs.
///
/// ADR 0013 § 7: one of these belongs to one event loop and is never touched
/// from another thread. A worker gets its own.
#[derive(Debug)]
pub struct Engine {
    objects: Objects,
    realm: Realm,
    names: Names,
    stop: Stop,
    /// What the last run answered, kept alive until the next one.
    ///
    /// Without this the value handed back would be a reference to a cell the
    /// next allocation may sweep: the stack that was holding it has gone, and an
    /// embedder holding a [`Value`] is not a root (ADR 0014 § 2).
    last: Option<Root>,
}

impl Engine {
    /// An engine with an empty heap and an empty realm.
    ///
    /// # Errors
    ///
    /// [`Escape::Full`] if the heap cannot hold a realm, which is a heap that
    /// was full before anything ran.
    pub fn new() -> Result<Self, Escape> {
        let mut objects = Objects::new();
        let realm = Realm::new(&mut objects).map_err(|why| Escape::refused(why, 0))?;
        let names = Names::new(&mut objects).map_err(|why| Escape::refused(why, 0))?;
        Ok(Self {
            objects,
            realm,
            names,
            stop: Stop::new(),
            last: None,
        })
    }

    /// The heap and the object model, for an embedder that has things to put in
    /// it.
    pub fn objects(&mut self) -> &mut Objects {
        &mut self.objects
    }

    /// The realm.
    pub fn realm(&mut self) -> &mut Realm {
        &mut self.realm
    }

    /// The global object, which is where an embedder's own things go.
    ///
    /// # Errors
    ///
    /// [`Escape::Broken`] if the engine has lost it, which is its own bug.
    pub fn global(&self) -> Result<Ref, Escape> {
        self.realm.global(&self.objects)
    }

    /// The switch that stops a script.
    pub fn stop(&self) -> Stop {
        self.stop.clone()
    }

    /// Compile a program and run it, answering what it evaluates to.
    ///
    /// # Errors
    ///
    /// [`Trouble`], which is either a program that did not compile or a run
    /// that ended some other way.
    pub fn evaluate(&mut self, program: &Program) -> Result<Value, Trouble> {
        let chunk = compile::compile(program).map_err(Trouble::NotCompiled)?;
        self.run(&chunk).map_err(Trouble::Escaped)
    }

    /// Run a compiled program.
    ///
    /// # Errors
    ///
    /// [`Escape`]: the script threw, the engine reached something it has not
    /// built, the heap filled, the embedder stopped it, or this engine has a
    /// bug.
    pub fn run(&mut self, chunk: &Chunk) -> Result<Value, Escape> {
        let stack = self
            .objects
            .slots()
            .map_err(|why| Escape::refused(why, 0))?;
        let stack = self.objects.heap_mut().root(stack);
        let constants = self
            .objects
            .slots()
            .map_err(|why| Escape::refused(why, 0))?;
        let constants = self.objects.heap_mut().root(constants);

        let outcome = self.run_rooted(chunk, &stack, &constants);

        // The value goes back to the caller, who is not a root: it is kept
        // alive here until the next run asks the same question.
        if let Ok(value) = outcome {
            self.keep(value);
        }
        self.objects.heap_mut().release(stack);
        self.objects.heap_mut().release(constants);
        outcome
    }

    /// Hold on to what a run answered.
    fn keep(&mut self, value: Value) {
        if let Some(root) = self.last.take() {
            self.objects.heap_mut().release(root);
        }
        if let Some(held) = value.reference() {
            self.last = Some(self.objects.heap_mut().root(held));
        }
    }

    /// The run itself, with its two lists already rooted.
    fn run_rooted(
        &mut self,
        chunk: &Chunk,
        stack: &Root,
        constants: &Root,
    ) -> Result<Value, Escape> {
        let stack = self.held(stack)?;
        let constants = self.held(constants)?;
        let keys = self.intern(chunk, constants)?;

        // Slot zero is the completion value; the frame's own slots follow it.
        let base = chunk.locals().saturating_add(1);
        self.objects
            .with_slots(stack, |slots, _| slots.grow_to(base))
            .ok_or_else(|| Escape::fault(crate::object::Fault::Gone))?;

        self.instantiate(chunk)?;

        let mut run = Run {
            chunk,
            keys,
            stack,
            constants,
            base,
            pc: 0,
        };
        self.walk(&mut run)?;
        match self.objects.slot(stack, 0) {
            Some(Held::Value(value)) => Ok(value),
            // Nothing wrote a completion, which the first instruction of every
            // compiled program does.
            Some(Held::Uninitialized) | None => Ok(Value::Undefined),
        }
    }

    /// What a root is holding, or this engine's own bug.
    fn held(&self, root: &Root) -> Result<Ref, Escape> {
        self.objects
            .heap()
            .holding(root)
            .ok_or_else(|| Escape::fault(crate::object::Fault::Gone))
    }

    /// Turn the chunk's texts into strings in the heap, and into keys.
    ///
    /// The two are made together on purpose. A [`Key`] holds a reference to the
    /// string that spells it and the intern table is **weak** (ADR 0014 § 11),
    /// so a key held only in this `Vec` would name a cell the next collection
    /// takes away. Interning first and keeping the *interned* string as the
    /// constant means the rooted list of constants is what keeps every key
    /// alive, and there is one string cell rather than two.
    fn intern(&mut self, chunk: &Chunk, constants: Ref) -> Result<Vec<Key>, Escape> {
        let mut keys = Vec::with_capacity(chunk.texts().len());
        for text in chunk.texts() {
            let key = self
                .objects
                .key(text)
                .map_err(|why| Escape::refused(why, 0))?;
            let held = match key.reference() {
                Some(held) => held,
                // An array index is a key with no string in it, so the constant
                // needs a string of its own — `a["0"]` is a key that allocates
                // nothing and `"0"` is a value that does.
                None => self
                    .objects
                    .text(text.clone())
                    .map_err(|why| Escape::refused(why, 0))?,
            };
            self.objects
                .with_slots(constants, |slots, _| slots.push(Value::Text(held)))
                .ok_or_else(|| Escape::fault(crate::object::Fault::Gone))?;
            keys.push(key);
        }
        Ok(keys)
    }

    /// What a script declares before any of it runs.
    ///
    /// `GlobalDeclarationInstantiation`, which is where a redeclaration across
    /// two scripts is refused — before either statement of either script has
    /// run.
    fn instantiate(&mut self, chunk: &Chunk) -> Result<(), Escape> {
        for name in chunk.vars() {
            let Some(units) = chunk.text(*name) else {
                return Err(Escape::Broken(Internal::StackIsWrong));
            };
            let units = units.to_vec();
            self.realm.declare_var(&mut self.objects, &units, 0)?;
        }
        for lexical in chunk.lexical() {
            let Some(units) = chunk.text(lexical.name) else {
                return Err(Escape::Broken(Internal::StackIsWrong));
            };
            let units = units.to_vec();
            self.realm
                .declare_lexical(&mut self.objects, &units, lexical.mutable, 0)?;
        }
        Ok(())
    }
}

/// One run of one chunk.
struct Run<'a> {
    chunk: &'a Chunk,
    keys: Vec<Key>,
    stack: Ref,
    constants: Ref,
    /// Where the operands begin: one for the completion value, then the frame's
    /// own slots.
    base: usize,
    pc: usize,
}

impl Engine {
    /// The loop.
    fn walk(&mut self, run: &mut Run) -> Result<(), Escape> {
        while let Some(op) = run.chunk.op(run.pc) {
            let at = run.chunk.at(run.pc);
            let next = run.pc.saturating_add(1);
            run.pc = next;
            self.step(run, op, at)?;
        }
        Ok(())
    }

    /// One instruction.
    ///
    /// One arm per instruction and nothing else in it: what an instruction
    /// *does* lives in a method below, so that this stays a list of what the
    /// machine can be told to do rather than a place things accumulate.
    fn step(&mut self, run: &mut Run, op: Op, at: usize) -> Result<(), Escape> {
        match op {
            Op::Text(which) => {
                let value = constant(&self.objects, run, which)?;
                self.push(run, value)?;
            }
            Op::Number(number) => self.push(run, Value::Number(number))?,
            Op::Undefined => self.push(run, Value::Undefined)?,
            Op::Null => self.push(run, Value::Null)?,
            Op::Bool(is) => self.push(run, Value::Bool(is))?,

            Op::Pop => {
                self.pop(run)?;
            }
            Op::Dup => self.duplicate(run, 1)?,
            Op::DupTwo => self.duplicate(run, 2)?,

            Op::Load(slot) => self.load(run, slot, at)?,
            Op::Store(slot) => self.store(run, slot, at)?,
            Op::Initialize(slot) => {
                let value = self.pop(run)?;
                self.write_slot(run, slot, value)?;
            }
            Op::Uninitialize(slot) => self.empty_slot(run, slot)?,
            Op::RefuseAssignment(which) => return Err(refuse(run, which, at)),

            Op::LoadGlobal(which) => self.load_global(run, which, at)?,
            Op::StoreGlobal(which) => self.store_global(run, which, at)?,
            Op::InitializeGlobal(which) => self.initialize_global(run, which)?,
            Op::TypeOfGlobal(which) => self.type_of_global(run, which, at)?,
            Op::DeleteGlobal(which) => self.delete_global(run, which)?,

            Op::GetNamed(which) => self.get_named(run, which, at)?,
            Op::GetKeyed => self.get_keyed(run, at)?,
            Op::SetNamed(which) => {
                let key = key(run, which)?;
                self.set_property(run, key, 1, at)?;
            }
            Op::SetKeyed => self.set_keyed(run, at)?,
            Op::DeleteNamed(which) => self.delete_named(run, which, at)?,
            Op::DeleteKeyed => self.delete_keyed(run, at)?,

            Op::Unary(simple) => self.one_operand(run, simple, at)?,
            Op::Binary(operator) => self.two_operands(run, operator, at)?,
            Op::TypeOf => {
                let value = self.peek(run, 0)?;
                let answer = self.type_of(value, at)?;
                self.replace(run, 1, answer)?;
            }
            Op::ToNumeric => self.make_numeric(run, at)?,
            Op::ToText => self.make_text(run, at)?,

            Op::Object => self.new_object(run, at)?,
            Op::DefineNamed(which) => self.define_named(run, which)?,
            Op::DefineKeyed => self.define_keyed(run, at)?,
            Op::SetPrototype => self.set_prototype(run)?,

            Op::Jump(to) => self.jump(run, to)?,
            Op::JumpIfFalse(to) => {
                let value = self.pop(run)?;
                if !convert::to_boolean(&self.objects, value) {
                    self.jump(run, to)?;
                }
            }
            Op::JumpIfFalseKeep(to) => self.keep_or_jump(run, to, When::Falsy)?,
            Op::JumpIfTrueKeep(to) => self.keep_or_jump(run, to, When::Truthy)?,
            Op::JumpIfNotNullishKeep(to) => self.keep_or_jump(run, to, When::NotNullish)?,
            Op::SkipTheChain(to) => {
                // The value stays either way: the rest of the chain reads from
                // it, and the end of the chain drops it.
                let value = self.peek(run, 0)?;
                if is_nullish(value) {
                    self.jump(run, to)?;
                }
            }

            Op::Complete => {
                let value = self.pop(run)?;
                self.write_completion(run, value)?;
            }
            Op::CompleteEmpty => self.write_completion(run, Value::Undefined)?,
            Op::Throw => {
                let value = self.pop(run)?;
                return Err(Escape::Thrown(Thrown::Value { value, at }));
            }
        }
        Ok(())
    }

    // --- The instructions with more than a line in them ---------------------

    /// Give a realm's lexical binding its first value.
    fn initialize_global(&mut self, run: &mut Run, which: u32) -> Result<(), Escape> {
        let name = units(run, which)?;
        let value = self.pop(run)?;
        self.realm.initialize(&mut self.objects, &name, value)
    }

    /// `delete a`, which only sloppy code may write.
    fn delete_global(&mut self, run: &mut Run, which: u32) -> Result<(), Escape> {
        let name = units(run, which)?;
        let went = self.realm.delete(&mut self.objects, &name)?;
        self.push(run, Value::Bool(went))
    }

    /// `a.b`.
    fn get_named(&mut self, run: &mut Run, which: u32, at: usize) -> Result<(), Escape> {
        let key = key(run, which)?;
        let object = self.peek(run, 0)?;
        let value = self.read(object, key, at)?;
        self.replace(run, 1, value)
    }

    /// `delete a.b`.
    fn delete_named(&mut self, run: &mut Run, which: u32, at: usize) -> Result<(), Escape> {
        let key = key(run, which)?;
        let object = self.peek(run, 0)?;
        let went = self.remove(object, key, run.chunk.strict(), at)?;
        self.replace(run, 1, Value::Bool(went))
    }

    /// What `++` and `--` do to the old value before they add to it.
    fn make_numeric(&mut self, run: &mut Run, at: usize) -> Result<(), Escape> {
        let value = self.peek(run, 0)?;
        let number = convert::to_number(&self.objects, &self.names, value, at)?;
        self.replace(run, 1, Value::Number(number))
    }

    /// What a template substitution does, which is not what `+` does.
    fn make_text(&mut self, run: &mut Run, at: usize) -> Result<(), Escape> {
        let value = self.peek(run, 0)?;
        let text = convert::to_text(&mut self.objects, &self.names, value, at)?;
        self.replace(run, 1, text)
    }

    /// Push second copies of the top `how_many` values, in the order they were.
    ///
    /// Every one is read **before** any is pushed, which is not fussiness: a
    /// copy pushed half way through changes what the next one would read, and
    /// `a[b] += 1` then reads its own object where it wanted its key.
    fn duplicate(&mut self, run: &mut Run, how_many: usize) -> Result<(), Escape> {
        let mut copies = Vec::with_capacity(how_many);
        for back in (0..how_many).rev() {
            copies.push(self.peek(run, back)?);
        }
        for value in copies {
            self.push(run, value)?;
        }
        Ok(())
    }

    /// `-a`, `!a`, `~a`, `+a`, `void a`.
    fn one_operand(&mut self, run: &mut Run, operator: Simple, at: usize) -> Result<(), Escape> {
        let value = self.peek(run, 0)?;
        let answer = operate::unary(&self.objects, &self.names, operator, value, at)?;
        self.replace(run, 1, answer)
    }

    /// Every operator that takes two values.
    fn two_operands(
        &mut self,
        run: &mut Run,
        operator: crate::ast::Binary,
        at: usize,
    ) -> Result<(), Escape> {
        // Both stay on the stack while the answer is computed, because
        // computing it may allocate — which is this file's whole discipline.
        let left = self.peek(run, 1)?;
        let right = self.peek(run, 0)?;
        let answer = operate::binary(&mut self.objects, &self.names, operator, left, right, at)?;
        self.replace(run, 2, answer)
    }

    /// `{}` — an object with no prototype and no properties.
    fn new_object(&mut self, run: &mut Run, at: usize) -> Result<(), Escape> {
        let object = self
            .objects
            .object(None)
            .map_err(|why| Escape::refused(why, at))?;
        self.push(run, Value::Object(object))
    }

    /// `{ a: 1 }`, one property of it.
    fn define_named(&mut self, run: &mut Run, which: u32) -> Result<(), Escape> {
        let key = key(run, which)?;
        let object = self.peek(run, 1)?;
        let value = self.peek(run, 0)?;
        self.define(object, key, value)?;
        self.pop(run)?;
        Ok(())
    }

    /// The three jumps that keep the value they tested when they are taken and
    /// drop it when they are not: `&&`, `||` and `??`.
    fn keep_or_jump(&mut self, run: &mut Run, to: u32, when: When) -> Result<(), Escape> {
        let value = self.peek(run, 0)?;
        let taken = match when {
            When::Truthy => convert::to_boolean(&self.objects, value),
            When::Falsy => !convert::to_boolean(&self.objects, value),
            When::NotNullish => !is_nullish(value),
        };
        if taken {
            return self.jump(run, to);
        }
        self.pop(run)?;
        Ok(())
    }

    /// Read a frame slot, which is a `ReferenceError` inside its dead zone.
    fn load(&mut self, run: &mut Run, slot: u32, at: usize) -> Result<(), Escape> {
        let value = match self.slot(run, slot)? {
            Held::Value(value) => value,
            Held::Uninitialized => return Err(dead_zone(run, slot, at)),
        };
        self.push(run, value)
    }

    /// Write one, leaving the value on the stack.
    fn store(&mut self, run: &mut Run, slot: u32, at: usize) -> Result<(), Escape> {
        if self.slot(run, slot)? == Held::Uninitialized {
            return Err(dead_zone(run, slot, at));
        }
        let value = self.peek(run, 0)?;
        self.write_slot(run, slot, value)
    }

    /// Put a slot back in its dead zone.
    fn empty_slot(&mut self, run: &mut Run, slot: u32) -> Result<(), Escape> {
        let at = slot_index(run, slot)?;
        let stack = run.stack;
        self.objects
            .with_slots(stack, |slots, barrier| slots.uninitialize(barrier, at))
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        Ok(())
    }

    /// Read a name the realm answers for.
    fn load_global(&mut self, run: &mut Run, which: u32, at: usize) -> Result<(), Escape> {
        let name = units(run, which)?;
        let value = match self.realm.resolve(&self.objects, &name)? {
            Resolved::Lexical(value) | Resolved::Property(value) => value,
            Resolved::Dead => {
                return Err(Escape::reference_error(
                    format!("'{}' is used before it is declared", show(&name)),
                    at,
                ));
            }
            Resolved::Nothing => {
                return Err(Escape::reference_error(
                    format!("'{}' is not defined", show(&name)),
                    at,
                ));
            }
        };
        self.push(run, value)
    }

    /// Write one, leaving the value on the stack.
    fn store_global(&mut self, run: &mut Run, which: u32, at: usize) -> Result<(), Escape> {
        let name = units(run, which)?;
        let value = self.peek(run, 0)?;
        let strict = run.chunk.strict();
        self.realm
            .assign(&mut self.objects, &name, value, strict, at)
    }

    /// `typeof` on a name, which a name nothing declares survives.
    fn type_of_global(&mut self, run: &mut Run, which: u32, at: usize) -> Result<(), Escape> {
        let name = units(run, which)?;
        let value = match self.realm.resolve(&self.objects, &name)? {
            Resolved::Lexical(value) | Resolved::Property(value) => value,
            Resolved::Dead => {
                // `typeof` saves a name nothing declared and does not save one
                // that is still in its dead zone, which is the one place the
                // two differ.
                return Err(Escape::reference_error(
                    format!("'{}' is used before it is declared", show(&name)),
                    at,
                ));
            }
            Resolved::Nothing => Value::Undefined,
        };
        let answer = self.type_of(value, at)?;
        self.push(run, answer)
    }

    /// `a[b]`.
    fn get_keyed(&mut self, run: &mut Run, at: usize) -> Result<(), Escape> {
        let object = self.peek(run, 1)?;
        let name = self.peek(run, 0)?;
        let key = convert::to_property_key(&mut self.objects, &self.names, name, at)?;
        let value = self.read(object, key, at)?;
        self.replace(run, 2, value)
    }

    /// `a.b = c` and `a[b] = c`, which differ only in how far down the object
    /// is.
    fn set_property(
        &mut self,
        run: &mut Run,
        key: Key,
        under: usize,
        at: usize,
    ) -> Result<(), Escape> {
        let object = self.peek(run, under)?;
        let value = self.peek(run, 0)?;
        self.write(object, key, value, run.chunk.strict(), at)?;
        self.replace(run, under.saturating_add(1), value)
    }

    /// `a[b] = c`.
    fn set_keyed(&mut self, run: &mut Run, at: usize) -> Result<(), Escape> {
        let name = self.peek(run, 1)?;
        let key = convert::to_property_key(&mut self.objects, &self.names, name, at)?;
        self.set_property(run, key, 2, at)
    }

    /// `delete a[b]`.
    fn delete_keyed(&mut self, run: &mut Run, at: usize) -> Result<(), Escape> {
        let object = self.peek(run, 1)?;
        let name = self.peek(run, 0)?;
        let key = convert::to_property_key(&mut self.objects, &self.names, name, at)?;
        let went = self.remove(object, key, run.chunk.strict(), at)?;
        self.replace(run, 2, Value::Bool(went))
    }

    /// `{ [a]: b }`.
    fn define_keyed(&mut self, run: &mut Run, at: usize) -> Result<(), Escape> {
        let object = self.peek(run, 2)?;
        let name = self.peek(run, 1)?;
        let value = self.peek(run, 0)?;
        let key = convert::to_property_key(&mut self.objects, &self.names, name, at)?;
        self.define(object, key, value)?;
        self.pop(run)?;
        self.pop(run)?;
        Ok(())
    }

    /// `{ __proto__: a }`.
    fn set_prototype(&mut self, run: &mut Run) -> Result<(), Escape> {
        let object = self.peek(run, 1)?;
        let value = self.peek(run, 0)?;
        if let Value::Object(object) = object {
            // Anything that is not an object and not `null` is ignored, which
            // is the literal's own rule rather than a refusal.
            match value {
                Value::Object(prototype) => {
                    self.objects.set_prototype(object, Some(prototype))?;
                }
                Value::Null => {
                    self.objects.set_prototype(object, None)?;
                }
                _ => {}
            }
        }
        self.pop(run)?;
        Ok(())
    }

    // --- The stack ----------------------------------------------------------

    /// Put a value on the stack.
    fn push(&mut self, run: &mut Run, value: Value) -> Result<(), Escape> {
        let stack = run.stack;
        let height = self
            .objects
            .slot_count(stack)
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        if height >= bounds::VALUES_ON_THE_STACK {
            return Err(Escape::range_error(
                "this script needs more values at once than this engine will hold",
                run.chunk.at(run.pc),
            ));
        }
        self.objects
            .with_slots(stack, |slots, _| slots.push(value))
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        Ok(())
    }

    /// Take the top value off.
    fn pop(&mut self, run: &mut Run) -> Result<Value, Escape> {
        let stack = run.stack;
        let height = self
            .objects
            .slot_count(stack)
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        if height <= run.base {
            // The compiler said there would be a value here. There is not, so
            // the compiler and this loop disagree, which is our bug.
            return Err(Escape::Broken(Internal::StackIsWrong));
        }
        match self
            .objects
            .with_slots(stack, |slots, _| slots.pop())
            .flatten()
        {
            Some(Held::Value(value)) => Ok(value),
            Some(Held::Uninitialized) | None => Err(Escape::Broken(Internal::StackIsWrong)),
        }
    }

    /// Read a value without taking it off: `back` is how far down, zero being
    /// the top.
    fn peek(&self, run: &Run, back: usize) -> Result<Value, Escape> {
        let height = self
            .objects
            .slot_count(run.stack)
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        let at = height
            .checked_sub(back.saturating_add(1))
            .filter(|at| *at >= run.base)
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        match self.objects.slot(run.stack, at) {
            Some(Held::Value(value)) => Ok(value),
            Some(Held::Uninitialized) | None => Err(Escape::Broken(Internal::StackIsWrong)),
        }
    }

    /// Take `how_many` values off and put one back.
    ///
    /// The order matters and is the whole discipline of this file: the answer
    /// is computed while the operands are still on the stack, so nothing it
    /// allocated on the way is holding a reference the collector cannot see.
    fn replace(&mut self, run: &mut Run, how_many: usize, value: Value) -> Result<(), Escape> {
        for _ in 0..how_many {
            self.pop(run)?;
        }
        self.push(run, value)
    }

    /// What a frame slot holds.
    fn slot(&self, run: &Run, slot: u32) -> Result<Held, Escape> {
        let at = slot_index(run, slot)?;
        self.objects
            .slot(run.stack, at)
            .ok_or(Escape::Broken(Internal::StackIsWrong))
    }

    /// Put a value in a frame slot.
    fn write_slot(&mut self, run: &mut Run, slot: u32, value: Value) -> Result<(), Escape> {
        let at = slot_index(run, slot)?;
        let stack = run.stack;
        self.objects
            .with_slots(stack, |slots, barrier| slots.set(barrier, at, value))
            .filter(|wrote| *wrote)
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        Ok(())
    }

    /// Say what the program evaluates to so far.
    fn write_completion(&mut self, run: &mut Run, value: Value) -> Result<(), Escape> {
        let stack = run.stack;
        self.objects
            .with_slots(stack, |slots, barrier| slots.set(barrier, 0, value))
            .filter(|wrote| *wrote)
            .ok_or(Escape::Broken(Internal::StackIsWrong))?;
        Ok(())
    }

    /// Go to an instruction, checking the embedder's switch on the way back.
    fn jump(&mut self, run: &mut Run, to: u32) -> Result<(), Escape> {
        let to = usize::try_from(to).map_err(|_| Escape::Broken(Internal::JumpIsWrong))?;
        if to > run.chunk.code().len() {
            return Err(Escape::Broken(Internal::JumpIsWrong));
        }
        // Backwards is the only way a program runs for ever, so it is the only
        // place worth asking whether somebody wants it to stop.
        if to <= run.pc && self.stop.asked() {
            return Err(Escape::Interrupted);
        }
        run.pc = to;
        Ok(())
    }

    // --- Names, keys and constants ------------------------------------------

    // --- Properties ---------------------------------------------------------

    /// `[[Get]]`, with the two answers this engine cannot give yet named.
    fn read(&mut self, object: Value, key: Key, at: usize) -> Result<Value, Escape> {
        let held = self.object_of(object, key, at, "read")?;
        match self.objects.get(held, key)? {
            Found::Missing => Ok(Value::Undefined),
            Found::Value(value) => Ok(value),
            // The property is there and reading it means calling a function.
            Found::Getter(_) => Err(Escape::NotBuiltYet(Missing::ACall)),
        }
    }

    /// `[[Set]]`, with sloppy mode's silence and strict mode's `TypeError`.
    fn write(
        &mut self,
        object: Value,
        key: Key,
        value: Value,
        strict: bool,
        at: usize,
    ) -> Result<(), Escape> {
        let held = self.object_of(object, key, at, "write")?;
        match self.objects.set(held, key, value)? {
            Set::Done => Ok(()),
            Set::Setter(_) => Err(Escape::NotBuiltYet(Missing::ACall)),
            Set::Refused => {
                if strict {
                    return Err(Escape::type_error(
                        format!("{} cannot be written", self.describe(key)),
                        at,
                    ));
                }
                Ok(())
            }
        }
    }

    /// `[[Delete]]`.
    fn remove(&mut self, object: Value, key: Key, strict: bool, at: usize) -> Result<bool, Escape> {
        let held = self.object_of(object, key, at, "delete")?;
        let went = self.objects.delete(held, key)?;
        if !went && strict {
            return Err(Escape::type_error(
                format!("{} cannot be deleted", self.describe(key)),
                at,
            ));
        }
        Ok(went)
    }

    /// `[[DefineOwnProperty]]` with a plain data property, which is what an
    /// object literal writes.
    fn define(&mut self, object: Value, key: Key, value: Value) -> Result<(), Escape> {
        let Value::Object(held) = object else {
            // Only an object literal defines, and it defines on the object it
            // has just made.
            return Err(Escape::Broken(Internal::StackIsWrong));
        };
        self.objects.define(held, key, Property::plain(value))?;
        Ok(())
    }

    /// The object a property access is on, or the error it is instead.
    ///
    /// Two different answers, and keeping them apart is ADR 0013 § 3 in the
    /// place it matters most: `null.a` is a `TypeError` **the language
    /// specifies** and a page may catch, and `"abc".a` is a wrapper object this
    /// engine has not built (queue item 73) — which is not an error the language
    /// has, and must not be reported as one.
    fn object_of(&self, object: Value, key: Key, at: usize, doing: &str) -> Result<Ref, Escape> {
        match object {
            Value::Object(held) => Ok(held),
            Value::Undefined | Value::Null => Err(Escape::type_error(
                format!(
                    "cannot {doing} {} of {}",
                    self.describe(key),
                    if matches!(object, Value::Null) {
                        "null"
                    } else {
                        "undefined"
                    }
                ),
                at,
            )),
            Value::Bool(_) | Value::Number(_) | Value::Text(_) | Value::Symbol(_) => {
                Err(Escape::NotBuiltYet(Missing::AWrapperObject))
            }
        }
    }

    /// A key, for a message a person reads.
    fn describe(&self, key: Key) -> String {
        if let Some(index) = key.as_index() {
            return format!("property '{index}'");
        }
        match key.as_text().and_then(|held| self.objects.units(held)) {
            Some(units) => format!("property '{}'", show(units)),
            None => "that property".to_owned(),
        }
    }

    /// `typeof`, as the string it answers with.
    fn type_of(&mut self, value: Value, at: usize) -> Result<Value, Escape> {
        let answer = convert::type_of(value);
        let units: Vec<u16> = answer.encode_utf16().collect();
        // Interned rather than allocated: there are seven of these strings in
        // this engine's language and eight in the finished one, and a page may
        // ask in a loop.
        let key = self
            .objects
            .key(&units)
            .map_err(|why| Escape::refused(why, at))?;
        match key.reference() {
            Some(held) => Ok(Value::Text(held)),
            None => Err(Escape::Broken(Internal::StackIsWrong)),
        }
    }
}

/// The `TypeError` for an assignment to a `const` the compiler could see.
fn refuse(run: &Run, which: u32, at: usize) -> Escape {
    match units(run, which) {
        Ok(name) => Escape::type_error(
            format!("'{}' is a constant and cannot be assigned to", show(&name)),
            at,
        ),
        Err(why) => why,
    }
}

/// Where a frame slot is in the stack: past the completion value.
fn slot_index(run: &Run, slot: u32) -> Result<usize, Escape> {
    let slot = usize::try_from(slot).map_err(|_| Escape::Broken(Internal::StackIsWrong))?;
    if slot >= run.chunk.locals() {
        return Err(Escape::Broken(Internal::StackIsWrong));
    }
    Ok(slot.saturating_add(1))
}

/// Which way a keeping jump goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum When {
    /// `a || b` jumps when `a` is truthy.
    Truthy,
    /// `a && b` jumps when `a` is falsy.
    Falsy,
    /// `a ?? b` jumps when `a` is neither `null` nor `undefined`.
    NotNullish,
}

/// The string constant at an index.
fn constant(objects: &Objects, run: &Run, which: u32) -> Result<Value, Escape> {
    let at = usize::try_from(which).map_err(|_| Escape::Broken(Internal::StackIsWrong))?;
    match objects.slot(run.constants, at) {
        Some(Held::Value(value)) => Ok(value),
        Some(Held::Uninitialized) | None => Err(Escape::Broken(Internal::StackIsWrong)),
    }
}

/// The key an index names.
fn key(run: &Run, which: u32) -> Result<Key, Escape> {
    let at = usize::try_from(which).map_err(|_| Escape::Broken(Internal::StackIsWrong))?;
    run.keys
        .get(at)
        .copied()
        .ok_or(Escape::Broken(Internal::StackIsWrong))
}

/// The code units an index names.
fn units(run: &Run, which: u32) -> Result<Vec<u16>, Escape> {
    run.chunk
        .text(which)
        .map(<[u16]>::to_vec)
        .ok_or(Escape::Broken(Internal::StackIsWrong))
}

/// The `ReferenceError` for a binding read before it was declared, naming it
/// where the compiler recorded a name.
///
/// A slot the compiler took for itself — a `switch`'s discriminant, the old
/// value of an `a.b++` — has no name, and nothing a script can write reads one
/// before it is written. So the nameless message is unreachable rather than
/// vague, and it is here because *unreachable* is not a thing to prove with a
/// panic.
fn dead_zone(run: &Run, slot: u32, at: usize) -> Escape {
    match run.chunk.slot_name(slot) {
        Some(units) => Escape::reference_error(
            format!("'{}' is used before it is declared", show(units)),
            at,
        ),
        None => Escape::reference_error("a binding is used before it is declared", at),
    }
}

/// Whether a value is `null` or `undefined`, which is what `?.` and `??` ask.
fn is_nullish(value: Value) -> bool {
    matches!(value, Value::Undefined | Value::Null)
}

/// Code units as text, for a message.
fn show(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}
