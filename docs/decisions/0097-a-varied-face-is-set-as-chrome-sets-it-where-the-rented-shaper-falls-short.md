# ADR 0097 — a varied face is set as Chrome sets it, where the rented shaper falls short

**Status:** accepted — built by queue items 795 and 796 (2 November 2026), with 797
**Date:** 2026-11-02
**Context:** CLAUDE.md, *Rent the physics, build the engine* (text shaping is
rented: HarfBuzz's port, `rustybuzz`); ADR 0084 (the interface's typefaces,
Instrument Sans and Newsreader, both variable); ADR 0054 § 4 (Chrome's
layout unit); queue item 752 (Newsreader's optical size), which found and cut
795, 796 and 797; Chrome 156.0.8078.76, measured on this Mac (its source not
read)

## The question in one line

The shaper this engine rents sets a variable face's text differently from
Chrome in two ways that move every line of the interface — is the answer a
newer shaper, its successor, a fix offered upstream, or a correction of our
own around it?

## 0. What was measured

In Chrome 156, from the interface's own font files as web fonts, after
`document.fonts.load` resolved: glyph advances by a canvas's `measureText`,
and 2,400 random runs of one to five words — both faces, every weight from
200 to 800, eleven to ninety pixels, `font-optical-sizing` `auto` and `none`
— by a `white-space: pre` span's `getBoundingClientRect().width`, in 64ths.
The runs are frozen in `crates/alo-renderer/tests/references/varied-runs.chrome.txt`.

With each run shaped whole (797) and nothing else changed, **759 of 2,400
were Chrome's width to its layout unit, and the worst 354 64ths off** (5.5
px; Newsreader at 800 and 90 px). Two faults of the rented shaper made the
rest, each found in its source (`rustybuzz` 0.20.1 and the parser it is
built on, `ttf-parser` 0.25.1):

1. **A varied advance is rounded to a whole font unit.** A variable face's
   glyph is its `hmtx` advance moved by the delta `HVAR` interpolates for
   the instance (or, with no `HVAR`, by `gvar`'s right phantom point) — a
   fraction of a unit anywhere but at a master. `ttf-parser`'s
   `glyph_hor_advance` returns it as a `u16`, rounded, and `rustybuzz`
   positions in whole units. Chrome keeps the fraction: Newsreader's `o` at
   `opsz` 40 is 21.2311 px on its canvas and was 21.24 here; Instrument Sans
   at 500 and 600, the interface's two weights between masters, up to 0.0054
   px a glyph at 16 px.
2. **A kerning pair's own variation is lost or rounded.** A pair's value in
   `GPOS` can carry a *variation index* table: Newsreader kerns `iv` −14
   units at `opsz` 18 and varies it by +14 towards 72, so at 72 the pair is
   not kerned at all (Chrome: 60.671875 px; here 60.156). In a pair listed
   glyph by glyph (`PairPos` format 1) `ttf-parser` counts the table's
   offset from the pair records, two bytes into their `PairSet`, rather than
   from the `PairSet`'s start as HarfBuzz and the specification have it, and
   reads only within the records — so the table is almost never found and
   the delta never applied. 21,795 of Newsreader's 22,297 listed pairs vary,
   and 1,656 of Instrument Sans' 1,661. In a pair kerned by classes (format
   2), where the offset is read right, the delta is rounded to a whole unit
   as every position is; Chrome keeps the fraction.

## 1. Decided: a correction of our own, in `alo-text`, around the shaper

Both are corrected after `rustybuzz` shapes a run, in `alo-text`, and
nothing else of shaping is touched:

- `advance.rs` gives back what was rounded off each glyph's advance: the
  exact advance (`hmtx` plus `HVAR`'s `f32` delta, or the phantom point's)
  less the rounded one, in font units — never to a mark (by `GDEF` class)
  or a glyph the shaper set to nothing.
- `pair_variation.rs` walks a run's glyphs as HarfBuzz's pair positioning
  does — each lookup of the run's script's `kern` and `dist` features in
  order, a glyph and the next one the lookup does not skip (by `GDEF` class,
  mark filtering set and attachment class), the first subtable holding the
  pair, and past the second glyph when it has a value of its own — and gives
  each glyph the exact delta of its value's tables less **what the shaper
  applied of it**: the shaper's own reading repeated, its wrong offset and
  its rounding included, so that a pair it did apply is never counted twice
  and a pair it did not is not invented.

Both read the face through `font.rs`'s `Varied` — the face at the run's
instance as `ttf-parser` reads it, set to the same axes as the shaper's
face — so that the parser stays behind the one file ADR 0001 lets name it,
and the two read one instance (at the run's own size, not the size its face
is cut to: an instance read at 13.33 for a run at 13.337 was 0.03 px off,
which `a_typeface_at_its_optical_size.rs` caught).

Measured after: **2,370 of the 2,400 runs are Chrome's to its layout unit,
and the other 30 a 64th apart**, each within a quarter of a 64th of the
grid's edge; every glyph advance at Newsreader's `opsz` 40 and Instrument
Sans' 500 and 600 within 0.000 23 px of Chrome's canvas. What is left is
`ttf-parser`'s coordinate on an axis, cut to a whole 16,384th (6,674 at
`opsz` 40, where Chrome keeps 6,674.96), and sums in `f32`.

## 2. Why not the alternatives

- **A newer `rustybuzz` or `ttf-parser`.** 0.20.1 and 0.25.1 are the
  newest on crates.io (2 November 2026); the project names `harfrust` its
  successor.
- **`harfrust`** (0.15.0), the port of HarfBuzz to `read-fonts`. It may read
  the pair's table at the right offset — not tried here — but it positions
  in whole font units as `rustybuzz` does (by its documentation, not
  measured) — HarfBuzz itself is given a fractional scale by
  Chrome, which no Rust port takes — so both corrections' fractions would
  still be ours to give back. Moving the shaper is a change of its own
  (every shaping call, a second font parser beside `ttf-parser`), worth
  making when it gives something this does not.
- **A fix offered upstream.** The format 1 offset is a bug worth reporting
  to `ttf-parser`, and the owner may choose to; this repository never edits
  another one. When a release reads the offset right, the shaper's own
  reading repeated in `pair_variation.rs` must follow it, or the delta is
  counted twice — the 2,400 runs' test fails at once if it is.
- **Patching the crate in the workspace** (`[patch]` with a copy): a fork
  of rented physics to keep in step, for a correction that fits in two
  files of our own.

## 3. Not covered

- A run set right to left is left as the shaper set it: its glyphs come
  out in drawing order, and HarfBuzz positions them in logical order —
  stage 2's bidirectional work.
- A font whose `GPOS` swaps lookups along an axis (`FeatureVariations`) is
  left alone: which lookups ran there is the shaper's to say.
- A face with no `GDEF` glyph classes is left alone: the shaper decides its
  marks from the characters, which the corrected glyphs no longer name.
- Hinting device tables, for a size in pixels, are applied by neither the
  shaper nor Chrome (neither is given one).

## Consequences

- Every line of the interface set at Instrument Sans 500 or 600, or in
  Newsreader away from `opsz` 18 and 400, moves by fractions of a pixel;
  the references that moved were each looked at (STATE, 2 November 2026).
- The corrections cost a pass over each varied run's glyphs; an unvaried
  face (every corpus case's `DejaVu Sans`) is left at once.
- `crates/alo-renderer/tests/a_varied_face_set_as_chrome_sets_it.rs` and
  `a_typeface_at_its_optical_size.rs` hold Chrome's numbers; a change of
  shaper or parser is checked against them.
