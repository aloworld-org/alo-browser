# ADR 0066 — The box an agent acted on through Chrome is marked by Chrome, and no window of ours goes over the page

**Status:** accepted
**Date:** 2026-10-27
**Context:** ADR 0061 § 3 (the mark is Chrome's to draw, with
`Overlay.highlightNode`, against the node the agent named); ADR 0062 (our
frame floats beside Chrome's window); ADR 0002 (no verb takes a
coordinate); ADR 0024 § 1 (the window is rented, and every call into it is
checked safe); law 4; queue items 624 and 680

## The decision in one line

Through Chrome, the box a verb acted on is marked **by Chrome**, with
`Overlay.highlightNode` against the node the verb's outcome names: its
border drawn in alo's accent and **Chrome's label naming it in the agent's
words**. No window of ours is put over Chrome's page. ADR 0061 § 3 stands,
and the window item 680 proposed is decided against.

## Why this is a decision rather than an implementation detail

Because item 680 was written as though § 3 of ADR 0061 had not been. It
said the ring "has to be drawn over [the page], by a window of ours over
Chrome's page, transparent and taking no pointer", when an accepted ADR had
already given the mark to Chrome's own `Overlay` domain. Two accepted
answers to one question is how a repository starts building both. And the
two are not variations: one is a third window of ours, tracking a page
that scrolls on another process's clock, and the other is a drawing in
that process's own compositor that does not look like alo's.

## 1. What Chrome's mark does, measured

Chrome 156.0.8078.48, macOS 26.5.2, Stage Manager on, a shown Chrome in
`--app` mode at 200,140 with an 800×600 page at device scale 2. The page
held a borderless button filled with alo's accent at 100.5,120.25 and
120×40, a button with a one-pixel grey border at 300,120 and 120×40, a
field, a `MutationObserver` over the whole document, and listeners for
pointer, focus, resize and visibility events. `Overlay.enable` after
`DOM.getDocument`; the page's viewport captured with `screencapture -R`
and compared in device pixels. A temporary probe, outside the repository.

- **It draws the box's own regions and nothing else.** With
  `borderColor` alo's accent and every other region clear, the bordered
  button's border was drawn in the accent — and the **borderless button
  was drawn exactly as with no highlight at all**: not one device pixel
  differed. `HighlightConfig` fills content, padding, border and margin as
  the page laid them out; it has no outline and no offset. alo's ring —
  two pixels of `--accent`, two pixels outside the box (`mark.rs`) — is
  not something it can draw. Filling the content instead is the failure
  624's first reference render found: an accent button under an accent
  fill, which nobody can tell is marked.
- **Its label names the box in the agent's words, on every box.** With
  `showInfo` and `showAccessibilityInfo`, the borderless button was shown
  with Chrome's label under it: `button#b`, `120 × 40`, *Name* "Submit
  order", *Role* "button", *Keyboard-focusable* — the name and role the
  agent named it by. It is about 155 by 120 CSS pixels and stands beside
  the box, not on it.
- **It follows the page.** `scrollTo(0, 50)` moved the drawn border up
  100 device pixels in the next capture, and the label moved with the
  page's scroll of 40 the same way; the page moving the box itself (its
  `top` changed) moved the mark with it. Chrome draws it in its own
  compositor against the node, frame by frame.
- **It takes no pointer.** A press on the marked button, by `cliclick` at
  its centre, ran the page's own click handler. A press elsewhere on the
  page **took the mark away**, in the one run made of it.
- **The page is told nothing.** Highlighting changed no element (the
  observer saw no mutation, the document kept its nine), dispatched none
  of the events listened for, and left `visibilityState` `visible`. No
  script is injected; the command names a node id, never a position.
- **It is not in Chrome's own picture of the page.**
  `Page.captureScreenshot` taken with the mark drawn has no pixel of it;
  only the screen does.
- **It answers at once.** `Overlay.highlightNode` was answered in 3 ms.

`Overlay.highlightRect` was measured too, because it is the one command
that draws an outline: asked for 298,118 and 124×44 at device scale 2, it
drew a rectangle at **twice** that place and size, outlined four device
pixels thick — and as a rectangle it is measured once and stays, while the
page scrolls under it. It is refused for that reason, and not because it
takes a coordinate: a rectangle read to draw is not a verb (ADR 0002, and
`mark.rs` draws from rectangles itself).

## 2. What a window of ours over the page would cost

- **It would hide the page.** Our windows are drawn through `softbuffer`
  0.4.8, whose macOS presenter makes its picture with
  `CGImageAlphaInfo::NoneSkipFirst` — every pixel opaque, whatever alpha
  was written. A window of ours over Chrome's page is a window of black
  and accent over it, however transparent `winit` is asked to make the
  window itself. A transparent one needs a presenter other than
  `softbuffer`'s, and this repository has none; whether one can be built
  without `unsafe` of ours was not checked, and law 4 forbids it with
  `unsafe` unless an ADR of its own says why.
