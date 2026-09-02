# ADR 0004 — We own the layout tree, `taffy` owns the algorithms

**Status:** accepted
**Date:** 2026-09-03
**Context:** ADR 0001 (rent the physics, build the engine);
`docs/autonomy/QUEUE.md` item 15, which called this "a decision rather than a
chore"

## The decision in one line

This engine keeps its own arena of layout nodes — their styles, their caches,
their children, their results — and implements `taffy`'s tree traits over it,
rather than using `taffy`'s ready-made `TaffyTree`. The **algorithms** stay
rented; the **tree** is ours.

## What forced it

`width: calc(100% - 2rem)`.

`taffy` carries a `calc()` value as an opaque handle and asks the tree to
resolve it — `LayoutPartialTree::resolve_calc_value(handle, basis)`. It has to:
the basis is the containing block's size, and only the algorithm running knows
that. `TaffyTree`'s own implementation of that method returns `0.0`, and there
is no hook to replace it. So with the ready-made tree, every `calc()` with a
percentage in it is either a zero or a refusal, and this engine chose the
refusal — a wrong pixel is a bug, and a silent zero is a wrong pixel that looks
deliberate.

The refusal is the right behaviour for a value nobody can compute. It is the
wrong behaviour for a value that is perfectly computable and that a design
system writes every day.

## Why this is not a walk back from ADR 0001

ADR 0001 says: rent the physics, build the engine. **Flexbox, grid and block
sizing are the physics.** They are thousands of lines of specification with
decades of interoperability in them, and this repository has no interest in
rewriting them. They are still `taffy`'s, and this ADR does not touch them.

**A tree of nodes with styles, children and a cache is not physics.** It is
storage. Every engine has one, ours already has three others — the DOM, the box
tree, the layout tree — and it is the one place a browser has to be able to
answer its own questions. `taffy` is explicit that the tree is the embedder's:
the trait set exists for exactly this, and its own `TaffyTree` is documented as
a convenience.

So the line moved to where ADR 0001 already drew it. What changed is that we
noticed the ready-made tree was on the wrong side of it.

## What the alternatives cost

**Refuse `calc()` with a percentage forever.** Honest, and what the engine did
until now. But `calc(100% - 2rem)` is not an exotic value — it is how a design
system writes a full-width thing with a gutter, and alo's own sheets will
write it. Refusing it means every such rule silently falls back to an initial
value and the page is subtly wrong in a way the issue list explains and the
picture does not.

**Resolve the percentage in a second pass.** Lay out once with the property at
`auto`, read the containing block's size, resolve, lay out again. Exact when
the containing block's size does not depend on its children — which is most
block layout, and is *not* flex or grid with an auto-sized container. It would
be right almost always and quietly wrong sometimes, which is worse than being
refused: a value that is usually right is a value nobody checks.

**Fork `taffy`.** Carries every future upstream fix as a merge, for the sake of
one method. And the method is one we are *supposed* to implement.

**Replace `taffy` outright.** Rewriting flexbox and grid to gain `calc()` is
the exact trade ADR 0001 refuses.

## How the handle works, without `unsafe`

`taffy` types the handle as `*const ()` and says in its own documentation that
it "may be a pointer, index, etc." — the only constraints are that it is not
null and that its low three bits are zero.

So it is an **index**, not a pointer: the arena keeps a `Vec` of the
expressions it could not reduce to a number, and the handle is
`(index + 1) * 8` cast to `*const ()`. Casting an integer to a pointer is safe
in Rust; casting it back is safe; only dereferencing would not be, and nothing
dereferences it. Law 4 holds with nothing to declare.

An index is also the better handle on its own merits: it cannot dangle, it
survives the `Vec` reallocating, and a handle from another arena resolves to
nothing rather than to somebody else's expression — the same argument ADR 0003
makes about node identity.

## Consequences

- **`resolve_calc_value` is ours, so `calc()` with a percentage works** in
  every property that reaches layout: widths and heights, minimums and
  maximums, margins, padding, insets, gaps and grid tracks.
- **The arena is a file with one responsibility**,
  `crates/alo-layout/src/arena.rs`: the nodes `taffy` walks, and the answers to
  the questions it asks about them. `engine.rs` builds it and reads the results
  back. Both name `taffy`, and `scripts/gate.sh` checks that nothing else does.
- **Rounding is ours to skip.** `taffy`'s `round_layout` is a separate function
  over a separate trait, and this engine is sub-pixel throughout — so the trait
  is not implemented and the rounding never happens, rather than being
  configured off.
- **An upgrade of `taffy` may change the trait set.** That is a real cost and
  it is the cost of every rented crate; the compiler reports it, which is the
  best kind of breakage.
- **The measure function is no longer a closure passed in.** It is a branch in
  `compute_child_layout`, which is where `taffy` expects a leaf to be measured.
  One fewer indirection, and the borrow is easier to see.
