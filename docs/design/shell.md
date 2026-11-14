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

- **The mark**: the owner's icon (`docs/brand/`), 24 px square, then 8 px
  on, *alo* in the serif, 22 px (item 913), in a row 46 px tall with the
  button that hides the panel. It is the icon's small drawing,
  `alo-browser-icon-small.svg`, as every picture under 64 device pixels
  is (item 914; ADR 0067 § 6).
- **New chat**, a white button 40 px tall, a plus before its words and
  `⌘T` at its end.
- **Search chats**, 38 px.
- **Open** — the chats in use. Each row is 38 px, 13.5 px text, the site
  open in it as a 16 px mark before its name. The chat on screen is white.
  A chat where the agent is working keeps its whole name on the first
  line, and under it, 2 px down, alo's mark, 14 px, and **Working**, 12 px
  600 in deep teal, so the name is never cut for them — the row 56 px tall
  (item 985; item 974 put them after the name, and a 7 px terracotta dot
  before that). One it finished has a check mark and **Done**, 12 px 600 in
  the navy ink at its row's end — no green, so deep teal means only alo
  acting (item 985). Both say so in words to an agent.
- **Earlier** — past chats, in quiet text, without marks. Closing a chat
  never loses it: it moves here, first (item 791). The chats kept from
  before alo was last closed are here, newest first; pressed, one is on
  screen and under *Open* again — first there, the most recent (items 757,
  760, ADR 0092).
- **A chat's name** is the first words said in it — up to 48 characters,
  whole words, `…` when there was more — and *New chat* until something is
  said (item 758). Only the first turn names it, so a name never joins two
  turns' words. **An address in it reads as its host** (item 859):
  *https://en.wikipedia.org/wiki/Kampala* is *en.wikipedia.org*, a port
  kept (*127.0.0.1:8736*), the scheme and path left to the turn, which
  holds the address whole a press away — the path was considered and
  left out: it is mostly opaque (ids, `wiki/…`), it would push every
  second site's name past the row, and a host is what tells two chats
  apart in the panel and on the rail. **Two addresses typed with nothing
  between** — an `http://` or `https://` straight after a letter or digit
  — are two words, never one; one after `/`, `=` or `?` is part of the
  address that carries it. The same host twice in a row is said once.
  Two hosts glued with no scheme between (*example.comexample.com*) are
  one host to anyone, and stay so. **A name kept before 859** that is
  exactly what 758's rule made of the chat's first kept turn is alo's own,
  and is named again by this rule as it is read; any other name is kept as
  it is (ADR 0092's amendment of 7 November 2026).
- **Settings**, at the foot, under a line, with a settings icon — two
  lines, a knob on each — before its words. Pressed, it opens *Settings*
  in the conversation's place, and its row is then white, as the chat on
  screen's is, and no chat's row is (item 854, below).
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
window: the mark — the owner's icon alone, 28 square, in the middle (item
913), in its small drawing (item 914) — 40 tall 10 down; *New chat*, a 44-square button with an
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
  chat* `NC`, a name with no letter a middle dot) — **a word that is a host
  or an address giving its site's name's letter** (item 860,
  `rail_initials.rs`): the first letter of its host's first label that is
  not a prefix, a prefix being, before at least two more labels, `www` (or
  `www` and digits), `m`, `mobile` or `amp`, or two letters with perhaps a
  hyphen and two to four more — a language or a country (*en.wikipedia.org*
  `W`, *en.m.wikipedia.org* `W`, *www.bbc.com* `B`, *m.youtube.com* `Y`, a
  whole address kept before 859 *https://www.theguardian.com/…* `T`, never
  `H`). A host of two labels keeps its first (*go.dev* `G`), and a first
  label of the site's own stays (*news.bbc.co.uk* `N`); a site whose own
  name is two letters before two labels (*my.site.example*) is read past,
  to `S` — the rule's known limit. An IP address is its first digit —, 12 px 600 in the ink in
  a **32 px circle** on the line colour. **The chat on screen** is white
  with a shadow, as its row is white in the panel; one the agent works in
  has, under its circle, a line the rail's width holding alo's mark, 12
  px, and **Working**, 10 px 600 in deep teal; one it finished a check mark
  and **Done** there in the ink; either button 58 tall (items 974, 985; the
  mark at the circle's shoulder, and a 7 px terracotta dot, before). Five fit above *Show the panel* in
  the least window.
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

The box has the keyboard as the window opens and on every new chat —
*New chat* pressed, or ⌘T, the key drawn on it — with its caret blinking,
so a person's first keys reach it with no press, as a Mac's search field
in a new window has them (item 875). A press on the box gives it the
keyboard; a press anywhere else takes it away, and while it has it the
box's border is the quiet text's colour.
What a person types is shown as the field's value. Under the box, 8 px
below it, a white card 720 px wide with a line border, 14 px corners and
6 px inside holds the offers, each 52 px tall — its words at 15 px, and
under them at 13 px in quiet text where it leads or *Not set up yet* —
the one Return chooses on the paper's colour with 10 px corners:

- *Go to the site*, and the address it goes to, first — only when what is
  typed is an address: `example.com`, `localhost:8080`, `127.0.0.1`,
  `https://…`; never words with spaces between them, `3.14`, a mail
  address, or another scheme. Typed with no scheme it is read as
  `https://`, but for this machine and its own network — `localhost`, a
  name ending `.localhost`, and an address that is not the public
  internet's (loopback, `10.`, `172.16.`–`172.31.`, `192.168.`,
  link-local, a unique local IPv6 address) — which is read as `http://`,
  as Chrome reads them: a server a person runs there almost never has a
  certificate, so `https://` was an error page (item 874). A scheme
  written is kept.
