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

### Search chats and hiding the panel (item 711)

**Search chats** is 38 tall, 6 under *New chat*, rounded 10: a 16 px glass
12 in, then the field after 8, 13.5 px, its hint in quiet text. Pressed, it
takes the keyboard from the box (and the box from it), and is drawn white
while it has it. What a person types there shows only the chats whose
names hold it, whatever its case; a group with none found is not drawn,
and with none at all the panel says *No chat is called "…"* in quiet text,
18 under the field. Escape empties it, and empty lets the keyboard go.

**The button that hides the panel** stands at the mark's row's end, 36
square, rounded 8, an 18 px icon — a window with its left side ruled off —
in the panel's text colour, named *Hide the panel*. Pressed, the panel
becomes **the rail**, 64 wide, and the conversation takes the rest of the
window: the mark, 40 tall 10 down; *New chat*, a 44-square button with an
18 px plus, 6 under it; a 28-wide line 4 above and below; and at the foot,
12 up, *Show the panel*, 44 square with the same icon, which brings the
panel back as it was, what was searched kept. The design's rail also holds
each open chat as its site's mark; that is item 760.

### Icons (item 712)

Where the design has an icon it is a simple line drawing: unfilled, stroked
in the colour of the words it sits with (`currentColor`), round caps and
joins, about 1.5–1.8 px of stroke, 16 px in the panel and 18 px on the
first screen. A plus before *New chat*; a glass before *Search chats*; sliders before
*Settings*; the panel's own outline, its left side ruled off, in the button
that hides it and the rail's that shows it, both 18 px and named in words; an arrow
in *Go* on the first screen, in place of the "→" it said; a plus above *Add a
site*, standing as tall as a site's letter so the tiles' words stay level.
Each is inline SVG written in the interface's own templates, never built
from anybody's string, and `aria-hidden`: an agent reads the words, not the
picture.

## What appears when a person types

1. **A website** (`evhomeguide.org`): *Go to the site* is offered first and
   Return opens it, with no model involved. Tab asks the agent instead.
2. **Anything else**: the **All** tab first, as a search page does it — a
   short answer from the model, two or three sentences straight on the page's
   ground (no box), with its source numbers, *Full answer ›* (the Answer tab)
   and how many sites it drew on, a line under it; then the top sites from
   the search API (ADR 0082), each its site's mark, name and address, the
   page's title as a link and a two-line description; then a row of images
   with *See all images*; then *More sites ›*. A site the answer drew on says
   *alo used this*. The owner chose this on 31 October 2026.
3. The tabs, in order: **All · Answer · Sites · Images · Videos · News**. The
   Answer tab holds the full answer and its sources; each other tab one kind
   of result.

### What is built of it (item 701)

A press on the box gives it the keyboard; a press anywhere else takes it
away, and while it has it the box's border is the quiet text's colour.
What a person types is shown as the field's value. Under the box, 8 px
below it, a white card 720 px wide with a line border, 14 px corners and
6 px inside holds the offers, each 52 px tall — its words at 15 px, and
under them at 13 px in quiet text where it leads or *Not set up yet* —
the one Return chooses on the paper's colour with 10 px corners:

- *Go to the site*, and the address it goes to, first — only when what is
  typed is an address: `example.com`, `localhost:8080`, `127.0.0.1`,
  `https://…`; never words with spaces between them, `3.14`, a mail
  address, or another scheme.
- *Search for “…”*, saying *Brave Search* under it when a key is held,
  and otherwise, in quiet text, *Not set up yet — why* (708); then *Ask
  alo about “…”*, in quiet text, saying *Not set up yet* until a model is
  chosen (707).

Backspace erases the last character, Escape empties the box (and, empty,
lets the keyboard go), and Return or *Go* chooses the first offer. A press
on any offer chooses that one as Return chooses the first (714); in a
conversation the offers stand above the box, and a press on one standing
over a live card's page is the offer's, not the page's (759).

### Going there (item 713)

An address chosen opens in the machine's Chrome, in Chrome's own window
put over the first screen — its title bar over the conversation's, the
panel left in view — with no frame of ours beside it (ADR 0075). The line
at the window's foot says what became of it, in these words:

- *opening ADDRESS in Chrome, in a window of its own*
- *ADDRESS is open in Chrome's own window — Chrome/VERSION, driven by this
  browser*
