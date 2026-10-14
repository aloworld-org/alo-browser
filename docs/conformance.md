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
all, is half an em for that unit, as CSS says. The `font` shorthand is
expanded only when it is a CSS-wide keyword alone (`font: inherit` is every
font longhand and `line-height` inherited); any other value is not, so a
page that sets its font only through it keeps whatever else set it (queue
item 513). A form control is set in Chrome's own 13.33 px Arial, or
`monospace` for a textarea and a date or time field, whatever the page's
text is, as Chrome 154 computes it (queue item 506). A face is set at its
font size cut to hundredths, in a 32-bit float, as Chrome 154 sets it
(queue item 514): `13.333333px` is 13.33 and `8.11px` is 8.1. A run's
advances, the room a line takes, `ch`, `ex` and a field's width are all
read at that size, and `em` is not. Of 2,922 runs of text measured at
fractional sizes in three faces, 2,916 are Chrome's width and all are
Chrome's height. The six that are not lie within 0.03 of a unit of a grid
line, a sum this engine's shaper ends a hair on the other side of from
Chrome's (queue item 527).

Lines are real too: text wraps between words across inline boxes, everything on
a line sits on one baseline, and a link broken over two lines is two rectangles.
`text-align` moves a whole line — text, images and inline-blocks together —
including a line in an anonymous block, which inherits its container's value.
`justify` is not implemented and is read as `start`; right-to-left `start`
and `end` wait on writing modes (item 98). An image or inline-block sits on
the line by its **margin box**: its margins take room across the line and
make the line taller. Every line starts with a **strut** — its container's
font's ascent and descent — so a line of only a picture has the font's
descent below it, as in other browsers. **A line is whole pixels of font**,
as Chrome 154's are (ADR 0050, item 460): a font's ascent, descent and line
gap are each rounded half up, so 16 px `DejaVu Sans` is 15 above its
baseline and 4 below where its exact figures are 14.85 and 3.77.
**`line-height`** sets how much room text, an inline box and the strut take
on a line, placed on Chrome's grid of 1/64 px: a number is floored from the
font size rounded to 1/64 (16 px at 1.23 is 19.671875), and a length or
percentage is rounded. What the room has beyond the font is the leading, and
its top half, floored to a whole pixel, goes above the letters, the rest
below; a value smaller than the font is a negative leading, floored the same
way. What is drawn is still the font's height. `normal` is the font's
ascent, descent and line gap; the corpus's faces have no gap. Of the 1,214
element boxes in the corpus measured against Chrome with the same fonts,
835 now have Chrome's top and height to 1/64 px, where 498 did. A fallback
font on a `normal` line does not yet add its own room (465). A control's
own line is the line its text would be (467), where it was 1.2 em: a
field or a button is its font's whole pixels inside its padding, as
Chrome's is, so `a-filled-form` and alo Settings are Chrome's to the unit.
A button's and a field's border are Chrome's 2 px (505), drawn as Chrome's
own picture of the control while the page leaves the border and background
alone, so `a-script-hears-an-event` is Chrome's box for box. A control's
font is Chrome's own 13.33 px (506), so `web-a-form`'s fields are Chrome's
21 tall, where the page's 16 px made them 25, and every box down to its
fieldsets stands where Chrome's does. A text field is as many
characters wide as its `size` says, counted as Chrome counts them (511):
the face's `x`, rounded only when that makes it wider, and its ascent
less one of them. So `web-a-form`'s fields are Chrome's 172 and its
labels Chrome's to the unit, and `a-text-field`'s nine fields, of other
sizes, types and faces, are Chrome's box for box. A face with no `x` is
counted in its `0`, which is near Chrome's figure and not it (526). A date or time field holds its line in segments a
pixel inside it above and below, as Chrome's does (507), so it is 2 px
taller than a text field with the same line. An unstyled one is still a
third of a pixel short of Chrome's, whose calendar indicator sets a floor
(515), and its width is `20ch` where Chrome's is what it draws (517). A
`<textarea>` draws a field's picture in its one solid pixel, as Chrome's
does (510): its sides are Chrome's screenshot's exactly in `web-a-form`
and `a-text-area`. Its box is still `20ch` by `3em`, where Chrome's is
`cols` and `rows` (523), so `web-a-form`'s is 10 px taller than
Chrome's, and Chrome's resize grip is not drawn (522). A `<select>` is
still a square solid pixel with no chevron (521), and a field whose page
writes only a border shows the page through where Chrome's is white
(525). **Every box is on Chrome's grid of 1/64 px** (ADR 0054, item 466): a
length, a percentage and a `calc()` are each truncated toward zero onto it,
a run of text's width is rounded up — by Chrome's item, one text box's
text on one line, so a `<span>` and what follows it are on the grid too
(item 574: 950 of 1,267 corpus boxes are Chrome's exactly at one where 899
were, 949 at two where 926 were, and `text-on-the-grid` is Chrome's to the
unit at both) — and a box's edges are placed on it as
they are read back, so centring, auto margins and flexible tracks put each
edge where Chrome does. Measured again with the same fonts, 707 of the
corpus's 1,260 element boxes have Chrome's left, top, width and height
exactly, where 514 did before, and 855 since item 467 (measured in
standards mode: the earlier counts put Chrome in quirks mode for cases with
no `</head>`, and were 807 there), 892 since item 507;
`alo-sites-booking`'s are all Chrome's. What the grid cannot reach yet
is what `taffy` decides between its own passes (499): the mini-month's
columns are their ideal edges floored, 1,837 and 1,838 units, where Chrome
sizes them again and makes every one 1,838, and the shares of flexible
items are within two units of Chrome's, and a grid's `justify-content`
after the first within a unit. How Chrome shares a flex line is measured
(ADR 0054's addendum of 2026-10-13: 5,120 lines at one and two, every one
given by its rule) and waits for `taffy`'s hook to be built. A line whose
factors sum below one with a gap between its items flexes by all of its
free space where Chrome flexes by the part, a fault in `taffy` (572). A value that is negative, not finite or cannot be read is
`normal`. One that is merely enormous makes a line that tall, as an
enormous `height` makes a box that tall, and past the range of a float
both reach geometry that is infinite; the page is still drawn. An atomic box stands on the
baseline of its **last line** — a button on its label's, a text field on its
value's, an inline-block on its last line of text — and an `<input>` with
nothing typed in it on the line it would hold, its strut's baseline, so it
stands where it will once typed into (item 384; an empty button and
`<textarea>` do not) — and on its bottom margin edge when it has no line or its `overflow` is not `visible`, which is where an
image and an SVG stand. A **checkbox and a radio** stand on their bottom
*border* edge, as Chrome 154 stands them, below the user-agent sheet's 3
pixels of margin (item 443). Chrome ignores an author's padding and border
on either and this engine draws them (453). The line is found through the blocks
inside the box, last first; a flex or grid container, or a scroll container,
met on the way is **refused**, and the box stands on its bottom margin edge
instead. An **`inline-flex` box** stands on its *first* baseline, its first
item's first line's, or that item's bottom border edge when it has none
(item 395); a row with an item aligned on its baseline (396), an
`inline-grid` and a scroll container are refused and stand on their bottom
edges. An author's `inline-block`, `inline-flex` or `inline-grid` holding
blocks is one atomic box holding them, and an `inline-block` stands on its
last block's last line (item 286; until then the box tree broke it around
them as if it were a `<span>`). A percentage width on an
inline-block is drawn against the room the line gave it rather than its
container (item 284).

