# ADR 0112 — alo browser becomes alo OS's default browser, by alo OS's guidelines

**Status:** accepted — the owner's decision, 11 November 2026, relayed
with the alo OS coordinator's requirements. The first three steps that
need neither Linux nor the coordinator's answers are built (queue items
956 and 957 closed; 958 built on macOS, its Linux half owed; *Built*,
below); the rest are queued from 959.
**Date:** 2026-11-11
**Context:** `ROADMAP.md`, *Where it runs* (alo OS: *"Firefox remains the
right pin until this browser meets stage 2's exit gate, and nothing here
asks for the slot before then"*); ADR 0001 (stages); ADR 0002 (the tree is
the agent's; no verb takes a coordinate); ADR 0010 (the sandbox); ADR 0024
§ 4 (a stranger's string is never shaped in the browser process); ADR 0055
(the agent socket); ADR 0074 (the interface is a conversation); ADR 0076
(a page drawn as a card, streamed from a parked window); ADR 0077 (alo
carries its own Chromium); ADR 0105 (the window kept between frames);
ADR 0111 (sign-ins); queue items 169, 480, 481, 483, 696, 721, 728; and,
read-only, `alo-os` at `origin/main` 62b56a72 (26 October 2026):
`crates/alo-software/shipped.toml:42–47` pins `org.mozilla.firefox`
156.0 from Flathub for `role = "web-browser"`.

## The decision in one line

alo browser will be **alo OS's default web browser** once it passes **alo
OS's bar, on an alo OS machine**; until then Firefox stays the default,
and **a person can always choose another default browser**. Toward that,
this repository **follows alo OS's guidelines** for an application — the
eleven requirements below — and treats them as requirements, not
suggestions.

## The two rules

1. **The bar, not our stage gate, decides the slot.** `ROADMAP.md` said
   alo browser would not seek alo OS's browser slot before stage 2's exit
   gate (a person uses it as their browser for a week). The owner has
   decided otherwise: the slot is sought when the bar below is met on an
   alo OS machine, which is alo OS's judgement under their gates, not a
   certification this repository makes. Stage 2's exit gate is unchanged
   and still governs stage 2; it no longer gates alo OS.
2. **alo OS's guidelines bind here as they bind any application.** Where a
   guideline and an earlier decision of ours disagree, the earlier
   decision is amended in an ADR of its own — never quietly bent — and a
   guideline whose detail is still open with the coordinator (the look:
   item 10) is not acted on until it is answered.

What does **not** change: the four laws; ADR 0002 (no verb takes a
coordinate; the tree is the agent's tree); ADR 0024 § 4 on this
process's own text shaping; nothing leaves silently (which item 9 makes
alo OS's requirement too); the person's choice of browser, which neither
repository may take away.

## The eleven requirements, and where this repository stands today

Status words: **met**, **partly**, **not yet**, **open** (waiting on the
coordinator). Paths are this repository's.

| # | Requirement (the coordinator's, relayed by the owner) | Today |
|---|---|---|
| 1 | Wayland only, no X11; a real `xdg_toplevel` `app_id` equal to the `.desktop` name; the title is the page's title | **partly.** There is no Linux build at all (§ *What blocks*). Built today (958): the conversation's window asks for `app_id` `org.aloworld.Browser` on Linux (`crates/alo-window/src/window.rs`, `named`; the identifier is `info_plist.rs:24`), and its title follows the live card's page (`window_title.rs`, `conversation_window/title.rs`, `chrome_card_title.rs`) — handed to the window server only where the window server draws it, which under Wayland's server-side decorations is the compositor; on macOS `AppKit` would shape it in this process, so there the window stays *alo* (ADR 0024 § 4). Not compiled for Linux here: no Linux target on this machine. |
| 2 | Server-side decorations only; no window buttons of our own | **not yet verified.** The conversation draws no close/minimise/maximise of its own; whether `winit` asks for server-side decorations on alo OS's compositor is unmeasured (item 962). |
| 3 | No GPU required: software rendering must work; the GPU only accelerates | **partly.** This engine draws on the CPU (`softbuffer`; `wgpu` only on macOS, ADR 0105). The carried Chromium (CEF) has not been run without a GPU (item 960). |
| 4 | Cursors by name (`wp_cursor_shape_v1`); `xdg_toplevel.set_fullscreen` for video and F11 | **partly.** Cursors are named (`CursorIcon`, `window.rs:1102`), which `winit` maps to the cursor-shape protocol where the compositor has it — unmeasured on alo OS. No full screen: F11 and a video's full screen are not built (item 962). |
| 5 | Sandboxed Flatpak-style; files, downloads, links and secrets through `xdg-desktop-portal` | **not yet.** No Flatpak (483), no portal code. Keys are in macOS's keychain through `/usr/bin/security` (`keychain.rs:35`); the clipboard is `pbcopy`/`pbpaste` (`clipboard.rs:8`). Whether the renderer's own sandbox can nest inside a Flatpak is 481's open question (ADR 0010, 169). |
| 6 | A `.desktop` file: `MimeType` `x-scheme-handler/http`, `x-scheme-handler/https`, `text/html`; `GenericName` *Web Browser*; translated `Keywords` | **not yet** (item 963). |
| 7 | Three equal roads: an adapter with typed verbs (open URL, new tab, read page text, find, fill a field, click a named control, print or save as PDF) and a full AT-SPI tree published by default | **partly.** The socket (ADR 0055) already reads the tree and fills a field and presses a control **by name** (`put_text`, `activate`); built today (956): `open` (in a named tab, or a new one), `new_tab`, `find` (answered by name, never by place) and `print_pdf` (to a directory the person allowed). *Read page text* is the tree; a plain-text ask is item 964. No AT-SPI tree (item 965); no adapter in alo OS's own format (item 966). |
| 8 | A separate instance with its own profile directory, like Firefox's `--profile DIR --no-remote`; no helper shared across profiles | **partly → built for the profile (957).** `alo --data DIR` keeps its settings and its socket under one directory, and a second alo with another directory shares none of them; with no secret store yet, it keeps no key at all — sign-ins and chats last only for the run, and a person's own service keys are not kept (957, as amended below); each card's Chromium was already its own, thrown away with its card (ADR 0069). Not built: a second launch with the *same* directory handing its address to the running one — which alo OS specifies through OpenURI (open question) — and keys in a real secret store (§ 5). |
| 9 | Nothing leaves silently: no telemetry, crash upload, phone-home, prefetch or safe-browsing calls unless the person turns them on; every request attributable to the browser process | **partly.** No telemetry exists (`ROADMAP.md`, *Not built*); every page request is held and recorded (ADRs 0060, 0073). Chromium's own last requests — the on-device models manifest among them — still leave (696). Unmeasured on alo OS with a net log (item 968). |
| 10 | Every string externalised for the 24 EU languages; Rust only; alo OS's palette (navy `#102A43`; deep teal `#0F6B72` reserved for alo acting, not the browser's own chrome) and typeface Manrope | **not yet; the look is open.** Strings are written in English in the code. The palette, the agent's colour and the typeface are **open with the coordinator** and are not changed here until answered (§ *Open questions*). |
| 11 | Logical units, scaled per display | **partly.** The conversation lays out in CSS pixels at the window's scale (`scale.rs`), per display on macOS; Wayland's fractional scale is unmeasured (item 962). |

## What blocks most of it: there is no Linux build

Said plainly, because every row above that says *not yet* starts here:

- **The carried Chromium's host is macOS only.**
  `crates/alo-cef-host/src/main.rs:41` is a `compile_error!` elsewhere
  (item 728), and `alo-window`'s pipe to Chromium on descriptors 3 and 4
  is Unix's.
- **The renderer's sandbox has no Linux backend** (ADR 0010, item 169),
  and a renderer that cannot sandbox refuses to read a page.
- **Eighteen source files carry `cfg(target_os = "macos")`**, among them
  the clipboard (`clipboard.rs`), every key (`keychain.rs`,
  `chat_key.rs`, `sign_in_key.rs`), what the window server shows
  (`window_server.rs`, `osascript`) and a card's menu (`chrome_menu.rs`).
- **A card cannot be built on Wayland as it is built today.** ADR 0076 B
  parks each card's Chromium window off every display
  (`chrome_window.rs:352`, `PARKED`) and streams its frames, and *Expand*
  places Chromium's own window over the conversation
  (`conversation_chrome.rs:477`). Wayland lets no client place a window or
  put it off screen. The likely road is CEF's off-screen rendering — the
  page drawn into a buffer we are handed, with no window of Chromium's at
  all — which is a change to ADR 0076 and is decided in its own ADR,
  measured, before it is built (item 959).

## The bar

From the coordinator, relayed by the owner. alo browser is offered as the
default when all of this holds **on a clean alo OS image, on a machine
with no GPU and 4 GB of memory**:

1. It installs from its source and starts.
2. Real sites work.
3. alo OS's band, minimise, maximise and full screen work on its window.
4. A download and an upload go through the portal.
5. The launcher finds it by *browser*.
6. A link opened in another application opens in it.
7. `alo-app` and alo can open a URL in it and read a page.
8. A second instance runs on its own profile.
9. It makes no network traffic while idle.

The judgement is alo OS's. This repository records each of the nine as
measured on that machine — the image, the machine and the date named —
and never certifies the bar itself (`LOOP.md`, *Stage 2 ends where a loop
cannot follow*).

## What changes in this repository's stance

- **`ROADMAP.md`'s alo OS passage is amended** in the same change: the
  slot is sought at the bar above, not at stage 2's exit gate; Firefox
  stays the pin until then; a person can choose another browser.
- **Running on Linux moves from "stage 2 work, gated on 169" to the head
  of the road**, together with the card's off-screen drawing (959).
  Queue items 480, 481, 483 and 728 are what it builds on; their *blocked:
  stage 2's exit gate* is lifted for the alo OS road by this decision, a
  person's (`LOOP.md`: *Stage 4 does not start until a person says so*).
- **"It needs hardware" is true for part of this road and is said per
  item**: every item marked *alo OS machine* below needs an alo OS image —
  real hardware or the qemu image the coordinator can provide — and is
  not verified until run there. The rest runs on this Mac.

## Open questions, sent to the coordinator

1. **The palette and the agent's colour.** alo browser's interface uses
   ADR 0074's palette, and alo's agent at work is drawn in terracotta. alo
   OS reserves deep teal `#0F6B72` for *alo acting*. Should the browser's
   agent-at-work colour become that teal, and its own chrome take navy
   `#102A43`?
2. **The logo.** alo browser's mark uses a teal of its own; does it take
   alo OS's, or keep its own within alo OS's rules?
3. **The typeface.** The conversation draws in Inter by default (ADR
   0084's amendment, chosen in *Settings*); alo OS's is Manrope. Is Manrope
   required for the browser's own interface, and is Inter kept as a
   choice?
4. **OpenURI and an instance already running.** alo OS opens an address
   through `org.freedesktop.portal.OpenURI` (481's answer); the
   already-running case was owed by them. How is an address handed to an
   alo browser already running on the same profile?
5. **A way to run the qemu image** here, so the *alo OS machine* items
   can be verified without the owner's hardware.

Until they are answered, nothing in item 10's look changes, and item 4
is built only as far as a separate instance.

### Answered (the coordinator, on the owner's decisions, 12 November 2026)

- **The app id** is `org.aloworld.Browser`: the Flatpak id, the
  `xdg_toplevel` app_id and the `.desktop` name.
- **1. The agent's colour — "Deep teal everywhere".** Deep teal `#0F6B72`
  (dark `#77C8C6`) means *alo is acting* on every alo product and platform,
  the Mac included; terracotta is retired from that role. It is always
  drawn with alo's mark and a word, never colour alone. ADR 0074 § 2 is
  amended by item 974.
- **2. The logo — "Allowed: a logo is a mark".** The owner's teal-gradient
  logo stays as it is.
- **3. The typeface — "Manrope".** Manrope is final for alo's interfaces,
  Geist Mono for monospace; the conversation moves off Inter as its
  default (item 975, amending ADR 0084).
- **4. OpenURI.** alo OS will answer `OpenURI.OpenURI` for http and https
  by opening the person's chosen default browser (owed on alo OS's side,
  their #1167). A second launch on the **same** profile hands its address
  to the running instance, which opens it and comes forward; a launch with
  a **different** `--data DIR` — alo's own copy — stays a separate
  instance (item 976).
- **5. No image is handed out**, by alo OS's rule, until signing is in
  place. The loop instead: this repository publishes a Linux artifact (a
  Flatpak bundle, or at first a tarball or a minimal test binary) with its
  SHA-256; the coordinator installs it on a fresh image in qemu (no GPU,
  4 GB, 1280 × 800, Wayland only, no Xwayland), runs exactly the
  measurement named, and sends back screenshots, journal lines and which
  bar items passed, each marked measured once. Publishing an artifact is
  outward-facing and asks the owner first.
- **The roads.** alo OS's "Three equal roads" (its `features.md`, the
  owner, 6 November 2026), its terminal commands (`alo-app`, its ADR
  0147) and the own-copy rule (`work-in-alos-copy`, its
  `application-door.md`, #1163) are newer than the copy read here. alo's
  copy starts the application with a profile of its own and counts only
  that copy's traffic: `--data DIR` is what alo's copy will pass, and alo's
  copy is signed in once by the person and stays signed in (item 969).
- **The adapter.** alo OS's `app-adapters.md` is current. Our socket's
  JSON lines are the *application's automation interface* road, the best
  one; no D-Bus service is needed from the browser. The adapter is Rust,
  implementing alo OS's `Adapter` trait, mapping its typed verbs onto the
  socket's asks, and never offering a verb that runs code a model wrote.
  The coordinator proposes this repository writes it (item 977; where its
  code lives is the owner's to decide, since this repository never edits
  alo OS's).
- **The accessibility tree.** alo OS found an Electron app publishing
  only its top-level window unless started with
  `--force-renderer-accessibility`, and now adds that switch for
  Chromium-family apps it launches; this browser keeps Chromium's tree
  reaching the bus in item 965.

## The plan, in order

| Order | What | Items | Needs |
|---|---|---|---|
| 1 | A Linux/Wayland build whose cards need no window placement: the ADR for CEF off-screen drawing (ADR 0076's change), the host on Linux, the renderer's sandbox, the workspace's macOS-only files given Linux counterparts, software rendering with no GPU | 959, 960, 961, 728, 169, 480 | 959's ADR on this Mac; the rest an **alo OS machine** |
| 2 | Flatpak and portals: the manifest under `org.aloworld.Browser`; downloads, uploads, `print_pdf`'s files, links out and keys through the portals | 483, 967 | **alo OS machine** |
| 3 | The roads: AT-SPI published by default; the socket's remaining asks (a page's text); alo OS's adapter; the data directory's keys in a secret store | 964, 965, 966, 969 | 964 here; 965–966 **alo OS machine**; 969 the coordinator's portal answer |
| 4 | Registration and the window: the `.desktop` file, the default-browser handler, server-side decorations, full screen, cursor shapes, fractional scale | 962, 963 | **alo OS machine** |
| 5 | Egress: nothing leaves silently, measured with a net log on alo OS, Chromium's own last requests stopped | 696, 968 | 696 here; 968 **alo OS machine** |
| 6 | Translations and the look: strings externalised in Rust for the 24 EU languages; palette, agent colour, logo and typeface once answered | 970, 971 | 970 here (an ADR first); 971 **the coordinator's answers** |
| — | The bar, run whole on a clean image | 972 | **alo OS machine**, and alo OS's judgement |

## Built (11 November 2026)

**956 — the socket's `open`, `new_tab`, `find` and `print_pdf`**
(`docs/contracts/agent-socket.md`, additive; `asking.rs`, `protocol.rs`,
`serving.rs`, `agent_find.rs`, `agent_saves.rs`, `agent_scope.rs`,
`conversation_agents.rs`, `conversation_window/outside.rs`,
`chrome_tab.rs` `go_for_agent` and `print_pdf`, `chrome_protocol.rs`
`Page.printToPDF`; `alo-ask --plain` prints each). Decided here:

- **`find` answers nodes by number, role and name, never a rectangle**
  (ADR 0002), from the same tree `read` answers; the document's own name
  (its title) is never found, and a node repeating its found ancestor's
  name is not answered twice.
- **`print_pdf` writes only where a person allowed** — `alo --agent-saves
  DIR`, as often as they like; with none, every path is refused — judged
  with every link followed, before any tab is looked at; the file is made
  new (`O_EXCL`, never over a file or a link), `0600`, written by the
  browser process and only if what the renderer printed begins `%PDF-`.
  On alo OS the document portal replaces this (967).
- **`open` names its tab by address, or none for a new one**; the address
  is judged (http or https, the agent's sites) before any tab is looked
  at; a tab sent somewhere is an agent action in the record and is
  answered by where it arrived, and a server's redirect to a site agents
  may not reach is answered `NotOpened`, never named.
- **alo's new tab is a new chat.** `new_tab` is *New chat*; `open` with
  no tab is *Open in new chat* — the person's own roads, so the card is
  held to every rule a person's is. Only the conversation offers them;
  `alo --chrome` and this engine's window answer `NotOffered`.

**957 — `alo --data DIR`** (`data_dir.rs`, `opening.rs`, `bin/alo.rs`,
`service_keys.rs`): what is not secret — the settings and the sites
blocking is off for, in `DIR/Chats/`, and the agent socket at
`DIR/alo-browser/agent.sock` — under one private directory; a second alo
with another directory shares none of it.

**Nothing secret is kept under `--data` until there is a secret store**
(the coordinator, 11 November 2026). `--data` is a product flag — alo OS's
own-profile instance — not a test fixture, and a keychain file unlocked
by a password written in this public repository is unlocked by a public
password: it protects nothing. A first build did exactly that and was
withdrawn before the gate; the code path is gone. So under `--data`, until
969 gives a real secret store:

1. **the cards' sign-ins are held in memory only**, for the run, and never
   written; *Settings › Privacy* says *Sign-ins are kept only until alo
   quits* (the row's existing words), and the terminal the same;
2. **a person's own service keys are not kept**: *Settings* says *Your own
   keys can't be kept in this data folder yet. The keys alo carries still
   work.*, naming no provider, and the keys the application carries are
   still used;
3. **the chats are not kept**, since their key would have nowhere to be;
   the window says why, as it says any chats not kept;
4. **no keychain is touched at all** — neither a file under `DIR` nor the
   person's login keychain — so a second instance never reads, makes or
   replaces a key of the first's.

An existing directory is taken only if already private and is never
changed; `--data` beside `--chats` or `--sign-ins` is refused. (The
scratch keychain `--sign-ins DIR` makes for a hand run, ADR 0111 § 6,
predates this and is unchanged; it is not `--data`'s.)

**958 — the window's title and the application's name** (as in row 1).

Tests: unit tests of each new file and of the protocol, the serving and
the relay; `a_page_opened_found_and_printed_by_an_agent_through_chrome.rs`
and `a_new_chat_opened_by_an_agent_outside_through_chrome.rs`, each
through the machine's Chrome and alo's CEF host; and
`two_alos_with_their_own_data_in_the_window.rs`, two `alo` processes
reached at their own sockets, nothing secret written under either
directory (no keychain, no sealed file, no sign-ins) and the terminal
saying sign-ins are kept only until alo quits; the data directory's
keeping (`data_dir.rs`: no keychain, sign-ins in memory, a sign-in held
and no file written, the *Settings* row's words), carried-only keys
(`service_keys.rs`), and *Settings*' line for a key given
(`conversation_window.rs`).

## Alternatives rejected

- **Keep stage 2's exit gate as the gate for alo OS.** It was this
  repository's own reading; the owner decided the slot belongs to alo
  OS's bar, which is about alo OS's machine and is judged there.
- **Ask for the slot now and let Firefox go.** A default that does not
  yet install on alo OS would leave a person with no browser; Firefox
  stays until the bar holds.
- **Set the page's title on macOS's window too.** `AppKit` draws the
  title bar in this process, which would shape a stranger's string here
  (ADR 0024 § 4). The rule stays for macOS until an ADR changes it.
- **Let an agent name any path for a PDF.** The browser process would
  write wherever the person can; refused.
- **A data directory whose keys go in the person's login keychain under
  the same items.** *Forget all sign-ins* in one instance would replace
  the other's key and make its sign-ins unreadable.
- **A data directory whose keys go in a keychain file of its own, unlocked
  by a password this repository writes.** Built first and withdrawn: a
  public password protects nothing, and `--data` is a product flag.

## Consequences

- Queue items 956 and 957 are closed, 958 is built and open for its
  Linux half; 959–972 are queued, each saying what
  closes it and whether it needs an alo OS machine.
- `ROADMAP.md` is amended; stage 2's exit gate is not.
- Firefox remains alo OS's pinned browser until alo OS judges the bar met.
