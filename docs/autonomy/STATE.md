# Journal

One entry per loop iteration, newest last. What was built, what the gate said,
and anything the next iteration should know before it starts.

`LOOP COMPLETE` and `LOOP HALT` are read from this file by the supervisor, so
they appear only when they are true.

---

## 2026-09-02 — before the first iteration

Nothing is built. The repository holds its constitution, two decisions and a
queue.

What the first iteration should know:

- **Read `docs/decisions/0002` before touching layout.** It says the layout tree
  *is* the agent's tree, and that constrains the shape of the box tree from the
  first commit. Retrofitting it means rewriting layout.
- **Item 3 is not optional decoration.** `alo-workplace`'s design system is
  custom properties throughout, so an engine that cannot resolve `var()` renders
  nothing of alo at all — which is the whole target of stage 1.
- **Assert numbers, not images.** A layout test says where the box is. Reference
  renders exist as well, but a failure reading "row three moved 4px" is worth ten
  reading "the image differs".
- **The output is a PNG from a software rasteriser**, deliberately: it needs no
  GPU and no window, it is deterministic, and it makes every visual change
  reviewable as a diff. Hardware acceleration comes after correctness.
- **Sibling repositories worth reading, never editing:** `alo-os` for the shell
  that will embed this and for its verb contract, and `alo-workplace` for
  `web/src/ds/tokens.css` — which is the specification for what "correct colour"
  means here.

---

## 2026-09-02 — iteration 1: a DOM of our own (queue item 1)

**Before the item.** `docs/features.md` was already written and committed
(`e95277f`), and it covers every stage 1 line in `ROADMAP.md` with a tier — so
`LOOP.md` step 2 can do its job and the first item was ready to build. Nothing
was needed there.

**What was built.** The workspace, and `crates/alo-dom`: the document tree.

- `name.rs` — `Namespace` and `QualifiedName`, ours rather than the parser's.
  An unrecognised namespace URI is kept verbatim, not dropped.
- `node.rs` — `NodeId`, `Node`, `NodeKind`, `Element`, `Attribute`.
- `document.rs` — the arena that owns every node and the only thing that may
  change the links between them. Building it is `pub(crate)`: `features.md`
  puts DOM mutation in stage 2, and this makes that a compile error rather than
  a thing to remember.
- `parse.rs` — **the only module that names `html5ever`.** The `TreeSink`, and
  `parse_document` / `parse_fragment`. Names convert once, at this boundary.
- `serialize.rs` — the tree back to HTML, ours, following the modern fragment
  serialisation algorithm and none of the legacy of it.

**Decisions worth knowing about, both written down:**

- **ADR 0003** — node identity is allocated once and never reused. A detached
  node keeps its id and its slot is not freed, so a stale id names something
  that is gone rather than something else that is not. ADR 0002's agent surface
  stands on this, which is why it is in the first commit of the tree.
- **Quirks mode is recorded and never honoured.** `html5ever` reports it; we
  keep it in `Document::quirks_signal` for diagnostics and lay out every
  document as standards. A test asserts the two trees are the same tree.
- **Declarative shadow roots are refused** at the sink rather than half-built.
  They are not in `features.md`, and a parser told "yes" by a sink that cannot
  attach one leaves a tree claiming something happened that did not.

**The gate.** `cargo fmt` clean. `cargo clippy --all-targets -- -D warnings`
with zero warnings and zero errors — and the workspace lints go past the gate
deliberately: `pedantic`, plus `unsafe_code = "forbid"`, `unwrap_used`,
`expect_used`, `panic`, `indexing_slicing`, `todo`, `unimplemented`. Law 4 is
now enforced by the compiler instead of by review. `clippy.toml` permits the
panic family inside tests, which is exactly where `CLAUDE.md` permits it. 59
tests pass: 33 unit, 25 integration across `round_trip.rs` and `malformed.rs`,
one doctest.

**What the gate did not ask for here, and why.** No layout assertion: the DOM
neither positions nor sizes anything, and there is no number to assert yet. No
reference render: nothing here is visual. Both arrive with items 5 and 7, and
neither has been skipped — there was nothing to skip.

**What the next iteration should know.**

- `parse.rs` is the boundary. If item 2 finds itself importing `html5ever`
  outside it, that is the design going wrong, not a convenience.
- `Document` mutation is `pub(crate)`. A styled tree, a box tree and the agent
  tree are all *views* over this one — ADR 0002 is explicit that a second
  structure is the failure mode — so item 4 should build boxes that refer to
  `NodeId`s rather than copies of nodes.
- The serialiser is only as good as its round trip. If item 2 or 3 adds
  anything the tree can hold, add the case to `tests/round_trip.rs` in the same
  change, or the next person will find it missing by accident.
- Nothing renders yet. `docs/conformance.md` is still honest.

---

## 2026-09-02 — the gate, made runnable

Not a queue item; the owner asked that the rules be enforced rather than
remembered, and `CLAUDE.md` already said what they are.

