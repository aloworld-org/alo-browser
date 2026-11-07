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
| Chats panel | 240 px wide, full height | collapses to a 64 px rail while the agent works, or on a window narrower than 760 px (item 775); shown there, it slides over the conversation (item 776) |
| Top bar | 52 px tall | the chat's name, *Share* |
| Conversation | a column 880 px wide, centred in what the panel leaves | narrower in a narrow window: what the panel leaves less 32 px either side (item 774) |
| The box | 56 px tall, 880 px wide, 24 px above the window's foot | 64 px tall on the first screen; as wide as the column |
| The window | at least 600 × 600 px | never narrower nor shorter (item 775) |

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
  never loses it: it moves here, first (item 791). The chats kept from
  before alo was last closed are here, newest first; pressed, one is on
  screen and under *Open* again — first there, the most recent (items 757,
  760, ADR 0092).
- **A chat's name** is the first words said in it — up to 48 characters,
  whole words, `…` when there was more — and *New chat* until something is
  said (item 758).
- **Settings**, at the foot, under a line, with a settings icon — two
  lines, a knob on each — before its words.
- **Closing a chat** (item 791, ADR 0092's amendment; approved by the
  owner 2026-11-06 — *an × appears on hover over an open chat; closing
  moves it from Open to Earlier; nothing is deleted*): the open row the
  pointer is over is a shade lighter and has **an ×** at its end — 24 px
  square, rounded 6, a 12 px cross in the ink on the line colour, named
  *Close* and the chat's name; the row does not move. Pressed, the chat
  moves to the top of *Earlier*, its turns kept and a live card put away as
  its address; nothing is deleted and no file removed. The chat on screen
  closed puts the open chat below it on screen, or else the one above, or
  else a new chat. **The empty chat on screen has no ×** — closed, it would
  only come back as *New chat* — and an empty chat that is not on screen,
  closed, simply goes: nothing would be kept of it.
- **Which rows offer which** (decided with item 791): **an open row offers
  its ×, an earlier row offers *Delete*** — never both, and the rail
  neither. Closing loses nothing, so it is what a chat in use offers;
  deleting is offered only for a chat already put away, so an open chat is
  deleted in two deliberate presses, closed then deleted, and a person
  reaching for one can never press the other.
