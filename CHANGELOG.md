# Changelog

What changed, in words a person outside this repository can read. Newest first.

---

## Unreleased

- **★ An agent can read the interface as what it is.** "Invoice list, twelve
  rows, row three selected" — the sentence `docs/decisions/0002` opens with —
  is now a question this engine answers about a page it rendered, by role and
  by name, with no screenshot anywhere in the chain and no second tree to
  disagree with the first.
- It is a **view**, not a structure. Nothing is built: a node is a box's id and
  a borrow of the trees that already draw the page, and every question is
  answered from them when it is asked. If the two could disagree, an agent
  would eventually act on something that is not on screen.
- **The same view is the accessibility tree.** A screen reader and an agent
  want identical facts, and two implementations would guarantee one is wrong.
- A `<div>` that means nothing is read through, exactly as a screen reader does
  — a page is mostly `<div>`s, and a tree that showed them would bury the rows
  an agent is looking for. What the author hid with `aria-hidden` is not read
  at all.
- **A node knows whether it is on screen.** That is the thing a DOM cannot say:
  a scrolled-away row looks identical to a visible one in it.
- Every corpus case now pins what an agent reads beside what a person sees, so
  the two cannot drift apart without a test noticing.
- The agent tree found a layout bug on its first run: a space that was its own
  text box took no room, so `All` and `Due` touched, and `small and` read
  `smalland`.

- **There is a reference corpus.** Six cases — an invoice list, a rounded card,
  wrapping prose, a flex row, a grid, and three font sizes on one line — each a
  directory holding what to render and four expectations: the box tree it
  builds, where every box ends up, what is drawn, and what it looks like. A
  fifth file records everything the engine refused, so a case that renders
  oddly says why.
- **A failure names the case, the expectation and the line.** The corpus
  reports every case that differs and every expectation inside it, all at once,
  rather than the first one and then stopping — because a change usually shows
  up in more than one of them and finding that out should not take four runs.
- The expectations are **files**, so a change is a diff a person reads rather
  than a failure they have to reproduce.
- The corpus found its first bug on the day it was written: a box with rounded
  corners and no border was pushing a clip for the border it did not have.

- **A box can be round.** `border-radius` changes what shape a box is, and
  `overflow: hidden` clips what is inside it to that same shape — one question
  asked twice. A card with rounded corners now clips its banner to them, which
  is the second reference render.
- A border of one width and colour all the way round is drawn as a **ring** —
  the box's shape with the box's shape inside it, wound the other way — so it
  follows the corners. Four rectangles would have square corners over a rounded
  background, which is exactly what the first attempt looked like.
- Radii that ask for more room than an edge has are scaled down together rather
  than clamped one at a time, which is what CSS says and what keeps a shape's
  proportions instead of making one side rounder than another.

- **The engine draws.** A laid-out document becomes a display list, the list
  becomes pixels, and the pixels become a PNG — and the first reference render
  is committed: a list of invoices, with a heading, three rows, separators and
  a selected row highlighted, drawn from HTML and CSS with real fonts.
- **A picture that changes says what changed.** The display list is compared
  first, in words — "fill box#8 rgb(228 228 231) at (8, 64) 184×25" — and then
  the pixels. A failure reads "the row's background moved four pixels" rather
  than "the image differs", which is the whole reason there is a list in the
  middle.
- **Text is now measured at the size it is set in.** It was measured at one
  size for the whole document, so a heading and a caption were laid out as
  though they were the same — caught by the first picture, which is what a
  picture is for.
- Paint order is decided once, in the display list: backgrounds and borders
  before content, parents before children, and positioned boxes after
  everything in the flow, ordered by `z-index` and then by which came first.

- **A letter has a shape.** A font's outlines are read, scaled and turned into
  coverage — how much of each pixel a letter covers, anti-aliased. An `l` is a
  vertical bar, an `H` has a gap at the top and none in the middle, and a space
  covers nothing; those are the assertions, because a mask is better checked by
  saying what shape it is than by committing a picture of it.
- Everything this engine draws is one shape type, so a glyph and the box behind
  it come out of the same rasteriser with the same anti-aliasing — which is
  what stops them disagreeing along the edge they share.
- Coverage is not colour. A mask says how much of each pixel is covered and
  nothing about what colour it is, which is why the same mask serves black text
  on white and white text on black, and can be reused for a shadow rather than
  rasterised twice.
- The Y axis turns over in one place: a font measures up from the baseline and
  a screen measures down from the top, and the single minus sign that reconciles
  them lives at the boundary so that it lives nowhere else.

- **Colours are channels.** Hex in every length CSS allows, the named colours,
  `rgb()` and `hsl()` in both the modern and the legacy form, `transparent`,
  and `currentColor` — which is carried as itself until there is an element to
  ask, because it is the initial value of every border and folding it into
  black at parse time would draw them all wrong.
- Channels are floats rather than bytes, because compositing multiplies and
  adds them and doing that in eight bits loses a little every time — which
  shows up as banding in exactly the gradients a design system uses. Bytes come
  back at the very end.
- `oklch`, `lab`, `color()` and `color-mix()` are refused rather than
  approximated: they are a different colour space, and a colour converted by
  guesswork is a wrong pixel that looks nearly right.

- **Text and boxes share a line properly.** A sentence spread over several
  `<span>`s is one sentence and wraps between any two of its words, not only
  around the spans; everything on a line sits on one baseline, so a tall image
  beside small text pushes the line down rather than the text up; and a link
  broken across two lines is **two rectangles**, not one covering the gap
  between them. None of those is expressible as a row of boxes, which is what
  the stand-in was.
- An `inline-block`, an image or a button on a line is laid out on its own and
  placed whole — the same layout code, called again, one formatting context
  down.
- Layout now reports the pieces a box was drawn in as well as where it is.
  Where it *is* is the union of the pieces; what should be **drawn** is the
  pieces, and a background painted from the union would cross the gap between
  two lines.

- **The engine works out how wide text is.** Fonts load, `rustybuzz` shapes
  them, UAX #14 says where a line may break and we decide where it does, and
  layout's measuring seam is filled — so a paragraph in a narrow window now
  takes more lines than the same paragraph in a wide one, with numbers that
  came from a font rather than from a guess.
- **The awkward scripts work, and they were done first.** Arabic joins and runs
  right to left; `e` followed by a combining acute is one glyph, not two.
  Nothing in the pipeline is indexed by character — a glyph names the *byte
  range* it came from, several glyphs can name the same range, and one glyph
  can cover several characters. A pipeline that assumed otherwise is one that
  gets rewritten, so it never assumed it.
- **A font is asked whether it has a character**, never guessed at from a
  language tag, and a character no font has takes no room rather than being
  drawn as something it is not.
- Breaking is UAX #14 rather than "split on spaces", which would have been
  wrong for most of the world's writing — Thai has no spaces and a hyphen is a
  break that is not one. A word wider than its line overflows rather than being
  cut in half.
- `rustybuzz` and `unicode-linebreak` each stay behind one file, and the gate
  checks it.

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
