# ADR 0042 — An empty date field shows how a date is written, and the order is the person's

**Status:** accepted
**Date:** 2026-10-10
**Context:** queue item 385, *What an empty date field shows*, cut from 384
by iteration 250 and marked **needs ADR**. It is opened by a page:
`alo-sites-booking`, the booking section alo Sites publishes for its
customers, whose form holds `<input type="date" required>` labelled
"Choose a day". A browser draws that field, empty, as its date's format —
`mm/dd/yyyy` to a reader in the United States, `dd/mm/yyyy` to one in
Britain — with a button that opens a calendar. This engine draws the field
empty, as it draws any field with no value, and an agent reads it as
`generic "Choose a day" [required]`, a box it is told nothing about.

Also read: ADR 0002 (the layout tree is the agent's tree; roles are
declared, not inferred; acting is typed verbs, *put this text in that
field*, and no verb takes a coordinate); ADR 0013 § 3 (*absent beats
approximate*); ADR 0018 §§ 4–6 (an agent's verb is the browser's input,
and a field's text arrives through `beforeinput`); ADR 0024 § 4 (the tab
strip is a document the engine renders, which is how the browser's own
surfaces are made); ADR 0030 § 2 and *What this does not decide* (the
renderer reads no locale from the machine, and `language` waits for the
person's settings, item 128); ADR 0036 § 3 (a page's time zone is UTC until
the person chooses one, told to the renderer with the page). HTML's
*date state* (`type=date`): its value is a *valid date string*,
`yyyy-mm-dd`, and a value that is not one is sanitised to the empty string;
*how it is presented* is left to the user agent. HTML-AAM: `input
type=date` has **no ARIA role**; its platform mappings are AT-SPI's
`ROLE_DATE_EDITOR` and macOS's `AXDateField`. Chromium's user-agent sheet
(`html.css`, read 2026-10-10): it gives an empty date field's segments **no
colour of their own** — they are drawn in the field's `color`, and only a
disabled segment is `GrayText` — and draws the picker's indicator as a 1em
icon with 2 px of padding at the field's inline end. Firefox's rendering was
not checked, and nothing below rests on it.

The code this is about: `alo-box`'s `tree.rs` (`field_text`, which shows
the value of the kinds of `<input>` that hold text and nothing for any
other) and `role.rs` (`input_role`, which answers `generic` for a date
field and says why: *HTML gives no role … inventing one is what this file
exists to prevent*); `alo-agent`'s `verb.rs` (`PutText`); and `alo-layout`'s
`engine.rs`, which since 384 stands a one-line field with no line on the
line it would hold.

## The decision in one line

An empty `<input type="date">` **draws how a date is written in it** —
`yyyy-mm-dd`, as text in the field's own colour, laid out as its line —
because the order of a date's parts is the **reader's**, and until the
person chooses a region in the browser's settings the order is **ISO
8601's**, which is the order of the field's own value and the one order no
reader misreads. The agent reads it as a **`date`**, the platforms' own
role, with the value in the value's form and the format never mistaken for
one, and puts a date into it as a **valid date string**. The calendar is
**not drawn** until it opens something.

## Why this is a decision rather than a chore

Everything else about an empty field is CSS's or HTML's: where its line
stands (384), how high it is, what colour its text is. Three things here are
nobody's but ours, and the first line of code would decide each by
accident.

**Whose order it is.** `03/04/2026` is the third of April to one reader and
the fourth of March to another, and the field exists so that a person can
say which day they mean. HTML specifies the value and leaves the
presentation to the user agent. Every user agent answers with *a locale*,
and the only question is whose: the machine's, the page's (`lang`), or the
person's.

**What to show before anybody has said.** ADR 0030 tells a page nothing
about the machine it did not choose to, and leaves the person's language to
the settings that do not exist yet (item 128). ADR 0036 answered the same
question for the time zone with UTC. A date field needs an answer of the
same kind, and *American by default* is the one most engines give without
deciding it.

**What an agent is told.** `role.rs` refuses to invent a role, rightly, and
so an agent reading the booking form today cannot tell the day field from a
`<div>`. Either the field has a role or an agent cannot act on the one thing
the form exists for.

## 1. The order is the reader's, from the browser's settings, never the machine's or the page's

The order a date's parts are drawn in, and the separator between them, is
the **person's region**, which they choose in the browser's settings with
their language and time zone (item 128). The browser process tells the
renderer the region with the page, as it tells it the user agent (ADR 0030
§ 4) and the time zone (ADR 0036 § 3). The renderer **never reads the
machine's locale**.

