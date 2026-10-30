# ADR 0079 — alo carries Chromium as CEF

**Status:** accepted — chosen by the owner on 30 October 2026: **CEF now,
alo's own Chromium build (ADR 0078) before alo is offered widely.** The
first part of § 8's probe was done on this Mac the same day (§ 8a); the
host program was built the same day **with no `unsafe`** (§ 8b, queue item
725), so the `unsafe` boundary this ADR expected, and ADR 0080 for it, are
not needed; the rest is owed by 720, 726 and 727–730.

## The question in one line

ADR 0078 chose a Chromium that alo builds itself, for three systems, and
found that no machine alo has can build one. The **Chromium Embedded
Framework (CEF)** publishes Chromium builds, made to be carried inside
another application, for all three systems, every few days. Can alo carry
CEF instead — without giving up what ADR 0058 kept (a pipe, nothing linked
into alo's process, no `unsafe` in alo), the ROADMAP's refusals, or the
promise of security fixes within days?

## The answer in short

| | Answer | How sure |
|---|---|---|
| Builds for macOS, Windows, Linux | **yes**, arm64 and x64 on all three | documented [1] |
| Same version as Chrome stable | **no**: CEF 156.0.8078.48 is current; Chrome is 156.0.8078.80 | read from [1][3] |
| Security lag | usually **1–5 days**; on macOS **17 days** with no new build in October, and **3 days and counting** today | computed from [1][3], § 2 |
| Driven over `--remote-debugging-pipe` | **yes**, reported on CEF 156 macOS arm64 — with Chrome-style windows only | an issue report [6], not documentation |
| Nothing linked into alo's process | **yes**, if alo starts a small **host executable** of its own | inferred from [4][8] |
| No `unsafe` in alo | **not on macOS**: the host needs an `NSApplication` subclass, which is `unsafe` in Rust | read from [8] |
| The name a person sees | **the host application's own** — `alo` — with no build of Chromium | inferred, § 4 |
| No proprietary codecs | **yes**, off in CEF's builds | documented [10] |
| No Widevine | **the CDM is not shipped**, but Widevine *support* is compiled in and cannot be switched off without patching CEF | read from [9] |
| Build machines needed | **none** for Chromium; an ordinary machine per system to build the host, sign and package | inferred, § 5 |

## Why this is a decision

ADR 0078 is accepted, and its first step (723, build machines for three
systems) waits on money the repository does not have. CEF is the one
published build that is not Chrome for Testing, not a snapshot, not a
distribution with codecs — and it was not looked at in ADR 0078. If it meets
the criteria, it changes 720, 722, 723 and 724 at once. If it does not, the
reasons belong in the record so that it is not proposed again.

## 1. What CEF publishes (read 30 October 2026)

- **Who:** CEF is a BSD-licensed project (`LICENSE.txt`, Marshall A.
  Greenblatt and Google [11]) that adds an embedding API and a patch set to
  Chromium. Its binary builds are made by an automated builder hosted by
  Spotify, `cef-builds.spotifycdn.com`, which publishes `index.json` with
  every build, its Chromium version, its files, sizes and SHA-1 [1].
- **Platforms:** `macosarm64`, `macosx64`, `windows64`, `windowsarm64`,
  `windows32`, `linux64`, `linuxarm64`, `linuxarm` [1].
- **Current:** stable **156.0.14+g4fa4ab5+chromium-156.0.8078.48**, published
  24 October (macOS arm64 at 04:22 UTC) [1]. Chrome stable is
  **156.0.8078.80**, released 29 October with 21 security fixes, one Critical
  (ADR 0078 source [1]; [3]).
- **Which Chromium versions:** *"CEF is currently tracking even-numbered
  Chromium milestone branches (new CEF branch every ~4 weeks…). Support for
  older branches ends when they exit the Chromium stable channel (after 2
  weeks as stable), unless that branch also qualifies for extended and/or
  LTC/LTS channels."* Every fourth milestone from 152 is an extended branch
  (~8 weeks of fixes); an LTS branch (150 now) gets ~9 months [2]. Chrome
  moved to a stable release every two weeks from 153 (8 September) [2][3];
  CEF builds **no** odd milestone — there is no CEF 153 or 155 [1].
- **How fast:** *"Updating CEF branches is currently a manual process so
  there will likely be a delay between Chromium release announcements and
  the availability of associated CEF builds."* [2]
- **Distributions and sizes (macOS arm64, 156.0.14, compressed):**
  `minimal` 135 MB (release build only), `client` 135 MB (the `cefclient`
  sample, built), `standard` 312 MB (debug and release builds and the sample
  sources), `tools` 156 MB, `release_symbols` 2.1 GB. Windows x64: minimal
  175 MB, standard 367 MB, plus a `signed` package (140 MB, contents not
  checked). Linux x64: minimal 332 MB, standard 696 MB [1]. Unpacked sizes
  were not measured. For comparison, Chrome for Testing's `mac-arm64` zip is
  195 MB and a snapshot 180 MB (ADR 0078 § 1).
- **Licence:** CEF's BSD licence, which *"you should include … somewhere in
  your application's distribution"*, plus Chromium's `about:credits` [4][11].

## 2. Security lag, measured from the published dates

Chrome's release times come from Chromium Dash (`fetch_releases`, Mac,
Stable and Extended) [3]; CEF's from the earliest file of each macOS arm64
stable build in `index.json` [1]. A Chrome release counts as reached when a
CEF build of the **same milestone** with the **same or a later** Chromium
version is published. That a later point version carries an earlier one's
fixes is assumed, not checked.

