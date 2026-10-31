# ADR 0076 — Where a page opened in the conversation is drawn

**Status:** accepted — B, chosen by the owner on 30 October 2026; the card built by queue item 717 (31 October 2026, *Built* below), input by 718
**Date:** 2026-10-30
**Context:** ADR 0058 (Chromium is a renderer we drive); ADR 0059 (the
product path and the engine path); ADR 0061 (Chrome draws the page in its own
window; § 1 measured what a photograph costs); ADR 0062 (Stage Manager shows
one application at a time); ADR 0066 (no window of ours goes over the page);
ADR 0024 § 5 (a person's pointer is a coordinate, and only a person's); ADR
0074 (the browser's own interface is a conversation drawn by this engine);
ADR 0075 (a page chosen in the conversation opens in Chrome's own window);
`docs/design/shell.md`, *A page inside the conversation*; queue items 702
and 704

## The question in one line

The design opens a page **as a card inside the conversation** — about 880 CSS
px wide and 340 to 740 tall, scrolled with the conversation, expandable to
the whole window, and a real page a person scrolls, follows and fills in. On
the product path Chrome draws the page in a process of its own. How does what
Chrome draws come to be inside a card of ours?

## Why this is a decision

Because the candidates are not variations of each other. One puts another
application's window over ours and moves it as we scroll; one puts pictures
of Chrome's page into our own drawing; one waits for this engine to draw the
page itself. Each changes what a person sees under Stage Manager, what the
agent's pipe carries, and which of ADRs 0061 and 0066 still hold as written.
ADR 0075 § 3 left exactly this to item 702.

## How it was measured

Chrome 156.0.8078.76, macOS 26.5.2 on an Apple M3 (8 cores), **Stage Manager
on** (`GloballyEnabled 1`, `AppWindowGroupingBehavior 1`), screen unlocked
(`CGSSessionScreenIsLocked` absent). Temporary probes outside the repository:
Node CDP clients starting Chrome with a fresh profile, `--app=data:,` (or
`--headless`), `--disable-background-networking` and
`--disable-component-update`; a stand-in for the conversation's window — an
AppKit window of **another process**, drawing a card outlined in red that
scrolls at a set speed and saying, every frame, where the card is on the
screen; and the window server read every millisecond by
`CGWindowListCopyWindowInfo` (on-screen windows only, front to back). Every
Chrome was closed and its profile removed; the owner's own Chrome was never
touched.

## A. Chrome's window placed over the card, moved as the conversation scrolls

### A1. How fast a placed window follows

`Browser.setWindowBounds` with a new `left`/`top`, Chrome's window 880×632 (a
600-tall page under Chrome's 32-point title bar):

| | median | p90 | max |
|---|---|---|---|
| one move, sent and answered, awaited before the next (100) | 0.31 ms | 2.3 ms | 11.9 ms |
| one resize, the same (40) | 0.51 ms | 10.4 ms | 19.4 ms |
| 200 moves sent at once | 40 ms for all | — | — |

**Chrome answers at about 5,000 moves a second; the screen takes 60.** Paced
moves, each to a distinct place, against the window server's list:

| moves sent a second | moves the window server listed | sent → listed there |
|---|---|---|
| 60 | 180 of 180 | median 21.7 ms, p90 23.5, max 25.1 |
| 120 | 181 of 360 | median 12.6 ms, p90 14.4 |
| 240 | 187 of 720 | median 9.5 ms, p90 11.1 |

The window server applies a window's position once a display frame and takes
the latest; anything sent faster is dropped, harmlessly.

### A2. What Stage Manager does with the two windows

Chrome's window placed exactly over the stand-in's card:

| step | ours (ordinary, level 0) | Chrome's |
|---|---|---|
| Chrome brought in front (`Target.activateTarget`) | **in the strip**, 98×133 | over the card |
| ours brought in front (`NSApp.activate`) | at its place | **in the strip**, 91×109 |
| ours ordered just below Chrome's window by number (`NSWindow.order(.below, relativeTo:)`) | **in the strip** | over the card |
| ours made full screen, then Chrome activated | its full-screen Space left for Chrome's | shown — on the other Space |

This is ADR 0062 § 1 and ADR 0075 § 3 again, now with ordering tried too:
**an ordinary window of ours and Chrome's are never on the screen together
under Stage Manager**, however they are ordered, and a full-screen window of
ours is a Space Chrome's window is not in. A card that is Chrome's window over
an ordinary window of ours cannot be built on this machine.

### A3. Ours floating, with a hole where the card is

The only arrangement found that shows both: our window **floating**
(`NSWindow.Level.floating`, layer 3), not opaque, the card's rectangle drawn
fully transparent, and Chrome's ordinary window under it, lined up with the
hole — so Chrome's page is seen through ours, and Chrome's title bar can sit
under our own card header.

| step | ours | Chrome's |
|---|---|---|
| ours in front | 215,48 1000×832 | 255,200 880×632 — **both on the screen** |
| Chrome in front | the same | the same |
| a click through the hole on the page's field | heard nothing | `mousedown` at 60,80, the field focused, `hello` typed into it — Chrome became the front application |
| a click on ours beside the card | became the front application | **stayed on the screen** |
| another application in front (a second ordinary window) | **floating over it, the hole showing that application** | in the strip |
| ours brought in front again | at its place | **still in the strip** |
| Chrome activated, then ours | at its place | over the card again |
| ours dropped to level 0 while Finder is in front (two runs) | once in the strip, once left at its place | in the strip both times |
| ours asked to go full screen while floating | **nothing happened** | — |
| ours minimised | in the strip | left on the screen alone until minimised itself |

Two findings the table needs said in words:

- **A transparent pixel does not let a click through on macOS 26.** A click
  in the hole was heard by our window. Clicks reached the page only when our
  window was told to ignore the pointer (`ignoresMouseEvents`) while the
  pointer was over the card — a timer reading `NSEvent.mouseLocation` 120
  times a second, flipping it at the card's edge.
- **Following a scroll trails by about two display frames.** The stand-in
  scrolled its card at a constant speed, drawing every frame; each new
  position was sent to Chrome at once. Two runs each, without screen
  captures loading the machine:

| scroll speed | card drawn → Chrome's window listed there | Chrome's distance from where the card was drawn |
|---|---|---|
| 200 pt/s | median 24 ms, p90 26 | 7 pt |
| 400 pt/s | median 24 ms, p90 28 | 13–14 pt |
| 1,200 pt/s | median 25 ms, p90 29 | 40 pt |

  What a person sees between placements (screen captures during the scroll):
  the page lags behind its frame, so the hole shows the page shifted by that
  distance and, at its leading edge, whatever lies under Chrome's window —
  the desktop or another application. With screen captures running at the
  same time the trail grew to p90 73 pt at 400 pt/s and 300 pt at 1,200: the
  following is only as good as the machine is idle.
- **A window is not clipped by another window.** When the card is partly
  scrolled out of the conversation, Chrome's window hangs outside our window
  (seen in the captures, below its bottom edge). Keeping it inside means
  resizing Chrome's window, which lays the page out again at every step, or
  hiding it; a window cannot rise above the menu bar at all (ADR 0061 § 2).

### What A3 costs

- **ADR 0066 as written is broken**: our window is over Chrome's, with a hole.
  The page is not covered, but its window is.
- **`softbuffer` presents every pixel opaque on macOS** (ADR 0066), so the
  hole needs a presentation path this repository does not have.
- **Ours floats over other applications**, and when one comes in front
  Chrome's window goes to the strip and the hole shows that application. So
  ours must drop to level 0 whenever neither ours nor Chrome is in front, and
  bringing ours back must activate Chrome and then ours, or the card is
  empty. This is ADR 0062's standing-aside, made larger, for the window that
  *is* the browser rather than a strip beside it.
- **No full screen**: a floating window does not go full screen, and an
  ordinary one leaves Chrome's window on another Space.
- The trail, the overhang at the conversation's edges, and a timer moving the
  pointer between two applications.

## B. Chrome's frames streamed into the card

The card's page at 880×600 CSS and device scale 2, a page of text with a field
and a fixed box. `Page.startScreencast` with `maxWidth` 1760 and `maxHeight`
1200 delivered **every frame at the full 1760×1200** — not the reduced frames
ADR 0061 § 1 saw for a whole window without a maximum. Headless, and shown
(an `--app` window on the screen), measured alike within noise.

### B1. What a frame costs

One frame by `Page.captureScreenshot`, eleven each:

| format | Chrome's answer | on the wire |
|---|---|---|
| png | median 51 ms | 266 KiB |
| jpeg q80 | 50 ms | 314 KiB |
| jpeg q50 | 50 ms | 213 KiB |
| webp q80 | 115 ms | 189 KiB |

A live stream, three seconds each:

| | frames a second | bytes a second | a frame | gap between frames |
|---|---|---|---|---|
| jpeg q80, page scrolling (a wheel every 16 ms) | 59.2–59.8 | **21.2–21.4 MiB/s** | 366 KiB | median 16.6 ms, p90 17.7 |
| jpeg q80, a box spinning | 60.3 | 21.9 MiB/s | 371 KiB | median 16.6 ms |
| png, page scrolling | 59.3–59.7 | **24.0 MiB/s** | 412 KiB | median 16.7 ms, p90 17.3 |
| png, a box spinning | 60.0–60.3 | 24.4–24.6 MiB/s | 417 KiB | median 16.7 ms |
| either, nothing changing | **no frames** once the page is still | 0 | — | — |

Chrome's processor time while a 200-paragraph page scrolled for five seconds,
headless: 24–52% of a core with no stream; **157% with a jpeg stream, 145%
with png** — about one more core while anything moves. Decoding the frames
and putting them in our window is on top, and was not measured.

**At card size the lossless format costs 13% more than jpeg**, and jpeg
changes colours: the background `#ffe0e0` arrived as 254,224,224 and
`#e0e0ff` as 224,225,255; png's were exact.

### B2. The agent's pipe while it carries pictures

`Runtime.evaluate("1")` on the same connection, sixty times, the page
animating:

| | median | p90 | max |
|---|---|---|---|
| no stream | 0.75–1.39 ms | 1.15–1.68 | 1.93 |
| jpeg stream running | **1.76–7.09 ms** | **7.7–9.6** | 11.6 |

ADR 0061's objection stands at card size, in numbers: the agent's commands
wait behind about 21 MiB/s of pictures, a few milliseconds each.

### B3. A person's input, forwarded, to the frame that shows it

`Input.dispatchKeyEvent` (a key down and up) or `Input.dispatchMouseEvent` (a
press and release) — the page changes its whole background on each — to the
first frame that arrives after it. Every first frame did show the change
(checked by its pixels).

| | median | p90 | max |
|---|---|---|---|
| headless, jpeg | 44.5–45.8 ms | 48.4–53.3 | 62.0 |
| headless, png | 51.9–52.6 ms | 54.1–70.0 | 70.7 |
| shown, png | 52.1 ms | 54.4 | 56.9 |

To that add decoding a 1760×1200 frame, drawing it into the conversation and
the display's next frame — so roughly **60–80 ms from a key to the glass**,
against a native window's one or two frames. Estimated, not measured.

### B4. A shown Chrome keeps streaming when its window is not seen

A shown Chrome's page went on painting and streaming at **60 frames a second**
with its window in front, with another application in front and its window in
the strip, after `windowState: minimized`, and with its window asked to
`left: -20000` — which Chrome clamped to −3727, off every display. Its page
said `visible` throughout (as ADR 0062 § 2 found). So **one Chrome can stream
the card and later show the same tab in its own window** when the card is
expanded, with no second Chrome and no lost session.

### What B costs

- **Pictures, not the page.** What is in the card is Chrome's drawing, a frame
  behind. Anything Chrome draws outside the page's compositor is not in the
  stream and is not measured here: a `<select>`'s menu (on macOS a native
  menu, ADRs 0071 and 0072), a date picker, the context menu, an input
  method's composition window, the pointer's shape, a file chooser, a
  permission prompt, a drag. Each needs its own answer or the expanded window.
- **About 21–24 MiB/s and a core of Chrome's while the page moves**, nothing
  while it is still, down the pipe the agent's commands use (B2).
- **A key is about three to five display frames from the glass** (B3).
- **ADR 0061 § 1's reference-render objection is smaller than it was.** On
  the product path nothing is compared against a committed raster either way —
  Chrome draws — and png frames are Chrome's pixels exactly. The gate's
  reference renders are the engine's and are untouched by any of this.
- **What it gains**: the picture is ours, drawn in our layout, so it scrolls
  with the conversation in the same frame, is clipped by the conversation's
  edges, has rounded corners if the design says so, and is the same under
  Stage Manager, full screen, a second display, or no Stage Manager at all.
  Nothing floats over anybody else's work, and ADR 0066 is untouched: no
  window of ours is over Chrome's, and a person's pointer reaches the page as
  ADR 0024 § 5 allows, as a coordinate the browser process forwards.

## C. This engine draws the page in the card

The engine path (ADR 0059) needs nothing measured to say what it costs: one
renderer, one window, one layout tree, the card a box in the conversation's
own document, reference renders as for everything else. What it cannot do
today is draw a page somebody else wrote: `docs/conformance.md` lists alo's
own pages, frozen alo Sites sections and two frozen web pages (the HTML
specification's pizza form and the first web page) as what is drawn, with
script on a small surface — stage 1's measure. **A site a person chooses from
the conversation is not something the engine draws today**, so C answers the
card for alo's own pages and for nothing a person browses to.

## D. Anything else

- **Chrome's window as a child window of ours: impossible across processes.**
  `addChildWindow(_:ordered:)` takes an `NSWindow`, and the only public way
  from a window number to an `NSWindow`, `NSApp.window(withWindowNumber:)`,
  answered **`nil` for Chrome's window** and our own window for ours (the
  control). There is no public API that makes another process's window a
  child of ours; what does it in other programs is the window server's
  private `CGS…` calls, which are private API and `unsafe` both.
- **Ordering against another process's window works, and does not help.**
  `NSWindow.order(.below, relativeTo: chromesNumber)` did put ours directly
  under Chrome's in the window list — and Stage Manager then moved ours to
  the strip (A2).
- **Hosting Chrome's layers in ours** (`CALayerHost`, `CAContext`) is private
  API. Not tried.
- **ScreenCaptureKit** could capture Chrome's window as pictures, with a
  screen-recording permission a person must grant: B by a longer road, and
  only while the window is drawn. Not measured.

## The recommendation

**B for the card, with Chrome's own window for *expand*.** One shown Chrome
per page, its window parked off the screen while the page is a card; the card
drawn by this engine from Chrome's png frames, streamed only while the card is
on the screen; a person's pointer, wheel and keys over the card forwarded to
the page as ADR 0024 § 5's coordinates. *Expand* places Chrome's window over
the conversation as ADR 0075 already does — the same tab, nothing reloaded —
and that is also where a native menu, a picker or an input method is used.
For alo's own pages, C, as soon as a card holds one.

Why:

1. **A is refused by this machine.** An ordinary window of ours and Chrome's
   are never on the screen together under Stage Manager, however ordered
   (A2), and the owner runs Stage Manager.
2. **A3 works by stacking costs**: a floating browser window, a transparent
   presentation path we do not have, a pointer timer, an activation dance
   when other applications come and go, no full screen, a page that trails
   its frame by two display frames and overhangs the conversation's edges —
   and it breaks ADR 0066 as written. Each is a place for the two windows
   to come apart in front of a person, and *correct before fast* says a page
   that slides against its own frame is a wrong picture, not a slow one.
3. **B's costs are numbers we can hold**: about 24 MiB/s and one core of
   Chrome's only while the page moves, a few milliseconds on the agent's
   commands, a key about 60–80 ms from the glass. The card is a preview a
   person reads and occasionally types into; the full page is one press away
   in Chrome's own window, where nothing is a picture.
4. **png, not jpeg**: at card size it is only 13% more and keeps Chrome's
   colours exact, which is what makes the card the same page as the expanded
   one.

**The owner's choice is between:**

| | the card is | under Stage Manager | full screen | ADR 0066 | new machinery |
|---|---|---|---|---|---|
| **B + expand** (recommended) | Chrome's frames, drawn by us | the same | the same | holds | stream decoding and drawing; input forwarding; a parked window |
| **A3** | Chrome's window under a hole in ours | both shown while ours or Chrome is in front | no | broken as written | a transparent presentation; a floating level managed on every activation; a pointer timer; the follower |
| **ADR 0075 as it is** | no card: the page is Chrome's own window over the first screen | one or the other | no | holds | none |

## What is not measured, and is not hidden

- **Stage Manager off.** It is the owner's setting, and an iteration does not
  change a person's system to measure it (ADR 0062's addendum). With it off,
  two ordinary windows stay where they are, so plain A becomes possible; the
  trail (A1, A3), the overhang and the full-screen Space are the same.
