# ADR 0037 — The global object is a `Window` the embedder makes, and it is last on every path

**Status:** accepted
**Date:** 2026-10-09
**Context:** queue item 362, *`window`, and the global object as an event
target*, opened by a page: `alo-sites-cta`, the call-to-action section alo
Sites publishes for its customers. Its analytics script now runs past
`Date.now()`, `encodeURIComponent` and `location.pathname`, and stops on line
32, `window.addEventListener("pagehide", record)`, with "ReferenceError:
'window' is not defined". The item asks two questions before anything is
built: *whether the global object becoming an event target is 251's step or
can come before it, and where an event dispatched at the window sits on a
node's path*. It says that if ADRs 0017–0019 do not decide them, the item is
`needs design` first. They do not:
- ADR 0018 § 1 says the global *becomes one with item 251 (`Window`, which
  waits on item 73)*, and that a path *gains the global when it is a
  `Window`*. It does not say what makes it one, where its listeners live, or
  what the path is.
- ADR 0019 § 2 says the realm's host is for *the global object's own members
  once it is a `Window`*. It does not say how it becomes one.

Also read: ADR 0013 §§ 3 and 6 (absent beats approximate; the engine knows
nothing of the DOM); ADR 0014 (one heap, and every edge something the
collector walks); ADR 0017 § 4 (a native reaches its page through its `this`);
ADR 0005 (one process per site); `CLAUDE.md`'s law 1. And the code this is
about:
- `alo-js`'s `realm.rs`: `Realm::new` makes the global object as an ordinary
  object inheriting from `Object.prototype`, and defines the language's values
  and builtins on it.
- `alo-js`'s `object/native.rs`: `Make`, an embedder's function that makes its
  cell from a prototype.
