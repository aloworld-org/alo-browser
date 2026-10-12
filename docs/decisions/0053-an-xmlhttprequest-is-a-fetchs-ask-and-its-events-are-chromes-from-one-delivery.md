# ADR 0053 — An `XMLHttpRequest` is a fetch's ask, and its events are Chrome's, fired from one delivery

**Status:** accepted
**Date:** 2026-10-12
**Context:** queue item 83, *`fetch()` and `XMLHttpRequest`, over the same
stack as everything else rather than beside it*, which the owner moved from
`needs design` to `needs ADR` on 2026-10-11 *for the half that is left*; ADR
0032, which decided and built the other half and gave `XMLHttpRequest` one
section (§ 6: *the same ask with a different front*, synchronous refused by
name) without saying what the front is; queue item 336, cut from 83 by that
section and waiting for a page; ADR 0005 (a renderer has no network and
never waits); ADR 0016 §§ 2 and 6 (a response is a task, and work after it
falls outside an agent's window); ADR 0018 §§ 3 and 6 (the browser's
dispatch runs a checkpoint after each listener, a script's does not); ADR
0019 (`isTrusted`); ADR 0031 (a builtin keeps eight slots at most); and the
code this leans on — `alo-bindings`' `fetch.rs`, `fetch_init.rs`,
`body_init.rs`, `delivering.rs`, `document_cell.rs` and `response.rs`,
`alo-renderer`'s `fetch.rs` (`FetchAsk`, `Fetched`) and `fetch_decide.rs`,
and `alo-net`'s `encoding.rs`, the one file that names `encoding_rs`

## The decision in one line

An `XMLHttpRequest` makes **exactly the ask a `fetch` makes** — the same
message, the same decision in the browser process, the same filtered
answer, nothing new on the wire — and differs only in the renderer: its
answer is **one delivery that fires Chrome 154's measured sequence of
events**, as the browser's own dispatch; it is **held by the document while
its answer is owed**, so a request nobody kept a reference to still reaches
its listeners; `abort()` before the ask has left **withdraws it unsent**;
a **synchronous** one is refused at `open` with the error Chrome itself
throws for synchronous misuse; and what is not built is refused by name.

## Why this is a decision rather than a chore

ADR 0032 § 6 settled the boundary and left the front, and the front is
where every choice below is visible to a page:

**Which events, in which order, with which numbers.** The specification's
text and Chrome disagree — the text fires `progress` twice for a body that
arrives at once, Chrome once — and a page's progress bar and its
`onreadystatechange === 4` check are written against Chrome.

**How many tasks.** The specification queues a task per step: the
response's headers, each chunk of body, its end. ADR 0032 § 4 sends a body
whole in one message, so either one message becomes several tasks or the
steps share one, and that is a choice about the event loop.

**Who keeps it alive.** `new XMLHttpRequest()` assigned to nothing, given an
`onload` and sent, is ordinary code. The collector must not take it while
an answer is owed, and must take it once none is.

**What `abort()` does to a request that has already left.** The renderer
cannot reach into the browser process's pool, and ADR 0032 deliberately
left cancellation undecided.

**What "refused by name" means for a synchronous request.** Chrome makes
one; we do not; the page needs an answer it can catch.

## Measured

In Google Chrome 154, headless, driven over the DevTools protocol, from a
probe page in the scratchpad. Chrome's source was not read. Two set-ups:
every request intercepted and answered by `Fetch.fulfillRequest` (so
nothing reached a network), and a real HTTP server on `127.0.0.1` for the
upload events, which **interception suppresses** — under it Chrome fired
`upload`'s `loadstart` and nothing after it, and against a real server it
fires the whole sequence. Each event's listener logged the event, the
`readyState`, the `status`, and the event's `loaded`, `total` and
`lengthComputable`, and queued a microtask that logged too.

1. **`open()`** fires `readystatechange` (state 1) **synchronously, inside
   `open`**, trusted; a second `open` on an opened request fires nothing,
   silently drops the request in flight, and leaves state 1.
2. **`send()`** fires `loadstart` (0 of 0) synchronously, then — only if a
   listener was on `upload` before `send` — `upload`'s `loadstart` (0 of the
   body's length, length computable). Microtasks queued by those listeners
   run after the calling script ends, as for any event a script's call
   dispatches.
3. **A response with a body**, from the server's answer: `upload`'s
   `progress`, `load` and `loadend` (the body's length of itself, once each,
   for an 11-byte body and a 3,000,000-byte one alike) when there is an upload
   listener; then `readystatechange` 2; `readystatechange` 3; **one**
   `progress` (the body's length, of the `Content-Length` or of 0, length
   computable only when there was one); `readystatechange` 4; `load`;
   `loadend`. A body trickled in four chunks over 480 ms still fired one
   `readystatechange` 3 and one `progress`. Every event is trusted; the
   microtasks each listener queued run before the next event.
4. **A response with no body** (`204`): `readystatechange` 2, then 4, then
   `load` and `loadend`, each 0 of 0 — no state 3 and no `progress`.
5. **A network error** — a refused connection, or a cross-origin answer
   without CORS: `readystatechange` 4 with `status` 0; `upload`'s `error`
   and `loadend` if it had a listener; `error`; `loadend`; all 0 of 0.
6. **`abort()`** after `send`, in the same task or later: `readystatechange`
   4, `abort`, `loadend`, synchronously inside `abort`, after which
   `readyState` is **0**. Of two runs aborting in the same task as `send`,
   one request reached the interception and one did not.
7. **Synchronous**: `open('GET', url, false)` then `send()` succeeds;
   setting `responseType` on it throws **`InvalidAccessError`**.
8. **Headers**: `getAllResponseHeaders()` is every readable header,
   **lower-cased, sorted by name**, each `name: value\r\n`, without
   `Set-Cookie`; `getResponseHeader` ignores case and answers `null` for
   `Set-Cookie`. `setRequestHeader('X-Q', '1')` then `'2'` sends `X-Q: 1, 2`;
   `Cookie` is dropped silently. `statusText` is the server's reason phrase,
   and `responseURL` the final URL after a redirect, which is followed.
9. **A string body** is sent as `text/plain;charset=UTF-8` on a `POST` and
   **not at all** on a `GET`, without an error.
10. **Text**: a `charset=iso-8859-1` body reads `café` through
    `responseText`; with `responseType = 'json'`, `response` is the parsed
    value and reading `responseText` throws **`InvalidStateError`**.
11. **State errors**: `send` before `open`, `send` twice, and
    `setRequestHeader` after `send` each throw `InvalidStateError`.

## 1. The same ask, and nothing new crosses

`send()` records a `FetchAsk` in the document cell exactly as `fetch` does
(ADR 0032 §§ 1–2), under a number from **the same counter**, so the two
never collide and the browser process's bounds count both together:

- the URL resolved against the document's base URL at `open`, as the
  specification resolves it there; the method normalised at `open`;
- the headers from `setRequestHeader`, combined with `, ` as measured, a
  forbidden name dropped silently as `fetch`'s `Headers` drops it;
- mode **`cors`** always — an `XMLHttpRequest` cannot be `no-cors` — so a
  cross-origin answer the server did not agree to is a network error, never
  opaque;
- credentials **`same-origin`**, or **`include`** when `withCredentials` is
  `true`; redirect **`follow`**; the document's referrer policy;
- the body from `body_init.rs`, shared with `fetch` and `sendBeacon`, with
  `XMLHttpRequest`'s one difference: **a `GET` or `HEAD` drops its body
  rather than refusing it** (measured 9). For a string body whose author
  gave a `Content-Type` with a `charset`, that charset is rewritten to
  `UTF-8`, as the specification requires, because the bytes are UTF-8.

The browser process cannot tell this ask from a `fetch`'s and has no need
to: ADR 0032 § 3 decides it, § 4 filters what comes back, the cause is the
same rule, a corpus case answers it from the same frozen responses (§ 7),
and an answer for a document that has gone is answered by nobody. **No new
message, field or rule** crosses the boundary in this decision; § 5's
timeout is the one future exception, and it is decided there.

## 2. One delivery fires Chrome's sequence, as the browser's dispatch

When the answer arrives, its handling is **one task** (ADR 0016 § 2) that
fires measured 3, 4 or 5 in order — the upload events, then each state —
each a trusted event (ADR 0019) sent by **the browser's dispatch** (ADR 0018
§ 3), so every listener's microtasks run before the next listener and the
next event, as measured. The specification's separate tasks are not
reproduced: with the whole body in one message, the only thing that could
run between them is another task, and the renderer has no timer or other
task source that could be due between two parts of one message (items 92
and 233). Firing them in one task is therefore indistinguishable from
Chrome today, and § *How we will know* says what changes that.

