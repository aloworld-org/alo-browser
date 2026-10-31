# ADR 0086 — A search's images cost a second request, and come only from Brave

**Status:** accepted — built by queue items 743 and 744 (31 October 2026)
**Date:** 2026-10-31
**Context:** ADR 0083 (a search is asked by the browser process, with the
words alone); ADR 0082 § 0 (the Brave Search API; alo pays it through a
subscription); ADR 0005 (a renderer has no network, and a stranger's bytes
are read in a confined process); ADR 0085 § 2 (every frame of a live card
sends the whole first screen again); `docs/design/shell.md`, *What appears
when a person types* (All shows a row of images with *See all images*;
the tabs All · Answer · Sites · Images · Videos · News); the owner's canvas
of 31 October 2026 (`Links.dc.html`'s `.imgs`, `Images.dc.html`'s `.grid`,
the Sites board's list); queue items 743 and 744

## The question in one line

The design's All tab shows a row of images, which Brave's web search does
not return: does every search pay for a second request to draw it, which
site does a person's machine fetch each picture from, and which process
reads those pictures?

## 1. What a search costs

Brave's web search (`/res/v1/web/search`) returns no images; its image
search (`/res/v1/images/search`) is a request of its own, paid as one. At
the price read for ADR 0082 (about $5 per 1,000 requests), a search that
draws the design's All tab costs **two requests**, about $10 per 1,000
searches.

**Chosen: the image search is asked only after the web search has found
at least one site, and only if no newer search is waiting.** It is one
request, of the words alone, and it serves both All's row (its first
five) and the Images tab (its first twenty) — the tab costs nothing more.

- A web search refused (401, 403, 429), failed (5xx, no connection,
  unreadable) or empty draws no row, so asks no images: a failure is never
  paid twice.
- A person who searches again before the images are asked is not made to
  pay for the first search's images; the first turn's Images tab says
  *Brave Search was not asked for images of "…": a newer search came first.*
- The Sites tab costs nothing: Brave's web search returns up to twenty
  sites unasked (`count` defaults to its most, 20), and All draws three of
  them. *More sites ›* shows the rest. A further page — `offset`, asked
  only when Brave says `query.more_results_available` — is a third request
  per press, and is cut to its own item (761).

**Considered and not chosen:**

1. **Images only when the Images tab is opened**, and no row on All. Half
   the cost, and the design's All without the row the owner chose on 31
   October 2026.
2. **Both requests at once.** The row would arrive sooner (one round trip
   instead of two in a row), and every failed or empty web search would pay
   for images nobody sees.

If the owner wants option 1 — say, to halve the bill while the subscription
is priced — it is one branch in `conversation_search.rs` (`images_follow`)
and the row is simply absent; nothing else depends on it.

## 2. What is sent

