# Changelog

What changed, in words a person outside this repository can read. Newest first.

---

## Unreleased

- **The loop has rules for stage 2, and stage 2 has a queue.** Stage 1 asked one
  question — *does alo render?* — and answered it with a committed PNG and a
  committed box tree. Stage 2 has neither, so `docs/autonomy/LOOP.md` says what
  replaces them: **a real page decides, and the page is frozen** (a suite that
  fetched would be flaky, would fail on an aeroplane, and would let any site's
  owner break our build); **the bytes are hostile now**, so anything that parses
  them gets a malformed-input test and must return an error rather than panic;
  **order follows dependencies** rather than the file; and **a decision gets an
  ADR as its own iteration**.
- `docs/autonomy/QUEUE.md` gains stage 2 as **86 items** in ten groups, from the
  roadmap's own ninety lines — each with what it depends on and what closes it.
  Eleven are marked as needing an ADR before any code depends on them, and the
  handful that genuinely need hardware to verify say so rather than being
  discovered later.
- Numbering starts at 50. The sixteen coarse items sketched as 26 to 41, before
  the roadmap grew the real list, are **retired rather than reused**: two items
  with one number is how a reference quietly starts pointing at the wrong work.

- **An agent reads alo's Settings screen and acts on it by name** — the last
  clause of stage 1's exit gate. It finds the sections as buttons called what a
  person would call them, knows which one is open, activates one by name with no
  coordinate anywhere in the transaction, ticks the out-of-office box and types
  a date, and is refused when it asks for something the screen does not have.
- **`aria-current` is read.** ADR 0002's own example sentence is "invoice list,
  twelve rows, row three selected"; for this screen it is "which section is
  open", and an agent that could not read it would have to guess from a colour.
  It is a word rather than a flag — `page`, `step`, `date` — because a nav item
  being the current *page* is not the same claim as a cell being the current
  *date*.
- Honest about the edge: pressing a nav row runs the page's own code, and there
  is none in stage 1. The verb finds the row and reports what it pressed, and
  the screen does not change. There is a test that says exactly that, so the
  day scripting arrives it will fail and have to be rewritten on purpose.

- **alo's Settings screen renders**, from its own markup and its own rules, with
  no substitutions — the second of the two screens stage 1's exit gate names.
  Corpus case `alo-settings`.
- **A form control holds what it shows in a box of its own**, the way browsers
  do. That is why a tall button's label sits in the middle of it and why an
  empty field is still one line tall — and it replaces two approximations in the
  user-agent style sheet that the Settings screen walked straight into:
  - The sheet centred a button's label with `justify-content`. An author who
    made a button a flex container — which alo's settings nav does — could not
    override a rule they could not see, so every nav item was centred. **An
    author cannot override the user-agent sheet's flex alignment**, which is why
    this could never have been a rule.
  - The sheet gave every `<input>` a fixed `height: 1.2em`, so that an empty one
    was not a hairline. Once a field showed its value that height was too
    *short* for it, and the text hung out of the box. A minimum, from the box
    the control holds its text in, is right either way.

- **alo's sign-in screen renders from its own stylesheet, with no
  substitutions.** The last of the four was `transition`, `:hover` and
  `:focus-visible`, and removing it needed no new code — the engine already
  read all three and already made an interaction state match nothing. What was
  owed was finding that out and putting the rules back, which is why the item
  existed: a substitution nobody re-checks outlives the reason for it.
- That the rules change nothing is the **correct** answer rather than a missing
  one. A still picture of a settled page is what a transition has finished
  doing, and nothing is hovered or focused because there is no pointer and no
  keyboard. Two tests now pin it, so a later change that started dropping those
  rules would say so.

- **`letter-spacing`.** The third of the four substitutions in alo's own
  sign-in case is gone: the headline's `-0.02em` is the screen's own value now,
  and with it "Your servers." stops wrapping — four lines instead of five.
- It is applied **where the text is measured**, not where it is drawn. Spacing
  changes what a run is worth, so it changes where every line breaks; a version
  that only moved the pen at paint time would have drawn different text from the
  one the line was made of.
