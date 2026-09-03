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

---

## 2026-09-02 — iteration 6: layout (queue item 5)

**What was built.** `crates/alo-layout`. Every box now has a rectangle.

- `geometry.rs` — `Point`, `Size`, `Rect`, `Edges`, ours rather than the layout
  engine's, so that replacing `taffy` is not a rewrite of everything that reads
  a rectangle.
- `keyword.rs` — the keyword properties, in one macro, because a keyword parsed
  slightly differently in two places is a bug on the property nobody tested.
- `sizing.rs`, `track.rs`, `placement.rs` — the value grammars layout needs.
- `style.rs` — what layout reads from a computed style, in our vocabulary, so
  the boundary file is a translation and nothing else.
- `measure.rs` — the text seam.
- `engine.rs` — **the only file in the repository that names `taffy`**, checked
  by `scripts/gate.sh`.
- `tree.rs` — the result, and `to_outline`, which is what the gate's layout
  assertion is written against.

**Decisions worth knowing about:**

- **Text is measured by the caller.** `MeasureText` has no default
  implementation on purpose: a built-in eight-pixels-a-character would be a
  wrong number every layout quietly depended on, and law 3 says a wrong pixel
  is a bug. Item 6 arrives by implementing the trait rather than by replacing
  something, and the test in `tests/numbers.rs` uses a measurer named as a fake.
- **Positions are on the page**, not relative to a parent. A caller nearly
  always wants the page, and the other direction is a walk up the tree inside a
  loop.
- **Inline formatting is a stand-in and the rule is written out.** Several
  inline children become a wrapping flex row so they sit beside each other; a
  single text child stays a block child so a paragraph fills its container and
  wraps. Neither gets baselines or breaks at the right place between two inline
  boxes. Item 6 replaces both with one real inline formatting context.

**The bug worth remembering.** `AutoLength` derived `Default` as `Auto`, so
every box in every document got `margin: auto` on all four sides and centred
itself — grid items collapsed to zero width, flex items spread out as though
`space-around` had been asked for. Four failing tests, one cause, and the cause
was a derived default rather than a written one. The initial value of a property
is part of the specification and is now stated where it is read: margin is zero,
`top`/`left` are `auto`, and `flex-shrink` is one. **A derived `Default` is a
guess at CSS.** The next crate that reads properties should assume the same.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 430 tests, no stubs, boundaries held including `taffy`'s.
`tests/numbers.rs` is the **layout assertion in numbers** the gate asks for, and
it asserts the whole tree rather than one rectangle. Still no reference render:
nothing is drawn yet, and item 7 is where the first pixel appears.

**The scope cut**, written into the queue as item 15: a `calc()` mixing
percentages in a layout property. `taffy` carries such a value as an opaque
handle only a tree implementing its own traits can resolve, and using
`taffy`'s ready-made tree is the whole point of renting it. Refused and
recorded; `calc()` of lengths only — which is what a design system writes — is
already a plain number by then and works.

**What the next iteration should know.** Item 6 is text: HarfBuzz shaping and
font rasterisation.

- **The seam is `alo_layout::MeasureText`.** Implement it and layout starts
  measuring real text; nothing else has to change to get correct widths.
- The harder half is inline formatting, and it is `engine.rs`'s
  `needs_a_line_of_its_own` that has to go — a real line box, with baselines
  and with breaking between inline boxes rather than only inside one text run.
  That is layout work living in the text item because it needs a shaper to be
  possible at all.
- `FontMetrics::estimated` in `alo-value` guesses `ex` and `ch` at half the
  font size and `line-height: normal` at 1.2. Those are the two numbers a real
  font replaces, and they are named there for that reason.
- `alo-style`'s `metrics.rs` is where a resolved font size lives, and
  `ComputedStyle::metrics()` hands it out. Item 6 should fill in the rest of
  `FontMetrics` from the font it loads rather than adding a second place for
  font facts.

---

## 2026-09-02 — iteration 7: text (queue item 6)

**What was built.** `crates/alo-text`. Layout's measuring seam is filled.

- `font.rs` — a loaded font and the handful of measurements the rest of the
  engine asks for, in CSS pixels at a size rather than in font units.
- `database.rs` — which font, and the fallback chain. Ours, because it is a
  policy question: a font is **asked** whether it has the character.
- `shape.rs` — **the only file that names `rustybuzz`.**
- `run.rs` — splitting text into runs of one direction and one font.
- `linebreak.rs` — **the only file that names `unicode-linebreak`.**
- `line.rs` — where a line actually breaks, which is ours.
- `measure.rs` — `alo_layout::MeasureText`, implemented.

**Decisions worth knowing about:**

- **`rustybuzz` rather than HarfBuzz itself.** ADR 0001 says rent the shaper;
  `rustybuzz` is HarfBuzz ported to Rust, so we rent the algorithm without
  putting a C library in a process whose second argument is memory safety. Same
  rented thing, no FFI, no `unsafe` on our side.
- **Nothing is indexed by character.** A `ShapedGlyph` names the byte *range*
  it came from. Two of the tests exist to keep that true: Arabic's glyph order
  runs down the text rather than up, and `e` + combining acute composes to one
  glyph covering both characters' bytes. `docs/features.md` asks for the
  awkward scripts first for exactly this reason, and doing them first is what
  made the shape of the data right rather than a thing to fix later.
- **The test font comes from the `dejavu` crate** (MIT/Apache-2.0), as a
  dev-dependency. Nothing binary lands in this repository, the version is
  pinned, and the tests are the same on every machine. DejaVu covers Latin,
  Arabic and Hebrew and does not cover Devanagari — which is why the
  "no font has this character" test uses Devanagari, and why reordering scripts
  are not yet tested. When a font for one is available, that test belongs
  beside the Arabic one.

**The bug worth remembering.** The greedy line-breaker took the last break that
fitted and then moved on to the *next* opportunity — so the opportunity it had
just rejected was never reconsidered from the new line's start, and lines came
out wider than the width they were given. Wrapping is a loop that sometimes does
not advance, and writing it as a `for` made that impossible to express. The test
that caught it asserts every line fits, which is the assertion to keep.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 492 tests, no stubs, boundaries held including `rustybuzz`'s and
`unicode-linebreak`'s. No layout assertion beyond item 5's — this item measures
rather than positions, and the end-to-end test asserts that a narrower window
takes more lines, which is the number that matters here. No reference render:
still nothing drawn.

**Two scope cuts**, both written into the queue:

- **16. Inline formatting.** Several inline boxes on one line, with baselines,
  and breaking *between* them rather than only inside one run. It is layout
  work that needed a shaper to be possible, and it replaces `engine.rs`'s
  `needs_a_line_of_its_own`.
- **17. Glyph rasterisation.** Folded in beside item 7 rather than before it: a
  glyph bitmap with no canvas to draw into can only be tested against itself,
  and next to paint it is tested against a picture.

**What the next iteration should know.** Item 14 is colours, and item 7 is
paint. Doing 14 first is probably right for the same reason 12 came before 5:
paint will want channels, and building a colour parser inside paint is how the
value layer grows a second one.

- `alo-value` is where a colour belongs: it is the crate that turns text into
  numbers, and `cssparser` already exposes `parse_hash_color` and
  `parse_named_color`, which are the two tables nobody should retype.
  `cssparser` is already in `alo-value`'s boundary list.
- **`FontMetrics::estimated` in `alo-value` is now wrong twice over.** `ex` and
  `ch` are still half the font size and `line-height: normal` is still 1.2,
  while `alo-text` can now answer all three from the font. Wiring that through
  means `alo-style` asking a font database for metrics, which means style
  depending on text — a real design decision, and the reason it was not done in
  this iteration rather than an oversight. Whoever does it should decide
  whether the font database belongs above style or beside it.

---

## 2026-09-02 — iteration 8: a real line box (queue item 16)

**What was built.** `alo-layout/src/inline.rs`, and the engine rewired around
it. The wrapping-flex-row stand-in from item 5 is gone.

- An inline formatting context is handed to `taffy` as a **leaf**. `taffy` has
  block, flex and grid and no inline layout at all, and inline layout is a
  different algorithm rather than a special case of the others — so it is ours,
  the way it is in every engine.
- `MeasureText` grew three methods: where a line may break, and the ascender
  and descender. A line box needs all three, and each is something layout
  cannot work out for itself.
- An atomic inline-level box — an `inline-block`, an image, a button — is laid
  out by calling the same layout again, one formatting context down, and the
  line places it whole and on the baseline.
- `LayoutTree` now reports **fragments**: the pieces a box was drawn in, one
  per line it is on.

**The three things a row of boxes could not do**, each with a test:

- A sentence breaks *between* two inline boxes, so `the <em>quick brown</em>
  fox` wraps between any two of its words.
- Everything on a line sits on one baseline, so a forty-pixel image beside
  sixteen-pixel text pushes the line down rather than the text up.
- A box that wraps has one rectangle per line. The union of them is where the
  box *is*; the pieces are what should be **drawn**, and a background painted
  from the union would cross the gap between the lines.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 511 tests, no stubs, boundaries held. `tests/numbers.rs` grew three
assertions in numbers for the three behaviours above, and the whole-interface
outline changed in a way worth noticing: text boxes are now as wide as their
text rather than as wide as their parent, which is what an inline box is.

**What the next iteration should know.** Item 17 is glyph rasterisation and item
7 is paint; item 14, colours, should come before both for the same reason item
12 came before item 5 — paint wants channels, and a colour parser built inside
paint is how the value layer grows a second one.

- `alo-value` is where a colour belongs. `cssparser` already exposes
  `parse_hash_color` and `parse_named_color`, which are the two tables nobody
  should retype, and `cssparser` is already in `alo-value`'s boundary list.
- Paint wants `LayoutTree::fragments`, not `border_box`. That distinction is
  the one thing in this iteration that is easy to get wrong later.
- **`FontMetrics::estimated` is still guessing** `ex`, `ch` and
  `line-height: normal` while `alo-text` can now answer all three. Wiring it
  through means style depending on text, which is a real design decision and
  the reason it is still not done.

---

## 2026-09-02 — iteration 9: colours (queue item 14)

**Moved ahead of items 17 and 7, and the queue says why.** Paint wants channels,
and a colour parser built inside paint is how the value layer grows a second
one — the same reasoning that moved item 12 ahead of item 5.

**What was built.** `alo-value/src/color.rs` and colour parsing in
`alo-value/src/parse.rs`, plus `ComputedStyle::color` and
`ComputedStyle::current_color` so the cascade hands out resolved colours.

**Decisions worth knowing about:**

- **`currentColor` is carried, not folded.** It is the initial value of every
  border colour, and it means "whatever `color` is on this element" — which is
  not knowable until there is an element. An engine that resolved it at parse
  time would draw every default border black.
- **Channels are floats from zero to one.** Compositing multiplies and adds
  them, and eight bits loses a little each time, which shows up as banding in
  exactly the gradients a design system uses. `Rgba::to_rgba8` rounds rather
  than truncates, for the same reason.
- **`color` does not need writing back**, and it is worth knowing why, because
  `font-size` did. A child inheriting the text `2em` resolves it again against
  its own font and compounds; a child inheriting the text `currentColor`
  resolves it against its parent and gets the same answer its parent got. Same
  rule about computed values, and this one happens to need nothing.
- **Other colour spaces are refused.** `oklch`, `lab`, `color()`,
  `color-mix()`: a different space, and a colour converted by guesswork is a
  wrong pixel that looks nearly right.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 534 tests, no stubs, boundaries held. No layout assertion and no
reference render: this item turns text into numbers and draws nothing.

**What the next iteration should know.** Item 17 is glyph rasterisation and item
7 is paint, and they are the two halves of the first picture.

- Everything paint needs is now reachable: `LayoutTree::fragments` for what to
  draw and where, `ComputedStyle::color` for what colour, and
  `alo_text::shape` for the glyphs in a fragment.
- **Paint wants `fragments`, not `border_box`.** A box that wrapped has one
  rectangle per line, and a background drawn from the union of them crosses the
  gap between the lines.
- `Rgba::over` is already the source-over compositing paint needs, and it lives
  with the channels rather than in paint on purpose.

---

## 2026-09-02 — iteration 10: glyph rasterisation (queue item 17)

**What was built.** `crates/alo-paint`, the first half of it.

- `path.rs` — one shape type for everything this engine draws. A rectangle is
  four lines, a rounded corner is an arc, a letter is a few dozen curves; one
  type means one rasteriser and one set of anti-aliasing rules, which is what
  stops a glyph and the box behind it disagreeing along their shared edge.
- `glyph.rs` — **the only file that names `ttf-parser`**, apart from
  `alo-text/src/font.rs`, which reads a face's metrics through the same parser
  `rustybuzz` is built on. Both are in the gate's list.
- `raster.rs` — **the only file that names `tiny-skia`.** A path in, coverage
  out.

**Decisions worth knowing about:**

- **Coverage is not colour.** A mask says how much of each pixel a shape covers
  and nothing about what colour it is. That is why the same glyph mask serves
  black text on white and white text on black, and why a shadow can reuse a
  mask rather than rasterise the glyph twice.
- **The Y axis turns over at the boundary and nowhere else.** A font measures
  up from the baseline; a screen measures down from the top. One minus sign, in
  `glyph.rs`, so that no later stage has to remember which way a glyph's numbers
  go.
- **A blank glyph and an unreadable font are different answers.** A space has no
  outline and comes back as an empty glyph; a font that cannot be parsed comes
  back as nothing. A caller that confused them would draw a missing-glyph box
  for every space.
- **A shape too large to raster is refused.** A typo should not ask for a
  terabyte.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 562 tests, no stubs, boundaries held including the two new ones. **No
reference render, deliberately**: a coverage mask with no canvas to draw onto
can only be compared against a picture of itself, and the assertions here say
what *shape* the mask is — an `l` is a vertical bar, an `H` has a gap at the top
and none in the middle — which is the stronger statement. Item 7 is where a
picture arrives and where the gate's reference render becomes possible.

**What the next iteration should know.** Item 7 is paint, and it is the first
item that can produce a picture — so it is the first that owes the gate a
**reference render**: a small deterministic raster, committed, diffed on every
change.

- Everything is now reachable: `LayoutTree::fragments` for what to draw and
  where, `ComputedStyle::color` for what colour, `alo_text::shape` for the
  glyphs in a text fragment, `alo_paint::outline` for their shapes, and
  `alo_paint::fill` for their coverage.
- **Paint wants `fragments`, not `border_box`.** A box that wrapped has one
  rectangle per line, and a background drawn from the union crosses the gap.
- `Rgba::over` is the source-over compositing to use; it lives with the
  channels rather than in paint on purpose.
- A PNG encoder is still needed. `png` is the obvious rental and it should get
  its own boundary file, listed in `scripts/gate.sh` in the same change.

---

## 2026-09-02 — iteration 11: paint (queue item 7)

**The engine draws.** HTML and CSS in, a PNG out. The first reference render is
committed at `crates/alo-paint/tests/references/invoices.png`: a list of
invoices with a heading, three rows, separators and a selected row highlighted.

**What was built.** The second half of `crates/alo-paint`.

- `display.rs` — the display list: what to draw, in what order. It earns its
  place twice: paint order is decided **once**, here, rather than being implied
  by whichever loop happens to visit boxes; and it is what a failing reference
  render is diffed against first, in words, so that a difference says *what*
  changed.
- `canvas.rs` — pixels, held as floats for the reason colours are: a page draws
  a background, a border over it and text over that, and three roundings on
  every pixel is how a flat colour turns into a slightly wrong one.
- `render.rs` — the list onto the canvas. Deliberately dull: the decisions were
  made when the list was built.
- `encode.rs` — **the only file that names `png`.** Both directions, because
  reading a reference back is half of comparing against one.

**The bug the first picture found**, which is exactly what a picture is for:
**text was measured at one size for the whole document.** `TextMeasurer` held a
family and a size, so a twenty-pixel heading and a fourteen-pixel row were laid
out as though both were sixteen. The fix widened the measuring seam:
`MeasureText` now takes an `alo_layout::TextStyle` **per piece of text**, and
the engine derives it from the computed style of the nearest element. That is
also what makes two sizes share one baseline correctly, so the line box got
better in the same change.

**Decisions worth knowing about:**

- **`background` the shorthand is read as well as `background-color`.** Stage 1
  does not expand shorthands in the cascade, and `background: #fff` is how a
  style sheet actually says this — reading only the longhand drew nothing at
  all. A `background` that is an image or a gradient is not a colour and is
  ignored rather than having a colour guessed out of it.
- **Only `solid` borders are drawn.** `none` and `hidden` draw nothing whatever
  their width, which is what CSS says; every other style is not implemented,
  and a dashed border drawn solid would be a wrong pixel that looks nearly
  right.
- **`z-index` only means anything on a positioned box.** That is the rule, and
  it is written where the sorting happens because it is the one people are
  surprised by.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 589 tests, no stubs, boundaries held including `png`'s. **A reference
render exists and is diffed** — the gate's requirement for anything visual, met
for the first time. `ALO_UPDATE_REFERENCES=1` rewrites it; the test says so, and
says to read the diff before committing.

**The scope cut**, written into the queue as item 18: rounded corners, shadows,
gradients, clipping, transforms and opacity — the rest of `features.md`'s Paint
line. Item 7 draws a colour inside a shape and the shape is always a rectangle;
every one of those changes what the shape is or how colours combine, and each is
worth its own reference render. Item 11's real alo screen will need at least the
rounded corners.

**What the next iteration should know.** Item 8 is the reference corpus.

- The machinery is in `crates/alo-paint/tests/reference_renders.rs` and is
  worth moving somewhere shared rather than copying: `draw`, `compare`, and the
  `ALO_UPDATE_REFERENCES` escape hatch.
- Item 8's queue text asks for **each case with its expected image *and* its
  expected box tree**. Both already exist as assertions — `BoxTree::to_outline`,
  `LayoutTree::to_outline`, `DisplayList::to_outline` — so a case is four
  files' worth of expectation, and the corpus is a directory of them rather
  than new machinery.
- The one thing missing is a way to run one case by name and see all four
  differences at once, which is what makes a corpus usable rather than a wall.

---

## 2026-09-02 — iteration 12: the shape of a box (queue item 18)

**Item 18 was split before it was built**, into three, because it bundled three
different kinds of work: what shape a box *is* (this item), how a colour *fills*
a shape (item 19: shadows and gradients), and how a drawn thing is *combined*
with what is behind it (item 20: transforms and opacity). Each wants its own
reference render, and each has its own value grammar.

**What was built.** `alo-paint/src/corner.rs`, and clipping in the display list
and the renderer.

- `border-radius`, in every form CSS writes it: one to four values, the `/`
  that splits horizontal radii from vertical, and the four per-corner
  longhands. The two-value form pairs the *diagonals* where every other
  box-model shorthand pairs opposite sides, which is written down beside the
  code that does it.
- Radii that do not fit are **scaled down together** rather than clamped one at
  a time. That is CSS's rule and it is the one that keeps a shape's
  proportions; clamping would make one side rounder than another.
- `overflow` other than `visible` pushes the box's own shape as a clip around
  its children, and the renderer keeps a stack of them because clips nest. A
  clip **multiplies** coverage rather than switching it on and off, so the edge
  of a rounded clip is as smooth as the shape it came from.

**The thing the picture showed.** The first version drew a uniform border as
four rectangles, which had square corners sitting over a rounded background — a
visible seam at every corner. A border of one width and one colour is now a
**ring**: the box's shape with the box's shape inside it, wound the other way,
so the non-zero fill rule leaves the middle empty. A border whose sides differ
is still four rectangles, clipped to the box's shape so they cannot stick out;
its inner corner is squarer than CSS draws it, which shows only with a thick
border and a large radius, and that is written where the code is.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 600 tests, no stubs, boundaries held. **A second reference render** —
`rounded-card.png` — with assertions beside it that say *why* the picture is
right: the card's corner is the page's background because it was clipped away,
and the middle of the banner is not.

**What the next iteration should know.** Item 8 is the reference corpus, and it
is now the right time for it: there are two reference renders and the machinery
to make more, sitting in one test file that wants to be shared.

- `draw`, `compare` and the `ALO_UPDATE_REFERENCES` escape hatch are the parts
  to move somewhere a corpus can use.
- Item 8 asks for each case with its expected image **and** its expected box
  tree. `BoxTree::to_outline`, `LayoutTree::to_outline` and
  `DisplayList::to_outline` are all there; a case is four expectations, and the
  corpus is a directory of them rather than new machinery.
- The one thing missing is running one case by name and seeing all four
  differences at once, which is what makes a corpus usable rather than a wall.

---

## 2026-09-02 — iteration 13: the reference corpus (queue item 8)

**What was built.** `crates/alo-corpus`: six cases, each a directory, each with
four expectations beside it.

- `case.rs` — a case is a **directory of files**. That is the decision the rest
  follows from: an expectation that lives in a file shows up in a diff, and a
  reviewer reads "row three moved four pixels" out of the diff rather than
  reproducing a test failure to find out.
- `pipeline.rs` — every stage in one call. It existed three times already, in
  three test files, and three copies of the pipeline is three places for it to
  be assembled differently. It is **not** an embedding surface and says so.
- `check.rs` — **all differences at once**. A case that changed usually changed
  in more than one way — a box moved, so the display list moved, so the picture
  moved — and reporting the first and stopping means four runs to find out what
  happened.

**Four expectations, and why none is redundant:** `boxes.txt` catches a change
in what exists, `layout.txt` in where it is, `display.txt` in what is drawn, and
`render.png` everything the other three cannot describe — anti-aliasing, glyph
shapes, compositing. A fifth, `issues.txt`, records what the engine refused, so
a case that renders oddly says why rather than being investigated.

**The corpus found a bug the day it was written**, which is the argument for it.
Removing a stray clip from the paint code changed `grid-of-three`, and the
report named the case, the expectation and the line: `was: clip box#4 to (8, 8)
184×30`. A box with rounded corners and **no border** had been pushing a clip
for the border it did not have.

**Two things moved to where they belong.** The two reference pictures that lived
in `alo-paint/tests` are now cases in the corpus — one place, one runner, one
way to update them. What stayed in `alo-paint` is `tests/clipping.rs`: the half
a picture cannot say, which is *why* the picture is right. A card's corner is
the page's background **because it was clipped away**, and an assertion says
that where an image can only differ.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 608 tests, no stubs, boundaries held. The corpus is the gate's reference
render requirement, now general rather than one picture.

**What the next iteration should know.** Items 9 and 10 are next, and they are
the ★ ones — the reason `docs/decisions/0002` says this engine exists rather
than a faster fork of somebody else's.

- **Everything item 9 needs is already on the boxes.** `BoxNode::semantics`
  carries the role, the states and the declared name, put there when the box
  was made; `LayoutTree` says where each box is and `fragments` says what it
  was drawn as. The agent tree is a **view** over those, and ADR 0002 is
  explicit that a second structure is the failure mode.
- The one thing not built is the **accessible name** algorithm — the full one,
  which falls back to a box's own text and to a `<label>` pointing at a field.
  `alo-box/src/semantics.rs` says so where it stops, and item 9 is where it
  belongs, because it needs the finished tree to walk.