**Chrome's numbers, not the specification's count.** One `progress` for a
body, carrying its length as `loaded` and the `Content-Length` as `total`
(0, and `lengthComputable` false, without one); none for an empty body,
and no state 3. The specification's text would fire a second `progress`
at the end of the body; Chrome does not, and a page's progress handler is
written against Chrome.

What a script's own call fires — `readystatechange` in `open`,
`loadstart` and `upload`'s `loadstart` in `send`, the three events in
`abort` — is dispatched **inside that call**, as `click()`'s events are
(ADR 0018 § 6): trusted, but with no checkpoint between listeners, so their
microtasks run after the calling script, as measured.

The *upload listener flag* is the specification's: set by `send` when
`upload` has any listener at that moment, and only then are `upload`'s
events fired. Its `progress`, `load` and `loadend` are fired at the start
of the delivery, each with the body's length as both numbers, because by
the time any answer exists the whole body has been sent; a request that
failed before its answer fires `upload`'s `error` and `loadend` instead.

## 3. The document holds a request while its answer is owed

`send()` puts the `XMLHttpRequest`'s wrapper in the document cell under the
ask's number, beside where `fetch` keeps its promise, and the delivery takes
it out. So a request referenced by nothing but its own listeners is kept
exactly as long as an answer is owed to it — the specification's rule that
an object with a pending request and listeners is not collected — and
released by the same events that release a fetch's promise: its answer
delivered, its document gone, or its ask withdrawn. A request that is
never sent is held by nothing of ours.