| Period | Chrome release → CEF build carrying it |
|---|---|
| May–Aug 2026, CEF following every milestone (148–151) | mostly **1.2–5.5 days**; up to 8 days; up to 16–21 days on the LTS branch 150 |
| A new milestone's first stable | 13–25 days after Chrome's (early-)stable, while the previous milestone kept its fixes |
| 152.0.7977.83 (3 Sep) | 3.4 days |
| 154.0.8037.58 (22 Sep), .94 (30 Sep), .98 (2 Oct) | 1.4, 1.5, 3.2 days |
| 154.0.8037.99 (6 Oct), .100 (7 Oct) | **never** — CEF 154 ended on 5 October |
| 155.0.8059.41 to .86 (13–20 Oct, several security releases) | **never** — CEF builds no odd milestone |
| Extended 152.0.7977.158, .167, .169 (6, 13, 15 Oct) | Windows and Linux: 2–6 days. **macOS: never** — the last macOS 152 build is .152, 5 October |
| 156.0.8078.47/.48 (20 Oct) | 2.4 days |
| 156.0.8078.73/.74 (27 Oct), .76/.79/.80 (29 Oct, one Critical) | **not yet** at 30 Oct 12:00 UTC: 3 days and counting, 1 day for the Critical |

**What this means for alo on macOS:** between 5 and 22 October 2026 there
was **no** CEF build for macOS that carried Chrome's security releases of 6,
7, 13, 14, 15 and 20 October — **17 days**. Under Chrome's two-week cadence
and CEF's even-milestones-only policy, a gap like this is expected roughly
once a month, unless the extended branch is built for macOS (it was not in
October; why is not known). When CEF is in step, it is usually 1–3 days
behind. ADR 0077 asks for fixes *within days*; CEF meets that most weeks
and misses it, on macOS, for about two weeks a month.

## 3. Architecture: can ADR 0058 still hold?

### 3.1 What a CEF application is

CEF is a library (`Chromium Embedded Framework.framework` on macOS,
`libcef.dll`/`libcef.so` elsewhere). There is no CEF browser to start; the
application that loads the library **is** Chromium's browser process, and
its helper executables are the renderer, GPU and utility processes. On macOS
the layout is fixed: `<App>.app/Contents/Frameworks/` holds the framework and
helper apps `<App> Helper.app`, `(GPU)`, `(Renderer)`, `(Plugin)`,
`(Alerts)` [4][7]. Since M128 every CEF application runs on Chrome's own
bootstrap; windows are either *Chrome style* (Chrome's browser window
machinery) or *Alloy style* (the content layer, and the only style with
off-screen rendering) [5].

### 3.2 Option H — a host executable of alo's own, driven over the pipe

