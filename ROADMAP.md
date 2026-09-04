# alo browser — ROADMAP.md

The only order things get built in. **A tick means done, not written** — where
something is built and tested but has not met the gate on a real screen, the item
says so rather than being ticked optimistically.

Every item appears in `docs/features.md`. If it is not there, it is not built.

## Three states, because two are not enough

A queue item is usually smaller than a line here, so a line is routinely *part
built* — and a box that can only be empty or ticked has no way to say that. It
said the wrong thing instead: fifteen of twenty-eight consecutive commits left
this file untouched, and stage 2's first line read as unstarted after its ADR
was accepted and its message boundary was built and tested.

So a line is written in one of three states, and there is no fourth:

- `- [ ]` **Not started.** Nothing exists.
- `- [ ]` **· Built: … · Owed: …** — part done. The clause names what exists
  *and* what is still missing, both concretely enough to check: a crate, a queue
  item, a named capability. It stays an empty box, because half a line is not a
  line.
- `- [x]` **Done** — the whole line, gated, with any remainder stated in the
  item's own words.

**The clause is a claim, so it names something.** A Built clause that cannot
name a crate or a landed capability is decoration, and decoration is how a
roadmap starts lying slowly.

---

## Stage 1 — it renders alo

No scripting, no hostile pages, no compatibility burden. The point is that
something real depends on this within months rather than years: when alo's own
screens render through it, every later stage has a working thing to grow from
instead of a demo.

**This stage needs no hardware and no operating system.** Not the certified
machine, not alo OS's compositor, not a GPU, a window or a network. HTML and CSS
in, a PNG and a box tree out — both files — so every item is built *and
verified* on an ordinary laptop by anybody who clones this repository. That is
deliberate, and it is why the browser is not waiting on the OS and the OS is not
waiting on the browser.

**What "an alo screen" means.** An alo screen is markup and custom properties.
`alo-workplace`'s screens are checked out beside this repository and its
`tokens.css` is the colour specification, so "correct" is diffable **today**.
An alo screen is alo's whichever repository it lives in; the gate below used to
name `alo-os`'s specifically, which made a finished item unclosable and is
corrected.

**Order.** A software raster to a PNG is deterministic and diffable, which makes
every step testable from the first one — hardware acceleration comes after
correctness, not before it.

- [x] **A DOM of our own**, built from `html5ever`'s parse events
- [x] **Stylesheets**: `cssparser` into rules we hold, selectors matched with `selectors`
- [x] **Computed style**: the cascade, inheritance, and `var()` — alo's design system is custom properties throughout, so this is not optional decoration
- [x] **The box tree**, and what each box *means* (ADR 0002) rather than only its rectangle
- [x] **Layout**: flexbox and grid, on `taffy` to begin with, behind our own boundary
- [x] **Text**: HarfBuzz shaping (via `rustybuzz`), the fallback chain and line breaking, with the awkward scripts working before the easy ones. Rasterisation is queue item 17, beside paint
- [x] **Paint**: a display list, then a software raster to a PNG
- [x] **Reference renders**: a committed corpus, diffed on every change
- [x] ★ **The agent tree**: the layout tree read as roles, states and positions
- [x] ★ **Typed verbs**: activate, type, scroll — and never a coordinate.
      A verb **changes the page**: text put into a field is in it, a checkbox
      ticks, a radio un-chooses its group, and the page is rendered again from
      the same document so held ids stay valid. *This line was ticked when a
      verb decided and reported and changed nothing, which is not what the line
      says. Queue item 42 made it true.*
- [x] **A real alo screen renders correctly** — the sign-in screen, then
      Settings. Both are `alo-workplace`'s own markup and rules, rendered from
      their own stylesheets with **no substitutions**, and diffed on every run
      against a committed image *and* a committed box tree (queue items 44,
      47, 48, 49 and 45). *One thing is true of both and is not a defect in
      either: the corpus renders in DejaVu Sans and the app loads Inter, which
      is narrower, so alo's headline wraps one line more here. Web fonts are
      stage 2.*

