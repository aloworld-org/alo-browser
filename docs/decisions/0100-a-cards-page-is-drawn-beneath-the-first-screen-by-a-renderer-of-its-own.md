# ADR 0100 — A card's page is drawn beneath the first screen, by a renderer of its own

**Status:** accepted — built by queue item 869 (7 November 2026)
**Date:** 2026-11-07
**Context:** ADR 0076 B (a card is drawn from the PNG frames Chromium
streams of its page, decoded in a confined renderer as every picture is);
ADR 0090 (frames at the window's scale, the stream paced at 24 MiB a
second); ADR 0074 (the panel and the first screen, each drawn by a
confined renderer on the drawing thread); ADR 0024 § 2 (the event loop
never waits on a renderer); ADR 0005 (a stranger's bytes are decoded only
in a confined process); queue items 861 (the one box as a text field,
whose hand run found this) and 869; `CLAUDE.md`, *correct before fast* and
*every rendering decision is testable*

## The question in one line

With a live card streaming beside the one box, a key's edit was drawn up to
about a second after the key (`handrun38/13`–`15`); with no card, at once.
Where did the time go, and how is an edit never made to wait for a frame of
the card?

## Measured

On this Mac — macOS 26.5, the Retina display at two at the origin — through
the release application, alo's CEF host (Chromium 156.0.8078.48) holding a
local page of a box turning and a bar sliding under CSS animations
(`127.0.0.1:8739`), the window at 1280 × 852 points. Timestamps were put,
for the measurement only, where a key reaches the window, where its drawing
is sent to the drawing thread, where that thread begins it, where the
first screen's renderer answers, where the window hears the frame and where
it is presented; keys typed through System Events, 0.7 s apart.

**Before** (2541477):

| | median | p90 |
|---|---|---|
| a card's frame painting the first screen, in its renderer | 203.7 ms | 229.2 ms |
| the window composing and presenting | 34.3 ms | 35.5 ms |

| a key, with the card streaming | after the key |
|---|---|
| its drawing begun by the drawing thread | 116–182 ms |
| the first screen's renderer answering with it | 326–392 ms |
| presented | **376–448 ms** |

| a key, the card closed | after the key |
|---|---|
| begun | 0.1 ms |
| answered | 51.5–53.3 ms |
| presented | **84–90 ms** |

Chromium streamed 59.6 frames a second of about 216 KB, **13 MiB a second,
under ADR 0090's 24**: the pace never held a frame back, so it was not the
cause. The channel to the first screen's renderer was the cause. Each frame of
the card was a new first screen: its PNG went into the first screen's
document as the card's `<img>`, and the whole first screen was built, laid
out and painted again around it. That took 204 ms, against 53 ms with no
card. So the renderer that draws the box drew only four frames a second of the
card, and was always busy with one. A key's edit waited for the frame being
painted, about 160 ms, and was then painted with the card again, about 205
ms. A heavier page made each paint longer, hence the second seen by hand. The
window itself redrew for every frame Chromium sent, 22 times a second, and
was 76% busy doing it.

## Decision

### 1. The first screen leaves the card's page clear

Once the live card has a frame, the first screen is asked for with its
picture **beneath** it (`Card::picture_at`, `PictureAt::Beneath`): the
frame's bytes are not sent, the card's `<img>` is laid out and read by an
agent exactly as before — the page, as an image named *The page, as Chrome
last drew it* — and its place is painted **clear**: a display item of its
own (`alo_paint::DisplayItem::Beneath`, `alo_paint::beneath::clear`) that
takes away each pixel's alpha by as much as the picture would have covered
it inside the clip in force, and keeps its colour. The renderer says where
it left the place, in device pixels of the frame it painted
(`ToRenderer::PictureBeneath`, `alo_renderer::beneath`), told to the window
just before that frame (`Drawn::Beneath`).

A new frame of the page is then the same first screen, and is not sent to
its renderer at all. Only the card's first frame asks for the first screen,
since it is what makes the `<img>` appear.

### 2. A renderer of its own draws the page

The window sends the newest frame, and the size of the place, to a thread
of its own holding a confined renderer of its own (`conversation_picture`,
site `alo:card-picture`, `Renderers::twin`). That renderer decodes the PNG
through the one picture path every `<img>` takes and draws it into exactly
that many device pixels (`ToRenderer::CardPicture`,
`alo_renderer::card_picture`). Of the frames waiting it draws only the
newest; the same frame at the same size is not asked twice; each picture
is numbered, and `drawn_as_asked` waits for the newest as it waits for the
parts' drawings (queue item 862). The stranger's bytes are still decoded
only in a confined process (ADR 0005, ADR 0076 B); a renderer that goes is
said once and not asked again.

### 3. The window composes the page beneath the first screen

