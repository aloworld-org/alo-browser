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

- [ ] **3. Computed style.** The cascade, inheritance, and **`var()`**.
  `alo-workplace`'s design system is custom properties throughout, so a renderer
  that cannot resolve them renders nothing of alo at all — this is not decoration
  and it is not deferrable. Tests: specificity order; inheritance through a gap;
  a variable defined on `:root` and used four levels down; a cycle refused rather
  than looped.

- [ ] **4. The box tree.** Boxes from styled elements — and **what each box
  means**, not only its rectangle (ADR 0002). Role, state, and the text a person
  would read. A layout pass that keeps only geometry cannot be retrofitted into
  an agent tree, which is the whole reason this item sits before layout.

- [ ] **5. Layout.** Flexbox and grid, on `taffy` behind our own boundary — one
  file may name `taffy`, as `alo-os` does with its runtime. Tests are
  **numbers**: assert the computed box, never a screenshot somebody eyeballed.

- [ ] **6. Text.** HarfBuzz shaping and font rasterisation. Do the awkward
  scripts before the easy ones — a pipeline that assumed left-to-right and one
  glyph per character is one that gets rewritten. Line breaking, and the
  fallback chain when a font lacks a glyph.

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
