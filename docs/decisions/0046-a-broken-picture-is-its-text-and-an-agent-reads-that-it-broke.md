# ADR 0046 — A broken picture is its text, and an agent reads that it broke

**Status:** accepted
**Date:** 2026-10-11
**Context:** queue item 413, *A broken picture is drawn as its text*, opened
by `alo-sites-rich-blocks` (iteration 264) and marked **needs ADR**. The
figure alo Sites makes of an alo Docs image block holds
`<img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB"
alt="Orange notebook <front>" width="640">`. Those 24 bytes are a PNG
signature and the start of a header, cut off: no picture, in any browser.
Every browser a person compares us with then shows the words "Orange
notebook <front>" where the picture would have been. This engine shows
nothing: the `<img>` is an empty `inline-block`, 0 × 0 on the figure's
first line, and an agent reads `image "Orange notebook <front>"` as if a
person could see a notebook.

Also read: ADR 0002 (one tree, two readers; what a person sees an agent can
read); ADR 0004 (we own the layout tree); ADR 0013 § 3 (*absent beats
approximate*); ADR 0022 § 4 and ADR 0027 § 6 (an image is named by its
`alt`, never by what is inside the file); ADR 0043 § 1 and ADR 0044 §§ 4–5
(a box made for an element takes the element's style through
`nearest_style`, and what an agent is told beside a name is a property of
the node, not text in it); `alo-renderer`'s `pictures.rs`, `resource.rs`
and `pipeline.rs` (pictures are read after the box tree is built, from the
resources a drawing was handed, all already fetched); `alo-box`'s
`tree.rs` (the `natural` side map), `natural.rs` and `semantics.rs`;
`alo-layout`'s `replaced.rs` and `engine.rs` (`is_atomic`: a box with a
natural size is atomic whatever its `display`); `alo-style`'s
`user_agent.rs` (`img` is in the `inline-block` list) and
`presentation.rs` (`width` and `height` attributes are presentation
attributes on an `<svg>` only); `alo-agent`'s `tree.rs`
(`write_described`); the cases `alo-sites-rich-blocks` and `a-picture`,
whose `.missing` picture "keeps the box its style asked for"; and the HTML
standard's rendering section, § 15.4.2 *Images*, fetched 2026-10-11.

**Evidence.** HTML § 15.4.2 applies its rules to an `img` in order, the
first that applies winning:

1. it **represents an image** — a replaced element, drawn;
2. it does not, and either the user agent has reason to believe the image
   will become available, or it has **no `alt` attribute**, or the document
   is in quirks mode and the element has natural dimensions — a
   **replaced element whose content is the text it represents**, if any,
   optionally with an icon that it is loading;
3. it **represents text** and that is not expected to change — a
   **non-replaced phrasing element whose content is the text**, optionally
   with an icon saying the picture is missing (omitted in non-graphical
   contexts);
4. it **represents nothing** and that is not expected to change — a
   **replaced element whose natural dimensions are 0**.

And what Chrome does, **measured on this machine** in Google Chrome
154.0.8037.98, headless, by throwaway pages in the scratchpad reading
`getBoundingClientRect()` and `getComputedStyle()` and by two screenshots.
Chromium's source was not read; Firefox is not installed and is not cited.

