# ADR 0033 — An element's style is its `style` attribute, and a value is kept as written

**Status:** accepted
**Date:** 2026-10-08
**Context:** queue item 339, *An element's `style`, from a page*, cut from 89
(CSSOM) in iteration 215 with the instruction that the iteration taking it
first say *whether the declaration's shorthand handling is a decision or a
specification to follow*; `alo-downloads`, alo's public download page, whose
`mark(a)` stops at `a.style.background = "#c7bfb2"` with "TypeError: cannot
write property 'background' of undefined"; ADR 0017 § 5 (every change goes
through `alo-dom`, and every change is counted) and § 6 (a changed document
is rendered again whole, when its rendering is read); ADR 0001 (rented
parsers carry none of our value); and the code this is about — `alo-css`'s
`declaration.rs` (a value is kept as written; `DeclarationBlock::push` splits
the shorthands that split by counting) and `shorthand.rs` (`background`,
`border` and `font` are *not* split, and are read where they are used,
longhand first); `alo-style`'s `cascade.rs` (the four questions, and
presentation hints as specificity-zero author declarations counted first);
`alo-paint`'s `background.rs` (`background-color` beats the shorthand's
colour wherever either was written); `alo-net`'s `csp.rs` (`Inline::Style`
with `Placement::Attribute`, and `'unsafe-hashes'`, item 191);
`alo-renderer`'s `pipeline.rs` (`draw` gathers `<style>` and `<link>`
sheets, and applies no policy to either); and `alo-bindings`'
`token_list.rs` (`classList`: a `[SameObject]` view holding only its
element's wrapper, live from the attribute).

## The decision in one line

The `style` attribute **is** the element's inline declaration block — the
only place it is kept. It is cascaded as an author declaration above every
selector and below `!important`, as CSS Cascade places it. `element.style`
is a `CSSStyleDeclaration` that holds **only its element**, reads the
attribute every time it is asked and writes it back through `alo-dom` every
time it is told. It names **only the properties this engine acts on**, from
one list in `alo-css`. It keeps **a value as written**, refusing only what a
style sheet would refuse. And **`background` stays one declaration**: CSSOM's
longhand expansion is not followed, because this engine does not hold
longhands for the shorthands it reads by kind, and holding them only here
would make the inline block disagree with every style sheet.

## Why this is a decision rather than a specification to follow

Most of CSSOM is specification, and is followed. Three parts of it presume
an engine that **parses every value against its property's grammar when it
is written**: setting a value the grammar refuses is ignored; a shorthand is
stored as its longhands; and reading a value back gives its canonical
serialisation, so `#c7bfb2` reads back as `rgb(199, 191, 178)`.

This engine was built the other way, on purpose and before this ADR.
`alo-css` keeps a value **as written** and leaves it to the stage that uses
it to read it (`declaration.rs`, stage 1): that is what lets an unknown
property be *kept and ignored*, and what keeps one parser per kind of value
rather than a grammar table that has to agree with every reader. And
`background`, `border` and `font` are **not split** into longhands, because
splitting them is parsing (`shorthand.rs`); `alo-paint` reads `background`
whole and lets `background-color` beat its colour.

So following CSSOM to the letter means building, for the inline block alone,
the grammar table and the canonical serialiser the rest of the engine
decided not to have. The inline block would then answer differently from the
style sheet beside it, for the same text, and the two would be cascaded
against each other. That choice is a decision, and it is made here.

## 1. The `style` attribute is cascaded where CSS places it

CSS Style Attributes: the attribute's value is parsed **as the contents of a
declaration block** — a list of declarations with no selector and no braces,
by the same parser and the same refusals as a style sheet's block, the
counted shorthands split by the same `DeclarationBlock::push`. A declaration
that cannot be read is dropped and said, as in a sheet.

CSS Cascade 4 § 6.1, *element-attached styles*: its declarations are **author
declarations that beat every declaration of the same importance reached
through a selector**, whatever that selector's specificity. So the cascade's
key gains one question between *origin and importance* and *specificity*:
whether the declaration is attached to the element. That is the
specification's order, not a very high specificity: `!important` in a sheet
still beats a normal inline declaration, and an important inline declaration
beats an important sheet declaration.

It applies to an HTML or SVG element in the document. A `style` inside a
`<template>`'s contents styles nothing, as a `<style>` there does not. A
presentation attribute stays where `cascade.rs` puts it: a specificity-zero
author hint counted before every sheet, so `style="fill: red"` beats
`fill="blue"` by being element-attached.

**It is read from the attribute every time the page is drawn.** Nothing
parsed is kept on the element. ADR 0017 § 6 renders a changed page whole, so
a cache of parsed attributes would be a second copy of the document that had
to be kept right, for a speed nobody has measured (law 3).

## 2. A page's policy and the `style` attribute

`style-src` governs a `style` attribute written in the markup or set with
`setAttribute`, and `csp.rs` already decides it (`Inline::Style`,
`Placement::Attribute`, a digest only under `'unsafe-hashes'`). **A write
through `element.style` is not refused**: CSSOM marks the attribute as being
updated by the declaration, so HTML's attribute-change steps do not re-check
it, and every engine allows it.

The renderer does not apply `style-src` to inline style today, to a
`<style>` element or anything else (`pipeline.rs`'s `draw` takes no policy).
Applying it is **its own item (343)**, for `<style>` and the attribute
together, because one renderer-side rule for both is the right shape and
the `<style>` half is a gap that exists already. That item must keep CSSOM's
distinction. A blocked attribute contributes nothing to the cascade, and the
`CSSStyleDeclaration` reads it as empty. A write through `element.style`
replaces it and is applied. So `alo-dom` has to record, per element, whether
the attribute's current value was written by the declaration. Item 343
builds that record, and this ADR requires it. Until 343, a page's
`style-src` refuses neither a `<style>` nor a `style` attribute, and
`docs/conformance.md` says so.

## 3. `element.style` holds its element and nothing else

`ElementCSSInlineStyle`: `style` on every HTML and SVG element is
`[SameObject, PutForwards=cssText]`. Its `CSSStyleDeclaration` is built
**exactly as `classList` is** (`token_list.rs`): an embedder cell holding
its element's wrapper, made once and held by the wrapper, both edges strong
and traced (ADR 0017 § 3).

**It holds no declarations.** Every read parses the element's `style`
attribute (§ 1's parser). Every write parses it, makes the change CSSOM
describes, serialises the result and **sets the attribute through
`alo-dom`'s operation** — or removes it when nothing is left, as CSSOM's
*update style attribute* does. So:

- the attribute and the declaration can never disagree, because there is
  one of them;
- a `setAttribute("style", …)` is seen by the next read, with no attribute-
  change step to keep in sync;
- every write is a counted change (ADR 0017 § 5), so the page is drawn again
  when it is read (§ 6), and an agent sees it;
- a write that changes nothing — the same value and priority as already
  held, or removing a property that is not there — leaves the attribute
  alone and counts no change, as CSSOM's *update style attribute* runs only
  when its set or remove reported a change.

Serialising writes each declaration the block holds **as it was written**
(`name: value` with ` !important` if it had it), joined by `; `. It never
writes the longhands `DeclarationBlock::push` added for a counted shorthand,
since those were never written. This is CSSOM's *serialize a CSS declaration
block* without its shorthand-combining step, because the block holds the
shorthand itself (§ 5).

## 4. It names only the properties this engine acts on

CSSOM gives a `CSSStyleDeclaration` one camel-cased attribute
(`pointerEvents`), one dashed attribute (`pointer-events`) and, for a
`-webkit-` property, one WebKit-cased attribute **for each supported CSS
property**. `setProperty`, `getPropertyValue` and `removeProperty` ignore a
name that is not one. Which properties are supported is therefore what a page
detects with `'backdropFilter' in el.style`.

**A property is supported when some stage of this engine acts on it** —
style, boxes, layout, paint, SVG or the agent's tree — and not because a
specification lists it. That is the rule `docs/features.md` already holds a
script to: *a page that checks before using a feature reads the truth*.

There is no list today. A property's reader is wherever it is read. So
**one list is made, in `alo-css`** (`properties.rs`), with each entry
naming the crate that reads it. A test asserts the list is sorted and has no
duplicates. A test in each reading crate asserts that every property that
crate reads is listed, so a property cannot be read without being listed,
or listed without being read. Custom properties (`--anything`) are always
supported, through `setProperty` and `getPropertyValue` only, as CSSOM says.

So `a.style.cursor = "default"` and `a.style.pointerEvents = "none"` on
`alo-downloads` set **ordinary properties of the object**. No stage reads
`cursor` or `pointer-events` (there is no pointer to point with), and every
engine treats an unsupported property name this way. An agent's press is a
keyboard's click (ADR 0018), which `pointer-events` does not stop in any
engine, so nothing an agent can do changes either way.

## 5. A value is kept as written, and `background` stays whole

**Setting.** CSSOM's *setProperty* steps are followed except at step 6. The
name is lowercased (unless custom) and must be supported. The empty string
removes. A priority must be empty or `important`. Step 6 parses the value
against the property's grammar; here instead the value is **accepted when a
style sheet would keep it** as one declaration's value: it tokenises, it is
not empty after trimming, and it carries no `!important` or `;` of its own,
which would make it a second declaration. Nothing more is checked, because
nothing more is checked of a style sheet's value until it is used. A value
that cannot be used is then refused **where it is read** and said
(`IssueKind::UnsupportedValue`), as a sheet's is.

**Shorthands.** A counted shorthand (`margin`, `padding`, the three
per-side borders, `place-*`) is split exactly as in a sheet, because
`DeclarationBlock::push` does it. A shorthand read by kind (`background`,
`border`, `font`) **stays one declaration**, as in a sheet. One part of
CSSOM's longhand handling is kept, because without it this engine would draw
the wrong thing. **Setting a shorthand removes every declaration of its
longhands from the block** before the shorthand is written, and the
shorthand-to-longhands table lives in `alo-css` beside the counted ones.
`alo-paint` lets `background-color` beat `background` wherever both are
written, so `style="background-color: red"` followed by
`style.background = "blue"` would otherwise stay red. Setting a longhand
leaves its shorthand in place and is written after it, which the cascade's
order already resolves within the block.

**Reading.** `getPropertyValue` and the named getters answer the **value as
written** in the last declaration of that name, and `""` when there is none.
Two answers differ from other engines:

- `#c7bfb2` reads back as `#c7bfb2`, where they answer `rgb(199, 191,
  178)`;
- `backgroundColor` after `background: red` reads `""`, where they answer
  the colour.

Both follow from holding text rather than parsed values. Both are recorded in
`docs/conformance.md`. And both are the trigger in *How we will know*.

## 6. What is built first

Item 341 builds § 1: the attribute cascaded, with a layout assertion and a
reference render of its own. Item 342 builds § 4's list and, on `CSSStyleDeclaration.prototype`: the named attributes of
§ 4, `cssText` (get and set, the setter replacing the whole attribute),
`length`, `item()`, `getPropertyValue`, `getPropertyPriority`,
`setProperty`, `removeProperty`, `parentRule` (always `null`, since no
inline block has a rule) and `cssFloat` only if `float` is ever supported,
which it is not in stage 2 (law 1). An indexed getter (`style[0]`) comes
with `item()`. **Nothing else is half-built.** `getComputedStyle`, a
`CSSStyleSheet` and `document.styleSheets` are the rest of item 89, each
opened by a page.

## What this costs

**A page that checks its own write by reading it back** gets the text it
wrote rather than the canonical form. A page comparing `el.style.color ===
"rgb(255, 0, 0)"` after setting `"red"` takes the other branch here.

**An invalid value set from script overrides a valid one from a sheet**,
where other engines would have refused the write and kept the sheet's. This
is already true of an invalid value written in a later style rule, for the
same reason, and the issue list says which value was refused.

**Every read of `element.style` re-parses the attribute.** A page that sets
forty properties in a loop parses a growing attribute forty times. No claim
about speed is made, and none is measured.

## Alternatives rejected

- **Parse every value against its grammar when it is set**, as CSSOM says.
  It needs a grammar for every supported property. Built for the inline
  block alone, it makes the attribute and the sheet beside it disagree about
  the same text. Built for the whole engine, it reverses `alo-css`'s first
  decision. That is worth doing only when a page fails because of it, and
  then it is its own ADR.
- **Expand `background` into its eight longhands in the declaration**, as
  CSSOM says. The cascade would then compete the inline longhands against
  sheet shorthands that are *not* expanded, by property name. That is the
  exact bug `DeclarationBlock::push` records for `padding` (an author's
  shorthand silently losing to a longhand), turned around.
- **Keep the declarations on the element and write the attribute as a
  copy.** Two copies of one fact, and `setAttribute("style")` would need the
  attribute-change steps to re-parse into the copy. One copy cannot
  disagree with itself.
- **Name every property CSS defines.** A page detecting `backdropFilter`
  would be told it exists, and then not see it drawn.
- **A very large specificity for the attribute.** Cascade 4 makes it its
  own step, and the difference shows: an important inline declaration must
  beat an important sheet declaration with an id selector, and a normal one
  must lose to any important sheet declaration.

## What this does not decide

- **The mechanism of § 2.** It is item 343's to build. It may need its own
  ADR if *who records that a value came from the declaration* turns out to
  be a decision about `alo-dom`'s element rather than a flag on it.
- **`getComputedStyle`** and computed values. Those are what the cascade
  produces, and serialising them is the same canonical-form question as § 5,
  asked of a different block.
- **`document.styleSheets`**, `CSSStyleSheet`, `insertRule` and constructed
  sheets — item 89, each opened by a page.
- **Validating values when they are parsed**, for sheets and the attribute
  alike. If a page ever needs it, it is decided for both at once, here or in
  a successor ADR.

## How we will know if this was wrong

**If a frozen page reads back a style it set and compares it** to another
engine's canonical form, and takes a different branch because of it, § 5's
*reading* is wrong for real pages. Canonical serialisation is then owed, and
it is owed for the value kinds that page reads, not for every property.

**If a frozen page sets a value this engine keeps and another engine
refuses**, and the page then looks different because the sheet's value lost,
§ 5's *setting* is wrong, and validation is owed for that property.

**If a property is acted on somewhere and missing from `properties.rs`**, or
listed and read nowhere, § 4's tests were not written as § 4 says. That is a
bug in those tests, not a question about the list.