- `alo-bindings`' `install.rs`, `location.rs`, `listeners.rs`, `dispatch.rs`
  (whose path is `Vec<NodeId>`) and `interface/event_target.rs` (whose brand
  check accepts a node's wrapper).

## The decision in one line

A page's global object is **a `Window`, an embedder cell `alo-bindings`
makes**. The engine gains ECMAScript's own provision for this: a realm whose
global object the host makes. That cell **holds its own listener list**, as a
node's wrapper does. Its members are **its own properties**, because `Window`
is Web IDL's `[Global]`: `window` and `self` answer the global object itself,
since there is no `WindowProxy` without frames. An event's path **ends at the
`Window`** after the page's document, except for `load`. And this is item
362's step, **before** 251, because only `document`'s property descriptor
needs item 73.

## Why this is a decision rather than a chore

`window` looks like one line: a property of the global object that answers
the global object. A page observes four things beyond it.

**What the global object is.** `addEventListener` on it must keep a listener
somewhere the collector walks, and find it again when an event is dispatched.
Today the global is an ordinary object the engine made. Whatever holds the
window's listeners is either that object, which it cannot be, or something
beside it.

**Where an event goes.** In HTML a click on a button reaches a capturing
listener on the window *before* the document's, and a bubbling one after. A
page that listens on `window` for `click`, `keydown` or `error` depends on
that order. ADR 0018 § 2's path stops at the document.

**What `window` is.** HTML answers a `WindowProxy`, not the `Window`. The two
differ only when one browsing context's global object is replaced by
another.

**When.** ADR 0018 tied the window to item 73, the standard library. That is
years of work, and a page is waiting now.

## 1. A realm's global object may be the host's

ECMAScript's *InitializeHostDefinedRealm* says it plainly: *if the host
requires use of an exotic object to serve as realm's global object, let global
be such an object created in a host-defined manner*. HTML requires it, and
the `Window` is that object.

`alo-js` gains it the way ADR 0019 gave it `[[HostDefined]]`:
- **An engine may be made with a global object an embedder makes**, from a
  `Make` given the realm's `Object.prototype`. The realm then defines
  `undefined`, `globalThis`, the errors, `Promise`, `Date` and the global
  functions on that cell, exactly as on an ordinary one.
- **Every way a script reaches the global must work on such a cell**: a name
  resolved, a `var` or function declared, `globalThis`, the top-level `this`,
  an assignment in sloppy and strict code. A cell answers these through
  `Ordinary` storage as a wrapper does, so this is a test to write, not a new
  mechanism. It is tested in `alo-js` with an embedder cell of the test's
  own, under `Heap::stress`.
- **`Engine::new` and `Engine::with_clock` are unchanged.** Their global is
  ordinary, for every embedder that is not a page.

The engine learns nothing of windows (ADR 0013 § 6). It holds a reference to
a cell it cannot name, as it already does for `[[HostDefined]]`.

**The prototype is set once, by `install`.** `Window.prototype` cannot exist
before the realm, whose intrinsics it inherits from, so the cell is made
inheriting from `Object.prototype`. `install` then sets its `[[Prototype]]`
to `Window.prototype` after `furnish`, before any script can run. No page
can observe the interval.

## 2. The `Window` holds its listeners and its document

The `Window` cell (`alo-bindings`, one file) holds:
- **its listener list**, the same `listeners.rs` list a node's wrapper holds,
  traced as strong edges and counted in its footprint (ADR 0018 § 1, ADR 0014
  § 9);
- **an edge to the document cell**: its *associated `Document`*.

The document cell holds an edge back to it, set by `install`. A dispatch that
starts at a node finds the window there, and both are rooted by the realm for
as long as the page lives.

ADR 0018 § 1's sentence, *a listener lives in its target's wrapper*, holds
unchanged. A `Window` is its own wrapper: there is no node behind it for a
wrapper to stand in for. ADR 0018 rejected a table of listeners in the
document cell, keyed by node, and this does not bring one back.

**The brand check widens by one kind.** `EventTarget`'s members accept a
node's wrapper **or a `Window`** as their `this`. They refuse anything else
with the same `TypeError` as today. The listener list is reached through the
same accessor for both, so `addEventListener`'s options, `handleEvent`,
`once`, `passive` and the *removed* rule cannot differ between a node and the
window.

`passive` by default for `touchstart`, `touchmove`, `wheel` and
`mousewheel` covers the window too. It is the first target that list names
in the standard, and it is added with the rest.

## 3. The path ends at the `Window`, except for `load`

HTML gives a `Document` a *get the parent*: **its relevant global object,
unless the event's type is `load` or the document has no browsing context**.
So the dispatch path in `dispatch.rs` is:
- the target node and its ancestors up to their root, as now;
- then, **if that root is the realm's own document** (the document cell's)
  **and the event's type is not `load`**, the `Window`.

A dispatch **at** the window has a path of the `Window` alone.

A path entry is therefore *a node, or the window* rather than a node id. That
is one type in `dispatch.rs`, not a second stepper. Capture visits the window
first and bubble last. `currentTarget`, `eventPhase` and `composedPath()` read
it as they read a node, and *at target* is the window when it was the target.

This changes what existing dispatches do, and that is the point. The
browser's trusted `click` for an agent's `Activate`, its `beforeinput`,
`input` and `change`, and every script's `dispatchEvent` at a node in the
document now reach the window's listeners too. A page that has none sees
nothing different. The tests that pin each path gain the window at its end.

A document with no browsing context, which `createHTMLDocument` or
`DOMParser` would make, is not the cell's document. Its paths stop at its
root, as HTML's do. No such document exists yet, and the rule is written now
so that it is not discovered later.

## 4. Its members are its own, and `window` is the global object

Web IDL puts a `[Global]` interface's regular attributes and operations **on
the object itself**, not on its prototype. Its unforgeable members are its
own anyway (ADR 0019 § 3). `EventTarget` is not `[Global]`, so
`addEventListener` stays on `EventTarget.prototype` and `window` inherits it.
`window.hasOwnProperty("addEventListener")` is `false`, as in every browser.

