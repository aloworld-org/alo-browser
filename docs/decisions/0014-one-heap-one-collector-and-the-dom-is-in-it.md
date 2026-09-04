# ADR 0014 — One heap, one collector, and the DOM is in it

**Status:** accepted
**Date:** 2026-09-04
**Context:** ADR 0013 (our own JavaScript engine), whose § 6 states this problem
and deliberately leaves it open — *the single thing the engine demands of an
embedder object is that it can be traced, and that demand is the whole reason
the collector is the next decision rather than this one*; ADR 0013 § 1, which
refused a rented engine partly because **a rented collector decides how
`alo-dom` is stored**, so this ADR is where that refusal is paid for; ADR 0003
(node identity is allocated once and never reused), which a heap that reuses
slots has to answer to; ADR 0004 (we own the layout tree, `taffy` owns the
algorithms), whose handle is an index rather than a pointer *so that there is no
`unsafe` near it* — the same move, made again; ADR 0005 (one process per site),
which says a renderer may die and the browser process may not; `CLAUDE.md`'s law
3 (correct before fast) and law 4 (no `unsafe` outside a reviewed boundary);
`docs/autonomy/QUEUE.md` item 71, which asks for this decision by name and says
in six words what it is about — *a collector is a decision about pauses*

## The decision in one line

The heap is an **arena of slots in safe Rust** and a reference into it is an
**index carrying a generation**; one **precise, non-moving, stop-the-world
mark-and-sweep** collector owns it; the **DOM is in the same graph**, traced
rather than counted; and the three things that cannot be added afterwards go in
from the first line — a **write barrier** every mutation passes through, an
**ephemeron fixpoint** in the mark phase, and a **marker that never recurses**.

## Why this is a decision rather than a chore

Item 71 is one line of queue and it is under everything: item 72's interpreter,
every builtin in 73, the promises in 75, the mutation in 80 and the storage in
90 are all things that allocate. Four of its clauses cannot be changed later
without touching every one of them.

**Where a reference may live** is the clause with the longest reach. A precise
collector can only run where it can find every live reference, so the answer
decides the shape of the interpreter's stack, the signature of every builtin,
and what a native function is allowed to hold across a call. An engine that
decides this after it has two hundred builtins rewrites two hundred builtins.

**Whether the DOM is in the graph** decides what `alo-dom` is. A page's objects
and a document's nodes reference each other in cycles — a node holds a listener,
the listener closes over the node — and every browser that answered this with
two mechanisms spent years finding leaks that only appear after a day of use.

**Whether there is a write barrier** looks like an optimisation and is not. Both
answers to a pause somebody can see — incremental marking, and a generational
nursery — need every store of a heap reference to be visible to the collector.
Installing that hook when it does nothing costs a function call. Installing it
afterwards means auditing every mutation in the engine and being sure.

**And a collector that recurses is a collector a script chooses the depth of.**
Item 204 learned this on the parser, where a bound of 256 refused nothing
because the process aborted before the counter reached it. The object graph is
the same shape of problem with none of the same protections, and deciding it
here is cheaper than discovering it at the bottom of a mark phase.

## 1. Tracing, because the graph has cycles by construction

Reference counting is refused. Not because it is slow — for much of a heap it is
faster — but because the cycles are the **normal case** rather than the corner:
`node.addEventListener("click", () => node.focus())` is a cycle in the first
line of most pages, and so is every closure a framework stores on the object it
was made for. A counted heap needs a second mechanism to find those, which means
two collectors with two ideas of what is alive, which is where a leak becomes
invisible for months rather than obvious in a test.

So: **one tracing collector, and it is the only thing that decides what is
alive.** Nothing in this engine is freed because a count reached zero.

## 2. Precise, which means every root is somewhere we walk

The alternative is conservative scanning: read the machine stack as a row of
words, and treat anything that looks like a heap address as one. It is what lets
an engine be written without discipline about roots, and it is refused twice
over — it needs `unsafe` to read the stack at all (law 4), and it retains by
accident, so a stale word in a register keeps a whole page's heap alive and
nobody can reproduce it.

