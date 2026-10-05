# ADR 0017 — The document moves into the heap, and a wrapper lives as long as its tree

**Status:** accepted
**Date:** 2026-10-05
**Context:** ADR 0014 § 6 (*one graph: the DOM is traced, not counted*), which
fixes two rules — the trait lives in `alo-js`, and a wrapper is **one per node
for as long as the node lives** — and whose *What this does not decide* names
**the shape of the DOM bindings beyond the trace trait and the one-wrapper
rule** as queue item 80's; ADR 0013 § 6 (the engine knows nothing about the
DOM) and § 3 (*absent beats approximate*); ADR 0003 (node identity is allocated
once and never reused), whose consequences say a detached node's slot is not
freed *because peak memory is bounded by the input* — true of a parser, false
of a script; ADR 0002 (the layout tree is the agent's tree); ADR 0016 § 5 (a
frame is the browser process's to call) and § 6 (an `Act` is one task);
`CLAUDE.md`'s law 1 (no legacy DOM surface) and law 3 (correct before fast);
`docs/autonomy/QUEUE.md` item 80, *mutation from script, and the invalidation
that has to follow it*, whose closing condition is *a script changes a document
and the next render shows it, with node identity surviving (ADR 0003)*

## The decision in one line

A page's `alo_dom::Document` **moves into its heap** — one embedder cell,
rooted by the renderer — the moment the page first runs script, and has exactly
that one owner for the rest of the page's life; a node's **wrapper** is an
embedder cell holding the node's id and a reference to that document cell, made
on demand and kept **while the node's tree is reachable**: the document's own
tree always, a detached tree while any of its wrappers is; native code reaches
a node **only through the object it was called on**, never through anything
ambient; every change goes through **`alo-dom`'s own operations**, which hold
the DOM standard's validity rules for script and agent alike; and a changed
document is **rendered again whole, from the same document, the next time
anything reads its rendering** — never inside a task.

## Why this is a decision rather than a chore

Item 80 looks like a list of methods — `createElement`, `appendChild`,
`removeChild` — and every one of them is easy once four questions are answered.
None of them is answered yet, and each decides something no later item can
change cheaply.

**Where the document lives** decides who may change it. Today `Rendered` owns
it, the agent's `apply` borrows it mutably, and the scripts at load borrow it
immutably after the render. A script that can mutate needs it *during* a task,
from inside a native function that is handed the heap and nothing else
(`object::native`: *a native function holds no edge at all*). Answer this
wrong and there are two owners, or a pointer, or a `RefCell` that panics.

**How long a wrapper lives** is the clause a script can observe. A wrapper
dropped and remade is an object whose expando properties vanish and whose
identity a page can see change — ADR 0014 § 6 forbids it — and a wrapper kept
for ever is a page that leaks every node it ever made, which with script is
no longer *bounded by the input* as ADR 0003 assumed.

**How a native finds its node** decides whether the engine stays ignorant of
the DOM (ADR 0013 § 6). The tempting answer — hand every native a host pointer
— makes the document reachable from `Array.prototype.push`.

**When a change is rendered** decides what the agent reads (ADR 0002) and when
a person sees it (ADR 0016 § 5). Rendering inside a task is a layout a script
can force a hundred times in a loop; never rendering is the closing condition
failing.

## 1. A bindings crate, and it is the only one that names both

`alo-bindings` depends on `alo-js` and `alo-dom` and is the only crate that
does — ADR 0014 § 6's rule, given a name. `alo-renderer` depends on it, as it
already depends on both halves. `alo-dom` gains no dependency on the engine and
the engine none on the DOM, so a renderer that never runs script never builds a
heap, and stage 1 renders exactly as it did.

Inside it: **one file per interface** (`Node`, `Element`, `Document`,
`Text`, and each later one), each holding that interface's prototype — its
attributes as accessor properties whose halves are native functions, its
operations as native methods — and the WebIDL conversions its arguments need.
The file is the interface's one reason to change (`CLAUDE.md`). Writing them
by hand rather than generating them from IDL is the first-cut choice: a
generator is a second compiler to keep correct, and it earns its place when the
hand-written ones show a pattern worth generating, not before.

## 2. The document moves into the heap, and has one owner from then on

**Until a page runs script, the document is the renderer's**, owned by what
the pipeline rendered, exactly as today. **When the page's first script is
about to run, the document moves into the page's heap** as one embedder cell —
the *document cell* — and the renderer holds that cell by one `Root` (ADR 0014
§ 2: *the embedder's own roots, the document*). It never moves back: a page
whose document has been in script's hands keeps it there until the page goes,
and the page's heap goes with it.

Everything else that reads the document — style, layout, paint, the agent's
tree, `apply` — **borrows it from the cell** for as long as the read lasts, and
never across a point where script could run. So the borrow is an ordinary Rust
borrow out of the heap, checked by the compiler, and there is no second owner,
no shared pointer and no runtime borrow flag to fail.

Rejected: **the document beside the heap, lent to every native as a host
pointer.** It is how many engines are embedded, and it fails three ways here.
It makes the document reachable from every builtin, including the language's
own (ADR 0013 § 6). It leaves the wrappers' table outside the heap, so § 3's
liveness would have to be a set of roots maintained by hand. And it adds a
field to `Call` that the engine must thread through every suspension of a
builtin (`Answer::Want`), for a value the engine is forbidden to know the type
of.

Rejected: **`Rc<RefCell<Document>>` shared by the renderer and the cell.** Safe
code, and a borrow that fails at run time is a panic or a refusal in a place
nothing can report it — the reason ADR 0014 § 1 refused `RefCell` throughout.

**The cell's footprint is the document's size** (`Trace::footprint`). A node
lives in `alo-dom`'s arena rather than in a heap slot, and a script that makes
nodes without the heap counting them could exhaust a renderer below the heap's
ceiling (ADR 0014 § 9). Counted, a page that builds an unbounded document meets
the same ceiling an unbounded array does, collects first, and is stopped with
a reason second.

## 3. A wrapper lives as long as its tree is reachable

A wrapper is an embedder cell holding **the node's id, a reference to the
document cell, and an ordinary object's part** — its prototype and its own
properties, so a page may hang expandos off a node as every page does. It is
made the first time script asks for the node, and the document cell keeps the
table from node to wrapper, so asking again gives the same object (ADR 0014
§ 6's one-wrapper rule).

**When is a node alive?** The DOM answers it: a node is reachable from script
exactly when its **tree** is — `parentNode` and `firstChild` walk the whole
tree from any node in it. So:

- a node in **the document's own tree** is alive, and its wrapper with it,
  for as long as the document is: the document cell traces **every wrapper in
  its table whose node is attached** as a strong edge;
- a node in a **detached tree** — removed, or created and never inserted — is
  alive while **any** wrapper of any node in that tree is alive. The document
  cell reports each detached tree's wrappers as a **ring of ephemerons** (each
  wrapper the key to the next, the last to the first), and ADR 0014 § 7's
  fixpoint marks the whole ring the moment any one of them is marked. A ring of
  *n* wrappers is *n* pairs, not *n²*;
- a `<template>`'s contents are part of their template's tree for this purpose,
  since the template reaches them.

**What is not reachable is freed, not recycled.** At the sweep
(`Trace::clear_weak`), the document cell drops its table's entries for
wrappers that did not survive, and every detached tree **none** of whose nodes
has a surviving wrapper is released from `alo-dom`: its nodes' contents are
dropped and **their ids answer nothing** from then on. A detached tree nobody
ever wrapped — `innerHTML` replacing children no script held — is unreachable
the moment it is detached and goes at the next collection.

This keeps ADR 0003's promise at the level it was made — **no id is ever
reused**; the arena keeps a tombstone where the node was, and a stale id asks
a question that answers *nothing* rather than a confident wrong node — and
retires the consequence that assumed *peak memory follows the input*. The
tombstone is a few bytes per node ever created, counted in the cell's
footprint; a page that creates and drops nodes for ever meets the heap's
ceiling and is stopped with a reason, after an unreasonably long time, rather
than growing unseen. ADR 0003's *compaction pass that rewrites ids as one
explicit step* stays the answer if a real page ever makes the tombstones
expensive, and it is not built until one does.

ADR 0014 § 6's cycle — *node → listener → closure → node* — is then one graph,
as it promised: a listener is held by its node's wrapper (item 81), the
wrapper by its tree, and a detached tree with nothing in script reaching it is
collected listener, closure and all.

Rejected: **every wrapper rooted until the page goes.** One line of code and
a leak in every single-page application, which never goes; ADR 0014 refused
*free the arena when the page goes away* for exactly that page.

Rejected: **a wrapper weak, remade on demand.** The table would be smaller and
the page would see `el.foo` vanish after a collection — the failure ADR 0014
§ 6 names.

## 4. A native reaches its node through the object it was called on

A DOM method's body is a native function (`object::native`), handed the heap,
its `this` and its arguments — and that is all it will ever be handed. To act
on a node it asks the heap for **its `this` as a wrapper**, and through the
wrapper's reference for the document cell.

So the engine gains one thing: **an embedder may ask for its own exotic object
back by type** — a typed borrow of a cell an embedder made, shared or
exclusive, answering `None` for any other kind of cell. It is safe Rust
(`std::any::Any`), the engine learns no type of the embedder's, and the
`Exotic` trait gains the one method that makes the borrow possible.

**A wrong `this` is a `TypeError`**, the WebIDL brand check: calling
`Node.prototype.appendChild` on a plain object, or on a `Text` where an
`Element` is required, throws rather than guesses. A wrapper whose document is
not the one the cell holds — impossible today, since a renderer holds one page
— is the same `TypeError` rather than a cross-document write.

A native that both changes the document and makes a wrapper **releases its
borrow of the document cell before it allocates**, which the borrow checker
enforces (`object::internal`: *an exotic object may not allocate*) — the
discipline ADR 0014 already asks of every builtin, with the compiler checking
the one place it would bite.

## 5. Every change goes through `alo-dom`, under the standard's rules

`alo-dom`'s tree operations stop being `pub(crate)`. They become **the DOM
standard's own operations by name** — insert, append, replace, remove, with
the **pre-insertion validity checks** — and they answer a refusal rather than
`false`: a node inserted into its own descendant, a second element under the
document, a doctype where one may not go. The bindings turn a refusal into the
`DOMException` the standard names (`HierarchyRequestError`, `NotFoundError`);
the agent's `apply` gets the same refusals from the same function.

The rules live in `alo-dom` rather than in the bindings because **there are
two callers**: a script and an agent change one document, and two copies of
*what is a valid insertion* is how they come to disagree. The parser keeps its
own crate-private operations, which the HTML parsing algorithm specifies
differently and which never refuse.

**Every change advances the document's change count**, a number in `alo-dom`
the renderer compares to decide whether what it rendered is stale (§ 6). A
counter rather than a list of what changed: the first renderer re-renders
whole, and a list it does not read is a cache it would have to keep correct.

A node a script creates gets its id **from the same counter as the parser's**
(ADR 0003), so `createElement` on a page with 40 parsed nodes makes `#40`, and
the agent can name it, act on it and come back to it like any other.

`DOMException` is the bindings' first interface that is not a node: an error
object a page's `catch` receives, with `name` and `message` as the standard
gives them and `Error.prototype` on its chain — the engine's error machinery
(queue item 227), extended rather than imitated.

## 6. A changed document is rendered again whole, when its rendering is read

**Correct before fast** (law 3), and `Renderer::act` already does it: when the
document has changed, the page is rendered again — style, boxes, layout and
paint — **from the same document**, never re-parsed, so every id the agent
holds still names what it named.

**When** is decided by who reads the rendering, never by the script:

- a **rendering opportunity** (ADR 0016 § 5; today's `Paint`), after the
  current task and its checkpoint — where a person's frame comes from;
- **`ReadTree`** and an **`Act`'s decision**, because the agent reads the
  layout tree (ADR 0002) and an agent reading a tree the page has already
  changed is an agent acting on a page that is not there;
- **the end of a `Load`**, after the page's scripts, so what the load reports —
  its issues, the fonts it wants — is about the page the scripts left, not the
  markup it arrived as;
- and a **`Resize`**, which lays out **the document the page has** rather than
  parsing its markup again, as `lay_out`'s own comment says it must once a
  script can change one.

**Never inside a task.** No script-visible API reads layout yet, so nothing
can observe a stale layout during a task, and a page that changes the document
ten thousand times in a loop costs ten thousand tree operations and one render.
When an API that reads geometry arrives (`getBoundingClientRect`,
`offsetWidth`), it renders the same way, synchronously, when the count says the
rendering is stale — the specification's forced layout, decided then with that
API in hand, since it is the one place a script chooses how often layout runs.

Rendering only what changed is item 113 and waits for a measurement; the
change count is the hook it will start from.

## 7. A parser-inserted script sees the document up to its own element

Today a page's scripts run after the whole document is parsed and rendered.
While no script could see the document that order was unobservable; with
bindings it is the first thing a page observes. An inline script in the middle
of `<body>` that reads `document.body.lastChild` must find **its own
`<script>` element**, not the paragraph written after it — every page that
inserts content beside the script that does it depends on this.

So **a classic, parser-inserted script runs when the parser reaches its end
tag**, against the document parsed so far, and the parser continues after its
task and checkpoint. `html5ever` already reports the moment — its tokenizer
stops and hands back the script element when the tree builder reaches a
script's end tag (`TokenizerResult::Script`) — so this is a change to
`alo-dom`'s `parse` and to the renderer's load, not a new parser. There is no
`document.write` (law 1), so what the parser continues with is exactly the
bytes it was going to read. The page is rendered once, at the end of its load
(§ 6), as now. A fetched script (item 238) and a module (item 77) keep their
own timing, decided with them.

## 8. The surface is the modern one, and absent beats approximate

Law 1 decides what is never built here: **no live collections** —
`childNodes`, `children` and `getElementsByTagName` return live views that
are stage 3's (queue item 137), scheduled by a page that fails without them —
no `document.write`, and no legacy reflection. Where a modern static form
exists, it is the one built: `querySelectorAll` returns a static list.

ADR 0013 § 3 decides what an absent member looks like: **absent**, so
`typeof document.createRange` is `"undefined"` and a page's feature test reads
it correctly, rather than a method that does half of what it should.

The first members are item 80's closing condition and nothing more — the
document and its root element, making elements and text, the four structural
changes the item names (create, append, remove, replace), reading and writing
a node's text and attributes, and walking to a parent, a child and a sibling.
Each later member is added when a page or an item needs it, in its
interface's file.

## What this costs

**The document is in the heap for the rest of a scripted page's life.**
Rendering borrows it from the cell, so the pipeline's entry point takes a
borrowed document rather than an owned one, and `Rendered` stops owning it.
That is a change to stage 1 code made for stage 2's sake; stage 1's behaviour
does not change and its reference renders say so.

**A render after every change that is read.** A page that mutates on every
frame lays out the whole page every frame until item 113. That is law 3's
trade, stated, and its cost is a measurement on hardware.

**A ring per detached tree** is work at every collection proportional to the
number of wrapped detached nodes. Bounded by the heap's ceiling, since each is
a wrapper.

**Tombstones.** A few bytes per node a page ever created, until a compaction
step is built — § 3.

**A script now changes when it runs** (§ 7). The renderer's tests that load
scripts were written when the order was unobservable; the item that builds § 7
re-reads each against the new order rather than assuming it still holds.

## Alternatives rejected

- **The document beside the heap, lent to natives as a host pointer** — § 2.
- **`Rc<RefCell<Document>>`** — § 2.
- **Every wrapper rooted for the page's life** — § 3, the leak.
- **Weak wrappers remade on demand** — § 3, the vanishing expando.
- **Reference-counted nodes with wrappers as their owners** — two collectors,
  ADR 0014 § 1.
- **The validity rules in the bindings** — § 5: the agent changes the same
  document, and two copies drift.
- **Incremental style and layout now** — § 6, item 113, a cache built before
  the behaviour it caches is settled.
- **Generated bindings from WebIDL now** — § 1, a second compiler before there
  is a pattern for it to generate.
- **Running scripts after the parse, as today** — § 7: observably wrong on the
  first page that reads its own surroundings.

## What this does not decide

- **Events** — item 81: dispatch, capture and bubble, default actions. This
  decides only that a listener is held by its node's wrapper.
- **Mutation observers** and the microtask they queue — when a page needs one,
  against ADR 0016's checkpoint.
- **Geometry from script and forced layout** — § 6 says how; the API decides
  when.
- **Shadow trees and custom elements** — item 87; a shadow root is a tree,
  and § 3's rule will be asked of it then.
- **`innerHTML` and the fragment parser from script** — `alo-dom` has the
  fragment parser; exposing it is a member added in its interface's file, with
  the same validity rules.
- **Identity across a reload** — still ADR 0003's open question.

## How we will know if this was wrong

**If a frozen page sees a node's expando vanish or a wrapper change identity**,
§ 3's liveness has a hole, and the fix is in the ring or the table, never a
root added to hide it.

**If a page left open for a day grows while its reachable node count does
not**, something holds a tree § 3 should have freed — ADR 0014's own test,
applied at the boundary it named.

**And if the person using this for a week finds a page janking on every
change**, § 6's whole re-render is the thing to replace, with that measurement
— item 113, starting from the change count.
