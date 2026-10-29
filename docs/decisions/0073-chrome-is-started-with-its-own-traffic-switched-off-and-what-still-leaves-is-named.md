# ADR 0073 — Chrome is started with its own traffic switched off, and what still leaves is named

**Status:** accepted — built (queue item 649; decided in iteration 442, built in 443)
**Date:** 2026-10-29
**Context:** ADR 0060 (Chrome's own network carries a page's requests and
the browser process decides each before it leaves), whose § 2 decides every
request the browser process *can be shown*; ADR 0012 (every request says
what caused it, and there is no `Internal`); ADR 0008 (the machine's
resolver); ADR 0061 (a page in Chrome is shown in an `--app` window of its
own); ADR 0058 § 3 (a private profile, gone with the process); queue items
633, 635 and 649

## The decision in one line

Chrome is started with the switches that, measured, stop what it asks for of
its own accord — background networking, component updates, and six named
features — so that of the dozen kinds of request it makes on an empty profile,
**two still leave**, and those two are named here, with why they are not
stopped yet; stopping them too is measured to work, costs the `--app` window
ADR 0061 shows a page in, and is cut as its own item (696) rather than paid
for by implication.

## Why this is a decision rather than a chore

Item 649 was found building 635: a Chrome started as the browser starts it
asks Google for a dozen things before any page is open, and none of it is a
tab's, so none of it is paused, judged or recorded. ADR 0060 § 2 decides
what Chrome can show us; these requests it never shows. ADR 0012 § 2 says an
engine-made request with no antecedent is the one line nobody can account
for. So something is set aside, and which, at what cost, is a decision.

The costs pull apart. Component updates carry Chrome's certificate
revocation lists and Safe Browsing's; switching them off on a profile that
lives one session is a small loss, but it is a loss, and it should be
written down rather than discovered.

## 1. What was measured

Chrome 156.0.8078.73 on macOS 26.5.2, on a profile made as `chrome.rs`
makes one (`Local State` pinning DNS off, item 635), spoken to over
`--remote-debugging-pipe`, its net log written by
`--log-net-log` at the default capture mode. Each run was twelve seconds,
with a page — a document, a style sheet, a picture — at a name of the
probe's own mapped to a local server (`--host-resolver-rules`), so that the
page's requests are told from Chrome's by host. Requests are read from the
net log's `REQUEST_ALIVE` events with their traffic annotation and final
error; connections from its socket events. Probes were scratch files; every
run below was made at least twice, headless and shown (`--app=data:,`)
alike unless said.

1. **As the browser starts it today**, Chrome asks: a variations seed
   (`clientservices.googleapis.com`); extension and component updates
   (`clients2.google.com`, `update.googleapis.com`), then **downloads**
   `.crx` files (`*.gvt1.com`, `clients2.googleusercontent.com`) and has one
   verified (`www.googleapis.com/chromewebstore`); on-device model
   manifests and models (`optimizationguide-pa.googleapis.com`); Safe
   Browsing's OHTTP keys (`www.gstatic.com`); network time
   (`clients2.google.com/time`); `accounts.google.com/ListAccounts`; and
   AI-mode eligibility (`www.google.com/async/folae`), after a preconnect
   to `www.google.com`. A dozen kinds, about twenty requests, each
   resolved by the machine's resolver and carried to Google.
2. **A browser-level `Fetch.enable` sees none of them.** With `*` paused on
   the browser's own session, what was paused was the page's four requests
   and two of Chrome's own component extensions' scripts; every request
   above went out unpaused. Item 649's second candidate is rejected:
   Chrome's services do not reach the DevTools interception at all.
3. **Switches stop most of it.** `--disable-background-networking` stops
   the variations seed, extension updates and their downloads and the
   web-store verify. `--disable-component-update`
   stops scheduled component updates. `--disable-features=` with
   `AimEnabled` stops AI-mode eligibility, `PreconnectToSearch` the
   preconnect to the default search engine,
   `NetworkTimeServiceQuerying` network time (`BrowserNetworkTimeQueriesEnabled`,
   tried alone, does not), `SafeBrowsingHashPrefixRealTimeLookups` Safe
   Browsing's keys (background networking alone does not), and
   `OptimizationHints` and `OptGuideOnDeviceModel` the model fetches. Writing preferences into the
   profile — `omnibox.ai_mode_settings`, a default search engine of
   nobody's — changed nothing.
