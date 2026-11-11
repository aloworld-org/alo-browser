# ADR 0113 — A card drawn with no window of Chromium's

**Status:** proposed — queue item 959, 11 November 2026. Not accepted: the
owner accepts, and two questions below are the owner's alone (*Asked of the
owner*). Nothing alo starts changes until then: the spike is behind a switch
alo never gives.
**Date:** 2026-11-11
**Context:** ADR 0076 (a card is a parked window of Chromium's, streamed;
every amendment, the sixth's exact sRGB above all); ADR 0058 (Chromium is a
renderer we drive, over the pipe); ADR 0060 (no request leaves before the
browser process lets it go — every target held, waiting, from Chromium's own
session); ADR 0069; ADR 0077; ADR 0079 § 3.2–3.3 (CEF, Chrome style because
of CEF issue #4308, and *"must not use off-screen rendering"*); ADR 0080 (the
host's one `unsafe` boundary); ADR 0090 (the card's scale and the stream's
pace); ADR 0105 (the workspace's one boundary); ADR 0112 (alo OS: Wayland
only, no GPU required, the accessibility tree to AT-SPI); ADR 0002 (the tree
is the agent's; no verb takes a coordinate); law 4; queue items 959, 960,
965, 967

## The question in one line

Wayland lets no client place a window or put one off the screen, and today a
card is a window of Chromium's parked off the screen and streamed, and
*Expand* places that window over alo's: can a card be drawn with **no window
of Chromium's at all** — CEF's windowless (off-screen) rendering — and what
does that give, cost and lose?

## What CEF 156 says (read in its own headers)

From `$CEF_PATH/include` of CEF 156.0.14 (Chromium 156.0.8078.48), the
distribution the host is built against, and the `cef` crate
156.2.0+156.0.14's bindings:

1. **Windowless is Alloy style, always.** `internal/cef_types_runtime.h`:
   *"Windowless rendering will always use Alloy style."*
   `internal/cef_types_mac.h` (`cef_window_info_t.runtime_style`): *"Alloy
   style will always be used if |windowless_rendering_enabled| is true."*
   Every page of the host today is Chrome style, for ADR 0060's hold (ADR
   0079 § 3.2, CEF issue #4308).
2. **Turned on twice:** `CefSettings.windowless_rendering_enabled` for the
   process (*"do not enable this value if the application does not use
   windowless rendering as it may reduce rendering performance"*,
   `internal/cef_types.h`), and `cef_window_info_t.windowless_rendering_enabled`
   for the browser. A windowless browser *"will be painted transparent
   unless `CefBrowserSettings.background_color` is opaque"*. Its parent view
   is optional: without one *"the main screen monitor will be used and some
   functionality that requires a parent view may not function correctly"*.
3. **The render handler** (`cef_render_handler.h`): `GetViewRect` (the view,
   in CSS pixels), `GetScreenInfo` (`device_scale_factor` — so the scale is
   the host's to say, never a display's), `OnPaint` (*"|buffer| will be
   |width|\*|height|\*4 bytes in size and represents a BGRA image with an
   upper-left origin"*, with dirty rectangles; the view or the popup
   widget), `OnPopupShow`/`OnPopupSize`, `StartDragging`/`UpdateDragCursor`,
   `OnImeCompositionRangeChanged`, `OnTextSelectionChanged`,
   `GetAccessibilityHandler`.
4. **Shared textures.** `cef_window_info_t.shared_texture_enabled`'s comment
   still says *"Currently only supported on Windows (D3D11)"*, while
   `OnAcceleratedPaint`'s says that on macOS its handle *"is an IOSurface
   pointer"* and on Linux *"several planes, each with an fd"*, valid only
   inside the callback. Which is true on macOS was not tried.
5. **Frame rate.** `windowless_frame_rate`: at most this many `OnPaint`s a
   second, *"the minimum value is 1 and the default value is 30"*
   (`cef_browser.h`, `SetWindowlessFrameRate`); `external_begin_frame_enabled`
   lets the host pace frames itself (`SendExternalBeginFrame`).
6. **Input** goes to the browser host (`SendMouseClickEvent`,
   `SendMouseMoveEvent`, `SendMouseWheelEvent`, `SendKeyEvent`,
   `SendTouchEvent`), **input methods** through `ImeSetComposition`,
   `ImeCommitText`, `ImeFinishComposingText` — *"only used when window
   rendering is disabled"* — and **drags into the page** through
   `DragTargetDragEnter/Over/Leave/Drop`, out of it through `StartDragging`
   and `DragSourceEndedAt`.
7. **Accessibility.** `cef_browser.h`, `SetAccessibilityState`: *"For
   windowless browsers accessibility will be enabled in TreeOnly mode …
   renderer accessibility is enabled, the full tree is computed, and events
   are passed to CefAccessibiltyHandler, but **platform accessibility objects
   are not created**."* So a windowless page is in no platform tree — not
   VoiceOver's, not AT-SPI's — whatever Chromium is told.

What the `cef` crate gives: every call above is a safe function, and the
callbacks are built by its `wrap_render_handler!` macro — except that
**`on_paint`'s pixels arrive as `buffer: *const u8`**, and reading them is
`std::slice::from_raw_parts`, which is `unsafe`. The crate's
`osr_texture_import` (feature `accelerated_osr`) imports an
`OnAcceleratedPaint` texture into `wgpu` through
`unsafe { SharedTextureHandle::import_texture(…) }` — `unsafe` of the
caller's too. *Not verified:* CEF's online documentation was not read this
time; the architecture page ADR 0079 cites [5] says the same of Alloy
style.

## The spike (built, behind a switch alo never gives)

`crates/alo-cef-host`: told `--alo-windowless=WIDTHxHEIGHT@SCALE` beside
`--app=`, the host turns windowless rendering on for the process and opens
the page with `WindowInfo::set_as_windowless(null)` and
`browser_host_create_browser`, `windowless_frame_rate` 60, an opaque white
ground, no parent view (`window.rs` `open_windowless`, `Drawn`;
`windowless.rs`; `switches.rs` `windowless`; `host.rs`). The page's client is
a parked card's for its life (`Stage::Parked`: told lines, prompts
dismissed, its own moves refused) and the host stays an accessory
application. **It reads no pixel**: reading `OnPaint`'s buffer is the new
`unsafe` this ADR asks the owner about, so each paint is said on the host's
error output as one line (time, view or popup, size, dirty rectangles), and
the frames reach the probe as they reach alo today — `Page.startScreencast`
over the DevTools pipe. No new `unsafe`; `cef` is still named in only the
three files ADR 0080 lists.