The window draws the picture at its place inside the first screen's room,
over the colour the first screen kept where it cleared it, and then the
first screen's frame over it (`conversation_compose`). Two things follow.
Whatever the first screen draws over the place still stands over the page:
its rounded corners, alo's menu over the page, words offered over the box.
And a page's translucent pixels show the card's surface through them, as
they did in the card.

That is the card drawn with its picture in it, which a test holds. Against
the committed `card-holding-a-frame.png` at one, and against the card drawn
then at two, no channel of any pixel is more than one level apart: 18
pixels at one and 22 at two, all on the rounded corners' soft edges, which
are written as bytes once more. The same holds for a frame with clear and
half-seen pixels, at one and at two. A frame of the card asks for no
redraw of the window until its picture is drawn.

**After**, measured the same way with the same page:

| a key, with the card streaming | after the key |
|---|---|
| its drawing begun | **0.1 ms** — never behind a frame of the card |
| answered | 137–143 ms |
| heard by the window | 0.1–0.2 ms after the answer |
| presented | **173–180 ms**, one redraw (36 ms) after it was heard |

A frame of the card costs its own renderer 66 ms (p90 78) at two, and 15.1
pictures a second were drawn — the card **streams at about four times the
rate it did** (4.2 frames a second before). The window composes in 20.7 ms
and presents in 36, 74% busy while the card streams.

## ADR 0090 holds

The pace is unchanged and still applies to the card's frames alone
(`chrome_screencast`). The first screen's parts never went through it, and
were not held back by it, as measured above. Nothing of ADR 0090 is
amended.

## Rejected

- **The box's drawings first, on the one renderer.** A drawing being
  painted cannot be stopped, and the first screen's renderer was always
  painting one of the card's frames. Putting the edit first in the queue
  still leaves it waiting up to 230 ms, and then paying for the card in its
  own paint.
- **The card's picture drawn by the window, over the first screen.** That
  would cover alo's menu over the page, the offers over the box and the
  card's rounded corners, everything the first screen draws over the page.
  It would also have the browser process decode a stranger's PNG, which
  ADR 0005 refuses.
- **A cache of the first screen, its picture painted again alone.** Still
  the one renderer: an edit would still wait behind a frame being painted,
  and the paint would need a split of the display list that a backdrop or
  an opacity group would make wrong.
- **Two renderers drawing the whole first screen in turn.** Frames would
  come back from states that are not the newest, and a press is decided
  against the picture that was pressed. Which picture that was would no
  longer be one renderer's to say.
- **Taking the card's pictures as opaque.** This was the first assumption,
  and it was measured wrong by hand. The Chromium alo carries draws a page
  with no background of its own as clear, so its card showed the window's
  paper (243, 241, 237) instead of the card's white. The colour kept under
  a cleared pixel is what makes it right (`07-a-page-of-no-background`).

## What it costs

- One more confined renderer, and its thread, while the conversation's
  window is open, whether or not a card is.
- The card's place in the first screen's frame is clear until the
  picture of a new size is drawn: resized, the place shows the last
  picture at its corner, cropped or with the card's surface beside it, for
  one picture's drawing (66 ms at two).
- **An edit is still not drawn at once**. It is drawn in the first frame the
  first screen's renderer paints after the key, 137–143 ms with a card on
  the first screen against 53 ms with none. The card's own turn costs that
  paint: its box shadow is blurred at two, and the conversation around it is
  laid out and drawn again. One redraw of the window follows, 36 ms. Neither
  is about the card's frames, and each is queue item 870.
- The window composes the page box a pixel at a time, as it composes
  everything; an opaque pixel of the page is stored, a translucent one
  blended twice.

## Built

Queue item 869: `alo_paint::beneath` and `DisplayItem::Beneath`,
`PaintContext::beneath`; `alo_renderer`'s `Resource::beneath`,
`Pictures`' beneath boxes, `card::PictureAt`, `card_document::frame`,
`beneath` (where, `PictureBeneath`), `card_picture` (`CardPicture`,
`draw`, `site`), `wire/beneath.rs`, `Renderers::twin`; `alo-window`'s
`conversation_picture` (`CardPictures`), `ConversationDrawing::picture`,
`pictured_through`, `forget_picture`, `Drawn::Beneath` and
`Drawn::Pictured`, `conversation_window/beneath.rs`, `ConversationScene::
beneath` and `first_screen_seen`. Tests: `the_cards_page_beneath.rs`
(the place in numbers at one and two, composed against the reference and
the card drawn at two, a translucent frame), `messages_across_a_boundary.rs`
(each crosses, every hostile one refused by name, every truncation
refused), `an_edit_beside_a_streaming_card.rs` (through the window's own
road: no frame of the card draws the first screen, an edit typed between
two frames is the first frame painted after it, another card never shows
the first card's page), and units in each file.
