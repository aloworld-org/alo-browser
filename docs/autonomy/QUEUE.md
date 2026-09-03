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

- [x] **49. `transition`, `:hover` and `:focus-visible`, accepted rather than
  dropped.** The case deletes them. On a static render of a settled page they
  change nothing — a transition has run, and nothing is hovered or focused
  because there is no pointer and no focus. So what is owed is that the engine
  **reads** them without recording a refusal, and that `:hover` and
  `:focus-visible` match nothing rather than being an unparseable selector that
  drops the whole rule. Nothing here claims animation; that needs a clock, and a
  clock is not stage 1's. Cut from item 44.

  **Done, and it needed no new code.** The engine already read all three and
  already made an interaction state match nothing; what was owed was finding
  that out and putting the rules back. That is the case for the item existing:
  a substitution nobody re-checks outlives the reason for it. **alo's sign-in
  screen now renders from its own stylesheet with no substitutions at all.**

- [x] **45. alo's Settings screen in the corpus.** The second screen the gate
  names, and it was not rendered at all. Same shape as the sign-in case: its own
  markup, its own rules, colours from `tokens.css`, a committed reference render
  and an expected box tree.

  **Done, with no substitutions**, and it found two engine defects on the way —
  both the same root, and both now fixed: a form control needs a **box of its
  own** to hold what it shows. The user-agent sheet had been centring a button's
  label with `justify-content`, which an author who made a button a flex
  container could not override (alo's settings nav is exactly that), and giving
  every `<input>` a fixed height, which became too *short* once a field showed
  its value.

- [x] **46. An agent reads Settings and activates a row by name.** The last
  clause of the exit gate. Reading works on pages we wrote, and a verb finds its
  target and reports what it decided — but it does not yet write back to the
  document, which was item 42. **Item 42 is done**, so this one is unblocked: a
  verb changes the page now. What this adds beyond 42 is asserting it against a
  real alo screen, which is where
  a role declared wrongly actually shows up.

  **Done: `crates/alo-renderer/tests/an_agent_on_settings.rs`.** It reads the
  screen from the corpus case, so the test and the committed render look at the
  same thing. It also found that `aria-current` was dropped — alo's nav says
  which section is open and the tree could not say it — so that is read now. And
  it says out loud what a nav row does *next*: nothing, because that is the
  page's own code and stage 1 has none.

  **Stage 1's exit gate is met with this item.**

**Still genuinely not this queue's**, and now recorded in `ROADMAP.md` outside
the stage 1 list so they cannot block it: hardware acceleration (needs a GPU),
embedding into alo OS's shell (needs a compositor that does not exist), and any
claim about speed (measured on hardware, or not made).

---

# Queue — stage 2

`ROADMAP.md`: **it renders the modern web.** That file names the whole of it in
about ninety lines and is blunt about the size — *"years of work, and naming it
completely is the point"*. This is the same list as items a loop can take, in
the order dependencies allow.

**Read `docs/autonomy/LOOP.md`'s stage 2 section before the first one.** Four
things are different from stage 1 and none of them is optional: a real page
decides and the page is **frozen**; the bytes are **hostile** now, so anything
that parses them gets a malformed-input test and must return an error rather
than panic; order follows **dependencies** rather than the file; and a decision
gets an **ADR as its own iteration**.

**Numbering starts at 50.** Stage 2 was first sketched as sixteen coarse items
numbered 26 to 41, before `ROADMAP.md` grew the real list. Those numbers are
retired rather than reused: `STATE.md` refers to some of them, and two items
with one number is how a reference quietly starts pointing at the wrong work.

**What closes an item** is written into it. An item that cannot say what closes
it is not ready — mark it `needs design` and take the next one.

---

## A. The network

The origin is what every security decision below is made against, so URLs come
first. Nothing here needs JavaScript.

- [ ] **50. URLs, properly.** WHATWG parsing, resolution against a base, and the
  **origin** as a value other code compares. IDNA and punycode with it, because
  a look-alike domain is a security bug rather than a display one.
  *Closes when:* a table of the WHATWG URL test cases parses to the same
  answers, and an origin compares equal only when it should.

- [ ] **51. Fetching what needs no network.** The shape of a load — a request, a
  response, a status, headers, a content type, a body — with `file:` and `data:`
  as the only schemes. Encoding sniffed the way HTML says rather than assumed to
  be UTF-8.
  *Depends on 50. Closes when:* the renderer loads a page from a path rather
  than from a string handed to it, and a mislabelled encoding still reads.

- [ ] **52. TLS with `rustls`.** Rented (ADR 0001), behind its own file like
  every other rented crate. A certificate error is a **decision a person makes**,
  not a dialogue they click through: the error says what is wrong and what
  trusting it would mean.
  *Depends on 51. Closes when:* a good certificate connects, a bad one is
  refused with a reason in words, and the refusal is not bypassable by default.

- [ ] **53. HTTP/1.1**, with connection pooling and keep-alive. A response body
  that arrives in pieces, and a request that can be cancelled.
  *Depends on 52. Closes when:* a frozen page's own byte stream replays through
  it identically, and a truncated response is an error rather than a short page.

- [ ] **54. Content encodings**: gzip, brotli, zstd, rented.
  *Depends on 53. Closes when:* each round-trips, and a corrupt stream is
  refused rather than decoded into rubbish.

- [ ] **55. Redirects, byte ranges, and downloads that resume.** Redirect loops
  bounded, cross-origin redirects losing what they should.
  *Depends on 53.*

- [ ] **56. The HTTP cache, with real semantics** — freshness, revalidation,
  `Vary`. `ROADMAP.md`: *"subtly wrong here is invisible for months and then
  serves somebody a stale bank page."*
  *Depends on 53. Closes when:* a table of responses and clocks produces the
  right hit, miss and revalidate for each, including the ones that are only
  wrong an hour later.

- [ ] **57. Cookies, partitioned by default.** `SameSite`, `Secure`,
  `HttpOnly`. **The default is a product decision** rather than a parser detail,
  so it is written down where a person can argue with it.
  *Depends on 50, 53. Needs ADR* — what a default costs and who it protects.

- [ ] **58. DNS, and encrypted DNS as a choice somebody made** rather than a
  default nobody was told about.
  *Depends on 53. Needs ADR* — the same argument as 57, about a different
  server seeing every name you look up.

- [ ] **59. HTTP/2**, once 1.1 is correct.
  *Depends on 53.*

- [ ] **60. HTTP/3 and QUIC**, once both of those are.
  *Depends on 59.*

## B. Origins, and the model that keeps sites apart

ADR 0005's four reasons, made real. Three of these are code we write and can get
wrong, which is the argument for the fourth.

- [ ] **61. The same-origin policy, CORS and preflight.**
  *Depends on 50, 53. Closes when:* a cross-origin read that should fail does,
  in a test that names the attack rather than the header.

- [ ] **62. Content Security Policy, referrer policy, HSTS, mixed-content
  blocking.**
  *Depends on 61.*

- [ ] **63. The process split, and the sandbox.** One process per site,
  renderers with almost no privilege, the platform's own sandbox rather than a
  hopeful one of ours — seccomp-bpf and user namespaces on Linux, Seatbelt on
  macOS. ADR 0005 decided it; `alo-renderer` made it a change of **transport**
  rather than a redesign.
  *Depends on 51 — a sandboxed renderer cannot fetch, so the browser process
  must be able to. Closes when:* two sites are two processes, a renderer cannot
  open a file, and killing one leaves the other running.
  *This is the roadmap's "queue item 29", renumbered with the rest.*

- [ ] **64. The transport, and the lifecycle** that starts, reuses and reaps
  renderers, with a bound on how many exist.
  *Depends on 63.*

- [ ] **65. A renderer that dies takes its tab and nothing else** — and says so,
  rather than leaving a blank rectangle.
  *Depends on 63. Closes when:* a renderer is killed from outside and the tab
  says what happened while every other tab keeps working.

- [ ] **66. Where one site ends and another begins.** The origin, the site, the
  registrable domain, and which of them gets a process.
  *Depends on 50, 63.*

- [ ] **67. ★ Every request attributable** — which page, and **which agent
  action**, caused it. `ROADMAP.md`: *"no other engine has needed to answer
  that, and an agent-driven browser that cannot is one nobody should trust."*
  *Depends on 53. Needs ADR* — what is recorded, for how long, and who may read
  it, in the shape of `alo-os` ADR 0001.

## C. Pages that are not ours

- [ ] **68. The web corpus.** A second kind of case beside the alo ones: a page
  from the web, **frozen** — its bytes as they were, with where they came from
  and when, and its own expected trees and render. Never fetched at test time,
  for the reasons `LOOP.md` gives.
  *Depends on 51. Closes when:* one real page renders, is diffed on every run,
  and the suite still passes with the network unplugged.

## D. JavaScript, ours, in Rust

The long pole, and the thing most of section E is unreachable without.

- [ ] **69. ADR 0006 — our own JavaScript engine.** *Needs ADR, and it is the
  first item here.* What it is: a bytecode compiler and an interpreter,
  **correct first**. What it is not: a JIT, until there is a measured reason and
  an ADR weighing the speed against the attack surface. Why it is ours at all:
  taking somebody else's C++ engine spends the memory-safety argument this
  project is built on (ADR 0001), and spending it quietly is worse than not
  having made it.

- [ ] **70. Lexer and parser to a syntax tree** — the language pages actually
  ship, not ES5. Automatic semicolon insertion, and the grammar that needs a
  decision rather than a guess: a regular expression against division, an arrow
  function against a parenthesised expression.
  *Depends on 69. Closes when:* a frozen page's own script parses, and every
  ambiguity above is settled at parse time rather than later.

- [ ] **71. The object model, and a garbage collector.** Objects, properties,
  prototypes, and something that reclaims them.
  *Depends on 69. Needs ADR* — a collector is a decision about pauses.

- [ ] **72. A bytecode compiler and an interpreter.** Values, scopes, calls,
  `this`, closures, exceptions.
  *Depends on 70, 71. Closes when:* a suite of small programs produces the
  values the specification says, run as a table rather than as prose.

- [ ] **73. The ECMAScript builtins**, in the order real pages need them rather
  than the order the specification lists them.
  *Depends on 72.*

- [ ] **74. Regular expressions**, with the syntax the language actually has.
  *Depends on 72. Closes when:* a hostile pattern is refused or bounded rather
  than running for ever — a catastrophic backtrack in a renderer is a denial of
  service.

- [ ] **75. Promises, `async`/`await`, generators and iterators.**
  *Depends on 72, 76.*

- [ ] **76. The event loop** — tasks, microtasks, the rendering steps,
  `requestAnimationFrame`. `ROADMAP.md`: *"where 'it works, but the animation
  stutters' is decided."*
  *Depends on 72. Closes when:* the order of a table of interleaved tasks and
  microtasks is what the specification says, because that order is observable to
  every script on every page.

- [ ] **77. Modules**: ESM, dynamic `import()`, and the loader that fetches them.
  *Depends on 53, 72.*

- [ ] **78. Errors and stack traces** good enough to debug somebody else's
  minified page.
  *Depends on 72.*

- [ ] **79. `Intl`, rented** rather than written.
  *Depends on 73.*

## E. The DOM, and the pages that use it

- [ ] **80. Mutation from script**, and the invalidation that has to follow it.
  Adding and removing nodes is still the parser's alone today (`alo-dom` says
  so); this is where that stops being true.
  *Depends on 72. Closes when:* a script changes a document and the next render
  shows it, with node identity surviving (ADR 0003).

