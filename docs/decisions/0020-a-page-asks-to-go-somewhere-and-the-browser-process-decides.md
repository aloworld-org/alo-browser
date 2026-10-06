# ADR 0020 — A page asks to go somewhere, and the browser process decides

**Status:** accepted
**Date:** 2026-10-06
**Context:** queue item 263, *a script's `click()` follows a link*, which
depends on *a decision about what a renderer may ask the browser process to
navigate to, which **needs ADR** if ADR 0005 and 0012 do not already decide
it*; ADR 0005 (the browser process sends work and a renderer returns results,
never calling back and never waiting); ADR 0012 (every request carries a cause
the browser process assigns, § 4: *while a verb is being applied, that tab's
requests are the agent's*, the boundary being the task); ADR 0016 § 6 (an
`Act` is one task); ADR 0018 §§ 5–6 (a link is followed only if nobody
cancelled its click, and activation behaviour lives once in `alo-dom`); ADR
0019 (a native reaches its realm's document cell through `[[HostDefined]]`);
ADR 0013 § 3 (*absent beats approximate*); ADR 0014 § 9 (every number has a
reason); `alo-workplace`'s `FilesView.tsx` and `TaskDetail.tsx`, which make an
`<a download>` for a `blob:` URL, call `a.click()` and revoke the URL on the
next line; and the code this is about — `alo-renderer`'s `message.rs`
(`FromRenderer::Loaded` already carries two *claims* the browser process judges
from its own copy of the facts: `wanted` fonts and `objections`), `page.rs`
(`Page` carries no URL), `tab.rs` (`Tabs::load` takes the cause the load's
request carried; `Tabs::act` mints the action), `press.rs` and `renderer.rs`
(an agent's link answers `Outcome::Followed` with its `href` as written, and
whoever called `Tabs::act` goes there), `alo-dom`'s `activation.rs`
(`Follows::Link`) and `alo-bindings`' `interface/html_element.rs` (a script's
click on a link refused by name, after its listeners).

## The decision in one line

A renderer **asks** to navigate, as a **claim carried in the answer to the
message whose work made it** — never a call, never awaited — with the URL
already resolved against its document's base and nothing else it could not
know better than the browser process; the **browser process decides**: it
parses the URL as untrusted, refuses by name every scheme a page may not send a
tab to, assigns the cause by ADR 0012 § 4 from *which message it was answering*
rather than from anything the renderer said, and keeps one navigation per
answer, the last; and a **download is a different ask** with its own rules, cut
to its own item.

## Why this is a decision rather than a chore

ADR 0005 decides the direction: there is no call back, so a navigation cannot
be a request a renderer makes and waits on. ADR 0012 decides who names the
cause: the browser process, never the renderer. Neither decides the five
questions a script's `a.click()` puts, and each one is visible to a page or to
the person:

**What a renderer may ask for.** A renderer is the process that parsed a
stranger's page (ADR 0005). *Navigate this tab to `file:///Users/…`*, *to
`data:text/html,<form action=…>`*, *to a scheme only the browser's own screens
use* — each is a sentence a hostile page would write, and a browser process
that obeyed whatever the page's process asked would have moved the page's
privileges into the one process that has a filesystem.

**Where the URL is resolved.** An `href` is relative, to the document's *base
URL* — its own address, or a `<base href>` in its markup. The renderer has the
markup and not the address (`Page` carries none); the browser process has the
address and must never read the markup.

**What cause a page's navigation carries.** A listener in an agent's
`Activate` that calls `a.click()` is the page's script, inside the agent's
task. ADR 0012 § 4 answers this, and the answer has to be applied by somebody
who cannot be lied to.

**Several in one task.** A page can click three links before its script
returns. HTML's *navigate* lets a later navigation of the same navigable abort
an earlier one still in progress, and a renderer that asked for three would be
asking the browser to choose.

**A download.** `<a download>` is a link whose activation writes a file rather
than replacing the page, and alo's own `FilesView.tsx` does exactly that with a
`blob:` URL whose bytes exist only in the renderer and whose URL is revoked on
the next line.

## 1. An ask is a claim in an answer, and the renderer never learns what became of it

A page's navigation crosses the boundary the way `wanted` and `objections`
already do: **a field of the answer to the message whose work made it** —
`Loaded` for a script that ran at load, `Acted` for one that ran in a verb's
task, and the answer to whatever message lets a renderer run its own due tasks
when item 233 builds one. Nothing new flows from renderer to browser process
outside an answer, and nothing flows back: the renderer is not told whether the
browser went, and the page goes on running until the browser process loads
something else into its tab. That is HTML's own shape — navigation is
asynchronous and the old document lives until the new one replaces it — and it
is ADR 0005's: a renderer that waited for an answer would be a renderer making
a call.

