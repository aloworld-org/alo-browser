# ADR 0024 — A window is rented, and the tab strip is a page we render

**Status:** accepted
**Date:** 2026-10-06
**Context:** queue item 118, *a window, tabs, and a tab strip*, the first line
of `ROADMAP.md`'s *The browser itself* — what stage 2's exit gate measures, *"a
person uses it as their browser for a week"*, and which nothing in this
repository can be used as without; ADR 0001 (rent the physics, build the
engine); ADR 0002 (the layout tree is the agent's tree, and no verb takes a
coordinate); ADR 0005, which gave the browser process *"the display, the
window"* and forbade it to parse a page; ADR 0009 (every rental sits beside
MPL-2.0); ADR 0010 (a rented crate's `unsafe` is the crate's, not ours); ADR
0018, which deferred *"input from a person — a window, a pointer and real
coordinates"* to this; ADR 0020, which refuses a link's `target="_blank"` by
name *"until there is"* a tab strip; law 4; and the code this has to fit —
`alo-renderer`'s `tab.rs` (`Tabs`, which keeps each tab's last frame),
`host.rs` (one renderer per site, every exchange bounded by
`answers::LONGEST_SILENCE`) and `frame.rs` (a frame is read-only pixels, the
one thing ADR 0005 lets processes share)

## The decision in one line

The window is **rented** — `winit` for the window and its events, `softbuffer`
for putting pixels in it, both through safe interfaces and each named in one
file — and owned by the browser process, which **composes** it from frames it
was sent and never waits on a renderer to do so; the **tab strip is a document
of our own, built from the browser's state as data and rendered by the engine
in a sandboxed renderer of its own**, so the agent reads tabs the way it reads
a page; and a person's pointer is the one coordinate that ever reaches a
renderer, hit-tested there, with what it meant **claimed by the renderer and
decided by the browser process**.

## Why this is a decision rather than a chore

The item reads like plumbing and is four decisions an iteration could each make
quietly. *Which crate* puts a toolkit's threading model and licence inside the
one process that holds the profile. *Who draws the tab strip* decides whether
ADR 0002 holds for the browser's own interface or only for pages. *Where page
titles are drawn* decides whether a stranger's string is rendered in a
privileged process. And *where a person's click goes* is the coordinate ADR
0002 forbids an agent, arriving for the first time.

## The survey, taken 2026-10-06

From crates.io and each project's repository on the date above; licences and
dates as each states them.

| Candidate | What it is | Verdict |
|---|---|---|
| **`winit` 0.30.13** (2 March 2026; Apache-2.0) | The Rust ecosystem's window and event crate: a window, its size and scale factor, pointer, keyboard, IME and focus events, on macOS, Windows, X11 and Wayland | **Taken** (§ 1) |
| `winit` 0.31 | In beta (`0.31.0-beta.3`, 4 September 2026) | **Not yet.** A beta is not rented; moving to 0.31 is a version bump when it is stable |
| **`softbuffer` 0.4.8** (13 December 2025; MIT OR Apache-2.0) | Shows a buffer of CPU-drawn pixels in a window that implements `raw-window-handle` 0.6, from the same organisation as `winit` | **Taken** (§ 1) |
| `tao` | Tauri's fork of `winit` | Not taken: the same model, fewer users, tied to one application framework's needs |
| `wgpu` and a GPU surface | The way to put pixels on screen fast | **Not now.** Hardware acceleration for paint is item 116 and needs a GPU to verify; the software path is right first (law 3) |
| SDL2, GTK, Qt | Whole toolkits | **C or C++**, each with its own threads and widgets, in the privileged process |
| `objc2` and AppKit by hand | What `winit` itself does on macOS | **`unsafe` of ours** on every call; law 4 |

`winit` and `softbuffer` carry `unsafe` inside them, as every crate that talks
to a window server must: on macOS through the `objc2` crates (MIT). That is
ADR 0010's *the crate's, not ours*. Both are **used through safe interfaces**
— `winit`'s `ApplicationHandler` and `create_window`, and `softbuffer`'s
`Context::new`, `Surface::new`, `buffer_mut` and `present` over an `Rc` of the
window — so **this decision authorises no `unsafe` in this repository**. That
each of those is a safe function is the crates' documentation's claim, taken
from it here, and the commit that adds them checks it in their source; if one
is not, that commit stops and this ADR is reopened.

## 1. The window is rented, and named in one file each

- **`winit` 0.30, default features on**, named only in a new crate's
  `window.rs`; **`softbuffer` 0.4**, named only in its `present.rs`. Both are
  added to `gate.sh`'s boundary list in the commit that adds them. Neither is
  ever named by `alo-renderer`, `alo-paint` or the engine: the window is the
  browser process's (ADR 0005), and the engine stays something a test can run
  with no window at all.
