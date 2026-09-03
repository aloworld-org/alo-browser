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

- [x] **50. URLs, properly.** WHATWG parsing, resolution against a base, and the
  **origin** as a value other code compares. IDNA and punycode with it, because
  a look-alike domain is a security bug rather than a display one.
  *Closes when:* a table of the WHATWG URL test cases parses to the same
  answers, and an origin compares equal only when it should.

  **Done: `alo-url`.** Parsing rented behind `parse.rs`; the types are ours. The
  table is written in `tests/the_standard.rs` rather than fetched, per
  `LOOP.md`. The rule worth reading twice is a type: an **opaque origin is the
  same as itself and nothing else**, which `file:` and every unregistered
  scheme get, because unknown must never mean "probably fine".

- [x] **51. Fetching what needs no network.** The shape of a load — a request, a
  response, a status, headers, a content type, a body — with `file:` and `data:`
  as the only schemes. Encoding sniffed the way HTML says rather than assumed to
  be UTF-8.
  *Depends on 50. Closes when:* the renderer loads a page from a path rather
  than from a string handed to it, and a mislabelled encoding still reads.

  **Done: `alo-net`.** The fetching happens outside the renderer, which
  ADR 0005 gives no filesystem — `Page::from_response` is how bytes reach it.
  Encoding tables rented (`encoding_rs`), the algorithm ours, and a page that
  decoded badly says so rather than showing question marks nobody can explain.

- [x] **52. TLS with `rustls`.** Rented (ADR 0001), behind its own file like
  every other rented crate. A certificate error is a **decision a person makes**,
  not a dialogue they click through: the error says what is wrong and what
  trusting it would mean.
  *Depends on 51. Closes when:* a good certificate connects, a bad one is
  refused with a reason in words, and the refusal is not bypassable by default.

  **Done.** Tested with a certificate authority made at test time and a server
  on loopback, so a real handshake and real validation run with no network
  anywhere. Not bypassable **at all**, rather than by default: there is no flag,
  no constructor and no feature, and trusting nobody trusts nothing. The
  refusal is a type carrying what is wrong, what trusting it would mean, and
  whether the fault has an innocent explanation.

- [x] **53. HTTP/1.1.** A response body that arrives in pieces.
  *Depends on 52. Closes when:* a frozen page's own byte stream replays through
  it identically, and a truncated response is an error rather than a short page.

  **Done**, and `http:`/`https:` fetch over a socket. The parsing is ours
  because the difficulty is refusing the readings that are *almost* right —
  two disagreeing `Content-Length`s, a length and an encoding together, a space
  before the colon, a folded header — each of which is a request-smuggling
  vector and each of which is refused by name. **Pooling and keep-alive are cut
  into item 54**: framing is the half where being wrong is a security bug, and
  it was worth the whole iteration.

- [x] **54. Connection pooling and keep-alive.** Cut from item 53. A pool that
  hands out a stream, a connection reused across exchanges, and a request that
  can be cancelled. `exchange` already takes a stream from anywhere, so this is
  the pool rather than a change to the framing — and `Connection: close` comes
  out of the request when it lands.
  *Depends on 53. Closes when:* two fetches of the same host use one socket,
  and a server that closes a pooled connection mid-exchange is a failure rather
  than a hang.

  **Done.** The change that made it possible was moving the read-ahead buffer
  from the exchange to the connection — a reader thrown away between exchanges
  takes the start of the next response with it. The retry is narrow on purpose:
  reused, **and** nothing arrived, **and** the method may be repeated. A `POST`
  is never retried, because a payment that has happened must not happen twice.

- [x] **152. Content encodings**: gzip, brotli, zstd, rented. *(Numbered out of
  sequence because 54 was allocated to pooling before this line was read. A
  number here is an identity, not a position — the same rule ADR 0003 gives
  node ids, for the same reason: a reused number makes two different pieces of
  history look like one.)*
  *Depends on 53. Closes when:* each round-trips, and a corrupt stream is
  refused rather than decoded into rubbish.

  **Done.** Round-trips against fixtures made by `gzip`, `brotli`, `zstd` and
  Python's `zlib` rather than by the crates that read them. The bound is on
  what comes **out**, which is the only such bound in the crate and the only
  one a bomb cannot walk past. `ruzstd` turned out to compute a frame's
  checksum and compare it with nothing, so a corrupt zstd body decoded into
  rubbish and reported success — the comparison is ours now, and named in a
  test of its own.

- [x] **153. `Transfer-Encoding` that is not `chunked`.** Cut from 152 rather
  than folded into it: `Transfer-Encoding: gzip, chunked` is legal, is rare,
  and is a *different header* from the one item 152 undoes. Today the chunks
  come off and the gzip does not, which yields compressed bytes labelled as a
  page.
  *Depends on 152. Closes when:* it decodes, or is refused by name — either is
  an answer; handing up compressed bytes is not.

  **Done: it decodes.** The item's stated symptom was wrong and is left above
  as written — nothing handed up compressed bytes, because item 53 compared the
  whole header value against `chunked` and refused everything else, a legal
  response included. `alo-net/src/transfer.rs` reads the list, and the order it
  fixes is the one that matters: the chunks were written *around* the gzip, so
  they come off first. Refused by name, each for a reading two parsers could
  differ on: `chunked` anywhere but last (which is what refuses `chunked,
  chunked`), a coding we cannot undo, an empty element, and a compressed body
  not ended by `chunked` — legal, delimited by the connection closing, and
  indistinguishable from one cut short when the coding is brotli or raw
  deflate, neither of which carries a checksum.

- [x] **55. Redirects.** Redirect loops bounded, cross-origin redirects losing
  what they should. *Byte ranges and resumable downloads were cut to item 154 —
  scope, not depth: they share a roadmap line with redirects and share nothing
  else.*
  *Depends on 53.*

  **Done.** The deciding is a pure function, so every security rule is asserted
  without a socket: `Authorization` dropped at an origin boundary (scheme and
  port included, which is what a hand-written host comparison gets wrong), a
  `POST` demoted to `GET` on 301/302/303 and preserved on 307/308, `HEAD`
  untouched by all five, and `file:` and `data:` refused as destinations
  because a server that could send a load into `file:///` would be reading the
  disk of whoever opened the page.

