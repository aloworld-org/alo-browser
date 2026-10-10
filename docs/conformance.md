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
interface is asserted as numbers rather than looked at. The `place-*`
shorthands split into their `align-*` and `justify-*` pair; of the values,
the engine reads the single keywords, so `safe center` reaches the longhand
and is refused there. Text straight inside a flex or grid container is
wrapped in an anonymous block item, one per run of text, as CSS says, so its
lines take their `line-height` like any other block's; a run that is only
whitespace is no item.

Text is real. Fonts load, text is shaped — including Arabic, which joins and
runs right to left — lines break by UAX #14, and a paragraph in a narrow window
takes more lines than the same paragraph in a wide one.

`ex` and `ch` are the face's own: the height of its `x` and the advance of
its `0`, in the first font the element's `font-family` finds, at its weight
and size — which is why a form field, 20 `ch` wide by the user-agent sheet, is
now as wide as twenty of the page's zeros. A font size written in them is
the parent's; a face with no `x` or no `0`, or a page drawn with no fonts at
all, is half an em for that unit, as CSS says. The `font` shorthand is not
expanded, so a page that sets its font only through it keeps the inherited
one.

Lines are real too: text wraps between words across inline boxes, everything on
a line sits on one baseline, and a link broken over two lines is two rectangles.
`text-align` moves a whole line — text, images and inline-blocks together —
including a line in an anonymous block, which inherits its container's value.
`justify` is not implemented and is read as `start`; right-to-left `start`
and `end` wait on writing modes (item 98). An image or inline-block sits on
the line by its **margin box**: its margins take room across the line and
make the line taller. Every line starts with a **strut** — its container's
font's ascent and descent — so a line of only a picture has the font's
descent below it, as in other browsers. **`line-height`** sets how much room
text, an inline box and the strut take on a line: half of the difference
from the font's ascent and descent goes above the letters and half below,
and a value smaller than the font is a negative leading. What is drawn is
still the font's height. `normal` is the font's ascent and descent and
nothing more. That is right for the faces the corpus uses, whose tables ask
for no gap between lines; a face whose table asks for one is laid out
without it. A value that is negative, not finite or cannot be read is
`normal`. One that is merely enormous makes a line that tall, as an
enormous `height` makes a box that tall, and past the range of a float
both reach geometry that is infinite; the page is still drawn. An atomic box stands on the
baseline of its **last line** — a button on its label's, a text field on its
value's, an inline-block on its last line of text — and an `<input>` with
nothing typed in it on the line it would hold, its strut's baseline, so it
stands where it will once typed into (item 384; an empty button and
`<textarea>` do not) — and on its bottom margin edge when it has no line or its `overflow` is not `visible`, which is where an
image, an SVG and a checkbox stand. The line is found through the blocks
inside the box, last first; a flex or grid container, or a scroll container,
met on the way is **refused**, and the box stands on its bottom margin edge
instead — a flex or grid container's baseline is its items', which is not
worked out yet. An author's `inline-block` holding blocks is broken around
them by the box tree as if it were a `<span>` (item 286). A percentage width on an
inline-block is drawn against the room the line gave it rather than its
container (item 284).

**`vertical-align`** moves an image, an inline-block or an inline box, and
everything inside it, from its parent's baseline (item 312). Every keyword
is read: `baseline`, `middle` (half the parent's x-height over its
baseline), `text-top`, `text-bottom`, `sub` and `super` (Chromium's
distances, a fifth of the font size and a pixel down, a third and a pixel
up), `top` and `bottom` (against the line box, after everything else has
said how tall it is), a length, and a percentage of the box's own
`line-height`. What moved makes its line taller. An inline box is aligned
by the room its `line-height` gives it, leading and all, as CSS says. A face whose table has no x-height, DejaVu among
them, is measured by its own `x`, as browsers do.

