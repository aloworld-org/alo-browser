# What renders correctly today

Honest state, updated by the loop as items land. **Nothing renders yet.**

What exists: a document tree and a style sheet. HTML parses into the tree, it
round trips back to the same text, and malformed input produces a usable tree
with a record of what had to be repaired. CSS parses into rules we hold, and a
selector can be matched against the tree — so the engine can say *which rules
apply to which element*, on a given viewport width and colour scheme.

The cascade runs: every element of a document gets the style it should have,
with inheritance and `var()` resolved, on a given viewport width and colour
scheme. A design system defined once on `:root` reaches the whole document,
which is the thing alo is made of.

Boxes are built, and each one carries what it means — its role, its state and
what it is called — so an interface can already be read as a tree of what it
*is*. An agent could find the selected row of a list by asking what the rows
are, which is the whole argument of `docs/decisions/0002`.

Lengths are numbers: `16px` is sixteen, `2em` is twice whatever font is in
force, and `calc(50% - 10px)` is an expression waiting for a basis that only
layout can give it.

Boxes are laid out. Block, flexbox and grid all work, with the box model,
positioning, overflow and percentages, and the whole layout of a small
interface is asserted as numbers rather than looked at.

Text is real. Fonts load, text is shaped — including Arabic, which joins and
runs right to left — lines break by UAX #14, and a paragraph in a narrow window
takes more lines than the same paragraph in a wide one.

Lines are real too: text wraps between words across inline boxes, everything on
a line sits on one baseline, and a link broken over two lines is two rectangles.

Colours are channels, `currentColor` included, so the engine now knows what
colour everything is.

**The engine draws a page.** HTML and CSS in, a PNG out, with real fonts —
backgrounds, borders, and anti-aliased text. The first reference render is
committed at `crates/alo-paint/tests/references/invoices.png` and is diffed on
every run.

Boxes can be round, and clip what is inside them to their own shape.

**An agent can read a rendered page as a tree of what it is** — roles, names,
states and positions, with no screenshot involved — **and act on it by name**:
activate, put text, scroll, with no verb taking a coordinate. Every corpus case
pins that tree beside its picture.

A box can cast a shadow — offset, blurred, spread, and `inset` — and be filled
with a `linear-gradient` or a `radial-gradient`; text casts a shadow too.
Refused rather than approximated: `conic-gradient`, the repeating gradients,
interpolation hints, and any colour space but sRGB.

A box can be moved, scaled, turned and slanted by `transform`, about a
`transform-origin`, and faded by `opacity` — as a group, drawn once and
composited once, which is what `opacity` means. A transform changes what is
*drawn* and not what is laid out, so an agent goes on reading positions out of
the layout tree. Paint order follows stacking contexts: a positioned box is
painted last in the context it belongs to, and a negative `z-index` goes behind
its parent's content and in front of its background.

An inline box holding a block-level box is **broken around it**, into a piece
on each side, with the block a sibling of the anonymous blocks those pieces sit
in. Each piece draws its own background. Two things about that are not right
yet and both are recorded on the tree: a piece with *nothing* in it is dropped
where CSS keeps it, and an inline box's own border and padding are neither laid
out nor drawn.

What does not exist: any transform with a third dimension in it — `rotate3d`,
`matrix3d`, `perspective` — which is refused rather than flattened. A border
with four different widths still turns its inner corner squarer than CSS draws
it. A blur under a non-uniform scale or a skew is softened by the average of
the two axes, because a blur radius is one number. The targets below are
still `not yet`, because they are alo's own screens rather than pages we wrote
to test with.

This file exists instead of a conformance percentage. A Web Platform Tests score
would grade us against thirty years of legacy we are deliberately refusing
(`docs/decisions/0001`), so it would measure the wrong thing and flatter or
punish us for the wrong reasons.

The measure is alo. These are the targets, in order:

| Target | State |
|---|---|
| `alo-os` sign-in screen | **not yet** — `alo-os` is not checked out beside this repository, so its markup has never been rendered. `alo-workplace`'s sign-in screen is, and is in the corpus |
| `alo-os` Settings | not yet |
| `alo-os` agent overlay | not yet |
| An agent reading Settings as a tree and activating a row by name | reading and activating both work on pages we wrote; Settings itself is not yet rendered |

Colours are correct when they match `alo-workplace`'s `web/src/ds/tokens.css`,
which is the specification for what "correct" means here.

## The one screen that is alo's

`crates/alo-corpus/cases/alo-sign-in/` is **alo-workplace's own sign-in
screen** — its markup, its rules from `web/src/auth/LoginPage.module.css`, and
its colours from `web/src/ds/tokens.css` — rendered by this engine and diffed on
every run. Four substitutions are written into the case's own stylesheet, each
naming a thing this engine does not implement: `clamp()` and viewport units,
`white-space: pre-line`, `letter-spacing`, and transitions.

It is a real alo screen. It is **not** the screen `ROADMAP.md`'s exit gate
names, which is `alo-os`'s, and it has never been seen on the certified machine.
Stage 1's exit gate is not met.

## The corpus

`crates/alo-corpus/cases/` holds the small cases this engine is checked against
on every run — six of them today. Each is a directory with what to render and
four expectations beside it, so a change that moves a box says which box, in
which case, on which line.

That is not the same as the table above. The corpus is pages we wrote to test
with; the table is alo's own screens, which is the measure that matters.

## How a target becomes correct

A reference render is committed alongside its expected box tree. A target is
correct when both match and the numbers are asserted — not when somebody looked
at an image and thought it seemed right.