- **Not the page's `lang`.** `lang` says what language the page's *text* is
  in, which is the author's. How the *reader* writes the fourth of March is
  the reader's: a British reader of an English page from an American
  company still means day-first, and a field that switched order with the
  page's `lang` would have the same person read the same digits two ways on
  two sites.
- **Not the machine's.** A locale is a fact about the person, read off
  their machine without their having said anything — exactly what ADR 0030
  § 2 refuses. It is the same argument ADR 0036 § 3 made for the zone.
- **A region chosen is visible to a page, a little.** A script can measure
  an auto-width date field, and a region whose format is wider makes a
  wider field. That is a bit the person chose to give, once they choose
  one; until then the field is the same width for everyone on a release.

## 2. Until the person chooses, the order is ISO 8601's: `yyyy-mm-dd`

With no region chosen, an empty date field draws **`yyyy-mm-dd`**, and a
date field holding a value draws it in the same order, `2026-10-12`.

- **It is the value's own form.** HTML's valid date string is
  `yyyy-mm-dd`, so what the field draws is what it submits and what a
  script reads from `value`: one form in three places, and nothing to
  convert.
- **No reader misreads it.** A four-digit year first cannot be taken for a
  day, and a year-first date with a month and a day after it is the
  international standard's, read year, month, day. `mm/dd/yyyy` and
  `dd/mm/yyyy` are each a guess about the person, and each guess is wrong for most of the people the other is right for —
  and alo's own pages are written for readers in Europe, of whom the
  American order is wrong for nearly all.
- **The words are the browser's own.** `yyyy`, `mm` and `dd` are English,
  the browser's interface language, which is also the person's to choose
  (item 128) and is the same setting's: a person who chooses French reads
  `aaaa`, `mm` and `jj` if their region writes them so. Until then English
  is not a guess about the person but the language the browser itself is
  written in.
- **A field and its value never disagree on order.** A region chosen
  changes both the format drawn empty and the order a value is drawn in,
  together, so a field does not reorder itself when somebody fills it.

## 3. The format is the field's text: its own colour, its own line

The format is drawn **as the field's text**, exactly where a value would
be, in the field's computed `color`, through the box `field_text` already
makes for a value.

- **Its own colour, not a placeholder's.** Chromium gives an empty date
  field's segments no colour of their own; they are the field's text. A
  `placeholder` is a different thing — the author's hint, styled by
  `::placeholder` — and is not this.
- **Its own line.** Because it is the field's text, the field's baseline
  is that line's, which is where 384 already stands an empty field, so the
  booking form's numbers do not move: the field 343 × 48.4 at (432, 120.6),
  the form 162 tall at 89.4.
- **A width the author did not set is the format's.** A date field with
  `width: auto` is as wide as its text, as any field is today. The
  indicator's 1em and padding are not added while there is no indicator
  (§ 5).
- **A value that is not a date is no value.** HTML sanitises a `value`
  that is not a valid date string to empty, so `value="12/10/2026"` draws
  the format, not those digits, and reads as empty to an agent and to
  constraint validation.

## 4. An agent reads a `date`, and puts a valid date string into it

The agent's tree gives `<input type="date">` the role **`date`**, and
`role.rs` says why: HTML-AAM gives it no ARIA role, but it gives it a
platform role on each accessibility API we will speak (AT-SPI's date
editor, macOS's date field), and ADR 0002 makes the agent's tree and the
accessibility tree one tree. So this is the platforms' role, not one we
made up, and `role.rs`' rule — *nothing true to say, so say nothing* — is
kept: there is now something true to say.

- **The value in the value's form.** The outline writes a held value as
  `2026-10-12`, the value's own form, whatever order the region draws it in,
  so an agent never parses a locale. An empty field has **no value** in
  the outline. The format drawn in it is not its value, and an agent that
  read `yyyy-mm-dd` as text would believe the field filled.
- **Putting a date is `PutText` with a valid date string.** ADR 0002 says
  *put this text in that field*, and a date is text in its value's form.
  It goes through `beforeinput` and `input` as any field's text does (ADR
  0018 § 5), so a page cannot tell the agent's date from a person's.
- **Anything else is refused by name**, not sanitised to empty. A person
  typing `12/10/2026` into the field is stopped by its segments; an agent
  whose text the field would silently empty would believe it had chosen a
  day. The refusal says the text is not a date the field can hold.