**Media queries** answer `min-width`, `max-width`, `width` and
`prefers-color-scheme`, joined by `and`, after an optional `not` or `only`
and a media type (`screen` and `all` match, `print` and the rest do not).
A width is written in `px`, `em` or `rem`, or is a bare zero. `em` and
`rem` are the initial font size, 16 px, not the page's, because Media
Queries says a query's relative units are never taken from a declaration
(item 315): Meet's `(max-width: 48rem)` applies at 768 px and below. Every
other feature, unit and the range syntax (`width >= 600px`) is recorded
and the query treated as `not all`.

Colours are channels, `currentColor` included, so the engine now knows what
colour everything is. `color-mix()` is read in sRGB, the one space it can mix
in without converting (item 377); in any other space it is refused.

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

**A form control holds what it shows in a box of its own**, the way browsers
do: a tall button's label sits in the middle of it, and an empty field is still
one line tall. It is a box in the tree rather than a rule in the user-agent
sheet, and it has to be — a rule that centred a button's label would also
centre the children of a button an author had made a flex container, and an
author cannot override a rule they cannot see.

**A form control draws its state.** A checked checkbox draws a tick, a chosen
radio a dot, and a checkbox that is neither on nor off — `aria-checked="mixed"`,
the "select all" box above a half-selected list — a dash. The colour is
`accent-color` if the page names one, and the mark on top of it is black or
white by whichever shows up against it. The mark is drawn by the engine rather
than set in the user-agent sheet, for the same reason a control's inner box is:
CSS has no way to say "and draw a check inside it".

**A control nobody can operate says so, and still says what state it is in.** A
disabled control draws its mark in grey rather than the accent, and its border
pales — so on, off, on-and-locked and off-and-locked are four different
pictures. That much is ordinary colour and does live in the user-agent sheet,
where a page can override it.

**A group of controls looks like a group.** A `<fieldset>` draws a border, and
its `<legend>` sits **in** that border rather than above it: the block-start
border is drawn in the two pieces the legend leaves, which is what writes the
group's name into the line around it. The band the legend sits in *replaces* the
block-start border rather than adding to it, so a fieldset is exactly as tall as
a browser draws one. The border is a `groove` of `#c0c0c0`, as every other
browser gives a fieldset: the mitred two-toned sides, with the legend's part of
the block-start line cut out of them, across the line's depth and no further
out than the side borders — so a legend wider than the fieldset leaves its
corners drawn (corpus cases `fieldset-group` and `web-a-form`). Any other
style is cut the same way: a `dashed`, `dotted` or `double` block-start side
is laid out along the whole side, exactly as it would be with no legend, and
then cut where the legend is, so a dash or dot the legend's edge falls on is
cut there rather than moved (corpus case `fieldset-patterns`). A rounded
corner on a fieldset showing a legend is still drawn square.

**Two-toned borders.** `inset`, `outset`, `groove` and `ridge` are drawn in a
darker and a lighter tone of the border's colour, lit from the top left, with
each side mitred so a corner splits where the tones change and a groove's
halves follow a rounded corner's curve (corpus case `border-styles`). The
exact tones are ours, which CSS allows.
Where two different tones meet on a corner's diagonal, the page shows faintly
through that one line of anti-aliased pixels.

**Patterned borders.** `dashed`, `dotted` and `double` are drawn (corpus case
`border-patterns`): dashes about three widths long with equal gaps, and round
dots a width across, each spaced so a side starts and ends on one; `double` as
two lines a third of the width each, both following a rounded corner. The
spacing is ours, which CSS allows. Along a rounded corner a dash or a dot is
placed on the straight side and clipped by the curve rather than spaced along
it, and where sides of different widths meet, a corner dot is clipped to its
own side's share of the corner. A side is drawn with at most 16 384 dashes or
dots; past that they are spaced further apart. Beside a fieldset's legend
each is cut where the legend is (above).

**There is still no focus ring**, which is the rest of queue item 43: a focused
field looks exactly like an unfocused one. Nothing in this stage has focus to
draw, and the day something does, this is what it needs.