| `<img>`, its picture failed | Chrome 154 |
|---|---|
| `alt="Orange notebook"`, 16 px sans-serif | `display: inline`, 139.65625 × 18: a 16 × 16 icon then the text, whose own width is 123.65625, with no gap |
| the same at 40 px / 8 px | 325.125 × 46 / 77.828125 × 16: the icon stays 16 × 16, the text scales |
| … `width=640`, `width=640 height=100`, `width=20`, `height=50`, `style="width: 200px"` | 139.65625 × 18 every time: **a size does not apply** |
| … in a 100 px block, a long `alt` | 100 × 72: the text **wraps across four lines**, as inline text does |
| … `style="display: block"` | 740 × 18: a block as wide as its container, holding the text |
| … `style="display: inline-block"` | 56 × 16 with no size (shrinks to the text); with `width=640 height=100`, 640 × 100 |
| … `color: red`, `text-decoration: underline`, a border and padding | the text red and underlined, the border and padding around it: **the text is drawn in the element's own style** |
| the case's page, frozen | the figure's first line holds the icon and "Orange notebook <front>", 194.796875 × 18 in Chrome's sans-serif; `width="640"` does nothing |
| no `alt`, `width: 40px; height: 20px` (the `a-picture` case) | 40 × 20, a grey frame with the icon inside |
| `alt=""`, a size from CSS or from attributes | 40 × 20, the same frame |
| no `alt`, no size | 16 × 16, the icon alone |
| `alt=""`, no size | 0 × 0 |
| no `src` and no `alt` | 0 × 0 |
| no `src`, or `src=""`, with an `alt` | the icon and the text, as with a failed `src` |

## The decision in one line

An `<img>` whose picture is not shown, and whose `alt` is not empty, is
**a non-replaced element whose content is its `alt`**, laid out and drawn
as text in the element's own style, at the `display` its style gives it —
`inline` from the user-agent sheet, as every engine computes it; any other
`<img>` without a picture is **a replaced box with natural dimensions of
0**; neither draws an icon; and an agent reads every `<img>` without a
picture as **`broken`** — `image "Orange notebook <front>" [broken]`.

## Why this is a decision rather than a chore

**The box changes kind with something the box tree does not know.** Whether
an `<img>` is a picture or a run of text depends on whether its bytes
decoded, which the renderer learns after the box tree is built. Who holds
that fact, and when, decides whether layout, paint and the agent can
disagree about it.

**`img` would change its `display`.** The user-agent sheet makes it
`inline-block`, which is harmless for a picture (a replaced box is atomic
whatever `display` says) and wrong for text: an `inline-block` of text does
not wrap across lines and does take a `width`. Changing the sheet touches
every `<img>` in every case.

**An icon is a picture this engine would have to own.** HTML makes it
optional; Chrome draws one, 16 × 16 at any font size. Drawing one or not
moves every broken picture's width.

**What an agent is told is new.** A person sees text where a picture
should be — and in Chrome, an icon saying so. An agent told only
`image "Orange notebook <front>"` believes a person can see a notebook.

## 1. Which `<img>` has no picture, decided once, before the boxes

An `<img>` **shows a picture** when its `src`, trimmed, is not empty, and
the drawing was handed a resource of that name that reads as a picture —
decoded by `alo-paint`'s `picture::read`, or an SVG file read by ADR 0027.
**Every other `<img>` has no picture**: no `src`, an empty one, a name the
drawing was not handed, bytes that are no picture, an SVG file refused.

That is decided **once per drawing, by `alo-renderer`'s `pictures.rs`,
before `alo_box::build`**, and handed to the box tree, which is the only
thing layout, paint and the agent read. Today `Pictures::read` runs after
the boxes and finds `<img>` boxes to size; it becomes a reading of the
document's `<img>` elements first, so the box tree can be built knowing
which have pictures. The box tree still knows nothing about pictures (the
comment on its `natural` map stays true): it is told which elements have a
picture and at what natural size, and which have none.

**"Not expected to change"** (rules 3 and 4) is true here by construction:
a drawing is handed its pictures already fetched, so one it was not handed
does not arrive during that drawing. The next drawing asks again. Rule 2's
first branch — a picture on its way, drawn as a box of its size meanwhile —
needs a renderer that knows a request is in flight, which none here does;
it is cut to its own item (420).

## 2. With an `alt`, it is the `alt`, as text