## Measured

**On this Mac** — an M3 (8 GB), macOS 26.5, the built-in Retina panel at
two and the VG27VQM at one to its right — through the host built from this
tree (debug, the host's own workspace, as the tests build it), laid out as
`cef_host::assemble` lays it out, driven over `--remote-debugging-pipe` by
a Python probe of the scratchpad's (`osr/probe.py`, `osr/cdp.py`) started as
`chrome.rs` starts it (descriptors 3, 4 and the told 5), with ADR 0076's
sixth amendment's `--force-color-profile=srgb`. The page: 880 × 600 CSS, the
sixth amendment's eight stripes, a field, a *Send* button in a form, a link,
a closed list, a draggable box and a box that spins. Every host was closed
and its profile removed; Chrome was not touched.

**The main display was asleep throughout** (`CGDisplayIsAsleep` 1; the
screen not locked). That decided what could be compared, below.

### What works on a windowless page (all four: scale 1 and 2, GPU and software)

| | |
|---|---|
| the target | listed as a `page` at its address; `Target.attachToTarget` |
| `Browser.getWindowForTarget` | **refused**: `-32000 Browser window not found` |
| the tree | `Accessibility.getFullAXTree` 30 nodes; the button found by its name, *Send* |
| activate by name | `DOM.resolveNode` of its node, `click()`: heard |
| put text | `DOM.focus` then `Input.insertText` `Ada`: the field `Ada` |
| an input method | `Input.imeSetComposition` `に` → `Adaに`; `Input.insertText` `日本` → `Ada日本` |
| press | Return (`Input.dispatchKeyEvent`): the form sent, `Ada日本` |
| `DOM.getDocument` | answered |
| `Page.printToPDF` | 29,274–29,275 bytes, `%PDF-` |
| the screencast | `Page.startScreencast` PNG at `maxWidth`/`maxHeight` of the view × scale: frames 1760 × 1200 at two, 880 × 600 at one; nothing while the page is still |
| `Page.captureScreenshot` | the same sizes |
| colours | **every stripe exact** (`#2a9d8f` read (42, 157, 143), and the other seven) in the stream and the screenshot, at one and two, GPU and software |
| the pointer's shape | told `cursor pointer` over the link, `default` off it (display handler, as today) |
| a context menu | a right press told `menu 19 112 <link> A link 0` (as ADR 0076's fourth amendment) |
| a closed list | pressed, CEF said **`OnPopupShow`**, `OnPopupSize` 52 × 73 at (43, 122) CSS, and painted a **popup** 104 × 146 at two (53 × 74 at one): Chromium draws the list itself, no native menu; ↓ then Return chose *Two*, heard as `change` |
| a drag | the page heard `dragstart`; `StartDragging` was **not** called within half a second — not explained |
| resized with no window | `Emulation.setDeviceMetricsOverride` 600 × 400: the page 600 × 400 at its scale, the screenshot 1200 × 800 at two |
| `devicePixelRatio` | the viewport's scale, at either; no display under it |

### What a frame costs (a box spinning, ten seconds; CPU of the host and every helper, `ps`)

| windowless, scale, drawing | frames a second | MiB a second | cores | ms of CPU a frame | key → first frame showing it, median / p90 / max | key → its `OnPaint` |
|---|---|---|---|---|---|---|
| two, GPU, streaming | 60.1 | 2.9 | 0.734 | 12.2 | 26.3 / 44.5 / 122.0 | 6.5 / 24.5 / 114.0 |
| two, GPU, painting only | 60.3 paints | — | 0.237 | 3.9 | — | — |
| two, software, streaming | 60.1 | 2.9 | 0.851 | 14.2 | 36.0 / 37.2 / 54.0 | 21.2 / 22.2 / 38.4 |
| two, software, painting only | 60.1 paints | — | **0.145** | 2.4 | — | — |
| one, GPU, streaming | 60.2 | 1.0 | 0.379 | 6.3 | 13.5 / 15.0 / 15.3 | 6.8 / 7.6 / 8.1 |
| one, GPU, painting only | 60.2 paints | — | 0.267 | 4.4 | — | — |
| one, software, streaming | 60.2 | 1.0 | 0.380 | 6.3 | 14.3 / 15.3 / 16.4 | 6.9 / 7.8 / 9.2 |
| one, software, painting only | 60.2 paints | — | **0.098** | 1.6 | — | — |
| two, GPU, half the page noise, streaming | 40.9 | 125.3 | 1.916 | 46.9 | — | — |
| two, GPU, half the page noise, painting only | 60.3 paints | — | 0.452 | 7.5 | — | — |
| two, software, half the page noise, streaming | 40.6 | 124.5 | 2.209 | 54.4 | — | — |
| two, software, half the page noise, painting only | 60.4 paints | — | 0.514 | 8.5 | — | — |
| still (either) | 0 | 0 | 0.01 | — | — | — |

"Software" is `--disable-gpu --disable-gpu-compositing`. "Painting only"
is the stream stopped, `OnPaint` still called — what a frame costs Chromium
before anything hands it on; handing `OnPaint`'s pixels to alo would cost
what that hand-off costs on top, which was not built. Key latency is ADR
0076 B3's: a key that repaints the whole ground, to the first PNG whose
pixel shows the new colour (eleven, after one warm-up), on the probe's
clock; `OnPaint`'s is the first paint line after the key, both on the wall
clock. The probe answered each frame at once, unpaced: alo paces the stream
to 24 MiB a second (ADR 0090), which the noise page would be held to.

**Most of a streamed frame's cost is the screencast's own PNG, in the
browser process** (1.55 of 1.92 cores with the noise, 0.65 of 0.73 without);
the paint itself is a quarter to a seventh of it. **Software drawing was
cheaper than the GPU's when nothing is streamed** (0.145 against 0.237 cores
at two; 0.098 against 0.267 at one) and costlier only where it matters least
(streaming at two: 0.851 against 0.734); its key latency at two was 36 ms
against the GPU's 26.

### Against today's parked window: not on the same page

The same probe, `--alo-card`, the window fitted to 880 × 600 and parked
(−20000, 0, read back at −625, 34 by the built-in panel at two) or put on the
VG27VQM at one: the first frames came, every colour exact, and every verb
above answered as on the windowless page, with Chromium's own select menu
and no popup told. Then **no frame at all** — none for a key, none for the
spinning box, ten seconds. A check (`osr/vis.py`): the window just shown,
`requestAnimationFrame` ran 62 times a second; once moved, never in five
seconds; an opaque window, not even before it was moved. **With the display
asleep a windowed Chromium stops drawing, and a windowless one does not**
(60 paints a second, every row of the table above was measured asleep). So
today's stream is not re-measured here; its numbers stand as ADR 0076 B1
measured them (PNG at card size and two, 24.0–24.6 MiB a second, about one
more core of Chrome's while anything moves; key to frame 51.9–52.6 ms
median headless) and ADR 0090 as paced. The windowless page's key to frame,
26.3 ms at two, is half B3's — but on another page, another day, and not in
alo.

### ADR 0060's hold, through a `window.open` (CEF issue #4308)

The page's own session held (`Target.setAutoAttach`, waiting, `Fetch.enable`)
and a `window.open` run with a person's gesture
(`Runtime.evaluate … userGesture`):

| Chromium's own session held too (as `chrome_tab.rs` does) | `window.open` | the popup |
|---|---|---|
| yes | **never returned** (15 s) | never told of, never asked for — the server saw nothing |
| no, the page's session only | returned `true` in 132 ms | asked for at once, **told of to no session**, so never held |
| neither | returned `true` in 144 ms | asked for at once |

Without a gesture `window.open` opened nothing. And **a windowless host with
a popup did not end** on `Browser.close` within fifteen seconds, held or not
(each was killed, its profile removed); one with no popup ended with status
0. This is #4308 as ADR 0079 § 3.2 read it, now measured windowless: in
Alloy style the hold ADR 0060 needs stalls a popup; without the hold the
popup's requests leave unseen.

## What it gives

- **No window is placed, parked, shown, made transparent or kept out of the
  Dock**: none exists. The page draws whatever displays there are, asleep
  or not, at the scale the host says. Everything ADR 0076's amendments built
  to hide a parked window — the strip, transparency, the accessory policy's
  reason, Stage Manager, the window server's clamping, the sixth
  amendment's display-dependent colours — has no object.
- **The agent's surface is unchanged**: every verb measured answers over the
  pipe as on a window, the stream and its colours are exact, and the told
  lines are the same.
- **A closed list's list is drawn by Chromium as a popup widget** (handed to
  the host, `PET_POPUP`), keyboard-driven as a page's list is; not a native
  menu that can crash a host (ADR 0080) or needs the card expanded (ADR 0076
  amendment 4 § 5).