- **A person's own scrolling.** The stand-in scrolled at constant speeds; a
  trackpad's momentum and a 120 Hz display were not tried. The built-in
  display ran at 60 positions a second here.
- **A key to the glass.** B3 stops at the frame's arrival; decoding it,
  drawing it into the conversation and presenting it in this engine were not
  built or timed.
- **Native pieces through B**: `<select>` menus, pickers, input methods,
  pointer shapes, context menus, drags, file choosers, permission prompts.
- **Memory**, on either side, and a page with video or WebGL.
- **A3's resize-to-clip**: what a page laid out again on every scroll step
  looks like.
- **What the Dock, ⌘-Tab and Stage Manager's strip do with a parked
  Chrome** (B4): a person may find Chrome's application there and bring a
  window that is off every display to the front. To be measured by the item
  that builds B, and answered by hiding the application or by headless
  Chrome if it cannot be.
- **ScreenCaptureKit**, and anything private.

## Consequences — B was chosen

- Item 717 builds the card: a stream of png frames from the tab the
  conversation opened, decoded and drawn as a picture in the conversation's
  document, with a layout assertion for the card's box and a reference render
  of the card with a committed frame in it.
- Item 718 forwards a person's pointer, wheel and keys from the card to
  the page as coordinates (ADR 0024 § 5), never as an agent's verb (ADR 0002).
