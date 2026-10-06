# alo browser — features.md

Feature inventory. Four tiers, matching the stages in `ROADMAP.md`:
**[1]** = renders alo · **[2]** = renders the modern web · **[3]** = the legacy
tail · **[4]** = a browser somebody chooses. **★** marks the things no other
engine offers.

**[4] was added when the roadmap gained a fourth stage.** The product work —
extensions, sync, a mobile port — used to sit under "not scheduled", which had
begun doing the work of "not thought about". It is gated behind stage 2's exit
gate exactly as before; it is now *named*.

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
- [1] **`clamp()`, `min()` and `max()`**, one family with `calc()` and nesting in each other, type-checked once at parse time
- [1] **Viewport units** — `vw`, `vh`, `vmin`, `vmax` — which need a window, and answer zero rather than a plausible number when there is none
- [1] Colours as channels — hex, `rgb()`, `hsl()`, the named colours. Blocks paint rather than layout
- [1] An unknown property is kept and ignored rather than dropped, so a later stage can implement it without re-parsing
- [1] Media queries for width, and `prefers-color-scheme` — the light and dark the workspace already ships
- [2] Animations and transitions
- [2] Container queries, `:has()`, cascade layers, `@property`
- [2] Filters, `backdrop-filter`, blend modes, masks and `clip-path`
- [2] Paged media and print styles
- [3] Vendor prefixes, and anything that exists only for a page written before 2015

## Layout

- [1] The box model — content, padding, border, margin, and the box's *meaning* alongside its rectangle (ADR 0002)
- [1] **A form control holds what it shows in a box of its own** — a tall button's label in the middle of it, an empty field still one line tall. A box in the tree rather than a rule in the user-agent sheet, because a rule would also catch a control an author had made a flex container
- [1] Box generation: `display: none` removes a subtree, `display: contents` removes a box and keeps its children, and a container whose children are a mix of block and inline grows the anonymous boxes that make them one kind
- [1] A user-agent style sheet — what an element looks like before anybody says otherwise. The modern elements only; no defaults for what we do not lay out
- [1] **A block-level box inside an inline one, broken around it the way the specification says** — a piece on each side, the block a sibling of the anonymous blocks they sit in, so a background stops and starts again rather than running straight through
- [1] **An inline box's own border and padding**: horizontal ones take room on the line, vertical ones draw without changing its height, and a box that wraps is one rectangle per line with its start border on the first piece and its end border on the last
- [1] An *empty* piece of such a break keeps its border, and costs no line when it has none — CSS's zero-height line box
- [1] **Flexbox and grid**, on `taffy` behind our own boundary. One file may name it
- [1] Absolute and relative positioning, `z-index`, stacking
- [1] Overflow and scrolling regions
- [1] Inline formatting: a line of text and the boxes in it, with breaking and baselines
- [1] **`letter-spacing`**, applied where the text is measured — it changes what a run is worth and so where every line breaks
- [1] **`white-space`**: `normal`, `pre`, `pre-wrap`, `pre-line`, `nowrap` — runs of whitespace collapsed when the box is built, and where a line may break decided when the line is built
- [1] Layout is asserted in **numbers** — the computed box — never by eyeballing an image
- [2] Writing modes, and layout that is right-to-left rather than mirrored afterwards
- [2] Multi-column, `position: sticky`, scroll snap and overscroll behaviour
- [3] **Floats as layout, CSS-table layout, quirks mode.** Deliberately last, and possibly never: refusing these is what makes the scope survivable

## Text

