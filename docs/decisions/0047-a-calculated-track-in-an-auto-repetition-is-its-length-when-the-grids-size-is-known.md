# ADR 0047 — A calculated track in an auto repetition is its length, when the grid's size is known

**Status:** accepted
**Date:** 2026-10-11
**Context:** queue item 422, *A grid track whose size is a calculation,
repeated to fit*, found by `alo-sites-blog-index` (iteration 271) and marked
**needs ADR**. alo Sites' blog grid is

```css
.blog-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 18rem), 1fr));
  gap: 1.5rem;
}
```

Chrome 154 draws its cards in two columns of 368 at x 20 and 412. This
engine draws one column of rows 760 wide: `taffy` throws the whole template
away, the grid has no explicit columns, and every card is an implicit row.
Nothing is recorded, so the issue list does not say why.

Also read: ADR 0001 (rent the physics, build the engine); ADR 0004 (we own
the layout tree, `taffy` owns the algorithms — and the two ways round it
refuses: a fork, and a second layout pass); ADR 0013 § 3 (*absent beats
approximate*); `alo-layout`'s `arena.rs` (the taffy trait implementations
and `resolve_calc_value`), `engine.rs` (`template`, `track_function`,
`min_track`, `max_track`: a track length with a percentage in a
calculation becomes a calc handle) and `track.rs` (the grammar);
`scripts/gate.sh` (which files may name `taffy`); `taffy` 0.14.0's
`compute/grid/mod.rs` and `compute/grid/explicit_grid.rs`, and
`style/grid.rs`' `has_fixed_component` and `definite_value`, read on this
machine, and its `main` checked the same day by iteration 271; and CSS Grid
Layout 1 § 7.2.3.2 (*Repeat-to-fill*) and § 7.2.3 (the `<fixed-size>`
grammar).

## Evidence

**What the specification allows.** An `auto-fill` or `auto-fit` repetition
takes only `<fixed-size>` tracks: a `<fixed-breadth>`, or a `minmax()` with
one at either end. A `<fixed-breadth>` is a `<length-percentage [0,∞]>`, and
a math function — `calc()`, `min()`, `max()`, `clamp()` — is a
`<length-percentage>`. So `minmax(min(100%, 18rem), 1fr)` is valid, and is
exactly how a design system writes "cards at least 18rem, but never wider
than the grid".

**What `taffy` does, read rather than remembered.**
`compute_explicit_grid_size_in_axis` accepts a template with an auto
repetition only if every track `has_fixed_component`, and that asks
`is_length_or_percentage()` of each end — a tag test that a calc handle
fails. The template is then *invalid*, and the explicit grid is empty.
The same function's count, a few lines further on, already resolves a calc
handle through the tree (`definite_value` calls `tree.calc(handle,
basis)`): the arithmetic is there; only the gate before it refuses. The fix
upstream is one condition, and it is not ours to make in place (ADR 0004).

**What Chrome 154 does, measured** on this machine, headless, from a
scratch page whose script reads every grid's columns and items. The source
was not read. Every grid has `gap: 24px` and items 10 tall; the track is
`minmax(min(100%, 288px), 1fr)` unless it says *plain*, which is
`minmax(288px, 1fr)`; `auto-fit` unless it says `auto-fill`.

| Case | Container | Chrome's columns |
|---|---|---|
| A | `width: 760px`, 3 items | 368 368 |
| A plain | the same | 368 368 |
| B | `width: 760px`, 1 item | 760 0 (the empty track collapsed) |
| B `auto-fill` | the same | 368 368 |
| C | `width: 200px` | 200 (the `min()` takes the 100 %) |
| D | `width: 800px; padding: 20px; border: 3px; box-sizing: border-box` | 365 365, at 23 and 412 |
| E | `inline-grid`, size `auto`, items 50, 50, 0 wide | 50: one repetition, 50 wide |
| E plain | the same | 288 |
| F | `inline-grid`, `max-width: 700px` only | 198 |
| F plain | the same | 288 288 |
| G | `inline-grid`, `min-width: 700px` only | 338 338 |
| G plain | the same | 288 288 288 |

