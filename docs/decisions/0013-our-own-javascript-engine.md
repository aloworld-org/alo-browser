# ADR 0013 — Our own JavaScript engine: bytecode, correct first, no JIT yet

**Status:** accepted
**Date:** 2026-09-04
**Context:** ADR 0001 (our own engine in Rust), which refused V8 by name and made
memory safety the argument this decision either keeps or spends; ADR 0005 (one
process per site), which decides *where* a script runs and what it may reach;
ADR 0009 (the engine is MPL), which is why a licence is not what refuses a rented
engine here; ADR 0010 (the sandbox is rented), whose confinement a script
inherits; ADR 0012 § 4, which already leans on a task boundary this engine has to
define; `CLAUDE.md`'s four laws, of which the fourth — one language, and no
`unsafe` outside a reviewed boundary — is the one a JIT would spend;
`docs/autonomy/QUEUE.md` item 69, which asks for this decision by name and is the
first item of section D; `ROADMAP.md`'s *JavaScript, ours, in Rust*

## The decision in one line

We write a JavaScript engine — `alo-js` — in safe Rust: a **parser, a bytecode
compiler and an interpreter**, correct before fast, with **no JIT** until a
measurement on real hardware says the interpreter is why somebody reached for
another browser and an ADR of its own weighs that against the attack surface a
JIT adds.

## Why this is a decision rather than a chore

Everything in section D and most of section E is downstream of it, and three of
its clauses cannot be changed later without a rewrite.

**Whether it is ours at all** is the largest scope decision left in this project.
ADR 0001 refused V8 in one paragraph, when JavaScript was years away and the
refusal cost nothing. It is not years away now, the crates that would save us
exist, and a refusal that was cheap when it was hypothetical has to be paid for
in the iteration where it becomes work. So it is re-argued here, in full,
including the option ADR 0001 never considered: a JavaScript engine already
written in Rust.

**Bytecode against an AST walker** looks like an implementation detail and is
not. Generators, `async`/`await` and a debugger all need a frame that can be
suspended and resumed, and a tree-walking interpreter expresses that by rewriting
every semantic it has. Engines that started by walking the tree rewrote it;
choosing after there are builtins is choosing to do the work twice.

**And a JIT is where Rust stops helping.** A JIT emits machine code and jumps to
it: that is `unsafe` by construction, in the largest and most valuable target in
the browser, and law 4 says such a thing needs an ADR naming the boundary and the
reason. JIT bugs are the single most exploited class of browser vulnerability
there is. Deciding this *before* the interpreter exists keeps the interpreter
honest — an engine written expecting to be rescued by a compiler later is written
carelessly now.

## 1. It is ours, and this is where ADR 0001's argument is actually spent

ADR 0001's claim is *a memory-safe, independent engine*. A browser whose largest
component is somebody else's C++ has spent that claim, whatever the rest of the
repository is written in — and spending it quietly, inside a commit that was
mostly plumbing, is worse than never having made it.

So V8, SpiderMonkey and JavaScriptCore are refused for the reason ADR 0001 gave,
unchanged.

**The harder refusal is a JavaScript engine already written in Rust** — Boa most
of all. It is real, it is permissively licensed, and MPL (ADR 0009) makes taking
it legally trivial, so licence is not the objection and neither is memory safety.
The objection is what `CLAUDE.md` already draws the line on: *rent the physics,
build the engine*. A shaper, a codec, a TLS stack and a Unicode table are physics
— nobody's engine differs by them, and nobody's differs by getting them wrong.
An interpreter is not physics. It is where this browser's three claims are made
good or not:

- **The object graph is one graph.** A page's objects and the DOM's nodes
  reference each other in cycles, and whichever collector traces them decides how
  the DOM is stored, how a node is kept alive, and what a leak is. Renting an
  engine is adopting its collector's rules for `alo-dom`, which is the one
  structure ADR 0003 already made promises about.
