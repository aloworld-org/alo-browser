# ADR 0021 — AVIF waits for a decoder we can call without `unsafe`

**Status:** accepted
**Date:** 2026-10-06
**Context:** queue item 269, *AVIF*, cut from 180 and marked **needs ADR**: an
AVIF is an AV1 frame in an ISO-BMFF box, *"the box is the easy half"*, and
*"which decoder, on what licence, behind what boundary, is the decision"*;
ADR 0001 (rent the physics, build the engine); ADR 0005 (untrusted bytes are
decoded in the least privileged process that can do it, and *the physics we
rent has `unsafe` in it*); ADR 0009 (the engine is MPL-2.0, and a rented crate
has to sit beside that); ADR 0010's *whose `unsafe` this is* — **a rented
crate containing `unsafe` is the crate's `unsafe`, not ours**, and FFI we would
have to write ourselves is a different decision; ADR 0015, which rented
`num-bigint` on the same three tests used here (pure Rust, licence, reach) and
refused GMP for being C; law 4 in `CLAUDE.md`; `LOOP.md`'s stage 2 clause 2
(a rented crate must not be handed input it panics on); and the code this is
about — `alo-paint`'s `picture.rs` (format sniffing and the one bound,
`agreed_size`), `jpeg_picture.rs`, `gif_picture.rs` and `webp_picture.rs`,
which are the shape an AVIF reader would take

## The decision in one line

AVIF is read by **two rented crates** — **`avif-parse`** for the box and
**`rav1d`**, through **its own safe Rust API**, for the AV1 frame — with the
colour conversion between them ours; and because the only `rav1d` on
crates.io today offers nothing but a C interface, which this repository could
call only by writing `unsafe` itself, **item 269 is blocked until `rav1d`
publishes a release carrying that API**, and until then an AVIF is refused, as
it is today, and never claimed.

## Why this is a decision rather than a dependency line

GIF and WebP were a dependency line each, because for both there was a crate
that was pure Rust, `#![forbid(unsafe_code)]`, permissively licensed and
widely used. For AV1 no crate passes all four, and which test gives way is the
whole question. Iteration 166 surveyed it once and asked for the survey to be
redone here rather than trusted; this is that survey, taken from crates.io and
from each crate's published source on 2026-10-06.

| Crate | What it is | Why it is not taken today |
|---|---|---|
| `dav1d` 0.11 over `dav1d-sys` | Bindings to VideoLAN's dav1d, which every major browser ships | **C**, in the renderer, built by a C toolchain this repository does not otherwise need. ADR 0015 refused GMP for being C in the process that runs a stranger's input; dav1d is the same trade |
| `rav1d` 1.1.0 (May 2025, BSD-2-Clause) | ISRG's Rust port of dav1d, the memory-safety project's own answer to exactly this question | **Its only public interface is dav1d's C ABI** — `pub unsafe extern "C" fn dav1d_open` and the rest. Calling it is `unsafe` in *our* code, which is the FFI ADR 0010 says is a different decision. The crate's own `unsafe` (about five hundred occurrences in fifty-five thousand lines, with the assembly feature-gated) is ADR 0010's *the crate's, not ours*, and would be acceptable |
| `rav1d`, `main` branch | The same, with `src/rust_api.rs`: dav1d-rs's safe API, merged 2026-04-04 (rav1d #1439), and made not to force panics on 2026-05-05 (#1484) | **Unreleased.** See § 2 |
| `re_rav1d` 0.1.3 | A fork of rav1d by Rerun with a safe API | **Archived** since October 2024. A codec nobody is fixing is not one to put in front of strangers' bytes |
| `rav1d-safe` 0.6, `zenavif` 0.1 | A rav1d fork that is `forbid(unsafe_code)` unless assembly or its C interface is enabled — the closest technical fit of any | **AGPL-3.0 or a commercial licence.** ADR 0009 chose MPL-2.0 so that the engine can be embedded in a closed product; an AGPL dependency removes that for every embedder |
| `oxideav-av1` 0.1.20 (MIT) | A clean-room AV1 decoder in pure Rust with no `unsafe` at all | **Reach and maturity.** Six months old and about six thousand downloads, and its own crate documentation calls it an *"orphan-rebuild scaffold"* whose *"decoder/encoder pipeline is not wired up yet"*, while its README reports conformance streams decoding. A decoder whose own documents disagree about whether it decodes is not one to rent yet. Not rejected on merit; see *How we will know* |
| `gamut-avif` 1.1 (MIT or Apache-2.0) | A pure-Rust AVIF **container** reader, `forbid(unsafe_code)` | It decodes everything but the frame, and asks the caller for an AV1 decoder. It answers the easy half only |

