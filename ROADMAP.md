# alo browser — ROADMAP.md

The only order things get built in. **A tick means done, not written** — where
something is built and tested but has not met the gate on a real screen, the item
says so rather than being ticked optimistically.

Every item appears in `docs/features.md`. If it is not there, it is not built.

For the remaining-work audit and executable continuation loop, see
[`docs/autonomy/REMAINING.md`](docs/autonomy/REMAINING.md). The loop follows
these gates; running it does not certify them.

## Three states, because two are not enough

A queue item is usually smaller than a line here, so a line is routinely *part
built* — and a box that can only be empty or ticked has no way to say that. It
said the wrong thing instead: fifteen of twenty-eight consecutive commits left
this file untouched, and stage 2's first line read as unstarted after its ADR
was accepted and its message boundary was built and tested.

So a line is written in one of three states, and there is no fourth:

- `- [ ]` **Not started.** Nothing exists.
- `- [ ]` **· Built: … · Owed: …** — part done. The clause names what exists
  *and* what is still missing, both concretely enough to check: a crate, a queue
  item, a named capability. It stays an empty box, because half a line is not a
  line.
- `- [x]` **Done** — the whole line, gated, with any remainder stated in the
  item's own words.

**The clause is a claim, so it names something.** A Built clause that cannot
name a crate or a landed capability is decoration, and decoration is how a
roadmap starts lying slowly.

---

## Where it runs

**For every operating system.** alo OS is the first that asked, not the one
this is for.

This file said nothing about where it runs until 2026-10-12, and then said it
wrongly twice in one day — first *destined for alo OS*, then *built on macOS* —
and the owner corrected both. Each put a **machine** where a **purpose**
belongs. It happens to be developed and verified on a Mac today; that is where
the work is being done, not what the work is, and other machines will join.

The difference is not pedantry. **The agent surface is ours** — a page read as
a tree, actions by name, no screenshot and no coordinate (ADR 0002) — and it
belongs on macOS, Windows and Linux alike rather than to whichever system
integrates it first. So it is built as our own interface, documented, and an
operating system's adapter *wraps* it.

That ordering matters because of what it un-blocks. alo OS's own letter named
three gaps on their side — no argument kind for a web address, no kind for
model-written text containing a slash, and no grant narrower than a whole
application. Written the other way round, those gaps block this browser.
Written this way they block one wrapper around it, and everything else here
proceeds.

alo OS is an operating system in the sibling repository `alo-os`: an unmodified
Linux kernel, a native shell, Wayland through Smithay, x86-64. Its premise is
that an agent reaches the machine through one enumerated list of verbs, each
proposed before it runs and recorded after — which is the same claim as law 2
of `CLAUDE.md`, made about a machine rather than a page.

Its `crates/alo-software/shipped.toml` pins a web browser for
`Role::WebBrowser`, and today that is Firefox 156.0 from Flathub. Its roadmap
gives the reason in its own words: *"a pinned upstream one, since our own
engine is not scheduled."*

**That clause is what changed.** The engine is scheduled. Firefox remains the
right pin until this browser meets stage 2's exit gate, and nothing here asks
for the slot before then — a pinned upstream browser that works beats a
sovereign one that does not, and the gate exists to stop us pretending
otherwise.

**Running on Linux is stage 2 work, not a stage 4 port.** The distinction is
load-bearing. Three files carry `cfg(target_os = "macos")` — `sandbox.rs`,
`fonts.rs`, `generic.rs` — and the window is already rented from `winit` and
`softbuffer`, both of which speak Wayland. The one real gap is the sandbox:
ADR 0010 already decided Linux's — *seccomp-bpf, a user namespace and
Landlock* — and queue item 169 is where it gets built. Until 169 lands, this
browser cannot run anywhere but macOS, so 169 is the gate on everything
alo-OS-shaped rather than one item among many.

**Being installed on alo OS is stage 4 work**, and theirs as much as ours: one
application per role, so a pin for this browser is a pin removed from another,
and that is their decision made under their gates. What is ours is being
installable the way any application is — from a source, through the rented
tool, arriving granted nothing.

**One question is open and could change a decision rather than a detail.**
ADR 0010 makes it fatal for a renderer to fail to obtain a sandbox. Whether a
Flatpak can host a nested user namespace and a seccomp filter is not something
this repository can answer, and if it cannot, ADR 0010 is superseded for that
platform rather than quietly bent. It is asked of alo OS in writing before 169
is built.

---

## The two paths

*ADR 0059, 24 October 2026.* There are now **two implementations of one
boundary**, and this file described only one of them until today.

ADR 0005 put a typed message boundary between the browser process and whatever
renders a page. ADR 0058 put the machine's own Chrome on the far side of it,
driven over a pipe. So the same tree, the same verbs by name, the same grants
and the same record now have two things underneath them:

| | **The product path** | **The engine path** |
|---|---|---|
| What renders | the machine's own Chrome, over a pipe | `alo-render`, ours, in Rust |
| What is ours | the tree, the verbs, the grants, the record, the window | all of it |
| Reaches a browser a person can use in | weeks | years |
| What it proves | that reading an interface beats photographing it | that a browser need not be written in a language that permits the bug |
| Its gate | ADR 0052's two gates — the agent's and the person's | stage 2's exit gate, below, unchanged |
| Where its work is listed | `QUEUE.md`, *The product path* | the stages below |

Neither is a fallback for the other, and neither is a stage. The product path
answers *can a person use this*. The engine path answers *can this be written
safely* — which is the argument for having started, and which Chrome does not
answer and never will.

**The stage numbers below describe the engine.** They always have. Stage 2 is
not "when the browser works"; it is when **our own renderer** renders the
modern web. What the product can do while a stage is open is stated under that
stage, in its own words. The numbers were deliberately not reassigned: `stage
2` names a body of work that a hundred queue items close against, and handing
that name to different work is the mistake ADR 0003 forbids (ADR 0059 § 1).

**A claim names its renderer.** "alo browser renders the open web" is true on
one path and false on the other, and the two will be in different places for
years. A statement that does not say which path it is about is not a statement
about this project (ADR 0058 § 6).

### Where the product path stands today