- **Bounds are ours or they are nobody's.** Section 4 is a list of limits, and
  `alo-net` is a crate full of them for the same reason: *a limit somebody else
  chooses is not a limit*. Renting the interpreter means inheriting its idea of
  how much memory a stranger's script may cause us to allocate, and finding out
  what that idea is by being wrong about it.
- **A script is what an agent's page runs.** ADR 0002's tree and ADR 0012's
  attribution both cross into script the moment pages have any, and both are the
  part of this browser nobody else is building.

There is an honest cost, and it is not small: Boa exists today and `alo-js` is
years away. That is the same trade ADR 0001 took and is taken again with open
eyes. What makes it survivable is the same thing that made stage 1 survivable —
**scope, not depth**: section 3 is a much smaller language than a compatibility
engine has to implement.

## 2. A bytecode compiler and an interpreter, and no JIT

**Bytecode from the first line of the compiler**, for the reason above: a
suspendable frame is the shape generators, `async` and a debugger all need, and
it is not a thing to retrofit into a tree walker.

**No JIT.** Not "later" as an aspiration and not "never" as a slogan — refused
now, with the condition for re-opening it written down so that it is a decision
somebody makes rather than a thing that creeps in:

1. **A measurement on real hardware**, naming a page and a task somebody actually
   performs, showing the interpreter is the reason. `LOOP.md` is already blunt
   about this: *any claim about speed at all* is measured on hardware or not
   made. "Interpreters are slow" is not a measurement.
2. **An ADR of its own**, weighing that against what a JIT costs: `unsafe` at the
   heart of the engine (law 4), writable-then-executable memory inside the
   process that parses hostile bytes (ADR 0005), and a class of type-confusion
   bug that no amount of Rust prevents because the bug is in the code we emit.

Two cheaper things come first and are named here so the JIT is not reached for
before them: an interpreter that is actually optimised (inline caches, a sensible
value representation, no allocation per property access), and *not running the
script at all* — which is most of what a page's start-up cost is.

**No `unsafe` in the value representation either.** NaN-boxing and tagged
pointers are the obvious first `unsafe` in any engine, they are worth real
performance, and they are refused under law 4 on the same terms: measured first,
ADR second. An enum is the starting representation, and it is allowed to be
replaced by a *safe* compact one whenever somebody has the numbers.

## 3. The language is the one pages ship, and absent beats approximate

Law 1 applies to the language exactly as it applies to layout. We implement
modern ECMAScript — the language a page minified this year is written in — and
not thirty years of it.

**Refused to stage 3, and recorded rather than half-built:** `with`, sloppy-mode
`arguments` aliasing, HTML-like comments in source, legacy octal escapes and the
rest of queue item 142's list, along with `document.write` (item 137), which is a
parser decision this engine must not quietly force. Each is opened by a real page
failing, which is `ROADMAP.md`'s rule for the whole legacy tail.

**And the rule that decides what "not implemented yet" looks like: absent is
better than approximate.** A builtin we have not written is **not defined** —
not a stub returning a plausible value, not a function that throws a message of
our own devising where the language specifies a value. Pages already cope with
missing features by testing for them, and `typeof x === "function"` is how they
do it; a stub is the one answer that defeats the check *and* produces wrong
behaviour afterwards. This is law 3 and the gate's no-stubs rule, restated for
the one place where a stub would look most reasonable.

Where the language itself specifies an error, we produce that error — a
`RangeError` for a stack that is too deep, a `TypeError` for a call on nothing —
because a script's own `catch` is the page's way of surviving us.

## 4. A script is a stranger's bytes, so every bound is ours

`LOOP.md`'s stage 2 clause 2 applies to this crate more than to any other in the
repository: a script is hostile input that we then *execute*. So:

- **It never panics.** Not on any source text, not on any program. A parser that
  recursed on nesting depth crashes the renderer on twenty thousand open
  brackets; the depth is bounded and the refusal is a syntax error. A refusal is
  a result; a crash in a renderer is a denial of service.
- **Every allocation a script can cause has a ceiling we chose**: source length,
  parser and compiler nesting depth, call-stack depth, heap size, string and
  array length, and the work a regular expression may do (item 74). Named
  constants, each with the reason beside it, in the shape `alo-net` already uses.