- *Search for “…”*, saying *alo searches the web* under it when a key is
  held (never the provider's name, item 900),
  and otherwise, in quiet text, *Not set up yet — why* (708); then *Ask
  alo about “…”*, in quiet text, saying *Not set up yet* until the agent
  is built (704); a search's own answer is drawn with its results (707).

Escape empties the box (and, empty, lets the keyboard go), and Return or
*Go* chooses the first offer; every other key is a text field's, below. A press
on any offer chooses that one as Return chooses the first (714); in a
conversation the offers stand above the box, and a press on one standing
over a live card's page is the offer's, not the page's (759).

### The one box as a text field (items 861, 848)

Approved by the owner 2026-11-07 (*"Yes please"*): the one box and
*Search chats* behave as a text field does on a Mac — paste, copy, cut and
select all; arrows moving the caret, by a word with ⌥ and to an end with
⌘, ⌥ and ⌘ with Backspace erasing a word or to the start, the forward
delete; a press placing the caret, ⇧ with the arrows, a drag and a double
press selecting; undo and redo; Japanese and Chinese input methods with
their candidates at the caret; a drawn caret and selection. Settled here,
following macOS's own fields where in doubt:

- **Keys**, while a field has the keyboard: ← → a letter back or on (a
  letter with its marks, an emoji's sequence or a flag is one), ⌥ a word,
  ⌘ to the start or the end; ↑ and ↓ to the start and the end, as a
  one-line field has them; ⇧ with any of them grows or shrinks the
  selection instead, and without ⇧ a selection is put away at its start
  (←) or its end (→). Backspace erases the letter before the caret, ⌥ the
  word before, ⌘ all before; ⌦ (fn-Backspace) the same after it. A letter
  typed over a selection replaces it. Return, Escape, Tab, Home, End and
  the paging keys are as before.
- **A word** is what Unicode's word boundaries (UAX #29) make one, nearly:
  letters, digits and `_`, going on across one `.`, `:`, `'`, `’` or `·`
  between two of them, and digits across `,` or `;` between two digits —
  so `en.wikipedia.org`, `don't` and `3,000` are each one word, and `/`,
  spaces and other marks stand between words. ⌥→ passes over what stands
  between to the next word's end, ⌥← to the word before's start.
- **Shortcuts.** A field takes six ⌘ keys: ⌘A select all, ⌘C copy, ⌘X
  cut, ⌘V paste, ⌘Z undo, ⇧⌘Z redo — by the key's letter in the person's
  layout. **Every other ⌘ key is the browser's or the system's and types
  nothing** — ⌘T, ⌘W, ⌘Q, ⌘, among them. With no field holding the
  keyboard the six go nowhere; with a card's page holding it, they are
  the page's.
- **macOS's Ctrl bindings** (item 865), as its text system's standard key
  bindings have them for a one-line field: ⌃A and ⌃E to the start and the
  end, ⌃B and ⌃F a letter back and on, ⌃P and ⌃N the line above and below
  — in one line, the start and the end — ⌃H and ⌃D the letter before and
  after, ⌃T the two letters about the caret swapped and the caret on (at
  the end, the last two), ⌃K **kill** from the caret, or the selection's
  start, to the end, and ⌃Y **yank** what was last killed in either field
  at the caret. The kill buffer is the window's own, never the clipboard,
  as a Mac's is its application's; a kill and a yank are each a step to
  undo. Nothing else with Ctrl types: ⌃ with ⇧, ⌥ or ⌘ too, ⌃O, ⌃L, ⌃V.
- **The clipboard.** ⌘C and ⌘X put what is selected on the system's
  clipboard with `pbcopy`, as *Copy link* does; ⌘V reads it with
  `pbpaste -Prefer txt`, at most 16 KiB of it, cut back to a whole
  character. A clipboard that cannot be read or written is said in the
  first screen's line, and nothing changes. A test hands the window a
  reader and a writer of its own, so no test touches a person's clipboard.
- **What is pasted is one line**: each line break — `\r\n`, `\r`, `\n`,
  U+2028, U+2029 — and each tab is a space, the white space at its two
  ends is dropped (so a line copied with its newline pastes as the line,
  and an address stays an address), and control characters are left out.
  The field holds at most 2,048 characters, as before: what would go past
  is left out, pasted or typed.
- **The caret**: 1 px wide — a point, as macOS's own — in the ink, 20
  tall in the box's 40, 16 in *Search chats*' 20, in the middle, standing
  where its place's letters end, on whole device pixels (2 px, tried first,
  read heavier than macOS's on a Retina display, by hand). It **blinks: shown half a second,
  hidden half a second**, from the moment it last moved (an edit, an
  arrow, a press, the field given the keyboard), so it is always there
  while a person types or moves it. Half a second is Chromium's default
  interval; a Mac's own setting is not read, and the rate was not measured
  against macOS's fields. It is drawn only while the field and the window
  both have the keyboard and nothing is selected.
- **The selection** is drawn behind its letters on **the ink's tint** —
  the ink at 18 % over the box's white, `#d4d9dd` since the ink is alo OS's
  navy (item 974; `#d6d8da` before) — the letters keeping the ink, 10.3 : 1
  on it; as tall as the face's line, 22 in the box and 18 in *Search
  chats* in Manrope (item 975; 20 and 16 in Inter). The
  palette's ink is *what is selected* (ADR 0074 § 2); a block of solid ink
  behind white letters read as a button, and the acting colour means the
  agent. macOS greys a selection in a window without the keyboard; here it
  keeps its tint (868).
- **Words longer than the field** scroll only to keep the caret in view,
  as macOS's do: a caret past the right edge brings it to stand at that
  edge, one past the left edge to the left edge, one in view moves nothing;
  the words are never scrolled while they fit, nor so far that room is left
  after their end; whole device pixels. A field without the keyboard shows
  its words' start.
- **Undo** groups as macOS's text system does: letters typed straight on,
  each where the one before left the caret, are one step; Backspaces in a
  row are one, forward deletes another; a paste, a cut and the field
  emptied with Escape are each a step alone; the first edit after the
  caret was moved, the selection changed or a press placed it begins a new
  step, and so does a letter typed over a selection. Undoing puts back the
  words and where the caret and the selection were; ⇧⌘Z does it again,
  until a new edit. At most 100 steps are kept. Choosing what the box holds
  — going, searching, asking — empties it and forgets them.
- **A press** on the field puts the caret at the place nearest it, as the
  words were last drawn — scrolled, at the first letter shown; a second
  press within half a second and 4 px selects the word there (or the run
  of spaces, or the one mark); a third selects everything; the pointer
  moved with the button still down after a first press selects from it, to
  the place nearest the pointer — past an edge, to that end at once (a
  gradual scroll while dragging past an edge is 866). **A press with ⇧**
  grows the selection to it (item 863): a place before the selection keeps
  its end, one after it its start, so what was selected stays selected;
  a place inside it moves the caret there, the anchor kept; however quick
  after another press, never a word selected; with the button still down,
  a drag goes on growing it. It is the field's only when the press lands
  on one — decided by the field's renderer as any press is — and anywhere
  else a press with ⇧ is nobody's, refused and said as every press with a
  modifier is (798, 805).
- **Input methods** (848): allowed while the box or *Search chats* has
  the keyboard, as they are while a card's page has it (802). What is
  being composed is drawn underlined, in the ink, at the caret in place of
  the selection; the candidates stand under the caret — 1 px wide, the
  field's height — and what is chosen goes in over the selection, as
  typing does (one step with typing beside it). A composition put away
  leaves nothing. While a person composes, the field's value as an agent
  reads it holds what is composed, as a browser's field does.
- **What an agent reads**: the field, a `textbox` named as before, its
  value, and **its selection as a range of characters** — `caret=16` when
  nothing is selected, `selection=8..18` when something is — counted in
  Unicode characters, while it has the keyboard; never a place on the
  screen (ADR 0002), and no verb is added. The line the words are drawn on
  is hidden from it.
- ***Search chats*** is the same field in every way, its words 13.5 px:
  Return chooses nothing, and Escape empties it and then lets the keyboard
  go.

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
*Searching for “…”…*, *No sites found for “…”.*, or why: the key refused
(401, 403), too many searches or the allowance used up (429), the search
service unable to answer (5xx), unreachable, or its answer unreadable —
each saying *The search service*, never the provider's name: the owner's
word of 9 November 2026, *"we do not need to show any brave words on the
ui or to the users"* (item 900). No word a person reads — a tab's
sentence, the line at the window's foot, the note under an answer, the
offer, what the agent says it found — names it. The line at the window's foot says the same, and how many sites
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
  An agent reads it either way.
- **Pressed and dragged** (item 845): the bar, 6 across, is too thin to
  hit, so a press is its anywhere in a strip **16 wide centred on it**,
  between the room's top and foot — where the pointer has already brought
  it back whole. On the thumb, a drag: the conversation follows the
  pointer as far as the thumb moves (the pointer's travel times the
  conversation's travel over the thumb's), kept inside how far the turns
  reach, until the button comes up, the bar whole all the while. Above or
  below the thumb, a page back or on — the room less a line, as Page Up
  and Page Down — as a Mac's scroll bar does with *Jump to the next page*,
  its default. **Held, it pages on** (item 876), as a Mac's does —
  measured on this Mac (macOS 26.5.2, an `NSScroller` of a probe's own, a
  press held by `cliclick`): a page at the press, the next **500 ms**
  later, then one every **50 ms**, until the thumb reaches the pointer
  (which may move as it is held), or the button comes up. A press on the
  bar is the window's own: never handed to the first screen's renderer to
  claim, and the keyboard stays where it was. Not in the split view, or
  while the turns fit; on *Settings* as here (item 880).
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

**A cookie banner answered** (item 920, ADR 0108 § 4): after the address
and before the shield, in the bar's quiet 13 px words as *Loading* is, 8
either side: **Cookies refused** once alo pressed the page's banner's most
private choice, or **Cookie banner left to you** when the banner offered
none alo would press (or its refusal could not be pressed). Nothing to
press; an agent reads it as a paragraph of the bar. The terminal says the
whole of it, the choice's own words quoted. Gone when the card's page goes
somewhere else.

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
new chat* over a link, and *Search for “…”* when words are selected —
searched whole, and shown ending in `…` where the line reaches the menu's
edge, the closing quote kept and following the `…` directly, in any face
(item 855: a cut at 16 characters fitted ordinary words, never wide
letters; item 857: the renderer measures the words in the line's face and
writes them already cut, since a box the stylesheet clips is as wide as
its room and left the quote at the menu's edge, apart from the `…`). A white box
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
*Search chats* take them too, their candidates at their own caret (848,
*The one box as a text field*).

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
works the card and its page are outlined in deep teal `#0F6B72` (2 over
the page, the card's border and 1 more around it), and a line 36 tall on
the teal's tint `#E2EDEE` holds alo's mark, 16 px, then 8 on *alo is
working on this page · click anywhere to take over* in the teal, 13 px,
600, with **Stop** at its end (28 tall, rounded 8, outlined in the teal)
(item 974; terracotta, `#F6E7E0` and `#7A3A24` before); 10 under it what the person asked (13.5 on 20, 600),
then one line per step in the quiet colour, 6 apart, why under a step when
it chose (12.5 on 18), the step under way in the teal. Asking before
pressing what pays, sends, deletes or sends a form: a box rounded 12, 12
by 14 inside, the question (600), then **Allow** in the ink and **Not
now**, 28 tall, 8 apart. Stuck: why, then each choice as a button. Done:
*Done in N steps* in the ink (item 985; green before) and the result in full, **as it was
written** (item 883): its lines kept, an empty line between paragraphs 8
apart, and lines beginning `- `, `* `, `•` or `–` a bulleted list (its
bullets 18 in), lines beginning `1.` or `1)` a numbered list keeping its
numbers, items 2 apart — nothing else read as structure; to an agent
paragraphs and lists. A press on
the page stops it and gives the person the page. It never types a
password, nor a card's number, security code or expiry: the card refuses
them, for any agent (826). Nothing of Chrome's is drawn over the card's
page — no inspector highlight, no tooltip (822); alo's own ring on a card
is 823. Its plan (824), taking over and carrying on (825) and what a
payment pays (826) are below, and so is beginning with no page open
(821). **Each run is kept** with the page it
worked on (820): once the page is shown earlier, under its line, what the
person asked as their bubble, then what alo did on a white box rounded 12,
14 by 16 inside — its steps in the quiet colour, 6 apart, why under a step
— and 10 under them *Done in N steps* in the ink (item 985; green before) and the result,
in its lines and lists as on the card (883), or why it was stuck or
stopped; nothing on it pressable. A text typed that
the person did not say is never kept: that step reads *Typed into a
field*.

### The split view (item 819)

**While the agent acts or asks, the window splits.** The panel is the
rail for the run, at any width; the first screen beside it is no longer
the conversation but **the page, large, on the left** — the card with its
bar, 16 in from the screen's top, left and foot and from the column, its
page box the rest of the card under the bar, outlined in the teal — and
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

### The agent's plan (item 824)

The owner's design of 6 November 2026 (*"I agree"*), approved by the
owner 2026-11-06: *before acting it writes 2–5 short steps in its column;
done steps ticked, the current one bold, the rest greyed.* Settled by the
builder, and recorded here:

- **How it is asked.** Each of the model's three verbs (`activate`,
  `put_text`, `scroll`) carries two more fields in the strict JSON schema
  every tool already has: `plan`, a list of strings or `null` — with its
  first action two to five short steps of the whole task, after that
  `null` — and `on`, the number of the plan's step this action is for,
  counting from 1. Both are required, as `strict` asks; `done` and `stuck`
  carry none. No request names another, so every request is sent the plan
  as it stands, each step's state `done`, `now` or `ahead`.
- **Kept to its bounds.** At most five steps, each at most 80 characters
  (cut with `…`), steps without words dropped; a step number past the last
  is the last. A model that gives no plan has none drawn, and the run goes
  on as before. The wire and the chat's file refuse a plan past five
  steps, a step past 80 characters, or one on no step.
- **Revised mid-run.** A plan given again, not `null`, replaces the plan
  whole; the steps already taken stay in their lines below it, so nothing
  done goes from sight. The window is told the plan each time it, or the
  step it is on, changes.
- **Drawn** under what was asked, 10 under it and 10 above the steps: a
  line of 20 each, 2 apart, 13 px; its words 18 in, after a tick's room of
  14 and 4 — the steps done ticked (`✓`) in the quiet colour, the one it is
  on in the ink, 600, the rest greyed `#bdb6aa`. Done, every step ticked;
  stopped or stuck, as it stood. The same in the split view's column,
  where its words wrap rather than being cut, and in the column under the
  page.
- **Kept with the run** in the chat's file (ADR 0092, sealed under ADR
  0098 like the rest): the plan as it last stood, its steps' words kept as
  the steps' lines are — a text typed that the person did not say
  replaced by `…` — and drawn under the page's line once it is shown
  earlier. A chat kept before plans were reads as having none.
- **To an agent reading the screen**: a list named *Plan*, each step a
  list item named by where it stands and its words — *Done: Search
  Wikipedia for Kampala*, *Now: Open the article on Kampala*, *Next: Read
  its population* — the one it is on `aria-current="step"`.

### Taking over and carrying on (item 825)

The owner's design, approved by the owner 2026-11-06: *after the person
takes over, a "Let alo continue" button; it resumes from the page as
left, telling the model what the person changed.* Settled:

- **Taking over holds the run, not ends it.** A press on the card's page
  while alo works (*click anywhere to take over*) — or any other press of
  the person's there — holds it at once: nothing more is sent to the
  page, a model's answer in flight is thrown away, a question waiting on
  the person is dropped, and a verb already sent is finished by the page.
  The window is the conversation again (no outline, no *Stop*, the panel
  what the person chose, Chrome told the conversation's page box), and
  under the page the column says *You took over: the page is yours. alo
  carries on from it as you leave it.* in the quiet colour, then **Let
  alo continue** 10 under it, 28 tall, in the ink. The plan and the steps
  stay drawn; no step is under way.
- **Let alo continue** reads the page anew and tells the model, as a step
  of the person's — *The person took the page over, then let alo
  continue* — what they changed since the run last read it: the address
  before and after when it moved (and then nothing of the old page's
  fields); each field whose value changed on the same page, by its name —
  text by its words, *from “Ada” to “Ada Lovelace”*, a checkbox or radio
  checked or not, a list's option chosen — and fields that came or went;
  at most twelve, each value cut to 80 characters; and where the run had
  got to: how many steps it had taken and the step of its plan it was on.
  **Never a password's value, never a card's details**: a field whose name
  or hint says it holds one, or whose words are a password's dots, is said
  only as changed, by its name. The model is told to go on from the page
  as it is now and not to undo what the person did unless asked.
- **The person's gate still holds.** The run carries on through the same
  road: every press that pays, sends or deletes is asked again — a
  question dropped as the person took over is asked anew if the model
  chooses that press again.
- **Its limits are counted across the pause**: its steps (the person's is
  not one of them) and its tokens. Held ten minutes, it stops: *alo
  stopped: the page was yours for 10 minutes. Ask again to go on from what
  the page shows now.* *Stop* is not drawn while it is held; asking alo
  something else, opening another page or closing the card stops it, as
  before.
- **Its limit, said:** the tree the run last read may have been read
  before its last verb was carried out, so a field that verb filled reads
  as the person's; the verb and what came of it are in the steps the
  model is sent too.
- **To an agent**: in the region *alo at work*, the line as a paragraph
  and *Let alo continue* as a button, claimed as itself and decided by the
  browser process only for the run it painted held.

### Paying (item 826, ADR 0099)

The owner's design, approved by the owner 2026-11-06: *before any
pay/confirm press, the question shows amount, merchant and the page's own
summary lines, with Pay and Not now. The agent never enters card
numbers.* Settled:

- **Which presses.** Those the person's gate holds because they pay — by
  the words *pay, payment, buy, purchase, checkout, order, subscribe,
  donate, book, rent* — and, since this item, *Confirm* or *Complete* on a
  page that shows a total to pay.
- **Read off the page's own tree, by the browser process, never by a
  script and never asked of the model** (ADR 0099 § 1): the **amount** —
  a currency's sign or code beside a number — in the words of what is
  pressed (*Pay UGX 45,000*), else on the last line before it that says it
  is a total (*Total*, *Order total*, *Amount due*, *To pay*, *You pay*,
  *Balance due*; *Subtotal* is not one); never a price merely nearest the
  button, which is as likely an item's. The **merchant**: the page's own
  name for itself — its title — and the site's host, which the page cannot
  say otherwise: *Kampala Coffee — Checkout (shop.example)*. The
  **summary**: the page's own lines under the first region, group, list or
  table named as a summary (*summary, your order, order, basket, cart,
  bag*) — whatever its role but a field, a button, a link or a heading —
  or under a heading saying so, to the next heading: a row (Chrome's
  `LayoutTableRow` too, as it reads a table with no headers), a list item,
  a paragraph each; never the words of a button or a field; at most six,
  the last saying how many more. A line saying it is a total with its price
  in the next cell takes that price.
- **What cannot be found is said plainly, and it still asks**: *alo could
  not find the amount on this page: read it on the page before you pay.*
  and *alo could not find a summary of what you pay for on this page.*
- **Drawn** in the question's box under the question: *Amount* and *To*
  in the quiet colour 56 wide, their values 8 after in the ink, 600, the
  amount at 15 px; 4 apart; then under a rule the summary lines, 12.5 on
  18 in the quiet colour; then **Pay** in the ink in place of *Allow*, and
  **Not now**. *Pay* is claimed as *Allow* is and allows only the question
  the person was shown: the page is read again and pressed only if it
  would ask exactly the same — the amount, the merchant and the lines too.
- **A card's details are never the agent's** (ADR 0099 § 2): text put
  into a field of a card's number, security code or expiry — or an option
  chosen in such a list — on a card's page is refused **by the card, for
  every agent**, by a refusal of its own kind, `CardDetails`, whatever the
  page names the field: by its `autocomplete` (`cc-number`, `cc-csc`,
  `cc-exp`, `cc-exp-month`, `cc-exp-year`), or by its `name`, `id`,
  `placeholder`, `aria-label` or name in the tree holding a name such a
  field goes by. A card's holder's name is not refused. Met by alo's
  agent, the run is stuck: *This page asks for a card's details, and alo
  never types or chooses a card's number, security code or expiry: paying
  details are yours.* What such a field holds is withheld from the model
  too, as a password's is.

### Beginning with no page (items 821, 878, ADR 0103)

The owner's design of 8 November 2026, approved in full: *with no page,
the box's second line is* Ask alo to do it *(*alo finds the site and works
on it*); the run gains two verbs used to begin, a search through the box's
own search (results read as words) and opening a result or an address it
proposes, each a step in its column, the page a card as if the person
opened it; it opens without asking a search result or a site the person
named, asks (*alo wants to open …*, Allow / Not now) for any other, and
never opens an address on this machine or its network or a file unless
the person typed it; at most 3 sites opened a run (links followed inside a
site not counted) beside the 20 verbs; earlier pages stay as cards; a site
that needs signing in stops the run and hands the page over, and* Let alo
continue *carries on. The limits are the person's to set (878).* Settled
here:

- **The box.** With no card open, Return still searches — searching is
  first — and the second line is **Ask alo to do it**, *alo finds the site
  and works on it* under it, in the offer's quiet words; pressed, it asks
  alo. With a card open the box offers *Ask alo about “…”* first, as
  before. The line at the window's foot says *alo is at work on “…”*.
- **Where the run stands before a page.** A turn of its own: what the
  person asked as their bubble, then, 16 under it, the column a card's run
  has, on a white box rounded 12 under a line of 1 the column's 880 wide —
  no page above it, so no rule and no outline — its working line *alo is
  finding the site to work on* with **Stop** (there is no page to take
  over yet), what was asked, its plan, one line per step, and a question
  with **Allow** and **Not now**. Not split: there is no page to put
  beside the column.
- **Searching** is a step — *Searching for “Uganda Museum opening
  hours”* — asked through the box's own search, the same provider, key and
  request a person's search makes, and its sites told to the model as
  words: numbered across the run, each its title, address and description,
  at most eight a search and forty-eight a run. They are not drawn as a
  search's results: the step says it.
- **Opening** is a step — *Opening the Uganda Museum's site* — and the page
  arrives **as a card, as if the person had opened it**: chosen as an
  address is and opened through the same road (ADR 0075), the turn asked
  with no page becoming that card, keeping what the person asked as its
  bubble, the run moved onto it and splitting the window as any run on a
  card does. A second site puts the first card away as its address, its run
  so far kept as *alo went on to …*, and opens the next as a new turn whose
  bubble is the result's title or the site's host. A page that cannot be
  opened leaves the run in its own turn again, the model told why.
- **Whether it asks** (the browser process's, never the model's, ADR 0103
  § 2 as amended): **by default it asks before no site** — the owner's
  decision of 8 November 2026 (item 884), less strict, as other browsers'
  agents are — and any public `http` or `https` site opens with no
  question. Set in *Settings* to *A site not found or named* (the default
  before 884): a search's result or a site the person named — an address
  in their words, the same host, or on the public internet the same
  registrable domain — opens with no question; any other is asked: *alo
  wants to open wikipedia.org. It is not a site a search found or one you
  named.* with **Allow** and **Not now**, in the question's box. *Not now*
  ends the run, nothing opened. Set to *Any site*, every site is asked:
  *alo wants to open ugandamuseum.go.ug.*
- **Never this machine or its network** unless the person wrote the
  address — whatever *Ask before opening* says, for this is not a setting: `localhost`, `….localhost`, `….local`, an address that is not
  the public internet's, and a public name that resolves only to one —
  refused, the model told *… is on this machine or its network, and alo
  opens such an address only when the person wrote it*, and the run goes
  on. Only `http` and `https` are opened: never `file:` or any other
  scheme, even typed — the box opens none either.
- **How many sites**: at most 3 a run unless set otherwise, a site being
  the registrable domain; a site it has been on — the page it began on, one
  a link took it to — is never counted again, and a link followed is never
  counted. Past it, the model is told *alo has opened the 3 sites one task
  may open (Settings, alo at work)* and goes on with the pages opened, or
  finishes. Each search, open, press, typing or scroll is one of its 20
  actions.
- **Signing in.** A page it reaches — opened, or by a link or a form —
  that shows a password field on the screen, or that refuses it a
  password, **hands the page over**, as taking it over does: no outline, no
  *Stop*, and under the page *This site needs you to sign in: the page is
  yours. Sign in on it, then let alo continue.* with **Let alo continue**,
  drawn where *You took over* is. Carried on, the model is told the page
  asked the person to sign in and what they changed — never a password's
  value.
- **To an agent reading the screen**: the turn's column is the region
  *alo at work* as a card's is; the offer is an option named *Ask alo to
  do it alo finds the site and works on it*.

### As designed

- The window splits: the page large on the left, the agent's column on the
  right (384 px), the panel a rail (built, item 819).
- First its plan, two to five short steps, those done ticked, the one it
  is on bold, the rest greyed (built, item 824); then **one line per
  step, as it happens**, in a person's words — *"Choosing Saturday,
  10:00"* — with why, when it chose: *"8:00 is taken, so this is the
  earliest free morning slot"*.
- The page has a deep teal outline and alo's mark with *alo is working on
  this page · click anywhere to take over*. Clicking the page takes over; the agent pauses,
  and *Let alo continue* carries it on from whatever the page then shows
  (built, item 825).
- **It stops before paying**, with the amount, the merchant and the
  page's own summary lines, and *Pay* / *Not now* (built, item 826).
  Paying is always asked; sending, deleting and cancelling are asked
  unless a person said otherwise.
- **It never types a password, nor a card's details.** Signing in and
  paying details are the person's: a page that needs signing in is handed
  to them (built, item 821).
- **With no page open it finds the site** — searching, opening a result or
  a site the person named, asking before any other, never their own
  network (built, items 821 and 878).
- **When it is stuck it says so** and offers choices, the closest to what was
  asked first. It never picks something it was not asked for.
- Done: one line, *Done in 6 steps*, the result in full, and what to do next.

## Typefaces

Approved by the owner on 7 November 2026 (*"Yes please"*): **Inter for
every word of the interface** (item 853); **Newsreader** — *the serif* in
this file — for the mark *alo* and the greeting, at 400, **whatever is
chosen**. A person may set the interface's words in one of four other
faces instead, in *Settings* (item 854, below): **Geist**, **DM Sans**,
**Plus Jakarta Sans**, and **Instrument Sans** — the design's face until
then, kept as a fifth choice. Each at 400, 500 and 600.

All six are under the SIL Open Font License, compiled into the
application and handed to the interface's renderers alone (items 706 and
853, ADR 0084) — all of them as the renderers start, so a face chosen is
drawn at once, with nothing handed again; never to a page's. A stylesheet
asks for `var(--face), system-ui, sans-serif` wherever it sets words, or
`Newsreader, serif`; `--face` is Inter on each document's root, and the
document builders put the face chosen there as the root's own style.
Inter's file calls its family *Inter Variable*, so that is the name asked
for. Emoji are drawn in **Noto Color Emoji**, carried and handed the same
way (ADR 0088). A face with an optical size — Newsreader's 6 to 72,
Inter's 14 to 32 and DM Sans' 9 to 40 — is set to the CSS pixels its text
is at, as Chrome sets it (measured in Chrome 157, item 853).

**Since 12 November 2026** (item 975; ADR 0112 *Answered* 3, the owner's
decision through alo OS's coordinator: *"Manrope"*): **Manrope** is every
word of the interface by default, and **Geist Mono** whatever it sets in a
monospace (`code`, `kbd`, `pre`, `samp` — none today); `--face` is Manrope
on each document's root and `--mono` Geist Mono. Inter stays, as a choice in
*Settings* with the other four — the owner's call — so *Settings* offers
six: **Manrope** (default), Inter, Geist, DM Sans, Plus Jakarta Sans and
Instrument Sans. Newsreader is still the mark's and the greeting's. Both new
files are under the OFL and carried as the others are (ADR 0084, amended).
Where this section or the next says *Inter* as the default, read Manrope.

## Settings (items 853, 854, 878, 886, 989)

Approved by the owner on 7 November 2026 (*"Yes please"*), in their
words: *Settings opens as a page in the conversation area, with one
section to start: Font. Choices: Inter (default), Geist, DM Sans, Plus
Jakarta Sans, and Instrument Sans. Each is shown as a sample of a real
result (title and snippet) set in that font. Choosing one changes the
whole interface at once, with no restart. Web pages inside cards are not
affected. The choice is saved on this Mac beside the chats and remembered
across launches. First Inter as the default, then the Settings page with
the choice.*

On 9 November 2026 the owner changed the sample (item 901): *"instead of
kampala we should use this — "if you live each day as if it was your last
someday you'll most certainly be right""* — so each face shows that one
sentence, not a result's title and snippet.

Approved by the owner on 8 November 2026 as **a popup** (item 886), in
their words: *a centred white panel, about 720 wide, rounded 16, with a
soft shadow, that scrolls inside; behind it the conversation blurred and
dimmed as the narrow window's sheet dims. Settings titles it with × at the
top right; its sections are inside (a list on its left once there are
more than two). It opens from the Settings button and from ⌘, and closes
with Escape, the ×, or a press on the blurred area, which presses nothing
under it. A choice applies at once. Keys walk it as they walk Settings
now; the keyboard comes back to where it was on closing. An agent reads it
as a dialog named Settings.*

Settled here, as the design left them open:

- **Opened** by the panel's *Settings* row, pressed — or in the sheet over
  a narrow window, which it closes, as a chat chosen does — and by **⌘,**
  wherever the keyboard is, as every Mac application's *Settings…* is:
  the key that writes `,` with nothing held, wherever the layout puts it
  (on this Mac's ABC – AZERTY, beside N), with exactly ⌘ held. ⌘, again
  while it is open is nothing. Its row is white while it is open, as the
  chat on screen's is, and no chat's row is; an agent reads the row's link
  as the current page. The rail has no *Settings*; ⌘, opens it there too.
- **Over the conversation.** The first screen is drawn under it exactly as
  it was — its turns, how far it was scrolled, what was typed into the
  box, a live card — **blurred by 8 and dimmed by the conversation's ink,
  `#1b2433`, at 0.32**, as the narrow window's sheet dims (776): a veil the
  whole first screen, `backdrop-filter: blur(8px)` (item 885, ADR 0104).
  A live card's page is not streamed while *Settings* is open (ADR 0076:
  only while the card is on the screen, and it is behind a modal); its last
  frame is drawn in the card, so it is blurred with the rest, and beneath
  again once *Settings* closes. Nothing under the veil is pressed,
  scrolled, hovered or typed into, and an agent reads none of it. The
  panel beside it is not under the veil: it is drawn and pressed as it
  always is, a chat chosen in it closing *Settings*.
- **Closed**, back to the chat as it was, by **Escape** (unless *Search
  chats* has the keyboard, which Escape empties first), by **its ×**, by
  **a press on the veil** — which presses nothing under it, not the box,
  not a card's page — by a chat chosen in the panel (the chat on screen
  included) and by *New chat*. Choosing a face does not close it. Closed by
  Escape, its × or the veil, **the keyboard goes back to where it was** as
  it opened: the box, *Search chats*, the card's page, or nothing; a chat
  chosen or *New chat* puts it where they put it.
- **The panel**, centred in the first screen: 720 wide, or the room less
  32 either side; as tall as what it holds, and at most the room less 32
  above and below; white, rounded 16, its shadow `0 24px 64px` and `0 2px
  8px` of the ink at 0.22 and 0.08. At its top, 20 down, 28 in from its
  left and 20 from its right, a row of 36: **Settings**, 22 px 600, and
  its **×**, 36 square, the card's ×. 8 under that row, what it holds,
  which scrolls: 28 in from either side, each section 24 under the one
  before, 28 under the last. **Font**, 15 px 600, and under it, in quiet
  13.5 px, *The face alo's own words are set in. Pages in cards keep their
  own.*; 14 under that the faces, **two a row, 10 apart**, in the order
  above. Its sections are one after the other while there are two; a list
  of them on its left is for when there are more.
- **A face** is a white box, 1 px line-colour border, rounded 14, 15 / 16
  / 16 inside: a row of 18 holding a radio, 16 across, drawn as Chrome
  draws one in the ink, and the face's name in 13 px 600 (*Manrope ·
  Default* for Manrope since item 975; *Inter · Default* before) — quiet, or the ink when chosen; 10 under it the
  sample. The face chosen has its border in the ink and a second pixel
  inside it, so nothing moves. The whole box is pressed as the face.
