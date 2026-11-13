# ADR 0116 — alo updates itself from a manifest signed with a key of alo's own

**Status:** proposed — queue item 721, 13 November 2026. Nothing is built.
Five questions wait for the owner (*Asked of the owner*).
**Date:** 2026-11-13
**Context:** ADR 0077 (alo carries its own Chromium; *"Nothing is given to
anyone outside the owner's machines before 721"*); ADR 0078 (alo's own
Chromium build, before alo is offered widely; 723, 724); ADR 0079 (alo
carries CEF's Chromium now; § 2, CEF's security lag); ADR 0080 (the host's
one `unsafe` boundary); ADR 0010 (a rented crate's `unsafe` is the crate's);
ADR 0012 (every request says what caused it); ADR 0025 § 3 (`rustix`, rented
for the file system); ADR 0055 (the agent socket); ADR 0060 and 0073 (every
request held and recorded; what Chromium still sends of its own); ADR 0112
(alo OS: requirement 9, *nothing leaves silently*; bar item 9, *no network
traffic while idle*; *Answered* 5, artifacts with their SHA-256, no image
before signing); queue items 720 (the application, sealed ad hoc), 728,
735 (Developer ID and notarisation — *"one of the last things"*, the owner,
13 November 2026), 483 (Flatpak), 696, 968; the owner's rule of 9 November
2026 (this repository stays provider-neutral; a private shipping repository
for keys, release builds and signing comes later, not now).

## The question in one line

When CEF publishes a Chromium with Chrome's security fixes, how does a new
`alo browser.app` reach a person within days — checked by alo, signed by
alo, and with the person told — without trusting the server it came from,
sending anything about the person, or restarting an agent's work?

## 1. What is updated, and how fast

**One thing is updated: the whole application.** alo's programs, the CEF
host and its helpers, CEF's framework and the licences are one bundle with
one version. They are built, sealed and tested together (720), so they are
shipped together. A Chromium updated alone would run against a host built
for another CEF, and CEF's API changes with every build. No delta updates
at first: correct before fast. The application is 361,036 KiB unpacked
(720's measurement, iteration 458), 333,312 KiB of it CEF's framework. The
packed size is not measured; CEF's own `minimal` macOS arm64 archive is
134,874,408 bytes, so expect a download of about 150 MB.

**A release follows each CEF build that carries a new Chromium version.**
A CEF build with the same Chromium (156.0.26, .27 and .28 all carry
156.0.8078.88) goes out with alo's next release of its own, unless CEF's
notes name a security fix.

**What CEF's builds give, measured.** Read from
`cef-builds.spotifycdn.com/index.json` at 18:26 UTC on 13 November 2026 —
the earliest file of each `stable` build (verified):

| CEF build | Chromium | macOS arm64 | Linux x64 |
|---|---|---|---|
| 154.0.34 | 154.0.8037.98 | 5 Oct 06:44 | 5 Oct 04:06 |
| 152.0.13–.15 (extended) | 152.0.7977.158–.169 | **none** | 8, 17, 21 Oct |
| 156.0.7 | 156.0.8078.17 | 22 Oct 04:14 | 22 Oct 03:05 |
| 156.0.8 / .14 | 156.0.8078.48 | 23 Oct 04:33 | 23 Oct 03:19 |
| 156.0.15 | 156.0.8078.80 | 31 Oct 05:35 | 31 Oct 04:22 |
| 156.0.26–.28 | 156.0.8078.88 | 7 Nov 05:24 | 7 Nov 04:05 |
| 156.0.30 | 156.0.8078.96 | 13 Nov 05:23 | 13 Nov 04:26 |

- **A new Chromium version every 6 to 8 days** since 23 October: .48, .80,
  .88, .96. So alo would ship about once a week.
- **Chrome 156.0.8078.80** (29 October, 21 security fixes, one Critical;
  ADR 0078 source 1) was in Chrome for Testing at 28 October 23:12 GMT (ADR
  0078). CEF carried it at 31 October 05:35 UTC: **2 days 6 hours** later.
  The .73 and .76 releases of 27–29 October got no build of their own; .80
  carried them, about 4 days after .73.
- **On macOS, 17 days with no build** (5–22 October), as ADR 0079 § 2
  found: CEF builds even milestones only, and built the extended 152 branch
  for Linux and Windows but not for macOS. Linux x64 had builds on 8, 17
  and 21 October.
- **Chrome 157 is not in CEF and will not be.** This repository's own notes
  have the owner's Chrome at 157 by 6 November. CEF's newest stable is still
  156 — an extended branch (ADR 0079 § 1: every fourth milestone from 152)
  — and its newest beta is 158.0.3 (13 November). Whether 156.0.8078.88 and
  .96 are Chrome's Extended Stable releases with 157's security fixes was
  **not checked**: Chrome's own release dates were not read for this ADR,
  only CEF's index.

**The promise, proposed:** *alo ships within 3 days of a CEF build that
carries a new Chromium.* That is about 4–5 days after Chrome when CEF is in
step, and longer when it is not, as in October. When CEF has no build for a
Chrome security release, alo cannot ship one, and the release process says
so in the signed manifest (§ 3, `behind`), so alo can tell the person.
Only alo's own Chromium build (ADR 0078, 723, 724) removes CEF's lag. N is
the owner's to choose (question 5).

## 2. The update's own signature

Apple's Developer ID (735) says the application came from the owner's
Apple account. It does not say the download is the newest, is not an older
one replayed, or came from alo's release. And today the application is
sealed ad hoc (720), so it says nothing at all. So **the update carries a
signature of alo's own, independent of Apple's**, and is checked with it
before anything is unpacked.

- **One Ed25519 key pair of alo's.** The public key is built into the
  application. The secret key is never in this repository, nor in any file
  this repository writes. It is made and kept by the owner, and later used
  only by the private shipping repository's release (not created now).
- **Two public keys are built in:** the one in use and a spare, made at the
  same time and kept offline. Either one's signature is accepted. If the key
  in use is lost or leaked, a release signed with the spare carries a new
  pair of public keys, and the leaked one is dropped. A release can change
  the keys only by shipping a new application; the manifest never names a
  key.
- **The signature format is minisign's**, prehashed (BLAKE2b-512 of the
  file, then Ed25519). The secret key is a password-protected file made by
  an off-the-shelf tool — `rsign2` 0.6.7 (Rust, MIT) or `minisign` itself.
  So alo writes no signing code, and the format is one other people already
  use for their own updates. Its *trusted comment* is signed too.
