# ADR 0059 — The product path and the engine path are two roads to one browser

**Status:** accepted
**Date:** 2026-10-24
**Context:** ADR 0058, whose last line this one reverses; ADR 0005 (the
renderer boundary both paths implement); ADR 0003 (identity is allocated once
and never reused); ADR 0052 (the agent's gate and the person's gate);
`CLAUDE.md`'s standing rule *stage by what it renders*; `ROADMAP.md`;
`docs/autonomy/QUEUE.md`

## The decision in one line

The roadmap gains **a second path, stated as plainly as the first** — Chrome
behind our surface, a usable browser in weeks — and the four stage numbers go
on meaning what they have always meant: how much of the open web **our own
engine** renders. Neither path is renumbered, nothing is deleted, and no
engine item is demoted.

## Why this is a decision rather than a tidy-up

ADR 0058's final line reads: *"Whether the engine's queue changes. It does
not."* That was the right call on the day, because the ADR was already making
the largest scope decision since 0013 and bundling a queue reorganisation into
it would have hidden one inside the other.

But it left the roadmap saying something that is no longer true. The roadmap
describes **one** road — build the engine until it renders the modern web —
and a browser a person can use sits at the end of it, years away. ADR 0058
built a second road to the same place that takes weeks, and the roadmap
mentions it in one line, in a subsection about the process model. A roadmap
whose shortest route to the product is a footnote is a roadmap that will be
read wrongly by everyone including us.

So this reverses that one line, and only that line.

## 1. Why the stage numbers do not move

The obvious edit is to make the product path stage 2 and push everything down.
It was measured before it was refused: **"stage 2" appears 480 times across 72
files, 34 of them Rust source.** Every one would have to be read and decided
on, and a commit message written a month ago would silently come to mean
something else.

That is the failure ADR 0003 forbids by name — identity allocated once and
never reused. A stage number is identity. `stage 2` is the name of a body of
work that a hundred items close against; handing that name to different work
is the same mistake as reusing a `BoxId`, and it is harder to find afterwards
because nothing fails.

So the numbers stay still and the second path is named rather than numbered.
**Stage numbers describe the engine.** What the product can do while a stage is
open is stated under that stage, in its own words.

## 2. The two paths, and what each one is for

Both implement the one boundary ADR 0005 drew. That is why there are two of
them and not two products.

| | The product path | The engine path |
|---|---|---|
| What renders | The machine's own Chrome, over a pipe (ADR 0058) | `alo-render`, ours, in Rust |
| What is ours | The tree, the verbs, the grants, the record, the window | All of it |
| Reaches a usable browser in | Weeks | Years |
| What it proves | That reading an interface beats photographing it | That a browser need not be written in a language that permits the bug |
| Gate | ADR 0052's two gates — the agent's and the person's | Stage 2's exit gate, unchanged |

Neither is a fallback for the other. The product path answers *can a person
use this*, which is a question we could not previously answer this decade. The
engine path answers *can this be written safely*, which is the argument for
having started, and which Chrome does not answer and never will.

## 3. What this does to the queue

Nothing is deleted, nothing is demoted, nothing is parked. The queue gains a
section at the top naming the items that lie on the product path, and every
other item stays exactly where it is, in the order it was in, and the loop goes
on building it.

This was the owner's call on 24 October 2026 and it is recorded as one, because
the alternative — parking the rasterisation tail now that Chrome rasterises for
us — is the judgement ADR 0058 § 7 reserved for the owner and an ADR. It may
yet be right. It has not been decided, and a section of this file is not the
place to decide it by implication.

## 4. What may be said about where we are

Unchanged from ADR 0058 § 6, and worth repeating because this ADR makes it
easier to break. **A claim names its renderer.** "alo browser renders the open
web" is true on the product path and false on the engine path, and the two
paths will be at different places for years. Any statement that does not say
which one it is about is not a statement about this project.

## Consequences

- `ROADMAP.md` gains a top-level section, *The two paths*, before stage 1, and
  each stage states what the product can do while that stage is open.
- `docs/autonomy/QUEUE.md` gains a product-path section at the top. No item
  moves, no item is renumbered.
- A new queue item exists for everything the product path needs and does not
  have, rather than it being assumed to fall out of engine work.
- ADR 0058's last line is reversed; 0058 itself gains a pointer here.

## What this does not decide

- Whether any engine item is ever parked. ADR 0058 § 7 reserves that.
- Whether Chromium is ever bundled rather than borrowed. Still no.
- The order of the engine's own items, which the loop and `QUEUE.md` keep.