- **The sample** (item 901) is the owner's one sentence, *If you live
  each day as if it was your last, someday you’ll most certainly be
  right.*, 16 px 400 in the ink on lines of 25, its room never less than
  three lines so every face is as tall, fixed, set in that face whatever
  the page is set in, and never cut: at the design's width Inter and DM
  Sans take three lines and Manrope, Geist, Plus Jakarta Sans and
  Instrument Sans two; in the least window it is four lines, whole, and the faces taller. (In the panel at the design's size each face is
  327 wide; in the least window, 203.)
- **Effect**: choosing a face sets every word of the panel, the rail and
  the first screen in it at once — both renderers draw again, neither is
  started anew — and never the mark or the greeting, and never a card's
  page, whose renderer is never handed these faces. The link strip of an
  expanded card is drawn by the Chromium alo carries, which is not told the
  face (item 852).
- **By keys** (item 856), as the ARIA radio group pattern and a Mac's
  radio buttons have it: **Tab** puts the keyboard on its ×, Tab again on
  the radio group *Font* — on the face chosen — and Tab on through *alo at
  work* and back round; **⇧Tab** the other way.
  On *Font*, **← ↑** choose the face before the one chosen and **→ ↓** the
  one after, round from either end, set and kept at once as a press sets
  and keeps it. On its ×, **Space** or **Return** presses it. Escape
  closes *Settings* wherever the keyboard is, as before. What the keyboard
  is on is drawn with **a ring**, 3 px of the ink at a third, outside its
  edge and following its rounding — around its ×, or around the face
  chosen, outside its ink edge — **only once Tab has put the keyboard
  there**, as a browser draws
  `:focus-visible`: opening *Settings*, by its row or otherwise, puts it on
  nothing, and a press of the pointer anywhere puts the ring away. Nothing
  moves for it. While *Search chats* has the keyboard these keys are its.
  How the rest of alo's interface takes keys beyond its fields — the
  panel, the rail, a card's own bar — is still 850's.
