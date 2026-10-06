# ADR 0022 — SVG is a picture we draw, and its insides are not boxes

**Status:** accepted
**Date:** 2026-10-06
**Context:** queue item 107, *SVG*: *"a second rendering model inside the
first, and far larger than its one line here suggests. **Cut this before
starting it**"*; ADR 0001 (rent the physics, build the engine; the DOM, the box
tree, layout and painting are ours); ADR 0002 (one tree, two readers, and roles
are declared rather than inferred); ADR 0005 (strangers' bytes are read in the
renderer, and must not be able to make it do what it was not asked); law 1 and
stage 3's *"let a broken render schedule the work"*; `LOOP.md` stage 2 § 1
(a real page decides) and § 2 (the bytes are hostile); item 176, which gave a
replaced element an intrinsic size and handed paint its pictures by box;
item 20, which gave paint transforms; and the page that needs this today —
**alo's own offline screen**, `alo-workplace/web/public/offline.html`, whose
only picture is an inline `<svg>` of the alo hand: four `<path>`s with arcs,
stroked in terracotta, `fill="none"`, round caps and joins, in a 24-unit
`viewBox` drawn into a 56 px square

## The decision in one line

An `<svg>` in a page is **one replaced box**, sized like a picture; nothing
inside it is a CSS box; what is inside is turned, after layout, into **a list
of filled and stroked paths in our own path vocabulary** by a new crate,
`alo-svg`, and handed to paint by box exactly as a decoded picture is — so SVG
adds a second *drawing* model to the engine without adding a second *tree*,
and it is built in the order alo's offline screen needs, with everything else
SVG has refused until a page asks for it.

## What happens today

`<svg>` is `inline-block` in the user-agent sheet and its role is already
*image* (`alo-box`'s `role.rs`). Everything inside it — every `<path>`,
`<g>`, `<circle>` — is laid out as an ordinary empty inline box. On the
offline screen that is a 56 × 56 hole where the hand should be, and four
zero-sized boxes that mean nothing to anybody. Nothing is wrong enough to
crash, which is why it is easy to leave; it is wrong on alo's own screen,
which is why it is not.

## 1. An `<svg>` is a replaced box; its contents are not boxes

**The outermost `<svg>`** — one whose parent is not an SVG element — is laid
out as a replaced element, through the path item 176 built for `<img>`:

- **Its size** is CSS's when CSS gives one, as on the offline screen. Without
  that, it is the `width` and `height` attributes, which are presentation
  attributes for those two properties (§ 3). Without those, the size falls
  back as SVG 2 and CSS say for a replaced element: a ratio from the
  `viewBox` if there is one, and 300 × 150 if there is nothing at all.
- **Its ratio** comes from the `viewBox`, through the same `taffy` aspect
  ratio item 176 found it had to use.

**Nothing inside it becomes a CSS box.** A `<path>` is not an inline box, a
`<g>` is not a block, and `display` on them means only *drawn or not*
(`display: none` hides an element and what it holds). This is what SVG says,
and it is the shape the engine already has: a text box holds glyphs that are
not boxes, and an `<img>` holds pixels that are not boxes.

**A nested `<svg>`** is not an outermost one. It is drawn as a new viewport
inside its parent's drawing (§ 2), not laid out.

## 2. The drawing is ours, in a crate of its own, built after layout

**A new crate, `alo-svg`**, has one responsibility: from an outermost `<svg>`,
the computed styles of what it holds, and the size layout gave it, to **a
drawing** — an ordered list of shapes, each a path in user space, the
transform that places it, the paint that fills it, and the paint and stroke
that outline it. It depends on `alo-dom`, `alo-style`, `alo-value` and
`alo-paint`, the last only for paint's vocabulary of shape (`path.rs`'s
`Path`, `Point` and `Segment`), so that there is **one** path type
in this engine. Neither `alo-paint` nor `alo-box` depends on it. `alo-renderer`'s
pipeline asks it for each `<svg>` box's drawing, and passes that drawing to paint
**by box id**, beside `PaintContext::pictures`. It is the same seam and for the same reason: choosing
what belongs to a box needs the document, and paint does not have one.