- **It would be a guess about the next layout.** A rectangle of ours is
  read from Chrome's tree at one moment, and Chrome's page scrolls and
  moves on its own clock. Our frame follows Chrome every `FOLLOW_EVERY`;
  a ring that followed the page as often would trail every scroll by up to
  that long — a fifth of a second — which is the guess `mark.rs`
  refuses: "a rectangle measured against one layout is a guess about the
  next." Chrome's mark is drawn
  against the node in the frame that draws the page.
- **It would be a third window under Stage Manager**, which ADR 0062
  measured showing one application's windows at a time — kept on the
  screen only by floating, and taken aside by the window server's word.

None of these is a cost the window could earn back: what it would draw
that Chrome's mark does not is alo's own look.

## 3. So: Chrome marks the box, in its look, and says so

Through Chrome, once a verb has been answered, the thread holding Chrome
asks it to highlight **the node the verb's outcome names** — the same node
624's ring is drawn from in this engine — with its border in alo's accent,
its other regions clear, and Chrome's label shown with the accessibility
part. A verb refused marks nothing and takes away any mark already drawn,
as a refused verb's repaint does in this engine. The next verb's mark
replaces the last. A person's press on the page took it away in the one
run that measured it, and that is Chrome's to do.

**It does not look like alo's ring, and that is said, not hidden.** ADR
0058 § 6: a claim names its renderer, and what a person sees is part of
what differs between the two. On the engine path the mark is alo's ring;
through Chrome it is Chrome's highlight and label. The label is the part
that is seen on every box, and what it says — a name and a role — is the
agent surface's own vocabulary, which is the point of the mark.

## What is not measured, and is not hidden

- **Whether navigating takes the mark away.** The probe's mark was on the
  borderless button when the page navigated, so a capture could not show
  it either way. The building iteration measures it, and hides the mark
  itself if Chrome does not.
- **Whether one mark replaces another of Chrome's own accord.** The
  probe's later highlights each followed a `hideHighlight` or a press.
- **How long the label covers what is beside the box.** It stays until
  the next verb, a press, or a refusal; whether a person wants it gone
  sooner is theirs to say, and it may send this back.
- **Two shown Chromes, or a page in a frame.** A highlight is per page
  target; a node in a frame is the building iteration's to check.

## Consequences

- Item 680 is buildable: `chrome_serving.rs`'s thread, which already
  tells the frame the agent's line after answering, also sends
  `Overlay.highlightNode` (or `Overlay.hideHighlight` for a refusal) on
  the tab's session, with `Overlay.enable` after `DOM.getDocument`. It
  closes on the mark seen **on the screen** — the captured window's
  device pixels at the node's border box from `DOM.getBoxModel`, and the
  label present — because Chrome's own screenshot does not carry it.
- No crate is added, no window is added, and nothing is added to the
  frame's window level or to Stage Manager's question (643).
- Item 549 (nothing drawn while nobody watches) does not bind here as it
  does in the engine: Chrome draws the mark whether or not anybody looks,
  and that costs one message per verb.

## What this does not decide

- Anything about the engine path, whose ring is unchanged.
- Whether alo's own look can ever be drawn over Chrome's page. If a
  presenter that can be transparent without `unsafe` of ours arrives, the
  window can be measured again; this decides on what exists.
- What an agent is told: a mark is downstream of the verb and changes no
  verb's outcome by one bit, as § 3 of ADR 0061 already says.

## Addendum, 2026-10-27 — built, and two of the open questions measured

Built by item 680 (`chrome_mark.rs`), as § 3 says, for a shown Chrome
only. Measured on Chrome 156.0.8078.48, macOS 26.5.2, by
`the_box_marked_through_chrome.rs`, which captures Chrome's window by the
window server's number (`screencapture -l`) rather than a region, so a
window in front of it changes nothing:

- **One mark replaces another.** With one box marked, highlighting
  another left the first in its own grey, with no `hideHighlight`
  between. So this decision's "the next verb's mark replaces the last"
  is Chrome's, and nothing of ours is needed for it.
- **A page going to another document on its own takes the mark away.**
  A box marked on a page that reloaded itself eight seconds later was
  unmarked on the page loaded again, through two seconds of captures.
  Nothing of ours is sent for that. A verb that itself takes the tab to
  another document is unmarked by us (the document before and after
  differ), and was seen unmarked.
- **The label moves.** Under a box near the top of the page, above one
  lower down; about 190 by 120 CSS pixels either way, so it covers what
  is there.
- **Chrome's accent is the page's.** The border Chrome drew and a swatch
  the page filled with the same `#e76f51` captured as the same pixel
  (216,118,89 on this screen).

Still open: a node in a frame, and how long a person wants the label.
