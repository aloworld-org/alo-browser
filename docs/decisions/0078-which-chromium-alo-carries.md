# ADR 0078 — Which Chromium alo carries

**Status:** accepted — the owner chose **our own build** on 30 October 2026, straight away, with no interim build carried (queue items 720, 723, 724)
**Date:** 2026-10-30
**Context:** ADR 0077 (alo carries its own Chromium and never uses the
machine's Chrome); ADR 0058 (Chromium is a renderer we drive over a pipe and
never link); ADR 0073 (Chrome is started with its own traffic switched off);
`ROADMAP.md`, *Not built, and not by accident* (no DRM, no proprietary
codecs we cannot ship freely, no telemetry, no search deal); queue items 720,
721 (the updater) and 722 (Chromium with alo's name)

## The question in one line

ADR 0077 decided that alo carries a Chromium. This ADR is about **which one
and from where**. The build has to be: (1) a stable-channel Chromium that
gets security fixes within days; (2) legal to ship inside alo, with its
licences and notices; (3) signed and notarized as part of `alo browser.app`
with the owner's Apple identity; (4) free of Google services we do not want,
and compatible with ADR 0073's switches; (5) driveable over
`--remote-debugging-pipe` with `--app` windows, as `chrome.rs` drives Chrome
today; and (6) ideally able to carry alo's name later (722).

**No build anyone publishes meets all six.** The options below differ in
which criteria they miss, and in what it costs to fill the gap.

## Why this is a decision rather than a download

Whatever alo carries is the largest attack surface in the product, and
whoever builds it decides how quickly a fix reaches the person using it.
Chrome's stable release of **29 October 2026** (156.0.8078.79/.80) fixed 21
security bugs, one of them rated Critical (CVE-2026-76024, MediaStream)
[1]. A build that is one such release behind leaves a person exposed to
bugs that are already public. Picking a source picks that delay.

## 1. What was looked at (all on 30 October 2026)

### A. Chromium snapshot builds (`chromium-browser-snapshots`)

- **What they are:** builds of the main branch, published as soon as each
  build finishes (continuous builds). They are not stable-channel builds.
  The newest `Mac_Arm` build is revision **1731290**, uploaded 2026-10-30
  10:32 UTC [2]. That revision is newer than Canary (159.0.8132.0 is
  r1730971) [3]. Stable 156 branched at r1706539 [3], and **no snapshot
  exists at that revision** [2]. Even one that did would hold none of the
  fixes merged onto the branch since then.
- **Google's own words:** builds are made *"on a best-effort basis"*, *"do
  not auto-update"*, and old ones *"do not have up-to-date security fixes"*
  [4].
- **Security:** a fix usually lands on the main branch before it is merged
  to stable, so a fresh snapshot often has it first. It also has a month or
  more of unreleased code that has not been through Beta. That is Canary
  quality or worse. It is not a stable-channel browser.
- **Name, codecs, DRM:** branded *Chromium* (`org.chromium.Chromium`) [5].
  `proprietary_codecs` is off unless the build is Chrome-branded [6], and
  Widevine is not enabled [7]. So the build agrees with the ROADMAP as it
  is.
- **Licence:** Chromium's BSD-3-Clause licence plus its third-party notices
  (`about:credits`). This build can be shipped.
- **Signing:** not verified (nothing was downloaded). We expect ad-hoc or
  development signing, which we would replace with our own.
- **Size:** `chrome-mac.zip` is 180 MB [2].

### B. Chrome for Testing (GoogleChromeLabs)

- **What it is:** *"a Chrome flavor that specifically targets web app
  testing and automation use cases … without auto-update, integrated into
  the Chrome release process, made available for every Chrome release"*
  [8]. It ships for `mac-arm64`, `mac-x64`, `linux64`, `linux-arm64` (since
  153) and Windows [9]. Stable today is **156.0.8078.80** (r1706539) [3].
  Its `mac-arm64` zip (195 MB) was uploaded on 28 October at 23:12 GMT [10],
  **before** Google's blog post announcing that release. It is the exact
  stable release, with no delay.
- **Google's warning:** *"created purely for browser automation and testing
  purposes. It should only be used to consume trustworthy content."* [8]
- **Name:** it is *Google Chrome for Testing.app* [9], with the Chrome logo
  marked "Test" [8]. That is Google's trademark, so alo cannot rename it,
  and 722 cannot be reached from it.
- **Codecs and DRM: it has both.** Its build arguments set
  `proprietary_codecs = true`, `ffmpeg_branding = "Chrome"`,
  `enable_widevine = true` and **`bundle_widevine_cdm = true`** [11]. It
  ships the Widevine CDM and the H.264/AAC decoders. This breaks the
  ROADMAP's list the moment alo is given to anyone.
- **Licence:** the repository is Apache-2.0 [9]. We found no statement
  saying whether the binaries may be redistributed, or under what terms.
  Not verified. It is Google's binary carrying Google's name, so we treat it
  as not redistributable.
- **What it gives us:** the same code as the Chrome that ADR 0073 measured.
  The ADR 0073 switches and `tests/what_chrome_asks_for_of_its_own.rs`
  should behave as they do today (not re-measured). Versions are published
  as JSON (`last-known-good-versions.json`, with hashes per download) [3],
  which `alo-bundle` and a first updater can read.

### C. Maintained distributions

| Distribution | Latest mac arm64 | Stable at the time | Who signs | Codecs / DRM | Licence |
|---|---|---|---|---|---|
| ungoogled-chromium-macos [12] | 156.0.8078.**47**, 28 Oct | .80 (29 Oct) | Developer ID *Qian Qian (B9A88FL5XJ)*, notarized; maintainer pays from sponsorships, promised to Oct 2027 | `proprietary_codecs=true`, `ffmpeg_branding="Chrome"`, `enable_widevine=true` [13] | BSD-3-Clause [14] |
| Helium (imputnet, built on ungoogled-chromium) [15] | 156.0.8078.**73**, 28 Oct | .80 | GitHub Actions; notarization not stated on the page | same flags [16] | GPL-3.0 [14] |
| Thorium macOS [17] | 156.0.8078.**28** notice (11 Oct), marked beta, in a fork's repository | — | not stated | — | — |

- **Pace:** ungoogled-chromium-macos's release list has no 151 and no 153
  build. At times it skips whole stable versions, and today it is two
  security releases behind. Helium follows each point release within a day
  or two. Thorium does not follow stable releases on macOS.
- **What they remove:** ungoogled-chromium removes Google's services in the
  source. It replaces Google domains with dead ones, removes prebuilt
  binaries, sets empty API keys, and sets `safe_browsing_mode=0` [13][18].
  That is close to what ADR 0073 achieves with switches, and goes further.
  It is not verified whether ADR 0073's switches, `--app` or the pipe
  behave differently in it. They are Chromium switches and should work.
- **Against the ROADMAP:** every published distribution build ships the
  proprietary codecs, so none can be carried as it is. Their **patches**
  could be used. ungoogled-chromium's patches are BSD. Helium's are GPL-3.0,
  which would put source obligations on whatever includes them.
- **Trust:** carrying one of these means trusting a small team's CI and
  signing key with the most exposed binary in alo. If we re-sign it, our
  signature covers their build. Neither build is reproducible bit for bit
  (not claimed by either project).

### D. Building Chromium ourselves

- **Requirements:** Chromium's macOS guide asks for an Intel or Arm Mac,
  Xcode with the SDK from `mac_sdk.gni`, and an APFS volume. The checkout
  takes *"30 minutes on even a fast connection"* [19], and *"Arm Macs are
  fully supported"* [20]. The guide gives no figures for memory, disk or
  time. Helium's development guide (a secondary source) gives **16 GB RAM
  minimum, 32 GB+ recommended** (linking fails with less), **100 GB+ disk**
  and 2–8 hours [21]. Official builds use ThinLTO, which multiplies link
  time and memory [22].
- **Branding:** `is_chrome_branded` is false by default. The name, company
  and bundle identifier come from `chrome/app/theme/chromium/BRANDING`
  (`PRODUCT_FULLNAME=Chromium`, `MAC_BUNDLE_ID=org.chromium.Chromium`,
  `MAC_TEAM_ID=`) [5]. 722 means replacing that file and the icons, through
  a patch carried from release to release, without Google's branding.
- **Codecs and DRM by construction:** do not set `proprietary_codecs` or
  `ffmpeg_branding="Chrome"`, and set `enable_widevine=false` [6][7]. That
  is the ROADMAP as a build argument.
- **Each security release** means syncing to the new stable tag (e.g.
  `156.0.8078.80`), applying our patches again, doing a full official
  build, then signing, notarizing and publishing. ungoogled-chromium-macos
  credits hosted macOS runners with cutting its builds *"from days to
  hours"* [12]. Brave and Vivaldi keep pace by having people and build farms
  waiting for every Chrome release. Brave usually ships the same Chromium
  point release the same day or the next day [23]. Vivaldi follows Extended
  Stable on odd-numbered versions [23].

### E. This Mac (read on 30 October 2026)

`sysctl -n hw.memsize hw.ncpu` reports **8 GB** (8 589 934 592 bytes) and
**8 cores** (Apple M3, 4 performance + 4 efficiency). `df -h /` reports
**103 GiB free** of 460 GiB.

**Building Chromium here is not feasible.** The memory is half the stated
minimum, and the free disk is at the minimum with no room for a second
checkout, a ThinLTO cache or a fetch of the next release. Even if the link
finished, an 8-core laptop would be busy for most of a day for each
security release (not measured). Building ourselves needs another machine.

## 2. The options compared

| | Stable + fast security fixes | Shippable to others | Signed as alo | No unwanted Google services (0073) | Pipe + `--app` | alo's name (722) | Codecs/DRM out (ROADMAP) |
|---|---|---|---|---|---|---|---|
| A. Snapshot | **no** (main-branch builds) | yes (BSD) | re-sign: yes | as today, plus no API keys | yes (not re-measured) | no | **yes** |
| B. Chrome for Testing | **yes, same day** | **no** (Google's name, codecs, Widevine) | re-sign: possible, not verified | exactly as measured in 0073 | yes (made for automation) | no | **no** |
| C1. ungoogled-chromium-macos binary | lags; skips versions | codecs prevent it | re-sign over theirs | better than 0073 | expected, not verified | no | **no** |
| C2. Helium binary | about 1 day behind | codecs, GPL, another product's name | re-sign over theirs | better than 0073 | expected, not verified | no | **no** |
| D. Our own build (with ungoogled's BSD patches if wanted) | as fast as our machines and people | **yes** | **yes**, our identity throughout | by construction | yes | **yes** | **yes**, by build arguments |

### What each costs

- **A.** Little money or time. The risk is quality: a browser built from
  code Google has not released, whose DevTools protocol can change between
  pins. Every pin is a guess at a good revision.
- **B.** No money and almost no time. `alo-bundle` reads the JSON, pins
  version and hash, and re-signs. The risk is legal and in principle: it can
  never be given to anyone, it carries Google's name, and it contains the
  CDM and codecs the ROADMAP refuses. Removing the bundled
  `WidevineCdm` from the bundle is possible but not tested. The codecs are
  compiled in and cannot be removed.
- **C.** No money. The risk is trust and lag: someone else's CI and signing
  key, codecs we refuse, and a release we cannot speed up when a Critical
  bug lands.
- **D.** Money and people. Build machines need at least 32 GB RAM, 250 GB+
  disk and many cores, which means a hosted macOS arm64 runner service
  (as ungoogled-chromium-macos uses) or a Mac of our own. Prices were not
  checked. Hosted Macs under Apple's licence are commonly rented for at
  least 24 hours at a time (not verified). The first build will take days
  of work to get right. After that, every Chrome stable or security release
  costs a rebuild of several hours on a large machine (not measured) plus
  signing, notarizing and publishing. If no person is watching for those
  releases, this option is worse than A, not better.

## The owner's choice

**alo carries a Chromium built by alo, from the stable tag, and nothing
else — not Chrome for Testing, not a snapshot, not anybody's distribution,
not even for a while on the owner's own machine.** It is built with no
proprietary codecs and no Widevine (`ROADMAP.md`, *Not built, and not by
accident*), with empty Google API keys and with alo's name (722), signed and
notarized with the owner's Apple identity, and it is what 721's updater
fetches.

What follows from it:

- **A build machine is needed first** (723). This Mac has 8 GB of memory and
  about 103 GB free; a Chromium build wants at least 16 GB and recommends
  32 GB or more, and 100 GB or more of disk. So the build runs elsewhere — a
  rented or owned Apple-silicon Mac with at least 32 GB and 250 GB free. That
  is the owner's to arrange; nothing else in this ADR waits on anything but
  it.
- **The build's recipe is ours and in this repository** (724): the
  Chromium version it follows, `args.gn` (no codecs, no Widevine, official
  and not Chrome-branded, empty API keys), alo's branding files, and a
  script that fetches, builds, packages and records what it built — written
  and reviewed here before any machine runs it.
- **Until the first build exists, the application carries no Chromium** and
  draws with this engine, saying so (ADR 0077, item 719).
- The recommendation below is kept as the record of what was weighed.

## 3. What was recommended (kept for the record)

**(a) Now, on the owner's machine only: Chrome for Testing, pinned.**
`alo-bundle` takes the stable Chrome for Testing for `mac-arm64` named in
`last-known-good-versions-with-downloads.json`, checks it against a
committed SHA-256, puts it inside `alo browser.app` with Chromium's notices,
and signs it with the application. The application is told it is a stopgap,
and says *Chrome for Testing* when asked which Chromium it uses (719).
Reasons:

- It is the only published build that is **the stable release, on the day
  it ships**. For a browser the owner actually browses with, that matters
  more than anything else in this list.
- It is the code ADR 0073 measured, so the traffic test keeps its meaning
  with no new measurements.
- ADR 0077 already says *nothing is given to anyone outside the owner's
  machines before 721*. The codecs, the CDM and Google's name therefore
  never leave the owner's machine. They must not, and § 4 makes that a
  check.
- The bundled `WidevineCdm` is removed when `alo-bundle` assembles the
  application. ADR 0073's `--disable-component-update` already stops a CDM
  from being downloaded. Whether the build still starts and plays nothing
  encrypted is a test for 720 to write.

If the owner does not want codecs on his own machine even for a while, the
alternative is **A, a snapshot pinned to a revision a few days old**. It
agrees with the ROADMAP and is BSD-licensed, but it is a main-branch build
and must be re-pinned weekly to pick up fixes.

**(b) Before anyone else gets alo: our own build (D), and it is 720, 721
and 722 together.**

- Build from the stable tag with `is_official_build=true`,
  `enable_widevine=false`, no `proprietary_codecs`, our `BRANDING` and icons
  (722), empty Google API keys, and `enable_updater=false`. Start from
  ungoogled-chromium's BSD patch set where it removes a request ADR 0073
  could only name (`ListAccounts`, the on-demand models manifest). Each
  patch we keep is a patch we rebase on every release.
- Build on hosted or owned macOS arm64 machines (not this Mac, § 1 E).
  Sign with the owner's Developer ID, using Chromium's own signing scripts
  and entitlements for the helper apps. Notarize. Publish the hash.
- 721's updater fetches **our** signed builds. Its promise to the person is
  *"within N days of a Chrome stable security release"*. N is chosen by the
  owner and kept, and a Critical fix may move it.
- Linux later is the same build on a Linux machine. The Linux build is the
  cheaper and better-documented one of the two.

Until (b) exists, alo stays on the owner's machines, as ADR 0077 already
requires.

## 4. Whichever is chosen, 720 builds

- The version and SHA-256 are pinned in the repository, `alo-bundle` checks
  them, and a mismatch fails the bundle.
- The licences (`about:credits` / `LICENSE` with third-party notices) ship
  inside the application.
- A test that the carried Chromium has no Widevine CDM in its bundle.
- A test for codecs, matching the choice: with D or A,
  `MediaSource.isTypeSupported('video/mp4; codecs="avc1.42E01E"')` is false.
  With B the test is turned the other way and marks the build *owner-only*,
  and `alo-bundle` refuses to make a release from it.
- ADR 0073's traffic test is run against the carried Chromium, not the
  machine's Chrome.

## What was not verified

- Whether a re-signed Chrome for Testing or snapshot passes notarization
  with the owner's identity, and which entitlements its helpers need.
- Whether Chrome for Testing's binaries may be redistributed, and under what
  terms (no statement found).
- How snapshot builds are signed (nothing downloaded).
- How ungoogled-chromium or Helium behave with ADR 0073's switches,
  `--app`, or `--remote-debugging-pipe`.
- Whether Chrome for Testing runs correctly with `WidevineCdm` removed.
- Build time and memory for an official arm64 build on any particular
  machine. The 16/32 GB, 100 GB and 2–8 hour figures come from a secondary
  source [21].
- The price of hosted macOS build machines.
- Whether macOS's own decoders can play H.264 in a Chromium built without
  `proprietary_codecs`. They are not expected to, because the container and
  codec are not recognised before decoding.

## Sources (read 30 October 2026)

1. Chrome Releases, *Stable Channel Update for Desktop*, 29 Oct 2026 —
   https://chromereleases.googleblog.com/2026/10/stable-channel-update-for-desktop_02107719452.html
2. Snapshot bucket listing, `Mac_Arm/LAST_CHANGE` and `Mac_Arm/1731290/` —
   https://www.googleapis.com/storage/v1/b/chromium-browser-snapshots/o?prefix=Mac_Arm/
3. Chrome for Testing, `last-known-good-versions.json` (timestamp
   2026-10-30T07:16Z) —
   https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions.json
4. *Download Chromium* — https://www.chromium.org/getting-involved/download-chromium/
5. `chrome/app/theme/chromium/BRANDING` —
   https://chromium.googlesource.com/chromium/src/+/main/chrome/app/theme/chromium/BRANDING
6. `build/config/features.gni` (`proprietary_codecs`) —
   https://chromium.googlesource.com/chromium/src/+/main/build/config/features.gni
7. `third_party/widevine/cdm/widevine.gni` —
   https://chromium.googlesource.com/chromium/src/+/main/third_party/widevine/cdm/widevine.gni
8. *Chrome for Testing: reliable downloads for browser automation* (last
   updated 2023-06-12) — https://developer.chrome.com/blog/chrome-for-testing
9. https://github.com/GoogleChromeLabs/chrome-for-testing
10. `chrome-for-testing-public/156.0.8078.80/mac-arm64/chrome-mac-arm64.zip`
    headers — https://storage.googleapis.com/chrome-for-testing-public/156.0.8078.80/mac-arm64/chrome-mac-arm64.zip
11. `chrome/browser/chrome_for_testing/args.gni` —
    https://chromium.googlesource.com/chromium/src/+/main/chrome/browser/chrome_for_testing/args.gni
12. https://github.com/ungoogled-software/ungoogled-chromium-macos/releases
13. `flags.gn` and `flags.macos.gn` —
    https://github.com/ungoogled-software/ungoogled-chromium/blob/master/flags.gn,
    https://github.com/ungoogled-software/ungoogled-chromium-macos/blob/master/flags.macos.gn
14. GitHub repository licences (API `spdx_id`) for
    ungoogled-software/ungoogled-chromium(-macos) and imputnet/helium(-macos)
15. https://github.com/imputnet/helium-macos/releases
16. https://github.com/imputnet/helium-macos/blob/main/flags.macos.gn
17. https://github.com/Alex313031/Thorium-MacOS/releases
18. https://github.com/ungoogled-software/ungoogled-chromium/blob/master/docs/design.md
19. https://chromium.googlesource.com/chromium/src/+/main/docs/mac_build_instructions.md
20. https://chromium.googlesource.com/chromium/src/+/main/docs/mac_arm64.md
21. Helium, *System Requirements* (secondary) —
    https://www.mintlify.com/imputnet/helium/development/requirements
22. Chromium code review on LTO link time —
    https://codereview.chromium.org/2333843002
23. PC-WELT on Brave and Vivaldi following Chrome security releases
    (2022–2023) —
    https://www.pcwelt.de/article/2019247/sicherheits-updates-fur-chromium-basierte-browser-brave-und-vivaldi-sind-wieder-aktuell.html