**After layout, not before**, because an SVG length in per cent is a share of
the viewport, the viewport is the box, and the box's size is layout's answer.
The `viewBox` and `preserveAspectRatio` become one transform from user space
to the box; the `transform` attribute and the `transform` property compose
under it. Paint already draws under transforms (item 20), and nothing new is
needed to place the drawing.

**Geometry is ours**: the basic shapes as paths, rounded `rect` corners, and
the path data grammar with its arcs turned into cubic curves (SVG 2's
implementation notes, endpoint to centre parameterisation). These are tens of
lines of specification arithmetic each, and where two engines' outlines
differ.

**Path data is parsed by us** rather than rented. `svgtypes`, the resvg
project's parser, would do it, and it is the closest call in this ADR. It is
not taken for three reasons. The grammar is one page of SVG 2. The two things
around it that matter are ours to get right. One is the error rule: *draw
the path up to the first error, and nothing after it*. The other is the
bound in § 5. And a rented crate costs a boundary in `gate.sh` and a supply
chain, which is a lot to pay for a few hundred lines. ADR 0001's rent list is
shaping, rasterisation, codecs and TLS, and a path grammar is none of those.

**Strokes and dashes are rented** — from `tiny-skia`, which is already rented
for every fill this engine draws, through its stroker and dasher, named in
`alo-paint`'s `raster.rs` and nowhere else. A stroker with joins, caps,
miter limits and curve offsetting is rasterisation physics, of exactly the
kind `raster.rs` exists to rent once. No new crate enters for strokes; the
boundary in `scripts/gate.sh` does not move.

## 3. Style goes through the one cascade

SVG's paint properties — `fill`, `fill-opacity`, `fill-rule`, `stroke` and
the `stroke-*` family, `opacity`, `display`, `visibility`, `color` — are CSS
properties, computed by `alo-style` as every other property is. Selectors
match SVG elements as they match HTML ones. `currentColor` is the computed
`color`, which inherits from the HTML around the `<svg>` — and that is the
point of it: an icon drawn in `currentColor` is the colour of the text beside
it.

**Presentation attributes** (`fill="none"`, `stroke-width="2"`) are read as
SVG 2 says: declarations at the **author** level with **specificity zero**,
ahead of every author rule, so any stylesheet overrides them and the
user-agent sheet does not. They are parsed by `alo-value` like any other
declaration's value. An unknown or invalid one is ignored, as an invalid
declaration is.

**Geometry attributes** (`d`, `x`, `y`, `width`, `height`, `cx`, `cy`, `r`,
`rx`, `ry`, `points`, `x1`…`y2`) are read as attributes. SVG 2 makes some of
them properties too, and that half is built when a page sets one from a
stylesheet.

## 4. What the agent reads

ADR 0002's tree has **one node for an outermost `<svg>`**, and no nodes inside
it:

- **Its role** is *image*, as it is today, unless the author declared another.
- **Its name** is its `aria-label`, then what `aria-labelledby` points at,
  then the text of its first child `<title>`. Those are the names
  `alo-box`'s `semantics.rs` already takes, plus SVG's own `<title>`.
- **An `<svg>` with `aria-hidden="true"`** — the offline screen's, which is
  decoration beside a heading that says the same thing — **is absent**, as
  any `aria-hidden` box already is.
- **An unnamed image** is reported as unnamed. A name is never made up out
  of the shapes, because a guessed name is the thing ADR 0002 refuses.

What is drawn and what the agent reads still come from one tree: the drawing
is the *content* of one box, as a picture's pixels are. An SVG with links or
controls inside it (`<a>`, `tabindex`) would need nodes inside the image. That
is a later item, opened by a page, and it adds nodes to this tree; it does not
build a second one.

## 5. The bytes are hostile, and every count is bounded

