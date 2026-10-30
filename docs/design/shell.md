# The browser's own interface

The design the owner settled on 30 October 2026, written down so that the
items that build it (ADR 0074, queue items 699–708) are checked against
words and numbers rather than a picture somebody remembers. Where this file
and a stylesheet disagree, one of them is a bug.

## The idea

The interface is **a conversation**. A person types anything into one box —
a website, a search, a question, a task — and what comes back appears in the
conversation: a list of sites, an answer, a page, the agent at work. Pages
open *inside* the conversation, and nothing a person asked for is lost when
they open one.

## The window

| Part | Size | |
|---|---|---|
| Chats panel | 240 px wide, full height | collapses to a 64 px rail while the agent works, or on a narrow window |
| Top bar | 52 px tall | the chat's name, *Share* |
| Conversation | a column 880 px wide, centred in what the panel leaves | |
| The box | 56 px tall, 880 px wide, 24 px above the window's foot | 64 px tall on the first screen |

## The chats panel (item 699)

Top to bottom, inside 6 px above and 10 px either side, 12 px below:

- **The mark**, *alo*, in the serif, 22 px, in a row 46 px tall with the
  button that hides the panel.
- **New chat**, a white button 40 px tall, a plus before its words and
  `⌘T` at its end.
- **Search chats**, 38 px.
- **Open** — the chats in use. Each row is 38 px, 13.5 px text, the site
  open in it as a 16 px mark before its name. The chat on screen is white.
  A chat where the agent is working has a 7 px terracotta dot after its
  name; one it finished has a green tick, and both say so in words to an
  agent.
- **Earlier** — past chats, in quiet text, without marks. Closing a chat
  never loses it: it moves here.
- **Settings**, at the foot, under a line, with a settings icon — two
  lines, a knob on each — before its words.

A group's heading is 11.5 px, capitals, 600, quiet text, 18 px above it and
6 px below.

### Icons (item 712)

Where the design has an icon it is a simple line drawing: unfilled, stroked
in the colour of the words it sits with (`currentColor`), round caps and
joins, about 1.5–1.8 px of stroke, 16 px in the panel and 18 px on the
first screen. A plus before *New chat*; sliders before *Settings*; an arrow
in *Go* on the first screen, in place of the "→" it said; a plus above *Add a
site*, standing as tall as a site's letter so the tiles' words stay level.
Each is inline SVG written in the interface's own templates, never built
from anybody's string, and `aria-hidden`: an agent reads the words, not the
picture.

## What appears when a person types

1. **A website** (`evhomeguide.org`): *Go to the site* is offered first and
   Return opens it, with no model involved. Tab asks the agent instead.
2. **Anything else**: the **Sites** tab first — each result its site's mark,
   name and address, the page's title as a link, a two-line description. A
   site the answer drew on says *alo used this*.
3. The tabs, in order: **Sites · Answer · Images · Videos · News**.

## The answer

- A one-line answer in the serif, then short points.
- A small number after each sentence a source supports. Hovering it shows the
  site, the page and **the exact line**, beside the answer, never over it.
- *3 sources ›* under the answer, counting **only the sites the answer used**.
- When nothing reliable was found it says so, and offers what it *can* do.

## A page inside the conversation

Clicking a source opens the page in a card under the answer, scrolled to the
line the answer used, highlighted, with *alo used the highlighted part of this
page* above it. The card has back, forward, *1 of 3 sources ‹ ›*, expand and
close. It is a real page: a person scrolls, follows links and fills it in.
Where Chrome's window goes to make this so is item 702's to measure.

## The agent at work

- The window splits: the page large on the left, the agent's column on the
  right (384 px), the panel a rail.
- First a sentence of plan, then **one line per step, as it happens**, in a
  person's words — *"Choosing Saturday, 10:00"* — with why, when it chose:
  *"8:00 is taken, so this is the earliest free morning slot"*. The next
  steps are shown greyed, so a person knows where it will stop.
- The page has a terracotta outline and *alo is working on this page · click
  anywhere to take over*. Clicking the page takes over; the agent pauses and
  carries on from whatever the page then shows.
- **It stops before paying**, with what, when, where and how much, and
  *Confirm and pay* / *Not now*. Paying is always asked; sending, deleting
  and cancelling are asked unless a person said otherwise.
- **It never types a password.** Signing in is the person's step.
- **When it is stuck it says so** and offers choices, the closest to what was
  asked first. It never picks something it was not asked for.
- Done: one line, *Done in 6 steps*, the result in full, and what to do next.

## Colours

ADR 0074 § 2 is the palette. The rule that matters most: **terracotta means
the agent is acting**, and appears nowhere else.

## What is not designed yet

Dark mode, a phone-width window, downloads and bookmarks, and the settings
screens other than *What alo may do*.