- Shaping is untouched. `alo_text::spaced` adds the room after every glyph
  afterwards, so the `rustybuzz` boundary stays exactly where it was — shaping
  is the rented part, and letter spacing is a CSS decision about the result.
- The test font honours it too. A fake that ignored it would have let a layout
  test pass without the spacing ever reaching the measurer.

- **`white-space`, and the whitespace processing that was never done at all.**
  Markup is written for people, so it is full of whitespace nobody meant to see
  — and until now the engine shaped whatever bytes the parser handed over, so
  `one   two` was three spaces on the screen and an indented paragraph was drawn
  with its indentation in it. Runs of whitespace now become one space.
- **`pre-line` keeps the newlines**, which removes the second of the four
  substitutions in alo's own sign-in case: the headline is one string with
  newlines in it, the way `alo-workplace` writes it, instead of three `<span>`s
  made blocks.
- `pre`, `pre-wrap` and `nowrap` too — and `<pre>` actually preserves its
  whitespace now. The user-agent sheet had said `pre { white-space: pre }` since
  it was written and nothing had ever read it.
- Collapsing happens **when the box is built**, so layout, paint and the agent
  tree all read the same text. Where a line may *break* stays the line builder's
  — a kept newline is a break that must happen, and `nowrap` forbids the ones
  that may. Two questions, and they are answered in two places on purpose.

- **`clamp()`, `min()`, `max()` and the viewport units.** One of the four
  substitutions standing in alo's own sign-in case is gone: the headline's
  `font-size: clamp(2.4rem, 4vw, 3.5rem)` is the screen's own value now instead
  of the `2.5rem` somebody worked out by hand. **The committed reference render
  did not change**, which is the best evidence the substitution was faithful and
  the implementation is right.
- The four are one family and are parsed as one, so they nest:
  `clamp(1rem, min(4vw, 30px), 5rem)` is a value. Each is type-checked once,
  when it is parsed — the smaller of a length and a number has no answer, and is
  refused rather than guessed at, exactly as `calc()` already was.
- `clamp(a, b, c)` is `max(a, min(b, c))`, so **when the bounds cross the lower
  one wins**. That is CSS's rule and not Rust's `clamp`, which refuses a
  reversed range.
- **A viewport unit needs a window, and says so when there is none.**
  `FontMetrics` carries an `Option<Viewport>`; without one, `4vw` is zero rather
  than a plausible-looking number nobody could trace. The cascade supplies it
  from the media context, including while resolving `font-size` itself — which
  is where the headline needed it.
- `MediaContext` has a height. No media query this engine evaluates asks for it
  yet; `vh` does, and a window with a width and no height is not a window.

- **A verb changes the page.** Putting text into a field puts it there; ticking
  a checkbox ticks it; choosing a radio un-chooses the rest of its group. Until
  now a verb decided, reported, and changed nothing — the half of "typed verbs"
  that makes an agent able to *drive* an interface rather than describe what it
  would do.
- **Deciding and changing are two steps, and the types say so.** The agent tree
  borrows the document, so nothing holding one can change it — which is right:
  the decision has to be made against the tree the agent read, and the change
  applied to the document afterwards. `alo_agent::apply` is the second half.
- **The page is rendered again from the same document, never re-parsed.** A
  re-parse would mint new node ids on every keystroke and silently invalidate
  every snapshot anybody was holding. ADR 0003's promise — an id names the node
  it named or nothing — is the thing that had to survive, and there is a test
  that holds an id across a change and uses it.
- **A field shows what it holds.** An `<input>`'s value becomes a text box
  nobody wrote, so a typed value is laid out and drawn rather than only being in
  the tree. A password shows one dot a character and never what it holds.
- **A `<label>`'s words are no longer read twice.** They were exposed as loose
  text *and* as the control's name, so every labelled field on every form
  answered to its own name ambiguously — and an agent's verbs then had to refuse
  it. alo's own sign-in screen read "Email" twice; now it reads it once.
- **A password field is findable.** ARIA gives `<input type=password>` no role
  on purpose, so that a screen reader does not read a password back — and a
  browser that then left it out of the tree would have an agent that cannot sign
  in to anything. Role says what a thing *is*; a new `takes_text` capability
  says what can be done to it.
