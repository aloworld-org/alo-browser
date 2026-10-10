# ADR 0041 — Requests are made off the conductor, and a page waits for its style only so long

**Status:** accepted
**Date:** 2026-10-10
**Context:** queue item 351, *The window waits for a load's style sheets*,
cut from 348 by iteration 226 and marked **needs design**. It is opened by
a page: `alo-sites-cta`, the call-to-action section alo Sites publishes,
links its whole style sheet. In the window it is painted **unstyled first**
and then again with its sheet. `alo-window`'s `a_page_styled_in_the_window.rs`
asserts that unstyled first frame today.

ADR 0035 § 5 says the browser process presents no first frame of a document
until every sheet asked for in its load's answer is answered, **within a
bound of its own**. A bound is needed because `alo-net`'s `PATIENCE` bounds
one read, not an exchange: a server that sends a byte every twenty-nine
seconds stays inside it for ever. After the bound the page is shown with
what has arrived, and the late sheet is applied when it comes.

Building 348 found why that cannot be built as things stand. The queue item
says so: *either the exchange gets a deadline of its own in `alo-net`, or
requests are made off the conductor's thread*. Which one is a decision about
the network stack or the conductor (ADR 0024 § 2), and this ADR makes it.

Also read: ADR 0005 (the browser process owns the network; a renderer never
waits); ADR 0012 §§ 4–6 (every request is in the session's record, which
outlives the window); ADR 0014 § 9 (a bound lands in the code with its
reason); ADR 0016 § 2 (every response is a task); ADR 0024 § 2 (the window
never waits on a renderer, and shows what it was last sent); ADR 0032 § 1
(an answer for a document that has gone is answered by nobody); ADR 0039
§§ 1–2 and ADR 0040 § 3 (a page told it is hidden and left; a keep-alive
fetch made after its page has gone, and one still waiting at close recorded
as not made).

The code this is about:
- `alo-window`'s `conductor.rs`. Between looking at its orders the
  conductor calls `Answering::answer_next`, which **makes one request on the
  conductor's own thread and waits for it**. It looks at its orders again
  only when that exchange ends. Every paint, resize, visibility change and
  close waits behind it.
- `alo-renderer`'s `fetch_answering.rs` (`Answering`: the queue, oldest
  first, sheets ahead of fetches), `fetch_make.rs` (`Network`: the pool, the
  jar and the preflights, one of each per session), `sheet_make.rs`, and
  `Refusal::record`, which writes into the pool's record.
- `alo-net`'s `connection.rs` (`PATIENCE`, a socket's read and write
  timeouts) and `pool.rs`. No exchange has a deadline of its own.
- `alo-window`'s `window.rs` and `bin/alo.rs`. Closing sends
  `CloseEverything`, and the event loop ends on `News::Closed`. The process
  then joins the conductor, which hands back the session's `Network`.

## The decision in one line

Every request the conductor makes for a page is made **on a thread of its
own** — one network thread, the only holder of the session's `Network`,
making one exchange at a time in the queue's order — so the conductor
**never waits on the network**. The conductor holds back a load's first
frame until every sheet asked for in that load's answer is answered, **or
until a bound of the window's own passes**, whichever is first. A page shown
at the bound is said to be shown before its style, and a sheet that arrives
after it is applied when it comes.

## Why this is a decision rather than a chore

The bound in ADR 0035 § 5 is a promise about what a person sees, and the
thread that keeps it must be awake when the bound passes. Today that thread
is inside an exchange it cannot leave. Two ways out are open, and they put
the bound in different places.

**A deadline on the exchange** puts the window's patience into the network
stack. The exchange is ended when the page has waited long enough, so the
late sheet is not late: it has failed. **A thread of its own** leaves the
exchange alone and lets the conductor stop waiting for it. The first changes
what a request is; the second changes who waits for one. Each costs
something the other does not, and neither is a refactor.

## 1. One network thread holds the session's network

The conductor starts **one network thread** beside itself. It is given the
session's `Network` (the pool, with its cache and record; the jar; the
preflights) and is **its only holder** from then on. Nothing else touches
the pool, so nothing is locked, and the record has one writer, as it does
today.

It is sent **jobs** over one channel, in order:
- **make** a decided fetch, a decided sheet, or a keep-alive fetch whose
  page has gone (ADR 0040 § 3);
- **record** a refusal (`Refusal::record`) — a refused fetch or sheet, an
  ask refused as its page was left (ADR 0039 § 4);
- **close**: make nothing more, and record every keep-alive fetch still
  waiting as not made, because the browser closed (ADR 0040 § 3).

It does each in turn, and sends each made job's result back to the
conductor. That result is what `fetch_make::make` and `sheet_make::make`
return today: what the renderer is sent and what the person is told.

