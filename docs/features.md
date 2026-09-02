# alo browser — features.md

Feature inventory. Three tiers, matching the stages in `ROADMAP.md`:
**[1]** = renders alo · **[2]** = renders the modern web · **[3]** = the legacy
tail. **★** marks the things no other engine offers.

Rule of the file: **nothing gets built that isn't listed here, and nothing gets
listed without a tier.** Additions go through the scope gate — this file, the
current stage, and Non-goals below.

The tiers are not a schedule. Stage 1 is the whole of the work for a long time,
and an item marked [2] is a decision that it is *not* stage 1's problem.

---

## The document

- [1] A DOM of our own, built from `html5ever`'s parse events — the tree is ours even though the parser is not
- [1] A stable identity per node, from the first commit, because the agent tree will need to name one and adding identity later means rewriting everything holding a reference
- [1] Fragments and malformed input produce a usable tree rather than an error
- [2] The DOM APIs a modern page actually uses — driven by pages that fail, never by a specification listing a method
- [2] Mutation, so scripting has something to mutate
- [3] The legacy surface: `document.write`, live collections, the rest

## Style

- [1] Stylesheets parsed with `cssparser` into rules we hold
- [1] Selector matching with `selectors` — the modern subset only
- [1] ★ **The cascade, inheritance and `var()`.** alo's design system is custom properties throughout, so an engine that cannot resolve them renders nothing of alo at all. This is stage 1's first hard requirement, not decoration
- [1] A variable cycle is refused rather than looped
- [1] Lengths as numbers: every unit CSS has that does not need a window, `calc()` type-checked and evaluated, and `em` and `rem` against the font size actually in force. A percentage is carried rather than resolved, because what it is a percentage *of* is layout's to say — and `calc(100% - 2rem)` reaches layout as an expression and is resolved there, against the basis the running algorithm knows (ADR 0004)
- [1] Colours as channels — hex, `rgb()`, `hsl()`, the named colours. Blocks paint rather than layout
- [2] Viewport units. They are relative to a window, and until there is one there is nothing true to say
- [1] An unknown property is kept and ignored rather than dropped, so a later stage can implement it without re-parsing
- [1] Media queries for width, and `prefers-color-scheme` — the light and dark the workspace already ships
- [2] Animations and transitions
- [2] Container queries, `:has()`, cascade layers
- [3] Vendor prefixes, and anything that exists only for a page written before 2015

## Layout

- [1] The box model — content, padding, border, margin, and the box's *meaning* alongside its rectangle (ADR 0002)
- [1] Box generation: `display: none` removes a subtree, `display: contents` removes a box and keeps its children, and a container whose children are a mix of block and inline grows the anonymous boxes that make them one kind
- [1] A user-agent style sheet — what an element looks like before anybody says otherwise. The modern elements only; no defaults for what we do not lay out
- [1] **A block-level box inside an inline one, broken around it the way the specification says** — a piece on each side, the block a sibling of the anonymous blocks they sit in, so a background stops and starts again rather than running straight through
- [1] **An inline box's own border and padding**: horizontal ones take room on the line, vertical ones draw without changing its height, and a box that wraps is one rectangle per line with its start border on the first piece and its end border on the last
- [1] An *empty* piece of such a break keeps its border, and costs no line when it has none — CSS's zero-height line box
- [1] **Flexbox and grid**, on `taffy` behind our own boundary. One file may name it
- [1] Absolute and relative positioning, `z-index`, stacking
- [1] Overflow and scrolling regions
- [1] Inline formatting: a line of text and the boxes in it, with breaking and baselines
- [1] Layout is asserted in **numbers** — the computed box — never by eyeballing an image
- [2] Writing modes, and layout that is right-to-left rather than mirrored afterwards
- [2] Multi-column, `position: sticky`
- [3] **Floats as layout, CSS-table layout, quirks mode.** Deliberately last, and possibly never: refusing these is what makes the scope survivable

## Text

