# ADR 0019 — A realm names its host, and an unforgeable member is copied from it

**Status:** accepted
**Date:** 2026-10-06
**Context:** queue item 260, *`isTrusted`, as Web IDL's `[LegacyUnforgeable]`
attribute*, whose own text leaves the choice open — *a place the constructor
and the browser's dispatch can find that getter (the document cell's
interfaces, or the engine giving an embedder's constructor more than its
prototype)*; ADR 0018 §§ 4 and 8 (`isTrusted` is a member of the first cut,
`true` for the browser's dispatch); ADR 0017 § 4 (*a native is handed the
heap, its `this` and its arguments — and that is all it will ever be
handed*); ADR 0013 § 3 (*absent beats approximate*) and § 6 (the engine knows
nothing about the DOM); ADR 0014 (one heap, and every reference something the
collector walks); and the code this is about — `alo-js`'s `object/native.rs`
(a native is a plain `fn` and holds no edge; `Call` carries the heap, `this`,
the arguments and, since queue item 230, the realm's intrinsics),
`interpret/construct.rs` (`Instance::Made` is given only the constructor's
`prototype`), `realm.rs`, and `alo-bindings`' `install.rs`, `interface.rs`,
`event.rs` and `interface/event.rs`.

## The decision in one line

The engine's realm gains the ECMAScript specification's **`[[HostDefined]]`
field** — one reference an embedder sets once, rooted by the realm, of a type
the engine never learns — and a native is handed it beside the intrinsics;
`alo-bindings` sets it to the **document cell**, which holds, beside each
interface's prototype, each interface's **unforgeables**: the object Web IDL
calls `[[Unforgeables]]`, made once per realm, whose properties are copied
onto every instance as it is made — so `isTrusted` is an own accessor on every
event, not configurable, whose getter is one function per realm.

## Why this is a decision rather than a chore

Web IDL's `[LegacyUnforgeable]` attribute is an accessor property **on the
instance** — not on the prototype — with `configurable` false, and its getter
is made **once per realm**: Web IDL makes an object for the interface's
unforgeable members when the realm's interface object is made (its
`[[Unforgeables]]` slot), and *internally create a new object implementing the
interface* copies that object's properties onto every instance, for the
interface and every interface it inherits from. A page can observe all three
facts: `Object.getOwnPropertyDescriptor(e, "isTrusted")` is defined, `delete
e.isTrusted` fails, and two events' getters are the same function.

Making that object is easy; **finding it is the decision.** Two callers make
events:

- **The browser** (`event::create`, item 255) is handed the document cell, so
  it can find anything the cell holds.
- **The `Event` and `CustomEvent` constructors** are natives, handed the heap,
  their `this` — the instance the engine made from the constructor's
  `prototype`, which is a plain `Event` cell with no edge to anything — their
  arguments and the realm's intrinsics. Nothing they are handed reaches the
  document cell, which is where every other piece of a page's per-realm state
  lives (ADR 0017 § 4: the prototypes are the cell's because a native's `this`
  reaches it).

ADR 0017 § 4 settled how a native reaches its page — *through the object it
was called on* — and said that is *all it will ever be handed*. That was right
for every member it had to cover, each of which has a wrapper for its `this`.
It is wrong for a **constructor**, whose `this` is an object the engine just
made and which no page has seen yet, and the same gap is already written down
twice more in this repository:

- `install.rs`: `document` is a data property on the global object rather than
  Web IDL's unforgeable accessor, because *its getter … would have nowhere to
  find the document*. When the global object is a `Window` (item 251) that
  getter's `this` is the global object, an ordinary object with no edge to the
  cell.
- A constructor that makes a node — `new Text("x")`, `new
  DocumentFragment()`, `new Comment()` — must make it in *the current global
  object's associated `Document`*. Its `this` is a fresh instance too.

So the question is not where one getter lives. It is **how a native reaches
its realm's state when its `this` is not a wrapper**, and it has to be
answered once rather than three ways.

## 1. A realm has a host-defined value, and a native is handed it

ECMAScript's Realm Record has a field for exactly this: **`[[HostDefined]]`,
*reserved for use by hosts that need to associate additional information with
a Realm Record***. HTML keeps its environment settings object there, and that
is how a browser's constructor finds its document.

`alo-js` gains it:

- **`Engine::host_defined(value)`** sets it, **once**. A realm's host is fixed
  for its life; a second call is refused with an error naming that, because an
  embedder setting it twice is an embedder bug and the first value stands —
  the rule `install` already applies to `document`.
- The realm **roots** it, as it roots its intrinsics and its global object, so
  whatever the embedder hangs from it lives exactly as long as the realm.
- **`Call::host_defined()`** answers it to a native, beside
  `Call::intrinsics()` — the realm the native is *called in*, which is today
  the only realm an engine has. `None` when no embedder set one: a builtin of
  the engine's own never asks, and an embedder's native that finds none is the
  embedder's bug, a fault rather than a page's error.

It is a **reference and nothing else**. The engine never learns what it
refers to: an embedder reads it back with the typed borrow ADR 0017 § 4 already
gave it (`embedded::<T>`), which answers `None` for any other kind of cell. A
console, a test harness and the DOM are all embedders that may set it, and
ADR 0013 § 6 holds — the engine still depends on nothing of the DOM's.

## 2. `alo-bindings` sets it to the document cell

