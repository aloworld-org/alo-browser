# ADR 0084 — The interface's typefaces are compiled in, and handed to its renderers alone

**Status:** accepted — built by queue item 706 (31 October 2026); Newsreader's optical size set by 752 (1 November 2026); amended 7 November 2026 — Inter the interface's face, and four more a person may choose (queue items 853, 854)
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

**Since 752 (1 November 2026)** a face's `opsz` is set to the CSS pixels a
run is set at, clamped to the axis (6–72 for Newsreader), in measuring,
shaping and outlining alike (`Font::optical_size`, `Font::variations`),
and `font-optical-sizing: none` leaves it at the face's default — as
Chrome 156.0.8078.76 does, measured on this Mac (its source not read): the
greeting at 40 is `opsz` 40, the mark at 22 is 22. Each glyph's advance at
40 is Chrome's to half a font unit (the rented shaper's advances are whole
units, 795), and a run's width Chrome's to its layout unit unless a
kerning pair in it varies along the axis (796); a line measured word by
word loses the kerning across a space at every size (797).

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
- Newsreader's optical size was queue item 752, built: every reference
  holding the mark or the greeting was redrawn (the mark at 22 barely, the
  greeting at 40 wider and finer).
- Both faces are variable, and the rented shaper set them a little off
  Chrome away from their masters — advances rounded to whole units, a
  kerning pair's variation lost or rounded — and a line was measured word
  by word: queue items 797, 795 and 796, built (2 November 2026, ADR 0097).
  2,370 of 2,400 measured runs are now Chrome's to its layout unit.

## Amendment, 2026-11-07 — Inter, and four faces a person chooses (queue items 853, 854)

The owner's design of 7 November 2026 (*"Yes please"*,
`docs/design/shell.md`, *Typefaces* and *Settings*): **Inter** for every
word of the interface, Newsreader kept for the mark and the greeting
whatever is chosen; and in *Settings* a choice of **Geist**, **DM Sans**,
**Plus Jakarta Sans** and **Instrument Sans** instead. Everything above
holds of the four new faces as of the first two — compiled in, filed by
what each font says it is, handed to the interface's renderers alone and
never a page's, each licence in the bundle — and this records what is
new.

### The files, as published

Each from its official upstream release only, downloaded into a directory
of its own and nothing but the font and its licence read from it. All
four are under the **SIL Open Font License 1.1**: each licence file says
so, and each font's `name` table (ID 13) and copyright notice (ID 0),
unmodified, say so too. Each is renamed only in its file's name, so none
is a Modified Version.