- [1] Shaping with HarfBuzz — `rustybuzz`, the Rust port, so there is no C in the process — and font rasterisation. Rented, as every engine rents them
- [1] The fallback chain: a font is *asked* whether it has the character, never guessed at from a language tag
- [1] **The awkward scripts before the easy ones.** A pipeline that assumed left-to-right and one glyph per character is a pipeline that gets rewritten
- [1] Line breaking, and the fallback chain when a font lacks a glyph
- [1] Web fonts
- [2] Bidirectional text end to end
- [2] Selection, carets, and text input
- [2] Hyphenation, `text-wrap: balance`

## Paint

- [1] Glyph rasterisation: an outline read from the font, scaled, and filled into coverage — how much of each pixel a letter covers
- [1] One shape type and one rasteriser, so a glyph and the box behind it agree along the edge they share
- [1] A display list from the box tree
- [1] **A software rasteriser to a PNG.** Deterministic and diffable, needing no GPU and no window — which is what makes every item testable from the first one
- [1] Rounded corners, and clipping to them — one question asked twice: what shape is this box
- [1] **Shadows and gradients**: `box-shadow` (with `inset`), `text-shadow`, `linear-gradient`, `radial-gradient` — a shadow is coverage blurred, so what is behind it is not blurred with it
- [1] **Transforms and opacity**: `translate`, `scale`, `rotate`, `skew`, `matrix`, about a `transform-origin`; `opacity` as a group drawn once and composited once, never box by box
- [1] **Reference renders**: a committed corpus, each with its expected image *and* its expected box tree
- [1] Hardware acceleration — after the software path is correct, never before
- [2] Compositing layers, and scrolling that does not repaint the world

## ★ The agent surface (ADR 0002)

The reason this exists rather than a faster fork of somebody else's engine.

- [1] ★ **The layout tree read as roles, states, positions and text** — a *view* of the tree that draws the page, never a parallel structure. Two structures eventually disagree, and the agent acts on whichever is wrong
- [1] ★ Roles are **declared, not inferred**. A box says it is a list, a row, a field, a button — guessing that from appearance is what screen-scraping already does badly
- [1] ★ **Typed verbs**: activate, put text, scroll. **No verb takes a coordinate**, because a coordinate is a guess about a layout that may have moved between the reading and the acting
- [1] ★ **One element, one thing to read** — an inline box broken around a block is read as one node, named by everything the element contains and positioned everywhere it was drawn, with the block inside it rather than beside it
- [1] ★ Reading is never watching — the tree is exposed when asked, and `alo-os`'s capability model decides who may ask
- [1] ★ **The same tree is the accessibility tree.** A screen reader and an agent want identical facts, and two implementations would guarantee one is wrong — so EN 301 549 conformance and agent capability are one piece of work, not two competing budgets
- [2] ★ The same tree over ordinary web pages, which no browser can offer today because none of them owns both halves
- [2] Assistive technology bridges — AT-SPI on Linux — over that same tree

## Embedding

- [1] A surface alo OS's shell can render into
- [1] alo's sign-in screen, then Settings, rendering correctly — the exit gate of stage 1
- [2] Several documents at once, the shape tabs need

## Networking, scripting and safety — stage 2

- [2] **The process and sandbox model, designed before the first hostile page is ever loaded.** One process per site, renderers with almost no privilege. Every browser that retrofitted this suffered for years, and it is the one item here that genuinely cannot be added later
- [2] HTTP with `rustls`, caching, and cookies with defaults that are not hostile
- [2] ★ **A JavaScript engine, ours, in Rust** — a correct interpreter first, a JIT much later or never. Stage 1 needs none at all, which is what removes the largest component of a browser from the critical path
- [2] Images and media, through rented codecs
- [2] Canvas
- [2] Chrome: tabs, address bar, history, downloads

## Non-goals

**No text shaper, font rasteriser, codec or TLS stack of our own** — we rent the
physics, as Chromium, Firefox, Servo and Ladybird all do, and not out of
timidity. **No fork** of Chromium or Ladybird. **No `unsafe`** outside a
reviewed, named boundary with an ADR. **No conformance-percentage target** — the
measure is whether alo renders correctly, because a Web Platform Tests score
grades us against the legacy we are deliberately refusing. **No extensions, no
sync, no mobile port** until stage 2's exit gate is met; they are product
features with a good story attached. **No plugin-shaped agent** bolted on
afterwards — that is what every other AI browser already is, and ADR 0002 exists
to prevent it.
