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

- [ ] **7. Paint.** A display list from the box tree, then a software raster to a
  PNG. Deterministic output is the point: it makes every visual change reviewable
  as a diff.

- [ ] **8. Reference renders.** A committed corpus of small cases, each with its
  expected image and **its expected box tree**. A failure that says "row three
  moved 4px" is worth ten that say "the image differs".

- [ ] **9. ★ The agent tree.** The layout tree read as roles, states, positions
  and text (ADR 0002). One tree, two readers — a *view*, never a parallel
  structure, because two structures eventually disagree and the agent acts on the
  one that is wrong.

- [ ] **10. ★ Typed verbs.** Activate, put text, scroll. **No verb takes a
  coordinate**: a coordinate is a guess about a layout that may have changed
  between the reading and the acting. Same shape as `alo-os`'s verb contract.

- [ ] **11. A real alo screen.** The sign-in screen from the Figma file, its
  colours from `tokens.css`, rendered and diffed against a reference. This is the
  item that turns the project from plausible into real.

- [ ] **13. A block inside an inline, split properly.** CSS says an inline box
  holding a block-level box is cut in three around it. This engine treats the
  inline box as a block container instead, which is the shape it ends up
  looking like and is not what the specification says — the difference shows in
  where backgrounds and borders stop. It is recorded as
  `IssueKind::UnsupportedStructure` on every tree that meets one, so a real page
  hitting it will say so rather than being found by reading. **Cut from item 4
  on the iteration that built it**: the wrapping of inline runs in anonymous
  boxes is the common case and is done properly, and this is the rare one.

- [ ] **15. `calc()` with a percentage in a layout property.** `taffy` carries
  such a value as an opaque handle that only a tree implementing its own traits
  can resolve, and this engine uses `taffy`'s ready-made tree — so
  `width: calc(100% - 2rem)` is refused and recorded rather than becoming
  something else. Doing it properly means owning the tree traits, which is most
  of the way to replacing `taffy`, and that is a decision rather than a chore.
  A `calc()` of lengths only is already a plain number by then and works today.
  **Cut from item 5 on the iteration that built it.**

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