A box can cast a shadow — offset, blurred, spread, and `inset` — and be filled
with a `linear-gradient` or a `radial-gradient`; text casts a shadow too.
Refused rather than approximated: `conic-gradient`, the repeating gradients,
interpolation hints, and any colour space but sRGB.

**A background can be several layers** (item 313): a comma list in
`background` or `background-image`, the colour beneath them all and the
first layer written on top, each measured against the padding box and
drawn over the border box. A radial gradient may be a `circle` or an
`ellipse`, sized by `closest-side`, `closest-corner`, `farthest-side` or
`farthest-corner`, and centred `at` one or two keywords, percentages or
pixel lengths. Stops are mixed with alpha premultiplied, as CSS says, so a
fade to `transparent` does not pass through grey. What is not drawn is
recorded in the page's issues: a `url()` layer (item 311), with every other
layer still drawn, and a list that cannot be read, which draws nothing. A
list cannot yet be read if any layer says where its picture sits, how big
it is, whether it repeats, or which box it is clipped to, if a radial size
is written as lengths, if a position has three or four parts or is in a
unit other than pixels, or if it uses an image function other than the two
gradients. `background-color` beats the shorthand's colour and
`background-image` beats its pictures, whichever came later in the sheet,
because the cascade does not yet expand `background`.

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

**An absolutely positioned box is blockified.** An inline with `position:
absolute` is block-level and in no line, and is placed at its offsets,
shrunk to what it holds (`cases/absolute-inline`). Two things are not right
yet, for an absolutely positioned block as much as an inline. One among a
line's content splits the line in two rather than taking no room on it, and
one with an inset left `auto` is not at its static position (item 354). It
is also placed against its parent rather than its nearest positioned
ancestor (item 355).

**`letter-spacing`** is applied where text is measured, so it changes where
lines break rather than only how the letters sit.

**`white-space` is processed**: runs of whitespace collapse to one space when
a box is built, `pre-line` keeps the newlines, `pre` and `pre-wrap` keep
everything, and `pre` and `nowrap` refuse to wrap. Before this the engine did
none of it and drew markup's own indentation.