- ADR 0075 § 3's placement becomes *expand*, and Chrome's window is parked
  while the page is a card.
- ADR 0061's refusal of screencast is narrowed, not reversed: a person still
  looks at a page they have expanded through Chrome's own window; the card is
  the case § 4 kept screencast for, at a size where its cost is measured
  above.
- Item 704's split view (the page large beside the agent's column) faces the
  same question at a larger size and is decided there, with these numbers.

## What this does not decide

- How the agent's steps appear on a card (items 703, 704).
- Whether the engine path ever draws a page somebody else wrote; that is the
  stages' business, not this ADR's.

## Built (31 October 2026, queue item 717)

The card is built as this ADR decided, and changes nothing it decided. What
building it measured, through the machine's Chrome 156.0.8078.76 and alo's
CEF host (Chromium 156.0.8078.48), in `alo-window`'s
`tests/a_page_as_a_card_through_chrome.rs`:

- **A parked window streams** at about 60 frames a second, as B4 found —
  at the scale of the display macOS clamps it by. On this Mac, with two
  displays at one beside the Retina one, frames asked at 1756 × 1040 came
  at 878 × 520. Queue item 738.
- **The agent's pipe, B2's question, through this browser's own reading of
  it.** A page whose moving noise makes each PNG about B's size (338–387 KiB,
  20–23 MiB a second): an agent's read of the page's tree through the
  socket took a median of 0.95 ms with no stream and 0.99 ms streaming
  through Chrome, 0.96 and 0.96 through the host. Not at first: reading each
  frame's half megabyte as JSON, and decoding it on the thread that reads
  the agent's answers, made a read wait 16–100 ms in a debug build. So a
  frame's picture is taken out before the rest of its message is parsed,
  and decoded on another thread (`chrome_cast_wire.rs`,
  `chrome_screencast.rs`).