- **The interpreter is interruptible.** A script that will not finish is stopped
  by the *embedder* — the browser process decides that a tab has stopped
  answering, which is a person's judgement about a page and not an engine's
  timer. The interpreter therefore checks an interrupt at points it defines, and
  unwinds cleanly when it sees one; there is no clock inside `alo-js`.
- **No arithmetic that can overflow on a hostile length.** The lints already deny
  `unwrap`, `expect`, `panic` and slice indexing; this is the part they do not
  catch, and it is called out because a bytecode interpreter is arithmetic on
  indices from beginning to end.

## 5. Script runs in a renderer, and `alo-js` can reach nothing

ADR 0005 gives a renderer almost no privilege, and this is the crate that makes
that matter: **the browser process never runs page script.** The process holding
the network, the cache, the cookie jar and the durable record does not execute
anything a stranger sent.

Inside the renderer, the boundary is structural rather than remembered:
**`alo-js` depends on no I/O crate** — not `alo-net`, not the filesystem, not a
clock, not an entropy source. `Date.now`, `Math.random`, `fetch` and every other
capability arrive as things the embedder passes in. Two things follow, and both
are the point:

- A capability the renderer was not given is one no script can reach, which is
  what ADR 0010's sandbox is for.
- The engine is testable with **nothing moving** — the same property that made
  items 55, 154 and 188 assertable, applied to the largest component in the
  browser.

## 6. The engine knows nothing about the DOM

`alo-js` defines **one trait** for objects an embedder supplies, and the DOM is
one embedder among several — the console and a test harness are others. The
engine holds no `alo-dom` dependency, and `alo-dom` holds no `alo-js` one, so
stage 1's renderer keeps working with no script in the process at all.

The single thing the engine demands of an embedder object is that it can be
**traced**, because it is in the graph the collector walks. That demand is the
whole reason the collector is the next decision rather than this one.

## 7. One heap per event loop, and nothing shared between threads

An engine instance belongs to one event loop and is never touched from another
thread. A worker (item 91) gets its own engine, its own heap and its own loop,
and values cross between them by **copying** — the structured clone the web
platform already specifies.

This is not a limitation being accepted; it is the shape that keeps a collector
writable in safe Rust without a concurrency ADR we would be making on no
evidence. `SharedArrayBuffer` is **refused for now and recorded**: it is shared
mutable memory between threads, it requires cross-origin isolation to be safe at
all, and it is the mechanism that turned Spectre from a paper into a web attack.
It is not in `docs/features.md`, and this decision does not add it.

## 8. What we rent inside it, and what we never will

**Rented**, each behind one file on `scripts/gate.sh`'s boundary list, exactly as
`html5ever`, `rustls` and `sha2` are:

- **Unicode tables** — identifier classes, case folding, normalisation. Physics,
  and a table nobody's engine differs by.
- **Number to string and back.** Shortest-round-trip formatting of a double and
  correct parsing of one are famously hard, entirely specified, and visible on
  every page that prints a number. This is the arithmetic equivalent of renting a
  shaper.
- **`Intl`** (item 79), which is ICU with a specification wrapped round it, and
  which `ROADMAP.md` already says is rented rather than written.

**Never rented:** the lexer, the parser, the compiler, the interpreter, the
object model and the collector. That list is the engine, and section 1 is why.

## 9. How correctness is measured

`CLAUDE.md` says the measure is alo rather than a conformance score, and that
rule was written against Web Platform Tests — a percentage scored against legacy
we deliberately refuse. A language is different in one respect: its specification
ships an executable test suite, and there is no honest way to claim an
interpreter is correct from a handful of examples.

So, in order:

- **Tables of small programs with the values the specification says**, which is
  already item 72's closing condition and is where a semantics argument gets
  settled in numbers rather than prose.
- **A frozen real page's own script**, which is stage 2's rule for everything: an
  item is opened by a page that fails and closed by the same page working.