Inline SVG reaches us through `html5ever` and the DOM, so its elements are
whatever the DOM already holds. What SVG adds are counts a stranger chooses
inside attribute values. Each is bounded **before** the work it would cause:

- **Path data**: a bound on segments per path, and on segments per drawing.
  Past it the drawing is refused and the box is left empty, as a refused
  picture leaves its box (item 176). It is never truncated silently.
- **Numbers**: every coordinate, length and arc radius is finite, or the
  element is not drawn. Arc radii follow SVG's correction rules (scale up, or
  draw a line). A transform that flattens to nothing draws nothing without
  failing, as `drawn_picture.rs` already does.
- **Stroke widths and dash arrays**: a dash array with more entries, or more
  dashes per path, than a bound is refused rather than dashed. A dasher fed a
  0.0001-unit dash over a long path produces millions of segments, which is
  the same attack as a decompression bomb.
- **Nesting** of nested `<svg>` viewports and `<g>` groups is walked without
  recursion, or with a bound, as the JavaScript engine's parser is.
- **References** (`<use>`, gradients, `href` of any kind) are not built in
  this cut. When they are, they come with a bound on the number of elements
  they expand to and a refusal of cycles, because `<use>` is SVG's billion
  laughs.

Malformed, truncated and adversarial input is tested for every one of these,
and the answer is an error or an empty box, never a panic (`LOOP.md` stage 2
§ 2). The numbers themselves are the building commit's, stated in the code
beside the reason for each, and pinned by a test at each edge.

## 6. The cut, in the order the offline screen needs it

Item 107 is replaced by these, each closed by a reference render, a layout
assertion where something is sized, and a test of hostile input where
something is parsed:

1. **The `<svg>` box** (§§ 1, 4): replaced sizing, no boxes inside, the agent
   node and its name. The offline screen's box is 56 × 56 with nothing inside it,
   and an `<svg>` with only attributes, or only a `viewBox`, has the size
   § 1 gives it. *Depends on 176.*
2. **Shapes, filled** (§§ 2, 3): `alo-svg`, the drawing handed to paint by
   box, `viewBox` and `preserveAspectRatio`, the basic shapes, `<g>`,
   `transform`, `fill`, `fill-rule`, `fill-opacity`, `opacity`,
   presentation attributes in the cascade, and `currentColor`.
3. **Path data** (§§ 2, 5): the grammar, every command, arcs, the
   draw-up-to-the-first-error rule, and its bounds.
4. **Strokes** (§§ 2, 5): width, caps, joins, miter limit, `stroke-opacity`,
   and dashes, through `tiny-skia`'s stroker in `raster.rs`. **This item
   closes on alo's offline screen**, frozen into the corpus as an alo case
   with its hand drawn.

After those four, and **only when a frozen page needs them** (the rule item
179 already follows):

5. **`<defs>`, `<symbol>` and `<use>`**, the icon sprite, with the expansion
   bound and cycle refusal in § 5.
6. **Gradients and patterns as paint**: `linearGradient` and
   `radialGradient` through the gradients paint already draws (item 19), and
   `pattern`.
7. **`<text>` in SVG**: text placed by coordinates, shaped by `alo-text`.
8. **An SVG file as a picture**: `<img src="…svg">` and SVG in
   `background-image`. *Needs ADR.* A standalone SVG file is XML, and XML
   parsing is stage 3's item 139. This one case comes earlier because SVG
   icons ship as files on the modern web. Its ADR decides which parser is
   rented, how it is held to SVG documents only, so that it does not open
   XML for stage 2 by the back door, and the secure static mode browsers
   use for an SVG image: no script, no animation, and no fetch of anything
   outside the file.

## 7. What stays refused

Each of these draws nothing, says so, and is opened by a page and not by a
specification:

- **`clipPath`, `mask`, `filter` and `marker`.** The CSS half of clipping,
  masks and filters is item 96, and the SVG half should share its
  implementation rather than precede it.
