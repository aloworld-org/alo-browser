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

