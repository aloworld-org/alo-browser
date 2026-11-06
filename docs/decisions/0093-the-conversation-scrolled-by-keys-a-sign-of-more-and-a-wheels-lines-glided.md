# ADR 0093 — The conversation scrolled by keys, a sign of more, and a wheel's lines glided

**Status:** accepted — built by queue items 754, 755 and 756 (1 November
2026); § 2's look confirmed by the owner, with a fade, on 6 November 2026
(793, amendment below)
**Date:** 2026-11-01
**Context:** ADR 0085 § 3 (the conversation is scrolled from the newest
turn's top by one number of the browser process's, kept inside the reach
the renderer measures); ADR 0024 § 5 (a person's pointer is a coordinate,
and the browser process decides where it lands); ADR 0002 (the layout tree
is the agent's tree; no verb takes a coordinate); queue items 703, 718, 754,
755 and 756; `docs/design/shell.md`, *The conversation*

## The question in one line

How does a person scroll the conversation without a wheel, how do they see
that there is more of it than the room shows — and an agent with them — and
how does a wheel's notch move it, if not at once?

## 1. Keys (item 754)

**Page Down and space** go on by **a room less a line** — the room is the
conversation's (`conversation_scroll::room`, 676 in the design's 820-tall
window), less 40 — so the line at the room's foot is at its top after and a
person keeps their place; never less than a line, however short the room.
**Page Up and Shift-space** go back by as much. **Home** goes to the first
turn's top, **End** to the newest turn's foot. A key held down repeats; ⌘,
⌃ or ⌥ with one of them is not this.

They scroll **only while nothing holding the keyboard would take them**:
never while the card's page has the keyboard (718), never while *Search
chats* has it, and never while the one box has it **and holds what a person
typed** — there a space is a space between their words. An empty box gives
them to the conversation, since none of them would edit it, as does a
window where nothing has the keyboard.

Measured on the way, for comparison and not adopted: Chrome 156 on this
Mac moves a page 613 tall by 593 for Page Down or space; the item's *a room
less a line* is the rule here. A key's scroll is made at once; Chrome glides
its keyboard scrolls too, on a curve not measured here (792).

## 2. A sign that it goes on (item 755)

The item offered three: a scroll bar, a fade at the room's edges, or *Back to
the newest*. **A bar** is built, because it says both things at once — that
there is more, and which way and how much — and because it needs nothing to
stand over the turns: it stands in **the room's right gutter**, the 32
pixels the clip keeps beside the column for the card's shadow, where nothing
can be pressed. A fade would lie over the turns' edges, and the hit test
finds the topmost element — a fade there would take presses meant for a
title under it; this engine has no `pointer-events` to let them through.
*Back to the newest* says only one of the two directions, and is a control
to design.

The bar is **6 wide, rounded 3, 13 in from the clip's right edge** (centred
in the gutter), in `--bar`, `#cbc3b6` — the line's colour a step darker, so
it reads on the paper without competing with the turns. It is **as long
against the room as the room is against the whole conversation** (never
under 32), and **as far down the room as the conversation is scrolled**,
each on a device pixel; it is drawn only while the turns do not fit the
room. Its numbers are **the browser process's** (`conversation_scroll::bar`,
from its own offset and the reach the renderer last measured), sent with
the first screen and refused past any room on the wire — never a renderer's
word, as the offset itself is not (ADR 0085 § 3). It is pressed by nobody:
dragging it, or pressing above or below it, is not built (793).

**An agent reads it** as an image named in words: *The conversation goes on
above and below; scrolled 50% of the way down* — or *above*, or *below* —
from 0 at the first turn's top to 100 at the newest turn's foot. Every turn
is still read wherever it is scrolled (ADR 0085 § 5); this says where the
room is among them.

## 3. A wheel's lines glided as Chrome glides them (item 756)

**Measured**, not read: Google Chrome 156.0.8078.76 on this Mac, a page
40 000 tall, a one-line wheel `CGEvent` posted at the page with the
pointer over it, `scrollY` sampled at every animation frame, at device
scales one and two. A notch is a `wheel` event of 40 CSS pixels
(`deltaMode` 0), and the page **glides**:

- on CSS's **ease-in-out**, `cubic-bezier(0.42, 0, 0.58, 1)`;
- for **`14 − |Δ| ÷ 60` frames of a sixtieth**, `Δ` the distance in
  **device** pixels, held from 6 frames to 12 — a line 200 ms at one and at
  two; three lines 200 ms at one and 167 at two; five 178 and 122; ten 122
  and 100 (fitted 195.5, 195, 175.5, 121; 197, 163.5, 119, 92 from samples
  16.7 ms apart);
- standing at each frame **on a whole device pixel**.