So no AV1 decoder can be rented today without breaking one of law 4, ADR
0009, or the reach that made every other codec here safe to take. The
decision is which one to wait for, what the reader looks like when it arrives,
and what an AVIF does meanwhile.

## 1. The frame: `rav1d`, through its Rust API, and only that

**`rav1d` is the AV1 decoder**, for four reasons that together no other
candidate has:

- **It is Rust**, and the only Rust decoder whose output is dav1d's, which is
  what every other browser draws. A picture that renders differently here than
  everywhere else is a bug report nobody can act on.
- **Its `unsafe` is its own.** With the API in § 2, this repository calls safe
  functions and writes no `unsafe`, which keeps ADR 0010's line exactly where
  it was drawn: the crate's `unsafe` is the crate's, and `unsafe_code =
  "forbid"` stays on `alo-paint` unchanged.
- **BSD-2-Clause**, which sits beside MPL-2.0 as every other rented licence
  here does.
- **It is a memory-safety project's decoder.** Its reason to exist is the
  reason ADR 0005 gives for the sandbox: codecs are the richest source of
  memory bugs in a browser. Its remaining `unsafe` is being removed by people
  whose job is removing it.

**On these terms:**

- **Default features off**, then `bitdepth_8` and `bitdepth_16` on. The `asm`
  features stay off, so no hand-written assembly is built and nothing but Rust
  is compiled — it is slower, and speed is a claim this repository makes only
  on hardware after a measurement.
- **One thread and no frame delay.** A still picture gains nothing from a pool,
  a renderer under ADR 0010's sandbox should not grow threads per picture, and
  a single-threaded decode is the deterministic one a reference render needs.
- **Its `frame_size_limit` set from `MOST_PIXELS`**, as a second layer
  behind § 4, never as the only one.
- **Named in one file**, `crates/alo-paint/src/avif_picture.rs`, added to
  `scripts/gate.sh`'s boundary list in the commit that adds the dependency, as
  `gif` and `image_webp` are.

## 2. Why the API has to be released, and why a git pin is not the answer

The tempting answer is to depend on `rav1d`'s `main` branch at a commit, which
has the API today. It is refused, for three reasons:

- **Reach is the protection, and a commit has none.** The argument for renting
  a codec is that its bugs are found by everyone who runs it. A release is what
  others run; a commit we chose is a decoder only we run.
- **A release is what security fixes are announced against.** Advisories name
  versions. A pinned commit is a codec whose fixes we would have to find by
  reading somebody else's history.
- **Nothing in this workspace is a git dependency**, and the first one would be
  in the process that decodes strangers' bytes.

**Nor do we write the FFI ourselves against 1.1.0.** It would be the first
`unsafe` in this repository, in the renderer, around a C interface whose
contracts (who frees a picture, which pointer outlives which call) are the
kind a Rust port exists to remove. Law 4 allows a reviewed, named boundary with
a written reason; the reason here would be *we did not want to wait*, and
`CLAUDE.md` says a date never justifies a shortcut.

So item 269 is **blocked: a `rav1d` release carrying `rust_api.rs`**. That is a
real block and the queue says so. It is lifted by the release, and by nothing
an iteration decides.

