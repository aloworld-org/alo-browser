# ADR 0067 — The macOS application is this engine's programs in a bundle, opening a page of its own

**Status:** accepted
**Date:** 2026-10-27
**Context:** queue item 477 (an application somebody can run, on macOS),
and the note item 642 left on it; ADR 0024 § 1 (`alo` is the browser a
person starts, and the window is rented); ADR 0005 (one renderer process
per site, sandboxed); ADR 0055 (the agent's socket); ADR 0058 (Chrome is a
renderer we drive and never ship); ADR 0059 § 4 (a claim names its
renderer); ADR 0011 (what may be written to a disk); law 4; queue items
119, 478, 479 and 483

## The decision in one line

The application is a bundle, `alo browser.app`, holding **this engine's
own programs** — `alo`, `alo-render` beside it and `alo-ask` — signed by
nobody but sealed ad hoc, with an icon this engine drew. Opened from the
Finder it is given nothing, so it opens **a page of its own**, shipped
inside it, drawn by this engine. It is closed by its window or by Quit,
never by its input. It does not start Chrome.

## Why this is a decision rather than packaging

Because item 477 asks for two things that cannot both be true. It says the
application "opens a page named on the command line", and it closes when
the application is "opened by double-clicking, and shows a page". A
double-click names nothing. And item 642 left a note on it: `alo
--chrome` closes when its input ends, "and an application opened from the
Finder is given no input at all, so it would close as it opened". Both
were measured before anything was chosen.

## 1. What an application opened by macOS is given, measured

macOS 26.5.2 on a Mac15,13 (Apple M3). A probe bundle in the scratchpad,
outside the repository, whose program wrote down what it was started with,
opened three ways: by `open`, by the Finder (`tell application "Finder"
to open`), and with a `.html` file handed to it by `open -a`.

- **No arguments, all three ways.** A file opened with the application is
  not in its arguments: LaunchServices sends it as an Apple Event, which
  only an application delegate hears.
- **Its input is `/dev/null`.** A character device that is not a
  terminal, and a read of it ends at once.
- **Its directory is `/`**, and under the Finder its `PATH` is
  `/usr/bin:/bin:/usr/sbin:/sbin`.
- **`TMPDIR` is the person's**, the same directory a terminal has. So the
  agent's socket is placed where `alo-ask` looks for it whichever way the
  browser was started (ADR 0055 § 1).

Then the real programs, `alo` and `alo-render` from `target/debug`, copied
into a bundle of the same shape:

- **Opened by the Finder, it ran.** One window, one empty tab
  (`about:blank`, by `alo-ask`'s `tabs`), its renderer started from beside
  it and the socket placed.
- **Handed a page with `open -a … --args PAGE.html`,** it drew the page —
  a heading and a paragraph in the machine's San Francisco (item 683) — in
  a renderer under its sandbox, unchanged by being inside a bundle.
  `beside.rs`'s answer holds there with no change: `Contents/MacOS/alo`
  finds `Contents/MacOS/alo-render`.
- **`alo --chrome PAGE.html`, opened by `open -W`, had ended two seconds
  later**, with no Chrome left running. Its input ended at once, as 642
  said it would.
- **Quit** (`tell application … to quit`, what ⌘Q and the Dock send)
  ended `alo` and both its renderers within four seconds. It ends the
  process inside AppKit, before `alo` returns from its window: the socket
  was left at its path. A browser started after removes a socket nobody
  answers at (`socket_place.rs`), so it is a file left over, not a place
  taken.
- **Unsealed, the bundle fails `codesign --verify`** ("code has no
  resources but signature indicates they must be present"): `ld` signs
  each program ad hoc and the bundle around them is signed by nothing.
  Signed ad hoc as a whole (`codesign -s -`) it is "valid on disk" and
  "satisfies its Designated Requirement"; `spctl -a` rejects it either
  way, since nobody's identity is in it.

## 2. It holds this engine, and not Chrome

The application starts this engine's renderer, because that is **the one
renderer it carries**. ADR 0058 says we never ship Chrome: on the product
path the browser drives the Chrome a person already has, and the Mac this
is copied to — 477 closes on one that never built it — may not have one.
What a person double-clicks has to work on any Mac it runs on.

And Chrome through the application has nothing to show and nothing to
close it. It needs a page named (`opening.rs` refuses `--chrome` without
one), and a double-click names none; it closes when its input ends, and an
application's input has ended before it starts. Making it work means
deciding how a person names a page to it, which is the address bar (119)
or a file opened from the Finder (§ 4), and how it is closed by its windows
alone. Neither is packaging, so neither is decided here: Chrome from the
application is a queue item of its own.

**Nothing is taken from the product path by this.** `alo --chrome` stays
what it is, run from a terminal — including as the bundle's own program,
`Contents/MacOS/alo --chrome`. And ADR 0059 § 4 binds what may be said:
the application draws with this engine, so what it renders is what the
engine renders, and a person is told which (`alo-ask`'s `renderer`).

## 3. What it opens: a page of its own

Given no page, `alo` opens an empty tab, and an empty tab is not "shows a
page". So the bundle carries one, in `Contents/Resources`, and `alo`
started from inside a bundle with no page named opens it. Named pages,
from `open --args` or a terminal, are opened as they always were.

The page is **markup we wrote**, which is stage 1's ground: no script, no
fetch of anything outside the bundle, no link off the machine. It says
what this is — alo browser, this engine drawing it — and what it is not
yet: there is no address bar (119), so a person cannot go anywhere from
it, and an agent reaches it through `alo-ask`, which the bundle carries.
It is **not a first run** (479): it asks nothing and grants nothing, and
it is the same page every time. Its render is committed and compared, like
any page this engine draws.

How `alo` knows it is in a bundle is by where it is: a program at
`NAME.app/Contents/MacOS/alo` whose `Contents/Resources` holds the page,
found the way `beside.rs` finds the renderer, never from the environment
or the working directory (which is `/`).

## 4. Opening a file from the Finder is not part of this

`application:openURLs:` is heard only by an application delegate, which
`winit` 0.30 leaves to the program to register through `objc2` — `unsafe`
code, and law 4 forbids that outside a reviewed boundary with an ADR of its
own. So the bundle **declares no document types**: an application that
said it opens `.html` and then showed its own page instead would be lying
to the Finder. A page opened from the Finder is a queue item of its own,
needing that ADR, and it is the second way (with 119) a person could name
a page for Chrome to draw.

## 5. How it is closed

By its window — closing it closes every tab and stops every renderer, as
`alo` always has — and by Quit, which ends the process at once (§ 1). This
browser writes nothing to a disk that a quit could leave half-written
today: its pool's cache and record are in memory and go with the process
(ADR 0011, ADR 0012 § 6). **The day it does** — storage (ADR 0025) on a
disk — Quit has to run the shutdown the window does, and that item says
so.

Its input is not watched. `alo` never watched it; only `--chrome` does.

## 6. What the bundle is

- `Contents/MacOS/alo` — the program the application runs, `CFBundleExecutable`.
- `Contents/MacOS/alo-render` beside it, where `beside.rs` looks.
- `Contents/MacOS/alo-ask`, the smallest caller of the agent's socket, so
  the Mac it is copied to can drive it without a checkout.
- `Contents/Resources/` — the page of § 3 and the icon.
- `Contents/Info.plist` — `CFBundleIdentifier` **`org.aloworld.Browser`**,
  the identifier the owner chose for 483; `CFBundleName` and
  `CFBundleDisplayName` "alo browser"; `CFBundlePackageType` `APPL`; the
  version, `CFBundleShortVersionString` and `CFBundleVersion`, both the
  workspace's own (`0.0.0` today, said as it is); `CFBundleIconFile`; and
  no `CFBundleDocumentTypes` (§ 4).
- **The fonts are the machine's.** Every Mac has its own San Francisco, New
  York and Menlo, which `alo` uses since 683; `--frozen-fonts`' DejaVu
  faces are in the program already. The bundle carries no font file.
- **The icon is drawn by this engine**, from a page in this repository, at
  the sizes an `.icns` holds, and those pictures are committed references
  compared by the gate like any other render. The `.icns` holds them as
  PNG, which is a container a few dozen lines of ours write; Apple's
  `iconutil` is not needed.
- **It is sealed ad hoc** with `codesign -s -`, Apple's own tool, rented
  as `screencapture` is in the tests. That is what makes it valid on disk
  (§ 1) — not trusted: `spctl` rejects it, and a copy macOS has marked as
  downloaded asks the person to allow it in System Settings once. A
  Developer ID and notarisation are 478's, which costs an account.
- **It is built for the machine that builds it** — `aarch64-apple-darwin`
  here. A universal program is a later item, opened when an Intel Mac is
  the one it is copied to.
- **It is made by a documented command, in Rust** (law 4): a program of
  its own that assembles the bundle from the release programs built
  beside it, signs it, and checks it with `codesign --verify`. The
  programs are built by `cargo build --release` first; the command says so
  rather than calling `cargo` itself.

## Consequences

- Item 477 is built as this says, and still closes only on a Mac that
  never built it, opened by double-clicking, showing the page — recorded in
  the journal with that machine named. This machine alone does not close
  it.
- `alo` gains one rule: inside a bundle, with no page named, it opens the
  bundle's page.
- Two queue items are written: a page opened from the Finder (§ 4, needs
  an ADR for its `unsafe` boundary), and Chrome from the application (§ 2,
  depending on 119 or that one, and on being closed by windows alone).
- 642's note on 477 is answered: the application is not closed by its
  input, because it does not run what watches it.

## What this does not decide

- Whether the application ever starts Chrome, and how; its own item.
- What a first run says (479), or what an agent may do before it is
  granted anything (93).
- Signing with a Developer ID and notarising (478).
- How alo OS installs it (483), which is a Flatpak, not a bundle.
- A universal program, an installer image (`.dmg`), or updates (146).
