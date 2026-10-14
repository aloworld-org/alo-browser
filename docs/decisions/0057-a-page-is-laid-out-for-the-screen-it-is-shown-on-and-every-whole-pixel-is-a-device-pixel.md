# ADR 0057 — A page is laid out for the screen it is shown on, and every whole pixel is a device pixel

**Status:** accepted
**Date:** 2026-10-13
**Context:** queue item 299, *device pixels*, cut from item 118 by ADR
0024 § 3. Its entry said the renderer *"lays out in CSS pixels and paints
at the window's scale factor"*, and that it closes on reference renders at
scale 1 and 2 *"with layout identical in CSS pixels"*, a border width being
*"the one layout number that is not identical"*. Iteration 330 measured
that before building on it, and it is not so. Most of the layout rules
this engine has already taken from Chrome round to whole pixels or to a
grid of pixels, and at a scale of two Chrome rounds every one of them in
device pixels. So which pixel a rule means is a decision, taken here
before any code depends on it, as LOOP.md asks of a decision found inside
an item.

Also read: ADR 0001 (law 3: a wrong pixel is a bug); ADR 0002 (the layout
tree is the agent's tree); ADR 0024 §§ 2–3 (the window composes frames and
replicates pixels until this item); ADR 0050 (a line is built from whole
pixels of font, and its leading is floored); ADR 0054 (a box is laid out
on a grid of 1/64 px); `alo-layout`'s `line_width.rs` (a border width in
whole device pixels, one to the CSS pixel *"until item 299"*),
`layout_unit.rs` and `line_height.rs`; `alo-paint`'s `snap.rs` (a
control's box and a plain background on whole pixels, *"when the canvas
learns a scale, this is where it rounds against it"*) and `render.rs`;
`alo-value`'s `Viewport` and `alo-css`'s `MediaContext`, the two places a
window's size reaches style; `alo-renderer`'s `pipeline.rs` and
`easel.rs`; and `alo-window`'s `place.rs`, `compose.rs` and `aim.rs`.

## Evidence

**Measured 2026-10-13, on this machine, in Google Chrome 154**, headless.
Chromium's source was not read. No other engine is installed here, so
nothing below claims what Firefox or Safari do.

**How the scale was set matters, and is the first finding.** DevTools'
`Emulation.setDeviceMetricsOverride` with `deviceScaleFactor: 2` changed
`devicePixelRatio` and almost nothing else: 72 of the 73 corpus cases
then laid out exactly as at 1, and the one that did not was a time field
in `web-a-form` growing from 78 to 82 px wide, which under a real scale
of two is 77.5. Starting Chrome with
`--force-device-scale-factor=2` (and `deviceScaleFactor: 0`, so DevTools
overrides only the size) is a real scale of two, and laid out differently.
**Every number below is from the real flag**, and a measurement of this
engine at a scale is never taken under emulation.

1. **The corpus.** All 73 cases of iteration 310's whole-corpus probe
   (each case at its `size.txt`, every element's `getBoundingClientRect()`
   in sixty-fourths, each element set in the corpus's DejaVu face for its
   computed family) were laid out at scale 1 and 2. At 1 the numbers are
   the ones the corpus has always been compared with. At 2, **48 of 73
   cases and 744 of 1,761 boxes differ**: 217 by at most 1/64 px, 326 by
   at most half a pixel, 122 by at most a pixel, and 79 by more, up to
   5.5 px at the foot of `text-decorations`, where each line's difference
   adds to the next.
2. **The grid is 1/64 of a device pixel.** At scale 2 every layout number
   was a whole number of 1/128 px (7,044 values; the only exceptions were
   inside an `<svg>` or under a `transform`, as at scale 1). Twelve
   lengths from 0.1 to 100.3 px, as a width and as a padding, were each
   truncated onto 1/(64·s) px at both scales (24 of 24): `0.31px` is
   38/128 at 1 and 39/128 at 2. A run of text is 2,794/64 at 1 and
   2,793.5/64 at 2: rounded up onto the finer grid.
3. **A line's font figures are rounded in device pixels.** `DejaVu Sans`
   (`hhea` 1901 / −483 / 0 of 2048) at 65 sizes from 8 to 40 px by halves:
   a `normal` line was its ascent and descent each rounded half up to
   whole device pixels, `(round(a·s) + round(d·s)) / s`, at every size and
   both scales (65 and 65). `text-wrapping`'s three lines are 64 px tall at
   1 and 65.5 at 2.
