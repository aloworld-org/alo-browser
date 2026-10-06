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

Iteration 146 built 250: the renderer hands a page's document to its script
(`held.rs`) and draws from what the script left — again whole when the
change count has moved, at a paint, a tree read or an act, once at the end
of a load, and from the document it has at a resize — and the corpus loads
a case that carries script through a renderer (`a-script-grows-a-list`).
It ticked 246, whose condition 248–250 met together, and added 252 (a
thrown `DOMException` reported by its name). Item 80 waits only on 247.
112 queue items are open; 247 is next; the next unused queue number is 253
and the next unused ADR 0018.

Iteration 147 built 247: the page is parsed a step at a time (`alo-dom`'s
`Parsing`), each step lent the document — in the page's heap once a script
has run — and each script runs at its own end tag against the document
parsed so far; no detached tree is released while the parse lasts, since
the parser holds open elements no wrapper does; `<meta>` policies are taken
as the parser made them. Corpus case `a-script-beside-itself`. It ticked
item 80 — 245, 246 and 247 all done — and added 253 (`document.body`).
111 queue items are open; 252 is next (it depends on nothing), and 81, 85,
87, 88 and 89 are unblocked by 80; the next unused queue number is 254 and
the next unused ADR 0018.

Iteration 148 built 252: a `DOMException` a page lets escape is reported by
the name and message it was made with, read from the `alo-bindings` cell by
type and running nothing (`described.rs`); every other thrown value is said
as before. 110 queue items are open; 253 is next (`document.body`, which
depends only on 249; 251 waits on item 73); the next unused queue number is
254 and the next unused ADR 0018.

Iteration 149 built 253: `document.body`, read as HTML's *the body element*
and assigned by replacing it or appending to the document element, a
non-body refused as `HierarchyRequestError` (`alo-dom`'s `body.rs`, one
accessor in `alo-bindings`). Corpus case `a-script-gives-a-new-body`.
109 queue items are open; 81 (events) is next, unblocked by 80, and 251
still waits on item 73; the next unused queue number is 254 and the next
unused ADR 0018.

Iteration 150 wrote item 81's decision, ADR 0018 (one dispatch algorithm in
`alo-bindings` with its state in the event, driven by a native for a
script's `dispatchEvent` and by the event loop for the browser, with a
checkpoint after every listener; listeners in their target's wrapper; an
agent's verb trusted and unmarked, `Activate` the one `click` keyboard
activation fires; activation behaviour in `alo-dom`; no ARIA state changed
by the agent on a scripted page). No code. Its code is cut as 254, 255 and
256, which close 81, and 257, 258 (needs design) and 259 after them. 115
queue items are open; 254 is next (it depends on nothing); the next unused
queue number is 260 and the next unused ADR 0019.

