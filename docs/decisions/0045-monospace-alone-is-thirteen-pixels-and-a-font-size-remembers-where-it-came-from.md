# ADR 0045 — `monospace` alone is thirteen pixels, and a font size remembers where it came from

**Status:** accepted
**Date:** 2026-10-11
**Context:** queue item 415, *`monospace`'s own default size*, cut from
item 412 in iteration 265 and marked **needs ADR**. `alo-sites-rich-blocks`
is the golden output of alo Sites' BlockNote renderer, frozen without a
sheet so the user-agent sheet draws it. Its two `<pre>` blocks and its
equation's `<code>` are `font-family: monospace` from that sheet and
nothing else, and since item 412 they are drawn in DejaVu Sans Mono — at
16 px, the size they inherit. Every browser a person compares us with draws
them at 13. Item 412 left that alone on purpose: the 13 is not a property
of the face but a rule about the generic, which moves every `em` under it,
and LOOP.md § 4 makes a rule like that its own decision.

Also read: ADR 0001 (law 1, the modern platform and not its legacy);
ADR 0013 § 3 (*absent beats approximate*); ADR 0036 § 1 and ADR 0042 § 1
(a fact that is the person's is told to the engine, and a test reads a
fixed one); `alo-style`'s `metrics.rs` (`DEFAULT_FONT_SIZE`, the keyword
ratios, `font_size_against`), `computed.rs` (`settle_font`) and
`user_agent.rs` (`code, kbd, samp, pre { font-family: monospace }`);
`alo-corpus`' `corpus_fonts` and `alo-window`'s `fonts::frozen` (both map
`monospace` to DejaVu Sans Mono since item 412); the case's `page.html`
and `origin.txt`, and `alo-downloads`' `.note code`.

**Measured 2026-10-11, on this machine, in Google Chrome 154.0.8037.98**,
headless, by pages written for this decision that read
`getComputedStyle(…).fontSize` (kept nowhere; each is a few lines and is
re-derivable from the table in § 3). Chromium's source was not read for
this ADR, and no other engine is installed here, so nothing below claims
what Firefox or Safari do. The frozen `alo-sites-rich-blocks` page itself,
with only a reading script appended, came back with both `pre`s, both
`code`s inside them and the equation's `code` at **13px**, each `pre`'s
top margin **13px**, and the caption at 16px.

## The decision in one line

A computed font size **carries where it came from** — a keyword, a size
relative to its parent's font, or a fixed one; an element whose computed
`font-family` is **exactly the generic `monospace`** has a `medium` of
**13 px** rather than 16; and where an element's family crosses that line
from its parent's, a keyword size is **read again** in the new family, a
relative size is **rescaled by 13/16** (or back by 16/13), and a fixed size
**stays**.

## Why this is a decision rather than a chore

**Nothing specifies it.** CSS Fonts leaves the generic's size to the user
agent, as item 415 records, and the rescaling of relative sizes is written
in no specification at all. It is a convention every engine has kept for
decades, which is exactly the shape law 1 is suspicious of.

**It changes what a font size is.** Today a computed font size is a number
of pixels. Taking the rule means carrying, beside every one of them, how it
was reached, because two elements at 16 px can need different answers
under one `<code>`.

**It moves things nobody asked to move.** Every `em` inside code — its
margins, its padding, the line height — follows the size.

## Why we take it

Law 1 refuses what exists to keep broken pages working: quirks mode, floats
as layout, `document.write`. This is not that. It is what every engine
does in standards mode, to pages written today, and authors write against
it: normalize.css's `font-family: monospace, monospace` exists only to step
out of it. alo Sites' rich blocks are a page alo publishes, and the people
reading a post on one see code at 13 px in their browser. *The measure is
alo*: a code block 23 % larger than the one alo's own customers see is a
wrong render of alo's page, whatever a specification declines to say.

## 1. A computed font size carries its origin

Beside its pixels, every computed font size is one of:

- **Keyword**, naming which of the eight absolute-size keywords
  (`xx-small` … `xxx-large`). The root's initial `medium` is one. A
  keyword size is always the keyword's size *in the element's own family*
  (§ 3).
- **Relative**: reached through the parent's font — `em`, `ex`, `ch`,
  `cap`, `ic`, `lh`, a percentage, `smaller` or `larger` — from a parent
  whose size is a keyword or relative.
- **Fixed**: everything else. `px` and the other absolute units; `rem` and
  `rlh`, which are the root's; the viewport units; **any math function**
  (`calc()`, `min()`, `max()`, `clamp()`), whatever it holds; and a
  relative length of a fixed parent.

