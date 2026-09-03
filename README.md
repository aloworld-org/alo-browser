# alo browser

**A browser in Rust, whose first job is to render alo.**

A browser is where this goes. It starts as the engine behind alo — the alo OS
shell and the alo workspace — because that is the only way a project like this
is ever usable before year seven: something real depends on it from the first
month, and the compatibility burden arrives last rather than first. It is written in Rust, it is memory-safe by
construction, and its layout tree is the same tree an agent reads, so an
agent operates on what the interface *is* rather than on a photograph of it.

**Status: nothing works yet.** This repository holds the decisions and the
first queue of work. See `ROADMAP.md`.

## Why build this

Chromium is around 35 million lines, and most of that is not capability — it
is compatibility with thirty years of broken pages. If you do not have to
render the 2003 web, an enormous amount of it evaporates: the parser's error
recovery, quirks mode, floats and table layout, the accreted DOM surface.

What a modern interface needs is a shorter list — a box model, flexbox and
grid, custom properties, text, transforms, compositing, input, and a retained
tree. That is a serious project, not an impossible one. And **stage 1 needs no
JavaScript engine at all**, which is the largest single component of a browser.

Two things follow that no existing engine offers:

- **Memory safety**, in a product whose other half is sold on auditability.
- **An agent that reads the interface as a tree** — "invoice list, twelve rows,
  row three selected" — and acts on it through typed verbs, never coordinates.
  Every AI browser shipping today scrapes or photographs somebody else's page,
  because none of them owns the engine.

## What it is not

- **Not a web browser yet**, and possibly not for years. Stage 1 renders alo.
- **Not a Chromium fork**, and not a Ladybird fork — that one is C++ moving to
  a memory-safe language that is not Rust.
- **Not a shaper, a codec or a TLS stack.** We rent the physics. Nobody writes
  their own text shaper, and not out of timidity.
- **Not a dependency of alo OS.** Its ADR 0002 makes the shell native whether or
  not this is ever finished.
- **And alo OS is not a dependency of this.** The independence runs both ways —
  easy to write down once and then quietly lose, which is what happened to the
  stage 1 exit gate until it was corrected. Stage 1 needs no compositor, no
  certified machine, no GPU and no network: HTML and CSS in, a PNG and a box
  tree out, both files. Anybody who clones this repository can build *and verify*
  all of it on an ordinary laptop — which is how most of it was built.

## The stages

| | |
|---|---|
| **1** | Renders alo. No scripting, no hostile pages, no compatibility burden. Ships as alo OS's renderer. |
| **2** | Renders the modern web. A JavaScript engine, the network stack, and a process/sandbox model designed **before** it ever loads a hostile page. |
| **3** | The legacy tail, scheduled by real pages failing rather than by a specification. |

## Licence

**MPL-2.0** (ADR 0009). An engine you want embedded and contributed to cannot be
copyleft-heavy, and this is not: MPL is *file-level* copyleft, so anyone may
embed this engine in a closed product. What they may not do is improve the
engine in private and ship a better version of our own work against us —
changes to these files come back.

It was Apache-2.0 until somebody asked the obvious question: that licence let
anyone take the agent tree, close it, and sell it. The permissive half of the
reasoning was right and is kept; the part that gave away the one genuinely novel
thing in here was not.

The alo workspace is AGPL-3.0 and alo OS is GPL-3.0; this is still the one meant
to be used by other people, and Servo — whose parser and selector engine we
rent — chose the same licence for the same reason.
