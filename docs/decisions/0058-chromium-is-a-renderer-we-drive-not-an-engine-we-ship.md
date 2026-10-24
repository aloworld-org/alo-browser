# ADR 0058 — Chromium is a renderer we drive, not an engine we ship

**Status:** accepted
**Date:** 2026-10-24
**Context:** ADR 0001, which refused Chromium and is answered directly below;
ADR 0005, whose message boundary this uses rather than invents; ADR 0010 (the
sandbox); ADR 0055 (the agent socket); `CLAUDE.md` laws 1, 2 and 4;
`docs/contracts/agent-socket.md`

## The decision in one line

The browser process may spawn **the Chrome already installed on the machine**
and drive it over a **pipe**, as a second implementation of the renderer
boundary ADR 0005 already drew — so that this browser renders the open web
today, while `alo-render` remains the other implementation of the same
boundary and the destination.

## Why this is a decision rather than a chore

Because ADR 0001 refused Chromium by name, and a repository that quietly does
what its founding decision forbade is a repository whose decisions mean
nothing. This is either a genuine distinction or it is a reversal, and it has
to be argued either way.

It is also the largest scope decision since ADR 0013. Everything about what
this product can do in the next year turns on it.

## 1. What ADR 0001 refused, and whether this is that

Its words: *"It works, and it is Google's C++ with a permanent stream of
memory-safety vulnerabilities. A product sold on sovereignty and auditability
that **ships** the world's largest C++ attack surface has an argument it cannot
finish."*

**Ships** is the hinge, and it is doing real work.

- We do **not** ship Chromium. Nothing is vendored, bundled, linked or
  redistributed. We spawn the browser a person already installed and updates
  themselves.
- We do **not** embed it. There is no FFI, no `unsafe`, no C++ in anything
  this repository builds. `unsafe_code = "forbid"` stands across the whole
  workspace, unchanged. Law 4 survives intact for every line we are
  responsible for.
- The attack surface is a **separate process**, in Chrome's own sandbox,
  patched by Google within days of a flaw being found — which is a better
  security position than a fork of it, and a better one than most of what is
  on a person's machine already.

**And the honest half.** We would nonetheless *depend* on that surface. A
person running this runs Chrome, and a page that exploits Chrome compromises
them, whoever shipped it. ADR 0001's argument is weakened by the word *ships*;
it is not dissolved. What changes is who answers for it and how fast: Google,
in days, for free — rather than us, eventually, for ever.

So this is not a reversal of ADR 0001. It is ADR 0001 holding for the
**engine**, which remains ours and remains the destination, while the
**renderer slot** is filled by something rented until the engine can fill it.

## 2. It sits behind a boundary that already exists

ADR 0005 put a typed message boundary between the browser process and a
renderer, so that *"a page which finds a way to take over the process it is
rendered in has taken over a process that holds one site's pages and nothing
else."*

Chromium is a different thing on the far side of that boundary. `ToRenderer`
and `FromRenderer` do not change. What changes is that one implementation
translates them into our renderer's work and the other translates them into
CDP.

This is the reason this decision is cheap: the seam was drawn two months ago
for a different reason and it fits.

## 3. A pipe, never a port

Chrome's DevTools protocol normally listens on a **TCP port**. ADR 0055
refused a port for our own socket, for the reason that applies here
identically: a page's own script can reach `localhost`, and a port is a door
into the browser showing that page.

`--remote-debugging-pipe` carries CDP over the spawned process's own
descriptors. No port, reachable by the parent and by nothing else.

## 4. The tree, the grants and the record do not move

They stay in the browser process, which is ours, in safe Rust, on this side of
the pipe:

- the **agent socket**, its verbs and its twelve refusals (ADR 0055);
- the **grants** — a site an agent was not given is refused *here*, before a
  single CDP command is written (item 497);
- the **record** — written where no page can reach, because Chromium is a
  process that cannot see it.

What Chromium supplies is a rendered page and a description of it. The tree an
agent reads is mapped from `Accessibility.getFullAXTree`, which Blink builds
from its layout objects — a weaker thing than our own tree, and named as
weaker in § 6.

## 5. Never forked, and never frozen

**Never forked.** Chrome ships security fixes every few weeks, often for flaws
already being exploited. A fork that stops following upstream is, within
months, a product with hundreds of published holes. Every organisation that
tried this maintains a small patch set instead, with engineers whose whole job
it is.

**Never frozen.** We use the machine's Chrome and it updates itself. If a
capability ever proves impossible over CDP, the escalation is a patch set
rebased each release — recorded in an ADR of its own, never drifted into.

## 6. What may be said, and what may not

This is the clause most likely to be broken by somebody in a hurry.

**May be said, because it is true today:** it acts by name and never by
coordinate; it refuses by kind; it keeps to the sites a person named, enforced
by the browser; and it writes a record the page cannot reach.

**May not be said while Chromium is behind it:** that this browser is
memory-safe, that it is independent of Google, or that it runs alo's own
engine. Those sentences belong to the engine. They become true the day it is
behind the boundary and not one day sooner — not in a deck, not in a footnote.

**And somewhere a person can read, the product says what is rendering.** A
product whose engine is a secret is a product whose claims cannot be checked,
and checkability is the whole of what this is for.

## 7. The engine is not abandoned, and this ADR is not a quiet way of doing so

`alo-render` remains the other implementation of this boundary and remains the
destination. The loop keeps building it. Its advantage is not rendering — it
is that the classes of bug behind most browser vulnerabilities **cannot be
written** in it, and there is no JIT for anyone to exploit.

If that work is ever to stop, it stops in an ADR that says so, with reasons,
signed by the owner. Not by neglect, and not by this.

## Consequences

- A new renderer implementation in Rust, speaking CDP over a pipe. No new
  rented crate is needed: `serde_json` is already behind its boundary file.
- `beside.rs`, which decides where `alo-render` is, gains the question of
  where Chrome is.
- The product renders the open web in weeks rather than years.
- `docs/features.md` and the roadmap must say which renderer is in use, per
  § 6.

## What this does not decide

- Whether to bundle Chromium rather than use the machine's. (No, for now:
  bundling means owning the patch cadence.)
- Anything about Firefox, which alo OS pins for its own reasons.
- ~~Whether the engine's queue changes. It does not.~~ **Reversed by ADR
  0059 (2026-10-24):** the queue's *order* is untouched and no item is
  demoted, but the roadmap gains the product path as a named road beside
  the stages, and the queue names the items on it. Left here struck through
  rather than deleted, because what an ADR said on the day is the record.
