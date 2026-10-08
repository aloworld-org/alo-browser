# ADR 0031 — A builtin keeps its state in slots on the stack, and says how many

**Status:** accepted
**Date:** 2026-10-08
**Context:** queue item 331, *`Array.prototype.forEach`, and a `NodeList`'s
iteration*, marked **needs ADR** because *how a builtin keeps state across
the calls it asks for decides `map`, `filter`, `reduce`, `every`, `some`,
`find` and every promise reaction after it*; `alo-downloads`, alo's public
download page, whose script stops at
`document.querySelectorAll(".btn[href]").forEach(…)` (iteration 205); queue
item 221 (*`Function.prototype.apply` and traced native scratch state*),
which named the same gap from the other side and carries two refusals
already written against it — `setAttribute` with an object for both
arguments (`Missing::ASecondArgumentBehindACall`) and `exec`/`test` after a
converted argument (`Missing::ATwoCallRegExpMethod`); queue item 228
(`Error.prototype.toString` of a `message` behind a call,
`Missing::AMessageBehindACall`), which depends on 221; `alo-js`'
`object/native.rs` (*a step is a number, and the answer arrives on the
stack*), `interpret/call.rs` (`wait`, `step_builtin`, `want_for`) and
`interpret/frame.rs` (`Waiting`, `answer_at`); `heap/root.rs`, the closed
list of places a reference may be; ADR 0013 §§ 3–4 (absent beats
approximate; every bound is ours); ADR 0014 § 2 (the collector is precise,
and its roots are a closed list); ADR 0018 §§ 2–3 (a dispatch's state lives
in the event, so its native driver keeps only a step); and ECMAScript's
`Array.prototype.forEach`, `map`, `reduce` and `CreateListFromArrayLike`,
which keep `len`, `k`, `A` and an accumulator in the algorithm's own
variables across every call they make

## The decision in one line

A builtin **declares, with its function, a fixed number of value slots** —
at most eight — that the interpreter reserves on the value stack directly
above its arguments when it is entered, fills with `undefined`, and takes
down with the call. The body reads and writes them through its `Call`, a
write goes **straight to the stack**, and so anything a builtin keeps across
a call it asks for is where the collector already walks, with no new kind of
root and nothing a body can forget to trace. The `u32` step still says
*where* in the body it resumes; the slots say *what* it had. State that grows
with the input lives in a heap object one slot holds.

## Why this is a decision rather than a chore

`object/native.rs` says a suspended builtin keeps *a `u32` step and nothing
else*. That was a decision, and a good one for the builtins written under
it: everything they needed was their `this`, their arguments, or one answer.
`dispatchEvent` was the first that needed more, and it was lucky — the
standard keeps a dispatch's state in the event (ADR 0018 § 2), an object the
collector already walks.

`forEach` is not lucky. The specification reads `length` **once**, into a
local `len`, then visits `k` from 0 to `len`, calling the page's callback at
each. A page can see the difference between keeping `len` and reading it
again: a callback that pushes onto the array it is walking makes the second
version loop for as long as the page likes. `k` can be anything up to
2⁵³ − 1. Neither fits in a step next to a phase, and there is no object of
the specification's to keep them in.

Every builtin after it has the same shape and more: `map` keeps the array it
is building, `reduce` its accumulator, `apply` a list growing one getter at a
time, `sort` every value before it compares any of them. Each would invent
its own way to keep them, and the first one written would be copied. So it is
decided once, before any of them, which is the rule `LOOP.md` stage 2 § 4
gives for exactly this.

## 1. Slots, on the stack, in the call's own region

A builtin's region of the stack is `callee | this | args | answer` today.
It becomes:

```text
  callee | this | arg0 … argN | kept0 … keptK | answer
  ^                             ^               ^
  callee_at                     kept_at         answer_at
```

`kept_at` is `callee_at + 2 + argc`, and `answer_at` moves up by the number
of slots. The arguments are the ones the caller actually passed — a builtin
called with fewer than it reads still answers `undefined` past the end, as
`Call::argument` does now — so the slots sit above whatever was pushed and
never alias an argument.

