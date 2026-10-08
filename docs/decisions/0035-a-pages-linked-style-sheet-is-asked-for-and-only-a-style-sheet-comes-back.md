# ADR 0035 — A page's linked style sheet is asked for, and only a style sheet comes back

**Status:** accepted
**Date:** 2026-10-08
**Context:** queue item 347, *A loaded page's linked style sheets*, opened
by a page: `alo-workplace`'s
`products/sites/alo-sites/tests/golden/section_cta.html`, one of the pages
alo Sites publishes for its customers. It links its whole style sheet,
`<link rel="stylesheet" href="/assets/site.css">`, and carries an inline
script, as every page alo Sites publishes does. Also read: ADR 0005 (a
renderer has no network and no filesystem, and the browser process owns
both); ADR 0007 (cookies partitioned by top-level site); ADR 0012 § 4 (the
browser process assigns a cause from which message it was answering);
ADR 0016 § 2 (every response is a task); ADR 0017 § 6 (a changed page is
drawn again whole when its rendering is read); ADR 0020 §§ 2 and 3 (a URL a
renderer resolves is a claim parsed again, and `file:` only from a `file:`
document); ADR 0032 (a script's fetch is an ask in an answer, decided by
the browser process, and only what the page may read crosses); and ADR
0034 (a page's `style-src` is asked of its inline style by the renderer).
The code this is about:
- `alo-corpus`'s `rendering.rs`: a case whose page runs script is loaded
  by a renderer, and one that also links a sheet is **refused by name** —
  *"its page runs script, so it is loaded by a renderer, which is handed no
  linked sheet or picture"*. So the page above cannot be a case.
- `alo-renderer`'s `page.rs` (`Page::sheets`, author sheets as text) and
  `renderer.rs`, which joins them into one sheet and draws with **no linked
  sheets at all**; `pipeline.rs` (`draw`), which applies a `<link>`'s sheet
  only if a caller handed it in by its `href` as written, and otherwise says
  *no style sheet was loaded*.
- `alo-window`'s `opening.rs`, the only thing that hands a renderer a sheet:
  files named on the command line.
- `alo-dom`'s `sheets.rs` (`asked_for`, `Sheet::Linked`).
- `alo-net`'s `request.rs` (`Purpose::Style`, which exists and nothing
  makes), `csp.rs` (`style-src` for `Purpose::Style`), `mixed.rs`,
  `cors.rs`, `media_type.rs`, `schemes.rs` (`file:` typed by extension) and
  `connection.rs` (`PATIENCE`).

## The decision in one line

A `<link rel="stylesheet">` in a page a renderer holds is **asked for** by
the renderer, in the answer to the message whose work found it, exactly as
ADR 0032 asks for a script's fetch; the **browser process decides and makes
the request** as a style request, under the page's header policy, mixed
content and its own copy of the document's origin and cause; and the body
crosses **only if it is a style sheet** — a successful response whose
`Content-Type` is `text/css` — so a renderer never holds the bytes of
something else a page merely named. Until every sheet its load asked for is
answered, the browser process **does not show the page**.

## Why this is a decision rather than a chore

Nothing in a renderer can fetch, and nothing in the browser process reads a
page's markup. So today a page that links its style sheet and runs a script
— which is nearly every page on the web, and every page alo Sites publishes
— is drawn unstyled, in the window and in the corpus alike, and the corpus
refuses to commit that picture rather than lie with it. Which of the two
processes finds a sheet, what crosses, and what the page may then hold are
the questions ADR 0032 answered for a script's fetch. A style sheet differs
in the one place that matters most:

**A page reads the body of a sheet from anywhere.** A cross-origin `fetch`
is opaque and its body never crosses (ADR 0032 § 4). A cross-origin style
sheet must cross, or no page using a CDN has any style, and HTML has always
applied one without asking the other site. So the filter cannot be the
same-origin policy. Something else has to stop `<link rel=stylesheet
href="https://bank.example/statement">` putting a person's statement into a
renderer's memory, where Spectre (ADR 0005, reason 1) can read it whatever
the bindings say.

**And the page is visible before its style arrives.** A browser that shows
a page as soon as its markup is parsed shows it unstyled first. That is a
choice about what a person sees, and it belongs to the process that shows
things.

## 1. The renderer finds the sheet and asks for it

At the end of each message's work — `Load`, `Act`, a delivered fetch or a
delivered sheet — the renderer takes the document's style sheet links as
`alo_dom::sheets::asked_for` finds them, resolves each `href` against the
document's base URL as ADR 0020 § 2 resolves a link's, and **asks for each
URL it has not asked for before in this document**. One ask per URL per
document: two links to one sheet are one request, and a link a script adds
later is asked for in the answer to the task that added it.

