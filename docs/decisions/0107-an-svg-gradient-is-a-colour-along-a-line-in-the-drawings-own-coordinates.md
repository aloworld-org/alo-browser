# ADR 0107 — An SVG gradient is a colour along a line in the drawing's own coordinates

**Status:** accepted — built by queue item 275's first part (`<linearGradient>`)
and used by queue item 913 (the owner's icon); measured against Chrome
157.0.8103.38 on 9 November 2026
**Date:** 2026-11-09
**Context:** ADR 0022 (SVG is a picture we draw, and its insides are not
boxes; § 6 cut gradients and patterns into item 275, *"opened only by a
frozen page that needs it"*); ADR 0067 § 6 (the application's icon is drawn
by this engine); queue item 785 (a `COLR` version 1 glyph's gradients, whose
colour line this reuses); law 3 (correct before fast); the page that needs
it today — **the owner's icon**, `docs/brand/alo-browser-icon.svg`, whose
teal swoosh is filled by a `<linearGradient>` from `#1d6f84` to `#149391`.

## The decision in one line

`fill` and `stroke` may name a `<linearGradient>` anywhere in the document,
and it is drawn as **a ramp**: how far along the gradient a point is, as an
affine function of where the point is in the drawing — `t = a·x + b·y + c`
— and the gradient's stops as a colour line mixed as straight colours, the
way Chrome mixes them; a `<radialGradient>`, a `<pattern>` and a gradient's
`href` are cut into items of their own, drawn in their fallback colour
meanwhile and recorded.

## 1. Why a field of three numbers

An SVG gradient's line runs from `(x1, y1)` to `(x2, y2)` in a space of its
own: the shape's bounding box as a unit square (`objectBoundingBox`, the
default) or the shape's user space (`userSpaceOnUse`), moved by its
`gradientTransform`, inside the shape's own transform, inside the
`viewBox`'s, and finally moved to where layout put the box. Whatever those
transforms are, a projection onto a line composed with an affine map is
still an affine function of the point. So a ramp carries three numbers in
the drawing's coordinates, and moving the drawing (`alo-paint`'s `build.rs`
puts it at the box) is composing them with the inverse of the move.

That is why a gradient across a wide, short box is **squashed with it** —
its lines of one colour no longer at right angles to its line — without a
case of its own: the case's `g2` asks for the diagonal of an 80 by 40 box,
and both other corners are half way, in this engine and in Chrome.

A CSS gradient (`Paint::Gradient`) was not reused: it runs at an angle
through the middle of a box, which cannot say *from this point to that
one under a skew*. Paint gains one word, `Paint::Ramp` and
`DrawingItem::Ramp` — the drawing's vocabulary grows a new kind of paint,
not a new seam (ADR 0022 § 2).

## 2. How the colours are mixed

As **straight colours, then premultiplied**: the colour line `alo-paint`'s
`colour_paint.rs` already holds for a `COLR` glyph's gradient, with its
pad, repeat and reflect. Measured, not read from Chrome's source: the case's
`g3` has a middle stop at `stop-opacity: 0.2`. Mixed so, every pixel of it
is within a level of Chrome's; mixed premultiplied — a mutation, tried —
it is 38 levels off.

## 3. What is drawn

- `x1`, `y1`, `x2`, `y2` (default `0%`, `0%`, `100%`, `0%`); in the box, a
  number and a percentage are the same fraction; in user space they are
  lengths and percentages of the viewport, as on a shape.
- `gradientUnits`, `gradientTransform` (the `transform` attribute's
  grammar), `spreadMethod` — `pad`, `reflect` and `repeat` all three, since
  the colour line had them.
- `<stop>`: `offset` (a number or a percentage, held to the line and never
  less than the one before), and `stop-color` and `stop-opacity`, which
  become presentation attributes and properties of the cascade like
  `fill`.
- No stop paints nothing; one paints its colour; a line of no length paints
  the last stop's colour; a box with no width or height under
  `objectBoundingBox` paints nothing. `fill-opacity` or `stroke-opacity`,
  and a shape's own `opacity`, fade every stop.
- The name is looked up in the whole document, the first element with the
  `id` winning, as `getElementById` answers — so a page may keep its
  gradients in one hidden `<svg>` and paint with them in another.
- A gradient may have at most 256 stops (`alo-svg`'s `bounds.rs`): every
  pixel walks them.

## 4. What is cut, and what stands in

Each recorded in the page's issues, and drawn in the colour written after
the reference, or not at all when none is — SVG 2's rule for a reference
that cannot be used:

- **`<radialGradient>`** — item 910.
- **`<pattern>`** — item 911.
- **A gradient's `href`**, by which it takes the stops and attributes of
  another — item 912. A gradient that has one is drawn from what it writes
  itself.
- A reference into another document is never followed: a drawing reaches
  nothing outside the document it is in, as ADR 0027 § 5 keeps an SVG
  file to itself.

## 5. Measured

`crates/alo-corpus/cases/svg-linear-gradients`: eleven 40 px drawings and
the owner's icon at 80, screenshotted in Chrome 157.0.8103.38, headless,
without `--disable-gpu`, at `--force-device-scale-factor` 1 and 2, and its
boxes dumped at both. Away from every edge the whole page is within one
level of Chrome's at both scales (53,685 pixels at one, 240,974 at two).
Edges differ as every edge between this engine and Chrome does — Chrome
smooths in quarters of a pixel, this engine by area — by at most 80 levels
at one and 89 at two on the icon's thin arcs, held as measured bounds in
`tests/svg_linear_gradients.rs`.

## Consequences

- The owner's icon can be drawn by alo itself: the application's icon
  (ADR 0067 § 6, item 913) and the mark inside alo.
- `docs/conformance.md` no longer says gradients in SVG are not drawn.
- Item 275 is ticked for its linear half; 910–912 hold the rest, each opened
  by a page that needs it.
