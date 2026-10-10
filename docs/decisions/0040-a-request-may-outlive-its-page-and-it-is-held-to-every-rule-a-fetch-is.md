# ADR 0040 — A request may outlive its page, and it is held to every rule a fetch is

**Status:** accepted
**Date:** 2026-10-10
**Context:** queue item 369, *`navigator.sendBeacon`*, opened by a page:
`alo-sites-cta`, the call-to-action section alo Sites publishes for its
customers. Its analytics script sends everything it measures through one
function:

```js
function send(body) {
  if (navigator.sendBeacon) { navigator.sendBeacon("/_alo/collect", body); }
}
```

It calls `send` from four places: its `pagehide` listener, its
`visibilitychange` listener when the page becomes hidden, a click listener,
and once for each form whose `action` starts `/f/`. Since items 364 and 373
the browser fires the first two. `navigator.sendBeacon` is absent, so the
script sends nothing, and the tests can only show what it would send through
a beacon they lend it. The item is marked **needs ADR**: ADR 0032 left
*keep-alive fetches that outlive their document, and `sendBeacon`*
undecided, because *a request that outlives the page that made it is a
tracking feature first*. It asks whether a page may send one, to whom, with
what credentials, and how it is recorded under ADR 0012.

Also read:
- ADR 0032, the whole of it: an ask is a claim in an answer (§ 1), what it
  carries (§ 2), the browser process's order of decisions (§ 3), the filter
  (§ 4), and an answer for a document that has gone, which is *answered by
  nobody*;
- ADR 0039 §§ 2–4: a page is left only through its leaving steps, as one task
  given a second, and § 4 refuses a leaving page's fetches *until item 369
  decides*, in `fetch_decide.rs` and nothing in the renderer, so that this
  decision changes one process;
- ADR 0007 (cookies partitioned by top-level site, `SameSite=Lax` when a
  site does not say) and ADR 0012 (every request says what caused it; §§ 4–7
  on who assigns it, what is kept and who may read it);
- ADR 0005 (a renderer has no network) and ADR 0024 § 2 (the window waits on
  nothing the conductor does);
- ADR 0013 § 3 (no approximate members).

And what the standards say:
- **Beacon**, *`sendBeacon(url, data)`*: the URL is parsed against the
  document's base URL, and a failure or a scheme other than HTTP(S) throws a
  `TypeError`. The request is a `POST` with `keepalive` set, credentials
  `include`, initiator type `beacon`, and mode `no-cors` unless the body's
  content type is not CORS-safelisted, when it is `cors`. A string body is
  sent as UTF-8 with `Content-Type: text/plain;charset=UTF-8`. The method
  returns `false` if the body would exceed what keep-alive requests may
  have in flight, and `true` otherwise. It never says what became of the
  request.
- **Fetch**, *keepalive*: a fetch group's keep-alive requests may have at
  most **64 KiB** of body in flight together, and a request past that is a
  network error. When a fetch group is terminated, every fetch in it whose
  `keepalive` is false is aborted. The ones that are true go on.

And the code:
- `alo-renderer`'s `fetch_decide.rs`: `leaving` and `Rule::Leaving`, which
  refuse everything a leaving page asks;
- `fetch_answering.rs`: a queued fetch whose document is no longer its tab's
  is not made, and nothing is sent;
- `tab.rs`: `Leaving::refusing`, and `Tabs::left`, which the conductor's
  `record_left` writes into the record;
- `alo-window`'s `conductor.rs`: one fetch made at a time between orders,
  and `close_everything`, after which the conductor finishes;
- `alo-bindings`' `fetch_init.rs`: `keepalive` other than `false` refused by
  name, as *outlives its document*.

And the server at the other end, read only: `alo-workplace`'s
`products/sites/alo-sites/src/serve/beacon.rs`. It takes a `POST` body of
at most 512 bytes, sets and reads no cookie, answers `204` with no body, and
says why: *`navigator.sendBeacon` cannot read a response anyway*.

## The decision in one line

A **keep-alive request** is a page's fetch that may be made after its page
is gone. **It is decided by every rule an ordinary fetch is**, and gains
nothing else. It carries the cookies the partitioned jar would send any
fetch, and the page's `connect-src` governs it. The only new thing it may do
is outlive its document: the **browser process makes it** after the page has
been left, answers it to nobody, and **writes it in the record as the
document's**. `navigator.sendBeacon` is one, built first because a frozen
page needs it. A page may have at most **64 KiB** of keep-alive body in
flight, Fetch's own number, counted by the renderer so that `sendBeacon` can
answer `false` and enforced again by the browser process. A request that did
not ask to outlive its page is still refused when asked as the page is left.
One that is still waiting when the browser closes is **not made**, and the
record says so.

