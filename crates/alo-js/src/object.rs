/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a script's things **are**, in the heap item 71 built for them.
//!
//! ADR 0014 § 11, and queue item 206. Item 71 stopped exactly here on purpose:
//! [`Heap<T>`](crate::heap::Heap) is generic in what a cell is, because the
//! collector's business is *reachability* and an object's business is
//! prototypes, properties and an order a page can observe. This module is the
//! second half, and nothing in `heap.rs` changed to let it in.
//!
//! # The five decisions, and where each one lives
//!
//! - **An ordinary object is a prototype, a property table and one flag** —
//!   [`ordinary`], and a property is data or accessor with three attributes and
//!   no fourth ([`property`]).
//! - **Property order is observable, so it is the specification's order from
//!   the first line** — [`table`], where it is a property of the storage rather
//!   than a sort done at enumeration time.
//! - **Internal methods are a trait, and it is the same trait an embedder
//!   gets** — [`internal`]. An array, a proxy and an `HTMLCollection` are one
//!   mechanism rather than two.
//! - **Property keys are interned and the intern table is weak** — [`key`] and
//!   [`intern`]. A page can mint unbounded distinct names, so a strong table
//!   would be a leak a stranger controls.
//! - **A string is a heap object, immutable once made, in UTF-16 code units** —
//!   [`text`].
//!
//! [`access`] is the sixth clause — *one interface for get, set, define, delete
//! and own keys* — and it is the file the rest of the engine talks to.
//!
//! # [`Objects`] is the heap plus the one thing that must sit beside it
//!
//! The intern table cannot live inside a cell: interning a name means comparing
//! it against the names already in the heap, and a cell cannot read the heap it
//! is in. So [`Objects`] owns both, and it is the type item 72's interpreter
//! will hold.
//!
//! # What is absent rather than approximate
//!
//! ADR 0013 § 3. There are no builtins here, no realm and no global object
//! (item 73) and there is no `BigInt` (item 207). A [`Function`] is here — it
//! is an ordinary object with a `[[Call]]`'s worth of code beside it (item 209)
//! — and so is the [`Environment`] a closure keeps, the [`Array`] whose
//! `length` keeps up with its indices (item 225), an error, which is an
//! ordinary object with one slot that holds nothing (item 227), and an array
//! iterator, which is an ordinary object with a generator's state beside it
//! (item 230), and a `RegExp`, which is an ordinary object with a compiled
//! pattern beside it (item 74). Everything else is absent
//! rather than stubbed, because a stub is the one answer that defeats a page's
//! own feature test.

pub mod access;
pub mod array;
pub mod array_iterator;
pub mod cell;
pub mod environment;
pub mod function;
pub mod intern;
pub mod internal;
pub mod key;
pub mod native;
pub mod ordinary;
pub mod property;
pub mod regexp;
pub mod slots;
pub mod symbol;
pub mod table;
pub mod text;
pub mod value;

use std::fmt;
use std::rc::Rc;

use crate::heap::{Barrier, Full, Heap, Ref};
use crate::unit::Unit;

pub use access::{Fault, Found, Named, Set};
pub use array::Array;
pub use array_iterator::ArrayIterator;
pub use cell::Cell;
pub use environment::Environment;
pub use function::{Code, Function};
pub use intern::Interner;
pub use internal::{Exotic, Internal, Typed};
pub use key::Key;
pub use native::Native;
pub use ordinary::Ordinary;
pub use property::Property;
pub use regexp::RegExp;
pub use slots::{Held, Slots};
pub use symbol::Symbol;
pub use table::Properties;
pub use text::Text;
pub use value::{Stored, Value};

/// Something a script asked for that it may not have.
///
/// Two refusals rather than one, because the language distinguishes them and a
/// page can tell: a string too long is a `RangeError` the script's own `catch`
/// can survive, and a heap at its ceiling is ADR 0014 § 9's [`Full`], which
/// goes to the embedder and stops the tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The heap is at its ceiling and a collection did not bring it under.
    Full(Full),
    /// A string longer than [`bounds::LONGEST_STRING`](crate::bounds).
    StringTooLong {
        /// How many code units were asked for.
        units: usize,
    },
}

