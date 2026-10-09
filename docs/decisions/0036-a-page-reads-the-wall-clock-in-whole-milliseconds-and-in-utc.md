# ADR 0036 — A page reads the wall clock in whole milliseconds, and in UTC

**Status:** accepted
**Date:** 2026-10-09
**Context:** queue item 353, *`Date`*, opened by a page: `alo-sites-cta`,
the call-to-action section alo Sites publishes for its customers (frozen in
iteration 227). Its analytics script, which every page alo Sites publishes
carries, stops at its third line, `var since = Date.now();`, with
"ReferenceError: 'Date' is not defined", before it adds a single listener.
The item is marked **needs ADR** because *what clock a page reads is a
decision — its precision is a timer a Spectre gadget (ADR 0005) wants, and
ADR 0030 says the browser tells a page no locale, of which a time zone is a
part.* Also read: ADR 0005 § *Why* (Spectre, and that site isolation is the
mitigation that works); ADR 0010 (the sandbox is rented, and the renderer is
inside it); ADR 0013 § 3 (*absent beats approximate*), § 4 (no panic, no
overflow on hostile input) and § 5 (*`alo-js` depends on … not a clock …
`Date.now`, `Math.random`, `fetch` and every other capability arrive as
things the embedder passes in*); ADR 0016, whose *What this does not decide*
leaves timers' clock and coarsening to item 92 because *a high-resolution
clock is a side channel*; ADR 0030 § 2 (*never read from the machine at run
time: no system version, no processor, no locale, no screen*) and its
*What this does not decide*, which leaves `language` to the person's
settings (item 128); ADR 0031 (a builtin's state in slots); ECMA-262
§ 21.4 (*Date Objects*: time values, `LocalTZA`, `Date.parse`, the string
forms); HTML's *Timers* and High Resolution Time's note on coarsening.
The code this is about: `alo-js/src/realm.rs`, whose realm has no `Date`,
no `Math` and no clock; and `alo-renderer/src/page.rs`, through which the
browser process tells a renderer what a page may know about the browser
(`user_agent`, `platform`).

## The decision in one line

`Date` is the engine's, and **the instant is the embedder's**: `alo-js`
defines a `Clock` the realm is handed, and the renderer's one is the
machine's wall clock read in **whole milliseconds, floored** — the grain the
language's time values already have, and nothing finer. A page's **local
time zone is UTC** until the person chooses one in the browser's settings;
the renderer never reads the machine's zone. Tests and corpus cases read a
**fixed** clock, so no reference depends on the day it was rendered.

## Why this is a decision rather than a chore

`Date` looks like arithmetic on a number, and almost all of it is: ECMA-262
specifies every step from a time value to a year, a month and an hour. Three
things in it are not specified, and the first line of code would decide each
of them by accident.

**Where now comes from.** ADR 0013 § 5 gives `alo-js` no clock, on purpose:
an engine that reads nothing is an engine a test can hold still. So the
instant has to be handed in, and *who* hands it in, from where, and what a
realm does when nothing was, are this ADR's.

**How fine it is.** A clock is a ruler, and a fine enough ruler measures a
cache hit. That is how Spectre was turned from a paper into a web attack:
`performance.now()` at microseconds, and then `SharedArrayBuffer` as a
counter when that was coarsened. Every browser has since decided a grain for
each clock a page can read. `Date.now()` is one of those clocks.

**Which zone "local" is.** `getHours`, `toString`, `getTimezoneOffset` and
every method without `UTC` in its name read *local time*, and ECMA-262 leaves
`LocalTZA` — the offset — to the implementation. The machine's zone is a
fact about the machine and, more than most, about where the person is. ADR
0030 says the renderer tells a page nothing about the machine it did not
choose to.

## 1. The instant is handed to the engine, never read by it

`alo-js` gains a trait, **`Clock`**, with one question: *what is the time
value now* — an `f64` of milliseconds since the epoch, an integer, or `NaN`
if the embedder cannot say. A realm is made with a clock or with none.

- The engine **never reads the machine**. ADR 0013 § 5 stands unchanged: the
  crate depends on no clock, and the clock is a thing passed in, like an
  interrupt.
- `Date` itself — the constructor, `Date.UTC`, every getter, setter and
  string form — is **the engine's**, because it is the language, and lives in
  `alo-js/src/builtin/` under ADR 0031's rules like every other builtin.
  Only *now* is the embedder's.
- **A realm with no clock refuses to say what time it is, by name.**
  `Date.now()`, `new Date()` and `Date()` throw a `TypeError` saying the
  realm was given no clock. Every other use of `Date` — `new Date(0)`,
  `Date.UTC(…)`, a getter on a date the page made — works, because it needs
  no clock. A made-up instant (zero, or the last one seen) would be the
  approximate answer ADR 0013 § 3 forbids: a page cannot tell it from the
  truth.
- **A clock that cannot say answers `NaN`**, never a panic: a machine whose
  clock is before 1970 or past ECMA-262's ±8.64 × 10¹⁵ ms range makes an
  *Invalid Date*, which a page can test for.

## 2. The renderer's clock is the machine's wall clock, in whole milliseconds

The renderer gives every realm it makes the same clock, composed in **one
file in `alo-renderer`**: the machine's wall clock (`SystemTime`), as
milliseconds since the epoch, **floored to a whole millisecond**.

- **The renderer reads it itself.** It is the one fact about the outside the
  renderer reads without asking, and it is in reach of the sandbox (ADR
  0010) on every system we build for. Asking the browser process for each
  reading would make `Date.now()` a round trip. Being *told* the time with
  each message — a clock frozen per task — was considered and is rejected
  below: it turns `while (Date.now() < end) {}` into a loop that never ends.
- **A whole millisecond is the language's own grain.** ECMA-262's time
  values are integers of milliseconds, and `Date.now()` answers one. So the
  grain is not a mitigation we chose; it is the floor of what `Date` can
  express, and this ADR's decision is that **nothing a page reads through
  `Date` is finer**, and the renderer does not hand the engine a fraction to
  round later.
- **No jitter.** Some browsers add noise to a clock so that a page cannot
  find the instant it ticks over and measure between two ticks. Noise makes
  two readings in order disagree with their order, which breaks code that is
  honest; and the attack it slows is answered by site isolation (ADR 0005):
  a renderer that holds no other site's data has nothing a timer can read.
  Rejected, with the condition for reopening it below.
- **No promise of monotonicity.** `Date` is the wall clock, and the wall
  clock moves when the machine's is set. That is what the language says and
  what every page that measures with `Date` already lives with. A monotonic
  clock is `performance.now()`'s, which this ADR does not build.

**This decides `Date` only.** `performance.now()`, a timer's due time and
`requestAnimationFrame`'s timestamp are other clocks, finer or monotonic by
specification, and each is coarsened by a decision of its own: timers are
item 92 (ADR 0016), and High Resolution Time is opened by a page that needs
it. Whatever they decide, **none may be used to read `Date` finer than a
millisecond** — a page given two clocks gets the coarser one's grain from
each.

## 3. A page's local time zone is UTC until the person chooses one

`LocalTZA` is **zero**, always, in every renderer, until the person has
chosen a time zone in the browser's settings. Then the browser process tells
the renderer that zone with the page, as it tells it the user agent (ADR
0030 § 4), and the renderer never reads the machine's zone itself.

- **So `getHours()` is `getUTCHours()`**, `getTimezoneOffset()` is `0`, and
  `toString()` ends `GMT+0000 (Coordinated Universal Time)` — the zone's
  name as the two largest engines spell it for UTC.
- **Why not the machine's zone.** A zone is around five bits about the
  person, and the bits are *where they are*. It is not a fact about the
  browser, like ADR 0030's frozen tokens; it is a fact about the person, like
  `language`, and ADR 0030 left that to the person's settings (item 128). A
  zone the browser read from the machine would be the one value under ADR
  0030 that varies by person without the person having said anything.
- **What it costs is named below**, and it is a real cost: a page that shows
  a time "in your time" shows it in UTC.
- **When the person chooses, the zone's rules are rented.** Offsets and
  daylight-saving transitions are the IANA database's, which nobody writes
  by hand. Which crate, behind which file, is decided with the setting
  (item 358), not now. Until then the engine needs no database at all: UTC
  has no rules.
- **`Intl` (item 79) must agree.** Whatever `Intl.DateTimeFormat` reports as
  the default zone is this one; two answers to one question would be a
  fingerprint and a bug.

## 4. A date as text: the language's forms, and nothing older

- **`toString`, `toDateString`, `toTimeString`, `toUTCString` and
  `toISOString`** write the forms ECMA-262 specifies, with § 3's zone.
- **`Date.parse` and `new Date(string)` read what ECMA-262 says they must**:
  the *Date Time String Format* (ISO 8601's subset), and whatever this
  engine's own `toString` and `toUTCString` write, so the round trip the
  specification promises holds. **Anything else is `NaN`** — an Invalid Date,
  which is the language's own answer to text it does not recognise, not an
  approximation. The forgiving parsers other engines kept for thirty years of
  pages (`"10/09/2026"`, `"Oct 9 2026"`) are the legacy surface law 1
  refuses; one is reopened by a frozen page that fails for want of it, and
  added as that form, by name.
- A date-only ISO form is **UTC** and a date-time form without an offset is
  **local**, as ECMA-262 says; with § 3 they agree until a zone is chosen.
- **`toLocaleString`, `toLocaleDateString` and `toLocaleTimeString`** are
  ECMA-402's, and wait for `Intl` (item 79). They are not built with `Date`.
  Until then `Object.prototype.toLocaleString` answers for the first, as
  ECMA-262 allows an engine without ECMA-402 to, and the other two are
  absent.
- **Annex B is refused**: `getYear`, `setYear` and `toGMTString` are absent,
  as Annex B's regular-expression forms are (ADR 0029).
- **The text is hostile input.** `Date.parse` reads a stranger's string, so
  it is bounded by what it reads (no backtracking), refuses rather than
  panics on every input, and its arithmetic on a year of `+275760` or a
  field of a billion digits is done in `f64` as ECMA-262 writes it, never in
  an integer a hostile length can overflow (ADR 0013 § 4).

## 5. A test and a corpus case read a fixed clock

**A reference never depends on the day it was rendered.** `alo-js`'s and
`alo-bindings`' tests hand a realm a clock that answers one chosen instant.
`alo-corpus` loads every case with a fixed clock — one instant, named in one
place with its reason — so a page that writes the date into itself draws the
same pixels every day, and a case's references move only when the engine
does. The renderer's real clock (§ 2) is tested by itself: that it floors,
that it is within the machine's own reading, that it answers `NaN` rather
than panicking out of range.

## What this costs

- **Times "in your zone" are in UTC** until the person sets one. A calendar,
  a "posted 3 hours ago", a shop's "open now" will be off by the person's
  offset. That is the price of not reading where the person is; it is paid
  until item 358 lets them say it, and it is the same price Tor Browser
  charges for the same reason.
- **The machine's clock is readable**, including how far it is from true
  time. Clock skew is a known fingerprint, and every browser exposes it the
  same way. A browser that lied about the time would break every page that
  checks an expiry, and buy nothing a second request could not take back.
- **A page sees time move in steps of a millisecond.** Code that measures
  something shorter than that gets zero, which it already gets in the
  browsers that coarsen most.

## Alternatives rejected

**The machine's time zone, as most browsers send.** Rejected in § 3: it is a
fact about the person that the person did not choose to tell, and ADR 0030
keeps those for the settings.

**A clock frozen for each task, told by the browser process with every
message.** It would make a page's own timing within a task worth nothing,
which is a real defence. It is rejected because `while (Date.now() < end)`
— a busy wait real pages write — would never end, and a page whose clock
does not move while it runs is a page that cannot measure its own work.

**Jitter, or a coarser grain (16 ms, 100 ms) for `Date`.** Rejected in § 2:
site isolation is the mitigation that works, and a coarser `Date` breaks
pages that compare two readings a few milliseconds apart for nothing that
isolation has not already given.

**A clock inside `alo-js`.** Rejected by ADR 0013 § 5, and this ADR does not
reopen it: the engine stays testable with nothing moving.

**A realm without a clock answering zero, or the epoch.** Rejected in § 1:
a page cannot tell a made-up instant from a real one, and absence that
throws is the honest answer.

**The forgiving date parser other engines have.** Rejected in § 4 under
law 1, until a frozen page names the form it needs.

## What this does not decide

- **`performance.now()`, `performance.timeOrigin` and High Resolution
  Time.** Their grain is a decision of its own, opened by a page that needs
  them. § 2's one constraint binds it.
- **Timers** (item 92): their clock, clamps and coarsening.
- **The zone setting itself** — where it lives, how the person sets it, and
  which crate's copy of the IANA database is rented (item 358, on item 128).
- **`Intl`** (item 79), beyond § 3's rule that it agrees with `Date`.
- **`Temporal`.** Not built, and opened by a page that uses it.

## What this makes buildable

- **356. `Date`, with a clock.** `alo-js`: the `Clock` trait and a realm made
  with one or none (§ 1); the `Date` constructor in all its forms but a
  string, `Date.now`, `Date.UTC`, the getters and setters, local and UTC
  (§ 3, `LocalTZA` zero), `valueOf`, `getTime`, `toISOString`, `toJSON` and
  `[Symbol.toPrimitive]`. `alo-renderer`: § 2's clock in one file, handed to
  every realm it makes. `alo-corpus` and the tests: § 5's fixed clock.
  `new Date(string)` and `Date.parse` are refused by name, pointing at 357.
  *Closes when:* `alo-sites-cta`'s script runs past line 3, in
  `tests/alo_sites_cta.rs`, and what it stops at next is opened as an item;
  a realm with no clock throws for `Date.now()` and still answers
  `new Date(0).getTime()`; the renderer's clock is floored and in range, in
  a test; and a fixed clock makes a date the page writes the same in every
  run.
- **357. A date as text.** *Depends on 356.* § 4: `Date.parse` and
  `new Date(string)` over the Date Time String Format and this engine's own
  `toString` and `toUTCString`; `toString`, `toDateString`,
  `toTimeString`, `toUTCString`, and `Date()` called as a function. *Closes
  when:* every form round-trips, a form the language does not specify is
  `NaN`, and malformed, truncated and adversarial strings are refused
  without a panic, in tests. *Opened by a frozen page that writes or reads
  a date as text, and not before.*
- **358. The person's time zone.** *Depends on 128 (settings).* § 3: the
  setting, the zone told to a renderer with the page, and the IANA rules
  rented behind one file. *Closes when:* a person who chose a zone sees a
  page's local time in it, and one who did not sees UTC.

## How we will know if this was wrong

**If the person's week in stage 2's exit gate names a site whose times are
wrong because they are in UTC**, and setting the zone is the answer they
reach for, item 358 is late, not wrong. If they reach for it on every site,
§ 3 is wrong for the default, and the remedy is an amendment that asks the
person once, on first run, rather than one that reads the machine silently.

**If a renderer that holds only its own site's data is shown to leak across
sites through `Date`'s millisecond**, § 2's grain was wrong, and the
amendment coarsens or jitters it with the measurement that showed it.
