# ADR 0090 — A card's Chromium is told the conversation's scale, and its stream is paced

**Status:** accepted — built with queue item 738 (31 October 2026); § 1 amended for the Chromium alo carries by queue item 772 (1 November 2026, *Amended* below)
**Date:** 2026-10-31
**Context:** ADR 0076 B (the card is drawn from the PNG frames Chromium
streams of its page, Chromium's window parked off every display; B1 and B2,
what a stream costs and what it does to the agent's pipe); ADR 0079 (alo
carries Chromium as CEF); ADR 0057 (a device scale is never measured under
emulation); ADR 0089; queue items 717 (the card), 738, 739; `CLAUDE.md`,
*correct before fast* and law 2

## The question in one line

A card's frames came at the scale of whichever display macOS put Chromium's
parked window by, not the conversation's: on this Mac a card asked for at
1756 × 1040 came at 878 × 520 and was drawn enlarged, blurred, on the Retina
display. How are frames had at the conversation window's scale — and what
does a stream four times the pixels do to the agent's pipe, which ADR 0076
B2 held a stream to?

## Measured

On this Mac — macOS 26.5, three displays: the built-in Retina display at two
(1710 × 1107 points, at the origin), a 1920 × 1080 display at one to its
left and a portrait 1080 × 1920 display at one left of that — through the
machine's Chrome 156.0.8078.76 and alo's CEF host (Chromium 156.0.8078.48),
each shown with `--app=data:,`, its window put with
`Browser.setWindowBounds` at 878 × 552, its page laid out at the card's
878 × 520, streamed with `Page.startScreencast` (PNG, at most 1756 × 1040).
Measured with a Node client of the scratchpad's, and then through this
browser's own reading (`a_page_as_a_card_through_chrome.rs`); Chromium's
source was not read.

### Where a parked window lands

| asked | put at, by Chromium's word | its scale | a frame |
|---|---|---|---|
| −20000, 0 (`PARKED`) | −3725, 0 — 153 points of it on the portrait display | 1 | 878 × 520 |
| −20000, −20000 | −3725, −50 | 1 | 878 × 520 |
| 0, 20000 | −1960, 1808 | 1 | 878 × 520 |
| 20000, 0 | 1670, 34 — 40 points of it on the Retina display | 2 | 1756 × 1040 |
| 20000, 20000 | 1670, 1075 | 2 | 1756 × 1040 |
| 0, −20000 | 0, 34 — wholly on the Retina display | 2 | 1756 × 1040 |

The host lands where Chrome does (−3725, 0, at one). **No place asked puts
the window off every display**: the window is moved back until a strip of
it is on one, and it draws at that display's scale. The window server
listed the strip on the screen, and a screenshot of the portrait display
showed it — the card's own page, `en.wikipedia.org`, run by hand. ADR 0076
B4's *clamped to −3727, off every display* was measured with other displays
or none beside this one; it is not true here (queue item 771).

With **one display** — reasoned, not measured: no display could be removed
here — the window is moved back onto the only display, the conversation's,
and draws at its scale: the frames were at the window's scale already, and
the strip of 771 is on the display the person is looking at.

### Telling Chromium the scale

| | `devicePixelRatio` | a frame |
|---|---|---|
| parked by a display at one, `Emulation.setDeviceMetricsOverride` with `deviceScaleFactor: 2` | 2 | 878 × 520 |
| the same, with `scale: 2` and the window 1756 × 1072 | 2 | 878 × 520 |
| parked by a display at one, started with `--force-device-scale-factor=2` | 2 | **1756 × 1040** |
| the host, the same | 2 | **1756 × 1040** |
| the host parked by the Retina display, started with `--force-device-scale-factor=1` | 1 | 878 × 520 |

Emulating the scale changes what the page is told and not the frame:
Chromium draws at two into a surface the size the window is at one. Started
with `--force-device-scale-factor=2`, a window parked by a display at one
streamed frames **identical, pixel for pixel, to the frames of the same
page from a window on the Retina display** — through Chrome, and through
the host (each against its own).

### What two costs the agent's pipe