- [x] **154. Byte ranges, and downloads that resume.** Cut from 55. A range
  request that resumes has to ask for `identity` — a byte range of a compressed
  stream is a range nobody can decompress — which is why item 152 left the
  caller's `Accept-Encoding` alone.
  *Depends on 55, 152. Closes when:* a download interrupted halfway resumes and
  the bytes are the same as an uninterrupted one, and a server that answers a
  range request with the whole thing is noticed rather than believed.

  **Done, both clauses, and the deciding is a pure function** — the shape item
  55 used, for the same reason: every rule here is a rule about *placing bytes
  at an offset*, and such a rule is asserted honestly only when nothing else is
  moving. `alo-net/src/download.rs` decides and `Pool::download` is the loop.
  The rule worth reading twice is that **a resume needs a validator**: without
  an `ETag` or a `Last-Modified` to put in `If-Range` there is nothing that
  could tell us the file changed between the two asks, so such a download starts
  again rather than splicing. Item 185 is the cut.

- [x] **185. A download that stops over HTTP/2 resumes rather than restarting.**
  Cut from 154. The HTTP/1.1 client hands up a body that stopped early with the
  reason beside it (`Exchanged::short`); the HTTP/2 client turns a stream that
  ends early into an error, so the bytes are gone and the download begins again
  at zero. Correct, and slower than it needs to be — and item 163's `DATA`
  handling is the code that has to learn the same distinction.
  *Depends on 161, 154. Closes when:* a download over HTTP/2 interrupted halfway
  opens one range request rather than starting again, in the same shape of test
  as `a_download_that_stops_half_way.rs`.

  **Done, and the distinction it needed is in the frame reader.** A connection
  that ends and a peer that misbehaves were one `Broken`; they are
  `frame::Arrived::Ended` and an error now, because bytes delivered by a
  connection that then ended were framed properly and bytes delivered by a peer
  breaking the protocol were not. **A reset counts as an ending** — a server
  hanging up part way through a body resets rather than closes tidily, since we
  are still writing it window updates for what it just sent. Two ways a stream
  ends early and only two: the connection ends, and the server gives up on the
  stream. The **loop moved** out of `Pool::download` into `download::whole_of`,
  which takes the exchange as an argument — protocol-blind, which is why this
  was a change to the HTTP/2 client and to nothing else, and which is what let
  the closing condition be run rather than reasoned about: this engine speaks
  HTTP/2 only over TLS and a test may not name `rustls` (ADR 0001), so the test
  drives the real loop over a plain-socket HTTP/2 server of its own.

- [x] **56. The HTTP cache, with real semantics** — freshness, revalidation,
  `Vary`. `ROADMAP.md`: *"subtly wrong here is invisible for months and then
  serves somebody a stale bank page."*
  *Depends on 53. Closes when:* a table of responses and clocks produces the
  right hit, miss and revalidate for each, including the ones that are only
  wrong an hour later.

  **Done.** The table is `tests/what_the_cache_serves.rs`, and the pairs either
  side of an expiry are the point — nothing in the cache reads the clock, so
  every case names a moment. `Age` is counted, which is what stops a chain of
  caches each granting one response a fresh lifetime. `Vary` is stored as the
  request values a response was chosen by, so a French page is never handed to a
  German reader, and `Vary: *` is not stored at all. Disk went to item 155.

- [x] **155. The cache on disk.** Cut from 56, which is memory only.
  *Depends on 56. **ADR 0011 is written and accepted** — what may be written to a
  disk other programs can read is a different question from what may be reused,
  and it has a different answer for a page behind a password. The code is what
  remains, and the ADR names the rules it must carry: the cache is **partitioned
  by top-level site** on the same `Partition` the cookie jar uses; what must not
  outlive the session is **never written** rather than written and deleted
  (`no-store`, `private`, a request carrying `Authorization`, a response carrying
  `Set-Cookie`, anything not `http:`/`https:`, a body that did not arrive whole,
  and any session-scoped profile); a cache file is **untrusted input** with a
  checksum, a version and a miss rather than an error when it does not read; and
  it lives in the **browser process only**, because a sandbox profile granting a
  renderer that directory would hand a compromised renderer every page the person
  has read. *Closes when:* a cache survives a restart, and a response that must
  not outlive the session does not.

  **Done.** `tests/a_cache_that_survives_a_restart.rs` closes both halves with a
  real restart — the `Cache` and its `Disk` are dropped and a second pair is
  opened on the same directory — and the never-written list is a table, one row
  per rule, each asserting three things: it is reusable in memory, the disk holds
  nothing, and no file is left behind. The site is in the key, so the same script
  fetched inside two sites is two entries and a restart does not launder the
  join. `disk.rs` is the directory and the policy; `record.rs` is the bytes and
  is the whole untrusted surface — every truncation and every single flipped byte
  of an entry is refused in a test that walks all of them. Two things the ADR
  named went to the queue as they were built: nothing here decides a **quota**
  (item 90), and the site boundary is still the host until item 156. One thing
  the ADR overstates is written down in `record.rs` rather than left implied: an
  unkeyed checksum catches a half-written file and cannot catch a program running
  as the person, which is section 3's boundary unchanged.

- [x] **57. Cookies, partitioned by default.** `SameSite`, `Secure`,
  `HttpOnly`. **The default is a product decision** rather than a parser detail,
  so it is written down where a person can argue with it.
  *Depends on 50, 53. **ADR 0007 is written and accepted** — what the default
  costs and who it protects.*

  **Done.** The promise is kept by the shape rather than by memory: every lookup
  takes a partition and no function returns the unpartitioned set. The prefixes
  are enforced rather than parsed — a `__Host-` cookie that does not qualify is
  rejected, because the value of a prefix is that a server can trust the name.
  Two things the ADR asks for went to the queue: 156 and 157.

- [x] **156. The public suffix list, rented.** Today the site boundary is the
  **host**, which is stricter than the registrable domain — `a.example.com` and
  `b.example.com` are separate sites, where they should be one. Stricter is the
  safe direction, and it is wrong.
  *Depends on 57. Closes when:* `bbc.co.uk` and `gov.co.uk` are different sites
  and `www.example.com` and `example.com` are the same one, in a test that names
  both.

  **Done, and it went in `alo-url` rather than `alo-net`** — the site is a
  property of a host, and three unrelated things were each answering it with the
  host on their own: the cookie partition (ADR 0007), the cache key (ADR 0011)
  and the renderer process (ADR 0005). `alo_url::site::of` is the one answer all
  three take now, and it takes a **`Host` rather than a string**, because
  `127.0.0.1` read as a name has the registrable domain `0.1` and the type is
  what already knows it is an address. It found a hole while it was there:
  `Domain=co.uk` was accepted, since the only rule was that a domain contain a
  dot. The cut is item 186: the list is a snapshot, and nothing says when it has
  aged.

