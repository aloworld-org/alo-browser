# ADR 0087 — A search's videos and news are paid for only when their tab is opened

**Status:** accepted — built by queue items 745 and 746 (31 October 2026)
**Date:** 2026-10-31
**Context:** ADR 0086 (a search's images cost a second request, asked with
every search that finds sites); ADR 0083 (a search is asked by the browser
process, with the words alone); ADR 0082 § 0 (the Brave Search API, paid
through a subscription); ADR 0005 (a renderer has no network, and a
stranger's bytes are read in a confined process); `docs/design/shell.md`,
*What appears when a person types* (the tabs All · Answer · Sites · Images ·
Videos · News); the owner's canvas of 31 October 2026 (`Videos.dc.html`'s
`.vr`, `News.dc.html`'s `.n`); queue items 745 and 746

## The question in one line

Brave's video search and news search are each a request of their own, paid
as one: when is each asked, and how often?

## 1. Asked only when a person opens the tab

**Chosen: never with a search.** A search asks the web search, then — ADR
0086 § 1 — the image search. The video search
(`/res/v1/videos/search`) is asked only when a person opens a turn's
**Videos** tab, and the news search (`/res/v1/news/search`) only when a
person opens its **News** tab.

- **Once for a turn.** The first time a turn's tab is shown, its search is
  asked (`Thread::ask_for` turns *not asked* into *asking*); whatever comes —
  results, none, or a refusal — is kept with the turn, and opening the tab
  again, or another tab and back, asks nothing. A refusal is not retried by
  a press: the person searches again for a fresh turn.
- **Only where something was found.** Videos and News open, as Sites does,
  once the web search found at least one site: a search refused, failed or
  empty pays for nothing more (ADR 0086 § 1's rule).
- **Not passed over by a newer search.** A search's images are not asked if
  a newer search is waiting (ADR 0086 § 1); a tab opened is different — a
  person pressed it and is looking at it — so the thread asks every tab
  opened, in the order opened, before the newest search waiting.

At the price read for ADR 0082 (about $5 per 1,000 requests), a search costs
two requests as before; each tab opened adds one, at most two more per turn.

**Considered and not chosen:**

1. **Asked with every search**, as images are, so the tabs open full. Two
   more requests on every search, for tabs most searches never open: the
   bill doubled for nothing a person sees.
2. **Asked on every opening of the tab.** Fresher, and a person flicking
   between tabs would pay for each flick.

## 2. What is sent

As every search (ADR 0083 § 3, ADR 0086 § 2): `GET` with **the words alone**
as `q` — no `count` (Brave's default, twenty, is read and kept as at most
twenty), no `country`, no `freshness`, no identifier — with
`X-Subscription-Token`, `Accept: application/json` and what `alo-net` sends
with every request.

## 3. Pictures

**A video's thumbnail comes only from Brave's own image proxy** and is
fetched, bounded and decoded exactly as an image's is (ADR 0086 §§ 3–4):
`thumbnail.src` at `imgs.search.brave.com`, never `thumbnail.original` nor
the video site's own picture; at most twenty, each at most 256 KiB and only
bytes that begin as a picture; no key, cookie or `Referer`; decoded only by
the confined renderer that draws them, and counted against the first
screen's 2 MiB of pictures. A video with no picture kept is drawn with its
tile's colour.

**A news story's photo is not fetched** in this item: the canvas shows one
beside each story; it is queued as 765, by the same rules when built.

## 4. What is read, and what is drawn

Brave's answers are read in `search_protocol.rs`, as the others are, and
refused whole by name (`NotVideos`, `NotNews`, and the shared refusals); a
result with no title or page, or a page not a site's, is left out and named.

- **A video:** its title, page, length (`video.duration`, kept only when
  it is digits split by colons — a length — and otherwise none), site
  (`video.publisher`, else `meta_url.hostname`), age and description.
  Drawn as the canvas draws `.vr`: the picture 200 × 112, rounded 10, on
  `#2A3446`, its length over the bottom right corner 8 in, white 11.5 on a
  dark rounded 5; 16 beside it the title (17 on 24, 600), the site's mark,
  name and age (13), and two lines of what is said (14 on 22); rows 20
  apart. The canvas's video playing in a card at the top is the card a
  title opens (§ 5); its *1 of 6 videos* is not built.
- **A story:** its title, page, site (`profile.name`, else
  `meta_url.hostname`), age and description. Drawn as `.n`: 16 above and
  below (the first 4 above) over a line of `#E6E0D6`, its site's mark, name
  and age on a line of 13, its headline 17 on 24, 600, and two lines of what
  is said.

## 5. What a press does

A video's title and a story's headline are titles as a site's is: claimed
by their place among the titles drawn, decided against what was painted,
and opened in the card as a new turn whose bubble is the title. A video's
picture claims nothing: its length is text an agent reads, and a link named
by a length would be a wrong name.

## Consequences

- `alo-renderer`: `results.rs` (`Tab::Videos`, `Tab::News`, `Asked<T>` —
  images, videos and news alike — the budget counting videos' pictures),
  `results_media.rs` (new: `Video`, `Story`, a length), `videos_document.rs`
  and `news_document.rs` (new), `start.css`, the wire.
- `alo-window`: `brave_search.rs` (two endpoints), `search_protocol.rs`
  (two answers), `search_thumbnails.rs` (videos' thumbnails),
  `conversation_search.rs` (work for a tab), `conversation_thread.rs` and
  `conversation.rs` (asked once for a turn, answered by number),
  `conversation_window.rs` (handed to the thread).
- Cut, queued: a news story's photo (765).