alo ships a small program, **`alo-chromium`** (name to be chosen), that loads
CEF and does nothing but start Chromium and open the windows it is asked
for. `alo-window` starts it exactly as `chrome.rs` starts Chrome today —
`/bin/sh` moving the pipes to descriptors 3 and 4, then
`--remote-debugging-pipe` — and drives it over CDP. alo's own process links
nothing.

- **Does the pipe work in CEF?** Chrome's DevTools pipe handler is part of
  the Chrome bootstrap CEF now uses. CEF's documentation names only
  `--remote-debugging-port` / `remote_debugging_port` [4]. But an issue filed
  12 October 2026 against **CEF 156, macOS arm64** drives `cefclient` with
  `--remote-debugging-pipe` on descriptors 3 and 4, and its maintainer
  reproduced it [6]. So: **works, reported, not documented.** The
  `tauri-runtime-cef` crate also offers a `Pipe` mode that passes
  `--remote-debugging-pipe` through [12].
- **The domains alo uses** (`Target`, `Page`, `Runtime`, `DOM`, `Input`,
  `Fetch`, `Network`, `Accessibility.getFullAXTree`/`getPartialAXTree`,
  `Overlay`, `Emulation`, `Browser.getWindowForTarget`/`setWindowBounds`/
  `close`, and `Page.startScreencast` from ADR 0076) are Chromium's, not
  CEF's. They are **expected** to behave as in Chrome for Chrome-style
  windows. Not one was tried. `Target.createTarget` crashed CEF 154 Chrome
  style until a fix in 154.0.32 (28 September) [13].
- **ADR 0060's hold breaks in Alloy style.** The same issue [6]: with
  `Target.setAutoAttach` and `waitForDebuggerOnStart: true` — exactly what
  ADR 0060 sends to hold every target — a `window.open` popup **stalls** in
  Alloy style, windowed or off-screen; **Chrome style passed**. Open,
  unfixed, on CEF 156. So alo must use Chrome-style windows, and must not
  use off-screen rendering (§ 3.3).
- **Switches change.** CEF takes its profile as `--root-cache-path` /
  `--cache-path` (the issue uses `--cache-path` [6]) rather than
  `--user-data-dir`. `--app=data:,` is Chrome's startup code and is not
  expected to work; the host opens windows itself (a `CefWindow` with a
  Chrome-style `CefBrowserView` and no toolbar). `--headless` is not a CEF
  mode (inferred); `alo --chrome` and the test suite keep using the
  machine's Chrome, as ADR 0077 already allows. Whether ADR 0073's switches
  (`--disable-background-networking`, `--disable-component-update`, the one
  `--disable-features`) are passed through and keep their effect must be
  measured again; CEF's official builds already disable field trials
  (`disable_fieldtrial_testing_config`) [9].
- **How much code.** The `cef` crate's `cefsimple` example is **922 lines**
  of Rust in six files: main 27, helper 28, application 215, handler 326,
  macOS application delegate 243 [8]. alo's host needs less of the handler
  (no menus, no title tracking) and one thing more (a window opened on
  request). Expected: **400–700 lines**, plus bundling of the helpers. CEF's
  own C++ `cefsimple` in the standard distribution is the same program in
  C++ [4].
- **`unsafe`.** On Linux and Windows the `cefsimple` example has **no**
  `unsafe` of its own [8] (the `cef` and `cef-dll-sys` crates have plenty, as
  `winit` and `objc2` do under alo today). On **macOS** CEF requires the
  application's `NSApplication` subclass to implement `CefAppProtocol`; the
  example defines it and its delegate with `objc2::define_class!`, which is
  **31 uses of `unsafe`** in one file [8]. So a Rust host needs an `unsafe`
  boundary in alo's code on macOS — one file, in its own crate, which
  relaxes `unsafe_code = "forbid"` in its own manifest, with an ADR of its
  own (`Cargo.toml`, law 4). Writing the host in C++ from CEF's `cefsimple`
  instead avoids Rust `unsafe` and breaks law 4's *one language*. Neither is
  free; the Rust boundary is the smaller departure.
