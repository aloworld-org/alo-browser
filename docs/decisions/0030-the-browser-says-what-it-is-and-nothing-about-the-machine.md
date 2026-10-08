# ADR 0030 — The browser says what it is, and nothing about the machine

**Status:** accepted
**Date:** 2026-10-08
**Context:** queue item 325, *`navigator`: the `Navigator` interface, with
`platform` and `userAgent`*, marked **needs ADR** because *a platform and a
user agent string are a fingerprinting surface, so the values need a decision
about what this browser says about itself, not a copy of another browser's*;
`alo-downloads`, alo's public download page, whose script now compiles and
stops at its second line, `var p = navigator.platform || "";` (iteration 200);
queue item 318 (`pdfViewerEnabled`, `plugins` and `mimeTypes`, which hang off
the same object) and ADR 0028 § 4 that decided their values; ADR 0005 (the
browser process decides, the renderer is told); ADR 0018 § 4 (*an agent's
verb is the browser's input, and the page cannot single it out*); ADR 0013
§ 3 (*absent beats approximate*); `alo-net`'s `http.rs` (`write_request`,
which writes `Host`, `Accept-Encoding` and `Content-Length` and no
`User-Agent`) and `csp_report.rs`, whose module note says *this engine sends
no `User-Agent` header anywhere, and a report is not the place to invent the
first fingerprint it ever emits*; `alo-renderer`'s `page.rs`, whose `Page`
carries `scheme` because *the browser process knows, and a page does not*;
HTML's *System state and capabilities* section (the `Navigator` interface,
`NavigatorID`, the *navigator compatibility mode*, and its privacy note); and
Fetch's *default `User-Agent` value* and the step of *HTTP-network-or-cache
fetch* that appends it

## The decision in one line

This browser names **itself** — `alo/` and its version — in one string that
is the same on every machine running the same release on the same kind of
operating system: `Mozilla/5.0 (<the system's frozen token>) alo/<major>.<minor>`.
It claims no other engine's tokens, measures nothing about the machine at run
time, is composed in **one file in `alo-net`**, is sent as the `User-Agent`
header of every request, and is the **same** string `navigator.userAgent`
answers. The renderer is **told** it by the browser process with the page. The
navigator compatibility mode is **Gecko**, the one that names no company, and
`navigator.platform` is the one frozen value for that kind of system.

## Why this is a decision rather than a chore

`Navigator` looks like a table of constants, and most of it is. Three things
in it are not, and the first line of code would decide each of them by
accident.

**What the user agent string says.** Fetch defines the *default `User-Agent`
value* as *an implementation-defined header value*, and HTML's `userAgent`
returns that value. Nobody else can write it for us. The web's strings are a
history of browsers claiming to be other browsers. Copying one would be a
decision to lie to every server. Inventing a fresh one without thought would
be a decision about how much it tells a stranger about the person.

**Whether it is sent at all.** This engine sends no `User-Agent` today, on
purpose (`csp_report.rs`). Fetch says a user agent *should* append one when
a request carries none. `navigator.userAgent` must equal the header. So
building `navigator.userAgent` is also deciding the header, and the two can
never be built apart.

**The navigator compatibility mode.** HTML gives every user agent one of
three, *Chrome*, *Gecko* or *WebKit*. It decides `vendor` (`"Google Inc."`,
the empty string, or `"Apple Computer, Inc."`), `productSub`, `appVersion`'s
shape, and whether `taintEnabled()` and `oscpu` exist. Building the interface
chooses one.

HTML's own note says what weighs on all three: *any information in this API
that varies from user to user can be used to profile or identify the user*,
and *user agent implementers are strongly urged to include as little
information in this API as possible*.

## 1. The string names this browser, and claims no other

The default `User-Agent` value is:

```
Mozilla/5.0 (<system token>) alo/<major>.<minor>
```

- **`Mozilla/5.0 (`** is kept. HTML itself fixes `appCodeName` as
  `"Mozilla"`, `appName` as `"Netscape"` and `product` as `"Gecko"`, and its
  `appVersion` steps answer the empty string for a user agent that does not
  start `Mozilla/5.0 (`. The prefix is no longer a claim to be any browser.
  It is the form the platform's own specification reads the string in. A
  string without it would make `appVersion` empty and break the one parse
  every server shares.
- **`alo/<major>.<minor>`** is the product token. It carries the workspace
  version's first two components, never the patch. A patch release would
  split the people running a release into smaller groups for the days an
  update takes to reach them, and no server needs to tell a patch apart. With
  the workspace at `0.0.0` today, the token is `alo/0.0`.
- **No other engine's tokens.** No `AppleWebKit/…`, `KHTML, like Gecko`,
  `Chrome/…`, `Safari/…`, `Gecko/…` or `Firefox/…`. Each says this is an
  engine it is not. A server that believes one sends code written for that
  engine's bugs, and a bug report a site's owner files against "Chrome" is a
  bug in us they will never find.