## 4. `abort()` withdraws what has not left, and forgets what has

`abort()` fires measured 6 synchronously and leaves state 0. Then:

- **If the ask is still in the cell** — `send` and `abort` in the same
  task, before the renderer answered the message whose work made it — it is
  **removed**, and nothing is sent. Chrome was measured both sending and not
  sending in that case; not sending is the one a page cannot tell from the
  other, and the one that costs nobody a request.
- **If it has left**, the renderer forgets the number: the wrapper is
  released from the cell, and when the answer arrives `delivering.rs` finds
  nothing waiting and drops it, as it already drops an answer for a page
  that has gone. The browser process is not told. **The request is still
  made and recorded** — a cost stated below, and the same one a `fetch` the
  page has stopped caring about has today.

A second `open` during a request (measured 1) is the same forgetting, with
no events.

## 5. Synchronous is refused at `open`, and a timeout is the browser process's

**`open(method, url, false)` throws `InvalidAccessError`** with a message
saying that this browser makes no synchronous request. ADR 0032 § 6 already
refused it — a renderer waiting on the browser process breaks ADR 0005 — and
this names how. `InvalidAccessError` is what Chrome throws for the
synchronous misuse it does refuse (measured 7), so a page that guards a
synchronous request with `try` already expects it; throwing at `open`
rather than `send` refuses before the page has set anything up for a
request that will not run.