**Precise means the set of places a live reference may be is a closed list**,
and here it is:

- the globals of each realm,
- the interpreter's frames and its value stack, which for that reason live in
  structures the collector walks rather than in Rust locals,
- the **scopes** native code holds — a builtin written in Rust does have locals,
  and a scope is where it puts the references it needs to keep across an
  allocation,
- the embedder's roots, which is § 6,
- and the keep-alive set a job accumulates, which is § 7.

Nothing else. A reference held anywhere but those, across a point where the
engine may allocate, is a bug — and § 3 decides what kind of bug it is, because
"be careful" is not a design.

## 3. A reference is an index, and the pair it makes is never reused

A heap object is a **slot in an arena**, and a reference to it is an index —
exactly ADR 0004's move for `taffy`'s handle, for exactly ADR 0004's reason:
an index is safe code where a pointer is `unsafe`. Sweeping a slot returns it to
a free list, and a later object takes it.

That collides with ADR 0003, which says a node's identity is allocated once and
never reused because a reused number makes two different things look like one. A
heap cannot afford never to reuse a slot. So what is never reused is the
**pair**: each slot carries a **generation** that increases every time the slot
is filled, a reference carries the generation it was made with, and a reference
whose generation no longer matches names *nothing* rather than naming whatever
took the slot. ADR 0003's promise is kept at the level it was made — no two
things ever have one identity — and the arithmetic is a comparison.

When a generation would wrap, the slot is **retired** instead: never handed out
again, one slot spent, and the one hole in the argument closed rather than
described.

**What a stale reference does is the point of the whole section.** In an engine
with pointers it is a use-after-free, which is the most valuable bug class in a
browser. Here it is a mismatch we can see, and it means the engine has a rooting
bug of its own — never a page's doing. It ends the **script** with an internal
error that is reported, never the process (ADR 0005), never a panic (ADR 0013
§ 4), and never a wrong object handed back as if it were right. Under test it
fails the test, loudly, which is § 10.

## 4. Non-moving mark and sweep, and § 3 is what keeps moving available

**Correct before fast** (law 3), so the first collector is the one whose
invariants fit on a page: mark from the roots, sweep what is unmarked, do not
move anything.

Non-moving costs fragmentation and locality, and both are permanent until
somebody has numbers. What it buys is that **a slot's contents are an ordinary
Rust value with an ordinary `Drop`** (§ 7), and that nothing in the engine ever
needs to know where an object *is*.

The expensive future decision — a compacting or generational collector, which
moves objects — is left open by § 3 rather than by hope: **rewriting an index is
safe code**, where rewriting a pointer held by a stranger is not. The one thing
that would foreclose it is handing an address to anybody, so **no embedder ever
receives an address**, only a handle.

## 5. The write barrier goes in from the first line, and today it does nothing

Every store of a heap reference *into* a heap object goes through one function
on the heap. Not by convention: an object's reference-bearing fields are private
to the heap module, so there is no second way to write one, which is this
repository's usual preference for a promise kept by shape over a promise kept by
memory.

Today it stores and returns. It exists because both answers to a visible pause
need it — incremental marking needs the tri-colour invariant maintained on
every store, and a generational nursery needs a remembered set of old-to-new
references — and because installing it later means auditing every mutation site
in an engine that by then has builtins, a DOM binding and a compiler emitting
stores. That is the retrofit ADR 0002 refused for the agent tree and ADR 0003
refused for identity, in the one place where it is a single function.

## 6. One graph: the DOM is traced, not counted

This is ADR 0013 § 1's claim — *the object graph is one graph* — made real, and
it is the clause that makes renting an engine unrecoverable rather than merely
expensive.

`alo-dom` keeps the tree it already owns, with the identity ADR 0003 gave it.
The join is a **wrapper**: a heap object holding a node's id, made on demand, and
**one per node for as long as the node lives**, because a wrapper that could be
made twice is an object whose identity a script can see change under it and
whose expando properties vanish.