An `<img>` with no picture whose `alt` attribute is present and, untrimmed,
not empty **represents text** (rule 3). Its box is **not replaced**: it has
no natural size, and the box tree gives it **one text box, whose text is
the `alt` as written and whose node is the `<img>`**, so `nearest_style`
answers its style with the `<img>`'s, as for a field's value. The text is
then white-space-processed, measured, broken across lines, decorated and
painted as any text is, in the `<img>`'s font, colour and decoration, as
the table's colour row shows Chrome doing.

**Its `display` is its style's.** The user-agent sheet moves `img` out of
the `inline-block` list and leaves it at CSS's initial `inline`, as Chrome
computes it. So by default the `alt` is an inline run that wraps with the
line; an author's `display: block` makes it a block holding the text; an
author's `inline-block` makes it an atomic box shrunk to the text. A
picture is unaffected: a box with a natural size is atomic whatever its
`display` says (`alo-layout`'s `is_atomic`), so every case with a decoded
picture lays out as it does today.

**A size does not apply** to the inline default, because `width` and
`height` do not apply to a non-replaced inline box — Chrome's every row.
The case's `width="640"` is therefore inert, here and there. The `width`
and `height` attributes are not presentation attributes on an `<img>` here
at all yet; that is a gap for pictures, not for this rule, and is cut to
its own item (419).

**No icon.** HTML makes it optional; § 4 says why it is left out.

## 3. Without an `alt`, it is a replaced box of nothing

An `<img>` with no picture whose `alt` is absent (rule 2, with no text and
no icon) or empty (rule 4) is **a replaced box whose natural width and
height are 0 and which has no ratio**. CSS's replaced sizing then gives it
the size its style fixes and 0 along any dimension left `auto` — never
CSS's default object size, 300 × 150, which is for content with no
dimensions at all rather than content whose dimensions are nothing.

That is today's behaviour in every number: `a-picture`'s `.missing`
picture stays 40 × 20, and an `<img>` with no `src` and no `alt` stays
0 × 0. It now holds because the box is replaced, as HTML says, rather than
because it happens to be an empty `inline-block` — which § 2's change to
the sheet would otherwise have collapsed to nothing. Chrome draws the frame
and icon in that box; this engine draws nothing in it (§ 4).

## 4. No icon and no frame, for now

Chrome draws a 16 × 16 icon before a broken picture's text, and a grey
frame with the icon inside a box that represents nothing. **This engine
draws neither.** HTML makes the icon optional in both rules, so leaving it
out is the standard and not an approximation of it. Drawing one means
owning a picture, a size rule that is Chrome's convention (16 pixels at any
font size, no gap) and a place for it in the line, for a fact § 5 already
gives the reader who most needs it. The cost is a person's: text where a
picture failed looks like text. Cut to its own item (418), opened by the
first page where that matters to somebody.

So a broken picture's text here is 16 px narrower than Chrome's, and the
`a-picture` box is blank rather than framed. Both are written in the case's
`origin.txt` when 413 is built.

## 5. An agent reads `broken`, and the `alt` once

- **An `<img>` with no picture carries `broken`**, written among its states:
  `image "Orange notebook <front>" [broken]`, `image [broken]`. It is held
  in the box tree beside the natural size, from § 1's one decision, and
  read by `alo-agent` — not computed from the document, which cannot know.
  An `<img>` whose picture is shown carries nothing, so `a-picture`'s five
  drawn pictures and `alo-meet-greeting`'s hand read as they do today.
- **Its name is still its `alt`** (ADR 0022 § 4), and the text box § 2 made
  is **not read as a node of its own**, as an image's contents never are —
  so the words are read once, as the name, not twice.
- **Why an agent and not only a person**: ADR 0002 gives an agent what a
  person sees. A person sees words where a chart, a code to type or a
  product photograph should be; an agent reading only the name would act as
  if the picture were there. When accessibility (item 130) maps this tree
  to a platform, `broken` is mapped by what that platform offers; that is
  decided there, not here.

## 6. Hostile input

No new reader of outside bytes: an `alt` is attribute text the HTML parser
already bounded, and a picture's bytes are read exactly as before. An `alt`
of a hundred thousand characters, or one full of control characters, is a
long run of text and is laid out as one; the build tests both without a
panic.

