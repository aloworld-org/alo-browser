# CLAUDE.md — the alo browser constitution

alo browser is a browser. It begins by rendering alo — the alo OS shell
and the alo workspace — because a project of this size is only ever
finished if something real depends on it from the first month. It is
written in Rust, it is memory-safe by construction, and it is built so
that an agent can read and drive an interface rather than photograph
it.

Everything here is absolute; everything else is judgment.

## The four laws

1. **We render the modern platform, not thirty years of it.** Flexbox,
   grid, custom properties, transforms, text, compositing. No quirks
   mode, no floats-as-layout, no CSS-table layout, no `document.write`,
   no legacy DOM surface. Chromium is mostly compatibility with broken
   pages; that is the part we are not building, and the discipline of
   refusing it is what makes this possible at all.
2. **The layout tree is the agent's tree.** An agent reads what the
   interface *is* — "invoice list, twelve rows, row three selected" —
   never a screenshot it has to interpret. This is designed in from the
   first commit, because bolted on later it is a plugin, and being a
   plugin is what every other AI browser already is.
3. **Correct before fast.** A wrong pixel is a bug; a slow one is a
   task. Optimising something whose behaviour is not yet settled buys
   speed we then cannot change.
4. **One language: Rust.** `unsafe` is forbidden outside a reviewed,
   named boundary with a written reason. A memory-safe engine is half
   the argument for building this at all, and a repository that leaks
   `unsafe` has spent that argument.

## Standing rules

- **Stage by what it renders**, never by specification completeness.
  Stage 1 renders alo — markup we write, no hostile pages, no
  compatibility burden. Stage 2 the modern web. Stage 3, if ever, the
  legacy tail, scheduled by real pages failing rather than by a
  specification listing a property.
- **Rent the physics, build the engine.** Text shaping (HarfBuzz),
  font rasterisation, image and video codecs, TLS: rented. Nobody
  writes their own shaper, and not because they are afraid of work.
  Layout, style, the DOM, the process model and the agent surface are
  ours.
- **Prefer Rust prior art where it is aligned.** `html5ever`,
  `cssparser`, `selectors` parse to specification and carry none of our
  value. Taking them is not a shortcut; taking Google's C++ JavaScript
  engine would have been, which is why we are not.
- **No JavaScript engine in stage 1.** alo's own interface does not
  need one, and it is the largest single component in a browser. When
  stage 2 needs it, it is ours and it is Rust: a correct interpreter
  first, a JIT much later or never.
- **The measure is alo, not a conformance score.** "Does alo render
  correctly and fast" is a question this repository can answer every
  day. A Web Platform Tests percentage scores us against the legacy we
  deliberately refuse.
- **Every rendering decision is testable.** A layout is a tree with
  numbers in it; assert on the numbers, not on a screenshot somebody
  eyeballed.
- **One file, one responsibility.** A file that gains a second reason
  to change gets split in the change that discovered it.
- **Settled decisions live in `docs/decisions/`.** Read the ADR before
  proposing an alternative.
- **Names are for strangers.** Conventional commit subjects,
  `type(scope): descriptive subject`.

## The gate — nothing is done until all of this passes

- `cargo fmt` clean, `cargo clippy` with zero warnings **and** zero
  errors.
- Unit tests for logic, and a **layout assertion** for anything that
  positions or sizes: the computed box, in numbers.
- **A reference render** for anything visual — a small deterministic
  raster compared against a committed reference, so a change that moves
  a pixel says so.
- Documentation in the same change, and a `CHANGELOG.md` line.
- **No `unsafe` without an ADR.**

**And no rushing.** A date never justifies a shortcut. When something
has to give it is scope — fewer properties, fewer selectors — never
depth. A renderer that is nearly right is a renderer nobody can debug.

## Map

- `README.md` — what this is, and what it is not.
- `docs/features.md` — the only list of what gets built.
- `ROADMAP.md` — the stages, and the exit gate for each.
- `docs/decisions/` — why things are as they are.
- `docs/autonomy/` — the build loop: `LOOP.md`, `QUEUE.md`, `STATE.md`.
- `docs/conformance.md` — what renders correctly today, honestly.
