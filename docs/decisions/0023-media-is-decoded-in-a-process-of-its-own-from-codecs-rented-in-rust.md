# ADR 0023 — Media is decoded in a process of its own, from codecs rented in Rust

**Status:** accepted
**Date:** 2026-10-06
**Context:** queue item 109, *audio and video playback through rented
decoders*, marked **needs ADR**: *"which decoders, on whose licence, and where
they run. `ROADMAP.md` refuses DRM and proprietary codecs outright"*; ADR 0001
(rent the physics — image and video codecs by name); ADR 0005, which deferred
exactly this: *"For media, where the rented codec is large and the attack
surface is historically the worst in any browser, it is a utility process more
restricted still — decided when [media] arrives"*; ADR 0009 (the engine is
MPL-2.0, and every rental has to sit beside that); ADR 0010 (the sandbox is
rented, failing to get one is fatal, and *a rented crate's `unsafe` is the
crate's, not ours*); ADR 0012 (every request says what caused it); ADR 0020 (a
renderer asks, the browser process decides); ADR 0021, which chose `rav1d` for
AV1 and is blocked on its release; `ROADMAP.md`'s *Not built*: no DRM, no
Encrypted Media Extensions, no proprietary codecs we cannot ship freely; law 4;
`LOOP.md`'s stage 2 clause 2 (hostile bytes return an error, never a panic);
and the code this has to fit — `alo-renderer`'s `host.rs` (one renderer per
site) and `sandbox.rs` (the profile applied at `exec`)

## The decision in one line

Audio and video are demuxed and decoded in **a media process per site**,
started by the browser process and confined by the **same sandbox profile as a
renderer**, holding nothing of the page but the bytes it was handed; audio is
read by **Symphonia** (MPL-2.0, safe Rust) for **Vorbis, FLAC, PCM and MP3**;
**AV1 video waits on the same `rav1d` release as AVIF**; **Opus waits for a
decoder that passes the tests every rental here passes**; VP8 and VP9 have no
decoder to rent; and AAC, H.264, H.265 and every DRM scheme are never played.

## Why this is a decision rather than a dependency line

Three questions are bundled in the item, and each is one somebody could get
wrong quietly. *Where* is the one ADR 0005 left open by name, and it decides
what a codec bug is worth to an attacker. *Which* decides what plays and what a
person is told does not. *On whose licence* is where a browser that calls itself
sovereign either keeps the promise in `ROADMAP.md` or ships somebody's patent
pool to get a video site working.

## The survey, taken 2026-10-06

| Candidate | What it is | Verdict |
|---|---|---|
| **Symphonia 0.6.1** (13 Aug 2026; 0.6.0 on 15 May 2026) | Pure Rust demuxing and audio decoding; MPL-2.0; its own documentation states *100% safe Rust*. FLAC, Vorbis, PCM and MP3 are rated *excellent*; Ogg, WAV, Matroska/WebM, MP4, AIFF and CAF containers | **Taken**, for the codecs in § 3 |
| Symphonia's Opus | Listed by the project as *in work* | Not a decoder yet; the most likely way § 4 is lifted |
| Symphonia's experimental video | Behind `exp-video-codecs`, which its own release notes say is not officially included | Not taken. A codec its authors do not offer is not one to put in front of strangers |
| `opus-rs` 0.1.34 (BSD-3-Clause; first release February 2026) | A pure-Rust port of libopus 1.6, with reach (about seventy thousand downloads a month, sixteen dependents) | **Not yet** (§ 4): its crate root allows `unsafe` throughout rather than forbidding it, and it is eight months old |
| `ruopus` 0.1.2 (MIT; first release June 2026) | Pure Rust, passes all twelve RFC 8251 conformance vectors, `unsafe` denied except in SIMD loops that no feature turns off | **Not yet** (§ 4): three months old, about a thousand downloads a month |
| libopus through `opus`/`audiopus` | The reference decoder | **C**, in the process that decodes a stranger's bytes — ADR 0021's refusal of dav1d, again |
| `rav1d` | AV1, Rust, BSD-2-Clause | **Chosen by ADR 0021**, and blocked there on a release carrying its safe Rust API (§ 5) |
| `vpx-rs` and the other VP8/VP9 crates | Bindings to Google's libvpx; `vp9-parser` reads a VP9 bitstream's headers and decodes nothing | **No VP8 or VP9 decoder in Rust exists to rent** (§ 5) |
| AAC, H.264, H.265 | Licensed through patent pools | **Never** — `ROADMAP.md`'s proprietary codecs (§ 6) |
| GStreamer, FFmpeg | Whole media frameworks | **C**, LGPL or GPL, and a second media engine in the renderer's neighbour |
| The operating system's decoders (AVFoundation and the like) | What a native app would call | **Refused** (*Alternatives*) |

