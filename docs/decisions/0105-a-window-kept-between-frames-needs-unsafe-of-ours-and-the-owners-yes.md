# ADR 0105 — A window kept between frames needs `unsafe` of ours, and the owner's yes

**Status:** accepted — the owner's yes to both questions, 9 November 2026
(*"Okay go ahead"*), on the condition that the tagged layer is measured as
near the machine's Chrome as `softbuffer` or nearer, which it is (*Measured
against Chrome*, below). Built the same day (queue items 873 and 902).
**Date:** 2026-11-09
**Context:** queue items 872 and 873; ADR 0024 § 1 (`softbuffer` puts the
window's pixels on screen, through safe interfaces, and `wgpu` was "not
now"); ADR 0010 (a rented crate's `unsafe` is the crate's, not ours); ADR
0080 (the one `unsafe` boundary there is, in the CEF host, *"and every crate
of alo's own workspace stays free of `unsafe`"*); ADR 0102 § 4 (the window
composes again only what changed, and presents all of it); ADR 0070 (Rust
1.89); law 4; `CLAUDE.md`'s gate, *No `unsafe` without an ADR*

## The question in one line

After 872 a letter is composed in 2–3 ms and then presented in 10.4: can
the window be given only what changed, by a surface that keeps its last
frame, without `unsafe` of ours and without a pixel on screen moving?

## Measured

**On this Mac** — an M3, macOS 26.5, the built-in Retina display at two —
by a program outside the repository (scratchpad `present873/`, release,
`winit` 0.30.13 as alo has it): a window of 1280 × 852 points, 2560 × 1704
pixels, the size of alo's, given a full frame of varied pixels and then 150
frames 33 ms apart, each changing 300 × 230 pixels, about what a letter
changes. Each present timed on the event loop; `/usr/bin/time -l` for the
process; a screenshot of the last frame, every way, compared pixel by
pixel. Rows in `handrun50/probe-*.log`.

| Route | Present, median (p90) | Instructions, whole run (the first two runs alike) | `unsafe` of ours | On screen, against `softbuffer` |
|---|---|---|---|---|
| `softbuffer` 0.4.8, as alo is | **11.6–13.2 ms** (12.3–14.3) | 30.8 billion | none | — |
| of which: its buffer 0.44, our copy 0.72, **its `present` 10.56** | | | | |
| `wgpu` 30.0.1, Metal, a kept texture, only the change written, copied to the drawable | **0.16–0.8 ms** (0.2–0.9) | 1.4 billion | none | **colours not matched**: sRGB red shown as the display's own red, (255, 0, 0) where `softbuffer` shows (234, 51, 35); 4.0 of 4.6 million pixels differ, by up to 117 |
| the same, the layer's colour space Display P3 | 0.55 ms | — | none | the same as above |
| the same, half floats, extended sRGB (the only matched space `wgpu` offers) | 0.41 ms | — | none | 72 % of pixels differ, by 1–2 |
| the same, the layer tagged as `softbuffer` tags its image (device RGB, or sRGB) after `wgpu` configures it | **0.16 ms** (0.22) | — | **one block** | flat colours identical; **15 % of colours one level off in one channel** (2,026 of 13,824 sampled), never more than 2 |
| an `IOSurface` as the layer's contents (not built) | not measured | — | several | not measured; matched at composition as the Metal layer is, so likely the same |

**Where `softbuffer`'s 10.4 ms goes.** Not our copy (0.72 ms): its
`present`, which makes a new `CGImage` of the whole buffer, in the device's
RGB, and sets it as the layer's contents. Its `age` is always 0 on macOS
(`softbuffer` 0.4.8, the newest; its repository's `master` is the same on 9
November 2026), so no version of it keeps a frame.

**Why the colours differ.** `softbuffer` tags its image with the device's
RGB, which Core Animation matches to the display as sRGB. A `CAMetalLayer`
whose colour space is unset is not matched at all, whatever `wgpu`'s comment
says; `wgpu` 30's `SurfaceColorSpace::Srgb` leaves it unset, and the only
other spaces it allows an 8-bit surface are Display P3 (not matched here
either) and, for half floats only, the extended spaces. Tagged as
`softbuffer` tags, the layer is matched — by the window server as it
composes, where `softbuffer`'s image was matched once, as it was set — and
the two round differently on one colour in seven.

## What the routes need

- **`wgpu`, safe throughout** (`Instance`, `create_surface` over an
  `Arc<Window>`, `configure`, `write_texture`, `copy_texture_to_texture`,
  `present`): 26 crates more (`wgpu`, `wgpu-core`, `wgpu-hal`, `naga`, …,
  Metal alone), MSRV 1.87, no `unsafe` of ours — and every colour wrong.
- **`wgpu` and the layer tagged**: the same, and **one `unsafe` block**:
  `Surface::as_hal::<wgpu::hal::api::Metal>()`, whose guard gives
  `render_layer()`, on which `objc2-quartz-core`'s `setColorspace` with
  `objc2-core-graphics`'s `CGColorSpace::new_device_rgb()` are safe calls.
  Its contract (`wgpu` 30's documentation): the surface is not destroyed
  through the guard, and every requirement of `wgpu-hal` is kept — here,
  the guard held only for the call, on the event loop's thread, with no
  frame of the surface out. It is again after every `configure`, which
  sets the colour space back.
- **An `IOSurface`**: the pixels written through the surface's base
  address while it is locked (a raw pointer); `CALayer::setContents`, which
  `objc2-quartz-core` 0.3 marks `unsafe`; and the window's `NSView` from
  `raw-window-handle`'s pointer — several blocks, and a layer of ours to
  keep in step with the view's size and scale, which `softbuffer` does with
  key-value observing.
- **The card's page as a layer of its own** (873's other half): a
  sublayer of the window's view — the same `unsafe` as an `IOSurface`.

## Why it waited for the owner

**The gate could not hold it, by design.** `scripts/gate.sh` refuses any
`unsafe` in a crate of the workspace, and any manifest that lifts
`unsafe_code`; it knew one boundary, the CEF host's, named in it by file
(ADR 0080). ADR 0080 said alo's own process holds no `unsafe`. A boundary
in `alo-window` was therefore a change to that rule and to the gate's check
of it — the owner's, as ADR 0080 was ("the owner's *Yes*"), not an
iteration's.

**And it moves pixels on screen.** No reference render changes — the
window's words are the same — but one colour in seven reaches the display
a level apart in one channel, because the window server matches them where
Core Animation matched them before. 873's *never a pixel moved* cannot be
met by any route measured.

## The two questions, as they were asked

1. **May `alo-window` hold one `unsafe` boundary** — one module,
   `crates/alo-window/src/present_kept.rs` (the only file to name `wgpu`,
   `objc2-quartz-core` and `objc2-core-graphics`), the lint lifted on that
   module alone as ADR 0080 lifts it on `mod application_mac`, and the gate
   extended to check exactly that, the way it checks the host — for one
   block, `Surface::as_hal`, with the reason and invariants above in its
   `// SAFETY:` comment?
2. **May the window's colours be matched by the window server**, one level
   apart in one channel on about 15 % of colours, for a present of 0.16 ms
   where it is 10.4?

If both are yes: the module as above, falling back to `softbuffer` where
`wgpu` finds no adapter; tests that count what is written (bytes of a
letter against the window's), a test that the kept texture read back is
every word composed whole, and the screenshot comparison above made
against the old present in the same run. If either is no: `softbuffer`
stays, and a letter's present stays 10.4 ms until a rented crate keeps a
frame on macOS with its colours matched as now.

## The answer: yes to both, nearer Chrome first

The owner answered yes to both, on one condition: that the tagged layer
be measured against the machine's Chrome first, and built only if it is
as near Chrome as `softbuffer` or nearer.

### Measured against Chrome

Google Chrome 157.0.8103.38, shown (`--app`, a fresh profile, the GPU on),
on the same display at two, showing each frame as an sRGB PNG at one
device pixel to one; `softbuffer` and the tagged layer showing the same
frame through the scratch program; the three windows captured at the same
place and compared over the frame's pixels (the screen's own floating icon,
top right, left out). Scratchpad `handrun50/chrome/`, `compare.txt`.

| Frame | Pixels | `softbuffer` vs Chrome | tagged layer vs Chrome | `softbuffer` vs tagged |
|---|---|---|---|---|
| varied pixels | 3,736,600 | 467,833 differ (12.52 %), max 147 at 177 edge pixels | **411,606 (11.02 %)**, the same 177 | 551,611, all by 1 |
| 13,824 colours in cells | 3,736,600 | 463,594 (12.41 %) | **421,169 (11.27 %)** | 550,672, all by 1 |
| 16 flat colours (primaries, greys, `#e62828`, accents) | 3,736,600 | 517 (0.01 %, the window's edge) | 517, the same | **0** |
| `settings-over-a-card.png` | 849,084 | 36,945 (4.35 %), all by 1 | **26,044 (3.07 %)**, all by 1 | 46,810, all by 1 |
| `start-screen.png` | 849,084 | 714,605 (84.16 %), all by 1 | **714,211 (84.12 %)**, all by 1 | 714,217, all by 1 |

The tagged layer is nearer Chrome on every frame with colours that differ,
and the same on flat colours; neither is Chrome. alo's paper, sRGB
(244, 241, 236), reaches this display as (243, 241, 237) through
`softbuffer`, (244, 241, 236) through the tagged layer and (244, 241, 237)
through Chrome — each one level from Chrome in one channel.

## Built (queue items 873 and 902)

- **`crates/alo-window/src/present_kept.rs`** — the one file naming `wgpu`,
  `objc2-core-graphics` and `bytemuck` (the layer's own type and its
  `setColorspace` are `objc2-quartz-core`'s, which `wgpu` brings with that
  method enabled; it is called, never named). `Held` keeps a texture of the
  window's size and writes into it only the regions `Composed` composed
  again (all of it when composed whole, or of a new size); `Kept` is the
  Metal surface, configured `Bgra8Unorm`, `COPY_DST`, opaque, FIFO, and
  tagged after every `configure` by `tag`, which holds **the one `unsafe`
  block** — `surface.as_hal::<wgpu::hal::api::Metal>()` — under a
  `// SAFETY:` comment giving `as_hal`'s two requirements and how each is
  kept. A frame copies the kept texture whole to the next drawable on the
  GPU and presents it; a covered window gives no drawable, keeps what was
  written, and is presented again when seen (`window.rs`).
- **`present_chosen.rs`** — `Screen`: the kept way, or `softbuffer`'s
  `Presenter` copying every word as before when `wgpu` finds no Metal
  surface or GPU for the window (said once, on standard error) or
  `ALO_PRESENT=copied` asks. Off macOS, always `softbuffer`.
- **`present_shown.rs` and `alo-present`** — a frame of known colours in a
  window of its own, shown either way, for a test and a person to read off
  the screen.
- **The lint**: the workspace's `unsafe_code` is `deny` where it was
  `forbid` (a `forbid` cannot be lifted on one module), lifted by one
  `#[expect(unsafe_code, reason = …)]` on `mod present_kept;` in
  `alo-window`'s `lib.rs`. ADRs 0080 and 0024 § 1 are amended.

## What is checked

**By the gate** (`scripts/gate.sh`, extended by file as it checks the
host): the workspace's lint is `deny` and named once in the root manifest;
no crate manifest names it; no file of the workspace but `present_kept.rs`
writes `unsafe` — a block, a function, an implementation, a trait or an
extern block — and that file writes it on exactly one line, a block, and
nothing else unsafe; `unsafe_code` is named once in the workspace's
sources, in `lib.rs`, on the attribute whose item is `mod present_kept;`;
`wgpu`, `objc2_core_graphics` and `bytemuck` are named in `present_kept.rs`
alone. Each refusal was seen to fire on a copy of the sources: a block in
`present.rs`; a second block in `present_kept.rs`; an `unsafe fn` there;
an `unsafe impl` there; no `unsafe` there; `#![allow(unsafe_code)]` in
`alo-paint`; the lift moved to `mod present`; another module between the
lift and `mod present_kept`; the workspace lint `allow`; a crate manifest
lifting it. The host's checks are unchanged.

**By tests:**
- `present_kept`'s `only_the_regions_given_are_written_and_the_texture_keeps_the_rest`
  — a window of an odd width (a row not a multiple of 256 bytes): the whole
  written once, then two regions alone (their 508 bytes, of 14,948), read
  back from the GPU every word; nothing for nothing; another size whole.
- `conversation_composed`'s
  `the_pixels_kept_on_the_gpu_are_every_word_composed_whole_given_only_what_changed`
  — composed scenes handed to the kept texture as the window hands them,
  read back after each: every word as composed whole; a letter's 432
  bytes of the window's 336,000; nothing changed, nothing written; a card
  picture's changed pixels; composed whole, written whole.
- `the_window_on_screen_kept_and_copied.rs` — `alo-present` showing the
  same frame copied (the way the fallback takes) and kept, each captured
  off the screen by the window server's number in the same run: the copied
  window presents (red, green, white and black stripes read as such); the
  kept one wrote the first frame whole; every flat stripe pixel for pixel
  the same; every other pixel within one level a channel, fewer than a
  quarter of them apart.
- **Mutations**, each caught: the layer left untagged (the screen test: the
  flat colours differ); every frame written whole (both counting tests);
  only the first region written (both read-backs).

## After (on hardware)

Release, the Retina display at two, the same build both ways in the same
session (`ALO_PRESENT=copied` for the old way), 872's timestamps in a
worktree for the measurement only (scratchpad `handrun50/kept-*.log`,
`copied-*.log`):

| | copied (`softbuffer`) | kept |
|---|---|---|
| present, a letter (median) | 10.5 ms | **0.3 ms** |
| present, a card's picture (median) | 10.4 ms | **0.4 ms** |
| a letter, key to screen, beside a streaming card (25 keys) | median 43.2 ms, 47.3 at the ninetieth, least 30.2 (one of 541.7) | **median 21.7 ms**, 23.6 at the ninetieth, least 19.5 (one of 55.1) |
| a letter, key to screen, no card (25 keys) | median 25.6 ms, 33.1 at the ninetieth, least 23.6 | **median 15.2 ms**, 17.4 at the ninetieth, least 14.1 (one of 35.5) |

The first screen captured off the screen both ways in that session: 74 %
of its pixels one level apart in one channel — the paper, which is one
level from Chrome either way (above) — and none further, but for the
screen's own floating icon.

## What would remove the `unsafe`

`wgpu` tagging an 8-bit surface's layer sRGB when asked for
`SurfaceColorSpace::Srgb` (a change upstream: today it clears the tag), or
`softbuffer` keeping its buffer on macOS. Either, released and within Rust
1.89, ends the boundary in the change that takes it.

## Rejected

- **`wgpu` untagged** — every colour off, by up to 117 levels: the page
  shown in the display's own gamut, not as written.
- **Half floats in extended sRGB** — safe, but 72 % of pixels one or two
  levels off, and the layer in extended dynamic range for nothing.
- **Our own matching into Display P3** — the display's profile copied into
  the engine, and still not ColorSync's rounding.
- **The gate's check widened** to let any workspace crate write `unsafe` —
  it names one file, as it names the host's.
- **An `IOSurface` as the layer's contents** — several `unsafe` blocks to
  the Metal layer's one, for the same matching by the window server.