| File in `crates/alo-renderer/interface/fonts/` | Published as | Bytes | SHA-256 |
|---|---|---|---|
| `Inter.ttf` | `InterVariable.ttf` in `Inter-4.1.zip`, the `v4.1` release of `github.com/rsms/inter` (tag commit `e3a3d4c5`; the zip's SHA-256 `9883fdd4…6b11e`) | 879 708 | `4989b125924991b90d05b2d16e0e388c48f7d5bb8b30539bbf9c755278d0ccaf` |
| `OFL-Inter.txt` | `LICENSE.txt` in the same zip (git blob `9b2ca37b`, the tag's own) | 4 380 | `262481e844521b326f5ecd053e59b98c8b2da78c8ee1bdbb6e8174305e54935a` |
| `Geist.ttf` | `geist-font/Geist/variable/Geist[wght].ttf` in `geist-font-v1.9.2.zip`, the `v1.9.2` release of `github.com/vercel/geist-font` (tag commit `ef2597cf`; the zip's SHA-256 `15f066d5…5d3d8`, the digest GitHub lists for the asset) | 169 056 | `b34955a09f71d9ac4b67e54cedb1825d3d5264c7992dbbafa0802080d617e241` |
| `OFL-Geist.txt` | `geist-font/OFL.txt` in the same zip (git blob `999fa237`, the tag's own) | 4 384 | `021caa749f89fecd7c2e40a50499de29e84d2bc19251659b47d499c2fbfffc17` |
| `DMSans.ttf` | `ofl/dmsans/DMSans[opsz,wght].ttf` of `github.com/google/fonts` at commit `86aced91de05cd4f96a06838124cca45be337b2b` (git blob `c672f980`) | 240 164 | `8cd08d97e89c24d0aa92edd2f0f4c8ee6195eee9b7c9f154865a58b02f0c1c0d` |
| `OFL-DMSans.txt` | `ofl/dmsans/OFL.txt` there (git blob `4430b85a`) | 4 482 | `9af36190332437f5ecd09974de43c1f7c77a310a996cdd8ceb25628b458840e1` |
| `PlusJakartaSans.ttf` | `ofl/plusjakartasans/PlusJakartaSans[wght].ttf` there (git blob `0cb13a99`) | 176 288 | `89b3fb38aa0d275d7a731d0d817a4f1622b316b4d7fbdedcf02ee9099ff68bc8` |
| `OFL-PlusJakartaSans.txt` | `ofl/plusjakartasans/OFL.txt` there (git blob `ea69b0ca`) | 4 402 | `995c7199cab65954f545996326755daee7b63cc6b42b06c13da1f9502ab08a99` |

**1 482 864 bytes** added in all: 1 465 216 of fonts, 17 648 of
licences. Geist's file in the release differs from the copy committed at
the tag (`fonts/Geist/variable`, the same size, 251 bytes apart — its
`head` and checksums); the release is the one carried. `OFL-DMSans.txt`
ends its lines CR LF as published, and `.gitattributes` keeps the
licences' bytes as they are, so the bundle's copy is the published one
byte for byte.

**Variable, all of them**, over every weight the design uses: Inter
`wght` 100–900 and `opsz` 14–32 (its default 14); Geist `wght` 100–900;
DM Sans `wght` 100–1000 and `opsz` 9–40 (its default 9); Plus Jakarta Sans
`wght` 200–800. Only the upright files are carried.

**Inter calls itself *Inter Variable*.** Its `name` table has no
typographic family and its legacy family is *Inter Variable*, and § 2's
rule files a face by what the font says, so that is the family a
stylesheet asks for (`typefaces::INTER`). DM Sans' typographic family is
*DM Sans* (its legacy one *DM Sans 9pt*).

### Their optical size, measured rather than assumed

Inter and DM Sans vary by `opsz`, and § 1's rule since 752 — a face's
optical size set to the CSS pixels its run is at, clamped to the axis —
was checked against Chrome rather than taken for them: in Chrome
157.0.8103.38, headless, the four files as web fonts after
`document.fonts.load`, each glyph's canvas advance at 9, 12, 13.5, 14, 20,
24, 32 and 40 px is this engine's to a thousandth of a pixel (below an
axis's least size and past its most, the axis's end), and 1,600 random
runs of the four faces at weights 300 to 700, 11 to 40 px, `auto` and
`none`, measured as Chrome's layout width: **1,570 Chrome's to its
layout unit, the other 30 a 64th apart**, each within 0.28 of a 64th of
the grid's edge — what this ADR's correction (ADR 0097) left of the first
two faces too. Frozen in
`crates/alo-renderer/tests/references/chosen-faces-runs.chrome.txt`,
held by `tests/a_chosen_face_set_as_chrome_sets_it.rs`.

### One face at a time, chosen without handing anything again

All five faces of words are handed to each interface renderer as it
starts, beside Newsreader and Noto (`fonts::for_the_interface`), so a
face chosen in *Settings* is drawn at once. Which one is drawn is said by
the browser process as one of a closed list (`interface_face.rs`), on the
panel's and the first screen's state, and put on each document's root as
its own style, `--face: "<family>"`; every stylesheet of the interface
asks for `var(--face), system-ui, sans-serif` and says `--face` is Inter
on the root. No stranger's string reaches the declaration. The tab
strip's stylesheet stays on `system-ui` (§ 3), and a card's page is never
handed any of these.

### The licences in the bundle

`alo-bundle` writes all seven licences into `Contents/Resources/licences/`
as before (`typefaces::LICENCES`); `tests/the_application_assembled.rs`
lists them and compares their bytes with the published ones.

### Consequences

- Every reference render of the interface was redrawn in Inter and looked
  at, and the layout assertions whose numbers are text widths were moved
  by what Inter measures — of the five, the widest on the interface's own
  lines that were measured (the box's hint, the agent's line, a menu's). The rail's buttons asked for no face and were drawn in a
  control's own; they ask for the face chosen now.
- 1.48 MB more in the program, and on each interface renderer's pipe as
  it starts.
- The link strip of an expanded card, drawn by the Chromium alo carries,
  asks for *Inter* where the system has it and is not told the face
  chosen (queue item 852).

