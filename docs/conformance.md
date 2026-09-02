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

What does not exist: numbers. A computed style holds the text a declaration was
written with — `16px` is four characters, not a length (queue item 12) — and no
box has a position or a size (queue item 5). Nothing is painted, so every target
below is still `not yet`.

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