- **`<image>` inside SVG**: a picture inside a picture, and a fetch.
- **`<foreignObject>`**: HTML laid out inside SVG inside HTML. It would mean
  boxes inside the drawing, which this ADR exists to avoid until a page shows
  it must be done.
- **SMIL animation** (`<animate>` and its family): refused. CSS animation of
  SVG properties is item 94's when it arrives.
- **SVG fonts**: refused and never queued. Firefox never shipped them and
  Chromium removed them in 2014, so supporting them is the legacy law 1
  refuses.
- **`<script>` inside SVG** does not run. That is true today (`alo-dom`'s
  `scripts.rs`). Running it would be script work, opened by a page, and it
  is not part of drawing SVG.
- **Hit testing and events inside the drawing.** No verb reaches inside an
  image (§ 4), and no verb ever takes a coordinate (ADR 0002).

## What this costs

- **A second drawing model.** Paint now draws a box's drawing as well as its
  box, text and picture. It is contained by the seam: paint draws a list of
  paths, and does not know it came from SVG.
- **Another crate in the paint path.** `alo-svg` is its own crate because it
  is its own responsibility: geometry, viewports and paint servers. Putting it
  in `alo-paint` would give that crate a document to read. Putting it in
  `alo-box` would give box construction a geometry engine.
- **Much of SVG draws nothing for a while.** Each refusal in § 7 leaves the
  rest of the drawing intact, and the agent still reads the `<svg>` by name.
  Pages that use SVG for icons, which is most of them, need §§ 1–4 and often
  5 and 8, and the queue reaches those first.

## Alternatives rejected

**Rent the whole thing: `usvg` and `resvg`.** It is the strongest Rust SVG
stack, it draws with the `tiny-skia` we already rent, and it would draw the
offline screen's hand within a day. It is refused because it is a second
engine inside this one. It has its own tree (`usvg` resolves styles, CSS and
references into a tree it owns), its own cascade, its own text layout, and
its own idea of what a length is. ADR 0001 says the DOM, style, layout and
painting are ours, and ADR 0004 drew the line at "the algorithms are rented,
the tree is ours". `usvg` rents the tree. An icon that ignores the page's
`currentColor`, or takes its fonts from somewhere other than ADR 0010's font
path, is the kind of bug that design produces.

**Lay SVG out as CSS boxes**, which is what happens today by accident.
Rejected because it is not what SVG means. A `<circle>` has no flow, no
margins and no line, and pretending otherwise gives the agent a tree full of
boxes that mean nothing.

**Parse path data with `svgtypes`.** Not rejected on merit; see § 2. If our
parser and `svgtypes` disagree on a frozen page, that is evidence to weigh.

**Write our own stroker.** Rejected by ADR 0001. Offset curves, round joins
and the degenerate cases of a stroker are rasterisation physics, and the
rented rasteriser already has them.

**Turn an `<svg>` into a picture with a rasteriser and treat it as an
`<img>`.** Rejected because a bitmap made at one size is wrong at every other
size and under every transform, and because it loses `currentColor`, which
is a property of the page around it.

## What this does not decide

- **The bounds' values** (§ 5): the commit that builds each, with a reason
  and a test at each edge.
- **Which XML parser an SVG file is read by**, and how an SVG image is kept
  from fetching. That is cut item 8's own ADR.
- **Interaction inside an SVG** (§ 4, § 7).
- **Whether path data is ever also a CSS value** (`d: path()`,
  `clip-path: path()`). If it is, the grammar moves to `alo-value` in that
  change, so that there is one parser and not two.

## How we will know if this was wrong

**If alo's offline screen draws its hand differently from Firefox and
Chromium** in the corpus case that closes cut item 4, by more than
anti-aliasing, the geometry is wrong. That is fixed in our code, not
excused.

**If a frozen page needs `<foreignObject>`** and the box seam cannot hold it,
§ 1's "nothing inside is a box" is reopened by that page. A person weighs it,
because the answer is boxes inside a drawing inside a box.

**If our path parser and `svgtypes` keep disagreeing** on real pages, and
ours is the one that is wrong, renting it is reopened.