impl From<Full> for Refused {
    fn from(full: Full) -> Self {
        Self::Full(full)
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refused::Full(full) => full.fmt(out),
            Refused::StringTooLong { units } => write!(
                out,
                "a string of {units} code units is longer than this engine will make"
            ),
        }
    }
}

/// A heap of a script's objects, and the names its properties are known by.
#[derive(Debug)]
pub struct Objects {
    heap: Heap<Cell>,
    names: Interner,
}

impl Default for Objects {
    fn default() -> Self {
        Self::new()
    }
}

impl Objects {
    /// An empty heap with nothing interned.
    pub fn new() -> Self {
        Self {
            heap: Heap::new(),
            names: Interner::new(),
        }
    }

    /// The heap, for the things that are the collector's rather than the object
    /// model's: rooting, scopes, collecting, and the invariant check.
    pub const fn heap(&self) -> &Heap<Cell> {
        &self.heap
    }

    /// The same, to hold a root or ask for a collection.
    pub fn heap_mut(&mut self) -> &mut Heap<Cell> {
        &mut self.heap
    }

    /// How many property names are interned.
    ///
    /// Including any whose string has been collected since the last prune,
    /// which is what makes this the number a test of the bound asserts on.
    pub const fn interned(&self) -> usize {
        self.names.len()
    }

    /// Make an ordinary object with this prototype.
    ///
    /// **This is a safepoint** — everything the caller means to keep must be
    /// rooted, which is ADR 0014 § 2's discipline and is why
    /// [`Heap::stress`](crate::heap::Heap::stress) exists.
    ///
    /// # Errors
    ///
    /// [`Refused::Full`] when the heap is at its ceiling.
    pub fn object(&mut self, prototype: Option<Ref>) -> Result<Ref, Refused> {
        let object = Ordinary::with_prototype(prototype);
        Ok(self.heap.allocate(Cell::Object(object))?)
    }

    /// Make an array of this length with no elements and this prototype:
    /// `ArrayCreate` (queue item 225).
    ///
    /// **This is a safepoint, twice** — interning `"length"` and making the
    /// array. Between the two the name is held by nothing but this function,
    /// and that is safe for a reason worth knowing rather than a coincidence:
    /// a collection that an allocation runs **traces the cell being
    /// allocated** ([`Heap::allocate`]), and an array traces its length's name.
    /// `what_an_array_is.rs` collects at every allocation to keep that true.
    /// The prototype is the caller's to have rooted, as [`Objects::object`]'s
    /// is.
    ///
    /// # Errors
    ///
    /// [`Refused::Full`] when the heap is at its ceiling.
    pub fn array(&mut self, prototype: Option<Ref>, length: u32) -> Result<Ref, Refused> {
        let units: Vec<u16> = "length".encode_utf16().collect();
        let key = self.key(&units)?;
        let array = Array::new(prototype, key, length);
        Ok(self.heap.allocate(Cell::Array(array))?)
    }

    /// The array a reference names, or [`None`] if it names anything else —
    /// `IsArray`, for an object that is not a proxy (item 217 is the proxy).
    pub fn as_array(&self, held: Ref) -> Option<&Array> {
        self.heap.get(held)?.array()
    }

    /// Make an iterator over `iterated` with this prototype:
    /// `CreateArrayIterator` (queue item 230).
    ///
    /// **This is a safepoint**, and both references are the caller's to have
    /// rooted, as [`Objects::object`]'s prototype is.
    ///
    /// # Errors
    ///
    /// [`Refused::Full`] when the heap is at its ceiling.
    pub fn array_iterator(
        &mut self,
        prototype: Option<Ref>,
        iterated: Ref,
        kind: array_iterator::Kind,
    ) -> Result<Ref, Refused> {
        let iterator = ArrayIterator::new(prototype, iterated, kind);
        Ok(self.heap.allocate(Cell::ArrayIterator(iterator))?)
    }