**A `<details>`** is its first `<summary>` child, laid out first wherever it
was written, then one block holding everything else (item 429, ADR 0048).
Without `open` that block is there and empty, so the `details` is its
summary, padding and border, and a flex or grid `details` keeps the gap
before it; nothing it hides is laid out, drawn or read by an agent. The
summary is the one `summary` an agent reads as such, with `expanded`, and
activating it toggles `open` — an agent's or a script's click alike, unless
a listener cancels it. No disclosure triangle is drawn (432), a `details`
with no summary is given none and says so (433), and the `toggle` event,
`HTMLDetailsElement.open` and `name` are not built (434).

**`vertical-align`** moves an image, an inline-block or an inline box, and
everything inside it, from its parent's baseline (item 312). Every keyword
is read: `baseline`, `middle` (half the parent's x-height over its
baseline, that half rounded onto the layout grid and the box's own half
height truncated onto it, as Chrome 154 does: item 598), `text-top`, `text-bottom`, `sub` and `super` (Chromium's
distances, a fifth of the font size and a pixel down, a third and a pixel
up), `top` and `bottom` (against the line box, after everything else has
said how tall it is), a length, and a percentage of the box's own
`line-height`. What moved makes its line taller. An inline box is aligned
by the room its `line-height` gives it, leading and all, as CSS says. A face whose table has no x-height, DejaVu among
them, is measured by its own `x`, as browsers do.

**Media queries** answer `min-width`, `max-width`, `width`,
`prefers-color-scheme`, and the screen's `resolution`, `min-resolution` and
`max-resolution` (item 567, below), joined by `and`, after an optional `not` or `only`
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

**A line under, over or through text is where Chrome 154 draws it**
(item 566), at one and at two device pixels to the CSS pixel: whole
device rows, a tenth of the font's size thick cut down, placed from the
text's top rounded to a device pixel. That rule drew all 480 lines of a
probe exactly as Chrome did, and `text-decorations`' lines are on Chrome's
rows at both scales. **An underline or an overline breaks where a
letter's ink crosses it, where Chrome breaks it** (item 579): measured on
1,022 gap edges of underlines and every gap of 44 overlines at 22 sizes
and both scales, the rule puts every overline gap and 1,014 underline
edges exactly on Chrome's device pixel, and the other eight one pixel off
where the edge falls on a half. `text-decorations`' lines are Chrome's
piece by piece at both scales, the space before the decorated span's
child included (item 580): a line runs under a space its line goes on
past and stops before one that ends the line. So does a space that is a
text box of its own — between two inlines — or the first thing in one
(item 582): `decorated-spaces` is Chrome's piece by piece at one and at
two, its narrow line ending bare and its two ellipsis lines stopping
where what is kept ends, and the leading spaces of five links on
`web-first-website` are underlined where Chrome's are. Before an
ellipsis a line stops at the last letter kept (item 583):
`text-overflow-ellipsis`' underline is Chrome's, 8 to 47, against a
frozen Chrome 154 picture beside the case, and a probe of six such lines
(a space kept in the cut piece, a piece kept whole before a span or a
bold of which nothing is, two preserved spaces, a box kept between) drew
every one on Chrome's pixels. `text-decoration-skip-ink` is read (item
581): `skip-ink-values` — `none`, `all` and `auto` under and over the
letters, a line through under `all`, a child's value under its parent's
line both ways, `bogus`, `initial`, `none auto` and an inherited `none` —
has every line's every piece on Chrome's pixels at one and at two, and
the letters on top of an underline or overline that runs whole and under
a line through, as Chrome's are. Under text that casts a shadow (item
584) each line casts the text's shadow and the shadows go where Chrome's
do: `text-shadow-on-lines` — shadows below and above, under, over and
through, two shadows, a blurred one, a line from a parent without a
shadow, a space between two spans, and lines' shadows reaching the other
line — has every pixel both engines draw in a plain colour the same at
one, and at two all but 155 single pixels at the letters' edges, where
Chrome's letters are thicker (576, 577).

Boxes can be round, and clip what is inside them to their own shape. **A
curve's edge is covered as its area says** (item 459): the rented rasteriser
samples a pixel four by four, which drew a dot twelve pixels across 111 on
its left edge and 95 on its right and lost two thirds of a pixel of its area,
so every tile a shape's edge passes through is sampled again sixteen by
sixteen. A small curve is then within 3 of 255 of its exact area, and a
straight edge within 8, the same on both sides. **Letters are sampled the
same way** (item 462). Against Chrome 154's screenshots of `DejaVu Sans` at
12, 13 and 16 px (`letters-at-three-sizes`), every line is nearer than with
the rented sampling: the mean difference in ink over inked pixels is 20, 37
and 17 levels at the three sizes where Chrome's letters are unthickened,
and 23, 39 and 22 as Chrome draws a page. **Not yet:** on macOS Chrome
thickens every letter, so this engine's carry 12 to 16 per cent less ink
than Chrome's screenshot (576), and Chrome pulls a lower-case letter's and a
capital's top up to a whole pixel, which at 13 px is most of a row (577).

An inline box broken around a block is read by an agent as **one thing**: one
node, named by everything the element contains, positioned everywhere it was
drawn, with the block read inside it rather than beside it. Still a view — the
box tree records which boxes belong to which whole and the reader follows it.

**An agent can read a rendered page as a tree of what it is** — roles, names,
states and positions, with no screenshot involved — **and aim a verb at it by
name**: activate, put text, scroll, with no verb taking a coordinate. Every
corpus case pins that tree beside its picture.

**A name read from content has Chrome's spaces** (queue item 530): a flex
or grid item, an `inline-block`, `inline-flex` or `inline-grid` stands
apart from its neighbour, and two inline boxes in one line run together,
as Chrome 154's accessibility tree reads 27 such bodies
(`crates/alo-agent/tests/a_name_across_its_items.rs`). A `<br>` and a
picture drawn as its `alt` in a line still run into their neighbours
(534).

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
the "select all" box above a half-selected list — a bar. The colour is
`accent-color` if the page names one, Chrome's `#0075ff` if not, and the mark
on top of it is black or white by whichever shows up against it. The mark is
drawn by the engine rather than set in the user-agent sheet, for the same
reason a control's inner box is: CSS has no way to say "and draw a check
inside it".