## Why this is a decision rather than a chore

`sendBeacon` is three lines of Fetch: a `POST`, a mode and `keepalive`.
What is not decided is the one thing those lines add, which ADR 0032 named
and set aside. A request whose response has nobody to go to is useful only
for what it tells the server. A page sends one at the moment it is being
left, so it is a report: how long the person stayed, how far they read, what
they clicked last.

So the question is not whether the request can be made. The browser process
makes every request, and nothing in the renderer has to live on. The
question is **what a person loses by a page being able to say goodbye**, and
whether refusing it protects them from anything.

## 1. What outliving adds, and what it does not

**It adds no new identity.** A beacon is a fetch, and a page that is still
open can already make the same request with `fetch`, to the same place,
carrying the same cookies. Who the request says the person is is decided by
ADR 0007's jar, partitioned by the top-level site, with `SameSite=Lax` when
a site does not say. A beacon to another site carries that site's cookie for
*this* top-level site and no other. It cannot join one site to another,
which is the harm ADR 0007 exists to prevent.

**It adds no new destination.** A page can reach anywhere its `connect-src`
allows while it is open. Refusing a beacon to another site would be a rule
a page avoids by sending its report to its own server and forwarding it from
there. That is what alo Sites does anyway: its beacon goes to the page's own
`/_alo/collect`.

**What it adds is timing.** A server learns *when* the person left, to the
second, instead of when they last did something the page reported. That is
the whole of what keep-alive adds.

**And refusing it does not take that away.** A page that cannot report as
it is left reports while it is open instead. It sends a heartbeat every few
seconds, or it holds the person in `unload` with a busy loop or a
synchronous request, which is why Beacon was written. This engine refuses
the last two already (ADR 0032 § 6, ADR 0039 § 3). Heartbeats remain, and
they tell a server more, not less: a steady stream of requests while the
page is open, where a beacon is one request at the end. Refusing beacons
would cost the person every page's last report, a report alo itself depends
on, and buy them nothing they could notice.

So a page may send one. **It is decided in the same way, and in the same
place, as every other fetch.**

## 2. A keep-alive ask is an ask, with one more claim

An ask gains one field: **`keepalive`**, whether the page asked for the
request to outlive its document. Like every field of an ask (ADR 0032 § 2),
it is a claim. It gains the page nothing a page could not have, because the
browser process enforces the bound in § 4 whatever the renderer says. A
compromised renderer that marks every ask keep-alive gets 64 KiB of reports
made after its page is gone, which an honest page is allowed.

The browser process decides a keep-alive ask by **ADR 0032 § 3, unchanged**:
the URL, the scheme, the document's own `connect-src`, mixed content, the
mode, the cookies under the document's top-level site, the referrer from its
own copy of where the document is, and the cause by which message it was
answering. Then it bounds it (§ 4) and queues it as any fetch.