**The ask is held in the document cell while script runs.** `click()` is a
native in the middle of a page's script (ADR 0018 § 6); it reaches its realm's
document cell through `[[HostDefined]]` (ADR 0019) and **records the ask
there** — HTML's *ongoing navigation* of the page's navigable, which is exactly
a per-document fact. The renderer takes it from the cell when the message's
work is done and puts it in the answer. `alo-dom`'s `activation.rs` keeps
saying what follows a click (`Follows::Link`) for both callers, as ADR 0018 § 6
put it there; who records the ask is the caller.

**An agent's own link is the same ask.** `Activate` on a link answers
`Outcome::Followed`, which stays what the agent is told about *its* verb; the
navigation itself is the ask in the same answer, whether the page runs script
or not. One path into the browser process for *this tab is to go somewhere*,
rather than one for agents read from an `Outcome` and another for pages.

## 2. What the ask says, and what it may not

The ask carries:

- **The URL, resolved and serialised by the renderer**, against the document's
  base URL — which means `Page` carries the document's URL, stated by the
  browser process from the response it fetched, as the address a page is
  entitled to know about itself (`document.URL` and `a.href` need it anyway).
  Resolving there is not a trust decision: a page chooses where its links go,
  `<base>` included, so a renderer that resolved *differently* could only have
  asked for a URL the page could have written outright. Whatever it sends, the
  browser process parses it again.
- **How it arose**, as a claim like `Purpose`: a link activated by the
  browser's own click or by a script's, and the link's `referrerpolicy` and
  `rel="noreferrer"`. These say *what*, which only the renderer knows; they
  never say *who*, which ADR 0012 § 4 keeps from it. A referrer policy is the
  page's to choose for its own address, so the browser process honours it when
  it builds the request's `Referer` (`alo-net`'s `referrer.rs`) from **its own
  copy** of the document's URL.

It does not carry a cause, a tab, a document id, or a target other than its
own tab. A link whose `target` names another browsing context — `_blank`, or a
name — opens a window, which is a popup policy and a tab strip (item 118), and
is **refused by name** until there is one; with no frames yet (item 86),
`_self`, `_parent` and `_top` are all the page's own tab.

## 3. The browser process decides where a page may send its tab

The browser process parses the ask's URL with `alo-url`, as hostile bytes:
**a URL that does not parse, or is longer than 2 MiB, is refused** — 2 MiB is
Chromium's limit for the same check, a URL a real page has never needed to
exceed, and short enough that a page cannot decide how much the browser process
holds. Then by scheme:

| Scheme | From a page | Why |
|---|---|---|
| `http`, `https` | **navigated** | the web |
| `about:blank` | **navigated** | an empty document, nothing fetched |
| `data:` | **refused** | a page-made document at an address that names no origin; Chromium and Gecko both refuse a page navigating a *top-level* tab to one, for phishing. A person typing one is not a page asking |
| `file:` | **only from a `file:` document** | ADR 0005 keeps the filesystem from every renderer; a page from the web asking to open a local file is the escape that rule exists to stop |
| `javascript:` | **refused** | running script by URL is not a navigation; it is the renderer's, under the page's policy, and its own item when a page needs it |
| `blob:` | **refused** | the bytes are the renderer's and `Blob` is not built; § 5 |
| anything else | **refused** | `mailto:` and the rest hand the person to another program, which is a permission (item 93), not a link |
| the browser's own schemes | **never** | a page may not open the screens that decide what pages may do |

A refusal is **said where the person can see it and recorded under ADR 0012**
— a request a rule of ours refused is a line naming the rule, with the cause
§ 4 assigns — and the page is told nothing, as in every other browser: a
navigation that did not happen is not an exception in somebody's script.

## 4. The cause is the browser process's, from which message was answered

ADR 0012 § 4 already says it, and this applies it rather than amending it:

- **An ask in the answer to an `Act`** is `Cause::Agent` — the action
  `Tabs::act` minted and the tab's document — whether the agent's own link or a
  listener's `a.click()` made it. The boundary is the task (ADR 0016 § 6), and
  the page's script ran inside it. *That the page's script did it* is the ask's
  claim (§ 2), kept beside the cause in the record, so *both answers, in
  order* (ADR 0012 § 3) survive: the agent acted, and the page clicked.
- **An ask in the answer to a `Load`**, or to anything else that is not an
  `Act`, is `Cause::Document`, naming the document the tab holds.

The renderer's claim never moves the cause. A renderer that said *a script did
this* in an `Act`'s answer cannot make it the document's, and one that said
*the browser's click* in a `Load`'s cannot make it the agent's or the
person's. The new document the navigation loads records that cause
(`Tabs::load`), and the chain walks on from there.