- Item 10's verbs must take a **name or an id, never a coordinate** (ADR 0002),
  and `scripts/gate.sh` cannot check that — a person must.

---

## 2026-09-02 — iteration 14: ★ the agent tree (queue item 9)

**The reason this repository exists.** `docs/decisions/0002` opens with a
sentence — *"invoice list, twelve rows, row three selected"* — and the engine
now answers it about a page it rendered, by role and by name, with no screenshot
anywhere in the chain.

**What was built.** `crates/alo-agent`.

- `tree.rs` — the view. **Nothing is built.** An `AgentNode` is a box's id and a
  borrow of the trees that already draw the page, and every question is answered
  from them when it is asked. ADR 0002 is unambiguous about why: if the two
  could disagree, an agent would eventually act on something that is not on
  screen. There is nothing here to disagree with.
- `name.rs` — the accessible name, in ARIA's order and for ARIA's reasons. This
  is the piece `alo-box` said it could not do, because steps three and four need
  a finished tree to walk: a `<label>` somewhere else in the document, and a
  button's own content.

**Decisions worth knowing about:**

- **A box that means nothing is read through**, exactly as a screen reader does.
  A page is mostly `<div>`s; a tree that showed all of them would bury the
  twelve rows an agent is looking for.
- **Text that already names its parent is not reported twice.** A button reads
  as `button "Save"`, not as a button containing a text node saying the same
  thing — an agent choosing between two nodes that are the same thing is an
  agent about to act on the wrong one.
- **A node says whether it is on screen.** ADR 0002 rejects exposing the DOM
  partly because *"a scrolled-away row looks identical to a visible one"*; this
  is the answer that makes it not.
- **`KnownRole::Text` lives in `alo-box`** rather than in the agent crate, so
  that there is one list of roles rather than two. No element has that role —
  text is not an element — and the view is what assigns it.

**The bug the agent tree found on its first run.** A space that arrived as its
own text box — the one between `<a>All</a>` and `<a>Due</a>` — advanced the pen
by nothing, so the two links touched and `small and` rendered as `smalland`. I
had seen it in a corpus thumbnail an hour earlier and talked myself out of it;
the agent tree made it unmissable because two links with no gap between them is
obvious in an outline in a way it is not in a picture. **The corpus then caught
the fix**, named the case and the line, and the new picture reads correctly.

**Every corpus case now pins `agent.txt`** beside its picture. ADR 0002 asked
for exactly that: *"Reference renders can assert the tree, not just pixels. A
test can say 'row three is selected and sits at these coordinates', which is a
far better failure message than an image diff."*

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 619 tests, no stubs, boundaries held.

**What the next iteration should know.** Item 10 is typed verbs, and it is the
other half of ADR 0002.

- **No verb takes a coordinate.** `scripts/gate.sh` cannot check that; a person
  must, and the queue says so.
- A verb names its target the way `AgentTree::named` and `with_role` do, and
  comes back with what it did — `alo-os`'s verb contract is the shape to
  follow.
- **A form control lays out at 0×0** today: `<input>` has no intrinsic size
  because the user-agent sheet gives it none. It does not block item 10, whose
  verbs work by name, but item 11's real alo screen will need it, and it is the
  kind of thing that belongs in `alo-style/src/user_agent.rs` rather than
  anywhere clever.

---

## 2026-09-02 — iteration 15: ★ typed verbs (queue item 10)

**The other half of ADR 0002.** An agent can now act on a page it read:
activate, put text, scroll — each aimed by a **description** rather than a
position. "The Save button." "The row called Invoice 12." A description
survives the page moving; a point does not, which is the whole of ADR 0002's
argument.

**What was built.** `alo-agent/src/verb.rs`.

- `Target` — every form of it is a description: a name, a role, both, or a
  `BoxId` the caller already read. The last one names rather than describes,
  and it is safe for ADR 0003's reason: ids are allocated once and never
  reused, so a stale one names nothing rather than something else. A test
  asserts exactly that.
- `Verb` — three of them, which is what `docs/features.md` lists.
- `Outcome` — a record of what was asked for and what happened, which is the
  guarantee a screenshot-and-guess agent cannot make.
- `Refusal` — and this is the half that makes the surface worth having.

**The two refusals worth naming:**

- **Ambiguous.** Two things called the same name is not a reason to pick one.
  The refusal names both, so the caller can narrow the request — and a test
  shows narrowing working.
- **Disabled.** A control that says it cannot be operated is not operated,
  though nothing physically prevents it. An agent that pressed a disabled
  button would be doing something a person cannot.

**A verb validates and reports rather than mutating.** Stage 1 has no scripting
and no DOM mutation — `docs/features.md` puts both in stage 2 — so `Activate` on
a link comes back with where it goes and `PutText` with the field and the text.
Applying that is the host's. That is not a stub: it is the whole of what a verb
contract *is* at this stage, and the record is the part `alo-os` rests on.

**The gate now checks the no-coordinate rule.** It was on the list of things a
person had to remember; a function in `crates/alo-agent/src` that takes a
`Point`, or an `x: f32` and a `y: f32`, now fails the run. I verified it bites
by adding `press_at(x, y)` and watching it fail. What the check still cannot see
is whether a verb's *meaning* depends on a position, and the gate says so.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 639 tests including fourteen verb paths and every refusal, no stubs,
boundaries held.

**What the next iteration should know.** Item 11 is a real alo screen — the one
that turns this from plausible into real, and stage 1's exit gate.

- It needs `alo-os`'s sign-in screen and `alo-workplace`'s
  `web/src/ds/tokens.css`, both of which are **read-only reference**. Neither
  is checked out beside this repository today: only `alo-workplace` is present
  at `~/Documents/GitHub/alo-workplace`, and `alo-os` is not. **That is the one
  thing that could block the loop**, and it should be checked before starting
  rather than half-way through.
- **A form control lays out at 0×0.** `<input>` has no intrinsic size, because
  the user-agent sheet gives it none. A sign-in screen is mostly form controls,
  so this is the first thing item 11 will hit. It belongs in
  `alo-style/src/user_agent.rs`.
- Items 13, 15, 19 and 20 remain, and none of them blocks item 11.

---

## 2026-09-02 — iteration 16: a real alo screen (queue item 11)

**alo's sign-in screen renders.** The real markup from
`alo-workplace/web/src/auth/LoginPage.tsx`, the real rules from its CSS module,
and the real colours from `web/src/ds/tokens.css` — the file
`docs/autonomy/QUEUE.md` calls "the specification for what correct means here".
It is `crates/alo-corpus/cases/alo-sign-in/`, and it is diffed on every run.

**The target changed, and this is the honest account of it.** The queue names
`alo-os`'s sign-in screen, from the Figma file. **`alo-os` is not checked out
beside this repository** and the Figma file is not reachable from here. What is
available is `alo-workplace`, which has its own sign-in screen in code — better
than a Figma file, because it is what ships. So that is what was rendered.

**Stage 1's exit gate is not met**, and `docs/conformance.md` and `ROADMAP.md`
both say so rather than being ticked. The gate names alo OS's sign-in screen and
Settings, on the certified machine; none of those three things happened here.
Both files carry the reason.

`alo-workplace` was **read only**. Nothing was written to it.

**Four bugs, which is what a real screen is for.** Every one of them was
invisible in six synthetic corpus cases and obvious in one real one:

1. **Text that was a flex item was never drawn.** Fragments come from a line
   box, and a text box that is a flex or grid item has none — so `draw_text`
   iterated an empty list and drew nothing. The SSO button was an empty
   rectangle.
2. **Rounding.** Layout rounded boxes to whole pixels and measured text
   unrounded, so a box 96.16 wide became 96 and its 96.16-wide text wrapped:
   "Remember me" on two lines inside a box wide enough for one. Layout is
   sub-pixel throughout now, and rounds once, at the end, when coverage becomes
   pixels.
3. **`border: 1px solid` was not read.** Only the longhands were, and nobody
   writes those. Splitting it is in `alo-value/src/shorthand.rs` because
   `border` splits by *kind* rather than by position — `1px solid red` and `red
   solid 1px` are the same border — which cannot be done by counting.
4. **An empty `<input>` laid out at nothing by nothing.** A field with nothing
   typed into it is still one line tall and about twenty characters wide, and
   the user-agent sheet had never said so.

**And `text-align`,** which a real screen needs and six synthetic ones did not:
a button's label sits in the middle of it, which browsers do with an anonymous
centring box and this engine does with a centring flex container in the
user-agent sheet.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 647 tests, no stubs, boundaries held. Seven corpus cases now, each with
five expectations.

**What the next iteration should know.** The queue's remaining items are 13
(block-in-inline), 15 (`calc()` with a percentage in a layout property), 19
(shadows and gradients) and 20 (transforms and opacity). None blocks another.

- **The most valuable next thing is probably none of them.** It is `alo-os`
  being checked out, so that the screen the exit gate actually names can be
  rendered. That is not something this loop can do.
- If item 19 is taken, `linear-gradient` is what alo's own screens will want
  first; `box-shadow` needs a blur, which is a rasteriser question rather than
  a value one.
- The sign-in case's stylesheet lists four things the engine does not implement.
  `clamp()` is the one a second real screen is most likely to need, and it is
  the same expression machinery `calc()` already has.

---

## Iteration 17 — queue item 19: shadows and gradients

**What was built.** A box can cast a shadow and be filled with a colour that
changes across it. `box-shadow` with offset, blur, spread, colour and `inset`;
`text-shadow`; `linear-gradient` with an angle or a `to <side>` phrase; and
`radial-gradient`, an ellipse through the farthest corner, which is CSS's
default and the reason a gradient in a wide box is an oval.

**The one decision worth writing down: a shadow is coverage blurred, not a
picture blurred.** The shape is rasterised to a mask — how much of each pixel
it covers — the mask is softened, and the colour arrives afterwards. Blurring
composited pixels would have blurred whatever was behind the shadow along with
it. It is also what makes an inset shadow the same code: an inset shadow is the
shadow of a *hole*, so it is the same blur run on the box with the box cut out
of it, clipped to the box. One blur, two kinds of shadow.

**And one bug avoided by thinking about it first.** A run of text is outlined
into a single shape before it is blurred. One blur per letter, composited, is
visibly darker where two letters touch — and it would have looked like a font
problem rather than a compositing one.

**Refused rather than approximated**: `conic-gradient`, the repeating forms,
interpolation hints, and interpolation in any colour space but sRGB. Each is a
different curve through colour; drawing one as another is a wrong pixel that
looks nearly right, which is the worst kind.

**Two files were split, because they had gained a second reason to change.**
`Coverage` left the rasteriser — more than one thing makes coverage now, and
the type they share should not belong to either of them. Building a display
list left the display list: a new CSS property changes the builder, a new kind
of drawing changes the list, and a file with both reasons is a file two changes
collide in.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 702 tests, no stubs, boundaries held, no verb takes a coordinate. Eight
corpus cases now. The new one, `shadowed-card`, has all four of the new things
in it at once, and its four expectation files plus its picture are committed.

**What the next iteration should know.** Two queue items remain: 13
(block-in-inline) and 20 (transforms and opacity). Item 15 (`calc()` with a
percentage in a layout property) is the third.

- Item 20 changes paint *order*, not just paint: both `transform` and `opacity`
  establish stacking contexts, and `opacity` needs a subtree drawn to its own
  surface and composited once. The renderer has no notion of a surface yet.
- The blur caps its mask at sixteen million pixels and comes back unblurred
  past that. Nothing on a real page approaches it; a full-page shadow on a very
  large window would.
- A border with four different widths still turns its inner corner squarer than
  CSS draws it. `docs/conformance.md` says so.
- The most valuable next thing is still `alo-os` being checked out, so the
  screen stage 1's exit gate actually names can be rendered. This loop cannot
  do that.

---

## Iteration 18 — queue item 20: transforms and opacity

**What was built.** `transform` — `translate`, `scale`, `rotate`, `skew` and
`matrix`, about a `transform-origin` that is the middle of the box unless the
author says otherwise — and `opacity`, as a number or a percentage.

**`opacity` is a group.** The subtree is drawn on a surface of its own and
composited back once. This is the whole reason it is not simply a multiplier
on every colour: two black squares exactly on top of one another, in a group at
half opacity, are one mid grey; faded box by box the second would show through
the first and come out three quarters dark. The test for that is worth more
than the picture.

**A gradient under a transform asks where the pixel came from.** The renderer
maps the pixel back through the inverse of the transform before asking the
gradient what colour it is, so a turned box's gradient turns with the box
rather than staying pinned to the page. That is one line of code and the reason
`Matrix::inverted` exists.

**Paint order stopped being a lie.** It was one flat list of positioned boxes,
sorted at the end, and a positioned box's *children* were left behind in the
flow. It is now what CSS describes: a positioned box is painted last in the
stacking context it belongs to, its subtree with it, and a box that does not
establish a context passes its positioned descendants up to the one that does.
A negative `z-index` goes behind its parent's content and in front of its
background. Nothing in the corpus moved, which is what a restructure should
look like when the old behaviour happened to be right for small pages.

**Refused rather than approximated:** anything with a third dimension —
`rotate3d`, `matrix3d`, `perspective`, `translateZ`. A value containing one is
refused whole rather than half applied, because half a transform puts a box
somewhere nobody asked for.

**Honest about one approximation.** A blur under a non-uniform scale or a skew
is softened by the square root of the area the transform multiplies by — the
average of the two axes — because a blur radius is one number.
`docs/conformance.md` says so.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 738 tests, no stubs, boundaries held, no verb takes a coordinate. Nine
corpus cases now; the new one, `turned-and-faded`, has a rotated tag, a faded
gradient panel and a negative-`z-index` band under a scaled box.

**What the next iteration should know.** Two queue items remain: 13
(block-in-inline) and 15 (`calc()` with a percentage in a layout property).

- Item 13 is a box-tree question rather than a paint one, and it is the last
  structurally hard thing in stage 1's inline model.
- A transform does not affect layout, which is correct, and it means the agent
  tree reports where a box *is laid out* rather than where it is drawn. That is
  the right answer under ADR 0002 — a verb names a thing rather than a point —
  but it is worth knowing before someone reports it as a bug.
- The most valuable next thing is still `alo-os` being checked out, so the
  screen stage 1's exit gate actually names can be rendered. This loop cannot
  do that.

---

## Iteration 19 — queue item 13: a block inside an inline, split properly

**What was built.** An inline box holding a block-level box is now **broken
around it**: a piece on each side, each a box of its own, with the block
between them. The engine used to treat the inline as a block container, which
looks nearly the same and is not the same — a highlighted phrase interrupted by
a block ran its background straight through the interruption instead of
stopping and starting again.

**The part that decided the shape of the code.** The block has to become a
*sibling* of the anonymous blocks the pieces sit in, one level up. That cannot
be done by rearranging an element's children in place, so building an inline
element now hands **several** boxes back to its parent — piece, block, piece —
and the parent's existing `arrange` wraps each run of pieces in an anonymous
block without knowing anything about the break. The recursion falls out: an
inline inside an inline splits too, because the outer one sees a block-level
child in its own list.

**A tree with a broken box in it is still one thing to an agent.** The pieces
come from one element and would both answer to the same name, which is exactly
the ambiguity ADR 0002's verbs refuse. The later pieces carry
`continued_from`, and the agent tree reads them *through* — one link, in two
boxes.

**Two gaps this found, both recorded rather than left quiet.**

1. CSS keeps an **empty** piece — "even if either side is empty" — and an empty
   inline with a border draws that border. This engine drops it, because its
   inline formatting would give it a line box of the font's height, which is a
   visible gap where CSS asks for none. `IssueKind::UnsupportedStructure` on
   every tree that meets one. Queue item 21.
2. **An inline box's own border and padding are neither laid out nor drawn.**
   The corpus case asked for a border and got none, which is how this was
   found. The background *is* drawn, which is why the case still shows the
   break. Queue item 22.

And one honest limit written down as item 23: the name of a broken link comes
from its first piece alone, and the block between the pieces is not read as
part of it. Reading it whole means the agent tree following the *document's*
containment where the box tree has split, which is a change to what a view is
and not something to slip into this item.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 743 tests, no stubs, boundaries held, no verb takes a coordinate. Ten
corpus cases; the new one, `broken-inline`, is a yellow highlight that stops
before a block and starts again after it.

**What the next iteration should know.** One item from the original queue
remains — 15, `calc()` with a percentage in a layout property — plus the three
written this iteration.

- Item 15 is a `taffy` problem rather than a CSS one: `taffy` carries such a
  value as an opaque handle only a tree implementing its own traits can
  resolve, and this engine uses `taffy`'s ready-made tree.
- Item 22 is the one a real page is most likely to hit: `border` and `padding`
  on a `<span>` is ordinary CSS.
- The most valuable next thing is still `alo-os` being checked out, so the
  screen stage 1's exit gate actually names can be rendered. This loop cannot
  do that.

---

## Iteration 20 — queue item 15: `calc()` with a percentage, and ADR 0004

**What was built.** `width: calc(100% - 2rem)` is a number. So is a `calc()`
with a percentage in a height, a minimum, a maximum, a margin, a padding, an
inset, a gap or a grid track. Until this iteration every one of them was
refused and recorded.

**The queue called this "a decision rather than a chore", and it was right.**
`taffy` carries a `calc()` as an opaque handle and asks the *tree* to resolve
it, because the basis is the containing block's size and only the running
algorithm knows that. Its ready-made `TaffyTree` answers `0.0` and offers no
hook. So the choice was: refuse the value forever, resolve it in a second pass
that is exact in block layout and quietly wrong in an auto-sized flex or grid
container, fork `taffy`, or **own the tree**. ADR 0004 is written and accepted:
the tree is ours, the algorithms stay rented.

That is not a walk back from ADR 0001. Flexbox, grid and block sizing are the
physics — thousands of lines of specification with decades of interoperability
in them, and this repository has no interest in rewriting them. A list of nodes
with styles, children, a cache and a result is *storage*, `taffy`'s own trait
set exists for embedders to provide it, and its `TaffyTree` is documented as a
convenience. The line did not move; the ready-made tree was on the wrong side
of it.

**No `unsafe`, and nothing to declare.** `taffy` types the handle as
`*const ()` and documents that it "may be a pointer, index, etc." — it only has
to be non-null with its low three bits clear. So it is `(index + 1) * 8`.
Casting an integer to a pointer is safe, casting it back is safe, and nothing
dereferences it. An index also cannot dangle, survives the `Vec` growing, and a
handle from another arena resolves to nothing rather than to somebody else's
expression — the same argument ADR 0003 makes about node identity.

**Two things came out better for free.** Rounding is now *impossible* rather
than switched off, because `taffy`'s rounding is a pass over a trait
`arena.rs` does not implement. And the measure function stopped being a closure
threaded through the call and became a branch in `compute_child_layout`, which
is where `taffy` expects a leaf to be measured.

**Still refused, still recorded:** a `calc()` inside `fit-content()`. The
algorithms have no spelling for it, and a fallback that guessed would be a
wrong pixel.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 750 tests, no stubs, boundaries held — `arena.rs` joined `engine.rs` as
a file allowed to name `taffy`, and nothing else does. Eleven corpus cases; the
new one, `calc-widths`, is a panel with a full-width bar less a gutter and a
half-width bar offset by a quarter.

**Every item in the original queue is now done.** What remains was written by
the iterations that found it:

- **21.** An empty piece of a broken inline keeps its border. Needs the
  zero-height line-box rule first.
- **22.** Borders and padding on an inline box — laid out and drawn. The most
  likely of the three to be hit by a real page: `border` on a `<span>` is
  ordinary CSS.
- **23.** An agent reads a broken inline as one whole thing.

And the thing this loop cannot do, said once more because it is still the most
valuable: **`alo-os` is not checked out beside this repository**, so the screen
stage 1's exit gate actually names has never been rendered. `ROADMAP.md`'s line
for it is deliberately not ticked and `docs/conformance.md` says why.

---

## Iteration 21 — queue item 22: an inline box has a box of its own

**Taken before item 21**, and the reason is written into the queue: 21 needs
the zero-height line-box rule and this did not, and a `border` on a `<span>` is
ordinary CSS that a real page writes.

**What was built.** A `<span>`'s own border and padding are laid out and drawn.
The horizontal ones take room on the line, once at its start and once at its
end; the vertical ones draw without changing the height of the line, which is
CSS's rule and what stops a padded `<em>` pushing a paragraph's lines apart.

**The change that made it possible was in the shape of the input.** An inline
box used to be *flattened*: its children joined the line's item list and the
box itself was never on the line at all — it got a position afterwards, from
the union of everything beneath it. It now arrives as an **open** and a
**close** around its content, so the line builder knows when it is inside one.

**That fixed a second bug nobody had reported.** Because the box's rectangle
was the union of its pieces, a `<span>` that wrapped across two lines was drawn
as *one* rectangle — with the gap between the lines painted over. It now gets
one fragment per line, like everything else that wraps, and paint draws one
area per fragment.

**And a third, in the same place.** A broken box's start border is drawn only
on its first piece and its end border only on its last, the way a browser draws
a wrapped `<a>`. A piece in the middle has neither.

**One detail worth remembering.** A piece ends at its *content*, not at the
pen. A line that ends in a space has advanced past the last glyph, so a
background painted to the pen ran out past the end of the text — visible as a
few pixels of colour hanging off the right of every wrapped line.

**Refused rather than guessed at, and recorded:** a percentage padding on an
inline box. It is a percentage of the containing block's width, and that is not
known where a line is built.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 758 tests, no stubs, boundaries held, no verb takes a coordinate.
Twelve corpus cases; the new one, `inline-box`, is a bordered chip broken
across two lines, and `broken-inline` has the border it originally asked for.

**What the next iteration should know.** Two items remain, both written by the
iterations that found them:

- **21.** An empty piece of a broken inline keeps its border. Still needs the
  zero-height line-box rule: with the open/close items in place an empty inline
  now *would* make a line, which is exactly the visible gap the cut was made to
  avoid.
- **23.** An agent reads a broken inline as one whole thing.

And still the most valuable thing this loop cannot do: **`alo-os` is not
checked out beside this repository**, so the screen stage 1's exit gate names
has never been rendered.

---

## Iteration 22 — queue item 21: the empty piece, and the rule it was waiting for

**What was built.** An inline box broken around a block keeps the piece on the
*empty* side, and that piece draws its border — which is what CSS asks for,
"even if either side is empty", because an empty inline with a border is still
a thing a page can see.

**The rule that made it free.** A line box holding no text, no preserved space
and no inline box with a margin, padding or border is **zero-height and treated
as not existing**. So the empty piece costs nothing at all when it has no
border, and costs exactly one line when it has one. The rule belongs in the
line builder, because that is the only place that knows what a line ended up
holding — and it is one boolean, set by text, by an atomic box, and by an
inline box with an edge of its own.

**Item 22 is what made this cheap.** An inline box only arrives at the line as
an open and a close because of last iteration's work; without that there was
nothing on the line to say "an inline box with a border was here", and the rule
could not have been written.

