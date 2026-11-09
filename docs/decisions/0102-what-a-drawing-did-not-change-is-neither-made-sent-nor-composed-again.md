# ADR 0102 — What a drawing did not change is neither made, sent nor composed again

**Status:** accepted — built by queue items 871 and 872 (7 November 2026); § 2 amended for queue item 892 (what a drawing does not use laid aside); §§ 2–4 amended for queue items 894 (a backdrop's blur and a picture laid aside) and 896 (a patch per piece, composed per piece)
**Date:** 2026-11-07
**Context:** ADR 0101 (a drawing painted over the last only where their
lists differ, which left the first letter beside a card painted whole and
the window's redraw untouched); ADR 0100 (the card's page drawn beneath the
first screen by a renderer of its own); ADR 0074 (the panel and the first
screen, each drawn by a confined renderer on the drawing thread); ADR 0024
§§ 1–2 (the window composes as a function of what it was sent, and
`softbuffer` puts it on screen); ADR 0005 (what crosses between processes
is read-only pixels); queue items 870, 871 and 872; `CLAUDE.md`, *correct
before fast* and *every rendering decision is testable*

## The question in one line

After ADR 0101, why did the first letter typed beside a card still take
152 ms in its renderer, and why did the window take another 26–38 ms to
show any letter once the renderer had answered?

## Measured

**On this Mac** — macOS 26.5, the Retina display at two at the origin —
through the release application with alo's CEF host, the window at 120,
60, 1280 × 852 points, ADR 0100's local page streaming as a card
(`http://127.0.0.1:8739/`, a box turning and a bar sliding). Timestamps
were put on the wall clock, for the measurement only, where a key reaches
the window, where the drawing thread asks the first screen's renderer and
hears it, in the renderer as it reads, answers and writes each message, as
the window hears the frame, and before and after it composes and presents.
Ten letters, ten Backspaces and five letters were typed through System
Events, 0.7 s apart. The same timestamps were put into a worktree of
eb4b0c4 for *before*. `handrun41/measured-before.txt` and
`measured-after.txt` hold every row.

### Where the first letter's 152 ms went (871)

The first letter opens the offers. In the display lists, the box stays
where it was, the offers come in above it, and the conversation's clip
ends at 590 CSS pixels where it ended at 716. **The card itself is not
moved and not changed:** every item of it is bit for bit the same, inside
a clip whose path differs only in its foot. ADR 0101's comparison matched
an item only inside pushes that draw alike, so every item of the card was
unmatched. Its two shadows reach past half the canvas, so the whole frame
was painted again: 113 ms in the renderer alone, 103 of them paint, and 49
of those the two shadows blurred at two.

The pixels that can actually change are fewer. They are the box and the
offers, and the strip of the card between the two feet: 1888 × 377 device
pixels, a fifth of the frame.

### Where the window's redraw went (872)

In the window at two, the frame for one letter at eb4b0c4, per letter beside
the card (median of 25):

| step | ms |
|---|---|
| the renderer making the frame bytes (`Frame::from_canvas`, 13.6 MB) | 4.5 |
| written down the pipe and read | 5.5 |
| the window waiting for its event loop (a card picture being composed) | 0–42 (median 26) |
| composing the whole window in floats, 4.4 million pixels | **22.4** |
| making each pixel a word and presenting | **15.2** |

So, with no card, a letter took 66–73 ms from key to screen, and 33–38 ms
of that was after the renderer answered. Composing and presenting were the
two largest steps. A card picture costs the window the same compose and
present, about 38 ms of its event loop 15 times a second. That is the
0–42 ms a letter waited before its own frame was even looked at.

Measured off the window, in a temporary example at the window's size:
composing 24.7 ms, turning pixels into words 7.0, `write_from_renderer` 0.7,
`read_from_renderer` 0.3, `Frame::from_canvas` 5.0.

## Decision

### 1. An item may be matched inside a clip that differs (871)

`alo_paint::damage` matches two items that draw alike inside pushes that
draw alike **but for clips**. Those are the same transforms and groups in
the same order, where a clip of one list may have another shape than the
other list's clip at the same place. Each such pair of clips is measured
on the very masks `render` makes for them (`alo_paint::clip::mask`, one
function for both). `clip::differ` gives the smallest region where the two
masks let different amounts through. The item is counted only where it can
reach **and** that region is.

This is correct for the same reason ADR 0101 § 2 is. `render` multiplies an
item's coverage at a pixel by each open clip's mask at that pixel
(`Clip::intersected`, `Clip::at`), and by nothing else of the clip. Where
every pair's masks are equal, the clip it is drawn through is equal, so the
item draws alike. A group or picture matched this way is the whole canvas,
since neither is drawn only through a mask. The items both lists begin
with are still matched only exactly, since § 2's shortcut takes a push
there to be the very same push.

### 2. Coverage is kept from one drawing to the next (871)

A shape's coverage, a shadow's blurred coverage, a clip's mask and the
clips open around an item combined are each a function of nothing but the
shape under its transform, the rule, the page's size and the blur. So the
coverage a drawing made is kept with it (`alo_paint::Kept`, carried in
`Drawing::kept`), keyed by those numbers **bit for bit**. The next drawing
takes it rather than making it again (`render_keeping`, `repaint`). A kept
coverage is the very coverage that would have been made, so no pixel can
differ.

Only what the last drawing's list draws, passes over or compares is kept.
Everything else is let go as each drawing ends. The total is capped at 64
MiB: past that a coverage is made and not kept, which costs time and never
a pixel. A first screen at two with a card keeps about 30 MB.

A confined `render` now also keeps a place left clear to the region's rows,
as it already kept a fill, and a `fill_rect` to the confinement.

### 3. A renderer sends only what changed since the frame it last sent (872)

A new ask, `ToRenderer::PaintOver`, is sent by an asker that holds the last
frame this renderer sent. The easel joins what every drawing since that
frame painted again (`Repainted::joined`) and starts again from nothing
when a frame is sent. The answer is `FromRenderer::PaintedOver(Patch)`: the
frame's size, that rectangle, and its pixels, made bytes for that rectangle
alone. It is the whole frame, `Painted`, when the change is every pixel,
as for a first frame or one of another size.

The conversation's drawing thread asks each part this way once it holds a
frame. It puts the patch over the frame it holds (`Patch::onto`), and tells
the window the whole frame, as before. A patch not of the frame it holds is
not believed: the whole frame is asked for. On the wire, a patch's
rectangle must lie inside its frame and carry exactly its pixels, or it is
refused.

### 4. The window composes again only what changed under it (872)

The conversation's window keeps its pixels and their words between
redraws (`alo_window::conversation_composed::Composed`). It also keeps what
they were made from. Each redraw compares the new scene with that:

- the window's size, scale, side, the rail's name, both lines, where
  everything is placed, and whether the sheet is out — another, and
  everything is composed again;
- the panel's frame — its place;
- the first screen's frame — the smallest rectangle where its bytes differ;
- the card's page beneath — its place before and after.

Only that region is put back to the paper and composed again
(`compose_within`), exactly as composing the whole window composes it. Then
only its words are made again. Composing is a pixel's own arithmetic onto
that pixel, so a pixel none of whose inputs changed comes out as it did.
`Presenter::show_words` copies the kept words to the window server.

## After

Measured the same way (`handrun41/measured-after.txt`), median of each:

| a key, the Retina display at two | renderer, ask to answer | presented after the key |
|---|---|---|
| beside the streaming card, the first letter (opens the offers) | **39.5 ms** (from 168.8) | **81 ms** (from 231) |
| beside it, emptying the box (closes them) | 32.8 (from 145.2) | 68 (from 183) |
| beside it, the first letter again | 37.4 (from 144.5) | 56 (from 200) |
| beside it, each other letter or Backspace | **17–23** (from 33–40) | **30–35, at most 57** (from 76–140) |
| no card, the first letter | 35.2 (from 40.8) | 55 (from 74) |
| no card, each other | 19–23 (from 33–38) | **32–38** (from 65–73) |
| `en.wikipedia.org/wiki/Kampala` as a card, the first letter | 47.0 (from 135.5) | 74 (from 171) |
| it, each other | 18–21 (from 32–34) | 31–35 (from 67–70) |

Per letter after the renderer answers: 0.4–0.7 ms read, composed in 2–3 ms
over about 70,000 pixels (the first letter 7 ms over 697,000), presented in
**10.4 ms**. A letter crosses in 0.3–0.4 MB, not 13.6. The first letter
crosses in 2.8 MB.

**What is left, named with numbers:**

- **10.4 ms to present.** `softbuffer` 0.4.8 on macOS hands the window
  server a new buffer for every frame (`buffer_mut` makes one zeroed,
  `age` is always 0). So the window's 17 MB of words are copied whole into
  it, and a `CGImage` is made of them, every time. Doing less needs a
  surface that keeps its last frame. On macOS that means `IOSurface` or
  Metal, reached through `unsafe` this repository has not reviewed, which
  is a decision of its own (queue item 873).
- **0–25 ms behind a picture of the card.** The window composes each new
  picture of the page beneath on its event loop: its place again, about
  1.8 million pixels, and then the same 10.4 ms present. A letter that
  arrives during one waits for it, which explains the 57 ms above. Doing
  less needs the picture composed off the event loop, or presented by the
  window server as a layer of its own (873).
- **The first letter beside a card, 35–47 ms in its renderer.** It is
  nearly what the first letter takes with no card (35 ms), and most of it
  is the strip of 1888 × 377 pixels painted again, with the three clips
  inside the conversation's clip combined again over the card (about 2
  million pixels each).