**One exchange at a time, in the queue's order.** `Answering` keeps its
queue and its rules on the conductor: oldest first, a load's sheets ahead of
its fetches, nothing made for a document that has gone except a keep-alive
fetch, and what a delivery asks for joining the end. The conductor hands the
network thread **the next job only when the last one has come back**. So
the order a page sees, and every rule ADR 0032 § 1 and ADR 0040 § 3 wrote
about it, is unchanged. Only the thread that waits for an exchange is
different. Several exchanges at once would be a claim about speed, and no
such claim is made here (`LOOP.md`).

A refused ask is answered in its turn in the queue, as today, with no
exchange to wait for. Its record line is a *record* job sent at that moment,
so the record keeps the order things happened in.

## 2. The conductor never waits on the network

The conductor has **one inbox**, holding the window's orders and the
network thread's results. Each time it looks, it takes everything waiting
there. When nothing is waiting, it waits on the inbox until something
arrives or the nearest bound in § 3 passes, never longer. A result is
delivered to its tab as `answer_next` delivers one now: a task through
`Tabs::fetched` or `Tabs::styled`, then a paint if that tab is selected, then
the reason said if it failed. The next job is then handed out.

So a resize, a visibility change, an exchange with another tab's renderer,
and `CloseEverything` are carried out **while a request is in flight**,
however slowly its server answers. ADR 0024 § 2 kept the event loop from
waiting on a renderer; this keeps the conductor, which waits on renderers by
design, from also waiting on servers.

**A network thread that has gone** — its channel closed, which the lints
make unreachable except by a panic — is said to the person, not assumed.
Every fetch and sheet still owed is answered as a failure, with that said,
and the conductor goes on serving the window.

## 3. A load's first frame waits for its sheets, within a bound

When a tab's `Load` is answered, the sheets that answer asked for are that
load's **owed sheets**, and the tab is **held**. While a tab is held, the
conductor does everything it does now — loads, lays out again at a new
size, supplies fonts and redraws, tells the page it is shown or hidden,
delivers results — but **paints nothing of it**. The window goes on showing
what it was last sent: the last frame, or its own background for a tab that
has had none (ADR 0024 § 2).

The hold ends at the **first** of:
- **every owed sheet answered**, delivered or refused or failed. The page
  is painted once, styled. A refusal or failure counts as an answer: the
  page waited for the sheet, and the sheet is not coming;
- **the bound passing**. The bound runs from the moment the conductor
  received the load's answer. The page is painted with what has arrived and
  is said to the person as shown before its style arrived (§ 4);
- **the tab ceasing to show that document**: closed, or loaded with another
  page. The hold is dropped with it. A new load starts a new hold.

A load whose answer asked for no sheet is not held, and is painted at once,
as today. **A sheet asked for after the load** — in the answer to a
delivery, an `Act` or a task that added a `<link>` — is never owed and holds
back no frame. That is HTML's rule, and ADR 0035 § 5's.

**The bound is the window's**, in `alo-window`, with its reason beside it
(ADR 0014 § 9). It is a few seconds, chosen in the build and well inside
`PATIENCE`, since its whole point is a server that stays inside `PATIENCE`.
A wait as long as `PATIENCE` would show a person nothing for half a minute
before admitting anything was wrong. It is not a network bound and the
network stack never learns it: no exchange is ended because the bound
passed.

## 4. What passing the bound says, and what comes after

The page is painted, and the person is told in the window's own words that
**this page is shown before its style arrived**. Those are the same words
whatever is late, as ADR 0035 § 3 says of a failed sheet. The sentence stays
until the page is next painted, as a fetch's failure does.

The late sheet is **still made and still applied**. Its exchange carries on
under `PATIENCE`, and when its answer comes it is delivered and the page is
painted again, as any sheet after a load is now. A sheet that never
finishes is answered by nothing. The page stays as it was shown, and the
fetches behind it in the queue wait behind it, as they do today (*What this
costs*).

## 5. Closing

`CloseEverything` is carried out at once (§ 2). Every tab is closed and each
page is left (ADR 0039 § 2), and the network thread is sent **close**. The
conductor tells the window `Closed` without waiting for the network thread,
so the window ends as promptly as its renderers allow.

The session's `Network` and its record are handed back by **joining the
network thread** when the conductor finishes. That waits for the exchange in
flight, if there is one, to end. Today that wait happens before the window is
told `Closed`, and it is no longer than it is today: one exchange, bounded
per read by `PATIENCE` and not as a whole. Bounding a whole exchange is
not decided here (*What this does not decide*), and it does not hold up
anything a person can see.

## 6. What closes queue item 351

In `alo-window`, against a server on this machine, in real `Tabs` over the
confined renderer:
- a page whose sheet is answered after a delay is **not painted before
  the answer** and is painted, styled, after it. This changes
  `a_page_styled_in_the_window.rs`' first assertion from unstyled to
  styled;
