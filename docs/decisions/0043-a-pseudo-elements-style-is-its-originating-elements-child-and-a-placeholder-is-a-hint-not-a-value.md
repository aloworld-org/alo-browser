# ADR 0043 — A pseudo-element's style is its originating element's child, and a placeholder is a hint, not a value

**Status:** accepted
**Date:** 2026-10-10
**Context:** queue item 389, *A field's `placeholder`*, opened by
`alo-sign-in` (iteration 251) and marked **needs ADR**. alo's sign-in screen
has `<input type="email" placeholder="you@company.eu">`, and alo's own
`Input` component (`alo-workplace`'s `web/src/ds/Input.tsx`, read only)
styles it `placeholder:text-tertiary` — Tailwind's spelling of
`::placeholder { color: var(--text-tertiary) }`. A browser draws
`you@company.eu` in that colour inside the empty field. This engine draws
nothing: `alo-box`'s `field_text` shows a value only, and `alo-css` keeps a
selector naming a pseudo-element, records it as
`PseudoElementNotProduced`, and never matches it (`matching.rs`), because
stage 1 produced no pseudo-elements.

Also read: ADR 0002 (the layout tree is the agent's tree, and a screen
reader and an agent get the same facts from it); ADR 0013 § 3 (*absent
beats approximate*); ADR 0042 § 3 (an empty date field's format is its
*text*, not a placeholder, and is not this); `alo-css`'s `selector.rs`,
`matching.rs`, `issue.rs`; `alo-style`'s `computed.rs` (`resolve_admitting`,
which styles elements in document order, parent first) and
`user_agent.rs`; `alo-box`'s `tree.rs` (`field_text`, `nearest_style`, the
one place a box asks which style it is set in); `alo-agent`'s `name.rs`;
`alo-paint`'s `build.rs` (`style_of`). Read 2026-10-10, fetched for this
decision: **Chromium's** user-agent sheet (`html.css`), which gives
`::-webkit-input-placeholder` `color: #757575`, and for an `<input>`'s
placeholder `line-height: initial !important`, `white-space: pre` and
`overflow: hidden`; **Firefox's** `forms.css`, which gives `::placeholder`
`color: color-mix(in srgb, currentColor 54%, transparent)` and clips an
`<input>`'s placeholder in its inline axis; **HTML-AAM** (the editor's
draft), which names an `<input>` by `aria-labelledby`, `aria-label`, its
`<label>`, its `title` and **then its `placeholder`**, maps `placeholder`
to ARIA's `aria-placeholder`, and — by its own change log, 21 November
2016 — *removed* `placeholder` from the description computation. The queue
item said HTML-AAM makes a placeholder a field's description when a label
names it; that is no longer so, and § 5 rests on what it says now. CSS
Pseudo-Elements 4: a pseudo-element **inherits from its originating
element**, and the properties that apply to `::placeholder` are those that
apply to `::first-line`.

## The decision in one line

A pseudo-element's style is **computed as a child of its originating
element** — from the rules that name it and match that element, inheriting
from that element's computed style — **only for an element that makes
one**, and is kept beside the element's own in the style tree, read through
the one place a box asks for its style. The first is **`::placeholder`**: an
empty text field with a `placeholder` draws it as a run of text in the
field's line, in `::placeholder`'s `color` (`#757575` unless a page says
otherwise), adding nothing to the field's size; an agent reads it as the
field's **`placeholder` property**, and as its name only when nothing else
names it — **never as its text or its value**.

## Why this is a decision rather than a chore

Two things are decided here, and the first is not about placeholders.

**How a pseudo-element is styled.** `::placeholder` is the first; `::before`,
`::after`, `::marker` and `::selection` come after it, each opened by its
own page. Where a pseudo-element's style comes from, what it inherits from,
when it is computed, where it is kept and who reads it are the same
question for all of them, and the first one built answers it for the rest
whether anybody meant it to or not. A model that fits only placeholders —
a colour looked up on the side, say — is the one every later pseudo-element
would have to unpick.

**What an agent is told.** A placeholder looks like text in the field and is
not in it. An agent that read `you@company.eu` as the field's content would
believe the field filled and submit an empty form; one that never learned
the placeholder exists loses the one hint the author gave about what goes
in. ADR 0002 makes the box tree the agent's tree, so the placeholder's box
is in the agent's tree, and what the agent makes of it has to be said.

## 1. A pseudo-element's style is computed as its originating element's child

For an element that makes a pseudo-element (§ 2), its style is a
`ComputedStyle` computed **straight after the element's own**, by the same
cascade:

- **Its declarations** are those of the rules whose selector names that
  pseudo-element and whose compound before it matches the originating
  element. Matching stays the `selectors` crate's (ADR 0001); what changes
  is that `alo-css` stops answering *never* for a pseudo-element this
  engine makes, and keeps answering it — and recording
  `PseudoElementNotProduced` — for every one it does not.