- [ ] **81. Events**: capture and bubble, listeners, default actions. **This is
  what makes a button do something**, which every agent verb has been honest
  about not doing since stage 1.
  *Depends on 80. Closes when:* `alo-renderer`'s test that a nav row changes
  nothing fails, and is rewritten to assert what it now does.

- [ ] **82. Forms**: the controls, constraint validation, submission, file
  inputs.
  *Depends on 81.*

- [ ] **83. `fetch()` and `XMLHttpRequest`**, over the same stack as everything
  else rather than beside it.
  *Depends on 61, 72.*

- [ ] **84. WebSocket.**
  *Depends on 53, 76.*

- [ ] **85. Navigation and session history**: `pushState`, back and forward, and
  what survives each.
  *Depends on 80.*

- [ ] **86. `iframe`s and the sandbox attribute** — a document inside a
  document, *"where a great many security bugs live"*.
  *Depends on 61, 63.*

- [ ] **87. Shadow DOM and custom elements.** Component frameworks are not
  optional on the modern web.
  *Depends on 80.*

- [ ] **88. Selection and ranges.**
  *Depends on 80.*

- [ ] **89. CSSOM** — styles readable and writable from script.
  *Depends on 80.*

- [ ] **90. Storage**: `localStorage`, `sessionStorage`, IndexedDB, the Cache
  API, and **one quota policy over all of them**.
  *Depends on 72, 66. Needs ADR* — a quota is a policy about somebody's disk.

