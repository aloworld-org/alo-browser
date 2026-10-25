# ADR 0063 — A tab an agent opens is its action, done in a tab rather than a document

**Status:** accepted
**Date:** 2026-10-25
**Context:** ADR 0012 (every request says what caused it), whose § 2 names
three causes and an agent action *"in the document it acted in"*; ADR 0003
(identity allocated once and never reused); ADR 0055 (the agent socket),
whose § 6 refuses an agent's ask before anything is looked at and whose § 8
keeps agents to sites; ADR 0060 (Chrome's own network, every request decided
here first); queue items 633, 634 and 638; `alo-grants`' `table.rs`, where a
person's cause is a privilege

## The decision in one line

When an agent asks for a tab to be opened, that ask is an **agent action**,
minted when the browser process accepts it, and the tab's first page is
caused by it — an agent action carries **either the document it acted in or
the tab it opened**, so there are still three causes and the person is never
named for what an agent did.

## The question

`Tab::open_for_agent` (item 633) opens a tab at an address an agent named,
after the agent's sites are checked and before Chrome hears of it. Item 634
then had to write a cause on that tab's first page, and ADR 0012 had nowhere
to put it:

- **Not the person.** Nobody asked; an agent did.
- **Not an agent action**, as § 2 words it, because an action carries *"the
  document it acted in"*, and a tab that does not exist yet holds none.
- **Not a document**, for the same reason.

So it was written as the person's, the tab being theirs, and item 634 said
in so many words that this is the one line in the record a person reading it
could take for something they did. Item 638 asked for one of three answers:
an agent action with no document, a fourth cause, or an agent that may not
open tabs at all.

## 1. It is the agent's, and never the person's

Three things already in this repository make the person's name the wrong
answer, not merely an imprecise one.

- **ADR 0012 § 4 is about exactly this sentence.** A cause that could be
  stated by the wrong party is refused because it *"would launder its
  fetches into the person did that, which is precisely the sentence
  somebody would later rely on"*. That ADR was guarding against a renderer.
  The browser process writing the same sentence about an agent is the same
  forgery made by the party that is supposed to prevent it.
- **ADR 0012 § 6 keeps durably only what reaches an agent action.** A tab an
  agent opened, recorded as the person's, falls out of the one record that
  survives a restart — the record that exists to answer *what did it do
  while I was not watching*. Opening a site is the first thing an agent
  does on most tasks, and it would be the thing missing.
- **A person's cause is a privilege.** `alo-grants` lets only
  `Cause::Person` answer or dismiss a prompt, and `person_opened` starts a
  remembered permission's thirty days again when a person opens its site
  themselves. An agent's visit written as the person's would keep a site's
  camera grant alive on the strength of a visit nobody made. Today no
  caller joins the two; the day one does, the wrong cause becomes a wrong
  grant rather than a wrong line.

## 2. Not a fourth cause: an agent action happens in a document or in a tab

ADR 0012 § 2's three causes already answer *who*: a person, a document, an
agent. What § 2 also wrote down is *where*, and only the person's cause was
given a tab — because a person's navigation is *"the one cause that happens
in a tab rather than in a page"* (`Cause::in_document`). Opening a tab is
the agent's version of the same thing: an action that makes the document
rather than acting in one.

So the **agent action's place** is one of two things:

- **the document it acted in**, as today, for every verb; or
- **the tab it opened**, for an agent's ask that opens a tab, when it acted
  in no document at all.

Who caused it stays one of three, and § 2's *there is no fourth* holds in
the sense it was written for: there is still no `Unknown`, `Internal` or
`Other`, and no category for awkward cases to collect in. A fourth variant
would have said *who* again with a different word — an agent is an agent
whether it clicked or opened — and every reader of the record that asks
*did an agent do this* (`Chain::action`, `Chain::followed_from`)
would have had a second variant to remember, which is how the first one
that forgets it is written.

