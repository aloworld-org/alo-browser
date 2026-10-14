# ADR 0050 — A line is built from whole pixels of font, and its leading is floored

**Status:** accepted
**Date:** 2026-10-11
**Context:** queue item 460, *a line is as tall as Chrome's whole-pixel
ascent and descent make it*, filed in iteration 290 while building item
457. Its entry says the rule must be written down before it is built,
because it moves every line. `checkbox-beside-text` is the case that
opened it. At 16 px, `DejaVu Sans`' ascent is 14.85 here and 15 in
Chrome 154. The case's first row is 19.77 tall here and 20 in Chrome,
and by its last row a control stands at y 160.05 here and 162 in Chrome.
`checkbox_beside_text.rs` allows 0.75 px for that difference.

Also read: ADR 0001 (law 1 and law 3); ADR 0045 (a font size remembers
where it came from, and the form a value was written in can matter);
`alo-text`'s `font.rs` (`FaceMetrics`, `Font::metrics`), `measure.rs`
(`TextMeasurer::ascender`, `descender`, `face`) and `line.rs` (a
paragraph line takes the tallest run's figures); `alo-layout`'s
`inline.rs` (`reach_of`, `font_of`, `empty_line_baseline`, the strut),
`measure.rs` (`TextStyle::line_height`), `text_style.rs`, `marker.rs`,
`ellipsis.rs` and `vertical_align.rs`; `alo-style`'s `metrics.rs`
(`set_line_height_in`) and `computed.rs` (`set_line_height`); and
`docs/features.md`' line-height entry.

**Measured 2026-10-11, on this machine, in Google Chrome 154**, headless
and without `--disable-gpu`. The probe pages were written for this
decision and are kept nowhere. Each one is a few lines and can be rebuilt
from the tables below. Each probe line is a `div` 400 px wide holding
`<span>xg</span>` and an `inline-block` 1 px wide and 0 tall that stands
on the baseline. Each line reports its height, the span's top and height
(the content area), and the marker's top (the baseline), all from
`getBoundingClientRect()`. There were 704 lines, in three probes. The fonts were the
corpus's `DejaVu Sans` and five probe fonts made from it with `fontTools`,
which differ only in their `hhea` table:

| font | `hhea` ascent / descent / line gap (of 2048) | what it isolates |
|---|---|---|
| `DejaVu Sans` | 1901 / −483 / 0 | the corpus's face |
| Gap | 1901 / −483 / 300 | a line gap |
| Odd | 1700 / −700 / 0 | which table is read: `OS/2` is unchanged |
| Half | 1536 / −512 / 512 | exact halves at 2, 6, 10 … px |
| Prim + Tall | Prim is `DejaVu Sans` without `Z`; Tall is only `Z`, 2111 / −555 | a fallback font on the line |

The sizes were 8 to 40 px, including 13.3, 13.309, 13.5, 15.3, 26.7 and
26.71. The line heights were `normal`, the numbers 1, 1.15, 1.2, 1.23,
1.37 and 1.5, `1.5em`, `137%`, and lengths from 12 px to 20 px including
17.32, 17.34, 17.99, 18.01 and 19.51. Chromium's source was not read for
this ADR. No other engine is installed here, so nothing below claims what
Firefox or Safari do, or what Chrome does on another operating system.

## The decision in one line

A font's **ascent, descent and line gap are each rounded to a whole
pixel**, half up, before a line uses them. A `normal` line is those three
added together. A line height that was set is **quantised to 1/64 px**:
a number is multiplied by the font size rounded to 1/64 and the product is
floored, and a length or percentage is rounded. The leading's top half is
**floored to a whole pixel**, and the bottom takes what is left.

## Why this is a decision rather than a chore

**It moves every line on every page.** Every text box's top, every line
box's height, and every block holding text moves by up to a pixel per
line. A page of twenty lines can drift twenty pixels from where this
engine puts it now. A commit that did that without a written rule would
leave the next reader unable to tell a deliberate pixel from a bug.

**Nothing specifies it.** CSS Inline 3 defines the content area and the
leading as real numbers. Whole-pixel ascent and descent, a floored half
leading and a 1/64 grid are what Chrome does, not what CSS asks for.
Following them breaks the principle this engine has kept so far: keep
the font's numbers exact and let paint decide pixels. That is a choice,
and § *Why we take it* says why it is made.

**It splits what was one number.** Today `FaceMetrics::ascender` is both
the font's figure and the line's. After this ADR they are two different
numbers. Underlines, `ex`, `ch` and `vertical-align: middle` keep the
exact figure, and a line takes the whole-pixel one (§ 5).

## 1. A font's three figures are rounded, each on its own, half up

At the font's used size, the line's ascent is `floor(ascent + 0.5)`, its
descent `floor(descent + 0.5)` and its line gap `floor(line gap + 0.5)`.
The three are rounded separately, not as a sum. The figures are the
`hhea` table's, which is what `ttf-parser`'s `ascender()`,
`descender()` and `line_gap()` already read for these faces.

| measured | ascent | descent | gap | Chrome |
|---|---|---|---|---|
| `DejaVu Sans` 16 px | 14.85 → 15 | 3.77 → 4 | 0 | content area 19 |
| `DejaVu Sans` 8 px | 7.43 → 7 | 1.89 → 2 | 0 | 9 |
| Gap 9 px | 8.35 → 8 | 2.12 → 2 | 1.32 → 1 | `normal` 11; the sum rounded would be 12 |
| Gap 20 px | 18.56 → 19 | 4.72 → 5 | 2.93 → 3 | `normal` 27; the sum rounded would be 26 |
| Odd 16 px | 13.28 → 13 | 5.47 → 5 | 0 | 18, baseline 13: `hhea`, not `OS/2` |
| Half 10 px | 7.5 → 8 | 2.5 → 3 | 2.5 → 3 | 11, and `normal` 14: halves go up, not to even |
| Half 2 px | 1.5 → 2 | 0.5 → 1 | 0.5 → 1 | 3, and `normal` 4 |

The **content area** is the rounded ascent plus the rounded descent. That
is the span's height in every row measured, so a text box and an inline
box's background are whole pixels tall.

## 2. A `normal` line is ascent, descent and line gap

With `line-height: normal`, a font takes `A + D + G` of room, using the
rounded figures. The line gap is leading like any other, so its top half
is floored (§ 4): Gap at 16 px is 21 tall, its letters 1 px down and its
baseline at 16.

This engine treats `normal` as having **no leading at all**. That is what
`inline.rs` and `docs/features.md` say, and it was true only because the
corpus's faces have no line gap. Building this ADR corrects both.

**On a `normal` line, a fallback font counts with its own figures.** In
Prim + Tall, `aZ` at 24 px is 32 tall where `ab` is 28: Tall's rounded 25
above and 7 below, against Prim's 22 and 6. The span's content area is
still Prim's 28, so the primary font sets the content area. With a line
height that was set (`1.5`, `20px`), `aZ` and `ab` are the same height, so
a fallback font changes nothing. Whether a fallback font's line gap also
counts was not measured, because Tall has none.

## 3. A set line height is quantised to 1/64 px, in one of two ways

The computed value stays exact: `getComputedStyle` reads `18.221px` for
`137%` at 13.3 px. The line box uses a value quantised to Chrome's 1/64 px
layout unit, and how it is quantised depends on how it was written:

- **A number** is multiplied by the font size *rounded to 1/64*, and the
  product is **floored** to 1/64. At 13.309 px, `1` is 13.3125 (852/64).
  The raw size floored would be 851. At 26.7 px, `1` is 26.703125 and
  `1.23` is 32.84375. At 16 px, `1.23` is 19.671875 (1259.52 floored),
  and at 13.3 px, `1.37` is 18.203125 (1165.87 floored).
- **A length or percentage** is **rounded** to 1/64. `17.34px` is
  17.34375 (1109.76 → 1110), `18.01px` is 18.015625, `1.5em` at 13.3 px
  is 19.953125 (1276.8 → 1277), and `137%` at 13.32 px is 18.25.

So what crosses from style to layout has to say which of the two was
written. A bare `f32` cannot carry that, for the same reason ADR 0045's
font size had to remember where it came from.

## 4. The leading's top half is floored to a whole pixel

With `L` as the quantised line height and `A + D` as the content area,
the leading is `L − (A + D)`. The room above the baseline is
`A + floor(leading / 2)`, floored towards negative infinity, and the room
below is `L` minus that. So a line holding only its strut is exactly `L`
tall, and any odd pixel or fraction goes below the letters:

| `DejaVu Sans` | `L` | leading | above the letters | baseline |
|---|---|---|---|---|
| 8 px, `1.5` | 12 | 3 | 1 | 8 |
| 8 px, `20px` | 20 | 11 | 5 | 12 |
| 8 px, `1` | 8 | −1 | −1 (−0.5 floored) | 6 |
| 8 px, `1.15` | 9.1875 | 0.1875 | 0 | 7 |
| 16 px, `17.3px` | 17.296875 | −1.703 | −1 | 14 |

The same rule applies to every inline box and every piece of text on the
line, each with its own font and its own `L`. A line of 16 px text with a
30.5 px span at `1.5` is 45.75 tall. The span's `L` is
`floor64(30.5 × 1.5)` = 45.75, its content area is 28 + 7, and so its
leading's top half is `floor(5.375)` = 5. Its room above is 33 and below
is 12.75, and those win over the strut's 17 and 7. With `20px` the same
span takes 20 above and 0 below, and the line is 25, as Chrome measures
it.

## 5. What stays exact

Only a line's vertical room changes. These keep the font's exact figures:

- **`vertical-align: middle`** uses the exact x-height. At 16 px, a 24 px
  span set `middle` puts the line's baseline at 18.375, which needs half of
  8.75, not of a rounded 9. `super` is `font size / 3 + 1`, as
  `vertical_align.rs` already says.
- **Underlines, `ex` and `ch`** keep `FaceMetrics`' exact figures.
  Whether Chrome rounds an underline's position was not measured, and
  this ADR does not move it. *(Since measured, by queue item 566: a
  decoration does not use the face's figures at all, and is drawn on
  whole device pixels; ADR 0057, Measured since. `ex` and `ch` stay
  exact.)*
- **Advances, widths and where a glyph is drawn across** are not touched.
- **The computed `line-height`** is not touched, so neither is an `lh`
  unit.

`FaceMetrics` stays the font's own, exact. The rounding happens where a
line asks for the font's room: `MeasureText::ascender` and `descender`,
and a line gap the trait does not yet have. Those answers then mean "how
far a line of this text reaches", and every reader of them in
`alo-layout` is about lines: the strut, inline boxes, text, markers and
the ellipsis. `alo-text`'s own paragraph lines (`line.rs`) follow the
same rule, so that `measure` and the line builder do not disagree.

## Why we take it

Two reasons, and either one alone would do.

**The measure is Chrome's picture.** Every comparison this engine has
made with Chrome since item 454 has had to say "compare at each one's own
position", because lines drift by up to two pixels over a short page. A
reference that drifts cannot hold an engine to anything below the drift.
Items 457 and 461 snap controls and backgrounds to whole pixels as Chrome
does, and those snaps are measured from positions that are themselves a
pixel off.

**It is not legacy.** Law 1 refuses what exists to keep broken pages
working. This rule exists so that text sits on whole pixels and lines
have whole-pixel heights, which every engine a person compares with does.
A page written today and laid out by this engine would otherwise be a
pixel per line away from the same page in the browser it was written
against.

## What is not decided here

- **A font whose `OS/2` table says to use its typographic metrics**
  (`USE_TYPO_METRICS`). No face here has that bit set, and none was
  measured.
- **A fallback font's line gap** on a `normal` line (§ 2).
- **Chrome on Linux, Windows or Android.** This ADR records macOS
  Chrome 154 only.
- **A device pixel ratio other than 1.** A whole pixel here is a CSS
  pixel, because the corpus renders at 1×.

## Consequences

- Queue item 460 builds §§ 1–4 for the primary font: the line's rounded
  figures, the line gap on `normal`, the quantised set line height and the
  floored leading. It moves every corpus case's `layout.txt`, `boxes.txt`,
  `display.txt` and `render.png`, and closes when `checkbox-beside-text`'s
  rows are Chrome's heights and its controls at Chrome's y to 1/64 px.
  The 0.75 px in `checkbox_beside_text.rs` becomes 1/64.
- Queue item 465 builds § 2's fallback rule: a fallback font on a `normal`
  line adds its own rounded room. It waits for a frozen page that shows
  it.
- `docs/features.md`' line-height entry says that `normal` adds the line
  gap, and that a line is built from whole pixels, in the change that
  builds 460, not in this one: nothing is built yet.

## How we would know this was wrong

**If a frozen case's rows drift from Chrome's after 460 is built**, at a
size or line height not in the tables above, the quantisation in § 3 or
the floor in § 4 was read wrongly from too few points.

**If a face with a line gap is drawn with its letters a pixel away from
Chrome's**, § 2's placement of the gap is wrong.