**A checkbox and a radio draw Chrome's picture of themselves** (item 454). Not
a CSS border — Chrome computes none — but a square rounded 2 pixels, or a
circle, white inside a one-pixel `#767676` edge; checked, a checkbox filled
with the accent and ticked, a radio with an accent edge, a white ring and an
accent dot six tenths of it across; drawn in the largest square that fits
the box, centred. Every shape was fitted to Chrome 154's screenshots at eight
device pixels to one and every colour read from them at one, the disabled
greys over black, white and grey to separate colour from opacity.
`cases/toggle-artwork` is compared with Chrome's own screenshot of it,
committed beside it: inside each shape the colour is Chrome's to a unit, and
the edges differ only by how each smooths them — Chrome quantises an edge's
coverage to quarters at one to one and this rasteriser does not — by at most
61 of 255 and 2.11 on average (64 and 3.0 before item 459, 2.14 before
578). A disabled control is drawn as Chrome draws it, fitted at one, two and
eight device pixels to one over black and white (578): an inside a fifth of a
pixel in from the edge's outside, and over it `#767676` at 77/255 as the
edge, the checked fill or the dot, each composited on its own. Its pieces
used to let the page through by up to 44 of 255 over black, and faded as one
picture (463) were 24 from Chrome's; over black they now differ by 15 at
most, on a tick's edge, and a disabled radio by 10. What remains is the
quantising: Chrome covers a straight edge pixel three quarters where the
inside reaches four fifths of it (7 over black), so a disabled checkbox over
black is nearer Chrome's at its worst pixel and further on average (2.76
where it was 2.09). 463's own closing condition, seams within the unit the
inside meets, is not met by that margin and waits on a person. Where a control stands on a fraction of a pixel
its picture is drawn on whole pixels, as Chrome's is (item 457): each edge of
its box rounded to the nearest pixel, a half up, measured in Chrome 154.
`checkbox-beside-text`'s controls differ from Chrome's committed screenshot
by 1.5 on average and 61 at most, where they differed by 16 (and by 47 before
item 454). Since item 460 every row and control stands where Chrome's does to
1/64 px, and the whole page differs from Chrome's by 1.0 on average where it
differed by 4.0. A
background at a fraction of a pixel is drawn on whole pixels, as Chrome draws
it (565); this page's rows were already on whole pixels, so it did not move,
and every one of its 49,525 pixels outside the controls and the text is
Chrome's exactly (461, `a_background_beside_controls.rs`). A native checkbox with `aria-checked` draws the
state it declares; Chrome draws its own checkedness (456). A light accent's
tick is black here and Chrome's dark grey `#3b3b3b` (458).

**A control nobody can operate says so, and still says what state it is in.** A
disabled control draws its mark in grey rather than the accent, whatever its
`accent-color`, and its border or edge pales — so on, off, on-and-locked and off-and-locked are four different
pictures. That much is ordinary colour and does live in the user-agent sheet,
where a page can override it. Its colours are Chrome 154's computed ones,
measured for every kind of control (item 509): a field's text a grey of 84, a
button's near-black at three tenths, both tinted and edged in translucent
greys; a list greyed and faded to seven tenths; a `<fieldset>` unchanged,
since only what it holds is disabled. In `cases/a-disabled-control` the
darkest pixel of each control's text is Chrome's committed screenshot's to a
level, and each fill is Chrome's exactly. A page's `background` or `border`
on a disabled control now beats the sheet's longhands, as the cascade says
it must: the shorthand resets what it covers. Chrome's fieldset groove is
`rgb(239, 239, 239)`, not the `#c0c0c0` below (520).

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
tones are Chrome 154's, measured over every grey and 650 colours with none
off (queue item 508): 0.33 off or on the brightest channel with the hue kept,
a colour too dark to darken drawn lighter (black is 84 and 168), and one too
light to lighten its own lighter tone. A button with an author's background,
drawn in the user-agent sheet's `2px outset black`, is Chrome's screenshot on
every side to the level (corpus case `a-button-with-its-own-background`).
Where two different tones meet on a corner's diagonal, the page shows faintly
through that one line of anti-aliased pixels, which Chrome draws as the two
tones' average: 49 off at worst there (518).

**Patterned borders.** `dashed`, `dotted` and `double` are drawn (corpus case
`border-patterns`): dashes about three widths long with equal gaps, and round
dots a width across, each spaced so a side starts and ends on one; `double` as
two lines a third of the width each, both following a rounded corner. The
spacing is ours, which CSS allows, on a straight side; Chrome's is
measured and not built (606). Round a rounded corner each side's dashes or
dots are laid along the border's centre line, as Chrome 154 lays them (596):
`dashes-and-dots-round-a-corner`, fifteen boxes, is Chrome's in every plain
pixel at one and at two but one at each, where Chrome's stroke round a curve
tighter than it reaches back before the top's first dash (604); a centre
line on half device pixels at one is spaced otherwise by Chrome (605).
Where sides of different widths meet on a square box, a corner dot is
clipped to its own side's share of the corner. A side is drawn with at most 16 384 dashes or
dots; past that they are spaced further apart. Beside a fieldset's legend
each is cut where the legend is (above).

**A border is whole device pixels wide.** A border width is snapped as CSS
computes one: a width of a pixel or more is floored to whole pixels and a
smaller one that is not zero is one, after `rem`, `em`, `vw` and `calc()` are
worked out (corpus case `border-widths`). So alo Sites' `border-left: .3rem`
is 4, as Chrome draws it, and a hairline of `0.5px` is one solid pixel rather
than a grey one. A device pixel is the screen's (item 560, ADR 0057): laid
out at a scale of two, `4.8px` is 4.5, as Chrome 154 floors it under a real
scale of two, which is how the window lays a page out on such a screen
(562).
A percentage, which is not a border width CSS accepts, is still read and laid
out unsnapped, and `thin`, `medium` and `thick` are not read (431).

**A page laid out for a screen's scale** (item 560, ADR 0057). Laid out at a
scale of two, every rule this engine rounds to whole pixels rounds to device
pixels and reports CSS pixels: a line's font figures and the top half of its
leading, a set line height and every length on a grid of 1/128 px, a
border's width, a text field's width, and the size a face is set at, cut to
hundredths of a device pixel. Chrome 154 started with
`--force-device-scale-factor=2` is the measure, never its emulated scale.
Of the 580 boxes in the corpus's 52 markup cases this engine places exactly
where Chrome does at one, 578 are exactly where Chrome puts them at two
(`a_page_at_a_scale_of_two.rs`, against Chrome's boxes frozen beside each
case). The other two are `border-widths`' flex items, which shrink only at
two and take Chrome's spare 1/128 px in another order (564); the same
shrink at one is a unit off too. Chrome's order is measured — the last
item first, each share rounded half away from zero — and waits for
`taffy`'s hook to be built (502). The 21 cases whose page runs script are
loaded by a renderer told the scale with the page (item 563): of their 313
boxes this engine places exactly where Chrome does at one, all 313 are
exactly where Chrome puts them at two, so 891 of 893 across all 73 cases.
A scale on the wire that is not a number refuses the message, and one that
is not a factor above zero is read as one.