**A `<br>` ends its line**, as HTML's rendering section says, even under
`nowrap`. Two in a row leave a blank line, one at the end of a paragraph adds
nothing, and a line that starts after one does not start with a space. Until
alo's download page showed it, a `<br>` was an empty inline box and ended
nothing. One difference from other browsers is known: a `<br>` an author has
given another `display`, such as `block` or `inline-block`, is laid out as
that box, empty, rather than kept a break as browsers keep it. No page has
been seen to do it.

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
the two axes, because a blur radius is one number. A picture under a
rotation, skew or mirror is drawn turned with its box and its outline is
smooth, but it is sampled nearest-neighbour, so the stripes *inside* a turned
or scaled picture have stepped edges; and a picture ignores a clip in force,
so one inside `overflow: hidden` is drawn whole. A picture is read as PNG,
JPEG, GIF or WebP. An animated GIF or WebP is drawn as its first frame and
does not move. AVIF is refused and the `<img>` keeps the box its style asked
for, as with any picture that did not arrive. An inline `<svg>` is one box
of the right size — from CSS, then its `width` and `height`, then its
`viewBox` ratio, then 300 × 150 — and an agent reads it as one named image.
Inside it, **filled basic shapes are drawn** — rectangles (rounded too),
circles, ellipses, polygons and polylines, in groups, under the `transform`
attribute, a `viewBox` and `preserveAspectRatio`, by `fill`, `fill-rule`,
`fill-opacity` and `opacity`, and in `currentColor` (corpus case
`svg-shapes-filled`). **A `<path>` is drawn filled** — every command of its
data, arcs included, up to its first error (corpus case `svg-path-data`).
**Strokes are drawn** — `stroke` and every `stroke-*` property: width, the
three caps, the three joins, the miter limit, `stroke-opacity`, and dashes with
their offset, a stroke squashed with its shape under a transform (corpus case
`svg-strokes`) — so alo's offline screen draws its hand (`alo-offline`).
SVG 2's `miter-clip` and `arcs` joins are not understood, as in every browser,
and `paint-order` and `vector-effect` are reported and not applied. A nested
`<svg>`, `<use>`, `<text>` and gradients are not drawn and are reported. **The
`transform` property on an SVG element is applied** — a stylesheet's replaces
the element's `transform` attribute, a child's composes inside its group's,
and `transform-box: fill-box` with `transform-origin` turns a shape about its
own box (corpus case `svg-transform-property`); `fill-box` on a `<g>` and
`stroke-box` are reported and measured as near as they can be (item 288).
**A `width` or `height` attribute in per cent or `em`
sizes its `<svg>`** through the cascade, beaten by any stylesheet rule (corpus
case `svg-relative-size`); a per cent on an inline-level `<svg>` is resolved
twice, as on any inline-block (item 284), and is right on a block-level one.
**A `style` attribute is applied** on an HTML or SVG element, as CSS
Cascade 4 places it: above every declaration a selector reaches, below any
`!important` one in a sheet, and an important one above an important sheet
declaration whatever its selector (item 341, corpus case
`style-attributes`). **A script's `element.style` reads and writes it**
on an HTML or SVG element (item 342): `cssText`, `length`, `item()`,
`getPropertyValue`, `getPropertyPriority`, `setProperty`,
`removeProperty`, `parentRule`, and a camel-cased and a dashed accessor
for each property this engine acts on and for no other, so
`'cursor' in el.style` is `false` and a write to it sets an ordinary
property of the object (`alo-downloads` greys its buttons this way). A
value is kept as written: `#c7bfb2` reads back as `#c7bfb2`, where other
engines say `rgb(199, 191, 178)`. `backgroundColor` after `background:
red` reads `""`, where they answer the colour. `length` counts what was
written, so `margin: 0` is one declaration where they count its four
sides. A declaration left unclosed at the very end of a `style`
attribute — a string or a bracket never closed — is left out when a script
first writes the attribute, where other engines would close it.
`el.style[0]` is `undefined` (item 345): `item(0)` answers the name. A
MathML element has no `style`, since this engine builds no MathML. A page's
`style-src` is applied to its inline style at every draw, by every policy the
page holds (item 343, ADR 0034), and that differs from other engines in two
ways, both refusing where they apply: a `<meta>` policy refuses inline style
written *before* it in the markup, which they had already applied; and a
`setAttribute("style", …)` the policy refuses takes the element's old style
away, where they keep it. What a redraw after load objects to is reported with
the next act or delivery, once per element, placement and text for the page's
life (item 346). Other engines report when the value arrives; here it is when
the page is next drawn and an answer can carry it, so a style set and removed
inside one task, never drawn, is not reported.
Most targets below are still
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
| alo sign-in screen | **yes** — `alo-workplace`'s, from its own stylesheet with no substitutions, diffed against a committed image *and* a committed box tree on every run |
| alo Settings | **yes** — `alo-workplace`'s, likewise, and its narrow-screen `@media` block evaluated rather than assumed away |
| An agent reading Settings as a tree and activating a row by name | **yes** — `crates/alo-renderer/tests/an_agent_on_settings.rs`, against that same screen, by name and never by position |
| alo agent overlay | not yet — the screen is not written in `alo-workplace` either |
| alo offline screen | **nearly** — `alo-workplace`'s `offline.html`, frozen byte for byte as `alo-offline`, its hand drawn (item 273), it and the button centred across (item 280) and the hand's bottom margin kept (item 281), the whole screen centred down the window by `place-items` (item 282), the font's descent under the hand (item 283), and the button standing on its label's baseline so its line is the button's height (item 285) |
| A page alo Sites publishes | **five sections** — `crates/alo-corpus/cases/alo-sites-cta`, its call-to-action section frozen with its `site.css` (item 349), is drawn with its sheet and checked in numbers by `tests/alo_sites_cta.rs`. Its skip link, `position: absolute` on an inline, is out of flow and off the page, so the section starts at the top (item 352). Its analytics script runs past `Date.now()` (items 353 and 356, ADR 0036) and past `encodeURIComponent(location.pathname)` on its eighth line (items 359 and 360), and past `window.addEventListener` on its thirty-second (item 362, ADR 0037), and runs to its end. It is told when it is hidden (item 364, ADR 0039 § 1): hidden, its own `visibilitychange` listener reports `d=1000&p=%2F&w=800` and then `t=0` through a beacon a test lends, and shown and hidden again it sends nothing more. It is left as other browsers leave it (item 373, ADR 0039 § 2): its own `pagehide` listener, fired by the browser, reports `d=1000&p=%2F&w=800` and then `t=0` through a beacon a test lends, its `visibilitychange` listener hearing `hidden` next sends nothing more, and what it sent leaves the renderer as fetches the browser refuses by name, since a `fetch` does not outlive its page. With **no beacon lent** it reports through the engine's own `navigator.sendBeacon` (item 369, ADR 0040): left, its two reports are beacons the browser process decides as the document's and — from this page's markup served on this machine, in `alo-renderer`'s `a_beacon_outlives_its_page.rs` — makes after the page has gone, each a `POST` of `text/plain;charset=UTF-8` recorded as a beacon; hidden, it sends the same two while it is held, and their answers free its 64 KiB for a third. It reads the window's size and scroll position truly, 800 × 600 at the top (item 366, ADR 0038), and the page's own size as the renderer measures it when asked: the root 800 × 600, `body` 253 tall, the skip link off to the left not counted (item 370). Given a beacon and a `pagehide` by a test, it reports a depth of `d=1000&p=%2F&w=800` and then `t=0`; given a click with coordinates, `x=0&y=0&p=%2F&w=800`. None of this changes anything drawn. In the window its first frame is held until its sheet has answered, and is painted **styled** (item 351, ADR 0041); a sheet slower than three seconds would have it shown first and said so. Its subscriptions section, `crates/alo-corpus/cases/alo-sites-pricing` (iteration 247), is drawn with the same sheet and checked in numbers by `tests/alo_sites_pricing.rs`: its one tier fills the `auto-fit` grid, is lifted 12 px by its transform, and casts its shadow, whose colour is `color-mix(in srgb, var(--text) 12%, transparent)` — refused, and the shadow with it, until item 377 read a mix in sRGB. A mix in any other space, or of `currentColor` (item 378), is still refused. Its features section, `crates/alo-corpus/cases/alo-sites-features` (iteration 248), is laid out with the same sheet and checked in numbers by `tests/alo_sites_features.rs`: its bento grid's first card spans both tracks and is padded 2.5rem above and below by `padding-block`, which was dropped until item 379 split the block-axis logical shorthands (`cases/block-axis-spacing` shows the rule in a picture). **Its picture is white**: the page's script hides every section that moves into view until an `IntersectionObserver` reports it, and this engine has none (item 381), so the section is drawn at opacity 0 — what the page does in any browser without the interface. Its testimonials section, `crates/alo-corpus/cases/alo-sites-testimonials` (iteration 249), is drawn with the same sheet and checked in numbers and pixels by `tests/alo_sites_testimonials.rs`: its featured quote is `font-style: italic`, the corpus has no slanted face, and the upright face is leaned 14° as a browser would — drawn upright until item 382. `font-synthesis: none` and a synthesised bold are not done (383). Its booking section, `crates/alo-corpus/cases/alo-sites-booking` (iteration 250), is drawn with the same sheet and checked in numbers by `tests/alo_sites_booking.rs`: its day field, empty, stands where its text will, so its line is the field's own 48.4 and the form, centred down the grid, is 162 tall at 89.4 — until item 384 the field stood on its bottom edge with the font's descent under it. The field draws how a date is written, `yyyy-mm-dd`, in its own colour (item 385, ADR 0042), checked in pixels there, and an agent reads it as `date "Choose a day" [required]` with no value. A browser in the United States draws `mm/dd/yyyy` and one in Britain `dd/mm/yyyy`; this draws ISO 8601's order until the person chooses a region (387), and **no calendar button**, which waits until one opens (386). The FAQ section lays its questions out in two columns, which waits on multi-column (97). The other sections are not frozen; the hero's picture cannot be yet (item 350) |

