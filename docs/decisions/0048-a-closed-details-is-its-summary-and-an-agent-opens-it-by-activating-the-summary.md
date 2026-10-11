# ADR 0048 — A closed `<details>` is its summary, and an agent opens it by activating the summary

**Status:** accepted
**Date:** 2026-10-11
**Context:** queue item 429, *A closed `<details>` shows only its summary*,
opened by `alo-sites-full-page` (iteration 274), its FAQ, and marked
**needs ADR**. alo Sites makes every FAQ block a native `<details>`
accordion:

```html
<details>
<summary>How fresh is the coffee?</summary>
<p>It ships the day it is roasted.</p>
</details>
```

There is no `open` attribute. Every browser a person compares us with shows
the question alone, in a box 53.19 tall. This engine lays out, draws and
reads the answer too, in a box 92.4 tall. Its agent outline says
`group [expanded=false]` and then reads the answer's text an agent's person
cannot see. And activating the summary, which an agent may already do
(`Summary` is an operable role in `alo-agent`'s `verb.rs`), changes nothing:
`alo-dom`'s `activation.rs` gives a summary no behaviour.

Also read: ADR 0002 (one tree, two readers; the same tree is the
accessibility tree; *"A hidden element is in it"* is why the DOM was
rejected as the agent's tree); ADR 0013 § 3 (*absent beats approximate*);
ADR 0018 §§ 5–7 (`Activate` is a keyboard's click; activation behaviour
lives once in `alo-dom`; ARIA state on a scriptless page); ADR 0043 §§ 1–2
and ADR 0044 §§ 1, 4–5 (pseudo-elements, markers, `inside` cut to item
404, and what an agent is told beside a name); `alo-box`'s `role.rs` (every
`summary` is `KnownRole::Summary`, every `details` `KnownRole::Details`) and
`state.rs` (`open_flag` puts `expanded` on the `details`);
`alo-style`'s `user_agent.rs` (`details` and `summary` are `display:
block`); `alo-agent`'s `verb.rs` and `apply.rs`; `alo-dom`'s
`activation.rs`; the case's `page.html`, `site.css` (the FAQ's rules at
lines 958–977), `layout.txt` and `agent.txt`.

Fetched 2026-10-11 for this decision: **HTML-AAM** § 3.5.31 `details`
(the `group` role; no expanded state listed) and § 3.5.124 `summary`: a
`summary` that is the first of its type within a parent `details` is
`html-summary`, exposed as a push button (MSAA), a button with the
*ExpandCollapse* pattern (UIA), a toggle button (ATK) and a *disclosure
triangle* (AX), **with the expanded and collapsed states on the summary**;
any other `summary` is `generic`. The HTML standard's `details` and
`summary` sections and its rendering section were not re-fetched; what they
say is cited from the queue entry iteration 274 wrote (`::details-content`
with `content-visibility: hidden`, the summary a `list-item` with a
disclosure marker) and confirmed below by measurement.

**Evidence, measured on this machine** in Google Chrome 154.0.8037.98,
headless, from throwaway pages in the scratchpad reading
`getBoundingClientRect()`, `getComputedStyle()` and `checkVisibility()`,
and one screenshot. The fonts were DejaVu Sans and DejaVu Sans Bold by
`@font-face`, so the numbers are like for like with this engine's.
Chromium's source was not read; Firefox is not installed and is not cited.

| What | Chrome 154 |
|---|---|
| The frozen FAQ's rules, closed | `details` 760 × 53.1875: 1 + 12 + the summary's 27.1875 + 12 + 1. The answer and its 12 px top margin take no room |
| The same, `open` | 760 × 92.375: the answer 27.1875 below a 12 px margin |
| The closed answer `<p>` | `checkVisibility()` false; drawn nowhere in the screenshot |
| `getComputedStyle(details, '::details-content')` | closed `display: block; content-visibility: hidden`, open `visible` |
| `details.innerText`, closed | `"How fresh is the coffee?"` — the answer is not text a person reads |
| The summary | `display: list-item`, `list-style-type: disclosure-closed` (open: `disclosure-open`), `list-style-position: inside` |
| Its text's start | 16.9375 right of the summary's content edge at 16 px, 33.90625 at 32 px — the same 16.9375 in DejaVu Sans, Courier and Times. Drawn as a filled right-pointing triangle, a down-pointing one when open |
| `display: flex; flex-direction: column; gap: 10px` on a closed `details`, line height 20 | 30 tall: the summary, **a gap**, and a content box of no size |
| The same with `display: grid; row-gap: 10px` | 30 tall, the same way |
| A `<summary>` written after the content | laid out first: the `details` is 20 tall and the summary at its top |
| A second `<summary>` in an open `details` | `display: block`, `list-style-type: disc` inherited, no marker: an ordinary block |
| A `details` with no `<summary>` | 53.1875 tall with the FAQ's rules, as if a summary were there; `innerText` `""` |

## The decision in one line

A `<details>` is **its summary, then a content box**; while it has no
`open` attribute that content box is **present, takes no room and holds
nothing that is laid out, drawn or read**; its summary — **the first
`<summary>` child, and only that one** — carries the **`expanded` state**
and is what an agent operates; **`Activate` on it toggles `open`**, by
HTML's activation behaviour, in `alo-dom`.

## Why this is a decision rather than a chore

Three things are decided here that code would otherwise decide by accident.

**What an agent may read of what is hidden.** The answer is in the
document. A screen reader does not speak it, `innerText` does not hold it,
and a person cannot see it. But it is not gone the way `display: none` is:
the page means it to be one action away. Reading it would let an agent
answer from text its person has never seen — the DOM-scraping failure ADR
0002 exists to prevent. Not reading it at all, with nothing said, would
hide that there is more.

**Where the state lives.** This engine puts `expanded` on the `details`.
HTML-AAM puts it on the summary, which every platform exposes as the
button that opens it. An agent looking for the thing to press and the
thing that says whether it is open should find one node, as a screen
reader does.

**Whether opening it is a verb.** It is already a verb — `Activate` on a
summary — that does nothing. Either it does what a person's click does, or
it is refused; a verb that reports success and changes nothing is the
failure the *Typed verbs* roadmap line was reopened for once (queue item
42).

## 1. A `details` is its summary, then its content box

The box tree builds a `details`' boxes in two slots, as HTML's rendering
section does:

- **the summary slot**: its first `<summary>` child, **wherever it is
  written among the children** — first, as Chrome lays it out;
- **the content slot**: an **anonymous block box** holding every other
  child, in document order, a second `<summary>` included, which is an
  ordinary element there.

The content box is a real box of the `details`, so a `details` that is a
flex or grid container has two items, and the gap between them — measured
above — is kept. It takes the `details`' style as an anonymous box does
(`nearest_style`), and has none of its own until a page styles
`::details-content`, which is **recorded as an issue naming it**, as ADR
0043 § 3 records what it does not read.

**A `details` with no `<summary>`** gets one in a browser — the localized
word "Details", made by the user agent. Making text the page did not write
is its own decision, cut to its own item. Until then, such a `details` has
an **empty summary slot**: closed, it is its padding and border, and an
issue records that its summary was not made. That is absent rather than
approximate, and it says so.

## 2. Closed, the content box is there, takes no room and holds nothing

While the `details` has no `open` attribute:

- **The content box is laid out with no size**: zero tall and, where it
  would be sized by its content, zero wide. Its children make **no
  boxes**, so nothing inside it is measured, placed, painted or read. Its
  children's margins are inside it, and so are not anywhere.
- **This is not `content-visibility` being built.** The property, its
  `auto` value and *findable* content (find in page, `hidden=until-found`)
  are cut until a page uses one; the slot behaves as `content-visibility:
  hidden` on a box no page here styles, and nothing more is claimed.
- **Open, the content box is an ordinary block** and the `details` lays
  out exactly as it does today, so every `open` case's numbers stand.

The summary is laid out the same open or closed.

## 3. An agent reads the summary and its state, and nothing it cannot see

- **The summary carries `expanded`**: `summary "How fresh is the coffee?"
  [expanded=false]`, `true` when open. It is the first `<summary>` child of
  its `details`, by § 1's rule, and **only it** has the `Summary` role; a
  `summary` anywhere else is generic, as HTML-AAM says.
- **The `details` is a `group` and carries no `expanded`.** One node says
  whether it is open, and it is the one an agent presses. An author's own
  `aria-expanded` on either element is still read on that element, as
  today.
- **What is hidden is not in the tree**: no node, no text, no name drawn
  from it — read exactly as if its boxes were never made, because they were
  not. The tree is still one tree (ADR 0002): an agent reads what is on
  screen and is told, by `expanded=false`, that more is one action away.
- **A summary's name** is its content, as today.

## 4. `Activate` on the summary toggles `open`, in `alo-dom`

HTML gives a `summary` an **activation behaviour**: if it is the summary
of its parent `details`, toggle that `details`' `open` attribute. It is
activation behaviour, not pre-activation behaviour: it runs only after a
click nobody cancelled. So, by ADR 0018 § 6, it lives once in `alo-dom`'s
`activation.rs`, beside a link's and a button's, for both callers — an
agent's `Activate` and a script's `click()`.

- **The activation target is found as today**: a click on text inside the
  summary bubbles to it; a click on a link or button inside the summary is
  that control's, as HTML says (the nearest ancestor with a behaviour).
- **On a page that has never run script**, `apply` reports the change it
  made — `open` set or removed on the `details` — and the page is rendered
  again from the same document, so held ids stay valid, as every verb does.
  This is **not** ADR 0018 § 7's accommodation: it is what a browser does,
  not an ARIA promise kept on an author's behalf.
- **On a page that runs script**, the click is dispatched and, if not
  cancelled, the same behaviour toggles `open`.
- **A summary that is not its `details`' summary** has no behaviour, and
  is generic (§ 3), so `Activate` refuses it as not operable.

The `toggle` event, the `open` IDL attribute, and `name` (exclusive
accordions) are script's and are cut until a page uses them.

## 5. The disclosure triangle is the summary's marker, and comes after this

Chrome draws the summary as a `list-item` with an **inside** marker,
`disclosure-closed` or `disclosure-open`, and its text starts 16.9375 to
the right at 16 px. That is ADR 0044's marker in the position it cut to
item 404, with two counter styles it did not build. **This page now asks
for `inside`**, so 404 is opened by it — but building the marker is not
429's: the user-agent rules (`details > summary:first-of-type {
display: list-item; list-style: disclosure-closed inside }`, `disclosure-open`
when open), the inside marker and the triangle's shape and size are cut to
their own item. Whether Chrome's triangle is a glyph or a drawn shape was
not established by the measurement above (the same width in three
families says only that it does not follow the page's font), and that item
decides it, measured.

Until it is built **no triangle is drawn** and the summary's text starts at
its content edge: 16.9375 left of Chrome's at the FAQ's size. That is a
known, stated difference in one number, and absent beats approximate.

## 6. What closes queue item 429

429 closes when:

- the frozen FAQ's `details` is **53.2** tall (1 + 12 + 27.2 + 12 + 1;
  Chrome's 53.1875 is its 1/64 layout unit), the CTA section after it
  moves up by 39.2, and its answer is neither in `layout.txt`,
  `display.txt` nor `agent.txt` — **in numbers**, in a test beside the
  page's other tests;
- the FAQ's outline reads `group` with no state, then `summary "How fresh
  is the coffee?" [expanded=false]`;
- `Activate` on that summary sets `open`, and the page rendered again
  reads `[expanded=true]`, the answer's text, and a `details` 92.4 tall;
  a second `Activate` closes it;
- a closed `details` that is a flex column with a 10 px gap is the
  summary's height plus 10, a summary written last is laid out first, a
  second summary is an ordinary block and generic, and a `details` with no
  summary is its padding and border with its issue recorded — each in
  numbers;
- every corpus reference that moves is reviewed in the same commit, and
  the `open` cases do not move.

## What this costs

**No triangle yet.** The FAQ's question sits 16.9375 px left of where a
browser puts it until the cut item lands.

**Hidden text is not findable.** An agent cannot search for the answer of
a closed question; it has to open it, as a person does. Find in page,
when it comes, decides findable content for both.

**Outlines move.** Every case with a `details` loses `expanded` on the
group and gains it on the summary; that is the point.

## Alternatives rejected

**Make no box for the hidden content**, as `display: none` would. Simpler,
and wrong by a gap: a flex or grid `details` is 10 px shorter than
Chrome's. Rejected (§ 1).

**Read the hidden content as hidden nodes**, marked so. An agent would
act on, and answer from, what nobody on screen can see — the reason the DOM
was rejected in ADR 0002. Rejected (§ 3).

**Keep `expanded` on the `details`.** The platforms put it on the summary,
and an agent would find the state on one node and the thing to press on
another. Rejected (§ 3).

**Refuse `Activate` on a summary** until script is built. A person's click
opens it with no script at all; the behaviour is the document's.
Rejected (§ 4).

**Build `::details-content` and `content-visibility` whole.** Two features
no page here styles, for one slot. Rejected for now, cut (§ 2).

**Draw the triangle in this item.** It is ADR 0044's `inside` marker and
two counter styles, a size rule this ADR could not settle by measurement,
and twice the work of the item that asked. Rejected, cut (§ 5).

## What this does not decide

- **The disclosure triangle** (§ 5), with 404's `inside`.
- **A generated "Details" summary**, and in which language.
- **`content-visibility`**, `::details-content`'s styling, `hidden=until-found`
  and find in page.
- **Script**: the `toggle` event, `HTMLDetailsElement.open`, and the `name`
  attribute's exclusive accordion.
- **`<dialog open>`**, the other element `open_flag` reads, whose
  `expanded` is untouched here.

## How we will know if this was wrong

**If an agent answers from a closed `details`' content**, or cannot tell
that a closed one has more, § 3 failed.

**If opening a `details` moves anything above it**, or closing one leaves
a gap a browser does not, §§ 1–2 were wrong.

**If a page's FAQ opens for a person and not for an agent**, § 4 failed.