What this item builds:
- **`window`**: `[LegacyUnforgeable]`. Made in `Window`'s unforgeables
  object (ADR 0019 § 3) and copied onto the one instance: an own accessor,
  enumerable, not configurable, with no setter. Its getter answers its
  `this` after the `Window` brand check.
- **`self`**: `[Replaceable]`. An own accessor, enumerable and configurable.
  Its getter answers its `this` after the brand check. Its setter does what
  `[Replaceable]` says: it defines an own data property of that name with
  the value assigned, so a page's `self = 1` replaces it.
- **`location`** moves to `Window`'s unforgeables, keeping its shape, and its
  getter gains the `Window` brand check. `location.rs` named that check as
  the one thing its getter lacked.
- **`document`** stays the non-writable, non-configurable data property it
  is. Its accessor is item 251's (§ 6).

**`window`, `self`, `globalThis` and the top-level `this` are all the global
object.** HTML answers a `WindowProxy` for each. A proxy exists so that a
reference survives its browsing context navigating to a new document, which
brings a new global object. Here every document gets a new realm (ADR 0017
§ 2) and every site its own process (ADR 0005). No script can hold a
reference to a window from another realm until there are frames or
`window.open` (items 86 and 118). Until then a proxy would forward every
operation to the one object it could ever point at, with nothing that tells
the two apart. It is built with the item that makes a second window
reachable.

**`Window.prototype`** inherits from `EventTarget.prototype` directly. Web
IDL puts a *named properties object* between them, and its one purpose is
**named access on the window**: `window.foo` answering the element whose
`id` is `foo`. That is legacy DOM surface, refused by law 1, so neither the
object nor the access exists. A page that relies on it fails visibly with a
`ReferenceError` or `undefined`, which it can be fixed for. Silently matching
ids would not tell it.

**The interface object `Window` is not put on the global object**, as no
interface object a page cannot construct is yet. That is the rule
`install.rs` follows, and this does not change it.

## 5. What the browser dispatches at the window: nothing yet

`pagehide` and `visibilitychange` are the page lifecycle: the browser saying
a page is being left or hidden. They are fired when a tab is hidden, a page
navigated away from, or a renderer told its page is going. None of those is
told to a renderer today.

So the window **can be listened on and dispatched at by script**, and the
browser fires nothing at it until the lifecycle is built. That is queue item
**364**, which decides when the browser process tells a renderer and what
the page may do in the time it is given. `alo-sites-cta`'s `record` is then
reached by the event the page waits for, rather than by a test calling it.

This is not the approximate member ADR 0018 § 1 refused, *a function that
registers listeners nothing will ever dispatch to*. Something does dispatch
to it: a script's `dispatchEvent`, and every event dispatched at a node in
the document that bubbles or is captured.

## 6. This is 362's step, and 251 keeps only `document`'s accessor

ADR 0018 § 1 tied the window to item 251 and 251 to item 73. Both ties were
about `document`: its accessor differs from today's data property **only in
its property descriptor**, and only `Object.getOwnPropertyDescriptor`, item
73's, can read one. Nothing in this ADR needs item 73.
- **ADR 0018 § 1 is amended**: the global object becomes an event target
  with item 362, not 251.
- **Item 251 shrinks** to `document` as Web IDL's unforgeable accessor on
  the `Window`. Its getter reads the document through the `Window` cell's
  edge. It still waits on item 73, the only thing that can observe it.
- **Item 363** is cut from this: the `Window`'s exotic `[[SetPrototypeOf]]`.
  A `[Global]` object's prototype is immutable, so
  `Object.setPrototypeOf(window, x)` throws. That too is reachable only
  through item 73.

## What this costs

**An engine change.** A realm can be made with an embedder's global object.
It is ECMAScript's own provision and small, and the engine still learns no
DOM type.

**Every event dispatched at a node in the document takes one step more**,
for the window, even when nobody listens there. That is a copy of an empty
list. It is what HTML does, and it is cheaper than a page discovering that
its window listener never hears a click.