## 1. Where: a media process per site

ADR 0005 put untrusted bytes in *"the least privileged process that can do the
work"*, and said media would get something more restricted than a renderer.
This is that, made concrete.

- **One media process per site**, started by the browser process the first
  time that site's renderer asks for media, and reaped with the site's
  renderer (item 64's rule: the last tab on a site closing stops it). Per site
  rather than per element because the Spectre argument in ADR 0005 is about
  which *site's* data shares an address space, and a process per element buys
  no isolation that argument recognises while costing a process each.
- **The renderer's profile, unchanged.** `sandbox.rs`'s profile already allows
  nothing but the two inherited pipes; there is no tighter Seatbelt profile to
  write, so *"more restricted still"* is made of what the process **holds**,
  not of what it may call: no document, no script heap, no cookies, no other
  element's state — only the encoded bytes it was handed and the samples and
  frames it hands back. A codec bug that takes it over gains one site's media
  and two pipes. The same codec bug inside the renderer would have had the
  page's script heap and document, which is why the codec is not put there,
  though a picture is.
- **Confined at `exec` and fatal if it cannot be**, as a renderer is (ADR
  0010). The Linux layers follow item 169 for both processes together.
- **It dies alone.** Its death is that site's media failing, said in the
  element as a decode error; the renderer keeps rendering, the tab keeps its
  page, and it is not restarted silently (ADR 0005). The next media request
  starts a new one.
- **Never the browser process.** Not even to sniff a container's first bytes;
  which demuxer a file needs is decided inside the media process.

## 2. How the bytes move

- **A renderer asks for a media resource** as it asks for a navigation (ADR
  0020): a claim in its answer, never a call. The browser process fetches it
  with a cause (ADR 0012) under the page's policies — the network is the
  browser process's and stays so.
- **The browser process forwards the bytes to the media process unread.**
  Forwarding a buffer is not parsing it.
- **Decoded audio goes to the browser process**, which owns the device as it
  owns the display (§ 7). **Decoded video frames go to the renderer**, which
  draws them in the element's box as it draws a picture.
- **Everything the media process says is a stranger's bytes** to whoever reads
  it, as a renderer's messages are since item 63: a frame's size is checked
  against the size it said it would have before anything is reserved for it,
  and a sample buffer against its stated channels and length.
- **The protocol is coarse** (ADR 0005): a buffer of samples or a frame at a
  time, never a sample at a time. Shared memory instead of pipes is a speed
  question, measured on hardware before it is asked.

## 3. Audio: Symphonia, for Vorbis, FLAC, PCM and MP3

**Symphonia is the audio demuxer and decoder**, because it is the one candidate
that passes all four tests every rental here has passed: Rust, safe by its own
statement, under a licence that sits beside MPL-2.0 (it *is* MPL-2.0), and the
reach of the Rust ecosystem's standard audio crate.

**On these terms:**

- **Default features off**, then exactly: the `ogg`, `wav` and `mkv`
  containers, and the `vorbis`, `flac`, `pcm` and `mp3` codecs. MP3's patents
  have expired, and Vorbis, FLAC and PCM never had a licence to pay.
- **`aac` stays off**, by § 6. **`isomp4` stays off** until a page needs an
  MP4 holding something this list decodes: nearly every MP4's audio is AAC, and
  a container opened only to refuse its contents is surface without use.
- **ALAC, ADPCM, AIFF and CAF stay off** until a page needs them. Each is
  legal; none is the web's.
- **The `opt-simd` features stay off**, as `rav1d`'s assembly does under ADR
  0021: speed is a claim made on hardware after a measurement.
- **Every size is ours, and asked first**: at most 8 channels and a sample rate
  between 8 000 and 384 000 Hz, or the track is refused by name; a declared
  duration is a claim, never an allocation; samples are decoded a packet at a
  time into a bounded buffer, so a file that claims to last for a week costs
  what has been decoded, not what was claimed. The buffer's bound is the
  building commit's, with its reason.
- **Every byte flipped and every prefix cut** of every frozen file, as for
  pictures; a panic found inside Symphonia is refused in front of it with a
  regression test, and reporting it upstream is a person's call, as it was
  for `image-webp`.
- **Named in one file** of a new crate, `alo-media`, added to `gate.sh`'s
  boundary list in the commit that adds the dependency.

## 4. Opus waits

Opus is the web's audio codec — WebM's usual audio, and all of WebRTC's — and
saying it waits is saying most audio on the web does not play yet. That is
written here rather than discovered.

No Opus decoder passes all four tests today. libopus is C. `opus-rs` has the
reach but its crate root allows `unsafe` throughout rather than confining it,
and it is eight months old. `ruopus` has the clearest correctness evidence of
any — the conformance vectors — and is three months old with few users. ADR
0021 declined `oxideav-av1` on exactly this ground, and the ground has not
moved.

So **Opus is refused by name**, and its item is **blocked: no Opus decoder to
rent**. It is lifted by the first of:

- **Symphonia shipping its own Opus decoder** in a release. It is the crate
  already rented, the boundary file already exists, and it is the likeliest
  route.
- **A pure-Rust Opus decoder** with a year of releases, the RFC 8251 vectors
  passing, users other than its authors, and `unsafe` either forbidden or
  behind a feature this repository leaves off. `ruopus` and `opus-rs` are
  both candidates and neither is rejected on merit.

## 5. Video: AV1 when `rav1d` releases, and nothing else yet

- **AV1 is decoded by `rav1d` on ADR 0021's terms** — its safe Rust API, its
  assembly off, one thread per stream — in the media process rather than the
  renderer, because a moving picture is media and not a picture. It is
  **blocked on the same release** 269 is, and lifted by it.
- **The container for video** — WebM through Symphonia's Matroska reader if it
  hands over video packets, or an MP4 reader if a page needs one — is the
  building commit's to settle against this ADR's four tests, in the same one
  file as the decoder.
- **VP8 and VP9 are not played**, because no decoder for either exists in Rust
  to rent and libvpx is C. They are reopened when one exists, on § 4's terms.
- **An animated GIF or WebP**, drawn today by its first frame, is not media
  and is not moved here. Its playback is a picture's timeline and needs no
  codec this ADR rents; it is its own item when a page needs it.

## 6. Never played

- **AAC, H.264 and H.265.** Each is licensed through patent pools, which is
  what `ROADMAP.md` means by *proprietary codecs we cannot ship freely*. This
  holds even when a rented crate decodes one: Symphonia's AAC decoder exists
  and its feature stays off.
- **DRM and Encrypted Media Extensions**, by `ROADMAP.md`'s *Not built*.
  `requestMediaKeySystemAccess` is never defined, so a page asks and is told
  no rather than being handed a stub that fails later.

**And nothing claims what does not play.** Pages choose formats by asking —
`canPlayType`, `<source type>`, `MediaSource.isTypeSupported` — and a browser
that answered yes for a format it cannot decode would be handed that format
instead of the one the page had ready. So, as ADR 0021 § 6 said for pictures,
every such answer is **derived from what `alo-media` decodes**, never written
out beside it, and a format enters it in the change that makes it decodable.
A page that offers Vorbis or MP3 beside AAC therefore gets the one we play.

## 7. Sound reaches a speaker through the browser process

**The audio device is the browser process's**, as the display is. A renderer
has no device and the media process has only pipes; neither gains one.

The crate that talks to the device is rented, and it will contain `unsafe`
calling the platform's audio interface: that is ADR 0010's *the crate's, not
ours*, and **this decision authorises no `unsafe` in this repository**. Which
crate is the building item's choice, by § 3's tests.

**What needs hardware, said once.** Everything up to the buffer handed to the
device — what was decoded, in what order, at what rate, bounded how — is
verified on files, on any machine. That a speaker made the sound, and that the
sound and a moving picture stay in step, are on `LOOP.md`'s short list of what
needs a real device, and every item touching them says so rather than claims
it.

## 8. Whose clock

The device's clock decides what time it is in a playing medium, and it is in
the browser process. **The renderer is told**, in a message, and never asks
synchronously (ADR 0005); what a script reads as `currentTime` is the last time
it was told. How often it is told is the building item's, measured on hardware
before it is called smooth.

## What this costs

- **Most of the web's audio and all of its video do not play when the first
  items land**: Opus and VP9 are the web's, AV1 waits on a release, and AAC and
  H.264 are refused for good. A page that offers Vorbis, FLAC or MP3 plays.
  That is the price of renting nothing in C and nothing on a patent pool, and
  it is paid in the open: the element says it cannot decode, and the negotiation
  never asks for what we cannot play.
- **A second kind of process.** Its spawn, confinement and reaping are new code
  beside `host.rs`, and its pipe is a second wire format to keep hostile-proof.
- **Pictures and video are decoded in different places**, so a moving AVIF
  sequence and a still AVIF are read by the same decoder in two processes. That
  is deliberate: a still picture needs no second process (ADR 0021's last
  alternative) and a stream does.

## Alternatives rejected

**Decode in the renderer.** It is where pictures are decoded, and it would save
a process. Rejected because ADR 0005 already said media is the worst surface in
any browser, and the renderer holds the page's script heap and document — the
two things a codec bug should not be able to reach.

**Decode in the browser process.** Rejected without discussion; ADR 0005's
first rule.

**The operating system's decoders.** They play AAC and H.264 under the
platform's licence and are fast. Rejected: calling them is FFI this repository
would write, which is `unsafe` of ours; their output differs by platform, so a
reference render would mean three references; and they would bring in through
the side door exactly the codecs `ROADMAP.md` keeps out by the front. Reopening
this is a person's call about the roadmap's *Not built*, not an iteration's.

**GStreamer or FFmpeg.** C, LGPL or GPL, and a whole second media framework
with its own threads, plugins and policies, in the process next to the page.

**`opus-rs` or `ruopus` now.** Rejected for now in § 4, on reach and age, not
on merit.

**A media process per element, or one shared by every site.** The first costs
a process per `<audio>` for isolation ADR 0005 does not ask for; the second
puts two sites' streams in one address space, which is what ADR 0005 exists to
prevent.

**Our own decoders.** ADR 0001 and `ROADMAP.md`'s *never*.

## What this does not decide

- **`HTMLMediaElement`'s members**, `<video>`'s box and poster, controls, and
  their order — cut when a page opens them.
- **An autoplay policy.** It is a decision about a person's attention and gets
  its own record before anything plays without being asked.
- **Resampling to the device's rate**, buffer sizes, and how often the clock
  is told — the building items', with reasons.
- **Media Source Extensions (110), Web Audio (111), and camera, microphone and
  WebRTC** (93 and later), which consume this but are not it.
- **Which crate talks to the audio device** (§ 7).

## How we will know if this was wrong

**If Symphonia ships Opus, or `rav1d` its API**, the blocked items are lifted
and built on these terms; nothing here is revisited.

**If a frozen real page needs Opus or VP9 and neither has a rentable decoder**,
that page is the evidence for a person to weigh a young crate's age against a
page that does not play. It is a person's call, as ADR 0021 left the same
trade for AVIF.

**If a media process per site turns out to cost more than a person's machine
can hold** — measured on hardware — sharing one between a site's tabs is
already what this says; sharing one between sites is not, and changing that
reopens ADR 0005.