    /// The array iterator a reference names, or [`None`] if it names anything
    /// else.
    pub fn as_array_iterator(&self, held: Ref) -> Option<&ArrayIterator> {
        self.heap.get(held)?.array_iterator()
    }

    /// Change an array iterator, through the barrier every store passes.
    pub fn with_array_iterator<R>(
        &mut self,
        held: Ref,
        with: impl FnOnce(&mut ArrayIterator, &mut Barrier) -> R,
    ) -> Option<R> {
        self.heap
            .write(held, |cell, barrier| {
                cell.array_iterator_mut()
                    .map(|iterator| with(iterator, barrier))
            })
            .flatten()
    }

    /// Make a `RegExp` object of this compiled pattern, with this prototype
    /// and no properties of its own yet: `RegExpAlloc` without its
    /// `lastIndex`, which the caller defines (queue item 74).
    ///
    /// **This is a safepoint**, and the prototype is the caller's to have
    /// rooted, as [`Objects::object`]'s is.
    ///
    /// # Errors
    ///
    /// [`Refused::Full`] when the heap is at its ceiling.
    pub fn regexp(
        &mut self,
        prototype: Option<Ref>,
        program: std::sync::Arc<crate::regexp::Program>,
    ) -> Result<Ref, Refused> {
        let regexp = RegExp::new(prototype, program);
        Ok(self.heap.allocate(Cell::RegExp(regexp))?)
    }

    /// The `RegExp` object a reference names, or [`None`] if it names
    /// anything else — `RequireInternalSlot(R, [[RegExpMatcher]])`.
    pub fn as_regexp(&self, held: Ref) -> Option<&RegExp> {
        self.heap.get(held)?.regexp()
    }

    /// Make an error with this prototype and no properties of its own: the
    /// object `OrdinaryCreateFromConstructor` makes for an `Error` constructor,
    /// with the `[[ErrorData]]` slot (queue item 227).
    ///
    /// **This is a safepoint**, and the prototype is the caller's to have
    /// rooted, as [`Objects::object`]'s is.
    ///
    /// # Errors
    ///
    /// [`Refused::Full`] when the heap is at its ceiling.
    pub fn error(&mut self, prototype: Option<Ref>) -> Result<Ref, Refused> {
        let object = Ordinary::with_prototype(prototype);
        Ok(self.heap.allocate(Cell::Error(object))?)
    }

    /// Whether a reference names an object with the `[[ErrorData]]` slot.
    pub fn is_error(&self, held: Ref) -> bool {
        self.heap.get(held).is_some_and(Cell::is_error)
    }

    /// Make a string of these code units.
    ///
    /// # Errors
    ///
    /// [`Refused::StringTooLong`] before anything is allocated, and
    /// [`Refused::Full`] when the heap is at its ceiling.
    pub fn text(&mut self, units: Vec<u16>) -> Result<Ref, Refused> {
        let asked = units.len();
        let Some(text) = Text::of(units) else {
            return Err(Refused::StringTooLong { units: asked });
        };
        Ok(self.heap.allocate(Cell::Text(text))?)
    }

    /// Make a symbol, with a description that is a string cell or nothing.
    ///
    /// Every call makes a **different** symbol, which is the whole of what a
    /// symbol is: two with the same description are two property keys.
    ///
    /// # Errors
    ///
    /// [`Refused::Full`] when the heap is at its ceiling.
    pub fn symbol(&mut self, description: Option<Ref>) -> Result<Ref, Refused> {
        Ok(self.heap.allocate(Cell::Symbol(Symbol::new(description)))?)
    }