- **Deleting a chat** (item 789, ADR 0092's amendment): the earlier row the
  pointer is over is a shade lighter and has **Delete** at its end — 24 px
  tall, 12 px 600 words on the line colour, 8 px either side; the row does
  not move, its name gives way. Pressed, the chat leaves the panel at
  once, and above *Settings* a dark pill, 36 px tall and rounded, says
  *Deleted “…”* in 12.5 px light words, the name cut where it ends, with
  **Undo** in a light pill at its end, for five seconds — the time alo's
  own *Undo send* holds a mail. *Undo* puts the chat back where it stood.

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
panel back as it was, what was searched kept.

### The rail's chats (item 760)

Approved by the owner 2026-11-06: *the narrow rail shows round initials for
the five most recent chats, with the name on hover; a press opens the chat;
a "+" at the top makes a new chat.*

- **Which five.** The first five the panel would draw — the open chats,
  most recent first, then the earlier ones, newest first — whatever *Search
  chats* holds. A chat opened again from *Earlier* is first under *Open*,
  and so first on the rail.
- **Each** is a 44-square button, 6 under the line and 6 apart — the first
  at (10, 121), then 171, 221, 271 and 321 — holding **its initials**, the
  first letter or digit of each of the first two words of its name as the
  panel shows it, in capitals (*Weekend in Lisbon* `WI`, *rust* `R`, *New
  chat* `NC`, a name with no letter a middle dot), 12 px 600 in the ink in
  a **32 px circle** on the line colour. **The chat on screen** is white
  with a shadow, as its row is white in the panel; one the agent works in
  has the panel's 7 px terracotta dot at its circle's shoulder. Five fit
  above *Show the panel* in the least window.
- **The name on hover.** The chat the pointer is over is a shade lighter,
  and its name stands beside it in a dark pill — `#2b2722`, 12.5 px 500 in
  `#f3efe8`, 28 tall, rounded 8, 10 inside, at most 240 wide and cut where
  it ends — centred on the button, 4 past the rail's edge, over the first
  screen. The rail is then laid out 320 wide and drawn on nothing, so only
  the rail and the pill stand over the first screen; the pill is not
  pressed (a press there is the first screen's), and the pointer leaving
  the rail takes it away.
- **A press** on a chat opens it, as its row in the panel does; the "+" at
  the top — *New chat*, under the mark — makes a new chat. The rail offers
  no × and no *Delete*.
- **An agent reads** each as a button named by the chat — with *Done* after
  a finished one, busy while the agent works in it, the one on screen
  current; the initials and the pill are hidden from it.

### Folded in a narrow window (item 775)

**In a window narrower than 760 px the panel folds to the rail on its
own**, and the conversation takes the rest; widened to 760 or more it
unfolds, as it was — what was searched in it kept. 760 is the first round
width at which the column beside the panel holds the search's tab row,
*All* to *News* (412.75 into the column), with a gutter's 32 to spare: at
760 the column is 456, at 759 beside the rail 631. *Search chats* lets the
keyboard go as the panel folds.

**A person's choice is remembered apart from the fold.** A panel a person
hid stays the rail when the window is widened past 760. *Show the panel*
pressed while the window is narrower than 760 is kept — the panel comes
back beside the conversation as soon as the window is 760 wide — and,
meanwhile, the panel slides over the conversation (below).

### The panel over the conversation in a narrow window (item 776)

Approved by the owner 2026-11-06: *below 760 wide, pressing the panel button
slides the panel in over the conversation with a dimmed backdrop; Escape, a
press outside, or choosing a chat closes it.*

- **The sheet.** *Show the panel* on the rail, in a window narrower than
  760, brings **the whole panel**, 240 wide and the window's height, over
  the conversation; the conversation stays laid out beside the rail under
  it. It **slides out from the rail** — its left edge from −176, where its
  right 64 stand over the rail, to the window's edge — in **200 ms, eased
  out** (`1 − (1 − t)³`), and back the same way, so it never shows the rail
  bare as it comes. Closed half way out, it goes back from where it was.
- **The backdrop** is the conversation's ink, `#1b2433`, at **0.32** over
  the conversation and the rail, as far on as the sheet is out.
- **In it** the panel is the panel: its rows hovered offer their × or
  *Delete*, *Search chats* takes the keyboard, *Undo* is there.
- **Closing it**: **Escape** — whatever has the keyboard, *Search chats*
  letting it go with what was typed kept; **a press outside it**, on the
  backdrop — which presses nothing under it, not the box, not a card's page;
  **choosing a chat** in it, or **New chat**; and *Hide the panel* in it,
  which is a person's choice of the rail. *Search chats*, × and *Delete*
  keep it out. Widened to 760, the panel stands beside the conversation and
  the sheet is gone at once.
- While it is out, nothing under it is hovered, scrolled or typed into. It
  is never shown while alo's agent works (the rail for the run, item 819).

**The window is never narrower than 600 nor shorter than 600.** Beside the
rail the tab row needs a window of 541; at 600 the column is 472. At 600
tall the conversation is seen from 40 to 496 and the first screen's sites
row whole. So the tab row never scrolls: the window server keeps the
window at least that size.

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
  alo about “…”*, in quiet text, saying *Not set up yet* until the agent
  is built (704); a search's own answer is drawn with its results (707).

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
  Images once images are asked for (743–746); Answer once an answer is
  asked for (707).
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
were found. Once sites are found, the short answer stands between the tab
row and the sites (707, *The answer* below). A press on a site's title opens it as a card, in a
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
  drawn above it, under the box or over it. **A wheel's notch glides** its
  40 over a fifth of a second on ease-in-out, shorter for more at once, as
  Chrome does (item 756, ADR 0093 § 3); a trackpad is followed as it moves.
- **Keys** (item 754): Page Down and space go on by the room less a line,
  Page Up and Shift-space back by as much, Home to the first turn's top and
  End to the newest turn's foot — while the box is empty or nothing has the
  keyboard; never while words are in the box, *Search chats* has it, or a
  card's page has it.
- **A sign that it goes on** (item 755): while the turns do not fit the
  room, a bar 6 wide, rounded 3, `#cbc3b6`, stands 13 in from the clip's
  right edge, in the gutter beside the column — as long against the room as
  the room is against the whole conversation, never under 32, and as far
  down it as the conversation is scrolled. **It fades** (item 793, ADR
  0093's amendment; approved by the owner 2026-11-06 — *keep the thin bar,
  but it fades out about a second after scrolling stops, as macOS does; it
  comes back while scrolling or when the pointer nears the right edge*):
  whole while the conversation moves — a wheel, a glide, a key, a new turn —
  and for a second after, then faded out in 300 ms; whole again at once
  when it moves, and while the pointer is within 24 px either side of it,
  between the room's top and foot. A chat opened shows none until it moves.
  An agent reads it either way. Nothing presses or drags it (845).
- The agent's own words and its steps are drawn on its live card (704);
  kept as turns of the chat is 820.

## The answer

### What is built of it (items 707 and 773, ADR 0095)

Once a search finds sites, the browser process asks OpenAI's
`gpt-6.1-terra` (ADR 0081) for an answer from the person's words and what
Brave said of the first eight sites — nothing else, no page fetched — and
checks every source number it cites against those eight before anything is
drawn. **On All**, 20 under the tab row: the sentences, 16 on 26 in the ink,
760 wide at most, each followed by its sources' numbers — each a pill 16
tall, rounded 8, 10.5 px, 600, on `#E9E3D8`; 10 under them *Full answer ›*,
13 px, 600, opening the Answer tab, and *Drew on N sites* in quiet text; 20
under that a line of `#E1DACE`, then the sites, 20 under it. A site the
answer drew on says *alo used this* after its address: 11.5 px, 600,
`#2F6B5E` on `#E3ECE8`, 6 inside either side, rounded 8. **The Answer tab**:
the answer in one line in the serif, 22 on 30; 14 under it the points, 15 on
24, 20 in with their bullets, 6 apart; 14 under them a line of `#E1DACE`
and 14 under it *N sources*, 13 px, 600, counting only those cited, then
each on a line of 13 — its number, its site's name in the ink, 600, its
address, its page's title in the link colour, cut at the line's end — 8
apart; 14 under them one quiet line of 12.5 on 18 saying what was sent to
OpenAI. Where there is no answer, one sentence of 14.5 on 23 in quiet text
stands in its place on both: *alo is writing an answer from these sites…*,
*I couldn't find a reliable answer.* and where the sites are, or why — no
key, the key refused, too many requests, OpenAI unable to answer,
unreachable, an answer unreadable or citing a site it was not given. A
chat reopened from *Earlier* shows the answer it had, on the tab it was on,
read from its file and never asked again (814). Not built yet: the exact line on hover (812), a source opened as a card at the
line it was used for (813), streaming (815).

### As designed

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
item 751). A link followed in the card, *Back* or *Forward*, or the page
sending itself elsewhere says *Loading* from that moment — while the server
is still to answer — until Chrome stops loading, as its own tab's spinner
turns; a link that leads to no page (a download, an empty answer) stops
saying it with the page it left still shown (ADR 0089's amendment, item
770).

The page in the card is as sharp as the window it is in: its frames are
drawn at the conversation window's scale, two on a Retina display, wherever
Chrome's parked window is (ADR 0090, item 738).

Nothing of the parked window is seen: the window server keeps a strip of it
on a display, so the Chromium alo carries draws it fully transparent and
lets a person's pointer through it until *Expand* shows it (ADR 0076 B4's
amendment, item 771). Through the machine's Chrome, run from a terminal,
the strip stays.