## Rejected

- **Making a clip's mask only where it differs.** Two rectangles that
  differ in their foot could be said to differ only in the rows between
  them. But the rented rasteriser (`tiny-skia`, refined by
  `crate::refine`) is not shown to fill a pixel from its neighbourhood
  alone, which is ADR 0101's reason for keeping coverage whole. The masks
  are made, compared and kept instead, for 2–4 ms on the first letter.
- **Telling the window what the renderer repainted, rather than comparing
  frames.** The window would trust a renderer's word about which of its
  pixels to keep. The window compares bytes it holds, which costs a
  memory comparison of 13.6 MB (about 1 ms), and cannot be wrong.
- **The window putting patches over its own frame.** That would save the
  drawing thread's copy of the frame (about 1.5 ms). But every test that
  reads the drawing thread's channel would then have to apply patches, and
  a frame the window lost would make every later patch wrong. The thread
  keeps the frame. The window is told whole frames, as ADR 0074 has it.
- **Composing in bytes rather than floats.** Faster, and a different
  rounding of every translucent pixel from the reference renders. Law 3
  keeps the arithmetic and composes less of it.
- **Keeping only the costly coverage.** Which coverage is costly depends on
  the page. Keeping all of it under one cap is simpler and is still
  bounded.

## What it costs