Each ask carries a **number the renderer chose**, unique for the life of the
document, which the browser process echoes and never interprets (ADR 0032
§ 1). It also carries what only the renderer knows: the resolved URL; the
`crossorigin` attribute as `alo-net`'s mode and credentials (absent:
`no-cors` with credentials; `anonymous`: `cors`, same-origin credentials;
`use-credentials`: `cors` with credentials); the `referrerpolicy` attribute
as `alo-net`'s enum; and the element's nonce, which `style-src` reads.

It never carries an origin, a cause, a cookie or a tab. The asks are bounded
in number per answer and per document, as ADR 0032 § 3 bounds a fetch's,
with the numbers landing in the code with their reasons.

**A `<meta>` policy is the renderer's to apply before it asks**, as ADR
0032 § 3 and ADR 0034 say of every other `<meta>` policy: a link its
`<meta>` policy refuses is not asked for, and what a header policy watching
the page would object to is the browser process's to say, from its own copy.

**A link with an `integrity` attribute is not asked for, and says so**, until
Subresource Integrity is built. Applying a sheet whose author asked for it to
be checked, unchecked, would be quietly weaker than the page asked for;
refusing it is a page drawn unstyled with a reason.

## 2. The browser process decides, as it decides a fetch

In ADR 0032 § 3's order, each refusal named and recorded, with
`Purpose::Style`:

1. **The URL parses**, under ADR 0020 § 3's bound.
2. **The scheme.** `http` and `https`; and `file:` **only from a `file:`
   document**, ADR 0020 § 3's rule for navigation, because `alo` opens local
   files and a local page's sheet beside it is the ordinary case. Every other
   scheme is a failure. A `data:` sheet is the page's own bytes and is the
   renderer's to read when a page needs one; until then it is refused by
   name in the renderer and never asked for.
3. **`style-src`** of the document's header policies, with the ask's nonce
   (`csp.rs` already maps `Purpose::Style` to it).
4. **Mixed content**: a style sheet is blockable, so from a secure document
   to an insecure URL it is refused, never upgraded.
5. **CORS** only for a `cors` ask, with the document's origin as the asker,
   as for a fetch. A `no-cors` ask is a `GET` with no page-set headers, so
   nothing is preflighted.
6. **Cookies** under the document's top-level site (ADR 0007), when the
   credentials mode allows them.
7. **The referrer** from the browser process's copy of the document's URL.
8. **The cause by ADR 0012 § 4**: an ask in an `Act`'s answer is the
   agent's; in any other answer it is the document's.

## 3. Only a style sheet crosses

The browser process sends the body **only when the response is a style
sheet**: its status is in the 200 range, and its `Content-Type`'s essence is
`text/css`. HTML requires exactly this of every sheet in a document that is
not in quirks mode, and this engine has no quirks mode (law 1), so it is
required of all of them, same-origin as well. A `cors` ask must also pass
CORS's check before anything crosses.

Anything else is a **failure, and its bytes are not sent**: an error page, a
redirect to a login form, a JSON document, an HTML page named by a hostile
`href`. That is what keeps `<link rel=stylesheet>` from being a way to read
another site: the only cross-origin bytes a renderer ever holds are bytes
their server said were CSS, which are bytes that site published to be
applied by anybody — the same thing every engine's opaque-response blocking
lets through for a style request, and nothing more.

**A failure carries no reason to the renderer**, as a fetch's does not
(ADR 0032 § 4): the sheet is not applied, and the page's issues say *the
style sheet at this URL did not arrive*, the same words whatever happened.
The reason is written where the person can see it and in the session's
record.

**The body crosses whole**, in one message, under `alo-net`'s
`LARGEST_BODY` and the wire's `LARGEST_MESSAGE`; a larger one is a failure
said to the person. **It crosses as bytes and the renderer decodes it**,
because the browser process does not read a page's bytes (ADR 0005): as
UTF-8, a byte-order mark removed, an invalid sequence replaced. A `charset`
or `@charset` naming another encoding is said among the issues and decoded
as UTF-8 regardless; legacy encodings are stage 3's (queue item 138).

## 4. A sheet's answer is a task, and the page is drawn again

The browser process answers each decided ask with a **new `ToRenderer`
message** carrying the ask's number and the bytes or the failure. It is a
task (ADR 0016 § 2). The renderer keeps what arrived, by URL, for the life
of the document, and the next draw applies it — `draw` already applies a
`<link>`'s sheet in document order among the `<style>`s when it has one.
Nothing is drawn eagerly: the page is drawn again when its rendering is next
read (ADR 0017 § 6). The answer to that message is the same as a delivered
fetch's, and may carry new asks.

