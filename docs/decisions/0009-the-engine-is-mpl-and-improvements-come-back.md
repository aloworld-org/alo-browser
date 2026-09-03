# ADR 0009 — The engine is MPL-2.0, so improvements come back

**Status:** accepted — relicences this repository from Apache-2.0
**Date:** 2026-09-03
**Context:** `LICENSE`, `README.md`, `Cargo.toml`,
[ADR 0002](0002-the-layout-tree-is-the-agents-tree.md) (the agent tree, which is
the thing being protected), `alo-os` GPL-3.0, `alo-workplace` AGPL-3.0

## The decision in one line

alo browser is **MPL-2.0**: anyone may embed this engine in a closed product,
and anyone who improves *the engine itself* publishes those improvements —
which keeps the whole of the original reasoning for being permissive while
closing the one hole it left open.

## What was actually wrong

The Apache-2.0 choice had a good argument behind it, recorded in `README.md`:
*an engine you want embedded and contributed to cannot be copyleft-heavy*. That
is true. Blink is BSD, WebKit is LGPL, Ladybird is BSD, and an engine nobody can
embed is an engine nobody adopts.

The argument was right about embedding and wrong about everything else, because
Apache-2.0 does not only permit embedding. **It permits a competitor to take
this engine, close their changes, and sell it** — including
[ADR 0002](0002-the-layout-tree-is-the-agents-tree.md)'s agent tree, which is
the one genuinely novel thing in this repository and the entire reason it exists
rather than a fork of somebody else's engine.

It also quietly falsified a claim made elsewhere: that a competitor wanting
alo's sovereignty story *"has to build an operating system and a browser
engine"*. Under Apache-2.0 they would not have had to build the engine. They
could have taken this one.

**This is not a general retreat from permissive licensing.** Every rented
component here keeps its own licence, the non-goals still refuse to write a
shaper or a codec, and the engine is still meant to be used by other people.
What changed is the recognition that *embeddable* and *enclosable* are two
different properties, and only the first one was ever wanted.

## The decision

**MPL-2.0**, which is file-level copyleft:

- **Embedding stays free.** A proprietary application may ship this engine, link
  it, and distribute it without opening its own source. That is what MPL was
  written for and it is precisely the property the original argument wanted.
- **Improvements to the engine come back.** If somebody modifies a file in this
  repository, those files must be published under the same licence. They cannot
  build a better alo browser in private and compete with us using our own work.
- **It stays GPL-compatible**, so `alo-os` — GPL-3.0 — can keep using it with no
  friction, and so can anyone else's copyleft project.

**Servo chose the same licence for the same reason**, which is worth noting
because we rent `cssparser`, `selectors` and `html5ever` from that lineage. Our
engine now carries the licence its own dependencies carry.

## Why not the alternatives

**Stay on Apache-2.0.** Rejected: it gives away the agent tree, which is the
asset. The adoption argument for it is real, and MPL keeps almost all of that
adoption — the only people it deters are the ones who wanted to enclose the
engine, and deterring them is the point.

**LGPL-3.0.** Rejected on Rust rather than on principle: LGPL assumes dynamic
linking as the boundary between "using" and "modifying", and Rust links
statically. The obligations become unclear at exactly the moment somebody needs
them to be clear, which makes it a worse licence *for adoption* than MPL despite
being no stronger in practice.

**GPL-3.0.** Rejected: it kills embedding outright, which was the correct half
of the original reasoning and is not being abandoned.

**Dual licence — copyleft plus a paid exception.** Rejected for now. It is a
real revenue model and it works when one party holds the copyright, which is
currently true here. But it makes contribution awkward (every contributor must
assign or licence to us), and this engine needs contributors far more than it
needs licence revenue at this stage. Worth revisiting only if somebody actually
asks to buy an exception.

## Consequences

- `LICENSE` is the MPL-2.0 text, and `Cargo.toml`'s workspace `license` field
  reads `MPL-2.0`; every crate inherits it through `license.workspace = true`.
- **Per-file Exhibit A headers were owed, and are attached** (queue item 159).
  MPL asks for its notice in each file, and permits a `LICENSE` in a location a
  recipient would look when that is not practical. The root `LICENSE` satisfied
  the licence meanwhile; the headers are the answer to the question a recipient
  who has only one file cannot otherwise ask, which is what *file-level*
  copyleft makes worth answering. `scripts/gate.sh` fails on a source file
  without one, because a header nothing checks is a header that stops being
  true the first time somebody adds a file.
- **The trademark is separate and still worth taking.** No open licence lets
  somebody call their fork "alo browser". Chromium is to Chrome as this is to
  whatever a fork would have to call itself.
- Nothing about the engineering changes. This is a decision about who may do
  what with the code, not about what the code does.

## The timing, which is most of why this happened now

**Relicensing is cheap today and becomes nearly impossible later.** Every commit
in this repository has one author, so the copyright is held by one person and
the change is a file swap. Once outside contributors arrive, every one of them
must agree, and a single unreachable contributor can freeze the licence
permanently. The difference between doing this now and in two years is the
difference between an afternoon and never.