- [1] Shaping with HarfBuzz — `rustybuzz`, the Rust port, so there is no C in the process — and font rasterisation. Rented, as every engine rents them
- [1] The fallback chain: a font is *asked* whether it has the character, never guessed at from a language tag
- [1] **The awkward scripts before the easy ones.** A pipeline that assumed left-to-right and one glyph per character is a pipeline that gets rewritten
- [1] Line breaking, and the fallback chain when a font lacks a glyph
- [1] Web fonts
- [2] **A variable font is one file and many weights**: the `wght` axis read as the range it covers rather than the one instance `OS/2` names, so one file answers a request for 400 and one for 700 — and is shaped, measured and outlined at the weight it was set to
- [2] Web fonts as pages actually ship them: WOFF2, the rest of a variable font's axes, and loading that does not flash
- [2] **Input methods.** A browser that cannot take Japanese or Chinese input is not a browser in those countries
- [2] `contenteditable`, which every rich text box on the web is built on
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
- [2] **Two-toned borders**: `inset`, `outset`, `groove` and `ridge`, in a darker and a lighter tone of the border's colour, each side mitred so that a corner splits where the tones change, and a groove's halves following a rounded corner (queue item 190; a fieldset's legend-broken groove is 267)
- [2] **Patterned borders**: `dashed`, `dotted` and `double` — dashes and round dots spaced so a side starts and ends on one, a corner shared without a seam, `double` as two lines a third of the width each following a rounded corner, and a bound on how many pieces a page can ask for (queue item 266; beside a fieldset's legend is 268)
- [1] **Transforms and opacity**: `translate`, `scale`, `rotate`, `skew`, `matrix`, about a `transform-origin`; `opacity` as a group drawn once and composited once, never box by box
- [1] **Reference renders**: a committed corpus, each with its expected image *and* its expected box tree
- [1] Hardware acceleration — after the software path is correct, never before
- [2] Compositing layers, and scrolling that does not repaint the world

## ★ The agent surface (ADR 0002)

The reason this exists rather than a faster fork of somebody else's engine.

- [1] ★ **The layout tree read as roles, states, positions and text** — a *view* of the tree that draws the page, never a parallel structure. Two structures eventually disagree, and the agent acts on whichever is wrong
- [1] ★ **`aria-current`**, as the word the author used rather than a flag — a nav item being the current *page* is not the claim a cell being the current *date* makes
- [1] ★ Roles are **declared, not inferred**. A box says it is a list, a row, a field, a button — guessing that from appearance is what screen-scraping already does badly
- [1] ★ **Typed verbs**: activate, put text, scroll. **No verb takes a coordinate**, because a coordinate is a guess about a layout that may have moved between the reading and the acting
- [1] ★ A verb **changes the page**: text into a field, a checkbox ticked, a radio chosen and its group un-chosen — rendered again from the same document, so every id an agent is holding still names what it named
- [1] A field shows what it holds; a password shows one dot a character, and the dots are not in the agent tree
- [2] A form control draws its state — a tick in a checked box, a dot in a chosen radio, a dash in one that is neither, in `accent-color` if the page names one
- [2] A control nobody can operate says so, **and still says what state it is in** — "you cannot change this" and "this is off" are different things to be told
- [2] A focused control draws a focus ring
- [1] ★ **One element, one thing to read** — an inline box broken around a block is read as one node, named by everything the element contains and positioned everywhere it was drawn, with the block inside it rather than beside it
- [1] ★ Reading is never watching — the tree is exposed when asked, and `alo-os`'s capability model decides who may ask
- [1] ★ **The same tree is the accessibility tree.** A screen reader and an agent want identical facts, and two implementations would guarantee one is wrong — so EN 301 549 conformance and agent capability are one piece of work, not two competing budgets
- [2] ★ The same tree over ordinary web pages, which no browser can offer today because none of them owns both halves
- [2] Assistive technology bridges — AT-SPI on Linux — over that same tree

## Embedding

- [1] alo's sign-in screen, then Settings, rendering correctly from their own markup and `tokens.css`, matched against a committed reference render **and** an expected box tree — the exit gate of stage 1, and reachable on any laptop
- [1] A rendering surface with no operating system behind it: HTML and CSS in, a PNG and a box tree out, both files. Everything in stage 1 is verified through it, which is what makes the stage buildable by anybody who clones this
- [2] A surface alo OS's shell can render into — **retiered from [1]**: it is the one embedding item needing a compositor that does not exist, and tiering it [1] is what put an unreachable dependency inside stage 1
- [2] Several documents at once, the shape tabs need

## The network — stage 2

- [2] **URLs**: WHATWG parsing, resolution against a base, IDNA and punycode — rented, because whether two spellings are one host is a security question with a Unicode table for an answer
- [2] **The shape of a load**: a request that says who asked and what for, a response of bytes, headers that keep their order and their repeats, and a media type — with `data:` and `file:`, so that HTTP is one more arm rather than a second pipeline
- [2] **Which encoding a page is in**: byte order mark, `Content-Type`, `<meta>`, then UTF-8 — the tables rented, the algorithm ours, and a page that decoded badly saying so
- [2] **HTTP/1.1**, ours: a request out, a response in, and body framing that refuses every message saying two things about where it ends — which is what request smuggling is
- [2] A truncated body is an **error**, not a short page
- [2] **Content encodings**: gzip, brotli, zstd, and `deflate` in both the
  spelling the specification asks for and the one servers actually send
- [2] **`Transfer-Encoding` as the list it is** — `gzip, chunked` is chunks
  holding a gzip stream, and the chunks come off first. A coding after
  `chunked`, one we cannot undo, or a compressed body the connection closing is
  the only end of, is refused by name
- [2] **A page's own style sheets** — `<style>` and `<link>` together, in
  document order, with an alternate sheet left alone and a missing one recorded
- [2] **`<img>` lays out at the picture's own size** and keeps its ratio when
  given one dimension
- [2] **PNG and JPEG from a page**, with the format taken from the bytes rather
  than the name, and the same bounds either way
- [2] **PNG pictures from a page**, read tolerantly and bounded before anything
  is allocated — a hundred-byte file cannot ask for seventeen gigabytes
- [2] **A wrapped inline is more than one rectangle** — and offscreen only when
  every piece of it is
- [2] **`text-decoration`**: underline, overline and line-through, stopping at
  the end of the inline rather than the edge of the line
- [2] **Forms**: labels that wrap their control, fieldsets named by their
  legend, and a radio you can tell from a checkbox
- [2] **A fieldset's border, broken by its legend** — the legend sits *in* the
  block-start border rather than above it, and the border is drawn in the two
  pieces it leaves. The band it sits in replaces that border rather than adding
  to it, so the fieldset is as tall as a browser draws one
- [2] **`border-radius` in per cent**, resolved against the box
- [2] **Links that look like links**, with no visited colour — `:visited` never
  matches, so history is not readable from a page
- [2] **The user-agent sheet's typographic defaults** — what a page looks like
  before anybody styles it
- [2] **Shorthands compete with longhands in the cascade**, so an author's
  `padding: 0` beats a user agent's `padding-left`
- [2] **Fonts handed across the boundary** — the browser process opens the
  files, the renderer opens nothing
- [2] **A font a page asked for by name, fetched on demand** — a renderer says
  which families it wanted and did not have, the browser process looks for each
  on the machine by the name the **font** gives itself rather than the one on
  its file, and sends over what it finds. A family that genuinely is not on the
  machine comes back named, and the page says in words what it was drawn in
  instead — never a stable render nobody can explain. A name longer than any
  font's family can be (512 characters, the longest `name` record this engine
  reads) is not asked for, and the page says so, quoting it by its beginning
  and its length (`alo-renderer`, queue item 244): it could only ever come
  back *not here*, and carried whole it let a page near the size limit make
  its own load report too large to send. Asking for it cut short is not done,
  because that would ask for a different font
- [2] **A font's own name, read in the encodings older than Unicode** — the
  Macintosh `name` records several of the fonts macOS ships carry *instead* of a
  Unicode one, in Mac OS Roman and Mac OS Cyrillic. The other Macintosh
  encodings are read by nobody here rather than guessed at as their near
  relatives, and a Unicode name wins wherever a font has one. No family anywhere
  comes from a filename
- [2] **A font's own name, read in the language a page would ask for it in** —
  a `name` table holds the same family once per language, and macOS's system
  font holds it thirty-five times. The unlocalised record wins where a font has
  one, because that is the name the font calls itself and the platform reads it
  the same way; English wins over every translation of it; and a font that
  states no English name at all is still filed under its own, since a font in
  one language is a font somebody has
- [2] **A face's weight and slant, from the font** — the `OS/2` table states
  both, so `Helvetica-Oblique` leans and `InterDisplay-SemiBold` is 600 rather
  than everything being one of the two words a filename might carry. A weight of
  zero is a font that did not say rather than the lightest face there is, and a
  font with no such table at all is still a face: normal, upright, and drawn
- [2] **What `sans-serif` means on this machine** — `serif`, `sans-serif`,
  `monospace` and `system-ui` are decided by the browser process from the
  families it actually found, and handed to a renderer with the fonts, because a
  confined renderer may not look at the machine. Each keeps every candidate that
  is here, in preference order; `cursive` and `fantasy` are answered by nobody
  rather than guessed at; and a machine that has none of them still says so,
  rather than a page being drawn in whatever font sorted first
- [2] **Renderers confined by the platform's own sandbox** (macOS) — no file
  read, no write, no socket, each watched failing rather than assumed
- [2] **One renderer process per site** — two sites are two processes, and
  killing one leaves the other running
- [2] **A renderer nothing wants any more is stopped** — closing the last tab
  on a site ends that site's process, and closing one of two tabs on a site
  ends nothing, so the memory one process per site costs is paid for pages
  somebody still has open rather than held until a ceiling evicts them
- [2] **A tab that keeps the last frame it painted when its renderer dies, and
  says what happened** — every tab in the dead process and no other tab
  anywhere, the picture still on the screen rather than a blank rectangle, and
  a fresh process only when a person asks for the page again
- [2] **A renderer that stops answering without dying is given up on** — an
  exchange has a bound, so a renderer that is alive and silent costs its own tab
  rather than freezing the browser and every other tab with it, and one that is
  merely slow is still waited for
- [2] **A wire format for the renderer boundary**, where a message from a
  renderer is untrusted because a renderer is the process that parsed the page
- [2] **HSTS**, ignored over plain HTTP so it cannot be used as a weapon
- [2] **Mixed-content blocking** — a script refused outright, an image tried
  over TLS first, and `http://localhost` treated as secure because it is
- [2] **Referrer policy**, defaulting to origin-only across sites and nothing at
  all across a downgrade. The origin goes out written as a URL,
  `https://example.com/`, as every other engine sends it
- [2] **The same-origin policy, CORS and preflight** — a page may send almost
  anywhere and may read almost nowhere, and a wildcard never covers a request
  that carried credentials. A request is unsafe by the *value* of its
  `Content-Type` as well as by the name, so a JSON post is asked about with the
  header it is unsafe by named in the question
- [2] **Content Security Policy** — `default-src`, `script-src`, `style-src`,
  `img-src` and `connect-src` enforced, with `'self'`, `'none'`, schemes, hosts
  with wildcards, ports and paths, nonces and `'strict-dynamic'`. A source
  expression this engine cannot read is kept and matches nothing, and the
  directive holding it is kept whole — a policy is never widened by our not
  understanding a word of it. A repeated directive keeps the first, so appending
  to the header cannot widen a policy, and two policies are an intersection
- [2] **A policy a site is only watching blocks nothing** —
  `Content-Security-Policy-Report-Only` is read as what it is, and what it
  objected to is reported rather than acted on
- [2] **A page's author is told about inline script their policy refused** —
  or, under a policy they are only watching, would have refused: the script
  runs, and a report naming `script-src` and `inline` is posted to wherever
  the page's own headers said. Only the part of the browser that holds the
  network writes and sends a report, from the headers it fetched itself, so a
  page that has taken over the process drawing it cannot point a report
  anywhere else or make it say anything else, and cannot make the browser send
  more than 64 for one load. A policy written into the page's markup with
  `<meta>` is obeyed but not yet reported (queue item 240)
- [2] **Inline content allowed by its hash** — `'sha256-…'`, `'sha384-…'` and
  `'sha512-…'` computed over the content and compared with what the policy
  named, in either base64 alphabet. A digest that mixes the two alphabets, sets
  bits that stand for no byte, or is the wrong length for the algorithm it names
  allows nothing: a hash source is a permission, and every laxness in reading one
  is a policy wider than its author wrote
- [2] **A `style` attribute allowed by its digest, and only where the page said
  `'unsafe-hashes'`** — content with no element of its own is the shape an
  injection most often takes, so a hash written for a `<style>` element never
  reaches one on its own. The keyword has to be in the directive that decides,
  it allows nothing by itself, and a refusal that names it says *no digest
  applies here*, which is a different thing to be told than *your digest does
  not match*
- [2] **A directive this engine does not act on is named** rather than assumed
  to be protecting the page
- [2] **A violation is reported to whoever the policy named** — `report-uri`
  and `report-to`, in the two documents collectors read, from an enforced
  policy and a watched one alike. A group name is resolved against the page's
  own `Reporting-Endpoints` header, and one nobody defined is said in words
  rather than dropped
- [2] **A report says a cross-origin URL as its origin and nothing more.** A
  report is posted to a server the page chose, so a full URL in one would be a
  way for a page to read a URL it was refused — a capability token in
  somebody else's query, where a redirect ended. A `data:` URL is reported as
  its scheme alone, and the page's own URLs lose their fragment and any
  credentials written into them
- [2] **A report that cannot be sent is not a load that fails.** A collector
  that is down, that answers an error, or that a public page pointed at this
  machine is a delivery that did not happen and is named as one — silence
  would make a policy reporting nowhere look exactly like a policy nothing
  violated
- [2] **The preflight cache** — a second request of the same shape sends no
  `OPTIONS`, and one of a different shape still does. Partitioned by top-level
  site, keyed by the asking origin as well, expiring on the caller's clock and
  capped at two hours however long a server asked for. A `*` is remembered as
  the method and headers it allowed rather than as a standing permission
- [2] **A request that sends something**, over both protocols — a body written
  after the blank line in HTTP/1.1 and in `DATA` frames in HTTP/2, cut to the
  frame size the server said it would read, with a window that closes part way
  through **waited on** rather than overrun, and the stream closed rather than
  left open when a server answers before it has finished reading. The length a
  request states is always the length of its bytes, never a header a caller
  wrote
- [2] **An interim response is not the answer** — `103 Early Hints` and its
  kind are read past on both protocols, whether or not anything asked for one,
  and a server that only ever says something first is refused. An `Expect` is
  refused by name, because an expectation is a promise to wait and nothing here
  can bound the waiting
- [2] **HTTP/2, negotiated by ALPN and spoken** — the protocol chosen during the
  handshake, so no request is ever sent twice to find out which one it is
- [2] **HTTP/2 streams and flow control**, with the CONTINUATION flood refused
  by a bound on the whole header block rather than on each frame
- [2] **HPACK**, against the specification's own worked examples — with the
  Huffman codes derived from the canonical structure rather than transcribed
- [2] **HTTP/2 framing**, with every length checked before anything is reserved
  and the padding underflow refused by name
- [2] **Names resolved by the machine's own resolver** (ADR 0008), with DNS
  rebinding refused — a page on the public web cannot be made to reach a private
  address
- [2] **Cookies, partitioned by default** (ADR 0007) — keyed by the setter *and*
  the top-level site, `SameSite=Lax` when a site says nothing, and the
  `__Host-`/`__Secure-` prefixes enforced rather than parsed
- [2] **The site is the registrable domain**, decided against the public suffix
  list — so two subdomains of one organisation are one site and share what a
  site is given, and two organisations under one suffix are two sites however
  alike their names look. One answer, used by the cookie jar, the cache and the
  process split
- [2] **The list is a snapshot, and its age is a message rather than a silence**
  — the day it was taken is recorded beside the version it was taken at, checked
  against what is actually compiled in, and the build fails once it is six
  months old. A stale list is two organisations sharing one site, which is the
  direction that costs
- [2] **A cookie for a whole public suffix is refused** — `Domain=co.uk` is a
  cookie for every school and council in the country, and nothing about the
  shape of the string says so
- [2] **An HTTP cache**: freshness, `Age`, revalidation with `ETag` and
  `Last-Modified`, and `Vary` as a contract so one reader never gets another
  reader's page
- [2] **A cache that survives a restart, partitioned by top-level site** exactly
  as cookies are (ADR 0011) — so no site can time a load to learn where else you
  have been, and an entry only you were ever given cannot follow you between
  sites
- [2] **What must not outlive the session is never written**, rather than
  written and deleted: a `private` response, a request that carried
  `Authorization`, a response carrying `Set-Cookie`, anything that did not come
  over HTTP, and a body that is not the length it was said to be. All of it
  still reusable from memory, and a session that is not meant to persist has no
  cache directory at all
- [2] **A cache file is untrusted input** — a version discarded rather than
  guessed at, a checksum over the whole entry, every length checked, and
  anything that does not read treated as a miss rather than as a failure to
  load. The directory and its files are private to their owner, and clearing it
  really removes them
- [2] **Redirects**, bounded and loop-detecting, with `Authorization` dropped
  at an origin boundary and `file:`/`data:` refused as destinations
- [2] **A download that stops asks for the rest of itself**, over HTTP/1.1 and
  over HTTP/2 — with a `206` required to begin exactly where it stopped, a
  `200` answering a range request never appended, and nothing encoded ever
  spliced
- [2] **A connection that ended is told apart from a peer that misbehaved**, in
  HTTP/2, so bytes that arrived on a stream the server gave up on are kept
  rather than thrown away with an error
- [2] **A decompression bomb is refused** — the bound is on what comes out,
  because every other bound in a loader watches what comes in
- [2] **Connections kept between requests**, with the retry that has to come with them: a reuse that fails before a byte arrives is tried again, and a request that must not happen twice never is
- [2] **TLS**, rented, with verification that cannot be turned off — no flag, no constructor, no feature
- [2] **A certificate refusal a person can act on**: what is wrong, what trusting it anyway would mean, and whether the fault has an innocent explanation — three things a caller cannot show one of without the others
- [2] **The origin as a value other code compares**, with an opaque origin that is the same as itself and nothing else — a `data:` URL, a local file, and any scheme nobody registered

## The process model — stage 2

- [2] **The process and sandbox model, designed before the first hostile page is ever loaded** (ADR 0005). One process per site, renderers with almost no privilege, the platform's own sandbox rather than one of ours, and work crossing as typed messages in one direction. Memory safety does not make this optional: Spectre is a hardware property, and the codecs we rent are not ours to make safe
- [2] A renderer that dies costs one tab and never the browser, and says so rather than leaving a blank rectangle
- [2] The transport, and the lifecycle that starts, reuses and reaps renderers
- [2] **Where one site ends and another begins — the origin, the site, and which of them gets a process.** The origin decides whether there is a site at all: where it is a scheme, a host and a port, the registrable domain widens it into a site and two tabs share a process, with the port left to the origin because two ports can already reach one another. Where it is **opaque** — a local file, a `data:` page, `about:`, a scheme nobody registered — there is no site, and the document is rendered in a process nothing else is ever put into, not even the same bytes opened a second time
- [2] What a **document inside a document** is given is still owed, because there are none yet: a sandboxed `iframe`'s opaque origin and `about:srcdoc` inheriting its parent's, and a `blob:` taking the origin of whoever created it

## The network — stage 2

- [2] **URLs, properly**: WHATWG parsing, origins, IDNA and punycode. Every security decision below is made against the origin this produces, which is why it comes first
- [2] TLS with `rustls`, and certificate errors a person can act on rather than click through
- [2] HTTP/1.1 and HTTP/2, with connection pooling and keep-alive
- [2] HTTP/3 and QUIC, after those are correct
- [2] DNS, with encrypted DNS as a choice somebody made rather than a default nobody was told about
- [2] Content encodings: gzip, brotli, zstd
- [2] Redirects, byte ranges, and downloads that resume
- [2] **The HTTP cache with real semantics** — freshness, revalidation, `Vary`. Subtly wrong here is invisible for months and then serves somebody a stale bank page. What of it may be written to a disk is a second question, and it has its own answer (ADR 0011) and now its own code: partitioned by top-level site like cookies, and a page behind a password never written down at all
- [2] **Cookies**: `SameSite`, `Secure`, `HttpOnly`, partitioned by default — the default is a product decision, not a parser detail
- [2] The same-origin policy, CORS and preflight
- [2] Content Security Policy, referrer policy, HSTS, mixed-content blocking
- [2] `fetch()` and `XMLHttpRequest`, over the same stack rather than beside it
- [2] WebSocket
- [2] ★ **Every request attributable** — which page, and which agent action, caused it. No other engine has needed to answer that, and an agent-driven browser that cannot is one nobody should trust. What is recorded, for how long, and who may read it is a decision rather than a detail, and it is made (ADR 0012): the cause is carried on the request rather than guessed at afterwards, a renderer never states one, the record is the session's except for what an agent did — which is kept until the person deletes it — and no page and no agent may read any of it. **The cause is carried**: three of them and no fourth, with no default, so a request that cannot say what caused it does not compile, and the requests the browser makes for itself — a redirect, a resumed download, a permission check, a policy violation report — each name whatever caused the thing they are about. **And the chain**: loading a page makes a document that records what caused its own load, so a fetch by that page walks back to the agent action that opened it and on to the person — both answers, in order, rather than one chosen. A page somebody opened themselves reaches no action at all, an action in one tab cannot reach into another, the walk stops rather than looping if it ever meets a document twice, and a chain that reaches a document too old to still be remembered says the piece is missing rather than reading like one that ended. **And the session's record**: every request the engine makes is a line in it, written where the request is made rather than by whoever remembered to — when, the cause, the method, the URL, the purpose, and what happened, and **never a body and never a header set**, because a record holding `Cookie` and `Authorization` is a file that logs somebody into their own bank. What the cache answered is a line that says so, and a hop a rule of ours refused is a line naming the rule, because *what did this page try to load, and what stopped it* is most of why anybody opens it. It dies with the process, it is bounded in lines and in bytes — a URL is as long as a page chooses — and it says how many it has dropped rather than quietly reading as a session in which less happened. Emptying it is real. No page and no agent can reach it: a renderer holds no pool, nothing crossing the process boundary carries a line, and the agent surface does not depend on the loader at all. **And what an agent did, kept until the person deletes it**: the small durable half beside the session's, holding only the requests whose chain reaches an agent action — so a person's own browsing is never written to a disk at all. Each chain is **frozen** as it is written, because a file read back next week has no documents left to walk against, and it holds numbers rather than identities because this morning's `action#0` is not last week's. A private window leaves **no file behind at all** — never written rather than written and deleted. The bound is counted in **actions**, so one action with three hundred requests in it cannot evict a week of ordinary ones, and the oldest action goes whole; a `data:` URL keeps its kind and loses its content, since a URL that *is* the content is a body wearing an address's clothes. It lives where the system keeps what it may not delete rather than beside the cache. A file that does not read is a **gap it counts** rather than an error, and it is left where it is because it is somebody's record. Deleting is real: the files, not a flag on them. Still to come: the action's own outcome beside the requests it caused

## JavaScript — stage 2

- [2] ★ **A JavaScript engine, ours, in Rust.** Stage 1 needs none at all, which is what removes the largest component of a browser from the critical path. What it is, what it is not, and why it is ours rather than rented is a decision rather than a detail, and it is made (ADR 0013): `alo-js`, a parser, a bytecode compiler and an interpreter in safe Rust, **correct before fast**, with no JIT until a measurement on hardware says the interpreter is why somebody reached for another browser and an ADR of its own weighs that against `unsafe` in the largest target in the browser. Renting is re-argued rather than inherited, and the close call is a Rust engine that already exists rather than V8: refused because a rented collector decides how the DOM is stored and a rented engine's limits are a stranger's idea of how much memory a script may make us allocate. Three clauses shape everything built under this heading — **absent beats approximate**, so a builtin we have not written is *not defined* rather than stubbed, because a stub is the one answer that defeats a page's own feature test; the engine **reaches nothing** — no network, no filesystem, no clock — so every capability comes from the embedder and the browser process never runs page script; and a script is hostile input that we then execute, so every allocation it can cause has a bound of ours, the interpreter is interruptible by the browser rather than by a clock of its own, and it never panics on any program
- [2] Lexer and parser to an AST — the current language, not ES5. **The lexer is built** (`alo-js`, queue item 70): source text into tokens, one token at a time, and the thing worth knowing about it is that **it does not guess**. A `/` is a division sign or the start of a regular expression and nothing in the characters says which — `a /b/ g` is three divisions or one regular expression, and which one it is decides what the program does. Every editor guesses from the token before and is wrong on real code; here the caller states which it expects, every single time, so the ambiguity is settled by the thing that knows rather than by a rule of thumb. The same goes for the `}` that ends a `${` inside a template. Everything the modern language spells is read: numbers in four bases with separators and `BigInt`, strings as the UTF-16 code units they really are — so a half of a surrogate pair survives being read — templates with both what the author wrote and what it means, private class members, names in any script Unicode has, and a word spelled with an escape remembered as one, because `if` is a name and not the keyword `if`. The three legacy forms ADR 0013 sends to the tail are refused **by name** rather than quietly mis-read: `0755`, `'\1'`, and HTML-like comments. And it never stops: every truncation and every cut of a nasty script is a refusal that says what was wrong and where, because a renderer that crashes on a script is a page that can take the tab down. The parser is queue item 204
- [2] **A parser, to a syntax tree** (`alo-js`, queue item 204). The tokens become a program: statements, declarations, functions, classes, patterns, modules, and every node saying which bytes of the source it came from — because a stack trace and `Function.prototype.toString` are both built from that and neither can be added afterwards without touching every line that makes a node. The second ambiguity the lexer named is settled here: `(a, b)` and `(a, b) => c` are the same characters until the closing parenthesis has been passed, so the parameter list is **tried and put back**, and a `(` that turned out not to be one is remembered so a page that nests them pays for the attempt once rather than at every level. Where the specification refuses to choose, so does this — `a ?? b || c` and `-a ** b` are refused rather than given a reading — and `with` is refused **by name** (ADR 0013 § 3). Automatic semicolon insertion is all three of its rules, so `return` alone on a line returns nothing. And there are two ceilings that are ours, because there are two ways a stranger's file makes something here go too deep. A script of twenty thousand open brackets is a refusal, and the parse runs on **a stack of the engine's own** rather than the caller's, because a limit somebody else chooses is not a limit. The second one was found by the work that comes next and is the less obvious of the two (queue item 208): `a.b.b.b…`, `a()()…`, `a?.b?.b…` and `a+a+a…` are read in a **loop**, so they cost the parser nothing and build a tree as deep as the file is long — and everything that reads that tree afterwards walks it one step at a time, starting with the ordinary business of throwing it away. Sixty thousand `+` signs, which is a hundred and eighty kilobytes of somebody else's page, parsed in a moment and then took the process down. Both are refusals now, each saying which of the two it is. What the bound deliberately does **not** count is width: a page holding an array of fifty thousand numbers or an object of twenty thousand entries is ordinary, and a limit that added those up would refuse most of the real web
- [2] Two frozen pages' own scripts, read: alo's own service worker and alo's own theme generator, in the corpus beside the pages, so what the lexer and the parser are judged against is real code somebody wrote for a different purpose rather than examples written to match the code. The second holds six regular expressions and no division at all, which is what makes the `/` question a real one rather than a theoretical one — a test walks the whole tree and requires that not one of them was read as arithmetic
- [2] **Function headers obey the body's strictness** (`alo-js`, queue item 205). A plain parameter or function binding named `eval`, `arguments` or a strict reserved word is refused even when the `"use strict"` that forbids it comes later, inside the body. Duplicate strict and arrow parameters are early errors. Rejection happens before any statement runs, including for a function never called. A method's property name and legal lexical shadowing remain valid, and `('use strict')` is an ordinary expression that ends the directive prologue. Complex parameter forms, private-name checks, named-expression binding separation and loader attributes remain item 222; parsing alone does not certify all early errors.
- [2] **A bytecode compiler and an interpreter: correct first.** **The machine is built, and functions run** (`alo-js`, queue items 72, 209, 212, 214 and 216). A program is turned into a flat list of instructions and then run, rather than being walked as a tree — the difference matters later rather than now: a paused function, which `await` and generators both are, is a thing you can build on a list of instructions and a rewrite on a tree. What runs today is numbers, strings, `true`, `null`, all of the operators with the conversions the language asks for in the order it asks for them, `var`, `let` and `const` with the *dead zone* that makes reading a `let` too early an error rather than `undefined`, blocks that shadow, objects and their properties, `a?.b`, backtick templates, every shape of control flow there is — `if`, three kinds of loop, `switch`, labels, `break` and `continue` — and **functions**: declaring one, calling one, `return`, arrow functions, `this`, and closures. A property can also be a **question rather than a thing** — `{ get total() { … } }` runs that line every time somebody reads `total`, and a `set` runs when somebody writes it, which is what frameworks keep two things in step with — and an object asked to be a string or a number is *asked*, so `"total: " + basket` calls the `toString` the page wrote for it. Five things about it are worth knowing. **What a script is holding lives where the memory collector can see it**: the engine's own working stack is in the same place a page's objects are, so a value half way through being computed is never quietly thrown away — and every one of the tests runs twice, the second time with the collector firing at *every* allocation, because that is the only way that kind of mistake shows up. **A closure keeps what it was written beside, and lets go of it when it is let go of**: the names a function declares live where a page's objects live rather than on a stack that disappears when the function returns, so `function counter() { var n = 0; return function () { return ++n; }; }` works the way every page expects — and a call that keeps nothing leaves nothing behind, which a test proves by counting a thousand calls' worth of objects back to zero rather than by watching how much memory the process is using. **Nesting costs no depth to run**: the thing that reads instructions never calls itself, and a *call* is a note on a list rather than the engine calling itself either — so a page cannot choose how much of this process's stack it uses by nesting brackets or by recursing for ever, and a recursion that will not end is the same error every other browser gives rather than a tab that vanishes. The thing that *compiles* does recurse, so it is given a stack of its own, measured against the deepest program the reader will accept. **A call that starts half way through something still costs no depth**: a getter, a setter and the method that turns an object into a string are calls nothing in the source spells, and each of them begins in the middle of an instruction — so the instruction hands its half-finished work over, the call joins the same list every other call is on, and the answer comes back to the place it was left. That is why a getter that reads itself for ever is the same catchable error a runaway function is, rather than the tab going away. And **the browser stops a script, not a timer**: there is no clock in the engine at all, and a page that will not finish is ended by the process that decided a tab has stopped answering. And **a loop gives every pass its own names**: `for (let i = 0; i < 10; i++)` making ten functions makes ten that answer with ten different numbers, rather than ten that all answer with the last one — the mistake every JavaScript programmer has met once, which the language fixed by giving each pass a copy and which this engine now does the same way. A function may read a name that a *block* around it declared, which is ordinary code that this browser refused until today. And **`new` makes things** (queue item 212): a function written with `function` comes with a `prototype` object, `new Basket(items)` makes an object that inherits from it, runs the function with that object as `this`, and answers with it — or with the object the function returned instead, if it returned one, which is a rule pages rely on. An arrow, a method and a builtin cannot be constructed and say so with the same error every browser gives, and that error comes only after the arguments have been worked out, because a page can see the order. What opened it was alo's own service worker, which did not get past its first `new Request(…)`. And **an array literal makes an array** (queue item 225): `[1, , 3]` is three long with nothing at all in the middle — not `undefined`, which a page can tell apart with `in` — writing past the end makes it longer, and setting `length` smaller throws the end away, as every browser does; a length that is not one, like `-1` or `1.5`, is the same `RangeError` everywhere else gives. That was the service worker's next stop, and it then compiled to the `try` on the line after it. And **`try`, `catch` and `finally` work** (queue item 210): a page's own `catch` catches what was thrown — from any depth of call, from inside a getter, or from the `toString` an error's message asked for — and an error the engine itself raises, like reading a property of `null`, is caught as a real `TypeError` with its message, which is how a page survives a browser that does something differently. A `finally` runs once whichever way its block was left: normally, by a throw, a `return`, a `break` or a `continue`, and two of them run innermost first. A recursion that catches its own "too deep" error and tries again can be stopped by the browser like any other script that will not finish. The service worker then compiled past its `try` to the `for…of` inside it. And **`for…of` works** (queue item 230): `for (const item of list)` visits every element in order — a hole as `undefined` — and an array that grows while it is being walked is walked to its new end. `let` and `const` give each pass a name of its own, so functions made in the loop remember different items, and `for (const x of x)` is the "used before it is declared" error rather than a read of some other `x`. The way it asks for each item is the way the language says, through calls a page can see and replace — which is how a library makes its own collections work in a `for…of` — and when a loop is left early, by `break`, `return` or an error, the thing being walked is told it will not be asked again, while a loop that simply finishes tells it nothing. `[].values()`, `keys()` and `entries()` exist for the same reason, and say `[object Array Iterator]` when asked what they are. With that, **alo's own service worker compiles from its first line to its last**; running it waits for `self`, the worker's global, which the browser supplies. What is not built is refused **by name**, each saying which piece of work builds it — classes, `super` and `new.target`, `instanceof`, spreading and taking a value apart (a `for…of` whose head takes an item apart included), `for…in` and `for await`, an array iterator reading an item through a getter, an object assigned to an array's `length`, `arguments`, default and rest parameters, and tagged templates — because a program that ran and was quietly wrong is worse than one that says what it could not do
- [2] A garbage collector, and the object model underneath it. **Both are built** (`alo-js`, queue items 71 and 206). What a script makes — objects, strings, functions — lives in one place the engine owns, and a reference to any of it is a **number naming a slot** rather than a machine address, which is why a mistake in this engine can be *seen* rather than being the kind of bug browsers get attacked through. Reclaiming is by **tracing**: start from what is definitely in use and keep what can be reached from it. Counting references instead would be simpler and would leak the commonest thing on the web — a button holding a function that mentions the button is a loop, and a count never reaches zero on a loop. **The page's own document is in the same graph**, which is the clause worth caring about: every browser that treated a page's objects and its elements as two separate things spent a decade finding leaks that only show up after a day of use, and here one thing decides what is alive. Three things go in before they are needed, because each is a rewrite if it arrives late: the hook that would let collection happen in slices if a person ever sees the browser pause, the loop that makes a `WeakMap` right rather than nearly right, and a collector that walks a page's objects **without recursing**, since a script can otherwise choose how deep it goes and take the tab down. Nothing that reclaims memory ever runs out of memory doing it, and nothing here is a stub. What that came to in practice: a chain of a million objects is tidied away with no risk of the walk running out of room, because the walk keeps its own list rather than calling itself; a list of things to look at that fills up costs the collector a second look rather than costing a page an object; a `WeakMap` with more live entries than that list holds keeps every one of them; and a page that asks for more memory than the browser will give one page is told so in a sentence, with the tab stopped, rather than taking the browser down with it. The tests count what survived rather than watching how much memory the process is using, because the second measures the machine. **And what a script makes is now a thing rather than whatever the caller put there**: an object is a prototype, a table of properties and one switch saying whether more may be added, and a property is a value with three switches of its own — whether it may be written, listed, or removed. The order a page gets when it asks an object for its property names is **the order every other browser gives**: the ones that look like whole numbers first and counting up, then the rest in the order they were added, then the invisible ones a page makes on purpose. That is not a detail to add later — it is invisible until the day a page enumerates and gets a different answer here than anywhere else, and by then everything is built on top of it. A name used twice is **stored once**, so ten thousand records each having a `title` hold one copy of the word; a name nothing uses any more is tidied away with everything else, which is what stops a page inventing names in a loop from growing the browser until it falls over. The way a page's own elements behave differently from ordinary objects — a list that answers to numbers, a property that refuses to be redefined — is **the same mechanism** the language itself uses for arrays and functions, rather than a second one beside it, which is what keeps a document and a script one thing that agrees with itself. Reading a property that a page computes with a function hands back the function rather than pretending to have called it, because calling one is the next piece of work and a browser that answered `undefined` instead would be quietly wrong. What a page cannot do, each with a test that has a number in it: build a chain of a hundred thousand objects and make the walk down it fall over, close that chain into a loop, or add and remove properties until the memory holding them grows without end
- [2] The ECMAScript standard library, in the order real pages need it. **The two objects the language is made of are built** (`alo-js`, queue item 218): the one every `{}` inherits from and the one every function inherits from. That is not a library, it is what an object *is* — until today `"" + {}` stopped the script, because an object had no `toString` to find, and that is the commonest thing a page does with a value it is not sure about. `toString`, `valueOf`, `hasOwnProperty`, `isPrototypeOf`, `propertyIsEnumerable` and `__proto__` all answer, and the last of them refuses to make a loop out of a prototype chain, which is what keeps a chain walkable. The mechanism underneath is the half that decides everything after it: **a builtin is a function whose body is ours rather than a script's, and it is called by exactly the machinery a page's own functions are called by** — so a page may replace one, a page's own method shadows one, and one is found where a `+` or a getter reaches for it rather than only where a page names it. A builtin can now suspend while the interpreter calls script or converts an object, then resume with the answer on the traced stack (queue item 219). `Function.prototype.call`, `Object.prototype.toLocaleString` and object arguments to property-name methods use that mechanism. Waiting builtins count toward the same recursion limit as script frames. Function source text remains refused by name (220); `apply` and traced native scratch state remain item 221. An array is an array to the library too: `Object.prototype.toString` says `[object Array]`, and the object every array inherits from is itself an empty array with **no methods on it yet** — `[].push` is `undefined`, which is what a page's own check for it reads correctly, rather than a `push` that does half of what it should (queue item 225). **The errors a page catches exist** (queue item 227): `Error`, `TypeError`, `RangeError`, `ReferenceError`, `SyntaxError`, `EvalError` and `URIError` are on the global object, `new TypeError("bad")` has the message it was given and an optional `cause`, `"" + error` reads `"TypeError: bad"`, and calling one without `new` makes the same object as with it. They are the first built-in functions that can be used with `new`. An error the engine itself throws becomes one of these when a page's `catch` catches it (queue item 210); an error's `toString` whose message is a getter is refused by name until a builtin can hold a value across a call (items 228 and 221), and `AggregateError` waits for iteration (229). **An array can be walked** (queue item 230): `keys`, `values` and `entries` hand out an iterator, and the same `values` is what `for…of` asks an array for — under a name no page can spell yet, `Symbol.iterator`, which the engine makes along with `Symbol.toStringTag`, the name `Object.prototype.toString` now reads before it answers. Still absent, each named rather than stubbed: `Object` and `Function` themselves, which are constructors; the `Array` constructor and every other array method, `Math`, `JSON` and the wrapper objects; the `Symbol` function and the rest of the well-known symbols; the iterator helpers; and the weak collections
- [2] Regular expressions, with the syntax the language actually has
- [2] Promises, the microtask queue, `async`/`await`, generators and iterators. The microtask queue is built, as the engine's half of the event loop below (queue item 232); promises themselves are not
- [2] Modules: ESM, dynamic `import()`, and the loader that fetches them
- [2] **The event loop** — tasks, microtasks, the rendering steps, `requestAnimationFrame`. **The engine's half is built** (`alo-js`, queue item 232, ADR 0016): the queue of small follow-up jobs — what `queueMicrotask` asks for, and what a promise's reactions will be — lives where the memory collector can see it, so a job waiting to run is never thrown away and a job nobody can run any more is. After a piece of work, the engine runs every waiting job, oldest first, **including the ones those jobs queue**, which is what lets a chain of promises finish before the page does anything else; a job that throws is reported and the next one runs, as every browser does; and a page the browser has stopped has its waiting jobs dropped rather than resumed in a state nothing on it expected. The order is the one the specification gives, down to the case libraries rely on: two click handlers a *person* triggered each see the other's jobs run between them, and the same two triggered by a script's `element.click()` do not. **And the loop itself is built** (`alo-renderer`, queue item 235): the work waiting for a page is kept in one queue and the oldest piece always runs next; after every piece of script — a script, or one handler of several the browser hands the same click to — the follow-up jobs it queued run before anything else does, so each handler a person triggered sees the one before it finish; `queueMicrotask` is on the page's global object; an error nothing caught is reported and the page carries on; anything else that ends a script — the browser stopping it, memory running out — stops the page, drops everything it was waiting to do and lets go of what that work was holding. Between two pieces of work the loop checks that nothing is still being held half way through, and stops the page rather than carry on if something is. **And a page's own scripts run** (queue item 236): a script written into the page runs as the page loads, in the order the page wrote them, each followed by the jobs it queued — but only where the page's own Content Security Policy allows it, whether that policy came in the response's headers or in a `<meta>` written before the script, and a script's nonce counts only when its markup is not the shape an injection leaves. A script that is fetched from elsewhere, a module, and a script the policy refused are each named in what the page load reports rather than skipped in silence; resizing the window does not run anything again; and each page starts with a fresh set of globals. A script that never finishes costs its own renderer, which the browser gives up on, rather than the browser. No script can see the page's document yet (80). Still to come: the loop running between the browser's messages, for work a page schedules for itself, and an agent's action answered only after the jobs it caused (233); drawing frames with `requestAnimationFrame` (234); and a page's fetched scripts (238)
- [2] Errors and stack traces good enough to debug somebody else's minified page. **An error nothing caught is reported by what it says** (`alo-renderer`, queue item 239): `TypeError: no such thing` rather than `an object`, by the error's own name — so a page that renamed its errors is reported by the new name — and its message. The words are read without running any of the page's code: a name or message only a page's function could produce is said to be one rather than asked for, and a page that replaced how errors turn themselves into text does not get to choose what its own failure says. A page's long strings are repeated only to their first 1024 characters, with the rest counted. **And where it happened** (`alo-js` and `alo-renderer`, queue item 241): the report names the script, line and column of the throw, then of each call it left on the way out — `(at script 2, line 3, column 5; called from script 1, line 1, column 9)` — so a function one script declared is found in that script when another calls it. Columns count UTF-16 code units, as every other engine and developer tool does, because in a minified page that is one line long the column is the only thing that finds anything; the renderer marks each script once as it keeps it so that finding a column never means re-reading a megabyte bundle from the start. A runaway recursion says its innermost thirty-two places and how many more it left. A rethrown error is placed where it was thrown again; `error.stack`, taken where an error is made and with function names in it, is still to come (78). **And there is a ceiling on how much is said** (`alo-renderer`, queue item 242): a load says at most 256 things about its scripts, then how many more there were, because everything a load says crosses to the browser in one message of bounded size and a page that throws in a loop would otherwise make its own report unsendable. The scripts all run either way, and past the ceiling a throw is counted rather than described, so a page that throws for ever costs a counter while it is being stopped rather than memory. **The same for its markup** (`alo-renderer`, queue item 243): what a page's markup, style sheets, pictures and fonts make the engine say is at most 256 lines a load, then how many more, and every line of a load's report — about markup or scripts — is at most 8192 characters, then how many more, because a line quoting what a page wrote quotes it escaped and a control character is five times its size there. Only the lines said are written out, so a page of millions of refusals costs a count; the page renders the same either way. **And the font names a load asks for** (queue item 244): a family longer than any font's can be is said rather than asked for, so nothing a load sends is longer than a font name except the bounded lines above
- [2] Internationalisation (`Intl`), rented rather than written
- [3] A JIT — refused until there is a measured reason and an ADR weighing it against the attack surface it adds

## The DOM as pages use it — stage 2

- [2] Mutation from script, and the invalidation that has to follow it. **The document's half is built** (`alo-dom`, queue item 245, ADR 0017 § 5): a page's tree can be changed — an element or text made, put in, moved, replaced or taken out — only in the ways the DOM standard allows, and a change it does not allow is refused by the standard's own name (`HierarchyRequestError`, `NotFoundError`, `InvalidCharacterError`) with nothing changed: no node inside itself, no second element or doctype under the document, no text directly under it, no template inside its own contents. A node made this way is numbered after every node the page already had, so an agent can name it like any other; every change is counted, so a renderer can tell what it drew is out of date; and a detached piece of the page nothing can reach any more can be let go, its numbers never handed out again. **The engine's half of keeping it is built too** (`alo-bindings`, queue item 248, ADR 0017 §§ 2–4): once a page runs script its document can be held in the page's own memory, counted at its real size so a page that builds without end is stopped like any other; the same element asked for twice is the same object, so what a page attaches to it stays; an element removed from the page is kept exactly as long as something still holds any part of the piece it is in, and freed at the next tidy-up when nothing does. **What a script calls is built too** (`alo-bindings`, queue item 249, ADR 0017 §§ 1, 4, 5, 8): a script given a document can find the page's root element and walk to any node's parent, first and last child and siblings; make an element or a piece of text; put it in, move it, swap it for another or take it out; read and set an element's attributes by the names a page writes them in; and read or replace a node's text. A change the standard forbids reaches the script as the error the standard names, which the script can catch; calling a member on something that is not the right kind of node, or handing it a node from another page, is a plain type error. Everything else a page might reach for is absent rather than half-done, so a page that checks before using a feature reads the truth. **And a page's own script reaches it** (`alo-renderer`, queue item 250, ADR 0017 §§ 2 and 6): when a page's first script that may run is about to, its document is handed to the page's script, and from then on the page is drawn from whatever the script left — the box tree, the layout, the picture and what an agent reads all show the element a script added, and an agent can name that element and act on it like any other. The page is drawn again only when something reads it — a frame, an agent reading or acting, a load ending, a window resized — so a script that changes its page ten thousand times in one go costs one drawing, and resizing a window lays out the page as it now is rather than as it arrived. A page none of whose scripts may run never has a script engine made for it. **And a script runs where it is written** (`alo-dom` and `alo-renderer`, queue item 247, ADR 0017 § 7): the page is read a piece at a time, stopping at the end of each script, so a script sees the page as far as it has been read — itself the last thing in it, nothing written after it there yet — and what it adds beside itself appears there, before the content written after it. Scripts still run in the order they are written, each under the policies stated before it; a policy a page states keeps governing the scripts after it even if a script removes the tag that stated it; and a script the page leaves unfinished at the end of its markup does not run, as every browser has it. **And a refusal a page lets escape is named** (`alo-renderer`, queue item 252): what a load reports for it is the standard's name and the rule that refused — `HierarchyRequestError: a document cannot hold text` — exactly what the page's own `catch` would have read, taken from the exception itself, so nothing the page did to it can change the words and reporting it runs none of the page's code. **And a script can find the page's body and swap it** (`alo-dom` and `alo-bindings`, queue item 253): `document.body` is the page's body — nothing before the page has one — and giving it a new body puts that one where the old one was, or adds it to the page when there was none; anything that is not a body is refused by the standard's name with nothing changed. Owed beyond this line: `document` as the standard's accessor (251)
- [2] **Events**: capture and bubble, listeners, default actions. **Events a page's own script sends are built** (`alo-js` and `alo-bindings`, queue item 254, ADR 0018 §§ 1–3 and 8): a script can listen on any node — for the trip down from the document to the target, or the trip back up — and send an event of its own (`Event`, or `CustomEvent` carrying whatever the page wants it to). Listeners hear it in the order the standard gives: down through each ancestor, at the target, then back up if it bubbles; each in the order it was added, a listener added or removed while the event is travelling counted as the standard counts it. A listener can stop the event going further, or stop even the listeners after it on the same node; one that asked to be called once is; one that promised not to cancel cannot; and the sender is told whether anyone cancelled. A listener that throws is reported, where it threw, and the next one still runs — the page's own `try` around the send does not see it. A listener goes when its node does, closure and all; a node the event is passing through is kept until it has passed. Absent rather than half-done: the time it was sent (92), the legacy forms, and listening on the window (251). **The browser's own dispatch is built too** (`alo-renderer`, queue item 255, ADR 0018 § 3): an event the browser makes is sent to a node as one task of the page's event loop, through the same rules, with whatever each listener left waiting run before the next listener starts — the difference from a script's own send that real pages rely on. A listener that throws is reported and the rest still hear it; a page told to stop mid-event stops; a page that never ran script is sent nothing and is not given a heap. **An agent's press arrives as a keyboard's click** (`alo-dom`, `alo-bindings` and `alo-renderer`, queue item 256, ADR 0018 §§ 4–7): on a page that runs script, an agent pressing something sends the page the one `click` a person pressing Enter on it would — trusted, a `PointerEvent` saying no pointer and no position — and the page's own listeners decide what it does. A checkbox or radio changes before the listeners run, so they read the new state, and goes back if one of them cancels; one nobody cancels then tells the page with `input` and `change`; a link is followed only if nobody cancelled. The agent is answered after all of it, with anything the page's script said. On such a page the agent no longer changes ARIA state itself — the page's listener does, and doing it twice would undo it; a page with no script is acted on as before. alo's Settings screen carries its nav script, and an agent pressing *Sharing* makes it the open row. **Every event says whether the browser sent it** (`alo-js` and `alo-bindings`, queue item 260, ADR 0019): `isTrusted` is `true` for an event the browser sent and `false` for one a script made or sent, and a page cannot change the answer — it is on each event itself, locked, rather than somewhere shared a page could replace. **A page's script can click** (`alo-bindings`, queue item 261, ADR 0018 § 6): every HTML element has `click()`, which sends the same click an agent's press does — except that it says a script sent it — through the same rules: a checkbox or radio changes before the listeners and goes back if one cancels, `input` and `change` follow, all before `click()` returns, and nothing a listener left waiting runs until the script that clicked is done. A disabled control ignores it, and a `click()` from inside the same element's own click does nothing. **A clicked link asks the browser to go there** (`alo-bindings` and `alo-renderer`, queue item 263, ADR 0020): a link a script clicks, or an agent presses, is followed by *asking* — the page's process works out the full address against the page's own (or its `<base>`) and says where the page wants to go in its answer, and the browser decides. It reads the address again itself, goes only to `http`, `https` and `about:blank` — a local file only from a local file — and refuses anything else by name, a `data:` page, a `javascript:` URL or another program's scheme among them; it records who caused it from what it had asked the page to do, so a link a page's listener clicks while an agent presses something is the agent's doing, with the page's click kept beside it. A task that clicks several links asks only for the last, and says how many it replaced; a link that opens a new window or asks for a download is not followed yet and says so (118, 264). Going there — loading the next page, history — is still to come (85), and each element's own interface — `HTMLInputElement` and the rest — is still to come (262). **An agent's typing reaches the page** (`alo-dom`, `alo-bindings` and `alo-renderer`, queue item 257, ADR 0018 § 5): on a page that runs script, an agent putting text into a field first asks the page with a trusted `beforeinput` — an `InputEvent` saying the field's text is being replaced and carrying the new text — and a page that cancels it keeps the field as it was, and the agent is told the page refused rather than that the text went in. Otherwise the text goes in and the page hears `input`, then `change`, so a page that echoes or checks what is typed does so for an agent as for a person; the agent is answered after all of it, with anything the page's script said. A page with no script takes the text as before. Still to come: focus (258) and `onclick` (259)
- [2] **Forms**: the controls, constraint validation, submission, file inputs
- [2] **Navigation and session history**: `pushState`, back and forward, and what survives each
- [2] `iframe`s and the sandbox attribute
- [2] Shadow DOM and custom elements — component frameworks are not optional on the modern web
- [2] Selection and ranges
- [2] CSSOM — styles readable and writable from script
- [2] Storage: `localStorage`, `sessionStorage`, IndexedDB, the Cache API, and one quota policy over all of them
- [2] Workers: dedicated, shared, and service workers with their fetch interception
- [2] Timers, clipboard, drag and drop
- [2] ★ **Permissions as capabilities** — camera, microphone, location, notifications, in the shape of `alo-os` ADR 0001: enumerated, visible, revocable, expiring, recorded. A browser is where most people meet a permission prompt, and every other one is a dialogue nobody can audit afterwards

## Pictures and media — stage 2

- [2] Image codecs, rented: PNG, JPEG, GIF, WebP, AVIF
- [2] **SVG** — a second rendering model inside the first, and far larger than one line suggests
- [2] Canvas 2D
- [2] Audio and video playback through rented decoders
- [2] Media Source Extensions, without which most video sites do not play at all
- [2] Web Audio
- [2] WebGL, then WebGPU — both large, both late, and neither before the software path is right

## Speed, where slow means unusable — stage 2

- [2] **Incremental style and layout** — recompute what changed, not the document. The largest single difference between an engine that renders a page and one somebody can use
- [2] Compositing layers, and scrolling that does not repaint the world
- [2] Off-main-thread scrolling and animation
- [2] A performance budget somebody can hold us to: named pages, measured, in CI

## The browser itself — stage 2

- [2] A window, tabs, and a tab strip
- [2] The address bar: what somebody typed, what it means, and a search that phones nobody by default
- [2] History, bookmarks, downloads
- [2] Find in page, zoom, and per-site settings that stick
- [2] Context menus, and keyboard operation of every one of them
- [2] Printing, print preview, export to PDF
- [2] Viewing a PDF, or saying plainly that we hand it to something else
- [2] Private browsing, and profiles that are genuinely separate
- [2] Autofill, and credentials held where the operating system holds secrets rather than in a file of ours
- [2] Security surfaces: certificate detail, permission state, what this page has stored — reachable, none of it buried
- [2] Settings
- [2] **Developer tools**: inspector, console, network, performance. A browser nobody can debug a site with is not one a developer keeps
- [2] **Accessibility on the shell**: keyboard operation of everything, focus always visible, and the EN 301 549 conformance the workspace is already held to

## The legacy tail — stage 3

Deliberately last, and possibly never finished. Refusing this list is what made
stages 1 and 2 survivable; a broken render schedules the work, not a
specification.

- [3] Quirks mode
- [3] **Floats as layout**, and CSS table layout — the two that most often turn an old page into a column of rubble
- [3] `document.write`, live `HTMLCollection`s, and the DOM as it was before it was a specification
- [3] Legacy character encodings, and detecting them
- [3] XML, XHTML and XSLT
- [3] `frameset`
- [3] Vendor prefixes, and anything that exists only for a page written before 2015
- [3] The sloppy-mode corners of JavaScript that only old code reaches

## A browser somebody chooses — stage 4

**[4]** is a tier this file did not have. It was added when the roadmap gained a
fourth stage: product work, gated behind stage 2's exit gate, listed rather than
left unnamed because "not scheduled" had begun doing the work of "not thought
about".

- [4] Extensions — and the decision, in an ADR, whether that means the WebExtensions API or something narrower we can actually secure
- [4] Sync, self-hosted: bookmarks, history, tabs and passwords, end-to-end encrypted, on the customer's own server
- [4] Updates that are signed, staged and reversible
- [4] A mobile port
- [4] Crash handling that helps us fix it without becoming telemetry
- [4] ★ **Translation on the machine** — alo already runs models locally; a page translated without sending it anywhere is the sovereign version of a feature every other browser sends to a server
- [4] ★ **Reading and summarising a page locally**, under the same grants and the same record
- [4] Enterprise: policy, managed configuration, and an update mirror an organisation hosts

## Non-goals

**No text shaper, font rasteriser, codec or TLS stack of our own** — we rent the
physics, as Chromium, Firefox, Servo and Ladybird all do, and not out of
timidity. **No fork** of Chromium or Ladybird. **No `unsafe`** outside a
reviewed, named boundary with an ADR. **No conformance-percentage target** — the
measure is whether alo renders correctly, because a Web Platform Tests score
grades us against the legacy we are deliberately refusing. **No plugin-shaped
agent** bolted on afterwards — that is what every other AI browser already is,
and ADR 0002 exists to prevent it.

And four that no later stage may quietly adopt:

- **No DRM, and no Encrypted Media Extensions.** A proprietary binary with privileges inside a sovereignty product is a contradiction. Sites requiring it will not play, and we say so rather than shipping a black box.
- **No proprietary codecs** we cannot ship freely.
- **No telemetry.** Not "anonymised telemetry". None — the rule alo OS already holds.
- **No search deal.** The address bar's default is decided for the person using it, not sold.