- Attributes can be changed on a document (`alo_dom::Document::set_attribute`).
  Adding and removing nodes is still the parser's alone, and arrives with the
  DOM APIs.

- **The engine is behind a message boundary** (`alo-renderer`, ADR 0005). A
  renderer is *sent* work and *returns* results: one direction, no callback in
  the signature, nowhere to wait, and nothing ambient — everything it needs
  arrives in a message or in its constructor.
- Every message is **owned, `Clone` and `Send + 'static`**, asserted by a test.
  That is the property that makes a transport possible later without changing a
  caller; choosing the transport is queue item 29's, and inventing a wire format
  before there is a process to send it to would be inventing.
- What crosses is a **frame** (finished bytes, the one thing ADR 0005 lets
  processes share) and a **snapshot** of the agent tree. A borrow cannot cross a
  process, so a snapshot is a copy — and it is safe to act on a moment later
  because it carries node identity, which ADR 0003 never reuses. A test pins
  that the snapshot reads *exactly* as the tree it came from, because a
  description that drifted would be the second structure ADR 0002 forbids.
- The pipeline moved out of the corpus and into the renderer, so there is one of
  it. The corpus still reaches inside for the trees it asserts on: it is a test
  of the engine's insides, and ADR 0005 says tests stay single-process.
- **A claim was corrected.** `docs/conformance.md` said an agent can "act on"
  a page. A verb finds its target, refuses what cannot be operated, and reports
  what it decided — and then nothing happens, because the document is never
  written back to. Trying to use the boundary end to end is what found it. The
  docs now say so and queue item 42 is where it stops being true.

- **Stage 1 needs no hardware and no operating system, and the roadmap now says
  so.** The exit gate asked for alo OS's screens to render *on the certified
  machine, without stutter* — a compositor that does not exist, hardware nobody
  has, and a speed claim measurable on neither. It had already begun doing
  damage: a finished capability was held open, three documents recorded "not
  met" for a reason that was about where a repository sits on a disk, and the
  build had moved on to stage 2 with stage 1 unfinished. The gate now names
  **alo's** screens — an alo screen is alo's whichever repository it lives in,
  and alo's are checked out here — and asks only for what this repository can
  produce and check on any laptop: a reference image and an expected box tree,
  so "correctly" is a diff rather than an opinion. Hardware acceleration,
  embedding into alo OS's shell and rendering without stutter are still real
  work, moved out of the stage 1 list into a section that cannot block it.
  Stage 1's genuine remainder is back in the queue as engine work: four
  substitutions still standing in for things the engine does not implement, the
  Settings screen, and an agent reading it by name.

- **ADR 0005: one process per site, and a sandbox we rent.** Stage 2's first
  roadmap item is a decision rather than code, and this is it — what runs where,
  which way the boundary points, and what happens when a renderer dies.
- It answers the question a memory-safe engine has to answer first: **why does
  Rust not make this unnecessary?** Spectre is a hardware property that no
  language prevents and only site isolation mitigates; the codecs and the TLS we
  rent have `unsafe` in them and are not ours to make safe; the same-origin
  policy is code we write and can get wrong; and a page must not be able to end
  the session. Three of those four would apply to a perfect engine.
- The expensive half is the **shape**, not the `fork`: a renderer never calls
  back synchronously, nothing is shared but a read-only frame, and every message
  is typed. So the boundary gets built while everything is still one process
  (queue item 25) and the split is a change of transport (item 29).

- **An agent reads a broken link as one link, whole.** Layout breaks an inline
  box holding a block into a piece on each side, with the block a *sibling* of
  the pieces — that is where layout needs them and it is not what the document
  says. The agent tree now reads the pieces and the blocks between them as one
  thing: one node, named by everything the element contains, positioned
  everywhere it was drawn, with the block read *inside* it rather than beside
  it.
- Still a **view**, not a second structure (ADR 0002). The box tree records
  which boxes belong to which whole — `broke_out_of`, `continued_from` — and
  the reader follows what is already there.
