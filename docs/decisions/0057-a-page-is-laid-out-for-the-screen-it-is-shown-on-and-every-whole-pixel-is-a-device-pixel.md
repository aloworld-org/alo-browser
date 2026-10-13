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
| A control's box and a plain background, each edge snapped | `snap.rs` | `floor(x·s + 0.5) / s` |
| A text field's character, rounded when wider, and its width, rounded up | `field_width.rs` | `round(c·s) / s`, `ceil(w·s) / s` |
| The size a face is set at, cut to hundredths | `face_size.rs` | `trunc(z·s·100) / 100 / s` |

The last two rows were measured by item 560 and added to the table by it
(see *Measured since*).

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
  measured with item 562.
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