4. **The half-leading is floored in device pixels.** At 16 px, with
   `line-height` from 17 to 30.2 px by 0.3, the baseline stood at the
   floored half-leading plus the rounded ascent, `floor(h·s) / s + round(a·s)
   / s`, at all 45 heights and both scales.
5. **A border width is floored in device pixels**, as CSS Values 4 says and
   `line_width.rs` foresaw: computed `4.8px` is `4px` at 1 and `4.5px` at
   2, `0.3px` and `0.7px` are `1px` and `0.5px`, `1.6px` is `1px` and
   `1.5px`, `2.74px` is `2px` and `2.5px`, and `1.3px` is `1px` at both.
   An `outline` of `2.6px` is `2px` and `2.5px`.
6. **A control's picture and a plain background are snapped in device
   pixels.** Screenshots at 1 and 2 of checkboxes and black boxes placed at
   tenths of a pixel: each edge is `snap.rs`' rule, `floor(x + 0.5)`, taken
   of `x·s`. A checkbox at (10.3, 22.77) is drawn from device pixel (21,
   46) at 2 — (10.5, 23) CSS — and from (10, 23) at 1. A box 13.4 square
   at (200.2, 22.2) covers device pixels 400 to 427 across at 2, which is
   13.5 CSS pixels wide, and 200 to 214 at 1.

## The decision in one line

**A page is laid out for the scale of the screen it is shown on.** Every
rule this engine holds that rounds to whole pixels, or to the grid of
1/64 px, rounds to **device** pixels — a value `v` in CSS pixels is
rounded as `rule(v·s) / s` — and everything is still **reported in CSS
pixels**. Paint draws the same display list scaled by `s` onto whole
device pixels, snapping in device pixels. The scale is **one number,
carried with the viewport**, from which `devicePixelRatio` and the
`resolution` media feature are answered.

## Why this is a decision rather than a chore

ADR 0024 § 3 said *"lay out in CSS pixels, paint at the scale factor"*,
and item 299 was written to close on layout being identical at 1 and 2.
Built that way it would have passed its own closing condition and been
wrong on the screen most Macs have, by up to five and a half pixels in
the corpus. Built this way, a layout is no longer a property of a page
and a viewport alone: the same page has different numbers on two screens,
and so does what an agent reads of it. That changes what a reference
means and what a test must say, and it amends an accepted ADR.

## 1. "In CSS pixels" means reported in them, not rounded in them

ADR 0024 § 3 stands, read this way: layout's numbers are CSS pixels, as
`getBoundingClientRect()` gives them, and the rounding inside layout is
the screen's. At a scale of one nothing changes, which is why every
reference this repository has is untouched by this decision.

## 2. Every whole-pixel rule is a device-pixel rule

Read *pixel* as *device pixel* in each rule this engine already holds,
and the rule is unchanged:

| Rule | Where it is | At scale `s` |
|---|---|---|
| A font's ascent, descent and line gap, each rounded half up | ADR 0050 § 1 | `round(a·s) / s` |
| The leading's top half, floored | ADR 0050 § 4 | `floor(h·s) / s` |
| A set line height, quantised | ADR 0050 § 3 | onto 1/(64·s) px |
| A length, a percentage and a `calc()`, truncated onto the grid | ADR 0054 §§ 1–2 | onto 1/(64·s) px |
| A run of text's width, rounded up onto the grid | ADR 0054 § 4 | onto 1/(64·s) px |
| Shares put on the grid | ADR 0054 § 3, ADR 0056 | onto 1/(64·s) px |
| A border width, floored, and at least one | `line_width.rs` | `floor(w·s) / s`, at least `1 / s` |
| A control's box and everything a box paints for its own area, each edge snapped | `snap.rs` | `floor(x·s + 0.5) / s` |
| An overflow clip: the border box, each edge snapped, less the border | `build.rs` | `floor(x·s + 0.5) / s` |
| A picture: its content box, each edge snapped | `build.rs` | `floor(x·s + 0.5) / s` |
| A text field's character, rounded when wider, and its width, rounded up | `field_width.rs` | `round(c·s) / s`, `ceil(w·s) / s` |
| The size a face is set at, cut to hundredths | `face_size.rs` | `trunc(z·s·100) / 100 / s` |

