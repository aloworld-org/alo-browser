# ADR 0061 — Chrome draws the page in its own window, and we hold the frame around it

**Status:** accepted
**Date:** 2026-10-24
**Context:** ADR 0058 (Chromium is a renderer we drive), whose § 2 moved a
renderer behind `ToRenderer`/`FromRenderer` and never said what becomes of
`Painted(Frame)` on the far side; ADR 0059 (the product path); ADR 0002 (the
layout tree is the agent's tree, and no verb takes a coordinate); ADR 0052
(the agent's gate and the person's gate); queue items 627, 623, 624, 625

## The decision in one line

On the product path **Chrome draws the page in a window of its own**, started
in `--app` mode and placed by this process over the pipe; our window holds
everything around it; and the mark that says what the agent is touching is
drawn by **Chrome's own `Overlay` domain**, against the node the agent named.
`Page.startScreencast` is refused for this, and kept for the case it is
actually for.

## Why this is a decision rather than an implementation detail

Because without it the product path shows a person nothing. Items 619, 620,
621 and 633 open a page in Chrome, read it as our tree, drive it by name and
hold its every request — and at the end of all four a page is loaded, read
and filled that **nobody can see**. `FromRenderer::Painted(Frame)` is how a
rendered page reaches a window, and the Chrome side of the boundary has no
answer for it.

There are two answers and they are not variations of each other. One makes
the product a window of photographs; the other makes it a frame around a real
browser. Everything about how it feels to use turns on which.

## 1. What a photograph costs, measured

Chrome 155.0.8059.86 on this machine, one frame at 1200×800 and device scale
2 — a retina window — of httpbin's form, median of eleven captures:

| format | encode | on the wire | ceiling from encoding alone |
|---|---|---|---|
| jpeg q80 | 34 ms | 81 KiB | 29.7 frames/s |
| jpeg q50 | 33 ms | 64 KiB | 29.9 frames/s |
| png | 50 ms | 69 KiB | 20.0 frames/s |

**The lossy format is the larger one.** 81 KiB against PNG's 69. An interface
is flat colour and text, which is what PNG compresses well and what JPEG
spends its bits ringing around. The lossy choice here does not even buy size;
q50 saves five kilobytes against lossless and looks worse doing it.

**Thirty frames a second is a ceiling, not a rate.** That 34 ms is encoding
alone, on a machine that was not busy. Transport, base64, decode and upload
all follow it, and the gate's own test runs put this machine at a load average
of 84 on 8 cores.

**At thirty frames a second it is 1.0 MiB/s — 8 Mbit/s — down the pipe the
agent's commands use.** The thing driving the browser would queue behind video
of the browser.

And a screencast is cheaper than that table only because **it reduces the
picture**: asked for a 1000×700 window it delivered frames of 756×469. What a
person looked at would be a downscaled, re-encoded copy of a page Chrome had
already drawn properly, a few frames behind.

There is a further cost with no number against it. **A reference render is how
this repository knows a change moved a pixel.** Every visual claim in the gate
rests on comparing a deterministic raster against a committed one. Put a JPEG
re-encode between the renderer and the window and that sentence stops being
true on the product path for ever.

*What is not measured, and is not hidden:* the frame rate of a live screencast.
Three frames in six seconds on a continuously animating page, headless **and**
headful, because the display was asleep and macOS stops compositing for a
sleeping display. That is a measurement of the display. The table above is the
cost of a frame, taken through `Page.captureScreenshot`, which shares Chrome's
encoder with screencast but not its whole path.

## 2. What a window costs, measured

Chrome in `--app` mode has no tab strip and no omnibox: inner 1000×668 inside
an outer 1000×700 — 32 px of title bar and **no side borders at all**.

Placing it over the pipe is exact wherever the position is legal:

    asked {"left":120,"top":80,"width":900,"height":640}
    got   {"left":120,"top":80,"width":900,"height":640}    EXACT

    asked {"left":0,"top":0,"width":1200,"height":800}
    got   {"left":0,"top":34,"width":1200,"height":800}     the menu bar

Where the system overrules the ask it overrules it predictably, and
`Browser.getWindowForTarget` reports where the window actually went, so the
frame can follow rather than assume.

**The cost, stated plainly: two windows.** macOS offers no supported way to
put another process's window inside ours, so a window of ours must track
Chrome's position, focus and resize. The measurement above says that tracking
is mechanically possible. It does not say it will feel like one browser, and
nothing in this ADR claims it will. That is item 627's own closing condition
to answer.

## 3. The mark is Chrome's to draw, and it is already in our words

`Overlay.highlightNode`, one command, against the node id the agent named:

    button              92.31 × 21
    ACCESSIBILITY
    Name                Submit order
    Role                button
    Keyboard-focusable  ✓

Drawn by Chrome's own compositor. **Name and role are the agent surface's own
vocabulary** — not a screenshot annotated afterwards, and not a guess. No
script is injected into the page, and what we pass is a node id, never a
position, so ADR 0002 is untouched: the mark is drawn *downstream* of a
decision the agent made by name, and a verb's outcome does not change by one
bit depending on whether anybody is watching.

This is most of queue item 624 arriving for free, and it exists only on this
path. Drawn into a screencast it would be re-encoded along with everything
else.

(`Overlay.enable` must be sent *after* `DOM.getDocument`, or it refuses with
*"Overlay must be enabled before a tool can be shown"*. Measured, not read.)

## 4. What this does not refuse

Screencast keeps one real advantage: **it is the only answer that works with
no display at all.** A browser driven on a machine nobody is sitting at — a
server, another person's machine, a test — needs frames over a wire, and that
is a thing we will want. This ADR refuses it as *the way a person looks at the
page*, not as a capability. It is built second, if a case asks for it, and it
is never what the gate's reference renders go through.

## Consequences

- The Chrome implementation of the renderer boundary answers `Painted` by
  placing a window rather than by producing a frame, and the difference
  between the two implementations is now visible at the boundary. That
  asymmetry is named here so it is not discovered later.
- `--app` and window placement join what `chrome.rs` starts Chrome with.
- Item 624 is built on `Overlay.highlightNode` rather than on drawing of our
  own, and 623 and 625 are our window's, not Chrome's.
- A window of ours tracks another process's window. This is the riskiest
  thing on the product path and it is not pretended otherwise.
- ADR 0058 § 6 still binds: a claim names its renderer, and *what a person
  sees* is now part of what differs between the two.

## What this does not decide

- How the two windows are made to feel like one. Item 627 closes on that,
  and it may send us back here.
- Whether screencast is ever built. Not now; not never.
- Anything about the engine path, which draws into its own window as it
  always has.

## Addendum, 2026-10-24 — what iteration 380 built, and what it found

Not a change to the decision. Queue item 627 was closed with its scope cut:
Chrome is started shown in `--app` mode on `data:,` (`about:blank` is passed
over and a tabbed window opens instead), its window's own page is the tab,
the window is sized so the page is laid out at the size asked — a 900×600
page in a 900×632 window — and placed, and `place` answers where the window
went rather than where it was sent (`crates/alo-window/src/chrome_window.rs`).

**How the two windows are made to feel like one is now item 642**, not 627,
and is still what may send us back here.

One thing measured that § 2 did not: **a window placed is not always a
window seen.** With Stage Manager on, a window that is not in front is a
thumbnail in its strip — the window server lists it at 16,773 and 97×115
while `Browser.getWindowForTarget` still says 160,120 and 900×632. So
Chrome's word for where its window is is not the screen's, and a frame of
ours that followed Chrome's word would follow a window nobody can see.
`Target.activateTarget` brings it in front alone and with two Chromes taking
turns, and did not, once in four runs, under the test suite's other shown
Chromes (item 643). Item 642's tracking is to be measured against the
window server, not only against Chrome.

## Addendum, 2026-10-24 — the frame's window, decided in ADR 0062

Not a change to the decision. Iteration 382 measured the two windows under
Stage Manager: an ordinary window of ours and Chrome's are never on the
screen together, since two programs are two applications and Stage Manager
shows one at a time. **ADR 0062** decides the frame is a floating window,
put by Chrome's word and shown only while the window server lists Chrome's
window on the screen there.