- [x] **186. The public suffix list has a date, and nothing reads it.** Cut from
  156. `psl` compiles a snapshot of the list in, which is right — a security
  boundary that arrived over the network would exist only when the network did —
  but a snapshot ages, and a suffix delegated after ours was taken is read as an
  ordinary registrable domain. That is two organisations sharing one site, which
  is the direction that costs rather than the direction that annoys. Updating it
  is a version bump somebody has to think of, and nobody is prompted to.
  *Depends on 156. Closes when:* something a person sees names the snapshot's
  age — the gate, or a test that fails when it is older than a stated number of
  months — so that an out-of-date boundary is a message rather than a silence.

  **Done: `alo-url`'s `snapshot`, and it is the test rather than the gate.** The
  stated number is **six months**, and the reason it is not twelve is written
  down: the list carries no date and `psl` publishes none, so what is recorded
  is the day the snapshot was taken *here*, which under-reports the true age by
  however long the crate version had already existed. Two constants — the
  version and the day — and a test that fails if `Cargo.lock` resolves `psl` to
  anything else, so the record cannot drift from the code it describes. The
  message a person gets names the version, the day, what a stale list costs (two
  organisations in one site) and the two commands that discharge it, and it says
  in its own doc comment that **this failure is not a fault in the change being
  tested** — because the iteration that meets it will otherwise spend itself
  looking for one.

- [ ] **157. The storage-access grant.** ADR 0007 specifies it mostly by what it
  must not be: never a global toggle, never an allowlist we ship. A person is
  told who is asking and inside what, and answers for that pair.
  *Depends on 57. Blocked: needs an interface to ask in.*

- [x] **58. DNS, and encrypted DNS as a choice somebody made** rather than a
  default nobody was told about.
  *Depends on 53. **ADR 0008 is written and accepted** — the same argument as
  57, about a different server seeing every name you look up. The code is what
  remains, and the ADR names two rules it must carry: DNS is never trusted for a
  security decision, and a public name resolving to a private address is
  refused.*

  **Done, except the setting.** Resolution goes through the machine's resolver,
  and the rebinding rule turns on **who asked** rather than on the address
  alone — which is the only way to let a person reach their own intranet while
  refusing a public page the same address. Connecting takes addresses rather
  than a name, because resolving twice is how the second answer differs from the
  first. The setting went to item 158.

- [x] **159. MPL Exhibit A headers on every source file.** ADR 0009 relicensed
  the engine and says the per-file headers are **owed** — left out deliberately,
  because touching 147 files while the loop is working would collide with real
  work. The root `LICENSE` satisfies MPL meanwhile, so this is tidiness rather
  than exposure; it is in the queue because owed work that lives in one commit
  message is owed work one person is remembering.
  *Depends on nothing. Closes when:* every `.rs` file carries the header and
  `scripts/gate.sh` fails on one that does not — a header nothing checks is a
  header that stops being true.

  **Done, on 198 files rather than the 147 the item remembered.** The notice is
  copied from this repository's own `LICENSE`, Exhibit A, verbatim — including
  the `http://` the licence text uses, because the notice a recipient checks
  should be the one distributed beside it rather than a tidied version of it.
  The gate compares the first three lines of each file against that exact text,
  so a *reworded* header fails as loudly as a missing one; both directions were
  run rather than reasoned about. It served no `ROADMAP.md` line and it is not
  in `docs/features.md`, for the reason written in `STATE.md`: it is not
  something the browser does.

- [ ] **158. The encrypted-DNS setting.** ADR 0008 says it must name the company
  that would see every site you visit, in the sentence where it is chosen, and
  that no provider is preselected and the order is not for sale. Falling back to
  plain DNS is a failure that says so, never a silence.
  *Depends on 58. Blocked: needs an interface to choose in — the same block as
  item 157.*

- [x] **59. HTTP/2 framing**, once 1.1 is correct. *Scope cut on starting: the
  protocol is four items, not one. HPACK, streams and negotiation are 160, 161
  and 162.*
  *Depends on 53.*

  **Done.** Framing first because everything else is carried inside it, and
  because it is where a peer gets to choose how much memory we allocate. The
  padding underflow is refused by name; a frame that is *entirely* padding is
  legal and tested, because a check written one off would refuse the frames
  servers send to disguise a response's size.

- [x] **160. HPACK.** The header compression HTTP/2 carries in its `HEADERS` and
  `CONTINUATION` blocks: static table, dynamic table, Huffman.
  *Depends on 59. Closes when:* the specification's own request and response
  examples round-trip, and a block that would grow the dynamic table past what
  was agreed is a `COMPRESSION_ERROR` rather than an allocation. **A decoding
  failure is fatal to the connection, never to one stream** — the table carries
  state between blocks, so a block nobody could decode leaves it in a condition
  nobody can reason about.

- [x] **161. Streams, flow control, and the connection state machine.**
  *Depends on 59, 160. Closes when:* a stream that is finished refuses further
  frames, a window that would go negative is a `FLOW_CONTROL_ERROR`, and a peer
  opening more streams than it was allowed is refused rather than accommodated.
  The bounds go in before the happy path: this is where a misbehaving peer
  allocates memory on our side.

  **Done, and they did go in first.** The CONTINUATION flood needed a bound on
  the whole block across frames rather than on each frame, which is why it is
  counted by the session. A window may legitimately be **negative** — lowering
  the initial size applies retroactively to streams that already exist — and a
  test I wrote expecting otherwise was wrong about the protocol rather than the
  code.

- [x] **162. Negotiating HTTP/2 at all** — ALPN in the TLS handshake, and
  choosing 1.1 when the server does not offer h2.
  *Depends on 59, 160, 161, 52. Closes when:* a server offering `h2` is spoken
  to in HTTP/2 and one offering nothing is spoken to in HTTP/1.1, with no
  request sent twice while finding out.

  **Done.** Nothing can send a request twice to find out, because the answer
  comes out of the handshake — which is what ALPN is *for*, and why it is not a
  header. The pseudo-header rules went in with it, on both directions: a
  response carrying a request's pseudo-header, or an ordinary header before
  `:status`, is refused.

