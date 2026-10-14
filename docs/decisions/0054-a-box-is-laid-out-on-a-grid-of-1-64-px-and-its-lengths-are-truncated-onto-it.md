# ADR 0054 — A box is laid out on a grid of 1/64 px, and its lengths are truncated onto it

**Status:** accepted
**Date:** 2026-10-12
**Context:** queue item 466, *a box's paddings, margins and grid tracks
are on Chrome's 1/64 px grid*, found while building item 460. Its entry
says the rule must be measured and written down before it is built,
because it moves most boxes. It asked two things: which lengths Chrome
rounds and which it floors, and how a track or a flexible size shares
what is left over. The rule is decided here as queue item 498, as ADR
0050 was decided as 464 before 460 was built.

Also read: ADR 0001 (`taffy` is a judgement call, replaceable); ADR 0004
(the tree is ours and `taffy` owns the algorithms; a fork is refused);
ADR 0047 (a `[patch]` of `taffy` is refused for the same reason); ADR
0050 (a line is built from whole pixels of font, and a set line height
is on this grid already); `alo-layout`'s `style.rs` (what layout reads
from a computed style), `line_width.rs` (a border width in whole device
pixels), `line_height.rs` (the one place this engine already uses the
grid) and `arena.rs` (the tree `taffy` walks, `resolve_calc_value`, and
why it never rounds); `taffy` 0.14's grid re-run of column sizing
(`compute/grid/mod.rs`); and the two corpus cases item 466 closes on,
`alo-agenda-mini-month` and `alo-sites-booking`.

**Measured 2026-10-12, on this machine, in Google Chrome 154**,
headless. Three probe pages were written for this decision and are kept
nowhere. Each probe puts one small arrangement of `div`s in an absolutely
placed stage at (0, 0), reads `getBoundingClientRect()` of the boxes it
names, relative to the stage, and multiplies by 64. Every number below is
in those units, sixty-fourths of a CSS pixel, unless it says px. Every
reading in all three probes was a whole number of units except the one
under a `transform`. The probes covered `padding`, `margin` and its
negatives, `width`, `height`, `min-width`, `max-width`, `left` and
`top` on relative and absolute boxes and their negatives, `right` and
`bottom`, `border-box` sizing, `column-gap` in flex and grid, fixed,
percentage and flexible grid tracks, `flex` grow and shrink, every
`justify-content` distribution in flex and in grid, `align-items:
center`, auto margins, `aspect-ratio`, percentages of percentages,
`calc()`, `vw`, `em` and `rem`, text in a shrink-to-fit box, and lengths
past the grid's range. The values were 0.1, 0.3, 0.5, 0.6, 7.2, 9.6,
13.3, 19.92, 33.3333 and lengths exactly ¼, ½ and ¾ of a unit off the
grid. Flexible sizes were measured with 3, 6, 7, 9 and 192 items in
containers 0.5, 1, 10, 13, 100, 100.3, 200, 201 and 207 px wide. Then
both corpus cases were laid out in Chrome over DevTools at their own
sizes, with the corpus's `DejaVu Sans`, and every element's box read.
Chromium's source was not read. No other engine is installed here, so
nothing below claims what Firefox or Safari do, or what Chrome does on
another operating system.

## The decision in one line

Layout works on a grid of **1/64 px**. A length, a percentage and a
`calc()` are each **truncated toward zero** onto it when layout uses
them, a run of text's width is **rounded up** onto it, and what layout
shares out (centring, auto margins, `justify-content`, flexible tracks
and flexible items) puts every edge **on** it. This engine builds that at
its own boundary with `taffy`. What `taffy` computes inside its own
algorithms, and the boundary therefore cannot reach, is named and left
to queue item 499.

## Why this is a decision rather than a chore

**It moves most boxes on most pages.** Any box whose padding, margin,
width, inset, gap or track is not already a whole number of sixty-fourths
moves by up to one unit, and a box inside several of them moves by the
sum. The difference from Chrome is small per box: the booking page's
button, with `0.6rem` of padding above and below, is 48.4 tall here and
48.375 in Chrome. It is everywhere, and while it
lasts the corpus's comparisons with Chrome have to allow for it.

