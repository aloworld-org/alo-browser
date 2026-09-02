# Changelog

What changed, in words a person outside this repository can read. Newest first.

---

## Unreleased

- **Boxes have positions and sizes.** Block, flexbox and grid, with the box
  model, positioning and overflow — every number a CSS pixel, asserted as a
  number. The whole layout of a small interface is written out in a test, so a
  change that moves a box says which box and by how much.
- Layout runs on `taffy`, which ADR 0001 calls a judgement call rather than
  physics: a real chunk of engine, taken because it gets us laying out sooner
  and meant to be replaceable. It is named in exactly one file, and the gate
  checks that on every run.
- **How wide a piece of text is, is asked rather than assumed.** Layout takes a
  measurer and there is deliberately no default: a built-in guess would be a
  wrong number every layout quietly depended on. Real text is the next item.
- Two limits are named rather than hidden: there is no inline formatting yet
  (a run of inline boxes is laid out as a wrapping row, which gets them side by
  side and not their baselines), and a `calc()` mixing percentages in a layout
  property is refused and recorded rather than becoming something else.
- **A margin now starts at zero.** It started at `auto` for one draft, which
  silently centred every box in every document — a layout that looks deliberate
  and is not.

- **Lengths are numbers.** `16px` was four characters; now it is sixteen. Every
  unit CSS has that does not need a window to answer, `calc()` type-checked
  once and evaluated whenever a caller has a font and a basis, and `em` and
  `rem` resolved against the font size actually in force — which is why this
  layer had to wait for the cascade rather than come before it.
- `calc(1px + 2)` is refused when it is read, rather than producing three of
  something. A length may be added to a length and multiplied by a number;
  anything else is somebody's mistake and is reported as one.
- A percentage is carried rather than resolved, because `50%` is half of
  *something* and which something depends on the property and the containing
  block. Only layout knows, so only layout is asked.
- **`font-size` and `line-height` now inherit as computed values**, which is
  what CSS says and what a first draft got wrong: a child that inherited the
  text `2em` resolved it again against its own font, so `2em` inside `2em` was
  four times the grandparent rather than twice the parent. A `line-height`
  written as a number stays a number, so a child with a larger font still gets
  a proportionally larger line.

- **There is a box tree**, and every box in it knows what it *is*. ADR 0002
  says the layout tree is the agent's tree, so a box carries its role, its
  state and what it is called from the moment it is made — "invoice list,
  twelve rows, row three selected" is a thing this engine can now answer, on a
  document, without a screenshot and without a plugin.
- **Roles are declared, never inferred.** They come from the `role` attribute
  or from what HTML says the element is; a role this engine does not know is
  kept as written rather than dropped. There is no third source, and in
  particular there is no "it has a border and some rows, so it is probably a
  table".
- Box generation follows the tree rather than the markup: `display: none`
  removes a subtree, `display: contents` removes one box and keeps its
  children, and a container whose children are a mix of block and inline grows
  the anonymous boxes that make them one kind. The whitespace between two
  paragraphs makes no box; the space between two links does, because it is the
  gap between two words.
- **The engine has its own style sheet** — what an element looks like before
  anybody says otherwise. It is CSS text that goes through exactly the same
  parser and cascade as anything an author writes, because a second path
  through the cascade would be a second cascade to be wrong.
- Not one number yet. Where a box ends up is the next item, and keeping the two
  apart is what stops a box's meaning depending on where it landed.

- **The cascade works, and so does `var()`.** For every element of a document:
  which declaration wins — origin, then `!important`, then specificity, then
  order — what a child inherits when nothing wins for it, and what a custom
  property actually reads. `docs/decisions/0001` calls this stage 1's first
  hard requirement rather than decoration, because alo's design system is
  custom properties throughout: an engine that cannot resolve them renders
  nothing of alo at all, not badly, nothing.
- **A variable cycle is refused rather than looped.** `--a: var(--b)` with
  `--b: var(--a)` makes every property in the ring invalid, which is what CSS
  says and the only answer that terminates. A `var()` naming something that is
  not set falls back if it can and is recorded if it cannot.
- The text around a substitution survives exactly as written, so
  `calc(var(--gap) * 2)` becomes `calc(8px * 2)` — a value this engine does not
  yet understand is still one it can pass along intact.
- `inherit`, `initial`, `unset` and `revert` all work, including the one that
  is easy to get wrong: `revert` steps a whole origin aside, both its ordinary
  and its important declarations, rather than taking second place.

- **Style sheets parse into rules we hold.** `cssparser` tokenises and
  `selectors` matches; what a rule is, which selectors exist, and what happens
  to what we do not implement are ours. A sheet in the shape alo's design
  system is written in — custom properties throughout, a light and a dark
  theme behind `prefers-color-scheme`, a width breakpoint — parses whole, and
  an engine can now ask which rules apply to which element of a document.
- **Nothing is dropped in silence.** An unknown property is kept with its
  value, so a later stage can implement it without re-parsing the sheet; an
  at-rule we do not implement is kept whole for the same reason; a selector we
  cannot evaluate takes its rule down, which is what CSS says, and says so.
  Every one of those is reported with the text that caused it and the line it
  was on.
- **Media queries: width and `prefers-color-scheme`.** A condition we do not
  understand is treated as not matching rather than guessed at — the
  alternative is a dark theme leaking into a light one, which looks like a
  rendering bug and is not one.
- **`:hover` and `:focus` parse and never match**, because nobody is hovering
  a document being rendered to a file, and **`:visited` never matches at all**:
  whether a link has been followed is history, and a style that depends on it
  is readable back off the page. `:disabled`, `:checked`, `:required` and
  `:read-only` do match, from the markup, including the awkward parts — a
  disabled `<fieldset>` reaches its controls but not the ones in its first
  `<legend>`.

- **There is a document tree.** `html5ever` parses HTML and alo browser holds
  the result: nodes, attributes, parents and children, in our own types rather
  than the parser's. A document round trips — parse it, write it back out, and
  the text is the same — and malformed input still produces a usable tree, with
  a record of everything the parser had to repair to get one.
- **Every node has an identity that is never reused** (ADR 0003). An id names a
  node for the life of its document, and a node that leaves the tree keeps it,
  so a stale id names something that is gone rather than something else that is
  not. This exists in the first commit of the tree because the agent surface
  will stand on it, and identity cannot be retrofitted.
- **Quirks mode is recorded and never honoured.** A document that would put
  another engine into quirks mode is laid out as standards, because law 1
  refuses quirks mode outright. What the parser observed is kept for
  diagnostics rather than thrown away.
- `unsafe` is now forbidden by the compiler rather than by review, across the
  whole workspace.
- **The gate is a command that fails**, not a paragraph to remember:
  `scripts/gate.sh`. It runs the formatter, the linter and the tests, refuses a
  stub, refuses a crate that has quietly opted out of the ban on `unsafe`, and
  checks that each rented crate is still named in only the one file that is
  allowed to name it. What it cannot check, it names, so that a green run is
  never mistaken for the whole gate.
- The scope is written down: what gets built, in which stage, and what will not
  be built at all. The engine renders the modern platform and refuses thirty
  years of legacy — no quirks mode, no floats-as-layout, no CSS-table layout —
  because refusing them is what makes a project this size survivable. Stage 1
  needs no JavaScript engine, which removes a browser's largest single component
  from the critical path.
- alo browser exists as a decision and a queue. Nothing renders yet.
