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
- [2] Web fonts as pages actually ship them: WOFF2, variable fonts, and loading that does not flash
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
- [2] A form control draws its state — a tick in a checked box, a focus ring
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
- [2] **One renderer process per site** — two sites are two processes, and
  killing one leaves the other running
- [2] **A wire format for the renderer boundary**, where a message from a
  renderer is untrusted because a renderer is the process that parsed the page
- [2] **HSTS**, ignored over plain HTTP so it cannot be used as a weapon
- [2] **Mixed-content blocking** — a script refused outright, an image tried
  over TLS first, and `http://localhost` treated as secure because it is
- [2] **Referrer policy**, defaulting to origin-only across sites and nothing at
  all across a downgrade
- [2] **The same-origin policy, CORS and preflight** — a page may send almost
  anywhere and may read almost nowhere, and a wildcard never covers a request
  that carried credentials
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
- [2] **An HTTP cache**: freshness, `Age`, revalidation with `ETag` and
  `Last-Modified`, and `Vary` as a contract so one reader never gets another
  reader's page
- [2] **Redirects**, bounded and loop-detecting, with `Authorization` dropped
  at an origin boundary and `file:`/`data:` refused as destinations
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
- [2] Where one site ends and another begins — the origin, the site, and which of them gets a process

## The network — stage 2

- [2] **URLs, properly**: WHATWG parsing, origins, IDNA and punycode. Every security decision below is made against the origin this produces, which is why it comes first
- [2] TLS with `rustls`, and certificate errors a person can act on rather than click through
- [2] HTTP/1.1 and HTTP/2, with connection pooling and keep-alive
- [2] HTTP/3 and QUIC, after those are correct
- [2] DNS, with encrypted DNS as a choice somebody made rather than a default nobody was told about
- [2] Content encodings: gzip, brotli, zstd
- [2] Redirects, byte ranges, and downloads that resume
- [2] **The HTTP cache with real semantics** — freshness, revalidation, `Vary`. Subtly wrong here is invisible for months and then serves somebody a stale bank page
- [2] **Cookies**: `SameSite`, `Secure`, `HttpOnly`, partitioned by default — the default is a product decision, not a parser detail
- [2] The same-origin policy, CORS and preflight
- [2] Content Security Policy, referrer policy, HSTS, mixed-content blocking
- [2] `fetch()` and `XMLHttpRequest`, over the same stack rather than beside it
- [2] WebSocket
- [2] ★ **Every request attributable** — which page, and which agent action, caused it. No other engine has needed to answer that, and an agent-driven browser that cannot is one nobody should trust

## JavaScript — stage 2

- [2] ★ **A JavaScript engine, ours, in Rust.** Stage 1 needs none at all, which is what removes the largest component of a browser from the critical path
- [2] Lexer and parser to an AST — the current language, not ES5
- [2] A bytecode compiler and an interpreter: correct first
- [2] A garbage collector, and the object model underneath it
- [2] The ECMAScript standard library, in the order real pages need it
- [2] Regular expressions, with the syntax the language actually has
- [2] Promises, the microtask queue, `async`/`await`, generators and iterators
- [2] Modules: ESM, dynamic `import()`, and the loader that fetches them
- [2] **The event loop** — tasks, microtasks, the rendering steps, `requestAnimationFrame`
- [2] Errors and stack traces good enough to debug somebody else's minified page
- [2] Internationalisation (`Intl`), rented rather than written
- [3] A JIT — refused until there is a measured reason and an ADR weighing it against the attack surface it adds

## The DOM as pages use it — stage 2

- [2] Mutation from script, and the invalidation that has to follow it
- [2] **Events**: capture and bubble, listeners, default actions
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