**Nothing specifies it.** CSS lays out in real numbers. The grid is
Chrome's fixed-point layout unit, and truncation is how Chrome converts a
real number into that unit. Following Chrome breaks the principle this
engine has kept since ADR 0004, that layout is sub-pixel throughout and
paint decides pixels. ADR 0050 already broke it for lines, for the same
reason.

**It meets a rented crate.** Some of the sharing happens inside
`taffy`'s flexbox and grid algorithms, which ADR 0004 says stay rented
and which ADRs 0004 and 0047 refused to fork or patch. So this ADR has to
say which half of the rule the boundary can carry and which half it
cannot, rather than decide the whole rule and leave the next iteration to
find that half of it cannot be built.

## 1. A length is truncated toward zero

Every length layout uses is placed on the grid by truncating its number
of units toward zero: `trunc(px × 64) / 64`. It is not rounded and not
floored.

| written | units | Chrome |
|---|---|---|
| `0.3px` | 19.2 | 19 |
| `0.6px` | 38.4 | 38 |
| `9.6px`, and `0.6rem` at 16 px | 614.4 | 614 |
| `19.92px`, and `0.83em` at 24 px | 1,274.88 | 1,274 |
| `10.015px` | 640.96 | 640 |
| `10.0078125px`, half a unit over 10 | 640.5 | 640 |
| `0.01171875px`, ¾ of a unit | 0.75 | 0 |
| `margin-left: -0.3px` | −19.2 | −19 |
| `margin-top: -19.92px` | −1,274.88 | −1,274 |
| `left: -0.1px` on a relative box | −6.4 | −6 |

The same holds for `padding`, `margin`, `width`, `height`, `min-width`,
`max-width`, `top`, `right`, `bottom`, `left` (relative and absolute),
`column-gap` and `row-gap` in flex and grid, a fixed grid track, and a
flex basis. `1.3vw` is a length like any other: 628.99 units became 628.

**Each length is truncated on its own.** Three nested boxes each with
`padding-left: 0.01px` put the innermost 0 units in, where one padding of
0.03 px would put it 1 unit in. A box whose height is the sum of three
children each `10.3px` tall is 1,977 units, three times 659.

`em`, `rem` and the font size behind them are not touched. The length
they compute to is truncated where layout uses it, which is why `0.6rem`
above is 614.

**Borders are already on the grid.** A border width is floored to whole
device pixels where it is computed (`line_width.rs`), and a whole pixel
is 64 units. **A set line height keeps ADR 0050's rule**, which rounds a
length rather than truncating it. Both were measured in Chrome and they
disagree, so they stay two rules.

## 2. A percentage is truncated after it is resolved, against a basis already on the grid

A percentage is resolved against its basis and the product is truncated.
The basis is itself a size on the grid, so a percentage of a container
written `100.3px` is a percentage of 6,419 units, not of 6,419.2:

| written | basis | units | Chrome |
|---|---|---|---|
| `width: 33.3333%` | 100 px | 2,133.33 | 2,133 |
| `width: 33.3333%` | 201 px | 4,287.99 | 4,287 |
| `width: 12.345%` | 100 px | 790.08 | 790 |
| `width: 50%` | 100.3 px, 6,419 units | 3,209.5 | 3,209 |
| `margin-left: -33.3333%` | 100 px | −2,133.33 | −2,133 |
| `33.3333%` of `33.3333%` | 100 px | 2,133 × 0.333333 = 710.99 | 710 |

The same holds for `padding`, `margin`, `left` and `top` on an absolute
box, `height` against a definite height, and a percentage grid track.

**A `calc()` is resolved whole and truncated once.**
`calc(0.01px + 0.01px + 0.01px)` is 1.92 units and becomes 1, where its
three terms truncated alone would be 0. `calc(33.3333% + 0.01px + 0.01px)`
of 100 px is 2,134.61 and becomes 2,134, where the percentage truncated
first would give 2,133.

## 3. What is left over is shared so that every edge is on the grid

Where layout divides free space, each share is truncated toward zero:

| arrangement | free space | Chrome |
|---|---|---|
| a 10 px box centred in 101 px | 5,824 units | 2,912 each side, exactly half |
| a 10 px box centred in 100.3 px | 5,779 | 2,889 in front, where half is 2,889.5 |
| a 10 px tall box centred in 31.3 px | 1,363 | 681 above |
| a 20 px box centred in 10.3 px, overflowing | −621 | −310 in front: toward zero |
| auto margins either side of 10 px in 101 px | 5,824 | 2,912 |
| grid `space-around`, three 10 px tracks in 100.3 px | 4,499 | 749 in front, then 1,499 between each |
| grid `space-evenly`, the same | 4,499 | 1,124 in front and between each |
| grid `space-between`, the same | 4,499 | 2,249 between each, and the last unit is left over |

**A grid's flexible tracks have edges at the ideal edges, floored.** In
Chrome the size of one `fr` is found as a 32-bit float, and each track in
turn takes that size plus whatever the track before it lost to the grid,
truncated. Each track's edge, measured from the first track's start, is
therefore the ideal edge floored. Seven `1fr` tracks in 201 px are 1,837,
1,838, 1,838, 1,837, 1,838, 1,838 and 1,837 wide, edges 0, 1,837, 3,675,
5,513, 7,350, 9,188, 11,026 and 12,863. The float makes the last edge
12,863 rather than 12,864, so a grid of flexible tracks may end a unit
short of its container. That model, with the float, matches all nine
grids measured, from 3 tracks in 0.5 px to 192 tracks in 200 px. `1fr
2fr 3fr` in 100 px is 1,066, 2,134 and 3,200. A gap is taken out first:
three `1fr` with a `0.3px` gap in 100 px are 2,120, 2,121 and 2,121, with
19-unit gaps.

**Flexible items fill their line exactly, and the leftover units go to
the later items.** A line of `flex: 1` items always ends exactly at its
container's edge. In all nine lines measured, the sizes are what you get
by going from the **last item to the first**, giving each the free space
still unshared divided by the flex factors still unshared, **rounded half
away from zero**. Seven items in 201 px are 1,837, 1,838, 1,837, 1,838,
1,838, 1,838 and 1,838. Six in 100 px are 1,066, 1,067, 1,066, 1,067,
1,067 and 1,067. Three items shrunk from 50 px to fit 100 px are 2,134,
2,133 and 2,133. Grow factors of 1, 2 and 3 in 100 px give 1,067, 2,133
and 3,200. A column is shared the same way as a row. This is a model that
fits the measurements, not Chrome's code. Two other things were measured
and not separated: a grow factor below one in total, where each of three
`0.3` items took 0.3 of 100 px, 1,920 units; and flex `justify-content`.
In 100.3 px with three 10 px items, `space-between` puts 2,250 then 2,249
between them, `space-evenly` 1,124, 1,125, 1,125 and 1,125, and
`space-around` 749 in front, then 1,500 and 1,499 between. All of these
fill the line exactly, where grid's distribution loses the odd unit, but
no single rule fitted all three.

**`aspect-ratio` truncates the size it derives.** A 10 px wide box with
`aspect-ratio: 3` is 213 units tall, where 10 ÷ 3 is 213.33.

## 4. Text is measured onto the grid by rounding up

A run of text's width is rounded **up** to the grid, so a box that
shrinks to fit its text always holds it. `W` at 13 px is 785.28 units by
the canvas's `measureText` and 786 in a box; `iii` at 10.5 px is 560.11
and 561; `October 2026` at 13 px is 4,551.63 and 4,552. A width already
on the grid stays: `Hello` at 16 px is 2,275 both ways. It is the same
inline-block, absolutely placed or as a flex item. The heights of lines
are ADR 0050's.

## 5. What stays off the grid

- **A transform.** `translateX(0.3px)` reads 19.2 units, because a
  transform is applied after layout. It is paint's.
- **Where a glyph is drawn**, and every advance inside a run of text. Only
  the run's width as a box is rounded up.
- **The computed value.** `getComputedStyle` still reads `19.92px`, and
  so does this engine's cascade.
- **A font size.**

**Past the grid's range, a length saturates.** Both `33554431.99px` and
`1000000000px` gave the same width, 2,147,483,392 units as
`getBoundingClientRect` reports it: the largest 32-bit float below 2³¹,
so the top of a 32-bit count of sixty-fourths, about 33,554,432 px. That is far beyond
where a 32-bit float can hold a sixty-fourth exactly: an `f32` holds
every multiple of 1/64 only up to 2²⁴ units, 262,144 px. This engine
lays out in `f32`. Its grid is therefore exact up to 262,144 px, and
beyond that it is as fine as the float allows. A length past Chrome's
range is clamped to it rather than left to become infinite.

