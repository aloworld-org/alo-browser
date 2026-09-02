# ADR 0002 — The layout tree is the agent's tree

**Status:** accepted — the reason this engine exists rather than a faster fork
**Date:** 2026-09-02
**Context:** `alo-os` ADR 0001 (the capability model), `docs/contracts/`

## The decision in one line

An agent reads the interface as a **tree of what things are and where they are**
— produced by the same layout pass that draws them — and acts on it through
typed verbs; it never receives a screenshot, and there is never a second tree
built for its benefit.

## What was actually wrong

Every AI browser shipping today does the same thing: a chat panel beside
somebody else's page, reading it by scraping a DOM designed for a different
purpose or by photographing the screen and guessing. Both are unreliable, both
break silently when a page changes, and neither can say afterwards what it
actually operated on.

They do that because they do not own the engine. We will.

## The decision

**One tree, two readers.** The layout pass already computes what every box is,
what it contains, where it sits and what state it is in — that is what painting
needs. An agent needs the same facts. So the agent surface is a *view* of the
layout tree, not a parallel structure: if the two could disagree, the agent would
eventually act on something that is not on screen.

**Roles are declared, not inferred.** A box says it is a list, a row, a field, a
button, and whether it is selected, disabled or busy. Guessing that from
appearance is what screen-scraping already does badly.

**Acting goes through typed verbs**, exactly as in `alo-os` ADR 0001: activate
this element, put this text in that field, scroll this list. **No verb takes a
coordinate**, because a coordinate is a guess about a layout that may have
changed between the reading and the acting.

**The same tree is the accessibility tree.** A screen reader and an agent want
the identical facts, and building two would guarantee one is wrong. This is also
why the work counts twice: EN 301 549 conformance and agent capability are one
implementation.

**Reading is never watching.** The engine exposes this only when asked, and the
capability model above decides who may ask. An engine that continuously streamed
its tree would have made `alo-os` ADR 0001 §4 impossible to keep.

## Consequences

- **This constrains layout's shape from the first commit.** A layout pass that
  throws away what a box *meant* and keeps only rectangles cannot be retrofitted
  into this, which is exactly what "designed in from the first commit" is
  protecting against.
- **A rendered interface becomes automatable without automation hooks.** No test
  ids, no accessibility bolt-ons, no brittle selectors — the tree is the truth,
  and it is the same one a person sees.
- **Reference renders can assert the tree**, not just pixels. A test can say
  "row three is selected and sits at these coordinates", which is a far better
  failure message than an image diff.
- We carry a cost the alternatives do not: every layout feature must decide what
  it means semantically, not only how it draws.

## Alternatives rejected

**Expose the DOM and let agents figure it out.** Rejected: the DOM says what an
author wrote, not what is on screen. A hidden element is in it; a scrolled-away
row looks identical to a visible one.

**Screenshots and a vision model.** Rejected as the primary mechanism: it cannot
be verified afterwards, which breaks the record guarantee `alo-os` rests on, and
it fails silently the moment a layout shifts.

**Build the agent surface later, once rendering works.** Rejected — it is the
whole reason for building an engine rather than using one. Added afterwards it is
a plugin, and a plugin is what everybody else already has.
