# ADR 0094 — Trackers and adverts are refused by lists alo carries, at the place every request is decided

**Status:** accepted
**Date:** 2026-11-01
**Context:** queue item 736, asked for by the owner on 30 October 2026 after
weighing Brave: alo does not fork Brave, but takes Brave's ad and tracker
blocker as a rented crate behind one file. ADR 0001 (rent the physics, build
the engine); ADR 0010's line on *whose* `unsafe` it is (a rented crate's
`unsafe` is the crate's); ADR 0029 (a regular expression a page writes is
ours); ADR 0058 (Chromium is a renderer we drive); ADR 0060 (Chrome's
network carries a request and the browser process decides it before it
leaves, § 2) and its fifth amendment (a refusal is no error page); ADR 0012
(every request says what caused it, and the record); ADR 0077 (alo carries
its own Chromium); queue items 705 (what the agent may do, a site's own
rules), 721 (an updater).

## The decision in one line

Every request a page in Chromium makes — through the machine's Chrome or the
Chromium alo carries — is asked, **after the browser process's own rules let
it go and before it leaves**, of **EasyList and EasyPrivacy as published,
carried inside the application**, read by **Brave's matcher, the `adblock`
crate, rented in one file**; a request they name is refused as every refusal
there is (`Fetch.failRequest`, `BlockedByClient`) and **written in the record
with the list, line and rule that refused it**. Network rules only: nothing
of ours is put into a page to hide parts of it.

## 1. The matcher: `adblock`, rented