- *ADDRESS could not be opened: WHY* — no Chrome on this machine, Chrome
  would not start, the page did not load
- *Chrome's window showing ADDRESS was closed*

A second address replaces the page the first opened. Closing the
conversation closes Chrome. Where a page goes *inside* the conversation is
still item 702's. Tab asking the agent waits on
707. The offers are shown only while the box has the keyboard.

### Searching (item 708)

Return or *Go* on words searches them with the Brave Search API (ADR 0082
§ 0, ADR 0083): the box empties and keeps the keyboard, and under the
person's words — in the 880 column, 40 below the top, 20 apart — stands the
**All** tab as the canvas of 31 October 2026 draws it:

- **The tab row**, 42 tall over a line of `#E1DACE`: each tab 14 across
  inside, 14 px quiet text; *All* in the ink, 600, underlined 2 px in the
  ink over the line. Sites, Videos and News open once sites are found and
  Images once images are asked for (743–746); Answer is drawn disabled
  until it is built (707).
- **The top three sites**, 20 under the row, 22 apart: a line of 13 px —
  the site's letter, white on one of four tints in an 18 square rounded 5;
  its name in the ink, 600; its address, `host › path`, in quiet text —
  then its title, 18 on 25, 600, in the link colour `#22406E`, one line;
  then two lines of its description, 14.5 on 23 in `#4A463F`, 740 wide at
  most, cut off after the second.
- **A row of images** (743), 20 under the last site, once they have come:
  a heading of 11.5 px, 600, in capitals, `#6B655C` — *Images* — with *See
  all images*, 13 px, 500, in the ink at its right; 8 under it five tiles
  84 tall, 10 apart, rounded 10, in `#E9E3D8`, each its picture the tile's
  width at its own proportions, centred and clipped by the tile, or with
  none its title in quiet words of 11.5 on 15.
- ***More sites ›***, 14 px, 600, 20 under the row (or the last site),
  opening the Sites tab (744).

While it searches, and when nothing is found or the search is not answered,
one sentence of 14.5 on 23 in quiet text stands where the sites would be —
*Searching Brave Search for “…”…*, *Brave Search found no sites for “…”.*,
or why: the key refused (401, 403), too many searches or the allowance used
up (429), Brave unable to answer (5xx), unreachable, or its answer
unreadable. The line at the window's foot says the same, and how many sites
were found. **No short answer is drawn** until a model is chosen (707), and
nothing in its place. A press on a site's title opens it as a card, in a
turn of its own under the results, its bubble the title (703).

### The Sites and Images tabs (items 743, 744, ADR 0086)

A press on an open tab shows it in that turn. **Sites** is every site the
search found — up to Brave's twenty — each as All draws its three, with
nothing under them; a further page (*Show more sites*) is item 761.
**Images** is a grid of four columns, 16 apart and 20 between rows: each
tile 150 tall, rounded 10, in `#E9E3D8`, holding its picture as the row
does; 8 under it the image's title, 13 on 18, 500, on one line, and its
site, 12 px in quiet words after its letter on a 14 square rounded 4. The
canvas's *All · Photos · Diagrams* filters are not drawn: the provider does
not say which an image is. While the images are asked for, or none came,
one sentence stands where the grid would be. A press on an image, in the
row or the grid, opens the page it was found on as a card, its bubble the
image's title. Pictures come only from Brave's own image proxy, and are
decoded only by the renderer that draws them (ADR 0086 § 3–4).

### The Videos and News tabs (items 745, 746, ADR 0087)

Neither is asked with a search: Brave is asked for a turn's videos, or its
news, the first time a person opens that tab, and never again for that
turn. While asked, or when none came or Brave refused, one sentence stands
where they would be.

**Videos** is a list, rows 20 apart: each video's picture 200 × 112,
rounded 10, on `#2A3446`, at its own proportions the tile's width, centred
and clipped; its length over the picture's bottom right corner, 8 in, white
11.5 on 15 on black at 72 %, rounded 5, 2 by 6 inside; then 16 beside it
the title, 17 on 24, 600, in the link colour, one line, 2 down; 4 under it
the site's letter on its 18 square, then its name and age, 13 px quiet
text; 6 under that two lines of what is said, 14 on 22 in `#4A463F`. A
picture comes only from Brave's image proxy, as an image's does. The
canvas's video playing in a card above the list is the card a title opens;
its *1 of 6 videos* is not built.