## 6. Where this engine builds it: at the boundary, and not inside `taffy`

ADR 0004 put the line between our tree and `taffy`'s algorithms, and
ADRs 0004 and 0047 refused a fork or a `[patch]`. This rule is built on
our side of that line, in three places that are already ours:

1. **`LayoutStyle`** (`style.rs`) truncates every length as it reads it,
   §§ 1 and 5's clamp included. `line_width.rs` and `line_height.rs`
   already put border widths and line heights on the grid in the same
   file, so the grid gets a file of its own that those two and this read,
   rather than a third copy of the constant.
2. **The arena's `resolve_calc_value`** (`arena.rs`) truncates the
   result of every `calc()` it resolves, as § 2 says. **A plain
   percentage is handed to `taffy` as a calculation**, so that it reaches
   the same place: `taffy` resolves a `Percent` itself, as `basis ×
   percent`, and there is no hook in that. ADR 0004's index handle
   already carries any expression, and a percentage is the simplest one.
   A leaf's measured text width is rounded up where the arena measures
   it, as § 4 says.
3. **The edges `taffy` hands back.** When `taffy` sets a child's layout,
   the arena truncates both of that child's edges toward zero, relative to
   its parent, and makes its size the distance between them. That puts
   § 3's shares on the grid. For centring, auto margins and grid
   distribution it gives exactly Chrome's numbers. For a grid's flexible
   tracks it gives the ideal edges floored, which is § 3's model, to
   within `taffy`'s float arithmetic. For flexible items it gives the
   floored edges, which are within one unit of Chrome's, not equal to
   them: seven `flex: 1` in 201 px would have one edge at 5,513 where
   Chrome's is at 5,512.

**What the boundary cannot reach is what `taffy` decides with its own
numbers before handing them back.** Two cases matter:

- **Flexible items' exact shares** and flex `justify-content`'s, as
  above. They are within one unit.
- **A grid that sizes its columns twice.** CSS Grid runs column sizing
  again when an item's min-content contribution changes once the rows are
  known, and `taffy` does so (`compute/grid/mod.rs`, *column sizing must
  be re-run*). In `alo-agenda-mini-month` every day is a button with
  `aspect-ratio: 1` in seven `1fr` columns, 207 px wide with 1 px gaps,
  so 201 px are shared, exactly as in § 3. In Chrome the first pass gives
  the columns 1,837 and 1,838 units. Each day is as tall as it is wide,
  so the row is 1,838 tall. Through the ratio, every day's min-content
  width becomes 1,838 too, and on the second pass **every column is
  1,838 wide**: seven of them and six gaps make 13,250 units in a grid of
  13,248, so the grid overflows by two units. Inside `taffy`, both passes
  see 28.714285 px exactly, nothing changes, and there is no second pass
  to change. Probes confirm it is the ratio: the same grid of plain
  `div`s, or of buttons with text and no ratio, keeps 1,837 and 1,838.
  With `minmax(0, 1fr)` columns, which have no min-content floor, the
  ratio changes nothing. Six columns in 100 px with ratio boxes all
  become 1,067.

Truncating at the boundary cannot produce that. Truncating the sizes
`taffy` asks the arena to measure would make every day 1,837, the row
1,837, and still no second pass. What it needs is for `taffy`'s own track
sizes to be on the grid between its two passes. That is either a hook
`taffy` does not have, offered upstream, or this engine taking over the
sharing of flexible space from `taffy`. ADR 0004 refused both a fork and
replacing `taffy` for less than this, and this ADR does not overturn ADR
0004 in passing. So it is cut as **queue item 499, needs ADR**, as its
own iteration.

## Why we take it

**The measure is Chrome's picture,** as ADR 0050 said of lines. Every
corpus comparison with Chrome since item 460 still allows about 1/160 px
for paddings, margins and tracks. A test that has to allow a difference
it cannot explain is a test that cannot catch a new one of the same size.