**The agent needed a smaller rule than expected.** Of the pieces of a broken
inline, the one that is read is the **first with anything in it**; the rest are
read through. A border is not something to read, so an empty piece is never a
second link with the same name — which is the ambiguity ADR 0002's verbs
refuse. `BoxTree::pieces_of` answers for any piece, so nothing has to know
whether a box was broken before it can ask.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 762 tests, no stubs, boundaries held, no verb takes a coordinate.
Thirteen corpus cases; the new one, `empty-piece`, is a block at the *start* of
a bordered `<span>`, so the piece before it holds nothing and is drawn as the
small mark a browser draws.

**What the next iteration should know.** One item remains:

- **23.** An agent reads a broken inline as one whole thing. What is done so
  far keeps the reading *unambiguous* — one link, not two — but the name of a
  broken link still comes from the piece that is read, and the block between
  the pieces is not read as part of it. Doing it properly means the agent tree
  following the **document's** containment where the box tree has split, which
  is a change to what a *view* is and deserves the same care ADR 0002 got.

And the thing this loop cannot do, unchanged: **`alo-os` is not checked out
beside this repository**, so the screen stage 1's exit gate names has never
been rendered. `ROADMAP.md`'s line for it is deliberately not ticked.

---

## Iteration 23 — queue item 23: an agent reads a broken link as one link

**What was built.** An inline box broken around a block is now read as **one
thing**: one node, named by everything the element contains, positioned
everywhere it was drawn, with the block read *inside* it rather than beside it.
`<a>Read the<p>manual</p>carefully</a>` is one link called "Read the manual
carefully", and the paragraph is a child of it.

**It stayed a view, which was the whole question.** ADR 0002 says the agent
tree is a *view* of the trees that already exist, never a parallel structure —
so the answer could not be "walk the document instead". It is: the **box tree
records which boxes belong to which whole**. A later piece already said which
box it continues; a block now says which inline box it was taken out of. The
reader follows what is there. Nothing is built and nothing can disagree.

**Three questions had to move from the box tree to the view.** Who holds a box
— a block taken out of a link is held by the link, not by the paragraph layout
put it beside. What is inside a box — the children of every piece, with the
blocks in their places. And where a box is — all of it. Once `view_parent`
existed, `aria-hidden` on a broken link hid the block inside it for free, which
it had not before.

**A name got a space, and that was not about breaking at all.** `<a>Read
the<div>the manual</div></a>` was called "Read thethe manual": a block-level
box is a line of its own on the screen and a name read out has to sound like
what a person sees. The corpus caught it immediately in a place nobody was
looking — alo's own sign-in headline, which had been "Your workspace.Your
servers.Your rules." and is now the three sentences it is. That is the corpus
doing exactly what it is for.

**One thing was made faster on purpose, not for speed but for scale.** "Is this
box broken" is asked of every box a reader walks, and answering it by searching
would have made reading a page quadratic. It is one field.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 765 tests, no stubs, boundaries held, no verb takes a coordinate.
Fourteen corpus cases; the new one, `broken-link`, is a link with a block in
the middle of it, read as one link.

---

# The queue

**Every item in `docs/autonomy/QUEUE.md` is ticked.** Twenty-three items: the
thirteen the roadmap named for stage 1, and ten more written by the iterations
that found them. Each was built whole, passed the gate, and was committed and
pushed on its own.

**What this engine does today.** HTML and CSS in; a DOM; stylesheets; a cascade
with `var()` and `calc()`; a box tree that knows what each box *means*; layout
in block, flex, grid and real inline formatting, sub-pixel throughout; text
shaped with real fonts and broken by UAX #14; a display list; shadows,
gradients, transforms and opacity; an anti-aliased software raster to a PNG.
And the thing it exists for: an agent tree that is a *view* of that layout, and
typed verbs that name things instead of pointing at them.

**Stage 1's exit gate is not met, and the reason is not something this loop can
fix.** `ROADMAP.md` asks for a real `alo-os` screen rendered correctly on the
certified machine. **`alo-os` is not checked out beside this repository**, and
no hardware verification has been done or claimed. `alo-workplace`'s sign-in
screen is in the corpus and renders; that is a different screen, and
`docs/conformance.md` says so plainly. The line in `ROADMAP.md` is deliberately
not ticked.

**What the next person should do first**, in order:

1. Check out `alo-os` beside this repository and add its sign-in screen to the
   corpus. Everything else is guesswork until a screen somebody actually ships
   goes through this engine.
2. Whatever that screen refuses. `docs/conformance.md` lists what is refused
   rather than approximated, and a real sheet will name the ones that matter.
   `clamp()` is the most likely first — it is the expression machinery
   `calc()` already has.
3. Stage 2's decisions, which are decisions rather than chores and want ADRs:
   the JavaScript engine (ours, in Rust, a correct interpreter first) and the
   process model.

LOOP COMPLETE

---

# Stage 2

`ROADMAP.md`'s stage 1 queue is finished. Its three unticked roadmap lines are
not work this loop can take — `alo-os` is not checked out beside this
repository, and hardware acceleration and embedding are explicitly after the
software path is right and want a machine. Nothing in the engine needs a GPU, a
window or a network to be *built or checked*; what needs a machine is the
verification the exit gate asks for, and that is not a thing a loop may claim.

So the queue was refilled from stage 2, in the roadmap's own order: eighteen
items, beginning where the roadmap begins.

## Iteration 24 — queue item 24: ADR 0005, the process and sandbox model

**What was decided.** A privileged browser process owns the network, the disk,
the display and the user; a renderer process per site owns everything that
touches a page, with almost no privilege and the platform's own sandbox around
it; work crosses as typed messages in one direction; a renderer that dies takes
nothing with it.

**The question this project had to answer first.** Chromium's process model
exists in large part because a C++ renderer is assumed to be exploitable, and
ours is not — so the obvious reading of ADR 0001 is that we do not need one.
Four reasons say otherwise and three of them survive a *perfect* engine:

1. **Spectre** is a hardware property. No language prevents it, and site
   isolation is the only mitigation that works. This decides the ADR on its own.
2. **The physics we rent has `unsafe` in it** — TLS, image and media codecs,
   shaping. Forbidding `unsafe` in our crates does not reach inside a
   dependency, and codecs are historically the worst surface in any browser.
3. **Logic bugs are not memory bugs.** The same-origin policy is code we will
   write and can get wrong; a process boundary is a second answer enforced by
   something that is not us.
4. **A page must not be able to end the session.**

**The expensive half is the shape, not the `fork`.** An engine written against
a synchronous, ambient, reach-anywhere API cannot be pulled apart afterwards
without rewriting everything that used it. So the boundary gets built while
everything is still one process (item 25) and the split is a change of
transport (item 29). That also keeps the corpus deterministic and
single-process, which is what keeps a reference render diffable.

**ADR 0003 paid for itself here.** A borrowed reference cannot cross a process,
so what crosses is a *message describing the agent tree at one instant* — a
copy, unavoidably. It carries node identity, ids are never reused, and a verb
sent back naming a node either finds the same node or finds nothing. That is
the same property ADR 0002 refuses coordinates for.

**Nothing was pre-authorised.** The sandbox will need syscalls; if a platform
crate does not cover something and we must write `unsafe` ourselves, that needs
its own ADR at that time. Law 4 is untouched.

**The gate.** `scripts/gate.sh` green. No crate changed — this item is a
decision — so the tests are the 765 that were already passing.

**What the next iteration should know.** Item 25 is the one that matters most
and is the easiest to get subtly wrong: the boundary has to be a *type*, and
every later item has to be written against it even where it is not needed yet.

---

## Iteration 25 — queue item 25: the engine behind a message boundary

**What was built.** `alo-renderer`: the renderer's side of ADR 0005.
`Renderer::handle` takes a `ToRenderer` and returns a `FromRenderer`, and that
is the whole surface — no callback in the signature, no handle to call back
through, nowhere to wait. Everything it needs arrives in a message or in
`Renderer::new`. Fonts are a constructor argument for a reason and not for
tidiness: a sandboxed renderer cannot open a font file, so in the split the
browser process hands them over.

**The test that matters most is four lines long.** `could_be_sent::<T>()`
requires every message type to be `Send + Clone + 'static`. A message holding a
borrow, or a handle, or anything tied to this process compiles perfectly well
today and cannot be sent tomorrow — and by then everything is written against
it. That is ADR 0005's "the shape is the expensive part", made mechanical.

**A snapshot is a copy, and saying so is not a retreat from ADR 0002.** A
borrow cannot cross a process, so what crosses is a description of the agent
tree at one instant. It is safe to act on a moment later for exactly the reason
ADR 0002 refuses coordinates: it carries node identity, ADR 0003 never reuses
one, and a verb naming a node finds the same node or nothing. A test pins that
the snapshot's outline is *character for character* the tree's — if those ever
differ, one of them is the second structure ADR 0002 forbids.

**The pipeline moved out of the corpus.** There is one of it now, inside the
renderer, which is where it will be when there are processes. The corpus still
reaches in for the box tree, the layout and the display list, and that is
correct: it is a test of the engine's insides and ADR 0005 says tests stay
single-process.

**And it caught an overclaim, which is the best thing it did.**
`docs/conformance.md` said an agent can *"act on"* a rendered page. Trying to
use the boundary end to end — put text into a field, then read the tree back —
showed that nothing happens. `perform` finds its target, refuses what cannot be
operated, and reports what it decided; it never writes back into the document.
The verbs have been like that since they were built and no test had ever asked
the second question. The docs now say what is true, the test pins what is true
rather than what the name suggests, and **queue item 42** is where it stops
being true.

That is worth naming plainly: *"typed verbs"* promises an agent that can drive
an interface, and today it is an agent that can describe what it would do.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 786 tests, no stubs, boundaries held, no verb takes a coordinate.

**What the next iteration should know.** Item 26 is a URL and loading what
needs no network, and it is the first item written *against* the boundary
rather than beside it. Item 42 is now the most valuable ★ item in the queue.

---

## Iteration 26 — queue item 42: a verb changes the page

**Taken before item 26**, and the reason is written into the queue: this is a
correctness gap in the ★ agent surface, which `CLAUDE.md` calls the reason this
project exists rather than a faster fork of somebody else's engine. An agent
that can only describe what it would do is not an agent.

**What was built.** Text put into a field is in it. A checkbox ticks and
un-ticks. Choosing a radio un-chooses the rest of its group, because a radio
that toggled would leave the page in a state a person could not have put it in.

**Deciding and changing are two steps, and the types say so.** The agent tree
borrows the document, so nothing holding one can change it — and that is right
rather than inconvenient: the decision has to be made against the tree the
agent *read*, and the change applied to the document afterwards.
`alo_agent::apply` is the second half, and the renderer does the three steps in
order: decide, apply, render again.

**Rendered again from the same document, never re-parsed.** This is the part
that would have been a silent disaster. Re-serialising and re-parsing is the
obvious way to "just re-render", and it mints new node ids on every keystroke —
every snapshot anybody was holding goes stale and nothing says so, because a
stale id would name a *different* node rather than none. ADR 0003's whole
promise depends on the document surviving, so `render_document` takes it by
value and hands it back. There is a test that holds an id across a change and
uses it.

**It found three more things, all of them real.**

1. **A `<label>`'s words were read twice** — once as loose text, once as the
   control's name. Every labelled field on every form therefore answered to its
   own name *ambiguously*, and the verbs correctly refused to guess. alo's own
   sign-in screen read "Email" twice. A label that names a control is now read
   through, because those words have already been read.
2. **A password field was in no tree at all.** ARIA gives `<input
   type=password>` no role on purpose, so that a screen reader does not read a
   password back — and a Generic box is only exposed when it was named. So the
   engine had an agent that could not sign in to anything. Role says what a
   thing *is*; a new `takes_text` capability says what can be *done* to it, and
   the two come apart on exactly this element.
3. **A field did not show what it held.** An `<input>`'s value is an attribute
   with no box, so typing into one changed nothing on the screen. It now
   generates the text box nobody wrote, which is what CSS says the inside of a
   replaced control is — and a password draws one dot a character. Those dots
   are *not* in the agent tree: they are a rendering of a secret, not something
   to read.

**And it left one, written down as item 43.** A checked checkbox draws the same
box as an unchecked one. The state is right in the tree and wrong on the
screen, which is the worse way round — the agent is correct and a person
looking at the same page is misled.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 797 tests, no stubs, boundaries held, no verb takes a coordinate.
Fifteen corpus cases; the new one, `a-filled-form`, is a form with a typed
email, a masked password and a ticked box.

**The roadmap line this item served** is stage 1's ★ *Typed verbs*, and it was
already ticked — ticked while a verb decided, reported, and changed nothing,
which is not what the line says. The line now says what a verb does and records
that it was ticked early. That is the only honest way to move a line that
should not have been ticked in the first place.

**What the next iteration should know.** Item 26 — a URL, and loading what
needs no network — is next in the roadmap's order. Item 43 is small and
visible. `alo-renderer/tests/signing_in.rs` is now the closest thing this
repository has to the thing it is for: an agent reading alo's sign-in screen,
filling it in by name, and reading back what it did.

---

## Iteration 27 — queue item 44: `clamp()`, `min()`, `max()` and the viewport units

**Stage 1 first.** The gate correction says stage 1 finishes before stage 2
continues, so the queue's stage 1 remainder is what gets worked — not item 26.

**What was built.** The four math functions as one family, and the four
viewport units. `font-size: clamp(2.4rem, 4vw, 3.5rem)` — alo's own headline —
resolves.

**The best evidence is the thing that did not happen.** The committed reference
render for `alo-sign-in` is **unchanged**. Somebody had worked out by hand that
the clamp comes to `2.5rem` at the thousand pixels the case renders at, written
that in, and said so in a comment. Replacing the hand-worked value with the
screen's own produced the same 40 pixels and the same picture. That is what a
faithful substitution looks like when it is finally paid off, and it is why the
corpus was worth building.

**One design decision worth keeping.** A viewport unit needs a window, and
sometimes there is not one — a test, a measurement taken before a page has a
size. `FontMetrics` therefore carries `Option<Viewport>`, and a viewport unit
resolved without one is **zero** rather than a plausible number. A plausible
number is the kind of wrongness nobody traces; a zero is visible immediately.

**Where it had to reach that nobody would guess.** `font-size` is resolved by
its own code path, which built its own `FontMetrics` from the parent and root
sizes — with no window. So the clamp took its floor and the headline came out
38.4 instead of 40, and the only sign was one wrong number in a diff. Resolving
a font size is exactly where a design system writes `clamp(…, 4vw, …)`, so that
path needed the window too.

**Parsed as one family, so they nest.** `clamp(1rem, min(4vw, 30px), 5rem)` is
a value. Each is type-checked once, at parse time — the smaller of a length and
a number has no answer, and is refused rather than guessed at, which is the
rule `calc()` already had. And `clamp(a, b, c)` is `max(a, min(b, c))`, so when
the bounds cross the **lower** one wins: CSS's rule, and not Rust's `clamp`,
which refuses a reversed range.

**The item was cut, as its own instruction said to be.** The original item 44
asked for all four substitutions and told the loop to cut it rather than leave
one in place. The other three are now items 47 (`white-space: pre-line`), 48
(`letter-spacing`) and 49 (`transition`, `:hover`, `:focus-visible` — accepted
rather than dropped, since on a static render of a settled page they change
nothing and the honest work is reading them without refusing them).

**The roadmap line this item served** is stage 1's *A real alo screen renders
correctly*, and its `· Built: … · Owed: …` clause moved: the headline's clamp is
now in Built, and Owed names three substitutions instead of four.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 809 tests, no stubs, boundaries held, no verb takes a coordinate.

**What the next iteration should know.** Item 47, `white-space: pre-line`. It
is a rule in the inline formatter rather than a new box, and the headline it
unblocks is three lines of one string — so the box tree for that case should
get *smaller*, which is a diff worth reading carefully.

---

## Iteration 28 — queue item 47: `white-space`

**The item was one substitution; the work underneath it was a property nobody
had implemented at all.** `pre-line` only means something if there is
whitespace processing to be an exception to, and there was none: the engine
shaped whatever bytes the parser handed over. `one   two` was three spaces on
the screen. An indented paragraph was drawn with its indentation in it. Nobody
had noticed because the corpus's markup is written without stray whitespace
inside its text — which is a good reminder that a corpus tests what it contains.

**What was built.** All five values. Runs of whitespace collapse to one space;
`pre-line` keeps the newlines; `pre` and `pre-wrap` keep everything; `pre` and
`nowrap` refuse to wrap. And `<pre>` preserves its whitespace for the first
time — `user_agent.rs` has said `pre { white-space: pre }` since it was
written, and nothing had ever read it.

**Two questions, answered in two places on purpose.** *What survives* is a fact
about the text, so it is settled when the box is built and layout, paint and
the agent tree all read the same string. *Where a line may break* is a fact
about the line, so it stays in the line builder: a kept newline is a break that
**must** happen, and `nowrap` forbids the ones that may. Collapsing in two
places would eventually disagree, which is ADR 0002's argument about two trees
in a smaller form.

**The substitution is gone.** `alo-sign-in`'s headline is now one string with
newlines in it, the way `alo-workplace` writes it, with `white-space: pre-line`
in the case's own stylesheet. The rendered screen is the same shape it was.

**The roadmap line this item served** is stage 1's *A real alo screen renders
correctly*; its Owed clause names two substitutions now instead of three.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 819 tests, no stubs, boundaries held, no verb takes a coordinate.

**What the next iteration should know.** Item 48, `letter-spacing`. It reaches
`alo-text` — it changes what a run measures, so it changes where every line
breaks, and the sign-in headline's `-0.02em` may well stop it wrapping. That
would move the reference render, and the diff is the review.

---

## Iteration 29 — queue item 48: `letter-spacing`

**What was built.** Extra room after every character, applied **where the text
is measured**. That is the whole decision: spacing changes what a run is worth,
so it changes where every line breaks. A version that only moved the pen at
paint time would have drawn different text from the one the line was made of,
and the two would have disagreed by exactly the spacing.

**Shaping is untouched.** `alo_text::spaced` adds the room after every glyph
*afterwards*, so the `rustybuzz` boundary is where it was. Shaping is the
rented part; letter spacing is a CSS decision about the result, and putting it
inside the shaper would have made it look like the shaper's.

**The test font honours it too**, which is not a nicety. `BlockFont` ignored
the style entirely, so the first version of the layout test passed with the
spacing never reaching the measurer at all. A fake that cannot be told apart
from the real thing being broken is not a useful fake.

**alo's headline is four lines instead of five.** `-0.02em` at 40 pixels is
enough for "Your servers." to fit. It still wraps one line more than the real
screen does, and that is a **font** difference rather than an engine one: the
corpus renders in DejaVu Sans and the app loads Inter, which is narrower. Web
fonts are stage 2, and `docs/conformance.md` now says so rather than leaving it
to be discovered.

**The roadmap line this item served** is stage 1's *A real alo screen renders
correctly*; its Owed clause names one substitution now instead of two.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 823 tests, no stubs, boundaries held, no verb takes a coordinate.

**What the next iteration should know.** Item 49 is the last substitution, and
it is the odd one: on a static render of a settled page a transition has
already run and nothing is hovered, so the honest work is to **read** those
rules without recording a refusal and to make `:hover` and `:focus-visible`
match nothing rather than drop the whole rule. Nothing there claims animation;
that needs a clock.

---

## Iteration 30 — queue item 49: the last substitution

**It needed no new code, and that is the point of the item.** The engine
already read `transition` — an unknown property is kept rather than refused,
which queue item 2 decided — and `:hover` and `:focus-visible` already parsed
and already matched nothing, which `alo-css/src/matching.rs` has said in a
comment since it was written. What was owed was **finding that out and putting
the rules back**.

A substitution nobody re-checks outlives the reason for it. Three of these four
were real gaps; the fourth had stopped being one and nothing told anybody.

**alo's sign-in screen now renders from its own stylesheet, rule for rule, with
no substitutions.** What the corpus diffs on every run is alo's screen rather
than a modified one.

**Two things about the case are transcriptions rather than substitutions**, and
both are written in its own stylesheet: the design tokens are declared inline
because `tokens.css` lives in a repository this one only reads, and Tailwind's
preflight is the one `box-sizing` rule that matters rather than the whole of
it. `docs/conformance.md` now says so, because "no substitutions" would
otherwise be read as more than it is.

**That the rules change nothing is correct rather than missing**, and two tests
pin it: a still picture of a settled page is what a transition has finished
doing, and nothing is hovered because there is no pointer. If a later change
started dropping those rules, the tests would say so.

**The roadmap line this item served** is stage 1's *A real alo screen renders
correctly*. Its Built clause now says the case carries no substitutions; its
Owed clause is down to Settings (item 45) and an agent reading it (item 46).

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 825 tests, no stubs, boundaries held, no verb takes a coordinate.

**What the next iteration should know.** Item 45: alo's Settings screen in the
corpus. It is the second screen the exit gate names and it is not rendered at
all. `alo-workplace` is checked out beside this repository — the screen's
markup and rules are there to be read, the way the sign-in case was built.

---

## Iteration 31 — queue item 45: alo's Settings screen

**What was built.** `crates/alo-corpus/cases/alo-settings/`:
`alo-workplace`'s own Settings screen — its markup, its rules from
`SettingsModal.module.css` and the `ds/Modal` shell it sits in, its colours
from `tokens.css` — rendered and diffed on every run. **No substitutions**, and
that includes the narrow-screen `@media` block, which is evaluated at this
width rather than assumed away.

**It found two engine defects before it rendered right, and they were the same
defect.** A form control needs a **box of its own** to hold what it shows.

