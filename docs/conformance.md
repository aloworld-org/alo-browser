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

An inline box broken around a block is read by an agent as **one thing**: one
node, named by everything the element contains, positioned everywhere it was
drawn, with the block read inside it rather than beside it. Still a view — the
box tree records which boxes belong to which whole and the reader follows it.

**An agent can read a rendered page as a tree of what it is** — roles, names,
states and positions, with no screenshot involved — **and aim a verb at it by
name**: activate, put text, scroll, with no verb taking a coordinate. Every
corpus case pins that tree beside its picture.

**A verb changes the page.** Text put into a field is in it, a checkbox ticks,
and choosing a radio un-chooses the rest of its group. The page is rendered
again from the **same document**, so every id an agent read a moment ago still
names what it named. What a verb cannot do is anything that needs a script: a
button on a page with no JavaScript does nothing when it is pressed, and the
outcome says what was pressed rather than pretending otherwise. Following a
link reports where it goes; navigating is the browser process's.

**A field shows what it holds**, and a password shows one dot a character and
never what. The dots are not in the agent tree at all — assistive technology
never reads a password back and neither does this.

**A form control does not draw its state.** A checkbox that is checked draws
the same box as one that is not; there is no tick and no focus ring. The state
is right in the tree and wrong on the screen, which is the worst way round —
queue item 43.

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
in. Each piece draws its own background. A piece with **nothing** in it is kept, and draws
its border — and costs nothing when it has none, because a line box holding
only empty inline boxes with no border and no padding is zero-height and
treated as not existing.

**An inline box has a box of its own.** A `<span>`'s border and padding are
laid out and drawn — horizontal ones take room on the line, vertical ones draw
without changing its height — and a `<span>` that wraps is one rectangle per
line, with its start border on the first piece and its end border on the last.
A *percentage* padding on an inline box is refused and recorded: it is of the
containing block's width, which is not known where a line is built.

**`letter-spacing`** is applied where text is measured, so it changes where
lines break rather than only how the letters sit.

**`white-space` is processed**: runs of whitespace collapse to one space when
a box is built, `pre-line` keeps the newlines, `pre` and `pre-wrap` keep
everything, and `pre` and `nowrap` refuse to wrap. Before this the engine did
none of it and drew markup's own indentation.

`clamp()`, `min()` and `max()` are read, nest in each other and in `calc()`,
and are type-checked once when they are parsed. The **viewport units** `vw`,
`vh`, `vmin` and `vmax` resolve against the window the page is being rendered
at — and answer zero, rather than a plausible number, when a value is resolved
without one.

`calc()` resolves everywhere, percentages included:
`width: calc(100% - 2rem)` is a number rather than a refusal, in widths and
heights, minimums and maximums, margins, padding, insets, gaps and grid tracks.
The layout **tree** is ours and the layout **algorithms** are `taffy`'s, which
is what makes that possible (ADR 0004). A `calc()` inside `fit-content()` is
still refused and recorded.

What does not exist: any transform with a third dimension in it — `rotate3d`,
`matrix3d`, `perspective` — which is refused rather than flattened. A border
with four different widths still turns its inner corner squarer than CSS draws
it. A blur under a non-uniform scale or a skew is softened by the average of
the two axes, because a blur radius is one number. Most targets below are still
`not yet`, because they are alo's own screens rather than pages we wrote to test
with — and the sign-in screen, which is alo's, is *nearly* rather than done: the
four substitutions in its case are four things this engine has yet to implement.

This file exists instead of a conformance percentage. A Web Platform Tests score
would grade us against thirty years of legacy we are deliberately refusing
(`docs/decisions/0001`), so it would measure the wrong thing and flatter or
punish us for the wrong reasons.

The measure is alo. These are the targets, in order — and each is **markup and
CSS that exists today**, not a screen waiting on a compositor. A target is the
document, not the operating system that will eventually show it, so every row
here can move on an ordinary laptop. An alo screen is alo's whichever repository
it lives in.

| Target | State |
|---|---|
| alo sign-in screen | **nearly** — `alo-workplace`'s renders and is diffed on every run, with four substitutions still in the case (see below). Not correct until those are gone |
| alo Settings | not yet — not rendered at all |
| alo agent overlay | not yet |
| An agent reading Settings as a tree and activating a row by name | reading and activating both work on pages we wrote; Settings itself is not yet rendered |

Colours are correct when they match `alo-workplace`'s `web/src/ds/tokens.css`,
which is the specification for what "correct" means here.

## The one screen that is alo's

`crates/alo-corpus/cases/alo-sign-in/` is **alo-workplace's own sign-in
screen** — its markup, its rules from `web/src/auth/LoginPage.module.css`, and
its colours from `web/src/ds/tokens.css` — rendered by this engine and diffed on
every run. **One** substitution is written into the case's own stylesheet: `transition`,
`:hover` and `:focus-visible`, dropped because there is nothing to animate and
no input to respond to. Three are gone — the headline's
`clamp(2.4rem, 4vw, 3.5rem)`, its `white-space: pre-line` (so the markup is one
string with newlines in it as `alo-workplace` writes it), and its
`letter-spacing`.

The headline still wraps one line more than the real screen does, and that is a
**font** difference rather than an engine one: the corpus renders in DejaVu
Sans, which is wider than the Inter the app loads. Web fonts are stage 2.

It is a real alo screen, and it is one of the screens `ROADMAP.md`'s exit gate
names — the gate used to name `alo-os`'s specifically, which was a fact about
repository layout rather than about this engine, and it has been corrected.

**Stage 1's exit gate is still not met**, for two reasons that are about the
engine: those four substitutions mean what is diffed is a modified screen, and
Settings is not rendered at all. Neither reason is hardware, and neither is
another repository — so both are this loop's to close.

## The corpus

`crates/alo-corpus/cases/` holds the small cases this engine is checked against
on every run — fourteen of them today. Each is a directory with what to render
and five expectations beside it, so a change that moves a box says which box, in
which case, on which line.

That is not the same as the table above. The corpus is pages we wrote to test
with; the table is alo's own screens, which is the measure that matters.

## How a target becomes correct

A reference render is committed alongside its expected box tree. A target is
correct when both match and the numbers are asserted — not when somebody looked
at an image and thought it seemed right.
