# Remaining roadmap and execution loop

Audit date: 2026-09-07. `ROADMAP.md` owns scope and stage gates;
`QUEUE.md` owns implementation order and closing conditions. These counts are
an inventory snapshot, not a completion percentage or an effort estimate.

| Stage | Roadmap state | Queue inventory | What closes it |
|---|---|---|---|
| 1: renders alo | 11 done; exit gate met | 29 done, 0 open | Sign-in and Settings reference images and box trees, plus named agent activation |
| 2: modern web | 9 done, 73 open or partly built | 73 done, 84 open | A person uses it for a week; an agent completes a real external-site task with an audit record |
| 3: legacy tail | 8 open | 8 open | No fixed completion gate; actual failing pages schedule individual work |
| 4: adoption | 8 open | 9 open | Somebody outside alo chooses it on their own machine and stays |

Stage 1 also has three separate follow-ups outside its met gate: GPU paint,
OS compositor embedding, and measured hardware performance. They are not in
the stage 1 queue count. Stage 4 has more queue items than roadmap lines because
extensions include a separate design decision. A partly built line remains open.
There are **101 open queue items** overall; splitting broad items will change
that number without changing the amount of work owed.

## Continuation order

Iteration 119 halted at item 222's reachable named-expression binding scope:
the required frozen real-script failure has not been established. Supply that
case with provenance and a failing execution assertion before implementing this
scope. Existing lexer/parser corpus tests do not close it. No item or stage was
completed, and this halt does not mean the queue or roadmap is finished.
Iteration 120 marked item 222 blocked in the queue's own words.

Iteration 122 wrote item 207's decision, ADR 0015 (`BigInt` limbs rented from
`num-bigint`, sizes and spellings ours). Its code half is blocked the same way
222 is: no frozen real script uses a `BigInt`. The next unused ADR is 0016;
the next unused queue number remains 223.

Iteration 123 built item 212 at its first step, `[[Construct]]` and `new`,
opened by a frozen real script: alo's service worker, which now compiles past
its `new Request(…)` and stops at an array literal (item 211). Classes,
`super`, `new.target` and private names are cut to item 223 and `instanceof` to
item 224, so 102 queue items are open. The next unused queue number is 225; the
next unused ADR remains 0016. The service worker's next refusal names item 211,
whose dependencies (73, 75) are not built — a real-script trigger for 211 is
not the same as its dependencies being met.

Iteration 124 took the piece of that dependency the trigger needed: item 225,
cut from 73 and 211, built the array exotic object, `Array.prototype` and the
array literal without a spread. The service worker now compiles to the `try`
at byte 2853, which is item 210 — whose dependency on 73 is for the `Error`
objects a `catch` binds. An object assigned to an array's `length` is cut to
item 226. 225 was added already closed and 226 was added open, so 103 queue
items are open. The next
unused queue number is 227; the next unused ADR remains 0016.

Iteration 125 took the piece of item 73 that item 210 waits on: item 227, the
seven error constructors, their prototypes, `Error.prototype.toString` and the
first builtin `[[Construct]]`. Item 210's dependency now names 227 rather than
73, so `try`/`catch`/`finally` — the service worker's refusal at byte 2853 — is
eligible next. `Error.prototype.toString` of a message behind a call is cut to
item 228 (on 221) and `AggregateError` to item 229, so 105 queue items are
open. The next unused queue number is 230; the next unused ADR remains 0016.

Iteration 130 built item 210, `try`/`catch`/`finally`, opened and closed
against the same frozen script: the service worker now compiles past its `try`
at 2853 to byte 2922, `for (const account of …)`, which is item 211. Item 211
therefore has a real-script trigger, but its dependencies on 73 (beyond what
225 took) and 75 are not met — the next iteration decides whether a cut of 211
is takeable, as 225 was cut from it. 104 queue items are open; the next unused
queue number remains 230 and the next unused ADR 0016.