- **A name gets a space where a block begins or ends.** `<a>Read the<div>the
  manual</div></a>` was called "Read thethe manual". A block-level box is a
  line of its own on the screen, and a name read out has to sound like what a
  person sees. The sign-in screen's headline changed with it, from "Your
  workspace.Your servers.Your rules." to the three sentences it is.

- **An empty piece of a broken inline is kept, and draws its border.** CSS says
  an inline box holding a block is broken into a piece on each side "even if
  either side is empty", because an empty inline with a border still draws one.
  This engine used to drop it.
- **A line box that holds nothing worth holding does not exist.** CSS's rule:
  a line with no text, no preserved space and no inline box with a margin,
  padding or border is zero-height and treated as not existing. That is what
  makes keeping the empty piece free — it costs a line only when it has
  something to draw.
- **An agent still reads one thing.** Of the pieces of a broken inline, the one
  that is read is the first with anything in it; the rest are read through. A
  border is not something to read, so an empty piece is never a second link.

- **An inline box has a box of its own.** A `<span>`'s border and padding are
  laid out and drawn: the horizontal ones take room on the line, once at its
  start and once at its end; the vertical ones draw without changing the height
  of the line, which is CSS's rule and what stops a padded `<em>` pushing a
  paragraph's lines apart.
- **A `<span>` that wraps is one rectangle per line**, not one rectangle with
  the gap between the lines painted over. Its background used to be drawn from
  the union of its pieces, which ran straight across that gap.
- **A broken box's border stops and starts.** The start border is drawn only on
  its first piece and the end border only on its last, the way a browser draws
  a wrapped `<a>` — a piece in the middle has neither.
- A piece ends at its **content**, not at the pen: a line that ends in a space
  has advanced past the last glyph, and a background painted to the pen ran out
  past the end of the text.
- Refused rather than guessed at, and recorded: a **percentage** padding on an
  inline box, which is of the containing block's width and is not known where
  the line is built.

- **`calc()` with a percentage in it works, in every layout property that takes
  one.** `width: calc(100% - 2rem)` — the thing a design system writes for a
  full-width box with a gutter — was refused and recorded. It is now a number:
  widths and heights, minimums and maximums, margins, padding, insets, gaps and
  grid tracks.
- **The layout tree is ours now; the layout algorithms are still rented.**
  ADR 0004. `taffy` asks the *tree* to resolve a `calc()` against a basis only
  the running algorithm knows, and its ready-made tree answers zero with no
  hook to replace it — so this engine keeps its own arena of nodes and
  implements `taffy`'s tree traits over it. Flexbox, grid and block sizing are
  untouched: those are the physics ADR 0001 says to rent.
- The handle `taffy` carries for an unresolved expression is an **index**, not
  a pointer, so there is no `unsafe` anywhere near it and a handle from another
  arena resolves to nothing rather than to somebody else's expression.
- Rounding is now impossible rather than switched off: `taffy`'s rounding is a
  pass over a trait this tree does not implement.
- Still refused, and still recorded: a `calc()` **inside `fit-content()`**,
  which the algorithms have no spelling for.

- **An inline box holding a block is broken around it, the way CSS says.** It
  used to be treated as a block container, which looks nearly the same and is
  not the same: the difference shows in where a background stops. A highlighted
  phrase interrupted by a block now stops before the block and starts again
  after it, because each piece is a box of its own.
- The block becomes a **sibling** of the anonymous blocks the pieces sit in —
  which is why this could not be done by rearranging children in place, and why
  breaking the inline hands several boxes back to its parent instead of one.
- **An agent still reads one link.** The pieces of a broken inline come from
  one element; the agent tree reads the first and reads the later ones through,
  so a verb is never handed two things with the same name to choose between.
- Two gaps the change found, both recorded rather than quietly left: an *empty*
  piece is dropped where CSS keeps it, and an inline box's own border and
  padding are still neither laid out nor drawn.

- **`transform` and `opacity`.** `translate`, `scale`, `rotate`, `skew` and
  `matrix`, about a `transform-origin` that defaults to the middle of the box;
  `opacity` as a number or a percentage. A rotated box carries everything
  inside it round with it, and a rotated letter is a real outline rather than a
  stretched picture of one.