The last two rows were measured by item 560 and added to the table by it,
the overflow clip's by item 575 and the picture's by item 587 (see
*Measured since*).

The rules are measured at scale 1 and at scale 2 above. Nothing here
claims a rule at 1.5 or 3 that was not measured; the form `rule(v·s) / s`
is what the two scales share, and an item that builds for a fractional
scale measures one before it is trusted.

The scale changes where a value is rounded, never what a value *is*: a
computed `16px` is 16 at every scale, `vw` is a hundredth of the viewport
in CSS pixels, and a face is shaped at the size the page set. This is not
CSS `zoom`, and nothing is laid out in device pixels and divided back.
Each rule in the table rounds the same number either way, so the two
designs differ in where the division lives, not in what is measured, and
rounding against `s` keeps one kind of number in every tree. Where the
corpus at 2 disagrees, the measurement decides, not this paragraph.

## 3. One scale, carried with the viewport

The scale is the window's, and it reaches a page the way the window's
size does. `alo-value`'s `Viewport`, which `vw` and `vh` already read,
gains it, and so does `alo-css`'s `MediaContext`. A style's metrics carry
the viewport to every length layout resolves, so a border width learns
the scale where it is snapped and no new argument threads through layout.
The renderer is told it with its size (`Resize`), the strip's renderer
too, and the frame a renderer sends is in device pixels.

- **Any finite factor above zero** is a scale; anything else is 1. It is
  not rounded: a window at 1.5 is laid out at 1.5. Until a fractional
  scale is measured (§ 2), the window keeps rounding one down to a whole
  factor, as it replicates today.
- `devicePixelRatio` answers the scale, and `(resolution: 2dppx)`,
  `min-resolution` and `max-resolution` are evaluated against it, where
  today they are refused as unknown. ADR 0024 § 3 kept the ratio absent
  so that no page was told a number the renderer did not paint at; once
  it does, the number is true.
- **The viewport in CSS pixels is the window's device pixels divided by
  the scale.** At a whole scale that is the size the window has today.
- The window stops replicating, and a person's pointer is divided by the
  scale as it reaches a page, as `aim.rs` divides by the replication now.

## 4. Paint draws the same list at the scale

The display list stays in CSS pixels: it is what makes a picture
diagnosable, and it should not change because the screen did. Paint draws
it under a scale of `s` onto a canvas of whole device pixels, the
viewport's size times `s`, so a letter is outlined at the size it is
shown rather than drawn small and enlarged. A blur, a shadow's offset and a
dash are already scaled by the transform in force. `snap.rs` takes the
scale and rounds in device pixels (§ 2).

## 5. A reference names its scale

The corpus's committed files are all at scale 1, and stay so: no
reference moves because of this decision. A case drawn at scale 2 has
its own expectations beside the others, never in place of them, and its
layout is asserted against Chrome 154 started with
`--force-device-scale-factor=2`, in numbers, to the grid of 1/128 px. A
test that lays a page out says its scale, as it says its size.

An agent reads CSS pixels at the window's scale. Two agents on two
screens can read numbers that differ by the rounding. Chrome's own
accessibility tree does the same, and an agent that needed one answer
everywhere would be reading geometry, which ADR 0002 already says it
should not lean on.

## 6. How it is built

Item 299 is cut into three parts, and closes when they do:

- **560. Layout for a scale** (§§ 2–3): the scale on `Viewport` and
  `MediaContext`, and every rule in § 2's table taking it. Closes on the
  corpus laid out at 2 matching Chrome's real scale-2 numbers in layout
  assertions, and every committed file at 1 unchanged.
- **561. Paint at a scale** (§ 4): `render` under the scale, `snap.rs`
  in device pixels, and `pipeline.rs` drawing a canvas of device pixels.
  Closes on reference renders of the same frozen page at 1 and 2.
- **562. The window at the screen's scale** (§ 3): `Resize` carrying the
  scale, both renderers painting at it, the window composing without
  replicating, the pointer divided by it, and `devicePixelRatio` and
  `resolution` answering it. Closes on `alo` on this Mac captured with
  `screencapture -l` and compared with the composed reference at 2.

## What this costs

- **A layout per screen.** A window moved to a screen of another density
  is laid out again, not only painted again.
