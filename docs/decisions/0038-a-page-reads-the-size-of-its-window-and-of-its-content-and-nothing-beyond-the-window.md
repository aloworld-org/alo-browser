# ADR 0038 — A page reads the size of its window and of its content, and nothing beyond the window

**Status:** accepted
**Date:** 2026-10-10
**Context:** queue item 366, *the viewport a script reads*, opened by a page:
`alo-sites-cta`, the call-to-action section alo Sites publishes for its
customers. Its analytics script reads `window.scrollY` and
`window.innerHeight` in `record`, `window.innerWidth` in `shape()`, and
`document.documentElement.scrollHeight` and `document.body.scrollHeight` in
`height()`. Its click listener also reads `documentElement.scrollWidth`. Each
answers `undefined` today. So `record`'s arithmetic is `NaN`, the depth it
reports is `0`, and the width is `0`. The item is marked **needs ADR**
because each of these tells a page something about the person's window.

Also read:
- ADR 0030, whose rule is *nothing about the machine*;
- ADR 0037 § *What this does not decide*, which names these members as
  undecided;
- ADR 0036 § 1, which answered "the engine needs a fact it may not read" for
  a clock with a trait the embedder implements;
- ADR 0013 § 3 (*absent beats approximate*) and § 6 (the engine knows no
  DOM);
