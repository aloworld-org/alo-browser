# ADR 0056 — `taffy`'s own shares are put on the grid by a hook offered upstream, and this engine does not take the sharing over

**Status:** accepted
**Date:** 2026-10-12
**Context:** queue item 499, *`taffy`'s flexible shares on the grid*,
found deciding item 498 (iteration 303) and widened building item 466
(iteration 305). It is marked **needs ADR**, as its own iteration,
because either way of building it would revise the line ADR 0004 drew
between our tree and `taffy`'s algorithms.

Also read: ADR 0001 (rent the physics, build the engine; `taffy` is a
judgement call, replaceable); ADR 0004 (the tree is ours, the algorithms
are `taffy`'s, and a fork is refused); ADR 0047 (a `[patch]` is a fork
under another name, and offering a fix upstream is a person's action);
ADR 0054 § 3, § 6 and its addendum (Chrome 154's shares, measured, and
what the boundary cannot reach); `alo-layout`'s `arena.rs`, `engine.rs`
and `layout_unit.rs`; `alo-layout/tests/on_the_grid.rs` and
`alo-corpus/tests/alo_agenda_mini_month.rs`, which pin today's numbers;
and `taffy` 0.14.0's `compute/grid/mod.rs`, `compute/grid/track_sizing.rs`
(`expand_flexible_tracks`), `compute/grid/alignment.rs` (`align_tracks`),
`compute/flexbox.rs` (`resolve_flexible_lengths`,
`distribute_remaining_free_space`) and `tree/traits.rs`, read on this
machine. `taffy`'s `main` was read the same day for `compute/grid/mod.rs`
and `tree/traits.rs`, and 0.14.0, released 2026-08-24, is still its latest release.

## Evidence

**What is owed**, from ADR 0054. Three kinds of number are decided inside
`taffy`, before the boundary sees them, and are within a unit of
Chrome's per share:

1. **A grid's flexible tracks, and a grid sized twice.** In
   `alo-agenda-mini-month`, seven `1fr` columns share 201 px. Chrome's
   first pass gives them 1,837 and 1,838 units. The `aspect-ratio: 1`
   days make the row 1,838 tall, and that row makes every day's
   min-content width 1,838, so Chrome's second column pass makes every
   column 1,838. Inside `taffy` every column is 28.714285 px on both
   passes, so the second pass changes nothing.
2. **Flexible items' shares.** Seven `flex: 1` in 201 px have an edge at
   5,513 here, where Chrome's is at 5,512.
3. **Content distribution's later shares.** Grid `space-around` of three
   10 px tracks in 100.3 px starts them at 749, 2,889 and 5,029 here,
   where Chrome's are 749, 2,888 and 5,027.

**A prototype, built to test this decision and thrown away.** A copy of
`taffy` 0.14.0 was put in the session's scratch directory. A scratch
worktree of this repository was pointed at it with a `[patch]`. Neither
was committed, and both were deleted before this ADR was written. Two
changes were tried, alone and together, against `alo-layout`'s and
`alo-corpus`'s tests:

- **A snap at the end of `expand_flexible_tracks`.** Each flexible track
  in order takes its size in sixty-fourths plus what the track before it
  lost, floored, and carries the remainder on. That is ADR 0054 § 3's
  model of Chrome.
- **The re-run check made whole.** `compute_grid_layout` decides whether
  to size columns again with `.any(…)` over the items. `.any` stops at the
  first item whose min-content contribution changed, so every later item
  keeps the contribution cached from the first pass.

What they gave:

| `taffy` | mini-month's columns | `grid-of-three` | everything else in `alo-layout` and `alo-corpus` |
|---|---|---|---|
| 0.14.0 as released | 1,837 and 1,838 (today) | 11,775 (Chrome's) | unchanged |
| re-run check whole, no snap | 1,837 and 1,838 | 11,775 | unchanged |
| snap only | 1,838, 1,837, 1,838, 1,837, 1,838, 1,838, 1,838 | — | — |
| both | **every one 1,838**, the last ending 2 units past the grid (Chrome's) | 11,775 | one box moved, below |

So **both** are needed, and with both the mini-month's days are Chrome's
to the unit. The snap makes the row 1,838 tall, so the ratio changes the
contributions. The whole re-run check then reads every item's new
contribution, and not only the first.

Two smaller findings came out of the prototype:

- **The snap has to be Chrome's carry, not a floored running sum.** A
  first try floored each track's far edge from a running total in `f32`.
  That put `grid-of-three`'s spanning item at 184 px, a unit wider than
  Chrome's 183.984375. Chrome's float error is its own: 3,669.33 units
  three times and two gaps reach 11,775.9998 by carrying, but a running
  sum rounds to 11,776. The carry is what ADR 0054 § 3 describes, and with
  it `grid-of-three` stays Chrome's.
- **One other corpus box moved.** In `alo-sites-full-page`, Jonas Meer's
  `h3` went from 423.53125 to 423.515625 wide. Nobody measured which
  number is Chrome's. That is owed by the build (§ 5).

**Upstream already fixed the re-run check.** `taffy`'s pull request
#1227, *Grid: fix stale item caches and wrong axis in the track sizing
re-run checks*, was merged on 2026-10-04, after 0.14.0. It replaces the
`.any` with a `fold` that refreshes every item. It is not released yet.
`main` has no hook that puts a share on a caller's unit.
`LayoutPartialTree` offers `resolve_calc_value`, which reaches a
length, but no method reaches a share.

## The decision in one line

This engine **does not take** the sharing of flexible space over from
`taffy`. Our side of the fix is a hook on `taffy`'s tree trait, with a
default that changes nothing, called where `taffy` fixes a share. The
hook is specified here, **a person offers it upstream**, and this
engine builds on it from the first `taffy` release that has both the
hook and #1227. Until then the difference stays as it is, pinned in
numbers.

## 1. Why the engine does not take the sharing over

To put a flexible track's size on the grid between passes, the engine
would have to own the code that computes it. In grid that code is the
track sizing algorithm: `expand_flexible_tracks` is one step of it, and
the second pass is the whole algorithm run again. In flexbox it is
`resolve_flexible_lengths`, which is the middle of the flexbox
algorithm. There is no smaller piece to own. Taking it over means writing
grid's track sizing again (`track_sizing.rs`, 1,513 lines in 0.14.0)
and the middle of the flexbox algorithm (`resolve_flexible_lengths`,
about 190 lines, which the 3,586-line file around it reads from and
writes to). Both carry the specification's own ordering.

ADR 0001 rents the physics, and these lines are the physics. ADR 0004
refused to replace `taffy` "to gain `calc()`". The gain here is smaller:
1/64 px per share, and in the worst case on file, 2 units across seven
columns. The prototype also shows the fix is small *inside* `taffy`: one
loop at the end of one function, plus a check that upstream has already
fixed. Rewriting two algorithms to make a one-loop change from outside
them is the trade ADR 0001 exists to refuse.

**Law 3 does not make this urgent.** "A wrong pixel is a bug." Here no
whole pixel is wrong. Every edge is already on the grid (ADR 0054, item
466), within a unit of Chrome's per share. A test holds it, so it cannot
grow without a test failing. This is a known, measured difference with
its cause named, not one nobody can explain.

## 2. Why not a fork, a `[patch]`, or a copy of the crate

ADRs 0004 and 0047 refused all three, and nothing here changes their
reasons. A fork carries every upstream change as a merge for the sake of
one loop. The prototype used a `[patch]` because it was evidence, not a
build, and it was deleted. **Committing it would be the fork ADR 0047
refused.**

## 3. The hook, as it is to be offered

One method on `LayoutPartialTree`, with a default body that leaves
every share as `taffy` computed it. Any other embedder of `taffy`, and
`taffy`'s own tests, then see no change. `taffy` calls it at three
places, each after a share is computed and before anything reads it:

| where, in 0.14.0 | what it hands over | what this engine does with it (ADR 0054 § 3) |
|---|---|---|
| end of `expand_flexible_tracks`, in each axis and on every pass | the flexible tracks' sizes, in order | each takes its size plus the carried remainder, floored, and carries the rest on, in sixty-fourths |
| end of `resolve_flexible_lengths` | each item's target main size and the flex factor it was sized by | last item to first, each takes the unshared space over the unshared factors, rounded half away from zero |
| each offset from `compute_alignment_offset`, in grid's `align_tracks` and in flex's `distribute_remaining_free_space` | the share, and whether it is the first | each share truncated toward zero, before it is added to the next |

The method's name and the type it hands over are for the person who opens
the pull request and for `taffy`'s maintainers to settle. This ADR fixes
only what is listed above. There are three places, a default that changes
nothing, and enough handed over at each to apply ADR 0054 § 3. The
method gets the numbers and puts them back. It never gets a node,
so the hook cannot change the tree.

It is a better change upstream than a grid-only one. Any embedder with a
fixed-point unit has the same problem, and the same hook serves a
whole-pixel UI as well as 1/64. For that reason it is offered as one hook
for all three, not as Chrome's rules written into `taffy`.

## 4. Until the hook is released

- **Nothing in this engine changes**, and nothing is approximated at the
  boundary (§ *Alternatives*). The numbers are those iteration 305 left:
  the mini-month's columns 1,837 and 1,838, flexible items within a unit,
  and grid distribution Chrome's for its first share.
- **The tests that pin them stay**, and stay honest about it:
  `on_the_grid.rs`'s distribution test asserts Chrome's first share, and
  that every later share is within a unit and on the grid; the mini-month
  test asserts the floored edges and names this ADR. A build that changes
  either has to change the test.
- **Upgrading `taffy` to a release with #1227 and no hook is its own
  ordinary change.** The prototype shows #1227 alone moves nothing in the
  corpus. That upgrade is not gated on this ADR.

## 5. What the build does, when the hook is released

Queue item **502**:

- the hook in `arena.rs`, with ADR 0054 § 3's rules in `layout_unit.rs`,
  the one file that holds the grid;
- `alo-agenda-mini-month`'s columns every one 1,838 units, its test and
  references rewritten;
- `on_the_grid.rs`'s distribution test asserting all of Chrome's
  starts, 749, 2,888 and 5,027, and a line of seven `flex: 1` items in
  201 px at Chrome's 1,837, 1,838, 1,837, 1,838, 1,838, 1,838 and 1,838;
- every corpus box that moves measured against Chrome 154, including
  `alo-sites-full-page`'s `h3` from § *Evidence*;
- the read-back placement from ADR 0054's addendum kept. An edge already
  on the grid is left where it is, so it does no harm, and it is still
  what places a parent's children against where the parent was placed.

**Flex `justify-content` is not in it.** ADR 0054 § 3 measured three
distributions that fill the line exactly, and no single rule fitted all
of them. The hook will reach those shares, but what to do with them is
still unknown. That is **503**, which needs measuring before it needs
code.

## What this costs

- **The mini-month stays two units wide of Chrome's, and other pages up
  to a unit a share,** for as long as upstream takes. Nobody can promise
  how long that is.
- **The fix is in somebody else's hands.** If `taffy` refuses the hook,
  the choice in § 1 has to be made again with that refusal on the table.
  That would be another ADR, written by or for a person (§ *How we will
  know*).
- **A person has to act.** Opening a pull request is publishing, and the
  loop does not publish (`LOOP.md`). Item 501 is blocked on that, and
  says so.

## Alternatives rejected

**Take the sharing over** (§ 1). It means owning grid track sizing and
flexbox line resolution for 1/64 px.

**Fork `taffy`, `[patch]` it, or vendor a copy** (§ 2).

**Re-share a line's edges as the tree is read back.** ADR 0054's
addendum already places edges at read-back. That spot could also apply
the last-to-first rule to a flex line's items. But read-back sees edges,
not lines, factors or free space, and would have to work all three out
again. It also cannot reach the grid's second pass, which is a layout
`taffy` never does, and that is the larger difference. A rule that fixes
the small half from numbers it has to guess at is ADR 0013 § 3's
*approximate*, which loses to *absent*.

**Snap what the arena measures.** If the arena rounded a ratio box's
measured height, the row might be 1,838. But it is asked about every box
the same way, without knowing which column it is in. That would give the
mini-month Chrome's number by accident and every other ratio box a
wrong one. ADR 0054 § 6 already ruled it out: truncating measured sizes
makes every day 1,837.

**Lay out twice from outside.** ADR 0004 refused a second pass from
outside as "right almost always and quietly wrong sometimes". Here it
would also have to repeat `taffy`'s own re-run condition.

## How we will know if this was wrong

- **If a frozen page shows a whole pixel wrong because of a share**,
  the units have added up to something a person can see. Then the cost
  in § 1 was weighed against the wrong gain, and taking the sharing over
  is worth a new ADR.
- **If `taffy` refuses the hook**, or a year passes with no release that
  has it, the decision in § 1 is open again.
- **If the build finds that the hook's three places are not enough**,
  because some share is fixed somewhere this ADR did not name, § 3 was
  read from too little of `taffy`. The prototype covered one of the three
  places.

## Measured since, 2026-10-13 (queue item 564)

ADR 0054's addendum of this date measured flexible items' shares on 5,120
lines at scales of one and two, shrinking as well as growing, and wrote the
rule out. It changes two things here. The hand-over at the end of
`resolve_flexible_lengths` (§ 3's second row) needs, besides each item's
target size and factor, the **scaled** shrink factor and whether the line
shared only the part of its free space that factors below one take,
truncated; `taffy` holds both there. And § 4's "flexible items within a
unit" is two units, measured: Chrome's own rounding can stand a unit and a
quarter from the exact edge. The decision stands. A fault that is not a
share was found beside it — `taffy` takes the gaps away twice when the
factors sum below one, a whole pixel and more on a measured line — and is
queue item 572, not this hook's.
