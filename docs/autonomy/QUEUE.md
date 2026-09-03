# Queue

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
**alo's** own sign-in screen and Settings, rendering correctly — an alo screen
is alo's whichever repository it lives in, and `alo-workplace`'s are checked out
beside this one. Their colours come from `alo-workplace`'s `tokens.css`, and
that file is the specification for what "correct" means here.

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

- [x] **21. An empty piece of a broken inline keeps its border.** CSS keeps a
  piece with nothing in it — *"even if either side is empty"* — and an empty
  inline with a border draws that border. This engine dropped it, because its
  inline formatting would have given it a line box of the font's height and
  that is a visible gap where CSS asks for none. **Cut from item 13**: the
  piece that holds something is the case a page actually has, and the empty one
  needs the zero-height line-box rule first.

  **Done, with the rule it was waiting for.** A line box holding no text, no
  preserved space and no inline box with a margin, padding or border is
  zero-height and treated as not existing — so the piece is kept, and costs a
  line only when it has something to draw. Of the pieces of a broken inline the
  agent reads the first with anything in it, so an empty piece is never a
  second link. Corpus case `empty-piece`.

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

- [x] **23. An agent reads a broken inline as one whole thing.** A `<div>`
  inside an `<a>` is still inside the link for a person and for a click, but
  the box tree has broken the link into pieces with the block a sibling of
  them. **Cut from item 13.**

  **Done, and it stayed a view.** The box tree records which boxes belong to
  which whole — a later piece says which box it continues, a block says which
  inline box it was taken out of — and the reader follows what is already
  there rather than building anything. One node, named by everything the
  element contains, positioned everywhere it was drawn, with the block read
  inside it. Corpus case `broken-link`. It also fixed a name that had nothing
  to do with breaking: a name now gets a space where a block begins or ends,
  so alo's own headline reads as three sentences rather than one run-on.

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

## Stage 1's last line is this queue's work after all

This section used to say stage 1's remaining work was **"blocked on something
outside this repository"**, because the exit gate named `alo-os`'s screens and
`alo-os` is not checked out here. That was a fact about where a repository sits
on a disk, not about the engine — and acting on it sent the loop into stage 2
with stage 1 unfinished.

The gate is corrected: it names **alo's** screens, and an alo screen is alo's
whichever repository it lives in. `alo-workplace`'s are checked out beside this
one. So what remains is ordinary engine work, it belongs here, and **stage 1 is
finished before stage 2 is continued.**

*Numbered 44 to 46 rather than 20 to 22: those numbers are already taken by
finished items, and `ROADMAP.md` refers to queue items by number. Two items with
one number is a reference that quietly points at the wrong thing.*

- [x] **44. `clamp()`, `min()`, `max()` and the viewport units.** The first of
  the four substitutions in `crates/alo-corpus/cases/alo-sign-in/`: the
  headline's `font-size: clamp(2.4rem, 4vw, 3.5rem)` was written out by hand as
  `2.5rem`, because the engine had neither piece.

  **Done.** The four math functions are one family and parse as one, so they
  nest; a viewport unit needs a window and answers zero rather than a plausible
  number when there is none. **The committed reference render did not change**,
  which is the best evidence the substitution was faithful. *Cut from the
  original item 44, which asked for all four substitutions at once — the queue's
  own instruction was to cut it rather than leave one in place.* The other three
  are items 47, 48 and 49.

- [x] **47. `white-space`.** The sign-in headline is one string with newlines
  in it; the case substituted three `<span>`s made blocks. Cut from item 44.

  **Done, and it was larger than the item said.** `pre-line` needs whitespace
  *processing*, and the engine did none: it shaped whatever bytes the parser
  handed over, so `one   two` was three spaces and an indented paragraph was
  drawn with its indentation. All five values are implemented, `<pre>` preserves
  its whitespace for the first time — the user-agent sheet had said so since it
  was written and nothing read it — and collapsing happens when the box is built
  so that layout, paint and the agent tree read the same text.

- [x] **48. `letter-spacing`.** Extra space after every character, which
  changes what a run of text measures and therefore where every line breaks. It
  reaches `alo-text`'s measuring rather than only paint. Cut from item 44.

  **Done.** Applied after shaping rather than inside it, so the `rustybuzz`
  boundary is untouched: shaping is the rented part and letter spacing is a CSS
  decision about the result. alo's headline is four lines instead of five. It
  still wraps one line more than the real screen, and that is the *font* — the
  corpus renders in DejaVu Sans and the app loads Inter, which is narrower.
  Web fonts are stage 2.