`Runtime.evaluate("1")` timed forty times on the page's pipe while it
streams a page whose noise makes each PNG as large as a PNG of it can be (the
717 test's page), and a page of text scrolling every frame:

| Chromium, page, scale, pace | frames a second | MiB a second | evaluate, median / p90 |
|---|---|---|---|
| Chrome, noise, 1, none | 64.9 | 21.4 | 0.29 / 12.6 ms |
| Chrome, noise, 2, none | 34.9 | 40.3 | **57.2 / 58.8 ms** |
| Chrome, noise, 2, parked by the Retina display, none | 35.5 | 40.1 | 55.7 / 57.9 ms |
| host, noise, 2, none | 30.2 | 40.4 | **66.4 / 67.5 ms** |
| Chrome, noise, 2, every second frame (`everyNthFrame: 2`) | 30.3 | 35.0 | 33.4 / 65.6 ms |
| Chrome, noise, 2, each answered 16 ms after it came | 34.8 | 40.2 | 57.2 / 60.1 ms |
| Chrome, noise, 2, each answered 25 ms after | 25.4 | 29.3 | **0.23 / 0.56 ms** |
| Chrome, noise, 2, each answered 33 ms after | 27.1 | 31.3 | 0.16 / 0.90 ms |
| host, noise, 2, each answered 33 ms after | 28.5 | 38.2 | 0.18 / 0.29 ms |
| Chrome, noise, 2, JPEG q90 | 61.1 | 17.1 | 0.25 / 12.1 ms |
| Chrome, text scrolling, 1, none | 59.9 | 10.6 | 2.1 / 5.0 ms |
| Chrome, text scrolling, 2, none | 54.8 | 22.9 | 19.2 / 28.4 ms |
| Chrome, text scrolling, 2, each answered 50 ms after | 33.3 | 14.0 | 8.7 / 17.9 ms |

So 717's 55 ms was the frames' size, not the display: a window parked by a
display at one and told two costs the pipe what one on the Retina display
does. Answered at once, Chromium makes frames as fast as it can encode them
and every command waits behind them; answered after a little longer than one
takes, it waits for none. The scrolling page's commands wait on the page's
own thread drawing at two, which no pace removes, and is half again what it
was at one.

## Decision

### 1. A card's Chromium is started at the conversation window's scale

The Chromium that holds a card is started with
`--force-device-scale-factor` set to the scale of the conversation's window
when the card is opened (`Chrome::start_shown_at`,
`ChromeServing::start_as_a_card`'s `scale`, carried by `Streamed`), and is
parked where it was. Wherever the window server keeps its window, the page
is drawn and streamed at that scale, and at one on a display at one — never
four times the pixels for nothing. It is Chromium's own switch, which CEF
reads as Chrome does, so both programs take it the same way.

It is said at start and cannot be changed after: a conversation dragged to
a display of another scale while a card is open has its card's frames at
the scale it opened with — made smaller by Chromium when the new scale is
lower, which is sharp, and enlarged by this engine when it is higher, which
is 738's blur again until the next card (queue item 772).

### 2. A stream is paced to the one ADR 0076 measured

Each frame is answered (`Page.screencastFrameAck`) once its bytes have
taken their time at **24 MiB a second** — ADR 0076 B1's PNG stream of a
card at two, 24.0–24.6 MiB a second, which ADR 0076 accepted and B2 held
the agent's waits to — after it was read or after the frame before it was
due, whichever is later (`chrome_screencast::STREAM_BUDGET`, `paced`,
`Casting::due`). Chromium sends no frame past the ones it has in flight
until one is answered, so the stream carries no more than that down the
agent's pipe whatever the scale. A frame owed an answer is answered as the
thread holding Chromium reads its pipe — between orders every 4 ms while it
streams, and while an agent's command waits — and every one owed is answered
before the stream is stopped, so Chromium is left owing nothing.

At one, a frame of the card is a third of a megabyte and is due within a
60 Hz display's frame: nothing changes. At two, a page of noise is paced to
about twenty frames a second; a page of text, whose frames are a few
hundred kilobytes at two, is not paced at all.

## Rejected

- **Parking by the conversation's display.** No place is off every display
  (above): parking by the Retina display leaves a strip of the page beside
  the conversation, and which display is beside which is the person's
  arrangement, not ours to rely on.
- **Telling the page its scale by emulation.** ADR 0057 found emulation a
  trap for measuring a layout at a scale; it is one for the frames too: the
  page says two and the frame is still the window's size at one.
- **Every card at two, whatever the window.** Four times the pixels and the
  pace on a display at one, for frames this engine then makes smaller.
- **Answering at once, frames at two.** Measured through this browser's own
  reading: the agent's read waited 9–18 ms more at the ninetieth centile —
  past the 717 test's bound of 17 through the host once in two runs — at
  68–79 MiB a second.
- **JPEG at two.** Cheap, and changes colours (ADR 0076 B1): a card is
  Chromium's pixels exactly, and ADR 0076 chose PNG for it.
- **A fixed frame rate.** Thirty a second for every page would halve a card
  at one that costs nothing; the budget paces only what is heavy.

## What it costs

- A card of a page that changes most of its pixels every frame at two —
  video, noise — shows about twenty frames a second, not sixty.
- The window's scale at the card's start is the card's for its life (772).
- The frames are four times the pixels at two: decoded by the opener's
  thread and read by the confined renderer as every picture is; neither was
  measured here past the stream's rate.
- One more switch on the card's Chromium's command line.

## Built

Queue item 738: `chrome::scaled` and `Chrome::start_shown_at`;
`ChromeServing::start_as_a_card` takes the scale; `Streamed::scale` carries
it from the window (`conversation_window.rs`, unchanged) to the starter
(`conversation_chrome.rs`); `STREAM_BUDGET`, `paced`, `MOST_OWED` and
`Casting`'s owed frames (`chrome_screencast.rs`); `Tab::receive`,
`answer_due` and `uncast` answering what is owed (`chrome_tab.rs`). Tested
through the machine's Chrome and alo's CEF host
(`a_page_as_a_card_through_chrome.rs`): a card started at two, parked by a
display at one on this Mac, streamed every frame at 1756 × 1040 (Chrome
21.2 a second, 24.9 MiB a second; the host 18.4, 25.0), held to the budget
and to B2's growth — an agent's read 0.91 → 0.97 ms at the median, 1.34 →
1.21 at p90 through Chrome; 0.94 → 0.98 and 1.08 → 1.11 through the host;
a card started at one streamed at 878 × 520 though 1756 × 1040 was asked.
Mutations: the switch not passed fails at the frames' size through both;
the pace taken out fails at the budget through both (67.6 and 77.5 MiB a
second). By hand (`handrun13/`): `en.wikipedia.org` in a card on the Retina
display, blurred before and sharp after.

## Amended (1 November 2026, queue item 772): the Chromium alo carries follows the conversation's scale

§ 1 told a card's Chromium its scale at start and said it could not be
changed after: a conversation dragged from a display at one to the Retina
display streamed its card at one, enlarged and blurred, until the next card.
This amendment measures what a running Chromium takes, and decides.

### Measured

The same Mac, macOS 26.5, Stage Manager on, the same three displays (the
Retina display at two at the origin; one at one left of it, −1920, 0; a
portrait one at one left of that, −3000, −80). Google Chrome 156.0.8078.76
and alo's CEF host (CEF 156.0.14, Chromium 156.0.8078.48; the host with
`--alo-card`, its window transparent), each shown with `--app=data:,`, its
window 878 × 552 parked at `left: -20000`, its page a box turning and a
field of noise, streamed as PNG at most 1756 × 1040; then its window moved
with `Browser.setWindowBounds` and read two seconds later — frames' sizes,
`devicePixelRatio`, where Chromium and the window server put it. A Node
client of the scratchpad's; neither program's source was read.

| started with | then | where it went | `devicePixelRatio` | frames |
|---|---|---|---|---|
| no scale switch (both) | parked at −20000, 0 | −3725, 0, 153 points on the portrait display | 1 | 878 × 520 |
| no scale switch (both) | asked 20000, 0 | 1670, 34, 40 points on the Retina display | 2 | **1756 × 1040** |
| no scale switch (both) | asked −20000, 0 again | −3725, 0 | 1 | 878 × 520 |
| no scale switch (host) | asked 0, 20000 | −1960, 1808, 40 × 32 on the portrait display | 1 | 878 × 520 |
| `--force-device-scale-factor=1` (both) | asked 20000, 0 | 1670, 34 | 1 | 878 × 520 |
| the same | `Emulation.setDeviceMetricsOverride`, `deviceScaleFactor: 2` | — | 2 | 878 × 520 |
| the same | the screencast stopped and started again | — | 2 | 878 × 520 |
| the same | the window resized to 900 and back | — | 2 | 878 × 520 |
| `--force-device-scale-factor=2` (Chrome) | moved between the two | — | 2 | 1756 × 1040 throughout |

Where the window server leaves a window asked just past a display's edge
(the host, no switch): 1711, 0 / 600 / 1000 → 1670 at that height, at two;
0 or 800, 1108 (under the Retina display) → that x, 1075 — 32 rows on it —
at two; −1000 or −1920, 1081 (under the display at one) → that x, 1048, at
one; −2500, 1841 (under the portrait one) → −2500, 1808, at one; above any
display (−1000, 1000 or −2500, −1000) → wholly under its top, 30 or 34. A
window asked past a display's edge with no display beyond it is moved back
onto that display, and drawn at its scale, within the two seconds read.

So: **nothing a running Chromium takes over the pipe changes a scale it was
told at start** — not a move onto a display of another, not emulation (the
page told two, the frame still one), not a new stream, not a resize; CEF's
own host-side calls for it (`NotifyScreenInfoChanged`, `WasResized`) are
documented by CEF as for windowless rendering only, which a Chrome-style
browser view — the one ADR 0060's held requests need — does not do (read in
CEF's API documentation; not tried). **Told nothing, either
program draws its window at the scale of the display holding it, and again
at another's as soon as it is moved there, while it streams.** Through the
carried Chromium only the place decides; through Chrome too, but a strip of
Chrome's parked window on the conversation's display would be seen (ADR
0076 B4's amendment, § 3).

### Decision

1. **The Chromium alo carries is told no scale.** A card of it is started
   with `--alo-card` alone (`chrome::as_a_card`, `told_its_scale`), and its
   window is **parked by a display at the conversation's scale**
   (`chrome_park.rs`): for each display the window server lists at exactly
   that scale (`window_server::displays`, which now reads each display's
   `backingScaleFactor`), just past its right edge, its left edge and its
   bottom edge — each only where the window asked there meets no display,
   never above (a window asked above is put wholly on the display). Where
   the window went is read back (`chrome_window::put_at`), and the scale of
   the display holding the most of it (`drawn_at`) must be the one asked,
   or the next place is tried; with none — no display at that scale, one
   at 1.25, the window server not answering — the window is parked at
   `PARKED` as before and drawn at whatever display keeps it.
2. **A new scale parks it again.** The conversation's window moved to a
   display of another scale asks for the card's stream at it (queue item
   774's restream, `Streamed::scale`), and `CardOrder::Stream` now carries
   the scale; the thread holding the card parks the window by a display at
   it before the stream starts again (`chrome_card::Scaled::ByItsDisplay`),
   once per scale, and again only if fitting the page moved the window off
   the display it was parked by. A parked window is fitted where it was
   parked (`Fitted::parked_at`), so it stays there. The page is not
   reloaded: the same Chromium, the same page, drawn again at the new scale.
3. **The machine's Chrome keeps § 1.** It is still told the conversation's
   scale at start (`Scaled::Told`) and keeps it: its parked window cannot be
   made unseen, and parking it by the conversation's display would put a
   strip of the live page beside the conversation. `alo --chrome` and tests
   start it; the application never does. Queue item 780.
4. **A new Chromium for a new scale is rejected** for both: it would lose
   the page's state — a form half filled, a scroll, a login in a page's
   memory — for a sharper picture; and through the carried Chromium it is
   not needed.

### What it costs

- **The parked window is on the conversation's display when no other
  display is at its scale**, as it already was with one display (ADR 0076
  B4's amendment): transparent and letting the pointer through, so the
  window server lists it drawn on no display. But **under Stage Manager it
  is put in the display's strip**, as a blank tile with the carried
  Chromium's icon — measured by hand with the conversation on the Retina
  display, both when the card opened there and when the conversation was
  dragged there; the window is then listed at the tile (16, 714, 103 × 111,
  alpha 0) and still streams at two and takes presses. Moved back to a
  display at one, the window is parked by it again and the tile goes.
  Neither the window's `Transient`, `IgnoresCycle` nor `Auxiliary`
  collection behaviour kept it out of the strip; making the host an
  accessory application did, but its window then was not moved and drew no
  card (measured, not built). Queue item 781 — **answered by ADR 0076 B4's
  second amendment** (queue items 781, 739, 778): the card's host is an
  accessory application until the card is seen and its unseen window floats
  over every ordinary one, so no tile is left in the strip; the accessory
  host's window had drawn no card because, no longer activated by showing
  it, it was shown under the conversation's window and occluded.
- Parking asks the window server for its displays (`osascript`, about
  80 ms) and moves the window (150 ms or more until it is still) at a
  card's start and at each new scale — not at each resize.
- A display at a scale this engine has not measured (1.25, 1.75) is
  parked by only when nothing else is — then the frames come at its scale
  and are drawn into the card's box as they come.

### Built and tested

Queue item 772: `chrome_park.rs` (`park`, `places`, `drawn_at`);
`window_server::Display` and the displays' scales; `chrome::told_its_scale`
and `as_a_card`; `chrome_card::Fitted`'s `Scaled`; `CardOrder::Stream`'s
`scale`; `chrome_serving`'s `placed_and_scaled`. Through alo's CEF host
(`a_page_as_a_card_through_chrome.rs`): a card started at one streams every
frame at 878 × 520, asked at two every frame at **1756 × 1040**, its window
listed at 1670, 34 by the Retina display and drawn on no display, and asked
at one again 878 × 520, parked at −3725, −50 by the portrait display; a card
started at two parked by the Retina display streams 1756 × 1040 at 18.7 a
second within the budget, an agent's read 0.95 → 0.99 ms median. Through
Chrome the same card at one asked at two streams 878 × 520 throughout.
Mutations: no new park on a new scale, and the carried Chromium told its
scale, each fail at *moved to two* (878 × 520). By hand (`handrun19/`):
`en.wikipedia.org` in a card at one, the conversation dragged to the Retina
display: the card sharp — mean |Laplacian| 11.99 over the page's crop at
1756 × 1040 where the crop at one enlarged two times bilinearly gives 5.54
— *Talk* pressed and followed, and dragged back, parked by the portrait
display again; the Stage Manager tile while on the Retina display.