**Exit gate — met.** From this repository alone, on any machine: alo's sign-in
screen and Settings render correctly from their own markup and `tokens.css`,
with no substitutions left in the case — matched against the committed
reference image *and* the expected box tree, so "correctly" is a diff rather
than an opinion — and an agent reads the Settings screen as a tree and
activates a row by name rather than by position.

All three clauses hold. The two screens are `crates/alo-corpus/cases/`'s
`alo-sign-in` and `alo-settings`, and the agent is
`crates/alo-renderer/tests/an_agent_on_settings.rs`. Every one of them runs on
the machine anybody clones this on.

**What the gate does not claim.** That this is a browser — it is an engine, and
stage 2 is the rest. That the screens are pixel-identical to the app — the
corpus renders in DejaVu Sans and the app loads Inter, so alo's headline wraps
one line more here; web fonts are stage 2. And nothing about speed, which is
measured on hardware or not said.

Nothing in that sentence needs a compositor, a certified machine or another
repository. That is the point of the gate rather than an accident of it: a gate
nobody can reach is a gate that quietly stops being used, and this one had
already begun holding a finished item open.

### After stage 1, and not gating it

Real work, and the only items in the stage that depend on anything outside this
repository. They are kept out of the list above so they cannot block it.

- [ ] Hardware acceleration, once the software path is right — needs a GPU
- [ ] Embedding: alo OS's shell renders through it — needs alo OS's compositor,
      which does not exist yet. **The engine is finished for stage 1's purposes
      before this lands**; this is the OS adopting it, and is that repository's
      milestone as much as ours
- [ ] Rendering at speed on the certified machine, without stutter — needs both
      of the above, and is a performance claim: measured on hardware, or not made

---


## Stage 2 — it renders the modern web

The stage that turns an engine into a browser. It is the largest of the three by
a wide margin: stage 1 refused scripting, hostile pages and compatibility, and
this is where all three arrive at once.

**An honest word about the size.** What follows is years of work, and naming it
completely is the point — a list that stops at the interesting parts is how a
project discovers the boring half after it has promised a date. The order inside
each group is ours, and the trigger for most items is a real page that fails,
never a specification listing a method.

**Two things gate the rest** and are already under way: the process model,
because it cannot be retrofitted, and JavaScript, because most of this list is
unreachable without it.

### The process model

- [ ] **The process and sandbox model — designed before the first hostile page is loaded.** One process per site, renderers with almost no privilege. Every browser that retrofitted this suffered for years, and it is the one thing here that cannot be added later
      · Built: ADR 0005, and `alo-renderer` — the engine behind a message
      boundary, sent work and returning results, every message owned and
      `Send + 'static`, with a frame and an agent-tree snapshot as the only
      things that cross. That is the expensive half, and it is the half that
      cannot be retrofitted. **The wire format** (queue item 63): both
      directions, every variant, and a message from a renderer treated as bytes
      a stranger chose — because a renderer is the process that parsed the page
      **The split itself** (queue item 166): a renderer process per site,
      spawned and talked to over pipes, with a test that kills one and watches
      the other keep working **The sandbox** (ADR 0010 and queue item 167): rented, applied by
      `exec` so a renderer is never unconfined, and fatal if unavailable —
      watched failing rather than assumed **Fonts across the boundary** (queue item 168): the browser
      process opens the files and passes bytes, and `alo-render` embeds nothing
      **A font a page asked for by name** (queue item 170): a load says which
      families it wanted and did not have, the browser process finds each on the
      machine by the name the font gives itself, and a family that is genuinely
      not there is a substitution said in words rather than a silence
      **And the name the font gives itself is read in the encodings older than
      Unicode** (queue item 192): the Macintosh records several of the fonts
      macOS ships carry *instead* of a Unicode one, so a machine that has Apple
      Braille no longer says it does not — and with it, no family anywhere comes
      from a filename
      **And what a *generic* family means here** (queue item 193): the browser
      process decides `serif`, `sans-serif`, `monospace` and `system-ui` from the
      families it actually found and hands the answer over with the fonts, so
      the `system-ui, sans-serif` every page asks for through the user-agent
      sheet is a font on this machine rather than two families nobody had
      **And which face of a family a file holds** (queue item 194): the weight
      and the slant come out of the font's `OS/2` table, so `Helvetica-Oblique`
      leans and a semibold is 600 — nothing about a face is read off its
      filename now, which is what items 192 and 194 together mean
      **And in which language that name is read** (queue item 195): a `name`
      table states the same family once per language, so the unlocalised record
      wins where there is one and English wins over every translation — four of
      this machine's fonts were filed under Chinese names until it did
      **And a font that is many weights rather than one** (queue item 196): the
      `wght` axis is read as the range it covers, so one file answers a request
      for 400 and a request for 700 and is shaped, measured and outlined at the
      weight it was set to — twenty-eight of this machine's fonts are such a
      file, its system font among them
      · Owed: the Linux sandbox, queue item 169; and the axes that are not
      weight — width, slant and optical size — which are queue item 197