- **Every test that lays out says at what scale**, and the scale-2
  expectations are a second set of files to keep.
- **The agent's numbers depend on the screen**, by the rounding (§ 5).

## Alternatives rejected

- **Lay out at 1 and paint at the scale** — ADR 0024 § 3 as first read,
  and item 299 as written. 744 of the corpus's 1,761 boxes would differ
  from Chrome's on a scale-2 screen, 79 of them by more than a pixel. A
  wrong pixel is a bug (law 3), and this one would be on most Macs.
- **Lay out in device pixels**, every length multiplied by `s` and every
  answer divided back. The rounding is the same (§ 2), but every reader
  of a tree — the agent, the hit test, `getBoundingClientRect()`, a
  corpus file — would divide, and one that forgot would be wrong by a
  factor of two rather than a fraction.
- **Measure under emulation** — not an alternative but a trap: it reports
  the ratio and lays out at 1 (Evidence), and would have confirmed the
  wrong closing condition.

## What is not decided here

- **A fractional scale's rules**, until measured (§ 2).
- **Where a pointer lands at a scale**: ADR 0024 § 5's point is a 1×1
  pixel, measured at scale 1; whether it is a device pixel at 2 is
  measured with item 568 (cut from 562).
- ~~**`devicePixelRatio` and `resolution`'s answers** in each unit~~:
  measured by item 567 (*Measured since*).
- ~~**A face's size cut to hundredths** at a scale~~: measured by item
  560, in device pixels (*Measured since*).
- **An `outline`'s width**, which Chrome also snaps (Evidence 5), when this
  engine draws outlines by width.

## Measured since

**Item 560, 2026-10-13, Chrome 154 under `--force-device-scale-factor=2`,**
the source not read.

- **A face's size is cut to hundredths of a device pixel.** A run of a
  hundred `x` in `DejaVu Sans` at ten sizes written to the thousandth or
  finer was the width of the face set at `trunc(z·s·100) / 100 / s` at nine
  of them, `13.337px` set at 13.335 rather than 13.33; the tenth, `9.123px`,
  is the same 1/64 px off at one. Cut in CSS pixels, seven would have been
  0.3 px narrow.