Iteration 131 took that cut: item 230, `for…of` with a name or a property as
its head and the iteration protocol it reads through, cut from 211, 75 and 73
(`Symbol.iterator`, `Symbol.toStringTag`, `%IteratorPrototype%` and the array
iterator). **The frozen service worker now compiles whole**; running it stops
at `self`, a worker's global and an embedder's to supply (item 91), so the
next real-script trigger is an embedder rather than the engine. An array
iterator reading through a getter is cut to item 231. Item 211's dependency on
75 is met by 230 for everything it has left (spread, patterns, `for…in`), but
no frozen script reaches any of them. 105 queue items are open (230 was added
closed and 231 open); the next unused queue number is 232 and the next unused
ADR remains 0016.

Iteration 132 wrote item 76's decision, ADR 0016 (the event loop is the
renderer's; the job queue is the engine's). No running frozen script reaches a
builtin of 73 or a regular expression (74) — the service worker stops at
`self`, and every handler it registers runs only as a task — and 75 depends on
76, so 76 was the first item on that script's path whose dependencies are met.
It could not be built before its decision, which ADRs 0012, 0013 and 0014 had
each left to it. No code was written and 76 is not done; its first code cut is
the job queue, the checkpoint and the task order. 105 queue items remain open;
the next unused queue number remains 232 and the next unused ADR is 0017.

Iteration 133 built the engine's half of item 76 as item 232: the job queue in
`alo-js`'s heap, `Engine::queue_job` and `Want::Job`, `Engine::checkpoint`
(oldest first, jobs queued by jobs included, a throw reported, the queue
dropped on any other escape, the job ended) and `Engine::call`. The renderer's
loop is item 233 and the rendering steps with `requestAnimationFrame` are 234;
76 closes when both have. 107 queue items are open (232 added closed, 233 and
234 open); the next unused queue number is 235 and the next unused ADR 0017.

Iteration 134 cut item 233 again and built the loop itself as item 235:
`alo-renderer`'s `event_loop` — one task queue run oldest first, a task's
script held by one root, a checkpoint after every script and every listener
call, `queueMicrotask` on the global object, throws reported, a stopped page's
tasks, roots and jobs dropped, the quiet point checked. 233 keeps the
`Renderer` holding it, which needs two things settled first: a way for a
renderer to run its own due tasks outside a message's answer, and the page's
`Content-Security-Policy` carried in before its own `<script>` elements run.
107 queue items are open (235 added closed); the next unused queue number is
236 and the next unused ADR 0017.

Iteration 135 settled the second of those and built it as item 236: the
`Renderer` holds one `EventLoop` per page, `Page` carries the response's
enforced `Content-Security-Policy` headers across the boundary, and a page's
own inline classic scripts run at load in document order, each a task with
its checkpoint, each asked first of the headers and of every `<meta>` policy
in `<head>` before it (`alo-dom`'s `scripts.rs` reads both, with CSP's
nonceable rule). 233 keeps the loop running between messages and `Act`
answered after its checkpoint, which no test can close until a listener (81)
or a timer (92) runs script in an `Act`'s task. Cut alongside: 237
(report-only inline violations reported), 238 (fetched scripts), 239 (a thrown
error object said by name and message). 110 queue items are open; the next
unused queue number is 240 and the next unused ADR 0017.

Iteration 136 built item 237, the first item whose dependencies were met
after 233 (which waits on 81 or 92): an inline script a header policy refused,
or a report-only one would have, is reported. The renderer names the
objecting policy by its place in `Page::stated`; the browser process writes
the report from its own copy of the headers (`alo-renderer`'s
`violations.rs`) and posts it with `Pool::report`, believing no place that
could not have objected and no more than 64 per load. A `<meta>` policy's
`report-to` is cut to item 240. 110 queue items are open (237 closed, 240
added); the next unused queue number is 241 and the next unused ADR 0017.