- [x] A renderer that dies takes its tab and nothing else — and says so, rather
      than leaving a blank rectangle (queue items 166 and 65). It is not
      restarted silently, because that hides a bug somebody needs to see.
      *This line was ticked when item 166 made one renderer's death survivable
      by the others, and the half about the **tab** was not built: nothing here
      was a tab, nothing kept a painted frame, and the blank rectangle the line
      names is exactly what a person would have been shown. Item 65 built it —
      `alo-renderer`'s `tab.rs`, a frame kept per tab, every tab in the dead
      process told and no other, and a new process only when somebody asks for
      the page again*
- [x] The transport, and the lifecycle that starts, reuses and reaps renderers
      (queue items 63, 166 and 64). Length-prefixed messages over pipes; a
      process started per site and reused; a ceiling of sixteen with the least
      recently used evicted; and, since item 64, **reaping** — closing the last
      tab on a site stops that site's process, and closing one of two tabs on a
      site stops nothing.
      *This line was ticked once before, with the two items above it, on a
      reading of "reaps" that the eviction satisfies. It did not: evicting to
      stay under a ceiling is what happens when reaping has **not** happened,
      and it was un-ticked. What was owed is built, which is what a tick means*
      *And since queue item 198, an exchange has a **bound**: a renderer that is
      alive and says nothing for ten seconds is given up on and stopped, where
      before it held the browser process in a read for as long as it lived and
      every other tab with it. A renderer that is merely slow is still waited
      for, which is the half that decides what the bound may be. The number is a
      choice rather than a measurement, and it stands in for a question — wait,
      or stop it? — that needs an interface to ask in*