1. **A button's label was centred by `justify-content` in the user-agent
   sheet.** alo's settings nav makes each item `display: flex` with
   `text-align: left`, and every one came out centred — because the user-agent
   sheet's `justify-content: center` was still in force and **an author cannot
   override a rule they cannot see**. The apology for that rule had been sitting
   in the sheet since item 11 ("a centring flex container is the same
   arrangement said in a way this engine already has"). It is not the same
   arrangement, and a real screen is what showed the difference.
2. **Every `<input>` had a fixed `height: 1.2em`**, added so an empty field
   would not be a hairline. Once item 42 made a field show its value, that
   fixed height was too *short* for it and the text hung out of the box. A
   *minimum* is right either way, and a minimum is what a content box gives.

**The fix is what browsers do**: `alo_box::Purpose::Control` — an anonymous box
inside a control, made only while the author has left the control a flow
container. Make it a flex container and the box is not generated, so the
author's own alignment is the only alignment. It fills the control, is one line
tall at least, and centres what is in it for a button and not for a field.

Three places had to agree on that: `alo-box` builds the box, `alo-layout` gives
it a style with no element to read one from, and the inline formatter centres
its lines. The box tree's own word for *why the box exists* is what each of them
reads — which is the same shape as `Purpose::Run` and the reason the enum has
two variants rather than a boolean somewhere.

**Both screens the exit gate names now render**, from their own stylesheets,
diffed on every run.

**The roadmap line this item served** is stage 1's *A real alo screen renders
correctly*. Its Built clause now names both screens; its Owed clause is down to
one item.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 829 tests, no stubs, boundaries held, no verb takes a coordinate.
Sixteen corpus cases.

**What the next iteration should know.** Item 46 is the last of stage 1: an
agent reads Settings as a tree and activates a row by name. Everything it needs
exists — the agent tree, the verbs, and now the screen. What it adds is
asserting it against a real alo screen, which is where a role declared wrongly
actually shows up. The settings nav is `<button>`s inside a `<nav>`, so what an
agent should find is buttons named "General", "Filters & rules" and so on.

---

## Iteration 32 — queue item 46: an agent on alo's Settings screen

**What was built.** `crates/alo-renderer/tests/an_agent_on_settings.rs`: seven
tests that read alo's Settings screen **from the corpus case on disk**, so the
test and the committed reference render are looking at the same bytes. The
agent finds the sections as buttons called what a person would call them, knows
which one is open, activates one by name, ticks the out-of-office box, types a
date, reads it all back, and is refused when it asks for something the screen
does not have.

**It found one thing missing, and it was the sentence ADR 0002 opens with.**
`aria-current` was dropped. alo's nav says which section is open and the tree
could not say it — an agent would have had to guess from a colour, which is
exactly the screen-scraping ADR 0002 exists to refuse. It is read now, and as
the **word** the author used rather than a flag: a nav item being the current
*page* is not the claim a cell being the current *date* makes.

**And one thing is said out loud rather than papered over.** Pressing a nav row
runs the page's own code, and stage 1 has none — so the verb finds the row,
reports what it pressed, and the screen does not change. There is a test that
asserts exactly that. The day scripting arrives it will fail, and somebody will
have to rewrite it on purpose, which is the point.

---

# Stage 1 is finished

**The exit gate is met, all three clauses**, and `ROADMAP.md` says so:

- alo's **sign-in screen** renders from its own stylesheet with no
  substitutions, diffed against a committed image *and* a committed box tree.
- alo's **Settings screen** does the same, its narrow-screen `@media` block
  evaluated rather than assumed away.
- An **agent reads Settings as a tree and activates a row by name**, never by
  position.

Every one of those runs on the machine anybody clones this on. No GPU, no
window, no network, no sibling repository, and no claim measured on hardware.

**What the gate does not claim**, said here so a tick is not read as more than
it is:

- **This is an engine, not a browser.** No scripting, no network, no hostile
  pages. Stage 2 is the rest, and `ROADMAP.md` sizes it honestly at years.
- **The screens are not pixel-identical to the app.** The corpus renders in
  DejaVu Sans and the app loads Inter, which is narrower, so alo's headline
  wraps one line more here. Web fonts are stage 2. `docs/conformance.md` says
  it in the same paragraph as the passing row, so nobody reads the row alone.
- **Nothing about speed.** Measured on hardware or not said.

**Twenty-three stage 1 queue items**, plus the ten written by the iterations
that found them, all ticked. 836 tests. Sixteen corpus cases. Five ADRs.

**Stage 2 begins at queue item 26**, and the first two lines of it are already
part built: ADR 0005 decided the process model and `alo-renderer` is the
boundary it needs, so what item 29 owes is the transport and the sandbox rather
than a redesign.

---

## Iteration 33 — the loop for stage 2

**This served no roadmap line, and here is why.** `LOOP.md` step 6 requires
either a line moved or a reason. This iteration wrote the loop's own rules and
the queue that drives it; it built nothing a roadmap line describes, and ticking
one would have been the exact failure step 6 forbids. `ROADMAP.md` changed in
one place only — the process-model line's Owed clause, because the item it
names by number was renumbered.

**What stage 2 needed that stage 1 did not.** Stage 1 had one question and one
answer: does alo render, and here is the committed PNG. That measure does not
exist here, and a loop with no measure marks things done because they compile.
So `LOOP.md` gains four rules:

1. **A real page decides, and the page is frozen.** The roadmap already says the
   trigger is a page that fails; what it did not say is that the page must be a
   *copy*. A suite that fetched would be flaky, would fail on an aeroplane, and
   would hand every site's owner the ability to break our build. And freezing is
   not licence to scrape: take the smallest thing that fails, from a site whose
   terms allow it, and write down where it came from.
2. **The bytes are hostile now.** Stage 1 rendered markup we wrote. The lints
   already forbid `unwrap`, `panic` and indexing; what they do not catch is
   arithmetic that overflows on a hostile length, and a rented crate that panics
   on input we passed straight through. So the gate gains a clause: anything
   reading bytes from outside gets a malformed-input test and returns an error.
   In a renderer, a crash is a denial of service.
3. **Order follows dependencies.** Ninety items with real ordering constraints
   cannot be worked top to bottom. Each item names what it depends on, and the
   loop takes the first whose dependencies are done — which is not always the
   first in the file.
4. **A decision is its own iteration.** Eleven items are marked *needs ADR*: the
   JavaScript engine, the garbage collector, cookie and DNS defaults, request
   attribution, permissions, the quota policy, which codecs we rent, PDF,
   credentials, and an agent crossing frames. ADR 0005 came before
   `alo-renderer` and that is the shape to keep — a decision made inside a
   commit that was mostly code is a decision nobody reviewed.

**The queue.** Eighty-six items in ten groups, from the roadmap's ninety lines.
The network first, because every security decision is made against the origin
and nothing there needs JavaScript. Then origins and the process split — placed
after `file:` and `data:` loading, because a sandboxed renderer cannot fetch and
the browser process has to be able to. Then a frozen page. Then JavaScript, the
long pole. Then the DOM, CSS, text, pictures, speed, the browser itself, and the
agent on somebody else's pages.

**Numbering starts at 50, and 26 to 41 are retired.** Those were a sixteen-item
sketch written before the roadmap grew the real list. Reusing them was tried
once already this session and produced two items numbered 20 — a reference that
points at the wrong work and says nothing about it.

**Two things the queue says out loud** so a later iteration does not discover
them:

- **Item 81, events, is what makes a button do something.** Every agent verb has
  been honest since stage 1 that pressing a nav row changes nothing. The item
  says its closing condition is that `alo-renderer`'s test asserting exactly
  that *fails* and has to be rewritten.
- **Item 105, web fonts, closes the last honest gap in stage 1's screens** — the
  corpus renders in DejaVu Sans and alo loads Inter, so its headline wraps one
  line more here.

**Items 107 (SVG) and 129 (developer tools) are marked "cut before starting".**
Each is several products, and discovering that halfway through an iteration is
how a half-built thing gets committed.

**The gate.** `scripts/gate.sh` green. No crate changed — this iteration is the
plan rather than the work — so the tests are the 836 that were already passing.

**What the next iteration should know.** Item 50, URLs. Nothing depends on
nothing else, everything in section A depends on it, and the WHATWG test suite
is a table it can be checked against on this machine.

---

## Iteration 34 — queue item 50: URLs and origins

**Stage 2 begins.** `alo-url`: URLs in parts, and the origin every security
decision is made against.

**Rented, and the reason is worth stating.** Parsing is `url`'s, behind one
file, which the gate now checks like every other rented crate. It drags IDNA in
with it — the Unicode specification deciding whether `аpple.com` written in
Cyrillic is the same host as `apple.com`. That is a **security** question whose
answer is a table, and writing our own would be effort spent on the part of a
browser nobody notices us doing well and everybody notices us doing badly.
ADR 0001's own words for `html5ever` and `cssparser`, applied unchanged.

**The one thing that is a type rather than a convention.** An **opaque origin
is the same as itself and nothing else**. Two `data:` URLs with identical bytes
are two origins; if they were one, every `data:` frame on a page could read
every other one. So `Origin::Opaque` carries an identity minted once and never
reused — the same argument ADR 0003 makes about nodes, for the same reason: a
value that could be recreated could be impersonated.

`file:` is opaque too, and every scheme this engine has not been told about.
One local file reading every other one is the oldest exfiltration bug there is,
and **unknown must never mean "probably fine"**.

**The boundary check caught something worth keeping.** Our own module was called
`url`, so `crate::url::` matched the rented crate's name and the gate refused
it. That is the check working: a module that shadows a rented crate's name is
exactly how a boundary stops being checkable. Renamed to `parts` — which is
what the file's own first line already called it.

**First item under the new hostile-input rule.** The test feeds the parser empty
strings, hundred-thousand-character hosts, a thousand colons inside IPv6
brackets, right-to-left overrides, null bytes and ten thousand percent signs,
and requires an *answer* rather than a panic. In a renderer a crash is a denial
of service, and a URL is the first thing a stranger controls.

**The table is ours and written down**, not fetched — `LOOP.md`'s frozen rule.
Every row is a case from the URL Standard's own text, small enough that a person
can check it by eye. It is not the whole of `web-platform-tests`, and the test
file says so.

**The roadmap line this item served** is stage 2's *URLs, properly*, now ticked
with what built it.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 851 tests, no stubs, boundaries held — `url` joined the nine crates
already checked — and no verb takes a coordinate.

**What the next iteration should know.** Item 51: fetching what needs no
network. It is the *shape* of a load — request, response, status, headers,
content type, body — with `file:` and `data:` as the only schemes, so that the
network in item 53 is one implementation of something already tested. The
encoding sniffing matters more than it sounds: a mislabelled page is common,
and the rule is HTML's rather than "assume UTF-8".

---

## Iteration 35 — queue item 51: the shape of a load

**What was built.** `alo-net`: a request, a response, a status, headers, a
media type and a body — with `data:` and `file:` as the only schemes and **no
network in it at all**.

**That absence is the design.** The shape is identical whether the bytes came
from a socket, a file or the URL itself, so it is built and tested against the
two that need nothing. HTTP is then *one more arm of a `match`* in `fetch.rs`,
and the comment saying so is in the file. A pipeline built network-first would
have had the network's shape pressed into everything above it.

**It lives in the browser process, and that is a privilege boundary.**
ADR 0005 gives a renderer no filesystem, no network and no way to name anything
outside itself. So the renderer is *handed* a fetched response —
`Page::from_response` — rather than a path to go and read. The test for it does
the fetching outside the renderer, which is what the split will look like when
item 63 makes it real.

**Rented the tables, kept the algorithm.** Which byte means which character in
`windows-1252` or `shift_jis` is twenty years of industry agreement in a table,
so `encoding_rs`, behind one file. But *which* encoding a page is in is a
**sequence of rules** — byte order mark, then `Content-Type`, then a `<meta>`
in the first kilobyte, then UTF-8 — and each step is there because the one
before it can be absent or wrong. That is ours, and it is where a browser
actually gets mojibake right or wrong.

**Two decisions worth keeping.**

- **A page that decoded badly says so.** `Decoded::had_errors` is kept rather
  than hidden. A browser that silently produced question marks would leave
  nobody able to find out why.
- **Headers are a list, not a map.** Names fold case, but order is observable
  and `Set-Cookie` appearing three times means three cookies. A
  `HashMap<String, String>` loses both, and loses the second one silently.

**A judgement call: base64 is ours, not rented.** Twenty lines, and a
dependency for twenty lines is its own kind of cost. Everything else in this
crate that is a table is rented.

**The roadmap line this item served** is stage 2's TLS line — not because this
item did any TLS, but because what it built is the shape TLS and HTTP arrive
into. Its Built clause says exactly that and its Owed clause says the whole of
TLS is still owed, so nobody reads the clause as progress on the thing it
names.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 884 tests, no stubs, boundaries held — `encoding_rs` joined the list —
and no verb takes a coordinate.

**What the next iteration should know.** Item 52, TLS with `rustls`. The
interesting half is not the handshake, which is rented: it is that **a
certificate error is a decision a person makes**, so the error has to say what
is wrong and what trusting it would mean, and must not be bypassable by
default. That is a design decision inside an item, and it is the part to get
right.

---

## Iteration 36 — queue item 52: TLS, and what a person is told when it fails

**The handshake is rented and took an afternoon. The sentence took the
thought.**

`rustls` behind one file, `ring` as the provider — both providers carry C,
`ring` is the smaller and more widely audited, and that C is precisely what
ADR 0005's second reason for the sandbox is about. Which is also why a renderer
never speaks TLS: the handshake is the browser process's, and the renderer is
handed bytes.

**What is ours is the refusal.** Every browser has arrived at the same place: a
full-page interstitial saying *your connection is not private*, and a button
that goes on anyway. People press the button — because the page tells them
neither **what is wrong** nor **what pressing it would mean**, so the only
information they have is that they wanted to see the page.

So `Refused` carries three things and a caller cannot show one without having
the others: what is wrong, in a sentence; what trusting it anyway would mean,
in a sentence; and whether the fault has an innocent explanation at all. An
expired certificate, a wrong clock and an organisation's own authority all do —
they happen constantly and none is an attack. A wrong host does not: it is what
an interception looks like, and no amount of a person's confidence changes what
the bytes say.

**Not bypassable *at all*, which is more than the item asked for.** The item
said "not bypassable by default". There is no flag, no constructor and no
feature: `rustls` makes an accept-everything verifier possible and this file
does not do it and does not expose the seam. And the closest thing the API can
express goes the safe way — `Trust::of(&[])` trusts *nothing* rather than
everything, and there is a test that says so.

**Trust is the operating system's.** Not a bundle compiled into us: an
organisation running its own certificate authority has already told the OS
about it, and a browser that ignored that is one nobody in an organisation can
use. A bundle we shipped would also go stale the day after.

**The boundary check caught something again, and again it was right.** The
tests were written in `tests/`, where they had to name `rustls` to start a TLS
*server* — and the gate refused. So they live in `tls.rs` now, which is the
honest place: the file that may name the crate is the file that tests it.
That is twice in three iterations the boundary has found something, which is a
good sign about the rule rather than about my typing.

**No network anywhere.** A certificate authority is made at test time, a server
runs on `127.0.0.1` with an ephemeral port, and the client trusts exactly that
authority. It is a real handshake down a real socket with real validation, and
it works on an aeroplane.

**The roadmap line this item served** is stage 2's TLS line, now ticked with
both items that built it.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 895 tests, no stubs, boundaries held — `rustls` joined the list — and no
verb takes a coordinate.

**What the next iteration should know.** Item 53, HTTP/1.1. The parsing is
ours and it is the most hostile input in the crate so far: a status line, a
header block and a body whose length a stranger declares. Every one of those is
a place where a length that overflows or a header count that is unbounded turns
into a denial of service, and `LOOP.md`'s hostile-bytes rule is aimed straight
at it. Connection pooling and keep-alive come with it, and `secure` is already
the right shape to put a socket through.

---

## Iteration 37 — the loop, to the end of the roadmap

**This served no roadmap line**, and `LOOP.md` step 6 asks me to say so rather
than tick something. It is the loop's own scaffolding: the queue now covers all
four stages, and the rules for crossing between them are written down. No
roadmap line describes that, and ticking one to discharge the obligation is the
exact failure step 6 forbids.

**What changed my mind about writing stages 3 and 4 now.** The stage 2 queue
said they would get one "when stage 2 is close enough that the order matters",
because writing them early is planning work whose shape two years decides. That
reasoning was about *ordering*, and it still holds — which is why almost every
item below stage 2 is written `blocked` rather than ordered. What the queue was
missing is not an order. It is the ability to **say what is next at every
point**, including at the two points where what is next is a person.

**The two boundaries a loop must not cross.**

- **Stage 2's exit gate is a judgement**: *a person uses it as their browser for
  a week and reaches for another one only for a site they can name.* No
  iteration can certify that. So when stage 2's queue empties, the loop writes
  `LOOP COMPLETE`, names what a person has to do, and stops. That is the honest
  answer to a gate about somebody's experience, not a failure to find work.
- **Stage 3 is opened by pages, not by the queue.** Every item is `blocked: no
  page yet` and that is its actual state. A loop taking one because it is the
  next unticked line would be building the legacy tail for its own sake, and
  refusing to do that is what made stages 1 and 2 survivable.

**`LOOP COMPLETE` now means something precise**: every remaining item is
blocked, the blocks are real, and the two kinds are listed separately — pages
nobody has hit, and a judgement nobody has made.

**One accuracy fix.** `LOOP.md`'s "Running it" section named only the PowerShell
supervisor when `run-loop.sh` sits beside it in `alo-workplace`. Both are named
now. The supervisor itself was not touched: it lives in that repository by
decision, and this one only ever reads it.

**The gate.** `scripts/gate.sh` green. No crate changed — this is the plan
rather than the work — so the tests are the 895 that were already passing.

**What the next iteration should know.** Item 53, HTTP/1.1, unchanged. Nothing
about this iteration moves the work along; it means the loop never has to guess
what comes after the work it is doing.

---

## Iteration 38 — queue item 53: HTTP/1.1

**Ours, not rented, and the reason is the whole iteration.** The syntax is a
few lines of ASCII. The difficulty is not reading it — it is **refusing the
readings that are almost right**, because nearly every famous HTTP bug is a
parser being generous:

- Two `Content-Length` headers that disagree. A parser that picks one has just
  disagreed with the proxy in front of it about where this response ends and
  the next begins. That is request smuggling in its plainest form.
- `Transfer-Encoding` and `Content-Length` together. The same bug, spelled
  differently.
- A space before the colon — `Content-Length : 5`. Some parsers accept it and
  some do not, and a chain containing both is a smuggling chain.
- A header continued onto the next line by leading whitespace, removed from the
  standard in 2014 for exactly this reason.

All four are refused **by name**, with the reason in the code beside them.

**A truncated body is an error, not a short page.** That is half the item's
closing condition and it is the half worth stating twice: a browser that showed
the first part of a bank statement and said nothing would be worse than one
that showed nothing at all.

**A `204` gets no body however loudly it claims one.** A parser that believed a
`Content-Length` on a `204` would read the *next* response as this one's body,
which is the same class of bug arriving from the other direction.

**Every limit is a named constant.** The longest line, the most headers, the
largest body, the largest chunk. Without them a server can make this process
allocate for as long as it cares to send, and it costs the sender nothing. The
first version of `read_line` used `read_until` with a `take` around it — which
reads the *whole* line into memory and then complains — so it reads a byte at a
time now, and the limit is a limit rather than a hope.

**Scope was cut, as `LOOP.md` asks.** The item said "with connection pooling
and keep-alive". Framing is the half where being wrong is a security bug, and
it deserved the iteration; pooling is item 54, and costs nothing to defer
because `exchange` already takes a stream from anywhere.

**A test caught me rather than the parser.** The chunked fetch failed with
`<p>hello\r</p>!` — because I had written a chunk announcing nine bytes for an
eight-byte string, and the parser correctly ate the carriage return as the
ninth. That is the framing being right about something I had got wrong, which
is the best kind of test failure.

**No network anywhere.** The HTTP server in the tests is thirty lines in the
test file, on `127.0.0.1` with a port the operating system picks, and half the
tests have it speak HTTP badly on purpose.

**The roadmap line this item served** is stage 2's *HTTP/1.1, then HTTP/2*. Its
Built clause names what landed; Owed names pooling (item 54) and HTTP/2 (item
59).

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 923 tests, no stubs, boundaries held, no verb takes a coordinate.

**What the next iteration should know.** Item 54, pooling and keep-alive. Two
things to get right: `Connection: close` comes *out* of the request when a
socket is to be kept, and a pooled connection that a server closed while it sat
idle must be a retry rather than a failure — that race is the one every HTTP
client gets wrong first.

---

## Iteration 39 — queue item 54: keeping a connection

**The change that made pooling possible was not the pool.** It was moving the
read-ahead buffer from the *exchange* to the *connection*. Reading a response
means reading ahead: by the time one body ends, the reader may already hold the
first bytes of the next response. Item 53 threw that reader away between
exchanges — correctly, because there was nothing to reuse — and doing the same
thing with a pool would have left every second request starting in the middle
of a sentence. `Connection` is now the buffer, which is why it is a type.

**A kept connection is a bet.** A server can close an idle one at any moment
and there is no way to be told, so every reuse is a gamble and the interesting
question is what losing looks like. It looks like a retry, and the conditions
are narrow on purpose — all three, or it is a failure:

1. the connection was **reused** rather than freshly opened,
2. **not one byte** of an answer arrived,
3. the method is one where doing it twice is the same as doing it once.

**The third is about the method, not about how likely it seems.** A `POST` that
failed after the server received it is a payment that has happened; sending it
again is a payment that has happened twice. There is a test that makes a `POST`
fail on a dead pooled connection and asserts that **no second socket is
opened** — which is the assertion that would catch somebody later "improving"
the retry into something more helpful.

**The scheme is part of which server a connection goes to.** An `http`
connection is never handed out for an `https` request; doing that would send a
page's cookies in the clear. It is one field in a key and it is worth naming.

**Bounds, because a pool without them is a file-descriptor leak.** Six per host
(what browsers settled on), sixty-four in all, and twenty seconds before an
idle connection is closed rather than gambled on — servers commonly close at
five, so keeping one for minutes means losing the bet nearly every time, and
every lost bet costs a round trip more than opening one would have.

**One thing the tests forced that is a real improvement.** The suite took
thirty seconds, all of it one test waiting out the browser's timeout on a
server that never answers. How long to wait is now a caller's choice: a browser
wants tens of seconds, that test wants half of one. The suite is back to half a
second and the engine gained something it was going to need anyway.

**The roadmap line this item served** is stage 2's *HTTP/1.1, then HTTP/2*; its
Built clause now names pooling and the retry, and Owed is down to HTTP/2 alone.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 930 tests, no stubs, boundaries held, no verb takes a coordinate.

**What the next iteration should know.** Item 55, content encodings — gzip,
brotli, zstd, all rented. The interesting half is not decoding: it is that a
**decompression bomb** is a body that is small on the wire and enormous in
memory, so `LARGEST_BODY` has to apply to what comes *out* rather than to what
came in. That is the hostile-input rule pointed at a new place.

---

## Iteration 40 — queue item 152: bodies that arrive compressed

**The queue had two items numbered 54**, fixed in its own commit first. Pooling
was cut out of 53 and given the next number without noticing a later line
already had it. The number moved rather than the identity: 54 is pooling in
three pushed documents, and renumbering the done item would make them point at
different work than they were written about. A queue number is an identity
allocated once and never reused — ADR 0003's rule for node ids, for the same
reason. Content encodings is 152.

**Three crates rented, all three pure Rust on purpose.** `flate2` on its
`rust_backend` rather than the C zlib it defaults to; `brotli-decompressor`;
`ruzstd` rather than the C `zstd` bindings. ADR 0001 says rent the physics, and
a codec is physics — but a decompressor is also the single place in a browser
where a memory bug is most directly a remote code execution, because the
attacker chooses every byte the allocator sees. That is worth a slower decoder.

**The bound is on what comes out, and that is the only such bound in the
crate.** Every other limit in `alo-net` watches what *arrives*: a
`Content-Length`, a chunk header, a status line. None of them help here, because
compression is the art of arriving small — a gigabyte of zeroes is a megabyte
of gzip and six hundred bytes of brotli. `tests/compressed/bomb.gz` is eight
kibibytes and decodes to eight mebibytes, and is refused.

The limit is a **parameter** with `LARGEST_BODY` as its default, which came out
of the test rather than out of taste: proving the bound at 256 MiB costs a
quarter of a gigabyte per run. The mechanism is identical at 64 KiB. And a
caller that knows a subresource should be small can now say so.

**A test found a real defect in a rented crate's contract.** `ruzstd` computes
a frame's XXH64, and reads the one the frame carries, and compares them for
nobody — both are getters. So a zstd body with a byte flipped in the middle
decoded into rubbish and returned success, which is precisely what this item
exists to prevent. The comparison is ours now and has a test named after it, so
deleting it fails one thing and nothing else.

**And one thing written down rather than promised.** Raw DEFLATE and brotli
carry no integrity check at all. A corruption that leaves a structurally valid
stream decodes to different bytes and no implementation could tell. That is a
property of the formats; what protects those two on the wire is TLS, which is a
different layer doing a different job. The corruption test therefore covers
gzip, zlib-deflate and zstd — and says in its own doc comment why brotli is
tested for a mislabelled body but not a flipped byte.

**The fixtures were made by something that is not us** — the `gzip`, `brotli`
and `zstd` command-line tools, and Python's `zlib`, each re-derivable from the
commands in `tests/compressed/README.md`. A suite that compressed with the crate
it decompresses with proves that one crate agrees with itself, which is not the
question anybody is asking.

**Cut into the queue rather than folded in:** item 153, `Transfer-Encoding` that
is not `chunked`. `Transfer-Encoding: gzip, chunked` is legal and rare, and it
is a *different header* from the one this item undoes — today the chunks come
off and the gzip does not, which hands up compressed bytes labelled as a page.

**The roadmap line this item served** is stage 2's *Content encodings: gzip,
brotli, zstd*, and it is **ticked** rather than annotated: all three are there,
`deflate` in both of its spellings is there, and what is left over went to the
queue as its own line rather than staying owed on this one.

**The gate.** `scripts/gate.sh` green: fmt clean, clippy zero warnings and zero
errors, 945 tests, no stubs, three new boundaries held, no verb takes a
coordinate. Nothing here positions, sizes or paints, so no layout assertion and
no reference render — the gate asks for those of anything visual, and a
decompressor is not.

**What the next iteration should know.** Item 55, redirects and byte ranges.
The security half is the one to get right and it is not the loop bound: it is
**what a redirect drops**. A cross-origin redirect must not carry the
`Authorization` header across, and a redirect to a scheme we do not fetch must
be a refusal rather than a silent stop. `Accept-Encoding: identity` on a ranged
request is already handled — that is why the caller's choice is respected.

---

## Iteration 41 — the supervisor moved into this repository (ADR 0006)

Not a queue item. The owner said the supervisor should be ours and for this
machine rather than `alo-workplace`'s, which overturns a written decision, so
the ADR came first and in its own commit.

**Why the borrowed one had to go, in one sentence:** a dependency that cannot
be maintained by the people depending on it. `LOOP.md` forbids editing
`alo-workplace`, and rightly — so when the command documented here turned out
to be wrong, the only place to fix it was somewhere the loop may not go. It had
been wrong since the file was written: it passed `--repo`, which that script has
never parsed, so the flag became the repository path and this checkout became a
track name. The premise for keeping it away — *"so this repository stays
Rust"* — was already false, because `scripts/gate.sh` is six and a half
thousand bytes of bash sitting next to it.

**And then running it found the defect that mattered.** The journal has
`LOOP COMPLETE` on line 1531 of 2500-odd. Stage 1 finished, said so, and a
person started stage 2 underneath it. Any supervisor that searches the file for
those words — the borrowed one included — stops on its first tick and reports
the queue complete **with ninety-nine items open**. That is the failure that
looks exactly like the work being done, and it would have happened the first
time anybody ran the command I had just handed them.

The rule now: a marker is live only when **no iteration entry follows it**. An
entry written afterwards means a person deliberately resumed, so the marker is
history rather than an instruction — which also means resuming a halted loop is
done by appending, never by editing an old entry.

**What was carried over from the borrowed script**, because its comments are
scar tissue and reading a sibling repository to learn what correct means is
what `LOOP.md` asks for: the anchored marker match, the idle-based hang guard
(a duration-only one once killed ninety minutes of honest work), the
machine-wide lock against detached wrappers spawning rival workers, and backing
off on a non-zero exit rather than spinning into a rate limit.

**What is new:** it refuses to start when `scripts/gate.sh` does not pass, since
an iteration opening on somebody else's red tree will either work around the
failure or spend itself diagnosing it. It has `--once`, `--dry-run` and
`--self-test`, so it can be understood without being let loose. And it stops at
a `LOOP COMPLETE` by *saying what a person now has to decide* rather than
treating the marker as a transient failure.

**What was dropped:** tracks, which belong to `alo-workplace`'s several parallel
queues and were a second concept to understand and then ignore here; and the
GNU `stat -c` fallback, which cannot apply on macOS and which poisoned that
script's age check on Git Bash.

**The gate.** Green, and it gained a step: `scripts/loop.sh --self-test` asserts
the stop rule against seven journals — a plain marker, a heading, a bold one, a
marker quoted mid-sentence, a retired one, two markers where the last wins, and
none at all. The gate runs it, because a rule whose failure mode is *reporting
success* is exactly the rule that must not be allowed to rot. Nothing here
positions, sizes or paints, so no layout assertion and no reference render.

**What this did not do:** a full unattended run. That spawns workers with
`--dangerously-skip-permissions`, which is the owner's call rather than mine.
Argument handling, the stop rule, the lock, the gate refusal and both help paths
were exercised; the spawn loop was read, not run.

**Next iteration** takes queue item 55, redirects and byte ranges. The half to
get right is not the loop bound — it is what a redirect *drops*: `Authorization`
must not cross an origin, and a redirect to a scheme this engine does not fetch
has to be a refusal rather than a silent stop.

---

## Iteration 42 — queue item 55: redirects

**Scope cut before anything was written.** The item was *redirects, byte ranges,
and downloads that resume* — three things sharing a roadmap line and sharing
nothing else. Ranges went to item 154. Scope, never depth: what is here is
finished, and what is not here is a line in the queue rather than a half-built
function.

**The whole design decision is that following is not the interesting part.**
Following a redirect is three lines of loop. Deciding *what to carry across one*
is where every bug lives, and every one of those bugs is a security bug. So
`redirect::next` is a pure function — a request and a response in, a decision
out, no socket anywhere near it — and fifteen of the seventeen tests need no
server at all. A security rule that can only be checked by standing up a socket
is a security rule that gets checked less often.

**What must not cross an origin**, and the case a hand-written check gets wrong:
`Authorization` is dropped when the origin changes, and **a scheme is part of an
origin**, so `https://example.com` → `http://example.com` is a crossing even
though the host is identical. So is a different port. Somebody comparing hosts
would carry a session cookie into the clear on the first of those. `Cookie` and
`Proxy-Authorization` are on the list too; cookies do not exist yet (item 57),
and a list that is already right beats a list somebody has to remember.

**The method rule is a place where the specification is not what to implement.**
301 and 302 say the method is preserved. Every browser has turned a redirected
`POST` into a `GET` since the nineteen-nineties, because servers were written
against that and because silently re-submitting a form somewhere new is worse
than being wrong about an RFC. 307 and 308 exist precisely so a server can ask
for the specified behaviour — those are honoured exactly, body headers and all.
`HEAD` survives all five, since turning it into a `GET` would fetch a body
nobody asked for.

**Two schemes are refused as destinations rather than followed.** This engine
fetches `file:` and `data:` when asked directly, and refuses to be *sent* to
either. A server that could redirect a load into `file:///` would be reading the
disk of whoever opened the page; one that could redirect into `data:` would
choose the bytes *and* inherit the URL they appear to have come from. Refused by
name, not ignored — an ignored redirect is a blank page with no reason in it.

**Three smaller things that are each a real decision.** A `3xx` with no
`Location` is the answer rather than a failure, because a redirect that does not
say where is not a redirect. A relative `Location` resolves against where the
*response* came from, which after one hop is not where the request started. And
the purpose and initiator survive a hop unchanged, so a redirect cannot launder
a request into looking like something else asked for it — which matters to item
61 rather than to today.

**A circle is told from a chain**, and the circle is checked first because it is
the more useful thing to say: twenty distinct URLs is a misconfiguration, two
pointing at each other is a specific bug somebody can go and find. `Trail` keeps
the order rather than a set, because the order is what a person debugging one
wants to read.

**The gate.** Green: fmt, clippy zero and zero, 962 tests. Nothing here
positions, sizes or paints, so no layout assertion and no reference render.

**What the next iteration should know.** Item 56, the HTTP cache. `ROADMAP.md`
calls it out by name — *"subtly wrong here is invisible for months and then
serves somebody a stale bank page"* — and the queue already asks for the shape
that catches it: a table of responses and clocks, asserting hit, miss and
revalidate for each, **including the ones that are only wrong an hour later**.
Freshness is arithmetic and testable; `Vary` is the part that quietly serves one
user another user's page.

---

## Iteration 43 — queue item 56: the HTTP cache

The roadmap singles this one out: *"subtly wrong here is invisible for months
and then serves somebody a stale bank page."* Two design decisions come straight
out of that sentence.

**Nothing in the cache reads the clock.** Every function takes `now`. A cache
that called `SystemTime::now()` internally can only be tested at the moment the
test runs, and the answers that matter are the ones that are **only wrong an
hour later** — which is exactly the class nobody finds by using the browser. The
table in `tests/what_the_cache_serves.rs` asserts pairs either side of an
expiry: fresh at 3599 seconds, not fresh at 3601.

**Age is not "how long we have had it".** A response can arrive already old and
say so in an `Age` header. A cache that counted from arrival grants it a second
full lifetime — which is how one `max-age=3600` becomes six hours of staleness
across a chain of caches. Time in transit counts too: a `max-age=5` that took
two seconds to arrive is fresh for three.

**`Vary` is a contract rather than a header.** What is stored is the response
together with **the request header values it was chosen by**. A later request
matches only if it would have produced the same choice, so a page fetched with
`Accept-Language: fr` is never served to one asking for `de`. An absent header
and an empty one are different, and are tested as different, because a server
may well answer them differently. `Vary: *` is not stored at all: the server is
saying it cannot promise the response answers anything else, and there is no key
that would be right.

**Four files, because they are four responsibilities.** `httpdate.rs` reads all
three date formats and writes the one anything may send — refusing the obsolete
two would make a real `Expires` unparseable, and an unparseable `Expires` means
*already stale*, so strictness there makes a browser slower and never safer.
`directives.rs` parses `Cache-Control` for both ends, because the mistakes are
in the syntax and solving them twice is solving them differently.
`freshness.rs` is the arithmetic and the verdict. `cache.rs` is the store and
the `Vary` key.

**Clippy found a real design improvement.** `Directives` began as seven
booleans and `struct_excessive_bools` refused it. It was right: they are a
**set**, not seven fields, and writing them as fields is what invites the bug
where somebody reads `no_cache` and means `no_store`. They are a `Flag` enum and
a bitset now, and every caller asks in the same words. The lint was not silenced.

**Wired in, not just built.** `Pool` owns the cache, so a load actually uses it:
a second `follow` of a fresh thing does not reach the server, a `304` refreshes
the headers and hands back the stored body, and a write to a URL forgets what
was kept for it. Four socket tests assert that end to end.

**A `304` for something we do not have is an error rather than an empty page.**
Nobody could have sent that conditional request, so handing up the `304`'s empty
body as though it were a page would be a blank screen with no reason in it.

**Cut into the queue:** item 155, the cache on disk, and it *needs an ADR*. What
may be written to a disk other programs can read is a different question from
what may be reused, and it has a different answer for a page behind a password.

**The gate.** Green: fmt, clippy zero and zero, 1004 tests. Nothing here
positions, sizes or paints, so no layout assertion and no reference render.

**What the next iteration should know.** Item 57, cookies, and the queue already
marks it *needs ADR* — partitioned by default is a **product decision** about
who is protected and what it costs, not a parser detail, and it belongs
somewhere a person can argue with it. The ADR is its own iteration, before any
code depends on it. `redirect.rs` already drops `Cookie` at an origin boundary,
so the day cookies exist that rule is in place rather than remembered.

---

## Iteration 44 — ADR 0007: cookies are partitioned by default

Not code. Item 57 is marked *needs ADR*, and `LOOP.md` says such an item gets
the ADR **as its own iteration, before any code depends on it** — the way
ADR 0005 came before `alo-renderer`. A decision made inside a commit that was
mostly code is a decision nobody reviewed.

**The decision.** A cookie is keyed by two things: the site that set it *and*
the top-level site the person was looking at. So a cookie `ads.example` sets
inside `news.example` is a different cookie from the one it sets inside
`shop.example`, and neither can see the other. Partitioning does not remove the
cookie; it removes the **joining**, which is the only part that does the harm.

**The argument the queue actually asked for — who it protects.** Not primarily
somebody worried about advertising. The threat model that decides this is
somebody whose reading history is *evidence*: of an illness, a pregnancy, an
immigration status, a sexuality, an intention to leave. For that person a joined
cross-site identity is not an annoyance, and a default that only protects the
people who know to go and change it protects nobody who most needs it.

**And what it costs, which is the half these documents usually skip.** Federated
sign-in breaks. Embedded payment, comments, support chat and video preferences
break. Corporate SSO breaks, which is the same cost arriving at somebody's job.
Worst of all, **the failure mode is silent**: a blocked cookie is not an error a
site can catch and explain, it is a login that quietly does not stick, and the
person concludes the browser is broken. They are not wrong to.

**One thing I made myself write down.** This is not a settled industry position.
Safari and Firefox do it; Chrome announced the end of third-party cookies, moved
the date four times, and abandoned the plan in 2024. So *"every other browser
does this"* is **not** available to us as a justification, and using it would
have been dishonest. What is available is that we have no advertising business
and no compatibility debt — which is a reason we *can*, not a reason it is free.

**Why take the cost.** Both costs are real; they are different shapes. A site
that wanted a cross-site credential and did not get one is a breakage somebody
can **see**, and can be asked about. A joined profile is a harm nobody can see
and cannot be undone once joined. Between a breakage somebody can see and a harm
nobody can, the ADR takes the breakage.

**The escape hatch is specified by what it must not be.** A per-site grant, made
by the person, naming who is asking and inside what. Never a global "allow
third-party cookies" toggle — support pages tell people to turn those on and
nobody turns them off — and never an allowlist we ship, because a list of sites
this browser trusts with cross-site identity is a business we would then be in,
and being able to sell a place on it is a pressure we should not have.

**One rule is cheap now and expensive later, and that is why it is in this
file.** `HttpOnly` binds the scripting engine from the day there is one. There
is no JavaScript in stage 1, which makes this the cheapest possible moment to
write the rule down and the worst possible moment to skip it: honoured from the
first commit it costs nothing, retrofitted it is a security review.

**And a way to find out it was wrong.** If the escape hatch is used constantly,
the default is not protecting people — it is annoying them into clicking yes,
which is worse than not having it. That is a measurement rather than an
argument, and it is written into the ADR as the signal to come back.

**The gate.** Green. Documentation-only, so no tests changed; the mechanical
half passed and the half a script cannot check is satisfied by the ADR being
what changed.

**What the next iteration should know.** Item 57's code, now that its decision
exists. Two things the ADR settles that the parser must not be able to lose: a
cookie **carries its partition** — no code path may produce one without it — and
a `__Host-` cookie that does not meet the prefix's conditions is **rejected**
rather than stored under a different name, because the whole value of a prefix
is that a server can trust the name. `redirect.rs` already drops `Cookie` at an
origin boundary, so that rule is in place rather than remembered.

---

## Iteration 45 — queue item 57: cookies, partitioned by default

ADR 0007's code, the iteration after its decision.

**The promise is kept by the shape, not by memory.** Every lookup on the jar
takes a partition, and **there is no function that returns the unpartitioned
set**. A caller cannot accidentally get one, because there is nothing to call.
That is the difference between a decision that survives a year of changes and
one that survives until somebody is in a hurry.

**The test that would catch the whole ADR being undone** is the first in the
file: one embedded server, two top-level sites, and no way to tell the person is
the same person. It sets `id=aaa` inside `news.example` and `id=bbb` inside
`shop.example`, and asserts each is invisible from the other.

**`SameSite=Lax` by default, and the middle case is the valuable one.** A `Lax`
cookie rides a **navigation** from another site — clicking a link to your bank —
but not anything a page *embedded*, which is exactly what an attacker's form
post or image request is. That one line removes a class of CSRF, and it is the
highest-value default in the ADR measured in bugs that stop existing.

**The prefixes are enforced rather than parsed.** A `__Host-` cookie that is not
`Secure`, or carries a `Domain`, or whose `Path` is not `/`, is **rejected** —
never stored with the prefix quietly relaxed. The whole value of a prefix is
that a server reading the name back can trust what it implies, and a browser
that stored it anyway would have removed that value without telling anybody.

**One small thing that is a real bug in other people's code.** Domain matching
requires the dot: `evil-example.com` is not a subdomain of `example.com`, though
a comparison by string suffix alone says it is. There is a test named after it.

**Something partitioning makes possible rather than merely safer.** "Clear this
site's data" now means every cookie anybody set *inside* that site, not only the
ones it set itself. Unpartitioned, that second set was unreachable — you could
not have offered the feature.

**Two things written down rather than assumed.** The site boundary is the
**host** today, which is stricter than the registrable domain a public suffix
list would give: `a.example.com` and `b.example.com` are separate sites where
they should be one. Stricter is the safe direction to be wrong in, and it *is*
wrong — queue item 156. And the escape hatch is item 157, blocked on there being
an interface to ask a person in.

**The gate.** Green: fmt, clippy zero and zero, 1025 tests. Nothing here
positions, sizes or paints.

**On the loop.** The owner asked for the supervisor to be started; the launch was
refused by this environment's permission classifier, which is the right refusal —
`scripts/loop.sh` spawns workers with `--dangerously-skip-permissions`, and that
is not a thing an agent should be able to start on its own behalf. It has to be
run from a terminal. Nothing about the loop is broken; it was checked with
`--dry-run` immediately before, and reported a clean tree and no stop marker.

**What the next iteration should know.** Item 58, DNS, and it is *needs ADR* for
the same reason 57 was: encrypted DNS means a different server sees every name
you look up, and choosing which one is a decision about who to trust rather than
a protocol detail. The ADR is its own iteration.

---

## Iteration 46 — ADR 0008: DNS is the machine's choice

Item 58 is marked *needs ADR*, so this iteration is the decision and nothing
else, the way ADR 0007 came before item 57's code.

**Why it is a decision at all.** DNS is the one place where every site somebody
visits appears in a single stream, in order, with timestamps. Not the pages, but
the names — usually enough. It is the most complete record of a person's
browsing that exists anywhere, and it is produced whether or not anybody asked
for it. So "which resolver" is a question about **who holds that record**, and
answering it silently is what this ADR exists to prevent.

**The thing I had to think hardest about, and got wrong on the first pass:**
encrypted DNS is not straightforwardly better. It does not make the record go
away — it **moves** it. Plain DNS scatters your browsing across whoever runs the
network you happen to be on. Encrypted DNS concentrates it at one resolver,
globally, tied to your IP, across every network you ever join. That is a smaller
number of watchers holding a much better record, and which is safer depends on
who the person is and where they are. A browser knows neither. Firefox learned
this in public in 2019: the objection to defaulting a country's DNS to one
company was not that the company was untrustworthy, it was that nobody had been
asked.

**And a default resolver is a business.** Same object as the allowlist ADR 0007
refused: a slot with enormous value that somebody would eventually offer to pay
for. The way not to be corrupted by that is not to have the slot.

**Why not override the machine either.** The system resolver is where five
things the person already chose live: a VPN, a corporate network's internal
names, a Pi-hole, `/etc/hosts`, and the operating system's own encrypted DNS —
which, when present, means they have already made this decision and we should
not make it again. A browser that resolves its own way breaks all five,
invisibly.

**Two rules the code must carry**, and they are why the ADR exists before it:

- **DNS is never trusted for a security decision.** Any resolver can lie about
  an address; TLS is what stops that mattering, because a wrong address produces
  a certificate error rather than a wrong page. This bounds plain DNS to a
  *privacy* problem rather than an authentication one — and it is why "we use
  encrypted DNS" must never be sold as a security feature.
- **A public name that resolves to a private address is refused.** DNS rebinding
  turns a browser into a way to reach things behind somebody's own firewall.
  Loopback, private, link-local and unspecified ranges are not valid answers for
  a name that came from the public web.

**And what it costs**, said rather than skipped: plain DNS on a hostile network
stays plain for everybody who never opens the setting, which is nearly everybody
— and that falls on exactly the person ADR 0007 was written about. The answer is
to make the choice easy and legible, not to make it silently; if the setting
turns out to be one nobody finds, the fix is in the interface.

**How we would know it was wrong:** if almost nobody ever changes it, the
setting is a decoration rather than a choice. The answer then is a prompt that
asks once, naming the trade — not a default that picks a company and says
nothing.

**The gate.** Green. Documentation only.

**What the next iteration should know.** Item 58's code. Resolution through the
system resolver is what `std::net::ToSocketAddrs` already does, so the work is
mostly the two rules above plus a cache that honours TTL — and the rebinding
rule is the one to write a test for first, because it is the one with an
attacker behind it.

---

## Iteration 47 — queue item 58: names becoming addresses

ADR 0008's code, the iteration after its decision.

**The rule with an attacker behind it turns on who asked, not on the address.**
This is the thing that took the thinking. "Refuse private addresses" sounds like
a property of an address, and written that way it breaks every corporate
intranet and every developer with a name pointed at `127.0.0.1`. The actual rule
is about **causation**: a person typing an intranet name should reach it; a page
on the public web that causes a request to `192.168.1.1` should not. Those two
are indistinguishable if you only look at where the name resolved. So `Reach` is
derived from the request's *initiator* — nobody asking means the person did.

**A page that is itself local may reach anywhere**, because it is already inside
whatever it would be reaching into. And an **opaque** origin — `file:`, `data:`,
anything we cannot judge — gets the restrictive answer, because "we cannot tell
who this is" reads that way or it reads wrong.

**The check that a naive version misses.** `::ffff:127.0.0.1` is not v6
loopback, and nothing in the v6 branch looks at the v4 address inside it. Without
the mapped-address case it walks straight past every rule in the file. There is
a test named after it, and the same for the ranges people forget: `169.254.169.254`
(every cloud's metadata service), `100.64/10` (carrier-grade NAT), `198.18/15`,
`240/4`.

**Connecting takes addresses rather than a name**, and that is a security change
rather than a refactor. `TcpStream::connect((host, port))` resolves the name a
*second* time, inside the standard library, where nothing can refuse a private
answer — and a name that answers differently the second time is exactly the
attack this rule exists to stop. So `Connection::open` lost its `port` parameter
too: an address carries one, and two places to say which port are two places to
disagree.

**A cached answer never carries a permission it was granted earlier.** The
lookup is shared between reaches; the rule is applied afterwards, every time.
There is a test that resolves `localhost` successfully as the person and then
fails as a public page, and asserts it was only looked up once.

**Something written down rather than claimed.** Answers are reused for half a
minute and **that is a guess**. The platform resolver does not return the
record's TTL, so there is nothing truthful to use — a cache that honours real
TTLs needs a DNS client of our own, which is precisely the thing ADR 0008
decided not to build. Saying "30 seconds, and here is why it is not the TTL" is
the honest version.

**One kindness that was also a bug fix:** every resolved address is tried, not
only the first. A host whose IPv6 address is unreachable from this network was a
host this browser could not load on a machine where every other browser could.

**Cut to the queue:** item 158, the encrypted-DNS setting, blocked on there
being an interface to choose in — the same block as item 157's storage-access
grant. Two items now wait on the same missing thing, which is worth noticing.

**The gate.** Green: fmt, clippy zero and zero, 1037 tests. Nothing here
positions, sizes or paints.

**The owner relicensed mid-iteration.** ADR 0009 arrived on `main` while this
was being written: the engine is MPL-2.0 now, not Apache-2.0, so a competitor
cannot take it, improve it privately and sell a better version back. Rebased
onto it; the only conflict was both of us adding to `CHANGELOG.md`'s Unreleased
section, and both entries were kept. That commit says per-file Exhibit A headers
are **owed** and deliberately deferred to avoid colliding with the loop — so
they are now **queue item 159**, because owed work that lives only in a commit
message is owed work one person is remembering.

**What the next iteration should know.** Item 59, HTTP/2. It is the first item
in a while that is a protocol rather than a policy: HPACK, streams, flow
control, and the thing to get right early is that a stream's state machine is
where a peer that misbehaves gets to allocate memory on our side. `MOST_HEADERS`
and the other bounds in `http.rs` have counterparts there and they should be
found before the happy path is, not after.

---

## Iteration 48 — queue item 59: HTTP/2 framing

**Scope cut on starting.** "HTTP/2" is four items, not one: framing, HPACK,
streams and flow control, and negotiating the protocol at all. They went in as
160, 161 and 162 before a line was written, because deciding that halfway
through is how a half-built state machine gets committed. Framing first —
everything else is carried inside it, and it is where a peer chooses how much
memory we allocate.

**The rule the whole file is built around:** a length is checked before anything
is reserved. HTTP/1.1 had two numbers a stranger chose — `Content-Length` and a
chunk size. This has one per frame, several thousand times a page. There is a
test that announces sixteen megabytes, sends nothing, and asserts the refusal
comes before the allocation.

**The classic parser bug, refused by name.** A padded frame's first byte says
how much of the rest is padding, and nothing stops it saying more than there is.
Subtracting without checking underflows; in a language where that is not caught
it reads whatever was next in memory. Rust would panic rather than leak, which
in a renderer is a denial of service — so it is a refusal, not a panic.

**And a test found my own comment wrong, which is the useful kind of failure.**
I had written that padding equal to what remains is "still wrong" because the
length byte is not padding. That is not what the specification says: the
comparison is against the *whole* payload, its own length byte included, so a
frame that is **nothing but padding** is legal and carries an empty body —
servers send them to disguise how large a response is. A check written one off
refuses real traffic. Both sides of that boundary now have a test, and the
comment says what is actually true.

**Two decisions about being generous rather than strict**, and both are the
protocol asking for it:

- An **unknown frame type is ignored**, not refused. Extensibility is on
  purpose, and a peer using an extension we have not heard of is not
  misbehaving. Its length is still checked and its bytes still consumed exactly
  — an "ignore" that lost the stream's place would be worse than a refusal, and
  there is a test that reads an unknown frame and then the real one after it.
- The **reserved top bit of a stream identifier is masked off**, not rejected. A
  reader that forgets sees stream numbers near two billion.

**One error that is not always fatal**, and the type carries the difference: a
`WINDOW_UPDATE` offering no more room kills the connection when it is on stream
zero and only the stream when it is not. Room for nothing is not room, and
unchecked it is a peer that can make this end wait forever.

**Clippy improved the structure again.** A 151-line `match` tripped
`too_many_lines`, and it was right — one function per frame type reads far
better and matches "one file, one responsibility" at the function level. Not
silenced.

**The gate.** Green: fmt, clippy zero and zero, 1055 tests. Nothing here
positions, sizes or paints.

**What the next iteration should know.** Item 160, HPACK, and one thing about it
is already written into the queue because it is the mistake to avoid: **a
decoding failure is fatal to the connection, never to one stream.** The dynamic
table carries state from one block to the next, so a block nobody could decode
leaves the table in a condition nobody can reason about — resetting just that
stream and continuing would mean decoding every later block against a table that
is quietly wrong.

---

## Iteration 49 — queue item 160: HPACK

**The decision that made this safe to write: derive the Huffman codes, do not
copy them.** The specification prints 257 rows of symbol, code and length.
Transcribing them is 257 chances at a bug that appears on the one byte nobody
tested — and a wrong code is not a crash, it is a header that silently decodes
to something else. But the code is **canonical**: sorted by length, the codes
run consecutively, each new length starting where the last stopped shifted along
by one. So the only thing written down is which symbols have which length, in
order, and the codes follow.

That turned a transcription problem into a structural one, and structure can be
checked. Two tests do it: **Kraft's equality**, that the code space is filled
exactly (a single wrong length anywhere breaks it), and a round trip of all 256
bytes. Both passed first time, and so did the specification's four printed
encodings, byte for byte.

Kraft in integers rather than floating point, incidentally — clippy objected to
the `usize as f64` and it was right for a better reason than it knew: a test
that sums 257 fractions can fail for a reason that is not the table. Counting in
units of `2^-30` makes it exact.

**Validation that means something.** A codec that agrees with itself proves
nothing here — HPACK's job is to agree with *somebody else's* encoder, one block
at a time, carrying state between them. So the tests assert the exact bytes the
specification prints **and the exact table sizes it says should exist after each
block**: 57, then 110, then 164; and 222 in the response example, where the
table is small enough that entries are evicted. Those numbers only come out
right if the whole thing is right, eviction and the 32-byte per-entry overhead
included.

**The rule the queue told the last iteration to remember, now enforced:** every
decoding failure is **fatal to the connection**, never to one stream. The table
carries state from block to block, so a block nobody could decode leaves it in a
condition nobody can reason about, and every later block would be decoded
against something quietly wrong. There is a test that walks five different
failures and asserts each is fatal — because "reset the stream and carry on" is
the tempting answer and it is how a connection starts silently mis-decoding.

**Refusals worth naming.** Index zero is not an index — it is the value meaning
"a name follows", and reading it as one is off-by-one into the static table. An
integer with enough continuation bytes is an overflow, refused at five groups of
seven bits rather than allowed to wrap. A size update larger than was agreed is
a peer choosing how much memory this end spends. A size update after a header in
the same block is a sender doing something it must not.

**One thing kept for a reader that does not exist yet.** `never-indexed`
survives decoding. It is how a sender says a value is a secret, and a relay that
forgot it would compress somebody's authorization token into a shared table.
Nothing relays today; the flag is carried so that when something does, the
information is there rather than remembered.

**The gate.** Green: fmt, clippy zero and zero, 1075 tests. Nothing here
positions, sizes or paints.

**What the next iteration should know.** Item 161, streams and flow control —
the connection state machine. This is the one where the bounds go in **before**
the happy path, because it is where a misbehaving peer allocates memory on our
side: streams opened and never used, a window that never opens, `CONTINUATION`
frames that never end. The last of those has a name — CONTINUATION flood — and
it should be refused by a bound on the total header block across frames, not by
a bound on each frame.

---

## Iteration 50 — queue item 161: streams and flow control

The queue said the bounds go in before the happy path. They did, and the file
reads that way: three files, and most of what is in them is refusals.

**Every way a peer spends our memory over HTTP/2 is a count, not one oversized
thing.** That is the shape of the whole item. A single frame is bounded by the
frame reader; what is not bounded there is *how many*. Three counts, three
bounds: streams open at once, closed streams remembered, and the total size of a
header block across `CONTINUATION` frames.

**The CONTINUATION flood is the one worth naming.** Each frame is individually
legal and inside the frame-size limit, and nothing in the protocol limits how
many there are. A bound per frame does nothing; the bound has to be on the
**total across frames**, which is why it is counted by the session rather than
by the frame reader. And a `CONTINUATION` sequence is uninterruptible — a peer
that could interleave a frame for another stream could make two header blocks
into one.

**A stream is not open or closed.** It is open in each direction separately, and
the two stop at different times. A request fully sent while its response is
still arriving is *half-closed locally*, and that is the normal state of every
request a browser makes. Collapsing it into a boolean is how a `DATA` frame
after a finished response becomes a body silently appended to a page instead of
a `STREAM_CLOSED` — there is a test named after exactly that.

**A test I wrote was wrong about the protocol, and finding out was the useful
part.** I asserted that lowering `SETTINGS_INITIAL_WINDOW_SIZE` puts an existing
stream's window at `new - old`, i.e. negative. It does not: the change applies
as a **difference**, so a stream that has spent none of its window simply lands
on the new size. A window goes below zero only when data was already in flight
against the old size — 535 left, minus 65,435, is −64,900. The code was right
and the test was wrong, and the corrected test now demonstrates the case that
actually matters rather than one that cannot happen. Refusing a negative window
would break a peer that did nothing wrong; the protocol allows it and this
engine has to as well.

**What is refused rather than saturated:** a window widened past the ceiling.
Saturating would leave the two ends disagreeing about how much may be sent,
which is worse than stopping.

**What is ignored rather than refused:** a `WINDOW_UPDATE` for a stream that is
already gone. The peer sent it before it knew, and the two crossed. Ending a
connection over a race nobody lost would be worse than the race.

**Two numbers that must not be confused:** what the peer allows us, and what we
allow the peer. Mixing them up means either refusing our own requests or
accepting an unbounded number of theirs. They are separate fields and there is a
test for each direction.

**Push is refused, not handled.** This engine sends `ENABLE_PUSH: 0`; a server
that pushes has ignored what it was told, and honouring it would be accepting a
response to a request nobody made.

**The gate.** Green: fmt, clippy zero and zero, 1100 tests. Nothing here
positions, sizes or paints.

**What the next iteration should know.** Item 162, negotiating HTTP/2 at all —
ALPN in the TLS handshake, and choosing 1.1 when the server does not offer h2.
`tls.rs` is the boundary file for `rustls` and ALPN is set on its client config,
so the change is there rather than in the h2 module. The closing condition names
the thing to get right: **no request sent twice while finding out** which
protocol is in use. Negotiation happens during the handshake, so the answer is
known before the first byte of a request — a client that discovered it later and
retried would be a client that sent a `POST` twice.

---

## Iteration 51 — queue item 162: negotiating HTTP/2, and speaking it

**The closing condition was the design.** *"No request sent twice while finding
out."* Nothing in this change can send one twice, and not because of care —
because the answer comes out of the **TLS handshake**, before a byte of any
request exists. That is what ALPN is *for*, and why the protocol version is not
a header: a client that discovered it afterwards would have to send the request
again, and a `POST` sent twice is a payment made twice.

So the change starts in `tls.rs`, which is the `rustls` boundary, and the ALPN
tests live there too — the same rule that moved item 52's tests, for the same
reason.

**A plain connection is always HTTP/1.1, and that is a decision rather than a
gap.** Reaching HTTP/2 without TLS needs prior knowledge — which is guessing —
or an `Upgrade`, which means sending a request that may have to be sent again.
This engine does neither, and the reason is written where somebody would
otherwise add it.

**There is no request line in HTTP/2.** The method, scheme, path and authority
are headers whose names begin with a colon, and a colon is a character no
ordinary header name may contain — which is exactly what makes them impossible
to forge from an ordinary one. They go first, and the rules are enforced in both
directions: a *response* carrying a request's pseudo-header is refused, and so is
an ordinary header arriving before `:status`. That second one matters more than
it looks: a pseudo-header after an ordinary one is how a message gets smuggled
past something that only reads the first few headers.

**The hop-by-hop headers are dropped, and that is not tidiness.** A server
receiving `Connection` or `Transfer-Encoding` over HTTP/2 must treat the message
as malformed. Sending one is not a compatibility gesture; it is a broken
request. `Host` becomes `:authority`, and a caller who set `Host` does not get to
choose the authority — the same rule `http.rs` already applies for the same
reason.

**Credentials are marked never-indexed on the way out.** `Authorization`,
`Cookie` and `Proxy-Authorization` are never put in a compression table, ours or
any relay's. It is ADR 0007's rule about cookies, pointed at compression.

**Where the state lives was the one structural decision.** The two HPACK tables
and the stream bookkeeping belong to the **connection**, so `Idle` in the pool
now carries them. Losing them between requests would mean the second request on
a connection could not be decoded at all — the same class of mistake as throwing
away a read-ahead buffer in item 54, and with the same symptom: everything works
exactly once.

**One deadlock avoided by ordering.** `SETTINGS` and `PING` are answered as they
arrive, not after the response is assembled. A peer waiting for an
acknowledgement stops sending, so a client that replied at the end would be
waiting for a response the server was waiting to be allowed to send.

**Cut to the queue:** item 163, a request with a body. Every request today goes
out with `END_STREAM` on its `HEADERS`, which is truthful and means no `POST` —
a body needs `DATA` frames sized to the window, and a window that closes
mid-body has to be waited on rather than overrun.

**The gate.** Green: fmt, clippy zero and zero, 1114 tests. Nothing here
positions, sizes or paints.

**What the next iteration should know.** Section A of the queue is finished
except items 60 (HTTP/3) and 163. The next unblocked item by dependency is 61,
the same-origin policy and CORS — and its closing condition is worth reading
before starting: *"a cross-origin read that should fail does, in a test that
names the attack rather than the header."* That is asking for tests written from
the attacker's side, not the specification's.

---

## Iteration 52 — queue item 61: the same-origin policy and CORS

**The queue's closing condition was a instruction about how to write the tests,
and it was worth following.** *"A cross-origin read that should fail does, in a
test that names the attack rather than the header."* So every test is named for
what somebody is trying to do —
`a_wildcard_does_not_hand_over_a_page_that_was_fetched_with_cookies`,
`a_page_cannot_read_a_set_cookie_it_was_never_given` — and the header is a detail
inside it.

**That naming rule found a real bug**, which is the argument for it. A file of
`allow_origin_header_is_checked` tests would have passed against what I first
wrote. `the_question_carries_no_credentials_of_its_own` did not: it showed
`Access-Control-Request-Headers: authorization, cookie`. `Cookie` is set by the
**browser**, never by the page, so counting it as an author header got two
things wrong at once — every credentialled request would have been preflighted,
and the preflight would have told the server the page asked for something it
cannot ask for, inviting it to allow something it cannot grant. There is a list
of browser-set headers now, and a test named after that too.

**The thing most explanations get backwards, written into the module doc:** a
page may **send** a request almost anywhere. What it may not do is **read the
answer**. An image from another site draws, a form posts, a script runs — none
of them hand the page anything readable. Refusing to send breaks the web;
allowing a read without agreement is how one site reads your bank statement.

**Why preflight exists, in one rule:** the question is not "is this dangerous",
it is **could a plain HTML form have done this already**. If it could, there is
nothing to protect and asking first would only make the web slower. If it could
not, ask — because a `DELETE` that arrived and was then refused is a `DELETE`
that happened. That is why the safelist is a list of what a form can do rather
than of what seems harmless, and the module says so where somebody would
otherwise add to it.

**Three refusals that are the whole point:**

- `*` does not cover a request that carried credentials. `*` means "anyone may
  read this, and it contains nothing personal", and cookies contradict that by
  existing. Without the rule, every server that ever wrote `*` for a public file
  would be giving away its logged-in pages.
- A wildcard in `Access-Control-Allow-Headers` never covers `Authorization`.
  `*` is written by people who mean "my public API", and a credential is never
  that.
- Two **opaque** origins are not the same origin as each other. A comparison on
  the serialised string — both are `null` — would make every `file:` page and
  every sandboxed frame one origin, all reading each other.

**One case handled literally and deliberately, with a test to say so.** A server
that writes `Access-Control-Allow-Origin: null` is not opening a door to one
page; it is opening one to every sandboxed frame and local file on earth. The
specification says to match it literally and this engine does — and the test
exists so that is a decision rather than an accident. It is still never enough
for credentials.

**Cut to the queue:** item 164, the preflight cache. Without it a cross-origin
request is two round trips every time.

**The gate.** Green: fmt, clippy zero and zero, 1134 tests. Nothing here
positions, sizes or paints.

**What the next iteration should know.** Item 62 — CSP, referrer policy, HSTS
and mixed-content blocking. Four things that share a shape: each is a *policy a
site states about itself*, which is the opposite direction from CORS, where a
site states what others may do. The one to be careful with is CSP, because its
grammar is large and getting a directive wrong quietly weakens a page's own
protection — so the rule should be that a directive we cannot parse makes the
policy **more** restrictive, never less.

---

## Iteration 53 — queue item 62: HSTS, mixed content and referrer policy

**Scope cut on starting.** The item named four things; CSP is a whole item on
its own and went in as 165 before a line was written. The other three share a
shape — each is a policy a site states **about itself**, which is the opposite
direction from CORS, where a site states what *others* may do.

**HSTS: two rules make it a defence rather than a weapon**, and both have tests
named after them.

A `Strict-Transport-Security` header arriving over **plain HTTP is ignored**.
Honouring it would let the attacker who is already rewriting your traffic pin
any domain for two years — turning the defence into a denial of service. And it
**never applies to an IP address**: an address belongs to whoever holds it
today, so a rule keyed on one would follow the address rather than the site.

The attack itself is worth restating because it is the reason the whole
mechanism exists: somebody types `example.com`, the browser tries `http://`, and
a network in between answers before the real server is ever asked. The redirect
the real server would have sent never happens. No amount of correct TLS touches
this, because the whole attack is over before any TLS begins.

**The subdomain walk is label by label, not by suffix.** A suffix comparison
says `evil-example.com` is under `example.com`. Here that would mean a lookalike
inheriting somebody else's pin. Same bug as the cookie domain check in item 57,
and it got the same test.

**Mixed content is not one rule**, and that is the thing to understand about it.
A script or stylesheet replaced in transit does not *look at* the page — it **is**
the page, and nothing recovers, so it is refused with nothing offered. An image
replaced in transit is a wrong picture: bad, and not the same thing. Those are
retried over TLS first, because a great many sites have an `http://` URL in
their markup and a perfectly good `https://` server, and blocking them would
break pages for nothing.

**`http://localhost` is secure**, and not as a convenience: there is no network
between the two ends, so there is nothing in between to attack. Refusing it
would break every developer on earth while protecting nobody.

**The referrer default is the modern one**, `strict-origin-when-cross-origin`,
and the reason is in the module doc: a full URL carries the path and the query,
and a great many paths and queries **are the message** —
`/reset-password?token=…`, `/results/hiv-test?patient=…`. The test that says so
is named `another_site_is_not_told_which_page_you_were_reading` and uses exactly
that kind of URL, because a test using `/foo?bar=baz` does not make anybody
think about what is being protected.

**One rule holds under every policy but the one named for breaking it:** a
referrer never survives a downgrade to `http`. What we would be sending is
precisely what an attacker on that connection is there to read. `unsafe-url` is
the exception and it is named for what it is.

**A policy nobody can read leaves the default alone rather than weakening it**,
and the last *known* value in a list wins rather than the last value. That is
how a site offers a strict policy to browsers that have it without an unknown
value at the end quietly discarding it — and it is the same principle item 165
will need for CSP, written down here first.

**The gate.** Green: fmt, clippy zero and zero, 1154 tests. Nothing here
positions, sizes or paints.

**What the next iteration should know.** Item 63, the process split and the
sandbox — ADR 0005's central claim made real, and the largest structural item in
stage 2. It cannot be retrofitted, which is why the roadmap put it first and why
it should not be put off further just because the network items were easier to
take. Read ADR 0005 before starting: the four reasons a memory-safe engine still
needs a sandbox are the design, not the justification.

---

## Iteration 54 — queue item 63: the boundary's wire format

**Scope cut on starting, and the ADR asked for it.** The item named three
things — a process per site, a sandbox, and the encoding that makes either
possible. ADR 0005's own consequences say the sandbox *"needs its own ADR at
that time, naming the boundary and the reason. This ADR does not pre-authorise
any of it."* So: encoding here, spawning as item 166, sandbox as item 167 and
marked **needs ADR**. Taking the sandbox inside this iteration would have been
exactly the thing LOOP.md forbids — a decision made inside a commit that was
mostly code.

**Which direction is untrusted, and the answer people get wrong.** Both, but the
one that matters is the message coming **back**. The browser process holds the
network, the disk and the profile; a renderer is the process that parsed a
hostile page. If that page found a way to steer it, everything the renderer says
afterwards is the page talking. So every length in a message from a renderer is
a number a stranger chose, and it is checked against what is actually left
before anything is reserved.

**The refusal I am most glad is there:** a tree deeper than 512 is refused
rather than recursed into. A snapshot arrives as a recursive structure, and a
decoder that recursed as far as it was told would run out of stack — a crash in
the **browser** process, caused by the renderer, which is the single thing ADR
0005 exists to prevent. There is a test that builds a tree 562 deep and asserts
the refusal.

**Three smaller ones, each a real class of bug.** A frame whose size and pixels
disagree is a frame something above would read past the end of. A `NaN` is
refused because every comparison against one answers false, which turns a bounds
check into a thing that passes. And anything left over after a message is
refused, because trailing bytes mean the two ends disagree and ignoring them
lets a sender append something a later version would read.

**`BoxId::from_wire`, and why it needed a paragraph.** The only other
constructor is `from_index_for_tests`, "named so that using it anywhere else
looks wrong" — deliberately, because an id from a number could name a box in a
different document. But a snapshot that crosses a boundary must arrive with its
ids intact or an agent cannot act on what it just read. So there is a second
constructor, and its documentation says the thing that makes it safe: **an id in
a message is a claim, not a fact.** ADR 0003's "allocated once, never reused" is
a promise the *allocating* process makes, and a process on the other side of a
pipe is not obliged to keep it.

**Roles cross by name rather than by number**, so adding one never renumbers the
others and the wire stays readable. That needed the reverse of `KnownRole::as_str`,
which did not exist — so `KnownRole::named` and `KnownRole::ALL` went in
together, with a round-trip test over all fifty. The two are written in separate
places and this is what stops them drifting; a role that went out as one thing
and came back as another would be a box an agent could no longer find.

**Why the encoding is written out rather than derived.** Deriving would mean a
serialisation crate reaching into `alo-box`, `alo-agent`, `alo-layout` and
`alo-paint` — four crates gaining a dependency and a set of derives for one
boundary. ADR 0005 says the protocol has to be **coarse**, and a coarse protocol
is small enough to write down. Writing it down also makes the wire format
something a person can read, which matters for a boundary that is a security
boundary.

**The gate.** Green: fmt, clippy zero and zero, 1168 tests. Nothing here
positions, sizes or paints.

**What the next iteration should know.** Item 166, the spawn. The shape is a
re-exec of this binary with a flag, a pipe each way, and a registry keyed by
*site* — scheme plus registrable domain, which is the thing item 156's public
suffix list is for and which today would have to be the host. Say that out loud
in the code rather than quietly using the host: two sites sharing a process
because we could not tell them apart is exactly the failure this whole structure
exists to prevent.

---

## Iteration 55 — queue item 166: one process per site

ADR 0005's central claim, as processes that exist. There is a binary now —
`alo-render` — and the tests spawn it.

**The test that is the whole point** kills a renderer process while another is
serving, and asserts the other keeps working. Everything else in the design is
in service of that sentence, and until this iteration it was a sentence in a
document. It is `killing_one_renderer_leaves_the_other_running`, it uses `kill
-9`, and it passes.

**A dead renderer is not quietly restarted**, and that has a test of its own.
ADR 0005: *"a browser that silently restarts a renderer hides a bug that
somebody needs to see."* So the failing request fails, the entry is dropped, and
the next **deliberate** load gets a fresh process. The distinction matters
because a silent restart turns a page that crashes its renderer every time into
an invisible loop — the test asserts the started-count does not go up on the
failing request and does on the next real one.

**Two endings told apart.** A stream that stops *between* messages is
`Arrived::Ended`; one that stops *inside* a message is an error. A renderer that
finished and exited is not a renderer that crashed, and a browser process that
could not tell them apart would report a bug every time a tab closed.

**What a site is, said out loud rather than assumed.** ADR 0005 says scheme plus
registrable domain. The registrable domain needs the public suffix list, which
is queue item 156 and does not exist — so today a site is scheme plus **host**.
That is *stricter*: `a.example.com` and `b.example.com` get separate processes
where they should share one. It costs memory and it never puts two sites
together, which is the direction to be wrong in. `site.rs` says all of that in
its module doc, because somebody adding the suffix list needs to **find** the
assumption rather than discover it.

**The renderer binary is deliberately tiny.** A loop, a decode, a call, an
encode. It opens no file it was not given, makes no connection, and knows
nothing about the profile — and it says in its own header that this is not the
sandbox but the shape the sandbox will be applied to. The point of keeping it
small is that when item 167 arrives there is very little here for a policy to
have to permit.

**One thing I decided rather than defaulted:** the child's standard error is
inherited rather than piped. A pipe nobody reads fills and blocks the process
that writes to it, and a renderer blocked on a diagnostic would look exactly
like a renderer that hung on a page. So diagnostics go to the terminal where a
person can see them.

**The gate.** Green: fmt, clippy zero and zero, 1175 tests. Nothing here
positions or sizes anything new — the frame that crosses is checked for size
against its pixels, which the wire format already asserts.

**What the next iteration should know.** Item 167, the sandbox, and it is marked
**needs ADR** — so the next iteration is the *decision*, not the code, the way
ADR 0007 preceded cookies and ADR 0008 preceded DNS. ADR 0005's consequences say
it explicitly: *"If the platform crate we use does not cover something and we
must write `unsafe` ourselves, that needs its own ADR at that time, naming the
boundary and the reason. This ADR does not pre-authorise any of it."* The
decision to make is which of Seatbelt, seccomp-bpf and user namespaces we rent
versus write, and what a renderer is *given* once it can ask for nothing — the
fonts question `alo-render` currently answers by embedding one.

---

## Iteration 56 — ADR 0010: the sandbox

Item 167 is marked **needs ADR**, and ADR 0005 asks for this one by name in its
own consequences. So this iteration is the decision and nothing else, the way
0007 preceded cookies and 0008 preceded DNS.

**Rent it — and the usual reason is not the reason.** The usual argument for
renting is effort. Here it is that **a sandbox we wrote would be a sandbox only
we had tested.** Seatbelt profiles and seccomp filters are load-bearing in
browsers that are attacked continuously, and the bugs in them were found by
people attacking them rather than by people reading them. Law 3 says correct
before fast; there is no equivalent law for *correct before adversarially
exercised*, and you cannot test your way there alone.

**The `unsafe` question, answered narrowly on purpose.** A rented crate's
`unsafe` is the crate's, which is exactly where ADR 0005 already puts TLS and
codecs. So this decision **authorises no `unsafe` in this repository** — and the
ADR says that in a sentence of its own, so that nobody later reads "the sandbox
ADR" as having pre-authorised FFI we write ourselves. If a platform turns out to
need that, it comes back for another decision naming the boundary and the
reason. That is the same shape ADR 0005 used to defer *this* decision to *me*,
and it seemed right to pass it on the same way rather than to quietly take a
wider authorisation than I was given.

**The hard part is failing closed.** A renderer that cannot apply its sandbox
exits. Somebody will want to reverse that on a bad afternoon, so the reasoning
is written out rather than assumed: rendering without a sandbox is not a
degraded browser, it is a browser that has removed a protection the person
believes it has, at the exact moment it discovered it could not provide it — and
the failure is silent by nature, because nothing about the page looks different.

The counter-argument is real and I wrote it down rather than around: a platform
quirk, an unusual kernel, a container without the right permissions, and the
browser will not open a page. That is somebody's Tuesday.

**So failing closed comes with a promise that makes it rare:** the browser does
not claim a platform it cannot sandbox. Windows is not on the list and the ADR
says so as a consequence rather than an omission. A platform with no sandbox is
one we do not ship — not one we ship with the protection quietly off, which is
the same decision as failing open made once instead of per launch, and worse for
being invisible.

**The consequence people underestimate:** a confined renderer cannot open a font
file. The rule is **the browser process passes bytes; the renderer opens
nothing** — not "permit the font directory read-only", which is the tempting
answer and which puts a filesystem path into the policy for every resource type
that follows. One rule that holds for fonts, images and whatever comes next
beats a policy that grows a hole per kind of thing. That is queue item 168, and
`alo-render` embedding one font today is what the design forces rather than a
gap in it.

**Two things the sandbox does not do**, written down because assuming otherwise
is how a half-measure gets mistaken for a measure. It does nothing about what a
compromised renderer *says* on its pipe — which is why item 63's decoder treats
every message as bytes a stranger chose, and the two decisions only work
together. And it does not protect a renderer from the page: it confines the
damage rather than preventing the compromise, which is why ADR 0005's four
reasons survive a memory-safe engine instead of being replaced by one.

**How we will know it works:** a test that watches a renderer fail to open a
file, never a flag saying a sandbox was applied. A policy that was installed and
permits everything reports success exactly like one that works.

**The gate.** Green. Documentation only.

**What the next iteration should know.** Item 167's code. The macOS half is the
one to start with, since that is the machine this loop runs on and ADR 0010 says
the test has to watch a real refusal rather than trust a flag — so the test is
`a_renderer_cannot_open_a_file`, and it should fail before the sandbox exists,
which is worth checking deliberately: a test that passes both before and after
is testing nothing.

---

## Iteration 57 — queue item 167: the sandbox, on macOS

ADR 0010's code. Renderers are confined now, and the confinement is watched
rather than trusted.

**The route, decided by law 4 rather than by preference.** macOS has two ways
in. `sandbox_init` is a C function, so FFI, so `unsafe` — and ADR 0010 says in a
sentence of its own that it authorises none in this repository. `sandbox-exec`
is a program that applies a profile and then execs, needing no FFI at all. So
that is the route, and its deprecation is written into the module as a real cost
rather than left to be discovered.

It turned out to have an advantage that is not a consolation prize: applying the
profile **by `exec`** means the process is never unconfined, not even for the
instant between starting and sealing itself. ADR 0010 rejected "apply it after
start-up" for exactly that reason and this route gets it for free.

**The profile was found by removing things until it stopped working.** Several
attempts failed with a bare `SIGABRT` and no diagnostic, which is what a
sandbox violation looks like from outside. The missing permission in the end was
a read of `/` itself — the root directory — which nothing about the failure
pointed at. That is worth remembering for item 169: the feedback loop here is
almost nonexistent, so the way through is bisection rather than reasoning.

**The check is of the renderer, in the state it actually runs in.** ADR 0010
asked for that specifically, so `alo-render --check-confinement` tries four
forbidden things and prints what happened, rather than a stand-in binary sharing
only a profile.

**Two probes lied on the first attempt, and fixing them is the substance of this
iteration.** Reading `/tmp` "failed" because it is a directory. Connecting to a
dead port "failed" with *connection refused* — which means the socket **was**
created and the sandbox did nothing. Both would have reported a working sandbox
on a machine with none. So a probe now counts only `PermissionDenied`: a file
not found means the open was allowed, and a connection refused means the socket
was allowed, and neither is confinement. The type says so — `Attempt::Refused`
against `Attempt::Allowed { what }`, where the second carries *why* it was not a
refusal.

**The test that makes the other test mean something.** The same binary is run
twice, confined and not, and the unconfined run must be **allowed all four**. A
test that passed both before and after would be testing nothing, and now there
is a test asserting it does not. That was the thing the last journal entry told
this iteration to check deliberately, and it was right to.

**One small hardening worth naming.** The executable's path goes into the
profile as a `-D` **parameter**, not pasted into the text. A checkout under a
directory with a quote or a bracket in its name would otherwise change the
meaning of the policy rather than filling in a blank — the same class of bug as
an injected quote anywhere else, and worse here because the thing being injected
into is a security policy.

**A platform with no sandbox gets no renderer.** `sandbox::confined` returns an
error on anything but macOS and `Renderers` turns that into a `Gone` rather than
falling back to a plain command. There is a test for that branch, so the
promise ADR 0010 made — *the browser does not claim a platform it cannot
sandbox* — is enforced rather than stated.

**The gate.** Green: fmt, clippy zero and zero, 1179 tests — and the process
tests from item 166 now spawn *through* the sandbox, so the whole boundary is
exercised confined.

**What the next iteration should know.** Item 168, fonts across the boundary,
and it is now a real problem rather than a tidy one: a confined renderer cannot
open a font file, and `alo-render` embeds one because the design forces it. The
rule ADR 0010 set is that the browser process passes **bytes** rather than the
policy permitting a directory — so the work is a message carrying a font, and
the temptation to resist is adding `(subpath "/System/Library/Fonts")` to the
profile, which would be one hole per resource type from then on.

---

## Iteration 58 — queue item 168: fonts across the boundary

The consequence of ADR 0010 that nobody sees until it bites: a confined renderer
cannot open a font file, so **somebody still has to**, and it has to be the
process that is allowed to.

**The temptation was named in advance and it was right to name it.** The easy
way out is `(subpath "/System/Library/Fonts")` in the sandbox profile. That puts
a filesystem path into a security policy for one kind of resource — and the next
kind arrives with the same argument and no way to refuse it, and the profile
becomes one hole per resource type. The harder way is the browser process
reading the files and passing bytes, and it is what ADR 0010 chose. The last
journal entry told this iteration to resist it, and having that written down
before starting is what made it a decision rather than a shortcut not taken.

**`alo-render` embeds nothing now.** It starts with an empty database, which is
the design rather than a gap, and the test that proves the design is real is
`a_renderer_given_no_fonts_has_none_and_cannot_fetch_any` — it checks both that
the renderer has none *and* that it is genuinely confined, because a renderer
with no fonts that could still open a file would just be one that had not tried
yet.

**Fonts go over once per renderer, not per page.** ADR 0005 asks for a coarse
protocol, and a font resent with every load would be megabytes a page. They are
sent immediately after spawn and before anything else — a renderer handed a page
first would lay it out with nothing to draw text in, and the result is a
rendering difference nobody could explain from outside.

**Two small refusals that are the same idea twice.** Bytes that are not a font
are refused *when they arrive*, not when text is shaped — a font that fails at
shaping fails a long way from the moment somebody could have been told. And the
renderer answers with the family it **actually found**, rather than echoing back
the name the browser process guessed from a filename, because a renderer drawing
with something other than what was asked for is exactly the kind of difference
nobody can explain from the outside.

**`.ttc` collections are skipped deliberately**, and the reason is in the code:
a collection holds several fonts, `alo-text` cannot pick one out of it yet, and
taking the first face and calling it the family would be a font that renders and
is not the one anybody asked for. Skipping is honest; guessing is not.

**The search is sorted and bounded**, so two runs on the same machine hand a
renderer the same fonts in the same order. That is what makes a rendering
difference *between runs* mean something, which is the only reason to care about
the order at all.

**The gate.** Green: fmt, clippy zero and zero, 1185 tests. Clippy caught the
panic-in-a-helper rule again — a `fn` outside `#[test]` may not panic — and the
fix was building the value directly rather than unwrapping, which reads better
anyway.

**What the next iteration should know.** Item 170, fonts a page asks for by
name, was cut out of this one and is the honest gap: every renderer gets the
same short list at startup, so a page asking for a family nobody sent gets a
fallback **silently**. The closing condition asks for the substitution to be
named rather than silent, which is the same principle as `FromRenderer::UsingFont`
reporting the family it found — a difference the person can see beats one they
cannot. Item 169, the Linux sandbox, is the other open one and it needs a Linux
machine to be checked on, which this loop does not have; that is a real
constraint rather than an excuse, and the queue says so.

---

## Iteration 59 — queue item 68: the first page we did not write

Taken out of order, deliberately, and the reason is worth recording because it
is a criticism of the fifteen iterations before it.

**LOOP.md's stage-2 rule says the trigger for most items is a real page that
fails, never a specification listing a method.** Iterations 44 to 58 were
HPACK, CORS, HSTS, frame padding, the sandbox — all correct, none of it
scheduled by anything failing. Meanwhile section C had exactly one item, and it
was the one that would say whether any of this renders a page. Sixteen corpus
cases, every one of them markup we wrote to exercise something we had just
built. That is a good way to check a thing works and a bad way to find out what
is missing.

**`example.com`, 559 bytes, frozen.** IANA publishes it for exactly this
purpose, so freezing it needs nobody's leave, and it is small enough that the
whole input to a failure can be read at once. It is nonetheless a real page: it
carries its style inside itself, sizes itself in viewport units, asks for
`system-ui`, sets an opacity, uses `:link`. Not one of those appears anywhere in
the alo cases.

**Three findings on the first run.**

**One had to be fixed before the page would render at all.** A page's style
sheet is *inside the page*, and nothing collected `<style>` elements — because
every case until now kept its CSS in a file beside the markup, which is how
alo's screens are built. This is the shape of the finding I want to remember:
the gap was not in anything anybody had considered and refused. It was in the
shape of the corpus, and it was invisible for as long as the corpus was ours.

**One is a silent substitution.** The page asks for `system-ui, sans-serif`; the
corpus has DejaVu Sans; the text was measured and drawn in a family the page did
not ask for, and `issues.txt` is **empty**. The render is stable and diffable and
it is not what the page looks like anywhere else. That is item 170, which was
cut a few iterations ago on a hunch and now has evidence.

**One is visible in the picture.** The user-agent sheet gives headings
`display: block; font-weight: bold` and no margin, and paragraphs none either,
so the heading and the paragraph butt together. Every real page will render
tighter than it should. Item 171, and its closing condition says the review is
that **every other case's render moves in the same commit** — a UA change that
moved none of them would mean they were all setting their own margins anyway.

**And what it did not find, which is as much the point.** `width:60vw` and
`margin:15vh auto` resolve exactly — 480 and 90 at 800×600. `opacity:0.8` groups
and ungroups. `a:link` gives `rgb(51 68 136)`. `font-size:1.5em` is 24px. Text
wraps where it should. None of that had ever been asked of a page we did not
write, and it all worked. A case that only reported failures would have made the
engine look worse than it is.

**The gate.** Green: fmt, clippy zero and zero, 1185 tests, and the suite still
passes with nothing plugged in — the case is bytes on disk and the test reads
files.

**What the next iteration should know.** Item 171, the block margins, and it is
the right next one: it is small, it is visible, and its diff touches every
committed render, which makes it the best possible check that the reference
machinery does what it claims. After that, **more pages** rather than more
specification. The queue now has three items that came from one page; a second
page will produce more, and that is the loop LOOP.md described and which had
stopped happening.

---

## Iteration 60 — queue item 171: what a page looks like before anybody styles it

The first item scheduled by a page rather than by a specification, and it went
somewhere I did not expect twice.

**The item as written was "block margins".** It became the whole of the
specification's typographic defaults, because a heading at 16px is the same
defect as a heading with no margin: the sheet said what elements *are* and
nothing about what they look like. Splitting those would have meant two
iterations each leaving the sheet half-right.

**It could not ship without fixing a cascade bug it exposed**, and that is the
finding. The cascade competed declarations **by property name**, so a
`padding-left` from one sheet and a `padding` from another never met — different
keys, and whichever the reader consulted first won, regardless of origin or
specificity. That was invisible for as long as the user-agent sheet set no box
longhands. The moment it did, an author writing `ul { padding: 0 }` was silently
overridden **by the user agent**, which is the cascade upside down.

The fix is to expand `margin` and `padding` into longhands where they are
written, so the two compete as the same property. Inserted at the shorthand's
position, so `padding: 1em; padding-left: 0` still ends with a left of zero.

**Then I made it worse, and a picture caught it.** My first expansion refused
values containing `var()`, on the reasoning that a custom property may hold
several values so the sides are not knowable until substitution. True, and
exactly wrong: it left an author's `padding: var(--a) var(--b)` as the *only*
unexpanded shorthand, so it lost to the user agent's longhand — and every
control on every alo screen lost its padding. The layout numbers had moved by
plausible amounts and I nearly accepted them. Rendering the settings screen and
looking at it beside the old one is what said no: the nav items were cramped and
the Save button had become a small pill.

That is the reference-render half of the gate doing precisely the job it is
there for, and it is worth writing down that **I had already read the numeric
diff and not seen it.** A twenty-eight pixel change in a dialog's height reads as
a margin arriving. It read as one to me.

The corrected split treats `var()` as one part like any other function and
respects parentheses, so `1px calc(2px + 3px)` is two values rather than four.

**The review the item asked for, answered the other way.** Its closing condition
said every other case's render should move, and that a change moving none of
them would mean they were all setting their own margins. **None moved** — every
existing case is byte-identical. That is the true branch, and it is confirmed by
reading the sheets: `body { margin: 0 }`, `h2, h3, p { margin: 0 }`,
`ul { padding: 0 }`. The one case that moved is the one that did not ask.

**Three other expectations moved, each for a reason worth keeping.** An
`aria-hidden` paragraph in an agent test now takes a paragraph's room — hidden
from the tree and still on the page, which is a useful distinction to have a
test for. And the layout-number tests now start from `body { margin: 0 }`,
stated once in their helper: every one of them is about where flex, grid and
`calc` put a box, and eight pixels of body margin would move all of them equally
while saying nothing. The margin is asserted in the corpus, against a page that
did not ask for it, which is where it belongs.

**The gate.** Green: fmt, clippy zero and zero, 1191 tests.

**What the next iteration should know.** Another page. The queue now has items
170 (fonts by name) and 156 (the public suffix list) waiting, but the thing this
iteration proved is that a page we did not write finds defects nothing else
does — two of them, one of which was in the cascade and had been wrong since the
cascade existed. A second page will find more, and it should be a harder one:
something with a linked stylesheet, an image, and a form.

---

## Iteration 61 — queue item 172: the first web page

The second page we did not write, chosen for a specific reason: **it has no
style sheet at all.** Every pixel comes from the user-agent sheet, which item
171 had just rewritten and which had no real coverage whatsoever. A page that
brings its own CSS would have hidden most of that work behind its own opinions.

**It is also 1991 markup**, which is a second thing to test entirely: uppercase
tags, an unclosed `<P>`, `<DT>` and `<DD>` closed by the next one starting, and
a `<HEADER>` element that meant `<HEAD>` and no longer does.

**What it found: links had no colour.** The sheet said
`text-decoration: underline` and nothing else. On a page that is almost entirely
links — this one — that renders as an undifferentiated wall of black text with
no way to see what can be followed. It is the same class of defect item 171
fixed: the sheet said what elements *are* and nothing about what they look like,
and no case noticed because every case set its own.

`a:any-link` rather than `a`, because an `<a>` without an `href` is an anchor
rather than a link, and this page is full of them.

**And a decision that was already made, now visible.** There are no purple
visited links, because `:visited` never matches in this engine — a privacy
decision taken when the selector list was written, since whether a link has been
visited is history and a style that depends on it is readable from the page. The
consequence is a page that looks slightly wrong to anybody expecting purple. I
wrote that into the sheet where somebody would otherwise add the rule, rather
than leaving it to look like an omission.

**Two findings filed rather than fixed**, because each is its own item and
neither is small. `text-decoration: underline` has been in the sheet all along
and **paints nothing** — it is parsed, it is inherited, and no paint operation
comes out. Nothing in the alo cases underlines anything, so nothing noticed
(item 173). And a wrapped inline reports **one** rectangle covering all of its
lines, so `link "Frequently Asked Questions"` comes back as 778×37 from the left
margin (item 174). Nothing acts on that — no verb takes a coordinate — but it
decides whether a node is offscreen and it is what a person reading the tree
sees.

**What it did not find is the point of running it.** The `<DL>` indents by forty
pixels, the `<H1>` is 2em with its margins, the paragraph has its own, and all
four things the parser could not make sense of are in `issues.txt` rather than
silently dropped. Item 171's work is now checked against a page that asked for
none of it.

**And one thing that is just pleasing.** `<HEADER>`, written in 1991 to mean
`<HEAD>`, is parsed as the HTML5 `<header>` element and comes back as a `banner`
landmark of zero height. That is the correct modern reading of markup written
before either existed, and it is the sort of thing you can only see once the
agent tree is a thing you can read.

**The gate.** Green: fmt, clippy zero and zero, 1191 tests. Two pages in the
corpus that nobody here wrote, and five queue items that came from them.

**What the next iteration should know.** Item 173, painting `text-decoration`,
and this page is its test — it is the only case in the corpus with an underline
in it. Its closing condition asks for `line-through` and `overline` in the same
change, because they are the same machinery and splitting them means building it
twice. The harder half is that a decoration has to stop at the end of an inline
rather than running to the edge of the line it is on, which is what
`alo_box`'s split-inline note in `tree.rs` is already about.

---

## Iteration 62 — queue item 173: painting `text-decoration`

Scheduled by a page, which is the third iteration running that has been true.

**The hard rule fell out of the shape rather than needing a special case.** The
item's closing condition asked that a decoration stop at the end of an inline
rather than running to the edge of the line it sits on — the thing that is
awkward in a renderer that thinks in lines. It is not awkward here, because
paint already walks **fragments**, and a fragment is one piece of one inline on
one line. Drawing one rectangle per fragment *is* the rule. The `broken-link`
case, which exists for a link split around a block, came out right without a
line of code aimed at it.

**Propagation is not inheritance, and the difference is why this walks
ancestors.** `text-decoration` does not inherit — it propagates, and the
consequence is that a descendant **cannot turn it off**: `text-decoration: none`
on a child of an underlined element removes nothing, in every browser, and that
is specified rather than a quirk. Adding the property to the inherited list
would have been one line and would have been close and wrong in a way somebody
would eventually hit. Walking up from the text box is what the propagation
actually is.

**The colour comes from the element that declared the decoration**, which is a
rule that is invisible until it is wrong. My first version of the test case
could not have told: the outer span and the inner child were both black. Rewrote
it so the declaring span is red and the child is black — the child now paints as
black text with a red line under it, and a wrong implementation would produce a
black line and pass the old case.

**The face decides where the line goes.** `FaceMetrics` gained the underline
offset and thickness, taken from the font rather than guessed, because how far a
face's letters descend is what decides where a line can go without cutting
through them. A face that reports nothing gets a fallback that is visible at
every size rather than a line of zero height.

**Four cases moved and one is new.** `broken-link` and the two web pages gained
their underlines; `alo-sign-in` did too, because it has a link in it. The new
`text-decorations` case is the only one that exercises `overline`, `line-through`
and two lines at once — which the item asked for in the same change, because
they are the same machinery and splitting them means building it twice.

**The gate.** Green: fmt, clippy zero and zero, 1191 tests.

**What the next iteration should know.** Item 174 — a wrapped inline reports one
rectangle covering all its lines — is the remaining finding from the first web
page, and it is now *more* visible rather than less: the decoration code proves
the fragments are there and correct, so the agent tree reporting their union is
a choice rather than a limitation. That makes it a smaller job than it looked.

---

## Iteration 63 — queue item 174: where a wrapped thing actually is

The last of the three findings from the first web page, and the previous
iteration's journal was right that item 173 had made it smaller: painting
decorations per fragment proved the fragments were there and correct, so
reporting their union in the agent tree was a **choice** rather than a
limitation.

**What was wrong.** A link crossing two lines is in two places — the end of one
line and the start of the next — and their union covers the text between them,
which belongs to somebody else. `link "Frequently Asked Questions"` came back
778 pixels wide starting at the left margin.

**Nothing acts on a rectangle**, because ADR 0002 means no verb takes a
coordinate, so the cost was never a misclick. It was two other things: "is this
on screen" answered from a box the thing does not occupy, and a person reading
the tree told something untrue about where a link is.

**The offscreen rule was wrong in both directions**, which is worth stating
because only one of them is obvious. A link whose first line has scrolled away
is still visible if its second has not — that one is easy to see. The other is
that a **union straddling the viewport edge looks visible when neither piece is
inside it**, which is the case a naive fix would leave in place. Both have
tests, and writing the second one is what caught my first attempt.

**And my first attempt at that test was wrong in a way worth recording.** I used
a one-pixel viewport, reasoning that nothing could be inside it. But with
`body { margin: 0 }` the link's first line starts at y=0, so it *was* inside —
the test failed and the code was right. The page needed a spacer so the link
begins below any window short enough to exclude it. A test that cannot produce
the situation it is named after is worse than no test, because it looks like
coverage.

**The union stays.** It is still the answer to *roughly where is this*, and the
outline still prints it — with `in 2 pieces` appended where a node is more than
one rectangle. Listing every rectangle would have been more honest and much
less readable; saying "this box is a union of two" is honest and costs four
words.

**They cross the boundary.** The browser process is where "what is visible" and
"what to draw a highlight around" both happen, and a union is not something it
could take apart again — so `SnapshotNode` carries the rectangles and the wire
format encodes them.

**The gate.** Green: fmt, clippy zero and zero, 1196 tests.

**What the next iteration should know.** All three findings from the first web
page are closed, and the corpus has two pages nobody here wrote. The pattern
that has held for five iterations is: take a page, let it find things, fix them,
take another. The next page should be harder than either — one with a **linked**
stylesheet, an image and a form — and the first thing it will need is corpus
machinery for a case with more than one file, which does not exist yet. That is
worth doing as its own item rather than smuggling into the page's.

---

## Iteration 64 — queue item 175: a case with more than one file

The machinery the last journal entry said should be its own item rather than
smuggled into a page's. It was right to separate them: this turned out to
contain a decision that would have been invisible inside a page's iteration.

**The decision: `<link>` and `<style>` are one list, in document order.** The
obvious implementation collects the linked sheets and the written ones
separately and concatenates them. That is wrong for every page that links a
sheet and then writes a `<style>` correcting it — which is a common shape, and
which would have come out with the correction losing. Nothing would have
crashed; the page would just have been the wrong colour, for a reason two
crates apart from where it looked.

So `alo_dom::sheets` returns one ordered list of *what the page asked for*,
written or linked, and the pipeline resolves the linked ones as it walks it. The
corpus case has a paragraph whose colour is red in the linked sheet and green in
the inline one after it, and the case's own `style.css` is **deliberately empty
of colour** — because a rule there is appended last and would decide the
question instead of document order. My first version of the case had the green
in `style.css` and would have passed against a completely broken ordering.

**A sheet that did not arrive is a state rather than an error.** The page renders
without it, and the fact goes into `issues.txt`. A page styled by a sheet that
never came looks wrong for a reason nobody can see from the page, so the engine
saying so is the whole difference between a mystery and a fact. The case links
one that is deliberately absent.

**`rel="stylesheet alternate"` is not applied**, and the case covers it. An
alternate sheet is one a person chooses; applying it as well would be applying
two. It is the sort of thing that works by accident until a page ships both.

**Frozen, never fetched.** `linked.txt` maps an `href` as the page wrote it to a
file beside the case, written down rather than inferred from filenames — because
the `href` is what the page said, and a mapping somebody can read is a mapping
somebody can check. Same reason a case carries an `origin.txt`.

**The gate.** Green: fmt, clippy zero and zero, 1196 tests.

**What the next iteration should know.** The corpus can now hold a real page
with a real stylesheet, which is what the last three journal entries have been
building towards. The remaining gap for a *hard* page is images: there is no
image decoding at all — `alo-paint` has nothing for it and section H has seven
open items — so a page with pictures will render their space and not their
content. That is worth knowing **before** freezing such a page, so the case is
taken with the gap understood rather than discovered as a failure.

---

## Iteration 65 — queue item 106: reading a picture a stranger sent

**Scope cut on starting, and it needed cutting twice over.** Item 106 asked for
five codecs *and* `<img>` laying out and drawing. Investigating it first turned
up the reason that is two items and not one: there is **no intrinsic sizing
anywhere** in `alo-box` or `alo-layout`, so "an image draws at its own size" is
a layout feature that happens to involve pictures. That went to item 176; the
other four codecs to 177.

What is left is the half ADR 0005 names by itself, and the half no sandbox
catches: a renderer that allocates seventeen gigabytes because a header said so
is doing nothing a sandbox forbids.

**There were already two jobs in one function.** `from_png` reads a *reference
render* — a file this engine wrote moments earlier, in one format — and being
strict there is a feature, because a reference render that is not eight-bit RGBA
means something is wrong. A page's picture needs the opposite on both counts, so
there are two readers now and the doc comment on each says which job it has.
Tolerant about what a PNG may be; unforgiving about every number that decides an
allocation.

**Two tests taught me something I had assumed, and both are worth keeping.**

I built the bomb as a *header alone*, reasoning that nothing else was needed to
make a decoder reserve memory. It was refused — with "unexpected end of file",
because the decoder never reached the size at all. A header on its own proves
nothing about a size check. So the bomb is now a **valid** picture whose header
has been rewritten to claim sixty-five thousand square, with the chunk's
checksum mended so the decoder is happy right up to the moment the bound stops
it. That is the actual attack: a hundred-odd bytes that parse perfectly.

And I asserted that **every** truncation is refused. It is not, and should not
be: at 87 bytes of 91 the file has lost only its end marker, and every byte of
its image data is there. Refusing that would refuse a picture whose last four
bytes were lost in transit — which browsers show, because showing what arrived
is most of the point of an image on a page. The test now asserts the thing that
would be dangerous: a prefix must never produce a canvas of a **size nobody
declared**, since that is the shape of reading past the end of what arrived.

**And a queue defect I made and fixed in the same iteration.** Cutting item 106
left the original text behind as a "106b" — the same duplicate-number defect as
the two 54s several iterations ago. Removed, with the one sentence worth keeping
(ADR 0005's reason) moved onto the item that survived. A queue with two items
for one piece of work is a queue that will have one of them done twice.

**The gate.** Green: fmt, clippy zero and zero, 1204 tests.

**What the next iteration should know.** Item 176, `<img>` laying out and
drawing, and the work is **intrinsic sizing** rather than pictures. Nothing in
`alo-box` or `alo-layout` has a notion of a box with a size of its own; taffy
supports it through a measure function, which is how `MeasureText` already
works, so the shape is probably a second measurer rather than a new field. Worth
checking that before writing anything, because a field on `BoxNode` is the
obvious answer and might be the wrong one.

---

## Iteration 66 — queue item 176: a picture that actually appears

The last journal entry said to check the measure hook before reaching for a
field on `BoxNode`, and that was the right instinct for the wrong reason: the
answer was neither. `NodeKind` in the layout arena already has a variant per
kind of leaf — text, an inline formatting context, a container, nothing — so a
box sized by its content is **another variant**, which is where the existing
design was already pointing.

The natural size itself does live on the box tree, as a side map filled in after
the tree is built. `alo-box` knows nothing about pictures and should not start,
so it holds a width and a height and no opinion about where they came from.

**Two things the corpus case caught that I had wrong.**

An `<img>` is `inline-block`, so it goes through the **inline** path and not
taffy's leaf layout at all. My first version sized only block-level images; the
case showed 4×3 where it should have shown 80×60 and that is what pointed at it.

And the ratio cannot be computed in the leaf's measure, which was my first
attempt. `width: 80px` came out 80×**3**, because the measure is asked *before*
the style width is applied — taffy asks how big the thing wants to be, then
applies the style, and never asks again with the width known. The right answer
is a `taffy` aspect ratio, which is precisely what resolves one definite
dimension against the other. Fighting the measure protocol was the wrong
instinct and the ratio was sitting there the whole time.

**A picture that did not arrive keeps its box**, and the fact is recorded. An
empty box of the right shape is what a browser shows for a broken image; a
collapsed page is what happens if the box goes away, and it makes every other
thing on the page move for a reason nobody can see.

**Two gaps named in the code rather than left to be found**, both because the
alternative was silence. The picture is drawn **nearest-neighbour** — exact at
one-to-one, which is what an `<img>` with no width does and most of what a page
has, and coarse anywhere else (item 179). And a **rotated** picture draws
upright inside the right area, because only the rectangle's corners are
transformed (item 178). Drawing nothing under a rotation would have been wrong
and *invisible*, which is worse.

**One thing I did and then undid.** I reached for `BoxId::from_index_for_tests`
to walk every box — a constructor whose own documentation says using it
elsewhere should look wrong. It did look wrong. `BoxTree::ids()` exists now, for
asking a question *of a kind of box* rather than following the tree's shape,
which is a different thing and one a walk answers badly.

**The gate.** Green: fmt, clippy zero and zero, 1204 tests. The new case has a
reference render, which is what the gate asks for of anything visual, and its
stripes are three different colours on purpose: a wrong row order or a flipped
picture is obvious rather than plausible.

**What the next iteration should know.** The corpus can now hold a page with a
linked stylesheet *and* pictures, which is what the last four iterations were
building towards — so the next page can be a hard one. The remaining known gaps
for such a page are the other codecs (item 177: JPEG is the one that matters,
since most photographs on the web are one) and forms, which nothing has looked
at. A page with a `<form>` would find things in `alo-box`'s control handling
that only the alo cases have exercised so far.

---

## Iteration 67 — queue item 177: JPEG

Cut on starting: four codecs is not one item, and JPEG is the one that matters
because most photographs on the web are one. GIF, WebP and AVIF are item 180.

**A different rented crate is a different file.** `png` belongs to
`encode.rs`, which rented it for reference renders, so `jpeg_decoder` gets
`picture.rs` — which also owns the thing that had nowhere to live before:
deciding **which** format a run of bytes is.

**The format comes from the bytes, not from the name**, and this is the part
worth having built. A `src` ending in `.png` is a string on a page; the server
that answered may have sent something else, by mistake or on purpose. So the
corpus case serves a JPEG as `/lying-name.png`, and it decodes — because what a
thing *is* cannot be lied about without also being true. A decoder handed the
wrong format either fails confusingly or, worse, finds something in it.

**The same bounds either way, from one list.** That is the reason JPEG and PNG
are one item: a second decoder with its own limits, or none, is a second way in.
Every refusal test walks both formats from a single list, so adding a third
means adding it to the list rather than remembering to. A JPEG's dimensions are
in its frame header, so the size is knowable without decoding — and there is a
test that rewrites that header to claim four billion pixels.

**What I got wrong, and it was about the test rather than the code.** The
picture was four by three with three one-row stripes, which is a lovely test for
PNG and a meaningless one for JPEG: a picture that small is a single DCT block,
and chroma subsampling returns it as mud. The green stripe came back
`(130, 123, 115)`. The picture is twenty-four square with eight-row stripes now,
and the test asks which channel is **largest** rather than for a colour —
because asking a lossy format for exact bytes is a test about the format rather
than about the code.

**A colour space this engine does not convert is refused by name.** Sixteen-bit
greyscale and CMYK exist and are rare on the web; a wrong conversion is a
picture in the wrong colours, and nobody looking at it would know which of the
two had happened.

**The gate.** Green: fmt, clippy zero and zero, 1211 tests, and the new rented
crate is behind its own file.

**What the next iteration should know.** The corpus can hold a page with a
linked stylesheet, PNG and JPEG — everything a real page needs except a form.
Nothing has looked at forms outside the alo cases, and `alo-box`'s control
handling has only ever seen markup we wrote. A page with a `<form>` is the next
one to freeze.

---

## Iteration 68 — the supervisor, made startable

The owner asked to recreate the loop. It was not broken — `--self-test` green,
`--dry-run` reporting a clean tree and 103 open items — and recreating it
byte-for-byte would have produced the same file, which is not an answer to
anything.

**What the check did turn up: it has never been run.** No lock, no log, nothing.
Four requests for a loop against a supervisor nobody has started once. So the
thing missing is not the loop; it is whatever makes somebody willing to start
it, and that is a different problem with three parts.

**`--items 5`.** "Run until the queue is empty" is a large thing to agree to on
faith. It is the same loop either way and only the number differs, so somebody
deciding whether to trust it at all can buy five iterations and read the five
commits. Offering only "all of it" was asking for a leap nobody needs to make.

**A log.** Everything goes to `docs/autonomy/loop.log` as well as the terminal,
because a terminal is the one place a record does not survive closing a window —
and an unattended run is by definition one nobody is watching. Gitignored: it is
a record of runs on one machine rather than source.

**A closing summary that counts the right thing.** It says what **closed** and
what was **committed**, not how many iterations it managed. An iteration that
halts honestly is worth more than one that invented a way past a problem, so
iterations were never the measure. And a run that closed nothing and committed
nothing now says so loudly, which is the outcome somebody most needs told.

**`--self-test` covers the arguments now**, and the gate runs it. A supervisor
that read `--items abc` as five hundred, or a typo as a request to run forever,
is one nobody should trust unattended — so the argument handling is checked the
same way the stop rule is, by asserting the exit code of eight actual
invocations.

**What I did not do, and will not.** Start it. I tried once and the environment
refused, correctly: the supervisor spawns workers with
`--dangerously-skip-permissions`, which is not a thing an agent should be able
to launch on its own behalf. That guard is doing its job and routing around it
would be the wrong kind of helpful.

**The gate.** Green.

**What the next iteration should know.** The queue is unchanged at 103 open, so
nothing here closed an item — this was tooling, and the journal should say so
rather than dress it up. The next *item* is a page with a `<form>`: nothing has
looked at forms outside the alo cases, and `alo-box`'s control handling has only
ever seen markup we wrote.

---

## Iteration 69 — queue item 181: a page with a form

The HTML specification's own example form, served by httpbin as a test
endpoint. 1397 bytes, no style sheet, and almost every part of a form at once:
labels **wrapping** their controls, two fieldsets with legends, radios,
checkboxes, a textarea, and three input types nothing here had seen.

By now this is a pattern worth naming: **the pages that find the most are the
ones with no CSS of their own.** Three of the four cases we did not write have
been like that, and each has found something in the user-agent sheet that no
alo screen could, because every alo screen states its own opinion about
everything.

**The first finding is the one I would not have found by reading.** A
`<fieldset>` was laid out **inline**, because the user-agent sheet declares
`fieldset` and `legend` as `display: block` and then, forty lines later, as
`inline-block`. A duplicate in one sheet is the later rule winning. So a
fieldset was an inline box, its contents came out as seven "pieces" of a broken
inline, and the page was ninety-six pixels too tall. The sheet had been wrong
since it was written and no case could show it: the corpus had every control and
not one *group* of them.

**A fieldset is named by its legend** now — the same shape as a `<label>` naming
the control it wraps, an element named by something it contains. Without it the
tree said `group` with the legend's words beside it as loose text, so an agent
asked to tick "Large" under "Pizza Size" had nothing to tell the two groups
apart by.

**A radio was drawn as a square**, which is the recurring shape of these
findings: the agent tree has told a radio from a checkbox since the first
commit, and a person looking at the page could not. A radio group and a checkbox
group ask different questions — one answer or several — so somebody who cannot
see which they are looking at is being asked a question without being told its
shape.

**And making it round found the next thing.** `border-radius: 50%` did nothing:
percentages resolved against zero, because the code computing the radii was
never given the box. That was written down in `corner.rs` as "a limitation
rather than a decision" and left. It is fixed — a percentage radius is a
percentage of the box, horizontally of its width and vertically of its height —
and the honest lesson is that a limitation written down is still a limitation.
Writing it down made it *legible*, not *acceptable*, and nothing was going to
find it except something that needed it.

**Two findings left open, and one of them is uncomfortable.** A checked checkbox
draws exactly one thing: its border. `[checked=true]` in the tree, nothing in
the picture. That has been true since controls were built, and there is an
example of it **sitting in the alo corpus** — `a-filled-form` has a checked box
and its committed render shows an empty square. Nobody looked until a page put
radios and checkboxes side by side. Item 182, and its closing condition asks for
the indeterminate and disabled cases too, because "you cannot change this" and
"this is off" are different things to be told. A fieldset also has no border,
which is item 183.

**The gate.** Green: fmt, clippy zero and zero, 1211 tests.

**What the next iteration should know.** Item 182 is the one to take, and not
because it is next: it is a state a person cannot see, it has been wrong the
whole time, and the corpus has been quietly committing a picture of it. The
paint work is a tick path and a dot — small — and the reference renders it moves
are the ones that prove it.


## Iteration 70 — queue item 182: a checked control looks checked

The item the last iteration named, and not because it was next: a state a
person could not see, wrong since controls were built, with a picture of it
committed in the corpus the whole time. `a-filled-form` has had a checked
checkbox and a render showing an empty square since iteration 16.

**The interesting part was deciding which half goes where.** A tick is drawn by
the engine and cannot be a style rule: CSS has no way to say "and draw a check
inside it", and the nearest thing — a `::before` with a character in it — puts
the mark at the mercy of whichever font loaded. That is the same argument that
put a control's inner box in `alo_box::Purpose::Control` rather than in the
sheet, and it is why `alo-paint/src/control.rs` exists.

But **whether a control is live is ordinary colour**, so that half *is* in the
user-agent sheet, where a page can override it. Splitting it that way is what
made the last clause of the closing condition reachable: "you cannot change
this" and "this is off" are different things to be told, and the mark cannot
tell you the first, because an unchecked control has no mark. The border does.

So the case has four pictures where a naive reading of the item would have had
two — on, off, on-and-locked, off-and-locked — and no two of the nine controls
in `control-states` look alike.

**`accent-color` came with it rather than a constant.** A hardcoded blue would
have been a colour no page could change, in the one place CSS has a property
for exactly this question. Reading it made a second rule necessary and worth
having: the mark is black or white by whichever shows up against the accent,
because a fixed white tick vanishes into `accent-color: yellow` and the page
that set it would have no way to see why. The corpus case has one such row.

**The item that the code had already scheduled.** Setting `border-color` on a
disabled control did nothing, because `border-color` was never expanded into
its four sides — and `alo_css::declaration`'s own comment said why: *"the engine
does not yet set any of them in the user-agent sheet, so nothing collides"*.
Writing that rule made it collide. `border-width`, `border-style` and
`border-color` split by exactly the rule `margin` and `padding` already use, so
this was three entries in a table; `border` itself still does not, because `red
solid 1px` and `1px solid red` are the same border and splitting that means
parsing rather than counting. Queue item 184, written down as taken.

That is the second time in three iterations that a limitation *written down*
turned out to be a limitation *scheduled*: iteration 69 found `border-radius`
in per cent refused with a note beside it. A comment naming the day something
becomes wrong is worth more than one saying it is wrong.

**What I got wrong, and it was in the test rather than the code.** The first
assertions counted pixels of exactly the mark's colour. A tick at the size a
page asks for is two and a half pixels across, so nearly all of it is
anti-aliased and almost none of it is exactly white — twenty pixels for the
whole tick, one for the part that reaches the top of the box. Counting which of
two colours a pixel is *closer to* is what measures a shape; counting exact
matches measures a flat fill. The corpus render was right the whole time and the
test was asking it the wrong question.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero and zero, 1234 tests. A
new reference render (`control-states`) and nine assertions in
`alo-paint/tests/control_states.rs` for the half a picture cannot say — that the
ink inside a checked box is the accent, that a radio's mark is round and a
checkbox's is not, that a disabled one is grey and still ticked.

**One case moved and only one**, twice over: `a-filled-form` when the mark
landed, `control-states` when the disabled border did. Nothing else in
twenty-three cases changed, which is the review — a user-agent change that moved
an unrelated screen would have been the thing to look at.

**`ROADMAP.md`.** The line moved is **Forms** in the DOM section, to a Built /
Owed clause: a control draws its own state; what a control *does* still needs
events (item 81), and the focus ring still needs something to have focus. Item
184 moved no line and that is the honest answer — the border shorthands are a
cascade fix under stage 1's ticked "Computed style", not a line of its own.

**What the next iteration should know.** Item 183 is the other half of what the
form page found: a fieldset draws no border, so the thing that makes a fieldset
worth using is invisible. The interesting part of it is that the legend sits
*in* the top border rather than above it, which is a hole in a shape — and
`corner.rs`'s `between` already draws one shape with another cut out of it,
which is what an inset shadow uses. That is the machinery to reach for.

Item 43 is now explicitly blocked on item 81 rather than open: the tick and the
dot are done, and the focus ring cannot be drawn while nothing in this engine
has focus. That is recorded in the queue rather than left as a half-open item.

---

## Iteration 71 — the log records runs rather than tests

Found by reading `loop.log` after the first real run, which is the first time
anybody had. `--self-test` starts the script eight times to check what the
arguments mean, and each child appended its startup lines and its deliberate
`FAILED:` messages to the same log. Twelve lines of noise per self-test, all
looking exactly like a run that had failed.

That is worse than untidy. The log exists so that a run nobody watched can still
be read, and its whole value is that a `FAILED:` line means something. A log
somebody has to filter before reading is a log they stop reading — and they stop
on the day it finally matters.

**The fix had to be an environment variable rather than a flag**, and finding
that out took one wrong attempt. Setting the log to nowhere once `--self-test`
had been parsed left four lines still there: the children that fail *during
argument parsing* fail before any flag is known. Something read at the top of
the file is the only thing that arrives early enough.

A dry run and a self-test write nowhere now. A real invocation that fails still
writes, because that is a run — and it is one line rather than twelve.

**The regression has a test of its own**, which is the point: a child told to
log nowhere must leave the real log exactly the length it was. Without it the
next person to touch the self-test puts the noise straight back.

**The gate.** Green.

**About the run before this.** The loop closed queue item 182 on its own — a
checked control that looks checked — and I verified rather than trusted it:
gate green, 1234 tests, and the committed render of `a-filled-form` now shows a
ticked box where it had shown an empty square since iteration 16. Its own commit
message reasons about why the mark is the engine's rather than a style rule,
which is the kind of thing this journal exists to keep.

