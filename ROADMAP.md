# alo browser — ROADMAP.md

The only order things get built in. **A tick means done, not written** — where
something is built and tested but has not met the gate on a real screen, the item
says so rather than being ticked optimistically.

Every item appears in `docs/features.md`. If it is not there, it is not built.

---

## Stage 1 — it renders alo

No scripting, no hostile pages, no compatibility burden. The point is that
something real depends on this within months rather than years: when alo OS's
shell renders through it, every later stage has a working thing to grow from
instead of a demo.

**Order.** Nothing here needs a GPU, a window or a network. A software raster to
a PNG is deterministic and diffable, which makes every step testable from the
first one — the display server and hardware acceleration come after correctness,
not before it.

- [ ] **A DOM of our own**, built from `html5ever`'s parse events
- [ ] **Stylesheets**: `cssparser` into rules we hold, selectors matched with `selectors`
- [ ] **Computed style**: the cascade, inheritance, and `var()` — alo's design system is custom properties throughout, so this is not optional decoration
- [ ] **The box tree**, and what each box *means* (ADR 0002) rather than only its rectangle
- [ ] **Layout**: flexbox and grid, on `taffy` to begin with, behind our own boundary
- [ ] **Text**: HarfBuzz shaping and font rasterisation, with the awkward scripts working before the easy ones
- [ ] **Paint**: a display list, then a software raster to a PNG
- [ ] **Reference renders**: a committed corpus, diffed on every change
- [ ] ★ **The agent tree**: the layout tree read as roles, states and positions
- [ ] ★ **Typed verbs**: activate, type, scroll — and never a coordinate
- [ ] **A real alo screen renders correctly** — the sign-in screen, then Settings
- [ ] Hardware acceleration, once the software path is right
- [ ] Embedding: alo OS's shell renders through it

**Exit gate.** alo OS's sign-in screen and Settings render through this engine on
the certified machine, correctly and without stutter, and an agent reads the
Settings screen as a tree and activates a row by name rather than by position.

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