- **Building the host.** `cef-dll-sys`'s `build.rs` **downloads CEF at build
  time** unless `CEF_PATH` names an unpacked distribution [8]. alo would pin
  the distribution by version and SHA-256 (CEF publishes only SHA-1 [1]),
  unpack it with `export-cef-dir`, and set `CEF_PATH`, so a build never
  fetches. The crates are Apache-2.0 OR MIT, versioned with CEF
  (`156.2.0+156.0.14`, last commit 25 October 2026) [8].

### 3.3 Option E — CEF in alo's own process (off-screen rendering)

alo-window loads CEF itself and takes each page's pixels from CEF's
`OnPaint` (or the `cef` crate's `accelerated_osr` textures) into the
conversation's own drawing [8].

- **Reverses ADR 0058**: Chromium's browser process would be alo's process.
  A Chromium browser-process compromise is then a compromise of the process
  that holds the agent socket, the grants and the record.
- **`unsafe` and FFI at the centre of alo-window**, not in a side program.
- **Collides with `winit`** on macOS: CEF needs the application's own
  `NSApplication` subclass (`CefAppProtocol`); `winit` installs its own.
  (Inferred; not tried.)
- **Off-screen rendering is Alloy style only** [5], where ADR 0060's hold
  stalls [6].

E is set aside. It is listed so that it is not proposed as the "simpler"
CEF.

## 4. Branding and signing

- **The name a person sees.** The menu bar, Dock and app switcher show the
  host application's `CFBundleName`; helpers appear as
  `<App> Helper (Renderer)` and so on [4][7]. With CEF the application is
  ours, so it is **alo** — reached without building Chromium, which is what
  722 needed build machines for. Still to check (not verified): what
  Chrome-style pages such as `chrome://version`, `about:` pages, permission
  and file dialogs say; and the user agent, which will name Chrome's
  version as every Chromium does.
- **macOS signing.** CEF's guide (October 2026) [7]: Developer ID
  Application identity; Hardened Runtime; sign inside out — the framework's
  `Libraries` Mach-O files, the framework, each helper app with its own
  entitlements (renderer and GPU `com.apple.security.cs.allow-jit`, others
  empty), then the outer app with Chromium's unbranded
  `app-entitlements.plist`; no `--deep`; notarize with `notarytool`, staple,
  and check with `spctl`. Same Team ID throughout, so library validation
  passes. This is the same work ADR 0078 already expected; with CEF it is
  done on a published build, not a Chromium alo built.
- **Windows signing.** Authenticode with `signtool` on every shipped
  executable and DLL; with the sandbox, the bootstrap executable and the
  adjacent `chrome_elf.dll` must share one certificate; a signed catalog
  covers `.pak`/`.dat` files [7]. The `cef` crate builds the Windows sandbox
  bootstrap as `bootstrap.exe` [8]. An Authenticode certificate costs money
  and SmartScreen reputation takes time (not priced).
- **Linux.** No platform signing; packages are signed per distribution
  format (not looked at).

## 5. Codecs and DRM

- **Proprietary codecs: off.** CEF's maintainer: *"By default, CEF/Chromium
  builds have proprietary codecs (H.264/AAC) disabled."* [10] CEF's
  `gn_args.py` on the 156 branch (`8078`) does not set
  `proprietary_codecs` or `ffmpeg_branding` [9]; CEF's own automated-build
  guide sets only `is_official_build=true` and toolchain options in
  `GN_DEFINES` [14]. That Spotify's builder uses exactly these is inferred,
  not stated.
- **Widevine: the host code is in, the CDM is not.** CEF **requires**
  `enable_widevine = true` and, on Windows and macOS,
  `enable_cdm_host_verification`, `enable_cdm_storage_id` and `enable_rlz`;
  a user's `GN_DEFINES` cannot override them [9]. `bundle_widevine_cdm` is
  not set, so the proprietary CDM binary is not in the distribution
  (inferred from Chromium's defaults; the archive's file list was not
  checked). The CDM would arrive only through the component updater, which
  ADR 0073's `--disable-component-update` already stops. Against the
  ROADMAP: the proprietary binary is never shipped and never fetched; the
  open-source code that would host it is present. Removing it means building
  CEF ourselves (§ 6, row "Own CEF build").
- **How to confirm on each carried build** (720's tests, unchanged in
  kind): no `WidevineCdm` directory anywhere in the bundle;
  `MediaSource.isTypeSupported('video/mp4; codecs="avc1.42E01E"')` and
  `…mp4a.40.2…` false; `navigator.requestMediaKeySystemAccess('com.widevine.alpha', …)`
  rejected; and ADR 0073's traffic test showing no component-updater
  request. CEF notes Chromium can decode H.264 through the OS on macOS and
  Windows *when* `proprietary_codecs` is on [10]; with it off, the container
  is refused before decoding (ADR 0078, not verified).

## 6. CEF against our own Chromium build (ADR 0078)

| | CEF published build (option H) | Own CEF build (CEF's tools, our args) | Own Chromium build (ADR 0078) |
|---|---|---|---|
| Build machines | **none for Chromium**; ordinary machines (this Mac is enough) to build the host, sign and package for each system | as ADR 0078: 32 GB+, 250 GB+, per system | 32 GB+ RAM, 250 GB+ disk, per system; this Mac cannot (ADR 0078 § 1 E) |
| Security lag | 1–5 days usually; **~2 weeks a month** on macOS with no build (§ 2); out of our hands | ours to set, but CEF's patch set for each milestone comes first (manual, even milestones only) | ours to set; every milestone, odd ones too; hours of build per system per release (not measured) plus people |
| Architecture / ADR 0058 | holds: separate process over the pipe; Chrome-style windows only; `--app` replaced by host-opened windows | same | unchanged from today's `chrome.rs` |
| `unsafe` in alo | **one boundary on macOS** (the host's `NSApplication` subclass), with its own ADR; none on Linux/Windows | same | **none** |
| Languages alo writes | Rust only (if the host is Rust) | Rust plus build scripts | Rust plus build scripts and a rebased patch set |
| Size (macOS arm64, compressed) | 135 MB (minimal) plus the host | similar | about 180–195 MB for comparable builds (ADR 0078) |
| Name | **alo**, from the host's bundle; no Chromium patch | alo | alo, through a carried `BRANDING` patch (722) |
| Codecs | off | off | off, by our args |
| Widevine | CDM absent; host support compiled in, required by CEF | same unless we patch CEF's `BUILD.gn` | **off** (`enable_widevine=false`) |
| Google services | Chromium defaults plus ADR 0073's switches (to be re-measured) | same, plus ungoogled patches if wanted | same, plus ungoogled patches if wanted |
| Trust | Spotify's builder and CEF's maintainer; we re-sign their binary | our machines | our machines |
| Effort to first build | days: host, bundling, signing, `chrome.rs` switches, re-run the CDP tests | weeks plus machines | weeks plus machines |
| Effort per Chrome security release | wait for CEF; bump the pinned version and hash, rebuild the host, sign, notarize, publish — an hour or so of machine time (not measured) | a full build per system | a full build per system |

## 7. What would be shipped per Chrome security release

- **With CEF:** watch `index.json` for a stable build of the current
  milestone; pin its version and SHA-256 in the repository; build the host
  against it (a point release within a branch keeps the CEF API *frozen*
  [2], so usually only the pin changes; a new branch every four weeks may
  change the API and needs the `cef` crate's matching release [8]); sign,
  notarize, publish to 721's updater. Nothing is compiled from Chromium.
  alo cannot ship a fix before CEF does.
- **With our own build:** sync to Chrome's stable tag, re-apply patches,
  build per system for hours, sign, notarize, publish. alo can ship on the
  day Chrome does — if the machines and a person are ready that day.

## The recommendation

**Run the probe in § 8 before deciding.** It can be run on this Mac: the
macOS arm64 minimal distribution is 135 MB and the host is a small Rust
program, not a Chromium build.

If the probe passes, **carry CEF's published builds (option H) for all
three systems**, with a host executable in Rust and one reviewed `unsafe`
boundary on macOS under its own ADR, and keep ADR 0078's own build as the
road for later rather than the first step. Reasons:

- It is the only route that puts a Chromium in alo **on all three systems
  without build machines**, which is what 723 is blocked on.
- It reaches **alo's own name** (722) with no Chromium patch at all.
- It agrees with the ROADMAP on codecs, and on DRM as far as the
  proprietary CDM goes.
- ADR 0058's shape holds: a separate process, over a pipe, behind our tree,
  grants and record.

And what the owner would be accepting, said plainly:

- **A security lag we do not control** — usually a few days, and on macOS
  about two weeks a month under Chrome's new cadence (§ 2). ADR 0077's
  *"within days"* would become *"within days of CEF"*. If that is not
  acceptable, CEF should be rejected now, not after building on it.
- **Widevine's host code inside alo**, without its CDM.
- **One `unsafe` file** in alo's repository (or a C++ host).
- **Dependence on one maintainer's manual branch updates and Spotify's
  builder**, with an open bug (alo must stay on Chrome style) [6].

If the owner keeps ADR 0078, this ADR is closed as **rejected**, and its
§ 2 is the reason.

## 8a. The hands-on probe, done on this Mac (30 October 2026, with the owner's leave)

CEF 156.0.14 (Chromium 156.0.8078.48), the macOS arm64 *client*
distribution from `cef-builds.spotifycdn.com`, SHA-1 checked against the
index; its prebuilt `cefclient` run from a scratch directory, never
installed, and removed afterwards.

| Checked | Result |
|---|---|
| `--remote-debugging-port` and `--remote-debugging-pipe` (fd 3/4, NUL-delimited) | both answered; `Browser.close` over the pipe ended it cleanly |
| `Browser.getVersion` | `Chrome/156.0.8078.48`; the user agent says plain Chrome, nothing of CEF |
| `Target.createTarget`, `attachToTarget` (flatten), `Page.navigate` | worked |
| `Fetch.enable` → `requestPaused` → `continueRequest` | worked |
| `Accessibility.getFullAXTree` | real roles and names (heading, navigation, textbox, button, link, list items) |
| `Page.startScreencast` (PNG) | frames on change, at device pixels |
| `Browser.getWindowForTarget` / `setWindowBounds` | worked |
| `Input.dispatchMouseEvent`, `dispatchKeyEvent`, `insertText` | focused a field and typed into it |
| H.264, AAC, HEVC | not supported (`isTypeSupported` false, `canPlayType` empty); VP9, AV1, Opus, MP3 supported |
| Widevine | **not in the download — but the component updater fetched WidevineCdm on its own within a minute** and talked to Google's hosts; ADR 0073's switches must stop it, and 726 checks they do |
| Name and signing | the sample is `cefclient`, ad-hoc signed; a host of ours carries alo's name and the owner's signature (725) |
| Size | 135 MB to download, about 350 MB unpacked |

Not checked by the probe: ADR 0060's held requests across popups, frames,
workers and sockets; ADR 0073's traffic with alo's switches; a host of our
own; signing and notarizing; Windows and Linux.

## 8b. The host, built (30 October 2026, queue item 725)

alo's host is `crates/alo-cef-host`, built against CEF 156.0.14's macOS
arm64 *minimal* distribution (SHA-1 `7359a9d2…0e13` matching `index.json`
and the published `.sha1`; SHA-256 of our own `ed994b90…9ccf6`), unpacked
outside the repository, with the `cef` crate `156.2.0+156.0.14`. What was
measured on this Mac, against what § 3.2 and § 8 expected:

| Expected | Measured |
|---|---|
| An `NSApplication` subclass, `unsafe` in Rust, one boundary | **Not needed.** CEF deliberately does not make Chrome's application object (`ChromeMainDelegateCef::PreBrowserMain`, branch 8078), and with none made before CEF starts Chromium uses `AppKit`'s own. The pipe, `Target.createTarget`, a page from the network read, `--headless` and `Browser.close` work through it; keys a person types into a shown window worked on the prototype the host was written from (727 asserts it on the host). The cost: `Browser.close` reaches `-[NSApplication terminate:]`, which ends the process at once, so CEF's shutdown never runs. |
| Profile as `--root-cache-path` / `--cache-path` | The host takes alo's `--user-data-dir` as both, and refuses to start without an absolute one. |
| `--headless` not a CEF mode | **It is**: the user agent says `HeadlessChrome` and no window reaches the screen. So `Chrome::start_from(&Chromium::Carried(…), false)` drives the host unchanged. |
| `--app=data:,` not expected to work | Confirmed: no target at all. The host's own window for it is 727. |
| Nothing written outside the profile | Not by default. Three things were, and are stopped: Chromium's temporary directories (`MAC_CHROMIUM_TMPDIR` into the profile), a `Chromium Safe Storage` key in the login keychain whose dialog on a rebuilt host held the network (`--use-mock-keychain`), and an application clone under the system's `X/` directory (`MacAppCodeSignClone` added to alo's one `--disable-features`). macOS's own Metal caches and a Notifications registration remain, and are 729's. |
| A build that never downloads | `cef-dll-sys` downloads CEF when `CEF_PATH` names none, so the host is a workspace of its own, excluded from the repository's, built only with `CEF_PATH` set; the gate does not reach it yet (730). |

## 8. The hands-on probe owed (none of it done)

A temporary probe outside the repository, as ADRs 0060 and 0076 were
measured:

1. Download the `macosarm64` **minimal** 156 build and check its SHA-1
   against `index.json`. List every file: no `WidevineCdm`; record the
   unpacked size.
2. Build the `cef` crate's `cefsimple` with `CEF_PATH` set (no download at
   build time), and bundle it with `bundle-cef-app`.
3. Start it as `chrome.rs` does — `/bin/sh` moving pipes to 3 and 4,
   `--remote-debugging-pipe`, a fresh `--root-cache-path`/`--cache-path`,
   ADR 0073's switches — and confirm `Browser.getVersion` answers.
4. Re-run, against it, what alo sends: `Target.createTarget`,
   `Target.setAutoAttach` with `waitForDebuggerOnStart` and flatten, `Fetch.enable`
   on every target (ADR 0060's held-request probe, including a popup, a
   cross-site frame, workers and a WebSocket), `Accessibility.getFullAXTree`,
   `Input.*`, `Overlay.highlightNode`, `Browser.getWindowForTarget` and
   `setWindowBounds`, `Page.startScreencast` at ADR 0076's sizes.
5. Re-run ADR 0073's traffic measurement: what the CEF build asks of the
   network on its own, with and without the switches.
6. Codecs and DRM: `MediaSource.isTypeSupported` for H.264 and AAC,
   `requestMediaKeySystemAccess('com.widevine.alpha')`, and confirm no
   component-updater request leaves.
7. Branding: menu bar, Dock, Activity Monitor, `chrome://version`, a
   permission prompt and a file dialog, under an app bundle named alo.
8. Signing: sign every part with a Developer ID per [7], notarize, and open
   it through Finder after a real download. (Needs the owner's identity.)
9. The same on Windows and Linux (needs those machines, or CI).

## What is not verified, and is not hidden

- **Nothing was downloaded or run.** No CEF distribution, no `cef` crate
  build, no CDP command against CEF. Every statement about how CEF behaves
  comes from documentation, source files read on the web, or other people's
  issue reports.
- `--remote-debugging-pipe` working in CEF rests on one issue report and its
  maintainer's reproduction [6], not on CEF documentation.
- Whether any CDP domain alo uses behaves differently in CEF Chrome style,
  including `Browser.setWindowBounds` and `Page.startScreencast`.
- Whether ADR 0073's switches pass through to Chromium in a CEF host and
  stop the same requests; which requests CEF itself makes.
- Whether the published distributions contain no `WidevineCdm`, and that
  Spotify's builder sets no codec arguments (inferred from [9][10][14]).
- Why the macOS extended-branch builds of 152 stopped on 5 October while
  Windows and Linux continued; whether that recurs.
- That a later CEF point version carries every fix of an earlier Chrome
  point version on the same milestone (assumed in § 2). The lag figures use
  Chromium Dash's times, which include early-stable releases, and CEF's file
  times on the CDN; both are UTC.
- The host's size in lines (estimated from the `cef` crate's example), the
  unpacked size of a carried CEF, and the time to build, sign and notarize
  one release.
- What Chrome-style UI inside CEF shows as the product name.
- That `winit` and CEF collide over `NSApplication` (§ 3.3) is inferred,
  not tried.
- The price of a Windows code-signing certificate.

## Consequences, if accepted

- ADR 0078 is amended: the first Chromium alo carries is CEF's published
  build; our own build becomes later work, taken up when CEF's lag or
  Widevine host code is no longer acceptable, or when build machines exist.
- 720 pins CEF's version and SHA-256 per system; 722 becomes the host's
  bundle name and icon; 723 shrinks to ordinary signing and packaging
  machines; 724 becomes the host program and its bundling.
- A new ADR for the host's `unsafe` boundary on macOS. — **Not needed**
  (§ 8b): the host has no `unsafe`, and no `NSApplication` of its own.
- `chrome.rs` gains the CEF host's switches beside Chrome's; `alo --chrome`
  and the tests keep the machine's Chrome. — **Not needed** (§ 8b): the
  host reads alo's `--user-data-dir` itself and adds what it needs to its
  own command line, so `chrome.rs` starts it unchanged, headless or (once
  727 opens its window) shown.
- `bundle_chromium.rs` looks for the host and CEF's framework instead of
  `Contents/Frameworks/Chromium.app`.

## Sources (read 30 October 2026)

1. CEF automated builds, `index.json` (all platforms, versions, files,
   sizes, SHA-1, `last_modified`) — https://cef-builds.spotifycdn.com/index.json
   (page: https://cef-builds.spotifycdn.com/index.html)
2. CEF, *Branches And Building* (release-branch policy, current branches 156
   stable and 150 LTS) — https://chromiumembedded.github.io/cef/branches_and_building.html
3. Chromium Dash, Mac Stable and Extended releases with times —
   https://chromiumdash.appspot.com/fetch_releases?channel=Stable&platform=Mac&num=200
   and `…channel=Extended…`; Chrome Releases, *Stable Channel Update for
   Desktop*, 29 Oct 2026 —
   https://chromereleases.googleblog.com/2026/10/stable-channel-update-for-desktop_02107719452.html
4. CEF, *General Usage* (app bundle layout, helpers, licence,
   `remote_debugging_port`) — https://chromiumembedded.github.io/cef/general_usage.html
5. CEF, *Architecture* (Chrome and Alloy styles; Alloy bootstrap removed in
   M128; Chrome style has no off-screen rendering) —
   https://chromiumembedded.github.io/cef/architecture.html
6. CEF issue #4308, *alloy: window.open stalls with browser-level CDP
   auto-attach before popup initialization*, opened 12 Oct 2026, open —
   https://github.com/chromiumembedded/cef/issues/4308
7. CEF, *Code Signing* (Windows and macOS, notarization; "as of October
   2026") — https://chromiumembedded.github.io/cef/code_signing.html
8. tauri-apps/cef-rs (`cef` crate 156.2.0+156.0.14; `examples/cefsimple`;
   `cef-dll-sys` build script; `bundle-cef-app`; `export-cef-dir`) —
   https://github.com/tauri-apps/cef-rs (branch `dev`, last commit
   2026-10-25)
9. CEF `tools/gn_args.py`, branch 8078 —
   https://github.com/chromiumembedded/cef/blob/8078/tools/gn_args.py
10. CEF issue #3559, *Enable proprietary codecs with OS decoding support
    only* (maintainer, 24 Aug 2023) — https://github.com/chromiumembedded/cef/issues/3559
11. CEF `LICENSE.txt` — https://github.com/chromiumembedded/cef/blob/master/LICENSE.txt
12. `tauri-runtime-cef` 3.0.0-alpha.6, `RemoteDebugging` —
    https://docs.rs/tauri-runtime-cef/3.0.0-alpha.6/tauri_runtime_cef/enum.RemoteDebugging.html
13. Ghostex issue #302 (CEF 154 crash on `Target.createTarget` in Chrome
    style; CEF issue #4285, fixed in 154.0.32, 28 Sep 2026) —
    https://github.com/maddada/Ghostex/issues/302
14. CEF, *Automated Build Setup* (`GN_DEFINES` for a builder like Spotify's) —
    https://chromiumembedded.github.io/cef/automated_build_setup.html
