# ADR 0044 — A list item's marker is a run of text outside its first line, and an agent reads it as a property

**Status:** accepted
**Date:** 2026-10-11
**Context:** queue item 401, *A list item's marker*, opened by
`alo-sites-rich-text` (iteration 260) and marked **needs ADR**. The body of
every blog post alo Sites publishes has a bulleted list with a list inside
it, a numbered list that starts at three (`<ol start="3">`) and a checklist.
A browser draws a disc beside each outer point, a hollow circle beside the
nested one, and "3." and "4." beside the numbered ones. This engine draws
none of them: `alo-box`'s `display.rs` records that a `display: list-item`
box is one and nothing makes its marker. ADR 0043 styled the first
pseudo-element, `::placeholder`, and left `::marker` to the first page that
wants one, to say then *what makes one and what its box is*. This is that
page.

Also read: ADR 0002 (one tree, two readers; roles and states declared);
ADR 0013 § 3 (*absent beats approximate*); ADR 0043 whole, whose §§ 1–2 this
follows and whose § 2 it has to extend; `alo-css`'s `selector.rs`
(`PseudoElement::is_produced`) and `shorthand.rs`; `alo-style`'s
`pseudo.rs`, `inheritance.rs` and `user_agent.rs` (`ul, ol, menu` are
already `list-style-type: disc`, `ol` `decimal`, `li` `list-item`);
`alo-box`'s `display.rs`, `tree.rs` (`nearest_style`, the placeholder set)
and `state.rs`; `alo-agent`'s `name.rs` and `tree.rs`; the case's page,
`agent.txt` and `origin.txt`. Read 2026-10-11, fetched for this decision:
**Chromium's** `list_marker.cc`, where a disc, a circle and a square are
text — the counter style's representation with its suffix, `"• "` — and
an outside marker is placed by `InlineMarginsForOutside`: a text marker's
start margin is minus its own width, so it ends where the item's line
starts; a symbol's start is `ascent × 2 / 3 + 7 + 1` before the item's
start (`kCMarkerPaddingPx` is 7); and **Chromium's** `html.css`, which
gives `ul, menu, dir` `disc`, `ol` `decimal`, `li` `display: list-item`, a
list inside a list `circle` and one inside that `square`, and has no
working `counter-reset: list-item` (a TODO naming its bug), because it
numbers items by HTML's ordinal-value algorithm rather than by CSS
counters. CSS Lists 3 (`::marker`, `list-style-*`, the `list-item`
counter, the outside marker taking no room), CSS Counter Styles 3 (`disc`
is `•` with suffix `" "`, `circle` `◦`, `square` `▪`, `decimal` with
suffix `". "`, and an unknown name is `decimal`), HTML § 4.4.8 (an `li`'s
*ordinal value*: its list owner, `start`, `reversed`, `value`).

## The decision in one line

A `display: list-item` box whose `list-style-type` is not `none` makes a
**`::marker`**, styled as ADR 0043 § 1 says; its text is **the counter
style's representation of the item's ordinal value**, by HTML's algorithm;
it is **a run of text set outside the item's first line**, on that line's
baseline, **taking no room** in it; and an agent reads it as the item's
**`marker` property** — `listitem "Draft" [marker="3."]` — **never as text
in the item's name**.

## Why this is a decision rather than a chore

Three things are decided here that code would otherwise decide by accident.

**Whether a pseudo-element exists can depend on style.** ADR 0043 § 2 made
"does this element make one" a fact about the document, held once. A
marker is made by a *computed* `display` and `list-style-type`, so that
cannot hold as written, and how it changes decides where the fact lives
for `::before` and `::after` too, which are made by `content`.

**How an item is numbered.** CSS has a `list-item` counter and HTML has an
ordinal value, and they agree for HTML's lists. Building CSS counters means
`counter-reset`, `counter-increment`, `counter-set`, scoping and
`counter()`, none of which a page here writes; Chromium has not finished
that path either. Which one this engine follows is a choice.

**What an agent is told.** "3." is meaning — the third step, not the first
— and a screen reader speaks it. A bullet is mostly decoration and a screen
reader speaks that too. Folding either into the item's name would break
every agent that finds a point by what it says; leaving them out would
withhold from an agent what a person sees, which ADR 0002 exists to stop.

## 1. A marker is made by the item's computed style, and that is one fact