The collector's roots include the **embedder's** roots — the document — and
tracing walks *through* embedder objects and back into the heap. So the cycle
that every browser leaked for a decade (node → listener → closure → node) is one
graph, walked by one collector, and it is reclaimed when the document no longer
reaches the node and nothing in script does. **Reachability decides, and nothing
else does.**

Two structural rules hold it up:

- **The trait lives in `alo-js`.** ADR 0013 § 6 gives that crate no dependency on
  `alo-dom`, and `alo-dom` has none on `alo-js`, so stage 1's renderer keeps
  working with no engine in the process. The crate that implements the trait for
  nodes is the **bindings** crate (item 80), which depends on both and is the
  only thing that does.
- **The engine demands one thing of an embedder object and it is `trace`.** Not a
  free, not a count, not a finaliser: tell the collector which heap references
  you hold, and it will decide the rest.

Rejected, and it is the closest call in this ADR: refcount the DOM and
cycle-collect it separately, which is what Gecko does and what the platform's
history is full of. Refused because it is two collectors with two ideas of alive
and because the failure mode is a leak nobody can attribute — the exact thing
`ROADMAP.md` says about the HTTP cache, in a different file: *subtly wrong here
is invisible for months.*

## 7. Weakness is designed in, and a finaliser frees nothing of ours

**Ephemerons, marked to a fixpoint, from the first line.** A `WeakMap`'s value is
reachable only while its key is, and the naive readings are both wrong in a way
tests written afterwards do not catch: mark values always and it leaks, clear
them in one pass and a chain of maps loses entries that are live. So the mark
phase iterates the weak sets until nothing new is marked. It is a loop in the
collector if it is there from the start and a rewrite of the collector if it is
not.

**`WeakRef` and `FinalizationRegistry` are cleared at the end of a collection,
and their callbacks run as tasks on the event loop** (item 76) — never during
a collection, because a finaliser allocates and mutates, and a heap being swept
is not a heap anybody can allocate in. The language permits a callback never to
run, and that is all we promise. The specification hands the callback a **held
value rather than the target**, which is what makes resurrection impossible
here, and it is worth writing down as a property we get rather than a rule we
enforce.

**And nothing the engine owns is released by a finaliser.** A swept slot is a
Rust value dropped, so a native resource behind an object — a decoded image, a
buffer — is released at the sweep, deterministically, by `Drop`. That is a thing
safe Rust and a non-moving heap give us that C++ engines pay for with finaliser
queues, and it means a page cannot hold an operating-system resource open by
never running a callback.

A last rule the language requires and a collector will otherwise break: a
`WeakRef` that has been dereferenced keeps its target alive for the rest of the
**job**, so a script cannot see the same reference answer twice differently. The
keep-alive set is a root (§ 2) and is cleared when the job ends.

## 8. The marker never recurses, and a collection never fails for want of memory

Item 204's finding, restated for the graph it is worse on. A script can build a
list a million objects deep in one line; a marker written as a recursive walk
aborts the process, and an abort is not a refusal.

So marking is an explicit **worklist**, iterative, with a **bounded** capacity —
because the worklist is itself an allocation whose size a stranger's script
chooses. Overflowing it is not a failure: the mark bits are the truth and the
worklist is only a list of what to look at next, so an overflow costs a **rescan**
and never correctness.

The rule underneath both: **a collection allocates nothing it has not already
got.** The moment we most need to collect is the moment there is no memory to
collect with, and a collector that allocates then is a collector that fails
exactly when it matters.

A collection also **runs to completion** once begun — an interrupt from the
embedder (ADR 0013 § 4) is checked at safepoints, and a half-marked heap is not
a state anything can resume from. That is bounded work rather than an open
promise, because the live heap is bounded by § 9.

## 9. When it runs, and what a full heap does