- **Software drawing works**, and is the cheaper of the two when still or
  painting only — ADR 0112's no-GPU rule met on this Mac at least.

## What it costs, and what is lost or must be rebuilt

- **Alloy style.** ADR 0060's hold stalls a popup (above). Before any card
  is windowless the hold must be rebuilt for it — most likely a popup never
  made: `OnBeforePopup` cancelled and told, opened as the conversation's
  next turn as a ⌘-press is (ADR 0076 amendment 4 § 4) — and the shared
  workers' hold, which needs Chromium's own session held, measured in Alloy
  style (not measured). Whatever else Chrome style gave a card and Alloy
  does not — its permission prompts (so `PermissionHandler` may never be
  asked), its Exit command, print preview, autofill — must be listed by
  measurement (not measured).
- **Expand has nothing to place.** It becomes alo drawing the same page's
  frames at the window's size: the view resized (`GetViewRect` asked again
  after `WasResized`), which needs a way for alo to tell the host a size and
  scale — the pipe carries only Chromium's protocol, and the told channel
  runs the other way. `Emulation.setDeviceMetricsOverride` resized the page
  with no channel (measured), but it is emulation, which ADR 0057 keeps for
  tests and ADR 0090 found changes no scale; a channel of the host's own is
  the honest road.
- **A person's input** still arrives as today, as `Input.*` over the pipe
  (measured: keys, presses, the composition); the host's own `Send*Event`
  is not needed. **An input method's window** stays alo's (ADR 0076
  amendment 4 § 6, unchanged).
