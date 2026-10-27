# ADR 0068 — The macOS application draws with the machine's Chrome

**Status:** accepted — supersedes ADR 0067 § 2 and the renderer in § 3
**Date:** 2026-10-27
**Context:** queue items 477 and 685; ADR 0067 (the application, decided
the same morning); ADR 0058 (Chrome is a renderer we drive and never
ship); ADR 0059 (two paths, and a claim names its renderer); ADR 0061 and
0062 (Chrome draws in its own window, and the frame of ours around it);
the owner, 2026-10-27: *"Our engine should be in chrome."*

## The decision in one line

`alo browser.app` opens **the machine's own Chrome**, driven by this
browser exactly as `alo --chrome` drives it — our tree, verbs, grants,
record, socket and frame around Chrome's window — and draws with this
engine only when the Mac has no Chrome, saying so.

## Why it changes

ADR 0067 chose this engine because it is the one renderer the bundle
carries. That holds, and it was a reasonable reading of 477. But the
application is what a person is handed to see the product, and the
product path (ADR 0059) is Chrome behind our surface: the engine renders
alo's own markup today and not the web a person will point an agent at.
The owner chose the product path for the application. Everything in ADR
0067 not named below stands: the bundle's shape, its identifier, its
icon, its seal, the command that makes it, Quit, and no document types.

## 1. What it runs

Opened with nothing named, `Contents/MacOS/alo` runs as `alo --chrome`
on **the bundle's own page** (ADR 0067 § 3's page, kept): that page is
the page named, so `opening.rs`'s rule that `--chrome` needs a page is
met without a person naming one. Pages named by `open --args` or a
terminal are opened as they are today. An agent goes further than the
bundle's page the way it already can — opening a tab through the socket,
under the grants — since there is still no address bar (119).

## 2. How it is closed

Inside a bundle, `alo --chrome` **does not watch its input**: macOS
gives it `/dev/null`, which ends at once (ADR 0067 § 1, measured), and a
program closed by that closes as it opens. It is closed by what already
closes it besides its input (item 642, asserted in the window server's
numbers): Chrome's window closing, which closes the frame and the
program; the frame of ours closing, which closes Chrome; and Quit.

**Quit must leave no Chrome running.** Quit ends `alo` inside AppKit
before its destructors run (ADR 0067 § 1). Whether a Chrome driven over
`--remote-debugging-pipe` ends by itself when the pipe's far end goes is
**not measured** and is not assumed: the build measures it and, if it does
not, makes Quit close Chrome first. The closing condition asserts it in
the process table, as 642's tests assert Chrome gone.

## 3. A Mac with no Chrome

ADR 0058 holds: Chrome is never shipped, and the Mac this is copied to
may not have it. Then the application draws the bundle's page with **this
engine**, which it carries, and the page drawn says that Chrome was not
found and that this engine draws it — the same wording the socket's
`renderer` answer gives (ADR 0059 § 4). It does not download Chrome, ask
the person to install it, or look anywhere but where `chrome.rs` already
looks.

## 4. Which renderer is a person's choice later, not now

There is no switch in the application. `alo` from a terminal keeps both:
this engine by default and Chrome with `--chrome`, as today. A setting a
person changes is a first-run or settings item (479), not this.

## Consequences

- Item 477 is built as ADR 0067 says with §§ 1–3 here in place of its
  § 2, and closes on the same condition: a Mac that never built it,
  opened by double-clicking, showing the page — now drawn by Chrome where
  that Mac has one, with the journal saying which.
- Item 685 is folded into 477: Chrome from the application is how 477 is
  built. 684 (a page opened from the Finder) is unchanged and still needs
  its own ADR.
- `alo` gains: inside a bundle with no page named, `--chrome` on the
  bundle's page; inside a bundle, input not watched; no Chrome found,
  this engine on the same page, saying so.

## Amendment — 27 October 2026: how Quit is heard, measured (queue item 688)

§ 2's *Quit leaves no Chrome* was measured in iteration 427 and held:
Chrome ends when its pipe closes. What Quit **did** leave was Chrome's
private profile — 67 MB after one page — and the socket, because the
shutdown that closes Chrome, waits for it and removes them was written
after the frame's event loop, and Quit never returns from it. ADR 0067
§ 5 already said the day this browser leaves anything on a disk, Quit
runs the shutdown the window does; a profile is that day.

Item 688 named two ways: remove a dead browser's profile at the next
start, or hear Quit through an application delegate of our own, which
is `unsafe` code. **Neither is taken.** `winit` 0.30.13 is already the
application's delegate, and its `applicationWillTerminate:` ends the
event loop the way the loop would end itself, calling the handler's
`exiting` before `AppKit` ends the process. So the shutdown is handed to
the frame and run in `exiting` (`closing.rs`), once, whichever way the
program ends. Measured on macOS 26.5.2 with Chrome 156.0.8078.48: quit by
the Apple Event the Dock and ⌘Q send, the application closed Chrome and
left neither the profile nor the socket; with `exiting` not running the
shutdown, the profile was left. No `unsafe` of ours, no new call into
`winit`.

What this does not cover, and says so: a browser **killed** — `SIGKILL`,
a crash, the machine losing power — runs nothing, and leaves its profile
as before. Chrome still ends with its pipe. Removing what a dead browser
left, at the next start, is queue item 690.