**Triggered by bytes allocated since the last collection, never by a clock.**
ADR 0013 § 5 gives this crate no clock and this ADR does not hand it one. The
**embedder may also ask** for a collection — the browser process is the thing
that knows a tab is in the background or that a person has stopped typing, and
that is a judgement about a person rather than a number an engine can have.

**A collection happens only at a safepoint**, which is a point the interpreter
chose and where § 2's closed list is the whole of the live references.

**The heap has a ceiling of ours**, in `alo-js`'s `bounds.rs` with the reason
beside it, exactly as `LONGEST_SOURCE` and `DEEPEST_NESTING` have: ADR 0013 § 4,
and `alo-net`'s sentence — *a limit somebody else chooses is not a limit*.
Reaching it collects first and fails second, and failing means:

- where the language specifies an error, we produce that error, because a
  script's own `catch` is the page's way of surviving us (ADR 0013 § 3) — a
  `RangeError` for a string or an array that cannot be made;
- where it does not — a heap that is simply full — the engine tells the
  **embedder**, which stops the tab and says so. A person gets a page that
  refused, with a reason.

Never an abort, never an allocation failure taken as a panic, and never the
browser process (ADR 0005). The number itself lands with the code, because a
ceiling written into an ADR is a number nobody can tune with evidence.

## 10. How correctness is measured, since a collector's bugs hide

A collector is the one component whose defects are invisible in ordinary tests:
a heap that never collects passes everything. So the evidence is built for it.

- **A heap invariant check, run after every collection under test**: nothing
  reachable is on the free list, nothing on the free list is referenced, every
  reachable reference's generation matches, and no mark bit survives a cycle.
- **Collection is explicit in tests** rather than triggered by pressure, so a
  test asserts *what was reclaimed* rather than *whether a collection happened*.
  This is ADR 0013 § 5's property — testable with nothing moving — applied to
  the part of the engine that is otherwise timing-dependent.
- **A stress mode that collects at every safepoint.** This is how a rooting bug
  is found, and § 2's discipline is not credible without it: a builtin holding a
  reference across an allocation is correct in every ordinary run and wrong in
  this one.
- **A cycle is reclaimed, counted rather than watched.** Including the
  DOM-to-script cycle of § 6, asserted by counting live objects — a test that
  watches process memory is a test that measures the allocator.
- **The hostile half** (`LOOP.md` stage 2, clause 2): a graph a million deep, a
  million objects of nothing, an unbounded number of distinct property keys, a
  weak map whose values are the keys of another. Each is a refusal or a
  collection, never a crash.
- **test262 per feature, frozen, read as a table** (ADR 0013 § 9) for
  `WeakMap`, `WeakSet`, `WeakRef` and `FinalizationRegistry`, which are where a
  collector's semantics become a script's observations.

## 11. The object model underneath it

Item 71's other half, and the parts of it that are decisions rather than code:

- **An ordinary object is a prototype reference (or null), a property table and
  an extensibility flag.** A property is data or accessor with the three
  attributes the language specifies; nothing here has a fourth.
- **Property order is observable, so it is the specification's order from the
  first line**: integer-like keys ascending, then string keys in insertion
  order, then symbols in insertion order. It costs nothing now, and it is
  invisible until the day a page enumerates and gets a different answer from
  every other browser.
- **Internal methods are a trait**, and it is the *same* trait ADR 0013 § 6
  promised the embedder. The specification already writes arrays, functions,
  proxies and the DOM's own oddities as exotic objects with their own internal
  methods, so one mechanism serves both and there is no second one to keep in
  step.
- **One interface for property access** — get, set, define, delete, own keys —
  because the representation behind it is the thing an engine changes when it
  gets fast. Hidden classes and inline caches are refused **now** under law 3 and
  are allowed **later without an ADR**, provided the semantics and the order are
  unchanged: they are an optimisation behind an interface, which is exactly what
  a JIT is not, and that difference is why one needs a decision and the other
  does not.
- **Property keys are interned, and the intern table is weak.** Interning makes a
  key comparison an integer comparison; a strong intern table makes a leak that a
  stranger's script controls, since a page can mint unbounded distinct keys. So
  the table is swept with everything else.