- **test262, vendored per feature and frozen**, never fetched at test time
  (`LOOP.md`), and read as **a table rather than a score**. We take the sections
  for the feature being built, they go in the repository with the change, and a
  case that is expected to fail is written down **with why**. No percentage is
  computed and none is published: a number that goes up is exactly the incentive
  that makes an engine implement the easy half of everything.

## What this costs

- **Years, and section E waits on it.** Most of the DOM list, all of the storage
  list and the whole agent-on-real-pages promise are unreachable until this runs.
  That is the cost of section 1 and it is being paid deliberately.
- **We will be slower than every other browser at running script**, for a long
  time and possibly permanently. Law 3 says a wrong pixel is a bug and a slow one
  is a task — but a task with a ceiling: stage 2's exit gate is somebody using
  this as their browser for a week, and stuttering is a reason to stop. That is
  why section 2 is a refusal with a condition rather than a refusal.
- **Every bound in section 4 is a page we might break** that another engine
  renders. Each one is a constant with a reason, and a page that hits it is a
  bug report we can act on — which a crash is not.

## Alternatives rejected

**Boa, or another Rust engine, embedded.** Rejected, and it is the closest call
in this ADR: safe Rust, permissive licence, exists today, and it would save
years. Refused because the collector, the bounds and the object graph are exactly
where this browser's own promises are kept, and because adopting an engine means
adopting its idea of how a DOM is stored — the one structure ADR 0003 already
made a promise about. Reconsidering it is legitimate; doing so silently, inside a
commit that was mostly plumbing, is not.

**QuickJS through `rquickjs`.** Rejected: small, excellent, and C. It is the same
trade as V8 in a smaller package, and the smaller package makes it easier to
agree to without noticing.

**V8 through `rusty_v8`, or SpiderMonkey through `mozjs`.** Rejected by ADR 0001,
for the reason it gave: the saving is real and the cost is the argument.

**A tree-walking interpreter first, bytecode later.** Rejected: it is choosing to
implement every semantic twice, and the second time is after there are builtins
depending on the first.

**Design for a JIT now so we do not have to rewrite.** Rejected: it is the same
mistake in the other direction. A design shaped by a compiler nobody has measured
the need for is a slower interpreter and a harder one to make correct.

**Implement a subset and let pages adapt.** Rejected: nobody adapts for us, and a
browser that runs *most* of a page's script produces a page that is broken in
ways nobody can diagnose. Section 3 refuses legacy corners, which is a different
thing: those are corners real modern pages do not use, and each is opened by a
page that proves otherwise.

## What this does not decide

- **The collector.** Queue item 71, its own ADR, as section 6 says. Pauses,
  precision and how the DOM is traced are one decision and it is not this one.
- **The task boundary.** Queue item 76 and the event loop, which ADR 0012 § 4
  already leans on and explicitly leaves to that item.
- **The shape of the DOM bindings** beyond the one trait in section 6 — queue
  item 80, and the invalidation that has to follow a mutation.
- **WebAssembly.** It is on no list in this repository, and this decision does
  not put it on one.
- **A debugger protocol**, which is queue item 129's developer tools, though
  section 2's suspendable frame is what makes one possible.
- **What an agent may do with script.** A verb runs a page's own handlers through
  the ordinary event path (item 81); who is allowed to is queue items 93 and 133,
  and ADR 0012 is careful that a record is not a permission.

## How we will know if this was wrong

Two measurements, which is the standard ADR 0007, 0008, 0011 and 0012 set.

**If frozen real pages keep failing on things section 3 refused**, the line
between "the language pages ship" and "the legacy tail" is drawn in the wrong
place. The answer is to move it in an amendment to this ADR, with the pages
named — not to implement sloppy mode quietly because a page needed it.

**And if the person who has to use this for a week reaches for another browser
because it stutters rather than because a site is missing**, then section 2's
refusal is the thing to re-argue, with that measurement in hand. That is the
condition it was written with, and meeting it is not this decision failing — it
is this decision working the way it was meant to.
