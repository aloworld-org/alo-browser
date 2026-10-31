# ADR 0084 — The interface's typefaces are compiled in, and handed to its renderers alone

**Status:** accepted — built by queue item 706 (31 October 2026)
**Date:** 2026-10-31
**Context:** ADR 0074 § 3 (the design is set in Instrument Sans and
Newsreader; carrying them is its own change, item 706); ADR 0010 (a renderer
opens no file, and fonts cross to it as bytes); ADR 0067 (what the macOS
application carries); ADR 0030 (the browser says what it is and nothing
about the machine); `CLAUDE.md`, *rent the physics*; the owner's design
canvas of 31 October 2026; queue item 706

## The question in one line

Where do the design's two typefaces live, how does a confined renderer get
them, which renderers get them, and how does their licence travel with the
application?

## 0. What the design asks for

The canvas sets every word of the interface in **Instrument Sans** at 400,
500 and 600 — `font-family: 'Instrument Sans', system-ui, sans-serif` — and
the mark *alo* and the greetings and headings in **Newsreader** at 400 and
500 — `font-family: Newsreader, serif`. It loads them from Google Fonts at
`wght` 400;500;600 and `opsz,wght` 6..72,400;500. No italic of either is
used.

## 1. The files, as published

Both are under the **SIL Open Font License 1.1**. They are taken from
Google Fonts' own repository, `github.com/google/fonts` at commit
`6c1dbe27dcf7d817d4b62cec033ba77fa8e2a5ec`, unmodified, each with the
`OFL.txt` published beside it:

| File in `crates/alo-renderer/interface/fonts/` | Published as | Bytes |
|---|---|---|
| `InstrumentSans.ttf` | `ofl/instrumentsans/InstrumentSans[wdth,wght].ttf` | 194 336 |
| `Newsreader.ttf` | `ofl/newsreader/Newsreader[opsz,wght].ttf` | 451 664 |
| `OFL-InstrumentSans.txt` | `ofl/instrumentsans/OFL.txt` | 4 403 |
| `OFL-Newsreader.txt` | `ofl/newsreader/OFL.txt` | 4 394 |

Their SHA-256: `InstrumentSans.ttf`
`b24f1812584816958afcf22e22d08e44318c5e51651e25d2438efdde389b33b1`;
`Newsreader.ttf`
`8a08d13f8a6c0d51be379a60af84f945f65369a67e509ee3c3bdcc421254d7c1`;
`OFL-InstrumentSans.txt`
`9e27a72ed30eb49a08678f6a5d6ed98ec7ba5368f541637ee0683ec9134ef966`;
`OFL-Newsreader.txt`
`fdfad38143ec470553cae82a1e45320bdd1b9ec70415d37bd0171051d8a4ded8`. Each
file's git blob is the one GitHub's contents API listed for it, so the
bytes are the published bytes.

The files are renamed only to drop the brackets from their names; the OFL
binds a font's *name*, not its file name, and nothing inside is changed, so
neither is a Modified Version and neither Reserved Font Name question
arises.

**Variable, not static.** Each is one file for every weight. This engine
sets a variable font's `wght` axis to the weight asked for and measures,
shapes and outlines that instance (`alo_text::Font::at_weight`, queue items
402 and 683), so 400, 500 and 600 are three instances of one file — 646 KB
for both families, against eight static files. Every other axis is left at
the file's default: Instrument Sans' `wdth` at 100, the design's; and
Newsreader's `opsz` at **18**, where a browser sets it to the size of the
text (`font-optical-sizing: auto`). So the greeting at 40 px is drawn with
the 18-point cut's proportions, a little heavier in its thin strokes than
the canvas. That is queue item 752's, and is the one way these faces are
drawn differently from the canvas.

Only the upright files are carried: the design uses no italic, and a slant
a page of ours never asks for is 700 KB nobody draws.

## 2. Compiled into the program

The four files are compiled into `alo-renderer` (`typefaces.rs`,
`include_bytes!` and `include_str!`), as the corpus's `DejaVu` faces are
compiled in by the `dejavu` crate. So:

- **nothing is read at run time** to find them — no path in the bundle to
  resolve, no file a person could move, and no network, ever;
- a test, the window, and the application all hand a renderer the same
  bytes, which is what makes a reference render of the interface the
  interface;
