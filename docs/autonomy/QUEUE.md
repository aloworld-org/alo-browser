# Queue — stage 1

Worked in order by `LOOP.md`. Every item names what it implements so an
iteration can read the reasoning rather than guess at it.

## Before the first item

**Read `docs/decisions/0001` and `0002` in full.** The first says what we are and
are not building and why the scope is survivable; the second constrains the shape
of layout from the very beginning, and cannot be retrofitted.

**Everything here runs on any machine.** No GPU, no window, no network. Output is
a PNG from a software rasteriser, which is deterministic and diffable — so every
item is testable from the first one, and the display server arrives after
correctness rather than before it.

**The measure is alo, not a conformance score.** The target that ends stage 1 is
`alo-os`'s own sign-in screen and Settings, rendering correctly. Those designs
exist in Figma and their colours come from `alo-workplace`'s `tokens.css` — that
file is the specification for what "correct" means here.

---

## Ready

- [x] **1. A DOM of our own.** `html5ever` parses; we hold the tree. Nodes,
  attributes, parent and child links, and a stable id per node — the agent tree
  in ADR 0002 will need to name a node later, and adding identity afterwards
  means rewriting everything that holds a reference. Tests: a document round
  trips; a malformed fragment still produces a usable tree.

- [x] **2. Stylesheets.** `cssparser` into rules we hold; `selectors` for
  matching. Only the modern subset — no quirks mode, no legacy pseudo-elements.
  An unknown property is kept and ignored rather than dropped, so a later stage
  can implement it without a re-parse.

- [x] **3. Computed style.** The cascade, inheritance, and **`var()`**.
  `alo-workplace`'s design system is custom properties throughout, so a renderer
  that cannot resolve them renders nothing of alo at all — this is not decoration
  and it is not deferrable. Tests: specificity order; inheritance through a gap;
  a variable defined on `:root` and used four levels down; a cycle refused rather
  than looped.

- [x] **4. The box tree.** Boxes from styled elements — and **what each box
  means**, not only its rectangle (ADR 0002). Role, state, and the text a person
  would read. A layout pass that keeps only geometry cannot be retrofitted into
  an agent tree, which is the whole reason this item sits before layout.

- [x] **12. Lengths as numbers.** *(Moved ahead of item 5 while building item
  4.)* Item 3 delivers *specified* values as text — `16px` is four characters,
  and a property nobody set is absent because absence is what "initial" means.
  `taffy` wants a number, and so does every stage after it. This is the layer
  that gives one: lengths in every unit CSS has, percentages kept as
  percentages because their basis is layout's, `calc()` evaluated, and `em` and
  `rem` resolved against the font size actually in force — which is why it
  needs the cascade to have run and could not have come earlier. **It sits
  before item 5 because layout would otherwise have to parse lengths itself,
  and then item 14 would build a second value parser for colours.** One value
  layer, used by layout and by paint.

- [x] **5. Layout.** Flexbox and grid, on `taffy` behind our own boundary — one
  file may name `taffy`, as `alo-os` does with its runtime. Tests are
  **numbers**: assert the computed box, never a screenshot somebody eyeballed.

- [x] **6. Text.** HarfBuzz shaping and font rasterisation. Do the awkward
  scripts before the easy ones — a pipeline that assumed left-to-right and one
  glyph per character is one that gets rewritten. Line breaking, and the
  fallback chain when a font lacks a glyph.

- [x] **16. Inline formatting: a real line box.** Shaping and breaking give a
  line its glyphs and its width; putting several inline boxes on one line, with
  baselines, and breaking *between* them rather than only inside one run, is
  layout work that needed a shaper before it was possible. `engine.rs`'s
  `needs_a_line_of_its_own` is the stand-in it replaces. **Cut from item 6 on
  the iteration that built it**, because measurement is the half that unblocks
  everything and this is the half that needs its own design.

