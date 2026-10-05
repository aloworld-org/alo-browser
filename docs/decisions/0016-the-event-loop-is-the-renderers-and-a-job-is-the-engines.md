# ADR 0016 — The event loop is the renderer's, and a job is the engine's

**Status:** accepted
**Date:** 2026-10-05
**Context:** ADR 0005 (one process per site), whose *the browser process sends
work; a renderer returns results* and *a renderer never makes a synchronous
call back* are what a task is made of here; ADR 0012 § 4, which defines the
agent's window as *the task, which the event loop defines (queue item 76)* and
leaves *its precise edge* to this decision; ADR 0013 § 4 (the interpreter is
interruptible, by the embedder), § 5 (`alo-js` reaches nothing — *not a clock*)
and § 7 (*one heap per event loop*), and its *What this does not decide*, which
names **the task boundary** as item 76's; ADR 0014 § 2 (where a live reference
may be), § 7 (`WeakRef` keep-alive for *the rest of the job*, finalisers as
*tasks on the event loop*) and its *What this does not decide*, which names
**where a safepoint falls relative to a task or a microtask** as item 76's;
`docs/autonomy/QUEUE.md` item 76, whose closing condition is a table of
interleaved tasks and microtasks in the order the specification gives

## The decision in one line

The event loop — task queues, the choice of which task runs next, the rendering
steps and `requestAnimationFrame` — **lives in the renderer, outside `alo-js`**;
the engine owns exactly one piece of it, **the job queue**, because a job is
made inside the engine and holds references only the collector may hold; a
**task** is one message from the browser process or one thing the renderer
itself scheduled, never a slice of time; and every task is followed by a
**microtask checkpoint** that ends the job, with the rendering steps run only
when the browser process has said it is time for a frame.

## Why this is a decision rather than a chore

Three accepted decisions already lean on a word nobody has defined. ADR 0012
draws the line around what an agent caused at *the task*. ADR 0014 promises a
dereferenced `WeakRef` stays alive *for the rest of the job* and that a
finaliser runs *as a task*. ADR 0013 gives the engine one heap *per event loop*.
Each of those is a promise written against a boundary that does not exist, and
each would be kept or broken by whoever wrote the loop first, in a commit that
was mostly code — which is the failure `LOOP.md`'s stage 2 § 4 exists to
prevent.

And the shape is expensive to change afterwards, for a reason that has nothing
to do with speed. Where the microtask queue lives decides how a promise
reaction is kept alive, which decides what the collector walks; what a task is
decides what the agent's record says; and whether the renderer owns a clock
decides whether the loop can be tested with nothing moving. Promises (item
75), events (81), timers (92), workers (91), `fetch` (83) and every `async`
function on every page are written against this, and each would otherwise make
the decision again, slightly differently, in its own file.

The specification does not settle it either. HTML defines the processing model
— what a task is *for*, the microtask checkpoint, the rendering steps — and then
leaves to the user agent the three questions that matter here: which task queue
runs next, when a rendering opportunity arises, and how the host enqueues a job
the language asks for. Those are ours to answer, and answering them once is the
point.

## 1. Two queues, two owners

**The job queue is the engine's.** A job — a promise reaction, a
`queueMicrotask` callback — is a function and its arguments, and every one of
them is a reference into the heap. ADR 0014 § 2 names the places a live
reference may be, and *a Rust value somebody outside the engine is holding* is
not one of them unless it is a `Root`. A queue of jobs held by the renderer
would be a queue of roots, taken and released once per promise reaction on
every page, with a leak for every path that forgets one. A queue held in the
heap is an ordinary structure the collector walks, and a job nobody can run any
more is garbage like anything else.

The language agrees about who makes them. `HostEnqueuePromiseJob` is called
from inside the engine, in the middle of resolving a promise; were the queue
the renderer's, that would be the engine calling out to its embedder in the
middle of a run, and an embedder re-entered half way through an instruction is
the shape ADR 0005 refuses at the process boundary and this refuses inside the
process too. So the engine queues the job itself, and the embedder supplies
`queueMicrotask` by asking the engine to queue one — one entry point, the same
queue, no host hook that runs code.

**Everything else about the loop is the renderer's.** Task queues, the choice
of the next task, the rendering steps and `requestAnimationFrame` know about
messages from the browser process, about documents and about frames — and ADR
0013 § 5 and § 6 say the engine knows about none of those. So the loop is a
module of `alo-renderer`, and `alo-js` gains no notion of a task at all. The
engine is asked to do two things and only two: **run this call**, and **drain
the job queue**.

**A task that holds script holds it by a `Root`.** A timer's callback, a
listener's function, a finaliser's held value: the renderer's task queue is
outside the heap, so what a task will call is rooted when it is queued and
released when the task has run or been dropped. That is a root per *task*, not
per job, and tasks are what arrive from outside — a message, a timer firing, a
response — so their number is bounded by the things that cause them rather
than by a page's promise chains.

## 2. A task is a message, and nothing is a slice of time