- ADR 0018 § 4 (an agent's verb cannot be singled out) and ADR 0002 (the
  layout tree is the agent's tree);
- ADR 0005 (the renderer is told; the browser process knows);
- `CLAUDE.md`'s law 1 (no quirks mode).

And the code this is about:
- `alo-renderer`'s `page.rs`: `Page::viewport`, the size the browser process
  sends with the page.
- `renderer.rs`: `resize`, which changes that size and draws again; and
  `fresh`, which draws again only when the document's change count has
  moved.
- `pipeline.rs`: `draw`, which lays out at `Page::viewport` and gives
  the same size to media queries and to `vw` and `vh` through
  `MediaContext::sized`.
- `alo-layout`'s `tree.rs`: `BoxGeometry::scrollable`, the size of a box's
  content including what spills out of it.
- `alo-window`'s `place.rs`: `viewport`, the window's pixels divided by the
  scale, which is whole CSS pixels by construction. `window.rs`'s
  `FIRST_SIZE`.
- `alo-agent`'s `apply.rs`: an agent's `Scroll` changes nothing in the
  document or the frame.
- `alo-bindings`' `Cargo.toml`, which does not depend on `alo-layout`.

## The decision in one line

A page may read **its own viewport and its own content's size**:
`innerWidth` and `innerHeight` are the size the renderer lays the page out
at, in whole CSS pixels. `scrollX` and `scrollY` are the viewport's scroll
position, which the renderer holds and which is **zero** because nothing
scrolls a viewport yet. An element's `scrollWidth` and `scrollHeight` are
its **scrolling area, measured from the layout the page would be drawn
with at the moment the page asks**. `alo-bindings` **asks** for each through
one trait the renderer implements, and never lays anything out itself.
Nothing beyond the window is answered: the screen, the window's place on
it, its outer size and the scale factor stay absent.

## Why this is a decision rather than a chore

These look like getters for numbers the renderer already has. Three things
in them are not chores, and the first line of code would decide each one by
accident.

**What a page may learn.** The window's size is a fact about the person's
screen and how they arranged it. Combined with the screen's size and scale,
it is one of the strongest signals a fingerprinting script reads. ADR
0030's rule says the browser tells a page nothing about the machine that it
did not choose to. Something has to say where the window ends and the
machine begins.

**When a size is true.** `scrollHeight` is a number from layout. Layout
happens when the renderer draws, and a script runs between draws. A
script that adds a row and then reads `scrollHeight` reads either the
document as it is now or the document as it was last drawn. Every other
browser answers *now*, which means laying out in the middle of a script.

**Who measures.** `alo-bindings` has no layout and depends on no crate
that has one. The renderer has the pipeline, the sheets, the fonts and the
policy judgment that decides what is drawn. A number a script reads that
was laid out by anything else could disagree with the page the person sees.

## 1. The window is the page's, and the machine is not

What a page may read is decided by **what its layout already depends on**:
- The viewport's size is already the page's. `vw`, `vh` and every
  `@media (width …)` and `(height …)` rule are resolved against it in
  `pipeline.rs`. Every box's size is a function of it, and a page's CSS can
  show or hide things by it. A page that wanted the width could set an
  element to `100vw` and measure it once § 4 exists. Refusing
  `innerWidth` would hide nothing and break every page that reads it
  honestly.
- The page's own content is the page's. Its scrolling area is a function
  of its markup, its sheets and the viewport, all of which it already has.
- The scroll position is what the person or the page did to the page.

So these are answered: `innerWidth`, `innerHeight`, `scrollX`, `scrollY`,
`pageXOffset` and `pageYOffset` on the `Window`, and `scrollWidth` and
`scrollHeight` on `Element`.

**What is beyond the window is not answered**: `screen` and everything on
it, `outerWidth` and `outerHeight`, `screenX`, `screenY`, `screenLeft` and
`screenTop`, and `devicePixelRatio`. None of these is anything the page's
layout depends on. Each is a fact about the display, the window's place on
it or the system's scale. They stay **absent** (ADR 0013 § 3). Each is
opened by a frozen page that fails without it. That page's item then
answers under ADR 0030's rule: the same value for everybody, or the least
informative value the specification allows.

## 2. The viewport is the size the renderer is told, read when it is asked

`innerWidth` and `innerHeight` are the width and height of `Page::viewport`,
in CSS pixels. That is the size the page is laid out at, the size `vw` and
`vh` are resolved against, and the size `Resize` changes.
- **It is read at each read**, from the renderer, never copied into the
  heap when the page is installed. A page's `resize` changes it, and a read
  after a resize must answer the new size. ADR 0030's identity is fixed for
  a page's life and so is handed over once. This is not fixed.
- **It is a whole number of CSS pixels.** Both members are Web IDL `long`.
  `alo-window` sends whole CSS pixels by construction: `place.rs` divides
  whole device pixels by a whole replication. A size with a fraction can only
  come from a test or a future embedder, so the rule is stated rather than
  assumed:
  - the value is rounded to the nearest integer, with a half rounded up;
  - it is clamped to `0 ..= 2³¹ − 1`;
  - a value that is not finite answers `0`.
- **No scrollbar is subtracted, and none is added.** CSSOM View's
  `innerWidth` includes a scrollbar's width. This renderer draws no
  scrollbar, so the viewport is the whole of what the page is laid out in,
  and the two definitions agree.

## 3. The scroll position is the renderer's, and today it is zero

`scrollX` and `scrollY` are the viewport's scroll position, in CSS pixels.
`pageXOffset` and `pageYOffset` are CSSOM View's other names for the same
two getters. They are not legacy surface: the specification defines them as
the same getters, so they are built with them.

**Today that position is zero, and zero is the truth, not an approximation.**
Nothing in this browser scrolls a viewport:
- `alo-window` takes no wheel or key that scrolls;
- an agent's `Scroll` decides an outcome and changes nothing in the
  document or the frame (`alo-agent`'s `apply.rs`);
- `scrollTo`, `scrollBy`, `scrollIntoView` and navigating to a fragment do
  not exist.

The frame is always the top of the page, and a page reading `0` reads where
it is. ADR 0013 § 3 forbids a value a page cannot tell from the truth. This
value is the truth.

**So the renderer holds the position, in one place, from the start.** The
trait in § 5 asks the renderer for it, and the renderer answers from its own
state, which is zero. **Whatever first scrolls a viewport changes that
state, and is not built without it.** A window that scrolls on a wheel, an
agent's `Scroll` reaching the frame or `scrollTo` must each move the one
position that `scrollX` reads. Otherwise the frame and the page would
disagree about where the page is. This is ADR 0030 § 3's rule, *never built
apart*, applied to a position.

What grain a non-zero position has, and whether it may be a fraction, is
decided by that item. `scrollX` is a Web IDL `double`, so either is
expressible.

## 4. An element's scrolling area, from the layout the page would be drawn with, now

`scrollWidth` and `scrollHeight` follow CSSOM View's steps, without the
quirks branch (law 1):
1. **An element with no box answers `0`.** That covers an element in a
   detached tree, one under `display: none`, and one in a document with no
   view.
2. **The root element answers the larger of the viewport's scrolling area
   and the viewport.** CSSOM View answers this for a document not in quirks
   mode. Here no document is in quirks mode: `alo-dom` records the parser's
   quirks signal and never honours it.
3. **Any other element answers its own scrolling area.** That is its
   padding box, extended by the scrollable overflow of what it contains
   **toward its end edges only**, right and bottom here. Overflow above or
   to the left cannot be scrolled to, so it is not in the area. The skip
   link `alo-sites-cta` places at `left: -999rem` must not make the root
   16 000 pixels wide.

   `body` takes this ordinary branch. CSSOM View gives `body` a special
   answer only in quirks mode.
4. **The value is rounded to the nearest integer, with a half rounded up**,
   and clamped as § 2 clamps. Both members are Web IDL `long`.

**The layout is the one the page would be drawn with if it were drawn
now**: the same sheets in the same order, the linked sheets that have
arrived, the same judgment of inline style under the page's policies, the
same fonts and the same viewport. A measurement is a draw that stops
before paint. It is **not** a second pipeline, because two pipelines are two
answers the day they disagree.

**At the moment it is asked.** If the document has not changed since the
last drawing, by the change count `fresh` already reads, and no sheet has
arrived since, the last drawing's layout answers. Otherwise the document
is laid out then, in the middle of the script. That layout is **kept as the
next drawing's**, so a page that changes the document once, reads
`scrollHeight`, and is then drawn is laid out once, not twice. Anything the
judgment objects to for the first time during a measurement is owed in the
next answer that carries objections, exactly as a draw's objections are
(queue item 346).

Laying out in the middle of a script is safe here because **layout runs no
script**: no JavaScript is reachable from style, boxes, text or layout. So
nothing can change the document while it is being measured, and a borrow
of the document for the measurement ends before the script resumes.

## 5. The bindings ask, and the renderer measures

`alo-bindings` defines one trait, **`View`**, with three questions:
- the viewport's size, for § 2;
- the viewport's scroll position, for § 3;
- the scrolling area of a node in a given document, for § 4. The answer
  is `None` when the node has no box.

The renderer implements it, in **one file in `alo-renderer`**, over the
page it holds and the drawing it keeps. The page is handed its `View`
when it is installed, as its engine is handed its `Clock` (ADR 0036 § 1).
`alo-bindings` keeps the view beside the `Window`. It is Rust state the
collector does not need to walk: it holds no reference into the heap.

- **`alo-bindings` does not depend on `alo-layout`**, and does not gain the
  dependency. It knows the questions, not how they are answered, as
  `alo-js` knows *what time is it* and not where the time comes from. This
  is what makes § 4's "one pipeline" enforceable: the only code that can
  lay a page out for a script is the code that draws it.
- **A page with no view refuses to say, by name.** Each member throws a
  `TypeError` saying the page was given no view. A made-up size would be
  the approximate answer ADR 0013 § 3 forbids. Only an embedder that is not
  a renderer, such as a test of `alo-bindings` alone, makes such a page.
  ADR 0036 § 1 treats a realm with no clock the same way.
- **The measurement is bounded by the document**, which the heap's limit
  already bounds. The engine's stop (ADR 0031 § 7) is asked between steps,
  not inside one measurement. A page that changes the document and reads
  `scrollHeight` in a loop lays out on every pass. That is quadratic, as
  it is in every browser. It is the page's cost, and the page can still be
  stopped between passes.

## 6. The same answers whoever drives

ADR 0018 § 4 says a page cannot single out an agent. So these members answer
the same way for a page in a window and for a page in a tab no window shows.
Both answer from the size the renderer lays out at.

That leaves one thing to decide: **the size of a tab no window shows.** A
distinctive size, such as `0 × 0` or an unusual fixed size, would mark every
agent's session to every page that reads `innerWidth`. And `0 × 0` would also
draw nothing (`Failure::Unpaintable`). So until the person chooses a size for
their agent's tabs, such a tab is told **the size a new window opens at**:
`alo-window`'s first size, 1000 × 700 CSS pixels. Where that choice is
kept, and whether an agent session may ask for another, is item 133's
(*under grants, and recorded*).

## 7. The cut

- **366. The viewport a script reads.** *Depends on 362 (done).* Builds:
  - `View` in `alo-bindings`, and the renderer's implementation over
    `Page::viewport` and a scroll position it holds at zero;
  - `innerWidth`, `innerHeight`, `scrollX`, `scrollY`, `pageXOffset` and
    `pageYOffset` as the `Window`'s own accessors (ADR 0037 § 4: a
    `[Global]` interface's attributes are its own). Each is `[Replaceable]`,
    as CSSOM View declares it, so a page's assignment replaces it, as `self`
    is replaced;
  - a page with no view refusing by name.

  *Closes when:*
  - `alo-sites-cta`'s `shape()` answers `&p=%2F&w=800` at 800 × 600, in
    `tests/alo_sites_cta.rs`;
  - `innerHeight` answers 600, and both scroll positions answer 0;
  - after a `Resize` to 640 × 480, a script run in the page reads 640 and
    480;
  - a fractional, a negative and an infinite viewport answer as § 2 says,
    in unit tests;
  - every script runs ordinarily and under `Heap::stress`.
- **370. An element's scrolling area.** *Depends on 366.* Builds
  `scrollWidth` and `scrollHeight` on `Element` by § 4, the renderer's
  measurement (§ 4's *at the moment it is asked*, with the layout kept for
  the next draw), and § 6's windowless size if nothing gives it first.
  *Closes when:*
  - at 800 × 600, `alo-sites-cta`'s `documentElement.scrollHeight` is 600,
    `body.scrollHeight` is 253 and `documentElement.scrollWidth` is 800 —
    the skip link's overflow to the left is not counted;
  - with a beacon the test lends, its `pagehide` listener sends
    `d=1000&p=%2F&w=800` and then `t=0`;
  - a script that appends a tall element and reads `scrollHeight` in the
    same task reads the new height, and the next draw does not lay out
    again;
  - an element with no box, and one in a detached tree, answer 0;
  - every script runs ordinarily and under `Heap::stress`.

## What this costs

- **The window's size is told to every page.** Every browser does this,
  and § 1 shows that refusing it would hide nothing that `vw` and a
  measured element would not reveal. It still means a page can tell two
  people apart by how they size their windows. The person can change that;
  the page cannot.
- **Layout in the middle of a script.** A getter that can take as long as a
  draw is a new kind of cost in `alo-bindings`, and a page that thrashes
  pays it on every read. It is the cost every engine pays, and the
  alternative is a wrong number.
- **One more trait across a boundary.** `View` is a second embedder
  question beside the clock. It is three questions, and the renderer
  answers them from state it already keeps.

## Alternatives rejected

- **Leaving every member absent.** The page's arithmetic stays `NaN`, its
  reports say depth `0` and width `0`, and nothing is hidden, by § 1.
  Absent beats approximate, but here the true answer is available and not
  sensitive beyond what layout already reveals.
- **Letterboxing**: answering the window's size rounded down to a coarse
  grid, as Tor Browser does, and laying out at that size inside a margin.
  It protects only if `vw`, `@media` and every measurement lie too, which
  means drawing the page smaller than the window the person chose and
  leaving the rest of it blank. That is a cost every person pays, all the
  time, for a protection only a browser built around anonymity can make
  hold. Reopened if stage 2's week finds sites that fingerprint by window
  size in a way a person can name.
- **The last drawing's layout.** It answers a script that just added a row
  with the height from before the row. That is a number a page cannot tell
  from the truth and is wrong, which is exactly what ADR 0013 § 3 forbids.
- **`alo-bindings` laying out itself**, by depending on `alo-layout`,
  `alo-style` and `alo-box`. It would be a second pipeline without the
  page's linked sheets, its fonts or its policy judgment, which only the
  renderer holds. It would answer one number while the screen showed
  another.
- **Copying the viewport into the heap at install**, as the identity is.
  It would be stale after the first `Resize`.
- **A fixed size for every page**, so that no window size reaches any page.
  The page would be laid out for a window the person does not have, unless
  layout used the fixed size too, which is letterboxing.

## What this does not decide

- **The screen, the window's place and outer size, and the scale factor**
  (§ 1). Each stays absent until a page opens it.
- **`clientWidth`, `clientHeight`, `offsetWidth`, `getBoundingClientRect`,
  `getClientRects`** and the rest of an element's geometry. Each would be
  answered through § 5's trait and § 4's moment, but each is opened by a
  page, not by this ADR.
- **Scrolling**: `scrollTo`, `scrollBy`, `scrollIntoView`, `scrollTop`
  written, the `scroll` event, and a window or an agent scrolling the frame.
  § 3 says only that whatever does it moves the one position.
- **The `resize` event** at the window, and when it is fired after a
  `Resize`. It is opened by a page that listens for it.
- **`visualViewport`, `matchMedia`**, and pinch zoom.
- **The page lifecycle** (item 364) and **`navigator.sendBeacon`** (item
  369), which `alo-sites-cta` also needs before it reports anything.

## How we will know if this was wrong

**If a frozen page reads a different `scrollHeight` or `scrollWidth` than
another engine reads for the same markup at the same size**, § 4's area is
wrong, or the layout under it is. It is fixed in the renderer's one
measurement, which is also the drawing's layout, so the fix moves a
reference render too and says so.

**If stage 2's week finds sites that tell people apart by window size** in a
way a person can name, § 1's reasoning undercounted the cost. The remedy
is a decision about letterboxing, which changes layout and is never a
change to these members alone.

**If a page is found that needs a member § 1 leaves absent**, it is opened
as an item and answered under ADR 0030's rule, never with the machine's
real value.
