# alo browser's mark

The owner's logo, supplied by the owner on 9 November 2026 and kept here
exactly as supplied. It is the owner's: nothing in this repository edits it,
and everything that shows it — the application's icon, the mark in the chats
panel and the rail, the README — copies it as it is (queue item 913).

- **`alo-browser-icon.svg`** — the icon, 512 by 512: a cream tile rounded 104,
  a navy globe of two arcs, and a teal swoosh ending in a dot. It defines a
  drop shadow (`feDropShadow`) it never uses; this engine skips it, as a
  browser does.
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

- **The application's icon** — `crates/alo-corpus/cases/the-applications-icon`
  draws this file on Apple's grid (824 of 1,024, 100 around it) with the soft
  shadow the icon has always had, at every size an `.icns` holds; `alo-bundle`
  packs those pictures (ADR 0067 § 6).
- **Inside alo** — the chats panel's head shows it, 24 square, before the word
  *alo*; the rail, which has room for one mark, shows it alone, 28 square
  (`crates/alo-renderer/interface/panel.html`, `rail.html`).
- **The README**, at its top.

Tests hold every copy to this file: `the_applications_icon.rs` and
`the_mark.rs` fail if a copy's drawing is not the owner's.