- **What an agent reads**: **a dialog named *Settings***, modal, holding
  its heading, its × named *Close*, and a region named *Font* holding its
  heading and **a radio group named *Font*** holding a
  radio named by each face — *Inter*, *Geist*, *DM Sans*, *Plus Jakarta
  Sans*, *Instrument Sans* — the one chosen checked — and *alo at work*'s
  region. The names, the circle and the samples are hidden from it, and so
  is everything of the conversation under it. Never a coordinate (ADR
  0002).
- **Kept** in `settings.txt` in the chats' directory (ADR 0092 § 1) — so
  `alo --conversation --chats DIR` keeps a hand run's choice apart from a
  person's, as it keeps its chats — `0600` in a `0700` directory, written
  whole or not at all (`.writing`, flushed, renamed, the directory
  flushed), not sealed (a face says nothing about a person, and a choice
  that needed the keychain would be lost with it). It reads `# alo: what a
  person chose in Settings (docs/design/shell.md)` then `font` and the
  face's name: `font Plus Jakarta Sans`. Written as the face is chosen,
  and said on the terminal; one that cannot be written is said in the
  first screen's line, and the interface is set in it until alo closes.
- **alo at work** (item 878, asked by the owner with 821 on 8 November
  2026), 24 under the faces: **alo at work**, 15 px 600, and in quiet
  13.5 px *How far alo goes on one task. It always asks before it pays,
  sends or deletes.*; 14 under it one white box, a line of 1, rounded 14,
  16 in either side, of three rows 44 tall ruled off from each other by a
  line of `#eee8de` — each its name at its left, 13.5 px 600 in the ink:
  - **Sites a task may open** — 1 to 10, **3** unless set — and **Actions
    a task may take** — 5 to 100 in fives, **20** unless set — each with a
    stepper at its right, 28 tall, a line of 1 rounded 8: **−** (*Fewer
    sites*), the number 40 wide between rules, **+** (*More sites*). A
    button that would pass its end is quiet and half seen and does
    nothing. The bounds, and why: one site is the least that lets a run
    begun with no page do anything; ten is past any task a person follows
    step by step; five actions searches, opens and still acts; a hundred is
    five times the default, and the run's token limit (about $0.40) still
    stops a long run on a large page first; in fives, so the keys reach
    either end in nineteen presses.
  - **Ask before opening** — three radios at its right, drawn as the
    faces' are: **Never · Default**, **A site not found or named**, and
    **Any site** (item 884; two before it, *A site not found or named* the
    default).
  Asking before paying, sending, deleting or sending a form, and never
  typing a password or a card's details, are not settings. **By keys**:
  Tab goes on from *Font* to the sites, the actions and *Ask before
  opening*, then round to its ×; on a number ← ↓ make it one
  step fewer and → ↑ one more, stopping at its ends (a number is not a
  choice among equals, and 100 going round to 5 would surprise); on the
  radio group → ↑ choose the next and ← ↓ the one before, round from either
  end, as a Mac's radio buttons do. The keyboard's ring as on
  *Font* — around the stepper, or the choice made. **Read at each run's
  start**: a run keeps the limits it started with. **To an agent**: a
  region named *alo at work*, each number a `spinbutton` named by its row
  holding the number beside *Fewer …* and *More …* buttons, and a radio
  group *Ask before opening*. In the design's window what the panel holds
  is 752 tall in a room of 692, so it scrolls 60.