**It is not legacy.** A fixed-point layout unit is how a modern engine
avoids accumulating float error across a deep tree. Law 1 refuses what
exists only to keep broken pages working, and this is not that. A page
written today is laid out on this grid by the browser its author checked
it in.

## What is not decided here

- **The exact rule for flexible items' shares and flex
  `justify-content`.** § 3's model for items fits every line measured,
  and it is still a model. Whether to build it exactly, which needs
  `taffy`'s own numbers, is item 499's.
- **A device pixel ratio other than 1.** The grid is 1/64 of a CSS pixel,
  measured at 1×.
- **Chrome on Linux, Windows or Android.** This ADR records macOS
  Chrome 154 only.
- **Writing modes other than horizontal, and right-to-left.** Truncation
  toward zero was measured only on the physical left and top.
- **Tables and multi-column**, which this engine does not lay out.

## Consequences

- **Queue item 466 builds § 6's three places**: lengths, percentages and
  `calc()` truncated, text widths rounded up, and the edges `taffy` hands
  back truncated. It moves most corpus cases' `layout.txt`, `boxes.txt`,
  `display.txt` and `render.png`. It closes when `alo-sites-booking`'s
  boxes are Chrome's to 1/64 px, except the date field's own line, which
  is item 467's, and `alo-agenda-mini-month`'s are too, except its column
  widths and what follows from them, which are item 499's.
- **Queue item 499 decides whether `taffy`'s flexible shares can be put
  on the grid**: a hook offered upstream, or this engine taking the
  sharing over. It needs an ADR, as its own iteration, because it would
  revise ADR 0004's line.
- **Item 466's entry corrects itself.** It said `alo-sites-rich-text`'s
  `h2` margin is 19.921875 in Chrome. Measured again over DevTools, the
  gap between the paragraph above and the `h2` is 19.90625, which is
  `0.83em` at 24 px truncated. It also said the mini-month's columns are
  28.71875 because of the grid's 1/64 unit. They are, but only because of
  the second column pass above. Without it the grid would give 1,837 and
  1,838.
- `docs/features.md` says the grid exists in the change that builds 466,
  not in this one. Nothing is built yet.

## How we would know this was wrong

**If a corpus box is a unit away from Chrome's after 466 is built**, at a
value or an arrangement not in the tables above, § 1's truncation or
§ 3's shares were read wrongly from too few points.

**If a shrink-to-fit box wraps its own text after 466 is built**, § 4's
rounding up is not happening where the box is measured.

## Addendum, 2026-10-12 — built by item 466, and three things the text above had wrong

Queue item 466 built § 6 in iteration 305 and measured the whole corpus
against Chrome 154 again with the corpus's fonts, before and after: 707 of
1,260 element boxes now have Chrome's left, top, width and height
exactly, where 514 did, and only the mini-month's columns, item 499's,
end up further from Chrome's. Building it corrected § 6 in three places.
The rule of § 1 to § 5 stands as measured.

**1. An edge is placed against where its parent was placed, so it is
done as the tree is read back, not as `taffy` hands a layout over.**
§ 6.3 said the arena truncates each edge relative to the parent when
`taffy` sets a child's layout. Built that way, a box filling its parent
did not fill it on the grid. `alo-sites-contact-form`'s form is a grid
item whose track `taffy` starts at 22,579.2 units and ends at 49,920, so
it is placed at 22,579 and is 27,341 wide, as Chrome's is. But its label
was laid out by `taffy` against the unplaced 27,340.8, and floored on its
own it came to 27,340, a unit narrower than the form that holds it, where
Chrome's label is 27,341. And when `taffy` sets a child's layout, where
the parent will be placed is not known yet, because a parent is set after
its children. So `alo-layout`'s `engine.rs` places each box as it reads
the tree back. A box's near edge is measured from where its parent was
**placed**. That offset is truncated toward zero, and its size is its far
edge floored less its near edge floored. For a box on the near side of
its parent, this is § 6's rule exactly. For one beyond its near edge, the
floor keeps a size already on the grid: the 20 px box centred in 10.3 px
is at −310 and still 1,280 wide, as § 3's table says Chrome makes it. To
truncate its far edge as well would make it 1,279.