Two findings decide this ADR.

1. **Where the grid's size is definite (A–D), a calculated track behaves
   exactly as the length it resolves to.** A and A plain are the same grid;
   B collapses its empty track and B `auto-fill` does not, as § 7.2.3.2
   says of any track (B's plain twin was not measured); D resolves
   the percentage against the content box, 754. One number, used for both
   the count and the sizing.
2. **Where it is not (E–G), it does not.** The percentage has no basis
   while the grid's own size is being found, and Chrome's answers differ
   from the plain length's in every case: 50 against 288, 198 against 600,
   two columns against three. A rule that lowered the calculation to a
   length there would be wrong in numbers we can already see.

## The decision in one line

When `taffy` is about to lay out a grid whose template holds an `auto-fill`
or `auto-fit` repetition, and the grid's size in that axis is definite, each
calculated track size in that axis's template is resolved against the
grid's content box and handed to `taffy` as the plain length it is; where
the size is not definite the template is left as written, `taffy` still
refuses it, and the refusal is recorded.

## 1. When: the grid's size in the axis is definite

The condition is the one `taffy` itself uses for a grid's own size — its
`outer_node_size` — and nothing looser: the size the caller fixed
(`known_dimensions`), or else, when the call asks for the inherent size,
the `width` or `height` the style gives, resolved against the parent's size
and adjusted for `box-sizing`; clamped by the minimum and maximum; never
less than the padding and border. The content box is that less the padding
and border.

That is the one case in which the basis `taffy` counts repetitions against
(`auto_fit_container_size`) and the basis it later resolves percentage
tracks against are the same number, so resolving the calculation once is
exact rather than a guess. A grid with only a `max-width` or a `min-width`
(F, G) counts against one number and sizes against another, and is
refused, as is a grid with no definite size at all (E).

Where `taffy` would fold in something this rule does not — an aspect ratio
on the grid container, a scroll container's scrollbar gutter — the lowering
is refused rather than approximated. Neither is reachable today: this
engine hands `taffy` an aspect ratio only from the style or a replaced box,
and a scrollbar width of zero. A page that reaches either opens an item.

## 2. What is lowered, and what is not

- **Only a template with an auto repetition.** A template without one is
  valid to `taffy` already, and its calculated tracks are resolved by
  `resolve_calc_value` at the moment `taffy` knows the basis, which is the
  right moment (ADR 0004). Nothing about such a template changes.
- **Only the axis whose size is definite.** Columns and rows are decided
  apart, each against its own content box.
- **Every calculated track size in that template**, both ends of every
  `minmax()`, inside the repetition and outside it, because `taffy` asks
  the whole template to be fixed-size, not only the repeated part. A size
  that is not a calculation — a length, a percentage, `auto`, a keyword,
  a fraction — is left exactly as written.
- **Never the gap.** `taffy` resolves a calculated gap through
  `resolve_calc_value` on every path, the count included.
- **The resolved number is the calculation's.** It is
  `Unresolved::resolve(handle, content_box)`, the same function
  `resolve_calc_value` calls, so a track resolved here and the same track
  resolved by `taffy` on a template it accepts cannot disagree. `resolve`
  clamps nothing, and neither does the lowering: a negative result is
  handed to `taffy` as the same negative number it would have been handed
  through `resolve_calc_value`, so whatever `taffy` does with it is what it
  already does. That a track breadth below zero should be zero is a
  question about `resolve`, for every property, and not one this ADR
  answers in one place only.

## 3. Where: one file, which names `taffy`, and is deleted the day `taffy` counts a calculation

The lowering is a new file, `crates/alo-layout/src/auto_repeat.rs`: given a
grid node's style and the call's inputs, the style to lay it out with —
the node's own, or a copy with the calculations replaced. It names
`taffy`, because a template is `taffy`'s type, and `scripts/gate.sh`'s
boundary list gains it in the same change, beside `arena.rs` and
`engine.rs`.

It is not folded into `arena.rs` because it has a different reason to
change and a different end: `arena.rs` is the nodes `taffy` walks and the
answers to its questions, and changes when `taffy`'s traits do; this file
exists only because one condition upstream is narrower than the
specification, and the correct end of it is deletion. Its first line says
so, and names the condition, so that an upgrade of `taffy` that accepts a
calculated fixed size is noticed: the agreement tests below will still
pass with the file deleted.

**The node's style is never changed.** `arena.rs`' `compute_child_layout`
asks `auto_repeat` for the style when the node is a grid, keeps the copy
for the length of that one `compute_grid_layout` call, and
`get_grid_container_style` answers with it while it is held; the node's own
`Style` is untouched, so the next call, with other inputs, starts from what
the author wrote. The copy is a function of the inputs, and the inputs are
what `taffy`'s cache is keyed by, so a cached answer is never the answer to
a different template. A grid's items are other nodes and read their own
styles; a grid inside a grid holds its own copy for its own call.

## 4. A refusal is recorded

`taffy` discarding a template is today silent, which is the worst kind of
wrong: the cards are one column and nothing says why. So whenever a grid's
template holds an auto repetition with a calculated track in an axis whose
size is not definite, the arena records the node, and the engine reads the
record back after layout, as it reads the layout itself, and turns it into
one issue per node: the template and that its repetition could not be
counted without a definite size. One issue per node, not per call: a grid
is laid out many times in one layout.

The template is not otherwise touched in that case. Discarding it is
`taffy`'s behaviour and E–G show the right answer is neither discarding it
nor lowering it; reproducing Chrome's intrinsic-size behaviour for a
percentage with no basis is track sizing — `taffy`'s algorithm — and is cut
to its own item, **424**, waiting for a page that needs it.

## 5. Hostile input

The template comes from page bytes. The lowering adds no allocation the
page sizes beyond one copy of a style the page already caused, per grid
layout call, and no arithmetic of its own beyond the content box's
subtraction, which is on `f32` and cannot overflow into a panic. It visits
each track size of the template once, so its work is the length of a list
the page already wrote and `engine.rs` already walked to build it; a
repetition's count is bounded by `track.rs` at `u16::MAX` and the explicit
grid by `taffy`'s own track limit, neither of which the lowering touches.
The build tests a template of many calculated tracks and a deeply nested
grid of such templates, and that neither panics.

## 6. What closes queue item 422

The build iteration, under this ADR:

- `auto_repeat.rs`, `arena.rs`' hold of the copy, and the record and its
  issue, as above; the gate's boundary list.
- **Agreement tests**, in numbers, in `alo-layout`: for each of A, B, B
  `auto-fill`, C and D, the calculated grid lays out the same as the
  plain-length grid `taffy` counts by itself, and both as Chrome measured
  them above. These are what notice the day `taffy`'s own rule or ours
  drifts.
- **Refusals in numbers**: E, F and G keep today's layout and each records
  one issue.
- **The frozen page**: `alo-sites-blog-index`'s cards in two columns of 368
  at x 20 and 412, in `alo_sites_blog_index.rs`, its references rewritten,
  and every other corpus case unmoved — or each moved number explained.
- A mutation that skips the lowering fails the frozen page's test and the
  agreement tests; a mutation that lowers in an indefinite axis fails E–G.

## What this costs

- **A rule of `taffy`'s, written down twice.** The definite size is
  computed here by the rule `taffy` uses, and could drift from it on an
  upgrade. The agreement tests are the guard; they compare our lowered
  grid with `taffy`'s own handling of the equivalent plain one, so a
  drift in either fails them.
- **One copy of a style per grid layout call**, for the grids that need
  it. Measured by nobody, and claimed as nothing (`LOOP.md`: a speed claim
  is measured or not made).
- **E–G stay wrong**, now visibly.

## Alternatives rejected

**Fork `taffy`, or patch it in `Cargo.toml`.** The fix is one condition and
correct, and a fork still carries every future upstream change as a merge
for the sake of it (ADR 0004). A `[patch]` to a git revision is the same
thing under another name. Offering the condition upstream is right, and is
a person's action: publishing is outside the loop. If `taffy` takes it, the
file in § 3 is deleted and nothing else changes.

**Count the repetitions ourselves and hand `taffy` `repeat(N, …)`.** Exact
for `auto-fill`, wrong for `auto-fit`: a counted repetition is never
collapsed, so case B would be two tracks of 368 with one item in the first,
where Chrome gives the item all 760.

**Lower against whatever basis is to hand** — the max-size, the min-size,
the parent's size. F and G measure what that gets wrong.

**A second layout pass**: lay out with the template discarded, read the
grid's width, resolve, lay out again. ADR 0004 refused this as right almost
always and quietly wrong sometimes. It is also unnecessary: where the size
is definite it is known before the first pass, and where it is not, the
first pass's width is the wrong basis by E–G.

**Refuse the template at parse time.** CSS says a template that breaks the
auto-repetition rules is invalid at parse time, and this one does not break
them. Refusing a valid value would be a wrong pixel that looks deliberate.

## What this does not decide

- Chrome's sizing of a calculated percentage track with no definite basis
  (E–G). Track sizing is `taffy`'s; item 424 waits for a page.
- That `track.rs` accepts `repeat(auto-fit, 1fr)`, which the specification
  makes invalid at parse time, so the cascade should fall back to an
  earlier declaration rather than hand `taffy` a template it discards. A
  separate fault, found while reading for this one, queued as **425** and
  waiting for a page that writes it.
- `fit-content()` tracks, named lines and `subgrid`, which `track.rs`
  refuses already.

## What the build found (iteration 273)

The rule in § 1 was built as written, and three of the expectations around
it were not quite what it does. None changes the decision; each is said
here so that the next reader does not take § 4 and § 6 at their word.

- **A grid sized by its content is lowered in its final call.** `taffy`
  sizes such a grid in two calls: it measures it with no size — the
  refusal, the template discarded — and then lays it out in the width that
  measurement gave, handed to it as `known_dimensions`, which is definite
  by § 1. So § 6's "E, F and G keep today's layout" holds for E and F,
  whose measured width is no wider than one track (E's 50 is Chrome's
  too, by coincidence; F's 0 is not Chrome's 198), and not for G: its
  minimum makes it 700, and counted against 700 it is two columns of 338,
  which is Chrome's answer. This is `taffy`'s own measure-then-lay-out, not
  a pass of ours, and it is what `taffy` does with a plain-length track in
  the same grid.
- **A refusal is recorded from whichever call meets it**, as § 4 says,
  once per grid *and axis*. That includes a grid measured for its content
  by its parent and then laid out with a definite size — a grid of cards
  inside a grid's track is measured that way — because what it measured
  as was the template discarded. The issue says the grid was *measured*
  without its template, which is true in every case, rather than that the
  template was not used.
- **The refusal cases were tested out of flow, not as `inline-grid`.** An
  `inline-grid` holding blocks is split around them by the box tree today,
  as an inline box would be, so it is no grid at all; that is its own
  fault, queued as 426. A grid `position: absolute` is sized to fit its
  content the same way.

The agreement tests compare each calculated grid with the plain grid of
the length it comes to — 288 for A, B and D, 200 for C — rather than with
`minmax(288px, 1fr)` throughout, which is not C's equal.

## How we will know if this was wrong

A frozen page whose grid has a definite size, a calculated track in an
auto repetition, and columns that differ from Chrome's. The agreement tests
say whether our rule or `taffy`'s moved; the page says which answer is
right.
