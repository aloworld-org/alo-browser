# ADR 0065 — A read can wait for the page to settle, and settled means the same on both renderers

**Status:** accepted
**Date:** 2026-10-26
**Context:** ADR 0002 (the layout tree is the agent's tree; *reading is never
watching*); ADR 0055 (the agent socket; a refusal is a kind); ADR 0058 § 6 (a
claim names its renderer, and must not depend on it in substance); ADR 0060 (no
request of a page in Chrome leaves before this process has seen it); queue items
676, 677, 630; `docs/contracts/agent-socket.md`, *What version 1 does not do*

## The decision in one line

`{"v":1,"ask":"read","tab":ADDRESS,"when":"settled"}` is answered with the tree
**once the page has settled** — nothing of the page's still owed, and the tree
the same twice running half a second apart — or, if that has not happened in
ten seconds, refused as `NotSettled` with what kept it busy. Settled is decided
on **our** tree and **our** record of what is owed, so it means the same thing
whichever renderer drew the page.

## Why this is a decision

Because the protocol cannot tell an agent when a page has finished reacting,
and so every agent guesses. An agent that fills a field, sleeps a guessed number
of milliseconds and reads again may read a tree the page is still rebuilding;
if it does, it has read a lie and nothing in the answer says so. That is the
guessing this browser exists to refuse, moved one level up — from *where to
click* to *when to look*.

And because ADR 0002 says **reading is never watching**, so any answer has to
be argued against it rather than slipped past it.

## 1. Why this is not watching

ADR 0002 gives its reason in its own words: *"An engine that continuously
streamed its tree would have made `alo-os` ADR 0001 § 4 impossible to keep."*
What it forbids is **exposing** the page continuously — a stream, a
subscription, a feed of every change.

A settled read exposes the tree **once, when asked, to a caller the grants
allow** — exactly as a plain read does. What changes is only *when* within a
bounded wait the one answer is given. While it waits the browser observes the
page for itself, as it already does to draw it; nothing of that reaches the
caller. One ask, one answer, no subscription, no callback. The rule stands
unweakened, and the contract's *"version 1 does not watch"* stays true.

## 2. What settled means

Both of these, at once:

1. **Nothing of the page's is owed.** No request the page made is still
   outstanding, and no navigation of the tab's own frame is under way.
2. **The tree has stopped changing.** Two reads of the tree **500 ms apart are
   identical** — every node's role, name, states and text the same.

**Each half is decided from records this process already keeps**, which is
what makes it renderer-independent:

- *Through Chrome*, no request leaves before this process has seen and released
  it (ADR 0060), so what is outstanding is **known exactly**, from our own
  holds — not inferred from Chrome's `networkIdle`, which is a heuristic over
  connections (no connection for 500 ms) and counts things this browser does
  not.
- *On our own engine*, the renderer already reports after every task what it
  still owes — `navigation`, `fetches`, `sheets` in `FromRenderer` — and it runs
  no timers, so a page with nothing owed is a page that will not change on its
  own.
- The tree compared is **our tree** in both cases: the one an agent reads.

Chrome's lifecycle events are **not** the definition. They may be used to wake
up and look sooner; they never decide.

## 3. The numbers

- **Quiet: 500 ms** between two identical trees. The same window Chrome's own
  idle heuristic uses, which is why it is chosen; it is a constant of this ADR,
  not of Chrome's.
- **Ceiling: 10 s.** A read not settled by then is refused rather than answered.
- **Neither is set by the caller in this version.** A caller-chosen wait is a
  second decision — how long may an agent hold a tab — and it is left for when a
  real page needs it.

## 4. A page that never settles is told so, as a kind

A clock, a ticker, an animation that changes the tree, a request held open for
ever: these pages do not settle, and that is a true fact about them, not a
failure of the browser. The answer is a refusal by kind, with its own fields:

    {"answer":"refused","refusal":{"kind":"NotSettled","tab":…,"waited_ms":10000,
      "still":"owed"|"changing"|"both"},"said":"…","v":1}

`still` says which half kept it busy. **No tree is sent with a refusal** — a
tree the browser has just said it does not trust is not an answer, and an agent
that wants the page as it is asks for it plainly.

## 5. What does not change

- A read **without** `when` behaves exactly as version 1 does today. No caller
  changes; the version number does not move.
- A browser that does not know `when` refuses it as `UnknownField`, which is the
  correct thing for an older browser to say to a newer caller.
- The grant is checked **before** anything is waited on (ADR 0055): a tab an
  agent may not read is refused at once, never after ten seconds.
- A settled read is written in the record as a read, with how long it waited.

## Consequences

- `docs/contracts/agent-socket.md` gains `when` on `read` and the `NotSettled`
  refusal, in the change that builds it — not before.
- Each renderer exposes *what is owed*: through Chrome from the holds in
  `chrome_hold`/`chrome_gate`; on our engine from the renderer's last report.
- Settling costs a read of the tree every half second while it waits — at most
  twenty — which on a large page through Chrome is not free. Watching DOM
  mutation instead would be cheaper and is left as an optimisation, on one
  condition: it may only make the answer **sooner**, never different.
- Item 677 (*a verb says what it changed*) is most useful computed against a
  settled tree. That is 677's to decide.

## What this does not decide

- Settling on **part** of a page — wait until *this* region is quiet, ignore the
  clock in the corner. Useful, and a second decision.
- A caller-chosen quiet or ceiling.
- Whether `act` should wait to settle before answering. Today it does not, and
  this ADR does not change it.
