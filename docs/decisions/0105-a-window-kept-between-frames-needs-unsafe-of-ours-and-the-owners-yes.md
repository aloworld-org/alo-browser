# ADR 0105 — A window kept between frames needs `unsafe` of ours, and the owner's yes

**Status:** proposed — measured for queue item 873 (9 November 2026);
**nothing of it is built**. It asks the owner two things (*The two
questions*, below) and is carried by queue item 902 until answered.
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

## Why this is not built

**The gate cannot hold it, by design.** `scripts/gate.sh` refuses any
`unsafe` in a crate of the workspace, and any manifest that lifts
`unsafe_code`; it knows one boundary, the CEF host's, named in it by file
(ADR 0080). ADR 0080 says alo's own process holds no `unsafe`. A boundary
in `alo-window` is therefore a change to that rule and to the gate's check
of it — the owner's, as ADR 0080 was ("the owner's *Yes*"), not an
iteration's. The check is not loosened to let this in.

**And it moves pixels on screen.** No reference render changes — the
window's words are the same — but one colour in seven reaches the display
a level apart in one channel, because the window server would match them
where Core Animation matched them before. 873's *never a pixel moved*
cannot be met by any route measured.

## The two questions

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
- **The gate's check widened** to let a workspace crate write `unsafe` — the
  owner's to decide, above, not this item's.