`scripts/gate.sh` runs the mechanical half of the gate and exits non-zero:
formatter, linter with zero warnings and zero errors, tests, no stubs, no crate
that has quietly opted out of the workspace ban on `unsafe`, a `CHANGELOG.md`
line whenever a crate changed, and — the one that is specific to this
repository — **every rented crate still named only in the files allowed to name
it**. That last check strips comment lines first, because explaining a boundary
is better than not, and it matches a path (`cssparser::`, `use cssparser`)
rather than a word, so a field called `selectors` is not a violation.

What no script can check, it prints rather than dropping: one file one
responsibility, a layout assertion in numbers, a reference render, an item's
section in `docs/features.md`, and that a tick means done. A green run is not a
passed gate, and the script says so.

---

## 2026-09-02 — iteration 2: stylesheets (queue item 2)

**What was built.** `crates/alo-css`. `cssparser` tokenises, `selectors`
matches, and the rules are ours.

- `ident.rs` — one string type for names, namespaces, classes and attribute
  values, with its hash computed once because matching asks for it in its
  innermost loop.
- `issue.rs` — what a sheet asked for that we did not do, with the text and the
  line. The same bargain `alo_dom::ParseIssue` makes for HTML.
- `declaration.rs` — property name, value **as written**, and importance.
  Custom properties are their own case because their names are case-sensitive
  and alo's design system is built from them.
- `selector.rs` — the `SelectorImpl`: which pseudo-classes and
  pseudo-elements exist here, and specificity unpacked into the three counts
  the cascade compares.
- `state.rs` — what HTML says about an element: `:disabled`, `:checked`,
  `:required`, `:read-write`. Written out rather than approximated, including a
  disabled `<fieldset>` reaching its controls but not its first `<legend>`.
- `matching.rs` — the `selectors::Element` adapter over `alo_dom`. The whole of
  the coupling between CSS and the DOM, in one file, so that the box tree has
  one place to be added to.
- `media.rs` — width and `prefers-color-scheme`, evaluated; anything else
  recorded as not understood and treated as not matching.
- `parse.rs` — the rule and declaration parsers.
- `stylesheet.rs` — the rules, in order, and `style_rules_for(device)` which
  flattens matching `@media` blocks in where they were written.

**Decisions worth knowing about:**

- **The boundary here is the public API, not one file.** `alo-dom` keeps
  `html5ever` in one file because an HTML parser is used once. A CSS tokeniser
  is not: selector text, media conditions and declaration values are read from
  the same token stream, and faking a single-file boundary would have meant
  copying strings about to keep something cosmetic. So no `cssparser` or
  `selectors` type appears in `alo-css`'s public API, and the files allowed to
  name them are listed in `scripts/gate.sh` and checked on every run.
- **`:visited` never matches, permanently.** Visitedness is history and a style
  that depends on it is readable back off the page. Every engine reached this;
  we start there.
- **The interaction states parse and never match.** There is no input in stage
  1. A sheet mentioning `:hover` must not be thrown away, and pretending to
  answer would be worse than saying nothing.
- **A pseudo-element selector is kept and never matches**, and the sheet is
  told at parse time. Stage 1 produces no box for one.
- **An unknown media condition fails closed.** Applying rules whose condition
  is unknown is how a dark theme leaks into a light one.

**The gate.** `scripts/gate.sh` green: `cargo fmt` clean, `cargo clippy
--workspace --all-targets -D warnings` with zero warnings and zero errors, 103
tests (86 unit, 18 integration across `stylesheet.rs` and
`against_a_document.rs`, one doctest), no stubs, boundaries held. No layout
assertion and no reference render: nothing here positions, sizes or draws — the
first numbers arrive with layout, and the first pixels with paint.

**Three bugs this iteration found by testing rather than by reading**, all in
the first draft and all fixed: `screen and (min-width: 600px)` did not parse
because the `and` after a media type was never consumed; `rgb(1, 2, 3)` was
recorded as the value `rgb(` because stepping over a function token leaves the
parser's position inside the block rather than after it; and
`most_specific_match` returned the *least* specific selector. The third is the
one worth remembering — it passed every unit test in `selector.rs` and was
caught only by the end-to-end test against a document.

**What the next iteration should know.** Item 3 is the cascade, and it is the
one `docs/decisions/0001` calls stage 1's first hard requirement.

- Everything it needs is already here: `style_rules_for(device)` gives the
  rules that apply in document order, `most_specific_match` gives the
  specificity of the selector that actually matched, and `Importance` is
  recorded separately from the value.
- **Declaration values are unparsed source text.** That is deliberate — it is
  what lets an unknown property be kept — and it means item 3 re-tokenises a
  value when it resolves `var()`. `cssparser` is already a dependency of
  `alo-css`; if the cascade lands in its own crate, add it to `scripts/gate.sh`'s
  boundary list in the same change.
- `DeclarationBlock::get` already answers "the last declaration of this
  property wins within a block". The cascade is what decides between blocks.
