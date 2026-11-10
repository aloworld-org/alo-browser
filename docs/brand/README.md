# alo browser's mark

The owner's logo, supplied by the owner on 9 November 2026 and kept here
exactly as supplied. It is the owner's: nothing in this repository edits it,
and everything that shows it — the application's icon, the mark in the chats
panel and the rail, the README — copies it as it is (queue item 913), or,
at small sizes, copies the one derived file below.

- **`alo-browser-icon.svg`** — the icon, 512 by 512: a cream tile rounded 104,
  a navy globe of two arcs, and a teal swoosh ending in a dot. It defines a
  drop shadow (`feDropShadow`) it never uses; this engine skips it, as a
  browser does.
- **`alo-browser-icon-small.svg`** — *derived*, not supplied: the icon for
  small sizes (queue item 914). At 16 to 56 device pixels the owner's thin
  navy arcs fade to grey, so this copy thickens them without redrawing
  anything: every path, colour and coordinate is the owner's, and each of
  the three paths is also **stroked 14 of 512** in its own paint — the two
  navy paths in `#1b3e5e`, the swoosh in its gradient — with round joins
  and round caps. The swoosh's dot grows with the stroke; nothing is moved
  or removed. Its gradient is named `teal-small` rather than `teal`, so
  that both drawings can stand on one page. The width was chosen by
  looking at 8, 12, 14, 16, 20, 24 and 28 at 16 to 64 pixels, at one and
  at two: under 12 the 16-pixel picture stayed grey and 12 was still
  lighter than 14; at 16 and over the gaps between the arcs began to close
  at 48 and 56, where the marks inside alo are drawn.
- **`lockup.png`** — how the icon is meant to stand beside the words
  *alo browser*: a bold geometric sans in the navy, on a warm off-white. A
  reference picture only; nothing draws it.

## Its colours

| What | Colour |
| --- | --- |
| The tile | `#fbfaf8` |
| The globe's arcs | `#1b3e5e` |
| The swoosh, a gradient left to right | `#1d6f84` to `#149391` |

The swoosh's gradient is a `<linearGradient>`, which this engine draws since
queue item 275 (ADR 0107); before it, the icon could not have been drawn by
alo itself.

## Where it is shown

**Which drawing, by size.** A picture **under 64 device pixels** a side is
the small drawing; 64 and up is the owner's, as it is (ADR 0067 § 6, amended
for queue item 914).

- **The application's icon** — `crates/alo-corpus/cases/the-applications-icon`
  draws the icon on Apple's grid (824 of 1,024, 100 around it) with the soft
  shadow the icon has always had, at every size an `.icns` holds; `alo-bundle`
  packs those pictures (ADR 0067 § 6). The 16- and 32-pixel pictures are the
  small drawing (a `max-width: 63px` media query on the page); 64 to 1,024
  are the owner's.
- **Inside alo** — the chats panel's head shows it, 24 square, before the word
  *alo*; the rail, which has room for one mark, shows it alone, 28 square
  (`crates/alo-renderer/interface/panel.html`, `rail.html`). Both are the
  small drawing: 24 and 28 are 48 and 56 device pixels at two, the most a
  Mac's screen has, so under 64 on every screen.
- **The README**, at its top.

Tests hold every copy to its file: `the_applications_icon.rs` and
`the_mark.rs` fail if a copy's drawing is not the owner's or the small one,
and `the_mark.rs` fails if the small file is anything but the owner's with
those strokes.
