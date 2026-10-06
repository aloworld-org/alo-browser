# ADR 0018 — One dispatch, two drivers, and an agent's verb is a keyboard's click

**Status:** accepted
**Date:** 2026-10-06
**Context:** queue item 81, *events: capture and bubble, listeners, default
actions*, whose closing condition is *`alo-renderer`'s test that a nav row
changes nothing fails, and is rewritten to assert what it now does*; ADR 0017,
whose *What this does not decide* hands **dispatch, capture and bubble, and
default actions** to item 81 and decides only that **a listener is held by its
node's wrapper** (§ 3); ADR 0016 § 3 (*a checkpoint after every callback*) and
§ 6 (*an `Act` is one task*), whose *What this does not decide* says the same;
ADR 0013 § 3 (*absent beats approximate*), § 6 (the engine knows nothing about
the DOM) and its *What this does not decide* (*a verb runs a page's own
handlers through the ordinary event path*); ADR 0002 (the layout tree is the
agent's tree, and no verb takes a coordinate); ADR 0014 § 9 (every number has a
reason); `CLAUDE.md`'s law 1 (no legacy DOM surface); and the code this is
about — `alo-js`'s `object/native.rs` (a builtin calls script only by
returning `Answer::Want` and being run again at a numbered step),
`interpret/checkpoint.rs` (a job's throw is *reported* through a closure the
embedder passes), `alo-bindings`' `wrapper.rs` and `liveness.rs`,
`alo-renderer`'s `renderer.rs` (`act`), `event_loop.rs` and
`event_loop/task.rs` (`Work::Calls`, a fixed list of callees), and
`alo-agent`'s `apply.rs` (`toggle`).

## The decision in one line

The DOM standard's **dispatch algorithm is written once**, in `alo-bindings`,
as a **stepper** whose state lives in the event object, and it is **driven two
ways** — by a native function suspending once per listener when a script
dispatches, and by the renderer's event loop, with a microtask checkpoint
after every listener, when the browser does; a listener lives in its target's
wrapper; **an agent's verb is the browser's own input**, trusted, and
`Activate` fires exactly the `click` that **keyboard activation** fires — no
pointer, no coordinate; **activation behaviour lives in `alo-dom`**, so a
script's `el.click()` and an agent's `Activate` toggle a checkbox through the
same rule and a cancelled click undoes it; and the surface is the modern one,
with each legacy member absent.

## Why this is a decision rather than a chore

Item 81 reads as an algorithm the DOM standard spells out step by step, and the
steps are not the hard part. Five questions are, and each fixes something a
page will observe.

**Who runs the algorithm.** A script's `dispatchEvent` runs listeners
synchronously, inside the native that was called; a person's click runs them
as a task, with the jobs each listener queued run before the next listener
(ADR 0016 § 3). Two copies of *which listener runs next, and whether
`stopPropagation` ended it* is how the two come to disagree, and one copy has
to fit both a native that may not nest a Rust call inside a Rust call
(`object::native`) and a loop that checkpoints between calls. The loop's
`Work::Calls` — a list of callees fixed when the task is queued — cannot be it:
whether the third listener runs depends on what the second one did.

**What an agent's verb is to a page.** `Activate` is either the browser's input
— trusted, indistinguishable from a person's — or something a page can tell
apart and treat differently. That choice decides whether an agent can drive
pages at all, and whether a page can single it out.

**What `Activate` fires.** A mouse click is ten events with coordinates; ADR
0002 forbids a verb that takes one, and making one up is a page reading a
position nobody pressed.

**Where a default action lives.** `alo-agent`'s `apply` toggles a checkbox
today. With events, the toggle has to happen *before* the listeners (they read
the new state) and be undone if one cancels, and a script's `el.click()` must
do exactly the same — so it cannot stay the agent's alone.

**What happens to ARIA state the agent used to change.** `apply` flips
`aria-checked` itself because a page without script had nobody else to. A page
with script flips it in its own click listener, and an agent that flipped it
too would flip it back.

## 1. A listener lives in its target's wrapper

`EventTarget` is an interface in `alo-bindings` (`interface/event_target.rs`),
and `Node.prototype` inherits from `EventTarget.prototype`. An event target's
**listener list is a field of its wrapper** — each entry the callback, its
`capture`, `once` and `passive`, and a *removed* flag — traced as strong edges.
That is ADR 0017 § 3's sentence made concrete: a listener is held by the
wrapper, the wrapper by its tree, and a detached tree that nothing in script
reaches goes at the next collection, listener, closure and all. A node that was
never wrapped has no listeners, since `addEventListener` is only reachable
through its wrapper — so a dispatch asks the document cell's table, and never
makes a wrapper to find out that a node has nobody listening.

