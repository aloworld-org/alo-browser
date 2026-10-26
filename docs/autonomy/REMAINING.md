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
There were **101 open queue items** when this table was taken. **The table's
figures are all as of 2026-09-07 and have not been re-audited since**; counted
on 2026-10-24 the queue holds **298 open items**, and the difference is almost
entirely broad items split into their parts, which changes the number without
changing the work owed. The per-stage breakdown above is therefore the shape of
the work, not its size, until somebody re-audits it.

**Two paths, since ADR 0059.** The stages above describe the engine. About
eight of the open items lie on the product path instead — Chrome rendering
behind our surface, ADR 0058 — and `QUEUE.md`'s own *The product path* section
names them. The loop takes items in `LOOP.md`'s order regardless; that section
is a map, not a queue of its own.

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

Iteration 180 marked item 108 (Canvas 2D) `needs design` — alo's own
picture-shrinking code uses a canvas, but the item names no ADR, contract or
closing condition — and wrote item 109's decision, ADR 0023: media is
decoded in a sandboxed media process per site, audio through Symphonia
(Vorbis, FLAC, PCM, MP3), with Opus, AV1 and VP8/VP9 waiting on decoders
that can be rented in Rust and AAC, H.264, H.265 and DRM never played. No
code. Its code is cut as 289–295: 289–292 are opened by a frozen page that
plays a file, 293–295 are blocked on decoders. 124 queue items are open; the
next unused queue number is 296 and the next unused ADR 0024.

Iteration 181 marked 111 (Web Audio) and 117 (a performance budget) `needs
design`, and 113 (incremental style and layout) blocked on the measurement
ADR 0017 § 6 says it waits for. It wrote item 118's decision, ADR 0024: the
window is rented (`winit`, `softbuffer`, safe interfaces, a new `alo-window`
crate) and composed from the frames the browser process was last sent, never
waiting on a renderer; the tab strip is a document we ship, built from data
and rendered by the engine in a sandboxed renderer of its own, so the agent
reads it; a person's pointer is hit-tested in the renderer, and a strip
click is claimed there and decided by the browser process. No code. Its code
is cut as 296–300, and **296 (the window shows a tab) is eligible now**: its
dependencies are done and its closing conditions are a reference render, a
test and a capture on this machine. 129 queue items are open; the next unused
queue number is 301 and the next unused ADR 0025.

Iteration 182 built item 296, the window. `alo-window` is a crate with
binary `alo`. It holds `winit` and `softbuffer` behind one file each, and a
conductor thread that alone waits on renderers. Composition is a function
with four reference renders. A real renderer stopped with `kill -STOP`
leaves the window showing its last frame and then saying why. 296 stays
open on one clause: the capture of the window with `screencapture -l`. This
machine refuses it because the process running the loop has no Screen
Recording permission, which only a person can grant. 297 depends on 296,
so the window's line waits on that permission. The loop halted to say so.
The next unused queue number is still 301 and the next unused ADR 0025.

Iteration 183 retired that halt by appending. 296 stays blocked on the
permission, and the queue still had decision work to do. Iteration 184 wrote
item 90's decision, ADR 0025: one bucket per origin and top-level site across
all four storage APIs, held only by the browser process; one fixed 1 GiB
quota that is the same on every machine and in private browsing; whole-bucket
eviction under a profile bound that is never reported; `sessionStorage`
never written to a disk; and a corrupt bucket set aside rather than served in
part. No code. Its code is cut as 301–305:
- **301**, the browser process's store, has its dependencies done and is
  eligible next.
- 302 (`localStorage` and `sessionStorage` in a page) is opened by a frozen
  page.
- 303 (`navigator.storage`) waits on 75.
- 304 (IndexedDB) needs its store's ADR.
- 305 (the Cache API) waits on 83 and 91.

134 queue items are open. The next unused queue number is 306 and the next
unused ADR 0026.

Iteration 185 built item 301 as `alo-storage`. It is the browser
process's store: buckets by origin and partition, one ledger against the
bucket quota and the profile bound, whole-bucket eviction, checksummed
records, set-aside, and clearing a site. Measuring the volume's free space
was cut to **306**, because it needs a rented crate. 302 is next for
storage, once a frozen page opens it, and 80 and 236 are done. 134 queue
items are open: 301 closed and 306 opened. The next unused queue number is
307 and the next unused ADR 0026.

Iteration 187 built item 306. The profile's bound is measured from the
store's own volume with `rustix`'s `statvfs`, checked against `df`. A volume
that cannot be asked bounds it at zero. 133 queue items are open. The next
unused queue number is 307 and the next unused ADR 0026.

Iteration 188 wrote item 93's decision, ADR 0026: a permission is a grant
in one table the browser process holds, one capability from a closed list
to one storage key, made by a person answering an ask made with a gesture.
It ends with the page or thirty days after the person's last visit, is
shown in one list and while in use, revoked at once, and recorded without
its content. No agent makes, answers or revokes one. No code. Its code is
cut as **307** (the grant table, `alo-grants`, depending only on 301, so
eligible next) and **308** (the prompt and the indicator, behind 297). 135
queue items are open: 307 and 308 opened, and 93 stays open until they
close. The next unused queue number is 309 and the next unused ADR 0027.

Iteration 189 built item 307 as `alo-grants`: the closed list, the ask
decided by named rules, the three answers and their endings, revocation that
stops uses in the same act, the record of sixty-four per key, and the file
set aside whole when it does not read. Only a person's cause answers or
revokes. 308 waits on 297, which waits on 296's Screen Recording grant. 134
queue items are open. The next unused queue number is 309 and the next
unused ADR 0027.

Iteration 190 decided item 277 in ADR 0027, opened by alo Meet's
`<img src="alo-waving-hand.svg">`. An SVG picture is recognised by its type
and its root, read by `quick-xml` behind `alo-dom`'s `xml.rs` into a document
of its own, refused whole on any XML error, and drawn by `alo-svg` with no
script, no animation and no request. It is cut into 309 (reading the file,
depends on nothing and is eligible next), 310 (`<img>`, after 309) and 311
(`background-image`, opened by a page). 137 queue items are open. The next
unused queue number is 312 and the next unused ADR 0028.

Iteration 191 built item 309: `alo_dom::read_svg` reads Meet's frozen hand
into `svg`, `title`, `g` and two `path`s, and refuses by name everything
ADR 0027 § 3 refuses and everything past its bounds. 310 (`<img
src="…svg">`) depends only on 309 and is eligible next. 136 queue items are
open. The next unused queue number is 312 and the next unused ADR 0028.

Iteration 192 built item 310, closing 277: an `<img>` whose resource is
typed `image/svg+xml` shows the file, drawn by `alo-svg` at the box's size
with its own cascade and nothing to fetch with. Meet's greeting is corpus
case `alo-meet-greeting`. The page opened 312 (`vertical-align`), 313
(layered backgrounds) and 315 (`rem` in media queries), each depending on
nothing and eligible. The tests found 314 (an `<img>` given a width and a
height), blocked on a page. 138 queue items are open. The next unused queue
number is 316 and the next unused ADR 0028.

Iteration 193 built item 312: `vertical-align`, every keyword, a length and
a percentage, on atomic boxes and on inline boxes with what they hold. `top`
and `bottom` are held by the line box's edges. A face with no x-height in
its table is measured by its `x`. Meet's hand is middle-aligned, and the
case and `text-decorations` moved. 313 and 315 depend on nothing and are
eligible. 137 queue items are open. The next unused queue number is 316 and
the next unused ADR 0028.

Iteration 194 built item 313: a background of several layers, the colour
beneath and the first written on top, and radial gradients with a shape,
an extent keyword and a centre. Stops now mix with alpha premultiplied.
Paint records a `url()` layer as undrawn and a list it cannot read as
refused. Meet's corner tint is drawn, and the new case `background-layers`
pins the rest. 315 depends on nothing and is eligible. 136 queue items are
open. The next unused queue number is 316 and the next unused ADR 0028.

Iteration 195 built item 315: a width in a media query may be `em` or
`rem`, each the initial 16 px, so Meet's `(max-width: 48rem)` is understood
and applies at 768 px and below. Its test found that `line-height` never
reaches text, which Meet's heading shows, and opened it as 316, depending
on nothing and eligible. 136 queue items are open. The next unused queue
number is 317 and the next unused ADR 0028.

Iteration 196 built item 316: text, inline boxes and the strut take their
`line-height` on a line, with half the leading above and half below, and
an inline box is aligned by it. Meet's heading is 40, and the offline,
sign-in and settings screens moved to their stylesheets' spacing. 284,
311 and 314 still wait for pages. 135 queue items are open. The next unused queue number is 317 and the next
unused ADR 0028.

Iteration 197 decided item 124 in ADR 0028: stage 2 has no PDF viewer. A
response typed as a PDF reaches no renderer and is offered to the person as
a file, opened elsewhere only by a person's act. It is cut into 317 (behind
85 and 120, opened by a page) and 318 (behind a `Navigator` interface,
opened by a page). 121 and 122 were marked *needs design*. 137 queue items
are open. The next unused queue number is 319 and the next unused ADR 0029.

Iteration 198 froze `alo-downloads`, alo's public download page, exactly, and
built what it found first: item 319, a `<br>` ends its line. It opened 320
(`ch` and `ex` from the font) and 321 (text straight inside a flex container
takes its `line-height`), both depending on nothing, and it is the first
running frozen script to reach a regular expression, which makes item 74
reachable. 139 queue items are open. The next unused queue number is 322 and
the next unused ADR 0029.

Iteration 199 decided item 74 in ADR 0029: the regular expression engine
is ours, a backtracking machine with a stack of its own, every step counted
against a budget and the embedder's stop checked inside it. Running out is
a `RangeError`. Only the Unicode tables are rented, and Annex B's pattern
forms are refused by name. 74 keeps the engine and its bound; 322 (`i` and
`\p`), 323 (the string methods) and 324 (the constructor, `v` and `d`)
were opened. 74 is now eligible to build. 142 queue items are open. The
next unused queue number is 325 and the next unused ADR 0030.

Iteration 200 built item 74: the regular expression engine ADR 0029
decided, in `alo-js/src/regexp/` — the whole pattern grammar parsed, Annex B
refused by name, a compiler, and a matcher whose steps and places are
counted, with the embedder's stop asked inside. A literal makes a `RegExp`
with `exec`, `test` and `lastIndex`. `alo-downloads`' script compiles and
now stops at `navigator`, which opened 325 (`Navigator`, needing an ADR on
what it says). 322, 323 and 324 are eligible. 142 queue items are open. The
next unused queue number is 326 and the next unused ADR 0030.

Iteration 201 decided item 325 in ADR 0030: the browser names itself
(`Mozilla/5.0 (<system token>) alo/<major>.<minor>`), claims no other
engine, freezes one token per kind of system at compile time and measures
nothing about the machine. The string is both the `User-Agent` header and
`navigator.userAgent`, composed once in `alo-net` and told to the renderer
in `Page`; the compatibility mode is Gecko. 326 (the header) was opened and
depends on nothing; 325 now depends on it. 322 and 324 are reached by no
frozen page, and 323 waits on 73. 143 queue items are open. The next unused
queue number is 327 and the next unused ADR 0031.

Iteration 202 built item 326: `alo-net/src/user_agent.rs` composes ADR
0030's string and platform from the build's system, refusing to compile for
a system with no row. Every HTTP/1.1 and HTTP/2 request carries it once
unless the caller set its own, and a Reporting API envelope carries it as
`user_agent`. 325 (`navigator`) is now eligible. 142 queue items are open.
The next unused queue number is 327 and the next unused ADR 0031.

Iteration 203 built item 325's `navigator`: `Page` carries the user agent
string and platform `alo-net` composes, the wire carries them, and
`alo-bindings` installs a `Navigator` answering every member of ADR 0030
§ 5. `alo-downloads`' script runs past its fifth line and stops at
`getElementById`; marking the card was cut to 327 (`getElementById`,
`hidden`, `classList`), which 325's last clause now waits on. 327 is
eligible. 143 queue items are open. The next unused queue number is 328
and the next unused ADR 0031.

Iteration 204 built item 327 and so closed 325: `getElementById`,
`HTMLElement.hidden` and `Element.classList` (a live `DOMTokenList` with
`length`, `value`, `contains`, `add`, `remove` and `toggle`). The corpus
renders every loaded page as ADR 0030's macOS row, and `alo-downloads`
outlines the Mac's card and shows its badge. The rest of `DOMTokenList`
was cut to 328 (no page yet); the script's next stop, `querySelectorAll`,
opened 329, which is eligible. 143 queue items are open. The next unused
queue number is 330 and the next unused ADR 0031.

Iteration 205 built item 329: `querySelectorAll` on a document, an
element or a fragment, parsed and matched by `alo-css`, answering a static
`NodeList` with `length`, `item` and an indexed getter. `alo-downloads`
finds its two buttons and stops at `forEach`. `forEach` and the list's
iterators were cut to 331, which **needs an ADR** first: how a builtin
keeps state across the calls it asks for. A style sheet nested deep enough
crashes the renderer; that is 330, eligible, and the next item in queue
order. 144 queue items are open. The next unused queue number is 332 and
the next unused ADR 0031.

Iteration 206 built item 330: `alo-css` measures each rule's selectors,
each declaration's value and each at-rule's prelude before the recursive
parsers read them, counts `@media` inside `@media`, and drops what nests
past 32 with an issue saying so. A sheet nested 100 000 deep in any of
those ways no longer crashes the renderer. 143 queue items are open; 331
still needs its ADR, and 320 is next in queue order. The next unused
queue number is 332 and the next unused ADR 0031.

Iteration 207 decided item 331 in ADR 0031: a builtin declares up to eight
value slots, reserved on the stack above its arguments and written there at
once, so what it keeps across a call it asks for is where the collector
already walks. No code changed. Building the slots is 332, opened and
eligible; 331 (`forEach`), 221 (`apply`) and 228 now depend on it. 144
queue items are open. The next unused queue number is 333 and the next
unused ADR 0032.

Iteration 208 built item 332: `Native::keeping` declares up to eight
slots, `wait` reserves them on the stack above the arguments against the
stack's bound, `Call::kept` and `Call::keep` read and write them on the
stack, and a native declaring nine is refused by `Objects::native`. 331
(`forEach`) is eligible and is next on `alo-downloads`' path; 221 still
depends on the length conversions from 73. 143 queue items are open. The
next unused queue number is 333 and the next unused ADR 0032.

Iteration 209 built item 331: `Array.prototype.forEach` keeps `len` and
`k` in two slots, asks for a `length` getter, an object length's
conversion, an element's getter and the callback, skips holes, and asks
the embedder's stop on every pass. `alo-bindings` puts the realm's own
`forEach`, `keys`, `values`, `entries` and `[Symbol.iterator]` on
`NodeList.prototype`. `alo-downloads` walks its buttons and stops at
`fetch` (item 75). `for…of` over a `NodeList` is refused by name at the
list's getter `length` (231). 142 queue items are open. The next unused
queue number is 333 and the next unused ADR 0032.

Iteration 210 built item 320: `ex` and `ch` are the `x` height and `0`
advance of the first face an element's text is set in, asked of the fonts
through `alo_style::MeasureFace` and answered by `alo-renderer`'s `Faces`;
half an em where there is no face or glyph. `alo-downloads`' lede is 44 of
DejaVu's zeros, 483.74, and form fields at the user-agent sheet's `20ch`
grew in `alo-settings` and `web-a-form`. It served no open roadmap line.
141 queue items are open; 321 is eligible and next. The next unused queue
number is 333 and the next unused ADR 0032.

Iteration 211 built item 321 (text straight inside a flex or grid container
is wrapped in a block). Iteration 212 decided item 83 in ADR 0032: a
script's fetch is an ask in the answer to the message whose work made it,
the browser process decides it and filters the response before it leaves,
and the response is a task of its own. No code was written. 83 is cut into
334 (the boundary and the decision), 335 (`fetch()` in a page, closed by
`alo-downloads`) and 336 (`XMLHttpRequest`, opened by a page); a promise is
cut from 75 as 333, on 232 and 235, which are built. 333 and 334 are
eligible. 144 queue items are open. The next unused queue number is 337 and
the next unused ADR 0033.

Iteration 213 built item 333, a promise: the cell, the constructor and its
executor, `then`, `catch`, `finally`, `Promise.resolve` and `Promise.reject`,
each reaction a job on the engine's queue, a thenable adopted by a job of
its own, and a rejection nobody handled reported at the end of the
checkpoint. A constructor other than `Promise` is refused by name and cut as
337, which waits for a page. 334 is eligible and next; 335, which moves
`alo-downloads`, waits only on it. 144 queue items are open. The next unused
queue number is 338 and the next unused ADR 0033.

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

Iteration 214 built item 334, the fetch boundary: the asks every answer
carries, `ToRenderer::Fetched` and its answer `Delivered`, both on the wire;
the browser process's decision (`fetch_decide.rs`), the filter
(`fetch_filter.rs`), the two bounds (`fetch_owed.rs`), and `Tabs` deciding
and delivering. Making a decided fetch through the pool is cut as 338, which
is eligible. 335, which moves `alo-downloads`, is eligible and next: its
dependencies 333 and 334 are done, and it answers the corpus's fetches from
frozen responses rather than through 338. 144 queue items are open. The next
unused queue number is 339 and the next unused ADR 0033.

Iteration 215 built item 335, `fetch()` in a page: the bindings' `fetch`,
its request steps and the read-only `Response` and `Headers`; each answer a
task settling the page's promise; the asks carried in every answer; and the
corpus answering a case's fetches from frozen responses through the
browser process's own decision and filter. `alo-downloads` asks for both
installers and, offline, marks both buttons; it then stops at `a.style`,
cut as item 339 from 89. 338 is eligible and next — a page can now ask, so
its closing test has a page to drive — and 339 is eligible after it. 144
queue items are open. The next unused queue number is 340 and the next
unused ADR 0033.

Iteration 216 built item 338: a decided fetch is made. `fetch_make` sends
each hop through the session's pool with its `Origin`, `Referer` and
`Cookie` decided again, asks first when CORS needs it, keeps `Set-Cookie`
only with credentials, checks every answer on a CORS chain, and follows,
stops at or refuses a redirect by its rule; `fetch_answering` answers a
tab's fetches one at a time; `alo-window`'s conductor makes them between
orders. `alo-net` keeps a response's reason phrase (cache format 2). A
redirect judged by `connect-src` is cut as item 340: it depends on nothing
open and sits before 339 in the queue, so it is next, and 339 after it. 144
queue items are open. The next unused queue number is 341 and the next
unused ADR 0033.

Iteration 217 built item 340: a redirect a page's fetch follows is judged by
`connect-src`. `alo-net`'s `Request` says whether a redirect led to it, set
only by `redirect::next`, and CSP matching ignores a host source's path for
such a request while still checking its scheme, host and port; the decided
`Fetch` carries the document's policies and `fetch_make` asks them about
every hop after the first, refusing and recording one the policy does not
allow. 339 is eligible and next. 143 queue items are open. The next unused
queue number is 341 and the next unused ADR 0033.

Iteration 218 decided item 339: ADR 0033, an element's style is its
`style` attribute, and a value is kept as written. No code changed. The
build is cut as 341 (the attribute cascaded, depending on nothing open),
342 (`element.style`, on 341, which moves `alo-downloads`) and 343 (a
page's `style-src` applied to inline style in the renderer, on 341 and for
its last half 342). 339 stays open until 341 and 342 close. 341 is eligible
and next. 146 queue items are open. The next unused queue number is 344 and
the next unused ADR 0034.