**The chain ends there.** A tab's first page caused by an agent action in
that tab has nothing behind it but the action, as a person's first page
has nothing behind it but the person: the walk ends at the action and the
tab, a fifth way for a chain to end beside the person's. The action's own
cause — the person who asked the agent — is outside the record, as it is for
every verb (§ 3).

## 3. What the action is, and when it is minted

- **Minted when the browser process accepts the ask**: after the address
  parses and the agent's sites reach it, and before Chrome — or this
  engine's renderer — is told anything. That is where a verb's action is
  minted (when it is accepted, not when it succeeds), moved to the only
  point an open has.
- **An ask refused before then mints nothing.** ADR 0055 § 6 refuses before
  anything is looked at, and the socket's own record already writes the
  refusal (`NotAnAddress`, `NotGranted`). No tab exists for an action to
  have been done in, and an action with no place is the category § 2
  refuses.
- **What it covers is the opening**: the first page's first hop and the hops
  a redirect leads it to (§ 2's *a redirect to whatever asked for the first
  hop*). What that page then asks for is the page's — a `Document` cause
  whose chain leads to the action, and so into the durable record, without
  the agent being credited with the page's whole life (§ 4's timer rule).
- **The tab is still the person's.** It is in their browser, among their
  tabs, and whatever they then do in it is theirs. Only the cause of what
  the agent did there says *agent*.

## 4. Agents may open tabs

Refusing it was the third answer, and it is not taken.

The check that matters is already made and made first: an agent kept to
sites cannot name an address off them, and that refusal is written before
Chrome is asked anything (item 633, ADR 0055 § 8). Removing the open would
not narrow what an agent can reach; it would make every task begin with a
person opening a tab for it, and hand an agent that wants a site it was
granted to the nearest page that links there. Item 633 is built and tested,
and no page has failed for it. Taking it away is a scope decision about the
product, which this ADR declines to make by implication, as ADR 0059 § 3
declined to demote the engine path.

## Consequences

- `alo-net`'s `Cause::Agent` carries its place, a document or a tab, rather
  than a document; `Cause::in_document` answers `None` for the second, as it
  does for a person; `Display` says *action#n, in tab#t*. Every match on the
  variant changes with it — which is the point of § 1 of ADR 0012: the
  compiler finds them.
- `alo-net`'s chain walk ends at an agent action in a tab, as it ends at a
  person's navigation, and `Chain::action` and `Chain::followed_from` find
  that action.
- `Tab::open_for_agent` mints its action once the agent's sites reach the
  address, and its tab's first page is minted with that cause. The tab can
  say which action opened it, as `Tab::acted` says which action a verb was.
- Item 638 closes when that is built, with a test reading the line back:
  the first page of a tab an agent opened is caused by the agent's action in
  that tab, its page's own requests lead back to it, and a tab the person
  opened is still the person's.
- `alo-grants` needs no change: it already refuses every cause but a
  person's, and now that refusal reaches an agent's opening.

## What this does not decide

- **Whether the socket offers an agent a way to open a tab.** ADR 0055 does
  not, and version 1 of its contract has no such ask. Adding one is
  additive and is its own item; this decides only what is recorded when a
  tab is opened at an agent's word, by whatever reaches `open_for_agent`.
- **A tab a page opens** (`window.open`, `target=_blank`). Chrome's tab
  closes any tab its page opens before it loads (item 633), and on this
  engine it is item 85's. A page's tab is the page's document's to cause
  (§ 2), and that is unchanged.
- **Who is allowed.** As ADR 0012 says of itself, a record is not a
  permission; grants are 497's and ADR 0055 § 8's.

## How we will know if this was wrong

- **If a person reads the record after an agent opened a site and cannot
  tell that the agent opened it** — if the line reads as theirs in any view
  item 127 builds — the place was the wrong field to put it in, and the
  answer is to show it, not to add a cause.
- **If a second thing an agent does turns out to happen in neither a
  document nor a tab** — closing a tab is the likely candidate — then *place*
  was the wrong axis and a cause of its own should be argued for then, with
  two cases rather than one.