The list's entries count in the wrapper's footprint (ADR 0014 § 9), so a page
that adds listeners for ever meets the heap's ceiling like an array does.

`addEventListener`'s options are Web IDL's dictionary, converted as it says:
`capture`, `once`, `passive`, and `signal` — whose type is `AbortSignal`, an
interface that does not exist yet, so **any `signal` that is not `undefined`
fails the conversion with the `TypeError` the standard's own conversion
throws**. That is not a refusal invented here; it is what Web IDL does with a
value that is not the member's type, and it is correct until `AbortSignal` is
built. A listener may be a function or an object with a `handleEvent` method
(the `EventListener` callback interface), looked up when the listener is
called, as the standard says.

**The global object is not an event target yet.** It becomes one with item 251
(`Window`, which waits on item 73); until then `addEventListener` on the global
is absent, not a function that registers listeners nothing ever dispatches to.
The path of a dispatch ends at the document, and gains the global when it is a
`Window`.

## 2. The dispatch algorithm is one stepper, and the event holds its state

The DOM standard's *dispatch* is written once, in `alo-bindings`
(`dispatch.rs`): the **path** is computed when dispatch starts — the target and
its inclusive ancestors to the document — and does not change if a listener
moves the target; the **capture** phase runs root to target, **at target**
both, the **bubble** phase target to root when the event bubbles; each
target's listener list is **copied when its turn comes**, so a listener added
during dispatch to the target being dispatched at does not run, and one
*removed* during dispatch does not run either (the *removed* flag); `once`
removes a listener before it is called; `passive` makes `preventDefault` do
nothing; `stopPropagation` ends dispatch after the current target's
listeners, `stopImmediatePropagation` after the current listener; and when it
ends, `eventPhase` is `NONE`, `currentTarget` is `null`, and the answer is
whether the event was cancelled.

It is a **stepper**: given the event, it answers *call this listener, with
this `this` and this event* or *done*, and is told when the call has finished.
**Its state — the path, where in it it is, which flags are set — lives in the
event object**, an embedder cell, which is exactly where the standard keeps it
(`eventPhase`, `currentTarget` and `composedPath()` are read from it by the
listeners). That is what lets the native driver below keep nothing across a
suspension but a step number, as `object::native` requires.

**A dispatch flag** on the event refuses a second dispatch of an event already
being dispatched with `InvalidStateError`, as the standard does; nesting
dispatches of *different* events is allowed and is bounded by the engine's
call depth, since each nested listener call is a frame on the interpreter's
stack (`object::native`: a builtin never nests a Rust call).

There is no retargeting: there are no shadow trees (item 87), and when there
are, retargeting is added to this stepper rather than written beside it.

## 3. Two drivers

**From script** — `dispatchEvent(event)` and, below, `el.click()` — the driver
is the native itself. It starts the stepper and returns
`Answer::Want` with the listener to call, at a step it is run again at; each
time it is run again, it tells the stepper the call finished and asks for the
next. No checkpoint runs between listeners, because script is still running
(ADR 0016 § 3) — the difference between a person's click and a script's
`element.click()` that real libraries depend on.

A listener that throws must not throw out of `dispatchEvent`: the standard
**reports** it and carries on with the next listener. Today a builtin's
`Want::Call` propagates a throw to the builtin's caller, so **the engine gains
a second kind of call a builtin may ask for: one whose throw is reported rather
than propagated** — set aside exactly as a job's throw is
(`interpret/checkpoint.rs`), handed to the embedder's report with the outer
call's result, and answered to the builtin as `undefined`. It is generic — a
promise's reaction or a `FinalizationRegistry` cleanup asks for the same thing
— so the engine still knows nothing of events (ADR 0013 § 6).

**From the browser** — an agent's verb now, a person's input when there is a
window — the driver is the renderer's event loop. The dispatch is **one task**
(ADR 0016 § 6), a new kind of `Work` beside `Script` and `Calls`, holding the
event by its task's root; running it steps the same stepper, calls each
listener through the engine, and performs a **microtask checkpoint after every
listener** (ADR 0016 § 3). A throw is reported like any other piece of script,
and the dispatch carries on. A stop — a full heap, the embedder's `Stop` —
stops the page as it does for any task.

**A page that has never run script is dispatched to by nobody.** Its document
is still the renderer's (ADR 0017 § 2), it has no heap and no wrapper, and so
no listener; the renderer does not build a heap to find that out. Stage 1's
pages behave as they did, and their reference renders say so.

## 4. An agent's verb is the browser's input, and the page cannot single it out

