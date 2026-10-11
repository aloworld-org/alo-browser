# ADR 0049 — Text is transformed where it is collapsed, and an agent reads what is drawn

**Status:** accepted
**Date:** 2026-10-11
**Context:** queue item 441, *`text-transform`*, opened by
`alo-agenda-calendars` (iteration 281) and marked **needs ADR**. alo Agenda's
sidebar heads its two calendar lists `text-transform: uppercase` and its
shared calendars' access badges `lowercase`. This engine reads the property
nowhere: it is inherited (`alo-style`'s `inheritance.rs`) and allowed on a
`::placeholder` (`pseudo.rs`), and nothing after the cascade looks at it. So
"My calendars" is drawn and measured as written, 95.46 wide where Chrome's
"MY CALENDARS" is 111.69, and "Can edit" 50.19 where Chrome's "can edit" is
48.42, which moves the toggle beside it by 1.77. A second frozen case,
`alo-downloads`, writes `uppercase` on its badge and is drawn as written
too.

Drawing the transformed text is CSS Text 3 § 2.1 and is not in question.
What an agent reads is: the text a person sees, or the text the page wrote.

Also read: ADR 0002 (one tree, two readers; the DOM says what an author
wrote, not what is on screen); ADR 0013 § 3 (*absent beats approximate*);
ADR 0043 § 5 (a placeholder is a hint read from its attribute); ADR 0044
(a marker is a run of text, and CSS Lists' `::marker { text-transform: none }`
left unwritten because nothing read it); `alo-box`'s `whitespace.rs`, which
collapses white space when a box is built *"so that everything downstream —
layout, paint, the agent tree — reads the same text"*; `alo-agent`'s
`name.rs` (`text_of` reads box text; `label_for` reads the label's
`text_content` from the document) and `tree.rs` (`named` compares names
ASCII-case-insensitively); the case's `origin.txt`, `style.css` and
`agent.txt`.

**Measured, 2026-10-11,** in Google Chrome 154 on this machine, headless,
with DejaVu Sans by `@font-face`, through the DevTools protocol's
`Accessibility.getFullAXTree` and `innerText`, from scratchpad pages.
Chromium's source was not read.

- **The accessibility tree carries the transformed text.** An `uppercase`
  heading is `heading "MY CALENDARS"`, its text `StaticText "MY CALENDARS"`;
  a button is `button "SAVE DRAFT"`; a link `link "LINK TEXT"`; a
  `lowercase` span `StaticText "can edit"`. `innerText` agrees with it
  every time, as HTML's definition of `innerText` says it should.
- **A name taken from a label is the label as drawn**: a wrapping label, a
  `<label for>` and an `aria-labelledby` span, all `uppercase`, name their
  fields `"WRAPPED LABEL"`, `"POINTING LABEL"` and `"LABELLED BY"`.
- **A name or value the page gives as an attribute is as written**:
  `aria-label="Close dialog"` on an `uppercase` button is `"Close dialog"`;
  `alt`, `title` and `placeholder` likewise. A field's **value** is as
  written (`value="typed value"`), though the text drawn inside the field is
  `"TYPED VALUE"`; a `<textarea>`'s the same.
- **The mapping is Unicode's full one, and language-sensitive.** `straße` is
  `STRASSE`; `ŉ ǰ ΐ` are `ʼN J̌ Ϊ́`; `ΟΔΟΣ` lowercased ends in a final `ς`.
  Under `lang="tr"`, `lang="TR"`, `lang="tr-CY"` and `lang="az-Latn"`,
  `istanbul` is `İSTANBUL` and `I` lowercases to `ı`; under `lang="en"` or
  `lang=""` inside a Turkish paragraph it is `ISTANBUL`. Under `lang="el"`,
  `Όσο άλλο` is `ΟΣΟ ΑΛΛΟ`, its accents dropped, and without a language
  `ΌΣΟ ΆΛΛΟ`. Under `lang="lt"`, `Ì` lowercases to `i̇̀`, keeping its dot.
- **`capitalize` does not follow Unicode's word boundaries.** `ab.cd` is
  `Ab.Cd` (UAX #29 keeps it one word), `ab_cd` `Ab_cd`, `can't` `Can't`,
  `o'neil` `O'neil`, `3d` `3d`, `a1b` `A1b`; a word that runs across
  elements is capitalised once (`one<b>two</b>` is `Onetwo`); `ǆ` is
  titlecased to `ǅ`; `ßtraße` and `ﬁsh` are left alone; Dutch `ijsland` is
  `Ijsland` even under `lang="nl"`.
- **`full-width` and `full-size-kana` are drawn as written**: `innerText`
  and the accessibility text are unchanged.
- **Collapsed white space stays collapsed** and preserved white space is
  kept under a transform: `"  TWO  SPACES  "` under `white-space: pre`.

## The decision in one line

`text-transform` is applied **where white space is collapsed — when a text
box is built** — so layout, paint and the agent read **one** text, the
transformed one; `none`, `uppercase` and `lowercase` are built, by
Unicode's full, language-sensitive case mapping **rented from ICU4X**;
everything **drawn from the document's text** is read by an agent as
drawn, and everything a page gives **as an attribute or a field's value**
is read as written.

## Why this is a decision rather than a chore

**What an agent is told.** ADR 0002 rejects reading the DOM because the DOM
says what an author wrote, not what is on screen, and ADR 0044 kept a
marker out of an item's name because folding it in would change what an
agent searches for. Here the two pull against each other: the page wrote
"Save draft", a person sees "SAVE DRAFT". Either answer is defensible, and
whichever the code picks becomes the answer every agent depends on.

**Where in the pipeline.** A transform applied at paint would leave layout
measuring the wrong width; one applied at shaping would leave the box tree,
and so the agent, holding text nobody sees. The place decides who agrees
with whom.

**What is rented.** Case mapping is Unicode data plus language tailorings,
and the Rust standard library has the data but not the tailorings.

## 1. The transform happens when a text box is built, after collapsing

A text box's text is made, as today, by `alo-box`: white space is processed
by the box's `white-space`, then — new — the result is transformed by the
box's computed `text-transform`, read from the same style. That is CSS
Text 3's order of operations: white space collapsing, then transformation,
then line breaking, which happens later in layout on the text the box holds.

So **the box holds the transformed text**, and nothing downstream knows a
transform happened: shaping, measurement, line breaking, paint, the agent
outline and every reference read one string. This is `whitespace.rs`'s
argument, made again for the same reason: two transformations would
eventually disagree.

**Each text box is mapped on its own.** Upper- and lowercasing are per
character except at two contexts — a Greek final sigma and the Greek
uppercasing that drops accents — which look at the letters around them.
A word split across elements (`ΟΔΟ<b>Σ</b>`) is mapped without its
neighbour, and is cut (§ 7).

## 2. `uppercase` and `lowercase` are Unicode's full case mapping, rented

- **Rented from ICU4X's `icu_casemap`**, behind **one file**, `alo-text`'s
  `case.rs`, added to `scripts/gate.sh`'s boundaries. It is the same
  project as the `icu_*` crates already in the lock (through `url`'s IDNA),
  under the same Unicode licence, pure Rust, with compiled-in data and no
  file to load. Nobody here writes a case-mapping table, for the reason
  nobody writes a shaper.
- **Full mapping**, so a character may become several (`ß` to `SS`, `ŉ` to
  `ʼN`) and the text grows; the final sigma is lowercase's.
- **Language-sensitive**, by the element's language: the `lang` attribute
  of the nearest inclusive ancestor element that has one, read as a BCP 47
  tag and compared ASCII-case-insensitively; **an empty `lang`, an
  unparseable one, or none at all is no language**, and the root mapping
  is used. ICU4X's tailorings — Turkish and Azeri dotted and dotless `i`,
  Lithuanian's kept dot, Greek uppercasing without accents — are what the
  measurements above show Chrome doing. The document's language is not
  taken from an HTTP header or a `<meta>`; that is a page's to ask for.
- **`none`**, the initial value, leaves the text alone.

## 3. What is not built draws as written, and says so

`capitalize`, `full-width`, `full-size-kana`, `math-auto` and any
combination with them **draw the text as written** and are **recorded as an
issue naming the value**, as an unbuilt list style is (ADR 0044 § 3).

- **`capitalize`** is cut to its own item. Its rule is not UAX #29's word
  boundary (`ab.cd` is `Ab.Cd` in Chrome) but a measured one, crosses
  element boundaries, and titlecases rather than uppercases; neither frozen
  page writes it. Uppercasing every word's first letter by `split_whitespace`
  would be approximate, and absent beats approximate (ADR 0013 § 3).
- **`full-width` and `full-size-kana`** are what Chrome 154 does too —
  drawn as written — and wait for a page.

## 4. An agent reads what is drawn from the document's text

- **Text, and a name from content, is the transformed text**, because it is
  the box's text (§ 1): `text "MY CALENDARS"`, `button "SAVE DRAFT"`,
  `text "can edit"`. This is what Chrome gives a screen reader, measured
  above, and ADR 0002 makes it one fact for both readers.
- **A name from a label is the label as drawn.** `alo-agent`'s `label_for`
  reads the label's box text, as `text_of` reads any other, rather than the
  document's `text_content`. A label that makes no box is read from the
  document, as today: it has no drawn text to read.
- **A name or property from an attribute is as written** — `aria-label`,
  `alt`, `title`, a placeholder's hint (ADR 0043 § 5). The page wrote it to
  be read, not drawn, and no style applies to it; Chrome agrees.
- **A field's value is as written.** It is what the form submits and what
  `PutText` writes; the text drawn inside the field is transformed, by § 1,
  like any other drawn text, but the value an agent reads, and acts on, is
  the field's.
- **Finding by name is unchanged.** `AgentTree::named` already compares
  ASCII-case-insensitively, so an agent asking for "Save draft" finds
  `button "SAVE DRAFT"`. It does not fold beyond ASCII — `"Élan"` does not
  find `"ÉLAN"`, nor `"straße"` `"STRASSE"` — and widening it is a change
  to every lookup, not to this property; it waits until an agent needs it.

## 5. The document is not changed

`textContent`, a text node's data, `value` and every attribute keep what
the page wrote, and CSSOM reads `text-transform` as the specified value. A
transform is presentation. When `innerText` is built, HTML defines it on
rendered text and it returns the transformed text, as Chrome's does.

## 6. A list item's marker is not transformed

CSS Lists' user-agent `::marker { text-transform: none }`, which ADR 0044
left out because nothing read the property, is **written into the
user-agent sheet in the same change** that first reads it. Today's markers
(`•`, `◦`, `▪`, digits) are unchanged by any case, so no reference moves;
a lettered list, when it comes, keeps its letters under an `uppercase`
item.

## 7. What closes queue item 441

441 closes when:

- in `alo-agenda-calendars`, the headings read and measure `MY CALENDARS`
  and `OTHER CALENDARS`, the first 111.69 wide as Chrome's is, and the
  badges `can edit` and `can view`, the first 48.42, with the toggle beside
  it moved by the 1.77 Chrome moves it — in `tests/alo_agenda_calendars.rs`,
  in numbers, and in the reference render;
- `alo-downloads`' badge is drawn uppercase, its moved numbers checked
  against Chrome 154 and its reference reviewed in the same commit; every
  other corpus reference that moves is a transform appearing;
- the case's agent outline reads `text "MY CALENDARS"` and `text "can
  edit"`, and an agent finding `Named("My calendars")` still finds it;
- unit tests show `ß` to `SS`, a final sigma, Turkish `i` under `tr`,
  `tr-CY` and `az-Latn` and not under `en` or `""`, Greek uppercasing under
  `el`, the nearest `lang` winning, `white-space: pre` kept, a label named
  as drawn, an `aria-label` and a field's value as written, the marker
  untransformed, and `capitalize` and `full-width` drawn as written and
  recorded;
- hostile input — a 100 000-character run that doubles under `uppercase`
  (`ß` repeated), a `lang` of 10 000 characters and of NULs and controls —
  is mapped or refused without a panic, and growing text overflows nothing.

## What this costs

**A rented crate** and its data, a few tens of kilobytes, behind one file.

**Text grows.** A box's text and the document's text are no longer the
same length (`ß` is two letters uppercased). Nothing maps between offsets
in them today; caret, selection and `Range` will need that map, and it is
theirs to build (§ What this does not decide).

**An agent searching outside ASCII** for the words the page wrote, under a
transform, will not find them by name until lookup folds case.

**`capitalize` draws as written** until its own item.

## Alternatives rejected

**Read the page's own text, with the transform as a property**
(`text "My calendars" [transform=uppercase]`), as ADR 0044 does for a
marker. A marker is a thing beside the text; a transform is how the text
itself looks, and the platforms give a screen reader the transformed text.
An agent and a screen reader would disagree, which ADR 0002 exists to stop.
Rejected.

**Read both.** Two strings per node, one of which is not on screen, is the
DOM-reading ADR 0002 rejects, kept beside the tree. Rejected.

**Transform at shaping or at paint.** The box tree, and so the agent, would
hold text nobody sees, or layout would measure text that is not drawn.
Rejected (§ 1).

**The standard library's `to_uppercase` and `to_lowercase`.** Correct for
no language, and wrong for Turkish, Azeri, Lithuanian and Greek, which
Chrome tailors. A second path for the tailorings would be the two-mappings
problem inside one file. Rejected.

**`capitalize` by whitespace-separated words.** Approximate where Chrome is
measured to differ (`ab.cd`, `x—y`, across elements, `ǆ`). Rejected for
now, cut.

## What this does not decide

- **`capitalize`**, **`full-width`**, **`full-size-kana`** and
  **`math-auto`**, each its own item when a page writes it.
- **A context across text boxes** — a final sigma or Greek accent whose
  neighbour is in another element.
- **Offsets between drawn and written text**, for caret, selection,
  `Range` and find-in-page.
- **Folding beyond ASCII** in `AgentTree::named`.
- **The document's language from anywhere but `lang`**, and `xml:lang`.

## How we will know if this was wrong

**If an agent cannot find what a person sees**, or finds something a
person does not, § 4 failed.

**If a transformed run is measured one width and drawn another**, § 1's
one text was not kept.

**If a page's language-sensitive text is drawn differently from Chrome's**
under the same `lang`, § 2's rental was the wrong one.

## Amendment — 2026-10-11: where the rented file lives, and three things the build found

Made by the change that built queue item 441 (iteration 283), before any
code depended on the parts it changes. None of it changes a decision above;
each is a fact the build met that this record did not know.

- **The rented file is `alo-box`'s `case.rs`, not `alo-text`'s.** § 2 named
  `alo-text` because case mapping is text physics, but `alo-text` sits
  *above* the box tree — it depends on `alo-layout`, which depends on
  `alo-box` — and § 1 puts the transform in `alo-box`, when a text box is
  built. Naming the file in `alo-text` would have been a dependency cycle,
  or a seam threaded through every caller of `alo_box::build` to carry one
  pure function back down. So the crate that applies the transform rents
  it: `icu_casemap` and `icu_locale_core` (which reads a `lang` tag) are
  named in `crates/alo-box/src/case.rs` only, and `scripts/gate.sh` holds
  both to that file. The versions are the 2.1 line already locked through
  `url`; adding them moved no other crate.
- **A marker takes `text-transform` from its own style.** CSS Lists 3's
  list of what applies to `::marker` does not name the property, but
  Chrome 154, measured, draws `::marker { text-transform: uppercase }` on a
  `lower-alpha` item as `A.`, and keeps `a.` when only the item is
  `uppercase`. So `alo-style` reads it on `::marker`, the user-agent sheet's
  `::marker { text-transform: none }` (§ 6) is what stops inheritance, and
  the marker's text box is cased by the marker's style. No marker this
  engine draws has a letter in it yet (item 406), so nothing moves today.
- **A field's hint is cased by its `::placeholder` style**, which inherits
  the field's: Chrome 154 draws both `HINT INHERITS` and, from
  `::placeholder { text-transform: uppercase }`, `HINT OWN RULE`.
  `alo-style` now reads the property on `::placeholder`. A broken
  picture's `alt` is drawn in the `<img>`'s case (`ALT TEXT`, measured) and
  read, as before, from the attribute.
- **A name from `aria-labelledby` is still read as written.** § 4's label
  rule is built for `<label>`; `aria-labelledby` is resolved when the
  pointing element's box is made, before the box it points at may exist,
  so reading the target as drawn needs that resolution moved after the
  tree is built. Cut to queue item 446, with Chrome's `"LABELLED BY"` above
  as its measurement.