- **Scrolled** (item 880) as the conversation is, **inside its panel**
  (886): what the panel holds, from 24 above *Font* to 28 below *alo at
  work*, scrolls in the room under the panel's heading — from 96 down the
  first screen to 32 above its foot, once the panel is as tall as it can
  be, which is the only time anything scrolls in it — by the wheel over
  the first screen (a wheel's lines glided), the paging keys — Page Up and
  Down, Home and End, and Space and ⇧Space while the keyboard is on
  nothing of it — and the bar, in the panel's right gutter, 13 in from
  its right edge, faded as the conversation's is, pressed, dragged and
  held as it is. As Tab moves the keyboard, what it
  moves to is scrolled into view, 16 clear of the top or the foot, once it
  is drawn there, and no further; a person's wheel afterwards is not
  pulled back to it. It opens at its top each time. To an agent the bar
  is an image *Settings goes on below; scrolled 0% of the way down*.
- **Kept with the font**: `settings.txt` holds `sites 3`, `actions 20` and
  `asks never` (or `not-found-or-named`, or `every`) after the `font`
  line, written whole as a face is; a file from before 878 holds only the
  font, and its limits are the default. **A file from before 884** says
  `asks unnamed`, its old default — but every such file wrote it whether
  the person chose it or not, so it cannot tell a choice from the default:
  it is read as the new default, *Never*, and nothing is said. A person
  who wants to be asked chooses so again.
- **A file that does not read** — a face alo does not carry, a line it
  does not know, no `font` line or two, a limit past its bounds or named
  twice, bytes that are not text, more than 4,096 bytes, or a file that
  cannot be opened — is **Manrope (Inter before item 975) and the default
  limits, and said**,
  with the file's path and why, on the terminal and in the first screen's
  line as the window opens; the file is left as it is until a face is next
  chosen, which writes it whole. No file is Manrope, said nothing; a file
  from before item 975 saying `font Inter` is read as it says, Inter.
- **Search and answers** (items 904, 905, ADR 0106; the owner's word of 9
  November 2026, *"since it will be open source we should not be brave
  hardcoded"*), 24 under *alo at work*: **Search and answers**, 15 px 600,
  and in quiet 13.5 px *The services alo searches and writes its answers
  with. A change is used from alo’s next start.*; 14 under it *alo at
  work*'s white box, its rows ruled off as its are:
  - **Search with** — a radio for each search service at its right, drawn
    as *Ask before opening*'s: **Brave Search** (the default) and
    **SearXNG**. **Answers by** — **OpenAI**. A service's own name is drawn
    here and **nowhere else** in alo (ADR 0106 § 5): choosing a service is
    choosing it by name.
  - Under a chooser, a row for each field its service takes — a
    SearXNG's **Address**, the **Search key**, the **AI key** — 72 tall: 10
    above, then at its right the field, 240 × 28, a line of 1 rounded 8,
    its words 13 px on one line (a key's as dots, `•`, as many as were
    typed, at most 48), *https://search.example.org* or *Paste your own
    key* quiet when empty, the caret 1 × 16 in the ink at its end while it
    has the keyboard, its edge then in the ink; 8 beside it **Keep**, and
    for a person's own key **Remove**, each 28 tall, 13 px 600, a line of
    1 rounded 8; under them, the row's whole width, 12.5 px quiet: whose
    key is used — *Your own key is used, kept in this Mac’s keychain.*,
    *The key this application carries is used.*, *A key in this Mac’s
    keychain is used.*, *No key yet.* — or *Searches go to ADDRESS.* /
    *No address yet.*
  - 10 under the box, 13 px in the ink, what the last *Keep* or *Remove*
    came to: *Your key for the search service is kept in this Mac’s
    keychain. alo uses it from its next start.*, *… was removed from this
    Mac’s keychain.*, *The address is kept. alo searches there from its
    next start.*, *That is not a site’s address: it begins https:// or
    http://.*, *Paste or type a key first.*
  **By keys**: Tab goes on from *Ask before opening* to the search
  chooser, its fields and buttons, the AI chooser, its, and round to the
  ×; on a chooser → ↑ choose the next and ← ↓ the one before, round; Space
  or Return on a button presses it. In a field, what is typed and ⌘V's
  first line go at its end, ⌫ takes the last character, Escape empties it
  (Escape in an empty one closes *Settings*), Return keeps it. A press on a
  field puts the keyboard in it. (Moving the caret, selecting, and a press
  placing it are 909.) **A key** typed is held by the browser process
  alone, and the renderer sent only its dots; kept, it goes to this Mac's
  keychain (`alo-search-brave`, `alo-ai-openai`) and never to
  `settings.txt`, a log or a chat; closing *Settings* forgets what was
  typed. **To an agent**: a region named *Search and answers*, radio groups
  *Search with* and *Answers by* with a radio named by each service, a text
  box named by each field — an address read, a key's dots never — and
  *Keep …* and *Remove …* buttons named by what they do. **Kept**: `search
  brave|searxng`, `searxng ADDRESS` and `answers openai` after the limits
  in `settings.txt`, only those a person chose. In the design's window the
  panel holds 1095 in a room of 692 with SearXNG chosen.
- **Privacy** (item 920, ADR 0108; the owner's word of 10 November 2026),
  24 under *Search and answers*: **Privacy**, 15 px 600, and in quiet 13.5
  px *What sites may keep on this Mac, and how alo answers their cookie
  banners. A cookie choice is used from the next page alo opens.*; 14
  under it *alo at work*'s white box of three rows ruled off as its are,
  each 10 above and below, its name (13.5 px 600) over its choices, one
  under another 2 apart, each 28 tall and drawn as *Ask before opening*'s
  (a radio 16 across, its words 13 px, quiet or the ink when chosen, its
  room 8 past the row either side):
  - **Cookies** — **Refuse other sites’ cookies · Default**, **Refuse all
    cookies** with, under its words in quiet 12.5 px, *Signing in to sites
    will fail: a site cannot remember you.*, and **Keep all cookies**.
  - **Cookie banners** — **Answer with the most private choice ·
    Default** and **Leave them to me**.
  - **Sign-ins** (item 935, ADR 0111) — its name, 4 under it in quiet
    12.5 px the line the browser process sends: *Signed-in sites are kept
    on this Mac, sealed under a key in your keychain, until you forget
    them.*, or after a press *Every sign-in on this Mac was forgotten.
    Sites you sign in to from now on are kept again.*, or why they are
    not kept (another alo open, no key, a file that does not open); 4
    under it the button **Forget all sign-ins**, 28 tall, as wide as its
    words, drawn as *Search and answers*' buttons. The row 98 tall.
  **By keys**: Tab goes on from *Search and answers*' last stop to
  *Cookies*, then *Cookie banners*, then *Forget all sign-ins* (Space or
  Return presses it), then round to the ×; on either group → ↑
  choose the next and ← ↓ the one before, round. The ring around the
  choice made, as on *Ask before opening*. **Effect**: a cookie choice is
  told to the whole browser process at once and used by every Chromium
  started after it — each page opened from the box starts one — so from
  the next page alo opens; a page already open keeps the choice it opened
  with. A banners' choice is used from the next page loaded, the open
  card's included. **To an agent**: a region named *Privacy*, radio groups
  *Cookies* and *Cookie banners* each holding a radio named by its choice,
  the one made checked; the line under *Refuse all cookies* read after it;
  a button *Forget all sign-ins* described by the line over it.
  **Kept**: `cookies all|keep` and `banners leave` at the end of
  `settings.txt`, each only when it is not the default. In the design's
  window, without *Search and answers*, what the panel holds is 1202 in a
  room of 692. *Forget all sign-ins* is kept nowhere: it is done — the
  jar, the file and the open card's cookies emptied, the key replaced.
- **Screen readers** (item 989, ADR 0115 *Phase (c)'s guard*; the
  owner's answers of 13 November 2026), 24 under *Privacy*: **Accessibility
  bus**, 15 px 600, and in quiet 13.5 px *What a screen reader, or any
  other program you let read the screen, may read of alo and do in it. A
  choice is used at once.*; 14 under it a white box as *Privacy*'s, of one
  row, **What they may do** (13.5 px 600), over three
  choices drawn as *Privacy*'s: **Read and act · Default**; **Read only**
  with, under its words in quiet 12.5 px, *A screen reader can read alo,
  but nothing it presses is done.*; **Off** with *A screen reader will not
  be able to read alo.* Each 28 tall, 45 with its line; the row 166 tall,
  the section 242. **By keys**: Tab goes on from *Forget all sign-ins* to
  the group, then round to the ×; → ↑ choose the next and ← ↓ the one
  before, round; ⇧Tab from nothing lands on it first, as the last stop.
  **Effect**, at once: *Read and act* publishes alo's interface and a
  card's page and takes a press, text put in and the keyboard moved, as a
  person's would be; *Read only* publishes the same and refuses every
  action, each refusal in the record; *Off* publishes the window alone —
  no panel, no first screen, no page — and reads no card's page. **Only a
  person's press changes it**: a press from the bus that comes to one of
  its choices is refused (ADR 0026's rule for grants), and nothing over the
  agent socket reaches *Settings*. **Why on by default**: a person who
  cannot see the screen cannot find a switch that is off without the tree
  it withholds. Under *Off* they turn it back on by keys alone — ⌘, opens
  *Settings*, ⇧Tab lands on the group, → reaches *Read and act* (tested
  through System Events) — or by removing the `bus off` line from
  `settings.txt`, which is read at launch. **To an agent**: a region named
  *Screen readers*, a radio group *What they may do*
  with a radio named by each choice, the one made checked, the line under
  *Read only* and *Off* read after it. **Kept**: `bus read|off` at the end
  of `settings.txt`, only when it is not the default; a file naming none,
  or one that does not read, is *Read and act*. In the design's window,
  without *Search and answers*, what the panel holds is 1468 in a room of
  692.
- **Updates** (item 721, ADR 0116 § 4; the owner's answers of 14 November
  2026), 24 under *Screen readers*: **Updates**, 15 px 600, and in quiet
  13.5 px alo's version and the Chromium it was built with (*alo 0.4.1,
  built with CEF 156.0.14 (Chromium 156.0.8078.48).*); 14 under it a white
  box as *Privacy*'s. **In a copy that updates itself**, two rows: **Check
  for updates automatically** (13.5 px 600) over two choices drawn as
  *Privacy*'s — **On · Default** with *Once a day. It sends nothing about
  you.*, and **Off** with *alo sends nothing until you press Check now, and
  may miss Chromium’s security fixes.* — each 45 tall with its line, the
  row 136; then **Last checked** over what the last check found, in
  quiet (*14 November 2026 at 09:00 UTC: alo 0.4.1 is up to date.*, or *Not
  checked yet.*), and **Check now** under it, drawn as *Forget all
  sign-ins*, the row 98; the section 292. **In one that does not** — every
  build of this repository today, which is given no update address — one
  row, **This copy**, over *This copy of alo does not update itself.*
  (inside a Flatpak, *Updates come from your software source.*), nothing
  to press; the section 119. **By keys**: Tab goes on from *Screen
  readers* to the group and then *Check now*, in a copy that updates
  itself; either arrow chooses the other; Space or Return presses *Check
  now*. **Effect**: *On* checks at launch and every 24 hours while alo
  runs; *Off* sends nothing until *Check now* is pressed; an update ready
  is said in the first screen's line — *alo 0.4.2 is ready. It installs
  when you quit alo.* — and installs only when alo quits. That line is the
  browser's own, along the window's bottom, 32 tall in `--warning-tint`
  under a rule of `--border-default`, kept until alo quits and shown
  whenever the line has nothing else to say (item 990); at its right end,
  12 in and centred down it, a button **Quit and update**, 128 by 24, in
  `--bg-surface` ringed 1 px in `--warning-ink`, its 13 px label in the
  same ink, the words cut 12 short of it. Pressed while an agent works in
  any chat — a run at work, or held for the person — the line says *An
  agent is working. alo will update when you quit.* until the next press,
  and alo stays; otherwise alo quits as a person's quit does. It is not on
  the accessibility bus or the keyboard yet, and no agent reaches it. The first time a
  copy that updates itself starts with the check on, the first screen's
  line says once *alo checks for security updates once a day. It sends
  nothing about you. Settings › Updates turns it off.* **To an agent**: a
  region named *Updates*, a radio group *Check for updates automatically*
  with a radio named by each choice, the one made checked, the line under
  each read after it, and a button *Check now* described by the line over
  it; nothing over the agent socket reaches it. **Kept**: `updates off` at
  the end of `settings.txt`, only when it is not the default; a file naming
  none, or one that does not read, checks. In the design's window, without
  *Search and answers*, what the panel holds is 1784 in a room of 692
  (1611 in a copy that does not update itself).

## Colours

ADR 0074 § 2 is the palette, amended 12 November 2026 (item 974, ADR 0112
*Answered* 1). The rule that matters most: **deep teal `#0F6B72` means alo
is acting**, appears nowhere else, and **is never drawn alone** — always with
alo's mark and a word. Terracotta, its colour until then, no longer means
it. The ink is alo OS's navy `#102A43`. A finished chat, and *Done in N
steps*, are in that ink with no green (item 985). Outside the conversation,
`alo PAGE`'s ring around the box a verb acted on and `alo --chrome`'s
highlight are the teal, and the line saying what the verb did carries alo's
mark, 16 px, before its words (item 984).

## What is not designed yet

Dark mode, a phone-width window, downloads and bookmarks, and the
settings other than *Font*, *alo at work*, *Search and answers*,
*Privacy*, *Screen readers* and *Updates* — *What alo may do* among
them (item 705) — and with them *Settings*' list of its sections.
*Settings* is walked by keys (item 856); the rest of alo's interface,
beyond its two fields and *Settings*, is pressed with the pointer only
(item 850).