ADR 0005 already says how work reaches a renderer: **the browser process sends
work, a renderer returns results**, as typed messages, and a renderer never
calls back and waits. So the boundary of a task is the boundary of a message.

- **Each `ToRenderer` message is handled as one task** — `Load`, `Resize`,
  `Act`, `ReadTree`. Nothing a page's script does can split one, and nothing
  can join two.
- **A task the renderer schedules itself is one task.** A timer firing (item
  92), a finaliser's cleanup (ADR 0014 § 7), and — because a renderer cannot
  fetch (ADR 0005) — **every response**: a script calling `fetch` posts a
  message and returns; the answer arrives later, as its own message, and so as
  its own task. A network round trip is two tasks, always, and never a wait in
  the middle of one.

**The next task is the oldest one.** HTML lets a user agent choose among task
queues however it likes, which is the freedom browsers use to put input ahead
of timers. We take the one choice that is deterministic and that a test can
state: one sequence number across every queue, and the lowest runs next. A
priority policy is a fine idea with a measurement behind it — *somebody's
typing waited behind a page's timers* — and changing the order of due tasks is
then a policy change in the loop rather than a new decision, as long as it
stays deterministic for a given arrival order. Without the measurement it is a
guess about what stutters, and law 3 says a guess about speed is the wrong
thing to settle first.

## 3. The microtask checkpoint ends the job

After every task, and after every call into script that leaves the engine with
nothing running — HTML's *clean up after running script* — the loop performs a
**microtask checkpoint**: the engine runs jobs, oldest first, until the queue
is empty, *including jobs queued by the jobs it is running*. Then it ends the
job: `Heap::end_job`, which is the specification's `ClearKeptObjects` and the
point at which ADR 0014 § 7's keep-alive set is let go.

Three consequences, each of which a page can observe and so each of which is
a rule rather than an implementation detail:

- **The checkpoint runs after each callback, not once after the task.** Two
  listeners on a button that a person clicked see each other's microtasks run
  between them; the same two dispatched by `element.click()` from a script do
  not, because the script is still running. That difference is in the
  specification, real libraries depend on it, and a loop that checkpointed
  only at the end of a task would get the first case wrong.
- **A checkpoint never nests.** A job that calls into script leaves the engine
  with nothing running when it returns, and the checkpoint it would trigger is
  the one already in progress. The specification guards this with a flag; so
  do we.
- **A job that queues a job for ever never yields.** That is the language,
  not a defect: a microtask loop starves rendering exactly as `while (true)`
  does. It is answered the way `while (true)` is — every job is a call, a call
  checks the embedder's `Stop` (ADR 0013 § 4), and the queue lives in the heap,
  so its length is bounded by the heap's ceiling rather than by a number of
  its own.

## 4. Where a safepoint falls

ADR 0014 left this here, and the answer is narrower than the question sounds.

**A collection may still begin at any allocation**, as it does today. Nothing
in this decision makes a task boundary the only place memory is reclaimed:
a page that allocates for a whole task would otherwise grow without bound
inside one, and ADR 0014 § 2 already makes every allocation a point where
everything live is somewhere the collector can see.

**What the loop adds is the quiet point.** Between the end of one task's
checkpoint and the start of the next, the script stack is empty, no native
code holds a scope, and the job has ended — so the keep-alive set is empty
too. That is the one moment where *everything that is live is a root or
reachable from one* is trivially true, and so it is the only place **the loop
itself** asks for a collection, when it has a reason to. And it is where the
results of `WeakRef` and `FinalizationRegistry` clearing are acted on: cleanup
callbacks are **queued as tasks** there, never run inside a checkpoint and never
during a collection — which is ADR 0014 § 7's rule, given its place.

## 5. A frame is the browser process's to call

**The renderer has no clock that decides when to draw.** A rendering
opportunity is a message from the browser process, which owns the display
(ADR 0005) and so is the only thing that knows when a frame can be shown —
today's `Paint` is that message, and it will carry the frame's time. When one
arrives, after the current task and its checkpoint, the loop runs the
rendering steps in the specification's order: the `requestAnimationFrame`
callbacks registered before the frame began, in registration order, each
handed the browser process's frame time and **each followed by a checkpoint**
(§ 3), then style, layout and paint. A callback registered during the frame
waits for the next one.

This makes `requestAnimationFrame` deterministic in a test — the frame time is
a number the test sent — and it means a confined renderer never needs a timer
of its own to know when to paint. What it does **not** make is a claim about
frame rate: the rate is the browser process's and the display's, and `LOOP.md`
is plain that any claim about speed is measured on hardware or not made.

## 6. The agent's task, to the edge ADR 0012 left

ADR 0012 § 4: *while a verb is being applied, that tab's requests are the
agent's*, and *the boundary is the task*. Here is the edge.

**An `Act` is one task.** The verb is applied, the page's handlers run, and the
checkpoint after the task runs every job they queued. **The renderer answers
the `Act` only after that checkpoint**, so the browser process's window — from
sending the verb to receiving its answer — contains exactly that task and the
jobs it caused. A request the browser process receives inside the window is
the agent's; one it receives after is not.