**A page drawn for a screen's scale** (item 561, ADR 0057 § 4). The display
list stays in CSS pixels and says the scale it was built for; it is drawn
under a growth of that scale onto a canvas of device pixels, so a letter is
outlined at the size it is shown, shaped at the size its face is set at on
that screen. A checkbox's, a radio's and a control face's picture is snapped
to whole device pixels. `checkbox-beside-text` drawn at two puts all nine
of its controls on exactly the device pixels Chrome 154 draws them on under
`--force-device-scale-factor=2`, each control's picture within 65 of 255 of
Chrome's at worst and 0.79 on average (`a_page_drawn_at_a_scale_of_two.rs`,
against `chrome-at-2.png`, with `render-at-2.png` as its reference beside
`render.png`). Since item 565 every box's own area — background, border, shadows, an
inline box's pieces — is drawn on whole device pixels, at one and at two:
`backgrounds-on-device-pixels` puts every pixel along each box's middle
row and column exactly where Chrome 154's are at both scales, but a shadow
moved by a fraction of a pixel, within 6 of 255 (this engine samples a
soft edge sixteen by sixteen, Chrome by its exact area). At corners where
two soft edges meet Chrome covers a pixel by the lesser edge. A spread
shadow's corners are Chrome's (573): square on a square box, and a radius
smaller than the spread grown by less of it, `spread-shadow-corners` at
one and at two. An inset shadow on a bordered, rounded box is clipped and
rounded by the border box's radii where Chrome uses the padding box's,
which leaves a sliver of background in each corner (585). An overflow clip is on the same whole device pixels (575): the snapped
border box less its border, so a child reaching its parent's edge ends
where the parent's background does; `clips-on-device-pixels` is Chrome's
exactly at one and at two but within 5 px of a rounded corner (within 43
and 51 of 255). A picture is drawn into its content box with each edge on
a whole device pixel (587), and one that starts off the page starts there;
`pictures-on-device-pixels` is Chrome's in every pixel at one and at two.
An `<svg>` is clipped to its content box on whole device pixels and
drawn from its border box's corner on one, plus its border and padding as
laid out; an SVG file in an `<img>` is drawn into its rounded content box,
grown to fill it (589). `svgs-on-device-pixels` is Chrome's in every plain
pixel at one and at two; where a shape's corner lies a fraction into a
pixel both ways the two differ by up to 31 and 41 of 255, Chrome covering
it by the lesser edge. At 1.5 (queue item 593, Chrome 154 under
`--force-device-scale-factor=1.5`) 975 of the 985 corpus boxes exact at
one are Chrome's, the probes of every rule are Chrome's in all 904
numbers, and six cases drawn at 1.5 are Chrome's in every plain pixel,
more than 40 of 255 apart only at rounded corners. A checkbox and a radio
are drawn in their snapped box cut to whole CSS pixels (597):
`toggles-at-fractional-scales`, 216 controls of nine sizes, is Chrome's in
every plain pixel at one, 1.5 and two, and every unchecked one covers the
device pixels Chrome's does; `checkbox-beside-text` at 1.5 is Chrome's in
every plain pixel but four at the foot of its last row's letters (607),
which no control touches. A centred line's start and a `middle` raise
are put on the grid as Chrome puts them (598): at 1.5 every one of the
986 corpus boxes exact at one is Chrome's but the two flex items 564 owns,
and
`centred-and-middle-on-the-grid`, 118 boxes, is Chrome's in every box and
pixel at one, 1.5 and two. Text is not compared (576). An `outline` is not drawn at all (591). A picture with both a width
and a height set is that size, on a line, as a block, as a flex item and
positioned, and its `min-` and `max-` limits are CSS 2 § 10.4's (590);
`pictures-sized-on-a-line` is Chrome's in all 35 boxes at one and at two,
and in every pixel of each picture drawn no smaller than it is, smoothed
across its texels as Chrome smooths it (594); nine drawn smaller on an
axis at one, and one at two, are Chrome's in every plain pixel and
differ where `wide.png`'s halves meet, which Chrome samples through
mipmaps (179). `a-picture`'s PNG drawn at 80 and 30 is Chrome's in every
pixel, and `a-picture-in-each-format`'s GIFs and lossless and animated
WebPs at twice their size; a JPEG or a lossy WebP differs by what its
decoder gives (a level for the JPEG, up to 14 for the lossy WebP), and a
see-through stripe's edge by a level in 36 pixels (600). A rounded clip inside a border follows the border's
inner curve, each radius less the border beside it (588), as Chrome's
does in 60 of 60 boxes measured; `rounded-clips-inside-borders` is
Chrome's in every plain pixel at one and at two. A border whose sides
differ in width or colour fills its rounded corners to the inner curve,
one colour as one ring and differing sides split on the mitre (592);
`borders-side-by-side` is Chrome's in every plain pixel at one and at two.
A dashed or dotted side round a rounded corner is laid along the
border's centre line, as Chrome's (596); `dashes-and-dots-round-a-corner`
is Chrome's in every plain pixel at one and at two but one pixel at each
(604). A side
whose style is `none` or `hidden`, or given none, has no width however its
width was written (595); `borders-with-a-side-none` is Chrome's in numbers
and in every plain pixel at one and at two. A style written with no width
is laid out with none where Chrome's is `medium`, 3px (602), and `border`
or `border-<side>` written after one of its longhands does not override it
(603). Where a line under text falls at two is not measured (566).