- Up to 64 MiB of kept coverage per renderer, beside the 54 MB canvas ADR
  0101 keeps. About 30 MB for the first screen at two with a card.
- The conversation's window keeps its canvas in floats (70 MB at two, as it
  made each redraw anyway), its words (17 MB) and a copy of each frame it
  was composed from. The drawing thread keeps each part's last frame.
- A renderer's protocol gains one ask and one answer, each read as hostile
  from a renderer. The tabs window and every other asker still ask
  `Paint`, and are sent whole frames as before.
- Every drawing that keeps coverage hashes its shapes' points. That is
  linear in the list, and nothing beside a paint.

## Built

Queue item 871: `alo_paint::clip` (`Clip`, `mask`, `differ`, `multiply`,
moved out of `render`), `alo_paint::kept` (`Kept`, `Making`, `Key`,
`MOST_BYTES`), `render_keeping`, `damage`'s matching inside clips that
differ (`inside_alike`, `Masks`, `Lined`) and its `Kept`, `repaint`'s
`Kept`, `beneath::clear` and `Canvas::fill_rect` kept to the confinement,
`Drawing::kept`. Queue item 872: `alo_renderer::frame_patch` (`Patch`),
`ToRenderer::PaintOver`, `FromRenderer::PaintedOver`, `wire/patch.rs`,
`Easel::send` and its `unsent`, `Repainted::joined`, the drawing thread's
held frames; `alo_window::conversation_composed` (`Composed`,
`Recomposed`), `conversation_compose::compose_within` and `beneath_rect`,
`Presenter::show_words`. Tests: `alo-renderer`'s
`an_edit_paints_again_only_what_it_changed.rs` (the offers opening and
closing beside a card paint at most a quarter of the frame and blur no
shadow; every edit still every byte as drawn whole),
`a_frame_crosses_only_what_changed.rs` (a letter crosses in at most a
twentieth of the frame's bytes, the first in a quarter, nothing changed in
none; two drawings between asks cross together; a whole frame starts the
changes again; another size crosses whole; every held frame is the frame
sent whole, every byte), `messages_across_a_boundary.rs` (the ask and the
answer cross; a patch past a side or foot, of more or fewer pixels, and
every truncation refused); units in `clip` (3), `kept` (3), `damage` (the
clip that differs, now only where its mask does), `repaint` (a clip's foot
moved, every pixel as drawn whole, no shadow blurred again), `frame_patch`
(3) and `conversation_composed` (3, each every word as composed whole).

