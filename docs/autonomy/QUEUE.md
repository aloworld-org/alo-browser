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
  ADR 0026 makes it the *storage access* capability in the one grant table
  (item 307), asked in item 308's prompt, so that prompt is the interface
  this waits on.

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

- [x] **265. An origin-only `Referer` ends in `/`.** *Found by item 263.*
  `alo-net`'s `referrer::for_request` writes the origin-only referrer as
  the origin's serialisation, `https://example.com`, where the Referrer
  Policy standard strips the URL to its origin and serialises it *as a
  URL*: `https://example.com/`, which is what Chromium and Gecko send.
  Every cross-origin request under the default policy says it.
  *Depends on nothing. Closes when:* `origin`, `strict-origin` and the
  cross-origin half of the `*-when-cross-origin` policies answer
  `https://example.com/`, `referrer.rs`' tests and `alo-renderer`'s
  `navigate.rs` test say so, and a port and an IPv6 host keep their form.

  **Done.** One function, `origin_only`, writes the origin and its `/` for
  all four policies, so they cannot drift apart. `referrer.rs`' tests live in
  `alo-net/tests/what_a_page_may_reach.rs`, where the new one walks every
  origin-only policy over a default port written out, another port, `http`
  with a port and two IPv6 hosts. An opaque origin sends nothing rather than
  `null/`; nothing reaches that from `http` or `https` today, and the
  function's comment says so rather than a test pretending otherwise.

- [ ] **60. HTTP/3 and QUIC**, once both of those are.
  *Depends on 59. Needs design:* name the QUIC rental boundary, transport
  integration, frozen protocol fixtures and bounded closing conditions before
  this becomes a build item. No protocol implementation is claimed.

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

- [x] **170. Fonts a page asks for by name.** *Item 68's first case is the
  evidence: it asks for `system-ui, sans-serif`, gets DejaVu Sans, and nothing
  says so.* Today every renderer is given the
  same short list at startup. A page asking for a family nobody sent gets a
  fallback, silently.
  *Depends on 168. Closes when:* a renderer can say which family it wanted and
  did not have, and the browser process can answer with it — and a family that
  genuinely is not on the machine is a named substitution rather than a silent
  one.

  **Done, all three clauses:
  `crates/alo-renderer/tests/a_font_a_page_asked_for.rs`**, over the real
  boundary with a spawned, confined renderer. `FromRenderer::Loaded` carries a
  `wanted` list, `Renderers::supply` is the answer, and it returns the families
  this machine genuinely does not have rather than swallowing them.

  **Two things are worth reading twice.** The first is a **distinction the
  whole item turns on**: a family that is not here is an *ask* — the machine may
  have it — and a substitution is a *message to a person*, which happens only
  when **nothing** the page named was here. A page whose second choice was found
  got the fallback its own author wrote, and reporting that would put a warning
  in front of somebody about a page working exactly as written. `alo_text::Absent`
  is that distinction as a type. The second is that `fonts::named` reads the
  family out of the **font** (`alo_text::family_in`) rather than off its
  filename: the startup list may guess, because a guess only decides what is in
  a database, but *"does this machine have Inter"* decides whether a page is
  drawn as its author wrote it, and a guess is wrong for every font somebody
  else named.

  **The corpus did not move, and that is the review.** Every alo case declares
  what its generics mean (`corpus_fonts`), so nothing there was ever silently
  substituted for, and a rule that reported those would have been the wrong
  rule. What was genuinely silent is what nobody had looked at: **the browser
  process declares no generic at all**, so a real renderer asked for
  `system-ui`, fell off the end of the chain, and said nothing. Two cuts: items
  192 and 193.

- [x] **192. A font whose name is only in a legacy platform encoding.** Cut
  from 170. `alo_text::family_in` reads the `name` table and answers [`None`]
  for a font that carries its name only in an old Macintosh encoding — several
  that macOS ships do, Apple Braille among them. Such a family cannot be found
  on demand, so a page asking for it by name is told the machine does not have
  it. **That is the safe direction and it is still wrong**: the machine has it.
  The alternative — falling back to the filename — was refused deliberately,
  because it would put a guess back inside the one answer that has to be a
  fact.
  *Depends on 170. Closes when:* a font naming itself only in a legacy encoding
  is found by the name a person would type, in a test that names the encoding
  — and nothing anywhere returns a filename as a font's family.

  **Done, both clauses, and the rule that decided the scope is
  `alo-text/src/macintosh.rs`'s whole reason to exist: read the encodings
  somebody else's table defines exactly, and guess at none of them.** Mac OS
  Roman and Mac OS Cyrillic are `macintosh` and `x-mac-cyrillic` in the WHATWG
  standard, so `encoding_rs` — already rented for a page's bytes, now rented for
  a font's name — holds Apple's own tables for those two. The other twenty
  Macintosh encodings have no such table: Mac OS Japanese is close to Shift JIS
  and is not Shift JIS, and a family read *wrongly* is worse than one not read,
  because it is a name a page can match by accident. They answer nothing, which
  is what every Macintosh record did before.

  **A Unicode name wins wherever a font has one**, which is what keeps this from
  changing the answer for any font that already had a readable one: a Macintosh
  record comes first in a well-formed table, so reading in file order would have
  quietly demoted every font that carries both. Two rules went in beside it,
  applied to every record whatever its encoding, because the bytes were written
  by somebody else: a name longer than `LONGEST_NAME` is not a family name, and
  neither is one carrying a control character.

  The tests build their fonts rather than looking for one, in
  `crates/alo-text/tests/a_font_that_names_itself_in_an_old_encoding.rs`: a real
  font with its `name` table replaced byte by byte, so the encoding is named in
  the bytes — `0xD5` is a right single quote in Mac OS Roman and `Õ` in Latin-1,
  and a test written in ASCII would have passed either way. A table lying about
  its own lengths, every truncation of one, and every single flipped bit of one
  are answers rather than crashes.

  **The second clause was the cheaper half and the more surprising one.**
  `fonts::from_file` named a face after its *file*, on the argument that a
  startup database is only a guess about what to look at and that opening every
  font to ask would be most of a second — and it already reads every one of
  those files, because ADR 0010 makes a face bytes rather than a path. So the
  cost was a `name` table rather than an open, and the argument had been wrong
  since the day the sandbox landed. The cut is item 194.

- [x] **194. A face's weight and slant, from the font rather than from its
  filename.** Cut from 192, which took the *family* off the filename and put it
  back where it belongs. The other two fields of a `Face` are still guessed at
  by looking for `bold` and `italic` in the file's name, which is wrong for
  every file somebody named by another convention — `Helvetica-Oblique`,
  `InterDisplay-SemiBold`, a variable font with a weight axis and no word for
  it. It is a smaller wrong than the family was: a face filed under the wrong
  weight is still drawn in the right family, because `FontDatabase` chooses
  among the faces of the family it has, where a face filed under the wrong
  family is not drawn at all. The `OS/2` table states both properly, and
  `alo-text/src/font.rs` already parses that face.
  *Depends on 192. Closes when:* a font stating a weight and a slant is filed
  under them whatever its file is called, in a test that names a file wrongly on
  purpose — and a font that states neither is still a face rather than nothing,
  since a family of one unlabelled face is most of the fonts on a machine.

  **Done, both clauses, and `alo_text::style_in` is the sibling of
  `family_in`** — one table further on and the same argument, which is why the
  pair comes back together: weight and slant are one sentence written side by
  side in `OS/2`, and a caller asking twice would parse the file twice to learn
  two halves of it. `from_file` no longer looks at the path for anything but
  opening it.

  **Two readings are decided rather than left to whatever a clamp does.** A
  stated weight of **zero is not a statement** — it is what a font writes when
  it did not say, and brought into CSS's range it would become 1, a hairline,
  which is a wrong answer that reads like a right one. So the only other thing
  the table says about heaviness is read instead, the bold bit, and a font
  saying neither is normal. Where a number and that bit disagree the **number
  wins**, because CSS asks its question as a number and the bit is a two-value
  shorthand. A weight in `1..=9` is kept as written: some fonts older than the
  current specification meant the nine-point scale, the bytes are identical
  either way, and a guess would draw a page in a face nobody chose.

  The test that names a file wrongly on purpose names **two**, swapped, so a
  rule reading the filename gets both wrong and a rule reading the font gets
  both right — and it ends in the numbers, because which face a page is given
  decides how wide its text is and so where every line of it breaks. On this
  machine the change is visible: Apple states 295 for its monospace face and
  1000 for its compact one, and `.SF NS Mono` had been filed at 400. The cut is
  item 196.

- [x] **196. A variable font is one file and many weights.** Found by 194,
  which reads a face's weight out of `OS/2` and thereby reads *one* weight out
  of a file that holds a continuum. macOS's `SFCompact.ttf` has a `wght` axis
  and states 1000, so this engine files the whole family as the heaviest thing
  CSS can name; `SFNSMono.ttf` states 295. Neither is wrong about the default
  instance and both are wrong about the font. Nothing is drawn in the wrong
  family — a family whose only face is 1000 still answers a request for 400 —
  so this is a face chosen badly rather than a page drawn in the wrong font,
  which is why it is a cut rather than a defect in 194.
  *Depends on 194, and on `docs/features.md`'s stage 2 line for variable fonts.
  Closes when:* a page asking for two weights of a variable family is drawn in
  two different widths of text, in a layout assertion — and a font with a `wght`
  axis reports the range it covers rather than the one instance its `OS/2`
  names.

  **Done, both clauses, and the decision the item was waiting for is what a
  weight *is*.** It stopped being a label and became an instruction: a
  [`Font`]'s weight is the instance every face parsed out of its bytes is set
  to, and `FontDatabase::chain` hands back fonts **set to the weight asked for**
  rather than references to the ones it holds. That is why the return type
  changed — the font a request gets from a variable file is not something the
  database has. Distance is measured to what a face *can be* rather than to what
  it is, which is the one rule the whole item turns on.

  **It reaches three parsers of the same bytes and all three had to agree.**
  Advances come from `HVAR`, outlines from `gvar`, and each is applied only once
  the face has been told which instance it is — so a font measured at 700 and
  drawn at 400 would put light letters at heavy spacing, which no width
  assertion would have caught. `Font::face` and `Font::shaper` are both in
  `font.rs` for that reason; `alo-paint` gets the number and the tag as plain
  values, because the parser is rented behind one file in each crate.

  **The fonts are built rather than found**, in both halves: a machine either
  has a variable font or does not. `alo-text`'s case writes an `fvar` and an
  `HVAR` into a real font and asserts what it measures; `alo-paint`'s writes an
  `fvar` and a `gvar` and asserts that a letter's first point moves by exactly
  the delta the file states. Two fonts rather than one on purpose — either half
  would otherwise pass on the other's evidence.

  **A survey of this machine is the review**: 28 of 370 readable fonts declare a
  weight axis, the system font among them. One of them decided a rule.
  `Skia.ttf` runs from **1 to 3** — an Apple axis older than `wght` having a
  shared meaning, with `OS/2` stating 5 — and read as CSS numbers every request
  would land on its heaviest end and ordinary text would come out black. An axis
  ending below the lightest weight CSS has a word for is left alone, which is
  the same refusal item 194 made one table earlier. The cut is item 197.

- [ ] **197. The axes that are not weight**: `wdth`, `slnt`, `ital` and `opsz`.
  Cut from 196, which reads `wght` and reads past the rest. Each is a separate
  CSS property — `font-stretch`, `font-style: oblique <angle>`,
  `font-optical-sizing` — with a grammar of its own, and guessing at one would
  draw a page narrower or slanted because this engine assumed an axis nobody had
  looked at. The machinery is in place: `Font::at_weight` and the tag crossing
  to `alo-paint` as a value are the shape each of these takes.
  *Depends on 196, and on the property it implements existing in `alo-style`.
  Closes when:* `font-stretch: condensed` on a family with a `wdth` axis is
  narrower text in a layout assertion, and a font whose axis is not in the
  property's scale is left alone the way `Skia.ttf`'s is.

- [x] **193. What a generic family means on this machine.** Cut from 170, and
  it is the gap that item made visible. `FontDatabase::map_generic` exists and
  **only tests call it**: the browser process hands over faces and never says
  which of them is this machine's `sans-serif`, `serif`, `monospace` or
  `system-ui`. So the user-agent sheet's own `font-family: system-ui,
  sans-serif` reaches every real page as two families nobody has, and is
  answered by falling off the end of the fallback chain. Item 170 made that
  *audible* — it is reported now — rather than fixing it, because what a
  generic means is a fact about the machine that has to cross the boundary and
  a face does not carry it.
  *Depends on 170. Closes when:* a renderer is told what each generic means as
  part of being given fonts, a page asking for `sans-serif` on a machine that
  has one is not reported as substituted, and one asking on a machine that has
  none still is.

  **Done, all three.** `ToRenderer::UseGenerics` carries the mapping, sent by
  `Renderers::start` after the faces and before any page — in that order,
  because a generic names a family and a renderer asked which of them it can
  answer before it holds a face would truthfully say none. The answer,
  `FromRenderer::UsingGenerics`, names only the generics a face actually
  resolves: a mapping to a family the renderer was never given would otherwise
  have the browser process believing every page here has a `sans-serif` while
  text kept coming out in whatever was to hand.

  **A generic keeps every candidate this machine has, in preference order**,
  because `FontDatabase` already holds one as several families and tries them in
  turn — so `sans-serif` on this machine means `.SF NS` and then `Geneva`, and a
  character the first lacks is still drawn by the second. `crate::generic` holds
  the candidate lists and the choosing; `choose` is separated from the compiled
  table so what this file *decides* is tested on every platform rather than only
  on the one it was written on.

  **Only four**, and `cursive` and `fantasy` are refused rather than guessed:
  there is no answer for them on any machine that is not a guess, and a guess
  here is a page drawn in a typeface nobody chose. They stay in the state item
  170 made reportable.

  Two things had to change in how a machine is read. The short list was
  alphabetical and stopped at the first two dozen **faces**, so whether a machine
  had a `sans-serif` at all was decided by where its family sorted — it now looks
  at up to `MOST_LOOKED_AT` files, keeps a family some generic wants even when
  the list is full, and puts those families first when it cuts down to
  `MOST_FONTS`. And `from_this_machine` returns the fonts and the generics
  **together**, as one `Machine`, because the second is read out of the first and
  a caller deriving it again would be two chances for them to disagree — which is
  the argument `fonts::named` already makes about a filename.

  The layout assertion is in `text_in_a_generic_is_measured_in_the_family_the_
  generic_means`: text asking for `sans-serif` lays out to the same width as text
  naming the family outright, and to a different one when nobody was told —
  because a generic decides what the text is *measured* in, and so where every
  line breaks.

- [x] **195. A font's name in a language somebody asked for.** Found while
  building 193. `alo_text::family_in` reads the `name` table and takes the
  **first** record of each kind, whatever language it is in. macOS's system font
  states its family thirty-five times over — `System Font`, `Police système`,
  `システムフォント` — and this engine is saved from filing it under Catalan only
  by the accident that its Unicode-platform record happens to come first. A font
  whose first Windows record is a localised one is filed under a name no page
  will ever ask for, which is item 192's whole failure mode arriving by another
  road. The `name` table states a language id per record: Windows English is
  `0x0409` and its regional relatives, Macintosh English is `0`.
  *Depends on 192. Closes when:* a font carrying its family in several languages
  is filed under the English one, in a test whose fixture puts a localised record
  first — and a font that states no English name at all is still filed under
  something, since a font in one language is a font a person may still have.

  **Done, both clauses, and the item's own guess about the accident was worth
  re-checking.** The accident holds — the system font's unlocalised record does
  come first — but four other fonts on this machine were filed under Chinese
  names all along: `Songti.ttc` under `宋體-簡`, `STHeiti Light.ttc` and
  `STHeiti Medium.ttc` under `黑體-繁`, `Hiragino Sans GB.ttc` under
  `冬青黑體簡體中文`. They are `Songti SC`, `Heiti TC` and `Hiragino Sans GB` now,
  which is what CoreText calls them.

  **The order has three steps rather than two, and the third is the one that
  needed evidence.** A record stating **no language** — the Unicode platform
  defines none — is the font's own name and wins over English, because that is
  how macOS reads its own system font: CoreText answers `.SF NS` for `SFNS.ttf`
  and keeps `System Font` as the name to *show* a person. Ranking English above
  it would have renamed this machine's `sans-serif` out from under item 193's
  candidate list. Then English, in any of the sixteen ids that spell it, then any
  other language with the first record winning.

  The language decides **inside** a kind of name and never between two, so the
  typographic name is still the family CSS means whatever language either is in —
  and item 192's rule that a Unicode record beats a Macintosh one is untouched.
  The cross-check that the whole file rests on is that the test writes out the
  sixteen English ids by hand while the engine reads a rented table: two roads to
  one answer, and they meet on all 65 536 ids a font could carry.

- [x] **64. The transport, and the lifecycle** that starts, reuses and reaps
  renderers, with a bound on how many exist.
  *Depends on 63.* **Most of it is built and one word of it is not**, which is
  worth writing down here rather than leaving somebody to find: item 63 is the
  transport, item 166 is starting a process per site, reusing it, bounding how
  many exist at [`MOST_RENDERERS`] and evicting the least recently used — and
  **nothing reaps**. A renderer whose last tab has closed keeps running until
  the ceiling happens to evict it. That could not be said before item 65,
  because there was nothing that was a tab; it can be said now.
  *Closes when:* closing the last tab on a site stops that site's process, in a
  test that watches the process go, and closing one of two tabs on a site stops
  nothing.

  **Done: `Renderers::reap`, and the division of labour is the thing to read.**
  `tab.rs`'s `close` had a comment saying reaping was not its to do, because
  deciding a process ends on the strength of holding the last reference to it is
  how a lifecycle ends up scattered. So the caller says what it still **wants** —
  the sites that have a tab open — and `host.rs` decides what that costs a
  process. A site left out of `wanted` by mistake costs a process that starts
  again; one left in by mistake would be a renderer nothing can ever reach, and
  that asymmetry is why the argument goes in that direction.
  `tests/a_renderer_nothing_wants.rs` asks the operating system rather than this
  program: `kill -0` on the process id, which is a real answer only because
  `stop` waits — an unwaited process is a zombie and a zombie answers `kill -0`
  like anything else.

  Two things went in with it. The tab whose page a reaped renderer was holding
  is forgotten, because a `held` entry outliving its process refuses the next
  tab on that site on behalf of a renderer nobody can reach — doctored out, and
  the test names it. And reaping **only ever ends things**: a wanted site with
  no renderer does not get one, which is the same rule as everywhere else here,
  that nothing starts a process except somebody asking for a page.

  The cut is **item 198**, found on the way and not folded in: nothing bounds
  how long an exchange may take.

- [x] **198. A renderer that stops answering without dying.** Found while
  building 64. `pipe::read` blocks until bytes arrive, so a renderer that is
  alive and never answers — wedged on a page, or on something a hostile page
  arranged — hangs the **browser** process, which is the one thing ADR 0005 says
  must never happen. Killing a hung renderer is a lifecycle act and the
  lifecycle has no clock: `Gone` can already say a renderer went, and nothing
  can decide that one should.
  *Depends on 64. Closes when:* a renderer that never answers is given up on
  after a bound a test can name, the tab says what happened in the same shape as
  a tab whose renderer died, and a renderer that is merely **slow** is not
  killed — which is the half that decides what the bound may be.

  **Done, all three clauses, and the wedged renderer in the test is the real
  binary stopped with `kill -STOP`** — alive, its pipe open, and never going to
  answer, which is the condition itself rather than a stand-in that shares only
  the silence. The same signal makes the other half exact: a renderer stopped
  and then continued is slow by precisely as long as a test says, which nothing
  about a real page could promise.

  **The clock needed a thread, and that is the whole of `answers.rs`.** A pipe
  read cannot be given a deadline in safe Rust — the calls that would are FFI,
  and ADR 0010 refused FFI for the sandbox itself on that ground — so the read
  happens on a thread and the browser process waits on a channel, which does
  take a bound. The channel holds **one** message, deliberately: a thread
  reading ahead as fast as a renderer can write is a renderer that fills the
  browser process's memory by talking, and the blocking read had that
  backpressure for free.

  **A bound without a kill would be worse than no bound.** The protocol is one
  answer per request, so an answer arriving after we stopped waiting would be
  handed back as the answer to the *next* question — a picture of the wrong
  page, or a tree an agent then acts on. So a silence is fatal to the renderer
  rather than something to retry.

  **Ten seconds, and it is a choice rather than a measurement** — said so in the
  constant, because `LOOP.md` says a claim about speed is measured on hardware
  or not made. Too short loses pages that were about to arrive; too long is a
  frozen browser. The honest version is a question — *wait, or stop it?* — and
  asking it needs an interface, which is items 157 and 158's block.

- [x] **65. A renderer that dies takes its tab and nothing else** — and says so,
  rather than leaving a blank rectangle.
  *Depends on 63. Closes when:* a renderer is killed from outside and the tab
  says what happened while every other tab keeps working.

  **Done: `alo-renderer`'s `tab.rs`, and the item was open because a tab did
  not exist.** Item 166 made one renderer's death survivable by the others and
  `ROADMAP.md` ticked this line beside it, but what that item built was a
  `Gone` **returned to whoever asked**. Nobody kept a painted frame anywhere,
  so the thing this line names — the blank rectangle — is precisely what a
  person would have been shown. A tab holds its last frame now and keeps it
  when its renderer goes.

  **The rule worth reading twice is that nothing here restarts anything.**
  `Renderers::ask` starts a process for a site that has none, so a repaint of a
  dead tab would have spawned a fresh one, found it holding no page, and
  reported that nothing was loaded — the crash gone from view, which is the
  silent restart ADR 0005 refuses arriving by another road. A tab that has been
  told answers from what it knows; only a deliberate `load` starts a renderer,
  and the test counts the processes to say so. The same check catches a
  renderer **evicted** to stay under the ceiling, which goes away without
  anybody dying.

  The deciding is a pure function (`may_ask`), the shape items 55, 154 and 188
  already use and for a version of the same reason: every rule in it is a rule
  about *not starting a process*, and that is asserted honestly only when
  nothing is moving. One thing was found while building it and is refused
  rather than answered wrongly: two tabs on one site share a process, a
  renderer holds **one** document, so the second tab to load displaces the
  first — `Lost::HoldsAnotherPage` says so instead of answering about somebody
  else's page. `docs/features.md`'s *"several documents at once, the shape tabs
  need"* is the item that ends it. The cut is written into item 64: nothing
  reaps.

- [x] **66. Where one site ends and another begins.** The origin, the site, the
  registrable domain, and which of them gets a process.
  *Depends on 50, 63. Closes when:* every URL a page could hold is given one of
  the three by a rule written down, and the case that was wrong is right — two
  documents whose origins are **opaque** are never in one renderer process, in a
  test with real processes in it.

  **Done, and the closing condition was written on taking it** — the item had
  none, which stage 2's rules say makes an item unready; what made it ready
  instead of `needs design` is that `ROADMAP.md` already named exactly what was
  owed, *"which of the origin, the site and the registrable domain a page is
  given, case by case"*, and the case that was wrong was findable by reading the
  three answers side by side.

  **The rule is one sentence: the origin decides whether there is a site at
  all.** Where it is a tuple the registrable domain widens it into a site, and
  the **port is left to the origin** — two ports are two origins that can
  already reach one another with a link and a cookie, so a process each would
  cost memory and buy nothing. Where it is opaque there is no site, and the
  document is `Site::Alone` with the opaque origin's own identity in it: a
  process nothing else is ever put into.

  **What was wrong is that a hostless URL was read as the scheme and nothing
  else**, so every `data:` page in the browser was one site, every `about:` page
  was one site, and — the one that matters — **every local file on the machine
  was one site sharing one address space**. `alo-url` has said since item 50
  that each of those is its own origin and that *"one local file being able to
  read every other one is the oldest exfiltration bug there is"*; the process
  split was quietly undoing it.

  The answer is **taken from `Origin::of` rather than restated** from the URL,
  which is the whole of why this is small: two functions deciding what is opaque
  are two functions that can come to disagree, and the disagreement would be a
  process holding two documents the security rules call strangers. It also
  settles the cases nobody had asked about — a scheme with no default port and
  no port written is opaque, so unknown still never means "probably fine".

  **The cost is written down rather than discovered later**: twenty local files
  open is twenty renderers, up to `MOST_RENDERERS`, past which the least
  recently used is evicted. ADR 0005 already priced that. And a rule went in
  beside it because the shape invites the opposite: a site is decided **once**,
  when the tab is opened, since `Site::of` on an opaque origin mints a new
  identity every call — a caller that asked again per request would give one tab
  a new process every time it painted. `tab.rs` asserts both halves.

  What is left is not this item's and is named in `ROADMAP.md` and
  `docs/features.md`: a **document inside a document**, which nothing here can
  yet produce — a sandboxed `iframe`'s opaque origin and `about:srcdoc`
  inheriting its parent's (item 86), and a `blob:` taking the origin of whoever
  created it (items 72 and 90).

- [x] **67. ★ Every request attributable** — which page, and **which agent
  action**, caused it. `ROADMAP.md`: *"no other engine has needed to answer
  that, and an agent-driven browser that cannot is one nobody should trust."*
  *Depends on 53. **ADR 0012 is written and accepted** — what is recorded, for
  how long, and who may read it, in the shape of `alo-os` ADR 0001. (The queue
  did not name a number and 0012 was the next free one; `alo-os` ADR 0001 is not
  checked out here, so the shape is taken from ADR 0002, which records it.) The
  code is what remains, and the ADR names the clauses it must carry: a
  **cause** on the request with **no default**, so one that cannot say what
  caused it does not compile; **three causes and no fourth** — a person, a
  document, an agent action — with engine-made requests attributed to whatever
  caused the thing they are about rather than to an `Unknown`; a cause is **a
  link in a chain**, each document recording what caused its own load, because
  *which page* and *which agent action* are two questions with two true answers;
  it is assigned by the **browser process** and never by a renderer, which
  states a [`Purpose`] and never a cause; everything is held **for the session
  in memory**, and only what reaches an agent action is **kept until the person
  deletes it**, under ADR 0011 section 3's rules and never opened at all for a
  session-scoped profile; and **no page and no agent may read it**, ever.
  *Closes when:* every request the engine makes names its cause and there is no
  way to make one that does not, an agent's action is reachable from every
  request that followed from it in a test that walks the chain, and a renderer
  that states a cause is a renderer that has been ignored.

  **Done for the first clause, and cut on starting into 199 and 200** — the
  shape the iteration before this one predicted, because the field, the chain
  and the durable record are three pieces of work and only the first is a
  prerequisite for the others. `alo-net/src/cause.rs` is the type;
  `Request::get` and `Request::sending` take a [`Cause`] as an **argument**, so
  the guarantee is a signature rather than a habit and the `compile_fail`
  example on `Request::get` is what checks it.

  Two things are worth reading twice. **The identities live in `alo-net`**, and
  `alo_renderer::tab::TabId` is now a re-export of `alo_net::cause::TabId`
  rather than a second type: a cause is a field on a request, a field cannot
  name a type from a crate that depends on it, and two identity spaces for one
  tab is exactly what ADR 0003 exists to refuse. Nothing but
  `cause::Identities` can mint one, which is also ADR 0012 § 4 made structural
  — a renderer holds no `Identities` and therefore has nothing to state a cause
  *with*. And **the four engine-made requests each clone the cause of the thing
  they are about** — a redirect hop, a resumed range request, a CORS preflight,
  a violation report — which is what let the decision have no `Unknown` in it;
  `tests/what_caused_a_request.rs` sweeps all four together rather than
  asserting each in its own file, because a fifth appearing with a fresh cause
  is the drift worth catching. One test is named for the attack: a server
  answering `302` cannot promote a page's fetch into something the person did.

  The third clause is **not** met and is not claimed: nothing today can be
  called a renderer that stated a cause, because no message crossing the
  boundary carries a request at all. It is written into item 199 rather than
  left implied.

- [x] **199. A cause is a link in a chain.** Cut from 67, which carries one
  cause per request and no way to get from a [`Cause::Document`] to what caused
  *that* document's load. ADR 0012 § 3: *"which page, and which agent action* is
  two questions, and the second one is usually answered indirectly" — an agent
  activates a link, a document loads, that document fetches a script, and only
  the walk says the script was the agent's doing. So a document has to record
  what caused its own load, which means a document has to be a thing with an
  identity outside a `Cause` — `alo_renderer::Tabs` mints tabs and mints no
  documents, and `Page` is markup and a viewport.
  *Depends on 67. Closes when:* an agent's action is reachable from every
  request that followed from it, in a test that walks the chain, and the walk
  terminates on a cycle rather than looping — ADR 0003's ids make a cycle
  impossible to *create*, and a walk that trusted that is a walk that hangs on
  the first bug. **And the clause item 67 could not reach**: a renderer that
  states a cause is a renderer that has been ignored, which needs a renderer
  that can ask for a subresource (items 80 and 83) before there is anything to
  ignore. If those are still unbuilt when this is taken, say so and cut it
  rather than asserting a boundary nothing crosses.

  **Done, both clauses that could be reached, and the third is said rather than
  asserted** — items 80 and 83 are unbuilt, no message crossing the boundary
  carries a request, and a test of a renderer being ignored would be a test of
  nothing. It is written into item 201 rather than left implied.

  The thing the item was waiting for is that **loading a page is what makes a
  document**: `Tabs::load` takes the [`Cause`] its own request carried, mints
  the document and records the pair in one act, so there is no moment at which
  a document exists without a cause and no second call that could give it a
  different one. `alo-net/src/chain.rs` is the record and the walk;
  `alo_renderer::Tabs` is the one thing that writes to it, which is ADR 0012
  § 4 unchanged — a renderer holds no `Tabs`, no `Identities` and no
  `Documents`, so it has nothing to attribute anything *with*.

  **Two rules are worth reading twice.** The document a cause names is taken
  from the **tab**, never from a caller or an answer
  (`Tabs::an_agent_acting`), so an agent acting in one tab cannot reach into
  another's browsing. And the walk carries the documents it has been through
  and stops if one comes back: a cycle cannot be created, and a walk that
  trusted that would hang the **browser** process, which is the one thing
  ADR 0005 says must never happen. Its test reaches past the constructor to
  build a cycle by hand, because what is asserted is that the walk survives a
  state nothing can put it in.

  The bound is ADR 0012 § 6's *bounded*, and the honesty owed with it is a
  variant: a chain reaching a document dropped under `MOST_DOCUMENTS` says
  `Forgotten`, and one reaching a document nothing ever recorded says
  `Unrecorded` — *we knew and no longer do* and *nobody ever said* are
  different answers, and a record that ran them together would be guessing in
  the one place that exists not to.

- [ ] **201. A renderer that states a cause is a renderer that has been
  ignored.** Cut from 199, which could not assert it: ADR 0012 § 4 says the
  browser process assigns a cause and a renderer never does, and today a
  renderer has nothing to state one *with* and nowhere to put it — `ToRenderer`
  and `FromRenderer` carry no request in either direction, and a renderer holds
  no `Identities`. So the boundary is real and nothing crosses it, which is why
  a test today would be a test of nothing.
  *Depends on 80 and 83 — a renderer that can ask for a subresource is what
  makes a request cross the boundary at all. Closes when:* a renderer asks for
  a subresource and the cause recorded is the one the browser process composed
  from the tab, in a test where the renderer's message says something else and
  is ignored.

- [x] **200. The record itself.** Cut from 67, which carries a cause on a
  request and writes nothing down. ADR 0012 §§ 5, 6 and 7 are all this item:
  **what is recorded** (when, the cause chain, the method and URL, the purpose,
  and what happened — never a body, never a header set, never anything a page
  chose to put there); **everything for the session, in memory, bounded**, which
  is what a person opens to see what a page is doing and what item 129's
  developer tools read; and **only what reaches an agent action kept until the
  person deletes it**, under ADR 0011 § 3's rules unchanged, never opened at all
  for a session-scoped profile, and bounded in **actions rather than bytes** so
  one busy action cannot evict a week of ordinary ones.
  *Depends on 199 — a record of chains needs chains; without them every durable
  entry would be a request whose cause happened to be an agent action, which is
  the narrowest possible reading of the promise and is not the one the ADR
  makes. Closes when:* a session's requests are all in the in-memory record and
  it is bounded, an agent's work survives a restart and a person's browsing does
  not, a private profile leaves no file behind at all (**never written**, rather
  than written and deleted — ADR 0011 § 2's rule), and there is no API of any
  kind by which a page or an agent could read any of it.

  **Done for the session's record, and cut on starting into 200 and 202** — the
  shape the item's own last sentence asked for, and the same seam the cache was
  cut at: item 56 was the cache in memory and item 155 was the cache on a disk,
  because *what may be reused* and *what may be written to a disk other programs
  can read* are two questions with two answers. `alo-net/src/activity.rs` is the
  record; `Pool` holds one.

  **The clause that decided where it lives is *everything*.** A record every
  caller writes to is a record missing exactly the lines nobody thought of, so
  it is written in `Pool::fetch_however_it_ends` — the one place every door in
  that type leads through — which is what makes the engine-made requests lines
  without any of them being asked to be. A retry inside that function is one
  line, because it is one thing that happened.

  **Three rules are worth reading twice.** *Never a body and never a header set*
  is the type rather than a discipline: an [`activity::Entry`] is built in one
  place, from six fields of a [`Request`], with `headers` and `body` in scope and
  unread — and the test asserts against the whole of what an entry can be made to
  say rather than against the fields it happens to have, because a field added
  later would pass a test that only checked the fields. The bound is **two**
  bounds, lines and bytes, since what a line costs is mostly a URL and a URL is
  as long as a page chooses — and a reason quoting what a server sent is cut at
  [`activity::LONGEST_REASON`], because a server that could write a thousand
  lines into a record is a server deciding how much memory this process uses.
  And an entry keeps the **cause** rather than a chain, walking against
  `Documents` on demand: a frozen chain in every line is the side table ADR 0012
  § 3 refuses by name, and one that disagreed with the browser process would
  still read like evidence.

  Both reachable clauses are met — `tests/what_the_record_says.rs` drives the
  real `Pool` over loopback for the first, and the fourth is kept by the shape:
  a renderer holds no `Pool`, nothing crossing the boundary carries a line (a
  match in `message.rs` that a fifth variant would break), and `alo-agent` does
  not depend on `alo-net` at all. The other two clauses are item 202's, because
  they are about a disk.

- [x] **202. What an agent did, kept until the person deletes it.** Cut from
  200, which keeps the session's record in memory and writes nothing down. ADR
  0012 § 6's second half: **only** what reaches an agent action, under ADR 0011
  § 3's rules unchanged, **never opened at all** for a session-scoped profile,
  and bounded in **actions rather than bytes** so that one action with three
  hundred requests in it cannot evict a week of ordinary ones.
  *Depends on 200. Closes when:* an agent's work survives a restart and a
  person's browsing does not, a private profile leaves no file behind at all
  (**never written**, rather than written and deleted — ADR 0011 § 2's rule),
  and a file that does not read is a record with a gap rather than an error.

  **One thing this cannot inherit and has to decide**: item 200's entry keeps a
  cause and walks the chain against `Documents`, which is bounded and dies with
  the process. A durable entry has neither, so it has to **freeze** the chain at
  the moment it is written — which is not the side table ADR 0012 § 3 refuses,
  because there is nothing left to disagree with it, and the reason is worth
  writing into the file rather than leaving for somebody to rediscover.

  **Done, all three closing clauses, over a real restart.**
  `crates/alo-net/src/kept.rs` is the directory, the policy and the bound;
  `deed.rs` is one action's file, which is the whole untrusted surface — the
  same division as `disk.rs` and `record.rs`, and it reads its bytes with the
  same reader, which is now `bytes.rs` rather than a second copy inside each.
  `Pool` holds an `Option<Kept>`, and [`None`] is what a session-scoped profile
  **is**.

  **The freezing needed one more decision than the item names**, and it is the
  one worth reading twice: a frozen link holds **numbers rather than
  identities**. ADR 0003's ids are minted once per browser *process*, so
  `action#0` exists in every session that had one — a `DocumentId` decoded off a
  disk that compared equal to one minted this morning would join two unrelated
  pieces of somebody's history into one story, which is the exact failure
  ADR 0003 exists to prevent. The same rule decides that **an action from an
  earlier session is never added to**: an action is matched to a file only
  within the session that minted it, and what names one across sessions is the
  number the disk counts up.

  **Where the write happens was the whole design, and it is a seam rather than
  a door.** Item 200 could put the session's record in
  `Pool::fetch_however_it_ends`, the one place every request passes. This cannot:
  deciding whether a request followed from an action needs the requests
  (`Activity`, held by the `Pool`, because a pool is what a session holds) **and**
  what caused each document's load (`Documents`, held by `alo_renderer::Tabs`,
  because ADR 0012 § 4 puts attribution where the tabs are) at the same instant
  — and a copy of either beside the other is precisely the side table § 3
  refuses. So the browser process brings them together, which is the one thing
  it is for: `Kept::take_from` walks `Documents` itself rather than trusting
  anything a caller says, so a durable line is exactly as unforgeable as a
  session one. It is idempotent by `Entry::sequence` — which is what that field
  was added for — and `Kept::missed` says how many lines went by uncounted, so
  a browser process that swept too rarely is a number rather than a silence.
  Reading brings it up to date, so there is no way to be handed a record
  somebody forgot to refresh.

  Two refusals are added to ADR 0012 § 5's list because a durable file is worse
  than memory in exactly two ways: a **`data:` URL keeps its kind and loses its
  content**, since a URL that *is* the content is a body wearing an address's
  clothes, and an address longer than `LONGEST_URL` is cut and says so. And the
  place is not the cache directory, which is the one clause of ADR 0011 § 3 that
  is deliberately not taken unchanged: a system may empty a cache because
  everything in one can be fetched again, and nothing here can.

  Two doctored runs rather than reasoning about them: with the chain frozen one
  link deep, four tests fail; with a person's browsing kept too, three do. The
  first doctoring found that the selection rule was being asked in two places,
  which is now one. The cut is item 203.

- [ ] **203. An action's own outcome, beside the requests it caused.** Cut from
  202. ADR 0012 § 6 says what is kept durably is *"only requests whose cause
  chain reaches an agent action, **plus the action and its outcome**"*, and item
  202 built the first half: a file per action, holding the requests. What it
  does not hold is what the verb itself did — activated what, refused why —
  because that is `alo_agent::Outcome` and `alo-net` does not depend on
  `alo-agent`. **That dependency is not an obstacle to route around**: it is
  what makes ADR 0012 § 7's *not the agent* structural, since a crate that
  cannot name the record cannot be handed one. So the outcome has to arrive from
  the browser process, which is the side that accepts a verb and is the
  authority § 4 names — and the path by which a verb reaches the loader does not
  exist yet, because `alo_renderer::Tabs` holds no `Pool`.
  *Depends on 202, and on a browser process that has both. Closes when:* an
  action's file says what the verb did as well as what it fetched, and the
  outcome it says is the one the browser process recorded rather than one a
  renderer or an agent stated. ADR 0016 § 6 adds the window's two ends — when
  the verb was sent and when the renderer answered it — to what that file
  keeps, so a renderer holding its answer open is visible in the record.

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

- [x] **69. An ADR for our own JavaScript engine.** *Needs ADR, and it is the
  first item here.* What it is: a bytecode compiler and an interpreter,
  **correct first**. What it is not: a JIT, until there is a measured reason and
  an ADR weighing the speed against the attack surface. Why it is ours at all:
  taking somebody else's C++ engine spends the memory-safety argument this
  project is built on (ADR 0001), and spending it quietly is worse than not
  having made it. *This item used to say "ADR 0006", which is the supervisor's
  number and was taken while this line sat unread; the decision is **ADR 0013**,
  renumbered rather than reused for the reason item 152 gives.*

  **Done: `docs/decisions/0013-our-own-javascript-engine.md`.** No code — a
  decision is its own iteration, and there is nothing yet for it to be a
  decision *about*.

  **The refusal the item did not name is the one that took the argument**: not
  V8, which ADR 0001 already refused, but **Boa** — safe Rust, permissively
  licensed, exists today, and MPL (ADR 0009) makes taking it legally trivial.
  It is refused because the collector, the bounds and the object graph are
  where this browser's own promises are kept: a rented engine's collector
  decides how `alo-dom` is stored, which is the one structure ADR 0003 already
  made a promise about, and a rented engine's limits are a stranger's idea of
  how much memory a script may make us allocate. The cost is written down
  rather than argued away — it exists and we are years from one.

  **Three clauses constrain items 70 to 79 so they are not each re-decided.**
  Bytecode from the first line of the compiler, because a suspendable frame is
  what generators, `async` and a debugger all need and a tree walker expresses
  it by being rewritten. **Absent beats approximate**: a builtin we have not
  written is *not defined*, since a stub is the one answer that defeats a
  page's own feature test and then behaves wrongly. And `alo-js` **depends on
  no I/O crate at all** — no network, no filesystem, no clock, no entropy —
  so every capability arrives from the embedder, the engine is testable with
  nothing moving, and the browser process never runs page script.

  Four things are refused and recorded rather than left open: a **JIT**, with
  the two conditions for re-opening it named (a measurement on hardware, and an
  ADR of its own weighing `unsafe` in the largest target in the browser);
  `unsafe` in the **value representation**, on the same terms, because
  NaN-boxing is the obvious first one; **`SharedArrayBuffer`**, which is shared
  mutable memory between threads and is on no list here; and **WebAssembly**,
  which this decision does not put on one.

- [x] **70. The lexer** — the language pages actually ship, not ES5. *Scope cut
  on starting: the parser and the syntax tree are item 204.* Originally: lexer
  and parser together, with automatic semicolon insertion and the grammar that
  needs a decision rather than a guess — a regular expression against division,
  an arrow function against a parenthesised expression.
  *Depends on 69. Closes when:* a frozen page's own script tokenises, and the
  first of those two ambiguities is answerable at the token level rather than
  guessed at.

  **Done: `alo-js`, and the cut is at the seam the language has.** A lexer turns
  characters into tokens and a parser turns tokens into a tree; the half taken
  here is the one a stranger's bytes reach first, and the one where being wrong
  is being wrong about *what a character is*. The arrow-against-parenthesis
  ambiguity went with the parser because it is decided by what follows a closing
  parenthesis — a question about a token stream, not about characters.

  **The interface is the item's own rule made structural.** `Lexer::next` takes
  a **`Goal` every call**, so `/` is division or a regular expression because
  the caller said which, and a `}` continues a template for the same reason. It
  is an argument rather than a mode that is set, because a mode is a thing a
  caller forgets to change — and there is no heuristic anywhere to fall back on,
  which is the point: every editor guesses from the previous token and every one
  of them is wrong on `return /re/` against `x++ /y/z`.

  Two rules fell out of the order rather than needing code. Trivia is skipped
  **before** the goal is consulted, which is why a pattern can never begin with
  `/` or `*` — the specification writes that as a lookahead restriction and here
  those two spellings were simply already taken by a comment. And `<!--` is not
  refused: Annex B is honoured by **not being implemented**, because `a <!--b`
  is ordinary modern code meaning `a < !(--b)` and refusing the characters would
  break a page over a decision about 1996.

  **The one bound is source length, and that is not an oversight** — a lexer has
  no nesting, so a million open brackets is a million tokens and no recursion.
  The depth bound belongs to item 204, which is the thing that recurses, and
  `bounds.rs` says so rather than leaving it to be noticed.

  Judged both ways, because neither is the other: a **frozen real script** —
  alo's own service worker, `crates/alo-corpus/scripts/alo-service-worker/`, a
  second kind of frozen thing beside the cases — where the gap between every
  pair of tokens is itself lexed and must come back empty, so a byte quietly
  skipped is a failure rather than a tidy token stream with a hole in it; and a
  table, which is where a grammar decision is settled in numbers. The hostile
  half cuts a nasty corpus at **every character boundary from both ends** and
  reads every code point up to U+FFFF alone: a list of cases finds what somebody
  thought of, and the cuts find what nobody did.

- [x] **204. The parser, to a syntax tree.** Cut from 70 on the iteration that
  built it. Automatic semicolon insertion — for which the lexer already records
  a line ending before every token, and settles nothing else — and the second
  ambiguity item 70 named: an arrow function against a parenthesised expression,
  which is decided by what follows the closing parenthesis and is therefore a
  question about a token stream rather than about characters.
  *Depends on 70. Closes when:* a frozen page's own script parses; a `}` that
  ends a template substitution is asked for with `Goal::TemplateContinuation`
  and one that closes a block is not; a reserved word written with a `\u` escape
  is refused rather than read as the keyword (the lexer records
  `Word::escaped` and does nothing with it); `with` is refused by name per
  ADR 0013 § 3; and nesting has a **depth bound** — the lexer has none because
  it does not recurse, and this is the item that does.

  *The frozen script the lexer closed on has no regular expression in it, which
  is why one goal read all of it. A parser needs a case that exercises the
  choice, so **freezing a second script is part of this item** rather than a
  thing to notice half way through.*

  **Done: `alo-js`'s `ast` and `parser`, and every clause of the closing
  condition.** Both frozen scripts parse, the second one frozen here
  (`alo-theme-generator`) precisely because the first has no `/` in it: it
  holds six regular expressions and **no division at all**, which a test
  asserts by walking the whole tree — a pattern read as arithmetic would be a
  different program that parses perfectly well, and every other assertion would
  still have passed.

  **The arrow ambiguity is settled where the item said it would be: on the
  token stream.** A parameter list is *tried* and the cursor put back when what
  follows is not `=>`. Trying costs a second read of what is inside the
  parentheses, and a `(` inside a `(` would pay it again at every level — which
  is quadratic on a page that chooses how deeply it nests — so a `(` that was
  not a parameter list is remembered by its offset and never tried twice. The
  same shape settles four other words the language leaves contextual: `let`,
  `async`, `static`, and any name before a `:`.

  **The depth bound needed a stack before it could mean anything.** A bound of
  512 refused nothing on a `cargo test` thread, because two mebibytes is under
  fifty bracket levels in a debug build and the process aborted before the
  counter ever reached its ceiling — an abort is not a refusal, and ADR 0013 § 4
  forbids it outright. So the parse **runs on a thread of its own**
  (`bounds::STACK_FOR_A_PARSE`, thirty-two mebibytes, measured), which is the
  same argument every bound in `alo-net` makes: *a limit somebody else chooses
  is not a limit*. `DEEPEST_NESTING` is 256 and now means the same thing in a
  debug build, a release build and a renderer.

  **Two refusals are the ones worth reading twice**, because both are places a
  parser can be quietly wrong rather than loudly: `a ?? b || c` is refused
  rather than given a precedence, and the tree cannot tell it from
  `(a || b) ?? c` afterwards — so it is caught while parsing, by the function
  that knows whether a `||` was written at that level. And `{ a = 1 }` is a
  pattern rather than an object literal, decided by an `=` that comes *after*
  the whole of it, so its refusal is **kept rather than raised** and dropped
  the moment the thing holding it becomes a pattern. That is the whole of the
  cover grammar this parser needs.

  **It found one defect in item 70's lexer**, in the place the design was most
  confident: `Goal::TemplateContinuation` skipped no trivia, on the reasoning
  that everything after the `}` is the template's own text. True, and about the
  wrong side of the brace — the space in `` `${ a }` `` belongs to the
  substitution that has just ended, which is why the specification's
  `InputElementTemplateTail` lists whitespace and comments. Ordinary code would
  not have parsed. Cut: item 205.

- [x] **205. The early errors for plain function headers.** Cut from 204,
  whose parser reads parameters before the body's directive prologue. **Scope
  cut in iteration 117:** the callable plain-name forms are finished here;
  remaining scope-sensitive errors and import attributes are item 222.
  Implements ADR 0013 §§ 3, 4 and 9 and `docs/features.md`'s parser/compiler
  promise. *Depends on 204 and 209. Closes when:* strict function parameters
  and binding names are checked under the body's final strictness, duplicate
  strict parameters are early errors, and valid shadowing remains valid. An
  invalid header must prevent every statement from running, including when the
  function would never be called; malformed/truncated headers must not panic.

  **Done.** The compiler checks plain names before assigning bindings, reusing
  the keyword table rather than creating another scope table. `eval`,
  `arguments`, strict reserved words and decoded escapes cannot bypass it.
  Duplicate strict and arrow parameters are early errors rather than reports
  of an unsupported parameter form. A method's property name remains legal.
  The parser now distinguishes `('use strict')` from a directive; a grouped
  string also ends the directive prologue. Eight integration tests cover
  refusal before side effects, engine reuse, collection stress, hostile input,
  and existing parameter/body collision and label-boundary checks. Legal
  shadowing across a function boundary is explicitly tested, not refused.

- [ ] **222. The remaining scope-sensitive early errors, and import
  attributes.** Cut from item 205 without claiming those mechanisms exist.
  The original scope also named private identifiers no class declares and
  import attributes delivered to the loader. Parameter defaults, destructuring,
  rest, async/generator headers, and uniqueness for non-strict methods need
  their own validation; plain sloppy duplicate parameters remain explicitly
  unsupported. A named function expression's own binding and a parameter of
  the same name also need separate scopes rather than a false duplicate error.
  *Depends on 205; parameter execution needs 213, async/generator execution
  needs 75, private-name validation needs 212, and import attributes need 77.*
  Cut a bounded subitem when one of these becomes reachable. *Closes when:*
  supported parameter forms are checked under the body's final strictness,
  including the ban on a local strict directive with a non-simple list;
  legal named-expression shadowing runs; a private name absent from its class
  is refused; and `import a from "b" with { type: "json" }` delivers the
  attribute to the loader rather than parsing and dropping it. Each must have
  a named refusal or value test. This does not lift ADR 0013's stage 3 gate
  for sloppy-mode aliasing.
  ***Blocked: no frozen real script demonstrates the binding defect.*** Written
  here, in the form the selection rule reads, so that the next iteration passes
  over this item rather than re-deriving iteration 119's halt. Unblocked by a
  frozen source, not by an opinion.

  **Iteration 119 halted before implementation.** Named-expression shadowing
  is reachable with the existing function and environment machinery, but no
  frozen real-script failure for that scope was established. The two scripts
  currently in the corpus have ordinary function declarations, not named
  function expressions. Their lexer/parser regressions do not close this
  execution defect. Before taking this bounded piece, freeze a permitted real
  source with provenance and demonstrate the binding failure, as LOOP's stage 2
  clause 1 and ADR 0013 § 9 require. Retain the other dependency gates above;
  this item is not done.

- [x] **71. The object model, and a garbage collector.** Objects, properties,
  prototypes, and something that reclaims them.
  *Depends on 69. **ADR 0014 is written and accepted** — a collector is a
  decision about pauses, and it turned out to be four decisions that cannot be
  changed afterwards. The code is what remains, and the ADR names the rules it
  must carry: the heap is an **arena of slots** and a reference is an **index
  with a generation**, so ADR 0003's promise survives slot reuse and a stale
  reference names nothing rather than naming whatever took the slot; the
  collector is **precise**, which makes the places a live reference may live a
  **closed list** — realm globals, the interpreter's frames and value stack, the
  scopes native code holds, the embedder's roots, and a job's keep-alive set —
  and nothing else may hold one across an allocation; the **DOM is in the same
  graph**, traced rather than counted, with one wrapper per node and the trait in
  `alo-js` so neither crate depends on the other; **non-moving mark and sweep**,
  stop-the-world, with a **write barrier from the first line** that does nothing
  today because incremental marking and a nursery both need it and neither can be
  retrofitted; the **marker never recurses** and a collection **allocates
  nothing**, since the moment we most need to collect is the moment there is none
  to spare; **ephemerons marked to a fixpoint**, because a `WeakMap` written
  afterwards is written wrong; a finaliser frees nothing of ours, `Drop` at the
  sweep does; and the heap's ceiling is ours, in `bounds.rs` with its reason,
  with a full heap an error the script or the embedder is told about and never an
  abort. Closes when:* an object graph with a cycle in it — including one through
  a DOM node — is reclaimed, counted rather than watched; a stress mode that
  collects at **every** safepoint passes, because that is the only thing that
  finds a rooting bug; the heap invariants hold after every collection in a test
  that checks them; and the hostile half is a refusal or a collection rather than
  a crash.

  **Done, for the collector; the object model is cut to item 206.** Scope
  rather than depth, and the cut is the one the ADR's own shape suggests: § § 1
  to 10 are the heap and § 11 is what a cell *is*. `Heap<T>` is generic in its
  cell, so the object model lands inside it without changing a line of
  `heap.rs` — and building it the other way round would have meant storing
  objects somewhere else first and moving them.

  All four closing conditions are met. A cycle is reclaimed and **counted**,
  including one through an embedder's object — a node, a listener, a closure
  back to the node — which is § 6's clause in the only form available before
  the bindings crate (item 80) exists. The stress mode collects at every
  safepoint, which today means at every allocation, and it has both halves
  asserted: a reference in a Rust local does not survive one and a rooted one
  does. The invariants are `Heap::check`, run after every collection in every
  test. And the hostile half is `tests/a_heap_that_is_hostile.rs`.

  **Three things are worth reading twice.** The bounded pair buffer nearly
  became a correctness bug: a `WeakMap` with more live entries than the buffer
  holds would have had entries silently dropped, so the mark phase settles a
  pair **where it is reported** when the key is already marked, and a
  collection that ever refused a pair does not end until a pass over every
  marked cell finds nothing new. A **retired** slot needed the retired
  generation to be reserved rather than reached, or the last reference handed
  out before retirement would have gone on matching for ever. And the cell
  *being allocated* is traced as a root for the collection its own allocation
  caused — otherwise the discipline would have included "do not build an
  object", which is not a discipline.

  Two things the ADR names are recorded as owed rather than done, each against
  the item that owns it: `WeakRef` and `FinalizationRegistry` **callbacks** are
  item 76's, since they run as tasks on an event loop that does not exist —
  what the heap owes them, clearing and reporting the loss, is here — and
  test262 for the weak collections needs a script to run, which is item 72.

- [x] **206. The object model.** Cut from 71, which built the heap and the
  collector and is generic in what a cell is. ADR 0014 § 11 is the whole
  specification and it is mostly decisions already made: an ordinary object is a
  prototype reference or null, a property table and an extensibility flag;
  **property order is observable, so it is the specification's order from the
  first line** — integer-like keys ascending, then strings in insertion order,
  then symbols; internal methods are the *same* trait ADR 0013 § 6 promised the
  embedder, so arrays, functions, proxies and the DOM's oddities are one
  mechanism rather than two; one interface for get, set, define, delete and own
  keys, because the representation behind it is what an engine changes when it
  gets fast; **property keys are interned and the intern table is weak**, since
  a strong one is a leak a stranger's script controls; and a string is a heap
  object, immutable once made, in UTF-16 code units.
  *Depends on 71. Closes when:* the property order a page can enumerate is the
  specification's in a test that mints keys of all three kinds out of order; a
  prototype chain answers a lookup and a cycle in one is refused rather than
  looped; and the hostile half is the one item 71 could not have — **an
  unbounded number of distinct property keys**, which is a refusal or a
  collection rather than a heap that grows for ever.

  **Done, all three closing conditions, and it landed inside the heap without
  one line of `heap.rs` changing** — which was the whole argument for building
  the collector first. Eleven files under `alo-js/src/object/`, one reason to
  change each: what a value is and how a cell holds one, what names a property,
  what a property is, the table that keeps the order, the string, the symbol,
  the internal methods, the ordinary object, the cell enumeration, the intern
  table, and the one interface for access.

  **The hostile clause is answered by a collection rather than by a refusal, and
  the test says which.** Two hundred thousand distinct names are minted, the
  collector fires **on its own** during the loop — `collections()` is asserted
  to be past zero, because a test that asked for the collection would be
  asserting that it can call a method — the arena ends with fewer slots than
  half the names, and the intern table ends empty. The refusal half of the
  disjunction is the heap's ceiling, which is item 71's and is unchanged.

  **Three things are worth reading twice.** The intern table holds **no copy of
  the text**: a `HashMap<Box<[u16]>, Ref>` would be the obvious table and would
  put a second copy of every property name *outside* the heap's ceiling, which
  is the same leak in a place nothing counts. So it is a seeded hash to the
  cells that hashed to it, and a lookup compares by reading the string cell it
  already has — the seeding being a security property rather than a detail,
  since a page chooses every name it writes. **The keys are edges**: a property
  named by a string keeps that string alive, which is what makes the weak intern
  table safe rather than merely small. And the **prototype walk is bounded by
  the number of slots in the heap**, which is exact rather than chosen: nothing
  a page writes can make a cycle, because `set_prototype` refuses one, but an
  embedder answers `[[GetPrototypeOf]]` for itself and a renderer that hung on a
  lying object would be a denial of service in the process that parses hostile
  bytes.

  **One thing became a decision rather than a chore and went to the queue as
  item 207**: there is no `BigInt` value, because arbitrary-precision arithmetic
  is a question about renting rather than a variant to add. Two smaller cuts are
  recorded where they belong rather than as items of their own: a **partial**
  property descriptor is `Object.defineProperty`'s reading of an argument object
  and is item 73's, and a **proxy** intercepts the walk itself rather than the
  own-property questions, which needs something that can call a trap — item 72.
  Both are written into the files that would otherwise look incomplete.

- [ ] **207. A `BigInt` that is a number rather than digits.** Cut from 206,
  which has no such value: `Value` is an enum of the language's primitives and a
  `BigInt` is not one of them, it is arbitrary-precision arithmetic. Item 70's
  lexer already keeps a `BigInt` literal's **digits as text**
  ([`token::Kind::BigInt`]) for exactly this reason, and a variant holding an
  `f64` would be a wrong answer that reads like a right one — `9007199254740993n`
  is a number a double cannot hold and the type exists to hold it.
  **Needs ADR** — arbitrary precision is physics in ADR 0001's sense (nobody
  differs by it, and the hard parts are division and the base conversions), so
  the question is whether it is rented like the Unicode tables and the
  double-to-string conversion ADR 0013 § 8 already rents, or written like the
  collector. Renting it puts a stranger's allocator in the path of numbers a
  page chooses the size of, which is the clause every bound in this engine is
  written against; writing it is a well-specified week nobody enjoys.
  *Depends on 206. Closes when:* the decision is written down, and then a
  `BigInt` round-trips through a value, compares, and refuses a size the page
  chose rather than allocating it.

  **The decision is written down: ADR 0015, accepted.** The limb arithmetic is
  rented from `num-bigint` behind one file (`crates/alo-js/src/bigint.rs`, added
  to the gate's boundary list with the dependency); every spelling, comparison
  and Number conversion is ours; and no rented function is called until our
  code has computed the result's largest possible size from its operands and
  refused it past a ceiling in `bounds.rs` — measured against the slowest
  single operation, because one rented call is not an interrupt point. The
  crate's three panics (division by zero, an out-of-range shift, an exponent
  that overflows memory) are preconditions checked before the call, each with
  a hostile test. **The code half is not built and this item is not done.**
  ***Blocked: no frozen real script uses a `BigInt`.*** LOOP's stage 2 clause 1
  opens the implementation with a page that fails on one; neither corpus script
  contains a `BigInt` literal or call. Unblocked by a frozen source with
  provenance, not by an opinion — the same form as item 222.

- [x] **208. The parser bounds the tree it builds, not only the brackets it
  counts.** Found by starting item 72 and **taken before it**, because item 72's
  compiler recurses over the same tree and cannot be built whole over a parser
  that aborts. It is a live denial of service rather than tidiness: nine short
  scripts end the renderer today, and ADR 0013 § 4 says in one sentence that
  this must not happen — *it never panics, not on any source text, not on any
  program*.
  *Depends on 204. Closes when:* every shape that deepens a tree is a refusal
  rather than an abort, each named for the bound that refused it, and the shapes
  that are **wide** rather than deep still parse.

  **Done, and it was two defects with one name.** Item 204 counted how deep the
  parser *recurses*, which is the right bound for brackets and the wrong
  question for everything else. Five shapes recursed where nothing counted —
  `!!!…a`, `- - - …a`, `typeof typeof …a`, `new new …a`, `a**a**a…` — and
  overflowed the parse thread's own thirty-two mebibytes. Four more are read in
  a **loop**, so they cost the parser no stack at all and build a tree as deep
  as the file is long: `a.b.b.b…`, `a()()…`, `a?.b?.b…`, `a+a+a…`, and a run of
  tagged templates with them. Those parsed *fine* and killed the process when
  the program was dropped — `Drop` walks the tree one frame per level, before
  any compiler gets near it.

  **Two bounds, because they are two questions**, and the second one is new:
  [`bounds::DEEPEST_NESTING`] is how deep the parser recurses (256, measured
  against `STACK_FOR_A_PARSE`, a bracket costing thirteen frames) and
  [`bounds::DEEPEST_EXPRESSION`] is how deep a tree it builds (4096, a level
  costing one frame in every walker, and a `cargo test` thread drops sixteen
  thousand levels without trouble). `Reason::ExpressionTooDeep` is a refusal of
  its own so a test asserts which bound answered.

  **The rule worth reading twice is what the second counter does *not* do.** It
  counts the **path** rather than the loop, and it is put back only around
  **siblings** — the right side of an operator, an argument, an array element, a
  property's value, a branch of a `?:`, a statement. A count that were put back
  when each loop ended would be defeated by nesting: two hundred levels, each a
  thousand links, none of which reaches the ceiling on its own and which
  together are two hundred thousand deep. That case is a test. So is the
  opposite one, which is the failure that would have been much harder to
  notice: an array of fifty thousand elements, an object of twenty thousand
  properties, a call with twenty thousand arguments and a file of twenty
  thousand statements all still parse, because a bound that added siblings up
  would refuse every bundle on the web.

- [x] **72. A bytecode compiler and an interpreter.** Values, scopes, calls,
  `this`, closures, exceptions.
  *Depends on 70, 71, 206, 208. Closes when:* a suite of small programs produces
  the values the specification says, run as a table rather than as prose.
  **Item 208 was cut out of this one on the iteration that started it**: a
  compiler walks the tree the parser built, one frame per level, so a tree
  nothing bounded is a compiler that cannot be written to ADR 0013 § 4 at all.
  It is done, and what it leaves here is a fact this item may rely on — the tree
  is at most `DEEPEST_NESTING + DEEPEST_EXPRESSION` deep — and a question it
  must answer for itself: **the compiler's own stack**, which is the parser's
  argument again (*a limit somebody else chooses is not a limit*) and which
  4096 levels of a compiler's frames will not fit in a caller's two mebibytes.
  **Item 206 left two things here by name**, each because it needs something
  that can call: an accessor property answers with its **getter** rather than a
  value ([`object::Found::Getter`]) and a setter likewise, so an interpreter
  that has not learned to call one will not compile against this interface; and
  a **proxy** overrides `[[Get]]` rather than `[[GetOwnProperty]]`, which is a
  method added to `object::Internal` when there is a trap to call.

  **Done, for the machine, and cut on starting into three items.** The cut is
  the one the last iteration wrote down: take the machine and *the language
  that needs no call* — values, scopes with their dead zone, the operators,
  objects, control flow — and leave calls (209), `try`/`catch`/`finally` (210)
  and the forms that take a value apart (211) as items of their own. Scope
  rather than depth: what is here is whole, and what is not is a **refusal that
  names its item** rather than something plausible (ADR 0013 § 3).

  `code.rs` is the instruction set, `compile.rs` (with `scope.rs` and
  `hoist.rs`) turns a tree into one, `interpret.rs` runs it, `realm.rs` is the
  global object and the global `let` bindings, `convert.rs` is the abstract
  operations and `operate.rs` the operators written in terms of them,
  `numeric.rs` is ADR 0013 § 8's rented arithmetic in the specification's own
  spelling, and `abrupt.rs` is the five ways a run ends that are not a value —
  kept apart because **they are answered by different people**: a `TypeError` is
  the page's, a full heap is the embedder's, a lost reference is ours.

  **Three things in it are decisions rather than detail.** The **value stack is
  a heap cell** (ADR 0014 § 2's last owed clause, `object/slots.rs`), so every
  instruction reads its operands *where they lie* and drops them only once the
  answer exists — and the whole table runs twice, the second time collecting at
  every allocation, which is the only thing that finds a rooting bug. The
  **interpreter never recurses**, so a page cannot choose how much stack this
  process uses by nesting; the **compiler** does, so it runs on a stack of its
  own like the parser, and the number is measured — four thousand additions
  overflow eight mebibytes in a debug build. And **stopping is the embedder's**,
  on a switch checked at every backward jump, because ADR 0013 § 5 gives this
  crate no clock.

  **Two things landed here that were somebody else's on paper.** Two of item
  205's early errors are in the compiler, because it cannot be correct without
  them — a name declared twice in one block would take a second slot or put a
  live binding back in its dead zone, and a `break` naming no label has no
  instruction to be. The rest of 205 is untouched. And the global object has the
  **three value properties** (`undefined`, `NaN`, `Infinity`) and `globalThis`,
  with the specification's attributes, because they are the only way to *write*
  three of the language's own values; the rest of the builtins are item 73's and
  are absent rather than stubbed.

- [x] **209. Calls, `this` and closures.** Cut from 72 on the iteration that
  built it, and it is the largest of the three: a function object with a
  `[[Call]]` and a `[[Construct]]`, a frame per call with its own slots, the
  argument list, `arguments`, `this` and how an arrow does not have one,
  closures over a scope that outlives its frame, `new`, classes and their
  private members, tagged templates, and `super`. Two things item 206 named are
  discharged here: an accessor property answers with its **getter**
  ([`object::Found::Getter`]) rather than a value, so reading one is a call, and
  a **proxy** overrides `[[Get]]` rather than `[[GetOwnProperty]]`, which is a
  method on `object::Internal` once there is a trap to call. Item 72's
  interpreter already refuses each by name, and [`bounds::VALUES_ON_THE_STACK`]
  is already the bound that turns runaway recursion into the `RangeError` the
  language specifies.
  *Depends on 72. Closes when:* a table of programs that call things produces
  the values the specification says, a closure keeps its scope alive after the
  frame it was made in has gone — counted rather than watched, which is item
  71's rule — and unbounded recursion is a `RangeError` rather than a process
  that stops.

  **Done, all three closing conditions, and cut on starting into five items.**
  The item's own words said it was the largest of the three, and what it named
  is five separable pieces of work rather than one: what is here is *calling*,
  whole — a function object with a `[[Call]]`, a frame per call, the argument
  list, `return`, `this` and how an arrow does not have one, closures, and the
  bound that turns runaway recursion into a `RangeError`. Constructing (212),
  `arguments` and the parameter forms that are not a plain name (213), a getter
  or setter as a call (214), tagged templates (215) and per-block environments
  (216) are items of their own, each **refused by name** where a program
  reaches it.

  **The decision worth reading twice is where a name lives, because it is two
  places rather than one.** A function's parameters, its `var`s, its body-level
  `let` and `const` and the functions it declares are **bindings of an
  environment**, which is a cell in the heap that a closure keeps alive after
  the call has returned. A **block's** `let` and the compiler's own temporaries
  stay frame slots in the value stack, which die with the call. Two mechanisms
  because only the first can be captured — and a function reading a block's
  binding is therefore **refused** (item 216) rather than compiled into
  something that shares one slot between two passes of a loop. That refusal is
  what keeps item 72's note true: *the language copies a `let` head into every
  iteration, and nothing can tell.* A closure is the only thing that could
  tell, and it is the case that is refused.

  Three more rules, each because the obvious implementation is wrong in a way
  a test written afterwards would not catch. **`this` is decided by the
  callee's strictness, not the caller's**: the compiler pushes `undefined` for
  a plain call and the receiver for a method call, and the call then applies
  `OrdinaryCallBindThis` — so a sloppy function called plainly gets the global
  object and a strict one gets `undefined`, and the caller never has to know
  which it is holding. An **arrow captures its `this` where it was written**
  rather than walking a chain for it, and it captures it whether the body says
  `this` or not, because an arrow nested inside it may say it after the frame
  has gone. And a **named function expression can see itself**, before anything
  has assigned it anywhere, so that binding is filled in by the call rather
  than by an instruction — and assigning to it is silence in sloppy code and a
  `TypeError` in strict code, which is a third answer to *what an assignment
  does* rather than a shade of the `const` one.

  **The chunk stopped being the unit of compilation.** A function is a chunk of
  its own, so a program is a [`unit::Unit`] — one pool of strings and every
  chunk in it — and a run interns that pool once. A function made by one script
  and called by the next brings its own unit with it, which is why a run holds a
  small list of loaded programs rather than one, and why the test that closes
  this item runs **two scripts in one engine**.

  **The bound is two bounds.** [`bounds::VALUES_ON_THE_STACK`] does bound a
  runaway recursion, as this item said it would — but a call costs a frame, an
  environment cell and a root as well as its two values, so a bound counting
  only values under-counts what it is bounding by an order of magnitude.
  [`bounds::CALLS_ON_THE_STACK`] is the second, ten thousand, with the reason
  written beside it.

- [x] **212. `new`, classes, `super` and private members.** Cut from 209, which
  builds `[[Call]]` and no `[[Construct]]`: a function here has no `prototype`
  property, `new` is refused by name, and a class is refused whole. The order
  inside it is the specification's own dependency: `[[Construct]]` first
  (which needs `Function.prototype` and therefore item 73 for the object a
  constructor's `prototype` is), then a class as sugar over it, then `super`,
  which needs `[[HomeObject]]` on a method and is the reason a method is not
  simply a function in a property. Private members are last and are their own
  mechanism — a name that is not a property key at all.
  *Depends on 209, and on 73 for `Function.prototype`. Closes when:* `new f()`
  makes an object whose prototype is `f.prototype`, a class with a constructor
  and a method produces the values the specification says in the same table
  item 72 uses, `super.m()` finds the method on the home object rather than on
  `this`, and a `#name` is unreachable from outside the class in a test that
  tries.

  **Done at the scope of its first step, `[[Construct]]` and `new`; classes,
  `super`, `new.target` and private names are cut to item 223, and
  `instanceof` to item 224.** The cut follows the item's own order — the
  specification's dependency is `[[Construct]]` first — and it was taken
  because a **frozen real script** asked for exactly that step: alo's own
  service worker (`crates/alo-corpus/scripts/alo-service-worker/`) did not
  compile, refused at byte 1438, `new Request(OFFLINE_URL, …)`. It now compiles
  past that `new` and stops at byte 2847's `[]`, which is item 211's. Its other
  `new` — `new Response(…)` at byte 4686 — lies beyond that array, so this
  script does not yet show it compiling; the table does.
  The dependency on 73 named *`Function.prototype`*, which item 218 built.
  Closing evidence: `crates/alo-js/tests/what_new_makes.rs`, every case run
  ordinarily and with the collector at every allocation except the two
  runaway recursions, which run ordinarily as `an_engine_that_is_hostile.rs`'s
  do — ten thousand frames each collecting the whole heap is quadratic, and a
  fifty-deep nesting covers the same rooting under stress.

  **Three things are decisions.** *What may be constructed is a fact about how
  the function was written*, so it is a flag on the chunk
  (`Chunk::constructs`): a plain `function`, declared or as an expression,
  constructs; an arrow, a method, a getter, a setter and every builtin do not,
  and say so with `TypeError: … is not a constructor` **after** the arguments
  are evaluated, which a page can observe. *A construction is a call with a
  different landing*: `Op::Construct` finds the stack in a call's shape with a
  placeholder `this`, writes the instance there, and enters the body through
  the same `enter_at` a call uses, so the frame, both bounds and the
  `RangeError` are shared; `After::Construct` then answers with the body's
  object if it returned one and with the instance from the `this` slot
  otherwise, which is safe because nothing can assign to `this`. And
  *`MakeConstructor` runs when the function is made*, not lazily: the
  `prototype` (writable, not enumerable, not configurable) and its
  `constructor` (writable, not enumerable, configurable) are ordinary
  properties, and because `prototype` may not be reconfigured an accessor or a
  missing one there is `Internal::ConstructorIsWrong` rather than a guess.

  **Two doctored runs**: without the scope that holds the new `prototype`
  object while its keys are interned, eight tests fail under stress; without
  the instance substitution, five fail. `instanceof`'s refusal named this item
  for a `prototype` that now exists, so it is re-pointed rather than left lying.

- [ ] **223. Classes, `super`, `new.target` and private members.** Cut from 212
  on the iteration that built `[[Construct]]`. The rest of that item, in its
  own order: a class as sugar over a constructor — whose `prototype` is **not**
  writable, which is one difference from a function's — then `super`, which
  needs `[[HomeObject]]` on a method, then a derived constructor, whose `this`
  is in its dead zone until `super()` returns and whose instance is made by
  the parent rather than by `Op::Construct`. `new.target` is here because it is
  only distinguishable from the callee once a derived class exists, and an
  arrow inherits it as it inherits `this` — a second captured value on the
  function. Private names are last and are their own mechanism, a name that is
  not a property key at all. Each is refused today as `What::AClass`.
  *Depends on 212. Closes when:* a class with a constructor and a method produces the values the
  specification says in the same table item 72 uses, calling a class without
  `new` is a `TypeError`, `super.m()` finds the method on the home object
  rather than on `this`, `new.target` is the constructor `new` named and
  `undefined` in a call, and a `#name` is unreachable from outside the class in
  a test that tries. A frozen real script that uses a class opens it.

- [ ] **224. `instanceof`, whole.** Cut from 212 on the iteration that built
  `[[Construct]]`. Item 214 answered the two questions that come first — a
  right-hand side that is not an object or not callable is a `TypeError`, and a
  primitive on the left is `false` — and refused the rest naming 212, because a
  function had no `prototype`. One has now. What remains is the operator as
  specified: `GetMethod(C, @@hasInstance)` **before** anything else, which needs
  the well-known symbols (item 73) and `Function.prototype[@@hasInstance]`;
  then `OrdinaryHasInstance` — `Get(C, "prototype")`, which on a builtin may be
  a getter and therefore a call (the re-entry item 214 built), a `TypeError`
  when it is not an object, and the walk up the left-hand side's chain, bounded
  as `Objects::reaches` is.
  *Depends on 212, 214 and on 73 for the well-known symbols. Closes when:*
  `new F() instanceof F` is `true` and `({}) instanceof F` is `false`, a
  `prototype` that is not an object is a `TypeError`, a `Symbol.hasInstance`
  method a page defines is called with the left-hand side, and a getter on a
  builtin's `prototype` runs once.

- [x] **225. An array: the exotic object, `Array.prototype`, and the literal
  that makes one.** Cut on the iteration that built it from item 73 (*the
  `Array` exotic object — the exotic part is `length`*) and from item 211 (*an
  array literal*, without a spread, which reads an iterable and stays with
  211). Opened by a frozen real script: after item 212 the service worker
  (`crates/alo-corpus/scripts/alo-service-worker/script.js`) was refused at
  byte 2847, `let changedTypes = [];`, and item 211's own dependency list says
  the thing that literal needs first is item 73's exotic object.
  *Depends on 72, 206 and 218. Closes when:* `[1, , 3]` has a length of three
  and no property at one, an index at or past the length grows it, a smaller
  length deletes from the end and stops at an element that will not go, a
  length that is not one is a `RangeError`, `length` has the specification's
  attributes, `Array.prototype` is itself an array and
  `Object.prototype.toString` says so, and the frozen script compiles past
  byte 2847.

  **Done, all of them: `crates/alo-js/tests/what_an_array_is.rs`**, seventeen
  tests, every program run ordinarily and with the collector at every
  allocation and required to agree. The frozen script now compiles to byte
  2853, the `try` on the next line, which is item 210's.

  **The decision worth reading twice is that every write is a definition.**
  `Internal` lost its mutable borrow of a property: `a.b = c` into a property
  `a` already has is now `OrdinarySet`'s own `[[DefineOwnProperty]]` with the
  attributes it had, so an exotic object sees every store rather than only the
  first one. Without that `a.length = 0` would have written a number into the
  property and deleted nothing — and an embedder's object would have had the
  same hole. Ordinary definitions now tell the write barrier about the value
  they replace and the one they store, since a definition is now how an
  ordinary store happens.

  **`length` is held beside the table, not in it**, so the one key the rules
  are about cannot be stored somewhere they would be walked round; it still
  answers as an own property and comes after the indices and before every other
  name. A shrinking length visits only the indices that **exist** — four
  billion to zero over two elements is two deletions. Converting a value is the
  interpreter's (it can throw), so the object model accepts only an exact
  length and refuses anything else rather than guessing; a `length` that is not
  writable is refused before anything converts, which is `OrdinarySet`'s order.
  The literal makes the array at its final length (`Op::Array(n)`), which is
  the same array the specification's trailing `Set(length)` makes, because no
  script can see it before the literal ends.

  **What was cut**: an **object** assigned to `length` to **item 226**, and
  everything else about arrays stays where it was — the `Array` constructor,
  `Array.isArray` and every method are item 73 (`[].push` is `undefined`,
  which a page's feature test reads correctly), and a spread is item 211.
  **Five doctored runs**: without growth three tests fail, without truncation
  two, with a store that goes round `define_own` five, without tracing the
  length's name two. A fifth — dropping a scope that held the name between
  interning it and making the array — failed nothing, and the reason is real
  rather than luck: a collection an allocation runs traces the cell being
  allocated, and the array traces the name. The scope was removed and that
  reason written beside `Objects::array` instead.

- [ ] **226. An object assigned to an array's `length`.** Cut from 225.
  `ArraySetLength` converts the value with `ToUint32` and then with `ToNumber`,
  each of which calls an object's `valueOf` — **twice**, and a page can count
  the calls — and the assignment still evaluates to the object rather than to
  the number it became. Every other conversion in the interpreter converts an
  operand in place and runs the instruction again, which here would call
  `valueOf` once and overwrite what the assignment answers. So it is refused by
  name today ([`Missing::AnObjectAsALength`]) rather than half right.
  *Depends on 225. Closes when:* `a.length = { valueOf() { n++; return 2; } }`
  leaves `n` two greater and `a.length` two, the assignment answers the object,
  a `valueOf` that answers differently the second time is the `RangeError` the
  specification gives, and a `valueOf` that throws leaves the array unchanged.
  Opened by a frozen real script that does it, and not before.

- [x] **227. `Error` and the six native errors: what a `catch` binds.** Cut
  from item 73 on the iteration that took it, by the real failure rather than
  by taste: the frozen service worker is refused at byte 2853, the `try` of its
  push handler, and item 210 — the `try` — waits on 73 for exactly these.
  *Depends on 218, 219 and 212. Closes when:* `Error`, `EvalError`,
  `RangeError`, `ReferenceError`, `SyntaxError`, `TypeError` and `URIError`
  are constructors on the global object (writable, configurable, not
  enumerable); each prototype has `constructor`, `name` and an empty `message`,
  the six inherit from `Error.prototype` and their constructors from `Error`;
  a constructor called without `new` makes the same object as with it; a
  `message` is converted with `ToString` — an object's own `toString` run —
  and is own and not enumerable, and `undefined` gives none; `options.cause`
  is installed when `HasProperty` says so, a getter run once and after the
  message; `Error.prototype.toString` answers the specification's string;
  `Object.prototype.toString` says `[object Error]` for an instance and not
  for a prototype; a builtin not made as a constructor is still not one; and a
  constructor or conversion that recurses for ever is a `RangeError`.

  **Done, all of them: `crates/alo-js/tests/what_an_error_is.rs`**, eighteen
  tests, every program run ordinarily and with the collector at every
  allocation, plus `builtin/error.rs`'s test that builds the seven with the
  collector at every allocation. The frozen script's refusal does not move —
  it is item 210's `try` — and that is the honest scope: this item is what 210
  needed, not what the script needed directly.

  **The decision worth reading twice is that a builtin constructor is given its
  instance.** A native keeps a step number and nothing else across the script
  it asks to run (item 219), and an error constructor may ask twice — a
  `toString` on its message, a getter for its `cause`. So the interpreter does
  `OrdinaryCreateFromConstructor` from the constructor's own fixed `prototype`
  and writes the instance into the `this` slot, which the collector walks, in
  the one place a call and a `new` both pass through (`Engine::make_instance`).
  `Native::constructor` says which kind of instance; `Native::new` still makes
  a builtin with no `[[Construct]]`. A different `NewTarget` needs
  `Reflect.construct` or a derived class (items 73, 223) and cannot occur yet.
  An instance is `Cell::Error`: an ordinary object whose kind is the
  `[[ErrorData]]` slot.

  **What was cut**: `Error.prototype.toString` of a `message` that is a getter
  or an object, to **item 228**; `AggregateError` to **item 229**. Turning an
  error this engine throws into an instance is item 210's `catch`, which now
  has `Intrinsics::error_constructor` and `Family::from(Kind)` to do it with.
  **Five doctored runs**: without the hold on a new prototype the stress build
  fails; without the instance on a call fourteen tests fail; an enumerable
  message fails two; no `[object Error]` tag fails two. The fifth — dropping
  the hold on each `name`/`message` string — failed nothing, and the reason is
  real: nothing allocates between making the string and the property that owns
  it. The hold was removed and that reason written beside it.

- [ ] **228. `Error.prototype.toString` of a `message` behind a call.** Cut
  from 227. When `message` is a getter, or an object whose own `toString` must
  run, `name` has already been read and converted, and a native has nowhere
  traced to keep it across the call; reading `name` again afterwards is a
  second getter call a page can count. Refused by name today
  ([`Missing::AMessageBehindACall`]).
  *Depends on 332, the traced native scratch state ADR 0031 decided for
  221 (built, iteration 208). Closes when:* a `message` getter runs once, after `name`'s, an
  object `message` is converted with its own `toString`, and a `name` getter that counts its calls is called once in
  both cases. Opened by a frozen real script that does it, and not before.

- [ ] **229. `AggregateError`.** Cut from 227. Its first argument is an
  iterable of errors, made into an array with `IterableToList`, so it needs the
  iteration protocol rather than an array-like walk.
  *Depends on 227, 211 and 75. Closes when:* `new AggregateError([a, b], 'm')`
  has an own `errors` array of the two, not enumerable, after `message` and
  `cause`; it inherits from `Error`; and an iterator that throws ends the
  construction with what it threw.

- [ ] **213. `arguments`, and the parameter forms that are not a plain name.**
  Cut from 209, which takes a plain list of distinct names and refuses the
  rest — a default, a `...rest`, a destructuring pattern, a repeated name, and
  the `arguments` object. They are one item because they are one part of the
  specification: a parameter list with any of them in it gets a **scope of its
  own**, separate from the body's, so that `function f(a, b = () => a) {}`
  closes over the parameter rather than over a `var a` in the body — and
  building them separately is how those two scopes come to disagree.
  `arguments` is here because it is the same machinery seen from the other
  side: it is made from the argument list before the body runs, and in sloppy
  code with a simple parameter list it is *mapped*, so writing `arguments[0]`
  writes the parameter. Today an `arguments` a function did not declare is
  refused by name rather than resolving to the realm, because *this engine has
  not built it* must not read as *this page has a typo*.
  *Depends on 209, on 211 for the destructuring a pattern parameter is, and on
  73 for the `Array` a `...rest` collects into. Closes when:* a default is
  evaluated once per call and only when the argument is `undefined`, a rest
  parameter is an array of what was left, `arguments.length` and `arguments[0]`
  answer, and the mapped and unmapped forms are told apart in a test naming
  which is which.

- [x] **214. A getter is a call, and so is a proxy's trap.** Cut from 209, and
  it is not "the rest of calling" — it is a different question. Item 209 calls
  a function from an instruction that is *about* calling; this is calling one
  from inside an instruction that is half way through something else, which
  means the interpreter has to be able to re-enter itself at a point where the
  operand stack is mid-expression. `object::Found::Getter` and
  `object::Set::Setter` already hand back the function rather than pretending
  to have called it, `convert::to_primitive` does the same when it finds a
  `valueOf`, and all three answer `Missing::ACall` today. A **proxy** is the
  same shape once more: it overrides `[[Get]]` rather than
  `[[GetOwnProperty]]`, which is a method added to `object::Internal` when
  there is a trap to call.
  *Depends on 209. Closes when:* `({ get a() { return 1; } }).a` is `1`, a
  setter sees what was assigned, `({}) + ''` calls a `toString` the object
  inherits rather than refusing, and a getter that calls something that reads
  the same property is bounded rather than a process that stops.

  **Done, all four closing conditions, and the proxy is cut to item 217.** The
  third one is met in the only form this engine can honestly meet it: a
  `toString` **the object inherits** is called — `({ __proto__: { toString() {
  … } } }) + ''` answers — and `({})` itself still has no prototype and so
  nothing to inherit, which is item 73's and is asserted as its own case rather
  than left to be discovered.

  **The decision worth reading twice is that the interpreter still does not
  recurse.** `interpret.rs` says it as a property rather than an accident, and a
  getter is exactly the thing that tempts an engine to break it: a call is
  wanted from inside an instruction that is half way through. So the instruction
  hands over instead, and the frame goes on the list every other call's frame
  goes on. What differs is a field on the frame ([`After`]) saying what the
  answer is *for* — the value the instruction leaves behind, a value to drop, a
  `typeof` to take, or one step of a conversion — so leaving a call is one
  `match` rather than four kinds of frame.

  **Two shapes carry all of it, and only one of them needs anything remembered.**
  A property access takes a known number of stack values and leaves one, and a
  call takes everything above its callee and leaves one in its place — so
  putting the getter *where the access's answer belongs* makes the getter's
  `return` the end of the access, with nothing resumed and nothing recorded. A
  setter is the exception, because `a.b = c` evaluates to `c` rather than to
  what the setter answered, so the value is written into the answer's place
  first and the call laid out above it with its answer dropped. A **conversion**
  is the one that genuinely resumes: the primitive is written into the operand's
  own stack slot and the instruction **runs again**. That is not a retry — every
  instruction in this engine reads its operands where they lie and takes them
  off only once the answer exists, so the second run is the same instruction on
  an operand that is now a primitive, which is the specification's own next
  step. `a + b` with objects on both sides runs three times and calls each
  side's `valueOf` once.

  **Neither loop.** A method that answers with an object again carries on at the
  *next* name and there are two, so running out is the `TypeError` the
  specification gives; and an accessor that reads itself makes a frame each
  time, which is `bounds::CALLS_ON_THE_STACK`. Six shapes of that are asserted,
  including one through a prototype and one that is a conversion rather than an
  access.

  **Two types keep the halves apart, and they are the change with the longest
  reach.** `convert::Primitive` wraps a value that is **not an object** and is
  the only way to make one, so `ToNumber` and `ToString` cannot be handed an
  object by mistake — before this, every one of them had an object arm
  answering *not built yet*, reachable from a dozen operators. And
  `operate::Applied::Wants` is how an operator says **which operand** it needs
  converted and with which hint, rather than converting it: that keeps the order
  `a > b` converts in — left first, which is what `LeftFirst` is *for* — inside
  the one file that knows it. `Missing::ACall` is gone from the engine
  entirely.

  **The realm went with it**, because a bare name can be an accessor too: a
  script cannot make one until item 73, and an **embedder** can, and a
  `document` behind a getter would otherwise be a name this engine could see and
  not read. `tests/a_name_behind_an_accessor.rs` is that path, with the getter
  and the setter written in the language rather than in Rust.

  **One thing was found and fixed rather than cut**, because item 209 had made
  it a lie: `instanceof` refused everything with *the right-hand side is not
  callable*, on the grounds that nothing in the heap was callable — and things
  are now. The two answers that come before `Get(C, "prototype")` are given (a
  right-hand side that is not callable is still that `TypeError`; a left-hand
  side that is not an object is `false`), and the rest names item 212.

  **Four new files, one reason to change each**: `interpret/property.rs`
  (reading and writing a property, either of which may be a call),
  `interpret/primitive.rs` (the conversion state machine), the two test files,
  and `frame.rs` gained the two types the frame now carries.

- [ ] **217. A proxy's trap.** Cut from 214 on the iteration that built it. A
  proxy overrides `[[Get]]` rather than `[[GetOwnProperty]]`, which is a method
  added to `object::Internal` — and the re-entry it needed is now built, so what
  remains is the trap object and the invariants. It is a cut rather than a
  refusal because **nothing can make one**: `new Proxy(target, handler)` needs
  `new` (item 212) and the `Proxy` constructor (item 73), so building the trap
  now would be building a mechanism no test could reach from a script and no
  page could reach at all. Item 214's own closing conditions do not name it.
  *Depends on 214, 212 and 73. Closes when:* a `get` trap runs and sees the key
  it was asked for, a trap that contradicts a non-configurable property is a
  `TypeError` rather than the answer it gave, and a proxy whose target is itself
  is bounded rather than a process that stops.

- [ ] **215. Tagged templates.** Cut from 209. `` tag`a${b}c` `` calls `tag`
  with an **array of the cooked strings, carrying a `raw` array**, then the
  substitutions — which is why it is not a call with a template as its first
  argument and why item 70 keeps a piece's raw text beside its cooked one. The
  strings array is also the one place in the language with an identity a page
  can rely on: the same template site hands the same array to every call, which
  is what a caching tag library is built on.
  *Depends on 209, and on 73 for the `Array` the strings arrive in. Closes
  when:* a tag receives the cooked and raw strings and the substitutions in the
  order the specification gives, a piece with an escape nobody can read is
  `undefined` in cooked and present in raw, and two calls of the same template
  site receive the same array.

- [x] **216. An environment per block, so a loop's `let` is a binding per
  pass.** Cut from 209, which puts a function's names in a heap environment and
  a **block's** names in frame slots that die with the call — so a function
  reading a name a block declared outside it is refused by name today. The
  refusal is honest and the hole is real: `for (let i = 0; …) { fns.push(() =>
  i); }` is ordinary code. What closes it is the specification's own shape —
  an environment for every scope that declares anything, pushed and popped
  around a block, and **copied** at each pass of a `for` head
  (`CreatePerIterationEnvironment`), which is what makes each closure see its
  own `i`. It is its own item because it changes what every name instruction
  means and because `break`, `continue` and `return` then have environments to
  unwind, which is a second thing to get right rather than the same one.
  *Depends on 209. Closes when:* a closure made in one pass of a `for (let …)`
  and one made in the next answer differently, a `let` in a loop's **body** is
  a fresh binding each pass in the same way, a `break` out of three nested
  blocks leaves three environments, and nothing that compiled before is refused
  after.

  **Done, all four: `crates/alo-js/tests/a_binding_per_pass.rs`.** The shape is
  the specification's own — an environment for every scope that declares
  anything, and `CreatePerIterationEnvironment` at each pass of a `let` head —
  and the thing worth reading twice is that **a copy is a sibling rather than a
  child**: the same parent, so every `hops` the compiler counted still means
  what it meant, which is what lets the copy happen at run time with no second
  instruction set.

  **Three rules make it small enough to be right.** A scope that declares
  nothing gets **no environment**, so a hop is counted by asking a scope rather
  than by counting levels — otherwise every empty block in a program would be a
  cell nothing could look a name up in and a link every name past it walked. A
  `const` head is **not** copied, because a copy could differ from the original
  only by existing. And **leaving is the jump's own business**: a `break` out of
  three blocks emits three pops, since the blocks it skips will never reach
  their own — which is why what a jump is leaving is found before anything is
  emitted rather than after.

  **`Where::Local` is gone entirely.** A block's names were frame slots and are
  now bindings, so nothing a script can name is a frame slot: what is left of
  one is the compiler's temporaries — a `switch`'s discriminant, the old value
  of an `a.b++` — which are written before they are read on every path. So
  `Op::Store` and `Op::Uninitialize` had no emitter left and are gone with them,
  a slot has no name to record, and reading an empty slot is
  `Internal::StackIsWrong` rather than a dead zone that no program can reach.

  **Two doctored runs rather than reasoning about them**: with the per-pass copy
  removed two tests fail, and with the unwinding pops removed two fail. The
  second doctoring found that one of them was passing by a coincidence of
  layout — the loop's `i` and the block's `seen` were each binding zero of their
  own environment and held the same number, so reading the wrong cell gave the
  right answer — which is why that case now declares a name in front of the one
  it reads.

- [x] **210. `try`, `catch` and `finally`.** Cut from 72, which throws and has
  nowhere for a throw to land: `Escape::Thrown` ends the script today, and what
  is owed is the handler. The hard half is **`finally`**, which runs on the way
  out of a `break`, a `continue`, a `return` and a throw alike, so a completion
  has to be carried across a jump rather than only propagated up.
  *Depends on 72, and on 227 (cut from 73) for the `Error` objects a `catch`
  binds — a thrown `TypeError` has a kind and a message and becomes an
  instance of `Intrinsics::error_constructor(Family::from(kind))` when caught.
  Closes when:* each of the five ways out of a `try` runs its `finally` exactly
  once, in a test naming which way it left, and a `catch` binds what was thrown.

  **Done, both of them: `crates/alo-js/tests/what_a_catch_catches.rs`**,
  twenty-one tests, every program run ordinarily and with the collector at
  every allocation except three runaway recursions, which run ordinarily only
  as `an_engine_that_is_hostile.rs`'s do. One test per way out — normally, by
  a throw, by a `return`, a `break` and a `continue` — each counting its
  `finally`. Opened by a frozen real script: the service worker
  (`crates/alo-corpus/scripts/alo-service-worker/script.js`) was refused at
  byte 2853, the `try` of its push handler; it now compiles to byte 2922, the
  `for (const account of …)` inside that `try`, which is item 211.

  **The decision worth reading twice is that a throw lands through a table and
  everything else through code.** A `try` emits no instruction at its start or
  its end: it writes a handler into the chunk — these instructions are guarded,
  a throw from them lands there — and the interpreter searches it only when
  something is thrown, innermost frame outwards. A `break`, `continue` or
  `return` the compiler can see, so each is compiled to go where it is going,
  and one that crosses a `finally` writes *which way it was leaving* into a
  frame slot and jumps into the block; at the block's end each way out is
  compiled **again, from there**, which is how a `break` past two `finally`s
  runs both, innermost first. A frame is searched by the instruction it is
  **running** (`Frame::now`), not its program counter less one, because a
  conversion rewinds the counter to run its instruction again. An error the
  engine threw becomes an instance of its kind's constructor **only when a
  `catch` lands it**, after the stack is cut, so nothing is in a Rust local
  while it is made.

  **Two consequences that are not about `try` at all.** A `RangeError` is now
  something a page can catch and carry on from, so a call that is refused for
  the value bound no longer roots an environment before it is refused — the
  check moved ahead of the allocation, or each refused call would have held one
  for the engine's life. And a recursion that catches its own `RangeError` can
  run for ever **without a backward jump** —
  `function f() { try { f() } finally { f() } }` doubles at every level — so the
  embedder's stop switch is read on every call into a script function as well
  as on a backward jump (ADR 0013 § 4).

  **What was refused rather than built**: a pattern as the `catch` parameter is
  item 211's, as every pattern is; and `catch (e) { var e; }`, which Annex B
  allows for old pages, is not a program here — that relaxation is item 142's.
  **Five doctored runs**: frames that keep their environment roots fail the
  test that counts live cells after three catches of ten thousand frames
  (`[20549, 41027, 61505]`); builtins left waiting fail one test; blocks left
  standing failed **nothing** at first, because every binding the tests read
  held the same value in the wrong environment as in the right one — three
  cases with a different letter in every binding were added, and now fail
  (`"yy"` for `"kk"`); no stop check on a call hangs the recursion test rather
  than failing it, which is the defect it describes. The fifth — searching by
  the program counter less one instead of `Frame::now` — fails nothing, and the
  reason is real rather than luck: the instruction that rewinds always has its
  operands' loads before it in the same guarded range, and the instruction at a
  range's end is always a jump this compiler emitted. `now` is kept because it
  is right by construction rather than by that argument.

- [ ] **211. The forms that take a value apart.** Cut from 72, which refuses
  them together because they are one mechanism seen from four sides: an array
  literal and a spread *build* from an iterable, a destructuring pattern and
  `for…of` *read* from one. `for…in` is here too, and it is the odd one — it
  enumerates keys rather than iterating, with a prototype-shadowing rule of its
  own.
  *Depends on 72, on 73 for the `Array` exotic object (the exotic part is
  `length`) and on 75 for the iteration protocol.* **Item 225 took the exotic
  object and the array literal without a spread**, and **item 230 took
  `for…of` with a name or a property as its head, and the protocol it reads
  through** — `GetIterator`, a step, `IteratorClose` — compiled to ordinary
  instructions that a spread and an array pattern can be compiled to as well.
  So the dependency on 75 is met for everything left here, which is the
  spread, both patterns (in a declaration, an assignment, a parameter, a
  `catch` and a `for…of` head) and `for…in`. No frozen script reaches any of
  them yet: the service worker compiles whole since item 230.
  *Closes when:* `[1, ...a]`,
  `let [a, b] = c`, `let { a } = b`, `for (const [k, v] of m)` and `for…in`
  produce what the specification says, in the same table item 72 uses, and a
  hole is not `undefined`. Opened by a frozen real script that uses one.

- [x] **230. `for…of` over an array, and the iteration protocol it reads
  through.** Cut on the iteration that built it from item 211 (`for…of`), item
  75 (*iterators*) and item 73 (`Symbol.iterator`, `Symbol.toStringTag` and
  the array iterator), because the real failure needed one piece of each.
  Opened by a frozen real script: after item 210 the service worker
  (`crates/alo-corpus/scripts/alo-service-worker/script.js`) was refused at
  byte 2922, `for (const account of Object.values(…))`.
  *Depends on 72, 210, 218, 219 and 225. Closes when:* `for…of` over an array
  produces what the specification says in a table, a hole included; `let`
  and `const` are a binding per pass and the head has a dead zone; `var`, a
  name and a property are assigned each pass; leaving by `break`, `return` or
  a `continue` of an outer loop calls the iterator's `return` and a failure of
  it is thrown, a throw from the body calls it and ignores what it does, and
  finishing, `continue` and a throw from the iterator itself call nothing;
  `next` is read once and called with the iterator as `this`, `done` every
  pass and `value` only when not done; a wrong answer from the protocol is the
  language's `TypeError`; an array iterator says `[object Array Iterator]`;
  what is not built is refused by name; and the frozen script compiles past
  byte 2922.

  **Done, all of them: `crates/alo-js/tests/what_for_of_reads.rs`**, thirty-three
  tests, every table program run ordinarily and with the collector at every
  allocation and required to agree, and
  `builtin/array_prototype.rs`'s unit test, which builds the new intrinsics
  with the collector at every allocation. **The frozen service worker now
  compiles whole.** Running it stops at its first line that reads `self`, a
  worker's global, which is an embedder's to supply (item 91).

  **The protocol is calls a page can see, compiled as calls.**
  `obj[Symbol.iterator]()`, each `next()`, and each read of `done` and `value`
  is an ordinary `GetKeyed`, `Call` or `GetNamed`, so a page's own `next`, a
  `done` behind a getter and a `return` that throws each behave as they do
  elsewhere, and the interpreter learned three things only: a well-known
  symbol no source can spell (`Op::WellKnown`) and the protocol's two
  `TypeError` checks (`Op::Iterable`, `Op::RequireObject`).

  **Closing reuses `finally`'s routing.** The loop pushes a `Finally` of its
  own (`Finally::closing`), so a `break`, a `return` and a `continue` of an
  outer loop are numbered, routed into the closing and carried on from its end
  exactly as through a `finally` — innermost first across nested loops and
  `try`s — and the loop's **own** `continue` is the one exit that does not
  cross it. A throw from the body lands in a handler around the body only, so
  a throw from the iterator closes nothing; a second handler around the call
  to `return` is what ignores whatever the closing does on that path.

  **Two well-known symbols, no `Symbol`.** `Symbol.iterator` and
  `Symbol.toStringTag` are made once per realm and rooted by the intrinsics;
  `Object.prototype.toString` now reads the tag, through a getter if it is one.
  `Engine::well_known` gives an embedder the symbols, because an embedder's
  object (a node list) becomes iterable the same way; no script can name them
  until item 73 builds `Symbol`.

  **Refused by name and cut to item 231**: an array iterator reading an
  element or an array-like's `length` through a getter or a conversion. A
  destructuring head stays item 211's and `for await` item 75's.

  **Six doctored runs**, each restored and the suite re-run green: no closing
  on `break` fails eight tests; no handler around the body fails one; a loop's
  own `continue` closing fails three; no dead zone for the head fails one; one
  environment for every pass fails four; and the result's value not held
  across its allocations fails two, both under the collector at every
  allocation.

- [ ] **231. An array iterator that reads through a call.** Cut from 230.
  `%ArrayIteratorPrototype%.next` reads an element with `Get`, and an
  array-like's `length` with `Get` and `ToLength` — any of which may run a
  page's getter or `valueOf`. The specification writes the iterator as a
  generator, so a call from inside it makes two generator states observable
  that this iterator does not keep: *executing*, in which a getter that calls
  `next` again gets a `TypeError`, and *completed* after a throw, after which
  every `next` answers `done`. So the three are refused by name
  ([`Missing::AnIteratedValueBehindACall`]) rather than half right. An array's
  own `length` is never one of them, so `for…of` over an ordinary array never
  reaches the refusal.
  *Depends on 230, and on 75 for a generator's states or on a native being
  told that a call it waited on was taken down by a throw (item 219's
  mechanism, extended). Closes when:* an element getter runs once per `next`,
  a getter that calls `next` again gets a `TypeError`, a throw from the getter
  leaves the iterator completed so the next `next` is `{ value: undefined,
  done: true }`, and an array-like's `length` getter and `valueOf` each run
  once per `next`. Opened by a frozen real script that does it, and not
  before. *Reachable since iteration 209:* `for (const n of
  document.querySelectorAll(…))` reaches the refusal, because a
  `NodeList`'s `length` is a getter; no frozen page does it yet.

- [x] **218. A builtin is a function this engine wrote, and `{}` has a
  `toString` of its own.** Cut from 73 on the iteration that started it, which
  named no closing condition of its own and could not have — *the ECMAScript
  builtins* is years of work. What is taken here is the piece every other one
  needs and the piece a page needs first: the **mechanism** (a function whose
  body is Rust, called by the machinery every other call goes through) and the
  two objects that are not a library but what an object and a function *are*.
  *Depends on 72, 209, 214. Closes when:* `({}) + ''` answers
  `"[object Object]"`, a page's own `toString` shadows the builtin and a page
  may replace the builtin itself, a builtin is reached by a conversion and by a
  getter rather than only by a call the source spells, and what is not built
  refuses **by name** rather than answering something plausible.

  **Done, all four: `crates/alo-js/tests/what_a_builtin_answers.rs`.** The
  thing to read twice is that a native is **the same cell** a compiled function
  is — `object::Code` is where the body came from and nothing else differs — so
  `typeof` needed no case, `IsCallable` needed no change, and a builtin works
  as a getter, as a setter and as the `toString` a `+` reaches for because
  `finish_call` is shared with `Op::Return` rather than written twice.

  **Three rules keep it small enough to be right.** A native is a **function
  pointer rather than a boxed closure**: everything a builtin could capture is
  either a reference the collector must walk or the realm it is reached
  through, so a native holds no edge and tracing one is nothing. It is given
  **no interpreter** — the heap, its `this`, its arguments and a source offset —
  which is the bound that makes a native call free of a frame and is why `call`,
  `apply` and a conversion inside a builtin are item 219 rather than something
  quietly allowed. And a builtin is **strict code**, so its `this` is what the
  caller wrote: a bare `toString()` is `"[object Undefined]"` here as it is in
  every other engine.

  **The doctored runs found the gap that mattered.** Removing the
  `Function.prototype.toString` refusal fails a test, and defining a method
  enumerable fails another — but removing the **scope that roots a name between
  interning it and defining it** failed nothing, because every other file turns
  stress on *after* `Engine::new` has already built the realm. So
  `builtin.rs` has a test that builds the intrinsics with the collector firing
  at every allocation, and that test fails without the holds.

  One thing the tests found rather than assumed: `a.__proto__ = null;
  a.__proto__` is `undefined` and not `null`, because the accessor lived on the
  prototype that was just cut away. That is what every engine answers, and it
  is only obvious once written down.

- [x] **219. A builtin that calls back into the script.** Cut from 218, whose
  natives are given the heap and no interpreter — so a builtin that must call
  something cannot, and each one that would says so by name rather than
  guessing. `Function.prototype.call` and `apply`, `Object.prototype.
  toLocaleString`, and `ToPrimitive` on an argument
  ([`Missing::AConversionInsideABuiltin`], which is what
  `({}).hasOwnProperty({})` answers today) are the three shapes of it, and
  `Array.prototype.map` is the shape everything after them takes. The
  interpreter already has the mechanism — [`Engine::begin_call`] lays a call
  out from a place an instruction chooses and [`After`] says what its answer is
  for — so what is owed is a way for a **native** to ask for one and be
  re-entered with the answer, which is a native that can suspend and is
  therefore a design rather than a chore.
  *Depends on 218. Closes when:* `f.call(o, 1)` runs `f` with `o` as its `this`,
  a builtin that asked for a call and was re-entered gets the answer in the
  place it left, and a builtin that calls itself for ever is the `RangeError` a
  runaway function is rather than a process that stops. **`bind` is not in
  it**: a bound function is an exotic object with its own `[[Call]]`, which is
  a different piece of work, and it goes in item 220 beside the rest of
  `Function.prototype`.

  **Done.** A native returns an `Answer::Want` with a continuation step;
  the interpreter runs the requested call or conversion and resumes it with
  the answer held on the traced value stack. `call`, `toLocaleString` and
  object-to-property-key conversion use it. The recursion bound counts both
  script frames and waiting builtins, including mixed chains. The closing
  conditions run normally and with collection at every allocation in
  `what_a_builtin_asks_for.rs`. **`apply` is cut to item 221**: retaining a
  partially assembled argument list across accessor calls needs traced state.

- [ ] **221. `Function.prototype.apply` and traced native scratch state.**
  Cut from item 219. A native currently retains its receiver, arguments, answer
  and a numeric continuation step. `apply` must accumulate an argument list
  while reading an array-like's `length` and indexed properties, any of which
  can call script. Keep that intermediate list in collector-visible storage.
  **Decided by ADR 0031 (iteration 207):** the scratch state is slots on
  the stack (item 332), and the growing list a prototype-less array one
  slot holds (§ 3).
  *Depends on 219, 332 and the length conversions from 73. Closes when:* `apply`
  forwards an array-like's values in order, null/undefined mean no arguments,
  getters execute once in order, getter exceptions propagate, and excessive
  lengths are bounded before allocating. All cases must survive collection at
  every allocation. Array objects themselves remain part of item 73.
  **Also owed here (iteration 145):** `el.setAttribute(a, b)` with an
  object for both arguments is refused by name
  (`Missing::ASecondArgumentBehindACall`), since the first argument's string
  is in the slot the second conversion's answer is written to; the same
  scratch state closes it, with a test that each `toString` runs once, in
  order.
  **And from item 74 (iteration 200):** `RegExp.prototype.exec` and `test`
  refuse the same shape by name (`Missing::ATwoCallRegExpMethod`): a string
  argument that ran script to convert, followed by a getter for `exec` or a
  `lastIndex` holding an object. Closes when both run each call once, in
  the specification's order.

- [ ] **220. A function's own `name` and `length`, its source text, and
  `bind`.** Cut from 218. A function object has **no own properties** but the
  `prototype` a constructor carries (item 212): `f.name` and `f.length` are `undefined`, and
  `Function.prototype.toString` refuses by name
  ([`Missing::AFunctionsSourceText`]) rather than letting
  `Object.prototype.toString` answer `"[object Function]"` — a sentence no
  engine produces, handed to a page as though it were right. `length` alone
  would be half an item, because a `name` needs the **compiler** to record one:
  `Chunk::own_name` is set only for a named function expression, so a
  `function f() {}` and a `const f = () => {}` each have a name nothing keeps.
  The source text is the same shape of gap — item 204 gave every node the bytes
  it came from, and no [`Unit`] keeps the source those bytes index into.
  *Depends on 218, and `bind` on 219. Closes when:* `function f(a, b) {}` has
  `f.name === 'f'` and `f.length === 2` with the attributes the specification
  gives them, an arrow and a method get the name they are assigned to,
  `f.toString()` answers text that parses back to the same function, and a
  bound function calls the one it wraps with the `this` it was bound to.

- [ ] **73. The ECMAScript builtins**, in the order real pages need them rather
  than the order the specification lists them.
  *Depends on 72, and on 218 for what a builtin is.* **Item 218 took the
  mechanism and the two prototypes**; what remains is the library — `Object`
  and `Function` themselves (which are constructors: item 212 built `new` for a
  function a script wrote, and a **builtin** with a `[[Construct]]` is this
  item's, since none exists yet),
  `Array` (whose exotic object and prototype item 225 built — what remains is
  the constructor, `Array.isArray` and every method), `Math`, `JSON` (the
  `Error` family is item 227, with 228 and 229 cut from it), and the `String`,
  `Number` and `Boolean` wrappers that
  [`Missing::AWrapperObject`] names. This item should be **cut again** the next
  time it is taken: a closing condition that names one family is an item, and
  the whole of a standard library is not.
  **Item 206 left three things here by name**: a **partial** property descriptor
  (`{ writable: false }` with no `value`), which is `Object.defineProperty`'s own
  reading of an argument object rather than a rule about properties — the rules
  are in `object::Property` and take a complete one; the **well-known symbols**
  and the `Symbol.for` registry, each of which is a symbol made once and rooted
  by the realm that owns it rather than a change to what a symbol is; and the
  **weak collections** — `WeakMap`, `WeakSet`, `WeakRef`, `FinalizationRegistry`
  — each of which is a cell of its own, over an ephemeron fixpoint that is
  already built and tested (item 71).
  **Item 230 took two of the well-known symbols and the iterators of
  `Array.prototype`**: `Symbol.iterator` and `Symbol.toStringTag`, made by the
  realm with no `Symbol` function to reach them; `Object.prototype.toString`'s
  reading of the tag; `%IteratorPrototype%` with its `[Symbol.iterator]`; and
  `keys`, `values`, `entries` and `[Symbol.iterator]` on `Array.prototype`,
  over `%ArrayIteratorPrototype%`. The `Symbol` function, the other eleven
  symbols and the iterator helpers stay here.
  **`Math` is cut into 365** (iteration 234), opened by alo Sites'
  analytics script.

- [x] **74. Regular expressions**, with the syntax the language actually has.
  *Depends on 72. Closes when:* a hostile pattern is refused or bounded rather
  than running for ever — a catastrophic backtrack in a renderer is a denial of
  service.
  **A running script reaches one (iteration 198):** `alo-downloads`, alo's
  public download page, is refused at byte 144 of its script, `/Mac/.test(p)`,
  "a regular expression literal is not built yet". Until then no frozen
  script that runs had reached a regular expression, which is why this item
  was left alone. That page also calls `classList.add`, `querySelectorAll`,
  `forEach`, `fetch` and `.then`, so making the script run whole needs more
  than this item; the first cut is the literal and `test`.
  **Decided in ADR 0029 (iteration 199).** The parser, compiler and matcher
  are ours: a backtracking machine over UTF-16 code units, with a
  backtrack stack of its own and no native recursion. Every step is counted
  against a budget in `bounds.rs`, and the embedder's stop is checked
  inside the matcher. Running out is a `RangeError`, never "no match".
  `regress` is refused because the bound has to live inside its loop. Only
  the Unicode tables are rented. Annex B's pattern forms are refused by
  name. **What 74 now builds** (ADR 0029 § 6): the parser for the whole
  grammar, with what is not built refused by name; the compiler; the
  matcher with its budget, its stack ceiling and the stop; a `RegExp` from
  a literal, compiled with its script so a bad pattern is an early
  `SyntaxError`; `exec` and `test`; `lastIndex` with `g` and `y`; and `s`
  and `m`. `i` and `\p` are 322, the string methods 323, and the
  constructor, `v`'s set operations and `d`'s indices 324.
  *Closes when:* `/(a+)+$/` against thirty `a`s and a `b` is a `RangeError`
  a page's `catch` catches, a stop from the embedder ends a match below the
  budget, a pattern nested past the parse bound is a `SyntaxError`, a table
  of patterns with the specification's captures passes (lazy and greedy,
  alternation order, captures reset in a quantifier, backreferences,
  lookahead and lookbehind), and `alo-downloads`' script gets past
  `/Mac/.test(p)`.
  **Built (iteration 200).** `alo-js/src/regexp/`: the pattern parser for the
  whole grammar (`u`, `v`'s set expressions and `\q{…}`, named groups and a
  name shared by alternatives, lookbehind, `(?ims-ims:…)`, `\p{…}` read
  but not looked up), checked by the parser so a bad pattern is an early
  `SyntaxError` naming what is wrong; Annex B's six forms refused as
  `Legacy`, by name, without `u` or `v`. The compiler, refusing `i` and
  `\p` (322) and `v` and `d` (324) by name. The matcher: a loop with its own
  stack of places to come back to, `bounds::STEPS_IN_A_MATCH` (2²⁴) counted
  per `RegExpBuiltinExec` and `bounds::PLACES_IN_A_MATCH` (2²¹), both a
  `RangeError`; the embedder's stop asked on every backtrack and every 1024
  steps, which builtins now reach through `Call::stop_asked`. A repeated
  single character holds one place however long it runs. A literal makes a
  `RegExp` cell per evaluation, sharing the program the unit compiled;
  `%RegExp.prototype%` has `exec` and `test` (generic, honouring a page's
  `exec`); `lastIndex` with `g` and `y`, and the pair rule under `u`;
  `"[object RegExp]"`. **Closed:** `/(a+)+$/` against thirty `a`s and a `b`
  is a `RangeError` a `catch` catches and the engine matches on after it; a
  stop asked before the run ends the match from inside the matcher, within
  1024 steps; a pattern nested past `bounds::DEEPEST_PATTERN` (256) is a
  `SyntaxError`; the specification's own examples, lookbehind's
  right-to-left captures, backreferences, captures reset in a repeat and
  named groups pass as tables run with the collector at every allocation
  (`tests/what_a_pattern_matches.rs`), and hostile, truncated and random
  patterns in all three modes are answered (`a_pattern_that_is_hostile.rs`).
  `alo-downloads`' whole script compiles, and its first five lines give
  `isMac` and `isWin` right for a Mac, Windows and Linux under a stand-in
  `navigator` (`a_frozen_page_tests_its_platform.rs`). In the renderer the
  script now stops at its second line, `navigator.platform` — item 325.
  **Refused by name, owed elsewhere:** an object string argument followed by
  a second call (an `exec` behind a getter, an object `lastIndex`) is
  `Missing::ATwoCallRegExpMethod`, item 221; `source`, `flags` and
  `toString` are 324's, so `String(/a/)` is `"[object RegExp]"` until then,
  as `String([1])` is `"[object Array]"` until 73.

- [ ] **322. `i`, and `\p{…}`: the rented Unicode tables.** *Cut from 74
  (ADR 0029 §§ 1, 6).* Case-insensitive matching uses simple case folding
  under `u` and `v`, and the specification's upper-case `Canonicalize`
  without them. `\p{…}` and `\P{…}` name `General_Category`, `Script`,
  `Script_Extensions` and the binary properties, and under `v` the
  properties of strings. The tables are rented, behind one file on
  `scripts/gate.sh`'s boundary list, from a crate that reaches nothing and
  adds no `unsafe` of ours. 74's first cut refuses `i` and `\p` by name.
  *Depends on 74. Closes when:* a table has `/ß/i`, `/ſ/i` and
  `/K/i` matching as the specification says with and without `u`, a
  `\p{Script=Greek}` and a `\p{Lu}` match and an unknown property is a
  `SyntaxError`, and the crate is named in exactly one file.

- [ ] **323. The string methods that take a regular expression.** *Cut
  from 74 (ADR 0029 §§ 5, 6).* `String.prototype.match`, `matchAll`,
  `replace` (with `$1`, `$<name>`, `$&` and a function replacer),
  `replaceAll`, `search` and `split`, and `RegExp.prototype`'s
  `[Symbol.match]`, `[Symbol.matchAll]`, `[Symbol.replace]`,
  `[Symbol.search]` and `[Symbol.split]`, reached through the symbols so a
  page's override is honoured. The theme generator's `.replace(/…/g, "")`
  and `.match(…)` are the frozen calls that need it.
  *Depends on 74 and on `String.prototype` (item 73). Closes when:* a
  table of each method's results matches the specification, a global
  `replace` on a hostile pattern is bounded per match, and the theme
  generator's calls give the values a browser gives.

- [ ] **324. The `RegExp` constructor, `v`'s set operations and `d`'s
  indices.** *Cut from 74 (ADR 0029 § 6).* `new RegExp(source, flags)` and
  `RegExp(…)` compile at run time, so a bad pattern throws where the call
  is; `source`, `flags` and the per-flag accessors on `RegExp.prototype`;
  `v`'s class set operations (`--`, `&&` and nested classes); and the
  `indices` array `d` adds to a match.
  *Depends on 74. Closes when:* `new RegExp("(")` throws a `SyntaxError` a
  `catch` catches, `/[\p{L}--[a-z]]/v` matches as specified (with 322),
  and `/(a)/d.exec("a").indices` is `[[0, 1], [0, 1]]`.

- [x] **325. `navigator`: the `Navigator` interface, with `platform` and
  `userAgent`.** *Opened by `alo-downloads` (iteration 200).* With item 74
  built its script compiles, and stops at its second line:
  `var p = navigator.platform || "";` is "ReferenceError: 'navigator' is not
  defined". The page reads `navigator.platform` and `navigator.userAgent` to
  mark the visitor's card. `Navigator` is a Web IDL interface on the window
  (`alo-bindings`), and what it answers is the browser's to decide: a
  platform and a user agent string are a fingerprinting surface, so the
  values need a decision about what this browser says about itself, not a
  copy of another browser's. Item 318's `pdfViewerEnabled`, `plugins` and
  `mimeTypes` hang off the same object.
  **Decided in ADR 0030 (iteration 201).** The browser names itself and
  claims no other engine: `Mozilla/5.0 (<system token>) alo/<major>.<minor>`,
  one frozen token per kind of system chosen at compile time (`Macintosh;
  Intel Mac OS X 10_15_7`, `Windows NT 10.0; Win64; x64`, `X11; Linux
  x86_64`), nothing measured about the machine. The same string is the
  `User-Agent` header (326) and `navigator.userAgent`, composed in one file
  in `alo-net` and told to the renderer in `Page`. The compatibility mode is
  Gecko: `vendor` empty, `productSub` `"20100101"`, `appVersion` by HTML's
  Gecko steps, `taintEnabled()` false and `oscpu` empty; `platform` is
  `MacIntel`, `Win32` or `Linux x86_64`. Until item 251, `navigator` is a
  data property on the global object, as `document` is.
  *Depends on 326, on the window's globals (`alo-bindings`) and, for its
  last clause, on 327. Closes when:* `alo-downloads`' script runs past its fifth line in the renderer
  and the card it marks matches the platform the browser says it is, in the
  box tree and the reference render, and every member in ADR 0030 § 5's
  table answers what it says.
  **Built (iteration 203).** `Page` carries `user_agent` and `platform`,
  filled from `alo-net::user_agent` by the browser process and sent across
  the wire with the page; `Held::scripted` hands them to `alo-bindings` as an
  `Identity`, and `introduce` puts the page's `Navigator` on the global
  object as a writable, configurable data property (`[Replaceable]`, until
  item 251). `Navigator` is an interface of its own in the document cell's
  list, inheriting `Object.prototype`, its members one file
  (`interface/navigator.rs`): every row of § 5, `appVersion` by HTML's
  Gecko steps over the told string (`navigator.rs`), each behind the brand
  check. **Met:** every member answers § 5's table for all three systems,
  with the collector at every allocation (`alo-bindings`'
  `what_a_page_is_told_the_browser_is.rs`); a page reads exactly the
  string `alo-net` sends, and one told otherwise answers what it was told
  after the wire (`alo-renderer`'s `a_page_is_told_the_browser.rs`); and
  `alo-downloads`' script runs past its fifth line in the renderer, its
  first five finding the Mac, Windows or neither as told
  (`alo_downloads.rs`). **Not met:** the card marked in the box tree and
  the reference render. The script's next line,
  `document.getElementById("card-mac").classList.add("rec")`, is DOM the
  page needs and `navigator` is not — **cut to 327**, which this item now
  depends on for its last clause.
  **Closed (iteration 204)** by 327: rendered as the Mac the corpus says
  it is, `alo-downloads` marks `card-mac` `.rec` and shows its badge, in
  the box tree, the layout and the reference render, and a Windows machine
  and a Linux one mark the Windows card and neither
  (`alo-corpus/tests/alo_downloads.rs`).

- [x] **327. `getElementById`, `hidden` and `classList`: what marks
  `alo-downloads`' card.** *Cut from 325 (iteration 203).* The page's
  script now reads `navigator`, finds the system, and stops at line 8,
  `document.getElementById("card-mac").classList.add("rec")` — "undefined
  is not a function" — and then sets `badge-mac`'s `hidden` to `false`.
  `Document.getElementById` (the first element in tree order with that
  `id`, `null` for none); `HTMLElement.hidden`, HTML's reflection
  (`true`, `false`, or `"until-found"`; setting `false` removes the
  attribute); and `Element.classList`, a `DOMTokenList` — `[SameObject]`,
  live over the `class` attribute — with at least `add` (an empty token a
  `SyntaxError`, one with whitespace an `InvalidCharacterError`, both
  `DOMException`s; the ordered set written back through `alo-dom`), and
  `contains`, `remove` and `toggle` if they fit, cut again by name if not.
  **The corpus must say which system it renders as** once the render
  depends on it: `Rendering::of` uses `Page::new`, which is this machine's
  identity, so a Mac and a Linux machine would draw different references.
  The case states one row of ADR 0030 § 2 (its `origin.txt` says which),
  and a test checks the other two in the box tree.
  *Depends on 325's built part. Closes when:* a table of `getElementById`,
  `hidden` and `classList` results matches the standards, each refusal is
  the right `DOMException`, and `alo-downloads` renders with the stated
  system's card marked `.rec` and its badge shown, in the box tree and the
  reference render — which then closes 325.
  **Built (iteration 204).** `Document.getElementById` (first element in
  tree order among the document's descendants with exactly that `id`;
  `""` and a template's contents never found). `HTMLElement.hidden` in
  its own file (`interface/hidden.rs`): `"until-found"`, `true` or
  `false`, and the setter's `(boolean or unrestricted double or
  DOMString)?` with HTML's steps. `Element.classList`, a `DOMTokenList`
  cell (`token_list.rs`) holding its element's wrapper and kept by it
  (`[SameObject]`), computing the ordered set from `class` on every call
  (`tokens.rs`), so it is live with no copy; `length`, `value` (and its
  setter), `toString`, `contains`, `add`, `remove` and `toggle`, with the
  update steps writing nothing to an element without `class` for an empty
  set. One object among `add`'s or `remove`'s tokens is converted; a
  second is `Missing::ASecondArgumentBehindACall` (item 221). The corpus
  renders every loaded case as `alo_corpus::SYSTEM`, ADR 0030 § 2's macOS
  row. **Met:** the table of results and refusals, with the collector at
  every allocation (`alo-bindings/tests/what_marks_an_element.rs`, 9
  tests); `alo-downloads` renders with `card-mac` `.rec` and its badge
  shown in the box tree and the reference render, and the other two rows
  checked in the box tree (`alo_downloads.rs`). The page grew 18.66 px
  with the badge, so the case is rendered at 800 × 780.
  **Cut, by name:** the indexed getter and `item`, `replace`, `supports`,
  iteration and `[PutForwards=value]` — **328**. The script now stops at
  `document.querySelectorAll(".btn[href]").forEach(…)` on line 17 —
  **329**.

- [ ] **328. The rest of `DOMTokenList`.** *Cut from 327 (iteration
  204).* `classList[0]` and `item(index)` (an indexed property getter,
  which needs the list to answer own properties it does not store, and
  `unsigned long`'s conversion); `replace(token, newToken)`;
  `supports(token)`, a `TypeError` for `classList` since `class` has no
  supported tokens; `forEach`, `keys`, `values`, `entries` and
  `[Symbol.iterator]`, Web IDL's value iterator (after item 75's
  iterators); and `classList`'s `[PutForwards=value]`, so assigning a
  string to `el.classList` sets its `value` rather than being ignored.
  *Depends on 327, and for iteration on 75. Opened by no page yet:* take it
  when a frozen page reaches one of these. *Closes when:* a table of each
  member's results and refusals matches the DOM standard, run with the
  collector at every allocation.

- [x] **329. `querySelectorAll`, and the `NodeList` it answers.** *Opened
  by `alo-downloads` (iteration 204).* With 327 built the page marks its
  card and stops at line 17,
  `document.querySelectorAll(".btn[href]").forEach(function (a) { … })`:
  "undefined is not a function". `ParentNode.querySelectorAll` on
  `Document` (and `Element`), matching with `selectors` as `alo-style`
  already does — one matcher, not a second — a selector that does not
  parse a `SyntaxError` `DOMException`, answering a **static** `NodeList`
  with `length`, `item`, the indexed getter and `forEach`. The callback's
  `fetch` and `.then` are items of their own (Fetch from script, 75), so
  the script would then stop there.
  *Depends on 327. Closes when:* a table of selectors and the elements
  they answer, in tree order, matches the standard; a selector that does
  not parse is the right `DOMException`; and `alo-downloads`' script runs
  past line 17's `querySelectorAll` and stops at its next missing member,
  named in the case's `origin.txt`.
  **Built (iteration 205).** `alo-css` reads a whole string as a selector
  list (`SelectorList::parse_text`, the parser style sheets use), refusing
  text whose blocks nest past 32 before the rented parser recurses into it
  (`nesting.rs`: `":is(".repeat(1000)` overflowed a test thread's stack),
  and its one matcher takes a `:scope` element (`MatchContext::scoped`).
  `ParentNode.querySelectorAll` (`interface/parent_node.rs`) is on
  `Document`, `Element` and `DocumentFragment`: the node's descendants in
  tree order, `:scope` the element asked or else `:root`, the embedder's
  stop asked at each node, and text that is not a selector list this
  engine has — `:has()` and an undeclared namespace prefix among it — a
  `SyntaxError` `DOMException`. It answers a **static** `NodeList`
  (`node_list.rs`): a cell holding each match's wrapper strongly, its own
  `[[GetOwnProperty]]`, `[[DefineOwnProperty]]`, `[[Delete]]`,
  `[[OwnPropertyKeys]]` and `[[PreventExtensions]]` written as Web IDL's
  legacy platform object with an indexed getter, and `length` and `item`
  (an `unsigned long`, modulo 2³²) on its prototype
  (`interface/node_list.rs`). **Met:** a table of selectors and what they
  answer, in tree order, from a document and from an element; every
  string that is not a selector list a `SyntaxError` `DOMException`; the
  list's indices, length, `item` and refusals, its staticness, and a node
  only it holds surviving collections — all with the collector at every
  allocation (`alo-bindings/tests/what_a_selector_finds.rs`, 10 tests,
  hostile nesting and a 2 000-element document among them); and
  `alo-downloads`' script runs past line 17's `querySelectorAll`, which
  finds its two buttons, and stops at `.forEach`, named in `origin.txt`.
  **Cut, by name:** `forEach`, `keys`, `values`, `entries` and
  `[Symbol.iterator]` — **331**: Web IDL makes them `Array.prototype`'s
  own functions, and this engine has no `Array.prototype.forEach`.
  `querySelector`, `children` and the rest of `ParentNode` stay absent
  until a page needs them. A style sheet nested as deep crashes the
  renderer the same way — **330**, found by this item and not its to fix.

- [x] **330. A style sheet nested past the limit is refused rather than
  overflowing the stack.** *Found by 329 (iteration 205).* `alo-css`'
  `parse_stylesheet` hands a page's `<style>` to `cssparser` and
  `selectors`, which recurse once per nested block: a sheet whose selector
  is `":is(".repeat(5000)` overflowed a test thread's stack and aborted the
  process. A page's style sheet is bytes from outside (`LOOP.md`, stage 2
  § 2), and a crash in a renderer is a denial of service. `nesting.rs`
  already measures the depth without recursing, for `querySelectorAll`;
  a sheet needs it per rule — a prelude or a block nested too deep drops
  that rule with a `StyleIssue`, never the sheet — and in declaration
  values (`var()` fallbacks, functions) and `@media` conditions, which are
  read from the same token stream.
  *Depends on nothing. Opened by no page:* found by a hostile input, which
  is what stage 2 § 2 asks for. *Closes when:* a sheet with a selector, a
  declaration value and a media condition each nested 100 000 deep is
  parsed without a crash, the rules nested past the limit are dropped with
  an issue that says why, and the rules around them are kept — on a
  thread with the renderer's own stack size.
  **Built (iteration 206).** `alo-css`' `parse.rs` measures each
  rule's selector list, each declaration's value and each at-rule's
  prelude with `nesting::within_limit` before anything recursive reads
  it — the text is drained token by token, which `cssparser` does with a
  heap stack rather than recursion, and the parser put back — and counts
  how many `@media` blocks a rule is inside. Past 32, that rule or that
  declaration is dropped with a new `IssueKind::NestedTooDeep` ("blocks
  nested deeper than 32, dropped"); the sheet around it is untouched.
  At `90d56b5`, before this change, a value, a selector and `@media`
  nested 100 000 deep each aborted an 8 MiB thread with a stack overflow
  (measured in a scratch worktree). **Met:** `alo-css/tests/nested_too_deep.rs`,
  8 tests on a thread with the 8 MiB stack `alo-render`'s main thread has
  on macOS and Linux, in a debug build: a selector, a value (in `(`, `[`,
  `calc(` and `{`), a `var()` fallback, a media condition and `@media`
  itself, each 100 000 deep, dropped with an issue naming the limit and
  the line, the rules either side kept, 32 levels of `@media` kept and
  the 33rd refused, 32-deep text of each kind kept, and five sheets that
  end inside 100 000 open blocks refused rather than crashed. What reads
  the values afterwards already bounds its own depth (`alo-style`'s
  substitution at 32; `alo-value`'s colours, gradients and transforms enter
  a fixed number of blocks, and its `calc()` stops at 16), so a value composed
  through several `var()`s cannot rebuild the depth this refuses.

- [x] **331. `Array.prototype.forEach`, and a `NodeList`'s iteration.**
  *Cut from 329 (iteration 205).* `alo-downloads`' script now stops at
  `document.querySelectorAll(".btn[href]").forEach(…)`: Web IDL makes a
  `NodeList`'s `forEach`, `keys`, `values`, `entries` and
  `[Symbol.iterator]` **the very functions** `Array.prototype` has under
  those names, and `Array.prototype.forEach` is not built (item 73). It is
  generic: it reads `length` once, then calls the callback for each index
  the object has. **Needs ADR:** a builtin keeps a `u32` step and nothing
  else across a call it asks for (`object/native.rs`), and `forEach` must
  keep its length and its index across one call per element, where
  reading `length` again would visit what the callback appended. How a
  builtin keeps state across the calls it asks for decides `map`,
  `filter`, `reduce`, `every`, `some`, `find` and every promise reaction
  after it, so it is decided once, before any of them is built.
  **Decided (iteration 207): ADR 0031** — a builtin declares up to eight
  value slots, reserved on the stack above its arguments and written there
  at once; `len` and `k` are two of them, and a loop over holes asks the
  embedder's stop (§ 7). The slots themselves are **332**.
  *Depends on 332 and 329, both built (iterations 208 and 205). Closes
  when:* `Array.prototype.forEach`
  answers a table of arrays and array-likes as the specification does —
  holes skipped, `thisArg` passed, the length read once, a throwing
  callback ending it — with the collector at every allocation; a
  `{ length: 2 ** 53 - 1 }` with no elements is ended by the embedder's
  stop rather than run to its end; a `NodeList`'s five members are `===`
  `Array.prototype`'s; and `alo-downloads`' script runs past `.forEach`
  and stops at its next missing member (`fetch`, item 75), named in
  `origin.txt`.
  **Built (iteration 209).** `alo-js/src/builtin/for_each.rs`: a native
  keeping two slots (`len`, `k`), asking for the `length` getter (with the
  object as `this`), `ToPrimitive` of an object length, an element's
  getter and the callback, each at its own step; `HasProperty` is
  answered in place, so holes and inherited indices are as the
  specification says; the callback check comes after the length; the
  embedder's stop is asked on every pass. `builtin/array_like.rs` now
  holds `ToLength` and an index's key for the iterator, `exec` and
  `forEach` alike; `builtin::native_method` puts a native that keeps
  slots on a prototype. `alo-bindings`' `define::array_iteration` puts the
  realm's own `entries`, `keys`, `values`, `forEach` (enumerable) and
  `values` under `Symbol.iterator` (not enumerable) on `NodeList.prototype`
  — each checked to be the engine's native of that name — for every
  interface `Interface::iterates_as_an_array` names. Tests:
  `alo-js/tests/what_for_each_visits.rs` (nine tables, each both ways with
  `Heap::check` after; the stop from another thread over 2⁵³ − 1 holes),
  and `alo-bindings/tests/what_a_selector_finds.rs` (the four by `===`,
  `[Symbol.iterator]` by the heap, enumerability, the page's own walk).
  `alo-downloads` stops at "ReferenceError: 'fetch' is not defined (at
  script 1, line 19, column 9; called from script 1, line 17, column 7)";
  no reference moved. Checked by mutation: visiting holes fails the holes
  table, and without the stop the 2⁵³ − 1 walk ran past sixty seconds and
  was killed. *Left:* `for…of` over a `NodeList` reaches the array
  iterator's refusal of a getter `length` (231), and `DOMTokenList`'s
  iteration waits on its indexed getter (328).

- [x] **332. The slots a builtin keeps.** *Cut from 331 (ADR 0031 §§ 1–5).*
  `Native` gains a declared slot count (at most `bounds::KEPT_BY_A_BUILTIN`,
  eight); `wait` reserves that many `undefined`s on the stack directly
  above the arguments, counted against `bounds::VALUES_ON_THE_STACK`;
  `Waiting` carries the count and `answer_at` moves up by it; `Call` gains
  `kept(n)` and `keep(n, value)`, which read and write the stack itself
  through its barrier rather than a copy. A builtin that declares none has
  today's region exactly. `object/native.rs`' *a step is a number* note
  says what changed. *Depends on nothing. Closes when:* a test native that
  allocates an object, keeps it, asks for a call that allocates, and
  answers with the kept object answers the same object with the collector
  at every allocation; a number kept before a call reads back after it; a
  call it asks for never disturbs a slot; a throw from that call takes the
  slots down with the builtin and leaves the stack as a throw from a
  builtin with none does; a slot number past the count is
  `Internal::BuiltinIsWrong`; a native declaring nine is refused when the
  realm is furnished, and a test walks every builtin `alo-js` and
  `alo-bindings` install; and reserving slots past the stack's bound is the
  `RangeError` a deep recursion is.
  **Built (iteration 208).** `Native::keeping(n)` and `Native::kept()`;
  `bounds::KEPT_BY_A_BUILTIN` is eight. `Engine::wait` reserves the slots
  as `undefined` above the arguments after the calls check, refusing with
  the stack's `RangeError` when `height + n` passes
  `VALUES_ON_THE_STACK`; `Waiting` carries `kept`, `kept_at()` is above
  the arguments and `answer_at()` above the slots. `Call::kept`,
  `Call::keep` (through the stack's barrier) and `Call::kept_number`
  answer `Internal::BuiltinIsWrong` past the count or in a `Call` built
  by hand. `Objects::native` — where every builtin's function is made —
  refuses more than eight as `Refused::KeepsTooMuch`, which is
  `Internal::BuiltinIsWrong`; `wait` checks again for a cell made some
  other way. `Heap::cells` and `Objects::natives` let a test walk what a
  realm was furnished with. Tests: `alo-js/tests/what_a_builtin_keeps.rs`
  (every closing condition, each table with and without the collector at
  every allocation; the bound shown by the most arguments a bottom call
  can take differing by exactly eight between a builtin keeping none and
  one keeping eight) and
  `alo-bindings/tests/what_a_builtin_it_installs_keeps.rs` (every builtin
  after `install` and `introduce`). Checked by mutation: dropping the
  reservation from `answer_at` or from the bound check fails five tests,
  and a test body that keeps its object only after a second allocation
  fails under the stressed collector.

- [x] **326. The `User-Agent` header.** *Cut from 325 (ADR 0030 §§ 1–4,
  7).* This engine sends no `User-Agent` today (`csp_report.rs` says so on
  purpose). One file in `alo-net` composes ADR 0030's string and platform
  from the system the binary is built for, and refuses to compile for a
  system with no row in § 2's table. `write_request` and HTTP/2's header
  block send it unless the request already has a `User-Agent`, as
  `Accept-Encoding` is left to a caller who set it, and the Reporting API
  envelope in `csp_report.rs` carries it. *Depends on nothing. Closes
  when:* a request written for HTTP/1.1 and one for HTTP/2 each carry
  exactly one `User-Agent` equal to the composed string; a request that set
  its own keeps it and gains no second one; a test asserts the string has
  no other engine's token (`AppleWebKit`, `KHTML`, `Chrome`, `Safari`,
  `Gecko/`, `Firefox`) and no patch version; and a CSP report's
  `user_agent` is the same string.
  **Built (iteration 202).** `alo-net/src/user_agent.rs` holds § 2's three
  rows, chooses one by `target_os` and has a `compile_error!` for any other
  system; `user_agent()` and `platform()` are what 325 tells the renderer.
  `write_request` and HTTP/2's `fields_for` send it unless the request has a
  `User-Agent` under any spelling, and the Reporting API envelope carries it
  as `user_agent`. Tests: the unit tests check every row, not only this
  build's; `http_messages.rs` and `speaking_http_2.rs` check exactly one
  header and a caller's own kept; `a_violation_a_page_reports.rs` checks the
  envelope and the posted report's header.

- [ ] **75. Promises, `async`/`await`, generators and iterators.**
  *Depends on 72, 76.* **Item 230 took the iteration protocol `for…of` reads**
  — `GetIterator`, a step, `IteratorClose`, `%IteratorPrototype%` and the
  array iterator — so *iterators* here now means a generator, which is a
  suspended frame, `for await` and the async iterators, and the iterator
  helpers (item 73's library).
  **Item 232 built the job queue a promise reaction will wait in** — in the
  heap, run by `Engine::checkpoint` — so a promise here queues its reactions
  with `Engine::queue_job` rather than building a queue of its own.
  **ADR 0032 § 5 (iteration 212) cut the promise itself out as item 333**,
  which depends on 232 and 235 rather than on all of 76: a promise needs the
  job queue and the checkpoint after every task, both built, and nothing in
  233 or 234. What stays here keeps its dependency on 76: the combinators
  (`all`, `allSettled`, `race`, `any`), `async`/`await` and generators, which
  suspend a frame, and the async iterators.
  **Item 333 is built (iteration 213)**: a combinator here is a builtin over
  `then`, `Promise.resolve` and a job, which all exist; `await` is a
  reaction whose handler resumes a frame.

- [x] **333. A promise.** *Cut from 75 by ADR 0032 § 5 (iteration 212).
  Depends on 232 and 235, both done.* The `Promise` constructor and its
  executor; `then`, `catch` and `finally` on `Promise.prototype`;
  `Promise.resolve` and `Promise.reject`; resolution by a thenable through a
  `NewPromiseResolveThenableJob` of its own; resolving a promise with itself a
  `TypeError`; a promise a cell in the heap holding its state, its value and
  its two lists of reactions, each reaction queued with `Engine::queue_job`;
  and a rejection still unhandled at the end of a checkpoint reported to the
  embedder as an uncaught throw is (item 241), and never reported if a
  handler is attached before that checkpoint ends. It is the promise `fetch`
  answers (items 334 and 335) and nothing else of 75. *Closes when:* a table
  of interleaved `then`, `queueMicrotask` and thenable resolutions runs in
  the order the specification gives — `Promise.resolve().then(a)` before a
  `queueMicrotask(b)` queued after it, a thenable adopted one job later than
  a plain value, `finally` passing the value through — ordinarily and with
  the collector at every allocation; an executor that throws rejects; a
  reaction that throws rejects the promise `then` made; an unhandled
  rejection is reported once and a handled one never; and every prefix cut
  of a script that makes promises is refused or run, never a panic.
  **Built (iteration 213).** `object/promise.rs` is the cell
  (`Cell::Promise`: state, result, one list of reactions with both
  handlers, `[[PromiseIsHandled]]`), made by `Objects::promise` and
  `Instance::Promise`. A builtin's *function object* may hold one value
  (`Objects::native_holding`, read with `Call::held`), the question ADR
  0031 left to where it is built: a resolving pair holds a two-value record
  (the promise and `[[AlreadyResolved]]`), `thenFinally`/`catchFinally` hold
  `onFinally`, the thunks hold what they answer. Two new asks:
  `Want::Catch`, a call whose uncaught throw stops at the builtin and is its
  answer with `Call::threw` (`catch.rs`'s `hand_back`, beside
  `set_aside`), and `Want::Settle`, which `interpret/settle.rs` does —
  write the state, push a `%PromiseReactionJob%` per reaction with
  `Jobs::push`, and remember a rejection with nothing handling it in
  `Rejections`, a rooted list `Engine::checkpoint` walks after its jobs,
  reporting each still unhandled through the same report with no calls
  left (`Drained::unhandled`); `abandon` forgets them with the jobs. The
  builtins are `builtin/promise.rs` (constructor, statics,
  `get [Symbol.species]`, furnishing), `promise_then.rs` (`then`, `catch`,
  `SpeciesConstructor`), `promise_finally.rs`, `promise_resolving.rs`
  (the pair and `%ResolvePromise%`, the one resolve procedure) and
  `promise_job.rs` (the reaction and thenable jobs). `Symbol.species` is a
  third well-known symbol; `builtin::error::made` makes an error object for
  a rejection and now also for `catch.rs`. A constructor other than
  `Promise` is refused as `Missing::APromiseOfAnotherConstructor`, cut as
  **item 337**. Tests: `alo-js/tests/what_a_promise_does.rs` (every closing
  condition; each table ordinarily and with the collector at every
  allocation; every prefix cut of a promise script, both ways). Checked by
  mutation: not holding `resolve` while `reject` is made, and keeping
  `thenFinally` only after `catchFinally` is made, each fail the stressed
  runs.

- [ ] **337. A promise made by a constructor other than `Promise`.** *Cut
  from 333 (iteration 213). Depends on 333, done; a subclass written with
  `class … extends Promise` also needs 223.* `NewPromiseCapability(C)` for
  any constructor `C`: construct it with a
  `GetCapabilitiesExecutor` closure that records the `resolve` and `reject`
  it is handed, refuse a second call of it and a capability without two
  callable functions with the `TypeError`s the specification gives, and
  resolve or reject through the recorded functions. `then`, `finally`,
  `Promise.resolve` and `Promise.reject` stop refusing
  `Missing::APromiseOfAnotherConstructor`. *Opened by:* a page that
  subclasses `Promise` or sets `Symbol.species` on one, frozen in the
  corpus. *Closes when:* `then` on a promise whose species is a script's
  constructor answers that constructor's object and calls its executor
  once; `Promise.resolve.call(C, x)` and `Promise.reject.call(C, r)` do
  the same; an executor called twice, or handing back something not
  callable, throws the specification's `TypeError`; and the tables of
  `what_a_promise_does.rs` still answer the same, with the collector at
  every allocation.

- [ ] **76. The event loop** — tasks, microtasks, the rendering steps,
  `requestAnimationFrame`. `ROADMAP.md`: *"where 'it works, but the animation
  stutters' is decided."*
  *Depends on 72. Closes when:* the order of a table of interleaved tasks and
  microtasks is what the specification says, because that order is observable to
  every script on every page.
  **Needs ADR, and it is written: ADR 0016, accepted** (iteration 132). The
  item could not name the decision it implements — ADR 0013 left *the task
  boundary* to it, ADR 0014 *where a safepoint falls*, and ADR 0012 § 4 the
  edge of the agent's window — so the decision came first, as `LOOP.md`
  stage 2 § 4 asks. In short: the loop lives in `alo-renderer`; the **job
  queue is `alo-js`'s**, in the heap, and `queueMicrotask` asks the engine to
  queue one; a task is one `ToRenderer` message or one thing the renderer
  scheduled (a timer, a finaliser's cleanup, every response); the next task is
  the oldest; a microtask checkpoint follows every task and every call that
  leaves nothing running, never nests, and ends the job (`Heap::end_job`); a
  frame is a message from the browser process carrying its time; an `Act` is
  answered only after its task's checkpoint; a stopped task stops the page.
  **No code is built and this item is not done.** Its trigger is the frozen
  service worker: running it stops at `self` (item 91), which depends on this
  item and on 83, and every handler it registers runs as a task. The first
  code cut is the job queue, the checkpoint and the task order, closed by the
  table above; the rendering steps and `requestAnimationFrame` may be cut
  from it if the iteration that takes it finds them a second item.
  **Iteration 133 cut it three ways**, by owner, as ADR 0016 § 1 draws the
  line: the engine's half is **item 232**, built; the renderer's loop is
  **item 233**; the rendering steps and `requestAnimationFrame` are **item
  234**. This item closes when 233 and 234 have, against its own table.
  **Iteration 134 cut 233 again**: the loop itself is **item 235**, built.
  **Iteration 135 cut 233 a third time**: the `Renderer` holding a loop per
  page and running a page's own scripts at load under its policy is **item
  236**, built.

- [x] **232. The engine's half of the event loop: the job queue and the
  checkpoint.** *Cut from 76 (iteration 133); ADR 0016 §§ 1, 3, 4 and 7.*
  `alo-js` holds the job queue **in the heap** (`job.rs`: one `Slots` cell
  rooted for the engine's life, jobs back to back, compacted as it is run);
  `Engine::queue_job` and a builtin's `Want::Job` are the one way in, and a
  callee that is not a function is the `TypeError` `queueMicrotask` throws,
  where it was queued; `Engine::checkpoint` runs jobs oldest first **including
  those jobs queue**, reports a throw to the embedder as it happens and runs
  the next, and on any other escape — `Stop`, a full heap, a refusal, a bug —
  drops the queue (§ 7); it ends the job (`Heap::end_job`) either way; and
  `Engine::call` runs a function with nothing running, which is how a loop
  calls a listener or a timer's callback. The checkpoint never nests because
  it holds `&mut Engine` for its whole length and a builtin is handed no
  engine — the borrow is the specification's flag.
  *Closed by:* `crates/alo-js/tests/what_a_checkpoint_runs.rs` — a job runs
  after its task and before the next; oldest first; jobs a job queued join
  the same checkpoint behind those waiting; a thousand-long chain is one
  checkpoint; a job's `this`; a throw reported and the next job run; a job's
  own `catch`; the `TypeError` for a non-function; **a checkpoint after each
  callback the loop calls but not after each a script calls** (ADR 0016 § 3's
  person's-click-versus-`element.click()` row, `1a2b` against `12ab`); an
  embedder's call with `this`, arguments, a throw, a builtin, a non-function
  and a runaway recursion; an embedder's job whose only reference is the
  queue surviving a collection; the last run's value kept across a
  checkpoint; the job ended, finished or stopped; a stopped checkpoint
  dropping its jobs; an endless job and an endless requeue stopped from
  another thread; live cells equal after three checkpoints of two thousand
  jobs; every prefix cut of a program that queues jobs. Each table runs
  ordinarily and with the collector at every allocation.
  `queueMicrotask` itself is HTML's, and an embedder's to define (ADR 0016
  § 1): the test defines it the way item 233 will.

- [ ] **233. The renderer's event loop: tasks, their order, and the
  checkpoint after each.** *Cut from 76 (iteration 133); ADR 0016 §§ 1–3, 6
  and 7. Depends on 232.* `alo-renderer` gains its loop: one sequence number
  across every task queue, the oldest due task next; each `ToRenderer`
  message is one task and an `Act` is answered only after its checkpoint; a
  task holding script holds it by a `Root`, released when it has run or been
  dropped; a checkpoint after every task and after every call the loop makes
  with nothing else running; the quiet point between tasks as the only place
  the loop asks for a collection and queues finaliser cleanups; a stopped task
  stops the page, its queues dropped and its roots released. `queueMicrotask`
  is installed on the global object here. The renderer runs no script today,
  so this is also where it first holds an `Engine`.
  *Closes when:* item 76's table of interleaved tasks and microtasks — in
  the order the specification gives — passes through the renderer's own
  loop, with `Act`'s answer arriving after its jobs.
  **Iteration 134 cut the loop itself out as item 235**, built: the task
  queue, its order, the root per task, the checkpoint after every piece of
  script, `queueMicrotask`, the quiet point checked and a stopped page. What
  is left here is **the `Renderer` holding it**, and it has two questions the
  loop did not: (1) `Renderer::handle` answers each message synchronously,
  so a task the page queues for itself has no idle moment to run in, and
  ADR 0016 § 6 forbids running it inside an `Act`'s window — so the
  boundary needs a way for the browser process to let a renderer run its own
  due tasks; and (2) **the only task that can carry a page's script today is
  the page's own `<script>` elements at load**, and running those obliges
  the renderer to know the page's `Content-Security-Policy` (item 165), which
  `Page` does not carry — running a page's inline script without its policy
  would be running script its author forbade. No timer (92), event (81) or
  response (83) exists to carry script otherwise. Both are the next cut's to
  settle, before `Act` can be shown answered after its jobs.
  *Depends on 235.*
  **Iteration 135 settled (2) and cut it out as item 236**, built: the
  `Renderer` holds one loop per page, and a page's own inline classic scripts
  run at load under its policy. What is left here is (1) — the loop running
  between messages for tasks a page queues for itself — and `Act` answered
  after its checkpoint, which needs a script that runs in an `Act`'s task:
  a listener (item 81) or a timer (92). Neither exists, so neither question
  has a test that could close it yet. *Depends on 81 or 92 for its closing
  condition.*
  **Iteration 155 (item 256) answers an `Act` after its task**: an agent's
  `Activate` on a page that runs script is a task whose listeners run with
  a checkpoint after each, and the renderer answers only once it has run
  (`an_agents_click.rs` sees a listener's microtask run before `input`).
  What is left here is (1), the loop running between messages, and item
  76's table through it.

- [x] **235. The renderer's event loop itself: tasks, their order, and the
  checkpoint after each piece of script.** *Cut from 233 (iteration 134);
  ADR 0016 §§ 1–4 and 7. Depends on 232.* `alo-renderer/src/event_loop.rs`
  owns an `Engine` with `queueMicrotask` on its global object
  (`event_loop/microtask.rs`, a builtin asking for `Want::Job`, made with
  the new `Engine::function`). `event_loop/task.rs` is the task queue: one
  sequence number across everything, oldest first, and a task that calls
  script holds `this`, its arguments and its callees **in one heap list
  under one `Root`**, released when it has run or been dropped. A task is a
  classic script's text or a list of callees called in turn with the same
  arguments — a dispatch to listeners — and **a microtask checkpoint follows
  every piece**. A throw nothing caught is a `Report` and the loop runs on,
  as are a script that does not parse and one the engine will not compile;
  anything else — the embedder's `Stop` (read before every piece, since a
  straight-line script never reads it), a full heap, a thing not built, a bug
  — stops the page: tasks dropped and their roots released, jobs dropped with
  the new `Engine::abandon`, nothing more queued or run. The quiet point after
  each task is checked (no open scope, nothing kept) and a noisy one stops
  the page. Asking for a collection there and queueing finaliser cleanups
  have no reason or registry yet (item 73), and no ceiling on waiting tasks
  is set because nothing a page controls queues one yet (92 brings the first).
  *Closed by:* `crates/alo-renderer/tests/what_the_event_loop_runs.rs` —
  item 76's table through the renderer's loop: a job after its task and
  before the next, jobs oldest first and jobs queued by jobs in the same
  checkpoint, tasks oldest first with their numbers, `1a2b` for a dispatch to
  two listeners against `12ab` for one script calling both, listener order,
  `this` and the argument, throws in a task, a job and a listener reported
  with the next run, `queueMicrotask(1)`, a script that does not parse, a
  callee that is not a function; a waiting task's argument surviving a
  collection and let go after it ran; fifteen hundred tasks of two listeners
  leaking no cell; a page stopped while idle, mid-task from another thread
  and in an endless requeue, with tasks, jobs and roots dropped; a noisy
  quiet point; every prefix cut of a script that queues. Each table runs
  ordinarily and with the collector at every allocation.

- [x] **236. A page's own scripts at load, under its policy, through its
  loop.** *Cut from 233 (iteration 135); ADR 0016 §§ 1, 2, 3 and 7; CSP as
  item 165 built it. Depends on 235 and 165.* `alo-dom/src/scripts.rs` reads
  what a document carries — HTML `<script>` elements and `<meta
  http-equiv="Content-Security-Policy">` in `<head>`, in one list in document
  order — with HTML's *prepare the script element* rules for the type
  (`type`, then `language`, JavaScript MIME type essences without
  parameters, `module`, `importmap`, everything else a data block), `nomodule`
  skipped, `src` kept as written, a `<template>`'s and SVG's scripts left out;
  and a script's nonce only where CSP's *is element nonceable* allows (no
  `<script`/`<style` in an attribute's name or value, no repeated attribute —
  which the parser's flag now reaches `Element::had_duplicate_attributes`
  for). `Page::policies` carries every enforced `Content-Security-Policy`
  header across the boundary (and the wire), parsed in the renderer by
  `alo-net`'s own rules. `alo-renderer/src/scripts.rs` asks each inline
  classic script of the response's policies and every `<meta>` policy before
  it, runs the allowed ones as one task each through a per-page `EventLoop`,
  and says everything else in `Loaded`'s issues: a refusal in the policy's
  words, a fetched script (238), a module or import map (77), each throw and
  report, and every script after one that stopped the page. A `Resize` lays
  out again and runs nothing; a new `Load` drops the last page's loop.
  *Closed by:* `crates/alo-renderer/tests/a_page_runs_its_scripts.rs`,
  twenty-one tests — document order with each script's jobs (and jobs'
  jobs) before the next; throws in a script and a job reported, a script
  that does not parse, the next script running; a stopped page stopping
  every later script; fetched, module and import-map scripts said; data
  blocks, `nomodule` and a template's script not run; a policy forbidding
  inline script by `script-src`, `default-src`, `'none'` and a nonce
  retiring `'unsafe-inline'`; `'unsafe-inline'`, a hash, an unrelated
  directive allowing it; a hash of other text refused; two policies
  intersected; a nonce letting in only its scripts; four injected-markup
  shapes refused; a `<meta>` governing only what follows it, narrowing the
  header and never widening it, and not counting outside `<head>`; an
  unreadable source refusing; a resize running nothing (the realm marked);
  a new page's fresh realm and no loop for a page without script; the page
  still laid out, in numbers, when its script throws; every prefix cut of a
  hostile page loading; and an endless script through the real renderer
  binary given up on within its bound while another site's renderer and the
  same site's next load work. `alo-dom`'s `scripts.rs` has eleven unit tests;
  the wire round trip carries two policies.

- [x] **237. A report-only policy told about inline script it would have
  refused.** *Cut from 236.* The renderer obeys only enforced policies, and
  `Page` carries only those: a `Content-Security-Policy-Report-Only` forbids
  nothing, and the violation report it asks for has to be posted, which a
  renderer cannot do (ADR 0005). `Policies::inline_violations` already makes
  the violation; what is missing is the renderer saying it and the browser
  process posting it (item 188's `Pool::report`). *Depends on 236 and 188.
  Closes when:* a page loaded under a report-only `script-src 'none'` runs
  its inline script and the browser process posts one report naming
  `script-src` and `inline`, and an enforced refusal is reported the same
  way.
  **Done (iteration 136), both clauses.** `Page::watching` carries the
  report-only headers (and the wire), and `Page::stated` is the one list —
  every header policy, both dispositions, in `Policies::stated_by`'s order —
  that a violation is named against. The renderer asks each inline classic
  script of that list (`Policies::objecting_to_inline`) and answers
  `Loaded::objections`: a policy's **place** and the content's kind, never a
  report — a renderer is the process a hostile page may be steering, and a
  report it wrote could name any collector. The browser process's half is
  `alo-renderer/src/violations.rs`: `reports(page, about, objections)` writes
  each report from its own copy of the headers with
  `Policies::inline_violation_of`, which builds the violation from the policy
  alone (nothing in an inline refusal depends on the content —
  `Policy::refuses_some_inline` was factored out of `objects_to_inline` to say
  so) and answers `None` for a place with no policy or a policy that lets every
  inline script in, which is said as *disbelieved* rather than posted. At most
  `MOST_OBJECTIONS` (64) per load: the renderer stops there and says how many
  it left out, the wire refuses a load claiming more, and `reports` takes no
  more. A watched objection to a script that runs is also said in the issues.
  A `<meta>` policy's objections do not cross (CSP drops `report-uri` from
  markup, and the browser process has not seen the markup): item 240.
  *Closed by:* `crates/alo-renderer/tests/a_policys_author_is_told.rs`, ten
  tests — the two closing clauses through the real `alo-render` binary in a
  tab, `violations::reports` and `Pool::report` to a loopback collector
  (`script-src`, `inline`, `report` / `enforce`, the document's URL), with the
  script's running asserted in-process; `report-to` resolved against the
  response's own `Reporting-Endpoints`; only the objecting policy named; nonce
  and hash allowed scripts not objected to; each script and each policy its
  own objection; a `<meta>` refusal obeyed and not passed on; no objection
  after the page stopped; seventy scripts carrying 64 objections and saying
  six left out; every prefix cut of a page with three policies objecting only
  to policies the browser process believes. `violations.rs` has four unit
  tests (the browser's own copy, a claim that could not have happened, the
  flood, a page with no policy), `csp.rs` three (places agree with
  `inline_violations`, a policy that could not have objected, a hash's part
  in a refusal written from a place), the wire two (a flood refused whole, a
  strange kind tag and trailing bytes refused) and its round trips carry
  `watching` and objections.

- [ ] **240. A `<meta>` policy's `report-to`.** *Cut from 237.* CSP removes
  `report-uri`, `frame-ancestors` and `sandbox` from a policy delivered in a
  `<meta>` element, but not `report-to`, so a page's markup can still ask for
  violations to be reported to a group its response's `Reporting-Endpoints`
  defined. The browser process has never seen the markup, so it cannot name
  such a policy by place the way item 237 names a header's, and a policy text
  the renderer sends is the page talking. Deciding what the browser process may
  accept — the policy's text as `original-policy`, an endpoint only from its
  own headers — is this item. *Depends on 237. Closes when:* a page whose
  `<meta>` policy says `script-src 'none'; report-to g` under a response
  defining `g` gets one report posted to `g`'s URL, a `<meta>` `report-uri` is
  never posted to, and a group the response did not define posts nothing.
  Opened by a frozen page that does it, and not before.

- [ ] **238. A page's fetched classic scripts.** *Cut from 236.* A `<script
  src>` is said not to have run, because a renderer cannot fetch and nothing
  hands it the text. The browser process fetching each one — under the
  page's policy with the element's nonce, mixed-content and CORS rules for
  `crossorigin` — and the renderer running them in HTML's order, with
  `defer` and `async` meaning what they say, is this item. *Depends on 236
  and 53; opened by a frozen page whose script is fetched. Closes when:* a
  frozen page's linked script runs in document order between its inline
  ones, one its policy refuses is not fetched at all, and a fetch that fails
  is said and the next script runs.

- [x] **239. A thrown error object said by its name and message.** *Cut from
  236.* A `TypeError` a script made and nothing caught is reported as
  `uncaught: an object`, because `Report::thrown` may not run script and the
  engine has no way to read a property without possibly calling a getter.
  An error object (`Objects::is_error`) whose `name` and `message` are data
  properties on it or its prototype chain can be described without a call;
  one whose are accessors says so. *Depends on 235 and 227. Closes when:* an
  uncaught `new TypeError('x')` is reported `TypeError: x`, a subclass-like
  object whose `name` was reassigned uses the new name, and a `message`
  getter is never called.
  **Done (iteration 137), all three clauses.** `alo-renderer`'s
  `event_loop/described.rs` puts a thrown value into words reading the heap
  and nothing else. An object with `[[ErrorData]]` is said as
  `Error.prototype.toString` would say it — `name` (default `Error`), `": "`,
  `message` (default empty), either left out when empty — with both read
  along the prototype chain through `Objects::existing_key` and
  `Objects::get`, so describing interns nothing and calls nothing. A getter,
  an object (whose `toString` would run) and a symbol (which `ToString`
  refuses) are each said in brackets instead, and a page's own
  `Error.prototype.toString` is not consulted. Any other object is still
  `an object`. Every string a page made — a thrown string, a name, a message —
  is cut at `LONGEST_SAID` (1024) code units, never half a surrogate pair,
  with the rest counted, because a page's strings run to some 268 million
  units and a load's whole answer crosses the wire in one bounded message.
  *Closed by:* `crates/alo-renderer/tests/an_error_said_by_its_name.rs`,
  fifteen tests through a real `Renderer` — `TypeError: x`; a reassigned
  `name` on the instance and on a prototype; a `message` getter and a `name`
  getter never run (a counter read back from the page's engine stays 0); an
  object message's `toString` never run; all seven constructors; one made
  without `new`; empty name, empty message and both empty; a name deleted
  everywhere; number and boolean parts; a replaced
  `Error.prototype.toString` not run; a look-alike plain object still `an
  object`; an engine-thrown `TypeError` caught and rethrown; one thrown from
  a job; a 2^13-unit message and thrown string cut with 7168 counted.
  `described.rs` has six unit tests (cuts, surrogate pairs, a symbol name
  made in the heap — there is no `Symbol` global yet — and a heap where
  nothing has a `name`, which stays uninterned). Two assertions in
  `a_page_runs_its_scripts.rs` that said `an object` now say the error.

- [ ] **234. The rendering steps and `requestAnimationFrame`.** *Cut from 76
  (iteration 133); ADR 0016 § 5. Depends on 233.* A frame is a message from
  the browser process carrying its time; after the current task and its
  checkpoint the loop runs the `requestAnimationFrame` callbacks registered
  before the frame began, in registration order, each handed that time and
  each followed by a checkpoint, then style, layout and paint. A callback
  registered during a frame waits for the next. No claim about frame rate.
  *Closes when:* a test sends two frames with given times and the callbacks
  and their jobs run in the specification's order with those times, and a
  callback registered inside a frame runs in the second.

- [ ] **77. Modules**: ESM, dynamic `import()`, and the loader that fetches them.
  *Depends on 53, 72.* **Needs design** (iteration 138, as `LOOP.md` step 2
  asks of an item that cannot name its contract): it has no closing
  condition, and a loader is a decision no ADR has made — a renderer cannot
  fetch (ADR 0005), so who fetches a module graph, under which policy and
  CORS mode, and what crosses the boundary is the same question item 238
  waits on for a classic script, with linking and a module map per realm on
  top. Nothing reaches it either: no corpus page has a module script, and the
  frozen `alo-theme-generator` is a Node program (`node:fs`), not a page's.

- [ ] **78. Errors and stack traces** good enough to debug somebody else's
  minified page.
  *Depends on 72.* **Iteration 138 cut item 241 from it**, built: an uncaught
  throw is reported with the script, line and column of the throw and of
  every call it left. What is left here: `error.stack` — a trace taken when an
  error is **made**, which a page can read and a rethrow does not change —
  and the function names in it (item 220); a builtin named as a call in a
  trace; source maps; and the browser process showing a person any of this
  (developer tools, item 129).

- [x] **241. An uncaught throw placed: script, line and column, and the calls
  it left.** *Cut from 78 (iteration 138). Depends on 72 and 235.* A throw no
  `try` caught is reported by **where** as well as what: the script, line and
  column of the throw, then of each call it unwound through on its way out,
  innermost first. `alo-js`'s `interpret/unwound.rs`: when `land` finds
  nothing guarding a throw — the last moment the calls it left exist — it
  reads every frame, innermost first, as a `Place` (the `Rc<Unit>` and the
  byte offset of the instruction the frame was on, `Frame::now`, so a throw
  from a `valueOf` an operator rewound for is placed at the operator), at
  most `bounds::PLACES_IN_A_TRACE` (32) with the rest counted.
  `Engine::unwound()` answers it after a run, call or job answered a throw;
  every run starts from nothing (`two_lists`), so a caught throw and the next
  run leave nothing; `Engine::checkpoint`'s report is handed it with each
  job's throw. `alo-renderer`'s `event_loop/source.rs`: the loop compiles each
  script itself and keeps it, under the name its embedder gave
  (`EventLoop::queue_script(name, text)`; a page's are `script N`, numbered
  as the load's issues number them), so a function one script declared is
  placed in that script when another calls it. A line and column are
  `alo_js::Position`'s — lines ended by every ECMAScript line terminator,
  columns in UTF-16 code units from one — counted from the nearest of marks
  laid every 4096 bytes when the script is kept, so a page throwing in a loop
  cannot make the renderer re-read a one-line bundle from the start for every
  place. A program the loop did not compile is said by its byte offset. The
  report reads `uncaught: Error: e (at script 2, line 3, column 5; called
  from script 1, line 1, column 9; and 4 calls further out)`; a throw no call
  was entered for (a callee that is not a function) is placed nowhere rather
  than somewhere invented.
  *Closes when:* a page's throw on a script's third line is placed there; a
  function one script declared and another called is placed in the first and
  called from the second; a column in a one-line script tens of kilobytes long
  is right in UTF-16 code units; and a runaway recursion says 32 places and
  how many more.
  *Closed by:* `crates/alo-renderer/tests/where_a_page_threw.rs`, twelve
  tests through a real `Renderer` — the four clauses (line 3 column 5; script
  1 line 2 column 10 called from script 2 line 1 column 9; column 20005 of a
  20-kilobyte line; 32 places at line 2 column 3 and `10208 calls further
  out`), a column that is 16 in code units where bytes would say 19 and
  characters 15, `\r\n`, `\r`, U+2028 and U+2029 each ending a line, a
  script refused by its policy still counted so names agree with the page, a
  job placed in the function that was queued, a rethrow placed at the
  rethrow, a builtin's throw placed at the call that entered it, a script
  that did not parse not placed, and every prefix of a page throwing from
  deep calls in a script and a job. `crates/alo-js/tests/where_a_throw_was.rs`,
  eleven tests of the engine half with offsets counted by hand — the throw,
  the waiting calls, the recursion's 32 + 10208 = `CALLS_ON_THE_STACK`, a
  caught throw and the next run leaving nothing, two programs told apart by
  identity, engine-thrown errors whose innermost place is the throw's own
  offset, a `valueOf` and a getter, an embedder's call, a non-function callee
  placed nowhere, and a job's throw handed to the checkpoint's report.
  `source.rs` has five unit tests (marks against counting from the start
  across every kind of line ending and character width, a mark never
  splitting `\r\n`, a one-line bundle, an unknown program) and `report.rs`
  one (the trace's words).

- [x] **242. A ceiling on what one load says about its scripts.** *Found
  while building 241.* Each uncaught throw is one line in a load's issues,
  and a page can queue as many throwing jobs as it likes in one load: nothing
  counts the lines, and the whole answer crosses the wire in one message
  capped at 64 MiB (`wire::LARGEST_MESSAGE`). Past the cap the renderer
  cannot send its answer and the tab sees a renderer that failed — safe, but
  the page's every issue is lost and the reason is not said. Item 239 cut
  each string a report repeats to 1024 code units and 241 bounds a trace at
  32 places, so one report is bounded; how many is this item: a ceiling on
  issues per load, with how many more there were said, as
  `MOST_OBJECTIONS` already does for objections. *Depends on 236. Closes
  when:* a page queueing a hundred thousand throwing jobs loads, says the
  ceiling's worth, and says how many it left out.
  **Done (iteration 139).** `alo-renderer/src/scripts.rs`: a load says at
  most `MOST_SAID` (256) lines about its scripts — reports, refusals, scripts
  not run, all of them — and then `N more things about this page's scripts
  were not said: one load says at most 256`. The ceiling is on what is said,
  never on what runs. It also had to hold **inside one turn**, which the item
  did not name: a job that throws and requeues itself for ever made one
  `Turn` grow by a described, placed report per job until the page was
  stopped. So `alo-renderer/src/event_loop.rs` keeps at most `MOST_REPORTS`
  (256) per turn and counts the rest in `Turn::unreported` **before**
  describing them; `EventLoop::run_next_within(room)` lets the load hand a
  turn only what room it has left.
  *Closed by:* `crates/alo-renderer/tests/what_one_load_says.rs`, eight
  tests — the closing clause through a real `Renderer` (256 lines, the first
  256 throws in order and each placed, `99744 more`, all 100000 jobs run, the
  answer under `LARGEST_MESSAGE` and round-tripping the wire); the ceiling
  across 300 scripts (`script 2` to `script 257`, `44 more`, all run);
  300 fetched scripts' "not run" lines counted the same way; exactly 256
  throws said whole with no count; every prefix of a page throwing 400 jobs
  within the ceiling; and, on the loop, a turn's 256 kept and 744 counted with
  1000 jobs run, a room of 3 and of 0 (11 counted, 10 jobs run) and of
  `usize::MAX` capped at 256, and a job throwing and requeueing itself for ever,
  stopped from another thread, keeping 256 and counting the rest.

- [x] **243. A ceiling on what a page's markup makes one load say.** *Found
  while building 242, by reading, not yet by a run.* The markup half of a
  load's issues (`pipeline::Rendered::issues` — the document's, the sheets',
  each picture's, the box tree's and layout's) has no ceiling either, and it
  amplifies: `<img>` is five bytes of page and `an <img> with no src` is
  twenty-eight bytes of answer with its eight-byte length, so a page of
  about eleven and a half megabytes of them — well under the 64 MiB cap the
  page itself crossed the wire in — would make an answer the wire refuses. *Depends on 242. Closes when:* a
  page of a few million `<img>` elements loads and says a ceiling's worth of
  them and how many more, under `LARGEST_MESSAGE`.
  **Built** in `alo-renderer/src/said.rs`: `of_markup` says at most
  `MOST_SAID_OF_MARKUP` (256) of what the markup made the engine say and then
  how many more, writing out only the lines said
  (`Rendered::each_issue` yields them unformatted). **And a second clause the
  item did not name, and why it is here:** a ceiling on how many lines is no
  ceiling while one line can be any length. A line quoting what the page
  wrote quotes it escaped — `\u{1}` is five characters for one — so one
  `<img>` whose `src` is fourteen million control characters said a line of
  seventy million, past the cap alone; and the scripts' half had the same
  hole (a fetched script's `src`), which 242's "each line is bounded" had
  missed. So `said::line` keeps every line of a load's report, both halves,
  to `LONGEST_LINE` (8192) characters, counting the rest as they are written
  rather than keeping them — long enough that no report of a throw (two
  strings of 1024 and 32 places) is ever cut.
  *Closed by:* `crates/alo-renderer/tests/what_a_pages_markup_says.rs`, eight
  tests — the closing clause through a real `Renderer` (2,500,000 `<img>`:
  the first 256 lines, then `2499744 more`, the answer under
  `LARGEST_MESSAGE` and round-tripping the wire); 300 counted and exactly
  256 said whole; a picture's `src` and a script's `src` of fourteen million
  control characters each said as one line cut at 8192 characters with the
  rest counted, and the answer sendable; markup and scripts each to their own
  ceiling in one load; a resize bounded the same; every prefix of a page
  saying more than the ceiling. `said.rs` four unit tests. Three doctored runs, each restored byte for
  byte: the count ceiling removed fails five, the line ceiling removed fails
  the two long-line tests, the scripts' lines left unbounded fails the
  script one.

- [x] **244. The font names a load asks for, bounded in length.** *Found while
  building 243, by reading, not yet by a run.* `FromRenderer::Loaded`'s
  `wanted` is at most `families::MOST_WANTED` (64) names, but each name is
  as long as the page wrote it, so a page near the wire's cap that names one
  enormous family can make an answer of itself plus the issues' bounded
  sixteen megabytes, which the wire refuses. Not an amplification — a name
  is never longer than the page wrote it — so only a page already close to
  the cap reaches it. What to do with a name longer than any font has is a
  choice (not asked for, or asked for cut, which would ask for a different
  font), and it is the families module's. *Depends on 243. Closes when:* a
  page naming a family of tens of megabytes loads and its answer crosses the
  wire, with what was not asked for said.
  **Done (iteration 141).** First found by a run: a page of 66479993 bytes
  (under the wire's 67108864) naming a family of 63 MiB beside 254 pictures
  each saying the longest line a load says made an answer of 68167392 bytes.
  **The choice: not asked for, and said.** No font this engine reads states
  a family longer than `alo_text::LONGEST_NAME` (512) characters — a `name`
  record of more bytes is skipped, and decoding never makes more characters
  than bytes — so `alo-renderer`'s `families::LONGEST_FAMILY` is that, and
  `families::could_be_a_family` asks it counting no further than one past.
  A longer name is left out of `Wanted::families` and said in the new
  `Wanted::not_asked` (`a family beginning "…" and N characters long was not
  asked for: …`, its first 32 characters quoted), chained into the issues
  before the substitutions; a substitution names it the same way rather than
  quoting it whole. The page's next choice is still asked for. Asking for it
  cut was refused, because that asks for a different font. The browser
  process's `Renderers::supply` answers such a name absent without a look
  through the machine's fonts, bounding what a renderer sent as it already
  bounds how many.
  *Closed by:* `crates/alo-renderer/tests/a_family_no_font_could_have.rs`,
  five tests — the closing clause (that page now answers no family asked for,
  the not-asked line and the substitution said, under `LARGEST_MESSAGE` and
  round-tripping the wire); 512 `a` and 512 `é` (1024 bytes) asked for and
  513 `é` not; a too-long name skipped, `Inter` after it asked for; the
  browser process answering a 513-character and a 20-million-character name
  absent; every prefix of a page naming a 600-character family bounded and
  sendable. `families.rs` two unit tests. One doctored run, restored byte for
  byte: the renderer's check removed fails four of the five. **Not
  discriminated by a test:** the browser process's check — without it the
  name is still answered absent, after a look through every font file, so
  only the cost differs; recorded rather than given a seam to observe it.

- [ ] **79. `Intl`, rented** rather than written.
  *Depends on 73.*

## E. The DOM, and the pages that use it

- [x] **80. Mutation from script**, and the invalidation that has to follow it.
  Adding and removing nodes is still the parser's alone today (`alo-dom` says
  so); this is where that stops being true.
  *Depends on 72. Closes when:* a script changes a document and the next render
  shows it, with node identity surviving (ADR 0003).
  **Its decision is ADR 0017 (iteration 142)**, written first because ADR 0014
  left *the shape of the DOM bindings* to this item and a native function had
  no way to reach a document: the document moves into the page's heap as one
  rooted cell when the page first runs script; a wrapper is one per node and
  lives while its **tree** is reachable (the document's always, a detached
  tree through a ring of ephemerons over its wrappers), and an unreachable
  detached tree is freed with its ids left as tombstones, never reused; a
  native reaches its node only through its `this`, by a typed borrow the
  engine gains; every change goes through `alo-dom`'s own operations under the
  standard's validity rules and advances a change count; a changed document is
  rendered again whole when its rendering is read, never inside a task; and a
  parser-inserted script runs at its own end tag. No code yet. It closes when
  245, 246 and 247 have, and the cut is in that order.
  **Done (iteration 147)**, when 247 closed: 245, 246 (through 248, 249 and
  250) and 247 are all done, and the closing condition was met by item
  250 (iteration 146) — the corpus case `a-script-grows-a-list` and
  `what_a_script_left.rs`, with every parsed node keeping its id — and at
  the order 247 settled, `a-script-beside-itself`. Left as items of their
  own, not as this one's remainder: `document` as Web IDL's accessor (251),
  a thrown `DOMException` named in a load's report (252), `document.body`
  (253), and rendering only what changed (113).

- [x] **245. `alo-dom`'s tree operations, public, under the standard's
  rules.** *Cut from 80 (ADR 0017 §§ 3 and 5). Depends on nothing.* Insert,
  append, replace and remove become public under the DOM standard's names,
  with its pre-insertion validity checks answered as a refusal that names the
  standard's exception (`HierarchyRequestError`, `NotFoundError`), not `false`;
  creating an element or a text node takes the next id from the parser's
  counter; every change advances `Document`'s change count; and releasing a
  detached tree drops its nodes' contents while their ids answer nothing and
  are never handed out again (a tombstone). The parser keeps its own
  crate-private operations. The agent's `apply` uses the public ones. No
  engine. *Closes when:* every validity rule the standard lists for insert and
  replace is a test that refuses with the right name and leaves the tree
  unchanged; a created node after 40 parsed ones is `#40`; the count moves on
  every change and not on a refusal; and a released tree's ids answer `None`
  while the next created node's id is still one past the highest ever made.
  **Done (iteration 143).** `alo-dom` gains three files, one rule each:
  `validity.rs` (the standard's *ensure pre-insertion validity* and the
  checks of *replace a child*, rule for rule, host-including through a
  template's contents, plus `createElement`'s *valid element local name*;
  `Refusal` names `HierarchyRequestError`, `NotFoundError` and
  `InvalidCharacterError`), `mutation.rs` (`create_element`, lowercased, a
  `<template>` made with its contents numbered after it;
  `create_text_node`; `insert_before`, `append_child`, `replace_child`,
  `remove_child` and `remove`, a fragment giving up its children in order)
  and `release.rs` (a detached tree's root, never the document, a node with
  a parent or a template's contents, released with every template's
  contents inside it; slots become one-pointer tombstones). `Document` keeps
  the arena, now `Option<Box<Node>>` per slot, a `host` link from contents
  to template, and `change_count`, advanced once per successful insertion,
  removal, replacement and attribute change and never by a refusal, by
  making a node, by releasing, by removing an absent attribute or by the
  parser. The parser's operations stay crate-private, renamed
  `attach_last`/`attach_before` so the standard's names are the public
  ones; `element_mut` became crate-private so no change skips the count.
  The agent's `apply` already changed the document only through the public
  `set_attribute`/`remove_attribute`, which now count; it has no tree
  change to make.
  *Closed by:* `crates/alo-dom/tests/mutation.rs`, 20 tests — every
  insertion and replacement rule refusing by name with every link of every
  node, the serialisation, the node count and the change count unchanged;
  `#40` after a forty-node page; the count across ten operations (six
  changes); a released tree's ids answering nothing, refusing as
  `NotFoundError`, the next id one past the highest; and every id the page
  has, and three it never made, in every position of every operation on a
  cloned page, each refusing or leaving a tree
  whose links agree and whose document holds at most one doctype before at
  most one element and no text. Unit tests in `validity.rs` (names),
  `mutation.rs` (numbering, a template's contents) and `release.rs`. One
  doctored run per rule, each restored: every one of the nineteen refusals
  in `validity.rs` disabled alone fails a test — the first run found *a
  document goes nowhere* masked by the ancestor rule, and a case under a
  detached parent was added so it is not.

- [x] **246. The bindings: a script changes the document and the next render
  shows it.** *Cut from 80 (ADR 0017 §§ 1–6). Depends on 245.* The
  `alo-bindings` crate; the typed borrow of an embedder's own cell in
  `alo-js`; the document cell (rooted by the renderer, its footprint the
  document's size) and the wrapper with its table, its strong edges for
  attached nodes and its ephemeron ring for each detached tree, and the
  release of unreachable trees at the sweep; `document` on the global
  object, and item 80's members only — the document and its root element,
  `createElement`, `createTextNode`, `appendChild`, `insertBefore`,
  `removeChild`, `replaceChild`, `remove`, `textContent`,
  `getAttribute`/`setAttribute`/`removeAttribute`, `parentNode`,
  `firstChild`, `lastChild`, `nextSibling`, `previousSibling` — with the
  brand check's `TypeError` and `DOMException`; and the renderer borrowing
  the document from the cell and rendering again whole when the change count
  says what it holds is stale, at `Paint`, `ReadTree`, an `Act`'s decision,
  the end of a `Load` and a `Resize`. *Closes when* (item 80's own
  condition): a page's script appends an element and the next render's box
  tree and layout have it, in numbers, with a reference render; the agent
  names the node the script made and acts on it; every parsed node keeps its
  id; one node asked for twice is one object, and its expando survives a
  forced collection; a detached tree no script holds is freed at a
  collection and one a script holds is not, counted; and the hostile half —
  a page appending to itself in a loop, a million detached nodes, a node
  inserted into its own child — refuses or collects and never panics.
  **Cut (iteration 144)** into 248, 249 and 250, in that order, because it is
  three changes to three crates each with its own reason to be wrong: what a
  wrapper is and how long it lives (the clause a script observes, and the one
  every member stands on), the members a script calls, and the renderer
  handing its document over and rendering it again. It closes when they have,
  on its own condition above — which is 250's.
  **Done (iteration 146)**, by 248, 249 and 250 together: every clause of its
  condition is one of theirs — the render, the agent and the ids are 250's,
  one object and its expando 248's and 249's, the freed and held trees
  248's, and the hostile half 248's, 249's and 250's.

- [x] **248. The document in the heap, and a wrapper that lives as long as its
  tree.** *Cut from 246 (ADR 0017 §§ 2–4). Depends on 245.* The typed borrow
  of an embedder's own cell in `alo-js`; the `alo-bindings` crate with the
  document cell (its footprint the document's size, kept as a sum so a
  change does not cost a walk), the wrapper (node id, document, an ordinary
  object's part) and the table from node to wrapper; the cell tracing every
  attached node's wrapper strongly and each detached tree's wrappers as a ring
  of ephemerons, and at the sweep dropping dead wrappers and releasing every
  detached tree none of whose nodes still has one, allocating nothing. No
  script member, no renderer change. *Closes when:* one node wrapped twice is
  one object and its expando survives forced collections; a detached tree no
  wrapper holds is released at a collection and one held through any one of
  its wrappers is not, counted; a `<template>`'s contents are kept with their
  template; a node wrapped while every allocation collects is kept; the heap
  grows by exactly what the document does; and the hostile half — a ring
  wider than the marker's pair buffer, a chain too deep to recurse, a long
  run of operations in an order nobody chose — keeps exactly what is held and
  never panics.
  **Done (iteration 144).** `alo-js`: `Exotic` gains the supertrait `Typed`
  (blanket-implemented, so an embedder writes nothing — the workspace's
  `rust-version` predates `dyn` upcasting, so `Any` cannot be named
  directly), and `Objects::embedded::<T>` / `write_embedded::<T>` answer an
  embedder's own object by type and `None` for every other cell. `alo-dom`:
  `Document::footprint` is a sum kept as nodes are made, edited and
  tombstoned (`footprint.rs`; every content edit goes through one accounting
  helper, `element_mut` became `edit_element`); `release` walks down
  unlinking and climbs back by the parent link, so it allocates nothing and
  cannot recurse; `next_detached_root` is a cursor over detached roots for a
  caller that releases as it goes. `alo-bindings` (new; the only crate
  naming both): `document_cell.rs` (the cell, the table, the pending node,
  the released counts), `wrapper.rs`, `tree.rs` (the host-including
  pre-order walk, going round, with a step budget), `liveness.rs` (trace and
  sweep), `embed.rs` (`adopt`, `wrap`, `node_of`, `document`,
  `change_document`). **Two refinements of ADR 0017 § 3's mechanism, its
  rule unchanged:** the ring follows tree order and each tree is walked once
  per collection, rather than each wrapper asking for its root (a held chain
  a million deep would make that quadratic); and a node whose wrapper is
  being made is *pending*, its tree kept by the collection that allocation
  may cause — without it, `createElement`'s node would be released while its
  first wrapper was being made. A walk that overruns its budget keeps every
  wrapper and releases nothing.
  *Closed by:* `crates/alo-bindings/tests/what_a_wrapper_keeps.rs` (8 tests)
  and `a_document_that_is_hostile.rs` (4: a ring of 19384 wrappers held by
  its last, which overflows the marker's 16384 pairs and is kept whole by a
  rescan; a 200001-node chain held from its bottom and then released; a
  20000-link chain in the page, every link wrapped; 4000 seeded operations
  checked after every collection); `tree.rs` (3 unit tests);
  `crates/alo-js/tests/what_an_embedder_gets_back.rs` (3);
  `crates/alo-dom/tests/what_a_document_weighs.rs` (4, every footprint
  recounted by hand). Doctored runs, each restored and checked identical:
  thirteen rules disabled alone — attached wrappers strong, the ring's
  forward pairs, its closing pair, the release, the pending node, pruning
  the table, the document counted, a template's contents walked, an existing
  wrapper reused, the downcast through the box rather than of it, an edit
  weighed, a tombstone weighed — each fails at least one test. **Not
  discriminated:** the overrun fallback, which a document `alo-dom`'s
  validity rules allow cannot reach.

- [x] **249. The interfaces a script calls.** *Cut from 246 (ADR 0017 §§ 1, 4,
  5 and 8). Depends on 248.* One file per interface in `alo-bindings` —
  `Node`, `Element`, `Document`, `Text` — each with its prototype, its
  attributes as accessors whose halves are natives and its operations as
  native methods, and item 80's members only: the document and its root
  element, `createElement`, `createTextNode`, `appendChild`, `insertBefore`,
  `removeChild`, `replaceChild`, `remove`, `textContent`,
  `getAttribute`/`setAttribute`/`removeAttribute`, `parentNode`,
  `firstChild`, `lastChild`, `nextSibling`, `previousSibling`; the brand
  check's `TypeError` for a wrong `this`; `DOMException` with `name` and
  `message` and `Error.prototype` on its chain, made from `alo-dom`'s
  `Refusal`; and `document` on the global object of an engine given a
  document cell. Every other member absent (`typeof` answers `"undefined"`).
  *Closes when:* a script run by the engine against an adopted document
  makes, inserts, moves, replaces and removes nodes and reads them back
  through every member; each refusal is the named `DOMException` a `catch`
  receives; a member called on a plain object, on a `Text` where an
  `Element` is required, or on a wrapper of another document throws a
  `TypeError`; one node read twice through different members is one object;
  and the hostile half — a node inserted into its own child, a script
  appending to itself in a loop until the heap's ceiling, a million detached
  nodes made and dropped — refuses or collects and never panics.
  **Done (iteration 145).** `alo-bindings` gains `interface.rs` (the ten
  interfaces in the standard's chain — `Node`; `CharacterData` and its
  `Text`, `Comment` and `ProcessingInstruction`; `Element`, `Document`,
  `DocumentType`, `DocumentFragment`; `DOMException` from `Error.prototype`
  — which one a node of each kind is, and their prototypes as strong edges
  of the document cell, since a native reaches nothing but its `this`),
  one file per interface with members — `interface/node.rs` (the five
  neighbours, `textContent` both ways, the four changes),
  `interface/element.rs` (`getAttribute`, `setAttribute`,
  `removeAttribute`), `interface/document.rs` (`documentElement`,
  `createElement`, `createTextNode`), `interface/child_node.rs` (`remove()`
  on `Element`, `CharacterData` and `DocumentType`, as the mixin is) and
  `interface/dom_exception.rs` (the exception as an embedder cell, `name`
  and `message` as prototype getters, as Web IDL has them) — with
  `idl.rs` (the brand check, argument count, a `Node` argument of the same
  document, `ToString` asked of the interpreter for an object),
  `define.rs` (Web IDL's property attributes) and `install.rs` (`furnish`,
  and `install`, which puts `document` on the global object). `alo-dom`
  gains `by_name.rs` (attributes by qualified name, lowercased on an HTML
  element, the *valid attribute local name* rule), `set_data` and
  `replace_all_with_text` (*string replace all*, one change). `alo-js`
  gains `Engine::intrinsics` and `Missing::ASecondArgumentBehindACall`.
  Text has no item-80 member, so `Text.prototype` is in the chain, empty.
  **Deviations, recorded:** `document` is a non-writable, non-configurable
  data property rather than Web IDL's accessor (item 251); a lone surrogate
  becomes U+FFFD in the document, as when the parser reads one; two object
  arguments to `setAttribute` are refused by name (item 221); and no
  interface object (`Node`, `DOMException`) is on the global object.
  *Closed by:* `crates/alo-bindings/tests/what_a_script_does_to_its_document.rs`
  (8 tests: every member made, inserted, moved, replaced, removed and read
  back, with the serialisation, eleven changes and four new ids asserted;
  the next id after the parsed ones and every parsed id kept; twelve
  refusals each the named `DOMException` with `Error.prototype` on its
  chain and the tree and count untouched; sixteen wrong `this`es and
  arguments — a plain object, a fake inheriting `Element.prototype`, a
  `Text` for an `Element`, the document for `remove`, a missing argument,
  a node of a second document as argument or as `this` — each a
  `TypeError`; one node through seven members one object, its expando
  through three collections; `textContent` on every kind; an object
  argument's `toString` run once each, six in all, and two objects refused
  by name; fourteen absent members) and
  `a_script_that_is_hostile_to_its_document.rs` (4: a node into its own
  child, itself or its ancestor six ways, refused; appending a mebibyte of
  text in a loop until the heap's ceiling stops the script with `Full`,
  over 500 nodes in, the heap unbroken; a million `createElement`s
  released, a million trees counted, only the document's wrapper left and
  the next id past all of them; every member run with the collector at
  every allocation). Doctored runs, each restored and checked identical by
  hash: twenty-one rules disabled alone — the three brand checks, the same
  document, the argument count, `null` as no node, the exception's
  prototype, its inheriting `Error.prototype`, the prototypes traced,
  `null` as the empty string, the second-argument refusal, `document`
  read-only, `replaceChild` answering its child, a text node's interface,
  HTML lowercasing, the attribute-name rule and its `=`, prefix matching,
  *replace all* counted once, a comment's data, and the prototype held
  before its members are made — each fails at least one test.
  **Measured:** the ceiling is enforced where the heap allocates, so the
  write that adds the last node can take it past by that one change (a
  mebibyte here) before the next allocation is refused.

- [x] **250. The renderer hands its document to script and renders what script
  left.** *Cut from 246 (ADR 0017 §§ 2 and 6). Depends on 249.* The document
  moves into the page's heap when its first script is about to run, rooted by
  the renderer, and every reader — style, layout, paint, the agent's tree,
  `apply` — borrows it from the cell; the page is rendered again whole, from
  the same document, when its change count says what was rendered is stale:
  at `Paint`, `ReadTree`, an `Act`'s decision, the end of a `Load` and a
  `Resize`. A page that runs no script never builds a heap, and every stage 1
  reference render still matches. *Closes when* (item 80's own condition): a
  page's script appends an element and the next render's box tree and layout
  have it, in numbers, with a reference render; the agent names the node the
  script made and acts on it; every parsed node keeps its id; and a page that
  changes its document ten thousand times in one task is rendered once.
  **Done (iteration 146).** `alo-renderer` gains `held.rs`: `Held` is where
  a page's document is — `Parsed`, the renderer's, until its first script
  that may run is about to; then `Scripted`, the page's heap's, behind one
  `Root` — and every reader borrows it (`document`), `apply` changes it
  (`change`), and `scripted` makes the event loop, adopts the document,
  roots it and installs `document`. `scripts::at_load` takes the held
  document and calls `scripted` only for a script that may run, so a page
  none of whose scripts may run never builds a heap. The pipeline's core is
  `pipeline::draw`, which **borrows** a document and answers a `Drawing`
  (styles, boxes, layout, display, canvas, the sheets' issues, the fonts
  wanted); `Rendered` is a parsed document and its drawing, for the corpus.
  `Renderer` keeps the held document, the last drawing and the change count
  it was drawn at, and draws again whole when the count has moved, at
  `Paint`, `ReadTree` and an `Act`'s decision (and after its change); a
  `Load` draws once, after its scripts; a `Resize` draws the document the
  page has, never its markup again. `Renderer::document`,
  `Renderer::rendered` (now a `Drawing`) and `Renderer::draws` are for
  tests, outside the boundary. **A heap that will not take the document
  hands it back**: `alo-js` gains `Heap::allocate_or_back`,
  `Objects::foreign_or_back` and `Typed::into_any` (the heap now asks whether
  a slot can be named before placing a cell, so nothing it refuses is
  dropped), and `alo_bindings::adopt` answers `Unadopted` with the
  document, which stays `Parsed` and is still drawn. The corpus renders a
  case whose page carries script **through a renderer** (`rendering.rs`),
  refusing by name one that also links a sheet or a picture; `check` takes
  a document and its drawing.
  *Closed by:* the corpus case `a-script-grows-a-list` — a script appends a
  row with a class, changes a row's text and removes a paragraph, and the
  committed `boxes.txt`, `layout.txt`, `agent.txt` and `render.png` have the
  third row at (8, 89.875) 184×25.296875, the changed text and no
  paragraph; `crates/alo-renderer/tests/what_a_script_left.rs` (11 tests: a
  field the script inserted laid out between two blocks, every box in
  numbers, and the serialisation; the agent naming it, its node `#N` past
  every parsed one, and putting text into it in the heap's document; every
  parsed element keeping its id; 10000 changes in one script drawn once, and
  a `Paint` and a `ReadTree` of an unchanged page drawing nothing; a task
  run after the load drawn by the next `Paint`, read by the next `ReadTree`
  and decided against by the next `Act`; a resize at 150 wide keeping the
  script's field and the agent's text, the script not run again; a page
  whose only scripts are refused building no heap; and the hostile half —
  what a script changed before it threw is drawn, and a page whose script
  removed its root element is drawn empty and still answers every
  message); `crates/alo-corpus/src/rendering.rs` (3 unit tests);
  `crates/alo-js/tests/what_an_embedder_gets_back.rs` (an object weighing
  the whole heap refused and handed back whole). Doctored runs, each
  restored and checked identical by hash: nine rules disabled alone — the
  fresh check at `Paint`, at `ReadTree`, before an `Act`'s decision and
  after its change, the staleness comparison, drawing after the scripts
  rather than before, a resize from the held document rather than the
  markup, a change reaching the heap's document, a heap only when a script
  may run — each fails at least one test. **Not discriminated:** the
  renderer's path for a document the heap refuses, which needs a document
  over the heap's 1 GiB ceiling; the engine half of it is tested.

- [ ] **251. `document` as Web IDL's accessor, on a global object that is a
  `Window`.** *Cut from 249.* Web IDL makes `document` an unforgeable accessor
  on the window; its getter is a native handed only its `this`, and today's
  global object is an ordinary one with nowhere to find the document, so 249
  made it a non-writable, non-configurable data property — the same to every
  member a script has, different to a property descriptor. *Depends on 250,
  and is observable only once `Object.getOwnPropertyDescriptor` exists (item
  73). Closes when:* the global object is an embedder cell that holds its
  document, `document` is an accessor whose getter reads it, and a
  descriptor reads as Web IDL's.
  *Narrowed by ADR 0037 § 6 (iteration 233):* the global object becomes a
  `Window` holding its document with item 362, before this one. What is left
  here is `document` as an unforgeable accessor on the `Window`, whose getter
  reads the document through the `Window`'s edge. It still waits on item 73,
  the only thing that can observe it.

- [x] **252. A thrown `DOMException` is reported by its name.** *Found by 250.*
  A script that lets a refusal escape — `appendChild(document)` — is said in
  the load's issues as `uncaught: an object`: `described.rs` names the
  engine's own errors and calls every other object *an object* (which
  object to trust is item 78's), and a `DOMException` is an embedder cell
  of `alo-bindings`, not one of the engine's errors. A page's author needs
  `HierarchyRequestError` and its message there, and the renderer can ask
  for them by type (`Objects::embedded::<DomException>`) without running a
  getter. *Depends on nothing. Closes when:* every `DOMException` a member
  of item 80's throws is reported with its name and message, read without
  running any of the page's code, and every other object is reported as
  now.
  **Done (iteration 148).** `alo-renderer`'s `described.rs` asks a thrown
  object, by type, whether it is `alo-bindings`' `DomException` cell
  (`Objects::embedded`), and says one as `name: message` **from the cell's
  own two slots** — what its getters answer, and what nothing a page does
  can change — before the error and plain-object cases, which are as they
  were. Not from the properties along its chain: those are getters, and a
  page that deleted or replaced them would choose what its own failure
  says. An object that only inherits from `DOMException.prototype` is not
  one. *Closed by:* `crates/alo-renderer/tests/a_dom_exception_said_by_its_name.rs`
  (7 tests: each of the six members that throw — `appendChild`,
  `insertBefore`, `removeChild`, `replaceChild`, `createElement`,
  `setAttribute` — made to refuse, eight refusals over all three names,
  each reported as `uncaught: <name>: <message>`; each the same words the
  page's own `catch` read through the getters; a page that deletes both
  getters, plants a name of its own and a counting `message` getter is
  reported by the exception's own name and message with the getter run
  zero times; an object inheriting from `DOMException.prototype`, a plain
  object shaped like one and a renamed `Error` said as before; a refusal
  thrown through a function said with both places; two refusals in two
  scripts each said), and `what_a_script_left.rs`'s hostile test now
  asserts the name. Doctored runs, each restored and checked identical by
  hash: the `DomException` case removed (five of the seven fail, and
  `what_a_script_left.rs`), and the exception read as an error's
  properties instead of its slots (five fail). **Not doctored:** asking by
  type rather than by prototype — no simple edit spells the wrong rule; the
  inheriting-object test pins it for whoever writes one.

- [x] **247. A parser-inserted script sees the document up to its own
  element.** *Cut from 80 (ADR 0017 § 7). Depends on 246.* The parser stops
  at each classic script's end tag (`html5ever`'s `TokenizerResult::Script`),
  the renderer runs it as a task with its checkpoint, and the parser
  continues; the page renders once, at the end of its load. *Closes when:* an
  inline script in the middle of `<body>` reads `document.body.lastChild` as
  its own `<script>`; a script before a `<p>` cannot find it and one after
  can; scripts still run in document order under the same policies; and the
  renderer's existing script tests are re-read against the new order.
  **Done (iteration 147).** `alo-dom`'s `parse.rs` gains `Parsing`: `start`
  hands out the document, and each `resume` is **lent** it (`&mut`), runs
  html5ever's tokenizer to the next `TokenizerResult::Script` or the end,
  and gives it back — moved into the tree builder's sink for the step and
  out again, since the builder outlives every borrow, with the caller's
  exclusive borrow held throughout. `parse_document` is the same parse run
  to its end. The sink records each HTML `<meta>` it makes (`take_metas`).
  `Document::is_being_parsed` holds from `start` to the end, and **while it
  does, `release` lets nothing go** — the parser holds open elements no
  wrapper marks, and a collection would otherwise tombstone a `<body>` a
  script detached while the parser was still inserting into it.
  `scripts::prepared` is what a `<script>` the parser stopped at is, by
  `carried`'s rules plus HTML's *not connected, return*; `scripts::stated`
  is a `<meta>`'s policy, asked as the parser made it. `alo-renderer`'s
  `scripts::at_load` drives the parse through `Held::change`, gathers the
  `<meta>` policies made before each stop, and runs the script there;
  `load` starts the `Parsing`. **Cut: `document.body`** — not one of item
  80's members, so the scripts reach the body as
  `document.documentElement.lastChild` (the root element's last child
  while it is being parsed), which is the same node; the member is item
  **253**. **A changed behaviour**: a `<script>` still open at the end of
  the markup no longer runs (HTML marks it already started); every other
  existing script test reads the same under the new order — each was
  re-read: the order tests run pure script, the policy tests' `<meta>`s
  already governed only what followed, `what_a_script_left.rs`'s scripts
  are each last in their body, and the prefix test asks only that every
  prefix loads.
  *Closed by:* `crates/alo-renderer/tests/a_script_at_its_own_end_tag.rs`
  (10 tests: a mid-body script is its body's last child, its previous
  sibling the element before it; a script before a `<p>` does not find it
  and one after does; three scripts count `1`, `3` and `6` children of the
  body in document order; a `<meta>` policy a script removes still refuses
  the script after it and admits the one with its nonce; a `<meta>` after a
  script does not reach back to it; a row a script appends beside itself
  laid out between the rows at (0, 20) 300×20, in numbers; the script's row
  numbered before the row the parser read after it; an unterminated script
  not run; and the hostile half — a script that takes out the body the
  parser is in, drops every hold on it inside a function and allocates
  eight mebibytes until the heap collects: no tree-builder broken promise,
  the script the parser then put in the detached body not run, the page
  drawn and answering; and a script that removes its own element);
  `crates/alo-dom/tests/a_parse_in_steps.rs` (9 tests: each stop with its
  script last and nothing after it; a stepped parse equal to a whole one —
  tree, ids and issues — over tables, misnested formatting, templates, SVG
  and `<select>`; a stop at every `</script>`, data blocks and templates'
  included, and none at an unterminated one; a change between steps kept
  and the parser's next row after it, on one counter; a body taken out
  between steps still inserted into, never released while the parse lasts
  and released (10 nodes) once it has ended; a document the parse did not
  start not built into; `<meta>`s handed over once, in order; every prefix
  of a page with scripts parsing in steps to the same document; ten
  thousand scripts, ten thousand stops); `scripts.rs` 3 unit tests
  (`prepared` agrees with `carried`; not connected, a template's, an SVG
  one; `stated` only in a `<head>` in the document); and the corpus case
  **`a-script-beside-itself`** — reference render looked at: *Older*, the
  script's green *From the script*, *Newer*; `layout.txt` has the script's
  row at (8, 64.578125) 184×25.296875. Doctored runs, each restored and
  checked identical by hash: nine rules disabled alone — release while
  parsing (the dom test, and after one fix the renderer's: its first
  version held the body's wrapper in a register, so it did not
  discriminate), `prepared` requiring connected, `stated` requiring
  connected, `resume` refusing a document it did not start, the end
  clearing *being parsed*, stopping at an end tag at all, a policy read as
  the parser made it rather than from the page as it now is, the sink
  recording `<meta>`s, and the corpus case's row order — each fails at
  least one test.

- [x] **253. `document.body`.** *Cut from 247.* Item 247's closing condition
  names `document.body.lastChild`; `body` is not one of item 80's members,
  so 247's scripts reach the same node as `document.documentElement.lastChild`.
  HTML's `body` is an attribute with a getter (the root element's first
  `body` child — and `frameset`, which law 1 leaves a page to fail
  without) and a setter (replace or append, refusing what is not a body
  as `HierarchyRequestError`); a getter alone would be an approximation
  (ADR 0013 § 3), so both arrive together. *Depends on 249. Closes when:* a
  script reads `document.body` as the body element and `null` before
  there is one, and assigning a body replaces the old one or is refused
  by name.
  **Done (iteration 149).** `alo-dom`'s new `body.rs` is HTML's *the body
  element*: `Document::body` (the first `body` or `frameset` child of the
  document element when that is an HTML `html`) and `Document::set_body`
  (the same element changes nothing; otherwise the body element is
  replaced through `replace_child`, or, with none, the new one appended to
  the document element through `append_child`; anything not a `body` or a
  `frameset` — `null` included — and a document with no element are
  `HierarchyRequestError`). `alo-bindings`' `Document.prototype` gains
  `body` as an accessor with both halves; the setter converts its value as
  Web IDL's `HTMLElement?` (`idl::nullable_html_element`: `null` and
  `undefined` are no element, anything not an element in the HTML
  namespace is a `TypeError`). **Decision inside the item:** `frameset`
  counts, as the standard says, against this entry's own sketch — law 1
  refuses to render frames, not to answer `document.body` as every other
  engine does on a page with one; leaving it out would be the approximate
  member ADR 0013 § 3 refuses. Recorded in `body.rs`.
  *Closed by:* `crates/alo-dom/tests/the_body_element.rs` (14 tests: a
  parsed page's body and an empty document's none; a page stopped at a
  script in its head has no body yet, and has it after; a frameset is the
  body element; the first `body`/`frameset` child of the html element and
  no deeper; a document element that is not `html` has none; a new body
  replaces the old where it was, the old detached keeping its children,
  one change counted; the same body changes nothing, not even the count;
  frameset and body replace each other; a body from inside the tree is
  moved into place; with no body, appended to the document element, and to
  a non-`html` one; `null`, a `div`, text, the `html` element and the
  document refused by name with nothing changed; no document element
  refused; the append's own cycle refusal passed through; and the hostile
  half — every id, minted or not, answered or refused with nothing
  changed); `crates/alo-bindings/tests/the_body_a_script_reads_and_replaces.rs`
  (8 tests: the body as one wrapper, an expando kept; `null` with none and
  under a non-`html` root; an assignment answering its value, the old body
  detached keeping its expando and children, the serialisation, two
  changes counted and the new body numbered after the parsed nodes; the
  same body no change, a frameset appended; five `HierarchyRequestError`s
  with nothing changed and the message read; no document element;
  nine `TypeError`s — an SVG element, text, the document, a plain object,
  a string, another document's body, and the getter and setter on objects
  that only inherit from `Document.prototype`; ten thousand replacements
  then a collection, the last body's expando kept);
  `crates/alo-renderer/tests/the_body_a_page_reads_and_replaces.rs` (3
  tests: item 247's sentence as written — a mid-body script reads
  `document.body.lastChild` as itself — and a script in the head reads
  `null`; a body a script assigns laid out in numbers, `Kept` at (0, 0) and
  `Made` at (0, 20), 300×20 each, read by the agent without the old body's
  row, and serialised; a refused assignment reported as
  `uncaught: HierarchyRequestError: …`, nothing changed, the page drawn);
  and the corpus case **`a-script-gives-a-new-body`** — reference render
  looked at: *Inbox* and the green *Three new messages*, no red *Loading*;
  `layout.txt` has the line at (8, 39.28125) 184×24.296875. Doctored runs,
  each restored and checked identical by hash and rebuilt: seven rules
  disabled alone — `frameset` not counted, any document element taken as
  the html element, the same body not short-circuited, the last rather
  than the first such child, `null` taken as no change, the HTML-namespace
  conversion, and the setter absent — each fails at least one test.

- [x] **81. Events**: capture and bubble, listeners, default actions. **This is
  what makes a button do something**, which every agent verb has been honest
  about not doing since stage 1.
  *Depends on 80. Closes when:* `alo-renderer`'s test that a nav row changes
  nothing fails, and is rewritten to assert what it now does.
  **Its decision is ADR 0018 (iteration 150)**, written first because ADR 0017
  and ADR 0016 each left dispatch, capture and default actions to this item by
  name, and the code made the gap real: a builtin calls script only by
  suspending at a numbered step and a throw unwinds through it, the loop's
  `Work::Calls` fixes its callees when queued, and `apply` toggles a checkbox
  and `aria-checked` itself. The dispatch algorithm is one stepper in
  `alo-bindings` whose state lives in the event, driven by a native for
  `dispatchEvent`/`click()` (the engine gains a call whose throw is reported)
  and by the loop for the browser, checkpointing after every listener;
  listeners live in their target's wrapper; an agent's verb is trusted and
  unmarked; `Activate` is the one `click` keyboard activation fires —
  `PointerEvent`, `pointerId` −1, no coordinate; activation behaviour lives in
  `alo-dom` and a cancelled click undoes a checkbox's toggle; on a scripted
  page the agent no longer changes ARIA state. No code yet. **The closing
  page:** `alo-settings` gains a plain-DOM script doing what alo-workplace's
  `SettingsModal.tsx` does on a nav click (`aria-current` and `navItemOn` move
  to the pressed row); its reference render must not move, which shows the
  script changed nothing at load. It closes when 254, 255 and 256 have.
  **Closed (iteration 155)** with 256: `an_agent_on_settings.rs`' nav-row
  test now asserts that pressing *Sharing* makes it `aria-current` and
  *General* not, the highlight's fill moved to (36, 208.1336) 169×31.132813,
  and `alo-settings`' references unchanged with the script in the page. What
  it cut and left open is its own items: 257, 258 (needs design), 259 and
  261.

- [x] **254. Events from script: `EventTarget`, `Event`, `CustomEvent` and
  the dispatch algorithm.** *Cut from 81 (ADR 0018 §§ 1–3, 8). Depends on
  nothing.* `Node.prototype` inherits from `EventTarget.prototype`; a
  wrapper holds its listener list, traced and counted in its footprint;
  `addEventListener`'s options converted as Web IDL's dictionary (a `signal`
  that is not `undefined` the conversion's `TypeError`), a listener a
  function or a `handleEvent` object; the stepper in `dispatch.rs` with its
  state in the event cell; `dispatchEvent` as a native driving it through
  `Answer::Want`; `alo-js` gains a call a builtin may ask for whose throw is
  reported, not propagated; the dispatch flag's `InvalidStateError`; the
  members § 8 names and none it calls absent. *Closes when:* a page's script
  adds capture and bubble listeners on a nested tree, dispatches a
  `CustomEvent`, and the order, `eventPhase`, `currentTarget`, `once`,
  `passive`, both stops and a throwing listener (reported, dispatch carrying
  on) are asserted — through `alo-bindings` tests and a corpus case whose
  script writes the order it saw into the page, with its layout in numbers.
  **Built (iteration 151).** `alo-js`: `Want::Report`, a call a builtin asks
  for whose throw nothing inside it catches **stops at it** (`catch.rs`), is
  set aside rooted with its trace (`interpret/reported.rs`, bounded by
  `REPORTS_SET_ASIDE` = 256 with the rest counted), and answers the builtin
  `undefined` with `Call::reported()`; the embedder takes them with
  `Engine::hand_over_reported` after a run or call, and a checkpoint hands a
  job's to its own report before the job's throw (`Drained::reported`,
  `unreported`); and `Instance::Made`, an embedder's constructor given its
  instance only by `new` (`Call::constructing()`). `alo-bindings`:
  `listeners.rs` (a wrapper's list, ids for the *removed* flag, counted in
  its footprint), `event.rs` (the event cell holding the dispatch's
  `Progress`), `dispatch.rs` (the stepper), `dictionary.rs`, and
  `interface/event_target.rs`, `event.rs`, `custom_event.rs`; `EventTarget`
  heads a node's chain; `Event` and `CustomEvent` are the only interface
  objects on the global; a dispatch's path is kept by the document cell
  (`on_path`) until it ends; `InvalidStateError` for a second dispatch.
  `alo-renderer` reports a listener's throw after its script, placed in it.
  **Cut, by scope:** `isTrusted` is item 260 — Web IDL makes it
  `[LegacyUnforgeable]`, and a prototype accessor would be approximate; the
  flag is kept and set. Tests: `alo-js/tests/what_a_reported_call_reports.rs`
  (7), `alo-bindings/tests/what_an_event_does.rs` (12, every script both
  ordinarily and collecting at every allocation),
  `alo-renderer/tests/what_a_listener_hears.rs` (2: the paragraphs in
  numbers, and the throw reported at script 1, line 16, column 70 on each
  dispatch); corpus case **`a-script-hears-an-event`**, reference render
  looked at. Doctored runs, each restored and checked identical: the path
  not kept, `once` not removed, `passive` ignored,
  `stopImmediatePropagation` ignored, the bubble pass skipping the target,
  no reporting boundary, and the renderer not handing over — each fails a
  test.

- [x] **255. A dispatch from the browser is a task.** *Cut from 81 (ADR 0018
  § 3, ADR 0016 §§ 3 and 6). Depends on 254.* A `Work` kind beside `Script`
  and `Calls` holding the event by its task's root and stepping 254's
  stepper, a microtask checkpoint after every listener, a throw reported and
  the dispatch carrying on, a stop stopping the page; a page that never ran
  script dispatched to by nobody and given no heap. *Closes when:* two
  listeners on one target, dispatched from the renderer, each see the other's
  microtasks run between them, and the same two dispatched by a script's
  `dispatchEvent` do not.
  **Built (iteration 152).** `alo-renderer`: `Work::Dispatch` (`task.rs`), one
  rooted list of the target's wrapper — made if the node had none — and an
  event the browser made (`alo-bindings`' `event::create`, a `Firing`'s type
  and init flags); `EventLoop::queue_dispatch` (`Unqueued`: stopped, no such
  node, no document); the driver in `event_loop/dispatched.rs`, which begins
  the dispatch trusted, calls each listener with nothing else running, hands
  over set-aside throws, reports its own, runs the checkpoint, and **only
  then** tells the stepper it returned — the standard's *inner invoke*, so a
  microtask's `stopImmediatePropagation` stops the next listener and its
  `preventDefault` after a passive listener does nothing; `Held::dispatch`,
  which answers `None` for a page that never ran script and builds no heap.
  `alo-bindings`: `dispatch::invoke`, how a callback is called (itself, its
  `handleEvent`, or its `handleEvent` getter's answer), now asked by both
  drivers. Tests: `alo-renderer/tests/a_dispatch_from_the_browser.rs` (13,
  most run ordinarily and collecting at every allocation). Doctored runs,
  each restored: no checkpoint per listener (5 tests fail), the stepper told
  before the checkpoint (1), the task not rooting its target (1).

- [x] **256. `Activate` is a keyboard's click.** *Cut from 81 (ADR 0018 §§ 4–7).
  Depends on 255 and 260* (an agent's click is trusted, § 4). `UIEvent`,
  `MouseEvent` and `PointerEvent` with the members § 5 names; `alo-dom`'s
  `activation.rs` (before, cancelled, after) for a checkbox, a radio and a
  link; `HTMLElement` between `Element` and an HTML
  element, and `click()`; `apply` stops toggling, and stops changing
  `aria-checked` on a scripted page; the agent's answer comes after the
  task. *Closes when:* item 81's closing condition — `alo-settings`' script,
  the nav row test rewritten to assert *Sharing* is `aria-current` and
  *General* is not, with the moved highlight's box asserted in numbers and
  the reference render unchanged at load — and a cancelled click on a
  checkbox leaving it unticked.
  **Built (iteration 155).** `alo-dom`: `activation.rs` — the activation
  target (the click's target or its nearest ancestor with an activation
  behaviour), `before` (a checkbox turned over, a radio checked and its
  group — same tree, same non-empty name; form owners are item 82's —
  cleared), `cancelled` (put back as HTML's legacy-canceled-activation
  says), `after` (`input` and `change` for a box still connected, a link to
  follow, nothing for a `button` until item 82). `alo-bindings`: the event
  cell's `Shape` (`Event`, `Custom`, `Pointer`); `UIEvent` (`detail`),
  `MouseEvent` (`screenX/Y`, `clientX/Y`, the four modifier keys, `button`,
  `buttons`, `relatedTarget`) and `PointerEvent` (`pointerId` −1,
  `pointerType` `""`) prototypes in the chain, no constructors on the
  global; `Firing` names its interface (`Fired`) with `CLICK`, `INPUT` and
  `CHANGE`; `event::create` makes either. `alo-agent`'s `apply` asks
  `alo-dom` instead of toggling and keeps `aria-checked` as ADR 0018 § 7's
  stage 1 accommodation, reached only on a page that never ran script.
  `alo-renderer`: `Work::Activate` and `event_loop/activated.rs` — before,
  the click, cancelled or after, `input` and `change` dispatched in the
  same task — `Turn::clicked`; `Held::activate`; `press.rs`, which runs the
  loop until that task has run and bounds what it says; `act` answers a
  cancelled link as activated, an uncancelled one as followed; a stopped
  page's click still changes its box and says nobody heard it.
  `FromRenderer::Acted` gains `issues` (wire tag 3: the outcome, then a
  counted list of lines). Tests: `alo-dom/tests/what_a_click_activates.rs`
  (13), `alo-renderer/tests/an_agents_click.rs` (10, three run ordinarily
  and collecting at every allocation), `an_agent_on_settings.rs` (the nav
  row rewritten, and one more that the screen is the markup's at load),
  `messages_across_a_boundary.rs` (an `Acted` with issues round-trips).
  Doctored runs, each restored: `cancelled` not unticking, the click on a
  scripted page falling back to `apply`, no `input`/`change`, the click a
  plain `Event`, and a cancelled link followed — each fails a test. The
  corpus case `alo-settings` passes with every reference unchanged.
  **Cut, by scope:** `HTMLElement` and `click()` are item 261 — the
  activation rule's second caller; the rule itself is built for both.

- [x] **257. `PutText` fires `beforeinput`, `input` and `change`.** *Cut from
  81 (ADR 0018 § 5). Depends on 256* (for `UIEvent`). `InputEvent` with
  `inputType` `"insertReplacementText"` and `data`; a cancelled
  `beforeinput` changes nothing. *Closes when:* a page's `input` listener
  echoes a field's text elsewhere and the agent reads the echo.
  **Built (iteration 157).** `alo-bindings`: `Interface::InputEvent`
  (`InputEvent`, inheriting `UIEvent`, no interface object on the global)
  and `interface/input_event.rs` — `data`, `inputType` and `isComposing`
  (`false`), read-only and brand-checked; `dataTransfer`,
  `getTargetRanges()` and the constructor absent. The event cell's
  `Shape::Input` holds the `inputType` and `data`, counted in its
  footprint; `Fired::InputEvent`, with `Firing::before_replacing` (a
  cancelable, bubbling, composed `beforeinput`) and `Firing::replaced`
  (the `input`, not cancelable). `UIEvent`'s `detail` answers `0` for an
  `InputEvent` too. `alo-dom`: `field.rs`, what text put into a field does
  (its `value` attribute until item 82), now the one rule both `alo-agent`'s
  `apply` and the renderer run. `alo-agent`: `Outcome::TextCanceled` —
  the verb was carried out and the page said no, so it is an outcome, not
  a refusal. `alo-renderer`: `Work::PutText` and `event_loop/typed.rs` —
  the `beforeinput`; if cancelled, nothing more; otherwise the text, then
  `input` and `change` at the field, all one task, a checkpoint after
  every listener, what it came to written into `Turn::typed` as each step
  happens so a page that stops partway is still answered truly;
  `Held::put_text`; `put.rs`, which answers `TextPut` or `TextCanceled`
  and puts the text into a stopped page's field, as a stopped page's box
  is still ticked; `run_to.rs`, the loop-running and bounded saying taken
  out of `press.rs` so both verbs share it; the wire's outcome tag 4.
  *Closing condition met:* in `an_agents_text.rs`, a page's `input`
  listener writes *You typed 12.50* into an `<output>`, the agent's
  `ReadTree` reads it, and the echo's text is laid out at (174, 55.2)
  129.45313×18.625 — ordinarily and collecting at every allocation.
  Tests: `alo-renderer/tests/an_agents_text.rs` (9: order and members,
  bubbling to the document, a job between listeners, cancelling, passive
  and uncancelable, the chain and brand checks, a throwing listener, a
  field removed mid-task, a stopped page, a scriptless page), `alo-dom`'s
  `field.rs` unit tests (2), the `TextCanceled` outcome's display and its
  wire round-trip. Doctored runs, each restored, touched and rebuilt: the
  cancel ignored, the text put before `beforeinput`, no `change`, `input`
  cancelable, a page stopped mid-task not given the text, `data` not held,
  a cancel answered as `TextPut`, and `UIEvent`'s `detail` refusing an
  `InputEvent` — each fails a test.

- [ ] **258. Focus.** *Cut from 81 (ADR 0018 § 5 and *What this does not
  decide*).* What has focus, `focus`/`blur`/`focusin`/`focusout`, a
  focusable target focused before `Activate`'s click as a keyboard user's
  was, and `:focus`/`:focus-visible` matching it — which is what item 43's
  focus ring is waiting for. *Depends on 256. Needs design* until somebody
  decides what an agent's `PutText` does to focus and whether it is ever
  keystrokes.

- [ ] **259. Event handler attributes and properties.** *Cut from 81 (ADR
  0018 § 8).* `onclick="…"` compiled under the page's policy as
  `csp::Inline::Script` with `csp::Content::attribute` (item 191's shape),
  and `el.onclick = f`. *Depends on 254. Opened by a page* that fails
  without them, as `ROADMAP.md` asks of stage 2.

- [x] **261. `HTMLElement` and `click()`.** *Cut from 256 (ADR 0018 § 6).
  Depends on 256.* An element in the HTML namespace gets the `HTMLElement`
  interface between `Element` and its own; `HTMLElement.prototype.click()`
  is the standard's: nothing on a disabled form control, nothing while that
  element's click is already in progress, otherwise an **untrusted**
  `PointerEvent` `click` (`pointerId` −1) dispatched synchronously from the
  native (ADR 0018 § 3's script driver — no checkpoint between listeners),
  with `alo-dom`'s `activation.rs` run around it exactly as the renderer's
  `Work::Activate` runs it, `input` and `change` included. *Closes when:* a
  page's script calls `box.click()` and its listeners read the box ticked,
  `isTrusted` `false`, and no microtask between them; a listener's
  `preventDefault` leaves the box as it was; and a second `click()` from
  inside the first's listener does nothing.
  **Built (iteration 156).** `alo-bindings`: `Interface::HtmlElement`
  (`HTMLElement`, inheriting `Element`) is the interface of every element
  in the HTML namespace, so `Interface::of` and the brand check
  (`Brand::HtmlElement`) ask the namespace; an SVG element stays an
  `Element`. `interface/html_element.rs`: `click()` — nothing on a
  `button`, `input`, `select` or `textarea` that `:disabled` matches
  (`alo-css`' `state::is_disabled`, so one rule for both), nothing while
  the element's click is in progress, otherwise an untrusted
  `PointerEvent` `click` made by `event::create` and dispatched with
  `alo-dom`'s `activation.rs` around it, `input` and `change` after, all
  inside the call. `clicking.rs`: the click in progress flag is the
  wrapper holding a `Clicking` — the event being dispatched, the
  activation target's wrapper and the pre-activation's record — traced as
  the wrapper's edges, so the native keeps nothing across a listener but
  its step. `scripted.rs`: the native driver, taken out of
  `dispatchEvent` so both natives drive the stepper one way, each from a
  base step. A click nobody cancelled on a **link** is refused by name
  after its listeners (`alo-js`' new `Missing::InTheEmbedder`, the
  embedder's own words; item 263). Tests:
  `alo-renderer/tests/a_scripts_click.rs` (10, run from an agent's press
  so the script's `click()` is inside the browser's dispatch, each page
  pressed ordinarily and collecting at every allocation and required to
  agree) and `html_element.rs`' unit test of which controls are disabled
  form controls. Doctored runs, each restored: `Clicking` not traced, no
  click in progress flag, a cancelled click not undone, the click
  trusted, a disabled control clicked, a link silently not followed, and
  no `input`/`change` — each fails a test.
  **Cut, by scope:** the interface of each element's own name is item
  262; a script's click following a link is item 263.

- [ ] **262. Each HTML element's own interface.** *Cut from 261.* An
  `<input>` is an `HTMLInputElement`, a `<div>` an `HTMLDivElement`, an
  element HTML does not name an `HTMLUnknownElement`, each inheriting
  `HTMLElement` — the chain HTML's table gives, with empty prototypes
  where no member is built, as `Text` and `Comment` are today. Until then
  an HTML element's prototype is `HTMLElement.prototype` itself, a link
  short and said so in `alo-bindings`' `interface.rs`. *Depends on 261.
  Opened by a page* whose script reads a member of one of them, or tells
  one element's interface from another's. *Closes when:* an `<input>`'s
  and a `<div>`'s prototypes differ and both inherit `HTMLElement`'s
  `click`.

- [x] **263. A script's `click()` follows a link.** *Cut from 261 (ADR
  0018 § 6).* A click nobody cancelled on an `a` or `area` with an `href`
  follows it, which is the page navigating itself: the renderer must ask
  the browser process to navigate, as `Act` answers `Followed` for an
  agent's press — and an `<a download>`'s activation is a download rather
  than a navigation. alo uses both: `alo-workplace`'s `FilesView.tsx` and
  `TaskDetail.tsx` make an `<a download>` and call `a.click()` on it.
  Until then the case is refused by name after the click's listeners have
  run. *Depends on 261*, and on a decision about what a renderer may ask
  the browser process to navigate to, which **needs ADR** if ADR 0005 and
  0012 do not already decide it. *Closes when:* a page's `a.click()` on a
  link nobody cancelled reaches the browser process as a navigation that
  says the page's script caused it.
  **Its decision is ADR 0020 (iteration 158)**, written because ADR 0005
  decides only the direction and ADR 0012 only who names the cause: a
  renderer's navigation is a **claim in the answer** to the message whose
  work made it, recorded meanwhile in the document cell (HTML's ongoing
  navigation, one, the last, with a count of those it replaced); the URL
  resolved by the renderer against the document's base, so `Page` gains
  its URL; the browser process parses it again, refuses by name what a
  page may not send its tab to (`data:`, `javascript:`, `blob:`, other
  schemes, `file:` from a non-`file:` document, unparseable or over
  2 MiB, a `target` naming another window), and assigns the cause from
  which message it answered — `Cause::Agent` in an `Act`'s answer,
  `Cause::Document` otherwise — with *a script's click* kept as the ask's
  claim beside it. An agent's own link answers through the same ask.
  `<a download>` is cut to item 264. No code yet. **Closing, made exact
  by the ADR:** a script at load calling `a.click()` reaches `Tabs::load`'s
  caller as a navigation with `Cause::Document` and the claim *a script's
  click*; inside an agent's `Activate`, the same click carries
  `Cause::Agent` and the same claim; a refused scheme is said and
  navigates nowhere.
  **Built (iteration 159).** `alo-bindings`' `navigating.rs`: following a
  link (an `<a download>` or a target naming another window is not
  followed and says why, an `href` is resolved against the document's
  first `<base href>` or its URL, `rel=noreferrer` and `referrerpolicy`
  kept) and the page's ongoing navigation — one, the last, a count of
  those replaced, at most 16 links not followed said and the rest
  counted — held in the document cell beside the document's URL
  (`Url::about_blank()` until `Held::scripted` states it). A script's
  `click()` records its link there and returns; on an `<a download>` it
  is refused by name for item 264. The browser's click records its link
  there too (`event_loop/activated.rs`, `Held::follow` for a stopped
  page), so a listener's `click()` and the agent's own link are one
  order; a page that never ran script asks in `Renderer::act` itself.
  `alo-renderer`: `Page::url` (stated by the browser process,
  `from_response`'s `response.url`, on the wire as text it parses);
  `ask.rs`' `Asked` (URL, `By`, referrer policy, replaced) in `Loaded` and
  `Acted` (`navigation`, wire-encoded, every tag checked) with the lines
  it says among the issues; `Outcome::Followed` only when following
  started a navigation. Browser side, `navigate.rs`: `decide` parses the
  URL again (refused unparsed or over `LONGEST_URL`, 2 MiB), navigates
  `http`, `https`, `about:blank`, `file:` only from a `file:` document,
  refuses every other scheme by name, works out `Referer` from its own
  copy of the document's URL; `Refusal::record` writes the line under
  ADR 0012. `Tabs::load` decides a `Loaded`'s ask as `Cause::Document`
  (the document it made), `Tabs::act` an `Acted`'s as `Cause::Agent`
  (its action), and `Tabs::navigation` hands the decision over once;
  `Tab::address` is the browser's copy. Tests:
  `alo-renderer/tests/a_page_asks_to_go_somewhere.rs` (14, scripted
  pages pressed ordinarily and collecting at every allocation, three
  driving real `Tabs` over the confined binary), `navigate.rs` (7),
  `ask.rs` (3), `navigating.rs` (8), wire round trips and refusals.
  Doctored runs, each restored: the first ask kept instead of the last,
  `data:`/`javascript:`/`file:` navigated, an `Act`'s ask attributed to
  the document, the browser's click not recorded, a base target and a
  base href ignored — each fails a test. **Not here:** going there
  (item 85), what the agent is told when the page goes (134), downloads
  (264), the `Referer` origin's trailing `/` (265), and a page's CSP
  `base-uri` (no item; `<base>` is the page's own markup).

- [ ] **264. A link's download.** *Cut from 263 (ADR 0020 § 6).* A click
  on an `<a download>` asks for a file, not a page: honoured for the
  document's own origin, `data:` and `blob:`, otherwise a navigation; the
  suggested name a claim the browser process cleans; a `blob:` URL's bytes
  taken at activation and carried with the ask, bounded; nothing written
  where the person did not choose. alo's `FilesView.tsx` and
  `TaskDetail.tsx` are the pages. *Depends on 263, on 120 for where a file
  goes, and on `Blob` (no item yet; `URL.createObjectURL` with it) for its
  `blob:` half. Closes when:* alo's download pattern — `blob:` URL,
  `a.download`, `a.click()`, `URL.revokeObjectURL` — reaches the browser
  process as a download of the blob's bytes under a cleaned name, and a
  cross-origin `download` is a navigation.

- [x] **260. `isTrusted`, as Web IDL's `[LegacyUnforgeable]` attribute.**
  *Cut from 254 (ADR 0018 §§ 4 and 8). Depends on nothing.* An own accessor
  on every `Event` instance, neither configurable nor writable, whose getter
  is **one function per realm** shared by every instance — which needs a
  place the constructor and the browser's dispatch can find that getter
  (the document cell's interfaces, or the engine giving an embedder's
  constructor more than its prototype). The flag is already kept and set:
  `false` for `dispatchEvent`, `true` for the browser's dispatch (255). A
  getter on `Event.prototype` instead would be the approximate member ADR
  0013 § 3 refuses. *Closes when:* `e.isTrusted` is `false` after a script's
  `dispatchEvent` and `true` in a listener for the browser's, its property
  is the instance's own and the same getter on two events, and a page
  cannot replace it.
  **Its decision is ADR 0019 (iteration 153)**, written first because
  the constructor's `this` reaches nothing per-realm and ADR 0017 § 4 said
  a native would never be handed more: `alo-js`'s realm gains ECMAScript's
  `[[HostDefined]]` (`Engine::host_defined`, set once and rooted;
  `Call::host_defined`), `install` sets it to the document cell, and the
  cell's `Interfaces` holds each interface's unforgeables object (Web IDL's
  `[[Unforgeables]]`), made once in `furnish` and copied onto every
  instance — by the constructors at their first step, before any page
  script can run, and by `event::create`. No code yet.
  **Built (iteration 154).** `alo-js`: the realm's `host` (`realm.rs`,
  rooted, set once — `Realm::define_host` hands a second value back),
  `Engine::host_defined` (a second call a `TypeError` naming it, the first
  standing) and `Call::host_defined`, passed by the interpreter beside the
  intrinsics (`interpret/call.rs`). `alo-bindings`: `Interfaces` holds an
  unforgeables slot per interface beside its prototype, traced by the cell;
  `install` names the cell as the realm's host before anything else, and
  `furnish` makes `Event`'s unforgeables object (no prototype) with
  `isTrusted` on it — `define::unforgeable_attribute`, enumerable, not
  configurable, no setter; `unforgeable.rs` is the one copy, walking the
  interface and those it inherits, allocation-free so not a safepoint; the
  `Event`/`CustomEvent` constructors copy at step 0 through
  `Call::host_defined`, and `event::create` copies from its cell. Tests:
  `alo-js/tests/what_a_realm_hosts.rs` (4: handed over, none when unset,
  rooted with the collector at every allocation, defined once),
  `alo-bindings/tests/who_sent_an_event.rs` (5: `false` before, during and
  after a script's dispatch and for a `CustomEvent`; not on either
  prototype; `delete`, sloppy and strict assignment, a prototype property
  and a cut prototype all fail to change it; one getter on two events, on
  the browser's event and on the unforgeables object, with no setter,
  enumerable and not configurable; a second `install` refused),
  `alo-renderer/tests/a_dispatch_from_the_browser.rs` (one more: `true` at
  the target and on the way up and after the dispatch, `false` once a
  script dispatches the same event). Doctored runs, each restored: the
  constructors not copying (5 tests fail), `event::create` not copying (2,
  one in each crate), the property configurable (2), the host held by a
  bare reference rather than a root (two tests fail where the collecting
  run disagrees with the ordinary one; that doctor also lost the set-once
  check, which two more caught).
  No corpus case: nothing positions, sizes or draws, and the closing
  condition is the three facts above.

- [ ] **82. Forms**: the controls, constraint validation, submission, file
  inputs.
  *Depends on 81.*
  *Needs design (iteration 161):* its dependencies are done, but it names no
  ADR, feature contract or closing condition, so `LOOP.md` step 2 says it is
  not ready to build. Cutting a first item from it, with those written, is
  the work that opens it.

- [ ] **83. `fetch()` and `XMLHttpRequest`**, over the same stack as everything
  else rather than beside it.
  *Depends on 61, 72.*
  *Needs design (iteration 161):* its dependencies are done, but it names no
  ADR, feature contract or closing condition, so `LOOP.md` step 2 says it is
  not ready to build. Cutting a first item from it, with those written, is
  the work that opens it.
  **Decided (iteration 212): ADR 0032, accepted.** Opened by
  `alo-downloads`, whose script stops at `fetch(href, { method: "HEAD" })`.
  In short: a script's fetch is an ask in the answer to the message whose
  work made it — every ask, in order, each under a number the renderer chose
  — and the response comes back as a `ToRenderer` message of its own, which
  is a task; the browser process decides from its own copy of the document's
  origin, header policy and cause (scheme, `connect-src`, mixed content,
  CORS and preflight, the partitioned jar, the referrer, ADR 0012 § 4's cause
  from which message it was answering) and filters the response **before**
  it leaves, so an opaque body never reaches a renderer; a network error
  tells the page nothing and the person why; a body crosses whole in one
  message; a synchronous `XMLHttpRequest` is refused by name. **No code is
  built and this item is not done.** It is cut three ways: the boundary
  (334), `fetch()` in a page (335) and `XMLHttpRequest` (336). It closes
  when all three have. **334 and 335 are built** (iterations 214 and 215);
  making a decided fetch is 338, and 336 waits for a page.

- [x] **334. A fetch crosses the boundary, and the browser process decides
  it.** *Cut from 83 (ADR 0032 §§ 1–4). Depends on 263 (the ask's shape),
  done, and nothing open.* `alo-renderer`: every answer that can run script
  — `Loaded`, `Acted`, and the answer to the new message — carries a bounded
  list of fetch asks, each with the renderer's number, the resolved URL, the
  method, the page's headers, the body bytes and `alo-net`'s `Mode`,
  `Credentials`, redirect mode and referrer `Policy`; `ToRenderer::Fetched`
  carries a number and either the filtered response or a failure with no
  reason in it. The wire format for both, with hostile input refused rather
  than panicking. The browser process's decision, beside `navigate.rs`, in
  § 3's order and each refusal named and recorded: the URL and its 2 MiB
  bound; `http`/`https` only; the header policy's `connect-src` from its own
  copy; mixed content; CORS and the preflight cache with the document's
  origin as the asker; credentials and the jar under the top-level site; the
  referrer from its own copy of the document's URL; the cause from which
  message was answered (`Tabs::a_page_fetching`, `Tabs::an_agent_acting`).
  A forbidden header, a `navigate` mode or a body on a `GET` in an ask is
  refused as a renderer that broke the boundary. The asks per answer and in
  flight per document bounded in the browser process, each number in the
  code with its reason. § 4's filter as one function from a request and a
  response to what crosses: `basic` without `Set-Cookie`, `cors` with only
  the readable headers, `opaque` and `opaqueredirect` with status 0 and
  **no bytes**, and a body larger than one message a failure. As navigation
  does today, the decided request is handed to whoever drives `Tabs`, which
  makes it with `Purpose::Fetch`. *Closes when:* tests show a same-origin
  ask in a `Load`'s answer decided as the document's and one in an `Act`'s
  as the agent's; a `no-cors` cross-origin answer crossing with no body
  bytes in the message; `Set-Cookie` never crossing; `connect-src 'none'`
  and an `https` page asking for `http` each refused by name and recorded;
  a `same-origin` ask to another origin refused before anything is sent; a
  CORS failure crossing as a failure with no reason in it; a forbidden
  header refused as a broken boundary; an ask past either bound a failure
  said among the issues; and malformed, truncated and adversarial bytes in
  either direction refused, never a panic.
  **Built (iteration 214).** `alo-renderer`: `fetch.rs` is what crosses —
  `FetchAsk` out (number, URL, method, headers, body, and `alo-net`'s `Mode`,
  `Credentials`, the new `redirect::Mode` and referrer `Policy`) and
  `Fetched` back (`Answer::Response(Readable)` of a `Kind` — basic, cors,
  opaque, opaqueredirect — or `Answer::NetworkError`, with no reason).
  `Loaded` and `Acted` carry `fetches`; `ToRenderer::Fetched` is answered by
  the new `FromRenderer::Delivered { issues, navigation, fetches }`.
  `wire/fetch.rs` encodes both, every tag from a closed list, and refuses an
  opaque answer carrying anything. `fetch_decide.rs` is the decision in § 3's
  order after the boundary check (`Broke`: navigate mode, a forbidden or
  malformed method or header, a body on a read, a `no-cors` request a form
  could not send), each refusal a named `Rule`, recordable with
  `Refusal::record`; the made `Fetch` answers `cookies` from the jar and
  `asking_first` from the preflight cache under the document's top-level
  site. `fetch_filter.rs` is § 4's one function, a body too large for one
  message (`wire::fetched_size`) a failure. `fetch_owed.rs` is what a
  document is owed and the two bounds, `MOST_IN_FLIGHT` and
  `MOST_ASKED_AT_ONCE`, 64 each with the reason beside them. `Tabs` decides
  every answer's asks as it passes (`Tab::loaded`, `acted`, `delivered`)
  against its own copy of the document's address and header policy, hands
  them over with `Tabs::fetches`, and delivers each answer with
  `Tabs::fetched` only while the document that asked is owed it. `alo-net`:
  `forbidden.rs` (Fetch's forbidden methods and request headers, one list
  for both sides) and `redirect::Mode`. A renderer still sends no asks and
  answers a delivery with "nothing on this page is waiting" until 335.
  **Making** a decided fetch — sending the preflight, the cookies and the
  request through a `Pool` with the redirect mode applied per hop, keeping
  `Set-Cookie`, and filtering — is whoever drives `Tabs`, as navigation's
  going is; that is cut as **item 338**. Tests: `fetch_decide`,
  `fetch_filter`, `fetch_owed` and `tab` unit tests (the cause by which
  message was answered, an answer delivered only to the document owed it),
  `alo-net`'s `forbidden` tests, and
  `tests/a_pages_fetch_crosses_the_boundary.rs` (round trips, every prefix
  cut and every byte changed in both directions, tags nobody has, opaque
  answers carrying something). Checked by mutation: an `Act`'s asks given
  the document's cause, `Set-Cookie` not stripped, and an opaque answer
  given the body each fail a test.

- [x] **338. A decided fetch is made.** *Cut from 334 (iteration 214).
  Depends on 334, done.* Whoever drives `Tabs` (`alo-window`'s conductor)
  takes `Tabs::fetches`, and for each `Decided::Make` sends
  `Fetch::asking_first`'s `OPTIONS` when there is one and checks it with
  `Preflights::allowed`, adds `Fetch::cookies`, makes the request with
  `Purpose::Fetch` through its `Pool` — following redirects only under
  `redirect::Mode::Follow`, deciding `Origin` and CORS again on a hop to
  another origin as Fetch does, and stopping at the first under `Manual` —
  keeps a response's `Set-Cookie` in the jar only when
  `Fetch::sends_credentials`, keeps the reason phrase for the status text,
  and hands the response to `fetch_filter::filter`; each refusal is answered
  with `Refusal::answer`, said to the person and recorded; each answer goes
  back with `Tabs::fetched`, and what that answer asks is made in turn,
  never inside the delivery that asked. *Closes when:* a page served by a
  local test server fetches a same-origin text, a cross-origin resource
  with and without `Access-Control-Allow-Origin`, and a redirect under each
  mode, and the record shows each request with its cause, the preflight
  before the request it asked about, and a refusal by its rule.
  **Built (iteration 216).** `alo-renderer`: `fetch_make.rs` makes one
  decided `Fetch` through a session's `Network` (its `Pool`, `Jar` and
  `Preflights`), hop by hop. Each hop's `Origin` (the document's, or `null`
  once a redirect has gone from one origin to a third — Fetch's tainted
  origin), `Referer` (from the document's URL under the page's policy, for
  where the hop goes) and `Cookie` (the jar under the document's top-level
  site, only while the credentials mode lets the hop carry them) are decided
  again; a `cors` hop to another origin is asked about first when the
  preflight cache does not cover it, the `OPTIONS` made and recorded before
  the request and needing an ok status; the hop is made with the new
  `Pool::hop` (one exchange, from the cache where it can, a redirect handed
  back); its `Set-Cookie` is kept by the new `Jar::keep_what_was_set` when
  the hop carried credentials (a site embedded in another may set only a
  `SameSite=None` cookie); every answer on a CORS chain, a redirect
  included, must agree (`cors::agreed_to_be_read`, without the same-origin
  shortcut, against `null` once tainted); and under `follow` the next hop
  is refused before it is sent — and recorded with `Pool::refused` — when it
  is a circle or past twenty hops, leaves a `same-origin` fetch's origin,
  carries credentials in its URL to another origin under `cors`, or is
  insecure from a secure page. `fetch_filter::filter` takes a `Route`
  (redirected, left the origin, tainted): a chain that left is never
  `basic`, a `3xx` without `Location` is an answer, and the status text is
  `alo-net`'s new `Response::reason`, which the HTTP/1.1 reader keeps
  (empty over HTTP/2) and the disk cache stores from format version 2.
  `fetch_answering.rs`'s `Answering` queues each tab's decided fetches with
  the document that asked, answers the oldest one at a time — making it, or
  recording a refusal (`Refusal::record` now writes to a `Pool`) — delivers
  it with `Tabs::fetched`, queues what the delivery asks behind the rest,
  and makes nothing for a document that has gone. `alo-window`: the
  conductor holds the `Network` and an `Answering`, takes a load's fetches,
  and between looking at its orders makes one fetch, paints the selected
  tab again after the delivery and says each failure's reason; `alo` starts
  it over a session pool trusting this machine. **Cut**: a redirect hop
  judged against `connect-src` with source paths ignored, as **item 340**.
  Tests: `alo-renderer/tests/a_decided_fetch_is_made.rs` — two local
  servers on two ports, a page at one loaded into real `Tabs` over the
  confined binary, every ask made through a real `Pool`: what the page
  heard (same-origin text with its reason phrase, cross-origin with and
  without `Access-Control-Allow-Origin`, a preflighted `PUT`, a redirect
  under `follow`, `manual` and `error`, two refusals), what each server was
  sent (cookies home only, `Origin` and `Referer` per hop, the preflight's
  `Access-Control-Request-*`, nothing refused ever sent), the record
  (every line the document's cause and `Purpose::Fetch`, `OPTIONS` before
  `PUT`, each refusal by its rule), a chain through another origin saying
  `null` and carrying no cookies home, a redirect circle refused, `omit`,
  `no-cors`, a redirect that did not agree, and a gone page's fetch made for
  nobody; `alo-window/tests/a_page_that_fetches_in_the_window.rs` — the
  conductor drawing the page again after each answer and saying a failure,
  and a page that never stops fetching not keeping the window open;
  `fetch_filter`'s route tests, `alo-net`'s `Headers::remove`, `Jar`'s
  `keep_what_was_set`, `cors::agreed_to_be_read`/`exposed` and the cache
  record's version 2. Checked by mutation, each restored and each failing a
  test: no tainted origin, same-origin credentials after leaving, no
  preflight, no per-hop CORS check, no `Set-Cookie` kept, a gone document's
  fetch made, and a chain that left read as `basic`.

- [x] **340. A redirect a page's fetch follows is judged by `connect-src`.**
  *Cut from 338 (iteration 216). Depends on nothing open.* CSP3's *does
  request match source list* ignores a source's path for a request that has
  been redirected, and `alo-net`'s `csp` cannot tell a redirect from a first
  request, so `fetch_make` judges only a fetch's first URL against the
  document's `connect-src` (in `fetch_decide`). Checking each hop with paths
  would refuse what every browser follows; not checking lets a page's
  allowed origin redirect it to one its policy forbids. *Needs:* the
  matching told whether the request was redirected (`csp_source`'s
  `HostSource::matches` and the callers above it), and `fetch_make` asking
  for each hop after the first with `Purpose::Fetch` against a copy of the
  document's enforced policies carried on the decided `Fetch`. Feature
  contract: `docs/features.md`'s `fetch()` line. *Closes when:* a fetch
  allowed by `connect-src https://a.example/api/` that `a.example` redirects
  to `https://a.example/other` is followed, one it redirects to
  `https://b.example/` is refused before it is sent and recorded by the
  rule, both against a local server, and `csp_source`'s unit tests show a
  path ignored only after a redirect.
  **Built (iteration 217).** `alo-net`: `Request::redirected`, Fetch's
  redirect count above zero, set by `redirect::next` and by nothing else;
  `csp`'s `Directive::permits` takes the request and hands `redirected` to
  `Source::matches` and `HostSource::matches`, which ignore a host source's
  path for a redirected request (CSP3's *does url match expression in origin
  with redirect count*) and still check its scheme, host and port; `Policies`
  (and its private `Policy` and `Directive`) are `PartialEq`/`Eq` so a decided
  `Fetch` can carry them. `alo-renderer`: `Fetch::policies`, the document's
  policies as `fetch_decide` judged the first hop by; `fetch_make`'s
  `Hop::then` asks them about each hop after the first, after the mixed
  content check, and a refusal is recorded with `Pool::refused` as "it was
  redirected, and" the policy's own words, and said to the person. Tests:
  `alo-renderer/tests/a_decided_fetch_is_made.rs`'s
  `a_redirect_is_judged_by_connect_src_with_its_paths_ignored` — a page under
  `connect-src {other}/api/` against two local servers: `/api/stay`
  redirected to `/other` on the same server is followed and read
  (`cors 200 OK true other`), `/api/away` redirected to the page's own
  origin, which the policy does not name, is refused, never sent and recorded
  by the rule, and a first fetch of `/other` is refused by the path, never
  sent; `csp_source`'s `a_path_is_ignored_only_once_a_redirect_has_led_there`
  (another host, scheme, port or subdomain still refused, `'self'` and
  `'none'` not widened) and `csp`'s
  `a_redirected_request_is_judged_without_the_paths_a_first_one_is_judged_by`
  (a hop from `redirect::next` says it was redirected). Checked by mutation,
  each restored: no per-hop check fails the closing test (the refused hop is
  read), and paths not ignored after a redirect fails it and both unit tests.

- [x] **335. `fetch()` in a page.** *Cut from 83 (ADR 0032 §§ 1, 4 and 7).
  Depends on 333 and 334.* `alo-bindings`: `fetch` on the global object,
  reading its `init` (`method`, `headers` as a plain object, a string
  `body`, `mode`, `credentials`, `redirect`, `referrerPolicy`), dropping
  forbidden headers as the `Headers` guard does, refusing by name what is
  not built (a `Request` argument, a body that is not a string, `data:` and
  `blob:` URLs, a `signal`), recording the ask in the document cell under a
  fresh number and answering a pending promise; and the `Response` it is
  settled with, read-only: `ok`, `status`, `statusText`, `url`, `type`,
  `redirected`, `headers.get` and `headers.has`, and `text()`. A failure
  rejects with one `TypeError` whatever the reason. `alo-renderer` settles
  the promise in the `Fetched` task, with a checkpoint after it, and lets
  go of the waiting promise when its page goes. `alo-corpus`: a case that
  fetches states its address and its frozen responses (§ 7), a URL it froze
  none for is a network error, and `origin.txt` says which. *Closes when:*
  `alo-downloads`' script runs past its line 19 and its `.then` or `.catch`
  decides each button, pinned in `tests/alo_downloads.rs`; its reference is
  moved and read; and a bindings test shows a same-origin text body read,
  an opaque response's `status` 0 and empty body, and a refused fetch's
  `TypeError`. If a response for the page's installers cannot be frozen
  with provenance, the case freezes none, and what it then pins — buttons
  marked *Building — available shortly* — is said in `origin.txt` as what
  the page does offline.
  **Built (iteration 215).** `alo-bindings`: `fetch` on the global object
  (`fetch.rs`) makes its promise and asks for a call of the **request
  steps**, a second native only `fetch` holds, with `Want::Catch`, so
  everything they throw — a refused argument, a bad URL, a forbidden method,
  a getter in `init` that throws — rejects rather than escapes. The steps
  convert `input` with `ToString` and `init` member by member in Web IDL's
  lexicographic order (`fetch_init.rs`), each getter and `toString` asked
  for and come back to, then run Fetch's `Request` steps: resolve against
  the base URL, refuse credentials in it, refuse `data:` and `blob:` by
  name, default and check `mode` (`navigate` a `TypeError`), `credentials`,
  `redirect` and `referrerPolicy`, normalise and check the method, drop
  forbidden headers and, under `no-cors`, any a form could not have sent
  (`alo-net`'s own lists, `cors::a_form_could_have_sent` made public), add
  `Content-Type: text/plain;charset=UTF-8` for a string body, and record the
  ask in the document cell (`fetching.rs`) under the next number. Refused by
  name: a body that is not a string, `cache`, `integrity`, `keepalive`,
  `priority`, `referrer` or `signal` set to other than their default,
  headers as pairs or a `Headers`, a header behind a getter or whose value
  is an object, and a header value past `0x7F`. The asks waiting to be
  taken are bounded at 32 MiB (`MOST_ASKED_BYTES`, half of one message);
  past it a fetch rejects as a failure. The waiting promises are strong
  edges of the cell and its footprint counts the asks. `response.rs` and
  `headers.rs` are the read-only `Response` (`type`, `url`, `redirected`,
  `status`, `ok`, `statusText`, `headers`, `bodyUsed`, `text()`, a second
  read rejecting) and `Headers` (`get`, `has`), no constructor on the
  global. `delivering.rs` is the task: one call of a native the cell holds,
  which resolves the promise through `%ResolvePromise%` or rejects it with
  one `TypeError` (`fetch::FAILED`). `alo-js` gained `Intrinsics::error`;
  `alo-url` gained `includes_credentials`. `alo-renderer`: `Held` offers
  `fetch` when a page's script first runs, `take_fetches` and `deliver`;
  `EventLoop::queue_delivery` roots the response across queueing;
  `deliver.rs` runs the task; `Loaded`, `Acted` and `Delivered` carry the
  asks, and a delivery draws the page again. `alo-corpus`: a case's
  `address.txt` and `responses.txt` (raw HTTP/1.1 responses), answered by
  `answering.rs` through `fetch_decide::decide` and `fetch_filter::filter`;
  an unfrozen URL is a network error, listed in `Answered::unfrozen`.
  `alo-downloads` is served from `https://alomails.com/download/` and froze
  no installer (none is in any repository); offline both buttons are marked
  *Building — available shortly* and lose their `href`, and the script then
  stops at `a.style`, which is item **339**, opened by this page. Tests:
  `alo-bindings/tests/what_a_page_fetches.rs` (a same-origin text body
  read, an opaque response's status 0 and empty body, a failed fetch's one
  `TypeError`, every request-step refusal rejecting and asking nothing,
  headers and the guard, the init's order and a getter's throw rejecting,
  every refusal by name, a body read twice, an answer nothing waits for —
  each also under a collection at every allocation);
  `alo-renderer/tests/a_page_fetches.rs` (a `Load`'s, an `Act`'s and a
  delivery's asks, the page drawn again, a new page letting go, and real
  `Tabs` over the confined binary deciding and delivering);
  `alo-corpus/tests/alo_downloads.rs` (offline, and a server with one
  installer and not the other); `answering.rs`'s unit tests. Checked by
  mutation: not tracing the waiting promises fails three stress runs; not
  dropping forbidden headers, and not drawing again after a delivery, each
  fail a test.

- [x] **339. An element's `style`, from a page.** *Cut from 89 (CSSOM),
  opened by `alo-downloads` (iteration 215).* The page's `mark(a)` sets
  `a.style.background`, `a.style.cursor` and `a.style.pointerEvents` to
  grey a button it has marked, and stops at the first: "TypeError: cannot
  write property 'background' of undefined". What it needs is both halves of
  an inline style: the `style` attribute taking part in the cascade (CSS
  Style Attributes — not applied today, so `<p style="color: red">` is not
  red), and an element's `style` as a `CSSStyleDeclaration` whose named
  properties write the attribute through `alo-dom`'s operations (ADR 0017
  § 5). Feature contract: `docs/features.md`'s CSSOM line. *Depends on
  nothing open.* *Closes when:* a style attribute is cascaded as the
  specification places it, in numbers and a reference render; `alo-downloads`'
  `mark(a)` runs to its end and both offline buttons are drawn `#c7bfb2`,
  pinned in `tests/alo_downloads.rs` and the moved reference. *Not yet read
  for an ADR*: the iteration that takes it says whether the declaration's
  shorthand handling (`background` sets eight longhands) is a decision or a
  specification to follow, before building.
  **Decided (iteration 218): ADR 0033, accepted.** It was a decision.
  CSSOM presumes an engine that parses every value against its property's
  grammar when it is written, and this one keeps a value as written and
  does not split `background` (`alo-css`'s `declaration.rs` and
  `shorthand.rs`). In short: the `style` attribute *is* the inline
  declaration block, the only copy, and is cascaded above every selector
  and below `!important` (Cascade 4's element-attached step).
  `element.style` holds only its element, like `classList`. It reads the
  attribute every time and writes it back through `alo-dom`. It names only
  the properties this engine acts on, from one list in `alo-css`. A value
  is kept when a style sheet would keep it. A shorthand read by kind stays
  one declaration, and setting it removes its longhands from the block.
  Nothing is built, and this item stays open: it closes when **341** and
  **342** do (344 was cut from 342 and is done). `cursor` and `pointer-events` are acted on by no stage, so
  the page's last two writes set ordinary properties of the object, as in
  every engine for a name it does not support (ADR 0033 § 4).
  **Done (iteration 221)**, as 341 and 342 closed: the attribute is
  cascaded (341, corpus case `style-attributes`), and `alo-downloads`'
  `mark(a)` runs to its end with both offline buttons drawn `#c7bfb2`
  (342, `tests/alo_downloads.rs` and the moved reference). `el.style[0]`
  was cut to 345 and 343 remains, neither part of this item's close.

- [x] **341. The `style` attribute, cascaded.** *Cut from 339 (ADR 0033
  § 1). Depends on nothing open.* `alo-css` parses an attribute's value as
  the contents of a declaration block, by the same parser, refusals and
  shorthand splitting as a sheet's block, and says what it dropped.
  `alo-style`'s cascade gains Cascade 4's element-attached step, between
  origin-and-importance and specificity. The attribute is read from the
  document on every draw, for HTML and SVG elements, never inside a
  `<template>`. Feature contract: `docs/features.md`'s CSSOM line, which
  says what a style attribute does. *Closes when:* `<p style="color: red">`
  is red. A normal inline declaration beats an id selector. An important
  sheet declaration beats a normal inline one. An important inline
  declaration beats an important sheet declaration with an id selector.
  `style="fill: …"` beats a presentation attribute. A dropped declaration
  is said. Each is pinned in a unit test. A new corpus case shows sizes and
  colours set only by `style` attributes, with a layout assertion in
  numbers and a reference render. Every corpus reference that moves is
  explained by a `style` attribute its page carries.
  **Done (iteration 219).** `alo-css`'s `parse_declaration_list` reads the
  attribute through the sheet's own `parse_declarations`.
  `alo-style`'s `attached.rs` gives an HTML or SVG element's attribute as
  declarations, saying each one dropped with its element. The cascade's
  `Contender::attached` is asked between level and specificity
  (`Applicable::gather_attached`), and `resolve_measured` reads the
  attribute for every element on every call. Pinned in `cascade.rs`'s unit
  tests (each ordering above, a presentation attribute, the last in the
  attribute winning), `computed.rs`'s (red, inherited, a refusal said),
  `attached.rs`'s and `parse.rs`'s (a hostile list refused, never
  panicking), and in corpus case `style-attributes`, whose `layout.txt` and
  `render.png` are the layout assertion and reference render. No other
  corpus reference moved: no other case's page carries a `style`
  attribute.

- [x] **344. What an inline block is to a script, in `alo-css`.** *Cut
  from 342 (iteration 220): ADR 0033 §§ 3–5 without a heap, so that 342 is
  only the binding.* `properties.rs`: one sorted list of the properties
  this engine acts on, each naming the crates that read it. Each reading
  crate gets a test that every property it reads is listed with it, and the
  list gets a test that each entry names a crate. `longhand.rs`: the
  longhands each shorthand covers, the kind-read ones included. `inline.rs`:
  the block parsed from the attribute, CSSOM's `setProperty`,
  `removeProperty`, `getPropertyValue`, `getPropertyPriority`, `length`
  and `item` over it, and a serialiser that writes only what was written.
  Hostile input: a value with `;`, `!important`, unbalanced brackets, a NUL
  or a megabyte is refused or kept exactly as § 5 says, never panicking.
  Feature contract: `docs/features.md`'s CSSOM line. *Closes when:* each of
  those is pinned in a unit test, and each reading crate's test fails when
  that crate reads a property the list does not name, or the list names one
  it does not read, shown by mutation.
  **Done (iteration 220).** `alo-css`'s `properties.rs` (`SUPPORTED`,
  `is_supported`, `read_by`, and `named_in`, the scan each crate's test
  uses), `longhand.rs` and `inline.rs` (`InlineStyle`, `Edit`).
  `DeclarationBlock` now knows which of its declarations were written
  (`written()`), so a counted shorthand's implied longhands are never
  serialised. `tests/what_it_reads_is_listed.rs` in `alo-style`, `alo-box`,
  `alo-layout`, `alo-paint`, `alo-svg` and `alo-renderer`. `alo-svg`'s also
  holds the eight properties it reads only to say they are not applied off
  the list. Checked by mutation: a `get("cursor")` added to `alo-box`, and
  `z-index` taken off the list, each fail their crate's test.

- [x] **342. `element.style`, a `CSSStyleDeclaration`.** *Cut from 339
  (ADR 0033 §§ 3–6). Depends on 341 and 344.* The list, the longhand table,
  the block's edits and its serialiser are 344's, in `alo-css`, and this
  item uses them rather than writing its own. `alo-bindings`' `style` on
  every HTML and SVG element is `[SameObject, PutForwards=cssText]`, an
  embedder cell holding its element's wrapper as `classList` does. Its
  members are those of ADR 0033 § 6, each reading the attribute and
  writing it back through `alo-dom` only when something changed. Hostile
  input: a value with `;`, `!important`, unbalanced brackets, a NUL or a
  megabyte is refused or kept exactly as § 5 says, never panicking.
  Feature contract: `docs/features.md`'s CSSOM line. *Closes when:*
  `alo-downloads`' `mark(a)` runs to its end with no issue. Both offline
  buttons are drawn `#c7bfb2`. `cursor` and `pointerEvents` are ordinary
  own properties of the declaration. All of this is pinned in
  `tests/alo_downloads.rs` and the moved reference, with this item's own
  `alo-bindings` tests for every member and a collection at every
  allocation.
  **Done (iteration 221).** `alo-bindings`' `style_declaration.rs` (the
  cell, kept by the wrapper as `classList` is), `interface/
  css_style_declaration.rs` (the members), `interface/
  element_css_inline_style.rs` (`style`, with `[PutForwards=cssText]` as
  a real `[[Set]]`) and `style_names.rs` (CSSOM's camel-cased, WebKit-cased
  and dashed names for each property in `alo-css`'s list). `style` is on
  `HTMLElement.prototype` and on a new `SVGElement.prototype`, between an
  SVG element and `Element`. Every member is pinned in
  `tests/what_an_elements_style_is.rs`, each script run plain and with
  the collector at every allocation, with the document's change count:
  a write that changes nothing counts nothing. `alo-downloads`' script
  runs to its end; both offline buttons are drawn `rgb(199 191 178)`, in
  `tests/alo_downloads.rs` and the moved `display.txt` and `render.png`.
  **Cut:** the indexed getter, `el.style[0]`, to 345. The ADR's "removes
  the attribute when nothing is left" was not built, because CSSOM's
  update steps set it to `""`; ADR 0033 carries the correction.

- [x] **343. A page's `style-src`, applied to its inline style.** *Cut from
  339 (ADR 0033 § 2). Depends on 341; the `CSSStyleDeclaration` half
  depends on 342.* The renderer applies the policies it already holds for
  scripts to every `<style>` element and every `style` attribute, through
  `csp::Policies::allows_inline` with `Inline::Style`. A refused one
  contributes nothing and is reported as an inline script's is (item 237).
  A `style` attribute last written through `element.style` is not
  refused. `alo-dom` records, per element, whether the attribute's current
  value came from the declaration. A blocked attribute reads as empty
  through `element.style`. *Needs:* if who records that turns out to be a
  decision about `alo-dom`'s element, an ADR first (ADR 0033, *What this
  does not decide*). *Closes when:* under `style-src 'self'`, a `<style>`
  and a `style` attribute are both refused and reported. The same attribute
  is applied under a digest with `'unsafe-hashes'`. A value written through
  `element.style` is applied under the refusing policy. Each is pinned in
  a renderer test over a real load.
  **Decided (iteration 222): ADR 0034, accepted.** The record was a
  decision about `alo-dom`'s element. A flag would have to be cleared by
  every other writer of a public `attrs`, and a missed one is a bypass. So:
  - `Element` remembers **the text `element.style` last wrote**, set only
    by one new counted `alo-dom` operation. An attribute is the
    declaration's own exactly when its value equals that text.
  - `style-src` is asked **at every draw**, of every policy the page holds
    then: its headers and every `<meta>` the parser made. The renderer
    keeps the `<meta>` policies for the page's life.
  - A `<style>` presents its nonce by the script's nonceable rule. An
    attribute's digest counts only under `'unsafe-hashes'`.
  - One function in `alo-bindings` answers whether an element's `style` is
    applied. The renderer's draw and `element.style` both ask it, so a
    refused attribute reads as `""` and a write starts from empty. The
    page's policies are stated into the document's cell, as its URL is.
  - An `Objection` carries its placement on the wire, and each element,
    placement and text is objected to once per page. The load's draw posts
    in `Loaded`. A later draw's objections are only said, and posting them
    is cut to **346**.
  The closing condition above stands. Add to it: the record's equality
  rule, a setAttribute replacing a declaration-written value being refused,
  and a refused attribute reading `""` through `element.style`, each pinned
  in a test. *Eligible and next.*
  **Done (iteration 223).** `alo-dom`'s `declared.rs`
  (`Document::set_declared_style`, `Element::style_is_declared`, the record
  counted in the footprint) and `nonce.rs` (*is element nonceable*, moved
  out of `scripts.rs` for a `<style>` to share); `Sheet::Written` names its
  element and nonce. `alo-style`'s `resolve_admitting`. `alo-bindings`'
  `style_policy.rs` (`applied`, the one function, and `state`), the
  page's policies in `DocumentCell`, and `CSSStyleDeclaration` reading
  through `applied` and writing through `set_declared_style`.
  `alo-renderer`'s `inline_style.rs` (`Judged`: each `<style>` and `style`
  attribute asked at every draw, refusals and watched objections said, at
  most 256 lines and 64 objections a draw, then counted), the pipeline
  dropping what is refused, `at_load` keeping every `<meta>` policy for the
  page's life (those after the last script too) and telling the heap, and
  `Objection.placement` on the wire, checked by the browser process.
  Pinned in `tests/a_pages_style_under_its_policy.rs` over real loads (both
  refused and reported under `style-src 'self'`; the digest only with
  `'unsafe-hashes'`; a nonce; `element.style`'s write applied and not
  reported; a refused attribute read as `""` and a write starting from
  nothing; `setAttribute` replacing the declaration's text refused and the
  same text written back admitted; a `<meta>` reaching back and told to the
  heap; a watched policy; a flood counted), and in unit tests in each crate
  and `messages_across_a_boundary.rs`. **Left to 346:** remembering, for
  the page's life, which element, placement and text was objected to
  (ADR 0034 § 4). Only the load's single draw posts today, and it visits
  each element once, so the memory has nothing to decide until a later
  draw's objections are carried.

- [x] **346. Inline style refused after load, reported.** *Cut from 343 by
  ADR 0034 § 4. Depends on 343.* A draw made after a script, an agent or a
  fetch's answer changed the page can find inline style that a policy
  objects to. Those objections are carried in the next `Acted` or
  `Delivered` answer, under the same bound of 64 and the same
  once-per-element-placement-and-text rule, which this item builds: the
  renderer remembers, for the page's life, each element, placement and
  text it has objected to, those the load carried included (343 left it
  here, since one draw visits each element once). One found by a draw that only
  `Paint` or `ReadTree` asked for waits for the next such answer. More
  than the bound are counted and said. Feature contract:
  `docs/features.md`'s *A page's author is told* line. *Closes when:* a
  script that sets a refused `style` attribute after load produces one
  report from the browser process, and the same attribute set twice
  produces one. A flood is bounded and said. Each is pinned in a renderer
  test over a real load and in `wire.rs`'s round trip.
  **Done (iteration 224).** `alo-renderer`'s `objected.rs` (`Objected`:
  each element, placement and SHA-256 of the text a header policy objected
  to, kept for the page's life and forgotten by a new load, and what draws
  found waiting, at most 64 and a count, for the next answer that carries
  objections). `Judged::of` objects only the first time it meets one, so a
  refusal is still made and said at every draw. `Acted` and `Delivered`
  gained `objections`, written and read by `wire.rs`'s one `objections`
  pair and refused past 64. The load's carry became `Objected::take`.
  Pinned in `tests/style_refused_after_load.rs` over real loads: a
  listener's attribute reported once per text, with the browser process's
  `violations::reports` writing one post to the policy's `report-uri`; a
  `Paint`'s find carried by the next `Acted`; a fetch's reaction carried by
  its `Delivered`; a flood of 100 carried as 64 and "36 more", and not found
  again; a new load forgetting. Also in `objected.rs`' and
  `inline_style.rs`' unit tests (hostile text, a second draw objecting to
  nothing), and in `messages_across_a_boundary.rs` (both answers' round
  trips, every prefix refused, 65 refused). Checked by mutation: a memory
  that forgets fails two of the new renderer tests.

- [ ] **345. `el.style[0]`, a declaration's indexed getter.** *Cut from 342
  (iteration 221).* ADR 0033 § 6 has `style[0]` come with `item()`, and
  `item()` is built. A `CSSStyleDeclaration` is a legacy platform object
  whose supported indices are the `style` attribute's declarations, read
  live. An embedder cell answers `[[GetOwnProperty]]` only from what it
  stores (`alo-js`'s `Internal::own_property` hands back a reference), and
  the names would have to be interned strings made at the moment of the
  read, which allocates. So this needs `alo-js` to let an exotic object
  answer an own property it computes, with the heap in hand. That is the
  same thing `classList[0]` waits on (328). *Depends on* that hook, which
  is the first part of whichever of 328 and 345 a page opens first.
  *Opened by no page yet:* take it when a frozen page reads `style[i]` or
  walks the declaration by index. *Closes when:* `el.style[0]` is the
  first declaration's name, an index past the end is `undefined`,
  `Object.keys(el.style)` lists the indices first, and a write to an
  index is refused, each pinned in `alo-bindings` with the collector at
  every allocation.

- [ ] **336. `XMLHttpRequest`, asynchronous.** *Cut from 83 (ADR 0032 § 6).
  Depends on 334 and on event dispatch (254, done).* The same ask, delivered
  as `readystatechange`, `load`, `error` and `loadend` events rather than a
  promise; a synchronous `open(…, false)` refused by name. *Opened by a
  frozen page that uses one, and not before.*

- [ ] **347. A loaded page's linked style sheets.** *Opened by a page
  (iteration 225):* `alo-workplace`'s
  `products/sites/alo-sites/tests/golden/section_cta.html`, a page alo
  Sites publishes for its customers, links its whole style sheet
  (`<link rel="stylesheet" href="/assets/site.css">`) and carries an inline
  script, as every page alo Sites publishes does. A page that runs script
  is loaded by a renderer, and **nothing hands a renderer a page's linked
  sheet**: `alo-renderer`'s `renderer.rs` draws with none, and only
  `alo-window`'s command-line `opening.rs` gives one any sheet. So the page
  is drawn unstyled in the window, and `alo-corpus`' `rendering.rs`
  refuses it as a case by name. *Depends on 334 and 335 (the ask's
  shape, both done). Needs ADR*, because which process finds a sheet, what crosses and what a
  renderer may then hold are decisions ADR 0032 made for a fetch and did
  not make for a sheet.
  **Decided (iteration 225): ADR 0035, accepted.** The renderer asks for
  each linked sheet's URL once per document, in the answer to the message
  whose work found it, as ADR 0032 asks for a fetch. The browser process
  decides it as a style request (`style-src`, mixed content, CORS only for
  `crossorigin`, cookies, referrer, the cause from which message was
  answered). The body crosses only if the status is 2xx and the
  `Content-Type` is `text/css`, so a renderer never holds the bytes of
  something else a page named. A sheet's answer is a task, and the next
  draw applies it. The browser process shows no first frame until the
  load's sheets are answered, within a bound of its own. Nothing is built,
  and this item stays open: it closes when **348** and **349** do, and
  **351**, which iteration 226 cut from 348. *348 and 349 are done
  (iterations 226 and 227); 351 remains.*

- [x] **348. A linked sheet asked for, decided and delivered.** *Cut from
  347 (ADR 0035 §§ 1–5). Depends on nothing open.* `alo-renderer`: a sheet
  ask in `Loaded`, `Acted` and `Delivered` (number, resolved URL,
  `crossorigin` as mode and credentials, `referrerpolicy`, nonce), one per
  URL per document, bounded per answer and per document with the numbers'
  reasons in the code; a `<meta>` policy applied before asking; a link with
  `integrity` and a `data:` sheet refused by name. The browser process's
  decision in ADR 0032 § 3's order with `Purpose::Style`, `file:` only from
  a `file:` document; the check that only a 2xx `text/css` body crosses; a
  failure said in the same words whatever happened, the reason recorded; a
  new `ToRenderer` message and its wire encoding, read as a stranger's
  bytes; the renderer decoding UTF-8 and keeping what arrived by URL;
  `alo-window`'s conductor making the asks between orders and presenting
  no first frame until the load's sheets are answered or its bound passes.
  *Closes when:* a renderer's page with a linked sheet is drawn with it,
  in a test that reads the computed colour after the delivery; a
  cross-origin response that is not `text/css` sends no body across, in a
  test that reads the message; a header `style-src` refusing it makes no
  request and records the refusal; two links to one URL make one request;
  a link a script adds is asked for in that task's answer; and the window
  presents the page after its sheet's answer, not before.
  **Cut (iteration 226):** the last clause, ADR 0035 § 5, is **351**. While
  building it, a gap the ADR did not see turned up: the conductor makes each
  request on its one thread and waits for it, and `alo-net` bounds a read,
  not an exchange, so a server that trickles holds the conductor past any
  bound the window could set. Holding the first frame back would then hold
  it back for as long as that server likes.
  **Done (iteration 226).** `alo-renderer`'s `linked.rs` asks for each linked
  sheet's resolved URL once per document, in the answer to the message whose
  work found it. Each ask carries a number, the `crossorigin` attribute as
  mode and credentials, `referrerpolicy` and the nonce (`sheet.rs`), at most
  `MOST_SHEETS` (64) per document. It applies `<meta>` policies through
  `alo-net`'s new `Policies::allows_load`, and refuses `data:` and
  `integrity` by name, each said once. `sheet_decide.rs` decides an ask as a
  style request, in ADR 0032 § 3's order, with `file:` only from a `file:`
  document. `sheet_owed.rs` bounds a document's asks, refused ones counted.
  `sheet_make.rs` makes the request through `fetch_make`'s `hops`, now
  shared and asked with the nonce on every hop. It sends the body only for
  a 2xx `text/css` answer that fits one message, and says the reason
  otherwise. `ToRenderer::Sheet` and the asks in `Loaded`, `Acted` and
  `Delivered` cross the wire (`wire/sheet.rs`). The renderer decodes UTF-8,
  keeps sheets by URL, and draws them on the links the document has at
  each draw. `fetch_answering.rs` queues a document's sheets ahead of its
  fetches, and the window's conductor makes them and paints again.
  Tests: `a_pages_linked_sheet.rs` (computed colour after delivery, one ask
  for two links, a script's link in its task's answer, `<meta>` against
  header policy, `data:` and `integrity`), `a_linked_sheet_is_made.rs`
  (real servers and `Tabs`: pixels; no body for a cross-origin `text/html`
  answer, read off the message; a header `style-src` makes no request and is
  recorded; every hop is a style line caused by the document),
  `a_pages_sheet_crosses_the_boundary.rs` (every prefix and every changed
  byte of an ask or an answer refused or read, never a panic) and
  `alo-window`'s `a_page_styled_in_the_window.rs`.

- [x] **349. `alo-sites-cta`, frozen.** *Cut from 347 (ADR 0035 § 6).
  Depends on 348.* `alo-corpus` answers a loaded case's sheet asks from its
  `linked.txt`, each name resolved against `address.txt`, through ADR 0035
  § 3's check, with the type the file's extension stands for; a loaded case
  that links a picture stays refused by name (350). Then
  `section_cta.html` and the `site.css` beside it are frozen byte for byte
  from `alo-workplace` with their provenance (at `738de614`, the page last
  changed in `fbe5b972`, the sheet in `5be41b70`), served from
  `https://nordwind.alosites.com/`. *Closes when:* the case renders with its
  sheet applied, with a layout assertion and a reference render; its
  `origin.txt` says where it came from and what its analytics script does
  offline; and what the render shows wrong is opened as items, in the
  order the page meets them.
  **Done (iteration 227).** `alo-corpus`'s `sheets.rs` answers a loaded
  case's sheet asks: each ask is decided by `alo-renderer`'s
  `sheet_decide`, and each frozen file is found by its `linked.txt` name
  resolved against `address.txt`, typed by its extension through
  `alo-net`'s `schemes::from_extension` (now public) and held to
  `sheet_make::style_sheet` (now public), the rule that only a 2xx
  `text/css` answer crosses. `answering.rs` answers sheets ahead of fetches,
  as the browser process queues them, and refuses by name a file frozen for
  a URL the page never asked for — a picture, until 350. `Case::frozen`
  keeps each `linked.txt` line's name, file and bytes, and `Answered` gained
  `sheets`, `said` and the load's own `loaded` issues. `cases/alo-sites-cta`
  freezes `section_cta.html` and `site.css` byte for byte (SHA-256s in its
  `origin.txt`) from `https://nordwind.alosites.com/`. It renders with its
  sheet: a `#1d4ed8` band, `h2` at 28 px, two buttons 48.4 tall, 12 apart
  and centred. Tests: `tests/alo_sites_cta.rs` (one sheet asked for and
  answered, nothing unfrozen, the load's words; the band's height and
  colour and both buttons' boxes and colours in numbers and pixels; the
  skip link's fault), unit tests in `sheets.rs` (resolved by URL, unfrozen,
  a non-CSS file refused, the browser's refusals kept) and `rendering.rs`
  (drawn with its sheet, unfrozen said, a frozen picture refused by name).
  What the render shows wrong is opened as **352** (the skip link, met
  first, at the top of the page) and **353** (the script stops at `Date`).

- [x] **352. An absolutely positioned inline is taken out of flow.**
  *Opened by `alo-sites-cta` (iteration 227). Feature: `docs/features.md`
  stage 1, "Absolute and relative positioning". Depends on nothing open.*
  alo Sites' `.skip-link` is an `<a>` with `position: absolute; left:
  -999rem; top: 0`. CSS 2 § 9.7 blockifies an absolutely positioned box and
  takes it out of its line; `alo-layout` hands an absolute *block* to its
  layout algorithm as absolute (`engine.rs`), but this inline stays in an anonymous line at the top left of the page,
  painted on its `--surface` grey, and pushes the section down by one line
  of `body` (27.2 px). *Closes when:* the skip link's box is laid out at
  `left: -999rem` (x = -15984) and `top: 0`, out of flow, so the section
  starts at y = 0, in a layout assertion; `cases/alo-sites-cta`'s
  references move and say so; and an inline with `position: absolute` in a
  small case of its own is blockified, in numbers.
  `tests/alo_sites_cta.rs`' `the_skip_link_is_drawn_in_flow_which_is_item_352`
  pins today's fault, and this item changes it.
  **Done (iteration 228).** `alo-box` blockifies an absolutely positioned
  box where it decides an element's `display` (`tree.rs`' `display_of`, CSS
  Display § 2.7): only the outside changes (`Display::blockified`, in
  `display.rs`), so `inline-block` becomes `flow-root` and `inline-flex`
  `flex`, and `none` and `contents` make no box to blockify. Only
  `absolute` blockifies, because it is the only out-of-flow value layout
  places: `fixed` and `sticky` fall back to `static` there. The block-level
  box is never wrapped in a line, and layout's existing absolute path
  places it. The skip link is now at (-15984, 0), 161.1 × 43.2 (its text
  and `padding: 0.5rem 1rem`), and the section starts at y = 0.
  Tests: `alo-box` unit tests (blockified and in no line; the inside kept;
  `static`, `relative`, `fixed` and `sticky` left inline; `none` and
  `contents` make no box), `Display::blockified`'s own test, `alo-layout`'s
  `an_absolutely_positioned_inline_leaves_the_flow_and_its_line` (at its
  offsets, shrunk to what it holds, the block after it at the top), a new
  case `cases/absolute-inline`, and `tests/alo_sites_cta.rs`'
  `the_skip_link_is_out_of_flow_and_off_the_page`, which replaces the test
  that pinned the fault. `alo-sites-cta`'s references moved: the page is
  27.2 shorter, the anonymous line is gone and the link is a block.
  Checked by mutation: with the blockification switched off, the layout
  assertion and two of the box-tree tests fail.
  **Cut (iteration 228):** two faults this did not cause and did not fix,
  both already true of an absolutely positioned *block*, are **354** and
  **355**.

- [ ] **354. An out-of-flow box in a line does not break the line.**
  *Found by iteration 228 while building 352, not opened by a page. Feature:
  `docs/features.md` stage 1, "Absolute and relative positioning". Waits
  for a page.* A box with `position: absolute` among a line's content is
  block-level, so `alo-box`'s `arrange` wraps the content before and after
  it in two anonymous blocks: `<p>one <a style="position: absolute">two</a>
  three</p>` is two lines, 32 tall with `BlockFont`, where browsers keep
  one line of 16. It takes no room on the line, and with its insets `auto`
  it stands where it would have been in the line (its static position,
  CSS 2 § 10.3.7), which layout does not work out. *Closes when:* that
  paragraph is one line, in numbers; and an inset left `auto` puts the box
  at its static position, in numbers. *Opened by a frozen page that puts an
  absolutely positioned box in a line, and not before.*

- [ ] **355. An absolute box is placed against its nearest positioned
  ancestor.** *Found by iteration 228 while building 352, not opened by a
  page. Feature: `docs/features.md` stage 1, "Absolute and relative
  positioning". Waits for a page.* CSS 2 § 10.1 places an absolutely
  positioned box against the padding box of its nearest ancestor with a
  `position` other than `static`, or the initial containing block if there
  is none. `taffy` places it against its parent: in `<div
  style="position: relative; margin-left: 50px"><section style="margin-left:
  30px"><span style="position: absolute; left: 0">`, the span is at x = 80,
  not 50. alo Sites' skip link is a child of `body`, which is where the
  initial containing block is, so it is right by coincidence. *Closes
  when:* that span is at x = 50, and a box with no positioned ancestor is
  against the viewport, in numbers. *Opened by a frozen page whose
  absolute box is not its containing block's child, and not before.*

- [x] **353. `Date`.** *Opened by `alo-sites-cta` (iteration 227); cut from
  73.* alo Sites' analytics script, which every page it publishes carries,
  stops at its third line, `var since = Date.now();`, with "ReferenceError:
  'Date' is not defined", before it adds a listener. *Needs ADR*: what clock
  a page reads is a decision — its precision is a timer a Spectre gadget
  (ADR 0005) wants, and ADR 0030 says the browser tells a page no locale,
  of which a time zone is a part. Which clock, at what grain,
  in which zone, is decided before `Date` is built. *Closes when:* the
  script runs past line 3, in `tests/alo_sites_cta.rs`; and what it stops
  at next, if anything, is opened as an item.
  **Decided (iteration 229): ADR 0036, accepted.** `Date` is the engine's
  and the instant is the embedder's: `alo-js` defines a `Clock` a realm is
  handed, and a realm with none refuses `Date.now()`, `new Date()` and
  `Date()` by name while every other use works. The renderer's clock is the
  machine's wall clock, read by the renderer in one file, floored to a
  whole millisecond — the language's own grain — with no jitter, because
  site isolation (ADR 0005) is the Spectre answer. A page's local zone is
  UTC until the person chooses one in settings; the renderer never reads
  the machine's zone. `Date.parse` reads the language's forms and nothing
  older, and Annex B's date methods are absent. Tests and corpus cases read
  a fixed clock. Nothing is built, and this item stays open: it closes when
  **356** does. **357** (a date as text) waits for a page, and **358** (the
  person's zone) for settings (128).
  **Done (iteration 230)** with 356: `alo-sites-cta`'s script runs past
  `Date.now()` on line 3 and stops at `encodeURIComponent` on line 8,
  opened as **359**.

- [x] **356. `Date`, with a clock.** *Cut from 353 (ADR 0036 §§ 1, 2, 3
  and 5). Depends on nothing open.* `alo-js`: the `Clock` trait and a realm
  made with one or none; the `Date` constructor in every form but a
  string, `Date.now`, `Date.UTC`, the getters and setters, local and UTC
  with `LocalTZA` zero, `valueOf`, `getTime`, `getTimezoneOffset`,
  `toISOString`, `toJSON` and `[Symbol.toPrimitive]`, each under ADR 0031's
  rules, with ECMA-262's arithmetic in `f64` so a hostile year cannot
  overflow. `new Date(string)`, `Date.parse` and the `toString` family are
  refused by name, pointing at 357; Annex B's `getYear`, `setYear` and
  `toGMTString` are absent. `alo-renderer`: ADR 0036 § 2's clock in one
  file (the wall clock, floored to a millisecond, `NaN` out of range),
  handed to every realm it makes. `alo-corpus`: one fixed instant, named in
  one place with its reason, for every loaded case; `alo-js`' and
  `alo-bindings`' tests likewise. *Closes when:* `alo-sites-cta`'s script
  runs past line 3, in `tests/alo_sites_cta.rs`, and what it stops at next
  is opened as an item; a realm with no clock throws a `TypeError` for
  `Date.now()` and still answers `new Date(0).getTime()`; the renderer's
  clock is floored and within the machine's own reading, in a test; a
  fixed clock makes the same answer in every run; and getters and setters
  agree with ECMA-262's worked values at the range's ends (±8.64 × 10¹⁵)
  and one past them, in tests.
  **Done (iteration 230).** `alo-js`: `clock.rs` (the `Clock` trait and
  `Fixed`, an instant that never moves); `Engine::with_clock`, and
  `Engine::new` a realm with none; the realm holds it and the interpreter
  hands it to every builtin (`Call::now`, through `TimeClip`, or the
  `TypeError` "this realm was given no clock"). `time.rs` is ECMA-262
  § 21.4.1's arithmetic in `f64`, `LocalTZA` zero in one constant, and
  `MakeDay` refusing a year past 10¹³ where its count of days would round.
  `object/date.rs` is the `[[DateValue]]` cell (`Instance::Date`).
  `builtin/date.rs` (the constructor in every form but a string, `now`,
  `UTC`, `parse` refused), `date_numbers.rs` (each argument through
  `ToNumber` once, in order, kept in ADR 0031's slots, the argument's
  index in the step), `date_prototype.rs` (16 getters, `getTime`,
  `valueOf`, `getTimezoneOffset`, `toISOString`, and the `toString`
  family refused after the brand check), `date_set.rs` (14 setters and
  `setTime`, the time value read first and kept, an Invalid Date not
  written over) and `date_convert.rs` (`toJSON`, and
  `[Symbol.toPrimitive]`, not writable). **`ToPrimitive` now asks for
  `Symbol.toPrimitive` first** (`convert.rs`, `interpret/primitive.rs`),
  calls it with the hint's string and takes its answer as final, and
  `Want::Ordinary` is `OrdinaryToPrimitive` alone for that method's last
  step: without it `date + ''` would have answered the number. A fourth
  well-known symbol, `Symbol.toPrimitive`. `Object.prototype.toString`
  says `[object Date]`. A date as text is `Missing::ADateAsText` (357).
  `alo-renderer`: `clock.rs`'s `WallClock` (the machine's wall clock,
  floored to a millisecond, `NaN` before 1970 or past 8.64 × 10¹⁵),
  handed to every realm through `EventLoop::new(clock)` and
  `Held::scripted`; `Renderer::told_the_time_by` hands a fixed one in.
  `alo-corpus`: `INSTANT` (2026-10-09T00:00:00.000Z), named once with its
  reason, for every loaded case, and a new case `a-script-writes-the-date`.
  Tests: `alo-js`'s `what_a_date_is.rs` (a realm with no clock refuses and
  still does arithmetic; a fixed clock; every getter; both ends of the
  range and one past them, by getters, setters, `Date.UTC` and `setTime`;
  arguments converted once each and in order; setters working from the
  value read first; the brand; `toJSON`; a date as text and as a number;
  hostile numbers), `what_to_primitive_asks_first.rs` (every shape of
  `Symbol.toPrimitive` an embedder can make, getters included), `time.rs`'s
  and `object/date.rs`'s unit tests; `alo-renderer`'s `clock.rs` tests
  (floored, within the machine's own reading, `NaN` out of range);
  `alo-corpus`'s `a_script_writes_the_date.rs` and `alo_sites_cta.rs`,
  whose script now stops at line 8. Checked by mutation: `ToPrimitive`
  skipping the symbol fails the hint test, and a setter re-reading its
  date fails the kept-value test. `alo-bindings`' tests read no date, so
  none needed a clock; a realm with none refuses one by name.
  **Cut (iteration 230):** what the script stops at next is **359**.

- [x] **359. `encodeURIComponent`.** *Opened by `alo-sites-cta` (iteration
  230); cut from 73. Depends on nothing open.* alo Sites' analytics script
  stops at its eighth line, `var page = "&p=" +
  encodeURIComponent(location.pathname) + "&w=";`, with "ReferenceError:
  'encodeURIComponent' is not defined". ECMA-262 § 19.2.6 specifies it
  whole: `ToString` of the argument, each code point outside the unreserved
  set written as the percent-escaped bytes of its UTF-8, and a lone
  surrogate the `URIError` the specification gives. `encodeURI`,
  `decodeURI` and `decodeURIComponent` are the same section, and are taken
  only if a page needs them. *Closes when:* the script runs past line 8,
  in `tests/alo_sites_cta.rs`, and what it stops at next is opened as an
  item; and every code point class — unreserved, reserved, two-, three-
  and four-byte, a paired and a lone surrogate — is written as the
  specification says, in tests, with a long and a hostile string refused
  or answered in bounded work.
  **Done (iteration 231).** `alo-js`: `uri.rs` is § 19.2.6's `Encode` over
  UTF-16 code units — the unreserved set (`uriAlpha`, `DecimalDigit`,
  `uriMark`) as itself, every other code point as its UTF-8 bytes in
  uppercase `%XX`, a pair as one four-byte code point, and a lone
  surrogate refused with its unit and index. The output is checked
  against `LONGEST_STRING` before every write and stops the moment it
  would pass it, so the work is bounded by what was written.
  `builtin/encode_uri_component.rs` is the function: `ToString` of its
  argument (an object's conversion asked for under ADR 0031 and resumed
  at a step of its own), the encoding, and the errors. A fourth `Kind`,
  `URIError`, which a `catch` makes an instance of the `URIError`
  constructor. `Realm::name_the_functions` puts it on the global object,
  writable, configurable and not enumerable; it is no intrinsic, since
  nothing in the engine calls it. Tests: `uri.rs`'s five unit tests (the
  71 unreserved units and nothing else below 128; reserved and the rest of
  ASCII; two-, three- and four-byte; lone leading and trailing surrogates,
  two leading ones, a pair then a trailing one; the length bound, lowered
  for the test, refused as the output grows, with a lone surrogate the
  string reaches first reported first, and a 2²⁰-unit string answered);
  `tests/what_encode_uri_component_answers.rs` (every class as a program;
  the `URIError` caught, named and an instance; the argument converted
  once, `toString` before `valueOf`, a throw passed through, a converted
  lone surrogate refused; the property's attributes, replaceable,
  deletable, no constructor; 2²⁰ `é`s by doubling), each run ordinarily
  and under `Heap::stress`. Checked by mutation: lowercase digits fail
  three tests, and a trailing surrogate written as U+FFFD fails two.
  `encodeURI`, `decodeURI` and `decodeURIComponent` stay `undefined`
  (item 73), as the item said.
  **Closing condition, said plainly:** the script does **not** yet run past
  line 8. `encodeURIComponent` now resolves and is called, and the stop
  moved to its argument on the same line: "ReferenceError: 'location' is
  not defined" at line 8, column 41, pinned in `tests/alo_sites_cta.rs`.
  The item was written without seeing that `location` is absent too. The
  part of the condition this function cannot meet is cut into **360**,
  whose closing condition is the script running past line 8; the rest of
  this item's conditions are met in tests.

- [x] **360. `location`, read.** *Opened by `alo-sites-cta` (iteration
  231); cut from 359. ADRs: 0019 (`Window`'s and `Document`'s `location`
  are `[LegacyUnforgeable]`, copied from the unforgeables object), 0020
  (a script that navigates asks the browser process, through item 85) and
  0035 (the page's address is what its URLs resolve against). Feature:
  `docs/features.md` stage 2, the DOM bindings.* alo Sites' analytics
  script stops on its eighth line, `encodeURIComponent(location.pathname)`,
  with "ReferenceError: 'location' is not defined" at column 41, and reads
  `location.hostname` again on its click handler. HTML's `Location`: the
  `location` getters on `Window` and `Document` answering the same object,
  and its reading members — `href`, `origin`, `protocol`, `host`,
  `hostname`, `port`, `pathname`, `search`, `hash` and `toString` — from
  the document's URL, which the renderer already holds as the address it
  loaded the page at. Everything that navigates (`assign`, `replace`,
  `reload`, and every setter) is item 85's under ADR 0020, and is refused
  by name until then rather than silently doing nothing. *Closes when:*
  `alo-sites-cta`'s script runs past line 8, in `tests/alo_sites_cta.rs`,
  and what it stops at next is opened as an item; each reading member
  answers the specification's value for an address with and without a
  port, query and fragment, in tests; a document with no address says so
  rather than inventing one; and a navigating member is refused by name.
  *Is it designed?* The next iteration that takes it decides: what a
  document loaded with no address answers, and whether refusing the
  navigating half is a decision ADR 0020 has already made. If either is
  not, the item is `needs design` first.
  **Done (iteration 232).** *Designed, both questions answered by what was
  already decided:* a document with no address is at `about:blank`, which
  is HTML's own URL for a document nobody gave another one and what the
  document cell already held (`DocumentCell::url`), so `href` is
  `"about:blank"` and `origin` `"null"`; and ADR 0020, *What this does not
  decide*, gives `location` and every other way a script navigates to item
  85 through the same ask, so until then each is refused by name under ADR
  0013 § 3. `alo-url`: `reading.rs`, the URL Standard's nine readings
  (`href`, `origin`, `protocol`, `host`, `hostname`, `port`, `pathname`,
  `search`, `hash`) written once over our `Url`, for `Location` now and
  `a.hostname` and `URL` later. `alo-bindings`: `location.rs`, the page's one
  `Location` — an embedder cell holding an edge to its document cell and
  nothing else, so every read is of the address the browser process stated
  last — made by `install` as HTML's *Location object creation* (the
  unforgeables copied, then own `valueOf`, the realm's
  `Object.prototype.valueOf`, and own `Symbol.toPrimitive`, `undefined`,
  each fixed), kept by the document cell, and put on the global object as
  an enumerable, non-configurable accessor whose getter finds it through
  `[[HostDefined]]` (ADR 0019 § 2) and whose setter is `[PutForwards=href]`,
  refused. `interface/location.rs`: every member `[LegacyUnforgeable]`,
  on `Location`'s unforgeables (a new `Interface::Location` whose
  prototype is empty) — the nine getters, `toString` as the stringifier,
  the eight setters and `assign`, `replace` and `reload` refused by name
  ("a script navigating by 'location' is queue item 85, through the ask of
  ADR 0020"), each behind the brand check. `Document` gained unforgeables
  too: `location`, the same object, copied onto the document node's
  wrapper by `install`. `define.rs` gained an unforgeable attribute's
  setter and an unforgeable operation (enumerable, neither writable nor
  configurable). Tests: `reading.rs`'s eight unit tests (every part; only
  a host; the scheme's own port never read; an empty query or fragment
  read as none; IPv6 in brackets; `about:blank`; `data:`; what the parser
  escaped) and `tests/where_a_page_is.rs`'s thirteen (both address shapes,
  `about:blank`, one object from both places, a held `Location` reading the
  address stated after it was taken, `"" + location` by way of `toString`,
  fifteen navigating forms refused by name in sloppy and strict code,
  `origin` read-only, the brand check, every member own and unforgeable,
  `valueOf` and `Symbol.toPrimitive` own and fixed, the global's and the
  document's `location` own and unforgeable with nothing on either
  prototype, and the object kept through 64 collections), every script
  run ordinarily and under `Heap::stress`. Checked by mutation: an empty
  query read as `?` fails the readings' test, and the document's wrapper
  not given its unforgeables fails four binding tests.
  **Closing condition:** `alo-sites-cta`'s script runs past line 8, past
  `document.addEventListener` on line 29, and stops at line 32,
  `window.addEventListener("pagehide", record)`, with "ReferenceError:
  'window' is not defined", pinned in `tests/alo_sites_cta.rs` and opened
  as **362**. `ancestorOrigins` is absent (always empty with no frames,
  item 86) and `Location`'s exotic internal methods are cut into **361**.

- [ ] **361. `Location`'s exotic internal methods.** *Cut from 360. Depends
  on nothing open; observable only through `Object.preventExtensions`,
  `Object.setPrototypeOf` and `Object.getOwnPropertyDescriptor`, which are
  item 73's.* HTML makes a `Location` an exotic object: same-origin, its
  `[[PreventExtensions]]` answers `false`, its prototype is immutable
  (`SetImmutablePrototype`), its `[[GetOwnProperty]]` reports each of its
  default properties configurable, and its `[[DefineOwnProperty]]` refuses
  a default property. With no frames every `Location` is same-origin, so
  the cross-origin half is item 86's. Today a `Location` is an ordinary
  object carrying its unforgeable members, which no reading member can
  tell apart. *Opened by a frozen page that freezes, re-prototypes or
  describes `location`, or by item 73 making those reachable. Closes
  when:* each of the four answers as HTML says, in `alo-bindings`' tests,
  under `Heap::stress`.

- [x] **362. `window`, and the global object as an event target.**
  *Opened by `alo-sites-cta` (iteration 232); cut from 360's closing
  condition. ADRs: 0018 § 1 (an event target is any node *until the global
  object is a `Window`*), 0019 (a `Window`'s unforgeable members are copied
  from its unforgeables object). Feature: `docs/features.md` stage 2,
  events. Related: 251 (`document` as a `Window`'s accessor).* alo Sites'
  analytics script stops on line 32, `window.addEventListener("pagehide",
  record)`, with "ReferenceError: 'window' is not defined" at column 3; it
  reads `window.innerWidth`, `window.scrollY` and `window.innerHeight` on
  the lines after, which are each their own question when a script reaches
  them. HTML's `window` (and `self`) answer the global object, and a
  `Window` is an `EventTarget`, so `addEventListener` on it keeps a
  listener the browser can later fire `pagehide` and `visibilitychange`
  at. *Closes when:* the script runs past line 32, in
  `tests/alo_sites_cta.rs`, and what it stops at next is opened as an item;
  `window === self` and both are the global object; and a listener added
  to the window is kept and called by a dispatch at it, in tests.
  *Is it designed?* The iteration that takes it decides whether the global
  object becoming an event target is 251's step (the global an embedder
  cell holding its document, and now its listeners) or can come before
  it, and where an event dispatched at the window sits on a node's path —
  HTML puts the `Window` after the document on every path, which ADR 0018
  § 2's path does not have yet. If either is not decided by ADRs 0017–0019,
  the item is `needs design` first.
  **Decided (iteration 233): ADR 0037.** ADRs 0017–0019 did not decide
  either question, so this iteration wrote the decision and built nothing. It
  is now designed, depends on nothing open, and is eligible.
  - The global object is a `Window`, an embedder cell `alo-bindings` makes.
    `alo-js` gains a realm whose global object the host makes, ECMAScript's
    own provision (§ 1).
  - The `Window` holds its own listener list and an edge to its document.
    `EventTarget`'s brand check accepts it (§ 2).
  - A path ends at the `Window` after the realm's own document, for every
    event but `load` (§ 3).
  - Its members are its own (`[Global]`): `window` (unforgeable), `self`
    (`[Replaceable]`) and `location` (moved to its unforgeables). `window`,
    `self`, `globalThis` and the top-level `this` are all the global object,
    with no `WindowProxy` until frames. There is no named properties object,
    by law 1 (§ 4).
  - The browser fires nothing at the window until the lifecycle (**364**) is
    built (§ 5).
  - This comes before 251. 251 shrinks to `document`'s accessor and still
    waits on 73 (§ 6), and ADR 0018 § 1 is amended to say so.

  *Closes when*, from ADR 0037's *What this makes buildable*:
  - `alo-sites-cta`'s script runs past line 32, in `tests/alo_sites_cta.rs`,
    and what it stops at next is opened as an item;
  - `window === self`, and both are `globalThis`;
  - a listener added to the window is kept through collections and called by
    a dispatch at it, and by a dispatch at a node, after the document when
    bubbling and before it when capturing;
  - a `load` event stops at the document;
  - every script runs ordinarily and under `Heap::stress`.

  **Built (iteration 234)**, as ADR 0037 designs it:
  - `alo-js`: `Engine::with_global(make, clock)` and `Realm::new`'s
    `global`, a realm whose global object an embedder's `Make` makes from
    `Object.prototype`; `Engine::new` and `with_clock` unchanged.
    `tests/a_global_the_host_makes.rs` (three tests) runs every way a script
    reaches the global against a cell of the test's own, under stress.
  - `alo-bindings`: `window.rs`, the `Window` cell (an ordinary part, its
    `Listeners`, an edge to its document) and `engine(clock)`, which
    `install` now requires, refusing an ordinary global by name;
    `Interface::Window` inheriting `EventTarget`, with
    `interface/window.rs`' unforgeable `window` and `location` (moved from
    the global's own accessor in `location.rs`, now brand-checked) and its
    `[Replaceable]` `self` on the instance; the document cell's edge back;
    `listeners::of` and `listeners::change` for a node or a window;
    `EventTarget`'s brand check taking either, `undefined` and `null`
    meaning the window, and the window passive by default for the four
    scrolling types; `dispatch.rs`' path of `Entry`s, the window last
    after the realm's own document except for `load`, and `composedPath()`
    ending with it.
  - `alo-renderer`: the event loop's engine is `alo_bindings::engine`.
  - Tests: `alo-bindings`' `tests/what_a_window_is.rs`, thirteen, every
    script ordinarily and under `Heap::stress`; `what_an_event_does.rs`'
    path now ends at the window and `addEventListener` is a global name;
    `alo-corpus`' `tests/alo_sites_cta.rs` no longer reports a throw.
  **Closing condition met:** `alo-sites-cta`'s script runs to its end; its
  `pagehide` listener, dispatched at the window by a test (nothing fires it
  until 364), reads `window.scrollY` and `innerHeight` as `undefined` and
  stops at `Math.max` on the script's seventeenth line with
  "ReferenceError: 'Math' is not defined", opened as **365**; the
  viewport reads are opened as **366**.

- [x] **365. `Math`.** *Cut from 73. Opened by `alo-sites-cta` (iteration
  234): its `pagehide` listener's `height()` calls `Math.max`, and
  `permille` calls `Math.max`, `Math.min` and `Math.round`; `shape()` calls
  `Math.round`. Depends on 218 (done). ADRs: 0013 § 3 (absent beats
  approximate), 0031 (a builtin's state). Feature: `docs/features.md` stage
  2, the language.* The `Math` namespace object on the global object,
  writable and configurable and not enumerable, its value properties and
  its functions as ECMA-262 § 21.3 gives them. *Is it designed?*
  `Math.random` needs a source the engine does not read from the machine on
  its own (ADR 0036 § 1's reasoning, for a clock); the iteration that takes
  this decides whether that needs an ADR first or is cut out by name.
  *Closes when:* `alo-sites-cta`'s `pagehide` listener runs past `height()`
  in `alo-bindings`' `tests/what_a_window_is.rs`, what it stops at next is
  opened as an item, and each function answers ECMA-262's values for the
  edge cases (`-0`, `NaN`, infinities, no arguments) under `Heap::stress`.
  **Built (iteration 236).** `Math.random` is **cut out by name** to 367:
  its source is a decision of the shape ADR 0036 § 1 made for a clock, so it
  gets an ADR before code. `Math.f16round` and `Math.sumPrecise` are cut to
  368. Everything else of § 21.3 is built:
  - `builtin/math.rs` holds the namespace object, an ordinary object over
    `Object.prototype`, bound on the global object (writable, configurable,
    not enumerable) by `Realm::name_the_math`. It has its eight values
    (none writable, enumerable or configurable), `Symbol.toStringTag` of
    `"Math"`, and thirty-four functions. `round` and `sign` are written
    out; `pow` is `operate::exponentiate`, now `pub(crate)`; the rest are
    Rust's `f64`, each checked against the specification's edge cases.
  - `builtin/math_fold.rs` is `max`, `min` and `hypot`: every argument
    converted as it is reached and folded at once, the index and the answer
    so far in two slots (ADR 0031 § 5: the count is the page's), and the
    stop asked on every pass (§ 7).
  - `builtin/date_numbers.rs` is renamed `builtin/numbers.rs`, since the
    one-, two- and seven-argument conversions it does are now Math's as
    well as Date's.
  - Tests: `alo-js`' `tests/what_math_answers.rs` (12 tests, each program
    run both ordinarily and under `Heap::stress`), and unit tests in both
    files.
  **Closing condition met.** The `pagehide` listener runs past `height()`
  to its end. With `navigator` introduced, as the renderer does, it throws
  nothing (`what_a_window_is.rs`); with a beacon the test lends, it sends
  `/_alo/collect t=0` once. What it reaches next is not a throw but an
  absence: `navigator.sendBeacon`, opened as **369**. The viewport it reads
  stays 366.

- [ ] **367. `Math.random`.** *Cut from 365 (iteration 236). **Needs ADR**,
  as its own iteration, before any code.* Where a renderer's randomness
  comes from, and who hands it to the engine: ADR 0013 § 5 has `alo-js`
  read nothing of the machine, and ADR 0036 § 1 answered the same question
  for a clock with a trait the embedder implements. The decision also
  covers whether a test or a corpus case reads a seeded source, as § 5 of
  that ADR gives them a fixed clock, and what a realm given none answers.
  ECMA-262 asks only for numbers in `[0, 1)`, roughly uniform, and leaves
  the algorithm to the implementation. *Opened by:* a frozen page that calls
  it; none does yet. *Closes when:* the ADR is accepted, and the item it
  makes buildable answers in tests.

- [ ] **368. `Math.f16round` and `Math.sumPrecise`.** *Cut from 365
  (iteration 236).* `f16round` rounds to binary16, specified beside
  `Float16Array`; `sumPrecise` reads an iterable through the iteration
  protocol and adds its numbers exactly. *Depends on* typed arrays for the
  first and on iteration through a call (item 231) for the second. *Opened
  by* a frozen page calling either. *Closes when:* each answers ECMA-262's
  values, the edge cases included, under `Heap::stress`.

- [ ] **369. `navigator.sendBeacon`.** *Opened by `alo-sites-cta`
  (iteration 236): with `Math` built, the analytics script's `pagehide`
  listener runs to `send()`, finds `navigator.sendBeacon` absent, and
  sends nothing. **Needs ADR**: ADR 0032 lists keep-alive requests that
  outlive their document, and `sendBeacon`, as undecided, because a request
  that outlives the page that made it is a tracking feature first.* Whether
  a page may send one, to whom, with what credentials, and how it is
  recorded under ADR 0012. Depends on 364, since the moment it matters is a
  page being left. *Closes when:* the decision is an ADR, and the item it
  makes buildable sends or refuses the frozen page's report in tests.

- [ ] **366. The viewport a script reads.** *Opened by `alo-sites-cta`
  (iteration 234): `record` reads `window.scrollY` and
  `window.innerHeight`, `shape()` `window.innerWidth`, and `height()`
  `document.documentElement.scrollHeight` and `body.scrollHeight`; each
  answers `undefined` today, so the script's arithmetic is `NaN` rather
  than a throw. Depends on 362 (done). **Needs ADR**: each tells a page
  something about the person's window, and ADR 0030's rule — *nothing
  about the machine* — is where the question starts; ADR 0037 names these
  as undecided.* Which of `innerWidth`, `innerHeight`, `scrollX`,
  `scrollY` and an element's `scrollWidth` and `scrollHeight` a page may
  read, from where the renderer knows them, and what an agent's renderer
  with no window answers. *Closes when:* the decision is an ADR, and the
  item it makes buildable answers each in numbers, in tests.

  **Decided (iteration 237): ADR 0038.** A page reads its own viewport
  and its own content's size, and nothing beyond the window:
  - `innerWidth` and `innerHeight` are `Page::viewport` in whole CSS
    pixels, read at each read so that a `Resize` shows (§ 2);
  - `scrollX`, `scrollY`, `pageXOffset` and `pageYOffset` are a scroll
    position the renderer holds. It is zero because nothing scrolls a
    viewport yet, and whatever first does must move it (§ 3);
  - `scrollWidth` and `scrollHeight` are CSSOM View's scrolling area, with
    no quirks branch, measured from the layout the page would be drawn
    with at the moment it is read. That layout is kept for the next draw
    (§ 4);
  - `alo-bindings` asks through one trait, `View`, that the renderer
    implements, and gains no layout dependency. A page with no view throws
    by name (§ 5);
  - a tab no window shows is told the first window's size, 1000 × 700
    (§ 6).

  The screen, the window's outer size and place, and `devicePixelRatio`
  stay absent. This item is now the first build: `View`, the renderer's
  implementation, and the six `[Replaceable]` window accessors. The
  scrolling area is cut to **370**.

  *Closes when* (ADR 0038 § 7):
  - `alo-sites-cta`'s `shape()` answers `&p=%2F&w=800` at 800 × 600, in
    `tests/alo_sites_cta.rs`;
  - `innerHeight` is 600 and both scroll positions are 0;
  - after a `Resize` to 640 × 480 a script reads 640 and 480;
  - a fractional, a negative and an infinite viewport answer as § 2 says;
  - every script runs ordinarily and under `Heap::stress`.

- [ ] **370. An element's scrolling area.** *Cut from 366 by ADR 0038 § 7.
  Depends on 366.* `scrollWidth` and `scrollHeight` on `Element`, by § 4:
  - no box answers 0;
  - the root answers the larger of the viewport's scrolling area and the
    viewport;
  - any other element answers its padding box extended toward its end
    edges only;
  - rounded and clamped as a `long`.

  It also builds the renderer's measurement at the moment of the read, with
  the layout kept for the next draw and first-found objections owed as a
  draw's are, and § 6's windowless size if nothing has given it first.
  *Closes when:*
  - at 800 × 600, `alo-sites-cta`'s `documentElement.scrollHeight` is 600,
    `body.scrollHeight` is 253, and `documentElement.scrollWidth` is 800,
    with the skip link's leftward overflow not counted;
  - with a beacon the test lends, its `pagehide` listener sends
    `d=1000&p=%2F&w=800` and then `t=0`;
  - a script that appends a tall element and reads `scrollHeight` in the
    same task reads the new height, and the next draw does not lay out
    again;
  - an element with no box, and one in a detached tree, answer 0;
  - every script runs ordinarily and under `Heap::stress`.

- [ ] **363. The `Window`'s immutable prototype.** *Cut from 362 by ADR 0037
  § 6. Depends on 362; observable only through `Object.setPrototypeOf` and
  `Reflect.setPrototypeOf`, item 73's.* A `[Global]` object's
  `[[SetPrototypeOf]]` is `SetImmutablePrototype`, so setting the window's
  prototype to anything but what it is answers `false` and throws from
  `Object.setPrototypeOf`. *Opened by a frozen page that re-prototypes the
  window, or by item 73 making it reachable. Closes when:* it answers as Web
  IDL says, in `alo-bindings`' tests, under `Heap::stress`.

- [ ] **364. The page lifecycle at its window.** *Cut from 362 by ADR 0037
  § 5. Depends on 362. **Needs design**: when the browser process tells a
  renderer its page is hidden, shown or being left, and how long the page's
  listeners are given before the renderer goes.* `pagehide`, `pageshow` and
  `visibilitychange`, and `document.visibilityState` and `document.hidden`.
  `alo-sites-cta`'s analytics script waits on `pagehide` at the window and
  `visibilitychange` at the document to send what it measured. *Closes
  when:* the decision is an ADR, and the item it makes buildable fires each
  event at the moment HTML says, in tests.

- [ ] **357. A date as text.** *Cut from 353 (ADR 0036 § 4). Depends on
  356.* `Date.parse` and `new Date(string)` over the Date Time String
  Format and this engine's own `toString` and `toUTCString` forms, anything
  else `NaN`; `toString`, `toDateString`, `toTimeString`, `toUTCString`
  with ADR 0036 § 3's zone; and `Date()` called as a function. *Closes
  when:* every form round-trips; a form the language does not specify is
  `NaN`; and malformed, truncated and adversarial strings are refused
  without a panic and in bounded work, in tests. *Opened by a frozen page
  that writes or reads a date as text, and not before.*

- [ ] **358. The person's time zone.** *Cut from 353 (ADR 0036 § 3).
  Depends on 128 (settings).* The setting; the zone told to a renderer
  with the page, as the user agent is (ADR 0030 § 4); the IANA rules
  rented behind one file, the crate chosen then; and `Intl`'s default zone
  (79) the same one. *Closes when:* a person who chose a zone sees a
  page's local time in it, and one who did not sees UTC, in tests.

- [ ] **351. The window waits for a load's style sheets.** *Cut from 348
  (ADR 0035 § 5). Depends on 348 (done).* The browser process presents no
  first frame of a document until every sheet asked for in its load's
  answer is answered, within a bound of its own, with the number's reason
  in the code (ADR 0014 § 9). A page shown before its style is said to the
  person, and a sheet a script added later blocks nothing.
  *Needs design (iteration 226):* ADR 0035 § 5 says the wait is bounded
  because `PATIENCE` alone does not bound a server that trickles. Today the
  conductor makes each request on its own thread and waits for it, and
  `alo-net`'s pool bounds a read, not an exchange. So no bound the window
  sets can end the wait while one trickling request runs. Either the
  exchange gets a deadline of its own in `alo-net`, or requests are made off
  the conductor's thread. Which one is a decision about the network stack
  or the conductor (ADR 0024 § 2), and it is not made yet. *Closes when:*
  the window presents a page after its sheet's answer and not before, in a
  test; a server that never finishes a sheet has the page shown after the
  bound, with that said; and a sheet a script adds holds back no frame.
  `alo-window`'s `a_page_styled_in_the_window.rs` asserts today's unstyled
  first frame, and this item changes that assertion.

- [ ] **350. A loaded page's pictures.** *Cut from 347 (ADR 0035, *What
  this does not decide*).* An `<img>` or a `background-image` in a page a
  renderer holds is never handed its bytes, as a sheet was not. It takes
  ADR 0035's ask, and what may cross for a no-cors picture — which bytes
  are plausibly an image — is a rule of its own. *Needs ADR. Opened by a
  frozen page whose picture can be frozen with it*: alo Sites' hero
  section has an `<img>`, but its bytes are served from the site's own
  store and are in no repository.

- [ ] **84. WebSocket.**
  *Depends on 53, 76.*

- [ ] **85. Navigation and session history**: `pushState`, back and forward, and
  what survives each.
  *Depends on 80.*
  *Needs design (iteration 161):* its dependencies are done, but it names no
  ADR, feature contract or closing condition, so `LOOP.md` step 2 says it is
  not ready to build. Cutting a first item from it, with those written, is
  the work that opens it.

- [ ] **86. `iframe`s and the sandbox attribute** — a document inside a
  document, *"where a great many security bugs live"*.
  *Depends on 61, 63.*
  *Needs design (iteration 161):* its dependencies are done, but it names no
  ADR, feature contract or closing condition, so `LOOP.md` step 2 says it is
  not ready to build. Cutting a first item from it, with those written, is
  the work that opens it.

- [ ] **87. Shadow DOM and custom elements.** Component frameworks are not
  optional on the modern web.
  *Depends on 80.*
  *Needs design (iteration 161):* its dependencies are done, but it names no
  ADR, feature contract or closing condition, so `LOOP.md` step 2 says it is
  not ready to build. Cutting a first item from it, with those written, is
  the work that opens it.

- [ ] **88. Selection and ranges.**
  *Depends on 80.*
  *Needs design (iteration 161):* its dependencies are done, but it names no
  ADR, feature contract or closing condition, so `LOOP.md` step 2 says it is
  not ready to build. Cutting a first item from it, with those written, is
  the work that opens it.

- [ ] **89. CSSOM** — styles readable and writable from script.
  *Depends on 80.* **Opened by a page (iteration 215):** `alo-downloads`
  sets an element's `style`; that first cut is item 339, decided by ADR
  0033 (iteration 218) and built as 341–343.
  *Needs design (iteration 161):* its dependencies are done, but it names no
  ADR, feature contract or closing condition, so `LOOP.md` step 2 says it is
  not ready to build. Cutting a first item from it, with those written, is
  the work that opens it.

- [ ] **90. Storage**: `localStorage`, `sessionStorage`, IndexedDB, the Cache
  API, and **one quota policy over all of them**.
  *Depends on 72, 66. Needs ADR* — a quota is a policy about somebody's disk.
  **The ADR is written: ADR 0025, accepted** (iteration 184). No code is
  built and this item is not done. In short:
  - Every API uses one bucket per **storage key**: the origin and ADR 0007's
    top-level `Partition`. An opaque origin has no storage.
  - **One fixed quota of 1 GiB** covers each bucket, and it is reported the
    same on every machine and in private browsing. `localStorage` and each
    `sessionStorage` area are capped at 5 MiB, counted as UTF-16 bytes.
  - The profile's own bound is the smaller of 8 GiB and a fifth of the free
    space at start, and is never reported to a page. Past it, **whole
    buckets** are evicted, least recently used by a counter, never one in use
    or kept by a person.
  - The **browser process** holds every bucket. It serves a renderer only the
    storage keys it loaded into that renderer. `localStorage` is a copy sent
    with the load, whose writes are posted back and checked again.
  - `sessionStorage` is **never written to a disk**.
  - IndexedDB values are bytes the browser process never parses. Keys use an
    encoding of ours that sorts as bytes. An opaque response in the Cache API
    counts at a fixed padded size.
  - A bucket that fails its check is **set aside whole** and recorded, never
    served in part. Clearing a site clears everything it stored in one act.
  - `persist()` answers `false` until item 93.

  It is cut five ways, and **this item closes when 301–305 have**, and 306,
  which was cut from 301 as it was built.

- [x] **301. The storage store, in the browser process.** *Cut from 90
  (ADR 0025 §§ 1–4, 6 and 8). Depends on 66 and 155.* Buckets keyed by
  origin and `Partition`. A type that cannot hold an opaque origin. The
  ledger that counts each bucket against 1 GiB and the profile against its
  bound. Whole-bucket eviction by a use counter, never the bucket writing or
  one marked in use. `localStorage` areas as the first thing it stores, in
  records under ADR 0011 § 4's rules. The set-aside rule. Clearing a site.
  The application-data directory, private to its owner. A session-scoped
  profile that opens no directory. No page reaches it yet. Like item 155, it
  is the browser process's half, built first so the page's half has something
  real to post to.
  *Closes when:* an area survives a restart (the store dropped and reopened on
  the same directory). A write past a bucket's quota is refused and changes
  nothing. A write past the profile's bound evicts whole buckets in least
  recently used order, sparing the writer and any bucket in use, and is
  refused when that is not enough. Every truncation and every flipped byte of
  a record sets its bucket aside without a panic, and the store records it.
  A session-scoped profile leaves no file. Clearing a site removes every
  bucket of it under every partition, set-aside ones included. Each is
  asserted in numbers: counted bytes, bucket counts, files on disk.

  **Done** (iteration 185). `alo-storage` is a crate of its own, because
  keeping what a page asked for is not loading. It uses `alo-net`'s
  hostile-input reader, private directory and `Partition` rather than
  copying them. `alo_net::bytes` became public for this, and
  `alo_net::private::application_data` is now the one answer to where
  application data lives, for `kept.rs` and storage alike.
  `tests/storage_that_survives_a_restart.rs` closes every clause with a
  real restart: the `Store` is dropped and another is opened on the same
  directory.
  - The restart test checks that the area reads back, at 60 counted bytes,
    in three files.
  - A write past the bucket's quota is refused, and the bucket's file is
    byte-for-byte unchanged.
  - Eviction is walked through four writes. Each asserts the bucket count,
    the total, the eviction count and the files on disk. The last two show
    a write refused while nothing is evicted, and then accepted once the open
    bucket closes. The order of use survives a restart.
  - Every truncation and every flipped byte of both records sets the bucket
    aside. The store records it with the site's name. It is never deleted,
    and nothing of it is served.
  - Clearing a site removes its buckets under three partitions, including
    one set aside, and leaves another site's bucket alone.
  A site is a directory, so clearing one is one removal. Names are SHA-256
  digests, and that is not claimed as privacy. A session-scoped store has
  no directory to write to.
  Three things were left out, and each is named:
  - **Measuring free space is 306.**
  - **A person's mark to keep a bucket is 93's grant.** No bucket carries
    one yet.
  - **The profile's bound in a session-scoped profile is 125's to choose.**
    ADR 0025 says only that it is far smaller. Until then the caller passes
    its limits.
  The profile's bound is held against counted bytes, not file sizes. The
  files are larger by their lengths and names.

- [ ] **302. `localStorage` and `sessionStorage` in a page.** *Cut from 90
  (ADR 0025 §§ 3 and 5). Depends on 301, 80 and 236.* `Window.localStorage`
  and `Window.sessionStorage`, served from the renderer's copy. The area
  goes with a load to a renderer that does not hold it. Every write is posted
  and checked again by the browser process, which refuses a storage key it
  did not load into that renderer and a write past the cap. Opaque origins
  throw `SecurityError`, and 5 MiB throws `QuotaExceededError`
  synchronously. `sessionStorage` is held per tab in browser-process memory.
  The `storage` event to the key's other documents needs item 81's dispatch.
  *Opened by* a frozen page that keeps a preference in `localStorage`, as
  alo's `i18n/locale.ts` does. *Closes when:* that page's stored value is
  read back after a restart. A `sessionStorage` value survives a reload and a
  navigation away and back, does not survive closing the tab, and appears in
  no file. A renderer's write for another key is refused.

- [ ] **303. `navigator.storage`: `estimate()`, `persist()` and
  `persisted()`.** *Cut from 90 (ADR 0025 §§ 3 and 7). Depends on 301, 302
  and 75.* `estimate()` reports the fixed quota and the bucket's counted
  usage, the same in a session-scoped profile. `persist()` and `persisted()`
  answer `false` until item 93 gives the grant, which ADR 0026 makes the
  *keep data* capability in item 307's table. *Closes when:* `estimate()`
  gives the same `quota` under two profiles on volumes of different free
  space and in a session-scoped one, and `usage` moves by the counted bytes
  of a write.

- [ ] **304. IndexedDB.** *Cut from 90 (ADR 0025 § 5). Depends on 301, 76
  and 81. Needs ADR:* what its ordered, transactional store is built on.
  ADR 0025 left that question open and named the conditions: rented only in
  Rust, a licence beside MPL-2.0, files read as untrusted. The rest is
  decided: values as structured-clone bytes the browser process never
  parses, keys in an encoding of ours that sorts as bytes, index keys
  extracted in the renderer, `complete` after the hand-off to the operating
  system and `strict` after a flush. *Opened by* a frozen page that opens a
  database. alo's quote studio does, once, to move an old copy to its server.

- [ ] **305. The Cache API.** *Cut from 90 (ADR 0025 § 5). Depends on 301,
  75, 83 and 91.* `caches` in secure contexts. What the page put in it is
  stored and counted, and an opaque response counts at a fixed padded size.
  *Opened by* alo's frozen service worker, which precaches `offline.html`.
  *Closes when:* that worker's install stores the page, its activate deletes
  the old version's cache, an offline navigation is answered from it, and an
  opaque response's `usage` does not depend on its length.

- [x] **306. The volume's free space, measured.** *Cut from 301 (ADR 0025
  § 3). Depends on 301.* The profile's bound is the smaller of 8 GiB and a
  fifth of the volume's free space when the browser starts.
  `alo_storage::Limits::for_a_volume_with` computes it from a number, and
  nothing yet asks the operating system for that number. The standard
  library has no call for it, and law 4 forbids writing the system call
  ourselves. So this rents a crate that makes the call safely, named in one
  file and added to `scripts/gate.sh`'s boundaries. That choice is made in
  the diff, with the crate's `unsafe` checked as ADR 0024 checked `winit`'s.
  *Closes when:* the store's directory reports a free-space figure, checked
  against the volume's own report on this machine. A volume that cannot be
  asked gives a bound of zero, which refuses every write. It never gives an
  unbounded one.

  **Done** (iteration 187). The crate is **`rustix` 1.1.5**, features `fs`
  and `std` only, already in the lock beneath `winit` and `softbuffer` on
  Linux. Its `statvfs` is a safe function, and the `unsafe` is the crate's
  (ADR 0010): on macOS `backend/libc/fs/syscalls.rs` calls the C library's
  `statvfs` into a `MaybeUninit` and converts every field to `u64`. It is
  named only in `alo-storage/src/volume.rs`, which `scripts/gate.sh` now
  holds it to, and is a dependency only on `cfg(unix)`.
  - The figure is `f_bavail × f_frsize`, so it counts space free to an
    ordinary user, which is what `df` calls *available*. A product that
    overflows is `None`. A platform without the call is `None`, and so is a
    path that is not there.
  - `Limits::for_the_volume_at` turns `None` into a profile bound of zero.
    `Store::on_its_volume` makes the directory, measures that directory and
    opens the store with those limits.
  - `tests/free_space_measured.rs` checks the store's directory against
    `df -Pk` on the same directory, within 512 MiB. `df` asks through
    `statfs` rather than `statvfs`. On this machine the two agreed at about
    21.8 GB. With `f_blocks` substituted, the test fails at 494 GB against
    21.8 GB. A store whose volume cannot be asked refuses a two-character
    write with `ProfileFull` and leaves no file.
  Nothing in the browser process opens a store yet, so "when the browser
  starts" is the call a caller makes. It is not wired, because no caller
  exists. The profile's bound in a session-scoped profile is still 125's.

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

  **The decision is written down: ADR 0026, accepted** (iteration 188). A
  permission is a grant in one table the browser process holds: one
  capability from a closed list (camera, microphone, location,
  notifications, keep data, storage access, open another program, read the
  clipboard), to one storage key (ADR 0025's origin and top-level site), made
  by a person answering an ask that a page made with transient activation.
  It ends when the page closes, or thirty days after the person last opened
  the site themselves. *Don't allow* is remembered on the same terms. There
  is no *always*, no blanket allow and no shipped allowlist. It is shown in
  one list and by an indicator in the tab strip while in use, revoked in one
  act that stops the device at once, and recorded without its content. **No
  agent can make, answer or revoke one.** **The table is built (307,
  iteration 189); this item is not done.** It closes when 308 does, and each
  capability's API arrives with its own item (303, 157, 92, 290) or page.

- [x] **307. The grant table, in the browser process.** *Cut from 93
  (ADR 0026 §§ 1–3, 5, 7–9). Depends on 301.* A crate of its own,
  `alo-grants`, with no device and no interface. It holds the closed
  `Capability` enum, grants keyed by `alo-storage`'s `StorageKey`, and the
  decision function: *may this key use this capability now*, from the ask's
  facts (secure context, transient activation, opaque origin, a refused
  document, a remembered answer, a blanket refusal, the clock). It also holds
  the two lifetimes, the thirty-day expiry counted from the person's own
  top-level visits, a clock that went backwards failing closed, revocation,
  clearing a site, the record of sixty-four entries per key with a counted
  drop and no content, and the file under ADR 0011 § 4. A private session
  writes nothing. *Closes when:* each refusal in §§ 2–3 has a named test that
  asks and is refused without a prompt; an *allow while open* grant ends with
  its document and an *allow on this site* grant ends thirty days after the
  last `Person` visit, while a page's own use does not extend it; a clock set
  backwards expires a grant; a revoked grant answers *no* on the next check;
  a malformed, truncated or adversarial table file returns an error and
  grants nothing, set aside and never deleted; and a private session leaves
  no file.

  **Done** (iteration 189). `alo-grants` is a crate of its own, with
  `alo-net` and `alo-storage` its only dependencies. It reads its file with
  `alo_net::bytes` and keeps causes as `alo_net::deed::Link`, rather than
  copying either. `StorageKey::from_parts` became public so a key can be read
  back. `tests/a_person_grants_and_it_ends.rs` closes every clause, and the
  restarts in it are real: the `Table` is dropped and another opened on the
  same directory.
  - One named test per refusal: opaque origin (`file:`, `data:` and `about:`),
    insecure context, a frame not delegated, no gesture (which reads as
    unasked), a document that dismissed a prompt, a remembered *don't allow*
    and a refusal everywhere. Each is `Decision::Refused` with no prompt.
  - *Allow while open* is never written and ends with `document_gone`,
    recorded `PageClosed`. *Allow on this site* holds to the second before
    day 30 and not at day 30. Twenty-nine days of use, page-caused loads and
    agent-caused loads leave `visited` at day 0. A `Person` visit at day 20
    moves the end to day 50.
  - A clock at day 9 ends a grant given at day 10, and a person's visit with
    that clock does not rescue it. It is recorded `ClockWentBack`.
  - A revocation answers *no* on the next `allows`, returns the document
    whose use it stopped, and records `UseEnded` then `Revoked`.
  - Every truncation, every flipped byte, two lying counts, a PNG and an
    empty file are each set aside as `table.aside.<n>` with their bytes
    kept. Each grants nothing and is believed in no part.
  - A private session's table has no directory and nothing to write with.
  Also asserted: an agent's or a page's answer, dismissal, revocation and
  refusal everywhere are refused, and the prompt is handed back. The prompt
  says an agent was acting. Fifty refused asks keep 64 entries and count 36
  dropped. Clearing a site removes its keys under every top-level site.
  Two things are left for later, and each is named:
  - **Refusing an ask that names a key the browser process did not load into
    that renderer** (ADR 0026 § 7) needs a renderer to ask from. It lands with
    the first capability API that reaches this table.
  - **Rows for item 133's agent grants** are that item's, in this table.

- [ ] **308. The prompt, and the indicator.** *Cut from 93 (ADR 0026 §§ 4,
  6). Depends on 307 and 297.* The prompt is a document of ours, rendered as
  the tab strip is and built from a typed message, with the origin and the
  top-level site as text nodes. It offers three answers, read from a person's
  own input, and states that an agent was acting when the ask came from an
  `Act`. Every agent verb on it is refused by name. A device in use shows in
  the strip's data and in the window. *Closes when:* reference renders of the
  prompt and of a tab with the indicator are committed, and the agent's tree
  reads the prompt while every verb on it is refused.

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

- [x] **190. The border styles that are two tones**: `groove`, `ridge`, `inset`
  and `outset`. Cut from 183, which draws a fieldset's border `solid` where
  every other browser draws a `groove` — and says so in the user-agent sheet,
  because a substitution nobody wrote down is one nobody re-checks. Each is the
  same shape drawn in two colours, a lighter and a darker derived from the
  border's own, and which side gets which is what makes it look raised or sunk.
  `dashed`, `dotted` and `double` are the same file's work and belong with them.
  *Depends on nothing. Closes when:* each draws in a reference render and none
  of them is another one — a `groove` and a `ridge` that came out identical
  would be a test passing on a picture nobody looked at.
  **Built (iteration 161)**, scope cut twice and written down as 266 and 267.
  `alo-paint`'s `border.rs`: `Line` (the styles drawn), `tones` (the darker
  tone a third off the brightest channel, the lighter a third on, hue kept,
  never equal), `colors_of` (lit from the top left; a groove is an `inset`
  outer half and an `outset` inner half, a ridge the reverse) and
  `draw_mitred` — each side the wedge of the box it is fewest of its own
  widths from, which is the mitre at each corner and a straight cut between
  opposite sides, filled by colour inside the border's ring, and the outer
  half again inside the ring half as thick. `build.rs` takes that path when
  any side is two-toned, so a solid border keeps its rectangles and no
  existing reference moved. Tests: `border.rs` 13 (tones never equal over
  every grey and a few hues, a third either way, black and white, alpha
  kept, which side is which, no two styles alike, the wedges cover the box
  exactly once and wind clockwise, a side with no width gives up its
  corners, the display items' order); `tests/two_toned_borders.rs` 10 in
  pixels (each style's sides, outer and inner half; no two pictures alike
  nor any like `solid`; a corner split on its diagonal; no seam where both
  sides are one tone; a rounded groove's halves meeting on the curve; a
  solid side beside a two-toned one mitred; a `dashed` side left empty).
  Corpus case `border-styles` is the reference render.
  A first version stopped each side at the padding box's rectangle and left
  a hole in every rounded corner, and a second bounded each side only by its
  two mitres and so overlapped the side opposite on uneven widths; both were
  caught by tests above before the commit.

- [x] **266. `dashed`, `dotted` and `double`.** *Cut from 190 on the
  iteration that built it.* The same file's work, but each is a pattern
  along a side rather than a tone across it: dashes and dots spaced so a
  side starts and ends on one, round dots for `dotted`, and `double` as two
  lines a third of the width each. Today a side with any of them is left
  empty, which is said in `docs/conformance.md`. *Depends on nothing.
  Closes when:* each draws in a reference render, a dotted side's dots are
  round, and neither a dashed nor a double side is drawn as solid.
  **Built (iteration 162)**, with one cut written down as 268 (the same
  styles beside a fieldset's legend). `alo-paint`'s new `pattern.rs` is the
  spacing: dashes about three widths long with equal gaps, stretched so a
  side starts and ends on one; round dots a width across, about a width
  apart, the end ones centred in the corners; `double` in thirds; and at
  most 16 384 pieces a side, so a page cannot ask for unbounded work.
  `border.rs` draws them in the mitred wedges, in layers: a dash is its
  wedge cut across; dots are clipped to the wedges of their colour
  together, so a corner dot is one dot; a `double` side is its wedge inside
  the outer and inner third rings, so both lines follow a rounded corner.
  Found and fixed on the way, both caught before the commit: a seam of the
  page's colour along every dashed corner's mitre, where two dashes cut off
  at different points on it made two different edges along one line — now
  every cut on a mitre is a joint both wedges share, to the bit; and the
  first fix for that put every joint into every wedge, which took 81 s to
  build a hair-thin dashed border round a large box — now each piece takes
  only the joints beside it, measured along its own side (0.2 s, debug
  build, this machine; not a performance claim). `border.rs` had gained
  three reasons to change, so it is split: `tone.rs` (the two tones),
  `mitre.rs` (which part of the box is each side's, and the joints), and
  `border.rs` (what each side is drawn as, in what order). `corner.rs`
  gains `Corners::inside`. Tests: `pattern.rs` 9, `mitre.rs` 9 (5 new),
  `border.rs` 10 (7 new), `tone.rs` 6 (moved);
  `tests/patterned_borders.rs` 10 in pixels (dashes' gaps and ends, no seam
  in a dashed corner, round and apart dots, a corner dot without a seam,
  `double`'s thirds and their rounded corner, none of the three solid or
  alike, a patterned side beside a solid one mitred, hair-thin borders on
  huge boxes bounded). Corpus case `border-patterns` is the reference
  render; no other reference moved.

- [x] **267. A fieldset's `groove`.** *Cut from 190 on the iteration that
  built it.* `draw_banded_border` draws only `solid`, so the user-agent sheet
  still gives a fieldset `2px solid #c0c0c0` where other browsers give it a
  groove. Two-toned sides there are the mitred wedges of `mitre.rs` with the
  legend's gap cut out of the block-start one, and then the sheet can say
  `groove`. *Depends on nothing. Closes when:* `fieldset-group` draws a
  groove broken by its legend, `a_border_a_legend_breaks.rs` asserts the
  pieces in both tones, and the user-agent sheet's comment about it is gone.
  **Built (iteration 163).** The border a legend breaks moved out of
  `build.rs` into `alo-paint`'s new `banded.rs`, which `build.rs` only
  dispatches to. A border that is solid wherever it is drawn keeps its five
  rectangles. One with a two-toned side is `border::draw_mitred` on the
  area below the band's inset, with the band's stroke as the top width,
  inside one clip: the area wound clockwise and the legend's hole the other
  way round, so under the non-zero rule the hole is outside. The hole is the
  band's gap across the stroke's depth, clamped to the side borders' inner
  edges, so a legend wider than the fieldset leaves the corners drawn,
  exactly as the solid pieces do. That is how Chrome's fieldset painter
  cuts it too (a clip-out of the legend's span). The user-agent sheet now
  says `2px groove #c0c0c0`, and its comment says so rather than naming a
  substitution. Tests: `banded.rs` 8 (the hole's geometry, its clamping, an
  empty gap or stroke cutting nothing, the clip's coverage in and out of
  the hole, solid as five rectangles and no clip, a groove's layers inside
  the clip, no clip when the gap cuts nothing, a patterned side left
  undrawn); `a_border_a_legend_breaks.rs` 5 new in pixels (both tones
  either side of the legend and nothing in the gap, the gap's edges to the
  pixel, the other three sides a groove from the line down, a legend wider
  than the box leaving the far corner, the sheet's two tones and never the
  grey itself), and its 4 counting tests now ask for `solid` by name.
  Doctored: no hole fails 3, an unclamped hole fails 1. Corpus cases
  `fieldset-group` and `web-a-form` moved (display list and picture only;
  layout, boxes and agent tree unchanged), and no other reference did.

- [x] **268. `dashed`, `dotted` and `double` beside a legend.** *Cut from
  266 on the iteration that built it.* `alo-paint`'s `banded.rs` (267)
  leaves a `dashed`, `dotted` or `double` side undrawn where a legend sits
  in the border — said in its module comment and in `docs/conformance.md`.
  The clip with the legend's hole that 267 built would cut them as it cuts
  a groove; what is open is whether a dash or dot cut off by the legend is
  the right answer, or the pattern should be spaced on each piece. *Depends on
  nothing; best taken with or after 267. Closes when:* a corpus case draws
  a dashed fieldset broken by its legend and `a_border_a_legend_breaks.rs`
  asserts the pieces either side of the gap.
  **Built (iteration 164).** The open question is answered as: **laid
  along the whole side, then cut.** `pattern.rs` spaces a side so it starts
  and ends on a dash at its corners, and a legend moves neither corner;
  spacing each piece afresh would make every dash on the line depend on
  how long the legend's words are. So `banded.rs` no longer filters the
  three styles out: they go through `border::draw_mitred` inside the same
  clip 267 built, and a dash or dot the legend's edge falls on is cut there.
  This is also how Chromium and Firefox paint a fieldset (a clip-out of the
  legend over an ordinary border), recalled rather than re-read this
  iteration. Tests: `banded.rs` (a patterned border's items inside the
  hole's clip are exactly `draw_mitred`'s for the same area, for all three
  styles; replaces the test that they were left undrawn);
  `a_border_a_legend_breaks.rs` 3 new in pixels (the block-start stroke is,
  pixel for pixel, the same box's stroke with no legend everywhere outside
  the legend's 30–70 and white inside it, all three styles and the whole
  depth; ink on both sides of the gap, the corner, the left side and the
  bottom; a dot spanning 68–74 cut at 70). Doctored: patterns filtered
  again fails 3 there and 1 in `banded.rs`; no hole fails 5. Corpus case
  `fieldset-patterns` is the reference render; no other reference moved.

- [x] **280. `text-align` moves an atomic inline.** *Opened by `alo-offline`
  (iteration 172).* alo's offline screen is centred by `text-align: center` on
  its `body`, and its `<svg>` and `<button>` are each the only thing on their
  line. The engine aligns text runs and leaves an atomic inline where the line
  starts, so both sit at the left of `main` (x = 24) where every browser
  centres them (the hand at x = 204, the button at x = 173.5 in a 416-wide
  `main`).
  *Depends on nothing.* *Closes when:* `alo-offline`'s layout assertion puts
  both at the centre of their line, a unit test does the same for `right` and
  `start`/`end`, and a line of text and an inline-block together moves as one.
  **Built (iteration 173).** The line builder already aligned every
  fragment, atomic ones included; the fault was in *which* alignment it was
  given. Each of the offline screen's two lines is an **anonymous block**
  (the `<svg>` and the button sit beside block-level siblings), and
  `alo-layout`'s `alignment_of` read `text-align` only from a box's own
  element, so a box nobody wrote always got `start`. It now asks
  `BoxTree::nearest_style`, as an anonymous box inherits from its parent;
  a control's internal box still answers from its purpose (a button's
  label centred, a field's at the start whatever the page says). Tests:
  `numbers.rs` 2 (an inline-block alone in an anonymous line at 0, 0, 80,
  160, 160 for `start`, `left`, `center`, `end`, `right` in 200 px; "ab"
  and a 40 px box moving as one, the box at 16, 88 and 160); `inline.rs` 1
  (an atomic box alone and after text under all three alignments).
  Doctored: the old `alignment_of` fails both `numbers.rs` tests. Corpus:
  `alo-offline` moved — the hand to (204, 24) and the button to
  (173.54688, 195.48438), exactly the queue's numbers — and no other case.

- [x] **281. An atomic inline's margins count in its line's height.** *Opened
  by `alo-offline` (iteration 172).* The offline screen's `<svg>` has
  `margin-bottom: 20px`; its line is laid out 56 tall and the heading starts
  straight under the hand, where browsers leave the margin (and the strut's
  descent) below it. The margin box, not the border box, is what sits on the
  baseline.
  *Depends on nothing.* *Closes when:* `alo-offline`'s layout assertion puts
  the heading below the hand's margin and the line's descent, and a unit test
  pins an inline-block's top and bottom margins in its line's height.
  **Built (iteration 174), with the strut cut to 283.** `InlineItem::Atomic`
  carries the box's margins, and its baseline is measured from the top of
  its **margin box**, which for a box with no line is the bottom margin
  edge. The line builder fits, advances the pen by, aligns and stands on the
  baseline the margin box; the fragment stays the border box, inside its
  top and left margins. The engine reads the margins from the box's own
  layout, and lays it out the second time in its margin box's room — given
  only its border box, a block takes its margins out again, and an
  auto-width inline-block with text in it came back narrower and wrapped.
  *What was cut:* the line has no **strut** (CSS's zero-width box with the
  container's font), so the offline screen's line is 76 tall where browsers
  add the font's descent below the margin; that is a change to every line in
  the engine, not to atomic boxes, and is item 283. Tests: `inline.rs` 2 (a
  40×20 box with margins 6/10/8/4 between "ab" and "c" at x 20, y 6, the
  line 38 tall on a baseline of 34, the next text at 70; alone on a line
  the 54-wide margin box aligned to 4, 77 and 150; a box that fits only
  without its margins wraps). `numbers.rs` 3 (an inline-block with margins
  6 0 20 12 at 12, 6, 40×20 and the next block at 46; "ab cd" in an
  inline-block with 12 px side margins stays 40×16 at 12; a 20 px left
  margin centred in 200 puts the box at 90). Doctored: no margins on the
  item fails all three `numbers.rs` tests; ignoring them in the builder
  fails both `inline.rs` tests; the second pass in the border box fails the
  width test. Corpus: `alo-offline` moved — the line 416×76, the heading
  at (24, 100), everything under it 20 px down — and no other case.

- [x] **282. `place-items`, `place-self` and `place-content` as shorthands.**
  *Opened by `alo-offline` (iteration 172).* The offline screen centres `main`
  with `body { display: grid; place-items: center }`. No shorthand of the
  `place-*` family is expanded, so the declaration is kept and ignored and
  `main` stretches to the grid's whole height (352 where it should be about 235
  and centred). Each is one or two keywords split into `align-*` and
  `justify-*` — counting, as item 184's shorthands are, not parsing.
  *Depends on nothing.* *Closes when:* `alo-offline`'s layout assertion has
  `main` its content's height and centred in the grid both ways, and a test
  splits one- and two-value forms of all three.
  **Built (iteration 175).** Shorthand expansion moved out of
  `alo-css`'s `declaration.rs` into its own `shorthand.rs`, because a second
  family of shorthands is a second reason for that file to change; the
  sided family moved unchanged with its tests. The new family, `PAIRED`,
  splits by counting *values*, not words: `safe`/`unsafe` take the next
  word and `first`/`last` take `baseline` in the block axis, and the
  inline axis may also be `legacy` with a direction in either order. One
  value is both axes, two are block then inline, anything else is left
  whole. CSS's two exceptions are kept: `place-content` with only a
  baseline gives `justify-content: start`, and `legacy` first is not a
  shorthand at all. The longhands go in at the shorthand's position, so a
  longhand written after it still wins. Tests: `shorthand.rs` 6 new (all
  three in one and two values and three refused; two-word values incl.
  `center legacy left`; the baseline exception; `legacy` first left whole;
  a `var()` copied; a longhand after it wins in a block). `numbers.rs` 3
  (a 100×40 item in a 300×200 cell at 100,80 / 0,160 / 200,0 for
  `center`, `end start`, `start end`, and 0,80 with `justify-items: start`
  after; `place-self: end center` beats `place-items: start` at 100,160;
  `place-content` puts a 100×40 track at 100,80 and 0,160). Doctored: the
  `PAIRED` branch disabled fails all three `numbers.rs` tests. Corpus:
  `alo-offline` moved — `main` 416×230.68437 at (32, 84.657814), 60.66 px
  inside the padding above and below and 8 px either side — and no other
  case.

- [x] **283. The strut: every line starts as tall as its container's font.**
  *Cut from 281 (iteration 174).* CSS gives each line box a zero-width
  inline box with the block container's font and line height, so a line of
  only a picture still has the font's descent below its baseline and a line
  of small text in a large-font block is as tall as the large font. The
  engine has none: a line is as tall as what is on it. alo's offline screen
  shows it — the hand's line is 76 (56 and its 20 px margin) where browsers
  add the descent of the body's 16 px font. It touches every line, so it
  moves every reference whose lines mix sizes or hold only atomic boxes,
  and each move is to be read. `line-height` is not applied to text yet
  (the offline screen's `p { line-height: 1.5 }` is laid out at the font's
  own height); the strut takes the font's ascent and descent as text does
  today, and `line-height` is its own item when a page needs it.
  *Depends on nothing.* *Closes when:* a unit test has a line of one 20 px
  atomic box in a 16 px block one font-descent taller than 20, a line of
  small text in a large-font block as tall as the large font, and an empty
  line still no line; `alo-offline`'s layout assertion puts the heading
  under the hand's margin *and* the descent.
  **Built (iteration 176).** `alo-layout`'s line builder takes the
  container's font as a strut and starts every line — the first, each
  wrapped one, each forced one — at that font's ascent and descent; a line
  with nothing worth a line box is still no line. The engine passes the
  font of the box holding the lines (`text_style_for`) both when it sizes
  that box and when it places the lines, so the two passes agree. A new
  test measurer, `ScaledFont`, grows with the font size, because
  `BlockFont` answers the same for every size and could not tell the
  container's font from the default. Tests: `inline.rs` 3 (a 20 px box
  alone in a 16 px block: baseline 20, line 24, and a 6 px box: line 16,
  the box at 6; "ab" at 10 px in a 40 px block: baseline 30, line 40, the
  text at 22.5, and two lines 80; a bracket, a lone space and nothing are
  still no line), the 281 margin-box test moved 34 → 38. `numbers.rs` 1
  new (the same three through the engine with `ScaledFont`, and an empty
  40 px block 0 tall), and the 281 test's next block moved 46 → 50.
  `measure.rs` 1 (the scaled font). Doctored: the engine passing the
  default font fails the new `numbers.rs` test; the builder starting lines
  at zero fails three `inline.rs` and two `numbers.rs` tests. Corpus: six
  cases moved, each read. `a-picture`, `an-svg-box` and `a-filled-form`'s
  checkbox line gain the descent under a picture, an SVG or a checkbox,
  as browsers do; `alo-offline`'s hand line is 79.77 (56 + 20 + 3.77), the
  heading under it. *What it exposed:* `alo-offline`'s, `web-a-form`'s and
  `a-script-hears-an-event`'s **button** lines also gained a descent, which
  browsers do not add — a button's baseline is its label's, and the engine
  takes every atomic box's as its bottom edge. That is a separate fault the
  strut uncovered rather than caused, and it is **285**.

- [ ] **284. A percentage width on an inline-block resolves against its
  containing block, both times.** *Found by iteration 174, not opened by a
  page.* An atomic box is laid out twice: once to size its slot on the line
  and once, placed, in the room the line gave it. A percentage width is
  resolved against the containing block the first time and against the
  slot the second, so `width: 50%` in a 400 px block reserves 200 on the
  line and draws 100 (112 with 12 px side margins). 200 is right. The
  second pass should be handed the box's size rather than room to find one
  in. *Depends on nothing. Blocked: no page yet* — nothing in alo writes a
  percentage width on an inline-level box, so a page that does opens it.
  An inline `<svg width="50%">` hits it the same way (iteration 178, item
  279), which is why `svg-relative-size` asks its per cents of block-level
  `<svg>`s; that case should gain an inline one when this closes.
  *Closes when:* a
  `numbers.rs` assertion has a 50%-wide inline-block in 400 px drawn 200
  wide, with and without margins, and the line's next text right after it.

- [x] **285. An inline-block's baseline is its last line's.** *Opened by
  `alo-offline`, `web-a-form` and `a-script-hears-an-event` (iteration
  176).* The engine gives every atomic inline box a baseline at its bottom
  margin edge (`engine.rs`'s `lay_out_subtree` reports the border box's
  height). That is CSS's rule for a picture, an SVG and a box with no line
  in it, and wrong for a `<button>`, a text field with a value and an
  `inline-block` holding text: CSS puts their baseline on their **last line
  box in normal flow** (the bottom margin edge only when there is none or
  `overflow` is not `visible`). Since the strut (283), a line holding only
  a button is one font-descent taller than in browsers — 3.77 px under
  alo's offline screen's "Try again", whose label's descent should have
  covered it — and a button beside text sits a descent too high.
  *Depends on nothing.* *Closes when:* a `numbers.rs` assertion has an
  inline-block holding a line of text, alone in a block of the same font,
  making a line exactly its own height, and stand on the baseline of the
  text beside it; an empty inline-block and one with `overflow: hidden`
  still sit on their bottom margin edge; `alo-offline`'s button line is the
  button's height, and `web-a-form`'s and `a-script-hears-an-event`'s
  button lines move back by the descent, as does `alo-agent`'s
  `reading_an_interface` outline (its form back to 44.90176) and
  `alo-renderer`'s `what_a_listener_hears` (its paragraphs back to 44.8
  and 77.09688). A flex or
  grid container's
  baseline (its items', not its last line's) is out of scope: say so where
  it is refused, and queue it if a page needs it.
  **Built (iteration 177).** A new `alo-layout` file, `baseline.rs`,
  holds the rule: the engine records where the last line of every inline
  formatting context in an atomic box's subtree stands (`place_inline_content`,
  after a control's label is centred), and `baseline::last_line` walks the
  box's in-flow block children last first to the first such line. `None`
  — no line, or the box's own `overflow` not `visible` — is the bottom
  margin edge, which `atomic_item` measures because only it has the
  margins; a line's baseline is measured from the margin box's top without
  the bottom margin added. A picture or an SVG has no lines; an absolutely
  positioned child is skipped. **Refused**, ending the search at the
  bottom margin edge (said in the module comment and in
  `docs/conformance.md`): a flex or grid container, whose baseline is its
  items', and a scroll container below the box, on whose baseline browsers
  differ. Tests: `numbers.rs` 3 new, through the engine with `ScaledFont`
  (16 px: 12 up, 4 down) — an inline-block of "cd" alone in a block is
  16×16 and the block 16; beside "ab " both at y 0 in a 16 line; two lines
  in it put "ab " at 16 in a 32 line; margins 6/10 put both at 6 in a 32
  line; with `overflow: hidden` the text drops to 4 in a 20 line; an empty
  20×20 box puts it at 8 in a 24 line; `inline-flex` at 4 in 20; a button
  with no edges is 19.2 tall, the text beside it at 1.6 and the line 19.2.
  Doctored: `last_line` always `None` fails the two baseline tests; no
  overflow check fails the bottom-edge test. Moved tests, each to the
  number this item named: `reading_an_interface`'s form 44.90176,
  `what_a_listener_hears`' paragraphs 44.800003 and 77.09688; and one it
  did not name, `an_agents_text`'s echo 55.2 → 52.625 — an `<output>`
  beside a text field, now level with the field's value (both at 52.625,
  18.625 tall), which is this item's text-field case. Corpus:
  `a-script-hears-an-event` and `web-a-form` are byte for byte their
  references from before the strut (283); `alo-offline`'s button line is
  39.2, the button's height, and `main` 234.45781 at (32, 82.771095) — the
  hand line keeps 283's descent. No other case moved. *What it exposed:*
  an author's `inline-block` holding a `<p>` is broken around it by
  `alo-box` as if it were a `<span>`, so the search through blocks is
  tested only through a button's anonymous label box; that is **286**.

- [ ] **286. An inline-block holding a block is not broken around it.**
  *Found by iteration 177, not opened by a page.* `alo-box`'s
  `build_one` (`tree.rs`, the `holds_a_block` branch) splits any box whose
  `display` is inline-level around a block-level child — right for an
  inline box (`display: inline`, inside `flow`), wrong for an
  `inline-block`, `inline-flex` or `inline-grid`, which establish their own
  formatting context and hold a block like any block container. A
  `<span id=i><p>cd</p><p>ef</p></span>` under `#i { display:
  inline-block }` came out as a 0×0 rectangle on its line rather than one
  atomic box holding two paragraphs. The fix is the branch
  asking for `Inside::Flow` as well; it moves every case that has such a
  box, and each move is to be read. *Depends on nothing. Blocked: no page
  yet* — no corpus page writes an `inline-block`, `inline-flex` or
  `inline-grid` at all (checked iteration 177), so a page that puts a
  block in one opens it. *Closes when:* a box-tree test has an inline-block
  holding two paragraphs as one box with two block children, and a
  `numbers.rs` assertion has it standing on its second paragraph's
  baseline beside text (`baseline.rs`'s search through blocks, end to end).

- [ ] **94. Animations and transitions.** Stage 1 reads them and they change
  nothing, which is correct for a still picture; this is the clock.
  *Depends on 76.*

- [ ] **95. Container queries, `:has()`, cascade layers, `@property`.**
  *Needs design (iteration 165):* no dependency, but four capabilities with
  no ADR, feature contract or closing condition — `LOOP.md` step 2.

- [ ] **96. Filters, `backdrop-filter`, blend modes, masks, `clip-path`.**
  *Needs design (iteration 165):* as 95 — no closing condition, and no page
  in the corpus that fails for want of any of them.

- [ ] **97. `position: sticky`, multi-column, scroll snap, overscroll
  behaviour.**
  *Needs design (iteration 165):* as 95; sticky and scroll snap also need a
  scroll position, which a still render does not have.

- [ ] **98. Writing modes, and layout that is right-to-left** rather than
  mirrored afterwards.
  *Needs design (iteration 165):* as 95 — no ADR, contract or closing
  condition.

- [ ] **99. Paged media and print styles.**
  *Depends on 98 for anything that is not left-to-right.*
  *Needs design (iteration 165):* as 95 — no ADR, contract or closing
  condition.

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
  *Needs design (iteration 165):* as 95 — and hyphenation needs a decision
  on whose dictionaries are rented.

- [ ] **105. Web fonts as pages ship them**: WOFF2, variable fonts, and loading
  that does not flash. **This is also what closes the last honest gap in stage
  1's screens** — the corpus renders in DejaVu Sans and alo loads Inter, so its
  headline wraps one line more here.
  *Depends on 53.* *Needs design (iteration 165):* 53 is done, but this is
  three capabilities — a rented WOFF2 decoder, `@font-face` fetched over the
  network stack, and a loading policy — with no ADR for the rental and no
  closing condition. Cut it into those before starting.

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

- [x] **178. A rotated picture is drawn rotated.** Today only the rectangle's
  corners are transformed, so a picture under a `rotate()` draws upright inside
  the right area. Wrong and visible, which is why it was preferred to drawing
  nothing.
  *Depends on 176. Closes when:* a picture under a rotation is drawn rotated, and
  a reference render says so.

  **Done (iteration 165).** `alo-paint`'s new `drawn_picture.rs`, split from
  `render.rs`. A transform that only moves and grows the rectangle keeps the
  exact whole-pixel path, so no scaled picture in the corpus moved. Anything
  else — a rotation, a skew, and a **mirror**, which the old path also drew
  the wrong way round — rasterises the rectangle as a transformed shape, so
  its outline is anti-aliased, and samples each covered pixel's centre back
  through the inverted transform. 11 unit tests: a quarter turn, a half turn
  and a mirror each move four distinct quarters to the right corners; an
  eighth of a turn is a diamond whose points reach past the square while the
  square's own corners stay white; the outline is blended; an upright scale
  still fills whole pixels; a flattening or non-finite transform draws
  nothing without failing. Doctored with the old upright-only path, 7 of them
  fail and so does the corpus. Corpus case `a-turned-picture` is the
  reference render (30°, 90° and `scaleY(-1)`); no other reference moved.
  Still nearest-neighbour inside (179), and a picture still ignores a clip
  in force — found reading the renderer, no page fails on it, no item opened.

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

- [x] **180. GIF, WebP and AVIF.** Rented. The same bound and the same refusals
  as PNG and JPEG, added to the one list the tests already walk.
  *Depends on 177. Closes when:* each decodes a frozen file and each is refused
  the same way — and an animated GIF shows its first frame rather than nothing,
  because a still picture is a better answer than a gap while item 109 is
  outstanding.
  *Scope cut on starting: AVIF went to item 269.* Its pixels are an AV1 frame,
  and no AV1 decoder was found that could be rented without `unsafe` this
  engine would have to answer for. Choosing one is a decision, so it needs an ADR, not a line in a
  manifest.

  **Done for GIF and WebP (iteration 166).** `gif` and `image-webp`, both pure
  Rust and both `#![forbid(unsafe_code)]`, each named in one file of
  `alo-paint`: `gif_picture.rs` and `webp_picture.rs`. JPEG moved to
  `jpeg_picture.rs` in the same change, so `picture.rs` is only the format
  sniffing and the one bound, `agreed_size`, which PNG now uses too.
  Seven frozen files in corpus case `a-picture-in-each-format`, made by Pillow
  from `a-picture`'s stripes, with the script in `origin.txt`. The cases cover
  plain, transparent and animated GIF, and lossy, lossless, extended-with-alpha
  and animated WebP. Both animated files show their first frame.
  Two size claims are bounded that the decoders would otherwise act on first:
  a GIF frame bigger than its screen, and an extended WebP's lossy bitstream
  bigger than its canvas. For the second, `image-webp` reserves memory by the
  bitstream's own header, up to 16 383 square, before comparing.
  **Mutation testing found a panic in `image-webp` 0.2.4, its newest release.**
  In an animation frame with an alpha chunk, the decoder never checks the
  lossy picture against the frame's size. After an `ALPH` chunk it also
  decodes the next chunk as that picture whatever it is named. A picture
  larger than its frame then indexes past the alpha plane and panics. Both
  variants were found and are refused in front of the decoder. Each has a
  regression test built from the frozen file, and each test panics inside the
  crate when the check is doctored out.
  Every byte of all nine frozen files is flipped through `read`. A corrupt file
  either is refused, or keeps its size, or changes size only because the
  flipped byte was the size field. A GIF screen and an animated WebP canvas
  repeat their size nowhere else, so a flip there can do that honestly.
  Still owed: AVIF (269); upstream has not been told about the panic, which
  is a person's call to make.

- [ ] **269. AVIF.** Cut from 180. An AVIF is an AV1 frame in an ISO-BMFF
  box, and the box is the easy half. The AV1 decoder is the hard half: `dav1d`
  is C, `rav1d` is a Rust translation of it that keeps a great deal of
  `unsafe`, and iteration 166 found no AV1 decoder that is both pure Rust and
  free of `unsafe` (a survey to redo in the ADR, not a settled fact). ADR 0005 says untrusted
  bytes are decoded in the least privileged process that can do it, and law 4
  says `unsafe` needs a named, reviewed boundary. Which decoder, on what
  licence, behind what boundary, is the decision.
  *Depends on 180. Needs ADR*, as its own iteration. *Closes when:* an AVIF
  decodes from a frozen file, through `picture::read` and `agreed_size` like
  the other four, and is refused the same way.

  **Decided: ADR 0021 (iteration 167).** `avif-parse` for the box and `rav1d`
  for the frame, through **`rav1d`'s own safe Rust API**, with its assembly
  off, one thread, and both named in `alo-paint`'s `avif_picture.rs`; the
  colour conversion is ours (§ 5). The survey was redone and found the same
  answer for a sharper reason: `rav1d` 1.1.0, the newest release, offers only
  dav1d's C ABI, so calling it would be `unsafe` in this repository. Its Rust
  API was merged on `main` in April 2026 (rav1d #1439, #1484) and is
  unreleased; a git pin and our own FFI are both refused (§ 2).
  ***blocked:** a `rav1d` release carrying `rust_api.rs`.* Lifted by that
  release and nothing else; ADR 0021's last section says what reopens the
  choice. Until then an AVIF is refused, and § 6 says the browser never
  claims a format it does not decode.

- [x] **107. SVG** — *"a second rendering model inside the first, and far larger
  than its one line here suggests."* **Cut this before starting it**; it is
  several iterations and nobody should discover that halfway through.

  **Decided and cut: ADR 0022 (iteration 168).** An outermost `<svg>` is one
  replaced box, nothing inside it is a CSS box, and its contents become a
  drawing of filled and stroked paths, made by a new crate `alo-svg` after
  layout and handed to paint by box, as a picture is. The page that opens it
  is **alo's own offline screen** (`alo-workplace/web/public/offline.html`),
  whose only picture is an inline `<svg>` hand: four stroked `<path>`s with
  arcs. Today it is a 56 × 56 hole. 107 is not built itself. It is the eight
  items below, and it closes when 270–273 have closed. 274–277 open only when
  a page needs them. ADR 0022 § 7 lists what stays refused.

  **Closed (iteration 172)**, as its own text says, by 270–273 closing: the
  offline screen's hand is drawn. What SVG still owes is items 274–278 and
  287 (279 closed by iteration 178), each
  with its own closing condition, and `ROADMAP.md`'s *SVG* line stays open
  for them.

- [x] **270. The `<svg>` box.** *Cut from 107 (ADR 0022 §§ 1, 4).* An
  outermost `<svg>` laid out as a replaced element through item 176's path. Its
  size comes from CSS, then from the `width` and `height` attributes, then from
  a ratio from the `viewBox`, then 300 × 150. Its ratio comes from the
  `viewBox`. Its descendants produce no boxes, and `display: none` inside it
  only hides. In the agent tree it is one image node named by `aria-label`,
  `aria-labelledby`, then its first child `<title>`, and it is absent under
  `aria-hidden`.
  *Depends on 176. Closes when:* a layout assertion pins the offline screen's
  `<svg>` at 56 × 56 with no child boxes, and an `<svg>` sized only by
  attributes, one sized only by a `viewBox`, and one with neither each have
  the size § 1 gives. A test shows the agent node's name from each source and
  its absence under `aria-hidden`.

  **Done (iteration 169).** `alo-box`'s new `svg.rs` says what an outermost
  `<svg>` is: one box with no children whatever its `display` (and `contents`
  on one is `none`, as on any replaced element), its natural size read from
  `width`, `height` and `viewBox`, and its first child `<title>` as a name
  after ARIA's. A natural size is now `alo-box`'s `NaturalSize` — a width, a
  height and a ratio, each optional — because a `viewBox` alone is a shape
  with no size. `alo-layout`'s new `replaced.rs` is CSS 2's replaced-element
  rule in numbers, including the 300 × 150 default and a ratio-only box
  filling its room. A replaced box is now atomic on a line whatever its
  `display` says. Corpus case `an-svg-box` is the layout assertion and the
  reference render: the offline screen's own `<svg>` and rule at 56 × 56
  with no child boxes, absent from the agent tree under `aria-hidden`; 48 × 24
  from attributes and named by `<title>`; 80 × 40 from a width and a
  `viewBox`, named by `aria-label`; a `viewBox`-only `<svg>` made
  `display: inline` filling its 320-wide paragraph at 320 × 80, named by
  `aria-labelledby`; and 300 × 150 with nothing written. Hostile `viewBox`
  and size values (non-finite, overflowing, negative, a million numbers) are
  refused and recorded, never a panic. No other reference moved.
  A `width` in per cent, `em` or `calc()` is recorded and not used: SVG 2
  makes it a presentation attribute, which is 271's cascade work.

- [x] **271. Shapes, filled.** *Cut from 107 (ADR 0022 §§ 2, 3).* The crate
  `alo-svg` and the drawing handed to paint by box id, beside
  `PaintContext::pictures`. Also: `viewBox` and `preserveAspectRatio` as one
  transform, `rect` (with `rx`/`ry`), `circle`, `ellipse`, `line`, `polyline`,
  `polygon`, `<g>`, a nested `<svg>` viewport, the `transform` attribute,
  `fill`, `fill-rule`, `fill-opacity` and `opacity`. Presentation attributes
  enter the cascade as author declarations of specificity zero, and
  `currentColor` inherits from the HTML around the `<svg>`.
  *Depends on 270. Closes when:* a reference render shows each shape filled,
  including `evenodd` against `nonzero`, a `currentColor` icon taking its
  paragraph's colour, and a stylesheet overriding a presentation attribute.
  Shape geometry and the viewport transform have unit tests in numbers. Every
  count § 5 names for these elements is bounded and tested hostile.

  **Done (iteration 170), with two cuts.** The new crate `alo-svg` walks an
  outermost `<svg>` after layout, in document order with its own stack, and
  makes an `alo_paint::Drawing` — paths in the box's coordinates with every
  SVG transform applied, each with a colour and a fill rule, and the groups
  `opacity` fades. The pipeline (`alo-renderer`'s `drawings.rs`) hands it to
  paint by box beside the pictures, and paint (`build.rs`'s `drawing_of`)
  draws it into the content box, clipped there by the user-agent sheet's new
  `svg { overflow: hidden }`. Paint gained `FillRule` (its display list says
  `evenodd` where it is not non-zero) and `fill_on_page`, which makes only the
  coverage that lands on the page, so a stranger's 60 000-pixel rect costs a
  page of mask and not 3.6 GB. `alo-style`'s new `presentation.rs` puts
  `fill`, `fill-opacity`, `fill-rule`, `opacity`, `display`, `visibility` and
  `color` attributes into the cascade as author declarations of specificity
  zero, counted before every sheet, invalid ones ignored and recorded; the
  three `fill` properties inherit. Shapes follow SVG 2's equivalent paths
  (rect with `rx`/`ry` rules, circle, ellipse, polygon, polyline; a line fills
  nothing). `viewBox` with `preserveAspectRatio` (all nine alignments, `meet`,
  `slice`, `none`) is one matrix; the `transform` attribute is SVG's own
  grammar, ignored whole on any error. A shape's own `opacity` is folded into
  its fill's alpha (one fill, the same as a group of one); a container's is a
  group, and an empty group is taken back out. Bounds, each tested at its
  edge: 65 536 points per `points`, 262 144 segments and 65 536 elements per
  drawing, 256 levels of nesting, 16 groups open at once and 64 in all; past
  any of them the drawing is refused whole and recorded. `<path>`, `<use>`,
  `<text>`, a nested `<svg>`, `<image>`, `<foreignObject>`, SMIL, and
  `clip-path`, `mask`, `filter` and markers are left out and recorded; paint
  servers draw their fallback colour, recorded. Corpus case
  `svg-shapes-filled` is the reference render: every shape, a star under
  `nonzero` and `evenodd`, a faded group against two `fill-opacity` shapes,
  a turned square, a stylesheet beating `fill="red"`, two `currentColor`
  icons in their paragraphs' colours, and the four aspect-ratio behaviours
  with a slice cut at its box. `an-svg-box` moved, as it should: the `<rect>`
  in its attribute-sized `<svg>` is now drawn black, and its four paths are
  recorded for 272. **Cut:** a nested `<svg>` viewport (278), and the
  `transform` property on SVG elements with per-cent and `em` `width` and
  `height` on an outermost `<svg>` (279; its `transform` half later cut
  again, to 287).

- [x] **272. Path data.** *Cut from 107 (ADR 0022 §§ 2, 5).* The `d` grammar,
  every command, absolute and relative, with arcs converted to cubic curves
  by SVG 2's implementation notes. A path is drawn up to its first error and
  no further. Segments per path and per drawing are bounded.
  *Depends on 271. Closes when:* unit tests pin each command's segments in
  numbers, including arcs whose radii need correcting and arcs of zero
  radius. A reference render shows a path of every command. Malformed,
  truncated and adversarial data (huge counts, non-finite numbers, exponent
  overflow) is refused or cut at the error, and never panics.

  **Done (iteration 171).** `alo-svg`'s new `path_data.rs` is SVG 2's `d`
  grammar, ours as ADR 0022 § 2 decided: `M L H V C S Q T A Z`, capital and
  lower case, pairs after a move as lines, `S` and `T` reflecting only a
  curve of their own kind, packed arc flags (`a1 1 0 0110 10`), and a new
  subpath at the same start for anything after `Z`, written into the path as
  a move. The new `arc.rs` is SVG 2's implementation notes F.6.2 and F.6.5 in
  `f64`: an arc ending where it starts is nothing, a zero radius is a line, a
  negative radius its size, radii too small are scaled up keeping their
  ratio, and the sweep is cut into at most four cubic curves with handles at
  `4/3 · tan(θ/4)`. Path data is drawn up to the last complete command before
  its first error (a half-written set is not drawn) and the error's byte is
  recorded; data not beginning with a move draws nothing. Every point is
  finite or it is the error: overflowing exponents, relative steps whose sum
  overflows, a reflected handle past a float, and an arc reaching past one
  all stop the path there. Bounds: 65 536 segments per path
  (`bounds::MOST_PATH_SEGMENTS`, an arc counted as every curve it makes,
  closes counted too), checked as the path is made, under the drawing's
  existing 262 144; past either the drawing is refused whole. `number.rs`'s
  scanner is now the shared `scan`, so `points`, `transform` and `d` read
  numbers one way. Corpus case `svg-path-data` is the reference render: a
  house absolute and relative, `C`/`S`, `q`/`t`, a heart of packed arcs, an
  `evenodd` ring of four arcs, the four flag pairs between two points,
  corrected and zero radii, a turned ellipse, a path cut at an error, and a
  relative line after `Z`. `an-svg-box`'s four "path data is item 272" issues
  are gone, as they should be: the offline hand's paths are `fill="none"`,
  so nothing new is drawn there until 273 strokes them.

- [x] **273. Strokes, and alo's offline screen.** *Cut from 107 (ADR 0022
  §§ 2, 5, 6).* `stroke`, `stroke-width`, `stroke-linecap`,
  `stroke-linejoin`, `stroke-miterlimit`, `stroke-opacity`,
  `stroke-dasharray` and `stroke-dashoffset`, through `tiny-skia`'s stroker
  and dasher, named only in `alo-paint`'s `raster.rs`. Dashes are bounded
  before they are made.
  *Depends on 272. Closes when:* the offline screen is frozen into the corpus
  as an alo case, with its hand drawn. Its reference render and box tree are
  committed. A reference render covers every cap, join and a dash pattern.
  A dash array that would make millions of dashes is refused, with a test.

  **Done (iteration 172).** `alo-paint`'s new `stroke.rs` is paint's
  vocabulary for a stroke (width, `LineCap`, `LineJoin`, miter limit,
  `Dashes`), and `raster.rs`'s new `outline` is the rented half: `tiny-skia`'s
  dasher and stroker, still named nowhere else, turning a path into the
  outline it covers. A stroke reaches paint as **the fill of that outline**,
  so `DrawingItem` and the display list gained nothing. `alo-svg` outlines in
  **user space** and transforms the outline with the shape, so a stroke under
  `scale(2 1)` is squashed with it; the resolution handed to the stroker is
  the transform's larger axis, so a 24-unit icon drawn at 56 px is offset
  finely enough. New files: `paint.rs` (`<paint>`, lifted out of `fill.rs` and
  shared by `fill` and `stroke`), `stroke.rs` (every `stroke-*` property from
  the computed style; a value in error is its initial value, recorded),
  `dashes.rs` (`stroke-dasharray`: commas or spaces, an odd list written
  twice, a sum of nothing solid, a negative length an error, percentages of
  the viewport's diagonal over √2). `alo-style` gained the eight stroke
  presentation attributes, each held to its grammar, and all eight inherit.
  `<line>` is now a path (stroked, never filled), and path data of moves and
  closes is kept so `M5 5Z` under a round or square cap is a dot. A shape
  with a fill and a stroke under `opacity` is one group, as 271 promised;
  with one of them, the opacity is folded into its colour. `paint-order` and
  `vector-effect` are recorded and not applied. **Bounds**, each tested at
  its edge: 256 lengths per `stroke-dasharray` (read no further than that),
  16 384 dashes per path counted *before* dashing along the path's control
  points (never fewer than the dashes laid), and every outline's segments
  counted toward the drawing's 262 144; past any of them the drawing is
  refused whole. Corpus case `svg-strokes` is the reference render: the three
  caps and three joins with guide lines, a spike beveled at the default
  miter limit and mitered at ten, a zigzag, dots from a zero-length `<line>`
  and `M Z`, a fill under its stroke, a squashed circle, even, odd and offset
  dashes, a dotted circle, dashes restarting per subpath, a dashed curve,
  `currentColor`, a stylesheet beating `stroke="red"`, `stroke-opacity`,
  `opacity` as one group beside the same faded alone, and a per-cent width.
  **Corpus case `alo-offline` is alo's offline screen, frozen byte for byte**
  from `alo-workplace` (origin and hash in its `origin.txt`), with its
  reference render, box tree and layout committed: the hand is four
  terracotta outlines in its 56 × 56 box, and `an-svg-box`'s hand (now on the
  offline canvas colour, so it can be seen) moved with it. The page also
  showed three layout faults that are not strokes; they are items 280–282.

- [ ] **274. `<defs>`, `<symbol>` and `<use>`.** *Cut from 107 (ADR 0022
  §§ 5, 6).* The icon sprite. Expansion is bounded and cycles are refused,
  because `<use>` is SVG's billion laughs.
  *Depends on 273. Opened only by a frozen page that needs it*, as 179 is.

- [ ] **275. Gradients and patterns as SVG paint.** *Cut from 107 (ADR 0022
  § 6).* `linearGradient` and `radialGradient` through the gradients paint
  already draws (item 19), and `pattern`.
  *Depends on 273. Opened only by a frozen page that needs it.*

- [ ] **276. `<text>` in SVG.** *Cut from 107 (ADR 0022 § 6).* Text placed by
  coordinates and shaped by `alo-text`.
  *Depends on 273. Opened only by a frozen page that needs it.*

- [x] **277. An SVG file as a picture.** *Cut from 107 (ADR 0022 § 6).*
  `<img src="…svg">` and SVG in `background-image`. A standalone SVG file is
  XML, and XML is stage 3's item 139. *Needs ADR*, as its own iteration: which
  parser is rented, how it is held to SVG documents only, and the secure
  static mode (no script, no animation, no fetch of anything outside the
  file).
  *Depends on 273.*

  **Decided: ADR 0027 (iteration 190)**, opened by alo's own Meet screen,
  whose greeting and hero draw `<img src={wavingHand}>` from
  `alo-workplace/web/src/assets/alo-waving-hand.svg`. An SVG picture is
  recognised by the type `image/svg+xml` (or `.svg` for a resource with no
  response) and an SVG `<svg>` root, never by sniffing. It is read by
  `quick-xml`'s pull reader, rented in `alo-dom`'s `xml.rs` alone, into a
  document of our own. The rules are UTF-8 and XML 1.0 only, no internal DTD
  subset, no entity but the five predefines and character references, and
  any XML error refuses the whole picture. The document is drawn by
  `alo-svg` at the `<img>`'s size with its own cascade. It sees nothing of
  the page, runs nothing, animates nothing and can cause no request. The
  agent reads the `<img>` by `alt`. 277 is not built itself. It closes when
  309 and 310 close, and its `background-image` half is 311.

  **Closed (iteration 192)**, as its own text says, by 309 and 310 closing:
  Meet's greeting shows the hand from its file. What remains of SVG in
  `background-image` is 311, opened by a page.

- [x] **309. Reading an SVG file.** *Cut from 277 (ADR 0027 §§ 2, 3, 5).*
  `alo-dom`'s `xml.rs`: `quick-xml` with default features off, named there
  and added to `gate.sh`'s boundary list, building an `alo_dom::Document`
  from pull events on our own open-element stack. It is configured
  strictly: end names checked, no unmatched ends, no bare `&`, comments
  checked, duplicate attributes and unbound prefixes refused. UTF-8 and
  version 1.0 only. A DOCTYPE is ignored without an internal subset and
  refused with one. Only the five predefined entities and character
  references are allowed. Processing instructions are ignored, CDATA is
  read as text, and the root must be SVG's `svg`. Bytes, elements, depth,
  attributes and name, value and text lengths are bounded before the work.
  *Depends on nothing. Closes when:* `alo-waving-hand.svg`, frozen byte for
  byte, reads into `svg`, `title`, `g` and two `path`s in the SVG
  namespace; every refusal in ADR 0027 § 3 and every bound has a named
  test at its edge; and every prefix and every flipped byte of that file
  returns a document or a refusal, never a panic.

  **Built (iteration 191).** `alo-dom`'s `xml.rs`, `alo_dom::read_svg`,
  with `quick-xml` 0.41 behind it and in `gate.sh`'s boundary list. The
  hand is frozen as `alo-corpus/pictures/alo-waving-hand/picture.svg` and
  reads into `svg`, `title`, `g` and two SVG `path`s
  (`alo-dom/tests/an_svg_file.rs`). Every refusal is a named `XmlRefusal`
  with its own test, and every bound (1 MiB, 65 536 elements, 256 deep, 256
  attributes, 1 KiB names, 256 KiB values and text runs) is pinned at its
  edge. Every prefix is refused until `</svg>` and accepted from it. Every
  byte flipped three ways answers without a panic. The reader's own
  duplicate check is off, because ours compares namespace and local name.
  The `Name` production is not checked, as ADR 0027 § 2 allows until a file
  shows it matters.

- [x] **310. `<img src="…svg">`.** *Cut from 277 (ADR 0027 §§ 1, 4–6).* The
  resource's type is carried beside its bytes. The image document gets its
  own cascade (the user-agent sheet and its own `<style>`). Its natural size
  comes from the root's absolute `width` and `height`, and its ratio from
  those or the `viewBox`. The pipeline draws it through `alo-svg`'s `draw`
  at the `<img>`'s content box and hands that to paint by box. It runs in
  secure mode, with no realm and no network handle, and the agent's node is
  named by `alt`. *Depends on 309. Closes when:* Meet's greeting, frozen as
  an alo corpus case with the hand file byte for byte, has a layout
  assertion of its `<img>` box and a committed reference render with the
  hand drawn. A test shows a file holding a script, an `<image>`, an
  external `<use>`, an `@import` and an external DTD draws its own shapes
  and causes no request. Another shows the same bytes under a type other
  than `image/svg+xml` are refused.

  **Built (iteration 192).** `alo-renderer`'s new `resource.rs` is the
  type beside the bytes: `Resource` (a `src`, an optional `Content-Type`,
  the bytes), whose `is_svg` compares the type's essence with
  `image/svg+xml` ignoring case, and whose `from_file` lets a file's own
  `.svg` stand in for the type when there was no response. The corpus reads
  every frozen picture that way. `pictures.rs`, moved out of `pipeline.rs`,
  reads every `<img>`'s resource: SVG-typed bytes go to `svg_picture.rs`
  and never to a raster decoder, and everything else goes to the raster
  decoders and is never read as SVG. `SvgPicture` holds the document
  `read_svg` made, its root, and its natural size from
  `alo_box::svg::natural_size`. Its `draw` runs the document's own cascade
  (the user-agent sheet and its own `<style>`s, media queries sized to the
  picture, a `<link>` recorded as not fetched) and `alo_svg::draw` at the
  `<img>`'s content box. The pipeline merges that drawing into paint's
  drawings by box, after layout. `alo_paint::Drawing` gained `confined`, so
  a picture's drawing is clipped to its box whatever the `<img>`'s
  `overflow` says. The issues from a file are prefixed with its `src`.
  Nothing in `svg_picture.rs` is handed resources, sheets or a realm: it
  takes bytes and a size. Corpus case **`alo-meet-greeting`** is Meet's
  header written out from `MeetModule.tsx` at `738de614` with the hand
  frozen once in `pictures/`: layout, box tree, agent tree and reference
  render committed, with the hand's two paths in its 20 × 20 box.
  `alo-corpus/tests/meets_greeting.rs` is the layout assertion, and
  `alo-renderer/tests/an_svg_picture.rs` covers the rest: natural sizes,
  vectors at the box's size and clipped, the same bytes refused under four
  other types, a PNG typed as SVG never decoded, a file asking for a
  script, `<image>`s, external `<use>`s, `@import`s, a stylesheet PI, a
  `<link>` and an external DTD drawing only its own rect with nothing it
  named reaching the page, the page's colour and custom properties not
  reaching inside, the agent reading only the `alt`, and refused files
  keeping their box. No other reference moved. The page also showed three
  faults that are not the picture's (312, 313, 315), and the tests one more
  (314).

- [ ] **311. A picture in `background-image`.** *Cut from 277 (ADR 0027
  § 7).* `url()` in `background-image`, raster or SVG: today it draws only
  gradients, so a picture behind a box is not built for any format. SVG
  reaches it through 310's path. *Depends on 310 for SVG. Opened only by a
  frozen page that needs it*, as 179 is. No alo stylesheet names one today.

- [x] **312. `vertical-align`.** *Opened by `alo-meet-greeting`
  (iteration 192).* Nothing reads the property, so every inline box sits on
  its line's baseline whatever it says. Meet's greeting icon is
  `vertical-align: middle` and sits a few pixels high, and nothing records
  that. The keywords `baseline`, `middle`, `top`, `bottom`, `text-top`,
  `text-bottom`, `sub` and `super`, and a length or a per cent, in the line
  builder, with the line box growing to hold what moved.
  *Depends on nothing. Closes when:* a `numbers.rs` assertion places a
  20 px atomic box under each keyword in a 13 px line, its midpoint for
  `middle` at the baseline plus half the x-height. `alo-meet-greeting`
  moves, and its hand sits where browsers put it.
  **Built (iteration 193):** `alo-layout`'s `vertical_align.rs` reads every
  keyword, a length and a percentage of the box's own `line-height`, and
  says how far each puts a box from its parent's baseline. `inline.rs`
  hangs every piece from its parent's baseline, moved by that distance.
  `top` and `bottom` are groups of their own, held by the line box's edge
  once the rest is settled and placed again on every line a held inline
  box reaches. Atomic boxes and inline boxes, with everything inside them,
  both read it. `MeasureText` gained `x_height`, and `alo-text` measures a
  face's own `x` when its OS/2 table has no x-height, as DejaVu's has none.
  `numbers.rs` places a 20 px inline-block in a 13 px line under all eleven
  values, and a `super` `<sup>` with a `<b>` inside it. `meets_greeting.rs`
  puts the hand's middle at the baseline less half of DejaVu Sans Bold's
  x-height. `alo-meet-greeting` moved, its line now the hand's 20, and so
  did `text-decorations`' line-through, by 0.33 px, from the measured
  x-height.

- [x] **313. A background of several layers.** *Opened by
  `alo-meet-greeting` (iteration 192).* `background: radial-gradient(…),
  var(--bg-app)`, Meet's `.module`, is two layers, and paint reads a
  background only as one gradient or one colour, so it draws nothing and
  records nothing. The comma list as layers, the last one's colour beneath
  them all, each gradient painted in order, the first on top. A layer this
  engine cannot draw (a `url()`, item 311) is recorded and the rest still
  drawn. *Depends on nothing. Closes when:* a reference render shows a
  gradient over a colour and two gradients over each other, a layer list
  that cannot be read is recorded, and `alo-meet-greeting`'s tint appears in
  its top right corner.
  **Built (iteration 194):** `alo-value`'s `background.rs` holds a list of
  layers, first on top, and the colour beneath, and `parse.rs` reads
  `background` and `background-image` into it: a layer is `none`, a
  gradient or a `url()`, and the last may also hold a colour. A layer that
  says anything else (a position, size, repeat or box) refuses the whole
  list. Meet's tint needed radial gradients that are more than centred
  ellipses, so `Gradient::Radial` gained a `Shape`, an `Extent` (all four
  keywords) and a `Position` (one or two keywords, percentages or pixel
  lengths), and `alo-paint`'s `paint.rs` places the last ring for each.
  Sizes written as lengths and positions of three or four parts are refused.
  Stops now mix premultiplied, as CSS says, so a fade to `transparent`
  keeps its colour. `alo-paint`'s new `background.rs` reads a box's
  background once, and `build.rs` draws the colour and then each layer from
  the bottom up. Paint now has an issue list: a `url()` layer is recorded
  as `UndrawnLayer` while the rest are drawn, an unreadable list as
  `UnsupportedValue`, once per box however many pieces it is in. The
  renderer's issues include it. The new corpus case `background-layers` has
  a gradient over a colour, two gradients crossed, a circle and an ellipse
  placed by keyword, a `url()` layer and an unreadable list.
  `alo-paint/tests/background_layers.rs` reads the order back in pixels.
  `alo-meet-greeting` moved: the tint is in its top right corner, and
  `meets_greeting.rs` reads its centre, half way out and past its edge.
  No other reference moved.

- [ ] **314. An `<img>` given a width and a height keeps neither ratio nor
  height.** *Found by iteration 192's tests, not opened by a page.* A
  replaced box with a natural ratio and both a CSS `width` and `height` is
  laid out `height = max(height, width / ratio)`: a 24 × 24 picture styled
  `80px` by `40px` is 80 × 80, and `30px` by `40px` is right. Raster and SVG
  pictures alike. A definite width and height are the size, and the
  picture is stretched (`object-fit: fill`). The automatic minimum size from
  the ratio looks like the cause and has to be confirmed first. *Depends on
  nothing. Blocked: no page yet*, as 284 is: Meet's icon is square and
  meets nothing here. *Closes when:* a `numbers.rs` assertion has an
  `<img>` of each kind styled wider and taller than its ratio laid out at
  exactly its width and height.

- [x] **315. `rem` and `em` in a media query.** *Opened by
  `alo-meet-greeting` (iteration 192).* Meet's stylesheet says
  `@media (max-width: 48rem)`, and a length in a media feature can only be
  in pixels here, so the query is recorded as not understood and treated as
  not matching. At Meet's width the answer is right by chance. In a window
  narrower than 768 pixels, Meet keeps its wide layout. Media Queries
  resolve `rem` and `em` against the initial font size, 16 px, and not the
  page's. *Depends on nothing. Closes when:* a test in `alo-css`'s
  `media.rs` matches `48rem` and `48em` at 767 and 768 pixels, and
  `alo-meet-greeting` drops that issue.
  **Built (iteration 195):** `media.rs` keeps a width as a `QueryLength`
  in the unit it was written in (`px`, `em`, `rem`, or a bare zero) and
  writes it back that way. `em` and `rem` are `QUERY_FONT_SIZE`, 16 px,
  which a test in `alo-style`'s `metrics.rs` holds equal to
  `DEFAULT_FONT_SIZE`. Other units (`ex`, `vw`, a bare number) are still
  refused and recorded. `media.rs` matches `48rem` and `48em`, in any case,
  at 767, 768 and 769 under `max-width`, `min-width` and `width`, and
  hostile lengths (`1e38rem`, a number past `f32`, a split unit) are
  answered without a panic, an infinite breakpoint wider than any window.
  `alo-meet-greeting`'s `issues.txt` lost the line and nothing else moved.
  `meets_greeting.rs` lays Meet out at 768 and 769: the header at (12, 36)
  and the heading at 28 px under the narrow block, at (24, 44) and 32 px
  above it.

- [x] **316. `line-height` on text.** *Found by iteration 195's test of
  `alo-meet-greeting`.* A line box is as tall as its fonts' ascents and
  descents, and `line-height` changes nothing about it: Meet's heading,
  `line-height: var(--leading-tight)` (1.25) at 32 px, is 37.25 tall where
  browsers make it 40, and its body text at 1.5 is laid out at the font's
  own height, so everything under the heading sits high. 283 deferred this
  to "its own item when a page needs it", and Meet, the offline screen and
  the sign-in screen all set it. Half-leading on every inline box and the
  strut, from `ComputedStyle::line_height`, which is already resolved. It
  moves nearly every reference, and each move is to be read. *Depends on
  nothing. Closes when:* a `numbers.rs` assertion has a 16 px line at
  `line-height: 1.5` 24 tall with the text's baseline half the leading
  down, a mixed line whose half-leadings differ, and `alo-meet-greeting`'s
  heading 40 tall.
  **Built (iteration 196):** `alo-layout`'s `TextStyle` carries a
  `line_height`, `None` for `normal`, from `ComputedStyle::set_line_height`;
  `alo-style` tells a set line height from `normal` with `set_line_height`
  in `metrics.rs`, and a value that is negative, not finite or unreadable
  now computes to `normal` rather than to 1.2 written as pixels. The line
  builder gives text, an inline box and the strut a `Reach`, their font's
  ascent and descent with half the leading on each side, and an inline box
  is aligned by it, as CSS 2.1 § 10.8.1 says. Fragments are still the
  font's height. `normal` adds nothing, which is right for DejaVu, whose
  line gap is zero. Tests: `numbers.rs` 3 new (16 px at 1.5 is 24 with the
  text 4 down; an inherited number at 32 px is 48; 0.5 is a negative
  leading, the text at -4; `normal` is 16; a mixed line of three
  leadings is 42 with the baseline at 24; `text-top` aligns a span's line
  height, not its letters); `metrics.rs` and `computed.rs` one each for
  `normal` against set and ten hostile values; `a_hostile_line_height.rs`
  renders with real fonts, nine unusable values laying out as `normal` and
  six enormous or tiny ones drawn without a panic. Doctored: no leading
  fails all three `numbers.rs` tests. Corpus: four cases moved, each read
  — Meet (heading 40, last paragraph 22.5, greeting line 20.70),
  `alo-offline` (the paragraph 3 × 24, the column re-centred 8.06 up),
  `alo-sign-in` (the 40 px heading at 1.06 four lines of 42.4, the text
  2.08 above its box; the paragraph 3 × 25.5) and `alo-settings` (two
  `.sectionDesc` lines at 1.7, the dialog 13.93 taller and 6.97 higher).
  `meets_greeting.rs` asserts the heading at 35 and 40 and derives the
  greeting's line from its face; `an_agent_on_settings.rs`'s pinned rows
  moved with the dialog, and `alo-window`'s three references that hold the
  offline page moved with it.

- [x] **319. A `<br>` ends its line.** *Opened by `alo-downloads`
  (iteration 198), alo's public download page, frozen exactly.* Its note is
  three sentences separated by `<br><br>`, and a `<br>` was an empty inline
  box that ended nothing: the sentences ran together on five lines where a
  browser sets eight. HTML's rendering section says a `<br>` is a forced
  line break; CSS has no `display` for that, so the box tree says it. *Depends
  on nothing. Closes when:* the note in `alo-downloads` is eight lines, two
  of them blank, in a layout assertion, and a `<br>` anywhere a page can
  put one is drawn without a panic.
  **Built (iteration 198):** `alo-box`'s `line_break.rs` says which boxes are
  breaks (a `<br>` that is `display: inline`), and `BoxTree` keeps them in a
  side set, `is_forced_break`, like its legends; `boxes.txt` marks one
  `· line break`. `alo-layout` has `InlineItem::Break`, which stands a
  zero-width fragment in its own font where what is drawn on the line ends
  (after the trailing space is removed), counts its `line-height` towards the
  line it ends, and ends it. A break alone on its line makes the line; one
  with nothing after it makes none. An author who gives a `<br>` another
  `display` gets the box they wrote, where browsers keep it a break;
  `docs/conformance.md` says so. Tests: `line_break.rs` 3, `tree.rs` 2,
  `inline.rs` 7 (doctored, five fail without the line ending);
  `tests/alo_downloads.rs` (the note 189.62 tall, the breaks one line
  apart, the blank ones and "Prefer the web?" at the 40 px gutter, and the
  page's script refused at its regular expression);
  `tests/a_hostile_line_break.rs` (5000 breaks are 5000 lines; `</br>` is a
  break; fourteen odd placements and enormous values drawn without a
  panic). No other case holds a `<br>`, so no other reference moved.

- [x] **320. `ch` and `ex` from the font.** *Opened by `alo-downloads`
  (iteration 198).* `.lede` is `max-width: 44ch` at 17.28 px: in DejaVu
  Sans, the `0` is 0.636 em, so 44ch is about 483.7 px, and it is laid out
  380.16 because `alo-style`'s `metrics_for` still gives `ch` and `ex` as
  half the font size, a stand-in its own comment calls an estimate.
  `alo-text`'s `FontMetrics` already measures the face's `0` and its
  x-height, so this is getting the first available face's measurements
  into the cascade's length resolution without `alo-style` depending on a
  font file. *Depends on nothing. Closes when:* a layout assertion has
  `.lede` 44 times the face's `0` advance at 17.28 px, `ex` is the face's
  x-height in a `numbers.rs` assertion, a face with no `0` still falls back
  to half an em, and every reference that moves is read.
  **Built (iteration 210):** `alo-style`'s `font_units.rs` is the seam:
  `MeasureFace::face_units(&ComputedStyle)` answers a face's `x` height and
  `0` advance, `FaceUnits::or_assumed` replaces a non-finite or negative one
  with half an em, and `NoFaces` is the answer with no fonts. `resolve` is
  `resolve_measured(…, &NoFaces)`; `resolve_measured` settles each element's
  font in CSS's order — the font size against the parent's measured font
  (so `2ex` is the parent's), then the face asked, then the line height
  against it (`lh`/`rlh` in a line height are now the parent's and the
  root's). The initial face the root inherits from is measured too.
  `alo-layout`'s `text_style.rs` (`text_style_of`) is the one reading of a
  style's font, used by layout and by `alo-renderer`'s `font_units.rs`,
  whose `Faces` answers from `TextMeasurer::face` — the first font in the
  chain, so `ch` is the face the text is set in. The pipeline resolves with
  it; an SVG file's styles keep `NoFaces`, as it draws no text. Tests:
  `font_units.rs` 3, `metrics.rs` 2; `alo-layout/tests/numbers.rs`
  `ex_and_ch_are_the_measured_faces` (a fixed face: 30ch, 10ex, a child at
  `2ex` and its own lengths, and an unmeasurable face at half an em);
  `alo-renderer/tests/what_ex_and_ch_measure.rs` 5 (DejaVu's `0` and `x`;
  regular, bold and mono and a fallback family each their own `0`; `font-size:
  2ex` and `line-height: 3ex`; a hand-built face with no characters and no
  fonts at all both half an em); `alo-corpus/tests/alo_downloads.rs`
  `.lede` 483.7388, 44 × the `0` at 17.28. Two mutations were caught: the
  pipeline on `NoFaces` (3 of 5 fail) and the font size against an
  unmeasured parent (1 fails). References moved and read: `alo-downloads`
  (the lede, now three lines of up to 480.39 wide), `alo-settings` (each
  date field 178.14 wide, from the user-agent sheet's `20ch`) and
  `web-a-form` (each field 209.59). The `font` shorthand is not expanded,
  which `docs/conformance.md` now says.

- [x] **321. Text straight inside a flex or grid container takes its
  `line-height`.** *Opened by `alo-downloads` (iteration 198).* The
  download buttons are `display: inline-flex` with their label as the only
  child, so the label is an anonymous flex item measured as a text leaf
  (`NodeKind::Text`), which is as tall as its font, 18.625, rather than its
  `line-height: 1.55`, 24.8. Each button is 42.625 tall where a browser makes
  it 48.8. Iteration 196 recorded that this path ignores `line-height` and
  had not checked whether a page reached it. CSS wraps such text in an
  anonymous block container, whose lines are laid out like any other.
  *Depends on nothing. Closes when:* a layout assertion has each of
  `alo-downloads`' buttons 48.8 tall with its label's line 24.8, and a
  `numbers.rs` assertion has a flex item that is only text as tall as its
  line height.
  **Built (iteration 211):** the fix is in the box tree, where CSS puts it.
  `alo-box`'s `arrange` gives a flex or grid container's children to the new
  `wrap_text_runs`. Every element child is still an item of its own, and
  each contiguous run of text boxes (text from a `display: contents` child
  included) is wrapped in an anonymous block (`Purpose::Run`), CSS Flexbox
  § 4 and Grid § 6. A run of only whitespace is no item. The wrapper holds
  only inline content, so `alo-layout` lays it out as an inline formatting
  context, with the strut and `line-height` of the element it inherits from.
  No layout code changed. Tests: `tree.rs`'s
  `a_flex_container_wraps_nothing_…` is replaced by `…_wraps_only_its_text_…`
  (the text is wrapped, the `<b>` and `<p>` are not), plus a grid with two
  runs across a `contents` child and a whitespace gap, and a flex container
  of only whitespace. `alo-layout/tests/numbers.rs`
  `text_straight_inside_a_flex_or_grid_container_takes_its_line_height`
  (`ScaledFont`): an `inline-flex` at 16 px × 1.5 with 10 px padding is 44
  tall and its letters 14 down; a grid at a 40 px line is 40 with 20 px
  letters 10 down. With the wrap reverted, that test fails.
  `alo-corpus/tests/alo_downloads.rs`
  `each_buttons_label_is_a_line_as_tall_as_its_line_height`: each label's
  wrapper is 24.8 and each button 48.8. References moved and read:
  `alo-downloads` (both buttons 48.8, the cards 211.496, the page 786.032
  tall, the note 6.175 lower; the PNG compared before and after) and
  `alo-sign-in` (its flex SSO button's label wrapped, at the same rectangle,
  because its `line-height` is `normal` and the button a fixed 46; the PNG
  did not move).

- [ ] **278. A nested `<svg>` viewport.** *Cut from 271 (ADR 0022 § 1).* An
  `<svg>` inside an `<svg>` is a new viewport in its parent's drawing: its
  `x`, `y`, `width` and `height` (per cent of the parent's viewport, 100% by
  default), its own `viewBox` and `preserveAspectRatio` through `alo-svg`'s
  `viewport.rs`, and a clip to that viewport unless its `overflow` is
  `visible`, which needs a clip word in `alo_paint::Drawing`. Nesting counts
  toward the walk's depth bound.
  *Depends on 271. Opened only by a frozen page that needs it*, as 179 is;
  until then a nested `<svg>` is left out and recorded.

- [x] **279. Relative sizes on an outermost `<svg>`.** *Cut from 271 (ADR
  0022 §§ 1, 3).* *Retitled by iteration 178*, which cut its other half —
  the `transform` property — to **287**: the two halves share a reason
  ("presentation attributes that are geometry") and no code, and the
  property needs its own grammar decision (below). `width` and `height` on
  an outermost `<svg>` as presentation attributes, so `width="50%"` or
  `height="2em"` size the box through the cascade instead of being recorded
  and ignored by `alo-box`'s `svg.rs`.
  *Depends on 271. Closes when:* a layout assertion sizes an `<svg>` from a
  per-cent and an `em` attribute, and a stylesheet beats each.
  **Built (iteration 178).** `alo-style`'s `presentation.rs` lists `width`
  and `height`, as presentation attributes **on an `<svg>` only**: on a
  `<rect>` or an `<image>` SVG 2 makes them properties too, but nothing
  there is a box and `alo-svg` reads them as geometry, as ADR 0022 § 3
  says until a page sets one from a stylesheet. A plain number is written
  into the declaration as pixels (`width="48"` is `width: 48px`), `auto`
  and `inherit` are kept, and a value that is negative, not finite
  (`1e39px`), or not a length is ignored and recorded as an invalid
  declaration, so layout is never handed an infinity. `alo-box`'s
  `svg.rs` still reads an absolute attribute as the **natural** size, and a
  relative one is no longer recorded there: it is used, through the
  cascade, and an invalid one is recorded once, by the cascade. Tests:
  `presentation.rs` 3 new (an `<svg>`'s size is a declaration and a number
  is pixels; a `<rect>`'s is not; twelve hostile values ignored and
  recorded, `1e39px` and `1e39%` among them); `svg.rs`'s relative-size and
  hostile-size tests now say they are not natural sizes and not recorded
  there; `tree.rs`'s recorded-issues test is down to the `viewBox`.
  `numbers.rs` 3 new: `width=50% height=2em` in a 400 px block with a
  10 px font is 200 × 20; `50%` and a 4:1 `viewBox` is 200 × 50; inline,
  `3em × 2em` is 30 × 20 (the `<svg>`'s own font, not the paragraph's
  20 px); `48` and `0.25in` are 48 × 24; `* { width: 30px }` beats the
  per cent (30 × 20), `svg { height: 7px }` the `em` alone (200 × 7), and
  `#s { width: 25%; height: 3em }` both (100 × 30); `-10`, `1e39px`,
  `twelve` and `NaN` as a width leave a 300 × 20 box. Doctored: hints
  off for `width`/`height` fails the two sizing tests; the finiteness and
  sign check removed fails the hostile one in both crates. Corpus case
  **`svg-relative-size`** is the reference render: a per-cent and `em`
  box, a per cent with a `viewBox`'s shape, an inline `em` box beside
  text, one beaten on both axes and one on its height alone, and an
  invalid width at the default 300. No other case moved.
  *What it found:* a per cent on an **inline-level** `<svg>` is item 284's
  double resolution (reserved 200, drawn 100), so the per-cent tests and
  case use block-level `<svg>`s and 284 now names this case too. And a
  block-level replaced box with an `auto` width and no natural width or
  ratio fills its container (an `<img>` does the same) where CSS 2
  § 10.3.4 gives 300 — whether browsers stretch a block `<svg>` there is
  not settled here, so nothing asserts either number; written into the
  journal to be checked before it is relied on.

- [x] **287. The `transform` property on SVG elements.** *Cut from 279 on
  the iteration that built its sizing half (ADR 0022 §§ 2, 3).* The CSS
  `transform` property on an element inside an `<svg>`, with
  `transform-box` and `transform-origin` as SVG 2 defines them (an SVG
  element's initial origin is `0 0` and its box the `view-box`), and the
  `transform` attribute as its presentation attribute — so a stylesheet's
  `transform` **replaces** the attribute on that element, and the two
  compose only across ancestors. What has to be decided while building it,
  and said in the code: the attribute's grammar is SVG's (unitless
  numbers, `rotate(a x y)`, comma separators), not CSS's, so the hint is
  either turned into a CSS value the cascade can hold or marked as the
  attribute's so `alo-svg` reads it by its own grammar; and `alo-style`
  cannot call `alo-svg`'s `transform.rs`, which depends on it. Today the
  property is recorded and not applied (`walk.rs`).
  *Depends on 271. Closes when:* a reference render shows a shape turned
  by the property about its own box (`transform-box: fill-box;
  transform-origin: center`), a stylesheet's `transform` replacing an
  element's attribute, and a property on a child composed under a `<g>`'s
  attribute; and a hostile-input test of every value the hint can carry.
  **Built (iteration 179).** *Decided, and said in the code:* the hint is
  turned into a CSS value the cascade can hold. The attribute's grammar
  moved, with SVG's number scanner, from `alo-svg` to `alo-value`
  (`svg_transform.rs`, `svg_number.rs`) — the rule ADR 0022 already gives
  for path data that becomes a CSS value, and the one way `alo-style` can
  read it without depending on `alo-svg`. `presentation.rs` lists
  `transform` (on every SVG element but an `<svg>`, whose own attribute is
  left as it was) and writes it as the one `matrix()` it comes to — exact,
  because the attribute has no units or per cents and an origin applies
  about the whole list; an invalid one is ignored and recorded by the
  cascade. `alo-svg`'s new `transform.rs` reads the computed property:
  `transform-box` `view-box` (initial; at the user-space origin, the
  `viewBox`'s size) or `fill-box`/`content-box`, a shape's **object
  bounding box** from the new `bbox.rs` (curve turning points, not control
  points); `transform-origin` `0 0` unless set; `translate` per cents of
  that box; a non-finite result ignored and recorded. *It found:* two
  finite attribute functions could multiply to an infinity (`scale(1e38)
  scale(1e38)`), which the old parser passed on; the list is now refused.
  Tests: `svg_transform.rs` 1 new (the CSS text reads back as the same
  matrix) and the overflow case; `presentation.rs` 3 new (a declaration in
  CSS grammar, `inherit` kept; not on an `<svg>`; seventeen hostile values
  ignored and recorded, a million-argument `matrix` and `1e99999` among
  them, and a 100 000-function list read in one pass); `bbox.rs` 5;
  `walking.rs` 6 new (replaces, `none` too; composed under a `<g>`; origin
  `0 0`, `fill-box center`, `50% 50%` of the view box; a fill box that is
  neither handles nor stroke; per cents of each box; what is cut is
  recorded; ten hostile stylesheet values, an infinite one refused).
  Doctored: paint's loose bounds as the fill box, a `50%` default origin,
  and the hint switched off each fail their tests. Corpus case
  **`svg-transform-property`** is the reference render; no other case moved.

- [ ] **288. `transform-box: fill-box` on a container, and `stroke-box`.**
  *Cut from 287.* A `<g>`'s fill box is the union of what it holds under
  their own transforms, which needs a measuring walk of its own, bounded as
  the drawing's is; a stroke box adds each stroke as SVG 2 defines it. Until
  then each is recorded and measured against the view box (a container) or
  the fill box (a shape, whose centre an even stroke does not move).
  *Depends on 287. Opened only by a frozen page that needs it*, as 278 is.

- [ ] **108. Canvas 2D.** The rasteriser exists; this is the API over it and the
  compositing rules around it.
  *Depends on 72.*
  *Needs design (iteration 180):* its dependency is done, but it names no
  ADR, feature contract or closing condition, so `LOOP.md` step 2 says it is
  not ready to build. It does have pages: alo's own `RichTextEditor.tsx` and
  `quote-studio/quoteImageData.ts` each shrink a chosen picture through a
  canvas — `drawImage`, a white `fillRect`, then `toDataURL` as PNG or JPEG.
  Both are also unreachable before `FileReader`, `Image` and a file input
  exist (82). Its design has decisions in it rather than chores: where the
  bitmap lives and how it reaches paint, which encoder `toDataURL` rents, and
  what reading pixels back (`getImageData`, `toDataURL`) means for
  fingerprinting. Writing those down — an ADR, as its own iteration — is the
  work that opens it.

- [ ] **109. Audio and video playback through rented decoders.**
  *Depends on 63, 106. Needs ADR* — which decoders, on whose licence, and where
  they run. `ROADMAP.md` refuses DRM and proprietary codecs outright.

  **Decided: ADR 0023 (iteration 180).** Media is demuxed and decoded in a
  **media process per site**, started by the browser process, confined by
  the renderer's own profile and holding nothing of the page but the bytes it
  was handed (§ 1); bytes are fetched by the browser process and forwarded
  unread, audio goes to the browser process's device and frames to the
  renderer (§ 2). Audio is **Symphonia**, default features off, `ogg`, `wav`,
  `mkv`, `vorbis`, `flac`, `pcm` and `mp3` on, bounded at 8 channels and
  8 000–384 000 Hz (§ 3). **Opus waits** for Symphonia's own decoder or a
  pure-Rust one with a year of releases (§ 4); **AV1 waits** on ADR 0021's
  `rav1d` release; VP8 and VP9 have no Rust decoder (§ 5); AAC, H.264, H.265
  and DRM are never played, and every "can you play this" answer is derived
  from what decodes (§ 6). No code was written. Its code is cut as 289–295,
  and 109 closes when all seven have; 293–295 are blocked on decoders that
  do not exist yet, so a line that plays only Vorbis is not ticked.
  No frozen page plays a file yet — alo's `<video>` (`ScanInput.tsx`) shows a
  camera stream, which is 93's, and its Meet room is WebRTC — so each code
  item is opened by a frozen page, as 274 is.

- [ ] **289. Audio decoded, in `alo-media`.** *Cut from 109 (ADR 0023 § 3).*
  A new crate, `alo-media`, with Symphonia named in one file and added to
  `gate.sh`'s boundary list: Ogg Vorbis, FLAC, WAV PCM and MP3 (and those
  in WebM) decoded a packet at a time into bounded interleaved samples;
  AAC, Opus and every other codec refused by name; a track over 8 channels
  or outside 8 000–384 000 Hz refused; a declared duration never allocated.
  *Depends on nothing. Opened only by a frozen page that plays one of those
  files. Closes when:* each frozen file decodes to its stated frame count,
  rate and channels and a pinned checksum of its samples, and every byte
  flipped and every prefix cut returns an error rather than panicking.

- [ ] **290. The media process.** *Cut from 109 (ADR 0023 §§ 1–2).* One per
  site, spawned by the browser process on its renderer's first media
  request, confined at `exec` by `sandbox.rs`'s profile and fatal if it
  cannot be, reaped with the site's renderer; bytes in, samples out, every
  message it sends checked as a stranger's. *Depends on 289, 166, 167.
  Closes when:* a test watches its sandbox refuse 167's probes, killing it
  leaves the renderer answering and the element reporting a decode error,
  and closing the site's last tab stops it.

- [ ] **291. `<audio>` and `HTMLMediaElement`, first members.** *Cut from 109.*
  The element, its claim for a resource (ADR 0020's shape), the clock it is
  told (ADR 0023 § 8), `canPlayType` and `<source type>` derived from what
  289 decodes (§ 6), and the media error a refused file gives. The members
  and their order are cut when a page opens it, and an autoplay policy is
  decided before anything plays unasked. *Depends on 290, 75 (`play()`
  returns a promise), 233.*

- [ ] **292. Sound reaches the device.** *Cut from 109 (ADR 0023 § 7).* The
  browser process hands decoded samples to the machine's audio device
  through a rented crate chosen by § 3's tests, with no `unsafe` of ours.
  Verified on files up to the buffer handed over; **that a speaker made the
  sound needs hardware, and says so**. *Depends on 290.*

- [ ] **293. Opus.** *Cut from 109 (ADR 0023 § 4).* ***blocked:** no Opus
  decoder to rent* — lifted by Symphonia releasing its own, or by a
  pure-Rust decoder with a year of releases, the RFC 8251 vectors passing,
  users beyond its authors, and `unsafe` forbidden or behind a feature left
  off. *Depends on 289.*

- [ ] **294. AV1 video.** *Cut from 109 (ADR 0023 § 5).* `rav1d` on ADR 0021's
  terms, in the media process, frames to the renderer drawn in a `<video>`'s
  box; the container decided by the building commit against the ADR's
  tests. ***blocked:** the `rav1d` release 269 waits on.* *Depends on 290,
  291.*

- [ ] **295. VP8 and VP9.** *Cut from 109 (ADR 0023 § 5).* ***blocked:** no
  VP8 or VP9 decoder exists in Rust to rent*; libvpx is C. Lifted on § 4's
  terms by one that does. *Depends on 294.*

- [ ] **110. Media Source Extensions**, without which most video sites do not
  play at all.
  *Depends on 109.*

- [ ] **111. Web Audio.**
  *Depends on 72.*
  *Needs design (iteration 181):* its dependency is done, but it names no
  ADR, feature contract or closing condition, so `LOOP.md` step 2 says it is
  not ready to build. It also consumes ADR 0023's decoded samples and device
  (289, 292), so a graph that plays anything waits on those; one that only
  computes (an `OfflineAudioContext`) does not, and is the likelier first
  cut. No frozen page uses it.

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
  ***blocked:** no measurement yet (iteration 181).* ADR 0017 § 6 decided a
  changed document is rendered again **whole**, rejected incremental style
  and layout *"now"* as *"a cache built before the behaviour it caches is
  settled"*, and wrote that this item *"waits for a measurement"*: a page
  janking on every change, measured on hardware. None has been taken, and
  replacing § 6 without one would be re-deciding an accepted ADR inside a
  code commit. Lifted by that measurement — most likely once a person uses
  the window (118) — and then it needs its own ADR on what is invalidated
  and how a test proves the incremental answer equals the whole one.

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
  *Needs design (iteration 181):* its dependency is done, but it names no
  pages, no budget and no closing condition, and this repository has no CI
  to measure in. Which pages, on what machine, measured how and failing at
  what number are decisions, written down before a number is.

## J. The browser itself

What stage 2's exit gate actually measures: a person using it.

- [ ] **118. A window, tabs, and a tab strip.** *Depends on 63, 64.*
  **Decided: ADR 0024 (iteration 181).** The window is rented — `winit`
  0.30 in `alo-window`'s `window.rs`, `softbuffer` 0.4 in its `present.rs`,
  both through safe interfaces, no `unsafe` of ours (§ 1); the event loop
  never calls a renderer, a *conductor* thread owns `Tabs`, and the window is
  **composed** by a function with a reference render from the frames it was
  last sent (§ 2); device pixels are the renderer's, and until 299 the
  window replicates pixels to the integer scale factor (§ 3); the **tab strip
  is a document we ship**, built from the browser's state as data — a title
  is a text node, never markup — and rendered in a sandboxed renderer of its
  own, so the agent reads tabs as it reads a page (§ 4); a person's pointer
  is the only coordinate that reaches a renderer, hit-tested there, and what
  a click on the strip means is claimed by its renderer and decided by the
  browser process (§ 5); a person opens a tab, and a page only by
  `target="_blank"` on a link a person activated (§ 6). No code was written.
  Cut as 296–300; 118 closes when all five have.
  *Opened by* stage 2's exit gate — *"a person uses it as their browser for
  a week"* — which nobody can begin without a window; the frozen alo pages
  in the corpus are what it shows first.

- [ ] **296. The window shows a tab.** *Cut from 118 (ADR 0024 §§ 1–3).* A new
  crate, `alo-window`, binary `alo`: `winit` named only in `window.rs`,
  `softbuffer` only in `present.rs`, both added to `gate.sh`'s boundary list;
  the safety of every function called checked in the crates' source first
  (§ 1's stop rule); the conductor thread owning `Tabs`, talking to the event
  loop only through messages and `winit`'s proxy; composition as a function
  of the window's size, the selected tab's frame or its sentence, and the
  background; a window resize sent as a `Resize`; integer pixel replication
  at the scale factor; closing the window closes every tab. *Depends on 63,
  64. Closes when:* the composition's reference renders (a frozen alo page;
  a resize before its new frame; a gone tab's sentence) are committed; a
  test shows a renderer that never answers leaves the composition answering
  with its last frame; and `alo` started on this macOS machine shows a
  frozen page, captured with `screencapture -l` and compared to the
  composed reference, recorded in the journal.

  **Built (iteration 182), and not closed.** `alo-window` exists. `winit`
  0.30.13 is named only in `window.rs`, and `softbuffer` 0.4.8 and
  `raw-window-handle` 0.6 only in `present.rs`. All three are on `gate.sh`'s
  boundary list. Every function called on them was checked safe in their
  source first. The conductor (`conductor.rs`) owns `Tabs` and speaks to the
  event loop only in `message.rs`'s `Order` and `News`. Composition
  (`compose.rs`, placed by `place.rs`, the sentence drawn by `notice.rs`) has
  four committed reference renders in `tests/references`:
  - the frozen `alo-offline` page at scale two;
  - a resize before its new frame;
  - a gone tab's sentence over its frame;
  - a tab gone before it painted.

  `tests/a_renderer_that_never_answers.rs` uses the real `alo-render` and
  stops it with `kill -STOP`. The window keeps composing its last frame at
  once, pixel for pixel; the sentence arrives at the bound; the renderer is
  stopped. `tests/closing_the_window.rs` shows closing, or the window going
  without a word, leaves no renderer running. `alo --frozen-fonts
  …/alo-offline/page.html` was started on this machine and showed a window
  (id 11971, 1000×732 points) with one confined `alo-render` under it. On
  `kill -TERM` both were gone.

  ***The capture, and nothing else, is owed.*** It was blocked: `screencapture`
  answered "could not create image from window", and then "could not create
  image from display" for a whole screen too, because Visual Studio Code — the
  application the loop runs under — had been **refused** Screen Recording. A
  refusal is remembered, so nothing re-asked; `tccutil reset ScreenCapture
  com.microsoft.VSCode` cleared the decision and the owner granted it in
  System Settings on 2026-10-10. Verified the same minute: a whole-display capture
  returned a 2.9 MB file and exit 0 where it had failed, with no restart of the
  application needed. No API reads another process's window without this, by
  design. The remaining work is:
  1. Start `alo --frozen-fonts crates/alo-corpus/cases/alo-offline/page.html`.
  2. Capture its window.
  3. Compare the content area with `compose` of the same page at the
     window's size and scale.
  4. Record the result here.

  Nothing else of 296 is owed.

  **Tried again (iteration 236), and the screen was locked.** `alo` started
  and showed its window (id 410), with one `alo-render` under it.
  `CGPreflightScreenCaptureAccess` answered true, so the permission stands.
  But the session reported `CGSSessionScreenIsLocked` = 1. A whole-display
  capture returned a black frame, and `screencapture -l 410` answered "could
  not create image from window". Both processes were gone after `kill -TERM`.
  A locked screen shows nothing, and only a person can unlock it. So this
  is taken again when the machine is unlocked, not worked around.

- [ ] **297. The tab strip.** *Cut from 118 (ADR 0024 §§ 4, 6).* Its own
  renderer at an internal site no page can name, under the renderer's
  sandbox profile; a typed message of the strip's state (tabs in order,
  each title, address, selected, loading or gone); a document built by
  `alo-dom`'s operations from a compiled-in template, each title a text node;
  a stylesheet after alo's design; `Tabs` gaining an order and a selected
  tab; the browser's own keys (a new tab, close this one, the next and
  previous) read by the browser process before any page. *Depends on 296.
  Closes when:* reference renders of one, four and a gone tab's strip; a
  layout assertion of each tab's box; the agent tree reads *"four tabs, the
  second selected"* with each title; and hostile titles — `<b>`, `</style>`,
  a NUL, 100 000 characters, a right-to-left override — each stay text in
  the tree and in the render, a long one truncated by the stylesheet, none
  panicking.

- [ ] **298. A person's pointer.** *Cut from 118 (ADR 0024 § 5; ADR 0018
  § 3).* The browser process decides which frame a point falls in from its
  own composition and sends it in that frame's CSS pixels as a person's
  pointer event; the renderer hit-tests against its layout tree and
  dispatches through ADR 0018's browser driver; the strip's renderer answers
  a click with a claim (select, close, open a tab) the browser process acts
  on only for a tab it holds, answering a pointer event it sent. *Depends on
  296, 297. Closes when:* a person's click on a link in a frozen page follows
  it, the same as the agent's `Activate` on it; a click on a strip tab
  selects it; a claim about a tab not held, or with no pointer event behind
  it, is refused; hit-testing has layout assertions at box edges, inside a
  transform and under an overflow clip; and no message an agent can cause
  carries a point.

- [ ] **299. Device pixels.** *Cut from 118 (ADR 0024 § 3).* The renderer lays
  out in CSS pixels and paints at the window's scale factor; `Resize` carries
  it; `devicePixelRatio` and the `resolution` media feature are read from it;
  the window stops replicating pixels. *Depends on 296. Opened by:* this
  machine's window at scale 2. *Closes when:* reference renders of the same
  frozen page at scale 1 and 2, with layout identical in CSS pixels, and
  every existing reference render unchanged at scale 1.

- [ ] **300. A link opens a tab.** *Cut from 118 (ADR 0024 § 6; ADR 0020
  § 2).* `target="_blank"` on a link a person activated becomes a decided
  ask that opens a tab beside its opener, under ADR 0020 § 3's rules for the
  address; a script's `window.open` and a named target stay refused by name.
  *Depends on 297, 298. Opened by* a frozen page with such a link. *Closes
  when:* that page's link opens a tab with the address decided by the
  browser process, and a `_blank` link activated by a script with no person
  behind it is refused. Whether an agent's `Activate` (ADR 0018's keyboard
  click) counts as a person's is decided with item 133's grants, not here.
- [ ] **119. The address bar**: what somebody typed, what it means, and a search
  that **phones nobody by default**. *Depends on 50, 118.*
- [ ] **120. History, bookmarks, downloads.** *Depends on 118.*
  ADR 0028 § 3 puts one thing in the downloads interface: a saved PDF (or
  any saved file) opened in the program the operating system uses for it,
  only when a person asks for that file. Never automatically, never from a
  page, never from an agent verb. § 2 leaves to this item whether an
  offered file's body is held in memory or not fetched until the person
  says where.
- [ ] **121. Find in page, zoom, and per-site settings that stick.**
  *Needs design (iteration 197):* it names no ADR, feature contract or
  closing condition, so `LOOP.md` step 2 says it is not ready to build. Each
  of the three is browser interface over a window that does not yet take a
  person's input (296, 298), and *settings that stick* is a file of the
  person's that ADR 0011 has not been asked about.
- [ ] **122. Context menus, and keyboard operation of every one of them.**
  *Needs design (iteration 197):* as 121 — no ADR, contract or closing
  condition, and a menu is browser interface over a window that takes no
  pointer yet (298).
- [ ] **123. Printing, print preview, export to PDF.** *Depends on 99.*
- [ ] **124. Viewing a PDF** — or saying plainly that we hand it to something
  else. *Needs ADR* — it is a decision, not an omission.
  **Decided: ADR 0028 (iteration 197).** Stage 2 has **no PDF viewer**, and
  *PDF viewer supported* is false. A response whose `Content-Type` essence
  is `application/pdf` or `text/pdf` — nothing sniffed — reaches no renderer
  and no parser of ours: the browser process offers it to the person as a
  file, says in its own words that it does not display PDFs, keeps the tab's
  page, and writes nothing until the person chooses where (§§ 1–2). Opening
  the saved file in another program is a person's act in the downloads
  interface, never automatic, never a page's ask and never an agent verb
  (§ 3). A frame shows the browser's sentence and offers nothing,
  `<object>` shows its fallback, `<embed>` shows nothing (§ 4);
  `navigator.pdfViewerEnabled` is false and `plugins` and `mimeTypes` are
  empty (§ 5). The agent reads the offered file as browser state and cannot
  read inside it (§ 6). A viewer is reopened only by a person's stage 2
  week naming PDFs, and then rented in Rust, in a process of its own, with
  no PDF JavaScript and its text read into a tree the agent reads (§ 7).
  The measure is alo, and alo serves every PDF as an attachment on purpose.
  No code was written. Cut into 317 and 318; 124 closes when both have.
- [ ] **317. A navigation to a PDF is offered as a file.** *Cut from 124
  (ADR 0028 §§ 1–2). Depends on 85, for the browser process holding a
  navigation's response, and on 120, for where a file goes. Opened by* a
  frozen page whose link reaches a PDF served inline; alo's own PDFs are
  attachments and reach the same path through 264. *Closes when:* a
  response typed `application/pdf` or `text/pdf`, in any case and with any
  parameters, reaches no renderer (a test recording every message a
  renderer is sent shows none carrying the bytes); the tab's page and its
  held ids are unchanged; the browser's sentence and the ADR 0012 record
  line are produced; nothing is written to a disk before the person's
  choice and nothing after a refusal; and the same bytes typed `text/html`
  load as a page, so the type decides. Hostile names, types and lengths are
  refused, never a panic.
- [ ] **318. `navigator.pdfViewerEnabled`, `navigator.plugins` and
  `navigator.mimeTypes`.** *Cut from 124 (ADR 0028 § 5). Depends on 325,
  which builds the `Navigator` interface (ADR 0030). Opened by* a page that
  reads one of them. *Closes when:* `pdfViewerEnabled` is `false` and both
  lists are empty, in a frozen page's script.
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
  Among them Annex B's `var` of the same name as a `catch` parameter
  (`catch (e) { var e; }`), which item 210 refuses as not a program.
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
