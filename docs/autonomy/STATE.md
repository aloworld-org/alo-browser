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


---

## Iteration 72 — queue item 153: `Transfer-Encoding` that is not `chunked`

**Taken because it was first.** `LOOP.md` says take the first item that is not
done and not blocked, and in stage 2 that means the first whose dependencies are
all done. Item 153's dependency is 152, which is done. The last iteration named
item 183 as the interesting next thing and it is still there; this one was ahead
of it in the file and ready.

**The item's stated symptom was wrong, and that is the first thing to record.**
The queue says *"today the chunks come off and the gzip does not, which yields
compressed bytes labelled as a page."* Nothing did that. Item 53 compared the
**whole header value** against `chunked`, so `gzip, chunked` never reached the
de-chunker at all — it was refused, along with a test asserting the refusal by
that exact example. So the defect was the other way round: a legal response
refused, rather than a wrong one accepted. The queue text is left as written
with the correction beneath it, because an item's symptom being wrong is worth
more as a record than as a tidy edit.

That does not make the item smaller. `Transfer-Encoding` is a list, this engine
read it as one word, and reading it properly is what the closing condition asked
for: *"it decodes, or is refused by name."* It decodes.

**What was built.** `crates/alo-net/src/transfer.rs`, and the three places that
now go through it.

- The list, parsed, with `chunked` recognised only where it can legally be.
- `http::check_framing_is_unambiguous` calls it instead of comparing strings.
  The `Content-Length`-beside-`Transfer-Encoding` refusal now turns on the
  header being **present** rather than on what it parses to — a
  `Transfer-Encoding: identity` applies no coding here and is still refused
  beside a length, because a recipient that treated `identity` as a coding
  would frame the message by the connection closing while we framed it by the
  length. That *is* the disagreement.
- `body::Framing::of` asks it whether the body is chunked.
- `connection::exchange` undoes the transfer codings after de-chunking and
  before `Content-Encoding`.

**Why it is a file rather than a few lines.** The two headers name the same
algorithms and mean different things: `Content-Encoding` is a property of the
resource and survives the hop, the cache and a saved file; `Transfer-Encoding`
is a property of this connection and does not survive it. The undoing is still
`decompress.rs`'s, because that is the boundary for the three rented crates and
there is no second one. What this file owns is **which codings and in what
order they come off** — and getting that backwards means looking for chunk
headers inside compressed bytes.

**Every refusal is a reading two parsers could differ on**, which is the file's
organising idea and the same one `http.rs` was written around:

- `chunked` anywhere but last. This is also what refuses `chunked, chunked`,
  and that is the better rule to have written: two `chunked`s is the shape a
  smuggling attempt takes when it is aimed at a recipient that de-chunks once
  and one that de-chunks twice, and a separate "not more than once" check would
  have been a second rule saying the same thing.
- A coding we cannot undo, named. `compress` is LZW and is not rented.
- An empty element. `chunked,` is one coding to some parsers and one-and-a-blank
  to others.
- **A transfer-coded body that is not ended by `chunked`.** This one is legal:
  the standard says the body then ends when the connection does. It is refused
  anyway, and the reason is in `decompress.rs`'s own note — gzip and zstd carry
  a checksum that would catch a body cut short, and **brotli and raw deflate
  carry nothing at all**. A close-delimited brotli body truncated by an attacker
  is a shorter page that nothing could tell from a whole one. Refusing by name
  is what the closing condition allows; the alternative was a page that is
  quietly the first half of a page.

**One test I nearly left saying the wrong thing.** `chunked,` was refused with
*"chunked is not the last transfer coding"* — true, and about the wrong thing:
the trailing comma is the defect and `chunked` stopping being last is a
consequence. The empty-element check runs over the whole list first now. A
refusal is only as useful as the reason in it, which is the argument for having
written the reasons out as separate refusals rather than one `is_err()`.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, 1260 tests, no stubs, boundaries held — `transfer.rs` names no rented
crate, which is the point of it delegating to `decompress.rs`. Eighteen new
integration tests in `a_body_encoded_for_one_hop.rs`, and the hostile-input
clause `LOOP.md` asks of anything reading outside bytes is met three ways: a
malformed list table, corrupt and truncated gzip inside the chunks, and a
sweep of ten header values against four bodies asserting only that nothing
panics. No layout assertion and no reference render: this reads bytes and
positions nothing.

**The evidence it decodes rather than merely parses** is that the frozen
`page.html.gz` — made by the `gzip` tool, not by the crate that reads it —
arrives as `page.html` after being cut into sixteen-byte chunks and put back
together. Small chunks on purpose: one chunk would not have noticed a reader
that de-chunked and decompressed in the wrong order.

**`ROADMAP.md`.** The line moved is *"HTTP/1.1, then HTTP/2"*, whose Built
clause gains the header. It stays an empty box: item 163, a request with a body
over HTTP/2, is still owed and named there.

**What the next iteration should know.** Item 154 (byte ranges and downloads
that resume) is next in the file and its dependencies are done. It has a real
interaction with what was built here and with item 152: a range request must ask
for `identity`, and `write_request` already leaves a caller's `Accept-Encoding`
alone for exactly that reason — but nothing yet stops a server answering a range
request with a `Transfer-Encoding` anyway, and a range of a transfer-coded
stream is a range of bytes nobody can reassemble.

Item 183, the fieldset border, is still the one iteration 70 named and is still
worth taking: `corner.rs`'s `between` draws one shape with another cut out of
it, which is what a legend breaking a border is.

---

## Iteration 73 — queue item 154: byte ranges, and downloads that resume

**The tree was not clean when this started, and that is the first thing to
record.** `crates/alo-net/src/range.rs` was sitting untracked: 353 lines, not in
`lib.rs`, with a doc comment referring to a `crate::download` that did not
exist. Nothing in the journal mentions it. It is what `LOOP.md` describes when
it says a worker gone silent is killed and *"the item it was building is redone
next time"* — an earlier attempt at this same item, stopped part way. So the
gate was failing on entry, with exactly one `FAIL`: **crates changed and
CHANGELOG.md did not**, which is that file and nothing else. Not a halt: it is
this item's own unfinished half, and finishing it is what clears it.

I read it rather than trusting it. It is good work and it is kept — the grammar,
the refusals and their reasons. What it did not have is everything the item is
actually about, which is the conversation.

**What was built.**

- `range.rs`, from the abandoned draft: `Content-Range` read as three numbers
  and `Accept-Ranges` read as a refusal only when it says `none`. Strict for a
  reason no other header has: those three numbers decide **where in a file the
  bytes that follow are written**, so a generous parser here does not render
  something wrong, it splices the middle of a download into the wrong offset and
  hands up a file of the right length that is not the thing.
- `download.rs`, and it is a **pure function** — the shape item 55 used, chosen
  again for the same reason. Every rule here is a rule about placing bytes at an
  offset, and a rule like that is asserted honestly only when nothing else is
  moving. `Download::asking` says what to ask for next, `Download::take` says
  what an answer means, and both are driven from a table in the crate's own
  tests with no socket anywhere.
- `Pool::download`, which is the loop, and is short because none of the deciding
  is in it.

**The four rules, and what each is protecting.**

- **A `206` must begin exactly where the download stopped.** `Content-Range` is
  checked against the length held rather than trusted to be the answer to what
  was asked. One byte off is one byte missing from the middle of a file.
- **A `200` answering a range request is never appended.** It is byte zero
  onwards whatever was asked for, and a server ignoring `Range` is common rather
  than misbehaviour. So the bytes are dropped and it starts again — and
  `Download::restarts` counts it, because *noticed* has to mean more than *not
  believed*: somebody has to be able to see that it happened, and a server that
  does it every time has to run out of attempts rather than loop.
- **Nothing coded is spliced.** A download asks `identity` from its **first**
  request, not from the resumed one — `write_request` has left a caller's
  `Accept-Encoding` alone since item 152, with a comment naming this day. A
  `206` carrying a `Content-Encoding` is refused outright: its offsets are into
  the *coded* representation and the bytes already held are not.
- **A resume needs a validator.** This is the one worth reading twice. Without
  an `ETag` or a `Last-Modified` to put in `If-Range` there is nothing that
  could tell us the file changed between the two asks, so such a download starts
  again rather than resuming. Slower, and the only reading that cannot be
  silently wrong. A **weak** `ETag` is not taken either: it says two
  representations are good enough to swap for one another, which is a different
  claim from "these are the same bytes" and is exactly the wrong claim to splice
  on.

**What had to change underneath, and it is the interesting half.** A body that
stopped early was an error and its bytes were thrown away with it — which is
right for a page and is the whole point of item 53. So `body::read_what_arrived`
now hands back both, `read` is that with the short answer turned into an error,
and `connection::exchange_however_it_ends` is the door a download comes in by
while `exchange` keeps item 53's promise unchanged.

Two things fell out of that and both are corrections rather than features. A
short body **keeps its codings on**: `crate::connection` undoes them only for a
body that arrived whole, because half a gzip decompressed is a prefix nothing
could tell from a whole page. And the connection it arrived on is **not kept**,
because there is nothing left on it that anybody can find the start of — that
was previously true by accident, since a truncated body errored before anything
could keep it.

**Two things the item did not say and the code found.**

- `Framing::UntilClose` cannot tell a finished body from a truncated one, so it
  never reports one short. A download can do better, because a `Content-Length`
  or a `Content-Range` is a length somebody stated: an answer whose framing was
  satisfied is still `Step::More` when it is shorter than the length it claims.
- A `Content-Length` on a response that **was** compressed counts the coded
  bytes, and the body has since been undone. A download that believed it would
  ask for a range past the end of something it already has all of. So a coded
  answer contributes no length at all.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, 1301 tests, no stubs, boundaries held — neither new file names a rented
crate. `LOOP.md`'s hostile-input clause is met twice: a table of sixteen
`Content-Range` values that could be placed wrongly, each refused by name, and a
sweep over nine answers a server can give a range request asserting that
whatever comes back is a **prefix of the real file** — which a spliced body
could not be, and which is a stronger thing to assert than "it did not panic".
No layout assertion and no reference render: this reads bytes and positions
nothing.

**The evidence, and it is the closing condition run rather than reasoned about.**
`a_download_that_stops_half_way.rs` puts a server on loopback that promises the
whole file and hangs up in the middle of it, and asserts the resumed bytes equal
those from a second server that never stopped — two sockets, one for each half.
A second server ignores `Range` entirely and the download comes back as the file
rather than as its first twenty-five bytes twice over.

**One thing that is deliberately absent.** A download does not go through the
cache. Half a response must never be stored, and a cache that holds whole
responses in memory is the wrong place for a file large enough to be worth
resuming. Item 155 is where a cache gets a disk and where that becomes worth
asking again; the reason is written where `Pool::download` is.

**`ROADMAP.md`.** The line moved is *"Redirects, byte ranges, and downloads that
resume"*, whose Built clause gains this half. It stays an empty box, and the
Owed clause names why: item 185.

**The cut, written into the queue as item 185.** A download over HTTP/2 starts
again where one over HTTP/1.1 resumes, because the HTTP/2 client turns a stream
that ends early into an error rather than a body with a reason beside it. That
is correct and slower than it needs to be, it is named in `pool.rs` where the
`short: None` is written, and item 163's `DATA` handling is the code that has to
learn the same distinction.

**What the next iteration should know.** Item 155 is next in the file and is
marked *needs ADR* — what may be written to a disk other programs can read is a
different question from what may be reused, and it has a different answer for a
page behind a password. `LOOP.md` is explicit that such an item gets the ADR as
its **own iteration**, before any code depends on it.

If a chore is wanted instead, item 156 (the public suffix list, rented) is ready
and its dependency is done: the site boundary is the host today, which is
stricter than the registrable domain and is wrong — `a.example.com` and
`b.example.com` should be one site.

And item 183, the fieldset border, is still the one iterations 70 and 72 both
named and still unclaimed. `corner.rs`'s `between` draws one shape with another
cut out of it, which is what a legend breaking a border is.

---

## Iteration 74 — queue item 185: a download that stops over HTTP/2 resumes

**The tree was clean on entry and `scripts/gate.sh` was green**, unlike last
time. Item 185 was the first unticked item in the file and both its
dependencies (161, 154) were done, so it was taken in file order.

**What the item said, and the one thing it did not.** The HTTP/2 client turned
a stream that ends early into an error, so a download over it began again at
zero where one over HTTP/1.1 resumed. True, and the fix needed a distinction
one layer further down that the item did not name: **a connection that ends is
not a peer that misbehaved**, and `frame::read` had exactly one way of saying
both. Everything else follows from having that.

**What was built.**

- `frame::read_however_it_ends`, returning `Arrived::Frame` or `Arrived::Ended`.
  `read` is now that with an ending turned back into an error, which is the same
  pair `body::read`/`read_what_arrived` and `connection::exchange`/
  `exchange_however_it_ends` already are. The bytes of a frame that arrived
  whole were framed and checked; the bytes of a peer breaking the protocol were
  not, and only the first are worth keeping.
- **A reset read counts as an ending**, and that is the line worth reading
  twice. A server hanging up part way through a body sends a reset rather than
  closing tidily, *because we are still writing it window updates for what it
  just sent us* — so a check that only looked for `Ok(0)` would have found the
  tidy case in a test and the wrong one in the world. A timeout is deliberately
  not on the list: a peer that has gone quiet may still be there, and reading a
  stall as an ending would turn every slow server into a half-finished
  download.
- `client::exchange_however_it_ends`, handing up the response with whatever
  body arrived and the reason beside it. **Two ways a stream ends early and only
  two**: the connection ends, and the server gives up on the stream with a
  `RST_STREAM`. A header block that will not decode, a window overrun, a frame
  where none may be — each still an error, taking the bytes with it, because
  bytes from a peer breaking the protocol are not bytes to build a file out of.
- A stream that stops **before its headers** is an error rather than a short
  response: there is no response to hand up and no byte to resume from, and it
  is what the pool's retry is for.
- Every write in the read loop is now an answer to something already read, so a
  connection that will not take one ends the response rather than failing it —
  except on the last frame, where a window that could not be widened cannot
  make a finished response unfinished.

**The refactor, and why it is not scope creep.** `Pool::download`'s loop moved
to `download::whole_of`, which takes the exchange as an argument. It is the
same loop; what changed is that it is now *visibly* protocol-blind, which is
the design claim item 185 rests on — the client under it changed and the loop
resumed without knowing. It is also what let the closing condition be **run**
rather than reasoned about: this engine speaks HTTP/2 only over TLS (item 162:
no request may be sent twice to find out), starting a TLS server needs
`rustls`, and ADR 0001 allows that name in `alo-net/src/tls.rs` and nowhere
else — a test included. So the test speaks HTTP/2 on a plain socket and drives
the real loop, with the pool's kept connection swapped for a fresh one per
exchange, which is what the pool does anyway after a body that stopped short.
`is_safe_to_repeat` moved with it, to `Request::may_be_repeated`: two callers
now need that list and two spellings of it is one of them being wrong about a
payment.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, 1310 tests (1301 before), no stubs, boundaries held. Nine new tests.
`LOOP.md`'s hostile-input clause is met by a sweep over seven points at which a
server can stop — including in the middle of a frame whose length it has
already declared — asserting that whatever comes back is a **prefix of the real
file**, which a spliced body could not be. No layout assertion and no reference
render: this reads bytes and positions nothing.

**The evidence, and it is the closing condition run.**
`a_download_that_stops_over_http_2.rs` puts an HTTP/2 server on loopback that
promises the whole file and stops sending in the middle of it without ever
setting `END_STREAM`; the download comes back as the file, in **two** exchanges
rather than three, and the second ask carries `range: bytes=25-` and
`if-range: "v1"`. A second server ends the stream with `RST_STREAM` instead and
is resumed from the same way. I checked the tests fail without the change
rather than assuming it: with `client::exchange` put back in the test's
exchange, three of the six fail with *"the connection ended"* and *"the server
gave up on the stream"* — which is the defect, in the words the new code uses
for it.

**`ROADMAP.md`.** The line moved is *"Redirects, byte ranges, and downloads that
resume"*, and it is **ticked**: its Owed clause named item 185 and nothing else,
items 55, 154 and 185 are all done, and no other queue item points at it. The
tick is earned rather than used to discharge the obligation — which `LOOP.md`
warns about, and which is why this paragraph says who checked. The
HTTP/1.1-then-HTTP/2 line gains a clause and stays an empty box: item 163, a
request with a body over HTTP/2, is still owed.

**What the next iteration should know.** Item 163 is the one this touched
without doing: sending a body in `DATA` frames sized to the window. Its reading
half now has the distinction it needs — a stream that ends early is a fact
rather than an error — and the queue entry's note about it learning the same
thing is discharged by `frame::read_however_it_ends` rather than by anything in
`client::exchange`'s writing half.

Item 155 is still next in the file and still marked *needs ADR*: what may be
written to a disk other programs can read is a different question from what may
be reused, and `LOOP.md` is explicit that such an item gets the ADR as its own
iteration. Item 156 (the public suffix list, rented) is a ready chore whose
dependency is done.

And item 183, the fieldset border, is still the one iterations 70, 72 and 73
each named and nobody has taken. `corner.rs`'s `between` draws one shape with
another cut out of it, which is what a legend breaking a border is.

---

## Iteration 75 — queue item 155: the decision about a disk (ADR 0011)

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 155 was
the first unticked item in the file, its one dependency (56) is done, and it is
marked *needs ADR* — so this iteration is the ADR and nothing else. `LOOP.md`:
*"a decision made inside a commit that was mostly code is a decision nobody
reviewed."* No code was written, deliberately.

**What the decision had to answer.** The queue asked it in one sentence: what
may be written to a disk other programs can read is a different question from
what may be reused, and it has a different answer for a page behind a password.
Writing it out found that the disk turns the cache into *two* things it is not
today — a durable record of everywhere somebody has been, and an **input** we
later hand to a page under that page's own origin. The second is the one that
was not in the queue entry and is the reason section 4 exists.

**ADR 0011, in the six clauses the code now has to carry.**

- **Partitioned by top-level site**, on the same `Partition` the cookie jar
  uses — so when item 156 corrects that answer from the host to the registrable
  domain it corrects both, and there is never a version where cookies and the
  cache disagree about where a boundary is. The argument is ADR 0007's: a shared
  cache answers *have you been somewhere that loads this* for any site that
  thinks to time a load, and an entry only one visitor was ever given is an
  identifier that survives clearing cookies.
- **Never written, rather than written and deleted.** A deleted file was still
  on the disk, and the window between the two operations is exactly where a
  power cut lands. The list: `no-store`, `private`, a request carrying
  `Authorization`, a response carrying `Set-Cookie`, anything not
  `http:`/`https:`, a body that did not arrive whole, and any session-scoped
  profile. Every one of them stays cached in **memory**, where being careful
  costs nothing.
- **A cache file is untrusted input** — `LOOP.md`'s stage 2 rule, which applies
  to a filesystem as fully as to a socket. A checksum over metadata and body, a
  format version discarded rather than guessed at, and an unreadable entry that
  is a **miss** rather than an error: a cache that can stop a page opening is a
  defect however correct its reasoning was.
- **The browser process only.** ADR 0010 named the temptation — permit a
  directory in the sandbox profile instead of passing bytes — and item 168
  refused it for fonts. Here the stakes are higher: that grant would hand a
  compromised renderer every page the person has read, across every site.
- **Bounded in bytes as well as entries**, oldest first by the insertion order
  `cache.rs` already keeps. And said explicitly: **this is not the quota
  decision** (item 90), and must not become it by precedent.
- **No encryption of ours.** ADR 0001 rents the physics; a key that has to live
  next to the data is not a key. So the honest boundary is stated instead:
  protected against another user account, not against a program running as the
  person — and neither is anything else they own.

**What it costs, which is in the ADR rather than left out.** Partitioning costs
re-fetches of shared libraries. The never-written list makes the disk cache
weakest **exactly where it would help most** — a site somebody is signed into
and uses daily is the one whose responses carry `Set-Cookie` or `private`. And
a cache that survives a restart is a browsing record that survives a restart. No
speed number is quoted: other browsers reported partitioning as cheap, and
quoting theirs as though it were ours is the thing `CLAUDE.md` forbids. Item 117
measures it or nobody does.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, 1310 tests unchanged, no stubs, no `unsafe`, boundaries held, a
`CHANGELOG.md` line. No layout assertion and no reference render, and no new
test — this iteration adds no behaviour to test, which is what an ADR-only
iteration is. `cache.rs`'s module comment now says which three clauses land in
that file when the disk arrives, so the decision is where the code is rather
than only in `docs/decisions/`.

**`ROADMAP.md`.** The line moved is *"The HTTP cache, with real semantics"*,
and it gains a `Built:` clause naming ADR 0011 and keeps an `Owed:` clause for
the code — item 155 is not done and the box stays as it was. `docs/features.md`
gains the same distinction on its planned line, so the item is promised before
it is built.

**What the next iteration should know.** Item 155's code is now unblocked and is
the natural next take: its rules are written down, `Cache::keep` is where the
second question goes, and the closing condition is unchanged — a cache survives
a restart, and a response that must not outlive the session does not. It will
need a place to put files and a hostile-input test over half-written ones.

Item 156 (the public suffix list, rented) is still a ready chore, and it is now
worth **more** than it was this morning: ADR 0011 puts the cache on the same
`Partition` as cookies, so the host-instead-of-registrable-domain answer is
about to be wrong in two places rather than one.

And a numbering trap, found while choosing 0011: **queue item 69 calls the
JavaScript engine decision "ADR 0006"**, and 0006 is the supervisor. The next
ADR is **0012**, whatever it decides. Fixing the queue's text was not this
item's, but the iteration that takes 69 should not take the number the queue
offers it.

Item 183, the fieldset border, is still the one iterations 70, 72, 73 and 74
each named and nobody has taken.

---

## Iteration 76 — queue item 155: the cache on a disk

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 155 was
the first unticked item, its dependency (56) is done, and iteration 75 had
already written its ADR — so this iteration is the code that ADR 0011 asked
for, and nothing else.

**What was built, in the three files it takes.**

- **`record.rs` — one entry as bytes, and the whole untrusted surface.** A magic
  number, a version, a sequence, a checksum, and then the response. Every length
  is checked against what is actually there before anything is reserved and every
  step that a hostile number could push past the end is `checked_`. The tests
  walk **every** truncation of a real entry and **every** single flipped byte of
  one, and each is refused; a version we do not know is discarded rather than
  interpreted; bytes appended after the end make it a miss, because that is what
  a file half overwritten by somebody else looks like.
- **`disk.rs` — the directory, the bound, and the policy.**
  `why_it_is_never_written` is ADR 0011 section 2 as one function, consulted in
  one place. The directory is created `0700` and every file `0600`, and set
  rather than assumed, because a directory that already existed was made by
  something else. A write goes to a `.writing` file, is `fsync`ed, and is renamed
  over the entry, so a power cut leaves the old one or the new one and never half
  of either. Bounds are **values** rather than constants — which is what made the
  byte bound testable without writing sixty-four megabytes to reach it.
- **`cache.rs` — the key, and the second question.** The key now carries the
  top-level site, on the same `Partition` the cookie jar uses, and there is no
  method that does not take one. `keep` asks `why_it_is_never_written` before it
  writes; when the answer is a reason, it **removes** any entry that key already
  had — a URL that was public yesterday and hands out a session token today must
  not be served from the disk after a restart. `refresh` asks again, because a
  `304` carries headers and one of them can be a `Set-Cookie`.

**What the shape refuses to allow.** `Pool::follow` takes the top-level site
now, and so does every `Cache` method. That is `jar.rs`'s promise repeated: the
alternative was a field on `Request` with a sensible default, and a default is
exactly how a subresource gets keyed under its own host and the cache is shared
across sites again with nobody having decided it. `fetch::fetch` supplies the
request's own URL, and the comment there says why that is right *only* there —
its pool's cache is created and discarded inside the call, so there is no second
site for anything to be joined to.

**One thing the ADR overstates, said in the code rather than left implied.**
Section 4 claims the checksum *"stops it writing a page into somebody's bank
origin"*. An unkeyed checksum does not: anything that can write the file can
compute the number that goes with it. What it does catch is exact and worth
having — a half-written file, a flipped bit, another program's file under our
name. `record.rs`'s module comment says both, and says that section 3's
boundary is unchanged: against another user account the cache is protected,
against a program running as the person it is not, and a key that would have to
live next to the data is not a key. That is a refinement, not a relaxation, and
it is written down where somebody will read it.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1340 tests** (up from 1310), no stubs, no `unsafe`, boundaries held, a
`CHANGELOG.md` line. No layout assertion and no reference render: nothing here
positions, sizes or draws. The stage 2 clause for anything that reads bytes from
outside is met by `record.rs`'s malformed, truncated and adversarial tests and by
`a_directory_full_of_rubbish_is_a_miss_rather_than_a_failure_to_load`, which puts
eight kinds of rubbish in the directory and then puts the real entry back — so
the refusals are the checks working rather than nothing ever hitting.

**`ROADMAP.md`.** The line moved is *"The HTTP cache, with real semantics"*. Its
`Owed:` clause named item 155 and nothing else, so the clause is gone and the
`Built:` clause now names the ADR **and** the code. The box was already ticked
by item 56 and is not touched. `docs/features.md` gains three lines in the built
section rather than one, because the disk, the never-written list and the
untrusted-input rule are three promises and not one.

**What the next iteration should know.** Item 156, the public suffix list, is
now the ready chore worth most: `Partition::of` is the host in **two** places
that must agree, and correcting it corrects both at once — which is the whole
reason ADR 0011 put the cache on the cookie jar's partition rather than on one
of its own.

Two things this deliberately did not do, both named in the ADR. There is no
**quota** policy here and it must not become one by precedent (item 90). And no
speed number is quoted anywhere: partitioning costs re-fetches, the never-written
list makes the disk weakest where it would help most, and how much either costs
is item 117 on hardware or is not said. `Cache::counts` across a restart is the
measurement ADR 0011 asks for; nothing has run it on real use yet.

And item 183, the fieldset border, is still the one iterations 70, 72, 73, 74
and 75 each named and nobody has taken.

---

## Iteration 77 — queue item 156: the public suffix list

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 156 was
the first unticked item in the file, its one dependency (57) is done, and the
two iterations before it each named it as the ready chore worth most — so it was
taken in file order.

**Where it went, which was not where the item implied.** The queue entry is
written about the cookie partition, so `alo-net` is where it reads as belonging.
It is in **`alo-url`**, because a site is a property of a host and *three*
unrelated things were each answering it with the host on their own: the cookie
partition (ADR 0007), the cache key (ADR 0011), and the renderer process
(ADR 0005) — whose own module comment named this item as the thing that would
correct it. Putting the answer in `alo-net` would have left the process model
with a second one, and this item exists precisely because there should be one.

**What was built.** `alo-url/src/site.rs`, the only file that may name `psl`
(the gate's boundary list gained the line), with two functions:

- `of(&Host) -> String` — the registrable domain, or the host itself when there
  is none. It takes a **`Host` and not a string**, and that is the design rather
  than a convenience: `127.0.0.1` read as a name has the registrable domain
  `0.1`, which would put every machine on an address ending that way into one
  site. The type has already decided which it is; a string has not.
- `is_a_public_suffix(&str)` — for a cookie's `Domain` attribute, which is a
  string and cannot be anything else.

Both lowercase what they are given rather than documenting that they need it.
The list is matched byte for byte, so `bbc.CO.UK` matches no rule, falls to the
rule of last resort and comes back with `CO.UK` as its registrable domain —
a wrong answer in the unsafe direction, made unreachable rather than noted.

**The hole it found, which was not in the item.** `Domain=co.uk` was **accepted**:
the only rule was that a domain contain a dot, which is exactly the rule the
comment there said was standing in until this item. That cookie is one for every
school, council and company in the country. Two smaller ones went with it, both
the same mistake about different things — `Domain=0.1` from a page at
`127.0.0.1`, which `covers` allows because it reads a host as a name; and
`Domain=localhost` at `localhost`, refused outright where the specification makes
it host-only, which is a refusal the page could do nothing about. The attribute's
rules are now one function, `domain_asked_for`, which is also what kept
`Cookie::parse` under the line limit clippy enforces.

**The direction this moved, said plainly.** Every previous note about this called
the host *stricter*, and it was. This makes the boundary **looser** — two
subdomains of one organisation are one site now, share a cookie jar, share a
cache entry and share a renderer process. That is ADR 0005's definition and
ADR 0007's, and the thing that makes it safe is the half a host comparison could
never do: `bbc.co.uk` and `gov.co.uk` are two sites, and no rule of syntax says
so. Where the list has nothing to say — an address, a host that is itself a
suffix, a name under a suffix nobody has registered — the answer is the host,
which is the strict direction.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1357 tests** (up from 1340), no stubs, no `unsafe`, boundaries held —
`psl` among them now — and a `CHANGELOG.md` line. No layout assertion and no
reference render: nothing here positions, sizes or draws. `LOOP.md`'s
hostile-input clause is met by feeding a host thirteen shapes of rubbish and a
name of ten thousand labels, each asserted to come back as part of the name it
was given rather than as a panic — a host arrives from a stranger's page, and a
crash in the code that decides where a cookie lives is a denial of service in
the browser process.

**The evidence, and it is the closing condition run rather than reasoned about.**
The item's two halves are one test naming both: `bbc.co.uk` and `gov.co.uk` are
different sites and `www.example.com` and `example.com` are the same one. Then
the consequence in each of the three places, because a unit test on a function
nobody had wired up would prove nothing: cookies
(`two_subdomains_of_one_site_are_one_partition`, and the boundary that must not
be lost with it), the cache
(`the_boundary_is_the_registrable_domain_rather_than_the_host`), and the process
split — where `a_scheme_is_enough_to_make_it_a_different_site` had been asserting
that two subdomains are two sites, which is the stand-in and not the rule, so it
was rewritten rather than deleted. I checked all of them fail without the change
rather than assuming it: with `of` put back to the host, the cache and process
tests fail; with `is_a_public_suffix` put back to "does it contain a dot", the
`Domain=co.uk` test fails.

**`ROADMAP.md`.** Two lines moved. *"Where one site ends and another begins"* had
this item as its whole `Owed:` clause; the clause is discharged and the line now
says what is built (the site, as ADR 0005 defines it) and what is not — **which**
of the origin, the site and the registrable domain a page gets, case by case,
which is item 66. It stays an empty box: item 66 is not done and a tick would be
the exact move `LOOP.md` warns about. The **Cookies** line was already ticked;
its `Owed:` clause loses item 156 and keeps item 157.

**What the next iteration should know.** Item 186 is the cut, and it is small:
the list is a snapshot, a snapshot ages, and a suffix delegated after ours was
taken reads as an ordinary registrable domain — two organisations in one site,
which is the direction that costs. Nothing prompts anybody to bump it.

One thing noticed and deliberately not taken, because it predates this item and
belongs to whoever takes item 66: `Partition::of` answers `"opaque"` for a URL
with **no host**, so every host-less top-level page shares one partition. Nothing
can navigate to one yet, which is why it is a note rather than an item.

And item 183, the fieldset border, is still the one iterations 70 and 72 to 76
each named and nobody has taken.

---

## Iteration 78 — queue item 186: the snapshot has a date, and now something reads it

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 186 was
the first unticked item in the file, its one dependency (156) is done, and the
iteration before it named this as the cut it left behind — so it was taken in
file order.

**What it is.** `crates/alo-url/src/snapshot.rs`: two recorded facts, three
functions and a test that fails. The facts are the `psl` version whose list is
compiled in (`2.1.223`) and the day that snapshot was taken into this repository
(2026-09-03, which is the day commit `a9977c1` added the dependency — `git log`
rather than a number somebody remembered). The test asserts the list is not more
than **183 days** old and, when it is, prints the version, the day, what a stale
list costs and the two commands that discharge it.

**Why the test rather than the gate**, which the item offered as the first
option. The gate would have meant the same arithmetic in shell, in a script
nothing tests, and `date` on macOS and on Linux do not take the same arguments —
so the check would have been least trustworthy on the machine that is not the
one it was written on, which is item 169's lesson arriving early. The gate runs
`cargo test`, so it fails either way; only one of the two has tests of its own.

**Why the age is measured from the day *we* took it, and why that decided the
number.** The list carries no date: `psl`'s `data/rules.txt` is the Mozilla file
with no header, its `src/list.rs` says only that it was generated, and the crate
publishes nothing else. So the honest record is when the snapshot entered this
tree, which is **at least** as old as the list and possibly younger than it — a
crate version may have sat on crates.io for weeks before we took it. The error
runs in the direction of under-reporting age, and that is what set the threshold
at six months rather than the twelve somebody could argue for: the unknown slack
has to fit inside it.

**The record cannot drift from the code**, which is the half that makes the day
worth measuring from. `the_version_recorded_here_is_the_one_actually_compiled_in`
`include_str!`s the workspace `Cargo.lock` and finds `psl`'s resolved version, so
bumping the crate without re-dating the snapshot fails with a message saying to
set both. Without that, the constant would have been a comment: true on the day
it was written and unfalsifiable afterwards.

**Two things done for the iteration that will meet this failure**, because it
will be a loop, six months from now, and `LOOP.md` tells it to halt when the gate
fails for a reason it did not cause. First, the test's own doc comment says in
bold that **its failing is not a fault in the change being tested**. Second, the
message names the work rather than the problem: bump `psl`, `cargo update -p
psl`, set the two constants. That is the difference between a halt somebody can
discharge in five minutes and a halt somebody spends an afternoon diagnosing.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero errors,
**1365 tests** (up from 1357), no stubs, no `unsafe`, boundaries held — `psl` is
still named only in `site.rs`, and this file names it in prose and in a string
rather than in code — and a `CHANGELOG.md` line. No layout assertion and no
reference render: nothing here positions, sizes or draws. `LOOP.md`'s
hostile-input clause has nothing to bite on — no bytes from outside reach this —
but the one input that is not ours is the **clock**, and it is tested from both
sides: a clock set before the snapshot answers an age of zero rather than a
negative number, and a clock before 1970 answers that it cannot say rather than
failing a build over a machine's own confusion.

**The evidence, run rather than reasoned about.** The failing path was checked by
doctoring `TAKEN` back to 2024 and watching the test fail with the real message
before putting it back. And the tests that name a moment are written from `TAKEN`
rather than from dates read off it — a helper counts days by pushing the day of
the month past the end of its month, which is arithmetic rather than a date, and
the property it relies on is asserted rather than claimed
(`a_day_of_the_month_past_the_end_of_it_is_the_day_it_counts_to`, anchored on one
date a calendar agrees with). The point of that is that the only thing a future
bump has to touch is the two constants; a test that needed re-dating with them
would be friction on the exact chore this file exists to ask for.

**One duplication, deliberate and written down.** `Day::in_epoch_days` is the
same eight lines of Howard Hinnant's `days_from_civil` that `alo-net`'s
`httpdate` uses. They are not shared: `alo-net` depends on `alo-url` and not the
other way round, so sharing them means inverting a dependency for some Gregorian
arithmetic, and renting a calendar crate for eight lines is not a boundary worth
ADR 0001's paperwork. The doc comment on the function says so, so the next person
to see both does not think one of them is an oversight.

**`ROADMAP.md`.** The line moved is *"Where one site ends and another begins"* —
the same line item 156 moved, and the same clause: its `Built:` half now says the
list is a snapshot, that it ages, and that the build fails once it is six months
old. It stays an empty box, because item 66 is what ticks it and item 66 is not
done. `docs/features.md` gains one line under the site, which is where a reader
would look for it.

**What the next iteration should know.** The first unticked item is now **157**
(the storage-access grant), which is blocked on an interface to ask in — as is
**158**, the encrypted-DNS setting, for the same reason. So the first *ready*
item in file order is **159**, the MPL Exhibit A headers on every source file:
it depends on nothing, it is owed by ADR 0009, and it is the kind of item that
stays owed for ever unless somebody takes it deliberately. It also asks for a
gate check, which is where the reasoning above about shell would need
revisiting — a header is a `grep`, not a calendar, so shell is right there.

And item 183, the fieldset border, is still the one iterations 70 and 72 to 77
each named and nobody has taken.

---

## Iteration 79 — queue item 159: every file says what licence it is under

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 159 was the
first *ready* item in file order — 157 and 158 are both blocked on an interface
to ask in, which the iteration before this one had already worked out and
written down. It depends on nothing.

**What it is.** Two things: the notice on 198 files, and a step in
`scripts/gate.sh` that fails on a file without it.

The notice is copied from this repository's own `LICENSE`, Exhibit A, **word for
word** — including the `http://mozilla.org/MPL/2.0/` the licence text uses rather
than the `https://` a modern eye reaches for. That is deliberate: the point of
the notice is that a recipient can compare it against the licence distributed
beside it, and a tidied version is one they have to reason about instead. It sits
above the `//!` module documentation, which every file in this repository already
opens with, followed by a blank line.

**Why it is worth doing at all**, since ADR 0009 already said the root `LICENSE`
satisfies the licence and called this tidiness. MPL is copyleft **per file**, so
which licence a file is under is a property of the file rather than of the
repository — and the person who most needs to know is the one holding a single
file out of an archive, a search result or a vendored copy, who has no root to
look in. That is the whole difference between MPL and a repository-level licence,
and until this commit the engine was not answering it.

**The check compares the first three lines against the exact text**, so a
*reworded* header fails as loudly as a missing one. That is not pedantry: a
notice that drifts is a notice a lawyer has to read rather than diff, and the
version that drifts first is the one somebody retyped from memory. Both
directions were **run rather than reasoned about** — the step was extracted and
driven against a tree with one file stripped of its header and one file's URL
changed to `https://`, and it named both files and printed the three lines to
paste. Then the tree was restored and it went green again.

**Shell rather than a Rust test, which is the opposite of what iteration 78
chose**, and the reason is the one that iteration's own journal predicted: *"a
header is a `grep`, not a calendar, so shell is right there."* Item 186 went to
Rust because date arithmetic in shell is not portable between macOS and Linux and
the check would have been least trustworthy on the machine it was not written
on. Nothing here is arithmetic. `head -n 3` and a string comparison behave the
same everywhere, and the check has to run over files that are not part of any
crate's compilation unit, which a Rust test would have to go looking for on the
filesystem anyway.

**The gate.** `scripts/gate.sh` green: fmt (the notice survives `cargo fmt`
untouched, which was checked before the other 197 files were written), clippy
zero warnings and zero errors, **1365 tests** — the same count as the iteration
before, and that is the honest number: this item adds no Rust test because it
adds no Rust behaviour. Its test is the gate step, verified by hand in both
directions as above. No stubs, no `unsafe`, boundaries held, and a
`CHANGELOG.md` line. No layout assertion and no reference render: nothing here
positions, sizes or draws. `LOOP.md`'s hostile-input clause has nothing to bite
on — no bytes from outside reach a comment.

**`ROADMAP.md` was not moved, and this is the answer step 6 asks for rather than
a silence.** There is no line for this item to move. `ROADMAP.md` is a list of
what the browser *does*, in four stages, and a licence notice is not something it
does — it is a property of the repository, decided in ADR 0009 and recorded
there. `docs/features.md` is skipped for the same reason and it is worth being
explicit, because the gate names it: that file's rule is *nothing gets built that
isn't listed here*, and every entry in it carries a tier from [1] to [4] naming
which stage of rendering it belongs to. This has no tier, and inventing one would
start turning a feature inventory into a list of repository chores. **ADR 0009 is
where it is recorded**, and its consequence bullet is updated in this commit from
*"owed"* to attached-and-checked, so the ADR no longer describes a debt that is
paid.

**What the next iteration should know.** The first unticked items in file order
are **157** and **158**, both blocked on an interface to choose in, and neither
is unblocked by this. So the next ready item is **163** — a request with a body
over HTTP/2 — whose dependency (162) is done, and which is the `Owed:` clause on
the roadmap's HTTP line: today every request goes out with `END_STREAM` on its
`HEADERS`, which is truthful and means no `POST` over HTTP/2.

One consequence to expect and not be alarmed by: **every future diff that adds a
file adds three lines nobody wrote**, and the gate will now stop a commit that
forgets them with a message that contains the exact text to paste. That is the
intended cost.

And item 183, the fieldset border, is still the one iterations 70 and 72 to 78
each named and nobody has taken.

---

## Iteration 80 — queue item 163: a request that sends something

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 163 was
what iteration 79's journal named as next: 157 and 158 are blocked on an
interface to choose in, and 163's dependency (162, ALPN) was done.

**The item said HTTP/2 and the work was both protocols**, and that is scope
against depth rather than scope creep. `Request` had **nowhere to put a body at
all** — `http::write_request` wrote a head and a blank line and stopped — so
adding a body to the type and teaching only the HTTP/2 client to send it would
have left every `POST` over HTTP/1.1 silently bodiless, which is a worse defect
than the one being fixed. `LOOP.md` says cut scope, never depth. The scope cut
is item 187; the depth is that both clients send a body and both obey the same
two rules.

**The two rules live on `Request`, not in the clients**, for the reason
`may_be_repeated`'s own doc comment gives about a payment: two spellings of a
framing rule is one of them being wrong.

- `declared_length` — **the length a request states is the length of its
  bytes.** A caller's `Content-Length` is dropped in both protocols. A body and
  a header disagreeing about where a message ends is the request half of request
  smuggling, and item 53 spent a whole iteration refusing the response half of
  exactly that. A method that anticipates content says `0` rather than nothing,
  which is the difference between a `POST` that sends nothing and a `POST` a
  server is still waiting on.
- `unmet_expectation` — an `Expect` is **refused by name**, which is the branch
  the item offered as an alternative to honouring it. The reason is written into
  the doc comment rather than left as a shrug: an expectation is a promise to
  *wait*, the only clock either client can reach is the caller's socket timeout
  at thirty seconds, and sending the header while not waiting is worse than
  either — it asks a server that does honour it to hold a stream open for a
  go-ahead we have stopped listening for. **Nothing on the web can reach it**:
  `Expect` is a forbidden request header in Fetch, so no page and no script may
  set one. That is what makes refusing affordable, and it is why item 187 waits
  for an upload that wants it.

**The window is the half only a large body reaches.** Both windows start at
sixty-four kilobytes, so an ordinary form goes out in one breath and nothing
about flow control is exercised by it. `push_body` sends the smaller of what is
left, what both windows allow, and the peer's `SETTINGS_MAX_FRAME_SIZE`; when
that is zero it returns, and the read loop calls it again after every frame,
because a `WINDOW_UPDATE` is body that may now go.

The test asserts the number rather than a bound. A hundred-kilobyte body, and a
server that sets a read timeout on its own socket and treats a timeout as *the
client has stopped of its own accord*: **exactly 65,535 bytes have arrived at
that moment.** Under it is a client that stalled; over it is one that overran;
only the exact number is the behaviour asked for. Run rather than reasoned
about — `room_to_send` was doctored out and the test failed with `0 bytes had
arrived`, which is what a client that ignores the window looks like from the
outside.

**A server may answer before it has read the request**, and then the rest of the
body is bytes nobody wants. The stream is reset with `CANCEL` rather than simply
abandoned: a stream this engine stopped writing to would stay open until the
connection ended, counting against the peer's concurrency limit for ever. The
write is allowed to fail without spoiling the response, because the response is
whole and a connection that will not take a reset is one the pool finds out
about on its next use.

**`SETTINGS_MAX_FRAME_SIZE` is now read, and refused rather than clamped at both
ends.** Below the floor is a peer asking us to cut a body into frames whose
headers cost more than their payloads; above the ceiling is a number that cannot
be written into a frame header's three bytes, so believing it would mean sending
something unreadable.

**What the item did not ask for and the work found: interim responses.** A `103
Early Hints` is sent unprompted by a great many servers, and **both protocols
were taking the first head they saw for the answer** — a blank page, on a
perfectly ordinary server. HTTP/2 was worse than that: the stream state machine
refused the *real* response as a second header block that does not end the
stream. Both read past them now, bounded at eight because a head with no body
costs a server almost nothing to send.

The HTTP/2 half needed one thing worth reading twice. `Stream` tells a response
from its **trailers** by whether a block has already arrived, and an interim
response is neither — so `headers_were_interim` is **told** rather than worked
out, because a `103` and a `200` are the same frame and only the decoded
`:status` tells them apart, three layers above where the rule lives.

**A redirect that demotes a `POST` now drops the body**, by exactly the
condition that already dropped `Content-Length` and `Content-Type`. It has to be
the same condition: a `GET` carrying a body its headers no longer describe is a
message the next server frames by guessing. `307` and `308` keep both, which is
what they exist for.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero errors
— it caught the real thing, refusing `exchange_however_it_ends` at 151 lines,
which is how `send_request` and `Assembling` came to be separate from the
exchange loop rather than inside it — **1391 tests** (up from 1365), no stubs,
no `unsafe`, boundaries held, the licence notice on the new file, and a
`CHANGELOG.md` line. No layout assertion and no reference render: nothing here
positions, sizes or draws.

**`LOOP.md`'s hostile-input clause bites here and is answered by name**, because
every new reading surface is bytes a stranger sent: an interim response that
ends the stream, more interim responses than anybody could mean, a trailer block
carrying a pseudo-header, a `DATA` frame before any headers said what message it
belongs to, and a `MAX_FRAME_SIZE` outside the protocol's range. Each is refused
with a reason rather than believed, and each has a test named after it.

**`ROADMAP.md`.** The line moved is *"HTTP/1.1, then HTTP/2"*. Its `Owed:` half
said *"a request with a body over HTTP/2, queue item 163"* and now says what was
built; the new `Owed:` is item 187, the expectation. It stays an empty box:
HTTP/3 and QUIC are on their own line, and this line is not finished while a
request with a body cannot make a promise it keeps. `docs/features.md` gains two
lines, the second of which is the interim-response finding — it was not in that
file because nobody knew it was missing.

**What the next iteration should know.** The first unticked items in file order
are still **157** and **158**, both blocked on an interface to choose in.
**Item 187** is ready in the sense that it is unblocked, and it should not be
taken next: the refusal it replaces is unreachable from any page, and the item
says in itself to wait for an upload that wants it. So the next ready item is
**164, the preflight cache** (depends on 61 and 56, both done), or **165,
Content Security Policy**, which is the larger and the more owed of the two —
`ROADMAP.md`'s security line names it and the rule that matters most in it is
already written down: a directive this engine cannot parse must make a policy
*more* restrictive, never less.

And item 183, the fieldset border, is still the one iterations 70 and 72 to 79
each named and nobody has taken.

---

## Iteration 81 — queue item 164: the preflight cache

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 164 was
one of the two iteration 80's journal named as next. It was taken over 165
(Content Security Policy) for the reason `LOOP.md` gives about size: 165 is a
grammar, a source-expression language and a reporting channel, and the rule that
matters most in it — an unparseable directive making a policy *more* restrictive
— is worth an iteration that is not also building three other things. 164 is one
question, and 187 says in itself to wait for an upload that wants it.

**The whole file is one rule applied four times**, and it is written at the top
of `preflight.rs` in those words: *what is remembered is what a server actually
said about a request that was actually made.* A preflight cache is a store of
**permissions**, so the only interesting way it can be wrong is by handing out
one nobody granted, and every design decision here is that rule again:

- A `*` in `Access-Control-Allow-Methods` or `Access-Control-Allow-Headers` is
  remembered as **the method and the headers this request asked for**. `*` is
  "and anything else you care to ask", which is a sentence about the request in
  front of the server rather than a standing offer — so a page allowed a `PUT`
  still asks about a `DELETE`. The nice consequence is that item 61's rule that
  a wildcard never covers `Authorization` **needs no restatement here**: a
  header is in the entry only because a server named it.
- An answer given to a request **without** credentials does not cover one with
  them; the server was never shown the harder question. The other direction
  does, and that is not symmetry for its own sake: a server that agreed to be
  read by this origin *with* cookies has agreed to the stricter case.
- **`Preflights::allowed` is the only way in**, and it calls
  `cors::asking_first_allowed` before it stores. Remembering a permission the
  server refused is then not a thing a caller can do by calling two functions in
  the wrong order.
- **`must_ask` is the only way out**, and it asks `cors::needs_asking_first`
  itself. A caller that consulted the cache first would skip a preflight for a
  request that needed one whenever some earlier request to the same URL had
  happened to need one and been allowed.

**The key is the site, the origin and the URL, and the opaque case is `None`.**
The site is ADR 0011 section 1 and ADR 0007's argument, unchanged by the fact
that this holds permissions rather than pages: an entry that made one site's
request faster because another site had already asked answers *have you been
there* to anybody who times a load, and survives clearing cookies. The origin is
finer than the site and is also required — an answer names an origin. And an
**opaque origin is never a key**, because every one of them serialises to
`null`: a key containing one would be shared between pages that are by
definition not each other, which is the rule `alo_url` states as a type.

**Nothing here reads a clock**, which is item 56's shape and was the real
content of the stated dependency on it. Every expiry in the tests is a named
moment and the pairs either side of one are the assertions — `59s: reuse`,
`60s: ask`, `61s: ask` — because a single moment in the middle passes against a
cache that never expires anything.

**Two hours is the cap, and the reason is written down.** A preflight answer is
a permission, and a permission nobody can revoke is not one: a server that wrote
`Access-Control-Max-Age: 31536000` once should not have to wait a year out to
change its mind about who may `DELETE`. Same argument as `cookie::LONGEST_LIFE`,
on a much shorter scale, because nothing here is a preference anybody chose.

**`LOOP.md`'s hostile-input clause bites on one header and is answered as a
table.** `Access-Control-Max-Age` is a number a server chose, which means it is
not necessarily a number, and the twelve rows say what each reading is worth
rather than only that it did not crash — which matters because *not remembered*
and *remembered for the default five seconds* are different outcomes and my
first draft of that table conflated them. Zero and negative are a server
declining. Unreadable is a server saying nothing, which Fetch gives five
seconds. Anything above `i64` is enormously above the cap and so is the cap —
reading `10000000000000000000` as five seconds would be defensible and would
surprise the only kind of person who writes it. And a clock so near the end of
representable time that two hours does not fit in it is refused rather than
overflowed; finding the end of time portably took six lines in the test, because
a test that panicked while building its own argument would have proved nothing.

**Three rules were doctored out and the named test failed each time**: the
credentials guard, the wildcard collapse, and the opaque-origin key. Run rather
than reasoned about, per the iterations before this one.

**What the item did not ask for and the work found: the safelist is a rule about
a value, and two of the three places that ask it used only the name.**
`Content-Type` is safelisted and `application/json` is not one of the three
values a form can produce. `needs_asking_first` knew that; `asking_first` and
`asking_first_allowed` did not — so a JSON post was **correctly preflighted with
a question that never named `Content-Type`**, and then allowed by a server that
had said nothing about it. That is the permissive direction, and it is one of
the most ordinary requests on the modern web.

It was fixed rather than cut, because the cache could not have been built
correctly around it: a cache's whole job is deciding whether two requests are
the same *shape*, and it would have inherited whichever answer it was given.
There is one function now — `cors::names_a_form_could_not_have_sent` — and all
three callers plus the cache take it. `needs_asking_first` became two lines as a
result, which is the usual sign the rule was in the wrong place.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1410 tests** (up from 1391), no stubs, no `unsafe`, boundaries held,
the licence notice on the new file, and a `CHANGELOG.md` line. No layout
assertion and no reference render: nothing here positions, sizes or draws. One
file one responsibility — `preflight.rs` answers *have we already asked*, which
is a different question from `cors.rs`'s *may this be done*, and the two are
joined by one function rather than by shared state.

**`ROADMAP.md`.** The line moved is *"The same-origin policy, CORS and preflight
(queue item 61)"*, which was ticked with `· Owed: the preflight cache, queue
item 164`. **The Owed clause is discharged rather than a box being ticked** —
the line was already `[x]` under the third state, *done with any remainder
stated*, and what changed is that there is no remainder. `docs/features.md`
gains two lines: the cache, and the `Content-Type` finding, which was not in that
file because nobody knew it was missing.

**What the next iteration should know.** Nothing calls `Preflights` yet, and
that is not an omission of this item: nothing calls `cors` either, because there
is no fetch pipeline to call it from. That is **queue item 83** (`fetch()` and
`XMLHttpRequest`, over the same stack as everything else), which depends on 72 —
the interpreter — and is therefore behind the whole of section D. Every piece of
CORS in this crate is a decision function waiting for a caller, and this one was
built in the same shape deliberately.

The first unticked items in file order are still **157** and **158**, both
blocked on an interface to choose in. **187** is unblocked and says in itself to
wait. So the next ready item is **165, Content Security Policy** — the larger
and the more owed of what remains in section B, and the last `Owed:` clause on
`ROADMAP.md`'s security line. It should be scope-cut on starting: the directive
grammar and the source expressions are one thing and reporting is another, and
the rule that must survive whatever is cut is already written into the item —
**a directive this engine cannot parse makes the policy more restrictive, never
less.**

And item 183, the fieldset border, is still the one iterations 70 and 72 to 80
each named and nobody has taken.

---

## Iteration 82 — queue item 165: Content Security Policy

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 165 was
the one iterations 80 and 81 both named as next, and the last `Owed:` clause on
`ROADMAP.md`'s security line. **Scope cut on starting**, per `LOOP.md`, and the
cuts are items 188 and 189 rather than remarks: reporting is a channel and a
report format, and computing a hash means renting a digest, and neither is the
thing that closes this item.

**This is the first rule in the crate that protects a site from itself.** Every
other one — CORS, HSTS, mixed content, cookies, the cache — is the browser
protecting a person from a site. `script-src 'self'` is a *site* saying *if a
script from anywhere else ever appears in me, something has gone wrong and you
should refuse it*, and the whole value of that sentence is that it holds on the
day the page is wrong about its own escaping. That is why the closing condition
is an injected script and not a parsed header.

**The rule that matters more than any single directive is three holes, not
one.** The item states it once — *a directive this engine cannot parse makes the
policy more restrictive, never less* — and writing the code found three separate
ways to violate it, each of which had to be closed on its own:

- A **source expression** we cannot read is kept and matches nothing
  (`Source::Unreadable`), rather than being dropped from its list.
- The **directive holding it** is kept whole. Discarding it would send its
  requests to `default-src`, or to nothing at all, and either is wider than what
  the author wrote. This is the one that is easy to get wrong, because throwing
  away a value you could not parse feels like the careful thing to do.
- A **directive name** we do not act on grants nothing — and is *named*, by
  `Policies::not_enforced`. That third one is not restrictiveness, it is
  honesty: `frame-ancestors 'none'` in a policy this engine does not enforce is
  a gap, and a gap nobody prints is a false sense of security.

Two more rules turned out to have the same shape and went in with it. **A
repeated directive keeps the first**, which the specification asks for and which
has a security reason worth writing down: anybody who can *append* to the header
— a reflected value, a careless proxy — could otherwise widen a policy by
restating one of its directives. And **two policies are an intersection**, so
adding a policy can only ever narrow what an existing one allowed.

**Two files, because a new source form and a newly enforced directive are
different reasons to change.** `csp_source.rs` is the grammar and the matching
algorithm — one question, *does this URL match what the author wrote* — and
`csp.rs` is the directives, the fallback to `default-src`, the intersection and
the refusal. Same split as `cors.rs` and `preflight.rs`, for the same reason.

**`Policy` is deliberately not public.** No caller may check one policy on its
own: checking one is how a report-only policy comes to block something, and how
the second of two policies comes to be forgotten. `Policies` is the only way to
ask, and it is what makes the intersection and the disposition rules
unbypassable rather than remembered.

**Eight rules were doctored out and the test named for each failed** — run
rather than reasoned about, as the last several iterations have done. Dropping
an unreadable source; discarding the whole directive; keeping the *last*
repeated directive; enforcing a report-only policy; letting `'unsafe-inline'`
win over a nonce beside it; `https` reaching `http`; ignoring
`'strict-dynamic'` and honouring the hosts anyway; a source with no port
matching any port. The one worth recording is the *third*: my first doctoring of
"a repeated directive keeps the first" changed nothing, because the dedup at
parse time and the find-first at lookup time are two guards of one rule and
either alone holds it. Breaking it needed both. A doctoring that passes is not
evidence the rule is safe — it can just as easily mean the doctoring missed.

**`LOOP.md`'s hostile-input clause bites hard here and is answered as two
tables.** A policy is a sentence a stranger's server wrote, and the ways to be
wrong about it are not exotic: an unterminated quote, `://` with nothing either
side, a port of `99999`, a host in Unicode this engine could never compare with
anything, four hundred sources in one directive, the largest header
`crate::http` will read. Nothing allocates on a number a server chose — the
8 KB header bound and the 200-header bound are already `http.rs`'s, which is why
there is no third bound here — and every token becomes a source, so there is no
path where a value is silently absent.

**Two things are narrower than a browser, on purpose, and say so.** A host
written in Unicode is unreadable rather than never-matching, because every host
this engine holds is already in ASCII and a silent never-match is a page that
stopped working for no stated reason. An IPv6 literal is unreadable, because
CSP's host grammar has no spelling for one and inventing a spelling would be
inventing a rule about a security boundary.

**One gap is a design decision rather than a cut, and it is written into the
module.** A **document load is not governed**. CSP governs a *nested* document
and deliberately does not govern a top-level navigation — clicking a link off a
site with `default-src 'self'` must still work — and this engine cannot tell one
from the other: `Purpose::Document` with an initiator is a link click and an
`<iframe>` alike. Guessing either way is bad in a way somebody would notice:
governing it breaks every link on a protected page, and the alternative protects
nothing. So `frame-src` is in `not_enforced()`, and item 86 is where a nested
document becomes a thing with a name.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1461 tests** (up from 1410), no stubs, no `unsafe`, boundaries held —
no crate was rented, which is what item 189 exists to do properly — the licence
notice on both new files, and a `CHANGELOG.md` line. No layout assertion and no
reference render: nothing here positions, sizes or draws. Clippy's
`match_same_arms` fired twice and both were real — one of them turned a
three-arm match into a clearer two-line `or`.

**`ROADMAP.md`.** The line moved is *"Content Security Policy, referrer policy,
HSTS, mixed-content blocking"*, which stays `- [ ]` with its `· Built:` clause
extended and its `· Owed:` clause rewritten from *"CSP, queue item 165"* to the
three things actually outstanding: reporting (188), a computed hash (189), and a
nested document (86). **It is not ticked**, because it is not done — the
temptation to tick after building the largest word in the line is exactly what
`ROADMAP.md`'s three states exist to refuse. `docs/features.md` gains three
lines.

**What the next iteration should know.** Nothing calls `Policies` yet, which is
the same sentence iteration 81 wrote about `Preflights` and is true for the same
reason: every piece of page-level security in this crate is a decision function
waiting for a fetch pipeline, and that is **item 83**, behind the whole of
section D. This one was built in that shape deliberately.

Section B now has three unticked items and each is genuinely blocked or
deferred: **157** and **158** need an interface to choose in, and **169** (the
Linux sandbox) says in itself that it must be run on Linux. **187** is unblocked
and says in itself to wait for an upload that wants it. So the next ready items
in file order are **170** (fonts a page asks for by name, which item 68's own
corpus case is the evidence for), **64** and **65** (the renderer lifecycle,
both depending on 63 which is done), and **66** (where one site ends and
another begins — much of which `alo_url::site` already answers since item 156,
so it should be read before it is built).

And item **183**, the fieldset border, is still the one iterations 70 and 72 to
81 each named and nobody has taken. Eleven now. It is a small item with a
reference render, it depends on nothing, and the reason it keeps being skipped
is that every iteration finds something with a security argument attached
instead. That is worth one iteration deciding on purpose rather than deferring
again.

---

## Iteration 83 — queue item 183: a fieldset looks like a group

**The tree was clean on entry and `scripts/gate.sh` was green.** This is the
item iterations 70 to 82 each named and nobody took — twelve now — and
iteration 82's own journal said out loud that it was "worth one iteration
deciding on purpose rather than deferring again". So it was taken on purpose.
It is not the first item in the file: 187 says in itself to wait for an upload
that wants it, 60 is HTTP/3, and 188 and 189 are CSP's channel and a rented
digest. This one depends on nothing, is opened by a page in the corpus, and
closes with a picture.

**The interesting part is not the border, it is the band.** A `<fieldset>` with
no border was the symptom — three radio buttons under "Pizza Size" looked
exactly like three radio buttons, which is the one thing a fieldset is *for*
being the one thing invisible — but the border alone is four lines of the
user-agent sheet. What the item is really about is that a legend sits **in**
the block-start border rather than above it, and CSS has nothing else shaped
like that: every other border in the language goes all the way round.

So `alo_layout::legend` states it as one rule and the rest of the engine knows
nothing about fieldsets: **a fieldset showing a legend has a band where its
block-start border would be**, as tall as the legend, with the border drawn
through the middle of it and not drawn behind the legend at all. The layout run
is given *no* block-start border — the band stands in for it — and the band is
recorded afterwards, carrying the stroke the style asked for and the gap the
legend leaves.

**The band replaces the border rather than adding to it, and that is the whole
difference between right and nearly right.** Laying the legend out as an
ordinary first child and then raising it would have left the fieldset the
border's own thickness taller than a browser draws it: two pixels, on every
fieldset, forever, and invisible until somebody put this engine beside another
one. Asserted in numbers rather than reasoned about — a fieldset holding one
line is 49.6 tall and one with no legend is 35.6, which is the same box with
its sixteen pixels of legend swapped for its two of border.

**Three decisions in the box tree, each of which could have gone the other
way.**

- The legend is **hoisted to the front**, because HTML draws a fieldset's
  *first legend* at the top whatever comes before it in the document. That
  cannot be inferred in layout, which has boxes and styles and no document, so
  the tree that does have the document records it — a side map, the same shape
  and the same argument as `natural`: almost no box is a fieldset.
- A fieldset the author made a **flex or grid container** has no rendered
  legend. Its children are items in an arrangement somebody wrote, and lifting
  one of them into the border would be this engine overruling them.
- An **inline-level** legend is not one either. The check runs over the
  *arranged* children, so a legend that ended up in a run with the text beside
  it is simply not found — the rule falls out of the shape rather than needing
  a case.

**There is no anonymous "fieldset content" box**, which the HTML specification
does describe. It was not needed: with the legend hoisted and the block-start
border zeroed, the padding lands where a browser puts it and the fieldset comes
out the right height. Adding a box nothing needs would have changed every
fieldset's agent tree for a structure no assertion could see.

**`solid` where every other browser draws a `groove`, and it is written into
the sheet.** This engine draws only solid borders and says why — a style drawn
as a different style is a wrong pixel that looks nearly right — so the fieldset
gets the colour a groove is made of, drawn the one way we can draw it honestly.
That is a substitution, and a substitution nobody writes down is one nobody
re-checks (queue items 47 and 49 are both about exactly that going wrong), so
it is named in `user_agent.rs`, in `docs/conformance.md`, in the changelog, and
as **queue item 190**.

**Two doctorings, run rather than reasoned about.** Drawing the block-start
border whole rather than in two pieces: the two paint tests failed and both
corpus cases failed with it. Letting the band add to the border rather than
replace it: the two layout assertions failed and the no-legend one stayed
green, which is what says they are testing the band and not the border.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1475 tests** (up from 1461), no stubs, no `unsafe`, boundaries held —
nothing was rented — the licence notice on both new files, and a `CHANGELOG.md`
line. The half no script can check: a **layout assertion in numbers**
(`numbers.rs`, three tests, every number written out), a **reference render**
(corpus case `fieldset-group`, and `web-a-form`'s two groups have their borders
now), one responsibility per file — the band is its own file rather than more
of `engine.rs` — and the item is in `docs/features.md`. Clippy's `float_cmp`
fired three times on assertions I had written with `assert_eq!`, and it was
right each time.

**`ROADMAP.md`.** The line moved is *"Forms: the controls, constraint
validation, submission, file inputs"*, whose `· Built:` clause gains the
fieldset beside item 182's control states. **It is not ticked**, and the
`Owed:` clause says why in its own words: everything a control *does* needs
events, and the focus ring needs something to have focus.

**What the next iteration should know.** `docs/conformance.md`'s controls
section is now accurate again, and the one remaining hole in it is still the
focus ring — item 43, blocked on item 81, and genuinely blocked rather than
skipped. The ready items in file order are **170** (fonts a page asks for by
name, which item 68's corpus case is the standing evidence for), **64** and
**65** (the renderer lifecycle), and **66** (where one site ends and another
begins — `alo_url::site` already answers much of it since item 156, so read it
before building it). **190** is new, small, depends on nothing, and is the same
kind of item this one was: a visible thing a real page asks for, with a
reference render as its answer.

One thing this iteration did *not* do and should be said plainly: a fieldset
with a `border-radius` and a legend draws square corners. The shape that
answers a rounded corner with a hole in one side properly is item 19's kind of
work, and drawing an approximation would have been a wrong pixel on the one
element this code exists for. It is written into `alo_paint`'s own doc comment
where somebody would otherwise add it, and into item 183 in the queue.

---

## Iteration 84 — queue item 188: a policy that was violated says so

**The tree was not clean on entry, and that is the first thing to record.** The
working tree held an interrupted iteration's work on this item — `csp_report.rs`
and `a_violation_a_page_reports.rs` written, `csp.rs`, `pool.rs`, `request.rs`,
`mixed.rs`, `lib.rs` changed, `ROADMAP.md` and `docs/features.md` already moved
— staged and never committed. `scripts/gate.sh` failed on exactly one clause:
*crates changed and CHANGELOG.md did not*. So the worker stopped between the
code and the commit, which is where `LOOP.md` says a hung one is presumed to
have stopped, and its item is redone.

**It was finished rather than discarded, and it was read before it was
believed.** Every file was read in full and the gate was run whole before
anything was added to it; the three clauses the item closes on were checked
against the tests that claim them rather than against the fact that the suite is
green. What this iteration wrote is the `CHANGELOG.md` entry, the queue's
`Done` paragraph, and this journal — the three things the gate and step 6 ask
for and the interrupted worker never reached.

**All three of the item's clauses are closed.** An enforced policy and a watched
one both report (`a_policy_being_enforced_and_one_being_watched_both_report`,
and two posts come out, one per policy). A report says which directive and which
URL without saying more than it may
(`a_cross_origin_url_reaches_a_collector_as_an_origin_and_nothing_more`, which
asserts the *absence* of the token, the path and the fragment rather than the
presence of the origin). And a report that cannot be sent is not a load that
fails (`a_report_that_cannot_be_sent_is_not_a_load_that_fails`, against a port
nothing listens on, ending by asserting the load's own answer is what it was
before anybody tried).

**The deciding is a pure function and the sending is a loop**, which is the
shape items 55 and 154 already use and is here for the same reason: what a
report may say is a rule about a *stranger's URL*, and such a rule is asserted
honestly only when nothing is moving. `csp_report.rs` builds the posts;
`Pool::report` is the only part that touches a socket, and it returns what
failed rather than an error anybody's page sees.

**Three decisions in it are worth reading twice.**

- A report names the **effective** directive rather than the deciding one, so
  `default-src 'none'` refusing a script reports `script-src`. Both come out of
  one function (`Policy::objects_to`), because computing the effective
  directive a second time in the reporting path is how the report and the
  message a person reads come to disagree.
- **`report-to` wins over `report-uri` when it resolves**, and reports nowhere
  when its group was never defined — named in `Posting::unusable` rather than
  falling back, since falling back would be this engine deciding that an author
  who wrote a group name meant something else.
- A report is its own **`Purpose::Report`** rather than a fetch, and that is a
  rule instead of a label: a policy governs what its page loads and
  deliberately does not govern its own reporting, so a report sent as a
  `Fetch` would be silenced by `connect-src 'none'` exactly when it had
  something to say. `mixed.rs` refuses it over plain HTTP for the opposite
  reason — it carries the URLs a secure page was refused.

**The fields nothing here can honestly fill are omitted rather than zeroed**,
and a test asserts their absence: `line-number`, `column-number`,
`source-file`, `script-sample`. A `"line-number": 0` is a wrong answer that
reads like a right one, and a field nobody sent is one an author can see is
missing.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1505 tests** (up from 1475), no stubs, no `unsafe`, boundaries held —
nothing was rented, which is still what item 189 exists to do properly — the
licence notice on both new files, and a `CHANGELOG.md` line. The half no script
can check: no layout assertion and no reference render, because nothing here
positions, sizes or draws; one responsibility per file — `csp.rs` decides and
`csp_report.rs` tells, which are different reasons to change; and the item is in
`docs/features.md`.

**`ROADMAP.md`.** The line moved is *"Content Security Policy, referrer policy,
HSTS, mixed-content blocking"*, whose `· Built:` clause gains reporting and
whose `· Owed:` clause loses it, leaving the two things actually outstanding: a
computed content hash (189) and a nested document (86). **It is still not
ticked**, and it should not be until those two are.

**What the next iteration should know.** Nothing calls `Policies` or
`Pool::report` from a page load yet — the same sentence iterations 81 and 82
wrote about `Preflights` and `Policies`, and true for the same reason: every
piece of page-level security in `alo-net` is a decision function waiting for a
fetch pipeline, which is **item 83**, behind the whole of section D. This is
now three built-and-uncalled security surfaces, and it is worth saying plainly
that the number is growing: they are each individually correct and none of them
protects anybody until something calls them.

Item **189** is now the only cut left from 165 that is not blocked on section D
or E, and it is the first item in stage 2's file order that is ready: it needs
a digest, which means **renting one** (ADR 0001) with an entry in
`scripts/gate.sh`'s boundary list — the first rented crate since `jpeg_decoder`.
Its one caveat is written into item 189 itself: there is inline *style* to hash
today and inline script needs item 72, so the item closes on `<style>` and says
so. After that the ready items in file order are **170** (fonts a page asks for
by name), **64** and **65** (the renderer lifecycle), **66** (much of which
`alo_url::site` already answers), and **190** (the two-tone border styles, small,
depends on nothing, and closes with a picture).

---

## Iteration 85 — queue item 189: a content hash, computed

**What was built.** A Content Security Policy may allow inline content by
naming its digest — `style-src 'sha256-…'` — and this engine has read that
sentence since item 165 without being able to act on it: the hash source was
parsed, its presence correctly disabled `'unsafe-inline'`, and the content was
refused with a message saying a hash would have allowed it. It computes one
now. `crates/alo-net/src/digest.rs` is the new file, `sha2` is the rented
digest behind it (ADR 0001 — a hash function is physics), and
`scripts/gate.sh`'s boundary list has the entry that keeps it there.

**All three of the item's clauses are closed**, in
`crates/alo-net/tests/a_hash_a_policy_named.rs`: an inline `<style>` whose
digest a policy names applies, one whose digest it does not is refused
(including the same rule one space longer, which is the case an injection
actually produces), and both alphabets are read.

**The work was not the hash. It was reading the value an author wrote**, and
that is why `digest.rs` holds the base64 as well and why every rule in it is
written down rather than implied. A hash source is a *permission*, so a decoder
that is lax in any direction is a policy quietly wider than its author wrote —
which is the same argument item 165 was built around, one layer further down.
So: the two alphabets are never mixed in one value, a value whose last group
holds bits standing for no byte is refused as a second spelling of one
permission, nothing is trimmed and no whitespace is skipped. Both of those
strictness rules were **doctored out and the test named for each failed**, in
the unit tests and in the integration table alike; the table's mixed-alphabet
row had to be rewritten to a SHA-512 to test the rule it claimed to, because the
first draft was the wrong *length* and was being refused a row earlier.

Two decisions in it are worth reading twice. A value of the wrong length for the
algorithm it names is a **non-match rather than an error**: `'sha256-YWJj'` is
an author's mistake, and the honest way for them to see it is content that does
not run. And the comparison is over **bytes** rather than text, which is what
makes one digest of ours enough — comparing spellings would mean producing our
own digest in both alphabets and with and without padding, and comparing against
each.

**Nothing here is constant-time, deliberately**, and `digest.rs` says so: both
sides are public. The content is the page's own and the expected value is in a
header anybody can read.

**What it found while it was there.** `Source::matches` was refusing a hash for
a URL for the reason "nothing computes one", which was about to become false. It
still refuses, and the reason is now the right one and is written down: a policy
is checked *before* anything is fetched, so a `<script src>` is allowed by where
it comes from and never by the digest of what arrives.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1525 tests** (up from 1505), no stubs, no `unsafe`, boundaries held
with `sha2` behind its new one, the licence notice on both new files, and a
`CHANGELOG.md` line. The half no script can check: no layout assertion and no
reference render, because nothing here positions, sizes or draws; one file one
responsibility — `digest.rs` answers *is this content the digest an author
named*, which is one question, and `csp_source.rs` and `csp.rs` keep the grammar
and the policy they already had; and the item is in `docs/features.md`.

**`ROADMAP.md`.** The line moved is *"Content Security Policy, referrer policy,
HSTS, mixed-content blocking"*, whose `· Built:` clause gains the computed hash
and whose `· Owed:` clause loses it. **It is still not ticked**: what remains
owed is a nested document (item 86) and `'unsafe-hashes'` (item 191, new).

**One cut, and it is item 191.** `Policies::allows_inline` now takes the
content, and takes `None` for content that has no element of its own — a `style`
attribute, an event handler. Hashing those is what `'unsafe-hashes'` enables,
this engine reads that keyword without acting on it, and deciding it silently
either way would be guessing about a permission. So the refusal names it:
`ByHash` is three answers rather than a bool, because "no hash was involved",
"your digest does not match this content" and "this is content we will not hash"
send an author to three different places.

**What the next iteration should know.** The signature change is the thing to
notice: `Policies::allows_inline` and `Policies::inline_violations` both take
the content now, and they must always be passed the *same* content — a report
saying a policy objected to something the policy allowed sends an author looking
for a bug in a page that works. There are still no callers: this is the fourth
built-and-uncalled security surface in `alo-net`, after `Preflights`,
`Policies` and `Pool::report`, and the number is still growing for the same
reason — every one of them is waiting on a fetch pipeline, which is **item 83**,
behind the whole of section D.

The ready items in stage 2's file order are now **170** (fonts a page asks for
by name, which item 68's corpus case is the standing evidence for), **64** and
**65** (the renderer lifecycle), **66** (where one site ends and another begins,
much of which `alo_url::site` already answers since item 156), **190** (the
two-tone border styles — small, depends on nothing, and closes with a picture),
and **191** above, which is small and whose second half waits on item 81.

---

## Iteration 86 — queue item 191: `'unsafe-hashes'`

**The tree was clean on entry and `scripts/gate.sh` was green.** This item was
the first ready one in stage 2's file order, and the previous iteration named it
as such: 157 and 158 need an interface to choose in, 169 says in itself that it
must be run on Linux, 187 says in itself to wait for an upload that wants it,
and 60 is HTTP/3.

**What was built.** Item 189 taught this engine to compute a content hash, and
left one thing it would not hash: content with no element of its own — a `style`
attribute, an event handler. Matching one of those by digest is exactly what
`'unsafe-hashes'` enables, the keyword was read and inert, and so
`Policies::allows_inline` took `None` for such content and refused it by name.
It matches one now, and only where the page asked for it in words. The item's
condition is closed in `crates/alo-net/tests/a_hash_a_policy_named.rs`:
`a_style_attribute_needs_the_keyword_as_well_as_the_digest` runs the same
digest under two policies, and the keyword is the whole difference between them.

**The shape is the thing worth reading rather than the keyword.** *Where*
content was written became a type of its own — `csp::Placement`, reached through
`csp::Content::element` and `csp::Content::attribute` — beside the `Inline` kind
that was already there. They are separate because they answer different
questions: the kind chooses `script-src` or `style-src`, and the placement
decides whether a hash in that directive may apply at all. Folding them into one
four-member enum, the way the specification names its own type ("script",
"style", "script attribute", "style attribute"), would have meant adding a
member nothing can construct until item 81.

So **the event handler half needed no code and no case**, which was the right
answer to a half the item told this iteration to leave alone: an event handler
is `Inline::Script` with `Content::attribute`, item 81 will pass it without
changing anything here, and what is genuinely owed to 81 is a handler to pass
rather than a rule to write. The message already says "an event handler" for
that pair, and a unit test asserts it, because a total function over four cases
is cheaper to test than to leave for later.

**Three rules went in with it, each because the alternative widens somebody's
policy**, and each has a test named for it:

- The keyword **grants nothing on its own**. It is a permission to *hash*, not a
  permission, so `style-src 'unsafe-hashes'` with no digest beside it allows no
  attribute at all — which is what stops it being `'unsafe-inline'` spelt
  differently.
- It is read from the **deciding directive** rather than from anywhere in the
  policy. `default-src 'unsafe-hashes'; style-src 'sha256-…'` does not allow the
  attribute that `style-src` decided about: the keyword is a source expression,
  so the list that decides is the deciding directive's own, and reading it
  otherwise would let a keyword in one sentence widen another.
- Two policies stay an **intersection**, so a second header cannot add the
  keyword to the first one's hash.

**`ByHash::NothingToHash` became `ByHash::NotWithoutTheKeyword`**, and that is a
correction rather than a rename. The old name was true when nothing was hashed;
now there is always something to hash and the honest sentence is *no digest
applies here* — the digest may well match, and the test asserts exactly that
case: the same content refused as an attribute while its digest is in the
policy, with the message not saying "byte for byte", because sending an author
to recompute a digest that is already right is worse than saying nothing.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1530 tests** (up from 1525), no stubs, no `unsafe`, boundaries held —
nothing was rented — the licence notice, and a `CHANGELOG.md` line. The half no
script can check: no layout assertion and no reference render, because nothing
here positions, sizes or draws; one file one responsibility — `csp_source.rs`
reads `'unsafe-hashes'` as a source and `csp.rs` acts on it, which is the split
those two files already had, because the keyword says nothing about any one
source expression and everything about the list it is in; and the item is in
`docs/features.md`.

**`ROADMAP.md`.** The line moved is *"Content Security Policy, referrer policy,
HSTS, mixed-content blocking"*, whose `· Built:` clause gains the `style`
attribute and whose `· Owed:` clause loses `'unsafe-hashes'`. **It is still not
ticked**: what remains owed is a nested document (item 86) and an event handler
matched by its hash, which is now waiting on item 81 rather than on this rule.

**What the next iteration should know.** The signature changed again, in the
same place as last time: `Policies::allows_inline` and
`Policies::inline_violations` take a `Content` rather than an `Option<&str>`,
and choosing the wrong constructor is the way to widen a policy silently — which
is why it is two named constructors rather than a `bool`. There are still **no
callers**: this remains the fourth built-and-uncalled security surface in
`alo-net`, after `Preflights`, `Policies` and `Pool::report`, all four waiting on
a fetch pipeline, which is **item 83**, behind the whole of section D.

Section B now has nothing ready in it. Every unticked item there is blocked or
deferred for a reason written into the item: 157 and 158 need an interface to
choose in, 169 must be run on Linux, 187 waits for an upload that wants it, 60
is HTTP/3, and 67 needs an ADR. So the ready items in stage 2's file order are
**170** (fonts a page asks for by name, which item 68's corpus case is the
standing evidence for), **64** and **65** (the renderer lifecycle, both
depending on 63 which is done), **66** (where one site ends and another begins —
much of which `alo_url::site` already answers since item 156, so it should be
read before it is built), and **190** (the two-tone border styles: small,
depends on nothing, and closes with a picture).

---

## Iteration 87 — queue item 170: fonts a page asks for by name

**The tree was clean on entry and `scripts/gate.sh` was green.** This item was
the first ready one in stage 2's file order, and the previous iteration named it
as such: section B has nothing ready left in it, and 170 is the next line.

**What was built.** A confined renderer cannot open a font file (ADR 0010), so
it starts with whatever short list the browser process found and handed over. A
page asking for anything outside that list was drawn in something else and
**nothing anywhere said so** — a render that was stable, diffable, and not what
the page looks like in any other browser, which is the worst way for a rendering
difference to be wrong: reproducible and unexplained. All three of the item's
clauses are closed in `crates/alo-renderer/tests/a_font_a_page_asked_for.rs`,
over the real boundary with a spawned, confined renderer rather than in-process.

**The distinction the whole item turns on is a type.** `alo_text::Absent` keeps
two things apart that look like one:

- A family that is **not here** is an *ask*. It goes to the browser process,
  which may open a file and so may go and look. It includes a family the page
  listed first and did not get even when a later one was found, because the
  machine may well have the first — and it stops at the first family that *was*
  found, since nothing was ever going to be drawn in the ones after it.
- A **substitution** is a *message to a person*, and happens only when nothing
  the page named was here at all. A page whose second choice was found got the
  fallback its own author wrote; reporting that would put a warning in front of
  somebody about a page working exactly as written.

Folding these into one answer was the tempting mistake, and it would have made
the corpus noisy in a way that reads like a bug.

**`fonts::named` asks the font, not the filename.** `from_this_machine` takes a
family from a file's name and that is right — it only decides what goes in a
database, and opening every font on a machine at startup to ask would be most of
a second before the first page. But *"does this machine have Inter"* decides
whether a page is drawn as its author wrote it, and an answer read off a
filename is wrong for every font somebody else named. So `alo_text::family_in`
reads the `name` table, preferring the typographic family (id 16) over the older
one (id 1), because a large family splits itself under the older name and CSS
means the whole family.

**The name in that request came off a page**, so it is compared against what a
font states about itself and joined to nothing: a family called
`../../../../etc/passwd` finds no font because no font is called that, and there
is a test walking eight such names. The bound is applied twice — in the renderer
that builds the list and again in `Renderers::supply` — because a limit a
renderer applied to itself is not one the browser process may rely on, the
renderer being the process that parsed the page.

**A refactor went in rather than a third copy.** Both `alo_layout::engine` and
`alo_paint::build` privately walked up the box tree to find the style a text box
inherits from, and this needed the same walk. It is `BoxTree::nearest_style` now
and all three call it: three copies of that rule is three chances for one of them
to stop agreeing about which font a line is in, which is a rendering difference
nobody could explain.

**The corpus did not move at all, and that is the review.** Every alo case
declares what its generics mean (`corpus_fonts` maps `system-ui`, `sans-serif`
and `serif`), so nothing in the corpus was ever silently substituted for, and a
rule that reported those would have been the wrong rule. `web-example-com`'s
`origin.txt` named this item as its evidence and said the substitution there was
silent; that is now corrected in the file rather than quietly left, because the
substitution there is **declared** by the corpus harness and a declared one
should not be reported. What was genuinely silent was what nobody had looked at,
and it is written below.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1558 tests** (up from 1530), no stubs, no `unsafe`, boundaries held —
nothing was rented, and `ttf_parser` stayed inside `alo-text/src/font.rs`, which
is why `family_in` lives there rather than beside the code that wanted it — the
licence notice, and a `CHANGELOG.md` line. The half no script can check: no
layout assertion and no reference render, because **nothing here positions,
sizes or draws** — this item changes what a load *reports*, and the strongest
evidence of that is the twenty-four committed renders that did not move; one
file one responsibility — `families.rs` is new and holds only the question of
which families a page asked for, `fonts.rs` gained the on-demand search beside
the startup one it already documented as the shape that would follow; and the
item is in `docs/features.md`.

**`ROADMAP.md`.** The line moved is the process-and-sandbox one, whose `· Built:`
clause gains item 170 beside item 168 — fonts across the boundary, and now fonts
a page asked for across it. **It is still not ticked**, and its `· Owed:` clause
gained a second entry as well as keeping the Linux sandbox.

**What the next iteration should know.** Two cuts, and the second is the more
interesting:

- **Item 192.** `family_in` answers `None` for a font naming itself only in a
  legacy Macintosh encoding — several macOS ships do, Apple Braille among them.
  Such a family cannot be found on demand, so a page asking for it is told the
  machine does not have it. Safe direction, still wrong. Falling back to the
  filename was refused deliberately: it would put the guess back inside the one
  answer that has to be a fact.
- **Item 193**, which this item made *audible* rather than fixed.
  `FontDatabase::map_generic` exists and **only tests call it**. The browser
  process hands over faces and never says which is this machine's `sans-serif`
  or `system-ui`, so the user-agent sheet's own `font-family: system-ui,
  sans-serif` reaches every real page as two families nobody has. It has always
  been so; the difference is that a load now says so out loud instead of the
  page just looking wrong. A face cannot carry that fact, so it needs something
  new crossing the boundary — which is why it is an item rather than a line.

`FromRenderer::Loaded` changed shape: it carries `wanted` beside `issues`, and
every pattern matching it needed a field or a `..`. The wire format grew a
second list in the same message, and it has its own hostile test — a decoder
that read the issues and stopped would hand up a load wanting nothing, which
reads exactly like a page that asked for nothing.

The ready items in stage 2's file order are now **64** and **65** (the renderer
lifecycle, both depending on 63 which is done — and much of both may already
exist in `host.rs`, so they should be read before they are built), **66** (where
one site ends and another begins, much of which `alo_url::site` answers since
item 156), **190** (the two-tone border styles: small, depends on nothing, and
closes with a picture), and the two cut above, **192** and **193**.

---

## Iteration 88 — queue item 192: a font whose name is only in an old encoding

**The tree was clean on entry and `scripts/gate.sh` was green.** This item is
the first ready one in stage 2's file order — it and 193 are the cuts the
previous iteration wrote in, and they sit above 64 in the file. 187 waits for an
upload that wants it, 157 and 158 need an interface, 169 must be run on Linux.

**What was built.** A font states its family in its own `name` table, once per
platform that was ever expected to read it. Nearly every font carries a Windows
record in UTF-16, which is Unicode; several of the ones macOS ships — Apple
Braille among them — carry **only** the Macintosh records, which are not. Those
were unreadable here, so `family_in` answered `None`, the browser process
reported the family as absent, and a machine that had the font said it did not.

**The rule that decided the scope is the new file's whole reason to exist: read
the encodings somebody else's table defines exactly, and guess at none of
them.** Mac OS Roman and Mac OS Cyrillic are `macintosh` and `x-mac-cyrillic` in
the WHATWG standard, so `encoding_rs` holds Apple's own tables for those two —
the same crate `alo-net` already rents for a page's bytes, rented one layer away
for a font's name, and `crates/alo-text/src/macintosh.rs` is its second
boundary file in `scripts/gate.sh`. The other twenty Macintosh encodings have no
such table. Mac OS Japanese is close to Shift JIS and is not Shift JIS, and
decoding one as the other would put a character Apple never wrote into somebody's
font name: **a family read wrongly is worse than a family not read**, because it
is a name a page can match by accident. They answer nothing, which is exactly
what every Macintosh record did before this iteration — so the direction the
engine fails in has not changed, only how often it fails.

**A Unicode name wins wherever a font has one.** This is the rule that keeps the
change from touching any font that already had a readable name, and it is not
the obvious implementation: a Macintosh record comes **first** in a well-formed
table, so reading in file order would have quietly demoted every font carrying
both — and Mac OS Roman cannot spell a family that UTF-16 can, so the demotion
would have been a worse name rather than a different one. `Stated` holds four
slots rather than two because *which* name and *how readable* it is are separate
questions: the typographic name wins because it is the family CSS means, and a
Unicode record wins within each because it can spell more.

**Two rules went in beside it, applied to every record whatever its encoding**,
because a font file was written by somebody else: a name longer than
`LONGEST_NAME` is not a family name, and neither is one carrying a control
character. Both skip rather than trim — a name half-cleaned is a name that
matches something by accident, which is the same sentence as the paragraph
above and is why they are here rather than in a later item.

**The tests build their fonts rather than looking for one.** A test that went
hunting for Apple Braille would pass on one machine, skip on every other, and
say nothing about *which* encoding was read. So each case takes a real font and
replaces its `name` table byte by byte, and the encoding is named in the bytes:
`0xD5` is a right single quote in Mac OS Roman and `Õ` in Latin-1, so a decoder
that had quietly fallen back to Latin-1 fails, where a test written in ASCII
would have passed either way. The hostile half is the other reason to build the
tables: a table lying about its own lengths, every truncation of one, and every
single flipped bit of one are answers rather than crashes — and the same sound
table still reads at the end, which is what stops that test passing because
nothing was looked at.

**The second clause was the cheaper half and the more surprising one.**
`fonts::from_file` named a face after its **file**, and the argument written
beside it was that a startup database is only a guess about what to look at and
that opening every font on the machine to ask would be most of a second before
the first page. The second half of that was wrong, and had been since ADR 0010:
`from_file` reads the whole file already, because a confined renderer cannot
open one and a face is therefore bytes rather than a path. So asking the font
costs a `name` table parse rather than an open. Nothing anywhere returns a
filename as a family now, `named` compares one name rather than deriving a
second, and a font stating no readable family is skipped — a real answer about a
file, since nothing could ever ask for it by name.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1573 tests** (up from 1558), no stubs, no `unsafe`, boundaries held —
`encoding_rs` gained `crates/alo-text/src/macintosh.rs` and is named in no other
new place — the licence notice, and a `CHANGELOG.md` line. The half no script
can check: no layout assertion and no reference render, because **nothing here
positions, sizes or draws** — this item changes which name a font is filed
under, and the strongest evidence of that is the twenty-four committed renders
that did not move; one file one responsibility — `macintosh.rs` holds only the
question of what a byte means in an encoding older than Unicode, and `font.rs`
keeps the question of which of a font's names is its family; and the item is in
`docs/features.md`.

One thing worth knowing about the lints: the panic family is permitted **in test
functions**, which clippy decides by the `#[test]` attribute, so a helper in a
`tests/` file is held to exactly what `src` is. The fixture builders here read
with `get` and convert with `unwrap_or`, and the honest note is in their own
documentation: a table too large to write down would fail the test's own
assertion, which is a better report than a helper's panic.

**`ROADMAP.md`.** The line moved is the process-and-sandbox one again, whose
`· Built:` clause gains item 192 beside 168 and 170 — the browser process finds
a font by the name the font gives itself, and now reads that name whatever
encoding it is in. **It is still not ticked**, and its `· Owed:` clause gained
item 194 beside the Linux sandbox and item 193.

**What the next iteration should know.** One cut, and it is deliberately small:

- **Item 194.** A `Face`'s weight and slant are still guessed from the filename,
  by looking for `bold` and `italic` in it. That is wrong for every file named
  by another convention, and it is a *smaller* wrong than the family was: a face
  filed under the wrong weight is still drawn in the right family, because
  `FontDatabase` chooses among the faces of the family it holds. The `OS/2`
  table states both, and `font.rs` already parses that face — so this is an
  hour's work whenever somebody's page is drawn in the wrong weight.

The ready items in stage 2's file order are now **193** (what a generic family
means on this machine — the gap item 170 made audible, and the one of these that
a real page hits every time, since the user-agent sheet's own `font-family:
system-ui, sans-serif` reaches every page as two families nobody has), **194**
(cut above), **64** and **65** (the renderer lifecycle, both depending on 63
which is done — and much of both may already exist in `host.rs`, so they should
be read before they are built), **66** (where one site ends and another begins,
much of which `alo_url::site` answers since item 156), and **190** (the two-tone
border styles: small, depends on nothing, and closes with a picture).

---

## Iteration 89 — queue item 193: what a generic family means on this machine

**The tree was clean on entry and `scripts/gate.sh` was green.** Two items were
ready and both were cuts from the same parent; the journal's previous entry named
193 first and this took it, because it is the one a real page hits **every**
time: the user-agent sheet sets `font-family: system-ui, sans-serif` on every
document, before anybody writes a line of CSS.

**What was silent.** `FontDatabase::map_generic` has existed since stage 1 and
**only tests called it**. The browser process handed a renderer faces and never
said which of them was this machine's `sans-serif`, so every real page asked for
two families nobody had and was answered by falling off the end of the fallback
chain into whatever face sorted first. Item 170 made that *audible* rather than
fixing it, because what a generic means is a fact about the **machine**, and the
machine is the thing ADR 0010 confines a renderer away from. So it had to cross
the boundary, and a `Face` cannot carry it: `sans-serif` is not a property of any
one font.

**The protocol gained one message in each direction.** `ToRenderer::UseGenerics`
carries the mapping and `Renderers::start` sends it **after** the faces and
before any page — in that order, because a generic names a family and a renderer
asked which of them it can answer before it holds a face would truthfully say
none of them. `FromRenderer::UsingGenerics` is that answer, and it is not an echo
of what was sent: it names only the generics a face actually resolves. A mapping
to a family the renderer was never given would otherwise have the browser process
believing every page here has a `sans-serif` while text kept coming out in
whatever was to hand.

**A generic keeps every candidate this machine has, in preference order**, which
was the one design decision worth making slowly. `FontDatabase` already holds a
generic as *several* families and tries them in turn, so nothing new was needed
to say that `sans-serif` here means `.SF NS` and then `Geneva` — and a character
the first lacks is still drawn by the second rather than by whatever the database
happens to hold. The candidate lists and the choosing live in
`crates/alo-renderer/src/generic.rs`, and `choose` is deliberately separated from
the compiled-in table so that what this file **decides** is tested on every
platform rather than only on the one it was written on.

**Four generics, and the refusal is the point.** `serif`, `sans-serif`,
`monospace` and `system-ui` are what a real page and our own sheet write.
`cursive` and `fantasy` have no answer on any machine that is not a guess —
WebKit says Apple Chancery and Papyrus on macOS and nothing anywhere else — and a
guess here is a page drawn in a typeface nobody chose. They stay unanswered,
which is a state this engine already reports in words.

**Reading a machine had to change in two ways, and the first was a real bug.**
The short list was alphabetical and stopped at the first two dozen *faces*, so
whether a machine had a `sans-serif` at all was decided by where its family
sorted — on this one, twenty-four faces of `.SF Arabic` through `Apple Braille`
would have answered nothing. It looks at up to `MOST_LOOKED_AT` files now, keeps
a family some generic wants even after the list is full, and puts those families
first when it cuts down to `MOST_FONTS`. The second: `from_this_machine` returns
the fonts and the generics **together**, as one `Machine`, because the second is
read out of the first — a caller deriving it again would be two derivations of
one fact, which is the argument `fonts::named` already makes about a filename.

**On this machine** the four are answered: `serif` is `.New York`, `sans-serif`
is `.SF NS` then `Geneva`, `monospace` is `.SF NS Mono` then `Monaco`, and
`system-ui` is `.SF NS` then `Geneva`. Every one of those is a family this engine
read out of a font's own `name` table, and every one is handed to the renderer
that is told about it — which is what the machine test asserts, in both
directions so that it is not vacuous on a machine with no fonts.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero errors,
**1591 tests** (up from 1573), no stubs, no `unsafe`, boundaries held — no rented
crate is named in a new place — the licence notice, and a `CHANGELOG.md` line.
The half no script can check: the **layout assertion in numbers** is
`text_in_a_generic_is_measured_in_the_family_the_generic_means`, and it is the
right assertion for this item rather than a formality — a generic decides what
text is *measured* in and so where every line breaks, so the test lays out one
word three times and asserts that `sans-serif` measures what the family named
outright measures, and that a renderer nobody told measures something else. No
new reference render: nothing here positions or draws anything new, and the
evidence is the twenty-four committed renders that did not move, because every
corpus case has declared its own generics since item 170. One file one
responsibility — `generic.rs` holds only the question of what a generic name
means, `fonts.rs` keeps the question of what is on the machine. The item is in
`docs/features.md`.

**Hostile input.** The two new message shapes are decoded from a pipe, so both
are bounded before anything is read: a count larger than any honest mapping is
refused in **both** directions, every prefix of a mapping is an error rather than
a half-read mapping saying `sans-serif` means nothing, and every single flipped
bit of one is an answer rather than a crash.

**`ROADMAP.md`.** The process-and-sandbox line again, whose `· Built:` clause
gains item 193 beside 168, 170 and 192 — a renderer is now told what the generics
mean as part of being given fonts. **It is still not ticked**; its `· Owed:`
clause keeps the Linux sandbox and item 194, and gains item 195.

**What the next iteration should know.** One new cut, and it is the more
interesting of the two now open:

- **Item 195.** `alo_text::family_in` takes the **first** `name` record of each
  kind, whatever language it is in. macOS's system font states its family
  thirty-five times over — `System Font`, `Police système`, `システムフォント` —
  and this engine is saved from filing it under Catalan only by the accident that
  its Unicode-platform record happens to come first in that particular file. A
  font whose first Windows record is a localised one is filed under a name no
  page will ever ask for, which is item 192's whole failure mode arriving by
  another road. The `name` table states a language id per record, so the fix is
  small and it wants its own iteration and its own fixture.

The ready items in stage 2's file order are now **194** (a face's weight and
slant, still read off its filename), **195** (above), **64** and **65** (the
renderer lifecycle, both depending on 63 which is done — and much of both may
already exist in `host.rs`, so they should be read before they are built), **66**
(where one site ends and another begins, much of which `alo_url::site` answers
since item 156), and **190** (the two-tone border styles: small, depends on
nothing, and closes with a picture).

---

## Iteration 90 — queue item 194: a face's weight and slant, from the font

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 194 is the
first unticked item in the queue's own order whose dependency is done, and the
previous entry named it first among the ready ones. It is the last of item 192's
three cuts to close the same question: **nothing about a face is read off its
filename now.**

**What was wrong, and how ordinary it is.** `fonts::from_file` decided which face
of a family it was holding by looking for `bold` and `italic` in the name of the
file. `Helvetica-Oblique` contains neither word. Neither does
`InterDisplay-SemiBold`, whose weight is 600 and which the old rule filed at 400.
Neither does **`DejaVuSans-Oblique.ttf`**, which is in this repository's own
dependency tree and has been since its first month: the file this engine has
tested with all along was filed upright.

**`alo_text::style_in` is the sibling of `family_in`**, one table further on and
the same argument. It answers the pair rather than either half, because weight
and slant are one sentence written side by side in `OS/2` — a caller asking twice
would parse the same file twice to learn two halves of it. `from_file` no longer
touches the path for anything but opening it, and its two lines now read as what
they are: two tables, two questions, and no guess.

**Two readings are decided rather than left to whatever a clamp does**, and both
are written into the function's own documentation because the alternative is a
number nobody can account for later:

- **Zero is not a statement.** It is what a font writes when it did not say, and
  several do. Brought into the range CSS allows it becomes **1** — a hairline,
  the lightest face CSS can name, and a wrong answer that reads like a right one.
  So the only other thing `OS/2` says about heaviness is read instead, the bold
  bit, and a font stating neither is ordinary.
- **A number wins over that bit where they disagree**, because CSS asks its
  question as a number and the bit is the two-value shorthand older software went
  by. A face stating 300 with the bold bit set is filed at 300.
- **A weight in `1..=9` is kept as written.** Some fonts older than the current
  specification meant the nine-point scale, where 9 was black; today 9 is very
  nearly invisible. The bytes are identical either way, nothing in the file says
  which was meant, and a guess would draw somebody's page in a face nobody chose.

**A missing table is not a missing font.** `OS/2` is the one table a font may
lack and still be a font — some older Macintosh ones do — and such a face is
normal, upright and **kept**: a family of one unlabelled face is most of what is
on a machine. It has not quite said nothing, either: an italic angle in `post`
still counts, so a face that leans and states no table still leans, which is a
test of its own.

**What it does to this machine, checked rather than assumed.** Before, every one
of the twenty-four faces handed to a renderer was 400 and upright unless its
filename happened to carry a word. Now `.SF NS Mono` is **295**, `.SF Compact` is
**1000**, `.Keyboard` is **100**, and `.New York` and `.SF NS` have italic faces.
Those numbers were confirmed against the files with `fontTools` rather than
believed: Apple really does state 1000 for its compact face. That last one is
also the item's cut — see below.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero errors,
**1604 tests** (up from 1591), no stubs, no `unsafe`, boundaries held — `OS/2` is
read through `ttf_parser` in `alo-text/src/font.rs`, which is where that crate is
already permitted — the licence notice, and a `CHANGELOG.md` line. The half no
script can check: the **layout assertion in numbers** is the end of
`a_face_is_weighed_by_the_font_and_never_by_its_filename`, and it is the right
assertion rather than a formality — which face a page is given decides how wide
its text is and so where every line of it breaks. Two files are named wrongly on
purpose and **swapped**, so a rule reading the filename gets both wrong and a
rule reading the font gets both right; the text measured at bold has to match, to
the pixel, what a database holding only the bold bytes measures. No new reference
render: the twenty-four committed ones did not move, and that is the review —
every corpus case declares its own faces, so nothing there was ever going through
`from_file`. One file one responsibility: `style_in` sits beside `family_in` in
`font.rs`, which is the file about what a font says about itself. The item is in
`docs/features.md`.

**Both halves were doctored to check the tests are not vacuous.** With the weight
forced to 400 six of the twelve new `alo-text` cases fail and the renderer one
fails on its first assertion; with the slant forced upright, four different ones
fail. That is worth the two minutes: a test that reads a real font and asserts
what that font happens to say passes whatever the code does.

**Hostile input.** A font file comes from somewhere else, so the new reading gets
the same treatment item 192's did: an `OS/2` table claiming a version from the
future, every truncation of one, and every single flipped bit of one, each an
answer rather than a crash — with a sound table still read at the end, so what
came back from the damaged ones was the engine refusing rather than the engine
failing to look.

**`ROADMAP.md`.** The process-and-sandbox line again, whose `· Built:` clause
gains item 194 beside 168, 170, 192 and 193 — and the clause now says the thing
those items add up to, which is that no part of a face comes from a filename. **It
is still not ticked**; its `· Owed:` clause drops 194, keeps the Linux sandbox
and item 195, and gains item 196.

**What the next iteration should know.** One new cut, and it is the interesting
one:

- **Item 196.** A variable font is one file and many weights, and this item reads
  **one** weight out of it. `SFCompact.ttf` has a `wght` axis and states 1000 in
  `OS/2`, so this engine files the whole family as the heaviest thing CSS can
  name; `SFNSMono.ttf` states 295. Neither number is wrong about the default
  instance and both are wrong about the font. Nothing is drawn in the wrong
  *family* — a family whose only face is 1000 still answers a request for 400 —
  which is why it is a cut rather than a defect here, and why it belongs with the
  variable-font line `docs/features.md` already carries for stage 2.

The ready items in stage 2's file order are now **195** (a font's name in the
language somebody asked for, rather than whichever record the file lists first),
**64** and **65** (the renderer lifecycle, both depending on 63 which is done —
and much of both may already exist in `host.rs`, so they should be read before
they are built), **66** (where one site ends and another begins, much of which
`alo_url::site` answers since item 156), **190** (the two-tone border styles:
small, depends on nothing, and closes with a picture), and **196** above.

---

## Iteration 91 — queue item 195: a font's name in the language a page asks in

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 195 is the
first unticked item whose dependency is done, and the previous entry named it
first among the ready ones. It is the last of item 192's line: 192 made a font's
name readable, 194 took the weight and slant off the filename, and this one
decides **which** of a font's names is the one a page would ask for.

**The item's own account of the damage was too kind, and checking it is the
thing worth repeating.** It said macOS's system font was saved from being filed
under Catalan only by an accident of record order. The accident holds — `SFNS.ttf`
really does carry its unlocalised record first — but the survey that checked it
found four other fonts on this machine that were never saved at all:
`Songti.ttc` filed under `宋體-簡`, `STHeiti Light.ttc` and `STHeiti Medium.ttc`
under `黑體-繁`, `Hiragino Sans GB.ttc` under `冬青黑體簡體中文`. A page asking
for Songti SC was drawn in something else and told so. They are `Songti SC`,
`Heiti TC` and `Hiragino Sans GB` now, and CoreText agrees on each.

The survey was a throwaway test that read every font in `/System/Library/Fonts`,
`/System/Library/Fonts/Supplemental` and `/Library/Fonts` — 663 files — and wrote
what `family_in` called each. Run before and after, it is the whole review of a
change with no picture in it: **five lines moved and 658 did not.**

**The order has three steps and the third is the one that needed evidence.**

- **A record that states no language wins.** The Unicode platform defines no
  language ids, so a record there is not written *in* anything: it is what the
  font calls itself, and the Windows records beside it are its translations.
- **Then English**, in any of the sixteen ids that spell it.
- **Then any other language**, first record winning, because a font in one
  language is still a font somebody has and its own name beats no name.

The third step is where a plausible alternative would have done damage. Ranking
English above the unlocalised record is what `fontTools` does for a "best family
name", and it would have renamed this machine's system font from `.SF NS` to
`System Font` — out from under item 193's `sans-serif` candidate list. **CoreText
was asked rather than reasoned about**: it answers `.SF NS` for that file's
family and keeps `System Font` as the name to *show* a person. So the evidence is
the platform's own reading, and it is written into `Spoken::Unstated`'s
documentation rather than into a commit message.

**The language decides inside a kind of name and never between two kinds.** A
font may state its typographic name in one language and its older name in
another, and the typographic one is still the family CSS means — a language that
outranked the kind would file such a font under a name for four of its faces.
This is also what keeps item 192's rule intact: a Unicode record still beats a
Macintosh one, and that item's test proves it unchanged.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero errors,
**1615 tests** (up from 1604), no stubs, no `unsafe`, boundaries held, the licence
notice, and a `CHANGELOG.md` line. The half no script can check: the **layout
assertion in numbers** is `text_asking_for_the_english_name_is_measured_in_the_
font_that_carries_it` — the same shape item 193's was, because it is the same
consequence: which family a font is filed under decides whether a page asking for
it by name is given it, and so how wide its text is. The face there is the bold
one, so a miss is a visibly different number, and the hit has to match a database
holding only those bytes to the pixel. No reference render: nothing visual moved,
and the corpus cases declare their own faces so none of them goes through this
path at all. One file one responsibility: the reading stayed in `font.rs`, which
is the file about what a font says about itself and the only file in the crate
permitted to name `ttf_parser` — a new file for it would have widened a rented
crate's boundary to say one sentence. The item is in `docs/features.md`.

**Both directions were doctored to check the tests are not vacuous.** With the
language ignored and the first record kept, 7 of the 11 new cases fail; with the
unlocalised record demoted to a translation, exactly the one case that asserts it
fails and no others. That second run is the more useful one — it says the
three-step order is being tested as three steps rather than as two.

**Hostile input.** A language id is two bytes out of somebody else's file, and
the table underneath is rented: `ttf_parser` maps an id to a language by looking
it up in a list, and a list is a thing with an end. So **all 65 536 ids** are
walked, in chunks of a thousand records, and each chunk asserts the answer as
well as the absence of a crash. The cross-check in that test is worth keeping:
the test writes out the sixteen English ids by hand from the specification while
the engine reads the rented table, so two roads meet on every id. It also found
its own fixture bug — four thousand records of fourteen characters is more
storage than a two-byte offset can point into, and the builder had been
saturating quietly — which is now an assertion in the builder rather than a
table of nonsense being asserted about.

**`ROADMAP.md`.** The process-and-sandbox line again, whose `· Built:` clause
gains item 195 beside 168, 170, 192, 193 and 194. **It is still not ticked**; its
`· Owed:` clause drops 195 and keeps the Linux sandbox (169) and item 196.

**What the next iteration should know.** No new cuts — this item closed both of
its clauses and found nothing it had to leave. One thing was noticed and is not a
cut, because it is not wrong: `family_in` reads face 0 of a font collection, so a
`.ttc` is named by its first face. Every `.ttc` on this machine states one family
across its faces, so nothing here is misfiled by it, and the day that stops being
true is the day it becomes an item.

The ready items in stage 2's file order are now **64** and **65** (the renderer
lifecycle, both depending on 63 which is done — and much of both may already
exist in `host.rs`, so they should be read before they are built), **66** (where
one site ends and another begins, much of which `alo_url::site` answers since
item 156), **190** (the two-tone border styles: small, depends on nothing, and
closes with a picture), and **196** (a variable font is one file and many
weights, which is the one that needs a decision about what a face *is* before it
needs code).

---

## Iteration 92 — queue item 196: a variable font is one file and many weights

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 196 is the
first unticked item whose dependency is done — 194, which found it — and the
previous entry named it as the one that needed a decision about what a face *is*
before it needed code. That is the right description and this is the decision:
**a weight stopped being a label and became an instruction.**

A `Font`'s weight is now the instance every face parsed out of its bytes is set
to. So `FontDatabase::chain` hands back fonts **set to the weight asked for**
rather than references to the ones it holds, and the return type changed to say
so: the font a request gets from a variable file is not something the database
has. `Font::at_weight` is the one place that decides, and it is a clone of a
shared `Arc` — for the ordinary one-weight face it is a clone and nothing else.

**The rule the whole item turns on is one line in `best_match`**: a face's
distance from a request is to what it *can be*, not to what it is. That is what
makes one file a candidate at every weight in its range, and it is what item 196
was really complaining about — `SFCompact.ttf` states 1000 in `OS/2`, so this
engine had the whole family down as the blackest thing CSS can name.

**It reaches three parsers of the same bytes and all three had to agree.**
Advances come from a font's `HVAR` table, outlines from its `gvar`, and each is
applied by the parser only once the face has been told which instance it is. A
font measured at 700 and drawn at 400 puts light letters at heavy spacing —
every word visibly loose, and no width assertion would have caught it. So
`Font::face` and `Font::shaper` both live in `font.rs`, which is the file that
knows which instance a font is; `alo-paint` gets the coordinate and the tag as
**plain values** (`alo_text::WEIGHT_AXIS`), because the parser is rented behind
one file in each crate and a tag written out twice is two chances to write it
differently.

**Two fonts were built, and building both was the point.** A machine either has
a variable font or does not, so neither case looks for one.
`alo-text/tests/a_font_that_is_many_weights.rs` writes an `fvar` and an `HVAR`
into a real font and asserts what it *measures*;
`alo-paint/tests/a_letter_at_a_weight.rs` writes an `fvar` and a `gvar` and
asserts that a letter's first point moves by exactly the delta the file states.
Deliberately two different fonts varying two different things: one fixture
carrying both tables would let either half pass on the other's evidence. Both
retag entries the font does not need — `FFTM` and `MATH` — so no offset in the
file moves and neither test is secretly about rewriting a font.

**The survey found the rule this item did not know it needed.** A throwaway test
read every font in `/System/Library/Fonts`, `/System/Library/Fonts/Supplemental`
and `/Library/Fonts` and printed what `style_in` made of each: **29 of 370
readable fonts declared a weight axis**, the system font among them. One of them
was wrong. **`Skia.ttf` runs from 1 to 3** — an Apple axis from before `wght`
had a shared meaning, with `OS/2` stating 5 — and read as CSS numbers the whole
axis is hairline, so every request would land on its heaviest end and a page of
ordinary text would come out black. An axis ending below the lightest weight CSS
has a *word* for is left alone, which is the same refusal item 194 made one
table earlier for the same reason. Re-run afterwards: **28 of 370. One line
moved and 28 did not.**

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1630 tests** (up from 1615), no stubs, no `unsafe`, boundaries held,
the licence notice, and a `CHANGELOG.md` line. The half no script can check: the
**layout assertion in numbers** is
`two_weights_of_one_variable_family_are_two_widths_of_text` — four weights, four
widths, strictly ordered, with the two ends asserted to the pixel because at the
end of an axis the delta is the whole of what the font states and nothing is
interpolated. The arithmetic is exact rather than nearly so: every advance is an
integer number of font units and the scale is 16/2048, a power of two. **A
reference render** would have been the answer for the corpus and the corpus did
not move — every case there declares its own static faces, so none of them goes
through this path — so the visual assertion is the one in `alo-paint`, in the
shape of the one `a_letter.rs` already uses: a glyph asserted as a shape rather
than as a committed picture. One file one responsibility: the reading stayed in
`font.rs`, which is the file about what a font says about itself and the only
file in the crate that may name `ttf_parser`; a new file for it would have
widened a rented crate's boundary to say one sentence. The item is in
`docs/features.md`, as its own `[2]` line.

**All three directions were doctored, and each failed the tests written for
it.** With `chain` handing back the fonts it holds rather than fonts set to the
weight, 4 of 13 fail and the widths collapse to one number. With the axis never
read, 8 of 13 fail. With the distance measured to what a face *is* rather than
to what it can be, **exactly one** fails — the case written for that rule and no
others, which is the run worth having. And with the outline half's three lines
removed, `alo-paint`'s case reports that the letter should have moved 14.65
pixels and moved 0.

**Hostile input.** Both tables are somebody else's bytes. Every truncation of an
`fvar` and every single flipped bit of one is an answer rather than a crash, and
each surviving font is then *shaped* rather than merely parsed, because a table
that parses and then divides by zero is still a tab that disappears. An `HVAR`
claiming fewer rows than the font has glyphs is a line with a finite width. One
guard was **removed** rather than added: `fvar` writes an axis bound as 16.16
fixed point, which is four bytes read as an integer and divided, so no file can
hold a NaN there — a check against one would have been a branch no font could
reach and no test could reach either.

**`ROADMAP.md`.** The process-and-sandbox line again, whose `· Built:` clause
gains item 196 beside 168, 170, 192, 193, 194 and 195. **It is still not
ticked**; its `· Owed:` clause drops 196 and now reads the Linux sandbox (169)
and item 197.

**What the next iteration should know.** One cut, written into the queue as item
197: the axes that are **not** weight — `wdth`, `slnt`, `ital`, `opsz`. Each is
a separate CSS property with a grammar of its own, and guessing at one would
draw a page narrower or slanted because this engine assumed an axis nobody had
looked at. The machinery is in place and 197 says what shape it takes. Two
things were noticed and are not cuts, because neither is wrong: `fonts::from_file`
still skips `.ttc` collections, so four of this machine's variable fonts are not
reachable by the browser process at all — that is item-worthy the day a page
fails on one; and CSS's own `font-variation-settings` and `font-optical-sizing`
do not exist here, which is item 197's dependency rather than a defect.

The ready items in stage 2's file order are now **64** and **65** (the renderer
lifecycle, both depending on 63 which is done — and much of both may already
exist in `host.rs`, so they should be read before they are built), **66** (where
one site ends and another begins, much of which `alo_url::site` answers since
item 156), **190** (the two-tone border styles: small, depends on nothing, and
closes with a picture), and **197** above, which is blocked in practice until
`alo-style` has the properties it implements.

---

## Iteration 93 — queue item 65: a tab that keeps its picture and says what happened

**The tree was clean on entry and `scripts/gate.sh` was green.** The previous
entry named 64 and 65 as the ready items in file order and said to read
`host.rs` before building either, because much of both might already be there.
That was the right instruction and reading it is what decided the iteration.

**Item 64 is nearly built and item 65 was not built at all**, and `ROADMAP.md`
said the opposite of both. Two lines under the process model were **ticked** —
*"a renderer that dies takes its tab and nothing else"* and *"the transport, and
the lifecycle that starts, reuses and reaps renderers"* — while their queue
items sat open. Ticked beside item 166, which is the honest cause: that item
built one process per site and a test that kills one and watches the other keep
working, and from a renderer's point of view the line is met. From a **tab's**
point of view none of it was, because there was nothing in this repository that
was a tab. `Renderers::ask` returned a `Gone` with a sentence in it to whoever
called; nothing kept a painted frame anywhere; so what a person would have been
shown when a renderer died is the **blank rectangle the line names**.

So this iteration built item 65 — `crates/alo-renderer/src/tab.rs` — and
corrected both lines. The first stays ticked and now says what actually met it
and when. The second is **un-ticked** into the `· Built: … · Owed: …` state that
file defines, because "reaps" is the one word of item 64 that nothing does: a
renderer whose last tab closed runs until the ceiling evicts it. Un-ticking is
not the thing `LOOP.md` forbids — it forbids ticking to discharge an obligation,
and the file's own preamble says a tick means done.

**What a tab is.** Its id, its site, the last frame it painted, and what it was
last told about its renderer. `Tabs` owns the `Renderers` rather than borrowing
them, which is the whole design in one line: every `Gone` passes through the one
door, so a tab that was not told its renderer died cannot exist.

**The rule worth reading twice is that nothing here restarts anything.**
`Renderers::ask` starts a process for a site that has none — so a repaint of a
dead tab would have spawned a fresh one, found it holding no page, and answered
that nothing was loaded. The tab would then be blank, the reason would be wrong,
and the bug that killed the first process would have vanished. That is ADR
0005's silent restart arriving by a road nobody had walked down. A tab that has
been told answers from what it knows; only a deliberate `load` starts anything,
and the test counts processes rather than reasoning about it. The same check
catches a renderer **evicted** to stay under `MOST_RENDERERS`, which goes away
without anybody dying and which nothing was going to tell a tab about.

**The deciding is a pure function** — `may_ask`, over an `Asking` value — which
is the shape items 55, 154 and 188 already use, for a version of the same
reason: every rule in it is a rule about **not starting a process**, and a rule
about not doing something is asserted honestly only when nothing is moving. Six
of the thirteen unit tests are of that function alone, and they reach
arrangements a test with real processes would take a minute to set up.

**One thing was found while building it, and it is refused rather than answered
wrongly.** Two tabs on one site share a process (ADR 0005) and a `Renderer`
holds **one** page, so the second tab to load displaces the first inside that
process — and a repaint of the displaced tab would have come back with somebody
else's page, which is a wrong picture that looks like a right one. `Lost::
HoldsAnotherPage` names the tab whose page the renderer is holding.
`docs/features.md`'s *"several documents at once, the shape tabs need"* is what
ends that, and it is not this item.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1646 tests** (up from 1630), no stubs, no `unsafe`, boundaries held,
the licence notice, and a `CHANGELOG.md` line. The half no script can check:
**nothing here positions or sizes anything**, so there is no layout assertion to
make and saying so is the honest answer — a tab holds a frame the renderer
already laid out, and the frame's size and pixel count are asserted by the wire
format. **The visual assertion is the one this item is actually about**: the
frame a tab keeps after its renderer is killed is compared **byte for byte**
with the one that came back from the paint, and the two pages in the test are
made two different pictures on purpose (the assertion that they differ comes
first, so a change that made every page render identically could not let this
pass). A committed reference render would have been the wrong tool: nothing new
is drawn, and what is being asserted is that a picture *survives a process*.
One file one responsibility: `host.rs` is still the renderers a browser process
holds, `tab.rs` is what a person opened and what became of it. The item is in
`docs/features.md`, as its own `[2]` line.

**Four directions were doctored, and each failed the tests written for it.**
With the frame not kept, two of the three process tests fail and both say the
picture is gone. With a death marking no tabs, all three fail and three unit
tests with them. With the gone check removed from `may_ask`, the process test
that counts renderers fails — and notably the *unit* test of the same rule still
passed, because the stand-in program exits and produces the same sentence by
accident, which is why the counting test is the one that matters. With the
displacement rule removed, exactly one process test and one unit test fail.

**Hostile input.** Nothing new reads bytes from outside: a frame arrives through
`wire.rs`, which already treats a renderer as the process that parsed the page
and bounds what it will decode. What this file adds is bookkeeping over values
that have already been through that door.

**What the next iteration should know.** Item 64 is now a small, well-defined
item and its closing condition is written into the queue: closing the last tab
on a site stops that site's process, closing one of two stops nothing. It was
deliberately **not** taken here — one item per iteration, and reaping is the
renderer lifecycle rather than what a tab is. The other ready items in stage 2's
file order are unchanged: **66** (where one site ends and another begins, much
of which `alo_url::site` answers since item 156) and **190** (the two-tone
border styles: small, depends on nothing, closes with a picture). 157, 158 and
187 are still deferred for reasons written into them, 169 must be run on Linux,
60 is HTTP/3, and 197 waits on properties `alo-style` does not have.

---

## Iteration 94 — queue item 64: a renderer nothing wants any more is stopped

**The tree was clean on entry and `scripts/gate.sh` was green.** The previous
entry left item 64 as a small, well-defined item with its closing condition
written into the queue, and that is what this iteration took: the lifecycle
starts renderers, reuses them and bounds how many exist, and **nothing reaped
one**. A renderer whose last tab had closed ran until the ceiling happened to
evict it — which is what happens when reaping has not happened rather than a
way of doing it, and which the previous iteration had already written into
`ROADMAP.md` when it un-ticked that line.

**The division of labour is the thing worth reading, not the stopping.**
`tab.rs`'s `close` carried a comment saying reaping was not its to do, because
deciding that a process ends on the strength of happening to hold the last
reference to it is how a lifecycle ends up scattered across the files that call
it. That comment was right, so the shape it asks for is what was built: the
caller says what it still **wants** — `Tabs::sites_open`, the sites that have a
tab open on them — and `Renderers::reap` decides what that costs a process. The
argument goes in that direction rather than the other because the mistakes are
not symmetric: a site left out of `wanted` costs a process that starts again,
and a site left in by mistake would be a renderer nothing can ever reach.

**The test asks the operating system rather than this program.** `kill -0` on
the process id, which is a real answer only because `stop` waits: an unwaited
process is a zombie and a zombie answers `kill -0` like anything living. A
bookkeeping entry that disappeared while the process kept running is exactly the
bug a test of the map would not have found.

**Two things went in beside it.** The tab whose page a reaped renderer was
holding is forgotten, because a `held` entry outliving its process refuses the
next tab on that site (`Lost::HoldsAnotherPage`) on behalf of a renderer nobody
can reach — and names a tab that has by then been closed, so the refusal would
be unanswerable as well as wrong. And reaping **only ever ends things**: a
wanted site with no renderer does not get one out of it, which is the same rule
as everywhere else here, that nothing starts a process except somebody asking
for a page.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1654 tests** (up from 1646), no stubs, no `unsafe`, boundaries held,
the licence notice, and a `CHANGELOG.md` line. The half no script can check:
**nothing here positions or sizes anything and nothing here draws**, so there is
no layout assertion and no reference render to make, and saying so is the honest
answer — this item is about a process existing or not existing, which is why the
assertions are `kill -0` rather than pixels. One file one responsibility:
`host.rs` gained the verb its own lifecycle was missing and `tab.rs` gained one
private function saying which sites are still open, which is the split the
comment in `close` had already argued for. The item is in `docs/features.md` as
its own `[2]` line.

**Three directions were doctored, and each failed the tests written for it.**
With the reaping taken out of `close`, two of the six fail and one says the last
tab on a site closed with its process still running. With the `held` entry left
behind, exactly one fails and it names the refusal it got. With `reap`'s filter
inverted — stopping what is wanted — four fail, including the two that call
`reap` directly with no tabs anywhere near it.

**Hostile input.** Nothing new reads bytes from outside. What was added is a set
difference over sites this process already holds, and a process id this process
already spawned.

**`ROADMAP.md` moved, and it is a tick this time.** *"The transport, and the
lifecycle that starts, reuses and reaps renderers"* names three verbs and all
three now exist, so the line is ticked with the history left beside it: it was
ticked once before on a reading of "reaps" that the eviction satisfied, and
un-ticked last iteration for that reason. Ticking it now is what a tick means.

**One thing was found and is queue item 198 rather than folded in.** `pipe::read`
blocks until bytes arrive, so a renderer that is **alive and never answers**
hangs the browser process — the one thing ADR 0005 says must never happen.
Killing a hung renderer is a lifecycle act and the lifecycle has no clock. It is
deliberately not this item: it is not one of the three verbs the roadmap line
names, its closing condition needs a bound a test can state, and the half that
decides what that bound may be is that a merely **slow** renderer must not be
killed. It is written into `ROADMAP.md` beside the ticked line as well, so it is
not a gap living in one commit message.

**What the next iteration should know.** The ready items in stage 2's file order
are now **66** (where one site ends and another begins — much of which
`alo_url::site` has answered since item 156, so read that before building
anything), **190** (the two-tone border styles: small, depends on nothing,
closes with a picture) and **198** above, which depends on 64 and is now
unblocked. 157 and 158 need an interface to ask in, 187 is deferred for the
reason written into it, 169 must be run on Linux, 60 is HTTP/3, and 197 waits on
properties `alo-style` does not have.

---

## Iteration 95 — queue item 198: a renderer that stops answering is given up on

**The tree was clean on entry and `scripts/gate.sh` was green.** The previous
iteration cut this item out of 64 and left it as the first ready item in file
order: `pipe::read` blocks until bytes arrive, so a renderer that is **alive and
never answers** — wedged on a page, or on something a hostile page arranged —
held the browser process in a read for as long as it lived. Every other tab, and
everything a person could click, waited with it. That is the one thing ADR 0005
says must never happen, arriving by the one road nobody had walked down: a
renderer that *dies* closes its pipe, and a read that ends is an answer.

**The clock needed a thread, and that is the whole of `answers.rs`.** A pipe read
cannot be given a deadline in safe Rust — the platform calls that would do it are
FFI, and ADR 0010 refused FFI for the sandbox itself on exactly that ground — so
the read happens on a thread of its own and the browser process waits on a
channel, which does take a bound. Two rules went in with it because the shape
would otherwise be a worse bug than the one it fixes:

- **The channel holds one message.** A thread reading ahead as fast as a renderer
  can write is a renderer that fills the *browser* process's memory by talking,
  and the blocking read had that backpressure for free. `sync_channel(1)` keeps
  it: the reader stops with one message in hand and everything after it stays in
  the pipe, where the operating system already bounds it.
- **A bound without a kill would be worse than no bound.** The protocol is one
  answer per request, so an answer arriving after we stopped waiting for it would
  be handed back as the answer to the *next* question — a picture of the wrong
  page, or a tree an agent then acts on. So a silence is fatal to the renderer
  rather than something to retry, and `Renderers::ask` sends it through `lost`,
  which is the same door a death goes through.

**The thread is detached and nothing ever joins it**, deliberately: a join is an
unbounded wait on a renderer, which is the exact bug this file is for, and it
would be taken at the worst possible moment — while stopping a renderer that has
already proved it does not answer. It ends on its own when the pipe closes, which
`stop`'s kill and wait guarantee.

**Ten seconds, and the constant says it is a choice rather than a measurement.**
`LOOP.md` says a claim about speed is measured on hardware or not made, so this
is not one: it is the point past which waiting is worse than losing the page, and
both directions cost something real — too short kills a renderer that was about
to answer, too long is a browser somebody force-quits. The honest version is not
a number at all but a question, *wait, or stop it?*, and asking it needs an
interface, which is what blocks items 157 and 158. The bound is a **field** on
`Renderers` rather than the constant used in place, because a test that waited
ten seconds to find out what happens after ten seconds is a test nobody runs.

**The wedged renderer in the test is the real binary stopped with `kill -STOP`.**
That is the condition itself rather than a stand-in that shares only the silence:
alive, `kill -0` finds it, its pipe is open, and it will never answer. The same
signal makes the other half exact — a renderer stopped and then continued with
`-CONT` is slow by precisely as long as the test says, which nothing about a real
page could promise, and *"a renderer that is merely slow is not killed"* is the
clause that decides whether the bound may exist at all.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero errors,
**1665 tests** (up from 1654 — eleven added, seven of `answers.rs` on readers a
test controls the timing of, four on real processes), no stubs, no `unsafe`,
boundaries held, the licence notice, and a `CHANGELOG.md` line. The half no
script can check: **nothing here positions or sizes anything and nothing here
draws**, so there is no layout assertion and no reference render to make, and
saying so is the honest answer — this item is about how long a process waits, so
the assertions are a clock, `kill -0` and the sentence a tab gives a person. One
file one responsibility: `pipe.rs` still says where a message *ends* and
`answers.rs` says how long we wait for one, which is why the thread and the clock
did not go into `pipe.rs`; `host.rs` gained a field and lost a `BufReader`. The
item is in `docs/features.md` as its own `[2]` line.

**Four directions were doctored.** With the timeout reported as a clean ending
rather than a silence, two tests fail and both quote a tab being told "it exited"
about a renderer that is alive. With the giving-up not stopping the process,
three fail — including the one that finds the wedged renderer still running, and
the one where a tab is refused a reload on a dead renderer's behalf. With
`waiting_at_most` ignored so the ten-second default applies, the wedged test
fails on the clock at 10.15s, which is the assertion that the *named* bound is
what fired. And with no bound at all — the tree as it was before this change —
the test **does not return**: it was still running after 45 seconds, which is the
bug itself and is written into the test file's own preamble, because an iteration
that breaks this would otherwise spend itself wondering why the suite stopped.

**Hostile input.** Nothing new reads bytes from outside: the bytes still go
through `pipe::read`, which already treats a renderer as the process that parsed
the page and refuses a length before allocating for it. What is new is the
*waiting*, and the hostile version of waiting is exactly what this bounds. The
unit tests feed the reader a length no message may have and assert it is refused
and ends the reading, since a stream that has lost its place in the message
boundaries has nothing further worth reading.

**`ROADMAP.md` moved, and it is not a tick.** The line *"the transport, and the
lifecycle that starts, reuses and reaps renderers"* was ticked by item 64 and
stays ticked — its three verbs are done. What it carried was a note saying this
was found and deliberately not folded in; that note now says what was built
instead, which is the `· Built:` half of the clause that file defines. Nothing
was re-ticked to discharge an obligation.

**What the next iteration should know.** The ready items in stage 2's file order
are now **66** (where one site ends and another begins — much of which
`alo_url::site` has answered since item 156, so read that before building
anything) and **190** (the two-tone border styles: small, depends on nothing,
closes with a picture). 157 and 158 need an interface to ask in — and this
iteration added a third thing waiting on that interface, since *"wait, or stop
it?"* is the honest form of the bound built here. 187 is deferred for the reason
written into it, 169 must be run on Linux, 60 is HTTP/3, and 197 waits on
properties `alo-style` does not have.

---

## Iteration 96 — queue item 66: which of the origin, the site and the registrable domain gets a process

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 66 was the
first ready item in stage 2's file order, and it arrived without a *closes when*
— which stage 2's rules say makes an item unready. It was taken rather than
marked `needs design` because `ROADMAP.md` had already written the owed half
precisely: *"which of the origin, the site and the registrable domain a page is
given, case by case"*. The closing condition is written into the queue item now,
before the code: every URL a page could hold is given one of the three by a rule
written down, and two documents whose origins are **opaque** are never in one
renderer process, in a test with real processes in it.

**Reading the three answers side by side is what found the defect.** `alo-url`
has three of them — `Origin::of`, `site::of`, and the `Url`'s own host — and
`alo-renderer`'s `Site::of` was consulting the last two and never the first. For
an ordinary address that is right and has been since item 156. For a URL with no
host it gave **the scheme and nothing else**, so every `data:` page in the
browser was one site, every `about:` page was one site, and every local file on
the machine was one site sharing one address space. `alo-url` has said since item
50 that each of those is its own origin, and says in its own words that *"one
local file being able to read every other one is the oldest exfiltration bug
there is"*. The process split was quietly undoing that: two documents that may
read nothing of one another's, in one renderer.

**The rule is one sentence — the origin decides whether there is a site at all.**
Where it is a tuple, the registrable domain widens it into a site and two tabs
share a process; the **port is left to the origin**, because two ports are two
origins that can already reach one another with a link and a cookie, so a process
each would cost memory and buy nothing. Where it is opaque there is no site, and
the document is `Site::Alone` carrying that opaque origin's own identity: a
process nothing else is ever put into, not another `data:` page with the same
bytes, and not the same file opened a second time.

**The answer is taken from `Origin::of` rather than restated**, which is the
whole reason the change is small. Two functions deciding what is opaque are two
functions that can come to disagree, and the disagreement would be a process
holding two documents the security rules call strangers. It also settled the
cases nobody had asked about without a line of code each: a scheme with no
default port and no port written is opaque there, so unknown still never means
"probably fine" here either.

**Two things went in beside it because the shape invites the opposite.** The
cost is written down rather than left to be discovered — twenty local files open
is twenty renderers, up to `MOST_RENDERERS`, past which the least recently used
is evicted, and ADR 0005 already priced that memory. And **a site is decided
once**, when the tab is opened: `Site::of` on an opaque origin mints a new
identity every call, so a caller that decided again per request would hand one
tab a new process every time it painted. `tab.rs` asserts both halves of that —
the site a tab keeps does not move, and deciding it again would not have given
the same answer.

**`Site::host()` returns an `Option` now**, which is the part of the type worth
reading. A document with no site has an identity rather than a name, and a caller
handed an empty string would read it as a host that every other hostless document
shares — which is exactly the belief this change exists to make impossible.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero errors,
**1678 tests** (up from 1665 — thirteen added: ten on the rule itself, two on a
tab keeping its site, one on real processes), no stubs, no `unsafe`, boundaries
held, the licence notice, and a `CHANGELOG.md` line. The half no script can
check: **nothing here positions, sizes or draws anything**, so there is no layout
assertion and no reference render to make, and saying so is the honest answer —
this item decides which process a document is given, so the assertion is three
real renderers with three different process ids. One file one responsibility:
`site.rs` still answers only *which process renders this document*, and it now
asks `alo-url` the question instead of half-answering it. The item is in
`docs/features.md`, with a second line for what is still owed.

**Eight tests were doctored out in three files.** With `Site::of` reading a
hostless URL as the scheme again — the tree exactly as it was before this change
— five of `site.rs`'s ten fail, both of `tab.rs`'s new ones fail, and the real-
process test fails with two `data:` documents in one renderer. The five that
still pass are the ones about ordinary addresses, which is the evidence the
change moved what it meant to move and nothing else.

**Hostile input.** A URL comes off a stranger's page, and
`every_url_a_page_could_hold_is_answered_rather_than_crashed_on` walks fourteen
shapes of one — punycode, an address with a port at the top of the range, a
trailing dot, a bare public suffix, `javascript:`, `mailto:`, an empty `data:` —
and asserts each is either a site or a process of its own. There is no third
outcome, which is the property that matters: no URL falls through to sharing a
process by accident.

**`ROADMAP.md` moved, and it is not a tick.** The line *"where one site ends and
another begins"* keeps its box empty and gains a `· Built:` clause for what item
66 decided, and its `· Owed:` clause was rewritten from "queue item 66" to the
thing that is genuinely left: what a **document inside a document** is given — a
sandboxed `iframe`'s opaque origin and `about:srcdoc` inheriting its parent's
(item 86), and a `blob:` taking the origin of whoever created it (items 72
and 90). None of those can exist yet, because nothing here can produce a document
inside a document, so ticking would have been ticking a line for the documents
that happen to be reachable today.

**What the next iteration should know.** Stage 2's section B is now finished
except **item 67**, which is the next item in file order and is a **decision
rather than a chore**: *every request attributable — which page, and which agent
action, caused it*, marked `needs ADR` in the shape of `alo-os` ADR 0001. Stage
2's rules say a decision gets the ADR as its own iteration, so that is what
taking 67 means. **Item 69 is the same shape** and is the first item of section D
— our own JavaScript engine — and both queue entries name ADR numbers that are
stale: the queue says "ADR 0006" for item 69 and 0006 has been *the supervisor
lives here* since it was written. **The next free number is 0012.** Beyond those,
**190** (the two-tone border styles: small, depends on nothing, closes with a
picture) is ready. 157 and 158 need an interface to ask in, and so does the
question item 198 stands in for; 187 is deferred for the reason written into it;
169 must be run on Linux; 60 is HTTP/3; and 197 waits on properties `alo-style`
does not have.

## Iteration 97 — queue item 67: ADR 0012, every request says what caused it

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 67 is the
first ready item in stage 2's file order — its one dependency (53) is done, and
the items above it in section B are finished — and it is marked *needs ADR*. So
this iteration is the decision and nothing else, which is `LOOP.md`'s stage 2
rule 4: *"a decision made inside a commit that was mostly code is a decision
nobody reviewed."* No code was written, deliberately. The number is **0012**,
which iteration 75 had already worked out was the next free one; the queue entry
named no number, so nothing had to be corrected. (Item 69 still names "ADR 0006"
wrongly, and that is the iteration that takes 69 to fix.)

**What the decision had to answer**, in the queue's own words: what is recorded,
for how long, and who may read it, in the shape of `alo-os` ADR 0001. That
repository is not checked out here, which `LOOP.md` says must never block an
item — the shape is taken from **ADR 0002**, which records it in this repository
as *enumerated, visible, revocable, expiring, recorded*.

**Writing it out found that the question pulls in two directions**, and that is
the reason the ADR is not short. Attribution is a **claim**, so if the process
that parsed a hostile page can make it, the record is a sentence somebody else
composed — and a forgeable record is worse than none, because people believe it.
But a record of every request is *a record of everywhere somebody has been*,
which is exactly what ADR 0011 spent five sections being careful about. So the
interesting half is not what to record. It is what **not to keep**, and who may
never read what is kept.

**ADR 0012, in the clauses the code now has to carry.**

- **A cause is carried, with no default** — a request that cannot say what
  caused it does not compile. The same structural shape as ADR 0002's *no verb
  takes a coordinate*, and for the same reason: the call site added in a hurry
  is exactly the one that would have omitted it.
- **Three causes and no fourth**: a person, a document, an agent action. No
  `Unknown` and no `Internal`, because a category like that does not stay empty:
  it becomes where the awkward cases go. An engine-made request is attributed
  to whatever caused the thing it is about, the way `Purpose::Report` already
  belongs to the load that violated the policy.
- **It is a chain rather than a label**, and each document records what caused
  its own load. *Which page* and *which agent action* are two questions with two
  true answers, and the walk cannot lie about which document it reached because
  ADR 0003's ids are allocated once and never reused.
- **The browser process assigns it; a renderer never does.** A renderer states a
  `Purpose` — it is the only thing that knows a script from a picture — and
  never a cause. ADR 0005 makes it the process that parsed a stranger's page, so
  a cause it could state is a cause it could forge into *the person did that*.
- **Everything for the session, in memory, bounded**; and **only what reaches an
  agent action is kept until the person deletes it**, under ADR 0011 section 3's
  rules unchanged, never opened at all for a session-scoped profile, and bounded
  in **actions rather than bytes** so one busy action cannot evict a week of
  ordinary ones.
- **No page and no agent may read it.** No API, ever: a record readable by
  script is a cross-site history oracle handed out by the browser, which undoes
  ADR 0007's partitioning and ADR 0011's per-site key in one move. Not the agent
  either — the record is *about* the agent and kept *for* the person, and one
  that could read it could read everywhere that person has been and check
  whether its own actions had been noticed.

**The edge case is named in the ADR rather than left to be discovered.** While a
verb is being applied, that tab's requests are the agent's — and a page that
fetches on a timer minutes later is **not**. Widening the window until every
consequence is captured makes the record true and useless. The precise boundary
is the task, which the event loop defines (item 76); until scripts exist a
verb's consequences are immediate, so the rule is writable now.

**What it costs, which is in the ADR rather than left out.** Memory per request
for the session. A durable file naming sites an agent visited, with ADR 0011's
honest boundary restated rather than quietly dropped — protected against another
user account, not against a program running as the person. And friction on
purpose: every place that makes a request must name a cause, and no default
rescues anybody from thinking about it.

**Two things it explicitly does not decide**, because a record is not a
permission: **grants** are items 93 and 133 and owe their own ADRs, and an
action being recorded must never become the argument that it was authorised.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1678 tests** unchanged, no stubs, no `unsafe`, boundaries held, the
licence notice, and a `CHANGELOG.md` line. No layout assertion, no reference
render and no new test — this iteration adds no behaviour to test, which is what
an ADR-only iteration is. One file one responsibility: the only source file
touched is `request.rs`, and only its module comment, which now names the four
clauses of ADR 0012 that land in that file — the decision where the code goes,
the way `cache.rs` carried ADR 0011's before item 155 was built.

**`ROADMAP.md` moved, and it is not a tick.** The line *"★ Every request
attributable"* keeps its empty box and gains a `· Built:` clause for the
decision and an `· Owed:` clause that says plainly that **all** of the code
remains — nothing today carries a cause. `docs/features.md` gains the same
distinction on its planned line, so the decision is promised before it is built.

**What the next iteration should know.** Item 67's code is unblocked and is the
natural next take: its clauses are written down, `alo-net/src/request.rs` is
where the field goes and says so, and the closing condition is now in the queue
— every request names its cause with no way to make one that does not, an
action is reachable from every request that followed from it, and a renderer
that states a cause is ignored. It is **larger than one iteration**: the field
and the three causes are one thing, the chain another, and the durable half a
third. Cut it on starting, into the queue, rather than half-building it.

**Item 69 is the other decision-shaped item** and is the first of section D —
our own JavaScript engine — and the queue still calls it "ADR 0006", which is
the supervisor. The next free number is now **0013**. Beyond those, **190** (the
two-tone border styles: small, depends on nothing, closes with a picture) is
ready. 157 and 158 need an interface to ask in, and so does the question item
198 stands in for; 187 is deferred for the reason written into it; 169 must be
run on Linux; 60 is HTTP/3; and 197 waits on properties `alo-style` does not
have.

## Iteration 98 — queue item 67: every request says what caused it, and cannot not

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 67's ADR
landed last iteration; its code was the natural next take and the journal
already said so. It also said the item is **larger than one iteration** and
named the cut: the field and the three causes, the chain, the durable record.
That is what happened — this iteration is the first of the three, and 199 and
200 are the other two, written into the queue on starting rather than left as a
half-built item.

**What is built.** `crates/alo-net/src/cause.rs`: `Cause` with three variants
and no fourth, `TabId` / `DocumentId` / `ActionId`, and `Identities`, which is
the only thing that can mint one. `Request` carries a `Cause`, and
`Request::get` and `Request::sending` take it as an **argument** — no builder,
no `Default`, no `..Default::default()` anywhere near it. So ADR 0012 § 1's
guarantee is a signature rather than a habit, and the thing that checks it is a
`compile_fail` doctest on `Request::get` paired with a passing one, so a rename
breaks the pair rather than silently turning the negative into a false pass.

**The identities went in `alo-net`, and `alo_renderer::tab::TabId` is now a
re-export of `alo_net::cause::TabId` rather than a type of its own.** That was
the one real design decision in the iteration and it is written into both files.
A cause is a *field on a request*; a field cannot name a type from a crate that
depends on this one; so either the causes carry a second tab identity mapped
onto the renderer's, or there is one identity and it lives here. Two identity
spaces for one tab is precisely what ADR 0003 exists to refuse — an id meaning
one tab in the record and another in the browser joins two unrelated pieces of
somebody's history into one story. It also made ADR 0012 § 4 structural for
free: a renderer holds no `Identities`, so it has nothing to state a cause
*with*.

**The four requests nobody asked for each clone the cause of the thing they are
about**, which is what let the decision have no `Unknown` in it: a redirect hop
(`redirect::next`), a resumed range request (`Download::asking`), a CORS
preflight (`cors::asking_first`) and a violation report (`csp_report`, which
needed the cause carried as far as its `Page` — the load that violated the
policy is the thing a report is about). `tests/what_caused_a_request.rs` sweeps
all four in one test as well as asserting each, because a **fifth** appearing
with a fresh cause of its own is the drift worth catching and it would not show
up in any one file. One test is named for the attack rather than for the field,
the way items 61 and 62's are: a server answering `302` cannot promote a page's
fetch into something the person did.

**The third closing clause is not met and is not claimed.** *A renderer that
states a cause is a renderer that has been ignored* has nothing to ignore today:
no message crossing the boundary carries a request at all, so `ToRenderer` and
`FromRenderer` could not express a cause if a renderer tried. Asserting it now
would be a test of nothing. It is written into item 199 with the dependency
named — items 80 and 83, where a renderer can ask for a subresource — rather
than left implied by a tick.

**The cost, which the ADR asked for on purpose.** 112 call sites gained an
argument. Every test file that makes a request now says what its requests
*mean* — `a_person()` where somebody is opening a page, `a_page()` where a
document is fetching a subresource — which is friction, and is the friction ADR
0012 § 8 named. It is also the first time those files say which of the two they
were about.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1694 tests** (1678 before, so 16 new — 8 in the new integration file,
5 unit tests in `cause.rs`, 2 rewritten and added in `request.rs`, 1
`compile_fail` doctest), no stubs, no `unsafe`, boundaries held, the licence
notice, and a `CHANGELOG.md` line. No layout assertion and no reference render:
this iteration positions nothing and paints nothing. One file one
responsibility: `cause.rs` is new and holds one thing — what caused a request,
and the identities a cause names, which exist only to be named by one.
`docs/features.md`'s starred line gains what is built and what is still to come.

**`ROADMAP.md` moved, and it is not a tick.** The ★ line keeps its empty box.
Its `· Built:` clause gains the cause itself and the four engine-made requests;
its `· Owed:` clause was *all of the code* and is now three named things — the
chain (199), the record (200), and the browser process assigning it in earnest,
which needs a renderer that can ask for a subresource. While editing it I
renumbered two of its existing references by accident and put them back: the
Macintosh-encoding and generic-family clauses are queue items **192 and 193**,
which is why the new cuts are **199 and 200** rather than the next two numbers
after 191. The next free number is **201**.

**What the next iteration should know.** Item 199 is the chain and is the
natural next take, but it is not small: it needs a **document to be a thing with
an identity** outside a `Cause`. `alo_renderer::Tabs` mints tabs and mints no
documents, and `Page` is markup and a viewport — so the work starts by deciding
where a document's identity is allocated and where it records what caused its
own load, and `Tabs` already holds the `Identities` that would mint it. Item 200
depends on 199 for the reason written into it: a record of chains needs chains,
and building it first would keep only requests whose own cause happened to be an
agent action, which is the narrowest reading of the ★ promise rather than the
one the ADR makes.

**Item 69 is the other decision-shaped item** and is the first of section D —
our own JavaScript engine — and the queue still calls it "ADR 0006", which is
the supervisor; the next free ADR number is **0013**. Beyond those, **190** (the
two-tone border styles: small, depends on nothing, closes with a picture) is
ready. 157 and 158 need an interface to ask in, and so does the question item
198 stands in for; 187 is deferred for the reason written into it; 169 must be
run on Linux; 60 is HTTP/3; and 197 waits on properties `alo-style` does not
have.

## Iteration 99 — queue item 199: a cause is a link in a chain

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 199 was
the natural next take and the previous journal said so, with the hard part
already named: a **document** had to become a thing with an identity outside a
`Cause`, because `Tabs` minted tabs and minted no documents and `Page` is markup
and a viewport.

**Where a document comes from turned out to be the whole design.** Loading a
page is what makes one, so `Tabs::load` now takes the [`Cause`] its own request
carried, mints the document and records the pair in **one act** —
`Documents::opened` mints and writes together, so there is no moment at which a
document exists without a cause and no second call that could give it a
different one. That is ADR 0012 § 3 made structural rather than a rule
somebody follows, and it is the same shape as § 1's *no constructor without a
cause* one layer up.

**What is built.** `crates/alo-net/src/chain.rs`: `Documents` (what caused each
document's load), `Documents::chain` (the walk), `Chain` and `End`.
`alo_renderer::Tabs` is the one thing that writes to it — `load` takes a cause,
`act` mints an `ActionId` and hands it back, and `a_page_fetching` and
`an_agent_acting` compose the causes for what follows. A `Tab` holds the
document it is showing.

**Both reachable clauses are met and the third is said rather than asserted.**
An agent's action is reachable from every request that followed from it, in
`crates/alo-renderer/tests/what_an_agent_set_off.rs`, which drives the **real**
renderer binary: a page loads, the agent activates a real link, the page the
link named loads attributed to the action, and a fetch by *that* page walks back
through it to the person. The walk terminates on a cycle rather than looping.
The clause item 67 could not reach — *a renderer that states a cause is a
renderer that has been ignored* — still has nothing to ignore: no message
crossing the boundary carries a request, so a test of it would be a test of
nothing. It is **item 201** now, with items 80 and 83 named as its dependency,
rather than implied by a tick.

**Three rules are worth reading twice.** The document a cause names is taken
from the **tab** rather than from a caller or from anything a renderer said, so
an agent acting in one tab cannot reach into another's browsing — that has a
test of its own, named for what it refuses. The walk carries the documents it
has been through and stops if one comes back: a cycle cannot be *created*, and a
walk that trusted that would hang the **browser** process, which is the one
thing ADR 0005 says must never happen; its test reaches past the constructor to
build a cycle by hand, because what is asserted is that the walk survives a
state nothing can put it in. And the bound (`MOST_DOCUMENTS`) came with the
honesty it owes: a chain reaching a document dropped under the ceiling says
`Forgotten`, and one reaching a document nothing ever recorded says
`Unrecorded` — *we knew and no longer do* and *nobody ever said* are different
answers, and running them together would be guessing in the one place that
exists not to.

**The negative test is the one that makes the positive worth anything.** A page
a person opened themselves reaches **no action at all**. An engine whose chains
found an action everywhere would answer the question the record exists for with
the same word every time.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1713 tests** (1694 before, so 19 new — 10 in `chain.rs`, 5 in
`tab.rs`, 4 in the new integration file), no stubs, no `unsafe`, boundaries
held, the licence notice, and a `CHANGELOG.md` line. No layout assertion and no
reference render: this iteration positions nothing and paints nothing. One file
one responsibility: `chain.rs` is new and holds one thing — what caused each
document's load, and the walk along it; `cause.rs` still holds one cause and the
identities, and did not change. `docs/features.md`'s starred line gains the
chain and what it refuses.

**`ROADMAP.md` moved, and it is not a tick.** The ★ *every request attributable*
line keeps its empty box. Its `· Built:` clause gains the chain and the three
refusals; its `· Owed:` clause loses the chain and now names two things — the
record itself (item 200), and a cause for a **subresource**, which needs a
renderer that can ask for one at all.

**What the next iteration should know.** Item 200 is the record and is the
natural next take: its dependency is discharged, `Documents` is the shape its
durable half will be keyed against, and ADR 0012 §§ 5, 6 and 7 are its whole
specification. It is not small — the session record, the bound in **actions**
rather than bytes, and a durable file under ADR 0011 § 3's rules that a
session-scoped profile never opens — so cut it on starting if it turns out to be
two.

**Item 69 is the other decision-shaped item** and is the first of section D —
our own JavaScript engine — and the queue still calls it "ADR 0006", which is
the supervisor; the next free ADR number is **0013**. Beyond those, **190** (the
two-tone border styles: small, depends on nothing, closes with a picture) is
ready. 157 and 158 need an interface to ask in, and so does the question item
198 stands in for; 187 is deferred for the reason written into it; 169 must be
run on Linux; 60 is HTTP/3; 197 waits on properties `alo-style` does not have;
and 201 waits on a renderer that can ask for a subresource. The next free queue
number is **202**.

## Iteration 100 — queue item 200: the record itself, for the session

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 200 was
the natural next take and the previous journal said so, along with the warning
it turned out to need: *cut it on starting if it turns out to be two.*

**It was two, and the seam was the one the cache was cut at.** Item 56 was the
cache in memory and item 155 was the cache on a disk, because *what may be
reused* and *what may be written to a disk other programs can read* are two
questions with two answers. The same split here: this iteration is ADR 0012
§ 6's first half — **everything, for the session, in memory, bounded** — and
**item 202** is its second, what an agent did kept until the person deletes it,
which is a different lifetime, a different bound (in actions rather than bytes)
and a file under ADR 0011 § 3.

**What is built.** `crates/alo-net/src/activity.rs`: `Activity`, `Entry`,
`Happened`, and the two bounds. `Pool` holds one; `Pool::activity` reads it and
`Pool::forget_the_record` empties it.

**Where the line is written was the whole design.** A record every caller writes
to is a record missing exactly the lines nobody thought of — the same failure
ADR 0012 § 1 refuses for causes, one layer along. So it is written in
`Pool::fetch_however_it_ends`, which is the one place every public door in that
type leads through: `fetch`, and therefore `follow` and `report`, and `download`
directly. That is what makes the engine-made requests lines of their own without
any of them being asked to be, and it is why *everything, for the session* is a
property of that file rather than a rule its callers keep. Two lines are written
outside it, each for a request that never reached a socket and each said out
loud in the code: what the **cache** answered, and a redirect hop a rule of ours
**refused**.

**Three rules are worth reading twice.** *Never a body and never a header set*
is the type rather than a discipline — an `Entry` is built in one place, from six
fields of a `Request`, with `headers` and `body` in scope and not read — and the
test asserts against the whole of what an entry can be made to say, its own words
and its `Debug`, rather than against the fields it happens to have: a field added
later would pass a test that only checked the fields. The bound is **two**
bounds, lines and bytes, because what a line costs is mostly a URL and a URL is
as long as a page chooses; a reason quoting what a server sent is cut at
`LONGEST_REASON`, since a server that could write a thousand lines into a record
is a server deciding how much memory this process uses, and one that could bury a
real line under its own is worse. And an entry keeps the **cause** rather than a
chain, walking against `Documents` on demand — a frozen chain in every line is
the side table ADR 0012 § 3 refuses by name, and one that disagreed with the
browser process would still read like evidence.

**The honesty the bound owes** is `Activity::forgotten`, in the shape item 199's
`End::Forgotten` already set: a record that quietly shortened itself would read
as a session in which less happened.

**Two of the four closing clauses are met and the other two are item 202's.**
`crates/alo-net/tests/what_the_record_says.rs` drives the **real** `Pool` over
loopback for the first — a redirect chain is more than one line, a cache hit is a
line that says it was the cache, a server that is not there is a line that says
nothing happened, a circle is a line naming the rule, and what an agent set off
is reachable from every line that followed from it while the person's own
browsing reaches no action at all. The fourth clause — no API by which a page or
an agent could read any of it — is kept by the **shape**: a renderer holds no
`Pool`, `alo-agent` does not depend on `alo-net` at all, and nothing crossing the
process boundary carries a line, which is now a match in `message.rs` that a
fifth variant on either enum would break. The other two clauses are about a
disk, and a disk is item 202.

**Two doctored runs rather than reasoning about them**: with the cache-hit line
removed, `what_the_cache_answered_is_a_line_that_says_it_was_the_cache` fails;
with the reason left unbounded, both tests named for the bound fail.

**One thing is written down rather than built**, because this engine cannot
reach it yet: a line is written once, with its outcome, since fetching here is
synchronous and there is no moment at which a request is outstanding and somebody
could be reading. Concurrent loads would need the line opened and closed, and
that is in `Activity::happened`'s own documentation rather than left to be
discovered.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero errors,
**1736 tests** (1713 before, so 23 new — 15 in `activity.rs`, 7 in the new
integration file, 1 in `message.rs`), no stubs, no `unsafe`, boundaries held, the
licence notice, and a `CHANGELOG.md` line. No layout assertion and no reference
render: this iteration positions nothing and paints nothing. One file one
responsibility: `activity.rs` is new and holds one thing — what was asked for and
what happened; `pool.rs` gained a field and two accessors and did not gain a
second reason to change, since where a request is made is where a line is
written. `docs/features.md`'s starred line gains the record and what it refuses.

**`ROADMAP.md` moved, and it is not a tick.** The ★ *every request attributable*
line keeps its empty box. Its `· Built:` clause gains the session's record — what
is in a line, what may never be, the two bounds, and who cannot read it; its
`· Owed:` clause loses the record and now names two things: what an agent did
kept until the person deletes it (item 202), and a cause for a **subresource**,
which still needs a renderer that can ask for one.

**What the next iteration should know.** Item 202 is the natural next take and it
is not small: ADR 0011 § 3's rules unchanged, never written for a session-scoped
profile, a bound counted in **actions**, and a file that is untrusted input the
way `record.rs` is. One thing it cannot inherit and has to decide is written into
the item: a durable entry has no `Documents` to walk against, so it must
**freeze** its chain when it is written — which is not the side table § 3
refuses, because there is nothing left for it to disagree with.

**Item 69 is the other decision-shaped item** and is the first of section D —
our own JavaScript engine — and the queue still calls it "ADR 0006", which is
the supervisor; the next free ADR number is **0013**. Beyond those, **190** (the
two-tone border styles: small, depends on nothing, closes with a picture) is
ready. 157 and 158 need an interface to ask in, and so does the question item
198 stands in for; 187 is deferred for the reason written into it; 169 must be
run on Linux; 60 is HTTP/3; 197 waits on properties `alo-style` does not have;
and 201 waits on a renderer that can ask for a subresource. The next free queue
number is **203**.

## Iteration 101 — queue item 202: what an agent did, kept until the person deletes it

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 202 was
the natural next take, the previous journal said so, and it was as large as that
journal warned.

**What is built.** `crates/alo-net/src/kept.rs` — the directory, the policy, the
bound and the one way a line gets in; `deed.rs` — one action's file, which is
the whole untrusted surface. The division is `disk.rs` and `record.rs`'s,
deliberately, because it is the same pair of questions: *what may be kept* and
*what these bytes are*. `Pool` holds an `Option<Kept>`; `Pool::keeping_what_an_agent_did`
gives it one, `Pool::what_an_agent_did` reads it and `Pool::forget_what_an_agent_did`
deletes it.

**Two files were extracted rather than copied, and that is the part of this
change a reviewer should look at first.** A second durable format needed a
hostile-input reader and a private-file writer, and both already existed inside
`record.rs` and `disk.rs`. Copying either would have been a second place for a
length check to be subtly weaker — and the weaker copy is the one nobody looks
at. So `bytes.rs` is the reader and the writer (`Reader::length` is the line both
formats are built around, and `Reader::how_many` is new: a **count** is a number
a stranger chose too, and the cost of believing one is a loop rather than an
allocation), and `private.rs` is ADR 0011 § 3's promise — the directory made
private, the file written privately, the length asked of the filesystem before
anything is reserved. `record.rs` and `disk.rs` use both now and are shorter for
it.

**The decision the item asked for is the freezing, and it needed one more
decision than the item named.** A durable entry has no `Documents` to walk, so
it freezes the chain — but a frozen link holds **numbers rather than
identities**. ADR 0003's ids are minted once per browser *process*, so `action#0`
exists in every session that had one; a `DocumentId` read off a disk that
compared equal to one minted this morning would join two unrelated pieces of
somebody's history into one story, which is the exact thing ADR 0003 exists to
prevent. The same rule settles what to do with an action from an earlier
session: **never add to it**. It is matched to a file only within the session
that minted it, and what names an action across sessions is the number the disk
counts up.

**Where the write happens is a seam rather than a door, and it is written down
at length in `Kept::take_from`.** Item 200 could put every session line in
`Pool::fetch_however_it_ends` — the one place every request passes. This cannot:
deciding whether a request followed from an action needs the requests
(`Activity`, in the `Pool`, because a pool is what a session holds) **and** what
caused each document's load (`Documents`, in `alo_renderer::Tabs`, because
ADR 0012 § 4 puts attribution where the tabs are) at the same instant, and a
copy of either beside the other is precisely the side table § 3 refuses by name.
So the browser process brings them together, which is the one thing it is for.
What makes that safe rather than a rule somebody keeps: the walk is made **here**
rather than trusted from a caller, so a durable line is exactly as unforgeable as
a session one; it is idempotent by `activity::Entry::sequence`, which is what
that new field is for; `Kept::missed` counts lines that went by before they were
taken, so a browser process that swept too rarely is a number rather than a
silence; and reading brings it up to date, so nothing can be handed a record
somebody forgot to refresh.

**Three things are refused that ADR 0012 § 5 did not have to say.** A `data:`
URL keeps its scheme and media type and loses its content — a URL that *is* the
content is a body wearing an address's clothes, and § 5 refuses bodies. An
address longer than `LONGEST_URL` is cut and says so. And the reason for a cut is
matched back to one of ours on the way in, because a sentence read off a disk and
shown to a person is a sentence somebody else could have written.

**One clause of ADR 0011 § 3 is deliberately not taken unchanged, and this is
the only place this iteration departs from a written decision.** § 3 says *"in
the place the operating system keeps caches"*. The **rules** are taken unchanged
— one directory per profile, private to its owner, no encryption of ours, the
same honest boundary — and the **place** is not: a system empties a cache when a
disk fills, and it is right to, because everything in a cache can be fetched
again. Nothing here can. A record of what an agent did while nobody was watching,
removed by the system on a Tuesday to make room, is the failure the decision
exists to prevent. So it is `Application Support` / `XDG_DATA_HOME` rather than
`Caches`, and the reason is in `kept.rs`'s own documentation and in
`where_the_system_keeps_records`.

**A file that does not read is a gap, and it is left where it is.** That is the
one place the cache's answer is wrong here: `disk.rs` deletes an entry it cannot
decode, because it can never be served and would otherwise sit against the bound
forever. This keeps it, counts it in `Kept::unreadable`, and says so — it is
somebody's record, we are the ones who cannot read it, and a later version of
this engine may be able to.

**All three closing clauses are met over a real restart.**
`crates/alo-net/tests/what_an_agent_did.rs` drives the real `Pool` over
loopback, drops it, and opens the directory again: the agent's two requests are
there with the whole chain still in them, the person's two are not on the disk at
all, a pool with no `Kept` leaves no **directory**, deleting removes the files and
they do not come back, and reading twice writes nothing twice. `kept.rs`'s own
tests cover the bound the ADR asks for by name — a busy action with fifty
requests evicts nothing, and the oldest actions go whole — and `deed.rs`'s walk
every truncation and every single flipped byte, as `record.rs`'s do.

**Two doctored runs rather than reasoning about them.** With the chain frozen one
link deep, four tests fail. With a person's browsing kept too, three fail — and
the first attempt at that doctoring **passed**, which found a real defect: the
selection rule was being asked in two places (`take_from` and `keep`), so
breaking one of them changed nothing. `keep` takes the action as an argument now
and the rule is asked once. A rule this important asked twice is a rule that can
come to be answered differently in one of them.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero errors,
**1783 tests** (1736 before, so 47 new — 7 in `bytes.rs`, 4 in `private.rs`, 16
in `deed.rs`, 15 in `kept.rs`, 5 in the new integration file), no stubs, no
`unsafe`, boundaries held, the licence notice, and a `CHANGELOG.md` line. No
layout assertion and no reference render: this iteration positions nothing and
paints nothing. One file one responsibility: four new files, two of which are
extractions that made two existing files smaller, and `pool.rs` gained a field
and three doors without gaining a second reason to change.
`docs/features.md`'s starred line gains the durable record and what it refuses.

**`ROADMAP.md` moved, and it is not a tick.** The ★ *every request attributable*
line keeps its empty box. Its `· Built:` clause gains the durable record — what
is in it, what never is, the frozen chain, the numbers-not-identities rule, the
bound in actions, where it lives and why, and the gap it counts; its `· Owed:`
clause loses it and now names two things: a cause for a **subresource**, which
still needs a renderer that can ask for one, and an action's own **outcome**,
which is the cut.

**What the next iteration should know.** The cut is **item 203** — an action's
own outcome beside the requests it caused, which is the half of ADR 0012 § 6's
sentence this did not build. It is not blocked on a decision; it is blocked on a
path that does not exist: `alo_agent::Outcome` lives in a crate `alo-net`
deliberately does not depend on (that direction is what makes § 7's *not the
agent* structural), and `alo_renderer::Tabs` holds no `Pool`, so there is nothing
today that could carry a verb's outcome to the record. **That absence is worth
noticing beyond item 203**: it is the same absence item 201 waits on, and this
iteration met it from the other side. Nothing yet wires a `Pool` and a `Tabs`
together, which is why the durable record is taken by a browser process rather
than written where the request is made.

**Item 69 is the decision-shaped item** and is the first of section D — our own
JavaScript engine — and the queue still calls it "ADR 0006", which is the
supervisor; the next free ADR number is **0013**. Beyond it, **190** (the
two-tone border styles: small, depends on nothing, closes with a picture) is
ready. 157 and 158 need an interface to ask in, and so does the question item
198 stands in for; 187 is deferred for the reason written into it; 169 must be
run on Linux; 60 is HTTP/3; 197 waits on properties `alo-style` does not have;
and 201 and 203 wait on the wiring described above. The next free queue number is
**204**.

---

## Iteration 102 — queue item 69: ADR 0013, our own JavaScript engine

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 69 is
marked *needs ADR* and is the first item of section D, so this iteration is the
decision and nothing else — `LOOP.md`'s stage 2 rule 4, and the same shape as
iteration 97. No code was written, deliberately, beyond one comment in the file
the first clause lands in.

**It is not the first unchecked item in the file, and taking it is stage 2's
rule 3 rather than a preference.** Ahead of it sit 157 and 158 (blocked on an
interface to ask in), 187 (deferred with the reason written into it — nothing on
the web can reach the refusal, since `Expect` is a forbidden request header),
169 (must be *run* on Linux), 197 (waits on properties `alo-style` does not
have), 201 and 203 (wait on a wiring iteration 101 described from the other
side), and **60**, HTTP/3, whose dependencies are met. 60 was left where it is:
nothing depends on it, and `LOOP.md` says an item nothing makes reachable is one
to leave alone until something does. Item 69 is the opposite — most of section E
is unreachable without it.

**The number is 0013.** The queue said "ADR 0006", which is the supervisor's and
was taken while that line sat unread; iteration 97 spotted it and said the
iteration taking 69 would fix it. Renumbered rather than reused, which is item
152's rule and ADR 0003's.

**The item named three things and the argument turned out to be a fourth.** What
it is (bytecode compiler and interpreter, correct first), what it is not (a JIT),
and why it is ours (ADR 0001's memory-safety argument) were all still right — but
ADR 0001 refused **V8**, in a paragraph written when JavaScript was years away
and the refusal cost nothing. The refusal that costs something now is **Boa**:
safe Rust, permissively licensed, exists today, and ADR 0009's MPL makes taking
it legally trivial. So licence is not the objection and neither is memory safety,
and an ADR that did not say why would be inheriting a decision rather than
making one.

**Why it is refused, in the terms `CLAUDE.md` already uses.** *Rent the physics,
build the engine.* A shaper, a codec and a Unicode table are physics — nobody's
engine differs by them. An interpreter is not: a page's objects and the DOM's
nodes are **one graph**, so whichever collector traces it decides how `alo-dom`
is stored, and that is the one structure ADR 0003 has already made a promise
about. And every bound on what a stranger's script can make this process
allocate would be somebody else's to choose, which is the thing `alo-net` says
in every file: *a limit somebody else chooses is not a limit*. The cost is
written into the ADR rather than argued away — Boa exists and `alo-js` is years
off.

**Three clauses were added because items 70 to 79 would otherwise each decide
them, differently.**

- **Bytecode from the first line of the compiler.** A suspendable frame is what
  generators, `async` and a debugger all need, and a tree walker expresses it by
  being rewritten. Choosing after there are builtins is choosing to implement
  every semantic twice.
- **Absent beats approximate.** A builtin we have not written is *not defined* —
  not a stub returning a plausible value. Pages already cope with missing
  features by testing for them, and a stub is the one answer that defeats the
  test *and* behaves wrongly afterwards. This is the gate's no-stubs rule
  restated for the place it would look most reasonable to break.
- **`alo-js` depends on no I/O crate at all** — no network, no filesystem, no
  clock, no entropy. Every capability arrives from the embedder, which makes the
  engine testable with nothing moving (the property that made items 55, 154 and
  188 assertable) and makes ADR 0005's *the browser process never runs page
  script* structural rather than remembered.

**Four things are refused and recorded**, each with what would re-open it: a
**JIT** (a measurement on hardware, plus an ADR weighing `unsafe` and
writable-then-executable memory in the process that parses hostile bytes);
`unsafe` in the **value representation**, on the same terms, since NaN-boxing is
the obvious first one and is worth real performance; **`SharedArrayBuffer`**,
which is shared mutable memory between threads and the mechanism that made
Spectre a web attack; and **WebAssembly**, which is on no list in this repository
and which this decision does not put on one.

**One clause is where `CLAUDE.md` and a language disagree, and the ADR says
which way it went.** *The measure is alo, not a conformance score* was written
against Web Platform Tests — a percentage scored against legacy we refuse. A
language ships an executable suite, and there is no honest way to call an
interpreter correct from a handful of examples. So **test262 is vendored per
feature, frozen, and read as a table rather than a score**: the sections for the
feature being built go in with the change, an expected failure is written down
with why, and **no percentage is computed or published** — a number that goes up
is exactly the incentive that makes an engine implement the easy half of
everything.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero errors,
1783 tests unchanged, no stubs, no `unsafe`, boundaries held, the licence notice,
and a `CHANGELOG.md` line. No layout assertion and no reference render: this
iteration positions nothing, paints nothing and executes nothing. One file one
responsibility: one new document and one comment.

**`ROADMAP.md` moved, and none of it is a tick.** Three lines under *JavaScript,
ours, in Rust* gain `· Built:` clauses and keep their empty boxes — the bytecode
and interpreter line (the decision, and the three clauses above), the garbage
collector line (§ 6 states the question: one trait, and the one thing the engine
demands of an embedder's object is that it can be **traced**), and the JIT line,
whose whole content is a refusal and which now names the two conditions for
re-opening it. `docs/features.md`'s starred JavaScript line gains the same in a
reader's words.

**What the next iteration should know.** The next ADR number is **0014** and the
next queue number is **204**. Section D's first buildable item is **70**, the
lexer and parser, whose dependency is now met — and **71** is *needs ADR* in its
own right (a collector is a decision about pauses), which ADR 0013 § 6
deliberately leaves open while stating its problem. Two orderings are worth
noticing before 70 is taken: 71 blocks 72, so its ADR is on the critical path
just as this one was; and item 70's closing condition names *a frozen page's own
script*, so the corpus needs a case whose script is worth parsing before that
item can close — no existing case has one. Outside section D, **190** (the
two-tone border styles) is still ready and depends on nothing.

---

## Iteration 103 — queue item 70: the lexer

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 70 is the
first buildable item of section D and its dependency — ADR 0013, iteration 102 —
is met. It is not the first unchecked line in the file, and taking it is stage
2's rule 3 rather than a preference: 157 and 158 are blocked on an interface to
ask in, 187 is deferred with the reason written into it, 169 must be *run* on
Linux, 197 waits on properties `alo-style` does not have, 201 and 203 wait on a
wiring that does not exist, and 60 is HTTP/3 — which nothing depends on and
nothing makes reachable, so it is left where it is for the third iteration
running.

**Cut on starting, and the cut is at the seam the language has.** Item 70 asked
for a lexer *and* a parser *and* automatic semicolon insertion *and* two
ambiguities. That is not one iteration at the depth this repository builds at,
and `LOOP.md` says to cut the scope and write the cut into the queue rather than
leave a half-built item. So this iteration is **the lexer**, item 70's title and
closing condition are narrowed to it in the queue with the original wording kept
above, and the parser is **item 204** — the same shape as items 59, 62 and 63,
which were each cut on starting.

The seam is the one the language itself has: a lexer turns characters into
tokens and a parser turns tokens into a tree. The half taken is the one a
stranger's bytes reach first, and the one where being wrong is being wrong about
*what a character is*. The arrow-against-parenthesis ambiguity went with the
parser because it is decided by what follows a closing parenthesis, which is a
question about a token stream rather than about characters.

**The interface is the item's own rule, made structural.** `Lexer::next` takes a
**`Goal` every call**. There is no heuristic anywhere and no mode that can be
left set: `/` is division or a regular expression because the caller said which,
and `}` continues a template for the same reason. Every editor guesses from the
previous token and every one of them is wrong on `return /re/` against
`x++ /y/z` — the failure is not cosmetic, it is that the two readings are
different programs. `a /b/ g` is asserted both ways in the table, five tokens and
three, so the thing the design refuses to do is visible as a test.

**Two rules fell out of the order rather than needing code**, and both are
written into the file that has them. Trivia is skipped *before* the goal is
consulted, which is why a pattern can never begin with `/` or `*`: those two
spellings were already taken by a comment. The specification writes that as a
lookahead restriction on the first character of a pattern; here there was
nothing to write. And **`<!--` is not refused**. ADR 0013 § 3 sends Annex B to
the legacy tail, and I started to refuse it by name before noticing that
`a <!--b` is ordinary modern code meaning `a < !(--b)`. Refusing the characters
would break a live page over a decision about 1996, which is law 1 backwards.
Annex B is honoured by **not being implemented** — the characters lex as the
punctuation they are and a page that meant them as comments fails in the parser.

**The one bound is source length, and that is not an oversight.** A lexer has no
nesting, so a million open brackets is a million tokens and no recursion — which
is asserted rather than reasoned about. The depth bound belongs to item 204,
which is the thing that recurses, and `bounds.rs` says so rather than leaving it
for somebody to notice its absence and add a second ceiling in the wrong place.

**One rented crate, and it is not the obvious one.** `unicode-id-start` rather
than `unicode-ident`, which is what the rest of Rust uses: the two answer
different questions — `XID_Start`/`XID_Continue` against ECMAScript's
`ID_Start`/`ID_Continue` — and taking the crate that answers the question the
specification asks costs nothing and leaves no list of exceptions for somebody
to maintain. `crates/alo-js/src/unicode.rs` is its boundary and
`scripts/gate.sh` now checks it. The file also records what is *not* rented and
why: `WhiteSpace` and `LineTerminator` are two short closed lists, and a crate
for either would be a dependency holding twenty numbers.

**`f64` rounding is where ADR 0013 § 8 turned out to be already discharged in
one direction and not the other.** The decimal path composes a plain literal and
hands it to `str::parse::<f64>`, which is correctly rounded and is the standard
library rather than a crate — so there is nothing to rent. The other three bases
*cannot* use it (`0x1p3` is a Rust hexadecimal float and not a JavaScript one),
and a literal with more than fifty-three significant bits has to round **once**.
So `number::from_power_of_two` walks the bits with a guard and a sticky bit,
nearest with ties to even, and allocates nothing — a literal is as long as the
page chose. `0x20000000000001` and `0x20000000000003` are the two cases in the
table, because they differ only in the parity of the significand and an
accumulate-as-you-go loop gets exactly one of them right.

**Strings are `Vec<u16>` and that is a correctness decision rather than a
representation one.** `'\uD800'` is a legal program: one code unit, half a
surrogate pair, standing for no character. Nothing in the crate goes through
`char` on the way out of a literal, and the table asserts it — the sketch a test
compares against falls back to `[U+D800]` when the units are not text, because a
test that could only show valid text could not tell that case from a refusal.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero errors,
**1819 tests** (1783 before; 36 new), no stubs, no `unsafe`, every boundary held
including the new one, the licence notice on all fourteen new files, and a
`CHANGELOG.md` line. No layout assertion and no reference render: this iteration
positions nothing and paints nothing. One file one responsibility: fourteen
files, each named for the one question it answers — `read.rs` is *how source
text is looked at without indexing into it*, and it exists because a byte offset
into UTF-8 is the one arithmetic in a lexer that panics.

**The hostile half is stage 2's clause 2 and it is the shape rather than a
list.** A list of malformed cases finds what somebody thought of. This cuts a
nasty corpus at **every character boundary, from both ends**, and reads every
code point up to U+FFFF on its own — deterministic, so a failure is reproducible
in a way a random fuzzer's is not. It also asserts the lexer *advanced*: a token
that consumed nothing is an infinite loop on somebody's page, and it is the
failure a "returns rather than panics" test would otherwise miss entirely.

**The frozen page is alo's own service worker**, and it went in
`crates/alo-corpus/scripts/` — a second kind of frozen thing beside `cases/`,
because a case is a page with an expected box tree and an expected picture and
nothing renders a service worker. What it shares with a case is the property
`LOOP.md` actually asks for: frozen, never fetched, with `origin.txt` saying
where it came from and when. `alo-js` reads it **by path** rather than through
`alo-corpus`, since ADR 0013 § 5 gives that crate no dependencies and a route
through the corpus would put the whole renderer behind a lexer — and behind a
dependency cycle, the day the renderer runs script.

The assertion worth reading twice is not the token count. It is that **the gap
between every pair of neighbouring tokens is itself lexed and must come back
empty**: a lexer that skipped a character it should have read would otherwise
produce a perfectly tidy token stream with a hole in it, and every span
assertion would still pass.

**It found nothing, and `origin.txt` says so** — which is the honest report
rather than a disappointing one. What it did settle is that reading the whole
file with one goal is the right reading, because the script has no division and
no regular expression in it; that is asserted (`no_slash_in_it_means_the_goal_
never_mattered`) rather than assumed, so the day somebody freezes a script with
a pattern in it the test fails and says why.

**`ROADMAP.md` moved, and it is not a tick.** *Lexer and parser to an AST* gains
a `· Built: … · Owed: …` clause and keeps its empty box: the lexer with its goal
argument, the bounds, the refusals and both kinds of evidence on the Built side;
the parser, automatic semicolon insertion and the second ambiguity on the Owed
side, named as item 204. `docs/features.md` gains the same in a reader's words,
plus a line for the frozen script.

**What the next iteration should know.** The next queue number is **205** and the
next ADR number is **0014**. Section D's next items are **204** (the parser, cut
here) and **71** (the object model and a collector), and **71 is `needs ADR` in
its own right** — ADR 0013 § 6 states its problem and deliberately leaves it
open. The ordering is worth noticing: 71 blocks 72, so its ADR is on the critical
path exactly as 69's was, and it is takeable *now* while 204 is a large build.
Item 204 carries one thing that is easy to lose: **freezing a second script with
a regular expression in it is part of that item**, because the one in the corpus
cannot exercise the goal choice. Outside section D, **190** (the two-tone border
styles: small, depends on nothing, closes with a picture) is still ready.

---

## Iteration 104 — queue item 204: the parser, to a syntax tree

**Taken because it is the first ready item in the file.** 157 and 158 need an
interface to ask in, 187 is deferred with the reason written into it, 169 must
be *run* on Linux, and 60 is HTTP/3. In section D the two ready items were 204
and 71; 204 comes first in the file and its one dependency (item 70) was built
last iteration, so the ordering rule took it. 71 is `needs ADR` and is where the
next iteration should look.

**Built: `crates/alo-js/src/ast.rs` and `crates/alo-js/src/parser{,/*}.rs`** —
the tree, the cursor, and six files of grammar. Both frozen scripts parse, which
is the item's own closing condition, and the second of them was frozen here
because the first could not close it.

**The seam this cut is at.** The lexer answers *what a character is*; the parser
answers *what a token stream means*, and the whole of the difference is visible
in one interface: `Lexer::next` takes a `Goal` every call, and the parser is the
thing that knows which. `parser.rs` names the two goals `OPERAND` and `OPERATOR`
so that a call site reads as the claim it is making. One token of lookahead is
kept **with the goal it was read under**, so asking again under a different goal
re-reads it from the source — which is what makes `` `${x}/y/` `` work: the
substitution ends by peeking at `}` as an operator, and the tail is then asked
for at the same offset as a template continuation, where `/y/` is text rather
than two divisions. That is asserted in the table rather than reasoned about.

**The arrow ambiguity is settled where item 70 said it would be.** `(a, b)` and
`(a, b) => c` are the same characters until the `)` has been passed, so the
parameter list is **tried and put back**. What the naive version of that gets
wrong is cost: trying costs a second read of what is inside the parentheses, and
a `(` inside a `(` pays it again at every level, which is quadratic on a page
that chooses how deeply it nests. So a `(` that turned out not to open a
parameter list is remembered by its offset and never tried twice. The same shape
settles four other contextual words — `let` before a name, `async` before
`function` **on the same line**, `static`, and any name before a `:`.

**The depth bound needed a stack before it could mean anything, and that is the
finding of this iteration.** `bounds.rs` had said since item 70 that nesting
belonged to the parser. I set it at 512, wrote the test that stands either side
of it, and the test **aborted**: a `cargo test` thread has two mebibytes, a
bracket level is thirteen frames, and a debug build gets under fifty levels
before the stack is gone. An abort is not a refusal — it is the process going
away, which is the one thing ADR 0013 § 4 forbids outright — and the counter
never reached its ceiling to say so.

Lowering the number to what a debug test thread survives would have made the
bound a property of *whoever called us*: fifty in a test, a few hundred in a
release build, whatever a renderer was given in production. `alo-net` has
written the answer to that in every file it has — **a limit somebody else
chooses is not a limit** — so the parse now runs on a *scoped* thread of its
own with `bounds::STACK_FOR_A_PARSE`, thirty-two mebibytes, which is measured
rather than guessed: 256 bracket levels needs under twelve in a debug build and
about a fifth of that in a release one. Scoped, so the source text is still
borrowed and nothing is copied to get it there; and a panic inside is raised
again on the caller's thread rather than turned into a refusal, because a bug
reported as a syntax error is a bug nobody finds.

`DEEPEST_NESTING` is 256 and now means the same thing in a debug build, a
release build and a renderer. The cost is a thread per parse — about thirty
microseconds, which is nothing beside a script and is why the hostile test that
parses two hundred thousand tiny programs takes five seconds. Any claim beyond
that is a performance claim and needs hardware.

**Two refusals are the ones worth reading twice**, because each is a place a
parser is quietly wrong rather than loudly. `a ?? b || c` is not a program, and
the tree cannot tell it from `(a || b) ?? c` afterwards — parentheses are not
nodes here — so it is refused *while parsing*, by the function that knows
whether a `||` was written at that level and returns the fact alongside the
expression. And `{ a = 1 }` is a destructuring pattern rather than an object
literal, decided by an `=` that comes after the whole of it: `[{ a = 1 }] = b`
is ordinary and `f({ a = 1 })` is not a program. So its refusal is **kept
rather than raised**, dropped the moment the thing holding it is turned into a
pattern, and raised where an expression can no longer become one. Those two are
the whole of the cover grammar this parser needs, because the other cover the
specification has is the arrow parameter list, and that is settled by trying it.

**It found one defect in the lexer, in the place that design was most confident
about.** `Goal::TemplateContinuation` skipped no trivia, on the stated reasoning
that everything after the `}` is the template's own text. That is true and it is
about the wrong side of the brace: the space in `` `${ a }` `` belongs to the
substitution that has just ended, and the specification's
`InputElementTemplateTail` lists whitespace, line terminators and comments for
exactly that reason. Ordinary code would not have parsed. Fixed, with a lexer
test named for the rule and the finding recorded in `lexer.rs` where somebody
will read it.

**The evidence is three kinds, and none of them is the other.** A **table**
(`what_the_parser_makes_of_it.rs`, 23 tests) where the answer is the tree
printed back as source with every grouping made explicit — `(a + (b * c))` —
because a wrong precedence, a wrong associativity and a missing node all change
the parentheses and a reader can check that by eye. **Two frozen scripts**
(`two_frozen_scripts_parse.rs`): alo's service worker, and alo's theme generator
frozen here because the first has no `/` in it at all. The generator holds six
regular expressions and **no division**, which a test asserts by walking the
whole tree — a pattern read as arithmetic is a different program that parses
perfectly well, and every other assertion would still have passed. And the
**hostile** half (`a_program_that_is_hostile.rs`): a nasty corpus cut at every
character boundary from both ends, both goals, every code point up to U+FFFF as
a program of its own, and the depth bound from both sides.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1853 tests** (1819 before; 34 new), no stubs, no `unsafe`, every
boundary held, the licence notice on all nine new files, and `CHANGELOG.md`.
No layout assertion and no reference render: this iteration positions nothing
and paints nothing. One file one responsibility: the grammar is six files under
`parser/` — expressions, statements, binding, functions, properties, classes,
modules — because a parser that is one file is a file with a reason to change
for every production.

Clippy earned two changes that are better code rather than lint appeasement,
and both are recorded because the reason outlives the lint. `Context`'s nine
booleans became `Inside` (which of six kinds of body), `Home` (what `super` has
to look in) and `Leaving` — and writing `Leaving` out as *two* facts rather than
one caught a bug I had not noticed: a `switch` inside a loop is something
`break` may leave and `continue` may not, so a single "innermost thing" would
have lost the loop the moment the `switch` was entered. `Function`'s four
booleans became `FunctionKind`, because `async` and `*` are read together
everywhere.

**`ROADMAP.md` moved, and it is not a tick.** *Lexer and parser to an AST* gains
a second `· Built:` clause naming the parser, the arrow decision, the goal
choice, the refusals and both kinds of evidence, and a new `· Owed:` naming item
205 — so the line keeps its empty box, because early errors that need a scope
are part of reading a program and are not built. `docs/features.md` gains two
lines in a reader's words.

**What the next iteration should know.** The next queue number is **206** and
the next ADR number is **0014**. Section D's ready items are **71** (the object
model and a collector) and **205** (cut here). **71 is `needs ADR` in its own
right** — ADR 0013 § 6 states its problem and deliberately leaves it open — and
it is on the critical path in a way 205 is not: 72 depends on 71, and every item
from 73 to 79 is behind 72. `LOOP.md`'s stage 2 clause 4 says a decision is its
own iteration, so 71's ADR is a whole iteration with no code in it, exactly as
69's was. Item 205 is the better second choice, and it is not urgent: nothing
depends on it, and its own content says why each refusal waits for a scope.
Outside section D, **190** (the two-tone border styles: small, depends on
nothing, closes with a picture) is still ready.

---

## Iteration 105 — queue item 71: ADR 0014, the collector and the object model

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 71 is
`needs ADR`, so by `LOOP.md`'s stage 2 clause 4 this iteration is the decision
and nothing else: **ADR 0014**, no code, exactly as iteration 102 was for
ADR 0013.

**Taken over 205, which is earlier in the file, and the reason is the ordering
rule rather than a preference.** Stage 2's clause 3 says dependencies decide.
Item 205 is the early errors that need a *scope*, and its own text says where a
scope belongs: *"a scope is the thing item 71's object model and item 72's
compiler both need, and building a second one inside the parser is how the two
come to disagree."* Taking 205 first would mean building the scope before the
thing that owns it, which is the item arguing against itself. Everything else
outside section D is where it was: 157 and 158 need an interface to ask in, 187
is deferred with its reason written into it, 169 must be *run* on Linux, 197
waits on properties `alo-style` does not have, 201 and 203 wait on a wiring that
does not exist, and 60 is HTTP/3, which nothing depends on and nothing makes
reachable.

**The item is one line of queue and the decision turned out to be four things
that cannot be changed afterwards**, which is what made it worth a whole
iteration:

- **Where a reference may live.** A precise collector runs only where it can
  find every live reference, so the answer decides the shape of the
  interpreter's stack and the signature of every builtin. An engine that settles
  this after it has two hundred builtins rewrites two hundred builtins.
- **Whether the DOM is in the graph**, which decides what `alo-dom` is and is
  the clause ADR 0013 § 1 refused a rented engine over.
- **Whether there is a write barrier**, which looks like an optimisation and is
  the hook both answers to a visible pause need.
- **Whether the marker recurses**, which is item 204's finding on a graph whose
  depth a script chooses.

**What was decided**, in the order the ADR argues it. Tracing rather than
counting, because the cycle is the normal case: `addEventListener("click", () =>
node.focus())` is one in the first line of most pages, and a counted heap needs
a second collector to find it. **Precise** rather than conservative, because
conservative scanning needs `unsafe` to read the machine stack and retains by
accident — so the places a live reference may be are a closed list of five, and
anything else holding one across an allocation is a bug. A reference is an
**index carrying a generation**, which is ADR 0004's move for `taffy`'s handle
made again for the same reason: an index is safe code where a pointer is
`unsafe`. **Non-moving mark and sweep**, stop-the-world, correct before fast.
The **DOM is traced rather than counted**, one graph, one collector, with the
trait in `alo-js` and the bindings crate the only thing depending on both.
**Ephemerons to a fixpoint** from the first line. The **marker never recurses**
and a collection **allocates nothing**. The heap's ceiling is ours, and a full
heap is an error somebody is told about rather than an abort.

**Two collisions with earlier decisions are settled rather than left implied.**
ADR 0003 says a node's identity is allocated once and never reused, and a heap
cannot afford never to reuse a slot — so what is never reused is the **pair**,
slot plus generation, and a reference whose generation no longer matches names
*nothing* rather than naming whatever took the slot. ADR 0003's promise is kept
at the level it was made. And a generation that would wrap **retires the slot**
instead of wrapping, which costs one slot and closes the one hole in that
argument rather than describing it.

The second is what a stale reference *does*. In an engine with pointers it is a
use-after-free, which is the most valuable bug class in a browser. Here it is a
mismatch the engine can see, it is always our bug rather than a page's, and it
ends the script with an internal error — never the process (ADR 0005), never a
panic (ADR 0013 § 4), and never a wrong object handed back as though it were
right.

**The write barrier is the clause I would most expect a later iteration to
argue with, so it is argued for here.** It does nothing today: it stores and
returns. It exists because incremental marking needs the tri-colour invariant
maintained on every store and a generational nursery needs a remembered set, and
because installing it afterwards means auditing every mutation in an engine that
by then has builtins, a DOM binding and a compiler emitting stores. It is
structural rather than remembered — an object's reference-bearing fields are
private to the heap module, so there is no second way to write one.

**One thing is explicitly allowed later without an ADR, and saying so is part of
the decision.** Hidden classes and inline caches are refused now under law 3 and
may arrive whenever somebody wants them, provided the semantics and the property
order are unchanged: they are an optimisation behind one interface. A JIT is not,
which is why ADR 0013 § 2 gives it two conditions and this gives shapes none.
The difference is the line, and an ADR that did not draw it would have every
future optimisation asking permission.

**What is deliberately not in the ADR: the numbers.** The heap ceiling, the
collection trigger and the worklist capacity land in `bounds.rs` with the code,
each with its reason beside it, as `LONGEST_SOURCE` and `DEEPEST_NESTING`
already do. A number written into a decision is a number nobody can tune with
evidence — and every number in this repository that is defensible was measured
by the iteration that needed it.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1853 tests** unchanged (no code), no stubs, no `unsafe`, every
boundary held, the licence notice intact, and a `CHANGELOG.md` line. No layout
assertion and no reference render: this iteration positions nothing, paints
nothing and executes nothing. One file one responsibility: one new document, and
one paragraph added to `alo-js`'s own module documentation, which is where
somebody building item 71 will actually be standing.

**`ROADMAP.md` moved, and it is not a tick.** *A garbage collector, and the
object model underneath it* gains a second `· Built:` clause — the decision, its
four retrofit-proof clauses, and the two refusals worth naming (counting, and
conservative scanning) — and keeps its empty box, with `· Owed:` naming all of
the code and the numbers that land with it. `docs/features.md`'s line gains the
same in a reader's words. No other line moved, because no other line's content
changed.

**What the next iteration should know.** The next queue number is **206** and
the next ADR number is **0015**. Section D's ready items are **71**, whose
decision is now made and whose code is a large build, and **205**. **71 is the
one to take**: 72 depends on it and every item from 73 to 79 is behind 72, and
its closing conditions are written into the queue item — a cycle reclaimed and
counted rather than watched, a stress mode that collects at *every* safepoint,
the heap invariants checked after every collection, and the hostile half. Two
things in it are easy to lose sight of and are named in the ADR rather than left
to be noticed: the stress mode is not optional, because a builtin holding a
reference across an allocation is correct in every ordinary run and wrong only
in that one; and a collection must allocate nothing, because the moment it is
most needed is the moment there is nothing to spare. Item 205 is still the
better second choice and still not urgent. Outside section D, **190** (the
two-tone border styles: small, depends on nothing, closes with a picture) is
still ready.

---

## Iteration 106 — queue item 71: the heap, and the collector that owns it

**The tree was clean on entry and `scripts/gate.sh` was green.** Item 71's ADR
was written by the iteration before this one, so this is the code — the shape
`LOOP.md`'s stage 2 clause 4 asks for, and the same shape ADR 0005 and
`alo-renderer` had.

**Taken over 205, which is earlier in the file, and the reason is unchanged
from last time**: 205's own text says a scope belongs to the thing that owns
it, and 71 is that thing. Everything else outside section D is where it was:
157 and 158 need an interface to ask in, 187 is deferred with its reason
written into it, 169 must be *run* on Linux, 197 waits on properties
`alo-style` does not have, 201 and 203 wait on a wiring that does not exist,
and 60 is HTTP/3, which nothing depends on and nothing makes reachable.

**Scope was cut and depth was not, and the cut is the ADR's own seam.** ADR
0014 § § 1 to 10 are the heap and the collector; § 11 is what a *cell* is —
prototypes, properties, an observable order, interned keys, strings. This
iteration built the first and wrote the second into the queue as **item 206**.
The order was not a preference: `Heap<T>` is generic in its cell, so the object
model lands inside it without changing a line of `heap.rs`, and building it the
other way round would have meant putting objects somewhere else first and then
moving them into a heap. Item 72 now depends on 71 **and** 206, which is
written into 72.

**What was built**, in six files, one responsibility each: `heap.rs` is the
arena and its interface; `heap/reference.rs` is what names a cell and the two
kinds of field that hold one; `heap/trace.rs` is the one demand the engine
makes of anything in the heap; `heap/root.rs` is the closed list of places a
live reference may be; `heap/collect.rs` is mark and sweep; `heap/check.rs` is
the invariants. Four numbers landed in `bounds.rs` with their reasons, which is
where ADR 0014 § 9 says they go rather than in the decision.

**All four closing conditions are met, and each is a test rather than a
sentence.** A cycle is reclaimed and **counted** — `Heap::live` is a number the
heap knows, so nothing here watches the process's memory. One of those cycles
goes through an **embedder's** object: a node, a listener on it, a closure back
to the node, which is § 6's clause in the only form available before the
bindings crate (item 80) exists, and the test says so in its own words rather
than implying it tested the real DOM. The stress mode collects at every
safepoint — today that is every allocation — and **both halves are asserted**,
because a mode that reclaimed everything would pass the first half by being
useless. The invariants are `Heap::check`, run after every collection in every
test, and it walks with the collector's own marker rather than a second one:
ADR 0014 § 1 refuses two ideas of what is alive, and a check with its own idea
would be that mistake in the place it is hardest to see.

**Three things are worth reading twice, and two of them are defects this
iteration found in its own first design.**

The **bounded ephemeron buffer was a correctness bug** before it was fixed. ADR
0014 § 8 says a collection allocates nothing, so the pair list is bounded like
the worklist — and the § 8 argument that an overflow *costs a rescan and never
correctness* is true of the worklist and was **not** true of the pairs. A
worklist overflow leaves a marked cell whose children were not followed, and a
rescan finds it by its mark bit; a pair overflow leaves nothing behind to find
it by, so a `WeakMap` with more live entries than the buffer holds would have
had entries silently dropped — a value a page can still ask for and would not
get. Two changes fix it, and the second is what makes the first sound: a pair
whose key is **already marked is settled where it is reported** and never
stored at all, which is the common case and empties the buffer of everything
decided; and a collection that ever refused a pair does not finish until a pass
over every marked cell marks nothing new. Since a rescan re-derives every pair
from the cell holding it, and by then a key may be marked, the last thing such
a collection does is a full pass that found nothing. `Heap::rescans` counts
those passes, and the two hostile tests **assert it is not zero** — a bound
nothing ever reaches is a bound nobody has checked is reachable, which is
exactly what item 204 learned about `DEEPEST_NESTING`.

A **retired slot needed the retired generation reserved rather than reached.**
ADR 0014 § 3 says a slot whose generation would wrap is retired instead. Written
the obvious way — stop when the counter cannot be raised — the last reference
handed out before retirement goes on matching for ever, so retiring the slot
keeps alive the one thing it was retired to let go of. `u32::MAX` is reserved
and `next_life` stops one below it, so no reference is ever made carrying it.

And **the cell being allocated is traced as a root** for the collection its own
allocation caused. Without it the discipline in § 2 would include "do not build
an object", since an allocation is a safepoint and the references the new cell
carries would be nobody's. It is not a weakening of precision: the cell is about
to be live and the collector has it in its hand.

**One thing about the write barrier is honest rather than absolute.** ADR 0014
§ 5 says an object's reference-bearing fields are private to the heap module so
there is no second way to write one. `Field` has no mutator but `set`, which
takes a `Barrier`, and the only `Barrier` comes from `Heap::write` — but Rust
cannot stop somebody assigning a whole field over. That is written into
`Field::holding`'s own documentation rather than left implied, with the second
half of § 5 named as what closes it: a cell keeps its fields private. Nothing in
this crate assigns one, and `Heap::stores` counts every store that went through
the barrier so a test can say so.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1879 tests** (up from 1853 — twenty-one new cases and five unit
tests), no stubs, no `unsafe`, every boundary held, the licence notice on all
six new files, and a `CHANGELOG.md` line. No layout assertion and no reference
render: this iteration positions nothing and paints nothing. One file one
responsibility: six files, each named above with the one reason it changes.

**One thing a later iteration will meet and should not spend an hour on.** The
panic family is denied outside a test, and clippy means a `#[test]` function
rather than a test *crate* — so a helper in `tests/*.rs` that unwraps is
production code as far as the lints are concerned, and they are right. The two
new test files allocate through a **macro**, which expands at the call site, and
each says why in a comment.

**`ROADMAP.md` moved, and it is not a tick.** *A garbage collector, and the
object model underneath it* gains a third `· Built:` clause naming the crate,
the three retrofit-proof clauses as things a reader can check, and the numbers;
its `· Owed:` names the object model (item 206), the DOM's real wrapper (item
80) and the weak-reference callbacks (item 76), and says plainly that no claim
about speed is made because none has been measured. The box stays empty,
because half a line is not a line. `docs/features.md`'s line gains the same in a
reader's words.

**What the next iteration should know.** The next queue number is **207** and
the next ADR number is **0015**. Section D's ready items are **206** and
**205**. **206 is the one to take**: item 72 depends on it, it is the other half
of an ADR that is already written, and its closing conditions are in the queue —
the observable property order, a prototype chain with a refused cycle, and the
hostile half item 71 could not have, which is an unbounded number of distinct
property keys. Two things it will want that are already there: `Heap::write` is
the only way to get `&mut` to a cell and it hands over the `Barrier` with it, and
`Trace::footprint` is how a cell that grows tells the heap it did — a property
table that grows without reporting is a ceiling that is not enforced. Item 205
is still the better second choice and still not urgent. Outside section D,
**190** (the two-tone border styles: small, depends on nothing, closes with a
picture) is still ready.

---

## Iteration 107 — queue item 206: the object model

**Item 206 is done**, which is ADR 0014 § 11 and the other half of item 71. It
was the item the last iteration named as the one to take, for the reason it
gave: item 72 depends on it, the ADR was already written, and its closing
conditions were in the queue.

**It landed inside the heap without one line of `heap.rs` changing.** That was
the argument for building the collector first and it held: `Heap<T>` is generic
in its cell, `object::Cell` is now that cell, and the only files under `heap/`
that changed at all are `reference.rs` — for a reason given below — and two
doc comments that said the object model was owed.

**All three closing conditions are met, and each is a test.** The order a page
enumerates is asserted from keys of all three kinds minted in the worst order
for the rule — a symbol first, the indices descending, the strings in the
middle — and the near misses have a test of their own, because `"01"`,
`"4294967295"`, `" 1"` and `"-0"` are string keys that look like indices and a
page can see where they come. A prototype chain answers a lookup, and a cycle
is refused twice over: at the assignment, which is the specification's own
rule, and by a bound on the walk, which is the defence against an embedder that
does not obey it.

**The hostile clause is answered by a collection rather than by a refusal, and
the test says which.** Item 206 allowed either. Two hundred thousand distinct
names are minted; the count is what makes it a bound rather than a hope, since
it is far past `COLLECT_AFTER` and so the collector fires **on its own** during
the loop. `Heap::collections` is asserted to be past zero for that reason: a
test that asked for the collection would be asserting that it can call a
method. The arena ends with fewer slots than half the names and the intern
table ends empty.

**Three things are worth reading twice, and the first is a hole this iteration
nearly left in the ceiling.**

The intern table holds **no copy of the text**. The obvious table is
`HashMap<Box<[u16]>, Ref>`, and it would put a second copy of every property
name *outside* `HEAP_CEILING` — which is the same leak ADR 0014 § 11 names, in
a place nothing counts, and it would have passed every test I had written. So
the table is a **seeded hash to the cells that hashed to it**, and a lookup
compares by reading the string cell it already has. The seeding is a security
property rather than a detail: a page chooses every name it writes, and a fixed
hash function is an invitation to engineer collisions until a lookup is a walk.
It is `RandomState`, which is what `HashSet` in `heap/root.rs` already relies
on, and it reaches no I/O crate — ADR 0013 § 5's rule is about dependencies.

**The keys are edges.** A property named by a string keeps that string alive,
which is what makes a weak intern table safe rather than merely small: what
holds a name is the object whose property it is, and interning holds nothing.
The test that says so is the one that deletes a property and finds the heap
down to one cell and the table down to none.

**The prototype walk is bounded by the number of slots in the heap**, which is
exact rather than chosen — a chain longer than that has visited a slot twice.
Nothing a page writes can make a cycle, because `set_prototype` refuses one;
an embedder answers `[[GetPrototypeOf]]` for itself, and the test builds
exactly that: a foreign object whose prototype is set straight through
`Heap::write` rather than through the rule, which is what an embedder that does
not consult the engine amounts to. A renderer that hung there would be a denial
of service in the process that parses hostile bytes (ADR 0005).

**One change to item 71's code, and it is the barrier's hook rather than a way
round it.** `Barrier::record` was private, so a property *value* — which is a
heap reference only sometimes — had no way to record its store. It is
`Barrier::stored` and public now, with the reason written into it: what
ADR 0014 § 5 forbids is a **mutator that skips the barrier**, and reaching the
barrier is the opposite of skipping it. A `Barrier` still cannot be made from
outside; the only one there is comes from `Heap::write`.

**One test seam was added and it is narrower than the one item 71 has.**
`Ref::for_a_test` is `#[cfg(test)]`, reaches no further than this crate's unit
tests, and exists because the property table's business is the **order** of
keys: making a real heap to get two distinct names for it would test the heap
in the file that tests the order. Every integration test, and every test that
collects, allocates properly.

**What was cut, and none of it is depth.** A `BigInt` value is **item 207** and
is marked `needs ADR`, because arbitrary-precision arithmetic is a question
about renting rather than a variant to add — item 70's lexer already keeps a
`BigInt` literal's digits as text for exactly this reason. A **partial**
property descriptor and the well-known symbols went to item 73, written into
it; a **proxy** intercepting the walk itself went to item 72, written into it.
Nothing here is callable, so an accessor answers with its **getter** rather
than with a value, which is ADR 0013 § 3's *absent beats approximate* in its
most literal form: an interpreter that has not learned to call one will not
compile against this interface, where an engine that answered `undefined` would
run and be wrong.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1920 tests** (up from 1879 — twenty-four cases in two new integration
files and seventeen unit tests), no stubs, no `unsafe`, every boundary held,
the licence notice on all eleven new files, and a `CHANGELOG.md` line. No
layout assertion and no reference render: this iteration positions nothing and
paints nothing. One file one responsibility: eleven files, each named in
`object.rs`'s own module documentation with the one reason it changes.

**`ROADMAP.md` moved, and it is not a tick.** *A garbage collector, and the
object model underneath it* gains a fourth `· Built:` clause — the five § 11
decisions as things a reader can check, and the hostile half by name — and its
`· Owed:` is rewritten: the DOM's real wrapper (item 80, whose trait is built
and tested against a stand-in), the weak-reference callbacks (item 76), the
partial descriptor and well-known symbols (item 73), `BigInt` (item 207), and a
proxy's own `[[Get]]` (item 72). The box stays empty because **nothing here is
callable**, and a line about objects that cannot yet answer a getter is not a
finished line. `docs/features.md`'s line gains the same in a reader's words.

**What the next iteration should know.** The next queue number is **208** and
the next ADR number is **0015**. Section D's ready items are **72** and **205**,
and **72 is the one to take**: its dependencies (70, 71, 206) are all done, it
is the item most of the rest of stage 2 is unreachable without, and it is where
this engine stops holding a program and starts running one. Two things it will
want that are already there: `object::Objects` is the type it holds — the heap
and the intern table together, since interning needs to read the heap — and
`Objects::define_named` is the shape that interns and defines with no
allocation in between, which is the rooting discipline written as one call
rather than remembered. One thing it must decide early: ADR 0014 § 2 says the
interpreter's **frames and value stack** live in structures the collector walks
rather than in Rust locals, and that is the clause with the longest reach in
the whole decision. Item 205 is still the better second choice and still not
urgent. Outside section D, **190** (the two-tone border styles: small, depends
on nothing, closes with a picture) is still ready.

---

## Iteration 108 — queue item 208: the parser bounds the tree it builds

**The item I took is not the item the last iteration named**, and the reason is
the whole of this entry. It named **72**, correctly. Item 72's compiler walks
the tree the parser hands it, one stack frame per level, so the first thing I
did was ask how deep that tree can be — and found that the answer is *as deep
as the file is long*, that this is reachable from any page, and that it ends
the renderer.

`script("+a".repeat(60_000))` parses in about a second and then **aborts the
process**. Not while reading it: while *dropping* it. `Drop` walks the tree one
frame per level like every other reader, and there was no reader before it, so
nothing had ever found out.

So item 72's scope was cut on starting, the cut is **item 208**, and 208 was
taken first. `LOOP.md` allows the cut (*"if it is turning out larger than one
iteration, cut its scope, never its depth, and write the cut into the queue as
a new item"*); what made it the item to take rather than one to file is that
ADR 0013 § 4 says *it never panics, not on any source text, not on any program*,
and a renderer that stops is the denial of service ADR 0005 is built around.
Building a compiler on top of it would have been building on a hole somebody
had already walked into.

**Nine shapes, and they are two defects wearing one name.** Item 204 counts how
deep the parser *recurses*, which is the right bound for a bracket and the
wrong question for everything else.

- **Five recursed where nothing counted** and overflowed the parse thread's own
  thirty-two mebibytes: `!!!…a`, `- - - …a`, `typeof typeof …a`, `new new …a`,
  and `a**a**a…`. Each is a recursive call in `expression.rs` that had no
  `deeper()` around it.
- **Four are read in a loop**, so they cost the parser no stack at all and only
  the *tree* gets deeper: `a.b.b.b…`, `a()()…`, `a?.b?.b…`, `a+a+a…`, with a run
  of tagged templates and `||`, `&&`, `??` beside them. These parsed perfectly
  and died on the way out.

**Two bounds now, and they are two questions.** `DEEPEST_NESTING` stays at 256
and means what it always meant: how deep this parser recurses, measured against
`STACK_FOR_A_PARSE`, where a bracket costs thirteen frames.
`DEEPEST_EXPRESSION` is new and is 4096: how deep a **tree** it will build,
where a level costs one frame in every walker there will ever be. The number is
measured rather than chosen — a `cargo test` thread has two mebibytes and drops
a tree sixteen thousand levels deep without trouble in a debug build, so 4096 is
that with a margin, and it is sixteen times the other because a level here is a
sixteenth of the cost. `Reason::ExpressionTooDeep` is a refusal of its own, so a
test asserts *which* bound answered rather than that something was refused.

**The rule worth reading twice is what the new counter does not do**, because
the obvious implementation is wrong in a way every test would have passed.
`Parser::linked` counts the **path** rather than the loop, and `Parser::beside`
puts the count back only around **siblings** — the right side of an operator, an
argument, an array element, a property's value, a branch of a `?:`, one
declarator, one statement. A counter that were put back when each *loop* ended
is defeated by nesting: two hundred levels, each a thousand links, none of which
reaches the ceiling alone and which together are two hundred thousand deep. I
built that program and it is now a test
(`a_chain_inside_a_chain_inside_a_chain_is_counted_as_all_three`).

**And the half that would have been much harder to notice.** A bound that added
siblings up would be sound and would refuse most of the real web. So the other
new test is the opposite one: an array of fifty thousand elements, an object of
twenty thousand properties, a call with twenty thousand arguments, a `var` with
five thousand declarators, sixty thousand comma operands, twenty thousand
statements and a template with twenty thousand substitutions all still parse and
are dropped. Both frozen real scripts still parse, unchanged, which is the
evidence that mattered most.

The counting is deliberately a slight **over-approximation** in one place: a
long chain whose *operands* are themselves long chains is charged for both. It
takes two hundred chains of two thousand terms each to notice, over-approximating
is the safe direction, and under-approximating is the bug this item exists to
fix.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1924 tests** (up from 1920 — four new cases in the parser's existing
hostile file), no stubs, no `unsafe`, every boundary held, the licence notice
intact, and a `CHANGELOG.md` line. No layout assertion and no reference render:
this iteration positions nothing and paints nothing. One file one
responsibility: no new file, because this is not a new responsibility — it is
`a_program_that_is_hostile.rs` finally asking its own question properly, plus a
bound in `bounds.rs`, a refusal in `error.rs` and two helpers in `parser.rs`
beside the two that were already there.

**`ROADMAP.md` moved, and it is not a tick.** *Lexer and parser to an AST* gains
a third `· Built:` clause naming the nine shapes, the two bounds and the two
directions the tests stand in; its `· Owed:` is unchanged, because item 205 is
untouched. `docs/features.md`'s parser line gains the same in a reader's words,
including the sentence about width that is the part somebody would otherwise get
wrong.

**What the next iteration should know.** The next queue number is **209** and
the next ADR number is **0015**. **Item 72 is the item to take**, and it is now
genuinely unblocked rather than apparently so. Three things it should know
before it starts:

1. **The tree is bounded** at `DEEPEST_NESTING + DEEPEST_EXPRESSION`, so a
   recursive compiler is a legitimate shape — but 4096 levels of a compiler's
   frames will not fit in a caller's two mebibytes, so it needs **a stack of its
   own**, which is `Parser::program`'s argument made a second time and should be
   a `STACK_FOR_A_COMPILE` beside `STACK_FOR_A_PARSE` with its own measurement.
2. **Item 72 is far larger than one iteration and should be cut again on
   starting.** The shape I had worked out before this item interrupted it, in
   case it is useful: take the machine and the language that needs no call —
   values, scopes with their temporal dead zone, the operators, control flow —
   and cut *calls, `this` and closures* and *`try`/`catch`/`finally`* into items
   of their own. Two things fall out of having no callable object and both are
   correct rather than approximate: `ToPrimitive` on an object throws a
   `TypeError`, because nothing in the heap is callable and `OrdinaryToPrimitive`
   has nothing to call; and per-iteration loop bindings are unobservable, because
   nothing can capture one.
3. **ADR 0014 § 2 is the clause with the longest reach**: the value stack lives
   in a structure the collector walks rather than in Rust locals, which means a
   cell of its own in `object::Cell` and every push and pop going through
   `Heap::write`. `object::Objects` is the type to hold, and
   `Objects::define_named` is the rooting discipline written as one call.

Outside section D, **190** (the two-tone border styles) is still ready and still
small.

---

## Iteration 109 — queue item 72: the machine, and the language that needs no call

**A page's script is executed by this browser for the first time.** Item 72 is
ticked, cut on starting into three items — 209, 210 and 211 — and the cut is
the one iteration 108 wrote down at the end of its own entry: take *the machine
and the language that needs no call*, and leave calls, `try`/`catch`/`finally`
and the forms that take a value apart as items of their own. Scope rather than
depth (`LOOP.md` step 3): everything here is whole, and everything that is not
here is a **refusal that names its queue item** rather than something plausible.

**Eleven new files, one reason to change each.** `code.rs` is the instruction
set and the chunk; `compile.rs` turns a tree into one, with `compile/scope.rs`
(which name is which slot) and `compile/hoist.rs` (what a statement list
declares before it runs) beside it; `interpret.rs` is the loop and the
`Engine`; `realm.rs` is the global object and the global `let` bindings;
`convert.rs` is the abstract operations and `operate.rs` the operators written
in terms of them; `numeric.rs` is ADR 0013 § 8's rented arithmetic in the
specification's own spelling; `abrupt.rs` is how a run ends when it is not with
a value; and `object/slots.rs` is a list of values that lives in the heap.

**Three things in it are decisions rather than detail.**

1. **The value stack is a heap cell**, which is ADR 0014 § 2's last owed clause
   — *the interpreter's frames and its value stack live in structures the
   collector walks rather than in Rust locals*. That decides how every
   instruction is written: **operands are read where they lie and taken off
   only once the answer exists**, because an instruction that popped two values
   into Rust locals and then allocated a string is correct in every ordinary run
   and wrong under `Heap::stress`. The whole table therefore runs **twice**, the
   second time collecting at every allocation, and the two runs must agree.
2. **The interpreter never recurses and the compiler does.** A bytecode loop
   runs the deepest expression in the world on a taller stack of *values*, which
   is bounded and costs no frames — a tree walker would let a page choose how
   much of this process's stack it uses. The compiler walks the tree, so it runs
   on a stack of its own exactly as the parser does, and the number is
   **measured rather than chosen**: four thousand additions overflow eight
   mebibytes in a debug build, compile in sixteen, and the deepest tree the
   parser will build (250 brackets around 4090 links) compiles in thirty-two.
3. **Stopping is the embedder's.** `Stop` is an `Arc<AtomicBool>` checked at
   every **backward** jump — the only way a program runs for ever — because
   ADR 0013 § 5 gives this crate no clock and *when* is a person's judgement
   about a tab. The test asks from another thread and then runs the same engine
   again, because a tab that was stopped is not a tab that was lost.

**What runs.** Values; every operator with the conversions the specification
asks for in the order it asks for them; `var` on the global object and `let`
and `const` in the realm with real dead zones; blocks that shadow; objects,
their properties, computed keys, `__proto__`, `delete`, `in`; optional
chaining; templates; and all of the control flow — `if`, `while`, `do…while`,
`for`, `switch`, labels, `break`, `continue`, `throw`. **Completion values are
the specification's**, which is the detail most engines get roughly right:
`2; {}` is `2` and `2; if (true) {}` is `undefined`, because a block that
produced nothing leaves the previous value and an `if` does not.

**Four rules are worth reading twice**, each because the obvious implementation
is wrong in a way tests written afterwards would not catch.

- **`+` converts both sides before it asks whether either is a string**, and a
  template does `ToString` where `+` does `ToPrimitive` — so `` `${a}` `` asks
  an object for `toString` first and `"" + a` asks for `valueOf` first. That is
  why `Op::ToText` exists rather than reusing the addition.
- **`<` answers three things**, not two: less, not less, and *undefined*, which
  is what a `NaN` produces and is why `a >= b` is not `!(a < b)`.
- **A `Number` is not printed the way Rust prints one.** `1e21` and `1e-7` are
  the two bands where the language writes an exponent and Rust does not, and a
  page sees every one of them. The digits are rented (Rust's shortest
  round-trip) and the *spelling* is ours, which is exactly ADR 0013 § 8's line.
- **`ToPrimitive` on an object throws today and that is correct rather than
  missing.** An object here has no prototype until item 73, so it has no
  `valueOf` and no `toString` to call — which is the answer a real engine gives
  for `Object.create(null) + ""`. Where this engine *finds* something it would
  have to call, it says `Missing::ACall` rather than skipping it.

**The five ways a run ends are kept apart because different people answer
them**: a `TypeError` is the page's and its own `catch` will survive it (item
210); a full heap is the embedder's and stops the tab (ADR 0014 § 9); an
interrupt is the browser's; a lost reference is **ours** and is a bug in this
engine rather than in anybody's page (ADR 0014 § 3); and *this is not built
yet* is a fifth that most engines do not have and this one needs while it is
being written — a sentence a person reads, never something a page can catch.

**Two things landed here that were somebody else's on paper**, and both are
written into the items they came from. Two of item 205's early errors are in
the compiler because it cannot be correct without them: a name declared twice in
one block would otherwise take a second slot or put a live binding back in its
dead zone, and a `break` naming no open label has no instruction it could be.
And the global object has the **three value properties** — `undefined`, `NaN`,
`Infinity` — plus `globalThis`, with the specification's attributes, because
they are the only way to *write* three of the language's own values. The rest of
the builtins are item 73's and are absent rather than stubbed.

**Two defects were found by the tests and are worth naming**, because both were
invisible in the shape of the code. A keeping jump (`&&`, `||`, `??`) takes its
value off itself on the path that carries on, so the compiler emitting a `Pop`
after it popped twice — which is why `a ||= 5` was an engine bug rather than a
five. And `?.` needs the value kept on **both** paths, since the rest of the
chain reads from it and the end of the chain drops it, so it is an instruction
of its own (`SkipTheChain`) rather than a spelling of `??`'s.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1975 tests** (up from 1924 — a table of small programs with the value
each must produce, run twice; a hostile file for what a *run* can do; and unit
tests in each new file), no stubs, no `unsafe`, every boundary held, the licence
notice on every new file, and a `CHANGELOG.md` line. No layout assertion and no
reference render: this iteration positions nothing and paints nothing. One file
one responsibility: eleven new files, and the two splits worth naming are
`convert.rs` against `operate.rs` (what a value is worth as something else,
against what an operator does with two of them) and `compile/scope.rs` against
`compile/hoist.rs` (which name is which slot, against what is declared before
anything runs).

**`ROADMAP.md` moved, and it is not a tick.** *A bytecode compiler and an
interpreter* gains a `· Built:` clause naming the machine and the three
decisions in it, and its `· Owed:` is rewritten from *all of the code* to the
four items that remain — 209, 210, 211 and 73 — because that line is now
mostly built rather than untouched. `docs/features.md`'s own line is rewritten
in a reader's words for the same reason.

**What the next iteration should know.** The next queue number is **212** and
the next ADR number is **0015**. **Item 209 is the item to take** — calls,
`this` and closures — and it is the largest of the three cuts; item 210
(`try`/`catch`/`finally`) is smaller and depends on 73 for the `Error` object a
`catch` binds, and item 211 depends on 73 and 75. Four things 209 should know
before it starts:

1. **The frame layout is already there and has room for it.** Slot zero of the
   stack is the completion value, then the frame's own slots, then the operands;
   a call frame is a second base into the same list rather than a new structure.
   `bounds::VALUES_ON_THE_STACK` is already the bound that turns runaway
   recursion into the `RangeError` the language specifies rather than into a
   process that stops — nothing can reach it today, which is said in its own
   doc comment.
2. **`Missing::ACall` is the complete list of what waits for it.** Six places
   raise it — a getter, a setter, `ToPrimitive` finding a method, `instanceof`,
   and the realm's own get and set — and every one is a `TypeError` or a value
   the moment there is something to call.
3. **A closure needs a scope that outlives its frame**, which is the first thing
   in this engine that does. `object/slots.rs` is the shape to reach for (the
   realm's lexical bindings are already one), and `compile/scope.rs` is where a
   name would learn to resolve to *an enclosing function's* slot rather than to
   a frame's.
4. **`Op::Complete` and `Op::CompleteEmpty` are a script's completion value, not
   a function's return.** A `return` is a different instruction and a different
   thing; the compiler refuses one today with `What::AFunction`, which is the
   honest reason (there is nothing to return from) rather than a missing case.

Outside section D, **190** (the two-tone border styles) is still ready and still
small.

---

## Iteration 110 — queue item 209: calls, `this` and closures

**A page can write a function and this browser will run it.** Item 209 is
ticked, cut on starting into **five** items — 212, 213, 214, 215 and 216 — and
the cut is the one the item's own text asks for: it said it was *the largest of
the three*, and what it named is five separable pieces rather than one. What is
here is **calling, whole**: a function object with a `[[Call]]`, a frame per
call, the argument list, `return`, `this` and how an arrow does not have one,
closures over a scope that outlives the call, and the bound that turns runaway
recursion into a `RangeError`. Scope rather than depth (`LOOP.md` step 3):
everything not here is a **refusal that names its item**.

**All three closing conditions are met**, and each has its own evidence.
`tests/what_a_program_evaluates_to.rs` gains five tables — calling, a function
body's own names, closures, `this`, and an optional call — run twice like every
other, the second time collecting at every allocation.
`tests/what_a_closure_keeps.rs` is new and is the second condition in the form
item 71 demands, **counted rather than watched**: a closure read after its call
has returned *and* after a collection, an environment reclaimed when nothing can
reach it with `Heap::live` back to the number it started at, and a thousand
calls that keep nothing leaving nothing behind. And
`an_engine_that_is_hostile.rs` gains the third: five shapes of unbounded
recursion, each a `RangeError` rather than a process that stops, and an engine
that runs an ordinary program afterwards.

**The decision worth reading twice is that a name lives in one of two places,
and only one of them can be captured.** A function's parameters, its `var`s, its
body-level `let` and `const` and the functions it declares are **bindings of an
environment** — `object/environment.rs`, a cell in the heap with a parent link —
which a closure keeps alive after the call has returned. A **block's** `let` and
the compiler's own temporaries stay **frame slots** in the value stack and die
with the call. Two mechanisms rather than one, and the reason is a loop: the
language gives `for (let i = …)` a fresh `i` every pass, so a closure made in
one pass and one made in the next must not share a slot. Rather than share one
quietly, a function reading a **block's** binding is **refused by name** (item
216). That is what keeps item 72's note honest — *nothing can tell; a closure is
what would* — because the only thing that could tell is now the thing that is
refused.

**Three rules went in because the obvious implementation is wrong in a way a
test written afterwards would not catch.**

- **`this` is the callee's business, not the caller's.** The compiler pushes
  `undefined` for a plain call and the receiver for a method call, and
  `interpret/call.rs` then applies `OrdinaryCallBindThis` — strict code keeps
  what it was given, sloppy code turns `undefined` and `null` into the global
  object. So a caller never has to know which kind of function it is holding,
  which is what the specification's order is *for*. A primitive receiver in
  sloppy code says `Missing::AWrapperObject` rather than passing the primitive
  through, because `this.length` inside a sloppy method would otherwise be
  quietly wrong.
- **An arrow captures its `this` where it was written**, as a field on the
  function, rather than walking a chain for it at call time — and it captures
  it whether the body says `this` or not, because an arrow nested inside it may
  say it after the frame has gone.
- **A named function expression can see itself**, before anything has assigned
  it anywhere, so that binding is filled in by the *call* rather than by an
  instruction (`Chunk::own_slot`). Assigning to it is **silence** in sloppy code
  and a `TypeError` in strict code, which is a third answer to *what an
  assignment does* rather than a shade of the `const` one — hence
  `scope::Assignment` with three variants.

**The chunk stopped being the unit of compilation, and that is the structural
change.** A function is a chunk of its own, so a program is a `unit::Unit`: one
pool of strings and every chunk in it. A run interns that pool **once**, which
is what stops `a.b` written in ten functions being ten string cells. And a
function made by one script and called by the next brings its own unit with it,
so a run holds a small list of loaded programs rather than one — which is why
every case in `what_a_closure_keeps.rs` runs **two scripts in one engine**: it
is the only shape in which the callee's code, strings and keys are provably the
callee's rather than the caller's.

**The bound is two bounds, and the queue's expectation was half right.** Item
209 said `bounds::VALUES_ON_THE_STACK` *is already the bound that turns runaway
recursion into the `RangeError` the language specifies*. It does bound it — but
a call costs a frame, an environment cell and a root as well as its two values,
so a quarter of a million values is a hundred thousand frames and far more
memory than four mebibytes. A bound that under-counts what it bounds is a bound
in name only, so `bounds::CALLS_ON_THE_STACK` is the second, ten thousand, with
its reason written beside it.

**Nine new files, one reason to change each**: `unit.rs` (a whole program: the
strings and the chunks), `object/environment.rs` (a function's bindings, and the
one it was written inside), `object/function.rs` (an object that can also be
called), `compile/function.rs` (one function into a chunk of its own),
`interpret/frame.rs` (what a run is made of) and `interpret/call.rs` (making a
function, entering it, leaving it), plus the test file and two module
directories. The two splits worth naming are `compile/function.rs` against
`compile.rs` (compiling *a body* against compiling *a statement*) and
`interpret/call.rs` against `interpret.rs` (the calling convention against the
instruction loop).

**One defect was found by the tests and is worth naming**, because it was
invisible in the shape of the code: `Op::Text` read its constant out of the
**stack** rather than out of the run's list of constants, which every string
literal in the language goes through. It was one line and it failed ten tables
at once, which is the argument for the tables.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **1998 tests** (up from 1975), no stubs, no `unsafe`, every boundary
held, the licence notice on every new file, and a `CHANGELOG.md` line. No layout
assertion and no reference render: this iteration positions nothing and paints
nothing.

**`ROADMAP.md` moved, and it is not a tick.** *A bytecode compiler and an
interpreter* gains a second `· Built:` clause naming functions and the three
decisions in them, and its `· Owed:` is rewritten from four items to nine —
which is more items and less owed, because what was one line saying *calls,
`this` and closures* is now five lines each naming a thing somebody can pick up.
`docs/features.md`'s own line is rewritten in a reader's words for the same
reason.

**What the next iteration should know.** The next queue number is **217** and
the next ADR number is **0015**. Four things:

1. **Item 214 is the one to take next if the goal is fewest refusals per line of
   code.** A getter, a setter, `to_primitive` finding a `valueOf`, and a proxy
   trap are all one problem — re-entering the interpreter from inside an
   instruction — and all four already answer `Missing::ACall`. The shape to
   reach for is `Engine::enter`: it pushes a frame and returns to the loop, so
   an instruction that wants a call has to be able to *resume itself* when that
   frame returns. Nothing in the machine does that yet.
2. **Item 216 is the one to take next if the goal is fewest surprises.** A
   function reading a block's binding is ordinary code and is refused today.
   `compile/scope.rs` already distinguishes the two kinds of scope and answers
   `Where::Captured` for exactly this case, so the compiler knows where every
   one of them is; what is missing is `PushEnvironment`/`PopEnvironment`/
   `CopyEnvironment` and the unwinding that `break` and `continue` then need.
3. **Item 73 is now unblocking rather than blocked.** `Function.prototype`,
   `Object.prototype` and `Array` are what most of the remaining refusals wait
   on — item 212 needs the first, 213 and 215 need `Array`, and `({}) + ''`
   throwing rather than answering `"[object Object]"` is the same gap.
4. **A `Chunk` is no longer a program.** Anything that reaches for
   `compile::compile` gets a `Unit` now, and `Engine::run` takes an
   `Rc<Unit>` — because a function outlives the run that made it and its code
   has to outlive it too.

## Iteration 111 — queue item 214: a call that begins half way through

**A property can be a question rather than a thing.** Item 214 is ticked, all
four closing conditions met, with the proxy cut to a new item 217 and the reason
written into it: nothing can *make* a proxy until `new` (212) and the `Proxy`
constructor (73) exist, so the trap would be a mechanism no test could reach
from a script — and the item's own closing conditions never named it. That is
scope rather than depth (`LOOP.md` step 3).

What is here is every call the source does not spell: **a getter, a setter, and
the `valueOf` or `toString` that turns an object into a primitive.** Object
literals compile `get`/`set` for the first time — the compiler refused them by
name until today — the two halves of one name are **one property** rather than
two definitions of which the second wins, and every spelling of a key reaches
them.

**The decision worth reading twice is that the interpreter still does not
recurse.** `interpret.rs` has said so since item 72 — *it does not recurse, and
that is a property rather than an accident* — and a getter is precisely the
thing that tempts an engine to break it, because the call is wanted from inside
an instruction that is half way through. A nested `walk` would have been twenty
lines and would have handed a page the process's own stack through
`obj = { get a() { return obj.a; } }`. So an instruction **hands over** instead:
the frame joins the list every other call's frame is on, and the frame carries
one new field ([`frame::After`]) saying what the answer is *for* — the value the
instruction leaves behind, a value to drop, a `typeof` to take, or one step of a
conversion. Leaving a call is one `match` rather than four kinds of frame, and
the field holds no `Value`, so ADR 0014 § 2's list of where a live reference may
be is unchanged.

**Two shapes carry all of it, and only one needs anything remembered.** A
property access takes a known number of stack values and leaves one; a call
takes everything above its callee and leaves one in its place. So putting the
getter **where the access's answer belongs** makes the getter's `return` the end
of the access — nothing resumed, nothing recorded. A setter is the exception,
because `a.b = c` evaluates to `c` rather than to what the setter answered, so
the value is written into the answer's place first and the call laid out above
it with its answer dropped. A **conversion** is the one that genuinely resumes:
the primitive is written into the operand's own stack slot and the instruction
**runs again**. That is not a retry. Every instruction in this engine reads its
operands where they lie and takes them off only once the answer exists — the
discipline the collector forced on it — so the second run is the same
instruction on an operand that is now a primitive, which is exactly the
specification's next step. `a + b` with objects on both sides runs three times
and calls each side's `valueOf` once, in order.

**Neither can loop, and both are asserted.** A method that answers with an
object again carries on at the *next* name and there are two, so running out is
the `TypeError` the specification gives; an accessor that reads itself makes a
frame each time, which is `bounds::CALLS_ON_THE_STACK` and a `RangeError` a page
can catch. `an_engine_that_is_hostile.rs` gains six shapes of that — the item's
own wording (a getter calling something that reads the same property), the
direct one, a setter, one through a prototype where the receiver is the child
every time, a **conversion** rather than an access, and one that allocates per
frame so the heap is under pressure while the frames pile up.

**Two types keep the halves apart, and they are the change with the longest
reach.** `convert::Primitive` wraps a value that is **not an object** and is the
only way to make one, so `ToNumber`, `ToString` and `ToPropertyKey` cannot be
handed an object by mistake — before this, every one of them had an object arm
answering *not built yet* and the arm was reachable from a dozen operators.
And `operate::Applied::Wants` is how an operator says **which** operand it needs
converted and with which hint, rather than converting it — which keeps the order
`a > b` converts in (left first, which is what the specification's `LeftFirst`
flag is *for*) inside the one file that knows it, rather than copied into the
interpreter. `Missing::ACall` is gone from the engine entirely.

**The realm went with it.** A bare name can be an accessor too. No script can
make one until item 73, and an **embedder** can today — a `document` behind a
getter would otherwise be a name this engine could see and not read. So
`Resolved::Getter` and `Assigned::Setter` are answers rather than refusals, and
`tests/a_name_behind_an_accessor.rs` drives that path with the getter and the
setter written in the language rather than in Rust.

**One defect was found and fixed rather than cut**, because item 209 had turned
it into a lie a script could see: `instanceof` refused everything with *the
right-hand side is not callable*, on the stated grounds that nothing in the heap
was callable — and since item 209 things are. `1 instanceof f` now answers
`false`, which is what the specification answers before it reads anything off
`f`; a genuinely non-callable right-hand side is still that `TypeError`; and the
rest names item 212, because a function has no `prototype` until it has a
`[[Construct]]`.

**Four new files, one reason to change each**: `interpret/property.rs` (reading
and writing a property, either of which may be a call), `interpret/primitive.rs`
(the conversion state machine), and the two test files. `interpret/frame.rs`
gained the two types a frame now carries, which is the same responsibility it
already had — *what a run is made of*.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **2007 tests** (up from 1998 — nine new test functions, two of them
tables carrying about seventy programs between them), no stubs, no `unsafe`,
every boundary held, the licence notice on every new file, and a `CHANGELOG.md`
line. No layout
assertion and no reference render: this iteration positions nothing and paints
nothing.

**`ROADMAP.md` moved, and it is not a tick.** *A bytecode compiler and an
interpreter* gains a fourth `· Built:` clause naming the re-entry and the two
types, and its `· Owed:` drops the getter and gains the proxy as item 217.
`docs/features.md`'s own line is rewritten in a reader's words, including the
half that reads as unrelated and is the same problem: `"total: " + basket` now
asks `basket` for its `toString`.

**What the next iteration should know.** The next queue number is **218** and
the next ADR number is **0015**. Four things:

1. **Item 216 is the one to take next if the goal is fewest surprises.** A
   function reading a block's binding is ordinary code and is still refused.
   `compile/scope.rs` already answers `Where::Captured` for exactly that case,
   so the compiler knows where every one of them is; what is missing is
   `PushEnvironment`/`PopEnvironment`/`CopyEnvironment` and the unwinding
   `break` and `continue` then need.
2. **Item 73 is the one to take next if the goal is unblocking.**
   `Function.prototype`, `Object.prototype` and `Array` are what 212, 213, 215
   and 211 all wait on, and `({}) + ''` still throwing rather than answering
   `"[object Object]"` is the same gap — item 214's third closing condition is
   met through a prototype the *script* set, and 73 is what makes the literal
   form of it true.
3. **`After` is the extension point, not a special case.** Anything else that
   needs a call from inside an instruction — a `Symbol.toPrimitive`, a proxy
   trap (217), an iterator's `next` (211) — adds a variant there and a handler
   in `give_back`, and must not add a nested `walk`. The one rule to keep: an
   instruction that will run again **must** rewind its own program counter, and
   `Engine::convert_at` does it rather than each caller, because it is the half
   that is invisible when it is left out.
4. **A conversion writes into an operand's stack slot.** Anything that changes
   how instructions read their operands — an inline cache, a register-based
   compiler — has to keep peek-then-replace, or the second run of an instruction
   stops being the same instruction.

## Iteration 112 — queue item 216: a binding per pass

**A loop gives every pass its own names.** Item 216 is ticked, all four closing
conditions met and asserted in `crates/alo-js/tests/a_binding_per_pass.rs`. What
it closes is the last refusal in this engine that was about *ordinary* code: a
function reading a name a block around it declared — a `let` inside an `if`,
read by a callback written two lines later — compiled to nothing at all before
today, because a block's names were frame slots that died with the call.

**Taken over item 205, which is earlier in the file, and the reason is that 205
is now partly blocked rather than ready.** Its dependency (204) is done and item
72's compiler took two of its early errors on the way past; of what is left, a
`#a` no class declares needs classes (item 212, which waits on 73) and import
attributes want *"the thing that would act on it"*, which is the module loader
(item 77). So taking it would have meant cutting on starting and leaving the cut
smaller than this item. Everything outside section D is where it was: 157 and
158 need an interface to ask in, 187 is deferred with its reason written into
it, 169 must be *run* on Linux, 60 is HTTP/3, 197 waits on properties
`alo-style` does not have, and 201 and 203 wait on a renderer that can ask for a
subresource.

**The shape is the specification's own, and the alternative was the thing worth
refusing.** A cheaper answer exists — keep block names in frame slots and
promote only the ones a nested function captures — and it needs a pass over the
tree the compiler does not have, so the compiler would have had two answers to
*where does this name live* and they would eventually disagree. So every scope
that declares anything is an environment ([`Op::PushEnvironment`]), left on the
way out, and a `for (let …)` head is **copied** at each pass
(`CreatePerIterationEnvironment`, [`Op::CopyEnvironment`]).

**A copy is a sibling rather than a child**, and that is the sentence the whole
change turns on. It has the *same parent*, so every `hops` the compiler counted
still means what it meant and no instruction has to be recompiled or adjusted
when a pass copies — which is what makes per-iteration bindings a run-time fact
with no second instruction set behind it. A child would have left each pass able
to see the one before it through one more hop.

**Three rules keep it small enough to be right.** A scope that declares nothing
gets **no environment**, so a hop is counted by asking a scope rather than by
counting levels — otherwise every empty block in a program would be a cell
nothing could look a name up in, and a link every name past it had to walk. A
**`const` head is not copied**, which is the specification's own rule rather
than an optimisation: a `const` cannot be assigned to, so a copy could differ
from the original only by existing. And **leaving is the jump's own business** —
a `break` out of three blocks emits three pops, because the blocks it skips will
never reach their own — which is why `leave` now finds what it is leaving
*before* it emits anything, rather than emitting the jump first.

**`Where::Local` is gone entirely, and that is the change with the longest
reach.** A block's names were the only thing a script could name that lived in a
frame slot; they are bindings now, so what is left of a slot is the compiler's
own temporaries — a `switch`'s discriminant, the old value of an `a.b++`, the
object under an `a?.b()` — every one of which is written before it is read on
every path. So `Op::Store` and `Op::Uninitialize` had no emitter left and are
gone, `Chunk` no longer records a name for a slot, and reading an empty slot is
`Internal::StackIsWrong` rather than a dead-zone `ReferenceError` no program can
reach. One kind of name, one kind of dead zone, one place a message comes from.

**Two doctored runs rather than reasoning about them.** With the per-pass copy
removed, two tests fail; with the unwinding pops removed, two fail. The second
doctoring is the one that earned its keep: it found that a test was passing by a
**coincidence of layout** — the loop's `i` and the block's `seen` were each
binding zero of their own environment and held the same number, so a `continue`
that left nothing read and wrote the wrong cell and still answered correctly. It
declares a name in front of the one it reads now.

**One new file, one reason to change**: `interpret/environment.rs` — which
environment is in force, and the three ways that changes. `environment_at` and
`environment_of` moved into it out of `interpret/call.rs`, which is *making a
function, entering it, leaving it* and had grown a second subject.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **2019 tests** (up from 2007 — eight new integration cases and four new
unit tests), no stubs, no `unsafe`, every boundary held, the licence notice on
every new file, and a `CHANGELOG.md` line. No layout assertion and no reference
render: this iteration positions nothing and paints nothing.

**`ROADMAP.md` moved, and it is not a tick.** *A bytecode compiler and an
interpreter* gains a fifth `· Built:` clause — a binding per pass, the copy as a
sibling, the empty block that is not a hop, and the pops a jump emits — and its
`· Owed:` drops the captured block binding. One sentence in the item 209 clause
was corrected rather than left standing: it said a block's bindings *live in the
frame that dies with the call*, which stopped being true today.
`docs/features.md`'s own line names the ten buttons that each know which row
they are on, because that is the form a person has met this bug in.

**What the next iteration should know.** The next queue number is **218** and
the next ADR number is **0015**. Four things:

1. **Item 73 is the one to take next.** It is now the only thing section D waits
   on that nothing else waits on: 212, 213, 215, 217, 211 and half of 210 each
   name it, and `({}) + ''` still throws rather than answering
   `"[object Object]"`. Item 216 was the last item that could be built without
   it.
2. **Every declaring scope allocates, and nothing has measured it.** A block
   with a `let` costs a cell each time it is entered and a `let` head costs one
   per pass, which is what the specification asks for and what every engine
   optimises away later with escape analysis. `LOOP.md` says a speed claim is
   measured on hardware or not made, so nothing is claimed — but the test that
   runs a thousand passes and counts the heap back to its baseline is the one to
   keep whichever way that goes.
3. **A frame slot is a temporary now.** Anything that gives a script a frame
   slot again — a register allocator, a fast path for a block nothing captures —
   has to put back the name a slot carries and the dead-zone message that goes
   with it, both of which came out today.
4. **`Frame::environments` is the balance check.** A pop with nothing to pop is
   the compiler and the interpreter disagreeing, and it says so. Anything that
   adds a new way out of a block — `try`/`finally` is item 210 and is exactly
   that — must emit its pops on every path or that counter will say so at run
   time rather than silently reading the wrong cell.

---

## Iteration 113 — queue item 218: a builtin is a function this engine wrote

**`"" + {}` answers `"[object Object]"`.** Item 218 is ticked, all four of its
closing conditions met and asserted in
`crates/alo-js/tests/what_a_builtin_answers.rs`. Until today an object in this
engine had nothing behind it at all — no prototype, so no `toString` to *find* —
and the commonest thing a page does with a value it is unsure of ended the
script.

**Item 218 is a cut from item 73, made on starting, and the cut is the whole
judgement of this iteration.** Item 73 is *"the ECMAScript builtins, in the
order real pages need them"* and it names **no closing condition**, which by
`LOOP.md`'s own rule makes it not ready to build as written. `LOOP.md` also says
what to do about an item larger than one iteration: **cut its scope, never its
depth, and write the cut into the queue**. So what was taken is the piece
everything else in that item needs and the piece a page needs first — the
**mechanism** for a builtin at all, and the two objects that are not a library
but are what an object and a function *are*. Items 219 and 220 are the cuts, and
item 73 now says in its own text that it should be cut again next time it is
taken.

**The shape is one cell rather than two, and that is the sentence the change
turns on.** A native function is a [`Cell::Function`] like any other; what
differs is [`object::Code`], which says whether the body is a chunk this engine
compiled or a piece of Rust this engine wrote. So `typeof` needed no case,
`IsCallable` needed no change, and — the part that matters most — a builtin is
found and called wherever a script's function is: as a getter, as a setter, and
as the `toString` a `+` reaches for. That last one works because `Op::Return`'s
tail became `Engine::finish_call` and a native calls the same function, rather
than a second path that would have had to learn `After::Convert` separately.

**Three rules keep it small enough to be right.** A native is a **function
pointer rather than a boxed closure**: everything a builtin could capture is
either a reference the collector must walk — which a boxed closure hides from it
— or the realm it is reached through, so a native holds no edge at all and
tracing one is nothing. It is handed **no interpreter**: the heap, its `this`,
its arguments and a source offset, which is the bound that makes a native call
need no frame and cannot recurse, and is why `call`, `apply` and a `ToPrimitive`
on an argument are refused **by name** rather than quietly allowed. And a
builtin is **strict code**, so its `this` is what the caller wrote and
`OrdinaryCallBindThis` does not run: a bare `toString()` is `"[object
Undefined]"` here as it is in every other engine, which is asserted rather than
assumed.

**Three doctored runs, and the third is the one that earned its keep.** Removing
the `Function.prototype.toString` refusal fails a test — it answers `"[object
Function]"`, a sentence no engine produces. Defining a builtin method as
enumerable fails another. But removing the **scope that roots an interned name
between interning it and defining the property that owns it** failed *nothing*,
and the reason is a gap in the suite rather than in the code: every other file
turns `Heap::stress` on **after** `Engine::new` has already built the realm, so
nothing anywhere covered building the intrinsics. `builtin.rs` now has a test
that builds them with the collector firing at every allocation, and that test
fails without the holds. **Anything that adds an intrinsic must be covered by
it** — the roots are otherwise unchecked.

**One thing the tests found rather than confirmed**: `a.__proto__ = null;
a.__proto__` is `undefined`, not `null`. The accessor lives on the prototype
that was just cut away, so the name is no longer there to read. That is what
every engine answers, and I had written `null` in the table.

**Two files, one reason to change each**: `object/native.rs` (what a builtin's
body is, and what it is given) and `builtin.rs` with `builtin/object_prototype.rs`
and `builtin/function_prototype.rs` (which objects a realm owns, and what is on
each of the two). `object::Objects::reaches` became public rather than a second
chain walk being written for `isPrototypeOf`: it is the same question a
prototype assignment asks to refuse a cycle, so the bound on a chain an
embedder's object describes for itself is stated once.

**Three clippy `#[expect]`s were added and each is the lint being wrong rather
than the gate being weakened**: `unnecessary_wraps` on the three builtin bodies
that cannot fail. Their signature is `native::Body`, which every builtin shares;
narrowing one of them is not a thing that could compile.

**The gate.** `scripts/gate.sh` green: fmt, clippy zero warnings and zero
errors, **2035 tests** (up from 2019 — ten new integration cases in a file of
their own, and six new unit tests), no stubs, no `unsafe`, every boundary held,
the licence notice on every new file, and a `CHANGELOG.md` line. No layout
assertion and no reference render: this iteration positions nothing and paints
nothing.

**`ROADMAP.md` moved, and it is not a tick.** *The standard library: the
ECMAScript builtins* gains its first `· Built:` clause and an `· Owed:` naming
what is left of item 73. One sentence on the interpreter's line was corrected
rather than left standing: its `· Owed:` listed *the builtins that make a `{}`
have a `toString` of its own (73)*, which is what this iteration built.
`docs/features.md`'s own line says the same in a page author's words.

**What the next iteration should know.** The next queue number is **221** and
the next ADR number is **0015**. Four things:

1. **Item 219 is the one to take next.** It is now what most of section D waits
   on through item 73: `Array.prototype` methods, `Object.keys`, a
   `toLocaleString` — every builtin that is not a pure function of its
   arguments needs a native that can ask for a call and be re-entered with the
   answer. The interpreter already has the half that lays a call out from a
   place an instruction chooses (`Engine::begin_call` and `After`); what is
   missing is a way for a *native* to use it, which is a native that can
   suspend and is a design rather than a chore.
2. **A native cannot re-enter the script, and nothing checks that it does not
   try.** The bound is the type — `native::Call` has no engine in it — so it is
   enforced by there being nothing to call *with*, which is the strongest kind
   of bound and also the kind that silently disappears the day somebody widens
   the struct. Item 219 is where that widening happens, deliberately.
3. **Every `{}` and every function now allocates with a prototype**, so the
   realm is reached on a path that used to touch nothing. Both go through
   `Realm::intrinsics()`, which is two root lookups per allocation. Nothing has
   measured it and `LOOP.md` says a speed claim is measured on hardware or not
   made, so nothing is claimed.
4. **`Realm::new` returns `Escape` rather than `Refused` now**, because making
   the intrinsics can fault as well as fill the heap. Anything that makes a
   realm outside `Engine::new` gets both errors rather than one.

---

## Iteration 114 — roadmap audit and supervisor continuation

The user requested analysis of the remaining roadmap and a loop to finish it.
`docs/autonomy/REMAINING.md` records the inventory: stage 1's gate is met;
stage 2 has 84 open queue items, stage 3 has 8, and stage 4 has 9. These are
scope counts, not estimates. Item 219 is the journal's recommended continuation
and already has substantial uncommitted JavaScript work in this checkout.

The existing supervisor is retained. It now preserves interrupted files,
refuses a dirty checkout, acquires its lock atomically before the gate, stops
on failed workers (including `--once`), and checks committed/journalled progress
and the full gate after successful workers. Local commits are separated from
publication. The worker contract and ADR 0006's amendment record the changes.

Validation: shell syntax, supervisor self-tests, isolated fake-worker regression
tests (including timeout preservation), and `git diff --check` pass. The
workspace baseline passed formatting and clippy. Its pre-existing new
`what_a_builtin_asks_for` executable was still consuming a CPU after more than
three minutes; this audit terminated that test process. The full gate is
therefore not certified. The unfinished test is
`a_builtin_that_calls_itself_for_ever_is_a_range_error`; the other five tests
in that executable passed. No existing JavaScript changes were edited or removed,
no worker was launched, and nothing was committed or published by this audit.

No roadmap feature line is ticked: this is execution infrastructure, not a
browser capability. The audit is linked from the roadmap and README. Resume by
reconciling the item 219 changes and resolving or explaining the long-running
test, then passing the complete gate and committing the prepared work. Stage 2's
week-long use gate and stage 4's external adoption evidence remain real gates;
this request is not evidence that either has happened. Stage 3 still needs real
failing pages. Next queue number remains 221; next ADR number remains 0015.

---

## Iteration 115 — finish item 219 and the prepared build loop

The user authorized finishing and committing the dirty checkout. The pending
native continuation implementation is complete: `call`, `toLocaleString` and
object arguments to property-key methods can ask the interpreter for script
execution or conversion, then resume with their answers on the traced stack.

The baseline blocker was `Run::calls()`: its comment promised to count both
kinds of pending call, but the implementation returned only `frames.len()`.
It now includes `builtins.len()`. The runaway native-only and mixed-call tests
reach the existing recursion bound and report `RangeError`; no limit was
lowered and no test was disabled. All seven builtin continuation tests pass,
including collection stress and reuse of an engine after an exception through
suspended builtins. The targeted suite finishes in about 2.5 seconds on this
machine, which is test timing rather than a browser performance claim.

Item 219 is ticked with its closing evidence. `apply` needs an intermediate
argument list that survives accessor calls, so that remainder is explicitly
item 221 rather than an unimplemented method hidden behind the tick. Item 220
remains function metadata, source text and binding. The next unused queue
number is 222; the next ADR number remains 0015.

`ROADMAP.md` and `docs/features.md` now describe the native continuation
capability; the standard-library roadmap line remains partly built. The
remaining-work audit retires the resolved dirty-tree blocker. The supervisor
hardening from iteration 114 is included in this cleanup, along with its tests,
worker contract and ADR amendment.

Validation: `scripts/gate.sh` passes in full — formatting, clippy without
warnings, workspace tests, source and boundary checks, supervisor self-tests
and isolated worker regressions. `git diff --check` passes. No layout assertion
or reference image was added because this work changes neither geometry nor
paint. The commit is local; no push and no unattended worker launch.

---

## Iteration 116 — a Codex supervisor, independent of Claude login

The user requested our own loop after the Claude worker's OAuth session expired.
The existing supervisor now launches one `codex exec` per eligible queue item,
with the configured local Codex model and login. The prompt explicitly loads
the repository constitution and worker contract, requires a verified local
commit, and forbids pushing or spawning a competing supervisor.

The idle timer measures growth of each worker's captured event stream under
Git's `alo-loop-runs` directory. It no longer searches Claude's private session
files. Errors and events survive worker failure. Authentication has a preflight
check; dry runs and tests do not need a real account. Atomic locking, clean-tree
checks, independent gates, stage boundaries and failure preservation remain.

Validation: a real read-only Codex CLI invocation returned `READY`; the stored
login reports ChatGPT authentication. The full `scripts/gate.sh` passes,
including fake-Codex argument, login-failure, timeout, lock, dirty-tree and
progress regressions. Shell syntax and `git diff --check` pass. No browser
feature or queue item is claimed by this infrastructure change; the roadmap
still has 101 open queue items. Next unused queue number is 222 and next ADR
number is 0015. The JavaScript branch can continue at item 220, while selection
must still follow the first eligible queue item and its actual dependencies.

This change is committed locally before starting a detached supervisor under
the user's standing request to run the loop. No push is authorized or made.
Runtime status belongs in `docs/autonomy/loop.log` and the per-worker events.


---

## Iteration 117 — strict function headers before any statement runs

One eligible queue iteration, item 205, narrowed to plain function headers and
closed at that scope. The checkout was clean on entry. Read the constitution,
loop, latest journal, ADRs 0001, 0002 and 0013, feature contract and queue
prerequisites before selecting it.

Earlier open items remain real work: 157 and 158 need an interface; 187 has no
upload caller requiring its bounded wait and is unreachable from page Fetch;
60 had no transport design or closing contract and is now marked `needs design`
under LOOP step 2; 169 requires the Linux probes on Linux, while this host is
Darwin; 197 lacks the style-property implementations its dependency names
(their names in the inheritance list are not those implementations); 201 needs
80 and 83; 203 needs the browser process that owns both verbs and a loader.
Item 205 can use the functions and scopes already built by 209 and 216.

The compiler now rechecks plain parameter names and function binding names
under the strictness returned by the body's directive prologue. Strict reserved
words, `eval`, `arguments` and decoded escapes are refused before bindings or
instructions run. Strict and arrow duplicates are early errors rather than an
unsupported-form diagnostic. Header validation lives in `compile/parameters.rs`;
function chunk generation keeps its existing responsibility and scope table.

The new valid-code tests found a related parser defect: parentheses are omitted
from the AST, so `('use strict')` was being treated as a directive. Comparing
the statement and expression starts distinguishes a grouped string without
adding another AST representation. It also ends the prologue, so a later plain
string cannot reactivate it. A method's property name and legal lexical
shadowing remain valid. Parameter/body lexical collisions and labels crossing a
function boundary have named assertions over the existing scope machinery.

Eight integration tests cover the forbidden names across declarations,
expressions, arrows, methods and setters; duplicate and escaped names;
inherited strictness; valid shadowing and non-directives; rejection before side
effects and reuse of the same engine; every character-boundary truncation of
malformed headers; and a wide adversarial parameter list. Execution checks run
with and without collection at every allocation. These are early-error and
hostile-input regressions, not a claim that another real page now runs; existing
frozen-script tests remain part of the workspace suite. No geometry or paint
changes, so no new layout assertion or reference raster is applicable.

Item 222 retains the remaining parameter/function early errors, private-name
validation, named-expression binding separation and import attributes, with the
mechanisms they require named. The original queue's wording about a `let`
shadowing across a function boundary is clarified: legal shadowing is allowed;
a body-level lexical declaration colliding with a parameter is an error.
ROADMAP's parser line moves with Built/Owed clauses and stays open; features,
changelog and the remaining-work audit move in the same commit. There are still
101 open queue items because closing the narrowed item also adds its remainder.
Next unused queue number: 223. Next ADR: 0015. No stage gate is certified.

Validation: the targeted eight tests pass. The first full gate found test-helper
panic lints and string-building style in the new test, both corrected without
weakening a lint. An earlier gate overlapped the initial edit and reported the
then-missing changelog; its clippy and workspace tests passed. The final full
`scripts/gate.sh` passes: clean formatting, clippy with zero warnings/errors,
workspace tests, source notices, rental boundaries, no stubs/unsafe opt-outs,
and isolated supervisor regressions. `git diff --check` passes. All iteration
changes are included in one local commit. No push or supervisor launch was
performed; the mechanical gate's isolated supervisor regressions are tests only.

---

## Iteration 118 — explicit compliance in every worker prompt

The user reminded the loop to follow all rules. The active worker was stopped
before it made any file changes so the reminder can apply to its replacement,
not just future processes. The supervisor prompt now explicitly requires all
applicable instructions, the constitution, loop contract, roadmap, relevant
ADRs and feature contracts. The loop contract requires a compliance review and
recorded mechanical and manual validation before committing. Unmet obligations
must be reported and work preserved; no rule or stage gate may be bypassed.

Validation: the full `scripts/gate.sh` passes. The fake-worker regression now
also checks that the mandatory instruction and compliance-review clauses reach
the actual worker argument. `git diff --check` passes. This changes execution
instructions only, so no layout assertion, reference render or browser feature
tick applies. The prepared local commit is followed by restarting the loop
under the user's standing run instruction. No remote publication is performed.

---

## Iteration 119 — halt at the missing real-script binding case

The checkout was clean on entry at `6c5ae12`. Read `CLAUDE.md`, the complete
`docs/autonomy/LOOP.md` and `ROADMAP.md`, iteration 118, the queue and remaining
audit, ADRs 0001, 0002 and 0013, and the function-header and interpreter
contracts in `docs/features.md`. No AGENTS.md was found in this repository or
its ancestor directories. No sibling repository was modified.

Selection followed queue order. Items 157 and 158 still require an interface;
187 names no reachable upload caller; 60 is already `needs design`; 169 needs
Linux execution and this host reports Darwin; 197's CSS properties are still
only names in the style inheritance list. Items 201 and 203 retain their
subresource and browser-owner dependencies. Item 222 is the first candidate
with a reachable bounded piece: named-function-expression binding separation,
using the functions and environments already built. Its other forms retain
their parameter, suspension, class and loader dependencies.

The compiler's `function_inside` currently declares a named expression's own
name in the parameter scope and explicitly refuses a matching parameter. The
queue identifies that defect, but it does not name a frozen real script that
fails on it. The two corpus scripts, `alo-service-worker/script.js` and
`alo-theme-generator/script.mjs`, contain ordinary named declarations, not named
function expressions. Their provenance files and existing tests describe lexer
and parser coverage. They are not execution evidence for this binding scope.

LOOP's stage 2 clause 1 requires an item to be opened by a real failure and
closed by the same frozen case working; ADR 0013 § 9 requires real-script
evidence alongside small-program tables. That obligation is not established
here. The iteration therefore halts before implementation rather than treating
handwritten examples or the existing green parser corpus as that evidence.
Resume this scope by freezing a permitted real source with provenance and a
failing assertion for the binding defect, then implementing and verifying the
same case. Do not manufacture a page solely to satisfy the condition.

The queue and remaining audit record this blocker. ROADMAP's parser line
records the evidence still owed and stays open. The changelog records only the
halt; `docs/features.md` remains unchanged because no capability changed.
No item is ticked, no new item number is allocated, and no stage gate is
certified. Next unused queue number remains 223; next ADR remains 0015.
This is one halted iteration, not a completed item or a finished roadmap.

Compliance review: no source, tests, dependencies, bounds, gate scripts or
rules were changed. Each edited file retains its existing documentation
responsibility. No geometry, painting or verb semantics changed, so additional
numeric layout assertions and reference renders are not applicable. No push
or supervisor launch was performed; the gate's isolated supervisor regressions
are tests. The only unresolved item obligation is the missing trigger evidence
for this selected scope, in addition to the remainder's existing dependencies.

Validation: the full `scripts/gate.sh` exited 0. Formatting is clean; clippy
reports zero warnings and errors; all workspace tests pass; stub, unsafe,
licence, rental-boundary and coordinate checks pass; isolated supervisor
regressions pass. The captured log is `/tmp/alo-iteration-119-gate.log` (local,
not a committed artifact). `git diff --check` passes. These results verify the
documentation-only halt record, not completion of item 222. The record is
prepared for one local conventional commit.

LOOP HALT

---

## Iteration 120 — the halt was right, and here is what it was owed

Not a queue item. Iteration 119 halted rather than implement item 222's
named-expression binding without a frozen real script demonstrating the defect,
and this entry resolves that halt by recording what was found rather than by
deciding the requirement was inconvenient.

**The halt's judgement was tested, not taken on trust.** Two things were
checked directly.

The defect is real and reproduces exactly as 119 described it:
`var f = function me(me) { return me; }` is refused with *"'me' is this
function's own name and a parameter of it"*. That is valid JavaScript and the
engine rejects it, so the item names something true.

And the evidence genuinely does not exist. The obvious candidate was
`browserslist/parse.js` — MIT, four kilobytes, a real dependency actually
installed in alo's own web application, and carrying a named function
expression on its last line. It is **not** evidence: the compiler refuses it at
offset 80 with `NotBuiltYet { what: TakingAValueApart }`, a different missing
feature entirely. It never reaches the binding question.

That is the shape of the problem, and it is worth writing down: code old enough
to use named function expressions is generally also code that uses several
things this engine has not built, so it fails earlier for an unrelated reason.
A script that exercises *this* defect and nothing unbuilt is a narrow target,
and searching for one is its own piece of work rather than a preliminary to
item 222.

**So item 222 is marked blocked**, in the form the selection rule reads, with
the reason named. The next iteration passes over it instead of re-deriving
119's halt from scratch — which is what would otherwise happen, because nothing
about the repository had changed to make the answer different.

**The halt is retired by this entry rather than by deleting the marker.** The
supervisor's stop rule is that a marker is live only while no iteration entry
follows it, which is exactly so that resuming is an append and never an edit of
history. Iteration 119's record stands unchanged.

**And one defect in the supervisor itself is now written down.** `scripts/loop.sh`
is a child of the terminal that starts it and dies with it; the first long run
ended at iteration 45 for that reason, mid-item, leaving eleven uncommitted
files, and from outside it looked like the loop had stopped of its own accord.
`LOOP.md` now carries the detached invocation beside the plain one, because
"run this while you watch" and "run this overnight" are different instructions
and only the first was documented.

**What is not claimed.** No item is ticked, no item number allocated, no stage
gate certified. Item 222 is not done and is not closer to done; it is correctly
labelled. The engine defect it names is still there.

**The gate.** Documentation only — no source, tests, dependencies or gate
scripts changed.

---

## Iteration 121 — the supervisor could not have run at all

Not a queue item. Starting the loop after retiring the halt found that
`scripts/loop.sh` names a worker this machine does not have: the preflight's
`command -v codex` fails, so every iteration would have stopped at once.

Worse, `--dry-run` said otherwise. It printed a fixed line — *"would run: codex
exec …"* — naming a program it had never looked for, and reported the gate, the
stop marker, the open count, the guards and the log beside it. Every
precondition it checked was true and the one it did not check was the one that
stops a run dead. A dry run that reports readiness it never tested is worse
than no dry run, because it is believed.

**The worker is now chosen rather than assumed.** Codex where it is installed
and logged in, which keeps the owner's choice wherever that choice works;
Claude Code where Codex is not installed at all. `ALO_LOOP_WORKER` demands one
by name and is told plainly when it is absent.

**The distinction that took a second attempt.** The first version fell back
whenever Codex was unusable, including when it was installed but not logged in
— and the existing fixture caught it, because `check unauthenticated 2` asserts
that an expired login stops the run. The fixture was right. Absence and
misconfiguration look alike from the supervisor and are not alike: one is a
machine that never had Codex, the other is a login somebody let expire, and
quietly using a different worker for the second turns something to fix into a
silent change of who wrote the next commit. Only absence falls through now.

That first version also took exit 4 for "no worker", which the same fixture
already uses for "the gate is not met". A missing worker exits 8; 2 keeps the
things a person typed wrongly, an expired login among them.

**Tests.** `--self-test` gains a case asserting a dry run never says "would
run" about a worker it has not found, and its argument cases now read an
accepted argument as "not refused" rather than "exit 0", so a machine with no
worker reports that fact instead of nine red argument failures. `test-loop.sh`
gains two: a parked `codex` falls through to a `claude` stub and the stub
actually runs — asserted on the journal line only that stub writes, so the real
`claude` on PATH cannot pass the test for it — and a worker demanded by name
and absent refuses with 8 before taking the lock, leaving the tree clean.

**What is not claimed.** No queue item is touched. The loop has still not run
an iteration under this supervisor; it is now capable of starting one.


---

## Iteration 122 — item 207's decision: `BigInt` limbs rented, sizes ours

The checkout was clean on entry at `4e37b21`. Read `CLAUDE.md`, the complete
`docs/autonomy/LOOP.md`, `ROADMAP.md`, `REMAINING.md`, iterations 119–121, the
open queue items, ADR 0013 in full and ADR 0014's head, ADR 0009's licence
section and ADR 0010's *whose `unsafe` this is*, and the JavaScript section of
`docs/features.md`. No `AGENTS.md` exists in this repository. No sibling
repository was read or modified.

**Selection followed queue order and dependencies.** 157 and 158 need an
interface; 187 has no upload caller; 60 is `needs design`; 169 needs Linux and
this host is Darwin; 197's properties do not exist in `alo-style`; 201 waits on
80 and 83; 203 on a browser process holding both a pool and tabs; 222 is
blocked on a frozen script (iteration 120). **Item 207 is next**: it depends on
206, which is done, and it is marked **needs ADR** — which `LOOP.md` stage 2
§ 4 makes an iteration of its own, before any code depends on it.

**What was built: ADR 0015, accepted.** The limb arithmetic is rented from
`num-bigint` behind one file; every spelling, cross-type comparison and Number
conversion is ours; and the queue item's objection — a stranger's allocator in
the path of sizes a page chooses — is answered structurally rather than
waved at: a `BigInt` result's largest size is computable from its operands'
bit lengths before the call, so the adapter refuses past a ceiling before the
rented code runs. The ceiling must bound *work* too, because one rented call
is not an interrupt point, so it is to be measured against the slowest single
operation and written beside the constant in `bounds.rs` (ADR 0014 § 9 keeps
the number out of the ADR). The crate's three panics are preconditions with
hostile tests. Facts in the ADR were checked against the `num-bigint` 0.4.8
source in the local cargo registry, not recalled: MIT/Apache-2.0; Karatsuba
and Toom-3 multiplication; `unsafe` limited to x86-64 carry intrinsics, an
unchecked UTF-8 conversion in `to_str_radix` (which the ADR never calls —
digits come from `to_radix_le` and are spelled by us) and the optional `rand`
feature; `panic!` on division by zero, negative shift and pow overflow.

**What is not built, and why it stops here.** No dependency, code or test was
added. The code half of 207 needs a frozen real script that fails on a
`BigInt` (LOOP stage 2 clause 1, ADR 0013 § 9); `grep` over both corpus scripts
finds no `BigInt` literal or call. So 207 stays unticked and is marked
***Blocked: no frozen real script uses a `BigInt`*** in the form the selection
rule reads, as 222 is. No page was manufactured to lift it.

**Roadmap.** The object-model line's Owed clause now says the renting decision
is made (ADR 0015) and the value waits for its script. Not ticked.
`docs/features.md` is unchanged: no capability changed, and there is no
`BigInt` feature line to amend — the lexer line already says only that a
`BigInt` literal is *read*. `object/value.rs`'s doc comment still says item 207
"is where the decision is made", which remains accurate as a pointer and was
left alone rather than touching code in a documentation-only iteration.

**Compliance review.** Rules read and applied: the four laws (law 4 — the ADR
authorises no `unsafe`; `unsafe_code = "forbid"` is unchanged), *rent the
physics, build the engine*, *settled decisions live in `docs/decisions/`*
(ADR 0013 § 8 read before deciding, and the ADR cites it), LOOP's one item per
iteration, its decision-is-its-own-iteration rule, and its never-tick rule. No
gate, lint or test was changed. Each edited file keeps its single
documentation responsibility. Nothing positions, sizes or paints, so layout
assertions and reference renders do not apply. No push, no supervisor launch.

**Gate.** `scripts/gate.sh` exited 0: formatting clean, clippy silent, all
workspace tests pass, no stubs, `unsafe` forbidden everywhere, Exhibit A on
every source file, every rented crate behind its boundary, no coordinate verb,
supervisor stop rule holds. `git diff --check` passes. The log was kept in this
session's scratchpad, not committed. That verifies a documentation-only change;
it says nothing about a `BigInt` that does not exist yet.

**Unresolved obligations.** Item 207's code half (blocked as above). Next
unused queue number remains **223**; next ADR is **0016**. The next eligible
queue item after 207 in order is for the next iteration to determine — 212,
213, 217, 215, 210 and 211 all name dependencies on 73, which is itself the
item `REMAINING.md` says must be cut into bounded families when taken. This is
one iteration, not a finished queue or roadmap.

---

## Iteration 123 — queue item 212, cut to `[[Construct]]`: `new` makes things

The checkout was clean on entry at `976e2fc`. Read `CLAUDE.md`, the complete
`docs/autonomy/LOOP.md`, `ROADMAP.md`'s stage 1 and JavaScript sections,
`REMAINING.md`, iterations 114–122, the open queue items, ADR 0013 §§ 3 and 9,
and the JavaScript section of `docs/features.md`. No `AGENTS.md` exists in this
repository. No sibling repository was read or modified.

**Selection followed queue order and dependencies.** 157, 158, 187, 60, 169,
197, 201 and 203 keep the blockers iteration 122 recorded; 222 and 207 are
blocked on a frozen script. **212 is next**: it depends on 209 (done) and on 73
*for `Function.prototype`*, which item 218 built. Iterations 119 and 122 held
that a code item in this stage needs a frozen real script that fails on it, so
that was checked rather than assumed: compiling the frozen service worker
(`crates/alo-corpus/scripts/alo-service-worker/script.js`, alo's own `sw.js`)
refused at byte 1438 with *`new`, a class, `super` or a private name … queue
item 212*. That is the trigger, from a real page, for exactly the first step of
212's own order. The probe was a throwaway example file, deleted before any
change.

**What was built.** `new`: `Op::Construct`, `Chunk::constructs`,
`After::Construct` and `interpret/construct.rs` (`MakeConstructor` when a plain
`function` is made, `[[Construct]]` at `new`). A construction is a call with a
different landing — the instance goes in the `this` slot and the body is
entered by the same `enter_at` a call uses, so frames and both bounds are
shared. Arrows, methods, getters, setters and builtins are not constructors and
are a `TypeError` after the arguments are evaluated. `prototype` and
`constructor` carry the specification's attributes, asserted from script with
`hasOwnProperty`, `propertyIsEnumerable` and `delete`.

**What was cut.** Classes, `super`, `new.target` and private names to **item
223** (`What::AClass`); `instanceof` to **item 224**, because its refusal named
212 for a `prototype` that now exists and what remains is `@@hasInstance`
(item 73's well-known symbols) and `OrdinaryHasInstance`. 212 is ticked at the
narrowed scope with both cuts written into it. 102 items are open (212 closed,
two added). Next unused queue number **225**; next ADR **0016**.

**Evidence.** `crates/alo-js/tests/what_new_makes.rs`, thirteen tests, every
case run ordinarily and with the collector at every allocation and required to
agree — except two runaway recursions, which run ordinarily only, as
`an_engine_that_is_hostile.rs`'s do: under stress they took 47 s, quadratic in
ten thousand frames, and a fifty-deep nesting covers the same rooting under
stress instead. This is test timing on this machine, not a performance claim.
The frozen script now compiles past byte 1438 and is refused at byte 2847,
`let changedTypes = [];`, naming item 211; the test pins both bytes. **Its
second `new`, `new Response(…)` at byte 4686, is beyond that array**, so the
script does not show it compiling — a first draft of the docs said "past every
`new`", which was checked, found false and corrected in all three places.
Hostile input: non-constructors of every kind, a constructor that constructs
itself for ever (`RangeError`), every prefix cut of a construction program, and
a tower of twenty thousand `new`s (refused by the parser's bound). Two doctored
runs: without the scope holding the new `prototype` object eight tests fail
under stress; without the instance substitution five fail. Both restored.

**Roadmap.** The interpreter line gains a Built clause for `new` and its Owed
clause names 223 and 224 instead of 212; the standard-library line says a
builtin `[[Construct]]` is item 73's. Neither is ticked. `docs/features.md`'s
interpreter line, `CHANGELOG.md` and `REMAINING.md` move with it. Items 73 and
220 had sentences that 212 made false and are corrected.

**Compliance review.** Rules applied: law 1 (modern language only); law 3 — no
stub, no `todo!`, no `unwrap` outside tests, and what is not built refuses by
name (ADR 0013 § 3); law 4 — no `unsafe`; ADR 0013 § 9 and LOOP stage 2 § 1 —
opened by a frozen real script and closed against the same script; LOOP § 2 —
hostile tests and no panic; one item per iteration with the cut written into the
queue. One file, one responsibility: construction is its own file;
`call.rs` gained only the landing a `return` performs, which is leaving a call.
No gate, lint or test was weakened; four existing tests asserting the old
refusal text (two in `what_a_program_evaluates_to.rs`, and the unit tests in
`compile.rs` and `abrupt.rs`) were updated to the new truth, and
`an_engine_that_is_hostile.rs`'s list gained six `new` cases. Nothing positions, sizes or paints,
so layout assertions and reference renders do not apply.

**Gate.** `scripts/gate.sh` exited 0: formatting clean, clippy silent, all
workspace tests pass, nothing stubbed, `unsafe` forbidden, Exhibit A on every
file, every rented crate behind its boundary, no coordinate verb, the
supervisor stop rule holds, `CHANGELOG.md` changed. `git diff --check` passes.
The log was kept in this session's scratchpad, not committed.

**Unresolved obligations.** Items 223 and 224 as written. The service worker
cannot *run* — it needs arrays (211), `try` (210) and an embedder's `self` —
so "the same page working" is met at compile time for the `new` that opened the
item, and no more. The next eligible item is for the next iteration to
determine; 211 now has a real-script trigger but depends on 73 and 75. This is
one iteration, not a finished queue or roadmap.

---

## Iteration 124 — queue item 225, cut from 73 and 211: an array is an object whose `length` keeps up

The checkout was clean on entry at `ee49354`. Read `CLAUDE.md`, the complete
`docs/autonomy/LOOP.md`, `ROADMAP.md`'s JavaScript lines, `REMAINING.md`,
iterations 119–123, the open queue items in section D, ADR 0013 (all of it)
and ADR 0014 §§ 9–11, and the JavaScript section of `docs/features.md`. No
`AGENTS.md` exists in this repository. No sibling repository was read or
modified.

**Selection followed queue order and dependencies.** 157, 158, 187, 60, 169,
197, 201 and 203 keep the blockers iteration 122 recorded; 222 and 207 are
blocked on a frozen script. 223 depends only on 212 (done), but neither frozen
corpus script contains a `class`, `super`, `new.target` or a private name, so
it has no real-script trigger and was not taken — the rule iterations 119 and
120 established, applied rather than re-argued. 224, 213, 217, 215, 210, 211
and 221 each depend on 73. 220's dependencies are done, and neither script
reads a function's `name`, `length` or source text or calls `bind`, so it has
no trigger either. **73 is next**, its dependencies done, and the queue says to
cut it when taken. The cut was chosen by the real failure rather than by
taste: item 123 left the service worker refused at byte 2847,
`let changedTypes = [];`, naming 211 — and 211 says the thing that literal
needs first is item 73's array exotic object. So **item 225** is that exotic
object plus the literal without a spread, cut from both, written into the queue
with its own closing conditions, and ticked at that scope.

223 and 220 were **not** marked blocked in the queue: neither had been
selected-and-halted the way 222 and 207 were, and marking them is a queue
decision this iteration did not need to make. The next iteration should apply
the same trigger check to them.

**What was built.** `object/array.rs`: `Array`, an `Ordinary` plus `length`
held beside the table, implementing `Internal` with the specification's
`ArrayDefineOwnProperty` and `ArraySetLength` — an index at or past the length
grows it (refused when `length` is not writable), a smaller length deletes
existing indices highest first and stops one past an element that is not
configurable. `Cell::Array`, `Objects::array` and `Objects::as_array`;
`Array.prototype` as a third intrinsic, itself an array of length zero with no
methods; `"[object Array]"` from `Object.prototype.toString`; `Op::Array(n)`
and `Op::DefineIndex(i)`, compiled from a literal with holes left undefined
and a spread still refused as item 211. The interpreter converts a value
assigned to an array's writable `length` (`ToNumber`, then the exact-length
test) and throws `RangeError` when it is not one; an object there is refused
by name as **item 226**, because the specification converts it twice and the
assignment still answers the object.

**The change with the longest reach**: `Internal::own_property_mut` is gone.
`Objects::set` stored into an existing own property through a mutable borrow,
which would have let `a.length = 0` write a number and delete nothing — and
gave every embedder's exotic object the same hole. A store is now
`OrdinarySet`'s own `[[DefineOwnProperty]]` with the attributes the property
had. Because that makes a definition the way an ordinary store happens,
`Ordinary::define_own` now reports the replaced and stored values to the write
barrier (`Property::edges`), not only the key. `Property::write` and
`Properties::get_mut` had no caller left and were removed.

**Evidence.** `crates/alo-js/tests/what_an_array_is.rs`, seventeen tests, every
program run ordinarily and with the collector at every allocation and required
to agree, plus object-model tests for the rules no script can reach yet (a
non-configurable element stopping truncation, a non-writable length, a length
nobody converted, key order) and a stress-mode test that the length's name
survives. Hostile input: every prefix cut of an array program, twenty thousand
nested brackets (refused by the parser's bound), a fifty-thousand-element
literal and one of fifty thousand holes, a length of 2³²−1 shrunk to zero over
two elements (two deletions), `NaN`, `Infinity`, `-1`, `1.5`, `2**32`, strings
and `undefined` as lengths, and the largest index and one past it. The frozen
service worker now compiles past byte 2847 to byte 2853, the `try` on the next
line, which is item 210; the test pins both bytes.

**Doctored runs, five.** Without growth three tests fail; without truncation
two; with stores going round `define_own` five; without tracing the length's
name two. The fifth — removing a heap scope that held `"length"` between
interning it and allocating the array — failed nothing, and that was checked
rather than shrugged at: `Heap::allocate` runs its collection with the
incoming cell traced (`collect_with(Some(&cell))`), and the array traces the
name. The scope was redundant, so it was removed and the real reason written
beside `Objects::array`; a doc comment claiming the scope was needed had been
drafted and was corrected before commit. All doctored files restored and the
suite re-run green.

**Roadmap.** The interpreter line gains a Built clause for the array literal
and its Owed clause names 211 for spread, patterns and loops and 226 for an
object as a length; the standard-library line gains a Built clause for the
array as an object and `Array.prototype`, and its Owed clause says the
constructor, `Array.isArray` and every method remain item 73's. Neither line is
ticked. `docs/features.md`'s interpreter and standard-library lines,
`CHANGELOG.md`, `REMAINING.md`, and queue items 73 and 211 move with it.

**Compliance review.** Rules applied: law 1 (modern language only; no legacy
array behaviour); law 3 — no stub, no `todo!`, no `unwrap` outside tests, and
what is not built refuses by name (ADR 0013 § 3: `[].push` is absent, not
approximate); law 4 — no `unsafe`; ADR 0013 § 4 and LOOP stage 2 § 2 — every
length a script chooses is bounded by what exists, never by the number it
names; ADR 0014 §§ 2, 5, 11 — rooting checked under stress, the barrier hears
every stored edge, internal methods stay one trait; ADR 0013 § 9 and LOOP
stage 2 § 1 — opened by a frozen real script and closed against it at compile
time; one item per iteration with both cuts written into the queue. One file,
one responsibility: the array is its own file; `interpret.rs`'s `step` went
over clippy's length limit and `InitializeBinding`'s body moved to a method of
its own rather than silencing the lint; the intrinsics' fields were renamed
rather than allowing `struct_field_names`. No gate, lint or test was weakened.
Existing tests that asserted the old refusal (`[1]` naming 211 in
`what_a_program_evaluates_to.rs` and `compile.rs`, the frozen-script byte in
`what_new_makes.rs`) were changed to the new truth, and the test embedder in
`what_an_object_is.rs` lost the trait method that no longer exists. Nothing
positions, sizes or paints, so layout assertions and reference renders do not
apply.

**Gate.** `scripts/gate.sh` exited 0: formatting clean, clippy silent with
`-D warnings` across the workspace and all targets, all workspace tests pass,
nothing stubbed, `unsafe` forbidden, Exhibit A on every file, every rented
crate behind its boundary, no coordinate verb, the supervisor stop rule holds,
`CHANGELOG.md` changed. `git diff --check` passes. The log was kept in this
session's scratchpad, not committed.

**Unresolved obligations.** Item 226 as written. The service worker cannot
*run* — it needs `try` (210), `Object.values`/`Object.keys` (73), `concat` and
`includes` (73), promises (75) and an embedder's `self` — so "the same page
working" is met at compile time for the literal that opened the item, and no
more. The next refusal, `try` at 2853, names 210, which depends on 73 for the
`Error` objects a `catch` binds. Next unused queue number **227**; next ADR
**0016**. 103 items are open (102 before; 225 was added already closed and 226
added open). This is one iteration, not a finished queue or
roadmap.

---

## Iteration 125 — queue item 227: errors are things a page can make

`Error` and the six native errors exist. `new TypeError("bad")` carries its
message, an optional `{ cause }` is kept only when `options` has one,
`"" + error` reads `"TypeError: bad"`, and `TypeError("bad")` without `new`
makes the same object. They are the first built-in functions that work with
`new`, and they are what `try`/`catch` was waiting for, since a caught error
has to be one of these.

Cut, as 225 was: `Error.prototype.toString` of a `message` behind a call is
**item 228**, and `AggregateError` is **item 229**.

**This entry is written after the fact, and by the supervisor rather than by
the worker that did the work.** That is unusual enough to say plainly.

The worker built the whole item — eighteen tests, the implementation, the
queue, the changelog, the features list and the roadmap — and was killed by
the supervisor's idle guard before it wrote this entry. Its changes were
preserved, which is what the guard is built to do. Nothing here was rebuilt or
reconstructed: the tree is the worker's, unmodified, and this entry and the
commit are the only things added.

**Why it was killed, which is a supervisor defect and not a worker one.**
The guard presumes a worker that writes to its transcript as it goes: a hung
one stops writing while an honest long one keeps writing. That is true of
`codex exec --json`. It is false of `claude -p`, which buffers everything and
writes once at the end — so its transcript is zero bytes from the first second
to the last, and the guard reads every iteration as silent from the start.

It was therefore not a hang detector at all for this worker. It was a
twenty-minute wall clock on every iteration. The three iterations before this
one took seven, nineteen and eighteen minutes and survived by a minute or two
without anybody noticing how close they were. This one needed longer.

The guard was right to preserve the work and right about what it saw. It was
asking a question the worker had no way to answer.

**Verified before committing.** `scripts/gate.sh` green on the preserved tree,
and the tests read rather than counted: they cover the refusal paths, the order
in which a message is converted before a cause is read, a getter that throws
part-way through construction, an error made for ever becoming a `RangeError`
rather than a process that stops, errors with causes surviving the collector,
and a page replacing the global leaving the intrinsic alone. That is an item
finished, not an item written.

**Next.** The supervisor's worker invocation is wrong and is fixed in its own
change, not this one.

---

## Iteration 126 — the guard was asking a question the worker could not answer

Not a queue item. The supervisor defect that killed iteration 125.

`scripts/loop.sh` watches each worker's transcript and kills one that has
written nothing for `IDLE_KILL_MIN` minutes. The reasoning is sound and is
written in the file: a hung worker stops writing while an honest long one
keeps writing. It is true of `codex exec --json`, which the guard was built
around.

It is false of `claude -p`. That mode buffers the whole response and writes it
once, at the end. The transcript is zero bytes from the first second to the
last, so the guard saw silence from the start of every iteration and killed on
its timer. The evidence is in the run directory: iterations 1 to 3 wrote their
transcripts at 14:49, 15:09 and 15:28 — each about a minute before finishing —
and iteration 4's file is still zero bytes.

So the guard was not detecting hangs for this worker. It was a twenty-minute
wall clock on all work, and nothing reported it as one, because an iteration
that finishes inside the window looks exactly like one the guard approves of.
Seven, nineteen and eighteen minutes: two of those cleared it by a minute.

**The fix is the invocation, not the guard.** The worker now runs with
`--output-format stream-json --verbose`, which emits an event per tool call
and result. Measured rather than assumed: a probe writing over a run of two
eight-second commands grew its output steadily throughout instead of arriving
in one piece at the end.

**Two tests, because the flag is easy to drop and the symptom is invisible.**
The fixture's Claude stub now refuses any invocation without the streaming
flags, so removing them fails the suite rather than quietly restoring a wall
clock. And a new `streaming` mode runs a worker that writes all the way
through an iteration longer than the idle window and asserts it is not killed
— the opposite case to `timeout`, which asserts that one which stops writing
is. The fixture's clock advances in finer steps for that mode, so the window
can only be crossed by a worker that has genuinely gone quiet rather than by
the clock outrunning it.

**What this does not fix.** A single tool call longer than the idle window is
still silence, because the stream carries a tool's result and not its
progress. The gate is comfortably inside twenty minutes today. If it ever is
not, the guard will be right about what it sees and wrong about what it means,
exactly as it was here.

---

## Iteration 127 — the half of the guard that was still guessing

Not a queue item. Iteration 126 made the worker stream so the idle guard could
see it between tool calls, and said plainly what that did not fix: *a single
tool call longer than the idle window is still silence, because the stream
carries a tool's result and not its progress*. This closes that.

**A worker is idle only when it is doing nothing by both measures** — writing
nothing to its transcript, and burning no processor time across its whole
process tree. Either alone is wrong in a different direction. Bytes cannot see
inside one long tool call, so a fifteen-minute compile reads as a deadlock.
Processor time cannot see a worker waiting on a network call that will never
be answered, which sits there costing nothing and looking busy.

**The test, and the proof that it is a test.** A new `busy` mode writes
nothing at all and burns processor time in pure shell arithmetic, past the
idle window, and must survive — the inside of one long tool call. Its
counterpart is the existing `timeout` mode, which sleeps: no bytes and no
processor time, and it is still killed.

That pair was run against the previous supervisor before this one was
committed. `busy` is killed there — *"silent for 1 minutes"*, exit 124, nothing
committed — and passes here. The test fails without the change, which is the
only thing that makes it a test rather than a description.

**The cost, which is chosen rather than overlooked.** A worker spinning in a
loop burns processor time and is therefore never idle by this measure. The
idle guard will not stop it. `CEILING_MIN` is what bounds that case, and it is
the right instrument for it: a runaway is a *duration* problem, and the thing
this guard exists to avoid is killing honest long work on a timer.

**One iteration was interrupted to do this.** The supervisor was mid-way into
`try`/`catch` compilation — one file modified, one added, no tests, no tick —
which is not a completable unit, so it was stashed rather than finished or
thrown away, and the next iteration will take the item properly.

---

## Iteration 128 — a tree's processor time goes down as well as up

Not a queue item. A defect in iteration 127, found by watching the thing it
had just built instead of trusting it.

Eight seconds of sampling a live worker tree returned **16 hundredths of a
processor second, then 13**. The total fell. `tree_cpu` sums only processes
that are alive, so when a child exits its time leaves the total with it — and
the gate spawns and reaps children constantly.

The rule said a worker was working when its processor time had *grown*. So a
poll in which a busy child finished read as a worker doing nothing.

**The rule is now "changed", not "grown".** A total that falls means a child
exited, and a child exiting is work finishing. Only a frozen set of processes
burning a frozen amount is doing nothing.

**What this did and did not risk.** It could not have caused a wrongful kill.
A fall resets the baseline lower, so the next poll sees growth again and the
window clears; at worst a genuine hang was noticed one poll late. The honest
description is a rule that was wrong about what it was measuring while
arriving at acceptable answers — which is the kind of thing that stays wrong
until it meets a case it cannot absorb.

**Test coverage, stated accurately.** The `busy` and `timeout` pair still
covers the two directions that matter: no bytes with processor time survives,
no bytes without it is killed. The falling-total case is **not** covered by a
fixture case. I could not construct one that isolates a decrease-only window
without depending on process timings fine enough to be flaky, and a flaky
check in the gate is worse than an honest gap. This change rests on a
measurement on a real tree and on the reasoning above, and that is recorded
here rather than implied by a green suite.

**Cost of the interruption.** The loop was thirty seconds into an iteration
with a clean tree; nothing was lost.

---

## Iteration 129 — the two gaps, closed and tested

Not a queue item. Iteration 128 named two things it had not covered. Both are
closed here, and each is covered by a check that fails without its change.

**The falling total now has a test, and it is deterministic.** 128 said a
fixture case could not isolate a decrease-only window without depending on
process timings fine enough to be flaky. That was true of *real* processes
and I stopped a step too early: the fixture can script `ps` instead. A
`shrinking` mode reports a total that only ever falls, and the worker must
survive it.

The division of labour between the two checks is the point. `busy` reads
processor time off a live tree, so the *reading* is exercised against real
processes. `shrinking` controls what is read, so the *interpretation* is
exercised without a race. Neither covers the other, and a stub is the right
instrument for the second because the question is what the guard concludes
from a number, not where the number comes from.

Run against the rule 128 replaced — `-gt` rather than `-ne` — `busy` passes
and `shrinking` is killed: *"silent and burning no processor time for 1
minutes"*. The check fails without the change.

**The spinning worker now has a bound of its own.** Processor time counting as
work is exactly what lets a long compile finish, and exactly why a worker
going round in circles never looks idle. Leaving `CEILING_MIN` — four hours —
as the only answer was accepting four hours of heat for nothing.

`SILENT_KILL_MIN`, an hour by default, asks the slower question: not *is it
doing anything* but *has it produced anything*. An honest tool call answers in
minutes. The guard now has three bounds and they ask three different
questions, which is why none of them replaces another:

| bound | question | default |
| --- | --- | --- |
| `IDLE_KILL_MIN` | is it doing anything | 20m |
| `SILENT_KILL_MIN` | has it produced anything | 60m |
| `CEILING_MIN` | has this gone on long enough | 240m |

A silence bound below the idle bound is refused outright, because it would
retire the idle guard without saying so — everything the shorter bound
catches, it would catch first. That refusal has its own check.

Run against the supervisor without this bound, the `spinning` worker is killed
by the ceiling instead and the suite fails: *"producing nothing"* appears
nowhere in its output.

**Eighteen fixture checks, from fifteen.** The loop was three minutes into an
iteration with a clean tree when it was stopped to apply this; nothing was
lost.


---

## Iteration 130 — queue item 210: a page can catch an error

The checkout was clean on entry at `7705c99`, with one stash
(`stash@{0}`, "iteration interrupted 16:06 — partial try-statement work,
superseded") left by iteration 127's interruption. Read `CLAUDE.md`, the
complete `docs/autonomy/LOOP.md`, `ROADMAP.md`'s JavaScript lines,
`REMAINING.md`, iterations 123–129, the open queue items in section D, ADR
0013 (all of it) and ADR 0014 § 2, and the JavaScript lines of
`docs/features.md`. No `AGENTS.md` exists in this repository. No sibling
repository was read or modified.

**Selection followed queue order and dependencies.** 157, 158, 187, 60, 169,
197, 201 and 203 keep their recorded blockers; 222 and 207 are blocked on a
frozen script; 223, 226 and 228 have no real-script trigger or an unbuilt
dependency (221), and 224, 229, 213, 217 and 215 depend on 73 or 211/75. **210
is next**: it depends on 72 and 227, both done, and the frozen service worker
was refused at byte 2853 naming it — the trigger, from a real page.

**The stash was read and not applied.** It held a sketch of the compiler half
(a handler table in the chunk and `compile/try_statement.rs`) and nothing of
the interpreter. Its design was taken as a starting point and rewritten in
place; it missed suspending the `finally` stack across a nested function, so a
`return` in a function written inside a `try` would have routed through the
outer `finally`. That case now has a test. **The stash is left where it is** —
it is preserved work, and dropping it is a person's call.

**What was built.**
`code.rs`: `Handler` (start, end, landing, environments) kept per chunk, and
`Op::Completion`. `compile/try_statement.rs`: the statement, the `catch`
parameter as a scope and an environment of its own, and the `finally` that
carries a way out across its block in two frame slots and dispatches on it at
the end; `break`, `continue` and `return` go through `exit`, which routes into
a `finally` when one is in the way. `interpret/catch.rs`: on an escape from an
instruction or a builtin, the loop looks for a handler around each frame's
running instruction (`Frame::now`, new), takes down the calls, builtins and
blocks above it, cuts the stack and lands the value — an engine error made
into an instance of its constructor with its message, after the cut.
`What::ACatch` is gone; nothing refuses a `try` any more.

**Two changes outside `try`, both forced by it.** A `RangeError` is now
catchable, and `enter_at` used to root a call's environment *before* the
value-bound check, so each refused call would have kept one for the engine's
life; the check moved ahead of the allocation. And a recursion that catches its
own `RangeError` can run for ever without a backward jump
(`function f() { try { f() } finally { f() } }`), so `Stop` is read on every
call into a script function too. The module comment and ADR 0013 § 4's
"points it defines" are what that serves.

**Evidence.** `crates/alo-js/tests/what_a_catch_catches.rs`, twenty-one tests,
every program run ordinarily and with the collector at every allocation and
required to agree, except three runaway recursions which run ordinarily only:
under stress ten thousand frames cost quadratic time (iteration 123 measured
47 s), and the rooting they exercise is the shallow throw across frames that
runs both ways. One test per way out of a `try`, each counting its `finally`;
what a `catch` binds; engine errors of three kinds caught as instances
(`constructor`, `__proto__`, own non-enumerable `message`, `[object Error]`);
throws from a getter, a setter, a `valueOf`, a `toString`, `new`, and from
inside a waiting builtin; blocks left; nested `finally`s innermost first; a
`finally` that leaves its own way winning; completion values; the stop switch
on a loop inside a `try` and on the doubling recursion (from another thread);
live cells after three catches of ten thousand frames equal; and the early
errors. Hostile input: every prefix cut of a program using every form, twenty
thousand nested `try`s (refused by the parser's bound), two hundred nested on
each way out, and two thousand in a row. The frozen script compiles past byte
2853 to byte 2922, `for (const account of …)`, item 211; the test pins both.

**Doctored runs, five**, each restored and the suite re-run green: frames
keeping their roots fail the live-cell test (`[20549, 41027, 61505]`);
builtins left waiting fail one test; no stop check on a call hangs the stop
test (the defect it describes; the process was killed by hand). Blocks left
standing **failed nothing at first** — every binding the tests read held the
same value in the wrong environment as in the right one, the coincidence
queue item 216's own doctored run warned about — so three cases with a different letter in
every binding were added, and the doctored build now answers `"yy"` for
`"kk"`. Searching by `pc - 1` instead of `Frame::now` fails nothing, and the
reason is structural: a rewinding instruction always has its operand loads
before it inside the same range, and the instruction at a range's end is always
a jump this compiler emits. `now` is kept because it is right by construction
rather than by that argument; no test distinguishes them, and that is recorded
rather than implied.

**Roadmap.** The interpreter line gains a Built clause for `try`, and its Owed
clause loses 210 and names a pattern as a `catch` parameter (211); the
standard-library line's sentence that an engine error "becomes one of these
when `try`/`catch` arrives" is now true and reworded. Neither line is ticked.
`docs/features.md`, `CHANGELOG.md`, `REMAINING.md`, and queue items 211
(its new real-script trigger) and 142 (Annex B's catch `var`) move with it.

**Compliance review.** Rules applied: law 1 — `catch (e) { var e; }`, which
only Annex B allows, is refused as not a program and recorded in item 142
rather than half-built; law 3 — no stub, no `todo!`, no `unwrap` outside
tests; law 4 — no `unsafe`; ADR 0013 § 3 — a `catch` reaches only the page's
escapes, never a full heap, an interruption, a lost reference or something not
built; § 4 — never panics, every bound ours, interruptible on every way a
program can run for ever; ADR 0014 § 2 — a thrown value is in a Rust local only
across code that cannot allocate, checked under stress; LOOP stage 2 §§ 1–2 —
opened and closed against a frozen real script, hostile tests and no panic.
One file, one responsibility: the statement is its own compiler file and the
landing its own interpreter file; `compile.rs`'s `leave` was split into finding
the target and `jump_out`. Clippy's four findings were fixed by renaming and
restructuring, not allowed. Tests asserting the old refusal were changed to the
new truth (`compile.rs`'s unit test, `what_a_program_evaluates_to.rs`'s table
and uncaught-throw test, `an_engine_that_is_hostile.rs`'s refusal test now on
`for…of`, `what_an_array_is.rs`'s frozen-script test now asserting past 2853).
Nothing positions, sizes or paints, so layout assertions and reference renders
do not apply.

**Gate.** `scripts/gate.sh` exited 0: formatting clean, clippy silent, all
workspace tests pass, nothing stubbed, `unsafe` forbidden, Exhibit A on every
file, every rented crate behind its boundary, no coordinate verb, the
supervisor stop rule holds, `CHANGELOG.md` changed. `git diff --check`
passes. The log was kept in this session's scratchpad, not committed.

**Unresolved obligations.** An uncaught throw that passed through a `finally`
is reported at the `try` and, for an engine error, as "the script threw a
value" rather than its kind and message, because what leaves the `finally` is
a rethrow of the object the error became; where it was first thrown is a stack
trace's business, item 78. The service worker still cannot *run*: it needs
`for…of` (211), `Object.values`/`keys` and `concat` (73), promises (75) and an
embedder's `self`. 104 items are open (105 before; 210 closed). Next unused
queue number **230**; next ADR **0016**. This is one iteration, not a finished
queue or roadmap.


---

## Iteration 131 — queue item 230, cut from 211, 75 and 73: `for…of`, and the protocol it reads through

The checkout was clean on entry at `82d6303`, with the one stash iteration 130
recorded (`stash@{0}`, superseded `try` work) left where it is — dropping it is
a person's call. Read `CLAUDE.md`, the complete `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s JavaScript lines, `REMAINING.md`, iteration 130 and the
selection reasoning of 124, the open queue items of section D, ADR 0013 (all
of it), ADR 0014's section list and § 2, and the JavaScript lines of
`docs/features.md`. No `AGENTS.md` exists in this repository. No sibling
repository was read or modified.

**Selection followed queue order, dependencies and the real-script rule.**
157, 158, 187, 60, 169, 197, 201 and 203 keep their recorded blockers; 222 and
207 are blocked on a frozen script; 223, 226 and 228 have no real-script
trigger or wait on 221; 224, 229, 213, 217 and 215 depend on 73, 211 or 75.
211 has the trigger — the service worker refused at byte 2922,
`for (const account of Object.values(…))` — but depends on 75 for the
iteration protocol, which is not built. 221 and 220 have no trigger. **73 is
the first item whose dependencies are done**, and the queue says to cut it
when taken; iteration 124 settled how — by the real failure. That failure
needed one piece each of 211 (`for…of`), 75 (the protocol and an iterator) and
73 (`Symbol.iterator` and the array iterator), so **item 230** is those
pieces, written into the queue with its own closing conditions and ticked at
that scope, exactly as 225 was cut from 73 and 211.

**What was built.**
`compile/for_of.rs`: `for…of` with a `let`, `const` or `var` name, a name, or
a property as its head; the head's dead zone around the iterable; a binding
per pass; `GetIterator`, each step and `IteratorClose` compiled to ordinary
`GetKeyed`/`Call`/`GetNamed` instructions, `next` read once into a frame slot.
Leaving early reuses `try_statement.rs`'s routing: the loop pushes
`Finally::closing`, so `break`, `return` and an outer `continue` are routed
through the closing and carried on from its end (`route_here` and `carry_on`
were split out of `finally` for both to use), while the loop's own `continue`
is the one exit that does not cross it. A throw from the body lands in a
handler that calls `return` inside a second handler which drops whatever it
does, then throws on; a throw from `next`, `done` or `value` is outside the
guarded range and closes nothing. `code.rs`: `Op::WellKnown`, `Op::Iterable`,
`Op::RequireObject` and `Expecting`; `interpret/iterate.rs` runs them.
`object/array_iterator.rs` and `Cell::ArrayIterator`; `builtin/array_prototype.rs`
(`keys`, `values`, `entries`, `[Symbol.iterator]` as the same function as
`values`), `builtin/array_iterator.rs` (`next`, the `"Array Iterator"` tag,
`CreateIterResultObject`), `builtin/iterator_prototype.rs`
(`[Symbol.iterator]` answering `this`); `WellKnown` in `object/symbol.rs`, and
the two symbols made and rooted by `Intrinsics`. `Object.prototype.toString`
reads `Symbol.toStringTag`, through a getter as a call at step 1, working the
builtin tag out again from `this` rather than keeping it. A native is now
handed the realm's intrinsics (`Call::within`), read-only, because `values`
and `next` make objects whose prototypes are intrinsics. `Engine::well_known`
gives an embedder the symbols, since a node list will become iterable the same
way. `Missing::AnIteratedValueBehindACall` refuses an element or an
array-like's `length` behind a getter or a conversion, cut to **item 231**:
a call from inside the iterator would make a generator's *executing* and
*completed* states observable, and this iterator keeps neither.

**Two changes outside the item, both forced by it.** `Engine::step`
crossed clippy's hundred lines with the three new arms; `Op::This`'s inline
body moved into `push_this`, which is what the function's own comment asks
for, rather than allowing the lint. And `What::TakingAValueApart`'s
description still named the array literal, which item 225 built; it now names
what it refuses.

**Evidence.** `crates/alo-js/tests/what_for_of_reads.rs`, thirty-three tests,
every table program run ordinarily and with the collector at every allocation
and required to agree: arrays in order with holes and a hole a prototype
fills; keys, values and entries; an array growing and shrinking while walked;
a finished iterator staying finished; an iterator iterating itself; array-likes
through `.call` with `ToLength`; the tag; a binding per pass; a `const` head;
the dead zone, including a closure made in the iterable; `var`, name and
property heads with the target evaluated each pass; completion values; no
closing on finishing or `continue`; closing on `break`, labelled `break`,
`return`, an outer `continue` (inner only), three nested loops innermost
first, a `finally` inside the body before the closing and one around the loop
after; a throw from the body closing and ignoring a throwing, non-object,
non-callable or getter-throwing `return`; a failed closing after `break` not
ignored; no closing for a throw from `next`, `done` or `value`; `next` read
once and `done`/`value` read in order; every protocol `TypeError`; refusals by
name for item 231, item 73's wrapper, item 211's patterns and `for…in`, and
item 75's `for await`; an embedder's object made iterable and tagged through
`Engine::well_known`, a non-callable `Symbol.iterator`, one answering no
object, and a tag behind a getter called once or throwing; the stop switch on
an endless iterator from another thread; live cells equal after three runs of
ten thousand passes; every prefix cut of a program using every form; twenty
thousand nested loops refused by the parser, two hundred closed by one
`break`, and two thousand in a row. `builtin/array_prototype.rs`'s unit test
builds the intrinsics with the collector at every allocation and checks the
chain, the identity of `values` and `[Symbol.iterator]`, every attribute and
both symbols' descriptions; `object/array_iterator.rs` has its own.

**Doctored runs, six**, each restored byte for byte and the suite re-run green:
no closing on `break`/`return` fails eight tests; no handler around the body
fails one; the loop's own `continue` closing fails three; no dead zone for the
head fails one; one environment for every pass fails four; the result's value
not held across its allocations fails two, under the collector at every
allocation.

**The frozen script compiles whole.** The four tests that pinned the next
refusal (`what_a_catch_catches.rs`, `what_an_array_is.rs`, `what_new_makes.rs`,
and the compile-refusal tests in `an_engine_that_is_hostile.rs` and
`what_a_program_evaluates_to.rs`) now accept or assert that, or use a
destructuring head as the refused form. Running it — checked with a throwaway
probe, not committed — stops at `ReferenceError: 'self' is not defined (at
byte 1168)`: a worker's global, which is an embedder's (ADR 0013 § 5, item
91). So the service worker's next trigger is an embedder rather than this
engine, and after that `Object.values`, `concat` (73) and promises (75).

**Roadmap.** The interpreter line gains a Built clause for `for…of` and its
Owed clause names a destructuring head, `for…in` (211) and `for await` (75)
instead of `for…of`; the standard-library line gains a Built clause for
iterating an array and its Owed clause names the `Symbol` function, the other
eleven symbols and the iterator helpers; and the line for promises, generators
and iterators, which read as unstarted, gains a Built clause for the protocol
and an Owed clause for everything else. None is ticked. `docs/features.md`,
`CHANGELOG.md`, `REMAINING.md`, and queue items 211 (its dependency on 75 met
by 230 for what it has left; no trigger), 73 and 75 move with it.

**Compliance review.** Law 1: nothing legacy; `for…in` is not taken because
it is a different mechanism and has no trigger. Law 3 and ADR 0013 § 3: no
stub, no `todo!`, no `unwrap` outside tests; no `Symbol` function was made to
carry two symbols, because a `Symbol` with only `iterator` would answer a
page's feature test wrongly — the symbols are reachable only by `for…of` and by
an embedder; the iterator helpers are absent and `[].values().map` is
`undefined`; a getter the iterator would have to call is refused by name
rather than called without the generator states. Law 4: no `unsafe`. ADR 0013
§ 4: never panics (every cut of a program, nesting bounds), every loop
interruptible (the backward jump, tested), arithmetic bounded (`ToLength`
clamps to 2⁵³−1 and the index never passes it). ADR 0014 § 2: an iterator and
its `next` live in frame slots between passes, every native allocation holds
what it made in a scope, and the stress runs and the doctored run say so.
LOOP stage 2 §§ 1–3: opened and closed against the frozen real script; the
bytes are a script we execute, and the hostile tests cover them; dependencies
were respected by cutting rather than by skipping. One file, one
responsibility: the statement, the instructions, the iterator cell and each of
the three prototypes are files of their own; `try_statement.rs` holds the
routing both a `finally` and a closing use, which its comment now says.
Clippy's findings were fixed by restructuring and renaming, never allowed.
Nothing positions, sizes or paints, so layout assertions and reference renders
do not apply.

**Gate.** `scripts/gate.sh` exited 0: formatting clean, clippy silent, all
workspace tests pass, nothing stubbed, `unsafe` forbidden, Exhibit A on every
file, every rented crate behind its boundary, no coordinate verb, the
supervisor's stop rule holds, `CHANGELOG.md` changed. `git diff --check`
passes. The log was kept in this session's scratchpad, not committed.

**Unresolved obligations.** Item 231 (an iterator reading through a call).
`REMAINING.md`'s summary table and its "101 open queue items" sentence were
already stale before this iteration and were not recomputed; the per-iteration
paragraph gives the current count. 105 items are open (104 before; 230 added
closed, 231 open). Next unused queue number **232**; next ADR **0016**. This
is one iteration, not a finished queue or roadmap.

---

## Iteration 132 — item 76's decision: the event loop is the renderer's, a job is the engine's

The checkout was clean on entry at `d4c9c99`, with iteration 130's stash
(`stash@{0}`, superseded `try` work) left where it is — dropping it is a
person's call. Read `CLAUDE.md`, the complete `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its JavaScript lines, `REMAINING.md`,
iterations 122 and 131, every open queue item through section E, ADR 0013 §§
3–9 and its *What this does not decide*, ADR 0014 § 7 and its *What this does
not decide*, ADR 0012 §§ 4–5, ADR 0005's *What runs where* and *Which way the
boundary points*, ADR 0015 for the form an ADR-only iteration takes, both
frozen scripts and their `origin.txt`, and the JavaScript lines of
`docs/features.md`. No `AGENTS.md` exists in this repository. No sibling
repository was read or modified.

**Selection followed queue order, dependencies and the real-script rule.**
157, 158, 187, 60, 169, 197, 201 and 203 keep their recorded blockers; 222 and
207 are blocked on a frozen script; 223, 226, 228 and 231 wait for a
real-script trigger or on 221; 224, 229, 213, 217 and 215 depend on 73, 211 or
75; 211 has no frozen script reaching what it has left; 221 and 220 have no
trigger. **73** has its dependencies met but no running real script reaches a
builtin of it: the service worker stops at `self` before any, and the
`Object.values`, `concat` and `Promise.all` it calls are inside handlers that
run only as tasks. **74** has no running script reaching a regular expression:
the theme generator has six, but it is a module (refused as `AModule`, item
77) and a Node script whose `node:fs` imports no browser resolves. **75**
depends on 76. **76** depends only on 72, which is done, and is on the
service worker's path — `self` is item 91, which depends on 76 and 83, and
every handler the worker registers is a task. But 76 could not name the
decision it implements: ADR 0013 leaves *the task boundary* to it, ADR 0014
*where a safepoint falls relative to a task or a microtask*, and ADR 0012 § 4
the precise edge of the agent's window. `LOOP.md` stage 2 § 4 makes that
decision its own iteration, before any code depends on it — iteration 122's
precedent for item 207.

**What was built: ADR 0016, accepted.**
`docs/decisions/0016-the-event-loop-is-the-renderers-and-a-job-is-the-engines.md`.
The loop lives in `alo-renderer`; the **job queue is `alo-js`'s**, in the
heap, because a job is a function and arguments the collector must see and
`HostEnqueuePromiseJob` is called mid-run, so a renderer-held queue would be a
root per promise reaction and an embedder re-entered half way through an
instruction; `queueMicrotask` asks the engine to queue one. A task held by the
renderer holds its script by a `Root`. A task is one `ToRenderer` message or
one thing the renderer scheduled — a timer, a finaliser's cleanup, and every
response, so a network round trip is always two tasks. The next task is the
oldest across all queues (deterministic; priority waits for a measurement). A
microtask checkpoint follows every task and every call that leaves nothing
running, never nests, and ends the job with `Heap::end_job`
(`ClearKeptObjects`). A collection may still begin at any allocation; the
quiet point between tasks is the only place the loop itself asks for one and
where finaliser callbacks are queued as tasks. A frame is a message from the
browser process carrying its time; `requestAnimationFrame` callbacks run in
registration order, each followed by a checkpoint. An `Act` is one task and is
answered only after its checkpoint, so a `setTimeout(0)`, a frame callback or
a response's continuation is not the agent's, and a renderer holding its
answer open is bounded by the browser process's `Stop` and made visible by
recording the window's two ends with the action (written into item 203). A
stopped task stops the page: the queues are dropped, not resumed. A worker's
loop is the same loop without the frame. Spec facts were checked against what
HTML's processing model says, not invented: the checkpoint's reentrancy flag,
`ClearKeptObjects` at its end, *clean up after running script*, the
user-agent-chosen task queue and rendering opportunity, and the
listener-microtask ordering difference between a person's click and
`element.click()`.

**What is not built, and why it stops here.** No code, dependency or test was
added. Item 76 stays unticked and now records that its ADR is written, what it
says, its trigger, and that the first code cut is the job queue, the
checkpoint and the task order closed by its table. Item 203 gains the window's
two ends. `docs/features.md` is unchanged: no capability changed, and its
event-loop line already describes the feature rather than claiming it.

**Roadmap.** The event-loop line, which read as unstarted, gains an Owed
clause saying the decision is made (ADR 0016) and that no code exists — the
job queue, checkpoint, task order, rendering steps and
`requestAnimationFrame` are all item 76. No Built clause, because a decision
is not a crate or a capability (`ROADMAP.md`: *a Built clause that cannot name
a crate or a landed capability is decoration*). Not ticked. `CHANGELOG.md`
and `REMAINING.md` move with it.

**Compliance review.** The four laws: law 1, nothing legacy; law 2, the
agent's window is made precise rather than widened; law 3, no speed claim —
the ADR says frame rate and input priority are measurements on hardware; law
4, the ADR authorises no `unsafe` and `unsafe_code = "forbid"` is unchanged.
*Settled decisions live in `docs/decisions/`*: ADRs 0005, 0012, 0013 and 0014
were read before deciding and the new one stays inside each — the engine
gains no clock, no I/O and no notion of a task (0013 §§ 5–6), the renderer
never calls back and waits (0005), and finalisers run as tasks (0014 § 7).
`LOOP.md`: one item, a decision as its own iteration, no tick for unfinished
work, no gate or test changed, no page manufactured. Each edited file keeps
its single responsibility. Nothing positions, sizes or paints, so layout
assertions and reference renders do not apply. No push, no supervisor
launch.

**Gate.** `scripts/gate.sh` exited 0 (it runs under `set -o pipefail`, so its
truncated `cargo test` output cannot mask a failure): formatting clean,
clippy silent, all workspace tests pass, nothing stubbed, `unsafe` forbidden,
Exhibit A on every file, every rented crate behind its boundary, no
coordinate verb, the supervisor's stop rule holds; the documentation check
reports no uncommitted code to judge, which is right for a documentation-only
change. `git diff --check` passes. The log was kept in this session's
scratchpad, not committed. That verifies the repository still meets its gate;
it says nothing about an event loop that does not exist yet.

**Unresolved obligations.** Item 76's code, all of it. 105 queue items are
open (unchanged: none added, none closed). Next unused queue number remains
**232**; next ADR is **0017**. This is one iteration, not a finished queue or
roadmap.

---

## Iteration 133 — queue item 232, cut from 76: the engine's half of the event loop

The checkout was clean on entry at `b9112d0`, with iteration 130's stash
(`stash@{0}`, superseded `try` work) left where it is — dropping it is a
person's call. Read `CLAUDE.md`, the complete `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its JavaScript lines, iterations 131 and 132,
queue items 73–79, ADR 0016 in full, and the parts of `alo-js` the change
touches (`interpret.rs`, `interpret/call.rs`, `interpret/frame.rs`,
`interpret/catch.rs`, `object/native.rs`, `object/slots.rs`, `object/cell.rs`,
`heap.rs`). ADR 0014 § 2 and § 7 and ADR 0013 §§ 4–5 were read as ADR 0016
quotes and depends on them. No `AGENTS.md` exists in this repository. No
sibling repository was read or modified.

**Selection followed queue order and dependencies.** Every item before 76
keeps the blocker iteration 132 recorded for it (157, 158, 187, 60, 169, 197,
201, 203; 222 and 207 on a frozen script; 223, 226, 228, 231 on a trigger or
221; 224, 229, 213, 217, 215 on 73, 211 or 75; 211, 221, 220, 73 and 74 with no
running frozen script reaching them), and 75 depends on 76. **76's dependency
(72) is done and its decision is now written (ADR 0016)**, and iteration 132
named its first code cut. It is larger than an iteration — it spans two crates
and the renderer runs no script at all yet — so it was **cut by owner, as ADR
0016 § 1 draws the line**: item 232 the engine's half (built and ticked here),
item 233 the renderer's loop, item 234 the rendering steps and
`requestAnimationFrame`. 76 stays open and closes when 233 and 234 have.

**What was built.** `job.rs`: the job queue as one `Slots` cell rooted for the
engine's life, jobs back to back with their argument counts beside the heap,
taken from the front and compacted once what has run is all or more than half
of the list, so a job requeueing itself for ever holds one job's slots.
`Jobs::take_onto` moves a job straight onto a rooted stack with nothing
allocated between. `interpret/checkpoint.rs`: `Engine::call` (a function run
with nothing running — the callee, `this` and arguments laid out at the bottom
of a fresh stack and entered; `enter_at`'s floor is zero when no frame is
running, the one change to an existing rule, which no existing caller could
reach), `Engine::queue_job` (refusing a non-function with the `TypeError`
`queueMicrotask` throws, at the call), `Engine::jobs_waiting`, and
`Engine::checkpoint` → `Drained`: jobs oldest first including those jobs
queue, `Stop` read before every job (a builtin job enters no frame and would
otherwise never look), a throw reported to the embedder's callback as it
happens — handed `&Objects` so it can describe a thrown value before anything
allocates and cannot run anything — and the next job run; any other escape
drops the queue (§ 7); `Heap::end_job` either way. A job's answer is not kept,
so the value the embedder's last run answered stays kept across a checkpoint —
found in review, given a test, and the test checked against the flaw.
`object/native.rs`: `Want::Job`, which is how an embedder's `queueMicrotask`
reaches the queue; `interpret/call.rs` answers it `undefined`. `Slots::remove_front`.
`Engine::run`'s list making and releasing moved into `two_lists` and
`finish`, shared with `call`, which also stops `run` leaking its stack's root
if the second list cannot be made.

**How the ADR's rules are held.** § 1: the queue is in the heap and the
engine queues to itself; `alo-js` gained no task, clock or I/O. § 3: a
checkpoint after every callback is the loop's to do, and the test shows the
engine supports it (`1a2b` for two loop calls, `12ab` for one script calling
both); *never nests* is held by the borrow — the checkpoint holds `&mut
Engine` throughout and a builtin is handed no engine — rather than by a flag,
which this entry records as the one place the code's mechanism differs from
the ADR's wording ("so do we" guard it with a flag) while keeping its rule.
§ 4: the checkpoint ends the job. § 7: stopped means dropped.

**Evidence.** `crates/alo-js/tests/what_a_checkpoint_runs.rs`, thirteen tests,
tables run ordinarily and with the collector at every allocation and required
to agree: order within and across tasks, FIFO, jobs queued by jobs, a
thousand-long chain, `this`, throws reported (a string, a `TypeError`, an
object still live when described), a job's own `catch`, non-function
refusals, the loop-call versus script-call row, `Engine::call` (`this`,
arguments, a throw, two hundred nested calls, a runaway recursion as
`RangeError`, a builtin, a non-function, `Heap::check`), an embedder's job
whose argument is held only by the queue across two runs and a collection,
the last run's value kept, the job ended when finished and when stopped,
stopped before starting, an endless job and two endless requeues stopped from
another thread (or the doubling one meeting the heap's ceiling, ADR 0016 § 3's
other answer), live cells equal after three checkpoints of two thousand jobs
with 286 throws each, and every prefix cut of a program that queues jobs.
`job.rs` has three unit tests and `slots.rs` one.

**Doctored runs, eight**, each restored byte for byte (`cmp`) and the suite
re-run green: running only the jobs waiting at the start fails two tests;
newest first fails the unit test and two integration tests (a first attempt
that only reversed the counts was not a reorder and was discarded rather than
counted); keeping the queue after a stop fails three; never ending the job
fails one; a throw ending the checkpoint fails two; no `this` slot fails
seven; moving a job's values out of the queue and allocating before they reach
the stack fails four under the collector at every allocation; keeping a job's
answer fails one.

**Roadmap.** The event-loop line gains a Built clause naming `alo-js`, item
232 and its capabilities, and its Owed clause now names 233 and 234; the
promises line's Built clause gains the microtask queue and its Owed clause
says promises' reactions will be jobs on it. Neither is ticked.
`docs/features.md` (event loop and promises lines), `CHANGELOG.md`,
`REMAINING.md`, `lib.rs`'s module comment, and queue items 75 and 76 move
with it.

**Compliance review.** Law 1: nothing legacy. Law 2: unchanged; the agent's
window (§ 6) is item 233's. Law 3: no stub, `todo!` or `unwrap` outside tests;
`queueMicrotask` is not shipped by the engine because it is HTML's and an
embedder's (§ 1), so the test defines it as item 233 will; no claim about
speed. Law 4: no `unsafe`. ADR 0013 § 4: never panics on any cut, every job
interruptible, the queue's length bounded by the heap's ceiling through
`Heap::write`'s accounting. ADR 0014 § 2: every value is on a rooted stack or
in the rooted queue across every allocation, and the stress runs and the
doctored run say so. LOOP stage 2: a script is hostile input and the prefix
and endless-job tests cover it; dependencies respected by cutting; the cut
written into the queue as items with closing conditions. One file, one
responsibility: the queue (`job.rs`) and what the engine does with it
(`interpret/checkpoint.rs`) are separate files; `interpret.rs` keeps run
setup, which `two_lists` and `finish` are. Clippy's findings in the new test
were fixed by restructuring helpers to answer `Option`, renaming, and the
`unnecessary_wraps` expectation the other builtins' tests already carry for a
`Body`. Nothing positions, sizes or paints, so layout assertions and
reference renders do not apply.

**Gate.** `scripts/gate.sh` exited 0: formatting clean, clippy silent, all
workspace tests pass, nothing stubbed, `unsafe` forbidden, Exhibit A on every
file, every rented crate behind its boundary, no coordinate verb, the
supervisor's stop rule holds, `CHANGELOG.md` changed. `git diff --check`
passes. The log was kept in this session's scratchpad, not committed.

**Unresolved obligations.** Item 76 is not done: items 233 (the renderer's
loop, where the renderer first holds an engine and `queueMicrotask` is
installed) and 234 (frames and `requestAnimationFrame`). No page runs a job
yet. 107 queue items are open (105 before; 232 added closed, 233 and 234
open). Next unused queue number **235**; next ADR **0017**. This is one
iteration, not a finished queue or roadmap.

---

## Iteration 134 — queue item 235, cut from 233: the renderer's event loop itself

The checkout was clean on entry at `8b7f1cf`, with iteration 130's stash
(`stash@{0}`) still left where it is — dropping it is a person's call. Read
`CLAUDE.md`, the complete `docs/autonomy/LOOP.md`, `ROADMAP.md`'s conventions
and its JavaScript lines, iteration 133's entry, queue items 73–79 and
232–234, ADR 0016 in full, and the parts of `alo-js` and `alo-renderer` the
change touches (`interpret.rs`, `interpret/checkpoint.rs`, `job.rs`,
`object/native.rs`, `heap.rs`, `abrupt.rs`, `renderer.rs`, `message.rs`,
`page.rs`, `lib.rs`). ADR 0014 § 2 and § 7, ADR 0013 §§ 4–5 and ADR 0005
were read as ADR 0016 quotes and depends on them. No `AGENTS.md` exists in
this repository. No sibling repository was read or modified.

**Selection followed queue order and dependencies.** Nothing landed since
iteration 133, so every item before 76 keeps the blocker it recorded; 75
depends on 76; 232 is done, so **233 was the first eligible item**.

**233 was cut, because building it whole needs two things nobody has
settled.** `Renderer::handle` answers each message synchronously, so a task a
page queues for itself has no idle moment to run in, and ADR 0016 § 6 forbids
running it inside an `Act`'s window — the boundary needs a way for the browser
process to let a renderer run its due tasks. And the only task that can carry
a page's script today is its own `<script>` elements at load: no timer (92),
event (81) or response (83) exists. Running those obliges the renderer to
know the page's `Content-Security-Policy` (item 165), which `Page` does not
carry, and running inline script its author forbade is not a shortcut this
loop may take. So, cutting scope and not depth, **item 235 is the loop
itself**, built and ticked here, and 233 keeps the `Renderer` holding it with
both questions written into it.

**What was built.** `alo-renderer/src/event_loop.rs`: `EventLoop` owns an
`Engine`; `queue_script` and `queue_calls` queue a task; `run_next` runs the
oldest and answers a `Turn` (its number, its reports, the jobs it ran, and
why it stopped the page if it did). A task is a script's text or callees
called in turn with one `this` and argument list — a dispatch — with **a
checkpoint after every piece**. A throw nothing caught, a script that does
not parse, and one the engine will not compile are `Report`s and the loop runs
on (`event_loop/report.rs`, describing a thrown value as it is thrown, before
anything allocates). Any other escape stops the page: waiting tasks dropped
and their roots released, jobs dropped, nothing more queued or run (ADR 0016
§ 7). The `Stop` switch is read before every piece because a straight-line
script never reads it. The quiet point after each task is checked — no open
scope, nothing kept — and a noisy one stops the page (§ 4).
`event_loop/task.rs`: one sequence number, oldest first, and a task that calls
script holds `this`, the arguments and the callees **in one heap list under
one `Root`** (§ 1's root per task). `event_loop/microtask.rs`:
`queueMicrotask` on the global object, a builtin asking for `Want::Job`. In
`alo-js`: `Engine::function` (an embedder's builtin inheriting the realm's
`Function.prototype`) and `Engine::abandon` (drop every job, run none, end the
job — what a loop does when a *task* stopped the page; `checkpoint` now uses
it for its own escape).

**Not built, and said so in the code and the queue:** asking for a collection
at the quiet point (no reason exists yet) and queueing finaliser cleanups
there (no `FinalizationRegistry` — item 73); a ceiling on waiting tasks (only
the renderer queues tasks today; item 92 brings the first a page controls).

**Evidence.** `crates/alo-renderer/tests/what_the_event_loop_runs.rs`,
fourteen tests, tables run ordinarily and with the collector at every
allocation and required to agree: a job after its task and before the next,
jobs oldest first, jobs queued by jobs in the same checkpoint, `queueMicrotask`
inheriting `Function.prototype`, tasks oldest first with their numbers and one
job each, `1a2b` for a dispatch to two listeners against `12ab` for one script
calling both and `2b1a` for the reverse order, `this` and the argument, a throw
in a task, a job and a listener reported with the rest run, `queueMicrotask(1)`,
a script that does not parse, a callee that is not a function; a waiting task's
argument surviving a collection when nothing else holds it and collected after
the task ran, with `Heap::check` clean; three rounds of five hundred two-listener
tasks leaving the live cell count equal; a page stopped while idle (tasks and
roots dropped, further queueing refused), mid-task from another thread with its
job dropped, and in an endless requeue; a noisy quiet point; every prefix cut
of a script that queues, reported or run, never panicking, with the next task
running. Unit tests: `task.rs` two, `report.rs` two. `alo-js` gained
`abandoning_drops_every_job_runs_none_and_ends_the_job`.

**Doctored runs, nine attempted, eight counted**, each restored byte for byte
(`cmp`) and the suite re-run green: one checkpoint per task rather than per
call fails two tests; newest-first fails two; a stop that keeps waiting tasks
fails two; a stop that keeps jobs fails one; no quiet-point check fails one; a
task's root never released fails three; `abandon` not ending the job fails two
in `alo-js`; no switch read before a piece **first passed** — the idle-stop
test's first task was a call, which reads the switch itself — so the test was
fixed to put a straight-line script first, and it then fails one. A doctored
run dropping a waiting task's root did not compile and is not counted; the
property is asserted directly by `a_waiting_task_holds_what_it_will_call_through_a_collection`.

**Roadmap.** The event-loop line's Built clause gains the renderer's loop
(item 235) and its Owed clause names what 233 keeps and 234; not ticked.
`docs/features.md` (event loop line), `CHANGELOG.md`, `REMAINING.md`, and
queue items 76, 233 and 235 move with it.

**Compliance review.** Law 1: nothing legacy. Law 2: unchanged; the agent's
window (§ 6) is 233's, and nothing here answers an `Act`. Law 3: no stub,
`todo!` or `unwrap` outside tests; nothing claims speed. Law 4: no `unsafe`.
ADR 0016 § 1: the loop is the renderer's and `alo-js` gained no task; the
engine queues its own jobs. § 2: oldest first, one number. § 3: a checkpoint
after every piece of script, never nested (the engine's borrow). § 4: the quiet
point is checked, and collection there is not asked for without a reason. § 7:
stopped means dropped. ADR 0014 § 2: a waiting task's values are under a root,
and the caller's must be rooted while the task is made, which the API says.
LOOP stage 2: a script is hostile input and the prefix and endless tests cover
it; dependencies respected by cutting; the cut written into the queue with a
closing condition. One file, one responsibility: the loop, its task queue,
`queueMicrotask`, and the words for a report are four files. Clippy's
`panic` findings in the new test's helpers were fixed by making them answer
`Option` and `Result`. Nothing positions, sizes or paints, so layout
assertions and reference renders do not apply.

**Gate.** `scripts/gate.sh` exited 0: formatting clean, clippy silent, all
workspace tests pass, nothing stubbed, `unsafe` forbidden, Exhibit A on every
file, every rented crate behind its boundary, no coordinate verb, the
supervisor's stop rule holds, `CHANGELOG.md` changed. `git diff --check`
passes. The log was kept in this session's scratchpad, not committed.

**Unresolved obligations.** Item 233 (the `Renderer` holding the loop, its two
open questions above) and 234 (frames and `requestAnimationFrame`); item 76
is not done. No page runs a job yet. 107 queue items are open (unchanged: 235
added closed). Next unused queue number **236**; next ADR **0017**. This is
one iteration, not a finished queue or roadmap.

## Iteration 135 — queue item 236, cut from 233: a page's own scripts at load, under its policy

The checkout was clean on entry at `38a3ccf`, with iteration 130's stash
(`stash@{0}`) still left where it is — dropping it is a person's call. Read
`CLAUDE.md`, the complete `docs/autonomy/LOOP.md`, `ROADMAP.md`'s conventions
and its event-loop line, iterations 132–134, queue items 73–79 and 232–235,
ADR 0016 in full, queue items 165 and 188 (CSP and its reporting), and the
code the change touches: `alo-renderer`'s `renderer.rs`, `page.rs`,
`message.rs`, `wire.rs`, `event_loop.rs` and its three files, `host.rs`'s
patience bound and `a_renderer_that_stops_answering.rs`; `alo-dom`'s
`sheets.rs`, `node.rs`, `document.rs` and `parse.rs`; `alo-net`'s `csp.rs`
(`Policies`, `allows_inline`, `Content`); and html5ever's duplicate-attribute
flag in `markup5ever`'s `ElementFlags`. ADR 0005 and ADR 0013 §§ 4–5 were read
as ADR 0016 quotes them. No `AGENTS.md` exists in this repository. No sibling
repository was read or modified.

**Selection followed queue order and dependencies.** Nothing landed since
iteration 134 that unblocks an earlier item, so every item before 76 keeps the
blocker iterations 132–134 recorded; 75 depends on 76; 76 closes when 233 and
234 have; 234 depends on 233. **233 was the first eligible item**: its
dependency 235 is done, and iteration 134 wrote its two open questions into it
as *the next cut's to settle*. Question (2) — the page's policy — needed
nothing undecided: CSP is built (165), and `Policies::allows_inline` already
takes a nonce and the content for a hash. Question (1) — the loop running
between messages — and `Act` answered after its checkpoint cannot be closed by
any test while no script runs in an `Act`'s task (no listener, item 81; no
timer, 92). So, cutting scope and not depth, **item 236 is (2) and the
`Renderer` holding the loop**, built and ticked here; 233 keeps (1) and the
`Act` clause, now noted as depending on 81 or 92 for its closing condition.
**No frozen page opened this**: the corpus has no page with a `<script>`, and
the trigger is the one 76 inherited (the service worker). That is recorded
rather than papered over; 238, which fetches scripts, is written to be opened
by a frozen page.

**What was built.** `alo-dom/src/scripts.rs`: `carried(document)` — HTML
`<script>` elements and `<meta http-equiv="Content-Security-Policy">` in
`<head>`, in one list in document order (a `<meta>` governs what follows it).
HTML's *prepare the script element*: `type`, else `language` as `text/…`,
else classic; the sixteen JavaScript MIME type essences without parameters;
`module`; `importmap`; anything else a data block, left out; `nomodule` on a
classic script skipped; `src` kept as written; a `<template>`'s and an SVG
`<script>` left out; an empty script nothing, whitespace a script. A nonce is
presented only where CSP's *is element nonceable* allows: no `<script` or
`<style` in any attribute's name or value, and no repeated attribute — which
the parser hides, so `Element::had_duplicate_attributes` now carries
html5ever's own flag through `create_element`. `alo-renderer`: `Page::policies`
— every enforced `Content-Security-Policy` header, filled by `from_response`,
carried over the wire, parsed in the renderer by `alo-net`'s own rules
(`Page::policies_of`). `scripts.rs`: `at_load` walks what the document
carries, adds each `<meta>` policy to the headers as it passes it, asks each
inline classic script `allows_inline(Inline::Script, nonce, Content::element)`,
runs the allowed ones as one task each through a per-page `EventLoop`
(created only when a script runs), and puts everything else into `Loaded`'s
issues as `script N: …` — a refusal in the policy's words, a fetched script
(238), a module or import map (77), every report, a stop, and every later
script after a stop. `Renderer` holds `script: Option<EventLoop>`: a `Load`
drops the last page's loop and runs the new page's scripts after rendering
(no script can see the document, so the order relative to layout is not
observable — item 80 changes that); a `Resize` re-renders through `lay_out`
and runs nothing; `Renderer::event_loop` is a test accessor, not part of the
boundary, as `rendered` is.

**Evidence.** `crates/alo-renderer/tests/a_page_runs_its_scripts.rs`,
twenty-one tests: order with each script's jobs and jobs' jobs before the next
(`abcdefg`); throws in a script and in a job reported, an unparsable script
reported, the next running; a stop (`Function.prototype.toString`, refused by
name) stopping every later script; fetched, module and import-map scripts
said; data blocks, `nomodule`, a template's script silent; four forbidding
policies, four allowing ones (one a SHA-256 computed with `openssl` for the
exact text), a hash of other text, two policies intersected, a nonce admitting
only its own scripts, four injected-markup shapes refused, a `<meta>` policy
governing only what follows, narrowing and never widening the header, and
not counting outside `<head>`, an unreadable source refusing; a resize
running nothing, proved by marking the realm; a new page's fresh realm and no
loop for a page without script; **the page still laid out when its script
throws — the paragraph's border box asserted as 184×20 at (8, 8)**; every
prefix cut of a hostile page with scripts loading; and **an endless
`while (true) {}` through the real renderer binary** given up on within eight
times a 400 ms bound, with another site's renderer and the same site's next
load working. `alo-dom`'s `scripts.rs` has eleven unit tests; the wire round
trip now carries two policies; `page.rs` gained a test that report-only
headers are not carried.

**Doctored runs, eight attempted, seven counted**, each restored byte for
byte (`cmp`) and the suite re-run green: `<meta>` policies ignored fails one;
header policies ignored fails seven; a stop not honoured fails one; the
duplicate-attribute rule removed fails one integration and one unit test;
removing the per-load loop reset **by deleting a line passed** — the line is
redundant with the assignment after it, so that was not a removal of the rule
and is not counted; keeping the old loop when a page runs none fails one; a
resize running the scripts again **first passed**, because a fresh realm
re-running a script leaves the same `out` — the test was fixed to mark the
existing realm and to make the script report, and it then fails one.

**Found and not hidden.** A `TypeError` a script made is reported as
`uncaught: an object`: `Report::thrown` may not run script and the engine has
no call-free property read. The tests assert today's words with a comment, and
the gap is **item 239**. My first test of an "unreadable" policy was wrong —
`'sha256-not base64'` splits into several tokens and none is a hash, so
`'unsafe-inline'` correctly stood, as it would in every browser; the test was
replaced by misspelt-keyword and control-character sources, which refuse.

**Roadmap.** The event-loop line's Built clause gains the `Renderer` holding a
loop per page and a page's own scripts at load under its policy (item 236);
its Owed clause names 233's remainder, 234 and 238. Not ticked.
`docs/features.md` (event loop line), `CHANGELOG.md`, `REMAINING.md`, the
module docs of `event_loop.rs`, and queue items 76, 233, 236–239 move with it.

**Compliance review.** Law 1: nothing legacy; `nomodule` follows the
specification for an engine with modules rather than running fallbacks.
Law 2: unchanged; no script reaches the document or the agent tree. Law 3: no
stub, `todo!` or `unwrap` outside tests; what is not run is said, by item;
nothing claims speed. Law 4: no `unsafe`. ADR 0005: the renderer is handed
the policy text, fetches nothing, and a runaway script is bounded by the
browser process's existing patience — tested through the real binary. ADR
0016 § 1: the loop is the renderer's, one per page; § 2: each script one
task, oldest first; § 3: a checkpoint after each; § 7: a stopped page runs
nothing more. CSP (item 165): a policy this engine cannot read stays stricter
— parsing is `alo-net`'s alone, and unreadable sources were tested to refuse;
an element's nonce is honoured only where *is element nonceable* allows.
LOOP stage 2 § 2: a page and its scripts are hostile input — prefix cuts, an
endless script, malformed nonces and policies. One file, one responsibility:
what markup carries (`alo-dom/scripts.rs`) and which of it runs
(`alo-renderer/scripts.rs`) are two files with two reasons to change; the
renderer gained one field and one split (`lay_out`), not a second job. The
layout assertion is in numbers; nothing paints differently — the corpus has
no scripts and every reference render still matches, so no new reference
render applies.

**Gate.** `scripts/gate.sh` exited 0: formatting clean, clippy silent (two
findings in the new test's helpers and one in `alo-dom` were fixed, not
allowed), all workspace tests pass (2229 passed, 0 failed), nothing stubbed,
`unsafe` forbidden, Exhibit A on every file, every rented crate behind its
boundary, no coordinate verb, the supervisor's stop rule holds, `CHANGELOG.md`
changed. `git diff --check` passes. The log was kept in this session's
scratchpad, not committed.

**Unresolved obligations.** Item 233 (the loop between messages, `Act` after
its checkpoint — closable once 81 or 92 runs script in a task), 234 (frames),
237 (report-only inline violations reported), 238 (fetched scripts), 239 (an
error object said by name); item 76 is not done. No script can reach the
document. 110 queue items are open (236 added closed; 237, 238 and 239 added
open). Next unused queue number **240**; next ADR **0017**. This is one
iteration, not a finished queue or roadmap.

## Iteration 136 — queue item 237: a policy's author is told about inline script it refused or would have

The checkout was clean on entry at `1b145b1`, with iteration 130's stash
(`stash@{0}`) still left where it is — dropping it is a person's call. Read
`CLAUDE.md`, the complete `docs/autonomy/LOOP.md`, `ROADMAP.md`'s conventions
and its CSP and event-loop lines, `docs/autonomy/REMAINING.md`, iteration
135's entry and the selection reasoning of 131–132, every open queue item from
157 to 93, queue items 165, 188, 236 and 237, ADR 0005's *What runs where* and
*Which way the boundary points*, the CSP lines of `docs/features.md`, and the
code the change touches: `alo-net`'s `csp.rs` and `csp_report.rs`, `pool.rs`'s
`report`, `a_violation_a_page_reports.rs`; `alo-renderer`'s `page.rs`,
`scripts.rs`, `renderer.rs`, `message.rs`, `wire.rs`, `tab.rs` and the tests
that construct a `Page` or a `Loaded`. No `AGENTS.md` exists in this
repository. No sibling repository was read or modified.

**Selection followed queue order and dependencies.** Nothing landed since
iteration 135 that unblocks an earlier item: 157, 158, 187, 60, 169, 197, 201
and 203 keep their recorded blockers; 222 and 207 are blocked on a frozen
script; 223, 226, 228 and 231 wait for a real-script trigger or on 221; 224,
229, 213, 217 and 215 depend on 73, 211 or 75; 211, 221 and 220 have no
trigger; 73 and 74 have no running script reaching them; 75 depends on 76;
76 closes when 233 and 234 have; **233 now depends on 81 or 92** for its
closing condition, as iteration 135 wrote. **237 was the first eligible
item**: its dependencies, 236 and 188, are both done, and its closing
condition names no frozen page. Like 236 it was opened by no page — the
corpus has no page with a `<script>` — and that is recorded rather than
papered over. 238 needs a frozen page with a fetched script; 239 comes after
237 in the file.

**The design decision, and why it is not an ADR.** Only the renderer sees an
inline script; only the browser process may post (ADR 0005). A renderer
handing over a finished report would be the page choosing a collector and a
body for the browser's network stack, so what crosses is an **objection** —
the place of the objecting policy in a list both processes build from the
same headers (`Page::stated`), and the content's kind — and the browser
process writes the report from its own copy. This is ADR 0005's existing rule
(*the renderer's word is a claim*) applied, not a new decision, and the wire's
module docs already state it for every message from a renderer.

**What was built.** `alo-net/src/csp.rs`: `Policy::refuses_some_inline`,
factored out of `objects_to_inline` — the refusal a policy gives inline
content of a kind and placement that no nonce or hash lets in, or `None` when
`'unsafe-inline'` lets everything in; nothing in it depends on the content.
`Policies::objecting_to_inline` (places) and `Policies::inline_violation_of`
(the violation for a place, or `None` for no such policy or one that could not
have objected). `alo-renderer`: `Page::watching` carries
`Content-Security-Policy-Report-Only` headers (filled by `from_response`,
carried on the wire) and `Page::stated` parses both into the list objections
are named against. `scripts.rs` asks each inline classic script of that list
and pushes an `Objection` per objecting policy, at most `MOST_OBJECTIONS` (64)
per load, saying how many it left out; a watched objection to a script that
runs is said in the issues as `script N: runs, but …`. `FromRenderer::Loaded`
gained `objections`; the wire reads them through `Reader::objections`, which
refuses a load claiming more than 64 and a kind it does not know.
`violations.rs` is the browser process's half: `reports(page, about,
objections)` → posts, unusable endpoints, and claims it disbelieves (a place
with no policy, a policy that could not have objected, anything past 64).
Nothing in the browser process calls it on its own yet — no part of it holds
both a `Pool` and `Tabs` (item 203's dependency) — so the closing tests
compose the two exactly as such a part would.

**Evidence.** `crates/alo-renderer/tests/a_policys_author_is_told.rs`, ten
tests. The two closing clauses run the real `alo-render` binary in a tab,
build the reports with `violations::reports` from what it answered and post
them with `Pool::report` to a collector on `127.0.0.1`: under report-only
`script-src 'none'` the script runs (`out == "ran"`, read in-process) and one
POST arrives with `"effective-directive":"script-src"`,
`"blocked-uri":"inline"`, `"disposition":"report"` and the document's URL;
under the same policy enforced, the script does not run and the POST says
`"disposition":"enforce"`. Also: `report-to` resolved against the response's
own `Reporting-Endpoints` (Reporting API document to `/reports`); only the
objecting policy named; nonce- and hash-allowed scripts not objected to (the
hash computed with `openssl` for the exact text); each script and each policy
its own objection; a `<meta>` refusal obeyed and not passed on; no objection
after the page stopped; seventy scripts carrying 64 objections and saying six
were left out; every prefix cut of a page with three policies objecting only
to places the browser process believes. Unit tests: four in `violations.rs`,
three in `csp.rs`, two new wire tests, and the existing round trips and prefix
cut extended to carry `watching` and objections.

**Doctored runs, six, each restored byte for byte (`cmp`) and the suite
re-run green**: the renderer sending no objection fails seven integration
tests; `inline_violation_of` believing any place fails the disbelief unit
test; the wire's ceiling removed fails the flood test; a watched header read
as enforced fails the browser's-own-copy unit test; the browser's own ceiling
removed fails the flood unit test; the watched objection not said in the
issues fails the first closing test.

**Found and not hidden.** My first draft of one test used `out += 'b'` in a
page where an earlier script had been refused, so `out` was never declared
and the script threw a `ReferenceError`; the test was wrong, not the engine,
and now assigns. Clippy's line limit on `read_from_renderer` was met by moving
the objections into a `Reader` method, as `outcome` and `refusal` already
are, not by allowing the lint.

**Roadmap.** The CSP line's Built clause gains inline script violations
reported across the boundary (item 237); its Owed clause gains a `<meta>`
policy's `report-to` (item 240). Not ticked — a nested document and event
handlers are still owed. `docs/features.md` (a new CSP line), `CHANGELOG.md`,
`REMAINING.md`, and queue items 237 (ticked, with its evidence) and 240 (new)
move with it.

**Compliance review.** Law 1: nothing legacy. Law 2: unchanged. Law 3: no
stub, `todo!` or `unwrap` outside tests; what is not reported is said (meta
policies, objections past the ceiling, disbelieved claims); no speed claim.
Law 4: no `unsafe`. ADR 0005: the browser process parses no page — it parses
the response's headers, which its network stack already does to enforce CSP
on fetches — and treats the renderer's objections as claims, bounded and
checked against its own copy. ADR 0012 § 2: a report carries the cause of the
load it is about (the tab's document). Item 165's rule: obeying is unchanged —
a report-only policy still forbids nothing, an unreadable source still
refuses. Item 188's rules (stripping, `report-to` over `report-uri`, a failed
post not failing a load) are reused, not restated. LOOP stage 2 § 2: the
renderer's answer is hostile input — a flood refused at the wire, a strange
tag and trailing bytes refused, impossible places disbelieved, prefix cuts.
One file, one responsibility: `csp.rs` decides (two new questions about the
same decision), `violations.rs` is the browser's turning of claims into
reports, `scripts.rs` still asks a page's policies about its scripts. Nothing
positions, sizes or paints differently, so no layout assertion or reference
render applies; every existing reference render still matches.

**Gate.** `scripts/gate.sh` exited 0: formatting clean, clippy silent (one
`too_many_lines` fixed by extraction, not allowed), all workspace tests pass
(2248 passed, 0 failed), nothing stubbed, `unsafe` forbidden, the licence
notice on every file, every rented crate behind its boundary, no coordinate
verb, the supervisor's stop rule holds, `CHANGELOG.md` changed.
`git diff --check` passes. The log was kept in this session's scratchpad, not
committed.

**Unresolved obligations.** Item 240 (a `<meta>` policy's `report-to`, opened
by a frozen page); nothing in the browser process calls `violations::reports`
on its own until a part of it holds both a `Pool` and `Tabs` (item 203's
dependency); 233, 234, 238 and 239 are open and item 76 is not done. No script
can reach the document. 110 queue items are open (237 closed, 240 added). Next
unused queue number **241**; next ADR **0017**. This is one iteration, not a
finished queue or roadmap.

## Iteration 137 — queue item 239: a thrown error object said by its name and message

The checkout was clean on entry at `535e72d`, with iteration 130's stash
(`stash@{0}`) still left where it is — dropping it is a person's call. Read
`CLAUDE.md`, the complete `docs/autonomy/LOOP.md`, `ROADMAP.md`'s conventions
and its event-loop and errors lines, `docs/autonomy/REMAINING.md`, iteration
136's entry and its selection reasoning, queue items 76, 227, 228, 232–240
and 77–93, ADR 0016 (§§ 1–3 and 7 on what a loop reports and that it runs
on), the JavaScript lines of `docs/features.md`, and the code the change
touches: `alo-renderer`'s `event_loop.rs`, `event_loop/report.rs`,
`scripts.rs`, the wire's bound on a message (`LARGEST_MESSAGE`); `alo-js`'s
`object.rs` (`is_error`, `existing_key`), `object/access.rs` (`get`,
`Found`), `convert.rs` (`to_units`) and `builtin/error.rs`
(`Error.prototype.toString`). No `AGENTS.md` exists in this repository. No
sibling repository was read or modified.

**Selection followed queue order and dependencies.** Nothing landed in
iteration 136 that unblocks an earlier item: every blocker iteration 136
recorded stands, 233 still depends on 81 or 92 for its closing condition, and
240 and 238 are each opened only by a frozen page that does not exist. **239
was the first eligible item**: its dependencies, 235 and 227, are both done,
and its closing condition names no frozen page. Like 236 and 237 it was cut
from a renderer test rather than opened by a page, and that is recorded
rather than papered over.

**What was built.** `alo-renderer/src/event_loop/described.rs` puts a thrown
value into words reading the heap and nothing else (split from `report.rs`,
which keeps the `Report` kinds and their display; this file is the reading of
a page's heap). An object with `[[ErrorData]]` is said as
`Error.prototype.toString` would join it — `name` defaulting to `Error`,
`message` to empty, `": "` only when both are non-empty — with each read
along the prototype chain through `Objects::existing_key` and
`Objects::get`, so describing interns nothing, allocates nothing and runs
nothing. A getter, an object (whose `toString` would run) and a symbol (which
`ToString` refuses) are said in brackets, e.g. `Error (its message is a
getter, which was not called)`. The page's own `Error.prototype.toString` is
deliberately not consulted: a page replacing it would otherwise choose what
its own failure says. A non-error object stays `an object` — which properties
of an arbitrary object to trust is item 78's question.

**A hostile-input clause the item did not name, and why it is here.** A page
can make strings of up to 2^28 − 1 code units, and a load's whole answer,
issues included, crosses the wire in one message capped at 64 MiB — so a
thrown long string could already push a load's answer past the cap, and an
error's message would add a second way. Every string a report repeats (a
thrown string, a name, a message) is now cut at `LONGEST_SAID` (1024) code
units, never half way through a surrogate pair, with the rest counted. This
is the only change to existing behaviour besides the item's own.

**Evidence.** `crates/alo-renderer/tests/an_error_said_by_its_name.rs`,
fifteen tests through a real `Renderer`: the three closing clauses —
`throw new TypeError('x')` said `TypeError: x`; a `name` reassigned on the
instance (`NotFound: x`) and on a prototype (`Bounds: far`); a `message`
getter on the error's chain never called (a counter read back from the
page's engine is still `0`) — and around them a `name` getter never called,
an object message's `toString` never run, all seven constructors, one called
without `new`, empty name / empty message / both, a `name` deleted
everywhere (`Error: anonymous`), number and boolean parts, a replaced
`Error.prototype.toString` not run, a look-alike plain object still `an
object`, an engine-thrown `TypeError` caught and rethrown, one thrown from a
job, and a 2^13-unit message and thrown string each cut with 7168 counted.
`described.rs` has six unit tests: short and long strings, a surrogate pair
at the cut, primitives, a symbol `name` (made in the heap — no page can name
a symbol yet, there is no `Symbol` global, item 73), and a heap where nothing
has a `name` or `message` staying uninterned after describing. Two
assertions in `a_page_runs_its_scripts.rs` that said `an object` now say
`TypeError: no such thing` and `Error: broken`.

**Doctored runs, four, each restored byte for byte (`cmp`) and the suite
re-run green**: the error branch removed fails the symbol and uninterned unit
tests; a getter read as absent fails both getter tests; the cut removed fails
the cut and surrogate tests; `toString`'s empty-message rule broken fails the
uninterned test.

**Found and not hidden.** My first draft tested a symbol `name` through a
page, and it failed with `'Symbol' is not defined` — the engine has no
`Symbol` global. The test moved to a unit test that builds the error in the
heap; the integration file does not claim a page can do it.

**Roadmap.** The *Errors and stack traces* line gains a Built clause (an
uncaught error said by name and message, item 239) and an Owed clause (stack
traces and source positions, item 78; a non-error object described). Not
ticked. `docs/features.md` (that line), `CHANGELOG.md`, `REMAINING.md` and
queue item 239 (ticked, with its evidence) move with it.

**Compliance review.** Law 1: nothing legacy. Law 2: unchanged. Law 3: no
stub, `todo!` or `unwrap` outside tests; what is not said is said to be
unsaid, in words; no speed claim. Law 4: no `unsafe`. ADR 0016 § 7: a throw
is still reported and the loop runs on; describing it now cannot itself fail
or run script. ADR 0013 § 3: absent beats approximate — a getter's value is
not guessed. LOOP stage 2 § 2: what a page put in its heap is hostile input
— no call, no allocation, a bound on length. One file, one responsibility:
`report.rs` says which kind of report, `described.rs` reads a thrown value
from the heap. Nothing positions, sizes or paints differently, so no layout
assertion or reference render applies; every existing reference render still
matches.

**Gate.** `scripts/gate.sh` exited 0: formatting clean, clippy silent (one
`needless_borrows_for_generic_args` in the new test fixed, not allowed),
all workspace tests pass (2269 passed, 0 failed, counted with `cargo test
--workspace`), nothing stubbed, `unsafe` forbidden, the licence notice on
every file, every rented crate behind its boundary, no coordinate verb, the
supervisor's stop rule holds, `CHANGELOG.md` changed. `git diff --check`
passes. The log was kept in this session's scratchpad, not committed.

**Unresolved obligations.** 233, 234, 238 and 240 are open and item 76 is not
done; `violations::reports` is still called by nothing in the browser process
on its own (item 203's dependency); stack traces are item 78. No script can
reach the document. 109 queue items are open (239 closed). Next unused queue
number **241**; next ADR **0017**. This is one iteration, not a finished
queue or roadmap.

## Iteration 138 — queue item 241, cut from 78: an uncaught throw placed by script, line and column

The checkout was clean on entry at `5694a05`, with iteration 130's stash
(`stash@{0}`) still left where it is — dropping it is a person's call. Read
`CLAUDE.md`, the complete `docs/autonomy/LOOP.md`, `ROADMAP.md`'s conventions
and its errors and event-loop lines, `docs/autonomy/REMAINING.md`,
iterations 136 and 137 and their selection reasoning, queue items 75–93 and
232–240, ADR 0013 § 3 and 0016 §§ 1, 3 and 7, the JavaScript lines of
`docs/features.md`, both frozen scripts' `origin.txt`, and the code the
change touches: `alo-js`'s `abrupt.rs`, `code.rs` (`Chunk::at`), `error.rs`
(`Position::of`), `unit.rs`, `interpret.rs` (`run`, `two_lists`, `walk`),
`interpret/frame.rs` (`Frame::now`, `Run`), `interpret/catch.rs` (`land`),
`interpret/checkpoint.rs`, `bounds.rs`; `alo-renderer`'s `event_loop.rs`,
`event_loop/report.rs`, `event_loop/task.rs`, `scripts.rs`, `pipe.rs` and
`wire.rs`'s message cap. No `AGENTS.md` exists in this repository. No sibling
repository was read or modified.

**Selection followed queue order and dependencies.** Nothing landed in
iteration 137 that unblocks an earlier item; every blocker iteration 136
listed stands, and 240 and 238 still wait for a frozen page. **234** depends
on 233, which is open. **77** came next: its listed dependencies (53, 72) are
done, but it has **no closing condition** and its loader is a decision no
ADR has made — a renderer cannot fetch (ADR 0005), so who fetches a module
graph under which policy and CORS mode is item 238's open question plus
linking and a module map. `LOOP.md` step 2 says such an item is marked
`needs design` and the next taken, and that is what the queue now says, with
the reason. Nothing reaches it either: no corpus page has a module script,
and the frozen `alo-theme-generator` is a Node program. **78 was next and was
eligible** (depends on 72, done) but as written had no closing condition
either; unlike 77 the decisions it needs were already made — `Chunk::at`
carries the offset ("what a stack trace (queue item 78) will be built
from"), and `Position` already defines a line and a UTF-16 column for item
78 by name — and iteration 137 left "where in the source a throw happened"
as its explicit remainder. So the iteration cut that piece as **item 241**
with its own closing condition, rather than inventing a policy. Like 236,
237 and 239 it is opened by a renderer test rather than a frozen page, and
that is recorded rather than papered over.

**What was built.** `alo-js/src/interpret/unwound.rs`: when `land` finds no
`try` for a throw — the last moment the calls it left exist — it reads each
frame, innermost first, as a `Place` (`Rc<Unit>` and the byte offset of
`Frame::now`'s instruction, so a throw from a `valueOf` is placed at the
operator that rewound for it), at most `bounds::PLACES_IN_A_TRACE` (32, a new
bound with its reasoning) with the rest counted. `Engine::unwound()` answers
it; `two_lists`, where every run, call and job starts, forgets it; the
checkpoint's report callback is handed it with each job's throw (a new third
argument). `alo-renderer/src/event_loop/source.rs`: the loop now compiles
each script itself (`compile` + `run` rather than `evaluate`) and keeps it
under the name it was queued with — `EventLoop::queue_script(name, text)`,
`script N` for a page, numbered as the load's issues are — so a function
declared in one script is placed there when another calls it. Lines and
columns are `Position::of`'s, counted from marks laid every 4096 bytes when
a script is kept (never splitting a `\r\n`), so a page throwing in a loop
cannot make the renderer re-read a one-line bundle per place. `Report::Threw`
became `{ what, trace }`, said as `uncaught: Error: e (at script 2, line 3,
column 5; called from script 1, line 1, column 9; and 4 calls further out)`;
a throw no call was entered for is placed nowhere.

**Evidence.** `crates/alo-renderer/tests/where_a_page_threw.rs`, twelve tests
through a real `Renderer`, every place counted by hand from the page's text
(one of my own counts was wrong — the UTF-16 test's comment said column 17
where code unit 15 from zero is column 16; the code was right and the comment
now gives all three counts, 19 bytes / 15 characters / 16 code units). The
four closing clauses, then: every line terminator, a refused script still
numbered, a job placed in the queued function, a rethrow, a builtin's throw
at its call, a script that did not parse, and every prefix of a page that
throws from deep calls in a script and a job (the job: 42 calls, 32 kept, 10
counted). `crates/alo-js/tests/where_a_throw_was.rs`, eleven tests of the
engine half including the recursion's 32 + 10208 = `CALLS_ON_THE_STACK`.
`source.rs` five unit tests, `report.rs` one. Existing assertions that now
carry a place were updated with hand-checked columns
(`what_the_event_loop_runs.rs` four, `a_page_runs_its_scripts.rs` four);
`an_error_said_by_its_name.rs`'s helper sets the place aside **only after
checking it is there** and in the page's one script on its one line, since
that file is about what is said, not where.

**Doctored runs, four, each restored byte for byte (`cmp`)**: nothing
recorded in `land` fails ten of the engine's eleven tests; `pc − 1` instead of
`now` fails the `valueOf` test; the bound raised a thousandfold fails the
recursion and prefix tests; a column not carried across a mark fails the
one-line bundle test.

**Found and not hidden.** A load's issues have no ceiling on *how many*: a
page can queue as many throwing jobs as it likes, and past the wire's 64 MiB
cap the renderer cannot send its answer (the tab sees a failed renderer —
safe, but every issue is lost and the reason unsaid). That predates this
change; a trace makes each report up to ~33 places long, so it is now a
closer edge. Recorded as **item 242** rather than widened into this one.

**Roadmap.** The *Errors and stack traces* line gains a Built clause (an
uncaught throw placed, item 241) and its Owed clause now names `error.stack`
and names in it (78, 220), source maps, developer tools (129), a non-error
object described, and 242. Not ticked. `docs/features.md` (that line),
`CHANGELOG.md`, `REMAINING.md`, two `alo-js` doc comments that promised this
to item 78, and the queue (77 `needs design`, 78's remainder, 241 ticked with
its evidence, 242 new) move with it.

**Compliance review.** Law 1: nothing legacy; `error.stack` (non-standard
but universal) is deliberately not built here. Law 2: unchanged. Law 3: no
stub, `todo!` or `unwrap` outside tests; an unknown program is said by its
offset and a call-less throw is placed nowhere, never guessed; no speed
claim — the marks are a bound on work a page can cause, not a performance
claim. Law 4: no `unsafe`. ADR 0013 § 3 (absent beats approximate) and § 4
(every ceiling ours, with a reason). ADR 0014: a `Place` holds an `Rc<Unit>`,
which is Rust memory with no heap edge, and recording allocates nothing in
the heap between a throw and its landing. ADR 0016 § 7: unchanged — a throw
is reported and the loop runs on. LOOP stage 2 § 2: what a page controls
here is recursion depth, script length and how often it throws — bounded at
32 places, at most 4096 bytes read per place, with a prefix-cut test. One
file, one responsibility: `unwound.rs` reads the calls a throw left,
`source.rs` keeps scripts and turns offsets into lines and columns,
`report.rs` says a report. Nothing positions, sizes or paints differently,
so no layout assertion or reference render applies; every existing
reference render still matches.

**Gate.** `scripts/gate.sh` exited 0 on the second run (the first failed only
`cargo fmt --check` on the new code; `cargo fmt --all` fixed it and nothing
was allowed or silenced): formatting clean, clippy silent, all workspace
tests pass (2298 passed, 0 failed, counted with `cargo test --workspace`;
2269 before, the 29 new tests exactly), nothing stubbed, `unsafe` forbidden,
the licence notice on every file, every rented crate behind its boundary, no
coordinate verb, the supervisor's stop rule holds, `CHANGELOG.md` changed.
`git diff --check` passes. The log was kept in this session's scratchpad,
not committed.

**Unresolved obligations.** 242 (a ceiling on reports per load); 78's
remainder (`error.stack`, names, source maps); 77 needs design; 233, 234,
238 and 240 are open and item 76 is not done; `violations::reports` is still
called by nothing in the browser process on its own (item 203's
dependency). No script can reach the document. 110 queue items are open (241
added closed, 242 added open). Next unused queue number **243**; next ADR
**0017**. This is one iteration, not a finished queue or roadmap.

## Iteration 139 — queue item 242: a ceiling on what one load says about its scripts

The checkout was clean on entry at `f1d0a31`, with iteration 130's stash
(`stash@{0}`) still left where it is — dropping it is a person's call. Read
`CLAUDE.md`, the complete `docs/autonomy/LOOP.md`, `ROADMAP.md`'s conventions
and its event-loop and errors lines, `docs/autonomy/REMAINING.md`,
iterations 136–138 and their selection reasoning, queue items 233–242, ADR
0016 (§§ 3 and 7, and *What this does not decide*'s **the numbers**: any
ceiling the loop needs lands in the code with its reason, so no ADR was
needed), the JavaScript lines of `docs/features.md`, and the code the change
touches: `alo-renderer`'s `scripts.rs`, `event_loop.rs`,
`event_loop/report.rs`, `renderer.rs` (`load`, `lay_out`), `message.rs`,
`pipeline.rs`'s issues, `wire.rs` (`LARGEST_MESSAGE`, how a `Loaded` and a
string are written) and `violations.rs`'s `MOST_OBJECTIONS`. No `AGENTS.md`
exists in this repository. No sibling repository was read or modified.

**Selection followed queue order and dependencies.** Nothing landed in
iteration 138 that unblocks an earlier item: every blocker iteration 136
listed stands; 240 and 238 wait for a frozen page; 234 depends on 233, which
depends on 81 or 92; 77 is `needs design`; 78's remainder has no closing
condition. **242 was the first eligible item**: its dependency, 236, is done,
and its closing condition names no frozen page. Like 236–241 it was found by
building rather than opened by a page, and that is recorded rather than
papered over.

**What was built.** `alo-renderer/src/scripts.rs`: everything `at_load` says
about a page's scripts — reports, refusals, scripts not run — goes through
`Said`, which keeps at most `MOST_SAID` (256) lines and counts the rest, and
the load ends with `N more things about this page's scripts were not said:
one load says at most 256`. The scripts run the same either way.
**A second clause the item did not name, and why it is here:** the ceiling
had to hold inside a single turn too. A job that throws and requeues itself
for ever made one `Turn`'s `reports` grow by a described, placed report per
job until the page was stopped — memory proportional to how long the browser
waits. So `event_loop.rs` keeps at most `MOST_REPORTS` (256) per turn and
counts the rest in the new `Turn::unreported`, asking *is there room* before
describing or placing a throw; `EventLoop::run_next_within(room)` lets the
load hand each turn only the room it has left (`run_next` is
`run_next_within(MOST_REPORTS)`, and room above it is capped to it). The
`Loaded` message's doc says which half of its issues is bounded.

**Evidence.** `crates/alo-renderer/tests/what_one_load_says.rs`, eight tests.
The closing clause through a real `Renderer`: 100000 throwing jobs give 256
lines, the first 256 throws in order and each placed, then `99744 more`; `n`
read back from the page's engine is 100000 (all ran); the answer is under
`LARGEST_MESSAGE` and round-trips the wire. Around it: 300 throwing scripts
said as `script 2` … `script 257` then `44 more`, all run; 300 fetched
scripts' "not run" lines counted the same way; exactly 256 throws said whole
with no count; every prefix of a page throwing 400 jobs within the ceiling
and under the cap. On the loop: 1000 jobs keep 256 and count 744 with 1000
run; a room of 3 (the script's own throw first) and of 0 (11 counted, 10 jobs
run); `usize::MAX` capped at 256; a job throwing and requeueing itself for
ever, stopped from another thread after 200 ms, keeping 256, counting the
rest, and `n` agreeing with kept + counted to within the one job the stop
interrupted. One of my hand-counted columns was wrong (61 for 62 — 11 + 19 +
8 + 23 characters precede the `throw`); the code was right and the test now
gives the count.

**Doctored runs, four, each restored byte for byte (`cmp`)**: the load's
ceiling removed fails one test; the checkpoint's turn ceiling removed fails
three; a turn's count not added to the load's fails two; a room above
`MOST_REPORTS` not capped fails one.

**Found and not hidden.** The markup half of a load's issues
(`pipeline::Rendered::issues`) has no ceiling either, and it amplifies: an
`<img>` with no `src` is five bytes of page and 28 bytes of answer (20 of
text, 8 of length), so about 11.5 MiB of them — well under the cap the page
itself crossed in — would make an answer the wire refuses. Found by reading
and arithmetic, **not yet by a run**; recorded as **item 243** rather than
widened into this one, whose title and closing condition are about scripts.

**Roadmap.** The *Errors and stack traces* line's Built clause gains the
ceiling (242, `scripts::MOST_SAID` and `event_loop::MOST_REPORTS`) and its
Owed clause replaces 242 with 243. Not ticked. `docs/features.md` (that
line), `CHANGELOG.md`, `REMAINING.md`, `message.rs`'s doc, and the queue (242
ticked with its evidence, 243 new) move with it.

**Compliance review.** Law 1: nothing legacy. Law 2: unchanged. Law 3: no
stub, `todo!` or `unwrap` outside tests; what is not said is counted and the
count is said; no speed claim — "costs a counter rather than memory" is a
bound on work a page can cause, not a performance claim. Law 4: no `unsafe`.
ADR 0016 § 3: a job that queues a job for ever still never yields and is
still answered by `Stop`; only what it costs to report is bounded. § 7 and
*the numbers*: unchanged behaviour, both ceilings in the code with their
reasons. ADR 0005: the renderer's answer must stay sendable; nothing new
crosses the boundary. LOOP stage 2 § 2: what a page controls here is how
often it throws — bounded per turn and per load, with a prefix-cut test.
One file, one responsibility: `scripts.rs` still decides what a load says
about its scripts, `event_loop.rs` what a turn keeps. Nothing positions,
sizes or paints differently, so no layout assertion or reference render
applies; every existing reference render still matches.

**Gate.** `scripts/gate.sh` exited 0 on the second run (the first failed
`cargo fmt --check` and clippy on the new test — `panic!` in two helpers and
a `usize as f64` cast; the helpers now answer a value or `None` as their
siblings do and the cast goes through `u32::try_from`, nothing allowed or
silenced), and again after a doc-comment change: formatting clean, clippy
silent, all workspace tests pass (2306 passed, 0 failed, counted with `cargo
test --workspace`; 2298 before, the 8 new tests exactly), nothing stubbed,
`unsafe` forbidden, the licence notice on every file, every rented crate
behind its boundary, no coordinate verb, the supervisor's stop rule holds,
`CHANGELOG.md` changed. `git diff --check` passes. `cargo doc` prints four
private-link warnings in `event_loop` and `generic` that predate this change
and are not the gate's. The logs were kept in this session's scratchpad, not
committed.

**Unresolved obligations.** 243 (the markup half's ceiling, found by reading,
unverified by a run); 78's remainder; 77 needs design; 233, 234, 238 and 240
are open and item 76 is not done; `violations::reports` is still called by
nothing in the browser process on its own (item 203's dependency). No script
can reach the document. 110 queue items are open (242 closed, 243 added).
Next unused queue number **244**; next ADR **0017**. This is one iteration,
not a finished queue or roadmap.

---

## Iteration 140 — queue item 243: a ceiling on what a page's markup makes one load say

A page's markup can no longer make its own load report unsendable. Eight tests,
and item 244 cut from it: the font names a load asks for, bounded in length.

**The worker built all of this and did not commit it.** The supervisor found
the tree dirty and stopped for inspection, which is what it is for. This entry
and the one-line fix below are the only things added.

**What it was doing when it stopped, which is worth reading.** Its own words
are in the run log: it ran the full test file against the real code, then
against three *doctored* builds with each ceiling removed in turn — run A, the
count ceiling, failed five tests including the closing clause; run B, the line
ceiling, failed exactly the two long-line tests. That is mutation testing. It
is a better answer to "do these tests discriminate" than reading them, which
is what I do, and nobody asked it to.

**Why it did not finish.** It started the gate *in the background* and spent
its remaining turns waiting: *"The gate is running in the background; I'll
write the journal entry once it reports"*, then *"Still waiting on the gate."*
The session ended while it waited. It was neither hung nor blocked — it was
holding a door open for a result that arrived after it was gone.

So it never saw what the gate had to say, which was one clippy error.

**The error, and why the fix is a rename rather than an allow.**
`struct Kept` had a field `kept`, which `clippy::pedantic` rejects for telling
the reader nothing twice. The deeper problem is that `alo-net` already exports
a public `Kept` — the record of what an agent did — and this was a second,
unrelated `Kept` meaning a truncated line buffer. Two types of one name, one
repository.

Its own doc comment already had the better name in it: *"a line as it is
written"*. It is now `Line`, which fixes the lint and the collision together,
and the struct says why in a comment so nobody reintroduces the clash.

**Verified.** The gate is green on the result. The worker's eight tests are
unchanged; only the type's name moved.


---

## Iteration 141 — queue item 244: the font names a load asks for, bounded in length

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its *Errors and stack traces* line,
`docs/autonomy/REMAINING.md`, iterations 139 and 140 and their selection
reasoning, queue items 78 and 241–244, and the code the change touches:
`alo-renderer`'s `families.rs`, `pipeline.rs` (`each_issue`), `said.rs`,
`renderer.rs` (`load`, `lay_out`), `message.rs` (`Loaded`), `wire.rs`
(`LARGEST_MESSAGE`), `host.rs` (`supply`), `fonts.rs` (`named`), and
`alo-text`'s `family_in`, `LONGEST_NAME` and `FontDatabase::absent`. No ADR
governs this choice — the item names it the families module's, and ADR 0010
(a renderer may not look for a font) and ADR 0005 (the renderer's answer must
stay sendable) are unchanged by it. The feature is `docs/features.md`'s
*font asked for by name* line and the *Errors and stack traces* line that
carried 244 as owed. No `AGENTS.md` exists. No sibling repository was read or
modified. The checkout was clean on entry at `b89f4dd`.

**Selection.** Nothing landed in iteration 140 that unblocks an earlier
item: the blockers iteration 139 listed stand (240 and 238 wait for a frozen
page, 234 on 233, 233 on 81 or 92, 77 `needs design`, 78's remainder has no
closing condition). **244 was the first eligible item**: its dependency, 243,
is done, and its closing condition names no frozen page. Like 242 and 243 it
was found by building, not opened by a page.

**Found by a run first.** The item said *by reading, not yet by a run*. A
probe before any change: a page of 66,479,993 bytes — under the wire's
67,108,864 — naming one family of 63 MiB beside 254 pictures each saying the
longest line a load says, made an answer of 68,167,392 bytes, which the wire
refuses.

**The choice, and why.** The item left open whether a name longer than any
font's is *not asked for* or *asked for cut*. Cut asks for a different font —
one the machine might have — and the page would be drawn in a family it never
named, so: **not asked for, and said.** What "longer than any font's" means is
not invented here: `alo-text`'s `family_in` skips every `name` record over
`LONGEST_NAME` (512) bytes, and no decoding it does makes more characters than
bytes, so no font it reads has a family over 512 characters.
`families::LONGEST_FAMILY` is that constant, in characters (512 `é` is 1024
bytes here and could be a Macintosh-encoded font's family), and
`families::could_be_a_family` counts no further than one past it.

**What was built.** `families::wanted` leaves a too-long name out of
`Wanted::families` and says it in the new `Wanted::not_asked` — `a family
beginning "<first 32 characters>" and N characters long was not asked for: no
font states a family of more than 512 characters` — chained into
`Rendered::each_issue` just before the substitutions, so `said::of_markup`
bounds it with everything else. A substitution sentence names such a family
the same way rather than quoting all of it, so the render no longer holds a
debug-escaped copy of the name. The page's next choice is still asked for.
The `MOST_WANTED` check became `continue` instead of `break` so a too-long
name later in a list is still said; what is asked for is unchanged.
`host::Renderers::supply` answers a too-long name absent without
`fonts::named`'s look through every font file — bounding what a renderer sent
as it already bounded how many. `message.rs`'s doc for `wanted` states both
bounds.

**Evidence.** `crates/alo-renderer/tests/a_family_no_font_could_have.rs`,
five tests: the closing clause on the probe's page (no family asked for, the
not-asked line and the substitution said as the last two of 256, the answer
under `LARGEST_MESSAGE` and round-tripping the wire, the page message itself
asserted under the cap); 512 `a` and 512 `é` asked for, 513 `é` not; a
600-character name skipped and `Inter` after it asked for, the name twice in
one list said once; the browser process answering a 513-character and a
20,000,000-character name absent; every prefix of a page naming a
600-character family asking for nothing over 512 characters and sendable.
`families.rs` gains two unit tests (the boundary in characters and trimmed;
the wording).

**Doctored runs, two, each restored byte for byte (`cmp`)**: the renderer's
check disabled fails four of the five tests. The browser process's check
disabled fails **none** — without it the name is still answered absent, after
a look through every font file, so only the cost differs. That guard is
therefore not discriminated by a test, and that is recorded rather than
hidden; adding a seam only to observe it was not done.

**Roadmap.** The *Errors and stack traces* line's Built clause gains the
bound (244, `families::LONGEST_FAMILY`) and its Owed clause drops 244. Not
ticked: `error.stack`, source maps, developer tools and a non-error object
described are still owed. `docs/features.md` (both lines), `CHANGELOG.md`,
`REMAINING.md`, `message.rs`'s doc and the queue (244 ticked with its
evidence) move with it.

**Compliance review.** Law 1: nothing legacy. Law 2: unchanged. Law 3: no
stub, `todo!`, or `unwrap` outside tests; what is not asked for is said; no
speed claim — "without a look through every font file" is about work a page
can cause, not a measurement. Law 4: no `unsafe`. LOOP stage 2 § 2: the
name is bytes from a stranger, tested malformed (every prefix of the sheet)
and at its extremes (63 MiB, and the exact boundary), refusing rather than
panicking. One file, one responsibility: `families.rs` still decides which
families a load asks for and what it says about them; `host.rs`'s change is
one guard in the function that answers that ask. Nothing positions, sizes or
paints differently, so no layout assertion or reference render applies;
every existing reference render still matches (the gate's tests include
them).

**Gate.** `scripts/gate.sh` exited 0 on the second run, in the foreground.
The first failed only `cargo fmt --check` on one assertion in the new test;
`cargo fmt --all` fixed it, nothing allowed or silenced. Formatting clean,
clippy silent, all tests pass, nothing stubbed, `unsafe` forbidden, licence
notices present, every rented crate behind its boundary, no coordinate verb,
the supervisor's stop rule holds, `CHANGELOG.md` changed. `cargo test
--workspace` counts 2325 passed, 0 failed (2306 at iteration 139, iteration
140's 12, this item's 7). `git diff --check` passes. Logs kept in this
session's scratchpad, not committed.

**Unresolved obligations.** The browser-side guard's lack of a
discriminating test (above). 78's remainder; 77 needs design; 233, 234, 238
and 240 are open and item 76 is not done; `violations::reports` is still
called by nothing in the browser process on its own (item 203's dependency).
No script can reach the document. 109 queue items are open (244 closed).
Next unused queue number **245**; next ADR **0017**. This is one iteration,
not a finished queue or roadmap.


---

## Iteration 142 — item 80's decision: the document moves into the heap, and a wrapper lives as long as its tree

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its DOM and event-loop lines,
`docs/autonomy/REMAINING.md`, iterations 132 (the form an ADR-only iteration
takes), 136, 138, 139 and 141 and their selection reasoning, queue items 78,
79–93 and 241–244, ADR 0014 in full, ADR 0013 § 6, ADR 0016 §§ 4–8 and its
*What this does not decide*, ADR 0003's alternatives and consequences, and the
code the decision is about: `alo-dom`'s `lib.rs`, `node.rs` and
`document.rs` (the arena that never frees, the `pub(crate)` tree operations),
`alo-js`'s `object/internal.rs` (`Internal`, `Exotic`), `object/native.rs`
(a native is a plain `fn` handed the heap, `this` and arguments),
`heap/root.rs`, `heap/trace.rs` (`ephemeron`, `footprint`, `clear_weak`),
`Objects::foreign`, `alo-renderer`'s `renderer.rs` (`act`, `load`,
`lay_out`) and `event_loop.rs`, `alo-agent`'s `apply`, and `html5ever`
0.39's tokenizer (`TokenizerResult::Script`). No `AGENTS.md` exists. No
sibling repository was read or modified. The checkout was clean on entry at
`a529f24`.

**Selection.** Nothing landed in iteration 141 that unblocks an earlier item:
every blocker iteration 136 listed stands, 240 and 238 wait for a frozen
page, 234 on 233, 233 on 81 or 92, 77 is `needs design`, 78's remainder has
no closing condition. 241–244 are done. **79** depends on 73, open. **80**
depends only on 72, which is done, and was the first eligible item — but it
could not name the decision it implements: ADR 0014's *What this does not
decide* hands **the shape of the DOM bindings** to item 80 by name, and the
code confirms the gap is real rather than a formality — a native function is
handed the heap, its `this` and its arguments and nothing else, there is no
way for an embedder to get its own exotic object back, the document is owned
by `Rendered` outside any heap, and ADR 0003's *a detached node's slot is
not freed* rests on *bounded by the input*, which a script makes false.
`LOOP.md` stage 2 § 4 makes the decision its own iteration, before any code
depends on it — iteration 132's precedent for item 76.

**What was built: ADR 0017, accepted.**
`docs/decisions/0017-the-document-moves-into-the-heap-and-a-wrapper-lives-as-long-as-its-tree.md`.
§ 1 the `alo-bindings` crate, the only one naming both, one file per
interface, hand-written before generated. § 2 the document moves into the
page's heap as one rooted embedder cell when the page first runs script and
never moves back; everything else borrows it from there; its footprint is the
document's size, so the heap's ceiling bounds a page's DOM. § 3 a wrapper is
one per node, holds the id, the document cell and an ordinary object's part;
the document cell traces attached nodes' wrappers strongly and each detached
tree's wrappers as a **ring of ephemerons** (n pairs), so a detached tree lives
while any of its wrappers does; an unreachable detached tree is released at
the sweep with its ids left as tombstones — never reused, ADR 0003's promise
kept and its *bounded by the input* consequence retired. § 4 a native reaches
its node only through its `this`, by a typed borrow (`Any`) the engine gains;
a wrong `this` is the WebIDL brand check's `TypeError`. § 5 `alo-dom`'s tree
operations become public under the standard's names and pre-insertion
validity rules, for script and agent alike, refusing with the standard's
exception names; a change count; script-made nodes take ids from the parser's
counter; `DOMException`. § 6 a changed document is rendered again whole, from
the same document, when its rendering is read — `Paint`, `ReadTree`, an
`Act`'s decision, the end of a `Load`, a `Resize` (which stops re-parsing) —
never inside a task; forced layout is decided with the first geometry API.
§ 7 a parser-inserted classic script runs at its own end tag and sees the
document up to its own element. § 8 law 1's surface: no live collections, no
`document.write`; absent members are absent. Facts checked rather than
assumed: `alo-dom` never frees a node; `Trace` has `ephemeron`, `footprint`
and `clear_weak(&mut self)` called on survivors before anything is freed;
`html5ever` 0.39's tokenizer returns `TokenizerResult::Script(node)` when the
tree builder reaches a script's end tag (a first draft said the tree builder
"asks whether to suspend", which is not 0.39's interface, and was corrected
before committing).

**What is not built, and why it stops here.** No code, dependency or test.
Item 80 stays unticked and records its ADR and what it says; its code is cut
as **245** (`alo-dom`'s public operations, validity refusals, change count,
tombstones — no dependency, so next), **246** (the bindings, the typed
borrow, the document cell and wrapper liveness, `document` and item 80's
members, the renderer's re-render — item 80's closing condition, with layout
assertions in numbers and a reference render named in it) and **247** (a
script at its own end tag), each with a closing condition.
`docs/features.md` is unchanged: no capability changed, and its line for 80
describes the feature rather than claiming it (its event-loop line's *No
script can see the page's document yet (80)* is still true).

**Roadmap.** The *Mutation from script* line, which read as unstarted, gains
an Owed clause: all of the code is owed, the decision is ADR 0017, built as
245, 246 and 247. No Built clause — a decision is not a crate or a capability
(`ROADMAP.md`). Not ticked. `CHANGELOG.md` and `REMAINING.md` move with it.

**Compliance review.** Law 1: the ADR refuses live collections and
`document.write` by name and routes them to stage 3. Law 2: § 6 makes the
agent read a re-rendered tree after any change, and § 5 makes the agent and
script share one set of rules. Law 3: whole re-render, no incremental cache;
no speed claim — the cost of re-rendering is named as a measurement on
hardware. Law 4: no `unsafe` authorised; the typed borrow is safe Rust and
`unsafe_code = "forbid"` is unchanged. *Settled decisions*: ADRs 0003, 0013,
0014 and 0016 were read first and 0017 stays inside each — the engine still
learns no DOM type (0013 § 6), one wrapper per node and reachability decides
(0014 § 6), ids never reused (0003, whose memory consequence is explicitly
retired with its reason rather than quietly contradicted), rendering on the
browser process's frame (0016 § 5). `LOOP.md`: one item, a decision as its
own iteration, no tick for unfinished work, no gate or test changed, no page
manufactured. Each edited file keeps its single responsibility. Nothing
positions, sizes or paints, so layout assertions and reference renders do
not apply to this change (246 owes both). No push, no supervisor launched.

**Gate.** `scripts/gate.sh` exited 0, run in the foreground and read in the
same step: formatting clean, clippy silent, all workspace tests pass, nothing
stubbed, `unsafe` forbidden, licence notices present, every rented crate
behind its boundary, no coordinate verb, the supervisor's stop rule holds;
the documentation check reports no uncommitted code to judge, which is right
for a documentation-only change. `git diff --check` passes. The log was kept
in this session's scratchpad, not committed. That verifies the repository
still meets its gate; it says nothing about bindings that do not exist yet.

**Unresolved obligations.** Item 80's code, all of it (245, 246, 247). 78's
remainder; 77 needs design; 233, 234, 238 and 240 are open and item 76 is not
done; `violations::reports` is still called by nothing in the browser
process (item 203's dependency); the browser-side font-name guard of
iteration 141 still has no discriminating test. 112 queue items are open (245,
246 and 247 added). Next unused queue number **248**; next ADR **0018**. This
is one iteration, not a finished queue or roadmap.


---

## Iteration 143 — item 245: `alo-dom`'s tree operations, public, under the standard's rules

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its *Mutation from script* line,
`docs/autonomy/REMAINING.md`'s tail, iteration 142's entry and selection
reasoning, queue items 80 and 245–247, ADR 0017 in full (§§ 3 and 5 are this
item's), and the code it changes: `alo-dom`'s `lib.rs`, `document.rs`,
`node.rs` and `parse.rs`, and `alo-agent`'s `apply.rs`. No `AGENTS.md`
exists. No sibling repository was read or modified. The checkout was clean
on entry at `f27e36c`.

**Selection.** Iteration 142 left every earlier item blocked for reasons
that still stand (79 on 73; 233, 234, 238, 240 on their dependencies or a
frozen page; 77 `needs design`; 78's remainder with no closing condition)
and named **245** next: cut from 80, it depends on nothing.

**What was built.** In `alo-dom`, three new files with one reason each to
change: `validity.rs` — the DOM standard's *ensure pre-insertion validity*
and *replace a child* checks rule for rule, host-including ancestry through
a template's contents (bounded by the arena's size, so a malformed chain
refuses rather than spins), `createElement`'s *valid element local name*,
and `Refusal`, naming `HierarchyRequestError`, `NotFoundError` and
`InvalidCharacterError`; `mutation.rs` — `create_element` (lowercased; a
`<template>` gets its contents fragment, numbered after it),
`create_text_node`, `insert_before`, `append_child`, `replace_child`,
`remove_child` and `remove`; `release.rs` — releasing a detached tree,
template contents included, into tombstones. `document.rs` keeps the arena
(now `Option<Box<Node>>` per slot, so a tombstone is one pointer), a `host`
link from template contents to template (set by the parser and by
`create_element`), and `change_count`. The parser's own operations stay
crate-private, renamed `attach_last`/`attach_before` so the public names are
the standard's; `element_mut` became crate-private so nothing outside the
crate changes an element without the count hearing. A stale or foreign id
refuses as `NotFoundError` (not a case the standard has; the message says
so). What counts as a change is written on `change_count`: a successful
insertion, removal, replacement or attribute change; not a refusal, making
a node, releasing, removing an absent attribute, or parsing.

The agent's `apply` needed no change: it alters the document only through
the public `set_attribute`/`remove_attribute`, which now advance the count,
and it makes no tree change. That is the queue item's *the agent's `apply`
uses the public ones*, satisfied as it stood — recorded rather than
manufactured into an edit.

**Evidence.** `crates/alo-dom/tests/mutation.rs`, 20 tests, covering the
closing condition clause by clause: every insertion and replacement rule
refuses by name with every link of every node, the serialisation, the node
count and the change count unchanged; a forty-node page's first made
element is `#40` and the next text node `#41`; the count across ten
operations moves six times, on exactly the successes; a released tree's
ids answer `None`, refuse as `NotFoundError`, and the next id is one past
the highest ever made. The hostile half (LOOP stage 2 § 2 — the ids and
names will come from a stranger's script): every id the page has and three
it never made, in every position of every operation on a clone of a page
with a template, a comment, a doctype, a loose subtree and a released node,
each refusing or leaving a tree whose links agree and whose document holds
at most one doctype before at most one element and no text; a megabyte
element name; names with NUL, `>`, `/`, spaces and a leading digit. Unit
tests: two in `validity.rs`, two in `mutation.rs`, three in `release.rs`.
**Doctored runs**, each restored and the file checked identical: every one
of the nineteen refusals in `validity.rs` disabled alone. Eighteen failed a
test the first time; *a document goes nowhere* did not, because the
ancestor rule refused the same calls first — a case under a detached parent
was added, and that rule disabled alone then failed
`a_document_goes_nowhere`.

**Compliance review.** Law 1: no legacy surface; the operations are the
modern standard's, and no live collection or `document.write` exists. Law
2: ids stay the agent's names — a made node is numbered on the parser's
counter, a released id answers nothing rather than another node (ADR 0003).
Law 3: no stub, `todo!` or `unwrap` outside tests; the arena's own
`attach_*` refusal is unreachable after validity and is commented as such
rather than silenced. No speed claim. Law 4: no `unsafe`. ADR 0017 § 5 is
built as written (rules in `alo-dom`, the parser's operations kept apart,
a counter rather than a list) and § 3's release half as far as `alo-dom`
reaches; deciding *when* a tree is unreachable is the bindings' (246). One
file, one responsibility: rules, operations, release and the arena are four
files; `node.rs` gained one link field, `parse.rs` sets it. Nothing
positions, sizes or paints differently, so no new layout assertion or
reference render applies; every existing one still matches (the gate runs
them). `docs/features.md`'s line now states the document's half and that no
script reaches it.

**Gate.** `scripts/gate.sh` exited 0, run in the foreground and read in the
same step: formatting clean, clippy silent (two earlier runs fixed, not
silenced: missing `# Errors` sections, and test helpers that unwrapped
outside a `#[test]`), all tests pass, nothing stubbed, `unsafe` forbidden,
licence notices present, every rented crate behind its boundary, no
coordinate verb, the supervisor's stop rule holds, `CHANGELOG.md` changed.
`cargo test --workspace --all-features` counts 2352 passed, 0 failed (2325
at iteration 141, this item's 27). `git diff --check` passes. The log was
kept in this session's scratchpad, not committed.

**Roadmap.** The *Mutation from script* line gains a Built clause naming
`alo-dom` and item 245 and keeps an Owed clause for 246 and 247. Not
ticked. Queue item 245 ticked with its evidence; `CHANGELOG.md`,
`docs/features.md` and `REMAINING.md` moved with it.

**Unresolved obligations.** Item 80 stays open until 246 and 247 close; no
script can reach the document. The sweep's tree check does not by itself
notice a document node given a parent (the named test does). 78's
remainder; 77 needs design; 233, 234, 238 and 240 are open and item 76 is
not done; `violations::reports` is still called by nothing in the browser
process (item 203's dependency); iteration 141's browser-side font-name
guard still has no discriminating test. 111 queue items are open. Next is
**246**. Next unused queue number **248**; next ADR **0018**. This is one
iteration, not a finished queue or roadmap.


---

## Iteration 144 — item 246 cut; item 248: the document in the heap, and a wrapper that lives as long as its tree

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its *Mutation from script* line,
`docs/autonomy/REMAINING.md`'s tail, iteration 143's entry, queue items 80
and 245–247, ADR 0017 in full, ADR 0014 §§ 2, 5–8 as the heap's code states
them, and the code this builds on: `alo-js`'s `object/internal.rs`,
`object/cell.rs`, `object/native.rs`, `object.rs`, `heap.rs`,
`heap/trace.rs`, `heap/collect.rs` and `bounds.rs`; `alo-dom`'s
`document.rs`, `node.rs`, `mutation.rs` and `release.rs`; the renderer's
`event_loop.rs` and `event_loop/microtask.rs` for how an embedder installs a
native. No `AGENTS.md` exists. No sibling repository was read or modified.
The checkout was clean on entry at `2a7c909`.

**Selection.** Iteration 143 named **246** next, and every earlier item is
still blocked for the reasons it recorded (79 on 73; 233, 234, 238, 240 on
their dependencies or a frozen page; 77 `needs design`; 78's remainder with
no closing condition). 245 is done, so 246's dependency is met.

**The cut.** 246 is three changes to three crates, each with its own way to
be wrong — what a wrapper is and how long it lives (`alo-js`, `alo-dom`, a
new crate), the members a script calls, and the renderer handing its
document over and rendering again. LOOP step 3: cut scope, never depth, and
write the cut into the queue. So 246 stays open as the umbrella that closes
on its own (item 80's) condition, and is cut as **248** (built here), **249**
(the interfaces, `DOMException`, `document` on the global, the brand check,
and the script-level hostile half) and **250** (the renderer: adopt at the
first script, borrow for every reader, render again on a stale change count;
item 80's reference-render condition). 248 came first because it is the
clause a script observes and every member stands on.

**What was built.** `alo-js`: `Exotic` gains the supertrait `Typed`, blanket-
implemented for every type, and `Objects::embedded::<T>` and
`Objects::write_embedded::<T>` answer an embedder's own object by type and
`None` for any other cell — through the box, not of it (a `Box<dyn Exotic>`
is itself `Any`, and asking it answers for the box). ADR 0017 § 4 says the
trait *gains the one method*; it gains a supertrait instead, because the
workspace's `rust-version` (1.85) predates `dyn` upcasting to `Any`, and a
blanket impl means an embedder writes nothing. `alo-dom`: `footprint.rs`
says what a node owns (lengths, not capacities, so a clone answers the same)
and `Document` keeps the sum as nodes are made, edited and tombstoned —
every content edit now goes through one accounting helper and
`element_mut` became `edit_element` — because the heap measures a cell
before and after every write and a walk would make building a page
quadratic. `release` was rewritten to walk down unlinking each child and
climb back by the parent link (`take_first_child`): it now allocates
nothing and cannot recurse, since the bindings call it inside a sweep (ADR
0014 § 8). `next_detached_root` is a cursor for a caller releasing as it
goes. `alo-bindings` (new; the only crate naming both, ADR 0017 § 1):
`document_cell.rs` (the cell, the node-to-wrapper table, the pending node,
the released counts; no prototype, no properties, refuses any — no script
is ever handed it), `wrapper.rs` (node id, the document cell held strongly,
an ordinary part for expandos), `tree.rs` (the host-including pre-order
walk, going round, with a step budget), `liveness.rs` (the trace: every
wrapper in the document's tree a strong edge, each detached tree's wrappers
a ring of ephemerons in tree order; the sweep: dead wrappers leave the
table, and every detached tree with no wrapped and no pending node is
released and counted), `embed.rs` (`adopt`, `wrap`, `node_of`, `document`,
`change_document`).

**Two refinements of ADR 0017 § 3's mechanism; its rule is unchanged.**
(1) The ring is formed by walking each tree once per collection, wrappers
linked in tree order, rather than by asking each wrapper for its tree's
root — a chain a million deep with every link held would make the latter
quadratic at every collection. (2) A node whose wrapper is being made is
*pending*, and the sweep keeps its tree: `createElement`'s node is in a tree
of its own with no wrapper, and the allocation that makes its first wrapper
may collect — without this it would be released mid-wrap (the doctored run
below shows the stress test catching exactly that). Also: a walk that
overruns its budget keeps every wrapper strongly and releases nothing it
could not see round. None of the three changes what a script observes, so
no ADR amendment; recorded here and in the queue for whoever reads § 3
next.

**Evidence.** `crates/alo-bindings/tests/what_a_wrapper_keeps.rs`, 8 tests:
one node wrapped twice is one object and its expando survives three forced
collections with nothing rooting the wrapper, every parsed id unchanged; a
detached tree held through its child is kept while a never-wrapped text
node is released (`Released { trees: 1, nodes: 1 }`), then released when let
go (`{2, 3}`), table emptied, wrapper freed, change count untouched, the
released id refusing as `NoSuchNode` and the next id one past the highest;
one held wrapper keeps all three of its tree's; a detached template kept
through a node in its contents, then released with them (3 nodes), and a
parsed template's contents kept by the document; a node's wrapper moving
between the two rules as it is inserted and removed; twenty wrappers made
under stress collection, each node kept; nothing but a wrapper answers as a
node (`NotADocument`, `NoSuchNode` for an id this document never made); the
heap's held bytes grew by exactly the document's footprint growth for a
megabyte of attributes, and fell back once released.
`a_document_that_is_hostile.rs`, 4 tests (LOOP stage 2 § 2): a ring of
19384 wrappers (16384 + 3000) held by its last, which overflows the
marker's pair buffer, costs a rescan, and is kept whole; a 200001-node chain
held from the bottom, then released whole; a 20000-link chain in the page
with every link wrapped, unchanged across three collections; 4000 seeded
operations (make, insert, move, remove, wrap, hold, let go, collect) with
the heap's invariants and the cell's — held nodes alive, every wrapper for a
live node, no detached tree kept that nobody holds — checked after every
collection. `tree.rs`, 3 unit tests. `crates/alo-js/tests/
what_an_embedder_gets_back.rs`, 3 tests (two types alike but for their
type; a stale reference; a reference stored through the borrow traced).
`crates/alo-dom/tests/what_a_document_weighs.rs`, 4 tests (every footprint
recounted by hand after parse, clone and each kind of change; a 300001-node
chain released without recursion; the cursor finding exactly the detached
roots).

A first version of the hostile tests built chains top-down and ran for
minutes: inserting into a node checks its ancestry (the standard's rule),
so that is the depth per link. The tests now build bottom-up, and say why.
That cost belongs to the standard's insertion check, and it will be met
again by a page that appends ever deeper; recorded for 249's hostile half
rather than fixed here.

**Doctored runs**, each restored and checked identical by hash: thirteen
rules disabled alone — attached wrappers strong, the ring's forward pairs,
its closing pair, the release, the pending node, pruning the table, the
document counted in the cell's footprint, a template's contents walked, an
existing wrapper reused, the downcast through the box, a node edit weighed,
a tombstone weighed — each fails at least one test (rerun with
`--no-fail-fast`, attached wrappers fail four tests across both binding
test files, and the template walk fails two unit tests and the
integration test). **Not discriminated:** the
budget-overrun fallback in `liveness.rs`, which no document `alo-dom`'s
validity rules allow can reach.

**Compliance review.** Law 1: no legacy surface; nothing script-visible was
added. Law 2: node ids remain the agent's names — a released id answers
nothing and is never reused (ADR 0003, tested). Law 3: no stub, `todo!` or
`unwrap` outside tests; refusals are answered (`Wrapping`), the unreachable
post-allocation case answered rather than assumed. No speed claim; the
complexity reasoning (one walk per tree per collection) is stated as
reasoning, not measured. Law 4: no `unsafe`; `Any` downcasting is safe
Rust. One file, one responsibility: `alo-dom` gained `footprint.rs`; the
bindings are five files with one reason each; `document.rs` gained only
the accounting its own edits need. ADR 0017 §§ 1–4 built as far as this
cut reaches, with the refinements above. Nothing positions, sizes or paints
differently, so no new layout assertion or reference render applies, and
every existing one still matches (the gate runs them). `docs/features.md`'s
item states the engine's half and what is still owed.

**Gate.** `scripts/gate.sh` exited 0, run in the foreground and read in the
same step: formatting clean, clippy silent (one run fixed, not silenced: a
doc word, and a test function over 100 lines split by extracting its check
into a helper), all tests pass, nothing stubbed, `unsafe` forbidden, every
file carries the licence notice, every rented crate behind its boundary, no
coordinate verb, the supervisor's stop rule holds, `CHANGELOG.md` changed.
`cargo test --workspace --all-features` counts 2374 passed, 0 failed (2352
at iteration 143, this item's 22). `git diff --check` passes. The log was
kept in this session's scratchpad, not committed.

**Roadmap.** The *Mutation from script* line's Built clause gains the
document in the heap and its wrappers' lifetime (item 248); its Owed clause
now names 249, 250 and 247. Not ticked. Queue: 246 cut and left open, 248
ticked with its evidence, 249 and 250 added; `CHANGELOG.md`,
`docs/features.md` and `REMAINING.md` moved with it.

**Unresolved obligations.** Item 80 and 246 stay open until 249, 250 and 247
close; no script can reach the document yet. Inserting at depth *d* costs
*d* (249's hostile half must face a page appending ever deeper). The
budget-overrun fallback is undiscriminated. 78's remainder; 77 needs
design; 233, 234, 238 and 240 are open and item 76 is not done;
`violations::reports` is still called by nothing in the browser process
(item 203's dependency); iteration 141's browser-side font-name guard still
has no discriminating test. 113 queue items are open. Next is **249**. Next
unused queue number **251**; next ADR **0018**. This is one iteration, not a
finished queue or roadmap.

## Iteration 145 — item 249: the interfaces a script calls

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its *Mutation from script* line, iteration
144's entry, queue items 80, 221, 228 and 246–250, ADR 0017 in full, and the
code this builds on: `alo-bindings` (all of it), `alo-js`'s `realm.rs`,
`interpret.rs`'s embedder surface, `abrupt.rs`, `object/native.rs`,
`builtin/error.rs`, `interpret/catch.rs` and the renderer's
`event_loop/microtask.rs` for how an embedder installs a native; `alo-dom`'s
`document.rs`, `node.rs`, `name.rs`, `mutation.rs` and `validity.rs`. No
`AGENTS.md` exists. No sibling repository was read or modified. The checkout
was clean on entry at `a4367b1`.

**Selection.** Iteration 144 named **249** next and its dependency, 248, is
done; every earlier open item is still blocked for the reasons iteration 144
recorded.

**What was built.** `alo-bindings`: `interface.rs` lists the ten interfaces
in the DOM standard's chain (`Node`; `CharacterData` with `Text`, `Comment`,
`ProcessingInstruction`; `Element`, `Document`, `DocumentType`,
`DocumentFragment`; `DOMException` from `Error.prototype`), which a node of
each kind is, and holds their prototypes — **in the document cell**, as
strong edges, because a native is handed only its `this`, and the wrapper it
is called on holds that cell. One file per interface with members:
`interface/node.rs`, `element.rs`, `document.rs`, `child_node.rs` (the
mixin's `remove()`, on `Element`, `CharacterData` and `DocumentType`) and
`dom_exception.rs` (an embedder cell; `name` and `message` are prototype
getters, as Web IDL has them). `idl.rs` is Web IDL's half: the brand check,
the argument count, a `Node` argument of the same document, `ToString` asked
of the interpreter for an object. `define.rs` is Web IDL's property
attributes; `install.rs` makes the prototypes (`furnish`) and puts
`document` on the global object (`install`). `alo-dom`: `by_name.rs`
(attributes by qualified name, lowercased on an HTML element, refused by the
*valid attribute local name* rule, the first match only), `set_data` in
`document.rs` and `replace_all_with_text` in `mutation.rs` (*string replace
all*, counted once). `alo-js`: `Engine::intrinsics` (the realm's intrinsics
beside the heap, so an embedder can hang prototypes from them) and
`Missing::ASecondArgumentBehindACall`. Text has no item-80 member, so its
file was not written; `Text.prototype` is in the chain, empty, and
`interface.rs` says why.

**Decisions taken inside the item, none changing ADR 0017's rules.**
(1) The prototypes live in the document cell: the ADR says a native reaches
its node through its `this`; this applies the same to what it must make.
(2) `document` is a non-writable, non-configurable **data** property, not
Web IDL's accessor: a getter native would be handed only the global object,
an ordinary object with nowhere to keep the document. Every member a script
has today observes the two alike; a descriptor would not. Queue item **251**
added. (3) `setAttribute` with **both** arguments objects is refused by name
rather than converting the first twice (a native keeps a step number only,
and the first answer is overwritten by the second); item 221 now owes it.
(4) A lone surrogate becomes U+FFFD crossing into `alo-dom`'s UTF-8, as it
does from the parser. (5) No interface object (`Node`, `DOMException`) on
the global object; absent, like every member outside item 80's list.

**Evidence.** `crates/alo-bindings/tests/what_a_script_does_to_its_document.rs`,
8 tests; `a_script_that_is_hostile_to_its_document.rs`, 4 tests (stage 2 § 2:
a node into its own child six ways, refused; a loop appending a mebibyte of
text until the heap's ceiling, stopped with `Full` after more than 500 nodes
and the heap unbroken; a million `createElement`s released — a million trees,
one wrapper left, the next id past them all; every member run with the
collector at every allocation); `alo-dom`: 2 unit tests in `by_name.rs`, 1 in
`validity.rs`, 2 in `tests/mutation.rs`; `alo-js`: 1 in
`tests/what_an_embedder_gets_back.rs` and an assertion in `abrupt.rs`. The
queue entry lists what each asserts. The two long hostile tests take about 19
and 14 seconds in a debug build; that is a statement about the test, not a
speed claim.

**Doctored runs**, each restored and checked identical by hash: twenty-one
rules disabled alone (listed in the queue entry), each failing at least one
test. The first pass found two gaps, both closed before committing:
`insertBefore(node, null)` was never exercised (the main-test script now
uses it) and one pattern had moved under `cargo fmt` (rerun, discriminated).

**Measured, and left as it is:** the heap's ceiling is enforced where it
allocates, and a document change is a write — so the write that adds the
last node can take the heap past the ceiling by that one change (a mebibyte
in the test, asserted as the bound) before the next allocation stops the
script. ADR 0017 § 2's rule — counted, collected, stopped with a reason —
holds; how far past one write may go is a question for the heap's own
items, recorded here rather than solved inside this one.

**Compliance review.** Law 1: nothing legacy — no live collections, no
`document.write`, no `code` on `DOMException`; every other member absent and
tested absent. Law 2: script-made nodes take the parser's counter and every
parsed id survives (tested); the agent's tree is unchanged. Law 3: no stub,
`todo!` or `unwrap` outside tests; what is not built is refused by name.
Law 4: no `unsafe`. One file, one responsibility: one per interface,
conversions apart from definitions apart from installation; `alo-dom`'s
by-name rules in a file of their own. Test helpers panic only through macros
expanded inside `#[test]` functions, matching existing tests, rather than
silencing the lint. Nothing positions, sizes or paints differently, so no
layout assertion or reference render applies, and every existing one still
matches (the gate runs them). `docs/features.md`'s item says what a script
can now do and what is still owed.

**Gate.** `scripts/gate.sh` exited 0, run in the foreground and read in the
same step: formatting clean, clippy silent (one round fixed, not silenced:
`similar_names` on `cell`/`call`, renamed to `owner`; panics in test helpers
moved into macros), all tests pass, nothing stubbed, `unsafe` forbidden,
every file carries the licence notice, every rented crate behind its
boundary, no coordinate verb, the supervisor's stop rule holds,
`CHANGELOG.md` changed. `cargo test --workspace --all-features` counts 2392
passed, 0 failed (2374 at iteration 144, this item's 18). `git diff --check`
passes. The log was kept in this session's scratchpad, not committed.

**Roadmap.** The *Mutation from script* line's Built clause gains the
interfaces a script calls (item 249); its Owed clause now names 250 and 247.
Not ticked. Queue: 249 ticked with its evidence, 221 extended, 251 added;
`CHANGELOG.md`, `docs/features.md` and `REMAINING.md` moved with it.

**Unresolved obligations.** Item 80 and 246 stay open until 250 and 247
close: no page's script reaches the document yet. `document`'s shape is item
251. Two object arguments to `setAttribute` are item 221's. The one-write
overshoot of the heap's ceiling is measured, not bounded below one change.
Inserting at depth *d* still costs *d* (the standard's ancestor check); the
hostile tests here append flat. 248's budget-overrun fallback is still
undiscriminated. 78's remainder; 77 needs design; 233, 234, 238 and 240 are
open and item 76 is not done; `violations::reports` is still called by
nothing in the browser process (item 203's dependency); iteration 141's
browser-side font-name guard still has no discriminating test. 113 queue
items are open. Next is **250**. Next unused queue number **252**; next ADR
**0018**. This is one iteration, not a finished queue or roadmap.

## Iteration 146 — item 250: the renderer hands its document to script and renders what script left

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its *Mutation from script* line, iteration
145's entry, queue items 80, 246–251 and 247, ADR 0017 in full, and the code
this changes: `alo-renderer`'s `renderer.rs`, `pipeline.rs`, `scripts.rs`,
`said.rs`, `event_loop.rs`; `alo-bindings`' `embed.rs`, `install.rs`,
`document_cell.rs`; `alo-js`'s `heap.rs` allocation and `object.rs`'s
`foreign`; `alo-dom`'s change count; `alo-agent`'s `apply`; `alo-corpus`'s
harness. No `AGENTS.md` exists. No sibling repository was read or modified.
The checkout was clean on entry at `4fcb038`.

**Selection.** Iteration 145 named **250** next; its dependency, 249, is
done, and every earlier open item is still blocked for the reasons iteration
144 recorded.

**What was built.** `alo-renderer/src/held.rs`: `Held` is where a page's
document is — `Parsed` (the renderer's) until the page's first script that
may run is about to, then `Scripted` (an event loop whose heap holds the
document cell, behind one `Root`). Every reader borrows it, `apply` changes
it through it, and `scripted()` makes the loop, adopts, roots and installs.
`pipeline::draw` borrows a document and answers a `Drawing`; `Rendered` is
a parsed document and its drawing. `Renderer` keeps the held document, the
last drawing and the change count it was drawn at, and draws again whole
when the count has moved — at `Paint`, `ReadTree`, before an `Act`'s
decision and after its change; once at the end of a `Load`, after its
scripts; and from the document it has at a `Resize` (no more re-parse).
`alo-js`: `Heap::allocate_or_back`, `Objects::foreign_or_back`,
`Typed::into_any` — a refused cell goes back to its maker, and the heap now
checks a slot can be named before placing a cell rather than after.
`alo_bindings::adopt` answers `Unadopted` with the document. `alo-corpus`:
`rendering.rs` loads a case that carries script through a `Renderer`;
`check` takes a document and its drawing; new case `a-script-grows-a-list`.

**Decisions taken inside the item, none changing ADR 0017's rules.**
(1) `Rendered` keeps its document for the corpus, and the renderer keeps a
`Drawing` alone — the ADR's *`Rendered` stops owning it* applied to the
renderer, where the document can be in the heap, rather than to markup the
corpus renders. (2) A document the heap refuses is handed back and the page
is drawn as one that ran no script, with the refusal said; the engine gained
the hand-back rather than the renderer re-parsing, which would mint the same
ids only by the accident of a deterministic parser. (3) The corpus loads a
scripted case through the renderer, since a reference render of markup
whose script never ran is a page nobody sees; such a case may not link a
sheet or picture yet (`Page` has no room for them), refused by name.
(4) `Act` now redraws by the change count rather than by `apply`'s answer,
since the count is the ADR's contract and `apply` changes only through
counted operations.

**Evidence.** `crates/alo-corpus/cases/a-script-grows-a-list` (reference
render looked at: three rows, the third green, the second's text changed,
the paragraph gone; `layout.txt` has the new row at (8, 89.875)
184×25.296875); `crates/alo-renderer/tests/what_a_script_left.rs`, 11 tests
(listed in the queue entry, the layout asserted box by box);
`crates/alo-corpus/src/rendering.rs`, 3 unit tests;
`crates/alo-js/tests/what_an_embedder_gets_back.rs`, 1 test. Doctored runs,
each restored and checked identical by hash: nine rules disabled alone, each
failing at least one test (the queue entry names them). One first pass was
not a real doctoring — it drew twice rather than before the scripts — and
was redone; cargo stops at the first failing test binary, so the counts in
the scratchpad log are lower bounds.

**Found.** A thrown `DOMException` is reported as `uncaught: an object`:
`described.rs` names only the engine's own errors. Queue item **252**.

**Compliance review.** Law 1: nothing legacy added. Law 2: the agent reads
the tree drawn from the script's document; a script-made node is `#N` past
every parsed one and acted on by name. Law 3: no stub, `todo!` or `unwrap`
outside tests; the two "not reached" answers (a heap returning a different
cell, a parsed page not becoming scripted) are answered, not assumed.
Law 4: no `unsafe`. One file, one responsibility: where the document is
(`held.rs`) apart from answering messages (`renderer.rs`); how a corpus case
is rendered (`rendering.rs`) apart from checking it (`check.rs`). Clippy's
`large_enum_variant` on `Held` was fixed by boxing, not silenced. Layout
assertion in numbers and a reference render: both, above. Every existing
reference render still matches (the gate runs the corpus). Stage 2 § 2: the
hostile half is a script that throws mid-change and one that removes its
root element; both drawn, and every message still answered.

**Gate.** `scripts/gate.sh` exited 0, run in the foreground and read in the
same step: formatting clean, clippy silent, all tests pass, nothing stubbed,
`unsafe` forbidden, every file carries the licence notice, every rented
crate behind its boundary, no coordinate verb, the supervisor's stop rule
holds, `CHANGELOG.md` changed. `cargo test --workspace --all-features`
counts 2407 passed, 0 failed (2392 at iteration 145, this item's 15).
`git diff --check` passes. The log was kept in this session's scratchpad.

**Roadmap.** The *Mutation from script* line's Built clause gains a page's
script reaching the document and the page drawn from what it left (item
250); its Owed clause now names only 247. Not ticked: item 80 closes when
247 has. Queue: 250 and 246 ticked with their evidence, 252 added;
`CHANGELOG.md`, `docs/features.md`, `docs/conformance.md` and `REMAINING.md`
moved with it.

**Unresolved obligations.** The renderer's path for a document the heap
refuses is not discriminated by a test (it needs a document over the heap's
1 GiB ceiling); its engine half is. Every script still sees the whole parsed
document (247). A thrown `DOMException` is not named in a report (252).
`document`'s shape is item 251. Carried from before: the one-write overshoot
of the heap's ceiling; 248's undiscriminated overrun fallback; 78's
remainder; 77 needs design; 233, 234, 238 and 240 open and item 76 not done;
`violations::reports` still called by nothing in the browser process (item
203's dependency); iteration 141's browser-side font-name guard still has no
discriminating test. 112 queue items are open. Next is **247**. Next unused
queue number **253**; next ADR **0018**. This is one iteration, not a
finished queue or roadmap.

## Iteration 147 — item 247: a parser-inserted script sees the document up to its own element

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its *Mutation from script* line, iterations
145 and 146's entries, queue items 80, 245–253 and 247, ADR 0017 in full
(§ 7 is this item's), and the code this changes: `alo-dom`'s `parse.rs`,
`scripts.rs`, `release.rs`, `document.rs`; `alo-bindings`' `document_cell.rs`,
`liveness.rs`, `embed.rs`; `alo-renderer`'s `scripts.rs`, `held.rs`,
`renderer.rs`, `pipeline.rs`; html5ever 0.39's `driver.rs`, `Tokenizer::feed`
and the tree builder's `</script>` rule; `alo-js`'s collection trigger
(`COLLECT_AFTER`). No `AGENTS.md` exists. No sibling repository was read or
modified. The checkout was clean on entry at `336347d`.

**Selection.** Iteration 146 named **247** next; its dependency, 246, is
done, and every earlier open item is still blocked for the reasons iteration
144 recorded.

**What was built.** `alo-dom`: `Parsing` in `parse.rs` (still the only file
naming html5ever) — `start` hands out the document; each `resume` is lent it
`&mut`, feeds html5ever's tokenizer to the next `TokenizerResult::Script` or
the end, and gives it back; `parse_document` is that parse run to its end.
The sink records each HTML `<meta>` it makes (`take_metas`).
`Document::is_being_parsed` holds from `start` to the end, and `release`
refuses while it does. `scripts::prepared` and `scripts::stated` answer, per
element, what `carried` answers for a whole page, plus HTML's *not
connected, return*. `alo-renderer`: `scripts::at_load` drives the parse
through `Held::change`, takes the `<meta>` policies made before each stop,
and runs the script there; `load` starts the `Parsing`. Corpus case
`a-script-beside-itself`.

**Decisions taken inside the item, none changing ADR 0017's rules.**
(1) **The document is lent to each step by a swap.** html5ever's tree
builder outlives every borrow, so its sink cannot hold a `&mut Document`;
the document is moved into the sink for the step and out before `resume`
returns, with the caller's exclusive borrow (out of the heap cell, through
`change_document`) held throughout. One owner at every moment, nothing able
to see the document half way — § 2's rule, kept in substance; the spelling
is a move because of the rented crate's shape, and `parse.rs` says so.
(2) **No release while the parse lasts.** The tree builder holds open
elements no wrapper marks; a collection would otherwise tombstone a `<body>`
a script detached while the parser was still inserting into it, and the
parser would ask a tombstone its name. Which nodes it holds is html5ever's
and not exposed, so the rule is whole-document and conservative: what a
script drops during a load is released at the first collection after it,
counted against the heap's ceiling until then. Recorded in `release.rs`.
(3) **A `<meta>` policy is read as the parser made it**, so removing the
tag does not lift the policy (HTML). A `<meta>` a script inserts states
nothing here, where HTML would apply it — the narrower rule, written down
in `scripts.rs` rather than approximated. (4) **`document.body` is cut** to
item **253**: it is not one of item 80's members, a getter without the
setter would be an approximation (ADR 0013 § 3), and the closing
condition's node is reached as `document.documentElement.lastChild`, which
is the body while it is being parsed. (5) A `<script>` still open at the
end of the markup no longer runs — HTML marks it already started; the old
whole-document walk ran it.

**Evidence.** `crates/alo-renderer/tests/a_script_at_its_own_end_tag.rs`
(10 tests), `crates/alo-dom/tests/a_parse_in_steps.rs` (9 tests), 3 unit
tests in `alo-dom`'s `scripts.rs`, and the corpus case
`a-script-beside-itself`: reference render looked at — *Older*, the
script's green *From the script*, *Newer* — and `layout.txt` with the
script's row at (8, 64.578125) 184×25.296875; the renderer test asserts the
same order box by box at (0, 0), (0, 20), (0, 40), each 300×20. The queue
entry lists what each test asserts. **The existing script tests were
re-read against the new order**, as the item requires: the order tests run
pure script, the policy tests' `<meta>`s already governed only what
followed, `what_a_script_left.rs`'s scripts are each last in their body,
the prefix test asks only that every prefix loads, and
`a-script-grows-a-list`'s script is last in its `<main>` — all pass
unchanged.

**Doctored runs**, each restored and checked identical by hash (log in the
scratchpad): nine rules disabled alone, each failing at least one test —
release while parsing, `prepared` requiring connected, `stated` requiring
connected, `resume` refusing a document it did not start, the end clearing
*being parsed*, stopping at an end tag at all, policies read as the parser
made them, the sink recording `<meta>`s, the corpus case's row order. The
first pass found one gap, closed before committing: the renderer's
collection test did not fail with release-while-parsing disabled, because
the script's own register still held the body's wrapper through the
collection; the removal moved into a function whose frame is gone first,
and it now fails. Cargo stops at the first failing test binary, so the
lists in the log are lower bounds.

**Compliance review.** Law 1: no `document.write`, nothing legacy; the
parser continues with exactly the markup it had. Law 2: the agent's tree is
the document's; a script's node is numbered before what the parser reads
after it (tested). Law 3: no stub, `todo!` or `unwrap` outside tests; the
heap cell ceasing to be a document mid-parse is answered with an issue, not
assumed. Law 4: no `unsafe`. One file, one responsibility: html5ever stays
in `parse.rs` alone (the gate checks it); per-element script rules beside
the whole-page ones in `scripts.rs`; the renderer's `scripts.rs` now also
drives the parse, which is the same reason to change — *when a page's
scripts run as it loads* — since a script runs only at a stop. Stage 2 § 2:
every prefix of a page with scripts parses in steps to the same document;
ten thousand scripts are ten thousand stops; a script that removes the body
the parser is in and forces a collection, and one that removes its own
element, both leave a page that is drawn and answers. Layout assertion in
numbers and a reference render: both, above; every existing reference
render still matches (the gate runs the corpus).

**Gate.** `scripts/gate.sh` exited 0, run in the foreground and read in the
same step (6 min 54 s): formatting clean, clippy silent, all tests pass,
nothing stubbed, `unsafe` forbidden, every file carries the licence notice,
every rented crate behind its boundary (html5ever included), no coordinate
verb, the supervisor's stop rule holds, `CHANGELOG.md` changed.
`cargo test --workspace --all-features` counts 2429 passed, 0 failed (2407
at iteration 146, this item's 22). `git diff --check` passes. Logs kept in
this session's scratchpad, not committed.

**Roadmap.** The *Mutation from script* line is **ticked**: item 80 said it
closes when 245, 246 and 247 have, and all three are done — its closing
condition met by item 250 (iteration 146) and the order settled here. The
line names its remainder, each an item of its own: `document.body` (253),
`document` as Web IDL's accessor (251), a thrown `DOMException` named in a
load's report (252), rendering only what changed (113). Queue: 247 and 80
ticked with their evidence, 253 added; `CHANGELOG.md`, `docs/features.md`,
`docs/conformance.md` and `REMAINING.md` moved with it.

**Unresolved obligations.** A `<meta>` policy a script inserts is not
applied (HTML would). Detached trees a script drops during a load wait for
the first collection after the parse. 253, 251 and 252 as above. Carried
from before: the renderer's path for a document the heap refuses is not
discriminated (it needs a document over the heap's 1 GiB ceiling); the
one-write overshoot of the heap's ceiling; 248's undiscriminated overrun
fallback; 78's remainder; 77 needs design; 233, 234, 238 and 240 open and
item 76 not done; `violations::reports` still called by nothing in the
browser process (item 203's dependency); iteration 141's browser-side
font-name guard still has no discriminating test. 111 queue items are open.
Next is **252** (it depends on nothing); closing 80 also unblocks 81, 85,
87, 88 and 89. Next unused queue number **254**; next ADR **0018**. This is
one iteration, not a finished queue or roadmap.

## Iteration 148 — item 252: a thrown `DOMException` is reported by its name

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s *Mutation from script* line, iteration 147's entry, queue
items 80, 249–253, and the code this changes and reads: `alo-renderer`'s
`event_loop/described.rs`, `event_loop/report.rs` and its error-report
tests; `alo-bindings`' `interface/dom_exception.rs` and each member that
throws (`node.rs`, `element.rs`, `document.rs`); `alo-dom`'s `Refusal`
and the rules that make one; `alo-js`'s `Objects::embedded`. ADR 0017 § 5
(what a refusal throws) and ADR 0013 § 3 apply; no ADR is changed. No
`AGENTS.md` exists. No sibling repository was read or modified. The
checkout was clean on entry at `30f1667`.

**Selection.** Iteration 147 named **252** next; it depends on nothing, and
every earlier open item is still blocked for the reasons iteration 144
recorded.

**What was built.** `described::thrown` asks a thrown object, by type,
whether it is `alo-bindings`' `DomException` cell, and says one as
`name: message` from the cell's own two slots — what its getters answer and
what no page can change — ahead of the error and plain-object cases, which
are unchanged. **Decision inside the item:** the slots, not the properties
along the chain. Those are getters (calling one would run code), and a page
that deleted or replaced them would choose what its own failure says — the
same reason an error's `toString` is not consulted. Recorded in
`described.rs`. The words are what `e.name + ': ' + e.message` gives an
untouched exception, matching how an error is said.

**Evidence.** `crates/alo-renderer/tests/a_dom_exception_said_by_its_name.rs`
(7 tests; the queue entry lists what each asserts) — all six members of item
80's that throw, eight refusals across all three names, each said exactly
and each the same words the page's own `catch` read; a page that deletes
both getters, plants its own name and a counting `message` getter is
reported by the exception's own words with the getter run zero times; an
object inheriting from `DOMException.prototype`, a look-alike plain object
and a renamed `Error` said as before; a refusal thrown through a function
said with both places; two refusals in two scripts each said.
`what_a_script_left.rs`'s hostile test now asserts the name it used to
leave to this item. No layout or pixel changes, so no layout assertion or
reference render applies; every committed reference render still matches
(the gate runs the corpus).

**Doctored runs**, each restored and checked identical by hash: the
`DomException` case removed (five of seven fail, and `what_a_script_left`);
the exception read as an error's properties rather than its slots (five
fail). Asking by type rather than by prototype was not doctored — no small
edit spells the wrong rule — and the inheriting-object test pins it.

**Compliance review.** Law 1: nothing legacy (no `code` attribute). Law 2:
untouched. Law 3: no stub, `todo!` or `unwrap` outside tests; reporting
runs no page code (tested). Law 4: no `unsafe`. One file, one
responsibility: `described.rs` is still *a thrown value in words*; the
renderer already depended on `alo-bindings`. Stage 2 § 2: the hostile half
is the tampering page above; the report reads only `'static` text the
engine made.

**Gate.** `scripts/gate.sh` first failed on `cargo fmt` alone (the new test
file); after `cargo fmt --all` it exited 0, run in the foreground and read
in the same step (about 6½ minutes): formatting clean, clippy silent, all
tests pass, nothing stubbed, `unsafe` forbidden, licence notices, every
rented crate behind its boundary, no coordinate verb, the stop rule holds,
`CHANGELOG.md` changed. `cargo test --workspace --all-features` counts 2436
passed, 0 failed (2429 at iteration 147, this item's 7). `git diff --check`
passes. Logs kept in this session's scratchpad, not committed.

**Roadmap.** The *Mutation from script* line was already ticked (iteration
147) and named 252 in its remainder; that remainder now records 252 built,
with its test, and names only 253, 251 and 113. Not a new tick. Queue: 252
ticked with its evidence; `CHANGELOG.md`, `docs/features.md` and
`REMAINING.md` moved with it. `docs/conformance.md` does not track script
reports, so it has nothing to move.

**Unresolved obligations.** Any other object a page throws is still said
as `an object` (item 78). Carried from before: a `<meta>` policy a script
inserts is not applied; detached trees a script drops during a load wait
for the first collection after the parse; the renderer's path for a
document the heap refuses is not discriminated (it needs a document over
the heap's 1 GiB ceiling); the one-write overshoot of the heap's ceiling;
248's undiscriminated overrun fallback; 78's remainder; 77 needs design;
233, 234, 238 and 240 open and item 76 not done; `violations::reports`
still called by nothing in the browser process (item 203's dependency);
iteration 141's browser-side font-name guard still has no discriminating
test. 110 queue items are open. Next is **253** (`document.body`, depends
only on 249; 251 waits on item 73). Next unused queue number **254**; next
ADR **0018**. This is one iteration, not a finished queue or roadmap.

## Iteration 149 — item 253: `document.body`

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s *Mutation from script* line, iteration 148's entry, queue
items 80, 81, 247 and 249–253, ADR 0017 (all of it; §§ 4, 5 and 8 apply)
and ADR 0013 § 3, `docs/features.md`'s *Mutation from script* entry, and
the code this changes and reads: `alo-dom`'s `mutation.rs`, `validity.rs`,
`name.rs`, `parse.rs` and `lib.rs`; `alo-bindings`' `idl.rs`, `define.rs`,
`interface.rs` and `interface/{document,node,element,dom_exception}.rs`
and their tests; `alo-renderer`'s `a_script_at_its_own_end_tag.rs` and
`a_dom_exception_said_by_its_name.rs`; `alo-corpus`'s `case.rs` and
`lib.rs`. No ADR is changed. No `AGENTS.md` exists. No sibling repository
was read or modified. The checkout was clean on entry at `679d52a`.

**Selection.** Iteration 148 named **253** next; it depends only on 249
(done). Every earlier open item is still blocked for the reasons iteration
144 recorded, and 251 waits on item 73.

**What was built.** `alo-dom`'s new `body.rs`: `Document::body` is HTML's
*the body element* — the first `body` or `frameset` child of the document
element when that is an HTML `html` — and `Document::set_body` is the
setter's algorithm: not a `body`/`frameset` (`null` included) is
`HierarchyRequestError`; the same element changes nothing; otherwise the
body element is replaced through `replace_child`, or with none the new one
is appended to the document element through `append_child`, and with no
document element it is `HierarchyRequestError`. The rules are in `alo-dom`
for ADR 0017 § 5's reason. `alo-bindings`: `Document.prototype.body`, an
accessor with both halves (ADR 0013 § 3: a getter alone would be
approximate); the setter's value is converted as Web IDL's `HTMLElement?`
by the new `idl::nullable_html_element` — `null`/`undefined` are no
element, anything not an element in the HTML namespace (an SVG element,
text, the document, a primitive) is a `TypeError`, another document's
node the existing cross-document `TypeError`.

**Decision inside the item:** `frameset` counts, as the standard says —
against the queue entry's own sketch, which left it out under law 1. Law 1
refuses to *render* frames; it does not license a `document.body` that
answers `null` where every other engine answers an element, or a setter
that appends beside a frameset instead of replacing it. That would be the
approximate member ADR 0013 § 3 refuses, and the name test costs one
comparison. Recorded in `body.rs`'s module comment and the queue entry.

**Evidence.** `crates/alo-dom/tests/the_body_element.rs` (14 tests),
`crates/alo-bindings/tests/the_body_a_script_reads_and_replaces.rs` (8),
`crates/alo-renderer/tests/the_body_a_page_reads_and_replaces.rs` (3) —
the queue entry lists what each asserts. **Layout assertion in numbers:**
a page whose script assigns a new body is laid out with `Kept` at (0, 0)
and `Made` at (0, 20), each 300×20, the old body's `Dropped` row nowhere,
and the agent's tree agreeing. **Reference render:** the new corpus case
`a-script-gives-a-new-body`, looked at — *Inbox* and the green *Three new
messages*, no red *Loading*; `layout.txt` has the line at
(8, 39.28125) 184×24.296875 and `issues.txt` is empty. Every other
committed reference render still matches (regenerating references touched
no other case; the gate runs the corpus). Item 247's closing sentence —
*a mid-body script reads `document.body.lastChild` as its own `<script>`*
— now has a test written exactly that way.

**Doctored runs**, each restored and checked identical by hash: seven
rules disabled alone — `frameset` not counted (3 fail), any document
element taken as the html element (2), the same body not short-circuited
(1), the last rather than the first such child (1), `null` taken as no
change (1), the HTML-namespace conversion (1), the setter absent (6).
**A mistake in the doctoring, caught and redone:** the first script
restored each file with `mv` from a backup older than the doctored build,
so cargo kept the doctored binary — the suites then failed on correct
source, which is how it was noticed. Runs 1–5 each rewrote `body.rs` fresh
and so were valid; runs 6 and 7 had run 5's edit still compiled into
`alo-dom` and were redone with a `touch` after every restore (results
above are the redone ones), then all three suites were rerun clean.

**One existing test changed, and why.** `what_a_script_does_to_its_document.rs`'s
`every_other_member_is_absent` listed `typeof document.body` among the
absent members; it is present now, by this item's purpose. It was replaced
in that list by `document.head`, still absent, so the test still asserts
fourteen absent members. `a_script_at_its_own_end_tag.rs`'s module comment,
which said `document.body` did not exist, now says it came after.

**Compliance review.** Law 1: no frame rendering, no legacy surface; `head`
and the rest stay absent. Law 2: the agent reads the assigned body
(tested). Law 3: no stub, `todo!` or `unwrap` outside tests, no unreachable
branch (the body's parent is the html element it was found in, not
looked up). Law 4: no `unsafe`. One file, one responsibility: `body.rs` is
*the body element*; `idl.rs` is still Web IDL's conversions; `document.rs`
is still the `Document` interface. Stage 2 § 2: every id, minted or not,
handed to `set_body` answers or refuses with nothing changed; nine
`TypeError`s and five `HierarchyRequestError`s from script, nothing
changed; ten thousand replacements then a collection, the body's expando
kept.

**Gate.** `scripts/gate.sh` first failed on `tests fail` alone (the
absent-member list above); after that change it exited 0, run in the
foreground and read in the same step (6 min 23 s): formatting clean,
clippy silent, all tests pass, nothing stubbed, `unsafe` forbidden,
licence notices, every rented crate behind its boundary, no coordinate
verb, the stop rule holds, `CHANGELOG.md` changed. `cargo test --workspace
--all-features` counts 2461 passed, 0 failed (2436 at iteration 148, this
item's 25). `git diff --check` passes. Logs kept in this session's
scratchpad, not committed.

**Roadmap.** The *Mutation from script* line was already ticked (iteration
147) and named 253 in its remainder; it now records `document.body` built,
with its corpus case and test, and names only 251 and 113. Not a new tick.
Queue: 253 ticked with its evidence; `CHANGELOG.md`, `docs/features.md`,
`docs/conformance.md` (twenty-seven cases, the new one named) and
`REMAINING.md` moved with it.

**Unresolved obligations.** `document.head` and every other absent member
wait for a page or an item. Carried from before: any other object a page
throws is said as `an object` (item 78); a `<meta>` policy a script inserts
is not applied; detached trees a script drops during a load wait for the
first collection after the parse; the renderer's path for a document the
heap refuses is not discriminated (it needs a document over the heap's
1 GiB ceiling); the one-write overshoot of the heap's ceiling; 248's
undiscriminated overrun fallback; 78's remainder; 77 needs design; 233,
234, 238 and 240 open and item 76 not done; `violations::reports` still
called by nothing in the browser process (item 203's dependency); iteration
141's browser-side font-name guard still has no discriminating test. 109
queue items are open. Next is **81** (events; its dependency 80 is done) —
dispatch, capture and listener lifetime are decisions ADR 0017 explicitly
left to it, so the next iteration should judge whether it needs an ADR of
its own first (LOOP.md, stage 2 § 4). 251 still waits on item 73. Next
unused queue number **254**; next ADR **0018**. This is one iteration, not
a finished queue or roadmap.

## Iteration 150 — item 81's decision: one dispatch, two drivers, and an agent's verb is a keyboard's click

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its *Events* and *Forms* lines, iteration
149's entry and iteration 142's (the form an ADR-only iteration takes),
queue items 43, 80, 81–93, 131, 191, 233 and 251, ADR 0017 in full, ADR
0016 §§ 3 and 6 and its *What this does not decide*, ADR 0013 §§ 3 and 6
and its *What this does not decide*, ADR 0002's no-coordinate rule, and the
code the decision is about: `alo-js`'s `object/native.rs` (`Answer::Want`,
`Want::Call`), `interpret/call.rs` (how a want is taken), `interpret/catch.rs`
(a throw unwinds through a waiting builtin), `interpret/checkpoint.rs` (a
job's throw reported through the embedder's closure); `alo-bindings`'
`wrapper.rs` and its bound members; `alo-renderer`'s `renderer.rs` (`act`),
`event_loop.rs`, `event_loop/task.rs` (`Work::Calls`), `held.rs`;
`alo-agent`'s `apply.rs` (`toggle`, which flips `checked` and
`aria-checked`) and `verb.rs`; `alo-net`'s `csp.rs` (`Inline`,
`Content::attribute`); the `alo-settings` corpus case and the test
`what_a_row_does_next_needs_a_script_and_this_says_so`. Read-only, for what
"correct" means: `alo-workplace/web/src/shell/SettingsModal.tsx`, whose nav
row's `onClick` moves `aria-current` and the `navItemOn` class. No
`AGENTS.md` exists. No sibling repository was modified. The checkout was
clean on entry at `f3e9936`.

**Selection.** Iteration 149 named **81** next (its dependency 80 is done)
and asked this iteration to judge whether it needs an ADR first. Every
earlier open item is still blocked for the reasons iteration 144 recorded,
and 251 waits on item 73. It does: ADR 0017 and ADR 0016 each name
dispatch, capture and default actions as item 81's to decide, and the code
makes each question real rather than formal — a builtin calls script only
by suspending at a numbered step and a throw unwinds through it (so
`dispatchEvent`'s *report and carry on* has no mechanism), the loop's
`Work::Calls` fixes its callees when queued (so `stopPropagation` cannot be
honoured by it), `apply` toggles a checkbox itself (so a cancelled click
cannot undo it and `el.click()` would need a second copy), and `apply`
flips `aria-checked` (which a page's own listener would flip back).
`LOOP.md` stage 2 § 4 makes the decision its own iteration.

**What was built: ADR 0018, accepted.**
`docs/decisions/0018-one-dispatch-two-drivers-and-an-agents-verb-is-a-keyboards-click.md`.
§ 1 `EventTarget` in `alo-bindings`; a listener list in the wrapper, traced
and counted; `signal` converted as Web IDL does (a `TypeError` until
`AbortSignal` exists); the global object not a target until item 251. § 2
the DOM standard's dispatch written once as a stepper whose state lives in
the event cell; dispatch flag. § 3 two drivers — a native via
`Answer::Want` (no checkpoint between listeners) with `alo-js` gaining a
call whose throw is reported, and the event loop's new `Work` kind with a
checkpoint after every listener; a page that never ran script dispatched to
by nobody. § 4 an agent's verb is trusted and unmarked; who may act is a
permission (items 93, 133). § 5 `Activate` is the one `click` keyboard
activation fires — `PointerEvent`, `pointerId` −1, `pointerType` `""`, every
coordinate 0, no key events; `PutText` fires `beforeinput`, `input`,
`change`; a link's `Activate` is followed only if not cancelled. § 6
activation behaviour (before, cancelled, after) in `alo-dom` for the agent
and `el.click()` alike. § 7 on a scripted page the agent changes no ARIA
state; the scriptless flip is kept as a stated stage 1 accommodation. § 8
the surface, law 1's absences (`returnValue`, `cancelBubble`, `srcElement`,
`initEvent`, `createEvent`, `window.event`), and `timeStamp` absent until
item 92 decides the clock. **Facts checked rather than assumed:** there is
no `Follow` verb (a link's `Activate` yields `Outcome::Followed`; a first
draft named one and was corrected); a throw does unwind through a waiting
builtin (`catch.rs` pops `run.builtins`); item 43 owns the focus *ring*,
not focus, and defers focus to 81 — so focus is cut as its own item 258
rather than pointed at 43, which a first draft did.

**What is not built, and why it stops here.** No code, dependency or test.
Item 81 stays unticked and records its ADR and its closing page: the
`alo-settings` case gains a plain-DOM script mirroring `SettingsModal.tsx`'s
nav click, whose reference render must not move. Its code is cut as **254**
(events from script; depends on nothing, so next), **255** (a browser
dispatch as a task), **256** (`Activate` as a click — closes 81), **257**
(`PutText`'s input events), **258** (focus; needs design) and **259** (event
handler attributes; opened by a page), each with a closing condition.
`docs/features.md` is unchanged, by iteration 142's precedent: no
capability changed, and its *Events* line describes the feature without
claiming it. `docs/conformance.md` is unchanged: nothing renders
differently.

**Compliance review.** Law 1: the ADR names every legacy event member it
refuses. Law 2: § 4 is the decision that keeps the agent on the ordinary
path and unsingled-out. Law 3: no code, so no stub; every cut item carries
a closing condition with tests, and 256's names a layout assertion in
numbers and an unchanged reference render. Law 4: the engine change § 3
asks for is safe Rust and generic (ADR 0013 § 6 holds). ADR 0002: no verb
gains a coordinate, and § 5 refuses to invent one inside the event.
`CLAUDE.md`'s *read the ADR before proposing an alternative*: ADRs 0013,
0016 and 0017 are extended where they deferred, contradicted nowhere. One
file, one responsibility: the ADR names one file per new interface and
`dispatch.rs`/`activation.rs` each for one rule. No layout assertion or
reference render applies to a document-only change.

**Gate.** `scripts/gate.sh` exited 0, run in the foreground and read in the
same step (6 min 28 s): formatting clean, clippy silent, all tests pass,
nothing stubbed, `unsafe` forbidden, licence notices, every rented crate
behind its boundary, no coordinate verb, the stop rule holds, no
uncommitted code to judge. `git diff --check` passes. Log kept in this
session's scratchpad, not committed.

**Roadmap.** The *Events* line, which read as unstarted, gains an Owed
clause naming ADR 0018 and items 254–259. Not a tick. `CHANGELOG.md` and
`REMAINING.md` moved with it.

**Unresolved obligations.** All of item 81's code (254–259). Carried from
before: `document.head` and every other absent member wait for a page or an
item; any other object a page throws is said as `an object` (item 78); a
`<meta>` policy a script inserts is not applied; detached trees a script
drops during a load wait for the first collection after the parse; the
renderer's path for a document the heap refuses is not discriminated; the
one-write overshoot of the heap's ceiling; 248's undiscriminated overrun
fallback; 78's remainder; 77 needs design; 233, 234, 238 and 240 open and
item 76 not done; `violations::reports` still called by nothing in the
browser process (item 203's dependency); iteration 141's browser-side
font-name guard still has no discriminating test. 115 queue items are
open. Next is **254**. Next unused queue number **260**; next ADR **0019**.
This is one iteration, not a finished queue or roadmap.

## Iteration 151 — item 254: events from script

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its *Events* line, iteration 150's entry, queue
items 81 and 254–259, ADR 0018 in full, ADR 0017 §§ 1–5 and 8, ADR 0016 §§ 1,
3 and 7, ADR 0014 §§ 2, 5, 7–9 and ADR 0013 §§ 3 and 6; and the code it
builds on — `alo-js`'s `object/native.rs`, `interpret/call.rs`, `catch.rs`,
`checkpoint.rs`, `construct.rs`, `frame.rs`, `unwound.rs`; `alo-bindings`'
every file; `alo-renderer`'s `event_loop.rs` and `event_loop/report.rs`; the
`alo-corpus` case format. No `AGENTS.md` exists. No sibling repository was
read or modified. The checkout was clean on entry at `7a40d83`.

**Selection.** Iteration 150 named **254** next; it depends on nothing, and
every earlier open item is still blocked for the reasons iteration 144
recorded. Its ADR (0018) and feature line (*Events*) exist.

**What was built.** *The engine* (`alo-js`, generic, ADR 0013 § 6 holds):
`Want::Report`, a call a builtin asks for whose throw nothing inside it
catches **stops at that call** — `land` searches only the frames inside the
innermost reporting builtin, takes them down, sets the throw aside rooted
with a trace of those frames only (`interpret/reported.rs`, at most
`bounds::REPORTS_SET_ASIDE` = 256 kept between hand-overs, the rest counted)
and answers the builtin `undefined` with `Call::reported()` true; only the
page's own escapes are reported, a stop still ends the run. The embedder
takes them with `Engine::hand_over_reported` after a run or a call; a
checkpoint hands a job's set-aside throws to its own report before the
job's own throw (`Drained::reported`, `Drained::unreported`). And
`Instance::Made`, an embedder's constructor given its instance only by
`new` (`Call::constructing()`), so a constructor that suspends to run a
dictionary getter keeps what it converted in its instance. `Waiting`'s
`answered` became a three-state `Slot` (clippy's bool limit, and clearer).
*The bindings* (`alo-bindings`): `listeners.rs` (a wrapper's list, an id per
listener standing for the *removed* flag, counted in the footprint);
`event.rs` (the event cell, the dispatch's `Progress` held in it, the
dispatch flag being having one); `dispatch.rs` (the stepper: path computed
at the start, capture then bubble, `AT_TARGET` both passes, lists copied as
ids at each target's turn, `once`, `passive`, both stops, `finish`);
`dictionary.rs`; `interface/event_target.rs`, `event.rs`,
`custom_event.rs`. `EventTarget` heads a node's chain; `Event` and
`CustomEvent` are the only interface objects on the global, each with
`prototype`, `constructor` back, `CustomEvent` inheriting from `Event`, the
phase constants on both `Event` and its prototype; `InvalidStateError` for
a second dispatch (`DomException::named`); `signal` the Web IDL
`TypeError`; the default passive value for scroll-blocking types. A node on
a dispatch's path is kept, tree and wrapper, until it ends (`DocumentCell`'s
`on_path`, traced in `liveness.rs`), as the standard's path keeps it.
`alo-dom`'s `document_element` became public, for the default passive
value. *The renderer* hands over set-aside throws after every script and
call, before the piece's own throw, and adds a checkpoint's uncounted ones.

**Cut, by scope, never depth:** `isTrusted` is **queue item 260** — Web IDL
makes it `[LegacyUnforgeable]`, an own accessor on every instance sharing
one getter per realm, which needs a place the constructor can find that
getter; a prototype accessor would be the approximate member ADR 0013 § 3
refuses. The flag is kept and set. 256 now depends on 260 as well.

**Tests and manual checks.** `alo-js/tests/what_a_reported_call_reports.rs`
(7 tests, both ordinarily and collecting at every allocation: carrying on,
order, a `try` inside versus around, the trace stopping at the call, a
non-function and a throwing builtin reported, nesting, a stop not reported,
the bound — 256 kept and 44 counted — a checkpoint's order, and
`Instance::Made` only by `new`). `alo-bindings/tests/what_an_event_does.rs`
(12 tests, every script run both ways: the order across document, html,
body, div, p and span with each phase and `currentTarget`; non-bubbling;
`once`; `passive` and its default; both stops; a throwing listener reported
with dispatch carrying on and the outer `try` not reached;
`InvalidStateError` caught and uncaught; nested dispatch; the list changed
during its own dispatch; `handleEvent` objects, getters and the
not-callable `TypeError`; argument conversion and getter order; the
constructors, constants and chain; `composedPath()` during and after; the
absent members; **a node on the path kept** through collections at every
allocation; a listener released with its detached tree; and hostile
scripts — a path five thousand deep, a runaway nested dispatch ending in
one reported `RangeError` with no Rust recursion, a thousand throwing
listeners). `alo-renderer/tests/what_a_listener_hears.rs` (2 tests: the two
paragraphs **in numbers** — 304×24.296875 at (8, 44.800003) and at (8,
77.09688), their text "section1 div1 button2 n once div3 true" and
"section1 div1 button2 n div3 true" — and the listener's throw reported as
`script 1: uncaught: Error: a listener threw (at script 1, line 16, column
70)` on each dispatch, the column checked by hand to be the `throw`). Corpus
case **`a-script-hears-an-event`**, read by the renderer test so the two
cannot drift; its **reference render looked at**: the *Pick* button and two
lavender lines saying what was heard. **Doctored runs**, each restored and
checked identical: the path not kept (the stressed run loses `a` and
`root`), `once` not removed, `passive` ignored, `stopImmediatePropagation`
ignored, the bubble pass skipping the target, no reporting boundary in the
engine, and the renderer not handing over — each fails at least one test.

**Compliance review.** Law 1: `returnValue`, `cancelBubble`, `srcElement`,
`initEvent`, `createEvent`, `initCustomEvent` asserted absent. Law 2: no
agent surface changed; nothing the browser does dispatches yet (255, 256).
Law 3: no stub, no `unwrap` outside tests; the cut is a queue item with a
closing condition. Law 4: no `unsafe`. ADR 0018 §§ 1–3 and 8 as written,
except the stated cut; § 1's *never makes a wrapper to find out* holds — a
dispatch asks the cell's table, and only `composedPath()` wraps. ADR 0014
§ 9: listener entries, a dispatch's path and its listener copy, and the
cell's `on_path` are counted in footprints; the new bound has its reason.
Stage 2 § 2, hostile bytes: a script is the hostile input here, and the
hostile cases return or report, never panic. One file, one responsibility:
each new file has one rule. Layout assertion in numbers and reference
render: above. `docs/features.md`'s *Events* line names what is built.

**Gate.** `scripts/gate.sh` exited 0, run in the foreground and read in the
same step (6 min 44 s): formatting clean, clippy silent, all tests pass,
nothing stubbed, `unsafe` forbidden, licence notices, every rented crate
behind its boundary, no coordinate verb, the stop rule holds, `CHANGELOG.md`
changed. A first run failed only `cargo fmt` on two new test files; they
were formatted and the whole gate run again. `git diff --check` passes. Log
kept in this session's scratchpad, not committed.

**Roadmap.** The *Events* line moves from *Owed: all of the code* to a
Built clause (events from script, corpus case) and an Owed clause (255,
256, 260, 257–259). Not a tick. `CHANGELOG.md`, `docs/features.md`,
`docs/conformance.md` (twenty-eight cases) and `REMAINING.md` moved with it.

**Unresolved obligations.** New: `isTrusted` (260); `addEventListener` with
an object type *and* an options getter refused by name
(`ASecondArgumentBehindACall`, item 221), as `setAttribute` with two objects
is; a dispatch abandoned by a non-page escape (a stop, a full heap) leaves
its event flagged as dispatching and its path kept — the event loop stops
the page in exactly that case, but an embedder driving `alo-bindings` alone
keeps the path until the page goes; an embedder that never calls
`hand_over_reported` holds up to 256 roots. Carried: item 81's remaining
code (255, 256, 257, 258 needs design, 259); `document.head` and every
other absent member wait for a page or an item; any other object a page
throws is said as `an object` (item 78); a `<meta>` policy a script inserts
is not applied; detached trees a script drops during a load wait for the
first collection after the parse; the renderer's path for a document the
heap refuses is not discriminated; the one-write overshoot of the heap's
ceiling; 248's undiscriminated overrun fallback; 78's remainder; 77 needs
design; 233, 234, 238 and 240 open and item 76 not done;
`violations::reports` still called by nothing in the browser process (item
203's dependency); iteration 141's browser-side font-name guard still has
no discriminating test. 115 queue items are open. Next is **255** (its
dependency 254 is done). Next unused queue number **261**; next ADR
**0019**. This is one iteration, not a finished queue or roadmap.

## Iteration 152 — item 255: a dispatch from the browser is a task

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its *Events* line, iteration 151's entry,
queue items 81 and 254–260, ADR 0018 in full and ADR 0016 in full (§§ 3, 6
and 7 apply); and the code it builds on — `alo-bindings`' `dispatch.rs`,
`event.rs`, `interface/event_target.rs`, `interface.rs`, `install.rs`,
`embed.rs`; `alo-renderer`'s `event_loop.rs`, `event_loop/task.rs`,
`event_loop/report.rs`, `held.rs`; `alo-js`'s `Engine` (`run`, `call`,
`checkpoint`, `hand_over_reported`). No `AGENTS.md` exists. No sibling
repository was read or modified. The checkout was clean on entry at
`8daddf8`.

**Selection.** Iteration 151 named **255** next; its one dependency, 254, is
done, and every earlier open item is still blocked for the reasons
iteration 144 recorded. Its ADR (0018 § 3, with ADR 0016 §§ 3 and 6) and
its feature line (*Events*) exist.

**What was built.** *The bindings* (`alo-bindings`): `event::create` and
`Firing` — the standard's *create an event* for the browser, an `Event`
inheriting from the prototype the page's document cell holds, its type and
three init flags as given; and `dispatch::invoke`, how a listener's
callback is called (the function, its `handleEvent`, or a `handleEvent`
getter to call first), now asked by `dispatchEvent`'s native **and** the
loop, so the two drivers cannot disagree about it. *The renderer*
(`alo-renderer`): `Work::Dispatch` (`event_loop/task.rs`), one list rooted
before anything else is allocated, holding the target's wrapper — made if
the node had none, since `event.target` must be an object — then the event;
`EventLoop::queue_dispatch` with its error `Unqueued` (stopped, which a
full heap also causes, as for `queue_calls`; no such node; not a document);
the driver, `event_loop/dispatched.rs`, which begins the dispatch
**trusted** (ADR 0018 § 4), checks the stop switch before every listener,
calls each with nothing else running, hands over set-aside throws, reports
its own, runs the **microtask checkpoint, and only then** tells the stepper
the listener returned. That order is the standard's *inner invoke* (calling
is *clean up after running script*, which checkpoints, before the passive
flag is unset and `stopImmediatePropagation` is looked at), and the tests
pin it. `Held::dispatch` is the renderer's entry: `None` for a page that
never ran script, which is given no heap (ADR 0018 § 3).

**Tests and manual checks.** `alo-renderer/tests/a_dispatch_from_the_browser.rs`
(13 tests; the table cases each run ordinarily and collecting at every
allocation and must agree): the **closing condition** — two listeners
dispatched from the renderer give `1a2b`, and the same two by a script's
`dispatchEvent` give `12ab`; the path and phases, with document, div and
button listeners and non-bubbling; the event's type and init flags as the
browser made them, and `eventPhase` 0, `currentTarget` `null`, empty
`composedPath()` after; a microtask's `stopImmediatePropagation` and
`stopPropagation` between listeners, and `preventDefault` from a passive
listener's microtask doing nothing; a throwing listener reported with the
dispatch and its jobs carrying on; `InvalidStateError` for the same event
dispatched again from a listener and from a microtask; `handleEvent`
objects, a getter, a throwing getter and a missing one (reported
`TypeError`), and the same objects through a script's dispatch; listeners
removed, added and `once` during the dispatch; **a waiting dispatch holding
its target** after the page detached it and dropped every reference and a
collection ran (`heap().check()` sound before and after); a target never
wrapped given one; a page that never ran script given no heap; a node from
another document refused as `NoSuchNode` with the page running on; a stop
during an endless listener stopping the page, dropping the task queued
behind it, and refusing the next dispatch; a thousand throwing listeners
keeping `MOST_REPORTS` (256) reports and counting 744. **Doctored runs**,
each restored and the file checked identical: no checkpoint per listener
(5 tests fail), the stepper told before the checkpoint (1 fails), the task
not rooting its target (1 fails). Nothing here positions, sizes or draws,
so there is no layout assertion and no reference render to make, and no
corpus case: nothing the browser does fires an event yet (256), so no page
can observe the difference this makes.

**Compliance review.** Law 1: nothing legacy added. Law 2: no agent surface
changed; `Activate` dispatching is 256. Law 3: no stub, no `todo!`, no
`unwrap` outside tests; a dispatch refused at run time — which only a bug
of ours could cause, since the task made both objects — stops the page as
the engine's bug, stated in the code. Law 4: no `unsafe`. ADR 0018 § 3 as
written; § 4's trusted flag is set (`isTrusted` itself is 260); § 1's
*never makes a wrapper to find out nobody listens* holds for the path,
which the stepper reads from the cell's table — only the target is wrapped,
because the event must name it. ADR 0016 § 3 (a checkpoint after every
callback) and § 7 (a stop is a stopped page) hold. Stage 2 § 2: the hostile
input is a page's script — an endless listener, a thousand throwing ones, a
listener re-dispatching its own event — and each returns, reports or stops
the page, never panics. One file, one responsibility: the browser's driver
is its own file; `task.rs` gains what a dispatch task holds, which is its
existing reason (what a waiting task holds); `event.rs` gains the
browser's way of making an event beside the page's. Rented crates: none new.

**Gate.** `scripts/gate.sh` exited 0, run in the foreground and read in the
same step (7 min 2 s): formatting clean, clippy silent, all tests pass,
nothing stubbed, `unsafe` forbidden, licence notices, every rented crate
behind its boundary, no coordinate verb, the stop rule holds, `CHANGELOG.md`
changed. A first run failed only `cargo fmt` (clippy and tests passed in
it); the code was formatted and the whole gate run again. `git diff --check`
passes. Logs kept in this session's scratchpad, not committed.

**Roadmap.** The *Events* line's Built clause gains the browser's dispatch
(255); its Owed clause now leads with something the browser does that fires
one, and `Activate` (256). Not a tick. `CHANGELOG.md`, `docs/features.md`,
`docs/autonomy/QUEUE.md` (255 ticked with what was built) and
`REMAINING.md` moved with it. `docs/conformance.md` lists corpus cases and
none was added.

**Unresolved obligations.** New: nothing in the browser fires a dispatch
yet — 256 (`Activate`) and item 233 (the loop running between messages, an
`Act` answered after its task's checkpoint) are where it is reached; the
`trusted` flag is set but unreadable until 260; a page whose script never
touched a node still gets that node's wrapper when the browser dispatches
to it, even with nobody listening anywhere on the path — correct, and a
cost worth measuring only when it shows; ADR 0016's ceiling on waiting
tasks still waits on the first task a page can queue for itself. Carried:
`isTrusted` (260); `addEventListener` with an object type and an options
getter refused by name (item 221); a dispatch abandoned by a non-page
escape leaves its event flagged and path kept — now true of the browser's
driver too, where the page is stopped in exactly that case; an embedder
that never calls `hand_over_reported` holds up to 256 roots; item 81's
remaining code (256, 257, 258 needs design, 259); `document.head` and every
other absent member wait for a page or an item; any other object a page
throws is said as `an object` (item 78); a `<meta>` policy a script inserts
is not applied; detached trees a script drops during a load wait for the
first collection after the parse; the renderer's path for a document the
heap refuses is not discriminated; the one-write overshoot of the heap's
ceiling; 248's undiscriminated overrun fallback; 78's remainder; 77 needs
design; 233, 234, 238 and 240 open and item 76 not done;
`violations::reports` still called by nothing in the browser process (item
203's dependency); iteration 141's browser-side font-name guard still has
no discriminating test. 114 queue items are open. 256 depends on 260, so
next is **260** (it depends on nothing). Next unused queue number **261**;
next ADR **0019**. This is one iteration, not a finished queue or roadmap.

## Iteration 153 — item 260's decision: ADR 0019, a realm names its host

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its *Events* line, iteration 152's entry,
queue items 81 and 254–260, ADR 0018 in full, ADR 0017 § 4 and ADR 0013
§§ 3 and 6; and the code item 260 would change — `alo-bindings`' `event.rs`,
`interface.rs`, `interface/event.rs`, `install.rs`, `define.rs`,
`document_cell.rs`; `alo-js`'s `object/native.rs` (`Call`, `Instance::Made`,
`Make`), `interpret/construct.rs` (`make_instance`), `builtin.rs`
(`Intrinsics`), `realm.rs` and `interpret.rs`. No `AGENTS.md` exists. No
sibling repository was read or modified. The checkout was clean on entry at
`ddd16a8`.

**Selection.** Iteration 152 named **260** next: 256 depends on it, and it
depends on nothing. Its ADRs (0018 §§ 4 and 8, 0013 § 3) and its feature
line (*Events*) exist.

**Why this iteration is a decision and not the code.** Item 260's own text
leaves its central question open — where the per-realm getter lives *and how
the constructor finds it*, naming two options. Reading the code settled that
neither is a chore: `new Event(…)`'s native is handed the heap, a `this` the
engine just made from the constructor's `prototype` (an `Event` cell with no
edge to anything), its arguments and the intrinsics, and nothing it holds
reaches the document cell; `Make` cannot read the heap. Every answer either
gives a native something more — which ADR 0017 § 4 says, in so many words,
it will never be handed — or invents a hidden edge on an interface
prototype. The same gap is already written down for `Window`'s `document`
(`install.rs`) and stands in front of every node constructor. `LOOP.md`
stage 2 § 4: *a decision made inside a commit that was mostly code is a
decision nobody reviewed* — so the decision is this iteration, as ADR 0018
was iteration 150's for item 81.

**What was decided (ADR 0019).** `alo-js`'s realm gains ECMAScript's
`[[HostDefined]]`: one reference an embedder sets once
(`Engine::host_defined`, a second call refused), rooted by the realm, handed
to a native beside the intrinsics (`Call::host_defined`), its type never
known to the engine (ADR 0013 § 6 holds). `alo-bindings`' `install` sets it
to the document cell. The cell's `Interfaces` holds each interface's
unforgeables object — Web IDL's `[[Unforgeables]]`, made once in `furnish`
with no prototype — and making an instance copies the unforgeables of the
interface and those it inherits onto it: the `Event` and `CustomEvent`
constructors at their first step, before any page script can run inside
them, and `event::create` from the cell it is handed. `isTrusted` is an own
accessor, enumerable, not configurable, no setter, one getter per realm.
ADR 0017 § 4 is amended, not replaced (a node's native still reaches it
through `this`), and carries a forward pointer to ADR 0019 § 2. Rejected
with reasons: a prototype getter (ADR 0013 § 3), a getter per instance, an
embedder-cell `Event.prototype` found by walking the chain, reading
`document` off the global, natives carrying data, and widening `Make`.

**Compliance review.** No code changed, so laws 1–4 are untouched by this
commit; the decision keeps law 4 (no `unsafe`: a field, a root and an
accessor), ADR 0013 § 6 (the engine learns no embedder type) and ADR 0014
(the value is rooted by the realm, nothing hidden from the collector).
Nothing positions, sizes or draws, so there is no layout assertion or
reference render to make, and no corpus case. Nothing was ticked: the queue
item stays open with its decision recorded, and the roadmap line gains the
ADR in its Owed clause.

**Gate.** `scripts/gate.sh` exited 0, run in the foreground and read in the
same step (6 min 47 s): formatting clean, clippy silent, all tests pass,
nothing stubbed, `unsafe` forbidden, licence notices, every rented crate
behind its boundary, no coordinate verb, the stop rule holds, and *no
uncommitted code to judge* for the documentation check (this change is
documentation only). `git diff --check` passes. Log kept in this session's
scratchpad, not committed.

**Roadmap.** The *Events* line's Owed clause now says `isTrusted` (260) has
its decision, ADR 0019, and none of its code. Not a tick. `CHANGELOG.md`,
`docs/autonomy/QUEUE.md` (260 records its decision), `REMAINING.md` and ADR
0017 § 4 (a forward pointer) moved with it. `docs/features.md` is unchanged:
it already lists `isTrusted` as absent until 260, which is still true.

**Unresolved obligations.** New: item 260's code — the realm field and its
root, `Call::host_defined`, `Interfaces`' unforgeables, the copy in both
constructors and `event::create`, the getter, and tests that a getter is
the same function on two events, that `delete` and assignment fail, and
`false`/`true` for a script's and the browser's dispatch; the ADR's
*does not decide* — several realms in one heap, `document` as an accessor
(251), node constructors. Carried, unchanged from iteration 152: nothing in
the browser fires a dispatch yet (256, 233); `addEventListener` with an
object type and an options getter refused by name (item 221); a dispatch
abandoned by a non-page escape leaves its event flagged and path kept; an
embedder that never calls `hand_over_reported` holds up to 256 roots; item
81's remaining code (256, 257, 258 needs design, 259); `document.head` and
every other absent member wait for a page or an item; any other object a
page throws is said as `an object` (item 78); a `<meta>` policy a script
inserts is not applied; detached trees a script drops during a load wait
for the first collection after the parse; the renderer's path for a
document the heap refuses is not discriminated; the one-write overshoot of
the heap's ceiling; 248's undiscriminated overrun fallback; 78's remainder;
77 needs design; 233, 234, 238 and 240 open and item 76 not done;
`violations::reports` still called by nothing in the browser process (item
203's dependency); iteration 141's browser-side font-name guard still has
no discriminating test. 114 queue items are open. Next is **260**, now
buildable against ADR 0019. Next unused queue number **261**; next ADR
**0020**. This is one iteration, not a finished queue or roadmap.

## Iteration 154 — item 260 built: `isTrusted`, unforgeable, one getter per realm

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its *Events* line, iteration 153's entry,
queue items 81 and 254–260, ADR 0019 in full and ADR 0018 §§ 4 and 8; and
the code it changes — `alo-js`'s `realm.rs`, `interpret.rs`,
`interpret/call.rs`, `object/native.rs`, `object/access.rs`; `alo-bindings`'
`install.rs`, `interface.rs`, `document_cell.rs`, `define.rs`, `event.rs`,
`interface/event.rs`, `interface/custom_event.rs`, and the tests of items
254 and 255. No `AGENTS.md` exists. No sibling repository was read or
modified. The checkout was clean on entry at `73b90de`.

**Selection.** Iteration 153 named **260** next: it depends on nothing, its
decision (ADR 0019) is accepted, and its feature line is *Events*. 256
depends on it.

**What was built, as ADR 0019 decided it.**
- `alo-js` (§ 1): the realm's `host: Option<Root>` — `Realm::host_defined`
  and `Realm::define_host`, which roots the value and hands a second one
  back unrooted; `Engine::host_defined`, whose second call is a `TypeError`
  naming it with the first value standing; `Call::host_defined` and
  `Call::hosted_by`, the interpreter passing the realm's host beside its
  intrinsics at the one place a builtin is stepped. The engine learns no
  embedder type (ADR 0013 § 6).
- `alo-bindings` (§§ 2–3): `install` names the document cell as the
  realm's host before it makes anything; `Interfaces` holds an
  unforgeables slot per interface beside its prototype, traced with them;
  `furnish` makes `Event`'s unforgeables object (no prototype) and puts
  `isTrusted` on it with `define::unforgeable_attribute` — enumerable, not
  configurable, no setter; the new `unforgeable.rs` is the one copy, for
  the interface and each it inherits, allocation-free and so not a
  safepoint; the `Event` and `CustomEvent` constructors copy at step 0,
  before the type or dictionary is converted, reaching the cell through
  `Call::host_defined`; `event::create` copies from the cell it is handed.

**Closing condition, met.** `e.isTrusted` is `false` after a script's
`dispatchEvent` (and before, and in its listener, and for a
`CustomEvent`); `true` in listeners for the browser's dispatch, at the
target and on the way up, and still after it ends — `false` once a script
dispatches that same event; its property is the instance's own (neither
prototype has it) and the same getter on two events, on the browser's
event, and on the unforgeables object; and a page cannot replace it —
`delete`, sloppy and strict assignment, a property on `Event.prototype`
and a cut prototype each leave it as it was.

**Tests.** `alo-js/tests/what_a_realm_hosts.rs` (4),
`alo-bindings/tests/who_sent_an_event.rs` (5),
`alo-renderer/tests/a_dispatch_from_the_browser.rs` (+1, now 14); every
script run ordinarily and with the collector at every allocation.
`what_an_event_does.rs`' absent-members test no longer lists `isTrusted`.
**Doctored runs**, each restored from a copy and the tree checked against
the intended diff: constructors not copying (5 tests fail);
`event::create` not copying (2, one in `alo-bindings`, one in
`alo-renderer`); the property configurable (2); the host kept as a bare
reference instead of a root (the collecting runs disagree in two tests;
that doctor also dropped the set-once check, which two more caught).
Not doctored: a getter made per instance — the same-getter assertions are
the guard, but no run proved they would catch it.

**Compliance review.** Law 1: nothing legacy added. Law 2: untouched.
Law 3: no stubs, no `unwrap` outside tests; every new failure path is an
error (`Fault::Gone` for a realm never installed, `BuiltinIsWrong` for an
instance refusing a copy). Law 4: no `unsafe`. ADR 0014: the host is a
realm root, the unforgeables a traced edge of the cell. One file, one
responsibility: the copy is its own file; the realm's host is realm state;
`define.rs` still says only how a member goes on an object. Nothing
positions, sizes or draws, so there is no layout assertion or reference
render to make, and no corpus case — the item's closing condition names
none, and `a-script-hears-an-event` is unchanged and passing. No bytes from
outside are read, so the hostile-input clause does not apply beyond the
hostile-script cases above.

**Gate.** `scripts/gate.sh` exited 0, run in the foreground and read in the
same step (7 min 12 s): formatting clean, clippy silent, all tests pass,
nothing stubbed, `unsafe` forbidden, licence notices, every rented crate
behind its boundary, no coordinate verb, the stop rule holds, `CHANGELOG.md`
changed with the code. `git diff --check` passes. Log kept in this
session's scratchpad, not committed.

**Roadmap.** The *Events* line's Built clause gains `isTrusted` and its Owed
clause loses it. Not a tick: 256–259 are still owed. `docs/features.md`'s
Events entry says every event tells whether the browser sent it, and no
longer lists that as absent. `CHANGELOG.md`, `QUEUE.md` (260 ticked with its
Built note) and `REMAINING.md` moved with it.

**Unresolved obligations.** From ADR 0019's *does not decide*: several
realms in one heap; `furnish` for a second document cell makes a second
unforgeables object, so an event `event::create` makes in that cell would
carry a different getter from the realm's — nothing does that yet;
`document` as Web IDL's accessor (251); node constructors. Carried,
unchanged from iteration 153: nothing in the browser fires a dispatch yet
(256, 233); `addEventListener` with an object type and an options getter
refused by name (item 221); a dispatch abandoned by a non-page escape
leaves its event flagged and path kept; an embedder that never calls
`hand_over_reported` holds up to 256 roots; item 81's remaining code (256,
257, 258 needs design, 259); `document.head` and every other absent member
wait for a page or an item; any other object a page throws is said as `an
object` (item 78); a `<meta>` policy a script inserts is not applied;
detached trees a script drops during a load wait for the first collection
after the parse; the renderer's path for a document the heap refuses is
not discriminated; the one-write overshoot of the heap's ceiling; 248's
undiscriminated overrun fallback; 78's remainder; 77 needs design; 233,
234, 238 and 240 open and item 76 not done; `violations::reports` still
called by nothing in the browser process (item 203's dependency);
iteration 141's browser-side font-name guard still has no discriminating
test. 113 queue items are open. Next is **256** — its dependencies 255 and
260 are done. Next unused queue number **261**; next ADR **0020**. This is
one iteration, not a finished queue or roadmap.

## Iteration 155 — item 256 built: `Activate` is a keyboard's click, and item 81 closes

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its *Events* line, iterations 153 and 154's
entries, queue items 81, 233 and 254–260, ADR 0018 in full; HTML's
legacy-pre-activation, legacy-canceled-activation and checkbox/radio
activation behaviour (the standard's text, fetched and read); and the code
it changes — `alo-agent`'s `apply.rs` and `verb.rs`, `alo-bindings`'
`event.rs`, `interface.rs`, `interface/event.rs`, `custom_event.rs`,
`install.rs`, `dispatch.rs`, `unforgeable.rs`, `embed.rs`; `alo-renderer`'s
`renderer.rs`, `held.rs`, `event_loop.rs`, `event_loop/task.rs`,
`dispatched.rs`, `scripts.rs`, `said.rs`, `message.rs`, `wire.rs`;
`alo-corpus`' `rendering.rs`; and the Settings test and case. No
`AGENTS.md` exists. `alo-workplace/web/src/shell/SettingsModal.tsx` was read
for what a nav click does; no sibling repository was modified. The checkout
was clean on entry at `4e774d9`.

**Selection.** Iteration 154 named **256** next: its dependencies 255 and
260 are done, its decision is ADR 0018 §§ 4–7, its feature line *Events*.

**Scope cut, not depth.** The item named `HTMLElement` and `click()` beside
the browser's half. Both callers of the activation rule are ADR 0018 § 6's,
but the closing condition needs only the agent's. `click()` is cut to the
new item **261**, with its own closing condition; the rule it will call is
built whole here.

**What was built.**
- `alo-dom/src/activation.rs`: the activation target (the target or its
  nearest ancestor with an activation behaviour), `before`, `cancelled` and
  `after`, as the HTML standard writes them, for a checkbox, a radio (group
  = same tree, same non-empty `name`), a link with an `href`, and a `button`
  (nothing until item 82). Checkedness is the `checked` attribute until item
  82 separates them.
- `alo-bindings`: the event cell's `Shape`; `UIEvent`, `MouseEvent` and
  `PointerEvent` prototypes, one file each, with the members § 5 names and
  their keyboard-click values, brand-checked; no constructors on the global.
  `Firing` names its interface (`Fired`), with `CLICK`, `INPUT` and `CHANGE`.
- `alo-agent`'s `apply` asks `alo-dom` instead of toggling. It keeps the
  `aria-checked` flip as § 7's stage 1 accommodation, which now applies only
  to a page that never ran script.
- `alo-renderer`: `Work::Activate`, run by `event_loop/activated.rs` —
  before, the click, then cancelled or after, with `input` and `change`
  dispatched **inside the same task** — reported as `Turn::clicked`;
  `Held::activate`; `press.rs`, which runs the loop until that task has run
  and bounds what it says; `act` answers a cancelled link as activated and
  an uncancelled link as followed. `FromRenderer::Acted` gains `issues` (on
  the wire: the outcome, then a counted list), because a listener's throw
  during an `Act` must be said somewhere and was otherwise lost.
- `alo-settings` carries SettingsModal's nav behaviour in plain DOM.

**Closing condition, met.** Item 81's: the nav-row test now asserts that
pressing *Sharing* makes it current and *General* not. The highlight is
asserted in numbers: `navItemOn`'s fill moved to (36, 208.1336)
169×31.132813, and the row text colours swapped. The case's reference
render, boxes, layout and agent tree are **unchanged** with the script
present (the corpus passes without `ALO_UPDATE_REFERENCES`). A cancelled
click on a checkbox leaves it unticked, and the agent's tree says
`[checked=false]`.

**Tests.** `alo-dom/tests/what_a_click_activates.rs` (13);
`alo-renderer/tests/an_agents_click.rs` (10, three of them run ordinarily
and with the collector at every allocation): the click's members, a plain
event having none of them, a cancelled and an uncancelled checkbox
(`click:true job input… change…`, a listener's microtask before `input`), a
cancelled radio, a link cancelled and not, ARIA state left to the page, a
listener's throw in `Acted`'s issues, a stopped page, and a scriptless page
as stage 1. Also `an_agent_on_settings.rs` (nav-row test rewritten, plus a
load test) and `messages_across_a_boundary.rs` (an `Acted` with issues
round-trips, and is refused when cut short). **Doctored runs**, each
restored from a copy and checked:
- `cancelled` not unticking — fails the cancelled-checkbox test;
- a scripted page's click falling back to `apply` — fails the Settings
  nav-row test;
- no `input`/`change` — fails;
- the click a plain `Event` — fails;
- a cancelled link always followed — fails 3.

One doctor was cut off by a pipe before it restored `renderer.rs`; it was
restored from its copy and diffed back. **Not doctored:** the radio's
cancel path through the renderer (its rule is doctor-free but tested in
`alo-dom`), and the stopped-page fallback.

**Compliance review.**
- Law 1: nothing legacy added — no `initMouseEvent`, no `which`.
- Law 2: the agent still names and never points; the click carries no
  coordinate (§ 5), and the gate's coordinate check passes.
- Law 3: no stubs and no `unwrap` outside tests. Every failure is an error
  or a said issue.
- Law 4: no `unsafe`.
- One file, one responsibility: the rule (`activation.rs`), the task
  (`activated.rs`), the renderer's press (`press.rs`), and one file per
  interface. `renderer.rs`' `act` only routes.
- Positions and sizes: the layout is asserted in numbers (the row's
  rectangle unchanged, the highlight's fill at its numbers). The reference
  render is unchanged and checked by the corpus.
- No bytes from outside are newly read except the `Acted` issues on the
  wire. They go through the existing bounded reader, and a cut message is
  refused (tested).

**Gate.** `scripts/gate.sh` exited 0, run in the foreground and read in the
same step (7 min 13 s): formatting clean, clippy silent, all tests pass,
nothing stubbed, `unsafe` forbidden, licence notices, every rented crate
behind its boundary, no coordinate verb, the stop rule holds, and
`CHANGELOG.md` changed with the code. `git diff --check` passes. The log is
in this session's scratchpad, not committed.

**Roadmap.** The *Events* line's Built clause gains `Activate` as a
keyboard's click. Its Owed clause is now 261, 257, 258 and 259. Not a
tick. `docs/features.md`, `CHANGELOG.md`, `QUEUE.md` (256 and 81 ticked
with notes, 261 added, a note on 233) and `REMAINING.md` moved with it.

**Unresolved obligations.**
- New from this item:
  - `click()` and `HTMLElement` (261).
  - On a page that **never ran script**, activating a non-link inside a link
    answers `Activated` (stage 1's `perform` decision). On a scripted page
    the ancestor link is followed, as the standard says. The two paths
    differ until stage 1's accommodation is retired.
  - Radio groups ignore form owners. Checkedness is the attribute, and
    indeterminateness is not held (all item 82).
  - A click task that stops the page leaves a box it already changed
    changed.
  - `getModifierState`, CSSOM View's coordinates, the three constructors and
    every device member of `PointerEvent` are absent.
  - Item 233 still owes the loop running between messages.
- Carried, unchanged from iteration 154:
  - several realms in one heap, and a second document cell's
    unforgeables;
  - `document` as an accessor (251), and node constructors;
  - `addEventListener` with an object type or an options getter is refused
    by name (221);
  - a dispatch abandoned by a non-page escape leaves its event flagged;
  - `hand_over_reported`'s 256 roots;
  - absent members wait for a page or an item;
  - other thrown objects are said as `an object` (78);
  - a `<meta>` policy a script inserts is not applied;
  - detached trees wait for the first collection after the parse;
  - the heap-refused document path is not discriminated;
  - the heap ceiling's one-write overshoot;
  - 248's overrun fallback;
  - 78's remainder; 77 needs design; 233, 234, 238 and 240 are open, and
    76 is not done;
  - `violations::reports` is still uncalled (203);
  - iteration 141's font-name guard has no discriminating test.

112 queue items are open. Next is **261** or **257**, both depending only
on 256. The next unused queue number is **262** and the next ADR **0020**.
This is one iteration, not a finished queue or roadmap.

## Iteration 156 — item 261 built: `HTMLElement`, and a script's `el.click()`

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its *Events* line, iteration 155's entry,
`REMAINING.md`'s continuation order, queue items 254–263, ADR 0018 in full;
and the code it touches — `alo-dom`'s `activation.rs`; `alo-bindings`'
`interface.rs`, `interface/element.rs`, `interface/event_target.rs`,
`event.rs`, `dispatch.rs`, `wrapper.rs`, `idl.rs`, `embed.rs`,
`liveness.rs`, `unforgeable.rs`, `tree.rs`, `lib.rs`; `alo-js`'
`object/native.rs` and `abrupt.rs`; `alo-css`' `state.rs`; `alo-renderer`'s
`event_loop/activated.rs`, `event_loop/task.rs` and `dispatched.rs`; and the
renderer's and bindings' event tests. No `AGENTS.md` exists.
`alo-workplace/web/src` was searched, read-only, for `.click()`; no sibling
repository was modified. The checkout was clean on entry at `525e3bb`.

**Selection.** `REMAINING.md` and iteration 155 named **261** next; its one
dependency, 256, is done; its decision is ADR 0018 § 6, its feature line
*Events*. It was cut from 256 with a closing condition, not opened by a
frozen page — no corpus case was added, and none is claimed.

**Scope cut, not depth.** Two pieces are cut to new items:
- **262**, each HTML element's own interface (`HTMLInputElement`…). Until
  then an HTML element's prototype is `HTMLElement.prototype` itself, a
  link short, said in `interface.rs`.
- **263**, a script's click following a link. That needs the renderer to
  ask the browser process to navigate, and may need an ADR. alo's own
  `FilesView.tsx` and `TaskDetail.tsx` call `a.click()` on an
  `<a download>`, so it has real pages waiting.

**What was built.**
- `alo-bindings`:
  - `Interface::HtmlElement` inherits `Element` and is every HTML-namespace
    element's interface. `Brand::HtmlElement` asks the namespace, so an SVG
    element stays an `Element`.
  - `interface/html_element.rs`: `click()`. It does nothing on a disabled
    `button`/`input`/`select`/`textarea` (`alo-css`' `is_disabled`, which
    is now one rule for `:disabled` and `click()`; `alo-bindings` gains
    `alo-css` as a dependency for it). It does nothing while the element's
    click is in progress. Otherwise it fires an untrusted `PointerEvent`
    `click` with `activation.rs` around it, then `input` and `change`, all
    inside the call.
  - `clicking.rs`: the click in progress flag is the wrapper holding a
    `Clicking` — the current event, the activation target's wrapper and the
    pre-activation record — traced as the wrapper's edges.
  - `scripted.rs`: the native driver, moved out of `dispatchEvent` so that
    both natives drive the stepper one way, each from a base step.
- `alo-js`: `Missing::InTheEmbedder(&'static str)`, a refusal in the
  embedder's own words, so the engine still knows nothing of the DOM. An
  uncancelled link click from script is refused with it, after its
  listeners have run. **This is a small engine API addition made inside a
  code commit.** It decides nothing about events, but a reviewer may want
  it as an ADR note.

**Closing condition, met.** In `a_scripts_click.rs`, a page's script calls
`box.click()`. Its listeners read the box ticked, `isTrusted` `false`,
`pointerId` `-1`; `input` and `change` are untrusted and run before
`click()` returns `undefined`; and a job queued in the first listener runs
after the script, not between listeners. A `preventDefault` leaves the box
as it was, with no `input`/`change`. A second `click()` from inside the
first's listener answers `undefined` and does nothing, and the flag is down
again afterwards.

**Tests.** `alo-renderer/tests/a_scripts_click.rs` has 10 tests. Each
scenario runs from an agent's press of *Go*, so the script's `click()`
nests inside the browser's trusted dispatch after the heap is set to
collect at every allocation. Each page is pressed both ways and must
agree. Besides the closing condition, the tests cover:
- a click on another element from inside a listener;
- disabled controls, including a disabled fieldset's legend;
- a cancelled radio;
- a box removed by its listener, which changes and tells nobody;
- a throwing listener reported while the click carries on, plus
  `handleEvent` objects and a `handleEvent` getter on `input`/`change`
  (the driver's offset steps);
- a cancelled link click through a span, and an uncancelled one refused
  as item 263;
- `click` present on HTML elements and absent on SVG ones, the
  `HTMLElement` link in the chain, and `TypeError` on the wrong `this`.

`html_element.rs` adds a unit test of which elements count as disabled
form controls.

An agent's `Activate` already refuses a disabled control in `alo-agent`,
before the renderer is reached. It uses `alo-box`'s state, which is the same
`alo-css` rule plus `aria-disabled`, so the two callers agree on native
controls.

**Doctored runs**, each restored from a copy and diffed back. Each of these
fails a test:
- `Clicking` not traced (fails under stress);
- no click in progress flag;
- a cancelled click not undone;
- the click trusted;
- a disabled control clicked;
- a link silently not followed;
- no `input`/`change`.

**Compliance review.**
- Law 1: nothing legacy — no `HTMLElement` global, no other members.
- Law 2: no verb added or changed; the gate's coordinate check passes.
- Law 3: no stubs and no `unwrap` outside tests. The link case is refused
  by name rather than faked.
- Law 4: no `unsafe`.
- One file, one responsibility: the interface (`html_element.rs`), what
  the click keeps (`clicking.rs`), and how a native drives a dispatch
  (`scripted.rs`, which removed a copy from `event_target.rs`).
- Positions and sizes: nothing positions or sizes. The corpus, including
  `alo-settings` and `a-script-hears-an-event`, passes with every
  reference unchanged now that every HTML element's chain has gained
  `HTMLElement`.
- Bytes from outside: none newly read. `click()` takes no arguments.

**Gate.** `scripts/gate.sh` exited 0, run in the foreground and read in the
same step (7 min 18 s). Every check passed: formatting clean, clippy
silent, all tests pass, nothing stubbed, `unsafe` forbidden, licence
notices, every rented crate behind its boundary, no coordinate verb, the
stop rule holds, and `CHANGELOG.md` changed with the code.
`git diff --check` passes. `cargo doc -p alo-bindings` warns exactly as
often as before the change (6, none from this item's files). The log is in
this session's scratchpad, not committed.

**Roadmap.** The *Events* line's Built clause gains a script's `el.click()`.
Its Owed clause is now 263, 262, 257, 258 and 259. It is not ticked.
`docs/features.md`, `CHANGELOG.md`, `QUEUE.md` (261 ticked with its note;
262 and 263 added), `REMAINING.md` and `alo-bindings`' crate docs moved
with it.

**Unresolved obligations.**
- New from this item:
  - A script's click on a link (263) and each element's own interface
    (262).
  - A `click()` abandoned by a non-page escape inside a listener (a stop,
    a full heap, something not built yet) leaves that element's click in
    progress flag set, as an abandoned `dispatchEvent` leaves its event
    flagged. Later `click()`s on that element then do nothing.
  - `click()`'s event has no `view` (no `Window`, item 251).
- Carried from iteration 155, unchanged:
  - the scriptless-page link difference, radio form owners and
    checkedness (82);
  - a stopped click task's changed box;
  - absent `getModifierState`, coordinates and constructors;
  - 233 still owes the loop between messages;
  - several realms in one heap, and a second document cell's
    unforgeables;
  - `document` as an accessor (251), and node constructors;
  - 221's refusals;
  - an abandoned dispatch's flag;
  - `hand_over_reported`'s 256 roots;
  - other thrown objects said as `an object` (78);
  - a script-inserted `<meta>` policy;
  - detached trees waiting for the first collection;
  - the undiscriminated heap-refused document path;
  - the heap ceiling's overshoot;
  - 248's fallback;
  - 78's remainder; 77 needs design; 233, 234, 238 and 240 are open, and
    76 is not done;
  - `violations::reports` uncalled (203);
  - iteration 141's font-name guard.

113 queue items are open. Next is **257** (`PutText`'s input events, which
depends only on 256). 262 and 263 wait on a page and a decision
respectively. The next unused queue number is **264** and the next ADR
**0020**. This is one iteration, not a finished queue or roadmap.

## Iteration 157 — item 257 built: `PutText` fires `beforeinput`, `input` and `change`

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its *Events* line, iteration 156's entry,
`REMAINING.md`'s continuation order, queue items 81 and 254–263, ADR 0018 in
full (§ 5 is this item's decision), and the code it touches: `alo-agent`'s
`verb.rs` and `apply.rs`; `alo-dom`'s `activation.rs` and `lib.rs`;
`alo-bindings`' `event.rs`, `interface.rs`, `interface/ui_event.rs`,
`mouse_event.rs`, `pointer_event.rs`, `element.rs` and `lib.rs`;
`alo-renderer`'s `renderer.rs`, `press.rs`, `held.rs`, `event_loop.rs`,
`event_loop/task.rs`, `activated.rs`, `dispatched.rs`, `wire.rs`; and the
renderer's agent tests. No `AGENTS.md` exists. No sibling repository was
read or modified. The checkout was clean on entry at `ca91e9e`.

**Selection.** Iteration 156 and `REMAINING.md` named **257** next; its one
dependency, 256, is done; its decision is ADR 0018 § 5 (*`PutText` fires
what replacing a field's text fires*), its feature line *Events*. Like 261
it was cut with a closing condition rather than opened by a frozen page, so
no corpus case was added and none is claimed.

**What was built.**
- `alo-bindings`: `InputEvent` (inheriting `UIEvent`, no interface object
  on the global, as for the rest of its family) with `data`, `inputType`
  and `isComposing` (`false`); `dataTransfer`, `getTargetRanges()` and the
  constructor are absent, said in `interface/input_event.rs`. The event
  cell's `Shape::Input` holds the `inputType` and `data`, counted in its
  footprint. `Fired` gains a lifetime and `InputEvent { input_type, data }`.
  `Firing::before_replacing` is cancelable; `Firing::replaced` is the
  `input`, not cancelable. `UIEvent`'s `detail` is `0` for an `InputEvent`.
- `alo-dom`: `field.rs`, what text put into a field does (the `value`
  attribute until item 82). It has two callers, `apply` and the renderer,
  so for ADR 0017 § 5's reason it lives once, as `activation.rs` does.
- `alo-agent`: `Outcome::TextCanceled`. **This is a decision made inside a
  code commit, and a reviewer may want it written down.** The ADR says a
  cancelled `beforeinput` changes nothing, but not what the agent is told.
  Answering `TextPut` would be false. It is not a `Refusal`, because the
  verb was carried out and the page said no. It crosses the wire as
  outcome tag 4.
- `alo-renderer`: `Work::PutText`, run by `event_loop/typed.rs`: the
  `beforeinput`, then nothing more if it was cancelled, otherwise the text,
  `input` and `change`, all one task with a checkpoint after every listener.
  The task writes what it came to into `Turn::typed` *as each step
  happens*, so a page that stops partway still gets a true answer.
  `Held::put_text` (the three queueing methods now share one helper).
  `put.rs` answers `TextPut` or `TextCanceled`. On a page that stopped,
  before or during the task without answering the `beforeinput`, the text
  still goes in and nobody is told, as a stopped page's box is still
  ticked. `run_to.rs` holds the loop-running and the bounded saying, taken
  out of `press.rs` so both verbs share it; the wording of a click's lines
  is unchanged and its tests pass.

**Closing condition, met.** In `an_agents_text.rs`, a page's `input`
listener writes *You typed 12.50* into an `<output>` and the agent's
`ReadTree` reads it, ordinarily and collecting at every allocation. The
echo's text is laid out at (174, 55.2) 129.45313×18.625, asserted in
numbers.

**Tests.** `an_agents_text.rs` has 9 tests:
- the order, with every member read: `beforeinput` sees the old text with
  the new in `data`, and its job runs before the text goes in;
- each event bubbling to the document;
- a cancelled `beforeinput` answering `TextCanceled` and leaving the
  field's box as it was;
- a passive listener unable to cancel, and `input` uncancelable;
- the chain and brand checks;
- a throwing listener said as `the text: …` while the text still goes in;
- a field removed by a `beforeinput` listener, which still takes the text
  and hears `input` and `change`, while the document hears nothing;
- a stopped page;
- a scriptless page, which builds no heap.

`field.rs` adds 2 unit tests. The outcome's display has a unit test and its
wire round-trip is in `messages_across_a_boundary.rs`.

**Doctored runs.** Each of these fails a test:
- the cancel ignored;
- the text put before `beforeinput`;
- no `change`;
- `input` cancelable;
- a page stopped mid-task not given the text;
- `data` not held;
- a cancel answered as `TextPut`;
- `UIEvent`'s `detail` refusing an `InputEvent`.

**A note for whoever doctors next.** The first pass restored each file by
moving a copy back. That gives the file an mtime older than the doctored
build, so cargo kept the doctored binary: later runs reported failures the
code did not have, and one test failed on clean sources until the files
were touched. Every result above is from a second pass that touched each
file on restore, with the clean tree passing 9/9 before and after. Restore
with a fresh mtime, or the evidence is about the wrong binary.

**Compliance review.**
- Law 1: nothing legacy.
- Law 2: no verb added or changed in what it takes; the coordinate check
  passes. A page still cannot tell an agent's text from a person's (§ 4).
- Law 3: no stubs, and no `unwrap` outside tests.
- Law 4: no `unsafe`.
- One file, one responsibility: the interface (`input_event.rs`), the
  field rule (`field.rs`), the task (`typed.rs`), the verb's answer
  (`put.rs`), and running to a task (`run_to.rs`, split out of `press.rs`
  in this change because it gained a second caller).
- Positions and sizes: the echo's box is asserted in numbers.
- Reference renders: nothing about how anything is drawn changed. The
  corpus, `alo-settings` included, passes with every reference unchanged.
- Bytes from outside: none newly read. The text comes from the agent
  through the existing wire reader.

**Gate.** The first `scripts/gate.sh` run failed on `cargo fmt` alone:
clippy, tests and every other check passed. After `cargo fmt` (formatting
only) and three doc-comment link fixes, it was run again in the foreground
on the tree being committed and read in the same step; that run is
recorded in this entry's last lines. `git diff --check` passes.
`cargo doc` warns exactly as often as before the change: `alo-bindings` 5,
`alo-renderer` 9, `alo-dom` 1, `alo-agent` 0, compared against a stash of
the change.

**Roadmap.** The *Events* line's Built clause gains `PutText`'s events. Its
Owed clause is now 263, 262, 258 and 259. It is not ticked.
`docs/features.md`, `CHANGELOG.md`, `QUEUE.md` (257 ticked with its note)
and `REMAINING.md` moved with it.

**Unresolved obligations.**
- New from this item:
  - `Outcome::TextCanceled` is an agent-surface addition decided in code
    (above).
  - `change` follows `input` at once, as § 5 says. When focus exists
    (258), when a field is committed may need revisiting.
  - A field's text is still its `value` attribute (82).
  - The `beforeinput` and `input` have no `view` (251).
- Carried from iteration 156, unchanged: 263 and 262; an abandoned
  `click()`'s flag; `click()`'s missing `view`; and the whole list in that
  entry.

112 queue items are open. Of the events group, 258 needs design, 259 and
262 wait on a page, and 263 needs its ADR as its own iteration. The next
iteration takes the first eligible item as `LOOP.md` says. The next unused
queue number is **264** and the next ADR **0020**. This is one iteration,
not a finished queue or roadmap.

**Final gate run.** `scripts/gate.sh` exited 0 on the tree committed here,
run in the foreground and read in the same step (7 min 26 s). Every check
passed: formatting clean, clippy silent, all tests pass, nothing stubbed,
`unsafe` forbidden, licence notices, every rented crate behind its
boundary, no coordinate verb, the stop rule holds, and `CHANGELOG.md`
changed with the code. The log is in this session's scratchpad, not
committed.

## Iteration 158 — item 263's decision: ADR 0020, a page asks to go somewhere and the browser process decides

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its *Events* and *Navigation* lines,
iteration 157's entry, `REMAINING.md`'s continuation order, every open queue
item's dependency line (233, 234, 251 and 258–263 in full, and 43 and 190),
ADRs 0005, 0012 and 0018 in full and 0019's head, and the code the decision
is about: `alo-renderer`'s `message.rs`, `page.rs`, `tab.rs`, `press.rs` and
`renderer.rs` (`act`); `alo-agent`'s `verb.rs`; `alo-dom`'s `activation.rs`;
`alo-bindings`' `interface/html_element.rs`; `alo-net`'s `cause.rs`,
`chain.rs`, `schemes.rs` and `download.rs`'s head. No `AGENTS.md` exists.
`alo-workplace/web/src/tasks/FilesView.tsx` and `TaskDetail.tsx` were read,
read-only, for their `a.click()`; no sibling repository was modified. The
checkout was clean on entry at `b31efce`.

**Selection.** In file order, every open item before 263 is blocked on an
unbuilt dependency, needs design, waits on a frozen page, or (233) owes only
the loop running between messages, which no task source yet exists to
exercise — as iterations 155–157 recorded. 263's one code dependency, 261,
is done, and its other dependency is a decision it names as **needs ADR if
ADR 0005 and 0012 do not already decide it**. They do not: 0005 decides the
direction (no call back) and 0012 who names the cause, but neither what a
renderer may ask to navigate to, where the URL is resolved, what several
asks in one task come to, or what a download is. `LOOP.md` stage 2 § 4: a
decision is its own iteration. So this iteration is ADR 0020, and no code.
(Item 190, ready and small, is after 263 in the file.)

**What was decided** (ADR 0020):
- § 1: a navigation is a **claim in the answer** to the message whose work
  made it (`Loaded`, `Acted`, and 233's message when it exists), as `wanted`
  and `objections` already are; never a call, never awaited, never answered.
  While script runs it is held in the document cell, reached by `click()`
  through `[[HostDefined]]` (ADR 0019) — HTML's ongoing navigation. An
  agent's own link goes through the same ask; `Outcome::Followed` stays what
  the agent is told.
- § 2: the renderer resolves the URL against the document's base, so `Page`
  gains its URL, stated by the browser process. The ask also says how it
  arose (the browser's click or a script's) and the link's referrer policy —
  *what*, never *who*. A `target` naming another window is refused by name.
- § 3: the browser process parses the URL again; refuses one that does not
  parse or is over 2 MiB (Chromium's limit for the same check); navigates
  `http`, `https`, `about:blank`; `file:` only from a `file:` document;
  refuses `data:`, `javascript:`, `blob:`, every other scheme and its own.
  A refusal is said and recorded under ADR 0012; the page is told nothing.
- § 4: the cause is assigned from which message was answered — `Act` →
  `Cause::Agent`, otherwise `Cause::Document` — applying ADR 0012 § 4
  rather than amending it; the claim *a script's click* is kept beside it.
- § 5: one ask per answer, the last, with a count of those replaced.
- § 6: `<a download>` is a different ask (own origin, `data:`, `blob:`;
  cross-origin is a navigation; a cleaned name; `blob:` bytes cross with it;
  nothing written where the person did not choose), **cut to item 264**.

**Decisions a reviewer may want to look at.** The scheme table in § 3 is
policy: in particular refusing page-initiated `data:` navigations and
allowing `file:` only from `file:` documents. Both follow what Chromium and
Gecko ship, cited in the ADR, but they are choices, and they are written
down so they can be argued with. § 6's "`ping` is never sent" is stated
under *What this does not decide* as absent, with adding it a decision.

**Compliance review.**
- Law 1: nothing legacy enters; `javascript:` URLs and `ping` stay absent.
- Law 2: no verb is added or changed; the agent's surface is unchanged.
- Law 3: no code, so no stubs. The closing condition of 263 was made exact
  in the queue rather than loosened.
- Law 4: no `unsafe`.
- ADRs: 0005 and 0012 are applied, not amended; 0018 § 6's
  `activation.rs` keeps saying what follows a click.
- One file, one responsibility: no source file changed.
- Layout assertions, reference renders: nothing positions, sizes or draws.
- Bytes from outside: none read yet. The ADR requires the URL to be parsed
  as hostile in the browser process, which is 263's code to test with
  malformed and oversized input.
- `docs/features.md` is not changed: its *Events* line already promises a
  script's link click (263) and *Navigation* is unchanged; a decision is
  not a built feature. `ROADMAP.md`'s *Events* Owed clause names ADR 0020
  and 264; nothing is ticked. The *Navigation* line is not moved because
  nothing on it is built.

**Gate.** `scripts/gate.sh` exited 0 on the tree committed here, run in
the foreground and read in the same step (6 min 54 s): formatting clean,
clippy silent, all tests pass (the script runs under `pipefail`), nothing
stubbed, `unsafe` forbidden, licence notices, every rented crate behind its
boundary, no coordinate verb, the stop rule holds, and documentation
changed with the code (no code changed). `git diff --check` passes. The log
is in this session's scratchpad, not committed.

**Roadmap.** The *Events* line's Owed clause now says 263's decision is
ADR 0020 and adds 264. It is not ticked.

**Unresolved obligations.**
- New from this iteration: 263's code (now buildable: `Page`'s URL, the
  ask in the cell and in `Loaded`/`Acted` with a wire tag, the browser's
  parse, scheme table and cause, hostile-URL tests); 264 waits on 120 and
  on a `Blob` item that does not exist yet.
- Carried from iteration 157, unchanged: `Outcome::TextCanceled` decided in
  code; `change` timing pending focus (258); a field's text is its `value`
  attribute (82); `beforeinput`/`input` have no `view` (251); 262; an
  abandoned `click()`'s flag; `click()`'s missing `view`; 233 still owes the
  loop between messages; 78's remainder; 77 needs design; 234, 238 and 240
  open and 76 not done.

113 queue items are open (264 added). **263** is next, now buildable. The
next unused queue number is **265** and the next ADR **0021**. This is one
iteration, not a finished queue or roadmap.

## Iteration 159 — item 263: a script's `click()` follows a link, by asking

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its *Events* line, iteration 158's entry,
`REMAINING.md`'s continuation order, item 263 and 264 in full, ADR 0020 in
full (item 263's decision) and the code it names: `alo-renderer`'s
`message.rs`, `page.rs`, `tab.rs`, `press.rs`, `renderer.rs`, `held.rs`,
`run_to.rs`, `wire.rs` and `event_loop/activated.rs`; `alo-bindings`'
`interface/html_element.rs`, `document_cell.rs` and `embed.rs`;
`alo-dom`'s `activation.rs`; `alo-net`'s `referrer.rs`, `activity.rs`,
`request.rs` and `cause.rs`; `alo-url`'s `parts.rs` and `parse.rs`. No
`AGENTS.md` exists. No sibling repository was read or modified. The
checkout was clean on entry at `ec2cd25`.

**Selection.** Iteration 158 left 263 next and buildable: its code
dependency 261 is done and its decision is ADR 0020. Every open item
before it in file order is blocked, needs design, or waits on a page, as
iterations 155–158 recorded.

**What was built** (ADR 0020 §§ 1–5):
- `alo-url`: `Url::about_blank()` and `Url::is_about_blank()`, the made
  one tested equal to the parsed one.
- `alo-bindings`' new `navigating.rs` (one responsibility: where a page has
  asked to go): `follow` — a `download` link and a target naming another
  window (the link's `target`, else the first `<base target>`; `""`,
  `_self`, `_parent`, `_top` are this tab) are not followed; the `href` is
  resolved against the first `<base href>` (resolved against the
  document's URL) or the document's URL; one that does not resolve goes
  nowhere — and `Ongoing`, the page's ongoing navigation: one, the last, a
  count of those replaced, at most `MOST_NOT_FOLLOWED` (16) reasons kept
  and the rest counted. `DocumentCell` holds the document's URL and the
  `Ongoing`. A script's `click()` records its link there after its
  listeners and returns; on an `<a download>` it is refused by name for
  item 264, as ADR 0020 § 6 says.
- `alo-renderer`: `Page::url` (`about:blank` by `Page::new`,
  `response.url` by `from_response`, `Page::at`), stated to the cell by
  `Held::scripted(url)` before any script runs; the browser's click records
  its link in the same cell (`event_loop/activated.rs`, `Held::follow` on a
  stopped page), so a listener's `click()` and the agent's own link are one
  order and the agent's, coming after its listeners, is the last; a page
  that never ran script is asked for in `Renderer::act` itself. New
  `ask.rs`: `Asked` (URL serialised, `By`, referrer policy from
  `referrerpolicy` or `rel=noreferrer`, replaced count), carried as
  `navigation` in `Loaded` and `Acted`, with the reasons and the replaced
  count said among the issues. `Outcome::Followed` is answered only when
  following started a navigation. The wire carries the page's URL (parsed
  on arrival) and the ask (every tag checked, the URL as text).
- Browser side, new `navigate.rs`: `decide` parses the ask's URL again,
  refuses one over `LONGEST_URL` (2 MiB) or unparsed, navigates `http`,
  `https`, `about:blank` and `file:` only from a `file:` document, refuses
  every other scheme by name (`Rule`), works out `Referer` with `alo-net`'s
  `referrer::for_request` from its own copy of the document's URL;
  `Refusal::record` writes the refusal into an `Activity` as ADR 0012 asks
  (not for a URL that did not parse, which names nothing a line can hold —
  it is only said). `Tabs::load` decides a `Loaded`'s ask with
  `Cause::Document` naming the document it made; `Tabs::act` an `Acted`'s
  with `Cause::Agent` naming its action and the tab's document;
  `Tabs::navigation` hands the decision over once; `Tab::address` is the
  browser's copy of the document's URL.

**Decisions made in code a reviewer may want to look at.**
- *Not followed* (a download from the browser's click, another window, an
  unresolvable `href`) replaces no ongoing navigation, following HTML, and
  is said rather than thrown. A script's click on a download is still
  *thrown* by name (`Missing::InTheEmbedder`), because ADR 0020 § 6 says it
  stays refused as every link was.
- An agent's `Activate` on a link that was not followed now answers
  `Outcome::Activated` (it answered `Followed` before); a link on a page at
  `about:blank` with a relative `href` therefore no longer answers
  `Followed`. Three existing tests' pages were given an address
  (`what_an_agent_set_off.rs`, `an_agents_click.rs`); their assertions are
  unchanged.
- An `Act`'s ask in a tab with no document is not decided (nobody to
  attribute it to). A failed load or a refused act clears any decision
  waiting.
- The ask's `referrerpolicy` is read with `alo-net`'s `Policy::named`
  (trimmed, any case), slightly looser than HTML's enumerated attribute.
- A page's CSP `base-uri` is not enforced on `<base>`; there is no item.

**Found, not fixed.** `alo-net`'s `referrer::for_request` writes an
origin-only `Referer` as `https://example.com`, without the `/` the
standard's URL serialisation has. Queued as **item 265**;
`navigate.rs`' test asserts the current value and names 265.

**Compliance review.**
- Law 1: nothing legacy; `javascript:` URLs, `ping` and windows stay
  absent and are refused by name.
- Law 2: the agent's surface gains no verb; `Followed` now means a
  navigation was asked for.
- Law 3: no stubs, `todo!`, or `unwrap` outside tests. Going there (85),
  downloads (264) and windows (118) are named absences.
- Law 4: no `unsafe`.
- ADRs: 0020 is applied as written; 0005 (no call back: the ask is a field
  of an answer, never awaited), 0012 § 4 (the cause from the message), 0018
  § 6 (`activation.rs` still says what follows; the callers record).
- One file, one responsibility: `navigating.rs` (renderer-side following
  and the ongoing ask), `ask.rs` (the ask as it crosses), `navigate.rs`
  (the browser process's decision). `wire.rs` gained a `texts` helper to
  stay within clippy's function length rather than an `allow`; one wire
  round-trip test was split for the same reason.
- Layout assertions, reference renders: nothing positions, sizes or
  draws. The corpus passes with every reference unchanged.
- Bytes from outside: the ask's URL is parsed in the browser process as
  hostile — empty, nonsense, an unclosed IPv6 host, a space in a host, a
  NUL, one byte over 2 MiB (refused, said at 200 characters) and exactly
  2 MiB (navigated) are tested; the wire refuses an unknown cause or
  policy tag, a replaced count over `u32`, every truncation of an `Acted`
  carrying an ask, and a page whose address is not a URL.
- `cargo doc` warns exactly as often as before: `alo-url` 2 (both
  pre-existing), `alo-bindings` 5, `alo-renderer` 9.

**Tests.** `alo-renderer/tests/a_page_asks_to_go_somewhere.rs` (14; the
scripted pages pressed ordinarily and collecting at every allocation; three
drive real `Tabs` over the confined `alo-render` binary and are 263's
closing condition: a load-time `a.click()` reaches `Tabs::navigation` as
`Cause::Document` and *a script's click*; inside an agent's `Activate` the
same click is `Cause::Agent` with the same claim; `data:`, `javascript:`
and `file:` from the web are refused, said, recorded, and go nowhere).
Unit tests: `navigating.rs` 8, `ask.rs` 3, `navigate.rs` 7, `parts.rs` 1;
wire: a round trip of every policy and claim, and the refusals above.
Doctored runs, each restored: the first ask kept instead of the last (5
failures), `data:`/`javascript:`/`file:` navigated (6), an `Act`'s ask
attributed to the document (2), the browser's click not recorded (4), the
base target ignored (1, in `alo-bindings`), the base href ignored (2).

**Roadmap.** The *Events* line's Built clause gains *a followed link is an
ask*; its Owed clause drops 263 and gains going there (85). It is not
ticked. The *Navigation and session history* line is not moved: nothing
loads the decided navigation yet (85). `docs/features.md`, `CHANGELOG.md`,
`QUEUE.md` (263 ticked with its note, 265 added) and `REMAINING.md` moved
with it.

**Unresolved obligations.**
- New from this item: going where the browser decided (85); downloads
  (264); windows (118); 265; CSP `base-uri` (no item); what an agent is
  told when the page it acted on goes somewhere (134).
- Carried from iteration 158, unchanged: `Outcome::TextCanceled` decided
  in code; `change` timing pending focus (258); a field's text is its
  `value` attribute (82); `beforeinput`/`input` have no `view` (251); 262;
  an abandoned `click()`'s flag; `click()`'s missing `view`; 233 still owes
  the loop between messages; 78's remainder; 77 needs design; 234, 238 and
  240 open and 76 not done.

113 queue items are open (263 closed, 265 added). The next iteration takes
the first eligible item as `LOOP.md` says (iteration 158 noted 190, ready
and small; 265 is ready too). The next unused queue number is **266** and
the next ADR **0021**. This is one iteration, not a finished queue or
roadmap.

**Final gate run.** `scripts/gate.sh` exited 0 on the tree committed here,
run in the foreground and read in the same step (7 min 24 s): formatting
clean, clippy silent, all tests pass (the corpus included, every
reference unchanged), nothing stubbed, `unsafe` forbidden, licence
notices, every rented crate behind its boundary, no coordinate verb, the
stop rule holds, and `CHANGELOG.md` changed with the code. `git diff
--check` passes. The log is in this session's scratchpad, not committed.

## Iteration 160 — item 265 built: an origin-only `Referer` ends in `/`

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and its *Content Security Policy, referrer
policy, HSTS, mixed-content blocking* line, iteration 159's entry,
`REMAINING.md`'s continuation order, item 265 in full, and the code it
names: `alo-net`'s `referrer.rs` and its tests in
`tests/what_a_page_may_reach.rs`, `alo-renderer`'s `navigate.rs`, and
`alo-url`'s `origin.rs` (the origin's `Display`). Item 265 names no ADR;
it serves `docs/features.md`'s *Referrer policy* line (queue item 62's).
No `AGENTS.md` exists. No sibling repository was read or modified. The
checkout was clean on entry at `9980822`.

**Selection.** In file order, 157, 158 and 187 keep their recorded
blockers (an interface to ask in, an interface to choose in, an upload
caller), and 265 depends on nothing. It is the first eligible item.

**What was built.** `referrer.rs` gains `origin_only`: the origin of the
referring URL, by `alo-url`'s `Origin` serialisation, followed by `/`.
`origin`, `strict-origin` and the cross-origin half of
`origin-when-cross-origin` and `strict-origin-when-cross-origin` all go
through it, so the four cannot drift apart. An opaque origin sends
nothing rather than `null/`. `for_request` only reaches it from `http`
and `https`, whose origins are never opaque, and the comment says so
rather than a test pretending to reach it.

**Decision a reviewer may want to look at.** The trailing `/` follows
what other engines send, which the queue item named as the target. A
strictly literal reading of the URL serialiser over a path set to the
empty list would give no `/`. Browsers do not send that form, and this
iteration did not settle the reading against the specification text.
That is why the changelog says "the form other browsers send" and does
not cite the standard.

**Compliance review.**
- Law 1: nothing legacy. Law 2: the agent surface is unchanged. Law 3: no
  stubs, `todo!` or `unwrap` outside tests. Law 4: no `unsafe`.
- One file, one responsibility: `referrer.rs` still answers one question,
  what `Referer` says. The new function is part of that answer.
- Layout assertions, reference renders: nothing positions, sizes or draws.
  The corpus passes with every reference unchanged.
- Bytes from outside: no new parsing. The input is a `Url` that was already
  parsed, and the hostile-URL tests in `navigate.rs` still pass.

**Tests.** `the_origin_a_site_is_told_is_written_as_a_url` covers each
origin-only policy across sites, from a default port written out, another
port, `http` with a port and two IPv6 hosts. It also checks `origin` and
`strict-origin` to the same site, and `origin` across a downgrade. The
existing default-policy test and `navigate.rs`' referrer test now assert
`https://example.com/`, and the comment pointing at 265 is gone. Doctored
run: with the `/` removed, 2 `alo-net` tests and 1 `alo-renderer` test
failed; the source was then restored.

**Roadmap.** The *Content Security Policy, referrer policy, HSTS,
mixed-content blocking* line's Built clause now says the origin-only form
is written as a URL (item 265). It is not ticked; its Owed clause is
unchanged. `docs/features.md`, `CHANGELOG.md`, `QUEUE.md` (265 ticked with
its note) and `REMAINING.md` changed with it.

**Unresolved obligations.** None new. Carried from iteration 159 unchanged:
going where the browser decided (85), downloads (264), windows (118), CSP
`base-uri` (no item), and what an agent is told when the page it acted on
goes somewhere (134). The iteration 158 carry-overs also stand.

112 queue items are open. Iteration 158 noted 190 as ready and small; the
next iteration takes the first eligible item as `LOOP.md` says. The next
unused queue number is **266** and the next ADR **0021**. This is one
iteration, not a finished queue or roadmap.

**Final gate run.** `scripts/gate.sh` exited 0 on this tree. It ran in the
foreground and was read in the same step (7 min 12 s). Formatting is
clean, clippy is silent, and all tests pass, the corpus included with
every reference unchanged. Nothing is stubbed, `unsafe` is forbidden, the
licence notices are present, and every rented crate stays behind its
boundary. No verb takes a coordinate, the stop rule holds, and
`CHANGELOG.md` changed with the code. `git diff --check` passes. The log is
in this session's scratchpad and is not committed. The only later change
was this journal entry, which is documentation.

## Iteration 161 — item 190 built: `inset`, `outset`, `groove` and `ridge`

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions, its *CSS beyond what alo needed* section and its
*Forms* line, iterations 158–160's entries, `REMAINING.md`'s continuation
order, queue items 157–265 for eligibility and item 190 in full, and the code
it names: `alo-paint`'s `build.rs` (`draw_borders`, `draw_banded_border`,
`uniform_border`, `border_style`, `border_color`), `corner.rs` (`ring`,
`Corners`), `display.rs`, `render.rs`'s clip, `alo-style`'s user-agent sheet,
`alo-value`'s `shorthand.rs`, and `alo-corpus`'s case layout. Item 190 names
no ADR; its feature home is `docs/features.md`'s *Paint* section, where this
change adds its line. No `AGENTS.md` exists. No sibling repository was read
or modified. The checkout was clean on entry at `a81835a`.

**Selection.** In file order, everything before 263 keeps its recorded
blocker (iterations 155–160). After it: 264 waits on 120 and `Blob`; 84 on
76; 43's remainder on focus (258). Items 82, 83 and 85–89 have every
dependency done, but each is a one-line capability naming no ADR, feature
contract or closing condition — `LOOP.md` step 2 says such an item is not
ready, so each is now marked `needs design` in the queue rather than left
for the next iteration to re-derive. 190 depends on nothing and has a
closing condition; iteration 158 had already named it ready. It is the
first eligible item.

**What was built.** `alo-paint`'s new `border.rs`, one responsibility —
what a two-toned border is drawn as:
- `Line`: the styles drawn (`solid` and the four); anything else is `None`
  and left undrawn rather than drawn as something else.
- `tones`: the darker tone takes a third off the brightest channel and
  scales the others with it (hue kept), the lighter adds a third up to
  white; black lightens to a third-grey. Never equal for any colour; alpha
  kept. CSS leaves the colours to the browser, and the changelog and
  conformance page say ours are ours rather than claiming another engine's.
- `colors_of`: lit from the top left. `inset` dark top/left, `outset` the
  reverse, `groove` an inset outer half and outset inner half, `ridge` the
  reverse.
- `draw_mitred`: each side is the **wedge of the box it is fewest of its own
  widths from** — the mitre at each corner, a proportional straight cut
  between opposite sides, so the four wedges are the whole box exactly once
  — filled one path per colour inside the border's ring, then the outer
  halves again inside the ring half as thick, so a groove's halves follow a
  rounded corner.
`build.rs` takes that path when any side is two-toned (a solid side beside
one is mitred too); an all-solid border keeps its rectangles and ring, so no
existing reference moved.

**Two wrong versions, caught before the commit.** The first stopped each
side at the padding box's rectangle and left a hole in every rounded corner
(the rounded-groove pixel test failed; a dump of the corner showed it). The
second bounded each side by its two mitres only, which overlapped the side
opposite on uneven widths (the area-sum test failed: 5416.7 of 5000).

**Scope cut, written into the queue.** 266: `dashed`, `dotted`, `double` —
patterns along a side, not tones across it. 267: a fieldset's legend-broken
groove — `draw_banded_border` stays solid-only, so the user-agent sheet
still says `solid`, and its comment now names 267 rather than 190.

**Compliance review.**
- Law 1: nothing legacy. Law 2: the agent surface is unchanged. Law 3: no
  stubs, `todo!` or `unwrap` outside tests; two-toned sides that are not
  implemented are left empty, never approximated. Law 4: no `unsafe`.
- One file, one responsibility: `border.rs` is the two-toned border;
  `build.rs` gained only the dispatch to it (`two_toned`), not the drawing.
- Layout assertions: nothing new is positioned or sized; the case's
  `layout.txt` records its boxes (44×36, and 46×32 for the uneven widths).
- Reference render: corpus case `border-styles` (172×96: the four styles in
  grey, a rounded groove and an uneven ridge in blue), looked at upscaled
  before committing. `ALO_UPDATE_REFERENCES=1` changed no other case.
- Bytes from outside: no new parsing; the inputs are computed styles and
  laid-out rectangles.

**Tests.** `border.rs` 13, `tests/two_toned_borders.rs` 10 (listed under
item 190 in the queue). Doctored runs, each restored: groove's halves
swapped (3 unit tests fail), the outer half clipped to the full ring instead
of half (3 pixel tests fail), only `groove` routed to the mitred path (8
pixel tests fail).

**Roadmap.** Item 190 served no roadmap line of its own: border styles are
not a line in *CSS beyond what alo needed*, and the item was cut from 183,
which served *Forms*. That line is not ticked; its Owed clause now names the
fieldset's groove (267) and says a groove on an ordinary box is drawn (190).
`docs/features.md` (*Paint*: two-toned borders), `docs/conformance.md`,
`CHANGELOG.md`, `QUEUE.md` (190 ticked with its evidence; 266 and 267 added;
82, 83, 85–89 marked `needs design`) and `REMAINING.md` moved with it.

**Unresolved obligations.**
- New: 266, 267. Where two different tones meet on a corner's diagonal the
  page shows faintly through one line of anti-aliased pixels (said in
  `border.rs` and `conformance.md`); 82, 83, 85–89 need their designs.
- Carried from iterations 158–160 unchanged: going where the browser decided
  (85), downloads (264), windows (118), CSP `base-uri` (no item), what an
  agent is told when the page it acted on goes somewhere (134), and the
  iteration 158 carry-overs.

113 queue items are open (190 closed; 266 and 267 added). The next iteration
takes the first eligible item as `LOOP.md` says; 266 and 267 depend on
nothing. The next unused queue number is **268** and the next ADR **0021**.
This is one iteration, not a finished queue or roadmap.

**Final gate run.** `scripts/gate.sh` exited 0 on this tree, run in the
foreground and read in the same step (7 min 20 s): formatting clean, clippy
silent, all tests pass (the corpus included, every existing reference
unchanged and `border-styles` new), nothing stubbed, `unsafe` forbidden,
licence notices present, every rented crate behind its boundary, no
coordinate verb, the stop rule holds, and `CHANGELOG.md` changed with the
code. `git diff --check` passes. The log is in this session's scratchpad and
is not committed. The only later change was this journal entry, which is
documentation.

## Iteration 162 — item 266 built: `dashed`, `dotted` and `double`

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md` (its conventions, *Forms* and *CSS beyond what alo needed*),
iteration 161's entry, `REMAINING.md`'s continuation order, the queue's
open items for eligibility and items 190, 266 and 267 in full, and the code
item 266 names: `alo-paint`'s `border.rs`, `build.rs` (`draw_borders`,
`draw_banded_border`, `border_style`, `border_color`), `corner.rs`,
`display.rs`, `path.rs`, `render.rs`'s clip, `raster.rs`'s fill rule
(non-zero), `tests/two_toned_borders.rs`, and `alo-corpus`'s case layout.
Item 266 names no ADR; its feature home is `docs/features.md`'s *Paint*
section, where this change adds its line. No `AGENTS.md` exists. No sibling
repository was read or modified. The checkout was clean on entry at
`ab5f39b`.

**Selection.** Iteration 161 recorded every item before 266 as blocked or
`needs design` and named 266 and 267 as depending on nothing; nothing has
changed since. 266 comes first in file order, so it is the first eligible
item.

**What was built.**
- `pattern.rs` (new): the spacing alone. Dashes are about three widths
  long with equal gaps, stretched so a side starts and ends on a dash.
  Dots are round and a width across, about a width apart, with the end ones
  centred half a width in, which is in the corners. `double` is cut in
  thirds. `MAX_PIECES` (16 384) caps a side's dashes or dots, and a side
  with a non-finite or non-positive length or width gets none.
- `border.rs`: `Line` gains the three styles, and `draw_mitred` draws in
  four layers, each one clip with one fill per colour. The first is sides
  across their whole width (solid, the toned sides' inner colour, dashes).
  Then the toned sides' outer halves, then `double` inside the outer and
  inner third rings (as one clip), then dots inside the ring and the
  wedges of their colour. A dash is its wedge cut across, and a corner dot
  is one dot because both sides' wedges clip it together.
- `build.rs`: any non-`solid` side takes the mitred path (`mitred_sides`,
  renamed from `two_toned`). `draw_banded_border` is still solid-only, and
  its comment names 268.
- `corner.rs`: `Corners::inside`, used by `ring` and by `double`'s inner
  ring.

**Two defects, found and fixed before the commit.**
1. *A seam of the page's colour along every dashed corner's mitre*, seen
   in the first reference render (pixel 101 where the dash is 51). The top's
   last dash and the right's first are cut off at different points on the
   shared mitre, which gives two different edges along one line. The
   rasteriser rounds each one separately, and its samples lie exactly on
   that 45° line. Fix: every point where a dash is cut off on a mitre
   becomes a joint (`mitre::Joints`). The piece cut there is snapped onto
   the joint exactly, and the joint is put into whatever lies across the
   mitre, so both sides share their edges to the bit.
2. *The first version of that fix was a denial of service.* It put every
   joint into every wedge before cutting, which is quadratic: a 0.01px
   dashed border round a 100 000px box took 81 s to build, debug, on this
   machine. Now each piece takes only the joints beside it, from sorted
   lists by binary search. The first version of *that* measured the range
   across rather than along the side, so a deep left or right wedge still
   took every joint (24 s). Measured along each side's own axis, the worst
   cases tried build in 0.1–0.2 s (debug, this machine). These timings are
   diagnostics, not a performance claim.

**One file, one responsibility.** `border.rs` had gained three reasons to
change, so this change splits it. `tone.rs` holds the two tones (`tones`,
`colors_of`) and their six tests, moved. `mitre.rs` holds which part of the
box is each side's (`Side`, `wedge`, `kept`, `polygon_path`) and the joints,
with four tests moved and five new. `border.rs` keeps what each side is drawn
as and in what order: `Line`, `DrawnSide`, `draw_mitred`, dashes and dots.

**Scope cut, written into the queue.** 268: the three styles beside a
fieldset's legend, where `draw_banded_border` still draws only `solid`.

**Compliance review.**
- Law 1: nothing legacy. Law 2: the agent surface is unchanged (the gate's
  coordinate check passes). Law 3: no stubs, `todo!` or `unwrap` outside
  tests, and a style that is not drawn is left empty, never approximated.
  Law 4: no `unsafe`.
- Bytes from outside: the inputs are computed styles and laid-out
  rectangles, which come from the page in stage 2. Non-finite, zero and
  negative lengths and widths give no pieces (unit test). Piece counts are
  bounded (unit tests at 1e30 and 1e9). Two pixel tests draw hair-thin
  dotted and dashed borders round 100 000px boxes, and they draw rather
  than panic or run away.
- Layout assertions: nothing new is positioned or sized. The case's
  `layout.txt` records its six boxes (44×36, and 42×34 for the mixed one).
- Reference render: corpus case `border-patterns` (172×96: dashed, dotted
  and double in grey; thin blue dashes; a rounded double; one box with a
  different style on each side). I looked at it upscaled before committing,
  and the seam above was found that way. `ALO_UPDATE_REFERENCES=1` changed
  no other case, so `border-styles` is unmoved.

**Tests.** `pattern.rs` 9, `mitre.rs` 9 (5 new), `border.rs` 10 (7 new),
`tone.rs` 6 (moved), `tests/patterned_borders.rs` 10 in pixels.
`two_toned_borders.rs`'s last test now uses `hidden` for its undrawn side,
since `dashed` is drawn. Each doctored run was restored afterwards:
- Joint ranges measured across: `a_piece_takes_only_the_joints_beside_it`
  fails.
- Snapping removed: `a_dash_cut_off_on_a_mitre_shares_its_corner_…` fails.
- Joints empty: that test and the pixel `a_dashed_corner_has_no_seam` fail.
- Square dots: `a_dotted_sides_dots_are_round_and_apart` fails.
- `double` clipped to the whole ring: `a_double_border_is_one_fill_…`
  fails.

The first draft of the selectivity and snapping tests did *not* fail when
doctored. They checked output that later code filters again. Both were
rewritten until they did.

**Roadmap.** Item 266 served no roadmap line of its own, for the same
reason as 190: border styles are not a line in *CSS beyond what alo
needed*, and the chain was cut from 183, which served *Forms*. That line is
not ticked. Its Owed clause now also names 268 and says these styles are
drawn on an ordinary box (266). `docs/features.md` (*Paint*: patterned
borders), `docs/conformance.md`, `CHANGELOG.md`, `QUEUE.md` (266 ticked
with its evidence, 268 added, 267's text pointing at `mitre.rs`) and
`REMAINING.md` moved with it.

**Unresolved obligations.**
- New: 268. Along a rounded corner, dashes and dots are placed on the
  straight side and clipped rather than spaced along the curve. Where
  sides of different widths meet, a corner dot is clipped to its own
  side's share. Both are stated in `border.rs` and `conformance.md`.
- Carried unchanged from iteration 161: 267, and 82, 83 and 85–89 need
  their designs. Also going where the browser decided (85), downloads
  (264), windows (118), CSP `base-uri` (no item), what an agent is told
  when the page it acted on goes somewhere (134), and the iteration 158
  carry-overs.

113 queue items are open (266 closed, 268 added). The next iteration takes
the first eligible item as `LOOP.md` says. 267 depends on nothing, and 268
is best taken with or after it. The next unused queue number is **269** and
the next ADR **0021**. This is one iteration, not a finished queue or
roadmap.

**Final gate run.** `scripts/gate.sh` exited 0 on this tree. It ran in the
foreground and was read in the same step (7 min 25 s). Formatting is
clean, clippy is silent, and all tests pass, the corpus included with
every existing reference unchanged and `border-patterns` new. Nothing is
stubbed, `unsafe` is forbidden, the licence notices are present, and every
rented crate stays behind its boundary. No verb takes a coordinate, the
stop rule holds, and `CHANGELOG.md` changed with the code. `git diff
--check` passes. The log is in this session's scratchpad and is not
committed. The only later change was this journal entry, which is
documentation.

## Iteration 163 — item 267 built: a fieldset's `groove`

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md` (its conventions and the *Forms* line), iterations 161 and
162's entries, the queue's section F for eligibility and items 190, 266, 267
and 268 in full, and the code item 267 names: `alo-paint`'s `build.rs`
(`draw_borders`, `draw_banded_border`, `mitred_sides`), `border.rs`
(`draw_mitred`), `mitre.rs`, `tone.rs`, `corner.rs`, `coverage.rs`,
`render.rs`'s clip stack (clips intersect; non-zero fill), `alo-layout`'s
`legend.rs` (`Band`), `alo-style`'s user-agent sheet, the test
`a_border_a_legend_breaks.rs` and corpus case `fieldset-group`. Item 267
names no ADR; its feature home is `docs/features.md`'s *Paint* section, whose
two-toned line it already named. No `AGENTS.md` exists. No sibling repository
was read or modified. The checkout was clean on entry at `0a4f17a`.

**Selection.** Iteration 162 recorded every item before 267 as blocked or
`needs design`, and nothing has changed since. 267 depends on nothing and
comes before 268 in file order, so it is the first eligible item.

**What was built.**
- `alo-paint`'s new `banded.rs`, one responsibility: the border a legend
  breaks. `draw_banded_border` moved out of `build.rs` into it, since adding
  tones to it would have given `build.rs` another reason to change.
  `build.rs` gains `drawn_sides` (which `mitred_sides` now uses) and only
  dispatches.
- Solid wherever drawn: the same five rectangles as before, in the same
  order.
- Any two-toned side: `border::draw_mitred` on the area below the band's
  inset, with the band's stroke as the top width, inside one clip. The
  clip is the area wound clockwise and the hole the other way round, so
  under the non-zero rule the hole is outside. The hole is the band's gap
  across the stroke's depth, **clamped to the side borders' inner edges**.
  So a legend wider than the box leaves the corners drawn, as the solid
  pieces always did. This matches how Chrome's fieldset painter cuts it (a
  clip-out of the legend's span); no clip is pushed when the hole is empty.
- `dashed`, `dotted` and `double` beside a legend stay undrawn (268). The
  module comment says so, and so do the queue and the conformance page.
- The user-agent sheet says `2px groove #c0c0c0`. Its comment about the
  `solid` stand-in is gone, replaced by one line naming `banded.rs`.

**Found on the way, not a defect of this change.** The layout's band gap is
the legend's *margin box*, so a negative margin shrinks the gap rather than
moving it. The first draft of the corner test pulled the legend left and so
tested nothing. It now uses a legend wider than the fieldset. Whether the gap
should be the margin box or the border box is `alo-layout`'s question. It is
not this item's, and nothing is changed for it.

**Compliance review.**
- Law 1: nothing legacy. Law 2: the agent surface is unchanged (the
  coordinate check passes; the corpus's `agent.txt` files did not move).
  Law 3: no stubs, `todo!` or `unwrap` outside tests; an undrawn style is
  left empty, never approximated. Law 4: no `unsafe`.
- One file, one responsibility: `banded.rs` is the legend-broken border,
  and `build.rs` lost that responsibility rather than gaining a second.
- Layout assertions: nothing is newly positioned or sized. The band's
  numbers stay asserted in `alo-layout`'s `numbers.rs`, and the
  `layout.txt` and `boxes.txt` files of both moved cases are unchanged.
- Reference render: `fieldset-group` and `web-a-form` moved (display list
  and picture only). I looked at `fieldset-group` upscaled 4× beside the
  old one: a groove broken by "Pizza size", the top and left dark outside
  and light inside, the right and bottom the reverse.
  `ALO_UPDATE_REFERENCES=1` changed no other case.
- Bytes from outside: no new parsing. The gap and widths come from layout;
  `max`/`min` clamping and the `ends > starts` test cover inverted or
  out-of-box gaps (unit tests).

**Tests.** `banded.rs` 8 unit tests. `a_border_a_legend_breaks.rs` has 5 new
pixel tests: both tones either side of the legend and nothing in the gap;
the gap's edges to the pixel; the other three sides a groove from the line
down; a legend wider than the box leaving the far corner; the sheet's two
tones (`rgb(107 107 107)`, `rgb(255 255 255)`) and never `#c0c0c0` itself.
Its 4 older tests now ask for `solid` by name, since they count rectangles.
Doctored runs, each restored afterwards: with no hole cut, 3 tests fail;
with the hole unclamped, 1 fails.

**Roadmap.** *Forms* is not ticked. Its Built clause now names the groove
(267, `banded.rs`); its Owed clause drops 267 and keeps 268, everything a
control does (81) and the focus ring (43). `docs/features.md` (*Paint*,
two-toned borders), `docs/conformance.md`, `CHANGELOG.md`, `QUEUE.md` (267
ticked with its evidence, 268's text now pointing at `banded.rs`) and
`REMAINING.md` moved with it.

**Unresolved obligations.**
- 268 is unchanged in scope. Its open question is now only where a dash
  falls beside the gap, because the clip that would cut it exists.
- A rounded corner on a fieldset showing a legend is still drawn square
  (said in `banded.rs` and `conformance.md`, as before).
- New, for whoever owns `alo-layout`'s legend: the gap is the margin box
  (above). No queue item is opened, because no page fails on it.
- Carried unchanged from iteration 162: 82, 83 and 85–89 need their
  designs. Also going where the browser decided (85), downloads (264),
  windows (118), CSP `base-uri` (no item), what an agent is told when the
  page it acted on goes somewhere (134), and the iteration 158 carry-overs.

112 queue items are open (267 closed). The next iteration takes the first
eligible item as `LOOP.md` says; 268 depends on nothing. The next unused
queue number is **269** and the next ADR **0021**. This is one iteration,
not a finished queue or roadmap.

**Final gate run.** `scripts/gate.sh` exited 0 on this tree (7 min 31 s). It
ran in the foreground and was read in the same step. Formatting is clean,
clippy is silent and all tests pass: the corpus is included, with
`fieldset-group` and `web-a-form` updated and every other reference
unchanged. Nothing is stubbed, `unsafe` is forbidden, the licence notices
are present, and every rented crate stays behind its boundary. No verb
takes a coordinate, the stop rule holds, and `CHANGELOG.md` changed with
the code. `git diff --check` passes. The log is in this session's scratchpad
and is not committed. The only later change was this journal entry, which
is documentation.

## Iteration 164 — item 268 built: patterned borders beside a legend

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md` (its conventions and the *Forms* line), iteration 163's entry,
the queue's section F (items 43, 190, 266, 267 and 268 in full), and the
code item 268 names: `alo-paint`'s `banded.rs`, `build.rs`'s border
dispatch (`draw_borders`, `drawn_sides`, `mitred_sides`), `border.rs`'s
`draw_mitred`, `alo-layout`'s `Band::inset`, the test
`a_border_a_legend_breaks.rs`, and corpus cases `border-patterns` and
`fieldset-group` with `alo-corpus`'s `case.rs`. Item 268 names no ADR; its
feature home is `docs/features.md`'s *Paint* section, the patterned-borders
line, which already named it. No `AGENTS.md` exists. No sibling repository
was read or modified. The checkout was clean on entry at `61488c4`.

**Selection.** Iteration 163 recorded every item before 268 as blocked or
`needs design`, and nothing has changed since. 268 depends on nothing and
is the first eligible item.

**The open question, and its answer.** The item asked whether a dash or dot
cut off by the legend is right, or whether the pattern should be spaced on
each piece. The answer is **laid along the whole side, then cut**:
`pattern.rs` spaces a side so it starts and ends on a dash at its corners,
and a legend moves neither corner. Spacing each piece afresh would make
every dash on the line depend on how long the legend's words are. That is
a paint detail inside an existing module, not a decision that needed an
ADR. It is written into `banded.rs`'s module comment and the queue.
Chromium and Firefox paint a fieldset the same way, with a clip-out of the
legend over an ordinary border. **That is from memory and was not re-read
this iteration.** The argument above does not rest on it.

**What was built.** `banded.rs` no longer filters `dashed`, `dotted` and
`double` out. Any border that is not solid wherever it is drawn now goes
through `border::draw_mitred` inside the clip 267 built, so the three
patterns are cut exactly as the groove is. The module comment's section on
what is not drawn now names only the radius.

**Compliance review.**
- Law 1: nothing legacy. Law 2: the agent surface is unchanged (the
  coordinate check passes; no `agent.txt` moved). Law 3: no stubs, `todo!`
  or `unwrap` outside tests. Law 4: no `unsafe`.
- One file, one responsibility: `banded.rs` is still the legend-broken
  border and lost code rather than gaining a responsibility.
- Layout assertions: nothing is newly positioned or sized. The band's
  numbers stay asserted in `alo-layout`'s `numbers.rs`, and no existing
  `layout.txt` or `boxes.txt` moved.
- Reference render: new corpus case `fieldset-patterns`, with three
  fieldsets (dashed, dotted, and double at 6px) each broken by its legend.
  I looked at it upscaled 4×. Each line is broken by its legend's words,
  the dotted one has a half-dot cut at each of the legend's edges, and both
  of the double's lines are cut. The first draft used the `font` shorthand,
  which this engine does not apply, and the legends came out 16px. It now
  uses `font-size` and `font-family`. `ALO_UPDATE_REFERENCES=1` changed no
  other case.
- Bytes from outside: no new parsing.

**Tests.** `banded.rs`: one test replaces the one asserting patterns were
left undrawn. For all three styles, it checks that the hole's clip is the
first item and that what is inside it is exactly `draw_mitred`'s output for
the same area. `a_border_a_legend_breaks.rs` gains 3 pixel tests:
- Over the stroke's whole depth, all three styles: white inside the
  legend's 30–70, and everywhere else the same box's stroke with no legend,
  pixel for pixel.
- Ink before the gap, after it, in the corner, down the left side and along
  the bottom.
- A dot spanning 68–74 is cut at 70.

Doctored runs, each restored afterwards:
- Patterns filtered out again: 3 of the new pixel tests fail, and so does
  the new `banded.rs` test.
- No hole cut: 5 pixel tests fail.

**Roadmap.** *Forms* is not ticked. Its Built clause now names 268 and
`fieldset-patterns`. Its Owed clause drops 268 and keeps everything a
control does (81) and the focus ring (43). Also updated:
`docs/features.md` (*Paint*, patterned borders), `docs/conformance.md`
(two paragraphs), `CHANGELOG.md`, `QUEUE.md` (268 ticked with its evidence)
and `REMAINING.md`.

**Unresolved obligations.**
- A rounded corner on a fieldset showing a legend is still drawn square
  (said in `banded.rs` and `conformance.md`). No queue item is opened,
  because no page fails on it.
- Carried from iteration 163: the legend's band gap is its margin box
  (`alo-layout`'s question; no page fails on it).
- Carried unchanged: 82, 83 and 85–89 need their designs. Also going where
  the browser decided (85), downloads (264), windows (118), CSP `base-uri`
  (no item), what an agent is told when the page it acted on goes somewhere
  (134), and the iteration 158 carry-overs.
- Not verified this iteration: the claim about what Chromium and Firefox do
  (above).

111 queue items are open (268 closed). Section F's next open item is 94,
which, like 95–99, has no ADR or closing condition written yet. The next
iteration takes the first eligible item as `LOOP.md` says. The next unused
queue number is **269** and the next ADR **0021**. This is one iteration,
not a finished queue or roadmap.

**Final gate run.** `scripts/gate.sh` exited 0 on this tree (7 min 3 s). It
ran in the foreground and was read in the same step. The first run failed
only on `cargo fmt`: the new test code was unformatted. `cargo fmt --all`
touched only that file, and the second run was fully green. Formatting is
clean, clippy is silent and all tests pass. The corpus is included, with
`fieldset-patterns` new and every other reference unchanged. Nothing is
stubbed, `unsafe` is forbidden, the licence notices are present, and every
rented crate stays behind its boundary. No verb takes a coordinate, the
stop rule holds, and `CHANGELOG.md` changed with the code. `git diff
--check` passes. The log is in this session's scratchpad and is not
committed. The only later change was this journal entry, which is
documentation.

---

## Iteration 155 — a verified iteration publishes itself

Not a queue item. The owner asked that every finished task reach GitHub.

It was not reaching it. Twenty-four commits — a whole night's work, from the
DOM tree operations through to the fieldset borders — were sitting on this
machine only, because the worker prompt forbids pushing and nothing else was
doing it. They went out by hand before this change, and the backlog is the
argument for the change: an unattended run that only commits locally is a run
nobody can see until somebody thinks to look, and a disk is a single copy.

**The supervisor pushes, not the worker.** The prompt's rule stands as
written. Pushing from one place means there is one place to check that it
happened, and one place where it can be turned off.

**After the independent gate, not after the worker's commit.** The supervisor
already re-runs the whole gate on the finished tree before accepting an
iteration. Publishing sits after that, so what reaches origin is what passed
verification here rather than what a worker believed it had finished.

**A refused push does not stop the run.** The commits are safe locally and
`git push` sends everything outstanding, so the next iteration carries them;
losing a night to one refused connection would be the worse trade. It is said
loudly every time instead, so a remote that has been refusing for hours cannot
read as quiet.

**Three checks, and a real remote to check against.** The fixture now has a
bare repository to push to, which makes the question "did the commit arrive"
rather than "was the command spelled correctly". A verified iteration reaches
origin; `ALO_LOOP_PUSH=0` leaves origin where it was; and a remote pointed at
nothing is reported while the iteration still counts and the tree stays clean.
Run against the supervisor before this change, the first of those fails
exactly where it should: *"origin did not receive the verified iteration"*.

Twenty-one fixture checks, from eighteen. The loop was stopped with a clean
tree and nothing uncommitted; nothing was lost.


---

## Iteration 165 — item 178 built: a rotated picture is drawn rotated

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md` (its conventions and the *Pictures, and things that move*
section), iterations 164 and 155's entries, the queue's sections F, G and H
in full for eligibility, and the code item 178 names: `alo-paint`'s
`render.rs` (`draw_picture`, `draw_coverage`, the clip and transform
stacks), `display.rs`'s `Picture` item, `picture.rs`, `canvas.rs`'s
`blend`, `raster.rs`'s `fill` and its checked casts, `alo-value`'s
`Matrix`, and corpus cases `a-picture` and `turned-and-faded` with
`alo-corpus`'s `case.rs`. Item 178 names no ADR; its feature home is
`docs/features.md`'s pictures lines, where this change adds one. No
`AGENTS.md` exists. No sibling repository was read or modified. The
checkout was clean on entry at `f428c42`.

**Selection.** Iteration 164 recorded every item before 268 as blocked or
`needs design`, and nothing has changed since. After 268, in file order:
94 waits on 76; 95–99 and 104 name no ADR, contract or closing condition,
and 105 (its dependency 53 is done) is three capabilities with no ADR for
its rental and no closing condition — `LOOP.md` step 2, so each is now
marked `needs design` in the queue rather than left to be re-derived. 100
and 101 wait on 88, 102 on 101, 103 on 101. 178 depends on 176, which is
done, and has a closing condition: it is the first eligible item.

**What was built.** `alo-paint`'s new `drawn_picture.rs`, split from
`render.rs` in the change that gave picture drawing a second path — the
renderer walks a list; this is how one kind of item becomes pixels.
- **Upright**: a transform with nothing crossing axes and neither axis
  reversed keeps the old whole-pixel integer path, unchanged.
- **Turned**: anything else — rotation, skew, and a **mirror**, which the
  old path also drew the wrong way round — rasterises the rectangle as a
  transformed shape (an anti-aliased outline, as any edge), and samples
  each covered pixel's centre back through the inverted transform. A
  flattening transform has no inverse and draws nothing.
- Why two paths rather than one: centre sampling picks different source
  pixels from the old corner sampling whenever a picture is scaled, so a
  single path would have moved every scaled picture in `a-picture`. That
  is item 179's question, decided when a page asks it.
`render.rs` lost the function and exposes `place` and `pixel_centre` to
the crate; nothing else in it changed.

**Compliance review.**
- Law 1: nothing legacy. Law 2: the agent surface is unchanged (no
  `agent.txt` moved; the coordinate check passes). Law 3: no stubs, `todo!`
  or `unwrap` outside tests; the two casts carry `#[expect]` with a reason,
  after a range check, exactly as `raster.rs` does — no lint was lowered.
  Law 4: no `unsafe`.
- One file, one responsibility: picture drawing moved out of `render.rs`
  into its own file rather than growing it.
- Layout assertion: nothing is newly positioned or sized — a transform
  changes what is drawn, not the layout. `a-turned-picture/layout.txt`
  pins all three images at 48×48 at (28, 40), (108, 40) and (188, 40), the
  same as unturned.
- Reference render: new corpus case `a-turned-picture` — the 24×24 stripes
  at 48px under `rotate(30deg)`, `rotate(90deg)` and `scaleY(-1)`. I looked
  at it upscaled. The first is a tilted square with a smooth outline,
  stripes sloping down to the right and the upright box's corners white;
  the second has red on the right and the stripes standing up; the third
  has blue on top. The first draft had no doctype, and html5ever recorded
  `line 1: Unexpected token` in `issues.txt`; the case now has one and its
  `issues.txt` is empty. `ALO_UPDATE_REFERENCES=1` changed no other case.
- Bytes from outside: no new parsing. A non-finite transform is tested to
  draw nothing and not fail.

**Tests.** 11 unit tests in `drawn_picture.rs` over a 2×2 picture of four
distinct quarters: upright; a quarter turn, a half turn and a mirror each
put every quarter in the right corner; an eighth of a turn is a diamond
whose points reach past the square while the square's own corners stay
white, with each quarter where the turn puts it; the outline is blended;
an upright 2× scale still fills whole pixels to the canvas edge; a
flattening transform and a NaN or infinite one draw nothing; a picture
turned partly off the page draws the part on it; and `source` keeps a
fraction inside the picture and refuses NaN. Doctored with every transform
sent down the old upright path: 7 of the 11 fail, and so does the corpus
(`a-turned-picture`). Restored afterwards.

**Roadmap.** Item 178 served no `ROADMAP.md` line, and no line moved. The
nearest, *Image codecs, rented*, is about decoding; this is drawing an
already-decoded picture under a transform, which is stage 1's
*Transforms* feature applied to a stage 2 `<img>`. Noticed and left alone,
because it is not this item's line: *Image codecs* has no Built clause
although PNG (106) and JPEG (177) are built — the next iteration that
serves it should add one. Also updated: `docs/features.md` (a new pictures
line), `docs/conformance.md`, `CHANGELOG.md`, `QUEUE.md` (178 ticked with
its evidence; 95–99, 104, 105 marked `needs design`) and `REMAINING.md`.

**Unresolved obligations.**
- **A picture ignores a clip in force.** Found reading `render.rs`: the
  `Picture` branch never receives the clip stack, so an `<img>` inside
  `overflow: hidden` or a rounded clip is drawn whole. Written into
  `conformance.md`. No queue item is opened, because no page fails on it,
  as with iteration 164's rounded fieldset corner.
- Inside a turned or scaled picture, sampling is still nearest-neighbour
  (179, waiting on a page).
- Carried unchanged: 82, 83 and 85–89 need their designs, and now 95–99,
  104 and 105; the iteration 164 carry-overs.

110 queue items are open (178 closed). The next eligible candidates in
file order are 179 (waits on a page, by its own closing condition) and 180
(GIF, WebP, AVIF; depends on 177, done). The next iteration takes the
first eligible item as `LOOP.md` says. The next unused queue number is
**269** and the next ADR **0021**. This is one iteration, not a finished
queue or roadmap.

**Final gate run.** `scripts/gate.sh` exited 0 on this tree (7 min 19 s),
run in the foreground and read in the same step, after `cargo fmt --all`.
Formatting is clean, clippy is silent and all tests pass. The corpus is
included, with `a-turned-picture` new and every other reference
unchanged. Nothing is stubbed, `unsafe` is forbidden, every rented crate
stays behind its boundary, no verb takes a coordinate, the stop rule
holds, and `CHANGELOG.md` changed with the code. `git diff --check`
passes. The log is in this session's scratchpad and is not committed. The
only later change was this journal entry, which is documentation.


---

## Iteration 166 — item 180 built for GIF and WebP; AVIF cut to 269

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md` (its conventions and the *Pictures, and things that move*
section), iteration 165's entry, the queue's section H in full and sections
I–K for eligibility, and the code item 180 names: `alo-paint`'s `picture.rs`,
`encode.rs` (`MOST_PIXELS`, `picture_from_png`), `canvas.rs`, the two-format
test, `alo-renderer`'s picture loading in `pipeline.rs`, `alo-corpus`'s
`case.rs`, and corpus cases `a-picture` and `web-a-form` (for `origin.txt`).
I also read the source of the two rented decoders where they allocate:
`gif` 0.14.2's `reader/mod.rs`, and `image-webp` 0.2.4's `decoder.rs`,
`vp8.rs` and `lossless.rs`. Item 180 names no ADR of its own. It rents under
ADR 0001, decodes untrusted bytes in the renderer under ADR 0005, and is
pure Rust for ADR 0010's reason, as `jpeg-decoder` was. Its feature home is
`docs/features.md`'s pictures lines. No `AGENTS.md` exists. No sibling
repository was read or modified. The checkout was clean on entry at
`3586ebd`.

**Selection.** Nothing before 179 has changed since iteration 165 recorded
it blocked or `needs design`. 179 waits on a page by its own closing
condition. 180 depends on 177, which is done, and has a closing condition, so
it is the first eligible item.

**Scope cut.** AVIF went to new item **269**, marked *needs ADR*. Its pixels
are an AV1 frame. `dav1d` is C, and `rav1d` keeps a great deal of `unsafe`.
I found no AV1 decoder that is pure Rust and free of `unsafe`. That is a
survey for the ADR to redo, not a settled fact. Choosing a decoder is a
decision, so per `LOOP.md` § 4 it gets its own iteration. GIF and WebP are
built whole.

**What was built.**
- **Rented**: `gif` 0.14 (without default features) and `image-webp` 0.2. Both
  are pure Rust and `#![forbid(unsafe_code)]`, as are `weezl` and
  `byteorder-lite`. `quick-error` contains no `unsafe` at all but does not
  declare the forbid. All are MIT or Apache-2.0. `scripts/gate.sh` gained
  two boundaries, and `jpeg_decoder`'s boundary moved.
- **Split for one responsibility**: `picture.rs` would have held three
  decoders. JPEG moved to `jpeg_picture.rs`, and GIF and WebP got
  `gif_picture.rs` and `webp_picture.rs`. `picture.rs` keeps format sniffing,
  dispatch and the one bound, `agreed_size`. PNG's reader now calls that
  bound too, with identical messages, so the bound lives in one place.
- **GIF**: the screen is bounded first. Then the first frame's own rectangle,
  read from its descriptor, is bounded before any pixel is decoded. Bounding
  the screen alone would let a 1×1 GIF carry a 65535² frame. The frame is laid
  on a transparent screen at its offset and cut at the screen's edge. A frame
  of no size gives a clear picture of the screen's size. The decoder's memory
  limit is set to our bound instead of its own 50 MB default.
- **WebP**: before the decoder sees a byte, a walk over the RIFF chunks bounds
  every lossy bitstream's declared size. That covers top-level `VP8 ` chunks
  and the lossy bitstream in every `ANMF` frame. The walk exists because
  `image-webp`'s VP8 decoder reserves luma and chroma planes by the
  bitstream's own header (`vp8.rs:1200`, up to 16383², about 400 MB) and
  only afterwards compares that with the canvas. That was established by
  reading the source. RSS does not show it because zeroed allocations are
  committed lazily. Lossless needs no walk: `lossless.rs:107` compares sizes
  before it reserves anything.
- **Animated pictures** are drawn as their first frame, for both formats.

**A panic in the rented WebP decoder, found and guarded.** I ran a throwaway
mutation search in the scratchpad: random byte changes and truncations through
`picture::read`, with overflow checks on. It found
`index out of bounds` at `image-webp` 0.2.4 `decoder.rs:858`, in 1 of
300 000 inputs. In an animation frame with an `ALPH` chunk, the decoder walks
the lossy picture's size over an alpha plane of the frame's size and never
compares the two. I added that comparison to the walk. A second search, a
million inputs on another seed, found the same line again. The cause this
time: after `ALPH`, the decoder decodes the next chunk as lossy **whatever its
name**, so a walk that looked for `VP8 ` by name let it through. The walk now
follows the decoder's rule: the first chunk if it is `VP8 `, or the second if
the first is `ALPH`.

After both fixes, the search ran six seeds of a million inputs each, with
splices and deletions added and PNG and JPEG included: no panics. 0.2.4 is the
newest release. The two captured inputs were overwritten by later runs and
are not kept. The regression tests rebuild both variants from the frozen
`stripes-moving.webp`, and each panics at `decoder.rs:858` when the check is
doctored out. **Upstream has not been told.** Filing an issue is outward-facing
and is a person's call.

**Compliance review.**
- Law 1: nothing legacy.
- Law 2: agent surface unchanged; the coordinate check passes.
- Law 3: no stubs, `todo!` or `unwrap` outside tests. No `#[expect]` was added
  and no lint was lowered. The gate's stub check caught a test chunk named
  `XXXX`, which I renamed to `JUNK` instead of touching the check.
- Law 4: no `unsafe`, and every new crate is `unsafe`-free.
- One file, one responsibility: the split above.
- Bytes from outside (`LOOP.md` stage 2 § 2): every byte of all nine frozen
  files is flipped, and every prefix is cut, through `read`. A refusal is
  checked for each claimed size, and the WebP walk has unit tests for lengths
  that lie and headers cut short.
- Layout assertion: `a-picture-in-each-format/layout.txt` pins seven images
  at 48×48, at x = 8, 64, 120 … 344 and y = 8, in a 400×64 page. `display.txt`
  says each picture is 24×24, which is the decoded size.
- Reference render: `render.png` is the new corpus case. I looked at it
  upscaled 3×. All seven show red, green and blue top to bottom. Both `-clear`
  pictures show the page's colour where blue was. Both animated ones show the
  first frame, red on top, not the flipped second frame. The lossy WebPs blur
  slightly at the stripe edges. `issues.txt` is empty. `ALO_UPDATE_REFERENCES=1`
  changed no other case.
- Frozen files: made by Pillow 10.4.0 with libwebp 1.4.0 from `a-picture`'s
  stripes. The encoder shares no code with the decoders. The script, the
  reason no page was used, and SHA-256 sums (re-verified) are in `origin.txt`.

**Tests.** 4 unit tests in `picture.rs`: sniffing for both GIF versions,
WebP only when RIFF says `WEBP`, AVIF refused, and the bound's edges and
overflow. 7 in `gif_picture.rs`, over hand-written GIFs: a frame placed by
its rectangle, cut at the screen, of no size, a screen of no size, and a
65535² frame on a 1×1 screen refused *for its size*. 9 in `webp_picture.rs`:
a one-pixel canvas carrying a 16383² bitstream, the same inside an animation
frame, an 83 MP canvas, a frame of another size, the chunk after `ALPH`
whatever its name, the scale bits, short bitstreams, and the chunk walk's
padding, lying lengths and cut headers. The integration test
`pictures_in_two_formats.rs` is now `pictures_in_every_format.rs`, with 12
tests over one list of seven files plus two transparent ones.

Two premises in the inherited tests were corrected rather than bent.
- *A corrupt byte never changes the size* holds only where the size is
  repeated or checksummed. A GIF's screen and an animated WebP's canvas are
  stated once. The every-seventh-byte sampling never hit them; every byte
  does. The test now says what is true: refused, the same size, or a new size
  only when the flipped byte is the size field (`size_field`, per format).
- Two of my own first tests were wrong. 16 000 000 × 4 is under the bound,
  and the `gif` crate refuses a frameless GIF in `read_info`. Both were fixed
  to say what they mean.

Doctored, each test was run with its check removed: without the walk, 2 WebP
unit tests fail. Without the frame comparison, or with a name-only rule, the
regression test panics inside the crate. Each check was restored, and the
restore is in the tree the gate ran on.

**Roadmap.** *Image codecs, rented* gained its first Built/Owed clause, which
iteration 165 noted was missing: PNG (106), JPEG (177), GIF and WebP (180)
built; AVIF (269) and playback (109) owed. It stays an empty box because AVIF
is not built. Also updated: `docs/features.md` (a GIF and WebP line, and the
stage 2 codecs line), `docs/conformance.md`, `CHANGELOG.md`, `Cargo.toml`
(with why each crate), `QUEUE.md` (180 ticked with its evidence, 269 opened)
and `REMAINING.md`.

**Unresolved obligations.**
- The `image-webp` panic should go upstream. A person decides whether and how.
- AVIF (269) needs its ADR as its own iteration.
- An animation does not move (109). Sampling is still nearest-neighbour
  (179). A picture still ignores a clip in force (iteration 165).
- Carried unchanged: 82, 83, 85–89, 95–99, 104 and 105 need their designs.

110 queue items are open (180 closed, 269 opened). In file order, the next
candidates after 180 are 269 (an ADR iteration, eligible since 180 is done),
107 (SVG, which must be cut before starting) and 108 (Canvas 2D, which
depends on 72). The next iteration takes the first eligible item as
`LOOP.md` says. The next unused queue number is **270** and the next ADR is
**0021**. This is one iteration, not a finished queue or roadmap.

**Final gate run.** `scripts/gate.sh` exited 0 on this tree (442 s), run in
the foreground and read in the same step, after `cargo fmt --all`. The run
before it failed only the stub check, on the `XXXX` test chunk. Formatting
is clean, clippy is silent, and all tests pass, corpus included, with
`a-picture-in-each-format` new and every other reference unchanged. Nothing
is stubbed, `unsafe` is forbidden, and all 21 rented crates stay behind their
boundaries, `gif` and `image_webp` among them. No verb takes a coordinate,
the stop rule holds, and `CHANGELOG.md` changed with the code.
`git diff --check` passes. The log is in this session's scratchpad and is not
committed. The only later change was this journal entry, which is
documentation.

## Iteration 167 — item 269's decision: ADR 0021, AVIF waits for a decoder we can call without `unsafe`

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md` (its conventions and the *Pictures, and things that move*
section), iteration 166's entry and those of the earlier ADR iterations (158)
for their form, the queue's section H in full, ADRs 0005, 0009, 0010 and 0015
in full and 0020's head, `docs/features.md`'s picture lines,
`docs/conformance.md`'s picture paragraph, `REMAINING.md`'s tail, and the
code the decision is about: `alo-paint`'s `picture.rs` and the boundary list
in `scripts/gate.sh`. Item 269 names no feature contract beyond
`docs/features.md`'s codec lines. No `AGENTS.md` exists. No sibling
repository was read or modified. The checkout was clean on entry at
`56ba9e7`.

**Selection.** Nothing before 179 has changed since iteration 166 recorded
it. 179 waits on a page by its own closing condition, and 180 is done. 269
depends only on 180 and is marked *needs ADR*. Under `LOOP.md` stage 2 § 4
the ADR is its own iteration, so this iteration is ADR 0021 and no code.

**The survey** (crates.io and each crate's published source, downloaded to
this session's scratchpad and read, never built into the workspace):
- `rav1d` 1.1.0 (May 2025, BSD-2-Clause) is the newest release. Its only
  public interface is dav1d's C ABI (`pub unsafe extern "C" fn dav1d_*`).
  It has 510 `unsafe` occurrences in about 55k lines, and its assembly is
  behind the `asm` features. Its safe Rust API, `src/rust_api.rs`, was merged
  on `main` on 2026-04-04 (#1439), and #1484 made it stop forcing panics on
  2026-05-05. Neither is released, and `main` still says 1.1.0.
- `dav1d`/`dav1d-sys` are bindings to C. `re_rav1d` (Rerun's fork, with a
  safe API) has been archived since October 2024. `rav1d-safe` and `zenavif`
  forbid `unsafe` by default but are AGPL-3.0 or commercial.
  `oxideav-av1` is MIT and has no `unsafe`, but it is six months old with
  about 6k downloads, and its crate docs call it a scaffold whose pipeline
  "is not wired up yet", which disagrees with its README. `gamut-avif` is
  the container only. `avif-parse` 2.1.0 is MPL-2.0 and a fork of Firefox's
  mp4parse. Its only `unsafe` is its feature-gated C API, it refuses grids
  by name, and it exposes the sequence header's maximum frame size.

**What was decided** (ADR 0021):
- § 1: `rav1d` decodes the frame, through its safe Rust API only. Default
  features off except the two bit depths, with no assembly, one thread, no
  frame delay, and `frame_size_limit` from `MOST_PIXELS`. It goes in one
  file, `avif_picture.rs`, with a `gate.sh` boundary added in the same
  commit.
- § 2: the API must come from a crates.io release. A git pin is refused for
  three reasons: no reach, advisories name versions, and there is no git
  dependency anywhere in the workspace. FFI of our own to 1.1.0 is refused
  because it would be the repository's first `unsafe`, written to avoid
  waiting. **So 269 is blocked on a `rav1d` release.**
- § 3: `avif-parse` reads the box, named in the same file. `gamut-avif` is
  the named alternative.
- § 4: `ispe` and the sequence header's maximum size both go through
  `agreed_size`. The alpha item must match the primary item's size, and the
  decoded picture must be the agreed size. Every byte is flipped and every
  prefix cut, as for the other formats.
- § 5: the colour conversion is ours, with matrices 0, 1, 5, 6 and 9. Any
  other matrix is refused by name, and so are PQ and HLG. The upsampling
  filter and the bit-depth rounding are left to the commit that builds them.
  ICC is not applied, as for the other formats.
- § 6: until then an AVIF is refused exactly as today. Any list of formats
  the browser states (an image `Accept`, `<picture>` `type`) is derived from
  what `picture.rs` decodes. Nothing states such a list today, so no code is
  owed for this now.
- § 7: a grid stays refused. A sequence shows its still primary item.

**Decisions a reviewer may want to look at.** Refusing a git pin of `rav1d`
is the call that costs the most: AVIF stays unread for an unknown time.
The ADR's last section leaves re-weighing it to a person, prompted by a
frozen page that needs AVIF. Also look at the refusal of PQ/HLG and of
unlisted matrices, and at the choice of `avif-parse` over `gamut-avif` on
reach.

**Compliance review.**
- Law 1: nothing legacy enters.
- Law 2: the agent surface is unchanged.
- Law 3: no code, so no stubs. 269's closing condition is unchanged and the
  item is not ticked. It is marked blocked, with what lifts the block.
- Law 4: no `unsafe`. The ADR refuses the one route that would have
  introduced it.
- ADRs 0005, 0009, 0010 and 0015 are applied, not amended.
- One file, one responsibility: the only source change is `picture.rs`'s
  module comment, which now cites ADR 0021. That file's responsibility is
  unchanged.
- Layout assertions and reference renders: nothing positions, sizes or draws.
- Bytes from outside: none are read by anything new. § 4 states the hostile
  input tests 269's code owes.
- `docs/features.md`: both codec lines now say AVIF is decided and blocked.
  A decision is not a built feature, so nothing is promoted.

**Gate.** `scripts/gate.sh` exited 0 on this tree (443 s), run in the
foreground and read in the same step. Formatting is clean, clippy is silent
and all tests pass. Nothing is stubbed, `unsafe` is forbidden, every source
file carries Exhibit A, all 21 rented crates stay behind their boundaries,
no verb takes a coordinate, the stop rule holds, and `CHANGELOG.md` changed.
`git diff --check` passes. The log is in this session's scratchpad and is not
committed. The only later change was this journal entry, which is
documentation.

**Roadmap.** The *Image codecs, rented* Owed clause now says AVIF is decided
in ADR 0021 and blocked on a `rav1d` release, and why. It is not ticked.
Also updated: `QUEUE.md` (269's decision and block), `docs/features.md`,
`REMAINING.md`, `CHANGELOG.md`, and `picture.rs`'s module comment.
`docs/conformance.md` is unchanged because what renders has not changed.

**Unresolved obligations.**
- 269's code, once `rav1d` releases its Rust API. Watching for that release
  is a person's job or a later iteration's check. Nothing here polls for it.
- Carried from 166, unchanged: the `image-webp` panic should go upstream (a
  person's call). Also 109, 179, a picture ignoring a clip in force, and
  82, 83, 85–89, 95–99, 104 and 105, which need their designs.

110 queue items are open, none closed or opened. 269 is now blocked. In file
order the next candidates are 107 (SVG, which must be cut before it is
started), then 108 (Canvas 2D, depends on 72) and later items. The next
iteration takes the first eligible item as `LOOP.md` says. The next unused
queue number is **270** and the next ADR is **0022**. This is one
iteration, not a finished queue or roadmap.

## Iteration 168 — item 107 cut: ADR 0022, SVG is a picture we draw and its insides are not boxes

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md` (its conventions and the *Pictures* lines), iterations 166 and
167, the queue's section H in full and its item list for eligibility, ADRs
0001, 0002 and 0021 in full and the heads of 0004 and 0005,
`docs/features.md`'s pictures lines, and the code the decision is about:
`alo-box`'s `role.rs` (an `<svg>` is already an image), `tree.rs` and
`semantics.rs`; `alo-style`'s user-agent sheet (`<svg>` is `inline-block`);
`alo-paint`'s `raster.rs`, `path.rs` and `build.rs` (`PaintContext::pictures`);
`alo-dom`'s `scripts.rs`; the crate dependency lists; and `tiny-skia-path`
0.12's `stroker.rs` and `dash.rs` in the local registry. Item 107 names no
ADR or contract beyond `docs/features.md`'s SVG line. No `AGENTS.md` exists.
`alo-workplace` was **read only**: `web/public/offline.html` and three
`web/public/icons/*.svg`. Nothing in any sibling repository was modified.
The checkout was clean on entry at `37a3580`.

**Selection.** Nothing before 179 has changed since iteration 167 recorded
it. 179 waits on a page by its own closing condition, 180 is done, and 269 is
blocked on a `rav1d` release. 107 is next and eligible, and its own text
says *"Cut this before starting it"*. How SVG is drawn is a decision (who
owns the tree, what is rented, what the agent reads), so under `LOOP.md`
stage 2 § 4 the cut and its decision are this iteration's work, with no
engine code.

**The page that opens it** (stage 2 § 1): alo's own offline screen,
`alo-workplace/web/public/offline.html`. Its only picture is an inline
`<svg>` of the alo hand: four `<path>`s with arcs, `fill="none"`, stroked
`#e76f51` with round caps and joins, in a 24-unit `viewBox` drawn into a
56 px square by its stylesheet. Read from the code, today it is a 56 × 56
`inline-block` box with four empty inline boxes inside it, so the hand is
missing. No case was frozen this iteration. 273 freezes it as the closing
case.

**What was decided** (ADR 0022):
- § 1: an outermost `<svg>` is a replaced box sized through item 176's path.
  CSS sets the size first, then the attributes, then the `viewBox` ratio,
  then 300 × 150. **Nothing inside it is a CSS box.**
- § 2: a new crate `alo-svg` makes a drawing after layout: paths in
  `alo-paint`'s own `Path` vocabulary, with transforms and paint. The
  pipeline hands it to paint by box id beside `PaintContext::pictures`, and
  neither `alo-paint` nor `alo-box` depends on it. Geometry and the path
  grammar are ours. `svgtypes` was the close call, refused on size,
  the error rule and the bounds. Strokes and dashes are rented from
  `tiny-skia`, which is already rented, through `raster.rs` only, so no new
  crate and no boundary change.
- § 3: SVG paint properties go through the one cascade. Presentation
  attributes are author declarations of specificity zero, and
  `currentColor` inherits from the HTML around the `<svg>`.
- § 4: the agent tree gets one image node, named by `aria-label`,
  `aria-labelledby`, then `<title>`. It is absent under `aria-hidden`, which
  covers the offline screen's icon. No name is guessed from shapes.
- § 5: path segments, dashes, nesting and (later) `<use>` expansion are
  bounded before the work happens. A refusal leaves an empty box and never
  panics. The values belong to the building commits.
- § 6: the cut. **270** the box, **271** filled shapes, **272** path data and
  **273** strokes, which closes on the offline screen frozen as an alo case.
  Then **274** `<use>`, **275** gradients, **276** `<text>`, each opened only
  by a page, and **277** an SVG file as a picture, which *needs ADR* because
  a standalone SVG file is XML and XML is stage 3's item 139.
- § 7: refused until a page asks: `clipPath`, `mask`, `filter`, `marker`,
  `<image>`, `<foreignObject>`, SMIL, events inside the drawing. SVG fonts
  are refused for good, and SVG `<script>` stays unrun.

**Decisions a reviewer may want to look at.**
- Refusing `usvg`/`resvg`. It is the cheapest route to the offline icon, and
  it is refused because it owns its own tree and cascade (ADRs 0001 and
  0004).
- Writing our own path parser instead of renting `svgtypes`. The ADR says
  what reopens that.
- Putting `alo-svg` above `alo-paint` so that there is one path type.

**Compliance review.**
- Law 1: nothing legacy enters, and SVG fonts are refused by name.
- Law 2: one tree is kept. The drawing is a box's content, and no verb
  reaches into it or takes a coordinate.
- Law 3: no code, so no stubs. 107 is not ticked. It stays open until
  270–273 close.
- Law 4: no `unsafe`. The one rented capability added later (the stroker)
  is in a crate already behind its boundary.
- ADRs 0001, 0002, 0004 and 0005 are applied, not amended.
- One file, one responsibility: the only source change is a module comment
  in `alo-dom`'s `scripts.rs`. It sent SVG `<script>` to item 107, which is
  now wrong, and now cites ADR 0022 § 7.
- Layout assertions and reference renders: nothing positions, sizes or draws
  in this change. 270–273 each name theirs.
- Bytes from outside: nothing new reads them. § 5 states the hostile-input
  tests the building items owe.

**Gate.** `scripts/gate.sh` exited 0 on this tree (455 s), run in the
foreground and read in the same step. Formatting is clean, clippy is silent
and all tests pass. Nothing is stubbed, `unsafe` is forbidden, every source
file carries Exhibit A, all 21 rented crates stay behind their boundaries,
no verb takes a coordinate, the stop rule holds, and `CHANGELOG.md` changed.
`git diff --check` passes. The log is in this session's scratchpad and is not
committed. The only later change was this journal entry, which is
documentation.

**Roadmap.** The *SVG* line stays an empty box, because nothing is built. It
now says so, names ADR 0022 and items 270–277, and names the offline screen
as the page they close on. Also updated: `QUEUE.md` (107's decision and cut,
and 270–277 opened), `docs/features.md`, `CHANGELOG.md`, `REMAINING.md`, and
`scripts.rs`'s module comment. `docs/conformance.md` is unchanged because
what renders has not changed.

**Unresolved obligations.**
- 270–273 are to be built, in order. 270 is eligible now.
- 277 needs its ADR before any code, once 273 is done.
- Carried from 167, unchanged: 269 is blocked on a `rav1d` release, and the
  `image-webp` panic should go upstream (a person's call). Also 109, 179, a
  picture ignoring a clip in force, and 82, 83, 85–89, 95–99, 104 and 105,
  which need their designs.

118 queue items are open (eight opened, none closed). The first eligible
item in file order is now **270**. The next unused queue number is **278**
and the next ADR is **0023**. This is one iteration, not a finished queue or
roadmap.

## Iteration 169 — item 270: the `<svg>` box

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and *Pictures* lines, iteration 168's entry, the
queue's section H in full, ADR 0022 in full, `docs/features.md`'s pictures
lines, and the code item 176 built: `alo-box`'s `tree.rs`, `semantics.rs`
and `role.rs`, `alo-layout`'s `engine.rs` and `arena.rs`, and
`alo-renderer`'s `pictures_for`. No `AGENTS.md` exists. `alo-workplace` was
**read only** (`web/public/offline.html`, for the `<svg>` markup and its
56 px rule); nothing in any sibling repository was modified. The checkout was
clean on entry at `0594ffe`.

**Selection.** Iteration 168 recorded 270 as the first eligible item, and
nothing before it changed since: 179 waits on a page, 269 on a `rav1d`
release, and 107 is closed by 270–273. 270 depends on 176, which is done.

**What was built.**
- `alo-box`'s new `natural.rs`: `NaturalSize`, a width, a height and a ratio,
  each optional. The box tree's natural size was a `(f32, f32)`, which cannot
  say "a `viewBox` and no size" or "nothing written, still replaced".
- `alo-box`'s new `svg.rs`: an outermost `<svg>` (SVG namespace, parent not
  SVG); its natural size from `width`/`height` (numbers, or absolute units)
  and `viewBox`; its first child `<title>`. Per cent, `em` and `calc()`
  widths are recorded as issues and not used (271's presentation attributes).
- `tree.rs`: an outermost `<svg>` makes one box with **no children** whatever
  its `display`, and `display: contents` on it makes nothing, as on any
  replaced element.
- `semantics.rs`: an `<svg>` is named by `aria-labelledby`, then
  `aria-label`, then its `<title>`. The queue line and ADR 0022 § 4 list
  `aria-label` before `aria-labelledby`; the existing code (and ARIA) put
  `aria-labelledby` first, and that order was kept for every element rather
  than giving `<svg>` a different one. A reviewer may want the ADR's wording
  corrected.
- `alo-layout`'s new `replaced.rs`: CSS 2's replaced-element sizing in
  numbers, used by the leaf measure in `arena.rs` (taffy stays in its two
  files). The 300 × 150 default, and a ratio-only box filling its room.
- `engine.rs`: a replaced box is atomic on a line whatever its `display` says
  (`.ratio { display: inline }` in the case).

**Gate, mechanical.** `scripts/gate.sh` exited 0, run in the foreground and
read in the same step: fmt clean, clippy silent, every test passes, no stubs,
no `unsafe`, every rented crate behind its boundary, no verb takes a
coordinate, the stop rule holds, `CHANGELOG.md` changed. `git diff --check`
passes. The log is in this session's scratchpad, not committed. The only
later change was this entry.

**Gate, manual.**
- Layout assertion in numbers: corpus case `an-svg-box`'s `layout.txt` —
  the offline screen's own `<svg>` and rule at 56 × 56 with no child boxes;
  48 × 24 from attributes; 80 × 40 from `width=80` and a 2:1 `viewBox`;
  a `viewBox`-only `<svg>` made inline filling its paragraph at 320 × 80; and
  300 × 150 with nothing written. `replaced.rs` has eight unit tests in
  numbers; `tree.rs` five on boxes and natural sizes.
- Reference render: `an-svg-box/render.png`, each `<svg>` given a background
  so its box is visible. Looked at; matches the numbers. No other reference
  in the corpus moved.
- Agent tree: `agent.txt` names one image by `<title>`, one by
  `aria-label`, one by `aria-labelledby`, one unnamed, and the
  `aria-hidden` one is absent. `semantics.rs` has a unit test for the order.
- Hostile bytes: `svg.rs` refuses non-finite, overflowing, negative and
  malformed `viewBox` and size values, and a `viewBox` of a million numbers
  stops after five, with tests; none panic.
- One responsibility per file: sizing rule (`replaced.rs`), what an `<svg>`
  is (`svg.rs`), what content claims (`natural.rs`) are each their own file.
- `docs/features.md`'s SVG line names the item.

**Roadmap.** The *SVG* line stays an empty box and gains a `Built:` clause
(the box, `svg.rs`, `replaced.rs`, `an-svg-box`) and an `Owed:` clause
(271–273, and relative `width` attributes). Also updated: `QUEUE.md` (270
ticked with what was done), `docs/features.md`, `docs/conformance.md`,
`CHANGELOG.md`, `REMAINING.md`.

**Unresolved obligations.**
- 271–273 are to be built, in order; 271 is eligible now. 107 stays open.
- A per cent, `em` or `calc()` `width`/`height` attribute on an `<svg>` is
  recorded and ignored until 271 puts presentation attributes in the cascade.
- ADR 0022 § 4's name order wording (above) is a person's call.
- Carried, unchanged: 269 blocked on a `rav1d` release; the `image-webp`
  panic upstream report (a person's call); 109, 179, a picture ignoring a
  clip in force, and 82, 83, 85–89, 95–99, 104, 105 needing their designs.

117 queue items are open (one closed). The first eligible item in file order
is now **271**. The next unused queue number is **278** and the next ADR is
**0023**. This is one iteration, not a finished queue or roadmap.

## Iteration 170 — item 271: shapes, filled

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s conventions and *SVG* line, iteration 169's entry, the queue's
section H (107, 270–277), ADR 0022 in full, `docs/features.md`'s pictures
lines, and the code 271 touches: `alo-paint`'s `path.rs`, `raster.rs`,
`display.rs`, `build.rs` and `render.rs`; `alo-style`'s `cascade.rs`,
`computed.rs`, `inheritance.rs`, `origin.rs` and `user_agent.rs`; `alo-box`'s
`svg.rs`; and `alo-renderer`'s `pipeline.rs`. No `AGENTS.md` exists. No
sibling repository was read or modified this iteration. The checkout was clean
on entry at `b103193`.

**Selection.** Iteration 169 recorded 271 as the first eligible item, and
nothing before it changed: 179 waits on a page, 269 on a `rav1d` release, and
107 is closed by 270–273. 271 depends on 270, which is done.

**What was built.**
- A new crate, `alo-svg` (ADR 0022 § 2): `walk.rs` turns an outermost `<svg>`
  into an `alo_paint::Drawing` after layout, in document order, with its own
  stack; `shape.rs` gives SVG 2's equivalent path for `rect` (with the
  `rx`/`ry` rules), `circle`, `ellipse`, `polygon` and `polyline`, and a line
  fills nothing; `viewport.rs` is `viewBox` with `preserveAspectRatio` as one
  matrix; `transform.rs` is the `transform` attribute's own grammar, ignored
  whole on any error; `fill.rs` reads `fill`, `fill-opacity` and `fill-rule`
  from the computed style; `length.rs` and `number.rs` are user units and
  SVG's number lists; `bounds.rs` holds every bound and its reason.
- `alo-paint`: `drawing.rs` (the vocabulary handed over by box),
  `fill_rule.rs`, a `rule` on `DisplayItem::Fill` (the outline says `evenodd`
  only where it is), `PaintContext::drawings`, `build.rs`'s `drawing_of`
  (into the content box, clipped there unless `overflow` is `visible`), and
  `raster::fill_on_page`, which makes only the coverage that lands on the
  page. Without it a stranger's `<rect width="60000" height="60000">` would
  have asked for a 3.6 GB mask: a test of the old `fill` did exactly that on
  the first run, which is how it was found.
- `alo-style`: `presentation.rs` makes `fill`, `fill-opacity`, `fill-rule`,
  `opacity`, `display`, `visibility` and `color` attributes on SVG elements
  author declarations of specificity zero, counted before every sheet
  (`Applicable::gather_with_hints`); invalid ones are ignored and recorded.
  The three `fill` properties inherit. The user-agent sheet gains
  `svg { overflow: hidden }`, as SVG's own sheet has.
- A box with no children no longer pushes an empty clip in `build.rs`
  (every `<svg>` would otherwise have had one). No case other than the two
  SVG ones moved.
- `alo-renderer`'s new `drawings.rs` asks `alo-svg` for each `<svg>` box's
  drawing after layout and passes the drawings to paint.

**Cuts, each written into the queue.** A nested `<svg>` viewport is item 278
(opened by a page, as 274–277 are). The `transform` *property* on SVG
elements, and `width`/`height` attributes in per cent or `em` on an outermost
`<svg>`, are item 279; `alo-box`'s messages now name 279 instead of 271.
Both are recorded as issues when a page uses them.

**Gate, mechanical.** `scripts/gate.sh` exited 0 in 7 min 28 s, run in the
foreground and read in the same step: fmt clean, clippy silent, every test
passes, no stubs, no `unsafe`, every rented crate behind its boundary (none
added; `tiny-skia`'s even-odd rule is named only in `raster.rs`), no verb
takes a coordinate, the stop rule holds, `CHANGELOG.md` changed.
`git diff --check` passes. The log is in this session's scratchpad, not
committed. Only this entry changed after the gate ran.

**Gate, manual.**
- Reference render: new corpus case `svg-shapes-filled`, looked at. It shows
  every shape filled, a star under `nonzero` (filled) and `evenodd` (a hole),
  a faded group whose overlap is no darker against two `fill-opacity` squares
  whose overlap is darker, a square turned about its middle, a stylesheet
  beating `fill="red"`, two `currentColor` icons in their paragraphs'
  colours, and `meet`, `xMinYMid`, `slice` (cut at its box) and `none`.
  `an-svg-box` moved, and should have: the `<rect>` in its attribute-sized
  `<svg>` is now black (1152 pixels, 48 × 24), and its four `<path>`s are
  recorded for 272. No other reference moved.
- Numbers: `svg-shapes-filled/display.txt` pins every fill's bounds (for
  example the rect at (8, 12) 32 × 24 from (2, 4) 16 × 12 in a doubled
  viewBox, the turned square at 28.28 × 28.28, the slice at 72 × 72 cut to
  72 × 32). Unit tests pin shape segments, arc accuracy (within 0.03 of a
  100-unit radius), each transform function, list order, and each
  aspect-ratio case. Nothing new is laid out, so `layout.txt` and `boxes.txt`
  of `an-svg-box` did not change.
- Hostile bytes: `number.rs` refuses `inf`, `NaN`, overflowing exponents and
  10 000-digit numbers; `transform.rs` refuses a million-argument function
  after seven; each bound in `bounds.rs` (65 536 points, 262 144 segments,
  65 536 elements, depth 256, 16 groups open, 64 groups) is tested at its
  edge and one past it, and past it the drawing is refused whole with the
  reason recorded. A test of non-finite and enormous values never panics.
- One responsibility per file: `alo-svg` is eight files with one job each;
  the presentation rule is its own file in `alo-style`; `drawing.rs` and
  `fill_rule.rs` are vocabulary in `alo-paint`.
- `docs/features.md`'s SVG line names 271, 278 and 279.

**For a reviewer.**
- ADR 0022 § 2 lists `alo-svg`'s dependencies as `alo-dom`, `alo-style`,
  `alo-value` and `alo-paint`. It also depends on `alo-box`, for
  `svg::is_outermost` and `svg::view_box`, so the box and the drawing read a
  `viewBox` one way. `alo-paint` already depends on `alo-box`, so no new edge
  appears between crates outside `alo-svg`. A person may want the ADR's list
  corrected.
- A shape's own `opacity` is folded into its fill's alpha rather than opened
  as a group: with one fill that is the same picture and costs no page of
  pixels. Item 273 must make it a group once a shape has a stroke too, and
  `walk.rs` says so where it happens.

**Roadmap.** The *SVG* line stays an empty box; its `Built:` clause gains
filled shapes (271, `alo-svg`, `presentation.rs`, `svg-shapes-filled`) and its
`Owed:` clause is now 272, 273, 278 and 279. Also updated: `QUEUE.md` (271
ticked with what was done; 278 and 279 opened), `docs/features.md`,
`docs/conformance.md`, `CHANGELOG.md`, `REMAINING.md`, and `alo-box`'s
`svg.rs` messages.

**Unresolved obligations.**
- 272 and 273 are to be built in order; 272 is eligible now. 107 stays open.
- 278 waits on a page; 279 is eligible after 272 in file order.
- ADR 0022 § 2's dependency list and § 4's name order are a person's call.
- Carried, unchanged: 269 blocked on a `rav1d` release; the `image-webp`
  panic upstream report (a person's call); 109, 179, a picture ignoring a
  clip in force, and 82, 83, 85–89, 95–99, 104, 105 needing their designs.

118 queue items are open (one closed, two opened). The first eligible item in
file order is now **272**. The next unused queue number is **280** and the
next ADR is **0023**. This is one iteration, not a finished queue or roadmap.

## Iteration 171 — item 272: path data

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s *SVG* line, iteration 170's entry, the queue's section H
(107, 270–279), ADR 0022 in full, `docs/features.md`'s SVG line, and the
code 272 touches: `alo-svg`'s `walk.rs`, `shape.rs`, `number.rs`,
`bounds.rs` and its `walking.rs` tests; `alo-paint`'s `path.rs`; and
`alo-renderer`'s `drawings.rs`. No `AGENTS.md` exists. No sibling repository
was read or modified. The checkout was clean on entry at `2e176ce`.

**Selection.** Iteration 170 recorded 272 as the first eligible item, and
nothing before it changed: 179 waits on a page, 269 on a `rav1d` release, 107
is closed by 270–273, and 271, 272's one dependency, is done.

**What was built.**
- `alo-svg`'s new `path_data.rs`: SVG 2's `d` grammar, every command in both
  cases, pairs after a move as lines, `S`/`T` reflecting only a curve of
  their own kind, packed arc flags, and a new subpath at the same start for
  anything after `Z` (written into the path as an explicit move). Drawn up to
  the last complete command before the first error, whose byte is recorded;
  data not beginning with a move draws nothing; a half-written set is not
  drawn.
- `alo-svg`'s new `arc.rs`: SVG 2's implementation notes F.6.2 and F.6.5 in
  `f64` — zero-length arc omitted, zero radius a line, negative radius its
  size, small radii scaled up keeping their ratio, at most four cubics with
  `4/3 · tan(θ/4)` handles, the last ending exactly at the written endpoint.
  An arc made aside, so one refused part way leaves nothing in the path.
- `bounds::MOST_PATH_SEGMENTS` (65 536, an arc counted as every curve it
  makes, closes counted), checked as the path is made, under the drawing's
  existing 262 144; past either the drawing is refused whole.
- `number.rs`'s scanner became the shared `scan`, so `points`, `transform`
  and `d` read numbers one way.
- `shape.rs` gives `<path>` its data (`d` absent, `none`, or only moves and
  closes fills nothing); `walk.rs` draws `<path>` as a shape.

**Gate, mechanical.** The first `scripts/gate.sh` run failed one test:
`alo-renderer`'s `drawings::each_svg_box_draws_into_its_content_box` counted
the "path data is item 272" issue that no longer exists. Its second `<svg>`
now holds `<path d="M0 0 oops">`, which draws nothing and records its error,
so the test asserts what it always meant. The second run exited 0 in
7 min 20 s, run in the foreground and read in the same step: fmt clean,
clippy silent, every test passes, no stubs, no `unsafe`, every rented crate
behind its boundary (none added), no verb takes a coordinate, the stop rule
holds, `CHANGELOG.md` changed. `git diff --check` passes. The log is in this
session's scratchpad. Only this entry changed after the gate ran.

**Gate, manual.**
- Reference render: new corpus case `svg-path-data`, looked at enlarged. A
  house absolute and relative (identical), a `C`/`S` wave whose second hump
  is the first reflected, a `q`/`t` wave, a heart of packed-flag arcs, an
  `evenodd` ring of four arcs, the four flag pairs between the same two
  points (small below, small above, large below, large above), a half disc
  from radii 1 scaled to 8 over a zero-radius line, a 9 × 4 ellipse turned
  45°, a path cut at `oops` drawn as a triangle, and a relative line after
  `Z` starting at the subpath's start. `an-svg-box` changed only in
  `issues.txt`: its four "item 272" lines are gone; the hand's paths are
  `fill="none"`, so its pixels did not move. No other reference moved.
- Numbers: `svg-path-data/display.txt` pins each fill's bounds (e.g. the
  small arcs' sagitta at 5.367 px = 6 − √11 user units doubled). Unit tests
  pin every command's segments, a quarter circle's handles at 5.5228, the
  four flag combinations' midpoints and piece counts, radius correction to a
  half circle and a 10 × 5 half ellipse, rotation, every midpoint of a
  three-quarter arc within 0.003 of the radius, and the after-`Z` move.
  Nothing is laid out, so no `layout.txt` changed.
- Hostile bytes: overflowing exponents, `inf`, `NaN`, a relative sum past
  `f32`, a reflected handle past `f32`, an arc reaching past `f32`, a
  100 000-digit number, 10 000 `M`s, control characters and non-ASCII digits
  are cut at the error or refused, never a panic; extreme radii, rotations
  and endpoints across every flag pair never produce a non-finite point. The
  per-path bound is tested at its edge and one past it (lines, arcs, closes)
  and through the walk, and four paths at their bound fill the drawing's
  bound while five refuse it.
- One responsibility per file: grammar (`path_data.rs`) and arc geometry
  (`arc.rs`) are separate files; `shape.rs` only maps `<path>` to its data.
- `docs/features.md`'s SVG line says path data is drawn.

**Roadmap.** The *SVG* line stays an empty box; its `Built:` clause gains
path data (272, `path_data.rs`, `arc.rs`, `svg-path-data`) and its `Owed:`
clause is now 273, 278 and 279. Also updated: `QUEUE.md` (272 ticked with
what was done), `docs/features.md`, `docs/conformance.md`, `CHANGELOG.md`,
`REMAINING.md`.

**Unresolved obligations.**
- 273 (strokes, closing on the offline screen) is eligible now; 107 stays
  open until it closes. A zero-length subpath under a round cap draws a dot
  in other browsers; `shape.rs` returns no path for data with only moves and
  closes, and 273 must revisit that when strokes exist.
- ADR 0022 § 5 says "a bound on segments per path, and on segments per
  drawing"; both now exist. ADR 0022 § 2's dependency list and § 4's name
  order remain a person's call, as iteration 170 recorded.
- Carried, unchanged: 269 blocked on a `rav1d` release; the `image-webp`
  panic upstream report (a person's call); 109, 179, a picture ignoring a
  clip in force, and 82, 83, 85–89, 95–99, 104, 105 needing their designs.

117 queue items are open (one closed). The first eligible item in file order
is now **273**. The next unused queue number is **280** and the next ADR is
**0023**. This is one iteration, not a finished queue or roadmap.

## Iteration 172 — item 273: strokes, and alo's offline screen

**Read before choosing.** `CLAUDE.md`, the whole of `docs/autonomy/LOOP.md`,
`ROADMAP.md`'s header and *SVG* line, iteration 171's entry, the queue's
section H (107, 270–279) and section F, ADR 0022 in full, `docs/features.md`'s
SVG line, and the code 273 touches: `alo-svg` (`lib.rs`, `walk.rs`, `fill.rs`,
`shape.rs`, `bounds.rs`, `length.rs`, its `walking.rs` tests), `alo-paint`'s
`drawing.rs`, `raster.rs`, `fill_rule.rs`, `build.rs`'s `drawing_of`,
`display.rs` and `render.rs`; `alo-style`'s `presentation.rs` and
`inheritance.rs`; `alo-corpus`'s `case.rs` and `rendering.rs`; and
`tiny-skia-path` 0.12's stroker, dasher and path builder. No `AGENTS.md`
exists. `alo-workplace/web/public/offline.html` was read and copied, never
written; that checkout's `git status` is empty after the iteration. The
checkout here was clean on entry at `108a4fd`.

**Selection.** Iteration 171 recorded 273 as the first eligible item, and
nothing before it changed: 43 waits on 81, 179 on a page, 269 on a `rav1d`
release; 272, 273's one dependency, is done.

**What was built.**
- `alo-paint`: `stroke.rs`, a stroke's vocabulary (`Stroke`, `LineCap`,
  `LineJoin`, `Dashes`, SVG's initial values as `Default`); `raster.rs`'s
  `outline`, `tiny-skia`'s dasher then stroker turning a path into the
  outline it covers, refusing a width or pattern that is not one and any
  non-finite point. A stroke reaches paint as **the fill of its outline**:
  `DrawingItem`, the display list and `render.rs` are unchanged.
- `alo-svg`: `paint.rs` (`<paint>`, lifted from `fill.rs`, shared by `fill`
  and `stroke`); `stroke.rs` (every `stroke-*` property from the computed
  style, in user units, a value in error its initial value and recorded);
  `dashes.rs` (`stroke-dasharray` as SVG 2 reads it, and `count`, an upper
  bound on the dashes a pattern lays, measured along control points). The
  walk fills then strokes, outlines in user space and transforms the outline
  (a stroke under `scale(2 1)` is squashed with it), hands the stroker the
  transform's larger axis as its resolution, groups a fill and a stroke under
  a shape's `opacity` (271's promise), and records `paint-order` and
  `vector-effect`. `<line>` is a path, never filled; path data of moves and
  closes is kept, so `M5 5Z` under a round cap is a dot.
- Bounds (`bounds.rs`): `MOST_DASH_LENGTHS` 256 (read no further), and
  `MOST_DASHES` 16 384 per path, counted before dashing; every outline's
  segments count toward the drawing's 262 144. Past any, the drawing is
  refused whole.
- `alo-style`: the eight stroke presentation attributes, each held to its
  grammar (`miter-clip` and `arcs` refused, as no browser draws them), and all
  eight inherit.

**Gate, mechanical.** The first `scripts/gate.sh` run failed on clippy alone:
a negated float comparison in `walk.rs` and six pedantic `float_cmp` /
`cast_precision_loss` findings in new tests (`cargo test` had not built with
those lints). Each was fixed in the code, none silenced. The second run exited
0 in 7 min 30 s, run in the foreground and read in the same step: fmt clean,
clippy silent, every test passes (the corpus among them, so every committed
reference reproduces), no stubs, no `unsafe`, every rented crate behind its
boundary (`tiny_skia` named only in `alo-paint/src/raster.rs`; none added),
no verb takes a coordinate, the stop rule holds, `CHANGELOG.md` changed.
`git diff --check` passes. The logs are in this session's scratchpad. Only
this entry changed after the gate ran.

**Gate, manual.**
- Reference render: new case `svg-strokes`, looked at enlarged — the three
  caps and three joins with guide lines, a spike beveled at the default miter
  limit and mitered at ten (17.8 units against 12.2, as 1/sin 9.9° says), a
  zigzag, dots from a zero-length `<line>` and `M Z`, a fill under its stroke,
  a squashed circle, even, odd and offset dashes, a dotted circle, dashes
  restarting per subpath, a dashed curve, `currentColor`, a stylesheet beating
  `stroke="red"`, `stroke-opacity`, `opacity` as one group beside the same
  faded alone, and a per-cent width. The first version of the miter pair was
  wrong (a 2.9-width miter, under both limits, so the pair was identical); its
  geometry was corrected and re-rendered before commit.
- **The offline screen**, new case `alo-offline`: the page frozen byte for
  byte (SHA-256 and source commit in its `origin.txt`), its reference render,
  box tree, layout, display list and agent tree committed. The hand is four
  terracotta outlines in its 56 × 56 box and, enlarged, is the shape its
  source draws. Its script runs (it is loaded by a renderer) and its issues
  list is empty. `an-svg-box`'s hand now draws too; its box was terracotta on
  terracotta, so that case's `.hand` now has the offline canvas colour and its
  render moved, as it should. No other reference moved.
- Numbers: unit tests pin caps, joins, the miter limit, dashes and offsets,
  zero-length dots, every refusal, and the dash count. Walk tests pin a stroke
  ring at ±1 unit, a square-capped line, a 4 px stroke from a 24-unit
  viewBox, the squash, the group, the dot, and the offline hand's first
  finger to within 0.05 px of (13, 3)–(19, 12) units × 56/24.
- Hostile bytes: a 3e38 width, coordinates past a float, a 1e-38 width,
  enormous miter limits and offsets, 1e-45 dashes, zero and enormous
  transforms, repeated `M Z` under square caps — never a panic; `outline`
  never returns a non-finite point. Each bound is tested at its edge and one
  past it (256 and 257 lengths, 16 384 and 16 385 dashes, a million-entry
  list read no further than the bound, four paths at their bound refused
  once stroked), and a 0.00001 dash along 100 units is refused.
- One responsibility per file: grammar (`dashes.rs`), properties
  (`stroke.rs`), the shared paint value (`paint.rs`), vocabulary
  (`alo-paint`'s `stroke.rs`) and the rented stroker (`raster.rs`, whose
  responsibility stays "the one file that names `tiny-skia`").
- `docs/features.md`'s SVG line says strokes are drawn.

**What the offline screen found.** Three layout faults that are not strokes:
`text-align: center` does not move an atomic inline (the `<svg>` and the
button sit at x = 24, not centred); an atomic inline's margins are not in its
line's height (the heading starts under the hand, its 20 px margin lost); and
`place-items` is an unexpanded shorthand (`main` is stretched to 352 px rather
than centred). One item per iteration, so they are queued as **280, 281 and
282** in section F, each closing on `alo-offline`'s layout assertion, and
`docs/conformance.md` gives the offline screen a row that says **nearly**. The
committed reference pins today's render, faults included, so each of those
items will move it and say so.

**For a reviewer.**
- ADR 0022 § 2 says `alo-svg` uses `alo-paint` "only for paint's vocabulary
  of shape". It now also calls `alo_paint::raster::outline`, because § 2 also
  says strokes go through the stroker named in `raster.rs` and nowhere else,
  and stroking must happen in user space, before the transform, which only
  `alo-svg` holds. No crate edge was added. A person may want § 2's wording
  corrected along with iteration 170's dependency-list note.
- ADR 0022's "how we will know if this was wrong" compares the hand with
  Firefox and Chromium. Neither was run here; the hand was compared by eye
  with its source shape and pinned in numbers. A side-by-side with a real
  browser is still a person's check.

**Roadmap.** The *SVG* line stays an empty box. Its `Built:` clause gains
strokes (273, `stroke.rs`, `dashes.rs`, `raster::outline`, `svg-strokes`,
`alo-offline`); its `Owed:` clause is now 279, and 274–278 each when a page
needs it. Queue: 273 ticked with what was done; **107 ticked**, because its
own text closes it when 270–273 have; 280–282 opened. Also updated:
`docs/features.md`, `docs/conformance.md`, `CHANGELOG.md`, `REMAINING.md`.

**Unresolved obligations.**
- 280–282 (the offline screen's layout) are eligible now; 279 too.
- ADR 0022 § 2's wording (above, and iteration 170's note) and § 4's name
  order remain a person's call; so does a browser side-by-side of the hand.
- Carried, unchanged: 269 blocked on a `rav1d` release; the `image-webp`
  panic upstream report (a person's call); 109, 179, a picture ignoring a
  clip in force, and 82, 83, 85–89, 95–99, 104, 105 needing their designs.

118 queue items are open (two closed, three opened). The first eligible item
in file order is now **280**. The next unused queue number is **283** and the
next ADR is **0023**. This is one iteration, not a finished queue or roadmap.

## Iteration 173 — item 280: `text-align` moves an atomic inline

**Contracts read.** `CLAUDE.md`, `docs/autonomy/LOOP.md`, `ROADMAP.md` (its
three states and the stage 2 lines), iteration 172's entry, queue item 280,
`docs/features.md`'s layout section and `docs/conformance.md`. No
`AGENTS.md` exists in this repository. ADR 0004 (we own the layout tree;
`taffy` owns the algorithms) applies and is unchanged: the fix is in our
inline layout, `taffy` is not touched. No ADR is needed; nothing is decided.

**Selection.** Iteration 172 recorded 280 as the first eligible item, and
nothing before it changed: 43 waits on 81, 179 on a page, 269 on a `rav1d`
release, and the needs-design items still need designs. 280 depends on
nothing.

**What was built.** The line builder (`inline.rs`) already moved every
fragment of a line by its alignment, atomic boxes included. The fault was
which alignment it was given: both of the offline screen's lines are
anonymous blocks (the `<svg>` and the button each sit beside block-level
siblings), and `engine.rs`'s `alignment_of` read `text-align` only from a
box's own element, so a box nobody wrote always got `start`. It now asks
`BoxTree::nearest_style`, because an anonymous box inherits from its parent.
A control's internal box still answers from its `Purpose`: a button's label
is centred and a field's sits at the start, whatever the page says. That is
today's behaviour, now stated outright, where before it fell through by
accident.

**Gate, mechanical.** `scripts/gate.sh` exited 0, read in the same step:
fmt clean, clippy silent, every test passes (the corpus included, so every
committed reference reproduces), no stubs, no `unsafe`, every rented crate
behind its boundary, no verb takes a coordinate, the stop rule holds,
`CHANGELOG.md` changed. `git diff --check` passes.
*Process note:* the run took longer than the 600 s foreground limit, so the
tool moved it to the background. This iteration then waited for it in the
foreground (a blocking wait on the process) and read the result, exit 0,
before writing this entry or committing. Only this entry changed after the
gate ran. The log is in this session's scratchpad.

**Gate, manual.**
- Layout assertions in numbers (`alo-layout/tests/numbers.rs`, 2 new): an
  inline-block alone in an anonymous line of a 200 px container is at 0, 0,
  80, 160 and 160 for `start`, `left`, `center`, `end` and `right`; "ab" (16
  px) and a 40 px box move as one line, the box at 16, 88 and 160.
  `inline.rs` adds 1 unit test: an atomic box alone and after text under all
  three alignments. Doctored: the old `alignment_of` fails both
  `numbers.rs` tests.
- Reference render: `alo-offline` moved, and no other case did. The hand is
  now at (204, 24) and the button at (173.54688, 195.48438), the exact
  numbers the queue item named. Its layout, display list, agent tree and
  picture are re-committed. I looked at the new render: hand and button
  centred across; the hand still touches the heading (281) and the screen is
  not centred down the page (282), as expected.
- One responsibility per file: the change stays in `engine.rs`'s question of
  which alignment a context has; no file gained a second reason to change.
- `docs/features.md` gains a `text-align` line; `docs/conformance.md` says
  what aligns, that `justify` is read as `start`, and that right-to-left
  `start`/`end` wait on 98; its offline-screen row now names only 281 and
  282.

**Roadmap.** This item served **no open roadmap line**, and `ROADMAP.md` is
deliberately unchanged. It corrects inline layout, which sits under stage
1's ticked *Layout* line. alo's offline screen found the fault, but no stage
2 line covers it: the *SVG* line's Owed clause never listed it, and *CSS
beyond what alo needed* is new capabilities, not a fix to existing ones.
Ticking or annotating a line to discharge this would be the erosion
`LOOP.md` warns against. Queue: 280 ticked with what was done. Also updated:
`CHANGELOG.md`, `docs/features.md`, `docs/conformance.md`, `REMAINING.md`.

**Unresolved obligations.**
- 281 and 282 (the offline screen's vertical layout) are eligible; 281 is
  first in file order. 279 is eligible too.
- Carried, unchanged: ADR 0022 § 2's wording and § 4's name order, and a
  side-by-side of the hand with a real browser (a person's call); 269 blocked
  on a `rav1d` release; the `image-webp` upstream report; 109, 179, and 82,
  83, 85–89, 95–99, 104 and 105 still needing their designs.

117 queue items are open. The first eligible item in file order is now
**281**. The next unused queue number is **283** and the next ADR is
**0023**. This is one iteration, not a finished queue or roadmap.

## Iteration 174 — item 281: an atomic inline's margins count in its line

**Contracts read.** `CLAUDE.md`, `docs/autonomy/LOOP.md`, `ROADMAP.md` (its
three states and the stage 2 lines), iteration 173's entry, queue items
280–282, `docs/features.md`'s layout section and `docs/conformance.md`. No
`AGENTS.md` exists in this repository. ADR 0004 (we own the layout tree;
`taffy` owns the algorithms) applies and is unchanged: the fix is in our line
builder and in how our engine calls `taffy`, whose algorithms are untouched.
No ADR is needed; nothing is decided.

**Selection.** Iteration 173 recorded 281 as the first eligible item in file
order, and nothing before it changed since. It depends on nothing.

**What was built.** `InlineItem::Atomic` carries the box's margins, and its
baseline is measured from the top of its **margin box** — for a box with no
line in it, the bottom margin edge, which is CSS's rule. The line builder
fits, advances the pen by, aligns and stands on the baseline the margin box;
the fragment it records is still the border box, inside its top and left
margins, because that is what draws. The engine reads the margins from the
box's own layout. It also lays the box out the second time in its margin
box's room: `taffy` takes a block root's margins out of the room it is
given, so handing it only its border box shrank an auto-width inline-block
by its own margins and wrapped its text — a fault that only showed once
margins counted at all. The `Atomic.baseline` doc comment, which described
a measure from the bottom edge the code never used, now says what the code
does.

**What was cut.** The item's text also asks for the **strut's** descent
under the hand. The engine has no strut — a line is as tall as what is on
it — and adding one changes every line, not atomic boxes. Cut, not
deepened: it is **283**, eligible, closing on the same `alo-offline`
assertion. So the offline screen's heading is at 100 (24 + 56 + 20), not
yet at 100 plus the font's descent. 281 is ticked for the margin box, with
the cut written into its Built clause.

**Found, not built.** Probing the second pass showed a pre-existing fault:
a percentage width on an inline-block is resolved against its containing
block the first time and against its slot the second, so `width: 50%` in
400 px reserves 200 on the line and draws 100 (112 with 12 px side margins,
which this change moves from 100 — both wrong). Queued as **284**, *blocked:
no page yet*, since nothing in alo writes it.

**Gate, mechanical.** `scripts/gate.sh` exited 0 (fmt clean, clippy silent,
every test passes — the corpus included — no stubs, no `unsafe`, licence
notices, rented crates behind their boundaries, no coordinate verbs, the stop
rule, `CHANGELOG.md` changed). `git diff --check` passes. *Process note:* the
first run failed clippy on two lints in this iteration's own tests
(`cloned_ref_to_slice_refs`, `similar_names`); both were fixed in the code,
not allowed, that run was stopped as stale, and the gate was run again from
the start. That run outlasted the tool's 600 s foreground limit, so this
iteration blocked on its log until it wrote its exit status and read it —
`GATE EXIT 0`, "The gate is met." — before writing this entry. Only this
entry changed after the gate ran.

**Gate, manual.**
- Layout assertions in numbers (`numbers.rs`, 3 new): an inline-block with
  margins `6px 0 20px 12px` at (12, 6) 40×20 and the next block at 46; "ab
  cd" in an inline-block with 12 px side margins 40×16 at 12, one line; a
  20 px left margin centred in 200 puts the box at 90. `inline.rs` 2 new: a
  40×20 box with margins 6/10/8/4 between "ab" and "c" at (20, 6), the line
  38 tall on a baseline of 34, "c" at 70; alone, its 54-wide margin box
  aligned to 4, 77 and 150; and a box that fits only without its margins
  wraps. Doctored three ways: no margins on the item fails all three
  `numbers.rs` tests; the builder ignoring them fails both `inline.rs`
  tests; the second pass in the border box fails the width test (it first
  did not — "abcd" cannot wrap — so the test now uses "ab cd", and fails).
- Reference render: `alo-offline` moved, no other case did. Its anonymous
  line is 416×76, the heading at (24, 100) and everything under it 20 px
  down; layout, display list, agent tree and picture re-committed. I looked
  at the new render: the gap under the hand is there, hand and button still
  centred across; the screen is still not centred down (282).
- One responsibility per file: the line's geometry stays in `inline.rs`,
  and the margins are read in `engine.rs` where the item is made; no file
  gained a second reason to change.
- `docs/features.md` gains the margin-box line; `docs/conformance.md` says
  what sits on a line and names 283 and 284; its offline-screen row now
  names 282 and 283.

**Roadmap.** This item served **no open roadmap line**, and `ROADMAP.md` is
deliberately unchanged, for iteration 173's reason: it corrects inline
layout under stage 1's ticked *Layout* line, and no stage 2 line covers it
(*CSS beyond what alo needed* is new capability, not a fix). Queue: 281
ticked with what was done and what was cut; 283 and 284 opened. Also
updated: `CHANGELOG.md`, `docs/features.md`, `docs/conformance.md`,
`REMAINING.md`.

**Unresolved obligations.**
- 282 (the `place-*` shorthands) is the first eligible item in file order;
  283 (the strut) and 279 are eligible too. 284 waits on a page.
- Carried, unchanged: ADR 0022 § 2's wording and § 4's name order, and a
  side-by-side of the hand with a real browser (a person's call); 269
  blocked on a `rav1d` release; the `image-webp` upstream report; 109, 179,
  and 82, 83, 85–89, 95–99, 104 and 105 still needing their designs.

118 queue items are open (one closed, two opened). The first eligible item in
file order is now **282**. The next unused queue number is **285** and the
next ADR is **0023**. This is one iteration, not a finished queue or roadmap.