## Amended (9 November 2026, queue item 892)

§ 2 let go of every coverage the last drawing did not use. *Settings*
closed then lost its panel's two shadows, and every opening blurred them
again: 42 of opening's 54 ms of shadow at two. Now, as a drawing ends,
what it did not use is **laid aside**, the most recently used first, up to
`ASIDE_BYTES` (24 MiB) inside the 64 MiB cap; what a drawing makes is
given room by letting the oldest laid aside go first. A coverage is a
function of its key alone, so what was made under a key many drawings ago
is what this one would make: the next opening takes the panel's shadows,
fills and clips as they were (`Making::aside` counts them), and paints
every pixel as drawn whole (`settings_opened_again_takes_what_it_made.rs`,
at one and two). A backdrop's blur is never laid aside: it is a function
of the pixels under it too (ADR 0104 § 2). **What it costs:** the first
screen at two keeps about 56 MB with *Settings* closed where it kept
about 31 — still under the cap, which bounds it.

## Amended (9 November 2026, queue items 894 and 896)

**§ 2: a backdrop's blur is laid aside too (894).** 892 never laid one
aside, its blur being a function of the pixels under it as well. It is now
kept with the items drawn before it (ADR 0104, amended), so it is laid
aside with them and taken by a drawing whose items before it draw alike;
one blurred on a canvas whose colour was not said is let go as before.
`ASIDE_BYTES` grew from 24 to 32 MiB for it: *Settings* closed over a card
at two lays aside about 31 MB, its veil's blur 3.4 of them, and the first
screen keeps about 62 MB in all, under the 64 MiB cap. And a picture a
drawing did not show is laid aside by the renderer, up to 32 MiB of pixels
(`alo_renderer::pictures::Decoded`): a card's frame drawn under *Settings*,
then beneath the first screen while it is closed, is the very same picture
— not decoded again, and alike to the list its veil's blur was kept with.

**§ 3: a patch per piece (896).** A renderer sent the rectangle holding
what every drawing since its last frame painted: two rings far apart in
*Settings* at two crossed as 721,392 pixels where 196,200 were painted.
Now it sends each piece (`frame_patch::Patch`'s `pieces`, `unsent::Unsent`):
the pieces of every drawing since, joined as a drawing's places are joined
(`alo_paint::pieces::cut`) once there are more than eight. On the wire each
piece is read as hostile on its own — inside the frame, exactly its
rectangle's pixels — and no more than eight are read, the count said and
refused before any piece is.

**§ 4: composed per piece (896).** The window compared frames and composed
the smallest rectangle holding every byte that differs. It still compares
— it never takes a renderer's word for which of its pixels changed — but
by runs of rows that differ, each the smallest rectangle holding its
differing pixels, joined as a drawing's places are. Measured in the window
alone (release, this Mac, *Settings* at two, *Close* → *Font*): composed
again over 196,200 pixels in 2.19 ms where it was 721,392 in 5.01.
