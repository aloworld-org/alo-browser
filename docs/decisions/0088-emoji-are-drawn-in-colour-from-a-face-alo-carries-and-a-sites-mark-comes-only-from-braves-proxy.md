# ADR 0088 — Emoji are drawn in colour from a face alo carries, and a site's mark comes only from Brave's proxy

**Status:** accepted — built with queue item 747 (31 October 2026)
**Date:** 2026-10-31
**Context:** ADR 0084 (the interface's typefaces are compiled in and handed
to its renderers alone; § 4, a character neither has); ADR 0010 (a renderer
opens no file, and fonts cross to it as bytes); ADR 0005 (a stranger's
bytes are read in a confined process); ADR 0083 § 6 (a site's favicon not
fetched: *one more request … of a picture*); ADR 0086 §§ 3–4 (thumbnails
only from Brave's proxy, fetched by the browser process, decoded in the
renderer, 2 MiB a first screen); ADR 0087 § 3; `CLAUDE.md`, *rent the
physics*; the owner's canvas of 31 October 2026 (`.fav`, 18 square,
rounded 5; 14 on Images); queue items 706, 747; STATE, iteration 469 (*a
description's emoji is drawn as a box*)

## The questions in one line

A search's titles and descriptions hold emoji and characters the
interface's two faces do not have, and are drawn as boxes: which face
draws them, can this engine draw a colour emoji at all, and where does it
come from on three systems? And: a site is drawn as a coloured letter
where the design shows its mark — whose mark, fetched from where, read by
which process?

## Part 1 — characters the interface's faces lack

### 1. What was drawn, and why

The interface's renderers are handed the machine's faces (or, under
`--frozen-fonts`, the corpus's five `DejaVu` faces) and then Instrument
Sans and Newsreader (ADR 0084 § 3). The machine's are at most 24, read from
`/System/Library/Fonts` and `/Library/Fonts` — `.ttf` and `.otf` only, never
a `.ttc` collection (`alo-renderer`'s `fonts.rs`) — the generic families'
first and the rest by name. On this Mac that is the Apple Braille faces,
Apple Symbols, Geneva, Keyboard, **LastResort**, Monaco, New York and the
SF faces. None has an emoji. `LastResort` has a glyph for *every* code
point — the box that names a character's block — so an emoji, and a CJK
character, fell through the chain to it and was drawn as `⊡`: the boxes
seen on Brave's video descriptions and an IMDb title.

The chain itself was right (ADR 0084 § 4, `ink_runs.rs`); what it held was
not. And two smaller faults made emoji worse than missing:

- **A sequence was split.** `❤️` is U+2764 then U+FE0F; `👋🏽`, a hand then a
  skin tone; `👩‍💻`, two people joined by U+200D. The selector, the tone and
  the joiner were each looked up on their own and could land in another
  face — a run of its own — so no face was handed the sequence it draws as
  one picture.
- **A character's meaning was not asked.** `©`, `#` and the digits are in an
  emoji face too, and are meant as letters; `😀` is meant as a picture. A
  chain that asks only *who has it* cannot tell them apart.

### 2. Can this engine draw a colour emoji?

Not before this change. `alo-paint` draws a glyph as its outline
(`glyph.rs`, the rented `ttf-parser`), filled in the text's colour. A colour
emoji is one of four other things:

| Format | Where | What it is |
|---|---|---|
| `sbix` | Apple Color Emoji (macOS) | PNG pictures, several sizes |
| `CBDT`/`CBLC` | Noto Color Emoji (Linux, Android) | PNG pictures, one size |
| `COLR` v0/v1 + `CPAL` | Segoe UI Emoji (Windows); Noto's COLRv1 build | layers of outlines; v1 adds gradients, transforms, blending |
| `SVG ` | some fonts | an SVG document per glyph |

`rustybuzz` shapes all of them — it reads `cmap` and `GSUB` (and Apple's
`morx`), which is how a sequence becomes one glyph — and `ttf-parser` reads
all four tables. What was missing was paint.

### 3. Chosen: carry Noto Color Emoji, and draw a `CBDT` or `sbix` glyph as the picture it is

**The face.** `NotoColorEmoji.ttf`, the `CBDT` build, as `googlefonts/noto-emoji`
publishes it in `fonts/` at tag **`v2.051`** (*Unicode 17.0 update mk1*,
commit `8998f5dd683424a73e2314a8c1f1e359c19e8742`), unmodified:

| File in `crates/alo-renderer/interface/fonts/` | Published as | Bytes | SHA-256 |
|---|---|---|---|
| `NotoColorEmoji.ttf` | `fonts/NotoColorEmoji.ttf` | 10 673 480 | `72a635cb3d2f3524c51620cdde406b217204e8a6a06c6a096ff8ed4b5fd6e27b` |
| `OFL-NotoColorEmoji.txt` | `fonts/LICENSE` | 4 301 | `6a73f9541c2de74158c0e7cf6b0a58ef774f5a780bf191f2d7ec9cc53efe2bf2` |

Their git blobs, `943741df…` and `d952d62c…`, are the ones GitHub's
contents API lists at that tag. **The licence is the SIL Open Font License
1.1** — `fonts/LICENSE` says so, the font's own `name` table says so (ID
13), and the repository's README: *"Emoji fonts (under the fonts
subdirectory) are under the SIL Open Font License, version 1.1."* The
copyright notice the OFL asks to travel with the font, *Copyright 2022
Google Inc.*, is in the font's `name` table (ID 0), which is unmodified; the
licence file goes in the bundle beside the other two (ADR 0084 § 5). The
font is renamed only in its file's name, so it is not a Modified Version.

It is compiled into `alo-renderer` (`typefaces.rs`) as Instrument Sans and
Newsreader are, and handed **to the interface's renderers alone** by the
same road (`fonts::for_the_interface`) — never to a page's, for ADR 0084
§ 3's two reasons (a page would be drawn differently here than elsewhere,
and a family no other browser hands a page marks every alo session).
10.7 MB in the program, and 10.7 MB on each interface renderer's pipe as
it starts (`LARGEST_FONT` is 16 MiB; a message 64 MiB).