**The window at its screen's scale** (items 562 and 599, ADR 0057 § 3).
The window tells the page's renderer and the strip's its screen's scale —
a whole factor, or 1.5, and for any other factor the largest of those
below it — with its size, on the wire as a stranger's number (one that is not a
number refuses the message; one that is not a factor above zero is one),
and places each frame one device pixel to each of its own. A window moved
to a screen of another scale has its page laid out and painted again at it,
and its strip too (`a_window_on_a_screen_at_its_scale.rs`, at two, one and
1.5, each frame equal to the same page or strip drawn at that scale in the
test). `alo` on this
Mac's Retina display, captured with `screencapture -l`, is the window
composed at two through the real conductor: every opaque pixel within one
level once put through the display's profile, 99.84% exact. A page is
told its scale (567): `devicePixelRatio` answers it, asked at every read,
and `resolution`, `min-resolution` and `max-resolution` are matched against
it in `dppx`, `x`, `dpi` and `dpcm`, answering each of 2,984 queries as
Chrome 154 did at one and at two (`cases/a-page-told-its-scale`, every box
Chrome's at both scales). Not yet: `matchMedia` (570), and a resolution
written with `calc()`, the range syntax or the boolean `(resolution)`
(571); where a person's pointer lands at two is not measured (568); and a
page's picture is at most 4,096 device pixels a side, so a wider window is
cut (569). At 1.5 (item 599) the strip's band is 54 device pixels and a
window's line 48, and `alo --frozen-fonts --scale 1.5` on this Mac's
external screen, captured with `screencapture -l`, is the window composed
at 1.5 through the real conductor in every opaque pixel (references
`a-strip-of-one-tab-at-one-and-a-half.png` and
`a-gone-tab-at-one-and-a-half.png`).

**A function in the `border` shorthand is read whole.** `border: 1px solid
rgb(29 78 216)`, a comma-separated `rgb()`, `hsl()`, a `color-mix()` and a
`calc()` width are each one part of the shorthand, whatever spaces are
inside them (corpus case `border-colour-functions`, queue item 435); they
were split at every space and lost, so the border was drawn in
`currentColor`, or 0 wide for the `calc()`. alo Sites' presented cards are
ringed in their text colour at 14%, as Chrome 154 computes it, and over
white that ring is `(223, 224, 225)`, the exact blend, as Chrome's default
raster path draws it (see the next paragraph). A colour function this
engine does not read, such as `oklch()`, is still left out whole, and the
border is drawn in `currentColor`.

**A translucent colour is blended exactly and rounded once** (corpus case
`translucent-fills`, queue item 436): source-over, in floats, written out
to bytes when the picture is. Measured on twelve translucent colours over
white, blue and yellow, Chrome 154 draws two pictures. Its default raster
path is the same blend: it agrees with this one on every cell whose blend
is not exactly a half, and on the nine that are, it rounds either way, so
three of them differ by one. Its software path (`--disable-gpu`) blends in
eight bits — the alpha rounded to a byte, the colour premultiplied and
rounded, the ground scaled by (256 − alpha) / 256 and truncated — which
accounts for every one of its 36 cells and draws up to a unit darker; that
is where the `(222, 224, 225)` item 435 compared against came from. That is
an approximation for speed, and it is not copied.

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
none of it and drew markup's own indentation. **That includes an
`inline-block` or a picture** (queue item 447): a box in a `nowrap` line
stays on it and overflows. Whether a line may break before such a box is
decided by the space before it, or else by the box holding both it and what
came before, never by the box's own `white-space`. Chrome 154 agrees on
every row of `crates/alo-corpus/cases/atomic-in-nowrap-line`, to its layout
unit across, line for line. Until then every such box that did not fit
started a new line. **Text follows the same rule** (queue item 449): where
two boxes meet, a break is the space's box's to allow, or the box of what
came before it, or a box that closed since; never the next box's own. So
`ab <span nowrap>…` in a `normal` block breaks at the space, and Chrome's
break after an atomic box in a `normal` span is made here too. Between two
letters there is no break at all, whichever boxes they are in, and a
no-break space is no longer collapsed into an ordinary space, nor left out
of the width that has to fit. Chrome 154 agrees on all fourteen rows of
`crates/alo-corpus/cases/soft-wrap-between-boxes`, and the render matches
its screenshot row for row. Three differences from Chrome are known, each in
the queue: a word that runs across two boxes and does not fit overflows
where it stands, where Chrome moves it whole to the next line if there was
an earlier break on its line (452); an inline box's trailing space at the
end of a line still widens the box (450); and an inline box that opens just
before a break keeps an empty piece on the line before (451).

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
picture, or one drawn smaller than it is, have stepped edges — one drawn
larger is smoothed as Chrome smooths it (594); and a picture ignores a clip in force,
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
| alo Settings | **yes** — `alo-workplace`'s, likewise, and its narrow-screen `@media` block evaluated rather than assumed away. Until item 440 (iteration 281) its `flex: 1` was ignored, so its content was 115.5 px narrower and Cancel and Save stood at the footer's left; its widths now match Chrome 154's |
| An agent reading Settings as a tree and activating a row by name | **yes** — `crates/alo-renderer/tests/an_agent_on_settings.rs`, against that same screen, by name and never by position |
| An agent reading and filling a page nobody wrote for us, by name | **yes, on two frozen pages** — `crates/alo-corpus/tests/an_agent_on_somebody_elses_page.rs` (queue item 131). The HTML specification's pizza form (`web-a-form`) is read by the words on it. Its fields, textarea, size and toppings are filled and read back, and a reference render of the filled form is committed. The first web page's links are followed by their words. The form's `time` field is refused by name, because nothing draws what one holds yet (388). Neither page runs script |
| An agent sending a form on a page nobody wrote for us | **asked for and decided, on pages that run script and pages that do not, and sent by a page's own `fetch`** — `crates/alo-corpus/tests/a_form_on_somebody_elses_page_is_sent.rs` (queue items 468, 485 and 472), `crates/alo-renderer/tests/a_form_is_decided.rs` (queue item 470) and `crates/alo-renderer/tests/a_form_sent_on_a_page_that_runs_script.rs` (queue item 485). Pressing the pizza form's *Submit order* checks it and asks to `POST` to `https://httpbin.org/post` with `custname`, `custtel`, `custemail`, `delivery` and `comments`, as Chrome 154 posts them, and with a size and toppings chosen, the entries Chrome's `FormData` gives in its order. An address that is not one stops it with Chrome's sentence. `alo-sites-booking`'s form, without its analytics script, says `Please fill out this field.` of its empty date field, and with a date asks to `GET` its booking page with `date=2026-10-20`. Text past a field's `maxlength` is refused. Through real tabs, the browser process decides the pizza form as the agent's action with Chrome's exact body, `custname=Ada+Lovelace&custtel=&custemail=&delivery=&comments=`, as `application/x-www-form-urlencoded`, and the booking form as Chrome's `GET` of `…/b/studio-consultation?date=2026-10-20`; a query already on a form's address is replaced, as Chrome replaces it; a secure page's form to `http` is refused and recorded; and a header `form-action` is obeyed. The frozen booking page, which runs script, is checked and sent as Chrome does it: no day fires one trusted, cancelable, non-bubbling `invalid` at the date field and sends nothing; with a day, a trusted, bubbling, cancelable `submit` names *See free times* as its submitter, the page asks for Chrome's `GET`, and the browser process decides it through real tabs. A `submit` listener's `preventDefault` keeps the form; `invalid` reaches each failing field in tree order; a listener pressing the button again sends nothing twice. **Not yet:** the decision is not loaded (item 85, for every navigation), a `<meta>` policy's `form-action` is not applied (486), a script's own `click()` on a submit button is refused by name (487), and offline the frozen contact form's fallback calls `form.submit()`, which is absent until item 471, so the form stays where Chrome would post it. **Sent by the page's own script** (queue item 472): the frozen contact form's `submit` listener cancels the press and posts `website=&name=Ada+Lovelace&email=ada%40example.com&message=…` as `application/x-www-form-urlencoded;charset=UTF-8`, Chrome's request exactly; answered from the case's frozen response, it replaces the form with *Thanks — talk soon.*, whose box and the section's are Chrome's to 1/64 px, drawn as `sent.png` (Chrome's picture differs only inside glyphs, item 462). `FormData` and `URLSearchParams` are pinned against Chrome 154 in `crates/alo-bindings/tests/what_a_page_builds_to_send.rs`. **Read before pressing** (queue item 476, `crates/alo-corpus/tests/what_an_agent_reads_of_a_field_its_form_would_refuse.rs`): the frozen booking page's day field reads `date "Choose a day" [required invalid]` as it loads and after a refused press, and `[required]` with a day in it; the contact form's empty required fields are not `invalid`, before or after a refused press, and its email field holding `nope` is — Chrome 154's accessibility tree, field for field |
| alo agent overlay | not yet — the screen is not written in `alo-workplace` either |
| The browser's tab strip | **yes, in the window** — shown across the top of the window with the selected tab's page below it, each tab called by its page's `<title>` as HTML reads it (queue item 528; `crates/alo-window/tests/the_window_composed.rs`' reference renders of one, three and a stopped strip, `the_strip_in_the_window.rs` for a hostile and a 100,000-character title through the real confined renderers, `a_strip_that_never_answers.rs` for a stopped strip renderer). Its own document is markup and a stylesheet after alo's tokens that ship in `alo-renderer` (`interface/strip.html`, `strip.css`), filled from the browser's list of tabs with each title a text node (queue item 297, ADR 0024 § 4), and checked by `crates/alo-renderer/tests/the_tab_strip.rs`: three committed reference renders (one tab, four, and one stopped), every tab's box in numbers, the agent's reading, and five hostile titles drawn and read as written. No other browser draws this strip, so the reference is alo's design rather than Chrome. A person chooses among the tabs with ⌘T, ⌘W, ⌃Tab and ⌃⇧Tab (queue item 529; `the_browsers_keys.rs` drives each key through the real conductor and renderers and compares every strip, and reads from the pages' own fetches that the tab left is told it is hidden before the tab chosen is told it is seen; `a-new-tab.png` is a new tab chosen over a page). An agent cannot yet read it through its socket (531), a title a page changes after loading is not followed (532), and a press of the main button on a tab chooses it too (queue item 539; `crates/alo-window/tests/a_press_on_the_strip.rs` presses each tab, a gap, the band above them and past the last through the real conductor and renderers and compares every strip, and reads the order the pages are told from their fetches; `crates/alo-renderer/src/strip_claim.rs` pins which points are on which tab, the rounded top corners on none). The strip has no control yet to close a tab or open one (558) |
| Which element a pointer is over | **yes, as Chrome 154 finds it, on 21 pages** — `crates/alo-renderer/tests/where_a_pointer_falls.rs` (queue item 298): box edges at whole and fractional pixels, overlapping and stacked boxes, a negative `z-index` under the body, borders against margins, turned, moved and scaled boxes, clips square, rounded and turned, rounded corners, an invisible box, inline boxes on their whole pixels, text in an element with no box, an inline block, a field's text and placeholder, and the root below a short page — each answer Chrome's `elementFromPoint`, which its mouse events agree with, and each box the points depend on in numbers. It follows what is drawn where Chrome does not: a positioned box that clips with square corners (536), and a positioned box inside a positioned box, which is drawn under it here (538). An `<svg>` is found whole (540), and a list item over the whole of its marker, where Chrome finds it over the bullet only (541) |
| A person's click | **yes for the main button, as Chrome 154 fires it** — `crates/alo-renderer/tests/a_persons_click.rs`, `a_persons_click_crosses_the_boundary.rs` and `alo-window/tests/a_persons_click_in_the_window.rs` (queue item 535): `pointerdown` and `mousedown` as one task, `pointerup`, `mouseup` and `click` as the next, each with Chrome's coordinates, `button`, `buttons`, `detail`, `pointerId` and `pointerType`; a cancelled `pointerdown` dropping the mouse events; a release elsewhere clicking the common ancestor, outside the page the root, after the pressed element left no click; a link followed to the same address as an agent's `Activate`, on the frozen `web-example-com` through confined renderers. A submit button's click sends its form as an agent's press does, decided as the person's, on the frozen `alo-sites-booking` and `web-a-form` (`a_persons_click_sends_a_form.rs`, queue item 543); a form not sent is said in the answer and not yet shown in the window (557). Not yet: other buttons (544), modifiers (545), click counts (547), focus (258) and selection (88) |
| A person's pointer moving | **yes, as Chrome 154 fires it** — `crates/alo-renderer/tests/a_persons_pointer_moving.rs` against Chrome's own record of the same page (`tests/references/a-persons-pointer-moving.{html,chrome.txt}`), and `alo-window/tests/a_persons_pointer_in_the_window.rs` (queue item 542): the crossing's pointer events then its mouse events, out and leaves, over and enters, each with its `relatedTarget`; `pointermove` and `mousemove`; the document entered from and left to nowhere; a press or release crossing first with the button that changed; a drag outside the viewport over the root; `mousemove` held back after a cancelled `pointerdown`; an element removed from under the pointer not told; moves coalesced. Not yet: `pointerrawupdate` (551), a page changing under a still pointer (552), another tab chosen under it (553), modifiers on a move (545) |
| `:hover` under a person's pointer | **yes, as Chrome 154 matches and draws it** — `crates/alo-renderer/tests/hover_under_a_persons_pointer.rs` against Chrome's own record of `querySelectorAll(':hover')` in every listener (`tests/references/hover-under-a-persons-pointer.{html,chrome.txt}`), and `hover_drawn_under_a_persons_pointer.rs`, whose three reference renders are pixel for pixel Chrome's screenshots of the same page under the same pointer (queue item 550): the element under the pointer and its ancestors, `<html>` included, hovered before the crossing's first event; nothing outside the page; a drag hovering what it is over; an element taken out hovering the parent it left. A hovered `<label>` hovers the control it labels (queue item 554; `a_label_hovered.rs` against Chrome's record on `tests/references/label-hovered.{html,chrome.txt}`, and `a_label_hovered_drawn.rs`, whose two reference renders are Chrome's screenshots pixel for pixel): by `for`, wrapped, the first of two, none for a `<div>`, a hidden input or an id a `<div>` took first, and the control kept while the label stays hovered. Where Chrome's control stays hovered after its label stops being hovered — its `for` or the control's `id` changed, a control put before it, the label taken out — this engine ends it with the label's, deliberately; the record marks those sections `DIVERGES`. Not yet: the crossing Chrome fires on a timer after the page changes under a still pointer (552) |
| alo offline screen | **nearly** — `alo-workplace`'s `offline.html`, frozen byte for byte as `alo-offline`, its hand drawn (item 273), it and the button centred across (item 280) and the hand's bottom margin kept (item 281), the whole screen centred down the window by `place-items` (item 282), the font's descent under the hand (item 283), and the button standing on its label's baseline so its line is the button's height (item 285) |
| alo Agenda's month navigator | **yes, in numbers** — `alo-workplace`'s `MiniMonth`, rebuilt as the markup React produces with its own rules and tokens, frozen as `crates/alo-corpus/cases/alo-agenda-mini-month` (iteration 280) and checked by `tests/alo_agenda_mini_month.rs`: its 42 days are square, 28.71 a side from seven `1fr` columns and `aspect-ratio: 1`, its picked day a terracotta circle, and its two arrow buttons 19 × 19, each a 15 px block `<svg>` in 2 px of padding — until item 437 a button was always at least a line tall inside, each arrow 23.5 and the grid 4 px low. Every box matches Chrome 154 on the same page given the same fonts, to its layout unit, except its columns: since item 466 every box is on the grid of 1/64 px, and the columns are their ideal edges floored, 1,837 and 1,838 units, where Chrome sizes them a second time and makes every one 1,838 (ADR 0056: it waits for a hook in `taffy`, items 501 and 502). An agent reads 42 buttons named by their number and nothing about which is today or picked, because the markup says neither |
| alo Agenda's calendar list | **yes** — `alo-workplace`'s Agenda sidebar (its "New event" button, both calendar lists and "Working hours"), rebuilt as the markup React produces with its own rules and tokens, frozen as `crates/alo-corpus/cases/alo-agenda-calendars` (iteration 281) and checked by `tests/alo_agenda_calendars.rs`. Each row's toggle takes the room its buttons leave by `flex: 1`, which until item 440 nothing read. Its list headings are `MY CALENDARS` and `OTHER CALENDARS` and its access badges `can edit` and `can view`, by `text-transform` (item 441, ADR 0049): measured, drawn and read by an agent as a person sees them, each width and the toggle beside each badge Chrome 154's to its layout unit — until then they were drawn as written, each badge's toggle 1.78 px narrow. A name too long for its row ends in "…" inside it (item 442): Mira's calendar's is drawn "Mira's calendar…", the "…" where Chrome 154's is to its layout unit and its dots in Chrome's pixel columns, and an agent reads the whole name — until then it was cut mid-letter at the row's edge |
| A page alo Sites publishes | **eleven sections, its blog index, its whole home page, and its rich text and blocks** — `crates/alo-corpus/cases/alo-sites-cta`, its call-to-action section frozen with its `site.css` (item 349), is drawn with its sheet and checked in numbers by `tests/alo_sites_cta.rs`. Its skip link, `position: absolute` on an inline, is out of flow and off the page, so the section starts at the top (item 352). Its analytics script runs past `Date.now()` (items 353 and 356, ADR 0036) and past `encodeURIComponent(location.pathname)` on its eighth line (items 359 and 360), and past `window.addEventListener` on its thirty-second (item 362, ADR 0037), and runs to its end. It is told when it is hidden (item 364, ADR 0039 § 1): hidden, its own `visibilitychange` listener reports `d=1000&p=%2F&w=800` and then `t=0` through a beacon a test lends, and shown and hidden again it sends nothing more. It is left as other browsers leave it (item 373, ADR 0039 § 2): its own `pagehide` listener, fired by the browser, reports `d=1000&p=%2F&w=800` and then `t=0` through a beacon a test lends, its `visibilitychange` listener hearing `hidden` next sends nothing more, and what it sent leaves the renderer as fetches the browser refuses by name, since a `fetch` does not outlive its page. With **no beacon lent** it reports through the engine's own `navigator.sendBeacon` (item 369, ADR 0040): left, its two reports are beacons the browser process decides as the document's and — from this page's markup served on this machine, in `alo-renderer`'s `a_beacon_outlives_its_page.rs` — makes after the page has gone, each a `POST` of `text/plain;charset=UTF-8` recorded as a beacon; hidden, it sends the same two while it is held, and their answers free its 64 KiB for a third. It reads the window's size and scroll position truly, 800 × 600 at the top (item 366, ADR 0038), and the page's own size as the renderer measures it when asked: the root 800 × 600, `body` 253 tall, the skip link off to the left not counted (item 370). Given a beacon and a `pagehide` by a test, it reports a depth of `d=1000&p=%2F&w=800` and then `t=0`; given a click with coordinates, `x=0&y=0&p=%2F&w=800`. None of this changes anything drawn. In the window its first frame is held until its sheet has answered, and is painted **styled** (item 351, ADR 0041); a sheet slower than three seconds would have it shown first and said so. Its subscriptions section, `crates/alo-corpus/cases/alo-sites-pricing` (iteration 247), is drawn with the same sheet and checked in numbers by `tests/alo_sites_pricing.rs`: its one tier fills the `auto-fit` grid, is lifted 12 px by its transform, and casts its shadow, whose colour is `color-mix(in srgb, var(--text) 12%, transparent)` — refused, and the shadow with it, until item 377 read a mix in sRGB. A mix in any other space, or of `currentColor` (item 378), is still refused. Its features section, `crates/alo-corpus/cases/alo-sites-features` (iteration 248), is laid out with the same sheet and checked in numbers by `tests/alo_sites_features.rs`: its bento grid's first card spans both tracks and is padded 2.5rem above and below by `padding-block`, which was dropped until item 379 split the block-axis logical shorthands (`cases/block-axis-spacing` shows the rule in a picture). **Its picture is white**: the page's script hides every section that moves into view until an `IntersectionObserver` reports it, and this engine has none (item 381), so the section is drawn at opacity 0 — what the page does in any browser without the interface. Its testimonials section, `crates/alo-corpus/cases/alo-sites-testimonials` (iteration 249), is drawn with the same sheet and checked in numbers and pixels by `tests/alo_sites_testimonials.rs`: its featured quote is `font-style: italic`, the corpus has no slanted face, and the upright face is leaned 14° as a browser would — drawn upright until item 382. `font-synthesis: none` and a synthesised bold are not done (383). Its booking section, `crates/alo-corpus/cases/alo-sites-booking` (iteration 250), is drawn with the same sheet and checked in numbers by `tests/alo_sites_booking.rs`: its day field, empty, stands where its text will, so its line is the field's own 48.4 and the form, centred down the grid, is 162 tall at 89.4 — until item 384 the field stood on its bottom edge with the font's descent under it. The field draws how a date is written, `yyyy-mm-dd`, in its own colour (item 385, ADR 0042), checked in pixels there, and an agent reads it as `date "Choose a day" [required]` with no value. A browser in the United States draws `mm/dd/yyyy` and one in Britain `dd/mm/yyyy`; this draws ISO 8601's order until the person chooses a region (387), and **no calendar button**, which waits until one opens (386). Its tickets section, `crates/alo-corpus/cases/alo-sites-tickets` (iteration 255), is drawn with the same sheet and checked in numbers by `tests/alo_sites_tickets.rs`: its banner undoes the card's `max-width: 38rem` with `max-width: none`, so it is the page's whole 800, and its button stands at the start of its flex row under centred text, as a browser draws it. Until item 393 layout recorded that `none` as a value it did not implement, though the box came out right. Its booking section on a day it is closed, `crates/alo-corpus/cases/alo-sites-booking-closed` (iteration 256), is drawn with the same sheet and checked in numbers by `tests/alo_sites_booking_closed.rs`: with no day field its six blocks fill the split layout's two `1fr` columns two to a row, each column 343 wide, and the closed notice wraps after "the" — until item 394 the notice's one line was taken as the narrowest it could be, and the columns were 320 and 366. Its navigation bar, `crates/alo-corpus/cases/alo-sites-nav` (iteration 257), is drawn with the same sheet and checked in numbers by `tests/alo_sites_nav.rs`: each menu link is `inline-flex`, 44 tall with its text centred, and stands on its text's baseline, so its list item is 44 (the button's 48.4) and the bar 73.4 — until item 395 an `inline-flex` box stood on its bottom edge, the strut's descent hung under every link, and the bar was 81.1. Its `position: sticky` is recorded and not done (97); at the top of a page that has not scrolled it changes nothing. Its shop section, and the catalog and collection sections with nothing in them, were rendered too and check against the sheet by hand, but found nothing, so they were not kept. Its contact form, `crates/alo-corpus/cases/alo-sites-contact-form` (iteration 258), is drawn with the same sheet and checked in numbers by `tests/alo_sites_contact_form.rs`: its split layout's `gap: clamp(2rem, 6vw, 5rem)` is 48 both ways on an 800-wide page, so its columns are 284.8 and 427.2, 48 apart, and its introduction sits 48 under the heading's row — until item 397 the gap was split at every space, read as none, and the columns were 304 and 456, touching. Its honeypot field's `clip-path` is not read; its `overflow: hidden` 1 px box already shows nothing. Its footer, `crates/alo-corpus/cases/alo-sites-footer` (iteration 259), is laid out with the same sheet and checked in numbers by `tests/alo_sites_footer.rs`: its `<main>` is empty, so the footer's 3rem margin collapses through `body`, which starts 48 down, and the skip link, with nothing positioned above it, stays at the top of the page — until item 355 every absolute box was placed against its parent, and the link sat 48 down with `body`, off the page to the left so that only the layout tree showed it. The footer's split grid has two 356-wide columns 3rem apart. Like the features section, it is set to move into view, so **its picture is white** until item 381. The transition section is `display: none` and draws nothing. The FAQ section lays its questions out in two columns, which waits on multi-column (97). Its rich text — what alo Sites makes of an alo Docs document, the body of every blog post — is `crates/alo-corpus/cases/alo-sites-rich-text` (iteration 260), drawn by the user-agent sheet alone and checked in numbers by `tests/alo_sites_rich_text.rs`: its nested point starts where the line above it ends and the finished task is struck through, which until item 400 stood 16 pixels away and was drawn plain. Its lists draw their markers (item 401, ADR 0044): a disc beside each outer point and the checklist's task, a hollow circle beside the nested one, and "3." and "4." on the list that starts at three, each outside its point's first line without moving any text, and read by an agent as the point's `marker`. A lettered or roman list, an `inside` marker and a picture marker are drawn without one and said (404–406). Its headings and bold words, laid out bold and drawn regular with a gap after "clear & deliberate" until item 402, are drawn in the bold face, each run's ink reaching the edge of the room it was given. Its other blocks — a figure, two code blocks and an equation — are `crates/alo-corpus/cases/alo-sites-rich-blocks` (iteration 264), drawn by the user-agent sheet alone and checked in numbers by `tests/alo_sites_rich_blocks.rs`: the Rust block's indented line starts four spaces in, which until item 410 stood flush with the line above it. Its code is drawn in DejaVu Sans Mono, every character one advance wide — in DejaVu Sans, and said, until item 412 — at 13 px, the size a browser draws it, with each `pre`'s `1em` margins 13 (item 415, ADR 0045; 16 px until then); its picture's bytes are cut off, and the broken picture is drawn as its `alt`, "Orange notebook <front>", inline on the figure's first line in the figure's own text, and read by an agent as `image "Orange notebook <front>" [broken]` (item 413, ADR 0046) — it drew nothing there until then. Chrome draws a 16 px icon before the words; this does not (418). A blank line in a code block is lost (411). Its text-and-image section, `crates/alo-corpus/cases/alo-sites-text-image` (iteration 270), is drawn with the same sheet and checked in numbers by `tests/alo_sites_text_image.rs`: its overlapping text card is `margin-inline-start: -4rem`, so it is 428 wide at x 352, reaching back over the picture's column, with its sentence on one line — until item 380 no inline-axis logical property was read, and the card stood in its own column, 364 wide, its sentence on two lines. Every number matches Chrome 154 on the same page. The mapping is left to right's; a right-to-left page would want the margin on the other side (421). Its picture's address is made up and has no bytes anywhere, so it is drawn as its `alt` in every browser. Its blog index, `crates/alo-corpus/cases/alo-sites-blog-index` (iteration 271), is drawn with the same sheet and checked in numbers by `tests/alo_sites_blog_index.rs`: each card's `Read article` link is one piece on one line, with its screen-reader `<span>`, `position: absolute`, at the link's end and its line's top less its 1 px margin, as Chrome 154 places it — until item 354 the link was broken around the `<span>` and an agent read it in two pieces. Its cards are two columns of 368 at x 20 and 412, as Chrome's are: the grid's `repeat(auto-fit, minmax(min(100%, 18rem), 1fr))`, which `taffy` throws away because a track's size is a calculation, has the calculation resolved against the grid's 760 first and handed over as 288 (item 422, ADR 0047) — until then the cards were one column of rows 760 wide. A grid like it whose width is not definite, sized by its content, is measured without its template and said (424). Its team, gallery, hero, collection, catalog and ordering catalog sections were laid out too and matched Chrome 154 given the same fonts, to its layout unit, so they were not kept. Its whole home page, `crates/alo-corpus/cases/alo-sites-full-page` (iteration 274), one of every kind of section in the variants the others do not use, is drawn with the same sheet and checked in numbers by `tests/alo_sites_full_page.rs`: its brand link, `inline-flex` holding a logo the sheet makes `display: block`, is one box at (20, 14.2) holding the logo, as Chrome 154 places it — until item 286 the box tree broke the link around the logo, two empty links with the picture between them. Chrome's link is 16 wider, its broken-picture icon (418). Its closed `<details>` is its question alone, 53.2 tall as Chrome's, its answer neither laid out, drawn nor read, and an agent reads `summary "How fresh is the coffee?" [expanded=false]` and opens it to 92.4 by activating it (item 429, ADR 0048) — until then the answer was drawn and read, 92.4 tall closed; no disclosure triangle is drawn beside the question (432); its custom-code section's `.3rem` border is 4 wide and its content starts at 44, as Chrome's — until item 430 the border was 4.8 and the content at 44.8; its features card, like `alo-sites-features`' own, is ringed in its text colour at 14%, `rgb(23 33 43 / 0.14)`, as Chrome computes it — until item 435 the shorthand's `color-mix()` was split at its spaces and the ring drawn opaque. A blog post and the catalog with one category were laid out too and matched Chrome 154 given the same fonts, so they were not kept. Every alo Sites golden page has now been frozen or measured |

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
is what a transition has finished doing, and nothing is hovered because a
still picture has no pointer; under a person's pointer the `:hover` rules are
drawn (queue item 550).

Two things about the case are still transcriptions rather than substitutions,
and both are noted in its own stylesheet: the design tokens are declared inline
because `tokens.css` lives in another repository this one only reads, and
Tailwind's preflight is one `box-sizing` rule rather than the whole of it.

The headline still wraps one line more than the real screen does, and that is a
**font** difference rather than an engine one: the corpus renders in DejaVu
Sans, which is wider than the Inter the app loads. Web fonts are stage 2.

**The email field's placeholder is drawn** (item 389, ADR 0043):
`you@company.eu` in `--text-tertiary`, through the rule alo's `Input`
component writes as `placeholder:text-tertiary` — `.input::placeholder {
color: var(--text-tertiary) }`, which the case's stylesheet carries and says
so. It stands in the middle of the 46-pixel field, as a browser draws it; an
agent reads the field as `textbox "Email" [placeholder="you@company.eu"]`
with nothing typed in it. Of what a page may set on `::placeholder`, only
its colour is drawn (item 392), and a hint longer than its field is not yet
clipped (item 390); neither is on this screen.

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
on every run — sixty-eight of them today. A case whose page carries script is
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