- **It inherits from the originating element's computed style**, as CSS
  Pseudo-Elements 4 says: an inherited property it does not set is the
  element's, and so is every custom property, so `var(--text-tertiary)` on
  `::placeholder` resolves against the `--text-tertiary` the field sees.
- **The cascade's order is unchanged.** Origin, importance, attachment,
  specificity, order: a pseudo-element's rules compete with each other, by
  the five questions `cascade.rs` already asks, and never with the
  element's. A `style` attribute styles the element, never its
  pseudo-elements, because an attribute has no selector to name one.
- **It is kept beside the element's style**, in the style tree, under the
  element and the pseudo-element together. An element that makes none has
  none kept, and costs nothing.

## 2. It is computed only for an element that makes one, and that is one fact

Whether an element makes a pseudo-element is a fact about the **document**,
not about the style sheet, and it is held **once**, as `alo-dom`'s `date.rs`
holds what a date field holds, for the three that ask: the style tree (is
there a style to compute), the box tree (is there a box to make) and the
agent tree (is there a property to read).

For `::placeholder` it is HTML's rule: a **`<textarea>`**, or an
**`<input>`** of a kind that takes one — `text`, `search`, `url`, `tel`,
`email`, `password`, `number` — with a **non-empty** `placeholder`
attribute, whose value is **empty**. A date field takes none (its format is
its text, ADR 0042 § 3). For an `<input>` the hint is shown with its line
breaks stripped, as HTML asks; a `<textarea>` keeps them.

A rule naming a pseudo-element no element makes is not an error: it
matches nothing, as a rule naming an absent class does. A rule naming one
this engine does not make **at all** is still recorded as
`PseudoElementNotProduced`, so a page that wants `::before` says so in its
issues rather than silently missing it.

## 3. The placeholder is a run of text in the field's line, in its own colour

The box tree makes the placeholder **a text box inside the field, where the
field's text would be**, recorded as being the field's `::placeholder`.
When the box tree asks which style that box is set in (`nearest_style`), it
answers the pseudo-element's, so layout and paint read one style in one
place and neither learns what a pseudo-element is.

- **Its colour is `::placeholder`'s `color`.** The user-agent sheet gains
  `::placeholder { color: #757575 }`, Chromium's colour: a fixed grey, 4.6:1
  against white, which an author's sheet overrides as alo's does. Firefox's
  `currentColor` at 54% is a better colour on a dark field and needs
  `currentColor` in a mix (item 378, not built); it is not taken by halves.
- **Its line is the field's.** It is laid out as the field's value would
  be, in the field's font and line, so the empty field 384 stands on its
  strut's baseline stands there still, and a field neither moves nor
  changes height when somebody types into it. Chromium reaches the same
  place another way, setting the placeholder's `line-height` to `initial`
  with `!important` so no author can move it; here the run simply has no
  line of its own to move.
- **It adds nothing to the field's size.** A browser sizes an auto-width
  field by its `size`, never by its placeholder, and a field that shrank
  when somebody started typing would be wrong in a way they would see. So
  the placeholder's run is drawn and contributes no width of its own to an
  auto-sized field.
- **Of the properties that apply to it, `color` is read.** CSS gives
  `::placeholder` `::first-line`'s list — fonts, spacing, `text-transform`,
  backgrounds, decorations. Each of the others either moves the run or
  paints a box around it, and none is asked for by a page. Until one is, a
  declaration of one on `::placeholder` is **recorded as an issue naming
  it**, not silently dropped (item 392). Properties that do not apply to it
  at all — `width`, `margin`, `display` — are ignored, as CSS says.
- **A placeholder longer than its field is clipped** to the field's content
  box, as both engines do. No frozen page has one, so this is cut to its
  own item (390) rather than built unasked: `alo-sign-in`'s fits.

## 4. A value hides it, whoever wrote the value

A field holding any value draws **its value and no placeholder**; a field
emptied draws the placeholder again. The value read is the one `field_text`
reads, so a `PutText` from an agent and a person's typing hide it alike —
the page sees one kind of input (ADR 0018) and the field shows one thing.

## 5. An agent reads a `placeholder` property, never text

The agent's tree **does not read the placeholder's box as text**: an empty
field with a placeholder has no text under it, exactly as it has none
without one, and so reads as empty. Instead:

- **The field carries a `placeholder` property** with the hint as written
  (after § 2's stripping), shown in the outline beside its other states:
  `textbox "Email" [placeholder="you@company.eu"]`. This is ARIA's
  `aria-placeholder`, which HTML-AAM maps `placeholder` to, so it is a
  fact a screen reader is given too, not one invented for agents. It is
  read only while the placeholder is shown (§ 4); a filled field has none.
- **It names the field only as a last resort**, after `aria-labelledby`,
  `aria-label`, a `<label>` and `title` — HTML-AAM's order, `name.rs`'s
  step 6. `alo-sign-in`'s field is named `Email` by its label and keeps
  that name.
- **It is not the field's description.** HTML-AAM removed it from the
  description computation in 2016, and an agent reading it twice — as a
  property and as a description — would be told one thing as two.
- **An author's `aria-placeholder`** is read as the property when
  `placeholder` is absent or empty, as HTML-AAM says. It is ARIA's, for an
  author's own widget, and draws nothing: only HTML's `placeholder` makes a
  `::placeholder`.

## 6. What closes queue item 389

389 closes when:

- `alo-sign-in`'s `style.css` carries the rule alo's `Input` writes,
  `.input::placeholder { color: var(--text-tertiary) }`, said in the case
  as Tailwind's spelling of `placeholder:text-tertiary`;
- its email field draws `you@company.eu` in `--text-tertiary`
  (`#7a6f62`) **in pixels**, at the field's text position, and the case's
  layout numbers are unchanged apart from the new text box;
- its agent outline reads `textbox "Email" [placeholder="you@company.eu"]`
  with no text under it, and a `PutText` into it hides the placeholder and
  reads the value as its text with no `placeholder`;
- a box-tree test shows the rule of § 2 — each `<input>` kind that takes
  one, a `<textarea>` keeping its line breaks, an `<input>` losing them, a
  date field, an empty attribute and a held value making none — and a
  `numbers.rs` assertion shows an auto-width field with a placeholder as
  wide as one without;
- a style test shows `::placeholder` inheriting a custom property from its
  field and a user-agent `#757575` beaten by an author's colour, and a
  `::before` rule still recorded as `PseudoElementNotProduced`;
- hostile `placeholder` attributes — 100 000 bytes, nothing but line
  breaks, control characters — are drawn, stripped or read without a
  panic.

## What this costs

**A style tree that is no longer one style per element.** Everything that
reads it reads elements, and keeps doing so; only `nearest_style` learns to
answer for a pseudo-element's box. That is the cost of every pseudo-element
after this one, paid once.

**`color` alone, for now.** An author who sets `::placeholder { font-style:
italic }` gets an upright hint and an issue saying so, until a page asks
(392).

**A placeholder longer than its field overflows it** until 390, which no
page we hold has.

## Alternatives rejected

**Look the placeholder's colour up on the side** — a special case in paint
that asks for `::placeholder`'s `color` and nothing else. It would draw
`alo-sign-in` today, and leave `::before`, `::marker` and every later one to
discover that pseudo-elements have no style of their own and build the
model this ADR builds, under a page that needed it yesterday. Rejected.

**Compute every pseudo-element for every element.** Correct, and a style
for each of seven pseudo-elements on every `<div>` that makes none of them.
§ 2's one fact costs nothing and is needed anyway by the box tree.
Rejected.

**Inherit from the field's parent, as a sibling of the field.** It is not a
sibling: it is inside the field and CSS says it inherits from it, which is
how `var()` reaches the field's custom properties. Rejected.

**Read the placeholder to an agent as the field's text.** The agent would
believe the field filled. Rejected (§ 5).

**Leave it out of the agent's tree.** The hint would be drawn for a person
and withheld from an agent and a screen reader, which is the
screenshot-reading gap ADR 0002 exists to close. Rejected.

**Firefox's colour, `currentColor` at 54%.** Needs 378, and would be the
first user-agent rule this engine could not draw. Rejected for now, and
revisited if 378 is built and a dark field's hint is found unreadable.

## What this does not decide

- **`:placeholder-shown`**, which reads § 2's fact; it waits for a page
  (391).
- **`::before`, `::after`, `::marker`, `::selection`, `::first-line`,
  `::first-letter`** — each follows §§ 1–2 when a page opens it, and each
  says then what makes one and what its box is. `::first-line` and
  `::first-letter` style part of a box rather than a box of their own and
  will need more than this.
- **Script**: a placeholder changed by script is the attribute changing,
  and is read the same way; the `placeholder` IDL attribute is forms'
  (item 82).
- **A field with focus**, which some platforms draw without its
  placeholder; nothing has focus yet (item 43).

## How we will know if this was wrong

**If an agent submits a form it believed filled** because it read a
placeholder as a value, § 5 failed at the one thing it was for.

**If the second pseudo-element built cannot use §§ 1–2** and needs its own
way of being styled, the model was a placeholder's after all.

**If a field moves or resizes when somebody types into it**, § 3's line or
width rule is wrong, and the page it happened on says how.
