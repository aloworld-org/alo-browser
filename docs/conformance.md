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

A glyph can be turned into coverage: outlines read from the font, scaled, and
filled with anti-aliasing.

What does not exist: a picture. Nothing composites coverage into pixels and
nothing writes a PNG (queue item 7), so there is still no reference render to
diff. Every target below is still `not yet`.

This file exists instead of a conformance percentage. A Web Platform Tests score
would grade us against thirty years of legacy we are deliberately refusing
(`docs/decisions/0001`), so it would measure the wrong thing and flatter or
punish us for the wrong reasons.

The measure is alo. These are the targets, in order:

| Target | State |
|---|---|
| `alo-os` sign-in screen | not yet |
| `alo-os` Settings | not yet |
| `alo-os` agent overlay | not yet |
| An agent reading Settings as a tree and activating a row by name | not yet |

Colours are correct when they match `alo-workplace`'s `web/src/ds/tokens.css`,
which is the specification for what "correct" means here.

## How a target becomes correct

A reference render is committed alongside its expected box tree. A target is
correct when both match and the numbers are asserted — not when somebody looked
at an image and thought it seemed right.