- [ ] **91. Workers**: dedicated, shared, and service workers with their fetch
  interception.
  *Depends on 76, 83.*

- [ ] **92. Timers, clipboard, drag and drop.**
  *Depends on 76, 81.*

- [ ] **93. ★ Permissions as capabilities** — camera, microphone, location,
  notifications, in the shape of `alo-os` ADR 0001: enumerated, visible,
  revocable, expiring, recorded. *"A browser is where most people meet a
  permission prompt, and every other one is a dialogue nobody can audit
  afterwards."*
  *Depends on 63, 67. Needs ADR.*

## F. CSS beyond what alo needed

- [ ] **43. A form control draws its state.** *(Kept at its own number: it was
  found in stage 1 and is referenced from `docs/conformance.md`.)* A checked
  checkbox draws the same box as an unchecked one — no tick, no radio dot, no
  focus ring. The state is right in the tree and wrong on the screen, which is
  the worse way round.
  *Closes when:* a corpus case shows a ticked box, a chosen radio and a focused
  field, each differing from its resting state.

- [ ] **94. Animations and transitions.** Stage 1 reads them and they change
  nothing, which is correct for a still picture; this is the clock.
  *Depends on 76.*

- [ ] **95. Container queries, `:has()`, cascade layers, `@property`.**