Iteration 137 built item 239, the first eligible item after 240 and 238 (both
waiting on a frozen page): an uncaught error object is reported by its `name`
and `message`, read from data properties along its chain without running any
of the page's script, and every string a report repeats is cut at 1024 code
units. 109 queue items are open; the next unused queue number is 241 and the
next unused ADR 0017.

Iteration 138 found 234 waiting on 233, marked item 77 `needs design` (no
closing condition, and its loader is the fetch-across-the-boundary decision
238 also waits on), and cut item 241 from 78: an uncaught throw is reported
with the script, line and column — UTF-16 code units — of the throw and of
each call it left, at most 32 with the rest counted. Item 242 (a ceiling on
how many reports one load carries) was found and added. 110 queue items are
open (241 added closed, 242 added open); the next unused queue number is 243
and the next unused ADR 0017.

Iteration 139 built item 242, the first eligible item (233 and 234 wait on 81
or 92 and on 233, 77 needs design, 78's remainder has no closing condition):
a load says at most 256 lines about its scripts and then how many more, and
a turn of the event loop keeps at most 256 reports and counts the rest
without describing them, so a job throwing for ever costs a counter. Item 243
(the same ceiling for the markup half of a load's issues) was found by
reading and added. 110 queue items are open (242 closed, 243 added); the next
unused queue number is 244 and the next unused ADR 0017.

Iteration 140 built item 243 (a ceiling on what a page's markup makes one
load say, and every line at most 8192 characters) and cut item 244.
Iteration 141 built item 244, the first eligible item for the same reasons
as 242 and 243: a family longer than any font's can be (512 characters,
`alo_text::LONGEST_NAME`) is not asked for and that is said, and the browser
process answers one a renderer sends anyway absent without looking. 109 queue
items are open; the next unused queue number is 245 and the next unused ADR
0017.

Iteration 142 wrote item 80's decision, ADR 0017 (the document moves into
the page's heap; a wrapper lives as long as its tree is reachable; a native
reaches its node only through its `this`; every change goes through
`alo-dom` under the standard's rules; a changed document is rendered again
whole when read; a parser-inserted script sees the document up to its own
element). No code. Item 80 was the first eligible item after 244 (79 waits
on 73), and ADR 0014 had left its bindings' shape to it. Its code is cut as
245 (`alo-dom`'s operations, no dependency — next), 246 (the bindings and
re-render, item 80's closing condition) and 247 (a script at its own end
tag). 112 queue items are open; the next unused queue number is 248 and the
next unused ADR 0018.

Iteration 143 built item 245, the first eligible item (no dependency):
`alo-dom`'s insert, append, replace and remove under the DOM standard's
names and validity rules, refusing by the standard's exception names with
the tree unchanged; `createElement`'s name rule; a change count; and
releasing a detached tree into tombstones whose ids are never reused. No
script reaches it yet. 111 queue items are open; 246 (the bindings and the
re-render) is next; the next unused queue number is 248 and the next unused
ADR 0018.

Iteration 144 cut item 246 into 248 (the document in the heap and wrapper
liveness), 249 (the interfaces a script calls) and 250 (the renderer handing
its document over and rendering again), and built 248: the engine's typed
borrow, `alo-dom`'s footprint kept as a sum and an allocation-free release,
and the `alo-bindings` crate with the document cell, the wrapper, and the
ring-and-sweep that keeps a wrapper as long as its tree. No script reaches
it yet. 113 queue items are open; 249 is next; the next unused queue number
is 251 and the next unused ADR 0018.

Iteration 145 built 249: the interfaces a script calls, in `alo-bindings` —
`Node`, `Element`, `Document`, the `ChildNode` mixin and `DOMException`,
item 80's members only, in the standard's prototype chain, behind Web IDL's
brand check, with `document` on the global object — and the attribute
operations by name, *replace data* and *string replace all* in `alo-dom`.
Proven by scripts the engine runs against an adopted document; no page's
script reaches it until 250. It added 251 (`document` as Web IDL's
accessor). 113 queue items are open; 250 is next; the next unused queue
number is 252 and the next unused ADR 0018.