- **The crate is `alo-window`**, and its binary `alo` is the browser a person
  starts. It holds the browser process's window and nothing of a page: it
  owns a `Tabs`, which owns the renderers.
- **One window first.** Several windows, and a tab dragged between them, are
  a later item opened by a person wanting one, not by this decision.
- **macOS first**, because it is the machine this is built and verified on,
  and `winit` does not make the others a different design: Linux waits on its
  sandbox (item 169) before a person should run pages on it at all, and
  Windows has no sandbox item yet. The code is not written to exclude them.

## 2. The window is composed, and never waits on a renderer

`winit` runs its event loop on the **main thread** (macOS requires it), and a
`Tabs` exchange with a renderer can take up to `LONGEST_SILENCE`. A window that
froze for that long when one site stopped answering would make ADR 0005's
*"every other tab is untouched"* false at the level a person sees.

- **The event loop never calls a renderer.** Every exchange with a renderer
  happens on a thread of the browser process that owns the `Tabs` — the
  *conductor* — which is sent what the person did, as typed messages, and
  posts what it learnt back to the event loop through `winit`'s own proxy.
- **The window shows what it was last sent.** A frame, once it arrives, is
  kept; a resize shows the old frame at its old size, against the window's
  background, until the new one arrives. A renderer that never answers leaves
  its last frame and, at `LONGEST_SILENCE`, the sentence `tab.rs` already
  says. Nothing is drawn as though it were current that is not.
- **Composition is a function**: given the window's size, the tab strip's
  frame, the selected tab's frame (or its sentence, if it is gone) and the
  window's colours, it returns the window's pixels. It lives in its own file,
  takes no window and no thread, and is **tested like every other picture
  here: a reference render**, so a change that moves a pixel of the window's
  layout says so. `present.rs` only copies those pixels into `softbuffer`'s
  buffer.
- **What a test cannot see**, said once: that the window server put the
  pixels on a screen. That is checked by **starting `alo` on this machine and
  capturing its window with the platform's own screen capture** — macOS's
  `screencapture -l` — and comparing it to the composed reference, recorded in
  the journal by the iteration that builds it. It is not deferred to "needs
  hardware": the hardware is the machine the loop runs on.

## 3. Device pixels are the renderer's, and arrive as their own item

A window has a **scale factor** — 2 on most Macs — and a frame painted at CSS
pixels shown at device pixels is either a quarter of the window or blurred.

- **The right answer is the renderer's**: lay out in CSS pixels, paint at the
  scale factor, report `devicePixelRatio` and the `resolution` media feature
  from it. That changes `alo-paint`'s raster, the frame's size and the
  reference renders' meaning, so it is **an item of its own**, with reference
  renders at 1 and 2.
- **Until it lands, the window replicates each pixel** to the integer scale
  factor, so geometry is right and text is coarse, and the window says nothing
  it does not know. A fractional factor is rounded down and the remainder is
  background. This is a stated cost, not an approximation of a feature: no
  page is told a ratio, and `devicePixelRatio` stays absent (ADR 0013 § 3).

## 4. The tab strip is a document, rendered by the engine

ADR 0002 says an agent reads what an interface *is*. The browser's own
interface is an interface: *"tab strip, four tabs, the second selected, titled
'Invoices'"* is exactly the sentence the agent should be able to read, and a
tab strip drawn by hand with `tiny-skia` in the browser process would be a
picture nobody can read — the screenshot ADR 0002 exists to refuse, in our own
interface.

So the tab strip is **markup and a stylesheet we ship**, rendered by **this
engine** — the stage 1 promise, *"it renders alo"*, applied to the browser
itself.

- **It is rendered in a renderer of its own**, under the same sandbox profile
  as any other (ADR 0010), at an internal site no page can name or navigate
  to. Not in the browser process, because **a page's title is a stranger's
  string** — a tab strip shows every open page's — and ADR 0005 keeps a
  stranger's bytes out of the privileged process, text shaping included. Not
  in a page's own renderer, because the strip shows every site's titles and a
  renderer holds one site's.
- **It is built from data, never from markup the browser process writes.** The
  browser process sends the strip's renderer its state — the tabs, in order,
  each with its title, its address and whether it is selected, loading or
  gone — as a typed message. The strip's renderer builds its document with
  `alo-dom`'s own operations from a template it was compiled with, and a title
  goes in **as a text node**, so there is no string a page can choose that
  becomes an element. No markup is ever assembled by concatenation.
- **It runs no script.** Stage 1 needed none for alo's own interface and this
  needs none either; what a click on it means is § 5's.
- **It is a frame like any other**, composed at the top of the window (§ 2),
  and its renderer dying is a strip that keeps its last frame and says so, as
  a tab does.
- **The agent reads it** through the same tree as a page, under the same rules,
  so *"which tabs are open"* is a reading and *"select the tab titled
  'Invoices'"* is a verb on a named node. Who may do that is item 133's grants;
  until they exist the agent's verbs on the strip are refused by name, and
  only reading is offered.

