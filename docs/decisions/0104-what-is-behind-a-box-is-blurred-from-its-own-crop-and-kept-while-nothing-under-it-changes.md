# ADR 0104 — What is behind a box is blurred from its own crop, and kept while nothing under it changes

**Status:** accepted — built by queue items 885 and 886 (8 November 2026)
**Date:** 2026-11-08
**Context:** Filter Effects 2 (`backdrop-filter`, the backdrop root) and
Filter Effects 1 § 13.2.1 (`blur()`'s length is a standard deviation);
ADR 0101 (a drawing painted over the last only where their lists differ,
§ 3: *nothing `render` draws reads one pixel to write another*); ADR 0102
(coverage kept from one drawing to the next, `alo_paint::Kept`); ADR 0100
(a card's page drawn beneath the first screen by a renderer of its own);
ADR 0057 § 4 (a list drawn at its scale onto device pixels); queue items
885 and 886, and the owner's design of 8 November 2026 (*Settings* as a
popup over the blurred, dimmed conversation); `CLAUDE.md`, *correct
before fast* and *every rendering decision is testable*

## The question in one line

When a box asks for what is behind it to be blurred, which pixels are
"behind it", how are they blurred so that the picture is Chrome's, and how
does painting over only what changed (ADR 0101) stay right once one item
reads pixels it did not draw?

## Measured

Google Chrome **157.0.8103.38**, headless, on its GPU path (no
`--disable-gpu`), on this Mac, 8 November 2026, at
`--force-device-scale-factor` 1, 2 and 8; pages of one-device-pixel lines,
random grey stripes, random bars, colour checkers and edges under boxes of
`backdrop-filter: blur(R)`, screenshots read per pixel (the session
scratchpad's `m885/`, its `README.txt` naming each probe; Chrome's source
was not read). The Chromium alo carries (CEF 156) was not measured: its
distribution has no client to screenshot with, and building one was not
this item's.

- **The length is the standard deviation**, `σ = R × scale` device pixels
  — not half of it, as a shadow's blur radius is. A line's spread has
  moments 0.996, 2.01, 2.94 and 3.92 for σ of 1 to 4.
- **Up to σ = 4, a Gaussian**: `exp(−i²/2σ²)` for `|i| ≤ ⌈3σ⌉`,
  normalised, across and then down. It draws Chrome's line spread exactly
  at σ 1, 2 and 3, and within a level at 4.
- **Past 4, smaller first.** Halved `L` times until `σ / 2^L ≤ 4` — each
  halving the mean of each pair of pixels from the crop's own top left, an
  odd last pixel paired with itself — blurred there, and grown back a
  halving at a time by bilinear sampling at `(x + ½)/2 − ½`, clamped. On
  bars of 3 to 40 pixels this is within 2 levels of Chrome's, 0.12–0.38 on
  average, where a full-size Gaussian is within 4, 0.3–1.3 on average.
- **Only the box's crop is read.** White inside and black just outside
  stays exactly white to the edge, and the reverse stays black. Past the
  crop it is **mirrored, the edge pixel repeated** (`abc|cba`): on a 4 px
  stripe at the edge, mirrored is within 1 level, clamped 16–68 off,
  reflected 14–22, transparent 47–94. A box at the page's corner likewise
  sees only its own pixels.
- **The crop** is the border box, a transparent border blurred with the
  rest, each axis from its start rounded half up, as long as the box
  rounded half up but no further than the box reaches past where its
  rounded start is: 128 of 128 boxes at left and width in eighths at one
  and two. The background's own rounding of each edge (`snap.rs`) fits
  118; on the others the blur ends a pixel short of the background.
- **Composited in two layers**: the blur drawn over what is there where
  the border box with its radii covers, by its area, and the box's own
  background over that. Inside, the background over the blur is within 2
  levels (0.36 on average). The box's own shadow is not behind it.
- In the colour the page is drawn in, each channel on its own: Display P3
  and linear light were 4.1 and 34 levels off on average.
- `--disable-gpu` draws otherwise (up to 21 levels apart, 1.5 on average
  on a page of six boxes): another blur, which nobody sees on a Mac.

## Decision

### 1. What is behind a box is what was drawn before it, inside its crop

A box with `backdrop-filter: blur(R)` lists **a backdrop item**
(`alo_paint::DisplayItem::Backdrop`) first of all it draws, before its own
shadows and background: its border box as laid out (which the crop is
taken from, `alo_paint::backdrop::crop_of`), its border box with its radii
as its background is filled (where it is drawn), and `R`. It is drawn
onto what the list has drawn so far — the canvas, or the faded group it is
inside, which is Filter Effects 2's backdrop root for an `opacity` below
one — reading only the crop, mirrored at its edges, blurred as measured
above (`alo_paint::backdrop`), and drawn over what is there where the
border box covers inside the clip in force. Grown back from the halvings in
one step per axis, their bilinear weights multiplied out: the same
numbers, and a pixel computed alike whichever region it is asked in.

A box with any `backdrop-filter` but `none` is a **stacking context** and
the **containing block** of what is absolutely positioned inside it, as
Filter Effects 2 § 3 says (`alo-paint`'s `build`, `alo-layout`'s
`containing`). Every other filter function, a list of more than one, and a
`calc()` length are read as CSS and **refused by name** — said in the
page's issues and drawn as if absent — until a page needs one
(`alo_value::BackdropFilter::Unbuilt`).

How close, in `alo-corpus`' `backdrops-blurred` (three boxes over 4 px
cells: `blur(4px)` alone, under `rgba(255, 255, 255, 0.7)` rounded 16,
and `blur(8px)` under *Settings*' veil), every pixel compared with
Chrome's frozen screenshots:

| scale | outside the boxes | inside | on the rim (±2 device px) |
|---|---|---|---|
| one | exact | at most 1, 0.55 on average | at most 15, 0.40 |
| two | exact | at most 2, 0.59 | at most 10, 0.40 |
| eight | exact | at most 2, 0.61 | at most 19, 0.43 |

The rim's worst is at the rounded box's corners, where Chrome covers an
edge pixel otherwise than by its area — the corners' own antialiasing, not
the blur. On a 10 px colour checker blurred with halvings in both
directions a smooth ±3 field is left that the measurements could not
explain (σ, alignment, phase and colour space each ruled out); it is
bounded in the case.

### 2. Painting over: a backdrop's crop is painted whole, or its blur kept

ADR 0101 § 3's argument rested on nothing `render` draws reading one pixel
to write another. A backdrop does — but **only inside its crop**, and it
writes only inside it too. So `alo_paint::damage` (ADR 0101 § 2) goes on to
ask of each backdrop matched in both lists whether any change it found
**drawn before it, in either list,** reaches its crop:

- If one does, what it blurs differs: **its whole crop is damaged**, and
  counted as a change drawn where the backdrop is, for every backdrop
  after it. A repaint confined to a region holding the whole crop draws
  every pixel the blur reads afresh before reading them.
- If none does, what it blurs is what the last drawing blurred: the blur
  is **kept** (`Kept::backdrop`, ADR 0102 § 2) under its crop and its
  standard deviation, bit for bit, and the repaint **takes it as kept**
  (`Kept::stand`) — drawing only the pixels the region holds — rather
  than reading a crop it does not paint whole. A blur not kept (past the
  cap, or a key two backdrops share) is not taken: its crop is damaged
  whole instead.

An unmatched backdrop damages its crop, as any item damages its reach.
Why this is right: a pixel outside the damage is outside every crop whose
blur changed, so every backdrop there draws what it drew; and inside the
region, a backdrop either reads a crop the region holds whole — drawn
afresh, in the same order, before it — or draws the very blur the last
drawing made of the very same pixels. ADR 0101 § 3 is amended to say so.

### 3. The first screen: a card's page is drawn in it while *Settings* is open

Since ADR 0100 a live card's page is composed by the window *beneath* the
first screen, which leaves its place clear: a backdrop over it would blur
the clear place, not the page. So while *Settings* stands over the
conversation (queue item 886) the window asks for the first screen with
the card's last frame **in** it, as before ADR 0100, and the page is not
streamed then (ADR 0076, as when *Settings* stood in the conversation's
place): the page is a picture in the list, drawn before the veil, and
blurred with the rest. Closed, it is beneath again. Two things keep that
from painting the whole screen for every change beside it:

- **A picture of the same bytes is the very same picture** from one
  drawing to the next (`alo_renderer::pictures::Decoded`): the renderer
  takes it as the last drawing decoded it, so ADR 0101 § 2 matches it.
- **A picture is drawn only where a confined canvas keeps** (`drawn_picture`,
  `clipped_picture`): each pixel's mix is its own, so the rest would be
  refused all the same.

## What it costs

Measured in the renderer alone, release, on this Mac, the design's first
screen at two (2080 × 1640 device pixels) with a live card whose frame is
1696 × 1100:

| | ms |
|---|---|
| *Settings* opened over the card: painted whole, the veil blurred | about 350 (the veil's blur 42 of it) |
| Tab, a face or a stepper in it: the rings or the choice painted again, nothing blurred again | 60–105 |
| the same with *Settings* closed, the page in the card | 8 (nothing differs) |

Of a Tab's 60, about 28 is every run of text outlined again — text is
never passed over on a confined canvas, before this ADR as after it — and
the document styled and laid out again. Typing in the conversation with
*Settings* closed draws no backdrop at all: its list has none, and the
first screen's letters cost what they did (ADR 0102's numbers; this
batch's journal says what was measured again). Memory: a kept blur is the
halved image, 3.4 MB for the first screen's veil at two, inside ADR 0102's
64 MiB cap.

## Rejected

- **A full-size Gaussian at every σ.** Simpler, and further from Chrome
  (§ Measured): the halvings are Chrome's picture, not an optimisation.
- **Reusing the shadows' blur (`alo_paint::blur`).** Three box passes at
  half the length are CSS's shadow, not Chrome's backdrop: the kernel is
  another. What is reused is `Kept`, which keeps a blur between drawings
  as it keeps a shadow's.
- **Clamping the crop's edge, or reading the page past it.** Each measured
  wrong: clamped up to 68 levels off at the edge, and a box beside a
  black edge would darken where Chrome's stays white.
- **Damaging every crop a change reaches, before or after the backdrop.**
  Right, and every Tab in *Settings* would be the veil's whole crop — the
  whole screen — painted and blurred again: 230 ms at two, measured while
  every change beside the card's frame still painted the screen whole.
  What is drawn after a backdrop, over it, changes nothing it reads.
- **Blurring in the window**, composing the conversation's frame blurred
  under a frame of *Settings*: the window would decode and blur a page's
  pixels, which ADR 0005 keeps to a confined renderer, and a page's own
  `backdrop-filter` would still need this.
- **Leaving the card's page beneath and drawing *Settings* over its clear
  place**: the page would show sharp through the veil.

## What is not decided here

- **Nested backdrops.** A box with `backdrop-filter` is a backdrop root
  for what is inside it; here only a faded group is one, so a backdrop
  inside another's box reads everything before it (queue item 887).
- **A backdrop under a transform that turns.** Its crop is the bounding
  box of the turned border box; Chrome was not measured there (887).
- **The Chromium alo carries** was not measured (§ Measured).

## Built

Queue item 885: `alo_value::filter` (`BackdropFilter`) and
`parse_backdrop_filter`; `alo-css`' `properties.rs` (`backdrop-filter`,
layout and paint); `alo-layout`'s `containing` (a containing block);
`alo_paint::backdrop` (`Blurred`, `crop_of`, `blurred`, `draw`, `key`),
`DisplayItem::Backdrop`, `build`'s backdrop and stacking context,
`render`'s `draw_backdrop`, `extent::crop_under`, `damage_standing` and
`stood`, `Kept::backdrop`, `stand` and `holds_backdrop`, `repaint`'s
standing backdrops. Queue item 886: `alo_renderer::pictures::Decoded`
and `Drawing::decoded`; pictures drawn only inside a confinement;
`alo-window`'s `start_asked`. Tests: `alo-corpus`' `backdrops-blurred`
with Chrome's frozen screenshots at one, two and eight
(`a_backdrop_blurred_as_chrome_blurs_it.rs`); `alo-paint`'s
`a_blurred_backdrop.rs` and units in `backdrop` (Chrome's line spread at
σ 2, the halvings, the mirrored crop, a region grown back bit for bit as
the whole), `extent`, `repaint` (a change under a blurred box paints its
whole crop; a change over it or past its crop takes the blur kept; a blur
not kept is blurred from a crop painted whole); `alo-renderer`'s
`settings_over_the_conversation.rs` (a card's page blurred under the
veil; a Tab blurs nothing again; a change under the veil every pixel as
drawn whole, at one and two).