- [ ] **49. `transition`, `:hover` and `:focus-visible`, accepted rather than
  dropped.** The case deletes them. On a static render of a settled page they
  change nothing — a transition has run, and nothing is hovered or focused
  because there is no pointer and no focus. So what is owed is that the engine
  **reads** them without recording a refusal, and that `:hover` and
  `:focus-visible` match nothing rather than being an unparseable selector that
  drops the whole rule. Nothing here claims animation; that needs a clock, and a
  clock is not stage 1's. Cut from item 44.

- [ ] **45. alo's Settings screen in the corpus.** The second screen the gate
  names, and it is not rendered at all. Same shape as the sign-in case: its own
  markup, its own rules, colours from `tokens.css`, a committed reference render
  and an expected box tree.

- [ ] **46. An agent reads Settings and activates a row by name.** The last
  clause of the exit gate. Reading works on pages we wrote, and a verb finds its
  target and reports what it decided — but it does not yet write back to the
  document, which was item 42. **Item 42 is done**, so this one is unblocked: a
  verb changes the page now. What this adds beyond 42 is asserting it against a
  real alo screen, which is where
  a role declared wrongly actually shows up.

**Still genuinely not this queue's**, and now recorded in `ROADMAP.md` outside
the stage 1 list so they cannot block it: hardware acceleration (needs a GPU),
embedding into alo OS's shell (needs a compositor that does not exist), and any
claim about speed (measured on hardware, or not made).

---

# Queue — stage 2

`ROADMAP.md`: **it renders the modern web.** The order below is the roadmap's,
and the roadmap's own reason is at the top of it — the process model is first
because it is the one thing that cannot be added later.

**Staged by what it renders**, still. An item earns its place by a page that
fails without it, not by a specification that lists it.

## Ready

- [x] **24. ADR 0005 — the process and sandbox model.** The decision, written
  before any code depends on it. What runs where, what a renderer is allowed to
  touch, what crosses the boundary and in which direction, and what happens when
  a renderer dies. `ROADMAP.md` is blunt: *"Every browser that retrofitted this
  suffered for years, and it is the one thing here that cannot be added later."*
  An ADR rather than an item of code because the expensive part is the shape,
  and the shape is a decision.

  **Done: `docs/decisions/0005-one-process-per-site-and-a-sandbox-we-rent.md`.**
  It answers the question this project has to answer before it copies anybody's
  process model — why a memory-safe engine still needs one. Spectre is a
  hardware property no language prevents; the codecs and TLS we rent are not
  ours to make safe; the same-origin policy is code we write; and a page must
  not be able to end the session.

- [x] **25. The engine behind a message boundary.** The renderer becomes a thing
  that is *sent* work and *returns* results — a typed protocol, no ambient
  state, no synchronous call back into whoever asked. In one process to begin
  with, because what is expensive to retrofit is the **shape**, not the
  `fork`. Every later item is written against this boundary, which is what makes
  item 29 a change of transport rather than a rewrite.

  **Done: the `alo-renderer` crate.** `Renderer::handle` takes work and returns
  a result — no callback in the signature, nowhere to wait. Every message is
  owned, `Clone` and `Send + 'static`, asserted by a test, because a message
  holding a borrow compiles today and cannot be sent tomorrow. The pipeline
  moved out of the corpus so there is one of it. **It found item 42** by trying
  to use the boundary end to end.

- [ ] **26. A URL, and loading what needs no network.** A URL we hold and can
  resolve against a base; a request, a response, a content type, and a decode to
  text with the encoding sniffed the way HTML says. `file:` and `data:` only.
  This is the loading pipeline with the network left out, so that the network is
  one implementation of something already tested.

- [ ] **27. HTTP, and TLS we rent.** `rustls` for TLS — ADR 0001's rule, and
  nobody writes their own — behind our own boundary like every other rented
  crate. Redirects, status codes, and a body that arrives in pieces.

- [ ] **28. A cache, and cookies with sane defaults.** Sane means: `SameSite`
  by default, no third-party cookies, and a cache that obeys what a server
  actually said rather than what it meant.

- [ ] **29. The process split, and the sandbox.** One process per site,
  renderers with almost no privilege. Item 25 made this a change of transport;
  this is where it becomes real, with the platform's own sandbox rather than a
  hopeful one of ours.