**The stack is a cell the collector walks** (ADR 0014 § 2, `heap/root.rs`),
so a value in a slot is rooted for as long as the call stands, across every
step and every call the builtin asks for. Nothing about the collector
changes: there is no new root, no trace function for a builtin to write, and
no list the marker has to learn. That is the decision's main reason. In a
precise collector the bug to fear is a reference held somewhere the marker
does not look; a design in which there is nowhere else to hold one cannot
have that bug.

**A call the builtin asks for is laid out from `answer_at`** exactly as
today (`want_for`), so it can never disturb a slot. A throw that ends the
builtin truncates the stack below `callee_at` and the slots go with it, as
its arguments already do. A run that ends clears the waiting builtins
(`let_go`), and the slots were never anywhere else.

## 2. How many is said with the function, and is bounded

A `Native` declares its slot count when it is made, beside its name and
body. The count is a property of the **builtin**, never of its input: no page
can make a builtin reserve more of the stack than its author wrote down.
Reserving the slots on entry counts against
`bounds::VALUES_ON_THE_STACK` like any other push, and running out is the
same `RangeError` a deep recursion is.

The count is at most **eight** (`bounds::KEPT_BY_A_BUILTIN`). The largest
specified algorithm the queue names needs three (`map`: `len`, `k`, `A`;
`reduce`: `len`, `k`, the accumulator; `apply`: `len`, the index, the list),
and eight leaves room without inviting a builtin to use the stack as a
heap. A native declaring more is this engine's mistake, refused as
`Internal::BuiltinIsWrong` when the realm is furnished, and a test walks
every builtin the engine and `alo-bindings` install.

A builtin that declares none — every builtin written so far — has exactly
today's region and pays nothing.

## 3. A slot holds a `Value`, and a number is a number

A slot is a `Value`, the same type an argument is. A length or an index is a
`Value::Number`: the specification bounds both with `ToLength` at
2⁵³ − 1, which an `f64` holds exactly, so nothing is lost and no second kind
of slot is needed. The body converts at the edge, with a checked conversion
that is `Internal::BuiltinIsWrong` if a slot it wrote as an index reads back
as anything else.

**State that grows with the input lives in the heap, and one slot holds
it.** `apply`'s argument list, `sort`'s values and `Array.from`'s result are
as long as the page makes them. They are a heap object the builtin allocates
— an array with no prototype, never handed to the page — and keep in a slot.
The heap's ceiling bounds them, as it bounds every other allocation a script
can cause (ADR 0013 § 4), and the slot count stays a constant.

## 4. A write is to the stack at once, and a read is from it

`Call` gains `kept(n)`, which reads slot `n` from the stack, and `keep(n,
value)`, which writes it there through the stack's barrier
(ADR 0014 § 5). Neither copies the slots into the `Call` and back afterwards.
That is deliberate: `map` allocates its array, keeps it, then allocates again
to read an element; if the kept value sat in a Rust copy until the body
returned, the second allocation could collect the array the first one made.
Written at once, it is rooted from the moment it is kept — the same rule
`object/native.rs` already gives for `Want::Call`'s argument list, applied
to the one place it would otherwise bite.

A slot number at or past the declared count is `Internal::BuiltinIsWrong`
rather than a panic or a write into the answer slot.

## 5. The step stays a number, and says where rather than what

The `u32` step keeps its meaning: which point in the body runs next, never
zero once a builtin has asked for something. A builtin that loops — `forEach`
calling the callback for each `k` — resumes at the same step each time and
reads `k` from its slot. Encoding a small fixed value in the step, as
`addEventListener` does with the option bits it has read so far, remains
allowed where the value is genuinely bounded by the builtin, but nothing that
can be as large as a page chooses may be put there.

## 6. Where the specification keeps state in an object, the object keeps it

Slots are for what the specification keeps in **an algorithm's own
variables**. Where it keeps state in an object, the object keeps it: an array
iterator's position is in the iterator (item 230), a dispatch's path in the
event (ADR 0018 § 2), and a promise's reactions and a capability's
`resolve` and `reject` will be in the promise and its records (item 75). A
builtin that drives one of those keeps at most a reference to the object in
a slot, and only when its `this` and arguments do not already hold it.

That is also the answer for *every promise reaction after it*: the reaction
job's own steps — call the handler, then resolve the derived promise with
what it returned — are a builtin that keeps the capability in a slot across
the handler call, and the reaction record itself is the promise's.