A verb arrives from the browser process (ADR 0005), and an event the browser
dispatches is **trusted**: `isTrusted` is `true`, as for a person's. There is
**no marker** on the event, the page or anything else a script reads that says
an agent caused it.

That is what ADR 0013 already said a verb is — *a page's own handlers, through
the ordinary event path* — and the alternative fails the thing this repository
exists for: a page that can tell an agent's click from a person's can refuse
the agent, and an agent that drives only the pages that let it is a plugin
again (law 2). *Whether* an agent may act on a page is a different question,
and it is a permission — queue items 93 and 133, and ADR 0012's record of what
caused what, which the browser keeps and the page never sees. Nothing here
weakens either: this decides what the page sees, not who may act.

## 5. `Activate` is the click keyboard activation fires

A person who tabs to a button and presses Enter gets **one `click`**: no
`pointerdown`, no `mousedown`, no position — because nothing pointed. That is
the honest model of an agent's `Activate`, which also points at nothing (ADR
0002), and every page that works for a keyboard user works for it. So
`Activate` fires exactly that event and no other:

- interface `PointerEvent` (UI Events and Pointer Events: `click` is a
  `PointerEvent`), so on its chain `MouseEvent`, `UIEvent`, `Event`;
- `type` `"click"`, bubbling, cancelable, composed, trusted;
- `pointerId` `-1` and `pointerType` `""` — the values the standard gives a
  click that no pointing device caused;
- every coordinate `0`, `button` `0`, `buttons` `0`, `detail` `0`, every
  modifier `false` — what a keyboard's click carries, and not a made-up
  position a page would read as a place somebody pressed.

**No key events are fired**: the agent did not press a key, and a `keydown`
for Enter is a claim about a keyboard that did not happen. **Focus** before the
click is item 258's — what a keyboard user's click had first — and is added
there, not invented here.

The interfaces are built with the members a click needs and the rest absent
(ADR 0013 § 3): `clientX` is present and `0`; `getModifierState` is present
or absent, not present and wrong.

**`PutText`** fires what replacing a field's text fires: `beforeinput`
(cancelable; cancelled, nothing changes), then the change, then `input`, each
an `InputEvent` with `inputType` `"insertReplacementText"` and the text as its
`data`, then `change`. **`Activate` on a link** — whose outcome today is
`Outcome::Followed` — is the same click, and the link is followed only if
nothing cancelled it. **`Scroll`** fires nothing yet; a
`scroll` event is decided with the first page that listens for one.

## 6. Activation behaviour lives in `alo-dom`, and a cancelled click undoes it

The HTML standard gives some elements an **activation behaviour**, and a
checkbox or radio a **legacy-pre-activation behaviour**: the box is checked
*before* the click's listeners run, so they read the new state, and put back
if one of them cancels. A link's is following it; a submit button's is
submitting its form (item 82); a `type="button"`'s is nothing.

These are rules about the document, and there are **two callers** — an agent's
`Activate` and a script's `el.click()` — so, for ADR 0017 § 5's reason, they
live once, in `alo-dom` (`activation.rs`): *before*, which makes the
pre-activation change and returns what undoes it; *cancelled*, which undoes
it; and *after*, which says what follows a click nobody cancelled — the
`input` and `change` a toggled box fires, a link to follow. The sequence is
the standard's *activation* steps around one dispatch, and it is run by
whoever dispatched: the renderer for a verb, the `click()` native for a
script. `alo-agent`'s `apply` keeps **deciding** what a verb is aimed at and
stops changing a checkbox itself.

Where a box's checked state is held — today its `checked` attribute; the
standard's *checkedness*, separate from the attribute, is item 82's — is the
part item 82 changes. The sequence does not change with it.

`el.click()` is the standard's: on a disabled form control, nothing; while that
element's click is already in progress, nothing; otherwise a click that is
**not trusted**, `pointerId` `-1`, with the same activation behaviour. It lives
on `HTMLElement.prototype`, so the item that builds it gives an element in the
HTML namespace the `HTMLElement` interface between `Element` and its own.

## 7. ARIA state is the author's on a page that runs script

`aria-checked` is a promise the page's author makes about a widget they built,
and maintaining it is the author's code. On a page whose document is in a heap
(ADR 0017 § 2), **`Activate` dispatches the click and changes no ARIA state**:
the page's listener does that, and an agent that did it as well would undo it.

On a page that has **never run script** there is nobody else to keep the
promise, and stage 1's agent verbs, which flip `aria-checked` on alo's own
scriptless screens, are what those screens were tested against. That stays as
it is, **stated as a stage 1 accommodation**: it is a convenience for markup we
write, it is the one place this engine does something a browser would not, and
it is retired, not extended, if a page we did not write is ever found to
depend on the difference.

