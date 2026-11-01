# ADR 0089 — A card opens at its page's commit, and a page that never loads stays a card

**Status:** accepted — built with queue item 751 (31 October 2026); § 2
amended by queue item 770 (1 November 2026, below)
**Date:** 2026-10-31
**Context:** ADR 0076 B (the card is drawn from the frames Chrome streams of
its page, Chrome's window parked off every display); ADR 0085 (the
conversation keeps every turn); ADR 0058 § 4 (a tab in Chrome); queue items
717 (the card), 742 (the card run by hand), 749 (a card said unopened is
kept, and `Tab::go` follows the newest document), 751; `CLAUDE.md`, *correct
before fast*

## The question in one line

A card's page was opened as `alo --chrome` opens one — waited for until its
`load` — and only then streamed. So a slow page was a white card for as long
as it loaded, and a page whose `load` never came within Chrome's thirty
seconds of patience was said *could not be opened*, though it may have drawn
everything a person needed. When does a card begin to show its page, what
does it say meanwhile, and what does a card whose page never loads become?

## Why this is a decision

Because "loaded" is a word every part of this browser leaned on: an agent's
tab is handed over loaded, so that its first read is of the page and not of
`about:blank`; `alo --chrome` says the page is open once it has loaded; and
the card took that over unchanged. Changing it for the card, and not for the
rest, needs a reason written down, and so does what happens to a card that
never reaches the moment the rest of the browser waits for. Neither has one
obvious answer: a browser could close it, give up after a time, or keep it.

## Decision

### 1. A card opens at its navigation's commit

The card's tab is waited for only until its navigation has **committed**:
Chrome has answered `Page.navigate` with a loader, and the tab's own frame
has begun a document since it was asked (its `init` stage,
`Page.lifecycleEvent`; `chrome_loading.rs`). From then the page is the new
one, and may paint; the card is said open (`Went::Opened`) and its stream is
asked for at once, so the first frame shown is the page's first paint. Only
the card is opened so (`Tab::open_as_a_card`, chosen by
`ChromeServing::start_as_a_card`); every other tab — an agent's, `alo
--chrome`'s, a frozen page's — is still waited for until it has loaded, since
an agent reads what a page is, and a page half-built is not that.

A navigation that never commits — a server that never answers, a name that
does not resolve within Chrome's own retries — is still not opened, after
Chrome's patience, and kept as 749 keeps it: there is nothing to show.

### 2. The card says *Loading* until its page has loaded

While the newest document of the tab's own frame has not loaded, the card's
bar says **Loading** after the address, in the bar's quiet words, before
*Expand*; the address gives it the room. It is a paragraph of the card's
document, so a person and an agent read the same word. What counts as the
page is 749's rule, kept in one place now: the newest document the tab's own
frame began, or the loader `Page.navigate` named before any began — so a
page that replaces itself (`www.reddit.com`'s challenge) is loading until the
page it went on to has loaded, and a frame inside the page is not the page.

The thread holding Chrome reads whether the page is loading from what Chrome
has already told it, and tells the conversation only as that changes, while
the card is streamed (`chrome_card::tell_loading`, `Went::Loading`), before
the frame it read in the same pass — so no frame is shown without the card
knowing. A link followed later is loading again until its own `load`.

### 3. A card whose page never loads stays a card

A page that has committed and never loads **stays open as a card**, streamed
and pressable, saying *Loading*, until a person closes it, opens something
else, or the page goes somewhere that does load. No timer closes it, and none
stops it saying *Loading*.

Chosen because it is what is true and what a person can use. A page whose
`load` waits on a picture, an advertisement or a long poll that never
finishes has usually drawn everything it will draw; closing it throws away
what the person asked for, for a reason they cannot see. Chrome's own tab
keeps its spinner turning for as long as the page loads and never closes it;
this card does the same in words. A time after which *Loading* is dropped
would say something Chrome has not said — that the page is done — and would
be a number to tune with nothing to measure it against.

## Rejected

- **Close the card after Chrome's thirty seconds, as before.** What 751 was
  queued to undo: the page had drawn, and the person was told it could not be
  opened.
- **Stop saying *Loading* after a time, and keep the card.** A guess at when
  a page is finished, said as if it were Chrome's word. If people find a
  forever-*Loading* card wrong, the place to decide it is with measurements of
  real pages, queued then.
- **Open every tab at commit.** An agent's first read would race the page's
  own building; `alo --chrome`'s *open* would mean less. The card is the one
  place a person watches the page arrive.
- **Stream from the navigation's start.** Before the commit the tab still
  holds the page before it (`about:blank`, or Chrome's `data:,`), and a frame
  of that is not the page asked for.

## What it costs

- A card's first frames may be of a page half-built — the challenge page
  white, a skeleton of placeholders — where before only a loaded page was
  shown. That is what a person sees in any browser.
- One more thing the thread holding Chrome tells the conversation, and one
  boolean more in the card a renderer is handed (`Card::loading`, on the
  wire after *Forward*).
- The card's tab no longer reaches `load` before the conversation is told it
  is open, so anything that asks the card's page something at once must not
  assume it has loaded. Nothing does today: no agent reaches the card's tab
  (queue item 716).

## Built

Queue item 751: `chrome_loading.rs` (what Chrome told of the tab's own
frame's stages: committed, loaded), `Tab::open_as_a_card` and
`Tab::is_loading` in `chrome_tab.rs`, `chrome_card::tell_loading`,
`ToFrame::Loading`, `Went::Loading`, `Conversation::card_loading`;
`Card::loading` and the bar's *Loading* (`card_document.rs`, `start.css`).
Tested through the window's own road against the machine's Chrome and alo's
CEF host (`a_card_drawn_before_its_page_loads_through_chrome.rs`): a local
page whose `load` waits on a held picture shows a frame of its own colour
while the card says *Loading*, is still a card, still drawn and still
loading past Chrome's thirty seconds, and once the picture is answered the
card no longer says it. With the card opened as before, the same test fails
with the page said unopened.

## Amendment — 1 November 2026: *Loading* from a navigation's start (queue item 770)

§ 2 said the card is loading while the newest document of the tab's own
frame has not loaded. So a link pressed in the card, *Back* or *Forward*, or
a script's `location`, said nothing between the press and the new
document's commit — a slow server's whole wait — where Chrome's own tab
turns its spinner from the press. § 2 now reads: **the card says *Loading*
from the start of a navigation of its page's own frame to another document
until Chrome says that frame has stopped loading**, whether or not the
navigation committed. § 1 and § 3 are unchanged: the card still opens at its
first navigation's commit, and a page that commits and never loads still
stays a card saying *Loading*.

**Measured**, Chrome 156.0.8078.76 and alo's CEF host (CEF 156.0.14,
Chromium 156.0.8078.48), shown with `--app=data:,`, over the pipe, every
request paused and let go as the tab does, from a Node probe in the session
scratchpad (`m770/probe.mjs`). For the tab's own frame both programs send
the same events, in the same order:

| navigation | before its request | after |
|---|---|---|
| a link pressed, a script's `location` | `frameRequestedNavigation`, `frameStartedNavigating` (`differentDocument`), `frameStartedLoading` | `init`, `frameNavigated`, `load`, `frameStoppedLoading` |
| `Page.navigate`; *Back*/*Forward* by `Page.navigateToHistoryEntry` | **no** `frameRequestedNavigation`; `frameStartedNavigating` (`differentDocument`, `historyDifferentDocument`), `frameStartedLoading` | the same |
| a `204`; a download; `Page.stopLoading` before the commit; a `mailto:` in Chrome | the same start | `frameStoppedLoading` alone — nothing commits |
| a refused port; a request failed by the browser process | the same start | `init`, `frameNavigated` to `chrome-error://chromewebdata/`, `load`, `frameStoppedLoading` — Chrome's error page commits (a `mailto:` in the CEF host did the same, until item 777 had the host cancel it: ADR 0060's fifth amendment) |
| `Page.stopLoading` after the commit, a picture held | — | `frameStoppedLoading`, and **no** `load` ever |
| another navigation begun before the first commits | a second start | the second's commit, `load` and stop; the first never commits |
| a fragment, `history.pushState` | `frameStartedLoading` alone; a step back to a pushed entry adds `frameStartedNavigating` (`historySameDocument`) | `navigatedWithinDocument`, `frameStoppedLoading`, within 5 ms |