**A beacon is recorded as a beacon.** The record gains a purpose, **beacon**,
beside *fetch*, *style* and the rest (`alo-net`'s `Purpose`). Fetch tells
the two apart by initiator type, and a person reading what a page sent
should be able to tell its last report from its ordinary traffic.
`connect-src` governs it, as CSP says it governs `sendBeacon`. A keep-alive
`fetch` is still a *fetch*.

## 3. It outlives its page, and only that

**While the page is held**, a keep-alive request is made and answered like
any fetch, and its answer is a task like any other (ADR 0032 §§ 1 and 4).
`sendBeacon` has no promise, so its bindings ignore what the answer says and
use it only to know the request is no longer in flight (§ 4). The browser
process filters the answer as it filters any, so nothing crosses that the
page could not read with `fetch`.

**Once the page has gone** — left by ADR 0039 § 2's steps, or replaced by a
`Load` — **a keep-alive request is still made.** This is the one exception
to *an ask whose document has gone is answered by nobody* (ADR 0032 § 1),
and it is the exception exactly: it is **made**, and **answered by nobody**.
The response is read, filtered and recorded as any response is, and nothing
is sent to any renderer. Every other request still waiting for that document
is not made, as today.

**A keep-alive ask made during the leaving task is decided, not refused.**
ADR 0039 § 4 refused all of them, as a placeholder for this decision.
`Rule::Leaving` now refuses only asks that did not ask to outlive their
page. That is what Fetch does to a fetch group being terminated: every
request without `keepalive` is aborted. A page cannot tell the difference
between a fetch refused there and one aborted there, and the record says
which rule refused it.

**What a left page asks is the document's.** Leaving is not an agent's
verb, so ADR 0012 § 4 makes every ask in a `Left` answer, or in the left
page's part of a `Loaded`, the document's. That is what `Leaving::refusing`
assigns today. A beacon sent from an agent's click while the page is held is
the agent's, by ADR 0032 § 3, and it is made at once like any fetch.

**When the browser closes, what is still waiting is not made.** The
conductor finishes after `close_everything` (ADR 0024 § 2). The keep-alive
requests still waiting in its queue at that moment are **written into the
record as not made, because the browser closed**, and are not sent. A
browser the person has told to close does not keep talking to the network
for a page they have left, and nothing is written to disk to send at the
next start. A tab closed while the browser goes on, and a page replaced by
the next, send their beacons. Closing the window loses whatever beacons
were still waiting, and the record says it did.

**A dead renderer sends nothing.** An ask crosses only in an answer, so a
renderer that crashed or was stopped for silence never sent its page's last
asks. That is ADR 0039 § 2's *what is not a leave*, and no page is promised a
last word by a process that died.

## 4. 64 KiB in flight, counted twice

Fetch bounds a fetch group's keep-alive requests at **64 KiB of body in
flight together**. That is the standard's number, and pages are written
against it. A beacon larger than that, or one that would take the total past
it, is not sent.

**The renderer counts, so that the page is told.** `sendBeacon` must answer
`true` or `false` before it returns, and only the renderer can answer then.
The document cell counts the bodies of its keep-alive asks whose answers
have not arrived. `sendBeacon` answers `false` and asks nothing if its body
would take that past 64 KiB. Otherwise it records the ask and answers
`true`. An answer arriving for one of these asks takes its bytes off the
count.

**The browser process counts too, because the renderer's count is a
claim.** It keeps, per document, the bodies of keep-alive requests decided
and not yet made. An ask that would take that past 64 KiB is refused by a
rule of its own, named in the record, and is not made. It keeps counting
after the page has gone, until each request is made. An honest renderer
never meets this rule, because it answered `false` first.

**The other bounds are unchanged.** ADR 0032 § 3's 64 asks in one answer and
64 waiting for one document cover keep-alive asks too, empty bodies
included, so a page cannot leave behind a flood of empty beacons. Each
request is one of the conductor's, made one at a time between orders. A
left page's beacons wait their turn behind everything already queued, and
cost the tabs still open no more than any page's fetches do.

## 5. What `sendBeacon` is, in this engine

`Navigator.prototype.sendBeacon(url, data)`, by Beacon's steps:
- `url` is converted to a string and parsed against the document's base
  URL. A failure throws a `TypeError`, and so does a scheme that is not
  `http` or `https`.
- `data` is `null` or omitted for no body, or a string, sent as UTF-8 with
  `Content-Type: text/plain;charset=UTF-8`. That type is CORS-safelisted,
  so the mode is `no-cors`. Web IDL converts any other value to a string,
  because none of `BodyInit`'s object types exists in this engine yet.
  That is the correct conversion today, not a substitute for one. When
  `Blob`, `FormData`, `URLSearchParams` or a buffer source is built, the
  item that builds it adds it to `sendBeacon`'s body as well. A body whose
  type is not safelisted, which only those can have, makes the mode `cors`.
- The ask is a `POST` with `keepalive`, credentials `include`, redirect
  `follow`, and the document's referrer policy.
- It answers `false` by § 4's count and `true` otherwise. It never says
  what became of the request: Beacon does not, and ADR 0032 § 4 gives a page
  no reason for a failure anyway.

A beacon asked in a task is carried in that task's answer, as every ask is.
Nothing is sent from inside the call.

## 6. The cut

- **369. `navigator.sendBeacon`.** Builds:
  - the `keepalive` field on `FetchAsk`, across the wire, with hostile
    bytes refused;
  - `Purpose::Beacon` in `alo-net`, governed by `connect-src`;
  - the browser process's keep-alive count and its rule (§ 4);
  - `Rule::Leaving` narrowed to asks without `keepalive`, and a leaving
    page's keep-alive asks decided by `fetch_decide::decide`;
  - `fetch_answering` making a keep-alive fetch whose document has gone,
    and answering nobody;
  - the conductor recording what was still waiting as not made when the
    browser closed;
  - `sendBeacon` on `Navigator.prototype`, with the renderer's count (§ 5).

  *Closes when:*
  - in `tests/alo_sites_cta.rs`, **with no beacon lent**, leaving the page
    makes the browser process send `POST
    https://nordwind.alosites.com/_alo/collect` with the body
    `d=1000&p=%2F&w=800` and then with `t=0`, each with `Content-Type:
    text/plain;charset=UTF-8`. Each is written in the record as a beacon
    caused by the document, and is made after the page has gone;
  - hiding the page sends the same two while it is held, and their answers
    reach the renderer and free the count;
  - a page's `fetch` asked as it is left is still refused by
    `Rule::Leaving`, beside the beacon that is made;
  - `sendBeacon` with a body of 65 536 bytes answers `true`, and a second
    of one byte answers `false` until the first is answered. One of
    65 537 bytes answers `false` and asks nothing;
  - an unparseable URL and a `data:` URL throw a `TypeError`;
  - an ask claiming `keepalive` past the browser process's count is refused
    by name, and the next renderer message is still read;
  - a beacon the page's `connect-src` forbids is refused by name;
  - a beacon still waiting when the conductor closes everything is
    recorded as not made, in a conductor test;
  - every script runs ordinarily and under `Heap::stress`.
- **375. `fetch(…, { keepalive: true })`.** *Depends on 369.* The same
  ask from `fetch`, under the same count. Fetch's own refusal of a stream
  body does not arise, because no page can make one yet. *Opened by* a
  frozen page that uses it: none does. Until then the bindings go on
  refusing it by name, as *outlives its document*. *Closes when:* such a
  fetch is made after its page is left, and a 64 KiB total rejects the next
  one, in tests.

## What this costs

- **A page's last report reaches its server.** For alo Sites that is the
  point. For a tracker it means the moment a person left. § 1 explains why
  that is no more than a page can learn without it.
- **A request is made for a page nobody is looking at**, after it has
  gone, at most 64 KiB of it per page. The person can see it in the record
  as the page's beacon.
- **Closing the window loses whatever beacons were still waiting.** An
  analytics script loses its report when the person closes the window
  rather than the tab. That is the right way round.
- **One more field on an ask, one more purpose and one more rule.**

## Alternatives rejected

- **Refuse every beacon, as ADR 0039 § 4 does for now.** It protects
  nothing a page cannot learn with heartbeats, and it costs alo's own
  sites, and every page that saves state as it is left, their last word,
  silently. § 1.
- **Beacons only to the page's own site.** A page forwards from its own
  server, so this refuses only the honest page that does not bother. ADR
  0007 already decides what a request to another site may say about the
  person.
- **Beacons without credentials.** Beacon says `include`. Sending none
  would be an approximate member (ADR 0013 § 3), and the cookies it would
  carry are already partitioned and `Lax` by default (ADR 0007).
- **Keep the renderer alive to send them.** A renderer has no network
  (ADR 0005), and keeping a left page's process for its reports keeps
  exactly what ADR 0039 § 3 bounds to a second.
- **The renderer drops what is past the count, and the browser process
  trusts it.** The count is a claim. ADR 0039 § 4 put the decision in the
  browser process so that a compromised renderer's asks are still seen.
- **Finish every waiting beacon before the browser closes**, or save them
  to send at the next start. A browser the person told to close keeps
  working for pages they have left, or writes their last reports to disk
  for later. § 3.
- **A new number of our own instead of 64 KiB.** Pages are written against
  Fetch's, and a smaller one would make `sendBeacon` answer `false` where
  every other engine answers `true`.

## What this does not decide

- **A person turning beacons off**, for one site or for all. That is a
  per-site setting (item 121). A switch would change only *when* a page
  reports, not *whether* it can (§ 1), so it is not owed before then.
- **`Blob`, `FormData`, `URLSearchParams` and buffer bodies.** Each is its
  own item, opened by a page, and § 5 says what it adds to `sendBeacon`.
- **`fetchLater`**, which is a request deferred until a page is left. It is
  a different decision about deferral, opened by a page that uses it.
- **A beacon from a worker** (item 91).
- **Service workers seeing a beacon** (item 91).

## How we will know if this was wrong

**If a frozen page's beacon reaches its server in another engine and not
here**, and no rule named in ADR 0032 § 3 or here refused it, § 3 or § 4 is
wrong, and the fix names the step.

**If a page is found that learns more about a person from a beacon than it
could from heartbeats** — an identity, or a site joined to another — then
§ 1's argument is wrong, and beacons go back to being refused until it is
mended.

**If the record shows a request made for a left page that did not ask to
outlive it**, § 3's exception has widened, and that is a bug.
