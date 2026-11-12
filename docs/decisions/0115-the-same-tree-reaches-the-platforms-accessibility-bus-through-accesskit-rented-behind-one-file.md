# ADR 0115 — The same tree reaches the platform's accessibility bus through AccessKit, rented behind one file

**Status:** proposed — queue item 965, 12 November 2026. Nothing is built.
Questions for the owner are under *Asked of the owner*, below.
**Date:** 2026-11-12
**Context:** ADR 0002 (the layout tree is the agent's tree; *"the same tree
is the accessibility tree"*; no verb takes a coordinate; reading is never
watching); ADR 0003 (an id is never reused); ADR 0009 (MPL-2.0); ADR 0010
(a rented crate's `unsafe` is the crate's); ADR 0024 (the window is rented,
`winit` 0.30.13; § 2 the event loop never waits; § 4 a stranger's string is
never shaped in the browser process); ADR 0055 (the agent socket); ADR 0058
§ 4 (Chromium's tree read as ours); ADR 0074 (the conversation); ADR 0096
§ 6 and ADR 0099 (never a password, never a card's details); ADR 0105 (the
window's pixels kept on the GPU); ADR 0112 (requirement 7, and *Answered*:
*"this browser keeps Chromium's tree reaching the bus in item 965"*); ADR
0113 (a card drawn with no window of Chromium's; § 7, a windowless page has
no platform accessibility objects); law 4; queue items 960, 965, 966.

## The question in one line

alo OS asks for a full AT-SPI tree, published by default, and ADR 0002 says
that tree is the agent's tree: what do we rent to publish it, where do its
nodes come from — alo's own interface, and a card's page — and what is
never published?

## 1. What is rented: AccessKit

Read on crates.io and in the crates' own source, 12 November 2026:

| Crate | Version | Licence | `rust-version` | What it is |
|---|---|---|---|---|
| `accesskit` | 0.25.2 (25 October 2026) | MIT OR Apache-2.0 | 1.87 | The tree's types: `Node`, `Role`, `Action`, `TreeUpdate`, `TreeId` |
| `accesskit_winit` | 0.34.2 (25 October 2026) | Apache-2.0 | 1.87 | The adapter for a `winit` window; depends on `winit ^0.30.5` |
| `accesskit_macos` | 0.27.2 | MIT OR Apache-2.0 | 1.87 | NSAccessibility, on `objc2` 0.5 and `objc2-app-kit` 0.2 |
| `accesskit_unix` | 0.24.1 | MIT OR Apache-2.0 | 1.87 | AT-SPI over `zbus` 5.19, through `atspi` 0.29 and `accesskit_atspi_common` 0.21.1 |
| `accesskit_consumer` | 0.39.2 | MIT OR Apache-2.0 | 1.87 | The tree as the platform adapters read it |

Apache-2.0 is a licence this repository already takes beside MPL-2.0
(`winit` 0.30.13 is Apache-2.0, ADR 0024). AccessKit is the Rust
ecosystem's accessibility layer, from the same orbit as `winit`, and it
carries none of our value: it is the platform's physics, as HarfBuzz is.

**Measured: a probe resolves and compiles.** A throwaway crate in this
session's scratchpad (`ak-probe`, never in this repository) with
`rust-version = "1.89"`, `winit = "=0.30.13"`, `accesskit = "=0.25.2"` and
`accesskit_winit = "=0.34.2"` (default features off; `rwh_06`,
`accesskit_unix`, `async-io` on) was locked by cargo 1.97.1's
MSRV-aware resolver *"to latest Rust 1.89 compatible versions"* and
`cargo check` and `cargo clippy` passed on `aarch64-apple-darwin` with
`#![forbid(unsafe_code)]` at its root. The highest `rust-version` among the
crates it adds is 1.89 (`uuid` 1.29.2). **Not done:** a build with a 1.89
toolchain (this Mac has only 1.97.1; `rustup toolchain list`), and any build
for Linux (no Linux target is installed); for Linux only the resolution was
read, with `cargo tree --target x86_64-unknown-linux-gnu`.

**What it adds.** On macOS, five crates: the four AccessKit crates above
that macOS builds, and `uuid`. `objc2` 0.5.3, `objc2-foundation` 0.2.2,
`objc2-app-kit` 0.2.2 and `hashbrown` are already in `Cargo.lock`, beneath
`winit`. On Linux, 45 crates: the AccessKit crates, `zbus` 5.19 and its
`zvariant` and `zbus_names`, `atspi`, and the `async-io` runtime family
(`async-executor`, `async-io`, `async-channel`, `async-lock`, `blocking`,
`event-listener`, `futures-lite`, …). Every one is MIT, Apache-2.0, or
either, read from `cargo metadata`. `tokio` is not taken: `accesskit_unix`'s
`tokio` feature asks `^1.54.1` and the lock holds 1.53.2, and its default
`async-io` runs its own threads, which nothing of ours waits on.

**No `unsafe` of ours.** Every function we would call is a safe one in the
source: `Adapter::with_direct_handlers` and `with_mixed_handlers`,
`process_event`, `update_if_active`, and the `ActivationHandler`,
`ActionHandler` and `DeactivationHandler` traits we implement. The
`unsafe` is the crates' (ADR 0010): `accesskit_winit`'s `macos.rs`
subclasses the window's `NSView` with one `unsafe` call
(`SubclassingAdapter::new`), and `accesskit_macos` has `unsafe` in eight
files. `accesskit_unix` and `accesskit_atspi_common` have none.

**Behind one file.** `accesskit_winit` is named in one file of
`alo-window`, `accessible.rs`: it makes the adapter, hands it the window's
events, sends it updates and takes its requests. It is handed `winit`'s
`ActiveEventLoop`, `Window` and `WindowEvent` by `window.rs`, as
`present_shown.rs` already is. What a tree *becomes* is a second
responsibility, so it is a second file, `accessible_tree.rs`: a `Snapshot`
in, an `accesskit::TreeUpdate` out, a pure function, unit-tested on
numbers. No other file names `accesskit`.

Two facts the build must honour, read in `accesskit_winit` 0.34.2's
`lib.rs`: the adapter **must be made before the window is first shown**, or
it panics; today the conversation's window is created visible
(`window.rs`, `create_window` with no `with_visible(false)`), so it would
be created hidden and shown after. And the handlers *"may be called on any
thread"* — AccessKit's own on Linux — so they only send to the event loop,
which never waits on them (ADR 0024 § 2). Whether subclassing the view
disturbs ADR 0105's Metal layer is not measured.

### Rejected: AT-SPI by hand, over `zbus` and `atspi`

Publishing ourselves — `org.a11y.atspi.Accessible`, `Component`, `Action`,
`Text`, `Value`, the registry, the cache, the events — is the same
`zbus` and `atspi` crates and a large protocol of ours to keep right, with
no macOS road at all, so nothing of it could be tested on this Mac.
AccessKit already is that code, written by people whose work is
exactly that. Rent the physics.

## 2. Where the tree comes from

There is no second tree (ADR 0002). Each part of alo's own interface — the
chats panel, the rail, the first screen, a card's frame and the strip — is
a document of ours drawn by a confined renderer of its own (ADR 0074, ADR
0024 § 4), from markup that declares its roles (`panel_document.rs`,
`rail_document.rs`, `start_document.rs`, `card_document.rs`:
`aria-current`, `aria-busy`, `aria-label`, `aria-hidden`). Each renderer
already answers `ToRenderer::ReadTree` with a `Snapshot` (`alo-renderer`'s
`snapshot.rs`): the `AgentTree` of `alo-agent`, copied at one instant. Today
only tests ask the panel for it (`a_press_on_the_conversation.rs`); the
window does not.

The window would ask each part's renderer for its tree **only while an
assistive technology is active** — AccessKit calls `request_initial_tree`
when one asks, and `deactivate_accessibility` when it goes; on Linux it
watches `org.a11y.Status`'s `IsEnabled` — and again after a part paints a
new frame. That is ADR 0002's *reading is never watching*: nothing is read
until something asks, and it is read from the trees that drew the frame.

Each part is an AccessKit **subtree** (`TreeId`, AccessKit 0.25's graft
points): the window's root is a `Window` node; each part is a graft node
placed where the window composed it; its `Snapshot` is that subtree. Each
renderer allocates its own `BoxId`s (ADR 0003), so each tree is its own id
space and none can name another's node.

| `SnapshotNode` | AccessKit `Node` |
|---|---|
| `id` (a `BoxId`) | `NodeId`, within the part's `TreeId` |
| `role`, a `KnownRole` | the same role by name: `TextBox` → `TextInput`, `SearchBox` → `SearchInput`, `Radio` → `RadioButton`, `Option` → `ListBoxOption`, `Date` → `DateInput`, `ProgressBar` → `ProgressIndicator`, `Separator` → `Splitter`, `Summary` → `DisclosureTriangle`, `Text` → `Label`; every other `KnownRole` has an AccessKit role of its own name |
| `role`, `Generic` / `Presentational` | `GenericContainer`, or read through |
| `role`, `Declared` | a role only from a written table (Chrome's `InputTime` → `TimeInput`); otherwise `GenericContainer`, never the nearest guess |
| `name`, `placeholder` | `label`, `placeholder` |
| `states` | `disabled`, `toggled` (checked), `selected`, `expanded`, `pressed`, `required`, `read_only`, `busy`, `invalid`, `level`, `aria-current` |
| what a field holds (the text inside it) | `value`, unless § 4 withholds it |
| `rect` | `bounds`: the part's place in the window plus the rect, times the window's scale |
| `children` | `children`, in the order a person meets them |

**Bounds are physical pixels of the window's content.** Measured in the
probe: a node given `Rect::new(8, 8, 96, 40)` at scale 2 read back over
NSAccessibility as size 44 × 16 at four points in from the window's left
edge. Bounds are read, never acted on: a screen reader draws its focus
ring around them, and AT-SPI's hit test at a point is answered by
AccessKit from them, with nothing sent to alo.

### The actions we accept, and those we refuse

A node offers only what alo's verbs already do, by node, never by place:

| AccessKit `Action` | alo |
|---|---|
| `Click` | `Verb::Activate` on that node |
| `SetValue` with `ActionData::Value` | `Verb::PutText` on that field |
| `Focus` | the person's focus moved to that node, as a press on it would — **a new verb**, so built with tests of its own |

**Refused**, never offered on a node and ignored if asked anyway (the AT-SPI
adapter forwards some without checking what a node offers):
`ScrollToPoint` and `SetScrollOffset` (each carries a point, a coordinate;
ADR 0002), and `SetTextSelection`, `ReplaceSelectedText`, `ScrollUp`,
`ScrollDown`, `ScrollLeft`, `ScrollRight`, `ScrollIntoView`, `Increment`,
`Decrement`, `Expand`, `Collapse`, `ShowContextMenu`, `ShowTooltip`,
`HideTooltip`, `CustomAction`, `Blur` and
`SetSequentialFocusNavigationStartingPoint`, until each is a verb of ours
with tests. `ScrollDown` by a page, say, is an amount, not a place, and may
become `Verb::Scroll` later; it is not taken now.

## 3. A card's page

**With a window of Chromium's** (every card today, ADR 0076), Chromium
publishes its own platform tree for that window. CEF 156.0.14's
`cef_browser.h`, `SetAccessibilityState`: *"For windowed browsers
accessibility will be enabled in Complete mode … all platform
accessibility objects will be created and managed by Chromium's internal
implementation"*, and with the default state *"the state may be further
controlled with the "force-renderer-accessibility" … command-line
switches"*. On Linux that is Chromium's own AT-SPI bridge. But that window
is parked off every display and is not where the person sees the page;
whether VoiceOver or Orca would read it sensibly was not measured.

**With no window of Chromium's** (ADR 0113, behind `--alo-windowless`), the
same header: *"For windowless browsers accessibility will be enabled in
TreeOnly mode … platform accessibility objects are not created."* So alo
OS's `--force-renderer-accessibility` gives a windowless card nothing on
the bus.

**Proposed: one road for both.** A card's page reaches the bus only as a
subtree of alo's, under the card's graft node, built from the tree alo
already reads: `Accessibility.getFullAXTree` over CDP, mapped to a
`Snapshot` by `chrome_tree.rs` (ADR 0058 § 4; ADR 0113 measured 30 nodes,
the *Send* button found by name). Its node ids are Chrome's
`backendDOMNodeId`s, allocated once per document; its rects are CSS pixels
of the page, placed at the card's page box. A `Click` on it is the same
activation the socket's `activate` sends, a `SetValue` the same `put_text`.
`CefAccessibilityHandler`'s events are not taken: they are a second reader
of the same tree, and the CDP road is built and bounded.

**Bounds on size** are the ones that already hold: a message from Chrome
nests at most `chrome_protocol::MOST_NESTING` (16) deep, a tree is read
with at most `MOST_NODES` (100 000) nodes and `MOST_DEPTH` (512) deep
(`chrome_tree.rs`), and a tree deeper than `asking::MOST_DEPTH` (256) is
refused as the socket refuses it (`Refused::TreeTooDeep`). A page past them
is published as its card's graft node alone, named *"this page is too large
to read"*, never half a tree said to be whole. The page's tree is read only
while an assistive technology is active, at most once per frame streamed
(ADR 0090's pace).

## 4. What is never published

The socket's rule, kept exactly (`agent_page.rs`'s `is_secret`,
`agent_text.rs`, `serving.rs`'s `Refused::Password` and
`Refused::CardDetails`, ADR 0096 § 6, ADR 0099):

- **A password field** is published as `PasswordInput`, with its name and
  no value at all: not its dots either (AccessKit allows a password's value
  only as masking characters; we send none).
- **A field whose name or hint says it holds a secret** — a passcode, a
  PIN, a card's number, security code or expiry, a one-time code
  (`is_secret`) — is published with its role and name and **no value**, and
  nothing inside it is published.
- **`SetValue` is refused on both**, as the socket's `put_text` is refused.

The bus is read by whatever runs as the person — a screen reader, and
`alo-app` acting for an agent alike — so a rule that kept a secret from the
socket and gave it to the bus would keep nothing. The cost is a person's:
Orca cannot read back the card number they typed (*Asked of the owner*, 2).

## 5. What this Mac can test, and what only alo OS can

**Measured on this Mac** (macOS 26.5.2), with the probe shown: `osascript`
and System Events read the window's tree that AccessKit published —
`button "Send" of group 1 of window "ak-probe"`, its `position` and `size`,
and its actions (`AXPress`) — and `perform action "AXPress"` arrived at our
`ActionHandler` as `Action::Click` on node 1. No `unsafe` and no AX API of
ours was needed. The caller needs the Mac's Accessibility permission; this
session had it.

So a test here starts alo's conversation window shown, and asserts in
numbers: the panel's chats as links by name, the one `aria-current`; the
first screen's box as a text field by name; a node's size and place in
points; a password field with no value; a press by `AXPress` reaching
`Verb::Activate` and nothing else; and that a node's actions, as System
Events lists them, are only those of § 2's accepted table. It is a shown-window test, in the
shown-Chrome group's threads (`.config/nextest.toml`), and it needs an
unlocked screen.

**Only on alo OS** (queue item 960 first; no such machine here): the same
tree on AT-SPI under Wayland — Orca reading the panel, the first screen and
a windowless card's page; `alo-app <app> controls`, `read` and `press`
through AT-SPI; that `ScrollToPoint` from an AT-SPI client changes nothing;
and the window's bounds on screen, which Wayland does not tell a client, so
AT-SPI's screen coordinates may be wrong there (not measured).

## 6. Phases — scope is cut, never depth

1. **(a) alo's own interface, on macOS.** `accessible.rs` and
   `accessible_tree.rs`; the conversation's window created hidden and shown
   after the adapter; each part's `Snapshot` read while active; Click,
   SetValue, Focus; § 4's rule; the osascript test above. On this Mac.
2. **(b) A card's page grafted**, from `chrome_tree.rs`'s `Snapshot`, for
   windowed and windowless cards alike, with § 3's bounds. On this Mac,
   behind `--alo-windowless` until ADR 0113's items are done.
3. **(c) AT-SPI on alo OS**: `accesskit_unix` built in item 960's Linux
   build, verified there by Orca and `alo-app`. Closes item 965.

## Asked of the owner

1. **Is an action from the bus a person's or an agent's?** Orca and an
   agent behind `alo-app` both send `Click` on AT-SPI, and nothing says
   which. This ADR proposes: a press that pays is held for the person's
   yes as alo's agent's is (ADR 0099), and every action from the bus is
   in the record (ADR 0012 § 4) as *"assistive technology"*. Or none
   held, as for a person?
2. **A person's own secret read back to them.** § 4 withholds a card's
   number from Orca too. Keep it so (proposed), or publish it to the bus
   and accept that any process of the person reads it?
3. **Who may read.** The socket is the person's grant (ADR 0055); the
   accessibility bus is open to every process of the person's session.
   ADR 0002 leaves *who may ask* to alo OS's capability model. Is the bus
   alo OS's to guard, as this ADR assumes?

## Rejected

- **AT-SPI by hand** over `zbus` and `atspi`: § 1.
- **Chromium's own platform objects for a card's page**: a windowless page
  has none (CEF's header), and a windowed one's sit in a window parked off
  the screen; two roads would be two trees for one page.
- **`CefAccessibilityHandler`'s events**: a second reader of what CDP
  already gives, bounded.
- **Publishing the tree at all times**: it would be watching, not reading
  (ADR 0002); AccessKit asks only while something listens.
- **`accesskit_macos` and `accesskit_unix` directly**, without
  `accesskit_winit`: making the macOS adapter for a view is an `unsafe fn`
  taking a raw pointer, which would be `unsafe` of ours.

## What is not measured, and is not hidden

- A build with a 1.89 toolchain, and any Linux build; only resolution was
  read for Linux.
- AccessKit's subclassed view beside ADR 0105's Metal layer.
- Whether a parked Chromium window's own tree is readable by VoiceOver.
- Anything on AT-SPI, Wayland, Orca or `alo-app`.
- How long a part's `ReadTree` takes while a screen reader is active, and
  whether the first tree arrives *"no later than the next display
  refresh"*, as AccessKit asks, when a renderer is slow.
- That an AccessKit graft node with no subtree yet is left out of the
  platform tree: in the probe it was not among the window's elements, which
  is all that was seen.
