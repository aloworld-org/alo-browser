# ADR 0032 — A page's fetch is an ask, and what comes back is only what it may read

**Status:** accepted
**Date:** 2026-10-08
**Context:** queue item 83, *`fetch()` and `XMLHttpRequest`, over the same
stack as everything else rather than beside it*, marked **needs design** in
iteration 161 because it named no ADR, contract or closing condition;
`alo-downloads`, alo's public download page, whose script stops at
`fetch(href, { method: "HEAD" }).then(…).catch(…)` on its line 19
(iteration 209); queue item 75 (promises), which depends on 76, and 76's
pieces — item 232 (the job queue and the checkpoint, built), 235 (the loop,
built), 236 (a page's scripts at load, built), 233 (the loop between
messages, open) and 234 (frames, open); ADR 0005 (the browser process owns
the network and sends work, a renderer returns results and never waits);
ADR 0007 (cookies partitioned by top-level site); ADR 0012 §§ 1 and 4 (a
cause is carried, and the browser process assigns it from which message it
was answering); ADR 0016 §§ 2 and 6 (*every response* is a task of its own,
and a continuation waiting on a response falls outside an agent's window);
ADR 0020 (a page's navigation is a claim in the answer to the message whose
work made it, decided by the browser process from its own copy of the
facts); ADR 0025 § 5 (an asynchronous storage request is a message whose
answer is its own task, and a storage key is never taken on a renderer's
word); and the code this is about — `alo-net`'s `cors.rs` (`Mode`,
`Credentials`, `may_read`, `readable`, `made_opaque`), `preflight.rs`,
`csp.rs` (`connect-src`), `mixed.rs`, `request.rs` (`Purpose::Fetch`),
`body.rs` (`LARGEST_BODY`); `alo-renderer`'s `message.rs` (`Loaded` and
`Acted` already carry `wanted`, `objections` and `navigation` as claims),
`wire.rs` (`LARGEST_MESSAGE`), `navigate.rs` and `tab.rs`
(`Tabs::a_page_fetching`, `Tabs::an_agent_acting`); and `alo-workplace`'s
`deploy/production/Caddyfile`, which serves the download page and its
installers from one origin under `/download/` with `connect-src 'self'` on
its marketing site

## The decision in one line

A script's `fetch` is an **ask carried in the answer to the message whose
work made it** — every ask, in order, each under a number the renderer
chose — and never a call; the **browser process decides and makes the
request**, from its own copy of the document's origin, policy and cause,
through the same `alo-net` stack every other request uses; the response
comes back as **a message of its own, which is a task of its own**, and it
carries **only what the page may read** — the filtering is done before the
bytes leave the browser process, so a renderer never holds a body the
same-origin policy kept from it; and the promise `fetch` answers needs only
the job queue and the checkpoint, both built, so a promise is cut from item
75 and built first.

## Why this is a decision rather than a chore

ADR 0005 says a renderer has no network. ADR 0016 § 2 says a fetch is two
tasks. Neither says what crosses, and each of the questions below is visible
to a page, to the person, or to somebody attacking either:

**Who makes the request, and with whose identity.** A renderer that could
say *I am `https://bank.example`* and be believed would be a renderer that
could read the bank. The origin a request is made *from* decides cookies,
CORS and the `Origin` header, and only the browser process knows it without
being told by the page.

**Where the same-origin policy is enforced.** `cors.rs` already decides
whether a page may read a response. The question is *in which process* the
answer is applied. Spectre is ADR 0005's first reason: a body a renderer
holds is a body the page can read, whatever the bindings say. Filtering in
the renderer would make the policy a rule script is asked to keep.

**What a network error tells a page.** Fetch rejects with a `TypeError` that
says nothing about why, on purpose: *the connection was refused* versus *the
CORS check failed* is a port scanner and a cross-origin probe. The person,
who is not the attacker, deserves the reason.

