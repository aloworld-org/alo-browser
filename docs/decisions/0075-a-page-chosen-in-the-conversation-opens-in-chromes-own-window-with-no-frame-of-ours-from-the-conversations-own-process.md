# ADR 0075 — A page chosen in the conversation opens in Chrome's own window, with no frame of ours, from the conversation's own process

**Status:** accepted
**Date:** 2026-10-30
**Context:** ADR 0058 (Chromium is a renderer we drive); ADR 0060 (Chrome's
own network carries a request and the browser process decides it before it
leaves); ADR 0061 (Chrome draws the page in its own window, and we hold the
frame around it); ADR 0062 (our frame floats beside Chrome's window); ADR
0069 (a Chrome profile is held by a lock); ADR 0073 (Chrome is started with
its own traffic switched off); ADR 0074 (the browser's own interface is a
conversation); ADR 0024 § 2 (the event loop never waits on a renderer);
queue items 701, 713 and 702

## The decision in one line

When a person chooses *Go to the site* in `alo --conversation`, the address
opens in the machine's Chrome **from the conversation's own process**,
through the same `ChromeServing` that `alo --chrome` uses, on a thread the
window hands the address to and never waits on — and Chrome's `--app`
window is placed by us over the first screen **with no frame of ours beside
it**, because the conversation's window is ours.

## Why this is a decision

ADR 0061 says our window holds everything around Chrome's, and ADR 0062
decided that window is a floating frame that follows Chrome's. Read
literally, every shown Chrome comes with that frame — and the frame runs on
a `winit` event loop of its own, which a process has one of, which is why
item 701 cut this. Opening a page from the conversation without the frame
is therefore a departure from how `alo --chrome` shows a page, and is
written down rather than slipped in.

## 1. In the conversation's process, not a second one

Two ways were open:

- **A second process** — `current_exe() --chrome ADDRESS`. It would reuse
  `alo --chrome` whole, frame and all, but each address would then be a
  browser of its own: its own agent socket fighting for the one place
  (ADR 0055 § 1's *one browser per place*), its own grants and its own
  record, none of which the conversation could see or say anything about,
  and a frame floating over the conversation's window saying what the
  conversation already says.
- **A thread here.** `ChromeServing` already holds Chrome on a thread of
  its own and tells whoever started it what happened through a function
  (`TellFrame`); nothing in it needs an event loop. The conversation's
  window hands the address to one more thread
  (`alo-window`'s `conversation_chrome.rs`), which starts the serving —
  blocking, as `ChromeServing::start_shown` does until the page has loaded
  — and tells the window *opening*, *opened* or why not as events, as
  `conversation_drawing.rs` tells it what renderers painted.

The second is taken. It costs nothing in depth: Chrome is started by
`Chrome::start_shown` with our switches (ADR 0073), the page is opened by
`Tab::open` with every request held and decided before it leaves and
recorded (ADR 0060), its profile is locked and swept as every other is
(ADR 0069), and it is closed, waited for and its profile removed when the
conversation's window closes.

## 2. No frame of ours, and what is watched instead

The frame exists because under `alo --chrome` it is the only place a
person can read which renderer draws the page and what an agent did. In the
conversation, the conversation is that place: its first screen's line says
what became of the address — *opening …*, *… is open in Chrome's own
window — Chrome/156…, driven by this browser*, *… could not be opened:
why*, *Chrome's window showing … was closed*.

So `ChromeServing::start_shown_alone` holds a shown Chrome that nothing of
ours stands beside. The frame's reading (`chrome_frame::read`) asks the
window server through `osascript` every 200 ms — 80 ms wall and 20–30 ms of
a core a reading on this machine (ADR 0062 § 3) — to decide where a frame
stands. With no frame there is nothing to stand, so the window server is
not asked at all: Chrome alone is, once a second, whether its window is
gone (`chrome_frame::gone`, the half of `read` that is Chrome's word), and
a window gone is said in the conversation and its Chrome closed.

## 3. Where Chrome's window goes, for now

Over the first screen: its top left corner where the conversation window's
first screen begins (the window's outer corner, read with `winit`'s
`Window::outer_position`, plus the panel's 240 points), and its page as
large as the first screen. Chrome's title bar then lies over the
conversation's and the panel stays in view. Where a window's position
cannot be read, where `alo --chrome` puts it (`CHROME_AT`, `CHROME_SIZE`).

**Measured, by hand, on 30 October 2026** (Chrome 156.0.8078.76, macOS
26.5, Stage Manager on): the conversation's window at 215,90 1280×852;
typing `http://127.0.0.1:8765/page.html` into the box and Return opened the
page in Chrome's window at 455,90 — over the first screen as intended —
the local server asked for the page and a favicon
and nothing else, and the line said it was open. **ADR 0062 § 1 holds
here too:** the conversation's window is an ordinary one, so with Chrome in
front it went to Stage Manager's strip, and brought back in front it sent
Chrome's to the strip (listed at 15,702 102×136). A person sees one or the
other, not both. That is the cost of this placement under Stage Manager,
and it is item 702's to measure further and decide; this ADR does not
pretend the two windows feel like one.

## 4. A second address replaces the first

One Chrome, one page: the Chrome showing the first address is closed,
waited for and its profile removed, and a new Chrome opens the second.
`ChromeServing` holds one tab, with the record and grants for that tab, and
no navigation order exists to send it; a second window would be a page a
person did not ask to keep, with nothing of ours saying which is which.
Until a conversation holds pages (702, 703), what a person chose last is
what is shown. Addresses chosen while a Chrome is still starting are not
queued; the newest is opened once it has.

## 5. What leaves the browser process

The chosen URL, and nothing else typed: Chrome is told the `Url` that
`alo-url` parsed, as `alo --chrome` is told the one it was named.

## Consequences

- `chrome_serving.rs` gains `ChromeServing::start_shown_alone`, a
  `WATCH_EVERY` of one second, and `ChromeServing::profile`;
  `chrome_frame.rs` gains `gone`.
- `conversation_chrome.rs` is new: the thread, what it tells the window
  (`Went`), where Chrome's window goes (`Placing`).
- `window.rs` hears a second kind of event and calls one more `winit`
  function, `Window::outer_position`, a safe function in 0.30.13's source.
- **No agent socket is opened for a page the conversation opens**, so
  `--agent-site` is still refused beside `--conversation`, now saying so.
  An agent reaching that page, kept to the sites named, is queue item 716.
- Closing the conversation's window waits for the Chrome showing a page to
  close — at most as long as Chrome is given to answer.

## What this does not decide

- Where a page goes *inside* the conversation, and whether two windows can
  feel like one under Stage Manager: item 702.
- Whether the frame of ADR 0062 is ever drawn by the conversation's own
  loop. Nothing here needs it.
