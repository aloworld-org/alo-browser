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