- [x] **163. A request with a body over HTTP/2.** Today every request goes out
  with `END_STREAM` on its `HEADERS`, which is truthful and means no `POST`.
  *Depends on 162. Closes when:* a body goes out in `DATA` frames sized to the
  window, a window that closes mid-body is waited on rather than overrun, and a
  `100-continue` is either honoured or refused by name.

  **Done, and it was never only HTTP/2's.** A `Request` had nowhere to put a
  body at all, so HTTP/1.1 had no `POST` either and would have silently dropped
  one — the item's scope was HTTP/2 and its depth was both, which is why both
  send a body now. The two framing rules live on `Request` rather than in each
  client, for the reason `may_be_repeated` already gives: **the length a request
  states is the length of its bytes**, never a header a caller wrote, and an
  `Expect` is **refused by name** on both. The window clause is asserted by a
  server that goes quiet: a hundred-kilobyte body, and exactly sixty-four
  kilobytes have arrived when the client stops of its own accord — under it is a
  stall and over it is an overrun, so the number is asserted rather than
  bounded. Two things came out of it: item 187, and interim responses, which
  were being taken for the answer on both protocols and are read past now.

- [ ] **187. `Expect: 100-continue`, honoured rather than refused.** Cut from
  163, which refuses it — the item allowed either, and refusing is what an
  engine that cannot *bound* the waiting should do. An expectation is a promise
  to wait for a go-ahead, and the only clock reachable from either client is the
  caller's socket timeout at thirty seconds, which would turn every upload to a
  server that has never heard of the header into half a minute of nothing.
  Honouring it means a short bounded wait and then sending anyway, which means a
  clock crossing the boundary that `exchange` takes an `impl Read + Write` over.
  **Nothing on the web can reach the refusal**: `Expect` is a forbidden request
  header in Fetch, so no page and no script may set one — which is why this is
  worth doing when an upload wants it rather than before.
  *Depends on 163. Closes when:* a server that answers `100` is sent the body
  after it, a server that answers a final status is not sent the body at all,
  and a server that says nothing is sent the body after a bound a test can name.

- [ ] **60. HTTP/3 and QUIC**, once both of those are.
  *Depends on 59.*

## B. Origins, and the model that keeps sites apart

ADR 0005's four reasons, made real. Three of these are code we write and can get
wrong, which is the argument for the fourth.

- [x] **61. The same-origin policy, CORS and preflight.**
  *Depends on 50, 53. Closes when:* a cross-origin read that should fail does,
  in a test that names the attack rather than the header.

  **Done, and the naming rule earned its place.** Writing the tests from the
  attacker's side found a real bug: `Cookie` is set by the browser and never by
  the page, and treating it as an author header would have preflighted every
  credentialled request *and* named `cookie` in `Access-Control-Request-Headers`.
  A file of `allow_origin_header_is_checked` tests would have passed throughout.

- [x] **164. The preflight cache.** `Access-Control-Max-Age`, so a cross-origin
  request is not two round trips every time.
  *Depends on 61, 56. Closes when:* a second request of the same shape sends no
  `OPTIONS`, one of a *different* shape still does, and an entry expires on the
  clock the caller passes in rather than one the cache reads.

  **Done: `alo-net/src/preflight.rs`**, all three clauses, and the dependency on
  56 turned out to be a dependency on its *shape* rather than on its code — the
  clock is the caller's and the key carries the [`Partition`] here for exactly
  ADR 0011 section 1's reason. The rule the whole file is one application of:
  **what is remembered is what a server said about a request that was actually
  made**, never anything wider. So a `*` is stored as the method and headers it
  allowed rather than as a wildcard, which is what makes the rule that `*` never
  covers `Authorization` need no restatement here; an answer given without
  credentials does not cover a request carrying them; an opaque origin is never
  a key, because every one of them serialises to `null`; and there is one way
  in, `Preflights::allowed`, which checks before it stores so that remembering
  a refused permission is not a thing a caller can do by getting an order
  wrong. Two hours is the cap, because a permission nobody can revoke is not
  one.

  **It found one thing and fixed it rather than cutting it**, because the cache
  could not have been built correctly around it: the safelist was applied by
  header *name* in two of the three places that ask what shape a request is, and
  it is a rule about the value too. So `Content-Type: application/json` was
  correctly preflighted and then asked about with a question that never named
  `Content-Type`, and allowed by a server that had said nothing about it. One
  function answers it now — `cors::names_a_form_could_not_have_sent` — and this
  cache matches against the same one.

- [x] **62. Referrer policy, HSTS, mixed-content blocking.** *Scope cut on
  starting: CSP is a whole item on its own — see 165 — and its grammar is where
  doing it badly quietly weakens the protection a page asked for.*
  *Depends on 61.*

  **Done.** The tests are named for the attacks, the way item 61's were. The two
  rules that make HSTS a defence rather than a weapon are the ones with tests
  named after them: a header over plain HTTP is ignored, and an address cannot
  pin itself.

- [x] **165. Content Security Policy.** The directives, the source expressions,
  and reporting. *Scope cut on starting: reporting is item 188 and computing a
  content hash is item 189.*
  *Depends on 62. Closes when:* a policy that would block an injected script
  does, and — the rule that matters more than any single directive — **a
  directive this engine cannot parse makes the policy more restrictive, never
  less.** A page that asked for a protection must not lose it to our not
  understanding the sentence it asked in.

  **Done, both clauses, and the second one is three separate holes rather than
  one.** A source expression we cannot read is *kept* and matches nothing; the
  directive holding it is *kept whole*, because discarding it would send its
  requests to `default-src` or to nothing; and a directive name we do not act on
  grants nothing and is **named** by `Policies::not_enforced`, because the
  honest answer to "is this page protected" is sometimes "in four respects and
  not in a fifth". Two more rules of the same shape went in with it: a repeated
  directive keeps the **first**, so anybody who can append to the header cannot
  widen a policy by restating a directive, and two policies are an
  **intersection**. `csp_source.rs` is the grammar and the matching,
  `csp.rs` the directives and the decision — two files because a new source form
  and a newly enforced directive are different reasons to change. Eight rules
  were doctored out and the test named for each failed. **One gap is a decision
  rather than a cut**: a document load is not governed, because CSP governs a
  *nested* document and not a top-level navigation and nothing here can yet tell
  a link click from an `<iframe>` — item 86.