**2. A length a float holds just short of a grid line is on it. A shared
edge is not.** This engine resolves a length in `f32`. Chrome resolves it
in double before it becomes a float, so `0.53em` at 100 px is 52.999996
here and 53 there, and truncated those are 3,391 units and 3,392. A
length or a text width within four of a float's own steps of a grid line,
2⁻²¹ of the number, is taken to be on it. It is relative because a
float's error is. The nearest to a line that Chrome was measured to keep
off it is a percentage of a percentage, 710.9993 units, which it
truncates to 710; four steps of a float there are 0.0003 of a unit. An
edge `taffy` shares out gets **no** slack. Chrome shares free space in
float, as `taffy` does, and three `1fr` tracks in 184 px with 6 px gaps
end at 12,287.999 units in both. Chrome floors that to 12,287, a unit
short of the container (§ 3 records the same thing for seven tracks).
With the slack the last track was 3,670 wide; without it, it is Chrome's
3,669. ADR 0050's line-height rules are unchanged and take no slack.

**3. A grid's `justify-content` is Chrome's only for its first share.**
§ 6 said that truncating at the boundary gives exactly Chrome's numbers
for centring, auto margins and grid distribution. It does for the first
two. For distribution, Chrome truncates **each** share and adds them up.
`space-around` with three 10 px tracks in 100.3 px puts 749 units in
front and 1,499 between, so its tracks start at 749, 2,888 and 5,027.
The boundary sees only where `taffy` put each track, which is its exact
shares added up: 749.8, 2,889.5 and 5,029.2. Truncated, those are 749,
2,889 and 5,029. So each later track is within a unit for each share
before it. That is the same thing § 6 says of flexible items, `taffy`'s
numbers before it hands them back, and it moves to item 499.

**What 466 left off the grid**, each said where it is read: a
`fit-content()` limit written as a percentage, which `taffy` resolves
itself and has no calculation spelling for; a `vertical-align` length; and
where an atomic box stands inside a line, which is `crate::inline`'s
placement and not a box `taffy` hands back. No corpus case has failed on
any of the three.

## Addendum, 2026-10-13 — flexible items' shares, measured at one and at two, shrinking too

Queue item 564 found `border-widths`' five items a unit off Chrome's at a
scale of two, where they shrink, and asked how Chrome shares a shrinking
line. § 3's model was fitted on nine lines, all but one growing and none at
a scale. It was measured again, on **5,120 lines** in Chrome 154, headless,
under `--force-device-scale-factor=1` and `=2` (never emulated), the source
not read. Each line is a flex container of 2 to 9 empty items, each with a
written width, a padding on each side and a factor, at widths and gaps
written to up to three decimals, shrinking or growing; at two the grid is
1/128 px (ADR 0057). The rule below was fitted on 3,600 lines (1,800
written lines, each at both scales). It was then confirmed, with no
exception, on 1,520 lines it was not fitted to: 500 written lines seeded
at random and 260 written to land on exact halves or in `border-widths`'
shape, each at both scales. Those 760, and the two fitted ones that tell
the ratio in 4 from a product, are frozen as `alo-layout/tests/references/flex-lines.chrome.txt`, and
`alo-layout/tests/flexible_items_share_a_line.rs` checks the rule against
every one of them.

**The rule, at any scale, in units of the grid of that scale:**

1. Every length is truncated onto the grid first (§ 1), and the free space
   is the container's inner size less the gaps and each item's outer flex
   base size, in whole units.
2. A shrinking line's factors are each item's `flex-shrink` times its
   **inner** base size in units (its content box, not its padding); a
   growing line's are its `flex-grow`.
3. If the factors as written sum below one, the space shared is the
   first free space times that sum, **truncated toward zero**, when that
   is smaller than what is left.
4. From the **last item to the first**, each takes the space still unshared
   times *its factor over the factors still unshared*, that ratio taken
   first, in double, **rounded half away from zero**. A half is decided
   this way: two equal items 1/64 px too wide at one take 0 and −1, the
   last taking the −1. Rounding half up (toward positive) misses 164 of
   the 5,120 lines, and taking the product before dividing misses 2.
5. An item that comes under its minimum is fixed at it and the rest are
   shared again, each pass by the same rule.

