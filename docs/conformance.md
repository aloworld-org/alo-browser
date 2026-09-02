# What renders correctly today

Honest state, updated by the loop as items land. **Nothing renders yet.**

What exists: a document tree and a style sheet. HTML parses into the tree, it
round trips back to the same text, and malformed input produces a usable tree
with a record of what had to be repaired. CSS parses into rules we hold, and a
selector can be matched against the tree — so the engine can say *which rules
apply to which element*, on a given viewport width and colour scheme.

What does not exist: the cascade. Nothing decides yet which of several matching
declarations wins, nothing resolves `var()`, and nothing is laid out or
painted. Every target below is still `not yet`.

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