**Why naming ourselves costs no privacy that hiding would buy.** A string
copied from Chrome would put alo's users in Chrome's crowd only until the
first script asked anything. An engine is recognised by what it does: which
features exist, how text measures, what an error message says. Any site that
wants to know which engine it is talking to can find out in a few lines,
whatever the header claims. Lying in the header would buy the appearance of a
larger crowd and the certainty of being served another engine's workarounds.

## 2. The system token is frozen per kind of system, and nothing is measured

The `<system token>` is decided by the **operating system the binary was
built for**, at compile time. It is never read from the machine at run time:
no system version, no processor, no locale, no screen. There is exactly one
token per kind of system, and it is the frozen one the major browsers already
send, so that it adds nothing to what a server already sees:

| Built for | `<system token>` | `navigator.platform` | `appVersion` |
|---|---|---|---|
| macOS | `Macintosh; Intel Mac OS X 10_15_7` | `MacIntel` | `5.0 (Macintosh)` |
| Windows | `Windows NT 10.0; Win64; x64` | `Win32` | `5.0 (Windows)` |
| Linux | `X11; Linux x86_64` | `Linux x86_64` | `5.0 (X11)` |

**These are tokens for a kind of system, not descriptions of a machine.** A
Mac with an Apple processor says `Intel`, a 64-bit Windows says `Win32` and an
ARM Linux machine says `x86_64`. Every major browser freezes them the same
way. HTML allows it: `platform` may answer *a string that is commonly
returned on another platform* for privacy and compatibility. A truer token
would split each crowd by processor and version, which is exactly the
information HTML asks implementers to withhold.

A build for a system not in this table **does not compile** the file that
holds the table. A port adds its row by amending this ADR. A missing row is
found when the port is built, never by a server receiving a guess.

**Why the platform is answered at all**, rather than with the empty string
HTML also allows. Telling a Mac from Windows is the kind of system the
person is on, which every major browser's header already says. `alo-downloads` uses it
for the purpose it exists for: to show the visitor the installer for their
own computer. The kind of system is already in every crowd these strings
form, so answering it adds nothing a server did not have.

## 3. The header is sent, and the page's string is the header

**Every request `alo-net` writes carries `User-Agent` with this string**,
over HTTP/1.1 and HTTP/2 alike, unless the request already has a
`User-Agent` of its own. That is what Fetch's *HTTP-network-or-cache fetch* asks, and the same rule
`write_request` already follows for `Accept-Encoding`: a caller that set the
header keeps it. `User-Agent` is not a forbidden request header, so a page's
`fetch()` may set one when item 83 builds it, as Fetch says.

**`navigator.userAgent` answers the same string**, and so do `appVersion`
and every other member derived from it. HTML's `userAgent` is *the
environment default `User-Agent` value*. A page that reads one value while
its server receives another is a page whose two halves disagree about which
browser they are on. So **neither is ever built without the other**: the
header comes first (§ 7).

When the header is sent, `csp_report.rs`'s reason for leaving the Reporting
API envelope's `user_agent` out is gone. The envelope then carries the same
string the request carried, which adds nothing a report collector did not
already receive.

## 4. The browser process composes it, and the renderer is told

The string, and the platform value beside it, are composed **in one file in
`alo-net`**. The network is the browser process's, and the header is the
first place the string is used. Nothing else builds the string or a piece
of it.

The renderer never composes it. It is **told**, as it is told the page's
`scheme` (`Page::scheme`: *light or dark, which the browser process knows and
a page does not*): `Page` gains the user agent string and the platform, sent
with the page in `ToRenderer::Load`, and `alo-bindings` is handed them when
it installs the page's globals. ADR 0005's direction holds. A renderer can
tell a page only what the browser process sent it, and a later per-site
answer (see *How we will know*) would be the browser process's choice in one
place, not a renderer's.

## 5. The compatibility mode is Gecko

HTML's three modes differ in what `vendor` names. *Chrome* answers
`"Google Inc."` and *WebKit* `"Apple Computer, Inc."`, each a company that
did not make this browser. **Gecko** answers the empty string, which is the
only honest one of the three. So:

| Member | Answer |
|---|---|
| `appCodeName` | `"Mozilla"` (HTML's constant) |
| `appName` | `"Netscape"` (HTML's constant) |
| `appVersion` | HTML's Gecko steps over the string: § 2's table |
| `platform` | § 2's table |
| `product` | `"Gecko"` (HTML's constant) |
| `productSub` | `"20100101"` (Gecko) |
| `userAgent` | the string, § 1 |
| `vendor` | `""` (Gecko) |
| `vendorSub` | `""` (HTML's constant) |
| `taintEnabled()` | `false` (Gecko-only, HTML's constant) |
| `oscpu` | `""` (Gecko-only; HTML allows the empty string) |

`oscpu` is answered empty because HTML allows it and nothing has asked for
more. A page that needs the system already has `platform`.

`product` saying `"Gecko"` is not a claim to be Gecko. HTML fixes it for
every browser in every mode. The same is true of `appCodeName` and
`appName`.

## 6. The same answer whoever drives

ADR 0018 § 4: *an agent's verb is the browser's input, and the page cannot
single it out*. The string, the platform and every member here are the same
whether a person or alo's agent is acting. Whether an agent's session
announces itself to the sites it visits is a question about what an agent
may do on somebody else's pages. It is item 133's (*under grants, and
recorded*), and it is not answered by changing what the browser says it
is.

## 7. The cut

- **326. The `User-Agent` header.** *Depends on nothing.* The one file in
  `alo-net` that composes § 1's string and § 2's platform from the target
  system at compile time, refusing to compile for a system with no row;
  `write_request` and HTTP/2's header block sending it unless the request
  has one; and the Reporting API envelope carrying it. *Closes when:* a
  request written for HTTP/1.1 and one for HTTP/2 each carry exactly one
  `User-Agent` equal to the composed string; a request that set its own
  keeps it; a test asserts the string has no other engine's token and no
  patch version; and the CSP report envelope's `user_agent` is the same
  string.
- **325. `navigator`.** *Depends on 326 and on the window's globals
  (`alo-bindings`).* `Page` carries the string and the platform;
  `alo-bindings` installs a `Navigator` with `NavigatorID` and the
  Gecko-only members in § 5's table. Until the global object is a `Window`
  (item 251), `navigator` is a data property, as `document` is today. Its
  closing condition is unchanged: `alo-downloads`' script runs past its
  fifth line in the renderer, and the card it marks matches the platform the
  browser says it is, in the box tree and the reference render.

## What this costs

- **Sites that sniff for Chrome or Safari will treat this browser as
  unknown**, and some will serve a degraded page or a "browser not
  supported" notice. That is the price of not lying. It is paid in exactly
  the place law 1 already pays: pages written against one engine's quirks
  are not this repository's compatibility burden until a named page makes
  them so.
- **A small browser's name is itself a signal.** Until alo has many users,
  `alo/0.0` marks a small crowd. § 1 says why hiding it would not make that
  crowd larger in any way that survives a script.
- **The header goes out on every request, where none went before.** It
  carries one bit of entropy beyond what the request already revealed: that
  the client is alo of a given release on a given kind of system.

## Alternatives rejected

**A string copied from Chrome, or the "everybody's tokens" string Chrome
and Safari send.** Rejected in § 1. It is a lie told to every server. It
makes the server send another engine's workarounds, and a site owner who
sees a Chrome bug from us will look for it in Chrome.

**No `User-Agent` at all**, as today. Fetch says a browser *should* send
one, some servers refuse a request without one, and `navigator.userAgent`
would have to be the empty string. That makes `appVersion` empty as well,
and breaks every page's `ua.indexOf(…)` in a way that reads as a bug in the
page. Absence beats approximation in the engine (ADR 0013 § 3). Here
absence *is* an answer, and it is a worse one than the truth.

**The real system version and processor.** `Mac OS X 14_6`, `arm64` and
similar would make the string describe the machine. Rejected in § 2 because
it is the information HTML's privacy note asks implementers to keep out.

**Detecting the system at run time.** It would let a build for Linux running
under a compatibility layer, or a future port, say something nobody chose.
The table is decided at compile time and in this ADR.

**The *Chrome* or *WebKit* compatibility mode.** It is the more common
choice, and the one some pages test for. Rejected in § 5 because `vendor`
would then name Google or Apple.

**Client hints and `navigator.userAgentData`.** They are not in HTML and
were designed by one engine to replace the string with values a server asks
for one at a time. Not built, and reopened only by a frozen page that fails
without them.

**The empty string for `platform`.** HTML allows it. Rejected in § 2 because
the kind of system is already in the string, and the page that opened this
item uses it for the person's benefit.

## What this does not decide

- **`language` and `languages`, and `Accept-Language`.** They are the
  person's preference, not the browser's identity, and they belong with the
  settings that hold it (item 128). Until then nothing is answered.
- **`hardwareConcurrency`, `deviceMemory`, `onLine`, `cookieEnabled` and
  every other member.** Each is opened by a page. Each must then answer
  under this ADR's rule: a value the same for everybody on the same release
  and kind of system, or the least informative answer the specification
  allows.
- **`pdfViewerEnabled`, `plugins` and `mimeTypes`.** ADR 0028 § 4 already
  decided them (`false`, and empty). Item 318 builds them on this object.
- **`navigator.webdriver`.** Not built. Whether an agent's session announces
  itself is item 133's (§ 6).
- **A per-site answer**, as other browsers keep for sites that refuse them.
  Not built. See below.

## How we will know if this was wrong

**If the person's week in stage 2's exit gate names sites that refuse this
browser by its string**, the remedy is a per-site answer chosen by the
browser process for the named sites, written into an amendment with the
sites listed. It is never a global change to what this browser claims to
be.

**If a frozen page's platform test fails** because a frozen token says
something the page reads differently from other browsers, § 2's table is
wrong. The row changes in an amendment, with the page named.
