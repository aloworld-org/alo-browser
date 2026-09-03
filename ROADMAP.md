# alo browser — ROADMAP.md

The only order things get built in. **A tick means done, not written** — where
something is built and tested but has not met the gate on a real screen, the item
says so rather than being ticked optimistically.

Every item appears in `docs/features.md`. If it is not there, it is not built.

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
- [x] ★ **Typed verbs**: activate, type, scroll — and never a coordinate
- [ ] **A real alo screen renders correctly** — the sign-in screen, then
      Settings. *`alo-workplace`'s sign-in screen renders and is diffed on every
      run. Not ticked for two true reasons: Settings is not rendered at all, and
      the sign-in case carries four substitutions — `clamp()` and viewport
      units, `white-space: pre-line`, `letter-spacing`, transitions — each
      naming something this engine does not implement, so what is diffed is a
      modified screen. The old reason, that the gate named alo OS's screens, was
      not a fact about this engine and is gone.*

**Exit gate.** From this repository alone, on any machine: alo's sign-in screen
and Settings render correctly from their own markup and `tokens.css`, with no
substitutions left in the case — matched against the committed reference image
*and* the expected box tree, so "correctly" is a diff rather than an opinion —
and an agent reads the Settings screen as a tree and activates a row by name
rather than by position.

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

- [ ] **The process and sandbox model — designed before the first hostile page is loaded.** One process per site, renderers with almost no privilege. Every browser that retrofitted this suffered for years, and it is the one thing here that cannot be added later
- [ ] Network: HTTP, `rustls`, caching, cookies with sane defaults
- [ ] **A JavaScript engine, ours, in Rust** — a correct interpreter first; a JIT much later or never
- [ ] The DOM APIs a modern page actually uses, driven by pages that fail
- [ ] Images, media, canvas
- [ ] Chrome: tabs, address bar, history, downloads
- [ ] ★ The agent reads and acts on ordinary web pages, through the same tree

**Exit gate.** A person uses it as their browser for a week and reaches for
another one only for a site they can name.

---

## Stage 3 — the rest

The legacy tail, and it is deliberately last. Keep a corpus of sites people
actually use; let a broken render schedule the work. Nothing here is built
because a specification lists it.

---

## Not scheduled

Extensions. Sync. A mobile port. Anything that is a browser *product* feature
rather than a rendering one — until stage 2's exit gate is met, they are
distractions with a good story attached.