- [ ] **30. ADR 0006 — our own JavaScript engine.** What it is and, more
  importantly, what it is not: a **correct interpreter first**, a JIT much later
  or never (`CLAUDE.md`, law 4 and the standing rules). Taking somebody else's
  C++ engine would spend the memory-safety argument this project is built on,
  which is exactly why it needs to be written down rather than assumed.

- [ ] **31. JavaScript: source to a syntax tree.** A lexer and a parser for the
  language a modern page actually ships, with automatic semicolon insertion and
  the awkward grammar — regular expressions against division, arrow functions
  against parenthesised expressions — settled at parse time rather than guessed
  at later.

- [ ] **32. JavaScript: values, and an interpreter that walks the tree.**
  Numbers, strings, `undefined` and `null`, the abstract operations that convert
  between them, scopes, and calls. Correct before fast: a tree walk is the thing
  whose behaviour can be checked against the specification line by line.

- [ ] **33. JavaScript: objects, prototypes, closures, `this` and exceptions.**
  The half of the language a page actually depends on, and the half that is
  hardest to add afterwards because everything else is written assuming it.

- [ ] **34. The event loop, tasks and microtasks.** A page is not a program that
  runs and stops; it is a queue. Ordering here is observable to every script on
  every page, and getting it wrong is a class of bug that looks like a race.

- [ ] **35. The DOM, from JavaScript.** The bindings: a document a script can
  read and change, and changes that reach style, layout and paint through the
  boundary item 25 built rather than around it.

- [ ] **36. The DOM APIs a modern page actually uses.** *Driven by pages that
  fail* — the roadmap's words. Each one arrives because a real page needed it,
  and the page goes in the corpus with it.

- [ ] **37. Images.** Decoding is rented (ADR 0001: nobody writes their own
  JPEG); `<img>` laid out with its intrinsic size and aspect ratio, and drawn.

- [ ] **38. Canvas.** A drawing surface a script owns. The rasteriser already
  exists; this is the API over it and the compositing rules around it.

- [ ] **39. Media.** Audio and video, codecs rented, with the parts that are
  ours being the element, its states, and how a frame reaches the display list.

- [ ] **40. Chrome: tabs, address bar, history, downloads.** The browser as a
  thing a person uses, which is what stage 2's exit gate measures.

- [ ] **41. ★ The agent on ordinary web pages.** The same tree and the same
  typed verbs, on pages nobody wrote for us. ADR 0002 holds or it does not, and
  this is where it is found out.

- [x] **42. ★ A verb changes the page.** `perform` finds its target, refuses
  what cannot be operated, and reports what it decided — and then nothing
  happens: the document is never written back to, so a field that was "typed
  into" reads the same afterwards. **Found by item 25**, which tried to use the
  boundary end to end and discovered that the second half of a verb does not
  exist. It needs a document that can be changed and a pipeline that runs
  again, and it is the difference between an agent that can *drive* an
  interface and one that can only describe what it would do.

  **Done, and taken before item 26** because it is a correctness gap in the ★
  agent surface, which `CLAUDE.md` calls the reason this project exists rather
  than a faster fork of somebody else's engine. Deciding and changing are two
  steps and the types say so: the decision is made against the tree the agent
  read, `alo_agent::apply` changes the document, and the page is rendered again
  **from the same document** so that ADR 0003's promise survives. It found three
  more things, all fixed: a `<label>`'s words were read twice and made every
  labelled field ambiguous; a password field was in no tree at all; and a field
  did not show what it held. Corpus case `a-filled-form`.

- [ ] **43. A form control draws its state.** A checked checkbox draws the same
  box as an unchecked one — no tick, no focus ring, no radio dot. The state is
  right in the tree and wrong on the screen, which is the worse way round: an
  agent is correct and a person looking at the same page is misled. **Found by
  item 42**, which made checking possible and then had nothing to show for it.

**Exit gate** (`ROADMAP.md`): a person uses it as their browser for a week and
reaches for another one only for a site they can name.

---

## After stage 2

## Never in this queue

- **A JavaScript engine.** Stage 2, and only when a page we need requires it.
- **Legacy compatibility.** Quirks mode, floats-as-layout, CSS-table layout, the
  old DOM surface. Refusing these is what makes the scope survivable.
- **`unsafe`**, without an ADR.
- **Anything measured against a conformance percentage** rather than against alo.