A sheet whose `<link>` has since been removed, or whose `rel` no longer says
`stylesheet`, is kept and not applied: `asked_for` reads the document at
every draw. **An ask whose document has gone is answered by nobody**, as for
a fetch.

## 5. A page is not shown until its sheets are answered

The browser process **does not present a document's first frame until
every sheet asked for in its load's answer is answered**, delivered or
failed. This is HTML's render-blocking style sheet, decided in the process
that presents rather than by a renderer waiting: the renderer never waits on
anything (ADR 0005), and the browser process simply does not ask it to paint
yet.

It is bounded. A server that sends a byte every twenty-nine seconds stays
inside `connection.rs`'s `PATIENCE` for ever, so the wait has a bound of its
own, after which the page is shown with what has arrived and the late
sheet is applied when it comes. The number lands in the code with its
reason, ADR 0014 § 9's rule, and a page shown before its style is said to
the person as such.

A sheet asked for after the load — one a script added — blocks nothing.
That is also HTML's rule.

## 6. A corpus case's sheets are answered from the files beside it

A loaded case answers its renderer's sheet asks from its `linked.txt`, each
name resolved against its `address.txt` — so `/assets/site.css` means
`https://nordwind.alosites.com/assets/site.css` — through **the same check
as § 3**, with the type the frozen file's extension stands for, as
`Resource::from_file` already decides it. A sheet the case froze nothing for
is a failure, and the case says so, as ADR 0032 § 7 says of a fetch. The
reference is drawn after every answer, as the window would show it once
§ 5 lets it. Pictures in a loaded case stay refused by name, as below.

## What this costs

**A page that links its style is two round trips before it is shown**,
where an engine that fetched in the browser process from a preload scanner
would be one. A preload scanner is the browser process reading a stranger's
markup, which ADR 0005 does not allow, and the cost is a measurement away
from mattering: no claim about speed is made here.

**A no-cors sheet's body is in the renderer**, cross-origin or not. That is
the web's rule and every engine's; § 3 is what keeps it to sheets.

**A sheet served as `text/plain` is not applied**, which a quirks-mode page
in another engine would apply. That page is a stage 3 page.

## Alternatives rejected

- **The browser process scans the markup for `<link>`s.** It would be
  parsing a stranger's bytes in the process that may not crash, and it would
  disagree with the renderer about what the document links the first time a
  script added one.
- **Hand the renderer every response, and filter in the renderer.** Then
  `href="https://bank.example/statement"` is a statement in the page's
  memory. Spectre, again.
- **The same filter as a fetch: opaque bodies never cross.** No page using a
  style sheet from another host would have style.
- **Show the page at once and restyle it.** Every page would flash unstyled,
  and an agent reading the tree at the first frame would read a page that
  never looked like that to anybody.
- **Let the renderer wait for its sheets before drawing.** A renderer that
  waits is the thing ADR 0005 forbids, and the browser process already
  decides when to ask for a frame.

## What this does not decide

- **Pictures in a loaded page** (`<img>`, `background-image`). They will be
  asked for the same way, but what may cross for a no-cors picture — which
  bytes are plausibly an image — is its own rule, and its own item.
- **`@import`** inside a sheet, which is a request a *sheet* makes. It stays
  unapplied until a page needs it.
- **`<script src>`**, which item 238 already describes in this shape.
- **A `<link>`'s `load` and `error` events**, and **`media`** on a `<link>`:
  each its own item when a page needs it. `media` is not honoured on the
  markup path either, and this changes nothing about that.
- **Whether a script waits for a sheet before it runs.** HTML makes a script
  after a pending sheet wait, so that what it reads of layout is styled.
  Nothing a script can call reads style or layout today; the item that
  builds the first such reader decides it.
- **Subresource Integrity**, beyond refusing by name a link that asks for
  it.
- **The bounds' values** — §§ 1 and 5; they land in the code with their
  reasons.

## How we will know if this was wrong

**If a frozen page's linked sheet applies in another engine and not here**,
and the reason is not § 3's rule on purpose, the order or the check is
wrong, and the fix names the step.

**If a renderer is ever found holding bytes from a style request whose
response was not `text/css`**, § 3 was not followed, and that is a security
bug.

**If a person sees a page unstyled for longer than its sheets took to
arrive**, § 5's bound is wrong, and it is measured rather than guessed again.