- **What is signed is the manifest** (§ 3), not the package. The manifest
  names the package's SHA-256, which alo already computes (`sha2` 0.10.9, in
  the workspace for `alo-net`).

### What was weighed (crates.io, read 13 November 2026)

| | Version | Licence | Dependencies | Verdict |
|---|---|---|---|---|
| **`minisign-verify`** | 0.3.0 (25 Sep 2026) | MIT | **none** | **chosen** |
| `ed25519-dalek` | 3.0.0 (6 Jul 2026, MSRV 1.85) | BSD-3-Clause | `curve25519-dalek`, `ed25519`, `sha2`, `subtle` | the alternative |
| `tough` (TUF) | 0.24.0 (10 Jul 2026) | MIT OR Apache-2.0 | 28, with `tokio`, `aws-lc-rs`, `rustls`, `serde_json` | too much |
| Sparkle | — | MIT | Objective-C | refused by law 4 |

- **`minisign-verify`** verifies minisign signatures and nothing else, in
  686 lines of its own plus 2,195 of its own Ed25519, Curve25519, SHA-512
  and BLAKE2b, with no `unsafe` in the source (read in the 0.3.0 crate;
  verified). It has a streaming verifier. It is what Tauri's Rust updater
  uses (`tauri-plugin-updater` 2.14.1 depends on `minisign-verify ^0.3`;
  verified on crates.io). Its cost: its curve code is its own, not the
  widely audited `dalek` one. For verifying a public signature over public
  bytes, side channels do not matter, but correctness does. So alo adds
  test vectors of its own (§ 6).
- **`ed25519-dalek`** is the audited, most used Rust Ed25519 (273 million
  downloads). It would verify a raw Ed25519 signature with
  `verify_strict`. But alo would then need a signing program of its own and
  a key-file format of its own, and the owner a tool nobody else uses. It
  is the right answer if the owner prefers the audited curve to the
  off-the-shelf signer (question 1).
- **TUF (`tough`)** solves more than alo has: many roles, thresholds,
  delegations and mirrors. It brings an async runtime and `aws-lc-rs`, a C
  and assembly crypto library. One owner and one key do not need it. alo
  takes its ideas instead: a signed expiry (no freeze attack), a version
  that only rises (no rollback), and a spare key. Revisit if releases ever
  have more than one signer.