Chrome's own tab, screenshotted at each step of the same pages in a window
with a tab strip: its spinner turned from a link's press while the server
held the answer, stopped when a `204` was answered, stopped at
`Page.stopLoading` before a commit, and stopped at `Page.stopLoading` after
a commit whose picture was still held — no `load` told — the page's icon
shown again.

**So the start read is `Page.frameStartedNavigating`** of the tab's own frame,
not `frameRequestedNavigation`: Chrome sends the latter only for a
navigation the page asks for, never for *Back*, *Forward* or
`Page.navigate`. It is told before the request leaves, whoever began the
navigation, and names its kind, so a move within the document — over which
Chrome turns no spinner, and whose stop follows within milliseconds — is
passed over rather than blinking the card's word. **The end read is
`Page.frameStoppedLoading`** of the tab's own frame, not the newest
document's `load`: it is what ends Chrome's spinner, it comes after `load`
when there is one, and it comes when a navigation ends without committing
or a page is stopped before its `load`, when no `load` ever would. Until
Chrome has told of either since the tab was sent somewhere, the newest
document's `load` decides, as § 2 first said. A frame inside the page is
still not the page.

**Built:** `chrome_protocol::started`/`stopped` read the two events,
`Loading::started`/`stopped`/`is_loading` keep them (`chrome_loading.rs`),
and `Tab::is_loading` answers from them; nothing the conversation is told
changed. Tested through the window's own road and both programs
(`a_cards_loading_from_a_navigations_start_through_chrome.rs`): a link
pressed in the card to a page the server holds says *Loading* while the card
is still at the first page, for two seconds of the hold; a link answered
`204` says it while held and no more once answered, at the page it was at;
*Back* to a page held says it from the press.