- [ ] **96. Filters, `backdrop-filter`, blend modes, masks, `clip-path`.**

- [ ] **97. `position: sticky`, multi-column, scroll snap, overscroll
  behaviour.**

- [ ] **98. Writing modes, and layout that is right-to-left** rather than
  mirrored afterwards.

- [ ] **99. Paged media and print styles.**
  *Depends on 98 for anything that is not left-to-right.*

## G. Text, properly

- [ ] **100. Bidirectional text end to end.** Stage 1 shapes it; this makes
  selection, caret movement and editing behave.
  *Depends on 88.*

- [ ] **101. Selection, carets and text input inside the page.**
  *Depends on 81, 88.*

- [ ] **102. Input methods.** *"A browser that cannot take Japanese or Chinese
  input is not a browser in those countries."*
  *Depends on 101.*

- [ ] **103. `contenteditable`**, which every rich text box on the web is built
  on.
  *Depends on 80, 101.*

- [ ] **104. Hyphenation, `text-wrap: balance`.**

- [ ] **105. Web fonts as pages ship them**: WOFF2, variable fonts, and loading
  that does not flash. **This is also what closes the last honest gap in stage
  1's screens** — the corpus renders in DejaVu Sans and alo loads Inter, so its
  headline wraps one line more here.
  *Depends on 53.*

## H. Pictures, and things that move

- [ ] **106. Image codecs, rented**: PNG, JPEG, GIF, WebP, AVIF. ADR 0005's
  second reason for the sandbox is that these have `unsafe` in them and are not
  ours to make safe, so **untrusted bytes are decoded in the least privileged
  process that can do it**.
  *Depends on 63 for where they run. Closes when:* `<img>` lays out with its
  intrinsic size and aspect ratio and draws, and a fuzzed file is refused rather
  than decoded.

- [ ] **107. SVG** — *"a second rendering model inside the first, and far larger
  than its one line here suggests."* **Cut this before starting it**; it is
  several iterations and nobody should discover that halfway through.

- [ ] **108. Canvas 2D.** The rasteriser exists; this is the API over it and the
  compositing rules around it.
  *Depends on 72.*

- [ ] **109. Audio and video playback through rented decoders.**
  *Depends on 63, 106. Needs ADR* — which decoders, on whose licence, and where
  they run. `ROADMAP.md` refuses DRM and proprietary codecs outright.

- [ ] **110. Media Source Extensions**, without which most video sites do not
  play at all.
  *Depends on 109.*

- [ ] **111. Web Audio.**
  *Depends on 72.*

- [ ] **112. WebGL, then WebGPU.** Both large, both late, and neither before the
  software path is right. **Needs hardware to verify**, and says so.
  *Depends on 116.*

## I. Making it fast enough to use

Correctness before speed is the rule everywhere else. These are the items where
being right and being unusable are the same outcome — and **every one of them is
a claim measured on hardware or not made**.

- [ ] **113. Incremental style and layout** — recompute what changed, not the
  document. *"The largest single difference between an engine that renders a
  page and one somebody can use."*
  *Depends on 80. Closes when:* a changed attribute restyles a subtree rather
  than a document, shown by a count rather than by a stopwatch.

- [ ] **114. Compositing layers, and scrolling that does not repaint the world.**
  *Depends on 113.*