So § 3's model holds as written, for shrinking as for growing, with the
scaled factor, the truncated part and the clamping loop added. At two it is
the same rule on the finer grid: `border-widths`' five items shrink 256
units of 1/128 px between them, 51.2 each, and from the last they take 51,
51, 51, then 103 over two, a half, 52, then 51 — Chrome's 4,173, 3,148,
3,405, 3,533 and 4,173.

**§ 6 said flexible items were within one unit of Chrome's. They are within
two.** Today the shares are `taffy`'s floats and each edge is floored as the
tree is read back, which is within a unit of the exact edge. Chrome's
last-to-first rounding can itself put an edge a unit and a quarter from the
exact one — eight 10 px items 3/64 px too wide, at two, have an edge at
3,839 where the exact edge is 3,837.75 — so this engine's edge can be two
units from Chrome's. On the 762 frozen lines it is never more than two,
every edge is on the grid, and 224 lines at one and 232 at two are exact;
the test pins both counts. Building the rule is still ADR 0056's hook
(queue item 502), whose hand-over at the end of `resolve_flexible_lengths`
needs the scaled factor and the free space as truncated in 3, both of which
`taffy` holds there.

**One thing is not rounding, and is `taffy`'s.** With factors summing below
one and a gap between the items, `taffy` 0.14.0 (and its `main` as read on
2026-10-13) takes the gaps away a second time from the part in 3, so the
part loses to the whole free space and the items flex by all of it. Two of
the frozen lines show it: factors 0.5 and 0.3 with 4 px gaps, where
Chrome's first item ends 6.55 px further on than this engine's — a whole
pixel wrong, and more — and factors 0.3 with a 0.5 px gap, 0.23 px. That is
a fault in `taffy`, reported nowhere yet, and queue item **572**.

## Addendum, 2026-10-14 — § 4 rounds a text item, not a word or a line

Queue item 574 found a padded `<span>` holding one `&nbsp;` 44.7265625 px
wide here where Chrome 154's is 44.734375: § 4's rounding up had been built
where a leaf or a whole formatting context is measured, so a box that
shrinks to its text held it, but nothing inside a line was on the grid. A
`<span>`'s width, where the box after it starts, and a piece of text's own
rectangle were exact floats.

Measured in Chrome 154, headless, at `--force-device-scale-factor=1` and
`=2`, with the corpus's `DejaVu Sans` at 11.1, 13, 13.7, 16.3 and 20.45 px,
the source not read: 300 lines of one to five words joined by spaces and
no-break spaces, some split by an empty `<b>`, each with an empty `<i>`
after it; 120 padded spans; 120 blocks narrow enough to wrap; and 620
lines' `Range` rectangles. Each was predicted from the canvas's
`measureText` of the same strings.

- **The unit rounded is the item**: one text box's text on one line,
  between whatever else the line holds. Its advance is its exact width
  rounded up onto the grid, and what follows starts there. At one every
  one of the 300 lines puts its `<i>` at the sum of its items rounded so;
  rounding each word on its own gives 181 of them, and the whole line at
  once 258. A wrapped line's share of an item is rounded on its own: 120
  of 120 at one.
- **What an item draws is its letters' width rounded up**, without the
  white space a line ends in: 620 of 620 `Range` rectangles at both scales
  (that probe left 16.3 px out).
- A padded `<span>` is its paddings truncated (§ 1) and its item rounded:
  120 of 120 at one.

At two the same rule on the grid of 1/128 px predicts every line and span
set at 11.1, 13, 13.7 and 20.45 px, and misses 84 of the 87 at 16.3 px:
there Chrome's canvas measures the text up to 1.2 units narrower than its
layout sets it. This engine sets 16.3 px text where Chrome's layout does
(below), so it is the canvas that differs at that size and the rule
holds.

Built by item 574 in `alo-layout`'s `text_run.rs`, which the line builder
lays each piece of text through. On the same probe page this engine now
places all 300 lines and 120 spans where Chrome does, at one and at two,
where it had placed 5 and none; 88 of the wrapped blocks, where it had
placed 1 (the other 32 break their lines elsewhere than Chrome, as they
did before, queue item 586). Across the corpus, 950 of 1,267 element boxes
are Chrome's exactly at one, where 899 were, and 949 at two, where 926
were; no box that was within two units of Chrome's moved further from it.