**Inheriting keeps the origin**: an element that sets no `font-size` has
its parent's size *and* its parent's origin, and § 4 then applies to it if
its family differs.

Measured, in the table below: `calc(1em + 0px)` on a `<code>` is 16, not
13, so a math function is fixed even when it is only `em` — Chrome treats it
so, and this engine follows rather than inventing a finer rule; `max(1em,
10px)` on a parent leaves its `<code>` at 16; `0.86rem` is 13.76 either way;
`1em` of a 20 px parent is 20 under `monospace`.

## 2. Which family is `monospace`-sized

An element is **`monospace`-sized** when its computed `font-family` is one
family and that family is the generic `monospace`, compared ASCII
case-insensitively. **Nothing else is**: not `monospace, monospace`, not
`"Courier New", monospace`, not `ui-monospace`, not a quoted
`"monospace"`, which names a family rather than the generic. The test is
on the computed value after `var()`, so one written through a custom
property counts.

Measured: `MONOSPACE` is 13; `monospace, monospace`, `"Courier New",
monospace` and `ui-monospace` are 16.

## 3. `medium` is the family's, and the keywords are ratios of it

There are two mediums: **16 px**, and **13 px** for a `monospace`-sized
element. Each absolute-size keyword stays the ratio of `medium`
`metrics.rs` already holds (CSS Fonts' 3/5, 3/4, 8/9, 1, 6/5, 3/2, 2, 3),
applied to the element's own medium: `large` is 19.2 in sans-serif and
15.6 in `monospace`.

**This is not what Chrome draws for every keyword**, and that is said here
rather than discovered later. Chrome sizes the keywords from an integer
table, not from the ratios:

| keyword | Chrome, 16 medium | Chrome, `monospace` | here today, 16 | here, `monospace` |
|---|---|---|---|---|
| `xx-small` | 9 | 9 | 9.6 | 7.8 |
| `x-small` | 10 | 10 | 12 | 9.75 |
| `small` | 13 | 12 | 14.22 | 11.56 |
| `medium` | 16 | 13 | 16 | 13 |
| `large` | 18 | 16 | 19.2 | 15.6 |
| `x-large` | 24 | 20 | 24 | 19.5 |
| `xx-large` | 32 | 26 | 32 | 26 |
| `xxx-large` | 48 | 39 | 48 | 39 |

The sans-serif column already differs at four of eight keywords, before
this ADR, so the question is not `monospace`'s: it is whether this engine's
keywords are CSS's ratios or the engines' table, and it is one question for
both families. It is **cut to queue item 416**, waiting for a page that
writes a keyword size. Until then `monospace` follows the rule its sibling
already follows, so the two columns change together or not at all.

## 4. Crossing the line

Where an element is `monospace`-sized and its parent is not, or the other
way round (the root's parent being an element with neither):

- a **keyword** size is that keyword's size in the element's family (§ 3);
- a **relative** size is multiplied by the element's medium over the
  parent's — 13/16 going into `monospace`, 16/13 coming out;
- a **fixed** size is left as it is.

Where both are on the same side, nothing happens: a size is computed
exactly as today. So a page that never writes `monospace` alone computes
every size it computes now.

Measured, beside what this rule gives. The two rows that differ in
number differ only because `large` is 18 in Chrome's table and 19.2 here (§ 3):

| case | Chrome | here |
|---|---|---|
| `<pre>`, `<code>`, `font-family: monospace` under the body's `medium` | 13 | 13 |
| `<code>` with `font-size: 1em` or `100%` | 13 | 13 |
| `<code>` with `font-size: medium` | 13 | 13 |
| `<code>` with `font-size: smaller` (16 / 1.2, then × 13/16) | 10.8333 | 10.8333 |
| `<code>` under a `1.25em` or `125%` parent (20 × 13/16) | 16.25 | 16.25 |
| `<code>` under a `large` parent (the keyword read again) | 16 | 15.6 (§ 3, item 416) |
| `font-family: monospace; font-size: 1em` under a `large` parent (`large` × 13/16) | 14.625 | 15.6 |
| `<code>` under a 20 px, `2rem` or `3vw` parent | 20, 32, 22.68 | 20, 32, 22.68 |
| `<h1>` inside a `monospace` block (2em of 13) | 26 | 26 |
| `font-size: 2em; font-family: sans-serif` inside one (26 × 16/13) | 32 | 32 |
| `<span style="font-family: sans-serif">` inside a `<pre>` | 16 | 16 |
| the same, inside a `monospace` block whose size is 14 px | 14 | 14 |
| `<b>` inside a `<pre>` | 13 | 13 |
| `<html style="font-family: monospace">`, its `<p>`, and `1rem` under it | 13, 13, 13 | 13, 13, 13 |
| `<code>` at `2ch`, a `<span>` at `2ch` (Chrome's `ch` is 8 in its default face) | 13, 16 | the `<span>`'s × 13/16, and the `<span>`'s |

## 5. The two mediums are the person's, later

16 and 13 are what a browser ships, and a browser lets the person change
both. Here they become **two named constants in `alo-style`** beside
`DEFAULT_FONT_SIZE`, and the ratio in § 4 is computed from them, never
written as `0.8125`. When settings exist (item 128), the person's two sizes
are told to a renderer as ADR 0036 § 1 tells it a clock, and a test, the
corpus and `--frozen-fonts` keep the shipped 16 and 13, so a frozen page's
reference does not depend on whose machine drew it.

## 6. What does not change

- **Layout, paint and the text stack read one number**, the computed size,
  as they do now. The origin is the style's business and stops there.
- **An agent reads nothing new.** A font size is not in the agent's tree.
  A script reading the computed `font-size` reads the pixels, `13px`.
- **The face is unchanged**: `monospace` is still DejaVu Sans Mono in the
  corpus and the frozen window (item 412).

## 7. What closes queue item 415

415 closes when:

- `alo-sites-rich-blocks`' two `<pre>`s and its equation's `<code>` are
  computed at **13 px**, in numbers: every code line's advance 13 × 1233 /
  2048, the indented line four of them in, each `pre`'s top and bottom
  margin 13, and every moved `layout.txt` number worked by hand;
- `alo-downloads`' two `.note code`s are **unmoved** at 13.76 (they are
  `0.86rem` on their parent, which is fixed), and so is every other case;
  each case reference that moves is looked at in that commit;
- unit tests show § 1's origins (every unit named there, a math function,
  inheriting), § 2's test (each measured spelling, and a `var()`), § 3's
  two mediums with a keyword in each, and every row of § 4's table;
- a mutation that drops the rescaling, or treats `calc()` as relative,
  fails a test.

## What this costs

**One more fact per computed font size**, and a step in `settle_font` that
reads the element's family before its size is final — the order the
cascade already allows, since `font-family` does not depend on size.

**A convention with no specification behind it**, chosen because the pages
alo publishes are drawn by it everywhere else. If engines drop it, this
goes with it.

## Alternatives rejected

**Keep 16.** Simple, specified by nothing either, and wrong on alo's own
page in every browser its readers use. Rejected (*why we take it*).

**Write `font-size: 0.8125em` on `code, kbd, samp, pre` in the user-agent
sheet.** Gets the plain case right and every other row of § 4 wrong: an
author's `font-family: monospace` on any element, `monospace, monospace`
(which would shrink when it should not), a keyword under code, and coming
back out of code. A rule that looks right on one page and is wrong on the
next is approximate. Rejected.

**Rescale every size, fixed ones included.** Then `0.86rem` and `14px`
code shrink, which Chrome does not do. Rejected.

**Treat a math function as relative when it holds `em`.** Finer, and not
what Chrome draws: `calc(1em + 0px)` is 16 there. Rejected; following the
engines here is the whole point of the decision.

**Take Chrome's keyword table for `monospace` alone.** Two families would
size keywords by two different rules. Rejected; cut to 416 for both.

## What this does not decide

- **Keyword sizes as the engines' integer table** rather than CSS's ratios,
  for every family — queue item 416, waiting for a page.
- **A minimum font size.** Chrome drew `1ex` and `1cap` on a `<code>` at
  9 px, which neither rule here explains and which is consistent with a
  minimum size; a minimum is a person's setting and comes with item 128.
- **`font-size-adjust`** and the `font` shorthand, which this engine does
  not read; each waits for a page.
- **Other generics.** Only `monospace` has a medium of its own; `serif`,
  `sans-serif`, `system-ui` and the `ui-*` families keep 16, as measured.

## How we will know if this was wrong

**If a page alo publishes draws code at a size its readers' browsers do
not**, § 1's origins or § 4's crossing rule are wrong for it — the corpus
case is where that shows.

**If a size moves that never touched `monospace` alone**, § 4's *same side,
nothing happens* was not kept.