An element **makes a `::marker`** when its computed `display` is a list
item (`alo-box`'s `display.rs` already says so) and its computed
`list-style-type` is anything but `none`. This extends ADR 0043 § 2: the
fact is still held **once**, but for a pseudo-element made by style it is
held **where the style is computed**. ADR 0043 § 1 already computes a
pseudo-element straight after its element, so the element's own style is
finished when the question is asked. The style tree answers it, computes
the marker's style only for an element that makes one, and the box tree
and the agent tree read the same answer rather than asking `display` and
`list-style-type` again. `::placeholder` keeps its document fact; nothing
about it changes.

`PseudoElement::is_produced` gains `::marker`. A rule naming `::marker` on
an element that makes none matches nothing, as ADR 0043 § 2 says of any
pseudo-element.

## 2. Its style is `::marker`'s, as ADR 0043 § 1 computes it

Computed as the item's child, inheriting from it — so a marker is the
item's colour and font unless a page says otherwise, and `var()` on it reads
the item's custom properties. The user-agent sheet gains HTML's rendering
section's two nesting rules: a `menu`, `ol` or `ul` inside one of those is
`circle`, and one inside two is `square` (obsolete `dir` left out, law 1).
CSS Lists' `::marker` defaults — `unicode-bidi: isolate`,
`font-variant-numeric: tabular-nums`, `white-space: pre`, `text-transform:
none` — are not written into the sheet: this engine reads none of those on
a marker, and a rule it cannot read is decoration.

**Of what applies to `::marker`, `color` and the font properties are
read.** They are what draws the run; the font properties are read as the
text already reads them. `content` on `::marker`, and every other property
CSS lets a marker take, is **recorded as an issue naming it** and left out,
as ADR 0043 § 3 does for `::placeholder`, until a page asks.

**`list-style` is split.** alo Sites' sheet takes the markers off its
navigation and its card grids with `list-style: none`, eight times — the
corpus holds ten frozen copies of it — and alo's workspace writes it in
its own `global.css`. A marker drawn while the shorthand was unread would
put a bullet beside every one of those links and cards. So `alo-css` splits `list-style`
into `list-style-type`, `list-style-position` and `list-style-image` **in
the same change that first draws a marker**, by CSS Lists 3's grammar,
`none` included (a lone `none` sets the type and the image).

## 3. Its text is the counter style's representation of the ordinal value

**The number is HTML's ordinal value** (§ 4.4.8): the item's list owner,
the owner's `start` and `reversed`, an item's `value`, and the items it
owns. Not CSS counters, for the reason above; they are cut to their own
item and wait for a page. Integers are read by HTML's rules for parsing
integers, and one that does not fit an `i32` is read as absent, as a
browser does; the arithmetic saturates rather than overflows, so `start`
of `2147483647` and a reversed list of a million items are answered, not
panicked on.

**The representation is CSS Counter Styles 3's**, with its suffix:

| `list-style-type` | text |
|---|---|
| `disc` | `• ` |
| `circle` | `◦ ` |
| `square` | `▪ ` |
| `decimal` | the number, `-` before a negative one, then `. ` |
| `none` | no marker (§ 1) |

These five are built. **A name no specification defines is `decimal`**, as
Counter Styles says. **A predefined style this engine has not built** —
`lower-alpha`, `upper-roman`, and the rest — and a `<string>` value draw
**no marker** and are recorded as an issue naming them: a decimal "4." where
the page asked for "iv." is approximate, and absent beats approximate
(ADR 0013 § 3). `<ol type>`, which maps to these, comes with them.

A disc, circle and square are **text, not drawn shapes**, as Chromium has
them: one run, through the same shaping and fallback chain as any other
text. A machine with no font holding `◦` gets what any missing glyph gets,
said where any missing glyph is said.

## 4. It is a run of text outside the item's first line, and takes no room

The box tree makes the marker **a text box, recorded as the item's
`::marker`**, and `nearest_style` answers its style with the
pseudo-element's, as it does for a placeholder — so layout and paint read
one style and neither learns what a marker is.

- **Outside is the position built.** `list-style-position: inside`, which
  makes the marker an inline box at the start of the first line, is cut to
  its own item; until then an `inside` marker is **not drawn** and is
  recorded, by § 3's rule.
- **It stands on the item's first line**: its baseline is that line's
  baseline. The first line is the first line box in the item's flow, inside
  a block descendant if the item starts with one, as CSS says. An item with
  **no line at all** has nothing to stand on; it draws no marker and the
  case is cut to its own item, because where a browser puts it then is a
  rule no page here has shown.
- **It takes no room.** The item, its line and every box in it are exactly
  where they are without a marker, so no layout number in any reference
  moves except the new box's own. It is set in the item's start padding or
  margin, where the 40 px of a list's indent leaves it room.
- **Where it stands across the line** is Chromium's rule: a **text**
  marker ends at the start of the item's line, its suffix's space being the
  gap; a **disc, circle or square** starts `⌊ascent × 2 / 3⌋ + 8` pixels
  before it, the ascent being the marker font's. Both are measured from the
  item's content edge on the line's start side.
- **It is painted after the item's background and with its text**, and is
  clipped by whatever clips the item, as any of its content is.

## 5. An agent reads a `marker` property, never text

- **The item carries a `marker` property**: the marker's text as drawn,
  with its trailing space trimmed — `[marker="3."]`, `[marker="•"]`,
  `[marker="◦"]`. A screen reader is given the same text: the platforms
  expose a list marker's text, and that is what is spoken before the
  point. It is one fact for both readers (ADR 0002), not one invented for
  agents.
- **It is not in the item's name.** A list item is named from its content,
  and the marker is not content: `listitem "Draft" [marker="3."]`, never
  `"3. Draft"`. An agent looking for the point that says *Draft* finds it.
- **The marker's box is not read as a node of its own** and not as text,
  exactly as a placeholder's is not.
- **An item that makes no marker carries no property**, so a list styled
  `list-style: none`, as alo's navigation and card grids are, reads
  exactly as it does today.

## 6. What closes queue item 401

401 closes when:

- `alo-sites-rich-text` draws a disc beside "Choose one useful outcome",
  "Make the next step visible" and "Publish", a circle beside "Keep
  supporting detail nested", and "3." and "4." beside "Draft" and
  "Review" — **in pixels** (ink where § 4 puts each marker and none in the
  item's own text area that was not there before) **and in numbers** (each
  marker box's position worked by hand from § 4 and the font's metrics);
- no other number in the case's `layout.txt` moves, and every corpus
  reference that moves is a marker appearing, reviewed in the same commit;
  the cases whose sheets write `list-style: none` do not move at all;
- its agent outline reads `listitem "Draft" [marker="3."]`, the nested
  point `[marker="◦"]`, and every item's name unchanged;
- unit tests show § 3's table, `start`, `reversed` and `value`, a name no
  specification defines read as `decimal`, an unbuilt style and `inside`
  drawing nothing and recorded, `list-style` split with `none`, and an
  author's `::marker { color }` beating the item's;
- hostile `start` and `value` attributes — 100 000 digits, `-2147483649`,
  `2147483647` with a reversed list of 100 000 items, text — are read or
  refused without a panic.

## What this costs

**Five counter styles.** A page with a lettered or roman list loses its
markers, and says so in its issues, until one asks.

**No CSS counters.** `counter-reset: list-item 5` on an `ol` does nothing
until a page writes it, and is recorded as a property this engine does not
read. HTML's attributes, which is what every frozen page here uses, work.

**One extension to ADR 0043 § 2.** The fact of a pseudo-element can now be
style's as well as the document's. Both are still held once.

## Alternatives rejected

**Draw a disc, circle and square as shapes**, as browsers once did. They
are text in Chromium's current code, and a shape would need its own size
rule, paint path and agent text when the run already has all three.
Rejected.

**CSS counters for numbering.** Correct in general and three properties,
their scoping and `counter()` that no page here writes, where HTML's
algorithm numbers every list a page here has. Rejected for now, cut.

**Read the marker as part of the item's name.** An agent would look for
"3. Draft". Rejected (§ 5).

**Leave the marker out of the agent's tree.** "Step 3" would be drawn for a
person and withheld from an agent and a screen reader. Rejected (§ 5).

**Draw `decimal` for every list style not built.** Approximate where the
page asked for something else; ADR 0013 § 3. Rejected.

**Give the marker room in the line**, as `inside` does. That is a
different position, and every list's text would move. Rejected for
`outside`, cut for `inside`.

## What this does not decide

- **`list-style-position: inside`**, **`list-style-image`**, the counter
  styles beyond § 3's five with `<ol type>`, `<string>` markers and
  `::marker { content }`, **CSS counters**, `::marker`'s other properties,
  and **an item with no line** — each waits for a page, as its own item.
- **`<details>`' `<summary>`** and its disclosure triangle, which is a
  marker too and comes with `<details>`.
- **Right-to-left lists**, whose marker stands on the right; with
  writing modes (item 98).
- **Script**: an item added or renumbered by script is the document
  changing, read the same way.

## How we will know if this was wrong

**If an agent misreads a list** — takes "3." for part of a point's text, or
cannot tell step three from step one — § 5 failed.

**If adding a marker moves text** anywhere, § 4's *no room* was not kept.

**If `::before` or `::after` cannot use § 1** because the fact of whether
one is made needs something neither the document nor the element's style
holds, the extension was a marker's after all.
