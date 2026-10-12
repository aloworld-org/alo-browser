# ADR 0055 — An agent reaches this browser over a socket, and a refusal is a kind

> **Renumbered from 0053 on 2026-10-12, and the commit that added it says 0053.**
> The build loop committed its own ADR 0053 — `XMLHttpRequest` — at 08:41, and
> this one landed at 09:15 with the same number. The loop had it first, so this
> moved. 0054 was already claimed by an iteration in flight.
>
> It happened because this was written in the same window the loop was writing
> in, having been careful about that for `loop.sh` and `gate.sh` all day and not
> about the one thing with a shared counter. ADR 0003's rule — identity
> allocated once and never reused — is about nodes and reads the same for ADR
> numbers, which is why the number moved rather than being shared.

**Status:** accepted
**Date:** 2026-10-12
**Context:** `CLAUDE.md` law 2 and ADR 0002 (the layout tree is the agent's
tree; no verb takes a coordinate); ADR 0005 (one process per site, and the
browser process never parses a page); ADR 0052 and its addendum, which reversed
an earlier choice of D-Bus and requires this interface to be ours and the same
everywhere; queue items 131 (built), 496 and 497; `alo-agent`'s `verb.rs`;
`alo-renderer/src/message.rs`

## The decision in one line

An agent outside this browser speaks to the **browser process** over a **Unix
domain socket**, one bounded JSON object per line, and every answer is a
**kind** — an outcome or one of `verb.rs`'s refusals — never a sentence a
caller has to parse.

## Why this is a decision rather than a chore

Item 131 built the capability and nothing can reach it. `message.rs` carries a
`Verb` across the renderer boundary and no binary sends one, so the claim this
project is justified by — *an agent reads what the interface is, never a
screenshot* — is true inside one process and unavailable to anybody.

What makes it a decision rather than plumbing is that **other people build
against it**. alo OS learned that on their own adapter format and said it
plainly: *contracts outlive code, so a shape you adopted for us and nobody
else is one you would carry for years.* They then asked us not to speak their
transport, so that we would get this right for every system instead of theirs.

## 1. A socket, and emphatically not a port

**Not a TCP port on localhost.** A page's own script can reach `localhost`
with `fetch`. A port would let any page drive the browser showing it — read
every tab, type into every field, follow every link — and no grant would be
consulted, because the page would be speaking the agent's protocol rather than
the page's. That is not a weakness to mitigate; it is a different product.

**Not stdio on a child process.** It inverts the lifetime: the agent would own
the browser, and a person's browser outlives any one agent's task. It also
admits only one agent, and makes "attach to the browser already open"
impossible.

**A Unix domain socket**, at a path under the user's own runtime directory,
with the permissions the operating system already enforces on it. Reachable by
processes that can read that path and by nothing else — no page, no site, no
network.

**On macOS and Linux that is `std::os::unix::net`** — in the standard library,
no rented crate and no `unsafe`.

**Windows is not decided here, and an earlier draft of this ADR claimed it
was.** That draft said a socket is *"one answer on all three systems"* because
Windows has supported `AF_UNIX` since Windows 10. The operating system does;
**Rust's standard library does not expose it**, and `std::os::unix::net` is
Unix-only by definition. So Windows needs a rented crate, or a named pipe, or
`unsafe` against the Windows API — and the third is refused by law 4 without an
ADR naming the boundary, which this is not.

The claim was corrected before this was committed rather than after, and it is
recorded because it is the shape of mistake this repository keeps making: a
true fact about the platform standing in for an unchecked fact about the tool.
Windows gets its own decision when there is a reason to make it; the protocol
above does not depend on which answer it gets.

## 2. The browser process answers, and never a renderer

ADR 0005 keeps page bytes out of the browser process and the renderer behind a
sandbox. Both survive here: the socket is held by the **browser process**,
which holds no page and parses none, and a verb is forwarded to the right
renderer over the message boundary that already carries it.

So the sandboxed thing stays unreachable from outside, and the thing reachable
from outside stays ignorant of page bytes. An agent cannot address a renderer
directly, and nothing new crosses into one.

## 3. One bounded JSON object per line

**JSON, rented behind one file.** An interface outsiders build against has to
be writable in any language in an afternoon. A binary format of ours would be
correct, smaller, and unused — and ADR 0001 rents physics, which a JSON reader
is: nobody's engine differs by it.

**Bounded by us, in front of the rented reader**, because ADR 0013's rule
holds here too: *a limit somebody else chooses is not a limit.* A line's
length, a name's length, the depth of a tree answered and the number of
verbs in flight are all ours and all refused rather than truncated.

## 4. A refusal is a kind, not a sentence

`verb.rs` has twelve — `NotFound`, `Ambiguous`, `NotOperable`, `Disabled`,
`NotAField`, `ReadOnly`, `NotADate`, `Unbuilt`, `DoesNotScroll`, `TooLong`,
`Unchecked`, `NotSubmittable` — and each carries its own fields. They cross as
a tag and those fields.

A refusal flattened into English is a refusal a caller parses with a regular
expression, and then our wording becomes their dependency: we could not
improve a sentence without breaking somebody. The English stays, beside the
kind, for a person reading a record.

## 5. A tab is named, because an index is a coordinate

With several tabs open a verb has to say which. Not a number: ADR 0002 refuses
a coordinate *"because a coordinate is a guess about a layout that may have
changed between the reading and the acting"*, and a tab index is that guess
about a strip.

A tab is named by its **address**. Two tabs at one address refuse with
`Ambiguous`, which already exists for exactly this shape of problem when two
elements share a name.

## 6. The grant is checked before anything about this browser is read

Taken from alo OS, whose `opening_the_web.rs` judges an asking application's
grant **before** reading what the machine has, so that a refusal cannot tell
an application whether a browser is installed or which one.

The same hole exists here and is closed the same way. An agent holding no
grant is refused **before** any tab is looked at, so a refusal cannot reveal
what a person has open, what they are reading, or that they are reading
anything. Ordering is the whole of this; checking the same things in the other
order leaks a person's tabs to anything that can open a socket.

## 7. The record is the browser process's, and the renderer cannot write it

A record says what was read and what changed (ADR 0052's agent gate; ADR 0012's
attribution). The browser process writes it, because it is the only part that
sees the verb and the outcome *and* the only part a page cannot reach. A
renderer that could write the record could lie about what it did.

## 8. The grant is per site, and it is ours — not anybody's grant

Item 497, and alo OS's distinction, which binds wherever this runs: a grant
their kernel cannot see is not enumerated, visible, revocable or expiring, so
**this is a refusal and not a grant**. It only ever narrows — any grant over
the application is a ceiling this cannot raise — and it is never shown to a
person as another system's grant.

## Consequences

- A new boundary file for the rented JSON reader, on `gate.sh`'s list.
- `alo-window` gains the socket; `alo-agent` gains no I/O, as ADR 0013 § 5
  requires of `alo-js`.
- The protocol is documented as a contract in `docs/contracts/`, versioned,
  additive-only — the shape alo OS's own contract takes, for the reason they
  gave.
- An agent in a separate process can meet ADR 0052's agent gate on any of the
  three systems, and on macOS first because that is where the work is.

## What this does not decide

- Which agent. Nothing here runs a model, and ADR 0052 § 6 refuses one in the
  rendering path; a model in the *test* path would break the same determinism.
- Discovery of a browser not already running, and starting one.
- alo OS's wrapper, which is theirs and thin by their own answer.