**How much a page may make the browser process hold.** Every ask is memory
and sockets in the process that may not crash. A page decides how many it
makes and how large their bodies are.

**And the promise.** `fetch` answers a promise. Item 75 depends on 76, whose
open halves are the loop running between messages (233) and frames (234). A
fetch's answer arrives *as* a message, so it needs neither — and if this
decision did not say so, the page would wait on work it does not use.

## 1. An ask is a claim in an answer, and a response is a task

A script's `fetch` records an ask in the realm's document cell, as a click
on a link records a navigation (ADR 0020 § 1), and returns a pending promise.
When the message's work is done — `Load`, `Act`, or the message carrying an
earlier response — the renderer takes the asks from the cell and puts them in
its answer. **All of them, in the order they were made**, unlike a
navigation: a page that fetches three things wants three things, and HTML
does not replace one fetch with the next.

Each ask carries a **number the renderer chose**, unique for the life of the
document. It names the promise waiting for it and nothing else: the browser
process echoes it back and never interprets it, so a renderer that reused or
invented one confuses only its own page.

The browser process answers each decided ask, later, with a **new
`ToRenderer` message** carrying that number and the response or the failure.
That message is a task (ADR 0016 § 2): its handling settles the promise and
the checkpoint after it runs every reaction. Its answer may carry new asks —
a `.then` that fetches again — and so on. Nothing waits; nothing calls back.

**An ask whose document has gone is answered by nobody.** A response that
arrives for a renderer no longer holding the page that asked is dropped by
the browser process before it is sent, as `Tabs` already refuses to ask about
a page a renderer no longer holds.

## 2. What the ask says, and what it may not

The ask carries what only the renderer knows:

- **The URL, resolved and serialised by the renderer** against the
  document's base URL, as ADR 0020 § 2 resolves a link's. A claim: it is
  parsed again.
- **The method**, normalised as Fetch normalises it; **the headers the page
  set**, with every forbidden request header already dropped as the
  specification's `Headers` guard drops it; **the body**, as bytes the
  renderer serialised; the **mode** (`cors`, `no-cors`, `same-origin`), the
  **credentials** mode, the **redirect** mode, and the **referrer policy** —
  each one of `alo-net`'s own enums, never a string the browser process has
  to interpret.

It never carries **an origin, a cause, a cookie, a tab or a document**. The
origin is the one the browser process loaded into that renderer for that
document, as ADR 0025 § 5 takes a storage key; the cause is § 3's; cookies
are the jar's.