- a page whose sheet's server never finishes is painted **after the bound
  and not before**, unstyled, with the sentence said. While that sheet's
  exchange is still in flight, a `Resize` is answered with a frame at the
  new size and `CloseEverything` with `Closed`;
- a sheet a page's script adds after its load holds back no frame;
- a page that links no sheet is painted at once, as today;
- a tab closed while held paints nothing more, and its sheet's answer is
  answered by nobody;
- the session's record still has every request made, refused and not made,
  in the order they happened. The existing fetch, beacon and close tests in
  `alo-window` and `alo-renderer` pass unchanged in what they assert.

The browser and the corpus draw the same thing: ADR 0035 § 6's corpus already
draws after every answer, and nothing here changes it.

## What this costs

**A thread.** The browser process gains one thread that lives as long as the
conductor. It shares nothing with it but two channels.

**A trickling sheet still holds the queue behind it.** One exchange at a
time means that a sheet whose server never finishes delays every fetch and
sheet queued after it, in every tab, until `PATIENCE` ends a silent read.
That is true today, and today it also holds the conductor. After this it
holds only the queue. Making several exchanges at once would end it. That
needs ordering rules ADR 0032 § 1 does not have and would be a speed claim,
so it waits for a page and a measurement.

**A held page is a moment of nothing.** A person sees the last frame or the
window's background for up to the bound. That is what every engine's
render-blocking sheet shows, and the bound is what keeps it short.

**Closing still waits for one exchange** before the process can hand back
its record and end, though no longer before the window is told it has
closed.

## Alternatives rejected

- **A deadline on the exchange, in `alo-net`, equal to the bound.** The
  conductor would still be asleep inside an exchange until it ended. A
  resize or another tab's answer would wait out the deadline, and so would
  the bound, which could only be checked between exchanges. A load with
  three slow sheets would wait three times as long, unless the deadline
  were the load's remaining patience passed into the network stack: a
  window's policy in a crate that has no window. And a sheet cut off at the
  bound has *failed*, not arrived late, so ADR 0035 § 5's *the late sheet is
  applied when it comes* would be false.
- **A deadline on every exchange, of the network's own.** It may be wanted,
  but it does not solve this. A number long enough for a legitimate slow
  answer under `LARGEST_BODY` is far too long for a person looking at a
  blank window. One short enough for them fails real requests.
- **The pool behind a lock, shared by the conductor and a network thread.**
  The conductor would block on the lock for as long as an exchange held it,
  which is today's problem with an extra step. And the record would have two
  writers.
- **A thread per request, or several network threads.** Concurrency the
  queue's order does not describe, a pool and record with many writers, and
  a claim about speed that has not been measured.
- **A rented asynchronous runtime.** A dependency the size of a scheduler,
  to wait on two channels. The process model is ours to build, not to rent
  (`CLAUDE.md`, *rent the physics, build the engine*).
- **The renderer waits for its sheets.** ADR 0035 already rejected it. A
  renderer never waits (ADR 0005), and the browser process already decides
  when a frame is asked for.
- **No bound: wait for every sheet.** A server that trickles would keep a
  page blank for as long as it liked. That is the hostage `PATIENCE` exists
  to refuse, moved from the socket to the window.

## What this does not decide

- **A bound on a whole exchange**, so that a server sending one byte every
  twenty-nine seconds cannot hold the queue and the browser's last moments
  for ever. It is a decision about every request the network stack makes,
  downloads included, and it is queue item 376.
- **Making several exchanges at once**, as above.
- **What an agent reads while a page is held.** An agent reads the tree
  through the renderer, not the frame. Whether it waits for the load's
  sheets too is the agent surface's decision when a window carries one
  (items 131, 134).
- **A held page's paint for a tab that is not selected.** Nothing paints
  such a tab today. When the tab strip chooses among tabs (item 297), a tab
  chosen while held keeps its hold, and the same rules apply.
- **`<script>` waiting for a pending sheet**, which ADR 0035 also left to
  the first item whose script reads style or layout.
- **The bound's value**, which lands in the code with its reason.

## How we will know if this was wrong

**If a window stops answering a resize or a close while a request is in
flight**, § 2 was not followed: something on the conductor waits on the
network again.

**If the record's order ever differs from the order things were made and
refused**, the record jobs were not sent in their turn (§ 1).

**If people see pages shown before their style on ordinary connections**,
the bound is too short. If they see blank windows for longer than a slow
sheet takes, it is too long. Either way it is measured, not guessed again.

**If a page is ever found depending on two of its requests being in flight
at once**, one exchange at a time is too strict, and that page opens the
concurrency this ADR leaves undecided.