Iteration 151 built item 254, events from script: `alo-js` gained a call a
builtin asks to have reported (`Want::Report`, its throw stopped at the call,
set aside rooted and handed over with the run's result) and an embedder's
constructor given its instance only by `new`; `alo-bindings` gained
`EventTarget`, `Event`, `CustomEvent`, listener lists in wrappers and the
dispatch stepper; the renderer reports a listener's throw placed in its
script. Corpus case `a-script-hears-an-event`. `isTrusted` was cut as 260,
which 256 now also depends on. 115 queue items are open; 255 is next (its
dependency 254 is done); the next unused queue number is 261 and the next
unused ADR 0019.

Iteration 152 built item 255, the browser's dispatch as a task: a
`Work::Dispatch` in `alo-renderer`'s event loop holds a browser-made event
and its target by one root and steps `alo-bindings`' one dispatch algorithm,
with a microtask checkpoint after every listener and before the stepper
hears it returned; a page that never ran script is given no heap. Nothing
the browser does fires one yet. 114 queue items are open; 256 waits on 260,
so **260** is next (it depends on nothing); the next unused queue number is
261 and the next unused ADR 0019.

Iteration 153 wrote item 260's decision, ADR 0019: `alo-js`'s realm gains
ECMAScript's `[[HostDefined]]`, set once by the embedder and handed to a
native beside the intrinsics (amending ADR 0017 § 4 for natives whose `this`
is not a wrapper); `alo-bindings` sets it to the document cell, which holds
each interface's unforgeables object, copied onto every instance. No code.
114 queue items are open; **260** is next, now buildable; the next unused
queue number is 261 and the next unused ADR 0020.

Iteration 154 built item 260 against ADR 0019: the realm's
`[[HostDefined]]` (`Engine::host_defined`, `Call::host_defined`), set by
`install` to the document cell, which now holds `Event`'s unforgeables
object; `isTrusted` is copied onto every event by `alo-bindings`'
`unforgeable.rs` from both constructors and `event::create` — one getter
per realm, own, not configurable. 113 queue items are open; **256** is next
(its dependencies 255 and 260 are done); the next unused queue number is
261 and the next unused ADR 0020.

Iteration 155 built item 256, closing item 81: an agent's `Activate` on a
page that runs script is a trusted `PointerEvent` `click` dispatched as one
task, with `alo-dom`'s `activation.rs` around it, `input` and `change`
after a box nobody cancelled, and `Acted` carrying what the script said;
`alo-settings` carries its nav script. `HTMLElement` and `click()` were cut
to the new item 261. 112 queue items are open; **261** is next (it depends
only on 256), then 257; the next unused queue number is 262 and the next
unused ADR 0020.

Iteration 156 built item 261: `HTMLElement` between `Element` and every
HTML element, and `click()` — an untrusted click driven from the native
with `alo-dom`'s `activation.rs` around it. Each element's own interface
is cut to item 262 and a script's click following a link to item 263
(alo's own `FilesView.tsx` and `TaskDetail.tsx` call `a.click()` on a
download link). 113 queue items are open; **257** is next (it depends
only on 256); the next unused queue number is 264 and the next unused ADR
0020.

Iteration 157 built item 257: an agent's `PutText` on a page that runs
script is one task — a trusted `beforeinput` `InputEvent`
(`insertReplacementText`, the text as `data`) a page may cancel, which
answers the new `Outcome::TextCanceled`; otherwise the text, put by
`alo-dom`'s `field.rs`, then `input` and `change`. 112 queue items are open.
Of the events group, 258 needs design, 259 and 262 wait on a page, and 263
needs its ADR first (a decision, its own iteration). The next unused queue
number is 264 and the next unused ADR 0020.

Iteration 158 wrote item 263's decision, ADR 0020: a renderer's navigation
is a claim in the answer to the message whose work made it, held meanwhile
in the document cell (one, the last); the renderer resolves the URL against
the document's base (`Page` gains its URL) and the browser process parses
it again, refuses by name the schemes and targets a page may not send its
tab to, and assigns the cause from which message it answered. No code.
`<a download>` is cut to item 264 (on 263, 120 and `Blob`). 113 queue items
are open; **263** is next, now buildable; the next unused queue number is
265 and the next unused ADR 0021.

Iteration 159 built item 263: a followed link — a script's `click()` or
the agent's own — is the page's ongoing navigation, held in the document
cell (`alo-bindings`' `navigating.rs`), resolved against the document's
base (`Page::url`), asked for in `Loaded` and `Acted`, and decided by the
browser process (`alo-renderer`'s `navigate.rs`) with the cause from which
message it answered (`Tabs::navigation`). Item 265 (an origin-only
`Referer` lacks its `/`, found by 263) is new. 113 queue items are open;
the next unused queue number is 266 and the next unused ADR 0021.

Iteration 160 built item 265: an origin-only `Referer` is the origin
written as a URL, `https://example.com/`, under `origin`, `strict-origin`
and the cross-origin half of the `*-when-cross-origin` policies, with a
port and an IPv6 host kept in the origin's form. 112 queue items are
open; the next unused queue number is 266 and the next unused ADR 0021.

Iteration 161 built item 190: `inset`, `outset`, `groove` and `ridge` are
drawn in two tones of the border's colour (`alo-paint`'s `border.rs`), each
side the mitred wedge of the box nearest it, clipped to the border's ring;
corpus case `border-styles`. `dashed`, `dotted` and `double` are cut to 266
and a fieldset's legend-broken groove to 267. Items 82, 83 and 85–89 have
their dependencies but no ADR, contract or closing condition, and are now
marked `needs design` in the queue. 113 queue items are open; the next
unused queue number is 268 and the next unused ADR 0021.

Iteration 162 built item 266: `dashed`, `dotted` and `double` are drawn in
the mitred wedges (`alo-paint`'s `pattern.rs` for the spacing, `border.rs`
for the layers), with the joints every cut on a mitre makes shared by both
wedges there so a dashed corner has no seam; `border.rs` is split into
`tone.rs`, `mitre.rs` and `border.rs`. Corpus case `border-patterns`. The
same styles beside a legend are cut to 268. 113 queue items are open; the
next unused queue number is 269 and the next unused ADR 0021.

Iteration 163 built item 267: a fieldset's border is the `groove` other
browsers give it. `alo-paint`'s new `banded.rs` draws the border a legend
breaks — five rectangles when solid, otherwise the mitred sides inside a
clip with the legend's part of the block-start line cut out, clamped to the
side borders so the corners stay. Corpus cases `fieldset-group` and
`web-a-form` moved. 112 queue items are open; the next unused queue number
is 269 and the next unused ADR 0021.

Iteration 164 built item 268: a `dashed`, `dotted` or `double` fieldset
border is drawn round its legend, laid along the whole side exactly as
without a legend and cut by the clip 267 built, so a dash or dot the
legend's edge falls on is cut there. Corpus case `fieldset-patterns`. 111
queue items are open; the next unused queue number is 269 and the next
unused ADR 0021.

Iteration 165 built item 178: a picture under a rotation, skew or mirror is
drawn turned with its box, the outline anti-aliased and each pixel sampled
back through the inverted transform; an upright picture keeps the exact
whole-pixel path. Corpus case `a-turned-picture`. Items 95–98, 104 and 105
are marked `needs design`. 110 queue items are open; the next unused queue
number is 269 and the next unused ADR 0021.

Iteration 166 built item 180 for GIF and WebP. Both are rented as pure Rust,
each behind one file, and every format answers to one size bound,
`agreed_size`. An animated one is drawn as its first frame. Corpus case
`a-picture-in-each-format`. Mutation testing found a panic in `image-webp`
0.2.4 (a WebP animation frame whose picture is larger than the frame), and it
is now refused in front of the decoder. AVIF is cut to item 269, which needs an
ADR. 110 queue items are open; the next unused queue number is 270 and the next
unused ADR 0021.

Iteration 167 wrote ADR 0021 for item 269: AVIF is to be read by `avif-parse`
and `rav1d`, through `rav1d`'s safe Rust API, with the colour conversion ours.
That API is merged but unreleased, and `rav1d` 1.1.0 offers only a C interface,
so 269 is **blocked on a `rav1d` release** rather than built on a git pin or
on FFI of our own. 110 queue items are open; the next unused queue number is
270 and the next unused ADR 0022.

Iteration 168 wrote ADR 0022 and cut item 107, SVG, as 107 asked before it was
started. An `<svg>` is a replaced box whose contents become a drawing of paths
made by a new crate, `alo-svg`. 270–273 (the box, filled shapes, path data,
strokes) close on alo's offline screen. 274–277 wait on a page, and 277 also
needs its own ADR. 107 stays open until 270–273 close. 118 queue items are
open; the next unused queue number is 278 and the next unused ADR 0023.

Iteration 169 built 270, the `<svg>` box: one replaced box sized from CSS,
its attributes, its `viewBox` or 300 × 150, no boxes inside it, and one agent
image. 271 (filled shapes, and the crate `alo-svg`) is next. 117 queue items
are open; the next unused queue number is 278 and the next unused ADR 0023.

Iteration 170 built 271: the crate `alo-svg`, a drawing handed to paint by
box, SVG presentation attributes in the cascade, and the basic shapes filled
under `transform`, `viewBox` and `preserveAspectRatio`. It cut a nested `<svg>`
(278) and the `transform` property with relative `width`/`height` attributes
(279) into items of their own. 272 (path data) is next. 118 queue items are
open; the next unused queue number is 280 and the next unused ADR 0023.

Iteration 171 built 272: `<path>` and its data, every command, with arcs
turned into cubic curves, drawn up to its first error and bounded at 65 536
segments per path. 273 (strokes, closing on alo's offline screen) is next.
117 queue items are open; the next unused queue number is 280 and the next
unused ADR 0023.

Iteration 172 built 273: strokes, every `stroke-*` property, outlined by the
rented stroker in user space and transformed with their shape, dashes bounded
before they are cut. It froze alo's offline screen as `alo-offline` with its
hand drawn, which closes 107. The page showed three layout faults, queued as
280–282 (an atomic inline under `text-align`, its margins in its line, and
the `place-*` shorthands), and 280 is next. 118 queue items are open; the
next unused queue number is 283 and the next unused ADR 0023.

Iteration 173 built 280: a line in an anonymous block takes `text-align`
from the nearest element above it, so alo's offline screen centres its hand
and its button. 281 (an atomic inline's margins in its line) is next. 117
queue items are open; the next unused queue number is 283 and the next unused
ADR 0023.

Iteration 174 built 281: an atomic inline's margin box sits on its line, so
alo's offline screen keeps the 20 px under its hand. The strut it also
wanted is cut to 283; a percentage-width inline-block drawn against its slot
rather than its container is 284, blocked until a page needs it. 282 (the
`place-*` shorthands) is next. 118 queue items are open; the next unused
queue number is 285 and the next unused ADR 0023.

Iteration 175 built 282: `place-items`, `place-self` and `place-content`
split into their `align-*` and `justify-*` pair, so alo's offline screen is
centred down the window as well as across. 283 (the strut) is next. 117
queue items are open; the next unused queue number is 285 and the next
unused ADR 0023.

Iteration 176 built 283: every line starts as tall as its container's font,
so alo's offline screen has the font's descent under its hand. That exposed
an atomic box's baseline taken as its bottom edge even for a button, whose
line is now one descent too tall; that is 285, eligible and next. 117 queue
items are open; the next unused queue number is 286 and the next unused ADR
0023.

Iteration 177 built 285: an atomic inline box stands on its last line's
baseline — a button on its label's, a text field on its value's — so the
button lines 283 made too tall are the button's height again, and two of
three corpus cases are back to their pre-strut references byte for byte.
It found an inline-block holding a block broken around it by the box tree;
that is 286, blocked until a page needs it, like 284. 117 queue items are
open; the next unused queue number is 287 and the next unused ADR 0023.

Iteration 178 built the sizing half of 279: `width` and `height` on an
`<svg>` are presentation attributes, so a per cent or an `em` sizes the box
through the cascade and a stylesheet beats it (corpus case
`svg-relative-size`). Its `transform`-property half is cut to 287, eligible.
A per cent on an inline-level `<svg>` is 284's double resolution. 117 queue
items are open; the next unused queue number is 288 and the next unused ADR
0023.

Iteration 179 built 287: the `transform` property on SVG elements. The
attribute is its presentation attribute — its grammar moved to `alo-value`
so the cascade can read it, written there as a `matrix()` — and `alo-svg`
measures the property against `transform-box` and `transform-origin`
(corpus case `svg-transform-property`). `fill-box` on a `<g>` and
`stroke-box` are cut to 288, opened by a page. 117 queue items are open;
the next unused queue number is 289 and the next unused ADR 0023.

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