- **A closed list's popup** is not in the screencast's view (*not checked*:
  the popup's paints were counted, the frames were not looked at for it), so
  alo draws its own list from the DOM, as an agent's choice already is
  (`agent_dropdown.rs`), or takes the popup's pixels from `OnPaint`.
- **A drag** out of the page: `StartDragging` never came (above); into it,
  `DragTarget*`. A **file chooser**: `CefDialogHandler`, then the portal on
  alo OS (967). Neither built.
- **Accessibility**: a windowless page has no platform accessibility objects
  (CEF's header, § 7) — **so `--force-renderer-accessibility`, which alo OS
  adds for Chromium-family applications, cannot put a windowless card's page
  in AT-SPI**: Chromium computes the tree and hands it to the host, and makes
  nothing an assistive technology reads. On macOS VoiceOver likewise reads
  nothing of a card's page today through Chromium's window either, since
  that window is transparent and parked (not measured). The page's tree must
  reach AT-SPI **through alo**, published as part of alo's own tree — which
  is ADR 0002's rule and item 965's road: the same `Accessibility` tree the
  agent reads (measured: 30 nodes) or `CefAccessibilityHandler`'s events.
  Not measured on Linux.
- **Frames to alo.** Two roads. (a) **The screencast, as today** — no new
  `unsafe`, measured exact, paced by ADR 0090; the PNG is most of the cost.
  (b) **`OnPaint`'s pixels**, handed to alo by the host — a quarter to a
  seventh of a streamed frame's CPU before the hand-off — which needs the
  host to read a raw pointer: **one new `unsafe` line**, in a named file
  (`std::slice::from_raw_parts` over `width * height * 4` bytes, inside the
  callback, valid only there). Handing them on: a pipe of dirty rectangles
  needs nothing more (a full frame at two is 8.4 MB; sixty a second would be
  ~500 MB/s, so only changes, or encoded); shared memory would need `unsafe`
  in alo's own process as well, which ADR 0105 allows only in
  `present_kept.rs`. `OnAcceleratedPaint` needs `unsafe` of ours, a GPU and
  a texture handed between processes — set aside (ADR 0112: no GPU
  required).
- **What stays macOS's**: the `NSApplication` subclass (CEF on macOS, ADR
  0080) and the menu bar; `unseen_mac.rs`, `strip_mac.rs` and
  `dismissed_mac.rs` become a windowed card's only. On Linux the host needs
  none of them, and a windowless browser needs no window of its own from the
  compositor at all — whether CEF on Linux starts with no display server
  connection (Ozone's headless platform, say) was not tried.

## The proposal

1. **Accept windowless rendering as the road for cards on alo OS**, and on
   macOS behind a switch until it is the better card there too.
2. **Frames by the screencast first** (road a): nothing new is `unsafe`, and
   alo's stream, pace and picture path stay as they are.
3. **No windowless card ships while ADR 0060's hold stalls**: the popup road
   (978) and the shared workers' hold under Alloy are built and measured
   first, and ADR 0060 amended for them.
4. ***Expand*** **becomes alo's own**: the card's frames drawn at the
   window's size, the view told its new size and scale over a channel of the
   host's (979); no window of Chromium's is ever placed.
5. **The page's accessibility tree reaches AT-SPI through alo's** (965),
   never through Chromium's platform objects, which a windowless page does
   not have.

## Asked of the owner

1. **A new `unsafe` boundary in the host?** Road (b) needs one line reading
   `OnPaint`'s buffer, in a file of its own named beside
   `application_mac.rs` (ADR 0080's form). This ADR proposes **not yet** —
   road (a) works — and asks the owner to say whether (b) may be measured
   behind it (981), or never.
2. **Alloy style for every card**, with Chrome style kept for pages a person
   opens in a window of their own? ADR 0079 § 3.2 chose Chrome style for the
   hold; this ADR proposes Alloy for windowless cards only once 978 has
   rebuilt the hold.

## Rejected

- **Today's parked window on Wayland**: no client may place or hide one.
- **CEF in alo's own process** (ADR 0079 § 3.3, option E): still reverses
  ADR 0058, and windowless does not change that.
- **Shared textures** (`OnAcceleratedPaint`): `unsafe` of ours, a GPU
  required, a handle valid only in the callback.
- **Emulation for *Expand*'s size**: emulation is for tests (ADR 0057).

## Queue items cut from it

978 the popup road and the hold under Alloy; 979 the host's channel for size,
scale, focus and hiding, and *Expand* as alo's; 980 windowless cards in alo
behind a switch, on this Mac; 981 (the owner's yes first) `OnPaint`'s pixels
to alo; 982 a closed list's popup, drags and the file chooser on a windowless
card; 983 what Alloy style takes from a card, measured. 960 (Linux) and 965
(AT-SPI) depend on them.

## What is not measured, and is not hidden

- Today's stream on the same page and day: the display was asleep (above).
- Anything on Linux, Wayland or alo OS; CEF on Linux with no display server.
- Whether a closed list's popup is in the screencast; why `StartDragging`
  never came; whether `OnAcceleratedPaint` works on macOS.
- `Target.createTarget` in a windowless host (it would open a Chrome-style
  window, shown); a page's permission prompt in Alloy style; shared workers
  held from Chromium's own session in Alloy style.
- Why a host with a popup did not end on `Browser.close`.
- The key latency from a key to alo's glass; CPU of alo's own process
  decoding the frames (the same as today's path, not re-measured).
- Memory.