- **`opacity` is a group, not a number applied to each box.** The subtree is
  drawn on a surface of its own and composited back once. Fading box by box
  would show every box in a group through every other one — two black squares
  on top of one another at half opacity would come out three quarters dark
  instead of a mid grey.
- **A gradient under a transform asks where the pixel came from.** The pixel is
  mapped back through the inverse of the transform before the gradient is asked
  what colour it is, so a turned box's gradient turns with it rather than
  staying pinned to the page.
- **Paint order follows stacking contexts now.** A positioned box is painted
  last in the stacking context it *belongs to*, which may be several ancestors
  up — so a positioned box inside a transformed one is painted inside that
  transform rather than over the whole page. A negative `z-index` goes behind
  its parent's content and in front of its background, which is the part of
  stacking that surprises people.
- **A transform moves what is drawn, not what is laid out.** That is what CSS
  says, and it is what lets an agent keep reading positions out of the layout
  tree while the page is animating.
- **Refused rather than approximated**: anything with a third dimension in it —
  `rotate3d`, `matrix3d`, `perspective`, `translateZ`. A value containing one is
  refused whole rather than half applied, because half a transform puts a box
  somewhere nobody asked for.

- **A box can cast a shadow and be filled with a gradient.** `box-shadow`
  (offset, blur, spread, colour, and `inset`), `text-shadow`,
  `linear-gradient` and `radial-gradient`. A page stops being flat colour in
  flat shapes.
- **A shadow is coverage, blurred — not a picture, blurred.** The shape is
  rasterised to a mask, the mask is softened by three box-blur passes, and the
  colour arrives afterwards, so what is *behind* the shadow is not blurred
  along with it. An inset shadow is the same blur run on the shape with a hole
  in it, which is why there is one blur rather than two.
- **A run of text casts one shadow, not one per letter.** The whole run is
  outlined into a single shape before it is blurred: two letters that touch
  would otherwise be blurred separately and composited, and the overlap would
  be visibly darker.
- **Refused rather than approximated**: `conic-gradient`, the repeating
  gradients, colour interpolation hints, and interpolation in any space but
  sRGB. Each is a different curve through colour, and drawing one as another is
  a wrong pixel that looks nearly right.
- Paint was three files' worth of work in two files. `Coverage` moved out of
  the rasteriser — more than one thing makes coverage now — and building a
  display list moved out of the display list itself, because a new CSS property
  and a new kind of drawing are different reasons to change.

- **alo's sign-in screen renders.** The real markup, the real rules and the
  real design tokens, drawn by this engine and diffed on every run: the
  charcoal brand panel, the headline, the fields, the terracotta button, the
  divider. It is `alo-workplace`'s sign-in screen rather than `alo-os`'s,
  because that repository is not checked out here — so stage 1's exit gate is
  **not** met, and `docs/conformance.md` says so.
- **A real screen found four bugs, which is what real screens are for.** Text
  that was a flex item was never drawn at all; a box rounded down to 96 pixels
  wrapped text that measured 96.16, so "Remember me" became two lines in a box
  wide enough for one; `border: 1px solid` was not read, because only the
  longhands were; and an empty `<input>` laid out at nothing by nothing.
- Layout is sub-pixel throughout now. It rounded to whole pixels and measured
  unrounded, which is a disagreement that shows up as a word on the wrong line.
- `text-align` works, and a button's label sits in the middle of it.

- **★ An agent can act on the interface, and no verb takes a coordinate.**
  Activate, put text, scroll — each aimed by a *description* rather than a
  position: "the Save button", "the row called Invoice 12". A description
  survives the page moving; a point does not.
- **Refusing is a result.** Two matter most: two things called the same name is
  refused rather than guessed at, with both of them named so the caller can
  narrow the request — acting on the wrong row is worse than acting on none —
  and a control that says it is disabled is not operated, even though nothing
  physically prevents it.
- Every outcome is a record of what was asked for and what happened, which is
  the guarantee a screenshot-and-guess agent cannot make.
- **The gate now checks the no-coordinate rule.** It was a rule a person had to
  remember; a function in the agent surface that takes a point or an `x` and a
  `y` now fails the run.

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