4. **Two kinds still leave, headless and shown alike.** One is
   `accounts.google.com/ListAccounts`, a `POST` asking which Google accounts
   this profile is signed into (an empty one: none). The other is an
   **on-demand** component install — `installsource: ondemand`, so not the
   scheduled updates the switch stops — of
   `ceofaddefefcbblgcgnibnonglccbfja`, the on-device models' manifest
   (`OptimizationGuideModelsManifest`), whose request names the operating
   system and its version, the architecture and the memory, and whose
   answer is a download from `edgedl.me.gvt1.com`. No switch or feature
   tried stopped either. With every switch in § 2, these two and nothing
   else of Chrome's own left, in four runs, two headless and two shown.
5. **A proxy that does not exist stops all of Chrome's own.** Started with
   `--proxy-server=http://127.0.0.1:9`, every request of Chrome's own failed
   at the proxy (`ERR_PROXY_CONNECTION_FAILED`, −130) before any name was
   resolved — no lookup, no connection, ListAccounts and the on-demand
   install included.
6. **A page is let through by a context of its own.** A browser context
   made with `Target.createBrowserContext` and `proxyServer: "direct://"`
   loads its page directly — the local server saw every request — while a
   context made without it inherits the dead proxy and its page fails at
   −130 too. In the direct context, AI-mode eligibility and the search
   preconnect come back (they belong to each profile, and that context is
   one); with `AimEnabled` and `PreconnectToSearch` off as well, **nothing
   left the machine** in three runs: no lookup, no connection, only Chrome's
   IPv6 reachability check, a UDP `connect` to `[2001:4860:4860::8888]:443`
   that sends no packet and failed unreachable.
7. **That context's window is not an `--app` window.** Shown, a target
   created in the new context opens an ordinary Chrome window — a tab
   strip, an address bar, a bookmark star — and not the window ADR 0061
   shows a page in, which has neither. CDP has no way to put a page of
   another context into `--app` mode.

## 2. What is decided

- **Chrome is started with these switches**, headless and shown alike:
  `--disable-background-networking`, `--disable-component-update`, and
  `--disable-features=AimEnabled,PreconnectToSearch,NetworkTimeServiceQuerying,OptimizationHints,OptGuideOnDeviceModel,SafeBrowsingHashPrefixRealTimeLookups`.
  Each is in the list because a measurement in § 1 stopped something with
  it; a switch that stopped nothing is not added in hope.
- **What still leaves is named, and the record says so.** ListAccounts and
  the on-demand models manifest are the two requests Chrome sends that this
  browser neither decides nor records, on the same footing as ADR 0060
  § 2's WebSocket: told of here, by name, as not decided. The product path
  says so where it says what it cannot enforce (ADR 0058 § 6).
- **A test holds the list.** Item 649 closes on a test that starts Chrome as
  the browser starts it, behind a stand-in adding the net log's flags (as
  `chromes_resolver_is_the_machines.rs` does), loads a page from a local
  server, and asserts that every request in the net log is the page's or
  one of the two named here. A Chrome that updates itself and starts asking
  for something new fails that test, and that is the point: a new request
  of Chrome's own is something a person should learn about from this
  repository's gate, not from a packet capture.

## 3. What is not decided here, and why

**Stopping the last two** is measured to work (§ 1, 5–6) and is not taken,
because its price is ADR 0061's window: every page would live in a context
whose window has a tab strip and an address bar, on a screen whose address
is meant to be ours. Whether that is worth two requests, whether the page
is shown some other way, or whether Chrome is patched (ADR 0058 § 5's
escalation) is a judgement about what a person sees, and it is cut as item
696 for an ADR of its own rather than made inside this one. The dead proxy
must also be a port nobody can answer on — a port the browser process
itself holds and refuses — not a number assumed empty.

## 4. What it costs

- **Chrome's components do not update while it runs.** The revocation list
  (CRLSet) and Safe Browsing's lists are what Chrome was installed with or
  last fetched on the person's own profile — never the profile this browser
  makes, which is new each start. Chrome's own update of itself is the
  machine's updater's and is not touched.
- **Safe Browsing's real-time lookups are off**, and with them the keys
  they fetch; its list-based checks are whatever lists the install carries.
- **On-device AI features are off** in that Chrome: model manifests and
  models are not fetched, and AI-mode is not offered. Nothing in this
  browser uses them.
- **The switch names are Chrome's, and change.** `DnsOverHttps` stopped
  meaning anything by Chrome 155 (item 635). The test in § 2 is how a name
  that stops working is found.

## Alternatives rejected

- **A browser-level `Fetch`** (item 649's second candidate): measured to
  see none of these requests (§ 1, 2).
- **Set everything aside by name**, switching nothing off: about twenty
  requests a start, a dozen kinds, a download of executable components among
  them, each uncaused — ADR 0012 § 2's uncaused line, multiplied.
- **Preferences written into the profile**: measured to change nothing for
  the two that are left (§ 1, 3).