- **PNG on a shown Chrome at two costs Chrome itself.** Measured with a Node
  client, not this browser: Chrome's window shown on the Retina display,
  streaming PNG at 1756 × 1040, answered every command about 55 ms late
  (`Accessibility.getFullAXTree` and `Runtime.evaluate` alike); the same
  window parked by a display at one, none late. B2 measured JPEG only. Not
  a decision changed here; it is 738's to keep in mind if frames are had at
  two.
- **Stopping is answered.** Closing the card, or the card going off the
  screen, sends `Page.stopScreencast` and waits for Chrome's answer; no
  frame was told after it, and none was read after Chrome said it had
  stopped.

Still not measured: what the Dock, ⌘-Tab and Stage Manager do with a parked
Chrome (queue item 739); a key to the glass (718, now 740).

## Built (31 October 2026, queue item 718): a person's input on the card

A person's press, drag, release, wheel and keys over the card's page reach
the page as B3 measured they could: Chrome's `Input` domain, sent by the
thread holding the card's tab, at the point in the page's CSS pixels. As
this ADR and ADR 0024 § 5 decided, and nothing changed:

- **The browser process decides the point from its own composition.** The
  card's page box is computed from the first screen's viewport
  (`alo-renderer`'s `card_place.rs` — the 880 column centred, the person's
  line, the card's border and bar), never read from the renderer, and the
  renderer's card test holds it to the layout the engine computes at four
  sizes; the conversation's window maps a point of the window in device
  pixels to the page's CSS pixels (`alo-window`'s `card_aim.rs`). Chrome
  lays the page out at the page box's size, so one CSS pixel of the first
  screen is one of the page's at any scale. Anywhere outside the box — the
  bar, what the person said, the box, the panel — is the part's, claimed
  and decided as before.
