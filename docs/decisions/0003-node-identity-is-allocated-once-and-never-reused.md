# ADR 0003 — Node identity is allocated once and never reused

**Status:** accepted
**Date:** 2026-09-02
**Context:** ADR 0002 (the layout tree is the agent's tree); `docs/features.md`
§ The document, "a stable identity per node, from the first commit"

## The decision in one line

Every node gets a `NodeId` when it is created, ids are handed out in creation
order and **never reused**, and a node that leaves the tree keeps the one it
has — so a stale id names a node that is gone rather than a different node that
is not.

## Why this is decided now rather than later

`docs/autonomy/QUEUE.md` item 1 is blunt about it: "adding identity afterwards
means rewriting everything that holds a reference." That is not a guess about
the future. ADR 0002 says an agent reads the interface as a tree and then acts
on it through typed verbs, which means there is always a gap between *naming* a
thing and *doing* something to it. Something has to name the thing across that
gap, and it cannot be a coordinate, because ADR 0002 forbids that for exactly
the same reason.

So identity is the thing the agent surface stands on, and it has to exist
before the tree does.

## What the alternatives cost

**A pointer or a reference.** Cannot cross the gap: a handle held across a
reparse is a handle to a tree that is gone. It also drags a lifetime through
every type that touches the DOM, style, layout and paint — which is how a Rust
engine ends up with an arena and index handles anyway, two rewrites later.

**A reused slot, as a generational arena or a free list does.** This is the
tempting one, because a browser detaches a great many nodes and the memory
looks free. It is refused: the whole value of identity here is that an id that
no longer names anything says so. A recycled id answers a stale question with a
confident wrong node, and that failure is silent at every layer above it.

**A hash of the node's content or its path.** Two identical rows in a list hash
the same, and a list of identical-looking rows is precisely what an agent is
asked to act on. A path changes when a sibling is inserted, which is the moment
identity matters most.

## Consequences

- **A detached node's slot is not freed.** A document holds every node it ever
  created, so peak memory follows total nodes created rather than nodes
  currently attached. Bounded by the input, and the parser detaches only during
  error recovery and the adoption agency. If a real document ever makes this
  expensive, the fix is a compaction pass that rewrites ids as one explicit
  step — not a free list that recycles them quietly.
- **`Document::is_attached` is the question to ask**, not "does this id
  resolve". Both are answerable, and they answer different things.
- **An id is meaningful only in the document that minted it.** Passing one to
  another document returns `None` rather than another document's node, which is
  a wrong answer this design can actually give.
- **Identity across a reload is a separate problem**, and this ADR does not
  solve it. When the agent surface needs to say "the same row as before" across
  a reparse, that is a mapping built on top of these ids, and it will need its
  own decision.