## 3. The box: `avif-parse`

The ISO-BMFF container is rented from **`avif-parse`** 2.x:

- **MPL-2.0** — the engine's own licence.
- **Mozilla's lineage.** It is a fork of `mp4parse`, the MP4 parser Firefox
  ships for untrusted files, maintained as its own crate by the author of
  `ravif` and `avif-serialize`, which are among its fifteen dependents.
- **Fallible allocation.** It reads into `fallible_collections`, so a box that
  claims more than can be allocated is an error rather than an abort.
- **Its only `unsafe` is its optional C interface** (`c_api.rs`, behind the
  `c_api` feature), which is not enabled.
- **It returns the primary item and the alpha item as AV1 bytes, and the
  sequence header's size before any frame is decoded** — which is the hook § 4
  needs.
- **Named in the same one file** as `rav1d`, since the two together are one
  format's reader, and the boundary check names both.

`gamut-avif` is the alternative and is not rejected on merit: pure Rust,
`forbid(unsafe_code)`, permissive, and it does more (grids, `irot`, `imir`,
colour). It is four months old and brings five crates of its own; reach decides
it, as it decided `num-bigint` over `ibig` in ADR 0015. Swapping one container
crate for another touches the one file in § 1.

## 4. Every size is ours, and asked first

The same rule as every other format, which is why AVIF joins the same list
rather than getting its own:

- **The `ispe` size, and the AV1 sequence header's maximum frame size, both go
  through `agreed_size`** before `rav1d` is handed a byte. Both are claims a
  stranger wrote; a picture whose sequence header admits more than its box says
  is refused rather than trusted on either.
- **The alpha item must be the primary item's size**, checked before it is
  decoded. A WebP frame whose picture was not the frame's size was the panic
  iteration 166 found in `image-webp`, and the same shape is checked here first
  rather than discovered.
- **The decoded picture must be the size agreed**, or it is refused rather than
  cropped or stretched.
- **Every byte of every frozen AVIF is flipped and every prefix cut**, through
  `picture::read`, as the other nine frozen files are. A panic found inside
  `rav1d` that way is refused in front of it with a regression test, and
  reporting it upstream is a person's call, as it was for `image-webp`.

## 5. The colour between them is ours

`rav1d` hands back planes of luma and chroma; a canvas holds RGBA. The
conversion is **ours**, in `avif_picture.rs`, because it is the specification's
arithmetic rather than physics — a matrix per ITU-T H.273 matrix-coefficients
code, a full or limited range, a chroma plane at half or full resolution, and a
rounding rule — and it is exactly where two engines' pictures visibly differ,
which is the line ADR 0015 drew around what it rented.

- **Matrices built:** BT.709 (1), BT.601 (5, 6), BT.2020 non-constant
  luminance (9), and identity (0, where the planes are already G, B, R). Any
  other code is **refused by name** rather than drawn with a guessed matrix
  (ADR 0013 § 3: absent beats approximate).
- **Transfer functions:** sRGB-like transfers are drawn as the other formats'
  pixels are. **PQ and HLG are refused by name**: an HDR picture drawn without
  tone mapping is not a picture of the right colour, and tone mapping is its
  own decision.
- **10- and 12-bit** samples are reduced to 8 by a rounding the commit states
  and a unit test pins.
- **How chroma is upsampled** is decided by the commit that builds it, with its
  reason beside it and a reference render that says what it looks like — a
  filter written into an ADR is a filter nobody can tune with evidence.
- **An ICC profile** in `colr` is treated as PNG's and JPEG's are today: not
  applied. That is an engine-wide gap, not an AVIF one, and it is not decided
  here.

## 6. What an AVIF does until then, and what we never claim

**Refused, exactly as today**: `Format::of` does not recognise it, `read`
returns an error, and the `<img>` keeps the box its style asked for, like any
picture that did not arrive (item 176). Nothing changes in the code for this
decision.