**`timeout` is not built, and when it is the browser process keeps it.**
The renderer runs nothing between messages (item 233) and has no timer
(92), so it cannot notice a deadline passing; the browser process already
waits on every exchange. When a page sets one, the ask carries the number of
milliseconds, the browser process stops the exchange at the deadline, and
the answer says it timed out, which the renderer fires as `readystatechange`
4, `timeout` and `loadend`, measured. That tells the page nothing a clock
it already has would not: it set the deadline. Until then a non-zero
`timeout` throws `NotSupportedError` by name; zero is the default and is
accepted. A timeout set after `send` would need a message to the browser
process mid-exchange and is refused by name as well.

## 6. What a page reads, and what is refused by name

Built with the first item: `readyState`, `status`, `statusText`,
`responseURL`, `getResponseHeader`, `getAllResponseHeaders` (measured 8,
from headers already filtered by ADR 0032 § 4), `withCredentials` (an
`InvalidStateError` once sent), `overrideMimeType`, `upload`, and
`responseText` and `response` for `responseType` `""` and `"text"`,
**decoded by the response's charset** — `overrideMimeType`'s first, then
`Content-Type`'s, a byte order mark beating both — through `alo-net`'s
`encoding.rs`, the one file that names `encoding_rs`, and UTF-8 when nothing
names one. Every state error is measured 11's `InvalidStateError`.

Refused by name, each its own item when a page needs it:

- **`responseType` `"json"`**, which needs `JSON` (item 73); **`"document"`**,
  which needs a second document in a realm and, for XML, a parser this
  engine does not have; **`"arraybuffer"` and `"blob"`**, which need the
  objects. Setting one throws `NotSupportedError`; an unknown value is
  ignored, as Web IDL's enumeration rule has it.
- **A body that is not a string or a `URLSearchParams`** — a `FormData`
  (492), a `Blob`, a buffer, a `Document`.
- **`timeout`**, as § 5 says.

`responseXML` is `null`, which is what it is for every `responseType` this
builds.

## What this costs

**An aborted request that had already left is still made.** The page sees
the abort as Chrome's page does, but the server receives the request and
the record holds it. A cancellation message, naming the ask's number, is
the decision a page that aborts a long request — or a `fetch` with a
`signal` — will open.

**The specification's tasks are merged into one.** Invisible today, and §
*How we will know* says when it stops being.

**A synchronous request does not run.** A page that depends on one is a
stage 3 page, as ADR 0032 § 6 already said.

## Alternatives rejected

- **A message of its own for `XMLHttpRequest`.** Nothing about the decision
  or the filter differs; a second message would be a second boundary to
  keep in step with the first.
- **One task per specification step.** It needs the message split or a
  task source that queues work for later, and nothing could tell the
  difference until timers exist.
- **The specification's two `progress` events.** Chrome fires one; a page
  is written against Chrome.
- **Sending an aborted ask anyway.** A request nobody will read is a cost
  to the person and to the server, and Chrome was measured not always
  making it.
- **A renderer-side `timeout`.** The renderer cannot run when nothing is
  sent to it.
- **Running a synchronous request by blocking the renderer.** ADR 0005.

## What this does not decide

- **Cancelling a request that has left** — § *What this costs*.
- **`responseType` beyond text, other bodies, and `timeout`** — § 6.
- **A response delivered in parts**, which would bring back the
  specification's state 3 per chunk — ADR 0032's `ReadableStream` item.
- **Workers' `XMLHttpRequest`**, where a synchronous request is permitted
  by the specification — item 91's.

## How we will know if this was wrong

**If a page's events differ from Chrome's** in order, count or numbers, §
2's sequence is wrong, and it is corrected from a new measurement rather
than from the specification's text.

**If timers or another task source arrive (items 92, 233) and a page can
schedule work that Chrome runs between `readystatechange` 2 and 4**, the
single task of § 2 is no longer invisible and the delivery is split.

**If a page's request is collected while its listeners still wait**, or a
finished request is never collected, § 3 is not being followed.