Iteration 219 built item 341: an HTML or SVG element's `style` attribute
is parsed as a sheet's block (`alo-css`'s `parse_declaration_list`) and
cascaded at CSS Cascade 4's element-attached step in `alo-style`, read
from the document on every draw. Corpus case `style-attributes` is its
layout assertion and reference render. 342 is eligible and next, then 343.
145 queue items are open. The next unused queue number is 344 and the next
unused ADR 0034.

Iteration 220 cut item 344 from 342 and built it: `alo-css`'s list of the
properties this engine acts on (`properties.rs`), held true by a test in
each of the six crates that read one, the longhands each shorthand covers
(`longhand.rs`), and `InlineStyle` (`inline.rs`), CSSOM's edits and
serialiser over a `style` attribute's text. 342 is now only the binding
over it, eligible and next, then 343. 145 queue items are open. The next
unused queue number is 345 and the next unused ADR 0034.

Iteration 221 built item 342: `element.style`, a `CSSStyleDeclaration` over
the `style` attribute, on HTML elements and a new `SVGElement`, with every
member of ADR 0033 § 6 but the indexed getter, which was cut to 345.
`alo-downloads`' script runs to its end and its offline buttons are greyed,
so 339 closed with it. 343 is eligible and next. 144 queue items are open.
The next unused queue number is 346 and the next unused ADR 0034.

Iteration 222 decided how item 343 records a style written by script: ADR
0034, a style written by script is remembered by its text, and a policy is
asked when it is drawn. No code changed. `alo-dom`'s element will keep the
text `element.style` last wrote, and an attribute equal to it is exempt
from `style-src`. Every other `style` attribute and every `<style>` is
judged at each draw by all the policies the page holds. One function in
`alo-bindings` serves the renderer and `element.style`, so a refused
attribute reads as empty. Reporting what a draw after load finds is cut to
346, which depends on 343. 343 is eligible and next. 145 queue items are
open. The next unused queue number is 347 and the next unused ADR 0035.