**The choice of face.** A face per glyph: emoji are looked for in
Noto first, whatever the machine has — so alo's interface draws the same
emoji, pixel for pixel, on macOS, Windows and Linux, and a reference
render of the interface is the interface everywhere. It is not what Chrome
draws on a Mac (Apple's art); it is what Chrome draws on Linux and Android.

**Paint.** A face whose glyphs are pictures (`Font::draws_pictures`: a
`CBDT` or `sbix` table) has its glyphs drawn by `alo-paint`'s
`glyph_picture.rs`: each shaped glyph's PNG is taken from the strike
nearest its size in device pixels (`glyph::picture_at`), read by the
picture readers every `<img>` goes through (`picture::read`, with their one
bound on pixels), and put in the display list as a picture item at the
glyph's place and size — the face's bearing and the strike's pixels per em
— after the text item of its run. So it is drawn down (Noto's strike is
109 to the em; a 16 px emoji is a 136 × 128 picture drawn 20 × 18.8),
clipped and faded exactly as a picture is, in its own colours, never the
text's ink. `sbix` comes with it for nothing: the same function reads
Apple's table, though no renderer is handed Apple's face (§ 4).

**Which face a character asks for** (`alo-text`'s `emoji.rs`). Unicode's
own table — `Emoji_Presentation` (UTS #51), from `unicode-properties`, the
crate and version `rustybuzz` already carries, named in that one file —
says whether a character is meant as a picture; a following U+FE0F asks
for the picture and U+FE0E for the letter. Meant as a picture, the chain's
first face that draws pictures and has it; meant as a letter, its first
face that does not (`FontDatabase::font_shown`). Either falls back to the
plain chain, so a character some face has is never lost for being meant
otherwise. And a selector, a joiner, the keycap's mark, a skin tone or a
tag stays in the run of the character before it (`emoji::joins_previous`),
so the face is handed the whole sequence and its `GSUB` makes it one
picture. Layout and paint read the same split, so an emoji is laid out at
the width it is drawn: Noto's advance, 2550/2048 em.

**What an agent reads** is unchanged: the characters (ADR 0002). An emoji
is text in the tree; its picture is paint.

### 4. Considered and not chosen

1. **The system's emoji face, handed as bytes.** Apple Color Emoji is a
   192 MB `.ttc` — twelve times `LARGEST_FONT`, copied into every
   interface renderer — and a collection, which the machine scan does not
   read; Windows' Segoe UI Emoji is `COLR`, which this engine does not
   paint (§ 5); a Linux machine may have nothing. Three systems, three
   answers, and no reference render that holds on all of them. Carrying
   one face is the opposite of each.
2. **Noto Color Emoji's `COLRv1` build** (5 MB, vectors, sharp at any
   size). Half the bytes, but drawing it is gradients — linear, radial and
   sweep — transforms, clips and blending modes in the paint graph `COLR`
   v1 describes: an engine of its own, queued (766), not a step of this
   item. The `CBDT` build needs a picture drawn, which this engine already
   does exactly.
3. **Noto Emoji, monochrome** (2 MB, outlines, in the text's colour). No
   engine work, and not what anyone expects an emoji to look like; colour
   was within reach.
4. **Twemoji's `COLR` v0 build.** Its artwork is CC-BY 4.0, not the OFL,
   and Twitter's set is no longer kept; and v0 is still `COLR` (§ 5).
5. **The whole of Google Fonts' `NotoColorEmoji-Regular.ttf`** (25 MB, both
   tables): past `LARGEST_FONT`, for nothing `CBDT` alone does not draw.

### 5. What is still drawn as a box, or not in colour

- **CJK, and every script no handed face has.** On macOS the CJK faces are
  collections (`Hiragino Sans GB.ttc`, 23 MB), which the scan does not read,
  and `LastResort` stays in the cut: a Chinese, Japanese or Korean title in
  the interface is still `LastResort`'s box. Queued (767): reading a
  collection's face, and a chain that tries `LastResort` last. A face
  carried for CJK is 16–20 MB a weight and is not proposed.
- **A `COLR` or `SVG` face is drawn as its outlines**, in the text's colour,
  where it has them (Segoe UI Emoji's base glyphs). Queued with `COLR`
  v1 (766).
- **An emoji casts no `text-shadow`** and a decoration line does not step
  aside for it: the shadow is cast by the run's outlines, which a picture
  glyph has none of. Nothing in the interface shadows text; queued with
  766.
- **Pages are unchanged on macOS and Windows.** A page's renderer is never
  handed Noto. The rule of § 3 runs everywhere, but a page reaches a face
  of pictures only where the machine scan hands it one — a Linux machine
  with Noto Color Emoji as a `.ttf` among its 24 — where that page's emoji
  are now drawn in colour rather than as nothing.

## Part 2 — a site's own mark (queue item 747)

### 6. Where the mark comes from

Brave's web, video and news results each carry `meta_url.favicon` —
*"the favicon used for the URL"* (Brave's API reference) — an address at
its own image proxy, `imgs.search.brave.com`, which serves a site's icon
resized (32 square). **alo fetches that, and only that**:

- **Only from the proxy.** A mark is fetched only when it is at the one
  origin the provider's thumbnails come from (ADR 0086 § 3), never from the
  site — a request to the site for its `favicon.ico` would tell it this
  person searched for something it holds. A redirect is not followed.
- **The person's, with nothing of theirs.** `Accept: image/*` and what
  `alo-net` always sends; no key (the proxy needs none, so a mark is never
  billed: **a mark costs no paid request**), no cookie, no `Referer`.
- **By the browser process,** on the search's thread, through its `alo-net`
  pool (`search_marks.rs`, sharing `search_thumbnails::picture`).
- **Bounded.** Each distinct address once, however many results name it
  (a tab of one site's videos is one icon); at most **20**
  (`MOST_MARKS`) for one answer; each at most **16 KiB** (`LARGEST_MARK`),
  larger left out; only bytes that begin as a picture this engine reads.
- **When.** A search's sites are told to the window at once, with their
  letters; then their marks are fetched and told (`Came::Marks`), before
  the image search — not at all if a newer search waits. A video's and a
  story's mark is fetched with its tab's answer (ADR 0087), and told with
  it.

### 7. Which process reads it

**The confined renderer**, as a thumbnail (ADR 0086 § 4). The wire refuses
a mark over 16 KiB or not beginning as a picture. As the first screen's
document is built, a mark's bytes are read (`marks_document::reads`): bytes
that do not decode, or decode larger than 256 either way, leave the site
**its letter** — never an empty square where a mark should be. One that
reads is an `<img>` named by us (`alo:mark-TURN-site|video|story-N`), never
the icon's address, in the letter's tile.

**The budget.** Marks are counted in the first screen's 2 MiB of pictures
(`Results::picture_bytes`), only for the rows a tab draws (`as_drawn`), and
weighed **before** a turn's thumbnails (`within_budget`): a site drawn with
its letter is the larger loss, and twenty marks at the most are 320 KiB.

### 8. How it is drawn

As the canvas's `.fav`: 18 square, rounded 5, where the letter was, the
name 8 beside it — the icon drawn down from 32 into a tile that clips it
round, since this engine does not round a picture by its own
`border-radius`. On All and Sites, on Videos and on News. Not on the Images
grid (its 14-square letters): whether Brave's image results name a site's
mark is not read here, and is queued (768). A mark is decoration, hidden
from the tree, as the letter was.

## Consequences

- `alo-text`: `emoji.rs` (new), `Font::draws_pictures`,
  `FontDatabase::font_shown`, `run.rs`'s split keeps a sequence whole; the
  workspace gains `unicode-properties`, already in the lockfile.
- `alo-paint`: `glyph::picture_at`, `glyph_picture.rs` (new); a run in a
  face of pictures puts picture items after its text item (`build.rs`).
- `alo-renderer`: `typefaces.rs` carries Noto Color Emoji and its licence;
  `results.rs` (`Found::mark`, `LARGEST_MARK`, `MOST_MARKS`, the budget),
  `results_media.rs` (`Video::mark`, `Story::mark`), `marks_document.rs`
  (new), the wire, `start.css`.
- `alo-window`: `search_protocol.rs` reads `meta_url.favicon`;
  `brave_search.rs` returns where each mark is; `search_marks.rs` (new);
  `conversation_search.rs` (`Came::Marks`), `conversation.rs`,
  `conversation_thread.rs`; the bundle carries `OFL-NotoColorEmoji.txt`.
- ADR 0083 § 6's *a site's favicon is not fetched* is superseded by § 6
  here; ADR 0084 § 4's *characters no font has* is narrowed by § 3.
- Queued: `COLR`/`SVG` colour glyphs and a picture glyph's shadow (766);
  CJK in the interface (767); the Images grid's mark (768); a picture
  rounded by its own `border-radius` (769).