What falls outside is named rather than discovered: a timer the handler set,
even for zero milliseconds; a `requestAnimationFrame` callback; and a
continuation that waits on a response, which arrives as its own task (§ 2). Each
of those is a *new task*, and ADR 0012 already chose a narrow record that is
true over a wide one that attributes a page's whole life to whoever last
touched it.

**A renderer that holds the answer open** widens its own window. That buys a
compromised renderer time rather than a new ability — it could already make
requests inside the window — and the time is bounded by the same judgement
that decides a tab has stopped answering, which is the browser process's
`Stop`. And the window's two ends — when the verb was sent and when it was
answered — belong in what is kept for the action, beside the outcome queue item
203 already owes it, so a long window is visible to whoever reads the record
rather than inferred from the times of the requests inside it.

## 7. A stopped task is a stopped page

When the embedder's `Stop` ends a task or a job, the loop does not resume the
page from where it stopped. The checkpoint that was running is abandoned, the
job queue is emptied — its references become garbage — and the task queue's
roots are released. ADR 0005 already says what a dead renderer looks like (*the
tab keeps the last frame it painted and says what happened*), and a page
stopped half way through a task is a page in a state no script on it expected;
running its next task would be inventing a continuation. Reloading is the
person's to ask for.

## 8. A worker has a loop too, without the frame

ADR 0013 § 7 gives a worker its own engine, heap and loop on its own thread.
That loop is this loop with § 5 removed: tasks, checkpoints and the quiet point
are the same code, and a worker has no rendering opportunity because it has
nothing to render. How a worker's thread is started and how messages cross to
it are item 91's; that its loop is this one is decided here, so the two do not
diverge into two loops with two orders.

## What this costs

**A `setTimeout(0)` in a click handler is not the agent's.** A page that defers
its real work by one task — a common pattern — will show the agent clicking and
then, separately, the page fetching. That is ADR 0012's chosen trade, made
precise here rather than reopened: a wider window would be a record of
everything after the click, which is a record of nothing.

**Oldest-first is not what other browsers do under load.** They favour input,
and on a page flooding its own timers we will feel slower to type into. The
cure is a measurement and a policy change (§ 2), and it is deliberately not
pre-empted.

**Frame timing crosses a process.** The renderer waits for a message to paint,
which is a round trip a renderer with its own vsync timer would not pay. It is
the same direction every other piece of work already flows in, and whether it
costs a frame is a measurement on hardware.

## Alternatives rejected

**The event loop inside `alo-js`.** The loop needs messages, frames and time,
and ADR 0013 § 5 gives the engine none of them — not a clock, not an I/O crate.
An engine that owned the loop would either acquire those or hold an interface
to them as wide as the renderer, and the property that makes the engine
testable with nothing moving would go with it.

**The job queue in the renderer, behind a host hook.** This is how the
specification is *written* — `HostEnqueuePromiseJob` — and how some engines are
built. Rejected for § 1's two reasons: every job would be a root held outside
the heap, and the engine would call into its embedder in the middle of a run.
The hook still exists in our reading of the specification; it is just
implemented by the engine queueing to itself.

**A renderer that decides its own frames.** A vsync timer in the renderer is
how a single-process browser works. Here it would be a clock inside the
process ADR 0010 confines, deciding something the browser process — which owns
the display — knows better, and it would make every test of
`requestAnimationFrame` depend on how fast the machine was.

**Checkpointing only at the end of a task.** Simpler, and observably wrong
(§ 3), on exactly the pages — framework event handlers — where the order is
relied on.

**Priority queues for input now.** § 2: the right answer with a measurement,
and a guess without one.

## What this does not decide

- **Promises, `async` functions and generators** — item 75. This decides where
  their jobs wait, not what a promise is.
- **Timers** — item 92: their clock, its coarsening (a high-resolution clock is
  a side channel), and nesting clamps. This decides only that a timer firing is
  a task in the same order as everything else.
- **Event dispatch** — item 81: capture, bubble and default actions. This
  decides only that a dispatch from the browser process is a task and that a
  checkpoint follows every listener the dispatch calls with nothing else
  running.
- **How a worker is started and spoken to** — item 91.
- **The numbers.** Any ceiling the loop needs — on queued tasks, on callbacks
  per frame — lands in the code with its reason, ADR 0014 § 9's rule.

## How we will know if this was wrong

**If a frozen real page depends on an order this loop does not produce**, the
specification table that closes item 76 was missing a row. The answer is the
row and the fix, with the page as the evidence.

**If the person using this for a week finds input lagging behind a busy page**,
§ 2's oldest-first is the thing to change — with that measurement, as a policy
in the loop.

**And if a promise reaction ever outlives the heap that made it, or a job runs
after its page was stopped**, § 1 or § 7 was not followed, and the fix is in
the code rather than in this decision.
