# ADR 0077 — alo is a standalone browser that carries its own Chromium

**Status:** accepted — chosen by the owner on 30 October 2026; built by
queue items 719–722
**Date:** 2026-10-30
**Context:** ADR 0058 (Chromium is a renderer we drive — *"Chrome is never
shipped"*, the machine's own Chrome is used); ADR 0059 (the product path);
ADR 0067 and 0068 (the macOS application, drawing with the machine's
Chrome); ADR 0073 (Chrome started with its own traffic switched off); ADR
0076 (a page in the conversation drawn from Chrome's frames)

## The decision in one line

alo carries **its own Chromium inside the application** and draws pages with
it, so a person installs alo and nothing else. **The application never uses
the machine's Chrome, even when one is installed:** it behaves as it would
on a computer where alo is the only browser. If its own Chromium is missing
or damaged it says so and draws with this engine, as ADR 0068 § 3 does on a
Mac with no Chrome — never with somebody else's browser. Everything else ADR 0058 decided stays: Chromium is a
renderer we drive over a pipe, behind our tree, our verbs, our grants and
our record, and it is never linked into our process.

## Why the owner changed it

ADR 0058 chose the machine's Chrome so that this repository would carry no
browser of anybody else's and no duty to keep one patched. That made alo
something a person could only use if they already used Chrome. Every
browser built on Chromium that people choose — Brave, Edge, Opera, Vivaldi,
Arc — carries its own; none asks for Chrome. The owner's words: *"We should
be a stand alone browser that uses chromium."*

## What it costs, said plainly

1. **Security updates are ours.** Chromium has security releases every week
   or two between its four-weekly stable releases. A browser that carries one
   must ship them within days, so alo needs an updater (721). Until it has
   one, alo must not be given to people outside the owner's own machines.
2. **Size.** The application grows by a few hundred megabytes.
3. **Signing.** The Chromium inside alo is signed and notarized as part of
   alo, with the owner's Apple identity.
4. **Its name.** A stock Chromium build names itself *Chromium* in the menu
   bar when a page is expanded into its own window (ADR 0076). Carrying alo's
   name means building Chromium from source with our branding (722), which
   needs build machines this repository does not have.
5. **Licences.** Chromium is BSD-licensed with many third-party notices; they
   ship with alo.

What does **not** change: proprietary codecs and DRM stay out (`ROADMAP.md`,
*Not built, and not by accident*) — an open-source Chromium build has
neither, which agrees with that list.

## How it is built, in order

| Item | What |
|---|---|
| 719 | the application starts only the Chromium inside its own bundle — never the machine's Chrome — and says which Chromium it is using |
| 720 | Which Chromium build, from where: pinned by version and checked by its hash when `alo-bundle` assembles the application, with its licences; the source is decided in this item's ADR |
| 721 | An updater that brings a new Chromium (and alo) to a person within days of a security release |
| 722 | Chromium built from source with alo's name and icon — blocked on build machines |

## What this does not decide

- Which Chromium build is carried (720).
- How the updater works or where it fetches from (721).
- Whether the engine path (ADR 0059) ever replaces Chromium; that is the
  stages' business.

## Consequences

- ADR 0058's *"Chrome is never shipped"* and ADR 0068's *"draws with the
  machine's Chrome"* are superseded by this for the application, at the
  owner's word: *no fall back — as if the user only has alo-browser on his
  PC*. `alo --chrome` run from a terminal is a developer's tool, not the
  application, and keeps driving the machine's Chrome for this
  repository's tests.
- Until 720 puts a Chromium in the application, the application draws with
  this engine and says so.
- Nothing is given to anyone outside the owner's machines before 721.
