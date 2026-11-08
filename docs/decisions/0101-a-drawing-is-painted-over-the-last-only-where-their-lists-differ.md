# ADR 0101 — A drawing is painted over the last only where their lists differ

**Status:** accepted — built by queue item 870 (7 November 2026); § 2 extended and what was left answered by ADR 0102 (queue items 871, 872); § 3 amended by ADR 0104 (queue item 885, a backdrop reads pixels of its own crop)
**Date:** 2026-11-07
**Context:** ADR 0100 (a card's page drawn beneath the first screen by a
renderer of its own, which left an edit's own paint at 137–143 ms with a
card against 53 with none); ADR 0057 § 4 (a list drawn at its scale onto a
canvas of device pixels); ADR 0074 (the first screen held by a confined
renderer, built again from state for every change); queue item 179 (a
picture's first draw decides its copy's mipmap levels); queue items 869 and
870; `CLAUDE.md`, *correct before fast* and *every rendering decision is
testable*

## The question in one line

With a card on the first screen, why did the first screen's renderer take
three times as long to answer a letter typed into the box as it did with no
card, and how is that time not spent?

## Measured

**On this Mac** — macOS 26.5, the Retina display at two at the origin —
through the release application, alo's CEF host holding the local page of
ADR 0100 (`127.0.0.1:8739`, a box turning and a bar sliding under CSS
animations), the window at 1280 × 852 points; timestamps put, for the
measurement only, where a key reaches the window, where the drawing thread
asks the first screen's renderer and hears its frame, and where the frame is
presented; ten letters typed through System Events, 0.7 s apart.
`handrun40/measured-before.txt`, with painting over switched off in the
same tree, matches 869's numbers:

| a letter typed | renderer, ask to answer | presented after the key |
|---|---|---|
| with the card streaming | 140.9–162.0 ms (median 143.7) | 155–190 ms (median 179.7) |
| no card (closed) | 51.5–54.6 ms (median 53.0) | 84.5–87.9 ms (median 86.1) |

**Where the 143 ms went**, measured in the renderer itself at two on the
first screen of the window's size (1040 × 820 CSS pixels, 2080 × 1640 device
pixels) with a card whose page is beneath, timestamps put in
`alo-renderer`'s pipeline and `alo-paint`'s renderer and taken out (release;
a letter with no card in brackets):

| stage | with a card |
|---|---|
| sheets, cascade, boxes | 1.6 ms (1.3) |
| layout | 1.9 ms (2.1) |
| display list built | 0.3 ms (0.4) |
| **painted** | **103 ms** (21.7) |
| the frame made bytes | 4 ms (4) |

and of the 103 ms painted, item by item:

| item | ms |
|---|---|
| the card's two box shadows, blurred at two | 26.5 + 22.3 |
| the page's background, the whole canvas | 13.4 |
| the card's fill and border | 9.3 + 3.6 |
| four clips around the card (conversation, card, page box twice) | 2.0 + 4.7 + 4.7 + 4.3 |
| the place left clear for the page (`Beneath`) | 5.7 |
| the box, its text, its caret and everything else | about 6 |

**The cause:** every letter is a new first screen, and the first screen was
painted whole. The card's shadows, fills, clips and the place left clear for
its page were all painted again for a letter that changed none of them.
That was 83 ms of the 103. Style and layout were not the cause (4 ms, the
same with no card), nor was the display list (0.3 ms), nor any PNG (a frame
crosses as RGBA bytes). With no card the background alone is most of the
21.7 ms.

## Decision

### 1. The last drawing's canvas is kept, and painted over

The easel hands its last drawing to the next draw, and keeps it across a
new page and a page left (`Easel::before`), since the first screen is a new
page for every letter. When the canvas it holds is the same size and was
made in the same colour, the new list is painted over it (`alo_renderer::
paint_over`); a canvas somebody still reads is copied first. Otherwise it is
painted whole, as before.

### 2. Only where the two lists can differ is painted again

`alo_paint::damage` says where two display lists can draw differently, and
it is the part that has to be right. It works in three steps:

- **The lists are lined up in order.** Areas a pointer finds boxes by are
  left out. The items both lists start and end with are matched, and the
  rest by the longest run alike, up to 2¹⁸ pairs. Two items match only
  when they **draw alike** (every field that reaches a pixel bit for bit
  equal, a picture the very same picture, a face the very same face,
  `Font::is`) **inside pushes that draw alike, in the same order**. Those
  pushes are the same clips under the same transforms into the same
  groups. They are compared outwards, as far as the pushes both lists
  begin with.
- **Every unmatched item is counted where it can reach** (`alo_paint::
  extent`). That reach comes from the very bounds `render` draws it in: a
  shape's points under its transform, whole pixels outwards and one more,
  and a blur's whole reach beyond. A text item is outlined as it is drawn.
  A kind whose reach is not that plain is unknown.
- **The answer is the whole canvas** when there is an unmatched group push
  or pop, or an unmatched picture. It is also the whole canvas when pushes
  do not nest as one stack, when a reach is unknown, at another scale,
  when the middle is too long to line up, or when the damage reaches half
  the canvas.

This is correct because of how `render` draws. What an item does to a pixel
depends only on the item, on the pushes open around it, on that pixel's
colour, and, for a picture, on the pictures drawn before it. Take a pixel
that no unmatched item can reach. It sees the same matched items, in the
same order, drawn the same way. Every group and picture is matched, so it
also sees the same groups drawn back and the same pictures first drawn in
the same order. So it ends the same colour.

### 3. The canvas is confined to that region, and the whole list drawn