**One thing is true of both screens and is not a defect in either**: the corpus
renders in DejaVu Sans, and the app loads Inter. Inter is narrower, so alo's
headline wraps one line more here than it does in the app. The reference render
is a diff against itself and passes; a person holding the two side by side would
see it. Web fonts are stage 2.

Colours are correct when they match `alo-workplace`'s `web/src/ds/tokens.css`,
which is the specification for what "correct" means here.

## The two screens that are alo's

`crates/alo-corpus/cases/alo-sign-in/` is **alo-workplace's own sign-in
screen** — its markup, its rules from `web/src/auth/LoginPage.module.css`, and
its colours from `web/src/ds/tokens.css` — rendered by this engine and diffed on
every run. **No substitutions.** It is the screen's own stylesheet, rule for rule. Its
`transition`s and its `:hover` and `:focus-visible` rules are there and change
nothing, which is correct rather than missing: a still picture of a settled page
is what a transition has finished doing, and nothing is hovered because there is
no pointer.

Two things about the case are still transcriptions rather than substitutions,
and both are noted in its own stylesheet: the design tokens are declared inline
because `tokens.css` lives in another repository this one only reads, and
Tailwind's preflight is one `box-sizing` rule rather than the whole of it.

The headline still wraps one line more than the real screen does, and that is a
**font** difference rather than an engine one: the corpus renders in DejaVu
Sans, which is wider than the Inter the app loads. Web fonts are stage 2.