## 5. A person's pointer is a coordinate, and only a person's

ADR 0002 forbids a coordinate in an **agent's** verb. A person's pointer is a
coordinate by nature, and ADR 0018 already decided it goes through § 3's
browser driver unchanged once it arrives. This decides how it arrives.

- **The browser process decides which frame a point is in** — the strip or the
  selected tab — from its own composition (§ 2), and sends the point, in that
  frame's CSS pixels, to that frame's renderer as **a person's pointer event**.
  It never sends one on an agent's behalf, and no message an agent can cause
  carries a point.
- **The renderer hit-tests it against its own layout** — the layout tree is
  where both geometry and identity are — and dispatches the events ADR 0018's
  browser driver dispatches, so a click on a page is the same click a test
  drives today. Hit-testing is the renderer's because it is the only process
  holding the boxes.
- **What a click on the strip means is claimed and decided**, ADR 0020's
  shape: the strip's renderer answers with a claim — *select tab 4*, *close tab
  4*, *open a new tab* — and the browser process acts only on a claim about a
  tab it holds, answering a pointer event it sent. A strip renderer that lies
  can at worst choose among a person's own tabs, at the moment the person
  clicked; it cannot open a page or name an address.
- **The keyboard** reaches the selected tab's renderer as its own events when
  focus is item 258's to decide, and the browser's own keys — a new tab, close
  this one, the next one — are the browser process's, read before any page
  sees them, so no page can take them.

## 6. What is a tab, and what opens one

- **A tab is `alo-renderer`'s `Tab`**, unchanged: an address, a site, the last
  frame, and what happened. `Tabs` gains a **selected** tab and an order, both
  the browser process's, and both part of the state § 4 sends.
- **A person opens a tab** with the strip's control or the browser's key, and
  it opens empty (`about:blank`) until the address bar exists (item 119).
- **A page opens a tab** only by `target="_blank"` on a link a person activated
  — ADR 0020's ask, lifted from refused to decided for that one target. A
  script's `window.open` and a named target stay refused by name: a popup
  policy is a decision about a person's attention and gets its own record when
  a page needs it.
- **Closing the last tab closes the window**, and closing the window closes
  every tab, which item 64's lifecycle already turns into renderers reaped.

## What this costs

- **Two more rented crates in the privileged process**, with the window
  server's `unsafe` inside them. It is the platform's surface, and there is no
  window without crossing it.
- **A renderer process for the browser's own interface**, which is memory a
  hand-drawn strip would not need. It is the price of the strip being readable
  by the agent and of a stranger's title never being shaped in the browser
  process.
- **A thread in the browser process**, and messages between it and the event
  loop where a direct call would have been simpler. It is what keeps one silent
  site from freezing the window.
- **Coarse text on a high-density screen** until § 3's item lands.

## Alternatives rejected

- **The tab strip drawn by hand in the browser process** — a picture the agent
  cannot read, and a stranger's title shaped in the privileged process (§ 4).
- **The strip built from markup the browser process writes** — the first
  title containing `<` is the first injection (§ 4).
- **The strip in the selected page's renderer** — one site's process holding
  every site's titles; ADR 0005.
- **The event loop asking renderers directly** — one silent site freezes every
  tab (§ 2).
- **A GPU surface now** — item 116, after the software path is right.
- **Our own window code over AppKit** — `unsafe` of ours, law 4.
- **A web view** — another engine; ADR 0001.
- **Hit-testing in the browser process** — it would need the boxes, which are
  the renderer's, and a second copy of geometry would drift from the first.

## What this does not decide

- **The address bar** (item 119), which is the strip's neighbour and needs text
  input, carets and input methods (101, 102) and a search that phones nobody.
- **Focus and keyboard events inside a page** — item 258.
- **Scrolling** — a wheel event is a person's input under § 5, and what it
  scrolls is the renderer's; compositing layers and scrolling that does not
  repaint are items 114 and 115.
- **A popup policy, several windows, and dragging a tab** — each with the page
  or the person that needs it.
- **The agent's grants over the strip** — item 133.
- **What the strip looks like.** Its markup and stylesheet are ordinary changes
  with reference renders; alo's own design is the reference.

## How we will know if this was wrong

**If one site's renderer stopping makes the window stop**, § 2's conductor
leaks a wait into the event loop, and that is a defect to fix there.

**If the agent cannot answer "which tabs are open" from the tree**, § 4 has
been bypassed, and the strip has become a picture.

**If a page's title ever appears in the strip as anything but text**, § 4's
"built from data" has a hole, and it is a security bug, not a rendering one.

**If `winit` or `softbuffer` turn out to need `unsafe` of ours**, § 1's
premise is false, and the rental is reopened rather than the `unsafe` written.