- [ ] **115. Off-main-thread scrolling and animation.**
  *Depends on 114.*

- [ ] **116. Hardware acceleration for paint**, once the software path is
  correct. **Needs a GPU to verify.**
  *Depends on 114.*

- [ ] **117. A performance budget somebody can hold us to**: named pages,
  measured, in CI. **Needs hardware**, and until it exists no item in this
  section may claim a speed.
  *Depends on 68.*

## J. The browser itself

What stage 2's exit gate actually measures: a person using it.

- [ ] **118. A window, tabs, and a tab strip.** *Depends on 63, 64.*
- [ ] **119. The address bar**: what somebody typed, what it means, and a search
  that **phones nobody by default**. *Depends on 50, 118.*
- [ ] **120. History, bookmarks, downloads.** *Depends on 118.*
- [ ] **121. Find in page, zoom, and per-site settings that stick.**
- [ ] **122. Context menus, and keyboard operation of every one of them.**
- [ ] **123. Printing, print preview, export to PDF.** *Depends on 99.*
- [ ] **124. Viewing a PDF** — or saying plainly that we hand it to something
  else. *Needs ADR* — it is a decision, not an omission.
- [ ] **125. Private browsing, and profiles that are genuinely separate.**
  *Depends on 90.*
- [ ] **126. Autofill, and credentials held where the operating system holds
  secrets** rather than in a file of ours. *Depends on 82. Needs ADR.*
- [ ] **127. Security surfaces**: certificate detail, permission state, what this
  page has stored — reachable, none of it buried. *Depends on 62, 90, 93.*
- [ ] **128. Settings.**
- [ ] **129. Developer tools**: inspector, console, network, performance. *"A
  browser nobody can debug a site with is not one a developer keeps."* **Cut
  before starting**; it is four products.
- [ ] **130. Accessibility**: AT-SPI over **the same tree the agent reads**
  (ADR 0002), keyboard operation of everything, focus always visible, and the
  EN 301 549 conformance the workspace is already held to. *Depends on 122.*

## K. ★ The agent, on somebody else's pages

The reason this exists rather than a faster fork. ADR 0002 holds here or it does
not, and this is where it is found out.

- [ ] **131. The agent reads and acts on ordinary web pages** through the same
  tree — no screenshot, no scraping, no coordinates.
  *Depends on 68, 81. Closes when:* a frozen real page is read and driven by
  name, with the same verbs and the same refusals.

- [ ] **132. Across frames**, without becoming a way around the same-origin
  policy. *Depends on 86, 131. Needs ADR* — this is a security boundary an agent
  could be used to cross.

- [ ] **133. Under grants, and recorded**: what it read, what it did, on whose
  approval — `alo-os` ADR 0001's model reaching the web.
  *Depends on 67, 93, 131.*

- [ ] **134. Agent-driven navigation, and a page that changes underneath an
  agent mid-action.** The failure ADR 0003 was written for, on a page nobody
  wrote for us. *Depends on 85, 131.*

**Exit gate** (`ROADMAP.md`): a person uses it as their browser for a week and
reaches for another one only for a site they can name. An agent completes a real
task on a site nobody wrote for us, and the record afterwards says what it read
and what it changed.

---

## After stage 2

**Stage 3, the legacy tail**, and **stage 4, the product** — both in
`ROADMAP.md`, neither in this queue yet. Stage 4 is gated: nothing in it starts
until stage 2's exit gate is met. Stage 3 is scheduled by a **broken render on a
site somebody uses**, never by a specification listing a feature, so its items
arrive one at a time with the page that asked for them.

They get a queue when stage 2 is close enough that the order matters. Writing it
now would be planning work whose shape the intervening two years decides.

## Never in this queue

- **Legacy compatibility, before stage 3.** Quirks mode, floats-as-layout,
  CSS-table layout, the old DOM surface. Refusing these is what makes the scope
  survivable, and stage 3 takes them only when a real page forces one.
- **`unsafe`**, without an ADR.
- **Anything measured against a conformance percentage** rather than against alo
  and then against real pages that fail.
- **Anything in `ROADMAP.md`'s "Not built, and not by accident"**: DRM and EME,
  proprietary codecs, telemetry of any kind, a search deal, and our own shaper,
  codec or TLS stack. A later stage adopting one of these quietly is exactly
  what that section exists to prevent.

*(The JavaScript engine used to be on this list, with "stage 2, and only when a
page we need requires it" beside it. Stage 2 is here, so it is item 69.)*