- **A text field's width is a device-pixel rule.** 621 fields in `DejaVu
  Sans`, `DejaVu Sans Mono` and `DejaVu Serif` at 69 sizes, each at `size`
  1, 3 and 20, were all `ceil(w·s) / s` with the `x` rounded as
  `round(c·s) / s` when that makes it wider, at one and at two; either
  rounding in CSS pixels left 148 or more wrong. The corpus's text fields
  are 164.5 px wide at two, where this engine first made them 165.
- **The corpus at two.** With every rule above taking the scale, the 580
  boxes of the 52 markup cases this engine places exactly where Chrome does
  at one, 578 are exactly where Chrome puts them at two. The two that are
  not are flex items that shrink only at two; the same shrink at one is a
  unit off too, so it is how a shrunk item's share is rounded, not the
  scale (item 564).

**Item 561, 2026-10-13, the same Chrome under the same flag,** a
screenshot of `cases/checkbox-beside-text` at 600 by 400, each element set
in the corpus's DejaVu face, without `--disable-gpu`.

- **A control's picture is on whole device pixels, as § 2 says.** Each of
  the case's nine checkboxes and radios is drawn from
  `floor(x·2 + 0.5)` of its `getBoundingClientRect()` at two, every edge
  of the box, and an unchecked checkbox's 1 px edge is two device pixels
  of `rgb(118, 118, 118)`. This engine, drawing the same list under a
  growth of two, puts all nine on the same device pixels.
- **A plain background is in the table and not built.** The row was taken
  from iteration 290's measurement at one, but this engine has never
  snapped a plain background, at one or at two, and doing so moves
  pictures at one. It is item 565, measured at both scales before any
  reference moves, and until then that half of the row is not true of
  this engine.

**Item 563, 2026-10-13, against the same Chrome boxes** (frozen beside each
case by iteration 330; nothing new was measured). A renderer is told the
scale with the page it loads and lays the page out at it for the page's
life, a resize and a script's measurement included. The 21 cases whose
page runs script, loaded so, put every one of the 313 boxes this engine
places exactly where Chrome does at one exactly where Chrome puts them at
two; with the markup cases, 891 of 893.

**Item 562, 2026-10-13, on this Mac's built-in Retina display** (2,880 ×
1,864 device pixels, scale two); nothing was measured in Chrome. The window
tells both renderers its whole scale with its size and places their frames
one to one. `alo --frozen-fonts` on `cases/alo-offline`, captured with
`screencapture -l`, is the window composed at two through the real
conductor: of 2,799,416 opaque pixels, 2,795,059 are exact and all are
within one level once put through the capture's display profile, as at
scale one in iteration 316.

- **A page's picture stays at most 4,096 device pixels a side**, at every
  scale, decided here: `pipeline.rs`' bound is the frame's and is not
  multiplied by the scale. A frame of 4,096 square is already as large as
  a message across the boundary may be, so a window wider than that —
  2,048 CSS pixels at two — shows its page cut at that edge and the
  window's background beyond, until item 569 decides how a larger frame
  crosses.

**Item 567, 2026-10-13, Chrome 154 under `--force-device-scale-factor=1`
and `=2`,** the source not read; 3,032 `matchMedia` answers, frozen as
`alo-css/tests/references/resolution.chrome.txt`, and a sheet's `@media`
answering alike.

- **`devicePixelRatio` is the scale**, 1 and 2: an own accessor of the
  window, getter and setter, enumerable and configurable, not on
  `Window.prototype`, and `[Replaceable]` — assigned 3, it is a writable
  data property holding 3. It is answered here from the page's view, asked
  at every read, so a window moved to a screen of another scale answers
  the new one.
- **A resolution is read as a double and compared as an `f32`** in
  `dppx`: `dpi` is a 96th of one and `dpcm` 2.54/96, converted before the
  number is made an `f32`, so `192.00001dpi` is two and `191.9dpi` and
  `1.99999dppx` are not. **`dpcm` alone is compared in hundredths**: the
  query and the scale are each rounded to a hundredth of a `dppx`, a half
  up, so `75.59dpcm` and `75.6dpcm` are both two. This fits every one of
  the 2,856 answers of the second probe; exact comparison misses 160.
- **Units in any case; zero is a resolution, a negative one is not**;
  `infinite`, a bare number and `px` match nothing. Chrome also takes
  `calc()`, the range syntax, the boolean `(resolution)` and the prefixed
  `-webkit-device-pixel-ratio`; this engine refuses each as it refuses them
  for `width` (item 571; the prefix is stage 3's, item 141).
- **`cases/a-page-told-its-scale`**, whose script rounds a floating box to
  the screen's device pixels with `devicePixelRatio` (floating-ui's
  `roundByDPR`, as alo's workspace bundles it) and whose sheet asks
  `resolution` three ways, has every box Chrome's at one and at two, and
  its hairline, band and floating box on Chrome's device pixels at two.

**Item 565, 2026-10-13, the same Chrome under `--force-device-scale-factor=1`
and `=2`,** screenshots without `--disable-gpu`, the source not read: two
probes of twelve kinds of box, 56 of each kind at tenths of a pixel in
each (one written, one seeded), and a third of 96 inset and 96 blurred
shadows; every box drawn at one and at two, and compared with Chrome's own
`getBoundingClientRect()` from the same run. A count below is of drawings,
two to a box.

- **Everything a box paints for its own area is drawn from its border box
  on whole device pixels**, each edge `floor(x·s + 0.5) / s`, at one and
  at two: a background colour (224 of 224), a gradient (224), a border on its outer and inner edges (224 and 224), a
  rounded box (224), a faded box (224), a box inside a box (224), a piece
  of an inline box (224), a spread shadow (224), an inset shadow (192 of 192,
  exact), and a blurred shadow (192, within 0.011 of a Gaussian of the
  snapped box where the unsnapped box is 0.068 off). This is the table's
  row, now true of this engine: `alo-paint`'s `build.rs` draws a piece's
  shadows, background and border from `snap::to_device_pixels` of it.
- **A shadow's offset is added after the box is snapped** (224 of 224),
  so an offset of a fraction leaves the shadow's edges a fraction into a
  pixel: `4.3px` on a box snapped to 240 ends at 244.3.
- **A transform moves the snapped box.** Under `scale(1)` a box is snapped
  as without one (224); under `translate(0.5px, 0.5px)` its edges are the
  snapped box's moved by a half (224); under a translation of a fraction
  223 of 224 are the snapped box moved, and one, 10.3 wide at 490 moved
  by 0.8 at one, is drawn a pixel wider by Chrome, which is not explained.
- **A fieldset's border is snapped from the line through its legend**,
  not from the top of its box: `web-a-form`'s second fieldset, laid out
  from 287.59375 with the line 8.5 below, is drawn from 296 where snapping
  the box first would draw it from 297 (`banded.rs`).
- **What Chrome draws differently and this item does not change.** At a
  corner where two soft edges meet, Chrome covers a pixel by the lesser of
  the two edges' coverage, not by area; a spread shadow on a square box
  keeps square corners where this engine rounds them by the spread (item
  573); this engine samples a soft edge sixteen by sixteen (item 459), so
  0.625 where Chrome covers 0.6.
- **Not measured, and not snapped here:** an overflow clip, a picture's
  box, an `<svg>`'s viewport, and a fractional scale (item 575).
- `cases/backgrounds-on-device-pixels` has one of each kind; every pixel
  along each box's middle row and column is Chrome's exactly at one and
  at two, but the shadow moved by a fraction, within 6 of 255. At one,
  34 corpus cases' display lists changed (31 of their `render.png`s, and
  `sent.png` and `filled.png`), every changed line a box's own fill or
  shadow moved onto whole pixels or a fieldset's band snapped on its own;
  `a-page-told-its-scale` at two is nearer Chrome's picture (1.06 levels
  on average to 0.93). `web-a-form`'s fieldsets now draw their grooves on
  Chrome's rows, in this engine's colours rather than Chrome's (item 520),
  so it is further from Chrome's picture by those rows (2.57 to 2.67),
  where the smear had happened to land near Chrome's grey.

**Item 566, 2026-10-14, Chrome 154 under `--force-device-scale-factor=1`
and `=2`,** screenshots without `--disable-gpu`, the source not read: `x`,
three no-break spaces and `x` in `DejaVu Sans` at twenty sizes from 8 to
64 px, each underlined, overlined and struck through, on a whole pixel
and a quarter, a half and three quarters below one; 480 lines, read from
the columns over the spaces.

- **A text decoration is a whole-device-pixel rule** (480 of 480), and
  every row it touches is wholly inked. In device pixels: its thickness is
  `t = max(1, size·s/10)`, drawn cut to a whole number; the top of the
  text is its piece's top rounded as `floor(y·s + 0.5)`, the baseline the
  ascent below it; an underline starts `max(1, ceil(t/2))` below the
  baseline, an overline its drawn thickness above the top, a line through
  `2/3` of the ascent below the top less `t/2`; and each line's top is
  rounded as `floor(y + 0.5)`. The face's own underline figures are not
  used, so ADR 0050 § 5's exact underline is replaced by this rule; that
  section said the rounding was not measured.
- **It is not one rule at one and another at two**: the rule above is the
  same at both scales, and at one it moved this engine's lines too,
  `text-decorations` among them, from fractions of a pixel onto Chrome's
  rows. `alo-paint`'s `decoration.rs` draws it.

**Item 575, 2026-10-14, Chrome 154 under `--force-device-scale-factor=1`
and `=2`,** screenshots without `--disable-gpu`, the source not read: two
probes (one written at tenths, one seeded) of nine kinds of box that
clips, 56 of each, each holding a black child reaching 5 px past it on
every side, so that what is drawn of the child is the clip; read along
each box's middle row and column against Chrome's own
`getBoundingClientRect()` and computed border from the same run.

- **An overflow clip is on whole device pixels**: the border box with
  each edge rounded as `floor(x·s + 0.5) / s`, less the border, which is
  already whole device pixels (§ 2's row). 2,016 of 2,016 clips, at one
  and at two: a plain box, one with a background, a `2px` border, a
  `2.6px` border (floored first), a rounded box, `overflow: auto`,
  `overflow-x` alone, a box in the flow and an inline block on a line.
  The padding box as laid out, smoothed across its edge pixels, fits 27,
  each a box already on whole pixels; rounding outwards or inwards fits
  193 and 160. So a clip is where the
  box's own background is drawn, and a child reaching its edge is cut
  where that background ends. `alo-paint`'s `build.rs` clips so.
- `cases/clips-on-device-pixels` has one of each kind and
  `overflow: clip`; every pixel of it is Chrome's exactly at one and at
  two, but within 5 px of the rounded box's corners, where the curve's
  smoothing is within 43 and 51 of 255 (item 459). At one, five corpus
  cases' display lists moved, each a clip onto whole pixels; no picture
  changed.
- **Not measured here**, and cut to item 587: a picture's box, an
  `<svg>`'s viewport, an outline, and any fractional scale. Whether a
  clip's corners are the padding box's radii, as an inset shadow's are
  (item 585), is item 588.

**Item 587, 2026-10-14, Chrome 154 under `--force-device-scale-factor=1`
and `=2`,** screenshots without `--disable-gpu`, the source not read: two
probes (one written at tenths, one seeded) of seven kinds of `<img>`, 56
of each, each showing an 8 × 8 PNG of solid black, so that the black
drawn is the picture's box; read along each picture's middle row and
column against Chrome's own `getBoundingClientRect()`, computed border
and padding from the same run.

- **A picture is drawn into its content box with each edge on a whole
  device pixel**, rounded on its own as `floor(x·s + 0.5) / s`: 1,568 of
  1,568 pictures at one and at two — at its own size, at a size set, with
  a `2px` border, with a `2.6px` border (floored first), with a padding of
  fractions, in the flow inside a positioned box, and on a line. It is the
  content box rounded, not the snapped border box less its border and
  padding: of the 224 padded pictures the second fits none, because its
  edges fall between pixels, and 37 even when those are rounded again.
  This engine drew a picture from its content box's left edge rounded and
  its width rounded, which, modelled, fits 1,055 of the 1,568. `alo-paint`'s `build.rs` draws the picture so.
- **A picture that starts off the page starts there.** This engine slid
  a picture whose left or top edge was below zero back onto the page
  whole; `drawn_picture.rs` now passes over the columns and rows off it.
- `cases/pictures-on-device-pixels` has one of each kind and two pictures
  starting off the page; every pixel of it is Chrome's exactly at one and
  at two. No other corpus case moved: every picture in it was already on
  whole pixels.
- **Not measured here**, and cut to item 589: an `<svg>`'s viewport and
  any fractional scale. What is drawn *inside* a picture not at its own
  size, which at two is every picture, stays item 179's: the case's
  pictures are of one colour, so sampling is not what it compares. An
  `outline` is not drawn by this engine at all (item 591), so there is
  nothing of it to snap yet.

**Item 588, 2026-10-14, Chrome 154 under `--force-device-scale-factor=1`,
`=2` and `=8`,** screenshots without `--disable-gpu`, the source not
read: two probes, twelve chosen and forty-eight seeded boxes 44 by 40 on
whole pixels, each with a transparent border of one to thirteen pixels a
side, rounded corners (lengths, percentages, ellipses, four different
corners, radii too large for the box) and `overflow: hidden`, each holding
a black child reaching past it; every pixel of each padding box scored
against a model of the clip sampled 4 by 4.

- **A rounded clip is the padding edge's curve**: each radius resolved
  against the snapped border box, fitted to it, and then less the border
  beside it, never below square, which is CSS Backgrounds 3's rule. It
  fits 60 of 60 within 0.14 of coverage at eight and two and 0.19 at one.
  This engine's rule — the radii resolved against the padding box and
  fitted to it, with nothing taken off — fits 2 of 60, both where the two
  agree; taking the border off without fitting the radii to the border
  box first fits 56 at eight. `alo-paint`'s `build.rs` `clip_of` clips so.
- `cases/rounded-clips-inside-borders` has seventeen such boxes, seven
  with blue borders and two at tenths; wherever both pictures have a plain
  colour it is the same, at one and at two. `rounded-card`'s picture moved
  in 58 pixels at its card's corners, which are rounded by 11 inside its
  1 px border where they were rounded by 12; no display list moved.

## How we will know if this was wrong

**If a case laid out at 2 differs from Chrome's real scale-2 numbers** by
more than a unit of 1/128 px after item 560, a rule in § 2 is not the
device-pixel rule it was read as, and the table is corrected from a
measurement.

**If a page is told a `devicePixelRatio` it is not painted at**, § 3 has
been broken, and the ratio is withdrawn until it is true again.

**If any reference at scale 1 moves** because of items 560–562, the scale
has leaked into the default, and that is a defect in the change, not a
new reference.
