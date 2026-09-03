# ADR 0005 — One process per site, and a sandbox we rent

**Status:** accepted
**Date:** 2026-09-03
**Context:** ADR 0001 (our own engine in Rust); ADR 0002 (the layout tree is the
agent's tree); ADR 0003 (node identity); `ROADMAP.md` stage 2, first item;
`docs/autonomy/QUEUE.md` items 24, 25 and 29

## The decision in one line

A privileged **browser process** owns the network, the disk, the display and
the user; a **renderer process per site** owns everything that touches a page,
with almost no privilege and the platform's own sandbox around it; work crosses
between them as **typed messages in one direction**, and a renderer that dies
takes nothing with it.

## Why a memory-safe engine still needs this

This is the question worth answering first, because the obvious reading of
ADR 0001 is that we do not. Chromium's process model exists in large part
because a C++ renderer is assumed to be exploitable, and ours is not: `unsafe`
is forbidden outside a reviewed, named boundary, and law 4 means that stays
true.

It is not enough, for four reasons that survive memory safety intact:

1. **Spectre.** A speculative side channel reads memory the language says is
   unreachable, because the leak is in the hardware rather than in the program.
   No amount of Rust prevents it. **Site isolation is the only mitigation that
   works**: if a page's process never holds another site's data, there is
   nothing in reach to read. This is the argument that decides the ADR on its
   own.
2. **The physics we rent has `unsafe` in it.** TLS, image codecs, media codecs,
   font shaping — ADR 0001 says to rent them, and historically codecs are the
   single richest source of memory-safety bugs in any browser. Our forbidding
   `unsafe` does not reach inside a dependency.
3. **Logic bugs are not memory bugs.** The same-origin policy is code we will
   write, and code we write can be wrong. A process boundary is a second answer
   to "may this page see that data", enforced by something that is not us.
4. **A page must not be able to end the session.** A renderer that exhausts
   memory, spins forever, or panics should cost one tab. Crash isolation is not
   a security property and it is the one users notice.

So the model is not inherited from C++ browsers out of habit. Three of those
four reasons would apply to a perfect engine.

## What runs where

**The browser process.** The network stack, the disk, the profile, the display,
the window, and the decisions about who may do what — including the agent
capability decisions ADR 0002 defers to `alo-os`. It **never parses a page**:
not HTML, not CSS, not an image, not a script. Untrusted bytes reaching a
privileged process is the failure this whole structure exists to prevent.

**A renderer process per site.** A *site* is the scheme and the registrable
domain, so two tabs on the same site share a process and two sites never do.
The renderer parses, styles, lays out, shapes text, runs script, and paints
into a buffer the browser process shows. It has no filesystem, no network, no
ability to start another process, and no way to name anything outside itself.

**Untrusted bytes are decoded in the least privileged process that can do the
work,** and never in the browser process. For images that is the renderer,
which is already the least privileged thing we have. For media, where the
rented codec is large and the attack surface is historically the worst in any
browser, it is a utility process more restricted still — decided when item 39
arrives and there is something real to restrict.

## Which way the boundary points

**The browser process sends work; a renderer returns results.** A renderer
never makes a synchronous call back into the browser process and never waits on
one. Everything crossing is a typed message, serialised — no shared mutable
state, and the one thing shared at all is a read-only buffer of painted pixels.

This is the part that is expensive to retrofit, and it is expensive for a
reason that has nothing to do with processes: an engine written against a
synchronous, ambient, reach-anywhere API cannot be pulled apart afterwards
without rewriting everything that used it. So **queue item 25 builds the
boundary while everything still runs in one process**, and item 29 changes the
transport. The shape is the decision; the `fork` is a detail that follows it.

## What a renderer dying looks like

The tab keeps the last frame it painted and says what happened. Every other tab
is untouched, because no other tab was in that process. Reloading is the user's
to ask for; a browser that silently restarts a renderer hides a bug that
somebody needs to see.

**A renderer crash is never a browser crash.** If it ever is, that is a defect
in the boundary rather than in the page.

## What this does to the agent surface

ADR 0002 says the agent tree is a *view* of the layout tree and never a
parallel structure. That is still true — **inside the renderer**, which is
where both trees are. What crosses the boundary is a **message describing the
tree at one instant**, which is a copy, and there is no way around that: a
borrowed reference cannot cross a process.

This is where ADR 0003 pays for itself. The message carries node identity, ids
are allocated once and never reused, and so a verb sent back naming a node
either finds the same node or finds nothing. That is exactly the property that
makes acting on a description safe when the description is a moment old — and
it is the same reason ADR 0002 refuses coordinates.

**The capability decision stays in the browser process.** A renderer must never
be able to grant itself agent access to its own page, because a page is the
thing an attacker controls.

## What the alternatives cost

**One process, memory-safe, and trust it.** Cheapest, and it is what a Rust
engine is tempted into. It has no answer to Spectre, no answer to `unsafe` in a
rented codec, and no answer to a page that spins. The first hostile page is the
one that finds out.

**One process per tab.** More processes than sites, for no more isolation:
two tabs on the same site can already reach each other through `window.opener`
and the DOM, so separating them buys nothing and costs a process each.

**A language-level sandbox instead of an OS one** — an interpreter jail, or
Wasm. In-process isolation is not a boundary against a side channel, because
the data is in the same address space by construction. It is a useful second
layer and it is not the first one.

**Writing our own sandbox.** ADR 0001's rule again: the sandbox is physics.
seccomp-bpf and user namespaces on Linux, Seatbelt on macOS — the platform's
own, used the way the platform documents. A hand-rolled sandbox is a security
boundary nobody has attacked.

## Consequences

- **The protocol has to be coarse.** Serialisation costs something on every
  crossing, so the browser process asks for *a layout* or *a frame* or *the
  agent tree*, never for a box at a time. A chatty protocol is the way this
  design becomes slow, and it is easier to keep coarse than to make coarse.
- **Everything after item 25 is written against the boundary**, including the
  parts that do not need it yet. That is the discipline the whole ADR is for.
- **Tests stay single-process.** The boundary is a type, not a transport, so a
  corpus case still runs deterministically in one process — which is what keeps
  a reference render diffable.
- **N processes cost N processes.** Memory goes up and it is the price of the
  first reason in this document. A bound on how many renderers exist, and reuse
  of the ones we have, belong with item 29 rather than here.
- **The sandbox will need syscalls, and law 4 still holds.** If the platform
  crate we use does not cover something and we must write `unsafe` ourselves,
  that needs its own ADR at that time, naming the boundary and the reason. This
  ADR does not pre-authorise any of it.