- **Sparkle** is Objective-C, so law 4 refuses it. Its appcast (RSS with
  `sparkle:edSignature`) is not reused either: it would put an XML reader on
  the update's path, and the fields alo needs are a dozen lines.

All four licences sit with MPL-2.0 (ADR 0009).

## 3. Where updates come from

**The address is not in this repository.** A build is given its update
address and its two public keys when it is made, by `alo-bundle`'s
configuration, as the shipped app's service keys are given (the owner's
rule of 9 November). The private repository will set them. Any other
person building alo sets their own, or none.

**A build with no address or no keys does not update itself**, and
*Settings* says so: *This copy of alo does not update itself.* Every build
from this repository today is such a build. The `0.0.0` version (the
workspace's) never updates and is never offered.

**The host is anything that serves a file over HTTPS** — GitHub Releases
of the private repository, or a plain static host. alo trusts the
signature, not the host or its certificate. Two files at a fixed address
per platform:

```text
<address>/macos-arm64/manifest
<address>/macos-arm64/manifest.minisig
```

**The manifest is plain lines**, `name value`, read by one small file of
alo's own (no JSON, no TOML):

```text
alo-update 1
platform macos-arm64
version 0.4.2
serial 118
published 2026-11-14T09:00:00Z
expires 2026-11-28T09:00:00Z
minimum 0.3.0
chromium 156.0.8078.96
package https://…/alo-browser-0.4.2-macos-arm64.zip
size 158204416
sha256 3f6c…
security yes
behind no
notes Chromium 156.0.8078.96, with Chrome's security fixes of 12 November.
```

**What alo checks, in order, and refuses on the first failure:**

1. The signature, by one of the two built-in keys.
2. `alo-update 1` and `platform` are its own.
3. `serial` is higher than the highest it has seen. That number is kept
   in alo's own state, outside any profile. A replayed older manifest is
   refused.
4. `expires` has not passed. A manifest kept back by a hostile network
   stops being believed, and after its expiry alo says it could not check
   (§ 4).
5. `version` is higher than the installed one. **An older or equal version
   is never installed**, whatever is signed.
6. The installed version is at least `minimum`. If not, alo says to
   download it again from the place the person got it.
7. There is room on the disk for twice `size`. alo asks with `rustix`'s
   `statvfs` (ADR 0025 § 3).

**Then the download.** It goes through `alo-net` (the browser process's
own network, with `rustls`), never through Chromium. It is written to
`<state>/updates/<version>.partial`, and resumed with a `Range` request if
cut. It must be exactly `size` bytes, and its SHA-256 must be `sha256`.
Only then is it renamed `<version>.zip`. Nothing of the installed
application has been touched.

**Then the unpacking, beside the installed application.** The archive is
unpacked with `/usr/bin/ditto -x -k`. That tool is macOS's own, and keeps a
bundle's symbolic links and its signature. It goes into a hidden folder in
the same folder as the installed `alo browser.app`, so on the same volume.
The unpacked bundle must answer `codesign --verify --strict`. Once 735 is
done, its Team ID must be the running application's too. Its
`CFBundleShortVersionString` must be the manifest's `version`.

**Then the swap, atomic.** The two bundles are exchanged in one call:
`rustix::fs::renameat_with(…, RenameFlags::EXCHANGE)`. That is
`renameatx_np` with `RENAME_SWAP` on macOS, and `renameat2` with
`RENAME_EXCHANGE` on Linux. `rustix` 1.1.5 is already in the workspace
with `fs`, and its function is safe (read in its source; verified). No
`unsafe` of alo's, and no ADR 0080 change. Either the old application is
at the path or the new one is, never neither. The old one, now in the
hidden folder, is removed at the next launch.

**alo never asks for more rights than the person has.** If the folder that
holds the application cannot be written by the person (a standard account
and `/Applications`), or macOS is running it from a translocated copy, alo
does not update. It says why and what to do: move alo to a folder you can
write, or ask the computer's administrator. There is no privileged helper
and no password prompt.

### Linux, alo OS and Windows

- **On alo OS, the updater is off.** alo OS installs software from Flatpak
  remotes: its `alo-software/shipped.toml` pins Firefox from Flathub for the
  `web-browser` role (ADR 0112, *Context*). alo browser's app id is the
  Flatpak id `org.aloworld.Browser` (ADR 0112, *Answered*). Flatpak updates
  an application, checked with the remote's own signatures, and a program
  inside a Flatpak cannot replace itself. So alo inside a Flatpak (the file
  `/.flatpak-info` exists) never checks for updates. *Settings* says
  *Updates come from your software source.* That also keeps ADR 0112's bar
  item 9, no traffic while idle. Who publishes the Flatpak, and from which
  remote, is the owner's and the coordinator's (question 4).
- **Linux outside Flatpak** (a tarball, as ADR 0112 *Answered* 5 allows at
  first) is off too, until it is asked for. The same manifest and the same
  `RENAME_EXCHANGE` swap would serve it.
- **Windows** has no build yet (728). A running `.exe` cannot be renamed
  over, so its swap is its own design. The manifest and signature are the
  same.

## 4. The person told

- ***Settings › Updates*** says, always: alo's version and its Chromium's,
  when alo last checked and what it found, and the switch *Check for
  updates automatically* (§ 5). It also has a button, *Check now*.
- **When an update is ready** — downloaded, checked and unpacked — a line
  appears in alo's own window, never inside a page: *alo 0.4.2 is ready. It
  installs when you quit alo.* With `security yes` it says *a security
  update*, and names the Chromium. It has a button, *Quit and update*.
- **alo never restarts by itself.** It does not restart at night, when
  idle, or when an update is critical. While an agent is working in any
  chat, *Quit and update* waits and says why: *An agent is working. alo
  will update when you quit.* A person's quit is the only start.
- **It installs at quit.** CEF starts its helpers from the bundle on
  demand, so a bundle swapped under a running alo would start a new helper
  against an old framework. So the swap happens after every card's host has
  ended, and then alo exits. If alo did not quit cleanly, the next launch
  swaps first, before any Chromium starts, and then starts the new alo in
  its place.
- **When alo cannot keep up, it says so.** It cannot check (no network, or
  an expired or refused manifest) for more than 3 days. Or the manifest says
  `behind yes` (CEF has no build for Chrome's latest security release). In
  both cases *Settings* and the window's line say *alo's Chromium may be
  missing security fixes*, and why. A refused manifest or package is
  recorded with the reason, as any refusal is.
- **The agent socket has no part in it.** No ask starts, delays, refuses or
  reads an update. An agent cannot press *Quit and update*: the socket
  reaches the page shown in a card, not alo's own window's controls
  (`docs/contracts/agent-socket.md`), and no ask is added that would.

## 5. Privacy: a request the person did not cause

ADR 0012 says every request has a cause, and ADR 0112 requirement 9 says
nothing leaves *"unless the person turns [it] on"*. An update check is a
request the person did not make at that moment. The proposal, for the
owner:

- **A choice in *Settings*, said on the first screen.** The first time alo
  starts, the first screen says once: *alo checks for security updates once
  a day. It sends nothing about you.* It links to *Settings › Updates*.
  **Default: on** (question 3). A browser that carries Chromium and does not
  update is the larger harm, and the person is told before the first check.
- **What the check is.** Two `GET`s, the manifest and its signature, at
  launch and then every 24 hours while alo runs. Then one `GET` for a
  package. No cookies, no query string, no identifier, no install ID, no
  version in the address. The `User-Agent` is `alo-update`, no version.
  No report is sent back of success, failure or anything else. The host
  learns an address and a time, as any server does.
- **It is recorded like any request.** It goes through `alo-net` in the
  browser process, its cause *update check (Settings › Updates)*, so the
  record shows it as every request is shown (ADRs 0012, 0060).
- **Off means off.** With the switch off, alo sends nothing until *Check
  now* is pressed. That press is the person's cause.
- On alo OS the question does not arise: the updater is off there (§ 3).

## 6. Testable here, with no real key

Every check is a unit test, plus one test that runs the whole road against
a local server on `127.0.0.1`. The test key pair is made once with
`rsign2` and committed under the tests, named *test key*. The secret key is
in the repository, so the key is worthless by design. `alo-bundle` refuses
to build in a public key equal to it.

- **A local manifest server** (the test's own, as the CDP tests already
  have) serves a manifest, its signature and a package. It is a small
  `alo browser.app` stand-in, a folder with an `Info.plist` and a file.
- **Refused, and the installed copy untouched (checked byte for byte):**
  a manifest signed by another key; a manifest with one byte changed; a
  package with one byte changed (SHA-256); a package one byte short or long
  (`size`); a lower `serial`, replayed; an expired manifest; a `version`
  older than the installed one; an equal one; another `platform`; an
  installed version below `minimum`.
- **An interrupted download** (the server closes the connection half way)
  leaves a `.partial` and the installed copy intact. The next try resumes
  with `Range` and finishes. A server that answers the `Range` with the
  whole file is handled too.
- **The swap**: after it, the new folder is at the path and the old one in
  the hidden folder. A swap refused (a read-only folder) leaves the old one
  in place and says why. Folders in `$TMPDIR`, no `/Applications`.
- **Test vectors for `minisign-verify`**: known-good and known-bad
  signatures, including a changed trusted comment and a key with the wrong
  ID, so a crate update that changes its answers fails here.
- **Nothing leaves**: with the switch off, an idle alo makes no update
  request (the record and the local server agree). With it on, there is
  exactly the two `GET`s, carrying no cookie.
- Not testable here: Gatekeeper on a downloaded, notarised copy (735), and
  the Flatpak path (alo OS machine, 483).

## 7. What is public and what is private

- **Public, in this repository:** the updater, the manifest's format, the
  checks, the tests and the test key. It is also a small tool that writes a
  manifest from a package, so anyone can run an update feed with their own
  key.
- **Private, later, in the shipping repository** (not created by this ADR):
  the secret key's use, the update address, Developer ID signing and
  notarisation (735), the release builds, and watching CEF's index for new
  builds.

## The proposal

1. **One update: the whole application**, released within N days of a CEF
   build that carries a new Chromium (N = 3 proposed).
2. **A minisign signature of alo's own over a plain-lines manifest**,
   verified by `minisign-verify` 0.3.0 (MIT, no dependencies). Two public
   keys are built in, and no secret key is ever in this repository.
3. **The address and keys given at build time.** With none, the build does
   not update itself.
4. **Signature, serial, expiry, version, minimum, size, SHA-256 and
   `codesign`, in that order**, then an atomic swap with `rustix`'s
   `renameat_with(EXCHANGE)`. There is no `unsafe` of alo's.
5. **The person told; installed only at quit; never a restart by itself;**
   nothing for the agent socket.
6. **Checked daily, default on, said on the first screen**, with no
   identifiers and no report. It is off on alo OS, where Flatpak updates alo.
7. **Nothing given to anyone outside the owner's machines** until this is
   built, the key exists, and 735 is done.

## Asked of the owner

1. **The signer:** minisign's format with `minisign-verify` (an
   off-the-shelf signing tool, its own curve code), or raw Ed25519 with
   `ed25519-dalek` 3.0.0 (the audited curve, and a signer of alo's own to
   write)? This ADR proposes minisign.
2. **The key:** made by you, on a machine of yours, with a spare, both
   password-protected, never on this Mac's repository. Where is it kept? Who
   else may sign? (Until the private repository exists: nobody.)
3. **The check's default:** on (proposed, said on the first screen) or off
   until the person turns it on (alo OS's rule read strictly)?
4. **The address:** GitHub Releases of the future private repository, or a
   static host of yours? And on alo OS, who publishes
   `org.aloworld.Browser` to which Flatpak remote — Flathub, or alo OS's
   own (the coordinator's)?
5. **N, the promise:** 3 days after CEF's build (proposed)? And may a
   release ship before 735? It could go to the owner's own Macs only,
   checked by alo's signature, with Gatekeeper still refusing it elsewhere.

## Rejected

- **Updating Chromium apart from alo**: CEF's API changes every build, and
  the host is built for one.
- **Trusting HTTPS alone**: anyone who controls the host or a certificate
  could ship a browser.
- **Apple's signature alone**: it does not stop a replayed older version,
  and today there is none (735).
- **TUF (`tough`)**: right for many signers and mirrors, and too much for
  one. Its expiry, rising version and spare key are taken.
- **Sparkle**: Objective-C (law 4), and its RSS feed needs an XML reader.
- **A privileged helper to update `/Applications` for a standard user**: a
  root process alo does not need.
- **Restarting when idle**: an agent's work, or a person's half-typed
  words, is not alo's to end.
- **The update address in this repository**: the owner's rule; the public
  repository names no service.

## What is not measured, and is not hidden

- Chrome's own release dates after 30 October; whether CEF 156.0.8078.88
  and .96 carry Chrome 157's security fixes.
- The packed size of `alo browser.app`, and how long `ditto` takes to
  unpack it.
- Whether a bundle swapped with `RENAME_SWAP` keeps its notarisation ticket
  and passes Gatekeeper on first open (needs 735).
- Whether `ditto` keeps every extended attribute CEF's framework needs.
- How much a `GET` of a manifest every 24 hours costs a host (expected:
  nothing).