## 7. What closes queue item 413

413 closes when:

- `alo-sites-rich-blocks` draws "Orange notebook <front>" on the figure's
  first line, at its start, in DejaVu Sans at the figure's 16 px — **in
  numbers** (the `<img>` box as wide as that text and as tall as the line,
  each worked by hand in the case's `origin.txt`) and **in its picture**
  (the words' ink where the empty box was);
- its agent outline reads `image "Orange notebook <front>" [broken]` with no
  `text` node under it;
- `a-picture`'s `.missing` box is still 40 × 20 where it is, and reads
  `[broken]`; no layout number in any other case moves, and every
  reference that changes — a `boxes.txt` saying `inline` where it said
  `inline-block` of a picture, an outline gaining `[broken]` — is reviewed
  in that commit;
- unit tests show § 1's cases (no `src`, an empty one, one not handed, bytes
  that are no picture, a refused SVG), § 2's (the `alt` inline and wrapping,
  `display: block` filling its container, the element's colour reaching
  the text, `width` doing nothing to the inline default), § 3's (`alt=""`
  and no `alt`, with and without a size from CSS, 0 × 0 and the style's
  size, never 300 × 150) and § 5's (`broken` only without a picture; no
  node for the text);
- § 6's hostile `alt`s are laid out without a panic.

## What this costs

**One sheet rule, read by every `<img>`.** `img` becomes `inline`. A
picture does not move, because it is replaced; its box tree line says so
differently.

**A reordering in the renderer.** Pictures are read before the boxes are
built rather than after; their decoding does not change.

**No icon.** A broken picture's text is 16 px narrower than Chrome's, and a
person cannot tell it from the text around it.

## Alternatives rejected

**Keep `img` an `inline-block` and put the text inside it.** One line
shorter, and wrong: the `alt` would not wrap with its line (Chrome's 100 px
row) and would take the `width` an author wrote for a picture. Rejected.

**Draw the `alt` in the empty box, after layout, without a text box.** The
box is 0 × 0, so the text would be drawn where nothing made room for it,
and layout, paint and the agent would each know a different thing.
Rejected.

**Decide in the box tree by looking at `src`.** The box tree cannot know
whether bytes decoded; deciding there would draw a corrupt picture's empty
box rather than its text. Rejected (§ 1).

**Draw Chrome's icon.** A picture of our own to own, at Chrome's size rule,
for something HTML leaves optional; cut rather than rejected (§ 4).

**Name the image "Orange notebook <front> (broken)".** An agent finding
the picture by its words would not find it. The same reasoning as ADR 0044
§ 5. Rejected.

**Default object size, 300 × 150, for an `<img>` with nothing.** HTML says
natural dimensions of 0, Chrome draws 0 × 0 with no icon, and today's
`a-picture` box depends on it. Rejected.

## What this does not decide

- **An icon or frame** for a broken picture (418).
- **`width` and `height` attributes on an `<img>`** as presentation
  attributes for its size (419).
- **A picture still on its way** (rule 2's first branch, 420).
- **`<input type=image>`**, whose rule 5 is a button holding its `alt`;
  with that control.
- **`alt=""` as role `presentation`.** HTML-AAM gives such an image no
  role; this engine reads it as an unnamed `image` today, and changing what
  every decorative picture reads as is its own question.
- **`srcset`, `<picture>` and lazy loading**, none of which a page here
  depends on; `loading="lazy"` is ignored, as it is for every picture.

## How we will know if this was wrong

**If an agent acts on a picture a person could not see**, § 5 failed.

**If a decoded picture moves** when `img` becomes `inline`, § 2's reliance
on a replaced box being atomic was wrong.

**If the box tree and the renderer ever disagree** about whether an `<img>`
has a picture — text drawn over a picture, or a picture with no box — § 1's
one decision is being made twice.
