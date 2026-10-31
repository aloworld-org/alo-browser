# ADR 0090 — A card's Chromium is told the conversation's scale, and its stream is paced

**Status:** accepted — built with queue item 738 (31 October 2026)
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