- the faces are filed by what the font says it is
  (`face::Face::described`: its `name` table's family, its `OS/2` weight and
  slant), the same rule as a face read off the machine's disk, never by a
  name written in this repository — Newsreader's legacy family is
  *Newsreader 16pt*, and its typographic family, which a page asks for,
  *Newsreader*.

The alternative, files in `Contents/Resources/fonts` read by the browser
process as it starts, was refused: it adds a run-time failure (a bundle
missing a file) and a second road a test does not take, to save 646 KB in
a program that already carries a JavaScript engine.

## 3. Handed to the interface's renderers, and only to them

A renderer opens no file (ADR 0010); the browser process hands it every
face as bytes on its pipe before any work (`host.rs`, `UseFont`). The
design's typefaces go by the same road, added where the conversation's
renderers start — `alo-window`'s `ConversationDrawing::start`, through
`fonts::for_the_interface` — after whatever fonts those renderers were
given: the machine's, or `--frozen-fonts`' corpus faces.

**A page's renderer is never handed them.** The renderers that draw web
pages (`alo PAGE`, `Tabs`) and the tab strip's keep the machine's fonts
alone. Two reasons, either sufficient:

- **Rendering.** A page asking for `Instrument Sans` without loading it
  would be drawn in it in alo and in something else everywhere else — a
  difference this engine would have made, not the page.
- **Fingerprinting.** Which fonts a page can find is one of the oldest
  ways to tell one browser from another. Handing pages two families no
  other browser has would mark every alo session (ADR 0030: the browser
  says what it is and nothing more).

The stylesheets ask for each family first and keep the design's fallbacks
after it — `"Instrument Sans", system-ui, sans-serif` and `Newsreader,
serif` — so a renderer that was not handed them (a test's, a strip's)
draws the generics, never nothing.

The tab strip's stylesheet is left on `system-ui`: the strip is not in the
design, its renderers draw beside a page's in `alo PAGE`, and ADR 0074 § 2
keeps it as it is until it is folded into the panel.

## 4. A character neither has

Instrument Sans has 343 characters, Latin only; it has no `⌘`, which *New
chat*'s `⌘T` uses, and no Greek or Cyrillic, which a site's name in the
results may. Layout already measured each run of a line in the first font
of its chain that has its characters, but paint drew the whole piece in the
chain's first font — invisible while that font was `DejaVu Sans` or the
system's, which have them. `alo-paint`'s `ink_runs.rs` draws a piece as the
runs layout measured, each in its font, each starting where the one before
ended, as a browser falls back glyph by glyph. Characters no font has are
drawn as before, so no corpus render moved.

*(Since: ADR 0088. Neither face has an emoji, and on the machine the
chain's last resort was `LastResort`'s box; the interface now carries a
third face, Noto Color Emoji, under the OFL, drawn in colour, and a
character meant as an emoji is looked for in it first.)*

## 5. The licence in the bundle

The OFL lets the fonts be "bundled, embedded, redistributed and/or sold
with any software provided that any reserved names are not used by
derivative works" and requires the copyright notice and the licence to go
with them. `alo-bundle` writes both `OFL.txt` files, byte for byte, into
**`alo browser.app/Contents/Resources/licences/`** as
`OFL-InstrumentSans.txt` and `OFL-Newsreader.txt` — the directory where
CEF's licence and Chromium's credits already are (queue item 720) — in
every application, with or without a Chromium. They are sealed with the
rest. `tests/the_application_assembled.rs` lists them among the
application's files and compares their bytes with the published ones.

The repository carries them beside the fonts, in
`crates/alo-renderer/interface/fonts/`. The repository's own licence, the
MPL 2.0, does not cover those four files; the OFL does.

## Consequences

- `alo-renderer` gains `typefaces.rs` (the bytes, the licences, the faces)
  and `Face::described`; `interface/panel.css` and `start.css` ask for the
  two families.
- `alo-window`'s `fonts::for_the_interface` and `ConversationDrawing::start`
  hand them to the conversation's renderers; `bundle_made.rs` writes the
  licences.
- `alo-paint`'s `ink_runs.rs` draws each run in the font layout measured it
  in.
- ADR 0067's line that *the bundle carries no font file* stays true of
  files and is now incomplete: `alo` carries two typefaces compiled in, and
  the bundle their licences.
- Every reference render of the panel, the first screen, the results and
  the card was redrawn in the two faces, and the layout assertions whose
  numbers are text widths were moved, each by what the new face measures.
- Newsreader's optical size is queue item 752.