1. **Item 205's plain-header scope is finished.** Iteration 117 rechecks
   function headers under the body's strictness and preserves the remaining
   early errors and import attributes as item 222. Item 60 is `needs design`;
   item 187 still awaits an upload caller, and item 169 needs Linux execution.
   Follow actual queue dependencies; item 207 requires an ADR-only iteration
   before implementation. The next unused queue number is 223.
2. **Item 219 is finished.** Native calls can suspend and resume; the
   recursion limit counts waiting builtins as well as script frames. Its
   previously unbounded test now passes, including collection stress. Item 221
   retains `apply` and the traced scratch storage it needs, so the total number
   of open items remains 101 despite closing 219. Continue with the queue's
   first eligible item; the JavaScript branch can now take item 220.
3. **Continue JavaScript's dependency chain.** Item 220 covers function
   metadata/source and binding; item 73 must be split into bounded builtin
   families with explicit closing conditions. Constructors (212), exceptions
   (210), parameter forms (213), proxies (217), tagged templates (215) and
   destructuring (211) depend on pieces of this work. Resolve dependencies per
   item rather than treating this paragraph as a new queue order.
4. **Complete process/network prerequisites as they become eligible.** Linux
   sandbox probes (169) require execution on Linux; this macOS checkout cannot
   certify them. Remaining font axes (197), request causality, HTTP behaviour,
   encrypted DNS settings and storage-access policy retain their queue gates.
5. **Connect script to live pages.** The event loop, DOM wrappers/mutation,
   events, forms, navigation, frames, storage and workers unlock useful web
   interactions. Split large capabilities when starting, preserve their
   remainders, and freeze the smallest allowed real-page case that demonstrates
   each missing behaviour.
6. **Finish the modern browser surface.** CSS/text/input, media, incremental
   rendering, window/tabs, accessibility, developer tools and the agent's
   permission/audit model all remain stage 2 work. Use the queue's actual
   prerequisites. Hardware and usability claims need the specified evidence.
7. **Exercise stage 2's real exit gate.** Record the week's browser use,
   fallback sites and external agent task. Queue exhaustion cannot stand in
   for that evidence. Then eligible stage 3 page failures and stage 4 product
   work can proceed under the existing stage rules. Stage 3 is not a finite
   promise that a loop can mark globally complete.

## The repeated operation

Read the current queue and journal → choose the first eligible bounded item →
read its ADR and feature contract → implement and exercise error paths → run
its tests and the full gate → update features, changelog, roadmap and journal →
commit locally → supervisor verifies the gate and recorded progress → repeat.

`scripts/loop.sh` already implemented this workflow; it is extended rather than
replaced with a competing supervisor. `LOOP.md` contains the worker contract.
Do not loosen a gate, manufacture a real-page trigger, or erase unfinished work
to keep the loop running. A blocked stage is reported with what would unblock
it. `LOOP COMPLETE` means no eligible work remains, not all stages are finished.

## Run and resume

Prerequisites: macOS, Rust toolchain, authenticated `codex` CLI, a clean
checkout, and a passing `scripts/gate.sh`. No supervisor was started by this audit or by the follow-up cleanup. The
item 219 recursion blocker is resolved. The follow-up validation and commit
are recorded in `STATE.md`; start from a clean checkout after the full gate.

```sh
scripts/loop.sh --dry-run
scripts/loop.sh --self-test
scripts/test-loop.sh
scripts/gate.sh
scripts/loop.sh                # up to 500 iterations, or a stop condition
```

`--items N` bounds a run, and `--once` verifies one iteration. Keep the session
alive; the script is not a scheduled service. Progress goes to
`docs/autonomy/loop.log`, commits and `STATE.md`. After interruption, inspect
preserved work, finish or repair it, verify and commit, then rerun. Retire a
resolved halt with a new journal iteration rather than deleting history.
Publishing changes is separate from this local build loop.