## 5. One per answer, the last

HTML's *navigate* aborts a navigable's ongoing navigation when another starts,
so a task that clicks three links goes to the third. The document cell keeps
**one ask, the last**, and counts the ones it replaced; the answer carries the
ask and the count, and the count is said in its issues. Nothing about a
navigation a page replaced reaches the browser process as a URL — it never
became a request.

A page that navigates on every load, for ever, is a navigation loop and costs
one tab a load at a time; a bound on how fast a tab may be navigated is item
85's to decide with session history, measured against a real page rather than
chosen here.

## 6. A download is a different ask

A click on an `<a download>` asks for a **file**, not a page, and is decided
differently:

- `download` is honoured only when the URL is the document's own origin, a
  `data:` URL or a `blob:` URL; for any other origin the attribute is ignored
  and the link is a navigation — HTML allows it and Chromium and Gecko do it,
  because a filename suggested by one origin for another origin's bytes is a
  way to dress one thing as another.
- The suggested name is a claim, cleaned by the browser process — no path
  separators, no leading dot, no control characters, bounded — and **nothing
  is written anywhere the person did not choose**, which is item 120's
  interface.
- A `blob:` URL's bytes live in the renderer, so they **cross with the ask**,
  taken when the link is activated — HTML resolves a `blob:` URL's entry when
  the URL is parsed, which is why alo's own code may revoke it on the next line
  — bounded, and never parsed by the browser process, which only writes them.

It is **cut to its own item (264)**, which waits on `Blob` for its `blob:`
half and on item 120 for where a file goes. Until then a script's click on an
`<a download>` stays refused by name, as every link's is today; item 263 builds
the navigation.

## What this costs

**`Page` gains a URL.** The browser process states one more fact about a page
to the process that renders it — a fact the page is entitled to, and one every
later item (`document.URL`, `location`, `a.href`, relative `fetch`) needs.

**Every answer that can run script gains a field**, and the wire format a tag.
Small, and the price of no new message kind and no call back.

**A navigation a page asked for and the browser refused is invisible to the
page.** That is every browser's behaviour, and a script that depended on
knowing would be a script probing the browser's policy.

**Downloads wait.** alo's own `FilesView.tsx` and `TaskDetail.tsx` stay
refused by name until 264, because a file written without a place the person
chose is worse than a file not written.

## Alternatives rejected

- **The renderer navigates by sending a message of its own.** There is no
  message from a renderer that is not an answer (ADR 0005), and inventing one
  for navigation is the first call back.
- **The browser process resolves the `href`**, given the base. It would need
  the `<base>` element's value, which is the page's markup in another shape,
  and it would gain nothing: the page chooses its links' destinations either
  way (§ 2).
- **Trust a renderer's scheme check.** The renderer is the process this policy
  is about.
- **Every ask in a task, in order, for the browser to choose.** HTML has
  already chosen (§ 5), and a list is a page deciding how much the browser
  process holds.
- **A page's navigation in an agent's task attributed to the page.** ADR 0012
  § 4 decided otherwise, for the reason it gives: the task is the boundary,
  and the claim keeps what the page did beside it.
- **`data:` navigations allowed because nothing is fetched.** The danger was
  never the fetch; it is a document no origin answers for, shown in a tab a
  person believes.
- **Follow `target="_blank"` in the same tab.** Approximate (ADR 0013 § 3): the
  page asked for a window, and putting its destination over the page the
  person was on is a different thing happening.

## What this does not decide

- **Navigation itself** — loading the new page, session history, back and
  forward, a bound on how fast a tab navigates, what survives: item 85. Until
  it is built, the browser process hands the decided navigation, with its
  cause, to whoever drove `Tabs`, as `Outcome::Followed` is handed today.
- **What an agent is told when the page it acted on goes somewhere** — item
  134.
- **Windows, popups and `target`** — item 118.
- **Frames**, and which navigable a link in one names — item 86.
- **Forms** — a submission is a navigation with a body, and item 82's; it will
  take this ask's shape, not a second one.
- **`location`, `history.pushState`** and every other way a script navigates —
  item 85, through this same ask.
- **`ping`.** Nothing sends it. A request to a third party on every click is a
  tracking feature, and adding it is a decision, not a chore.
- **`javascript:` URLs** — their own item, when a page needs one.

## How we will know if this was wrong

**If a real page's link goes somewhere in another engine and nowhere here, and
the URL is not one § 3 refuses on purpose**, the resolution or the table is
wrong, and the fix names the scheme or the base.

**If the record says an agent caused a navigation a person would say the page
made on its own** — a page that navigates on a timer minutes after an agent's
click — ADR 0012 § 4's edge is in the wrong place, and it is fixed there, where
both ADRs read it.