- **Built:** the window (296); the agent socket, a Unix socket and never a port
  (496); a grant per site, refused by us and recorded (497); the tree, the
  verbs and the refusals, driven on a real external page (131); Chrome spawned
  and spoken to over a pipe (619); a frozen page opened in Chrome and its tree
  read as ours, agreeing with the engine's on every role, name and state they
  both carry (620); the verbs on a page in Chrome, decided by the engine's own
  rules before Chrome is asked anything and carried out as a real click or
  real typing, httpbin's order coming to the same outcomes through both
  renderers (621); a page from the network opened in Chrome, every frame and
  worker of its tab held until its requests are paused and every request
  judged by our rules before Chrome is let send it — none of a local page's,
  its other-site frame's, its worker's or its service worker's reached the
  server unseen — and an agent's off-site address refused before a single
  command is written (633); every request of such a page in the record,
  each hop of a navigation a line and a WebSocket a line saying it was not
  decided, with the cause this process gave it — the person's first page,
  the page's own requests, and an agent's `activate` named on the
  navigation it caused (634); a page's shared worker — which Chrome tells
  only its own session of — held there, so its `fetch`, its imported script
  and what it asks for as a page connects are each paused before a local
  server sees them, and its WebSocket told of and recorded (637, which
  closes 629); the agent's socket reaching a tab in Chrome through the same
  serving and the same grants as the engine's, the same eight lines filling
  httpbin's form there, every refusal the same kind, the record naming
  Chrome and its version, and the tab named by where its page is now (630),
  a link to a part of the page and a script's `pushState` included, the link
  answered `Followed` as the engine answers it (640; the engine's tab, which
  follows no link until item 85, is still named where it was opened); and
  the program a person starts told to use Chrome — `alo --chrome` with an
  address or a page's file, saying on the terminal which Chrome draws it by
  the version it gives itself, answering `tabs`, `read` and `act` over the
  socket from another process, and closing Chrome when its input ends
  (641); and **the page in a window a person can see** — Chrome started
  shown in `--app` mode, its window sized so the page is laid out at the
  size asked and placed on the screen by this process, answered by where it
  went and brought in front, `alo --chrome` saying on the terminal where in
  numbers (627, `chrome_window.rs`) — and a window the screen gives less
  room than asked used at what it was given and said so, rather than
  refused, so the browser starts on a small or locked screen and an agent
  still reaches it (671); and **where a frame of ours stands around
  Chrome's window** — directly above it, as wide as it is, by Chrome's
  word, and only while the window server shows Chrome's window at exactly
  those bounds; aside while it is in Stage Manager's strip, minimised or
  behind another application; gone with it when it closes — read through
  `osascript`, since the crate's reading needs `unsafe` of ours, and
  asserted in the window server's numbers (672, `chrome_frame.rs`,
  `window_server.rs`); and **a window of ours standing there** — a
  floating, borderless strip `alo --chrome` runs, made without taking the
  screen from Chrome, following Chrome's window as it is moved and
  resized, hidden while it is not on the screen, closing when it closes
  and closing it when ours is closed or the program's input ends,
  asserted in the window server's numbers on the real program (642,
  `chrome_follow.rs`, `window.rs`'s `frame`, gated 2026-10-26); and
  **what that window says** — which Chrome draws the page, in the one
  wording the socket's `renderer` answer has, and then each verb's
  sentence and each refusal, the line the conductor's own rule makes of
  the answer this process sent, side by side in the band, oldest first,
  as long as the engine's window keeps them (679, `frame_place.rs`,
  `compose_frame`, `frame_told.rs`, gated 2026-10-26 once 682 was
  closed); and **the page's tab in that window**, first in the band —
  what Chrome calls the page, read from its list of targets in the frame's
  own cycle and told only as it changes, drawn by a tab strip's renderer
  of this engine's own on a thread that is never waited on, so a title
  holding markup is its characters and is never shaped in this process,
  asserted against a shown Chrome whose page retitles itself and in two
  reference renders (681, `chrome_named.rs`, `frame_strip.rs`); and
  **the running browser asked which
  renderer draws a tab** — the socket's `renderer` ask, sent by a person
  with `alo-ask`, answered by the browser process and never the page: this
  engine, or Chrome by the version it gives itself, kept to the sites as a
  read is and recorded (622); and **the agent's sentence where a person
  can read it**, in this engine's window — each verb's `said`, word for
  word, in a line of the browser's own, which no page can read or forge
  (623; through Chrome in the frame above Chrome's window, 679); and **the box
  an agent acted on, ringed** on the frame painted after the verb — the
  node's own rectangles read from the same layout, drawn as alo draws focus
  (624; through Chrome owed as 680, decided in ADR 0066 as Chrome's own highlight); and **a refusal where a person can read it**,
  in this engine's window — each verb refused leaves its own sentence, the
  refusal's `said`, in a line of the browser's own in its own colours,
  kept until the second verb that runs after it, two refusals in a row two
  lines (625; through Chrome in the frame, 679); and **a form Chrome would not send,
  answered as the engine answers it** — `NotSubmitted`, naming the control
  Chrome stopped at and the words of the bubble it draws there, read from
  its tree, which a page cannot forge (631; the first control and no
  constraint, where the engine names each and its constraint); and **a
  date put in through Chrome** — a segment at a time, in the order Chrome
  lays them out, read from its tree rather than guessed from a locale, so
  a form sends the dates it sends through the engine (632); and **each
  frame a document of its own in the record**, caused by the page holding
  it, so a frame's `fetch` leads back through the frame and the page to
  the person (636); and **what a page's script said while a verb ran,
  through Chrome** — its console and its uncaught throws, heard only while
  the verb runs and put into the engine's words, and a line longer than
  Chrome's messages may be said to be so without ending the pipe (639),
  a frame from another site and a dedicated worker heard as the page is,
  each in a target of its own (652), and a shared and a service worker
  too, whose every page is the tab's (654);
  and **a frame sent to a scheme Chrome never pauses, refused** — by
  itself from either site or by the page, judged as Chrome tells of it and
  recorded against the frame's document (651); and **a tab an agent asked
  to open, the agent's in the record** — its first page caused by the
  agent's action done in that tab, what the page asks for leading back to
  that action and to no person, as ADR 0063 decides (638); and **a service
  worker that stops and starts again, held again** — stopped, never waited
  on or heard by a verb, and started again in the target it had, asked
  what it was asked at its first start and let go, so the page's request
  through it goes on (655); and **what Chrome says, held to a bound in
  bytes** — 32 MiB at most, beside the 256 messages, waiting to be read,
  so a page talking faster than its tab reads is held back in Chrome's
  pipe rather than in the browser's memory (653); and **a form Chrome
  refuses and draws no bubble for, answered `NotSubmitted`** — each failing
  control it cannot give the keyboard, hidden or inert, read from what
  Chrome logs of it with the control itself, in Chrome's words and with no
  box an agent could not name, beside the bubble's or alone; a cancelled
  `invalid` answers `Activated`, since Chrome tells of it as of a cancelled
  `submit` (646, ADR 0051's second amendment of 25 October); and **a
  page's request its own when Chrome pauses it before telling of the
  page** — a request paused while its frame goes to a new document let
  go at once and its line kept until Chrome names the loader that made
  it, the arriving document's making it that document's, and the frame's
  renderer asked after the commit to answer after everything it told of,
  so what it never told of stays the old page's, a frame's documents
  as the page's (647 and 656, ADR 0060's amendment of 25 October); and
  **an old page's request its own when Chrome pauses it after telling of
  the new page** — a beacon or a `keepalive` fetch from `pagehide`, which
  Chrome names no request id for as it names none for any request of a
  page being left, written as the page the frame left until its next
  navigation (657, ADR 0060's second amendment); and **text on a
  `<select>` refused by name** — `Chooses`, on this engine's page and
  through Chrome by the same rule, where the engine once answered `TextPut`
  and changed nothing its form sends (648, ADR 0064 § 4); and **an option
  chosen by activating it on this engine's page** — a list's options
  nodes of the agent's tree under it with no rectangle, `Activate` on one
  choosing it or toggling it in a list of several, the page hearing
  `input` then `change`, and the answer `Chosen`, read from the page after
  its listeners ran; the form sends it (659, ADR 0064 §§ 1–3 and 5); and
  **an option chosen through a list Chrome draws** — a list with a `size`
  or `multiple` clicked at the option's middle, with ⌘ held in a list of
  several so the click toggles rather than choosing it alone, the answer
  `Chosen` read back from Chrome's tree, and a closed list's option not
  pressed at all and answered `NotCarriedOut` by name (660, ADR 0064 § 6).
- **Decided:** whose network a page in Chrome uses — Chrome's own, with
  every request held and judged by the browser process before it leaves and
  recorded here, an agent's off-site address refused before Chrome hears of
  it, and the network promises that do not hold there named (ADR 0060);
  what a person looks at — Chrome's own window, with ours around it, not
  frames copied into ours (ADR 0061); and the window of ours around it —
  floating, since an ordinary one and Chrome's are never on the screen
  together under Stage Manager, put by Chrome's word and shown only while
  the window server shows Chrome's (ADR 0062); and whose a tab an agent
  asks to open is — the agent's, an action done in that tab rather than in
  a document, so its first page is never written as the person's, and
  still one of ADR 0012's three causes (ADR 0063); and how an agent
  chooses an option of a `<select>` — by activating the option, which in
  a list of several toggles it, answered by what the list holds after;
  a list takes no text; and through Chrome a closed list's option is
  refused by name, since every key that reaches it chooses options nobody
  chose on the way (ADR 0064).
- **Owed:** a closed list's option through a shown Chrome, measured with
  the screen in use (658); the
  rest of the agent's work where a person can see it — **the ring around
  the box a verb acted on through Chrome, decided in ADR 0066 — Chrome's
  own `Overlay.highlightNode` on the node, with its label, and no window
  of ours over the page — and unbuilt (680)**; words drawn with this machine's own fonts, which the engine
  draws none of in its window or the frame's tab while the frozen fonts
  draw them (683); a placed window
  that stays where a
  person sees it, which Stage Manager's strip does not promise (643); an
  application somebody can run (477).

642 is closed: a frame of ours above Chrome's window that follows it,
stands aside when it is not on the screen and closes with it was built on
2026-10-26 (iteration 419) and gated that day once the machine was
restarted — ADR 0061's riskiest step, without its decision being sent
back. And the frame says something (679, gated once 682 was closed):
which Chrome draws the page, and what each agent's verb was answered with
or why it was refused, told by the thread holding Chrome from the answers
it sent; and it shows the page's tab, drawn outside this process (681).
What it does not show is the box an agent acted on, marked over Chrome's
page (680, decided in ADR 0066 and not yet built); and run on this machine, the tab draws no words, as this
engine's own window draws none with the machine's fonts (683).

---

## Stage 1 — it renders alo

No scripting, no hostile pages, no compatibility burden. The point is that
something real depends on this within months rather than years: when alo's own
screens render through it, every later stage has a working thing to grow from
instead of a demo.

**This stage needs no hardware and no operating system.** Not the certified
machine, not alo OS's compositor, not a GPU, a window or a network. HTML and CSS
in, a PNG and a box tree out — both files — so every item is built *and
verified* on an ordinary laptop by anybody who clones this repository. That is
deliberate, and it is why the browser is not waiting on the OS and the OS is not
waiting on the browser.

**What "an alo screen" means.** An alo screen is markup and custom properties.
`alo-workplace`'s screens are checked out beside this repository and its
`tokens.css` is the colour specification, so "correct" is diffable **today**.
An alo screen is alo's whichever repository it lives in; the gate below used to
name `alo-os`'s specifically, which made a finished item unclosable and is
corrected.

**Order.** A software raster to a PNG is deterministic and diffable, which makes
every step testable from the first one — hardware acceleration comes after
correctness, not before it.

- [x] **A DOM of our own**, built from `html5ever`'s parse events
- [x] **Stylesheets**: `cssparser` into rules we hold, selectors matched with `selectors`
- [x] **Computed style**: the cascade, inheritance, and `var()` — alo's design system is custom properties throughout, so this is not optional decoration
- [x] **The box tree**, and what each box *means* (ADR 0002) rather than only its rectangle
- [x] **Layout**: flexbox and grid, on `taffy` to begin with, behind our own boundary
      *The `flex` shorthand was read by nothing until queue item 440
      (iteration 281), so `flex: 1` grew nothing; `alo-css`' `flex.rs` now
      splits it into its three longhands, measured against Chrome 154.*
      *Boxes were laid out in exact floats, where Chrome 154 lays them out
      on a grid of 1/64 px, so paddings, margins and tracks differed from
      Chrome's by up to a unit each. The rule was decided in iteration 303
      (queue item 498, ADR 0054). · Built (iteration 305, queue item 466):
      `alo-layout`'s `layout_unit.rs`. Lengths, percentages and `calc()`
      are truncated toward zero, text widths are rounded up (by Chrome's
      text item, one text box's text on one line, since item 574,
      iteration 350: `text_run.rs`, `text-on-the-grid`; 950 of 1,267
      corpus boxes exact at one where 899 were), and every
      box's edges are placed on the grid against where its parent was
      placed. 707 of the corpus's 1,260 element boxes are now Chrome's
      exactly, where 514 were, and `alo-sites-booking`'s all are except
      what its date field's line moves (507, since built: all are). Those counts were taken
      with Chrome in quirks mode for the cases with no `</head>`; in
      standards mode they were 807, and after 467 they are 855. Decided (iteration 306,
      queue item 499, ADR 0056): this engine does not take the sharing of
      flexible space over from `taffy`; a hook on `taffy`'s tree trait,
      offered upstream by a person, puts its shares on the grid. A
      throwaway prototype of it, with upstream's unreleased fix to its
      re-run check, made the mini-month's columns Chrome's.
      · Owed: what `taffy` decides inside its own algorithms. That is a
      grid's second column pass, so the mini-month's columns are still
      1,837 and 1,838 units where Chrome's are all 1,838. It is also
      flexible items' exact shares and a grid's `justify-content` after
      its first share. They wait for the hook to be offered (501) and
      released, then built on (502); flex `justify-content` needs
      measuring first (503). Measured (iteration 336, item 564): how
      Chrome 154 shares a flex line, shrinking and growing, at one and at
      two — 5,120 lines, a rule that gives every one (ADR 0054's addendum
      of 2026-10-13), 762 frozen and checked by a test that pins this
      engine within two units of each. Owed beside it: `taffy` takes a
      line's gaps away twice when its factors sum below one, a whole pixel
      and more on a measured line, which a person has to report upstream
      (572).*
      *A picture with both a width and a height set was laid out at least
      its width over its ratio tall — 30 by 30 for a square at 30 by 20 —
      and a `max-height` beside a set width narrowed it, until queue item
      590 (iteration 355): `taffy` was handed a picture's own ratio and
      applies it past what the style fixed. · Built: a replaced box's size
      is `alo-layout`'s `replaced.rs` (`used_size`, CSS 2 § 10.4's limits)
      and `taffy` is handed only an author's `aspect-ratio`;
      `pictures-sized-on-a-line` is Chrome's in all 35 boxes at one and at
      two, on a line, as a block, as a flex item and positioned, and
      `a-turned-picture`'s flex row now shrinks as Chrome's does.*
- [x] **Text**: HarfBuzz shaping (via `rustybuzz`), the fallback chain and line breaking, with the awkward scripts working before the easy ones. Rasterisation is queue item 17, beside paint
      *`text-transform` was read by nothing until queue item 441
      (iteration 283, ADR 0049): uppercased headings were drawn, measured
      and read as written. `uppercase` and `lowercase` are now applied as
      a text box is built, by Unicode's language-sensitive mapping rented
      from ICU4X, measured against Chrome 154 on alo Agenda's lists.*
      *`text-overflow` was read by nothing until queue item 442
      (iteration 284): a name too long for its row was cut mid-letter at
      the box's edge. A line that overflows a clipping box now ends in "…"
      by the rules measured in Chrome 154, drawn only, with every box and
      line where it was and the whole text read by an agent.*
      *Line breaking broke a `nowrap` line before an `inline-block` that
      did not fit until queue item 447 (iteration 285). A line now breaks
      before an atomic box only where the space before it or the box
      holding both allows it, measured row by row against Chrome 154.
      *Text broke by its own `white-space` wherever two boxes met until
      queue item 449 (iteration 286): a `nowrap` span after a space that
      wraps stayed on its line, and `abc<span>def</span>` broke between
      the "c" and the "d". A break where two boxes meet is now decided by
      the space before it or the box before it, between two letters there
      is none, and a no-break space is no longer collapsed as white space,
      each rule measured against Chrome 154. Owed: moving a word that
      spans two boxes back to an earlier break on its line (452), 450 and
      451.*
      *Lines were built from a font's exact ascent and descent, where
      Chrome 154's are whole pixels, so rows drifted up to 2 px from
      Chrome's over a short page. The rule was decided in iteration 292
      (queue item 464, ADR 0050) and built in iteration 293 (460): ascent,
      descent and line gap each rounded half up, a set line height on a
      1/64 px grid, and the leading's top half floored. Of 1,214 corpus
      boxes measured against Chrome with the same fonts, 835 have its top
      and height to 1/64 px, where 498 did. Paddings, margins and grid
      tracks were put on the 1/64 grid by 466 (see *Layout*). A
      control's own line was 1.2 em until queue item 467 (iteration 308)
      and is now the line its text would be, so alo Settings' boxes are
      all Chrome's. Owed: a fallback font's own room on a `normal` line
      (465).*
      *A form control took the page's font until queue item 506
      (iteration 310), where Chrome 154 sets every control in 13.33 px, in
      Arial or, for a textarea and a date or time field, `monospace`. Both
      are now Chrome's, and `font` as a CSS-wide keyword alone is split
      into its longhands, so an alo screen's `font: inherit` still takes
      the page's font back. `web-a-form`'s fields are Chrome's 21, and of
      1,261 corpus boxes 886 are Chrome's to 1/64 px, where 865 were, with
      none further. Owed: the rest of a control's text reset (512) and the
      `font` shorthand's own grammar (513). A unit of text width at a
      fractional size was 514's, built in iteration 317.*
      *A face was set at its font size as written until queue item 514
      (iteration 317), where Chrome 154 sets it at the size cut to
      hundredths in a 32-bit float: 13.333333 px is 13.33, and 8.11 is 8.1.
      Built (`alo-text`'s `face_size.rs`, read where a face is scaled), so a
      control's text, its line, `ch`, `ex` and a field's width are Chrome's:
      `web-a-form`'s button is Chrome's 6,597 units where it was 6,598, and
      2,916 of 2,922 runs measured at fractional sizes are Chrome's width.
      Owed: six runs whose sums end within 0.03 of a unit of a grid line,
      on the other side of it from Chrome's (527).*
      *A text field was `20ch` until queue item 511 (iteration 315), where
      Chrome 154 makes it its `size` in characters of the face's `x`, plus
      its ascent less one: measured over 3,000 fields and confirmed on 288
      more. Built (`alo-box`'s `field_size.rs`, `alo-layout`'s
      `field_width.rs`, `MeasureText::x_advance`), so `web-a-form`'s fields
      and labels are Chrome's and `a-text-field`'s nine boxes are Chrome's to
      the unit. Owed: a face with no `x` (526), and a date or time field's
      and a `<select>`'s own widths (517, 521), which keep `20ch`.*
      *A date or time field was a text field's height until queue item
      507 (iteration 311), where Chrome 154 holds its line in segments a
      pixel inside it above and below, and pads it `0 0 0 1px`. Both are
      built (`alo-box`'s `in_segments`, `alo-layout`'s `SEGMENT_EDGE`, the
      user-agent sheet), so `alo-sites-booking`'s every box is Chrome's, and
      of 1,261 corpus boxes 892 are Chrome's to 1/64 px, where 886 were,
      with none further. Owed: the calendar indicator's height as a floor
      (515, needs design), a text field's line never below `normal` (516),
      and a date or time field's own width (517).*
- [x] **Paint**: a display list, then a software raster to a PNG
      *A small curve was under-covered and leaned to one side until queue
      item 459 (iteration 291): the rented rasteriser's four-by-four
      sampling drew a radio's dot 159 on one edge and 191 on the other. A
      shape's edge pixels are now sampled sixteen by sixteen, within 3 of
      255 of their exact area, and since queue item 462 (iteration 339)
      letters are too: against Chrome 154's screenshots at 12, 13 and 16
      px every line is nearer, within a measured bound
      (`letters-at-three-sizes`). Owed: Chrome's macOS thickening of
      letters (576, a decision) and its letter tops on whole pixels (577).
      Since queue
      item 565 (iteration 337) a box's background, border and shadows are
      drawn from its box on whole device pixels, as Chrome 154 draws them
      at one and at two, and since queue item 461 (iteration 338)
      `checkbox-beside-text` is Chrome's screenshot exactly in all 49,525
      pixels outside its controls and text. Since queue item 575
      (iteration 351) an overflow clip is on the same whole device pixels,
      2,016 of 2,016 clips measured (`clips-on-device-pixels`), and
      since queue item 587 (iteration 352) a picture is drawn into its
      content box on whole device pixels, 1,568 of 1,568 pictures
      measured (`pictures-on-device-pixels`), and since queue item 588
      (iteration 353) a rounded clip inside a border follows its inner
      curve, 60 of 60 clips measured (`rounded-clips-inside-borders`),
      and since queue item 589 (iteration 354) an `<svg>` is clipped and
      drawn on whole device pixels, 1,792 of 1,792 `<svg>`s and 672 of 672
      SVG pictures measured (`svgs-on-device-pixels`), and since queue
      item 592 (iteration 356) a solid border of one colour is one ring
      and any other border's sides are split on the mitre and fill a
      rounded corner to its inner curve, as Chrome 154 draws them
      (`borders-side-by-side`, every plain pixel Chrome's at one and at
      two), and since queue item 593 (iteration 357) a page at a
      fractional scale is laid out and drawn as Chrome does it at 1.5 —
      every rule of ADR 0057 § 2 counted in sixty-fourths of a device
      pixel, which no float holds there (975 of 985 corpus boxes, 904 of
      904 probe numbers, six cases' pictures in every plain pixel), and
      since queue item 597 (iteration 361) a checkbox and a radio are
      drawn in their snapped box cut to whole CSS pixels, as Chrome 154
      draws them (`alo-paint`'s `toggle::picture_box`;
      `toggles-at-fractional-scales` Chrome's in every plain pixel at one,
      1.5 and two, `checkbox-beside-text` at 1.5 but four pixels of
      letters), and since queue item 598 (iteration 362) a centred line's
      start and a `middle` raise are put on the layout grid as Chrome 154
      puts them (`alo-layout`'s `layout_unit::half_truncated` and
      `half_rounded`; `centred-and-middle-on-the-grid` Chrome's in every
      box and pixel at one, 1.5 and two, and the corpus at 1.5 Chrome's in
      every box exact at one but 564's two). Owed: the foot of a line's
      letters at 1.5 (607), an out-of-flow box's static position on an
      empty centred line (608), a button's `width` as its border box
      (609), an `outline`, which is not drawn at all (591). Since queue
      item 599 (iteration 363) the window takes a screen at 1.5 at its
      own scale rather than rounding it to one (`alo-window`'s
      `scale.rs`; `alo --scale 1.5` captured on screen is the window
      composed at 1.5 in every opaque pixel). Since queue item 596
      (iteration 360) a dashed or dotted side with a curve inside a corner
      is laid along the border's centre line as Chrome 154 lays it, the
      line measured as coarsely as Chrome measures it and each dash swept
      as wide as Chrome's stroke (`alo-paint`'s `contour.rs`,
      `centre_line.rs` and `sweep.rs`; `dashes-and-dots-round-a-corner`
      Chrome's in every plain pixel at one and at two but one pixel at
      each). Since queue item 615 (iteration 368) a line too short for
      two dashes and two gaps takes two of each, a third and a sixth of
      it, as Chrome 155 draws it (`centre_line.rs`' `fitted`;
      `dashes-round-a-short-line` Chrome's in every plain pixel at one and
      at two but 3 and 24 that 604 owns). Since queue item 617
      (iteration 369) a dashed line is fitted to its length truncated to
      whole device pixels, the leftover running on into the first dash,
      as Chrome 155 draws it (`centre_line.rs`' `whole_pixels`;
      `dashes-fitted-to-whole-pixels` Chrome's in every plain pixel at one
      and at two; `dashes-and-dots-round-a-corner` now so at one and but 2
      at two, `dashes-round-a-short-line` at one and but 19 at two, all on
      dash ends round curves tighter than their stroke). Owed: a stroke wider than its curve is tight (604), a
      centre line on half device pixels at one (605), Chrome's spacing on
      a straight side (606), a dotted ring too short for its dots drawn
      whole (616), and whether round dots are fitted to whole device
      pixels too (618). Since queue item 595 (iteration 359) a side
      styled `none` or `hidden`, or given no style, has no width however
      it was written (`alo-layout`'s `border_style.rs`,
      `borders-with-a-side-none` Chrome's in numbers and every plain pixel
      at one and at two). Since queue item 602 (iteration 365) a side
      given a style and no width is `medium`, 3px, a side shorthand naming
      no width resets it, and a width longhand takes `thin`, `medium` and
      `thick` (`alo-value`'s `parse_line_width`,
      `borders-with-no-width-written` Chrome's in numbers and every plain
      pixel at one and at two). Since queue item 603 (iteration 366) a
      side's shorthand takes out the longhands it beat in the cascade, as
      `border` already did, so `border-left` written after
      `border-left-style` overrides it (`alo-style`'s `cascade.rs`,
      `border-shorthands-after-longhands` Chrome's in numbers and every
      plain pixel at one and at two). Owed: a two-colour mitre's seam
      anti-aliased as Chrome's (518).
      Since queue item
      573 (iteration 349) a spread shadow's corners are Chrome's —
      square on a square box, a radius under the spread grown by less of
      it (`Corners::spread`, `spread-shadow-corners`). Owed: an inset
      shadow's corners from the padding box's radii (585).*
      *A button and a text field were a 1 px grey square border until queue
      item 505 (iteration 309), where Chrome 154 computes a 2 px `outset` or
      `inset` border and draws its own picture in that room: a one-pixel
      rounded edge over the control's fill. Both are now Chrome's, measured
      at eight device pixels to one, and a page that writes any border or
      background on the control gets its CSS instead, as in Chrome.
      `a-script-hears-an-event`'s button is Chrome's 22 and its edge is
      Chrome's screenshot's within 7 levels. A control whose page sets a
      background draws the sheet's border in Chrome's two tones (508,
      iteration 312): black's `outset` is 168 and 84, every side to the
      level of Chrome's screenshot. A disabled control's colours are
      Chrome's computed ones (509, iteration 313), its text's ink matching
      Chrome's screenshot to a level, and a page's `background` or
      `border` beats the sheet's longhands. A `<textarea>` draws a field's
      picture (510, iteration 314), its sides Chrome's screenshot's
      exactly in `web-a-form`. Owed: a mitre's diagonal pixel (518), a
      button-like `<input>`'s value (519), a fieldset's groove colour
      (520), a `<select>`'s picture and chevron (521), a resize grip
      (522), a textarea's `cols` and `rows` (523), an edge's inner corner
      pixel (524), and a restyled field's white (525).*
- [x] **Reference renders**: a committed corpus, diffed on every change
- [x] ★ **The agent tree**: the layout tree read as roles, states and positions
      *A name read from content now has its spaces where Chrome 154's
      accessibility tree has them: a flex or grid item, and an atomic box
      on a line, stands apart from its neighbour (queue item 530). A `<br>`
      and a picture drawn as its `alt` in a line still run together (534).*
- [x] ★ **Typed verbs**: activate, type, scroll — and never a coordinate.
      A verb **changes the page**: text put into a field is in it, a checkbox
      ticks, a radio un-chooses its group, and the page is rendered again from
      the same document so held ids stay valid. *This line was ticked when a
      verb decided and reported and changed nothing, which is not what the line
      says. Queue item 42 made it true.*
- [x] **A real alo screen renders correctly** — the sign-in screen, then
      Settings. Both are `alo-workplace`'s own markup and rules, rendered from
      their own stylesheets with **no substitutions**, and diffed on every run
      against a committed image *and* a committed box tree (queue items 44,
      47, 48, 49 and 45). *One thing is true of both and is not a defect in
      either: the corpus renders in DejaVu Sans and the app loads Inter, which
      is narrower, so alo's headline wraps one line more here. Web fonts are
      stage 2.*
      *Settings was diffed against a reference that was itself wrong in one
      way until queue item 440 (iteration 281): its `flex: 1` was ignored,
      so its content column was 115.5 px narrower than Chrome draws it and
      Cancel and Save stood at the footer's left. Its widths now agree with
      Chrome 154's to its layout unit, and the reference was rewritten.*

**Exit gate — met.** From this repository alone, on any machine: alo's sign-in
screen and Settings render correctly from their own markup and `tokens.css`,
with no substitutions left in the case — matched against the committed
reference image *and* the expected box tree, so "correctly" is a diff rather
than an opinion — and an agent reads the Settings screen as a tree and
activates a row by name rather than by position.

All three clauses hold. The two screens are `crates/alo-corpus/cases/`'s
`alo-sign-in` and `alo-settings`, and the agent is
`crates/alo-renderer/tests/an_agent_on_settings.rs`. Every one of them runs on
the machine anybody clones this on.

**What the gate does not claim.** That this is a browser — it is an engine, and
stage 2 is the rest. That the screens are pixel-identical to the app — the
corpus renders in DejaVu Sans and the app loads Inter, so alo's headline wraps
one line more here; web fonts are stage 2. And nothing about speed, which is
measured on hardware or not said.

Nothing in that sentence needs a compositor, a certified machine or another
repository. That is the point of the gate rather than an accident of it: a gate
nobody can reach is a gate that quietly stops being used, and this one had
already begun holding a finished item open.

### After stage 1, and not gating it

Real work, and the only items in the stage that depend on anything outside this
repository. They are kept out of the list above so they cannot block it.

- [ ] Hardware acceleration, once the software path is right — needs a GPU
- [ ] Embedding: alo OS's shell renders through it — needs alo OS's compositor,
      which does not exist yet. **The engine is finished for stage 1's purposes
      before this lands**; this is the OS adopting it, and is that repository's
      milestone as much as ours
- [ ] Rendering at speed on the certified machine, without stutter — needs both
      of the above, and is a performance claim: measured on hardware, or not made

---


## Stage 2 — it renders the modern web

The stage that turns **our own engine** into a browser. It is the largest of the
three by a wide margin: stage 1 refused scripting, hostile pages and
compatibility, and this is where all three arrive at once.

**What the product can do while this stage is open.** Everything the product
path reaches first, on Chrome's rendering: the open web, read as a tree and
driven by name, under grants and recorded. This stage is what makes that true a
second time with no C++ underneath it. Until it closes, a claim about rendering
names which path it is about (ADR 0058 § 6, ADR 0059 § 4).

**An honest word about the size.** What follows is years of work, and naming it
completely is the point — a list that stops at the interesting parts is how a
project discovers the boring half after it has promised a date. The order inside
each group is ours, and the trigger for most items is a real page that fails,
never a specification listing a method.

**Two things gate the rest** and are already under way: the process model,
because it cannot be retrofitted, and JavaScript, because most of this list is
unreachable without it.

### The process model

- [ ] **The process and sandbox model — designed before the first hostile page is loaded.** One process per site, renderers with almost no privilege. Every browser that retrofitted this suffered for years, and it is the one thing here that cannot be added later
      · Built: ADR 0005, and `alo-renderer` — the engine behind a message
      boundary, sent work and returning results, every message owned and
      `Send + 'static`, with a frame and an agent-tree snapshot as the only
      things that cross. That is the expensive half, and it is the half that
      cannot be retrofitted. **The wire format** (queue item 63): both
      directions, every variant, and a message from a renderer treated as bytes
      a stranger chose — because a renderer is the process that parsed the page
      **The split itself** (queue item 166): a renderer process per site,
      spawned and talked to over pipes, with a test that kills one and watches
      the other keep working **The sandbox** (ADR 0010 and queue item 167): rented, applied by
      `exec` so a renderer is never unconfined, and fatal if unavailable —
      watched failing rather than assumed **Fonts across the boundary** (queue item 168): the browser
      process opens the files and passes bytes, and `alo-render` embeds nothing
      **A font a page asked for by name** (queue item 170): a load says which
      families it wanted and did not have, the browser process finds each on the
      machine by the name the font gives itself, and a family that is genuinely
      not there is a substitution said in words rather than a silence
      **And the name the font gives itself is read in the encodings older than
      Unicode** (queue item 192): the Macintosh records several of the fonts
      macOS ships carry *instead* of a Unicode one, so a machine that has Apple
      Braille no longer says it does not — and with it, no family anywhere comes
      from a filename
      **And what a *generic* family means here** (queue item 193): the browser
      process decides `serif`, `sans-serif`, `monospace` and `system-ui` from the
      families it actually found and hands the answer over with the fonts, so
      the `system-ui, sans-serif` every page asks for through the user-agent
      sheet is a font on this machine rather than two families nobody had
      **And which face of a family a file holds** (queue item 194): the weight
      and the slant come out of the font's `OS/2` table, so `Helvetica-Oblique`
      leans and a semibold is 600 — nothing about a face is read off its
      filename now, which is what items 192 and 194 together mean
      **And in which language that name is read** (queue item 195): a `name`
      table states the same family once per language, so the unlocalised record
      wins where there is one and English wins over every translation — four of
      this machine's fonts were filed under Chinese names until it did
      **And a font that is many weights rather than one** (queue item 196): the
      `wght` axis is read as the range it covers, so one file answers a request
      for 400 and a request for 700 and is shaped, measured and outlined at the
      weight it was set to — twenty-eight of this machine's fonts are such a
      file, its system font among them
      **And a page's linked style sheet crossing the boundary** (queue item
      348, ADR 0035 §§ 1–4): a renderer asks for each `<link>`ed sheet once
      per document, in the answer to the work that found it, and applies its
      `<meta>` policy first. The browser process decides the ask as a style
      request under the page's header policy. It makes the request through
      the same hops as a page's fetch, and sends the bytes only when the
      answer is a 2xx `text/css`, so a link cannot read another site's page.
      The window's conductor makes the asks and paints the page again with
      what arrives. A loaded corpus case's sheets are answered from the files
      frozen beside it, through the browser process's own decision and
      check (queue item 349, ADR 0035 § 6), which froze the first page alo
      Sites publishes, its call-to-action section, as `alo-sites-cta`.
      Every request the window makes for a page is made on a network thread
      of its own, so its conductor never waits on a server, and a load's
      first frame is held until its sheets answer or three seconds pass,
      said when it is shown before its style (queue item 351, ADR 0041)
      · Owed: the Linux sandbox, queue item 169; the axes that are not
      weight — width, slant and optical size — which are queue item 197;
      a bound on a whole exchange, so a trickling server cannot hold the
      network thread's queue (item 376); and a loaded page's pictures
      (item 350)
- [x] A renderer that dies takes its tab and nothing else — and says so, rather
      than leaving a blank rectangle (queue items 166 and 65). It is not
      restarted silently, because that hides a bug somebody needs to see.
      *This line was ticked when item 166 made one renderer's death survivable
      by the others, and the half about the **tab** was not built: nothing here
      was a tab, nothing kept a painted frame, and the blank rectangle the line
      names is exactly what a person would have been shown. Item 65 built it —
      `alo-renderer`'s `tab.rs`, a frame kept per tab, every tab in the dead
      process told and no other, and a new process only when somebody asks for
      the page again*
- [x] The transport, and the lifecycle that starts, reuses and reaps renderers
      (queue items 63, 166 and 64). Length-prefixed messages over pipes; a
      process started per site and reused; a ceiling of sixteen with the least
      recently used evicted; and, since item 64, **reaping** — closing the last
      tab on a site stops that site's process, and closing one of two tabs on a
      site stops nothing.
      *This line was ticked once before, with the two items above it, on a
      reading of "reaps" that the eviction satisfies. It did not: evicting to
      stay under a ceiling is what happens when reaping has **not** happened,
      and it was un-ticked. What was owed is built, which is what a tick means*
      *And since queue item 198, an exchange has a **bound**: a renderer that is
      alive and says nothing for ten seconds is given up on and stopped, where
      before it held the browser process in a read for as long as it lived and
      every other tab with it. A renderer that is merely slow is still waited
      for, which is the half that decides what the bound may be. The number is a
      choice rather than a measurement, and it stands in for a question — wait,
      or stop it? — that needs an interface to ask in*
- [ ] Where one site ends and another begins — the origin, the site, and which
      of them gets a process · Built: the site, as ADR 0005 defines it — scheme
      plus **registrable domain**, decided against the public suffix list
      (`alo-url`'s `site`, queue item 156), and the one answer the cookie jar,
      the cache and the process split all use. The list is a compiled-in
      snapshot and it ages, so its age is recorded and the build fails once it
      is six months old (`alo-url`'s `snapshot`, queue item 186) — a boundary
      that has quietly stopped being current is the one failure here nobody
      would see
      **And which of the three a page is given** (queue item 66): the **origin**
      decides whether there is a site at all. Where it is a tuple the
      registrable domain widens it into a site and two tabs share a process,
      with the port left to the origin because two ports can already reach one
      another. Where it is **opaque** there is no site: a local file, a `data:`
      page, `about:`, a scheme nobody registered — each is rendered in a process
      nothing else is ever put into, where before every local file on the
      machine was one site sharing one address space
      · Owed: what a **document inside a
      document** is given, which nothing here can yet produce — a sandboxed
      `iframe`'s opaque origin and `about:srcdoc` inheriting its parent's (queue
      item 86), and a `blob:` taking the origin of whoever created it (queue
      items 72 and 90)

- [ ] **A rented renderer behind the same boundary — the machine's own Chrome, driven over a pipe (ADR 0058)**, so the open web renders while the engine is built
      · Built: the pipe (queue item 619) — `alo-window`'s `chrome.rs` starts
      the Chrome its installer put on the machine (`beside.rs`), headless on
      a private profile, with `--remote-debugging-pipe` and never a port;
      `chrome_wire.rs` frames its NUL-ended messages under a bound of ours and
      `chrome_protocol.rs` reads them, refusing every malformed one as a kind;
      Chrome 155 answers `Browser.getVersion` and is gone with its helpers and
      profile whether the browser closes it or dies; a frozen page opened in a
      tab of it with every other request refused (`chrome_tab.rs`, item 620),
      and its accessibility tree read as this browser's `Snapshot`
      (`chrome_tree.rs`) — `web-a-form` reads node for node as the engine
      reads it, but for an unnamed `form` Chrome exposes and the time field
      Chrome builds and the engine does not; the verbs on it (`chrome_act.rs`,
      item 621), decided by `alo-agent`'s `perform` against a `Reading` of
      Chrome's page (`chrome_reading.rs`) with the engine's own element rules,
      every refusal reached by name before Chrome is asked anything, and
      carried out over `Input` as a click checked against Chrome's hit test,
      or as a field focused, selected and typed into — `web-a-form`'s order
      comes to the same five outcomes through Chrome as through the engine;
      and a tab a frozen page opens is held before it loads anything and
      closed, so a click cannot take a frozen page to the network; a page
      from the network opened in a tab of it (item 633), every frame and
      worker held until its requests are paused (`chrome_hold.rs`) and every
      request judged by ADR 0020's rules before it is let go or refused by
      name (`chrome_gate.rs`) — a local page's other-site frame, dedicated
      worker and service worker sent nothing the tab had not let go first,
      a `mailto:` it sends itself to is refused by name, and an agent's
      off-site address is `NotGranted` with nothing written to Chrome;
      and every request of such a page in the record (item 634,
      `chrome_record.rs`), each with the cause this process gave it — a
      redirect's two hops one navigation of the person's, a style sheet the
      page's, a WebSocket a line saying it was opened and not decided, an
      agent's `activate` named on the navigation it caused; and a page's
      shared worker held through Chrome's own session (item 637), every
      request of it paused before a local server saw it; and the agent
      socket reaching a tab in Chrome (item 630, `pages.rs`,
      `chrome_serving.rs`) through the same serving and grants as the
      engine's tabs, httpbin's order filling the form by the same eight
      lines, its refusals the same kinds, the record naming Chrome and its
      version, and a tab named by where its page is now, after a link to a
      part of it or a `pushState` too (item 640, `chrome_protocol::within`),
      and `alo --chrome` opening an address or a page's file in Chrome from
      the program a person starts, naming the Chrome that draws it on the
      terminal (item 641, `opening::Renderer`), and showing the page in
      Chrome's own window, sized around the page and placed by this
      process (item 627, `chrome_window.rs`, ADR 0061) — at the size the
      screen gives it when that is less than asked, said on the terminal
      (item 671, `Placed::unlike`) — and the running
      browser answering a person who asks which renderer draws a tab, over
      the socket with `alo-ask` (item 622, the `renderer` ask), and a
      form Chrome would not send answered `NotSubmitted`, with the control
      Chrome stopped at and the words of the bubble it draws beside it,
      which are the engine's words for it (item 631, `chrome_bubble.rs`),
      and a date put into a date field through Chrome as a person puts one
      in — each of its segments, in the order Chrome lays them out, read
      from its tree by the part each says it is, given the keyboard and
      typed — so a form sends the very dates it sends through the engine,
      and a date Chrome's field would not hold as written is not typed at
      all (item 632, `chrome_date.rs`), and Chrome kept on the machine's
      resolver — measured in Chrome's own NetLog starting in *automatic*
      secure DNS, which upgrades a resolver it recognises to encrypted DNS
      unasked, and pinned off in the profile it is given (item 635,
      `chrome_profile.rs`), and each frame of a page in Chrome a document of
      its own in the record — caused by the page holding it, the other
      site's frame too, its `fetch` and its worker's its own, measured from
      where Chrome 155 tells of each frame's making and commit (item 636,
      `chrome_record.rs`), and what a page's script said while a verb ran
      answered as its issues through Chrome — Chrome's `Runtime` on only
      while a verb runs, what was said before forgotten first, and each
      console call and uncaught throw put into the engine's words, with a
      message from Chrome too long to read skipped to its end rather than
      ending the pipe (item 639, `chrome_said.rs`, `chrome_wire.rs`) — a
      frame from another site and a dedicated worker heard too, each told
      to forget, tell and stop in its own target (item 652,
      `chrome_tab.rs`), and a shared and a service worker likewise, a
      service worker started mid-verb let go without waiting on its answer
      (item 654, `chrome_hold.rs`) — and a
      frame sent to a scheme Chrome never pauses refused by name and
      recorded against its document, told of through the tab's session or
      the frame's own target (item 651, `chrome_tab.rs`), and a tab an
      agent asked to open recorded as that agent's action done in the tab
      — both hops of its first page the action's, what the page asks for
      leading back to it and never to the person, an ask refused first
      minting no action (item 638, `alo-net`'s `Place`, `chrome_record.rs`),
      and a service worker that stops and starts again in the target it
      had held again and let go, and never waited on by a verb while it is
      stopped (item 655, `chrome_hold.rs`, `chrome_tab.rs`), and what
      Chrome says and the tab has not yet read held to 32 MiB as well as
      to 256 messages (item 653, `chrome_held.rs`), and a form Chrome
      refuses for a control it cannot focus answered `NotSubmitted` from
      what Chrome logs of the control, a cancelled `invalid` answered
      `Activated` as a cancelled `submit` is (item 646,
      `chrome_unfocusable.rs`, `chrome_act.rs`), and a request paused while
      its frame goes to a new document recorded as the document whose
      loader Chrome says made it, the old page's when it says none or
      another, a frame's as the page's (items 647 and 656, `chrome_owed.rs`),
      and a request Chrome names no id for, paused after a frame's commit,
      recorded as the page the frame left (item 657, `chrome_record.rs`'s
      `Causes::of_unnamed`), and text on a `<select>` refused `Chooses`
      by the engine's own rule asked of Chrome's element, as on this
      engine's page (item 648, `alo_dom::field::is_chosen_from`), and
      an option chosen by `Activate` answered `Chosen` with the socket's
      kind for it, on this engine's page (item 659, `alo_dom::choice`)
      and through a list Chrome draws, clicked with ⌘ held in a list of
      several and read back from Chrome's tree, a closed list's option
      not pressed and refused by name (item 660, `chrome_choose`)
      · Decided: Chrome's own network carries a page's requests, each held
      and judged by the browser process before it leaves, and trust, cookie
      partitioning, the resolver, the cache and the browser's name are set
      aside on this path by name (ADR 0060); the window of ours around
      Chrome's floats, is put by Chrome's word and is shown only while the
      window server shows Chrome's (ADR 0062); and a tab an agent opens is
      the agent's action, done in that tab rather than in a document, never
      the person's (ADR 0063), and which of a frame's two documents a
      request is, by Chrome's word for its loader, which chooses between
      documents this process minted and never names a cause (ADR 0060's
      amendment), and so does a request it names no id for, which is the
      page the frame left (ADR 0060's second amendment, 657); and an
      option of a `<select>` chosen by activating it, a list taking no
      text, and a closed list's option refused through Chrome (ADR 0064)
      · Owed: what Chrome itself asks Google for, with no page
      open, outside the gate and the record (649); whether Chrome's own DNS
      client honours what the system's would — scoped resolvers, `.local`,
      the system's own encrypted DNS (650); a closed list's option
      through a shown Chrome (658), the ring around the box an agent
      acted on through Chrome (680, 624; decided, ADR 0066) and words drawn with the
      machine's own fonts (683) — the frame itself stands, follows, and
      carries the page's tab, the agent's sentence and its refusals (642,
      679, 681) — and a window that
      stays where a person sees it (643)

### The network

- [x] **URLs, properly**: WHATWG parsing, origins, IDNA and punycode. Every security decision below is made against the origin this produces, which is why it is first
      · `alo-url`: parsing rented behind one file, the types ours, and an
      opaque origin that is the same as itself and nothing else
- [x] TLS with `rustls`, and certificate errors a person can act on rather than click through
      · `alo-net` (queue items 51 and 52): the shape a fetch produces, and TLS
      over it. A refusal carries what is wrong, what trusting it anyway would
      mean, and whether the fault has an innocent explanation — three things a
      caller cannot show one of without the others. Verification cannot be
      turned off: no flag, no constructor, no feature
- [ ] HTTP/1.1, then HTTP/2 — connection pooling and keep-alive with them
      · Built: HTTP/1.1 (queue item 53) — a request out, a response in, body
      framing by length, by chunks and by close, and every message that says
      two things about where it ends refused by name. `http:` and `https:`
      fetch over a socket. **Pooling and keep-alive** (queue item 54), with the
      retry that has to come with them: a reuse that fails before a byte
      arrives is tried again, and a request that must not happen twice never is.
      **HTTP/2 framing** (queue item 59) — every length checked before anything
      is reserved, and the padding underflow that is the classic parser bug
      refused by name. **HPACK** (queue item 160), checked against the
      specification's own worked examples, with the Huffman codes derived from
      the canonical structure rather than transcribed. **Streams, flow control
      and the connection state machine** (queue item 161), with the CONTINUATION
      flood refused by a bound on the whole block. **Negotiated by ALPN and
      spoken** (queue item 162) — chosen during the handshake, so no request is
      sent twice to find out which protocol it is. **`Transfer-Encoding` as the
      list it is** (queue item 153) — `gzip, chunked` is chunks holding a gzip
      stream and decodes as one; `chunked` anywhere but last, a coding we
      cannot undo, and a compressed body the connection closing is the only end
      of are each refused by name. **A connection that ended told apart from a
      peer that misbehaved** (queue item 185), so a stream the server gave up
      on keeps the bytes that arrived on it, and a server that closed is the
      connection ending whether a read or a write is first to find it gone
      (queue item 455). **A request that sends something**
      (queue item 163) — a body after the blank line in HTTP/1.1 and in `DATA`
      frames in HTTP/2, cut to the frame size the peer allows, with a window
      that closes part way through waited on rather than overrun and the stream
      closed rather than left open when a server answers early; the length a
      request states is always its bytes rather than a header a caller wrote,
      and an interim response is read past rather than taken for the answer.
      **Which browser is asking** (queue item 326) — every request in both
      protocols carries, once, the `User-Agent` ADR 0030 composes in
      `alo-net`'s `user_agent.rs`, and a caller that set its own keeps it ·
      Owed: an `Expect` is refused by name rather than honoured, which needs a
      bounded wait — queue item 187
- [ ] HTTP/3 and QUIC, once those two are correct
- [ ] DNS, and encrypted DNS as a choice somebody made rather than a default
      nobody was told about · Built: ADR 0008 and resolution through the
      machine's own resolver (queue item 58), with DNS rebinding refused — the
      rule turns on who asked, so a person reaches their intranet and a page on
      the web does not · Owed: the encrypted-DNS setting itself, queue item 158,
      which needs an interface to choose in
- [x] Content encodings: gzip, brotli, zstd (queue item 152) — and `deflate` in
      both the spelling the specification asks for and the one servers send.
      All three rented and all three pure Rust, because a decompressor is where
      a memory bug is most directly a remote code execution. The bound is on
      what comes **out**: a bomb is small on the wire by definition
- [x] Redirects, byte ranges, and downloads that resume · Built: redirects
      (queue item 55) — bounded, loop-detecting, `Authorization` dropped at an
      origin boundary, a redirected `POST` demoted to `GET` on 301/302/303 and
      preserved on 307/308, and `file:`/`data:` refused as destinations; byte
      ranges with downloads that resume (queue item 154) — a body that
      stops early keeps its bytes and the rest of them are asked for, with a
      `206` required to begin **exactly** where the download stopped, a `200`
      answering a range request never appended, and nothing encoded ever
      spliced; and **the same over HTTP/2** (queue item 185), where a stream
      that ended early used to be an error that took the bytes with it. The
      loop is protocol-blind and lives in `download::whole_of`, which is why
      that was a change to the HTTP/2 client and to nothing else
- [x] **The HTTP cache, with real semantics** — freshness, revalidation, `Vary`
      (queue item 56). Nothing in it reads the clock, because the answers that
      matter are the ones only wrong an hour later. `Age` is counted, so a
      response that arrives old expires early; `Vary` is stored as a contract,
      so a French page is never served to a German reader; `Vary: *` is not
      stored at all · Built: the decision about a disk (ADR 0011) and the code
      that keeps it (queue item 155) — the key carries the top-level site, so a
      shared cache is no longer a history oracle or an identifier that survives
      clearing cookies; what must not outlive the session is never written
      rather than written and deleted, because a deleted file was still on the
      disk; and what comes back off the disk is read as what it is, which is
      bytes from outside, with anything unreadable a miss rather than a page
      that will not open
- [x] **Cookies**: `SameSite`, `Secure`, `HttpOnly`, partitioned by default
      (ADR 0007, queue item 57). The default is a product decision and the ADR
      argues it — who it protects, and what it costs. There is no way to ask for
      cookies without a partition. The partition is the **registrable domain**
      since queue item 156, so a person signed in at `example.com` is signed in
      at `www.example.com` and `Domain=co.uk` is refused · Owed: the escape
      hatch a person grants per-site (queue item 157)
- [x] The same-origin policy, CORS and preflight (queue item 61) — code we write
      and can get wrong, which is one of ADR 0005's four reasons for the
      sandbox. A page may send almost anywhere and read almost nowhere; a
      wildcard never covers a request that carried credentials; a `Set-Cookie`
      is honoured and unreadable. **The preflight cache** (queue item 164): a
      second request of the same shape sends no `OPTIONS` and one of a different
      shape still does, partitioned by top-level site, expiring on the caller's
      clock and capped at two hours however long a server asked for — because a
      permission nobody can revoke is not one. A `*` is remembered as what it
      allowed rather than as a standing offer. Building it found the safelist
      being applied by header *name* where it is a rule about the value too, so
      a JSON post was preflighted with a question that never named
      `Content-Type`
- [ ] Content Security Policy, referrer policy, HSTS, mixed-content blocking
      · Built: HSTS, mixed content and referrer policy (queue item 62) — a
      `Strict-Transport-Security` over plain HTTP ignored so it cannot be used
      as a weapon, a script refused outright where an image is retried over TLS,
      and a referrer that never survives a downgrade — its origin-only form
      written as a URL, `https://example.com/`, as other engines send it
      (queue item 265). **CSP is enforced** (queue
      item 165): `default-src`, `script-src`, `style-src`, `img-src` and
      `connect-src`, the source expressions pages use, nonces and
      `'strict-dynamic'` — and the rule the whole thing is built around, that a
      word this engine cannot read is kept and matches nothing rather than
      taking its directive down with it. A repeated directive keeps the first
      and two policies are an intersection, so nobody widens a policy by
      appending to it. **A violation is reported** (queue item 188):
      `report-uri` and `report-to` both, an enforced policy and a watched one
      alike, in the two documents collectors read — and a report says a
      cross-origin URL as its origin and nothing more, because a report is
      posted to a server the page chose and would otherwise be a way to read
      one. **A content hash is computed** (queue item 189): a policy that names
      the digest of its own inline stylesheet allows that stylesheet and refuses
      every other one, in either base64 alphabet, with a value that mixes the
      alphabets or is the wrong length allowing nothing — a hash source is a
      permission, so reading one loosely is a policy wider than its author wrote.
      **A `style` attribute can be allowed by its digest** (queue item 191),
      where the deciding directive also says `'unsafe-hashes'` — the keyword
      grants nothing on its own, does not reach out of the directive it was
      written in, and a policy without it refuses an attribute whose digest it
      names, because content with no element of its own is the shape an
      injection takes. **An inline script a policy refused, or a watched one
      would have, is reported** (queue item 237): the renderer, the only
      process that sees the script, names the objecting header policy by its
      place, and the browser process writes the report from its own copy of
      the headers and posts it — so a renderer can neither choose where a
      report goes nor what it says, a claim naming a policy that could not have
      objected is not believed, and one load carries at most 64. **Inline
      style is refused by `style-src`** (queue item 343, ADR 0034): every
      `<style>` and `style` attribute judged at every draw by every policy the
      page holds, its headers' and its `<meta>`s', kept for the page's life
      (`alo-renderer`'s `inline_style.rs`); a `<style>` presenting its nonce by
      a script's rule (`alo-dom`'s `nonce.rs`); a value `element.style` wrote
      exempt by its exact text (`alo-dom`'s `declared.rs`), and one function
      (`alo-bindings`' `style_policy.rs`) answering for the draw and for
      `element.style`, which reads a refused attribute as `""`; and what the
      load's draw objects to reported with its placement on the wire; and
      what a draw **after** load objects to carried in the next `Acted` or
      `Delivered`, each element, placement and text once for the page's life
      and a `Paint`'s finds waiting for the next answer, under the same bound
      of 64 (queue item 346, `alo-renderer`'s `objected.rs`)
      · Owed: a `<meta>` policy's `report-to` (queue item 240);
      **a nested document**, which is what `frame-src` needs and which
      nothing here can yet tell from a link click (queue item 86); and an
      **event handler** matched by its hash, which is the same rule as the
      `style` attribute and waits only for there to be handlers (queue item 81)
- [ ] `fetch()` and `XMLHttpRequest`, over the same stack rather than beside it
      · Decided (ADR 0032, queue item 83): a script's fetch is an ask in
      the answer to the message whose work made it, the browser process
      decides it from its own copy of the document's origin, policy and
      cause, and the response comes back as a task of its own carrying only
      what the page may read
      · Decided (ADR 0053, iteration 302, queue item 83's half that was
      left): an `XMLHttpRequest` makes exactly `fetch`'s ask, mode `cors`,
      with nothing new on the wire; its answer is one task firing Chrome
      154's measured events as the browser's dispatch; the document holds a
      request while its answer is owed; `abort()` withdraws an ask that has
      not left; a synchronous `open` throws `InvalidAccessError`; a
      `timeout` will be the browser process's to keep
      · Built: the boundary and the decision (queue item 334) — every answer
      that runs script carries its asks and `ToRenderer::Fetched` carries a
      response, both on the wire with hostile bytes refused; `Tabs` decides
      each ask as the answer passes, refusing by name a bad or overlong URL,
      a scheme other than `http(s)`, the document's `connect-src`, mixed
      content and a `same-origin` ask elsewhere, and a forbidden header,
      method or mode as a renderer that broke the boundary; the cause is the
      document's after a `Load` or a delivered response and the agent's after
      an `Act`; 64 asks at once and 64 waiting per document; and the filter
      sends `basic` without `Set-Cookie`, `cors` with only the readable
      headers, and opaque answers with no bytes
      · Built: `fetch()` in a page (queue item 335) — `alo-bindings`' `fetch`
      converts its arguments by Web IDL's rules, runs Fetch's request steps
      and records the ask in the document cell, every failure rejecting; the
      answer is a task that settles the promise with a read-only `Response`
      (`ok`, `status`, `statusText`, `url`, `type`, `redirected`, `headers`,
      `bodyUsed`, `text()`) or one `TypeError`; the renderer carries the asks
      in `Loaded`, `Acted` and `Delivered` and draws the page again after a
      delivery; and the corpus answers a case's fetches from its frozen
      responses through the browser process's own decision and filter —
      `alo-downloads` asks for both installers and, offline, marks both
      buttons
      · Built: a decided fetch is made (queue item 338) — `alo-renderer`'s
      `fetch_make` sends each hop through the session's `Pool` with its own
      `Origin` (`null` once tainted), `Referer` and jar `Cookie`, asks first
      with a recorded `OPTIONS` when CORS needs it, keeps `Set-Cookie` only
      when the hop carried credentials, checks every answer on a CORS chain,
      and follows, stops at or fails on a redirect as the page asked,
      refusing and recording a circle, a `same-origin` fetch leaving, and
      an insecure hop; `fetch_answering` answers a tab's fetches one at a
      time and makes nothing for a page that has gone; `alo-window`'s
      conductor makes them between orders, draws the page again and says
      why one failed; the status text is the server's own reason phrase,
      now kept by `alo-net` and its disk cache; and every hop after the
      first judged by the document's `connect-src` with its sources' paths
      ignored, as CSP3 has it for a redirected request (queue item 340,
      `alo-net`'s `Request::redirected`); and a request that outlives its
      page (ADR 0040, queue item 369): `FetchAsk`'s `keepalive` claim across
      the wire, `Purpose::Beacon` under `connect-src`, the browser process's
      own 64 KiB count per document (`fetch_kept.rs`, `Rule::KeptAlive`), a
      left page's keep-alive asks decided rather than refused and made after
      it has gone, answered by nobody (`Tabs::leaving`,
      `Answering::outlive`), and what is still waiting when the browser
      closes recorded as not made (`Answering::close`, `Rule::Closed`)
      · Owed: `Request`, `Headers` and `Response`
      a page constructs, other bodies, `json()`, a `signal` and the `init`
      members refused by name, each when a page needs one; `keepalive` on
      `fetch` (375); and `XMLHttpRequest` as ADR 0053 decides it when a page
      uses one (336), its `timeout` (493), its response types beyond text
      (494), and cancelling a request that has left (495, needs ADR)
- [ ] WebSocket
- [ ] ★ **Every request attributable** — which page, and which agent action,
      caused it. No other engine has needed to answer that, and an agent-driven
      browser that cannot is one nobody should trust · Built: the decision
      (ADR 0012) — a cause is **carried** on the request with no default rather
      than reconstructed from a log afterwards, because a guess recorded in the
      shape of a fact cannot be checked later; there are three causes and no
      `Unknown`, so an engine-made request belongs to whatever caused the thing
      it is about; a cause is a **chain**, since which page and which agent
      action are two questions with two true answers; and the **browser
      process** assigns it, because a renderer parsed a stranger's page and a
      cause it could state is a cause it could forge. The record is the
      session's, in memory, except what an agent did, which is kept until the
      person deletes it — and **no page and no agent may read any of it**;
      and the **cause itself**, on every request there is (queue item 67).
      Three causes and no fourth, with no default and no builder, so a request
      that cannot say what caused it does not compile; the four requests the
      engine makes for itself — a redirect hop, a resumed range request, a CORS
      preflight, a violation report — each carry the cause of the thing they are
      about, which is what let there be no `Unknown`; and a server answering
      `302` cannot turn a page's fetch into something the person did;
      and the **chain** (queue item 199) — loading a page makes a document that
      records what caused its own load, so a fetch by that page walks back to
      the agent action that opened it and on to the person, with both answers
      kept in order rather than one chosen. A page somebody opened themselves
      reaches no action, an action in one tab cannot reach into another, and a
      walk that meets a document twice stops rather than hanging the browser
      process;
      and the **session's record** (queue item 200) — every request the engine
      makes is a line in it, written where the request is made rather than by
      whoever remembered to, holding when, the cause, the method, the URL, the
      purpose and what happened, and **never a body, never a header set**, so
      the file that would log somebody into their own bank is one the type
      cannot become. A cache hit is a line that says it was the cache and a rule
      of ours refusing a hop is a line naming the rule, because *what did this
      page try to load, and what stopped it* is most of why anybody opens it. It
      dies with the process, it is bounded in lines **and** in bytes since a URL
      is something a page chooses, it says how many lines it has dropped, and
      emptying it is real. No page and no agent can reach it: a renderer holds
      no pool, nothing crossing the process boundary carries a line, and the
      agent surface does not depend on the loader at all;
      and **what an agent did, kept until the person deletes it** (queue item
      202) — the small durable half beside the session's, holding only requests
      whose chain reaches an agent action, so a person's own browsing is never
      written to a disk at all. It **freezes** each chain as it writes it, since
      a file read back next week has no documents left to walk against, and it
      keeps them as numbers rather than identities because this morning's
      `action#0` is not last week's. A session-scoped profile leaves **no file
      behind at all** — never written rather than written and deleted, which is
      a record nobody opened rather than a flag anything reads. The bound is
      counted in **actions**, so one action with three hundred requests in it
      cannot evict a week of ordinary ones; the oldest action goes whole; and
      everything it shortens it says the size of. It lives where the system
      keeps what it may not delete rather than beside the cache, because a
      record of what an agent did while nobody was watching must not be removed
      on a Tuesday to make room. What comes back off the disk is untrusted the
      way a cache entry is, and a file that does not read is a **gap it counts**
      rather than an error — and it is left where it is, because it is somebody's
      record and a later version of this engine may be able to read it. Deleting
      is real: the files, not a flag on them;
      and **a navigation refused is a line too** (queue item 548): the
      window's conductor takes where a page asked to go after every answer
      that can carry it — a load's, an agent's action's, a delivery's, a
      visibility change's, a person's click's — and hands each refusal to the
      network thread to write in its turn, as a refused fetch's is, under the
      method it was to go by and the cause the browser process assigned
      (`conductor.rs`' `navigation`, `fetch_exchange::Record::Navigation`).
      Only a person's click is said; a navigation let go writes nothing
      · Owed: the browser process assigning a cause to a *subresource*, which
      needs a renderer that can ask for one at all (queue items 80 and 83) —
      until then the page's own load is the only request it makes; and an
      action's own **outcome** beside the requests it caused (queue item 203),
      which needs the verb path to reach the loader

### JavaScript, ours, in Rust

- [ ] Lexer and parser to an AST — the current language, not ES5
      · Built: the **lexer** (`alo-js`, queue item 70, cut on starting) — source
      text into tokens, with the first of the two ambiguities the item names
      settled rather than guessed: `Lexer::next` **takes a goal symbol every
      call**, so `/` is division or a regular expression because the parser said
      which, and a `}` continues a template for the same reason. Everything the
      modern language spells is read — four bases with separators and `BigInt`,
      strings as UTF-16 code units so a lone surrogate survives, templates raw
      and cooked, private names, `ID_Start` identifiers with `\u` escapes
      remembered as escaped so an escaped reserved word is a name — and the
      three legacy forms ADR 0013 § 3 refuses are refused **by name**. It never
      panics: every cut of a hostile corpus, and every code point up to U+FFFF
      alone, is a token or a refusal with a line and column. Judged against a
      **frozen real script** in the corpus as well as a table
      · Built: the **parser and the syntax tree** (queue item 204), which is
      the rest of this line. Tokens into a program: statements, declarations,
      functions, classes, patterns and module declarations, every node carrying
      the bytes it came from because a stack trace and
      `Function.prototype.toString` are both built from those and neither can be
      retrofitted. The second ambiguity is settled the way the item said — on
      the **token stream**: `(a, b)` and `(a, b) => c` are the same characters
      until the `)` is passed, so the parameter list is tried and put back, with
      a `(` that was not one remembered so nesting costs the attempt once rather
      than at every level. Automatic semicolon insertion is all three rules and
      the seven restricted productions. Where the specification refuses a
      reading, so does this — `a ?? b || c` and `-a ** b` — and `with` is
      refused **by name** (ADR 0013 § 3), as is a reserved word written with an
      escape. **Nesting has a ceiling of ours and a stack to match it**: the
      parse runs on its own thread, because a depth limit that depends on the
      caller's stack is not a limit. Judged against **two** frozen real scripts
      now, the second holding six regular expressions and no division at all
      · Built: **a second ceiling, on the tree rather than on the parser**
      (queue item 208, found by item 72 and taken before it). Counting brackets
      answered a narrower question than the one that matters: nine shapes got
      past it and aborted the process, five by recursing where nothing counted
      — `!!!…a`, `new new …a`, `a**a**a…` — and four by being read in a *loop*,
      which costs the parser no stack at all and builds a tree as deep as the
      file is long — `a.b.b.b…`, `a()()…`, `a?.b?.b…`, `a+a+a…`. Sixty thousand
      `+` parsed in a moment and killed the process when the program was let go
      of, because `Drop` walks the tree one frame per level and every reader
      after it does too. So there are two bounds now, and they are two
      questions: how deep this parser **recurses**, and how deep a tree it
      **builds**. The second counts the path rather than the loop, so a chain
      inside a chain inside a chain is counted as all three, and it is put back
      around **siblings** — an array of fifty thousand elements is two deep and
      must go on parsing, or the bound would refuse every bundle there is
      · Built: plain function headers checked under the body's final
      strictness (queue item 205): reserved bindings and duplicate strict or
      arrow parameters are early errors before any statement runs. Grouped
      strings do not start or extend a directive prologue. Existing scope
      checks reject parameter/body collisions and labels across function
      boundaries while allowing legal lexical shadowing
      · Owed: early errors for the remaining parameter and function forms,
      private-name validation, named-expression binding separation, and import
      attributes delivered to the loader (queue item 222). The parser alone
      still does not certify every early error
      · Owed before item 222's named-expression binding work: a frozen real
      script demonstrating that failure. Iteration 119 halted without changing
      the compiler; the existing lexer/parser corpus is not execution evidence
      for this scope
- [ ] A bytecode compiler and an interpreter. **Correct first; a JIT much later or never**
      · Built: the decision (ADR 0013, queue item 69) — `alo-js`, ours, in safe
      Rust, **bytecode from the first line of the compiler** because a
      suspendable frame is what generators, `async` and a debugger all need and
      a tree walker expresses it by being rewritten. It re-argues renting rather
      than inheriting ADR 0001's refusal, and the close call is **Boa** rather
      than V8: safe Rust, permissive, exists today, and refused because a rented
      collector decides how `alo-dom` is stored and a rented engine's bounds are
      a stranger's idea of how much memory a script may make us allocate. Three
      clauses constrain every item under this heading — **absent beats
      approximate**, so a builtin we have not written is *not defined* rather
      than stubbed; **`alo-js` depends on no I/O crate**, so every capability
      comes from the embedder and the browser process never runs page script;
      and every bound a script can reach is a named constant of ours, with the
      interpreter interruptible and **never panicking** on any program
      · Built: **the machine** (queue item 72, cut on starting). A program
      becomes a flat array of instructions and an engine runs them: values, all
      of the operators with the conversions the specification asks for in the
      order it asks for them, `var` on the global object and `let` and `const`
      with real dead zones, blocks that shadow, objects and their properties,
      optional chaining, templates, and every shape of control flow — `if`,
      three kinds of loop, `switch`, labels, `break` and `continue`. Three
      things in it are decisions rather than detail. **The value stack lives in
      the heap** (ADR 0014 § 2), so the collector can walk it — which is why
      every instruction reads its operands where they lie and drops them only
      once the answer exists, and why the whole suite runs twice with the
      collector firing at every allocation. **The interpreter never recurses**,
      so the deepest expression a page can write costs a taller stack of values
      rather than a taller stack of frames; the *compiler* does recurse, so it
      runs on a stack of its own exactly as the parser does, measured against
      the deepest tree the parser will build. And **the embedder stops a
      script**, on a switch checked at every backward jump, because there is no
      clock in this crate and *when* is a person's judgement about a tab
      · Built: **functions** (queue item 209, cut on starting). A function is a
      value, calling one is a **frame on a list rather than a recursion of the
      engine**, and a closure keeps the names it was written beside after the
      call that made them has returned — which is why a function's parameters
      and body-level bindings live in a **cell in the heap** rather than in the
      frame that dies with the call. Three things in it are decisions.
      `this` is decided by the **callee's** strictness rather than the
      caller's, so a plain call is `undefined` in strict code and the global
      object in sloppy code and the caller does not have to know which; an
      **arrow captures the `this` where it was written**, so it is one field on
      the function rather than a chain to walk; and a recursion that will not
      end is the `RangeError` the language specifies, bounded twice, because a
      bound on the value stack alone under-counts what a frame costs
      · Built: **a call that begins half way through an instruction** (queue
      item 214). A getter, a setter and the `valueOf` or `toString` that turns
      an object into a primitive are all calls nothing in the source spells, and
      all three now run. The decision is that the interpreter **still does not
      recurse**: an instruction that needs a call hands its operands over, the
      frame goes on the same list every other call goes on, and the answer comes
      back to the place the instruction left. A getter's answer lands where its
      operands were, so a property access needs nothing remembered at all; a
      conversion writes the primitive into the operand's own slot and the
      instruction runs **again**, which works only because every instruction
      reads its operands where they lie. So an accessor that reads itself for
      ever is the same `RangeError` a runaway function is, and the conversion
      cannot loop because the search only ever moves on to the next name.
      Two types carry it: a conversion takes a value that **cannot be an
      object**, so no arithmetic can be handed one by mistake, and an operator
      that meets one answers *which* operand it needs converted rather than
      converting it, so the order `a > b` converts in stays in the one file that
      knows it
      · Built: **a binding per pass** (queue item 216). A block that declares
      something is an environment of its own now, made on the way in and left on
      the way out, so a function may read a name a block around it declared —
      ordinary code this engine refused by name until today. The decision is the
      specification's own shape rather than a cheaper one: a `for (let i = …)`
      head is **copied** at each pass, so a closure made in one pass keeps
      values the next pass cannot write to and a loop that makes ten functions
      makes ten different numbers rather than ten of the last one. A copy is a
      **sibling** — the same parent, so every hop the compiler counted still
      means what it meant — and a block that declares nothing makes no
      environment at all, which is why a hop is counted by asking a scope rather
      than by counting levels. Leaving is the jump's own business: a `break` out
      of three blocks emits three pops, because the blocks it skips will never
      reach their own. What is left of a frame slot is the compiler's
      temporaries, so nothing a script can name is one
      · Built: **`new`** (queue item 212, cut on starting to `[[Construct]]`).
      A plain `function` is given its `prototype` when it is made, and `new F()`
      makes an instance from it, runs the body with the instance as `this`, and
      answers with the instance unless the body returned an object. The
      decision is that **a construction is a call with a different landing**:
      the stack has a call's shape, the instance is written into the `this`
      slot, and the body is entered by the same code a call uses, so the frame,
      both bounds and the `RangeError` are shared — only what `return` does
      with the answer differs. Whether something constructs is a fact about how
      it was **written**, recorded on the chunk: an arrow, a method and every
      builtin are a `TypeError`, raised after the arguments are evaluated
      because a page can see the order. Opened by a **frozen real script** —
      alo's own service worker, refused at its `new Request(…)` — which then
      compiled past it to an array literal
      · Built: **the array literal** (queue item 225, cut from 211 and 73).
      `[a, , b]` makes an array of the literal's length with each element
      defined and each hole left out, so a hole is not `undefined` and `in`
      can tell. Opened by the same frozen script at that literal, which then
      compiled to the `try` on the next line (item 210)
      · Built: **`try`, `catch` and `finally`** (queue item 210). A throw lands
      in the nearest `try` around the instruction that threw, in its own call or
      any call waiting on it, and an error the engine threw is caught as an
      instance of its constructor. The decision is that **a throw lands through
      a table and everything else through code**: a `try` emits no instruction
      at its start or end but writes the range it guards into the chunk, so
      leaving one normally costs nothing and the table is read only when
      something is thrown; a `break`, `continue` or `return` that crosses a
      `finally` is compiled to remember which way it was leaving, run the
      block, and leave that way again from its end, which is how two
      `finally`s run innermost first. Because a page can now catch the
      `RangeError` that ended a runaway recursion, the embedder's stop switch is
      read on every call as well as on every backward jump. The frozen service
      worker compiles past its `try` to the `for…of` inside it (item 230)
      · Built: **`for…of`** (queue item 230, cut from 211, 75 and 73), with a
      `let`, `const` or `var` name or a property as its head. The decision is
      that **the protocol is compiled as the calls a page can see**: reading
      `obj[Symbol.iterator]` and calling it, calling `next` each pass and
      reading `done` and `value` are ordinary property reads and calls, so a
      page's own `next` or a getter on `done` behaves as anywhere else, and the
      interpreter gained only a symbol no source can spell and the protocol's
      two `TypeError` checks. Leaving early **closes the iterator by reusing
      `finally`'s routing**: the loop pushes a closing of its own, a `break`,
      `return` or outer `continue` is routed through it innermost first, its
      own `continue` is not, and a throw from the body lands in a handler that
      closes and ignores what the closing does — while a throw from the
      iterator itself closes nothing. `let` and `const` are a binding per pass
      and the head has a dead zone. **The frozen service worker now compiles
      whole**; running it stops at `self`, a worker's global and an
      embedder's to supply (item 91)
      · Owed: the rest of the language, and each piece is refused **by name**
      rather than half-built — classes, `super`, `new.target` and private names
      (queue item 223), `instanceof` past its first two answers (224),
      `arguments` and the parameter forms that are not a plain name (213),
      tagged templates (215), a `catch` parameter that is a pattern, spread,
      destructuring — in a `for…of` head too — and `for…in` (211), `for await`
      (75), an object assigned to an array's `length`, which is
      converted twice (226), and a **proxy**, whose trap is the same re-entry as a
      getter and which nothing can make until a builtin `Proxy` constructor
      exists (217). A program using one of them does not compile — or, for
      `instanceof`, stops where it is reached — and says which item builds it. The builtins that make a `{}` have a `toString` of its own
      were owed here and are built (queue item 218), on the standard library's
      own line
- [ ] A garbage collector, and the object model underneath it
      · Built: the question, stated (ADR 0013 § 6) — the engine defines **one
      trait** for an embedder's objects and the DOM is one embedder, so the
      single thing it demands is that such an object can be **traced**, which is
      what makes the collector's problem statable
      · Built: the decision (ADR 0014, queue item 71) — **one heap, one
      collector, and the DOM is in it**. A reference is an **index with a
      generation** rather than a pointer, which is ADR 0004's move again and is
      what keeps a stale reference a detectable engine bug instead of the most
      valuable bug class in a browser; ADR 0003's promise survives slot reuse
      because what is never reused is the *pair*. The collector is **precise**,
      so where a live reference may live is a closed list and conservative stack
      scanning — `unsafe`, and retention by accident — is refused. The **DOM is
      traced rather than counted**: node, listener and closure are one graph one
      collector reclaims, which is ADR 0013 § 1's claim made real and the reason
      renting an engine was unrecoverable. **Non-moving mark and sweep**,
      stop-the-world, correct before fast — with the three things that cannot be
      retrofitted in from the first line: a **write barrier** that does nothing
      today because both answers to a visible pause need it, an **ephemeron
      fixpoint** because a `WeakMap` added afterwards is wrong, and a **marker
      that never recurses**, which is item 204's finding restated for a graph a
      script chooses the depth of
      · Built: the heap and the collector (`alo-js`'s `heap`, queue item 71) —
      an arena of slots, a reference that is an index carrying a generation, a
      **precise non-moving mark and sweep**, and every clause of the decision
      above in code rather than in prose. The three that cannot be retrofitted
      are the ones to check: the barrier is the only way a reference is stored
      into a cell and it is counted so a test can prove it; the ephemeron
      fixpoint keeps a chain of weak maps that a single pass loses; and the
      marker uses a bounded worklist, so **a graph a million objects deep is
      marked with no recursion at all** and a buffer that fills up costs a
      rescan rather than an object. A cycle through an embedder's object is
      reclaimed, counted rather than watched. The numbers landed with it in
      `bounds.rs`, each with its reason: a heap ceiling, a collection trigger,
      and the two marking buffers
      · Built: **the object model the collector collects** (`alo-js`'s
      `object`, queue item 206) — and it landed *inside* the heap without one
      line of `heap.rs` changing, which was the argument for building the two in
      this order. An ordinary object is a prototype, a property table and one
      extensibility flag; a property is data or accessor with the three
      attributes and no fourth, and `ValidateAndApplyPropertyDescriptor` is in
      the object model rather than in a builtin, so what a page froze cannot be
      unfrozen by whichever caller happens to define next. **Property order is
      the specification's from the first line** — integer-like keys ascending,
      then strings in insertion order, then symbols — and it is a property of
      the storage rather than a sort at enumeration time, so a later hidden-class
      representation cannot quietly lose it. **Internal methods are a trait and
      it is the trait an embedder gets**, so an array, a proxy and an element are
      one mechanism rather than two. **Keys are interned against a weak table**
      that holds no copy of the text and no edge, so ten thousand objects with a
      `title` share one string and two hundred thousand names nobody kept are
      collected — with the collector firing on its own during the loop rather
      than because a test asked. A canonical array index is a key that allocates
      nothing at all. A string is a heap object, immutable once made, in UTF-16
      code units, so a lone surrogate is a name like any other and two of them
      are two names. The hostile half is its own file: a chain a hundred thousand
      deep walked without recursion, a cycle refused where the specification
      refuses it and **bounded** where an embedder lies about its own prototype,
      a hundred-digit name that is a string key rather than a saturated index,
      and add-then-delete in a loop that compacts rather than growing
      · Owed: the DOM's real wrapper (queue item 80) — the trait it will
      implement is built and tested against a stand-in; `WeakRef` and
      `FinalizationRegistry` callbacks as tasks (queue item 76); a partial
      property descriptor and the well-known symbols (item 73); a `BigInt`
      value (item 207) — the renting decision is made (ADR 0015: the limbs
      rented from `num-bigint` behind one file, every size checked by us before
      the call, every spelling ours), and the value itself waits for a frozen
      script that uses one; and a proxy's ability to intercept the walk
      itself, which needs something that can call a trap (item 72). Nothing here
      is callable, so an accessor hands back its getter rather than a value —
      which is why this line is still not ticked. No claim about speed is made
      here or anywhere: none has been measured
- [ ] The standard library: the ECMAScript builtins, in the order real pages need them
      · Built: **what a builtin is, and the two objects the language is made
      of** (queue item 218, cut on starting — the item named no closing
      condition, so what was taken is the piece everything else needs). A
      builtin is a function whose body is Rust, in the **same cell** a script's
      function is in, so `typeof` answers `"function"`, a page may hang a
      property on one, and it is called by the machinery every other call goes
      through — which is why one is found where a `+`, a getter or a setter
      reaches for it and not only where a page names it. Three things in it are
      decisions. It is a **function pointer rather than a boxed closure**,
      because everything a builtin could capture is either a reference the
      collector must walk or the realm it is reached through, so a native holds
      no edge at all. It gets **no interpreter**, which is the bound that makes
      it cheap — no bytecode frame and no Rust recursion. Item 219 adds
      suspension: a native asks the interpreter for a call or conversion, then
      resumes with its answer on the traced value stack. `call`,
      `toLocaleString` and object-to-property-key conversions use this path,
      and the recursion limit counts waiting builtins alongside script frames. And a builtin is **strict
      code**, so its `this` is what the caller wrote: a bare `toString()` is
      `"[object Undefined]"` here as it is everywhere else. `Object.prototype`
      and `Function.prototype` are the two objects, rooted by the realm with
      every method reachable from them, and `({}) + ''` answers
      `"[object Object]"` for the first time
      · Built: **the array, as an object** (queue item 225) — the exotic object
      whose `length` grows with an index and deletes from the end when made
      smaller, and `Array.prototype`, which is itself an empty array and
      carries **no method yet**, so `[].push` is `undefined` rather than half
      of what the specification says. The decision with the longest reach is
      that **every write is a definition**: the object model's trait lost its
      mutable borrow of a property, because a store that went round
      `[[DefineOwnProperty]]` would go round every exotic object's rules — an
      array's and an embedder's alike. A length a value is not is a
      `RangeError`; an object as a length is refused by name (item 226)
      · Built: **the errors a `catch` binds** (queue item 227) — `Error` and
      the six native errors, as constructors on the global object, with their
      prototypes, `constructor`, `name`, an empty `message`, the `cause`
      option and `Error.prototype.toString`; `Object.prototype.toString` says
      `[object Error]`. They are the **first builtins with a `[[Construct]]`**,
      and the decision worth reading is that a builtin constructor is *given*
      its instance in the traced `this` slot, called or constructed alike,
      because a native keeps nothing else across the script it asks to run.
      An error this engine throws is not yet an instance of one: turning it
      into one is what item 210's `catch` does
      · Built: **iterating an array** (queue item 230) — `keys`, `values`,
      `entries` and `[Symbol.iterator]` on `Array.prototype`, the last the very
      function `values` is; the array iterator, which re-reads the length every
      step and lets go of the array once done; `%IteratorPrototype%`; and two
      well-known symbols, `Symbol.iterator` and `Symbol.toStringTag`, made by
      the realm with no `Symbol` function to reach them, so
      `Object.prototype.toString` answers `[object Array Iterator]` by reading
      the tag rather than by a case for it. An element or an array-like's
      `length` behind a getter is refused by name (item 231), because a call
      from inside the iterator would make a generator's states observable
      · Built: **the slots a builtin keeps** (queue item 332, ADR 0031) —
      `alo-js`' `Native::keeping` declares up to
      `bounds::KEPT_BY_A_BUILTIN` (eight), reserved as `undefined` on the
      stack above the arguments and counted against its bound, read and
      written on the stack itself by `Call::kept` and `Call::keep`, and a
      declaration over eight refused where the function is made
      · Built: **`Array.prototype.forEach`** (queue item 331), the first
      builtin on those slots — `len` and `k` kept across every call, a
      `length` or element behind a getter called and an object length
      converted rather than refused, holes skipped, the embedder's stop
      asked on every pass; and `alo-bindings` gives a `NodeList` the very
      `forEach`, `keys`, `values`, `entries` and `[Symbol.iterator]` the
      realm's `Array.prototype` has, so `alo-downloads` walks its buttons
      and stops at `fetch` (item 75)
      · Owed: `apply` (221) on those slots, and an array iterator over a
      `length` behind a getter, which `for…of` over a `NodeList` reaches
      and which is still refused by name (231); and the
      remaining library. `Error.prototype.toString` of a `message` that is
      a getter or an object is refused by name (item 228, on 221), and
      `AggregateError` is item 229. `Object` and `Function` themselves are
      constructors item 73 has not built; a
      function's own `name` and `length` and its source text are 220; the
      `Array` constructor, `Array.isArray` and every array method but the
      three iterators and `forEach`, `JSON`, the wrapper objects (489), a
      string's iterator (490) and every string method but three, the `Symbol`
      function and the other eleven well-known symbols, the iterator helpers
      and the weak collections are still item 73, which is what remains of it;
      and **`Date` as text**: ADR 0036 decided `Date` (queue item 353), and
      item 356 built it — the engine's `Clock` and a realm given one or
      none, ECMA-262's time arithmetic (`alo-js/src/time.rs`), the
      constructor but from a string, `Date.now`, `Date.UTC`, every getter
      and setter, `toISOString`, `toJSON` and `Symbol.toPrimitive`, with
      `ToPrimitive` now asking for that symbol first; the renderer's wall
      clock in whole milliseconds (`alo-renderer/src/clock.rs`); and the
      corpus's one fixed instant. alo Sites' analytics script runs past it;
      and **`encodeURIComponent`** (queue item 359): ECMA-262's `Encode` in
      `alo-js/src/uri.rs`, bounded by the longest string, the function in
      `builtin/encode_uri_component.rs`, and `URIError` as an error the
      engine throws. The script now runs past that call, whose argument
      `location` is built (360), past `window` (362, ADR 0037) and on to
      its end; and **`Math`** (queue item 365): the namespace object in
      `builtin/math.rs`, its eight values and thirty-four functions, `max`,
      `min` and `hypot` folding however many arguments a page passes
      (`builtin/math_fold.rs`), every argument converted once and in order
      (`builtin/numbers.rs`). The `pagehide` listener, reached by a
      dispatch, now runs to its end; and **a string's properties** (queue
      item 488): its own `length` and units, then `String.prototype` — a
      `String` object of the empty string (`object/string_object.rs`,
      `builtin/string_create.rs`) — with `toString`, `valueOf` and `slice`
      (`builtin/string_prototype.rs`, `builtin/string_slice.rs`), read,
      written and deleted as the wrapper would answer without making one
      (`interpret/primitive_base.rs`). The same script now runs to its end
      on the contact form and the full page, past `slice(3)`. Still owed: a
      date as text (357), the person's zone (358), `Math.random` (367, needs
      an ADR), `Math.f16round` and `Math.sumPrecise` (368), `encodeURI` and
      the two decoders (73), `ToObject` of a primitive and the `String`,
      `Number` and `Boolean` functions (489), and a string's iterator (490)
- [ ] Regular expressions, with the syntax the language actually has
      · Built: the engine ADR 0029 decided, in `alo-js/src/regexp/` (queue
      item 74) — the pattern parser for the whole grammar, so a bad pattern
      is an early `SyntaxError` and Annex B's forms are refused by name; the
      compiler; and a backtracking matcher over UTF-16 code units with a
      stack of its own, every step counted against
      `bounds::STEPS_IN_A_MATCH` and every place against
      `bounds::PLACES_IN_A_MATCH`, so `/(a+)+$/` on thirty `a`s and a `b` is
      a `RangeError` a page catches, and the embedder's stop asked inside. A
      literal makes a `RegExp` object with `lastIndex`, and
      `%RegExp.prototype%` has `exec` and `test`, with `g`, `y`, `m`, `s`,
      `u`, named groups, lookbehind and the `(?ms-ms:…)` modifiers.
      `alo-downloads`' script compiles and its patterns answer right · Owed:
      `i` and `\p{…}` with the rented Unicode tables (322), the string
      methods that take a pattern (323), the `RegExp` constructor, `source`,
      `flags`, `toString`, `v`'s set operations and `d`'s indices (324), and
      an object string argument followed by a second call (221)
- [ ] Promises, the microtask queue, `async`/`await`, generators and iterators
      · Built: **the iteration protocol** that `for…of` reads (queue item
      230) — `GetIterator`, a step and `IteratorClose`, compiled to ordinary
      calls, with `%IteratorPrototype%` and the array iterator behind them;
      and **the microtask queue** (queue item 232) — `alo-js`'s job queue in
      the heap and the checkpoint that drains it; and **a promise** (queue
      item 333, ADR 0032 § 5) — `Promise` and its executor, `then`, `catch`,
      `finally`, `Promise.resolve`, `Promise.reject` and `Symbol.species`,
      a promise a cell in the heap, each reaction a job on that queue, a
      thenable adopted by a job of its own, an executor's or a handler's
      throw a rejection (`Want::Catch`), and a rejection nobody handled by
      the end of a checkpoint reported as an uncaught throw is
      · Owed: a promise made by a constructor other than `Promise` — a
      subclass, another `Symbol.species` — refused by name (queue item 337);
      generators, which are a suspended frame; the combinators;
      `async`/`await`; `for await` and async iterators (queue item 75)
- [ ] Modules: ESM, dynamic `import()`, and the loader that fetches them
- [ ] **The event loop** — tasks, microtasks, the rendering steps, `requestAnimationFrame`. Where "it works, but the animation stutters" is decided
      · Built: **the engine's half** (`alo-js`, queue item 232, under ADR
      0016) — the job queue in the heap, `Engine::queue_job` and a builtin's
      `Want::Job`, `Engine::checkpoint` running jobs oldest first including
      those jobs queue, reporting a throw and running on, dropping the queue
      when stopped and ending the job either way, and `Engine::call` for a
      loop to call a listener with nothing running; and **the renderer's
      loop** (`alo-renderer`'s `event_loop`, queue item 235) — one task queue
      run oldest first, a task's script held by one root, a checkpoint after
      every script and every listener call, `queueMicrotask` on the global
      object, a throw reported and the loop running on, a stopped page's
      tasks, roots and jobs dropped, and the quiet point between tasks checked;
      and **the `Renderer` holding one loop per page** (queue item 236) — a
      page's own inline classic scripts run as it loads, in document order,
      each a task with its checkpoint, each asked first of the page's
      `Content-Security-Policy` (the response's headers, carried in `Page`, and
      every `<meta>` policy in `<head>` before it, with nonces only from markup
      CSP calls nonceable); a resize runs nothing again and a new page gets a
      new realm
      · Owed: the loop running between messages for tasks a page queues
      itself, and `Act` answered after its checkpoint (queue item 233); the
      rendering steps and `requestAnimationFrame` (234); a page's fetched
      scripts (238); item 76 closes when 233 and 234 have
- [ ] Errors and stack traces good enough to debug somebody else's minified page
      · Built: **an uncaught error said by name and message** (queue item
      239, `alo-renderer`'s `event_loop/described.rs`) — read from data
      properties along the error's chain as `Error.prototype.toString` would
      join them, with no getter called, no object converted and a page's own
      `toString` not consulted; every string a report repeats cut at 1024
      code units; **an uncaught throw placed** (queue item 241, `alo-js`'s
      `interpret/unwound.rs` and `alo-renderer`'s `event_loop/source.rs`) —
      the script, line and column, in UTF-16 code units, of the throw and of
      every call it left, at most 32 with the rest counted, found in a
      one-line bundle without re-reading it; **a ceiling on what one load
      says** (queue item 242, `alo-renderer`'s `scripts::MOST_SAID` and
      `event_loop::MOST_REPORTS`) — 256 lines about a load's scripts and then
      how many more, and a turn of the loop keeping at most 256 reports and
      counting the rest without describing them; **the same for a page's
      markup** (queue item 243, `alo-renderer`'s `said.rs`) — 256 lines about
      what the markup made the engine say and then how many more, only those
      written out, and every line of a load's report at most 8192 characters
      with the rest counted; **the font names a load asks for, bounded**
      (queue item 244, `alo-renderer`'s `families::LONGEST_FAMILY`) — a family
      longer than any font's can be is not asked for, and that is said
      · Owed: `error.stack`, a trace taken where an error is made, and names
      in it (queue items 78 and 220); source maps; showing any of it to a
      person (developer tools, 129); a non-error object described
- [ ] Internationalisation (`Intl`), rented rather than written
- [ ] Refused for now and recorded: a JIT, until there is a measured reason and an ADR weighing it against the attack surface it adds
      · Built: the recording, with its two conditions for re-opening named
      (ADR 0013 § 2) — a **measurement on hardware** saying the interpreter is
      why somebody reached for another browser, and an ADR of its own weighing
      `unsafe` and writable-then-executable memory inside the process that
      parses hostile bytes. Two cheaper things are named ahead of it so it is
      not reached for first: an interpreter actually optimised, and not running
      the script at all. `unsafe` in the **value representation** is refused on
      the same terms, since NaN-boxing is the obvious first one · Owed: nothing
      until a measurement re-opens it — which is what this line is for

### The DOM, and the pages that use it

- [x] Mutation from script — create, append, remove, replace — and the invalidation that has to follow it
      · Built: **the document's half** (`alo-dom`, queue item 245) — create,
      insert, append, replace and remove under the DOM standard's names and
      validity rules, refusing as `HierarchyRequestError`, `NotFoundError`
      or `InvalidCharacterError` with the tree unchanged; a change count;
      script-made nodes numbered on the parser's counter; and a released
      detached tree's ids answering nothing, never reused. **The document
      in the heap and its wrappers' lifetime** (`alo-bindings`, queue item
      248) — the engine hands an embedder its own object back by type; a
      page's document becomes one heap cell counted at its size; a node's
      wrapper is one object kept while its tree is reachable (the
      document's always, a detached tree's while any of its wrappers is),
      and a detached tree nothing holds is released at a collection,
      without allocating. **The interfaces a script calls** (queue item
      249) — `Node`, `Element`, `Document` and the `ChildNode` mixin with
      item 80's members only, the standard's prototype chain, Web IDL's
      brand check and argument rules as a `TypeError`, every refusal thrown
      as the `DOMException` it names with `Error.prototype` on its chain,
      `document` on the global object, and every other member absent;
      proven by scripts the engine runs against an adopted document. **A
      page's script reaches it, and the page is drawn from what it left**
      (`alo-renderer`, queue item 250) — the document moves into the
      page's heap when its first script that may run is about to, every
      render borrows it wherever it is, and the page is drawn again whole
      when the change count has moved, at `Paint`, `ReadTree` and an
      `Act`, once at the end of a `Load` and from the document it has at a
      `Resize`; a page that runs no script builds no heap; proven by the
      corpus case `a-script-grows-a-list`, loaded through a renderer, and
      `what_a_script_left.rs`. **A script runs at its own end tag**
      (`alo-dom` and `alo-renderer`, queue item 247) — the page is parsed a
      step at a time, each step lent the document wherever it is, and each
      script sees the document parsed so far, itself its last element; no
      detached tree is released while the parser may hold it; a `<meta>`
      policy holds from where the parser made it; proven by the corpus case
      `a-script-beside-itself` and `a_script_at_its_own_end_tag.rs`. Done
      as queue item 80 (iteration 147). **A thrown `DOMException` is
      named in a load's report** (`alo-renderer`, queue item 252, iteration
      148) — by the name and message it was made with, read from the cell
      and running nothing; proven by `a_dom_exception_said_by_its_name.rs`.
      **`document.body`** (`alo-dom`'s `body.rs` and `alo-bindings`, queue
      item 253, iteration 149) — read as HTML's *the body element* and
      assigned by replacing it or appending to the document element, a
      non-body refused as `HierarchyRequestError`; proven by the corpus
      case `a-script-gives-a-new-body` and `the_body_element.rs`.
      The remainder, each its own item: `document` as Web IDL's accessor
      (251), and rendering only what changed (113). The decision is made (ADR 0017): the document
      moves into the page's heap when it first runs script; a node's wrapper
      lives while its tree is reachable and an unreachable detached tree is
      freed, its ids never reused; every change goes through `alo-dom` under
      the standard's validity rules; a changed document is rendered again
      whole when its rendering is read; and a parser-inserted script sees the
      document up to its own element
- [ ] **Events**: capture and bubble, listeners, default actions
      · Built: **events from script** (queue item 254, ADR 0018 §§ 1–3, 8)
      — `EventTarget` heads every node's chain, a listener lives in its
      target's wrapper, and the DOM's dispatch algorithm is written once in
      `alo-bindings` (`dispatch.rs`) with its state in the event, driven by
      `dispatchEvent`: capture and bubble, `once`, `passive`, both stops, a
      listener's throw reported by the engine (`alo-js`'s `Want::Report`)
      and the dispatch carrying on; `Event` and `CustomEvent` constructed by
      a page. Corpus case `a-script-hears-an-event`. **The browser's
      dispatch** (queue item 255, ADR 0018 § 3): a `Work::Dispatch` task in
      `alo-renderer`'s event loop holding a browser-made event and its
      target by one root, stepping the same stepper with a microtask
      checkpoint after every listener; a page that never ran script given
      no heap (`Held::dispatch`). **`isTrusted`** (queue item 260, ADR 0019)
      — `alo-js`'s realm has ECMAScript's `[[HostDefined]]`, set once to the
      document cell and handed to a native (`Call::host_defined`), and the
      cell holds `Event`'s unforgeables object, whose one getter per realm
      `unforgeable.rs` copies onto every event as an own, non-configurable
      accessor: `false` for a script's dispatch, `true` for the browser's.
      **An agent's `Activate` is a keyboard's click** (queue item 256, ADR
      0018 §§ 4–7, closing queue item 81): on a page that runs script it is
      one `Work::Activate` task — a trusted `PointerEvent` `click` with no
      position (`alo-bindings`' `UIEvent`, `MouseEvent`, `PointerEvent`),
      wrapped in `alo-dom`'s `activation.rs` (a checkbox or radio changed
      before the listeners and put back if one cancels; `input` and
      `change` after; a link followed only if nobody cancelled; a
      `<details>`' summary toggling its `open` after a click nobody
      cancelled, on a page with script or without — queue item 429, ADR
      0048) — answered
      after the task, with what its script said in `Acted`'s issues; the
      agent changes no ARIA state there. `alo-settings` carries
      `SettingsModal.tsx`'s nav script, and pressing *Sharing* makes it the
      open row. **A script's `el.click()`** (queue item 261, ADR 0018 § 6):
      `HTMLElement` between `Element` and every HTML element
      (`alo-bindings`' `interface/html_element.rs`), and `click()` the
      standard's — nothing on a disabled form control or while the
      element's click is in progress, otherwise an untrusted `PointerEvent`
      `click` driven from the native as `dispatchEvent` drives one
      (`scripted.rs`), no checkpoint between listeners, with the same
      `activation.rs` around it and `input` and `change` inside the call.
      **An agent's `PutText` fires what replacing a field's text fires**
      (queue item 257, ADR 0018 § 5): on a page that runs script, one task
      — a trusted `beforeinput`, an `InputEvent` (`alo-bindings`'
      `interface/input_event.rs`, inheriting `UIEvent`) whose `inputType` is
      `"insertReplacementText"` and whose `data` is the text; a cancelled
      one leaves the field as it was and answers the new
      `Outcome::TextCanceled` (wire tag 4); otherwise the text, put by
      `alo-dom`'s `field.rs` (the rule `apply` now asks too), then `input`
      and `change` (`alo-renderer`'s `event_loop/typed.rs` and `put.rs`).
      **A followed link is an ask** (queue item 263, ADR 0020): a script's
      `click()` and the agent's own link record the page's ongoing
      navigation in the document cell (`alo-bindings`' `navigating.rs`,
      resolved against the document's base; one, the last), it crosses in
      `Loaded` and `Acted` (`alo-renderer`'s `ask.rs`, `Page::url`), and the
      browser process decides it (`navigate.rs`: parsed again, the scheme
      table, `Referer` from its own copy) with the cause from which message
      it answered (`Tabs::load`, `Tabs::act`, `Tabs::navigation`).
      **The window is an event target** (ADR 0037, queue item 362): the
      global object is a `Window` (`alo-bindings`' `window.rs`, made through
      `alo-js`'s `Engine::with_global`) holding its own listeners, with
      `window`, `self` and `location` as its own members
      (`interface/window.rs`), and every path in `dispatch.rs` ends at it
      after the document, except for `load`. **A page is told whether it can
      be seen** (ADR 0039 § 1, queue item 364): `Page::visibility` and
      `ToRenderer::Visibility` (wire tag 9), a task that sets the document
      cell's state and fires `visibilitychange` only on a change
      (`alo-renderer`'s `event_loop/shown.rs` and `show.rs`), answered as a
      delivery is and decided as the document's (`Tabs::visibility`);
      `document.visibilityState` and `document.hidden` (`alo-bindings`'
      `visibility.rs`, `interface/document_visibility.rs`); and `alo-window`
      telling the selected tab on `Occluded` (`Order::Visibility`). **A page
      is shown and left** (ADR 0039 §§ 2–4, queue item 373): `pageshow` at
      the window as a load's last step, and the leaving steps — `pagehide`,
      `hidden`, `unload` — as one task (`alo-renderer`'s
      `event_loop/transition.rs`, `transition.rs`), run by
      `ToRenderer::Leave` (wire tag 10) and before every `Load` into a
      renderer holding a page, under a one-second `Deadline` on the page's
      `Stop` (`deadline.rs`); `FromRenderer::Left` and `Loaded`'s `left`
      carry the page's last fetches, which `fetch_decide::leaving` refuses by
      name and `Tabs::close` and the conductor record; `PageTransitionEvent`
      (`alo-bindings`' `interface/page_transition_event.rs`); and
      `navigator.sendBeacon` (ADR 0040, queue item 369, `alo-bindings`'
      `beacon.rs`), counted against 64 KiB in flight in the document cell
      and freed as each answer arrives, so `alo-sites-cta` reports with no
      beacon lent and its reports are made after it has gone · Owed: its
      immutable prototype (363), the legacy target override (374), choosing
      among tabs telling each (528), going there (85), a
      link's download (264), each element's own interface (262), focus
      (258) and event handler attributes (259)
- [ ] **Forms**: the controls, constraint validation, submission, file inputs
      · Built: **a control draws its own state** (queue item 182) — a tick in a
      checked box, a dot in a chosen radio, a dash in one that is neither, in
      `accent-color` if the page names one; and a control nobody can operate
      says so *while still saying what state it is in*. Corpus case
      `control-states`. **A group of controls looks like a group** (queue item
      183) — a fieldset draws its border, and its legend sits *in* that border
      rather than above it, so the border is drawn in the two pieces the legend
      leaves. Corpus case `fieldset-group`. **Its border is a `groove`**, as
      other browsers draw it, with the legend's part cut out (queue item 267,
      `alo-paint`'s `banded.rs`), and a `dashed`, `dotted` or `double` one is
      cut the same way, its pattern laid along the whole side (queue item
      268, corpus case `fieldset-patterns`). **An empty field stands where
      its text will** (queue item 384, `alo-layout`'s `baseline.rs` and
      `inline::empty_line_baseline`): an `<input>` nobody has typed into is
      level with its label, and does not drop when typed into. Corpus cases
      `alo-sites-booking` and `web-a-form`. **An empty date field draws how a
      date is written** (queue item 385, ADR 0042): `yyyy-mm-dd` in the
      field's own colour (`alo-box`'s `field_text`, `alo-dom`'s `date.rs`, a
      valid date string or nothing), read by an agent as a `date` with no
      value, and filled by `PutText` of a valid date string only — anything
      else refused as `NotADate`. A field's text no longer paints the field's
      background and border a second time around itself (`alo-paint`'s
      `style_of`). **A field's `placeholder` is drawn** (queue item 389,
      ADR 0043): the first pseudo-element this engine styles, its style
      computed as its field's child (`alo-css`'s `matches_pseudo`,
      `alo-style`'s `pseudo.rs` and `StyleTree::pseudo`, `alo-dom`'s
      `placeholder.rs`); the hint a run in the field's line in
      `::placeholder`'s colour, adding no width (`alo-box`'s
      `is_placeholder`, `alo-layout`'s `measure_inline`), read by an agent
      as a `placeholder` property and never as text, through the renderer's
      snapshot too. Corpus case `alo-sign-in`. A one-line field taller than
      its line now holds its line in the middle, as browsers do. **A
      `<button>` is as tall as what it holds** (queue item 437, `alo-box`'s
      `Purpose::Control` `at_least_a_line`, read by `alo-layout`'s
      `control_content_style`): a line tall inside only when it holds text
      or another inline, so an icon button is its icon and its padding, as
      browsers draw it, while an empty field is still a line. Corpus case
      `alo-agenda-mini-month`. **A checkbox and a radio stand where Chrome
      puts them** (queue item 443): 13 × 13 with their border and Chrome
      154's user-agent margins (`margin: 3px 3px 3px 4px`, a radio's `3px
      3px 0 5px`), standing on the baseline by their bottom border edge
      (`alo-box`'s `toggle.rs`, read by `alo-layout`'s `baseline.rs`), so
      alo Settings' row is 19 tall with its checkbox and label at Chrome's
      x. Corpus case `checkbox-beside-text`. **And each draws Chrome's
      picture of itself** (queue item 454, `alo-paint`'s `toggle.rs`; the
      marks' shapes in `control.rs`): no CSS border, a rounded square or a
      circle inside a grey edge, the accent and a tick or dot when checked,
      translucent greys when disabled, all measured from Chrome 154's
      screenshots. Corpus case `toggle-artwork`, compared with Chrome's
      screenshot beside it pixel by pixel within a measured bound; a
      disabled one is an inside a fifth of a pixel short of its edge with
      `#767676` at 77/255 over it as edge, fill or dot, fitted at one, two
      and eight device pixels to one, so the page no longer shows where
      they meet (queue items 463 and 578, iterations 340–341: 44 of 255
      at worst over black, then 24, now 15, a radio 10). **Drawn
      on whole pixels** where it stands on fractions (queue item 457,
      `alo-paint`'s `snap.rs`), each edge rounded as Chrome 154 rounds it;
      `checkbox-beside-text` compared with Chrome's screenshot. **A form
      is checked and asked for** (queue item 468, iteration 296): a submit
      button submits its form owner (`alo-dom`'s `activation.rs`,
      `owner.rs`); its controls are checked by `valueMissing` and
      `typeMismatch` in Chrome 154's words, and a constraint not built
      refuses by name where it could fail (`constraint.rs`, `email.rs`);
      the entry list, method, action and enctype are Chrome's
      (`submission.rs`, `value.rs`); and on a page that never ran script an
      agent's press answers `Submitted`, `NotSubmitted` or a refusal, the
      ask crossing the boundary as names and values (`alo-renderer`'s
      `submit.rs`, `wire/form.rs`). Corpus cases `web-a-form` and
      `alo-sites-booking`, in `a_form_on_somebody_elses_page_is_sent.rs`;
      and the browser process decides what was asked (queue item 470,
      iteration 297): a `GET`'s entries replace the action's query, a
      `POST`'s are an urlencoded body with no charset parameter, each
      Chrome 154's bytes (`alo-url`'s `form.rs`, `alo-renderer`'s
      `form_body.rs`); the header policy's `form-action`, a secure page's
      form to `http` and multipart and plain-text bodies are refused by name
      and recorded under the form's method (`navigate.rs`); and the jar
      keeps a `SameSite=Lax` cookie off a `POST` from another site
      (`header_for_navigation`). On the same two frozen pages through real
      tabs, in `alo-renderer`'s `a_form_is_decided.rs`; and a form on a
      page that runs script is submitted by an agent's press (queue item
      485, iteration 298): inside the click's task, a trusted `invalid` at
      each failing control or a trusted `SubmitEvent` naming its submitter
      (`alo-bindings`' `submit_event.rs`), HTML's guard against a second
      submission (`submitting.rs`), and the ask built after the listeners
      (`alo-renderer`'s `event_loop/submitted.rs`), on the frozen
      `alo-sites-booking` whole and decided through real tabs.
      **A page sends its own form by `fetch`** (queue item 472):
      `FormData` from the entry list a submission uses, iterable over
      pairs; `URLSearchParams` from a string, a record or a sequence of
      pairs walked one call at a time; a `URLSearchParams` as a
      fetch's or a beacon's body with Chrome's type; `className` and
      `replaceWith`. `alo-sites-contact-form`'s listener posts Chrome's
      exact body and, answered from the case's first frozen response, puts
      its thanks where the form was, boxed where Chrome boxes it
      (`sent.png`). **The agent tree reads a control's validity** (queue
      item 476, iteration 301): `invalid` when `constraint.rs` says the
      form would refuse it, as Chrome 154's accessibility tree was measured
      to say it — from load, a press changing nothing, an empty text field
      never — an author's `aria-invalid` winning, and `invalid=unknown`
      where a constraint is not built (`alo-box`' `invalid.rs`; ADR 0051's
      amendment) ·
      Decided (ADR 0051, iteration 294): a control's state is held apart
      from its attributes; its form owner is worked out when asked for;
      each constraint is a rule in `alo-dom`, and a constraint not built yet
      refuses the submission by name; submission is HTML's algorithm, ending
      in ADR 0020's ask with a method and an entry list that the browser
      process encodes and decides; an agent submits by activating the submit
      button; and a file is chosen only by a person · Owed:
      the rest of what a control **does** (queue items 469, 471–475, 486
      and 487: a script's own `click()` on a submit button, 487; the state apart
      from its attributes, 469; a `<meta>` policy's `form-action` and its
      violations reported, 486; script's form members, 471, whose
      `form.submit()` the contact form's offline fallback now reaches; the
      `formdata` event, 491; a `FormData` as a body, 492; the rest of the
      constraints,
      473; multipart and plain-text bodies, 474; a file chosen by a person,
      475) — constraint validation,
      submission, file inputs — the
      focus ring, which needs something to have focus (queue item 43); a
      date field's calendar (386), the person's region (387), the other
      temporal kinds (388); a long hint's clipping (390),
      `:placeholder-shown` (391) and `::placeholder`'s other properties
      (392), and a button's two measured edges, an empty `<span>` and a
      kept space (438), each waiting for a page; an author's padding and
      border on a checkbox, which Chrome ignores (453, waits for a page);
      a native checkbox's picture following its own checkedness rather
      than `aria-checked` (456, waits for a page), a light accent's
      dark tick (458, waits for a page), and whether a disabled control's
      seams at one device pixel to one must meet Chrome's quantised
      coverage (463, waits on a person)
- [ ] **Navigation and session history**: `pushState`, back and forward, and what survives each
      · Built: **`location`, read** (queue item 360) — `alo-url`'s
      `reading.rs` (the URL Standard's nine readings over our `Url`) and
      `alo-bindings`' `location.rs` and `interface/location.rs`: the page's
      one `Location`, every member `[LegacyUnforgeable]`, reading the
      document's URL at each read (`about:blank` when none was stated), the
      same object from the global object and from `document.location`;
      everything that navigates refused by name · Owed: navigating through
      it, by ADR 0020's ask (85); `Location`'s exotic internal methods
      (361); `ancestorOrigins`, with frames (86)
- [ ] `iframe`s and the sandbox attribute — a document inside a document, where a great many security bugs live
- [ ] Shadow DOM and custom elements; component frameworks are not optional on the modern web
- [ ] Selection and ranges
- [ ] CSSOM — styles readable and writable from script
      · Decided (ADR 0033, queue item 339): the `style` attribute is the
      inline declaration block, cascaded above every selector and below
      `!important`; `element.style` holds only its element and reads and
      writes the attribute; it names only the properties this engine acts
      on; a value is kept as written and `background` stays one declaration
      · Built: the `style` attribute cascaded (queue item 341) —
      `alo-css`'s `parse_declaration_list` reads it as a sheet's block, and
      `alo-style`'s `attached.rs` and the cascade's element-attached step
      place it, for HTML and SVG elements, read from the document on every
      draw (corpus case `style-attributes`); and what the inline block is to
      a script, without a heap (queue item 344) — `alo-css`'s
      `properties.rs`, the properties this engine acts on, each with the
      crates that read it and held true by a test in each of them;
      `longhand.rs`; and `inline.rs`'s `InlineStyle`, CSSOM's edits and
      serialiser over the attribute's text; and `element.style` itself
      (queue item 342) — `alo-bindings`' `CSSStyleDeclaration`, every
      member of ADR 0033 § 6 and a named accessor for each listed property,
      on HTML elements and a new `SVGElement`, which greys `alo-downloads`'
      offline buttons `#c7bfb2`; and a page's `style-src` applied to inline
      style (queue item 343, ADR 0034), what `element.style` wrote exempt by
      its text and a refused attribute read through it as `""`
      · Decided (ADR 0038, queue item 366): what a script reads of the
      page's size — `innerWidth`/`innerHeight` from `Page::viewport`,
      `scrollX`/`scrollY` from a scroll position the renderer holds at zero,
      `scrollWidth`/`scrollHeight` from the layout the page would be drawn
      with when asked, through a `View` trait the renderer implements;
      nothing beyond the window
      · Built: the window's viewport (queue item 366) — `alo-bindings`'
      `view.rs`, the `View` trait and `show`, and
      `interface/window_cssom_view.rs`, `innerWidth`, `innerHeight`,
      `scrollX`, `scrollY`, `pageXOffset` and `pageYOffset` as the window's
      own `[Replaceable]` accessors, asked at every read and refusing by
      name with no view; `alo-renderer`'s `view.rs`, `PageView`, set at load
      and at every `Resize`, its scroll position zero. alo Sites' analytics
      script reports `&p=%2F&w=800` at 800 × 600
      · Built: an element's scrolling area (queue item 370) —
      `interface/element_cssom_view.rs`, `scrollWidth` and `scrollHeight`
      on `Element.prototype`, asking the `View`'s third question;
      `alo-renderer`'s `easel.rs`, what a page is drawn with and its last
      drawing, shared by the renderer and the view, so a script is measured
      mid-task by the one pipeline and the drawing kept for the next paint;
      `scrolling_area.rs`, CSSOM View's steps without quirks; and
      `alo-layout`'s `BoxGeometry::reach` and `scrolling_area`, overflow
      toward the end edges only. alo Sites' `pagehide` listener reports
      `d=1000&p=%2F&w=800` and then `t=0`
      · Owed: the size a tab no window shows is told (371, opened by the
      first embedder that opens one), a measurement stopping before paint
      (372, a speed),
      `el.style[0]`, the indexed getter (345), and `getComputedStyle`,
      `document.styleSheets` and the rest of item 89, each opened by a page
- [ ] Storage: `localStorage`, `sessionStorage`, IndexedDB, the Cache API, and one quota policy over all of them
      · Built: `alo-storage`, the browser process's store (queue item 301).
      It holds buckets keyed by origin and top-level site, and its key type
      cannot hold an opaque origin. A ledger holds each bucket to its quota
      and the profile to its bound, evicting whole buckets least recently
      used first. `localStorage` areas are kept in checksummed records that
      survive a restart. A bucket that fails its check is set aside and
      recorded. Clearing a site is one removal, and a session-scoped store
      writes no file. The profile's bound is measured from the store's own
      volume through a rented `statvfs` (306), and a volume that cannot be
      asked bounds it at zero.
      · Owed: no page reaches it yet. Still to come are `localStorage` and
      `sessionStorage` in a page (302), `navigator.storage` (303),
      IndexedDB (304) and the Cache API (305). Decided in ADR 0025. All four kinds
      of storage use one bucket per origin and top-level site, held only by the
      browser process, and an opaque origin has none. One fixed 1 GiB quota
      covers each bucket. It is reported the same on every machine and in
      private browsing, and `localStorage` is capped at 5 MiB within it. Past
      the profile's own unreported bound, whole buckets are evicted, least
      recently used first, never one in use. `sessionStorage` is never written
      to a disk, and a corrupt bucket is set aside, never served in part.
      IndexedDB's store needs its own ADR
- [ ] Workers: dedicated, shared, and service workers with their fetch interception
- [ ] Timers, clipboard, drag and drop
- [ ] ★ **Permissions as capabilities** — camera, microphone, location, notifications, in the shape of `alo-os` ADR 0001: enumerated, visible, revocable, expiring, recorded. A browser is where most people meet a permission prompt, and every other one is a dialogue nobody can audit afterwards
      · Built: the grant table, `alo-grants` (queue item 307). It holds the
      closed list of eight capabilities and decides an ask: refused without a
      prompt for an opaque origin, an insecure context, a frame its embedder
      did not allow, a refusal everywhere or remembered, a document already
      refused, or no gesture. It keeps *allow while open* until the document
      goes and *allow on this site* or *don't allow* until thirty days after
      the person's own last visit, which a page's use does not extend, and a
      clock set backwards ends it. Revoking stops every use in the same act.
      Only a person's cause answers or revokes. The record is sixty-four
      entries per key with a counted drop, and the file is set aside whole
      when it does not read. A private session's table has no directory.
      · Owed: the prompt and the indicator (308, behind 297), and each
      capability's API (303, 157, 92, 290 or a page), which is also where the
      browser process refuses an ask naming a key it did not load into that
      renderer (§ 7). Decided in ADR 0026. A permission is
      a grant in one table the browser process holds: one capability from a
      closed list, to one origin under one top-level site, made by a person
      answering an ask a page made while somebody was using it. It ends when
      the page closes or thirty days after the person last opened the site,
      with no *always* and no shipped allowlist. It is shown in one list and
      in the tab strip while a device is in use, revoked in one act that stops
      the device at once, and recorded without what it carried. No agent can
      make, answer or revoke one. Cut into queue items 307 (the grant table,
      `alo-grants`) and 308 (the prompt and the indicator, behind the tab
      strip); each capability's API arrives with its own item or page

### CSS beyond what alo needed

- [ ] Animations and transitions
- [ ] Container queries, `:has()`, cascade layers, `@property`
- [ ] Filters, `backdrop-filter`, blend modes, masks, `clip-path`
- [ ] `position: sticky`, multi-column, scroll snap, overscroll behaviour
- [ ] Writing modes, and layout that is right-to-left rather than mirrored afterwards
      · Built: `padding-block` and `margin-block`, split in `alo-css` into
      the top and bottom they are under `horizontal-tb`, the one writing mode
      laid out (queue item 379, opened by `alo-sites-features`; corpus case
      `block-axis-spacing`). The inline axis and every logical margin and
      padding longhand, mapped to their sides in `alo-css`' `logical.rs`
      under left to right, said to be left to right's (queue item 380,
      opened by `alo-sites-text-image`).
      · Owed: `writing-mode` and `direction` themselves, read by nobody yet;
      a right-to-left page's inline-axis properties (421); moving the
      logical mapping to computed-value time once a box can be other than
      `horizontal-tb` and left to right; and layout that is right-to-left
      (100)
- [ ] Paged media and print styles

### Text, properly

- [ ] Bidirectional text end to end — stage 1 shapes it; this makes selection, caret movement and editing behave
- [ ] Selection, carets and text input inside the page
- [ ] **Input methods.** A browser that cannot take Japanese or Chinese input is not a browser in those countries
- [ ] `contenteditable`, which every rich text box on the web is built on
- [ ] Hyphenation, `text-wrap: balance`
- [ ] Web fonts as pages ship them: WOFF2, variable fonts, and loading that does not flash

### Pictures, and things that move

- [ ] Image codecs, rented: PNG, JPEG, GIF, WebP, AVIF
      · Built: PNG (queue item 106), JPEG (177), and GIF and WebP (180), in
      `alo-paint` behind `picture::read`. Each rented decoder is pure Rust,
      named in one file, and `scripts/gate.sh` checks that. The format comes
      from the bytes rather than the name. One size bound, `agreed_size`, is
      asked of every size a file declares before anything is reserved for it:
      a GIF's screen and its frame, and a WebP's canvas and each lossy
      bitstream inside it. An animated GIF or WebP is drawn as its first frame.
      Frozen files in corpus cases `a-picture` and `a-picture-in-each-format`
      go through one list of truncation and corruption tests. Since queue
      item 594 (iteration 358) a picture drawn no smaller than it is is
      smoothed across its texels as Chrome 154 smooths it — bilinear onto
      its box as drawn, the weights in 256ths, a mix rounded a
      thirty-second of a level towards the middle (`alo-paint`'s
      `bilinear.rs`) — so every opaque pixel of 37 probes at one and at two
      is Chrome's, and so is every pixel of every such picture in
      `pictures-sized-on-a-line`. Since queue item 600 (iteration 364) a
      translucent one is too — each texel premultiplied to a level, the mix
      held in sixteenths of a level and put onto the pixel beneath in one
      32-bit fused multiply-add (`alo-paint`'s `mix_onto.rs`) — so
      `translucent-pictures-drawn-larger` is Chrome's in every pixel at one
      and at two. Since queue item 179 (iteration 367) a picture drawn
      smaller on both axes goes through Chrome 155's mipmap levels — the
      copy it uploads for the box, Skia's levels, two of them mixed by the
      larger ratio's logarithm as a GPU takes it, and, as Chrome's decode
      cache holds one picture per file, every later draw of a copy that has
      levels mixed too (`alo-paint`'s `mip_level.rs` and
      `picture_filter.rs`) — so every pixel of `pictures-sized-on-a-line`
      is Chrome's at one and at two, and `pictures-drawn-smaller` all but
      four a level apart where the mix is a tie
      · Owed: a turned picture (613); the copy where an odd side is halved
      (611), a translucent picture drawn smaller (612) and levels made for
      a copy first drawn without them (614); a translucent picture over a pixel that
      is not opaque, inside a group at an opacity (not measured);
      `image-rendering` (601); AVIF (269: decided in ADR 0021, `avif-parse` and `rav1d`'s safe
      Rust API; blocked until `rav1d` releases that API, because its newest
      release offers only a C interface this repository could call only with
      `unsafe` of its own); playback of an animation (109)
- [ ] **SVG** — a second rendering model inside the first, and far larger than its one line here suggests
      · Built: the `<svg>` box (item 270) — an outermost `<svg>` is a replaced
      box in `alo-box`'s `svg.rs`, sized by `alo-layout`'s `replaced.rs` from
      CSS, its attributes, its `viewBox` ratio or 300 × 150, with no boxes
      inside it, and one agent image named by ARIA or its `<title>`; corpus
      case `an-svg-box`; filled shapes (item 271) — the crate `alo-svg` turns
      what an `<svg>` holds into an `alo_paint::Drawing` handed to paint by
      box: `rect`, `circle`, `ellipse`, `polygon`, `polyline`, `<g>`, the
      `transform` attribute, `viewBox` and `preserveAspectRatio`, `fill`,
      `fill-rule`, `fill-opacity` and `opacity`, presentation attributes in
      `alo-style`'s cascade (`presentation.rs`), every count bounded; corpus
      case `svg-shapes-filled`; path data (item 272) — `<path>` drawn from
      `alo-svg`'s `path_data.rs` (every command, drawn up to its first error,
      65 536 segments per path) with arcs as cubic curves by `arc.rs`; corpus
      case `svg-path-data`; strokes (item 273) — `stroke` and every
      `stroke-*` property read by `alo-svg`'s `stroke.rs` and `dashes.rs`,
      outlined in user space by `tiny-skia`'s stroker and dasher behind
      `alo-paint`'s `raster.rs` (`outline`), dashes counted and bounded before
      they are cut; corpus cases `svg-strokes` and `alo-offline`, alo's own
      offline screen frozen byte for byte with its hand drawn; relative sizes
      (item 279) — `width` and `height` on an `<svg>` are presentation
      attributes in `presentation.rs`, so a per cent or an `em` sizes the box
      through the cascade and a stylesheet beats it; corpus case
      `svg-relative-size`; the `transform` property (item 287) — the
      attribute is its presentation attribute, read by `alo-value`'s
      `svg_transform.rs` and held by the cascade as a `matrix()`, so a
      stylesheet replaces it; `alo-svg`'s `transform.rs` measures it against
      `transform-box` (`view-box`, or a shape's object bounding box from
      `bbox.rs`) about `transform-origin`; corpus case
      `svg-transform-property`; an SVG file as a picture (items 309 and 310,
      closing 277, ADR 0027) — `alo_dom::read_svg` reads it behind
      `quick-xml` in `alo-dom`'s `xml.rs`, `alo-renderer`'s `Resource`
      carries the type beside the bytes so only `image/svg+xml` is read as
      SVG, and `svg_picture.rs` draws it through `alo-svg` at the `<img>`'s
      content box with its own cascade, confined there, and handed nothing
      it could fetch with; corpus case `alo-meet-greeting`, alo Meet's
      greeting with its waving hand
      · Owed: `fill-box` on a `<g>` and `stroke-box` (288); a per cent on
      an inline-level `<svg>`, resolved twice like any inline-block's (284);
      a nested `<svg>` (278), `<use>` (274), SVG paint servers (275),
      `<text>` (276), each when a page needs it; and an SVG file in
      `background-image` (311), when a page needs one.
      Decided in ADR 0022. An `<svg>` is one
      replaced box with no boxes inside it, and its contents become a drawing
      of paths, made by a new crate `alo-svg` after layout and handed to paint
      by box. Strokes go through the `tiny-skia` already rented. Cut into queue
      items 270–273 (the box, filled shapes, path data, strokes), which close
      on alo's own offline screen and its inline `<svg>` hand. Items 274–277
      (`<use>`, gradients, `<text>`, and an SVG file as a picture, which needs
      its own ADR because it is XML) open when a page needs them
- [ ] Canvas 2D
- [ ] Audio and video playback through rented decoders
      — not started: nothing is built. Decided in ADR 0023. Media is decoded
      in a media process per site, started by the browser process, confined by
      the renderer's own sandbox profile and holding nothing of the page but
      the bytes it was handed. Audio is Symphonia (MPL-2.0, safe Rust) for
      Vorbis, FLAC, PCM and MP3; Opus waits for a decoder that can be rented in
      Rust, AV1 for the `rav1d` release AVIF waits on, and VP8 and VP9 have no
      Rust decoder at all; AAC, H.264 and H.265 are never played, and every
      "can you play this" answer is derived from what decodes. Cut into queue
      items 289–292 (audio decoded, the media process, `<audio>`, sound to the
      device), each opened by a frozen page that plays a file, and 293–295
      (Opus, AV1, VP8 and VP9), each blocked on a decoder
- [ ] Media Source Extensions, without which most video sites do not play at all
- [ ] Web Audio
- [ ] WebGL, then WebGPU — both large, both late, and neither before the software path is right

### Making it fast enough to use

Correctness before speed is the rule everywhere else in this file. These are the
items where being right and being unusable are the same outcome.

- [ ] **Incremental style and layout** — recompute what changed, not the document. The largest single difference between an engine that renders a page and one somebody can use
- [ ] Compositing layers, and scrolling that does not repaint the world
- [ ] Off-main-thread scrolling and animation
- [ ] Hardware acceleration for paint, once the software path is correct
- [ ] A performance budget somebody can hold us to: named pages, measured, in CI

### The browser itself

- [ ] A window, tabs, and a tab strip
      · Built: the window (296, `alo-window`, binary `alo`, decided in ADR
      0024). `winit` and `softbuffer` are rented through safe functions
      checked in their source, each named in one file. A conductor thread
      owns `Tabs` and is the only thing that waits on a renderer. The window
      is composed by a function from the frames it was last sent, with four
      reference renders. A test shows a renderer stopped with `kill -STOP`
      leaves the window composing its last frame at once and then its
      sentence. Closing the window stops every renderer. `--frozen-fonts` makes a
      renderer process paint a frozen alo page identical to the corpus's
      committed render. Started on this Mac and captured with
      `screencapture -l`, the window is its composition: every opaque pixel
      within one level once put through the display's colour profile, and
      99.86% exact (296, done). The tab strip is a document the engine
      renders (297, done): `ToRenderer::Strip` carries the tabs as data to a
      confined renderer at `alo:tab-strip`, whose opaque site no page can
      name, which builds the strip from a compiled-in template with
      `alo-dom`'s operations, each title a text node, and draws it with a
      stylesheet after alo's tokens. Three reference renders, each tab's
      box in numbers, the agent reading four tabs with the second
      selected, and five hostile titles kept as text in the tree and the
      render. The strip is in the window (528, done): `Tabs` keeps a
      selected tab, each tab's `<title>` from its load (`alo-dom`'s
      `title.rs`, bounded on the wire) and the strip's renderer; the
      conductor redraws the strip when it changes and as a load starts;
      the window composes it in a 36 px band above the page, with reference
      renders of one, three and a stopped strip; a hostile and a
      100,000-character title reach the strip through the real renderers;
      a strip renderer stopped with `kill -STOP` leaves the window composing
      and the tabs working; captured on this Mac, every composed colour maps
      to one captured colour. The browser's keys (529, done): ⌘T, ⌘W,
      ⌃Tab and ⌃⇧Tab are read by the window's event loop (`keys.rs`,
      exact modifiers, repeats ignored) and choose tabs in the conductor
      (`conductor/choosing.rs`); choosing tells the tab left `hidden`
      before the tab chosen `visible`, shown by the order of the pages'
      fetches; a tab chosen replaces what the window shows (`News::Chosen`,
      a new reference render); closing the last tab closes the window; on
      this Mac each key was pressed in the real window and captured.
      Where a pointer falls (298, done): the display list records each
      box's area in paint order (`alo-paint`'s `DisplayItem::Area`, drawn
      as nothing), `hit.rs` finds the box drawn last over a point's pixel
      through the list's transforms and clips, and `alo-renderer`'s
      `hit.rs` turns it into the element a person pointed at; 21 pages
      put to Chrome 154's `elementFromPoint` and mouse events, each box
      the points depend on asserted in numbers.
      A person's click reaching a page (535, done): the window keeps the
      cursor and aims a press of the main button from its own placement
      (`aim.rs`); the conductor sends it to the selected tab as
      `ToRenderer::Pointer`, the one message carrying a point, and the
      release to the tab the press went to (`conductor/pointing.rs`); the
      renderer hit-tests and fires Chrome 154's `pointerdown`, `mousedown`,
      `pointerup`, `mouseup` and `click` as two tasks with their measured
      members (`event_loop/pointed.rs`), the click at the nearest node both
      ends are in and HTML's activation steps around it; a link clicked is
      decided as `Cause::Person` and said in the window. Pinned against
      CDP measurements, through confined renderers on the frozen
      `web-example-com` (the same address as the agent's `Activate`), and
      clicked with a real mouse event in the window on this Mac.
      The pointer moving (542, done): the window sends a move over the
      page, its leaving once, and every move of a drag the page pressed
      (`cursor.rs`), the conductor coalescing moves that arrived together;
      the renderer keeps the node last under the pointer (`pointed.rs`)
      and fires Chrome 154's crossings and moves as one task
      (`crossing.rs`, `event_loop/crossed.rs`), `relatedTarget` and all,
      a press or release crossing first. Pinned line for line against
      Chrome's own record of the same page, through confined renderers in
      the window, and moved over with a real cursor on this Mac.
      `:hover` (550, done): the document keeps the element the pointer is
      over (`alo-dom`'s `hover.rs`), set before the crossing fires and
      moved to the parent an element was taken out of; style and
      `querySelectorAll` match it and its ancestors, and the page is drawn
      again at each crossing. Pinned against Chrome's record of `:hover`
      in every listener, and three reference renders that are Chrome's
      screenshots pixel for pixel. A hovered `<label>` hovers the control
      it labels (554, done), by HTML's labelled-control rule in `alo-dom`'s
      `label.rs`, which the agent's names now ask as well; pinned against
      Chrome's record and two reference renders, with Chrome's stuck hover
      after a label changes deliberately not copied. A person's click on a
      submit button sends its form (543, done), as an agent's press does and
      decided as the person's, on the frozen booking and pizza forms; the
      window says it as a form sent. A press on a tab in the strip chooses
      it (539, done): the window sends it as `Order::StripPressed`, the
      strip's renderer claims the tab by its place (`strip_claim.rs`,
      `FromRenderer::Claimed`), and the browser decides it against the tabs
      the strip showed when it last painted (`strip_decide.rs`), refusing a
      place not shown, a tab closed since and a claim with no press behind
      it by name; chosen as a key chooses, shown by the order of the pages'
      fetches, through confined renderers and clicked on this Mac. A page
      is laid out for a screen's scale (560, built; ADR 0057): the scale
      rides on `Viewport` and `MediaContext`, and a line's font figures,
      its leading, a set line height, the 1/64 px grid, a border's width,
      a text field's width and the size a face is set at all round in
      device pixels, reported in CSS pixels; of the 580 boxes of the
      corpus's 52 markup cases this engine places exactly where Chrome 154
      does at one, 578 are exactly where Chrome puts them under a real
      scale of two, and nothing at one moved. Such a page is painted at its
      scale (561, done): the display list says its scale and is drawn
      under it onto a canvas of device pixels, letters outlined at the size
      shown, and `checkbox-beside-text`'s nine controls fall on exactly
      Chrome's device pixels at two (`render-at-2.png` beside `render.png`).
      A renderer is told its page's scale with the page (563, done), on
      the wire and refused or made one when it is not a scale, and lays
      the page out and paints it there: the 21 corpus cases whose page
      runs script put all 313 boxes exact at one exactly where Chrome puts
      them at two. The window is at its screen's scale (562, done): a
      `Resize` carries a `Screen`, the conductor tells the page and the
      strip the window's whole scale, both paint in device pixels and the
      window places their frames one to one, replicating nothing; a window
      moved to a screen of another scale has both drawn again there; and
      `alo` on this Mac's Retina display, captured with `screencapture -l`,
      is its composition at two in every opaque pixel within one level.
      A screen at 1.5 is taken at 1.5 rather than one (599, done): the
      window's band, lines and viewport are its lengths at the scale, and
      `alo --scale 1.5`, captured on screen, is its composition at 1.5 in
      every opaque pixel; a factor nobody measured is shown at the largest
      measured scale below it.
      A page is told its scale (567, done): `devicePixelRatio` answers it
      and `resolution`, `min-resolution` and `max-resolution` match it, as
      Chrome 154 answered 2,984 queries at one and two, and
      `a-page-told-its-scale` is Chrome's in every box at both. A box's
      own area — background, border, shadows — is drawn on whole device
      pixels at one and at two, as Chrome draws it (565, done). A line
      under, over or through text is drawn on whole device pixels, a tenth
      of the size thick, as Chrome 154 drew all 480 lines measured at one
      and at two (566, done): `text-decorations` is on Chrome's rows at
      both. An underline or an overline steps aside where a letter's ink
      crosses it, as Chrome 154 does, every overline gap and 1,014 of 1,022
      underline gap edges measured exactly (579, done). A decoration runs
      under a space its line goes on past and not under one that ends the
      line, as Chrome's does (580, done): layout's `Fragment::space`. So
      does a space that is a text box of its own or begins one, and not
      one where an ellipsis ends the line (582, done): layout's
      `LineBox::spaces`. Before an ellipsis a line stops at the last
      letter kept, never under the space after it, as Chrome 154's does
      (583, done): layout's `Cut::width`, and `text-overflow-ellipsis`
      is Chrome's underline in a frozen Chrome picture.
      `text-decoration-skip-ink` is read as the text's own, inherited:
      `none` draws whole and `all` as `auto`, and an underline and an
      overline are drawn beneath the letters, as Chrome 154 draws them
      (581, done): `alo-paint`'s `SkipInk`, and `skip-ink-values` is
      Chrome's at one and at two. Under text that casts a shadow the
      letters' shadows are drawn first, then the underline's and
      overline's shadows, those lines, the letters, and the line through's
      shadow and the line through, each line casting the text's own
      `text-shadow`, as Chrome 154 draws them (584, done): `alo-paint`'s
      `build.rs` (`between_lines`, `draw_lines`), and
      `text-shadow-on-lines` is Chrome's at one and at two.
      · Owed: telling the person why a form was not sent (557); an image
      button's point (556); the agent reading the strip, which needs design (531); a
      title changed after load (532); `pointerrawupdate` (551), a page changing under a still pointer
      (552), a tab chosen under the pointer (553); the other buttons (544), modifiers (545) and click counts
      (547); the strip's close and new-tab controls (558); device pixels
      (299), decided by ADR 0057 — a page is laid out for its screen's
      scale, every whole-pixel rule rounding in device pixels as Chrome 154
      was measured to at a real scale of two — and owed as where a
      person's pointer lands at two (568), `matchMedia` (570), a media
      query's range syntax, boolean form and `calc()` (571), a window wider than 4,096 device
      pixels (569), a shrunk flex item's share of a pixel (564, measured,
      built with 502), and the two boxes at two that keep 560 open;
      a link opening a tab
      (300)
- [ ] The address bar: what somebody typed, what it means, and a search that phones nobody by default
- [ ] History, bookmarks, downloads
- [ ] Find in page, zoom, and per-site settings that stick
- [ ] Context menus, and keyboard operation of every one of them
- [ ] Printing, print preview, export to PDF
- [ ] Viewing a PDF — or saying plainly that we hand it to something else
      — not started: nothing is built. Decided in ADR 0028: stage 2 has no
      PDF viewer, and says so. A response typed `application/pdf` or
      `text/pdf` reaches no renderer and no parser of ours; the browser
      process offers it to the person as a file, in its own words, keeps the
      tab's page, and writes nothing until the person chooses where. Opening
      the saved file in another program is a person's act, never a page's or
      an agent's. alo serves every PDF as an attachment on purpose, so no alo
      screen needs a viewer. One is reopened only if a person's stage 2 week
      names PDFs, and then rented in Rust, in a process of its own, with no
      PDF JavaScript and its text in the agent's tree. Cut into queue items
      317 (a navigation to a PDF offered as a file, behind 85 and 120) and
      318 (`navigator.pdfViewerEnabled` and the empty plugin lists)
- [ ] Private browsing, and profiles that are genuinely separate
- [ ] Autofill, and credentials held where the operating system holds secrets rather than in a file of ours
- [ ] Security surfaces: certificate detail, permission state, what this page has stored — reachable, none of it buried
- [ ] Settings
- [ ] **Developer tools**: inspector, console, network, performance. A browser nobody can debug a site with is not one a developer keeps
- [ ] **Accessibility**: AT-SPI over the same tree the agent reads (ADR 0002), keyboard operation of everything, focus always visible, and the EN 301 549 conformance the workspace is already held to

### ★ The agent, on somebody else's pages

- [ ] The agent reads and acts on ordinary web pages through the same tree — no screenshot, no scraping, no coordinates
      · Built: on two frozen pages nobody wrote for us (queue item 131,
      iteration 295). `web-a-form`, the HTML specification's pizza form,
      is read by the words on it, and its fields, textarea, radios and
      checkboxes are filled and read back. `web-first-website`'s links are
      followed by their words, to the URL resolved where the page was
      served. The same refusals hold there as on alo's own screens
      (`alo-corpus`' `an_agent_on_somebody_elses_page.rs`, with a reference
      render of the filled form). What the form found is fixed:
      - a field's text inside its `<label>` is read, which it was not;
      - text put into a `<textarea>` is held, drawn and read, which it was
        not;
      - a `time` field refuses text by name rather than taking it unseen.
      **And the form is sent by its button** (queue item 468, iteration
      296): pressing *Submit order* checks it and asks to `POST` to
      `https://httpbin.org/post` with the entries Chrome 154 sends, in its
      order; an address that is not one stops it with Chrome's words
      (`a_form_on_somebody_elses_page_is_sent.rs`).
      The browser process then decides it as the agent's, with Chrome's
      exact body (queue item 470, `a_form_is_decided.rs`).
      · Owed: loading what was decided, which is item 85's for every
      navigation; `time`, `month`, `week` and `datetime-local` fields
      (388); and a stranger's page that runs
      script, which neither frozen web page does — though alo Sites' frozen
      booking page now runs its own script through the agent's press and
      submission (queue item 485, iteration 298), and an agent reads which
      of its fields would be refused before pressing, as Chrome's
      accessibility tree says it (queue item 476, iteration 301)
- [ ] Across frames, without becoming a way around the same-origin policy
- [ ] Under grants, and recorded: what it read, what it did, on whose approval — `alo-os` ADR 0001's model reaching the web
      · Built: the record half for an agent outside the browser (queue item
      496): the browser process writes what each one read and did, refused
      requests included, and nothing over the socket can read it. A
      person keeps every agent to the sites they name (`--agent-site`,
      queue item 497): any other is refused by its site before a tab is
      looked at, a redirect or a page that navigated itself is refused
      where it ended, and every entry in the record names the site.
      · Owed: `alo-os`'s grant (133); sites per agent rather than for every
      agent at once, which needs an agent to have an identity the socket
      does not give it
- [ ] Agent-driven navigation, and a page that changes underneath an agent mid-action

**Exit gate — two gates, met separately and dated separately** (ADR 0052). It
was one sentence with a semicolon in it, and read as one gate the smaller half
disappeared behind the larger.

- [ ] **The agent's gate.** An agent completes a real task on a site nobody
      wrote for us, and the record afterwards says what it read and what it
      changed. This needs a page's *structure* read correctly and its controls
      nameable — not pixel fidelity — and queue item 131 already does it. What
      it waits on is not in this repository: alo OS's argument model has no
      kind for a web address and no grant narrower than an application
      (item 484).
      · Built: the door an agent outside the browser comes in by (queue
      item 496, ADR 0055). A separate process reads the frozen pizza form
      as a tree over the browser's socket, fills it and sends the order by
      name, and the record the browser process wrote says what it read and
      what it changed (`alo-window`'s `an_agent_outside_the_browser.rs`).
      An agent can be kept to the sites a person names (queue item 497),
      and a person watching the window reads each verb's sentence in a
      line of the browser's own (queue item 623), sees the box it acted
      on ringed on its own rectangles (queue item 624), and reads why a
      verb was refused, each refusal its own line (queue item 625) — an
      ambiguous one naming the nodes it could mean as `box#7 or box#14`,
      as every other sentence names a node, rather than in Rust's own
      notation (queue item 626). A scroll is answered only where Chrome
      would have something to scroll, by half a device pixel, so a plain
      button is refused (queue item 645) — and only of a scroll container,
      so a box that spills under `overflow: visible` is refused too (queue
      item 661). The root answers for the viewport, from the page's
      scrolling area against the window, and a body whose `overflow` the
      viewport takes neither scrolls nor clips (queue item 662). A
      `<textarea>` keeps its lines and is a scroll container, as Chrome
      155 computes it, so one holding more than it shows is scrolled (queue
      item 663), and cuts a word too long for its line onto the next, so
      it is not scrolled across to it (queue item 666).
      · Owed: a list box that shows its options and scrolls to the rest
      (665), `text-overflow` on a box whose `overflow` the viewport takes
      (664); loading where a verb leads, which is
      item 85's for every navigation; a box's id kept across a verb that
      adds a box (500); and Windows, which ADR 0055 leaves undecided.
- [ ] **The person's gate.** A person uses it as their browser for a week and
      reaches for another one only for a site they can name. This is the one
      that is years away, and ADR 0052 § 5 measures where those years are: not
      in CSS, where the gap against real pages is about ten properties, but in
      ES modules, Shadow DOM and CSSOM — items 77, 87 and 89, all awaiting a
      decision rather than work.

Neither gate may be ticked by the loop. Both are judgements a person makes.

---

## Stage 3 — the legacy tail

Deliberately last, and possibly never finished — a choice, not a failure.
Refusing this list is what made stages 1 and 2 survivable. Keep a corpus of
sites people actually use and let a broken render schedule the work.

- [ ] Quirks mode
- [ ] **Floats as layout**, and CSS table layout — the two that most often turn an old page into a column of rubble
- [ ] `document.write`, live `HTMLCollection`s, and the DOM as it was before it was a specification
- [ ] Legacy character encodings, and detecting them
- [ ] XML, XHTML and XSLT
- [ ] `frameset`
- [ ] Vendor prefixes, and anything that exists only for a page written before 2015
- [ ] The sloppy-mode corners of JavaScript that only old code reaches

---

## Stage 4 — a browser somebody chooses

Product work, and gated: **nothing here starts until stage 2's exit gate is
met.** It is listed rather than left unnamed because "not scheduled" had begun
doing the work of "not thought about", and these decide whether anybody switches.

- [ ] Extensions — and the decision, in an ADR, whether that means the WebExtensions API or something narrower we can actually secure
- [ ] Sync, self-hosted: bookmarks, history, tabs and passwords, end-to-end encrypted, on the customer's own server
- [ ] Updates that are signed, staged and reversible
- [ ] A mobile port
- [ ] Crash handling that helps us fix it without becoming telemetry
- [ ] ★ **Translation on the machine.** alo already runs models locally; a page translated without sending it anywhere is the sovereign version of a feature every other browser sends to a server
- [ ] ★ **Reading and summarising a page locally**, under the same grants and the same record
- [ ] Enterprise: policy, managed configuration, and an update mirror an organisation hosts

**Exit gate.** Somebody outside alo chooses this browser, on a machine we did not
set up, and stays.

---

## Not built, and not by accident

Stated here so that no later stage quietly adopts them:

- **DRM and Encrypted Media Extensions.** A proprietary binary with privileges inside a sovereignty product is a contradiction. Sites that require it will not play, and we say so rather than shipping a black box.
- **Proprietary codecs** we cannot ship freely.
- **Telemetry.** Not "anonymised telemetry". None — the rule alo OS already holds.
- **A search deal.** The address bar's default is decided for the person using it, not sold.
- **Our own shaper, codec or TLS stack.** We rent the physics, as every engine does.
- **A conformance percentage as a target.** The measure is alo, then real pages that fail.