- [x] **188. A policy that was violated says so.** Cut from 165, which enforces
  and does not report. `report-uri`, `report-to`, the violation report's own
  shape, and posting it. `Policies::objections` is already the list such a
  report would be made from, and `Disposition` already keeps a report-only
  policy from being enforced — what is missing is the channel.
  *Depends on 165. Closes when:* an enforced violation and a report-only one
  both produce a report, the report says which directive and which URL without
  saying more than the specification allows (a cross-origin URL is stripped, or
  the report is a way to read one), and a report that cannot be sent is not a
  load that fails.

  **Done, all three clauses, and the deciding is a pure function** — the shape
  items 55 and 154 already use, for the same reason: what a report may say is a
  rule about a *stranger's URL*, and such a rule is asserted honestly only when
  nothing is moving. `csp_report.rs` builds the posts and `Pool::report` is the
  loop that sends them. `Policies::objections` became `Policies::violations`,
  which is the join between the two files and the only thing that can build a
  `Violation` — a violation nobody's policy objected to is not a thing.

  Three rules are worth reading twice. A report names the **effective**
  directive rather than the deciding one, so `default-src 'none'` refusing a
  script reports `script-src`; both answers come out of one function, because
  computing it twice is how the report and the message come to disagree.
  `report-to` **wins over `report-uri` when it resolves** and reports nowhere
  when its group was never defined, since falling back would be this engine
  deciding an author who wrote a group name meant something else. And a report
  is its own `Purpose`, not a fetch — a policy does not govern its own
  reporting, so a report sent as a fetch would be silenced by `connect-src
  'none'` exactly when it had something to say.

  The fields this engine cannot honestly fill — `line-number`, `column-number`,
  `source-file`, `script-sample` — are **omitted rather than zeroed**, and a
  test asserts their absence: a `"line-number": 0` is a wrong answer that reads
  like a right one. Two cuts are recorded rather than taken: the `Report-To`
  JSON header is not read (deprecated, and two spellings of where somebody's
  reports go is two chances to disagree), and nothing here queues, batches or
  rate-limits a report.

- [x] **189. A content hash, computed.** Cut from 165, which reads
  `'sha256-…'`, lets its presence correctly disable `'unsafe-inline'`, and
  matches nothing — so a policy that allows inline content only by hash refuses
  it and says so in words. Closing this needs a digest, which means **renting
  one** (ADR 0001 — a hash function is physics) with an entry in
  `scripts/gate.sh`'s boundary list.
  *Depends on 165, and on there being content to hash — inline style exists
  today, inline script needs item 72. Closes when:* an inline `<style>` whose
  digest a policy names is allowed and one whose digest it does not is refused,
  and both alphabets a policy may write the digest in are read.

  **Done, all three clauses: `crates/alo-net/tests/a_hash_a_policy_named.rs`.**
  `sha2` is the rented digest and `alo-net/src/digest.rs` is its boundary. What
  took the thinking was not the hash — it is one call — but **reading the value
  an author wrote**, which is why the file also holds the base64 and why every
  rule in it is written down: a hash source is a *permission*, so a decoder that
  is lax in any direction is a policy quietly wider than its author wrote. So
  the two alphabets are never mixed, a value whose last group has bits standing
  for no byte is refused as a second spelling of one permission, and nothing is
  trimmed. A value of the wrong length for the algorithm it names is a
  **non-match rather than an error**, because `'sha256-YWJj'` is an author's
  mistake that should show up as content that does not run.
  `Digest::names` compares **bytes**, so there is one spelling of our own digest
  to compare against rather than four of the author's.

  It also settled where a hash *is not* the answer: `Source::matches` refuses a
  hash for a URL, since a policy is checked before anything is fetched and a
  `<script src>` is allowed by where it comes from. The cut is item 191.

- [x] **191. `'unsafe-hashes'`, so a `style` attribute can be allowed by its
  digest.** Cut from 189, which hashes content that has an element of its own
  and refuses to hash anything else — a `style` attribute, an event handler.
  Matching one of those by hash is exactly what `'unsafe-hashes'` enables, and
  this engine reads that keyword as inert, so `Policies::allows_inline` takes
  `None` for such content and says so in the refusal
  ([`csp::ByHash::NothingToHash`]). Deciding it silently either way would be
  guessing about a permission.
  *Depends on 189. Closes when:* a `style` attribute whose digest a policy names
  applies **only** where that policy also says `'unsafe-hashes'`, and the same
  policy without the keyword refuses it — the second half being the one that
  matters, since the keyword exists to make the permission deliberate. The event
  handler half waits for item 81, which is where a handler is a thing at all.

  **Done, and the shape is what to read rather than the keyword.** *Where*
  content was written became a type of its own — `csp::Content::element` and
  `csp::Content::attribute` — beside the kind that picks the directive, because
  they answer different questions: the kind chooses `script-src` or
  `style-src`, and the placement decides whether a hash in it may apply. That
  is why **the event handler half needed no code and no case**: it is
  `Inline::Script` with `Content::attribute`, and item 81 will pass it without
  changing anything here. What is genuinely owed to 81 is a handler to pass.

  Three rules went in with it, each because the alternative widens somebody's
  policy: the keyword grants **nothing on its own**, so a directive holding it
  and no digest allows no attribute; it is read from the **deciding directive**
  rather than from anywhere in the policy, so one in `default-src` does not
  reach a `style-src` that decided; and two policies stay an intersection, so a
  second header cannot add the keyword to the first one's hash.
  `ByHash::NothingToHash` became `ByHash::NotWithoutTheKeyword`, which is the
  honest sentence now: the digest may well match, and no digest applies here.

- [x] **63. The boundary's wire format.** *Scope cut on starting: the split is
  three items, and this is the one that has to be right before anything is
  spawned. Spawning is 166; the sandbox is 167 and needs an ADR, because ADR
  0005 says explicitly that it does not pre-authorise the `unsafe` a sandbox may
  require.* Originally: one process per site,
  renderers with almost no privilege, the platform's own sandbox rather than a
  hopeful one of ours — seccomp-bpf and user namespaces on Linux, Seatbelt on
  macOS. ADR 0005 decided it; `alo-renderer` made it a change of **transport**
  rather than a redesign.
  *Depends on 51 — a sandboxed renderer cannot fetch, so the browser process
  must be able to. Closes when:* two sites are two processes, a renderer cannot
  open a file, and killing one leaves the other running.
  *This is the roadmap's "queue item 29", renumbered with the rest.*

  **Done, for the encoding.** Both directions, every variant, with the hostile
  half being the messages coming *back*: a renderer is the process that parsed
  the page. A tree deeper than 512 is refused rather than recursed into, because
  a decoder that recursed as far as it was told would crash the **browser**
  process on a message — which is the one thing ADR 0005 says must never happen.

