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

---

## 2026-09-02 — iteration 3: computed style (queue item 3)

**What was built.** `crates/alo-style`. The cascade, inheritance and `var()`.

- `origin.rs` — user agent and author, and the level each importance gives
  them. `!important` reverses the origins, which is the part worth a test:
  an engine that could be shouted down by a page could not insist on anything.
- `inheritance.rs` — the table of which properties inherit, because CSS is a
  table and there is no rule that derives it. A property not in the table does
  not inherit; a custom property always does.
- `keyword.rs` — `inherit`, `initial`, `unset`, `revert`, in one place, because
  handling them per property is how three of the four end up subtly different.
- `variables.rs` — custom property resolution and `var()` substitution, with
  cycles refused.
- `cascade.rs` — which declaration wins, and nothing else.
- `computed.rs` — the whole document, in document order, because a child's
  `var(--surface)` resolves against the map its parent ended up with.

**Decisions worth knowing about:**

- **A property's initial value is its absence.** A computed style holds only
  what was set, and whoever reads it knows the initial value for the property
  it asked about. CSS says "nobody set this" and "somebody set this to its
  initial value" are the same state, so this engine carries no table of initial
  values that would have to be kept right in a second place.
- **Specified values, as text.** `16px` is four characters. Turning text into
  numbers is now **queue item 12** — the scope cut this iteration made, written
  into the queue rather than left implied, and given its line in
  `docs/features.md` so the thing that promised it exists. It belongs with the
  code that knows which unit each property wants: layout will parse a length
  from `width`, paint a colour from `color`, and a general answer is one that
  has to be wrong somewhere.
- **Substitution is textual over the token stream**, so the value between
  `var()` calls comes through exactly as written and `calc(var(--gap) * 2)`
  becomes `calc(8px * 2)`. A value this engine does not yet understand is one it
  can still pass along intact.
- **A cycle refuses the whole ring**, and a property in a ring keeps neither its
  own value nor the one it inherited — which is what CSS says, and is not what
  a naive implementation does.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 230 tests (186 unit, 43 integration, one doctest), no stubs, boundaries
held — `cssparser` is now also named in `alo-style/src/variables.rs`, which is
in the list. Still no layout assertion and no reference render: nothing here
positions, sizes or draws. Items 4 and 5 are where the first numbers appear,
and item 7 the first pixels.

**Two bugs worth remembering**, both found by tests rather than by reading:

- `margin: inherit` did nothing, because the child's style was seeded with only
  the *inherited* half of its parent's and `margin` is exactly a property that
  would not be in it. `inherit` needs the parent's **whole** style. The fix is
  small; the class of bug is not, and item 4 will have the same shape when a box
  asks its parent something.
- Substitution lost the whitespace before every `var()`, because a parser's
  position before `next()` is the token's start only if nothing is skipped —
  and `next()` skips whitespace. Reading with
  `next_including_whitespace_and_comments` is what makes the text between
  substitutions come out as written.

**What the next iteration should know.** Item 4 is the box tree, and ADR 0002
is the one to read first: it says the box tree keeps what a box *means*, not
only its rectangle, and that it cannot be retrofitted.

- Everything a box needs is now available per element: `StyleTree::get(id)`
  gives the computed style, `ComputedStyle::get("display")` the text of a
  property, and absence means initial.
- **There is no user-agent style sheet yet**, and item 4 is where one becomes
  necessary: `display: block` on a `<div>` has to come from somewhere.
  `Origin::UserAgent` already exists and is already ordered correctly, so that
  sheet is a string and a `SourcedSheet`, not a mechanism.
- Item 12 (computed values) is not a prerequisite for item 4. It is for item 5,
  which needs lengths as numbers to hand to `taffy`. Doing 12 before 5 is
  probably right; the queue leaves it where it is so that the decision is made
  by whoever reaches it.

---

## 2026-09-02 — iteration 4: the box tree (queue item 4)

**What was built.** `crates/alo-box`, and the engine's own style sheet in
`alo-style/src/user_agent.rs`.

- `display.rs` — `display` parsed into the three separate questions it actually
  asks: does this make a box, does it sit in a line, how are its children
  arranged. The modern two-value syntax is what it parses into, because that is
  what the single keywords are shorthands for.
- `role.rs` — what a box is. **Declared, never inferred**: the `role` attribute
  or what HTML says the element is, and nothing else. A role we do not know is
  kept as written.
- `state.rs` — what is true of a box. `aria-*` first because it is the more
  explicit declaration, then the HTML state, which is asked of
  `alo_css::state` rather than re-derived — two implementations of "is this
  disabled" would eventually disagree.
- `semantics.rs` — role, state and the declared name in one place, on the box.
- `tree.rs` — box generation, anonymous boxes, and the outline a test asserts.

**Decisions worth knowing about:**

- **The engine's style sheet is CSS text.** It goes through the same parser and
  the same cascade as an author's, because a user-agent sheet that took a
  private path would be a second implementation of the cascade, and the second
  one is the one that is wrong. A test asserts the engine's own sheet contains
  nothing the engine refuses.
- **Whitespace is decided about in `arrange`, not when the text box is made.**
  Whether a space is content depends on what is beside it: between two `<p>`s
  it is nothing, between two `<a>`s it is the gap between two words. The first
  draft dropped all whitespace-only text and would have rendered `AllDue`; the
  integration test caught it.