**The shield** (item 807, ADR 0094's amendment): after the address,
before *Expand*, a pill 28 px tall, rounded, under a line-colour border:
a 16 px shield in the done green and how many requests the lists refused
on the page, 12 px 600. Pressed, blocking is off for the page's site: the
shield and its word **Off** are grey; pressed again, on. Either way the
page loads again. What the lists name as an advert's box is hidden in the
page (item 806) — nothing of it is drawn by alo.

### A person on a card's page (items 799, 811, 798, 810, 802)

Five designs the owner approved on 6 November 2026 (*"I agree"*),
approved by the owner 2026-11-06; what the design left open was settled
by the builder and is recorded here. Each is built through the Chromium
alo carries, which tells alo what the DevTools pipe does not carry — the
pointer's shape, a context menu, a link to open elsewhere — on a line of
its own beside the pipe (descriptor 5, `chrome_told.rs`, ADR 0076's
fourth amendment); through the machine's Chrome, run from a terminal,
they are not, and the window says so where a press would otherwise do
nothing.

**The pointer follows the page** (item 799): the hand over a link, the
I-beam over text and over a field, and whatever else the page's `cursor`
asks — every shape CSS names — exactly as Chromium decides it, while the
pointer is over the card's page box, and wherever it is while a press on
the page is held (a selection dragged past the page's edge); the arrow
over everything of alo's own. A new page begins at the arrow. `none`
hides the pointer over the page; a page's own picture for a pointer is
shown as the arrow (846).

**A link's address on hover, in an expanded card** (item 811): a strip at
the bottom left of the expanded page's window, drawn by alo's own host
program, never Chromium's status bubble: the interface's paper `#f4f1ec`,
its words in the quiet ink `#5f5a52`, 12 points, 24 points tall, 8 either
side of the words, at most a third of the window wide and an address
longer than that cut at its end with `…`; gone while the pointer is over
no link. Its face is *Instrument Sans* where the system has it, and the
system's own otherwise — the host carries no font of alo's (852). A card
in the conversation shows no strip: its page is a picture there.

**alo's own small menu** (item 798): a press of the secondary button on
the card's page — or the main one with Ctrl, as a Mac takes it — is the
page's first: it hears its `contextmenu`, and a page that takes it for
itself shows its own. Otherwise alo draws its menu where the press was:
*Copy* (greyed when nothing is selected), *Copy link* and *Open link in
new chat* over a link, and *Search for “…”* when words are selected — the
words shown cut at 16 characters with `…` so the line fits, searched whole. A white box
rounded 10 under a line-colour border and a soft shadow, 220 wide, a line
30 tall in the ink, 13 px 500, 12 in from its left, 4 above the first
line and below the last; its top left corner at the press, moved left to
stay inside the page box, and drawn above the press when there is no room
under it. A press on a line does it and closes the menu; a press anywhere
else, Escape and a wheel close it and do nothing else. *Copy* copies the
page's selection as ⌘C does there; *Copy link* puts the address on the
system's clipboard; *Open link in new chat* opens it as a new chat's first
turn; *Search* searches the words as this chat's next turn. On a Mac a
secondary press selects the word under it, so over a link the menu also
offers *Search for* the link's words. The menu is walked by the pointer
alone for now (850). **The middle button, and the main one with ⌘, on a
link** open it as the conversation's next turn — its address the turn's
bubble — and never in a tab nobody sees; a page that takes the press for
itself opens nothing. Every other modifier held with a press is said and
nobody's (851).

**A dropdown on a card** (item 810): **while the agent works**, its
choosing in a closed list on the card's page never opens the list's menu
— a window nobody sees, taking the person's keys — and the list's options
stand in the agent's own column under the step choosing, on white rounded
8 under a line-colour border, a line of 20 each in the quiet colour, the
one being chosen ticked (`✓`) in the ink; at most eight shown, the
chosen among them, and *and N more* after. It chooses by the one key
that reaches the option alone (the option's first letter, when no other
option begins with it) — the page hearing one `input` and one `change`,
for it alone — and otherwise not at all, saying the person chooses it
(849). **When the person uses the page**, a press on a closed list
expands the card and opens Chromium's own menu there, under the list, as
Chrome's does: the normal dropdown.

**Chinese and Japanese input** (item 802): while the card's page has the
keyboard, macOS's input methods compose in it — the composition
underlined in the page's field as Chrome's own input methods do, and what
is chosen put in — and their candidates stand under where the person is
typing: the end of the field's words, on their line, or the start of an
empty field (847: a caret moved back into the words). alo's own box and
*Search chats* take no input method yet (848).

*Back* and *Forward* are the page's own session history, as Chrome holds it:
each is drawn dark and pressable only when there is a page of the card's
that way, and greyed otherwise. The blank page the card's window began at is
not the card's, so a card's first page has *Back* greyed (item 737).

## The agent at work

### What is built of it (item 704, ADR 0096)

Words typed while a card is open, and Return, ask alo: the box offers *Ask
alo about "…"* first, saying *alo works on this page*. The agent works on
the card's page through the same road an agent on the socket takes, one
step at a time, and is drawn **under the page, inside the card**: while it
works the card and its page are outlined in terracotta (2 over the page,
the card's border and 1 more around it), and a line 36 tall on
`#F6E7E0` says *alo is working on this page · click anywhere to take over*
in `#7A3A24`, 13 px, 600, with **Stop** at its end (28 tall, rounded 8,
outlined terracotta); 10 under it what the person asked (13.5 on 20, 600),
then one line per step in the quiet colour, 6 apart, why under a step when
it chose (12.5 on 18), the step under way in terracotta. Asking before
pressing what pays, sends, deletes or sends a form: a box rounded 12, 12
by 14 inside, the question (600), then **Allow** in the ink and **Not
now**, 28 tall, 8 apart. Stuck: why, then each choice as a button. Done:
*Done in N steps* in the done colour and the result in full. A press on
the page stops it and gives the person the page. It never types a
password: the card refuses it, for any agent. Nothing of Chrome's is
drawn over the card's page — no inspector highlight, no tooltip (822);
alo's own ring on a card is 823. Not built yet: a sentence of plan and
the next steps greyed (824), carrying on after taking over (825), what,
when, where and how much before paying, and *Confirm and pay* (826); an
agent opening its own page (821). **Each run is kept** with the page it
worked on (820): once the page is shown earlier, under its line, what the
person asked as their bubble, then what alo did on a white box rounded 12,
14 by 16 inside — its steps in the quiet colour, 6 apart, why under a step
— and 10 under them *Done in N steps* in the done colour and the result,
or why it was stuck or stopped; nothing on it pressable. A text typed that
the person did not say is never kept: that step reads *Typed into a
field*.

### The split view (item 819)

**While the agent acts or asks, the window splits.** The panel is the
rail for the run, at any width; the first screen beside it is no longer
the conversation but **the page, large, on the left** — the card with its
bar, 16 in from the screen's top, left and foot and from the column, its
page box the rest of the card under the bar, outlined in terracotta — and
**the agent's column, 384 wide its rule included, on the right**: white,
ruled off on its left in the line's colour, 16 inside it and 24 above its
foot. In it, from the top, the working line with *Stop* (its words on two
lines of 18 in the column, 8 inside above and below), what was asked, the
steps — each line wrapping rather than cut, the newest kept seen and the
oldest clipped above when they do not fit — then the question with
*Allow* and *Not now*; at least 16 under them, the offers and the box, 56
tall, at the column's foot, where the conversation's box is. In the
column the box's hint is *Ask, search, or type a website…*: the first
screen's is wider than the column's field and would be cut (item 827). The
conversation's earlier turns are not drawn while it works.

| The window, beside the rail | The page box | The column |
|---|---|---|
| 1280 × 820 (the design's) | (17, 63) 798 × 740 — 878 × 520 in the conversation | (832, 0) 384 × 820 |
| 1680 × 1050 | 1198 × 970 | (1232, 0) 384 × 1050 |
| 1202 × 820 (the least) | 720 × 740 | (754, 0) 384 × 820 |
| 1201 and narrower | the conversation's: the column under the page, as above | — |

**Its least width is a page box 720 wide** — a window of 1202 beside the
rail: past any phone's width, so a site lays the page out as for a tablet
or a desktop while alo works on it, and the split never makes the page
more than 158 narrower than the conversation's 878. Chrome is told the
split's page box as the run starts and the conversation's as it ends,
through the road a resize takes (item 774), and a press is aimed at the
page box where the picture pressed drew it.

**The panel's own choice is kept.** The run never writes over what a
person chose: a panel shown before it is shown after it, one hidden stays
hidden; *Hide the panel* or *Show the panel* pressed during the run is
kept for after it, and *Show* says *alo is working on this page: the
panel comes back when it has finished* until it does.

**When the run ends** — done, stuck, stopped, or a press on the page
taking over — the window is the conversation again, the panel what the
person chose, and what the run ended with is drawn under the page in the
card, stuck's choices with it, as before the split.

### As designed

- The window splits: the page large on the left, the agent's column on the
  right (384 px), the panel a rail (built, item 819).
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