    /// Make an empty list of values (queue item 72).
    ///
    /// ADR 0014 § 2: the interpreter's stack and a realm's lexical bindings are
    /// lists of values, and a precise collector can only keep what it can walk
    /// to — so they are cells rather than Rust locals. Nothing a script can name
    /// is one.
    ///
    /// # Errors
    ///
    /// [`Refused::Full`] when the heap is at its ceiling.
    pub fn slots(&mut self) -> Result<Ref, Refused> {
        Ok(self.heap.allocate(Cell::Slots(Slots::new()))?)
    }

    /// What the slot at `at` of a list holds, or [`None`] if the reference
    /// names no list or the list is not that long.
    pub fn slot(&self, list: Ref, at: usize) -> Option<Held> {
        self.heap.get(list)?.slots()?.get(at)
    }

    /// How many slots a list has, or [`None`] if the reference names no list.
    pub fn slot_count(&self, list: Ref) -> Option<usize> {
        Some(self.heap.get(list)?.slots()?.len())
    }

    /// Change a list of values, through the barrier every store passes.
    ///
    /// [`None`] if the reference names no list, which is the engine's own
    /// mistake rather than a script's (ADR 0014 § 3) — an interpreter that has
    /// lost its stack has a rooting bug.
    pub fn with_slots<R>(
        &mut self,
        list: Ref,
        with: impl FnOnce(&mut Slots, &mut Barrier) -> R,
    ) -> Option<R> {
        self.heap
            .write(list, |cell, barrier| {
                cell.slots_mut().map(|slots| with(slots, barrier))
            })
            .flatten()
    }

    /// Make a function of this chunk, closing over this environment (queue item
    /// 209).
    ///
    /// **This is a safepoint.** `environment` is a reference the caller must be
    /// able to name *after* it — which it can, because the only caller reads it
    /// off a cell that is on the interpreter's stack, and this collector does
    /// not move what it keeps.
    ///
    /// # Errors
    ///
    /// [`Refused::Full`] when the heap is at its ceiling.
    pub fn function(
        &mut self,
        unit: Rc<Unit>,
        chunk: u32,
        environment: Option<Ref>,
        captured: Option<Value>,
        prototype: Option<Ref>,
    ) -> Result<Ref, Refused> {
        let function = Function::of(unit, chunk, environment, captured, prototype);
        Ok(self.heap.allocate(Cell::Function(function))?)
    }

    /// Make a builtin: a function whose body is Rust (queue item 218).
    ///
    /// **This is a safepoint**, and `prototype` is the realm's
    /// `Function.prototype`, which is rooted by the realm for as long as the
    /// engine lives.
    ///
    /// # Errors
    ///
    /// [`Refused::Full`] when the heap is at its ceiling.
    pub fn native(&mut self, native: Native, prototype: Option<Ref>) -> Result<Ref, Refused> {
        let function = Function::native(native, prototype);
        Ok(self.heap.allocate(Cell::Function(function))?)
    }

    /// The function a reference names, or [`None`] if it names anything else —
    /// which is what `a()` asks before it decides to throw a `TypeError`.
    ///
    /// A builtin answers here like anything else: `IsCallable` is a question
    /// about the cell, not about where the body was written.
    pub fn callable(&self, held: Ref) -> Option<&Function> {
        self.heap.get(held)?.function()
    }

    /// Make an environment of `bindings` places under `parent` (queue item
    /// 209).
    ///
    /// # Errors
    ///
    /// [`Refused::Full`] when the heap is at its ceiling.
    pub fn environment(&mut self, parent: Option<Ref>, bindings: usize) -> Result<Ref, Refused> {
        let environment = Environment::under(parent, bindings);
        Ok(self.heap.allocate(Cell::Environment(environment))?)
    }