## 8. The surface, and what is absent

Built with the first cut: `EventTarget` (`addEventListener`,
`removeEventListener`, `dispatchEvent`); `Event` (its constructor and
dictionary, `type`, `target`, `currentTarget`, `eventPhase` and its four
constants, `bubbles`, `cancelable`, `defaultPrevented`, `composed`,
`isTrusted`, `stopPropagation`, `stopImmediatePropagation`, `preventDefault`,
`composedPath`); `CustomEvent` and its `detail`; and, with `Activate`,
`UIEvent`, `MouseEvent` and `PointerEvent` with the members § 5 names, and
`HTMLElement.prototype.click`.

**Absent by law 1**: `returnValue`, `cancelBubble`, `srcElement`, `initEvent`,
`document.createEvent` and `window.event` — the legacy forms of things the
modern surface already does.

**Absent until their item** (ADR 0013 § 3): **`timeStamp`**, which is a clock,
and a clock a page can read is a side channel whose resolution is item 92's
decision; **event handler attributes and properties** (`onclick="…"`,
`el.onclick = f`), not legacy but their own item (259) — compiled, when
built, under the page's policy as `csp::Inline::Script` with
`csp::Content::attribute`, which item 191 already shaped for exactly this;
and the global object as a target (§ 1).

## What this costs

**An engine change for a `dispatchEvent`.** A builtin gains a call whose throw
is reported, not propagated (§ 3). Small, generic, and the price of keeping the
dispatch algorithm in one place rather than one copy for natives and another
for the loop.

**The event object is heavier than a record of its fields**, because it
carries the dispatch's path while it is dispatched. That path is what
`composedPath()` returns anyway.

**Two behaviours for `aria-checked`** (§ 7), stated rather than hidden, until
stage 1's accommodation is retired.

**`Activate` is not a mouse.** A page that listens only for `mousedown` and
never for `click` does nothing when an agent presses its button — exactly what
it does for a keyboard user. That is the page being inaccessible, and the
agent being honest about what it did; inventing a pointer to work around it
would be a verb with a coordinate in all but name.

## Alternatives rejected

- **Dispatch as `Work::Calls`, the listeners listed when the task is queued** —
  § 3: whether a listener runs depends on what the one before it did.
- **Two dispatch implementations, one for natives and one for the loop** —
  § 3, the drift ADR 0017 § 5 refused for tree operations.
- **The dispatch's state in a Rust structure beside the event** — § 2: a native
  may keep nothing across a suspension the collector cannot see, and the
  standard keeps it in the event.
- **Listeners in a table in the document cell, keyed by node** — § 1: a second
  structure with its own liveness, when ADR 0017 already decided the wrapper
  holds them and the wrapper already lives exactly as long as it should.
- **An agent's events untrusted, or marked** — § 4: a page that can single out
  an agent can refuse it, and who may act is a permission, not a flag a page
  reads.
- **`Activate` as the full mouse sequence at the box's centre** — § 5: a
  coordinate nobody chose, and ten events claiming a pointer that did not
  exist.
- **Activation behaviour in `alo-agent`** — § 6: `el.click()` is a second
  caller.
- **`addEventListener` on the global object now, before it is a `Window`** —
  § 1: a function that registers listeners nothing will ever dispatch to is
  the approximate member ADR 0013 § 3 refuses.

## What this does not decide

- **Focus and keyboard events** — item 258: what has focus, what a key does, and
  whether an agent's `PutText` is ever keystrokes.
- **Input from a person** — a window, a pointer and real coordinates arrive
  with the compositor; they go through § 3's browser driver unchanged.
- **Event handler attributes and properties** — item 259 (§ 8).
- **`scroll`, `load`, `DOMContentLoaded`, `resize`** and every other event the
  browser fires on its own schedule — each with the page that needs it, most
  after the global is a `Window` (item 251).
- **Mutation observers**, still ADR 0017's *does not decide*.
- **Who may act on a page** — items 93 and 133.

## How we will know if this was wrong

**If a frozen page's listeners run in an order, or a number of times, that
another engine does not produce**, the stepper is wrong, and it is fixed there
— in the one place both drivers read.

**If a real page works for a keyboard user and not for an agent**, § 5's model
of `Activate` has a hole: something keyboard activation does that this does
not, most likely focus (item 258).

**If a real page works for a person and does nothing for an agent, and it is
not inaccessible to a keyboard**, § 5 was the wrong model altogether, and the
replacement says which event the page needed and why it is not a coordinate.