- [x] **14. Colours as channels.** *(Moved ahead of items 17 and 7 after item
  16.)* The other half of what item 12 was originally written as, split from it
  when item 12 was built: hex, `rgb()`,
  `hsl()`, the named colours, `currentColor` and `transparent`, into channels.
  It blocks paint rather than layout, which is why it was not item 12's problem —
  a layout pass has never needed to know what colour anything is. **It comes
  before paint for the same reason item 12 came before layout:** a colour parser
  built inside paint is how the value layer grows a second one.

- [x] **17. Glyph rasterisation.** Turning a shaped glyph into coverage. It is
  cut from item 6 and folded in beside item 7 rather than before it: a glyph
  bitmap with no canvas to draw into can only be tested against itself, and
  next to paint it is tested against a picture. **Cut from item 6.**

- [x] **7. Paint.** A display list from the box tree, then a software raster to a
  PNG. Deterministic output is the point: it makes every visual change reviewable
  as a diff.

- [x] **18. The shape of a box: rounded corners, and clipping to them.** Item 7
  draws a colour inside a shape and the shape is always a rectangle.
  `border-radius` changes what the shape *is*, and `overflow: hidden` clips
  what is inside a box to that same shape — one question, asked twice. **Cut
  from item 7**, and item 11's real alo screen needs it.

- [x] **19. Shadows and gradients.** How a colour *fills* a shape, rather than
  what the shape is: `box-shadow`, `text-shadow`, `linear-gradient` and
  `radial-gradient`. Each needs a value grammar of its own and a blur, and each
  is worth its own reference render. **Cut from item 18** when that item was
  split, because changing the shape and changing the fill are different work.

  **Done.** A shadow is coverage blurred rather than a picture blurred, so what
  is behind it survives; an inset shadow is the same blur run on the shape with
  a hole in it. A run of text is outlined into one shape before it is blurred,
  because one blur per letter is darker where two letters touch. Corpus case
  `shadowed-card`.

- [x] **20. Transforms and opacity.** How a drawn thing is *combined* with what
  is behind it: `transform` moves a shape's points and `opacity` composites a
  whole subtree as a group, which means drawing it to its own surface first.
  Both establish stacking contexts, so paint order changes with them. **Cut
  from item 18** for the same reason as item 19.

  **Done.** Paint order now follows stacking contexts rather than one flat list
  of positioned boxes: a positioned box is painted last in the context it
  *belongs to*, which is what keeps a positioned box inside a transformed one
  inside that transform. Corpus case `turned-and-faded`.

- [x] **8. Reference renders.** A committed corpus of small cases, each with its
  expected image and **its expected box tree**. A failure that says "row three
  moved 4px" is worth ten that say "the image differs".

- [x] **9. ★ The agent tree.** The layout tree read as roles, states, positions
  and text (ADR 0002). One tree, two readers — a *view*, never a parallel
  structure, because two structures eventually disagree and the agent acts on the
  one that is wrong.

- [x] **10. ★ Typed verbs.** Activate, put text, scroll. **No verb takes a
  coordinate**: a coordinate is a guess about a layout that may have changed
  between the reading and the acting. Same shape as `alo-os`'s verb contract.

- [x] **11. A real alo screen.** The sign-in screen, its colours from
  `tokens.css`, rendered and diffed against a reference. This is the item that
  turns the project from plausible into real.

  **Done, with the target changed and the change written down.** `alo-os` is not
  checked out beside this repository and the Figma file is not reachable, so the
  screen rendered is **`alo-workplace`'s** sign-in screen — its real markup, its
  real rules, its real tokens — rather than `alo-os`'s. That is a real alo
  screen and it is not the one `ROADMAP.md`'s exit gate names, so the exit gate
  is **not** met. See `docs/conformance.md`, which says so plainly.