- [x] **166. One process per site.** Spawn a renderer, talk to it over a pipe,
  key them by site, bound how many exist, and reuse the ones there are.
  *Depends on 63. Closes when:* two sites are two processes, and killing one
  leaves the other running and its tab showing the last frame it painted.

  **Done.** The tests spawn the real `alo-render` binary and one of them kills a
  process while another is serving, which is the only way to check the thing the
  design is for. A dead renderer is **not** silently restarted — that has its own
  test, because a silent restart turns a page that crashes its renderer every
  time into an invisible loop.

- [x] **167. The sandbox, on macOS.** Seccomp-bpf and user namespaces on Linux, Seatbelt
  on macOS. **Needs ADR** — ADR 0005 says in its own consequences that it does
  not pre-authorise any `unsafe` a sandbox needs, and that such a thing wants
  its own decision naming the boundary and the reason.
  *Depends on 166. **ADR 0010 is written and accepted** — rented, applied before
  any page bytes, fatal if unavailable, and authorising no `unsafe` of ours. The
  code is what remains. Closes when:* a renderer cannot open a file, and the
  test that says so watches it fail rather than trusting a flag.

  **Done, for macOS.** `sandbox-exec` rather than `sandbox_init`, because the
  latter is FFI and ADR 0010 authorises no `unsafe` here — deprecated, said so
  in the module, and with the advantage that the profile is applied *by* `exec`
  so the process is never unconfined. The test runs the same binary confined and
  unconfined and requires the unconfined run to be **allowed** all four things,
  because a test that passed both ways would be testing nothing.

- [ ] **169. The sandbox, on Linux.** seccomp-bpf, a user namespace and
  Landlock, as ADR 0010 names them.
  *Depends on 167. Closes when:* the same four probes are refused on Linux, in
  the same test, run on Linux — because a sandbox only ever checked on the
  machine of whoever wrote it stops working on a Tuesday without anybody
  noticing.

- [x] **168. Fonts across the boundary.** ADR 0010's consequence: a confined
  renderer cannot open a font file, and the rule is that the browser process
  passes bytes rather than the policy permitting a directory. `alo-render`
  embeds one font today, which is what the design forces.
  *Depends on 167. Closes when:* a renderer draws with a font it was handed and
  never with one it went looking for.

  **Done.** `alo-render` embeds nothing now and starts with an empty database.
  The temptation the ADR named — adding `(subpath "/System/Library/Fonts")` to
  the profile — was resisted, and the test that would have made it tempting is
  the one asserting a renderer given no fonts really has none and is still
  confined.

- [x] **172. A second page we did not write.** The first web page, which has no
  style sheet at all and is therefore the only real test of the user-agent
  sheet. Found that links had no colour, which is fixed, and two things that are
  not — items 173 and 174.
  *Depends on 68.*

- [x] **173. Paint `text-decoration`.** It has been in the user-agent sheet all
  along — `underline` on every link — and produces no paint operation at all.
  Nothing in the alo cases underlines anything, so nothing noticed until a page
  made of links arrived.
  *Depends on 172. Closes when:* the first web page's links are underlined in
  its committed render, a decoration stops at the end of an inline rather than
  running to the edge of its line, and `line-through` and `overline` work too —
  they are the same machinery and leaving them out would mean doing this twice.

  **Done, and the hard rule fell out of the shape rather than needing a special
  case.** A decoration is drawn per *fragment*, and a fragment is one piece of
  one inline on one line — so "stops at the end of the inline" is what drawing
  per fragment already means. The propagation is a walk up the ancestors rather
  than the property being made inheritable, because a descendant cannot turn a
  decoration off and an inherited property could be.

- [x] **174. A wrapped inline is more than one rectangle.** `link "Frequently
  Asked Questions"` comes back from the agent tree as 778×37 starting at the
  left margin, because it wraps and the tree reports the union of its fragments.
  No verb takes a coordinate (ADR 0002) so nothing acts on it — but it decides
  whether a node counts as offscreen, and it is what a person reading the tree
  sees.
  *Depends on 172. Closes when:* a link split across two lines reports the boxes
  it actually occupies, and a node is offscreen only when **none** of them is on
  screen.

  **Done.** Item 173 had already proved the fragments were there and correct, so
  this was a choice rather than a limitation — as that iteration's journal
  predicted. The outline says `in 2 pieces` rather than listing them, which
  keeps it readable while no longer implying a wrapped link is a rectangle.

- [x] **175. A corpus case with more than one file.** A real page keeps its
  style in a second file, so a corpus that could only hold one could only ever
  hold pages that keep it inline — which is almost none of them. `linked.txt`
  maps an `href` to a file frozen beside the case.
  *Depends on 68.*

  **Done**, and it needed `<link>` and `<style>` gathered into one list in
  document order rather than two — which is the part that would have been
  silently wrong if the sheets had been collected by kind.

- [ ] **170. Fonts a page asks for by name.** *Item 68's first case is the
  evidence: it asks for `system-ui, sans-serif`, gets DejaVu Sans, and nothing
  says so.* Today every renderer is given the
  same short list at startup. A page asking for a family nobody sent gets a
  fallback, silently.
  *Depends on 168. Closes when:* a renderer can say which family it wanted and
  did not have, and the browser process can answer with it — and a family that
  genuinely is not on the machine is a named substitution rather than a silent
  one.

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

- [x] **68. The web corpus.** A second kind of case beside the alo ones: a page
  from the web, **frozen** — its bytes as they were, with where they came from
  and when, and its own expected trees and render. Never fetched at test time,
  for the reasons `LOOP.md` gives.
  *Depends on 51. Closes when:* one real page renders, is diffed on every run,
  and the suite still passes with the network unplugged.

  **Done**, and it earned its place immediately: three findings on the first
  run, one of which had to be fixed before the page would render at all. See
  `cases/web-example-com/origin.txt`, which records what it found *and* what it
  did not — the second being as much the point as the first.