The same as a web search (ADR 0083 § 3): `GET
https://api.search.brave.com/res/v1/images/search?q=WORDS`, the words alone —
no `count` (Brave's default of 50 is read and cut to twenty), no `country`,
no `safesearch` (Brave's default is `strict`, which drops adult content), no
identifier — with `X-Subscription-Token`, `Accept: application/json` and what
`alo-net` sends with every request.

## 3. Where each picture comes from

**Only Brave's own image proxy, `imgs.search.brave.com`.** Each image result
names its thumbnail there (`thumbnail.src`, resized by Brave to 500 px wide)
and the original picture's own address on the site that holds it
(`properties.url`). alo fetches the first and never reads the second: showing
a search's results contacts **no site but Brave's**, so no site learns from a
thumbnail request that this person searched for something it holds a picture
of. A thumbnail named anywhere else is not fetched, and the image is shown
as its title.

Each thumbnail is fetched by the browser process, on the search's thread,
through `alo-net`'s pool (`search_thumbnails.rs`):

- `Accept: image/*` and what `alo-net` always sends; **no key** — the proxy
  needs none, so a thumbnail can never be billed to alo's key — no cookie,
  no `Referer`. A redirect is not followed.
- **Bounded:** at most twenty, one after another; each at most 256 KiB
  (`LARGEST_THUMBNAIL`), larger left out; and only bytes that begin as a
  picture this engine reads (PNG, JPEG, GIF, WebP, by their signature).
  `alo-net` reads a body whole before it can be weighed — bounded by its own
  most and the pool's patience — and a bound set per request is queued (762).

What Brave keeps of a thumbnail request is Brave's; it is the same company
that was just sent the words.

## 4. Which process reads the pictures

**The confined renderer, never the browser process.** A thumbnail's bytes are
a stranger's, even through Brave's proxy. The browser process only weighs
them and looks at their first bytes; it does not decode them. They are sent
to the first screen's renderer as data, beside the document, under a name of
ours (`alo:thumbnail-TURN-IMAGE`), exactly as the card's frame is (ADR 0076 B),
and decoded there by the picture readers every `<img>` goes through, with
their one bound on pixels. Bytes that begin as a picture and do not decode
are drawn as the tile's colour, and the renderer says so as an issue.

**What a first screen carries.** ADR 0085 § 2: the first screen is sent whole
for every frame of a live card. So only the pictures a tab draws are sent —
All's five, every one on Images, none on Sites — and at most **2 MiB** of
pictures in one first screen (`MOST_PICTURE_BYTES`), the newest turns' first;
an older turn past that shows its images as their titles. Brave's 500 px
thumbnails are tens of kilobytes, so a search's row is a few hundred
kilobytes at most, beside a live card's frame of 300–400 KiB. The wire
refuses more than the budget, more than twenty images, a thumbnail over
256 KiB, one that is not a picture, and an image on a page that is not a
site's. Sending pictures once and keeping them in the renderer is the larger
change ADR 0085 § 2 names, still not asked for by anything a person has hit.

## 5. How it is drawn

As the canvas draws it. **All:** under the three sites, 20 below, a heading
*IMAGES* (11.5, 600, capitals, `#6b655c`) and *See all images* (13, 500), 8
over five tiles 84 tall, 10 apart, rounded 10, `#e9e3d8`; then *More sites ›*.
**Images:** a grid of four columns, 16 apart and 20 between rows, each tile
150 tall, its title (13 on 18, 500, one line) and its site (12, with its
mark, 14 square). The canvas's *All · Photos · Diagrams* filters are not
drawn: Brave does not say which an image is, and a filter that guessed would
be wrong. **Sites:** every site as All draws its three.

The engine does not read `object-fit`, so a picture is laid out the full
width of its tile at its own proportions, centred up and down and clipped by
the tile — a photograph fills a row's tile; a picture wider than its tile's
proportions leaves the tile's colour above and below. Never stretched.

## 6. What a press does

Every open tab — the one shown, and each other that can be (Sites once sites
were found, Images once images were asked for) — is claimed by its place
among the open tabs drawn, and decided against what the browser process
painted: which chat, which turn, which tab. *More sites ›* opens that turn's
Sites tab, *See all images* its Images tab. An image, in the row or the
grid, is claimed by its place among the images drawn and opens **its page**
(`url`, the page it was found on) in the card, as a site's title does, its
bubble the image's title. Answer, Videos and News stay disabled and claim
nothing (707, 745, 746). *(Since built: Videos and News, by queue items 745
and 746 — asked only when their tab is opened, ADR 0087.)*

## Consequences

- `alo-window`: `brave_search.rs` (the image search, the proxy's origin),
  `search_protocol.rs` (an image answer read and refused by name — the same
  protocol, so the same file `serde_json` may be named in),
  `search_thumbnails.rs` (new), `conversation_search.rs` (sites told, then
  images), `conversation_thread.rs` and `conversation.rs` (a turn's tab, its
  images, the presses), `conversation_drawing.rs` (what was painted).
- `alo-renderer`: `results.rs` (`Tab`, `Images`, `Pictured`, the budget),
  `images_document.rs` (new: the row, the grid, their pictures),
  `results_document.rs` (the tab shown), the claims and the wire.
- Cut, each queued: the Sites tab's further pages (761); a bound on a
  response's body set per request in `alo-net` (762). Found on the way: a
  press on a relatively positioned box inside the conversation finds the box
  behind it (763); the tiles are not positioned.