- **No geometry.** Not one number in the crate. Where a box ends up is item 5,
  and mixing the two would make a box's meaning depend on where it landed —
  which is the failure this whole ordering exists to prevent.
- **`States` keeps `Option<bool>` where a state may not apply.** `expanded:
  None` is "this is not a thing that opens", which is not `Some(false)`, "this
  opens and is closed". An agent told the second can act on it.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 299 tests, no stubs, boundaries held. One `#[expect]` was added, on
`clippy::struct_excessive_bools` over `States`, with the reason written beside
it: the lint reads many `bool`s as a state machine wanting an enum, and these
are independently-true flags — a box can be disabled and required and busy at
once. That is a pedantic lint above the gate misreading the design, not a real
finding being silenced.

Still no layout assertion and no reference render, and this is the last item
that can honestly say so: item 5 is where the first numbers appear.

**The scope cut**, written into the queue as item 13: a block-level box inside
an inline one is treated as making the inline box a block container, rather
than splitting it in three the way CSS says. Every tree that meets one records
`IssueKind::UnsupportedStructure`, so a real page hitting it will say so rather
than being found by reading. The common case — wrapping runs of inline children
in anonymous blocks — is done properly.

**What the next iteration should know.** Item 5 is layout, on `taffy`, behind
our own boundary.

- `BoxTree` is what layout walks. `BoxKind::inside()` says flow, flow-root,
  flex or grid; `outside()` says block or inline. Every container's children
  are already all of one kind, which is what the anonymous boxes are for — so
  layout never has to ask "is this a mix".
- **One file may name `taffy`**, as `alo-os` does with its runtime, and
  `scripts/gate.sh` will check it. Add the entry to `BOUNDARIES` in the same
  change that adds the dependency.
- **Item 12 first, probably.** `taffy` wants lengths as numbers and a computed
  style holds `"16px"` as text. Doing 12 before 5 means layout reads numbers
  instead of parsing strings twice; the queue leaves the order to whoever gets
  there, but this is the reason to consider it.
- Text is a box with a string in it and no size. Measuring it is item 6, and
  `taffy` takes a measure function for exactly that — so item 5 can leave a
  seam there rather than a stub, and should say which.

---

## 2026-09-02 — iteration 5: lengths as numbers (queue item 12)

**The queue was reordered before this item, and the reason is written into it.**
Item 5 is layout on `taffy`, and `taffy` wants numbers where a computed style
holds `"16px"`. Building item 5 first would have meant parsing lengths inside
the layout crate, and then item 14 building a second value parser for colours.
One value layer, used by layout and by paint, is the shape that avoids that — so
item 12 moved ahead of item 5, and item 12's own colour half became item 14.

**What was built.** `crates/alo-value`, and the font resolution in
`alo-style/src/metrics.rs` that gives it something to be relative to.

- `unit.rs` — every unit CSS has that does not need a window. `vw` and `vh` are
  deliberately absent: they are relative to a viewport, and a viewport belongs
  to layout.
- `length.rs` — `Length`, `LengthPercentage` and `FontMetrics`. A percentage is
  carried rather than resolved, because only the caller knows what it is a
  percentage of.
- `calc.rs` — the expression, with the useful half of CSS's type system: a
  length may be added to a length and multiplied by a number, and anything else
  is refused. Checked once when parsed, so evaluating later cannot fail.
- `parse.rs` — text in, values out, and nothing approximated.
- `alo-style/src/metrics.rs` — font size and line height per element, including
  the keyword sizes and the rule that `em` in a *font size* means the parent's.

**The bug worth remembering.** `font-size` inherited as the specified text, so
`2em` inside `2em` compounded to four times the grandparent's font rather than
twice the parent's. The fix is what CSS actually says: `font-size` and
`line-height` inherit as **computed** values, so the resolved number is written
back after each element is styled. A `line-height` written as a number stays a
number, because that is its computed value and it is why one writes
`line-height: 1.5` rather than `line-height: 24px`. This is the second time this
loop has been caught by inheritance semantics — `margin: inherit` was the first
— and the pattern is the same: *what* inherits is not the same question as
*what form* it inherits in.

**Two estimates are written down rather than buried**, both in
`FontMetrics::estimated`: `ex` and `ch` are half the font size, and
`line-height: normal` is 1.2 times it. The real answers come from a font, and
queue item 6 is where a font arrives to be asked. They are named here so that
the day they are wrong, the wrongness is findable.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 361 tests, no stubs, boundaries held — `cssparser` is now also named in
`alo-value/src/parse.rs`, which is in the list. No layout assertion and no
reference render: this item resolves numbers, it does not position anything.

**What the next iteration should know.** Item 5 is layout, and it now has
everything it needs.

- `ComputedStyle::px(name, basis)` gives a number, `length(name)` gives the
  value if the caller wants to decide about percentages itself, and
  `number(name)` gives a plain number for `flex-grow` and the like. `None` from
  any of them means "absent, or something this engine cannot read" — both of
  which are "use the initial value".
- `ComputedStyle::metrics()` is the font in force, already resolved.
- **One file may name `taffy`**, and `scripts/gate.sh` will check it. Add the
  entry to `BOUNDARIES` in the same change that adds the dependency.
- Text is a box with a string in it and no size. `taffy` takes a measure
  function for exactly that, so item 5 can leave a named seam there rather than
  a stub — and should say in its journal entry which seam, so item 6 knows
  where to arrive.