- [x] **171. Block margins in the user-agent sheet.** Headings and paragraphs
  get `display: block` and no margin, so every real page renders visibly tighter
  than it should. Found by item 68's first case; invisible before it, because
  alo's own sheets set their own spacing.
  *Depends on 68. Closes when:* the committed render of `web-example-com` has
  the spacing a browser gives it — **and every other case's render moves in the
  same commit**, which is the review: a UA change that did not move them would
  mean they were all setting their own margins, and a change that moved one
  wrongly is a diff somebody can see.

  **Done, and the review answered the other way.** *No* existing case moved,
  which the closing condition named as the alternative and which is the true
  one: alo's own screens set all their own spacing. Getting there needed a
  cascade fix the defaults exposed — shorthands and longhands competed as
  different property names, so a user agent's `padding-left` beat an author's
  `padding: 0`. Heading font sizes went in with the margins, because a heading
  at 16px is the same defect.

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

  **The tick and the dot are done** — item 182, corpus case `control-states`.
  **What is left is the focus ring**, and it is left rather than forgotten: this
  engine has nothing that *has* focus, because focus arrives with events (item
  81). `:focus-visible` already parses and matches nothing, which is the correct
  answer for a still picture of a page nobody is using, so drawing a ring today
  would mean inventing a focused element to draw it on.
  *So: depends on 81.*

- [x] **184. `border-width`, `border-style` and `border-color` as shorthands.**
  Each is one value per side and splits by exactly the rule `margin` and
  `padding` already use; none of them was expanded, so `border-color: red` was
  kept and ignored. **Taken inside item 182** rather than queued after it: that
  item needed the user-agent sheet to set `border-color` on a disabled control,
  and `alo_css`'s own comment had already named the day these should be added —
  *"the engine does not yet set any of them in the user-agent sheet, so nothing
  collides"*. It collided.
  *Closes when:* each splits into its four sides in a test that names all three,
  and `border` itself still does not — `red solid 1px` and `1px solid red` are
  the same border, so splitting that one means parsing rather than counting.

  **Done.** No corpus case moved except the one the disabled rule was written
  for, which is the evidence that nothing was relying on the old behaviour.

- [ ] **190. The border styles that are two tones**: `groove`, `ridge`, `inset`
  and `outset`. Cut from 183, which draws a fieldset's border `solid` where
  every other browser draws a `groove` — and says so in the user-agent sheet,
  because a substitution nobody wrote down is one nobody re-checks. Each is the
  same shape drawn in two colours, a lighter and a darker derived from the
  border's own, and which side gets which is what makes it look raised or sunk.
  `dashed`, `dotted` and `double` are the same file's work and belong with them.
  *Depends on nothing. Closes when:* each draws in a reference render and none
  of them is another one — a `groove` and a `ridge` that came out identical
  would be a test passing on a picture nobody looked at.

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

- [x] **106. Reading a picture a stranger sent.** ADR 0005's second reason for
  the sandbox is that image codecs have `unsafe` in them and are not ours to
  make safe, so **untrusted bytes are decoded in the least privileged process
  that can do it** — which the renderer already is.
  *Scope cut on starting: five codecs and a whole layout mode is not one item.
  This is the untrusted-bytes half, for PNG — the half ADR 0005 names, and the
  half no sandbox would catch, since a renderer that allocated seventeen
  gigabytes because a header said so is doing nothing a sandbox forbids. Laying
  out and drawing is item 176; the other codecs are 177.*

  **Done.** Tolerant of every colour type a PNG may have, and bounded at
  sixty-four megapixels **before** the allocation, because a hundred-byte file
  that parses perfectly can declare seventeen gigabytes.

- [x] **176. `<img>` lays out and draws.** A decoded picture has an intrinsic
  size and nothing uses it: there is no intrinsic sizing anywhere in `alo-box`
  or `alo-layout`, which is the actual work here rather than the decoding.
  *Depends on 106, 175 — a case needs to hold the picture beside the page.*
  *Closes when:* an `<img>` with no width or height lays out at the picture's own
  size and aspect ratio, one with a width keeps the ratio, and a picture that
  was refused leaves a box of the size the page asked for rather than nothing.

  **Done**, and the journal was right that the work was intrinsic sizing rather
  than pictures. Two things the case caught: an `<img>` is `inline-block` and so
  goes through the inline path rather than taffy's leaf layout, and the ratio has
  to be a `taffy` aspect ratio because a leaf's measure is asked *before* the
  style width is applied.

- [ ] **178. A rotated picture is drawn rotated.** Today only the rectangle's
  corners are transformed, so a picture under a `rotate()` draws upright inside
  the right area. Wrong and visible, which is why it was preferred to drawing
  nothing.
  *Depends on 176. Closes when:* a picture under a rotation is drawn rotated, and
  a reference render says so.

- [ ] **179. Sampling a picture that is not at its own size.** Nearest-neighbour
  today: exact at one-to-one, coarse anywhere else.
  *Depends on 176. Closes when:* a picture drawn at half its size is not
  visibly speckled, in a reference render — and law 3 applies, so this waits
  for a page that needs it rather than being guessed at.

- [x] **177. JPEG.** Rented, pure Rust. *Scope cut on starting: GIF, WebP and
  AVIF went to item 180 — JPEG is the one that matters, because most
  photographs on the web are one.*
  *Depends on 106. Closes when:* each lays out and draws from a frozen file, and
  each has the same bound and the same refusals as PNG — which is the reason
  they are one item rather than four.

  **Done for JPEG.** The format is decided by the bytes rather than by the
  `src`, and the corpus case has a JPEG served under a `.png` name to say so.
  Every refusal test runs against both formats from one list, so adding a third
  means adding it to the list rather than remembering to.

- [x] **181. A page with a form.** The HTML specification's own example form,
  frozen. Found four things — a `<fieldset>` laid out inline because the
  user-agent sheet declared it twice, a fieldset with no name, a radio drawn as
  a square, and `border-radius` in per cent resolving against nothing — and all
  four are fixed. Two more are 182 and 183.
  *Depends on 68.*

- [x] **182. A checked control looks checked.** `[checked=true]` in the tree and
  one fill in the display list: the border. True since controls were built, and
  the alo corpus has an example of it — it took a page with radios and
  checkboxes side by side to make anybody look.
  *Depends on 181. Closes when:* a checked checkbox and a checked radio are
  visibly different from unchecked ones in a reference render, an indeterminate
  checkbox is different from both, and a disabled one still shows its state —
  because "you cannot change this" and "this is off" are different things to be
  told.

  **Done, and the split between the two halves is the thing worth reading.** The
  *mark* is drawn by the engine (`alo_paint::control`) because CSS has no way to
  say "and draw a check inside it" — the same argument that put a control's
  inner box in `alo_box` rather than the sheet. Whether the control is **live**
  is ordinary colour, so it is in the user-agent sheet where a page can override
  it. Corpus case `control-states`: nine controls, no two the same picture.
  Setting `border-color` on a disabled control needed item 184, which was taken
  with it because the code had already written down the day it would be needed.