**And the browser never says it reads AVIF while it does not.** Pages choose
AVIF by negotiation — an `Accept` header that lists `image/avif`, or a
`<picture>`'s `<source type="image/avif">` — and a browser that claimed it
without decoding it would get the AVIF *instead of* the JPEG the page had
ready. So wherever this engine comes to state the picture formats it reads,
that list is **derived from the formats `picture.rs` decodes**, never written
out beside them, and AVIF enters it in the same change that makes it
decodable. Today nothing states a list — no image `Accept` is sent and
`<picture>` is not built — so this is a rule for the commit that first does,
not code owed now.

## 7. Grids and sequences

**A grid AVIF** — one picture assembled from tiles, which cameras produce for
large photographs — is refused by `avif-parse` by name. It stays refused when
269 is built; assembling tiles is its own item when a frozen page needs one.

**An image sequence** (`avis`) is drawn by its still primary item when it has
one, as an animated GIF or WebP is drawn by its first frame; one without is
refused. Playing it is item 109's.

## What this costs

- **AVIF stays unread for an unknown time.** No date for a `rav1d` release has
  been announced. Pages that offer AVIF by negotiation lose nothing, because
  § 6 keeps us from asking for it; a page that serves AVIF to everybody shows
  an empty box. That is the price, and it is paid by a page the
  negotiation already tells to do otherwise.
- **The decoder is slower than anyone else's** with its assembly off. Measured
  on hardware before it is called slow, and turning the assembly on is a
  decision about hand-written assembly in a sandboxed renderer — this ADR does
  not take it.
- **Two rented crates for one format**, where the other formats needed one.
  One boundary file holds both, so the cost is a line in `gate.sh` rather than
  a second place to look.

## Alternatives rejected

**dav1d, through `dav1d-rs`.** Rejected: C, as above. It is what Chromium and
Firefox use and it is very good; it is also the QuickJS and GMP trade again, in
the process ADR 0005 built for the case where a stranger's bytes find a bug.

**Our own FFI to `rav1d` 1.1.0.** Rejected in § 2: the first `unsafe` in this
repository, written to avoid waiting.

**A git dependency on `rav1d`'s `main`.** Rejected in § 2.

**Our own AV1 decoder.** Rejected by ADR 0001 and by `ROADMAP.md`'s *never*:
no codec of our own. AV1 intra decoding alone is transforms, prediction, two
loop filters, CDEF, restoration and film grain — physics, and young physics
written by its only user is the trade every rental here refuses.

**`rav1d-safe`.** Rejected on licence alone, and it is the closest call
technically. Re-opened if it is ever offered under a licence that sits beside
MPL-2.0.

**Decode AVIF in a separate, more restricted process**, as ADR 0005 plans for
media. Not needed: the renderer is already the least privileged process that
can do the work for an image, which is the position ADR 0005 takes for every
other picture format, and a still AVIF is a picture.

## What this does not decide

- **The upsampling filter, the bit-depth rounding and the colour tests' tolerances**
  — the building commit's, with reasons (§ 5).
- **Tone mapping, ICC profiles and wide gamut**, which are engine-wide.
- **Turning on `rav1d`'s assembly.**
- **What an image `Accept` header says**, beyond § 6's rule that it is derived
  rather than written.

## How we will know if this was wrong

**If `rav1d` publishes its API**, 269 is unblocked and built on these terms;
nothing here needs revisiting.

**If `rav1d` stops being maintained** — no release and no commits for a year
— this decision is reopened, and the first candidate is `oxideav-av1` (or any
other pure-Rust decoder) once it has a release whose documentation and
behaviour agree, a conformance corpus it decodes, and users other than its
authors.

**If a frozen real page needs AVIF** and the release has still not come, that
page is the evidence for a person to weigh the trade in § 2 again. It is a
person's to weigh, because the alternative is the first `unsafe` in this
repository or a codec only we run, and an iteration does not choose either
on its own.