- **Every other temporal kind stays `generic`** — `time`,
  `datetime-local`, `month` and `week` — until a page asks for one (item
  388). Each would take its own value's form by the same rule.

## 5. The calendar is not drawn until it opens something

Chromium draws a calendar icon at the field's end that opens a picker. This
engine **draws none**, and adds no width for one, until the picker exists.

- **A button that does nothing is approximate.** ADR 0013 § 3: an
  indicator a person can press and nothing happens is worse than none,
  because they believe the browser broke. Without it the field is still
  fully usable: a person types a date and an agent puts one.
- **The picker is the browser's, drawn as a page.** When it is built it is
  a surface of the browser process, rendered by the engine as a document
  as the tab strip is (ADR 0024 § 4), opened by `Activate` on the
  indicator, and it puts its choice into the field as `PutText` would, so
  the page sees one kind of input whoever chose the day. That is item
  386, and it waits for a window to open it in (296).

## 6. What closes queue item 385

385 closes when, with no region chosen:

- `alo-sites-booking`'s day field draws `yyyy-mm-dd` in the field's
  computed colour, at the field's text position, **in pixels** — and the
  booking layout's numbers are unchanged (§ 3);
- its agent outline reads `date "Choose a day" [required]` with no value,
  and a filled date field reads its value as `yyyy-mm-dd`;
- a `PutText` of `2026-10-12` fills it, a `PutText` of `12/10/2026` is
  refused by name and leaves it empty, and malformed, truncated and
  adversarial value attributes and verb texts (out-of-range months and
  days, the 29th of February in a common year, years of more than four
  digits and none, hostile lengths) are sanitised or refused **without a
  panic**;
- `role.rs`' comment and the `date` role's test say why it is the
  platforms' role.

## What this costs

**A reader in the United States sees `yyyy-mm-dd`** where every other
browser shows them `mm/dd/yyyy`, until they choose a region. It is a format
they can read, not one they would misread, and it is the cost ADR 0036 paid
for the time zone, for the same reason.

**No calendar.** A person who would rather pick a day than type one cannot,
until 386. A date field is still usable, and nothing is drawn that does not
work.

**One role outside ARIA.** The agent's vocabulary was ARIA's roles until
now. `date` is the first platform role in it, and the next temporal kind
will want its own.

## Alternatives rejected

**`mm/dd/yyyy`, as most engines default.** A guess about the person, wrong
for most of Europe, and ambiguous for exactly the dates where being wrong
books the wrong day. Rejected.

**The page's `lang`.** The author's language decides the reader's date
order — and the same reader reads the same digits two ways on two sites.
Rejected (§ 1).

**The machine's locale.** It is what other browsers read, and it is a fact
about the person they did not tell us; ADR 0030 § 2. Rejected.

**Leave the field empty until the person chooses a region.** A box with
nothing in it does not say it wants a date, and an empty date field is
what a page that asks for a date relies on the browser to explain. ISO
order is an honest answer available today.

**Draw the format as a placeholder, greyed.** Not what Chromium does, and
it would make the format the first user of `::placeholder`, which is a
decision of its own (item 389). Rejected.

**Draw the calendar icon now and make it open nothing.** Approximate
(§ 5). Rejected.

## What this does not decide

- **The region setting itself**: where it is chosen, which regions and
  orders it offers, and the rented data behind them — CLDR's patterns are
  rented as the IANA zones are (ADR 0036 § 3), the crate chosen then. Item
  387, with item 128.
- **The picker** (386).
- **`time`, `datetime-local`, `month` and `week`** (388), opened by a page.
- **`min`, `max`, `step`, `required`'s validation and `valueAsDate`**,
  which are forms' and script's (item 82), and read the value, not the
  format.
- **`placeholder` and `::placeholder`** (389), opened by `alo-sign-in`,
  which needs the first pseudo-element and is its own decision.
- **Right-to-left and other scripts' digits**, which the region's patterns
  carry when 387 rents them, and 98 and 100 lay out.

## How we will know if this was wrong

**If a person reads a date field's order wrong** — books the wrong day — the
default failed at the one thing it was chosen for, and the region setting
was not reached in time.

**If an agent believes a field filled that is empty**, the format leaked
into the value (§ 4).

**If pages are found that read an empty date field's width** to decide
their layout, and break at `yyyy-mm-dd`'s, the width rule (§ 3) is wrong,
and that page says how.