Every sample of one and three lines fits that exactly, and of five and ten
lines within one CSS pixel (the fitted start and length free to a
millisecond). **A notch while gliding** moves the end on by its lines and starts
again from where it stands, **as fast as it was going** — the new curve's
first control point `(0.42, 0.42 × s)`, `s` the speed it had as a share of
the new distance over the new duration: two notches 100 ms apart fit within
1.5 CSS pixels at every frame at one and two, where starting again from
rest misses by up to 8.

So the browser process glides a wheel's lines the same way
(`conversation_glide.rs`): the window steps the glide at every sixtieth of
a second while it is under way (`about_to_wait`, `ControlFlow::WaitUntil`),
each step a new offset kept inside the reach and drawn; the drawing thread
draws only the newest it was asked. **A trackpad's pixels are not glided**:
macOS sends them already smooth, momentum and all, and they are followed as
they come, ending a glide under way — as Chrome follows them. A new turn, a
key or the reach measured anew ends one too.

The frames are a timer's sixtieths, not the display's own refresh: on a
120 Hz display Chrome steps twice as often along the same curve. Where the
glide stands at a moment is the same either way; how often it is drawn is
queue item 794.

## Consequences

- `alo-window`: `conversation_keys.rs` (which keys, how far),
  `conversation_glide.rs` (the curve, its duration and its retargeting),
  `keys.rs`' Space, Page Up, Page Down, Home and End read apart by the
  window, `Thread::scroll_to_end`, `glide_lines`, `glide` and `bar`,
  `Conversation::keys_scroll` and `scroll_by_key`, the window's `key`,
  `wheeled_at`, `gliding`, `glide` and its event loop's `about_to_wait`.
- `alo-renderer`: `conversation_scroll::Bar` and `bar`, `Start::bar` on the
  wire, the bar built into the conversation (`conversation_document.rs`)
  and drawn by `start.css`.
- Tests: the keys and their refusals (`conversation_keys`, `conversation`,
  `conversation_window`), the glide against Chrome's frames
  (`conversation_glide`), the bar's numbers (`conversation_scroll`), its
  box, pixels, reading and press, two reference renders
  (`alo-renderer/tests/a_sign_that_the_conversation_goes_on.rs`), the wire,
  and the window's own road
  (`alo-window/tests/a_conversation_scrolled_in_the_window.rs`).
- Cut, each queued: keyboard scrolls glided as Chrome's (792); the owner's
  look for the sign, and a bar a person can drag or press (793); the glide
  stepped at the display's own refresh (794).

## Amendment, 2026-11-06 — the bar fades (queue item 793)

The owner approved this on 6 November 2026 (*"I agree"*): **keep the thin
bar, but it fades out about a second after scrolling stops (as macOS does);
it comes back while scrolling or when the pointer nears the right edge.**
§ 2's bar — its place, size, colour, numbers and reading — is kept as it
was; what changes is how much of it is seen.

- **Seen as a fraction of the browser process's** (`Bar::seen`, `0` to
  `1`, refused on the wire as anything else), drawn as the bar's
  `opacity`. Kept in `alo-window`'s `conversation_bar.rs`, time and nothing
  else, tested at given instants with an injected clock.
- **Whole while the conversation moves** — a wheel or a trackpad, each step
  of a glide, a key, a new turn put at the room's top: the window sees the
  turns or the offset it draws change — and **for a second after** (1,000
  ms), then **faded out in 300 ms**, straight, to nothing. It comes back
  whole at once, not faded in, as macOS's does. A chat opened, or one
  pressed in the panel, shows no bar until it moves.
- **Near** is the pointer over the first screen, between the room's top and
  its foot, within **24 CSS pixels either side of the bar** — the gutter
  beside the column and a little of the column's edge. While it is there the
  bar is whole; leaving starts its second from then.
- **Faded, it is still there to an agent**, read as § 2 says: an agent
  reads what the interface is, not how faint it is.
- **The window wakes** as the second ends and at each sixtieth of the fade,
  and not otherwise; the first screen is drawn again only when what is seen
  changed.
- **Dragging the bar, and pressing above or below it, are not in the
  owner's design**, and stay unbuilt: queue item 845.

Checked by `conversation_bar`'s tests (whole for the second, 0.75, 0.50 and
0.25 at 75, 150 and 225 ms into the fade, nothing after; whole again at
once; near and leaving; the strip either side; when to wake), the window's
(`conversation_window`: a trackpad's scroll, then 1.00, 0.50 and 0.00 at
999, 1,150 and 1,300 ms on the window's own clock, woken at the second's end
and each frame of the fade, whole with the pointer at 1,240, not at 1,244 or
above the room), the wire's (a fraction, and nothing else), and
`a_sign_that_the_conversation_goes_on.rs` (half seen, each channel within a
level of the mean of the bar's and the paper's; nothing seen, the paper;
read either way; reference `conversation-goes-on-fading.png`; the bar's left
edge 973 at the design's width and 676 at a room of 695). Mutation — the
bar always whole — fails.