- **A string is a heap object, immutable once made**, in the UTF-16 code units
  item 70 already decided, and its length is bounded by § 9. Ropes and slices are
  a later representation change behind the same interface, on the same terms as
  hidden classes.

## What this costs

- **Every builtin is written to a discipline.** A native function holding a
  reference in a Rust local across an allocation is a bug that is invisible
  except under § 10's stress mode. That is the price of precision, and the
  alternative was `unsafe` plus retention nobody can reproduce.
- **Fragmentation and locality**, permanently, until somebody has a measurement
  and § 4's door is opened.
- **A pause proportional to the live heap**, which is what stop-the-world means
  and is the thing item 71 named. § 5 is why the answer is available.
- **A bounds check on every dereference**, which is what an index costs and what
  law 4 buys with it.
- **The DOM's storage is now partly this ADR's business.** § 6 is a constraint on
  item 80's bindings, and it was going to be somebody's — ADR 0013 § 1 refused a
  rented engine precisely so it would be ours.

## Alternatives rejected

**`Rc`/`RefCell` throughout, with a cycle collector beside it.** Rejected in § 1:
two mechanisms, two ideas of alive, and the leak is the one you find in
production.

**Renting a Rust collector — `gc`, `dumpster`, `shredder`.** Rejected by
ADR 0013 § 8, which lists the collector among the things never rented, and for
one reason beyond the list: those crates collect a Rust program's graph, and this
is a language's heap with ephemerons, exotic objects, an embedder in the graph
and a mutator that is hostile. Renting one is adopting its answer to § 2 and § 6,
which are the two questions this ADR exists to answer.

**Conservative stack scanning.** Rejected in § 2: `unsafe` to read the stack, and
retention by accident.

**Generational from the start.** Rejected as premature: a nursery is a copying
collector, which is moving, which is § 4's later decision. The barrier is
installed so this is a change rather than a rewrite.

**Concurrent or incremental marking now.** Rejected: ADR 0013 § 7 gives one heap
per event loop and nothing shared across threads, and incremental marking is the
answer to a measurement nobody has taken.

**Hidden classes and inline caches now.** Rejected under law 3, and explicitly
allowed later without an ADR — § 11.

**No collector at all: free the arena when the page goes away.** Rejected,
because a single-page application never goes away, and it is the shape of page
this browser exists to render well.

## What this does not decide

- **The numbers.** The heap ceiling, the trigger threshold and the worklist
  capacity land in `bounds.rs` with the code, each with its reason, and each is
  tunable with evidence in a way an ADR is not.
- **Whether the collector eventually moves.** § 4 keeps it available and § 3 is
  what makes it safe code when somebody wants it.
- **The shape of the DOM bindings** beyond the trace trait and the one-wrapper
  rule — queue item 80.
- **Where a safepoint falls relative to a task or a microtask** — queue item 76,
  the event loop, which ADR 0013 already left there.
- **The value representation's compactness.** ADR 0013 § 2 refused `unsafe`
  there — NaN-boxing and tagged pointers — on terms this ADR does not reopen.
- **A JIT**, refused by ADR 0013 § 2 with the two conditions for re-opening it.

## How we will know if this was wrong

Two measurements, which is the standard ADR 0007, 0008, 0011, 0012 and 0013 set.

**If the person using this as their browser for a week (stage 2's exit gate) sees
a page hitch, and the hitch is a collection rather than a missing feature**, then
stop-the-world was the wrong first answer. The response is incremental marking on
the barrier § 5 already installed, with the measurement in hand — taken on
hardware, because `LOOP.md` says any claim about speed is measured or not made.
That is this decision working rather than failing: it is why the barrier is
there.

**And if a page left open for a day grows without bound while its live object
count does not**, then something is rooted that should not be — almost certainly
at § 6's boundary, which is the only place a reference crosses out of the
engine. The answer is to name what is holding it, in that boundary, and never to
add a second mechanism that frees things the collector thinks are alive.