- [ ] Where one site ends and another begins — the origin, the site, and which
      of them gets a process · Built: the site, as ADR 0005 defines it — scheme
      plus **registrable domain**, decided against the public suffix list
      (`alo-url`'s `site`, queue item 156), and the one answer the cookie jar,
      the cache and the process split all use. The list is a compiled-in
      snapshot and it ages, so its age is recorded and the build fails once it
      is six months old (`alo-url`'s `snapshot`, queue item 186) — a boundary
      that has quietly stopped being current is the one failure here nobody
      would see
      **And which of the three a page is given** (queue item 66): the **origin**
      decides whether there is a site at all. Where it is a tuple the
      registrable domain widens it into a site and two tabs share a process,
      with the port left to the origin because two ports can already reach one
      another. Where it is **opaque** there is no site: a local file, a `data:`
      page, `about:`, a scheme nobody registered — each is rendered in a process
      nothing else is ever put into, where before every local file on the
      machine was one site sharing one address space
      · Owed: what a **document inside a
      document** is given, which nothing here can yet produce — a sandboxed
      `iframe`'s opaque origin and `about:srcdoc` inheriting its parent's (queue
      item 86), and a `blob:` taking the origin of whoever created it (queue
      items 72 and 90)

### The network

- [x] **URLs, properly**: WHATWG parsing, origins, IDNA and punycode. Every security decision below is made against the origin this produces, which is why it is first
      · `alo-url`: parsing rented behind one file, the types ours, and an
      opaque origin that is the same as itself and nothing else
- [x] TLS with `rustls`, and certificate errors a person can act on rather than click through
      · `alo-net` (queue items 51 and 52): the shape a fetch produces, and TLS
      over it. A refusal carries what is wrong, what trusting it anyway would
      mean, and whether the fault has an innocent explanation — three things a
      caller cannot show one of without the others. Verification cannot be
      turned off: no flag, no constructor, no feature
- [ ] HTTP/1.1, then HTTP/2 — connection pooling and keep-alive with them
      · Built: HTTP/1.1 (queue item 53) — a request out, a response in, body
      framing by length, by chunks and by close, and every message that says
      two things about where it ends refused by name. `http:` and `https:`
      fetch over a socket. **Pooling and keep-alive** (queue item 54), with the
      retry that has to come with them: a reuse that fails before a byte
      arrives is tried again, and a request that must not happen twice never is.
      **HTTP/2 framing** (queue item 59) — every length checked before anything
      is reserved, and the padding underflow that is the classic parser bug
      refused by name. **HPACK** (queue item 160), checked against the
      specification's own worked examples, with the Huffman codes derived from
      the canonical structure rather than transcribed. **Streams, flow control
      and the connection state machine** (queue item 161), with the CONTINUATION
      flood refused by a bound on the whole block. **Negotiated by ALPN and
      spoken** (queue item 162) — chosen during the handshake, so no request is
      sent twice to find out which protocol it is. **`Transfer-Encoding` as the
      list it is** (queue item 153) — `gzip, chunked` is chunks holding a gzip
      stream and decodes as one; `chunked` anywhere but last, a coding we
      cannot undo, and a compressed body the connection closing is the only end
      of are each refused by name. **A connection that ended told apart from a
      peer that misbehaved** (queue item 185), so a stream the server gave up
      on keeps the bytes that arrived on it. **A request that sends something**
      (queue item 163) — a body after the blank line in HTTP/1.1 and in `DATA`
      frames in HTTP/2, cut to the frame size the peer allows, with a window
      that closes part way through waited on rather than overrun and the stream
      closed rather than left open when a server answers early; the length a
      request states is always its bytes rather than a header a caller wrote,
      and an interim response is read past rather than taken for the answer ·
      Owed: an `Expect` is refused by name rather than honoured, which needs a
      bounded wait — queue item 187
- [ ] HTTP/3 and QUIC, once those two are correct
- [ ] DNS, and encrypted DNS as a choice somebody made rather than a default
      nobody was told about · Built: ADR 0008 and resolution through the
      machine's own resolver (queue item 58), with DNS rebinding refused — the
      rule turns on who asked, so a person reaches their intranet and a page on
      the web does not · Owed: the encrypted-DNS setting itself, queue item 158,
      which needs an interface to choose in
- [x] Content encodings: gzip, brotli, zstd (queue item 152) — and `deflate` in
      both the spelling the specification asks for and the one servers send.
      All three rented and all three pure Rust, because a decompressor is where
      a memory bug is most directly a remote code execution. The bound is on
      what comes **out**: a bomb is small on the wire by definition
- [x] Redirects, byte ranges, and downloads that resume · Built: redirects
      (queue item 55) — bounded, loop-detecting, `Authorization` dropped at an
      origin boundary, a redirected `POST` demoted to `GET` on 301/302/303 and
      preserved on 307/308, and `file:`/`data:` refused as destinations; byte
      ranges with downloads that resume (queue item 154) — a body that
      stops early keeps its bytes and the rest of them are asked for, with a
      `206` required to begin **exactly** where the download stopped, a `200`
      answering a range request never appended, and nothing encoded ever
      spliced; and **the same over HTTP/2** (queue item 185), where a stream
      that ended early used to be an error that took the bytes with it. The
      loop is protocol-blind and lives in `download::whole_of`, which is why
      that was a change to the HTTP/2 client and to nothing else
- [x] **The HTTP cache, with real semantics** — freshness, revalidation, `Vary`
      (queue item 56). Nothing in it reads the clock, because the answers that
      matter are the ones only wrong an hour later. `Age` is counted, so a
      response that arrives old expires early; `Vary` is stored as a contract,
      so a French page is never served to a German reader; `Vary: *` is not
      stored at all · Built: the decision about a disk (ADR 0011) and the code
      that keeps it (queue item 155) — the key carries the top-level site, so a
      shared cache is no longer a history oracle or an identifier that survives
      clearing cookies; what must not outlive the session is never written
      rather than written and deleted, because a deleted file was still on the
      disk; and what comes back off the disk is read as what it is, which is
      bytes from outside, with anything unreadable a miss rather than a page
      that will not open
- [x] **Cookies**: `SameSite`, `Secure`, `HttpOnly`, partitioned by default
      (ADR 0007, queue item 57). The default is a product decision and the ADR
      argues it — who it protects, and what it costs. There is no way to ask for
      cookies without a partition. The partition is the **registrable domain**
      since queue item 156, so a person signed in at `example.com` is signed in
      at `www.example.com` and `Domain=co.uk` is refused · Owed: the escape
      hatch a person grants per-site (queue item 157)
- [x] The same-origin policy, CORS and preflight (queue item 61) — code we write
      and can get wrong, which is one of ADR 0005's four reasons for the
      sandbox. A page may send almost anywhere and read almost nowhere; a
      wildcard never covers a request that carried credentials; a `Set-Cookie`
      is honoured and unreadable. **The preflight cache** (queue item 164): a
      second request of the same shape sends no `OPTIONS` and one of a different
      shape still does, partitioned by top-level site, expiring on the caller's
      clock and capped at two hours however long a server asked for — because a
      permission nobody can revoke is not one. A `*` is remembered as what it
      allowed rather than as a standing offer. Building it found the safelist
      being applied by header *name* where it is a rule about the value too, so
      a JSON post was preflighted with a question that never named
      `Content-Type`
- [ ] Content Security Policy, referrer policy, HSTS, mixed-content blocking
      · Built: HSTS, mixed content and referrer policy (queue item 62) — a
      `Strict-Transport-Security` over plain HTTP ignored so it cannot be used
      as a weapon, a script refused outright where an image is retried over TLS,
      and a referrer that never survives a downgrade. **CSP is enforced** (queue
      item 165): `default-src`, `script-src`, `style-src`, `img-src` and
      `connect-src`, the source expressions pages use, nonces and
      `'strict-dynamic'` — and the rule the whole thing is built around, that a
      word this engine cannot read is kept and matches nothing rather than
      taking its directive down with it. A repeated directive keeps the first
      and two policies are an intersection, so nobody widens a policy by
      appending to it. **A violation is reported** (queue item 188):
      `report-uri` and `report-to` both, an enforced policy and a watched one
      alike, in the two documents collectors read — and a report says a
      cross-origin URL as its origin and nothing more, because a report is
      posted to a server the page chose and would otherwise be a way to read
      one. **A content hash is computed** (queue item 189): a policy that names
      the digest of its own inline stylesheet allows that stylesheet and refuses
      every other one, in either base64 alphabet, with a value that mixes the
      alphabets or is the wrong length allowing nothing — a hash source is a
      permission, so reading one loosely is a policy wider than its author wrote.
      **A `style` attribute can be allowed by its digest** (queue item 191),
      where the deciding directive also says `'unsafe-hashes'` — the keyword
      grants nothing on its own, does not reach out of the directive it was
      written in, and a policy without it refuses an attribute whose digest it
      names, because content with no element of its own is the shape an
      injection takes
      · Owed: **a nested document**, which is what `frame-src` needs and which
      nothing here can yet tell from a link click (queue item 86); and an
      **event handler** matched by its hash, which is the same rule as the
      `style` attribute and waits only for there to be handlers (queue item 81)
- [ ] `fetch()` and `XMLHttpRequest`, over the same stack rather than beside it
- [ ] WebSocket
- [ ] ★ **Every request attributable** — which page, and which agent action, caused it. No other engine has needed to answer that, and an agent-driven browser that cannot is one nobody should trust

### JavaScript, ours, in Rust

- [ ] Lexer and parser to an AST — the current language, not ES5
- [ ] A bytecode compiler and an interpreter. **Correct first; a JIT much later or never**
- [ ] A garbage collector, and the object model underneath it
- [ ] The standard library: the ECMAScript builtins, in the order real pages need them
- [ ] Regular expressions, with the syntax the language actually has
- [ ] Promises, the microtask queue, `async`/`await`, generators and iterators
- [ ] Modules: ESM, dynamic `import()`, and the loader that fetches them
- [ ] **The event loop** — tasks, microtasks, the rendering steps, `requestAnimationFrame`. Where "it works, but the animation stutters" is decided
- [ ] Errors and stack traces good enough to debug somebody else's minified page
- [ ] Internationalisation (`Intl`), rented rather than written
- [ ] Refused for now and recorded: a JIT, until there is a measured reason and an ADR weighing it against the attack surface it adds

### The DOM, and the pages that use it

- [ ] Mutation from script — create, append, remove, replace — and the invalidation that has to follow it
- [ ] **Events**: capture and bubble, listeners, default actions
- [ ] **Forms**: the controls, constraint validation, submission, file inputs
      · Built: **a control draws its own state** (queue item 182) — a tick in a
      checked box, a dot in a chosen radio, a dash in one that is neither, in
      `accent-color` if the page names one; and a control nobody can operate
      says so *while still saying what state it is in*. Corpus case
      `control-states`. **A group of controls looks like a group** (queue item
      183) — a fieldset draws its border, and its legend sits *in* that border
      rather than above it, so the border is drawn in the two pieces the legend
      leaves. Corpus case `fieldset-group` · Owed: everything a control **does**,
      which needs events
      (queue item 81) — constraint validation, submission, file inputs — and the
      focus ring, which needs something to have focus (queue item 43)
- [ ] **Navigation and session history**: `pushState`, back and forward, and what survives each
- [ ] `iframe`s and the sandbox attribute — a document inside a document, where a great many security bugs live
- [ ] Shadow DOM and custom elements; component frameworks are not optional on the modern web
- [ ] Selection and ranges
- [ ] CSSOM — styles readable and writable from script
- [ ] Storage: `localStorage`, `sessionStorage`, IndexedDB, the Cache API, and one quota policy over all of them
- [ ] Workers: dedicated, shared, and service workers with their fetch interception
- [ ] Timers, clipboard, drag and drop
- [ ] ★ **Permissions as capabilities** — camera, microphone, location, notifications, in the shape of `alo-os` ADR 0001: enumerated, visible, revocable, expiring, recorded. A browser is where most people meet a permission prompt, and every other one is a dialogue nobody can audit afterwards

### CSS beyond what alo needed

- [ ] Animations and transitions
- [ ] Container queries, `:has()`, cascade layers, `@property`
- [ ] Filters, `backdrop-filter`, blend modes, masks, `clip-path`
- [ ] `position: sticky`, multi-column, scroll snap, overscroll behaviour
- [ ] Writing modes, and layout that is right-to-left rather than mirrored afterwards
- [ ] Paged media and print styles

### Text, properly

- [ ] Bidirectional text end to end — stage 1 shapes it; this makes selection, caret movement and editing behave
- [ ] Selection, carets and text input inside the page
- [ ] **Input methods.** A browser that cannot take Japanese or Chinese input is not a browser in those countries
- [ ] `contenteditable`, which every rich text box on the web is built on
- [ ] Hyphenation, `text-wrap: balance`
- [ ] Web fonts as pages ship them: WOFF2, variable fonts, and loading that does not flash

### Pictures, and things that move

- [ ] Image codecs, rented: PNG, JPEG, GIF, WebP, AVIF
- [ ] **SVG** — a second rendering model inside the first, and far larger than its one line here suggests
- [ ] Canvas 2D
- [ ] Audio and video playback through rented decoders
- [ ] Media Source Extensions, without which most video sites do not play at all
- [ ] Web Audio
- [ ] WebGL, then WebGPU — both large, both late, and neither before the software path is right

### Making it fast enough to use

Correctness before speed is the rule everywhere else in this file. These are the
items where being right and being unusable are the same outcome.

- [ ] **Incremental style and layout** — recompute what changed, not the document. The largest single difference between an engine that renders a page and one somebody can use
- [ ] Compositing layers, and scrolling that does not repaint the world
- [ ] Off-main-thread scrolling and animation
- [ ] Hardware acceleration for paint, once the software path is correct
- [ ] A performance budget somebody can hold us to: named pages, measured, in CI

### The browser itself

- [ ] A window, tabs, and a tab strip
- [ ] The address bar: what somebody typed, what it means, and a search that phones nobody by default
- [ ] History, bookmarks, downloads
- [ ] Find in page, zoom, and per-site settings that stick
- [ ] Context menus, and keyboard operation of every one of them
- [ ] Printing, print preview, export to PDF
- [ ] Viewing a PDF — or saying plainly that we hand it to something else
- [ ] Private browsing, and profiles that are genuinely separate
- [ ] Autofill, and credentials held where the operating system holds secrets rather than in a file of ours
- [ ] Security surfaces: certificate detail, permission state, what this page has stored — reachable, none of it buried
- [ ] Settings
- [ ] **Developer tools**: inspector, console, network, performance. A browser nobody can debug a site with is not one a developer keeps
- [ ] **Accessibility**: AT-SPI over the same tree the agent reads (ADR 0002), keyboard operation of everything, focus always visible, and the EN 301 549 conformance the workspace is already held to

### ★ The agent, on somebody else's pages

- [ ] The agent reads and acts on ordinary web pages through the same tree — no screenshot, no scraping, no coordinates
- [ ] Across frames, without becoming a way around the same-origin policy
- [ ] Under grants, and recorded: what it read, what it did, on whose approval — `alo-os` ADR 0001's model reaching the web
- [ ] Agent-driven navigation, and a page that changes underneath an agent mid-action

**Exit gate.** A person uses it as their browser for a week and reaches for
another one only for a site they can name. An agent completes a real task on a
site nobody wrote for us, and the record afterwards says what it read and what
it changed.

---

## Stage 3 — the legacy tail

Deliberately last, and possibly never finished — a choice, not a failure.
Refusing this list is what made stages 1 and 2 survivable. Keep a corpus of
sites people actually use and let a broken render schedule the work.

- [ ] Quirks mode
- [ ] **Floats as layout**, and CSS table layout — the two that most often turn an old page into a column of rubble
- [ ] `document.write`, live `HTMLCollection`s, and the DOM as it was before it was a specification
- [ ] Legacy character encodings, and detecting them
- [ ] XML, XHTML and XSLT
- [ ] `frameset`
- [ ] Vendor prefixes, and anything that exists only for a page written before 2015
- [ ] The sloppy-mode corners of JavaScript that only old code reaches

---

## Stage 4 — a browser somebody chooses

Product work, and gated: **nothing here starts until stage 2's exit gate is
met.** It is listed rather than left unnamed because "not scheduled" had begun
doing the work of "not thought about", and these decide whether anybody switches.

- [ ] Extensions — and the decision, in an ADR, whether that means the WebExtensions API or something narrower we can actually secure
- [ ] Sync, self-hosted: bookmarks, history, tabs and passwords, end-to-end encrypted, on the customer's own server
- [ ] Updates that are signed, staged and reversible
- [ ] A mobile port
- [ ] Crash handling that helps us fix it without becoming telemetry
- [ ] ★ **Translation on the machine.** alo already runs models locally; a page translated without sending it anywhere is the sovereign version of a feature every other browser sends to a server
- [ ] ★ **Reading and summarising a page locally**, under the same grants and the same record
- [ ] Enterprise: policy, managed configuration, and an update mirror an organisation hosts

**Exit gate.** Somebody outside alo chooses this browser, on a machine we did not
set up, and stays.

---

## Not built, and not by accident

Stated here so that no later stage quietly adopts them:

- **DRM and Encrypted Media Extensions.** A proprietary binary with privileges inside a sovereignty product is a contradiction. Sites that require it will not play, and we say so rather than shipping a black box.
- **Proprietary codecs** we cannot ship freely.
- **Telemetry.** Not "anonymised telemetry". None — the rule alo OS already holds.
- **A search deal.** The address bar's default is decided for the person using it, not sold.
- **Our own shaper, codec or TLS stack.** We rent the physics, as every engine does.
- **A conformance percentage as a target.** The measure is alo, then real pages that fail.
