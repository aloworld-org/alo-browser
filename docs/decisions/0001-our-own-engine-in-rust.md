# ADR 0001 — Our own engine, in Rust, staged by what it renders

**Status:** accepted
**Date:** 2026-09-02
**Context:** `alo-os` ADR 0002 (the shell is native) and its `docs/features.md`;
`alo-workplace`'s design system

## The decision in one line

We write our own rendering engine in **Rust**, staged by **what it must render**
rather than by specification completeness — stage 1 renders alo's own interface,
which needs no JavaScript engine and no compatibility with the legacy web at all.

## Why not rent one

Every other option was considered and each fails on something we are not willing
to give up.

**Chromium.** It works, and it is Google's C++ with a permanent stream of
memory-safety vulnerabilities. A product sold on sovereignty and auditability
that ships the world's largest C++ attack surface has an argument it cannot
finish.

**WebKitGTK**, which is what an embedded webview on Linux actually means: the
weakest engine available for performance and GPU acceleration, as the foundation
of a session.

**Servo.** Rust, and genuinely aligned — but aimed at the whole web since 2012
and still not a daily driver. We take its parts (below) rather than its scope.

**Ladybird.** Admirable, BSD, and C++ moving toward a memory-safe successor that
is not Rust. Adopting it means two more languages in a repository whose second
argument is memory safety.

## What makes this possible

**Chromium is around 35 million lines, and most of it is not capability.** It is
compatibility with thirty years of broken pages: the HTML parser's error
recovery, quirks mode, floats and CSS-table layout, the accreted DOM surface,
vendor-prefix archaeology. If you do not have to render the 2003 web, an
enormous amount of that evaporates.

What a modern interface actually needs is a much shorter list: a box model,
flexbox and grid, custom properties, text layout, transforms, compositing, input
and a retained tree. That is a serious project. It is not an impossible one.

**And stage 1 needs no JavaScript engine**, which is the single largest component
of a browser. alo's own interface can be driven from Rust; JavaScript becomes
necessary only when we go after the general web, and by then we will know far
more than we do now.

## The staging

**Stage 1 — render alo.** Markup we write ourselves. No hostile pages, no
compatibility burden, no scripting. It ships as the renderer for alo OS's shell
and workspace, so it is in daily production use while everything else is still
being built. The measure is a real alo screen rendering correctly, not a
conformance percentage.

**Stage 2 — render the modern web.** A JavaScript engine, the network stack, and
the process and sandbox model. That last one must be designed **before** the
engine ever loads a hostile page: one process per site, renderers with almost no
privilege. Every browser that retrofitted it suffered for years.

**Stage 3 — the rest.** The legacy tail, scheduled by real pages failing rather
than by a specification listing a property. Keep a corpus of sites people
actually use and let a broken render schedule the work. This is how a project
converges instead of drowning.

## What we take, and what we write

**Take:** `html5ever` (HTML parsing to specification), `cssparser` and
`selectors` (CSS tokenising and selector matching), HarfBuzz for text shaping,
font rasterisation, image and video codecs, `rustls`. Nobody writes their own
shaper — Chromium, Firefox, Servo and Ladybird all rent that one, and not out of
timidity.

**Judgement calls, taken initially and replaceable:** `taffy` for flexbox and
grid, `stylo` for style resolution. These are real chunks of engine rather than
physics. Starting on them gets us rendering far sooner; writing them is more of
the thing that is actually ours. Keep the boundary clean and replace them when we
have an opinion they do not serve.

**Write:** the DOM, the box tree, layout, painting, the process model, and the
agent surface (ADR 0002).

## Consequences

- **The repository is Apache-2.0**, not AGPL like the workspace. An engine you
  want embedded and contributed to cannot be copyleft-heavy — Ladybird and Servo
  both chose permissive deliberately, and they were right.
- **`alo-os` does not depend on this.** Its ADR 0002 says the shell is native
  whether or not the engine is ever built, and that stays true: this replaces the
  workspace's rendering when it earns it, module by module.
- **A European, memory-safe, independent engine is fundable** — Sovereign Tech
  Fund, NLnet/NGI Zero and the EU sovereignty programmes fund precisely this. The
  scope being ambitious helps rather than hurts there.
- We will be slower than a fork for a long time, and the first year produces
  something only alo can use. That is the trade, taken deliberately.

## Alternatives rejected

**Embed V8 through `rusty_v8` to get JavaScript years earlier.** Rejected: it is
Google's C++ inside a project whose claim is a memory-safe independent engine.
The saving is real and the cost is the argument.

**Aim at the open web from the start**, as Servo did. Rejected: it is the
decision that keeps a project unusable for a decade. Ladybird began as the
renderer for one hobby operating system and became a browser by being good
first — that is the path.

**Build a UI toolkit instead and call it done.** Rejected as the *ceiling*, kept
as the honest description of stage 1. The difference is that the architecture
leaves stage 2 possible rather than foreclosing it.