- [x] **13. A block inside an inline, split properly.** CSS says an inline box
  holding a block-level box is cut in three around it. This engine treated the
  inline box as a block container instead, which is the shape it ends up
  looking like and is not what the specification says — the difference shows in
  where backgrounds and borders stop. **Cut from item 4 on the iteration that
  built it**: the wrapping of inline runs in anonymous boxes is the common case
  and is done properly, and this is the rare one.

  **Done.** The inline box is broken into a piece on each side of the block,
  and the block becomes a sibling of the anonymous blocks the pieces sit in —
  which is why it could not be done by rearranging children in place. Each
  piece is a box of its own and draws its own background, so the highlight
  stops before the block and starts again after it. Corpus case
  `broken-inline`. Two cuts, both written below as items 21 and 22.

- [ ] **21. An empty piece of a broken inline keeps its border.** CSS keeps a
  piece with nothing in it — *"even if either side is empty"* — and an empty
  inline with a border draws that border. This engine drops it, because its
  inline formatting would give it a line box of the font's height and that is a
  visible gap where CSS asks for none. Recorded as
  `IssueKind::UnsupportedStructure` on every tree that meets one. **Cut from
  item 13**: the piece that holds something is the case a page actually has,
  and the empty one needs the zero-height line-box rule first.

- [x] **22. Borders and padding on an inline box.** An inline box's own border
  and padding were not laid out and not drawn: horizontal ones should add to
  the advance where the box starts and ends, vertical ones should draw without
  changing the line's height. **Found by item 13's corpus case**, which asked
  for a border and got none.

  **Done, and it took the wrapping bug with it.** An inline box arrives at the
  line as an *open* and a *close* around its content, so it gets one fragment
  per line like anything else that wraps — which is what fixed a background
  that used to be drawn from the union of its pieces and painted straight
  across the gap between two lines. The start border is drawn only on the first
  piece and the end border only on the last. Corpus case `inline-box`, and
  `broken-inline` has its border back. **Taken before item 21** because 21
  needs the zero-height line-box rule and this did not, and because a `border`
  on a `<span>` is ordinary CSS that a real page writes.

- [ ] **23. An agent reads a broken inline as one whole thing.** A `<div>`
  inside an `<a>` is still inside the link for a person and for a click, but
  the box tree has broken the link into pieces with the block a sibling of
  them. The agent tree reads the first piece and reads the later ones through,
  so nothing is doubled and no verb is made ambiguous — but the name of a
  broken link comes from its first piece alone, and the block between the
  pieces is not read as part of it. Doing this properly means the agent tree
  following the *document's* containment where the box tree has split, which is
  a change to what a view is. **Cut from item 13.**

- [x] **15. `calc()` with a percentage in a layout property.** `taffy` carries
  such a value as an opaque handle that only a tree implementing its own traits
  can resolve, and this engine used `taffy`'s ready-made tree — so
  `width: calc(100% - 2rem)` was refused and recorded rather than becoming
  something else. Doing it properly means owning the tree traits, and that is a
  decision rather than a chore. **Cut from item 5 on the iteration that built
  it.**

  **Done, and the decision is written down: ADR 0004.** The tree is ours, the
  algorithms are still `taffy`'s — a list of nodes with styles, a cache and a
  result is storage rather than physics, and `taffy`'s own trait set exists for
  exactly this. The handle for an unresolved expression is an index rather than
  a pointer, so there is no `unsafe` near it. Corpus case `calc-widths`. One
  thing is still refused and recorded: a `calc()` inside `fit-content()`, which
  the algorithms have no spelling for.

---

## After stage 1's ready items

Hardware acceleration, a display server surface, and embedding into `alo-os`'s
shell. All three want the software path correct first — accelerating something
whose behaviour is unsettled buys speed nobody can then change.

## Never in this queue

- **A JavaScript engine.** Stage 2, and only when a page we need requires it.
- **Legacy compatibility.** Quirks mode, floats-as-layout, CSS-table layout, the
  old DOM surface. Refusing these is what makes the scope survivable.
- **`unsafe`**, without an ADR.
- **Anything measured against a conformance percentage** rather than against alo.