`install` sets the realm's host-defined value to **the document cell**, before
it makes anything a page can call. The cell already holds the page's
prototypes for ADR 0017 § 4's reason, and it is the one object whose life is
the page's; nothing new is introduced to hold per-realm state.

**ADR 0017 § 4 is amended, not replaced.** A native acting on a node still
reaches it **through its `this`**, with the Web IDL brand check as its
`TypeError`; the realm's host is for what has no `this` to reach through — a
constructor's instance, and the global object's own members once it is a
`Window`. A member that has a wrapper for its `this` keeps using the wrapper's
cell: today the two are the same cell, and when a heap holds more than one
document (frames) the wrapper's is the one the standard means for a node's
operation.

## 3. An interface's unforgeables are an object, made once, and copied

Beside each interface's prototype, the document cell's `Interfaces` holds that
interface's **unforgeables** — an ordinary object with no prototype, made in
`furnish` with the prototypes, holding the interface's `[LegacyUnforgeable]`
members as Web IDL defines them on it. Only `Event` has one now: `isTrusted`,
an accessor whose getter is a native reading the event's trusted flag, with no
setter, enumerable, **not configurable**. Every other interface's is empty and
is not made — an empty slot, not an empty object.

Making an instance **copies** the unforgeables of the interface and of every
interface it inherits from onto it, property by property, before anything else
is done to it:

- **`new Event(…)` and `new CustomEvent(…)`** copy at their first step, which
  runs before the type is converted or the dictionary read — before any page
  script can run inside the constructor — reaching the cell through
  `Call::host_defined()`.
- **`event::create`**, the browser's *create an event*, copies them from the
  cell it is already handed.

Copying the **property** rather than making a getter per instance is what
makes the getter *one function per realm*: every event's `isTrusted` holds
the same function object, as Web IDL's do. And because the copying is one
function in `alo-bindings`, the two callers cannot disagree about what an
instance gets.

## 4. What it costs a page that never runs script

Nothing. A page with no script has no heap (ADR 0017 § 2), so no realm, no
host-defined value and no unforgeables object; stage 1's pages are unchanged.

## What this costs

**One more thing a native is handed.** ADR 0017 § 4's sentence was *that is
all it will ever be handed*, and queue item 230 had already added the
intrinsics; this adds the realm's one host reference. The rule that made the
sentence worth writing survives: a native keeps nothing across a suspension
but a step number, holds no edge of its own, and reaches its node through its
`this`.

**An engine change for an attribute.** Small — a field on the realm, one root,
one accessor on `Call` — and generic: it is the specification's own field, and
`Window`'s `document` and every node constructor need the same thing.

**Every event carries one more property** in its own table. That is what Web
IDL makes it carry, and what a page can observe.

## Alternatives rejected

- **`isTrusted` on `Event.prototype`.** Every reader would get the right
  answer, and `Object.getOwnPropertyDescriptor(e, "isTrusted")` and a page's
  own `Object.defineProperty(Event.prototype, "isTrusted", …)` would each give
  the wrong one. That is the approximate member ADR 0013 § 3 refuses, and the
  member `[LegacyUnforgeable]` exists to prevent: a page that can replace a
  prototype's getter can make every event claim to be trusted.
- **A getter made per instance.** Own and unforgeable, and two events'
  getters different functions where Web IDL's are one — observable, and a
  function allocated for every event a page makes.
- **`Event.prototype` as an embedder cell with a hidden edge to the document
  cell**, found by walking the instance's prototype. No engine change, but an
  interface prototype object is an ordinary object in Web IDL and this one
  would only behave like one; it would break as soon as a derived class's
  instance (item 223) inherits from a prototype the page made; and it answers
  only for events — `Window`'s `document` and `new Text()` would each need a
  trick of their own.
- **Reading `document` off the global object.** It is non-configurable today,
  so it would work — until item 251 makes it Web IDL's accessor, whose getter
  needs exactly the thing this decides. A route through a property a page can
  see, to find the state that property is supposed to be computed from, is
  circular.
- **Natives that carry data** — a closure, or a slot on the function object.
  `object::native` refuses captured state so that a native holds no edge the
  collector cannot see; and the state is the realm's, not each function's,
  so every constructor would carry its own copy of the same reference.
- **`Instance::Made` given more than the prototype.** `Make` is a function
  pointer that makes a cell before it is in the heap, and cannot read the
  heap to find anything; widening it to take the heap moves the same
  question into the engine's construction path and answers it only for
  constructors.

## What this does not decide

- **Several realms in one heap** — frames, and whether they share an event
  loop. The field is per realm, and a native reads the realm it is called in;
  which realm that is when there is more than one is the item that builds the
  second.
- **`document` as Web IDL's accessor on a `Window`** — item 251. This makes it
  buildable; it does not build it.
- **Node constructors** (`new Text()` and the rest) — each with the page or the
  item that needs it, under ADR 0017 § 8.
- **Every other `[LegacyUnforgeable]` member.** `Location`'s and `Document`'s
  `location`, and `Window`'s `window`, `document`, `location` and `top`, use
  the same unforgeables object when their interfaces are built.

## How we will know if this was wrong

**If a frozen page sees a property on an event, or the absence of one, that
another engine does not show it** — a getter that differs between two events,
or an `isTrusted` a page managed to redefine — § 3 is wrong, and it is fixed in
the one function that copies.

**If a second realm in one heap needs its host found some other way than "the
realm the native was called in"**, § 1 was too narrow, and the replacement
says which realm the standard meant and how a native reaches it.