**The email field's placeholder is not drawn.** The real screen shows
`you@company.eu` in it, in `--text-tertiary`, through alo's `Input` component's
`placeholder:text-tertiary` — a `::placeholder` rule that the case's stylesheet
does not carry, and that this engine would not match if it did, because it
styles no pseudo-element yet. That is item 389, decided by ADR 0043 and not
built: the case gains that rule when the hint is drawn.

`crates/alo-corpus/cases/alo-settings/` is **alo-workplace's own Settings
screen** — its markup, its rules from `web/src/shell/SettingsModal.module.css`
and the `ds/Modal` shell it sits in, and its colours from the same `tokens.css`.
**No substitutions** there either: its `transition`s, its `:hover` and
`:focus-within` rules and its narrow-screen `@media` block are all present, the
last of them evaluated rather than assumed. The same two transcriptions apply —
the tokens are declared inline, and `ds/Modal`'s Tailwind utilities are written
out as the plain rules they generate.

Building it found two engine defects, both now fixed and both the same root: a
form control needs a box of its own to hold what it shows. See the paragraph
above.

They are real alo screens, and they are the screens `ROADMAP.md`'s exit gate
names — the gate used to name `alo-os`'s specifically, which was a fact about
repository layout rather than about this engine, and it has been corrected.

**Stage 1's exit gate is still not met**, for two reasons that are about the
engine: those four substitutions mean what is diffed is a modified screen, and
Settings is not rendered at all. Neither reason is hardware, and neither is
another repository — so both are this loop's to close.

## The corpus

`crates/alo-corpus/cases/` holds the small cases this engine is checked against
on every run — forty-three of them today. A case whose page carries script is
loaded the way a page is, so its script runs — at its own end tag, against the
page read so far — and the expectations are the page it left
(`a-script-grows-a-list`, `a-script-beside-itself`,
`a-script-gives-a-new-body`, and `a-script-hears-an-event`, whose script
writes what its listeners heard into the page); every other case is its
markup, rendered. Each is a directory with what to render
and five expectations beside it, so a change that moves a box says which box, in
which case, on which line.

That is not the same as the table above. The corpus is pages we wrote to test
with; the table is alo's own screens, which is the measure that matters.

## How a target becomes correct

A reference render is committed alongside its expected box tree. A target is
correct when both match and the numbers are asserted — not when somebody looked
at an image and thought it seemed right.