    /// Make another environment under the same parent, holding what this one
    /// holds (queue item 216).
    ///
    /// **This is a safepoint.** The values are read out of the cell before the
    /// allocation, which is safe for the reason [`Objects::function`] gives: the
    /// environment being copied is rooted by the frame that is running, so a
    /// collection during the allocation reaches everything it holds, and this
    /// collector does not move what it keeps.
    ///
    /// Two layers of answer and they mean different things: [`Ok(None)`] is a
    /// reference that names no environment, which is this engine's own mistake,
    /// and the error is a heap at its ceiling.
    ///
    /// # Errors
    ///
    /// [`Refused::Full`] when the heap is at its ceiling.
    pub fn copy_environment(&mut self, environment: Ref) -> Result<Option<Ref>, Refused> {
        let Some(copy) = self
            .heap
            .get(environment)
            .and_then(Cell::environment)
            .map(Environment::copied)
        else {
            return Ok(None);
        };
        Ok(Some(self.heap.allocate(Cell::Environment(copy))?))
    }

    /// What the binding at `at` of an environment holds, or [`None`] if the
    /// reference names no environment or it has no such binding.
    pub fn binding(&self, environment: Ref, at: usize) -> Option<Held> {
        self.heap.get(environment)?.environment()?.get(at)
    }

    /// The environment an environment was made inside.
    ///
    /// Two layers of [`Option`] and they mean different things: the outer one is
    /// a reference that names no environment, which is this engine's own
    /// mistake, and the inner one is the end of the chain, which is where every
    /// function written at a script's top level starts.
    pub fn enclosing(&self, environment: Ref) -> Option<Option<Ref>> {
        Some(self.heap.get(environment)?.environment()?.parent())
    }

    /// Change an environment, through the barrier every store passes.
    pub fn with_environment<R>(
        &mut self,
        environment: Ref,
        with: impl FnOnce(&mut Environment, &mut Barrier) -> R,
    ) -> Option<R> {
        self.heap
            .write(environment, |cell, barrier| {
                cell.environment_mut()
                    .map(|environment| with(environment, barrier))
            })
            .flatten()
    }

    /// Put an embedder's object in the heap — a node, a console, a harness.
    ///
    /// ADR 0014 § 6: it is in the **same graph**, so the cycle a page makes
    /// between a node, a listener and a closure is one this collector reclaims.
    ///
    /// # Errors
    ///
    /// [`Refused::Full`] when the heap is at its ceiling.
    pub fn foreign(&mut self, exotic: Box<dyn Exotic>) -> Result<Ref, Refused> {
        Ok(self.heap.allocate(Cell::Foreign(exotic))?)
    }

    /// [`foreign`](Self::foreign), handing the object back when the heap
    /// refuses it — for an embedder's object that owns what cannot be made
    /// again, such as a page's document (ADR 0017 § 2).
    ///
    /// # Errors
    ///
    /// [`Refused::Full`] and the object, unchanged, when the heap is at its
    /// ceiling. The object is [`None`] only if the heap handed back a cell
    /// other than the one it was given, which it does not: answered rather
    /// than assumed.
    pub fn foreign_or_back<T: Exotic>(
        &mut self,
        exotic: T,
    ) -> Result<Ref, (Refused, Option<Box<T>>)> {
        self.heap
            .allocate_or_back(Cell::Foreign(Box::new(exotic)))
            .map_err(|(full, cell)| {
                let back = match cell {
                    Cell::Foreign(exotic) => exotic.into_any().downcast::<T>().ok(),
                    _ => None,
                };
                (Refused::Full(full), back)
            })
    }

    /// The embedder's object `held` names, borrowed as the embedder's own
    /// type — or [`None`] if it names anything else (ADR 0017 § 4).
    ///
    /// This is the Web IDL brand check's first half: a DOM method asks for its
    /// `this` as a wrapper, and a plain object, a function, a string, another
    /// embedder's object or a wrapper of a different type answers [`None`]
    /// alike. The engine learns nothing of `T` to answer.
    pub fn embedded<T: Exotic>(&self, held: Ref) -> Option<&T> {
        match self.heap.get(held)? {
            // Through the box, not of it: the box is an `Any` too, and asking
            // it would answer for `Box<dyn Exotic>` rather than for `T`.
            Cell::Foreign(exotic) => exotic.as_ref().as_any().downcast_ref::<T>(),
            _ => None,
        }
    }

