# ADR 0062 — Our frame floats beside Chrome's window, and stands aside when the screen does not show it

**Status:** accepted
**Date:** 2026-10-24
**Context:** ADR 0061 (Chrome draws the page in its own window, and we hold
the frame around it), whose § 2 accepted two windows and left how they are
made to feel like one to the item that builds it; ADR 0024 § 1 (the window
is rented, and every call into it is checked safe); law 4; queue items 642
and 643

## The decision in one line

The window of ours that goes around Chrome's is a **floating** window — it
stays above ordinary windows — put beside Chrome's by **Chrome's word** for
where its window is, and **shown only while the window server lists
Chrome's window on the screen at those bounds**; when it does not, ours
stands aside until it does.

## Why this is a decision rather than an implementation detail

Because the obvious window — an ordinary one, like the engine's — measured
as one that a person never sees beside Chrome's at all. The level a window
is at decides whether it stays above every other application, and a window
that floats over a person's other work is a cost they did not ask for. That
is not something a commit that is mostly code should settle.

## 1. What Stage Manager does to two applications' windows, measured

Chrome 155.0.8059.86, macOS 26.5.2, Stage Manager on (`GloballyEnabled 1`,
`AppWindowGroupingBehavior 1`), a shown Chrome in `--app` mode placed by
`chrome_window::place` at 120,100 and 1200×832, and a borderless window of
ours made by `winit` 0.30.13 at 120,60 and 1200×40 — a strip directly above
it. Read from `CGWindowListCopyWindowInfo`, on-screen windows only, a
second or two after each step. A temporary probe, removed again.

**An ordinary window (layer 0):**

| step | ours | Chrome's |
|---|---|---|
| ours alone | 120,60 1200×40 | — |
| Chrome placed and brought in front | **15,689 164×29** | 120,100 1200×832 |
| Terminal brought in front | 15,679 164×27 | 15,719 116×137 |
| Chrome brought in front again | **15,689 164×29** | 120,100 1200×832 |
| ours brought in front | 120,60 1200×40 | **15,644 116×130** |

The two windows are **never on the screen together**. Stage Manager shows
one application's windows at a time, and two programs are two
applications: whichever is in front, the other is a thumbnail in the strip.
A frame of this kind would follow a window nobody can see, or be seen in
place of it.

**A floating window (`WindowLevel::AlwaysOnTop`, layer 3):**

| step | ours | Chrome's |
|---|---|---|
| Chrome placed and brought in front | 120,60 1200×40 | 120,100 1200×832 |
| Terminal brought in front | 120,60 1200×40 | **15,759 116×141** |
| Chrome brought in front again | 120,60 1200×40 | 120,100 1200×832 |

Ours stays where it was put, and Chrome's stays beside it while Chrome is in
front — in each of five runs, and the ordinary window was in the strip in
each of its two. **But when another application comes in
front, Chrome's goes to the strip and ours is left floating alone**, over
whatever that application shows. That is the cost, and § 3 is what pays it.

## 2. Chrome cannot say when its window is a thumbnail

The page in the probe wrote `document.visibilityState` into its own title,
which the window server lists beside the window's bounds. With Chrome's
window shrunk into the strip, the title still read **`visible`**: Stage
Manager keeps drawing the thumbnail live, and Chrome's page is not told it
is hidden. ADR 0061's addendum already measured that
`Browser.getWindowForTarget` still gives the bounds the window was put at.
So nothing over the pipe says whether a person can see Chrome's window.
**Only the window server knows.**

## 3. So ours is shown by the window server's word

The frame is put by Chrome's word, read over the pipe — it is exact while
Chrome's window is in front (ADR 0061 § 2), it costs a message, and it is
the same reading `chrome_window::placed` already makes. Whether it is shown
at all is the window server's: while Chrome's own window is listed on the
screen at the bounds Chrome gives, ours is shown beside it; while it is
not — in the strip, minimised, behind another application, closed — ours
stands aside. A frame that floats over somebody else's work with nothing of
ours under it is the failure § 1 found, and this is the rule that refuses
it.

**How the window server is read, without `unsafe` of ours.** Both ways were
looked at:

- `objc2-core-graphics` 0.3.2 is **already in the build**, beneath
  `softbuffer` 0.4.8, and its `CGWindowListCopyWindowInfo` is a safe
  function. Taking it means turning on its `CGWindow` feature and naming it
  in one file. What has not been checked is whether every reading of the
  dictionaries it answers with — owner, layer, bounds — is safe as well;
  `objc2-core-foundation`'s typed `CFArray::get` and `CFDictionary::get`
  are, and reaching them from the untyped array it returns may not be.