**Named access on the window is absent** (law 1). Old pages that write
`myForm.submit()` for an element with `id="myForm"` stop with a
`ReferenceError`. That is the refusal working, and stated.

## Alternatives rejected

- **The global stays ordinary, and its listeners live in the document cell.**
  A second place a listener lives. Its brand check would be *is `this` the
  realm's global*, an identity test no other target has. And the
  `[Global]` exotic behaviours (item 363) would have nowhere to go but a
  special case in the engine's ordinary object. ADR 0018 rejected the same
  table for nodes.
- **Replacing the realm's global after the realm is made.** The language's
  values and builtins are already on the first object. Copying them is a
  second definition of what a realm's global holds, and the first object
  would still have been `globalThis` for no reason.
- **Waiting for item 73**, as ADR 0018 § 1 had it. Item 73 is the whole
  standard library. § 6 shows that what waits on it is one descriptor, not
  the window.
- **A `WindowProxy` now.** § 4: it forwards to the only object it could ever
  forward to, and its own behaviours (cross-origin access, navigation) are
  items 86 and 85's.
- **The named properties object, empty.** An object that exists only to do
  a thing that is refused is the approximate member ADR 0013 § 3 refuses.
- **The window not on a node's path until something fires at it.** § 3:
  every page that listens on the window for an event at a node would hear
  nothing, which is the bug HTML's *get the parent* exists to prevent.

## What this does not decide

- **The page lifecycle**: `pagehide`, `visibilitychange`,
  `document.visibilityState`, and when the browser fires them (item 364).
- **`innerWidth`, `innerHeight`, `scrollY`** and the rest of the viewport a
  script reads. Each tells a page something about the person's window, so
  each is its own question when a script reaches it. ADR 0030's rule, *nothing
  about the machine*, is where that question starts.
- **`load`, `DOMContentLoaded`, `error` and `unhandledrejection`** at the
  window. § 3 writes the `load` exception into the path, and each event is
  fired by the item that needs it.
- **Event handler properties** on the window (`window.onload = f`): item 259.
- **Frames, `WindowProxy`, `top`, `parent`, `opener`**: items 86 and 118.

## What this makes buildable

- **362. `window`, and the global object as an event target.** As the queue
  writes it, now designed:
  - `alo-js`: an engine made with an embedder's global (§ 1).
  - `alo-bindings`: the `Window` cell, `Interface::Window` inheriting
    `EventTarget`, and its unforgeables `window` and `location`; `self`;
    `install` making and furnishing it; the brand check (§ 2); the path
    (§ 3).
  - `alo-renderer`: the engine it makes for a page has a `Window` global.

  *Closes when:*
  - `alo-sites-cta`'s script runs past line 32, in `tests/alo_sites_cta.rs`,
    and what it stops at next is opened as an item;
  - `window === self`, and both are `globalThis`;
  - a listener added to the window is kept through collections and called
    by a dispatch at it, and by a dispatch at a node, after the document
    when bubbling and before it when capturing;
  - a `load` event stops at the document;
  - every script runs ordinarily and under `Heap::stress`.
- **363. The `Window`'s immutable prototype.** *Depends on 362; observable
  only through item 73.*
- **364. The page lifecycle at its window.** *Depends on 362; needs design*
  (§ 5).

## How we will know if this was wrong

**If a frozen page's listener on the window runs in another order, or another
number of times, than another engine runs it**, § 3's path is wrong. It is
fixed in `dispatch.rs`, the one place both drivers read.

**If a page is found to depend on the difference between a `WindowProxy` and
its `Window`** without frames or `window.open`, § 4's reasoning missed a way
for a script to hold a window from another realm. The proxy is built then,
and this says where.

**If named access on the window is what stands between a person and a site
they name in stage 2's exit gate**, law 1's refusal has met a real page. It
goes to stage 3 as a page-scheduled item, as `ROADMAP.md` schedules
everything legacy.