## 7. A loop the page sized asks the stop

`forEach` over `{ length: 2 ** 53 - 1 }` calls nothing: every index is a
hole, and `HasProperty` on an ordinary object runs no script. The interpreter
sees no backward jump and no call, so the embedder's stop would never be
asked. So a builtin whose loop is as long as a page chooses asks
`Call::stop_asked` on every pass that does not suspend, and answers
`Escape::Interrupted` when it is true — exactly as the regular expression
matcher does (ADR 0029 § 3). The builtins that suspend on every pass are
asked by the interpreter's own call check and need nothing more.

## What this costs

- **Every builtin's entry writes its slots.** For the ones that declare
  none, nothing. For `forEach`, two `undefined`s on a stack that is already
  being written.
- **`answer_at` depends on the native as well as on its arguments.**
  `Waiting` carries the count, and everything that computes the answer slot
  goes through the one function that already computes it.
- **A slot is untyped.** A body that writes an index and reads it back as an
  object is caught at run time as this engine's bug, not at compile time.
  A typed slot would be a second kind of stack value the collector has to
  understand; a checked read is cheaper than that.

## Alternatives rejected

**More bits in the step.** A `u64` step, or a step with the index in its high
bits. `len` would still have nowhere to live, and `map`'s array and
`reduce`'s accumulator are references, which no integer can hold for the
collector. It solves `forEach` and nothing after it.

**Reading `length` again on every resumption.** Simple, and observably
wrong: a callback that pushes onto the array walks for ever, and a `length`
getter runs once per element where the specification runs it once. Absent
beats approximate (ADR 0013 § 3).

**A typed state object per builtin, held on the `Waiting` and traced.** A
`Box<dyn Suspended>` with a trace method each builtin writes. It reads well,
and it is the design a precise collector is most afraid of: a new kind of
root outside ADR 0014 § 2's closed list, and a trace method per builtin, any
of which can forget an edge and be right in every run but a stressed one.
It also allocates a box per call to keep two numbers.

**A hidden heap object per call**, the way an event keeps its dispatch. It
is traced already, so it is safe, but it allocates on every `forEach` and
needs a cell kind no page may reach. Kept for the cases the specification
already gives an object (§ 6), not made the general rule.

**Self-hosting: write `forEach` and its family in JavaScript.** Several
engines do. It makes a builtin's state the script's own locals, which is
elegant, but a self-hosted builtin is observable unless the engine adds a
privileged dialect, guards every intrinsic it reaches against a page that
replaced it, and hides its frames from stack traces and its source from
`Function.prototype.toString`. That is a second language inside the engine,
and it is not a cost this engine pays to keep two numbers.

**Let a builtin call the interpreter directly.** Rejected by
`object/native.rs` already and for the same reason: a Rust call inside a Rust
call grows this process's stack, and a builtin that asks for itself for ever
would overflow it rather than being the `RangeError` the language specifies.

## What this does not decide

- **State a builtin *function object* carries between calls** — a bound
  function's target (item 220), a promise's resolving functions' promise
  (item 75). That lives in the function's own cell, which is a question about
  the object model rather than about a suspended call, and it is decided
  where those are built.
- **Generators and `await`** (item 75). A suspended *script* frame keeps its
  locals in its environment already; this ADR is about builtins only.
- **Which builtins are built.** Each is still opened by a page or by an item
  that depends on it. This ADR makes `forEach` (331), `apply` (221) and
  `Error.prototype.toString`'s message (228) buildable, and lets the
  `setAttribute` and `exec`/`test` refusals that item 221 carries be closed
  there.

## How we will know if this was wrong

**If a builtin needs more than eight slots**, it is a sign the builtin is
keeping a list in the stack that belongs in the heap (§ 3). The bound changes
only in an amendment that names the builtin and says why its state cannot be
one heap object.

**If a test under `Heap::stress` loses a kept value**, § 4 has been broken
somewhere — a body kept a reference in a Rust local across an allocation
instead of in a slot — and the fix is in that body, not a relaxation here.

**If the stack's bound is reached by slots rather than by recursion** in a
real page, the slot reservation is too generous for how deeply builtins
nest, and § 2's ceiling is lowered rather than the stack's raised.