- `osascript` asking the same function, as the tests already do: 80 ms
  wall and 20–30 ms of processor time a reading on this machine, idle. At
  two readings a second that is about one twentieth of a core spent on
  being told where a window is.

The crate is preferred, **on ADR 0024 § 1's stop rule**: the iteration that
builds the frame checks each call it makes in the crate's source, and if
any reading needs `unsafe` of ours, it uses `osascript` instead and says
so. Neither way is allowed to add `unsafe` here, which law 4 forbids
without an ADR of its own.

## 4. What is not measured, and is not hidden

The screen was locked while all of the above was measured
(`CGSSessionScreenIsLocked`), so:

- **A person's drag was not measured.** `cliclick` dragging Chrome's title
  bar did not move it, and System Events listed none of Chrome's windows,
  which is what a locked screen does. Whether `Browser.getWindowForTarget`
  follows a window a person moves, and how soon, is the first thing the
  building iteration measures — with the screen in use.
- **Ours brought in front, floating, was not measured.** Asked to make our
  floating window's process frontmost, the system left the person's editor
  in front — so Chrome's window staying on the screen then is not a result,
  and is not in the table. Whether a
  person clicking the frame sends Chrome's window to the strip is open,
  and if it does, a frame a person clicks is a frame that hides the page,
  and this decision is reopened.
- **Stage Manager off** was not measured. Two ordinary windows would then
  stay where they are; whether the frame should float there too is the
  same question with a smaller cost, and is left to the building
  iteration to measure rather than assume.

## Consequences

- `window.rs` remains the one file that names `winit`, and gains the
  window level and position it was not asked for before; both are safe
  functions in `winit` 0.30.13 and are checked as § 1 of ADR 0024 asks.
- One more file may name a rented crate — `objc2-core-graphics` — or none
  does and a subprocess is run instead; the gate's list says which.
- A frame of ours floats above other applications while Chrome is in
  front. Standing aside is the whole of what keeps that from being a
  window over somebody else's work, so it is asserted, not assumed.
- Item 643's question changes shape. A floating frame does not keep
  Chrome's window out of the strip; it only refuses to be left behind
  when it goes there.

## What this does not decide

- What the frame draws: the tab strip, 622's renderer, 623's sentence and
  625's refusals are their own items.
- Whether two windows feel like one browser. This makes them stay together
  where the screen allows; a person using it decides the rest, and the
  measurements in § 4 may send it back here.
- Anything about the engine path, whose window is one window and is
  unaffected.

## Addendum, 2026-10-26 — § 4 measured, and § 3's stop rule taken

Not a change to the decision. Iteration 415 measured what § 4 left open,
with the screen in use (`CGSSessionScreenIsLocked` absent), Chrome
155.0.8059.86, macOS 26.5.2 and Stage Manager on, Chrome's window placed at
200,140 and a floating `winit` window of ours at 200,100 1000×40:

- **A person's drag is followed.** `cliclick` dragging Chrome's title bar
  in thirty steps of five points, a tenth of a second apart, was read by
  `Browser.getWindowForTarget` at every reading — about a third of a second
  apart — to the point, and the window server listed the same. Dragging the
  corner was followed the same way, and the page was laid out again at each
  size: 940×552 to 900×520 in ten steps, by Chrome's word and by the page's
  own `innerWidth` and `innerHeight`.
- **A person clicking the frame does not send Chrome's window to the
  strip.** Five runs of five: a click on ours made our process frontmost,
  and Chrome's window stayed on the screen at its bounds for the three
  seconds it was watched. So the decision is not reopened.
- **A person closing Chrome's window** (⌘W) closed its page: the next
  reading through the tab's session was declined and the window server
  listed nothing of Chrome's.
- **Stage Manager off** was not measured. It is the owner's setting, and an
  iteration does not change a person's system to measure it.

**§ 3's stop rule was taken.** In `objc2-core-graphics` 0.3.2,
`CGWindowListCopyWindowInfo` is safe, but it answers an untyped `CFArray`
whose safe `get` exists only for an array of a known type — made one by
`cast_unchecked`, an `unsafe fn` — the `kCGWindow…` keys are `extern`
statics, which only `unsafe` reads, and `CGRectMakeWithDictionaryRepresentation`
takes a raw pointer. So the window server is read through `osascript`
(`crates/alo-window/src/window_server.rs`), and no crate was added to the
gate's list. Where the frame stands, from the two witnesses, is
`crates/alo-window/src/chrome_frame.rs` (queue item 672).