**News** is a list of stories, each 16 above and below — the first 4
above — over a line of `#E6E0D6`: the site's letter, its name 600 in the
ink and its age in quiet text on a line of 13; 4 under it the headline, 17
on 24, 600, in the link colour; 4 under that two lines of what is said, 14
on 22. The canvas's photo beside each story is item 765.

A press on a title or a headline opens its page as a card, its bubble the
title. A video's picture is not pressed.

### A site's own mark (item 747, ADR 0088)

Where a site, a video's site or a story's site is drawn on All, Sites,
Videos or News, its **mark** is the site's own icon when one came: drawn
down into the letter's 18 square, rounded 5, the name where it was. Brave
names each icon at its own image proxy; the browser process fetches it from
there alone — never from the site — and the renderer that draws it reads
it. A search's sites are drawn with their letters first and take their
marks as they come; a site whose icon is missing, refused or does not read
keeps its letter on its tint. The Images grid's 14-square marks stay
letters (768).

### Emoji (ADR 0088)

Neither typeface has an emoji. The interface carries **Noto Color Emoji**
and draws an emoji in it, in its own colours, wherever a title, a
description, a name or a person's words hold one; a character such as `©`
or a digit stays in Instrument Sans. A character neither face nor the
emoji face has is drawn by the machine's fonts — and, for Chinese,
Japanese and Korean on a Mac, still as a box (767).

### The conversation (item 703, ADR 0085)

Every search and every page opened is **a turn under the last**: the
person's words as their bubble at the column's right, then what came back.
Nothing is replaced. Within a turn the bubble and what follows are 16 apart
(20 for results); turns are **32 apart**. Each chat keeps its own turns;
*New chat* starts empty.

- **A page** is a live card only as the newest turn. When anything newer
  comes, or it is closed, or its page goes, it is kept as **its address**:
  a white line 44 tall, rounded 12, ringed in the line's colour, 16 inside,
  13 px — the host 600 in the ink, then the rest in quiet text. Pressing it
  opens the page again as a new turn.
- **A page that could not be opened** says why in its turn's place, 14.5 on
  23 in quiet text.
- **Scrolling.** A new turn's top stands at the room's top, 40 down. The
  wheel or trackpad over the conversation scrolls it — back as far as the
  first turn's top, on as far as the newest turn's foot at the room's foot
  — 40 a line, a trackpad's own pixels; over a live card's page the wheel
  is the page's. The room runs from 40 down to 24 over the box; nothing is
  drawn above it, under the box or over it.
- The agent's own words and its steps are turns too, when there is an agent
  to say them (704, 707).

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
How: ADR 0076 B — the card is drawn by this engine from Chrome's frames,
Chrome's window parked until *Expand* (built by 717; a person's input on it
is 718).

The card opens as its page's navigation commits and shows the page from its
first paint; until the page has loaded, its bar says **Loading** after the
address, in the bar's quiet 13 px words, 8 either side, before *Expand* —
the address gives it the room. A page that never loads stays a card saying
*Loading* until a person closes it; nothing closes it for them (ADR 0089,
item 751).

The page in the card is as sharp as the window it is in: its frames are
drawn at the conversation window's scale, two on a Retina display, wherever
Chrome's parked window is (ADR 0090, item 738).

*Back* and *Forward* are the page's own session history, as Chrome holds it:
each is drawn dark and pressable only when there is a page of the card's
that way, and greyed otherwise. The blank page the card's window began at is
not the card's, so a card's first page has *Back* greyed (item 737).

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

## Typefaces

**Instrument Sans** for every word of the interface, at 400, 500 and 600;
**Newsreader** — *the serif* in this file — for the mark *alo* and the
greeting, at 400. Both under the SIL Open Font License, compiled into the
application and handed to the interface's renderers alone (item 706, ADR
0084); a stylesheet asks for each first and keeps `system-ui, sans-serif`
or `serif` after it. Emoji are drawn in **Noto Color Emoji**, carried and
handed the same way (ADR 0088). Newsreader's optical size follows the text's size in
the canvas; here it is the file's 18 until item 752.

## Colours

ADR 0074 § 2 is the palette. The rule that matters most: **terracotta means
the agent is acting**, and appears nowhere else.

## What is not designed yet

Dark mode, a phone-width window, downloads and bookmarks, and the settings
screens other than *What alo may do*.