`adblock` 0.13.5 (`github.com/brave/adblock-rust`), **MPL-2.0** — the licence
this repository is under — is the engine of Brave's Shields: it parses
Adblock Plus's syntax, which both lists are written in, and answers a request
(its address, the page's address, its kind, its method) with whether a rule
names it, an exception excuses it, or an `important` rule overrides the
exception. Writing that matcher is not this project's value: the rules are
a community's and the matching is a decade of other people's edge cases,
which is what ADR 0001 rents.

- **Pinned exactly** (`=0.13.5`): a new matcher is a diff somebody reads.
- **Default features off.** Not `single-thread`, so one matcher is `Sync`
  and shared by every page of the process (§ 4); with
  `embedded-domain-resolver` (the public suffix list, through `addr` over the
  `psl` already in this tree — a rule's `third-party` and `domain=` are read
  by site) and `full-regex-handling` (a rule written as a regular
  expression). Without `css-validation` (cosmetic rules, § 5) and
  `resource-assembler` (the replacement scripts Brave answers some requests
  with).
- **What it brings**, ten crates new to the lockfile: `adblock` (MPL-2.0),
  `addr` (MIT/Apache-2.0), `flatbuffers` (Apache-2.0), `regex`,
  `regex-automata`, `regex-syntax`, `itertools`, `either` (each MIT OR
  Apache-2.0), `aho-corasick` (Unlicense OR MIT), `seahash` (MIT). The rest
  it needs is here already (`idna`, `psl`, `serde`, `serde_json`, `memchr`,
  `base64`, `bitflags`, `smallvec`, `rustc-hash`, `arrayvec`,
  `percent-encoding`, `thiserror`).
- **No `unsafe` of ours.** `adblock` and `flatbuffers` hold `unsafe` of
  their own (its matcher's compact form is a flatbuffer); by ADR 0010's line
  that is the crates', as `rustls`' and `winit`'s is. Nothing here calls an
  `unsafe` function: `Engine::new_with_filter_set`, `FilterSet::new`,
  `add_filter_list`, `Request::new` and `Engine::check_network_request` are
  safe, checked in its source.
- **The Rust it needs.** `adblock` declares no `rust-version`; it is edition
  2024 (1.85 or later), and Cargo resolved every new crate for the
  workspace's 1.89. Nothing of ours calls a standard API past the minimum.
- **ADR 0029 is not touched.** `regex` enters as `adblock`'s, matching only
  the rules of the lists alo carries — never a pattern a page wrote; a
  page's regular expressions are still `alo-js`'s own, counted, and the
  Rust `regex` crate is linear-time besides.
- **Behind one file**, `alo-window`'s `blocking.rs`, the only file that may
  name it (the gate's boundary list). What crosses the boundary is ours: a
  request as text (`Asked`) and what refused it (`Blocked` — the list's
  name, the line, the rule).

## 2. The lists: EasyList and EasyPrivacy, carried, never fetched

- **Which.** EasyList (adverts) and EasyPrivacy (trackers) — the two every
  blocker starts from, the two Brave's Shields and uBlock Origin turn on by
  default. Not EasyList's language supplements, not annoyance or cookie
  lists, not uBlock's own: each is a list of its own with its own breakage,
  for a later decision when a real page needs it.
- **Their licence.** Dual: the GNU GPL version 3 or later, and Creative
  Commons Attribution-ShareAlike 3.0 Unported or later
  (`easylist.to/pages/licence.html`, read 1 November 2026). alo takes them
  under **CC BY-SA 3.0**: the lists are distributed unmodified, each with its
  own header, attributed to *The EasyList authors (https://easylist.to/)*,
  with the licence's address. They are data, carried beside an MPL-2.0
  program and not relicensed by it; CC BY-SA's share-alike binds the lists,
  which are passed on as they came.
- **In the repository, pinned**: `crates/alo-window/lists/easylist.txt`
  (version 202611011105, 2,036,694 bytes, 76,978 lines) and
  `easyprivacy.txt` (version 202611011057, 1,512,288 bytes, 57,223 lines),
  taken on 1 November 2026 from `easylist.to/easylist/`, with
  `lists/LICENCE.txt` naming both versions, their SHA-256s, the source, the
  attribution and the licence. **Not fetched at build time**: a build that
  reaches the network is one this repository never makes (the CEF host's
  `CEF_PATH` rule), and a list fetched as the program builds is one nobody
  read in a diff.
- **Carried with the application**: compiled into `alo`
  (`include_str!`), so there is no file a page or another program could
  change, and nothing is fetched by a page's load or by anything else;
  `alo-bundle` puts the licence beside the application's other licences
  (`Contents/Resources/licences/EasyList-and-EasyPrivacy.txt`). **Updated
  with the application** — a new copy in a diff here until the updater
  (721) carries a new alo; EasyList says its lists expire in four days,
  which a carried copy will not meet until then, and that is the cost.

## 3. Where a request is asked: the place ADR 0060 holds and decides it

ADR 0060 § 2 holds every target of a tab until its requests are paused and
judges every paused request in the browser process (`chrome_tab.rs`'s
`answer`). The lists are asked **there, after** `chrome_gate`'s rules let a
request go — a request those rules refuse is refused by name as before — so
both Chromiums, the machine's Chrome and alo's CEF host, are held to the
same lists by the same code, with nothing of the lists inside either.

- **Refused** as every refusal there is: `Fetch.failRequest` with
  `BlockedByClient` — what Chrome's own extensions' blocking reports — never
  a fulfilment pretending to be a server.
- **Recorded** (ADR 0012): the line in the record is the request's, its
  cause assigned as for any other, `Refused` with *a tracker or an advert by
  EasyPrivacy, line 973: /fbevents.js*. The tab keeps them in order
  (`Tab::blocked`, and the address in `Tab::refused`).
- **Asked with** the request's address, **the address of the page the tab
  shows** as the side a rule's `third-party` and `domain=` are read against
  (as Brave's Shields read them against the tab's site), its kind in
  Chrome's word mapped to the lists' (`Script` → `script`, `XHR` and
  `Fetch` → `xmlhttprequest`, `Ping` → `ping`, a frame's `Document` →
  `subdocument`, …), and its method.
- **Never asked of the lists**: the tab's own document and its redirects —
  a person or an agent chose to go there, and a list refusing the page asked
  for would be the browser overruling the person; a WebSocket, which Chrome
  does not let be held (ADR 0060 § 2: told of, recorded as not decided);
  what Chrome asks for of its own accord (ADR 0060's third amendment); a
  frozen page's requests, which are all refused but the page already.
- **The engine's own path** (alo's renderer and `alo-net`) is not asked: in
  stage 1 it draws alo's own pages and frozen cases, which make no request a
  list would name. When the engine draws strangers' pages (stage 2) the same
  file is asked from `alo-net`'s decision, in the change that does it.

## 4. Read once, kept with each rule's line

The lists are read into one matcher per process, on the first request asked
of it (`Blocker::carried`), network rules only, **each rule kept with its
text and line** (`FilterSet::new(true)`) so a refusal names the rule. Measured
on this Mac (Apple silicon, 1 November 2026) with a scratch program over the
same two lists and the same features:

| | release | debug |
|---|---|---|
| read into a matcher | 30–36 ms | 177 ms |
| one request asked | 1.1–1.2 µs, plus 0.9 µs to read its address | 53 µs, plus 2.6 µs |
| the process's resident memory, from before the lists were read to after the matcher was built (the two lists' text read from files included, about 7 MB) | +57 MiB (+41 MiB without the rules' text) | +58 MiB |

and in the application, `alo` built for release: 23,474,064 bytes before
and **30,227,792** after, 6.4 MiB more — the lists' 3.5 MB of text and about
3 MB of the matcher's and `regex`'s code; the repository 3.5 MB of lists. A news page's few hundred requests cost a
millisecond of matching in all; the memory is the cost that matters, and
16 MiB of it is the rules' text kept to name them, which a refusal that
names nothing would not need — named is decided, since a refusal no one can
trace to a rule is one no one can argue with.

## 5. Not here: hiding parts of a page

Cosmetic rules — `##.ad-banner`, hiding what is left of an advert once its
request is refused — are **not read**. Carrying them out means putting
something of ours into somebody else's page in Chromium: a style sheet per
frame (`CSS.createStyleSheet` and `CSS.addRule` over the pipe), or a script
that watches the page as Brave's and uBlock's do. A script of ours in a page
is ruled out (ADR 0058: the page is Chromium's to run, ours to drive); a
style sheet per frame is possible, but EasyList holds about fifty thousand
such rules, their generic half would be a sheet of thousands of selectors in
every frame, and how that costs layout on a real page was not measured. So
a page may show the empty box its advert would have filled. Queue item 806.

## 6. A breakage undone, and what a person sees

- **A site's own switch** — the lists off for one site, as Brave's Shields
  are switched off per site — is **705's** (a site's own rules, in the
  settings screen over `alo-grants`): designed there, not built here. Its
  shape, for 705 to take: a value per site held by the browser process,
  asked in `answer` before the lists, so a site switched off is never asked
  of them; the record still says each request was let go. **Until then
  there is no switch**: a page the lists break stays broken in alo, and is
  reported as a breakage of the lists.
- **What a person sees: nothing in the window yet.** The conversation's
  window says each refused request **on its terminal** — *the card's page
  asked for …, refused as a tracker or an advert by EasyPrivacy, line 973:
  /fbevents.js (ADR 0094)* — because a refusal is never silent, and the
  record holds every one. A count on the card, its list of what was refused,
  and the switch beside it are a design for the owner (queue item 807), not
  decided by a loop.

## Alternatives rejected

- **Forking Brave** — a browser of somebody else's, C++ and Chromium's
  patches, to get a matcher; the owner weighed it and chose this.
- **Writing the matcher** — ADR 0001: nobody's engine differs by it.
- **Chromium's own blocking** (`Network.setBlockedURLs`) — patterns with no
  exceptions, no `third-party`, no `domain=`, refusals no rule names, and a
  second gate beside the `Fetch` one that already holds every request (ADR
  0060 § 2).
- **Fetching the lists at a page's load or at the program's start** — a
  page's load would depend on a third party's server, which would learn
  every start of alo; and a list that changes under a person between two
  loads breaks a page with no diff anyone read.
- **Fetching them at build time** — § 2.
- **The lists' GPL** — CC BY-SA asks only attribution and share-alike of
  the lists themselves, both met by carrying them unmodified with their
  notice.