`alo_paint::repaint` puts the region back to the colour a new canvas starts
in, confines the canvas to it (`Canvas::confine`), and draws the whole new
list. Every pixel changes only through four methods of `Canvas`, and each
refuses a pixel outside the region. Nothing `render` draws reads one pixel
to write another. So every pixel inside the region comes out exactly as a
canvas painted whole would hold it, and every pixel outside is one the two
lists draw alike.

What this saves is what a confined `render` passes over:

- a fill, a shadow or a place left clear that cannot reach the region, or
  what the clips in force let through of it;
- a clip that cannot reach it, whose mask is never made;
- the rows and columns of a coverage outside it.

Everything else is drawn, and the canvas keeps it in.

**After**, measured the same way with the same page
(`handrun40/measured-after.txt`):

| a letter typed | renderer, ask to answer | presented after the key |
|---|---|---|
| with the card streaming, each after the first | **33.5–37.7 ms (median 35.3)** | 42–69 ms (median 64) |
| no card | 32.7–37.8 ms (the first 51.6) | 65.8–72.5 ms (the first 84.7) |
| with the card, the first letter | 151.9 ms | 190 ms |

In the renderer itself, with no clock between processes, a letter with a
card at two now repaints a region of about 100,000 of the frame's 3.4
million pixels (the box and the offers under it) in 21 ms. With no card it
also takes 21 ms; whole, the two took 113 ms and 30 ms.

## What is left, and why

- **The first letter beside a card is still painted whole** (151.9 ms). It
  opens the offers under the box, and the conversation's room shrinks
  with them: its clip ends at 590 CSS pixels where it ended at 716. The
  card really is clipped anew there, so every item of the card is
  unmatched, and its shadows reach past half the canvas. Emptying the box
  closes the offers and does the same. Painting that cheaply needs the
  card's shadows and fills drawn only where they change, through a clip
  that differs only in its foot. That is queue item 871.
- **The window's own redraw** after the answer, 26–38 ms (once 7), and the frame
  carried whole between processes are queue item 872. A letter's frame is
  answered in 35 ms on hardware against 21 ms in the renderer alone, and
  the gap between the two is not measured apart.

## Rejected

- **Making the coverage of a shape only inside the region.** That would be
  cheaper for the card's shadows in the first letter. But the rented
  rasteriser fills a path moved by the region's corner and refines tiles
  laid from it, and a pixel filled from another origin is not shown to be
  the same pixel. Law 3 asks for that before the speed. Item 871 has to
  answer it first, or to cache what was filled.
- **Avoiding the conversation's layout for an edit.** Measured at 1.9 ms,
  the same with no card, so it was never the cause.
- **Tiles, painted again by a dirty bit.** A tile still needs to know which
  items changed under it, which is this same comparison. And every tile
  the card's shadow crosses would be painted again whole.
- **Telling the renderer what changed, from the window.** The window knows
  a letter was typed, not what the letter changes: the offers, the room
  and the card's clip all follow from layout. Comparing what was drawn is
  the one answer that cannot disagree with the drawing.
- **Prefix and suffix alone.** This was the first cut. It painted the card
  again whenever the offers' items came before the card's and the box's
  after it, so every letter's middle held the card.

## What it costs

- Every renderer keeps its last drawing's canvas past the page it was of,
  until its next drawing: about 54 MB at two for the first screen, which
  it held anyway between drawings.
- Two lists are compared at every drawing: linear in their length, plus at
  most 2¹⁸ comparisons of what differs between them. That is nothing
  beside a paint. A page of 2.5 million pictures, drawn once, took 386 s
  in its test against 403 s before.
- A drawing painted whole is drawn exactly as before: `render` passes
  nothing over on a canvas that is not confined.

## Built

Queue item 870: `alo_paint`'s `region` (`Region`), `extent` (`Reach`,
`reach_of`), `damage` (`Damage`, `damage`), `repaint` (`Repainted`,
`repaint`), `Canvas::confine`, `confinement` and `reset`, `render`'s
confined passing over (`Seen`); `alo_text::Font::is`; `alo_renderer`'s
`paint_over`, `pipeline::draw_over`, `Drawing::ground` and
`Drawing::repainted`, and `Easel`'s kept drawing. Tests:
`alo-renderer`'s `an_edit_paints_again_only_what_it_changed.rs` (each letter
after the first beside a card and with none paints again at most a
twentieth of the frame; every edit — the offers opening and closing, a
letter, words, a selection, typing over it, the caret moved, the card
closed — exactly the frame a renderer that never drew paints, every byte,
at one and at two), and units in each new file.

## Amended (7 November 2026, queue items 871 and 872)

ADR 0102 answers both items left above. § 2 now also matches an item inside
pushes alike **but for clips**, and counts it only where it can reach and
where the masks of the clips that differ differ. Those masks are measured on
the very masks `render` makes. The coverage a drawing makes is kept for the
next, bit for bit by what it was made from (`alo_paint::Kept`). That is the
answer this ADR's rejection of filling inside the region asked for: what was
filled is kept, not filled again from another origin. The first letter
beside a card is answered in 37–47 ms on this Mac, from 145–169. The
window's redraw and the frame carried whole are 0102's §§ 3–4.

## Amended (8 November 2026, queue item 885)

§ 3's *nothing `render` draws reads one pixel to write another* is no
longer whole: a box's backdrop (`backdrop-filter: blur()`, ADR 0104)
reads the pixels of its own crop, and writes only inside it. § 2 now asks
of each backdrop matched in both lists whether a change drawn before it
reaches its crop: if one does, its whole crop is damaged, so a repaint
draws every pixel it reads afresh first; if none does, its blur is the
last drawing's, kept (`Kept::backdrop`) and taken as kept. ADR 0104 § 2
says why that is right.