**A renderer that sends what the bindings would have refused** — a forbidden
header, `navigate` as a mode, a body on a `GET` — is not quietly corrected.
The ask is refused and recorded as a renderer that broke the boundary
(ADR 0025 § 5's wording), because a page cannot produce one and only a
compromised renderer can.

## 3. The browser process decides, through the stack every request uses

In this order, each refusal named:

1. **The URL parses**, with `alo-url`, under ADR 0020 § 3's 2 MiB bound.
2. **The scheme is `http` or `https`.** Fetch answers `about:`, `data:` and
   `blob:` without a network, so when a page needs one they are answered in
   the renderer and never asked for: a `data:` URL is the page's own bytes,
   which the browser process does not parse (ADR 0005), and a `blob:` URL's
   bytes are the renderer's (ADR 0020 § 6). Until then the bindings refuse
   each by name. An ask that names one anyway, or `file:`, or any other
   scheme, is a network error, as in every browser.
3. **The document's own header policy allows it under `connect-src`**
   (`csp.rs`, `Purpose::Fetch`), checked against the browser process's copy
   of the headers. A policy delivered in a `<meta>` element is the renderer's
   to apply before it asks, and an objection it raises is a claim beside the
   answer, as script objections are today (`Objection`). A renderer that
   ignores its own `<meta>` policy hurts only the page that wrote it.
4. **Mixed content** (`mixed.rs`): a fetch is blockable content, so from a
   secure document to an insecure URL it is refused, never upgraded.
5. **CORS** (`cors.rs`, `preflight.rs`), with the document's origin as the
   asker: `same-origin` refuses another origin before anything is sent;
   `cors` asks first when `needs_asking_first` says so, under the preflight
   cache's top-level-site partition; `no-cors` restricts the method and
   headers to what a form could send.
6. **Cookies** from and into the jar under the document's top-level site
   (ADR 0007), only when the credentials mode allows it for this origin.
7. **The referrer** from the browser process's copy of the document's URL,
   under the ask's policy (`referrer.rs`), as ADR 0020 § 2 builds a
   navigation's.
8. **The cause by ADR 0012 § 4, from which message was answered.** An ask in
   an `Act`'s answer is the agent's (`Tabs::an_agent_acting`); in any other
   answer — a `Load`'s, or the message that carried an earlier response — it
   is the document's (`Tabs::a_page_fetching`). That is ADR 0016 § 6
   applied: a `.then` that fetches after an agent's click runs in a new
   task, and it is the page's.

Then the request is made with `Purpose::Fetch` and recorded in the session's
record like every other request, including the ones a rule refused.

**Bounds, enforced here and stated in the code.** The number of asks in one
answer and the number in flight for one document are bounded in the browser
process, whatever the renderer says, and an ask past either bound is a
network error said among the issues. The numbers land in the code with their
reasons, ADR 0014 § 9's rule, set by the frozen pages that fetch rather than
by a guess. A request body is bounded by the answer that carries it, which is
one message (`LARGEST_MESSAGE`, 64 MiB).

## 4. What comes back is filtered before it leaves

The browser process sends the response **as the page may see it**, built by
Fetch's filtered-response rules from `cors.rs`:

- **Same origin (`basic`)**: status, status text, final URL, whether it was
  redirected, every header **but `Set-Cookie` and `Set-Cookie2`**, and the
  body.
- **Cross-origin and allowed (`cors`)**: the same, with headers cut to the
  CORS-safelisted response headers and those `Access-Control-Expose-Headers`
  named (`readable`).
- **Cross-origin and not readable (`opaque`)**, and a `manual` redirect
  (`opaqueredirect`): **status 0, no headers, no URL and no body.** The
  bytes are not sent, so they are not in the renderer's memory at all —
  which is the property ADR 0005 pays a process per site for.

**A network error carries no reason to the renderer.** The page's promise
rejects with a `TypeError` whose message is the same for every failure. The
reason — the refused port, the CORS header that was missing, the rule that
refused it — is written where the person can see it and in the session's
record, never in the message the renderer is sent.

**The body crosses whole, in the same message.** Until a page needs a
response streamed (`ReadableStream`, its own item), a body is read to its end
by the browser process, under `alo-net`'s existing `LARGEST_BODY`, and sent
in one message under `LARGEST_MESSAGE`. A body too large for one message is a
network error that says so to the person. No new number is introduced: both
bounds already exist and each has its reason beside it.

## 5. A promise needs only what is built

A promise is a cell in the heap with its state, its value and two lists of
reactions; resolving it queues a job with `Engine::queue_job` (item 232), and
the checkpoint after every task runs it (item 235). **Nothing in item 233 or
234 is involved**: 233 is the loop running between messages for tasks a page
schedules for itself — a timer — and a response is not one, it is a message;
234 is frames. So the part of item 75 that `fetch` needs is **cut out of it**
as its own item, depending on 232 and 235, which are done:

the `Promise` constructor and its executor, `then`, `catch` and `finally`,
`Promise.resolve` and `Promise.reject`, resolution by a thenable through a
job of its own, and a rejection nobody handled by the end of the checkpoint
reported to the embedder the way an uncaught throw is (item 241's report). The combinators (`all`,
`allSettled`, `race`, `any` — the last on `AggregateError`, item 229),
`async`/`await` and generators stay in 75 with its dependency unchanged,
because `await` suspends a frame, which is a different mechanism.

This narrows a dependency; it does not skip one. 76 is depended on by 75 for
its job queue and its checkpoint, and those are the pieces of 76 that are
built.

## 6. `XMLHttpRequest` is the same ask, and synchronous is refused

An asynchronous `XMLHttpRequest` is this ask with a different front: the same
decision, the same filtered answer, delivered as events rather than a
promise. It is its own item, opened by a frozen page that uses one, and it
waits on event dispatch for its `load` and `readystatechange`.

**A synchronous `XMLHttpRequest` is refused by name.** It would be a renderer
waiting on the browser process, which ADR 0005 forbids, and the
specification already deprecates it on a document's main thread for the same
reason: it stops the page. A page that depends on one is a stage 3 page.

## 7. A corpus page's fetch is answered from frozen responses

A corpus case never touches the network (`LOOP.md`, stage 2 § 1), so a case
whose script fetches **states its address and its responses**: the URL the
page was served from, so that a relative URL means what it meant, and for
each URL it fetches, a response frozen beside it with where and when it was
taken. The corpus answers through the same filtering as § 4, so a frozen
cross-origin response is as opaque to the page as a live one. **A fetch the
case froze no response for is a network error**, and the case's `origin.txt`
says which, so a reference render of a page that greys out its buttons when a
file is missing is read as what the page does offline, not mistaken for what
it does live.

## What this costs

**Every answer that can run script gains a list, and a new message kind
exists in the browser-to-renderer direction.** That is the direction ADR 0005
allows, and the list is bounded.

**A response's body is copied once, across the boundary,** and held whole.
Streaming is a measurement and a page away.

**A page cannot tell why its fetch failed.** Neither can it in any other
browser, and the person can.

**A renderer cannot cache, coalesce or retry a fetch itself.** It has
nothing to do it with; the browser process's cache and pool do it for every
request alike, which is the point of *over the same stack*.

## Alternatives rejected

- **A network process, or a socket handed to the renderer.** ADR 0005 gave
  the network to the browser process, and a renderer with a socket is a
  renderer that can reach whatever the machine can.
- **The renderer filters the response.** Spectre (ADR 0005, reason 1): a
  body in the renderer's memory is readable by the page in it.
- **One fetch per answer, the last, as for navigation.** HTML aborts a
  navigation for the next; it never aborts a fetch for the next.
- **The renderer names its origin**, and the browser process checks it
  against a list. Then the list is the policy and the claim is noise; the
  browser process already knows the origin, because it loaded the document.
- **A reason in the `TypeError`.** It turns every page into a probe of the
  network the person is on.
- **Build promises after 233.** 233 waits on timers or events to close, and
  neither is what a response is.

## What this does not decide

- **`Request`, `Headers` and `Response` as page-constructible objects**,
  `body` as a stream, `clone`, `FormData`, `URLSearchParams` and `Blob`
  bodies — each its own item when a page needs it. The first build makes
  only what a fetch answers with, and reads only `ok`, `status`,
  `statusText`, `url`, `type`, `redirected`, `headers.get` and `has`, and
  `text()`; `json()` comes with `JSON` (item 73).
- **`AbortSignal`** — a page that cancels a fetch opens it; the ask's number
  is what a cancellation would name.
- **Service workers intercepting a fetch** — item 91.
- **Keep-alive fetches that outlive their document**, and `sendBeacon`. A
  request that outlives the page that made it is a tracking feature first,
  and deciding it is a decision.
- **The bounds' values** — § 3; they land in the code with their reasons.

## How we will know if this was wrong

**If a frozen page's fetch succeeds in another engine and fails here**, and
the reason is not a rule § 3 names on purpose, the decision order or the
filter is wrong, and the fix names the step.

**If a renderer is ever found holding the body of a response its page could
not read**, § 4 was not followed, and that is a security bug rather than a
design question.

**If the record says an agent fetched something a person would say the page
fetched on its own**, ADR 0012 § 4's edge is wrong, and it is fixed there.