Iteration 223 built item 343 (ADR 0034): a page's `style-src` is applied
to its inline style. Every `<style>` and `style` attribute is judged at
every draw by every policy the page holds, its `<meta>` policies kept for
the page's life; a value `element.style` wrote is exempt by its text, and a
refused attribute reads as `""` through `element.style`. What the load's
draw objects to crosses with its placement and is reported. 346 (a later
draw's objections, carried) is now eligible. 144 queue items are open. The
next unused queue number is 347 and the next unused ADR 0035.

Iteration 224 built item 346 (ADR 0034 § 4): inline style a policy objects
to, found by a draw after load, is reported. The renderer remembers each
element, placement and text a header policy objected to for the page's
life (`objected.rs`), and what draws find waits for the next `Acted` or
`Delivered`, which gained `objections` on the wire, under the bound of 64.
143 queue items are open. The next unused queue number is 347 and the next
unused ADR 0035.

Iteration 225 found nothing eligible ahead in the queue (every open item
waits on a page, a person, an open dependency or Linux) and took a real
page: alo Sites' `section_cta.html`, from `alo-workplace`. It cannot be a
case: it runs a script and links its style sheet, and nothing hands a
renderer a linked sheet. That opened item 347, decided as ADR 0035: a
renderer asks for each linked sheet, the browser process fetches it as a
style request, and only a 2xx `text/css` body crosses. No code changed.
The build is cut as 348 (asked, decided, delivered, and the window waiting
for the load's sheets; depends on nothing open, eligible and next) and 349
(the corpus answering a loaded case's sheets, and the page frozen; on
348). A loaded page's pictures are 350, which needs its own ADR and a page
whose picture can be frozen. 147 queue items are open. The next unused
queue number is 351 and the next unused ADR 0036.

Iteration 226 built item 348 (ADR 0035 §§ 1–4): a renderer asks for each
linked sheet once per document (`linked.rs`), the browser process decides
it as a style request (`sheet_decide.rs`, `sheet_owed.rs`) and makes it
through a fetch's hops, sending only a 2xx `text/css` body
(`sheet_make.rs`), and the window's conductor makes the asks and paints
again. ADR 0035 § 5, the window holding the first frame until the load's
sheets answer, is cut to 351. It needs design first, because one trickling
request holds the conductor's thread and no window bound can end that.
349 (the corpus answering a loaded case's sheets, and `alo-sites-cta`
frozen) now depends on nothing open and is eligible and next. 147 queue
items are open. The next unused queue number is 352 and the next unused
ADR 0036.

Iteration 227 built item 349 (ADR 0035 § 6): the corpus answers a loaded
case's sheet asks from its `linked.txt`, by URL against its `address.txt`,
through the browser process's own decision (`sheet_decide`) and check
(`sheet_make::style_sheet`), in `alo-corpus`'s `sheets.rs`. alo Sites'
call-to-action section is frozen as `alo-sites-cta` and renders with its
sheet. What it shows wrong is opened as 352 (an absolutely positioned
inline left in flow: the skip link; depends on nothing open, and eligible)
and 353 (`Date`, which needs an ADR first). 347 now waits only on 351. 148
queue items are open. The next unused queue number is 354 and the next
unused ADR 0036.

Iteration 228 built item 352: `alo-box` blockifies an absolutely
positioned box where it decides `display` (CSS Display § 2.7), so alo
Sites' skip link is out of flow and off the page, and `alo-sites-cta`'s
section starts at the top. A new case, `absolute-inline`, pins it. Two
faults it found, already true of an absolutely positioned block, wait for
pages: 354 (one among a line's content splits the line, and the static
position) and 355 (the nearest positioned ancestor). 353 (`Date`) needs
its ADR as an iteration of its own. 351 still needs design. 149 queue items
are open. The next unused queue number is 356 and the next unused ADR 0036.

Iteration 229 decided item 353 as ADR 0036: `Date` is the engine's and the
instant is the embedder's (a `Clock` handed to a realm, refused by name
when absent), the renderer reads the wall clock in whole milliseconds with
no jitter, and a page's local zone is UTC until the person sets one. The
build is cut as 356 (`Date` with a clock; depends on nothing open, eligible
and next), 357 (a date as text; waits for a page) and 358 (the person's
zone; on settings, 128). 353 closes when 356 does. 152 queue items are
open. The next unused queue number is 359 and the next unused ADR 0037.

Iteration 230 built item 356, and with it closed 353: `alo-js` is handed a
`Clock` (or none, and refuses by name), does ECMA-262's time arithmetic in
`f64`, and has `Date` but for text — the constructor, `now`, `UTC`, the
getters and setters, `toISOString`, `toJSON` and `Symbol.toPrimitive`,
which `ToPrimitive` now asks for first. The renderer reads the wall clock
in whole milliseconds; the corpus loads every case at one fixed instant
(`INSTANT`), and `a-script-writes-the-date` draws it. alo Sites' analytics
script stops next at `encodeURIComponent`, opened as 359 (depends on
nothing open, eligible and next). 357 waits for a page, 358 for settings.
151 queue items are open. The next unused queue number is 360 and the next
unused ADR 0037.

Iteration 231 built item 359: `encodeURIComponent` (`alo-js/src/uri.rs`
for ECMA-262's `Encode`, bounded by `LONGEST_STRING` as it grows, and
`builtin/encode_uri_component.rs` for the function), with `URIError` now an
error the engine throws. alo Sites' analytics script resolves it and stops
at its argument on the same line, `location`, so "runs past line 8" is not
met and is cut into **360** (`location`, read; whether it needs design
first is the next iteration's question). 357 waits for a page, 358 for
settings. 151 queue items are open. The next unused queue number is 361
and the next unused ADR 0037.

Iteration 232 built item 360: `location`, read (`alo-url/src/reading.rs`
for the URL Standard's nine readings; `alo-bindings`' `location.rs` for the
page's one `Location` and the global's accessor, `interface/location.rs`
for its unforgeable members, and `Document`'s unforgeable `location`).
Navigating through it is refused by name until 85. alo Sites' analytics
script runs past line 8 and stops at line 32, `window`, opened as **362**
(whether it needs design is the next iteration's question; 251 is
related). `Location`'s exotic internal methods are cut into **361**
(waits for a page or item 73). 357 waits for a page, 358 for settings.
152 queue items are open. The next unused queue number is 363 and the next
unused ADR 0037.

Iteration 233 decided item 362 as ADR 0037, and built nothing. ADRs
0017–0019 had not answered the item's two questions.
- The global object becomes a `Window` that `alo-bindings` makes. The
  engine gains a realm whose global the host makes, which is ECMAScript's
  own provision.
- The `Window` holds its own listeners, and every path ends at it after the
  document, except for `load`.
- `window`, `self` and `globalThis` are all the global object, with no
  `WindowProxy` until frames, and there is no named access, by law 1.
- This comes before 251. 251 shrinks to `document`'s accessor and still
  waits on 73. ADR 0018 § 1 is amended.

362 is designed, eligible and next. Cut from it: **363**, the window's
immutable prototype (waits on 73), and **364**, the page lifecycle at the
window (`pagehide`, `visibilitychange`; needs design). 154 queue items are
open. The next unused queue number is 365 and the next unused ADR 0038.

Iteration 234 built item 362 as ADR 0037 designs it. `alo-js` makes a
realm whose global object an embedder makes (`Engine::with_global`), and
`alo-bindings` makes it a `Window` (`window.rs`, `interface/window.rs`):
- `window`, `self`, `globalThis` and the top-level `this` are one object;
- it holds its own listeners;
- `location` is its own unforgeable member.
Every event path ends at it after the document, except for `load`, and the
renderer's event loop makes its engine that way. alo Sites' analytics
script runs to its end. Its `pagehide` listener, dispatched by a test,
stops at `Math`, opened as **365** (cut from 73; whether `Math.random`
needs an ADR is that item's question). The viewport it reads
(`innerWidth`, `scrollY` and the rest, `undefined` today) is opened as
**366**, which needs an ADR. 363 waits on 73 and 364 needs design. 155
queue items are open. The next unused queue number is 367 and the next
unused ADR 0038.

Iteration 236 tried item 296's capture first, since 235 had unblocked it.
The screen was locked, so the capture came back black: 296 waits for a
person to unlock the machine. It then built item 365, `Math`, in
`alo-js`' `builtin/math.rs` and `builtin/math_fold.rs`, and renamed
`date_numbers.rs` to `numbers.rs`. The frozen page's `pagehide` listener
now runs to its end. Cut from it: **367**, `Math.random`, which needs an
ADR, and **368**, `f16round` and `sumPrecise`. Opened: **369**,
`navigator.sendBeacon`, which needs an ADR (ADR 0032 left it undecided).
157 queue items are open. The next unused queue number is 370 and the next
unused ADR 0038.

Iteration 237 found the screen still locked, so 296's capture is not
takeable. 367 has no page that opens it, and 369 waits on 364. So it
decided item 366 as ADR 0038, and built nothing. A page reads its viewport
(`innerWidth`, `innerHeight`), a scroll position the renderer holds at zero
(`scrollX`, `scrollY`, `pageXOffset`, `pageYOffset`), and an element's
scrolling area measured from the layout it would be drawn with at that
moment (`scrollWidth`, `scrollHeight`). It reads these through a `View`
trait the renderer implements, and reads nothing beyond the window. 366 is
now the first build, designed and eligible. The scrolling area is cut to
**370**, which depends on 366. 158 queue items are open. The next unused
queue number is 371 and the next unused ADR 0039.

Iteration 238 found the screen still locked, so 296's capture is still not
takeable. It built item 366: `alo-bindings`' `View` trait and `show`, the
window's `innerWidth`, `innerHeight`, `scrollX`, `scrollY`, `pageXOffset`
and `pageYOffset` (`interface/window_cssom_view.rs`), and `alo-renderer`'s
`PageView`, which the renderer sets at load and at every `Resize`. alo
Sites' analytics script reads 800 × 600 at the top, and its `shape()`
reports `&p=%2F&w=800`. 370 is now eligible and next. 157 queue items are
open. The next unused queue number is 371 and the next unused ADR 0039.

Iteration 239 found the screen still locked, so 296's capture is still not
takeable. It built item 370: `scrollWidth` and `scrollHeight` on `Element`
(`interface/element_cssom_view.rs`), measured by the renderer mid-script on
an `Easel` it shares with the page's view (`easel.rs`,
`scrolling_area.rs`), the drawing kept for the next paint; and
`BoxGeometry::reach` in `alo-layout`. alo Sites' `pagehide` listener
reports `d=1000&p=%2F&w=800` and `t=0`. It cut 371, the size a tab no
window shows (no embedder opens one yet), and 372, a measurement stopping
before paint. 158 queue items are open. The next unused queue number is
373 and the next unused ADR 0039.

Iteration 240 found the screen still locked, so 296's capture is still not
takeable. It decided item 364 as ADR 0039, and built nothing. The browser
process owns a page's visibility state: a tab is `visible` when it is the
selected tab of a window that is not occluded or minimised, and a tab no
window shows is `visible`. The renderer is told it with the `Page` and by
`ToRenderer::Visibility`, a task that fires `visibilitychange` when it
changes. A page is left only through HTML's leaving steps (`pagehide`, the
state becoming `hidden`, `unload`), as one task with a one-second deadline,
run by `Leave` and before every `Load` into a renderer holding a page. A
leaving page's fetches are refused by name until 369 decides keep-alive.
There is no back-forward cache and no `beforeunload`. 364 is now the first
build, designed and eligible. The leaving steps are cut to **373**, and
369 now depends on 373. 159 queue items are open. The next unused queue
number is 374 and the next unused ADR 0040.

Iteration 241 built item 364, the visibility state (ADR 0039 § 1). A
`Page` carries the state it loads in and `ToRenderer::Visibility` changes
it: a task that sets the document cell's state and fires one
`visibilitychange`, at the document and then the window, only when it
changed (`event_loop/shown.rs`, `show.rs`). It is answered as a delivery
is, and `Tabs::visibility` decides what its listeners ask for as the
document's. `document.visibilityState` and `document.hidden` read the cell
(`alo-bindings`' `visibility.rs`, `interface/document_visibility.rs`), and
a document no window was associated with stays `"hidden"`. `alo-window`
sends `Order::Visibility` on `Occluded`, and the conductor tells the
selected tab, paints it again, and loads later pages in the window's
state. alo Sites' page, hidden, reports `d=1000&p=%2F&w=800` and `t=0`
from its own listener. 373, a page left, is next and eligible; 369 follows
it. 158 queue items are open. The next unused queue number is 374 and the
next unused ADR 0040.

Iteration 242 built item 373, a page left (ADR 0039 §§ 2–4). A load's last
step is `pageshow` at the window, and every `Load` into a renderer holding a
page and every `ToRenderer::Leave` runs the page's leaving steps —
`pagehide`, `hidden`, `unload` — as one task under a one-second deadline on
the page's own `Stop`, then lets go of the page whole. `FromRenderer::Left`
and `Loaded`'s `left` carry its last fetches, which the browser process
refuses by name (`Rule::Leaving`) as that page's document's, and
`Tabs::close` and the conductor's `close_everything` leave every page and
record the refusals. alo Sites' page, left, reports `d=1000&p=%2F&w=800` and
`t=0` from its own `pagehide`. ADR 0039's flags for `pagehide` were
corrected to HTML's, and the legacy target override is cut to **374**,
opened by a page that reads such an event's `target`. 369, `sendBeacon`,
now has its dependency met and needs its ADR. 158 queue items are open. The
next unused queue number is 375 and the next unused ADR 0040.

Iteration 243 decided item 369 as ADR 0040, and built nothing. A
keep-alive request is a page's fetch that may be made after its page is
gone. It is decided by every rule an ordinary fetch is (ADR 0032 § 3) and
gains nothing else: the partitioned jar's cookies, the document's
`connect-src`, and a record line with a purpose of its own, **beacon**,
caused by the document. Once its page has gone it is made and answered by
nobody. Every other fetch for that page is still not made, and
`Rule::Leaving` now refuses only asks without `keepalive`. One still waiting
when the browser closes is not made, and is recorded as such. A document may
have 64 KiB of keep-alive body in flight, Fetch's number, counted by the
renderer so that `sendBeacon` can answer `false`, and enforced again by the
browser process. 369 is now the first build, designed and eligible.
`fetch(…, { keepalive: true })` is cut to **375**, opened by a page that
uses it. 159 queue items are open. The next unused queue number is 376 and
the next unused ADR 0041.

Iteration 244 built item 369. `navigator.sendBeacon` is on
`Navigator.prototype` (`alo-bindings`' `beacon.rs`), and an ask carries a
`keepalive` claim (`Keepalive`) across the wire. The renderer counts
keep-alive bodies in flight against 64 KiB and frees each as its answer
arrives, and the browser process counts again (`fetch_kept.rs`) and refuses
past it by `Rule::KeptAlive`. A beacon is recorded as `Purpose::Beacon`
under `connect-src`. A left page's asks without `keepalive` are still
refused by `Rule::Leaving`; its beacons are decided as the document's and
made after it has gone, answered by nobody (`Tabs::leaving`,
`Answering::outlive`). What is still waiting when the browser closes is
recorded as not made (`Answering::close`, `Rule::Closed`). alo Sites' page
reports with no beacon lent. 375 is now the bindings' `keepalive` member
alone, still waiting on a page. 158 queue items are open. The next unused
queue number is 376 and the next unused ADR 0041.

Iteration 245 decided item 351 as ADR 0041, and built nothing. Every
request the conductor makes for a page moves to one network thread, the
only holder of the session's `Network`, sent jobs in order and making one
exchange at a time in `Answering`'s unchanged order. The conductor gains one
inbox for the window's orders and the network's results, and never waits on
a server. A tab whose load asked for sheets is held, painting nothing, until
every owed sheet is answered or the window's own bound passes. At the bound
the page is shown and said to be shown before its style, and the late sheet
is still applied when it comes. A bound on a whole exchange is cut to
**376**, found rather than opened by a page, and needing design. 351 is now
the first build, designed and eligible. 159 queue items are open. The next
unused queue number is 377 and the next unused ADR 0042.

Iteration 246 built item 351, and with it closed 347. Requests a page asks
for are made on `alo-window`'s network thread, the only holder of the
session's `Network`, one exchange at a time in `Answering`'s order, which is
now split into `next` and `exchanged` (`alo-renderer`'s `fetch_exchange.rs`).
The conductor waits only on its inbox. A load's first frame is held until
its sheets answer or `LONGEST_HOLD`, three seconds, passes, and is then said
to be shown before its style. 376 is unchanged: a trickling sheet still
holds the network thread's queue, though no longer the window. 157 queue
items are open. The next unused queue number is 377 and the next unused ADR
0042.

Iteration 247 found every open item blocked or waiting for a page (296's
screen is still locked), so it froze the next alo Sites section with no
picture, its pricing section, as `alo-sites-pricing`. Its highlighted tier's
shadow is a `color-mix(in srgb, …)`, which was refused, and with it the
shadow: that opened item 377, built in the same iteration. `currentColor` in
a mix is cut to 378, which waits for a page. 158 queue items are open (377
added closed, 378 added open). The next unused queue number is 379 and the
next unused ADR 0042.

Iteration 248 found every open item blocked or waiting for a page (296's
screen is still locked), so it froze the next alo Sites section with no
picture, its features section, as `alo-sites-features`. Its bento grid's
first card's `padding-block: 2.5rem` was dropped, since no logical shorthand
was split: that opened item 379, built in the same iteration for the block
axis, which is exact under the only writing mode laid out. The inline axis
is cut to 380, waiting for a page. The section itself is drawn at opacity 0
until an `IntersectionObserver` reports it, so the page's picture is white:
that opened 381, which needs an ADR and depends on 234's rendering steps.
160 queue items are open (379 added closed, 380 and 381 added open). The
next unused queue number is 382 and the next unused ADR 0042.

Iteration 249 found every open item blocked or waiting for a page (296's
screen is still locked), so it froze the next alo Sites section with no
picture and nothing set to move into view, its testimonials section, as
`alo-sites-testimonials`. Its layout checked against the sheet by hand, but
its italic quote was drawn upright: the corpus has no slanted face, and the
engine made none up where every browser leans the upright one. That opened
item 382, built in the same iteration in `alo-paint`'s `synthesis.rs`.
`font-synthesis` itself, a synthesised bold and an angled `oblique` are cut
to 383, waiting for a page. 161 queue items are open (382 added closed, 383
added open). The next unused queue number is 384 and the next unused ADR
0042.

Iteration 250 found every open item blocked or waiting for a page (296's
screen is still locked), so it froze the next alo Sites section with no
picture and nothing set to move into view whose layout this engine can do:
its booking section, as `alo-sites-booking` (the FAQ section, first in that
list, lays out in two columns, item 97, and was not kept). Its empty day
field stood on its bottom edge with the font's descent under it, so the form
was 7.71 taller than a browser makes it. That opened item 384, built in the
same iteration: a one-line field with nothing typed in it stands on its
strut's baseline. What an empty date field shows, its format and a calendar
button, is cut to 385, which needs an ADR. 162 queue items are open (384
added closed, 385 added open). The next unused queue number is 386 and the
next unused ADR 0042.

Iteration 251 took item 385, the first open item whose next step depends on
nothing unbuilt, and wrote its decision rather than code, as LOOP.md § 4
asks: ADR 0042. An empty date field draws `yyyy-mm-dd` in its own colour
until the person chooses a region; an agent reads the role `date` and puts a
valid date string into it; no calendar is drawn until it opens something.
385 stays open with a *Decided* paragraph and is buildable next. The cuts are
386 (the calendar, after 296), 387 (the person's region, after 128) and 388
(the other temporal kinds, waiting for a page); 389, a field's `placeholder`,
was opened by `alo-sign-in` and needs an ADR of its own (the first
pseudo-element). 166 queue items are open (386–389 added open). The next
unused queue number is 390 and the next unused ADR 0043.

Iteration 252 built item 385 by ADR 0042: an empty date field draws
`yyyy-mm-dd` in its own colour, an agent reads it as a `date` with no value,
and `PutText` takes a valid date string and refuses anything else as
`NotADate`. It also fixed a stage 1 paint fault it found: a field's text
painted the field's background and border again around itself. 165 queue
items are open. The next unused queue number is 390 and the next unused
ADR 0043.

Iteration 253 took item 389, the first open item whose next step depends on
nothing unbuilt, and wrote its decision rather than code: ADR 0043. A
pseudo-element's style is computed as its originating element's child and
kept beside it; `::placeholder` is the first, drawn in its own colour in the
field's line and read by an agent as a `placeholder` property, never as
text. 389 stays open with a *Decided* paragraph and is buildable next. The
cuts are 390 (clipping a long hint), 391 (`:placeholder-shown`) and 392
(`::placeholder`'s other properties), each waiting for a page. 168 queue
items are open (390–392 added open). The next unused queue number is 393
and the next unused ADR 0044.

Iteration 254 built item 389 (ADR 0043). `::placeholder` is the first
pseudo-element this engine styles: matched through its field
(`alo-css`'s `matches_pseudo`), computed as the field's child and kept
beside it (`alo-style`'s `pseudo.rs`, `StyleTree::pseudo`), made only for a
field that shows one (`alo-dom`'s `placeholder.rs`), drawn as a run in the
field's line that adds nothing to its size, and read by an agent as a
`placeholder` property, through the renderer's snapshot too. alo's
sign-in screen draws its email hint in `--text-tertiary`. Found and fixed
on the way: a one-line field taller than its line now holds its line in
the middle. 167 queue items are open (389 closed). The next unused queue
number is 393 and the next unused ADR 0044.

Iteration 255 found every open item blocked or waiting for a page (296's
screen is still locked), so it froze the next alo Sites sections with no
picture and nothing set to move into view. The shop section rendered and
checked against the sheet by hand, and found nothing, so it was not kept.
The tickets section was kept as `alo-sites-tickets`: its banner lifts the
card's limit with `max-width: none`, which layout recorded as a value it
did not implement, though the box came out right. That opened item 393,
built in the same iteration: `max-width` and `max-height` read `none` as
no limit and refuse `auto`. 167 queue items are open (393 added closed).
The next unused queue number is 394 and the next unused ADR 0044.

Iteration 256 found every open item as iteration 255 left it, so it froze
the next alo Sites sections with no picture and nothing set to move into
view. The empty catalog and empty collection rendered and checked against
the sheet by hand, and found nothing, so they were not kept. The closed
booking section was kept as `alo-sites-booking-closed`: its two `1fr`
columns came out 320 and 366, because layout answered "how narrow can this
text be" with its whole line. That opened item 394, built in the same
iteration: text at min-content is laid out in no room, so it is as narrow
as its widest word. 167 queue items are open (394 added closed). The next
unused queue number is 395 and the next unused ADR 0044.

Iteration 257 found every open item as iteration 256 left it, and found
that three alo Sites sections thought to be set to move into view name
`section-motion` only in their script: the navigation, the contact form
and the transition. The transition is `display: none`. The navigation was
kept as `alo-sites-nav`: its `inline-flex` menu links stood on their
bottom edges, so every list item had the strut's descent under it and the
bar was 81.1 where a browser draws 73.4. That opened item 395, built in the
same iteration: an `inline-flex` box stands on its first item's baseline.
The contact form found item 397, a `gap` written with a function read as
no gap, which depends on nothing open and is buildable next with that page
frozen. Cut or found: 396 (a row aligned on baselines) and 398 (bare text
in a flex item does not shrink), both waiting for a page. 170 queue items
are open (395 added closed, 396–398 added open). The next unused queue
number is 399 and the next unused ADR 0044.

Iteration 258 took item 397, which iteration 257 left buildable next, and
froze alo Sites' contact form as `alo-sites-contact-form`. The `gap`
shorthand is now split into `row-gap` and `column-gap` in `alo-css` as it
is written, beside `padding-block`, at the spaces between values only, so
`clamp(2rem, 6vw, 5rem)` is one 48-pixel gap and the form's columns are
284.8 and 427.2, 48 apart; no other case moved. Nothing was cut or found.
169 queue items are open (397 closed). The next unused queue number is 399
and the next unused ADR 0044.

Iteration 259 found every open item as iteration 258 left it, so it froze
alo Sites' footer, one of the last sections with no picture, as
`alo-sites-footer`. Its empty `<main>` lets the footer's margin collapse
through `body`, which starts 48 down, and the skip link went with it: that
opened item 355, built in the same iteration. An absolute box is now laid
out against its nearest positioned or transformed ancestor, or the
initial containing block, keeping its static position on an axis with no
inset. No other case moved. Cut: 399 (static position in a flex or grid
container further in, a containing block beyond an atomic inline box, and
the containing blocks of properties not read). 169 queue items are open
(355 closed, 399 added open). The next unused queue number is 400 and the
next unused ADR 0044.

Iteration 260 found every open item as iteration 259 left it, so it froze
alo Sites' rich text, the golden output of its BlockNote renderer and the
body of every blog post it publishes, as `alo-sites-rich-text`. It has no
sheet, so the user-agent sheet draws it all. That opened item 400, built in
the same iteration: a list in a list has no block margins, and `<s>`,
`<del>`, `<u>` and `<ins>` are drawn as edits. No other case moved. Found:
401 (list markers, needs ADR) and 402 (`font-weight: bold` measured bold by
layout and drawn regular by paint, which depends on nothing open and is
buildable next with this page frozen). 171 queue items are open (400 added
closed, 401 and 402 added open). The next unused queue number is 403 and
the next unused ADR 0044.

Iteration 261 built item 402: `font-weight` is read once, by
`alo-style`'s `ComputedStyle::font_weight`, and layout and paint both ask
it, so text written `bold` is drawn in the bold face it was measured in.
Eleven case pictures moved, each reviewed (only bold text changed), and no
layout file. Cut: 403 (`bolder` and `lighter`, waits for a page). 171 queue
items are open (402 closed, 403 added open). The next unused queue number
is 404 and the next unused ADR 0044.

Iteration 262 decided item 401: ADR 0044, a list item's marker. A
`::marker` is made by the item's computed style (extending ADR 0043 § 2),
styled as its child, numbered by HTML's ordinal value, written in one of
five counter styles as text, set outside the item's first line taking no
room, and read by an agent as the item's `marker` property. No code; 401
stays open, decided and buildable next. Cut, each waiting for a page:
404–409. 177 queue items are open (404–409 added open). The next unused
queue number is 410 and the next unused ADR 0045.

Iteration 263 built item 401: a list item draws its `::marker`. `list-style`
is split in `alo-css`; `alo-style` reads `display` (its file moved from
`alo-box`), decides which elements make a marker, numbers them by HTML's
ordinal value and writes the five counter styles; `alo-box` makes the
marker a text box first in its item; `alo-layout` keeps it out of every
line and stands it outside the item's first line; an agent reads it as the
item's `marker`. Four case references moved, each only a marker appearing;
the ten alo Sites copies that write `list-style: none` did not. Nothing
cut beyond 404–409, and 409 now names an inline list item. 176 queue items
are open (401 closed). The next unused queue number is 410 and the next
unused ADR 0045.

Iteration 264 found every open item blocked, as iteration 263 left it, so
it froze alo Sites' other rich blocks — a figure, two code blocks and an
equation, the golden output of its BlockNote renderer — as
`alo-sites-rich-blocks`. That opened item 410, built in the same
iteration: a space `pre` keeps takes its room at the start of a line, so
the Rust block's indented line starts four spaces in. No other case moved.
Found: 411 (a line of only kept white space, such as a blank line in a
`<pre>`, is no line; waits for a page), 412 (a monospace face in the
corpus; depends on nothing open and is buildable next), 413 (a broken
picture drawn as its `alt`; needs ADR) and 414 (a picture in a `data:`
URL; waits for a page). 180 queue items are open (410 added closed,
411–414 added open). The next unused queue number is 415 and the next
unused ADR 0045.

Iteration 265 built item 412, the one item iteration 264 left buildable:
the corpus and `--frozen-fonts` carry DejaVu Sans Mono and answer
`monospace` with it, so `alo-sites-rich-blocks`' code and `alo-downloads`'
`<code>`s are drawn monospaced and neither says the family is missing. No
other case moved. Whether `monospace` takes browsers' smaller default size
was cut to 415, which needs its ADR. 180 queue items are open (412 closed,
415 added). The next unused queue number is 416 and the next unused ADR
0045.

Iteration 266 decided item 415: ADR 0045, `monospace`'s own default size,
measured in Chrome 154 on this machine, `alo-sites-rich-blocks` included.
A computed font size carries whether it came from a keyword, its parent's
font or a fixed length; `font-family: monospace` alone has a `medium` of
13 px; where an element crosses into or out of it a keyword is read again
and a relative size rescaled by 13/16, and a fixed one stays. No code; 415
stays open, decided and buildable next. Cut: 416 (keyword sizes as the
engines' integer table rather than CSS's ratios, for every family; waits
for a page). 181 queue items are open (416 added open). The next unused
queue number is 417 and the next unused ADR 0046.

Iteration 267 built item 415 (ADR 0045): a computed font size carries its
origin, `font-family: monospace` alone has a `medium` of 13 px, and a size
crossing into or out of it is read again, rescaled or kept by that origin.
`font-size` no longer inherits as text. `alo-sites-rich-blocks`' code is
13 px, as Chrome draws it; no other case moved. Found: 417 (`font-size:
initial` is the parent's size rather than `medium`; waits for a page). 181
queue items are open (415 closed, 417 added). The next unused queue number
is 418 and the next unused ADR 0046.

Iteration 268 decided item 413: ADR 0046, a broken picture is its text,
measured in Chrome 154 on this machine, `alo-sites-rich-blocks` included.
Whether an `<img>` shows a picture is decided once per drawing, before the
boxes; with no picture and a non-empty `alt` it is inline text of that
`alt` in its own style (`img` becomes `display: inline`), otherwise a
replaced box of natural size 0; no icon; an agent reads `[broken]`. No
code; 413 stays open, decided and buildable next. Cut, each waiting for a
page: 418 (an icon), 419 (`<img>`'s `width` and `height` attributes), 420
(a picture on its way). 184 queue items are open (418–420 added open). The
next unused queue number is 421 and the next unused ADR 0047.

Iteration 269 built item 413 (ADR 0046): whether an `<img>` shows a picture
is decided by `alo-renderer`'s `pictures.rs` before the boxes and handed to
`alo-box` as a `Shown`; a broken `<img>` with an `alt` is inline text of it
in its own style, without one a replaced box of nothing; `img` is `display:
inline`; an agent reads `[broken]` and the `alt` once. A replaced box with a
flow `display` is handed to `taffy` as a `flow-root`, found when an inline
picture stretched. `alo-sites-rich-blocks` draws "Orange notebook <front>";
no picture moved. 183 queue items are open (413 closed). The next unused
queue number is 421 and the next unused ADR 0047.


Iteration 270 found every open item waiting on a page and froze one: alo
Sites' text-and-image section, `alo-sites-text-image`, whose picture's
address has no bytes anywhere and is drawn as its `alt` since 413. It
opened item 380: the overlapping card's `margin-inline-start: -4rem` was
ignored. `alo-css`' new `logical.rs` maps the logical margin and padding
longhands to their sides under left to right, `margin-inline` and
`padding-inline` are split beside the block pair, and the card is 428 at
x 352, as Chrome 154 measures it. The right-to-left half is cut to 421,
waiting for a page. 183 queue items are open (380 closed, 421 added). The
next unused queue number is 422 and the next unused ADR 0047.

Iteration 271 found every open item waiting on a page and probed alo Sites'
unfrozen goldens against Chrome 154 given the corpus' fonts: team, gallery,
hero, collection, catalog and the ordering catalog matched and were dropped;
the blog index did not, and was frozen as `alo-sites-blog-index`. It opened
item 354, now built: a box `position: absolute` written in a line stays in
it, takes no room, and stands at its static position in each axis whose
insets are `auto` (`alo-box`'s `is_out_of_flow`, `inline.rs`'s `Anchor`,
`alo-layout`'s new `out_of_flow.rs`), so each card's `Read article` link is
one piece with its screen-reader `<span>` at its end, as in Chrome. It also
found 422 (needs ADR): `taffy` throws away an `auto-fit` track list whose
track size is a `min()`, so the cards are one column. Cut: 423 (a containing
block inside the lines; waits for a page). 184 queue items are open (354
closed, 422 and 423 added). The next unused queue number is 424 and the next
unused ADR 0047.

Iteration 272 decided item 422: ADR 0047, measured in Chrome 154. Where a
grid's size in an axis is definite (by `taffy`'s own rule), a calculated
track size in a template holding an `auto-fill` or `auto-fit` repetition is
resolved against the content box and handed to `taffy` as that length, in a
copy of the style held for one grid layout call, by a new
`alo-layout/src/auto_repeat.rs` added to the gate's `taffy` boundary list;
where it is not, the template is left as written, `taffy` discards it, and
one issue per node is recorded. No code; 422 stays open, decided and
buildable next. Cut, each waiting for a page: 424 (a calculated track with
no definite basis) and 425 (`repeat(auto-fit, 1fr)` accepted when CSS makes
it invalid). 186 queue items are open (424 and 425 added open). The next
unused queue number is 426 and the next unused ADR 0048.

Iteration 273 built item 422 under ADR 0047: `alo-layout`'s new
`auto_repeat.rs` resolves a calculated track in an auto repetition against
the grid's content box where that is definite, and `arena.rs` holds the
copy for one grid layout call, so alo Sites' blog cards are two columns of
368 at x 20 and 412, as in Chrome 154; a grid measured without a definite
size is said once per grid and axis. The build found that `taffy` lays out
a content-sized grid in the width it measured, so the ADR's case G comes
out Chrome's two columns rather than today's one, and wrote that into the
ADR. Cut, each waiting for a page: 426 (an `inline-grid` holding blocks is
split as an inline box), 427 (a deep nest of grids overflows the stack;
needs an ADR) and 428 (`min()` inside a `calc` is written back as
`calcmin(`). 188 queue items are open (422 closed, 426–428 added). The
next unused queue number is 429 and the next unused ADR 0048.

Iteration 274 found every open item waiting on a page, a dependency, a
design or an ADR, and measured the last three unfrozen alo Sites goldens
against Chrome 154 given the corpus' fonts. A blog post and the catalog with
one category matched and were dropped; the full page did not, and was
frozen as `alo-sites-full-page`. It opened item 286, built: `alo-box`
breaks only an inline box (`display: inline`) around a block, so an
`inline-block`, `inline-flex` or `inline-grid` is one box holding its
blocks, and alo Sites' brand link holds its logo where Chrome puts it.
Found: 429 (a closed `<details>` lays out, draws and reads its answer;
needs ADR) and 430 (a border width is not floored to whole device pixels;
depends on nothing and is buildable next). 426's box-tree half is done; it
still waits for a page. Every alo Sites golden page has now been frozen or
measured. 189 queue items are open (286 closed, 429 and 430 added). The
next unused queue number is 431 and the next unused ADR 0048.

Iteration 275 built item 430: `alo-layout`'s `line_width.rs` snaps a
border width to whole device pixels (one to the CSS pixel until 299), and
the border reader hands layout the snapped length, so alo Sites' editorial
section starts its content at 44 as Chrome does, and the new corpus case
`border-widths` pins 4.8, 0.5, 1.5, 2 and a `calc()` in pixels. Found: 431
(border-width keywords, the initial `medium`, and percentages refused;
waits for a page). 189 queue items are open (430 closed, 431 added). The
next unused queue number is 432 and the next unused ADR 0048.

Iteration 276 decided item 429: ADR 0048, measured in Chrome 154 and
grounded in HTML-AAM. A `details` is its first `<summary>`, laid out first,
then an anonymous content box; closed, the box is present but takes no room
and its children make no boxes, so the answer is neither laid out, drawn nor
read. The summary carries `expanded` (the `details` group no longer does)
and only it has the `Summary` role; `Activate` on it toggles `open` by
HTML's activation behaviour in `alo-dom`. No code; 429 stays open, decided
and buildable next. Cut: 432 (the disclosure triangle, with 404's `inside`,
which this page now opens), 433 (a generated "Details" summary; waits for a
page) and 434 (`toggle`, `open` and `name` for script; waits for a page).
192 queue items are open (432–434 added). The next unused queue number is
435 and the next unused ADR 0049.

Iteration 277 built item 429 by ADR 0048. `alo-dom`'s new `details.rs`
answers which `<summary>` is a `<details>`' summary for every reader, and
`activation.rs` gives that summary HTML's activation behaviour: `after`
toggles `open` (it now takes the document mutably and reports
`Follows::Toggled`). `alo-box` slots a `details`' children (`details.rs`):
the summary first, then an anonymous `DetailsContent` block that holds the
rest only when open; layout lays that block out as a block. The summary
carries `expanded` and the `Summary` role, the `details` neither. The
frozen FAQ is 53.2 tall and everything under it 39.2 higher, its answer in
no reference; activating it opens it to 92.4 and closes it again. The new
corpus case `details-closed-and-open` draws the three shapes. Found: 435
(a functional colour in the `border` shorthand is torn apart at its
spaces and the border drawn in `currentColor`). 192 queue items are open
(429 closed, 435 added). The next unused queue number is 436 and the next
unused ADR 0049.

Iteration 278 built item 435: `alo-value` splits a shorthand into its
top-level parts (`parse.rs`' `components`) and `parse_border` reads those,
so a colour written `rgb(29 78 216)`, `hsl()` or `color-mix()` and a width
written `calc()` survive the `border` shorthand. alo Sites' presented cards
are ringed `rgb(23 33 43 / 0.14)`, as Chrome 154 computes them, where they
were drawn in their opaque text colour; the new corpus case
`border-colour-functions` draws each form. Found: 436 (a translucent colour
over white blends a unit off Chrome's in one channel; depends on nothing).
192 queue items are open (435 closed, 436 added). The next unused queue
number is 437 and the next unused ADR 0049.

Iteration 279 closed item 436 by measurement, with no engine change.
Chrome 154 draws translucent fills two ways. Its default raster path is the
exact source-over blend, as this engine's canvas is: the two agree on all 27
measured cells whose blend is not exactly a half. Its software path
(`--disable-gpu`) is an eight-bit blend, up to a unit darker, and is where
the earlier `222` came from. The new corpus case `translucent-fills` and
`alo-corpus`' `translucent_fills.rs` pin 36 cells over three grounds. 191
queue items are open (436 closed). The next unused queue number is 437 and
the next unused ADR 0049.

Iteration 280 found the next real page where iteration 274 pointed: the
workspace's own screens. alo Agenda's month navigator (`MiniMonth.tsx`),
rebuilt as the markup React produces with its own rules and tokens, is the
corpus case `alo-agenda-mini-month`; every box matched Chrome 154 but the
two arrow buttons, which opened and closed item 437: a `<button>` is a line
tall inside only when it holds something a line is made of
(`alo_box::Purpose::Control`'s `at_least_a_line`). Found: 438 (an empty
`<span>` and a kept space, where the shared line test differs from Chrome)
and 439 (an empty `<button>` stands on its content box's bottom in Chrome);
both wait for a page. 193 queue items are open (437 closed, 438 and 439
added). The next unused queue number is 440 and the next unused ADR 0049.

Iteration 281 froze the next workspace screen: alo Agenda's calendar list
(`AgendaModule.tsx`'s sidebar), as the corpus case `alo-agenda-calendars`.
Its rows grow by `flex: 1`, which opened and closed item 440: nothing split
or read the `flex` shorthand. `alo-css`' `flex.rs` now splits it into its
three longhands as it is written, every form measured against Chrome 154.
alo Settings, stage 1's reference screen, had the same fault unseen (Cancel
and Save at the footer's left, the content 115.5 px narrower); its widths
now match Chrome's and its reference was redrawn. Found: 441
(`text-transform`, needs ADR for what an agent reads), 442 (`text-overflow:
ellipsis`, buildable), 443 (a checkbox's user-agent margin, buildable). 196
queue items are open (440 closed, 441–443 added). The next unused queue
number is 444 and the next unused ADR 0049.

Iteration 282 decided item 441: ADR 0049, measured in Chrome 154 through
its accessibility tree. `text-transform` is applied when a text box is
built, after white space is collapsed, so layout, paint and the agent read
one text; `uppercase` and `lowercase` are Unicode's full, language-sensitive
case mapping, rented from ICU4X's `icu_casemap` behind `alo-text`'s
`case.rs`. An agent reads drawn text, names from content and from labels as
drawn, and attributes and a field's value as written. No code; 441 stays
open, decided and buildable next. Cut: 444 (`capitalize`, whose word rule
Chrome was measured not to take from UAX #29) and 445 (`full-width`,
`full-size-kana`, `math-auto`); both wait for a page. 442 and 443 remain
buildable. 198 queue items are open (444 and 445 added). The next unused
queue number is 446 and the next unused ADR 0050.

Iteration 283 built item 441 (ADR 0049, amended): `uppercase` and
`lowercase` are applied in `alo-box` as a text box is built, after white
space is collapsed, by ICU4X's `icu_casemap` behind `alo-box`'s `case.rs`
(not `alo-text`'s, which sits above the box tree; the amendment says so).
Fields, hints, a broken picture's `alt` and markers are cased by their own
styles; `::marker { text-transform: none }` is in the user-agent sheet. An
agent names a field from its `<label>` as drawn and reads a field's value
as written. alo Agenda's headings and badges, and alo's downloads badge,
match Chrome 154 to its layout unit. Cut: 446 (`aria-labelledby` as drawn,
waits for a page). 442 and 443 are buildable next and depend on nothing.
198 queue items are open (441 closed, 446 added). The next unused queue
number is 447 and the next unused ADR 0050.

Iteration 284 built item 442: `text-overflow: ellipsis`, by rules measured
in Chrome 154 (`alo-layout`'s `ellipsis.rs`). A line wider than a block
container whose `overflow` is not `visible` ends in "…" inside its content
box; whole clusters (`alo-text`'s `cluster.rs`, behind the measurer seam's
new `cluster_ends`) and whole atomic boxes are hidden, and only what is
drawn changes. alo Agenda's "Mira's calendar…" matches Chrome to its layout
unit and its pixel columns; `cases/text-overflow-ellipsis` pins the rules.
Found: 447 (an atomic box in a `nowrap` line wraps; buildable). Cut: 448 (a
"…" after a space that is its own text box; waits for a page). 443 and 447
are buildable next and depend on nothing. 199 queue items are open (442
closed, 447 and 448 added). The next unused queue number is 449 and the
next unused ADR 0050.

Iteration 285 built item 447: an atomic box in a line that may not wrap
stays on it. Whether a line breaks before an `inline-block` or a picture
is decided by the space before it, or else by the nearest box holding both
it and what came before (CSS Text 3 § 5.1, `alo-layout`'s `soft_wrap.rs`),
measured row by row against Chrome 154 in `cases/atomic-in-nowrap-line`.
Found: 449 (a break before text by the same rule, and Chrome's break after
an atomic box; buildable), 450 (a trailing space at a line's end widens its
inline box; waits for a page), 451 (an inline box opened before a break
keeps an empty piece on the line before; waits for a page). 443 and 449 are
buildable next and depend on nothing. 201 queue items are open (447 closed,
449–451 added). The next unused queue number is 452 and the next unused ADR
0050.

Iteration 286 built item 449: where two boxes meet, whether a line may break
is decided by the space before it or what came before, never by the next
box's own `white-space` (`alo-layout`'s `soft_wrap.rs`, Chrome 154's rule
measured over 31 probe rows). Between two letters there is no break; a
no-break space is no longer collapsed as white space (`alo-box`). Chrome
agrees on all fourteen rows of `cases/soft-wrap-between-boxes`. Found: 452
(a word across two boxes that does not fit is not moved back to an earlier
break; waits for a page). 443 is buildable next and depends on nothing. 201
queue items are open (449 closed, 452 added). The next unused queue number is
453 and the next unused ADR 0050.

Iteration 287 built item 443: a checkbox and a radio carry Chrome 154's
user-agent margins (a radio's differ from a checkbox's) and `box-sizing:
border-box`, and stand on the baseline by their bottom border edge
(`alo-style`'s sheet, `alo-box`'s `toggle.rs`, `alo-layout`'s
`baseline.rs`). `cases/checkbox-beside-text` agrees with Chrome across to
1/64 px; alo Settings' row is at Chrome's x and height. Found: 453 (an
author's padding and border on a checkbox, which Chrome ignores; waits for a
page) and 454 (a checkbox's and radio's own artwork; opened by Settings,
needs measuring), and 455 (an HTTP/2 body that stops is sometimes reported
as a failed write; the gate hit it once, not caused by this item). 203 queue
items are open (443 closed, 453–455 added). The next unused queue number is
456 and the next unused ADR 0050.

Iteration 288 built item 454: a checkbox and a radio draw Chrome 154's own
picture of themselves — no CSS border, a rounded square or a circle inside a
grey edge, the accent with a tick or a dot when checked, translucent greys
when disabled — every shape and colour measured from Chrome's screenshots
(`alo-paint`'s `toggle.rs`, the marks in `control.rs`, `alo-box`'s
`toggle.rs`, `alo-style`'s sheet). `cases/toggle-artwork` is compared with
Chrome's committed screenshot of it within a measured bound. Found: 456 (a
native checkbox's picture follows `aria-checked`; waits for a page), 457 (a
control's picture snapped to whole pixels as Chrome's is; opened by
`checkbox-beside-text`), 458 (a light accent's dark tick; waits for a page)
and 459 (the rasteriser under-covers a small curve, and to one side; opened
by `toggle-artwork`, needs measuring). 206 queue items are open (454 closed,
456–459 added). The next unused queue number is 460 and the next unused ADR
0050.

Iteration 289 built item 455: an HTTP/2 server that closed is reported as
"the connection ended" whether a read or a write is the first to find it
gone (`alo-net`'s `h2/frame.rs` `ended_under_a_write`, used by
`h2/client.rs`), so the gate's one-off "Broken pipe" cannot recur; a write
that stalls is still a write failing. A scripted wire forces the write-first
order in `a_download_that_stops_over_http_2.rs`. 457 and 459 are buildable
next. 205 queue items are open (455 closed). The next unused queue number is
460 and the next unused ADR 0050.

Iteration 290 built item 457: a checkbox's and a radio's picture is drawn on
whole pixels where layout puts the control at fractions, each edge of its box
rounded to the nearest pixel, a half up, as measured in Chrome 154
(`alo-paint`'s `snap.rs`, used by `build.rs`' `toggle_of`); the box itself
does not move. `checkbox-beside-text` is compared with Chrome's committed
screenshot of it within `toggle-artwork`'s bound. Found: 460 (a line as tall
as Chrome's whole-pixel ascent and descent make it; opened by
`checkbox-beside-text`, needs measuring and its rule written down) and 461 (a
plain background drawn on whole pixels; opened by the same case, needs
measuring). 459, 460 and 461 depend on nothing. 206 queue items are open (457
closed, 460–461 added). The next unused queue number is 462 and the next
unused ADR 0050.

Iteration 291 built item 459: a shape's edge pixels are sampled sixteen by
sixteen. The rented rasteriser fills it, and `alo-paint`'s `refine.rs` plans
which 16-pixel tiles an edge passes through, which `raster.rs` fills again
four times larger and averages. A small curve is within 3 of 255 of its exact
area and alike on both sides. `toggle-artwork`'s controls are nearer Chrome's
(2.47 on average where they were 3.0). Letters keep the rented sampling.
Found: 462 (letters' coverage against Chrome's; needs measuring) and 463 (a
disabled control's translucent pieces leave a seam; needs measuring). 460,
461, 462 and 463 depend on nothing. 207 queue items are open (459 closed,
462–463 added). The next unused queue number is 464 and the next unused ADR
0050.

Iteration 292 decided the rule for queue item 460, as item 464: ADR 0050,
measured in Chrome 154 on 704 probe lines in `DejaVu Sans` and five fonts
made from it that differ only in `hhea`. A line's ascent, descent and line
gap are each rounded half up to a whole pixel, and a `normal` line adds the
line gap. A set line height is quantised to 1/64 px: a number by the font
size rounded to 1/64, floored, and a length or percentage rounded. The
leading's top half is floored to a whole pixel. On a `normal` line a
fallback font adds its own rounded room. No code changed. 460 stays open,
decided and buildable next. Found: 465 (a fallback font's room on a
`normal` line; depends on 460, waits for a page). 461, 462 and 463 still
depend on nothing. 208 queue items are open (464 added and closed, 465
added). The next unused queue number is 466 and the next unused ADR 0051.

Iteration 293 built item 460, by ADR 0050. A line takes a font's ascent,
descent and line gap in whole pixels (`alo-text`'s `room.rs`), a set line
height on a grid of 1/64 px (`alo-layout`'s `line_height.rs`), and the top
half of its leading floored (`inline.rs`). `checkbox-beside-text`'s rows and
controls are Chrome's to 1/64 px. Measured against Chrome 154 with the
corpus's fonts, 835 of the corpus's 1,214 element boxes have Chrome's top and
height, where 498 did. Found: 466 (paddings, margins and grid tracks on
Chrome's 1/64 grid; needs measuring and its rule written down) and 467 (a
control's own `normal` line; depends on 460, buildable). 461, 462, 463 and
467 depend on nothing open. 209 queue items are open (460 closed, 466–467
added). The next unused queue number is 468 and the next unused ADR 0051.

Iteration 294 decided item 82 (forms), which the owner had moved to
`needs ADR` on 2026-10-11: ADR 0051, grounded in Chrome 154's behaviour
measured over DevTools on a probe page, `web-a-form` and
`alo-sites-booking`. A control's state is held apart from its attributes,
a form owner is worked out when asked for, each constraint is an `alo-dom`
rule, and a constraint not built yet refuses by name. Submission is HTML's
algorithm, ending in ADR 0020's ask with a method and an entry list that
the browser process encodes. An agent submits by activating the button,
and only a person chooses a file. Nothing is built. 82 is cut into
468–476, and 468 (the submission in the renderer, opened by
`alo-sites-booking`) depends on nothing open. 218 queue items are open (9
added). The next unused queue number is 477 and the next unused ADR 0052.

Iteration 295 built item 131, which the owner marked next on 2026-10-12:
the agent reads and drives pages nobody wrote for us, by name. On
`web-a-form` it fills every field, the textarea, a size and two toppings,
and reads each back. On `web-first-website` it follows links by their
words. The form found three defects, all fixed or refused by name: a
field's text inside its `<label>` was left out of the tree; text put into
a `<textarea>` went into an attribute nothing reads; and a `time` field
took text it never draws, so it is now refused as `Unbuilt` until 388.
388 is opened by that page. The roadmap line gains a Built clause, and its
Owed clause names 468, 470, 388, 476 and a stranger's scripted page. 220
queue items are open (131 closed). The next unused queue number is 480
and the next unused ADR 0052.

Iteration 296 built item 468, the renderer's half of a form's submission,
with its scope cut and not its depth: on a page that runs no script an
agent's press on a submit button is checked (`valueMissing` and
`typeMismatch` in Chrome 154's words, every other constraint refused by
name where it could fail), planned (Chrome's entry list, method, action
and enctype) and asked for as the page's navigation, answering
`Submitted` or `NotSubmitted`. `web-a-form` asks to `POST` httpbin with
Chrome's entries. The browser process refuses every submission by name
until 470, and the scripted half, which `alo-sites-booking` needs, is cut
into 484 (depends on nothing open). 225 queue items are open (468 closed,
484 added). The next unused queue number is 485 and the next unused ADR
0052.

Iteration 297 built item 470, the browser process's half of a form's
submission: the entries are written as Chrome 154 writes them (a `GET`'s
query replacing the action's, an urlencoded `POST` body with no charset),
and decided as a link is plus the header policy's `form-action`, a secure
page's form to `http` refused, and the jar keeping a `SameSite=Lax` cookie
off a cross-site `POST`. `web-a-form` and the booking markup are decided
through real tabs with Chrome's bytes. A `<meta>` policy's `form-action`
and reports of a violation are cut into 486. The scripted half of 468,
numbered 484 by iteration 296, is renumbered **485**, because 2dfc8e1 had
already given 484 to `@browser`. 225 queue items are open (470 closed, 486
added). The next unused queue number is 487 and the next unused ADR 0053.

Iteration 298 built item 485, a form's submission on a page that runs
script, for an agent's press: inside the click's task, a trusted `invalid`
at each failing control in tree order and nothing more, or a trusted
`SubmitEvent` naming its submitter, which a listener may cancel; HTML's
*firing submission events* guard in the document cell; and the ask built
after the listeners. The frozen `alo-sites-booking`, script and all, is
checked and asked for as Chrome does it, and decided through real tabs. A
form this engine cannot check is refused before the click is queued. A
script's own `click()` on a submit button is cut into **487** (depends on
nothing open). Found: `alo-sites-contact-form`'s analytics script stops the
page as it loads (queue item 73), so its `submit` listener never hears the
press, which 472 now notes. 225 queue items are open (485 closed, 487
added). The next unused queue number is 488 and the next unused ADR 0053.

Iteration 299 built item 488, cut from 73: a property of a string. A
string's own `length` and units, then `String.prototype` — a `String`
object of the empty string — with `toString`, `valueOf` and `slice`, read,
written and deleted as the wrapper would answer, without making one.
`alo-sites-contact-form` and `alo-sites-full-page` now run alo Sites'
analytics script to its end, past `slice(3)`: the contact form reports
that its form was seen, its first edit, and its own `submit` listener hears
the press and stops on `URLSearchParams` (472), so nothing is sent where
before the form was sent as a scriptless page's. `ToObject` of a primitive
is cut into **489** and a string's iterator into **490**. 226 queue items
are open (488 closed, 489 and 490 added). The next unused queue number is
491 and the next unused ADR 0053.

Iteration 300 built item 472: `FormData` and `URLSearchParams`. Measured
first in Chrome 154 over DevTools on the frozen contact form at its address.
`FormData` takes its list from the function a submission uses and iterates
as pairs; `URLSearchParams` reads a string, a record, or a sequence walked
by the iterator protocol one call at a time; a `URLSearchParams` is a fetch's
or a beacon's body with Chrome's type. `className` and `replaceWith` came
with them. `alo-sites-contact-form`, the first case to freeze a fetch's
answer (rebuilt from alo Sites' source), now posts Chrome's exact body when
an agent presses Send, and puts its thanks where the form was, boxed where
Chrome boxes it (`sent.png`). The `formdata` event is cut into **491** and a
`FormData` body into **492**; offline, the page's `form.submit()` fallback
waits on 471. 227 queue items are open (472 closed, 491 and 492 added). The
next unused queue number is 493 and the next unused ADR 0053.

Iteration 301 built item 476: the agent tree reads a control's validity.
Measured first in Chrome 154's accessibility tree over DevTools, on the
frozen booking and contact pages and on probe pages: a control is `invalid`
from load and a press changes nothing; an empty required text field is
never `invalid` for being empty; `aria-invalid` wins; the tree carries no
message. ADR 0051 gains an amendment saying so. `alo-box`' `invalid.rs`
reads it from `constraint.rs`, and a constraint not built reads
`invalid=unknown`. The booking page's day field now reads
`[required invalid]`. 226 queue items are open (476 closed). The next
unused queue number is 493 and the next unused ADR 0053.

Iteration 302 decided item 83's half that was left in ADR 0053:
`XMLHttpRequest`. Measured first in Chrome 154 over DevTools, intercepted,
and against a server on `127.0.0.1` for the upload events interception
suppresses. It makes exactly `fetch`'s ask with nothing new on the wire; its
answer is one task firing Chrome's events as the browser's dispatch; the
document holds a request while its answer is owed; `abort()` withdraws an
unsent ask; a synchronous `open` throws `InvalidAccessError`. No code was
written. 336 now carries its contract and closing condition and still waits
for a page; **493** (`timeout`), **494** (response types beyond text) and
**495** (cancelling a request that has left, needs ADR) are added. 229 queue
items are open (493–495 added). The next unused queue number is 496 and the
next unused ADR 0054.

Iteration 303 decided the rule for queue item 466, as item 498: ADR 0054,
measured in Chrome 154 on three probe pages and on 466's two corpus cases
over DevTools. Layout is on a grid of 1/64 px. A length, a percentage
(after it is resolved against a basis already on the grid) and a `calc()`
(as a whole) are each truncated toward zero onto it, and a run of text's
width is rounded up. Centring, auto margins and grid distribution truncate
each share. A grid's flexible tracks have their ideal edges floored, and a
line of flexible items fills its container exactly. It is built at the
boundary with `taffy`: `LayoutStyle`, the arena's `resolve_calc_value`
with every percentage handed over as a calculation, and the edges `taffy`
hands back, truncated. No code changed. 466 stays open, decided and
buildable next. Found: **499** (needs ADR: `taffy`'s flexible shares on the
grid, and the grid's second column pass, which makes every column of
`alo-agenda-mini-month` 1,838 units in Chrome). 233 queue items are open
(498 added and closed, 499 added). The next unused queue number is 500 and
the next unused ADR 0056: the owner's ADR on the agent's socket, committed
as a second 0053, is being renumbered 0055.

Iteration 304 built queue item 496 on ADR 0055, the door an agent outside
the browser comes in by. `alo-window` holds a Unix socket at a private
place, reads one bounded JSON line at a time through `protocol.rs`, the one
file that names `serde_json`, and answers on the conductor's thread from
the tabs. A tab is named by its address. Every answer and refusal is a
kind, and the browser process records what each agent read and changed.
`alo-ask`, a separate process, reads, fills and sends the frozen pizza form
over it in `an_agent_outside_the_browser.rs`. The contract is
`docs/contracts/agent-socket.md`. Found: **500** (a box's id does not
survive a verb that adds a box). 233 queue items are open (496 closed, 500
added). The next unused queue number is 501 and the next unused ADR 0056.

Iteration 305 built queue item 466 on ADR 0054: every box is on Chrome's
grid of 1/64 px. `alo-layout`'s `layout_unit.rs` holds the grid and its
rules. Lengths are truncated where `engine.rs` turns them into numbers.
Percentages are handed to `taffy` as calculations, and the arena truncates
each one it resolves. Text widths are rounded up. Each box's edges are
placed as the tree is read back, against where its parent was placed. The
ADR gained an addendum with three corrections: where edges are placed, a
float's slack for a length but not for a shared edge, and a grid's
`justify-content` being Chrome's only for its first share. That last one
widens 499. Against Chrome 154, 707 of 1,260 corpus boxes are now exact,
where 514 were. `alo-sites-booking` is Chrome's except what its date
field's line moves (467). The mini-month's columns wait for 499. 232 queue
items are open (466 closed). The next unused queue number is 501 and the
next unused ADR 0056.

Iteration 306 decided queue item 499: ADR 0056. This engine does not take
the sharing of flexible space over from `taffy`, which would mean owning
grid track sizing and flexbox's flexible lengths for 1/64 px. A fork, a
`[patch]` and a vendored copy stay refused. The fix is a hook on `taffy`'s
tree trait, with a default that changes nothing, called at three places
where `taffy` fixes a share. A throwaway prototype in the scratchpad tested
it. With the snap and `taffy`'s re-run check refreshing every item,
`alo-agenda-mini-month`'s columns are every one 1,838 units, as Chrome's
are. Either change alone is not enough. Upstream's #1227 (merged
2026-10-04, unreleased) already fixes the re-run check. No engine code
changed. Found: **501** (a person offers the hook upstream; blocked: a
person), **502** (building on it; depends on 501 and a release) and
**503** (flex `justify-content`'s rule; needs measuring). 234 queue items
are open (499 closed, 501–503 added). The next unused queue number is 504
and the next unused ADR 0057.

Iteration 307 built queue item 497 on ADR 0055 § 8. A person keeps every
agent outside the browser to the sites they name with `alo --agent-site
ADDRESS` (`agent_sites.rs`, `opening.rs`). Any other site is refused as
`NotGranted` with its site, before any tab is looked at (`serving.rs`). The
list of tabs leaves out what an agent may not reach, and every record entry
names the site. A tab is judged by the page it shows, so a redirect or a
page that navigated itself is refused where it ended. It is ours, a refusal
and never a grant, and it only narrows. `an_agent_kept_to_a_site.rs` proves
it from a separate process and over real tabs. Found: **504** (sites per
agent rather than for every agent; needs ADR). 234 queue items are open
(497 closed, 504 added). The next unused queue number is 505 and the next
unused ADR 0057.

Iteration 308 built queue item 467. A form control's line is the line its
text would be, as Chrome's is: the line height it set, or with `normal`
its font's whole-pixel ascent, descent and line gap (ADR 0050), where it
was 1.2 em (`alo-layout`'s `inline::empty_line_height`, which
`control_line` asks for). `a-filled-form` and alo Settings are now
Chrome's box for box. Across the corpus, 855 of 1,260 boxes are Chrome's
exactly, where 807 were, measured in standards mode: iteration 305's 707
put Chrome in quirks mode for pages with no `</head>`. A button is still
2 px shorter than Chrome's, because Chrome's border is 2 px and drawn as
its own picture. Found: **505** (that border and picture), **506** (a
control's own 13.33 px font) and **507** (a date field's 2 px taller
line). 236 queue items are open (467 closed, 505–507 added). The next
unused queue number is 508 and the next unused ADR 0057.

Iteration 309 built queue item 505. A button and a text field take
Chrome 154's 2 px `outset` and `inset` borders, and while the page writes
no border or background of its own they draw Chrome's picture in that
room: a one-pixel `#767676` edge rounded 2.5 px over a `#efefef` or white
fill, translucent when disabled. It was measured at 8× (`alo-box` and
`alo-paint` `face.rs`). The cascade now keeps which properties the page
wrote (`ComputedStyle::authored`), because Chrome drops the picture even
for a page's `background: #efefef`. `a-script-hears-an-event` is Chrome's
box for box, and its button's edge matches Chrome's committed screenshot
within 7. `web-a-form`'s 16 px fields moved 2 px further from Chrome's
until 506 gives a control its own font. Found: **508** (black's two
tones), **509** (a disabled control's computed colours) and **510** (a
`<select>`'s and a `<textarea>`'s pictures). 238 queue items are open
(505 closed, 508–510 added). The next unused queue number is 511 and the
next unused ADR 0057.

Iteration 310 built queue item 506. Every form control is set in Chrome
154's own font, 13.33 px, weight 400, upright, with a `normal` line, in
Arial (the machine's `sans-serif` where it has none) or `monospace` for a
textarea and the five date and time fields. `font` as a CSS-wide keyword
alone is now split into its longhands (`alo-css` `font.rs`), so an alo
screen's `font: inherit` still takes the page's font back. `web-a-form`'s
fields are Chrome's 21 and every box down to its fieldsets is Chrome's; of
1,261 corpus boxes 886 are Chrome's exactly, where 865 were, and none
moved away. Found: **511** (a field's width by Chrome's rule, not `20ch`),
**512** (the rest of a control's text reset), **513** (the `font`
shorthand's grammar) and **514** (a unit of text width at a fractional
size). 241 queue items are open (506 closed, 511–514 added). The next
unused queue number is 515 and the next unused ADR 0057.

Iteration 311 built queue item 507. A `date`, `month`, `week`, `time` or
`datetime-local` field holds its line in segments with a pixel above and
below, as Chrome 154's edit does (`alo-box`'s `in_segments`, `alo-layout`'s
`SEGMENT_EDGE`), and takes Chrome's `padding: 0 0 0 1px`.
`alo-sites-booking`'s every box is Chrome's; of 1,261 corpus boxes 892 are
Chrome's exactly, where 886 were, and none moved away. Found: **515** (the
calendar indicator's height as a floor, needs design against ADR 0042 §
5), **516** (a text field's line never below `normal`) and **517** (a date
or time field's own width). 243 queue items are open (507 closed, 515–517
added). The next unused queue number is 518 and the next unused ADR 0057.

Iteration 312 built queue item 508. The two tones of an `inset`, `outset`,
`groove` and `ridge` border are Chrome 154's, measured over every grey and
650 colours with not one channel off (`alo-paint`'s `tone.rs`): 0.33 off or
on the brightest channel with the hue kept, each channel truncated to a
byte; a colour whose luminance is no more than `rgb(32, 32, 32)`'s drawn
lighter instead, so black is 84 and 168; and one brighter than
`rgb(235, 235, 235)` its own lighter tone. Corpus case
`a-button-with-its-own-background` is Chrome's screenshot on every side to
the level. Found: **518** (a mitre's diagonal pixel) and **519** (a
button-like `<input>`'s value is not drawn). 244 queue items are open (508
closed, 518–519 added). The next unused queue number is 520 and the next
unused ADR 0057.

Iteration 313 built queue item 509. A disabled control's colours are
Chrome 154's computed ones, measured for every kind of control: a field's
text `rgb(84, 84, 84)`, a button's `rgba(16, 16, 16, 0.3)`, both tinted
`rgba(239, 239, 239, 0.3)` and edged `rgba(118, 118, 118, 0.3)`, a list
grey and at 0.7 opacity, a fieldset unchanged (`alo-style`'s user-agent
sheet). The cascade now resets a longhand that a whole `background` or
`border` beat (`cascade.rs`'s `reset_by_shorthands`), so a page's
background on a disabled control beats the sheet's `background-color`.
Corpus case `a-disabled-control` matches Chrome's screenshot: each text's
darkest pixel to a level, each fill exactly. Found: **520** (a
fieldset's groove is Chrome's `rgb(239, 239, 239)`). 244 queue items are
open (509 closed, 520 added). The next unused queue number is 521 and the
next unused ADR 0057.

Iteration 314 built queue item 510, with the `<select>` cut to 521 and the
grip to 522. A `<textarea>` draws a field's picture (`alo-box`'s
`face::of`), which Chrome 154 draws in it pixel for pixel, live, disabled
and read-only, measured at one and eight device pixels to one. A page's
border or background takes it away, as a field's. `web-a-form`'s textarea
is Chrome's committed screenshot exactly along its sides and within 9 in
its corners (43 and 107 before), and new corpus case `a-text-area`
matches Chrome's to the same bounds as a field of its size. Found:
**523** (a textarea's `cols` and `rows`), **524** (an edge's inner corner
pixel) and **525** (a field whose page writes only a border is still
white). 248 queue items are open (510 closed, 521–525 added). The next
unused queue number is 526 and the next unused ADR 0057.

Iteration 315 built queue item 511. A text field is as wide as its `size`
says, twenty when it says nothing usable and always for a `number` field,
counted as Chrome 154 counts it: the face's `x`, rounded to a whole pixel
only when that makes it wider, `size` times, and the face's whole-pixel
ascent less one `x` (`alo-box`'s `field_size.rs`, `alo-layout`'s
`field_width.rs`). Measured over 3,000 fields in three faces, of which it
gives 2,997, and confirmed on 288 more in faces and sizes it was not
fitted to. `web-a-form`'s fields are Chrome's 172 and its labels Chrome's
to the unit, and new corpus case `a-text-field` is Chrome's box for box
and within 9 levels of Chrome's screenshot. Found: **526** (a face with
no `x`), and a lead on 514: Chrome seems to set a face at its size cut to
hundredths. 248 queue items are open (511 closed, 526 added). The next
unused queue number is 527 and the next unused ADR 0057.

Iteration 316 closed queue item 296, the window. The screen was unlocked,
so its one owed clause could be met. `alo --frozen-fonts` on the frozen
`alo-offline` page was captured with `screencapture -l` and compared with
`compose` of the same page at the window's size and scale. The frame came
from the real conductor and `alo-render`. Once the composition is put
through the display's colour profile, every opaque pixel is within one
level and 99.86% are exact. The only others are the translucent rounded
corners the window server adds. 297, the tab strip, is now eligible.
247 queue items are open (296 closed). The next unused queue number is
527 and the next unused ADR 0057.

Iteration 317 built queue item 514. A face is set at its font size cut to
hundredths in a 32-bit float, as Chrome 154 sets it, so `13.333333px`, the
size of every control, is 13.33 and `8.11px` is 8.1 (`alo-text`'s
`face_size.rs`). Advances, a line's room, `ch`, `ex` and a field's width
follow; `em` does not. Of 2,922 runs measured in three faces at 128 sizes,
2,916 are Chrome's width and all are Chrome's height. `web-a-form`'s
button is Chrome's 6,597 units, and every corpus number that moved is now
Chrome's. Found: **527** (six runs whose sums end a hair from a grid
line). 247 queue items are open (514 closed, 527 added). The next unused
queue number is 528 and the next unused ADR 0057.

Iteration 318 built queue item 297. The tab strip is a document the engine
renders. `ToRenderer::Strip` carries the tabs to a confined renderer at
`alo:tab-strip`, an opaque site no page can name, as data. That renderer
builds the strip from a compiled-in template with `alo-dom`'s operations,
each title a text node, and draws it with a stylesheet after alo's tokens.
`alo-renderer`'s `tests/the_tab_strip.rs` pins:
- three reference renders;
- every tab's box in numbers;
- the agent reading four tabs with the second selected;
- five hostile titles, kept as text in the tree and in the render.

A title is drawn to 1,024 characters, because 100,000 took 15 s. Cut:
**528** (the strip in the window, with page titles) and **529** (the
browser's keys). Found: **530** (a flex item's name runs into its
neighbour's). 249 queue items are open (297 closed, 528–530 added). The
next unused queue number is 531 and the next unused ADR 0057.

Iteration 319 built queue item 528. The tab strip is in the window: a
36 px band across the top, the selected tab below it. `alo-dom`'s
`title.rs` reads a document's title as HTML does, and a load's answer
carries it, bounded on the wire. `Tabs` keeps a selected tab, each tab's
title and the strip's renderer (`tab/strip_of.rs`), and the conductor
redraws the strip when it changes and as a load starts. Pinned:
- three new window reference renders, the four earlier ones rewritten
  with the band;
- a hostile and a 100,000-character title through the real renderers;
- a strip renderer stopped with `kill -STOP` leaving the window composing
  and the tabs working;
- `alo` captured on this Mac, every composed colour one captured colour.

Cut: **531** (the agent reading the strip, which needs design: the strip
shows every site's titles, and an agent kept to some sites must not see
them) and choosing a tab, into **529**. Found: **532** (a title changed
after load). 250 queue items are open (528 closed, 531 and 532 added). The
next unused queue number is 533 and the next unused ADR 0057.

Iteration 320 built queue item 529. The browser's own keys: ⌘T, ⌘W,
⌃Tab and ⌃⇧Tab, read by the window's event loop (`keys.rs`, the one place
a press is given a meaning) and carried out by the conductor
(`conductor/choosing.rs`). Choosing a tab — by a key, by opening one or by
closing the selected one — tells the tab left `hidden` before the tab
chosen `visible` (ADR 0039 § 1), and the window is told the tab chosen
replaces what it shows. Closing the last tab closes the window. Pinned:
- every key through the real conductor and renderers, each strip compared
  with the same strip drawn in-process;
- the order the two pages are told, from their own fetches;
- `a-new-tab.png`, a new tab chosen over a page;
- each key pressed in `alo` on this Mac and the window captured.

Found: **533** (an IPv6 literal host is looked up with its brackets, so no
`http://[::1]/` request is ever made). 250 queue items are open (529 closed,
533 added). The next unused queue number is 534 and the next unused ADR
0057.

Iteration 321 built queue item 530. A name read from what is inside a
thing has its spaces where Chrome 154's accessibility tree has them: a
space at both edges of every box that is not an inline box in its parent's
lines — a flex or grid item however it was written, an `inline-block`,
`inline-flex` or `inline-grid` — measured over 39 arrangements and pinned
by `alo-agent/tests/a_name_across_its_items.rs`. The tab strip's own
workaround for it is gone. Found: **534** (a `<br>`, and a picture drawn
as its `alt`, run into their neighbours in a name). 250 queue items are
open (530 closed, 534 added). The next unused queue number is 535 and the
next unused ADR 0057.

Iteration 322 built queue item 298 at its first piece: which element a
person's pointer is over, as Chrome 154 finds it. The display list records
each box's area where it paints (`DisplayItem::Area`, drawn as nothing);
`alo-paint`'s `hit.rs` finds the box drawn last over a point's pixel
through the list's transforms and clips, and `alo-renderer`'s `hit.rs`
turns it into an element. Pinned by 21 pages put to Chrome's
`elementFromPoint` (`where_a_pointer_falls.rs`), with each box the points
depend on in numbers. Cut: **535** (a person's click reaching that
element) and **539** (a click on the strip). Found: **536** (Chrome finds
a positioned clip's content under its square border), **537** (a space
between two inline boxes is 0.023 px narrow), **538** (a positioned box
inside a positioned one is drawn under it and outside its clip) and
**540** (an `<svg>`'s shapes are not found one by one) and **541** (a
list marker is found where Chrome finds only its bullet). 256 queue items
are open (298 closed, 535–541 added). The next unused queue number is 542
and the next unused ADR 0057.

Iteration 323 built queue item 535: a person's main button reaches the
page. The window keeps the cursor and aims a press from its own placement
(`alo-window`'s `aim.rs`); the conductor sends it to the selected tab and
its release to the tab the press went to (`conductor/pointing.rs`), as
`ToRenderer::Pointer`, the one message that carries a point; the renderer
hit-tests it and fires what Chrome 154 was measured to fire, as two tasks
(`event_loop/pointed.rs`), with HTML's activation steps around the click.
A link a person clicks is decided as `Cause::Person` and said in the
window. Cut: **542** (movement, hover and boundary events), **543** (a
person's click submitting a form), **544** (other buttons), **545** (a
click with a modifier held), **546** (a pointing device's other
`PointerEvent` members), **547** (click counts) and **548** (a navigation
refused in the window is not recorded). 262 queue items are open (535
closed, 542–548 added). The next unused queue number is 549 and the next
unused ADR 0057.

Iteration 324 built queue item 542: a person's pointer moving over a page.
The window sends a move over the page, its leaving once, and every move of
a drag the page pressed (`alo-window`'s `cursor.rs`); the conductor
coalesces moves that arrived together. The renderer keeps the node last
under the pointer and fires what Chrome 154 was measured to fire as one
task (`crossing.rs`, `event_loop/crossed.rs`), with `relatedTarget` on the
events (`alo-bindings`). Cut: **550** (`:hover` drawn), **551**
(`pointerrawupdate`), **552** (a page changing under a still pointer) and
**553** (a tab chosen under the pointer). 266 queue items are open (542
closed, 550–553 added; 549 was the owner's). The next unused queue number
is 554 and the next unused ADR 0057.

Iteration 325 built queue item 550: `:hover` under a person's pointer. The
document keeps the element the pointer is over (`alo-dom`'s `hover.rs`),
which the renderer sets before the crossing to it fires and which moves to
the parent an element was taken out of, as Chrome 154 was measured to;
`alo-css` matches `:hover` on it and its ancestors, so the page is drawn
again at each crossing and a script's `querySelectorAll(':hover')` agrees.
Cut: **554** (a `<label>` hovered hovers the control it labels). 266 queue
items are open (550 closed, 554 added). The next unused queue number is 555
and the next unused ADR 0057.

Iteration 326 built queue item 554: a `<label>` hovered hovers the control
it labels. Which control a label labels is HTML's rule, held once in
`alo-dom`'s `label.rs` and asked by the agent's names and by the hover;
the document keeps each hovered label with the control it hovered, as
Chrome 154 was measured to, except that the control's hover ends with the
label's where Chrome's is left stuck. Cut: **555** (`Element.id`, and a
label's `htmlFor` and `control`). 266 queue items are open (554 closed,
555 added). The next unused queue number is 556 and the next unused ADR
0057.

Iteration 327 built queue item 543: a person's click on a submit button
sends its form. The click's task submits it as an agent's `Activate` does,
and a page nobody listens on submits it by `alo-dom`'s rules alone
(`alo-renderer`'s `sent.rs`); a person's click is never refused, so a form
not sent is said. The decided navigation now says whether a form asked for
it, and the window says *this form is sent by …*. Cut: **556** (an image
button's point) and **557** (the person told why a form was not sent). 267
queue items are open (543 closed, 556 and 557 added). The next unused queue
number is 558 and the next unused ADR 0057.

Iteration 328 built queue item 548: a navigation refused in the window is
recorded. The conductor takes where a page asked to go after every answer
that can carry it, not only a person's click, and each refusal is a line
in the session's record written by the network thread in its turn, as a
refused fetch's is. Nothing was cut. 266 queue items are open (548
closed). The next unused queue number is 558 and the next unused ADR 0057.

Iteration 329 built queue item 539: a press on a tab in the strip chooses
it. The window sends a press of the main button over the band as
`Order::StripPressed`; the strip's renderer claims the tab under it by its
place (`alo-renderer`'s `strip_claim.rs`), and the conductor decides the
claim against the tabs the strip showed when it last painted
(`strip_decide.rs`), refusing by name a place not shown, a tab closed
since and a claim with no press behind it. A tab chosen so is chosen as a
key chooses one. Cut: **558** (the strip's close and new-tab controls).
Found: **559** (a body's background does not paint the canvas). 267 queue
items are open (539 closed, 558 and 559 added). The next unused queue
number is 560 and the next unused ADR 0057.

Iteration 330 decided queue item 299 as ADR 0057, its own iteration,
because a measurement contradicted the item. Chrome 154 started with a
real scale of two lays out 744 of the corpus's 1,761 boxes differently
from scale 1: every rule this engine rounds to whole pixels or to the
1/64 px grid rounds in device pixels there. DevTools' emulated scale does
not show it. So a page is laid out for its screen's scale and reported in
CSS pixels, and 299 is cut into **560** (layout for a scale), **561**
(paint at a scale) and **562** (the window at the screen's scale). 270
queue items are open (560–562 added; 299 stays open until they close).
The next unused queue number is 563 and the next unused ADR 0058.

Iteration 331 built queue item 560, layout for a scale, without closing it.
The scale rides on `Viewport` and `MediaContext` and every whole-pixel rule
in ADR 0057 § 2 rounds in device pixels; a text field's width and the size
a face is set at were measured to be device-pixel rules too and added to
the ADR's table. Against Chrome 154 under a real scale of two, 578 of the
580 corpus boxes exact at one are exact at two, and nothing at one moved.
Cut: **563** (a loaded page laid out at its scale). Found: **564** (a
shrunk flex item's share of a pixel). 560 stays open on 563, 564 and 502;
561 now depends on 560's built layout. 272 queue items are open (563 and
564 added). The next unused queue number is 565 and the next unused ADR
0058.

Iteration 332 closed queue item 561, paint at a scale. A display list says
the scale it was built for and is drawn under it onto a canvas of device
pixels; a control's picture is snapped to whole device pixels, and a run's
letters are shaped and outlined at the size their face is set at on that
screen. `checkbox-beside-text` at two puts all nine controls on Chrome
154's device pixels, with `render-at-2.png` committed beside `render.png`,
and nothing at one moved. Cut: **565** (a plain background on whole device
pixels, which this engine never snapped at one either). Found: **566** (a
line under text at a scale). 273 queue items are open (561 closed; 565 and
566 added). The next unused queue number is 567 and the next unused ADR
0058.

Iteration 333 closed queue item 563, a loaded page laid out at its scale.
`Page` carries the screen's scale, on the wire too (a scale that is not a
number refuses the message; one not above zero is read as one), and a
renderer lays its page out and paints it at that scale for the page's life.
The 21 corpus cases whose page runs script put all 313 boxes exact at one
exactly where Chrome 154 puts them at a real scale of two, and nothing at
one moved. 560 stays open on 564 and 502; 562 now has both dependencies
done. 272 queue items are open (563 closed). The next unused queue number
is 567 and the next unused ADR 0058.

Iteration 334 closed queue item 562, the window at its screen's scale, cut
to its window half. A `Resize` carries the screen's scale, and the window
tells the page's renderer and the strip's its whole scale; both paint in
device pixels and the window places their frames one to one, replicating
nothing, so text on this Mac's Retina display is drawn at its resolution.
`alo` captured there is its composition at two in every opaque pixel within
one level. Cut: **567** (`devicePixelRatio` and `resolution`), **568**
(where a pointer lands at two) and **569** (a window wider than a page's
picture can be). 299 stays open on 560 and 567. 274 queue items are open
(562 closed; 567–569 added). The next unused queue number is 570 and the
next unused ADR 0058.

Iteration 335 closed queue item 567, `devicePixelRatio` and the
`resolution` media features. A page reads the scale it is drawn at, and its
sheets match `resolution`, `min-resolution` and `max-resolution` in `dppx`,
`x`, `dpi` and `dpcm` exactly as Chrome 154 answered 2,984 measured queries
at one and two; `cases/a-page-told-its-scale` is Chrome's in every box at
both scales. Found: **570** (`matchMedia`) and **571** (a media query's
range syntax, boolean form and `calc()`), each waiting for a frozen page.
299 stays open on 560 alone. 275 queue items are open (567 closed; 570 and
571 added). The next unused queue number is 572 and the next unused ADR
0058.

Iteration 336 measured queue item 564, a shrunk flex item's share of a
pixel, and did not close it. Chrome 154 at scales of one and two was
measured on 5,120 flex lines, and one rule gives every one: ADR 0054 § 3's
last-to-first rounding, half away from zero, holds for shrinking with the
shrink factor scaled by the inner base size, the share taken as a ratio,
and factors under one sharing their part of the free space truncated
(ADR 0054's addendum of 2026-10-13). 762 lines are frozen and checked in
`alo-layout`. Building the rule is ADR 0056's hook, so 564 now depends on
502, and 560 waits on 502 alone. Found: **572**, `taffy` takes a line's
gaps away twice when its factors sum below one, blocked on a person to
report it. 276 queue items are open (572 added). The next unused queue
number is 573 and the next unused ADR 0058.

Iteration 337 closed queue item 565: a box's own area is drawn on whole
device pixels. Chrome 154 at scales of one and two was measured on 1,536
boxes at tenths of a pixel, and everything a box paints for its own area —
a colour, a gradient, a border on both edges, a rounded corner, an inline
box's piece, spread, inset and blurred shadows — is drawn from its border
box with each edge snapped, a shadow's offset added after and a transform
moving the snapped box (ADR 0057, *Measured since*). `alo-paint` now
draws so, and a fieldset's border from the line through its legend;
`cases/backgrounds-on-device-pixels` is Chrome's along every box's middle
at both scales. 34 corpus cases' display lists moved at one, each line a
box onto whole pixels. Found: **573** (a spread shadow's square corners)
and **574** (a run of one `&nbsp;` off the grid); cut: **575** (a clip, a
picture's box, an `<svg>`'s viewport, a fractional scale). 461 waits only
on its closing check. 278 queue items are open (565 closed; 573–575
added). The next unused queue number is 576 and the next unused ADR 0058.

Iteration 338 closed queue item 461 on its closing check: every pixel of
`checkbox-beside-text` outside its controls and text, 49,525 of 60,000, is
Chrome 154's screenshot exactly, held by `a_background_beside_controls.rs`.
Nothing in the engine changed; 460 and 565 had built what it asked. 277
queue items are open (461 closed). The next unused queue number is 576
and the next unused ADR 0058.

Iteration 339 closed queue item 462: letters are sampled sixteen by
sixteen where an edge is, as every shape has been since 459. Measured
against Chrome 154's screenshots of `DejaVu Sans` at 12, 13 and 16 px
(`cases/letters-at-three-sizes`, as Chrome draws it and with
`-webkit-font-smoothing: antialiased`), every line is nearer than with the
rented four-by-four sampling, and `letters_against_chromes.rs` holds each
size to its measured bound. 73 reference pictures moved, every changed
pixel at a letter's edge. Found: **576** (Chrome on macOS thickens every
letter, an eighth more ink; a decision, since it is one platform's) and
**577** (Chrome pulls a letter's top up to a whole pixel; needs
measuring). 278 queue items are open (462 closed; 576–577 added). The next
unused queue number is 578 and the next unused ADR 0058.

Iteration 340 built queue item 463 without closing it: a disabled
checkbox's or radio's edge and inside are drawn as one picture faded
once (`alo-paint`'s `toggle.rs`, `Artwork::joined`), because Chrome 154's
seam, measured at eight device pixels to one, is the two pieces mixed by
coverage. `toggle-artwork`'s disabled radio over black is 2.96 from
Chrome's on average where it was 9.22, 24 at worst where it was 44. The
rest is Chrome's ring at one device pixel to one, thinner on its
diagonal than at eight: cut to **578**, on which 463 now depends. 279
queue items are open (578 added). The next unused queue number is 579
and the next unused ADR 0058.

Iteration 341 closed queue item 578: Chrome 154 draws a disabled checkbox
or radio as an inside a fifth of a pixel short of its edge with
`#767676` at 77/255 over it as edge, checked fill or dot, fitted at one,
two and eight device pixels to one (`alo-paint`'s `toggle.rs`; the
joined picture and its group are gone, and `face.rs` shares the exact
edge colour). Over black `toggle-artwork`'s worst disabled pixel is 15
where it was 24, a radio's 10. 463's own check was run and is not met by
Chrome's one-to-one coverage quantising, which this engine does not
reproduce; it waits on a person. 278 queue items are open (578 closed).
The next unused queue number is 579 and the next unused ADR 0058.

Iteration 342 closed queue item 566: a line under, over or through text
is drawn where Chrome 154 draws it, at one and at two. Measured on 480
lines at twenty sizes and four offsets at both scales, Chrome puts every
one on whole device rows, `max(1, size·s/10)` thick cut down, from the
text's top rounded to a device pixel, ignoring the face's underline
figures (ADR 0057, *Measured since*); `alo-paint`'s `decoration.rs` draws
that rule, and `text-decorations` is on Chrome's rows at one and at two
(`chrome.png`, `chrome-at-2.png`, `render-at-2.png`). The case at one
moved too, since the rule is Chrome's at one as well. Found: **579**
(an underline's gap around a descender) and **580** (the space before a
decorated span's child). 279 queue items are open (566 closed; 579–580
added). The next unused queue number is 581 and the next unused ADR 0058.

Iteration 343 closed queue item 579: an underline and an overline step
aside where a letter's ink crosses them, as Chrome 154's do
(`text-decoration-skip-ink: auto`). Measured with a red line under blue
letters at 22 sizes and both scales, Chrome's gaps are whole device
pixels: the band from half a pixel below the line's top to `t − ½`, a
glyph's ink in it counting its control points, widened by the unrounded
thickness `t` each side and rounded. `alo-paint`'s `skip_ink.rs` draws
that rule, and `text-decorations` is Chrome's piece by piece at one and
at two but for 580's space. Found: **581** (`text-decoration-skip-ink`'s
values are not read). 279 queue items are open (579 closed; 581 added).
The next unused queue number is 582 and the next unused ADR 0058.

Iteration 344 closed queue item 580: a decoration runs under a space its
line goes on past and not under one that ends the line, as Chrome 154's
does. `alo-layout`'s `Fragment` carries the width of the space a piece of
text ends in, zero where the space hangs at the line's end; `alo-paint`
draws a piece's lines over it. `text-decorations` is Chrome's piece by
piece at one and at two with no exception left in its test. Found:
**582** (a space that is a text box of its own is still bare). 279 queue
items are open (580 closed; 582 added). The next unused queue number is
583 and the next unused ADR 0058.

Iteration 345 closed queue item 582: a decoration runs under a space that
is a text box of its own, or the first thing in one, where its line goes
on past it, as Chrome 154's does. `alo-layout`'s `LineBox::spaces` records
such a space with the room it takes, never as a fragment, so no box moves;
an ellipsis hides the ones where what it keeps ends; `alo-paint` draws a
text box's decorations over them. `decorated-spaces` is Chrome's piece by
piece at one and at two. Found: **583** (before an ellipsis, a kept piece
of text's own last space is underlined, where Chrome's is not). 279 queue
items are open (582 closed; 583 added). The next unused queue number is
584 and the next unused ADR 0058.

Iteration 346 closed queue item 583: before an ellipsis a decoration stops
at the last letter kept, as Chrome 154's does. `alo-layout`'s `Cut::width`
is the kept letters without the white space they end in, a piece kept
whole and last before the cut is cut there too, and a cut is found by the
piece whose text holds its end. `text-overflow-ellipsis`' underline is
Chrome's in a frozen `chrome.png`. 278 queue items are open (583 closed;
none added). The next unused queue number is 584 and the next unused ADR
0058.

Iteration 347 closed queue item 581: `text-decoration-skip-ink` is read
as the text's own, inherited value — `none` draws an underline or overline
whole, `all` as `auto`, and a value that is none of the three is passed
over for the inherited one — and an underline and an overline are drawn
beneath the letters, a line through over them, as Chrome 154 draws them.
`skip-ink-values` is Chrome's at one and at two. Found: **584** (a text's
shadow beneath its lines, and the lines' own shadows). 278 queue items
are open (581 closed; 584 added). The next unused queue number is 585 and
the next unused ADR 0058.

Iteration 348 closed queue item 584: under text that casts a shadow each
decoration line casts it too, and the shadows go where Chrome 154 puts
them — the letters' shadows, then the underline's and overline's shadows,
those lines, the letters, the line through's shadows and the line through.
`text-shadow-on-lines` is Chrome's at one and at two. 277 queue items are
open (584 closed; none added). The next unused queue number is 585 and the
next unused ADR 0058.

Iteration 349 closed queue item 573: a spread shadow's corners follow CSS
Backgrounds 3's rule, as Chrome 154 was measured drawing them on 82 boxes
— a radius at least the spread grows by all of it, a smaller one by the
spread times `1 + (r − 1)³`, a square one not at all, each radius on its
own and from the radii as fitted to the box. `alo-paint`'s
`Corners::spread`; `spread-shadow-corners` is Chrome's at one and at two,
and `backgrounds-on-device-pixels`' spread shadow's corners are exact.
Found: **585** (an inset shadow's corners are the padding box's). 277
queue items are open (573 closed; 585 added). The next unused queue number
is 586 and the next unused ADR 0058.

Iteration 350 closed queue item 574: inside a line, text is on the grid
of 1/64 of a device pixel. Chrome 154 rounds a text item — one text box's
text on one line — up onto the grid and starts what follows there, and
draws its letters' width rounded up; measured on 300 lines, 120 spans,
120 wrapped blocks and 620 text rectangles at one and at two (ADR 0054's
addendum of 2026-10-14). `alo-layout`'s `text_run.rs`; `text-on-the-grid`
is Chrome's in all 20 boxes at one and at two, and 950 of the corpus's
1,267 boxes are Chrome's exactly at one where 899 were. Found: **586**
(where Chrome breaks a line before punctuation). 277 queue items are open
(574 closed; 586 added). The next unused queue number is 587 and the next
unused ADR 0058.

Iteration 351 closed queue item 575 for the overflow clip and cut the
rest: a box that clips clips to its border box on whole device pixels,
less its border, where its own background is drawn, as Chrome 154 was
measured clipping 2,016 of 2,016 boxes at one and at two (ADR 0057,
*Measured since*). `clips-on-device-pixels` is Chrome's in every pixel at
both scales but a rounded corner's curve. Cut: **587** (a picture's box,
an `<svg>`'s viewport, an outline and a fractional scale). Found: **588**
(a rounded clip's corners). 278 queue items are open (575 closed; 587 and
588 added). The next unused queue number is 589 and the next unused ADR
0058.

Iteration 352 closed queue item 587 for a picture's box and cut the rest:
a picture is drawn into its content box with each edge rounded to a whole
device pixel on its own, as Chrome 154 was measured drawing 1,568 of 1,568
pictures of seven kinds at one and at two (ADR 0057, *Measured since*),
and a picture starting off the page starts there rather than being slid
onto it. `pictures-on-device-pixels` is Chrome's in every pixel at both
scales. Cut: **589** (an `<svg>`'s viewport and a fractional scale).
Found: **590** (a picture on a line with a width and a height set is laid
out to its ratio) and **591** (an `outline` is not drawn). 280 queue
items are open (587 closed; 589–591 added). The next unused queue number
is 592 and the next unused ADR 0058.

Iteration 353 closed queue item 588: a rounded box that clips inside a
border clips to its padding edge's curve, each radius resolved against
the border box, fitted to it and less the border beside it, as Chrome 154
was measured clipping 60 of 60 bordered, rounded boxes at one, two and
eight (ADR 0057, *Measured since*). `rounded-clips-inside-borders` is
Chrome's in every plain pixel at both scales. Found: **592** (a border
drawn side by side leaves its rounded corners' inner curve unfilled). 280
queue items are open (588 closed; 592 added). The next unused queue number
is 593 and the next unused ADR 0058.

Iteration 354 closed queue item 589 for the `<svg>` and cut the rest: an
`<svg>` is clipped to its content box with each edge rounded to a whole
device pixel and drawn from its border box's corner rounded so, plus its
border and padding as laid out; an SVG file in an `<img>` is drawn into its
rounded content box and grown to fill it, as Chrome 154 was measured doing
for 1,792 `<svg>`s and 672 such pictures at one and at two (ADR 0057,
*Measured since*). `svgs-on-device-pixels` is Chrome's in every plain pixel
at both scales. Cut: **593** (ADR 0057's table at a fractional scale). 280
queue items are open (589 closed; 593 added). The next unused queue number
is 594 and the next unused ADR 0058.

Iteration 355 closed queue item 590: a replaced box is sized by
`alo-layout`'s `replaced.rs` — what its style or its parent fixed, a fixed
dimension clamped on its own and the other following from it, CSS 2
§ 10.4's table when neither is fixed — and `taffy` is handed only an
author's `aspect-ratio`, where it was handed a picture's own ratio and
held the picture at least its width over it tall. Measured in Chrome 154
on lines, as blocks, as flex items and positioned; `pictures-sized-on-a-line`
is Chrome's in all 35 boxes at one and at two, and `a-turned-picture`'s
flex row shrinks as Chrome's does. Found: **594** (a picture scaled up is
smoothed across its texels). 280 queue items are open (590 closed; 594
added). The next unused queue number is 595 and the next unused ADR 0058.

Iteration 356 closed queue item 592: a `solid` border of one colour on
every side that has a width is drawn as one ring, the border box's shape
less the padding box's, and every other border side by side, its sides
split on the mitre through the border box's corner and the padding box's
and carried into a rounded corner as far as its inner curve — as Chrome
154 was measured drawing sixteen such boxes at one, two and eight. A
solid border whose sides differed was four rectangles, which left the
inside of every rounded corner white. `borders-side-by-side` is Chrome's
in every plain pixel at both scales. Found: **595** (a `border-*-style:
none` longhand leaves a width `border-width` set) and **596** (dashes and
dots along a rounded corner). 281 queue items are open (592 closed; 595
and 596 added). The next unused queue number is 597 and the next unused
ADR 0058.

Iteration 357 closed queue item 593: ADR 0057's table measured at a scale
of 1.5 in Chrome 154. Every row holds as written, counted in sixty-fourths
of a device pixel, which no float holds at 1.5; `alo-value`'s
`device_pixels` puts a number within a sixteenth of one on it there, and a
box's edge, a leading, a number's line height and every snapped edge go
through it. 975 of the 985 corpus boxes exact at one are Chrome's at 1.5,
the probes of the rules are Chrome's in all 904 numbers, and six cases
drawn at 1.5 are Chrome's in every plain pixel; nothing at one or two
moved. Cut: **597** (a checkbox and a radio drawn at a fractional scale),
**598** (a centred line's start and a `middle` raise off the grid) and
**599** (the window at a fractional screen's own scale). 283 queue items
are open (593 closed; 597, 598 and 599 added). The next unused queue
number is 600 and the next unused ADR 0058.

Iteration 358 closed queue item 594, its scope cut to pictures drawn no
smaller than they are: such a picture is smoothed across its texels as
Chrome 154 smooths it — bilinear onto its content box as drawn, clamped to
its edge texels, the weights in 256ths, and the mix moved a thirty-second
of a level towards the middle before it is rounded (`alo-paint`'s
`bilinear.rs`). Every opaque pixel of 37 probes at one and at two is
Chrome's, and all but 59 of 1,470,000 channel samples of random colour
pairs; `pictures-sized-on-a-line`'s pictures drawn no smaller are Chrome's
in every pixel at both scales, and so are `a-picture`'s PNG and
`a-picture-in-each-format`'s GIFs and lossless WebPs. A picture drawn
smaller on an axis, which Chrome samples through mipmaps, now opens item
179. Cut: **600** (a translucent texel's last rounding) and **601**
(`image-rendering`, blocked on a page). 284 queue items are open (594
closed; 600 and 601 added). The next unused queue number is 602 and the
next unused ADR 0058.

Iteration 359 closed queue item 595: a side whose border style is `none`
or `hidden`, or given no style, has no width however its width was
written, as Chrome 154 computes it. Layout and paint read a side's style
through one function, `alo-layout`'s `border_style.rs`, in which a side
shorthand that names no style resets it. `borders-with-a-side-none`, ten
boxes, is Chrome's in numbers and in every plain pixel at one and at two;
no other case moved. Found: **602** (a style with no width is `medium`)
and **603** (`border` written after a longhand does not override it). 285
queue items are open (595 closed; 602 and 603 added). The next unused
queue number is 604 and the next unused ADR 0058.

Iteration 360 closed queue item 596: a dashed or dotted side with a curve
inside either of its corners is laid along the border's centre line, one
closed line from where the top-left curve meets the top, in the side's own
pattern and inside its own wedge, as Chrome 154 was measured laying it on
49 probe boxes at one, two and eight. The line is measured as coarsely as
Chrome measures it (`alo-paint`'s `contour.rs`), the dashes and dots are
spaced to go round it a whole number of times (`centre_line.rs`), and a
dash is swept as the true reach of its normals, which covers the middle of
a curve tighter than its half width where the rented stroker left it open
(`sweep.rs`). `dashes-and-dots-round-a-corner`, fifteen boxes, is Chrome's
in every plain pixel at one and at two but one pixel at each, where 2,112
and 10,707 differed. Cut: **604** (a stroke round a curve tighter than
it), **605** (a centre line on half device pixels at one) and **606**
(Chrome's spacing on a straight side). 287 queue items are open (596
closed; 604, 605 and 606 added). The next unused queue number is 607 and
the next unused ADR 0058.

Iteration 361 closed queue item 597: a checkbox and a radio are drawn in
their border box snapped to whole device pixels and cut to whole CSS
pixels from its corner, as Chrome 154 was measured drawing 624 of them at
one, two and 1.5 (`alo-paint`'s `toggle::picture_box`). The new case
`toggles-at-fractional-scales`, 216 controls, is Chrome's in every plain
pixel at one, 1.5 and two, where 677 and 1,248 differed at 1.5 and two,
and `checkbox-beside-text` at 1.5 in every plain pixel but four at the foot
of its last row's letters, where 74 differed. Found: **607** (the foot of
a line's letters at 1.5). 287 queue items are open (597 closed; 607
added). The next unused queue number is 608 and the next unused ADR 0058.

Iteration 362 closed queue item 598: a centred line starts half its
leftover room in, that half truncated onto the layout grid, and a box
aligned `middle` is raised by half its parent's x-height rounded to the
grid, a half going up, less half its own height truncated, as Chrome 154
was measured doing on 1,280 probe rows at one, two and 1.5 (`alo-layout`'s
`layout_unit::half_truncated` and `half_rounded`, ADR 0054's addendum of
2026-10-14). The seven boxes `a_page_at_a_scale_of_one_and_a_half.rs` held
for it are Chrome's, and the new case `centred-and-middle-on-the-grid`, 118
boxes, is Chrome's in every box and pixel at one, 1.5 and two. Found:
**608** (an out-of-flow box's static position on an empty centred line)
and **609** (a button's `width` is its border box). 288 queue items are
open (598 closed; 608 and 609 added). The next unused queue number is 610
and the next unused ADR 0058.

Iteration 363 closed queue item 599: the window takes a screen at 1.5 at
its own scale rather than rounding it down to one (`alo-window`'s
`scale.rs`: the measured scales, a window's lengths at them, and the most
whole CSS pixels that fit), and `alo --scale 1.5` lays it out at 1.5 on
any screen. Captured with `screencapture -l` on this Mac's external screen,
the window is its composition at 1.5 through the real conductor in all
699,822 opaque pixels. Cut: **610** (a fractional scale other than 1.5).
288 queue items are open (599 closed; 610 added). The next unused queue
number is 611 and the next unused ADR 0058.

Iteration 364 closed queue item 600: a translucent picture drawn larger
than it is has each texel premultiplied to a level, its mix held in
sixteenths of a level, and the mix put onto the eight-bit pixel beneath in
one 32-bit fused multiply-add, as Chrome 154 was measured doing on 3.9
million samples (`alo-paint`'s `bilinear.rs` and new `mix_onto.rs`). The
same arithmetic is item 594's opaque rule exactly. The new case
`translucent-pictures-drawn-larger` is Chrome's in every pixel at one and
two, where 2,850 and 12,399 differed, and `a-picture-in-each-format`'s
see-through WebP is Chrome's in every pixel. 287 queue items are open (600
closed). The next unused queue number is 611 and the next unused ADR 0058.

Iteration 365 closed queue item 602: a side given a border style and no
width is `medium`, 3px, as Chrome 155 computes it — by `border-style`
alone, `border: solid`, a style longhand or a side shorthand — and a side
shorthand naming no width resets its side to 3px under a wider `border`;
a width longhand takes `thin`, `medium` and `thick` (`alo-value`'s
`parse_line_width` and `initial_line_width`, `alo-layout`'s
`declared_border_width`). The new case `borders-with-no-width-written`,
twelve boxes, is Chrome's in numbers and in every plain pixel at one and
at two; its worst blend, a two-colour mitre, is noted on 518. 286 queue
items are open (602 closed). The next unused queue number is 611 and the
next unused ADR 0058.

Iteration 366 closed queue item 603: `border-top`, `border-right`,
`border-bottom` and `border-left` take out the longhands they beat in the
cascade, as `border` already did (`alo-style`'s `RESET_BY`, and
`alo-css`' `longhand.rs`, whose lists CSSOM's `setProperty` and
`removeProperty` share), so a side's shorthand written after its longhand
overrides it. The new case `border-shorthands-after-longhands`, twelve
boxes measured in Chrome 155, is Chrome's in numbers and in every plain
pixel at one and at two; its worst blend, 76, is a two-colour mitre (518).
No other reference moved. 285 queue items are open (603 closed). The next
unused queue number is 611 and the next unused ADR 0058.

Iteration 367 closed queue item 179, its scope cut: a picture drawn
smaller than it is on both axes goes through Chrome 155's mipmap levels.
Chrome uploads the largest level that still covers the box. A level is
Skia's: halved rounded down, one-two-one on an odd side, the sum rounded
down. Two levels are mixed by the larger ratio's logarithm as a GPU takes
it, straight between quarters of the mantissa, to the nearest 256th less
120, and the mix is held once in sixteenths. Chrome keeps one decoded
picture per file, so once a copy has gone through its levels every later
draw of it does too (`alo-paint`'s `mip_level.rs` and `picture_filter.rs`,
`alo-renderer`'s `pictures.rs` decoding each file once). A picture smaller
on one axis only stays bilinear. `pictures-sized-on-a-line` is Chrome's in
every pixel of every picture at one and at two. The new case
`pictures-drawn-smaller` is Chrome 155's in every pixel but four, each a
level apart where the mix is a tie. Cut: **611** (the copy where an odd
side is halved), **612** (a translucent picture drawn smaller), **613** (a
turned picture) and **614** (levels made for a copy first drawn without
them). 288 queue items are open (179 closed, four cut). The next unused
queue number is 615 and the next unused ADR 0058.

Iteration 368 took queue item 604 and found its two small boxes were not
the stroker: a closed line longer than two dashes but shorter than two
dashes and two gaps takes two of each, the asked-for pair shortened
together to a third and a sixth of the line, as Chrome 155 draws 133
measured circles. Built as item 615 (`alo-paint`'s `centre_line.rs`
`fitted`) with the new case `dashes-round-a-short-line`, Chrome's in every
plain pixel but 3 at one and 24 at two, which 604 keeps with its stroker
question. A dotted ring too short for its dots, drawn whole by Chrome, is
cut as 616. No other reference moved. 289 queue items are open (615
added closed, 616 added open). The next unused queue number is 617 and
the next unused ADR 0058.

Iteration 369 took queue item 604 and found that its pixels at one, and
ten at two, were not the stroker: Chrome 155 fits a dashed line's gaps to
its length truncated to whole device pixels and lays the pattern along
the whole line, so the leftover, under a device pixel, starts one more
dash run on into the first, and the first dash starts that much before
the line — on circles of any size and along long boxes. Built as item
617 (`alo-paint`'s `centre_line.rs` `whole_pixels`) with the new case
`dashes-fitted-to-whole-pixels`, Chrome's in every plain pixel at one and
at two (13 and 39 before). The two earlier dash cases lose their pixels
at one; six at two on dash ends round tight curves moved off Chrome's
and stay with 604. Whether round dots are fitted the same way is cut as
618, measured but not settled. No other reference moved. 290 queue items
are open (617 added closed, 618 added open). The next unused queue number
is 619 and the next unused ADR 0058.

Iteration 370 took queue item 619, the first of ADR 0058's four, ahead of
its place by the owner's choice (2026-10-24): the browser process starts
the machine's own Chrome headless on a private profile and speaks to it
over `--remote-debugging-pipe` (`alo-window`'s `chrome.rs`,
`chrome_wire.rs`, `chrome_protocol.rs`), never a port. Chrome 155 answers
`Browser.getVersion` and is gone with its helpers and profile whether the
browser closes it or dies. 620 (a page read as our tree) is next in ADR
0058's line. 293 queue items are open. The next unused queue number is
623 and the next unused ADR 0059.

Iteration 371 took queue item 620, the second of ADR 0058's four: a
frozen page opened in a tab of the machine's Chrome, every request but its
own refused (`alo-window`'s `chrome_tab.rs`), and its accessibility tree
mapped into the engine's own `Snapshot` (`chrome_tree.rs`). `web-a-form`
reads node for node as the engine reads it, apart from an unnamed `form`
Chrome exposes and the time field Chrome builds, both listed in the item.
Opening a page from the network is cut as 629. 299 queue items are open
(620 closed, 629 added). The next unused queue number is 630 and the next
unused ADR 0060.

Iteration 372 took queue item 621, the third of ADR 0058's four: a verb on
a page in Chrome is decided by `alo-agent`'s own `perform`, now made against
a `Reading` that either renderer's page can be (`reading.rs`; Chrome's is
`alo-window`'s `chrome_reading.rs`), and carried out over CDP's `Input`
domain as a click checked against Chrome's hit test or as typing into a
focused field (`chrome_act.rs`) — never script. `web-a-form`'s order comes
to the same outcomes through Chrome as through the engine, and every
refusal `perform` makes is reached on Chrome's page by name before Chrome
is asked anything. A tab a frozen page opens is now held and closed before
it loads anything. The socket reaching a tab in Chrome is cut as 630, a
form Chrome would not send as 631, a date and a choice through Chrome as
632. 301 queue items are open (621 closed; 630, 631 and 632 added). The
next unused queue number is 633 and the next unused ADR 0060.

Iteration 373 decided queue item 629 as ADR 0060, measured against Chrome
155 with a local server: Chrome's own network carries a page's requests —
`alo-net` could not carry its WebSockets, does not own the cookie jar a
fulfilled response fills, and cannot stream into Chrome — and the browser
process holds every target of a tab until its requests are paused, judges
and records each before it leaves, refuses an agent's off-site address
before any command, and records a WebSocket as not decided. Trust, ADR
0007's partitioning, ADR 0008's resolver, ADR 0011's cache and ADR 0030's
name are set aside on that path by name. Cut as 633 (the tab from the
network), 634 (its record) and 635 (whether Chrome keeps the machine's
resolver). 304 queue items are open (633, 634 and 635 added; 629 stays open
until 633 and 634). The next unused queue number is 636 and the next unused
ADR 0061.

Iteration 374 built queue item 633: `Tab::open` opens a page from the
network in Chrome and `Tab::open_for_agent` does so after the agent's
sites are judged, an off-site address `NotGranted` with nothing written.
Every frame and worker of the tab is held (`chrome_hold.rs`) until its
requests are paused — measured, a dedicated worker has no `Fetch` and its
requests pause through its document — and every paused request is judged
by ADR 0020's rules (`chrome_gate.rs`) and let go or refused; a navigation
Chrome never pauses, such as `mailto:`, is judged as it is told of. The
frozen tab is held the same way: a frozen page's service worker script is
paused in its own target and refused, asserted against a server that was
asked nothing. 303 queue items are open (633 closed). 634 is
next on the product path, and closes 629. The next unused queue number is
636 and the next unused ADR 0061.

Iteration 375 built queue item 634: every request of a page from the
network in Chrome is a line of `Chrome::record()`'s `Activity`, with a
cause the browser process gives it (`chrome_record.rs`) — the person's
first page, the page's own requests, an agent's verb and the navigation
it set going — each hop of a navigation a line, a WebSocket
`Happened::Undecided`, a request let go `Happened::LetGo`, and a kind
`alo-net` has no word for `Purpose::Unnamed`. Measuring it found that a
shared worker is never told of through the tab and its `fetch` reaches the
network unpaused: 637, which now closes 629. Cut: 636 (a frame as a
document of its own) and 638 (whose cause a tab an agent opened is, needs
an ADR). 305 queue items are open (634 closed; 636, 637 and 638 added).
The next unused queue number is 639 and the next unused ADR 0061.

Iteration 376 built queue item 637, closing 629: a page's shared worker in
Chrome is held through Chrome's own session — measured against Chrome 155,
it is told of there alone, waiting, answers `Fetch.enable`,
`Target.setAutoAttach` and `Network.enable` before it runs, and with
`Fetch` on its own session every request it makes is paused — so
`chrome_tab.rs` holds it as the tab's. Measured too: before this the
browser's own hold left it waiting for good, so it never ran rather than
escaped; 634's escape was a probe that held nothing at Chrome's level.
303 queue items are open (629 and 637 closed). 630 is next on the product
path. The next unused queue number is 639 and the next unused ADR 0061.

Iteration 377 built queue item 630: the agent socket reaches a tab in
Chrome. What `serving.rs` asks of the tabs is the trait `pages::Pages`,
answered from this engine's tabs (`engine_pages.rs`) and from a tab in
Chrome (`chrome_pages.rs`), which `chrome_serving.rs` holds on a thread of
its own and is sent `Order::Agent` by the same `Listening`. The same lines
fill `web-a-form` in Chrome, the refusals arrive as the same kinds, and the
agent record names the renderer; the two records differ in that and in
each renderer's own box numbers and tree sizes. A tab in Chrome is named by
where its page is now; a verb on a page that left is refused as the new
kind `NotCarriedOut`. Cut: 639 (issues through Chrome) and 640 (a fragment
through Chrome). 304 queue items are open (630 closed; 639 and 640 added).
The next unused queue number is 641 and the next unused ADR 0062.

Iteration 378 built queue item 640: a page in Chrome that moves within
itself — a link to `#part`, a script's `pushState` — is named where it
moved, the part included, read from `Page.navigatedWithinDocument`
(`chrome_protocol::within`); such a link answers `Followed`, as the
engine's does. The engine's tab is still named where it was opened, because
it follows no link until item 85; the item closes saying so, asserted in a
test of each renderer. 303 queue items are open (640 closed). 641 is next
on the product path. The next unused queue number is 642 and the next
unused ADR 0062.

Iteration 379 built queue item 641: `alo --chrome (ADDRESS | PAGE.html)`
runs the Chrome path from the program a person starts (`opening::Renderer`,
`Opening::chrome_page`, `ChromeServing::rendered_by`). It says on the
terminal which renderer draws the page — Chrome by its own version, or this
engine — answers the socket from another process, and, drawing in no window
until 627, runs until its input ends. A scale, the frozen fonts and a
page's sheets are refused beside it. Asserted by running the binary in
`the_program_told_to_use_chrome.rs`; a live run of the same page through
both renderers is in the journal. 303 queue items are open (641 closed;
iteration 378's count of 303 was one short).
627 is next on the product path. The next unused queue number is 642 and
the next unused ADR 0062.

Iteration 380 built queue item 627, with its scope cut: a page in Chrome
is drawn in Chrome's own window (ADR 0061) — started shown in `--app` mode,
its window's page the tab, the window sized around the page so it is laid
out at the size asked, placed by this process, answered by where it went
and brought in front (`chrome_window.rs`, `Chrome::start_shown`,
`ChromeServing::start_shown`). `alo --chrome` shows the page so and says
where on the terminal. Cut into 642 (a window of ours around Chrome's,
following it) and 643 (with Stage Manager on, a window not in front is a
thumbnail in its strip, and bringing it in front did not always work under
the suite's other shown Chromes). 304 queue items are open (627 closed; 642
and 643 added). 642 is next on the product path. The next unused queue
number is 644 and the next unused ADR 0062.

Iteration 381 closed queue item 644, the gate failure that stopped the
loop at 21:10 on 24 October: reproduced on purpose under load, it was the
page's first paint outlasting the test's four-second bound, not the
renderer the test stops. Four window tests now give a renderer `alo`'s own
`LONGEST_SILENCE`, and the two that stop one kill it if they fail first.
It served no roadmap line. 304 queue items are open (644 closed). 642 is
next on the product path. The next unused queue number is 645 and the next
unused ADR 0062.

Iteration 382 wrote item 642's decision, ADR 0062, before any code depends
on it: measured under Stage Manager, an ordinary window of ours and
Chrome's are never on the screen together, so the frame around Chrome's
is a floating window, put by Chrome's word and shown only while the window
server lists Chrome's window on the screen at those bounds — Chrome's page
reads `visible` even as a thumbnail, so nothing over the pipe can say.
The window server is read through `objc2-core-graphics`, already beneath
`softbuffer`, if every call is safe, and through `osascript` if not. A
person's drag and our window brought in front were not measured (the screen
was locked), and the building iteration measures them first. No code was
written and 642 is not done. 304 queue items are open. 642 is next on the
product path. The next unused queue number is 645 and the next unused ADR
0063.

Iteration 383 closed queue item 622: a person asks the running browser
which renderer draws a tab with `alo-ask` and the socket's new `renderer`
ask, answered by the browser process from what it started — this engine,
or Chrome by the version it gives itself — never by the page, kept to the
sites and recorded as a read is. 642 waits on a screen in use (it was
locked again), so this iteration took 622 rather than build the frame
unmeasured. 303 queue items are open (622 closed). 642 is next on the
product path once the screen can be measured; 623 is the next open item
that needs no screen. The next unused queue number is 645 and the next
unused ADR 0063.

Iteration 384 closed queue item 623 in this engine's window: each verb an
agent runs over the socket leaves the sentence it was answered with — the
answer's own `said` — in a line of the browser's own along the bottom of
the window, which no page can read or forge (`showing.rs`, `place.rs`'s
`agent_line`, `compose.rs`, two reference renders). Through Chrome there
is no window of ours yet; 642 now owes carrying the line. The screen was
locked again, so 642 and 643 still wait on it. 624 and 625 build on 623
and need no screen. 302 queue items are open (623 closed). The next
unused queue number is 645 and the next unused ADR 0063.

Iteration 385 closed queue item 624 in this engine's window: the box an
agent's verb acted on is ringed on the frame painted after it, around the
node's own rectangles read from the same layout, as alo draws focus — two
pixels of accent, two pixels outside the box (`mark.rs`,
`conductor/marking.rs`, `showing.rs`, two reference renders). "While it
runs" is read as the frame that shows it, since nothing can be drawn
inside the renderer's one exchange. Through Chrome it waits on 642 with
623's line. The screen was locked again, so 642 and 643 still wait on it;
625 builds on 623 and needs no screen. 301 queue items are open (624
closed). The next unused queue number is 645 and the next unused ADR 0063.

Iteration 386 closed queue item 625 in this engine's window: a verb an
agent asked for and was refused — by the page or by the browser process —
leaves the refusal's own `said` in a line of its own on alo's danger tint,
kept until the second verb that runs after it, two refusals two lines, at
most three agent lines kept (`agent_lines.rs`, `place.rs`'s
`agent_lines`, `compose.rs`, two reference renders). Through Chrome it
waits on 642 with 623's line and 624's ring. The screen was locked again,
so 642 and 643 still wait on it. Found: 645, a plain button that says it
has something to scroll. 301 queue items are open (625 closed, 645
opened). The next unused queue number is 646 and the next unused ADR 0063.

Iteration 387 closed queue item 631 by a candidate it had not named,
measured against Chrome 155: a refused press puts Chrome's bubble in its
accessibility tree as an alert describing no DOM node, which a page
cannot forge, and gives the failing control the keyboard. A press through
Chrome on a submit button that sends nothing and leaves a new bubble now
answers `NotSubmitted` with that control and the bubble's words, which
are the engine's words wherever the engine has them
(`chrome_bubble.rs`). Chrome names no constraint, so `Invalid`'s is now
optional, `null` on the socket (ADR 0051's amendment of 25 October), and
it tells of the first control only. A form Chrome refuses with no bubble
is cut as 646. The screen was locked again, so 642 and 643 still wait on
it. 301 queue items are open (631 closed, 646 opened). The next unused
queue number is 647 and the next unused ADR 0063.

Iteration 388 closed queue item 632 with its scope cut, measured against
Chrome 155: a date field's three segments are `spinbutton`s under it in
Chrome's tree, in the order the machine's region lays them out, each
describing a shadow element whose `pseudo` names its part. A date put
through Chrome is now typed a segment at a time, each given the keyboard
(`chrome_date.rs`), and a form sends the dates the engine sends; a date
Chrome's field would not hold as written — past 275760-09-13, or a long
year with leading zeros — is not typed and answers `NotCarriedOut`. A
`<select>` stays refused as `Unbuilt`: how an agent chooses an option is
undecided on either renderer, and the engine's own `PutText` on one is
answered `TextPut` while changing nothing a form sends — both cut as 648,
needing an ADR. The screen was locked, so 642 and 643 still wait on it.
302 queue items are open (632 closed, 648 opened; 647 was added by the
owner). The next unused queue number is 649 and the next unused ADR 0063.

Iteration 389 closed queue item 635, measured from Chrome 155's own
NetLog: on a fresh profile Chrome's DNS is in *automatic* secure DNS
mode, which upgrades a resolver it recognises to encrypted DNS unasked —
the silent replacement ADR 0008 forbids. Every Chrome the browser starts
now gets a profile whose `Local State` turns that off
(`chrome_profile.rs`), and Chrome's log then reads mode 0 for its
configuration and every lookup (`tests/chromes_resolver_is_the_machines.rs`).
`--disable-features=DnsOverHttps` does nothing in 155. Found in the same
log: Chrome itself, with no page open, fetches a variations seed,
downloads and installs extension updates, checks in for push messaging
and asks `accounts.google.com` — none of it gated or recorded (649); and
Chrome asks the nameservers through its own DNS client, whose fidelity to
the system's is unmeasured (650). The screen was locked, so 642 and 643
still wait on it. 303 queue items are open (635 closed, 649 and 650
opened). The next unused queue number is 651 and the next unused ADR 0063.

Iteration 390 closed queue item 636, measured against Chrome 155: every
frame's document request is paused through the session of the frame
holding it, a same-site frame commits through the tab's session and an
other-site frame only through its own target once sent `Page.enable`, and
a dedicated worker's requests are named for its document's frame. Each
frame is now a document of its own in the record (`chrome_record.rs`),
caused by the page holding it, its `fetch` and its worker's its own, and
a page holds at most the 1000 frames Chrome makes. Cut: 651 (a frame
sending itself to a scheme Chrome never pauses is not judged). The screen
was locked, so 642 and 643 still wait on it. 303 queue items are open
(636 closed, 651 opened). The next unused queue number is 652 and the
next unused ADR 0063.

Iteration 391 closed queue item 639, measured against Chrome 155: with
`Runtime` on, a page's console calls and uncaught throws are told of
through the tab's session before a later `Runtime` answer, earlier ones
are told of again on enabling unless forgotten first, and a page's
20-million character `console.log` is a 20 MB message — past the 16 MiB
the wire read, which ended the whole pipe. The wire now skips such a
message to its NUL and goes on (`chrome_wire.rs`); a tab hears only from
a verb's decision to its answer, forgetting what was said before; and
what it heard is the answer's `issues` in the engine's words
(`chrome_said.rs`), the differences — places by address and page line,
a console the engine does not have, an error's words as constructed, an
unhandled rejection told a task late — written down in the queue. Cut:
652 (frames and workers in targets of their own) and 653 (messages held
by count, not size). The screen was locked, so 642 and 643 still wait on
it. 304 queue items are open (639 closed, 652 and 653 opened). The next
unused queue number is 654 and the next unused ADR 0063.

Iteration 392 closed queue item 651, measured against Chrome 155: a frame
sending itself to `mailto:` is told of through the tab's session when it
is from the page's site and through its own target when it is from
another, and the page sending its other-site frame there through the
tab's — once each, naming the frame and not who asked — and Chrome
commits nothing. Each is now judged as the tab's own ask is, refused by
name and recorded with the frame's document as its cause
(`chrome_tab.rs`). The screen was locked, so 642 and 643 still wait on
it. 303 queue items are open (651 closed). The next unused queue number
is 654 and the next unused ADR 0063.

Iteration 393 closed queue item 652, measured against Chrome 155: a frame
from another site and a dedicated worker each answer `Runtime`'s forget,
enable and disable in their own sessions, tell again of everything since
they started unless they forget first, and, held, answer `Runtime.enable`
before they run. Each is now told to forget, tell and stop as the tab's
own session is, and one held while a verb runs is told to tell before it
is let go (`chrome_tab.rs`, `chrome_hold.rs`). Not told of while a verb
holds the page's own thread: a page's worker's uncaught throw, and a
worker the page starts in that task. Cut: 654 (shared and service
workers). The screen was locked, so 642 and 643 still wait on it. 303
queue items are open (652 closed, 654 opened). The next unused queue
number is 655 and the next unused ADR 0063.

Iteration 394 closed queue item 654, measured against Chrome 155: a
shared worker (through Chrome's own session) and a service worker
(through the tab's) each answer `Runtime`'s forget, enable and disable in
their own sessions, tell again of what they said before unless they
forget first, and go on — throwing, told of — while a verb holds the
page's thread. Both are now heard (`chrome_hold.rs`), since every page
either serves is the one tab's; a service worker started mid-verb is let
go without waiting for its answer (`chrome_tab.rs`). Found and cut: 655,
a service worker that stops and starts again waits in its old target and
is never let go; and 656, a frame's next document's fetch pinned on the
document before it, from a gate run that failed in a test this change
cannot reach (647 read from its own gate failure the same day). The
screen was locked, so 642 and 643 still wait on it. 304 queue items are
open (654 closed, 655 and 656 opened). The next unused queue number is
657 and the next unused ADR 0063.

Iteration 395 wrote queue item 638's decision, ADR 0063: a tab an agent
asks to open is an agent action, minted once the agent's sites reach the
address, and an agent action carries either the document it acted in or
the tab it opened — so the tab's first page is the agent's, never the
person's, its chain ends at that action in that tab, and ADR 0012's three
causes stand (an addendum there says so). No code was written; 638 stays
open for its code half and the test reading the line back. The screen was
locked, so 642 and 643 still wait on it. 304 queue items are open. The
next unused queue number is 657 and the next unused ADR 0064.

Iteration 396 built queue item 638's code half: an agent action carries a
place — the document a verb acted in, or the tab an agent's open made —
so a tab opened in Chrome at an agent's word has its first page caused by
that action in that tab, its page's requests lead back to it, and its
chain ends there rather than at the person; the durable record and the
grants' history keep the new place under new tags. The screen was locked,
so 642 and 643 still wait on it. 303 queue items are open (638 closed).
The next unused queue number is 657 and the next unused ADR 0064.

Iteration 397 closed queue item 655, measured against Chrome 155: a held
service worker stopped — by `ServiceWorker.stopWorker`, `stopAllWorkers`
or a close of its own target — keeps its target and session, is told of
there as `Inspector.targetCrashed`, answers nothing until it starts again
in that target (`Inspector.targetReloadedAfterCrash`, no new attach) and
then waits until it is let go, answering everything asked of it first.
`chrome_hold.rs` marks one stopped, so a verb neither tells nor waits on
it, and `chrome_tab.rs` holds one started again as at its first start and
lets it go. An update of its script is a new target. Memory pressure and a
crash of its process are not measured. The screen was locked, so 642 and
643 still wait on it. 302 queue items are open (655 closed). The next
unused queue number is 657 and the next unused ADR 0064.

Iteration 398 closed queue item 653: what Chrome says and the tab has not
yet read is held by `chrome_held.rs` to 256 messages and 32 MiB together
(`HELD_BYTES`, two of the longest message), the reader and Chrome's
writing waiting past either, with a far side writing 4 MiB events as fast
as the pipe takes them held within the bound. The screen was locked, so
642 and 643 still wait on it. 301 queue items are open (653 closed). The
next unused queue number is 657 and the next unused ADR 0064.

Iteration 399 closed queue item 646, measured against Chrome 155: a
failing control Chrome cannot focus — hidden, `display: none`, inert — is
logged as `Log.entryAdded` with the control as its node argument, each one
of the form, with or without a bubble for another, and `Log.enable`
replays what was logged unless `Log.clear` comes first. A press on a
submit button now has `Log` on, cleared, while it is carried out, and
answers `NotSubmitted` with each such control in Chrome's words and no
constraint, after the bubble's (`chrome_unfocusable.rs`). A cancelled
`invalid` is told of by nothing that tells it from a cancelled `submit`,
so it answers `Activated`, written into ADR 0051's second amendment of
25 October. The screen was locked, so 642 and 643 still wait on it. 300
queue items are open (646 closed). The next unused queue number is 657
and the next unused ADR 0064.

Iteration 400 closed queue items 647 and 656, the race that stopped the
loop three times. Measured against Chrome 155: under load Chrome pauses a
new document's sub-resource before it tells of that document's commit, and
then names the new document's loader in `Network.requestWillBeSent`; a
navigation's `networkId` is that loader. A request paused while its frame
goes to a new document is let go and its line owed until its loader is
told of, or until the frame's renderer, asked again after the commit,
answers after everything it told of (`chrome_owed.rs`, ADR 0060's
amendment). A test runs the machine's Chrome behind a relay that holds a
commit back behind the request named, so both orders fail without the fix
every time. Opened 657: an old page's request paused after the new page's
commit is still the new page's. 299 queue items are open (647 and 656
closed, 657 opened). The next unused queue number is 658 and the next
unused ADR 0064.

Iteration 401 closed 657, an old page's request paused after the new
page's commit. Measured against Chrome 155: every request of a page being
left — 391 beacons, `keepalive` fetches, fetches and pictures from
`pagehide`, `unload` and `visibilitychange` — is paused with no
`networkId`, 312 of them after the new page's commit (all of them when the
new page is from another site), and every request of a living page has
one. A frame now remembers the document it left until its next
navigation's first hop, and a request with no `networkId` paused through a
frame's session is that document's (`Causes::of_unnamed`, ADR 0060's
second amendment). The relay test holds a page's beacon behind the new
page's commit, and fails without the fix. 298 queue items are open (657
closed). The next unused queue number is 658 and the next unused ADR 0064.

Iteration 402 wrote ADR 0064 for queue item 648, how an agent chooses an
option. Measured against Chrome 155: its tree holds a closed list's
options, each with its `selected` and `disabled` state, though none has a
box; on macOS no key changes a closed list but type-ahead, which chooses
and tells the page of each option it passes on the way; a click chooses a
drawn option, and Meta-click toggles one in a `multiple` list. Decided: an
option is chosen by `Activate` on it, a toggle in a list of several,
answered `Chosen` with what the list holds after; `PutText` on a list is
refused `Chooses`; this engine's tree holds a list's options; through
Chrome a closed list's option is refused by name. Opened 658, a closed
list through a shown Chrome, measured with the screen in use. No code
depends on it yet, so 648 stays open. The screen was locked, so 642 and
643 still wait on it. 299 queue items are open (658 opened). The next
unused queue number is 659 and the next unused ADR 0065.

Iteration 403 closed queue item 648 with its scope cut. ADR 0064 reaches
the style sheet, the box tree, layout, `alo-agent`, the boundary, the
socket and Chrome, so 648 closed on its § 4 alone: text on a `<select>` is
refused `Chooses` on both renderers, by `alo_dom::field::is_chosen_from`
asked of either renderer's element, where this engine once answered
`TextPut` and changed nothing its form sends and Chrome answered
`Unbuilt`. The rest is cut as 659 (options in this engine's tree, chosen
by `Activate`, answered `Chosen`) and 660 (the same through a list Chrome
draws, a closed list's option refused by name); 658 now depends on 660.
The screen was locked, so 642 and 643 still wait on it. 300 queue items
are open (648 closed, 659 and 660 opened). The next unused queue number
is 661 and the next unused ADR 0065.

Iteration 404 closed queue item 659: an option chosen by activating it on
this engine's page (ADR 0064 §§ 1, 2, 3 and 5). A `<select>`'s options and
groups are boxes the box tree makes under the list and outside its
children, so nothing that lays out or draws meets them and they have no
rectangle; the agent's tree reads them there, the outline and the socket
saying they are not drawn. `Activate` on one answers `Chosen`, carried out
by `alo_dom::choice` — through `apply` on a page without script, and on one
with script as a task that fires `input` then `change` at the list — and
whether it is chosen is read from the page afterwards. The window rings the
list. Chrome's reading names no option's list yet, so through Chrome an
option is still a click answered `Activated` (660). The screen was locked,
so 642 and 643 still wait on it. 299 queue items are open (659 closed). The
next unused queue number is 661 and the next unused ADR 0065.

Iteration 405 closed queue item 660: an option chosen through a list Chrome
draws (ADR 0064 § 6). Chrome's reading finds an option's list as the
nearest `listbox` or `combobox` above it in Chrome's tree, and it is one
only when both are described as HTML's `<option>` and `<select>`; a list
that drops down (`alo_dom::choice::drops_down`) is answered
`Unacted::Closed` before any input, and any other is chosen in by a click
at the option's middle, with ⌘ held in a list of several
(`chrome_choose`), the answer read back from Chrome's tree. The screen was
locked, so 642 and 643 still wait on it. 298 queue items are open (660
closed). The next unused queue number is 661 and the next unused ADR 0065.

Iteration 406 closed queue item 626: the ambiguous refusal's sentence names
its candidates as `box#3 or box#4`, as every other sentence names a node,
rather than as `[BoxId(3), BoxId(4)]`. The screen was locked, so 642, 643
and 658 still wait on it. 297 queue items are open (626 closed). The next
unused queue number is 661 and the next unused ADR 0065.

Iteration 407 closed queue item 645: a box has something to scroll only
when its scrolling area passes its padding box by half a device pixel, as
Chrome 155 measures, so a plain button is refused `DoesNotScroll`; cut from
it, 661, a spilling box that is not a scroll container. The screen was
locked, so 642, 643 and 658 still wait on it. 297 queue items are open (645
closed, 661 opened). The next unused queue number is 662 and the next
unused ADR 0065.

Iteration 408 closed queue item 661: the layout tree says which boxes are
scroll containers — `overflow` `hidden`, `scroll` or `auto` on either axis,
on a box that is neither inline nor replaced, or a one-line text field — and
an agent's scroll is answered only for those, as Chrome 155 measures; cut
from it, 662 (the root as the viewport, and a body the viewport takes its
`overflow` from) and 663 (a `<textarea>` and a list box). The screen was
locked, so 642, 643 and 658 still wait on it. 298 queue items are open (661
closed, 662 and 663 opened). The next unused queue number is 664 and the
next unused ADR 0065.

Iteration 409 closed queue item 662: the root answers for the viewport —
scrolled when the page's scrolling area passes the window by half a device
pixel, whatever the root's `overflow`, `clip` included as Chrome 155 was
re-measured to scroll it — and a body whose `overflow` the viewport takes
under a root of `visible` is laid out, drawn and answered as `visible`, with
a corpus case that is Chrome's in every pixel; cut from it, 664
(`text-overflow` on such a box). The screen was locked, so 642, 643 and 658
still wait on it. 298 queue items are open (662 closed, 664 opened). The
next unused queue number is 665 and the next unused ADR 0065.

Iteration 410 closed queue item 663 for the `<textarea>`: it keeps its lines
(`white-space: pre-wrap`) and is a scroll container (`overflow: auto`, an
author's `visible` computed `auto`), as Chrome 155 computes it, so one of two
rows holding six lines is scrolled with a scrolling area of 94, its lines past
its edge are clipped, and written in it stands beside its label where it stood
empty. Cut from it: 665 (a list box's options), 666 (`overflow-wrap` in a
`<textarea>`) and 667 (an inline block of `overflow: clip` on its first line).
The screen was locked, so 642, 643 and 658 still wait on it. 300 queue items
are open (663 closed, 665, 666 and 667 opened). The next unused queue number
is 668 and the next unused ADR 0065.

Iteration 411 closed queue item 666: `overflow-wrap: break-word` and
`anywhere` cut a word too wide for its line between its clusters where
nothing else lets the line break, as Chrome 155 cuts it, a `break-word`
cut left out of a box's min-content width and an `anywhere` one counted,
and the engine's sheet gives a `<textarea>` `break-word`; every line and
box of the new `a-word-cut-to-its-line` case is Chrome's to 1/64 px. Cut
from it: 668 (`word-wrap`). The screen was locked, so 642, 643 and 658
still wait on it, and 665 waits on 521. 300 queue items are open (666
closed, 668 opened). The next unused queue number is 669 and the next
unused ADR 0065.

Iteration 412 closed queue item 667: an inline block under `overflow: clip`
— on both axes, or on one beside `visible` — is no scroll container and
stands on its last line, as Chrome 155 stands it, even a line its height
cuts off; only `hidden`, `scroll` or `auto` on an axis puts it on its bottom
edge. Every row of the new `a-clipped-box-stands-on-its-line` case is
Chrome's to 1/64 px. Cut from it: 669 (an empty `<textarea>` under `clip`).
The screen was locked, so 642, 643 and 658 still wait on it; 665 waits on
521 and 668 on a page. 300 queue items are open (667 closed, 669 opened).
The next unused queue number is 670 and the next unused ADR 0065.