- **Never an agent's verb (ADR 0002).** The window alone makes a person's
  input (`card_input.rs`), handed to the thread holding Chrome as
  `CardOrder::Input`; an agent's request is `Order::Agent`, which carries
  no point and becomes none.
- **The keyboard, decided here.** A press on the card's page gives the page
  the keyboard and takes it from the box; a press anywhere else in either
  part — the card's bar too — gives it back to the part, the box when it is
  the box that was pressed; a refused claim leaves it where it was; the card
  closing takes it. One thing has the keyboard at a time, as in any window.
  While the page has it, a key that writes text is sent as `keyDown` with
  that text, Return with `"\r"`, Backspace (with Chrome's `deleteBackward`),
  Tab and Escape as `rawKeyDown`, each then `keyUp` (`card_keys.rs`,
  `chrome_protocol.rs`). Nothing held with ⌘ or Ctrl reaches the page: those
  are the browser's keys and the system's. This is the ordinary rule of a
  window with two places to type, and ADR 0024 § 5 left focus to the item
  that builds it, so it is recorded here rather than as an ADR of its own.
- **A wheel** scrolls the page by Chrome's own 40 CSS pixels a line, a
  trackpad by the device pixels it moved over the scale.

Tested through the machine's Chrome and alo's CEF host, from the window's
own aim of each point (`alo-window`'s
`tests/a_persons_input_on_a_card_reaches_chromes_page.rs`): a press in a
field and `Ada Lx`, Backspace, left the field's value `Ada L`, as the page
said it; a press on a link followed it, the page it led to said its
address, and a frame streamed after showed it; three notches down
scrolled the page 120. Still not done: the pointer's moves with no button
held, other buttons, double clicks and a key or press to the glass (740);
the arrows and the page's own shortcuts (741).

## Built (31 October 2026, queue item 742): run by hand

Items 717 and 718 were run in the real window for the first time, and what
it found changes nothing this ADR decided:

- **A parked window still takes the front.** B4 parks Chrome's window off
  every display, and it is never *brought* in front; but showing a window
  at all makes its program the active one. Measured by hand through alo's
  CEF host on macOS 26.5: Return made *alo Chromium* frontmost within
  0.6 s, the conversation's window lost the keyboard and, under Stage
  Manager, went into the strip. CEF's views cannot show a
  window without activating its program (CEF 156 has no inactive show
  state). So the conversation's window takes the front back
  (`Window::focus_window`) as soon as it loses it while the card's
  Chromium starts, and once more when the card is open if it has not had
  it back. Polled every ~80 ms afterwards, *alo browser* never left the
  front. A person who brings another program in front in those seconds
  has it taken back too. What the Dock and ⌘-Tab do with the parked
  program is still 739's.
- **The card follows its page.** The tab already follows its main frame's
  `Page.frameNavigated` and `Page.navigatedWithinDocument`; while it
  streams, a change is told to the conversation, and the card's address —
  host bold, then path, as text nodes — is where the page is now.
- **Expand under Stage Manager.** Chrome's window put over the first
  screen and brought in front sends the conversation's window into the
  strip, as A2 measured of any two windows, and a person brings it back
  from there; closing Chrome's window closes the card. Not changed here.

## Built (31 October 2026, queue items 749 and 750): a card that fails, and the box under it

Run by hand again, a search's title on `www.reddit.com` opened a card that
stayed white and closed with nothing said. What it found changes nothing
this ADR decided:

- **The card waits for its page's `load`, and a page may replace itself
  first.** The thread holding the card's tab opens the page as `alo
  --chrome` does, and streams only once it has loaded. Reddit first
  answers a challenge whose script goes on to the page itself
  (`location.replace`) before the challenge's own `load`; the tab waited for
  the loader `Page.navigate` named, which never loads, for Chrome's whole
  thirty seconds — measured through the machine's Chrome and alo's CEF
  host alike, the second loader loading 0.2 s in. The tab now waits for the
  newest document its own frame began (`Tab::go`, `Page.lifecycleEvent`'s
  `init` on the tab's frame). The card is still white until the page has
  loaded; streaming from its first paint is queue item 751.
- **Why a card failed is kept.** Closing the card painted the first
  screen, which cleared its line; the reason is now kept as a refused
  press is (742), and said on the terminal. A card that fails gives the box
  the keyboard back: a press on the title had taken it, a card with no
  picture cannot be pressed for it, and the next address typed by hand
  went nowhere.
- **The box at the window's foot.** Once the conversation holds
  something, the box is the design's 880 × 56, 24 above the window's foot,
  and the conversation above it is clipped there; the part of the page box
  clipped away is not the page's to be pressed
  (`card_place::seen_page_box`). Scrolling is 703's.

