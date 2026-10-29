# ADR 0060 — Chrome's own network carries a request, and the browser process decides it before it leaves

**Status:** accepted
**Date:** 2026-10-24
**Context:** ADR 0058 (Chromium is a renderer we drive), whose § 4 keeps the
grants and the record in the browser process and does not say whose network
a Chrome request uses; ADR 0005 (the browser process owns the network);
ADR 0007 (partitioned cookies); ADR 0008 (the machine's resolver); ADR 0011
(the cache); ADR 0012 (every request says what caused it); ADR 0020 (a page
asks to go somewhere and the browser process decides); ADR 0030 (the browser
says what it is); ADR 0052 (the agent's gate and the person's gate); ADR 0055
(the agent socket); queue items 497, 620 and 629

## The decision in one line

On the product path a request a page in Chrome makes is **carried by
Chrome's own network** — its TLS, its trust, its cookies, its cache, in the
profile this browser made for it — but **no request leaves Chrome until the
browser process has seen it and let it go**: every target of the tab is held
until its requests are paused, every paused request is judged by our rules
and recorded on our side of the pipe, an agent naming a site it was not given
is refused before Chrome is sent a single command, and what Chrome will not
let us hold — a WebSocket — is told of, recorded as not decided, and named.

## Why this is a decision rather than a consequence of 0058

ADR 0005 put the network in the browser process, and every network ADR since
— cookies partitioned by two sites (0007), the machine's resolver (0008), what
the cache may write (0011), a cause on every request (0012), the browser's own
name (0030) — is a promise `alo-net` keeps. ADR 0058 § 4 keeps the grants and
the record on our side of the pipe and is silent on the bytes. So either
`alo-net` goes on carrying them, with Chrome handed each response, or Chrome
carries them and those promises are set aside on this path. That is a choice
between two things this repository has written down, and item 629 asked for it
in an ADR if it was one. It is.

## What was measured

Chrome 155.0.8059.86, headless on a new profile, spoken to over
`--remote-debugging-pipe` exactly as `chrome.rs` starts it, by a Python probe
against a local server of its own that logged every request it received. The
probe page asked for a style sheet that imports another and a font, a
picture, a script, a `fetch`, an `XMLHttpRequest`, a dedicated worker that
fetches, a service worker that fetches as it installs, a WebSocket, an
`EventSource`, a beacon, a prefetch, a same-site frame and a cross-site frame
(`127.0.0.1` and `localhost`), each of which fetches. Every request was paused
with `Fetch.enable` for `*` and continued, and what the server saw was compared
with what was paused.

1. **On the tab's own session alone, four requests escape.** Paused: the
   document, both style sheets, the font, the picture, the script, the
   `fetch`, the `XMLHttpRequest`, the `EventSource`, the beacon (`Ping`), the
   prefetch, the favicon, both frames' documents, the same-site frame's
   `fetch`, the dedicated worker's script *and* its `fetch`. Reached the
   server without being paused: **the service worker's script, the service
   worker's own `fetch`, the cross-site frame's `fetch`** (it is another
   process and another target) **and the WebSocket**.
2. **Held and paused target by target, one escapes.** With
   `Target.setAutoAttach` on the tab's session — `waitForDebuggerOnStart`,
   flattened — and on each target it attaches, and `Fetch.enable` sent to each
   before it is let go, the cross-site frame, the dedicated worker and the
   service worker are each told of, and every request of every one of them is
   paused. **Only the WebSocket still reaches the server unpaused.**
3. **A WebSocket cannot be held or stopped over CDP.** It never appears in
   `Fetch.requestPaused`. `Network.setBlockedURLs` with `ws://*`, `*/ws`, and
   even `*` — which blocked every other request on the page, each reported
   `blockedReason: inspector` — did not stop it: the socket opened and the
   server saw both handshakes. The new `urlPatterns` form refuses `*` as a
   pattern. What CDP does give is **word of it before its handshake**:
   `Network.webSocketCreated`, with its address, then
   `webSocketWillSendHandshakeRequest`.
4. **A paused request carries Chrome's cookies.** After the document set
   `fromserver=1`, every same-site request paused at the request stage
   carried `Cookie: fromserver=1`, and the cross-site frame's carried none.
5. **A response we fulfil is still Chrome's response.** A document answered
   with `Fetch.fulfillRequest` carrying `Set-Cookie: fromfulfil=1`: the page's
   `document.cookie` read `fromfulfil=1`, and the next request, continued to
   the real server, sent it. A cross-site `fetch` we fulfilled with no
   `Access-Control-Allow-Origin` was refused to the page as a CORS failure.
   So Chrome keeps its own jar and its own CORS over bytes we hand it.
6. **A fulfilment is one message.** `Fetch.fulfillRequest` takes the whole
   body, base64-encoded, in one command; nothing in the domain streams a
   response *into* Chrome. A response `alo-net` fetched would be held whole in
   this process and written whole down the pipe — a video, a large download, a
   long `EventSource` alike.
7. **Every hop of a navigation is paused.** A document at `127.0.0.1`
   answering `302` to `localhost` was paused, then the hop to `localhost` was
   paused again as a `Document` carrying `redirectedRequestId`. So the browser
   process sees where a navigation is going at each hop, before it goes.
8. **Chrome names itself.** The server read `User-Agent: Mozilla/5.0
   (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko)
   HeadlessChrome/155.0.0.0 Safari/537.36`.

The probe pages were scratchpad files and are described above well enough to
rebuild; nothing here is a claim about speed.

## 1. Chrome's network carries the bytes

**`alo-net` does not fetch for Chrome**, because measured it cannot do so
honestly:

- It **could not carry everything.** A WebSocket never reaches `Fetch` (3),
  so a page with one would be served by two networks, and the one with no
  rules of ours would be the one carrying a live, two-way channel.
- It **would not own the jar it fills.** Chrome stores the cookies of a
  response we fulfil and sends them on requests we let it make itself (5),
  so ADR 0007's partitioning would be one of two jars, and the page would read
  the other. Making Chrome keep none would mean deleting cookies behind it on
  every response, which is a race and not a policy.
- It **could not stream** (6). Every response held whole in the browser
  process and written as one message is a bound we would either set and
  break pages at, or not set and give a page a way to fill our memory.
- It **would not be what the page is checked against.** Chrome applies its
  own CORS to what we hand it (5), so `alo-net`'s CORS would run as well and
  decide nothing.

**A middle road — `alo-net` for documents, Chrome for the rest — is
rejected** for the same reasons at a smaller scale: one page, two networks,
two jars, two sets of trust, and a person told neither which carried what.

So Chrome's network carries the requests of a page in Chrome, in the profile
`chrome.rs` makes for it — new, `0700`, and gone with the process — so its
cookies and its cache outlive nothing.

## 2. The browser process decides every request it can be shown

Chrome carrying the bytes is not Chrome deciding which bytes. ADR 0005's
privilege boundary survives as a **gate on Chrome's network**, kept here:

- **Every target of a tab is held before it can ask for anything.** The tab's
  session auto-attaches its frames and workers, waiting for the debugger, and
  each one attached does the same; `Fetch.enable` for every address reaches
  each before it is let go (measured: 2). A target that cannot be held is a
  target whose requests escape, so a tab whose target could not be held is not
  let go: it is closed, and that is an error by name.
- **Every paused request is judged, then continued or refused.** The rules
  are ours and in safe Rust: an address is parsed as untrusted, and a scheme a
  page may not send a tab to is refused by name exactly as ADR 0020 refuses it
  on the engine path. What Chrome is let go to is `Fetch.continueRequest`;
  what is refused is `Fetch.failRequest`, never a fulfilment pretending to be
  a server.
- **Every request is recorded, with its cause, on our side of the pipe**
  (ADR 0012): the person, the document, or the agent action whose work made
  it, assigned here from *what this process was doing* — never from anything
  Chrome or the page said. A navigation is recorded at each hop (7), so a
  redirect is a line of its own.
- **A WebSocket is told of, recorded as not decided, and named.** It cannot
  be held (3); `Network.webSocketCreated` arrives before its handshake, and
  the record says a socket was opened to that address *and that this browser
  did not decide it*. The product path says so where it says what it cannot
  enforce. If a page's socket must one day be refused, the escalation is ADR
  0058 § 5's — a patch set, in an ADR of its own — not a claim made around it.

## 3. The agent's gate is ours, and it is shut before Chrome hears anything

ADR 0055 and item 497: a site an agent was not given is refused **here**.
On the product path that means:

- **An agent naming an address outside its sites is refused as
  `NotGranted`, by site, before a single command is written** — no target
  created, no navigation asked for, nothing paused, because there is nothing
  for Chrome to have heard of.
- **A page that goes somewhere else on its own is not stopped by the agent's
  gate.** A redirect, a page navigating itself, a person following a link:
  the tab goes, as it does on the engine path, and an agent asking for that
  tab is refused by the site of the page it **now** shows — which is
  `serving.rs`'s rule and item 630's work. ADR 0052: the agent's gate and the
  person's gate are two gates; narrowing an agent never narrows a person's
  own browsing.

## 4. What is knowingly set aside on this path

Each of these holds for the **engine** and does **not** hold for a page in
Chrome. ADR 0058 § 6 forbids saying otherwise, and the product says which
renderer is in use (item 622) so that a person can tell which list applies.

| Promise | On the engine path | On the product path |
|---|---|---|
| **Trust** (`alo-net`'s `Trust`) | the roots `alo-net` is given | Chrome's own certificate verifier and roots |
| **Cookies partitioned** (ADR 0007) | two keys, ours | Chrome's jar and Chrome's third-party policy, in a profile that ends with the process |
| **The machine's resolver** (ADR 0008) | the system's, never silently replaced | the system's resolver, `getaddrinfo`, for every name — Chrome's own DNS client switched off (item 650, fourth amendment) and Chrome's own encrypted DNS turned off in the profile it is given — Chrome 155 starts in *automatic* secure DNS, which upgrades a resolver it recognises on its own, and is pinned off (item 635, amended 25 October) |
| **The cache** (ADR 0011) | partitioned, never written when it must not outlive the session | Chrome's, in the same profile, so nothing outlives the session; a person's own profile, if ever, is a decision of its own |
| **The browser's name** (ADR 0030) | `alo/<major>.<minor>` | Chrome's own string (8) — a page deciding what it may send is told the engine that will render it, which is the truth; sending ours over Chrome's rendering would claim an engine that is not there |
| **CORS, mixed content, CSP** | `alo-net`'s | Chrome's, which applies them even to bytes handed to it (5) |

What is **not** set aside: the record (§ 2), the grants (§ 3), the refusal of
a scheme a page may not send a tab to (§ 2), and that nothing a page or Chrome
says assigns a cause.

## Alternatives rejected

- **`alo-net` fetches and Chrome is handed every response** (§ 1): it cannot
  carry a WebSocket, does not own the jar it fills, cannot stream, and runs a
  CORS that decides nothing.
- **Documents through `alo-net`, the rest through Chrome** (§ 1): two networks
  for one page.
- **Chrome's network with nothing of ours in front of it** — every request
  continued unseen: it gives up ADR 0012's record and ADR 0020's refusals for
  no gain, since pausing costs a message each way and is what 620 already
  does for a frozen page.
- **Refuse a page that opens a WebSocket**: there is no way to refuse it
  before it opens (3), only to close the tab after, which punishes the person
  for a page's choice and still lets the handshake out.

## Consequences

- Item 629 is decided and stays open until the gate is built: **633** opens a
  page from the network in a tab of Chrome — every target held and paused,
  every request judged and continued or refused, an agent's off-site address
  refused as `NotGranted` before any command; **634** writes every request of
  a Chrome tab into the record with its cause, each navigation hop a line,
  and a WebSocket recorded as not decided; **635** measures whether Chrome
  replaces the machine's resolver and pins it if it does.
- `chrome_tab.rs`'s frozen tab is the special case of § 2 where every
  request but the page's own is refused; it does not change.
- `docs/features.md` and the roadmap say, beside the Chrome renderer, that
  its requests are carried by Chrome's network under the browser process's
  gate, and which of the network promises do not hold there.

## What this does not decide

- How a person's own browsing in Chrome is recorded beyond ADR 0012's lines —
  what a person is shown of it is 623–625's.
- A persistent profile for a person — their sign-ins kept between runs. It
  would make Chrome's jar and cache outlive the process, against ADR 0011, and
  is a decision of its own if anybody asks for it.
- Anything about the engine path, where `alo-net` remains the network.

## Amendment, 2026-10-25 — which of a frame's two documents a request is

*Queue item 647.* § 2 says a cause is assigned from what this process was
doing, never from anything Chrome or the page said, and that stays. What
this amendment settles is narrower: **which document** a request belongs
to, when a frame holds two.

Between a navigation's first hop and its commit a frame holds the document
it had and is getting another, both minted by this process. Item 634 took
every request paused in that window as the old document's, on a
measurement that Chrome tells of the commit before anything the new
document asks for. Under load that is false — measured against Chrome 155,
10 requests of a new page in 100 navigations under 96 busy processes, and
the gate's own failure three times — because Chrome pauses a request in its
browser process and tells of a commit from the page's renderer, through two
channels with no order between them. The new page's stylesheet was written
as the old page's, and as the agent's action in it.

**Decided:** Chrome's word for a request's *loader* is taken as its
`frameId` already is — as where a request comes from, never who wanted it.

- A navigation's first hop names its `networkId`, which is the loader of
  the document it commits (through every redirect, measured). A request
  paused in the window is let go at once — its rule depends only on its
  address — and its line waits for `Network.requestWillBeSent`, whose
  `requestId` is the paused request's `networkId` and whose `loaderId`
  names the document that made it.
- **A loader chooses between the two documents this process minted, and
  nothing else.** The arriving document's loader makes the line that
  document's, whose cause is the navigation's; any other loader, none, or a
  request never told of, leaves the line the cause it had when paused.
  Nothing Chrome says mints a document or names a person or an action. A
  renderer that lied could move its request from the page being left to
  the page arriving — whose chain leads back to the same navigation — and
  nowhere else.
- **When to stop waiting is decided by the renderer, not a clock.** Once
  the commit is read, the renderer that told of it is sent `Network.enable`
  again; it answers after every request it told of before (10 of 10 under
  load). A line still waiting then is one Chrome will never name a loader
  for — as Chrome 155 often does for an old page's requests as it is left
  — and keeps its cause. A cause that depended on how long this process waited would be
  the thing ADR 0012 refuses: one answer on a quiet machine and another on
  a busy one.

**Not decided here:** a request with no `networkId` (a beacon, a
`keepalive` fetch) has no loader to name and stays the document the frame
holds when it is paused; and an old page's request paused *after* the new
page's commit is read is still taken as the new page's — the same race the
other way round, queue item 657.

## Second amendment, 2026-10-25 — a request Chrome names no id for

*Queue item 657.* The first amendment left two things open, and they turn
out to be one: a request with no `networkId`, and an old page's request
paused after the new page's commit is read.

Measured against Chrome 155.0.8059.86 the same day, a page sending
beacons, `keepalive` fetches, plain fetches and pictures from `pagehide`,
`unload` and `visibilitychange` as a link was followed: **all 391 of
those requests, under 96 busy processes, were paused with no
`networkId`**, none was ever told of by `Network.requestWillBeSent`, and
312 were paused after the new page's commit was read — every one of them
when the new page was from another site, on a quiet machine too. **Every
request of a living page had one**: 240 of the new page's, its own
beacons and `keepalive` fetches among them, and 31 kinds of request on a
page asking for everything it can, headless and shown — service worker,
shared and dedicated worker scripts, preloads, fonts, media, a ping, a
form's submission.

**Decided:** the absence of a `networkId` is taken as a loader is — as
which of a frame's two documents asked, never who wanted it. A request
paused with none, through a frame's session, is the document the frame
**left**, from the commit of the one it holds until its next navigation's
first hop; before that commit, or with no document left, it is the one the
frame holds, as before. Nothing is waited for: the line is written as it
is paused. A renderer that could withhold the id from a living page's
request would move that request to the page before, in the same frame,
whose chain the page it holds leads back through — and nowhere else.

**Not decided here:** a shared or service worker's request with no
`networkId` stays the page's, since no such request was measured; and a
living page's request with none, which Chrome 155 was not seen to make,
would be written as the page left's until the next navigation.

## Third amendment, 2026-10-29 — what Chrome asks for of its own accord

§ 2 decides every request Chrome can show this process; what Chrome asks
for itself — updates, a variations seed, account and model checks — it
never shows, and a browser-level `Fetch` does not see it either. ADR 0073
decides that Chrome is started with the switches that stop it, measured,
and names the two kinds that still leave (`ListAccounts` and an on-demand
component install) as not decided, as § 2 names a WebSocket (item 649).

## Fourth amendment, 2026-10-29 — every name asked of the system's resolver

§ 4's resolver row read *the system's nameservers, asked by Chrome's own
DNS client*. That is not the system's resolver: on macOS the system's
resolver also carries a VPN's scoped resolvers (`/etc/resolver/<domain>`)
and the system's own encrypted DNS, and Chrome's client honours them only
as far as it reads the same configuration (item 650).

Measured against Chrome 156.0.8078.73 the same day, from its net log, on
this machine (resolver its router, `192.168.178.1`): as started by item
649, Chrome looked up `accounts.google.com`, `update.googleapis.com` and
`edgedl.me.gvt1.com` with its own client — `HOST_RESOLVER_DNS_TASK`, DNS
transactions to the router, asking for `HTTPS` records beside `A`. With
`--disable-features=…,AsyncDns`, every lookup was a
`HOST_RESOLVER_SYSTEM_TASK` — `getaddrinfo` — and there was no DNS
transaction at all, the same addresses answered. Chrome 156 reads only the
**last** `--disable-features` it is given: a second switch carrying
`AsyncDns` alone turned ADR 0073's features back on, so it is one more
name in the same switch.

**Decided:** Chrome is started with its own DNS client off, and every name
is asked of the system's resolver — ADR 0008's default, by construction
rather than by agreement. Asserted by
`tests/chromes_resolver_is_the_machines.rs`, which fails with the switch
taken out (six lookups by Chrome's own client, eighteen transactions).

**What it costs.** Chrome asks no `HTTPS` records any more: what such a
record would have told it — a server's encrypted client hello
configuration, an upgrade to `https` before the first request — it does
not learn from DNS, as any program asking `getaddrinfo` does not. ADR 0008
already chose the machine's resolver over a browser's own; this is that
choice's price, named.

**Not measured here.** A scoped resolver and the system's own encrypted
DNS: setting either up needs an administrator, which a test does not have.
They are the system's resolver's to honour, and every name now goes to it;
that each is honoured is measured by item 698. A `.local` name: Chrome
asked the system's resolver for one even with its own client on, but the
machine's own `.local` name — answered with `127.0.0.1` among others by
`dscacheutil`, `curl` and Python's `getaddrinfo` in milliseconds — was
unanswered in Chrome after 9 s with its own client on, 12 s with its
sandbox off, and 25 s with its own client off, a retry at 6 s each time;
this Mac's network settings do not deny Chrome multicast. Why is item 697.