- [x] **183. A fieldset looks like a group.** No border, so the thing that makes
  a fieldset worth using is invisible. Real browsers draw a groove the legend
  breaks through, which is the interesting part: the legend sits *in* the top
  border rather than above it.
  *Depends on 181. Closes when:* a fieldset draws a border with its legend
  breaking it, in a reference render.

  **Done: corpus case `fieldset-group`, and `web-a-form` has its two groups
  back.** The interesting part is a **band**: a fieldset showing a legend gets
  no block-start border for the layout run and a band as tall as the legend
  instead, with the border recorded beside it and drawn through the middle
  afterwards. The band *replaces* that border rather than adding to it, which
  is the difference between a fieldset as tall as a browser's and one two
  pixels taller — `alo_layout::legend` is the whole rule, and it is the only
  place in the engine that knows a fieldset from any other block. The box tree
  hoists the legend to the front, because HTML draws a fieldset's **first**
  legend at the top whatever comes before it; a flex or grid fieldset is left
  alone, because lifting an item out of a layout somebody wrote is this engine
  overruling them. Two things went to the queue: 190, and the note that a
  fieldset with a `border-radius` and a legend has square corners.

- [ ] **180. GIF, WebP and AVIF.** Rented. The same bound and the same refusals
  as PNG and JPEG, added to the one list the tests already walk.
  *Depends on 177. Closes when:* each decodes a frozen file and each is refused
  the same way — and an animated GIF shows its first frame rather than nothing,
  because a still picture is a better answer than a gap while item 109 is
  outstanding.

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

---

# Queue — stage 3

`ROADMAP.md`: **the legacy tail.** *"Deliberately last, and possibly never
finished — a choice, not a failure. Refusing this list is what made stages 1 and
2 survivable."*

**Nothing here is taken because it is next.** Every item below is opened by a
**page in the corpus that fails because of it**, and by nothing else — not by a
specification listing a feature, not by a queue position, and not by a loop
looking for something to do. `ROADMAP.md` says it plainly: *let a broken render
schedule the work.* `LOOP.md`'s stage-boundary rules say what that means for an
iteration.

So every item here is written `blocked: no page yet`, and stays that way until
somebody adds the page. That is not a placeholder — it is the state the item is
actually in, and a loop that started one anyway would be building stage 3 for
its own sake, which is the thing this stage exists to refuse.

- [ ] **135. Quirks mode.** `alo-dom` already records the doctype signal and
  refuses to honour it (law 1). This is where a page that needs it gets it.
  *blocked: no page yet. Needs ADR* — law 1 refuses quirks mode outright, so
  implementing it is a change to the constitution rather than an item.

- [ ] **136. Floats as layout, and CSS table layout.** The two that most often
  turn an old page into a column of rubble.
  *blocked: no page yet. Cut before starting*; they are two items and probably
  more.

- [ ] **137. `document.write`, live `HTMLCollection`s, and the DOM as it was
  before it was a specification.** *blocked: no page yet. Depends on 80.*

- [ ] **138. Legacy character encodings, and detecting them.** The tables are
  already rented (queue item 51 brought `encoding_rs` for the declared ones);
  what is owed is **detection** — guessing from the shape of the bytes when
  nobody has said. This engine does not guess today, and that is stated in
  `alo-net::encoding`.
  *blocked: no page yet.*

- [ ] **139. XML, XHTML and XSLT.** *blocked: no page yet. Cut before
  starting.*

- [ ] **140. `frameset`.** *blocked: no page yet. Depends on 86.*

- [ ] **141. Vendor prefixes, and anything that exists only for a page written
  before 2015.** *blocked: no page yet.*

- [ ] **142. The sloppy-mode corners of JavaScript that only old code reaches.**
  *blocked: no page yet. Depends on 72.*

**There is no exit gate**, and that is the design. Stage 3 is a standing offer
rather than a milestone: it is finished when nobody is finding broken pages any
more, which is not a state anybody declares.

---

# Queue — stage 4

`ROADMAP.md`: **a browser somebody chooses.** Product work, and **gated:
nothing here starts until stage 2's exit gate is met.**

**A loop cannot open this stage.** Stage 2's gate is *"a person uses it as their
browser for a week and reaches for another one only for a site they can name"* —
which is a judgement a person makes and a loop must never make on their behalf.
When stage 2's queue empties, the loop writes `LOOP COMPLETE` and stops; a
person unblocks this stage or does not.

- [ ] **143. ADR 0008 — what an extension is.** WebExtensions, or something
  narrower we can actually secure. *Needs ADR, and it is the first item here*:
  every other browser's extension API is a privilege surface bolted to the
  side, and adopting one without deciding is how a sovereignty product acquires
  somebody else's threat model.
  *blocked: stage 2's exit gate.*

- [ ] **144. Extensions.** *Depends on 143. blocked: stage 2's exit gate.*

- [ ] **145. Sync, self-hosted.** Bookmarks, history, tabs and passwords,
  end-to-end encrypted, on the customer's own server. *Depends on 90, 126.
  Needs ADR* — end-to-end encrypted against whom, and what the server can see.
  *blocked: stage 2's exit gate.*

- [ ] **146. Updates that are signed, staged and reversible.**
  *blocked: stage 2's exit gate.*

- [ ] **147. Crash handling that helps us fix it without becoming telemetry.**
  `ROADMAP.md` refuses telemetry outright, so the interesting half is what a
  crash report may contain and who decides to send it. *Needs ADR.*
  *blocked: stage 2's exit gate.*

- [ ] **148. ★ Translation on the machine.** alo already runs models locally; a
  page translated without sending it anywhere is the sovereign version of a
  feature every other browser sends to a server. *Depends on 133 for the record
  of what was read. blocked: stage 2's exit gate.*

- [ ] **149. ★ Reading and summarising a page locally**, under the same grants
  and the same record. *Depends on 133, 148. blocked: stage 2's exit gate.*

- [ ] **150. Enterprise: policy, managed configuration, and an update mirror an
  organisation hosts.** *Depends on 146. blocked: stage 2's exit gate.*

- [ ] **151. A mobile port.** Last, and it is a port rather than a feature: what
  it needs is everything above it to be finished. *blocked: stage 2's exit
  gate.*

**Exit gate** (`ROADMAP.md`): somebody outside alo chooses this browser, on a
machine we did not set up, and stays.

---

## After stage 4

Nothing. `ROADMAP.md` names four stages and this queue now covers all of them,
which means **the loop can always say what is next** — and, at the two places
where only a person can decide, can say that instead.

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
