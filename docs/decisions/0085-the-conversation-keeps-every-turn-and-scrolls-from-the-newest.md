# ADR 0085 — The conversation keeps every turn, and scrolls from the newest

**Status:** accepted — built by queue item 703 (31 October 2026)
**Date:** 2026-10-31
**Context:** ADR 0074 (the interface is a conversation, each part a
document of ours drawn in a confined renderer); ADR 0076 B (a page opened in
the conversation is a card drawn from Chrome's frames; one shown Chrome per
page); ADR 0024 § 5 (a person's pointer is a coordinate, and the browser
process decides where it lands); ADR 0002 (the layout tree is the agent's
tree, and no verb takes a coordinate); `docs/design/shell.md`, *The idea*
(*nothing a person asked for is lost when they open one*); queue items 703,
708, 717, 718 and 750

## The question in one line

When a conversation holds more than one turn, what becomes of a page that
was a card once a newer one opens, which renderer draws the turns, and how
does it scroll without a renderer deciding where a person's press on a live
card's page lands?

## 1. Every turn is kept, oldest first, per chat

Each search and each page opened — from the box, from a site's title, from
a page shown earlier — is **a turn appended under the last**: the person's
words as their bubble, then what came back. Nothing replaces anything. Each
chat holds its own turns; *New chat* starts with none, and a chat pressed in
the panel shows its own. The bubble of a page opened from a link is the
words that link was drawn with — a site's title, or what was said for a
page shown earlier — since that is what the person chose.

The turns that exist are the ones something produces today: a search's
results, a live card, a page shown earlier, and a failure in words. **The
agent's words and the steps it took are not a turn yet**: what they are
waits on a model (707, the owner's choice) and on the agent's record given
a face (704). Nothing stands in for them.

## 2. An earlier card becomes its address, not a picture

One Chrome is open for the conversation's card (ADR 0076 B), so a card is
live only as the **newest** turn. When anything newer comes — another page,
a search, another chat — or the card is closed or its page goes on its own,
the card is **put away as a line holding its address**, host bold and then
the rest, as the card's bar writes it. The line is a link of the class a
site's title has, so a press on it opens the page again as a new turn,
claimed by its place exactly as a title is. A page that could not be
opened leaves why, in this browser's words, in its turn's place.

The alternative considered was keeping the card's last frame as a still
picture with its address. Refused, for now, on a number: every frame of the
live card sends the whole first screen to its renderer again — sixty times a
second while the page moves — and an earlier card's still is ~300–400 KiB of
PNG. Three earlier stills would be about a megabyte more per frame, some
60 MiB a second down a pipe whose only purpose was the one live picture. A
still could be sent once and kept by the renderer, which needs the first
screen to be sent in parts rather than whole; that is a larger change than
this item, and not asked for by anything a person has hit. The line keeps
what matters — what it was and where — and is one press from the page
itself.

## 3. The conversation is scrolled from the newest turn's top

The browser process holds how far the conversation is scrolled as one
number, `down`: how far **the newest turn's top** is above the top of the
room the conversation is seen in (40 down; the room ends 24 over the box,
ADR 0074 / item 750). The document is built so that the newest turn stands
at `40 − down` whatever came before it — the turns in a box moved by `top:
-down` (a number of the browser process's, never a string), the newest in
flow, the earlier ones in a box placed against it from above (`bottom:
100%`, 32 apart and 32 over it).

So the live card's page box, which decides where a person's press, drag and
wheel land in the page (ADR 0024 § 5), is `card_place::page_box` moved up
by `down`: the browser process's own arithmetic, from its own number and the
viewport, as it was before the conversation scrolled. **A renderer never
says where the page box is.** What is outside the room is clipped and is not
the page's to be pressed (`card_place::seen_page_box`).

How far it may scroll is text laid out, which only the renderer does. So
after painting a conversation, the renderer answers how far it **reaches**
— the earlier turns' height with the gap under them, and the newest turn's
— (`ToRenderer::ConversationReach`, `FromRenderer::ConversationReached`),
read as two finite lengths bounded at a million pixels. The browser keeps a
person's wheel inside it: back until the first turn's top is at the room's
top, on until the newest turn's foot is at the room's foot. A renderer that
lies about its reach can let the conversation scroll onto empty paper or
stop it short; it cannot move where a press lands.

A new turn sets `down` to `0`: its top in view at the room's top. The wheel
over the conversation scrolls it, 40 CSS pixels a line as Chrome's own and
a trackpad's pixels over the scale; over a live card's page it is still the
page's (718); over the panel, nobody's. The window aims a press by the
offset of the frame it lands on: the drawing thread tells the reach and the
offset it painted at just before that frame.

## 4. One renderer for the first screen and the conversation

Item 703's text says *in a renderer of its own*. It is drawn by **the first
screen's renderer**, the one that already draws the box, the card and the
results, at a site no page can name. Splitting the turns from the box would
put a seam in one column — two compositions, two clips, a press aimed across
both — to isolate strings that are already only text nodes in a document of
ours, from a renderer that already holds no page. The interface stays
apart from every page's renderer, which is what ADR 0074 § 1 asks. If chats
come to be kept for long, a renderer per chat is the natural boundary;
that is for the item that keeps them.

## 5. The agent's read

An agent reads the conversation as a `region` named *Conversation*, each
turn a `region` named by what the person said — their words, then the
results' region, the card's region, a link named by a page's address, or a
paragraph saying why not — every turn, wherever it is scrolled. Nothing
here is a verb, and nothing takes a coordinate.

## What was found on the way

The engine painted a positioned box with the stacking context it belongs
to, **past the clip of an `overflow: hidden` box that is not one**, where
CSS clips a relatively positioned child of such a box (CSS Overflow 3 § 3;
read, not measured against Chrome here). The conversation's clip is given
a stacking context of its own (`position: relative; z-index: 0`), under
which both rules draw the same; the engine's defect is queue item 753.

## Consequences

- `alo-renderer`: `turn.rs` (the turns), `conversation_document.rs` (built
  into the first screen), `conversation_scroll.rs` (the reach and the
  bounds), `card_place.rs` (the page box moved by `down`, clipped above and
  below), the wire's turns and reach. The results' tab row is made with
  `alo-dom`'s operations rather than moved out of a template, since a
  conversation holds as many as it holds searches.
- `alo-window`: `conversation_thread.rs` (a chat's turns and its scroll),
  `Conversation` holding one per chat, the window's wheel, and the drawing
  thread asking each conversing paint's reach.
- Cut, each queued: keyboard scrolling (754), a scrollbar or a sign of more
  above (755), smoothing a wheel's lines (756), chats kept across launches
  (757), a chat named by its first words in the panel (758), and the room
  the offers take above the box, which the aim does not yet know (759).