    /// The same, to be written through, with the barrier every store of a
    /// reference passes (ADR 0014 § 5).
    ///
    /// [`None`], changing nothing, if `held` is not a `T`. Not a safepoint,
    /// and `change` has no way to allocate: an exotic object may not.
    pub fn write_embedded<T: Exotic, R>(
        &mut self,
        held: Ref,
        change: impl FnOnce(&mut T, &mut Barrier) -> R,
    ) -> Option<R> {
        self.heap
            .write(held, |cell, barrier| match cell {
                Cell::Foreign(exotic) => exotic
                    .as_mut()
                    .as_any_mut()
                    .downcast_mut::<T>()
                    .map(|mine| change(mine, barrier)),
                _ => None,
            })
            .flatten()
    }

    /// The key this text names, interning it if it is new.
    ///
    /// A text that is the canonical decimal of an array index is that index and
    /// **allocates nothing** — `a[i]` in a loop interns no strings at all. Every
    /// other text is one string cell, shared by every object that has a
    /// property of that name.
    ///
    /// **This is a safepoint when the name is new**, so a caller holding
    /// references across it must have rooted them. What it hands back is *not*
    /// rooted: the key's string is kept alive by the object it becomes a
    /// property of, and until then by the caller.
    ///
    /// # Errors
    ///
    /// Whatever [`Objects::text`] refuses.
    pub fn key(&mut self, units: &[u16]) -> Result<Key, Refused> {
        if let Some(at) = key::array_index(units) {
            if let Some(key) = Key::index(at) {
                return Ok(key);
            }
        }
        self.names.prune(&self.heap);
        if let Some(held) = self.names.find(&self.heap, units) {
            return Ok(Key::text(held));
        }
        let held = self.text(units.to_vec())?;
        self.names.remember(units, held);
        Ok(Key::text(held))
    }

    /// The key this text names **only if something already has it**.
    ///
    /// [`Objects::key`] interns, which allocates, which is a safepoint. This one
    /// does not, and it exists because two very ordinary things must not
    /// allocate: `typeof somethingNobodyDeclared` (queue item 72) would
    /// otherwise put a name in the heap on its way to answering
    /// `"undefined"`, and a page can write that in a loop.
    ///
    /// [`None`] means no object anywhere has a property of that name, which is
    /// the answer the caller wanted anyway.
    pub fn existing_key(&self, units: &[u16]) -> Option<Key> {
        if let Some(at) = key::array_index(units) {
            if let Some(key) = Key::index(at) {
                return Some(key);
            }
        }
        self.names.find(&self.heap, units).map(Key::text)
    }

    /// The key a symbol is.
    ///
    /// # Errors
    ///
    /// [`Fault::NotASymbol`] if the reference names anything else, and
    /// [`Fault::Gone`] if it names nothing.
    pub fn symbol_key(&self, held: Ref) -> Result<Key, Fault> {
        match self.heap.get(held) {
            Some(cell) if cell.symbol().is_some() => Ok(Key::symbol(held)),
            Some(_) => Err(Fault::NotASymbol),
            None => Err(Fault::Gone),
        }
    }

    /// The code units of a string cell, or [`None`] if it names something else.
    pub fn units(&self, held: Ref) -> Option<&[u16]> {
        self.heap.get(held).and_then(Cell::text).map(Text::units)
    }

    /// Drop the interned names whose string has been collected.
    ///
    /// [`Objects::key`] does this on its own, and this is for the caller who
    /// wants the table settled without interning anything — a test asserting
    /// the bound, and an embedder that has just been told the tab is in the
    /// background.
    pub fn prune(&mut self) -> bool {
        self.names.prune(&self.heap)
    }
}
